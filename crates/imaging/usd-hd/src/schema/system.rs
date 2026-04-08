//! System schema for holding system-level data in scene indices.
//!
//! The HdSystemSchema specifies a container that will hold "system" data. Each
//! piece of system data is identified by a key within the container. A piece
//! of system data is evaluated at a given location by walking up the namespace
//! looking for a system container that contains the corresponding key.

use crate::data_source::{
    HdContainerDataSourceHandle, HdDataSourceBaseHandle, HdDataSourceLocator,
};
use crate::scene_index::HdSceneIndexHandle;
use once_cell::sync::Lazy;
use usd_sdf::Path as SdfPath;
use usd_tf::Token;

use super::HdSchema;

/// Tokens for system schema.
pub struct HdSystemSchemaTokens;

impl HdSystemSchemaTokens {}

/// Token for "system" field
pub static SYSTEM: Lazy<Token> = Lazy::new(|| Token::new("system"));

/// Schema for system-level data in scene indices.
///
/// The HdSystemSchema provides access to system data that is stored in
/// the scene hierarchy and can be queried by walking up the namespace.
///
/// System data is stored at "system" locator and contains named fields
/// for different system subsystems (e.g., "assetResolution" for hdar).
#[derive(Debug, Clone)]
pub struct HdSystemSchema {
    base: HdSchema,
}

impl HdSystemSchema {
    /// Creates a new system schema from a container data source.
    pub fn new(container: HdContainerDataSourceHandle) -> Self {
        Self {
            base: HdSchema::new(container),
        }
    }

    /// Creates an empty system schema.
    pub fn empty() -> Self {
        Self {
            base: HdSchema::empty(),
        }
    }

    /// Returns the underlying container data source.
    pub fn get_container(&self) -> Option<&HdContainerDataSourceHandle> {
        self.base.get_container()
    }

    /// Returns true if this schema is defined (has a non-null container).
    pub fn is_defined(&self) -> bool {
        self.base.is_defined()
    }

    /// Retrieves a system schema from a parent container.
    ///
    /// Looks for a container data source at the "system" token in the
    /// parent container and constructs a HdSystemSchema instance.
    ///
    /// Because the requested container may not exist, the result should be
    /// checked with is_defined() before use.
    pub fn get_from_parent(from_parent: &HdContainerDataSourceHandle) -> Self {
        use crate::data_source::cast_to_container;

        if let Some(system_ds) = from_parent.get(&SYSTEM) {
            // Try to cast to container
            if let Some(container) = cast_to_container(&system_ds) {
                return Self::new(container);
            }
        }
        Self::empty()
    }

    /// Evaluates a key at the given path by walking up the namespace.
    ///
    /// If the key is found, returns the data source and optionally sets
    /// found_at_path to the path where it was found.
    ///
    /// This operation is linear in the length of from_path.
    ///
    /// # Arguments
    ///
    /// * `input_scene` - The scene index to query
    /// * `from_path` - The path to start searching from
    /// * `key` - The key to look for in system containers
    ///
    /// # Returns
    ///
    /// Tuple of (data_source, path_where_found) or (None, None) if not found
    pub fn get_from_path(
        input_scene: &HdSceneIndexHandle,
        from_path: &SdfPath,
        key: &Token,
    ) -> (Option<HdDataSourceBaseHandle>, Option<SdfPath>) {
        let locator = HdDataSourceLocator::new(&[SYSTEM.clone(), key.clone()]);

        let mut curr_path = from_path.clone();
        loop {
            let scene = input_scene.read();
            let prim = scene.get_prim(&curr_path);
            if let Some(prim_ds) = prim.data_source {
                // Try to get data at locator
                if let Some(ds) = Self::get_at_locator(&prim_ds, &locator) {
                    return (Some(ds), Some(curr_path));
                }
            }

            // Walk up to parent
            if curr_path.is_absolute_root_path() {
                break;
            }
            curr_path = curr_path.get_parent_path();
        }

        (None, None)
    }

    /// Composes system containers by walking up the namespace.
    ///
    /// Walks up the prim hierarchy from from_path and composes any system
    /// containers encountered.
    ///
    /// # Arguments
    ///
    /// * `input_scene` - The scene index to query
    /// * `from_path` - The path to start searching from
    ///
    /// # Returns
    ///
    /// Tuple of (composed_container, last_path_found) or (None, None) if no
    /// system containers were found
    pub fn compose(
        input_scene: &HdSceneIndexHandle,
        from_path: &SdfPath,
    ) -> (Option<HdContainerDataSourceHandle>, Option<SdfPath>) {
        let mut system_containers: Vec<HdContainerDataSourceHandle> = Vec::new();
        let mut last_found = None;

        let mut curr_path = from_path.clone();
        loop {
            let scene = input_scene.read();
            let prim = scene.get_prim(&curr_path);
            if let Some(prim_ds) = prim.data_source {
                let system_schema = Self::get_from_parent(&prim_ds);
                if let Some(container) = system_schema.get_container() {
                    system_containers.push(container.clone());
                    last_found = Some(curr_path.clone());
                }
            }

            // Walk up to parent
            if curr_path.is_absolute_root_path() {
                break;
            }
            curr_path = curr_path.get_parent_path();
        }

        if system_containers.is_empty() {
            return (None, None);
        }

        // Overlay containers (first has priority)
        use crate::data_source::HdOverlayContainerDataSource;
        let composed = if system_containers.len() == 1 {
            system_containers.into_iter().next().unwrap()
        } else {
            HdOverlayContainerDataSource::new(system_containers)
        };
        (Some(composed), last_found)
    }

    /// Composes system data as a prim-level data source.
    ///
    /// Similar to compose() but returns a container suitable for overlaying
    /// onto a prim's data source with the "system" key.
    ///
    /// # Arguments
    ///
    /// * `input_scene` - The scene index to query
    /// * `from_path` - The path to start searching from
    ///
    /// # Returns
    ///
    /// Tuple of (container_with_system_key, last_path_found)
    pub fn compose_as_prim_ds(
        input_scene: &HdSceneIndexHandle,
        from_path: &SdfPath,
    ) -> (Option<HdContainerDataSourceHandle>, Option<SdfPath>) {
        let (system_ds, found_path) = Self::compose(input_scene, from_path);
        if let Some(system_container) = system_ds {
            // Wrap in a container with "system" key
            use crate::data_source::HdRetainedContainerDataSource;
            let wrapped = HdRetainedContainerDataSource::new_1(SYSTEM.clone(), system_container);
            (Some(wrapped), found_path)
        } else {
            (None, None)
        }
    }

    /// Returns the schema token ("system").
    pub fn get_schema_token() -> Token {
        SYSTEM.clone()
    }

    /// Returns the default locator for this schema.
    pub fn get_default_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[SYSTEM.clone()])
    }

    // Helper to get data source at a locator
    fn get_at_locator(
        container: &HdContainerDataSourceHandle,
        locator: &HdDataSourceLocator,
    ) -> Option<HdDataSourceBaseHandle> {
        use crate::data_source::cast_to_container;

        let elements = locator.elements();
        if elements.is_empty() {
            return None;
        }

        let mut current: HdDataSourceBaseHandle = container.clone_box();

        for element in elements {
            // Try to cast to container
            if let Some(cont) = cast_to_container(&current) {
                if let Some(child) = cont.get(element) {
                    current = child;
                } else {
                    return None;
                }
            } else {
                return None;
            }
        }

        Some(current)
    }
}

impl Default for HdSystemSchema {
    fn default() -> Self {
        Self::empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_source::HdRetainedContainerDataSource;

    #[test]
    fn test_empty_schema() {
        let schema = HdSystemSchema::empty();
        assert!(!schema.is_defined());
    }

    #[test]
    fn test_schema_with_container() {
        let container = HdRetainedContainerDataSource::new_empty();
        let schema = HdSystemSchema::new(container);
        assert!(schema.is_defined());
    }

    #[test]
    fn test_get_schema_token() {
        let token = HdSystemSchema::get_schema_token();
        assert_eq!(token.as_str(), "system");
    }

    #[test]
    fn test_get_default_locator() {
        let locator = HdSystemSchema::get_default_locator();
        assert_eq!(locator.elements().len(), 1);
        assert_eq!(locator.elements()[0].as_str(), "system");
    }
}
