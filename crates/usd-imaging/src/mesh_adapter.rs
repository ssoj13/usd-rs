//! MeshAdapter - Adapter for UsdGeomMesh.
//!
//! Port of pxr/usdImaging/usdImaging/meshAdapter.h/cpp
//!
//! Provides imaging support for UsdGeomMesh prims, including:
//! - Mesh topology (face counts, vertex indices)
//! - Subdivision surface parameters
//! - Primvars (points, normals, UVs, displayColor)
//! - Geom subsets for material binding

use super::data_source_mesh::DataSourceMeshPrim;
use super::data_source_stage_globals::DataSourceStageGlobalsHandle;
use super::gprim_adapter::GprimAdapter;
use super::prim_adapter::PrimAdapter;
use super::types::PropertyInvalidationType;
use std::sync::Arc;
use usd_core::Prim;
use usd_hd::change_tracker::HdRprimDirtyBits;
use usd_hd::types::HdDirtyBits;
use usd_hd::{HdContainerDataSourceHandle, HdDataSourceLocatorSet};
use usd_sdf::TimeCode as SdfTimeCode;
use usd_tf::Token;

// Token constants
mod tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    pub static MESH: LazyLock<Token> = LazyLock::new(|| Token::new("mesh"));
}

/// Adapter for UsdGeomMesh prims.
///
/// This adapter converts UsdGeomMesh data to Hydra mesh primitives,
/// providing:
/// - Mesh topology (faceVertexCounts, faceVertexIndices)
/// - Subdivision surface parameters (scheme, creases, corners)
/// - Primvars (points, normals, UVs, colors)
/// - Material bindings including geom subsets
///
/// # Example
///
/// ```ignore
/// use usd_imaging::MeshAdapter;
///
/// let adapter = MeshAdapter::new();
/// // Register with adapter registry...
/// ```
#[derive(Debug, Clone)]
pub struct MeshAdapter {
    /// Base gprim adapter for shared functionality
    #[allow(dead_code)] // For future composition with GprimAdapter methods
    gprim: GprimAdapter,
}

impl Default for MeshAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl MeshAdapter {
    /// Create a new mesh adapter.
    pub fn new() -> Self {
        Self {
            gprim: GprimAdapter::new(tokens::MESH.clone()),
        }
    }

    /// Check if a primvar is built-in for meshes.
    ///
    /// Built-in primvars are handled specially and don't need
    /// generic primvar processing.
    pub fn is_builtin_primvar(primvar_name: &Token) -> bool {
        matches!(
            primvar_name.as_str(),
            "points"
                | "normals"
                | "velocities"
                | "accelerations"
                | "displayColor"
                | "displayOpacity"
        )
    }
}

impl PrimAdapter for MeshAdapter {
    /// Compute time-varying dirty bits for a mesh.
    /// Matches C++ UsdImagingMeshAdapter::TrackVariability.
    fn track_variability(&self, prim: &Prim, time: SdfTimeCode) -> HdDirtyBits {
        let mut bits = GprimAdapter::track_variability_base(prim, time);

        // Check points for time-varying (mesh-specific, not in gprim base).
        if let Some(attr) = prim.get_attribute("points") {
            let n_samples = attr.get_num_time_samples();
            let is_varying = attr.value_might_be_time_varying();
            {
                use std::sync::atomic::{AtomicU32, Ordering};
                static PTS_LOG: AtomicU32 = AtomicU32::new(0);
                let n = PTS_LOG.fetch_add(1, Ordering::Relaxed);
                if n < 3 {
                    let attr_names: Vec<_> = prim
                        .get_attribute_names()
                        .iter()
                        .map(|t| t.get_text().to_owned())
                        .collect();
                    let has_stage = prim.stage().is_some();
                    log::trace!(
                        "[PERF] mesh track_variability: {} points n_samples={} is_varying={} has_stage={} n_attrs={} attrs={:?}",
                        prim.path(),
                        n_samples,
                        is_varying,
                        has_stage,
                        attr_names.len(),
                        &attr_names[..attr_names.len().min(5)]
                    );
                }
            }
            if is_varying {
                bits |= HdRprimDirtyBits::DIRTY_POINTS;
            }
        } else {
            use std::sync::atomic::{AtomicU32, Ordering};
            static NO_PTS: AtomicU32 = AtomicU32::new(0);
            let n = NO_PTS.fetch_add(1, Ordering::Relaxed);
            if n < 3 {
                log::trace!(
                    "[PERF] mesh track_variability: {} NO points attr!",
                    prim.path()
                );
            }
        }

        // Check normals for time-varying (only for polygonal meshes, not subdiv).
        if let Some(attr) = prim.get_attribute("primvars:normals") {
            if attr.value_might_be_time_varying() {
                bits |= HdRprimDirtyBits::DIRTY_NORMALS;
            }
        } else if let Some(attr) = prim.get_attribute("normals") {
            if attr.value_might_be_time_varying() {
                bits |= HdRprimDirtyBits::DIRTY_NORMALS;
            }
        }

        bits
    }

    fn get_imaging_subprims(&self, _prim: &Prim) -> Vec<Token> {
        // Mesh produces a single imaging prim
        vec![Token::new("")]
    }

    fn get_imaging_subprim_type(&self, _prim: &Prim, subprim: &Token) -> Token {
        if subprim.is_empty() {
            tokens::MESH.clone()
        } else {
            Token::new("")
        }
    }

    fn get_imaging_subprim_data(
        &self,
        prim: &Prim,
        subprim: &Token,
        stage_globals: &DataSourceStageGlobalsHandle,
    ) -> Option<HdContainerDataSourceHandle> {
        if subprim.is_empty() {
            Some(DataSourceMeshPrim::new(
                prim.path().clone(),
                prim.clone(),
                stage_globals.clone(),
            ))
        } else {
            None
        }
    }

    fn invalidate_imaging_subprim(
        &self,
        prim: &Prim,
        subprim: &Token,
        properties: &[Token],
        invalidation_type: PropertyInvalidationType,
    ) -> HdDataSourceLocatorSet {
        DataSourceMeshPrim::invalidate(prim, subprim, properties, invalidation_type)
    }
}

/// Arc-wrapped MeshAdapter for sharing
pub type MeshAdapterHandle = Arc<MeshAdapter>;

/// Factory function for creating mesh adapters.
pub fn create_mesh_adapter() -> Arc<dyn PrimAdapter> {
    Arc::new(MeshAdapter::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_source_stage_globals::NoOpStageGlobals;
    use usd_core::Stage;

    fn create_test_globals() -> DataSourceStageGlobalsHandle {
        Arc::new(NoOpStageGlobals::default())
    }

    #[test]
    fn test_mesh_adapter_creation() {
        let adapter = MeshAdapter::new();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let subprim = Token::new("");

        let prim_type = adapter.get_imaging_subprim_type(&prim, &subprim);
        assert_eq!(prim_type.as_str(), "mesh");
    }

    #[test]
    fn test_mesh_adapter_subprims() {
        let adapter = MeshAdapter::new();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();

        let subprims = adapter.get_imaging_subprims(&prim);
        assert_eq!(subprims.len(), 1);
        assert!(subprims[0].is_empty());
    }

    #[test]
    fn test_mesh_adapter_data_source() {
        let adapter = MeshAdapter::new();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let globals = create_test_globals();

        let ds = adapter.get_imaging_subprim_data(&prim, &Token::new(""), &globals);
        assert!(ds.is_some());
    }

    #[test]
    fn test_builtin_primvars() {
        assert!(MeshAdapter::is_builtin_primvar(&Token::new("points")));
        assert!(MeshAdapter::is_builtin_primvar(&Token::new("normals")));
        assert!(MeshAdapter::is_builtin_primvar(&Token::new("displayColor")));
        assert!(!MeshAdapter::is_builtin_primvar(&Token::new("st"))); // UVs are not built-in
    }

    #[test]
    fn test_mesh_adapter_invalidation() {
        let adapter = MeshAdapter::new();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let properties = vec![Token::new("faceVertexCounts"), Token::new("points")];

        let locators = adapter.invalidate_imaging_subprim(
            &prim,
            &Token::new(""),
            &properties,
            PropertyInvalidationType::PropertyChanged,
        );

        assert!(!locators.is_empty());
    }
}
