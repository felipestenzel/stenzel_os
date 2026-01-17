//! System V Message Queues
//!
//! Implementation of msgget, msgsnd, msgrcv, msgctl syscalls for inter-process
//! communication via message queues.
//!
//! Message queues allow processes to exchange data in the form of messages.
//! Each message has a type (long integer) and a body (byte array).

use alloc::collections::{BTreeMap, VecDeque};
use alloc::vec::Vec;
use core::sync::atomic::{AtomicI32, AtomicU32, Ordering};

use crate::sched;
use crate::sync::IrqSafeMutex;
use crate::time;

/// Message queue ID type
pub type MsgId = i32;

/// IPC key for creating/finding queues
pub type IpcKey = i32;

// IPC flags (same as shm)
pub const IPC_CREAT: i32 = 0o1000;   // Create if doesn't exist
pub const IPC_EXCL: i32 = 0o2000;    // Fail if exists (with CREAT)
pub const IPC_NOWAIT: i32 = 0o4000;  // Return error instead of waiting
pub const IPC_PRIVATE: IpcKey = 0;   // Private key (always creates new)

// msgctl commands
pub const IPC_RMID: i32 = 0;  // Remove identifier
pub const IPC_STAT: i32 = 2;  // Get msqid_ds structure
pub const IPC_SET: i32 = 1;   // Set msqid_ds parameters

// msgrcv flags
pub const MSG_NOERROR: i32 = 0o10000;  // Truncate message if too long
pub const MSG_EXCEPT: i32 = 0o20000;   // Receive any message except type
pub const MSG_COPY: i32 = 0o40000;     // Copy instead of remove (peek)

// Default limits
pub const MSGMAX: usize = 8192;        // Max message size
pub const MSGMNB: usize = 16384;       // Max queue size in bytes
pub const MSGMNI: usize = 128;         // Max number of message queues

/// Permission mode (low 9 bits like file permissions)
#[derive(Debug, Clone, Copy)]
pub struct MsgPerm {
    pub key: IpcKey,
    pub uid: u32,
    pub gid: u32,
    pub cuid: u32,  // Creator UID
    pub cgid: u32,  // Creator GID
    pub mode: u16,  // Permissions
}

impl MsgPerm {
    fn new(key: IpcKey, uid: u32, gid: u32, mode: u16) -> Self {
        Self {
            key,
            uid,
            gid,
            cuid: uid,
            cgid: gid,
            mode: mode & 0o777,
        }
    }

    /// Check if user can read from the queue
    fn can_read(&self, uid: u32, gid: u32) -> bool {
        if uid == 0 { return true; }  // Root
        if uid == self.uid { return self.mode & 0o400 != 0; }
        if gid == self.gid { return self.mode & 0o040 != 0; }
        self.mode & 0o004 != 0
    }

    /// Check if user can write to the queue
    fn can_write(&self, uid: u32, gid: u32) -> bool {
        if uid == 0 { return true; }  // Root
        if uid == self.uid { return self.mode & 0o200 != 0; }
        if gid == self.gid { return self.mode & 0o020 != 0; }
        self.mode & 0o002 != 0
    }
}

/// Message in the queue
#[derive(Debug, Clone)]
pub struct Message {
    /// Message type (must be > 0)
    pub mtype: i64,
    /// Message data
    pub data: Vec<u8>,
}

impl Message {
    /// Create a new message
    pub fn new(mtype: i64, data: Vec<u8>) -> Self {
        Self { mtype, data }
    }

    /// Get total size (for queue accounting)
    fn size(&self) -> usize {
        self.data.len()
    }
}

/// Message queue status (msqid_ds structure)
#[repr(C)]
#[derive(Debug, Clone)]
pub struct MsqIdDs {
    /// Permissions
    pub msg_perm: MsgPerm,
    /// Time of last msgsnd
    pub msg_stime: u64,
    /// Time of last msgrcv
    pub msg_rtime: u64,
    /// Time of creation or last modification
    pub msg_ctime: u64,
    /// Current number of bytes in queue
    pub msg_cbytes: u64,
    /// Number of messages in queue
    pub msg_qnum: u64,
    /// Maximum bytes in queue
    pub msg_qbytes: u64,
    /// PID of last msgsnd
    pub msg_lspid: u32,
    /// PID of last msgrcv
    pub msg_lrpid: u32,
}

/// A message queue
struct MessageQueue {
    /// Queue ID
    id: MsgId,
    /// Queue status
    ds: MsqIdDs,
    /// Messages in the queue
    messages: VecDeque<Message>,
    /// Total bytes in queue
    total_bytes: usize,
    /// Whether the queue is marked for removal
    marked_for_removal: bool,
}

impl MessageQueue {
    fn new(id: MsgId, key: IpcKey, uid: u32, gid: u32, mode: u16) -> Self {
        let now = time::ticks();
        Self {
            id,
            ds: MsqIdDs {
                msg_perm: MsgPerm::new(key, uid, gid, mode),
                msg_stime: 0,
                msg_rtime: 0,
                msg_ctime: now,
                msg_cbytes: 0,
                msg_qnum: 0,
                msg_qbytes: MSGMNB as u64,
                msg_lspid: 0,
                msg_lrpid: 0,
            },
            messages: VecDeque::new(),
            total_bytes: 0,
            marked_for_removal: false,
        }
    }

    /// Check if there's space for a message
    fn has_space(&self, msg_size: usize) -> bool {
        self.total_bytes + msg_size <= self.ds.msg_qbytes as usize
    }

    /// Add a message to the queue
    fn send(&mut self, msg: Message, pid: u32) -> bool {
        let msg_size = msg.size();
        if !self.has_space(msg_size) {
            return false;
        }

        self.messages.push_back(msg);
        self.total_bytes += msg_size;
        self.ds.msg_cbytes = self.total_bytes as u64;
        self.ds.msg_qnum = self.messages.len() as u64;
        self.ds.msg_stime = time::ticks();
        self.ds.msg_lspid = pid;
        true
    }

    /// Receive a message from the queue
    ///
    /// - msgtyp == 0: receive first message
    /// - msgtyp > 0: receive first message of type msgtyp
    /// - msgtyp < 0: receive first message with lowest type <= |msgtyp|
    fn receive(&mut self, msgtyp: i64, flags: i32, pid: u32) -> Option<Message> {
        let except = flags & MSG_EXCEPT != 0;

        let idx = if msgtyp == 0 {
            // Receive first message
            if self.messages.is_empty() { None } else { Some(0) }
        } else if msgtyp > 0 {
            // Receive message of specific type
            if except {
                // Receive any message EXCEPT this type
                self.messages.iter().position(|m| m.mtype != msgtyp)
            } else {
                self.messages.iter().position(|m| m.mtype == msgtyp)
            }
        } else {
            // msgtyp < 0: receive lowest type <= |msgtyp|
            let abs_type = msgtyp.unsigned_abs() as i64;
            let mut best_idx = None;
            let mut best_type = i64::MAX;

            for (i, msg) in self.messages.iter().enumerate() {
                if msg.mtype <= abs_type && msg.mtype < best_type {
                    best_type = msg.mtype;
                    best_idx = Some(i);
                }
            }
            best_idx
        };

        idx.and_then(|i| {
            let msg = if flags & MSG_COPY != 0 {
                // Just copy, don't remove
                self.messages.get(i).cloned()
            } else {
                // Remove from queue
                self.messages.remove(i)
            };

            if let Some(ref m) = msg {
                if flags & MSG_COPY == 0 {
                    // Update stats only if we actually removed
                    self.total_bytes -= m.size();
                    self.ds.msg_cbytes = self.total_bytes as u64;
                    self.ds.msg_qnum = self.messages.len() as u64;
                }
                self.ds.msg_rtime = time::ticks();
                self.ds.msg_lrpid = pid;
            }
            msg
        })
    }
}

/// Global message queue table
struct MsgQueueTable {
    /// Queues indexed by ID
    queues: BTreeMap<MsgId, MessageQueue>,
    /// Map from key to ID (for lookup)
    key_to_id: BTreeMap<IpcKey, MsgId>,
    /// Next available ID
    next_id: AtomicI32,
}

impl MsgQueueTable {
    const fn new() -> Self {
        Self {
            queues: BTreeMap::new(),
            key_to_id: BTreeMap::new(),
            next_id: AtomicI32::new(1),
        }
    }

    fn alloc_id(&mut self) -> MsgId {
        loop {
            let id = self.next_id.fetch_add(1, Ordering::Relaxed);
            if id <= 0 {
                // Wrapped around, reset
                self.next_id.store(1, Ordering::Relaxed);
                continue;
            }
            if !self.queues.contains_key(&id) {
                return id;
            }
        }
    }
}

static MSG_TABLE: IrqSafeMutex<MsgQueueTable> = IrqSafeMutex::new(MsgQueueTable::new());

/// Initialize message queue subsystem
pub fn init() {
    // Nothing to initialize, table is statically allocated
    crate::kprintln!("ipc: message queue subsystem initialized");
}

// ============================================================================
// Syscall implementations
// ============================================================================

/// msgget - get a message queue identifier
///
/// If key is IPC_PRIVATE, a new queue is always created.
/// Otherwise, if IPC_CREAT is set, creates a new queue if it doesn't exist.
/// If IPC_EXCL is also set, fails if the queue already exists.
pub fn sys_msgget(key: IpcKey, msgflg: i32) -> i64 {
    let mut table = MSG_TABLE.lock();

    // Check if queue count is at maximum
    if table.queues.len() >= MSGMNI {
        return -crate::syscall::errno::ENOSPC as i64;
    }

    let cred = sched::current_cred();
    let uid = cred.euid.0;
    let gid = cred.egid.0;

    let mode = (msgflg & 0o777) as u16;

    // IPC_PRIVATE always creates a new queue
    if key == IPC_PRIVATE {
        let id = table.alloc_id();
        let queue = MessageQueue::new(id, key, uid, gid, mode);
        table.queues.insert(id, queue);
        return id as i64;
    }

    // Check if queue with this key exists
    if let Some(&id) = table.key_to_id.get(&key) {
        if msgflg & IPC_CREAT != 0 && msgflg & IPC_EXCL != 0 {
            // IPC_CREAT | IPC_EXCL: fail if exists
            return -crate::syscall::errno::EEXIST as i64;
        }

        // Check permissions
        if let Some(queue) = table.queues.get(&id) {
            if !queue.ds.msg_perm.can_read(uid, gid) && !queue.ds.msg_perm.can_write(uid, gid) {
                return -crate::syscall::errno::EACCES as i64;
            }
            return id as i64;
        }
    }

    // Queue doesn't exist
    if msgflg & IPC_CREAT == 0 {
        return -crate::syscall::errno::ENOENT as i64;
    }

    // Create new queue
    let id = table.alloc_id();
    let queue = MessageQueue::new(id, key, uid, gid, mode);
    table.queues.insert(id, queue);
    table.key_to_id.insert(key, id);

    id as i64
}

/// msgsnd - send a message to a message queue
///
/// msgp points to struct { long mtype; char mtext[msgsz]; }
/// Returns 0 on success, -1 on error
pub fn sys_msgsnd(msqid: i32, msgp: u64, msgsz: usize, msgflg: i32) -> i64 {
    use crate::syscall::{errno, validate_user_buffer};

    // Validate message size
    if msgsz > MSGMAX {
        return errno::EINVAL;
    }

    // Need at least 8 bytes for mtype (i64)
    let total_size = 8 + msgsz;
    let user_buf = unsafe {
        match validate_user_buffer(msgp, total_size) {
            Some(buf) => buf,
            None => return errno::EFAULT,
        }
    };

    // Read message type (first 8 bytes)
    let mtype = i64::from_ne_bytes([
        user_buf[0], user_buf[1], user_buf[2], user_buf[3],
        user_buf[4], user_buf[5], user_buf[6], user_buf[7],
    ]);

    // Message type must be positive
    if mtype <= 0 {
        return errno::EINVAL;
    }

    // Read message data
    let data = user_buf[8..8 + msgsz].to_vec();
    let msg = Message::new(mtype, data);

    let cred = sched::current_cred();
    let uid = cred.euid.0;
    let gid = cred.egid.0;
    let pid = sched::current_pid() as u32;

    let mut table = MSG_TABLE.lock();

    let queue = match table.queues.get_mut(&msqid) {
        Some(q) => q,
        None => return errno::EINVAL,
    };

    // Check if marked for removal
    if queue.marked_for_removal {
        return errno::EIDRM;
    }

    // Check write permission
    if !queue.ds.msg_perm.can_write(uid, gid) {
        return errno::EACCES;
    }

    // Try to send
    if queue.send(msg, pid) {
        return 0;
    }

    // Queue is full
    if msgflg & IPC_NOWAIT != 0 {
        return errno::EAGAIN;
    }

    // Would need to block - for now, just return error
    // Full blocking support would require wait queues
    errno::EAGAIN
}

/// msgrcv - receive a message from a message queue
///
/// msgp points to struct { long mtype; char mtext[msgsz]; }
/// Returns number of bytes received on success, -1 on error
pub fn sys_msgrcv(msqid: i32, msgp: u64, msgsz: usize, msgtyp: i64, msgflg: i32) -> i64 {
    use crate::syscall::{errno, validate_user_buffer_mut};

    // Need at least 8 bytes for mtype (i64)
    let total_size = 8 + msgsz;
    let user_buf = unsafe {
        match validate_user_buffer_mut(msgp, total_size) {
            Some(buf) => buf,
            None => return errno::EFAULT,
        }
    };

    let cred = sched::current_cred();
    let uid = cred.euid.0;
    let gid = cred.egid.0;
    let pid = sched::current_pid() as u32;

    let mut table = MSG_TABLE.lock();

    let queue = match table.queues.get_mut(&msqid) {
        Some(q) => q,
        None => return errno::EINVAL,
    };

    // Check if marked for removal
    if queue.marked_for_removal {
        return errno::EIDRM;
    }

    // Check read permission
    if !queue.ds.msg_perm.can_read(uid, gid) {
        return errno::EACCES;
    }

    // Try to receive
    match queue.receive(msgtyp, msgflg, pid) {
        Some(msg) => {
            // Check if message fits
            let copy_size = if msg.data.len() > msgsz {
                if msgflg & MSG_NOERROR == 0 {
                    // Message too big and no truncation allowed
                    // Put it back (simplified - doesn't maintain order)
                    queue.messages.push_front(msg);
                    return errno::E2BIG;
                }
                msgsz
            } else {
                msg.data.len()
            };

            // Write message type
            let mtype_bytes = msg.mtype.to_ne_bytes();
            user_buf[0..8].copy_from_slice(&mtype_bytes);

            // Write message data
            user_buf[8..8 + copy_size].copy_from_slice(&msg.data[..copy_size]);

            copy_size as i64
        }
        None => {
            // No matching message
            if msgflg & IPC_NOWAIT != 0 {
                return errno::ENOMSG;
            }

            // Would need to block - for now, just return error
            // Full blocking support would require wait queues
            errno::ENOMSG
        }
    }
}

/// msgctl - control operations on a message queue
pub fn sys_msgctl(msqid: i32, cmd: i32, buf: u64) -> i64 {
    use crate::syscall::errno;

    let cred = sched::current_cred();
    let uid = cred.euid.0;
    let gid = cred.egid.0;

    let mut table = MSG_TABLE.lock();

    match cmd {
        IPC_RMID => {
            // Remove the queue
            let queue = match table.queues.get(&msqid) {
                Some(q) => q,
                None => return errno::EINVAL,
            };

            // Check permission (must be owner or root)
            if uid != 0 && uid != queue.ds.msg_perm.uid && uid != queue.ds.msg_perm.cuid {
                return errno::EPERM;
            }

            // Remove key mapping
            let key = queue.ds.msg_perm.key;
            if key != IPC_PRIVATE {
                table.key_to_id.remove(&key);
            }

            // Remove the queue
            table.queues.remove(&msqid);
            0
        }

        IPC_STAT => {
            // Get queue status
            let queue = match table.queues.get(&msqid) {
                Some(q) => q,
                None => return errno::EINVAL,
            };

            // Check read permission
            if !queue.ds.msg_perm.can_read(uid, gid) {
                return errno::EACCES;
            }

            // Copy to user buffer
            if buf == 0 {
                return errno::EFAULT;
            }

            let ds_ptr = buf as *mut MsqIdDs;
            unsafe {
                *ds_ptr = queue.ds.clone();
            }
            0
        }

        IPC_SET => {
            // Set queue parameters
            let queue = match table.queues.get_mut(&msqid) {
                Some(q) => q,
                None => return errno::EINVAL,
            };

            // Check permission (must be owner or root)
            if uid != 0 && uid != queue.ds.msg_perm.uid && uid != queue.ds.msg_perm.cuid {
                return errno::EPERM;
            }

            if buf == 0 {
                return errno::EFAULT;
            }

            let ds_ptr = buf as *const MsqIdDs;
            let new_ds = unsafe { &*ds_ptr };

            // Can only set certain fields
            queue.ds.msg_perm.uid = new_ds.msg_perm.uid;
            queue.ds.msg_perm.gid = new_ds.msg_perm.gid;
            queue.ds.msg_perm.mode = new_ds.msg_perm.mode & 0o777;
            queue.ds.msg_qbytes = new_ds.msg_qbytes.min(MSGMNB as u64 * 4); // Limit max size
            queue.ds.msg_ctime = time::ticks();
            0
        }

        _ => errno::EINVAL,
    }
}

// ============================================================================
// Error codes
// ============================================================================

// Add to errno module if not present
pub mod errno {
    pub const EIDRM: i64 = -43;    // Identifier removed
    pub const ENOMSG: i64 = -42;   // No message of desired type
    pub const E2BIG: i64 = -7;     // Argument list too long (message too big)
}
