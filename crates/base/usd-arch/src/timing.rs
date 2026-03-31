//! High-resolution timing utilities.
//!
//! Provides cross-platform access to high-resolution timers and performance counters.

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// A timestamp in nanoseconds since an arbitrary point in time.
///
/// This is useful for measuring elapsed time with high precision.
pub type Ticks = u64;

/// Returns the current high-resolution tick count.
///
/// The returned value is suitable for measuring elapsed time, but the
/// absolute value has no particular meaning.
///
/// # Examples
///
/// ```
/// use usd_arch::{get_ticks, ticks_to_nanoseconds};
///
/// let start = get_ticks();
/// // ... do some work ...
/// let end = get_ticks();
/// let elapsed_ns = ticks_to_nanoseconds(end - start);
/// ```
#[must_use]
pub fn get_ticks() -> Ticks {
    // Use Instant as the basis for high-resolution timing
    // We measure from a fixed point (the first call to this function)
    static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    let start = START.get_or_init(Instant::now);
    let elapsed = start.elapsed();
    elapsed.as_nanos() as Ticks
}

/// Fenced "start" tick read for interval measurement.
///
/// Uses compiler-only fence (`atomic_signal_fence` equivalent) to prevent
/// compiler reordering without emitting a CPU memory barrier instruction.
/// C++ parity: `ArchGetStartTickTime()`.
#[inline]
#[must_use]
pub fn get_start_tick_time() -> Ticks {
    std::sync::atomic::compiler_fence(std::sync::atomic::Ordering::SeqCst);
    let t = get_ticks();
    std::sync::atomic::compiler_fence(std::sync::atomic::Ordering::SeqCst);
    t
}

/// Fenced "stop" tick read for interval measurement.
///
/// Uses compiler-only fence (`atomic_signal_fence` equivalent).
/// C++ parity: `ArchGetStopTickTime()`.
#[inline]
#[must_use]
pub fn get_stop_tick_time() -> Ticks {
    std::sync::atomic::compiler_fence(std::sync::atomic::Ordering::SeqCst);
    let t = get_ticks();
    std::sync::atomic::compiler_fence(std::sync::atomic::Ordering::SeqCst);
    t
}

/// Runs `f` repeatedly, attempting to reach a consensus on its fastest
/// execution time. Returns the consensus (or best-estimate) ticks.
///
/// Algorithm mirrors C++ `Arch_MeasureExecutionTime`:
/// 1. Warmup: run f 10 times to estimate ticks-per-call.
/// 2. Compute `sample_iters` so quantum noise < 0.1% of sample time.
/// 3. Collect 64 samples (each averaged over `sample_iters` calls).
/// 4. Sort; consensus = minimum equals median (samples[0] == samples[32]).
/// 5. If no consensus: replace slowest 1/3 and fastest 1/10 with new samples.
/// 6. Repeat until consensus or time budget exhausted.
///
/// C++ parity: `ArchMeasureExecutionTime<Fn>(fn, maxTicks, &reachedConsensus)`.
pub fn measure_execution_time<F: Fn()>(
    f: &F,
    max_ticks: Ticks,
    reached_consensus: Option<&mut bool>,
) -> Ticks {
    use std::sync::atomic::{Ordering, compiler_fence};

    // measure_n: run f nTimes, return total elapsed ticks (like C++ measureN)
    let measure_n = |n_times: u64| -> Ticks {
        let start = get_start_tick_time();
        for _ in 0..n_times {
            compiler_fence(Ordering::SeqCst);
            f();
            compiler_fence(Ordering::SeqCst);
        }
        get_stop_tick_time().saturating_sub(start)
    };

    // Step 1: warmup — 10 single runs to estimate ticks-per-call
    let mut est_ticks_per = Ticks::MAX;
    for _ in 0..10 {
        est_ticks_per = est_ticks_per.min(measure_n(1));
    }

    // Step 2: how many iterations per sample so quantum noise < 0.1%
    // C++: minTicksPerSample = 2000 * quantum; sampleIters = ceil(min/est)
    let quantum = get_tick_quantum();
    let min_ticks_per_sample = 2000 * quantum;
    let sample_iters: u64 = if est_ticks_per < min_ticks_per_sample {
        // rounded division: (min + est/2) / est
        (min_ticks_per_sample + est_ticks_per / 2) / est_ticks_per.max(1)
    } else {
        1
    };

    // measure_sample: run sample_iters calls, return per-call ticks (rounded)
    let measure_sample = || -> Ticks {
        let total = measure_n(sample_iters);
        // rounded integer division matching C++: (total + iters/2) / iters
        (total + sample_iters / 2) / sample_iters
    };

    // Step 3: fill 64-sample buffer
    const NUM_SAMPLES: usize = 64;
    let mut samples = [0u64; NUM_SAMPLES];
    for s in &mut samples {
        *s = measure_sample();
    }

    // Cap budget at 5 billion ticks, then start the limit timer
    let max_ticks = max_ticks.min(5_000_000_000);
    let limit_start = get_start_tick_time();

    // Step 4-6: sort, check consensus, resample
    let mut best_median = Ticks::MAX;
    loop {
        samples.sort_unstable();

        // Consensus: minimum == median (index 32 out of 64)
        if samples[0] == samples[NUM_SAMPLES / 2] {
            if let Some(rc) = reached_consensus {
                *rc = true;
            }
            return samples[0];
        }

        // Check time budget
        if get_stop_tick_time().saturating_sub(limit_start) >= max_ticks {
            break;
        }

        // Track best median seen so far
        best_median = best_median.min(samples[NUM_SAMPLES / 2]);

        // Replace slowest 1/3 (top end, after sort)
        let slow_start = NUM_SAMPLES - NUM_SAMPLES / 3;
        for i in slow_start..NUM_SAMPLES {
            samples[i] = measure_sample();
        }
        // Replace fastest 1/10 (bottom end — may be outlier artifacts)
        for i in 0..(NUM_SAMPLES / 10) {
            samples[i] = measure_sample();
        }
    }

    // No consensus reached — return best median we observed
    if let Some(rc) = reached_consensus {
        *rc = false;
    }
    best_median
}

/// Converts tick counts to nanoseconds.
///
/// # Examples
///
/// ```
/// use usd_arch::{get_ticks, ticks_to_nanoseconds};
///
/// let ticks = get_ticks();
/// let ns = ticks_to_nanoseconds(ticks);
/// ```
#[inline]
#[must_use]
pub const fn ticks_to_nanoseconds(ticks: Ticks) -> u64 {
    // Our ticks are already in nanoseconds
    ticks
}

/// Converts tick counts to seconds.
///
/// # Examples
///
/// ```
/// use usd_arch::{get_ticks, ticks_to_seconds};
///
/// let ticks = get_ticks();
/// let secs = ticks_to_seconds(ticks);
/// ```
#[inline]
#[must_use]
pub fn ticks_to_seconds(ticks: Ticks) -> f64 {
    ticks as f64 / 1_000_000_000.0
}

/// Converts nanoseconds to ticks.
#[inline]
#[must_use]
pub const fn nanoseconds_to_ticks(ns: u64) -> Ticks {
    ns
}

/// Converts seconds to ticks.
#[inline]
#[must_use]
pub fn seconds_to_ticks(secs: f64) -> Ticks {
    (secs * 1_000_000_000.0) as Ticks
}

/// Returns the tick frequency (ticks per second).
///
/// Since our ticks are in nanoseconds, this returns 1 billion.
#[inline]
#[must_use]
pub const fn get_ticks_per_second() -> u64 {
    1_000_000_000
}

/// Returns an estimate of the overhead of calling `get_ticks()`.
///
/// This can be useful for accurately measuring very short operations.
#[must_use]
pub fn get_ticks_overhead() -> Ticks {
    // Measure the overhead by timing empty intervals
    const SAMPLES: usize = 1000;
    let mut min_overhead = Ticks::MAX;

    for _ in 0..SAMPLES {
        let start = get_ticks();
        let end = get_ticks();
        let overhead = end.saturating_sub(start);
        min_overhead = min_overhead.min(overhead);
    }

    min_overhead
}

/// Returns the current time as seconds since the Unix epoch.
///
/// This is useful for wall-clock time, but not for measuring elapsed time
/// (use `get_ticks()` for that).
#[must_use]
pub fn get_time() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

/// Returns the current time in nanoseconds since the Unix epoch.
#[must_use]
pub fn get_time_nanos() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

/// A stopwatch for measuring elapsed time.
///
/// # Examples
///
/// ```
/// use usd_arch::Stopwatch;
///
/// let mut sw = Stopwatch::new();
/// sw.start();
/// // ... do some work ...
/// sw.stop();
/// println!("Elapsed: {} ms", sw.elapsed_ms());
/// ```
#[derive(Debug, Clone)]
pub struct Stopwatch {
    start_ticks: Option<Ticks>,
    accumulated_ticks: Ticks,
    running: bool,
}

impl Stopwatch {
    /// Creates a new stopped stopwatch.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            start_ticks: None,
            accumulated_ticks: 0,
            running: false,
        }
    }

    /// Creates a new stopwatch and immediately starts it.
    #[must_use]
    pub fn start_new() -> Self {
        let mut sw = Self::new();
        sw.start();
        sw
    }

    /// Starts or resumes the stopwatch.
    pub fn start(&mut self) {
        if !self.running {
            self.start_ticks = Some(get_ticks());
            self.running = true;
        }
    }

    /// Stops the stopwatch.
    pub fn stop(&mut self) {
        if self.running {
            if let Some(start) = self.start_ticks {
                self.accumulated_ticks += get_ticks().saturating_sub(start);
            }
            self.running = false;
            self.start_ticks = None;
        }
    }

    /// Resets the stopwatch to zero and stops it.
    pub fn reset(&mut self) {
        self.start_ticks = None;
        self.accumulated_ticks = 0;
        self.running = false;
    }

    /// Resets and immediately starts the stopwatch.
    pub fn restart(&mut self) {
        self.reset();
        self.start();
    }

    /// Returns true if the stopwatch is currently running.
    #[must_use]
    pub const fn is_running(&self) -> bool {
        self.running
    }

    /// Returns the elapsed time in ticks.
    #[must_use]
    pub fn elapsed_ticks(&self) -> Ticks {
        let current = if self.running {
            self.start_ticks
                .map(|start| get_ticks().saturating_sub(start))
                .unwrap_or(0)
        } else {
            0
        };
        self.accumulated_ticks + current
    }

    /// Returns the elapsed time in nanoseconds.
    #[must_use]
    pub fn elapsed_nanos(&self) -> u64 {
        ticks_to_nanoseconds(self.elapsed_ticks())
    }

    /// Returns the elapsed time in microseconds.
    #[must_use]
    pub fn elapsed_us(&self) -> u64 {
        self.elapsed_nanos() / 1_000
    }

    /// Returns the elapsed time in milliseconds.
    #[must_use]
    pub fn elapsed_ms(&self) -> u64 {
        self.elapsed_nanos() / 1_000_000
    }

    /// Returns the elapsed time in seconds.
    #[must_use]
    pub fn elapsed_secs(&self) -> f64 {
        ticks_to_seconds(self.elapsed_ticks())
    }

    /// Returns the elapsed time as a Duration.
    #[must_use]
    pub fn elapsed(&self) -> Duration {
        Duration::from_nanos(self.elapsed_nanos())
    }
}

impl Default for Stopwatch {
    fn default() -> Self {
        Self::new()
    }
}

/// A timer that measures intervals between marks.
///
/// Useful for profiling code by recording timestamps at various points.
#[derive(Debug, Clone)]
pub struct IntervalTimer {
    marks: Vec<(String, Ticks)>,
}

impl IntervalTimer {
    /// Creates a new interval timer.
    #[must_use]
    pub fn new() -> Self {
        Self { marks: Vec::new() }
    }

    /// Adds a mark with the given label.
    pub fn mark(&mut self, label: impl Into<String>) {
        self.marks.push((label.into(), get_ticks()));
    }

    /// Resets the timer, clearing all marks.
    pub fn reset(&mut self) {
        self.marks.clear();
    }

    /// Returns the intervals between consecutive marks.
    #[must_use]
    pub fn intervals(&self) -> Vec<(&str, Duration)> {
        self.marks
            .windows(2)
            .map(|w| {
                let duration = Duration::from_nanos(ticks_to_nanoseconds(w[1].1 - w[0].1));
                (w[1].0.as_str(), duration)
            })
            .collect()
    }

    /// Returns a formatted report of all intervals.
    #[must_use]
    pub fn report(&self) -> String {
        let mut result = String::new();
        for (label, duration) in self.intervals() {
            result.push_str(&format!("{}: {:?}\n", label, duration));
        }
        result
    }
}

impl Default for IntervalTimer {
    fn default() -> Self {
        Self::new()
    }
}

/// Sleeps for the specified duration in nanoseconds.
pub fn sleep_nanos(ns: u64) {
    std::thread::sleep(Duration::from_nanos(ns));
}

/// Sleeps for the specified duration in milliseconds.
pub fn sleep_ms(ms: u64) {
    std::thread::sleep(Duration::from_millis(ms));
}

/// Sleeps for the specified duration in seconds.
pub fn sleep_secs(secs: f64) {
    std::thread::sleep(Duration::from_secs_f64(secs));
}

/// Returns the tick counter quantum — the smallest measurable tick difference.
///
/// Mirrors `ArchGetTickQuantum()`: runs 64 trials, each reading 5 consecutive
/// ticks, computes 4 successive differences, and returns the global minimum
/// non-zero delta. Result is cached in a `OnceLock` (computed once per process).
///
/// Note that an interval measurement can be off by +/- one quantum.
///
/// # Examples
///
/// ```
/// use usd_arch::get_tick_quantum;
///
/// let quantum = get_tick_quantum();
/// println!("Timer resolution: {} ns", quantum);
/// ```
#[must_use]
pub fn get_tick_quantum() -> Ticks {
    static QUANTUM: std::sync::OnceLock<Ticks> = std::sync::OnceLock::new();
    *QUANTUM.get_or_init(compute_tick_quantum)
}

/// Computes the tick quantum by taking 64 trials of 5 consecutive reads.
/// Mirrors `Arch_ComputeTickQuantum()` from timing.cpp.
fn compute_tick_quantum() -> Ticks {
    const NUM_TRIALS: usize = 64;
    let mut curr_min = Ticks::MAX;

    for _ in 0..NUM_TRIALS {
        // Read 5 consecutive ticks
        let times = [
            get_ticks(),
            get_ticks(),
            get_ticks(),
            get_ticks(),
            get_ticks(),
        ];
        // Compute 4 successive differences and find the minimum
        for i in 0..4 {
            let delta = times[i + 1].saturating_sub(times[i]);
            if delta > 0 && delta < curr_min {
                curr_min = delta;
            }
        }
    }

    if curr_min == Ticks::MAX { 1 } else { curr_min }
}

/// Returns the overhead of creating and using an interval timer.
///
/// This measures the time cost of the timer itself, which is useful for
/// understanding measurement accuracy when timing very short operations.
///
/// # Examples
///
/// ```
/// use usd_arch::get_interval_timer_overhead;
///
/// let overhead = get_interval_timer_overhead();
/// println!("Timer overhead: {} ns", overhead);
/// ```
#[must_use]
pub fn get_interval_timer_overhead() -> Ticks {
    // Measure the cost of starting and stopping a timer
    const SAMPLES: usize = 100;
    let mut min_overhead = Ticks::MAX;

    for _ in 0..SAMPLES {
        let start = get_ticks();
        let mut timer = IntervalTimer::new();
        timer.mark("start");
        timer.mark("end");
        let _ = timer.intervals();
        let end = get_ticks();

        let overhead = end.saturating_sub(start);
        if overhead < min_overhead {
            min_overhead = overhead;
        }
    }

    min_overhead
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_ticks() {
        let t1 = get_ticks();
        std::thread::sleep(Duration::from_millis(1));
        let t2 = get_ticks();
        assert!(t2 > t1);
    }

    #[test]
    fn test_ticks_conversion() {
        let ns = 1_000_000_000u64; // 1 second in nanoseconds
        let ticks = nanoseconds_to_ticks(ns);
        assert_eq!(ticks_to_nanoseconds(ticks), ns);

        let secs = ticks_to_seconds(ticks);
        assert!((secs - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_get_ticks_per_second() {
        assert_eq!(get_ticks_per_second(), 1_000_000_000);
    }

    #[test]
    fn test_stopwatch() {
        let mut sw = Stopwatch::new();
        assert!(!sw.is_running());

        sw.start();
        assert!(sw.is_running());

        std::thread::sleep(Duration::from_millis(10));
        sw.stop();

        assert!(!sw.is_running());
        assert!(sw.elapsed_ms() >= 5); // Allow some tolerance
    }

    #[test]
    fn test_stopwatch_accumulation() {
        let mut sw = Stopwatch::new();

        sw.start();
        std::thread::sleep(Duration::from_millis(5));
        sw.stop();
        let first = sw.elapsed_ms();

        sw.start();
        std::thread::sleep(Duration::from_millis(5));
        sw.stop();
        let second = sw.elapsed_ms();

        assert!(second >= first);
    }

    #[test]
    fn test_stopwatch_restart() {
        let mut sw = Stopwatch::start_new();
        std::thread::sleep(Duration::from_millis(10));
        sw.restart();
        assert!(sw.elapsed_ms() < 5);
    }

    #[test]
    fn test_interval_timer() {
        let mut timer = IntervalTimer::new();
        timer.mark("start");
        std::thread::sleep(Duration::from_millis(5));
        timer.mark("middle");
        std::thread::sleep(Duration::from_millis(5));
        timer.mark("end");

        let intervals = timer.intervals();
        assert_eq!(intervals.len(), 2);
        assert_eq!(intervals[0].0, "middle");
        assert_eq!(intervals[1].0, "end");
    }

    #[test]
    fn test_get_time() {
        let t1 = get_time();
        std::thread::sleep(Duration::from_millis(1));
        let t2 = get_time();
        assert!(t2 > t1);
    }

    #[test]
    fn test_get_tick_quantum() {
        let quantum = get_tick_quantum();
        assert!(quantum > 0);
        assert!(quantum < 1_000_000_000); // Should be less than 1 second
        println!("Tick quantum: {} ns", quantum);
    }

    #[test]
    fn test_get_interval_timer_overhead() {
        let overhead = get_interval_timer_overhead();
        assert!(overhead > 0);
        // Overhead should be reasonable (less than 1ms)
        assert!(overhead < 1_000_000);
        println!("Interval timer overhead: {} ns", overhead);
    }
}
