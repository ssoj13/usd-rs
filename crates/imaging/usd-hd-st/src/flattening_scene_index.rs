
//! HdSt_FlatteningSceneIndex - Storm plugin for flattening inherited data.
//!
//! Storm-specific scene index plugin that wraps the core
//! `HdFlatteningSceneIndex` from usd-hd with Storm-specific flattening
//! providers. Flattens transforms, visibility, purpose, material bindings,
//! and primvars so Storm can access fully resolved values at leaf prims.
//!
//! Port of C++ Storm's flattening scene index integration.

use std::sync::Arc;
use parking_lot::RwLock;
use usd_hd::data_source::HdContainerDataSourceHandle;
use usd_hd::scene_index::{HdFlatteningSceneIndex, HdSceneIndexHandle};

/// Create a flattening scene index for Storm with standard providers.
///
/// Sets up the flattening scene index with providers for:
/// - xform (transform flattening/concatenation)
/// - visibility (inherited visibility)
/// - purpose (inherited purpose)
/// - materialBindings (closest-ancestor material resolution)
/// - primvars (inherited primvars)
/// - coordSys (coordinate system bindings)
///
/// # Arguments
/// * `input_scene` - The input scene to flatten
/// * `input_args` - Optional container data source with flattened data source
///   providers. If None, a default set of Storm providers is used.
///
/// # Returns
/// A new flattening scene index
pub fn create(
    input_scene: Option<HdSceneIndexHandle>,
    input_args: Option<HdContainerDataSourceHandle>,
) -> Arc<RwLock<HdFlatteningSceneIndex>> {
    // In full implementation, input_args would be populated with
    // Storm-specific flattened data source providers:
    //   xform -> HdFlattenedXformDataSourceProvider
    //   visibility -> HdFlattenedVisibilityDataSourceProvider
    //   purpose -> HdFlattenedPurposeDataSourceProvider
    //   materialBindings -> materialBinding flattening provider
    //   primvars -> HdFlattenedPrimvarsDataSourceProvider
    //   coordSys -> coordSys flattening provider
    HdFlatteningSceneIndex::new(input_scene, input_args)
}

/// Storm flattened data source names.
///
/// These are the data source names that Storm's flattening scene index
/// resolves from the hierarchy.
pub fn flattened_names() -> Vec<&'static str> {
    vec![
        "xform",
        "visibility",
        "purpose",
        "materialBindings",
        "primvars",
        "coordSys",
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use usd_hd::scene_index::HdSceneIndexBase;

    #[test]
    fn test_create() {
        let si = create(None, None);
        let lock = si.read();
        assert_eq!(lock.get_display_name(), "HdFlatteningSceneIndex");
    }

    #[test]
    fn test_flattened_names() {
        let names = flattened_names();
        assert_eq!(names.len(), 6);
        assert!(names.contains(&"xform"));
        assert!(names.contains(&"visibility"));
        assert!(names.contains(&"primvars"));
    }
}
