use anyhow::Context;
use std::process::Command;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let use_uefi = args.iter().any(|a| a == "--uefi");
    let use_gui = args.iter().any(|a| a == "--gui");

    // Collect extra arguments to pass to QEMU (skip program name and our flags)
    let extra_args: Vec<&str> = args.iter()
        .skip(1)
        .filter(|a| *a != "--uefi" && *a != "--gui")
        .map(|s| s.as_str())
        .collect();

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
        // xHCI USB controller + USB hub + USB keyboard + USB mouse
        // Note: Devices connect through the hub's ports
        "-device",
        "qemu-xhci,id=xhci",
        "-device",
        "usb-hub,id=hub1,bus=xhci.0,port=1",
        "-device",
        "usb-kbd,bus=xhci.0,port=2",
        "-device",
        "usb-mouse,bus=xhci.0,port=3",
        "-serial",
        "stdio",
        "-no-reboot",
    ]);

    // Display mode: use --gui to show QEMU window (needed for USB keyboard testing)
    if use_gui {
        cmd.args(["-display", "cocoa"]);
    } else {
        cmd.args(["-display", "none"]);
    }

    // Add extra arguments
    cmd.args(&extra_args);

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
