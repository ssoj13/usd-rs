//! # RenderMan-specific imaging adapters.
//!
//! Port of `pxr/usdImaging/usdRiPxrImaging`.
//!
//! These adapters provide scene-index/data-source parity for RenderMan-authored
//! prims and API schemas even though usd-rs renders through wgpu/Storm. They do
//! not provide HdPrman execution, but they do preserve authored data,
//! dependencies, and invalidation behavior so RenderMan-facing scene structure
//! remains consistent with OpenUSD.
//!
//! # What is here
//!
//! - **Adapters**: `PxrCameraAPIAdapter`, `PxrCameraProjectionAdapter`,
//!   `PxrAovLightAdapter`, `PxrIntegratorAdapter`, `PxrDisplayFilterAdapter`,
//!   `PxrSampleFilterAdapter` -- all produce Hydra-facing data sources.
//! - **Data sources**: `DataSourceRenderTerminalPrim` for render-terminal
//!   resource payloads and parameter extraction.
//! - **Schema**: `ProjectionSchema` and camera namespaced-property overlays.
//! - **Tokens**: RenderMan imaging tokens preserved for compatibility.

pub mod data_source_render_terminal;
pub mod projection_schema;
pub mod pxr_camera_adapter;
pub mod pxr_camera_projection_adapter;
pub mod pxr_camera_projection_api_adapter;
pub mod pxr_display_filter_adapter;
pub mod pxr_integrator_adapter;
pub mod pxr_light_adapter;
pub mod pxr_sample_filter_adapter;
pub mod render_terminal_helper;
pub mod tokens;

// Re-export adapters
pub use data_source_render_terminal::{
    DataSourceDisplayFilterPrim, DataSourceIntegratorPrim, DataSourceRenderTerminalPrim,
    DataSourceRenderTerminalPrimHandle, DataSourceSampleFilterPrim,
};
pub use projection_schema::{ProjectionSchema, ProjectionSchemaBuilder};
pub use pxr_camera_adapter::PxrCameraAPIAdapter;
pub use pxr_camera_projection_adapter::PxrCameraProjectionAdapter;
pub use pxr_camera_projection_api_adapter::PxrCameraProjectionAPIAdapter;
pub use pxr_display_filter_adapter::PxrDisplayFilterAdapter;
pub use pxr_integrator_adapter::PxrIntegratorAdapter;
pub use pxr_light_adapter::PxrAovLightAdapter;
pub use pxr_sample_filter_adapter::PxrSampleFilterAdapter;
pub use render_terminal_helper::{HdMaterialConnection2, HdMaterialNode2, RenderTerminalHelper};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_exports() {
        // Test that all adapters can be created
        let _camera = PxrCameraAPIAdapter::new();
        let _light = PxrAovLightAdapter::new();
        let _integrator = PxrIntegratorAdapter::new();
        let _display_filter = PxrDisplayFilterAdapter::new();
        let _sample_filter = PxrSampleFilterAdapter::new();
    }

    #[test]
    fn test_tokens_accessible() {
        // Test that tokens are accessible
        let _ = tokens::tokens::info_source();
        let _ = tokens::tokens::usd_varying_extent();
        let _ = tokens::prim_type_tokens::projection();
    }
}
