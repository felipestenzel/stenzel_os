//! SPKG Package Format
//!
//! .spkg files are archives containing:
//! - SPKG-INFO: Package metadata (TOML format)
//! - SPKG-SIG: Package signature (Ed25519)
//! - data.tar.zst: Compressed file archive
//!
//! Magic number: "SPKG" (0x53504B47)

use alloc::string::String;
use alloc::vec::Vec;
use crate::util::{KResult, KError};

/// SPKG magic number
pub const MAGIC: [u8; 4] = [0x53, 0x50, 0x4B, 0x47]; // "SPKG"

/// SPKG format version
pub const FORMAT_VERSION: u16 = 1;

/// Package header (at start of .spkg file)
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct PackageHeader {
    /// Magic number (SPKG)
    pub magic: [u8; 4],
    /// Format version
    pub version: u16,
    /// Header flags
    pub flags: u16,
    /// Offset to metadata section
    pub metadata_offset: u32,
    /// Size of metadata section
    pub metadata_size: u32,
    /// Offset to signature section
    pub signature_offset: u32,
    /// Size of signature section
    pub signature_size: u32,
    /// Offset to data section
    pub data_offset: u32,
    /// Size of data section (compressed)
    pub data_size: u32,
    /// Uncompressed data size
    pub data_uncompressed_size: u32,
    /// CRC32 of data section
    pub data_crc32: u32,
    /// Reserved for future use
    pub reserved: [u8; 16],
}

impl PackageHeader {
    pub const SIZE: usize = core::mem::size_of::<Self>();

    /// Create a new package header
    pub fn new() -> Self {
        Self {
            magic: MAGIC,
            version: FORMAT_VERSION,
            flags: 0,
            metadata_offset: 0,
            metadata_size: 0,
            signature_offset: 0,
            signature_size: 0,
            data_offset: 0,
            data_size: 0,
            data_uncompressed_size: 0,
            data_crc32: 0,
            reserved: [0; 16],
        }
    }

    /// Validate the header
    pub fn validate(&self) -> KResult<()> {
        if self.magic != MAGIC {
            return Err(KError::Invalid);
        }
        if self.version > FORMAT_VERSION {
            return Err(KError::NotSupported);
        }
        Ok(())
    }

    /// Parse header from bytes
    pub fn from_bytes(data: &[u8]) -> KResult<Self> {
        if data.len() < Self::SIZE {
            return Err(KError::Invalid);
        }

        let header = unsafe {
            core::ptr::read_unaligned(data.as_ptr() as *const Self)
        };

        header.validate()?;
        Ok(header)
    }

    /// Convert to bytes
    pub fn to_bytes(&self) -> [u8; Self::SIZE] {
        unsafe {
            core::mem::transmute_copy(self)
        }
    }
}

impl Default for PackageHeader {
    fn default() -> Self {
        Self::new()
    }
}

/// Header flags
pub mod flags {
    /// Package is signed
    pub const SIGNED: u16 = 1 << 0;
    /// Package uses zstd compression
    pub const ZSTD: u16 = 1 << 1;
    /// Package uses lz4 compression
    pub const LZ4: u16 = 1 << 2;
    /// Package contains scripts
    pub const HAS_SCRIPTS: u16 = 1 << 3;
    /// Package is a meta-package (no files)
    pub const META_PACKAGE: u16 = 1 << 4;
}

/// A loaded package ready for installation
#[derive(Debug)]
pub struct Package {
    pub header: PackageHeader,
    pub metadata: super::PackageMetadata,
    pub signature: Option<Vec<u8>>,
    pub data: Vec<u8>,
}

impl Package {
    /// Parse a package from raw bytes
    pub fn from_bytes(data: &[u8]) -> KResult<Self> {
        // Parse header
        let header = PackageHeader::from_bytes(data)?;

        // Parse metadata
        let meta_start = header.metadata_offset as usize;
        let meta_end = meta_start + header.metadata_size as usize;
        if meta_end > data.len() {
            return Err(KError::Invalid);
        }
        let meta_bytes = &data[meta_start..meta_end];
        let meta_str = core::str::from_utf8(meta_bytes).map_err(|_| KError::Invalid)?;
        let metadata = super::PackageMetadata::parse(meta_str)?;

        // Parse signature if present
        let signature = if header.flags & flags::SIGNED != 0 {
            let sig_start = header.signature_offset as usize;
            let sig_end = sig_start + header.signature_size as usize;
            if sig_end > data.len() {
                return Err(KError::Invalid);
            }
            Some(data[sig_start..sig_end].to_vec())
        } else {
            None
        };

        // Extract data section (compressed)
        let data_start = header.data_offset as usize;
        let data_end = data_start + header.data_size as usize;
        if data_end > data.len() {
            return Err(KError::Invalid);
        }
        let pkg_data = data[data_start..data_end].to_vec();

        Ok(Self {
            header,
            metadata,
            signature,
            data: pkg_data,
        })
    }

    /// Get decompressed data
    pub fn decompress_data(&self) -> KResult<Vec<u8>> {
        if self.header.flags & flags::ZSTD != 0 {
            // Use zstd decompression
            super::compress::decompress_zstd(&self.data, self.header.data_uncompressed_size as usize)
        } else {
            // Data is not compressed
            Ok(self.data.clone())
        }
    }

    /// Verify package signature
    pub fn verify_signature(&self, public_key: &[u8]) -> KResult<bool> {
        match &self.signature {
            Some(sig) => {
                super::sign::verify_signature(&self.data, sig, public_key)
            }
            None => Ok(false),
        }
    }

    /// Check if package is signed
    pub fn is_signed(&self) -> bool {
        self.header.flags & flags::SIGNED != 0
    }
}

/// TAR archive entry header
#[repr(C)]
#[derive(Debug, Clone)]
pub struct TarHeader {
    pub name: [u8; 100],
    pub mode: [u8; 8],
    pub uid: [u8; 8],
    pub gid: [u8; 8],
    pub size: [u8; 12],
    pub mtime: [u8; 12],
    pub checksum: [u8; 8],
    pub typeflag: u8,
    pub linkname: [u8; 100],
    pub magic: [u8; 6],
    pub version: [u8; 2],
    pub uname: [u8; 32],
    pub gname: [u8; 32],
    pub devmajor: [u8; 8],
    pub devminor: [u8; 8],
    pub prefix: [u8; 155],
    pub padding: [u8; 12],
}

impl TarHeader {
    pub const SIZE: usize = 512;

    /// Get file name
    pub fn name(&self) -> String {
        let name = core::str::from_utf8(&self.name)
            .unwrap_or("")
            .trim_end_matches('\0');
        let prefix = core::str::from_utf8(&self.prefix)
            .unwrap_or("")
            .trim_end_matches('\0');

        if prefix.is_empty() {
            String::from(name)
        } else {
            alloc::format!("{}/{}", prefix, name)
        }
    }

    /// Get file size
    pub fn size(&self) -> usize {
        let size_str = core::str::from_utf8(&self.size)
            .unwrap_or("0")
            .trim_end_matches('\0')
            .trim();
        usize::from_str_radix(size_str, 8).unwrap_or(0)
    }

    /// Get file mode
    pub fn mode(&self) -> u32 {
        let mode_str = core::str::from_utf8(&self.mode)
            .unwrap_or("0")
            .trim_end_matches('\0')
            .trim();
        u32::from_str_radix(mode_str, 8).unwrap_or(0o644)
    }

    /// Check if this is a regular file
    pub fn is_file(&self) -> bool {
        self.typeflag == b'0' || self.typeflag == 0
    }

    /// Check if this is a directory
    pub fn is_directory(&self) -> bool {
        self.typeflag == b'5'
    }

    /// Check if this is a symlink
    pub fn is_symlink(&self) -> bool {
        self.typeflag == b'2'
    }

    /// Get symlink target
    pub fn link_target(&self) -> String {
        String::from(core::str::from_utf8(&self.linkname)
            .unwrap_or("")
            .trim_end_matches('\0'))
    }

    /// Check if this is the end marker
    pub fn is_end(&self) -> bool {
        self.name[0] == 0
    }
}

/// TAR archive reader
pub struct TarArchive<'a> {
    data: &'a [u8],
    offset: usize,
}

impl<'a> TarArchive<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, offset: 0 }
    }

    /// Get next entry
    pub fn next_entry(&mut self) -> Option<TarEntry<'a>> {
        if self.offset + TarHeader::SIZE > self.data.len() {
            return None;
        }

        let header_data = &self.data[self.offset..self.offset + TarHeader::SIZE];
        let header: TarHeader = unsafe {
            core::ptr::read_unaligned(header_data.as_ptr() as *const TarHeader)
        };

        if header.is_end() {
            return None;
        }

        let size = header.size();
        let data_start = self.offset + TarHeader::SIZE;
        let data_end = data_start + size;

        if data_end > self.data.len() {
            return None;
        }

        let entry_data = &self.data[data_start..data_end];

        // Move to next entry (aligned to 512 bytes)
        let padded_size = (size + 511) & !511;
        self.offset = data_start + padded_size;

        Some(TarEntry { header, data: entry_data })
    }
}

/// A TAR archive entry
pub struct TarEntry<'a> {
    pub header: TarHeader,
    pub data: &'a [u8],
}
