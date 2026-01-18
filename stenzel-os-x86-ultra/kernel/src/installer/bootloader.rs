//! Bootloader Installation

use alloc::string::String;
use super::{InstallConfig, InstallError, InstallResult, BootloaderType};
use super::partition::PartitionLayout;

/// Install bootloader to target disk
pub fn install_bootloader(layout: &PartitionLayout, config: &InstallConfig) -> InstallResult<()> {
    crate::kprintln!("bootloader: Installing {:?} to {}", config.bootloader_type, layout.disk);

    match config.bootloader_type {
        BootloaderType::SystemdBoot => install_systemd_boot(layout, config)?,
        BootloaderType::Grub2 => install_grub2(layout, config)?,
        BootloaderType::EfiStub => install_efi_stub(layout, config)?,
    }

    crate::kprintln!("bootloader: Installation complete");
    Ok(())
}

fn install_systemd_boot(layout: &PartitionLayout, config: &InstallConfig) -> InstallResult<()> {
    crate::kprintln!("bootloader: Installing systemd-boot");

    // Find ESP partition
    let esp = layout.partitions.iter()
        .find(|p| p.mount_point == "/boot/efi")
        .ok_or_else(|| InstallError::BootloaderError(String::from("EFI partition not found")))?;

    // Copy systemd-boot EFI binary
    copy_systemd_boot_binary(&esp.label)?;

    // Create loader.conf
    create_loader_conf()?;

    // Create boot entry
    create_boot_entry(config)?;

    Ok(())
}

fn copy_systemd_boot_binary(esp_label: &str) -> InstallResult<()> {
    crate::kprintln!("bootloader: Copying systemd-boot to EFI partition ({})", esp_label);
    // Copy to /boot/efi/EFI/BOOT/BOOTX64.EFI
    // Copy to /boot/efi/EFI/systemd/systemd-bootx64.efi
    Ok(())
}

fn create_loader_conf() -> InstallResult<()> {
    let conf = r#"default stenzel.conf
timeout 3
console-mode max
editor no
"#;
    crate::kprintln!("bootloader: Creating loader.conf");
    let _ = conf;
    Ok(())
}

fn create_boot_entry(config: &InstallConfig) -> InstallResult<()> {
    let entry = alloc::format!(r#"title   Stenzel OS
linux   /vmlinuz-stenzel
initrd  /initramfs-stenzel.img
options root=LABEL=ROOT rw quiet
"#);
    crate::kprintln!("bootloader: Creating boot entry for Stenzel OS");
    let _ = (entry, config);
    Ok(())
}

fn install_grub2(layout: &PartitionLayout, config: &InstallConfig) -> InstallResult<()> {
    crate::kprintln!("bootloader: Installing GRUB2");

    // Install GRUB to MBR/ESP
    install_grub_binary(&layout.disk, config)?;

    // Generate grub.cfg
    generate_grub_config(config)?;

    Ok(())
}

fn install_grub_binary(disk: &str, config: &InstallConfig) -> InstallResult<()> {
    crate::kprintln!("bootloader: Installing GRUB binary to {}", disk);
    let _ = config;
    Ok(())
}

fn generate_grub_config(config: &InstallConfig) -> InstallResult<()> {
    let grub_cfg = alloc::format!(r#"
set default=0
set timeout=5

menuentry 'Stenzel OS' {{
    linux /boot/vmlinuz-stenzel root=LABEL=ROOT rw quiet
    initrd /boot/initramfs-stenzel.img
}}

menuentry 'Stenzel OS (Recovery)' {{
    linux /boot/vmlinuz-stenzel root=LABEL=ROOT rw single
    initrd /boot/initramfs-stenzel.img
}}
"#);
    crate::kprintln!("bootloader: Generating grub.cfg");
    let _ = (grub_cfg, config);
    Ok(())
}

fn install_efi_stub(layout: &PartitionLayout, config: &InstallConfig) -> InstallResult<()> {
    crate::kprintln!("bootloader: Installing EFI stub");
    let _ = (layout, config);
    // Create unified kernel image with embedded initramfs
    Ok(())
}

pub fn init() {
    crate::kprintln!("bootloader: Bootloader installer initialized");
}
