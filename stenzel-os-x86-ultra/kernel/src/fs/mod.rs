    #![allow(dead_code)]

    use alloc::string::String;
    use alloc::sync::Arc;
    use spin::Once;

    use crate::security::{self, Cred};
    use crate::storage::BlockDevice;
    use crate::sync::IrqSafeMutex;
    use crate::util::KResult;

    pub mod ext2;
    mod perm;
    mod tmpfs;
    mod vfs;

    pub use vfs::{Inode, InodeKind, Metadata, Mode, Vfs};

    static VFS: Once<IrqSafeMutex<Vfs>> = Once::new();

    pub fn init() {
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

    /// Monta uma partição ext2 do dispositivo de bloco
    pub fn mount_ext2(device: Arc<dyn BlockDevice>) -> KResult<Arc<ext2::Ext2Fs>> {
        ext2::Ext2Fs::mount(device)
    }

    /// Tenta montar ext2 da partição root e integrar no VFS
    pub fn mount_root_ext2(device: Arc<dyn BlockDevice>) -> KResult<()> {
        let fs = ext2::Ext2Fs::mount(device)?;
        let _root = fs.root();

        // Por enquanto, apenas logamos sucesso
        // No futuro: substituir o VFS root ou montar em /mnt
        crate::kprintln!("ext2: filesystem montado com sucesso");

        Ok(())
    }
