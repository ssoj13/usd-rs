//! Filesystem-backed asset implementation.
//!
//! This module provides an `Asset` implementation for files on the filesystem.
//!
//! # Examples
//!
//! ```no_run
//! use usd_ar::{Asset, FilesystemAsset};
//! use std::path::Path;
//!
//! let path = Path::new("/path/to/asset.usd");
//! if let Some(asset) = FilesystemAsset::open(path) {
//!     println!("Asset size: {} bytes", asset.size());
//! }
//! ```

use std::fs::File;
use std::io::{Read as IoRead, Seek, SeekFrom};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use super::asset::{Asset, InMemoryAsset};
use super::resolved_path::ResolvedPath;
use super::timestamp::Timestamp;

/// An asset backed by a file on the filesystem.
///
/// This implementation reads from a file handle and supports concurrent
/// reads through internal locking.
///
/// # Thread Safety
///
/// `FilesystemAsset` uses a mutex internally to synchronize file access,
/// making it safe for concurrent reads from multiple threads.
///
/// # Examples
///
/// ```no_run
/// use usd_ar::{Asset, FilesystemAsset};
/// use std::path::Path;
///
/// let path = Path::new("model.usd");
/// if let Some(asset) = FilesystemAsset::open(path) {
///     let mut buffer = vec![0u8; 1024];
///     let bytes_read = asset.read(&mut buffer, 0);
///     println!("Read {} bytes", bytes_read);
/// }
/// ```
pub struct FilesystemAsset {
    /// The file handle wrapped in a mutex for thread-safe access.
    file: Mutex<File>,
    /// Cached file size.
    size: usize,
}

impl FilesystemAsset {
    /// Opens a file at the given path and returns a new `FilesystemAsset`.
    ///
    /// Returns `None` if the file could not be opened or its size could
    /// not be determined.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the file
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use usd_ar::FilesystemAsset;
    /// use std::path::Path;
    ///
    /// let asset = FilesystemAsset::open(Path::new("asset.usd"));
    /// ```
    pub fn open(path: impl AsRef<Path>) -> Option<Self> {
        let file = File::open(path.as_ref()).ok()?;
        let metadata = file.metadata().ok()?;
        let size = metadata.len() as usize;

        Some(Self {
            file: Mutex::new(file),
            size,
        })
    }

    /// Opens a file at the given resolved path.
    ///
    /// Returns `None` if the file could not be opened.
    ///
    /// # Arguments
    ///
    /// * `resolved_path` - The resolved path to the file
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use usd_ar::{FilesystemAsset, ResolvedPath};
    ///
    /// let resolved = ResolvedPath::new("/absolute/path/to/asset.usd");
    /// let asset = FilesystemAsset::open_resolved(&resolved);
    /// ```
    pub fn open_resolved(resolved_path: &ResolvedPath) -> Option<Self> {
        Self::open(resolved_path.as_str())
    }

    /// Creates a `FilesystemAsset` from an existing [`File`] handle.
    ///
    /// Matches C++ `ArFilesystemAsset(FILE* file)`. Takes ownership of the file;
    /// it will be closed when the asset is dropped.
    ///
    /// Returns `None` if the file size could not be determined.
    pub fn from_file(file: File) -> Option<Self> {
        let size = file.metadata().ok()?.len() as usize;
        Some(Self {
            file: Mutex::new(file),
            size,
        })
    }

    /// Returns the modification timestamp of the file at the given path.
    ///
    /// Returns `None` if the timestamp could not be retrieved.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the file
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use usd_ar::FilesystemAsset;
    /// use std::path::Path;
    ///
    /// if let Some(timestamp) = FilesystemAsset::get_modification_timestamp(Path::new("file.usd")) {
    ///     println!("File modified at: {:?}", timestamp);
    /// }
    /// ```
    pub fn get_modification_timestamp(path: impl AsRef<Path>) -> Option<Timestamp> {
        let metadata = std::fs::metadata(path.as_ref()).ok()?;
        let modified = metadata.modified().ok()?;
        let duration = modified.duration_since(SystemTime::UNIX_EPOCH).ok()?;
        Some(Timestamp::new(duration.as_secs_f64()))
    }

    /// Returns the modification timestamp for a resolved path.
    ///
    /// # Arguments
    ///
    /// * `resolved_path` - The resolved path to check
    pub fn get_modification_timestamp_resolved(resolved_path: &ResolvedPath) -> Option<Timestamp> {
        Self::get_modification_timestamp(resolved_path.as_str())
    }
}

impl Asset for FilesystemAsset {
    fn size(&self) -> usize {
        self.size
    }

    fn get_buffer(&self) -> Option<Arc<[u8]>> {
        let mut file = self.file.lock().ok()?;

        // Seek to beginning
        file.seek(SeekFrom::Start(0)).ok()?;

        // Read entire file
        let mut buffer = vec![0u8; self.size];
        file.read_exact(&mut buffer).ok()?;

        Some(Arc::from(buffer.into_boxed_slice()))
    }

    fn read(&self, buffer: &mut [u8], offset: usize) -> usize {
        if offset >= self.size {
            return 0;
        }

        let Ok(mut file) = self.file.lock() else {
            return 0;
        };

        // Seek to offset
        if file.seek(SeekFrom::Start(offset as u64)).is_err() {
            return 0;
        }

        // Read data
        file.read(buffer).unwrap_or_default()
    }

    fn get_file_unsafe(&self) -> Option<(super::asset::RawFileDescriptor, usize)> {
        // SAFETY: The raw fd/handle remains valid after the MutexGuard drops
        // because the File object itself lives inside the Mutex and is never
        // closed until the FilesystemAsset is dropped. The caller must use
        // pread-style access (read at offset, no seeking) to avoid conflicts
        // with concurrent reads through the Mutex-guarded API.
        // This matches C++ ArFilesystemAsset::GetFileUnsafe() semantics.
        let file = self.file.lock().ok()?;
        #[cfg(unix)]
        {
            use std::os::unix::io::AsRawFd;
            let fd = file.as_raw_fd();
            drop(file); // Explicitly drop guard; fd outlives it
            Some((fd, 0))
        }
        #[cfg(windows)]
        {
            use std::os::windows::io::AsRawHandle;
            let handle = file.as_raw_handle();
            drop(file); // Explicitly drop guard; handle outlives it
            Some((handle, 0))
        }
    }

    fn get_detached(&self) -> Option<Arc<dyn Asset>> {
        let buffer = self.get_buffer()?;
        Some(Arc::new(InMemoryAsset::new(buffer)) as Arc<dyn Asset>)
    }
}

impl std::fmt::Debug for FilesystemAsset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FilesystemAsset")
            .field("size", &self.size)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn create_temp_file(content: &[u8]) -> NamedTempFile {
        let mut file = NamedTempFile::new().expect("failed to create temp file");
        file.write_all(content).expect("failed to write");
        file.flush().expect("failed to flush");
        file
    }

    #[test]
    fn test_open() {
        let temp = create_temp_file(b"Hello, World!");
        let asset = FilesystemAsset::open(temp.path()).expect("should open");
        assert_eq!(asset.size(), 13);
    }

    #[test]
    fn test_open_nonexistent() {
        let asset = FilesystemAsset::open("/nonexistent/path/to/file.usd");
        assert!(asset.is_none());
    }

    #[test]
    fn test_read() {
        let temp = create_temp_file(b"Hello, World!");
        let asset = FilesystemAsset::open(temp.path()).expect("should open");

        let mut buffer = vec![0u8; 5];
        let read = asset.read(&mut buffer, 0);
        assert_eq!(read, 5);
        assert_eq!(&buffer, b"Hello");
    }

    #[test]
    fn test_read_at_offset() {
        let temp = create_temp_file(b"Hello, World!");
        let asset = FilesystemAsset::open(temp.path()).expect("should open");

        let mut buffer = vec![0u8; 5];
        let read = asset.read(&mut buffer, 7);
        assert_eq!(read, 5);
        assert_eq!(&buffer, b"World");
    }

    #[test]
    fn test_read_past_end() {
        let temp = create_temp_file(b"Hi");
        let asset = FilesystemAsset::open(temp.path()).expect("should open");

        let mut buffer = vec![0u8; 5];
        let read = asset.read(&mut buffer, 0);
        assert_eq!(read, 2);
    }

    #[test]
    fn test_read_out_of_bounds() {
        let temp = create_temp_file(b"Hello");
        let asset = FilesystemAsset::open(temp.path()).expect("should open");

        let mut buffer = vec![0u8; 5];
        let read = asset.read(&mut buffer, 100);
        assert_eq!(read, 0);
    }

    #[test]
    fn test_get_buffer() {
        let temp = create_temp_file(b"Test data");
        let asset = FilesystemAsset::open(temp.path()).expect("should open");

        let buffer = asset.get_buffer().expect("should get buffer");
        assert_eq!(&*buffer, b"Test data");
    }

    #[test]
    fn test_get_detached() {
        let temp = create_temp_file(b"Detach me");
        let asset = FilesystemAsset::open(temp.path()).expect("should open");

        let detached = asset.get_detached().expect("should detach");
        assert_eq!(detached.size(), 9);

        // Verify detached content
        let mut buffer = vec![0u8; 9];
        let read = detached.read(&mut buffer, 0);
        assert_eq!(read, 9);
        assert_eq!(&buffer, b"Detach me");
    }

    #[test]
    fn test_modification_timestamp() {
        let temp = create_temp_file(b"timestamp test");
        let timestamp = FilesystemAsset::get_modification_timestamp(temp.path());
        assert!(timestamp.is_some());
        assert!(timestamp.expect("should have timestamp").is_valid());
    }

    #[test]
    fn test_modification_timestamp_nonexistent() {
        let timestamp = FilesystemAsset::get_modification_timestamp("/nonexistent/file");
        assert!(timestamp.is_none());
    }

    #[test]
    fn test_debug() {
        let temp = create_temp_file(b"debug");
        let asset = FilesystemAsset::open(temp.path()).expect("should open");
        let debug = format!("{:?}", asset);
        assert!(debug.contains("FilesystemAsset"));
        assert!(debug.contains("5"));
    }

    #[test]
    fn test_concurrent_reads() {
        use std::sync::Arc;
        use std::thread;

        let temp = create_temp_file(b"0123456789ABCDEF");
        let asset = Arc::new(FilesystemAsset::open(temp.path()).expect("should open"));

        let handles: Vec<_> = (0..4)
            .map(|i| {
                let asset = Arc::clone(&asset);
                thread::spawn(move || {
                    let mut buffer = vec![0u8; 4];
                    let read = asset.read(&mut buffer, i * 4);
                    (read, buffer)
                })
            })
            .collect();

        let results: Vec<_> = handles
            .into_iter()
            .map(|h| h.join().expect("thread panic"))
            .collect();

        assert_eq!(results[0], (4, b"0123".to_vec()));
        assert_eq!(results[1], (4, b"4567".to_vec()));
        assert_eq!(results[2], (4, b"89AB".to_vec()));
        assert_eq!(results[3], (4, b"CDEF".to_vec()));
    }
}
