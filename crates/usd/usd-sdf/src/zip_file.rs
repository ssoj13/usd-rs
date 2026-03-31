//! ZIP file handling for USDZ format.
//!
//! This module provides reading and writing of ZIP archives, primarily for
//! supporting the .usdz file format. It implements the subset of the ZIP
//! specification needed for USD packages.
//!
//! Per USDZ specification, file data is aligned to 64-byte boundaries.

use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

// ============================================================================
// Constants
// ============================================================================

/// Local file header signature
const LOCAL_FILE_HEADER_SIGNATURE: u32 = 0x04034b50;

/// Central directory header signature
const CENTRAL_DIRECTORY_HEADER_SIGNATURE: u32 = 0x02014b50;

/// End of central directory record signature
const END_OF_CENTRAL_DIRECTORY_SIGNATURE: u32 = 0x06054b50;

/// Fixed size of local file header (excluding variable fields)
const LOCAL_FILE_HEADER_FIXED_SIZE: usize = 30;

/// USDZ data alignment requirement (64 bytes)
const DATA_ALIGNMENT: usize = 64;

/// Extra field header size
const EXTRA_FIELD_HEADER_SIZE: usize = 4;

/// Arbitrary header ID for padding (unreserved)
const PADDING_HEADER_ID: u16 = 0x1986;

// ============================================================================
// MS-DOS Timestamp helpers (P2-1)
// ============================================================================

/// Encodes a `SystemTime` as an MS-DOS (FAT) date/time pair.
///
/// MS-DOS date: bits [15:9] = year-1980, [8:5] = month, [4:0] = day
/// MS-DOS time: bits [15:11] = hours, [10:5] = minutes, [4:0] = seconds/2
fn dos_timestamp(t: SystemTime) -> (u16, u16) {
    // Seconds since Unix epoch, saturating to 0 for times before 1970
    let secs = t.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();

    // Simple decomposition without pulling in chrono:
    // days since epoch → year/month/day
    let days = (secs / 86400) as u32;
    let time_of_day = (secs % 86400) as u32;

    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;

    // Gregorian calendar decomposition from day count (days since 1970-01-01)
    // Using the algorithm from POSIX (proleptic Gregorian)
    let z = days + 719468;
    let era = z / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };

    // Clamp to MS-DOS epoch range [1980..2107]
    let dos_year = if y >= 1980 { (y - 1980) as u16 } else { 0 };
    let dos_date = (dos_year << 9) | ((m as u16) << 5) | (d as u16);
    let dos_time = ((hours as u16) << 11) | ((minutes as u16) << 5) | ((seconds as u16) >> 1);

    (dos_time, dos_date)
}

/// Returns MS-DOS timestamp for the current moment.
#[inline]
fn dos_timestamp_now() -> (u16, u16) {
    dos_timestamp(SystemTime::now())
}

/// Max individual file entry size (ZIP32 limit: 4GB - 1).
/// Matches C++ SDF_MAX_ZIPFILE_ENTRY_SIZE.
const MAX_ENTRY_SIZE: usize = u32::MAX as usize;

/// Max total archive size (ZIP32 limit: 4GB - 1).
/// Matches C++ SDF_MAX_ZIPFILE_SIZE.
const MAX_ARCHIVE_SIZE: usize = u32::MAX as usize;

/// Allowed file extensions in a USDZ package per spec.
/// Includes USD scene formats, image textures, audio, and video.
const ALLOWED_USDZ_EXTENSIONS: &[&str] = &[
    // USD scene formats
    "usdc", "usdz", "usda", "usd", // Image textures
    "png", "jpg", "jpeg", "exr", "svg", "avif", "webp", // Audio
    "mp3", "m4a", "wav", "ogg", // Video
    "mp4", "m4v", "mov", // Geometry interchange
    "abc",
];

// ============================================================================
// Error types
// ============================================================================

/// Error type for ZIP operations.
#[derive(Debug, Clone)]
pub enum ZipError {
    /// IO error
    Io(String),
    /// Invalid zip file format
    InvalidFormat(String),
    /// File not found in archive
    FileNotFound(String),
    /// Archive too large
    ArchiveTooLarge,
}

impl std::fmt::Display for ZipError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(msg) => write!(f, "IO error: {}", msg),
            Self::InvalidFormat(msg) => write!(f, "Invalid ZIP format: {}", msg),
            Self::FileNotFound(path) => write!(f, "File not found in archive: {}", path),
            Self::ArchiveTooLarge => write!(f, "Archive exceeds maximum size"),
        }
    }
}

impl std::error::Error for ZipError {}

impl From<std::io::Error> for ZipError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err.to_string())
    }
}

// ============================================================================
// FileInfo - Information about a file in the archive
// ============================================================================

/// Information about a file in the zip archive.
#[derive(Debug, Clone, Default)]
pub struct FileInfo {
    /// Offset of the beginning of this file's data from archive start.
    pub data_offset: usize,
    /// Size of file as stored (compressed size if compressed).
    pub size: usize,
    /// Uncompressed size of this file.
    pub uncompressed_size: usize,
    /// CRC-32 checksum of uncompressed data.
    pub crc: u32,
    /// Compression method (0 = stored, 8 = deflate).
    pub compression_method: u16,
    /// Whether the file is encrypted.
    pub encrypted: bool,
}

// ============================================================================
// LocalFileHeader - Header for each file in the archive
// ============================================================================

/// Local file header for each file in the zip archive.
/// See section 4.3.7 in zip file specification.
#[derive(Debug, Clone, Default)]
struct LocalFileHeader {
    signature: u32,
    version_for_extract: u16,
    bits: u16,
    compression_method: u16,
    last_mod_time: u16,
    last_mod_date: u16,
    crc32: u32,
    compressed_size: u32,
    uncompressed_size: u32,
    filename_length: u16,
    extra_field_length: u16,
    /// Filename (not null-terminated)
    filename: String,
    /// Extra field data
    extra_field: Vec<u8>,
    /// Offset to start of file data
    data_offset: usize,
}

impl LocalFileHeader {
    /// Reads a local file header from bytes at given offset.
    fn read_from(data: &[u8], offset: usize) -> Option<(Self, usize)> {
        if data.len() < offset + LOCAL_FILE_HEADER_FIXED_SIZE {
            return None;
        }

        let mut pos = offset;

        let signature =
            u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]);
        pos += 4;

        if signature != LOCAL_FILE_HEADER_SIGNATURE {
            return None;
        }

        let version_for_extract = u16::from_le_bytes([data[pos], data[pos + 1]]);
        pos += 2;
        let bits = u16::from_le_bytes([data[pos], data[pos + 1]]);
        pos += 2;
        let compression_method = u16::from_le_bytes([data[pos], data[pos + 1]]);
        pos += 2;
        let last_mod_time = u16::from_le_bytes([data[pos], data[pos + 1]]);
        pos += 2;
        let last_mod_date = u16::from_le_bytes([data[pos], data[pos + 1]]);
        pos += 2;
        let crc32 = u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]);
        pos += 4;
        let compressed_size =
            u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]);
        pos += 4;
        let uncompressed_size =
            u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]);
        pos += 4;
        let filename_length = u16::from_le_bytes([data[pos], data[pos + 1]]);
        pos += 2;
        let extra_field_length = u16::from_le_bytes([data[pos], data[pos + 1]]);
        pos += 2;

        // Read filename
        let filename_end = pos + filename_length as usize;
        if data.len() < filename_end {
            return None;
        }
        let filename = String::from_utf8_lossy(&data[pos..filename_end]).to_string();
        pos = filename_end;

        // Read extra field
        let extra_end = pos + extra_field_length as usize;
        if data.len() < extra_end {
            return None;
        }
        let extra_field = data[pos..extra_end].to_vec();
        pos = extra_end;

        // Data starts after header
        let data_offset = pos;

        // Skip past the file data
        let next_offset = pos + compressed_size as usize;
        if data.len() < next_offset {
            return None;
        }

        Some((
            Self {
                signature,
                version_for_extract,
                bits,
                compression_method,
                last_mod_time,
                last_mod_date,
                crc32,
                compressed_size,
                uncompressed_size,
                filename_length,
                extra_field_length,
                filename,
                extra_field,
                data_offset,
            },
            next_offset,
        ))
    }

    /// Writes the local file header to a writer.
    fn write_to<W: Write>(&self, writer: &mut W, file_data: &[u8]) -> std::io::Result<()> {
        writer.write_all(&self.signature.to_le_bytes())?;
        writer.write_all(&self.version_for_extract.to_le_bytes())?;
        writer.write_all(&self.bits.to_le_bytes())?;
        writer.write_all(&self.compression_method.to_le_bytes())?;
        writer.write_all(&self.last_mod_time.to_le_bytes())?;
        writer.write_all(&self.last_mod_date.to_le_bytes())?;
        writer.write_all(&self.crc32.to_le_bytes())?;
        writer.write_all(&self.compressed_size.to_le_bytes())?;
        writer.write_all(&self.uncompressed_size.to_le_bytes())?;
        writer.write_all(&self.filename_length.to_le_bytes())?;
        writer.write_all(&self.extra_field_length.to_le_bytes())?;
        writer.write_all(self.filename.as_bytes())?;
        writer.write_all(&self.extra_field)?;
        writer.write_all(file_data)?;
        Ok(())
    }

    /// Returns file info for this header.
    fn file_info(&self) -> FileInfo {
        FileInfo {
            data_offset: self.data_offset,
            size: self.compressed_size as usize,
            uncompressed_size: self.uncompressed_size as usize,
            crc: self.crc32,
            compression_method: self.compression_method,
            encrypted: (self.bits & 0x1) != 0,
        }
    }
}

// ============================================================================
// CentralDirectoryHeader - Central directory entry
// ============================================================================

/// Central directory header for each file in the zip archive.
/// See section 4.3.12 in zip file specification.
#[derive(Debug, Clone, Default)]
struct CentralDirectoryHeader {
    signature: u32,
    version_made_by: u16,
    version_for_extract: u16,
    bits: u16,
    compression_method: u16,
    last_mod_time: u16,
    last_mod_date: u16,
    crc32: u32,
    compressed_size: u32,
    uncompressed_size: u32,
    filename_length: u16,
    extra_field_length: u16,
    comment_length: u16,
    disk_number_start: u16,
    internal_attrs: u16,
    external_attrs: u32,
    local_header_offset: u32,
    filename: String,
    extra_field: Vec<u8>,
    comment: Vec<u8>,
}

impl CentralDirectoryHeader {
    /// Creates a central directory header from a local file header.
    fn from_local(local: &LocalFileHeader, local_offset: u32) -> Self {
        Self {
            signature: CENTRAL_DIRECTORY_HEADER_SIGNATURE,
            version_made_by: 0,
            version_for_extract: local.version_for_extract,
            bits: local.bits,
            compression_method: local.compression_method,
            last_mod_time: local.last_mod_time,
            last_mod_date: local.last_mod_date,
            crc32: local.crc32,
            compressed_size: local.compressed_size,
            uncompressed_size: local.uncompressed_size,
            filename_length: local.filename_length,
            extra_field_length: local.extra_field_length,
            comment_length: 0,
            disk_number_start: 0,
            internal_attrs: 0,
            external_attrs: 0,
            local_header_offset: local_offset,
            filename: local.filename.clone(),
            extra_field: local.extra_field.clone(),
            comment: Vec::new(),
        }
    }

    /// Writes the central directory header to a writer.
    fn write_to<W: Write>(&self, writer: &mut W) -> std::io::Result<()> {
        writer.write_all(&self.signature.to_le_bytes())?;
        writer.write_all(&self.version_made_by.to_le_bytes())?;
        writer.write_all(&self.version_for_extract.to_le_bytes())?;
        writer.write_all(&self.bits.to_le_bytes())?;
        writer.write_all(&self.compression_method.to_le_bytes())?;
        writer.write_all(&self.last_mod_time.to_le_bytes())?;
        writer.write_all(&self.last_mod_date.to_le_bytes())?;
        writer.write_all(&self.crc32.to_le_bytes())?;
        writer.write_all(&self.compressed_size.to_le_bytes())?;
        writer.write_all(&self.uncompressed_size.to_le_bytes())?;
        writer.write_all(&self.filename_length.to_le_bytes())?;
        writer.write_all(&self.extra_field_length.to_le_bytes())?;
        writer.write_all(&self.comment_length.to_le_bytes())?;
        writer.write_all(&self.disk_number_start.to_le_bytes())?;
        writer.write_all(&self.internal_attrs.to_le_bytes())?;
        writer.write_all(&self.external_attrs.to_le_bytes())?;
        writer.write_all(&self.local_header_offset.to_le_bytes())?;
        writer.write_all(self.filename.as_bytes())?;
        writer.write_all(&self.extra_field)?;
        writer.write_all(&self.comment)?;
        Ok(())
    }
}

// ============================================================================
// EndOfCentralDirectory - End of central directory record
// ============================================================================

/// End of central directory record.
/// See section 4.3.16 in zip file specification.
#[derive(Debug, Clone, Default)]
struct EndOfCentralDirectory {
    signature: u32,
    disk_number: u16,
    disk_number_for_central_dir: u16,
    num_central_dir_entries_on_disk: u16,
    num_central_dir_entries: u16,
    central_dir_length: u32,
    central_dir_offset: u32,
    comment_length: u16,
    comment: Vec<u8>,
}

impl EndOfCentralDirectory {
    /// Writes the end of central directory record to a writer.
    fn write_to<W: Write>(&self, writer: &mut W) -> std::io::Result<()> {
        writer.write_all(&self.signature.to_le_bytes())?;
        writer.write_all(&self.disk_number.to_le_bytes())?;
        writer.write_all(&self.disk_number_for_central_dir.to_le_bytes())?;
        writer.write_all(&self.num_central_dir_entries_on_disk.to_le_bytes())?;
        writer.write_all(&self.num_central_dir_entries.to_le_bytes())?;
        writer.write_all(&self.central_dir_length.to_le_bytes())?;
        writer.write_all(&self.central_dir_offset.to_le_bytes())?;
        writer.write_all(&self.comment_length.to_le_bytes())?;
        writer.write_all(&self.comment)?;
        Ok(())
    }
}

// ============================================================================
// ZipFile - Reader for zip archives
// ============================================================================

/// Class for reading a zip file. Primarily intended for .usdz format.
///
/// This is not a general-purpose zip reader - it doesn't support compression
/// and reads by scanning local file headers rather than the central directory.
#[derive(Debug, Clone)]
pub struct ZipFile {
    /// Raw data buffer
    data: Vec<u8>,
    /// Cached file entries: filename -> (data_offset, header)
    entries: HashMap<String, (usize, LocalFileHeader)>,
}

impl ZipFile {
    /// Opens a zip archive from file path.
    pub fn open(file_path: impl AsRef<Path>) -> Result<Self, ZipError> {
        let mut file = File::open(file_path.as_ref())?;
        let mut data = Vec::new();
        file.read_to_end(&mut data)?;
        Self::from_bytes(data)
    }

    /// Opens a zip archive from bytes.
    pub fn from_bytes(data: Vec<u8>) -> Result<Self, ZipError> {
        let mut zip = Self {
            data,
            entries: HashMap::new(),
        };
        zip.scan_entries()?;
        Ok(zip)
    }

    /// Scans the archive for local file headers.
    fn scan_entries(&mut self) -> Result<(), ZipError> {
        let mut offset = 0;

        while offset < self.data.len() {
            if let Some((header, next_offset)) = LocalFileHeader::read_from(&self.data, offset) {
                let filename = header.filename.clone();
                self.entries.insert(filename, (offset, header));
                offset = next_offset;
            } else {
                // No more local file headers
                break;
            }
        }

        Ok(())
    }

    /// Returns true if this is a valid zip file.
    pub fn is_valid(&self) -> bool {
        !self.entries.is_empty()
    }

    /// Returns iterator over file names in the archive.
    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.entries.keys().map(|s| s.as_str())
    }

    /// Returns the first file in the archive (by scan order).
    pub fn first_file(&self) -> Option<&str> {
        // Find entry with lowest offset
        self.entries
            .iter()
            .min_by_key(|(_, (offset, _))| *offset)
            .map(|(name, _)| name.as_str())
    }

    /// Finds a file by path and returns its info.
    pub fn find(&self, path: &str) -> Option<FileInfo> {
        self.entries.get(path).map(|(_, header)| header.file_info())
    }

    /// Returns raw data for a file in the archive.
    pub fn get_file_data(&self, path: &str) -> Option<&[u8]> {
        let (_, header) = self.entries.get(path)?;
        let start = header.data_offset;
        let end = start + header.compressed_size as usize;
        if end <= self.data.len() {
            Some(&self.data[start..end])
        } else {
            None
        }
    }

    /// Returns all file names in the archive.
    pub fn file_names(&self) -> Vec<&str> {
        self.entries.keys().map(|s| s.as_str()).collect()
    }

    /// Returns number of files in the archive.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns true if the archive is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Dumps contents to stdout for diagnostics.
    pub fn dump_contents(&self) {
        println!("    Offset\t      Comp\t    Uncomp\tName");
        println!("    ------\t      ----\t    ------\t----");

        for (name, (_, header)) in &self.entries {
            let info = header.file_info();
            println!(
                "{:10}\t{:10}\t{:10}\t{}",
                info.data_offset, info.size, info.uncompressed_size, name
            );
        }

        println!("----------");
        println!("{} files total", self.entries.len());
    }
}

// ============================================================================
// ZipFileWriter - Writer for zip archives
// ============================================================================

/// Class for writing a zip file. Primarily intended for .usdz format.
///
/// All files are stored without compression. Data is aligned to 64-byte
/// boundaries per USDZ specification.
pub struct ZipFileWriter {
    /// Output file path
    file_path: String,
    /// Buffer for archive data
    buffer: Vec<u8>,
    /// Records of added files: (filename, local_header, offset)
    added_files: Vec<(String, LocalFileHeader, u32)>,
}

impl ZipFileWriter {
    /// Creates a new zip file writer for the given destination path.
    pub fn create_new(file_path: impl Into<String>) -> Self {
        Self {
            file_path: file_path.into(),
            buffer: Vec::new(),
            added_files: Vec::new(),
        }
    }

    /// Computes padding bytes needed to align data at given offset to 64 bytes.
    fn compute_padding_size(offset: usize) -> u16 {
        let remainder = offset % DATA_ALIGNMENT;
        if remainder == 0 {
            0
        } else {
            let mut padding = (DATA_ALIGNMENT - remainder) as u16;
            // If padding is too small for the header, bump it up
            if (padding as usize) < EXTRA_FIELD_HEADER_SIZE {
                padding += DATA_ALIGNMENT as u16;
            }
            padding
        }
    }

    /// Creates extra field padding buffer.
    fn create_padding_field(size: u16) -> Vec<u8> {
        if size == 0 {
            return Vec::new();
        }

        let mut field = Vec::with_capacity(size as usize);
        // Header ID
        field.extend_from_slice(&PADDING_HEADER_ID.to_le_bytes());
        // Data size (total size minus header)
        let data_size = size - EXTRA_FIELD_HEADER_SIZE as u16;
        field.extend_from_slice(&data_size.to_le_bytes());
        // Padding data (zeros)
        field.resize(size as usize, 0);
        field
    }

    /// Computes CRC-32 checksum for data.
    pub fn crc32(data: &[u8]) -> u32 {
        static CRC_TABLE: [u32; 256] = [
            0x00000000, 0x77073096, 0xEE0E612C, 0x990951BA, 0x076DC419, 0x706AF48F, 0xE963A535,
            0x9E6495A3, 0x0eDB8832, 0x79DCB8A4, 0xE0D5E91E, 0x97D2D988, 0x09B64C2B, 0x7EB17CBD,
            0xE7B82D07, 0x90BF1D91, 0x1DB71064, 0x6AB020F2, 0xF3B97148, 0x84BE41DE, 0x1ADAD47D,
            0x6DDDE4EB, 0xF4D4B551, 0x83D385C7, 0x136C9856, 0x646BA8C0, 0xFD62F97A, 0x8A65C9EC,
            0x14015C4F, 0x63066CD9, 0xFA0F3D63, 0x8D080DF5, 0x3B6E20C8, 0x4C69105E, 0xD56041E4,
            0xA2677172, 0x3C03E4D1, 0x4B04D447, 0xD20D85FD, 0xA50AB56B, 0x35B5A8FA, 0x42B2986C,
            0xDBBBC9D6, 0xACBCF940, 0x32D86CE3, 0x45DF5C75, 0xDCD60DCF, 0xABD13D59, 0x26D930AC,
            0x51DE003A, 0xC8D75180, 0xBFD06116, 0x21B4F4B5, 0x56B3C423, 0xCFBA9599, 0xB8BDA50F,
            0x2802B89E, 0x5F058808, 0xC60CD9B2, 0xB10BE924, 0x2F6F7C87, 0x58684C11, 0xC1611DAB,
            0xB6662D3D, 0x76DC4190, 0x01DB7106, 0x98D220BC, 0xEFD5102A, 0x71B18589, 0x06B6B51F,
            0x9FBFE4A5, 0xE8B8D433, 0x7807C9A2, 0x0F00F934, 0x9609A88E, 0xE10E9818, 0x7F6A0DBB,
            0x086D3D2D, 0x91646C97, 0xE6635C01, 0x6B6B51F4, 0x1C6C6162, 0x856530D8, 0xF262004E,
            0x6C0695ED, 0x1B01A57B, 0x8208F4C1, 0xF50FC457, 0x65B0D9C6, 0x12B7E950, 0x8BBEB8EA,
            0xFCB9887C, 0x62DD1DDF, 0x15DA2D49, 0x8CD37CF3, 0xFBD44C65, 0x4DB26158, 0x3AB551CE,
            0xA3BC0074, 0xD4BB30E2, 0x4ADFA541, 0x3DD895D7, 0xA4D1C46D, 0xD3D6F4FB, 0x4369E96A,
            0x346ED9FC, 0xAD678846, 0xDA60B8D0, 0x44042D73, 0x33031DE5, 0xAA0A4C5F, 0xDD0D7CC9,
            0x5005713C, 0x270241AA, 0xBE0B1010, 0xC90C2086, 0x5768B525, 0x206F85B3, 0xB966D409,
            0xCE61E49F, 0x5EDEF90E, 0x29D9C998, 0xB0D09822, 0xC7D7A8B4, 0x59B33D17, 0x2EB40D81,
            0xB7BD5C3B, 0xC0BA6CAD, 0xEDB88320, 0x9ABFB3B6, 0x03B6E20C, 0x74B1D29A, 0xEAD54739,
            0x9DD277AF, 0x04DB2615, 0x73DC1683, 0xE3630B12, 0x94643B84, 0x0D6D6A3E, 0x7A6A5AA8,
            0xE40ECF0B, 0x9309FF9D, 0x0A00AE27, 0x7D079EB1, 0xF00F9344, 0x8708A3D2, 0x1E01F268,
            0x6906C2FE, 0xF762575D, 0x806567CB, 0x196C3671, 0x6E6B06E7, 0xFED41B76, 0x89D32BE0,
            0x10DA7A5A, 0x67DD4ACC, 0xF9B9DF6F, 0x8EBEEFF9, 0x17B7BE43, 0x60B08ED5, 0xD6D6A3E8,
            0xA1D1937E, 0x38D8C2C4, 0x4FDFF252, 0xD1BB67F1, 0xA6BC5767, 0x3FB506DD, 0x48B2364B,
            0xD80D2BDA, 0xAF0A1B4C, 0x36034AF6, 0x41047A60, 0xDF60EFC3, 0xA867DF55, 0x316E8EEF,
            0x4669BE79, 0xCB61B38C, 0xBC66831A, 0x256FD2A0, 0x5268E236, 0xCC0C7795, 0xBB0B4703,
            0x220216B9, 0x5505262F, 0xC5BA3BBE, 0xB2BD0B28, 0x2BB45A92, 0x5CB36A04, 0xC2D7FFA7,
            0xB5D0CF31, 0x2CD99E8B, 0x5BDEAE1D, 0x9B64C2B0, 0xEC63F226, 0x756AA39C, 0x026D930A,
            0x9C0906A9, 0xEB0E363F, 0x72076785, 0x05005713, 0x95BF4A82, 0xE2B87A14, 0x7BB12BAE,
            0x0CB61B38, 0x92D28E9B, 0xE5D5BE0D, 0x7CDCEFB7, 0x0BDBDF21, 0x86D3D2D4, 0xF1D4E242,
            0x68DDB3F8, 0x1FDA836E, 0x81BE16CD, 0xF6B9265B, 0x6FB077E1, 0x18B74777, 0x88085AE6,
            0xFF0F6A70, 0x66063BCA, 0x11010B5C, 0x8F659EFF, 0xF862AE69, 0x616BFFD3, 0x166CCF45,
            0xA00AE278, 0xD70DD2EE, 0x4E048354, 0x3903B3C2, 0xA7672661, 0xD06016F7, 0x4969474D,
            0x3E6E77DB, 0xAED16A4A, 0xD9D65ADC, 0x40DF0B66, 0x37D83BF0, 0xA9BCAE53, 0xDEBB9EC5,
            0x47B2CF7F, 0x30B5FFE9, 0xBDBDF21C, 0xCABAC28A, 0x53B39330, 0x24B4A3A6, 0xBAD03605,
            0xCDD70693, 0x54DE5729, 0x23D967BF, 0xB3667A2E, 0xC4614AB8, 0x5D681B02, 0x2A6F2B94,
            0xB40BBE37, 0xC30C8EA1, 0x5A05DF1B, 0x2D02EF8D,
        ];

        let mut crc = !0u32;
        for &byte in data {
            crc = (crc >> 8) ^ CRC_TABLE[(byte ^ (crc as u8)) as usize];
        }
        !crc
    }

    /// Sanitizes file path for zip specification.
    /// - No drive letters
    /// - Forward slashes only
    /// - No leading slash
    fn sanitize_path(path: &str) -> String {
        let mut result = path.replace('\\', "/");
        // Strip drive letter if present (e.g., "C:/")
        if result.len() >= 2 && result.chars().nth(1) == Some(':') {
            result = result[2..].to_string();
        }
        // Strip leading slashes
        result = result.trim_start_matches('/').to_string();
        result
    }

    /// Adds a file from disk to the archive, using the source file's mtime for the ZIP header.
    /// Returns the path used in the archive, or error.
    pub fn add_file(
        &mut self,
        file_path: &str,
        archive_path: Option<&str>,
    ) -> Result<String, ZipError> {
        // P2-1: Capture source file mtime before reading (metadata() is cheap)
        let (mod_time, mod_date) = std::fs::metadata(file_path)
            .ok()
            .and_then(|m| m.modified().ok())
            .map(dos_timestamp)
            .unwrap_or_else(dos_timestamp_now);

        // Read file data
        let mut file = File::open(file_path)?;
        let mut data = Vec::new();
        file.read_to_end(&mut data)?;

        // Determine archive path
        let path_in_archive = archive_path.unwrap_or(file_path);
        let zip_path = Self::sanitize_path(path_in_archive);

        self.add_file_data_with_time(&zip_path, &data, mod_time, mod_date)
    }

    /// Checks whether `ext` (without dot) is an allowed USDZ file type.
    pub fn is_allowed_usdz_extension(ext: &str) -> bool {
        ALLOWED_USDZ_EXTENSIONS.contains(&ext.to_lowercase().as_str())
    }

    /// Adds file data to the archive using the current time as the modification timestamp.
    /// Returns the path used in the archive, or error.
    ///
    /// Validates:
    /// - File size does not exceed ZIP32 u32 limit (P1-1)
    /// - Archive total size does not exceed ZIP32 u32 limit (P1-1)
    /// - File extension is allowed per USDZ spec (P1-2)
    /// - Duplicate filenames are rejected (P2-4)
    pub fn add_file_data(&mut self, archive_path: &str, data: &[u8]) -> Result<String, ZipError> {
        // P2-1: Use current time for in-memory data
        let (mod_time, mod_date) = dos_timestamp_now();
        self.add_file_data_with_time(archive_path, data, mod_time, mod_date)
    }

    /// Adds file data with explicit MS-DOS timestamps (internal).
    fn add_file_data_with_time(
        &mut self,
        archive_path: &str,
        data: &[u8],
        mod_time: u16,
        mod_date: u16,
    ) -> Result<String, ZipError> {
        let zip_path = Self::sanitize_path(archive_path);

        // P1-1: Reject files exceeding ZIP32 individual entry limit
        if data.len() > MAX_ENTRY_SIZE {
            return Err(ZipError::ArchiveTooLarge);
        }

        // P1-2: Validate file extension against USDZ allowed types
        if let Some(ext) = std::path::Path::new(&zip_path)
            .extension()
            .and_then(|e| e.to_str())
        {
            if !Self::is_allowed_usdz_extension(ext) {
                return Err(ZipError::InvalidFormat(format!(
                    "File type '.{}' is not allowed in USDZ packages. Allowed: {:?}",
                    ext, ALLOWED_USDZ_EXTENSIONS
                )));
            }
        }
        // Files without extension are allowed (e.g. bare filenames in rare cases)

        // P2-4: Reject duplicate filenames
        if self
            .added_files
            .iter()
            .any(|(name, _, _)| name == &zip_path)
        {
            return Err(ZipError::InvalidFormat(format!(
                "Duplicate file in archive: '{}'",
                zip_path
            )));
        }

        // P1-1: Check that archive won't exceed ZIP32 total size limit.
        // Estimate: current buffer + header + filename + max padding + data + CD overhead
        let estimated_addition =
            LOCAL_FILE_HEADER_FIXED_SIZE + zip_path.len() + DATA_ALIGNMENT + data.len();
        if self.buffer.len().saturating_add(estimated_addition) > MAX_ARCHIVE_SIZE {
            return Err(ZipError::ArchiveTooLarge);
        }

        // Calculate header offset
        let offset = self.buffer.len() as u32;

        // Calculate data offset after header
        let data_offset_before_extra =
            offset as usize + LOCAL_FILE_HEADER_FIXED_SIZE + zip_path.len();

        // Calculate padding needed for 64-byte alignment
        let padding_size = Self::compute_padding_size(data_offset_before_extra);
        let extra_field = Self::create_padding_field(padding_size);

        // Build local file header
        let header = LocalFileHeader {
            signature: LOCAL_FILE_HEADER_SIGNATURE,
            version_for_extract: 10, // Default
            bits: 0,
            compression_method: 0, // No compression
            last_mod_time: mod_time,
            last_mod_date: mod_date,
            crc32: Self::crc32(data),
            compressed_size: data.len() as u32,
            uncompressed_size: data.len() as u32,
            filename_length: zip_path.len() as u16,
            extra_field_length: extra_field.len() as u16,
            filename: zip_path.clone(),
            extra_field,
            data_offset: 0,
        };

        // Write header and data to buffer
        header.write_to(&mut self.buffer, data)?;

        self.added_files.push((zip_path.clone(), header, offset));

        Ok(zip_path)
    }

    /// Finalizes and saves the zip archive to disk.
    ///
    /// Takes `&mut self` (P2-3) so caller retains ownership on error and can retry.
    /// Central directory and EOCD are appended to a temporary copy of the buffer
    /// so that `self.buffer` (containing only local entries) stays intact on error.
    pub fn save(&mut self) -> Result<(), ZipError> {
        // Build final archive in a scratch buffer: local entries + central dir + EOCD
        let mut out = self.buffer.clone();

        let central_dir_start = out.len() as u32;

        for (filename, local_header, local_offset) in &self.added_files {
            let mut cd_header = CentralDirectoryHeader::from_local(local_header, *local_offset);
            cd_header.filename = filename.clone();
            cd_header.write_to(&mut out)?;
        }

        let central_dir_end = out.len() as u32;

        // Write end of central directory record
        let eocd = EndOfCentralDirectory {
            signature: END_OF_CENTRAL_DIRECTORY_SIGNATURE,
            disk_number: 0,
            disk_number_for_central_dir: 0,
            num_central_dir_entries_on_disk: self.added_files.len() as u16,
            num_central_dir_entries: self.added_files.len() as u16,
            central_dir_length: central_dir_end - central_dir_start,
            central_dir_offset: central_dir_start,
            comment_length: 0,
            comment: Vec::new(),
        };
        eocd.write_to(&mut out)?;

        // Write the final archive; if this fails the writer state is unchanged
        let mut file = File::create(&self.file_path)?;
        file.write_all(&out)?;

        Ok(())
    }

    /// Discards the archive without saving.
    pub fn discard(self) {
        // Just drop self, buffer is discarded
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crc32() {
        // Test CRC32 implementation
        let data = b"Hello, World!";
        let crc = ZipFileWriter::crc32(data);
        // Known CRC32 for "Hello, World!"
        assert_eq!(crc, 0xEC4AC3D0);
    }

    #[test]
    fn test_sanitize_path() {
        assert_eq!(ZipFileWriter::sanitize_path("foo/bar.txt"), "foo/bar.txt");
        assert_eq!(ZipFileWriter::sanitize_path("/foo/bar.txt"), "foo/bar.txt");
        assert_eq!(
            ZipFileWriter::sanitize_path("C:/foo/bar.txt"),
            "foo/bar.txt"
        );
        assert_eq!(ZipFileWriter::sanitize_path("foo\\bar.txt"), "foo/bar.txt");
    }

    #[test]
    fn test_padding_calculation() {
        // At offset 0, no padding needed
        assert_eq!(ZipFileWriter::compute_padding_size(0), 0);
        // At offset 64, no padding needed
        assert_eq!(ZipFileWriter::compute_padding_size(64), 0);
        // At offset 1, need 63 bytes but that's less than header size (4)
        // So bump to 63 + 64 = ... actually let's verify
        let padding = ZipFileWriter::compute_padding_size(1);
        assert!(padding >= EXTRA_FIELD_HEADER_SIZE as u16);
        assert_eq!((1 + padding as usize) % DATA_ALIGNMENT, 0);
    }

    #[test]
    fn test_roundtrip_in_memory() {
        // Create a zip writer
        let temp_path = std::env::temp_dir().join("test_roundtrip.zip");
        let temp_path_str = temp_path.to_string_lossy().to_string();

        {
            let mut writer = ZipFileWriter::create_new(&temp_path_str);

            // Add files with USDZ-allowed extensions
            writer.add_file_data("test.usda", b"Hello, World!").unwrap();
            writer
                .add_file_data("subdir/file.usdc", b"Nested file")
                .unwrap();

            writer.save().unwrap();
        }

        // Read it back
        let zip = ZipFile::open(&temp_path_str).unwrap();
        assert_eq!(zip.len(), 2);

        let data1 = zip.get_file_data("test.usda").unwrap();
        assert_eq!(data1, b"Hello, World!");

        let data2 = zip.get_file_data("subdir/file.usdc").unwrap();
        assert_eq!(data2, b"Nested file");

        // Clean up
        std::fs::remove_file(&temp_path_str).ok();
    }

    #[test]
    fn test_first_file() {
        let temp_path = std::env::temp_dir().join("test_first.zip");
        let temp_path_str = temp_path.to_string_lossy().to_string();

        {
            let mut writer = ZipFileWriter::create_new(&temp_path_str);
            writer.add_file_data("first.usdc", b"USDC").unwrap();
            writer.add_file_data("second.png", b"PNG").unwrap();
            writer.save().unwrap();
        }

        let zip = ZipFile::open(&temp_path_str).unwrap();
        assert_eq!(zip.first_file(), Some("first.usdc"));

        std::fs::remove_file(&temp_path_str).ok();
    }

    #[test]
    fn test_reject_invalid_file_type() {
        let mut writer = ZipFileWriter::create_new("test.usdz");
        // .txt is not in the USDZ allowed extensions
        let result = writer.add_file_data("readme.txt", b"hello");
        assert!(result.is_err());
        assert!(
            result.unwrap_err().to_string().contains("not allowed"),
            "should mention disallowed type"
        );
    }

    #[test]
    fn test_accept_valid_file_types() {
        let mut writer = ZipFileWriter::create_new("test.usdz");
        assert!(writer.add_file_data("model.usdc", b"data").is_ok());
        assert!(writer.add_file_data("scene.usda", b"data").is_ok());
        assert!(writer.add_file_data("root.usd", b"data").is_ok());
        assert!(writer.add_file_data("tex.png", b"data").is_ok());
        assert!(writer.add_file_data("tex.jpg", b"data").is_ok());
        assert!(writer.add_file_data("tex.jpeg", b"data").is_ok());
        assert!(writer.add_file_data("tex.exr", b"data").is_ok());
        assert!(writer.add_file_data("nested.usdz", b"data").is_ok());
    }

    #[test]
    fn test_is_allowed_usdz_extension() {
        assert!(ZipFileWriter::is_allowed_usdz_extension("usdc"));
        assert!(ZipFileWriter::is_allowed_usdz_extension("USDC"));
        assert!(ZipFileWriter::is_allowed_usdz_extension("png"));
        assert!(ZipFileWriter::is_allowed_usdz_extension("JPG"));
        assert!(!ZipFileWriter::is_allowed_usdz_extension("txt"));
        assert!(!ZipFileWriter::is_allowed_usdz_extension("fbx"));
        assert!(!ZipFileWriter::is_allowed_usdz_extension("obj"));
    }

    /// P2-4: duplicate file should return Err
    #[test]
    fn test_duplicate_file_rejected() {
        let mut writer = ZipFileWriter::create_new("test_dup.usdz");
        assert!(writer.add_file_data("model.usdc", b"data1").is_ok());
        let res = writer.add_file_data("model.usdc", b"data2");
        assert!(res.is_err(), "duplicate should be rejected");
        assert!(
            res.unwrap_err().to_string().contains("Duplicate"),
            "error should mention Duplicate"
        );
    }

    /// P2-1: timestamps are non-zero (current time, not 1980-01-01)
    #[test]
    fn test_timestamps_non_zero() {
        let (time_val, date_val) = dos_timestamp_now();
        // Year field in DOS date = bits [15:9], must be >= 44 (2024-1980)
        let year_field = date_val >> 9;
        assert!(
            year_field >= 44,
            "DOS year should be >= 44 (year 2024+), got {}",
            year_field
        );
        let _ = time_val;
    }

    /// P2-1: dos_timestamp round-trips correctly for a known date
    #[test]
    fn test_dos_timestamp_decode() {
        use std::time::{Duration, UNIX_EPOCH};
        // 2026-02-27 12:30:44 UTC = unix timestamp 1772195444
        let ts = UNIX_EPOCH + Duration::from_secs(1_772_195_444);
        let (dos_time, dos_date) = dos_timestamp(ts);

        let year = (dos_date >> 9) + 1980;
        let month = (dos_date >> 5) & 0xF;
        let day = dos_date & 0x1F;
        let hours = dos_time >> 11;
        let minutes = (dos_time >> 5) & 0x3F;
        let seconds = (dos_time & 0x1F) * 2;

        assert_eq!(year, 2026, "year");
        assert_eq!(month, 2, "month");
        assert_eq!(day, 27, "day");
        assert_eq!(hours, 12, "hours");
        assert_eq!(minutes, 30, "minutes");
        // DOS time has 2-second resolution: 44s -> stored 22 -> decoded 44
        assert_eq!(seconds, 44, "seconds");
    }

    /// P2-3: save(&mut self) lets writer survive and continue after first save
    #[test]
    fn test_save_ref_mut_retry() {
        let temp_path = std::env::temp_dir().join("test_save_retry.usdz");
        let temp_str = temp_path.to_str().unwrap().to_string();

        let mut writer = ZipFileWriter::create_new(&temp_str);
        writer.add_file_data("model.usdc", b"data").unwrap();
        writer.save().unwrap();

        // Writer still alive — add more and save again
        writer.add_file_data("extra.usda", b"extra").unwrap();
        writer.save().unwrap();

        let zip = ZipFile::open(&temp_str).unwrap();
        assert_eq!(zip.len(), 2, "second save should include both files");

        std::fs::remove_file(&temp_str).ok();
    }
}
