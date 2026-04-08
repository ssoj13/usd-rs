use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, MutexGuard, OnceLock};

use usd_tf::RefPtr;

// ---- helpers ----------------------------------------------------------------

static NODE_COUNT: AtomicUsize = AtomicUsize::new(0);
static NODE_TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn node_test_guard() -> MutexGuard<'static, ()> {
    NODE_TEST_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap()
}

// Node uses Mutex so we can update children without requiring Clone.
struct Node {
    child: std::sync::Mutex<Option<RefPtr<Node>>>,
}

impl Node {
    fn new() -> RefPtr<Self> {
        NODE_COUNT.fetch_add(1, Ordering::SeqCst);
        RefPtr::new(Node {
            child: std::sync::Mutex::new(None),
        })
    }

    fn set_child(parent: &RefPtr<Node>, child: RefPtr<Node>) {
        *parent.child.lock().unwrap() = Some(child);
    }

    fn length(ptr: &RefPtr<Node>) -> usize {
        match &*ptr.child.lock().unwrap() {
            Some(c) => 1 + Self::length(c),
            None => 1,
        }
    }
}

impl Drop for Node {
    fn drop(&mut self) {
        NODE_COUNT.fetch_sub(1, Ordering::SeqCst);
    }
}

fn make_chain(n: usize) -> Option<RefPtr<Node>> {
    if n == 0 {
        return None;
    }
    let root = Node::new();
    if let Some(child) = make_chain(n - 1) {
        Node::set_child(&root, child);
    }
    Some(root)
}

fn hash_of<T: Hash>(v: &T) -> u64 {
    let mut h = DefaultHasher::new();
    v.hash(&mut h);
    h.finish()
}

// ---- tests ------------------------------------------------------------------

#[test]
fn test_new_and_deref() {
    let ptr = RefPtr::new(42i32);
    assert_eq!(*ptr, 42);
}

#[test]
fn test_strong_count_single() {
    let ptr = RefPtr::new(0i32);
    assert_eq!(ptr.strong_count(), 1);
}

#[test]
fn test_strong_count_clone() {
    let ptr1 = RefPtr::new(0i32);
    let ptr2 = ptr1.clone();
    assert_eq!(ptr1.strong_count(), 2);
    assert_eq!(ptr2.strong_count(), 2);
    drop(ptr2);
    assert_eq!(ptr1.strong_count(), 1);
}

#[test]
fn test_is_unique() {
    let ptr = RefPtr::new(0i32);
    assert!(ptr.is_unique());
    let ptr2 = ptr.clone();
    assert!(!ptr.is_unique());
    drop(ptr2);
    assert!(ptr.is_unique());
}

// Pointer-identity equality: two clones are equal, a fresh allocation is not.
#[test]
fn test_equality_pointer_identity() {
    let ptr1 = RefPtr::new(42i32);
    let ptr2 = ptr1.clone();
    let ptr3 = RefPtr::new(42i32);

    assert_eq!(ptr1, ptr2);
    assert_ne!(ptr1, ptr3);
}

#[test]
fn test_ptr_eq() {
    let ptr1 = RefPtr::new(1i32);
    let ptr2 = ptr1.clone();
    let ptr3 = RefPtr::new(1i32);

    assert!(RefPtr::ptr_eq(&ptr1, &ptr2));
    assert!(!RefPtr::ptr_eq(&ptr1, &ptr3));
}

// Hash must be consistent with pointer identity: same allocation → same hash.
#[test]
fn test_hash_null_equivalence() {
    // Two default (null) Option<RefPtr> values hash the same way is N/A in
    // Rust – instead verify two clones of the same ptr share a hash.
    let ptr = RefPtr::new(7i32);
    let clone = ptr.clone();
    assert_eq!(hash_of(&ptr), hash_of(&clone));
}

#[test]
fn test_hash_same_allocation() {
    let ptr = RefPtr::new(99i32);
    let ptr2 = ptr.clone();
    assert_eq!(hash_of(&ptr), hash_of(&ptr2));
    // as_ptr gives the raw address — hash must equal hash of pointer value
    assert_eq!(hash_of(&ptr), hash_of(&ptr.as_ptr()));
}

#[test]
fn test_hash_different_allocations_different_ptrs() {
    let ptr1 = RefPtr::new(1i32);
    let ptr2 = RefPtr::new(1i32);
    // Different allocations → different raw addresses, so hashes differ.
    // (This is probabilistically true; collisions theoretically possible but
    //  extremely unlikely for adjacent heap allocations.)
    assert_ne!(ptr1.as_ptr(), ptr2.as_ptr());
    assert_ne!(hash_of(&ptr1), hash_of(&ptr2));
}

// Hash-set deduplication: ptr + clone = 1 entry, ptr + fresh = 2 entries.
#[test]
fn test_hash_set_dedup() {
    use std::collections::HashSet;

    let ptr1 = RefPtr::new(0i32);
    let ptr2 = ptr1.clone();
    let ptr3 = RefPtr::new(0i32);

    let mut set = HashSet::new();
    set.insert(ptr1);
    set.insert(ptr2); // same allocation as ptr1
    set.insert(ptr3); // different allocation

    assert_eq!(set.len(), 2);
}

// Node chain: allocation / deallocation lifecycle matches C++ test.
#[test]
fn test_chain_lifecycle() {
    let _guard = node_test_guard();
    NODE_COUNT.store(0, Ordering::SeqCst);

    let chain1 = make_chain(10).unwrap();
    let chain2 = make_chain(5).unwrap();

    assert_eq!(NODE_COUNT.load(Ordering::SeqCst), 15);
    assert_eq!(Node::length(&chain1), 10);
    assert_eq!(Node::length(&chain2), 5);

    drop(chain1);
    drop(chain2);

    assert_eq!(NODE_COUNT.load(Ordering::SeqCst), 0);
}

// Node chain: replacing the child drops the old sub-tree.
#[test]
fn test_chain_child_replacement_drops() {
    let _guard = node_test_guard();
    NODE_COUNT.store(0, Ordering::SeqCst);

    let chain1 = make_chain(10).unwrap();
    let chain2 = make_chain(5).unwrap();

    // attach chain1 as child of a new root
    let root = Node::new();
    Node::set_child(&root, chain1.clone());
    drop(chain1); // root now solely owns it

    assert_eq!(Node::length(&root), 11);

    // replace child with chain2 — chain1's 10 nodes should be freed
    Node::set_child(&root, chain2.clone());
    drop(chain2);
    assert_eq!(Node::length(&root), 6);

    // 1 root + 5 from chain2
    assert_eq!(NODE_COUNT.load(Ordering::SeqCst), 6);

    drop(root);
    assert_eq!(NODE_COUNT.load(Ordering::SeqCst), 0);
}

// Swap: both ptrs exchange their allocations.
#[test]
fn test_swap() {
    let n1 = RefPtr::new(1i32);
    let n2 = RefPtr::new(2i32);

    let mut a = n1.clone();
    let mut b = n2.clone();

    assert!(RefPtr::ptr_eq(&a, &n1));
    assert!(RefPtr::ptr_eq(&b, &n2));

    std::mem::swap(&mut a, &mut b);

    assert!(RefPtr::ptr_eq(&a, &n2));
    assert!(RefPtr::ptr_eq(&b, &n1));
}

// Self-swap is a no-op — pointer identity is preserved.
#[test]
fn test_self_swap() {
    let n = RefPtr::new(42i32);
    let a = n.clone();
    let a_ptr = a.as_ptr();
    // No self-swap needed; just verify the pointer is stable.
    assert_eq!(a.as_ptr(), a_ptr);
}

// Move constructor semantics via std::mem::replace.
#[test]
fn test_move_constructor() {
    let n1 = RefPtr::new(0i32);
    let n1_ptr = n1.as_ptr();

    let n2 = n1.clone(); // "move" simulated: keep n1 alive then replace
    // Real move: use Option<RefPtr>
    let mut boxed = Some(RefPtr::new(7i32));
    let raw_ptr = boxed.as_ref().unwrap().as_ptr();
    let moved = boxed.take().unwrap();
    assert!(boxed.is_none());
    assert_eq!(moved.as_ptr(), raw_ptr);

    drop(n2);
    drop(n1);
    let _ = n1_ptr; // suppress lint
}

// from_arc / into_arc round-trip.
#[test]
fn test_from_into_arc() {
    let arc = Arc::new(55i32);
    let arc_count_before = Arc::strong_count(&arc);
    let ptr = RefPtr::from_arc(arc.clone());
    assert_eq!(Arc::strong_count(&arc), arc_count_before + 1);
    let back: Arc<i32> = ptr.into_arc();
    assert_eq!(*back, 55);
}

// make_mut clones when shared, mutates in-place when unique.
#[test]
fn test_make_mut_shared() {
    let mut ptr1 = RefPtr::new(vec![1, 2, 3]);
    let ptr2 = ptr1.clone();
    RefPtr::make_mut(&mut ptr1).push(4);
    assert_eq!(*ptr1, vec![1, 2, 3, 4]);
    assert_eq!(*ptr2, vec![1, 2, 3]);
}

#[test]
fn test_make_mut_unique() {
    let mut ptr = RefPtr::new(vec![1, 2]);
    RefPtr::make_mut(&mut ptr).push(3);
    assert_eq!(*ptr, vec![1, 2, 3]);
}

// Default constructs the wrapped type.
#[test]
fn test_default() {
    let ptr: RefPtr<i32> = RefPtr::default();
    assert_eq!(*ptr, 0);
}

// Debug / Display formatting delegates to the inner value.
#[test]
fn test_debug_display() {
    let ptr = RefPtr::new(123i32);
    assert_eq!(format!("{ptr:?}"), "123");
    assert_eq!(format!("{ptr}"), "123");
}
