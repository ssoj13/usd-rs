//! HdFlattenedDataSourceProviders - registry of all flattened providers in hd.
//!
//! Port of pxr/imaging/hd/flattenedDataSourceProviders.h
//!
//! Provides the default set of flattened data source providers for use
//! as inputArgs to HdFlatteningSceneIndex.

use crate::flattened_data_source_provider::HdFlattenedDataSourceProviderHandle;
use crate::flattened_overlay_data_source_provider::make_flattened_data_source_providers;
use usd_tf::Token;

/// Return all flattened data source providers implemented in hd.
///
/// Contains providers for xform, visibility, purpose, and primvars.
///
/// Port of `HdFlattenedDataSourceProviders()` from
/// pxr/imaging/hd/flattenedDataSourceProviders.h
pub fn hd_flattened_data_source_providers() -> Vec<(Token, HdFlattenedDataSourceProviderHandle)> {
    make_flattened_data_source_providers()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_providers() {
        let providers = hd_flattened_data_source_providers();
        assert!(providers.len() >= 4);
        let names: Vec<&str> = providers.iter().map(|(t, _)| t.as_str()).collect();
        assert!(names.contains(&"xform"));
        assert!(names.contains(&"visibility"));
        assert!(names.contains(&"purpose"));
        assert!(names.contains(&"primvars"));
    }
}
