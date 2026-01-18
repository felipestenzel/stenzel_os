    #![allow(dead_code)]

    use alloc::string::String;
    use alloc::sync::Arc;
    use spin::Once;

    use crate::security::{self, Cred};
    use crate::storage::BlockDevice;
    use crate::sync::IrqSafeMutex;
    use crate::util::KResult;

    pub mod dentry;
    pub mod devfs;
    pub mod exfat;
    pub mod ext2;
    pub mod ext4;
    pub mod fat32;
    pub mod inode_cache;
    pub mod iso9660;
    pub mod ntfs;
    pub mod page_cache;
    pub mod perm;
    pub mod procfs;
    pub mod sysfs;
    mod tmpfs;
    pub mod vfs;
    pub mod xattr;
    pub mod acl;
    pub mod tuning;

    pub use vfs::{Inode, InodeKind, Metadata, Mode, Vfs};

    static VFS: Once<IrqSafeMutex<Vfs>> = Once::new();

    pub fn init() {
        // Initialize page cache first
        page_cache::init();

        // Initialize dentry cache
        dentry::init();

        // Initialize inode cache
        inode_cache::init();

        let root = tmpfs::TmpFs::new_root();
        VFS.call_once(|| IrqSafeMutex::new(Vfs::new(root)));
    }

    pub fn vfs_lock() -> crate::sync::IrqSafeGuard<'static, Vfs> {
        let v = VFS.call_once(|| panic!("VFS não inicializado"));
        v.lock()
    }

    pub fn bootstrap_filesystem() {
        let root_cred = security::user_db().login("root").expect("root user");

        let mut vfs = vfs_lock();

        // Diretórios básicos
        let dirs = [
            "/bin",
            "/sbin",
            "/etc",
            "/etc/stenzel",
            "/home",
            "/home/user",
            "/root",
            "/var",
            "/var/log",
            "/tmp",
            "/dev",
            "/proc",
            "/sys",
        ];

        for d in dirs {
            let _ = vfs.mkdir_all(d, &root_cred, Mode::from_octal(0o755));
        }

        // Monta procfs em /proc
        let _ = mount_procfs(&mut vfs, &root_cred);

        // Monta devfs em /dev
        let _ = mount_devfs(&mut vfs, &root_cred);

        // Monta sysfs em /sys
        let _ = mount_sysfs(&mut vfs, &root_cred);

        // /etc/passwd-like
        let passwd = security::user_db().passwd_text();
        let _ = vfs.write_file("/etc/passwd", &root_cred, Mode::from_octal(0o644), passwd.as_bytes());

        // Config do sistema (simples; parser no futuro)
        let default_cfg = r#"
    hostname=stenzel
    default_user=user
    "#;
        let _ = vfs.write_file(
            "/etc/stenzel/system.conf",
            &root_cred,
            Mode::from_octal(0o644),
            default_cfg.as_bytes(),
        );

        // Log de boot
        let bootlog = String::from("boot ok\n");
        let _ = vfs.write_file("/var/log/boot.log", &root_cred, Mode::from_octal(0o644), bootlog.as_bytes());

        crate::kprintln!("vfs: bootstrap concluído");
    }

    pub fn read_file(path: &str, cred: &Cred) -> crate::util::KResult<alloc::vec::Vec<u8>> {
        let mut vfs = vfs_lock();
        vfs.read_file(path, cred)
    }

    /// Write a file to the filesystem
    pub fn write_file(path: &str, cred: &Cred, mode: vfs::Mode, data: &[u8]) -> KResult<()> {
        let mut vfs = vfs_lock();
        vfs.write_file(path, cred, mode, data)
    }

    /// Create a directory
    pub fn mkdir(path: &str, cred: &Cred, mode: vfs::Mode) -> KResult<()> {
        let mut vfs = vfs_lock();
        vfs.mkdir_all(path, cred, mode)
    }

    /// Remove a file
    pub fn unlink(path: &str, cred: &Cred) -> KResult<()> {
        let mut vfs = vfs_lock();
        vfs.unlink(path, cred)
    }

    /// Remove a directory
    pub fn rmdir(path: &str, cred: &Cred) -> KResult<()> {
        let mut vfs = vfs_lock();
        vfs.rmdir(path, cred)
    }

    /// Change file permissions
    pub fn chmod(path: &str, mode: vfs::Mode, cred: &Cred) -> KResult<()> {
        let mut vfs = vfs_lock();
        vfs.chmod(path, mode, cred)
    }

    /// Create a symbolic link
    pub fn symlink(target: &str, link_path: &str, cred: &Cred) -> KResult<vfs::Inode> {
        let mut vfs = vfs_lock();
        vfs.symlink(link_path, target, cred)
    }

    /// Get file metadata (stat)
    pub fn stat(path: &str, cred: &Cred) -> KResult<vfs::Metadata> {
        let vfs = vfs_lock();
        let inode = vfs.resolve(path, cred)?;
        Ok(inode.metadata())
    }

    /// Monta uma partição ext2 do dispositivo de bloco
    pub fn mount_ext2(device: Arc<dyn BlockDevice>) -> KResult<Arc<ext2::Ext2Fs>> {
        ext2::Ext2Fs::mount(device)
    }

    /// Monta uma partição ext4 do dispositivo de bloco
    pub fn mount_ext4(device: Arc<dyn BlockDevice>) -> KResult<Arc<ext4::Ext4Fs>> {
        ext4::Ext4Fs::mount(device)
    }

    /// Tenta montar ext4 ou ext2 automaticamente
    pub fn mount_ext_auto(device: Arc<dyn BlockDevice>) -> KResult<Inode> {
        // Try ext4 first (it can also mount ext2/ext3 without extents)
        if let Ok(is_ext4) = ext4::is_ext4(&device) {
            if is_ext4 {
                let fs = ext4::Ext4Fs::mount(device)?;
                return Ok(fs.root());
            }
        }

        // Fall back to ext2
        let fs = ext2::Ext2Fs::mount(device)?;
        Ok(fs.root())
    }

    /// Auto-detect and mount filesystem
    pub fn mount_auto(device: Arc<dyn BlockDevice>) -> KResult<Inode> {
        // Try ISO 9660 (CD/DVD)
        if let Ok(true) = iso9660::is_iso9660(&device) {
            crate::kprintln!("mount_auto: detected ISO 9660 filesystem");
            let fs = iso9660::Iso9660Fs::mount(Arc::clone(&device))?;
            return Ok(fs.root());
        }

        // Try NTFS
        if let Ok(true) = ntfs::is_ntfs(&device) {
            crate::kprintln!("mount_auto: detected NTFS filesystem");
            let fs = ntfs::NtfsFs::mount(Arc::clone(&device))?;
            return Ok(fs.root());
        }

        // Try exFAT
        if let Ok(true) = exfat::is_exfat(&device) {
            crate::kprintln!("mount_auto: detected exFAT filesystem");
            let fs = exfat::ExfatFs::mount(Arc::clone(&device))?;
            return Ok(fs.root());
        }

        // Try ext4
        if let Ok(true) = ext4::is_ext4(&device) {
            crate::kprintln!("mount_auto: detected ext4 filesystem");
            let fs = ext4::Ext4Fs::mount(Arc::clone(&device))?;
            return Ok(fs.root());
        }

        // Try FAT32/exFAT detection via boot sector
        // (fat32 and ext2 as fallback)
        if let Ok(fs) = fat32::Fat32Fs::mount(Arc::clone(&device)) {
            crate::kprintln!("mount_auto: detected FAT32 filesystem");
            return Ok(fs.root());
        }

        // Fall back to ext2
        crate::kprintln!("mount_auto: trying ext2 as fallback");
        let fs = ext2::Ext2Fs::mount(device)?;
        Ok(fs.root())
    }

    /// Mount ISO 9660 filesystem
    pub fn mount_iso9660(device: Arc<dyn BlockDevice>) -> KResult<Arc<iso9660::Iso9660Fs>> {
        iso9660::Iso9660Fs::mount(device)
    }

    /// Mount NTFS filesystem
    pub fn mount_ntfs(device: Arc<dyn BlockDevice>) -> KResult<Arc<ntfs::NtfsFs>> {
        ntfs::NtfsFs::mount(device)
    }

    /// Tenta montar ext2 da partição root e integrar no VFS
    pub fn mount_root_ext2(device: Arc<dyn BlockDevice>) -> KResult<()> {
        let fs = ext2::Ext2Fs::mount(device)?;
        let root = fs.root();

        // Cria /mnt se não existir e monta ext2 lá
        {
            let root_cred = security::user_db().login("root").expect("root user");
            let mut vfs = vfs_lock();
            let _ = vfs.mkdir_all("/mnt", &root_cred, Mode::from_octal(0o755));
            vfs.mount("/mnt", root);
        }

        crate::kprintln!("ext2: filesystem montado com sucesso em /mnt");

        Ok(())
    }

    /// Tenta montar ext4 ou ext2 automaticamente e integrar no VFS
    pub fn mount_root_ext(device: Arc<dyn BlockDevice>) -> KResult<()> {
        let root = mount_ext_auto(device)?;

        // Cria /mnt se não existir e monta lá
        {
            let root_cred = security::user_db().login("root").expect("root user");
            let mut vfs = vfs_lock();
            let _ = vfs.mkdir_all("/mnt", &root_cred, Mode::from_octal(0o755));
            vfs.mount("/mnt", root);
        }

        Ok(())
    }

    /// Monta procfs em /proc
    fn mount_procfs(vfs: &mut Vfs, _cred: &Cred) -> KResult<()> {
        let procfs_root = procfs::new_root();
        vfs.mount("/proc", procfs_root);
        crate::kprintln!("procfs: montado em /proc");
        Ok(())
    }

    /// Monta devfs em /dev
    fn mount_devfs(vfs: &mut Vfs, _cred: &Cred) -> KResult<()> {
        let devfs_root = devfs::new_root();
        vfs.mount("/dev", devfs_root);
        crate::kprintln!("devfs: montado em /dev");
        Ok(())
    }

    /// Monta sysfs em /sys
    fn mount_sysfs(vfs: &mut Vfs, _cred: &Cred) -> KResult<()> {
        let sysfs_root = sysfs::new_root();
        vfs.mount("/sys", sysfs_root);
        crate::kprintln!("sysfs: montado em /sys");
        Ok(())
    }

    // ========================================================================
    // Root Mount Functionality
    // ========================================================================

    use alloc::vec::Vec;
    use spin::Mutex;

    /// Boot parameters parsed from kernel command line
    #[derive(Debug, Clone, Default)]
    pub struct BootParams {
        /// Root device (e.g., "/dev/sda1", "UUID=xxx", "LABEL=xxx")
        pub root: Option<String>,
        /// Root filesystem type (e.g., "ext4", "ext2", "xfs")
        pub rootfstype: Option<String>,
        /// Mount options for root
        pub rootflags: Option<String>,
        /// Whether to mount root read-only initially
        pub ro: bool,
        /// Init program path
        pub init: Option<String>,
    }

    static BOOT_PARAMS: Once<Mutex<BootParams>> = Once::new();

    /// Parse boot parameters from kernel command line
    pub fn parse_boot_params(cmdline: &str) -> BootParams {
        let mut params = BootParams::default();

        for param in cmdline.split_whitespace() {
            if let Some(value) = param.strip_prefix("root=") {
                params.root = Some(String::from(value));
            } else if let Some(value) = param.strip_prefix("rootfstype=") {
                params.rootfstype = Some(String::from(value));
            } else if let Some(value) = param.strip_prefix("rootflags=") {
                params.rootflags = Some(String::from(value));
            } else if let Some(value) = param.strip_prefix("init=") {
                params.init = Some(String::from(value));
            } else if param == "ro" {
                params.ro = true;
            } else if param == "rw" {
                params.ro = false;
            }
        }

        params
    }

    /// Initialize boot parameters
    pub fn init_boot_params(cmdline: &str) {
        let params = parse_boot_params(cmdline);
        crate::kprintln!("boot: root={:?}, rootfstype={:?}",
            params.root, params.rootfstype);
        BOOT_PARAMS.call_once(|| Mutex::new(params));
    }

    /// Get boot parameters
    pub fn boot_params() -> Option<BootParams> {
        BOOT_PARAMS.get().map(|p| p.lock().clone())
    }

    /// fstab entry structure
    #[derive(Debug, Clone)]
    pub struct FstabEntry {
        pub device: String,      // Device spec (UUID=, LABEL=, /dev/xxx)
        pub mount_point: String, // Mount point
        pub fs_type: String,     // Filesystem type
        pub options: String,     // Mount options
        pub dump: u8,            // Dump frequency
        pub pass: u8,            // fsck pass number
    }

    /// Parse fstab file content
    pub fn parse_fstab(content: &str) -> Vec<FstabEntry> {
        let mut entries = Vec::new();

        for line in content.lines() {
            let line = line.trim();
            // Skip empty lines and comments
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 4 {
                entries.push(FstabEntry {
                    device: String::from(parts[0]),
                    mount_point: String::from(parts[1]),
                    fs_type: String::from(parts[2]),
                    options: String::from(parts[3]),
                    dump: parts.get(4).and_then(|s| s.parse().ok()).unwrap_or(0),
                    pass: parts.get(5).and_then(|s| s.parse().ok()).unwrap_or(0),
                });
            }
        }

        entries
    }

    /// Mount root filesystem from a block device
    ///
    /// This function mounts the given device as the root filesystem,
    /// replacing the current tmpfs root with the real root.
    pub fn mount_root_from_device(device: Arc<dyn BlockDevice>, fstype: Option<&str>) -> KResult<()> {
        // Determine filesystem type
        let fs_root = match fstype {
            Some("ext4") => {
                let fs = ext4::Ext4Fs::mount(Arc::clone(&device))?;
                fs.root()
            }
            Some("ext2") | Some("ext3") => {
                let fs = ext2::Ext2Fs::mount(Arc::clone(&device))?;
                fs.root()
            }
            Some("fat32") | Some("vfat") => {
                let fs = fat32::Fat32Fs::mount(Arc::clone(&device))?;
                fs.root()
            }
            Some("ntfs") => {
                let fs = ntfs::NtfsFs::mount(Arc::clone(&device))?;
                fs.root()
            }
            Some("exfat") => {
                let fs = exfat::ExfatFs::mount(Arc::clone(&device))?;
                fs.root()
            }
            Some("iso9660") | Some("cdrom") | Some("udf") => {
                let fs = iso9660::Iso9660Fs::mount(Arc::clone(&device))?;
                fs.root()
            }
            None | Some("auto") => {
                // Try auto-detection
                mount_auto(device)?
            }
            Some(other) => {
                crate::kprintln!("rootmount: unknown filesystem type: {}", other);
                return Err(crate::util::KError::NotSupported);
            }
        };

        // Perform switch_root
        switch_root(fs_root)?;

        crate::kprintln!("rootmount: root filesystem mounted successfully");
        Ok(())
    }

    /// Switch the VFS root to a new root inode
    ///
    /// This function:
    /// 1. Sets the new inode as the VFS root
    /// 2. Re-mounts virtual filesystems (procfs, devfs, sysfs)
    /// 3. Creates standard directories if they don't exist
    pub fn switch_root(new_root: Inode) -> KResult<()> {
        let root_cred = security::user_db().login("root").expect("root user");

        {
            let mut vfs = vfs_lock();

            // Switch the root
            vfs.set_root(new_root);
            crate::kprintln!("rootmount: switched to new root filesystem");

            // Create essential directories if they don't exist
            let dirs = ["/proc", "/dev", "/sys", "/tmp", "/var", "/var/log"];
            for d in dirs {
                let _ = vfs.mkdir_all(d, &root_cred, Mode::from_octal(0o755));
            }

            // Re-mount virtual filesystems
            let _ = mount_procfs(&mut vfs, &root_cred);
            let _ = mount_devfs(&mut vfs, &root_cred);
            let _ = mount_sysfs(&mut vfs, &root_cred);
        }

        Ok(())
    }

    /// Process fstab and mount all filesystems
    pub fn mount_from_fstab() -> KResult<()> {
        let root_cred = security::user_db().login("root").expect("root user");

        // Try to read /etc/fstab
        let fstab_content = match read_file("/etc/fstab", &root_cred) {
            Ok(content) => String::from_utf8_lossy(&content).into_owned(),
            Err(_) => {
                crate::kprintln!("fstab: /etc/fstab not found, skipping");
                return Ok(());
            }
        };

        let entries = parse_fstab(&fstab_content);
        crate::kprintln!("fstab: {} entries found", entries.len());

        for entry in &entries {
            // Skip root (should already be mounted)
            if entry.mount_point == "/" {
                continue;
            }

            // Skip swap
            if entry.fs_type == "swap" {
                continue;
            }

            // Mount virtual filesystems
            match entry.fs_type.as_str() {
                "proc" => {
                    let mut vfs = vfs_lock();
                    let _ = vfs.mkdir_all(&entry.mount_point, &root_cred, Mode::from_octal(0o755));
                    let _ = mount_procfs(&mut vfs, &root_cred);
                }
                "sysfs" => {
                    let mut vfs = vfs_lock();
                    let _ = vfs.mkdir_all(&entry.mount_point, &root_cred, Mode::from_octal(0o755));
                    let _ = mount_sysfs(&mut vfs, &root_cred);
                }
                "devtmpfs" | "devfs" => {
                    let mut vfs = vfs_lock();
                    let _ = vfs.mkdir_all(&entry.mount_point, &root_cred, Mode::from_octal(0o755));
                    let _ = mount_devfs(&mut vfs, &root_cred);
                }
                "tmpfs" => {
                    let mut vfs = vfs_lock();
                    let _ = vfs.mkdir_all(&entry.mount_point, &root_cred, Mode::from_octal(0o755));
                    let tmpfs_root = tmpfs::TmpFs::new_root();
                    vfs.mount(&entry.mount_point, tmpfs_root);
                    crate::kprintln!("fstab: mounted tmpfs at {}", entry.mount_point);
                }
                _ => {
                    // For block devices, we'd need to look up the device and mount it
                    // For now, skip - this would require device resolution
                    crate::kprintln!("fstab: skipping {} (device mounting not implemented)",
                        entry.mount_point);
                }
            }
        }

        Ok(())
    }

    /// Get the init path from boot parameters or default
    pub fn get_init_path() -> String {
        if let Some(params) = boot_params() {
            if let Some(init) = params.init {
                return init;
            }
        }
        String::from("/bin/init")
    }
