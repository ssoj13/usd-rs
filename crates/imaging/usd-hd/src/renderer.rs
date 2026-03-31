
//! HdRenderer - Hydra 2.0 renderer abstraction.
//!
//! Corresponds to pxr/imaging/hd/renderer.h and
//! pxr/imaging/hd/legacyRenderControlInterface.h.

use crate::aov::HdAovDescriptor;
use crate::command::{HdCommandArgs, HdCommandDescriptor};
use crate::render::HdRenderSettingDescriptor;
use crate::render_delegate_info::HdRenderDelegateInfo;
use std::collections::HashMap;
use usd_sdf::Path as SdfPath;
use usd_tf::Token;
use usd_vt::Value;

/// Transitory Hydra-1.0-like API for HdRenderer.
///
/// Port of C++ `HdLegacyRenderControlInterface` from
/// pxr/imaging/hd/legacyRenderControlInterface.h
pub trait HdLegacyRenderControlInterface: Send + Sync {
    // -- Task control --

    /// Execute render tasks at the given paths.
    fn execute(&mut self, _task_paths: &[SdfPath]) {}

    /// Returns true if all tasks have converged (finished rendering).
    fn are_tasks_converged(&self, _task_paths: &[SdfPath]) -> bool {
        true
    }

    /// Get named data from the task context.
    fn get_task_context_data(&self, _name: &Token) -> Option<Value> {
        None
    }

    /// Set named data in the task context.
    fn set_task_context_data(&mut self, _name: &Token, _data: Value) {}

    // -- AOVs --

    /// Get default AOV descriptor for a given AOV name.
    fn get_default_aov_descriptor(&self, _name: &Token) -> HdAovDescriptor {
        HdAovDescriptor::default()
    }

    // -- Render settings --

    /// Get list of supported render setting descriptors.
    fn get_render_setting_descriptors(&self) -> Vec<HdRenderSettingDescriptor> {
        Vec::new()
    }

    /// Get a named render setting value.
    fn get_render_setting(&self, _name: &Token) -> Option<Value> {
        None
    }

    /// Set a named render setting value.
    fn set_render_setting(&mut self, _name: &Token, _value: Value) {}

    // -- Commands --

    /// Get descriptors for available commands.
    fn get_command_descriptors(&self) -> Vec<HdCommandDescriptor> {
        Vec::new()
    }

    /// Invoke a command by name with optional args. Returns true on success.
    fn invoke_command(&mut self, _name: &Token, _args: &HdCommandArgs) -> bool {
        false
    }

    // -- Background rendering control --

    /// Whether pause/resume is supported.
    fn is_pause_supported(&self) -> bool {
        false
    }

    /// Pause rendering.
    fn pause(&mut self) -> bool {
        false
    }

    /// Resume rendering.
    fn resume(&mut self) -> bool {
        false
    }

    /// Whether stop/restart is supported.
    fn is_stop_supported(&self) -> bool {
        false
    }

    /// Stop rendering. If `blocking`, wait until fully stopped.
    fn stop(&mut self, _blocking: bool) -> bool {
        false
    }

    /// Restart rendering.
    fn restart(&mut self) -> bool {
        false
    }

    // -- Resolution information --

    /// Material binding purpose token (e.g. "full", "preview").
    fn get_material_binding_purpose(&self) -> Token {
        Token::new("full")
    }

    /// Render context tokens for material networks.
    fn get_material_render_contexts(&self) -> Vec<Token> {
        Vec::new()
    }

    /// Namespaces for render settings.
    fn get_render_settings_namespaces(&self) -> Vec<Token> {
        Vec::new()
    }

    /// Whether primvar filtering is needed.
    fn is_primvar_filtering_needed(&self) -> bool {
        true
    }

    /// Shader source types supported by the renderer.
    fn get_shader_source_types(&self) -> Vec<Token> {
        Vec::new()
    }

    /// Whether coordinate systems are supported (performance flag).
    fn is_coord_sys_supported(&self) -> bool {
        true
    }

    /// Get render delegate info (derived from other queries).
    fn get_render_delegate_info(&self) -> HdRenderDelegateInfo {
        HdRenderDelegateInfo::default()
    }

    // -- Misc --

    /// Whether Storm-specific tasks are required.
    fn requires_storm_tasks(&self) -> bool {
        false
    }

    /// Get render statistics as key-value dictionary.
    fn get_render_stats(&self) -> HashMap<String, Value> {
        HashMap::new()
    }

    /// Map a prim id (pick result) back to the rprim path.
    fn get_rprim_path_from_prim_id(&self, _prim_idx: i32) -> SdfPath {
        SdfPath::empty()
    }
}

/// A Hydra renderer (Hydra 2.0 API).
///
/// Typically constructed via HdRendererPlugin from a scene index.
/// Replaces HdRenderDelegate in the new architecture.
///
/// Corresponds to C++ `HdRenderer`.
pub trait HdRenderer: Send + Sync {
    /// Get legacy render control interface for Hydra 1.0 compatibility.
    fn get_legacy_render_control(&self) -> Option<&dyn HdLegacyRenderControlInterface> {
        None
    }
}
