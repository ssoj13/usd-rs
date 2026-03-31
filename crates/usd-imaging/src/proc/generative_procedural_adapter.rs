//! Generative Procedural Adapter for USD to Hydra translation.
//!
//! This adapter handles conversion of UsdProcGenerativeProcedural prims
//! to Hydra generative procedural representations.

use super::tokens::UsdProcImagingTokens;
use crate::data_source_prim::DataSourcePrim;
use crate::data_source_stage_globals::DataSourceStageGlobalsHandle;
use crate::tokens::UsdImagingTokens;
use crate::types::PropertyInvalidationType;
use std::sync::Arc;
use usd_core::Prim as UsdPrim;
use usd_hd::{HdContainerDataSourceHandle, HdDataSourceLocator, HdDataSourceLocatorSet};
use usd_proc::GenerativeProcedural;
use usd_proc::USD_PROC_TOKENS;
use usd_sdf::Path as SdfPath;
use usd_sdf::TimeCode;
use usd_tf::Token as TfToken;

/// Adapter for translating UsdProcGenerativeProcedural to Hydra.
///
/// This adapter extends the InstanceablePrimAdapter pattern to handle
/// generative procedural prims. It connects USD procedural definitions
/// to the HdGp (Hydra Generative Procedural) system.
///
/// # Architecture
///
/// The adapter provides:
/// - Imaging subprim enumeration
/// - Data source creation for procedural parameters
/// - Change tracking and invalidation
/// - Transform and visibility propagation
///
/// # Hydra Prim Type
///
/// The adapter determines the Hydra prim type from the `proceduralSystem`
/// attribute. If not specified, defaults to `inertGenerativeProcedural`.
///
/// # C++ Reference
///
/// Port of `UsdProcImagingGenerativeProceduralAdapter` from
/// `pxr/usdImaging/usdProcImaging/generativeProceduralAdapter.h`
#[derive(Debug, Clone)]
pub struct GenerativeProceduralAdapter {
    // Future: adapter state and caching
}

impl GenerativeProceduralAdapter {
    /// Create new generative procedural adapter.
    pub fn new() -> Self {
        Self {}
    }

    /// Get imaging subprims for the given prim.
    ///
    /// Returns empty token for the prim itself (default subprim).
    pub fn get_imaging_subprims(&self, _prim: &UsdPrim) -> Vec<TfToken> {
        vec![TfToken::empty()]
    }

    /// Get imaging subprim type.
    ///
    /// For the default subprim (empty token), returns the hydra prim type
    /// determined from the procedural system attribute.
    pub fn get_imaging_subprim_type(&self, prim: &UsdPrim, subprim: &TfToken) -> Option<TfToken> {
        if subprim.is_empty() {
            Some(self.get_hydra_prim_type(prim))
        } else {
            None
        }
    }

    /// Get imaging subprim data for the procedural prim.
    ///
    /// For the default subprim (empty token), returns a DataSourcePrim wrapping
    /// the USD prim. Without this, procedural prims are invisible to the scene index.
    ///
    /// Port of C++ `GetImagingSubprimData` (generativeProceduralAdapter.cpp:64-77).
    pub fn get_imaging_subprim_data(
        &self,
        prim: &UsdPrim,
        subprim: &TfToken,
        stage_globals: &DataSourceStageGlobalsHandle,
    ) -> Option<HdContainerDataSourceHandle> {
        if subprim.is_empty() {
            let ds = DataSourcePrim::new(prim.clone(), prim.path().clone(), stage_globals.clone());
            Some(Arc::new(ds) as HdContainerDataSourceHandle)
        } else {
            None
        }
    }

    /// Handle invalidation when USD properties change.
    ///
    /// If `proceduralSystem` changes, returns the repopulate locator to trigger
    /// a full resync in the stage scene index. For other properties, delegates
    /// to `DataSourcePrim::invalidate`.
    ///
    /// Port of C++ `InvalidateImagingSubprim` (generativeProceduralAdapter.cpp:80-105).
    pub fn invalidate_imaging_subprim(
        &self,
        prim: &UsdPrim,
        subprim: &TfToken,
        properties: &[TfToken],
        invalidation_type: PropertyInvalidationType,
    ) -> HdDataSourceLocatorSet {
        if subprim.is_empty() {
            // Check if proceduralSystem changed - requires full resync
            for name in properties {
                if *name == USD_PROC_TOKENS.procedural_system {
                    return HdDataSourceLocatorSet::from_locator(HdDataSourceLocator::from_token(
                        UsdImagingTokens::stage_scene_index_repopulate().clone(),
                    ));
                }
            }

            // Delegate other property changes to base DataSourcePrim
            return DataSourcePrim::invalidate(prim, subprim, properties, invalidation_type);
        }

        HdDataSourceLocatorSet::empty()
    }

    /// Check if adapter is supported.
    ///
    /// Generative procedural adapter is always supported.
    pub fn is_supported(&self) -> bool {
        true
    }

    /// Get Hydra prim type for the procedural prim.
    ///
    /// Reads the `proceduralSystem` attribute from the GenerativeProcedural
    /// prim. If not specified or empty, returns `inertGenerativeProcedural`.
    ///
    /// # Arguments
    ///
    /// * `prim` - USD prim to query
    ///
    /// # Returns
    ///
    /// Hydra prim type token (procedural system name or inert default)
    pub fn get_hydra_prim_type(&self, prim: &UsdPrim) -> TfToken {
        // Try to get procedural system attribute
        let gen_proc = GenerativeProcedural::new(prim.clone());
        if let Some(proc_sys_attr) = gen_proc.get_procedural_system_attr() {
            if let Some(value) = proc_sys_attr.get(TimeCode::default()) {
                if let Some(token) = value.get::<TfToken>() {
                    if !token.is_empty() {
                        return token.clone();
                    }
                }
            }
        }

        // Default to inert procedural type
        UsdProcImagingTokens::inert_generative_procedural().clone()
    }

    /// Populate the index with the procedural prim.
    ///
    /// # Arguments
    ///
    /// * `prim` - USD prim to populate
    /// * `cache_path` - Path in the Hydra index
    ///
    /// # Returns
    ///
    /// Cache path where the prim was inserted
    pub fn populate(&self, _prim: &UsdPrim, cache_path: &SdfPath) -> SdfPath {
        // Future: insert rprim into index
        // For now, just return the cache path
        cache_path.clone()
    }

    /// Mark the prim dirty with specified dirty bits.
    ///
    /// # Arguments
    ///
    /// * `cache_path` - Path to mark dirty
    /// * `dirty_bits` - Dirty bits to set
    pub fn mark_dirty(&self, _cache_path: &SdfPath, _dirty_bits: u32) {
        // Future: mark rprim dirty in index
    }

    /// Track time-varying attributes.
    ///
    /// Checks for time-varying primvars, extent, transforms, and visibility.
    ///
    /// # Arguments
    ///
    /// * `prim` - USD prim to check
    /// * `time_varying_bits` - Output dirty bits for time-varying attributes
    pub fn track_variability(&self, _prim: &UsdPrim, _time_varying_bits: &mut u32) {
        // Future: check for time-varying primvars, extent, xform, visibility
    }
}

impl Default for GenerativeProceduralAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use usd_core::{InitialLoadSet, Stage as UsdStage};

    #[test]
    fn test_adapter_creation() {
        let adapter = GenerativeProceduralAdapter::new();
        assert!(adapter.is_supported());
    }

    #[test]
    fn test_hydra_prim_type_default() {
        let stage = UsdStage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
        let prim = stage
            .define_prim("/Test", "GenerativeProcedural")
            .expect("define prim");

        let adapter = GenerativeProceduralAdapter::new();
        let prim_type = adapter.get_hydra_prim_type(&prim);

        assert_eq!(
            prim_type.as_str(),
            "inertGenerativeProcedural",
            "Should default to inert type"
        );
    }

    #[test]
    fn test_imaging_subprims() {
        let stage = UsdStage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
        let prim = stage
            .define_prim("/Test", "GenerativeProcedural")
            .expect("define prim");

        let adapter = GenerativeProceduralAdapter::new();
        let subprims = adapter.get_imaging_subprims(&prim);

        assert_eq!(subprims.len(), 1, "Should have one subprim");
        assert!(subprims[0].is_empty(), "Should be default subprim");
    }

    #[test]
    fn test_imaging_subprim_type() {
        let stage = UsdStage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
        let prim = stage
            .define_prim("/Test", "GenerativeProcedural")
            .expect("define prim");

        let adapter = GenerativeProceduralAdapter::new();
        let subprim_type = adapter
            .get_imaging_subprim_type(&prim, &TfToken::empty())
            .expect("Should return type");

        assert_eq!(
            subprim_type.as_str(),
            "inertGenerativeProcedural",
            "Should return inert type"
        );
    }
}
