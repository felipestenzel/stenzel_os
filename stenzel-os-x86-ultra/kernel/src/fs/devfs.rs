//! devfs - Virtual filesystem para device nodes.
//!
//! Expõe dispositivos em /dev (null, zero, urandom, tty, console).

#![allow(dead_code)]

use alloc::string::ToString;
use alloc::sync::{Arc, Weak};
use alloc::vec;
use alloc::vec::Vec;

use spin::RwLock;

use crate::security::{Gid, Uid};
use crate::util::{KError, KResult};

use super::vfs::{DirEntry, Inode, InodeKind, InodeOps, Metadata, Mode};

/// Tipo de dispositivo no devfs.
#[derive(Clone, Copy, PartialEq, Eq)]
enum DeviceType {
    /// Diretório raiz /dev
    Root,
    /// /dev/null - descarta escrita, retorna EOF
    Null,
    /// /dev/zero - retorna zeros, descarta escrita
    Zero,
    /// /dev/urandom - retorna bytes pseudo-aleatórios
    Urandom,
    /// /dev/tty - terminal atual
    Tty,
    /// /dev/console - console do sistema
    Console,
    /// /dev/stdin - link para fd 0
    Stdin,
    /// /dev/stdout - link para fd 1
    Stdout,
    /// /dev/stderr - link para fd 2
    Stderr,
    /// /dev/input - input devices directory
    InputDir,
    /// /dev/input/eventN - input event device
    InputEvent(u8),
    /// /dev/fb0 - framebuffer device
    Fb0,
}

/// Estado do PRNG para /dev/urandom
static URANDOM_STATE: spin::Mutex<u64> = spin::Mutex::new(0x853c49e6748fea9b);

/// Gera um byte pseudo-aleatório usando xorshift64
pub fn random_byte() -> u8 {
    let mut state = URANDOM_STATE.lock();
    // xorshift64
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    (*state & 0xFF) as u8
}

/// Semeia o PRNG com entropia do TSC
pub fn seed_urandom() {
    let tsc = unsafe {
        core::arch::x86_64::_rdtsc()
    };
    let mut state = URANDOM_STATE.lock();
    *state ^= tsc;
    // Mix adicional
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
}

/// Inode do devfs.
pub struct DevfsInode {
    device: DeviceType,
    parent: RwLock<Option<Weak<DevfsInode>>>,
    self_weak: Weak<DevfsInode>,
}

impl DevfsInode {
    fn metadata_for_device(device: DeviceType) -> Metadata {
        match device {
            DeviceType::Root | DeviceType::InputDir => Metadata::simple(
                Uid(0),
                Gid(0),
                Mode::from_octal(0o755),
                InodeKind::Dir,
            ),
            DeviceType::Null | DeviceType::Zero | DeviceType::Urandom => Metadata::simple(
                Uid(0),
                Gid(0),
                Mode::from_octal(0o666),
                InodeKind::CharDev,
            ),
            DeviceType::Tty | DeviceType::Console => Metadata::simple(
                Uid(0),
                Gid(0),
                Mode::from_octal(0o620),
                InodeKind::CharDev,
            ),
            DeviceType::Stdin | DeviceType::Stdout | DeviceType::Stderr => Metadata::simple(
                Uid(0),
                Gid(0),
                Mode::from_octal(0o777),
                InodeKind::Symlink,
            ),
            DeviceType::InputEvent(_) => Metadata::simple(
                Uid(0),
                Gid(0),
                Mode::from_octal(0o660),
                InodeKind::CharDev,
            ),
            DeviceType::Fb0 => Metadata::simple(
                Uid(0),
                Gid(0),
                Mode::from_octal(0o660),
                InodeKind::CharDev,
            ),
        }
    }

    /// Lista entradas do diretório /dev
    fn list_entries(&self) -> Vec<DirEntry> {
        match self.device {
            DeviceType::Root => {
                let mut entries = vec![
                    DirEntry { name: "null".to_string(), kind: InodeKind::CharDev },
                    DirEntry { name: "zero".to_string(), kind: InodeKind::CharDev },
                    DirEntry { name: "urandom".to_string(), kind: InodeKind::CharDev },
                    DirEntry { name: "random".to_string(), kind: InodeKind::CharDev },
                    DirEntry { name: "tty".to_string(), kind: InodeKind::CharDev },
                    DirEntry { name: "console".to_string(), kind: InodeKind::CharDev },
                    DirEntry { name: "stdin".to_string(), kind: InodeKind::Symlink },
                    DirEntry { name: "stdout".to_string(), kind: InodeKind::Symlink },
                    DirEntry { name: "stderr".to_string(), kind: InodeKind::Symlink },
                    DirEntry { name: "input".to_string(), kind: InodeKind::Dir },
                ];
                // Add fb0 if framebuffer is available
                if crate::drivers::framebuffer::is_available() {
                    entries.push(DirEntry { name: "fb0".to_string(), kind: InodeKind::CharDev });
                }
                entries
            }
            DeviceType::InputDir => {
                let count = crate::drivers::input::device_count();
                let mut entries = Vec::with_capacity(count);
                for i in 0..count {
                    entries.push(DirEntry {
                        name: alloc::format!("event{}", i),
                        kind: InodeKind::CharDev,
                    });
                }
                entries
            }
            _ => Vec::new(),
        }
    }

    /// Lookup de um dispositivo pelo nome
    fn lookup_device(&self, name: &str) -> KResult<DeviceType> {
        match self.device {
            DeviceType::Root => match name {
                "null" => Ok(DeviceType::Null),
                "zero" => Ok(DeviceType::Zero),
                "urandom" | "random" => Ok(DeviceType::Urandom),
                "tty" => Ok(DeviceType::Tty),
                "console" => Ok(DeviceType::Console),
                "stdin" => Ok(DeviceType::Stdin),
                "stdout" => Ok(DeviceType::Stdout),
                "stderr" => Ok(DeviceType::Stderr),
                "input" => Ok(DeviceType::InputDir),
                "fb0" => {
                    if crate::drivers::framebuffer::is_available() {
                        Ok(DeviceType::Fb0)
                    } else {
                        Err(KError::NotFound)
                    }
                }
                _ => Err(KError::NotFound),
            },
            DeviceType::InputDir => {
                // Parse "eventN" names
                if let Some(num_str) = name.strip_prefix("event") {
                    if let Ok(n) = num_str.parse::<u8>() {
                        let count = crate::drivers::input::device_count();
                        if (n as usize) < count {
                            return Ok(DeviceType::InputEvent(n));
                        }
                    }
                }
                Err(KError::NotFound)
            }
            _ => Err(KError::Invalid),
        }
    }
}

impl InodeOps for DevfsInode {
    fn metadata(&self) -> Metadata {
        Self::metadata_for_device(self.device)
    }

    fn set_metadata(&self, _meta: Metadata) {
        // devfs é read-only (metadados fixos)
    }

    fn parent(&self) -> Option<Inode> {
        let p = self.parent.read();
        p.as_ref()
            .and_then(|w| w.upgrade())
            .map(|arc| Inode(arc))
    }

    fn lookup(&self, name: &str) -> KResult<Inode> {
        let device = self.lookup_device(name)?;
        let parent_weak = self.self_weak.clone();

        let inode: Arc<DevfsInode> = Arc::new_cyclic(|weak| DevfsInode {
            device,
            parent: RwLock::new(Some(parent_weak)),
            self_weak: weak.clone(),
        });

        Ok(Inode(inode))
    }

    fn create(&self, _name: &str, _kind: InodeKind, _meta: Metadata) -> KResult<Inode> {
        // devfs é read-only
        Err(KError::NotSupported)
    }

    fn readdir(&self) -> KResult<Vec<DirEntry>> {
        Ok(self.list_entries())
    }

    fn read_at(&self, offset: usize, out: &mut [u8]) -> KResult<usize> {
        match self.device {
            DeviceType::Null => {
                // /dev/null sempre retorna EOF
                Ok(0)
            }
            DeviceType::Zero => {
                // /dev/zero retorna zeros
                for b in out.iter_mut() {
                    *b = 0;
                }
                Ok(out.len())
            }
            DeviceType::Urandom => {
                // /dev/urandom retorna bytes aleatórios
                for b in out.iter_mut() {
                    *b = random_byte();
                }
                Ok(out.len())
            }
            DeviceType::Tty | DeviceType::Console => {
                // Redireciona para o console do kernel
                // Por enquanto, apenas retorna EOF (a ser integrado com o console real)
                let _ = offset;
                Ok(0)
            }
            DeviceType::Stdin | DeviceType::Stdout | DeviceType::Stderr => {
                // Symlinks não são lidos diretamente
                Err(KError::Invalid)
            }
            DeviceType::InputEvent(n) => {
                // Read input events from the device
                if let Some(device) = crate::drivers::input::get_device(n as usize) {
                    let bytes_read = device.read(out);
                    Ok(bytes_read)
                } else {
                    Err(KError::NotFound)
                }
            }
            DeviceType::Fb0 => {
                // Read framebuffer data
                if let Some(bytes_read) = crate::drivers::framebuffer::with_framebuffer(|fb| {
                    let buffer = fb.buffer();
                    if offset >= buffer.len() {
                        return 0;
                    }
                    let available = buffer.len() - offset;
                    let to_read = out.len().min(available);
                    out[..to_read].copy_from_slice(&buffer[offset..offset + to_read]);
                    to_read
                }) {
                    Ok(bytes_read)
                } else {
                    Err(KError::NotFound)
                }
            }
            DeviceType::Root | DeviceType::InputDir => Err(KError::Invalid),
        }
    }

    fn write_at(&self, _offset: usize, data: &[u8]) -> KResult<usize> {
        match self.device {
            DeviceType::Null | DeviceType::Zero | DeviceType::Urandom => {
                // Descarta dados
                Ok(data.len())
            }
            DeviceType::Tty | DeviceType::Console => {
                // Escreve no console do kernel
                for &b in data {
                    crate::console::write_byte(b);
                }
                Ok(data.len())
            }
            DeviceType::Stdin | DeviceType::Stdout | DeviceType::Stderr => {
                // Symlinks não são escritos diretamente
                Err(KError::Invalid)
            }
            DeviceType::InputEvent(_) => {
                // Writing to input devices could be used for force feedback, etc.
                // For now, just discard
                Ok(data.len())
            }
            DeviceType::Fb0 => {
                // Write to framebuffer
                let offset = _offset; // Using the offset parameter
                if let Some(bytes_written) = crate::drivers::framebuffer::with_framebuffer(|fb| {
                    let buffer = fb.buffer_mut();
                    if offset >= buffer.len() {
                        return 0;
                    }
                    let available = buffer.len() - offset;
                    let to_write = data.len().min(available);
                    buffer[offset..offset + to_write].copy_from_slice(&data[..to_write]);
                    to_write
                }) {
                    Ok(bytes_written)
                } else {
                    Err(KError::NotFound)
                }
            }
            DeviceType::Root | DeviceType::InputDir => Err(KError::Invalid),
        }
    }

    fn truncate(&self, _size: usize) -> KResult<()> {
        // Dispositivos não podem ser truncados
        match self.device {
            DeviceType::Null | DeviceType::Zero | DeviceType::Urandom | DeviceType::Fb0 => Ok(()),
            _ => Err(KError::Invalid),
        }
    }

    fn size(&self) -> KResult<usize> {
        match self.device {
            DeviceType::Fb0 => {
                // Return framebuffer size
                Ok(crate::drivers::framebuffer::with_framebuffer(|fb| {
                    fb.buffer().len()
                }).unwrap_or(0))
            }
            // Other character devices have size 0
            _ => Ok(0),
        }
    }
}

/// Cria o inode raiz do devfs (/dev).
pub fn new_root() -> Inode {
    // Semeia o PRNG
    seed_urandom();

    let inode: Arc<DevfsInode> = Arc::new_cyclic(|weak| DevfsInode {
        device: DeviceType::Root,
        parent: RwLock::new(None),
        self_weak: weak.clone(),
    });
    Inode(inode)
}
