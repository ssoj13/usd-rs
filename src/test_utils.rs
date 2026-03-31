//! Test utilities for usd-rs.
//!
//! Provides timeout and other safety mechanisms for tests.

use std::panic::{self, AssertUnwindSafe};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

/// Default test timeout in seconds.
pub const DEFAULT_TEST_TIMEOUT_SECS: u64 = 120;

/// Runs a closure with a timeout, panicking if it exceeds the limit.
///
/// This helps detect deadlocks in tests instead of hanging forever.
///
/// # Arguments
///
/// * `timeout` - Maximum duration to wait
/// * `name` - Test name for error messages
/// * `f` - The test closure to run
///
/// # Panics
///
/// Panics if the closure takes longer than the timeout.
///
/// # Examples
///
/// ```ignore
/// use usd::test_utils::run_with_timeout;
/// use std::time::Duration;
///
/// run_with_timeout(Duration::from_secs(5), "my_test", || {
///     // test code here
/// });
/// ```
pub fn run_with_timeout<F, R>(timeout: Duration, name: &str, f: F) -> R
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    let (tx, rx) = mpsc::channel();
    let name_owned = name.to_string();

    let handle = thread::spawn(move || {
        let result = panic::catch_unwind(AssertUnwindSafe(f));
        let _ = tx.send(result);
    });

    match rx.recv_timeout(timeout) {
        Ok(Ok(result)) => result,
        Ok(Err(panic_payload)) => {
            // Test panicked - propagate it
            panic::resume_unwind(panic_payload);
        }
        Err(mpsc::RecvTimeoutError::Timeout) => {
            // Timeout! Don't wait for the thread - it might be deadlocked
            panic!(
                "TEST TIMEOUT: '{}' exceeded {} seconds - possible deadlock!",
                name_owned,
                timeout.as_secs()
            );
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            // Thread died without sending result
            let _ = handle.join();
            panic!(
                "TEST CRASHED: '{}' thread terminated unexpectedly",
                name_owned
            );
        }
    }
}

/// Macro to run a test with the default timeout (2 minutes).
///
/// # Examples
///
/// ```ignore
/// use usd::test_timeout;
///
/// #[test]
/// fn my_test() {
///     test_timeout!("my_test", {
///         // test code
///     });
/// }
/// ```
#[macro_export]
macro_rules! test_timeout {
    ($name:expr, $body:block) => {{
        $crate::test_utils::run_with_timeout(
            std::time::Duration::from_secs($crate::test_utils::DEFAULT_TEST_TIMEOUT_SECS),
            $name,
            move || $body,
        )
    }};
    ($name:expr, $timeout_secs:expr, $body:block) => {{
        $crate::test_utils::run_with_timeout(
            std::time::Duration::from_secs($timeout_secs),
            $name,
            move || $body,
        )
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timeout_success() {
        let result = run_with_timeout(Duration::from_secs(5), "quick_test", || {
            thread::sleep(Duration::from_millis(10));
            42
        });
        assert_eq!(result, 42);
    }

    #[test]
    #[should_panic(expected = "TEST TIMEOUT")]
    fn test_timeout_exceeded() {
        run_with_timeout(Duration::from_millis(50), "slow_test", || {
            thread::sleep(Duration::from_secs(10));
        });
    }

    #[test]
    #[should_panic(expected = "intentional panic")]
    fn test_panic_propagation() {
        run_with_timeout(Duration::from_secs(5), "panic_test", || {
            panic!("intentional panic");
        });
    }
}
