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
pub mod mbr;
pub mod raid;
pub mod ramdisk;
pub mod trim;
pub mod iosched;
pub mod writecache;

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

    // Se ainda não encontrou, tenta IDE (para drives legados)
    if ROOT_BLOCK.get().is_none() {
        // Probe for IDE controller via PCI
        for d in &pci_devs {
            if drivers::storage::ide::probe(d).is_some() {
                break; // Found IDE controller
            }
        }
        // Initialize IDE regardless of PCI (legacy ISA IDE)
        if let Ok(dev) = drivers::storage::ide::init_from_pci(()) {
            crate::kprintln!("storage: IDE inicializado ({} blocks)", dev.num_blocks());
            ROOT_BLOCK.call_once(|| dev);
        }
    }

    // Se ainda não encontrou nenhum dispositivo
    if ROOT_BLOCK.get().is_none() {
        crate::kprintln!("storage: nenhum dispositivo de bloco encontrado");
        crate::kprintln!("storage: continuando sem storage (somente tmpfs)");
        return;
    }

    // Lê GPT ou MBR e monta a primeira partição
    if let Some(dev) = ROOT_BLOCK.get() {
        // Try GPT first
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
        } else if let Ok(parts) = mbr::read_mbr_with_logical(&**dev) {
            // Try MBR if GPT not found
            mbr::print_mbr_info(&parts);

            // Mount first usable partition
            if let Some(part0) = parts.iter().find(|p| !p.partition_type.is_extended()) {
                let partition = PartitionBlockDevice::new(
                    Arc::clone(dev),
                    part0.first_lba as u64,
                    part0.last_lba() as u64,
                    BlockDeviceId(100),
                );
                let part_arc: Arc<dyn BlockDevice> = Arc::new(partition);
                ROOT_PARTITION.call_once(|| Arc::clone(&part_arc));

                // Try to mount based on partition type
                if part0.partition_type.is_linux() {
                    crate::kprintln!("ext2: tentando montar partição Linux...");
                    match fs::mount_root_ext2(Arc::clone(&part_arc)) {
                        Ok(()) => {
                            crate::kprintln!("ext2: montado com sucesso!");
                        }
                        Err(e) => {
                            crate::kprintln!("ext2: falha ao montar: {:?}", e);
                        }
                    }
                } else if part0.partition_type.is_fat() {
                    crate::kprintln!("fat32: tentando montar partição FAT...");
                    match fs::fat32::Fat32Fs::mount(part_arc) {
                        Ok(fat) => {
                            crate::kprintln!("fat32: montado com sucesso!");
                            // Mount FAT32 at /mnt
                            let root_cred = crate::security::user_db().login("root").expect("root user");
                            let mut vfs = fs::vfs_lock();
                            let _ = vfs.mkdir_all("/mnt", &root_cred, fs::Mode::from_octal(0o755));
                            vfs.mount("/mnt", fat.root());
                        }
                        Err(e) => {
                            crate::kprintln!("fat32: falha ao montar: {:?}", e);
                        }
                    }
                } else {
                    crate::kprintln!("mbr: tipo de partição não suportado: {:?}", part0.partition_type);
                }
            }
        } else {
            crate::kprintln!("storage: nenhuma tabela de partição encontrada (GPT/MBR)");
        }
    }
}

pub fn root_block() -> &'static Arc<dyn BlockDevice> {
    ROOT_BLOCK.get().expect("storage: ROOT_BLOCK não inicializado")
}

/// Find a block device by path (e.g., "/dev/sda1")
pub fn find_device_by_path(path: &str) -> Option<usize> {
    // Simple path parsing - in a real implementation this would query devfs
    // For now, we support:
    // /dev/sda, /dev/sdb, etc. -> root block device
    // /dev/sda1, /dev/sda2, etc. -> partitions

    if !path.starts_with("/dev/") {
        return None;
    }

    let name = &path[5..]; // Remove "/dev/"

    // Check if it's the root block device
    if name == "sda" || name == "vda" || name == "nvme0n1" {
        if ROOT_BLOCK.get().is_some() {
            return Some(0); // Root device ID
        }
    }

    // Check for partition (e.g., sda1, vda1, nvme0n1p1)
    if name.starts_with("sda") || name.starts_with("vda") || name.starts_with("nvme0n1p") {
        if ROOT_PARTITION.get().is_some() {
            return Some(100); // Partition device ID
        }
    }

    None
}

/// Get a block device by ID
pub fn get_device(device_id: usize) -> Option<Arc<dyn BlockDevice>> {
    match device_id {
        0 => ROOT_BLOCK.get().cloned(),
        100 => ROOT_PARTITION.get().cloned(),
        _ => None,
    }
}

/// Get root partition if available
pub fn root_partition() -> Option<&'static Arc<dyn BlockDevice>> {
    ROOT_PARTITION.get()
}

// ============================================================================
// Disk Discovery API (for installer)
// ============================================================================

use alloc::string::String;
use alloc::vec::Vec;

/// Information about a discovered disk drive
#[derive(Debug, Clone)]
pub struct DiskDriveInfo {
    pub path: String,
    pub model: String,
    pub size_bytes: u64,
    pub block_size: u32,
    pub is_ssd: bool,
    pub is_nvme: bool,
    pub is_removable: bool,
}

/// List all available disk drives in the system
pub fn list_drives() -> Vec<DiskDriveInfo> {
    let mut drives = Vec::new();
    let pci_devs = crate::drivers::pci::scan();

    // Check for NVMe drives
    for d in &pci_devs {
        if let Some(nvme) = crate::drivers::storage::nvme::probe(d) {
            if let Ok(dev) = crate::drivers::storage::nvme::init(nvme) {
                drives.push(DiskDriveInfo {
                    path: String::from("/dev/nvme0n1"),
                    model: String::from("NVMe SSD"),
                    size_bytes: dev.num_blocks() * dev.block_size() as u64,
                    block_size: dev.block_size(),
                    is_ssd: true,
                    is_nvme: true,
                    is_removable: false,
                });
            }
        }
    }

    // Check for AHCI/SATA drives
    for d in &pci_devs {
        if let Some(ahci) = crate::drivers::storage::ahci::probe(d) {
            if let Ok(dev) = crate::drivers::storage::ahci::init(ahci) {
                drives.push(DiskDriveInfo {
                    path: String::from("/dev/sda"),
                    model: String::from("SATA Drive"),
                    size_bytes: dev.num_blocks() * dev.block_size() as u64,
                    block_size: dev.block_size(),
                    is_ssd: false, // TODO: Detect via TRIM support
                    is_nvme: false,
                    is_removable: false,
                });
            }
        }
    }

    // Check for virtio-blk (QEMU)
    for d in &pci_devs {
        if let Some(vblk) = crate::drivers::storage::virtio_blk::probe(d) {
            if let Ok(dev) = crate::drivers::storage::virtio_blk::init(vblk) {
                drives.push(DiskDriveInfo {
                    path: String::from("/dev/vda"),
                    model: String::from("VirtIO Block Device"),
                    size_bytes: dev.num_blocks() * dev.block_size() as u64,
                    block_size: dev.block_size(),
                    is_ssd: true,
                    is_nvme: false,
                    is_removable: false,
                });
            }
        }
    }

    drives
}

/// Get total storage capacity in bytes
pub fn total_storage_capacity() -> u64 {
    list_drives().iter().map(|d| d.size_bytes).sum()
}

/// Check if system has NVMe storage
pub fn has_nvme() -> bool {
    list_drives().iter().any(|d| d.is_nvme)
}

/// Check if system has SATA storage
pub fn has_sata() -> bool {
    list_drives().iter().any(|d| !d.is_nvme && !d.path.starts_with("/dev/vd"))
}
