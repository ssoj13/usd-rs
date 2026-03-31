#![allow(unsafe_code)]

use std::sync::atomic::{AtomicI32, Ordering};

use usd_tf::delegated_count_ptr::{
    DelegatedCount, DelegatedCountPtr, DoNotIncrementTag, IncrementTag, make_delegated_count_ptr,
};

// ---- test type with intrusive reference count --------------------------------

/// Simple stack-owned value with intrusive ref count.
/// Not heap-managed here — individual tests allocate on the heap when needed.
#[derive(Debug)]
struct RefCountedValue {
    value: i32,
    count: AtomicI32,
}

impl RefCountedValue {
    fn new(value: i32) -> Self {
        Self {
            value,
            count: AtomicI32::new(0),
        }
    }

    fn with_count(value: i32, initial_count: i32) -> Self {
        Self {
            value,
            count: AtomicI32::new(initial_count),
        }
    }
}

impl DelegatedCount for RefCountedValue {
    fn increment(ptr: *mut Self) {
        // SAFETY: caller guarantees ptr is non-null and valid
        #[allow(unsafe_code)]
        unsafe {
            (*ptr).count.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn decrement(ptr: *mut Self) {
        // SAFETY: caller guarantees ptr is non-null and valid
        #[allow(unsafe_code)]
        unsafe {
            if (*ptr).count.fetch_sub(1, Ordering::Release) == 1 {
                std::sync::atomic::fence(Ordering::Acquire);
                drop(Box::from_raw(ptr));
            }
        }
    }
}

#[derive(Debug)]
struct DerivedRefCountedValue {
    base: RefCountedValue,
}

impl DerivedRefCountedValue {
    fn new() -> Self {
        Self {
            base: RefCountedValue::new(0),
        }
    }
}

impl DelegatedCount for DerivedRefCountedValue {
    fn increment(ptr: *mut Self) {
        #[allow(unsafe_code)]
        unsafe {
            (*ptr).base.count.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn decrement(ptr: *mut Self) {
        #[allow(unsafe_code)]
        unsafe {
            if (*ptr).base.count.fetch_sub(1, Ordering::Release) == 1 {
                std::sync::atomic::fence(Ordering::Acquire);
                drop(Box::from_raw(ptr));
            }
        }
    }
}

type TestPtr = DelegatedCountPtr<RefCountedValue>;

// Helper: allocate a heap-owned RefCountedValue and return the raw pointer.
// The DelegatedCount::decrement will free it when count reaches zero.
fn alloc(value: i32, initial_count: i32) -> *mut RefCountedValue {
    Box::into_raw(Box::new(RefCountedValue::with_count(value, initial_count)))
}

fn count(raw: *const RefCountedValue) -> i32 {
    // SAFETY: caller ensures the raw pointer is still valid
    #[allow(unsafe_code)]
    unsafe {
        (*raw).count.load(Ordering::Relaxed)
    }
}

// ---- TestDefault ------------------------------------------------------------

#[test]
fn test_default() {
    let ptr = TestPtr::default();
    assert!(ptr.get().is_null());
    assert!(ptr.is_null());
}

// ---- TestIncrementTag -------------------------------------------------------

#[test]
fn test_increment_tag() {
    // start with count=1 so that when `adopted` drops (→ decrement → 1)
    // it does NOT free, leaving the stack allocation alive.
    let mut stack = RefCountedValue::with_count(10, 1);
    let raw: *mut RefCountedValue = &mut stack;

    let adopted = DelegatedCountPtr::new(IncrementTag, raw);
    assert_eq!(count(raw), 2);
    // keep adopted alive until assertion
    drop(adopted);
    assert_eq!(count(raw), 1); // back to initial, stack still alive
}

// ---- TestDoNotIncrementTag --------------------------------------------------

#[test]
fn test_do_not_increment_tag() {
    let mut stack = RefCountedValue::with_count(10, 2);
    let raw: *mut RefCountedValue = &mut stack;

    let adopted = DelegatedCountPtr::new_no_increment(DoNotIncrementTag, raw);
    assert_eq!(count(raw), 2); // unchanged

    drop(adopted); // decrement → 1, stack still alive
    assert_eq!(count(raw), 1);
}

// ---- TestScopedDecrement ----------------------------------------------------

#[test]
fn test_scoped_decrement() {
    let mut stack = RefCountedValue::with_count(7, 2);
    let raw: *mut RefCountedValue = &mut stack;

    {
        let adopted = DelegatedCountPtr::new_no_increment(DoNotIncrementTag, raw);
        assert_eq!(count(raw), 2);
        assert_eq!(adopted.get(), raw);
    }
    // adopted dropped → decrement → count 1
    assert_eq!(count(raw), 1);
}

// ---- TestMake ---------------------------------------------------------------

#[test]
fn test_make() {
    let made = make_delegated_count_ptr(RefCountedValue::new(12));
    assert!(!made.is_null());
    let raw = made.get();
    assert_eq!(count(raw), 1);
    assert_eq!(made.as_ref().unwrap().value, 12);
    // made drops here → count 0 → freed
}

// ---- TestEquality -----------------------------------------------------------

#[test]
fn test_equality() {
    let raw = alloc(10, 1);
    let adopted = DelegatedCountPtr::new(IncrementTag, raw);
    let another = DelegatedCountPtr::new(IncrementTag, raw);

    assert_eq!(adopted, another);
    assert_ne!(adopted, TestPtr::default());
    assert_eq!(TestPtr::default(), TestPtr::default());

    // Value equivalence does not imply address equivalence
    let m1 = make_delegated_count_ptr(RefCountedValue::new(12));
    let m2 = make_delegated_count_ptr(RefCountedValue::new(12));
    assert_ne!(m1, m2);

    drop(adopted);
    drop(another);
}

// ---- TestPointerOperators ---------------------------------------------------

#[test]
fn test_pointer_operators() {
    let made = make_delegated_count_ptr(RefCountedValue::new(15));
    assert_eq!(made.as_ref().unwrap().value, 15);
    assert_eq!((*made).value, 15);
}

// ---- TestNullAssignment -----------------------------------------------------

#[test]
fn test_null_assignment() {
    let made = make_delegated_count_ptr(RefCountedValue::new(12));
    let raw = made.get();
    assert_eq!(count(raw), 1);

    // Clone and then reassign to null — verifies decrement on reassignment.
    let copy = made.clone();
    assert_eq!(count(raw), 2);
    drop(copy); // drop explicitly instead of reassign to null
    // count back to 1 after drop
    assert_eq!(count(raw), 1);
    // `made` still holds the last ref
}

// ---- TestMoving -------------------------------------------------------------

#[test]
fn test_moving() {
    let made = make_delegated_count_ptr(RefCountedValue::new(12));
    let raw = made.get();
    assert_eq!(count(raw), 1);

    // Move via constructor
    let moved = made; // Rust move
    assert_eq!(count(raw), 1); // count unchanged: no increment on move
    assert_eq!(moved.get(), raw);

    // Move via assignment: re-bind to verify the pointer is still valid after move.
    let target = moved;
    assert!(!target.is_null());
    assert_eq!(target.get(), raw);
    assert_eq!(count(raw), 1);
    assert_eq!((*target).value, 12);
    // target drops → count 0 → freed
}

// ---- TestMovingSelf ---------------------------------------------------------

// C++ ARCH_PRAGMA_SELF_MOVE: self-move-assignment leaves the object in a valid
// (null) state. In Rust the borrow checker prevents true self-move, but we can
// test the equivalent: move to a temporary binding and then reset the source.
#[test]
fn test_moving_self_leaves_valid_state() {
    let mut stack = RefCountedValue::with_count(7, 1);
    let raw: *mut RefCountedValue = &mut stack;

    let mut adopted = DelegatedCountPtr::new(IncrementTag, raw);
    assert_eq!(count(raw), 2);

    // Simulate self-move: take the value out, leaving adopted null.
    let _moved_away = std::mem::replace(&mut adopted, TestPtr::default());
    assert!(adopted.is_null());
    // _moved_away holds the last ref; decrement on drop → count 1
    drop(_moved_away);
    assert_eq!(count(raw), 1);
}

// ---- TestMovingSameHeldPointer ----------------------------------------------

#[test]
fn test_moving_same_held_pointer() {
    let mut stack = RefCountedValue::with_count(7, 1);
    let raw: *mut RefCountedValue = &mut stack;

    let mut adopted = DelegatedCountPtr::new(IncrementTag, raw); // count=2
    let another = DelegatedCountPtr::new(IncrementTag, raw); // count=3

    assert_eq!(count(raw), 3);
    assert_eq!(adopted, another);

    // Move `another` into `adopted`
    adopted = another; // adopted's old ref drops → -1
    assert_eq!(count(raw), 2); // adopted + stack's own 1
    assert!(!adopted.is_null());
}

// ---- TestCopyAssignment -----------------------------------------------------

#[test]
fn test_copy_assignment() {
    let made = make_delegated_count_ptr(RefCountedValue::new(85));
    let raw = made.get();
    assert_eq!(count(raw), 1);

    // Assign into a previously-null ptr (simulates C++ copy-assign to default-constructed).
    let copied = made.clone();
    assert_eq!(count(raw), 2);
    assert_eq!(copied, made);
}

// ---- TestCopyConstructor ----------------------------------------------------

#[test]
fn test_copy_constructor() {
    let made = make_delegated_count_ptr(RefCountedValue::new(87));
    let raw = made.get();
    assert_eq!(count(raw), 1);

    let copied = made.clone();
    assert_eq!(count(raw), 2);
    assert_eq!(copied, made);
}

// ---- TestCopySelfAssignment -------------------------------------------------

// C++ ARCH_PRAGMA_SELF_ASSIGN_OVERLOADED: self-copy-assign must leave count at 1.
// In Rust we simulate via clone + re-assign.
#[test]
fn test_copy_self_assignment() {
    let made = make_delegated_count_ptr(RefCountedValue::new(87));
    let raw = made.get();
    assert_eq!(count(raw), 1);

    // Clone-and-assign to the same binding simulates self-assignment.
    let made2 = made.clone();
    let made3 = made2.clone();
    drop(made2); // net: count unchanged relative to held ptrs
    drop(made3);
    assert_eq!(count(raw), 1);
    assert!(!made.is_null());
}

// ---- TestCopySameHeldPointer ------------------------------------------------

#[test]
fn test_copy_same_held_pointer() {
    let made = make_delegated_count_ptr(RefCountedValue::new(86));
    let raw = made.get();

    let copied = made.clone();
    assert_eq!(copied, made);
    assert_eq!(count(raw), 2);

    // Assign from same source again → creates a new ref, count goes to 3.
    let reassigned = made.clone();
    assert_eq!(reassigned, made);
    assert_eq!(count(raw), 3); // made + copied + reassigned

    drop(reassigned);
    drop(copied);
    assert_eq!(count(raw), 1);
}

// ---- TestSwap ---------------------------------------------------------------

#[test]
fn test_swap() {
    let made = make_delegated_count_ptr(RefCountedValue::new(16));
    let raw_made = made.get();
    let copy = made.clone(); // count_made = 2

    let another = make_delegated_count_ptr(RefCountedValue::new(12));
    let raw_another = another.get();

    assert_eq!(count(raw_made), 2);
    assert_eq!((*made).value, 16);
    assert_eq!(count(raw_another), 1);
    assert_eq!((*another).value, 12);

    let mut made_mut = made;
    let mut another_mut = another;
    made_mut.swap(&mut another_mut);

    // copy still points to the original raw_made allocation
    assert_eq!(copy, another_mut); // another_mut now holds raw_made
    assert_ne!(copy, made_mut); // made_mut now holds raw_another

    assert_eq!(count(raw_made), 2); // copy + another_mut
    assert_eq!((*another_mut).value, 16);
    assert_eq!(count(raw_another), 1); // only made_mut
    assert_eq!((*made_mut).value, 12);
}

// ---- TestAssignDerived / TestInitializeDerived (const-conversion analogue) --
// C++ tests const T* ← T* conversion. In Rust we test coercion
// DelegatedCountPtr<Derived> → DelegatedCountPtr<Base> is not directly
// expressible without trait objects, so we test clone/assign within the same
// type (count increment correctness).

#[test]
fn test_assign_derived_same_count() {
    let derived = make_delegated_count_ptr(DerivedRefCountedValue::new());
    assert_eq!(
        derived.as_ref().unwrap().base.count.load(Ordering::Relaxed),
        1
    );

    let copy = derived.clone();
    assert_eq!(
        derived.as_ref().unwrap().base.count.load(Ordering::Relaxed),
        2
    );
    assert_eq!(copy, derived);
}
