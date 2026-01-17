//! ext2 filesystem driver (read/write)
//!
//! Implementação enxuta focada em:

#![allow(dead_code)]
//! - Leitura e escrita de arquivos e diretórios
//! - Zero-copy onde possível
//! - Sem alocações desnecessárias
//!
//! Referências:
//! - https://www.nongnu.org/ext2-doc/ext2.html
//! - Linux kernel fs/ext2/

use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use core::mem::size_of;
use spin::RwLock;

use crate::security::{Gid, Uid};
use crate::storage::BlockDevice;
use crate::util::{KError, KResult};

use super::vfs::{DirEntry, Inode, InodeKind, InodeOps, Metadata, Mode};

// ============================================================================
// Constantes ext2
// ============================================================================

const EXT2_SUPER_MAGIC: u16 = 0xEF53;
const EXT2_SUPERBLOCK_OFFSET: u64 = 1024;
const EXT2_ROOT_INO: u32 = 2;

// Tipos de arquivo no inode (i_mode >> 12)
const EXT2_S_IFIFO: u16 = 0x1; // FIFO/named pipe
const EXT2_S_IFCHR: u16 = 0x2; // char device
const EXT2_S_IFDIR: u16 = 0x4; // directory
const EXT2_S_IFBLK: u16 = 0x6; // block device
const EXT2_S_IFREG: u16 = 0x8; // regular file
const EXT2_S_IFLNK: u16 = 0xA; // symlink
const EXT2_S_IFSOCK: u16 = 0xC; // socket

// Tipos de dirent (d_type)
const EXT2_FT_REG_FILE: u8 = 1;
const EXT2_FT_DIR: u8 = 2;
const EXT2_FT_CHRDEV: u8 = 3;
const EXT2_FT_BLKDEV: u8 = 4;
const EXT2_FT_FIFO: u8 = 5;
const EXT2_FT_SOCK: u8 = 6;
const EXT2_FT_SYMLINK: u8 = 7;

// ============================================================================
// Estruturas on-disk (packed, little-endian)
// ============================================================================

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
struct Ext2Superblock {
    s_inodes_count: u32,
    s_blocks_count: u32,
    s_r_blocks_count: u32,
    s_free_blocks_count: u32,
    s_free_inodes_count: u32,
    s_first_data_block: u32,
    s_log_block_size: u32,       // block_size = 1024 << s_log_block_size
    s_log_frag_size: u32,
    s_blocks_per_group: u32,
    s_frags_per_group: u32,
    s_inodes_per_group: u32,
    s_mtime: u32,
    s_wtime: u32,
    s_mnt_count: u16,
    s_max_mnt_count: u16,
    s_magic: u16,
    s_state: u16,
    s_errors: u16,
    s_minor_rev_level: u16,
    s_lastcheck: u32,
    s_checkinterval: u32,
    s_creator_os: u32,
    s_rev_level: u32,
    s_def_resuid: u16,
    s_def_resgid: u16,
    // Extended superblock fields (rev >= 1)
    s_first_ino: u32,
    s_inode_size: u16,
    s_block_group_nr: u16,
    s_feature_compat: u32,
    s_feature_incompat: u32,
    s_feature_ro_compat: u32,
    s_uuid: [u8; 16],
    s_volume_name: [u8; 16],
    // ... mais campos que não usamos
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
struct Ext2BlockGroupDesc {
    bg_block_bitmap: u32,
    bg_inode_bitmap: u32,
    bg_inode_table: u32,
    bg_free_blocks_count: u16,
    bg_free_inodes_count: u16,
    bg_used_dirs_count: u16,
    bg_pad: u16,
    bg_reserved: [u8; 12],
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
struct Ext2Inode {
    i_mode: u16,
    i_uid: u16,
    i_size: u32,
    i_atime: u32,
    i_ctime: u32,
    i_mtime: u32,
    i_dtime: u32,
    i_gid: u16,
    i_links_count: u16,
    i_blocks: u32,          // em blocos de 512 bytes
    i_flags: u32,
    i_osd1: u32,
    i_block: [u32; 15],     // ponteiros de bloco (12 diretos + 3 indiretos)
    i_generation: u32,
    i_file_acl: u32,
    i_dir_acl: u32,         // ou i_size_high em rev >= 1
    i_faddr: u32,
    i_osd2: [u8; 12],
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
struct Ext2DirEntry {
    inode: u32,
    rec_len: u16,
    name_len: u8,
    file_type: u8,
    // name: [u8; name_len] segue
}

// ============================================================================
// Estruturas do driver
// ============================================================================

/// Filesystem ext2 montado (read/write)
pub struct Ext2Fs {
    device: Arc<dyn BlockDevice>,
    block_size: u32,
    blocks_per_group: u32,
    inodes_per_group: u32,
    inode_size: u16,
    groups: RwLock<Vec<Ext2BlockGroupDesc>>,
    first_data_block: u32,
    total_blocks: u32,
    total_inodes: u32,
}

impl Ext2Fs {
    /// Monta um filesystem ext2 do dispositivo de bloco
    pub fn mount(device: Arc<dyn BlockDevice>) -> KResult<Arc<Self>> {
        // Lê o superblock (offset 1024)
        let sb_block = EXT2_SUPERBLOCK_OFFSET / device.block_size() as u64;
        let sb_offset = (EXT2_SUPERBLOCK_OFFSET % device.block_size() as u64) as usize;

        let mut buf = vec![0u8; device.block_size() as usize];
        device.read_blocks(sb_block, 1, &mut buf)?;

        let sb: Ext2Superblock = unsafe {
            core::ptr::read_unaligned(buf.as_ptr().add(sb_offset) as *const _)
        };

        // Verifica magic
        if sb.s_magic != EXT2_SUPER_MAGIC {
            return Err(KError::Invalid);
        }

        let block_size = 1024u32 << sb.s_log_block_size;
        let inode_size = if sb.s_rev_level >= 1 { sb.s_inode_size } else { 128 };

        // Calcula número de grupos
        let groups_count = (sb.s_blocks_count + sb.s_blocks_per_group - 1) / sb.s_blocks_per_group;

        // Lê a tabela de block group descriptors
        // Está no bloco logo após o superblock
        let bgdt_block = if block_size == 1024 { 2 } else { 1 };
        let bgdt_size = groups_count as usize * size_of::<Ext2BlockGroupDesc>();
        let bgdt_blocks = (bgdt_size + block_size as usize - 1) / block_size as usize;

        let mut bgdt_buf = vec![0u8; bgdt_blocks * block_size as usize];
        Self::read_blocks_raw(&device, block_size, bgdt_block as u64, bgdt_blocks as u32, &mut bgdt_buf)?;

        let mut groups = Vec::with_capacity(groups_count as usize);
        for i in 0..groups_count as usize {
            let desc: Ext2BlockGroupDesc = unsafe {
                core::ptr::read_unaligned(
                    bgdt_buf.as_ptr().add(i * size_of::<Ext2BlockGroupDesc>()) as *const _
                )
            };
            groups.push(desc);
        }

        Ok(Arc::new(Self {
            device,
            block_size,
            blocks_per_group: sb.s_blocks_per_group,
            inodes_per_group: sb.s_inodes_per_group,
            inode_size,
            groups: RwLock::new(groups),
            first_data_block: sb.s_first_data_block,
            total_blocks: sb.s_blocks_count,
            total_inodes: sb.s_inodes_count,
        }))
    }

    /// Lê blocos do device convertendo block_size
    fn read_blocks_raw(
        device: &Arc<dyn BlockDevice>,
        block_size: u32,
        block: u64,
        count: u32,
        buf: &mut [u8],
    ) -> KResult<()> {
        let dev_block_size = device.block_size();
        let factor = block_size / dev_block_size;
        let start_lba = block * factor as u64;
        let lba_count = count * factor;
        device.read_blocks(start_lba, lba_count, buf)
    }

    /// Lê um bloco do filesystem
    fn read_block(&self, block: u32, buf: &mut [u8]) -> KResult<()> {
        Self::read_blocks_raw(&self.device, self.block_size, block as u64, 1, buf)
    }

    /// Lê um inode pelo número
    fn read_inode(&self, ino: u32) -> KResult<Ext2Inode> {
        if ino == 0 {
            return Err(KError::Invalid);
        }

        let group = (ino - 1) / self.inodes_per_group;
        let index = (ino - 1) % self.inodes_per_group;

        let groups = self.groups.read();
        let inode_table_block = groups[group as usize].bg_inode_table;
        drop(groups);
        let inode_offset = index as usize * self.inode_size as usize;
        let block_offset = inode_offset / self.block_size as usize;
        let offset_in_block = inode_offset % self.block_size as usize;

        let mut buf = vec![0u8; self.block_size as usize];
        self.read_block(inode_table_block + block_offset as u32, &mut buf)?;

        let inode: Ext2Inode = unsafe {
            core::ptr::read_unaligned(buf.as_ptr().add(offset_in_block) as *const _)
        };

        Ok(inode)
    }

    /// Lê o conteúdo de um arquivo/diretório
    fn read_inode_data(&self, inode: &Ext2Inode, offset: usize, out: &mut [u8]) -> KResult<usize> {
        let size = inode.i_size as usize;
        if offset >= size {
            return Ok(0);
        }

        let to_read = core::cmp::min(out.len(), size - offset);
        let mut read = 0;
        let mut file_offset = offset;

        let mut block_buf = vec![0u8; self.block_size as usize];

        while read < to_read {
            let block_idx = file_offset / self.block_size as usize;
            let block_offset = file_offset % self.block_size as usize;
            let chunk = core::cmp::min(to_read - read, self.block_size as usize - block_offset);

            let block_num = self.get_block_num(inode, block_idx as u32)?;
            if block_num == 0 {
                // Sparse block - preenche com zeros
                out[read..read + chunk].fill(0);
            } else {
                self.read_block(block_num, &mut block_buf)?;
                out[read..read + chunk].copy_from_slice(&block_buf[block_offset..block_offset + chunk]);
            }

            read += chunk;
            file_offset += chunk;
        }

        Ok(read)
    }

    /// Obtém o número do bloco físico dado o índice lógico
    fn get_block_num(&self, inode: &Ext2Inode, block_idx: u32) -> KResult<u32> {
        let ptrs_per_block = self.block_size / 4;

        if block_idx < 12 {
            // Bloco direto
            Ok(inode.i_block[block_idx as usize])
        } else if block_idx < 12 + ptrs_per_block {
            // Indireto simples
            let idx = block_idx - 12;
            self.read_indirect(inode.i_block[12], idx)
        } else if block_idx < 12 + ptrs_per_block + ptrs_per_block * ptrs_per_block {
            // Indireto duplo
            let idx = block_idx - 12 - ptrs_per_block;
            let l1_idx = idx / ptrs_per_block;
            let l2_idx = idx % ptrs_per_block;
            let l1_block = self.read_indirect(inode.i_block[13], l1_idx)?;
            self.read_indirect(l1_block, l2_idx)
        } else {
            // Indireto triplo (raro, arquivos > 4GB com blocos de 4K)
            let idx = block_idx - 12 - ptrs_per_block - ptrs_per_block * ptrs_per_block;
            let l1_idx = idx / (ptrs_per_block * ptrs_per_block);
            let rem = idx % (ptrs_per_block * ptrs_per_block);
            let l2_idx = rem / ptrs_per_block;
            let l3_idx = rem % ptrs_per_block;
            let l1_block = self.read_indirect(inode.i_block[14], l1_idx)?;
            let l2_block = self.read_indirect(l1_block, l2_idx)?;
            self.read_indirect(l2_block, l3_idx)
        }
    }

    /// Lê um ponteiro de bloco indireto
    fn read_indirect(&self, block: u32, idx: u32) -> KResult<u32> {
        if block == 0 {
            return Ok(0);
        }

        let offset = idx as usize * 4;
        let block_in_buf = offset / self.block_size as usize;
        let offset_in_block = offset % self.block_size as usize;

        let mut buf = vec![0u8; self.block_size as usize];
        self.read_block(block + block_in_buf as u32, &mut buf)?;

        let ptr: u32 = unsafe {
            core::ptr::read_unaligned(buf.as_ptr().add(offset_in_block) as *const u32)
        };

        Ok(ptr)
    }

    // ========================================================================
    // Métodos de escrita
    // ========================================================================

    /// Escreve blocos no device convertendo block_size
    fn write_blocks_raw(
        device: &Arc<dyn BlockDevice>,
        block_size: u32,
        block: u64,
        count: u32,
        buf: &[u8],
    ) -> KResult<()> {
        let dev_block_size = device.block_size();
        let factor = block_size / dev_block_size;
        let start_lba = block * factor as u64;
        let lba_count = count * factor;
        device.write_blocks(start_lba, lba_count, buf)
    }

    /// Escreve um bloco do filesystem
    fn write_block(&self, block: u32, buf: &[u8]) -> KResult<()> {
        Self::write_blocks_raw(&self.device, self.block_size, block as u64, 1, buf)
    }

    /// Escreve um inode de volta ao disco
    fn write_inode(&self, ino: u32, inode: &Ext2Inode) -> KResult<()> {
        if ino == 0 {
            return Err(KError::Invalid);
        }

        let group = (ino - 1) / self.inodes_per_group;
        let index = (ino - 1) % self.inodes_per_group;

        let groups = self.groups.read();
        let inode_table_block = groups[group as usize].bg_inode_table;
        drop(groups);

        let inode_offset = index as usize * self.inode_size as usize;
        let block_offset = inode_offset / self.block_size as usize;
        let offset_in_block = inode_offset % self.block_size as usize;

        let mut buf = vec![0u8; self.block_size as usize];
        self.read_block(inode_table_block + block_offset as u32, &mut buf)?;

        unsafe {
            core::ptr::write_unaligned(
                buf.as_mut_ptr().add(offset_in_block) as *mut Ext2Inode,
                *inode,
            );
        }

        self.write_block(inode_table_block + block_offset as u32, &buf)
    }

    /// Aloca um bloco livre do bitmap
    fn alloc_block(&self) -> KResult<u32> {
        let mut groups = self.groups.write();

        for (group_idx, group) in groups.iter_mut().enumerate() {
            if group.bg_free_blocks_count == 0 {
                continue;
            }

            // Lê o bitmap de blocos
            let mut bitmap = vec![0u8; self.block_size as usize];
            Self::read_blocks_raw(&self.device, self.block_size, group.bg_block_bitmap as u64, 1, &mut bitmap)?;

            // Procura um bit livre
            for byte_idx in 0..self.block_size as usize {
                if bitmap[byte_idx] == 0xFF {
                    continue;
                }
                for bit_idx in 0..8 {
                    if bitmap[byte_idx] & (1 << bit_idx) == 0 {
                        // Encontrou bloco livre
                        bitmap[byte_idx] |= 1 << bit_idx;

                        // Escreve bitmap de volta
                        Self::write_blocks_raw(&self.device, self.block_size, group.bg_block_bitmap as u64, 1, &bitmap)?;

                        // Atualiza contador
                        group.bg_free_blocks_count -= 1;

                        // Calcula número do bloco
                        let block_num = self.first_data_block
                            + (group_idx as u32 * self.blocks_per_group)
                            + (byte_idx as u32 * 8)
                            + bit_idx as u32;

                        return Ok(block_num);
                    }
                }
            }
        }

        Err(KError::NoMemory)
    }

    /// Libera um bloco
    fn free_block(&self, block: u32) -> KResult<()> {
        if block < self.first_data_block || block >= self.total_blocks {
            return Err(KError::Invalid);
        }

        let relative_block = block - self.first_data_block;
        let group_idx = (relative_block / self.blocks_per_group) as usize;
        let block_in_group = relative_block % self.blocks_per_group;
        let byte_idx = (block_in_group / 8) as usize;
        let bit_idx = (block_in_group % 8) as usize;

        let mut groups = self.groups.write();
        let group = &mut groups[group_idx];

        // Lê bitmap
        let mut bitmap = vec![0u8; self.block_size as usize];
        Self::read_blocks_raw(&self.device, self.block_size, group.bg_block_bitmap as u64, 1, &mut bitmap)?;

        // Limpa bit
        bitmap[byte_idx] &= !(1 << bit_idx);

        // Escreve de volta
        Self::write_blocks_raw(&self.device, self.block_size, group.bg_block_bitmap as u64, 1, &bitmap)?;

        group.bg_free_blocks_count += 1;

        Ok(())
    }

    /// Aloca um inode livre
    fn alloc_inode(&self) -> KResult<u32> {
        let mut groups = self.groups.write();

        for (group_idx, group) in groups.iter_mut().enumerate() {
            if group.bg_free_inodes_count == 0 {
                continue;
            }

            // Lê o bitmap de inodes
            let mut bitmap = vec![0u8; self.block_size as usize];
            Self::read_blocks_raw(&self.device, self.block_size, group.bg_inode_bitmap as u64, 1, &mut bitmap)?;

            // Procura um bit livre
            for byte_idx in 0..(self.inodes_per_group as usize / 8) {
                if bitmap[byte_idx] == 0xFF {
                    continue;
                }
                for bit_idx in 0..8 {
                    if bitmap[byte_idx] & (1 << bit_idx) == 0 {
                        // Encontrou inode livre
                        bitmap[byte_idx] |= 1 << bit_idx;

                        // Escreve bitmap de volta
                        Self::write_blocks_raw(&self.device, self.block_size, group.bg_inode_bitmap as u64, 1, &bitmap)?;

                        // Atualiza contador
                        group.bg_free_inodes_count -= 1;

                        // Calcula número do inode (1-indexed)
                        let ino = (group_idx as u32 * self.inodes_per_group)
                            + (byte_idx as u32 * 8)
                            + bit_idx as u32
                            + 1;

                        return Ok(ino);
                    }
                }
            }
        }

        Err(KError::NoMemory)
    }

    /// Escreve dados em um inode, alocando blocos conforme necessário
    fn write_inode_data(&self, inode: &mut Ext2Inode, offset: usize, data: &[u8]) -> KResult<usize> {
        if data.is_empty() {
            return Ok(0);
        }

        let mut written = 0;
        let mut file_offset = offset;
        let mut block_buf = vec![0u8; self.block_size as usize];

        while written < data.len() {
            let block_idx = file_offset / self.block_size as usize;
            let block_offset = file_offset % self.block_size as usize;
            let chunk = core::cmp::min(data.len() - written, self.block_size as usize - block_offset);

            // Obtém ou aloca o bloco
            let mut block_num = self.get_block_num(inode, block_idx as u32)?;
            if block_num == 0 {
                // Precisa alocar novo bloco
                block_num = self.alloc_block()?;
                self.set_block_num(inode, block_idx as u32, block_num)?;
                // Zera o bloco novo
                block_buf.fill(0);
            } else if block_offset > 0 || chunk < self.block_size as usize {
                // Lê bloco existente para preservar dados
                self.read_block(block_num, &mut block_buf)?;
            }

            // Copia dados para o buffer
            block_buf[block_offset..block_offset + chunk].copy_from_slice(&data[written..written + chunk]);

            // Escreve o bloco
            self.write_block(block_num, &block_buf)?;

            written += chunk;
            file_offset += chunk;
        }

        // Atualiza tamanho se necessário
        let new_size = offset + written;
        if new_size > inode.i_size as usize {
            inode.i_size = new_size as u32;
        }

        // Atualiza i_blocks (em blocos de 512 bytes)
        let blocks_512 = ((inode.i_size as usize + self.block_size as usize - 1) / self.block_size as usize)
            * (self.block_size as usize / 512);
        inode.i_blocks = blocks_512 as u32;

        Ok(written)
    }

    /// Define o número do bloco físico para um índice lógico
    fn set_block_num(&self, inode: &mut Ext2Inode, block_idx: u32, block_num: u32) -> KResult<()> {
        let ptrs_per_block = self.block_size / 4;

        if block_idx < 12 {
            // Bloco direto
            inode.i_block[block_idx as usize] = block_num;
            Ok(())
        } else if block_idx < 12 + ptrs_per_block {
            // Indireto simples
            let idx = block_idx - 12;
            if inode.i_block[12] == 0 {
                inode.i_block[12] = self.alloc_block()?;
                // Zera o bloco indireto
                let zeros = vec![0u8; self.block_size as usize];
                self.write_block(inode.i_block[12], &zeros)?;
            }
            self.write_indirect(inode.i_block[12], idx, block_num)
        } else {
            // Indireto duplo e triplo - simplificado por enquanto
            Err(KError::NotSupported)
        }
    }

    /// Escreve um ponteiro em bloco indireto
    fn write_indirect(&self, indirect_block: u32, idx: u32, value: u32) -> KResult<()> {
        let offset = idx as usize * 4;
        let offset_in_block = offset % self.block_size as usize;

        let mut buf = vec![0u8; self.block_size as usize];
        self.read_block(indirect_block, &mut buf)?;

        unsafe {
            core::ptr::write_unaligned(
                buf.as_mut_ptr().add(offset_in_block) as *mut u32,
                value,
            );
        }

        self.write_block(indirect_block, &buf)
    }

    /// Adiciona uma entrada de diretório
    fn add_dir_entry(&self, dir_inode: &mut Ext2Inode, ino: u32, child_ino: u32, name: &str, file_type: u8) -> KResult<()> {
        let name_bytes = name.as_bytes();
        let entry_size = 8 + name_bytes.len();
        // Alinha para 4 bytes
        let aligned_size = (entry_size + 3) & !3;

        let dir_size = dir_inode.i_size as usize;
        let mut buf = vec![0u8; dir_size + self.block_size as usize];

        if dir_size > 0 {
            self.read_inode_data(dir_inode, 0, &mut buf[..dir_size])?;
        }

        // Procura espaço no diretório existente
        let mut offset = 0;
        while offset < dir_size {
            let entry: Ext2DirEntry = unsafe {
                core::ptr::read_unaligned(buf.as_ptr().add(offset) as *const _)
            };

            let actual_size = if entry.inode != 0 {
                8 + entry.name_len as usize
            } else {
                0
            };
            let actual_aligned = (actual_size + 3) & !3;
            let free_space = entry.rec_len as usize - actual_aligned;

            if free_space >= aligned_size {
                // Há espaço nesta entrada
                let new_rec_len = entry.rec_len - actual_aligned as u16;

                // Atualiza rec_len da entrada existente
                let updated_entry = Ext2DirEntry {
                    inode: entry.inode,
                    rec_len: actual_aligned as u16,
                    name_len: entry.name_len,
                    file_type: entry.file_type,
                };
                unsafe {
                    core::ptr::write_unaligned(
                        buf.as_mut_ptr().add(offset) as *mut Ext2DirEntry,
                        updated_entry,
                    );
                }

                // Adiciona nova entrada
                let new_offset = offset + actual_aligned;
                let new_entry = Ext2DirEntry {
                    inode: child_ino,
                    rec_len: new_rec_len,
                    name_len: name_bytes.len() as u8,
                    file_type,
                };
                unsafe {
                    core::ptr::write_unaligned(
                        buf.as_mut_ptr().add(new_offset) as *mut Ext2DirEntry,
                        new_entry,
                    );
                    core::ptr::copy_nonoverlapping(
                        name_bytes.as_ptr(),
                        buf.as_mut_ptr().add(new_offset + 8),
                        name_bytes.len(),
                    );
                }

                // Escreve de volta
                self.write_inode_data(dir_inode, 0, &buf[..dir_size])?;
                self.write_inode(ino, dir_inode)?;
                return Ok(());
            }

            offset += entry.rec_len as usize;
        }

        // Precisa de um novo bloco
        let new_entry = Ext2DirEntry {
            inode: child_ino,
            rec_len: self.block_size as u16,
            name_len: name_bytes.len() as u8,
            file_type,
        };

        let new_offset = dir_size;
        unsafe {
            core::ptr::write_unaligned(
                buf.as_mut_ptr().add(new_offset) as *mut Ext2DirEntry,
                new_entry,
            );
            core::ptr::copy_nonoverlapping(
                name_bytes.as_ptr(),
                buf.as_mut_ptr().add(new_offset + 8),
                name_bytes.len(),
            );
        }

        // Preenche resto do bloco com zeros
        buf[new_offset + 8 + name_bytes.len()..new_offset + self.block_size as usize].fill(0);

        // Escreve novo bloco
        let new_size = dir_size + self.block_size as usize;
        self.write_inode_data(dir_inode, 0, &buf[..new_size])?;
        self.write_inode(ino, dir_inode)?;

        Ok(())
    }

    /// Remove uma entrada de diretório
    fn remove_dir_entry(&self, dir_inode: &mut Ext2Inode, dir_ino: u32, name: &str) -> KResult<u32> {
        let dir_size = dir_inode.i_size as usize;

        if dir_size == 0 {
            return Err(KError::NotFound);
        }

        let mut buf = vec![0u8; dir_size];
        self.read_inode_data(dir_inode, 0, &mut buf)?;

        let mut offset = 0;
        let mut prev_offset: Option<usize> = None;

        while offset < dir_size {
            if offset + 8 > dir_size {
                break;
            }

            let entry: Ext2DirEntry = unsafe {
                core::ptr::read_unaligned(buf.as_ptr().add(offset) as *const _)
            };

            if entry.inode != 0 && entry.name_len > 0 {
                let name_start = offset + 8;
                let name_end = name_start + entry.name_len as usize;
                if name_end <= dir_size {
                    let entry_name = core::str::from_utf8(&buf[name_start..name_end])
                        .unwrap_or("");

                    if entry_name == name {
                        let removed_ino = entry.inode;

                        if let Some(prev) = prev_offset {
                            // Mescla com entrada anterior
                            let prev_entry: Ext2DirEntry = unsafe {
                                core::ptr::read_unaligned(buf.as_ptr().add(prev) as *const _)
                            };
                            let new_rec_len = prev_entry.rec_len + entry.rec_len;
                            let updated = Ext2DirEntry {
                                rec_len: new_rec_len,
                                ..prev_entry
                            };
                            unsafe {
                                core::ptr::write_unaligned(
                                    buf.as_mut_ptr().add(prev) as *mut Ext2DirEntry,
                                    updated,
                                );
                            }
                        } else {
                            // É a primeira entrada - marca como livre
                            let updated = Ext2DirEntry {
                                inode: 0,
                                rec_len: entry.rec_len,
                                name_len: 0,
                                file_type: 0,
                            };
                            unsafe {
                                core::ptr::write_unaligned(
                                    buf.as_mut_ptr().add(offset) as *mut Ext2DirEntry,
                                    updated,
                                );
                            }
                        }

                        // Escreve buffer de volta
                        self.write_inode_data(dir_inode, 0, &buf)?;
                        self.write_inode(dir_ino, dir_inode)?;

                        return Ok(removed_ino);
                    }
                }
            }

            if entry.rec_len == 0 {
                break;
            }

            if entry.inode != 0 {
                prev_offset = Some(offset);
            }
            offset += entry.rec_len as usize;
        }

        Err(KError::NotFound)
    }

    /// Libera um inode (marca como livre no bitmap)
    fn free_inode(&self, ino: u32) -> KResult<()> {
        if ino == 0 || ino > self.total_inodes {
            return Err(KError::Invalid);
        }

        let group_idx = ((ino - 1) / self.inodes_per_group) as usize;
        let inode_in_group = (ino - 1) % self.inodes_per_group;
        let byte_idx = (inode_in_group / 8) as usize;
        let bit_idx = (inode_in_group % 8) as usize;

        let mut groups = self.groups.write();
        let group = &mut groups[group_idx];

        // Lê bitmap
        let mut bitmap = vec![0u8; self.block_size as usize];
        Self::read_blocks_raw(&self.device, self.block_size, group.bg_inode_bitmap as u64, 1, &mut bitmap)?;

        // Limpa bit
        bitmap[byte_idx] &= !(1 << bit_idx);

        // Escreve de volta
        Self::write_blocks_raw(&self.device, self.block_size, group.bg_inode_bitmap as u64, 1, &bitmap)?;

        group.bg_free_inodes_count += 1;

        Ok(())
    }

    /// Libera todos os blocos de dados de um inode
    fn free_inode_blocks(&self, inode: &Ext2Inode) -> KResult<()> {
        let block_size = self.block_size as usize;
        let num_blocks = (inode.i_size as usize + block_size - 1) / block_size;

        // Libera blocos diretos
        for i in 0..core::cmp::min(num_blocks, 12) {
            if inode.i_block[i] != 0 {
                self.free_block(inode.i_block[i])?;
            }
        }

        // Libera bloco indireto simples e seus blocos
        if inode.i_block[12] != 0 {
            self.free_indirect_block(inode.i_block[12], 1)?;
        }

        // Libera bloco indireto duplo
        if inode.i_block[13] != 0 {
            self.free_indirect_block(inode.i_block[13], 2)?;
        }

        // Libera bloco indireto triplo
        if inode.i_block[14] != 0 {
            self.free_indirect_block(inode.i_block[14], 3)?;
        }

        Ok(())
    }

    /// Libera um bloco indireto e seus filhos recursivamente
    fn free_indirect_block(&self, block: u32, level: u32) -> KResult<()> {
        if block == 0 {
            return Ok(());
        }

        if level > 1 {
            // Lê os ponteiros e libera recursivamente
            let mut buf = vec![0u8; self.block_size as usize];
            self.read_block(block, &mut buf)?;

            let ptrs_per_block = self.block_size / 4;
            for i in 0..ptrs_per_block {
                let ptr: u32 = unsafe {
                    core::ptr::read_unaligned(buf.as_ptr().add(i as usize * 4) as *const u32)
                };
                if ptr != 0 {
                    self.free_indirect_block(ptr, level - 1)?;
                }
            }
        }

        // Libera o próprio bloco
        self.free_block(block)
    }

    /// Retorna o inode root
    pub fn root(self: &Arc<Self>) -> Inode {
        Inode(Arc::new(Ext2Inode2 {
            fs: Arc::clone(self),
            ino: EXT2_ROOT_INO,
            raw: RwLock::new(None),
            parent: None,
        }))
    }
}

// ============================================================================
// Implementação do trait InodeOps
// ============================================================================

struct Ext2Inode2 {
    fs: Arc<Ext2Fs>,
    ino: u32,
    raw: RwLock<Option<Ext2Inode>>,
    parent: Option<Inode>,
}

impl Ext2Inode2 {
    fn get_raw(&self) -> KResult<Ext2Inode> {
        {
            let guard = self.raw.read();
            if let Some(raw) = *guard {
                return Ok(raw);
            }
        }

        let raw = self.fs.read_inode(self.ino)?;
        *self.raw.write() = Some(raw);
        Ok(raw)
    }

    fn inode_kind(mode: u16) -> InodeKind {
        match (mode >> 12) & 0xF {
            EXT2_S_IFREG => InodeKind::File,
            EXT2_S_IFDIR => InodeKind::Dir,
            EXT2_S_IFLNK => InodeKind::Symlink,
            EXT2_S_IFCHR => InodeKind::CharDev,
            EXT2_S_IFBLK => InodeKind::BlockDev,
            EXT2_S_IFIFO => InodeKind::Fifo,
            EXT2_S_IFSOCK => InodeKind::Socket,
            _ => InodeKind::File,
        }
    }

    fn file_type_to_kind(ft: u8) -> InodeKind {
        match ft {
            EXT2_FT_REG_FILE => InodeKind::File,
            EXT2_FT_DIR => InodeKind::Dir,
            EXT2_FT_SYMLINK => InodeKind::Symlink,
            EXT2_FT_CHRDEV => InodeKind::CharDev,
            EXT2_FT_BLKDEV => InodeKind::BlockDev,
            EXT2_FT_FIFO => InodeKind::Fifo,
            EXT2_FT_SOCK => InodeKind::Socket,
            _ => InodeKind::File,
        }
    }
}

impl InodeOps for Ext2Inode2 {
    fn metadata(&self) -> Metadata {
        let raw = self.get_raw().unwrap_or(Ext2Inode {
            i_mode: 0,
            i_uid: 0,
            i_size: 0,
            i_atime: 0,
            i_ctime: 0,
            i_mtime: 0,
            i_dtime: 0,
            i_gid: 0,
            i_links_count: 0,
            i_blocks: 0,
            i_flags: 0,
            i_osd1: 0,
            i_block: [0; 15],
            i_generation: 0,
            i_file_acl: 0,
            i_dir_acl: 0,
            i_faddr: 0,
            i_osd2: [0; 12],
        });

        Metadata::simple(
            Uid(raw.i_uid as u32),
            Gid(raw.i_gid as u32),
            Mode::from_octal(raw.i_mode & 0o777),
            Self::inode_kind(raw.i_mode),
        )
    }

    fn set_metadata(&self, meta: Metadata) {
        if let Ok(mut raw) = self.get_raw() {
            // Preserva o tipo de arquivo (bits altos do mode)
            let file_type = raw.i_mode & 0xF000;
            raw.i_mode = file_type | meta.mode.to_octal();
            raw.i_uid = meta.uid.0 as u16;
            raw.i_gid = meta.gid.0 as u16;

            // Escreve de volta ao disco
            let _ = self.fs.write_inode(self.ino, &raw);
            *self.raw.write() = Some(raw);
        }
    }

    fn parent(&self) -> Option<Inode> {
        self.parent.clone()
    }

    fn lookup(&self, name: &str) -> KResult<Inode> {
        let raw = self.get_raw()?;
        if Self::inode_kind(raw.i_mode) != InodeKind::Dir {
            return Err(KError::NotFound);
        }

        let size = raw.i_size as usize;
        let mut buf = vec![0u8; size];
        self.fs.read_inode_data(&raw, 0, &mut buf)?;

        let mut offset = 0;
        while offset < size {
            if offset + 8 > size {
                break;
            }

            let entry: Ext2DirEntry = unsafe {
                core::ptr::read_unaligned(buf.as_ptr().add(offset) as *const _)
            };

            if entry.inode != 0 && entry.name_len > 0 {
                let name_start = offset + 8;
                let name_end = name_start + entry.name_len as usize;
                if name_end <= size {
                    let entry_name = core::str::from_utf8(&buf[name_start..name_end])
                        .unwrap_or("");

                    if entry_name == name {
                        let child = Arc::new(Ext2Inode2 {
                            fs: Arc::clone(&self.fs),
                            ino: entry.inode,
                            raw: RwLock::new(None),
                            parent: Some(Inode(Arc::new(Ext2Inode2 {
                                fs: Arc::clone(&self.fs),
                                ino: self.ino,
                                raw: RwLock::new(Some(raw)),
                                parent: self.parent.clone(),
                            }))),
                        });
                        return Ok(Inode(child));
                    }
                }
            }

            if entry.rec_len == 0 {
                break;
            }
            offset += entry.rec_len as usize;
        }

        Err(KError::NotFound)
    }

    fn create(&self, name: &str, kind: InodeKind, meta: Metadata) -> KResult<Inode> {
        let raw = self.get_raw()?;
        if Self::inode_kind(raw.i_mode) != InodeKind::Dir {
            return Err(KError::Invalid);
        }

        // Aloca novo inode
        let new_ino = self.fs.alloc_inode()?;

        // Determina o tipo de arquivo
        let (mode_type, file_type) = match kind {
            InodeKind::File => (EXT2_S_IFREG, EXT2_FT_REG_FILE),
            InodeKind::Dir => (EXT2_S_IFDIR, EXT2_FT_DIR),
            InodeKind::Symlink => (EXT2_S_IFLNK, EXT2_FT_SYMLINK),
            InodeKind::CharDev => (EXT2_S_IFCHR, EXT2_FT_CHRDEV),
            InodeKind::BlockDev => (EXT2_S_IFBLK, EXT2_FT_BLKDEV),
            InodeKind::Fifo => (EXT2_S_IFIFO, EXT2_FT_FIFO),
            InodeKind::Socket => (EXT2_S_IFSOCK, EXT2_FT_SOCK),
        };

        // Cria novo inode
        let mut new_raw = Ext2Inode {
            i_mode: (mode_type << 12) | meta.mode.to_octal(),
            i_uid: meta.uid.0 as u16,
            i_size: 0,
            i_atime: 0,
            i_ctime: 0,
            i_mtime: 0,
            i_dtime: 0,
            i_gid: meta.gid.0 as u16,
            i_links_count: 1,
            i_blocks: 0,
            i_flags: 0,
            i_osd1: 0,
            i_block: [0; 15],
            i_generation: 0,
            i_file_acl: 0,
            i_dir_acl: 0,
            i_faddr: 0,
            i_osd2: [0; 12],
        };

        // Se é diretório, cria . e ..
        if kind == InodeKind::Dir {
            // Aloca bloco para o diretório
            let dir_block = self.fs.alloc_block()?;
            new_raw.i_block[0] = dir_block;
            new_raw.i_size = self.fs.block_size;
            new_raw.i_blocks = (self.fs.block_size / 512) as u32;
            new_raw.i_links_count = 2; // . e entrada do pai

            // Cria entradas . e ..
            let mut dir_buf = vec![0u8; self.fs.block_size as usize];

            // Entrada .
            let dot = Ext2DirEntry {
                inode: new_ino,
                rec_len: 12,
                name_len: 1,
                file_type: EXT2_FT_DIR,
            };
            unsafe {
                core::ptr::write_unaligned(dir_buf.as_mut_ptr() as *mut Ext2DirEntry, dot);
                dir_buf[8] = b'.';
            }

            // Entrada ..
            let dotdot = Ext2DirEntry {
                inode: self.ino,
                rec_len: self.fs.block_size as u16 - 12,
                name_len: 2,
                file_type: EXT2_FT_DIR,
            };
            unsafe {
                core::ptr::write_unaligned(dir_buf.as_mut_ptr().add(12) as *mut Ext2DirEntry, dotdot);
                dir_buf[20] = b'.';
                dir_buf[21] = b'.';
            }

            self.fs.write_block(dir_block, &dir_buf)?;
        }

        // Escreve novo inode
        self.fs.write_inode(new_ino, &new_raw)?;

        // Adiciona entrada no diretório pai
        let mut dir_raw = raw;
        self.fs.add_dir_entry(&mut dir_raw, self.ino, new_ino, name, file_type)?;

        // Atualiza cache
        *self.raw.write() = Some(dir_raw);

        // Retorna novo inode
        Ok(Inode(Arc::new(Ext2Inode2 {
            fs: Arc::clone(&self.fs),
            ino: new_ino,
            raw: RwLock::new(Some(new_raw)),
            parent: Some(Inode(Arc::new(Ext2Inode2 {
                fs: Arc::clone(&self.fs),
                ino: self.ino,
                raw: RwLock::new(Some(dir_raw)),
                parent: self.parent.clone(),
            }))),
        })))
    }

    fn readdir(&self) -> KResult<Vec<DirEntry>> {
        // Read fresh from disk (not cache) to see latest entries
        let raw = self.fs.read_inode(self.ino)?;
        if Self::inode_kind(raw.i_mode) != InodeKind::Dir {
            return Err(KError::Invalid);
        }

        let size = raw.i_size as usize;
        let mut buf = vec![0u8; size];
        self.fs.read_inode_data(&raw, 0, &mut buf)?;

        let mut entries = Vec::new();
        let mut offset = 0;

        while offset < size {
            if offset + 8 > size {
                break;
            }

            let entry: Ext2DirEntry = unsafe {
                core::ptr::read_unaligned(buf.as_ptr().add(offset) as *const _)
            };

            if entry.inode != 0 && entry.name_len > 0 {
                let name_start = offset + 8;
                let name_end = name_start + entry.name_len as usize;
                if name_end <= size {
                    let name = core::str::from_utf8(&buf[name_start..name_end])
                        .unwrap_or("")
                        .to_string();

                    // Ignora . e ..
                    if name != "." && name != ".." {
                        entries.push(DirEntry {
                            name,
                            kind: Self::file_type_to_kind(entry.file_type),
                        });
                    }
                }
            }

            if entry.rec_len == 0 {
                break;
            }
            offset += entry.rec_len as usize;
        }

        Ok(entries)
    }

    fn read_at(&self, offset: usize, out: &mut [u8]) -> KResult<usize> {
        let raw = self.get_raw()?;
        self.fs.read_inode_data(&raw, offset, out)
    }

    fn write_at(&self, offset: usize, data: &[u8]) -> KResult<usize> {
        let mut raw = self.get_raw()?;
        if Self::inode_kind(raw.i_mode) != InodeKind::File {
            return Err(KError::Invalid);
        }

        let written = self.fs.write_inode_data(&mut raw, offset, data)?;

        // Escreve inode atualizado
        self.fs.write_inode(self.ino, &raw)?;

        // Atualiza cache
        *self.raw.write() = Some(raw);

        Ok(written)
    }

    fn truncate(&self, size: usize) -> KResult<()> {
        let mut raw = self.get_raw()?;
        if Self::inode_kind(raw.i_mode) != InodeKind::File {
            return Err(KError::Invalid);
        }

        let old_size = raw.i_size as usize;
        let block_size = self.fs.block_size as usize;

        if size < old_size {
            // Libera blocos além do novo tamanho
            let old_blocks = (old_size + block_size - 1) / block_size;
            let new_blocks = (size + block_size - 1) / block_size;

            for block_idx in new_blocks..old_blocks {
                let block_num = self.fs.get_block_num(&raw, block_idx as u32)?;
                if block_num != 0 {
                    self.fs.free_block(block_num)?;
                    // Não precisamos limpar os ponteiros diretos aqui,
                    // apenas para simplificar
                }
            }
        }

        raw.i_size = size as u32;
        raw.i_blocks = ((size + block_size - 1) / block_size * (block_size / 512)) as u32;

        self.fs.write_inode(self.ino, &raw)?;
        *self.raw.write() = Some(raw);

        Ok(())
    }

    fn size(&self) -> KResult<usize> {
        let raw = self.get_raw()?;
        Ok(raw.i_size as usize)
    }

    fn unlink(&self, name: &str) -> KResult<()> {
        let mut raw = self.get_raw()?;
        if Self::inode_kind(raw.i_mode) != InodeKind::Dir {
            return Err(KError::Invalid);
        }

        // Remove entrada do diretório e obtém o inode do arquivo
        let child_ino = self.fs.remove_dir_entry(&mut raw, self.ino, name)?;

        // Lê o inode do arquivo
        let child_raw = self.fs.read_inode(child_ino)?;

        // Verifica se é um arquivo (não diretório)
        if Self::inode_kind(child_raw.i_mode) == InodeKind::Dir {
            return Err(KError::Invalid);
        }

        // Decrementa link count
        let new_links = child_raw.i_links_count.saturating_sub(1);
        if new_links == 0 {
            // Libera blocos de dados
            self.fs.free_inode_blocks(&child_raw)?;
            // Libera o inode
            self.fs.free_inode(child_ino)?;
        } else {
            // Atualiza link count
            let mut updated = child_raw;
            updated.i_links_count = new_links;
            self.fs.write_inode(child_ino, &updated)?;
        }

        // Atualiza cache
        *self.raw.write() = Some(raw);

        Ok(())
    }

    fn rmdir(&self, name: &str) -> KResult<()> {
        let mut raw = self.get_raw()?;
        if Self::inode_kind(raw.i_mode) != InodeKind::Dir {
            return Err(KError::Invalid);
        }

        // Remove entrada do diretório e obtém o inode do subdiretório
        let child_ino = self.fs.remove_dir_entry(&mut raw, self.ino, name)?;

        // Lê o inode do diretório a ser removido
        let child_raw = self.fs.read_inode(child_ino)?;

        // Verifica se é um diretório
        if Self::inode_kind(child_raw.i_mode) != InodeKind::Dir {
            return Err(KError::Invalid);
        }

        // Libera blocos de dados (. e ..)
        self.fs.free_inode_blocks(&child_raw)?;

        // Libera o inode
        self.fs.free_inode(child_ino)?;

        // Decrementa link count do diretório pai (por causa do ..)
        raw.i_links_count = raw.i_links_count.saturating_sub(1);
        self.fs.write_inode(self.ino, &raw)?;

        // Atualiza cache
        *self.raw.write() = Some(raw);

        Ok(())
    }
}

// ============================================================================
// Helpers
// ============================================================================

use alloc::string::ToString;
