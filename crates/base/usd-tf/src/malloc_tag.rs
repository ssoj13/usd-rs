//! Top-down memory tagging system.
//!
//! Provides infrastructure for tracking memory allocations by named tags.
//! This enables hierarchical memory profiling where allocations are
//! attributed to the call stack of tags that were active at allocation time.
//!
//! # Overview
//!
//! The malloc tag system works by maintaining a thread-local stack of tags.
//! When memory is allocated, it's attributed to the current tag path.
//! Tags can be nested to create a hierarchical view of memory usage.
//!
//! # Note
//!
//! This is a simplified Rust port. The full C++ implementation hooks into
//! the system allocator, which requires platform-specific code. This
//! implementation provides the API and tracking infrastructure but does
//! not automatically intercept allocations.
//!
//! # Examples
//!
//! ```
//! use usd_tf::malloc_tag::{MallocTag, AutoMallocTag};
//!
//! // Initialize the tagging system (optional in Rust version)
//! MallocTag::initialize();
//!
//! {
//!     let _tag = AutoMallocTag::new("MyFeature");
//!     // Allocations here are attributed to "MyFeature"
//!
//!     {
//!         let _inner = AutoMallocTag::new("SubComponent");
//!         // Allocations here are attributed to "MyFeature/SubComponent"
//!     }
//! }
//! ```

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock, RwLock};

/// Global initialization flag.
static IS_INITIALIZED: AtomicBool = AtomicBool::new(false);

/// Total bytes allocated (tracked manually).
static TOTAL_BYTES: AtomicUsize = AtomicUsize::new(0);

/// Maximum total bytes ever allocated.
static MAX_TOTAL_BYTES: AtomicUsize = AtomicUsize::new(0);

// Thread-local tag stack.
thread_local! {
    static TAG_STACK: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

/// Global call tree data.
static CALL_TREE_DATA: OnceLock<RwLock<CallTreeData>> = OnceLock::new();

fn get_call_tree_data() -> &'static RwLock<CallTreeData> {
    CALL_TREE_DATA.get_or_init(|| RwLock::new(CallTreeData::new()))
}

/// Internal call tree tracking data.
#[derive(Debug, Default)]
struct CallTreeData {
    /// Call sites by name -> bytes.
    call_sites: HashMap<String, usize>,
    /// Root of the path tree.
    root: PathNodeData,
}

impl CallTreeData {
    fn new() -> Self {
        Self {
            call_sites: HashMap::new(),
            root: PathNodeData::new("root"),
        }
    }
}

/// Internal path node data.
#[derive(Debug, Default)]
struct PathNodeData {
    name: String,
    bytes: AtomicUsize,
    bytes_direct: AtomicUsize,
    allocations: AtomicUsize,
    children: Mutex<HashMap<String, PathNodeData>>,
}

impl PathNodeData {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            bytes: AtomicUsize::new(0),
            bytes_direct: AtomicUsize::new(0),
            allocations: AtomicUsize::new(0),
            children: Mutex::new(HashMap::new()),
        }
    }
}

/// Node in the call tree structure.
///
/// Represents a path through the tag hierarchy with memory statistics.
#[derive(Debug, Clone, Default)]
pub struct PathNode {
    /// Allocated bytes by this or descendant nodes.
    pub bytes: usize,
    /// Allocated bytes (only for this node).
    pub bytes_direct: usize,
    /// Number of allocations for this node.
    pub allocations: usize,
    /// Tag name.
    pub site_name: String,
    /// Children nodes.
    pub children: Vec<PathNode>,
}

/// Record of bytes allocated under each different tag.
#[derive(Debug, Clone, Default)]
pub struct CallSite {
    /// Tag name.
    pub name: String,
    /// Allocated bytes.
    pub bytes: usize,
}

/// Which parts of the report to print.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PrintSetting {
    /// Print the full call tree.
    Tree,
    /// Print just the call sites.
    CallSites,
    /// Print both tree and call sites.
    #[default]
    Both,
}

/// Summary data structure for malloc statistics.
#[derive(Debug, Clone, Default)]
pub struct CallTree {
    /// All call sites.
    pub call_sites: Vec<CallSite>,
    /// Root node of the call-site hierarchy.
    pub root: PathNode,
    /// Captured malloc stacks (for leak detection).
    pub captured_call_stacks: Vec<CallStackInfo>,
}

impl CallTree {
    /// Get a formatted report string.
    ///
    /// # Arguments
    ///
    /// * `setting` - Which parts of the report to include
    /// * `max_printed_nodes` - Maximum number of nodes to print
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::malloc_tag::{CallTree, PrintSetting};
    ///
    /// let tree = CallTree::default();
    /// let report = tree.get_pretty_print_string(PrintSetting::Both, 1000);
    /// ```
    pub fn get_pretty_print_string(
        &self,
        setting: PrintSetting,
        max_printed_nodes: usize,
    ) -> String {
        let mut result = String::new();

        if setting == PrintSetting::Tree || setting == PrintSetting::Both {
            result.push_str("=== Memory Usage Tree ===\n");
            self.print_tree_node(&self.root, 0, &mut result, &mut 0, max_printed_nodes);
        }

        if setting == PrintSetting::CallSites || setting == PrintSetting::Both {
            result.push_str("\n=== Call Sites ===\n");
            let mut sorted_sites: Vec<_> = self.call_sites.iter().collect();
            sorted_sites.sort_by(|a, b| b.bytes.cmp(&a.bytes));

            for site in sorted_sites.iter().take(max_printed_nodes) {
                result.push_str(&format!(
                    "{}: {} bytes\n",
                    site.name,
                    format_bytes(site.bytes)
                ));
            }
        }

        result
    }

    fn print_tree_node(
        &self,
        node: &PathNode,
        depth: usize,
        out: &mut String,
        count: &mut usize,
        max: usize,
    ) {
        if *count >= max {
            return;
        }
        *count += 1;

        let indent = "  ".repeat(depth);
        out.push_str(&format!(
            "{}{}: {} bytes ({} direct, {} allocs)\n",
            indent,
            node.site_name,
            format_bytes(node.bytes),
            format_bytes(node.bytes_direct),
            node.allocations
        ));

        for child in &node.children {
            self.print_tree_node(child, depth + 1, out, count, max);
        }
    }

    /// Write a report to a writer.
    pub fn report<W: std::io::Write>(&self, out: &mut W, root_name: Option<&str>) {
        let name = root_name.unwrap_or(&self.root.site_name);
        writeln!(out, "Memory Report for: {}", name).ok();
        writeln!(
            out,
            "{}",
            self.get_pretty_print_string(PrintSetting::Both, 100000)
        )
        .ok();
    }
}

/// Stack frame information for allocation tracking.
#[derive(Debug, Clone, Default)]
pub struct CallStackInfo {
    /// Stack frame pointers.
    pub stack: Vec<usize>,
    /// Allocated memory size.
    pub size: usize,
    /// Number of allocations.
    pub num_allocations: usize,
}

/// Format bytes in human-readable form.
fn format_bytes(bytes: usize) -> String {
    if bytes >= 1024 * 1024 * 1024 {
        format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    } else if bytes >= 1024 * 1024 {
        format!("{:.2} MB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{:.2} KB", bytes as f64 / 1024.0)
    } else {
        format!("{} B", bytes)
    }
}

/// Top-down memory tagging system.
///
/// This struct provides static methods for memory tracking and reporting.
pub struct MallocTag;

impl MallocTag {
    /// Initialize the memory tagging system.
    ///
    /// Returns `true` if initialization succeeded or was already done.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::malloc_tag::MallocTag;
    ///
    /// let result = MallocTag::initialize();
    /// assert!(result.is_ok());
    /// assert!(MallocTag::is_initialized());
    /// ```
    pub fn initialize() -> Result<(), String> {
        if IS_INITIALIZED.load(Ordering::Acquire) {
            return Ok(());
        }

        // Initialize global data
        let _ = get_call_tree_data();

        IS_INITIALIZED.store(true, Ordering::Release);
        Ok(())
    }

    /// Check if the tagging system is initialized.
    pub fn is_initialized() -> bool {
        IS_INITIALIZED.load(Ordering::Acquire)
    }

    /// Get total allocated bytes being tracked.
    pub fn get_total_bytes() -> usize {
        TOTAL_BYTES.load(Ordering::Relaxed)
    }

    /// Get maximum total bytes ever allocated.
    pub fn get_max_total_bytes() -> usize {
        MAX_TOTAL_BYTES.load(Ordering::Relaxed)
    }

    /// Get a snapshot of memory usage.
    ///
    /// # Arguments
    ///
    /// * `skip_repeated` - If true, skip repeated call sites
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::malloc_tag::MallocTag;
    ///
    /// MallocTag::initialize().ok();
    /// let tree = MallocTag::get_call_tree(true);
    /// println!("Total tracked: {} bytes", tree.root.bytes);
    /// ```
    pub fn get_call_tree(_skip_repeated: bool) -> CallTree {
        let data = get_call_tree_data();
        let Ok(guard) = data.read() else {
            return CallTree::default();
        };

        // Convert call sites
        let call_sites: Vec<CallSite> = guard
            .call_sites
            .iter()
            .map(|(name, &bytes)| CallSite {
                name: name.clone(),
                bytes,
            })
            .collect();

        // Build path tree
        let root = Self::build_path_node(&guard.root);

        CallTree {
            call_sites,
            root,
            captured_call_stacks: Vec::new(),
        }
    }

    fn build_path_node(data: &PathNodeData) -> PathNode {
        let Ok(children_guard) = data.children.lock() else {
            return PathNode {
                site_name: data.name.clone(),
                ..Default::default()
            };
        };
        let children: Vec<PathNode> = children_guard.values().map(Self::build_path_node).collect();

        PathNode {
            bytes: data.bytes.load(Ordering::Relaxed),
            bytes_direct: data.bytes_direct.load(Ordering::Relaxed),
            allocations: data.allocations.load(Ordering::Relaxed),
            site_name: data.name.clone(),
            children,
        }
    }

    /// Push a tag onto the stack.
    ///
    /// Prefer using [`AutoMallocTag`] for automatic cleanup.
    pub fn push(name: &str) {
        if !IS_INITIALIZED.load(Ordering::Acquire) {
            return;
        }

        TAG_STACK.with(|stack| {
            stack.borrow_mut().push(name.to_string());
        });
    }

    /// Pop a tag from the stack.
    ///
    /// Prefer using [`AutoMallocTag`] for automatic cleanup.
    pub fn pop() {
        if !IS_INITIALIZED.load(Ordering::Acquire) {
            return;
        }

        TAG_STACK.with(|stack| {
            stack.borrow_mut().pop();
        });
    }

    /// Get the current tag path.
    pub fn get_current_path() -> Vec<String> {
        TAG_STACK.with(|stack| stack.borrow().clone())
    }

    /// Record an allocation at the current tag path.
    ///
    /// This is called manually since we don't hook into the allocator.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::malloc_tag::{MallocTag, AutoMallocTag};
    ///
    /// MallocTag::initialize().ok();
    /// {
    ///     let _tag = AutoMallocTag::new("MyAllocation");
    ///     // Record a 1024 byte allocation
    ///     MallocTag::record_allocation(1024);
    /// }
    /// ```
    pub fn record_allocation(bytes: usize) {
        if !IS_INITIALIZED.load(Ordering::Acquire) {
            return;
        }

        // Update total bytes
        let new_total = TOTAL_BYTES.fetch_add(bytes, Ordering::Relaxed) + bytes;
        let mut current_max = MAX_TOTAL_BYTES.load(Ordering::Relaxed);
        while new_total > current_max {
            match MAX_TOTAL_BYTES.compare_exchange_weak(
                current_max,
                new_total,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(m) => current_max = m,
            }
        }

        // Get current path
        let path = Self::get_current_path();

        if path.is_empty() {
            return;
        }

        // Update call tree
        let data = get_call_tree_data();
        if let Ok(mut guard) = data.write() {
            // Update call site - safe because we checked path.is_empty() above
            if let Some(site_name) = path.last() {
                *guard.call_sites.entry(site_name.clone()).or_insert(0) += bytes;
            }

            // Update path tree
            Self::update_path_tree(&guard.root, &path, bytes);
        }
    }

    /// Record a deallocation.
    pub fn record_deallocation(bytes: usize) {
        if !IS_INITIALIZED.load(Ordering::Acquire) {
            return;
        }

        // Update total bytes
        let _ = TOTAL_BYTES.fetch_sub(
            bytes.min(TOTAL_BYTES.load(Ordering::Relaxed)),
            Ordering::Relaxed,
        );
    }

    fn update_path_tree(root: &PathNodeData, path: &[String], bytes: usize) {
        // Update bytes for root
        root.bytes.fetch_add(bytes, Ordering::Relaxed);

        if path.is_empty() {
            return;
        }

        // Update first level child (simplified - only goes one level deep)
        let name = &path[0];
        let is_last = path.len() == 1;

        let Ok(mut children) = root.children.lock() else {
            return;
        };
        let child = children
            .entry(name.clone())
            .or_insert_with(|| PathNodeData::new(name));

        child.bytes.fetch_add(bytes, Ordering::Relaxed);
        if is_last {
            child.bytes_direct.fetch_add(bytes, Ordering::Relaxed);
            child.allocations.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Set tags to trap in debugger (not implemented in Rust version).
    pub fn set_debug_match_list(_match_list: &str) {
        // Not implemented - would require debugger integration
    }

    /// Set tags to capture stack traces (not implemented in Rust version).
    pub fn set_captured_malloc_stacks_match_list(_match_list: &str) {
        // Not implemented - would require stack capture integration
    }

    /// Get captured malloc stacks (not implemented in Rust version).
    pub fn get_captured_malloc_stacks() -> Vec<Vec<usize>> {
        Vec::new()
    }
}

/// Scoped memory tag.
///
/// Push a tag when created, pop when dropped.
///
/// # Examples
///
/// ```
/// use usd_tf::malloc_tag::{MallocTag, AutoMallocTag};
///
/// MallocTag::initialize().ok();
///
/// {
///     let _tag = AutoMallocTag::new("OuterTag");
///     // Allocations here are under "OuterTag"
///
///     {
///         let _inner = AutoMallocTag::new("InnerTag");
///         // Allocations here are under "OuterTag/InnerTag"
///     }
///     // Back to just "OuterTag"
/// }
/// ```
pub struct AutoMallocTag {
    /// Number of tags pushed (for variadic support).
    n_tags: usize,
}

impl AutoMallocTag {
    /// Create a new auto malloc tag.
    ///
    /// The tag is pushed onto the stack immediately.
    pub fn new(name: &str) -> Self {
        MallocTag::push(name);
        Self { n_tags: 1 }
    }

    /// Create with multiple tags.
    pub fn new_multi(names: &[&str]) -> Self {
        for name in names {
            MallocTag::push(name);
        }
        Self {
            n_tags: names.len(),
        }
    }

    /// Release the tag early.
    ///
    /// Normally tags are released when dropped, but this allows
    /// early release.
    pub fn release(&mut self) {
        for _ in 0..self.n_tags {
            MallocTag::pop();
        }
        self.n_tags = 0;
    }
}

impl Drop for AutoMallocTag {
    fn drop(&mut self) {
        for _ in 0..self.n_tags {
            MallocTag::pop();
        }
    }
}

/// Type alias for compatibility.
pub type TfAutoMallocTag = AutoMallocTag;

/// Type alias for compatibility.
pub type TfAutoMallocTag2 = AutoMallocTag;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initialize() {
        let result = MallocTag::initialize();
        assert!(result.is_ok());
        assert!(MallocTag::is_initialized());
    }

    #[test]
    fn test_push_pop() {
        MallocTag::initialize().ok();

        MallocTag::push("Test");
        assert_eq!(MallocTag::get_current_path(), vec!["Test"]);

        MallocTag::push("Inner");
        assert_eq!(MallocTag::get_current_path(), vec!["Test", "Inner"]);

        MallocTag::pop();
        assert_eq!(MallocTag::get_current_path(), vec!["Test"]);

        MallocTag::pop();
        assert!(MallocTag::get_current_path().is_empty());
    }

    #[test]
    fn test_auto_malloc_tag() {
        MallocTag::initialize().ok();

        {
            let _tag = AutoMallocTag::new("Outer");
            assert_eq!(MallocTag::get_current_path(), vec!["Outer"]);

            {
                let _inner = AutoMallocTag::new("Inner");
                assert_eq!(MallocTag::get_current_path(), vec!["Outer", "Inner"]);
            }

            assert_eq!(MallocTag::get_current_path(), vec!["Outer"]);
        }

        assert!(MallocTag::get_current_path().is_empty());
    }

    #[test]
    fn test_auto_malloc_tag_release() {
        MallocTag::initialize().ok();

        let mut tag = AutoMallocTag::new("Test");
        assert_eq!(MallocTag::get_current_path(), vec!["Test"]);

        tag.release();
        assert!(MallocTag::get_current_path().is_empty());

        // Dropping after release should be safe
        drop(tag);
        assert!(MallocTag::get_current_path().is_empty());
    }

    #[test]
    fn test_auto_malloc_tag_multi() {
        MallocTag::initialize().ok();

        {
            let _tag = AutoMallocTag::new_multi(&["A", "B", "C"]);
            assert_eq!(MallocTag::get_current_path(), vec!["A", "B", "C"]);
        }

        assert!(MallocTag::get_current_path().is_empty());
    }

    #[test]
    fn test_record_allocation() {
        MallocTag::initialize().ok();

        let initial = MallocTag::get_total_bytes();

        {
            let _tag = AutoMallocTag::new("TestAlloc");
            MallocTag::record_allocation(1024);
        }

        assert!(MallocTag::get_total_bytes() >= initial + 1024);
    }

    #[test]
    fn test_record_deallocation() {
        MallocTag::initialize().ok();

        {
            let _tag = AutoMallocTag::new("TestDealloc");
            MallocTag::record_allocation(2048);
            let after_alloc = MallocTag::get_total_bytes();

            MallocTag::record_deallocation(1024);
            assert!(MallocTag::get_total_bytes() < after_alloc);
        }
    }

    #[test]
    fn test_get_call_tree() {
        MallocTag::initialize().ok();

        {
            let _tag = AutoMallocTag::new("TreeTest");
            MallocTag::record_allocation(512);
        }

        let tree = MallocTag::get_call_tree(true);
        // Tree should have root node
        assert!(!tree.root.site_name.is_empty());
    }

    #[test]
    fn test_call_tree_report() {
        let tree = CallTree::default();
        let report = tree.get_pretty_print_string(PrintSetting::Both, 100);
        assert!(report.contains("Memory Usage Tree"));
        assert!(report.contains("Call Sites"));
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(500), "500 B");
        assert!(format_bytes(1024).contains("KB"));
        assert!(format_bytes(1024 * 1024).contains("MB"));
        assert!(format_bytes(1024 * 1024 * 1024).contains("GB"));
    }

    #[test]
    fn test_print_setting_default() {
        assert_eq!(PrintSetting::default(), PrintSetting::Both);
    }

    #[test]
    fn test_max_total_bytes() {
        MallocTag::initialize().ok();

        {
            let _tag = AutoMallocTag::new("MaxTest");
            MallocTag::record_allocation(4096);
            let max_after = MallocTag::get_max_total_bytes();
            assert!(max_after >= 4096);
        }
    }

    #[test]
    fn test_thread_local_stacks() {
        use std::thread;

        MallocTag::initialize().ok();

        let handle = thread::spawn(|| {
            MallocTag::push("ThreadTag");
            let path = MallocTag::get_current_path();
            MallocTag::pop();
            path
        });

        // Main thread should have empty stack
        assert!(MallocTag::get_current_path().is_empty());

        // Other thread should have had its own stack
        let other_path = handle.join().unwrap();
        assert_eq!(other_path, vec!["ThreadTag"]);
    }

    #[test]
    fn test_call_tree_write_report() {
        let mut tree = CallTree::default();
        tree.root.site_name = "root".to_string();
        tree.root.bytes = 1000;

        let mut buf = Vec::new();
        tree.report(&mut buf, Some("TestRoot"));
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("TestRoot"));
    }

    #[test]
    fn test_path_node_default() {
        let node = PathNode::default();
        assert_eq!(node.bytes, 0);
        assert_eq!(node.allocations, 0);
        assert!(node.children.is_empty());
    }

    #[test]
    fn test_call_site_default() {
        let site = CallSite::default();
        assert!(site.name.is_empty());
        assert_eq!(site.bytes, 0);
    }

    #[test]
    fn test_call_stack_info_default() {
        let info = CallStackInfo::default();
        assert!(info.stack.is_empty());
        assert_eq!(info.size, 0);
        assert_eq!(info.num_allocations, 0);
    }

    #[test]
    fn test_not_implemented_functions() {
        // These should not panic
        MallocTag::set_debug_match_list("test");
        MallocTag::set_captured_malloc_stacks_match_list("test");
        let stacks = MallocTag::get_captured_malloc_stacks();
        assert!(stacks.is_empty());
    }
}
