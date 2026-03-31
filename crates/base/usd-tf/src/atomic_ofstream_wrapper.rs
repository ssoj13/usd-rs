//! Atomic file writer with ofstream-like interface.
//!
//! Provides improved tolerance for write failures by writing to a temporary
//! file first, then atomically renaming to the destination on commit.
//!
//! # Examples
//!
//! ```no_run
//! use usd_tf::atomic_ofstream_wrapper::AtomicOfstreamWrapper;
//! use std::io::Write;
//!
//! // Create wrapper with destination path
//! let mut wrapper = AtomicOfstreamWrapper::new("/home/user/file.txt");
//!
//! // Open for writing
//! wrapper.open()?;
//!
//! // Write content
//! if let Some(stream) = wrapper.stream_mut() {
//!     writeln!(stream, "Hello, world!")?;
//! }
//!
//! // Commit changes (atomic rename)
//! wrapper.commit()?;
//!
//! // If wrapper goes out of scope without commit, cancel() is called
//! // and the temporary file is removed.
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use std::fs::{self, File, OpenOptions};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};

use crate::atomic_rename::{atomic_rename_file_over, create_sibling_temp_file};

/// Atomic output file stream wrapper.
///
/// Writes to a temporary file on the same filesystem as the destination,
/// then atomically renames to the final path on commit. This ensures that
/// partial writes or failures don't corrupt the destination file.
pub struct AtomicOfstreamWrapper {
    /// Target file path.
    file_path: PathBuf,
    /// Temporary file path (set after open).
    tmp_file_path: Option<PathBuf>,
    /// The buffered file writer.
    stream: Option<BufWriter<File>>,
}

impl AtomicOfstreamWrapper {
    /// Creates a new AtomicOfstreamWrapper for the given destination path.
    ///
    /// The file is not opened until `open()` is called.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::atomic_ofstream_wrapper::AtomicOfstreamWrapper;
    ///
    /// let wrapper = AtomicOfstreamWrapper::new("/path/to/file.txt");
    /// assert!(!wrapper.is_open());
    /// ```
    pub fn new<P: AsRef<Path>>(file_path: P) -> Self {
        Self {
            file_path: file_path.as_ref().to_path_buf(),
            tmp_file_path: None,
            stream: None,
        }
    }

    /// Opens the temporary file for writing.
    ///
    /// Creates the destination directory if it doesn't exist.
    /// Returns an error if the stream is already open, the directory
    /// cannot be created, or the temp file cannot be opened.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Stream is already open
    /// - Destination directory doesn't exist and cannot be created
    /// - Temporary file cannot be created
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use usd_tf::atomic_ofstream_wrapper::AtomicOfstreamWrapper;
    ///
    /// let mut wrapper = AtomicOfstreamWrapper::new("/tmp/test.txt");
    /// wrapper.open()?;
    /// assert!(wrapper.is_open());
    /// # Ok::<(), std::io::Error>(())
    /// ```
    pub fn open(&mut self) -> io::Result<()> {
        if self.stream.is_some() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "Stream is already open",
            ));
        }

        // Create sibling temp file
        let (tmp_path, real_path) = create_sibling_temp_file(&self.file_path)?;
        self.file_path = real_path;

        // Open temp file for writing
        let file = OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(&tmp_path)?;

        self.tmp_file_path = Some(tmp_path);
        self.stream = Some(BufWriter::new(file));

        Ok(())
    }

    /// Commits the changes by syncing to disk and atomically renaming
    /// the temporary file to the destination.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Stream is not open
    /// - Sync or close fails
    /// - Atomic rename fails
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use usd_tf::atomic_ofstream_wrapper::AtomicOfstreamWrapper;
    /// use std::io::Write;
    ///
    /// let mut wrapper = AtomicOfstreamWrapper::new("/tmp/test.txt");
    /// wrapper.open()?;
    /// writeln!(wrapper.stream_mut().unwrap(), "content")?;
    /// wrapper.commit()?;
    /// # Ok::<(), std::io::Error>(())
    /// ```
    pub fn commit(&mut self) -> io::Result<()> {
        let stream = self
            .stream
            .take()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotConnected, "Stream is not open"))?;

        let tmp_path = self
            .tmp_file_path
            .take()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotConnected, "No temporary file"))?;

        // Flush and close the stream
        let file = stream.into_inner()?;
        file.sync_all()?;
        drop(file);

        // Atomically rename
        atomic_rename_file_over(&tmp_path, &self.file_path)
    }

    /// Cancels the write operation by closing and removing the temporary file.
    ///
    /// # Errors
    ///
    /// Returns an error if the stream is not open or the temp file
    /// cannot be removed.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use usd_tf::atomic_ofstream_wrapper::AtomicOfstreamWrapper;
    ///
    /// let mut wrapper = AtomicOfstreamWrapper::new("/tmp/test.txt");
    /// wrapper.open()?;
    /// // Decide not to write...
    /// wrapper.cancel()?;
    /// # Ok::<(), std::io::Error>(())
    /// ```
    pub fn cancel(&mut self) -> io::Result<()> {
        let stream = self
            .stream
            .take()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotConnected, "Stream is not open"))?;

        // Close the stream
        drop(stream);

        // Remove the temp file
        if let Some(tmp_path) = self.tmp_file_path.take() {
            match fs::remove_file(&tmp_path) {
                Ok(()) => Ok(()),
                Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
                Err(e) => Err(e),
            }
        } else {
            Ok(())
        }
    }

    /// Returns true if the stream is open.
    #[inline]
    pub fn is_open(&self) -> bool {
        self.stream.is_some()
    }

    /// Returns a reference to the underlying writer.
    ///
    /// Returns `None` if the stream is not open.
    pub fn stream(&self) -> Option<&BufWriter<File>> {
        self.stream.as_ref()
    }

    /// Returns a mutable reference to the underlying writer.
    ///
    /// Returns `None` if the stream is not open.
    pub fn stream_mut(&mut self) -> Option<&mut BufWriter<File>> {
        self.stream.as_mut()
    }

    /// Returns the destination file path.
    pub fn file_path(&self) -> &Path {
        &self.file_path
    }

    /// Returns the temporary file path, if open.
    pub fn temp_file_path(&self) -> Option<&Path> {
        self.tmp_file_path.as_deref()
    }
}

impl Drop for AtomicOfstreamWrapper {
    fn drop(&mut self) {
        // Cancel on drop if still open (don't propagate errors)
        if self.is_open() {
            let _ = self.cancel();
        }
    }
}

impl Write for AtomicOfstreamWrapper {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.stream
            .as_mut()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotConnected, "Stream not open"))?
            .write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.stream
            .as_mut()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotConnected, "Stream not open"))?
            .flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let wrapper = AtomicOfstreamWrapper::new("/tmp/test.txt");
        assert!(!wrapper.is_open());
        assert_eq!(wrapper.file_path().to_str().unwrap(), "/tmp/test.txt");
    }

    #[test]
    fn test_open_write_commit() {
        let temp_dir = std::env::temp_dir();
        let target = temp_dir.join("atomic_wrapper_test.txt");

        // Cleanup
        let _ = fs::remove_file(&target);

        let mut wrapper = AtomicOfstreamWrapper::new(&target);

        // Open
        wrapper.open().unwrap();
        assert!(wrapper.is_open());
        assert!(wrapper.temp_file_path().is_some());

        // Write
        writeln!(wrapper.stream_mut().unwrap(), "test content").unwrap();

        // Commit
        wrapper.commit().unwrap();
        assert!(!wrapper.is_open());

        // Verify
        let content = fs::read_to_string(&target).unwrap();
        assert_eq!(content.trim(), "test content");

        // Cleanup
        let _ = fs::remove_file(&target);
    }

    #[test]
    fn test_open_write_cancel() {
        let temp_dir = std::env::temp_dir();
        let target = temp_dir.join("atomic_wrapper_cancel_test.txt");

        // Create existing file
        fs::write(&target, "original").unwrap();

        let mut wrapper = AtomicOfstreamWrapper::new(&target);
        wrapper.open().unwrap();

        let temp_path = wrapper.temp_file_path().unwrap().to_path_buf();
        assert!(temp_path.exists());

        writeln!(wrapper.stream_mut().unwrap(), "new content").unwrap();

        // Cancel
        wrapper.cancel().unwrap();

        // Temp file should be removed
        assert!(!temp_path.exists());

        // Original should be unchanged
        let content = fs::read_to_string(&target).unwrap();
        assert_eq!(content, "original");

        // Cleanup
        let _ = fs::remove_file(&target);
    }

    #[test]
    fn test_drop_cancels() {
        let temp_dir = std::env::temp_dir();
        let target = temp_dir.join("atomic_wrapper_drop_test.txt");

        // Cleanup
        let _ = fs::remove_file(&target);

        let temp_path;
        {
            let mut wrapper = AtomicOfstreamWrapper::new(&target);
            wrapper.open().unwrap();
            temp_path = wrapper.temp_file_path().unwrap().to_path_buf();
            assert!(temp_path.exists());
            // Drop without commit
        }

        // Temp file should be removed
        assert!(!temp_path.exists());
        // Target should not exist
        assert!(!target.exists());
    }

    #[test]
    fn test_double_open_error() {
        let temp_dir = std::env::temp_dir();
        let target = temp_dir.join("atomic_wrapper_double_open.txt");

        let mut wrapper = AtomicOfstreamWrapper::new(&target);
        wrapper.open().unwrap();

        let result = wrapper.open();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::AlreadyExists);

        wrapper.cancel().unwrap();
    }

    #[test]
    fn test_commit_without_open() {
        let mut wrapper = AtomicOfstreamWrapper::new("/tmp/test.txt");
        let result = wrapper.commit();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::NotConnected);
    }

    #[test]
    fn test_cancel_without_open() {
        let mut wrapper = AtomicOfstreamWrapper::new("/tmp/test.txt");
        let result = wrapper.cancel();
        assert!(result.is_err());
    }

    #[test]
    fn test_write_trait() {
        let temp_dir = std::env::temp_dir();
        let target = temp_dir.join("atomic_wrapper_write_trait.txt");

        let _ = fs::remove_file(&target);

        let mut wrapper = AtomicOfstreamWrapper::new(&target);
        wrapper.open().unwrap();

        // Use Write trait
        write!(wrapper, "hello ").unwrap();
        write!(wrapper, "world").unwrap();
        wrapper.flush().unwrap();

        wrapper.commit().unwrap();

        let content = fs::read_to_string(&target).unwrap();
        assert_eq!(content, "hello world");

        let _ = fs::remove_file(&target);
    }

    #[test]
    fn test_replace_existing_file() {
        let temp_dir = std::env::temp_dir();
        let target = temp_dir.join("atomic_wrapper_replace.txt");

        // Create existing file
        fs::write(&target, "old content").unwrap();

        let mut wrapper = AtomicOfstreamWrapper::new(&target);
        wrapper.open().unwrap();
        write!(wrapper, "new content").unwrap();
        wrapper.commit().unwrap();

        let content = fs::read_to_string(&target).unwrap();
        assert_eq!(content, "new content");

        let _ = fs::remove_file(&target);
    }
}
