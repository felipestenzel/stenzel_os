//! Camada de storage do kernel (blocos + particionamento + cache).
//!
//! Nesta fase, fazemos:
//! - Probe PCI e inicialização do virtio-blk (para QEMU)
//! - Exposição de um `BlockDevice` "root" para o restante do kernel
//!
//! Drivers de hardware vivem em `crate::drivers::*`, mas a integração
//! e seleção de dispositivo padrão vive aqui.

#![allow(dead_code)]

pub mod block;
pub mod cache;
pub mod gpt;
pub mod ramdisk;

pub use block::{BlockDevice, BlockDeviceId, PartitionBlockDevice};

use alloc::sync::Arc;
use spin::Once;

use crate::drivers;
use crate::fs;

static ROOT_BLOCK: Once<Arc<dyn BlockDevice>> = Once::new();
static ROOT_PARTITION: Once<Arc<dyn BlockDevice>> = Once::new();

/// Inicializa o subsistema de storage.
///
/// Hoje:
/// - tenta virtio-blk (PCI) -> ideal para QEMU
/// - fallback: ramdisk
pub fn init() {
    // PCI scan
    let pci_devs = drivers::pci::scan();
    crate::kprintln!("pci: encontrados {} devices", pci_devs.len());

    // Tenta virtio-blk (para QEMU)
    for d in &pci_devs {
        if let Some(vblk) = drivers::storage::virtio_blk::probe(d) {
            crate::kprintln!(
                "storage: virtio-blk candidato @ {:02x}:{:02x}.{} (vendor={:04x} dev={:04x})",
                d.addr.bus,
                d.addr.device,
                d.addr.function,
                d.id.vendor_id,
                d.id.device_id
            );
            match drivers::storage::virtio_blk::init(vblk) {
                Ok(dev) => {
                    crate::kprintln!("storage: virtio-blk inicializado ({} blocks)", dev.num_blocks());
                    ROOT_BLOCK.call_once(|| Arc::new(dev));
                    break;
                }
                Err(e) => {
                    crate::kprintln!("storage: virtio-blk falhou: {:?}", e);
                }
            }
        }
    }

    // Se não encontrou virtio-blk, tenta NVMe (para hardware real)
    if ROOT_BLOCK.get().is_none() {
        for d in &pci_devs {
            if let Some(nvme) = drivers::storage::nvme::probe(d) {
                crate::kprintln!(
                    "storage: NVMe controller @ {:02x}:{:02x}.{}",
                    d.addr.bus,
                    d.addr.device,
                    d.addr.function
                );
                match drivers::storage::nvme::init(nvme) {
                    Ok(dev) => {
                        crate::kprintln!("storage: NVMe inicializado ({} blocks)", dev.num_blocks());
                        ROOT_BLOCK.call_once(|| dev);
                        break;
                    }
                    Err(e) => {
                        crate::kprintln!("storage: NVMe falhou: {:?}", e);
                    }
                }
            }
        }
    }

    // Se ainda não encontrou, tenta AHCI (para SATA drives)
    if ROOT_BLOCK.get().is_none() {
        for d in &pci_devs {
            if let Some(ahci) = drivers::storage::ahci::probe(d) {
                crate::kprintln!(
                    "storage: AHCI controller @ {:02x}:{:02x}.{}",
                    d.addr.bus,
                    d.addr.device,
                    d.addr.function
                );
                match drivers::storage::ahci::init(ahci) {
                    Ok(dev) => {
                        crate::kprintln!("storage: AHCI inicializado ({} blocks)", dev.num_blocks());
                        ROOT_BLOCK.call_once(|| dev);
                        break;
                    }
                    Err(e) => {
                        crate::kprintln!("storage: AHCI falhou: {:?}", e);
                    }
                }
            }
        }
    }

    // Se ainda não encontrou nenhum dispositivo
    if ROOT_BLOCK.get().is_none() {
        crate::kprintln!("storage: nenhum dispositivo de bloco encontrado");
        crate::kprintln!("storage: continuando sem storage (somente tmpfs)");
        return;
    }

    // Lê GPT e monta a primeira partição
    if let Some(dev) = ROOT_BLOCK.get() {
        if let Ok(parts) = gpt::read_gpt_partitions(&**dev) {
            crate::kprintln!("gpt: {} partições", parts.len());
            for (i, p) in parts.iter().enumerate() {
                crate::kprintln!(
                    "  {}: lba_start={} lba_end={} type={:02x?}",
                    i,
                    p.first_lba,
                    p.last_lba,
                    p.type_guid
                );
            }

            // Monta a primeira partição como root
            if let Some(part0) = parts.first() {
                let partition = PartitionBlockDevice::new(
                    Arc::clone(dev),
                    part0.first_lba,
                    part0.last_lba,
                    BlockDeviceId(100), // ID da partição
                );
                let part_arc: Arc<dyn BlockDevice> = Arc::new(partition);
                ROOT_PARTITION.call_once(|| Arc::clone(&part_arc));

                // Tenta montar ext2
                crate::kprintln!("ext2: tentando montar partição 0...");
                match fs::mount_root_ext2(part_arc) {
                    Ok(()) => {
                        crate::kprintln!("ext2: montado com sucesso!");
                    }
                    Err(e) => {
                        crate::kprintln!("ext2: falha ao montar: {:?}", e);
                    }
                }
            }
        } else {
            crate::kprintln!("gpt: não encontrado / inválido (ok para testes)");
        }
    }
}

pub fn root_block() -> &'static Arc<dyn BlockDevice> {
    ROOT_BLOCK.get().expect("storage: ROOT_BLOCK não inicializado")
}
