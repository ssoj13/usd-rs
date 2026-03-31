// Port of testenv/stopwatch.cpp

use std::thread;
use std::time::{Duration, Instant};
use usd_tf::stopwatch::Stopwatch;

/// Returns true when `value` is within `epsilon` (relative) of `reference`.
fn is_close(value: f64, reference: f64, epsilon: f64) -> bool {
    let diff = (value - reference).abs();
    diff <= epsilon * reference.abs()
}

/// Returns true when `lower * (1-eps) <= value <= upper * (1+eps)`.
fn is_closely_bounded(value: f64, lower: f64, upper: f64, epsilon: f64) -> bool {
    (1.0 - epsilon) * lower <= value && value <= (1.0 + epsilon) * upper
}

// ---------------------------------------------------------------------------
// Constructor / copy
// ---------------------------------------------------------------------------

#[test]
fn default_stopwatch_is_zero() {
    let watch = Stopwatch::new();
    assert_eq!(watch.seconds(), 0.0, "new stopwatch must report 0 seconds");
}

#[test]
fn clone_of_new_is_zero() {
    // Mirrors C++ copy-constructor test: watchCopy.GetSeconds() == watch1.GetSeconds()
    let watch1 = Stopwatch::new();
    let watch_copy = watch1.clone();
    assert_eq!(
        watch_copy.seconds(),
        watch1.seconds(),
        "cloned stopwatch must equal original"
    );
}

// ---------------------------------------------------------------------------
// Start / Stop timing
// ---------------------------------------------------------------------------

#[test]
fn measures_approximately_500ms() {
    // Mirrors: watch1.Start(); sleep 500ms; watch1.Stop();
    // Then checks  minElapsed < watch1.GetSeconds() < maxElapsed.
    let mut watch = Stopwatch::new();

    let pre_start = Instant::now();
    watch.start();
    let post_start = Instant::now();

    thread::sleep(Duration::from_millis(500));

    let pre_stop = Instant::now();
    watch.stop();
    let post_stop = Instant::now();

    let min_elapsed = pre_stop.duration_since(post_start).as_secs_f64();
    let max_elapsed = post_stop.duration_since(pre_start).as_secs_f64();

    assert!(
        is_closely_bounded(watch.seconds(), min_elapsed, max_elapsed, 1e-3),
        "expected ~0.5s but got {:.4}s (bounds [{:.4}, {:.4}])",
        watch.seconds(),
        min_elapsed,
        max_elapsed,
    );
}

#[test]
fn accumulates_two_500ms_intervals() {
    // Mirrors: second Start/Stop after the first → cumulative ~1 s.
    let mut watch = Stopwatch::new();

    let pre_start1 = Instant::now();
    watch.start();
    let post_start1 = Instant::now();
    thread::sleep(Duration::from_millis(500));
    let pre_stop1 = Instant::now();
    watch.stop();
    let post_stop1 = Instant::now();

    let pre_start2 = Instant::now();
    watch.start();
    let post_start2 = Instant::now();
    thread::sleep(Duration::from_millis(500));
    let pre_stop2 = Instant::now();
    watch.stop();
    let post_stop2 = Instant::now();

    let min_elapsed = pre_stop1.duration_since(post_start1).as_secs_f64()
        + pre_stop2.duration_since(post_start2).as_secs_f64();
    let max_elapsed = post_stop1.duration_since(pre_start1).as_secs_f64()
        + post_stop2.duration_since(pre_start2).as_secs_f64();

    assert!(
        is_closely_bounded(watch.seconds(), min_elapsed, max_elapsed, 1e-3),
        "expected ~1.0s but got {:.4}s (bounds [{:.4}, {:.4}])",
        watch.seconds(),
        min_elapsed,
        max_elapsed,
    );
}

// ---------------------------------------------------------------------------
// Clone does not share state with the original
// ---------------------------------------------------------------------------

#[test]
fn clone_does_not_accumulate_with_original() {
    // Mirrors: watchCopy must stay at 0 while watch1 runs.
    let watch1 = Stopwatch::new();
    let watch_copy = watch1.clone();

    // watch_copy was never started — must remain 0
    assert_eq!(
        watch_copy.seconds(),
        0.0,
        "cloned stopwatch must not share timing with original"
    );
    let _ = watch1; // silence unused warning
}

// ---------------------------------------------------------------------------
// AddFrom
// ---------------------------------------------------------------------------

#[test]
fn add_from_equals_original() {
    // First AddFrom: watchCopy.AddFrom(watch1) → watchCopy ≈ watch1
    let mut watch1 = Stopwatch::new();
    watch1.start();
    thread::sleep(Duration::from_millis(100));
    watch1.stop();

    let mut watch_copy = Stopwatch::new();
    watch_copy.add_from(&watch1);

    assert!(
        is_close(watch_copy.seconds(), watch1.seconds(), 1e-3),
        "after AddFrom, copy ({:.6}s) must equal original ({:.6}s)",
        watch_copy.seconds(),
        watch1.seconds(),
    );
}

#[test]
fn add_from_twice_doubles_time() {
    // Second AddFrom: watchCopy.AddFrom(watch1) again → watchCopy ≈ 2 * watch1
    let mut watch1 = Stopwatch::new();
    watch1.start();
    thread::sleep(Duration::from_millis(100));
    watch1.stop();

    let mut watch_copy = Stopwatch::new();
    watch_copy.add_from(&watch1);
    watch_copy.add_from(&watch1);

    let ratio = watch_copy.seconds() / watch1.seconds();
    assert!(
        is_close(ratio, 2.0, 1e-3),
        "after two AddFrom calls ratio must be ~2.0, got {:.6}",
        ratio,
    );
}

// ---------------------------------------------------------------------------
// Reset
// ---------------------------------------------------------------------------

#[test]
fn reset_returns_to_zero() {
    // Mirrors: watchCopy.Reset() → GetSeconds() == 0
    let mut watch = Stopwatch::new();
    watch.start();
    thread::sleep(Duration::from_millis(10));
    watch.stop();

    assert!(
        watch.seconds() > 0.0,
        "watch must have nonzero time before reset"
    );

    watch.reset();

    assert_eq!(watch.seconds(), 0.0, "reset watch must report 0 seconds");
    assert_eq!(watch.sample_count(), 0, "reset watch must report 0 samples");
}
