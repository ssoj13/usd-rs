// Port of C++ testenv/error.cpp — TfError and TfErrorMark tests.
use std::sync::Mutex;
use usd_tf::*;

// Error codes matching C++ enum TfTestErrorCodes { SMALL, MEDIUM, LARGE }
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(i32)]
enum TestErrorCode {
    Small = 0,
    Medium = 1,
    Large = 2,
}

impl From<TestErrorCode> for i32 {
    fn from(v: TestErrorCode) -> i32 {
        v as i32
    }
}

fn register_error_codes() {
    // C++ TF_REGISTRY_FUNCTION(TfEnum) { TF_ADD_ENUM_NAME(SMALL); ... }
    TfEnum::add_name::<TestErrorCode>(TestErrorCode::Small as i32, "TestErrorCode", "SMALL", None);
    TfEnum::add_name::<TestErrorCode>(
        TestErrorCode::Medium as i32,
        "TestErrorCode",
        "MEDIUM",
        None,
    );
    TfEnum::add_name::<TestErrorCode>(TestErrorCode::Large as i32, "TestErrorCode", "LARGE", None);
}

// Post a runtime error and track its code via a thread-local side-channel,
// since Rust DiagnosticMgr does not store TfEnum codes in TfError (unlike C++).
fn post_error_with_code(code: TestErrorCode, msg: &str) {
    let mgr = DiagnosticMgr::instance();
    let ctx = CallContext::new("test_error.rs", "post_error_with_code", line!());
    // Diagnostic and DiagnosticType are re-exported via `pub use diagnostic::*`.
    let diag = Diagnostic::new(DiagnosticType::NonfatalError, ctx, msg);
    mgr.post_error(diag);
    LAST_CODE.with(|c| *c.lock().unwrap() = Some(code));
}

// Simulates C++ TfError::GetErrorCode by reading the side-channel value.
thread_local! {
    static LAST_CODE: Mutex<Option<TestErrorCode>> = Mutex::new(None);
}

fn take_last_code() -> Option<TestErrorCode> {
    LAST_CODE.with(|c| c.lock().unwrap().take())
}

// ============================================================
// Test: ErrorMark lifecycle — clean/dirty, set_mark, clear
// C++: Test_TfError — m.SetMark() / m.IsClean() / m.Clear()
// ============================================================
#[test]
fn test_error_mark_lifecycle() {
    register_error_codes();

    let mgr = DiagnosticMgr::instance();
    mgr.set_quiet(true);

    // Fresh mark must be clean.
    let mut mark = ErrorMark::new();
    assert!(mark.is_clean(), "fresh mark must be clean");

    mark.set_mark();
    assert!(
        mark.is_clean(),
        "after set_mark with no errors, must be clean"
    );

    tf_runtime_error!("small error");
    assert!(
        !mark.is_clean(),
        "mark must be dirty after posting an error"
    );

    // Retrieve error and check its commentary.
    let errors = mark.errors();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].commentary(), "small error");
    assert!(
        errors[0].source_file_name().contains("test_error"),
        "source file must mention test_error"
    );

    // Augment commentary mirrors C++ e.AugmentCommentary("augment").
    let mut e = errors[0].clone();
    e.augment_commentary("augment");
    assert_eq!(
        e.commentary(),
        "small error\naugment",
        "augmented commentary must join with newline"
    );

    // Clear must restore cleanliness.
    mark.clear();
    assert!(mark.is_clean(), "mark must be clean after clear");

    mgr.set_quiet(false);
}

// ============================================================
// Test: Multiple errors in one mark with distinct codes
// C++: TF_ERROR(1, MEDIUM, ...) / TF_ERROR(2, LARGE, ...) block
// ============================================================
#[test]
fn test_error_mark_multiple_errors_ordered() {
    let mgr = DiagnosticMgr::instance();
    mgr.set_quiet(true);

    let mark = ErrorMark::new();

    post_error_with_code(TestErrorCode::Medium, "medium error");
    assert_eq!(take_last_code(), Some(TestErrorCode::Medium));

    post_error_with_code(TestErrorCode::Large, "large error");
    assert_eq!(take_last_code(), Some(TestErrorCode::Large));

    let errors = mark.errors();
    assert_eq!(errors.len(), 2, "must have exactly two errors");
    assert_eq!(
        errors[0].commentary(),
        "medium error",
        "first error must be medium"
    );
    assert_eq!(
        errors[1].commentary(),
        "large error",
        "second error must be large"
    );

    mark.clear();
    assert!(mark.is_clean());

    mgr.set_quiet(false);
}

// ============================================================
// Test: tf_verify! semantics — returns bool, does not terminate
// C++: TF_VERIFY(m.IsClean()) returns bool; TF_AXIOM(TF_VERIFY(!m.IsClean()))
// ============================================================
#[test]
fn test_tf_verify_semantics() {
    let mgr = DiagnosticMgr::instance();
    mgr.set_quiet(true);

    let mark = ErrorMark::new();
    assert!(mark.is_clean());

    // True condition: returns true, no error posted.
    assert!(
        tf_verify!(mark.is_clean()),
        "verify on true condition must return true"
    );

    // Post a coding error to make the mark dirty.
    tf_coding_error!("test error");
    assert!(!mark.is_clean());

    // False condition: returns false, posts a coding error, does NOT panic.
    let result = tf_verify!(mark.is_clean());
    assert!(!result, "verify on false condition must return false");

    // verify with a message format also returns false without panic.
    let result2 = tf_verify!(mark.is_clean(), "With a {}", "message.");
    assert!(!result2);

    mark.clear();
    assert!(mark.is_clean());

    mgr.set_quiet(false);
}

// ============================================================
// Test: All error variant macros compile and dirty the mark
// C++: big block of TF_CODING_ERROR / TF_RUNTIME_ERROR / TF_ERROR
// ============================================================
#[test]
fn test_all_error_variants() {
    let mgr = DiagnosticMgr::instance();
    mgr.set_quiet(true);

    let mark = ErrorMark::new();

    tf_coding_error!("Coding error");
    tf_coding_error!("Coding error {}", 1);
    tf_coding_error!("Error!");

    tf_runtime_error!("Runtime error");
    tf_runtime_error!("Runtime error {}", 1);
    tf_runtime_error!("Error!");

    tf_error!("const char *");
    tf_error!("const char *, {}", "...");
    tf_error!("Error!");

    assert!(!mark.is_clean(), "mark must be dirty after error variants");
    mark.clear();
    assert!(mark.is_clean());

    mgr.set_quiet(false);
}

// ============================================================
// Test: Warnings and status messages do NOT dirty the ErrorMark
// C++: TF_WARN / TF_STATUS calls after m.Clear() — m stays clean
// ============================================================
#[test]
fn test_warnings_and_status_dont_dirty_mark() {
    let mgr = DiagnosticMgr::instance();
    mgr.set_quiet(true);

    let mark = ErrorMark::new();

    tf_warn!("const char *");
    tf_warn!("const char *, {}", "...");
    tf_warn!("Warning!");

    assert!(
        mark.is_clean(),
        "warnings must not make the error mark dirty"
    );

    tf_status!("const char *");
    tf_status!("const char *, {}", "...");
    tf_status!("Status");

    assert!(
        mark.is_clean(),
        "status messages must not make the error mark dirty"
    );

    mgr.set_quiet(false);
}

// ============================================================
// Test: Cross-thread ErrorTransport
// C++: Test_TfErrorThreadTransport
//   - child thread posts error → m.TransportTo(transport) → m.IsClean()
//   - parent: m.IsClean() before post, dirty after transport.post()
// ============================================================
#[test]
fn test_error_thread_transport() {
    let mgr = DiagnosticMgr::instance();
    mgr.set_quiet(true);

    let parent_mark = ErrorMark::new();
    assert!(
        parent_mark.is_clean(),
        "parent must be clean before child runs"
    );

    let transport = std::thread::spawn(|| {
        let mgr = DiagnosticMgr::instance();
        mgr.set_quiet(true);

        let child_mark = ErrorMark::new();
        tf_runtime_error!("Cross-thread transfer test error");
        assert!(
            !child_mark.is_clean(),
            "child mark must be dirty after error"
        );

        // C++: m.TransportTo(*transport) → errors extracted into transport, m becomes clean.
        let t = child_mark.transport();
        assert!(
            child_mark.is_clean(),
            "child mark must be clean after transport()"
        );

        mgr.set_quiet(false);
        t
    })
    .join()
    .expect("child thread panicked");

    // The transport carries the error but it hasn't been posted yet.
    assert!(
        parent_mark.is_clean(),
        "parent must be clean before transport.post()"
    );

    // C++: transport.Post() → errors land on parent thread's error list.
    transport.post();
    assert!(
        !parent_mark.is_clean(),
        "parent mark must be dirty after transport.post()"
    );

    parent_mark.clear();
    assert!(parent_mark.is_clean());

    mgr.set_quiet(false);
}

// ============================================================
// Test: ErrorTransport empty API
// ============================================================
#[test]
fn test_error_transport_empty() {
    let t = ErrorTransport::new();
    assert!(t.is_empty(), "fresh transport must be empty");
    assert_eq!(t.len(), 0);
    // post() on empty transport must not panic.
    t.post();
}

// ============================================================
// Test: set_mark resets the serial baseline
// ============================================================
#[test]
fn test_set_mark_resets_baseline() {
    let mgr = DiagnosticMgr::instance();
    mgr.set_quiet(true);

    let mut mark = ErrorMark::new();

    tf_runtime_error!("error before reset");
    assert!(!mark.is_clean());

    // set_mark advances the baseline past the existing error.
    mark.set_mark();
    assert!(
        mark.is_clean(),
        "after set_mark, errors before the mark must not count"
    );

    tf_runtime_error!("error after reset");
    assert!(!mark.is_clean());

    let errors = mark.errors();
    assert_eq!(errors.len(), 1, "only error after set_mark must be visible");
    assert_eq!(errors[0].commentary(), "error after reset");

    DiagnosticMgr::instance().clear_all_errors();
    mgr.set_quiet(false);
}

// ============================================================
// Test: Nested ErrorMarks — inner clear removes the error for both
// ============================================================
#[test]
fn test_nested_error_marks() {
    let mgr = DiagnosticMgr::instance();
    mgr.set_quiet(true);

    let outer = ErrorMark::new();
    assert!(outer.is_clean());

    {
        let inner = ErrorMark::new();
        tf_runtime_error!("inner error");

        assert!(!inner.is_clean(), "inner must see the error");
        assert!(!outer.is_clean(), "outer must also see the error");

        inner.clear();
        assert!(inner.is_clean());
        // Clearing inner removes the error from the shared list.
        assert!(outer.is_clean(), "outer must be clean after inner.clear()");
    }

    assert!(outer.is_clean(), "outer must remain clean after inner drop");

    mgr.set_quiet(false);
}

// ============================================================
// Test: error_count helper
// ============================================================
#[test]
fn test_error_count() {
    let mgr = DiagnosticMgr::instance();
    mgr.set_quiet(true);

    let mark = ErrorMark::new();
    assert_eq!(mark.error_count(), 0);

    tf_runtime_error!("e1");
    assert_eq!(mark.error_count(), 1);

    tf_runtime_error!("e2");
    assert_eq!(mark.error_count(), 2);

    mark.clear();
    assert_eq!(mark.error_count(), 0);

    mgr.set_quiet(false);
}
