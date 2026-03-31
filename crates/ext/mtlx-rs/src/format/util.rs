//! Utility functions for loading MaterialX libraries -- port of MaterialXFormat/Util.h.

use std::collections::HashSet;
use std::path::PathBuf;

use crate::core::document::Document;
use crate::core::element::{ElementPtr, create_string_resolver};
// FILENAME_TYPE_STRING is re-exported from core::types via core::*
use crate::core::FILENAME_TYPE_STRING;
use crate::core::util::StringResolver;
use crate::format::file::{FilePath, FileSearchPath, PathFormat, get_environment_path};
use crate::format::xml_io::{MTLX_EXTENSION, XmlError, XmlReadOptions, read_from_xml_file};

// ── Tree walk helper ──────────────────────────────────────────────────────────

/// Collect all elements in the tree rooted at `elem` (depth-first).
fn collect_tree(elem: &ElementPtr, out: &mut Vec<ElementPtr>) {
    let children: Vec<ElementPtr> = elem.borrow().get_children().to_vec();
    for child in children {
        out.push(child.clone());
        collect_tree(&child, out);
    }
}

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Collect all `.mtlx` files inside `dir` and all of its subdirectories.
fn collect_mtlx_files(dir: &std::path::Path, out: &mut Vec<PathBuf>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    let mut subdirs: Vec<PathBuf> = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();
        let meta = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        if meta.is_dir() {
            subdirs.push(path);
        } else if meta.is_file() {
            if path.extension().and_then(|e| e.to_str()) == Some(MTLX_EXTENSION) {
                out.push(path);
            }
        }
    }

    for sub in subdirs {
        collect_mtlx_files(&sub, &mut *out);
    }
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Get all subdirectories for a given set of root directories and search paths.
/// Mirrors C++ `getSubdirectories`.
pub fn get_subdirectories(
    roots: &[FilePath],
    search_path: &FileSearchPath,
    sub_dirs: &mut Vec<FilePath>,
) {
    for root in roots {
        let resolved = search_path.find_path(root).unwrap_or_else(|| root.clone());
        if resolved.exists() {
            let children = resolved.get_sub_directories();
            sub_dirs.extend(children);
        }
    }
}

/// Scan `root_path` for `.mtlx` documents, loading those not in `skip_files`
/// and matching `include_files` (if non-empty).
///
/// Returns `(documents, paths, errors)`. Mirrors C++ `loadDocuments`.
pub fn load_documents(
    root_path: &FilePath,
    search_path: &FileSearchPath,
    skip_files: &HashSet<String>,
    include_files: &HashSet<String>,
) -> (Vec<Document>, Vec<String>, Vec<String>) {
    load_documents_with_options(root_path, search_path, skip_files, include_files, None)
}

/// Load documents with optional XmlReadOptions. Mirrors C++ `loadDocuments` full signature.
pub fn load_documents_with_options(
    root_path: &FilePath,
    search_path: &FileSearchPath,
    skip_files: &HashSet<String>,
    include_files: &HashSet<String>,
    read_options: Option<&XmlReadOptions>,
) -> (Vec<Document>, Vec<String>, Vec<String>) {
    let mut documents = Vec::new();
    let mut doc_paths = Vec::new();
    let mut errors = Vec::new();

    for dir in root_path.get_sub_directories() {
        for file in dir.get_files_in_directory(MTLX_EXTENSION) {
            let file_name = file.get_base_name().to_owned();
            if skip_files.contains(&file_name) {
                continue;
            }
            if !include_files.is_empty() && !include_files.contains(&file_name) {
                continue;
            }

            let file_path = dir.clone() / &file;
            let mut read_sp = search_path.clone();
            read_sp.append(dir.clone());

            match read_from_xml_file(file_path.as_path(), read_sp, read_options) {
                Ok(doc) => {
                    doc_paths.push(file_path.as_string(PathFormat::Native));
                    documents.push(doc);
                }
                Err(e) => {
                    errors.push(format!("Failed to load: {}. Error: {}", file_path, e));
                }
            }
        }
    }

    (documents, doc_paths, errors)
}

/// Load a single `.mtlx` file and import it into `doc`.
/// Mirrors C++ `loadLibrary(file, doc, searchPath, readOptions)`.
pub fn load_library(doc: &mut Document, file_path: &std::path::Path) -> Result<(), XmlError> {
    load_library_with_options(doc, file_path, &FileSearchPath::new(), None)
}

/// Load a library with explicit search path and read options.
/// Full C++ parity signature.
pub fn load_library_with_options(
    doc: &mut Document,
    file_path: &std::path::Path,
    search_path: &FileSearchPath,
    read_options: Option<&XmlReadOptions>,
) -> Result<(), XmlError> {
    let mut sp = search_path.clone();
    // Also add parent directory for relative xi:include resolution
    if let Some(parent) = file_path.parent() {
        sp.append(FilePath::new(parent));
    }
    let lib_doc = read_from_xml_file(file_path, sp, read_options)?;
    doc.import_library(&lib_doc);
    Ok(())
}

/// Load all MaterialX files within the given library folders into a document.
/// Mirrors C++ `loadLibraries(libraryFolders, searchPath, doc, excludeFiles, readOptions)`.
pub fn load_libraries(
    doc: &mut Document,
    search_paths: &[PathBuf],
    library_names: &[&str],
) -> Result<Vec<PathBuf>, XmlError> {
    load_libraries_with_options(doc, search_paths, library_names, &HashSet::new(), None)
}

/// Load libraries with exclude set and read options.
/// Full C++ parity: accepts `exclude_files` set and `read_options`.
pub fn load_libraries_with_options(
    doc: &mut Document,
    search_paths: &[PathBuf],
    library_names: &[&str],
    exclude_files: &HashSet<String>,
    read_options: Option<&XmlReadOptions>,
) -> Result<Vec<PathBuf>, XmlError> {
    // Build original search path (without env) for loadLibrary calls
    // C++ passes the original searchPath to loadLibrary, not the augmented one.
    let mut original_sp = FileSearchPath::new();
    for p in search_paths {
        original_sp.append(FilePath::new(p));
    }

    // Build library search path (with env) for finding library folders
    let mut sp = original_sp.clone();
    sp.append_search_path(&get_environment_path());

    let mut loaded: HashSet<PathBuf> = HashSet::new();
    let mut ordered: Vec<PathBuf> = Vec::new();

    if library_names.is_empty() {
        // No specific folders: scan every root in the search path
        for base_fp in sp.paths_iter() {
            let base = base_fp.as_path();
            let mut files = Vec::new();
            collect_mtlx_files(base, &mut files);
            for file in files {
                let fname = file
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or_default();
                if exclude_files.contains(fname) {
                    continue;
                }
                if loaded.insert(file.clone()) {
                    load_library_with_options(doc, &file, &original_sp, read_options)?;
                    ordered.push(file);
                }
            }
        }
    } else {
        // Look for each named library folder in the search path
        for name in library_names {
            if let Some(lib_fp) = sp.find(name) {
                let lib_dir = lib_fp.as_path().to_path_buf();
                let mut files = Vec::new();
                collect_mtlx_files(&lib_dir, &mut files);
                for file in files {
                    let fname = file
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or_default();
                    if exclude_files.contains(fname) {
                        continue;
                    }
                    if loaded.insert(file.clone()) {
                        load_library_with_options(doc, &file, &original_sp, read_options)?;
                        ordered.push(file);
                    }
                }
            }
        }
    }

    Ok(ordered)
}

/// Flatten all `filename`-typed value elements in `doc` to absolute paths.
///
/// Creates an element-level StringResolver (with fileprefix) for each element,
/// resolves against `search_path`, and optionally applies `custom_resolver`.
/// Then removes all fileprefix attributes.
///
/// Mirrors C++ `flattenFilenames(doc, searchPath, customResolver)`.
pub fn flatten_filenames(
    doc: &mut Document,
    search_path: &FileSearchPath,
    custom_resolver: Option<&dyn Fn(&str) -> String>,
) {
    let root = doc.get_root();

    // Collect all elements in the tree
    let mut all_elems: Vec<ElementPtr> = vec![root.clone()];
    collect_tree(&root, &mut all_elems);

    // Gather updates: (elem, new_value_string)
    let mut to_update: Vec<(ElementPtr, String)> = Vec::new();

    for elem in &all_elems {
        let type_attr = {
            let borrow = elem.borrow();
            borrow.get_type().map(|s| s.to_owned())
        };

        if type_attr.as_deref() != Some(FILENAME_TYPE_STRING) {
            continue;
        }

        let value_str = elem.borrow().get_value_string();
        if value_str.is_empty() {
            continue;
        }

        // Create element-level string resolver (includes fileprefix from ancestors)
        let mut elem_resolver = create_string_resolver(elem, "");
        let unresolved = FilePath::from(value_str.as_str());

        // If path is already absolute, clear fileprefix to avoid invalid double-prefix
        if unresolved.is_absolute() {
            elem_resolver.set_file_prefix("");
        }

        // Apply element resolver (prepends fileprefix, substitutes tokens)
        let mut resolved_str = elem_resolver.resolve(&value_str, FILENAME_TYPE_STRING);

        // Convert relative to absolute via search path
        if !search_path.is_empty() {
            let resolved_path = FilePath::from(resolved_str.as_str());
            if !resolved_path.is_absolute() {
                for i in 0..search_path.size() {
                    let test = search_path[i].clone() / &resolved_path;
                    let test = test.get_normalized();
                    if test.exists() {
                        resolved_str = test.as_string(PathFormat::Native);
                        break;
                    }
                }
            }
        }

        // Apply custom resolver if provided (matches C++ isResolvedType check)
        if let Some(resolver) = custom_resolver {
            if StringResolver::is_resolved_type(FILENAME_TYPE_STRING) {
                resolved_str = resolver(&resolved_str);
            }
        }

        to_update.push((elem.clone(), resolved_str));
    }

    // Apply value updates
    for (elem, new_val) in to_update {
        elem.borrow_mut().set_value_string(&new_val);
    }

    // Remove fileprefix attributes from all elements
    for elem in &all_elems {
        if elem.borrow().has_file_prefix() {
            elem.borrow_mut().remove_attribute("fileprefix");
        }
    }
}

/// Return a `FileSearchPath` containing the parent directory of each source URI.
/// Mirrors C++ `getSourceSearchPath`.
pub fn get_source_search_path(doc: &Document) -> FileSearchPath {
    use std::collections::BTreeSet;

    let root = doc.get_root();
    let mut all_elems: Vec<ElementPtr> = vec![root.clone()];
    collect_tree(&root, &mut all_elems);

    let mut path_set: BTreeSet<String> = BTreeSet::new();

    for elem in &all_elems {
        let borrow = elem.borrow();
        if let Some(uri) = borrow.get_source_uri() {
            if !uri.is_empty() {
                let parent = FilePath::from(uri).get_parent_path();
                path_set.insert(parent.as_string(PathFormat::Native));
            }
        }
    }

    let mut sp = FileSearchPath::new();
    for p in path_set {
        sp.append(FilePath::from(p.as_str()));
    }
    sp
}

/// Return a `FileSearchPath` pointing to the MaterialX data library root.
/// Mirrors C++ `getDefaultDataSearchPath`.
pub fn get_default_data_search_path() -> FileSearchPath {
    let required = FilePath::from("libraries/targets");
    let mut current = FilePath::get_module_path();
    let mut sp = FileSearchPath::new();

    loop {
        if current.is_empty() {
            break;
        }
        let candidate = current.clone() / &required;
        if candidate.exists() {
            sp.append(current.clone());
            break;
        }
        let parent = current.get_parent_path();
        if parent == current || parent.is_empty() {
            break;
        }
        current = parent;
    }
    sp
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::document::create_document;

    #[test]
    fn test_get_subdirectories_empty() {
        let sp = FileSearchPath::new();
        let mut result = Vec::new();
        get_subdirectories(&[], &sp, &mut result);
        assert!(result.is_empty());
    }

    #[test]
    fn test_load_documents_empty_dir() {
        let tmp = std::env::temp_dir();
        let root = FilePath::new(&tmp);
        let sp = FileSearchPath::new();
        let (docs, paths, errs) = load_documents(&root, &sp, &HashSet::new(), &HashSet::new());
        let _ = (docs, paths, errs);
    }

    #[test]
    fn test_flatten_filenames_noop() {
        let mut doc = create_document();
        let sp = FileSearchPath::new();
        flatten_filenames(&mut doc, &sp, None);
    }

    #[test]
    fn test_get_source_search_path_empty() {
        let doc = create_document();
        let sp = get_source_search_path(&doc);
        assert!(sp.is_empty());
    }

    #[test]
    fn test_get_default_data_search_path_runs() {
        let _sp = get_default_data_search_path();
    }

    #[test]
    fn test_flatten_filenames_with_fileprefix() {
        use crate::core::element::add_child_of_category;
        let mut doc = create_document();
        let root = doc.get_root();
        root.borrow_mut().set_file_prefix("textures/");

        // Add a child with type=filename and a value
        if let Ok(child) = add_child_of_category(&root, "input", "tex") {
            child.borrow_mut().set_attribute("type", "filename");
            child.borrow_mut().set_value_string("diffuse.png");
        }

        let sp = FileSearchPath::new();
        flatten_filenames(&mut doc, &sp, None);

        // After flattening, the value should have the fileprefix applied
        let root = doc.get_root();
        if let Some(child) = root.borrow().get_child("tex") {
            let val = child.borrow().get_value_string();
            assert!(
                val.contains("textures/"),
                "flatten should apply fileprefix: got '{}'",
                val
            );
        }

        // fileprefix should be removed
        assert!(
            !root.borrow().has_file_prefix(),
            "fileprefix attribute should be removed after flatten"
        );
    }
}
