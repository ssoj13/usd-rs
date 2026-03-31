//! Asset interface for reading asset contents.
//!
//! The `Asset` trait defines the interface for accessing the contents
//! of a resolved asset.

use std::io::{self, Read, Seek, SeekFrom};
use std::sync::Arc;

/// Platform-specific raw file descriptor type.
///
/// On Unix, this is `std::os::unix::io::RawFd` (i32).
/// On Windows, this is `std::os::windows::io::RawHandle` (*mut c_void).
///
/// Matches C++ `FILE*` semantics in `ArAsset::GetFileUnsafe()`.
#[cfg(unix)]
pub type RawFileDescriptor = std::os::unix::io::RawFd;
/// Platform-specific raw file descriptor type.
#[cfg(windows)]
pub type RawFileDescriptor = std::os::windows::io::RawHandle;

/// Interface for accessing the contents of an asset.
///
/// This trait defines the methods for reading data from a resolved asset.
/// Implementations may read from files, memory, archives, or other sources.
///
/// # Thread Safety
///
/// Asset implementations must be thread-safe for concurrent reads.
/// The `Read` method can be called from multiple threads simultaneously.
pub trait Asset: Send + Sync {
    /// Returns the total size of the asset in bytes.
    ///
    /// Matches C++ `ArAsset::GetSize()`.
    fn size(&self) -> usize;

    /// Returns a buffer containing the entire contents of the asset.
    ///
    /// Returns `None` if the contents could not be retrieved.
    /// The returned data must remain valid for the lifetime of the returned `Arc`.
    ///
    /// Matches C++ `ArAsset::GetBuffer()`.
    fn get_buffer(&self) -> Option<Arc<[u8]>>;

    /// Reads `count` bytes at `offset` from the beginning of the asset.
    ///
    /// Returns the number of bytes read, or 0 on error.
    /// Out-of-bounds reads should return 0.
    ///
    /// Matches C++ `ArAsset::Read(buffer, count, offset)`.
    fn read(&self, buffer: &mut [u8], offset: usize) -> usize;

    /// Returns the underlying raw file descriptor and offset for direct I/O.
    ///
    /// Returns `Some((fd, offset))` where `fd` is a platform raw file descriptor
    /// (on Unix: `RawFd`, on Windows: `RawHandle`) and `offset` is the byte
    /// offset within the file where this asset's data begins.
    /// Returns `None` if the asset is not backed by a file.
    ///
    /// # Safety
    ///
    /// The returned file descriptor must NOT be used with non-concurrent
    /// functions (read, fread, fseek). Use pread-style access only.
    /// The caller must not close the file descriptor.
    ///
    /// Matches C++ `ArAsset::GetFileUnsafe()` which returns
    /// `std::pair<FILE*, size_t>`.
    fn get_file_unsafe(&self) -> Option<(RawFileDescriptor, usize)> {
        None
    }

    /// Returns a detached copy of this asset.
    ///
    /// The returned asset's contents are independent of the original
    /// asset's serialized data. External changes to the original asset
    /// must not affect the detached copy.
    ///
    /// Matches C++ `ArAsset::GetDetachedAsset()`. Reads the entire asset
    /// via [`read`](Asset::read) if [`get_buffer`](Asset::get_buffer) returns `None`.
    fn get_detached(&self) -> Option<Arc<dyn Asset>>
    where
        Self: Sized,
    {
        InMemoryAsset::from_asset(self).map(|a| Arc::new(a) as Arc<dyn Asset>)
    }
}

/// An asset backed by in-memory data.
///
/// This is the default implementation used for detached assets.
///
/// # Examples
///
/// ```
/// use usd_ar::{Asset, InMemoryAsset};
/// use std::sync::Arc;
///
/// let data = vec![1, 2, 3, 4, 5];
/// let asset = InMemoryAsset::from_vec(data);
///
/// assert_eq!(asset.size(), 5);
/// ```
pub struct InMemoryAsset {
    /// The asset data.
    data: Arc<[u8]>,
}

impl InMemoryAsset {
    /// Creates an in-memory asset by reading the entire contents of `src_asset`.
    ///
    /// Matches C++ `ArInMemoryAsset::FromAsset`. Uses [`Asset::read`] to load data
    /// when [`Asset::get_buffer`] returns `None`.
    ///
    /// Returns `None` if allocation fails or read returns fewer bytes than expected.
    pub fn from_asset(src_asset: &dyn Asset) -> Option<Self> {
        let size = src_asset.size();
        if let Some(buffer) = src_asset.get_buffer() {
            return Some(Self::new(buffer));
        }
        let mut data = vec![0u8; size];
        let mut offset = 0;
        while offset < size {
            let n = src_asset.read(&mut data[offset..], offset);
            if n == 0 {
                return None;
            }
            offset += n;
        }
        Some(Self {
            data: Arc::from(data.into_boxed_slice()),
        })
    }

    /// Creates a new in-memory asset from an `Arc<[u8]>`.
    ///
    /// # Arguments
    ///
    /// * `data` - The asset data
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_ar::{Asset, InMemoryAsset};
    /// use std::sync::Arc;
    ///
    /// let data: Arc<[u8]> = Arc::from(vec![1, 2, 3, 4, 5].into_boxed_slice());
    /// let asset = InMemoryAsset::new(data);
    /// assert_eq!(asset.size(), 5);
    /// ```
    pub fn new(data: Arc<[u8]>) -> Self {
        Self { data }
    }

    /// Creates a new in-memory asset from a `Vec<u8>`.
    ///
    /// # Arguments
    ///
    /// * `data` - The asset data as a vector
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_ar::{Asset, InMemoryAsset};
    ///
    /// let asset = InMemoryAsset::from_vec(vec![1, 2, 3, 4, 5]);
    /// assert_eq!(asset.size(), 5);
    /// ```
    pub fn from_vec(data: Vec<u8>) -> Self {
        Self {
            data: Arc::from(data.into_boxed_slice()),
        }
    }

    /// Creates an empty in-memory asset.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_ar::{Asset, InMemoryAsset};
    ///
    /// let asset = InMemoryAsset::empty();
    /// assert_eq!(asset.size(), 0);
    /// ```
    pub fn empty() -> Self {
        Self {
            data: Arc::from(Vec::new().into_boxed_slice()),
        }
    }

    /// Returns a reference to the underlying data.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_ar::InMemoryAsset;
    ///
    /// let asset = InMemoryAsset::from_vec(vec![1, 2, 3]);
    /// assert_eq!(asset.as_bytes(), &[1, 2, 3]);
    /// ```
    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }
}

impl Asset for InMemoryAsset {
    fn size(&self) -> usize {
        self.data.len()
    }

    fn get_buffer(&self) -> Option<Arc<[u8]>> {
        Some(Arc::clone(&self.data))
    }

    fn read(&self, buffer: &mut [u8], offset: usize) -> usize {
        if offset >= self.data.len() {
            return 0;
        }

        let available = self.data.len() - offset;
        let to_read = buffer.len().min(available);
        buffer[..to_read].copy_from_slice(&self.data[offset..offset + to_read]);
        to_read
    }

    fn get_detached(&self) -> Option<Arc<dyn Asset>> {
        // Already in memory, just clone the Arc
        Some(Arc::new(Self {
            data: Arc::clone(&self.data),
        }))
    }
}

impl std::fmt::Debug for InMemoryAsset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InMemoryAsset")
            .field("size", &self.data.len())
            .finish()
    }
}

/// A wrapper that provides `Read` and `Seek` implementations for an `Asset`.
///
/// This allows assets to be used with standard I/O interfaces.
///
/// # Examples
///
/// ```
/// use usd_ar::{Asset, AssetReader, InMemoryAsset};
/// use std::io::Read;
///
/// let asset = InMemoryAsset::from_vec(b"Hello, World!".to_vec());
/// let mut reader = AssetReader::new(std::sync::Arc::new(asset));
///
/// let mut buffer = String::new();
/// reader.read_to_string(&mut buffer).unwrap();
/// assert_eq!(buffer, "Hello, World!");
/// ```
pub struct AssetReader {
    /// The underlying asset.
    asset: Arc<dyn Asset>,
    /// Current read position.
    position: usize,
}

impl AssetReader {
    /// Creates a new `AssetReader` for the given asset.
    ///
    /// # Arguments
    ///
    /// * `asset` - The asset to read from
    pub fn new(asset: Arc<dyn Asset>) -> Self {
        Self { asset, position: 0 }
    }

    /// Returns a reference to the underlying asset.
    pub fn asset(&self) -> &Arc<dyn Asset> {
        &self.asset
    }

    /// Returns the current read position.
    pub fn position(&self) -> usize {
        self.position
    }

    /// Sets the current read position.
    pub fn set_position(&mut self, position: usize) {
        self.position = position;
    }
}

impl Read for AssetReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let bytes_read = self.asset.read(buf, self.position);
        self.position += bytes_read;
        Ok(bytes_read)
    }
}

impl Seek for AssetReader {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let size = self.asset.size() as i64;
        let new_pos = match pos {
            SeekFrom::Start(offset) => offset as i64,
            SeekFrom::End(offset) => size + offset,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_in_memory_asset_new() {
        let data: Arc<[u8]> = Arc::from(vec![1, 2, 3, 4, 5].into_boxed_slice());
        let asset = InMemoryAsset::new(data);
        assert_eq!(asset.size(), 5);
    }

    #[test]
    fn test_in_memory_asset_from_vec() {
        let asset = InMemoryAsset::from_vec(vec![1, 2, 3, 4, 5]);
        assert_eq!(asset.size(), 5);
        assert_eq!(asset.as_bytes(), &[1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_in_memory_asset_empty() {
        let asset = InMemoryAsset::empty();
        assert_eq!(asset.size(), 0);
    }

    #[test]
    fn test_in_memory_asset_get_buffer() {
        let asset = InMemoryAsset::from_vec(vec![1, 2, 3]);
        let buffer = asset.get_buffer().expect("should have buffer");
        assert_eq!(&*buffer, &[1, 2, 3]);
    }

    #[test]
    fn test_in_memory_asset_read() {
        let asset = InMemoryAsset::from_vec(vec![1, 2, 3, 4, 5]);

        let mut buffer = vec![0u8; 3];
        let read = asset.read(&mut buffer, 0);
        assert_eq!(read, 3);
        assert_eq!(buffer, vec![1, 2, 3]);

        // Read from offset
        let read = asset.read(&mut buffer, 2);
        assert_eq!(read, 3);
        assert_eq!(buffer, vec![3, 4, 5]);
    }

    #[test]
    fn test_in_memory_asset_read_past_end() {
        let asset = InMemoryAsset::from_vec(vec![1, 2, 3]);

        let mut buffer = vec![0u8; 5];
        let read = asset.read(&mut buffer, 1);
        assert_eq!(read, 2); // Only 2 bytes available from offset 1
        assert_eq!(&buffer[..2], &[2, 3]);
    }

    #[test]
    fn test_in_memory_asset_read_out_of_bounds() {
        let asset = InMemoryAsset::from_vec(vec![1, 2, 3]);

        let mut buffer = vec![0u8; 5];
        let read = asset.read(&mut buffer, 10);
        assert_eq!(read, 0);
    }

    #[test]
    fn test_in_memory_asset_get_detached() {
        let asset = InMemoryAsset::from_vec(vec![1, 2, 3]);
        let detached = asset.get_detached().expect("should get detached");
        assert_eq!(detached.size(), 3);
    }

    #[test]
    fn test_in_memory_asset_from_asset() {
        let src = InMemoryAsset::from_vec(vec![10, 20, 30, 40]);
        let copy = InMemoryAsset::from_asset(&src).expect("should create");
        assert_eq!(copy.size(), 4);
        assert_eq!(copy.as_bytes(), &[10, 20, 30, 40]);
    }

    #[test]
    fn test_get_detached_via_read() {
        /// Asset that only supports read(), not get_buffer.
        struct ReadOnlyAsset {
            data: Vec<u8>,
        }
        impl Asset for ReadOnlyAsset {
            fn size(&self) -> usize {
                self.data.len()
            }
            fn get_buffer(&self) -> Option<Arc<[u8]>> {
                None
            }
            fn read(&self, buffer: &mut [u8], offset: usize) -> usize {
                if offset >= self.data.len() {
                    return 0;
                }
                let n = buffer.len().min(self.data.len() - offset);
                buffer[..n].copy_from_slice(&self.data[offset..offset + n]);
                n
            }
        }
        let asset = ReadOnlyAsset {
            data: vec![1, 2, 3, 4, 5],
        };
        let detached = asset
            .get_detached()
            .expect("get_detached should work via read");
        assert_eq!(detached.size(), 5);
        let buf = detached.get_buffer().expect("detached has buffer");
        assert_eq!(&*buf, &[1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_asset_reader_read() {
        let asset = Arc::new(InMemoryAsset::from_vec(b"Hello, World!".to_vec()));
        let mut reader = AssetReader::new(asset);

        let mut buffer = vec![0u8; 5];
        let read = reader.read(&mut buffer).expect("should read");
        assert_eq!(read, 5);
        assert_eq!(&buffer, b"Hello");
        assert_eq!(reader.position(), 5);
    }

    #[test]
    fn test_asset_reader_read_to_string() {
        let asset = Arc::new(InMemoryAsset::from_vec(b"Hello, World!".to_vec()));
        let mut reader = AssetReader::new(asset);

        let mut buffer = String::new();
        reader
            .read_to_string(&mut buffer)
            .expect("should read to string");
        assert_eq!(buffer, "Hello, World!");
    }

    #[test]
    fn test_asset_reader_seek_start() {
        let asset = Arc::new(InMemoryAsset::from_vec(b"Hello".to_vec()));
        let mut reader = AssetReader::new(asset);

        reader.seek(SeekFrom::Start(3)).expect("should seek");
        assert_eq!(reader.position(), 3);
    }

    #[test]
    fn test_asset_reader_seek_current() {
        let asset = Arc::new(InMemoryAsset::from_vec(b"Hello".to_vec()));
        let mut reader = AssetReader::new(asset);

        reader.set_position(2);
        reader.seek(SeekFrom::Current(1)).expect("should seek");
        assert_eq!(reader.position(), 3);

        reader.seek(SeekFrom::Current(-1)).expect("should seek");
        assert_eq!(reader.position(), 2);
    }

    #[test]
    fn test_asset_reader_seek_end() {
        let asset = Arc::new(InMemoryAsset::from_vec(b"Hello".to_vec()));
        let mut reader = AssetReader::new(asset);

        reader.seek(SeekFrom::End(-2)).expect("should seek");
        assert_eq!(reader.position(), 3);
    }

    #[test]
    fn test_asset_reader_seek_negative() {
        let asset = Arc::new(InMemoryAsset::from_vec(b"Hello".to_vec()));
        let mut reader = AssetReader::new(asset);

        let result = reader
            .seek(SeekFrom::Start(0))
            .and_then(|_| reader.seek(SeekFrom::Current(-1)));
        assert!(result.is_err());
    }

    #[test]
    fn test_in_memory_asset_debug() {
        let asset = InMemoryAsset::from_vec(vec![1, 2, 3]);
        let debug = format!("{:?}", asset);
        assert!(debug.contains("InMemoryAsset"));
        assert!(debug.contains("3"));
    }
}
