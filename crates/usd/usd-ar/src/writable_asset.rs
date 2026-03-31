//! Writable asset interface for writing asset contents.
//!
//! The `WritableAsset` trait defines the interface for writing data
//! to an asset.

use std::fs::{File, OpenOptions};
use std::io::{self, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use super::resolved_path::ResolvedPath;

/// Interface for writing data to an asset.
///
/// This trait defines the methods for writing data to a resolved asset.
/// Implementations may write to files, memory, archives, or other destinations.
///
/// # Write Modes
///
/// Assets can be opened in two modes:
/// - `Update`: Existing content is preserved, writes may overwrite existing data
/// - `Replace`: Existing content is discarded, asset is written fresh
pub trait WritableAsset: Send + Sync {
    /// Closes this asset, performing any necessary finalization.
    ///
    /// Returns `true` on success, `false` otherwise.
    ///
    /// After a successful close, reads to the written asset should reflect
    /// the fully written state. Further calls to any methods on this
    /// interface are invalid after close.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let mut asset = resolver.open_for_write(&path, WriteMode::Replace)?;
    /// asset.write(b"Hello", 0);
    /// asset.close();
    /// ```
    fn close(&mut self) -> bool;

    /// Writes `buffer` at `offset` from the beginning of the asset.
    ///
    /// Returns the number of bytes written, or 0 on error.
    ///
    /// # Arguments
    ///
    /// * `buffer` - The data to write
    /// * `offset` - The offset from the beginning of the asset
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let mut asset = resolver.open_for_write(&path, WriteMode::Replace)?;
    /// let written = asset.write(b"Hello, World!", 0);
    /// assert_eq!(written, 13);
    /// ```
    fn write(&mut self, buffer: &[u8], offset: usize) -> usize;
}

/// Write mode for opening assets for writing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum WriteMode {
    /// Open asset for in-place updates.
    ///
    /// If the asset exists, its contents will not be discarded and writes
    /// may overwrite existing data. Otherwise, the asset will be created.
    Update,

    /// Open asset for replacement.
    ///
    /// If the asset exists, its contents will be discarded by the time
    /// the `WritableAsset` is closed. Otherwise, the asset will be created.
    #[default]
    Replace,
}

impl std::fmt::Display for WriteMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WriteMode::Update => write!(f, "Update"),
            WriteMode::Replace => write!(f, "Replace"),
        }
    }
}

/// An in-memory writable asset for testing and buffering.
///
/// # Examples
///
/// ```
/// use usd_ar::{WritableAsset, InMemoryWritableAsset};
///
/// let mut asset = InMemoryWritableAsset::new();
/// asset.write(b"Hello", 0);
/// asset.write(b", World!", 5);
///
/// assert_eq!(asset.as_bytes(), b"Hello, World!");
/// ```
pub struct InMemoryWritableAsset {
    /// The asset data.
    data: Vec<u8>,
    /// Whether the asset has been closed.
    closed: bool,
}

impl InMemoryWritableAsset {
    /// Creates a new empty in-memory writable asset.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_ar::InMemoryWritableAsset;
    ///
    /// let asset = InMemoryWritableAsset::new();
    /// assert_eq!(asset.len(), 0);
    /// ```
    pub fn new() -> Self {
        Self {
            data: Vec::new(),
            closed: false,
        }
    }

    /// Creates a new in-memory writable asset with initial capacity.
    ///
    /// # Arguments
    ///
    /// * `capacity` - The initial capacity in bytes
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_ar::InMemoryWritableAsset;
    ///
    /// let asset = InMemoryWritableAsset::with_capacity(1024);
    /// assert!(asset.capacity() >= 1024);
    /// ```
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            data: Vec::with_capacity(capacity),
            closed: false,
        }
    }

    /// Creates a new in-memory writable asset from existing data.
    ///
    /// This is useful for update mode where existing content is preserved.
    ///
    /// # Arguments
    ///
    /// * `data` - The initial data
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_ar::InMemoryWritableAsset;
    ///
    /// let asset = InMemoryWritableAsset::from_vec(b"existing".to_vec());
    /// assert_eq!(asset.as_bytes(), b"existing");
    /// ```
    pub fn from_vec(data: Vec<u8>) -> Self {
        Self {
            data,
            closed: false,
        }
    }

    /// Returns the current length of the asset data.
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Returns `true` if the asset data is empty.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Returns the current capacity of the asset buffer.
    pub fn capacity(&self) -> usize {
        self.data.capacity()
    }

    /// Returns a reference to the asset data.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_ar::{WritableAsset, InMemoryWritableAsset};
    ///
    /// let mut asset = InMemoryWritableAsset::new();
    /// asset.write(b"Hello", 0);
    /// assert_eq!(asset.as_bytes(), b"Hello");
    /// ```
    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }

    /// Consumes the asset and returns the underlying data.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_ar::{WritableAsset, InMemoryWritableAsset};
    ///
    /// let mut asset = InMemoryWritableAsset::new();
    /// asset.write(b"Hello", 0);
    /// let data = asset.into_vec();
    /// assert_eq!(data, b"Hello");
    /// ```
    pub fn into_vec(self) -> Vec<u8> {
        self.data
    }

    /// Returns whether the asset has been closed.
    pub fn is_closed(&self) -> bool {
        self.closed
    }
}

impl Default for InMemoryWritableAsset {
    fn default() -> Self {
        Self::new()
    }
}

impl WritableAsset for InMemoryWritableAsset {
    fn close(&mut self) -> bool {
        self.closed = true;
        true
    }

    fn write(&mut self, buffer: &[u8], offset: usize) -> usize {
        if self.closed {
            return 0;
        }

        let end = offset + buffer.len();

        // Extend the data vector if necessary
        if end > self.data.len() {
            self.data.resize(end, 0);
        }

        // Copy the data
        self.data[offset..end].copy_from_slice(buffer);
        buffer.len()
    }
}

impl std::fmt::Debug for InMemoryWritableAsset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InMemoryWritableAsset")
            .field("len", &self.data.len())
            .field("closed", &self.closed)
            .finish()
    }
}

/// A wrapper that provides `Write` and `Seek` implementations for a `WritableAsset`.
///
/// This allows writable assets to be used with standard I/O interfaces.
///
/// # Examples
///
/// ```
/// use usd_ar::{WritableAsset, WritableAssetWriter, InMemoryWritableAsset};
/// use std::io::Write;
/// use std::sync::{Arc, Mutex};
///
/// let asset = Arc::new(Mutex::new(InMemoryWritableAsset::new()));
/// let mut writer = WritableAssetWriter::new(asset.clone());
///
/// write!(writer, "Hello, World!").unwrap();
///
/// let asset = asset.lock().expect("lock poisoned");
/// assert_eq!(asset.as_bytes(), b"Hello, World!");
/// ```
pub struct WritableAssetWriter<A: WritableAsset> {
    /// The underlying writable asset.
    asset: Arc<Mutex<A>>,
    /// Current write position.
    position: usize,
    /// Total size (for seek operations).
    size: usize,
}

impl<A: WritableAsset> WritableAssetWriter<A> {
    /// Creates a new `WritableAssetWriter` for the given asset.
    ///
    /// # Arguments
    ///
    /// * `asset` - The writable asset to write to
    pub fn new(asset: Arc<Mutex<A>>) -> Self {
        Self {
            asset,
            position: 0,
            size: 0,
        }
    }

    /// Returns the current write position.
    pub fn position(&self) -> usize {
        self.position
    }

    /// Returns the current size (highest written offset).
    pub fn size(&self) -> usize {
        self.size
    }
}

impl<A: WritableAsset> Write for WritableAssetWriter<A> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut asset = self
            .asset
            .lock()
            .map_err(|_| io::Error::other("Failed to acquire lock"))?;

        let written = asset.write(buf, self.position);
        self.position += written;
        self.size = self.size.max(self.position);

        if written == 0 && !buf.is_empty() {
            Err(io::Error::new(io::ErrorKind::WriteZero, "Write returned 0"))
        } else {
            Ok(written)
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        // No-op for most implementations
        Ok(())
    }
}

impl<A: WritableAsset> Seek for WritableAssetWriter<A> {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let new_pos = match pos {
            SeekFrom::Start(offset) => offset as i64,
            SeekFrom::End(offset) => self.size as i64 + offset,
            SeekFrom::Current(offset) => self.position as i64 + offset,
        };

        if new_pos < 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Seek to negative position",
            ));
        }

        self.position = new_pos as usize;
        Ok(self.position as u64)
    }
}

/// A filesystem-based writable asset using files on disk.
///
/// This implementation provides safe file writing with two modes:
/// - `Update`: Opens existing file for in-place updates
/// - `Replace`: Uses a temporary file and atomically renames on close
///
/// The Replace mode provides crash-safe writes by writing to a temporary
/// file first, then atomically moving it to the final location on successful
/// close.
///
/// # Examples
///
/// ```ignore
/// use usd_ar::{FilesystemWritableAsset, ResolvedPath, WriteMode};
///
/// let path = ResolvedPath::new("/path/to/asset.usd");
/// let asset = FilesystemWritableAsset::create(&path, WriteMode::Replace)?;
///
/// let mut asset = asset.lock().expect("lock poisoned");
/// asset.write(b"#usda 1.0\n", 0);
/// asset.close();
/// ```
pub struct FilesystemWritableAsset {
    /// The target file path
    target_path: PathBuf,
    /// The actual file being written (may be temp file)
    file: Option<File>,
    /// Temporary file path (for Replace mode)
    temp_path: Option<PathBuf>,
    /// Write mode
    mode: WriteMode,
    /// Whether the asset has been closed
    closed: bool,
}

impl FilesystemWritableAsset {
    /// Creates a new filesystem writable asset.
    ///
    /// # Arguments
    ///
    /// * `resolved_path` - The resolved path where the asset should be written
    /// * `write_mode` - The write mode (Update or Replace)
    ///
    /// # Returns
    ///
    /// A thread-safe writable asset wrapped in Arc<Mutex>, or None on error
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_ar::{FilesystemWritableAsset, ResolvedPath, WriteMode};
    ///
    /// let path = ResolvedPath::new("/path/to/asset.usd");
    /// if let Some(asset) = FilesystemWritableAsset::create(&path, WriteMode::Replace) {
    ///     let mut asset = asset.lock().expect("lock poisoned");
    ///     asset.write(b"data", 0);
    ///     asset.close();
    /// }
    /// ```
    pub fn create(resolved_path: &ResolvedPath, write_mode: WriteMode) -> Option<Arc<Mutex<Self>>> {
        let target_path = PathBuf::from(resolved_path.as_str());

        // Create parent directory if it doesn't exist
        if let Some(parent) = target_path.parent() {
            if !parent.exists() {
                if let Err(e) = std::fs::create_dir_all(parent) {
                    eprintln!(
                        "Could not create directory '{}' for asset '{}': {}",
                        parent.display(),
                        target_path.display(),
                        e
                    );
                    return None;
                }
            }
        }

        match write_mode {
            WriteMode::Update => {
                // Open file for read/write, create if doesn't exist
                // truncate(false) preserves existing contents for update mode
                let file = OpenOptions::new()
                    .read(true)
                    .write(true)
                    .create(true)
                    .truncate(false)
                    .open(&target_path)
                    .ok()?;

                Some(Arc::new(Mutex::new(Self {
                    target_path,
                    file: Some(file),
                    temp_path: None,
                    mode: write_mode,
                    closed: false,
                })))
            }
            WriteMode::Replace => {
                // Create temporary file in the same directory
                let temp_path = Self::create_temp_path(&target_path)?;

                let file = OpenOptions::new()
                    .write(true)
                    .create(true)
                    .truncate(true)
                    .open(&temp_path)
                    .ok()?;

                Some(Arc::new(Mutex::new(Self {
                    target_path,
                    file: Some(file),
                    temp_path: Some(temp_path),
                    mode: write_mode,
                    closed: false,
                })))
            }
        }
    }

    /// Creates a temporary file path in the same directory as the target.
    fn create_temp_path(target: &Path) -> Option<PathBuf> {
        let parent = target.parent().unwrap_or_else(|| Path::new("."));
        let filename = target.file_name()?.to_str()?;

        // Create temp filename: original.tmp.RANDOM
        use std::time::{SystemTime, UNIX_EPOCH};
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()?
            .as_micros();

        let temp_name = format!("{}.tmp.{}", filename, timestamp);
        Some(parent.join(temp_name))
    }

    /// Returns the target file path.
    pub fn target_path(&self) -> &Path {
        &self.target_path
    }

    /// Returns the current temporary file path (if in Replace mode).
    pub fn temp_path(&self) -> Option<&Path> {
        self.temp_path.as_deref()
    }

    /// Returns the write mode.
    pub fn write_mode(&self) -> WriteMode {
        self.mode
    }

    /// Returns whether the asset has been closed.
    pub fn is_closed(&self) -> bool {
        self.closed
    }
}

impl Drop for FilesystemWritableAsset {
    fn drop(&mut self) {
        if !self.closed {
            // Auto-close on drop
            let _ = self.close();
        }

        // Clean up temp file if it exists and wasn't properly moved
        if let Some(temp_path) = &self.temp_path {
            if temp_path.exists() {
                let _ = std::fs::remove_file(temp_path);
            }
        }
    }
}

impl WritableAsset for FilesystemWritableAsset {
    fn close(&mut self) -> bool {
        if self.closed {
            return true;
        }

        // Close the file handle
        self.file = None;

        // In Replace mode, atomically rename temp file to target
        if let Some(temp_path) = &self.temp_path {
            if let Err(e) = std::fs::rename(temp_path, &self.target_path) {
                eprintln!(
                    "Failed to rename '{}' to '{}': {}",
                    temp_path.display(),
                    self.target_path.display(),
                    e
                );
                return false;
            }
        }

        self.closed = true;
        true
    }

    fn write(&mut self, buffer: &[u8], offset: usize) -> usize {
        if self.closed {
            return 0;
        }

        let file = match &mut self.file {
            Some(f) => f,
            None => return 0,
        };

        // Seek to the desired offset
        if let Err(e) = file.seek(SeekFrom::Start(offset as u64)) {
            eprintln!("Seek failed: {}", e);
            return 0;
        }

        // Write the data
        match file.write(buffer) {
            Ok(written) => written,
            Err(e) => {
                eprintln!("Write failed: {}", e);
                0
            }
        }
    }
}

impl std::fmt::Debug for FilesystemWritableAsset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FilesystemWritableAsset")
            .field("target_path", &self.target_path)
            .field("mode", &self.mode)
            .field("closed", &self.closed)
            .field("has_temp", &self.temp_path.is_some())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_write_mode_default() {
        assert_eq!(WriteMode::default(), WriteMode::Replace);
    }

    #[test]
    fn test_write_mode_display() {
        assert_eq!(format!("{}", WriteMode::Update), "Update");
        assert_eq!(format!("{}", WriteMode::Replace), "Replace");
    }

    #[test]
    fn test_in_memory_writable_new() {
        let asset = InMemoryWritableAsset::new();
        assert_eq!(asset.len(), 0);
        assert!(asset.is_empty());
        assert!(!asset.is_closed());
    }

    #[test]
    fn test_in_memory_writable_with_capacity() {
        let asset = InMemoryWritableAsset::with_capacity(1024);
        assert!(asset.capacity() >= 1024);
    }

    #[test]
    fn test_in_memory_writable_from_vec() {
        let asset = InMemoryWritableAsset::from_vec(b"existing".to_vec());
        assert_eq!(asset.as_bytes(), b"existing");
        assert_eq!(asset.len(), 8);
    }

    #[test]
    fn test_in_memory_writable_write() {
        let mut asset = InMemoryWritableAsset::new();

        let written = asset.write(b"Hello", 0);
        assert_eq!(written, 5);
        assert_eq!(asset.as_bytes(), b"Hello");

        let written = asset.write(b", World!", 5);
        assert_eq!(written, 8);
        assert_eq!(asset.as_bytes(), b"Hello, World!");
    }

    #[test]
    fn test_in_memory_writable_write_with_gap() {
        let mut asset = InMemoryWritableAsset::new();

        // Write at offset 5, should fill with zeros
        let written = asset.write(b"World", 5);
        assert_eq!(written, 5);
        assert_eq!(asset.len(), 10);
        assert_eq!(&asset.as_bytes()[..5], &[0, 0, 0, 0, 0]);
        assert_eq!(&asset.as_bytes()[5..], b"World");
    }

    #[test]
    fn test_in_memory_writable_overwrite() {
        let mut asset = InMemoryWritableAsset::from_vec(b"Hello, World!".to_vec());

        // Overwrite middle portion
        let written = asset.write(b"Beautiful", 7);
        assert_eq!(written, 9);
        assert_eq!(asset.as_bytes(), b"Hello, Beautiful");
    }

    #[test]
    fn test_in_memory_writable_close() {
        let mut asset = InMemoryWritableAsset::new();
        asset.write(b"Hello", 0);

        assert!(!asset.is_closed());
        assert!(asset.close());
        assert!(asset.is_closed());

        // Write after close should return 0
        let written = asset.write(b"World", 5);
        assert_eq!(written, 0);
    }

    #[test]
    fn test_in_memory_writable_into_vec() {
        let mut asset = InMemoryWritableAsset::new();
        asset.write(b"Hello", 0);

        let data = asset.into_vec();
        assert_eq!(data, b"Hello");
    }

    #[test]
    fn test_in_memory_writable_debug() {
        let asset = InMemoryWritableAsset::from_vec(b"test".to_vec());
        let debug = format!("{:?}", asset);
        assert!(debug.contains("InMemoryWritableAsset"));
        assert!(debug.contains("4")); // len
    }

    #[test]
    fn test_writable_asset_writer_write() {
        let asset = Arc::new(Mutex::new(InMemoryWritableAsset::new()));
        let mut writer = WritableAssetWriter::new(asset.clone());

        let written = writer.write(b"Hello").expect("should write");
        assert_eq!(written, 5);
        assert_eq!(writer.position(), 5);

        let asset = asset.lock().expect("should lock");
        assert_eq!(asset.as_bytes(), b"Hello");
    }

    #[test]
    fn test_writable_asset_writer_write_all() {
        use std::io::Write;

        let asset = Arc::new(Mutex::new(InMemoryWritableAsset::new()));
        let mut writer = WritableAssetWriter::new(asset.clone());

        writer
            .write_all(b"Hello, World!")
            .expect("should write all");

        let asset = asset.lock().expect("should lock");
        assert_eq!(asset.as_bytes(), b"Hello, World!");
    }

    #[test]
    fn test_writable_asset_writer_write_fmt() {
        use std::io::Write;

        let asset = Arc::new(Mutex::new(InMemoryWritableAsset::new()));
        let mut writer = WritableAssetWriter::new(asset.clone());

        write!(writer, "Value: {}", 42).expect("should write");

        let asset = asset.lock().expect("should lock");
        assert_eq!(asset.as_bytes(), b"Value: 42");
    }

    #[test]
    fn test_writable_asset_writer_seek() {
        let asset = Arc::new(Mutex::new(InMemoryWritableAsset::new()));
        let mut writer = WritableAssetWriter::new(asset.clone());

        writer.write(b"Hello").expect("should write");
        writer.seek(SeekFrom::Start(0)).expect("should seek");
        assert_eq!(writer.position(), 0);

        writer.write(b"World").expect("should write");

        let asset = asset.lock().expect("should lock");
        assert_eq!(asset.as_bytes(), b"World");
    }

    #[test]
    fn test_writable_asset_writer_seek_current() {
        let asset = Arc::new(Mutex::new(InMemoryWritableAsset::new()));
        let mut writer = WritableAssetWriter::new(asset);

        writer.write(b"Hello").expect("should write");
        writer.seek(SeekFrom::Current(-3)).expect("should seek");
        assert_eq!(writer.position(), 2);
    }

    #[test]
    fn test_writable_asset_writer_seek_end() {
        let asset = Arc::new(Mutex::new(InMemoryWritableAsset::new()));
        let mut writer = WritableAssetWriter::new(asset);

        writer.write(b"Hello").expect("should write");
        writer.seek(SeekFrom::End(-2)).expect("should seek");
        assert_eq!(writer.position(), 3);
    }

    #[test]
    fn test_filesystem_writable_create_replace() {
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test_fs_writable_replace.txt");
        let _ = std::fs::remove_file(&test_file);

        let path = ResolvedPath::new(test_file.to_str().unwrap());
        let asset = FilesystemWritableAsset::create(&path, WriteMode::Replace)
            .expect("should create asset");

        {
            let mut asset = asset.lock().expect("lock poisoned");
            assert!(!asset.is_closed());
            assert_eq!(asset.write_mode(), WriteMode::Replace);
            assert!(asset.temp_path().is_some());

            let written = asset.write(b"Hello, World!", 0);
            assert_eq!(written, 13);

            assert!(asset.close());
            assert!(asset.is_closed());
        }

        // Verify file was created and contains data
        let content = std::fs::read_to_string(&test_file).expect("should read file");
        assert_eq!(content, "Hello, World!");

        let _ = std::fs::remove_file(&test_file);
    }

    #[test]
    fn test_filesystem_writable_create_update() {
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test_fs_writable_update.txt");
        let _ = std::fs::remove_file(&test_file);

        // Create initial file
        std::fs::write(&test_file, "Original Content").expect("should write");

        let path = ResolvedPath::new(test_file.to_str().unwrap());
        let asset =
            FilesystemWritableAsset::create(&path, WriteMode::Update).expect("should create asset");

        {
            let mut asset = asset.lock().expect("lock poisoned");
            assert_eq!(asset.write_mode(), WriteMode::Update);
            assert!(asset.temp_path().is_none());

            // Overwrite beginning
            let written = asset.write(b"Modified", 0);
            assert_eq!(written, 8);

            assert!(asset.close());
        }

        // Verify file was updated
        let content = std::fs::read_to_string(&test_file).expect("should read file");
        assert_eq!(content, "Modified Content");

        let _ = std::fs::remove_file(&test_file);
    }

    #[test]
    fn test_filesystem_writable_write_at_offset() {
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test_fs_writable_offset.txt");
        let _ = std::fs::remove_file(&test_file);

        let path = ResolvedPath::new(test_file.to_str().unwrap());
        let asset = FilesystemWritableAsset::create(&path, WriteMode::Replace)
            .expect("should create asset");

        {
            let mut asset = asset.lock().expect("lock poisoned");

            asset.write(b"Hello", 0);
            asset.write(b"World", 7);

            assert!(asset.close());
        }

        // File should have: "Hello" + 2 bytes gap (undefined) + "World"
        let content = std::fs::read(&test_file).expect("should read file");
        assert!(content.len() >= 12);
        assert_eq!(&content[0..5], b"Hello");
        assert_eq!(&content[7..12], b"World");

        let _ = std::fs::remove_file(&test_file);
    }

    #[test]
    fn test_filesystem_writable_auto_close_on_drop() {
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test_fs_writable_drop.txt");
        let _ = std::fs::remove_file(&test_file);

        let path = ResolvedPath::new(test_file.to_str().unwrap());

        {
            let asset = FilesystemWritableAsset::create(&path, WriteMode::Replace)
                .expect("should create asset");

            let mut asset = asset.lock().expect("lock poisoned");
            asset.write(b"Auto Close Test", 0);
            // Drop without explicit close
        }

        // File should still be created due to auto-close in Drop
        let content = std::fs::read_to_string(&test_file).expect("should read file");
        assert_eq!(content, "Auto Close Test");

        let _ = std::fs::remove_file(&test_file);
    }

    #[test]
    fn test_filesystem_writable_debug() {
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test_fs_writable_debug.txt");
        let path = ResolvedPath::new(test_file.to_str().unwrap());

        let asset = FilesystemWritableAsset::create(&path, WriteMode::Replace)
            .expect("should create asset");

        let asset = asset.lock().expect("lock poisoned");
        let debug = format!("{:?}", *asset);
        assert!(debug.contains("FilesystemWritableAsset"));
        assert!(debug.contains("Replace"));

        let _ = std::fs::remove_file(&test_file);
    }
}
