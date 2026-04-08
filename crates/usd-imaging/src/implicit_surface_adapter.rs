//! Implicit Surface Adapters - Adapters for USD implicit geometry primitives.
//!
//! Port of pxr/usdImaging/usdImaging/{cube,sphere,cylinder,cone,capsule,plane}Adapter.h/cpp
//!
//! Provides imaging support for implicit surface primitives:
//! - Cube
//! - Sphere
//! - Cylinder
//! - Cone
//! - Capsule
//! - Plane

use super::data_source_attribute::DataSourceAttribute;
use super::data_source_gprim::DataSourceGprim;
use super::data_source_stage_globals::DataSourceStageGlobalsHandle;
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
use usd_vt::Value;

// Token constants for implicit surfaces
#[allow(dead_code)] // Tokens defined for future use
mod tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    // Prim types
    pub static CUBE: LazyLock<Token> = LazyLock::new(|| Token::new("cube"));
    pub static SPHERE: LazyLock<Token> = LazyLock::new(|| Token::new("sphere"));
    pub static CYLINDER: LazyLock<Token> = LazyLock::new(|| Token::new("cylinder"));
    pub static CONE: LazyLock<Token> = LazyLock::new(|| Token::new("cone"));
    pub static CAPSULE: LazyLock<Token> = LazyLock::new(|| Token::new("capsule"));
    pub static PLANE: LazyLock<Token> = LazyLock::new(|| Token::new("plane"));

    // Cube attributes
    pub static SIZE: LazyLock<Token> = LazyLock::new(|| Token::new("size"));

    // Sphere attributes
    pub static RADIUS: LazyLock<Token> = LazyLock::new(|| Token::new("radius"));

    // Cylinder/Cone/Capsule attributes
    pub static HEIGHT: LazyLock<Token> = LazyLock::new(|| Token::new("height"));
    pub static AXIS: LazyLock<Token> = LazyLock::new(|| Token::new("axis"));

    // Plane attributes
    pub static WIDTH: LazyLock<Token> = LazyLock::new(|| Token::new("width"));
    pub static LENGTH: LazyLock<Token> = LazyLock::new(|| Token::new("length"));
    pub static DO_DOUBLE_SIDED: LazyLock<Token> = LazyLock::new(|| Token::new("doubleSided"));

    // Locators
    pub static MESH: LazyLock<Token> = LazyLock::new(|| Token::new("mesh"));
    pub static PRIMVARS: LazyLock<Token> = LazyLock::new(|| Token::new("primvars"));
    pub static POINTS: LazyLock<Token> = LazyLock::new(|| Token::new("points"));
}

// ============================================================================
// DataSourceImplicitSurface
// ============================================================================

/// Data source for implicit surface parameters.
#[derive(Clone)]
pub struct DataSourceImplicitSurface {
    #[allow(dead_code)] // For future implicit surface attribute reading
    prim: Prim,
    #[allow(dead_code)] // For future time-sampled reading
    stage_globals: DataSourceStageGlobalsHandle,
    surface_type: Token,
}

impl std::fmt::Debug for DataSourceImplicitSurface {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataSourceImplicitSurface")
            .field("surface_type", &self.surface_type)
            .finish()
    }
}

impl DataSourceImplicitSurface {
    /// Create new implicit surface data source.
    pub fn new(
        prim: Prim,
        stage_globals: DataSourceStageGlobalsHandle,
        surface_type: Token,
    ) -> Arc<Self> {
        Arc::new(Self {
            prim,
            stage_globals,
            surface_type,
        })
    }

    /// Get type-specific parameter names.
    fn get_type_specific_params(&self) -> Vec<Token> {
        match self.surface_type.as_str() {
            "cube" => vec![tokens::SIZE.clone()],
            "sphere" => vec![tokens::RADIUS.clone()],
            "cylinder" | "cone" | "capsule" => vec![
                tokens::RADIUS.clone(),
                tokens::HEIGHT.clone(),
                tokens::AXIS.clone(),
            ],
            "plane" => vec![
                tokens::WIDTH.clone(),
                tokens::LENGTH.clone(),
                tokens::DO_DOUBLE_SIDED.clone(),
                tokens::AXIS.clone(),
            ],
            _ => vec![],
        }
    }
}

impl HdDataSourceBase for DataSourceImplicitSurface {
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

impl HdContainerDataSource for DataSourceImplicitSurface {
    fn get_names(&self) -> Vec<Token> {
        self.get_type_specific_params()
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        let attr = self.prim.get_attribute(name.as_str())?;
        if !attr.is_valid() {
            return None;
        }

        if *name == *tokens::AXIS {
            return Some(DataSourceAttribute::<Token>::new(
                attr,
                self.stage_globals.clone(),
                self.prim.path().clone(),
            ) as HdDataSourceBaseHandle);
        }

        if *name == *tokens::DO_DOUBLE_SIDED {
            return Some(DataSourceAttribute::<bool>::new(
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
// DataSourceImplicitSurfacePrim
// ============================================================================

/// Prim data source for implicit surface prims.
#[derive(Clone)]
pub struct DataSourceImplicitSurfacePrim {
    #[allow(dead_code)]
    scene_index_path: Path,
    gprim_ds: Arc<DataSourceGprim>,
    surface_ds: Arc<DataSourceImplicitSurface>,
    surface_type: Token,
}

impl std::fmt::Debug for DataSourceImplicitSurfacePrim {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataSourceImplicitSurfacePrim")
            .field("surface_type", &self.surface_type)
            .finish()
    }
}

impl DataSourceImplicitSurfacePrim {
    /// Create new implicit surface prim data source.
    pub fn new(
        scene_index_path: Path,
        prim: Prim,
        stage_globals: DataSourceStageGlobalsHandle,
        surface_type: Token,
    ) -> Arc<Self> {
        let gprim_ds = DataSourceGprim::new(
            scene_index_path.clone(),
            prim.clone(),
            stage_globals.clone(),
        );
        let surface_ds = DataSourceImplicitSurface::new(prim, stage_globals, surface_type.clone());

        Arc::new(Self {
            scene_index_path,
            gprim_ds,
            surface_ds,
            surface_type,
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
                // Size affects mesh geometry
                "size" | "radius" | "height" | "axis" | "width" | "length" => {
                    locators.insert(HdDataSourceLocator::from_tokens_2(
                        tokens::PRIMVARS.clone(),
                        tokens::POINTS.clone(),
                    ));
                    locators.insert(HdDataSourceLocator::from_tokens_2(
                        tokens::MESH.clone(),
                        Token::new("topology"),
                    ));
                }
                _ => {}
            }
        }

        locators
    }
}

impl HdDataSourceBase for DataSourceImplicitSurfacePrim {
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

impl HdContainerDataSource for DataSourceImplicitSurfacePrim {
    fn get_names(&self) -> Vec<Token> {
        let mut names = self.gprim_ds.get_names();
        names.push(self.surface_type.clone());
        names
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        if *name == self.surface_type {
            return Some(Arc::clone(&self.surface_ds) as HdDataSourceBaseHandle);
        }
        self.gprim_ds.get(name)
    }
}

// ============================================================================
// Base Implicit Surface Adapter
// ============================================================================

/// Base adapter for implicit surface primitives.
#[derive(Debug, Clone)]
pub struct ImplicitSurfaceAdapter {
    surface_type: Token,
}

impl ImplicitSurfaceAdapter {
    /// Create a new implicit surface adapter.
    pub fn new(surface_type: Token) -> Self {
        Self { surface_type }
    }
}

impl PrimAdapter for ImplicitSurfaceAdapter {
    fn get_imaging_subprims(&self, _prim: &Prim) -> Vec<Token> {
        vec![Token::new("")]
    }

    fn get_imaging_subprim_type(&self, _prim: &Prim, subprim: &Token) -> Token {
        if subprim.is_empty() {
            // Implicit surfaces are rendered as meshes in Hydra
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
            Some(DataSourceImplicitSurfacePrim::new(
                prim.path().clone(),
                prim.clone(),
                stage_globals.clone(),
                self.surface_type.clone(),
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
        DataSourceImplicitSurfacePrim::invalidate(prim, subprim, properties, invalidation_type)
    }
}

// ============================================================================
// Specific Adapters
// ============================================================================

/// Adapter for UsdGeomCube prims.
#[derive(Debug, Clone)]
pub struct CubeAdapter {
    base: ImplicitSurfaceAdapter,
}

impl Default for CubeAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl CubeAdapter {
    /// Create a new cube adapter.
    pub fn new() -> Self {
        Self {
            base: ImplicitSurfaceAdapter::new(tokens::CUBE.clone()),
        }
    }
}

impl PrimAdapter for CubeAdapter {
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
        self.base
            .get_imaging_subprim_data(prim, subprim, stage_globals)
    }

    fn invalidate_imaging_subprim(
        &self,
        prim: &Prim,
        subprim: &Token,
        properties: &[Token],
        invalidation_type: PropertyInvalidationType,
    ) -> HdDataSourceLocatorSet {
        self.base
            .invalidate_imaging_subprim(prim, subprim, properties, invalidation_type)
    }
}

/// Adapter for UsdGeomSphere prims.
#[derive(Debug, Clone)]
pub struct SphereAdapter {
    base: ImplicitSurfaceAdapter,
}

impl Default for SphereAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl SphereAdapter {
    /// Create a new sphere adapter.
    pub fn new() -> Self {
        Self {
            base: ImplicitSurfaceAdapter::new(tokens::SPHERE.clone()),
        }
    }

    /// Number of radial segments about the Z axis.
    pub const NUM_RADIAL: usize = 10;
    /// Number of divisions along the Z axis.
    pub const NUM_AXIAL: usize = 10;
}

impl PrimAdapter for SphereAdapter {
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
        self.base
            .get_imaging_subprim_data(prim, subprim, stage_globals)
    }

    fn invalidate_imaging_subprim(
        &self,
        prim: &Prim,
        subprim: &Token,
        properties: &[Token],
        invalidation_type: PropertyInvalidationType,
    ) -> HdDataSourceLocatorSet {
        self.base
            .invalidate_imaging_subprim(prim, subprim, properties, invalidation_type)
    }
}

/// Adapter for UsdGeomCylinder prims.
#[derive(Debug, Clone)]
pub struct CylinderAdapter {
    base: ImplicitSurfaceAdapter,
}

impl Default for CylinderAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl CylinderAdapter {
    /// Create a new cylinder adapter.
    pub fn new() -> Self {
        Self {
            base: ImplicitSurfaceAdapter::new(tokens::CYLINDER.clone()),
        }
    }
}

impl PrimAdapter for CylinderAdapter {
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
        self.base
            .get_imaging_subprim_data(prim, subprim, stage_globals)
    }

    fn invalidate_imaging_subprim(
        &self,
        prim: &Prim,
        subprim: &Token,
        properties: &[Token],
        invalidation_type: PropertyInvalidationType,
    ) -> HdDataSourceLocatorSet {
        self.base
            .invalidate_imaging_subprim(prim, subprim, properties, invalidation_type)
    }
}

/// Adapter for UsdGeomCone prims.
#[derive(Debug, Clone)]
pub struct ConeAdapter {
    base: ImplicitSurfaceAdapter,
}

impl Default for ConeAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl ConeAdapter {
    /// Create a new cone adapter.
    pub fn new() -> Self {
        Self {
            base: ImplicitSurfaceAdapter::new(tokens::CONE.clone()),
        }
    }
}

impl PrimAdapter for ConeAdapter {
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
        self.base
            .get_imaging_subprim_data(prim, subprim, stage_globals)
    }

    fn invalidate_imaging_subprim(
        &self,
        prim: &Prim,
        subprim: &Token,
        properties: &[Token],
        invalidation_type: PropertyInvalidationType,
    ) -> HdDataSourceLocatorSet {
        self.base
            .invalidate_imaging_subprim(prim, subprim, properties, invalidation_type)
    }
}

/// Adapter for UsdGeomCapsule prims.
#[derive(Debug, Clone)]
pub struct CapsuleAdapter {
    base: ImplicitSurfaceAdapter,
}

impl Default for CapsuleAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl CapsuleAdapter {
    /// Create a new capsule adapter.
    pub fn new() -> Self {
        Self {
            base: ImplicitSurfaceAdapter::new(tokens::CAPSULE.clone()),
        }
    }
}

impl PrimAdapter for CapsuleAdapter {
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
        self.base
            .get_imaging_subprim_data(prim, subprim, stage_globals)
    }

    fn invalidate_imaging_subprim(
        &self,
        prim: &Prim,
        subprim: &Token,
        properties: &[Token],
        invalidation_type: PropertyInvalidationType,
    ) -> HdDataSourceLocatorSet {
        self.base
            .invalidate_imaging_subprim(prim, subprim, properties, invalidation_type)
    }
}

/// Adapter for UsdGeomPlane prims.
#[derive(Debug, Clone)]
pub struct PlaneAdapter {
    base: ImplicitSurfaceAdapter,
}

impl Default for PlaneAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl PlaneAdapter {
    /// Create a new plane adapter.
    pub fn new() -> Self {
        Self {
            base: ImplicitSurfaceAdapter::new(tokens::PLANE.clone()),
        }
    }
}

impl PrimAdapter for PlaneAdapter {
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
        self.base
            .get_imaging_subprim_data(prim, subprim, stage_globals)
    }

    fn invalidate_imaging_subprim(
        &self,
        prim: &Prim,
        subprim: &Token,
        properties: &[Token],
        invalidation_type: PropertyInvalidationType,
    ) -> HdDataSourceLocatorSet {
        self.base
            .invalidate_imaging_subprim(prim, subprim, properties, invalidation_type)
    }
}

// ============================================================================
// Factory functions
// ============================================================================

/// Factory for creating cube adapters.
pub fn create_cube_adapter() -> Arc<dyn PrimAdapter> {
    Arc::new(CubeAdapter::new())
}

/// Factory for creating sphere adapters.
pub fn create_sphere_adapter() -> Arc<dyn PrimAdapter> {
    Arc::new(SphereAdapter::new())
}

/// Factory for creating cylinder adapters.
pub fn create_cylinder_adapter() -> Arc<dyn PrimAdapter> {
    Arc::new(CylinderAdapter::new())
}

/// Factory for creating cone adapters.
pub fn create_cone_adapter() -> Arc<dyn PrimAdapter> {
    Arc::new(ConeAdapter::new())
}

/// Factory for creating capsule adapters.
pub fn create_capsule_adapter() -> Arc<dyn PrimAdapter> {
    Arc::new(CapsuleAdapter::new())
}

/// Factory for creating plane adapters.
pub fn create_plane_adapter() -> Arc<dyn PrimAdapter> {
    Arc::new(PlaneAdapter::new())
}

// ============================================================================
// Type aliases
// ============================================================================

/// Handle for CubeAdapter.
pub type CubeAdapterHandle = Arc<CubeAdapter>;
/// Handle for SphereAdapter.
pub type SphereAdapterHandle = Arc<SphereAdapter>;
/// Handle for CylinderAdapter.
pub type CylinderAdapterHandle = Arc<CylinderAdapter>;
/// Handle for ConeAdapter.
pub type ConeAdapterHandle = Arc<ConeAdapter>;
/// Handle for CapsuleAdapter.
pub type CapsuleAdapterHandle = Arc<CapsuleAdapter>;
/// Handle for PlaneAdapter.
pub type PlaneAdapterHandle = Arc<PlaneAdapter>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_source_stage_globals::NoOpStageGlobals;
    use usd_core::Stage;

    fn create_test_globals() -> DataSourceStageGlobalsHandle {
        Arc::new(NoOpStageGlobals::default())
    }

    #[test]
    fn test_cube_adapter() {
        let adapter = CubeAdapter::new();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();

        // Implicit surfaces are rendered as meshes
        let prim_type = adapter.get_imaging_subprim_type(&prim, &Token::new(""));
        assert_eq!(prim_type.as_str(), "mesh");
    }

    #[test]
    fn test_sphere_adapter() {
        let adapter = SphereAdapter::new();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();

        let prim_type = adapter.get_imaging_subprim_type(&prim, &Token::new(""));
        assert_eq!(prim_type.as_str(), "mesh");
    }

    #[test]
    fn test_cylinder_adapter() {
        let adapter = CylinderAdapter::new();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();

        let prim_type = adapter.get_imaging_subprim_type(&prim, &Token::new(""));
        assert_eq!(prim_type.as_str(), "mesh");
    }

    #[test]
    fn test_cone_adapter() {
        let adapter = ConeAdapter::new();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();

        let prim_type = adapter.get_imaging_subprim_type(&prim, &Token::new(""));
        assert_eq!(prim_type.as_str(), "mesh");
    }

    #[test]
    fn test_capsule_adapter() {
        let adapter = CapsuleAdapter::new();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();

        let prim_type = adapter.get_imaging_subprim_type(&prim, &Token::new(""));
        assert_eq!(prim_type.as_str(), "mesh");
    }

    #[test]
    fn test_plane_adapter() {
        let adapter = PlaneAdapter::new();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();

        let prim_type = adapter.get_imaging_subprim_type(&prim, &Token::new(""));
        assert_eq!(prim_type.as_str(), "mesh");
    }

    #[test]
    fn test_implicit_surface_subprims() {
        let adapter = CubeAdapter::new();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();

        let subprims = adapter.get_imaging_subprims(&prim);
        assert_eq!(subprims.len(), 1);
        assert!(subprims[0].is_empty());
    }

    #[test]
    fn test_implicit_surface_data_source() {
        let adapter = SphereAdapter::new();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let globals = create_test_globals();

        let ds = adapter.get_imaging_subprim_data(&prim, &Token::new(""), &globals);
        assert!(ds.is_some());
    }

    #[test]
    fn test_implicit_surface_invalidation() {
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let properties = vec![Token::new("radius"), Token::new("height")];

        let locators = DataSourceImplicitSurfacePrim::invalidate(
            &prim,
            &Token::new(""),
            &properties,
            PropertyInvalidationType::PropertyChanged,
        );

        assert!(!locators.is_empty());
    }

    #[test]
    fn test_sphere_constants() {
        assert_eq!(SphereAdapter::NUM_RADIAL, 10);
        assert_eq!(SphereAdapter::NUM_AXIAL, 10);
    }

    #[test]
    fn test_all_implicit_factories() {
        let _ = create_cube_adapter();
        let _ = create_sphere_adapter();
        let _ = create_cylinder_adapter();
        let _ = create_cone_adapter();
        let _ = create_capsule_adapter();
        let _ = create_plane_adapter();
    }
}
