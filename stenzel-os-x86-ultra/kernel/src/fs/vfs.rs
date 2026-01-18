    #![allow(dead_code)]

    use alloc::collections::BTreeMap;
    use alloc::string::{String, ToString};
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
        /// Named pipe (FIFO)
        Fifo,
        /// Unix domain socket
        Socket,
    }

    bitflags::bitflags! {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct Mode: u16 {
            /// Set-user-ID on execution
            const S_ISUID = 0o4000;
            /// Set-group-ID on execution
            const S_ISGID = 0o2000;
            /// Sticky bit (restricted deletion)
            const S_ISVTX = 0o1000;

            /// User read
            const UR = 0o400;
            /// User write
            const UW = 0o200;
            /// User execute
            const UX = 0o100;

            /// Group read
            const GR = 0o040;
            /// Group write
            const GW = 0o020;
            /// Group execute
            const GX = 0o010;

            /// Other read
            const OR = 0o004;
            /// Other write
            const OW = 0o002;
            /// Other execute
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

        /// Check if setuid bit is set
        pub fn is_setuid(&self) -> bool {
            self.contains(Mode::S_ISUID)
        }

        /// Check if setgid bit is set
        pub fn is_setgid(&self) -> bool {
            self.contains(Mode::S_ISGID)
        }

        /// Check if sticky bit is set
        pub fn is_sticky(&self) -> bool {
            self.contains(Mode::S_ISVTX)
        }
    }

    /// Timestamp for file metadata
    #[derive(Debug, Clone, Copy, Default)]
    pub struct Timespec {
        pub secs: u64,
        pub nsecs: u32,
    }

    #[derive(Debug, Clone, Copy)]
    pub struct Metadata {
        pub uid: Uid,
        pub gid: Gid,
        pub mode: Mode,
        pub kind: InodeKind,
        /// Inode number
        pub ino: u64,
        /// Number of hard links
        pub nlink: u32,
        /// Access time
        pub atime: Timespec,
        /// Modification time
        pub mtime: Timespec,
        /// Status change time
        pub ctime: Timespec,
    }

    impl Default for Metadata {
        fn default() -> Self {
            Self {
                uid: Uid(0),
                gid: Gid(0),
                mode: Mode::from_octal(0o644),
                kind: InodeKind::File,
                ino: 0,
                nlink: 1,
                atime: Timespec::default(),
                mtime: Timespec::default(),
                ctime: Timespec::default(),
            }
        }
    }

    impl Metadata {
        /// Create metadata with basic fields, using defaults for extended fields
        pub fn simple(uid: Uid, gid: Gid, mode: Mode, kind: InodeKind) -> Self {
            Self {
                uid,
                gid,
                mode,
                kind,
                ino: 0,
                nlink: 1,
                atime: Timespec::default(),
                mtime: Timespec::default(),
                ctime: Timespec::default(),
            }
        }

        /// Create metadata with inode number
        pub fn with_ino(uid: Uid, gid: Gid, mode: Mode, kind: InodeKind, ino: u64) -> Self {
            Self {
                uid,
                gid,
                mode,
                kind,
                ino,
                nlink: 1,
                atime: Timespec::default(),
                mtime: Timespec::default(),
                ctime: Timespec::default(),
            }
        }
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

        /// Remove uma entrada do diretório (arquivo ou link).
        fn unlink(&self, _name: &str) -> KResult<()> {
            Err(KError::NotSupported)
        }

        /// Remove um diretório vazio.
        fn rmdir(&self, _name: &str) -> KResult<()> {
            Err(KError::NotSupported)
        }

        /// Create a symbolic link pointing to target
        fn symlink(&self, _name: &str, _target: &str, _meta: Metadata) -> KResult<Inode> {
            Err(KError::NotSupported)
        }

        /// Read the target of a symbolic link
        fn readlink(&self) -> KResult<String> {
            Err(KError::NotSupported)
        }

        /// Create a named pipe (FIFO)
        fn mkfifo(&self, _name: &str, _meta: Metadata) -> KResult<Inode> {
            Err(KError::NotSupported)
        }

        /// Create a hard link to an existing inode
        fn link(&self, _name: &str, _target: Inode) -> KResult<()> {
            Err(KError::NotSupported)
        }

        /// Rename an entry within this directory (source) to another directory (or same)
        fn rename_to(&self, _old_name: &str, _new_parent: &Inode, _new_name: &str) -> KResult<()> {
            Err(KError::NotSupported)
        }

        // file ops
        fn read_at(&self, offset: usize, out: &mut [u8]) -> KResult<usize>;
        fn write_at(&self, offset: usize, data: &[u8]) -> KResult<usize>;
        fn truncate(&self, size: usize) -> KResult<()>;
        fn size(&self) -> KResult<usize>;

        // xattr ops (extended attributes)
        /// Get extended attribute value
        fn getxattr(&self, _name: &str) -> KResult<Vec<u8>> {
            Err(KError::NotSupported)
        }

        /// Set extended attribute
        fn setxattr(&self, _name: &str, _value: Vec<u8>, _flags: super::xattr::XattrFlags) -> KResult<()> {
            Err(KError::NotSupported)
        }

        /// Remove extended attribute
        fn removexattr(&self, _name: &str) -> KResult<()> {
            Err(KError::NotSupported)
        }

        /// List extended attribute names
        fn listxattr(&self) -> KResult<Vec<String>> {
            Err(KError::NotSupported)
        }
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
        /// Mount points: path -> inode
        mount_points: BTreeMap<String, Inode>,
    }

    impl Vfs {
        pub fn new(root: Inode) -> Self {
            Self {
                root,
                mount_points: BTreeMap::new(),
            }
        }

        pub fn root(&self) -> Inode {
            self.root.clone()
        }

        /// Set a new root inode (for switch_root / pivot_root)
        pub fn set_root(&mut self, new_root: Inode) {
            // Clear all existing mount points
            self.mount_points.clear();
            // Set the new root
            self.root = new_root;
        }

        /// Monta um filesystem em um caminho específico.
        pub fn mount(&mut self, path: &str, inode: Inode) {
            let path = path.trim_end_matches('/');
            self.mount_points.insert(path.to_string(), inode);
        }

        /// Verifica se há um mount point para o caminho e retorna o inode.
        fn check_mount_point(&self, path: &str) -> Option<Inode> {
            let path = path.trim_end_matches('/');
            self.mount_points.get(path).cloned()
        }

        pub fn resolve(&self, path: &str, cred: &Cred) -> KResult<Inode> {
            // Verifica mount point exato primeiro
            if let Some(inode) = self.check_mount_point(path) {
                return Ok(inode);
            }

            let mut cur = if path.starts_with('/') {
                self.root.clone()
            } else {
                // Sem cwd por enquanto; no futuro: usar cwd por processo/thread.
                self.root.clone()
            };

            let mut current_path = String::new();
            let comps: Vec<&str> = split_path(path).collect();

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
                    // Atualiza current_path para ..
                    if let Some(pos) = current_path.rfind('/') {
                        current_path.truncate(pos);
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

                // Atualiza o path atual
                current_path.push('/');
                current_path.push_str(comp);

                // Verifica se há mount point neste path
                if let Some(mounted) = self.check_mount_point(&current_path) {
                    cur = mounted;
                } else {
                    cur = cur.0.lookup(comp)?;
                }
            }

            Ok(cur)
        }

        pub fn mkdir_all(&mut self, path: &str, cred: &Cred, mode: Mode) -> KResult<()> {
            let mut cur = self.root.clone();
            let comps: Vec<&str> = split_path(path).collect();
            let mut current_path = String::new();

            for comp in comps {
                if comp.is_empty() || comp == "." {
                    continue;
                }

                if comp == ".." {
                    if let Some(p) = cur.0.parent() {
                        cur = p;
                    }
                    // Atualiza current_path para ..
                    if let Some(pos) = current_path.rfind('/') {
                        current_path.truncate(pos);
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

                // Atualiza o path atual
                current_path.push('/');
                current_path.push_str(comp);

                // Verifica se há mount point neste path
                if let Some(mounted) = self.check_mount_point(&current_path) {
                    cur = mounted;
                    continue;
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
                        let child_meta = Metadata::simple(cred.uid, cred.gid, mode, InodeKind::Dir);
                        cur = cur.0.create(comp, InodeKind::Dir, child_meta)?;
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
                    let fmeta = Metadata::simple(cred.uid, cred.gid, mode, InodeKind::File);
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

        /// Remove um arquivo ou link simbólico.
        pub fn unlink(&mut self, path: &str, cred: &Cred) -> KResult<()> {
            let (parent_path, name) = split_parent(path)?;
            let parent = self.resolve(parent_path, cred)?;
            let parent_meta = parent.metadata();

            if parent_meta.kind != InodeKind::Dir {
                return Err(KError::Invalid);
            }
            if !perm::can_write_dir(&parent_meta, cred) {
                return Err(KError::PermissionDenied);
            }

            // Verifica se o alvo existe e não é um diretório
            let target = parent.0.lookup(name)?;
            let target_meta = target.metadata();
            if target_meta.kind == InodeKind::Dir {
                return Err(KError::Invalid); // Use rmdir para diretórios
            }

            // Sticky bit check: if parent has sticky bit set,
            // only root, dir owner, or file owner can delete
            if parent_meta.mode.is_sticky() {
                let is_root = cred.uid.0 == 0;
                let is_dir_owner = cred.uid == parent_meta.uid;
                let is_file_owner = cred.uid == target_meta.uid;
                if !is_root && !is_dir_owner && !is_file_owner {
                    return Err(KError::PermissionDenied);
                }
            }

            parent.0.unlink(name)
        }

        /// Remove um diretório vazio.
        pub fn rmdir(&mut self, path: &str, cred: &Cred) -> KResult<()> {
            let (parent_path, name) = split_parent(path)?;
            let parent = self.resolve(parent_path, cred)?;
            let parent_meta = parent.metadata();

            if parent_meta.kind != InodeKind::Dir {
                return Err(KError::Invalid);
            }
            if !perm::can_write_dir(&parent_meta, cred) {
                return Err(KError::PermissionDenied);
            }

            // Verifica se o alvo é um diretório
            let target = parent.0.lookup(name)?;
            let target_meta = target.metadata();
            if target_meta.kind != InodeKind::Dir {
                return Err(KError::Invalid); // Não é diretório
            }

            // Sticky bit check: if parent has sticky bit set,
            // only root, dir owner, or target dir owner can delete
            if parent_meta.mode.is_sticky() {
                let is_root = cred.uid.0 == 0;
                let is_parent_owner = cred.uid == parent_meta.uid;
                let is_target_owner = cred.uid == target_meta.uid;
                if !is_root && !is_parent_owner && !is_target_owner {
                    return Err(KError::PermissionDenied);
                }
            }

            // Verifica se está vazio
            let entries = target.0.readdir()?;
            if !entries.is_empty() {
                return Err(KError::NotEmpty);
            }

            parent.0.rmdir(name)
        }

        /// Renomeia um arquivo ou diretório.
        /// Se newpath já existe e é um arquivo, é substituído.
        /// Se newpath já existe e é um diretório, deve estar vazio.
        pub fn rename(&mut self, oldpath: &str, newpath: &str, cred: &Cred) -> KResult<()> {
            // Parse paths
            let (old_parent_path, old_name) = split_parent(oldpath)?;
            let (new_parent_path, new_name) = split_parent(newpath)?;

            // Resolve parent directories
            let old_parent = self.resolve(old_parent_path, cred)?;
            let new_parent = self.resolve(new_parent_path, cred)?;

            // Check old parent permissions (need write to remove from old location)
            let old_parent_meta = old_parent.metadata();
            if old_parent_meta.kind != InodeKind::Dir {
                return Err(KError::NotADirectory);
            }
            if !perm::can_write_dir(&old_parent_meta, cred) {
                return Err(KError::PermissionDenied);
            }

            // Check new parent permissions (need write to add to new location)
            let new_parent_meta = new_parent.metadata();
            if new_parent_meta.kind != InodeKind::Dir {
                return Err(KError::NotADirectory);
            }
            if !perm::can_write_dir(&new_parent_meta, cred) {
                return Err(KError::PermissionDenied);
            }

            // Get source inode
            let source = old_parent.0.lookup(old_name)?;
            let source_meta = source.metadata();

            // Check sticky bit on old parent
            if old_parent_meta.mode.is_sticky() {
                let is_root = cred.uid.0 == 0;
                let is_dir_owner = cred.uid == old_parent_meta.uid;
                let is_file_owner = cred.uid == source_meta.uid;
                if !is_root && !is_dir_owner && !is_file_owner {
                    return Err(KError::PermissionDenied);
                }
            }

            // Check if destination exists
            match new_parent.0.lookup(new_name) {
                Ok(dest) => {
                    let dest_meta = dest.metadata();

                    // Check sticky bit on new parent
                    if new_parent_meta.mode.is_sticky() {
                        let is_root = cred.uid.0 == 0;
                        let is_dir_owner = cred.uid == new_parent_meta.uid;
                        let is_dest_owner = cred.uid == dest_meta.uid;
                        if !is_root && !is_dir_owner && !is_dest_owner {
                            return Err(KError::PermissionDenied);
                        }
                    }

                    // Can't rename a file to a directory or vice versa
                    if source_meta.kind == InodeKind::Dir && dest_meta.kind != InodeKind::Dir {
                        return Err(KError::NotADirectory);
                    }
                    if source_meta.kind != InodeKind::Dir && dest_meta.kind == InodeKind::Dir {
                        return Err(KError::IsADirectory);
                    }

                    // If destination is a directory, it must be empty
                    if dest_meta.kind == InodeKind::Dir {
                        let entries = dest.0.readdir()?;
                        if !entries.is_empty() {
                            return Err(KError::NotEmpty);
                        }
                        // Remove the empty directory
                        new_parent.0.rmdir(new_name)?;
                    } else {
                        // Remove the existing file
                        new_parent.0.unlink(new_name)?;
                    }
                }
                Err(KError::NotFound) => {
                    // Destination doesn't exist - that's fine
                }
                Err(e) => return Err(e),
            }

            // Perform the rename operation
            old_parent.0.rename_to(old_name, &new_parent, new_name)
        }

        /// Altera o modo (permissões) de um arquivo ou diretório.
        pub fn chmod(&mut self, path: &str, mode: Mode, cred: &Cred) -> KResult<()> {
            let inode = self.resolve(path, cred)?;
            let mut meta = inode.metadata();

            // Somente root ou o owner pode alterar permissões
            if cred.uid.0 != 0 && meta.uid != cred.uid {
                return Err(KError::PermissionDenied);
            }

            meta.mode = mode;
            inode.0.set_metadata(meta);
            Ok(())
        }

        /// Altera o dono de um arquivo ou diretório.
        pub fn chown(&mut self, path: &str, uid: Uid, gid: Gid, cred: &Cred) -> KResult<()> {
            let inode = self.resolve(path, cred)?;
            let mut meta = inode.metadata();

            // Somente root pode alterar owner
            if cred.uid.0 != 0 {
                // Não-root: só pode mudar grupo para um grupo que pertence
                if uid != meta.uid {
                    return Err(KError::PermissionDenied);
                }
                // Simplificação: permite mudar gid se for o owner
                if meta.uid != cred.uid {
                    return Err(KError::PermissionDenied);
                }
            }

            meta.uid = uid;
            meta.gid = gid;
            inode.0.set_metadata(meta);
            Ok(())
        }

        /// Create a symbolic link
        pub fn symlink(&mut self, link_path: &str, target: &str, cred: &Cred) -> KResult<Inode> {
            let (parent_path, name) = split_parent(link_path)?;
            let parent = self.resolve(parent_path, cred)?;

            // Check write permission on parent directory
            if !perm::can_write(&parent.metadata(), cred) {
                return Err(KError::PermissionDenied);
            }

            let meta = Metadata::simple(cred.uid, cred.gid, Mode::from_octal(0o777), InodeKind::Symlink);

            parent.0.symlink(name, target, meta)
        }

        /// Read the target of a symbolic link (does not follow symlinks)
        pub fn readlink(&self, path: &str, cred: &Cred) -> KResult<String> {
            let inode = self.resolve_nofollow(path, cred)?;

            if inode.metadata().kind != InodeKind::Symlink {
                return Err(KError::Invalid);
            }

            inode.0.readlink()
        }

        /// Create a named pipe (FIFO)
        pub fn mkfifo(&mut self, path: &str, mode: Mode, cred: &Cred) -> KResult<Inode> {
            let (parent_path, name) = split_parent(path)?;
            let parent = self.resolve(parent_path, cred)?;

            // Check write permission on parent directory
            if !perm::can_write(&parent.metadata(), cred) {
                return Err(KError::PermissionDenied);
            }

            let meta = Metadata::simple(cred.uid, cred.gid, mode, InodeKind::Fifo);

            parent.0.mkfifo(name, meta)
        }

        /// Resolve path without following the final symlink (for lstat)
        pub fn resolve_nofollow(&self, path: &str, cred: &Cred) -> KResult<Inode> {
            let path = path.trim_end_matches('/');
            if path.is_empty() || path == "/" {
                return Ok(self.root.clone());
            }

            // Check mount points
            for (mp, inode) in &self.mount_points {
                if path == mp {
                    return Ok(inode.clone());
                }
                if path.starts_with(mp) {
                    let rel = &path[mp.len()..];
                    if rel.starts_with('/') || rel.is_empty() {
                        return self.resolve_relative_nofollow(inode.clone(), rel.trim_start_matches('/'), cred);
                    }
                }
            }

            // Resolve from root
            self.resolve_relative_nofollow(self.root.clone(), path.trim_start_matches('/'), cred)
        }

        /// Resolve relative path without following the final symlink
        fn resolve_relative_nofollow(&self, mut cur: Inode, path: &str, cred: &Cred) -> KResult<Inode> {
            let components: Vec<&str> = path.split('/').filter(|c| !c.is_empty()).collect();
            let len = components.len();

            for (i, comp) in components.into_iter().enumerate() {
                let is_last = i == len - 1;

                if comp == "." {
                    continue;
                }
                if comp == ".." {
                    if let Some(p) = cur.0.parent() {
                        cur = p;
                    }
                    continue;
                }

                // Verificar permissão de execução no diretório
                if !perm::can_exec(&cur.metadata(), cred) {
                    return Err(KError::PermissionDenied);
                }

                cur = cur.0.lookup(comp)?;

                // Follow intermediate symlinks (but not the last one)
                if !is_last && cur.metadata().kind == InodeKind::Symlink {
                    let target = cur.0.readlink()?;
                    if target.starts_with('/') {
                        cur = self.resolve(&target, cred)?;
                    } else {
                        // Relative symlink - go back to parent and resolve
                        if let Some(p) = cur.0.parent() {
                            let mut full_target = String::new();
                            // This is a simplification; real implementation would need parent path
                            cur = self.resolve(&target, cred)?;
                        }
                    }
                }
            }

            Ok(cur)
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
