//! HdMtlx - MaterialX integration for Hydra.
//!
//! This module provides utilities for integrating MaterialX with Hydra's material system.
//! It enables conversion between Hydra material networks and MaterialX documents.
//!
//! # Overview
//!
//! MaterialX is an open standard for representing rich material and look-development
//! content in computer graphics applications. HdMtlx provides the bridge between
//! USD/Hydra materials and MaterialX documents.
//!
//! # Key Functionality
//!
//! - **Material Network Conversion**: Convert Hydra material networks to MaterialX documents
//! - **Texture and Primvar Tracking**: Track texture and primvar node usage
//! - **MaterialX Standard Library**: Access to MaterialX stdlib search paths
//! - **Debug Support**: Debug codes for MaterialX operations
//!
//! # Modules
//!
//! - [`types`] - Core data types for MaterialX/Hydra integration
//! - [`tokens`] - Standard tokens for MaterialX shader terminals
//! - [`debug_codes`] - Debug codes for MaterialX operations
//! - [`conversion`] - Conversion utilities (full implementation via mtlx-rs)
//! - [`network_interface`] - Minimal HdMaterialNetworkInterface trait
//!
//! # Example
//!
//! ```ignore
//! use usd_hd_mtlx::*;
//! use usd_sdf::Path;
//!
//! // Create texture/primvar data collector
//! let mut data = HdMtlxTexturePrimvarData::new();
//! data.add_texture_mapping("mx_diffuse".to_string(), "hd_baseColor".to_string());
//!
//! // Access shader terminal tokens
//! println!("Surface terminal: {}", tokens::SURFACE_SHADER_NAME.as_str());
//!
//! // Get search paths for MaterialX stdlib
//! let paths = get_search_paths();
//! ```

pub mod conversion;
pub mod debug_codes;
pub mod network_interface;
pub mod tokens;
pub mod types;

// Re-export commonly used types
pub use debug_codes::HdMtlxDebugCode;
pub use types::{HdMtlxTextureMap, HdMtlxTexturePrimvarData};

// Re-export network interface types
pub use network_interface::{HdMaterialNetworkInterface, InputConnection, NodeParamData};

// Re-export conversion functions
pub use conversion::{
    convert_to_string, create_mtlx_document_from_hd_network,
    create_mtlx_document_from_hd_network_interface, create_name_from_path, document_to_bytes,
    get_mx_terminal_name, get_mx_terminal_name_from_interface, get_node_def, get_node_def_name,
    get_search_paths, get_std_libraries,
};

#[cfg(test)]
mod tests {
    use super::*;
    use usd_sdf::Path as SdfPath;

    #[test]
    fn test_module_exports() {
        // Verify key types are accessible
        let _data = HdMtlxTexturePrimvarData::new();
        let _debug_code = HdMtlxDebugCode::Document;

        // Verify tokens are accessible
        let _surface = &*tokens::SURFACE_SHADER_NAME;
        let _displacement = &*tokens::DISPLACEMENT_SHADER_NAME;
    }

    #[test]
    fn test_conversion_functions() {
        // Test conversion utilities are accessible
        let paths = get_search_paths();
        // May or may not be empty depending on environment
        let _ = paths;

        let path = SdfPath::from_string("/Material/Shader").unwrap();
        let name = create_name_from_path(&path);
        assert!(!name.is_empty());

        let terminal = get_mx_terminal_name("surfaceshader");
        assert_eq!(terminal, "Surface");
    }

    #[test]
    fn test_texture_primvar_data_integration() {
        let mut data = HdMtlxTexturePrimvarData::new();

        data.add_texture_mapping("mx_tex".to_string(), "hd_tex".to_string());
        data.add_texture_node(SdfPath::from_string("/Tex").unwrap());
        data.add_primvar_node(SdfPath::from_string("/Primvar").unwrap());

        assert!(data.has_textures());
        assert!(data.has_primvars());
        assert_eq!(data.mx_hd_texture_map.len(), 1);
        assert_eq!(data.hd_texture_nodes.len(), 1);
        assert_eq!(data.hd_primvar_nodes.len(), 1);
    }
}
