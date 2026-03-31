//! Mesh feature ID sets (EXT_mesh_features).
//!
//! What: Describes per-vertex or per-texture feature IDs for a mesh.
//! Why: Mirrors Draco `MeshFeatures` used by glTF EXT_mesh_features.
//! How: Stores label, counts, attribute/texture bindings, and property table link.
//! Where used: `Mesh` feature set management and texture material masks.

use crate::texture::texture::Texture;
use crate::texture::texture_map::{TextureMap, TextureMapType};

/// Feature ID set for EXT_mesh_features.
#[derive(Debug, Clone)]
pub struct MeshFeatures {
    label: String,
    feature_count: i32,
    null_feature_id: i32,
    attribute_index: i32,
    texture_map: TextureMap,
    texture_channels: Vec<i32>,
    property_table_index: i32,
}

impl MeshFeatures {
    /// Creates an empty mesh feature set.
    pub fn new() -> Self {
        Self {
            label: String::new(),
            feature_count: 0,
            null_feature_id: -1,
            attribute_index: -1,
            texture_map: TextureMap::new(),
            texture_channels: Vec::new(),
            property_table_index: -1,
        }
    }

    /// Copies all data from `src`.
    pub fn copy_from(&mut self, src: &MeshFeatures) {
        self.label = src.label.clone();
        self.feature_count = src.feature_count;
        self.null_feature_id = src.null_feature_id;
        self.attribute_index = src.attribute_index;
        self.texture_map.copy_from(&src.texture_map);
        self.texture_channels = src.texture_channels.clone();
        self.property_table_index = src.property_table_index;
    }

    /// Sets feature label.
    pub fn set_label(&mut self, label: &str) {
        self.label = label.to_string();
    }

    /// Returns feature label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Sets number of unique features.
    pub fn set_feature_count(&mut self, feature_count: i32) {
        self.feature_count = feature_count;
    }

    /// Returns number of unique features.
    pub fn feature_count(&self) -> i32 {
        self.feature_count
    }

    /// Sets null feature id.
    pub fn set_null_feature_id(&mut self, null_feature_id: i32) {
        self.null_feature_id = null_feature_id;
    }

    /// Returns null feature id.
    pub fn null_feature_id(&self) -> i32 {
        self.null_feature_id
    }

    /// Sets attribute index.
    pub fn set_attribute_index(&mut self, attribute_index: i32) {
        self.attribute_index = attribute_index;
    }

    /// Returns attribute index.
    pub fn attribute_index(&self) -> i32 {
        self.attribute_index
    }

    /// Copies texture map binding.
    pub fn set_texture_map(&mut self, texture_map: &TextureMap) {
        self.texture_map.copy_from(texture_map);
    }

    /// Sets texture map binding from texture pointer and tex coord index.
    pub fn set_texture_map_from_texture(&mut self, texture: *mut Texture, tex_coord_index: i32) {
        self.texture_map
            .set_properties_with_tex_coord(TextureMapType::Generic, tex_coord_index);
        self.texture_map.set_texture_ptr(texture);
    }

    /// Returns texture map binding (immutable).
    pub fn texture_map(&self) -> &TextureMap {
        &self.texture_map
    }

    /// Returns texture map binding (mutable).
    pub fn texture_map_mut(&mut self) -> &mut TextureMap {
        &mut self.texture_map
    }

    /// Sets texture channels.
    pub fn set_texture_channels(&mut self, texture_channels: Vec<i32>) {
        self.texture_channels = texture_channels;
    }

    /// Returns texture channels.
    pub fn texture_channels(&self) -> &Vec<i32> {
        &self.texture_channels
    }

    /// Returns mutable texture channels.
    pub fn texture_channels_mut(&mut self) -> &mut Vec<i32> {
        &mut self.texture_channels
    }

    /// Sets property table index.
    pub fn set_property_table_index(&mut self, property_table_index: i32) {
        self.property_table_index = property_table_index;
    }

    /// Returns property table index.
    pub fn property_table_index(&self) -> i32 {
        self.property_table_index
    }
}

impl Default for MeshFeatures {
    fn default() -> Self {
        Self::new()
    }
}
