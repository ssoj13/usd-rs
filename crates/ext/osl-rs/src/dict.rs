//! Dictionary lookups for OSL `dict_find` / `dict_value` / `dict_next`.
//!
//! Port of `dictionary.cpp`. Supports simple XML and JSON dictionary access
//! for OSL shader metadata and structured attribute lookups.

use std::collections::HashMap;

/// A dictionary node (value or sub-dictionary).
#[derive(Debug, Clone)]
pub enum DictNode {
    /// A leaf string value.
    Value(String),
    /// A nested dictionary (key-value pairs).
    Dict(HashMap<String, DictNode>),
    /// An ordered list of nodes.
    Array(Vec<DictNode>),
}

impl DictNode {
    /// Get a string value from this node.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            DictNode::Value(s) => Some(s),
            _ => None,
        }
    }

    /// Get as integer.
    pub fn as_int(&self) -> Option<i32> {
        self.as_str()?.parse().ok()
    }

    /// Get as float.
    pub fn as_float(&self) -> Option<f32> {
        self.as_str()?.parse().ok()
    }

    /// Get a sub-node by key (for Dict nodes).
    pub fn get(&self, key: &str) -> Option<&DictNode> {
        match self {
            DictNode::Dict(map) => map.get(key),
            _ => None,
        }
    }

    /// Get an array element by index.
    pub fn get_index(&self, idx: usize) -> Option<&DictNode> {
        match self {
            DictNode::Array(arr) => arr.get(idx),
            _ => None,
        }
    }
}

/// A dictionary handle — an opaque integer returned by dict_find.
pub type DictHandle = i32;

/// Invalid handle.
pub const DICT_INVALID: DictHandle = -1;

// ---------------------------------------------------------------------------
// Internal node table (matches C++ Dictionary pattern)
// ---------------------------------------------------------------------------

/// An entry in the node table — a resolved query result.
#[derive(Debug, Clone)]
struct NodeEntry {
    /// Reference to the actual node data.
    node: DictNode,
    /// Next sibling node for the same query (0 = no more).
    next: i32,
}

/// Dictionary store — manages loaded dictionaries for a shading context.
/// Mirrors the C++ `Dictionary` class: node-ID based handles with sibling
/// iteration via `dict_next`.
#[derive(Debug, Default)]
pub struct DictStore {
    /// Parsed document roots, keyed by dictionary source string.
    documents: HashMap<String, DictNode>,
    /// Node table: index 0 is reserved (invalid sentinel).
    nodes: Vec<NodeEntry>,
}

impl DictStore {
    pub fn new() -> Self {
        Self {
            documents: HashMap::new(),
            // Index 0 = sentinel "not found"
            nodes: vec![NodeEntry {
                node: DictNode::Value(String::new()),
                next: 0,
            }],
        }
    }

    // --- Document loading (lazy, cached by source string) -----------------

    fn get_or_load_document(&mut self, source: &str) -> Option<DictNode> {
        if let Some(node) = self.documents.get(source) {
            return Some(node.clone());
        }
        // Try JSON first, then XML
        let node = if source.trim_start().starts_with('{') || source.trim_start().starts_with('[') {
            parse_json(source).ok()
        } else {
            parse_simple_xml(source).ok()
        };
        if let Some(ref n) = node {
            self.documents.insert(source.to_string(), n.clone());
        }
        node
    }

    // --- Public API matching C++ dict_find / dict_next / dict_value -------

    /// `dict_find(string dictionary, string query)` — load document and query.
    /// Returns a node ID (>0) or DICT_INVALID on failure.
    pub fn dict_find_str(&mut self, dictionary: &str, query: &str) -> i32 {
        let root = match self.get_or_load_document(dictionary) {
            Some(r) => r,
            None => return DICT_INVALID,
        };
        self.query_node(&root, query)
    }

    /// `dict_find(int nodeID, string query)` — query from a previously found node.
    pub fn dict_find_node(&mut self, node_id: i32, query: &str) -> i32 {
        if node_id <= 0 || node_id as usize >= self.nodes.len() {
            return DICT_INVALID;
        }
        let base = self.nodes[node_id as usize].node.clone();
        self.query_node(&base, query)
    }

    /// `dict_next(int nodeID)` — return the next sibling, or 0.
    pub fn dict_next(&self, node_id: i32) -> i32 {
        if node_id <= 0 || node_id as usize >= self.nodes.len() {
            return DICT_INVALID;
        }
        self.nodes[node_id as usize].next
    }

    /// `dict_value(int nodeID, string attribname)` — get a string value.
    /// Returns `Some(value)` or `None`.
    pub fn dict_value_str(&self, node_id: i32, attribname: &str) -> Option<String> {
        if node_id <= 0 || node_id as usize >= self.nodes.len() {
            return None;
        }
        let node = &self.nodes[node_id as usize].node;
        if attribname.is_empty() {
            // Return node's own value
            return node.as_str().map(|s| s.to_string());
        }
        // Look up attribute by name
        node.get(attribname)?.as_str().map(|s| s.to_string())
    }

    /// `dict_value` returning int.
    pub fn dict_value_int(&self, node_id: i32, attribname: &str) -> Option<i32> {
        self.dict_value_str(node_id, attribname)?.parse().ok()
    }

    /// `dict_value` returning float.
    pub fn dict_value_float(&self, node_id: i32, attribname: &str) -> Option<f32> {
        self.dict_value_str(node_id, attribname)?.parse().ok()
    }

    // --- Internal query engine --------------------------------------------

    /// Resolve a dot-path query against a node, returning all matches as
    /// a linked list in the node table. Returns the first node ID or 0.
    fn query_node(&mut self, root: &DictNode, query: &str) -> i32 {
        let mut matches = Vec::new();
        self.collect_matches(root, query, &mut matches);

        if matches.is_empty() {
            return DICT_INVALID;
        }

        let first_id = self.nodes.len() as i32;
        let count = matches.len();
        for (i, m) in matches.into_iter().enumerate() {
            let next = if i + 1 < count {
                first_id + (i as i32) + 1
            } else {
                0
            };
            self.nodes.push(NodeEntry { node: m, next });
        }
        first_id
    }

    /// Collect all nodes matching a dot-separated path query.
    fn collect_matches(&self, root: &DictNode, query: &str, out: &mut Vec<DictNode>) {
        let parts: Vec<&str> = query.split('.').filter(|s| !s.is_empty()).collect();
        if parts.is_empty() {
            out.push(root.clone());
            return;
        }
        self.collect_recursive(root, &parts, 0, out);
    }

    fn collect_recursive(
        &self,
        node: &DictNode,
        parts: &[&str],
        depth: usize,
        out: &mut Vec<DictNode>,
    ) {
        if depth >= parts.len() {
            out.push(node.clone());
            return;
        }
        let key = parts[depth];

        // Wildcard: match all children
        if key == "*" {
            match node {
                DictNode::Dict(map) => {
                    for child in map.values() {
                        self.collect_recursive(child, parts, depth + 1, out);
                    }
                }
                DictNode::Array(arr) => {
                    for child in arr {
                        self.collect_recursive(child, parts, depth + 1, out);
                    }
                }
                _ => {}
            }
            return;
        }

        // Try numeric index for arrays
        if let Ok(idx) = key.parse::<usize>() {
            if let Some(child) = node.get_index(idx) {
                self.collect_recursive(child, parts, depth + 1, out);
                return;
            }
        }

        // Named key
        if let Some(child) = node.get(key) {
            self.collect_recursive(child, parts, depth + 1, out);
        }
    }

    // --- Legacy convenience methods (backwards compat) --------------------
    // NOTE: Two API paths reach the same underlying data:
    //   1. C++-compatible: dict_find_str / dict_find_node / dict_next / dict_value_*
    //   2. Legacy: load_json / load_xml / find / value_*
    // Both are actively used (interp.rs uses path 1, jit.rs uses path 2).
    // Consolidating would break callers; keep both until one is deprecated.

    /// Load a dictionary from a JSON string. Returns a handle (node ID of root).
    pub fn load_json(&mut self, json: &str) -> DictHandle {
        match parse_json(json) {
            Ok(node) => {
                let id = self.nodes.len() as DictHandle;
                self.nodes.push(NodeEntry { node, next: 0 });
                id
            }
            Err(_) => DICT_INVALID,
        }
    }

    /// Load a dictionary from a simple XML string. Returns a handle.
    pub fn load_xml(&mut self, xml: &str) -> DictHandle {
        match parse_simple_xml(xml) {
            Ok(node) => {
                let id = self.nodes.len() as DictHandle;
                self.nodes.push(NodeEntry { node, next: 0 });
                id
            }
            Err(_) => DICT_INVALID,
        }
    }

    /// Find a value by path (e.g., "key.subkey.field").
    pub fn find(&self, handle: DictHandle, path: &str) -> Option<&DictNode> {
        if handle <= 0 || handle as usize >= self.nodes.len() {
            return None;
        }
        let mut current = &self.nodes[handle as usize].node;
        for key in path.split('.') {
            if key.is_empty() {
                continue;
            }
            current = current.get(key)?;
        }
        Some(current)
    }

    /// Get a string value at path.
    pub fn value_str(&self, handle: DictHandle, path: &str) -> Option<&str> {
        self.find(handle, path)?.as_str()
    }

    /// Get an int value at path.
    pub fn value_int(&self, handle: DictHandle, path: &str) -> Option<i32> {
        self.find(handle, path)?.as_int()
    }

    /// Get a float value at path.
    pub fn value_float(&self, handle: DictHandle, path: &str) -> Option<f32> {
        self.find(handle, path)?.as_float()
    }
}

// ---------------------------------------------------------------------------
// Simple JSON parser
// ---------------------------------------------------------------------------

/// Parse a simplified JSON string.
fn parse_json(s: &str) -> Result<DictNode, String> {
    let s = s.trim();
    if s.is_empty() {
        return Err("empty input".into());
    }

    let mut pos = 0;
    parse_json_value(s.as_bytes(), &mut pos)
}

fn parse_json_value(s: &[u8], pos: &mut usize) -> Result<DictNode, String> {
    skip_ws(s, pos);
    if *pos >= s.len() {
        return Err("unexpected end".into());
    }

    match s[*pos] {
        b'{' => parse_json_object(s, pos),
        b'[' => parse_json_array(s, pos),
        b'"' => {
            let sv = parse_json_string(s, pos)?;
            Ok(DictNode::Value(sv))
        }
        _ => {
            // Number, bool, null → treat as string value
            let start = *pos;
            while *pos < s.len()
                && s[*pos] != b','
                && s[*pos] != b'}'
                && s[*pos] != b']'
                && !s[*pos].is_ascii_whitespace()
            {
                *pos += 1;
            }
            let val = std::str::from_utf8(&s[start..*pos])
                .unwrap_or("")
                .to_string();
            Ok(DictNode::Value(val))
        }
    }
}

fn parse_json_object(s: &[u8], pos: &mut usize) -> Result<DictNode, String> {
    *pos += 1; // skip '{'
    let mut map = HashMap::new();

    skip_ws(s, pos);
    if *pos < s.len() && s[*pos] == b'}' {
        *pos += 1;
        return Ok(DictNode::Dict(map));
    }

    loop {
        skip_ws(s, pos);
        let key = parse_json_string(s, pos)?;
        skip_ws(s, pos);
        if *pos < s.len() && s[*pos] == b':' {
            *pos += 1;
        }
        let val = parse_json_value(s, pos)?;
        map.insert(key, val);

        skip_ws(s, pos);
        if *pos >= s.len() {
            break;
        }
        if s[*pos] == b',' {
            *pos += 1;
            continue;
        }
        if s[*pos] == b'}' {
            *pos += 1;
            break;
        }
        return Err(format!(
            "unexpected char '{}' in JSON object",
            s[*pos] as char
        ));
    }

    Ok(DictNode::Dict(map))
}

fn parse_json_array(s: &[u8], pos: &mut usize) -> Result<DictNode, String> {
    *pos += 1; // skip '['
    let mut arr = Vec::new();

    skip_ws(s, pos);
    if *pos < s.len() && s[*pos] == b']' {
        *pos += 1;
        return Ok(DictNode::Array(arr));
    }

    loop {
        let val = parse_json_value(s, pos)?;
        arr.push(val);

        skip_ws(s, pos);
        if *pos >= s.len() {
            break;
        }
        if s[*pos] == b',' {
            *pos += 1;
            continue;
        }
        if s[*pos] == b']' {
            *pos += 1;
            break;
        }
        return Err("unexpected char in JSON array".into());
    }

    Ok(DictNode::Array(arr))
}

fn parse_json_string(s: &[u8], pos: &mut usize) -> Result<String, String> {
    if *pos >= s.len() || s[*pos] != b'"' {
        return Err("expected '\"'".into());
    }
    *pos += 1;
    let mut result = String::new();
    while *pos < s.len() && s[*pos] != b'"' {
        if s[*pos] == b'\\' {
            *pos += 1;
            if *pos < s.len() {
                match s[*pos] {
                    b'n' => result.push('\n'),
                    b't' => result.push('\t'),
                    b'r' => result.push('\r'),
                    b'"' => result.push('"'),
                    b'\\' => result.push('\\'),
                    b'/' => result.push('/'),
                    b'b' => result.push('\x08'),
                    b'f' => result.push('\x0C'),
                    // \uNNNN unicode escape (with surrogate pair support)
                    b'u' => {
                        let cp = parse_hex4(s, pos);
                        if (0xD800..=0xDBFF).contains(&cp) {
                            // High surrogate — expect \uDC00..DFFF
                            if *pos + 2 < s.len() && s[*pos + 1] == b'\\' && s[*pos + 2] == b'u' {
                                *pos += 2; // skip \u
                                let lo = parse_hex4(s, pos);
                                if (0xDC00..=0xDFFF).contains(&lo) {
                                    let full = ((cp - 0xD800) << 10) + (lo - 0xDC00) + 0x10000;
                                    if let Some(ch) = char::from_u32(full) {
                                        result.push(ch);
                                    }
                                }
                            }
                        } else if let Some(ch) = char::from_u32(cp) {
                            result.push(ch);
                        }
                    }
                    c => {
                        result.push('\\');
                        result.push(c as char);
                    }
                }
            }
        } else {
            result.push(s[*pos] as char);
        }
        *pos += 1;
    }
    if *pos < s.len() {
        *pos += 1;
    } // skip closing "
    Ok(result)
}

/// Parse 4 hex digits from current pos+1..pos+4, advance pos by 4.
fn parse_hex4(s: &[u8], pos: &mut usize) -> u32 {
    let mut hex = String::with_capacity(4);
    for _ in 0..4 {
        *pos += 1;
        if *pos < s.len() {
            hex.push(s[*pos] as char);
        }
    }
    u32::from_str_radix(&hex, 16).unwrap_or(0xFFFD)
}

fn skip_ws(s: &[u8], pos: &mut usize) {
    while *pos < s.len() && s[*pos].is_ascii_whitespace() {
        *pos += 1;
    }
}

/// Decode standard XML entities: &amp; &lt; &gt; &apos; &quot;
fn decode_xml_entities(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&apos;", "'")
        .replace("&quot;", "\"")
}

// ---------------------------------------------------------------------------
// Simple XML parser (attribute-value only, for OSL dict)
// ---------------------------------------------------------------------------

/// Parse a simple XML string into a DictNode.
/// Supports: `<tag attr="val">text</tag>` and nested tags.
fn parse_simple_xml(xml: &str) -> Result<DictNode, String> {
    let xml = xml.trim();
    if xml.is_empty() {
        return Err("empty XML".into());
    }

    // Very simplified: parse top-level tag and its children
    let mut map = HashMap::new();
    let mut pos = 0;
    let bytes = xml.as_bytes();

    while pos < bytes.len() {
        skip_ws(bytes, &mut pos);
        if pos >= bytes.len() {
            break;
        }

        if bytes[pos] == b'<' {
            if pos + 1 < bytes.len() && bytes[pos + 1] == b'/' {
                break; // closing tag
            }
            pos += 1;

            // Read tag name
            let name_start = pos;
            while pos < bytes.len()
                && bytes[pos] != b'>'
                && bytes[pos] != b' '
                && bytes[pos] != b'/'
            {
                pos += 1;
            }
            let tag_name = std::str::from_utf8(&bytes[name_start..pos])
                .unwrap_or("")
                .to_string();

            // Parse attributes: key="value" pairs
            let mut attrs = HashMap::new();
            while pos < bytes.len() && bytes[pos] != b'>' && bytes[pos] != b'/' {
                // Skip whitespace
                while pos < bytes.len() && bytes[pos].is_ascii_whitespace() {
                    pos += 1;
                }
                if pos >= bytes.len() || bytes[pos] == b'>' || bytes[pos] == b'/' {
                    break;
                }
                // Read attribute name
                let attr_start = pos;
                while pos < bytes.len()
                    && bytes[pos] != b'='
                    && bytes[pos] != b'>'
                    && bytes[pos] != b'/'
                    && !bytes[pos].is_ascii_whitespace()
                {
                    pos += 1;
                }
                let attr_name = std::str::from_utf8(&bytes[attr_start..pos])
                    .unwrap_or("")
                    .to_string();
                if attr_name.is_empty() {
                    pos += 1;
                    continue;
                }
                // Skip whitespace and '='
                while pos < bytes.len() && bytes[pos].is_ascii_whitespace() {
                    pos += 1;
                }
                if pos < bytes.len() && bytes[pos] == b'=' {
                    pos += 1;
                    while pos < bytes.len() && bytes[pos].is_ascii_whitespace() {
                        pos += 1;
                    }
                    // Read attribute value (quoted)
                    if pos < bytes.len() && (bytes[pos] == b'"' || bytes[pos] == b'\'') {
                        let quote = bytes[pos];
                        pos += 1;
                        let val_start = pos;
                        while pos < bytes.len() && bytes[pos] != quote {
                            pos += 1;
                        }
                        let attr_val = decode_xml_entities(
                            std::str::from_utf8(&bytes[val_start..pos]).unwrap_or(""),
                        );
                        if pos < bytes.len() {
                            pos += 1;
                        } // skip closing quote
                        attrs.insert(attr_name, attr_val);
                    }
                }
            }

            if pos < bytes.len() && bytes[pos] == b'/' {
                // Self-closing tag with attributes
                pos += 1; // skip /
                if pos < bytes.len() && bytes[pos] == b'>' {
                    pos += 1;
                }
                if attrs.is_empty() {
                    map.insert(tag_name, DictNode::Value(String::new()));
                } else {
                    // Store attrs as @name children in a sub-map
                    let mut sub = HashMap::new();
                    for (k, v) in attrs {
                        sub.insert(format!("@{k}"), DictNode::Value(v));
                    }
                    map.insert(tag_name, DictNode::Dict(sub));
                }
            } else {
                if pos < bytes.len() {
                    pos += 1;
                } // skip >

                // Read content until closing tag
                let _content_start = pos;
                let closing = format!("</{tag_name}>");
                if let Some(end_pos) = xml[pos..].find(&closing) {
                    let content = decode_xml_entities(xml[pos..pos + end_pos].trim());
                    pos += end_pos + closing.len();

                    if content.starts_with('<') {
                        // Nested XML - merge attributes into the child dict
                        match parse_simple_xml(&content) {
                            Ok(DictNode::Dict(mut child_map)) => {
                                for (k, v) in &attrs {
                                    child_map.insert(format!("@{k}"), DictNode::Value(v.clone()));
                                }
                                map.insert(tag_name, DictNode::Dict(child_map));
                            }
                            Ok(node) => {
                                if attrs.is_empty() {
                                    map.insert(tag_name, node);
                                } else {
                                    let mut sub = HashMap::new();
                                    for (k, v) in &attrs {
                                        sub.insert(format!("@{k}"), DictNode::Value(v.clone()));
                                    }
                                    map.insert(tag_name, DictNode::Dict(sub));
                                }
                            }
                            Err(_) => {
                                map.insert(tag_name, DictNode::Value(content));
                            }
                        }
                    } else if !attrs.is_empty() {
                        // Tag with text content AND attributes
                        let mut sub = HashMap::new();
                        sub.insert("#text".to_string(), DictNode::Value(content));
                        for (k, v) in &attrs {
                            sub.insert(format!("@{k}"), DictNode::Value(v.clone()));
                        }
                        map.insert(tag_name, DictNode::Dict(sub));
                    } else {
                        map.insert(tag_name, DictNode::Value(content));
                    }
                } else {
                    // No closing tag found
                    break;
                }
            }
        } else {
            pos += 1;
        }
    }

    Ok(DictNode::Dict(map))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_object() {
        let mut store = DictStore::new();
        let h = store.load_json(r#"{"name": "test", "value": "42"}"#);
        assert_ne!(h, DICT_INVALID);
        assert_eq!(store.value_str(h, "name"), Some("test"));
        assert_eq!(store.value_int(h, "value"), Some(42));
    }

    #[test]
    fn test_json_nested() {
        let mut store = DictStore::new();
        let h = store.load_json(r#"{"outer": {"inner": "hello"}}"#);
        assert_ne!(h, DICT_INVALID);
        assert_eq!(store.value_str(h, "outer.inner"), Some("hello"));
    }

    #[test]
    fn test_json_array() {
        let mut store = DictStore::new();
        let h = store.load_json(r#"{"arr": [1, 2, 3]}"#);
        assert_ne!(h, DICT_INVALID);
        let node = store.find(h, "arr").unwrap();
        assert!(matches!(node, DictNode::Array(arr) if arr.len() == 3));
    }

    #[test]
    fn test_simple_xml() {
        let mut store = DictStore::new();
        let h = store.load_xml("<root><name>test</name><value>42</value></root>");
        assert_ne!(h, DICT_INVALID);
        assert_eq!(store.value_str(h, "root.name"), Some("test"));
        assert_eq!(store.value_int(h, "root.value"), Some(42));
    }

    #[test]
    fn test_invalid_handle() {
        let store = DictStore::new();
        assert_eq!(store.value_str(DICT_INVALID, "anything"), None);
        assert_eq!(store.value_str(999, "anything"), None);
    }

    #[test]
    fn test_xml_attributes() {
        let mut store = DictStore::new();
        let h = store.load_xml(r#"<root><item key="val" foo="bar">text</item></root>"#);
        assert_ne!(h, DICT_INVALID);
        assert_eq!(store.value_str(h, "root.item.@key"), Some("val"));
        assert_eq!(store.value_str(h, "root.item.@foo"), Some("bar"));
        assert_eq!(store.value_str(h, "root.item.#text"), Some("text"));
    }

    #[test]
    fn test_xml_entities() {
        let mut store = DictStore::new();
        let h = store.load_xml("<root><v>a &amp; b &lt; c</v></root>");
        assert_ne!(h, DICT_INVALID);
        assert_eq!(store.value_str(h, "root.v"), Some("a & b < c"));
    }

    #[test]
    fn test_xml_self_closing_attrs() {
        let mut store = DictStore::new();
        let h = store.load_xml(r#"<root><item key="val"/></root>"#);
        assert_ne!(h, DICT_INVALID);
        assert_eq!(store.value_str(h, "root.item.@key"), Some("val"));
    }

    #[test]
    fn test_xml_attr_entities() {
        let mut store = DictStore::new();
        let h = store.load_xml(r#"<root><t v="a&amp;b">x</t></root>"#);
        assert_ne!(h, DICT_INVALID);
        assert_eq!(store.value_str(h, "root.t.@v"), Some("a&b"));
    }

    #[test]
    fn test_json_unicode_bmp() {
        let mut store = DictStore::new();
        // \u00e9 = e with acute
        let h = store.load_json(r#"{"name": "caf\u00e9"}"#);
        assert_ne!(h, DICT_INVALID);
        assert_eq!(store.value_str(h, "name"), Some("caf\u{00e9}"));
    }

    #[test]
    fn test_json_unicode_surrogate_pair() {
        let mut store = DictStore::new();
        // \uD83D\uDE00 = U+1F600 (grinning face emoji)
        let h = store.load_json(r#"{"emoji": "\uD83D\uDE00"}"#);
        assert_ne!(h, DICT_INVALID);
        assert_eq!(store.value_str(h, "emoji"), Some("\u{1F600}"));
    }
}
