//! UsdStageRefPtrDataSource - Data source holding a USD stage reference.
//!
//! Port of UsdStageRefPtrDataSource = HdTypedSampledDataSource<UsdStageRefPtr> from
//! pxr/usdImaging/usdImaging/usdSceneIndexInputArgsSchema.h

use std::sync::Arc;
use usd_core::Stage;
use usd_hd::data_source::{
    HdDataSourceBase, HdSampledDataSource, HdSampledDataSourceTime, HdTypedSampledDataSource,
};
use usd_vt::Value;

/// Data source that holds an Arc<Stage> for use in UsdSceneIndexInputArgsSchema.
///
/// Port of UsdStageRefPtrDataSource.
pub struct UsdStageRefPtrDataSource {
    stage: Arc<Stage>,
}

impl std::fmt::Debug for UsdStageRefPtrDataSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UsdStageRefPtrDataSource")
            .field("stage", &"<Stage>")
            .finish()
    }
}

impl UsdStageRefPtrDataSource {
    /// Create a new stage data source.
    pub fn new(stage: Arc<Stage>) -> Arc<Self> {
        Arc::new(Self { stage })
    }
}

impl HdDataSourceBase for UsdStageRefPtrDataSource {
    fn clone_box(&self) -> usd_hd::data_source::HdDataSourceBaseHandle {
        Arc::new(Self {
            stage: self.stage.clone(),
        })
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_sampled(&self) -> Option<&dyn HdSampledDataSource> {
        Some(self)
    }

    fn sample_at_zero(&self) -> Option<usd_vt::Value> {
        Some(self.get_value(0.0))
    }
}

impl HdTypedSampledDataSource<Arc<Stage>> for UsdStageRefPtrDataSource {
    fn get_typed_value(&self, _shutter_offset: HdSampledDataSourceTime) -> Arc<Stage> {
        self.stage.clone()
    }
}

impl HdSampledDataSource for UsdStageRefPtrDataSource {
    fn get_value(&self, shutter_offset: HdSampledDataSourceTime) -> Value {
        Value::from_no_hash(self.get_typed_value(shutter_offset))
    }

    fn get_contributing_sample_times(
        &self,
        _start_time: HdSampledDataSourceTime,
        _end_time: HdSampledDataSourceTime,
        _out_sample_times: &mut Vec<HdSampledDataSourceTime>,
    ) -> bool {
        false
    }
}
