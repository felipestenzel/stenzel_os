//! Inter-Process Communication (IPC)
//!
//! System V IPC: shared memory, semaphores, message queues.
//! Linux-specific: eventfd, namespaces.

#![allow(dead_code)]

pub mod shm;
pub mod msg;
pub mod eventfd;
pub mod namespace;

pub use shm::{
    sys_shmget, sys_shmat, sys_shmdt, sys_shmctl,
    ShmId, IPC_CREAT, IPC_EXCL, IPC_PRIVATE, IPC_RMID, IPC_STAT, IPC_SET,
};

pub use msg::{
    sys_msgget, sys_msgsnd, sys_msgrcv, sys_msgctl,
    MsgId, IPC_NOWAIT, MSG_NOERROR, MSG_EXCEPT, MSG_COPY,
    MSGMAX, MSGMNB, MSGMNI,
};

pub use eventfd::{
    sys_eventfd, sys_eventfd2,
    EventFdFile, EFD_CLOEXEC, EFD_NONBLOCK, EFD_SEMAPHORE,
};

/// Initialize IPC subsystem
pub fn init() {
    shm::init();
    msg::init();
    crate::kprintln!("ipc: IPC subsystem initialized (shm, msg)");
}
