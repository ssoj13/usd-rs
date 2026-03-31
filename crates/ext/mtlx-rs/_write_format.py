"""Write all format module files for mtlx-rs parity check."""
import pathlib

BASE = pathlib.Path(__file__).parent / "src" / "format"

# ===== environ.rs =====
(BASE / "environ.rs").write_text(
"""//! Environment variable utilities -- port of MaterialXFormat/Environ.h.

/// Get environment variable value (matches C++ getEnviron).
pub fn get_environ(name: &str) -> Option<String> {
    std::env::var(name).ok()
}

/// Set environment variable (matches C++ setEnviron). Returns true on success.
pub fn set_environ(name: &str, value: &str) -> bool {
    // SAFETY: single-threaded usage; no async signal handlers
    unsafe { std::env::set_var(name, value) };
    true
}

/// Remove environment variable (matches C++ removeEnviron). Returns true on success.
pub fn remove_environ(name: &str) -> bool {
    // SAFETY: single-threaded usage
    unsafe { std::env::remove_var(name) };
    true
}

pub const MATERIALX_SEARCH_PATH_ENV_VAR: &str = "MATERIALX_SEARCH_PATH";
""")

# ===== mod.rs =====
(BASE / "mod.rs").write_text(
"""//! MaterialXFormat -- XML I/O, File, FilePath, Environ.

pub mod environ;
pub mod file;
pub mod xml_io;
pub mod util;

pub use environ::{get_environ, remove_environ, set_environ, MATERIALX_SEARCH_PATH_ENV_VAR};
pub use file::{
    get_environment_path, read_file, FilePath, FileSearchPath, PathFormat, PathType,
    MTLX_EXTENSION, PATH_LIST_SEPARATOR,
};
pub use xml_io::{
    prepend_xinclude, read_from_xml_buffer, read_from_xml_file, read_from_xml_file_path,
    read_from_xml_str, read_from_xml_str_with_options, write_to_xml_file, write_to_xml_string,
    write_to_xml_string_with_options, XmlError, XmlReadOptions, XmlWriteOptions,
    MAX_XINCLUDE_DEPTH, MAX_XML_TREE_DEPTH,
};
pub use util::{
    flatten_filenames, get_default_data_search_path, get_source_search_path, get_subdirectories,
    load_documents, load_libraries, load_library, load_library_path,
};
""")

# ===== file.rs =====
(BASE / "file.rs").write_text(
r"""//! File path and search path utilities -- port of MaterialXFormat/File.h.

use std::path::{Path, PathBuf};

use crate::format::environ;

/// Path list separator: `;` on Windows, `:` otherwise (matches C++ PATH_LIST_SEPARATOR).
#[cfg(windows)]
pub const PATH_LIST_SEPARATOR: &str = ";";
#[cfg(not(windows))]
pub const PATH_LIST_SEPARATOR: &str = ":";

/// MTLX file extension constant (matches C++ MTLX_EXTENSION).
pub const MTLX_EXTENSION: &str = "mtlx";

/// Path type classification (matches C++ FilePath::Type).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathType {
    Relative,
    Absolute,
    Network,
}

/// Format for path string conversion (matches C++ FilePath::Format).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathFormat {
    Windows,
    Posix,
    Native,
}

/// MaterialX file path (matches C++ FilePath).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FilePath {
    path: PathBuf,
}

impl FilePath {
    /// Construct from anything that can be a Path.
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self { path: path.as_ref().to_path_buf() }
    }

    /// Return path as &str (convenience).
    pub fn as_str(&self) -> &str {
        self.path.to_str().unwrap_or("")
    }

    /// Return as std::path::Path reference.
    pub fn as_path(&self) -> &Path {
        &self.path
    }

    /// Return true if path is absolute (matches C++ isAbsolute).
    pub fn is_absolute(&self) -> bool {
        self.path.is_absolute()
    }

    /// Return true if path is empty (matches C++ isEmpty).
    pub fn is_empty(&self) -> bool {
        self.path.as_os_str().is_empty()
    }

    /// Convert to string with given format (matches C++ asString).
    pub fn as_string(&self, format: PathFormat) -> String {
        let s = self.path.to_string_lossy();
        match format {
            PathFormat::Posix => s.replace('\\', "/"),
            PathFormat::Windows => s.replace('/', "\\"),
            PathFormat::Native => s.to_string(),
        }
    }

    /// Convenience: return as native format string.
    pub fn as_str_native(&self) -> String {
        self.as_string(PathFormat::Native)
    }

    /// Return the base name (last component) (matches C++ getBaseName).
    pub fn get_base_name(&self) -> &str {
        self.path.file_name().and_then(|n| n.to_str()).unwrap_or("")
    }

    /// Parent directory path (matches C++ getParentPath).
    pub fn get_parent_path(&self) -> FilePath {
        FilePath::new(self.path.parent().map(|p| p.to_path_buf()).unwrap_or_default())
    }

    /// Return file extension (matches C++ getExtension).
    pub fn get_extension(&self) -> String {
        self.path.extension().and_then(|e| e.to_str()).unwrap_or("").to_string()
    }

    /// Add file extension (matches C++ addExtension).
    pub fn add_extension(&mut self, ext: &str) {
        let mut s = self.path.to_string_lossy().into_owned();
        s.push('.');
        s.push_str(ext);
        self.path = PathBuf::from(s);
    }

    /// Remove file extension (matches C++ removeExtension).
    pub fn remove_extension(&mut self) {
        if let Some(stem) = self.path.file_stem() {
            if let Some(parent) = self.path.parent() {
                self.path = parent.join(stem);
            } else {
                self.path = PathBuf::from(stem);
            }
        }
    }

    /// Concatenate two paths (matches C++ operator/).
    pub fn join(&self, rhs: &FilePath) -> FilePath {
        FilePath::new(self.path.join(&rhs.path))
    }

    /// Number of path components (matches C++ size).
    pub fn size(&self) -> usize {
        self.path.components().count()
    }

    /// Return normalized path, collapsing . and .. (matches C++ getNormalized).
    pub fn get_normalized(&self) -> FilePath {
        let mut components: Vec<std::path::Component> = Vec::new();
        for c in self.path.components() {
            match c {
                std::path::Component::CurDir => {}
                std::path::Component::ParentDir => {
                    if let Some(last) = components.last() {
                        if !matches!(last, std::path::Component::ParentDir) {
                            components.pop();
                            continue;
                        }
                    }
                    components.push(c);
                }
                _ => components.push(c),
            }
        }
        let mut result = PathBuf::new();
        for c in components { result.push(c); }
        FilePath::new(result)
    }

    /// Check if path exists on filesystem (matches C++ exists).
    pub fn exists(&self) -> bool {
        !self.is_empty() && self.path.exists()
    }

    /// Check if path is a directory (matches C++ isDirectory).
    pub fn is_directory(&self) -> bool {
        !self.is_empty() && self.path.is_dir()
    }

    /// List files in directory with optional extension filter (matches C++ getFilesInDirectory).
    pub fn get_files_in_directory(&self, extension: &str) -> Vec<FilePath> {
        let mut files = Vec::new();
        let entries = match std::fs::read_dir(&self.path) {
            Ok(e) => e,
            Err(_) => return files,
        };
        for entry in entries.flatten() {
            let meta = match entry.metadata() { Ok(m) => m, Err(_) => continue };
            if !meta.is_dir() {
                let fp = FilePath::new(entry.file_name());
                if extension.is_empty() || fp.get_extension() == extension {
                    files.push(fp);
                }
            }
        }
        files
    }

    /// Get all subdirectories recursively, including self (matches C++ getSubDirectories).
    pub fn get_sub_directories(&self) -> Vec<FilePath> {
        if !self.is_directory() { return Vec::new(); }
        let mut dirs = vec![self.clone()];
        let entries = match std::fs::read_dir(&self.path) {
            Ok(e) => e,
            Err(_) => return dirs,
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str == "." || name_str == ".." { continue; }
            if let Ok(meta) = entry.metadata() {
                if meta.is_dir() {
                    let sub = self.join(&FilePath::new(name));
                    dirs.extend(sub.get_sub_directories());
                }
            }
        }
        dirs
    }

    /// Create directory on the file system (matches C++ createDirectory).
    pub fn create_directory(&self) {
        let _ = std::fs::create_dir_all(&self.path);
    }

    /// Set current working directory (matches C++ setCurrentPath).
    pub fn set_current_path(&self) -> bool {
        std::env::set_current_dir(&self.path).is_ok()
    }

    /// Get current working directory (matches C++ getCurrentPath).
    pub fn get_current_path() -> FilePath {
        match std::env::current_dir() { Ok(p) => FilePath::new(p), Err(_) => FilePath::default() }
    }

    /// Get module/executable directory (matches C++ getModulePath).
    pub fn get_module_path() -> FilePath {
        match std::env::current_exe() {
            Ok(p) => FilePath::new(p).get_parent_path(),
            Err(_) => FilePath::default(),
        }
    }

    /// Get path type (matches C++ FilePath::Type).
    pub fn get_type(&self) -> PathType {
        if self.is_empty() { return PathType::Relative; }
        let s = self.path.to_string_lossy();
        if s.starts_with("\\\\") { PathType::Network }
        else if self.path.is_absolute() { PathType::Absolute }
        else { PathType::Relative }
    }
}

impl std::fmt::Display for FilePath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.path.display())
    }
}

impl From<&str> for FilePath {
    fn from(s: &str) -> Self { Self::new(s) }
}
impl From<String> for FilePath {
    fn from(s: String) -> Self { Self::new(s) }
}
impl From<PathBuf> for FilePath {
    fn from(p: PathBuf) -> Self { Self::new(p) }
}
impl From<FilePath> for String {
    fn from(fp: FilePath) -> Self { fp.as_string(PathFormat::Native) }
}

/// Search path for resolving file references (matches C++ FileSearchPath).
#[derive(Clone, Debug, Default)]
pub struct FileSearchPath {
    paths: Vec<FilePath>,
}

impl FileSearchPath {
    pub fn new() -> Self { Self::default() }

    /// Construct from string with separator (matches C++ FileSearchPath(string, sep)).
    pub fn from_string(search_path: &str, sep: &str) -> Self {
        let mut sp = Self::new();
        for part in search_path.split(sep) {
            let p = part.trim();
            if !p.is_empty() { sp.append(FilePath::new(p)); }
        }
        sp
    }

    /// Convert to string with separator (matches C++ asString).
    pub fn as_string(&self, sep: &str) -> String {
        self.paths.iter().map(|p| p.as_string(PathFormat::Native)).collect::<Vec<_>>().join(sep)
    }

    pub fn append(&mut self, path: impl Into<FilePath>) { self.paths.push(path.into()); }

    pub fn append_search_path(&mut self, other: &FileSearchPath) {
        for p in &other.paths { self.paths.push(p.clone()); }
    }

    pub fn prepend(&mut self, path: impl Into<FilePath>) { self.paths.insert(0, path.into()); }

    pub fn clear(&mut self) { self.paths.clear(); }

    pub fn size(&self) -> usize { self.paths.len() }

    pub fn is_empty(&self) -> bool { self.paths.is_empty() }

    pub fn get(&self, index: usize) -> &FilePath { &self.paths[index] }

    pub fn get_mut(&mut self, index: usize) -> &mut FilePath { &mut self.paths[index] }

    pub fn paths_iter(&self) -> std::slice::Iter<'_, FilePath> { self.paths.iter() }

    pub fn iter(&self) -> std::slice::Iter<'_, FilePath> { self.paths.iter() }

    /// Resolve filename against search paths. Returns first existing file.
    pub fn find(&self, filename: &str) -> Option<FilePath> {
        let path = Path::new(filename);
        if path.is_absolute() && path.exists() { return Some(FilePath::new(path)); }
        for base in &self.paths {
            let full = base.as_path().join(filename);
            if full.exists() { return Some(FilePath::new(full)); }
        }
        None
    }

    /// Find matching C++ semantics: returns filename unchanged if not found.
    pub fn find_cpp(&self, filename: &FilePath) -> FilePath {
        if self.paths.is_empty() || filename.is_empty() { return filename.clone(); }
        if !filename.is_absolute() {
            for path in &self.paths {
                let combined = path.join(filename);
                if combined.exists() { return combined; }
            }
        }
        filename.clone()
    }
}

impl<'a> IntoIterator for &'a FileSearchPath {
    type Item = &'a FilePath;
    type IntoIter = std::slice::Iter<'a, FilePath>;
    fn into_iter(self) -> Self::IntoIter { self.paths.iter() }
}

/// Get FileSearchPath from MATERIALX_SEARCH_PATH env var (matches C++ getEnvironmentPath).
pub fn get_environment_path() -> FileSearchPath {
    let s = environ::get_environ(environ::MATERIALX_SEARCH_PATH_ENV_VAR).unwrap_or_default();
    FileSearchPath::from_string(&s, PATH_LIST_SEPARATOR)
}

/// Read file contents to string. Returns empty string on error (matches C++ readFile).
pub fn read_file(path: &FilePath) -> String {
    std::fs::read_to_string(path.as_path()).unwrap_or_default()
}
""")

# ===== util.rs =====
(BASE / "util.rs").write_text(
r"""//! Utility functions for loading MaterialX libraries -- port of MaterialXFormat/Util.h.

use std::collections::HashSet;
use std::path::Path;

use crate::core::document::Document;
use crate::core::TreeIterator;
use crate::format::file::{
    get_environment_path, FilePath, FileSearchPath, PathFormat, MTLX_EXTENSION,
};
use crate::format::xml_io::{read_from_xml_file, XmlError, XmlReadOptions};

/// Get all subdirectories for given root directories and search paths (matches C++ getSubdirectories).
pub fn get_subdirectories(
    root_directories: &[FilePath],
    search_path: &FileSearchPath,
    sub_directories: &mut Vec<FilePath>,
) {
    for root in root_directories {
        if let Some(root_path) = search_path.find(root.as_str()) {
            if root_path.exists() {
                sub_directories.extend(root_path.get_sub_directories());
            }
        }
    }
}

/// Scan for all documents under a root path (matches C++ loadDocuments).
pub fn load_documents(
    root_path: &FilePath, search_path: &FileSearchPath,
    skip_files: &HashSet<String>, include_files: &HashSet<String>,
    documents: &mut Vec<Document>, documents_paths: &mut Vec<String>,
    read_options: Option<&XmlReadOptions>, errors: Option<&mut Vec<String>>,
) {
    let mut error_list = Vec::new();
    let errors = errors.unwrap_or(&mut error_list);
    for dir in root_path.get_sub_directories() {
        for file in dir.get_files_in_directory(MTLX_EXTENSION) {
            let file_name = file.as_string(PathFormat::Native);
            if skip_files.contains(&file_name) { continue; }
            if !include_files.is_empty() && !include_files.contains(&file_name) { continue; }
            let file_path = dir.join(&file);
            let mut read_sp = search_path.clone();
            read_sp.append(dir.clone());
            match read_from_xml_file(file_path.as_path(), read_sp, read_options) {
                Ok(doc) => {
                    documents_paths.push(file_path.as_string(PathFormat::Native));
                    documents.push(doc);
                }
                Err(e) => {
                    errors.push(format!("Failed to load: {}. Error: {}",
                        file_path.as_string(PathFormat::Native), e));
                }
            }
        }
    }
}

/// Load a single MaterialX file and import into doc (matches C++ loadLibrary).
pub fn load_library(
    file: &FilePath, doc: &mut Document,
    search_path: &FileSearchPath, read_options: Option<&XmlReadOptions>,
) -> Result<(), XmlError> {
    let lib_doc = read_from_xml_file(file.as_path(), search_path.clone(), read_options)?;
    doc.import_library(&lib_doc);
    Ok(())
}

/// Convenience: load from a path with auto-detected search path.
pub fn load_library_path(doc: &mut Document, file_path: &Path) -> Result<(), XmlError> {
    let mut sp = FileSearchPath::new();
    if let Some(parent) = file_path.parent() { sp.append(FilePath::new(parent)); }
    let fp = FilePath::new(file_path);
    load_library(&fp, doc, &sp, None)
}

/// Load all MaterialX files within given library folders (matches C++ loadLibraries).
pub fn load_libraries(
    library_folders: &[FilePath], search_path: &FileSearchPath, doc: &mut Document,
    exclude_files: &HashSet<String>, read_options: Option<&XmlReadOptions>,
) -> HashSet<String> {
    let mut lib_search_path = search_path.clone();
    lib_search_path.append_search_path(&get_environment_path());
    let mut loaded: HashSet<String> = HashSet::new();

    if library_folders.is_empty() {
        for lib_path in lib_search_path.iter() {
            for path in lib_path.get_sub_directories() {
                for filename in path.get_files_in_directory(MTLX_EXTENSION) {
                    let fname = filename.as_string(PathFormat::Native);
                    if exclude_files.contains(&fname) { continue; }
                    let file = path.join(&filename);
                    let file_str = file.as_string(PathFormat::Native);
                    if !loaded.contains(&file_str) {
                        let _ = load_library(&file, doc, search_path, read_options);
                        loaded.insert(file_str);
                    }
                }
            }
        }
    } else {
        for lib_name in library_folders {
            if let Some(lib_path) = lib_search_path.find(lib_name.as_str()) {
                for path in lib_path.get_sub_directories() {
                    for filename in path.get_files_in_directory(MTLX_EXTENSION) {
                        let fname = filename.as_string(PathFormat::Native);
                        if exclude_files.contains(&fname) { continue; }
                        let file = path.join(&filename);
                        let file_str = file.as_string(PathFormat::Native);
                        if !loaded.contains(&file_str) {
                            let _ = load_library(&file, doc, search_path, read_options);
                            loaded.insert(file_str);
                        }
                    }
                }
            }
        }
    }
    loaded
}

/// Flatten all filenames in the document (matches C++ flattenFilenames).
pub fn flatten_filenames(doc: &Document, search_path: &FileSearchPath) {
    let root = doc.get_root();
    for elem in TreeIterator::new(root.clone()) {
        let type_str = { elem.borrow().get_type().unwrap_or("").to_string() };
        if type_str != "filename" { continue; }
        let value_string = { elem.borrow().get_attribute("value").unwrap_or("").to_string() };
        if value_string.is_empty() { continue; }
        let file_prefix = elem.borrow().get_active_file_prefix();
        let unresolved = FilePath::new(&value_string);
        let mut resolved_str = if unresolved.is_absolute() {
            value_string.clone()
        } else if !file_prefix.is_empty() {
            format!("{}{}", file_prefix, value_string)
        } else {
            value_string.clone()
        };
        if !search_path.is_empty() {
            let resolved_path = FilePath::new(&resolved_str);
            if !resolved_path.is_absolute() {
                for i in 0..search_path.size() {
                    let test_path = search_path.get(i).join(&resolved_path).get_normalized();
                    if test_path.exists() {
                        resolved_str = test_path.as_string(PathFormat::Native);
                        break;
                    }
                }
            }
        }
        elem.borrow_mut().set_attribute("value", &resolved_str);
    }
    // Remove fileprefix attributes
    for elem in TreeIterator::new(root) {
        let has_fp = elem.borrow().has_file_prefix();
        if has_fp { elem.borrow_mut().remove_attribute("fileprefix"); }
    }
}

/// Return search path from source URIs in document (matches C++ getSourceSearchPath).
pub fn get_source_search_path(doc: &Document) -> FileSearchPath {
    let mut path_set: HashSet<String> = HashSet::new();
    for elem in TreeIterator::new(doc.get_root()) {
        let e = elem.borrow();
        if e.has_source_uri() {
            if let Some(uri) = e.get_source_uri() {
                let parent = FilePath::new(uri).get_parent_path();
                path_set.insert(parent.as_string(PathFormat::Native));
            }
        }
    }
    let mut sp = FileSearchPath::new();
    for p in path_set { sp.append(FilePath::new(&p)); }
    sp
}

/// Return search path to default data library folder (matches C++ getDefaultDataSearchPath).
pub fn get_default_data_search_path() -> FileSearchPath {
    let required = FilePath::new("libraries/targets");
    let mut current = FilePath::get_module_path();
    let mut sp = FileSearchPath::new();
    while !current.is_empty() {
        if current.join(&required).exists() {
            sp.append(current.clone());
            break;
        }
        current = current.get_parent_path();
    }
    sp
}
""")

# Verify
for name in ["file.rs", "environ.rs", "mod.rs", "util.rs"]:
    p = BASE / name
    print(f"{name}: {len(p.read_text().splitlines())} lines")

print("Done!")
