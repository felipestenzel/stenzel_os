    #![allow(dead_code)]

    use alloc::collections::BTreeMap;
    use alloc::string::{String, ToString};
    use alloc::sync::{Arc, Weak};
    use alloc::vec::Vec;
    use core::sync::atomic::{AtomicU64, Ordering};

    use spin::RwLock;

    use crate::util::{KError, KResult};

    use super::vfs::{DirEntry, Inode, InodeKind, InodeOps, Metadata};

    pub struct TmpFs;

    impl TmpFs {
        pub fn new_root() -> Inode {
            let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
            let meta = Metadata {
                uid: crate::security::Uid(0),
                gid: crate::security::Gid(0),
                mode: super::vfs::Mode::from_octal(0o755),
                kind: InodeKind::Dir,
            };

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

        fn read_at(&self, offset: usize, out: &mut [u8]) -> KResult<usize> {
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
