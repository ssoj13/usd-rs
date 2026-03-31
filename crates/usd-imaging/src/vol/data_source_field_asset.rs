//! Data sources for volume field asset primitives.
//!
//! Provides container data sources for Field3DAsset and OpenVDBAsset prims,
//! exposing their attributes to Hydra's scene index system.
//!
//! # C++ Reference
//!
//! Port of `pxr/usdImaging/usdVolImaging/dataSourceFieldAsset.h`

use crate::data_source_attribute::DataSourceAttribute;
use crate::data_source_prim::DataSourcePrim;
use crate::data_source_stage_globals::DataSourceStageGlobalsHandle;
use std::sync::Arc;
use usd_core::Prim;
use usd_hd::data_source::{HdContainerDataSource, HdDataSourceBase, HdDataSourceBaseHandle};
use usd_sdf::Path;
use usd_tf::Token;
use usd_vt::Value;

/// Container data source representing volume field info.
///
/// Provides access to field asset attributes like filePath, fieldName,
/// fieldIndex, fieldClass, etc.
#[derive(Clone)]
pub struct DataSourceFieldAsset {
    scene_index_path: Path,
    usd_prim: Prim,
    /// Stage globals for time context
    stage_globals: DataSourceStageGlobalsHandle,
}

impl std::fmt::Debug for DataSourceFieldAsset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataSourceFieldAsset")
            .field("scene_index_path", &self.scene_index_path)
            .field("usd_prim", &self.usd_prim)
            .finish()
    }
}

impl DataSourceFieldAsset {
    /// Creates a new field asset data source.
    pub fn new(
        scene_index_path: Path,
        usd_prim: Prim,
        stage_globals: DataSourceStageGlobalsHandle,
    ) -> Arc<Self> {
        Arc::new(Self {
            scene_index_path,
            usd_prim,
            stage_globals,
        })
    }

    /// Returns attribute names based on field type.
    fn get_attribute_names(&self) -> Vec<Token> {
        // Common field asset attributes
        // In a complete implementation, this would be determined by the prim type
        vec![
            Token::new("filePath"),
            Token::new("fieldName"),
            Token::new("fieldIndex"),
            Token::new("fieldPurpose"),
            Token::new("fieldDataType"),
            Token::new("fieldClass"),
            Token::new("vectorDataRoleHint"),
        ]
    }
}

impl HdContainerDataSource for DataSourceFieldAsset {
    fn get_names(&self) -> Vec<Token> {
        self.get_attribute_names()
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        // Get attribute from USD prim and wrap as DataSourceAttribute
        if let Some(attr) = self.usd_prim.get_attribute(name.as_str()) {
            let ds = DataSourceAttribute::<Value>::new(
                attr,
                self.stage_globals.clone(),
                self.scene_index_path.clone(),
            );
            Some(ds as HdDataSourceBaseHandle)
        } else {
            None
        }
    }
}

/// Prim data source for Field3DAsset and OpenVDBAsset.
///
/// Extends base prim data source with volumeField container.
#[derive(Clone)]
pub struct DataSourceFieldAssetPrim {
    base: DataSourcePrim,
}

impl std::fmt::Debug for DataSourceFieldAssetPrim {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataSourceFieldAssetPrim")
            .field("hydra_path", self.base.hydra_path())
            .field("usd_prim", self.base.prim())
            .finish()
    }
}

impl DataSourceFieldAssetPrim {
    /// Creates a new field asset prim data source.
    pub fn new(
        scene_index_path: Path,
        usd_prim: Prim,
        stage_globals: DataSourceStageGlobalsHandle,
    ) -> Arc<Self> {
        Arc::new(Self {
            base: DataSourcePrim::new(usd_prim, scene_index_path, stage_globals),
        })
    }

    /// Returns locators to invalidate for property changes.
    pub fn invalidate(
        _prim: &Prim,
        _subprim: &Token,
        properties: &[Token],
        _invalidation_type: super::field_adapter::PropertyInvalidationType,
    ) -> super::field_adapter::DataSourceLocatorSet {
        let mut locators = Vec::new();

        // Check if any properties are field attributes
        let field_attr_names = vec![
            "filePath",
            "fieldName",
            "fieldIndex",
            "fieldPurpose",
            "fieldDataType",
            "fieldClass",
            "vectorDataRoleHint",
        ];

        for property in properties {
            if field_attr_names.contains(&property.as_str()) {
                locators.push(Token::new("volumeField"));
                break;
            }
        }

        locators
    }
}

impl HdContainerDataSource for DataSourceFieldAssetPrim {
    fn get_names(&self) -> Vec<Token> {
        let mut names = self.base.get_names();
        names.push(Token::new("volumeField"));
        names
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        if name == "volumeField" {
            let field_ds = DataSourceFieldAsset::new(
                self.base.hydra_path().clone(),
                self.base.prim().clone(),
                self.base.stage_globals().clone(),
            );
            Some(field_ds as HdDataSourceBaseHandle)
        } else {
            self.base.get(name)
        }
    }
}

// Implement HdDataSourceBase trait for both types
impl HdDataSourceBase for DataSourceFieldAsset {
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

impl HdDataSourceBase for DataSourceFieldAssetPrim {
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

/// Handle to field asset data source.
pub type DataSourceFieldAssetHandle = Arc<DataSourceFieldAsset>;

/// Handle to field asset prim data source.
pub type DataSourceFieldAssetPrimHandle = Arc<DataSourceFieldAssetPrim>;
