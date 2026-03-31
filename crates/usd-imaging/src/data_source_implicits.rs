//! DataSourceImplicits - Implicit geometry data sources for Hydra.
//!
//! Port of pxr/usdImaging/usdImaging/dataSourceImplicits-Impl.h
//!
//! Provides a generic prim data source template for implicit geometry
//! types (Cube, Sphere, Cylinder, Cone, Capsule, Plane).

use crate::data_source_attribute::DataSourceAttribute;
use crate::data_source_gprim::DataSourceGprim;
use crate::data_source_stage_globals::DataSourceStageGlobalsHandle;
use crate::types::PropertyInvalidationType;
use std::sync::Arc;
use usd_core::Prim;
use usd_hd::{
    HdContainerDataSource, HdDataSourceBaseHandle, HdDataSourceLocator, HdDataSourceLocatorSet,
};
use usd_sdf::Path;
use usd_tf::Token;
use usd_vt::Value;

// Token constants
mod tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    pub static CUBE: LazyLock<Token> = LazyLock::new(|| Token::new("cube"));
    pub static SPHERE: LazyLock<Token> = LazyLock::new(|| Token::new("sphere"));
    pub static CYLINDER: LazyLock<Token> = LazyLock::new(|| Token::new("cylinder"));
    pub static CONE: LazyLock<Token> = LazyLock::new(|| Token::new("cone"));
    pub static CAPSULE: LazyLock<Token> = LazyLock::new(|| Token::new("capsule"));
    pub static PLANE: LazyLock<Token> = LazyLock::new(|| Token::new("plane"));
    pub static SIZE: LazyLock<Token> = LazyLock::new(|| Token::new("size"));
    pub static RADIUS: LazyLock<Token> = LazyLock::new(|| Token::new("radius"));
    pub static HEIGHT: LazyLock<Token> = LazyLock::new(|| Token::new("height"));
    pub static AXIS: LazyLock<Token> = LazyLock::new(|| Token::new("axis"));
    pub static WIDTH: LazyLock<Token> = LazyLock::new(|| Token::new("width"));
    pub static LENGTH: LazyLock<Token> = LazyLock::new(|| Token::new("length"));
    pub static DO_DOUBLE_SIDED: LazyLock<Token> = LazyLock::new(|| Token::new("doubleSided"));
    pub static RADIUS_TOP: LazyLock<Token> = LazyLock::new(|| Token::new("radiusTop"));
    pub static RADIUS_BOTTOM: LazyLock<Token> = LazyLock::new(|| Token::new("radiusBottom"));
}

// ============================================================================
// ImplicitGeometryType
// ============================================================================

/// Enumeration of implicit geometry types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImplicitGeometryType {
    /// Cube geometry.
    Cube,
    /// Sphere geometry.
    Sphere,
    /// Cylinder geometry.
    Cylinder,
    /// Cone geometry.
    Cone,
    /// Capsule geometry.
    Capsule,
    /// Plane geometry.
    Plane,
}

impl ImplicitGeometryType {
    /// Get the schema token for this geometry type.
    pub fn schema_token(&self) -> Token {
        match self {
            Self::Cube => tokens::CUBE.clone(),
            Self::Sphere => tokens::SPHERE.clone(),
            Self::Cylinder => tokens::CYLINDER.clone(),
            Self::Cone => tokens::CONE.clone(),
            Self::Capsule => tokens::CAPSULE.clone(),
            Self::Plane => tokens::PLANE.clone(),
        }
    }
}

// ============================================================================
// DataSourceImplicit
// ============================================================================

/// Container data source for implicit geometry parameters.
///
/// Reads geometry attributes (radius, height, size, axis, etc.) from the
/// USD prim and exposes them via DataSourceAttribute.
#[derive(Clone)]
pub struct DataSourceImplicit {
    prim: Prim,
    geometry_type: ImplicitGeometryType,
    stage_globals: DataSourceStageGlobalsHandle,
}

impl DataSourceImplicit {
    /// Create a new implicit geometry data source.
    pub fn new(
        prim: Prim,
        geometry_type: ImplicitGeometryType,
        stage_globals: DataSourceStageGlobalsHandle,
    ) -> Self {
        Self {
            prim,
            geometry_type,
            stage_globals,
        }
    }

    /// Get property names for this geometry type.
    fn get_property_names(&self) -> Vec<Token> {
        match self.geometry_type {
            ImplicitGeometryType::Cube => vec![tokens::SIZE.clone()],
            ImplicitGeometryType::Sphere => vec![tokens::RADIUS.clone()],
            ImplicitGeometryType::Cylinder => vec![
                tokens::RADIUS.clone(),
                tokens::HEIGHT.clone(),
                tokens::AXIS.clone(),
            ],
            ImplicitGeometryType::Cone => vec![
                tokens::RADIUS.clone(),
                tokens::HEIGHT.clone(),
                tokens::AXIS.clone(),
            ],
            ImplicitGeometryType::Capsule => vec![
                tokens::RADIUS.clone(),
                tokens::RADIUS_TOP.clone(),
                tokens::RADIUS_BOTTOM.clone(),
                tokens::HEIGHT.clone(),
                tokens::AXIS.clone(),
            ],
            ImplicitGeometryType::Plane => vec![
                tokens::WIDTH.clone(),
                tokens::LENGTH.clone(),
                tokens::AXIS.clone(),
                tokens::DO_DOUBLE_SIDED.clone(),
            ],
        }
    }
}

impl std::fmt::Debug for DataSourceImplicit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DataSourceImplicit")
    }
}

impl usd_hd::HdDataSourceBase for DataSourceImplicit {
    fn clone_box(&self) -> usd_hd::HdDataSourceBaseHandle {
        std::sync::Arc::new(self.clone())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<usd_hd::HdContainerDataSourceHandle> {
        Some(std::sync::Arc::new(self.clone()))
    }
}

impl HdContainerDataSource for DataSourceImplicit {
    fn get_names(&self) -> Vec<Token> {
        self.get_property_names()
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        // Only serve properties that belong to this geometry type
        let props = self.get_property_names();
        if !props.contains(name) {
            return None;
        }

        // Read the attribute from the prim and wrap it
        let attr = self.prim.get_attribute(name.as_str())?;
        Some(DataSourceAttribute::<Value>::new(
            attr,
            self.stage_globals.clone(),
            // Use prim path as scene index path for implicit geometry
            self.prim.get_path().clone(),
        ) as HdDataSourceBaseHandle)
    }
}

/// Handle type for DataSourceImplicit.
pub type DataSourceImplicitHandle = Arc<DataSourceImplicit>;

// ============================================================================
// DataSourceImplicitsPrim
// ============================================================================

/// Generic prim data source for implicit geometry types.
///
/// Extends DataSourceGprim with geometry-type-specific data.
#[derive(Clone)]
pub struct DataSourceImplicitsPrim {
    /// Base gprim data source
    base: Arc<DataSourceGprim>,
    /// The USD prim
    prim: Prim,
    /// The geometry type
    geometry_type: ImplicitGeometryType,
    /// Stage globals
    stage_globals: DataSourceStageGlobalsHandle,
}

impl DataSourceImplicitsPrim {
    /// Create a new implicits prim data source.
    pub fn new(
        scene_index_path: Path,
        prim: Prim,
        geometry_type: ImplicitGeometryType,
        stage_globals: DataSourceStageGlobalsHandle,
    ) -> Self {
        Self {
            base: DataSourceGprim::new(
                scene_index_path.clone(),
                prim.clone(),
                stage_globals.clone(),
            ),
            prim,
            geometry_type,
            stage_globals,
        }
    }

    /// Get the list of data source names.
    pub fn get_names(&self) -> Vec<Token> {
        let mut names = self.base.get_names();
        names.push(self.geometry_type.schema_token());
        names
    }

    /// Get a data source by name.
    pub fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        if name == &self.geometry_type.schema_token() {
            let implicit_ds = DataSourceImplicit::new(
                self.prim.clone(),
                self.geometry_type,
                self.stage_globals.clone(),
            );
            return Some(Arc::new(implicit_ds) as HdDataSourceBaseHandle);
        }
        self.base.get(name)
    }

    /// Invalidate data source for property changes.
    pub fn invalidate(
        prim: &Prim,
        subprim: &Token,
        properties: &[Token],
        invalidation_type: PropertyInvalidationType,
        geometry_type: ImplicitGeometryType,
    ) -> HdDataSourceLocatorSet {
        let mut locators =
            DataSourceGprim::invalidate(prim, subprim, properties, invalidation_type);

        // Check for geometry-specific properties
        let geom_props = match geometry_type {
            ImplicitGeometryType::Cube => vec!["size"],
            ImplicitGeometryType::Sphere => vec!["radius"],
            ImplicitGeometryType::Cylinder | ImplicitGeometryType::Cone => {
                vec!["radius", "height", "axis"]
            }
            ImplicitGeometryType::Capsule => {
                vec!["radius", "radiusTop", "radiusBottom", "height", "axis"]
            }
            ImplicitGeometryType::Plane => vec!["width", "length", "axis", "doubleSided"],
        };

        for prop in properties {
            let prop_str = prop.as_str();
            if geom_props.contains(&prop_str) {
                locators.insert(HdDataSourceLocator::from_token(
                    geometry_type.schema_token(),
                ));
                break;
            }
        }

        locators
    }
}

impl std::fmt::Debug for DataSourceImplicitsPrim {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DataSourceImplicitsPrim")
    }
}

/// Handle type for DataSourceImplicitsPrim.
pub type DataSourceImplicitsPrimHandle = Arc<DataSourceImplicitsPrim>;

/// Factory function for creating implicit geometry prim data sources.
pub fn create_data_source_implicits_prim(
    scene_index_path: Path,
    prim: Prim,
    geometry_type: ImplicitGeometryType,
    stage_globals: DataSourceStageGlobalsHandle,
) -> DataSourceImplicitsPrimHandle {
    Arc::new(DataSourceImplicitsPrim::new(
        scene_index_path,
        prim,
        geometry_type,
        stage_globals,
    ))
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
    fn test_geometry_type_tokens() {
        assert_eq!(ImplicitGeometryType::Cube.schema_token().as_str(), "cube");
        assert_eq!(
            ImplicitGeometryType::Sphere.schema_token().as_str(),
            "sphere"
        );
        assert_eq!(
            ImplicitGeometryType::Cylinder.schema_token().as_str(),
            "cylinder"
        );
    }

    #[test]
    fn test_cube_data_source() {
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let globals = create_test_globals();

        let ds = DataSourceImplicit::new(prim, ImplicitGeometryType::Cube, globals);

        let names = ds.get_names();
        assert!(names.iter().any(|n| n == "size"));
    }

    #[test]
    fn test_prim_names() {
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let globals = create_test_globals();

        let ds = DataSourceImplicitsPrim::new(
            Path::absolute_root(),
            prim,
            ImplicitGeometryType::Sphere,
            globals,
        );

        let names = ds.get_names();
        assert!(names.iter().any(|n| n == "sphere"));
    }
}
