//! Userland binaries embarcados no kernel.
//!
//! Contém os binários ELF pré-compilados que serão instalados
//! no tmpfs durante o boot.

#![allow(dead_code)]

/// Binário /bin/init (PID 1)
pub static INIT_ELF: &[u8] = include_bytes!("init");

/// Binário /bin/sh (shell)
pub static SH_ELF: &[u8] = include_bytes!("sh");

/// Instala os binários userland no filesystem.
pub fn install_userland_binaries() {
    use crate::fs;
    use crate::security;

    let root_cred = security::user_db().login("root").expect("root user");
    let mut vfs = fs::vfs_lock();

    // Instala /bin/init
    match vfs.write_file(
        "/bin/init",
        &root_cred,
        fs::Mode::from_octal(0o755),
        INIT_ELF,
    ) {
        Ok(_) => crate::kprintln!("userland: /bin/init instalado ({} bytes)", INIT_ELF.len()),
        Err(e) => crate::kprintln!("userland: erro ao instalar /bin/init: {:?}", e),
    }

    // Instala /bin/sh
    match vfs.write_file(
        "/bin/sh",
        &root_cred,
        fs::Mode::from_octal(0o755),
        SH_ELF,
    ) {
        Ok(_) => crate::kprintln!("userland: /bin/sh instalado ({} bytes)", SH_ELF.len()),
        Err(e) => crate::kprintln!("userland: erro ao instalar /bin/sh: {:?}", e),
    }

    crate::kprintln!("userland: binários instalados com sucesso");
}
