//! Prim adapter trait for USD to Hydra conversion.

use super::data_source_stage_globals::DataSourceStageGlobalsHandle;
use super::types::{PopulationMode, PropertyInvalidationType};

use std::sync::Arc;
use usd_core::Prim;
use usd_gf::Matrix4d;
use usd_hd::HdExtComputationContext;
use usd_hd::enums::HdInterpolation;
use usd_hd::prim::mesh::HdMeshTopology;
use usd_hd::scene_delegate::{HdExtComputationPrimvarDescriptorVector, HdPrimvarDescriptorVector};
use usd_hd::{HdContainerDataSourceHandle, HdDataSourceLocatorSet, HdDirtyBits};
use usd_sdf::Path;
use usd_sdf::TimeCode as SdfTimeCode;
use usd_tf::Token;
use usd_vt::Value;

/// Base trait for all prim adapters.
///
/// Prim adapters are responsible for converting USD prims to Hydra data sources.
/// Each USD prim type (UsdGeomMesh, UsdLuxLight, etc.) has a corresponding adapter
/// that knows how to extract the relevant data and expose it through Hydra's
/// data source API.
///
/// # Scene Index Approach (Modern)
///
/// The scene index API uses these methods:
/// - [`get_imaging_subprims`] - Returns list of Hydra prims this USD prim generates
/// - [`get_imaging_subprim_type`] - Returns Hydra type for each subprim
/// - [`get_imaging_subprim_data`] - Returns data source for each subprim
/// - [`invalidate_imaging_subprim`] - Handles USD property changes
///
/// # Population Mode
///
/// Adapters can control how they handle descendants via [`get_population_mode`]:
/// - `RepresentsSelf` - Only responsible for registered prim type
/// - `RepresentsSelfAndDescendents` - Handles prim and all descendants
/// - `RepresentedByAncestor` - Handled by ancestor adapter
pub trait PrimAdapter: Send + Sync {
    /// Returns list of imaging subprims this USD prim generates.
    ///
    /// Most adapters return a single subprim (often empty token for the prim itself).
    /// Some adapters may generate multiple Hydra prims from one USD prim.
    ///
    /// # Arguments
    ///
    /// * `prim` - The USD prim to analyze
    ///
    /// # Returns
    ///
    /// Vector of subprim names (empty token for the prim itself)
    fn get_imaging_subprims(&self, _prim: &Prim) -> Vec<Token> {
        // Default: single subprim representing the prim itself
        vec![Token::new("")]
    }

    /// Returns the Hydra prim type for a given subprim.
    ///
    /// # Arguments
    ///
    /// * `prim` - The USD prim
    /// * `subprim` - The subprim name (from `get_imaging_subprims`)
    ///
    /// # Returns
    ///
    /// The Hydra prim type token (e.g., "mesh", "camera", "light")
    fn get_imaging_subprim_type(&self, prim: &Prim, subprim: &Token) -> Token;

    /// Returns the data source for a given subprim.
    ///
    /// This is the core method that creates the Hydra data source tree
    /// containing all the prim's data (geometry, materials, transforms, etc.).
    ///
    /// # Arguments
    ///
    /// * `prim` - The USD prim
    /// * `subprim` - The subprim name
    /// * `stage_globals` - Stage-level context for data sources
    ///
    /// # Returns
    ///
    /// Container data source with all prim data
    fn get_imaging_subprim_data(
        &self,
        prim: &Prim,
        subprim: &Token,
        stage_globals: &DataSourceStageGlobalsHandle,
    ) -> Option<HdContainerDataSourceHandle>;

    /// Handles invalidation when USD properties change.
    ///
    /// Returns the set of data source locators that need to be dirtied
    /// when the given USD properties change.
    ///
    /// # Arguments
    ///
    /// * `prim` - The USD prim
    /// * `subprim` - The subprim name
    /// * `properties` - List of changed USD property names
    /// * `invalidation_type` - Whether properties were resynced or changed
    ///
    /// # Returns
    ///
    /// Set of data source locators to mark as dirty
    fn invalidate_imaging_subprim(
        &self,
        _prim: &Prim,
        _subprim: &Token,
        _properties: &[Token],
        _invalidation_type: PropertyInvalidationType,
    ) -> HdDataSourceLocatorSet {
        // Default: invalidate everything
        HdDataSourceLocatorSet::universal()
    }

    /// Returns the population mode for this adapter.
    ///
    /// See [`PopulationMode`] for details.
    fn get_population_mode(&self) -> PopulationMode {
        PopulationMode::RepresentsSelf
    }

    /// Handles invalidation from descendant prims.
    ///
    /// Only called when this adapter's mode is `RepresentsSelfAndDescendents`
    /// and a descendant prim (with mode `RepresentedByAncestor`) changes.
    ///
    /// # Arguments
    ///
    /// * `prim` - The USD prim this adapter handles
    /// * `descendant_prim` - The descendant prim that changed
    /// * `subprim` - The subprim name
    /// * `properties` - List of changed properties on descendant
    /// * `invalidation_type` - Whether properties were resynced or changed
    ///
    /// # Returns
    ///
    /// Set of data source locators to mark as dirty
    fn invalidate_imaging_subprim_from_descendant(
        &self,
        _prim: &Prim,
        _descendant_prim: &Prim,
        _subprim: &Token,
        _properties: &[Token],
        _invalidation_type: PropertyInvalidationType,
    ) -> HdDataSourceLocatorSet {
        // Default: no invalidation from descendants
        HdDataSourceLocatorSet::empty()
    }

    /// Returns whether this adapter should cull its children during population.
    ///
    /// When true, the scene index will not traverse into this prim's children.
    /// Used by adapters that handle descendants themselves.
    fn should_cull_children(&self) -> bool {
        self.get_population_mode() == PopulationMode::RepresentsSelfAndDescendents
    }

    /// Returns whether native USD prim instancing should be ignored for this prim.
    fn should_ignore_native_instance(&self, _prim: &Prim) -> bool {
        false
    }

    /// Whether this adapter is an instancer adapter (native instancing).
    /// C++: `UsdImagingPrimAdapter::IsInstancerAdapter()`.
    fn is_instancer_adapter(&self) -> bool {
        false
    }

    /// Whether `cache_path` is a child of this adapter's root.
    /// C++: `UsdImagingPrimAdapter::IsChildPath()`.
    fn is_child_path(&self, _cache_path: &Path) -> bool {
        false
    }

    /// Per-instance light linking categories for instancer adapters.
    /// Returns one category token list per instance.
    /// C++: `UsdImagingPrimAdapter::GetInstanceCategories()`.
    fn get_instance_categories(
        &self,
        _prim: &Prim,
        _light_link_cache: &crate::light_linking_cache::LightLinkingCache,
    ) -> Vec<Vec<Token>> {
        Vec::new()
    }

    // -----------------------------------------------------------------------
    // Hydra 1.0 delegate adapter methods (legacy)
    // -----------------------------------------------------------------------

    /// Check whether this prim type should cull its subtree during population.
    /// C++: UsdImagingPrimAdapter::ShouldCullSubtree(). Returns true for
    /// non-imageable prim types that should never appear in the render index.
    fn should_cull_subtree(_prim: &Prim) -> bool
    where
        Self: Sized,
    {
        false
    }

    /// Non-imageable prim types that are never directly renderable.
    /// Population skips these and only recurses their children.
    /// C++: UsdImagingPrimAdapter::ShouldCullSubtree() checks !prim.IsA<UsdGeomImageable>().
    /// This static list covers all standard non-imageable schema types.
    fn non_imaging_prim_types() -> &'static [&'static str]
    where
        Self: Sized,
    {
        &[
            "",                // untyped prims
            "Scope",           // UsdGeomScope is imageable, but has no renderable geometry
            "NodeGraph",       // UsdShadeNodeGraph
            "Material",        // UsdShadeMaterial
            "Shader",          // UsdShadeShader
            "GeomSubset",      // UsdGeomSubset (face sets, not standalone geometry)
            "SkelAnimation",   // UsdSkelAnimation
            "BlendShape",      // UsdSkelBlendShape
            "SkelRoot",        // UsdSkelRoot (container, children populated individually)
            "PhysicsScene",    // UsdPhysicsScene
            "CoordSysBinding", // coordinate system binding prim
        ]
    }

    /// Compute per-adapter time-varying dirty bits (Hydra 1.0 TrackVariability).
    /// Returns the OR of all dirty bits that may change over time for this prim.
    /// Default: all bits (!0) — adapters should override to be more selective.
    fn track_variability(&self, _prim: &Prim, _time: SdfTimeCode) -> HdDirtyBits {
        !0
    }

    /// Update prim data for the given time (Hydra 1.0 UpdateForTime).
    /// Called during Sync() for each dirty prim. Adapters populate caches,
    /// compute extents, update primvar values, etc.
    /// `dirty_bits` contains the bits that need updating.
    /// Default: no-op — actual data pull happens in Get* methods on demand.
    fn update_for_time(&self, _prim: &Prim, _time: SdfTimeCode, _dirty_bits: HdDirtyBits) {
        // No-op by default. Adapters override to pre-compute cached data.
    }

    /// Get world transform for this prim (Hydra 1.0 adapter dispatch).
    /// Default: None (delegate falls back to XformCache).
    fn get_transform(&self, _prim: &Prim, _time: SdfTimeCode) -> Option<Matrix4d> {
        None
    }

    /// Get visibility for this prim (Hydra 1.0 adapter dispatch).
    /// Default: None (delegate falls back to inherited visibility walk).
    fn get_visible(&self, _prim: &Prim, _time: SdfTimeCode) -> Option<bool> {
        None
    }

    /// Get mesh topology for this prim (Hydra 1.0 adapter dispatch).
    /// Default: None (delegate falls back to direct Mesh API read).
    fn get_mesh_topology(&self, _prim: &Prim, _time: SdfTimeCode) -> Option<HdMeshTopology> {
        None
    }

    /// Get primvar descriptors for this prim (Hydra 1.0 adapter dispatch).
    /// Default: None (delegate falls back to scanning primvars: attributes).
    fn get_primvar_descriptors(
        &self,
        _prim: &Prim,
        _interpolation: HdInterpolation,
        _time: SdfTimeCode,
    ) -> Option<HdPrimvarDescriptorVector> {
        None
    }

    // -----------------------------------------------------------------------
    // ExtComputation adapter methods (Hydra 1.0)
    // -----------------------------------------------------------------------

    /// Get ext computation scene input names. C++: GetExtComputationSceneInputNames.
    fn get_ext_computation_scene_input_names(
        &self,
        _prim: &Prim,
        _computation_id: &Path,
    ) -> Vec<Token> {
        Vec::new()
    }

    /// Get ext computation input descriptors. C++: GetExtComputationInputDescriptors.
    /// Returns vec of (input_name, source_computation_output_name).
    fn get_ext_computation_input_descriptors(
        &self,
        _prim: &Prim,
        _computation_id: &Path,
    ) -> Vec<(Token, Token)> {
        Vec::new()
    }

    /// Get ext computation output descriptors. C++: GetExtComputationOutputDescriptors.
    /// Returns vec of (output_name, value_type_name).
    fn get_ext_computation_output_descriptors(
        &self,
        _prim: &Prim,
        _computation_id: &Path,
    ) -> Vec<(Token, Token)> {
        Vec::new()
    }

    /// Get ext computation primvar descriptors. C++: GetExtComputationPrimvarsDescriptors.
    fn get_ext_computation_primvar_descriptors(
        &self,
        _prim: &Prim,
        _computation_id: &Path,
        _interpolation: HdInterpolation,
    ) -> HdExtComputationPrimvarDescriptorVector {
        Vec::new()
    }

    /// Get ext computation input value. C++: GetExtComputationInput.
    fn get_ext_computation_input(
        &self,
        _prim: &Prim,
        _computation_id: &Path,
        _input: &Token,
    ) -> Option<Value> {
        None
    }

    /// Get ext computation kernel source. C++: GetExtComputationKernel.
    fn get_ext_computation_kernel(&self, _prim: &Prim, _computation_id: &Path) -> String {
        String::new()
    }

    /// Invoke ext computation. Matches C++ `UsdImagingPrimAdapter::InvokeComputation`.
    /// The adapter reads inputs from context, computes, and writes outputs back.
    fn invoke_ext_computation(
        &self,
        _prim: &Prim,
        _computation_id: &Path,
        _context: &mut dyn HdExtComputationContext,
    ) {
    }
}

/// Arc-wrapped prim adapter for sharing
pub type PrimAdapterHandle = Arc<dyn PrimAdapter>;

/// Default no-op adapter for testing
#[derive(Debug, Clone)]
pub struct NoOpAdapter {
    prim_type: Token,
}

impl NoOpAdapter {
    /// Create new no-op adapter for given prim type
    pub fn new(prim_type: Token) -> Self {
        Self { prim_type }
    }
}

impl PrimAdapter for NoOpAdapter {
    fn get_imaging_subprim_type(&self, _prim: &Prim, _subprim: &Token) -> Token {
        self.prim_type.clone()
    }

    fn get_imaging_subprim_data(
        &self,
        _prim: &Prim,
        _subprim: &Token,
        _stage_globals: &DataSourceStageGlobalsHandle,
    ) -> Option<HdContainerDataSourceHandle> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use usd_core::Stage;

    #[test]
    fn test_noop_adapter() {
        let adapter = NoOpAdapter::new(Token::new("mesh"));
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();

        let subprims = adapter.get_imaging_subprims(&prim);
        assert_eq!(subprims.len(), 1);
        assert_eq!(subprims[0].as_str(), "");

        let prim_type = adapter.get_imaging_subprim_type(&prim, &subprims[0]);
        assert_eq!(prim_type.as_str(), "mesh");
    }

    #[test]
    fn test_default_population_mode() {
        let adapter = NoOpAdapter::new(Token::new("mesh"));
        assert_eq!(
            adapter.get_population_mode(),
            PopulationMode::RepresentsSelf
        );
    }

    #[test]
    fn test_should_cull_children() {
        let adapter = NoOpAdapter::new(Token::new("mesh"));
        assert!(!adapter.should_cull_children());
    }

    #[test]
    fn test_default_invalidation() {
        let adapter = NoOpAdapter::new(Token::new("mesh"));
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let subprim = Token::new("");
        let properties = vec![Token::new("points")];

        let locators = adapter.invalidate_imaging_subprim(
            &prim,
            &subprim,
            &properties,
            PropertyInvalidationType::PropertyChanged,
        );

        assert!(locators.is_universal());
    }
}
