use bootloader::{BootConfig, UefiBoot};
use std::fs::{File, OpenOptions};
use std::io::{Seek, SeekFrom, Write};
use std::path::PathBuf;

fn write_le_u32(buf: &mut [u8], off: usize, v: u32) {
    buf[off..off + 4].copy_from_slice(&v.to_le_bytes());
}

fn write_le_u64(buf: &mut [u8], off: usize, v: u64) {
    buf[off..off + 8].copy_from_slice(&v.to_le_bytes());
}

fn write_utf16_name(buf: &mut [u8], off: usize, name: &str) {
    let mut o = off;
    for ch in name.encode_utf16() {
        if o + 2 > buf.len() {
            break;
        }
        buf[o..o + 2].copy_from_slice(&ch.to_le_bytes());
        o += 2;
    }
}

fn write_le_u16(buf: &mut [u8], off: usize, v: u16) {
    buf[off..off + 2].copy_from_slice(&v.to_le_bytes());
}

fn create_virtio_disk_image(path: &PathBuf) {
    // 64 MiB
    const SIZE_BYTES: u64 = 64 * 1024 * 1024;
    const SECTOR: u64 = 512;
    let sectors = SIZE_BYTES / SECTOR;

    let mut f = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)
        .expect("criar virtio disk");
    f.set_len(SIZE_BYTES).expect("set_len");

    // Escreve MBR protetivo (opcional, mas útil).
    let mut mbr = [0u8; 512];
    // Partition entry @ 446
    mbr[446 + 0] = 0x00; // status
    mbr[446 + 4] = 0xEE; // type: GPT protective
    // LBA start
    mbr[446 + 8..446 + 12].copy_from_slice(&1u32.to_le_bytes());
    // size in sectors (capado em u32)
    let mbr_secs = (sectors.saturating_sub(1)).min(u32::MAX as u64) as u32;
    mbr[446 + 12..446 + 16].copy_from_slice(&mbr_secs.to_le_bytes());
    mbr[510] = 0x55;
    mbr[511] = 0xAA;
    f.seek(SeekFrom::Start(0)).unwrap();
    f.write_all(&mbr).unwrap();

    // GPT header em LBA 1 (sem CRCs — o parser do kernel não valida CRC nesta fase).
    let mut hdr = [0u8; 512];
    hdr[0..8].copy_from_slice(b"EFI PART");
    write_le_u32(&mut hdr, 8, 0x0001_0000); // revision
    write_le_u32(&mut hdr, 12, 92); // header size
    // header_crc32 @ 16 (0)
    // reserved @ 20
    write_le_u64(&mut hdr, 24, 1); // current_lba
    write_le_u64(&mut hdr, 32, sectors - 1); // backup_lba
    write_le_u64(&mut hdr, 40, 34); // first_usable
    write_le_u64(&mut hdr, 48, sectors - 34); // last_usable
    // disk guid (qualquer coisa não-zero)
    hdr[56..72].copy_from_slice(&[0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, 0x00]);
    write_le_u64(&mut hdr, 72, 2); // part entries LBA
    write_le_u32(&mut hdr, 80, 128); // num entries
    write_le_u32(&mut hdr, 84, 128); // entry size
    // entries_crc32 @ 88 (0)
    f.seek(SeekFrom::Start(512)).unwrap();
    f.write_all(&hdr).unwrap();

    // Partition entry 0 em LBA 2
    let mut ent = [0u8; 128];
    // type_guid (não-zero)
    ent[0..16].copy_from_slice(&[0xAF, 0x3D, 0x63, 0x0F, 0x83, 0x84, 0x72, 0x47, 0x8E, 0x79, 0x3D, 0x69, 0xD8, 0x47, 0x7D, 0xE4]);
    // unique_guid
    ent[16..32].copy_from_slice(&[0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x10]);

    let first_lba = 2048u64;
    let last_lba = sectors.saturating_sub(2048 + 1);
    write_le_u64(&mut ent, 32, first_lba);
    write_le_u64(&mut ent, 40, last_lba);
    write_le_u64(&mut ent, 48, 0); // attrs
    write_utf16_name(&mut ent, 56, "STENZELROOT");

    f.seek(SeekFrom::Start(512 * 2)).unwrap();
    f.write_all(&ent).unwrap();

    // ========== Cria filesystem ext2 na partição ==========
    let part_offset = first_lba * SECTOR; // 2048 * 512 = 1 MiB
    let part_size = (last_lba - first_lba + 1) * SECTOR;

    // Configurações ext2
    const BLOCK_SIZE: u32 = 1024;
    let total_blocks = (part_size / BLOCK_SIZE as u64) as u32;

    // Layout:
    // Block 0: Boot block (primeiros 1024 bytes, não usado)
    // Block 1: Superblock
    // Block 2: Block Group Descriptor Table
    // Block 3: Block bitmap
    // Block 4: Inode bitmap
    // Blocks 5-36: Inode table (32 blocks = 256 inodes com 128 bytes cada)
    // Block 37+: Data blocks

    const INODES_PER_GROUP: u32 = 256;
    const INODE_SIZE: u16 = 128;
    const INODE_TABLE_BLOCKS: u32 = (INODES_PER_GROUP * INODE_SIZE as u32) / BLOCK_SIZE;
    const FIRST_DATA_BLOCK: u32 = 5 + INODE_TABLE_BLOCKS; // 5 + 32 = 37
    let free_blocks = total_blocks - FIRST_DATA_BLOCK;

    // ---- Superblock (Block 1, offset 1024 na partição) ----
    let mut sb = [0u8; 1024];
    write_le_u32(&mut sb, 0, INODES_PER_GROUP);           // s_inodes_count
    write_le_u32(&mut sb, 4, total_blocks);               // s_blocks_count
    write_le_u32(&mut sb, 8, 0);                          // s_r_blocks_count (reserved)
    write_le_u32(&mut sb, 12, free_blocks - 1);           // s_free_blocks_count (usamos 1 para root dir)
    write_le_u32(&mut sb, 16, INODES_PER_GROUP - 11);     // s_free_inodes_count (inodes 1-11 reservados)
    write_le_u32(&mut sb, 20, 1);                         // s_first_data_block (1 para block_size=1024)
    write_le_u32(&mut sb, 24, 0);                         // s_log_block_size (0 = 1024 bytes)
    write_le_u32(&mut sb, 28, 0);                         // s_log_frag_size
    write_le_u32(&mut sb, 32, total_blocks);              // s_blocks_per_group
    write_le_u32(&mut sb, 36, total_blocks);              // s_frags_per_group
    write_le_u32(&mut sb, 40, INODES_PER_GROUP);          // s_inodes_per_group
    write_le_u32(&mut sb, 44, 0);                         // s_mtime
    write_le_u32(&mut sb, 48, 0);                         // s_wtime
    write_le_u16(&mut sb, 52, 0);                         // s_mnt_count
    write_le_u16(&mut sb, 54, 0xFFFF);                    // s_max_mnt_count
    write_le_u16(&mut sb, 56, 0xEF53);                    // s_magic
    write_le_u16(&mut sb, 58, 1);                         // s_state (clean)
    write_le_u16(&mut sb, 60, 1);                         // s_errors (continue)
    write_le_u16(&mut sb, 62, 0);                         // s_minor_rev_level
    write_le_u32(&mut sb, 64, 0);                         // s_lastcheck
    write_le_u32(&mut sb, 68, 0);                         // s_checkinterval
    write_le_u32(&mut sb, 72, 0);                         // s_creator_os (Linux)
    write_le_u32(&mut sb, 76, 1);                         // s_rev_level (dynamic rev)
    write_le_u16(&mut sb, 80, 0);                         // s_def_resuid
    write_le_u16(&mut sb, 82, 0);                         // s_def_resgid
    // Extended superblock fields (rev 1)
    write_le_u32(&mut sb, 84, 11);                        // s_first_ino (first non-reserved inode)
    write_le_u16(&mut sb, 88, INODE_SIZE);                // s_inode_size
    write_le_u16(&mut sb, 90, 0);                         // s_block_group_nr
    write_le_u32(&mut sb, 92, 0);                         // s_feature_compat
    write_le_u32(&mut sb, 96, 0);                         // s_feature_incompat
    write_le_u32(&mut sb, 100, 0);                        // s_feature_ro_compat
    // UUID
    sb[104..120].copy_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF, 0xCA, 0xFE, 0xBA, 0xBE,
                                    0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0]);
    // Volume name
    sb[120..136].copy_from_slice(b"stenzel-root\0\0\0\0");

    f.seek(SeekFrom::Start(part_offset + 1024)).unwrap();
    f.write_all(&sb).unwrap();

    // ---- Block Group Descriptor (Block 2) ----
    let mut bgd = [0u8; 1024];
    write_le_u32(&mut bgd, 0, 3);                         // bg_block_bitmap (block 3)
    write_le_u32(&mut bgd, 4, 4);                         // bg_inode_bitmap (block 4)
    write_le_u32(&mut bgd, 8, 5);                         // bg_inode_table (block 5)
    write_le_u16(&mut bgd, 12, (free_blocks - 1) as u16); // bg_free_blocks_count
    write_le_u16(&mut bgd, 14, (INODES_PER_GROUP - 11) as u16); // bg_free_inodes_count
    write_le_u16(&mut bgd, 16, 1);                        // bg_used_dirs_count

    f.seek(SeekFrom::Start(part_offset + 2 * BLOCK_SIZE as u64)).unwrap();
    f.write_all(&bgd).unwrap();

    // ---- Block Bitmap (Block 3) ----
    let mut block_bitmap = [0u8; 1024];
    // Marca blocks 0-37 como usados (metadados + 1 data block para root dir)
    // Block FIRST_DATA_BLOCK é usado pelo diretório root
    let used_blocks = FIRST_DATA_BLOCK + 1; // metadados + root dir data
    for i in 0..used_blocks {
        let byte_idx = (i / 8) as usize;
        let bit_idx = i % 8;
        block_bitmap[byte_idx] |= 1 << bit_idx;
    }
    // Marca blocos além do total como usados (padding)
    let total_bits = total_blocks;
    for i in total_bits..(1024 * 8) {
        let byte_idx = (i / 8) as usize;
        let bit_idx = i % 8;
        if byte_idx < 1024 {
            block_bitmap[byte_idx] |= 1 << bit_idx;
        }
    }

    f.seek(SeekFrom::Start(part_offset + 3 * BLOCK_SIZE as u64)).unwrap();
    f.write_all(&block_bitmap).unwrap();

    // ---- Inode Bitmap (Block 4) ----
    let mut inode_bitmap = [0u8; 1024];
    // Inodes 1-11 são reservados, inode 2 é root
    // Marca inodes 1-11 como usados
    for i in 0..11u32 {
        let byte_idx = (i / 8) as usize;
        let bit_idx = i % 8;
        inode_bitmap[byte_idx] |= 1 << bit_idx;
    }

    f.seek(SeekFrom::Start(part_offset + 4 * BLOCK_SIZE as u64)).unwrap();
    f.write_all(&inode_bitmap).unwrap();

    // ---- Inode Table (Blocks 5-36) ----
    let mut inode_table = vec![0u8; (INODE_TABLE_BLOCKS * BLOCK_SIZE) as usize];

    // Inode 2: Root directory
    let root_inode_off = 1 * INODE_SIZE as usize; // inode 2 está no offset 1 (0-indexed)
    write_le_u16(&mut inode_table, root_inode_off + 0, 0o40755);  // i_mode (directory + rwxr-xr-x)
    write_le_u16(&mut inode_table, root_inode_off + 2, 0);        // i_uid
    write_le_u32(&mut inode_table, root_inode_off + 4, BLOCK_SIZE); // i_size (1 block)
    write_le_u32(&mut inode_table, root_inode_off + 8, 0);        // i_atime
    write_le_u32(&mut inode_table, root_inode_off + 12, 0);       // i_ctime
    write_le_u32(&mut inode_table, root_inode_off + 16, 0);       // i_mtime
    write_le_u32(&mut inode_table, root_inode_off + 20, 0);       // i_dtime
    write_le_u16(&mut inode_table, root_inode_off + 24, 0);       // i_gid
    write_le_u16(&mut inode_table, root_inode_off + 26, 2);       // i_links_count (. e ..)
    write_le_u32(&mut inode_table, root_inode_off + 28, 2);       // i_blocks (512-byte blocks = 2 para 1024 bytes)
    write_le_u32(&mut inode_table, root_inode_off + 32, 0);       // i_flags
    write_le_u32(&mut inode_table, root_inode_off + 36, 0);       // i_osd1
    // i_block[0] = primeiro bloco de dados (FIRST_DATA_BLOCK)
    write_le_u32(&mut inode_table, root_inode_off + 40, FIRST_DATA_BLOCK);

    f.seek(SeekFrom::Start(part_offset + 5 * BLOCK_SIZE as u64)).unwrap();
    f.write_all(&inode_table).unwrap();

    // ---- Root directory data (Block FIRST_DATA_BLOCK) ----
    let mut root_dir = [0u8; 1024];
    let mut off = 0usize;

    // Entry: "."
    write_le_u32(&mut root_dir, off + 0, 2);              // inode (root = 2)
    write_le_u16(&mut root_dir, off + 4, 12);             // rec_len
    root_dir[off + 6] = 1;                                // name_len
    root_dir[off + 7] = 2;                                // file_type (directory)
    root_dir[off + 8] = b'.';                             // name
    off += 12;

    // Entry: ".."
    write_le_u32(&mut root_dir, off + 0, 2);              // inode (parent = root = 2)
    write_le_u16(&mut root_dir, off + 4, (BLOCK_SIZE as usize - off) as u16); // rec_len (resto do bloco)
    root_dir[off + 6] = 2;                                // name_len
    root_dir[off + 7] = 2;                                // file_type (directory)
    root_dir[off + 8] = b'.';                             // name[0]
    root_dir[off + 9] = b'.';                             // name[1]

    f.seek(SeekFrom::Start(part_offset + FIRST_DATA_BLOCK as u64 * BLOCK_SIZE as u64)).unwrap();
    f.write_all(&root_dir).unwrap();

    println!("cargo:warning=ext2 filesystem criado: {} blocks, {} inodes", total_blocks, INODES_PER_GROUP);
}

fn main() {
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());

    // Caminho do kernel - tenta artifact dependency primeiro, senão usa caminho direto
    let kernel_path = std::env::var("CARGO_BIN_FILE_STENZEL_KERNEL_stenzel_kernel")
        .or_else(|_| std::env::var("CARGO_BIN_FILE_STENZEL_KERNEL"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            // Fallback: usa caminho direto do target
            // Detecta profile: release usa "release", debug usa "debug"
            let profile = std::env::var("PROFILE").unwrap_or_else(|_| "debug".to_string());
            let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
            let target_dir = manifest_dir.parent().unwrap().join(format!("target/x86_64-unknown-none/{}/stenzel_kernel", profile));
            println!("cargo:rerun-if-changed={}", target_dir.display());
            target_dir
        });

    let config = BootConfig::default();
    let bios = out_dir.join("stenzel-bios.img");
    let uefi = out_dir.join("stenzel-uefi.img");

    bootloader::BiosBoot::new(&kernel_path)
        .set_boot_config(&config)
        .create_disk_image(&bios)
        .expect("criar BIOS image");

    UefiBoot::new(&kernel_path)
        .set_boot_config(&config)
        .create_disk_image(&uefi)
        .expect("criar UEFI image");

    // Cria disco virtio-blk
    let virtio_disk = out_dir.join("stenzel-virtio-disk.img");
    create_virtio_disk_image(&virtio_disk);

    // Exporta caminhos para o runner
    println!("cargo:rustc-env=STENZEL_BIOS_IMG={}", bios.display());
    println!("cargo:rustc-env=STENZEL_UEFI_IMG={}", uefi.display());
    println!("cargo:rustc-env=STENZEL_VIRTIO_DISK={}", virtio_disk.display());
}
