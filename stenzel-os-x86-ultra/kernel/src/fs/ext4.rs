//! ext4 filesystem driver (read-only)
//!
//! Key differences from ext2:
//! - Extents (vs block pointers) - more efficient for large files
//! - 64-bit block addressing - larger filesystems
//! - Journaling - data integrity (we ignore the journal for now)
//! - Directory indexes (htree) - faster directory lookup
//! - Larger inodes (256 bytes default vs 128)
//!
//! References:
//! - https://ext4.wiki.kernel.org/index.php/Ext4_Disk_Layout
//! - Linux kernel fs/ext4/

#![allow(dead_code)]

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
// Constants
// ============================================================================

const EXT4_SUPER_MAGIC: u16 = 0xEF53;
const EXT4_SUPERBLOCK_OFFSET: u64 = 1024;
const EXT4_ROOT_INO: u32 = 2;

// Feature flags
const EXT4_FEATURE_INCOMPAT_EXTENTS: u32 = 0x0040;
const EXT4_FEATURE_INCOMPAT_64BIT: u32 = 0x0080;
const EXT4_FEATURE_INCOMPAT_FLEX_BG: u32 = 0x0200;

// Inode flags
const EXT4_EXTENTS_FL: u32 = 0x00080000;

// File types in inode (i_mode >> 12)
const EXT4_S_IFREG: u16 = 0x8; // regular file
const EXT4_S_IFDIR: u16 = 0x4; // directory
const EXT4_S_IFLNK: u16 = 0xA; // symlink
const EXT4_S_IFCHR: u16 = 0x2; // char device
const EXT4_S_IFBLK: u16 = 0x6; // block device

// Directory entry types (d_type)
const EXT4_FT_REG_FILE: u8 = 1;
const EXT4_FT_DIR: u8 = 2;
const EXT4_FT_CHRDEV: u8 = 3;
const EXT4_FT_BLKDEV: u8 = 4;
const EXT4_FT_SYMLINK: u8 = 7;

// Extent header magic
const EXT4_EXT_MAGIC: u16 = 0xF30A;

// ============================================================================
// On-disk structures (packed, little-endian)
// ============================================================================

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
struct Ext4Superblock {
    s_inodes_count: u32,
    s_blocks_count_lo: u32,
    s_r_blocks_count_lo: u32,
    s_free_blocks_count_lo: u32,
    s_free_inodes_count: u32,
    s_first_data_block: u32,
    s_log_block_size: u32,
    s_log_cluster_size: u32,
    s_blocks_per_group: u32,
    s_clusters_per_group: u32,
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
    // ext4 specific fields
    s_first_ino: u32,
    s_inode_size: u16,
    s_block_group_nr: u16,
    s_feature_compat: u32,
    s_feature_incompat: u32,
    s_feature_ro_compat: u32,
    s_uuid: [u8; 16],
    s_volume_name: [u8; 16],
    s_last_mounted: [u8; 64],
    s_algorithm_usage_bitmap: u32,
    // Performance hints
    s_prealloc_blocks: u8,
    s_prealloc_dir_blocks: u8,
    s_reserved_gdt_blocks: u16,
    // Journaling support
    s_journal_uuid: [u8; 16],
    s_journal_inum: u32,
    s_journal_dev: u32,
    s_last_orphan: u32,
    s_hash_seed: [u32; 4],
    s_def_hash_version: u8,
    s_jnl_backup_type: u8,
    s_desc_size: u16,
    s_default_mount_opts: u32,
    s_first_meta_bg: u32,
    s_mkfs_time: u32,
    s_jnl_blocks: [u32; 17],
    // 64-bit support
    s_blocks_count_hi: u32,
    s_r_blocks_count_hi: u32,
    s_free_blocks_count_hi: u32,
    s_min_extra_isize: u16,
    s_want_extra_isize: u16,
    s_flags: u32,
    s_raid_stride: u16,
    s_mmp_interval: u16,
    s_mmp_block: u64,
    s_raid_stripe_width: u32,
    s_log_groups_per_flex: u8,
    s_checksum_type: u8,
    s_reserved_pad: u16,
    // ... more fields we don't need
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
struct Ext4BlockGroupDesc {
    bg_block_bitmap_lo: u32,
    bg_inode_bitmap_lo: u32,
    bg_inode_table_lo: u32,
    bg_free_blocks_count_lo: u16,
    bg_free_inodes_count_lo: u16,
    bg_used_dirs_count_lo: u16,
    bg_flags: u16,
    bg_exclude_bitmap_lo: u32,
    bg_block_bitmap_csum_lo: u16,
    bg_inode_bitmap_csum_lo: u16,
    bg_itable_unused_lo: u16,
    bg_checksum: u16,
    // 64-bit fields
    bg_block_bitmap_hi: u32,
    bg_inode_bitmap_hi: u32,
    bg_inode_table_hi: u32,
    bg_free_blocks_count_hi: u16,
    bg_free_inodes_count_hi: u16,
    bg_used_dirs_count_hi: u16,
    bg_itable_unused_hi: u16,
    bg_exclude_bitmap_hi: u32,
    bg_block_bitmap_csum_hi: u16,
    bg_inode_bitmap_csum_hi: u16,
    bg_reserved: u32,
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
struct Ext4Inode {
    i_mode: u16,
    i_uid: u16,
    i_size_lo: u32,
    i_atime: u32,
    i_ctime: u32,
    i_mtime: u32,
    i_dtime: u32,
    i_gid: u16,
    i_links_count: u16,
    i_blocks_lo: u32,
    i_flags: u32,
    i_osd1: u32,
    i_block: [u32; 15], // Extent tree or direct block pointers
    i_generation: u32,
    i_file_acl_lo: u32,
    i_size_high: u32,
    i_obso_faddr: u32,
    i_osd2: [u8; 12],
    i_extra_isize: u16,
    i_checksum_hi: u16,
    i_ctime_extra: u32,
    i_mtime_extra: u32,
    i_atime_extra: u32,
    i_crtime: u32,
    i_crtime_extra: u32,
    i_version_hi: u32,
    i_projid: u32,
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
struct Ext4DirEntry {
    inode: u32,
    rec_len: u16,
    name_len: u8,
    file_type: u8,
    // name: [u8; name_len] follows
}

/// Extent header
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
struct Ext4ExtentHeader {
    eh_magic: u16,     // Magic number (0xF30A)
    eh_entries: u16,   // Number of valid entries
    eh_max: u16,       // Capacity of entries
    eh_depth: u16,     // Depth of tree (0 = leaf)
    eh_generation: u32,
}

/// Extent index (for internal nodes)
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
struct Ext4ExtentIdx {
    ei_block: u32,    // Logical block covered by index
    ei_leaf_lo: u32,  // Physical block of next level
    ei_leaf_hi: u16,  // High 16 bits of physical block
    ei_unused: u16,
}

/// Extent (leaf node)
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
struct Ext4Extent {
    ee_block: u32,    // First logical block
    ee_len: u16,      // Number of blocks (max 32768 for initialized)
    ee_start_hi: u16, // High 16 bits of physical block
    ee_start_lo: u32, // Low 32 bits of physical block
}

impl Ext4ExtentIdx {
    fn leaf_block(&self) -> u64 {
        (self.ei_leaf_hi as u64) << 32 | self.ei_leaf_lo as u64
    }
}

impl Ext4Extent {
    fn start_block(&self) -> u64 {
        (self.ee_start_hi as u64) << 32 | self.ee_start_lo as u64
    }

    fn block_count(&self) -> u32 {
        // If bit 15 is set, extent is uninitialized (sparse)
        let len = self.ee_len;
        if len > 32768 {
            (len - 32768) as u32
        } else {
            len as u32
        }
    }

    fn is_uninitialized(&self) -> bool {
        self.ee_len > 32768
    }
}

// ============================================================================
// Driver structures
// ============================================================================

/// Mounted ext4 filesystem (read-only)
pub struct Ext4Fs {
    device: Arc<dyn BlockDevice>,
    block_size: u32,
    blocks_per_group: u32,
    inodes_per_group: u32,
    inode_size: u16,
    groups: RwLock<Vec<Ext4BlockGroupDesc>>,
    first_data_block: u32,
    total_blocks: u64,
    total_inodes: u32,
    has_extents: bool,
    is_64bit: bool,
    desc_size: u16,
}

impl Ext4Fs {
    /// Mount an ext4 filesystem from a block device
    pub fn mount(device: Arc<dyn BlockDevice>) -> KResult<Arc<Self>> {
        // Read superblock (offset 1024)
        let sb_block = EXT4_SUPERBLOCK_OFFSET / device.block_size() as u64;
        let sb_offset = (EXT4_SUPERBLOCK_OFFSET % device.block_size() as u64) as usize;

        let mut buf = vec![0u8; device.block_size() as usize];
        device.read_blocks(sb_block, 1, &mut buf)?;

        let sb: Ext4Superblock = unsafe {
            core::ptr::read_unaligned(buf.as_ptr().add(sb_offset) as *const _)
        };

        // Verify magic
        let s_magic = unsafe { core::ptr::read_unaligned(core::ptr::addr_of!(sb.s_magic)) };
        if s_magic != EXT4_SUPER_MAGIC {
            crate::kprintln!("ext4: invalid magic 0x{:04x}", s_magic);
            return Err(KError::Invalid);
        }

        let block_size = 1024u32 << sb.s_log_block_size;
        let inode_size = if sb.s_rev_level >= 1 { sb.s_inode_size } else { 128 };

        // Check for ext4 features
        let has_extents = (sb.s_feature_incompat & EXT4_FEATURE_INCOMPAT_EXTENTS) != 0;
        let is_64bit = (sb.s_feature_incompat & EXT4_FEATURE_INCOMPAT_64BIT) != 0;

        let total_blocks = if is_64bit {
            (sb.s_blocks_count_hi as u64) << 32 | sb.s_blocks_count_lo as u64
        } else {
            sb.s_blocks_count_lo as u64
        };

        // Descriptor size (32 for ext2/3, 64 for ext4 with 64-bit feature)
        let desc_size = if is_64bit && sb.s_desc_size >= 64 {
            sb.s_desc_size
        } else {
            32
        };

        // Calculate number of block groups
        let groups_count = ((total_blocks + sb.s_blocks_per_group as u64 - 1)
            / sb.s_blocks_per_group as u64) as u32;

        // Read block group descriptor table
        let bgdt_block = if block_size == 1024 { 2 } else { 1 };
        let bgdt_size = groups_count as usize * desc_size as usize;
        let bgdt_blocks = (bgdt_size + block_size as usize - 1) / block_size as usize;

        let mut bgdt_buf = vec![0u8; bgdt_blocks * block_size as usize];
        Self::read_blocks_raw(&device, block_size, bgdt_block as u64, bgdt_blocks as u32, &mut bgdt_buf)?;

        let mut groups = Vec::with_capacity(groups_count as usize);
        for i in 0..groups_count as usize {
            let offset = i * desc_size as usize;
            if desc_size >= 64 {
                // Full 64-byte descriptor
                let desc: Ext4BlockGroupDesc = unsafe {
                    core::ptr::read_unaligned(bgdt_buf.as_ptr().add(offset) as *const _)
                };
                groups.push(desc);
            } else {
                // 32-byte descriptor, fill high parts with zeros
                let small_buf = &bgdt_buf[offset..offset + 32];
                let mut desc = Ext4BlockGroupDesc {
                    bg_block_bitmap_lo: 0,
                    bg_inode_bitmap_lo: 0,
                    bg_inode_table_lo: 0,
                    bg_free_blocks_count_lo: 0,
                    bg_free_inodes_count_lo: 0,
                    bg_used_dirs_count_lo: 0,
                    bg_flags: 0,
                    bg_exclude_bitmap_lo: 0,
                    bg_block_bitmap_csum_lo: 0,
                    bg_inode_bitmap_csum_lo: 0,
                    bg_itable_unused_lo: 0,
                    bg_checksum: 0,
                    bg_block_bitmap_hi: 0,
                    bg_inode_bitmap_hi: 0,
                    bg_inode_table_hi: 0,
                    bg_free_blocks_count_hi: 0,
                    bg_free_inodes_count_hi: 0,
                    bg_used_dirs_count_hi: 0,
                    bg_itable_unused_hi: 0,
                    bg_exclude_bitmap_hi: 0,
                    bg_block_bitmap_csum_hi: 0,
                    bg_inode_bitmap_csum_hi: 0,
                    bg_reserved: 0,
                };
                // Copy first 32 bytes
                unsafe {
                    core::ptr::copy_nonoverlapping(
                        small_buf.as_ptr(),
                        &mut desc as *mut _ as *mut u8,
                        32,
                    );
                }
                groups.push(desc);
            }
        }

        crate::kprintln!(
            "ext4: mounted (blocks={}, block_size={}, extents={}, 64bit={})",
            total_blocks,
            block_size,
            has_extents,
            is_64bit
        );

        Ok(Arc::new(Self {
            device,
            block_size,
            blocks_per_group: sb.s_blocks_per_group,
            inodes_per_group: sb.s_inodes_per_group,
            inode_size,
            groups: RwLock::new(groups),
            first_data_block: sb.s_first_data_block,
            total_blocks,
            total_inodes: sb.s_inodes_count,
            has_extents,
            is_64bit,
            desc_size,
        }))
    }

    /// Read blocks from device, converting block size
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

    /// Read a filesystem block
    fn read_block(&self, block: u64, buf: &mut [u8]) -> KResult<()> {
        Self::read_blocks_raw(&self.device, self.block_size, block, 1, buf)
    }

    /// Read an inode by number
    fn read_inode(&self, ino: u32) -> KResult<Ext4Inode> {
        if ino == 0 {
            return Err(KError::Invalid);
        }

        let group = (ino - 1) / self.inodes_per_group;
        let index = (ino - 1) % self.inodes_per_group;

        let groups = self.groups.read();
        let group_desc = &groups[group as usize];
        let inode_table_block = if self.is_64bit {
            (group_desc.bg_inode_table_hi as u64) << 32 | group_desc.bg_inode_table_lo as u64
        } else {
            group_desc.bg_inode_table_lo as u64
        };
        drop(groups);

        let inode_offset = index as usize * self.inode_size as usize;
        let block_offset = inode_offset / self.block_size as usize;
        let offset_in_block = inode_offset % self.block_size as usize;

        let mut buf = vec![0u8; self.block_size as usize];
        self.read_block(inode_table_block + block_offset as u64, &mut buf)?;

        let inode: Ext4Inode = unsafe {
            core::ptr::read_unaligned(buf.as_ptr().add(offset_in_block) as *const _)
        };

        Ok(inode)
    }

    /// Get file size (64-bit)
    fn inode_size(&self, inode: &Ext4Inode) -> u64 {
        (inode.i_size_high as u64) << 32 | inode.i_size_lo as u64
    }

    /// Check if inode uses extents
    fn uses_extents(&self, inode: &Ext4Inode) -> bool {
        self.has_extents && (inode.i_flags & EXT4_EXTENTS_FL) != 0
    }

    /// Read inode data using extent tree
    fn read_extent_data(&self, inode: &Ext4Inode, offset: usize, out: &mut [u8]) -> KResult<usize> {
        let size = self.inode_size(inode) as usize;
        if offset >= size {
            return Ok(0);
        }

        let to_read = core::cmp::min(out.len(), size - offset);
        let mut read = 0;
        let mut file_offset = offset;

        let mut block_buf = vec![0u8; self.block_size as usize];

        while read < to_read {
            let block_idx = (file_offset / self.block_size as usize) as u32;
            let block_offset = file_offset % self.block_size as usize;
            let chunk = core::cmp::min(to_read - read, self.block_size as usize - block_offset);

            // Find the physical block using extent tree
            let phys_block = self.extent_get_block(inode, block_idx)?;

            if phys_block == 0 {
                // Sparse/uninitialized - fill with zeros
                out[read..read + chunk].fill(0);
            } else {
                self.read_block(phys_block, &mut block_buf)?;
                out[read..read + chunk].copy_from_slice(&block_buf[block_offset..block_offset + chunk]);
            }

            read += chunk;
            file_offset += chunk;
        }

        Ok(read)
    }

    /// Get physical block from extent tree for a logical block
    fn extent_get_block(&self, inode: &Ext4Inode, logical_block: u32) -> KResult<u64> {
        // i_block contains the extent tree root - copy to avoid unaligned access
        let mut extent_data = [0u8; 60];
        unsafe {
            core::ptr::copy_nonoverlapping(
                core::ptr::addr_of!(inode.i_block) as *const u8,
                extent_data.as_mut_ptr(),
                60,
            );
        }

        self.search_extent_tree(&extent_data, logical_block)
    }

    /// Search extent tree for a logical block
    fn search_extent_tree(&self, data: &[u8], logical_block: u32) -> KResult<u64> {
        // Read extent header
        let header: Ext4ExtentHeader = unsafe {
            core::ptr::read_unaligned(data.as_ptr() as *const _)
        };

        if header.eh_magic != EXT4_EXT_MAGIC {
            // Not using extents, fall back to direct blocks
            return self.get_block_direct(data, logical_block);
        }

        if header.eh_depth == 0 {
            // Leaf node - search extents
            for i in 0..header.eh_entries as usize {
                let ext_offset = size_of::<Ext4ExtentHeader>() + i * size_of::<Ext4Extent>();
                let extent: Ext4Extent = unsafe {
                    core::ptr::read_unaligned(data.as_ptr().add(ext_offset) as *const _)
                };

                if logical_block >= extent.ee_block
                    && logical_block < extent.ee_block + extent.block_count()
                {
                    if extent.is_uninitialized() {
                        return Ok(0); // Sparse
                    }
                    let offset_in_extent = logical_block - extent.ee_block;
                    return Ok(extent.start_block() + offset_in_extent as u64);
                }
            }
            Ok(0) // Not found - sparse
        } else {
            // Internal node - find correct child
            let mut child_block: Option<u64> = None;

            for i in 0..header.eh_entries as usize {
                let idx_offset = size_of::<Ext4ExtentHeader>() + i * size_of::<Ext4ExtentIdx>();
                let idx: Ext4ExtentIdx = unsafe {
                    core::ptr::read_unaligned(data.as_ptr().add(idx_offset) as *const _)
                };

                if logical_block >= idx.ei_block {
                    child_block = Some(idx.leaf_block());
                } else {
                    break;
                }
            }

            if let Some(block) = child_block {
                // Read child node and recurse
                let mut buf = vec![0u8; self.block_size as usize];
                self.read_block(block, &mut buf)?;
                self.search_extent_tree(&buf, logical_block)
            } else {
                Ok(0) // Not found
            }
        }
    }

    /// Fall back to direct block pointers (for files not using extents)
    fn get_block_direct(&self, i_block: &[u8], logical_block: u32) -> KResult<u64> {
        // Read block pointers using unaligned access
        let read_block_ptr = |idx: usize| -> u32 {
            unsafe {
                core::ptr::read_unaligned(i_block.as_ptr().add(idx * 4) as *const u32)
            }
        };

        let ptrs_per_block = self.block_size / 4;

        if logical_block < 12 {
            // Direct block
            Ok(read_block_ptr(logical_block as usize) as u64)
        } else if logical_block < 12 + ptrs_per_block {
            // Single indirect
            let idx = logical_block - 12;
            self.read_indirect(read_block_ptr(12) as u64, idx)
        } else if logical_block < 12 + ptrs_per_block + ptrs_per_block * ptrs_per_block {
            // Double indirect
            let idx = logical_block - 12 - ptrs_per_block;
            let l1_idx = idx / ptrs_per_block;
            let l2_idx = idx % ptrs_per_block;
            let l1_block = self.read_indirect(read_block_ptr(13) as u64, l1_idx)?;
            self.read_indirect(l1_block, l2_idx)
        } else {
            // Triple indirect
            let idx = logical_block - 12 - ptrs_per_block - ptrs_per_block * ptrs_per_block;
            let l1_idx = idx / (ptrs_per_block * ptrs_per_block);
            let rem = idx % (ptrs_per_block * ptrs_per_block);
            let l2_idx = rem / ptrs_per_block;
            let l3_idx = rem % ptrs_per_block;
            let l1_block = self.read_indirect(read_block_ptr(14) as u64, l1_idx)?;
            let l2_block = self.read_indirect(l1_block, l2_idx)?;
            self.read_indirect(l2_block, l3_idx)
        }
    }

    /// Read an indirect block pointer
    fn read_indirect(&self, block: u64, idx: u32) -> KResult<u64> {
        if block == 0 {
            return Ok(0);
        }

        let mut buf = vec![0u8; self.block_size as usize];
        self.read_block(block, &mut buf)?;

        let ptr: u32 = unsafe {
            core::ptr::read_unaligned(buf.as_ptr().add(idx as usize * 4) as *const u32)
        };

        Ok(ptr as u64)
    }

    /// Read inode data (handles both extents and direct blocks)
    fn read_inode_data(&self, inode: &Ext4Inode, offset: usize, out: &mut [u8]) -> KResult<usize> {
        if self.uses_extents(inode) {
            self.read_extent_data(inode, offset, out)
        } else {
            // Use direct block method
            let size = self.inode_size(inode) as usize;
            if offset >= size {
                return Ok(0);
            }

            let to_read = core::cmp::min(out.len(), size - offset);
            let mut read = 0;
            let mut file_offset = offset;
            let mut block_buf = vec![0u8; self.block_size as usize];

            // Copy i_block to avoid unaligned access
            let mut i_block_data = [0u8; 60];
            unsafe {
                core::ptr::copy_nonoverlapping(
                    core::ptr::addr_of!(inode.i_block) as *const u8,
                    i_block_data.as_mut_ptr(),
                    60,
                );
            }

            while read < to_read {
                let block_idx = (file_offset / self.block_size as usize) as u32;
                let block_offset = file_offset % self.block_size as usize;
                let chunk = core::cmp::min(to_read - read, self.block_size as usize - block_offset);

                let block_num = self.get_block_direct(&i_block_data, block_idx)?;

                if block_num == 0 {
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
    }

    /// Return root inode
    pub fn root(self: &Arc<Self>) -> Inode {
        Inode(Arc::new(Ext4InodeWrapper {
            fs: Arc::clone(self),
            ino: EXT4_ROOT_INO,
            raw: RwLock::new(None),
            parent: None,
        }))
    }
}

// ============================================================================
// InodeOps implementation
// ============================================================================

struct Ext4InodeWrapper {
    fs: Arc<Ext4Fs>,
    ino: u32,
    raw: RwLock<Option<Ext4Inode>>,
    parent: Option<Inode>,
}

impl Ext4InodeWrapper {
    fn get_raw(&self) -> KResult<Ext4Inode> {
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
            EXT4_S_IFREG => InodeKind::File,
            EXT4_S_IFDIR => InodeKind::Dir,
            EXT4_S_IFLNK => InodeKind::Symlink,
            EXT4_S_IFCHR => InodeKind::CharDev,
            EXT4_S_IFBLK => InodeKind::BlockDev,
            _ => InodeKind::File,
        }
    }

    fn file_type_to_kind(ft: u8) -> InodeKind {
        match ft {
            EXT4_FT_REG_FILE => InodeKind::File,
            EXT4_FT_DIR => InodeKind::Dir,
            EXT4_FT_SYMLINK => InodeKind::Symlink,
            EXT4_FT_CHRDEV => InodeKind::CharDev,
            EXT4_FT_BLKDEV => InodeKind::BlockDev,
            _ => InodeKind::File,
        }
    }
}

impl InodeOps for Ext4InodeWrapper {
    fn metadata(&self) -> Metadata {
        let raw = self.get_raw().unwrap_or(Ext4Inode {
            i_mode: 0,
            i_uid: 0,
            i_size_lo: 0,
            i_atime: 0,
            i_ctime: 0,
            i_mtime: 0,
            i_dtime: 0,
            i_gid: 0,
            i_links_count: 0,
            i_blocks_lo: 0,
            i_flags: 0,
            i_osd1: 0,
            i_block: [0; 15],
            i_generation: 0,
            i_file_acl_lo: 0,
            i_size_high: 0,
            i_obso_faddr: 0,
            i_osd2: [0; 12],
            i_extra_isize: 0,
            i_checksum_hi: 0,
            i_ctime_extra: 0,
            i_mtime_extra: 0,
            i_atime_extra: 0,
            i_crtime: 0,
            i_crtime_extra: 0,
            i_version_hi: 0,
            i_projid: 0,
        });

        Metadata::simple(
            Uid(raw.i_uid as u32),
            Gid(raw.i_gid as u32),
            Mode::from_octal(raw.i_mode & 0o777),
            Self::inode_kind(raw.i_mode),
        )
    }

    fn set_metadata(&self, _meta: Metadata) {
        // Read-only filesystem
    }

    fn parent(&self) -> Option<Inode> {
        self.parent.clone()
    }

    fn lookup(&self, name: &str) -> KResult<Inode> {
        let raw = self.get_raw()?;
        if Self::inode_kind(raw.i_mode) != InodeKind::Dir {
            return Err(KError::NotFound);
        }

        let size = self.fs.inode_size(&raw) as usize;
        let mut buf = vec![0u8; size];
        self.fs.read_inode_data(&raw, 0, &mut buf)?;

        let mut offset = 0;
        while offset < size {
            if offset + 8 > size {
                break;
            }

            let entry: Ext4DirEntry = unsafe {
                core::ptr::read_unaligned(buf.as_ptr().add(offset) as *const _)
            };

            if entry.inode != 0 && entry.name_len > 0 {
                let name_start = offset + 8;
                let name_end = name_start + entry.name_len as usize;
                if name_end <= size {
                    let entry_name = core::str::from_utf8(&buf[name_start..name_end])
                        .unwrap_or("");

                    if entry_name == name {
                        let child = Arc::new(Ext4InodeWrapper {
                            fs: Arc::clone(&self.fs),
                            ino: entry.inode,
                            raw: RwLock::new(None),
                            parent: Some(Inode(Arc::new(Ext4InodeWrapper {
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

    fn create(&self, _name: &str, _kind: InodeKind, _meta: Metadata) -> KResult<Inode> {
        // Read-only filesystem
        Err(KError::NotSupported)
    }

    fn readdir(&self) -> KResult<Vec<DirEntry>> {
        let raw = self.fs.read_inode(self.ino)?;
        if Self::inode_kind(raw.i_mode) != InodeKind::Dir {
            return Err(KError::Invalid);
        }

        let size = self.fs.inode_size(&raw) as usize;
        let mut buf = vec![0u8; size];
        self.fs.read_inode_data(&raw, 0, &mut buf)?;

        let mut entries = Vec::new();
        let mut offset = 0;

        while offset < size {
            if offset + 8 > size {
                break;
            }

            let entry: Ext4DirEntry = unsafe {
                core::ptr::read_unaligned(buf.as_ptr().add(offset) as *const _)
            };

            if entry.inode != 0 && entry.name_len > 0 {
                let name_start = offset + 8;
                let name_end = name_start + entry.name_len as usize;
                if name_end <= size {
                    let name = core::str::from_utf8(&buf[name_start..name_end])
                        .unwrap_or("")
                        .to_string();

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

    fn write_at(&self, _offset: usize, _data: &[u8]) -> KResult<usize> {
        // Read-only filesystem
        Err(KError::NotSupported)
    }

    fn truncate(&self, _size: usize) -> KResult<()> {
        Err(KError::NotSupported)
    }

    fn size(&self) -> KResult<usize> {
        let raw = self.get_raw()?;
        Ok(self.fs.inode_size(&raw) as usize)
    }

    fn unlink(&self, _name: &str) -> KResult<()> {
        Err(KError::NotSupported)
    }

    fn rmdir(&self, _name: &str) -> KResult<()> {
        Err(KError::NotSupported)
    }
}

use alloc::string::ToString;

// ============================================================================
// Helper function to mount ext4
// ============================================================================

/// Try to mount ext4 filesystem (returns error if not ext4)
pub fn try_mount(device: Arc<dyn BlockDevice>) -> KResult<Arc<Ext4Fs>> {
    Ext4Fs::mount(device)
}

/// Check if a device contains an ext4 filesystem
pub fn is_ext4(device: &Arc<dyn BlockDevice>) -> KResult<bool> {
    let sb_block = EXT4_SUPERBLOCK_OFFSET / device.block_size() as u64;
    let sb_offset = (EXT4_SUPERBLOCK_OFFSET % device.block_size() as u64) as usize;

    let mut buf = vec![0u8; device.block_size() as usize];
    device.read_blocks(sb_block, 1, &mut buf)?;

    let magic = unsafe {
        core::ptr::read_unaligned(buf.as_ptr().add(sb_offset + 56) as *const u16)
    };

    if magic != EXT4_SUPER_MAGIC {
        return Ok(false);
    }

    // Check for ext4-specific features
    let feature_incompat = unsafe {
        core::ptr::read_unaligned(buf.as_ptr().add(sb_offset + 96) as *const u32)
    };

    // Has extents or 64-bit feature = ext4
    Ok((feature_incompat & (EXT4_FEATURE_INCOMPAT_EXTENTS | EXT4_FEATURE_INCOMPAT_64BIT)) != 0)
}
