//! Safe file output with FILE*-like interface.
//!
//! Opens a file for output, either for update ("r+") or to completely replace.
//! In the case of complete replacement, creates a sibling temporary file to
//! write to instead, then renames atomically when closing.
//!
//! # Examples
//!
//! ```no_run
//! use usd_tf::safe_output_file::SafeOutputFile;
//! use std::io::Write;
//!
//! // Replace mode: write to temp file, then atomic rename
//! let mut file = SafeOutputFile::replace("/home/user/file.txt")?;
//! write!(file, "content")?;
//! file.close()?; // Atomic rename happens here
//!
//! // Update mode: open existing file for in-place modification
//! let mut file = SafeOutputFile::update("/home/user/existing.txt")?;
//! write!(file, "updated ")?;
//! file.close()?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use std::fs::{File, OpenOptions};
use std::io::{self, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use crate::atomic_rename::{atomic_rename_file_over, create_sibling_temp_file};
use crate::file_utils::delete_file;

/// Mode in which the file was opened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OpenMode {
    /// File opened for update (in-place modification).
    Update,
    /// File opened for replace (write to temp, then rename).
    Replace,
}

/// Safe output file with automatic cleanup.
///
/// Provides two modes of operation:
/// - **Update**: Opens existing file for in-place modification
/// - **Replace**: Creates a temp file, writes to it, then atomically renames
///
/// In replace mode, other processes reading the file continue to see the old
/// contents until the new file is committed.
pub struct SafeOutputFile {
    /// The underlying file (buffered).
    file: Option<BufWriter<File>>,
    /// The target file path.
    target_path: PathBuf,
    /// The temporary file path (only for Replace mode).
    temp_path: Option<PathBuf>,
    /// The mode in which this file was opened.
    mode: OpenMode,
}

impl SafeOutputFile {
    /// Opens an existing file for update ("r+").
    ///
    /// The file must exist. Changes are written directly to the file.
    ///
    /// # Errors
    ///
    /// Returns an error if the file doesn't exist or cannot be opened.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use usd_tf::safe_output_file::SafeOutputFile;
    ///
    /// let file = SafeOutputFile::update("/path/to/existing.txt")?;
    /// assert!(file.is_open_for_update());
    /// # Ok::<(), std::io::Error>(())
    /// ```
    pub fn update<P: AsRef<Path>>(file_name: P) -> io::Result<Self> {
        let path = file_name.as_ref().to_path_buf();

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .map_err(|e| {
                io::Error::new(
                    e.kind(),
                    format!(
                        "Unable to open file '{}' for writing: {}",
                        path.display(),
                        e
                    ),
                )
            })?;

        Ok(Self {
            file: Some(BufWriter::new(file)),
            target_path: path,
            temp_path: None,
            mode: OpenMode::Update,
        })
    }

    /// Opens a file for replacement.
    ///
    /// Creates a sibling temporary file and opens it for writing.
    /// When `close()` is called, the temp file is atomically renamed
    /// to replace the target file.
    ///
    /// # Errors
    ///
    /// Returns an error if the temp file cannot be created or opened.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use usd_tf::safe_output_file::SafeOutputFile;
    ///
    /// let file = SafeOutputFile::replace("/path/to/file.txt")?;
    /// assert!(!file.is_open_for_update());
    /// # Ok::<(), std::io::Error>(())
    /// ```
    pub fn replace<P: AsRef<Path>>(file_name: P) -> io::Result<Self> {
        let (temp_path, target_path) = create_sibling_temp_file(file_name.as_ref())?;

        let file = OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(&temp_path)
            .map_err(|e| {
                io::Error::new(
                    e.kind(),
                    format!(
                        "Unable to open temp file '{}' for writing: {}",
                        temp_path.display(),
                        e
                    ),
                )
            })?;

        Ok(Self {
            file: Some(BufWriter::new(file)),
            target_path,
            temp_path: Some(temp_path),
            mode: OpenMode::Replace,
        })
    }

    /// Closes the file.
    ///
    /// If opened with `replace()`, atomically renames the temp file
    /// to the target path.
    ///
    /// # Errors
    ///
    /// Returns an error if closing or renaming fails.
    pub fn close(&mut self) -> io::Result<()> {
        let file = match self.file.take() {
            Some(f) => f,
            None => return Ok(()), // Already closed
        };

        // Flush and close
        let inner = file.into_inner()?;
        inner.sync_all()?;
        drop(inner);

        // If replace mode, rename temp to target
        if let Some(temp_path) = self.temp_path.take() {
            atomic_rename_file_over(&temp_path, &self.target_path)?;
        }

        Ok(())
    }

    /// Discards the file without committing changes.
    ///
    /// For files opened with `replace()`, removes the temp file without
    /// renaming. For files opened with `update()`, this is an error.
    ///
    /// # Errors
    ///
    /// Returns an error if called on a file opened for update, or if
    /// the temp file cannot be removed.
    pub fn discard(&mut self) -> io::Result<()> {
        if self.mode == OpenMode::Update {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Cannot discard file opened for update",
            ));
        }

        // Take temp path so close() won't rename
        let temp_path = self.temp_path.take();

        // Close the file
        if let Some(file) = self.file.take() {
            drop(file);
        }

        // Remove temp file
        if let Some(path) = temp_path {
            delete_file(&path);
        }

        Ok(())
    }

    /// Returns true if this file was opened for update.
    #[inline]
    pub fn is_open_for_update(&self) -> bool {
        self.mode == OpenMode::Update
    }

    /// Returns true if the file is currently open.
    #[inline]
    pub fn is_open(&self) -> bool {
        self.file.is_some()
    }

    /// Returns a reference to the underlying file, if open.
    pub fn get(&self) -> Option<&BufWriter<File>> {
        self.file.as_ref()
    }

    /// Returns a mutable reference to the underlying file, if open.
    pub fn get_mut(&mut self) -> Option<&mut BufWriter<File>> {
        self.file.as_mut()
    }

    /// Releases ownership of the underlying file.
    ///
    /// Only valid for files opened with `update()`. The caller takes
    /// responsibility for closing the file.
    ///
    /// # Errors
    ///
    /// Returns an error if the file was opened with `replace()`.
    pub fn release_updated_file(mut self) -> io::Result<File> {
        if self.mode != OpenMode::Update {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Cannot release file opened for replace",
            ));
        }

        let file = self
            .file
            .take()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotConnected, "File is not open"))?;

        Ok(file.into_inner()?)
    }

    /// Returns the target file path.
    pub fn target_path(&self) -> &Path {
        &self.target_path
    }

    /// Returns the temporary file path (only for replace mode).
    pub fn temp_path(&self) -> Option<&Path> {
        self.temp_path.as_deref()
    }
}

impl Drop for SafeOutputFile {
    fn drop(&mut self) {
        // Close on drop (ignore errors)
        let _ = self.close();
    }
}

impl Write for SafeOutputFile {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.file
            .as_mut()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotConnected, "File not open"))?
            .write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file
            .as_mut()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotConnected, "File not open"))?
            .flush()
    }
}

impl Read for SafeOutputFile {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.file
            .as_mut()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotConnected, "File not open"))?
            .get_mut()
            .read(buf)
    }
}

impl Seek for SafeOutputFile {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        self.file
            .as_mut()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotConnected, "File not open"))?
            .seek(pos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_replace_mode() {
        let temp_dir = std::env::temp_dir();
        let target = temp_dir.join("safe_output_replace.txt");

        let _ = fs::remove_file(&target);

        let mut file = SafeOutputFile::replace(&target).unwrap();
        assert!(!file.is_open_for_update());
        assert!(file.temp_path().is_some());

        write!(file, "new content").unwrap();
        file.close().unwrap();

        let content = fs::read_to_string(&target).unwrap();
        assert_eq!(content, "new content");

        let _ = fs::remove_file(&target);
    }

    #[test]
    fn test_update_mode() {
        let temp_dir = std::env::temp_dir();
        let target = temp_dir.join("safe_output_update.txt");

        // Create initial file
        fs::write(&target, "original content").unwrap();

        let mut file = SafeOutputFile::update(&target).unwrap();
        assert!(file.is_open_for_update());
        assert!(file.temp_path().is_none());

        write!(file, "UPDATED").unwrap();
        file.close().unwrap();

        let content = fs::read_to_string(&target).unwrap();
        assert_eq!(content, "UPDATEDl content");

        let _ = fs::remove_file(&target);
    }

    #[test]
    fn test_discard() {
        let temp_dir = std::env::temp_dir();
        let target = temp_dir.join("safe_output_discard.txt");

        // Create original
        fs::write(&target, "original").unwrap();

        let mut file = SafeOutputFile::replace(&target).unwrap();
        let temp_path = file.temp_path().unwrap().to_path_buf();

        write!(file, "new content").unwrap();
        file.discard().unwrap();

        // Temp should be gone
        assert!(!temp_path.exists());

        // Original unchanged
        let content = fs::read_to_string(&target).unwrap();
        assert_eq!(content, "original");

        let _ = fs::remove_file(&target);
    }

    #[test]
    fn test_discard_update_error() {
        let temp_dir = std::env::temp_dir();
        let target = temp_dir.join("safe_output_discard_update.txt");

        fs::write(&target, "content").unwrap();

        let mut file = SafeOutputFile::update(&target).unwrap();
        let result = file.discard();
        assert!(result.is_err());

        file.close().unwrap();
        let _ = fs::remove_file(&target);
    }

    #[test]
    fn test_release_updated_file() {
        let temp_dir = std::env::temp_dir();
        let target = temp_dir.join("safe_output_release.txt");

        fs::write(&target, "content").unwrap();

        let file = SafeOutputFile::update(&target).unwrap();
        let raw = file.release_updated_file().unwrap();
        drop(raw);

        let _ = fs::remove_file(&target);
    }

    #[test]
    fn test_release_replace_error() {
        let temp_dir = std::env::temp_dir();
        let target = temp_dir.join("safe_output_release_replace.txt");

        let _ = fs::remove_file(&target);

        let file = SafeOutputFile::replace(&target).unwrap();
        let result = file.release_updated_file();
        assert!(result.is_err());
    }

    #[test]
    fn test_drop_closes() {
        let temp_dir = std::env::temp_dir();
        let target = temp_dir.join("safe_output_drop.txt");

        let _ = fs::remove_file(&target);

        {
            let mut file = SafeOutputFile::replace(&target).unwrap();
            write!(file, "content").unwrap();
            // Drop commits
        }

        assert!(target.exists());
        let _ = fs::remove_file(&target);
    }

    #[test]
    fn test_seek_and_read() {
        let temp_dir = std::env::temp_dir();
        let target = temp_dir.join("safe_output_seek.txt");

        fs::write(&target, "hello world").unwrap();

        let mut file = SafeOutputFile::update(&target).unwrap();

        // Seek to position 6
        file.seek(SeekFrom::Start(6)).unwrap();

        // Read rest
        let mut buf = [0u8; 5];
        file.read_exact(&mut buf).unwrap();
        assert_eq!(&buf, b"world");

        file.close().unwrap();
        let _ = fs::remove_file(&target);
    }

    #[test]
    fn test_update_nonexistent_error() {
        let result = SafeOutputFile::update("/nonexistent/path/file.txt");
        assert!(result.is_err());
    }

    #[test]
    fn test_write_trait() {
        let temp_dir = std::env::temp_dir();
        let target = temp_dir.join("safe_output_write_trait.txt");

        let _ = fs::remove_file(&target);

        let mut file = SafeOutputFile::replace(&target).unwrap();
        writeln!(file, "line 1").unwrap();
        writeln!(file, "line 2").unwrap();
        file.flush().unwrap();
        file.close().unwrap();

        let content = fs::read_to_string(&target).unwrap();
        assert!(content.contains("line 1"));
        assert!(content.contains("line 2"));

        let _ = fs::remove_file(&target);
    }
}
