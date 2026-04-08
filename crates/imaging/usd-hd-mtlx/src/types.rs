//! Core types for MaterialX integration with Hydra.

use std::collections::{BTreeMap, BTreeSet};
use usd_sdf::Path as SdfPath;

/// Texture name mapping between MaterialX and Hydra.
///
/// Maps MaterialX texture node names to their corresponding Hydra texture names.
pub type HdMtlxTextureMap = BTreeMap<String, BTreeSet<String>>;

/// MaterialX-Hydra texture and primvar information.
///
/// Stores the mapping between MaterialX and Hydra texture nodes,
/// as well as paths to texture and primvar nodes in the Hydra network.
#[derive(Debug, Clone, Default)]
pub struct HdMtlxTexturePrimvarData {
    /// MaterialX to Hydra texture name mapping.
    /// Key: MaterialX texture node name
    /// Value: Set of corresponding Hydra texture names
    pub mx_hd_texture_map: HdMtlxTextureMap,

    /// Paths to Hydra texture nodes in the material network.
    pub hd_texture_nodes: BTreeSet<SdfPath>,

    /// Paths to Hydra primvar nodes in the material network.
    pub hd_primvar_nodes: BTreeSet<SdfPath>,
}

impl HdMtlxTexturePrimvarData {
    /// Creates a new empty texture/primvar data structure.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a texture mapping between MaterialX and Hydra names.
    pub fn add_texture_mapping(&mut self, mx_name: String, hd_name: String) {
        self.mx_hd_texture_map
            .entry(mx_name)
            .or_default()
            .insert(hd_name);
    }

    /// Inserts a texture mapping entry (MX node name → input name).
    /// Mirrors C++ mxHdTextureMap[mxNodeName].insert(inputName).
    #[allow(non_snake_case)]
    pub fn mxHdTextureMap_insert(&mut self, mx_node_name: &str, input_name: &str) {
        self.mx_hd_texture_map
            .entry(mx_node_name.to_string())
            .or_default()
            .insert(input_name.to_string());
    }

    /// Adds a Hydra texture node path.
    pub fn add_texture_node(&mut self, path: SdfPath) {
        self.hd_texture_nodes.insert(path);
    }

    /// Adds a Hydra primvar node path.
    pub fn add_primvar_node(&mut self, path: SdfPath) {
        self.hd_primvar_nodes.insert(path);
    }

    /// Clears all stored data.
    pub fn clear(&mut self) {
        self.mx_hd_texture_map.clear();
        self.hd_texture_nodes.clear();
        self.hd_primvar_nodes.clear();
    }

    /// Returns true if there are any texture nodes.
    pub fn has_textures(&self) -> bool {
        !self.hd_texture_nodes.is_empty()
    }

    /// Returns true if there are any primvar nodes.
    pub fn has_primvars(&self) -> bool {
        !self.hd_primvar_nodes.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_texture_primvar_data_new() {
        let data = HdMtlxTexturePrimvarData::new();
        assert!(data.mx_hd_texture_map.is_empty());
        assert!(data.hd_texture_nodes.is_empty());
        assert!(data.hd_primvar_nodes.is_empty());
        assert!(!data.has_textures());
        assert!(!data.has_primvars());
    }

    #[test]
    fn test_add_texture_mapping() {
        let mut data = HdMtlxTexturePrimvarData::new();
        data.add_texture_mapping("mx_diffuse".to_string(), "hd_diffuse_tex".to_string());
        data.add_texture_mapping("mx_diffuse".to_string(), "hd_color_tex".to_string());

        assert_eq!(data.mx_hd_texture_map.len(), 1);
        let hd_names = data.mx_hd_texture_map.get("mx_diffuse").unwrap();
        assert_eq!(hd_names.len(), 2);
        assert!(hd_names.contains("hd_diffuse_tex"));
        assert!(hd_names.contains("hd_color_tex"));
    }

    #[test]
    fn test_add_nodes() {
        let mut data = HdMtlxTexturePrimvarData::new();

        let tex_path = SdfPath::from_string("/Material/Texture").unwrap();
        let primvar_path = SdfPath::from_string("/Material/Primvar").unwrap();

        data.add_texture_node(tex_path.clone());
        data.add_primvar_node(primvar_path.clone());

        assert!(data.has_textures());
        assert!(data.has_primvars());
        assert!(data.hd_texture_nodes.contains(&tex_path));
        assert!(data.hd_primvar_nodes.contains(&primvar_path));
    }

    #[test]
    fn test_clear() {
        let mut data = HdMtlxTexturePrimvarData::new();
        data.add_texture_mapping("mx_tex".to_string(), "hd_tex".to_string());
        data.add_texture_node(SdfPath::from_string("/Tex").unwrap());
        data.add_primvar_node(SdfPath::from_string("/Primvar").unwrap());

        data.clear();

        assert!(data.mx_hd_texture_map.is_empty());
        assert!(data.hd_texture_nodes.is_empty());
        assert!(data.hd_primvar_nodes.is_empty());
    }
}
