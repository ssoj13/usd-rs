//! Stopwatch for high-resolution timing.
//!
//! This module provides a low-cost, high-resolution timer for performance
//! measurements. It wraps the architecture-specific timing functions and
//! adds sample counting for computing averages.
//!
//! # Examples
//!
//! ```
//! use usd_tf::stopwatch::Stopwatch;
//!
//! let mut sw = Stopwatch::new();
//!
//! // Time some operations
//! for _ in 0..100 {
//!     sw.start();
//!     // ... do work ...
//!     sw.stop();
//! }
//!
//! println!("Total time: {} ms", sw.milliseconds());
//! println!("Samples: {}", sw.sample_count());
//! println!("Average: {} us/sample", sw.microseconds() / sw.sample_count() as i64);
//! ```
//!
//! # Thread Safety
//!
//! `Stopwatch` is NOT thread-safe. If you need timing in multi-threaded code,
//! give each thread its own stopwatch and combine results using `add_from()`.

use std::fmt;
use usd_arch::timing;

/// High-resolution stopwatch for performance timing.
///
/// A `Stopwatch` accumulates elapsed time across multiple start/stop cycles
/// and counts the number of samples taken. This is useful for measuring
/// average execution time of repeated operations.
///
/// # Examples
///
/// ```
/// use usd_tf::stopwatch::Stopwatch;
///
/// let mut sw = Stopwatch::new();
///
/// sw.start();
/// // ... perform operation ...
/// sw.stop();
///
/// println!("Elapsed: {} ns", sw.nanoseconds());
/// ```
#[derive(Debug, Clone)]
pub struct Stopwatch {
    /// Accumulated ticks.
    ticks: u64,
    /// Start tick for current measurement.
    start_tick: u64,
    /// Number of samples (stop() calls).
    sample_count: usize,
}

impl Stopwatch {
    /// Create a new stopwatch.
    ///
    /// The stopwatch starts in the stopped state with zero accumulated time.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::stopwatch::Stopwatch;
    ///
    /// let sw = Stopwatch::new();
    /// assert_eq!(sw.sample_count(), 0);
    /// assert_eq!(sw.nanoseconds(), 0);
    /// ```
    #[must_use]
    pub const fn new() -> Self {
        Self {
            ticks: 0,
            start_tick: 0,
            sample_count: 0,
        }
    }

    /// Record the current time for use by the next `stop()` call.
    ///
    /// A subsequent call to `start()` before a call to `stop()` simply
    /// records a later current time, but does not change the accumulated
    /// time of the stopwatch.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::stopwatch::Stopwatch;
    ///
    /// let mut sw = Stopwatch::new();
    /// sw.start();
    /// // ... do work ...
    /// sw.stop();
    /// ```
    #[inline]
    pub fn start(&mut self) {
        self.start_tick = timing::get_ticks();
    }

    /// Increase the accumulated time stored in the stopwatch.
    ///
    /// The `stop()` function increases the accumulated time by the duration
    /// between the current time and the last time recorded by `start()`.
    /// A subsequent call to `stop()` before another call to `start()` will
    /// therefore double-count time and throw off the results.
    ///
    /// The stopwatch also counts the number of samples it has taken. The
    /// "sample count" is simply the number of times that `stop()` has been
    /// called.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::stopwatch::Stopwatch;
    ///
    /// let mut sw = Stopwatch::new();
    /// sw.start();
    /// sw.stop();
    /// assert_eq!(sw.sample_count(), 1);
    /// ```
    #[inline]
    pub fn stop(&mut self) {
        let end_tick = timing::get_ticks();
        self.ticks += end_tick.saturating_sub(self.start_tick);
        self.sample_count += 1;
    }

    /// Reset the accumulated time and sample count to zero.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::stopwatch::Stopwatch;
    ///
    /// let mut sw = Stopwatch::new();
    /// sw.start();
    /// sw.stop();
    /// assert!(sw.sample_count() > 0);
    ///
    /// sw.reset();
    /// assert_eq!(sw.sample_count(), 0);
    /// assert_eq!(sw.nanoseconds(), 0);
    /// ```
    pub fn reset(&mut self) {
        self.ticks = 0;
        self.start_tick = 0;
        self.sample_count = 0;
    }

    /// Add the accumulated time and sample count from another stopwatch.
    ///
    /// If you have several timers taking measurements, and you wish to
    /// combine them together, you can add one timer's results into another.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::stopwatch::Stopwatch;
    ///
    /// let mut sw1 = Stopwatch::new();
    /// sw1.start();
    /// sw1.stop();
    ///
    /// let mut sw2 = Stopwatch::new();
    /// sw2.start();
    /// sw2.stop();
    ///
    /// sw1.add_from(&sw2);
    /// assert_eq!(sw1.sample_count(), 2);
    /// ```
    pub fn add_from(&mut self, other: &Stopwatch) {
        self.ticks += other.ticks;
        self.sample_count += other.sample_count;
    }

    /// Return the accumulated time in nanoseconds.
    ///
    /// Note that this number can easily overflow a 32-bit counter, so take
    /// care to use `i64` for the result.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::stopwatch::Stopwatch;
    ///
    /// let mut sw = Stopwatch::new();
    /// sw.start();
    /// sw.stop();
    ///
    /// let ns = sw.nanoseconds();
    /// assert!(ns >= 0);
    /// ```
    #[must_use]
    pub fn nanoseconds(&self) -> i64 {
        timing::ticks_to_nanoseconds(self.ticks) as i64
    }

    /// Return the accumulated time in microseconds.
    ///
    /// Note that 45 minutes will overflow a 32-bit counter, so take care
    /// to use `i64` for the result.
    #[must_use]
    pub fn microseconds(&self) -> i64 {
        self.nanoseconds() / 1000
    }

    /// Return the accumulated time in milliseconds.
    #[must_use]
    pub fn milliseconds(&self) -> i64 {
        self.microseconds() / 1000
    }

    /// Return the accumulated time in seconds as a `f64`.
    #[must_use]
    pub fn seconds(&self) -> f64 {
        timing::ticks_to_seconds(self.ticks)
    }

    /// Return the current sample count.
    ///
    /// The sample count, which is simply the number of calls to `stop()`
    /// since creation or a call to `reset()`, is useful for computing
    /// average running times of a repeated task.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::stopwatch::Stopwatch;
    ///
    /// let mut sw = Stopwatch::new();
    /// for _ in 0..10 {
    ///     sw.start();
    ///     sw.stop();
    /// }
    /// assert_eq!(sw.sample_count(), 10);
    /// ```
    #[must_use]
    pub const fn sample_count(&self) -> usize {
        self.sample_count
    }

    /// Return the accumulated ticks (raw timer units).
    #[must_use]
    pub const fn ticks(&self) -> u64 {
        self.ticks
    }
}

impl Default for Stopwatch {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for Stopwatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.6} seconds", self.seconds())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_new() {
        let sw = Stopwatch::new();
        assert_eq!(sw.sample_count(), 0);
        assert_eq!(sw.nanoseconds(), 0);
        assert_eq!(sw.ticks(), 0);
    }

    #[test]
    fn test_default() {
        let sw: Stopwatch = Default::default();
        assert_eq!(sw.sample_count(), 0);
    }

    #[test]
    fn test_start_stop() {
        let mut sw = Stopwatch::new();
        sw.start();
        thread::sleep(Duration::from_millis(10));
        sw.stop();

        assert_eq!(sw.sample_count(), 1);
        // Should have measured at least some time
        assert!(sw.nanoseconds() > 0);
        assert!(sw.microseconds() > 0);
    }

    #[test]
    fn test_multiple_samples() {
        let mut sw = Stopwatch::new();

        for _ in 0..5 {
            sw.start();
            sw.stop();
        }

        assert_eq!(sw.sample_count(), 5);
    }

    #[test]
    fn test_reset() {
        let mut sw = Stopwatch::new();
        sw.start();
        thread::sleep(Duration::from_millis(1));
        sw.stop();

        assert!(sw.nanoseconds() > 0);
        assert_eq!(sw.sample_count(), 1);

        sw.reset();

        assert_eq!(sw.nanoseconds(), 0);
        assert_eq!(sw.sample_count(), 0);
    }

    #[test]
    fn test_add_from() {
        let mut sw1 = Stopwatch::new();
        sw1.start();
        thread::sleep(Duration::from_millis(5));
        sw1.stop();

        let mut sw2 = Stopwatch::new();
        sw2.start();
        thread::sleep(Duration::from_millis(5));
        sw2.stop();

        let combined_samples = sw1.sample_count() + sw2.sample_count();
        sw1.add_from(&sw2);

        assert_eq!(sw1.sample_count(), combined_samples);
    }

    #[test]
    fn test_time_units() {
        let mut sw = Stopwatch::new();
        sw.start();
        thread::sleep(Duration::from_millis(50));
        sw.stop();

        // Nanoseconds should be larger than microseconds
        assert!(sw.nanoseconds() > sw.microseconds());
        // Microseconds should be larger than milliseconds
        assert!(sw.microseconds() > sw.milliseconds());
        // Should have measured at least ~50ms
        assert!(sw.milliseconds() >= 40); // Allow some slack
    }

    #[test]
    fn test_seconds() {
        let mut sw = Stopwatch::new();
        sw.start();
        thread::sleep(Duration::from_millis(100));
        sw.stop();

        let seconds = sw.seconds();
        assert!(seconds >= 0.08); // Allow some slack
        assert!(seconds < 0.5); // Shouldn't be too long
    }

    #[test]
    fn test_display() {
        let sw = Stopwatch::new();
        let s = format!("{}", sw);
        assert!(s.contains("0.0"));
    }

    #[test]
    fn test_accumulation() {
        let mut sw = Stopwatch::new();

        sw.start();
        thread::sleep(Duration::from_millis(10));
        sw.stop();

        let first_time = sw.nanoseconds();

        sw.start();
        thread::sleep(Duration::from_millis(10));
        sw.stop();

        // Should have accumulated more time
        assert!(sw.nanoseconds() > first_time);
        assert_eq!(sw.sample_count(), 2);
    }
}
