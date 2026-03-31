//! Convenience API for working with SdfSite.
//!
//! Port of pxr/usd/sdf/siteUtils.h
//!
//! These functions provide a convenient way to access layer data
//! through a Site reference.

use crate::abstract_data::Value;
use crate::{PrimSpec, PropertySpec, Site};
use usd_tf::Token;

/// Gets the prim spec at the site's path.
pub fn get_prim_at_path(site: &Site) -> Option<PrimSpec> {
    site.layer.get_prim_at_path(&site.path)
}

/// Gets the property spec at the site's path.
pub fn get_property_at_path(site: &Site) -> Option<PropertySpec> {
    site.layer.get_property_at_path(&site.path)
}

/// Returns true if the site has the specified field.
pub fn has_field(site: &Site, field: &Token) -> bool {
    site.layer.has_field(&site.path, field)
}

/// Gets a field value from the site.
pub fn get_field(site: &Site, field: &Token) -> Option<Value> {
    site.layer.get_field(&site.path, field)
}

/// Gets a field value from the site, with a default.
pub fn get_field_or<T: Clone + 'static>(site: &Site, field: &Token, default: T) -> T {
    get_field(site, field)
        .and_then(|v| v.get::<T>().cloned())
        .unwrap_or(default)
}

/// Checks if the site is valid (has a layer and valid path).
pub fn is_valid_site(site: &Site) -> bool {
    site.layer.is_valid() && !site.path.is_empty()
}

/// Returns true if the site has a spec.
pub fn has_spec(site: &Site) -> bool {
    site.layer.has_spec(&site.path)
}

/// Lists all fields at the site.
pub fn list_fields(site: &Site) -> Vec<Token> {
    site.layer.list_fields(&site.path)
}

/// Gets the site's identifier (layer identifier + path).
pub fn get_site_identifier(site: &Site) -> String {
    if let Some(layer) = site.layer.upgrade() {
        format!("{}:{}", layer.identifier(), site.path.as_str())
    } else {
        format!("<no layer>:{}", site.path.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Layer, LayerHandle, Path};
    use std::sync::Arc;

    fn make_test_site() -> (Arc<Layer>, Site) {
        let layer = Layer::create_anonymous(Some("test"));
        let path = Path::from_string("/TestPrim").unwrap();
        let site = Site::new(LayerHandle::from_layer(&layer), path);
        (layer, site)
    }

    #[test]
    fn test_is_valid_site() {
        let (_layer, site) = make_test_site();
        assert!(is_valid_site(&site));

        let invalid = Site::empty();
        assert!(!is_valid_site(&invalid));
    }

    #[test]
    fn test_get_site_identifier() {
        let (_layer, site) = make_test_site();
        let id = get_site_identifier(&site);
        assert!(id.contains("/TestPrim"));
    }
}
