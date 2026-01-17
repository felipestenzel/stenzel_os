#![allow(dead_code)]

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::sync::{Arc, Weak};
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};

use spin::RwLock;

use crate::syscall::Pipe;
use crate::util::{KError, KResult};

use super::vfs::{DirEntry, Inode, InodeKind, InodeOps, Metadata};

pub struct TmpFs;

impl TmpFs {
    pub fn new_root() -> Inode {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let meta = Metadata::simple(
            crate::security::Uid(0),
            crate::security::Gid(0),
            super::vfs::Mode::from_octal(0o755),
            InodeKind::Dir,
        );

        let root: Arc<TmpfsInode> = Arc::new_cyclic(|weak| TmpfsInode {
            id,
            self_weak: weak.clone(),
            parent: RwLock::new(None),
            meta: RwLock::new(meta),
            node: TmpfsNode::Dir {
                children: RwLock::new(BTreeMap::new()),
            },
        });

        Inode(root)
    }
}

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

enum TmpfsNode {
    File {
        data: RwLock<Vec<u8>>,
    },
    Dir {
        children: RwLock<BTreeMap<String, Inode>>,
    },
    Symlink {
        target: RwLock<String>,
    },
    Fifo {
        pipe: Arc<Pipe>,
    },
}

pub struct TmpfsInode {
    #[allow(dead_code)]
    id: u64,
    self_weak: Weak<TmpfsInode>,
    parent: RwLock<Option<Weak<TmpfsInode>>>,
    meta: RwLock<Metadata>,
    node: TmpfsNode,
}

impl TmpfsInode {
    fn as_dir(&self) -> KResult<&RwLock<BTreeMap<String, Inode>>> {
        match &self.node {
            TmpfsNode::Dir { children } => Ok(children),
            _ => Err(KError::Invalid),
        }
    }

    fn as_file(&self) -> KResult<&RwLock<Vec<u8>>> {
        match &self.node {
            TmpfsNode::File { data } => Ok(data),
            _ => Err(KError::Invalid),
        }
    }

    fn as_symlink(&self) -> KResult<&RwLock<String>> {
        match &self.node {
            TmpfsNode::Symlink { target } => Ok(target),
            _ => Err(KError::Invalid),
        }
    }

    fn as_fifo(&self) -> KResult<&Arc<Pipe>> {
        match &self.node {
            TmpfsNode::Fifo { pipe } => Ok(pipe),
            _ => Err(KError::Invalid),
        }
    }
}

impl InodeOps for TmpfsInode {
    fn metadata(&self) -> Metadata {
        *self.meta.read()
    }

    fn set_metadata(&self, meta: Metadata) {
        *self.meta.write() = meta;
    }

    fn parent(&self) -> Option<Inode> {
        let p = self.parent.read();
        p.as_ref()
            .and_then(|w| w.upgrade())
            .map(|arc| Inode(arc))
    }

    fn lookup(&self, name: &str) -> KResult<Inode> {
        let children = self.as_dir()?;
        let map = children.read();
        map.get(name).cloned().ok_or(KError::NotFound)
    }

    fn create(&self, name: &str, kind: InodeKind, meta: Metadata) -> KResult<Inode> {
        if name.is_empty() || name.contains('/') {
            return Err(KError::Invalid);
        }

        let children = self.as_dir()?;
        {
            let map = children.read();
            if map.contains_key(name) {
                return Err(KError::AlreadyExists);
            }
        }

        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let parent_weak = self.self_weak.clone();

        let node: Arc<TmpfsInode> = Arc::new_cyclic(|weak| {
            let node = match kind {
                InodeKind::File => TmpfsNode::File {
                    data: RwLock::new(Vec::new()),
                },
                InodeKind::Dir => TmpfsNode::Dir {
                    children: RwLock::new(BTreeMap::new()),
                },
                InodeKind::Symlink => TmpfsNode::Symlink {
                    target: RwLock::new(String::new()),
                },
                InodeKind::Fifo => TmpfsNode::Fifo {
                    pipe: Pipe::new(),
                },
                _ => TmpfsNode::File {
                    data: RwLock::new(Vec::new()),
                },
            };

            TmpfsInode {
                id,
                self_weak: weak.clone(),
                parent: RwLock::new(Some(parent_weak.clone())),
                meta: RwLock::new(meta),
                node,
            }
        });

        let inode = Inode(node);

        let mut map = children.write();
        map.insert(name.to_string(), inode.clone());

        Ok(inode)
    }

    fn readdir(&self) -> KResult<Vec<DirEntry>> {
        let children = self.as_dir()?;
        let map = children.read();
        let mut out = Vec::new();
        for (name, inode) in map.iter() {
            out.push(DirEntry {
                name: name.clone(),
                kind: inode.kind(),
            });
        }
        Ok(out)
    }

    fn unlink(&self, name: &str) -> KResult<()> {
        let children = self.as_dir()?;
        let mut map = children.write();
        if map.remove(name).is_some() {
            Ok(())
        } else {
            Err(KError::NotFound)
        }
    }

    fn rmdir(&self, name: &str) -> KResult<()> {
        // rmdir é igual a unlink para tmpfs, a verificação de vazio é feita no VFS
        let children = self.as_dir()?;
        let mut map = children.write();
        if map.remove(name).is_some() {
            Ok(())
        } else {
            Err(KError::NotFound)
        }
    }

    fn symlink(&self, name: &str, target: &str, meta: Metadata) -> KResult<Inode> {
        if name.is_empty() || name.contains('/') {
            return Err(KError::Invalid);
        }

        let children = self.as_dir()?;
        {
            let map = children.read();
            if map.contains_key(name) {
                return Err(KError::AlreadyExists);
            }
        }

        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let parent_weak = self.self_weak.clone();

        let node: Arc<TmpfsInode> = Arc::new_cyclic(|weak| TmpfsInode {
            id,
            self_weak: weak.clone(),
            parent: RwLock::new(Some(parent_weak.clone())),
            meta: RwLock::new(meta),
            node: TmpfsNode::Symlink {
                target: RwLock::new(String::from(target)),
            },
        });

        let inode = Inode(node);

        let mut map = children.write();
        map.insert(name.to_string(), inode.clone());

        Ok(inode)
    }

    fn readlink(&self) -> KResult<String> {
        let target = self.as_symlink()?;
        Ok(target.read().clone())
    }

    fn mkfifo(&self, name: &str, meta: Metadata) -> KResult<Inode> {
        if name.is_empty() || name.contains('/') {
            return Err(KError::Invalid);
        }

        let children = self.as_dir()?;
        {
            let map = children.read();
            if map.contains_key(name) {
                return Err(KError::AlreadyExists);
            }
        }

        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let parent_weak = self.self_weak.clone();

        let node: Arc<TmpfsInode> = Arc::new_cyclic(|weak| TmpfsInode {
            id,
            self_weak: weak.clone(),
            parent: RwLock::new(Some(parent_weak.clone())),
            meta: RwLock::new(meta),
            node: TmpfsNode::Fifo {
                pipe: Pipe::new(),
            },
        });

        let inode = Inode(node);

        let mut map = children.write();
        map.insert(name.to_string(), inode.clone());

        Ok(inode)
    }

    fn link(&self, name: &str, target: Inode) -> KResult<()> {
        if name.is_empty() || name.contains('/') {
            return Err(KError::Invalid);
        }

        let children = self.as_dir()?;
        let mut map = children.write();

        if map.contains_key(name) {
            return Err(KError::AlreadyExists);
        }

        // Insert the existing inode with a new name
        map.insert(name.to_string(), target);
        Ok(())
    }

    fn rename_to(&self, old_name: &str, new_parent: &Inode, new_name: &str) -> KResult<()> {
        // Get the source children map
        let src_children = self.as_dir()?;

        // Remove from source
        let inode = {
            let mut map = src_children.write();
            match map.remove(old_name) {
                Some(i) => i,
                None => return Err(KError::NotFound),
            }
        };

        // Add to destination
        // Try to downcast to TmpfsInode to get its children
        let dst = new_parent.0.as_ref();

        // We need to access the new_parent's children
        // This is a bit tricky since we have trait objects
        // For tmpfs-to-tmpfs rename, both should be TmpfsInode
        // Let's use a generic approach: lookup fails so we create
        // Actually we should just call link on the new_parent

        match new_parent.0.link(new_name, inode.clone()) {
            Ok(()) => Ok(()),
            Err(e) => {
                // Restore the old entry on failure
                let mut map = src_children.write();
                map.insert(old_name.to_string(), inode);
                Err(e)
            }
        }
    }

    fn read_at(&self, offset: usize, out: &mut [u8]) -> KResult<usize> {
        // Check if this is a FIFO
        if let Ok(pipe) = self.as_fifo() {
            // FIFOs ignore offset, just read from the pipe
            return Ok(pipe.read(out));
        }

        // Regular file read
        let data = self.as_file()?;
        let buf = data.read();
        if offset >= buf.len() {
            return Ok(0);
        }
        let n = core::cmp::min(out.len(), buf.len() - offset);
        out[..n].copy_from_slice(&buf[offset..offset + n]);
        Ok(n)
    }

    fn write_at(&self, offset: usize, input: &[u8]) -> KResult<usize> {
        // Check if this is a FIFO
        if let Ok(pipe) = self.as_fifo() {
            // FIFOs ignore offset, just write to the pipe
            return Ok(pipe.write(input));
        }

        // Regular file write
        let data = self.as_file()?;
        let mut buf = data.write();

        let end = offset.checked_add(input.len()).ok_or(KError::Invalid)?;
        if end > buf.len() {
            buf.resize(end, 0);
        }
        buf[offset..end].copy_from_slice(input);
        Ok(input.len())
    }

    fn truncate(&self, size: usize) -> KResult<()> {
        let data = self.as_file()?;
        let mut buf = data.write();
        buf.resize(size, 0);
        Ok(())
    }

    fn size(&self) -> KResult<usize> {
        // FIFOs report 0 size
        if self.as_fifo().is_ok() {
            return Ok(0);
        }

        let data = self.as_file()?;
        Ok(data.read().len())
    }
}

// Permite coerção Arc<TmpfsInode> -> Arc<dyn InodeOps> via Inode wrapper.
impl From<Arc<TmpfsInode>> for Inode {
    fn from(a: Arc<TmpfsInode>) -> Self {
        Inode(a)
    }
}
