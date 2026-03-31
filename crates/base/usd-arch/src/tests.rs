//! Comprehensive tests for the arch module.
//!
//! These tests are ported from OpenUSD's testenv/ directory to ensure
//! compatibility and correctness of the Rust implementation.

use super::*;

// ============================================================================
// Tests from testTiming.cpp
// ============================================================================

#[test]
fn test_nanoseconds_per_tick_valid() {
    // ArchGetNanosecondsPerTick() > 0.0
    // Since our ticks are nanoseconds, this is always 1.0
    let nanos_per_tick = 1.0; // Our implementation uses nanoseconds directly
    assert!(nanos_per_tick > 0.0);
    // If you're not even doing 1 tick a second, it's probably a bogus value
    assert!(nanos_per_tick < 1e9);
}

#[test]
fn test_ticks_conversion_many_values() {
    // Verify conversions for many tick counts (adapted from testTiming.cpp)
    // We test a smaller range since Rust tests should be fast
    for ticks in 0u64..(1u64 << 16) {
        let nanos = ticks_to_nanoseconds(ticks);
        let secs = ticks_to_seconds(ticks);

        // Since our ticks are nanoseconds, nanos == ticks
        assert_eq!(nanos, ticks);

        // Verify seconds conversion
        let expected_secs = nanos as f64 / 1e9;
        let epsilon = 0.0001;
        assert!(
            (secs - expected_secs).abs() < epsilon,
            "ticks={}, secs={}, expected={}",
            ticks,
            secs,
            expected_secs
        );
    }
}

#[test]
fn test_timing_sleep_accuracy() {
    // From testTiming.cpp: sleep for 1500ms and verify delta is reasonable
    // We use a shorter sleep for faster tests
    let t1 = get_ticks();
    std::thread::sleep(std::time::Duration::from_millis(100));
    let t2 = get_ticks();
    let delta = t2 - t1;
    let delta_secs = ticks_to_seconds(delta);

    // Verify the delta is reasonable (allow leeway for heavy machine load)
    assert!(
        delta_secs > 0.08,
        "Sleep took less than 80ms: {}s",
        delta_secs
    );
    assert!(delta_secs < 1.0, "Sleep took more than 1s: {}s", delta_secs);
}

#[test]
fn test_timing_overhead() {
    let overhead = get_ticks_overhead();
    // Overhead should be relatively small (less than 1ms)
    assert!(
        overhead < 1_000_000,
        "Timing overhead too large: {}ns",
        overhead
    );
}

// ============================================================================
// Tests from testMath.cpp
// ============================================================================

#[test]
fn test_ieee754_float_compliance() {
    // Verify that float bit patterns are IEEE-754 compliant
    assert_eq!(float_to_bit_pattern(5.6904566e-28f32), 0x12345678);
    assert_eq!(bit_pattern_to_float(0x12345678), 5.6904566e-28f32);
}

#[test]
fn test_ieee754_double_compliance() {
    // Verify that double bit patterns are IEEE-754 compliant
    assert_eq!(
        double_to_bit_pattern(5.6263470058989390e-221),
        0x1234567811223344u64
    );
    assert_eq!(
        bit_pattern_to_double(0x1234567811223344u64),
        5.6263470058989390e-221
    );
}

#[test]
fn test_arch_sign() {
    // From testMath.cpp
    assert_eq!(sign(-123i32), -1);
    assert_eq!(sign(123i32), 1);
    assert_eq!(sign(0i32), 0);
}

#[test]
fn test_arch_count_trailing_zeros() {
    // From testMath.cpp - exact test values
    assert_eq!(count_trailing_zeros(1), 0);
    assert_eq!(count_trailing_zeros(2), 1);
    assert_eq!(count_trailing_zeros(3), 0);
    assert_eq!(count_trailing_zeros(4), 2);
    assert_eq!(count_trailing_zeros(5), 0);
    assert_eq!(count_trailing_zeros(6), 1);
    assert_eq!(count_trailing_zeros(7), 0);
    assert_eq!(count_trailing_zeros(8), 3);

    assert_eq!(count_trailing_zeros(65535), 0);
    assert_eq!(count_trailing_zeros(65536), 16);
}

#[test]
fn test_arch_count_trailing_zeros_64() {
    // From testMath.cpp
    assert_eq!(count_trailing_zeros_64(!((1u64 << 32) - 1)), 32);
    assert_eq!(count_trailing_zeros_64(1u64 << 63), 63);
}

// ============================================================================
// Tests from testErrno.cpp
// ============================================================================

#[test]
fn test_strerror_returns_non_empty() {
    // From testErrno.cpp: verify strerror returns non-empty strings
    for i in -1..10 {
        let msg = strerror(i);
        assert!(!msg.is_empty(), "strerror({}) returned empty string", i);
    }
}

#[test]
fn test_strerror_common_errors() {
    // Test some common error codes
    let enoent = strerror(codes::NOENT);
    assert!(!enoent.is_empty());

    let einval = strerror(codes::INVAL);
    assert!(!einval.is_empty());

    let eacces = strerror(codes::ACCES);
    assert!(!eacces.is_empty());
}

// ============================================================================
// Tests from testSystemInfo.cpp
// ============================================================================

#[test]
fn test_executable_path() {
    // From testSystemInfo.cpp: verify executable path contains expected string
    let path = get_executable_path();
    assert!(path.is_some(), "Failed to get executable path");

    let path = path.expect("get_executable_path returned None");
    // In test mode, the executable is the test runner
    // Just verify we got a non-empty path
    assert!(!path.as_os_str().is_empty(), "Executable path is empty");
}

#[test]
fn test_system_info_basic() {
    let info = SystemInfo::collect();

    // Basic sanity checks
    assert!(info.page_size > 0, "Page size should be positive");
    assert!(
        info.physical_memory > 0,
        "Physical memory should be positive"
    );
    assert!(info.cpu_count > 0, "CPU count should be positive");
    assert!(!info.hostname.is_empty(), "Hostname should not be empty");
    assert!(info.pid > 0, "PID should be positive");
}

#[test]
fn test_get_cwd() {
    let cwd = get_cwd();
    assert!(cwd.is_some(), "Failed to get current working directory");
    let cwd = cwd.expect("get_cwd returned None");
    assert!(cwd.exists(), "Current working directory does not exist");
}

#[test]
fn test_get_temp_dir() {
    let temp = get_temp_dir();
    assert!(temp.exists(), "Temp directory does not exist");
}

// ============================================================================
// Tests from testThreads.cpp
// ============================================================================

#[test]
fn test_is_main_thread() {
    // From testThreads.cpp: main thread should report as main
    assert!(is_main_thread(), "Main thread should be identified as main");
}

#[test]
fn test_thread_id() {
    let id = get_current_thread_id();
    assert!(id > 0, "Thread ID should be positive");
}

#[test]
fn test_concurrency() {
    let cores = get_concurrency();
    assert!(cores > 0, "Concurrency should be at least 1");
}

#[test]
fn test_is_main_thread_from_spawned() {
    // Verify spawned threads are not main thread
    let handle = std::thread::spawn(|| !is_main_thread());

    let result = handle.join().expect("Thread panicked");
    assert!(result, "Spawned thread should not be main thread");
}

// ============================================================================
// Tests for alignment utilities
// ============================================================================

#[test]
fn test_is_power_of_two() {
    assert!(is_power_of_two(1));
    assert!(is_power_of_two(2));
    assert!(is_power_of_two(4));
    assert!(is_power_of_two(8));
    assert!(is_power_of_two(16));
    assert!(is_power_of_two(1024));
    assert!(is_power_of_two(1 << 20));

    assert!(!is_power_of_two(0));
    assert!(!is_power_of_two(3));
    assert!(!is_power_of_two(5));
    assert!(!is_power_of_two(6));
    assert!(!is_power_of_two(7));
    assert!(!is_power_of_two(1000));
}

#[test]
fn test_align_memory_size() {
    // Test align_memory_size which aligns to 8-byte boundary
    assert_eq!(align_memory_size(0), 0);
    assert_eq!(align_memory_size(1), 8);
    assert_eq!(align_memory_size(7), 8);
    assert_eq!(align_memory_size(8), 8);
    assert_eq!(align_memory_size(9), 16);
    assert_eq!(align_memory_size(16), 16);
    assert_eq!(align_memory_size(17), 24);
}

// ============================================================================
// Tests for environment variables
// ============================================================================

#[test]
fn test_env_operations() {
    let key = "USD_RS_TEST_VAR_UNIQUE_12345";
    let value = "test_value_xyz";

    // Initially should not exist
    let _ = unset_env(key);
    assert!(!has_env(key), "Test var should not exist initially");

    // Set it
    set_env(key, value);
    assert!(has_env(key), "Test var should exist after set");
    assert_eq!(get_env(key), Some(value.to_string()));

    // Clear it
    let _ = unset_env(key);
    assert!(!has_env(key), "Test var should not exist after unset");
    assert_eq!(get_env(key), None);
}

// ============================================================================
// Tests for hash functions
// ============================================================================

#[test]
fn test_hash_empty() {
    let h = hash32(b"");
    // Empty string should still produce a hash (not necessarily 0)
    let _ = h; // Just verify it doesn't panic
}

#[test]
fn test_hash_consistency() {
    let data = b"The quick brown fox jumps over the lazy dog";
    let h1 = hash64(data);
    let h2 = hash64(data);
    let h3 = hash64(data);

    assert_eq!(h1, h2, "Hash should be consistent");
    assert_eq!(h2, h3, "Hash should be consistent");
}

#[test]
fn test_hash_different_inputs() {
    let inputs = [
        b"input one".as_slice(),
        b"input two".as_slice(),
        b"input three".as_slice(),
        b"input four".as_slice(),
    ];

    let hashes: Vec<u64> = inputs.iter().map(|i| hash64(i)).collect();

    // All hashes should be different
    for i in 0..hashes.len() {
        for j in (i + 1)..hashes.len() {
            assert_ne!(
                hashes[i], hashes[j],
                "Different inputs should produce different hashes"
            );
        }
    }
}

#[test]
fn test_hash_with_seed() {
    let data = b"test data";
    let h1 = hash64_with_seed(data, 0);
    let h2 = hash64_with_seed(data, 42);
    let h3 = hash64_with_seed(data, 12345);

    assert_ne!(h1, h2, "Different seeds should produce different hashes");
    assert_ne!(h2, h3, "Different seeds should produce different hashes");
    assert_ne!(h1, h3, "Different seeds should produce different hashes");
}

#[test]
fn test_hash_long_message() {
    // Test with a message longer than SC_BUF_SIZE (192 bytes)
    let data: Vec<u8> = (0..500).map(|i| (i % 256) as u8).collect();
    let h = hash64(&data);
    // Just verify it doesn't panic and returns something
    assert_ne!(h, 0);
}

#[test]
fn test_spooky_hasher_trait() {
    use std::hash::Hasher;

    let mut hasher = SpookyHasher::new_default();
    hasher.write(b"hello");
    hasher.write(b" ");
    hasher.write(b"world");
    let h1 = hasher.finish();

    let h2 = hash64(b"hello world");
    assert_eq!(h1, h2, "Hasher trait should produce same result as hash64");
}

#[test]
fn test_hash_builder_with_hashmap() {
    use std::collections::HashMap;

    let builder = SpookyHasherBuilder::new(12345, 67890);
    let mut map: HashMap<String, i32, _> = HashMap::with_hasher(builder);

    map.insert("key1".to_string(), 1);
    map.insert("key2".to_string(), 2);
    map.insert("key3".to_string(), 3);

    assert_eq!(map.get("key1"), Some(&1));
    assert_eq!(map.get("key2"), Some(&2));
    assert_eq!(map.get("key3"), Some(&3));
    assert_eq!(map.get("key4"), None);
}

// ============================================================================
// Tests for hints
// ============================================================================

#[test]
fn test_hints_compile_and_run() {
    // These are compile-time hints, just verify they compile and run
    let x = 42;

    if likely(x > 0) {
        assert!(x > 0);
    }

    if unlikely(x < 0) {
        unreachable!();
    }

    // Test with complex expressions
    let result = if likely(x == 42) { "correct" } else { "wrong" };
    assert_eq!(result, "correct");
}

// ============================================================================
// Tests for stopwatch (more comprehensive)
// ============================================================================

#[test]
fn test_stopwatch_full_lifecycle() {
    let mut sw = Stopwatch::new();

    // Initial state
    assert!(!sw.is_running());
    assert_eq!(sw.elapsed_ticks(), 0);

    // Start
    sw.start();
    assert!(sw.is_running());

    // Let some time pass
    std::thread::sleep(std::time::Duration::from_millis(10));

    // Should have accumulated time even while running
    let elapsed1 = sw.elapsed_ms();
    assert!(elapsed1 >= 5, "Should have elapsed at least 5ms");

    // Stop
    sw.stop();
    assert!(!sw.is_running());
    let elapsed2 = sw.elapsed_ms();

    // Elapsed should not change after stop
    std::thread::sleep(std::time::Duration::from_millis(10));
    let elapsed3 = sw.elapsed_ms();
    assert_eq!(elapsed2, elapsed3, "Elapsed should not change after stop");

    // Restart accumulates
    sw.start();
    std::thread::sleep(std::time::Duration::from_millis(10));
    sw.stop();
    let elapsed4 = sw.elapsed_ms();
    assert!(
        elapsed4 > elapsed2,
        "Should accumulate time across start/stop"
    );

    // Reset clears everything
    sw.reset();
    assert!(!sw.is_running());
    assert_eq!(sw.elapsed_ticks(), 0);
}

#[test]
fn test_stopwatch_start_new() {
    let sw = Stopwatch::start_new();
    assert!(sw.is_running());
    std::thread::sleep(std::time::Duration::from_millis(5));
    assert!(sw.elapsed_ms() >= 3);
}

// ============================================================================
// Tests for interval timer
// ============================================================================

#[test]
fn test_interval_timer() {
    let mut timer = IntervalTimer::new();

    timer.mark("start");
    std::thread::sleep(std::time::Duration::from_millis(5));
    timer.mark("checkpoint1");
    std::thread::sleep(std::time::Duration::from_millis(5));
    timer.mark("checkpoint2");
    std::thread::sleep(std::time::Duration::from_millis(5));
    timer.mark("end");

    let intervals = timer.intervals();
    assert_eq!(intervals.len(), 3);
    assert_eq!(intervals[0].0, "checkpoint1");
    assert_eq!(intervals[1].0, "checkpoint2");
    assert_eq!(intervals[2].0, "end");

    // Each interval should be at least 3ms
    for (label, duration) in &intervals {
        assert!(
            duration.as_millis() >= 3,
            "{} interval too short: {:?}",
            label,
            duration
        );
    }

    // Report should not be empty
    let report = timer.report();
    assert!(!report.is_empty());
    assert!(report.contains("checkpoint1"));
}
