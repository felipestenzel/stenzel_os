    #![allow(dead_code)]

    use alloc::string::String;
    use alloc::sync::Arc;
    use alloc::vec;
    use alloc::vec::Vec;

    use crate::security::{Cred, Gid, Uid};
    use crate::util::{KError, KResult};

    use super::perm;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum InodeKind {
        File,
        Dir,
        Symlink,
        CharDev,
        BlockDev,
    }

    bitflags::bitflags! {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct Mode: u16 {
            const UR = 0o400;
            const UW = 0o200;
            const UX = 0o100;

            const GR = 0o040;
            const GW = 0o020;
            const GX = 0o010;

            const OR = 0o004;
            const OW = 0o002;
            const OX = 0o001;
        }
    }

    impl Mode {
        pub const fn from_octal(v: u16) -> Mode {
            Mode::from_bits_truncate(v)
        }

        pub fn to_octal(self) -> u16 {
            self.bits()
        }
    }

    #[derive(Debug, Clone, Copy)]
    pub struct Metadata {
        pub uid: Uid,
        pub gid: Gid,
        pub mode: Mode,
        pub kind: InodeKind,
    }

    #[derive(Debug, Clone)]
    pub struct DirEntry {
        pub name: String,
        pub kind: InodeKind,
    }

    pub trait InodeOps: Send + Sync {
        fn metadata(&self) -> Metadata;
        fn set_metadata(&self, meta: Metadata);

        fn parent(&self) -> Option<Inode>;

        // dir ops
        fn lookup(&self, name: &str) -> KResult<Inode>;
        fn create(&self, name: &str, kind: InodeKind, meta: Metadata) -> KResult<Inode>;
        fn readdir(&self) -> KResult<Vec<DirEntry>>;

        // file ops
        fn read_at(&self, offset: usize, out: &mut [u8]) -> KResult<usize>;
        fn write_at(&self, offset: usize, data: &[u8]) -> KResult<usize>;
        fn truncate(&self, size: usize) -> KResult<()>;
        fn size(&self) -> KResult<usize>;
    }

    #[derive(Clone)]
    pub struct Inode(pub Arc<dyn InodeOps>);

    impl Inode {
        pub fn metadata(&self) -> Metadata {
            self.0.metadata()
        }
        pub fn kind(&self) -> InodeKind {
            self.metadata().kind
        }
    }

    pub struct Vfs {
        root: Inode,
    }

    impl Vfs {
        pub fn new(root: Inode) -> Self {
            Self { root }
        }

        pub fn root(&self) -> Inode {
            self.root.clone()
        }

        pub fn resolve(&self, path: &str, cred: &Cred) -> KResult<Inode> {
            let mut cur = if path.starts_with('/') {
                self.root.clone()
            } else {
                // Sem cwd por enquanto; no futuro: usar cwd por processo/thread.
                self.root.clone()
            };

            let comps = split_path(path);
            for comp in comps {
                if comp == "." || comp.is_empty() {
                    continue;
                }
                if comp == ".." {
                    if let Some(p) = cur.0.parent() {
                        cur = p;
                    } else {
                        cur = self.root.clone();
                    }
                    continue;
                }

                // Travessia exige exec no diretório.
                let meta = cur.metadata();
                if meta.kind != InodeKind::Dir {
                    return Err(KError::NotFound);
                }
                if !perm::can_exec_dir(&meta, cred) {
                    return Err(KError::PermissionDenied);
                }

                cur = cur.0.lookup(comp)?;
            }

            Ok(cur)
        }

        pub fn mkdir_all(&mut self, path: &str, cred: &Cred, mode: Mode) -> KResult<()> {
            let mut cur = self.root.clone();
            let comps = split_path(path);

            for comp in comps {
                if comp.is_empty() || comp == "." {
                    continue;
                }

                if comp == ".." {
                    if let Some(p) = cur.0.parent() {
                        cur = p;
                    }
                    continue;
                }

                // precisa ser dir e ter permissão de travessia
                let meta = cur.metadata();
                if meta.kind != InodeKind::Dir {
                    return Err(KError::Invalid);
                }
                if !perm::can_exec_dir(&meta, cred) {
                    return Err(KError::PermissionDenied);
                }

                // tenta lookup; se não existe, cria
                match cur.0.lookup(comp) {
                    Ok(next) => {
                        cur = next;
                    }
                    Err(KError::NotFound) => {
                        // precisa permissão de escrita no diretório pai
                        if !perm::can_write_dir(&meta, cred) {
                            return Err(KError::PermissionDenied);
                        }
                        let child_meta = Metadata {
                            uid: cred.uid,
                            gid: cred.gid,
                            mode,
                            kind: InodeKind::Dir,
                        };
                        let next = cur.0.create(comp, InodeKind::Dir, child_meta)?;
                        cur = next;
                    }
                    Err(e) => return Err(e),
                }
            }

            Ok(())
        }

        pub fn write_file(
            &mut self,
            path: &str,
            cred: &Cred,
            mode: Mode,
            data: &[u8],
        ) -> KResult<()> {
            let (parent_path, name) = split_parent(path)?;
            let parent = self.resolve(parent_path, cred)?;
            let meta = parent.metadata();
            if meta.kind != InodeKind::Dir {
                return Err(KError::Invalid);
            }
            if !perm::can_exec_dir(&meta, cred) || !perm::can_write_dir(&meta, cred) {
                return Err(KError::PermissionDenied);
            }

            let file = match parent.0.lookup(name) {
                Ok(i) => i,
                Err(KError::NotFound) => {
                    let fmeta = Metadata {
                        uid: cred.uid,
                        gid: cred.gid,
                        mode,
                        kind: InodeKind::File,
                    };
                    parent.0.create(name, InodeKind::File, fmeta)?
                }
                Err(e) => return Err(e),
            };

            // permissão de escrita no arquivo
            let fmeta = file.metadata();
            if !perm::can_write_file(&fmeta, cred) {
                return Err(KError::PermissionDenied);
            }

            file.0.truncate(0)?;
            let _ = file.0.write_at(0, data)?;
            Ok(())
        }

        pub fn read_file(&mut self, path: &str, cred: &Cred) -> KResult<Vec<u8>> {
            let inode = self.resolve(path, cred)?;
            let meta = inode.metadata();
            if meta.kind != InodeKind::File {
                return Err(KError::Invalid);
            }
            if !perm::can_read_file(&meta, cred) {
                return Err(KError::PermissionDenied);
            }
            let size = inode.0.size()?;
            let mut buf = vec![0u8; size];
            let _ = inode.0.read_at(0, &mut buf)?;
            Ok(buf)
        }

        pub fn list_dir(&mut self, path: &str, cred: &Cred) -> KResult<Vec<DirEntry>> {
            let inode = self.resolve(path, cred)?;
            let meta = inode.metadata();
            if meta.kind != InodeKind::Dir {
                return Err(KError::Invalid);
            }
            if !perm::can_exec_dir(&meta, cred) || !perm::can_read_dir(&meta, cred) {
                return Err(KError::PermissionDenied);
            }
            inode.0.readdir()
        }
    }

    fn split_path(path: &str) -> impl Iterator<Item = &str> {
        path.split('/').filter(|c| !c.is_empty())
    }

    fn split_parent(path: &str) -> KResult<(&str, &str)> {
        let p = path.trim_end_matches('/');
        if p.is_empty() || p == "/" {
            return Err(KError::Invalid);
        }
        if let Some(idx) = p.rfind('/') {
            let (a, b) = p.split_at(idx);
            let name = b.trim_start_matches('/');
            let parent = if a.is_empty() { "/" } else { a };
            if name.is_empty() {
                return Err(KError::Invalid);
            }
            Ok((parent, name))
        } else {
            // sem '/', parent é root
            Ok(("/", p))
        }
    }
