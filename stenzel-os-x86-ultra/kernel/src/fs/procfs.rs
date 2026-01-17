//! procfs - Virtual filesystem para informações de processos e sistema.
//!
//! Expõe informações do kernel e processos em /proc.

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

/// Tipo de conteúdo dinâmico do procfs.
#[derive(Clone)]
enum ProcContent {
    /// Diretório raiz /proc
    Root,
    /// Diretório de processo /proc/[pid]
    ProcessDir { pid: u64 },
    /// Arquivo /proc/[pid]/status
    ProcessStatus { pid: u64 },
    /// Arquivo /proc/[pid]/cmdline
    ProcessCmdline { pid: u64 },
    /// Arquivo /proc/meminfo
    Meminfo,
    /// Arquivo /proc/uptime
    Uptime,
    /// Arquivo /proc/version
    Version,
    /// Arquivo /proc/cpuinfo
    Cpuinfo,
    /// Arquivo /proc/pagecache
    PageCache,
}

/// Inode do procfs.
pub struct ProcfsInode {
    content: ProcContent,
    parent: RwLock<Option<Weak<ProcfsInode>>>,
    self_weak: Weak<ProcfsInode>,
}

impl ProcfsInode {
    fn metadata_for_content(content: &ProcContent) -> Metadata {
        match content {
            ProcContent::Root | ProcContent::ProcessDir { .. } => Metadata::simple(
                Uid(0),
                Gid(0),
                Mode::from_octal(0o555),
                InodeKind::Dir,
            ),
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
            ProcContent::Meminfo => generate_meminfo(),
            ProcContent::Uptime => generate_uptime(),
            ProcContent::Version => generate_version(),
            ProcContent::Cpuinfo => generate_cpuinfo(),
            ProcContent::PageCache => generate_pagecache(),
            ProcContent::ProcessStatus { pid } => generate_process_status(*pid),
            ProcContent::ProcessCmdline { pid } => generate_process_cmdline(*pid),
            _ => Vec::new(),
        }
    }

    /// Lista entradas de diretório.
    fn list_entries(&self) -> Vec<DirEntry> {
        match &self.content {
            ProcContent::Root => {
                let mut entries = vec![
                    DirEntry { name: "meminfo".to_string(), kind: InodeKind::File },
                    DirEntry { name: "uptime".to_string(), kind: InodeKind::File },
                    DirEntry { name: "version".to_string(), kind: InodeKind::File },
                    DirEntry { name: "cpuinfo".to_string(), kind: InodeKind::File },
                    DirEntry { name: "pagecache".to_string(), kind: InodeKind::File },
                ];

                // Adiciona diretórios de processos
                for pid in crate::sched::list_pids() {
                    entries.push(DirEntry {
                        name: format!("{}", pid),
                        kind: InodeKind::Dir,
                    });
                }

                entries
            }
            ProcContent::ProcessDir { .. } => {
                vec![
                    DirEntry { name: "status".to_string(), kind: InodeKind::File },
                    DirEntry { name: "cmdline".to_string(), kind: InodeKind::File },
                ]
            }
            _ => Vec::new(),
        }
    }

    /// Faz lookup de uma entrada no diretório.
    fn lookup_entry(&self, name: &str) -> KResult<ProcContent> {
        match &self.content {
            ProcContent::Root => {
                match name {
                    "meminfo" => Ok(ProcContent::Meminfo),
                    "uptime" => Ok(ProcContent::Uptime),
                    "version" => Ok(ProcContent::Version),
                    "cpuinfo" => Ok(ProcContent::Cpuinfo),
                    "pagecache" => Ok(ProcContent::PageCache),
                    _ => {
                        // Tenta parsear como PID
                        if let Ok(pid) = name.parse::<u64>() {
                            if crate::sched::task_exists(pid) {
                                return Ok(ProcContent::ProcessDir { pid });
                            }
                        }
                        Err(KError::NotFound)
                    }
                }
            }
            ProcContent::ProcessDir { pid } => {
                match name {
                    "status" => Ok(ProcContent::ProcessStatus { pid: *pid }),
                    "cmdline" => Ok(ProcContent::ProcessCmdline { pid: *pid }),
                    _ => Err(KError::NotFound),
                }
            }
            _ => Err(KError::Invalid),
        }
    }
}

impl InodeOps for ProcfsInode {
    fn metadata(&self) -> Metadata {
        Self::metadata_for_content(&self.content)
    }

    fn set_metadata(&self, _meta: Metadata) {
        // procfs é read-only
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

        let inode: Arc<ProcfsInode> = Arc::new_cyclic(|weak| ProcfsInode {
            content,
            parent: RwLock::new(Some(parent_weak)),
            self_weak: weak.clone(),
        });

        Ok(Inode(inode))
    }

    fn create(&self, _name: &str, _kind: InodeKind, _meta: Metadata) -> KResult<Inode> {
        // procfs é read-only
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
        // procfs é read-only
        Err(KError::NotSupported)
    }

    fn truncate(&self, _size: usize) -> KResult<()> {
        Err(KError::NotSupported)
    }

    fn size(&self) -> KResult<usize> {
        Ok(self.generate_content().len())
    }
}

/// Cria o inode raiz do procfs (/proc).
pub fn new_root() -> Inode {
    let inode: Arc<ProcfsInode> = Arc::new_cyclic(|weak| ProcfsInode {
        content: ProcContent::Root,
        parent: RwLock::new(None),
        self_weak: weak.clone(),
    });
    Inode(inode)
}

// ==================== Geradores de conteúdo ====================

fn generate_meminfo() -> Vec<u8> {
    let (total, free, used) = crate::mm::memory_stats();

    format!(
        "MemTotal:       {} kB\n\
         MemFree:        {} kB\n\
         MemUsed:        {} kB\n",
        total * 4, // frames para kB (4KB por frame)
        free * 4,
        used * 4,
    ).into_bytes()
}

fn generate_uptime() -> Vec<u8> {
    let ticks = crate::time::ticks();
    let hz = crate::time::hz();
    let seconds = if hz > 0 { ticks / hz } else { 0 };
    let frac = if hz > 0 { (ticks % hz) * 100 / hz } else { 0 };

    format!("{}.{:02} 0.00\n", seconds, frac).into_bytes()
}

fn generate_version() -> Vec<u8> {
    format!(
        "Stenzel OS version {} (rustc) #1\n",
        env!("CARGO_PKG_VERSION")
    ).into_bytes()
}

fn generate_cpuinfo() -> Vec<u8> {
    format!(
        "processor\t: 0\n\
         vendor_id\t: GenuineIntel\n\
         model name\t: x86_64 CPU\n\
         cpu MHz\t\t: 0.000\n\
         bogomips\t: 0.00\n"
    ).into_bytes()
}

fn generate_process_status(pid: u64) -> Vec<u8> {
    if let Some(info) = crate::sched::get_task_info(pid) {
        format!(
            "Name:\t{}\n\
             State:\t{}\n\
             Pid:\t{}\n\
             PPid:\t{}\n\
             Uid:\t{}\n\
             Gid:\t{}\n",
            info.name,
            info.state,
            info.pid,
            info.ppid,
            info.uid,
            info.gid,
        ).into_bytes()
    } else {
        Vec::new()
    }
}

fn generate_process_cmdline(pid: u64) -> Vec<u8> {
    if let Some(info) = crate::sched::get_task_info(pid) {
        let mut cmdline = info.name.into_bytes();
        cmdline.push(0); // null-terminated
        cmdline
    } else {
        Vec::new()
    }
}

fn generate_pagecache() -> Vec<u8> {
    let cache = super::page_cache::cache();
    let (hits, misses, evictions, writebacks) = cache.stats();
    let cached = cache.cached_pages();
    let dirty = cache.dirty_pages();

    let total = hits + misses;
    let hit_rate = if total > 0 {
        (hits * 100) / total
    } else {
        0
    };

    format!(
        "CachedPages:    {}\n\
         DirtyPages:     {}\n\
         Hits:           {}\n\
         Misses:         {}\n\
         HitRate:        {}%\n\
         Evictions:      {}\n\
         Writebacks:     {}\n",
        cached,
        dirty,
        hits,
        misses,
        hit_rate,
        evictions,
        writebacks,
    ).into_bytes()
}
