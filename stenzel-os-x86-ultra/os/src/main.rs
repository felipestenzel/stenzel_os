use anyhow::Context;
use std::process::Command;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let use_uefi = args.iter().any(|a| a == "--uefi");

    let bios_img = env!("STENZEL_BIOS_IMG");
    let uefi_img = env!("STENZEL_UEFI_IMG");
    let virtio_disk = env!("STENZEL_VIRTIO_DISK");
    let img = if use_uefi { uefi_img } else { bios_img };

    let mut cmd = Command::new("qemu-system-x86_64");

    cmd.args([
        "-m",
        "1024",
        // O kernel ainda é single-core (por design neste estágio).
        "-smp",
        "1",
        "-cpu",
        "qemu64",
        "-drive",
        &format!("format=raw,file={}", img),
        // virtio-blk (driver usa modo legacy I/O)
        "-drive",
        &format!("if=none,id=vdisk,format=raw,file={}", virtio_disk),
        "-device",
        "virtio-blk-pci,drive=vdisk,disable-modern=on",
        "-serial",
        "stdio",
        "-display",
        "none",
        "-no-reboot",
    ]);

    // Para UEFI, você precisa de OVMF. Exemplos comuns em Linux:
    //  - /usr/share/OVMF/OVMF_CODE.fd
    //  - /usr/share/edk2/ovmf/OVMF_CODE.fd
    // Ajuste conforme sua distro.
    if use_uefi {
        let ovmf = std::env::var("OVMF_CODE")
            .unwrap_or_else(|_| "/usr/share/OVMF/OVMF_CODE.fd".to_string());
        cmd.args(["-bios", &ovmf]);
    }

    let status = cmd.status().context("Falha ao rodar QEMU")?;
    if !status.success() {
        anyhow::bail!("QEMU saiu com status {}", status);
    }

    Ok(())
}
