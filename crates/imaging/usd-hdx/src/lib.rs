
//! Hydra Extensions (HDX) - High-level rendering utilities
//!
//! Port of pxr/imaging/hdx
//!
//! HDX provides high-level rendering infrastructure built on top of Hydra (HD):
//!
//! # Core Components
//!
//! - **Task System** - Rendering task management
//!   - [`HdxTask`](task::HdxTask) - Base task class with HGI support
//!   - [`HdxTaskController`](task_controller::HdxTaskController) - High-level task orchestration
//!
//! - **Task Implementations**
//!   - [`HdxRenderTask`](render_task::HdxRenderTask) - Main geometry rendering
//!   - [`HdxRenderSetupTask`](render_setup_task::HdxRenderSetupTask) - Render state setup
//!   - [`HdxSimpleLightTask`](simple_light_task::HdxSimpleLightTask) - Basic lighting
//!   - [`HdxShadowTask`](shadow_task::HdxShadowTask) - Shadow map generation
//!   - [`HdxSelectionTask`](selection_task::HdxSelectionTask) - Selection highlighting
//!   - [`HdxPickTask`](pick_task::HdxPickTask) - GPU-based picking
//!   - [`HdxPresentTask`](present_task::HdxPresentTask) - Present to screen
//!   - [`HdxColorCorrectionTask`](color_correction_task::HdxColorCorrectionTask) - Color grading
//!   - [`HdxAovInputTask`](aov_input_task::HdxAovInputTask) - AOV input processing
//!   - [`HdxBoundingBoxTask`](bounding_box_task::HdxBoundingBoxTask) - Bounding box visualization
//!   - [`HdxColorizeSelectionTask`](colorize_selection_task::HdxColorizeSelectionTask) - Selection colorization
//!   - [`HdxSkydomeTask`](skydome_task::HdxSkydomeTask) - Skydome rendering
//!   - [`HdxVisualizeAovTask`](visualize_aov_task::HdxVisualizeAovTask) - AOV visualization
//!
//! - **Shader Utilities**
//!   - [`HdxEffectsShader`](effects_shader::HdxEffectsShader) - Base class for screen-space effects
//!   - [`HdxFullscreenShader`](fullscreen_shader::HdxFullscreenShader) - Fullscreen rendering utility
//!   - [`HdxHgiConversions`](hgi_conversions::HdxHgiConversions) - HGI type conversions
//!
//! - **Selection System**
//!   - [`HdxSelection`](selection_tracker::HdxSelection) - Selection state
//!   - [`HdxSelectionTracker`](selection_tracker::HdxSelectionTracker) - Thread-safe selection tracking
//!
//! - **Types and Tokens**
//!   - [`tokens`](tokens) - Standard identifiers
//!   - [`types`](types) - Common data types
//!
//! # Architecture
//!
//! HDX provides a task-based rendering pipeline:
//!
//! ```text
//! HdxTaskController
//!   └─> Creates and manages tasks:
//!       ├─> HdxSimpleLightTask (setup lights)
//!       ├─> HdxShadowTask (render shadows)
//!       ├─> HdxRenderSetupTask (configure state)
//!       ├─> HdxRenderTask (draw geometry)
//!       ├─> HdxAovInputTask (read AOV data)
//!       ├─> HdxSelectionTask (highlight selection)
//!       ├─> HdxColorizeSelectionTask (colorize selection)
//!       ├─> HdxColorCorrectionTask (apply grading)
//!       ├─> HdxVisualizeAovTask (visualize AOVs)
//!       ├─> HdxBoundingBoxTask (show bboxes)
//!       ├─> HdxSkydomeTask (render skydome)
//!       └─> HdxPresentTask (blit to screen)
//! ```
//!
//! Post-processing tasks use a deferred backend contract in this port. During
//! `HdEngine::execute()` they record request structs into `HdTaskContext`
//! instead of assuming real backend AOV textures already exist. The actual
//! replay happens later in `usd-imaging::gl::Engine`, after Storm has rendered
//! the frame and the engine has published concrete AOV handles.
//!
//! # Task Execution Model
//!
//! Tasks execute in three phases:
//!
//! 1. **Sync** - Pull changed data from scene delegates
//! 2. **Prepare** - Resolve bindings, create resources
//! 3. **Execute** - Perform rendering work
//!
//! # Example Usage
//!
//! ```ignore
//! use usd_hdx::*;
//! use usd_sdf::Path;
//!
//! // Example showing task controller API
//! // let controller = HdxTaskController::new(
//! //     Path::from_string("/TaskController").unwrap(),
//! //     true // GPU enabled
//! // );
//! // controller.set_camera_path(Path::from_string("/cameras/main").unwrap());
//! // controller.set_enable_shadows(true);
//! ```
//!
//! # Selection Usage
//!
//! ```ignore
//! use usd_hdx::selection_tracker::*;
//! use usd_sdf::Path;
//!
//! // Create selection tracker
//! let tracker = create_selection_tracker();
//!
//! // Add selections
//! tracker.select(Path::from_string("/World/Mesh1").unwrap());
//! tracker.select(Path::from_string("/World/Mesh2").unwrap());
//!
//! // Query selection
//! if tracker.contains(&Path::from_string("/World/Mesh1").unwrap()) {
//!     println!("Mesh1 is selected");
//! }
//!
//! // Get all selected paths
//! let selected = tracker.get_selected_paths();
//! ```
//!
//! # Implementation Status
//!
//! - [x] Core infrastructure (tokens, types, task base)
//! - [x] Task controller with full C++ API parity (HdxTaskController)
//! - [x] Task controller scene index (Hydra 2.0: HdxTaskControllerSceneIndex)
//! - [x] Selection tracking system
//! - [x] All core render/state tasks (render, setup, light, shadow, selection, pick)
//! - [x] Deferred post-task contracts (AOV input, present, color correction, colorize selection, visualize AOV)
//! - [x] OIT tasks (OIT render, OIT resolve, OIT volume render, OIT buffer accessor)
//! - [x] Draw target task (offscreen FBO rendering)
//! - [x] Free camera prim data source (Hydra 2.0 camera data)
//! - [x] Shader utilities (effects shader, fullscreen shader, HGI conversions)
//! - [x] Pick support (HdxPickHit, HdxPickResult, HdxPrimOriginInfo, HdxInstancerContext)
//! - [x] Engine-side replay path for sRGB/OpenColorIO color correction, selection colorize, and AOV visualization
//! - [~] Locate-highlight producer-side parity (task/compositor contract is wired; runtime producers still need validation)
//! - [ ] Advanced lighting and shadow algorithms
//! - [ ] Performance optimizations

pub mod selection_tracker;
pub mod task;
pub mod task_controller;
pub mod tokens;
pub mod types;

// Shader utilities
pub mod effects_shader;
pub mod fullscreen_shader;
pub mod hgi_conversions;

// Task implementations

// New platform-agnostic tasks and utilities
pub mod aov_input_task;
pub mod bounding_box_task;
pub mod color_channel_task;
pub mod color_correction_task;
pub mod colorize_selection_task;
pub mod draw_target_task;
pub mod free_camera_prim_data_source;
pub mod free_camera_scene_delegate;
pub mod oit_buffer_accessor;
pub mod oit_render_task;
pub mod oit_resolve_task;
pub mod oit_volume_render_task;
pub mod pick_from_render_buffer_task;
pub mod pick_task;
pub mod present_task;
pub mod render_setup_task;
pub mod render_task;
pub mod selection_scene_index_observer;
pub mod selection_task;
pub mod shadow_matrix_computation;
pub mod shadow_task;
pub mod simple_light_task;
pub mod skydome_task;
pub mod task_controller_scene_index;
pub mod visualize_aov_task;

// Re-exports
pub use selection_tracker::{
    HdxSelection, HdxSelectionTracker, SelectionTrackerExt, create_selection_tracker,
};
pub use task::{HdxTask, HdxTaskBase};
pub use task_controller::HdxTaskController;
pub use tokens::*;
pub use types::*;

// Shader utility re-exports
pub use effects_shader::HdxEffectsShader;
pub use fullscreen_shader::{
    FULLSCREEN_VERTEX_SHADER, FULLSCREEN_VERTEX_TECHNIQUE, HdxFullscreenShader,
};
pub use hgi_conversions::{HdFormat, HdxHgiConversions};

// Task re-exports
pub use aov_input_task::{
    HdxAovInputTask, HdxAovInputTaskParams, HdxAovInputTaskRequest, aov_input_tokens,
};
pub use bounding_box_task::{BBoxMode, HdxBoundingBoxTask, HdxBoundingBoxTaskParams, bbox_tokens};
pub use color_correction_task::{
    HdxColorCorrectionTask, HdxColorCorrectionTaskParams, HdxColorCorrectionTaskRequest,
    color_correction_tokens,
};
pub use colorize_selection_task::{
    ColorizeMode, HdxColorizeSelectionTask, HdxColorizeSelectionTaskParams,
    HdxColorizeSelectionTaskRequest, colorize_tokens,
};
pub use pick_task::{
    HdxPickHit, HdxPickResult, HdxPickTask, HdxPickTaskContextParams, HdxPickTaskParams,
    HdxPickTaskPass, HdxPickTaskRequest, pick_tokens,
};
pub use present_task::{HdxPresentTask, HdxPresentTaskParams, HdxPresentTaskRequest};
pub use render_setup_task::{HdxRenderSetupTask, HdxRenderTaskParams};
pub use render_task::{HdxRenderTask, HdxRenderTaskRequest};
pub use selection_task::{HdxSelectionTask, HdxSelectionTaskParams};
pub use shadow_task::{HdxShadowTask, HdxShadowTaskParams};
pub use simple_light_task::{HdxShadowParams, HdxSimpleLightTask, HdxSimpleLightTaskParams};
pub use skydome_task::{HdxSkydomeTask, HdxSkydomeTaskParams, SkydomeMode, skydome_tokens};
pub use visualize_aov_task::{
    AovVisMode, HdxVisualizeAovTask, HdxVisualizeAovTaskParams, HdxVisualizeAovTaskRequest,
    visualize_aov_tokens,
};

// OIT re-exports
pub use oit_buffer_accessor::{HdxOitBufferAccessor, OitBufferHandle, OitBufferType};
pub use oit_render_task::{HdxOitRenderTask, HdxOitRenderTaskParams};
pub use oit_resolve_task::{
    HdxOitResolveRenderPassState, HdxOitResolveTask, HdxOitResolveTaskParams,
};
pub use oit_volume_render_task::{HdxOitVolumeRenderTask, HdxOitVolumeRenderTaskParams};

// New module re-exports
pub use color_channel_task::{
    ColorChannel, HdxColorChannelTask, HdxColorChannelTaskParams, color_channel_tokens,
};
pub use draw_target_task::{HdxDrawTargetTask, HdxDrawTargetTaskParams};
pub use free_camera_prim_data_source::{DataSourceValue, HdxFreeCameraPrimDataSource};
pub use free_camera_scene_delegate::HdxFreeCameraSceneDelegate;
pub use pick_from_render_buffer_task::{
    HdxPickFromRenderBufferTask, HdxPickFromRenderBufferTaskParams,
};
pub use pick_task::{HdxInstancerContext, HdxPrimOriginInfo};
pub use selection_scene_index_observer::HdxSelectionSceneIndexObserver;
pub use shadow_matrix_computation::{HdxShadowMatrixComputation, HdxSimpleShadowMatrixComputation};
pub use task_controller_scene_index::{
    HdAovDescriptor, HdxTaskControllerSceneIndex, SceneIndexPrim, TaskControllerSceneIndexParams,
    hdx_is_storm,
};

#[cfg(test)]
mod tests {
    use super::*;
    use usd_sdf::Path;

    #[test]
    fn test_module_exports() {
        // Verify main types are accessible
        let _selection = HdxSelection::new();
        let _tracker = create_selection_tracker();

        // Verify task types
        let _render_task = HdxRenderTask::new(Path::from_string("/render").unwrap());
        let _setup_task = HdxRenderSetupTask::new(Path::from_string("/setup").unwrap());
        let _light_task = HdxSimpleLightTask::new(Path::from_string("/light").unwrap());
        let _shadow_task = HdxShadowTask::new(Path::from_string("/shadow").unwrap());
        let _selection_task = HdxSelectionTask::new(Path::from_string("/selection").unwrap());
        let _pick_task = HdxPickTask::new(Path::from_string("/pick").unwrap());
        let _present_task = HdxPresentTask::new(Path::from_string("/present").unwrap());
        let _color_task = HdxColorCorrectionTask::new(Path::from_string("/color").unwrap());
        let _aov_input_task = HdxAovInputTask::new(Path::from_string("/aovInput").unwrap());
        let _bbox_task = HdxBoundingBoxTask::new(Path::from_string("/bbox").unwrap());
        let _colorize_task = HdxColorizeSelectionTask::new(Path::from_string("/colorize").unwrap());
        let _skydome_task = HdxSkydomeTask::new(Path::from_string("/skydome").unwrap());
        let _visualize_task = HdxVisualizeAovTask::new(Path::from_string("/visualize").unwrap());
    }

    #[test]
    fn test_tokens_available() {
        // Verify tokens are accessible
        assert_eq!(RENDER_TASK.as_str(), "renderTask");
        assert_eq!(SHADOW_TASK.as_str(), "shadowTask");
        assert_eq!(SELECTION.as_str(), "selection");
        assert_eq!(AOV_INPUT_TASK.as_str(), "aovInputTask");
        assert_eq!(BOUNDING_BOX_TASK.as_str(), "boundingBoxTask");
        assert_eq!(COLORIZE_SELECTION_TASK.as_str(), "colorizeSelectionTask");
    }

    #[test]
    fn test_shader_inputs() {
        let inputs = HdxShaderInputs::new();
        assert!(inputs.is_empty());
    }

    #[test]
    fn test_format_conversion() {
        use usd_hgi::HgiFormat;
        use usd_hio::HioFormat;

        let hio_format = types::get_hio_format(HgiFormat::Float32Vec4);
        assert_eq!(hio_format, HioFormat::Float32Vec4);
    }

    #[test]
    fn test_new_task_params_defaults() {
        // Test default params for new tasks
        let aov_params = HdxAovInputTaskParams::default();
        assert_eq!(aov_params.aov_name.as_str(), "color");

        let bbox_params = HdxBoundingBoxTaskParams::default();
        assert_eq!(bbox_params.mode, BBoxMode::AxisAligned);

        let colorize_params = HdxColorizeSelectionTaskParams::default();
        assert_eq!(colorize_params.mode, ColorizeMode::Outline);

        let skydome_params = HdxSkydomeTaskParams::default();
        assert_eq!(skydome_params.mode, SkydomeMode::LatLong);

        let visualize_params = HdxVisualizeAovTaskParams::default();
        assert_eq!(visualize_params.mode, AovVisMode::Raw);
    }

    #[test]
    fn test_hgi_conversions() {
        use usd_hgi::HgiFormat;

        // HdFormat and HgiFormat are now distinct enums.
        // get_hgi_format(HdFormat) -> HgiFormat, get_hd_format(HgiFormat) -> HdFormat.
        let hd_format = HdFormat::Float32Vec4;
        let hgi_format = HdxHgiConversions::get_hgi_format(hd_format);
        assert_eq!(hgi_format, HgiFormat::Float32Vec4);
        let back_hd = HdxHgiConversions::get_hd_format(hgi_format);
        assert_eq!(back_hd, HdFormat::Float32Vec4);
    }

    #[test]
    fn test_fullscreen_shader_tokens() {
        assert_eq!(
            FULLSCREEN_VERTEX_SHADER.as_str(),
            "hdx/shaders/fullscreen.glslfx"
        );
        assert_eq!(FULLSCREEN_VERTEX_TECHNIQUE.as_str(), "FullScreenVertex");
    }
}
