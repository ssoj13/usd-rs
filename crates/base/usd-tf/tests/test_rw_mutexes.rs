// Port of testenv/rwMutexes.cpp
//
// Original: tests throughput of TfBigRWMutex and TfSpinRWMutex under
// mixed read/write workload — mostly readers, occasional writers.

use std::sync::{
    Arc,
    atomic::{AtomicI32, Ordering},
};
use std::thread;
use std::time::{Duration, Instant};

use usd_tf::big_rw_mutex::BigRWMutex;
use usd_tf::spin_rw_mutex::SpinRWMutex;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Run the mixed-workload throughput test on any RW mutex type.
///
/// Each worker thread does 1024 reads then 1 write per iteration for
/// `run_secs` seconds.  Returns the final shared counter value.
fn run_throughput_spin(run_secs: f64, num_threads: usize) -> i32 {
    let mutex = Arc::new(SpinRWMutex::new());
    let value = Arc::new(AtomicI32::new(0));

    let handles: Vec<_> = (0..num_threads)
        .map(|_| {
            let mutex = Arc::clone(&mutex);
            let value = Arc::clone(&value);
            thread::spawn(move || {
                let start = Instant::now();
                loop {
                    // 1024 reads
                    for _ in 0..1024 {
                        let _guard = mutex.read();
                        let _ = value.load(Ordering::Relaxed);
                    }
                    // 1 write
                    {
                        let _guard = mutex.write();
                        value.fetch_add(1, Ordering::Relaxed);
                    }

                    if start.elapsed().as_secs_f64() >= run_secs {
                        break;
                    }
                }
            })
        })
        .collect();

    for h in handles {
        h.join().expect("worker thread panicked");
    }

    value.load(Ordering::Relaxed)
}

fn run_throughput_big(run_secs: f64, num_threads: usize) -> i32 {
    let mutex = Arc::new(BigRWMutex::new());
    let value = Arc::new(AtomicI32::new(0));

    let handles: Vec<_> = (0..num_threads)
        .map(|_| {
            let mutex = Arc::clone(&mutex);
            let value = Arc::clone(&value);
            thread::spawn(move || {
                let start = Instant::now();
                loop {
                    for _ in 0..1024 {
                        let _guard = mutex.read();
                        let _ = value.load(Ordering::Relaxed);
                    }
                    {
                        let _guard = mutex.write();
                        value.fetch_add(1, Ordering::Relaxed);
                    }

                    if start.elapsed().as_secs_f64() >= run_secs {
                        break;
                    }
                }
            })
        })
        .collect();

    for h in handles {
        h.join().expect("worker thread panicked");
    }

    value.load(Ordering::Relaxed)
}

// ---------------------------------------------------------------------------
// SpinRWMutex correctness tests  (mirrors C++ SpinRW path)
// ---------------------------------------------------------------------------

#[test]
fn spin_rw_basic_read() {
    let m = SpinRWMutex::new();
    let _g = m.read();
}

#[test]
fn spin_rw_basic_write() {
    let m = SpinRWMutex::new();
    let _g = m.write();
}

#[test]
fn spin_rw_multiple_readers_concurrent() {
    let m = SpinRWMutex::new();
    let _g1 = m.read();
    let _g2 = m.read();
    let _g3 = m.read();
    // All three read guards coexist — no deadlock.
}

#[test]
fn spin_rw_write_exclusion() {
    let m = SpinRWMutex::new();
    let _w = m.write();
    // A second try_write must fail while the first write lock is held.
    assert!(m.try_write().is_none());
}

#[test]
fn spin_rw_read_blocked_by_writer() {
    let m = SpinRWMutex::new();
    let _w = m.write();
    assert!(m.try_read().is_none());
}

#[test]
fn spin_rw_upgrade_downgrade() {
    let m = SpinRWMutex::new();
    let rg = m.read();
    let (wg, was_atomic) = rg.upgrade();
    assert!(was_atomic);
    let (_rg2, downgrade_atomic) = wg.downgrade();
    assert!(downgrade_atomic);
    // Can take a second concurrent read after downgrade.
    let _rg3 = m.read();
}

/// Throughput sanity: the counter must be positive, proving that writers
/// actually ran.  We use a very short duration (0.2s) to keep CI fast.
#[test]
fn spin_rw_throughput_sanity() {
    let num_threads = (thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(2)
        .saturating_sub(1))
    .max(1);
    let final_value = run_throughput_spin(0.2, num_threads);
    assert!(
        final_value > 0,
        "SpinRWMutex throughput test: counter should be positive, got {}",
        final_value
    );
}

// ---------------------------------------------------------------------------
// BigRWMutex correctness tests  (mirrors C++ BigRW path)
// ---------------------------------------------------------------------------

#[test]
fn big_rw_basic_read() {
    let m = BigRWMutex::new();
    let _g = m.read();
}

#[test]
fn big_rw_basic_write() {
    let m = BigRWMutex::new();
    let _g = m.write();
}

#[test]
fn big_rw_multiple_readers_concurrent() {
    let m = BigRWMutex::new();
    let _g1 = m.read();
    let _g2 = m.read();
    let _g3 = m.read();
}

#[test]
fn big_rw_write_after_read_release() {
    let m = BigRWMutex::new();
    {
        let _r = m.read();
    }
    // After the read guard is dropped, write lock must succeed.
    let _w = m.write();
}

/// Throughput sanity — same logic as SpinRW but for BigRWMutex.
#[test]
fn big_rw_throughput_sanity() {
    let num_threads = (thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(2)
        .saturating_sub(1))
    .max(1);
    let final_value = run_throughput_big(0.2, num_threads);
    assert!(
        final_value > 0,
        "BigRWMutex throughput test: counter should be positive, got {}",
        final_value
    );
}

// ---------------------------------------------------------------------------
// Cross-thread correctness: shared counter with reader + writer threads
// ---------------------------------------------------------------------------

#[test]
fn spin_rw_concurrent_read_write_counter() {
    let mutex = Arc::new(SpinRWMutex::new());
    let counter = Arc::new(AtomicI32::new(0));

    let mut handles = vec![];

    // 4 readers
    for _ in 0..4 {
        let m = Arc::clone(&mutex);
        let c = Arc::clone(&counter);
        handles.push(thread::spawn(move || {
            for _ in 0..200 {
                let _g = m.read();
                assert!(c.load(Ordering::Relaxed) >= 0);
            }
        }));
    }

    // 2 writers
    for _ in 0..2 {
        let m = Arc::clone(&mutex);
        let c = Arc::clone(&counter);
        handles.push(thread::spawn(move || {
            for _ in 0..100 {
                let _g = m.write();
                c.fetch_add(1, Ordering::Relaxed);
            }
        }));
    }

    for h in handles {
        h.join().expect("thread panicked");
    }

    assert_eq!(counter.load(Ordering::Relaxed), 200);
}

#[test]
fn big_rw_concurrent_read_write_counter() {
    let mutex = Arc::new(BigRWMutex::new());
    let counter = Arc::new(AtomicI32::new(0));

    let mut handles = vec![];

    for _ in 0..4 {
        let m = Arc::clone(&mutex);
        let c = Arc::clone(&counter);
        handles.push(thread::spawn(move || {
            for _ in 0..200 {
                let _g = m.read();
                assert!(c.load(Ordering::Relaxed) >= 0);
            }
        }));
    }

    for _ in 0..2 {
        let m = Arc::clone(&mutex);
        let c = Arc::clone(&counter);
        handles.push(thread::spawn(move || {
            for _ in 0..100 {
                let _g = m.write();
                c.fetch_add(1, Ordering::Relaxed);
            }
        }));
    }

    for h in handles {
        h.join().expect("thread panicked");
    }

    assert_eq!(counter.load(Ordering::Relaxed), 200);
}

// ---------------------------------------------------------------------------
// Duration smoke test — exercises the timeout-based loop from the C++ original
// ---------------------------------------------------------------------------

/// Mirrors the C++ loop: run for a fixed wall-clock duration; at the end,
/// the shared value must be strictly greater than zero (writes happened).
#[test]
fn spin_rw_timed_loop() {
    let mutex = Arc::new(SpinRWMutex::new());
    let value = Arc::new(AtomicI32::new(0));
    let timeout = Duration::from_millis(200);

    let handles: Vec<_> = (0..2)
        .map(|_| {
            let m = Arc::clone(&mutex);
            let v = Arc::clone(&value);
            thread::spawn(move || {
                let start = Instant::now();
                while start.elapsed() < timeout {
                    for _ in 0..64 {
                        let _g = m.read();
                        let _ = v.load(Ordering::Relaxed);
                    }
                    let _g = m.write();
                    v.fetch_add(1, Ordering::Relaxed);
                }
            })
        })
        .collect();

    for h in handles {
        h.join().expect("thread panicked");
    }

    let final_val = value.load(Ordering::Relaxed);
    assert!(
        final_val > 0,
        "timed loop must increment counter; got {}",
        final_val
    );
}
