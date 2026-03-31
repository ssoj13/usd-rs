//! File path and search path utilities — port of MaterialXFormat/File.h.

use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::{Div, Index};
use std::path::{Path, PathBuf};

use crate::format::environ;

/// MaterialX file path.
///
/// Wraps `PathBuf` and mirrors C++ `FilePath` with both syntactic and
/// filesystem operations.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FilePath {
    path: PathBuf,
}

/// Path string format (mirrors C++ `FilePath::Format`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathFormat {
    Posix,
    Native,
}

// ── Constructors / conversions ────────────────────────────────────────────────

impl FilePath {
    /// Construct from anything that can be a Path.
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
        }
    }

    /// Raw `&str` view; returns `""` on non-UTF-8 paths.
    pub fn as_str(&self) -> &str {
        self.path.to_str().unwrap_or("")
    }

    /// Return as std::path::Path reference.
    pub fn as_path(&self) -> &Path {
        &self.path
    }

    /// Return string in the requested format (replaces separators for Posix).
    pub fn as_string(&self, format: PathFormat) -> String {
        let s = self.path.to_string_lossy();
        match format {
            PathFormat::Posix => s.replace('\\', "/"),
            PathFormat::Native => s.into_owned(),
        }
    }

    /// `true` when the path has no components.
    pub fn is_empty(&self) -> bool {
        self.path.as_os_str().is_empty()
    }

    pub fn is_absolute(&self) -> bool {
        self.path.is_absolute()
    }
}

impl From<&str> for FilePath {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl From<String> for FilePath {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

impl From<&Path> for FilePath {
    fn from(p: &Path) -> Self {
        Self::new(p)
    }
}

impl From<PathBuf> for FilePath {
    fn from(p: PathBuf) -> Self {
        Self { path: p }
    }
}

impl fmt::Display for FilePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.path.display())
    }
}

impl Hash for FilePath {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.path.hash(state);
    }
}

// ── Syntactic operations ─────────────────────────────────────────────────────

impl FilePath {
    /// Last component of the path (filename), empty string when path is empty.
    pub fn get_base_name(&self) -> &str {
        self.path.file_name().and_then(|n| n.to_str()).unwrap_or("")
    }

    /// File extension after the last `.`, empty string if none.
    pub fn get_extension(&self) -> String {
        self.path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_owned()
    }

    /// Append `.ext` to the path (e.g. `"foo"` + `"mtlx"` → `"foo.mtlx"`).
    pub fn add_extension(&mut self, ext: &str) {
        let mut s = self.path.to_string_lossy().into_owned();
        s.push('.');
        s.push_str(ext);
        self.path = PathBuf::from(s);
    }

    /// Remove the file extension, if any (mutates in place).
    pub fn remove_extension(&mut self) {
        if let Some(stem) = self.path.file_stem() {
            if let Some(parent) = self.path.parent() {
                let new_path = parent.join(stem);
                self.path = new_path;
            }
        }
    }

    /// Parent directory.  Returns empty path when there is no parent.
    pub fn get_parent_path(&self) -> FilePath {
        FilePath::new(
            self.path
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_default(),
        )
    }

    /// Collapse `.` and `..` references in the path (mirrors `getNormalized`).
    ///
    /// `a/./b` and `c/../d/../a/b` both become `a/b`.
    pub fn get_normalized(&self) -> FilePath {
        let mut components: Vec<std::ffi::OsString> = Vec::new();

        for comp in self.path.components() {
            use std::path::Component;
            match comp {
                // Keep prefix (drive letter on Windows) and root slash as-is
                Component::Prefix(p) => components.push(p.as_os_str().to_owned()),
                Component::RootDir => components.push(std::ffi::OsString::from("/")),
                Component::CurDir => {} // skip `.`
                Component::ParentDir => {
                    // Pop the last real component (mirrors C++ behaviour: don't pop `..`)
                    let last_is_parent = components.last().map(|c| c == "..").unwrap_or(false);
                    if !components.is_empty() && !last_is_parent {
                        components.pop();
                    } else {
                        components.push(std::ffi::OsString::from(".."));
                    }
                }
                Component::Normal(n) => components.push(n.to_owned()),
            }
        }

        let mut result = PathBuf::new();
        for c in components {
            result.push(c);
        }
        FilePath::new(result)
    }

    /// Number of path components (mirrors C++ `size()`).
    pub fn component_count(&self) -> usize {
        self.path.components().count()
    }
}

/// Path joining: `lhs / rhs` where `rhs` must be relative (panics if absolute).
impl Div<&FilePath> for FilePath {
    type Output = FilePath;
    fn div(self, rhs: &FilePath) -> FilePath {
        assert!(!rhs.is_absolute(), "Appended path must be relative.");
        FilePath::new(self.path.join(&rhs.path))
    }
}

/// Convenience: `FilePath / FilePath` (owned rhs).
impl Div<FilePath> for FilePath {
    type Output = FilePath;
    fn div(self, rhs: FilePath) -> FilePath {
        self / &rhs
    }
}

/// Index into path components by position.
impl Index<usize> for FilePath {
    type Output = str;
    fn index(&self, idx: usize) -> &str {
        self.path
            .components()
            .nth(idx)
            .and_then(|c| c.as_os_str().to_str())
            .expect("FilePath index out of bounds")
    }
}

// ── Filesystem operations ─────────────────────────────────────────────────────

impl FilePath {
    /// `true` if the path exists on the filesystem.
    pub fn exists(&self) -> bool {
        self.path.exists()
    }

    /// `true` if the path points to a directory.
    pub fn is_directory(&self) -> bool {
        self.path.is_dir()
    }

    /// All files in this directory with the given extension.
    ///
    /// Pass `""` to get all files.  Only the filename (not full path) is
    /// returned — mirrors C++ behaviour where `cFileName` is stored.
    pub fn get_files_in_directory(&self, extension: &str) -> Vec<FilePath> {
        let mut files = Vec::new();
        let dir = match std::fs::read_dir(&self.path) {
            Ok(d) => d,
            Err(_) => return files,
        };
        for entry in dir.flatten() {
            let meta = match entry.metadata() {
                Ok(m) => m,
                Err(_) => continue,
            };
            if !meta.is_file() {
                continue;
            }
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if extension.is_empty() {
                files.push(FilePath::from(name_str.as_ref() as &str));
            } else {
                let fp = FilePath::from(name_str.as_ref() as &str);
                if fp.get_extension() == extension {
                    files.push(fp);
                }
            }
        }
        files
    }

    /// All sub-directories at or beneath this path (recursive), including self.
    ///
    /// Returns an empty vec when this path is not a directory.
    pub fn get_sub_directories(&self) -> Vec<FilePath> {
        if !self.is_directory() {
            return Vec::new();
        }

        let mut result = vec![self.clone()];
        self.collect_subdirs_into(&mut result);
        result
    }

    fn collect_subdirs_into(&self, out: &mut Vec<FilePath>) {
        let dir = match std::fs::read_dir(&self.path) {
            Ok(d) => d,
            Err(_) => return,
        };
        for entry in dir.flatten() {
            let meta = match entry.metadata() {
                Ok(m) => m,
                Err(_) => continue,
            };
            if meta.is_dir() {
                let child = FilePath::new(entry.path());
                // Push child first (pre-order DFS, matches C++ getSubDirectories)
                out.push(child.clone());
                child.collect_subdirs_into(out);
            }
        }
    }

    /// Create this directory on the filesystem (non-recursive).
    pub fn create_directory(&self) {
        let _ = std::fs::create_dir(&self.path);
    }

    /// Return the current working directory.
    pub fn get_current_path() -> FilePath {
        std::env::current_dir()
            .map(FilePath::new)
            .unwrap_or_default()
    }

    /// Set the current working directory to this path.
    /// Returns `true` on success. Mirrors C++ `FilePath::setCurrentPath`.
    pub fn set_current_path(&self) -> bool {
        std::env::set_current_dir(&self.path).is_ok()
    }

    /// Return the directory containing the running executable.
    pub fn get_module_path() -> FilePath {
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(FilePath::new))
            .unwrap_or_default()
    }

    /// Re-assign path from a string. Mirrors C++ `FilePath::assign`.
    pub fn assign(&mut self, s: &str) {
        self.path = PathBuf::from(s);
    }
}

// ── Path list separator ───────────────────────────────────────────────────────

/// Path list separator: `;` on Windows, `:` otherwise.
#[cfg(windows)]
pub const PATH_LIST_SEPARATOR: char = ';';
#[cfg(not(windows))]
pub const PATH_LIST_SEPARATOR: char = ':';

// ── FileSearchPath ────────────────────────────────────────────────────────────

/// A sequence of `FilePath` values searched in order for a given filename.
#[derive(Clone, Debug, Default)]
pub struct FileSearchPath {
    paths: Vec<FilePath>,
}

impl FileSearchPath {
    pub fn new() -> Self {
        Self::default()
    }

    /// Build from a separator-delimited string (mirrors C++ constructor).
    pub fn from_str(s: &str) -> Self {
        let mut sp = Self::new();
        for part in s.split(PATH_LIST_SEPARATOR) {
            let p = part.trim();
            if !p.is_empty() {
                sp.append(FilePath::from(p));
            }
        }
        sp
    }

    pub fn append(&mut self, path: impl Into<FilePath>) {
        self.paths.push(path.into());
    }

    pub fn prepend(&mut self, path: impl Into<FilePath>) {
        self.paths.insert(0, path.into());
    }

    pub fn clear(&mut self) {
        self.paths.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.paths.is_empty()
    }

    /// Number of paths in this sequence (mirrors C++ `size()`).
    pub fn size(&self) -> usize {
        self.paths.len()
    }

    /// Serialize this search path to a string with the given separator.
    /// Mirrors C++ `FileSearchPath::asString(sep)`.
    pub fn as_string(&self, sep: char) -> String {
        self.paths
            .iter()
            .map(|p| p.as_string(PathFormat::Native))
            .collect::<Vec<_>>()
            .join(&sep.to_string())
    }

    /// Append all paths from another `FileSearchPath`.
    pub fn append_search_path(&mut self, other: &FileSearchPath) {
        for p in &other.paths {
            self.paths.push(p.clone());
        }
    }

    /// Iterate over all paths.
    pub fn paths_iter(&self) -> std::slice::Iter<'_, FilePath> {
        self.paths.iter()
    }

    /// Find the first existing resolved path for `filename`.
    ///
    /// If `filename` is absolute and exists it is returned unchanged.
    /// Otherwise each path in this sequence is tried as a prefix.
    /// Returns `None` when no match found (caller falls back to original name).
    pub fn find_path(&self, filename: &FilePath) -> Option<FilePath> {
        if self.paths.is_empty() || filename.is_empty() {
            return Some(filename.clone());
        }
        if !filename.is_absolute() {
            for base in &self.paths {
                let combined = base.clone() / filename;
                if combined.exists() {
                    return Some(combined);
                }
            }
        }
        Some(filename.clone())
    }

    /// Legacy string-based find used by xml_io (returns first existing path or None).
    pub fn find(&self, filename: &str) -> Option<FilePath> {
        let path = Path::new(filename);
        if path.is_absolute() && path.exists() {
            return Some(FilePath::new(path));
        }
        for base in &self.paths {
            let full = base.as_path().join(filename);
            if full.exists() {
                return Some(FilePath::new(full));
            }
        }
        None
    }
}

impl Index<usize> for FileSearchPath {
    type Output = FilePath;
    fn index(&self, idx: usize) -> &FilePath {
        &self.paths[idx]
    }
}

impl std::ops::IndexMut<usize> for FileSearchPath {
    fn index_mut(&mut self, idx: usize) -> &mut FilePath {
        &mut self.paths[idx]
    }
}

// ── Iterators ─────────────────────────────────────────────────────────────────

impl<'a> IntoIterator for &'a FileSearchPath {
    type Item = &'a FilePath;
    type IntoIter = std::slice::Iter<'a, FilePath>;
    fn into_iter(self) -> Self::IntoIter {
        self.paths.iter()
    }
}

// ── Environment path ──────────────────────────────────────────────────────────

/// Build a `FileSearchPath` from the `MATERIALX_SEARCH_PATH` environment variable.
/// Accepts an optional custom separator (defaults to `PATH_LIST_SEPARATOR`).
/// Mirrors C++ `getEnvironmentPath(const string& sep)`.
pub fn get_environment_path() -> FileSearchPath {
    get_environment_path_with_sep(PATH_LIST_SEPARATOR)
}

/// Build a `FileSearchPath` from the `MATERIALX_SEARCH_PATH` env var with custom separator.
pub fn get_environment_path_with_sep(sep: char) -> FileSearchPath {
    let s = environ::get_environ(environ::MATERIALX_SEARCH_PATH_ENV_VAR);
    let mut sp = FileSearchPath::new();
    for part in s.split(sep) {
        let p = part.trim();
        if !p.is_empty() {
            sp.append(FilePath::from(p));
        }
    }
    sp
}

// ── Standalone helpers ────────────────────────────────────────────────────────

/// Read file contents to string.  Returns empty string on error (matches C++ `readFile`).
pub fn read_file(path: &FilePath) -> String {
    std::fs::read_to_string(path.as_path()).unwrap_or_default()
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base_name() {
        let p = FilePath::from("dir/sub/file.mtlx");
        assert_eq!(p.get_base_name(), "file.mtlx");
    }

    #[test]
    fn test_extension() {
        let p = FilePath::from("dir/sub/file.mtlx");
        assert_eq!(p.get_extension(), "mtlx");

        let no_ext = FilePath::from("dir/sub/file");
        assert_eq!(no_ext.get_extension(), "");
    }

    #[test]
    fn test_add_remove_extension() {
        let mut p = FilePath::from("dir/file");
        p.add_extension("mtlx");
        assert_eq!(p.get_extension(), "mtlx");
        p.remove_extension();
        assert_eq!(p.get_extension(), "");
        assert_eq!(p.get_base_name(), "file");
    }

    #[test]
    fn test_path_join() {
        let a = FilePath::from("dir/sub");
        let b = FilePath::from("file.mtlx");
        let c = a / b;
        assert_eq!(c.get_base_name(), "file.mtlx");
    }

    #[test]
    fn test_normalized() {
        let p = FilePath::from("a/./b/../c");
        let n = p.get_normalized();
        // Should collapse to a/c
        assert_eq!(n.as_string(PathFormat::Posix), "a/c");
    }

    #[test]
    fn test_display_and_hash() {
        use std::collections::HashSet;
        let p = FilePath::from("some/path.mtlx");
        let _s = format!("{}", p);
        let mut set = HashSet::new();
        set.insert(p.clone());
        assert!(set.contains(&p));
    }

    #[test]
    fn test_component_count() {
        let p = FilePath::from("a/b/c");
        assert_eq!(p.component_count(), 3);
    }

    #[test]
    fn test_search_path_size_index() {
        let mut sp = FileSearchPath::new();
        sp.append(FilePath::from("a"));
        sp.append(FilePath::from("b"));
        assert_eq!(sp.size(), 2);
        assert_eq!(sp[0].as_str(), "a");
        assert_eq!(sp[1].as_str(), "b");
    }

    #[test]
    fn test_from_string() {
        let p: FilePath = String::from("foo/bar").into();
        assert_eq!(p.get_base_name(), "bar");
    }
}
