//! sysfs - Virtual filesystem para informações de sistema e hardware.
//!
//! Expõe informações do kernel e dispositivos em /sys.

#![allow(dead_code)]

use alloc::format;
use alloc::string::ToString;
use alloc::sync::{Arc, Weak};
use alloc::vec;
use alloc::vec::Vec;

use spin::RwLock;

use crate::security::{Gid, Uid};
use crate::util::{KError, KResult};

use super::vfs::{DirEntry, Inode, InodeKind, InodeOps, Metadata, Mode};

/// Tipo de entrada no sysfs.
#[derive(Clone)]
enum SysContent {
    /// Diretório raiz /sys
    Root,
    /// Diretório /sys/kernel
    Kernel,
    /// Diretório /sys/devices
    Devices,
    /// Diretório /sys/class
    Class,
    /// Diretório /sys/class/block
    ClassBlock,
    /// Diretório /sys/class/tty
    ClassTty,
    /// Diretório /sys/class/graphics
    ClassGraphics,
    /// Diretório /sys/class/graphics/fb0
    ClassGraphicsFb0,
    /// Arquivo /sys/class/graphics/fb0/name
    Fb0Name,
    /// Arquivo /sys/class/graphics/fb0/bits_per_pixel
    Fb0BitsPerPixel,
    /// Arquivo /sys/class/graphics/fb0/mode
    Fb0Mode,
    /// Arquivo /sys/class/graphics/fb0/modes
    Fb0Modes,
    /// Arquivo /sys/class/graphics/fb0/virtual_size
    Fb0VirtualSize,
    /// Arquivo /sys/class/graphics/fb0/pan
    Fb0Pan,
    /// Arquivo /sys/class/graphics/fb0/stride
    Fb0Stride,
    /// Arquivo /sys/kernel/hostname
    Hostname,
    /// Arquivo /sys/kernel/osrelease
    OsRelease,
    /// Arquivo /sys/kernel/version
    Version,
    /// Arquivo /sys/kernel/ostype
    OsType,
    /// Diretório /sys/devices/system
    DevicesSystem,
    /// Diretório /sys/devices/system/cpu
    DevicesSystemCpu,
    /// Arquivo /sys/devices/system/cpu/online
    CpuOnline,
    /// Arquivo /sys/devices/system/cpu/possible
    CpuPossible,
    /// Diretório /sys/devices/virtual
    DevicesVirtual,
    /// Diretório /sys/devices/pci0000:00
    DevicesPci,
}

/// Inode do sysfs.
pub struct SysfsInode {
    content: SysContent,
    parent: RwLock<Option<Weak<SysfsInode>>>,
    self_weak: Weak<SysfsInode>,
}

impl SysfsInode {
    fn metadata_for_content(content: &SysContent) -> Metadata {
        match content {
            // Diretórios
            SysContent::Root
            | SysContent::Kernel
            | SysContent::Devices
            | SysContent::Class
            | SysContent::ClassBlock
            | SysContent::ClassTty
            | SysContent::ClassGraphics
            | SysContent::ClassGraphicsFb0
            | SysContent::DevicesSystem
            | SysContent::DevicesSystemCpu
            | SysContent::DevicesVirtual
            | SysContent::DevicesPci => Metadata::simple(
                Uid(0),
                Gid(0),
                Mode::from_octal(0o755),
                InodeKind::Dir,
            ),
            // Arquivos
            _ => Metadata::simple(
                Uid(0),
                Gid(0),
                Mode::from_octal(0o444),
                InodeKind::File,
            ),
        }
    }

    /// Gera o conteúdo do arquivo dinamicamente.
    fn generate_content(&self) -> Vec<u8> {
        match &self.content {
            SysContent::Hostname => b"stenzel\n".to_vec(),
            SysContent::OsRelease => format!("{}\n", env!("CARGO_PKG_VERSION")).into_bytes(),
            SysContent::Version => format!(
                "#1 SMP Stenzel OS {} (Rust)\n",
                env!("CARGO_PKG_VERSION")
            ).into_bytes(),
            SysContent::OsType => b"StenzelOS\n".to_vec(),
            SysContent::CpuOnline => b"0\n".to_vec(), // CPU 0 online
            SysContent::CpuPossible => b"0\n".to_vec(), // Apenas CPU 0 por enquanto
            // Framebuffer entries
            SysContent::Fb0Name => b"StenzelOS FB\n".to_vec(),
            SysContent::Fb0BitsPerPixel => {
                crate::drivers::framebuffer::bits_per_pixel_string()
                    .map(|s| format!("{}\n", s).into_bytes())
                    .unwrap_or_else(|| b"0\n".to_vec())
            }
            SysContent::Fb0Mode => {
                crate::drivers::framebuffer::mode_string()
                    .map(|s| format!("{}\n", s).into_bytes())
                    .unwrap_or_else(|| b"none\n".to_vec())
            }
            SysContent::Fb0Modes => {
                // List of available modes (only the current mode is available)
                crate::drivers::framebuffer::mode_string()
                    .map(|s| format!("{}\n", s).into_bytes())
                    .unwrap_or_else(|| Vec::new())
            }
            SysContent::Fb0VirtualSize => {
                crate::drivers::framebuffer::virtual_size_string()
                    .map(|s| format!("{}\n", s).into_bytes())
                    .unwrap_or_else(|| b"0,0\n".to_vec())
            }
            SysContent::Fb0Pan => {
                crate::drivers::framebuffer::pan_string()
                    .map(|s| format!("{}\n", s).into_bytes())
                    .unwrap_or_else(|| b"0,0\n".to_vec())
            }
            SysContent::Fb0Stride => {
                crate::drivers::framebuffer::stride_string()
                    .map(|s| format!("{}\n", s).into_bytes())
                    .unwrap_or_else(|| b"0\n".to_vec())
            }
            _ => Vec::new(),
        }
    }

    /// Lista entradas de diretório.
    fn list_entries(&self) -> Vec<DirEntry> {
        match &self.content {
            SysContent::Root => vec![
                DirEntry { name: "kernel".to_string(), kind: InodeKind::Dir },
                DirEntry { name: "devices".to_string(), kind: InodeKind::Dir },
                DirEntry { name: "class".to_string(), kind: InodeKind::Dir },
            ],
            SysContent::Kernel => vec![
                DirEntry { name: "hostname".to_string(), kind: InodeKind::File },
                DirEntry { name: "osrelease".to_string(), kind: InodeKind::File },
                DirEntry { name: "version".to_string(), kind: InodeKind::File },
                DirEntry { name: "ostype".to_string(), kind: InodeKind::File },
            ],
            SysContent::Devices => vec![
                DirEntry { name: "system".to_string(), kind: InodeKind::Dir },
                DirEntry { name: "virtual".to_string(), kind: InodeKind::Dir },
                DirEntry { name: "pci0000:00".to_string(), kind: InodeKind::Dir },
            ],
            SysContent::DevicesSystem => vec![
                DirEntry { name: "cpu".to_string(), kind: InodeKind::Dir },
            ],
            SysContent::DevicesSystemCpu => vec![
                DirEntry { name: "online".to_string(), kind: InodeKind::File },
                DirEntry { name: "possible".to_string(), kind: InodeKind::File },
            ],
            SysContent::Class => {
                let mut entries = vec![
                    DirEntry { name: "block".to_string(), kind: InodeKind::Dir },
                    DirEntry { name: "tty".to_string(), kind: InodeKind::Dir },
                ];
                // Only show graphics if framebuffer is available
                if crate::drivers::framebuffer::is_available() {
                    entries.push(DirEntry { name: "graphics".to_string(), kind: InodeKind::Dir });
                }
                entries
            }
            SysContent::ClassGraphics => {
                // Show fb0 if framebuffer is available
                if crate::drivers::framebuffer::is_available() {
                    vec![DirEntry { name: "fb0".to_string(), kind: InodeKind::Dir }]
                } else {
                    Vec::new()
                }
            }
            SysContent::ClassGraphicsFb0 => {
                vec![
                    DirEntry { name: "name".to_string(), kind: InodeKind::File },
                    DirEntry { name: "bits_per_pixel".to_string(), kind: InodeKind::File },
                    DirEntry { name: "mode".to_string(), kind: InodeKind::File },
                    DirEntry { name: "modes".to_string(), kind: InodeKind::File },
                    DirEntry { name: "virtual_size".to_string(), kind: InodeKind::File },
                    DirEntry { name: "pan".to_string(), kind: InodeKind::File },
                    DirEntry { name: "stride".to_string(), kind: InodeKind::File },
                ]
            }
            SysContent::ClassBlock | SysContent::ClassTty => {
                // Por enquanto, vazio - será populado quando tivermos mais dispositivos
                Vec::new()
            }
            SysContent::DevicesVirtual | SysContent::DevicesPci => {
                // Será populado dinamicamente no futuro
                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    /// Faz lookup de uma entrada no diretório.
    fn lookup_entry(&self, name: &str) -> KResult<SysContent> {
        match &self.content {
            SysContent::Root => match name {
                "kernel" => Ok(SysContent::Kernel),
                "devices" => Ok(SysContent::Devices),
                "class" => Ok(SysContent::Class),
                _ => Err(KError::NotFound),
            },
            SysContent::Kernel => match name {
                "hostname" => Ok(SysContent::Hostname),
                "osrelease" => Ok(SysContent::OsRelease),
                "version" => Ok(SysContent::Version),
                "ostype" => Ok(SysContent::OsType),
                _ => Err(KError::NotFound),
            },
            SysContent::Devices => match name {
                "system" => Ok(SysContent::DevicesSystem),
                "virtual" => Ok(SysContent::DevicesVirtual),
                "pci0000:00" => Ok(SysContent::DevicesPci),
                _ => Err(KError::NotFound),
            },
            SysContent::DevicesSystem => match name {
                "cpu" => Ok(SysContent::DevicesSystemCpu),
                _ => Err(KError::NotFound),
            },
            SysContent::DevicesSystemCpu => match name {
                "online" => Ok(SysContent::CpuOnline),
                "possible" => Ok(SysContent::CpuPossible),
                _ => Err(KError::NotFound),
            },
            SysContent::Class => match name {
                "block" => Ok(SysContent::ClassBlock),
                "tty" => Ok(SysContent::ClassTty),
                "graphics" => {
                    if crate::drivers::framebuffer::is_available() {
                        Ok(SysContent::ClassGraphics)
                    } else {
                        Err(KError::NotFound)
                    }
                }
                _ => Err(KError::NotFound),
            },
            SysContent::ClassGraphics => match name {
                "fb0" => {
                    if crate::drivers::framebuffer::is_available() {
                        Ok(SysContent::ClassGraphicsFb0)
                    } else {
                        Err(KError::NotFound)
                    }
                }
                _ => Err(KError::NotFound),
            },
            SysContent::ClassGraphicsFb0 => match name {
                "name" => Ok(SysContent::Fb0Name),
                "bits_per_pixel" => Ok(SysContent::Fb0BitsPerPixel),
                "mode" => Ok(SysContent::Fb0Mode),
                "modes" => Ok(SysContent::Fb0Modes),
                "virtual_size" => Ok(SysContent::Fb0VirtualSize),
                "pan" => Ok(SysContent::Fb0Pan),
                "stride" => Ok(SysContent::Fb0Stride),
                _ => Err(KError::NotFound),
            },
            _ => Err(KError::Invalid),
        }
    }
}

impl InodeOps for SysfsInode {
    fn metadata(&self) -> Metadata {
        Self::metadata_for_content(&self.content)
    }

    fn set_metadata(&self, _meta: Metadata) {
        // sysfs é read-only (exceto alguns arquivos especiais no futuro)
    }

    fn parent(&self) -> Option<Inode> {
        let p = self.parent.read();
        p.as_ref()
            .and_then(|w| w.upgrade())
            .map(|arc| Inode(arc))
    }

    fn lookup(&self, name: &str) -> KResult<Inode> {
        let content = self.lookup_entry(name)?;
        let parent_weak = self.self_weak.clone();

        let inode: Arc<SysfsInode> = Arc::new_cyclic(|weak| SysfsInode {
            content,
            parent: RwLock::new(Some(parent_weak)),
            self_weak: weak.clone(),
        });

        Ok(Inode(inode))
    }

    fn create(&self, _name: &str, _kind: InodeKind, _meta: Metadata) -> KResult<Inode> {
        // sysfs é read-only
        Err(KError::NotSupported)
    }

    fn readdir(&self) -> KResult<Vec<DirEntry>> {
        Ok(self.list_entries())
    }

    fn read_at(&self, offset: usize, out: &mut [u8]) -> KResult<usize> {
        let data = self.generate_content();
        if offset >= data.len() {
            return Ok(0);
        }
        let n = core::cmp::min(out.len(), data.len() - offset);
        out[..n].copy_from_slice(&data[offset..offset + n]);
        Ok(n)
    }

    fn write_at(&self, _offset: usize, _data: &[u8]) -> KResult<usize> {
        // sysfs é read-only (por enquanto)
        Err(KError::NotSupported)
    }

    fn truncate(&self, _size: usize) -> KResult<()> {
        Err(KError::NotSupported)
    }

    fn size(&self) -> KResult<usize> {
        Ok(self.generate_content().len())
    }
}

/// Cria o inode raiz do sysfs (/sys).
pub fn new_root() -> Inode {
    let inode: Arc<SysfsInode> = Arc::new_cyclic(|weak| SysfsInode {
        content: SysContent::Root,
        parent: RwLock::new(None),
        self_weak: weak.clone(),
    });
    Inode(inode)
}
