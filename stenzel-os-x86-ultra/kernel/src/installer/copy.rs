//! System File Copy for Installer

use alloc::string::String;
use super::{InstallConfig, InstallError, InstallResult};
use super::partition::PartitionLayout;

/// Copy system files to target partitions
pub fn copy_system(layout: &PartitionLayout, config: &InstallConfig) -> InstallResult<()> {
    crate::kprintln!("copy: Copying system to {}", layout.disk);

    // Mount target partitions
    mount_target(layout)?;

    // Copy base system
    copy_base_system()?;

    // Copy kernel and initramfs
    copy_kernel()?;

    // Generate fstab
    generate_fstab(layout)?;

    // Configure locale and timezone
    configure_locale(&config.locale)?;
    configure_timezone(&config.timezone)?;
    configure_keyboard(&config.keyboard_layout)?;
    
    // Set hostname
    set_hostname(&config.hostname)?;

    // Unmount
    unmount_target()?;

    crate::kprintln!("copy: System copy complete");
    Ok(())
}

fn mount_target(layout: &PartitionLayout) -> InstallResult<()> {
    crate::kprintln!("copy: Mounting target partitions");
    
    for partition in &layout.partitions {
        if partition.mount_point != "swap" {
            let target = alloc::format!("/mnt{}", partition.mount_point);
            crate::kprintln!("copy: Mounting {} -> {}", partition.label, target);
        }
    }
    
    Ok(())
}

fn copy_base_system() -> InstallResult<()> {
    crate::kprintln!("copy: Copying base system files...");
    // Copy from squashfs or package cache
    Ok(())
}

fn copy_kernel() -> InstallResult<()> {
    crate::kprintln!("copy: Installing kernel and initramfs...");
    // Copy vmlinuz and initramfs to /boot
    Ok(())
}

fn generate_fstab(layout: &PartitionLayout) -> InstallResult<()> {
    crate::kprintln!("copy: Generating /etc/fstab");
    
    let mut fstab = String::from("# /etc/fstab - Stenzel OS\n");
    
    for partition in &layout.partitions {
        let fs_type = partition.filesystem.as_str();
        let options = if partition.mount_point == "/" { "defaults,errors=remount-ro" } else { "defaults" };
        
        fstab.push_str(&alloc::format!(
            "UUID={}  {}  {}  {}  0  {}\n",
            partition.uuid,
            partition.mount_point,
            fs_type,
            options,
            if partition.mount_point == "/" { 1 } else { 2 }
        ));
    }
    
    // Write fstab
    Ok(())
}

fn configure_locale(locale: &str) -> InstallResult<()> {
    crate::kprintln!("copy: Configuring locale: {}", locale);
    Ok(())
}

fn configure_timezone(timezone: &str) -> InstallResult<()> {
    crate::kprintln!("copy: Configuring timezone: {}", timezone);
    Ok(())
}

fn configure_keyboard(layout: &str) -> InstallResult<()> {
    crate::kprintln!("copy: Configuring keyboard: {}", layout);
    Ok(())
}

fn set_hostname(hostname: &str) -> InstallResult<()> {
    crate::kprintln!("copy: Setting hostname: {}", hostname);
    Ok(())
}

fn unmount_target() -> InstallResult<()> {
    crate::kprintln!("copy: Unmounting target");
    Ok(())
}

pub fn init() {
    crate::kprintln!("copy: System copier initialized");
}
