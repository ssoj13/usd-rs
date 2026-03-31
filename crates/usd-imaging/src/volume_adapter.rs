//! VolumeAdapter - Adapter for UsdVolVolume.
//!
//! Port of pxr/usdImaging/usdImaging/volumeAdapter.h/cpp
//!
//! Provides imaging support for volume prims, which reference
//! volume field assets for rendering volumetric effects.

use super::data_source_gprim::DataSourceGprim;
use super::data_source_stage_globals::DataSourceStageGlobalsHandle;
use super::prim_adapter::PrimAdapter;
use super::types::PropertyInvalidationType;
use std::sync::Arc;
use usd_core::Prim;
use usd_hd::data_source::HdRetainedTypedSampledDataSource;
use usd_hd::scene_delegate::{HdVolumeFieldDescriptor, HdVolumeFieldDescriptorVector};
use usd_hd::{
    HdContainerDataSource, HdContainerDataSourceHandle, HdDataSourceBase, HdDataSourceBaseHandle,
    HdDataSourceLocator, HdDataSourceLocatorSet,
};
use usd_sdf::Path;
use usd_tf::Token;
use usd_vol::Volume;

// Token constants
#[allow(dead_code)]
mod tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    // Prim types
    pub static VOLUME: LazyLock<Token> = LazyLock::new(|| Token::new("volume"));
    pub static FIELD: LazyLock<Token> = LazyLock::new(|| Token::new("field"));
    pub static FIELD_3D_ASSET: LazyLock<Token> = LazyLock::new(|| Token::new("field3dAsset"));
    pub static OPENVDB_ASSET: LazyLock<Token> = LazyLock::new(|| Token::new("openVDBAsset"));

    // Volume field attributes
    pub static FILE_PATH: LazyLock<Token> = LazyLock::new(|| Token::new("filePath"));
    pub static FIELD_NAME: LazyLock<Token> = LazyLock::new(|| Token::new("fieldName"));
    pub static FIELD_INDEX: LazyLock<Token> = LazyLock::new(|| Token::new("fieldIndex"));
    pub static FIELD_DATA_TYPE: LazyLock<Token> = LazyLock::new(|| Token::new("fieldDataType"));
    pub static VECTOR_DATA_ROLE_HINT: LazyLock<Token> =
        LazyLock::new(|| Token::new("vectorDataRoleHint"));

    // Locators
    pub static VOLUME_FIELD_BINDING: LazyLock<Token> =
        LazyLock::new(|| Token::new("volumeFieldBinding"));
}

// ============================================================================
// DataSourceVolume
// ============================================================================

/// Data source for volume field bindings.
///
/// Implements the container that maps field names to their target prim paths.
/// Corresponds to C++ `UsdImagingDataSourceVolumeFieldBindings`.
#[derive(Clone)]
pub struct DataSourceVolume {
    prim: Prim,
    #[allow(dead_code)]
    stage_globals: DataSourceStageGlobalsHandle,
}

impl std::fmt::Debug for DataSourceVolume {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataSourceVolume").finish()
    }
}

impl DataSourceVolume {
    /// Create new volume data source.
    pub fn new(prim: Prim, stage_globals: DataSourceStageGlobalsHandle) -> Arc<Self> {
        Arc::new(Self {
            prim,
            stage_globals,
        })
    }
}

impl HdDataSourceBase for DataSourceVolume {
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

impl HdContainerDataSource for DataSourceVolume {
    /// Returns all field names bound to this volume (from field:* relationships).
    fn get_names(&self) -> Vec<Token> {
        let volume = Volume::from_prim(&self.prim);
        volume.get_field_paths().into_keys().collect()
    }

    /// Returns the target path for the named field as a retained typed data source.
    ///
    /// Mirrors C++ `UsdImagingDataSourceVolumeFieldBindings::Get()`: calls
    /// `UsdVolVolume::GetFieldPath(name)` and wraps the result in a
    /// `HdRetainedTypedSampledDataSource<SdfPath>`.
    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        let volume = Volume::from_prim(&self.prim);
        let path = volume.get_field_path(name)?;
        // Wrap the SdfPath in a retained typed sampled data source so
        // HdVolumeFieldBindingSchema can extract it via get_typed_value().
        Some(HdRetainedTypedSampledDataSource::<Path>::new(path) as HdDataSourceBaseHandle)
    }
}

// ============================================================================
// DataSourceVolumePrim
// ============================================================================

/// Prim data source for volume prims.
#[derive(Clone)]
pub struct DataSourceVolumePrim {
    #[allow(dead_code)]
    scene_index_path: Path,
    gprim_ds: Arc<DataSourceGprim>,
    volume_ds: Arc<DataSourceVolume>,
}

impl std::fmt::Debug for DataSourceVolumePrim {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataSourceVolumePrim").finish()
    }
}

impl DataSourceVolumePrim {
    /// Create new volume prim data source.
    pub fn new(
        scene_index_path: Path,
        prim: Prim,
        stage_globals: DataSourceStageGlobalsHandle,
    ) -> Arc<Self> {
        let gprim_ds = DataSourceGprim::new(
            scene_index_path.clone(),
            prim.clone(),
            stage_globals.clone(),
        );
        let volume_ds = DataSourceVolume::new(prim, stage_globals);

        Arc::new(Self {
            scene_index_path,
            gprim_ds,
            volume_ds,
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
            // C++: any "field:*" relationship change invalidates the whole
            // volumeFieldBinding container — break after first match as C++ does.
            if prop_str.starts_with("field:") {
                locators.insert(HdDataSourceLocator::from_token(
                    tokens::VOLUME_FIELD_BINDING.clone(),
                ));
                break;
            }
        }

        locators
    }
}

impl HdDataSourceBase for DataSourceVolumePrim {
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

impl HdContainerDataSource for DataSourceVolumePrim {
    fn get_names(&self) -> Vec<Token> {
        // C++: UsdImagingDataSourceVolumePrim::GetNames() appends
        // HdVolumeFieldBindingSchema::GetSchemaToken() == "volumeFieldBinding"
        let mut names = self.gprim_ds.get_names();
        names.push(tokens::VOLUME_FIELD_BINDING.clone());
        names
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        // C++: checks HdVolumeFieldBindingSchema::GetSchemaToken(), not "volume"
        if *name == *tokens::VOLUME_FIELD_BINDING {
            return Some(Arc::clone(&self.volume_ds) as HdDataSourceBaseHandle);
        }
        self.gprim_ds.get(name)
    }
}

// ============================================================================
// VolumeAdapter
// ============================================================================

/// Adapter for UsdVolVolume prims.
///
/// Volumes reference volume field prims that contain asset paths
/// to volumetric data (OpenVDB, Field3D, etc.).
#[derive(Debug, Clone)]
pub struct VolumeAdapter;

impl Default for VolumeAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl VolumeAdapter {
    /// Create a new volume adapter.
    pub fn new() -> Self {
        Self
    }

    /// Gather field paths from a volume prim.
    ///
    /// Wraps the prim as a `UsdVolVolume` and calls `get_field_paths()`.
    /// Returns `None` when the field map is empty (prim is not a valid volume
    /// or has no field relationships).
    ///
    /// Mirrors C++ `UsdImagingVolumeAdapter::_GatherVolumeData()`.
    fn gather_volume_data(prim: &Prim) -> Option<std::collections::HashMap<Token, Path>> {
        let volume = Volume::from_prim(prim);
        let fields = volume.get_field_paths();
        if fields.is_empty() {
            None
        } else {
            Some(fields)
        }
    }

    /// Build `HdVolumeFieldDescriptorVector` for all fields on a volume prim.
    ///
    /// For each field relationship on the volume, resolves the field prim type
    /// by looking at the USD prim type token, then constructs an
    /// `HdVolumeFieldDescriptor` with the field name, prim type, and path.
    ///
    /// Mirrors C++ `UsdImagingVolumeAdapter::GetVolumeFieldDescriptors()`.
    pub fn get_volume_field_descriptors(
        &self,
        prim: &Prim,
        stage: &std::sync::Arc<usd_core::Stage>,
    ) -> HdVolumeFieldDescriptorVector {
        let mut descriptors = HdVolumeFieldDescriptorVector::new();

        let Some(field_map) = Self::gather_volume_data(prim) else {
            return descriptors;
        };

        // Sort entries by key for deterministic ordering (BTreeMap-like)
        let mut sorted: Vec<(Token, Path)> = field_map.into_iter().collect();
        sorted.sort_by(|a, b| a.0.as_str().cmp(b.0.as_str()));

        for (field_name, field_path) in sorted {
            let Some(field_prim) = stage.get_prim_at_path(&field_path) else {
                continue;
            };

            // Skip prims that are not valid volume fields (abstract or inactive)
            if !field_prim.is_valid() {
                continue;
            }

            // Use the USD prim type name as the field prim type token,
            // matching what C++ FieldAdapter::GetPrimTypeToken() returns.
            let field_prim_type = field_prim.type_name().clone();

            // XXX(UsdImagingPaths): Using the USD path directly as the cache
            // path — same note as C++ reference (instancing not yet handled).
            descriptors.push(HdVolumeFieldDescriptor::new(
                field_name,
                field_prim_type,
                field_path,
            ));
        }

        descriptors
    }
}

impl PrimAdapter for VolumeAdapter {
    fn get_imaging_subprims(&self, _prim: &Prim) -> Vec<Token> {
        vec![Token::new("")]
    }

    fn get_imaging_subprim_type(&self, _prim: &Prim, subprim: &Token) -> Token {
        if subprim.is_empty() {
            tokens::VOLUME.clone()
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
            Some(DataSourceVolumePrim::new(
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
        // C++: only the empty subprim (the rprim itself) carries volume data.
        // Non-empty subprims return an empty locator set.
        if !subprim.is_empty() {
            return HdDataSourceLocatorSet::empty();
        }
        DataSourceVolumePrim::invalidate(prim, subprim, properties, invalidation_type)
    }
}

// ============================================================================
// FieldAdapter
// ============================================================================

/// Data source for field parameters.
///
/// Corresponds to C++ `UsdImagingDataSourceField`.
#[derive(Clone)]
pub struct DataSourceField {
    prim: Prim,
    #[allow(dead_code)]
    stage_globals: DataSourceStageGlobalsHandle,
}

impl std::fmt::Debug for DataSourceField {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataSourceField").finish()
    }
}

impl DataSourceField {
    /// Create new field data source.
    pub fn new(prim: Prim, stage_globals: DataSourceStageGlobalsHandle) -> Arc<Self> {
        Arc::new(Self {
            prim,
            stage_globals,
        })
    }
}

impl HdDataSourceBase for DataSourceField {
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

impl HdContainerDataSource for DataSourceField {
    fn get_names(&self) -> Vec<Token> {
        vec![
            tokens::FILE_PATH.clone(),
            tokens::FIELD_NAME.clone(),
            tokens::FIELD_INDEX.clone(),
            tokens::FIELD_DATA_TYPE.clone(),
            tokens::VECTOR_DATA_ROLE_HINT.clone(),
        ]
    }

    /// Returns the value for the named field attribute as a retained data source.
    ///
    /// Reads filePath, fieldName, fieldIndex, fieldDataType, and
    /// vectorDataRoleHint attributes from the USD field prim at the default
    /// time. Mirrors C++ `UsdImagingDataSourceField::Get()`.
    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        let attr = self.prim.get_attribute(name.as_str())?;
        let value = attr.get(usd_sdf::TimeCode::default())?;

        // Convert VtValue -> HdDataSourceBaseHandle via retained sampled DS.
        // This matches C++ UsdImagingDataSourceAttributeNew() behaviour which
        // wraps every attribute value in a typed retained data source.
        use usd_hd::data_source::HdRetainedSampledDataSource;
        Some(HdRetainedSampledDataSource::new(value) as HdDataSourceBaseHandle)
    }
}

/// Prim data source for field prims.
#[derive(Clone)]
pub struct DataSourceFieldPrim {
    #[allow(dead_code)]
    scene_index_path: Path,
    field_ds: Arc<DataSourceField>,
}

impl std::fmt::Debug for DataSourceFieldPrim {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataSourceFieldPrim").finish()
    }
}

impl DataSourceFieldPrim {
    /// Create new field prim data source.
    pub fn new(
        scene_index_path: Path,
        prim: Prim,
        stage_globals: DataSourceStageGlobalsHandle,
    ) -> Arc<Self> {
        let field_ds = DataSourceField::new(prim, stage_globals);
        Arc::new(Self {
            scene_index_path,
            field_ds,
        })
    }

    /// Compute invalidation for property changes.
    pub fn invalidate(
        _prim: &Prim,
        _subprim: &Token,
        properties: &[Token],
        _invalidation_type: PropertyInvalidationType,
    ) -> HdDataSourceLocatorSet {
        let mut locators = HdDataSourceLocatorSet::empty();

        for prop in properties {
            let prop_str = prop.as_str();
            match prop_str {
                "filePath" | "fieldName" | "fieldIndex" | "fieldDataType"
                | "vectorDataRoleHint" => {
                    locators.insert(HdDataSourceLocator::from_tokens_2(
                        tokens::FIELD.clone(),
                        prop.clone(),
                    ));
                }
                _ => {}
            }
        }

        locators
    }
}

impl HdDataSourceBase for DataSourceFieldPrim {
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

impl HdContainerDataSource for DataSourceFieldPrim {
    fn get_names(&self) -> Vec<Token> {
        vec![tokens::FIELD.clone()]
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        if *name == *tokens::FIELD {
            return Some(Arc::clone(&self.field_ds) as HdDataSourceBaseHandle);
        }
        None
    }
}

/// Adapter for volume field prims (OpenVDB, Field3D assets).
#[derive(Debug, Clone)]
pub struct FieldAdapter;

impl Default for FieldAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl FieldAdapter {
    /// Create a new field adapter.
    pub fn new() -> Self {
        Self
    }
}

impl PrimAdapter for FieldAdapter {
    fn get_imaging_subprims(&self, _prim: &Prim) -> Vec<Token> {
        vec![Token::new("")]
    }

    fn get_imaging_subprim_type(&self, _prim: &Prim, subprim: &Token) -> Token {
        if subprim.is_empty() {
            tokens::FIELD.clone()
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
            Some(DataSourceFieldPrim::new(
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
        DataSourceFieldPrim::invalidate(prim, subprim, properties, invalidation_type)
    }
}

// ============================================================================
// Factory functions
// ============================================================================

/// Factory for creating volume adapters.
pub fn create_volume_adapter() -> Arc<dyn PrimAdapter> {
    Arc::new(VolumeAdapter::new())
}

/// Factory for creating field adapters.
pub fn create_field_adapter() -> Arc<dyn PrimAdapter> {
    Arc::new(FieldAdapter::new())
}

/// Handle for VolumeAdapter.
pub type VolumeAdapterHandle = Arc<VolumeAdapter>;
/// Handle for FieldAdapter.
pub type FieldAdapterHandle = Arc<FieldAdapter>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_source_stage_globals::NoOpStageGlobals;
    use usd_core::Stage;

    fn create_test_globals() -> DataSourceStageGlobalsHandle {
        Arc::new(NoOpStageGlobals::default())
    }

    #[test]
    fn test_volume_adapter() {
        let adapter = VolumeAdapter::new();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();

        let prim_type = adapter.get_imaging_subprim_type(&prim, &Token::new(""));
        assert_eq!(prim_type.as_str(), "volume");
    }

    #[test]
    fn test_field_adapter() {
        let adapter = FieldAdapter::new();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();

        let prim_type = adapter.get_imaging_subprim_type(&prim, &Token::new(""));
        assert_eq!(prim_type.as_str(), "field");
    }

    #[test]
    fn test_volume_subprims() {
        let adapter = VolumeAdapter::new();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();

        let subprims = adapter.get_imaging_subprims(&prim);
        assert_eq!(subprims.len(), 1);
        assert!(subprims[0].is_empty());
    }

    #[test]
    fn test_volume_data_source() {
        let adapter = VolumeAdapter::new();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let globals = create_test_globals();

        let ds = adapter.get_imaging_subprim_data(&prim, &Token::new(""), &globals);
        assert!(ds.is_some());
    }

    #[test]
    fn test_volume_invalidation() {
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let properties = vec![Token::new("field:density")];

        let locators = DataSourceVolumePrim::invalidate(
            &prim,
            &Token::new(""),
            &properties,
            PropertyInvalidationType::PropertyChanged,
        );

        assert!(!locators.is_empty());
    }

    #[test]
    fn test_field_invalidation() {
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let properties = vec![Token::new("filePath"), Token::new("fieldName")];

        let locators = DataSourceFieldPrim::invalidate(
            &prim,
            &Token::new(""),
            &properties,
            PropertyInvalidationType::PropertyChanged,
        );

        assert!(!locators.is_empty());
    }

    #[test]
    fn test_all_volume_factories() {
        let _ = create_volume_adapter();
        let _ = create_field_adapter();
    }
}
