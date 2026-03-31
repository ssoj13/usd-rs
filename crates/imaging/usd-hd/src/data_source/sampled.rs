//! Sampled data source - time-sampled values with motion blur.

use super::base::HdDataSourceBase;
use std::sync::Arc;
use usd_vt::Value;

/// Time type for motion blur sampling.
///
/// Represents frame-relative time offsets for motion blur. Typically:
/// - 0.0 = current frame
/// - Negative values = before current frame
/// - Positive values = after current frame
pub type HdSampledDataSourceTime = f32;

/// A data source representing time-sampled values.
///
/// Sampled data sources provide values that can vary over time, supporting
/// motion blur in rendering. The scene index producing this data source is
/// responsible for tracking the current frame context.
///
/// # Motion Blur
///
/// Motion blur is handled by querying sample times within a shutter window
/// and then evaluating the data source at each sample time. The renderer
/// interpolates between samples as needed.
///
/// # Thread Safety
///
/// All methods must be thread-safe.
///
/// # Examples
///
/// ```
/// use usd_hd::data_source::*;
///
/// // Query value at current frame (time = 0.0)
/// // let sampled: Arc<dyn HdSampledDataSource> = ...;
/// // let value = sampled.get_value(0.0);
///
/// // Query sample times for motion blur
/// // let mut times = Vec::new();
/// // if sampled.get_contributing_sample_times(-0.25, 0.25, &mut times) {
/// //     // Evaluate at each sample time
/// //     for time in times {
/// //         let sample = sampled.get_value(time);
/// //     }
/// // }
/// ```
pub trait HdSampledDataSource: HdDataSourceBase {
    /// Returns the value at the given frame-relative time.
    ///
    /// The `shutter_offset` is relative to the current frame. The scene index
    /// is responsible for frame context. The type of the returned value should
    /// be consistent across all shutter offsets.
    ///
    /// # Arguments
    ///
    /// * `shutter_offset` - Time relative to current frame (0.0 = current frame)
    fn get_value(&self, shutter_offset: HdSampledDataSourceTime) -> Value;

    /// Returns sample times for a shutter window.
    ///
    /// Given a shutter window (`start_time` to `end_time`), returns a list of
    /// sample times that should be queried to reconstruct the signal over the
    /// window. For sample-based attributes, this might be times where samples
    /// are defined. For procedural data, this might be a generated distribution.
    ///
    /// # Returns
    ///
    /// - `true` - Value varies over time. `out_sample_times` is populated with
    ///   times to query via `get_value()`.
    /// - `false` - Value is uniform over the window. Call `get_value(0.0)` for
    ///   the constant value.
    ///
    /// # Notes
    ///
    /// Returned sample times don't need to be strictly within the window.
    /// Boundary samples outside the window may be returned for interpolation.
    ///
    /// # Arguments
    ///
    /// * `start_time` - Start of shutter window (frame-relative)
    /// * `end_time` - End of shutter window (frame-relative)
    /// * `out_sample_times` - Output vector for sample times
    fn get_contributing_sample_times(
        &self,
        start_time: HdSampledDataSourceTime,
        end_time: HdSampledDataSourceTime,
        out_sample_times: &mut Vec<HdSampledDataSourceTime>,
    ) -> bool;
}

/// Handle to a sampled data source.
pub type HdSampledDataSourceHandle = Arc<dyn HdSampledDataSource>;

/// Merges contributing sample times from multiple data sources.
///
/// This utility function takes sample times from multiple sampled data sources
/// and merges them into a single sorted list of unique times. Useful when
/// multiple attributes need to be sampled together for motion blur.
///
/// # Returns
///
/// - `true` if any input has varying samples
/// - `false` if all inputs are uniform
///
/// # Examples
///
/// ```
/// use usd_hd::data_source::*;
///
/// // Merge sample times from position and normal attributes
/// // let sources = vec![position_ds, normal_ds];
/// // let mut times = Vec::new();
/// // if hd_merge_contributing_sample_times(&sources, -0.25, 0.25, &mut times) {
/// //     // Sample all attributes at merged times
/// // }
/// ```
pub fn hd_merge_contributing_sample_times(
    input_sources: &[HdSampledDataSourceHandle],
    start_time: HdSampledDataSourceTime,
    end_time: HdSampledDataSourceTime,
    out_sample_times: &mut Vec<HdSampledDataSourceTime>,
) -> bool {
    out_sample_times.clear();

    let mut has_samples = false;
    let mut all_times = Vec::new();

    for source in input_sources {
        let mut times = Vec::new();
        if source.get_contributing_sample_times(start_time, end_time, &mut times) {
            has_samples = true;
            all_times.extend(times);
        }
    }

    if !has_samples {
        return false;
    }

    // Sort and deduplicate
    all_times.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    all_times.dedup_by(|a, b| (*a - *b).abs() < 0.0001);

    *out_sample_times = all_times;
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_source::retained::HdRetainedSampledDataSource;

    #[test]
    fn test_uniform_sample() {
        let ds = HdRetainedSampledDataSource::new(Value::from(42i32));

        let value = ds.get_value(0.0);
        assert!(value.is::<i32>());
        assert_eq!(value.get::<i32>(), Some(&42));

        let mut times = Vec::new();
        let has_samples = ds.get_contributing_sample_times(-1.0, 1.0, &mut times);
        assert!(!has_samples);
        assert_eq!(times.len(), 0);
    }

    #[test]
    fn test_merge_empty() {
        let sources: Vec<HdSampledDataSourceHandle> = vec![];
        let mut times = Vec::new();
        let result = hd_merge_contributing_sample_times(&sources, -1.0, 1.0, &mut times);
        assert!(!result);
        assert_eq!(times.len(), 0);
    }
}
