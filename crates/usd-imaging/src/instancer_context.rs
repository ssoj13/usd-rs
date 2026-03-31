//! Context passed from instancer adapters to prototype adapters.

use super::prim_adapter::PrimAdapterHandle;
use usd_sdf::Path;
use usd_tf::Token;

/// Context passed by instancer prim adapters to prototype prim adapters.
///
/// When processing instances, the instancer adapter needs to communicate
/// information about the instancing context (material bindings, draw modes,
/// purpose) to the prototype adapters. This struct carries that context.
///
/// # Use Cases
///
/// - Point instancers passing material bindings to prototypes
/// - Native instances communicating draw mode overrides
/// - Purpose inheritance from instance to prototype
///
/// # Example
///
/// ```ignore
/// let context = InstancerContext {
///     instancer_cache_path: Path::from_string("/World/Instancer").unwrap(),
///     child_name: Token::new("prototype"),
///     instancer_material_usd_path: Path::from_string("/World/Materials/Mat1").unwrap(),
///     instance_draw_mode: Token::new("default"),
///     instance_inheritable_purpose: Token::new("render"),
///     instancer_adapter: instancer.clone(),
/// };
/// ```
#[derive(Clone)]
pub struct InstancerContext {
    /// Cache path of the instancer prim in the render index.
    pub instancer_cache_path: Path,

    /// Name of the child prim, typically used for prototypes.
    ///
    /// This is the name component identifying which prototype is being
    /// processed within the instancer's hierarchy.
    pub child_name: Token,

    /// USD path to the material bound to the instance prim being processed.
    ///
    /// This allows instances to have different material bindings than
    /// their prototypes. Empty path means no material override.
    pub instancer_material_usd_path: Path,

    /// Draw mode bound to the instance prim being processed.
    ///
    /// Valid values: "default", "bounds", "cards", "origin".
    /// This allows per-instance draw mode overrides.
    pub instance_draw_mode: Token,

    /// Inheritable purpose bound to the instance prim being processed.
    ///
    /// If the instance prim has a purpose, prototypes without an explicit
    /// or inherited purpose will inherit this purpose from the instance.
    /// Valid values: "default", "render", "proxy", "guide".
    pub instance_inheritable_purpose: Token,

    /// The instancer's prim adapter.
    ///
    /// This is the adapter that created this context. Useful when an adapter
    /// is needed but the default adapter may be overridden for instancing.
    pub instancer_adapter: PrimAdapterHandle,
}

impl InstancerContext {
    /// Create new instancer context with all fields.
    pub fn new(
        instancer_cache_path: Path,
        child_name: Token,
        instancer_material_usd_path: Path,
        instance_draw_mode: Token,
        instance_inheritable_purpose: Token,
        instancer_adapter: PrimAdapterHandle,
    ) -> Self {
        Self {
            instancer_cache_path,
            child_name,
            instancer_material_usd_path,
            instance_draw_mode,
            instance_inheritable_purpose,
            instancer_adapter,
        }
    }

    /// Create default instancer context with required fields.
    ///
    /// Material path, draw mode, and purpose will be set to defaults.
    pub fn with_defaults(
        instancer_cache_path: Path,
        child_name: Token,
        instancer_adapter: PrimAdapterHandle,
    ) -> Self {
        Self {
            instancer_cache_path,
            child_name,
            instancer_material_usd_path: Path::empty(),
            instance_draw_mode: Token::new("default"),
            instance_inheritable_purpose: Token::new("default"),
            instancer_adapter,
        }
    }

    /// Check if instance has material override.
    pub fn has_material_override(&self) -> bool {
        !self.instancer_material_usd_path.is_empty()
    }

    /// Check if instance has non-default draw mode.
    pub fn has_draw_mode_override(&self) -> bool {
        self.instance_draw_mode != "default"
    }

    /// Check if instance has non-default purpose.
    pub fn has_purpose_override(&self) -> bool {
        self.instance_inheritable_purpose != "default"
    }
}

impl Default for InstancerContext {
    fn default() -> Self {
        Self {
            instancer_cache_path: Path::empty(),
            child_name: Token::new(""),
            instancer_material_usd_path: Path::empty(),
            instance_draw_mode: Token::new("default"),
            instance_inheritable_purpose: Token::new("default"),
            instancer_adapter: std::sync::Arc::new(super::prim_adapter::NoOpAdapter::new(
                Token::new(""),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prim_adapter::NoOpAdapter;
    use std::sync::Arc;

    #[test]
    fn test_new_context() {
        let adapter = Arc::new(NoOpAdapter::new(Token::new("PointInstancer")));
        let context = InstancerContext::new(
            Path::from_string("/World/Instancer").unwrap(),
            Token::new("proto"),
            Path::from_string("/Materials/Mat1").unwrap(),
            Token::new("bounds"),
            Token::new("render"),
            adapter.clone(),
        );

        assert_eq!(context.instancer_cache_path.get_text(), "/World/Instancer");
        assert_eq!(context.child_name.as_str(), "proto");
        assert_eq!(
            context.instancer_material_usd_path.get_text(),
            "/Materials/Mat1"
        );
        assert_eq!(context.instance_draw_mode.as_str(), "bounds");
        assert_eq!(context.instance_inheritable_purpose.as_str(), "render");
    }

    #[test]
    fn test_with_defaults() {
        let adapter = Arc::new(NoOpAdapter::new(Token::new("PointInstancer")));
        let context = InstancerContext::with_defaults(
            Path::from_string("/World/Instancer").unwrap(),
            Token::new("proto"),
            adapter,
        );

        assert_eq!(context.child_name.as_str(), "proto");
        assert!(context.instancer_material_usd_path.is_empty());
        assert_eq!(context.instance_draw_mode.as_str(), "default");
        assert_eq!(context.instance_inheritable_purpose.as_str(), "default");
    }

    #[test]
    fn test_has_overrides() {
        let adapter = Arc::new(NoOpAdapter::new(Token::new("PointInstancer")));

        let context_defaults = InstancerContext::with_defaults(
            Path::from_string("/World/Instancer").unwrap(),
            Token::new("proto"),
            adapter.clone(),
        );

        assert!(!context_defaults.has_material_override());
        assert!(!context_defaults.has_draw_mode_override());
        assert!(!context_defaults.has_purpose_override());

        let context_overrides = InstancerContext::new(
            Path::from_string("/World/Instancer").unwrap(),
            Token::new("proto"),
            Path::from_string("/Materials/Mat1").unwrap(),
            Token::new("bounds"),
            Token::new("render"),
            adapter,
        );

        assert!(context_overrides.has_material_override());
        assert!(context_overrides.has_draw_mode_override());
        assert!(context_overrides.has_purpose_override());
    }

    #[test]
    fn test_default_context() {
        let context = InstancerContext::default();
        assert!(context.instancer_cache_path.is_empty());
        assert_eq!(context.child_name.as_str(), "");
        assert!(!context.has_material_override());
        assert!(!context.has_draw_mode_override());
        assert!(!context.has_purpose_override());
    }

    #[test]
    fn test_context_clone() {
        let adapter = Arc::new(NoOpAdapter::new(Token::new("PointInstancer")));
        let context = InstancerContext::with_defaults(
            Path::from_string("/World/Instancer").unwrap(),
            Token::new("proto"),
            adapter,
        );

        let cloned = context.clone();
        assert_eq!(context.instancer_cache_path, cloned.instancer_cache_path);
        assert_eq!(context.child_name, cloned.child_name);
    }
}
