// Port of pxr/imaging/hd/testenv/testHdPerfLog.cpp
//
// NOTE: These tests must run sequentially (not parallel) because HdPerfLog
// is a global singleton. Use `cargo test -- --test-threads=1` or run this
// file alone.

use usd_hd::perf_log::HdPerfLog;
use usd_hd::{
    hd_perf_counter_add, hd_perf_counter_decr, hd_perf_counter_incr, hd_perf_counter_set,
    hd_perf_counter_subtract,
};
use usd_sdf::Path;
use usd_tf::Token;

fn is_close(a: f64, b: f64) -> bool {
    (a - b).abs() < 0.0000001
}

#[test]
fn test_counter() {
    let perf_log = HdPerfLog::get_instance();
    let foo = Token::new("test_counter_foo");
    let bar = Token::new("test_counter_bar");

    // Disable
    perf_log.disable();

    // Disabled: expect no tracking
    perf_log.increment_counter(&foo);
    assert_eq!(perf_log.get_counter(&foo), 0.0);
    perf_log.decrement_counter(&foo);
    assert_eq!(perf_log.get_counter(&foo), 0.0);
    perf_log.add_counter(&foo, 5.0);
    assert_eq!(perf_log.get_counter(&foo), 0.0);
    perf_log.subtract_counter(&foo, 6.0);
    assert_eq!(perf_log.get_counter(&foo), 0.0);

    // Enable
    perf_log.enable();
    // Still expect zero (nothing was tracked while disabled)
    assert_eq!(perf_log.get_counter(&foo), 0.0);

    // Incr, Decr, Set
    perf_log.increment_counter(&foo);
    assert_eq!(perf_log.get_counter(&foo), 1.0);
    perf_log.decrement_counter(&foo);
    assert_eq!(perf_log.get_counter(&foo), 0.0);
    perf_log.set_counter(&foo, 42.0);
    assert_eq!(perf_log.get_counter(&foo), 42.0);
    perf_log.add_counter(&foo, 5.0);
    assert_eq!(perf_log.get_counter(&foo), 47.0);
    perf_log.subtract_counter(&foo, 6.0);
    assert_eq!(perf_log.get_counter(&foo), 41.0);

    // Float counter
    perf_log.set_counter(&bar, 0.1);
    assert!(is_close(perf_log.get_counter(&bar), 0.1));
    perf_log.increment_counter(&bar);
    assert!(is_close(perf_log.get_counter(&bar), 1.1));
    perf_log.decrement_counter(&bar);
    assert!(is_close(perf_log.get_counter(&bar), 0.1));

    // Reset for macro tests
    perf_log.set_counter(&foo, 0.0);
    perf_log.set_counter(&bar, 0.0);

    // Macros
    hd_perf_counter_decr!(&foo);
    assert_eq!(perf_log.get_counter(&foo), -1.0);
    hd_perf_counter_incr!(&foo);
    assert_eq!(perf_log.get_counter(&foo), 0.0);
    hd_perf_counter_set!(&foo, 42);
    assert_eq!(perf_log.get_counter(&foo), 42.0);
    hd_perf_counter_decr!(&foo);
    assert_eq!(perf_log.get_counter(&foo), 41.0);
    hd_perf_counter_incr!(&foo);
    assert_eq!(perf_log.get_counter(&foo), 42.0);
    hd_perf_counter_add!(&foo, 5);
    assert_eq!(perf_log.get_counter(&foo), 47.0);
    hd_perf_counter_subtract!(&foo, 6);
    assert_eq!(perf_log.get_counter(&foo), 41.0);

    hd_perf_counter_set!(&bar, 0.1);
    assert!(is_close(perf_log.get_counter(&bar), 0.1));
    hd_perf_counter_decr!(&bar);
    assert!(is_close(perf_log.get_counter(&bar), -0.9));
    hd_perf_counter_incr!(&bar);
    assert!(is_close(perf_log.get_counter(&bar), 0.1));

    // Disable: reads still work, mutations are no-ops
    perf_log.disable();
    assert_eq!(perf_log.get_counter(&foo), 41.0);
    perf_log.increment_counter(&foo);
    assert_eq!(perf_log.get_counter(&foo), 41.0);
    perf_log.decrement_counter(&foo);
    assert_eq!(perf_log.get_counter(&foo), 41.0);
    perf_log.set_counter(&foo, 0.0);
    assert_eq!(perf_log.get_counter(&foo), 41.0);
    perf_log.add_counter(&foo, 5.0);
    assert_eq!(perf_log.get_counter(&foo), 41.0);
    perf_log.subtract_counter(&foo, 6.0);
    assert_eq!(perf_log.get_counter(&foo), 41.0);
}

#[test]
fn test_cache() {
    let perf_log = HdPerfLog::get_instance();
    let foo = Token::new("test_cache_foo");
    let bar = Token::new("test_cache_bar");
    let id = Path::from("/Some/Path");

    // Disable
    perf_log.disable();

    // Disabled: expect no tracking
    assert_eq!(perf_log.get_cache_hits(&foo), 0);
    assert_eq!(perf_log.get_cache_misses(&foo), 0);
    assert_eq!(perf_log.get_cache_hit_ratio(&foo), 0.0);
    assert_eq!(perf_log.get_cache_hits(&bar), 0);
    assert_eq!(perf_log.get_cache_misses(&bar), 0);
    assert_eq!(perf_log.get_cache_hit_ratio(&bar), 0.0);

    // Enable
    perf_log.enable();

    // Still zero
    assert_eq!(perf_log.get_cache_hits(&foo), 0);
    assert_eq!(perf_log.get_cache_misses(&foo), 0);
    assert_eq!(perf_log.get_cache_hit_ratio(&foo), 0.0);

    perf_log.add_cache_hit(&foo, &id);
    perf_log.add_cache_hit(&foo, &id);
    perf_log.add_cache_miss(&foo, &id);
    perf_log.add_cache_miss(&foo, &id);
    assert_eq!(perf_log.get_cache_hits(&foo), 2);
    assert_eq!(perf_log.get_cache_misses(&foo), 2);
    assert!(is_close(perf_log.get_cache_hit_ratio(&foo), 0.5));

    assert_eq!(perf_log.get_cache_hits(&bar), 0);
    assert_eq!(perf_log.get_cache_misses(&bar), 0);
    assert!(is_close(perf_log.get_cache_hit_ratio(&bar), 0.0));
    perf_log.add_cache_hit(&bar, &id);
    perf_log.add_cache_hit(&bar, &id);
    perf_log.add_cache_hit(&bar, &id);
    perf_log.add_cache_miss(&bar, &id);
    assert_eq!(perf_log.get_cache_hits(&bar), 3);
    assert_eq!(perf_log.get_cache_misses(&bar), 1);
    assert!(is_close(perf_log.get_cache_hit_ratio(&bar), 0.75));

    // Cache names should be sorted
    let names = perf_log.get_cache_names();
    // Our test-specific cache names should be present
    assert!(names.contains(&bar));
    assert!(names.contains(&foo));

    // Disable: reads still work
    perf_log.disable();
    assert_eq!(perf_log.get_cache_hits(&foo), 2);
    assert_eq!(perf_log.get_cache_misses(&foo), 2);
    assert!(is_close(perf_log.get_cache_hit_ratio(&foo), 0.5));
    assert_eq!(perf_log.get_cache_hits(&bar), 3);
    assert_eq!(perf_log.get_cache_misses(&bar), 1);
    assert!(is_close(perf_log.get_cache_hit_ratio(&bar), 0.75));
}
