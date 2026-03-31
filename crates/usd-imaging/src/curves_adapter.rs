//! Curves Adapters - Adapters for USD curve geometry primitives.
//!
//! Port of pxr/usdImaging/usdImaging/{basisCurves,hermiteCurves,nurbsCurves,nurbsPatch}Adapter.h/cpp
//!
//! Provides imaging support for curve primitives:
//! - BasisCurves
//! - HermiteCurves
//! - NurbsCurves
//! - NurbsPatch

use crate::data_source_basis_curves as canonical_basis_curves_data_source;
use crate::data_source_hermite_curves as canonical_hermite_curves_data_source;
use crate::data_source_nurbs_curves as canonical_nurbs_curves_data_source;
use crate::data_source_nurbs_patch as canonical_nurbs_patch_data_source;

use super::data_source_gprim::DataSourceGprim;
use super::data_source_stage_globals::DataSourceStageGlobalsHandle;
use super::data_source_attribute::DataSourceAttribute;
use super::prim_adapter::PrimAdapter;
use super::types::PropertyInvalidationType;
use std::sync::Arc;
use usd_core::Prim;
use usd_hd::{
    HdContainerDataSource, HdContainerDataSourceHandle, HdDataSourceBase, HdDataSourceBaseHandle,
    HdDataSourceLocator, HdDataSourceLocatorSet,
};
use usd_sdf::Path;
use usd_tf::Token;
use usd_vt::{Array, Value};

// Token constants for curves
#[allow(dead_code)] // Tokens defined for future use
mod tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    // Prim types
    pub static BASIS_CURVES: LazyLock<Token> = LazyLock::new(|| Token::new("basisCurves"));
    pub static HERMITE_CURVES: LazyLock<Token> = LazyLock::new(|| Token::new("hermiteCurves"));
    pub static NURBS_CURVES: LazyLock<Token> = LazyLock::new(|| Token::new("nurbsCurves"));
    pub static NURBS_PATCH: LazyLock<Token> = LazyLock::new(|| Token::new("nurbsPatch"));

    // BasisCurves attributes
    pub static TYPE: LazyLock<Token> = LazyLock::new(|| Token::new("type"));
    pub static BASIS: LazyLock<Token> = LazyLock::new(|| Token::new("basis"));
    pub static WRAP: LazyLock<Token> = LazyLock::new(|| Token::new("wrap"));
    pub static CURVE_VERTEX_COUNTS: LazyLock<Token> =
        LazyLock::new(|| Token::new("curveVertexCounts"));

    // Common curve attributes
    pub static WIDTHS: LazyLock<Token> = LazyLock::new(|| Token::new("widths"));
    pub static NORMALS: LazyLock<Token> = LazyLock::new(|| Token::new("normals"));

    // NurbsCurves/NurbsPatch attributes
    pub static ORDER: LazyLock<Token> = LazyLock::new(|| Token::new("order"));
    pub static KNOTS: LazyLock<Token> = LazyLock::new(|| Token::new("knots"));
    pub static RANGES: LazyLock<Token> = LazyLock::new(|| Token::new("ranges"));
    pub static POINT_WEIGHTS: LazyLock<Token> = LazyLock::new(|| Token::new("pointWeights"));

    // NurbsPatch-specific
    pub static U_VERTEX_COUNT: LazyLock<Token> = LazyLock::new(|| Token::new("uVertexCount"));
    pub static V_VERTEX_COUNT: LazyLock<Token> = LazyLock::new(|| Token::new("vVertexCount"));
    pub static U_ORDER: LazyLock<Token> = LazyLock::new(|| Token::new("uOrder"));
    pub static V_ORDER: LazyLock<Token> = LazyLock::new(|| Token::new("vOrder"));
    pub static U_KNOTS: LazyLock<Token> = LazyLock::new(|| Token::new("uKnots"));
    pub static V_KNOTS: LazyLock<Token> = LazyLock::new(|| Token::new("vKnots"));
    pub static U_RANGE: LazyLock<Token> = LazyLock::new(|| Token::new("uRange"));
    pub static V_RANGE: LazyLock<Token> = LazyLock::new(|| Token::new("vRange"));
    pub static U_FORM: LazyLock<Token> = LazyLock::new(|| Token::new("uForm"));
    pub static V_FORM: LazyLock<Token> = LazyLock::new(|| Token::new("vForm"));
    pub static TRIM_CURVE_COUNTS: LazyLock<Token> = LazyLock::new(|| Token::new("trimCurveCounts"));
    pub static TRIM_CURVE_ORDERS: LazyLock<Token> = LazyLock::new(|| Token::new("trimCurveOrders"));
    pub static TRIM_CURVE_VERTEX_COUNTS: LazyLock<Token> =
        LazyLock::new(|| Token::new("trimCurveVertexCounts"));
    pub static TRIM_CURVE_KNOTS: LazyLock<Token> = LazyLock::new(|| Token::new("trimCurveKnots"));
    pub static TRIM_CURVE_RANGES: LazyLock<Token> = LazyLock::new(|| Token::new("trimCurveRanges"));
    pub static TRIM_CURVE_POINTS: LazyLock<Token> = LazyLock::new(|| Token::new("trimCurvePoints"));

    // Locators
    pub static PRIMVARS: LazyLock<Token> = LazyLock::new(|| Token::new("primvars"));
    pub static POINTS: LazyLock<Token> = LazyLock::new(|| Token::new("points"));
    pub static TOPOLOGY: LazyLock<Token> = LazyLock::new(|| Token::new("topology"));
}

// ============================================================================
// DataSourceCurves
// ============================================================================

/// Data source for curve parameters.
#[derive(Clone)]
pub struct DataSourceCurves {
    #[allow(dead_code)] // For future curve attribute reading
    prim: Prim,
    #[allow(dead_code)] // For future time-sampled reading
    stage_globals: DataSourceStageGlobalsHandle,
    curve_type: Token,
}

impl std::fmt::Debug for DataSourceCurves {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataSourceCurves")
            .field("curve_type", &self.curve_type)
            .finish()
    }
}

impl DataSourceCurves {
    /// Create new curves data source.
    pub fn new(
        prim: Prim,
        stage_globals: DataSourceStageGlobalsHandle,
        curve_type: Token,
    ) -> Arc<Self> {
        Arc::new(Self {
            prim,
            stage_globals,
            curve_type,
        })
    }

    /// Get type-specific parameter names.
    fn get_type_specific_params(&self) -> Vec<Token> {
        match self.curve_type.as_str() {
            "basisCurves" => vec![
                tokens::TYPE.clone(),
                tokens::BASIS.clone(),
                tokens::WRAP.clone(),
                tokens::CURVE_VERTEX_COUNTS.clone(),
                tokens::WIDTHS.clone(),
            ],
            "hermiteCurves" => vec![tokens::CURVE_VERTEX_COUNTS.clone(), tokens::WIDTHS.clone()],
            "nurbsCurves" => vec![
                tokens::ORDER.clone(),
                tokens::KNOTS.clone(),
                tokens::RANGES.clone(),
                tokens::CURVE_VERTEX_COUNTS.clone(),
                tokens::POINT_WEIGHTS.clone(),
            ],
            "nurbsPatch" => vec![
                tokens::U_VERTEX_COUNT.clone(),
                tokens::V_VERTEX_COUNT.clone(),
                tokens::U_ORDER.clone(),
                tokens::V_ORDER.clone(),
                tokens::U_KNOTS.clone(),
                tokens::V_KNOTS.clone(),
                tokens::U_RANGE.clone(),
                tokens::V_RANGE.clone(),
                tokens::U_FORM.clone(),
                tokens::V_FORM.clone(),
                tokens::POINT_WEIGHTS.clone(),
            ],
            _ => vec![],
        }
    }
}

impl HdDataSourceBase for DataSourceCurves {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(self.clone())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<usd_hd::HdContainerDataSourceHandle> {
        Some(std::sync::Arc::new(self.clone()))
    }
}

impl HdContainerDataSource for DataSourceCurves {
    fn get_names(&self) -> Vec<Token> {
        self.get_type_specific_params()
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        let attr = self.prim.get_attribute(name.as_str())?;
        if !attr.is_valid() {
            return None;
        }

        if *name == *tokens::TYPE
            || *name == *tokens::BASIS
            || *name == *tokens::WRAP
            || *name == *tokens::U_FORM
            || *name == *tokens::V_FORM
        {
            return Some(DataSourceAttribute::<Token>::new(
                attr,
                self.stage_globals.clone(),
                self.prim.path().clone(),
            ) as HdDataSourceBaseHandle);
        }

        if *name == *tokens::CURVE_VERTEX_COUNTS
            || *name == *tokens::TRIM_CURVE_COUNTS
            || *name == *tokens::TRIM_CURVE_ORDERS
            || *name == *tokens::TRIM_CURVE_VERTEX_COUNTS
        {
            return Some(DataSourceAttribute::<Array<i32>>::new(
                attr,
                self.stage_globals.clone(),
                self.prim.path().clone(),
            ) as HdDataSourceBaseHandle);
        }

        Some(DataSourceAttribute::<Value>::new(
            attr,
            self.stage_globals.clone(),
            self.prim.path().clone(),
        ) as HdDataSourceBaseHandle)
    }
}

// ============================================================================
// DataSourceCurvesPrim
// ============================================================================

/// Prim data source for curve prims.
#[derive(Clone)]
pub struct DataSourceCurvesPrim {
    #[allow(dead_code)]
    scene_index_path: Path,
    gprim_ds: Arc<DataSourceGprim>,
    curves_ds: Arc<DataSourceCurves>,
    #[allow(dead_code)]
    curve_type: Token,
}

impl std::fmt::Debug for DataSourceCurvesPrim {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataSourceCurvesPrim").finish()
    }
}

impl DataSourceCurvesPrim {
    /// Create new curves prim data source.
    pub fn new(
        scene_index_path: Path,
        prim: Prim,
        stage_globals: DataSourceStageGlobalsHandle,
        curve_type: Token,
    ) -> Arc<Self> {
        let gprim_ds = DataSourceGprim::new(
            scene_index_path.clone(),
            prim.clone(),
            stage_globals.clone(),
        );
        let curves_ds = DataSourceCurves::new(prim, stage_globals, curve_type.clone());

        Arc::new(Self {
            scene_index_path,
            gprim_ds,
            curves_ds,
            curve_type,
        })
    }

    /// Compute invalidation for property changes.
    pub fn invalidate(
        prim: &Prim,
        subprim: &Token,
        properties: &[Token],
        invalidation_type: PropertyInvalidationType,
    ) -> HdDataSourceLocatorSet {
        let mut locators =
            DataSourceGprim::invalidate(prim, subprim, properties, invalidation_type);

        for prop in properties {
            let prop_str = prop.as_str();
            match prop_str {
                // Topology changes
                "curveVertexCounts" | "type" | "basis" | "wrap" | "order" | "knots" | "ranges"
                | "uVertexCount" | "vVertexCount" | "uOrder" | "vOrder" | "uKnots" | "vKnots"
                | "uRange" | "vRange" | "uForm" | "vForm" => {
                    locators.insert(HdDataSourceLocator::from_token(tokens::TOPOLOGY.clone()));
                }
                // Primvar changes
                "widths" => {
                    locators.insert(HdDataSourceLocator::from_tokens_2(
                        tokens::PRIMVARS.clone(),
                        tokens::WIDTHS.clone(),
                    ));
                }
                "normals" => {
                    locators.insert(HdDataSourceLocator::from_tokens_2(
                        tokens::PRIMVARS.clone(),
                        tokens::NORMALS.clone(),
                    ));
                }
                "pointWeights" => {
                    locators.insert(HdDataSourceLocator::from_tokens_2(
                        tokens::PRIMVARS.clone(),
                        tokens::POINTS.clone(),
                    ));
                }
                _ => {}
            }
        }

        locators
    }
}

impl HdDataSourceBase for DataSourceCurvesPrim {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(self.clone())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<usd_hd::HdContainerDataSourceHandle> {
        Some(std::sync::Arc::new(self.clone()))
    }
}

impl HdContainerDataSource for DataSourceCurvesPrim {
    fn get_names(&self) -> Vec<Token> {
        let mut names = self.gprim_ds.get_names();
        names.push(tokens::TOPOLOGY.clone());
        names
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        if *name == *tokens::TOPOLOGY {
            return Some(Arc::clone(&self.curves_ds) as HdDataSourceBaseHandle);
        }
        self.gprim_ds.get(name)
    }
}

// ============================================================================
// Base Curves Adapter
// ============================================================================

/// Base adapter for curve primitives.
#[derive(Debug, Clone)]
pub struct CurvesAdapter {
    curve_type: Token,
}

impl CurvesAdapter {
    /// Create a new curves adapter.
    pub fn new(curve_type: Token) -> Self {
        Self { curve_type }
    }
}

impl PrimAdapter for CurvesAdapter {
    fn get_imaging_subprims(&self, _prim: &Prim) -> Vec<Token> {
        vec![Token::new("")]
    }

    fn get_imaging_subprim_type(&self, _prim: &Prim, subprim: &Token) -> Token {
        if subprim.is_empty() {
            self.curve_type.clone()
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
            Some(DataSourceCurvesPrim::new(
                prim.path().clone(),
                prim.clone(),
                stage_globals.clone(),
                self.curve_type.clone(),
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
        DataSourceCurvesPrim::invalidate(prim, subprim, properties, invalidation_type)
    }
}

// ============================================================================
// Specific Curve Adapters
// ============================================================================

/// Adapter for UsdGeomBasisCurves prims.
///
/// BasisCurves support various curve types (linear, bezier, bspline, catmullRom)
/// and basis functions for smooth curve rendering.
#[derive(Debug, Clone)]
pub struct BasisCurvesAdapter {
    base: CurvesAdapter,
}

impl Default for BasisCurvesAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl BasisCurvesAdapter {
    /// Create a new basis curves adapter.
    pub fn new() -> Self {
        Self {
            base: CurvesAdapter::new(tokens::BASIS_CURVES.clone()),
        }
    }

    /// Check if a primvar is built-in for basis curves.
    pub fn is_builtin_primvar(primvar_name: &Token) -> bool {
        matches!(
            primvar_name.as_str(),
            "points"
                | "widths"
                | "normals"
                | "velocities"
                | "accelerations"
                | "displayColor"
                | "displayOpacity"
                | "pointSizeScale"
                | "screenSpaceWidths"
                | "minScreenSpaceWidths"
        )
    }
}

impl PrimAdapter for BasisCurvesAdapter {
    fn get_imaging_subprims(&self, prim: &Prim) -> Vec<Token> {
        self.base.get_imaging_subprims(prim)
    }

    fn get_imaging_subprim_type(&self, prim: &Prim, subprim: &Token) -> Token {
        self.base.get_imaging_subprim_type(prim, subprim)
    }

    fn get_imaging_subprim_data(
        &self,
        prim: &Prim,
        subprim: &Token,
        stage_globals: &DataSourceStageGlobalsHandle,
    ) -> Option<HdContainerDataSourceHandle> {
        if subprim.is_empty() {
            // BasisCurves has a dedicated `_ref`-style datasource module.
            // Route the adapter through it so topology/primvar schema layout
            // stays aligned with `UsdImagingDataSourceBasisCurvesPrim`.
            Some(Arc::new(
                canonical_basis_curves_data_source::DataSourceBasisCurvesPrim::new(
                    prim.path().clone(),
                    prim.clone(),
                    stage_globals.clone(),
                ),
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
        canonical_basis_curves_data_source::DataSourceBasisCurvesPrim::invalidate(
            prim,
            subprim,
            properties,
            invalidation_type,
        )
    }
}

/// Adapter for UsdGeomHermiteCurves prims.
///
/// HermiteCurves are cubic curves defined by positions and tangent vectors.
#[derive(Debug, Clone)]
pub struct HermiteCurvesAdapter {
    base: CurvesAdapter,
}

impl Default for HermiteCurvesAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl HermiteCurvesAdapter {
    /// Create a new hermite curves adapter.
    pub fn new() -> Self {
        Self {
            // OpenUSD does not populate a native Hydra "hermiteCurves" rprim.
            // `_ref` renders Hermite guides as linear basisCurves and ignores
            // tangents/weights for imaging. Keep the adapter token aligned with
            // the actual Hydra contract so scene-index and legacy delegate paths
            // agree on the prim type.
            base: CurvesAdapter::new(tokens::BASIS_CURVES.clone()),
        }
    }
}

impl PrimAdapter for HermiteCurvesAdapter {
    fn get_imaging_subprims(&self, prim: &Prim) -> Vec<Token> {
        self.base.get_imaging_subprims(prim)
    }

    fn get_imaging_subprim_type(&self, prim: &Prim, subprim: &Token) -> Token {
        self.base.get_imaging_subprim_type(prim, subprim)
    }

    fn get_imaging_subprim_data(
        &self,
        prim: &Prim,
        subprim: &Token,
        stage_globals: &DataSourceStageGlobalsHandle,
    ) -> Option<HdContainerDataSourceHandle> {
        if subprim.is_empty() {
            Some(Arc::new(
                canonical_hermite_curves_data_source::DataSourceHermiteCurvesPrim::new(
                    prim.path().clone(),
                    prim.clone(),
                    stage_globals.clone(),
                ),
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
        canonical_hermite_curves_data_source::DataSourceHermiteCurvesPrim::invalidate(
            prim,
            subprim,
            properties,
            invalidation_type,
        )
    }
}

/// Adapter for UsdGeomNurbsCurves prims.
///
/// NurbsCurves represent non-uniform rational B-spline curves.
#[derive(Debug, Clone)]
pub struct NurbsCurvesAdapter {
    base: CurvesAdapter,
}

impl Default for NurbsCurvesAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl NurbsCurvesAdapter {
    /// Create a new nurbs curves adapter.
    pub fn new() -> Self {
        Self {
            base: CurvesAdapter::new(tokens::NURBS_CURVES.clone()),
        }
    }
}

impl PrimAdapter for NurbsCurvesAdapter {
    fn get_imaging_subprims(&self, prim: &Prim) -> Vec<Token> {
        self.base.get_imaging_subprims(prim)
    }

    fn get_imaging_subprim_type(&self, prim: &Prim, subprim: &Token) -> Token {
        self.base.get_imaging_subprim_type(prim, subprim)
    }

    fn get_imaging_subprim_data(
        &self,
        prim: &Prim,
        subprim: &Token,
        stage_globals: &DataSourceStageGlobalsHandle,
    ) -> Option<HdContainerDataSourceHandle> {
        if subprim.is_empty() {
            Some(Arc::new(
                canonical_nurbs_curves_data_source::DataSourceNurbsCurvesPrim::new(
                    prim.path().clone(),
                    prim.clone(),
                    stage_globals.clone(),
                ),
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
        canonical_nurbs_curves_data_source::DataSourceNurbsCurvesPrim::invalidate(
            prim,
            subprim,
            properties,
            invalidation_type,
        )
    }
}

/// Adapter for UsdGeomNurbsPatch prims.
///
/// NurbsPatch represents non-uniform rational B-spline surfaces.
#[derive(Debug, Clone)]
pub struct NurbsPatchAdapter {
    base: CurvesAdapter,
}

impl Default for NurbsPatchAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl NurbsPatchAdapter {
    /// Create a new nurbs patch adapter.
    pub fn new() -> Self {
        Self {
            base: CurvesAdapter::new(tokens::NURBS_PATCH.clone()),
        }
    }
}

impl PrimAdapter for NurbsPatchAdapter {
    fn get_imaging_subprims(&self, prim: &Prim) -> Vec<Token> {
        self.base.get_imaging_subprims(prim)
    }

    fn get_imaging_subprim_type(&self, prim: &Prim, subprim: &Token) -> Token {
        self.base.get_imaging_subprim_type(prim, subprim)
    }

    fn get_imaging_subprim_data(
        &self,
        prim: &Prim,
        subprim: &Token,
        stage_globals: &DataSourceStageGlobalsHandle,
    ) -> Option<HdContainerDataSourceHandle> {
        if subprim.is_empty() {
            Some(Arc::new(
                canonical_nurbs_patch_data_source::DataSourceNurbsPatchPrim::new(
                    prim.path().clone(),
                    prim.clone(),
                    stage_globals.clone(),
                ),
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
        canonical_nurbs_patch_data_source::DataSourceNurbsPatchPrim::invalidate(
            prim,
            subprim,
            properties,
            invalidation_type,
        )
    }
}

// ============================================================================
// Factory functions
// ============================================================================

/// Factory for creating basis curves adapters.
pub fn create_basis_curves_adapter() -> Arc<dyn PrimAdapter> {
    Arc::new(BasisCurvesAdapter::new())
}

/// Factory for creating hermite curves adapters.
pub fn create_hermite_curves_adapter() -> Arc<dyn PrimAdapter> {
    Arc::new(HermiteCurvesAdapter::new())
}

/// Factory for creating nurbs curves adapters.
pub fn create_nurbs_curves_adapter() -> Arc<dyn PrimAdapter> {
    Arc::new(NurbsCurvesAdapter::new())
}

/// Factory for creating nurbs patch adapters.
pub fn create_nurbs_patch_adapter() -> Arc<dyn PrimAdapter> {
    Arc::new(NurbsPatchAdapter::new())
}

// ============================================================================
// Type aliases
// ============================================================================

/// Handle for BasisCurvesAdapter.
pub type BasisCurvesAdapterHandle = Arc<BasisCurvesAdapter>;
/// Handle for HermiteCurvesAdapter.
pub type HermiteCurvesAdapterHandle = Arc<HermiteCurvesAdapter>;
/// Handle for NurbsCurvesAdapter.
pub type NurbsCurvesAdapterHandle = Arc<NurbsCurvesAdapter>;
/// Handle for NurbsPatchAdapter.
pub type NurbsPatchAdapterHandle = Arc<NurbsPatchAdapter>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_source_stage_globals::NoOpStageGlobals;
    use usd_core::Stage;

    fn create_test_globals() -> DataSourceStageGlobalsHandle {
        Arc::new(NoOpStageGlobals::default())
    }

    #[test]
    fn test_basis_curves_adapter() {
        let adapter = BasisCurvesAdapter::new();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();

        let prim_type = adapter.get_imaging_subprim_type(&prim, &Token::new(""));
        assert_eq!(prim_type.as_str(), "basisCurves");
    }

    #[test]
    fn test_hermite_curves_adapter() {
        let adapter = HermiteCurvesAdapter::new();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();

        let prim_type = adapter.get_imaging_subprim_type(&prim, &Token::new(""));
        assert_eq!(prim_type.as_str(), "basisCurves");
    }

    #[test]
    fn test_nurbs_curves_adapter() {
        let adapter = NurbsCurvesAdapter::new();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();

        let prim_type = adapter.get_imaging_subprim_type(&prim, &Token::new(""));
        assert_eq!(prim_type.as_str(), "nurbsCurves");
    }

    #[test]
    fn test_nurbs_patch_adapter() {
        let adapter = NurbsPatchAdapter::new();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();

        let prim_type = adapter.get_imaging_subprim_type(&prim, &Token::new(""));
        assert_eq!(prim_type.as_str(), "nurbsPatch");
    }

    #[test]
    fn test_curves_subprims() {
        let adapter = BasisCurvesAdapter::new();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();

        let subprims = adapter.get_imaging_subprims(&prim);
        assert_eq!(subprims.len(), 1);
        assert!(subprims[0].is_empty());
    }

    #[test]
    fn test_curves_data_source() {
        let adapter = BasisCurvesAdapter::new();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let globals = create_test_globals();

        let ds = adapter.get_imaging_subprim_data(&prim, &Token::new(""), &globals);
        assert!(ds.is_some());
    }

    #[test]
    fn test_curves_invalidation() {
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let properties = vec![Token::new("curveVertexCounts"), Token::new("widths")];

        let locators = DataSourceCurvesPrim::invalidate(
            &prim,
            &Token::new(""),
            &properties,
            PropertyInvalidationType::PropertyChanged,
        );

        assert!(!locators.is_empty());
    }

    #[test]
    fn test_basis_curves_builtin_primvars() {
        assert!(BasisCurvesAdapter::is_builtin_primvar(&Token::new(
            "points"
        )));
        assert!(BasisCurvesAdapter::is_builtin_primvar(&Token::new(
            "widths"
        )));
        assert!(BasisCurvesAdapter::is_builtin_primvar(&Token::new(
            "normals"
        )));
        assert!(BasisCurvesAdapter::is_builtin_primvar(&Token::new(
            "pointSizeScale"
        )));
        assert!(BasisCurvesAdapter::is_builtin_primvar(&Token::new(
            "screenSpaceWidths"
        )));
        assert!(BasisCurvesAdapter::is_builtin_primvar(&Token::new(
            "minScreenSpaceWidths"
        )));
        assert!(!BasisCurvesAdapter::is_builtin_primvar(&Token::new(
            "custom"
        )));
    }

    #[test]
    fn test_all_curves_factories() {
        let _ = create_basis_curves_adapter();
        let _ = create_hermite_curves_adapter();
        let _ = create_nurbs_curves_adapter();
        let _ = create_nurbs_patch_adapter();
    }
}
