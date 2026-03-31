//! DataSourceRelationship - Relationship data source for Hydra.
//!
//! Port of pxr/usdImaging/usdImaging/dataSourceRelationship.h
//!
//! A data source that represents a USD relationship as a path array.

use crate::data_source_stage_globals::DataSourceStageGlobalsHandle;
use std::sync::Arc;
use usd_hd::HdSampledDataSource;
use usd_sdf::Path;
use usd_tf::Token;
use usd_vt::Value;

// ============================================================================
// DataSourceRelationship
// ============================================================================

/// Data source representing a USD relationship.
///
/// Exposes the relationship targets as a path array data source.
/// Relationships are time-invariant, so sample times are ignored.
#[derive(Clone)]
pub struct DataSourceRelationship {
    /// Cached target paths
    #[allow(dead_code)] // Part of data source infrastructure
    targets: Vec<Path>,
    /// Stage globals reference
    #[allow(dead_code)]
    stage_globals: DataSourceStageGlobalsHandle,
}

impl DataSourceRelationship {
    /// Create a new relationship data source.
    ///
    /// # Arguments
    /// * `targets` - The relationship target paths
    /// * `stage_globals` - Stage globals for evaluation context
    pub fn new(targets: Vec<Path>, stage_globals: DataSourceStageGlobalsHandle) -> Self {
        Self {
            targets,
            stage_globals,
        }
    }

    /// Create from relationship name on a prim.
    ///
    /// Resolves forwarded targets from the USD relationship and caches them.
    pub fn from_prim(
        prim: &usd_core::Prim,
        rel_name: &Token,
        stage_globals: DataSourceStageGlobalsHandle,
    ) -> Option<Self> {
        let rel = prim.get_relationship(rel_name.as_str())?;
        let targets = rel.get_forwarded_targets();
        Some(Self::new(targets, stage_globals))
    }
}

impl std::fmt::Debug for DataSourceRelationship {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DataSourceRelationship")
    }
}

impl usd_hd::HdDataSourceBase for DataSourceRelationship {
    fn clone_box(&self) -> usd_hd::HdDataSourceBaseHandle {
        Arc::new(self.clone())
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

impl HdSampledDataSource for DataSourceRelationship {
    fn get_value(&self, _shutter_offset: usd_hd::HdSampledDataSourceTime) -> Value {
        // Return cached target paths as VtArray<SdfPath>
        Value::from_no_hash(self.targets.clone())
    }

    fn get_contributing_sample_times(
        &self,
        _start_time: usd_hd::HdSampledDataSourceTime,
        _end_time: usd_hd::HdSampledDataSourceTime,
        _out_sample_times: &mut Vec<usd_hd::HdSampledDataSourceTime>,
    ) -> bool {
        // Relationships are time-invariant
        false
    }
}

/// Handle type for DataSourceRelationship.
pub type DataSourceRelationshipHandle = Arc<DataSourceRelationship>;

/// Create a relationship data source.
pub fn create_data_source_relationship(
    targets: Vec<Path>,
    stage_globals: DataSourceStageGlobalsHandle,
) -> DataSourceRelationshipHandle {
    Arc::new(DataSourceRelationship::new(targets, stage_globals))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_source_stage_globals::NoOpStageGlobals;

    fn create_test_globals() -> DataSourceStageGlobalsHandle {
        Arc::new(NoOpStageGlobals::default())
    }

    #[test]
    fn test_relationship_data_source() {
        let globals = create_test_globals();
        let targets = vec![
            Path::from_string("/World/Mesh").unwrap(),
            Path::from_string("/World/Light").unwrap(),
        ];

        let ds = DataSourceRelationship::new(targets, globals);

        // Relationships are time-invariant
        let mut times = Vec::new();
        assert!(!ds.get_contributing_sample_times(0.0, 1.0, &mut times));
    }

    #[test]
    fn test_empty_relationship() {
        let globals = create_test_globals();
        let ds = DataSourceRelationship::new(vec![], globals);

        let _value = ds.get_value(0.0);
    }
}
