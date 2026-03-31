// Port of testenv/exception.cpp
//
// C++ original tests:
//   1. PXR_TF_THROW(Tf_TestException, "test exception 1") —
//      caught as TfBaseException; what() matches; GetThrowContext() is set.
//   2. PXR_TF_THROW(Tf_TestException, TfSkipCallerFrames(2), "test exception 2") —
//      same checks but with a skip-frames hint.
//
// In Rust there is no inheritance hierarchy — TfException is the concrete
// base type, and "derived" exceptions are represented by wrapping it or by
// creating a newtype.  The tf_throw! macro panics with the TfException
// message; we use std::panic::catch_unwind to mirror the try/catch.

use usd_tf::call_context;
use usd_tf::exception::{SkipCallerFrames, TfException};
use usd_tf::tf_exception;

// ---------------------------------------------------------------------------
// Test 1 — basic TfException: message and context
// ---------------------------------------------------------------------------

#[test]
fn exception_message_and_context() {
    // Mirrors: PXR_TF_THROW(Tf_TestException, "test exception 1")
    let exc = TfException::new("test exception 1", call_context!());

    // what() equivalent: Display / message()
    assert_eq!(
        exc.message(),
        "test exception 1",
        "message must match the throw argument"
    );

    // GetThrowContext() must be non-empty (file field populated by macro)
    let ctx = exc.throw_context();
    assert!(
        !ctx.file().is_empty(),
        "throw context must have a non-empty file path"
    );
}

// ---------------------------------------------------------------------------
// Test 2 — TfException with SkipCallerFrames hint
// ---------------------------------------------------------------------------

#[test]
fn exception_with_skip_caller_frames() {
    // Mirrors: PXR_TF_THROW(Tf_TestException, TfSkipCallerFrames(2), "test exception 2")
    let exc = TfException::with_stack(
        "test exception 2",
        call_context!(),
        SkipCallerFrames::new(2),
    );

    assert_eq!(
        exc.message(),
        "test exception 2",
        "message must match with skip-frames path"
    );

    let ctx = exc.throw_context();
    assert!(
        !ctx.file().is_empty(),
        "throw context must be populated even with skip-frames"
    );
}

// ---------------------------------------------------------------------------
// Test: tf_exception! macro produces correct message
// ---------------------------------------------------------------------------

#[test]
fn tf_exception_macro_message() {
    let exc = tf_exception!("test exception 1");
    assert_eq!(exc.message(), "test exception 1");
}

#[test]
fn tf_exception_macro_formatted() {
    let exc = tf_exception!("value is {}", 42);
    assert_eq!(exc.message(), "value is 42");
}

// ---------------------------------------------------------------------------
// Test: exception implements std::error::Error
// ---------------------------------------------------------------------------

#[test]
fn exception_implements_error_trait() {
    use std::error::Error;

    let exc = TfException::new("some error", call_context!());
    // Coerce to &dyn Error to confirm the trait is satisfied.
    let err: &dyn Error = &exc;
    assert!(err.to_string().contains("some error"));
}

// ---------------------------------------------------------------------------
// Test: throw and catch via panic/catch_unwind  (mirrors try/catch in C++)
// ---------------------------------------------------------------------------

#[test]
fn throw_and_catch_via_panic() {
    // tf_throw! panics with the exception message; we catch with catch_unwind.
    let result = std::panic::catch_unwind(|| {
        usd_tf::tf_throw!("test exception 1");
    });

    let payload = result.expect_err("tf_throw! must cause a panic");

    // The panic payload contains the Display string of TfException.
    let msg = if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else if let Some(s) = payload.downcast_ref::<&str>() {
        s.to_string()
    } else {
        String::new()
    };

    assert!(
        msg.contains("test exception 1"),
        "caught panic payload must contain the exception message, got: {:?}",
        msg
    );
}

// ---------------------------------------------------------------------------
// Test: SkipCallerFrames constructors and default
// ---------------------------------------------------------------------------

#[test]
fn skip_caller_frames_constructors() {
    let skip = SkipCallerFrames::new(5);
    assert_eq!(skip.num_to_skip, 5);

    let skip: SkipCallerFrames = 3.into();
    assert_eq!(skip.num_to_skip, 3);

    let skip = SkipCallerFrames::default();
    assert_eq!(skip.num_to_skip, 0);
}

// ---------------------------------------------------------------------------
// Test: TfException clone and Debug
// ---------------------------------------------------------------------------

#[test]
fn exception_clone_and_debug() {
    let exc1 = TfException::new("clone test", call_context!());
    let exc2 = exc1.clone();
    assert_eq!(exc1.message(), exc2.message());

    let dbg = format!("{:?}", exc1);
    assert!(dbg.contains("TfException"));
    assert!(dbg.contains("clone test"));
}
