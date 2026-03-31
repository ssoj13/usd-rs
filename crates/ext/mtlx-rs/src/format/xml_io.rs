//! XML read/write for MaterialX documents -- port of MaterialXFormat/XmlIo.h/.cpp.

use indexmap::IndexMap;
use std::collections::HashSet;
use std::io::{Read, Write};
use std::path::Path;

use quick_xml::Reader;
use quick_xml::events::Event;

use crate::core::document::{Document, create_document};
use crate::core::element::{ElementPtr, add_child_of_category, category, change_child_category};
use crate::format::file::{self, FilePath, FileSearchPath, PathFormat};

pub const MTLX_EXTENSION: &str = "mtlx";

const XINCLUDE_TAG: &str = "xi:include";
const XINCLUDE_NAMESPACE: &str = "xmlns:xi";
const XINCLUDE_URL: &str = "http://www.w3.org/2001/XInclude";
const MAX_XINCLUDE_DEPTH: usize = 256;

/// Maximum allowed XML tree depth to prevent stack overflow on malicious input.
pub const MAX_XML_TREE_DEPTH: usize = 256;

/// Element predicate function type for filtering elements during write.
pub type ElementPredicate = Box<dyn Fn(&ElementPtr) -> bool>;

// ── Read options ──────────────────────────────────────────────────────────────

/// Options for reading XML. Mirrors C++ `XmlReadOptions`.
#[derive(Clone)]
pub struct XmlReadOptions {
    /// Enable XInclude processing (internal flag, set automatically by read_from_xml_file).
    pub read_xinclude: bool,
    /// Search path for resolving XInclude hrefs.
    pub search_path: Option<FileSearchPath>,
    /// Parent XInclude hrefs for cycle detection (internal).
    pub parent_xincludes: Vec<String>,
    /// If true, XML comments are read into the document as CommentElements. Default: false.
    pub read_comments: bool,
    /// If true, XML newlines between elements are preserved as NewlineElements. Default: false.
    pub read_newlines: bool,
    /// If true, documents from older MaterialX versions are upgraded. Default: true.
    pub upgrade_version: bool,
}

impl Default for XmlReadOptions {
    fn default() -> Self {
        Self {
            read_xinclude: false,
            search_path: None,
            parent_xincludes: Vec::new(),
            read_comments: false,
            read_newlines: false,
            upgrade_version: true,
        }
    }
}

// ── Write options ─────────────────────────────────────────────────────────────

/// Options for writing XML. Mirrors C++ `XmlWriteOptions`.
pub struct XmlWriteOptions {
    /// If true, elements with sourceUri are written as xi:include. Default: true.
    pub write_xinclude_enable: bool,
    /// Optional predicate to exclude elements from write (return false = skip).
    pub element_predicate: Option<ElementPredicate>,
}

impl Default for XmlWriteOptions {
    fn default() -> Self {
        Self {
            write_xinclude_enable: true,
            element_predicate: None,
        }
    }
}

// ── Internal XML node representation ──────────────────────────────────────────

/// Node type for parsed XML (tracks comments/newlines).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum XmlNodeType {
    Element,
    Comment,
    Newline,
}

/// Intermediate XML node used during parsing (before building the Element tree).
/// Uses IndexMap to preserve XML attribute order through the read->write roundtrip.
struct XmlNode {
    name: String,
    attrs: IndexMap<String, String>,
    children: Vec<XmlNode>,
    node_type: XmlNodeType,
    /// Comment text (only for Comment nodes).
    comment_text: String,
}

impl XmlNode {
    fn element(name: String, attrs: IndexMap<String, String>) -> Self {
        Self {
            name,
            attrs,
            children: Vec::new(),
            node_type: XmlNodeType::Element,
            comment_text: String::new(),
        }
    }

    fn comment(text: String) -> Self {
        Self {
            name: String::new(),
            attrs: IndexMap::new(),
            children: Vec::new(),
            node_type: XmlNodeType::Comment,
            comment_text: text,
        }
    }

    fn newline() -> Self {
        Self {
            name: String::new(),
            attrs: IndexMap::new(),
            children: Vec::new(),
            node_type: XmlNodeType::Newline,
            comment_text: String::new(),
        }
    }
}

// ── XML parsing ───────────────────────────────────────────────────────────────

fn parse_xml_to_nodes(xml: &str, options: &XmlReadOptions) -> Result<XmlNode, XmlError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(!options.read_newlines);

    let mut buf = Vec::new();
    let mut stack: Vec<XmlNode> = vec![];

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).into_owned();
                let mut attrs = IndexMap::new();
                for attr in e.attributes().flatten() {
                    let key = String::from_utf8_lossy(attr.key.as_ref()).into_owned();
                    let value = String::from_utf8_lossy(&attr.value).into_owned();
                    attrs.insert(key, value);
                }
                stack.push(XmlNode::element(name, attrs));
            }
            Ok(Event::End(_)) => {
                if let Some(node) = stack.pop() {
                    if let Some(parent) = stack.last_mut() {
                        parent.children.push(node);
                    } else {
                        // Root node -- push back
                        stack.push(node);
                    }
                }
            }
            Ok(Event::Empty(e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).into_owned();
                let mut attrs = IndexMap::new();
                for attr in e.attributes().flatten() {
                    let key = String::from_utf8_lossy(attr.key.as_ref()).into_owned();
                    let value = String::from_utf8_lossy(&attr.value).into_owned();
                    attrs.insert(key, value);
                }
                let node = XmlNode::element(name, attrs);
                if let Some(parent) = stack.last_mut() {
                    parent.children.push(node);
                }
            }
            Ok(Event::Comment(e)) => {
                if options.read_comments {
                    let text = String::from_utf8_lossy(e.as_ref()).trim().to_string();
                    if let Some(parent) = stack.last_mut() {
                        parent.children.push(XmlNode::comment(text));
                    }
                }
            }
            Ok(Event::Text(e)) => {
                // Detect whitespace-only text nodes as newlines
                if options.read_newlines {
                    let raw = String::from_utf8_lossy(e.as_ref());
                    if raw.contains('\n') {
                        if let Some(parent) = stack.last_mut() {
                            parent.children.push(XmlNode::newline());
                        }
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(XmlError::Parse(e)),
            _ => {}
        }
        buf.clear();
    }

    stack.into_iter().next().ok_or(XmlError::EmptyDocument)
}

// ── Element tree construction ─────────────────────────────────────────────────

/// Build Element tree from parsed XML nodes.
/// Enforces MAX_XML_TREE_DEPTH and skips duplicate child names (mirrors C++).
fn xml_node_to_element(
    parent: &ElementPtr,
    xml: &XmlNode,
    options: &XmlReadOptions,
    depth: usize,
) -> Result<(), XmlError> {
    match xml.node_type {
        XmlNodeType::Comment => {
            // Create a comment element if read_comments is enabled
            if options.read_comments {
                let count = parent.borrow().get_children().len();
                let gen_name = format!("comment{}", count + 1);
                if let Ok(child) = add_child_of_category(parent, "", &gen_name) {
                    if let Some(new_child) =
                        change_child_category(parent, &gen_name, category::COMMENT)
                    {
                        new_child.borrow_mut().set_doc_string(&xml.comment_text);
                    } else {
                        child.borrow_mut().set_doc_string(&xml.comment_text);
                    }
                }
            }
            return Ok(());
        }
        XmlNodeType::Newline => {
            // Create a newline element if read_newlines is enabled
            if options.read_newlines {
                let count = parent.borrow().get_children().len();
                let gen_name = format!("newline{}", count + 1);
                if let Ok(_child) = add_child_of_category(parent, "", &gen_name) {
                    change_child_category(parent, &gen_name, category::NEWLINE);
                }
            }
            return Ok(());
        }
        XmlNodeType::Element => {}
    }

    // Skip xi:include elements (handled separately in document_from_xml_nodes)
    if xml.name == XINCLUDE_TAG {
        return Ok(());
    }

    let name = xml.attrs.get("name").cloned().unwrap_or_default();

    // Skip duplicate children (matches C++: if previous child with same name exists, skip)
    if !name.is_empty() {
        if parent.borrow().get_child(&name).is_some() {
            return Ok(());
        }
    }

    // Enforce maximum tree depth
    if depth >= MAX_XML_TREE_DEPTH {
        return Err(XmlError::MaxTreeDepth);
    }

    let child = add_child_of_category(parent, &xml.name, &name).map_err(XmlError::AddChild)?;

    // Set attributes (skip "name" which was already handled by add_child_of_category)
    for (k, v) in &xml.attrs {
        if k != "name" {
            child.borrow_mut().set_attribute(k, v);
        }
    }

    // Recurse into children
    for c in &xml.children {
        xml_node_to_element(&child, c, options, depth + 1)?;
    }

    Ok(())
}

// ── Document construction from XML ────────────────────────────────────────────

/// Build document from parsed XML nodes. Processes XInclude + version upgrade.
fn document_from_xml_nodes(
    doc: &mut Document,
    root_xml: &XmlNode,
    search_path: Option<&mut FileSearchPath>,
    options: &XmlReadOptions,
) -> Result<(), XmlError> {
    let doc_root = doc.get_root();

    // Process XInclude directives when search_path is provided
    if let Some(sp) = search_path {
        for child_xml in &root_xml.children {
            if child_xml.name != XINCLUDE_TAG {
                continue;
            }
            let href = child_xml
                .attrs
                .get("href")
                .map(|s| s.as_str())
                .unwrap_or("");
            if href.is_empty() {
                continue;
            }

            if options.parent_xincludes.contains(&href.to_string()) {
                return Err(XmlError::XIncludeCycle(href.to_string()));
            }
            if options.parent_xincludes.len() >= MAX_XINCLUDE_DEPTH {
                return Err(XmlError::XIncludeMaxDepth);
            }

            let mut xi_parents = options.parent_xincludes.clone();
            xi_parents.push(href.to_string());

            let resolved = sp
                .find(href)
                .ok_or_else(|| XmlError::FileMissing(href.to_string()))?;

            let xml = std::fs::read_to_string(resolved.as_path()).map_err(XmlError::Io)?;
            let lib_doc = read_from_xml_str_with_options(
                &xml,
                &XmlReadOptions {
                    search_path: {
                        let mut inner = sp.clone();
                        inner.prepend(resolved.get_parent_path());
                        Some(inner)
                    },
                    parent_xincludes: xi_parents,
                    read_comments: options.read_comments,
                    read_newlines: options.read_newlines,
                    upgrade_version: options.upgrade_version,
                    read_xinclude: true,
                },
            )?;
            doc.import_library(&lib_doc);
        }
    }

    // Set root attributes
    for (k, v) in &root_xml.attrs {
        doc_root.borrow_mut().set_attribute(k, v);
    }
    if let Some(name) = root_xml.attrs.get("name") {
        doc_root.borrow_mut().set_name(name).ok();
    }

    // Build child elements (depth starts at 1, matching C++)
    for child_xml in &root_xml.children {
        xml_node_to_element(&doc_root, child_xml, options, 1)?;
    }

    // Upgrade version if requested (matches C++ documentFromXml)
    if options.upgrade_version {
        doc.upgrade_version();
    }

    Ok(())
}

// ── Public read API ───────────────────────────────────────────────────────────

/// Read a MaterialX document from XML string (default options).
pub fn read_from_xml_str(xml: &str) -> Result<Document, XmlError> {
    read_from_xml_str_with_options(xml, &XmlReadOptions::default())
}

/// Read a MaterialX document from XML string with options.
pub fn read_from_xml_str_with_options(
    xml: &str,
    options: &XmlReadOptions,
) -> Result<Document, XmlError> {
    let root_xml = parse_xml_to_nodes(xml, options)?;

    if root_xml.name != "materialx" {
        return Err(XmlError::InvalidRoot(root_xml.name));
    }

    let mut doc = create_document();

    // Only pass search_path for XInclude processing when read_xinclude is enabled
    let mut search_path = if options.read_xinclude {
        options.search_path.clone()
    } else {
        None
    };
    document_from_xml_nodes(&mut doc, &root_xml, search_path.as_mut(), options)?;

    Ok(doc)
}

/// Read a MaterialX document from file. Resolves path via search_path + env. Processes XInclude.
pub fn read_from_xml_file(
    path: impl AsRef<Path>,
    search_path: FileSearchPath,
    options: Option<&XmlReadOptions>,
) -> Result<Document, XmlError> {
    let mut sp = search_path;
    sp.append_search_path(&file::get_environment_path());
    let path_str = path.as_ref().to_string_lossy();
    let resolved = sp
        .find(&path_str)
        .ok_or_else(|| XmlError::FileMissing(path_str.to_string()))?;
    let xml = std::fs::read_to_string(resolved.as_path()).map_err(XmlError::Io)?;

    // Store source URI (matches C++: use parentXIncludes[0] if available)
    let source_path = if let Some(opts) = options {
        if !opts.parent_xincludes.is_empty() {
            FilePath::from(opts.parent_xincludes[0].as_str())
        } else {
            resolved.clone()
        }
    } else {
        resolved.clone()
    };

    // Prepend parent of current file so XInclude hrefs resolve relative to it
    let source_for_search = if !source_path.is_absolute() {
        sp.find_path(&source_path).unwrap_or(source_path.clone())
    } else {
        source_path.clone()
    };
    sp.prepend(source_for_search.get_parent_path());

    let mut opts = options.cloned().unwrap_or_default();
    opts.search_path = Some(sp);
    opts.read_xinclude = true;

    let doc = read_from_xml_str_with_options(&xml, &opts)?;

    let uri = source_path.as_string(PathFormat::Posix);
    doc.get_root().borrow_mut().set_source_uri(Some(uri));
    Ok(doc)
}

/// Convenience: read from file using parent directory and env as search path.
pub fn read_from_xml_file_path(path: impl AsRef<Path>) -> Result<Document, XmlError> {
    let p = path.as_ref();
    let mut sp = FileSearchPath::new();
    if let Some(parent) = p.parent() {
        sp.append(FilePath::new(parent));
    }
    read_from_xml_file(p, sp, None)
}

/// Read a document from a byte buffer (in-memory XML). Mirrors C++ `readFromXmlBuffer`.
pub fn read_from_xml_buffer(
    buffer: &[u8],
    search_path: FileSearchPath,
    options: Option<&XmlReadOptions>,
) -> Result<Document, XmlError> {
    let s = std::str::from_utf8(buffer).map_err(XmlError::Utf8)?;
    let mut sp = search_path;
    sp.append_search_path(&file::get_environment_path());
    let mut opts = options.cloned().unwrap_or_default();
    if opts.search_path.is_none() && !sp.is_empty() {
        opts.search_path = Some(sp);
        opts.read_xinclude = true;
    }
    read_from_xml_str_with_options(s, &opts)
}

/// Read a Document as XML from any `Read` implementor.
/// Mirrors C++ `readFromXmlStream`.
pub fn read_from_xml_stream(
    reader: &mut dyn Read,
    search_path: FileSearchPath,
    options: Option<&XmlReadOptions>,
) -> Result<Document, XmlError> {
    let mut buf = Vec::new();
    reader.read_to_end(&mut buf).map_err(XmlError::Io)?;
    read_from_xml_buffer(&buf, search_path, options)
}

// ── Public write API ──────────────────────────────────────────────────────────

/// Write document to XML string (default options).
pub fn write_to_xml_string(doc: &Document) -> Result<String, XmlError> {
    write_to_xml_string_with_options(doc, &XmlWriteOptions::default())
}

/// Write document to XML string with options.
pub fn write_to_xml_string_with_options(
    doc: &Document,
    options: &XmlWriteOptions,
) -> Result<String, XmlError> {
    let mut out = Vec::new();
    out.write_all(b"<?xml version=\"1.0\"?>\n")?;
    let root = doc.get_root();
    let mut written_source_files: HashSet<String> = HashSet::new();
    write_element_to_xml(&root, &mut out, 0, options, doc, &mut written_source_files)?;
    Ok(String::from_utf8(out).map_err(|e| XmlError::Utf8(e.utf8_error()))?)
}

/// Write document to an XML file on disk. Mirrors C++ `writeToXmlFile`.
pub fn write_to_xml_file(doc: &Document, filename: &FilePath) -> Result<(), XmlError> {
    let xml = write_to_xml_string(doc)?;
    std::fs::write(filename.as_path(), xml.as_bytes()).map_err(XmlError::Io)?;
    Ok(())
}

/// Write document to an XML file with options.
pub fn write_to_xml_file_with_options(
    doc: &Document,
    filename: &FilePath,
    options: &XmlWriteOptions,
) -> Result<(), XmlError> {
    let xml = write_to_xml_string_with_options(doc, options)?;
    std::fs::write(filename.as_path(), xml.as_bytes()).map_err(XmlError::Io)?;
    Ok(())
}

/// Write a Document as XML to any `Write` implementor.
/// Mirrors C++ `writeToXmlStream`.
pub fn write_to_xml_stream(
    doc: &Document,
    writer: &mut dyn Write,
    options: Option<&XmlWriteOptions>,
) -> Result<(), XmlError> {
    let xml = match options {
        Some(opts) => write_to_xml_string_with_options(doc, opts)?,
        None => write_to_xml_string(doc)?,
    };
    writer.write_all(xml.as_bytes()).map_err(XmlError::Io)?;
    Ok(())
}

// ── Internal write logic ──────────────────────────────────────────────────────

fn write_element_to_xml(
    elem: &ElementPtr,
    w: &mut Vec<u8>,
    indent: usize,
    options: &XmlWriteOptions,
    doc: &Document,
    written_source_files: &mut HashSet<String>,
) -> Result<(), XmlError> {
    // Collect all data from the borrow, then release it before recursion
    let (tag, name, children, attrs) = {
        let e = elem.borrow();
        let tag = e.get_category().to_owned();
        let name = e.get_name().to_owned();
        let children = e.get_children().to_vec();
        let attrs: Vec<(String, String)> = e
            .iter_attributes()
            .map(|(k, v)| (k.to_owned(), v.to_owned()))
            .collect();
        (tag, name, children, attrs)
    };

    // Write opening tag with indent
    for _ in 0..indent {
        w.write_all(b"  ")?;
    }
    w.write_all(b"<")?;
    w.write_all(tag.as_bytes())?;

    // "name" attribute comes first (matches C++ convention)
    if !name.is_empty() {
        w.write_all(b" name=\"")?;
        escape_attr(&name, w)?;
        w.write_all(b"\"")?;
    }

    // Remaining attributes in insertion order
    for (k, v) in &attrs {
        if k == "name" {
            continue;
        }
        w.write_all(b" ")?;
        w.write_all(k.as_bytes())?;
        w.write_all(b"=\"")?;
        escape_attr(v, w)?;
        w.write_all(b"\"")?;
    }

    if children.is_empty() {
        w.write_all(b" />\n")?;
        return Ok(());
    }

    // Track if we need the XInclude namespace attribute
    let doc_source_uri = doc
        .get_root()
        .borrow()
        .get_source_uri()
        .unwrap_or_default()
        .to_owned();
    // Check if any child needs XInclude -- if so, inject namespace into opening tag
    if options.write_xinclude_enable {
        let needs_xinclude_ns = children.iter().any(|child| {
            let cb = child.borrow();
            if cb.has_source_uri() {
                let source_uri = cb.get_source_uri().unwrap_or_default().to_owned();
                !source_uri.is_empty() && source_uri != doc_source_uri
            } else {
                false
            }
        });
        if needs_xinclude_ns {
            w.write_all(b" ")?;
            w.write_all(XINCLUDE_NAMESPACE.as_bytes())?;
            w.write_all(b"=\"")?;
            w.write_all(XINCLUDE_URL.as_bytes())?;
            w.write_all(b"\"")?;
        }
    }

    w.write_all(b">\n")?;

    for child in &children {
        // Apply element predicate filter
        if let Some(ref predicate) = options.element_predicate {
            if !predicate(child) {
                continue;
            }
        }

        let child_borrow = child.borrow();
        let child_cat = child_borrow.get_category().to_owned();
        let child_source_uri = child_borrow.get_source_uri().unwrap_or_default().to_owned();
        drop(child_borrow);

        // Write XInclude references for elements with foreign sourceUri
        if options.write_xinclude_enable && !child_source_uri.is_empty() {
            if child_source_uri != doc_source_uri {
                if !written_source_files.contains(&child_source_uri) {
                    // Write xi:include element
                    for _ in 0..indent + 1 {
                        w.write_all(b"  ")?;
                    }
                    let include_path = FilePath::from(child_source_uri.as_str());
                    // Relative paths in Posix format, absolute in native
                    let include_format = if include_path.is_absolute() {
                        PathFormat::Native
                    } else {
                        PathFormat::Posix
                    };
                    w.write_all(b"<xi:include href=\"")?;
                    escape_attr(&include_path.as_string(include_format), w)?;
                    w.write_all(b"\" />\n")?;

                    written_source_files.insert(child_source_uri);
                }
                continue;
            }
        }

        // Handle CommentElement -- write as XML comment
        if child_cat == category::COMMENT {
            for _ in 0..indent + 1 {
                w.write_all(b"  ")?;
            }
            let doc_str = child.borrow().get_doc_string();
            w.write_all(b"<!--")?;
            w.write_all(doc_str.as_bytes())?;
            w.write_all(b"-->\n")?;
            continue;
        }

        // Handle NewlineElement -- write as empty line
        if child_cat == category::NEWLINE {
            w.write_all(b"\n")?;
            continue;
        }

        // Normal element -- recurse
        write_element_to_xml(child, w, indent + 1, options, doc, written_source_files)?;
    }

    for _ in 0..indent {
        w.write_all(b"  ")?;
    }
    w.write_all(b"</")?;
    w.write_all(tag.as_bytes())?;
    w.write_all(b">\n")?;

    Ok(())
}

fn escape_attr(s: &str, w: &mut Vec<u8>) -> Result<(), std::io::Error> {
    for c in s.chars() {
        match c {
            '"' => w.write_all(b"&quot;")?,
            '&' => w.write_all(b"&amp;")?,
            '<' => w.write_all(b"&lt;")?,
            '>' => w.write_all(b"&gt;")?,
            _ => {
                let mut buf = [0u8; 4];
                let encoded = c.encode_utf8(&mut buf);
                w.write_all(encoded.as_bytes())?;
            }
        }
    }
    Ok(())
}

// ── Edit functions ────────────────────────────────────────────────────────────

/// Add an XInclude reference at the top of `doc` (position 0).
/// Mirrors C++ `prependXInclude`.
pub fn prepend_xinclude(doc: &mut Document, filename: &FilePath) {
    if filename.is_empty() {
        return;
    }
    // C++ uses asString() which defaults to FormatNative
    let uri = filename.as_string(PathFormat::Native);
    let root = doc.get_root();

    if let Ok(child) = add_child_of_category(&root, "xinclude", "") {
        child.borrow_mut().set_source_uri(Some(uri));
        let child_name = child.borrow().get_name().to_owned();
        let _ = root.borrow_mut().set_child_index(&child_name, 0);
    }
}

// ── Error type ────────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum XmlError {
    #[error("XML parse error: {0}")]
    Parse(#[from] quick_xml::Error),
    #[error("Empty document")]
    EmptyDocument,
    #[error("Expected materialx root, got: {0}")]
    InvalidRoot(String),
    #[error("Add child error: {0}")]
    AddChild(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("UTF-8 error: {0}")]
    Utf8(std::str::Utf8Error),
    #[error("File not found: {0}")]
    FileMissing(String),
    #[error("XInclude cycle detected: {0}")]
    XIncludeCycle(String),
    #[error("XInclude maximum depth exceeded")]
    XIncludeMaxDepth,
    #[error("Maximum XML tree depth exceeded")]
    MaxTreeDepth,
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const SIMPLE_MTLX: &str = r#"<?xml version="1.0"?>
<materialx version="1.39">
  <nodegraph name="NG_basic">
    <constant name="val" type="float">
      <input name="value" type="float" value="0.5" />
    </constant>
  </nodegraph>
</materialx>
"#;

    #[test]
    fn read_from_xml_stream_roundtrip() {
        let doc = read_from_xml_str(SIMPLE_MTLX).unwrap();
        let xml_str = write_to_xml_string(&doc).unwrap();

        let mut cursor = std::io::Cursor::new(xml_str.as_bytes());
        let doc2 = read_from_xml_stream(&mut cursor, FileSearchPath::new(), None).unwrap();
        let xml_str2 = write_to_xml_string(&doc2).unwrap();

        assert_eq!(
            xml_str, xml_str2,
            "stream round-trip should produce identical XML"
        );
    }

    #[test]
    fn write_to_xml_stream_bytes() {
        let doc = read_from_xml_str(SIMPLE_MTLX).unwrap();

        let mut buf: Vec<u8> = Vec::new();
        write_to_xml_stream(&doc, &mut buf, None).unwrap();

        let xml_via_stream = String::from_utf8(buf).unwrap();
        let xml_via_string = write_to_xml_string(&doc).unwrap();

        assert_eq!(
            xml_via_stream, xml_via_string,
            "stream write should match string write"
        );
    }

    #[test]
    fn write_to_xml_stream_with_options() {
        let doc = read_from_xml_str(SIMPLE_MTLX).unwrap();
        let opts = XmlWriteOptions {
            write_xinclude_enable: false,
            ..Default::default()
        };

        let mut buf: Vec<u8> = Vec::new();
        write_to_xml_stream(&doc, &mut buf, Some(&opts)).unwrap();

        let xml = String::from_utf8(buf).unwrap();
        assert!(xml.starts_with("<?xml"), "must start with XML declaration");
    }

    #[test]
    fn read_comments() {
        let xml = r#"<?xml version="1.0"?>
<materialx version="1.39">
  <!-- This is a test comment -->
  <nodegraph name="NG_test" />
</materialx>
"#;
        let opts = XmlReadOptions {
            read_comments: true,
            ..Default::default()
        };
        let doc = read_from_xml_str_with_options(xml, &opts).unwrap();
        let root = doc.get_root();
        let children = root.borrow().get_children().to_vec();
        // Should have a comment element and the nodegraph
        let has_comment = children
            .iter()
            .any(|c| c.borrow().get_category() == category::COMMENT);
        assert!(has_comment, "should parse XML comment into CommentElement");
    }

    #[test]
    fn write_comment_element() {
        let xml = r#"<?xml version="1.0"?>
<materialx version="1.39">
  <!-- Test comment -->
  <nodegraph name="NG_test" />
</materialx>
"#;
        let opts = XmlReadOptions {
            read_comments: true,
            ..Default::default()
        };
        let doc = read_from_xml_str_with_options(xml, &opts).unwrap();
        let output = write_to_xml_string(&doc).unwrap();
        assert!(
            output.contains("<!--"),
            "written XML should contain comment: {}",
            output
        );
    }

    #[test]
    fn duplicate_children_skipped() {
        let xml = r#"<?xml version="1.0"?>
<materialx version="1.39">
  <nodegraph name="NG_dup" />
  <nodegraph name="NG_dup" />
</materialx>
"#;
        let doc = read_from_xml_str(xml).unwrap();
        let root = doc.get_root();
        let children = root.borrow().get_children().to_vec();
        let ng_count = children
            .iter()
            .filter(|c| c.borrow().get_name() == "NG_dup")
            .count();
        assert_eq!(ng_count, 1, "duplicate children should be skipped");
    }

    #[test]
    fn max_tree_depth_enforced() {
        // Build an XML with depth > MAX_XML_TREE_DEPTH
        let mut xml = String::from("<?xml version=\"1.0\"?>\n<materialx version=\"1.39\">\n");
        for i in 0..MAX_XML_TREE_DEPTH + 1 {
            xml.push_str(&format!("<nodegraph name=\"n{}\">", i));
        }
        for _ in 0..MAX_XML_TREE_DEPTH + 1 {
            xml.push_str("</nodegraph>");
        }
        xml.push_str("\n</materialx>");

        let result = read_from_xml_str(&xml);
        assert!(
            matches!(result, Err(XmlError::MaxTreeDepth)),
            "should error on deep tree: {:?}",
            result
        );
    }

    #[test]
    fn element_predicate_filters_write() {
        let doc = read_from_xml_str(SIMPLE_MTLX).unwrap();
        let opts = XmlWriteOptions {
            write_xinclude_enable: true,
            element_predicate: Some(Box::new(|elem: &ElementPtr| {
                // Skip elements named "NG_basic"
                elem.borrow().get_name() != "NG_basic"
            })),
        };
        let xml = write_to_xml_string_with_options(&doc, &opts).unwrap();
        assert!(
            !xml.contains("NG_basic"),
            "predicate should filter out NG_basic: {}",
            xml
        );
    }

    #[test]
    fn xinclude_write_from_source_uri() {
        // Create a doc with a child that has a foreign sourceUri
        let doc = create_document();
        let root = doc.get_root();
        root.borrow_mut()
            .set_source_uri(Some("main.mtlx".to_string()));
        if let Ok(child) = add_child_of_category(&root, "nodegraph", "NG_lib") {
            child
                .borrow_mut()
                .set_source_uri(Some("stdlib.mtlx".to_string()));
        }

        let xml = write_to_xml_string(&doc).unwrap();
        assert!(
            xml.contains("xi:include"),
            "should generate xi:include for foreign sourceUri: {}",
            xml
        );
        assert!(
            xml.contains("href=\"stdlib.mtlx\""),
            "xi:include should reference stdlib.mtlx: {}",
            xml
        );
        assert!(
            !xml.contains("NG_lib"),
            "child with foreign sourceUri should not be written inline: {}",
            xml
        );
    }
}
