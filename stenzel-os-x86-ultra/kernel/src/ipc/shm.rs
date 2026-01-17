//! System V Shared Memory
//!
//! Implementation of shmget, shmat, shmdt, shmctl syscalls for inter-process
//! communication via shared memory segments.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU32, Ordering};
use x86_64::structures::paging::{Page, PageTableFlags, PhysFrame, Size4KiB};
use x86_64::VirtAddr;

use crate::mm;
use crate::sched;
use crate::sync::IrqSafeMutex;
use crate::time;

/// Shared memory segment ID type
pub type ShmId = i32;

/// IPC key for creating/finding segments
pub type IpcKey = i32;

// IPC flags
pub const IPC_CREAT: i32 = 0o1000;   // Create if doesn't exist
pub const IPC_EXCL: i32 = 0o2000;    // Fail if exists (with CREAT)
pub const IPC_PRIVATE: IpcKey = 0;   // Private key (always creates new)

// shmctl commands
pub const IPC_RMID: i32 = 0;  // Remove identifier
pub const IPC_STAT: i32 = 2;  // Get shmid_ds structure
pub const IPC_SET: i32 = 1;   // Set shmid_ds parameters

// shmat flags
pub const SHM_RDONLY: i32 = 0o10000;  // Attach read-only
#[allow(dead_code)]
pub const SHM_RND: i32 = 0o20000;     // Round attach address

/// Permission mode (low 9 bits like file permissions)
#[derive(Debug, Clone, Copy)]
pub struct ShmPerm {
    pub key: IpcKey,
    pub uid: u32,
    pub gid: u32,
    pub cuid: u32,  // Creator UID
    pub cgid: u32,  // Creator GID
    pub mode: u16,  // Permissions
}

impl ShmPerm {
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

    /// Check if user can read the segment
    fn can_read(&self, uid: u32, gid: u32) -> bool {
        if uid == 0 { return true; }  // Root
        if uid == self.uid { return self.mode & 0o400 != 0; }
        if gid == self.gid { return self.mode & 0o040 != 0; }
        self.mode & 0o004 != 0
    }

    /// Check if user can write the segment
    fn can_write(&self, uid: u32, gid: u32) -> bool {
        if uid == 0 { return true; }  // Root
        if uid == self.uid { return self.mode & 0o200 != 0; }
        if gid == self.gid { return self.mode & 0o020 != 0; }
        self.mode & 0o002 != 0
    }
}

/// Shared memory segment status (shmid_ds structure)
#[repr(C)]
#[derive(Debug, Clone)]
pub struct ShmIdDs {
    pub shm_perm: ShmPerm,
    pub shm_segsz: usize,     // Segment size in bytes
    pub shm_atime: u64,       // Last attach time
    pub shm_dtime: u64,       // Last detach time
    pub shm_ctime: u64,       // Last change time
    pub shm_cpid: u32,        // Creator PID
    pub shm_lpid: u32,        // Last shmat/shmdt PID
    pub shm_nattch: u16,      // Current # of attaches
}

/// Internal shared memory segment
struct ShmSegment {
    /// Unique ID
    id: ShmId,
    /// Status structure
    ds: ShmIdDs,
    /// Physical frames backing this segment
    frames: Vec<PhysFrame<Size4KiB>>,
    /// Marked for deletion (IPC_RMID called)
    marked_for_removal: bool,
}

fn current_time_secs() -> u64 {
    time::realtime().tv_sec as u64
}

impl ShmSegment {
    fn new(id: ShmId, key: IpcKey, size: usize, uid: u32, gid: u32, mode: u16, pid: u32) -> Self {
        let now = current_time_secs();
        Self {
            id,
            ds: ShmIdDs {
                shm_perm: ShmPerm::new(key, uid, gid, mode),
                shm_segsz: size,
                shm_atime: 0,
                shm_dtime: 0,
                shm_ctime: now,
                shm_cpid: pid,
                shm_lpid: pid,
                shm_nattch: 0,
            },
            frames: Vec::new(),
            marked_for_removal: false,
        }
    }
}

/// Attachment tracking (which process attached where)
#[derive(Debug, Clone)]
struct ShmAttach {
    /// Process ID (task id)
    pid: u64,
    /// Virtual address where attached
    addr: u64,
    /// Size of mapping
    size: usize,
    /// Read-only?
    #[allow(dead_code)]
    readonly: bool,
}

/// Global shared memory manager
struct ShmManager {
    /// Next available segment ID
    next_id: AtomicU32,
    /// Segments by ID
    segments: BTreeMap<ShmId, ShmSegment>,
    /// Key to ID mapping (for non-private keys)
    key_to_id: BTreeMap<IpcKey, ShmId>,
    /// Attachments per segment
    attachments: BTreeMap<ShmId, Vec<ShmAttach>>,
}

impl ShmManager {
    fn new() -> Self {
        Self {
            next_id: AtomicU32::new(1),
            segments: BTreeMap::new(),
            key_to_id: BTreeMap::new(),
            attachments: BTreeMap::new(),
        }
    }

    fn alloc_id(&self) -> ShmId {
        self.next_id.fetch_add(1, Ordering::SeqCst) as ShmId
    }
}

static SHM_MANAGER: IrqSafeMutex<Option<ShmManager>> = IrqSafeMutex::new(None);

/// Initialize shared memory subsystem
pub fn init() {
    let mut guard = SHM_MANAGER.lock();
    *guard = Some(ShmManager::new());
}

/// shmget - Get shared memory segment
///
/// key: IPC key (IPC_PRIVATE for new private segment)
/// size: Segment size (rounded up to page size)
/// shmflg: Flags (IPC_CREAT, IPC_EXCL, permission mode)
pub fn sys_shmget(key: IpcKey, size: usize, shmflg: i32) -> i64 {
    if size == 0 {
        return -22; // EINVAL
    }

    let task = sched::current_task();
    let cred = sched::current_cred();
    let uid = cred.uid.0;
    let gid = cred.gid.0;
    let pid = task.id() as u32;

    let mut guard = SHM_MANAGER.lock();
    let manager = match guard.as_mut() {
        Some(m) => m,
        None => return -1, // EPERM
    };

    // Check if segment with this key exists (for non-private keys)
    if key != IPC_PRIVATE {
        if let Some(&existing_id) = manager.key_to_id.get(&key) {
            // Key exists
            if shmflg & IPC_CREAT != 0 && shmflg & IPC_EXCL != 0 {
                return -17; // EEXIST
            }

            // Return existing segment ID
            if let Some(seg) = manager.segments.get(&existing_id) {
                // Check permissions
                if !seg.ds.shm_perm.can_read(uid, gid) {
                    return -13; // EACCES
                }

                // Check size
                if seg.ds.shm_segsz < size {
                    return -22; // EINVAL
                }

                return existing_id as i64;
            }
        } else if shmflg & IPC_CREAT == 0 {
            // Key doesn't exist and IPC_CREAT not set
            return -2; // ENOENT
        }
    }

    // Create new segment
    let mode = (shmflg & 0o777) as u16;
    let aligned_size = (size + 4095) & !4095;
    let num_pages = aligned_size / 4096;

    let id = manager.alloc_id();
    let mut segment = ShmSegment::new(id, key, aligned_size, uid, gid, mode, pid);

    // Allocate physical frames
    {
        let mut fa = mm::frame_allocator_lock();
        for _ in 0..num_pages {
            match fa.allocate() {
                Some(frame) => {
                    // Zero the frame
                    let virt = mm::phys_to_virt(frame.start_address());
                    unsafe {
                        core::ptr::write_bytes(virt.as_mut_ptr::<u8>(), 0, 4096);
                    }
                    segment.frames.push(frame);
                }
                None => {
                    // Free already allocated frames
                    for frame in segment.frames.drain(..) {
                        fa.deallocate(frame);
                    }
                    return -12; // ENOMEM
                }
            }
        }
    }

    // Register segment
    if key != IPC_PRIVATE {
        manager.key_to_id.insert(key, id);
    }
    manager.attachments.insert(id, Vec::new());
    manager.segments.insert(id, segment);

    id as i64
}

/// shmat - Attach shared memory segment
///
/// shmid: Segment ID from shmget
/// shmaddr: Requested address (0 for kernel to choose)
/// shmflg: Flags (SHM_RDONLY, SHM_RND)
///
/// Returns: Virtual address where segment is attached, or -1 on error
pub fn sys_shmat(shmid: ShmId, shmaddr: u64, shmflg: i32) -> i64 {
    let task = sched::current_task();
    let cred = sched::current_cred();
    let uid = cred.uid.0;
    let gid = cred.gid.0;
    let pid = task.id();

    let readonly = shmflg & SHM_RDONLY != 0;

    // First, get segment info and validate permissions
    let (frames, size) = {
        let mut guard = SHM_MANAGER.lock();
        let manager = match guard.as_mut() {
            Some(m) => m,
            None => return -22, // EINVAL
        };

        let seg = match manager.segments.get_mut(&shmid) {
            Some(s) => s,
            None => return -22, // EINVAL
        };

        // Check if marked for removal and no attachments
        if seg.marked_for_removal && seg.ds.shm_nattch == 0 {
            return -43; // EIDRM
        }

        // Check permissions
        if readonly {
            if !seg.ds.shm_perm.can_read(uid, gid) {
                return -13; // EACCES
            }
        } else {
            if !seg.ds.shm_perm.can_write(uid, gid) {
                return -13; // EACCES
            }
        }

        (seg.frames.clone(), seg.ds.shm_segsz)
    };

    // Find a free address in user space
    let addr = {
        let mut vma_guard = mm::vma::manager_lock();
        let vma_manager = match vma_guard.as_mut() {
            Some(v) => v,
            None => return -22, // EINVAL
        };

        // Use VmaManager to allocate address space
        let prot = if readonly {
            mm::vma::Protection::READ
        } else {
            mm::vma::Protection::READ_WRITE
        };

        let map_flags = mm::vma::MapFlags {
            shared: true,
            private: false,
            anonymous: false,
            fixed: shmaddr != 0,
        };

        match vma_manager.mmap(shmaddr, size, prot, map_flags) {
            Ok(a) => a,
            Err(_) => return -12, // ENOMEM
        }
    };

    // Map the shared frames into the address space
    {
        let mut mapper = mm::mapper_lock();
        let mut fa = mm::frame_allocator_lock();

        let page_flags = if readonly {
            PageTableFlags::PRESENT | PageTableFlags::USER_ACCESSIBLE | PageTableFlags::NO_EXECUTE
        } else {
            PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE | PageTableFlags::NO_EXECUTE
        };

        for (i, frame) in frames.iter().enumerate() {
            let page_addr = addr + (i * 4096) as u64;
            let page = Page::<Size4KiB>::containing_address(VirtAddr::new(page_addr));

            // Map the shared frame
            if let Err(_) = mapper.map_page(page, *frame, page_flags, &mut *fa) {
                // Rollback on failure
                for j in 0..i {
                    let prev_addr = addr + (j * 4096) as u64;
                    let prev_page = Page::<Size4KiB>::containing_address(VirtAddr::new(prev_addr));
                    let _ = mapper.unmap_page_simple(prev_page);
                }
                return -12; // ENOMEM
            }
        }
    }

    // Update segment stats and record attachment
    {
        let mut guard = SHM_MANAGER.lock();
        let manager = match guard.as_mut() {
            Some(m) => m,
            None => return addr as i64,
        };

        if let Some(seg) = manager.segments.get_mut(&shmid) {
            seg.ds.shm_nattch += 1;
            seg.ds.shm_atime = current_time_secs();
            seg.ds.shm_lpid = pid as u32;
        }

        // Record attachment
        let attach = ShmAttach {
            pid,
            addr,
            size,
            readonly,
        };
        if let Some(attachments) = manager.attachments.get_mut(&shmid) {
            attachments.push(attach);
        }
    }

    addr as i64
}

/// shmdt - Detach shared memory segment
///
/// shmaddr: Address returned by shmat
pub fn sys_shmdt(shmaddr: u64) -> i64 {
    let task = sched::current_task();
    let pid = task.id();

    // Find which segment this address belongs to
    let (shmid, size) = {
        let guard = SHM_MANAGER.lock();
        let manager = match guard.as_ref() {
            Some(m) => m,
            None => return -22, // EINVAL
        };

        let mut found = None;
        for (id, attachments) in &manager.attachments {
            for attach in attachments {
                if attach.pid == pid && attach.addr == shmaddr {
                    found = Some((*id, attach.size));
                    break;
                }
            }
            if found.is_some() { break; }
        }

        match found {
            Some(f) => f,
            None => return -22, // EINVAL - not attached at this address
        }
    };

    // Unmap the pages (don't free physical frames - they belong to the segment)
    {
        let mut mapper = mm::mapper_lock();
        let num_pages = size / 4096;
        for i in 0..num_pages {
            let page_addr = shmaddr + (i * 4096) as u64;
            let page = Page::<Size4KiB>::containing_address(VirtAddr::new(page_addr));
            // Unmap but don't deallocate the frame
            let _ = mapper.unmap_page_simple(page);
        }
    }

    // Remove from VMA manager
    {
        let mut vma_guard = mm::vma::manager_lock();
        if let Some(vma_manager) = vma_guard.as_mut() {
            // Remove the VMA entry (without freeing frames since they're shared)
            let _ = vma_manager.remove_vma(shmaddr);
        }
    }

    // Update segment stats and remove attachment
    let should_destroy = {
        let mut guard = SHM_MANAGER.lock();
        let manager = match guard.as_mut() {
            Some(m) => m,
            None => return 0,
        };

        // Remove attachment record
        if let Some(attachments) = manager.attachments.get_mut(&shmid) {
            attachments.retain(|a| !(a.pid == pid && a.addr == shmaddr));
        }

        // Update segment stats
        let should_destroy = if let Some(seg) = manager.segments.get_mut(&shmid) {
            if seg.ds.shm_nattch > 0 {
                seg.ds.shm_nattch -= 1;
            }
            seg.ds.shm_dtime = current_time_secs();
            seg.ds.shm_lpid = pid as u32;

            seg.marked_for_removal && seg.ds.shm_nattch == 0
        } else {
            false
        };

        should_destroy
    };

    // Destroy segment if marked and no more attachments
    if should_destroy {
        destroy_segment(shmid);
    }

    0
}

/// shmctl - Shared memory control
///
/// shmid: Segment ID
/// cmd: Command (IPC_RMID, IPC_STAT, IPC_SET)
/// buf: User buffer for stat/set operations
pub fn sys_shmctl(shmid: ShmId, cmd: i32, buf: u64) -> i64 {
    let cred = sched::current_cred();
    let uid = cred.uid.0;
    let gid = cred.gid.0;

    let mut guard = SHM_MANAGER.lock();
    let manager = match guard.as_mut() {
        Some(m) => m,
        None => return -22, // EINVAL
    };

    match cmd {
        IPC_STAT => {
            let seg = match manager.segments.get(&shmid) {
                Some(s) => s,
                None => return -22, // EINVAL
            };

            // Check read permission
            if !seg.ds.shm_perm.can_read(uid, gid) {
                return -13; // EACCES
            }

            if buf == 0 || !crate::syscall::is_user_range(buf, core::mem::size_of::<ShmIdDs>()) {
                return -14; // EFAULT
            }

            // Copy shmid_ds to user buffer
            unsafe {
                let user_ds = buf as *mut ShmIdDs;
                core::ptr::write(user_ds, seg.ds.clone());
            }

            0
        }

        IPC_SET => {
            let seg = match manager.segments.get_mut(&shmid) {
                Some(s) => s,
                None => return -22, // EINVAL
            };

            // Only owner or root can set
            if uid != 0 && uid != seg.ds.shm_perm.uid {
                return -1; // EPERM
            }

            if buf == 0 || !crate::syscall::is_user_range(buf, core::mem::size_of::<ShmIdDs>()) {
                return -14; // EFAULT
            }

            // Read from user buffer
            let user_ds = unsafe { &*(buf as *const ShmIdDs) };

            // Only uid, gid, and mode can be set
            seg.ds.shm_perm.uid = user_ds.shm_perm.uid;
            seg.ds.shm_perm.gid = user_ds.shm_perm.gid;
            seg.ds.shm_perm.mode = user_ds.shm_perm.mode & 0o777;
            seg.ds.shm_ctime = current_time_secs();

            0
        }

        IPC_RMID => {
            let seg = match manager.segments.get_mut(&shmid) {
                Some(s) => s,
                None => return -22, // EINVAL
            };

            // Only owner or root can remove
            if uid != 0 && uid != seg.ds.shm_perm.uid {
                return -1; // EPERM
            }

            // Mark for removal
            seg.marked_for_removal = true;

            // Remove from key mapping
            let key = seg.ds.shm_perm.key;
            if key != IPC_PRIVATE {
                manager.key_to_id.remove(&key);
            }

            // If no attachments, destroy immediately
            if seg.ds.shm_nattch == 0 {
                drop(guard);
                destroy_segment(shmid);
            }

            0
        }

        _ => -22, // EINVAL
    }
}

/// Destroy a shared memory segment and free its resources
fn destroy_segment(shmid: ShmId) {
    let mut guard = SHM_MANAGER.lock();
    let manager = match guard.as_mut() {
        Some(m) => m,
        None => return,
    };

    if let Some(seg) = manager.segments.remove(&shmid) {
        // Free physical frames
        let mut fa = mm::frame_allocator_lock();
        for frame in &seg.frames {
            fa.deallocate(*frame);
        }
    }

    // Remove attachment tracking
    manager.attachments.remove(&shmid);
}

/// Detach all shared memory from a process (called on exit)
#[allow(dead_code)]
pub fn detach_all_from_pid(pid: u64) {
    // Find all attachments for this process
    let attachments_to_remove: Vec<(ShmId, u64)> = {
        let guard = SHM_MANAGER.lock();
        let manager = match guard.as_ref() {
            Some(m) => m,
            None => return,
        };

        let mut to_remove = Vec::new();
        for (shmid, attachments) in &manager.attachments {
            for attach in attachments {
                if attach.pid == pid {
                    to_remove.push((*shmid, attach.addr));
                }
            }
        }
        to_remove
    };

    // Detach each one
    for (_, addr) in attachments_to_remove {
        sys_shmdt(addr);
    }
}
