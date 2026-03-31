//! SdfLayerStateDelegateBase - maintains authoring state information for a layer.
//!
//! Port of pxr/usd/sdf/layerStateDelegate.h
//!
//! Layers rely on a state delegate to determine whether or not they have been
//! dirtied by authoring operations. A layer's state delegate is invoked on
//! every authoring operation on that layer.

use crate::{Layer, Path, SpecType};
use std::sync::{Arc, Weak};
use usd_tf::Token;
use usd_vt::Value;

/// Trait for layer state delegates.
///
/// Maintains authoring state information for an associated layer.
/// For example, layers rely on a state delegate to determine whether
/// or not they have been dirtied by authoring operations.
pub trait LayerStateDelegate: Send + Sync {
    /// Returns true if the layer has been modified.
    fn is_dirty(&self) -> bool;

    /// Marks the current state as clean.
    fn mark_current_state_as_clean(&mut self);

    /// Marks the current state as dirty.
    fn mark_current_state_as_dirty(&mut self);

    /// Called when the delegate is associated with a layer.
    fn on_set_layer(&mut self, layer: Option<Weak<Layer>>);

    /// Called when a field is set.
    ///
    /// C++ signature: `_OnSetField(path, fieldName, value)`.
    /// Returns the old value if the caller requested it.
    fn on_set_field(&mut self, path: &Path, field: &Token, value: &Value);

    /// Called when a field is set, returning the previous value.
    ///
    /// Matches the C++ `SetField(..., VtValue *oldValue)` overload.
    /// Default implementation delegates to `on_set_field` and returns None.
    fn on_set_field_returning_old(
        &mut self,
        path: &Path,
        field: &Token,
        value: &Value,
    ) -> Option<Value> {
        self.on_set_field(path, field, value);
        None
    }

    /// Called when a field dict value is set by key.
    fn on_set_field_dict_value_by_key(
        &mut self,
        path: &Path,
        field: &Token,
        key_path: &Token,
        value: &Value,
    );

    /// Called when a time sample is set.
    fn on_set_time_sample(&mut self, path: &Path, time: f64, value: &Value);

    /// Called when a time sample is erased.
    ///
    /// Default implementation delegates to `on_set_time_sample` with an
    /// empty value, matching the C++ pattern where erasing a time sample
    /// is equivalent to setting it to VtValue().
    fn on_erase_time_sample(&mut self, path: &Path, time: f64) {
        self.on_set_time_sample(path, time, &Value::default());
    }

    /// Called when a spec is created.
    fn on_create_spec(&mut self, path: &Path, spec_type: SpecType, inert: bool);

    /// Called when a spec is deleted.
    fn on_delete_spec(&mut self, path: &Path, inert: bool);

    /// Called when a spec is moved.
    fn on_move_spec(&mut self, old_path: &Path, new_path: &Path);

    /// Called when a child token is pushed.
    fn on_push_child_token(&mut self, parent_path: &Path, field: &Token, value: &Token);

    /// Called when a child path is pushed.
    fn on_push_child_path(&mut self, parent_path: &Path, field: &Token, value: &Path);

    /// Called when a child token is popped.
    fn on_pop_child_token(&mut self, parent_path: &Path, field: &Token, old_value: &Token);

    /// Called when a child path is popped.
    fn on_pop_child_path(&mut self, parent_path: &Path, field: &Token, old_value: &Path);
}

/// A simple layer state delegate that tracks dirty state.
///
/// Simply records whether any changes have been made to a layer.
#[derive(Debug, Default)]
pub struct SimpleLayerStateDelegate {
    /// Whether the layer is dirty.
    dirty: bool,
    /// Weak reference to the associated layer.
    layer: Option<Weak<Layer>>,
}

impl SimpleLayerStateDelegate {
    /// Creates a new simple layer state delegate.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new simple layer state delegate wrapped in Arc.
    pub fn new_arc() -> Arc<std::sync::RwLock<Self>> {
        Arc::new(std::sync::RwLock::new(Self::new()))
    }

    /// Returns the associated layer.
    pub fn layer(&self) -> Option<Arc<Layer>> {
        self.layer.as_ref().and_then(|w| w.upgrade())
    }
}

impl LayerStateDelegate for SimpleLayerStateDelegate {
    fn is_dirty(&self) -> bool {
        self.dirty
    }

    fn mark_current_state_as_clean(&mut self) {
        self.dirty = false;
    }

    fn mark_current_state_as_dirty(&mut self) {
        self.dirty = true;
    }

    fn on_set_layer(&mut self, layer: Option<Weak<Layer>>) {
        self.layer = layer;
        self.dirty = false;
    }

    fn on_set_field(&mut self, _path: &Path, _field: &Token, _value: &Value) {
        self.dirty = true;
    }

    fn on_set_field_dict_value_by_key(
        &mut self,
        _path: &Path,
        _field: &Token,
        _key_path: &Token,
        _value: &Value,
    ) {
        self.dirty = true;
    }

    fn on_set_time_sample(&mut self, _path: &Path, _time: f64, _value: &Value) {
        self.dirty = true;
    }

    fn on_create_spec(&mut self, _path: &Path, _spec_type: SpecType, _inert: bool) {
        self.dirty = true;
    }

    fn on_delete_spec(&mut self, _path: &Path, _inert: bool) {
        self.dirty = true;
    }

    fn on_move_spec(&mut self, _old_path: &Path, _new_path: &Path) {
        self.dirty = true;
    }

    fn on_push_child_token(&mut self, _parent_path: &Path, _field: &Token, _value: &Token) {
        self.dirty = true;
    }

    fn on_push_child_path(&mut self, _parent_path: &Path, _field: &Token, _value: &Path) {
        self.dirty = true;
    }

    fn on_pop_child_token(&mut self, _parent_path: &Path, _field: &Token, _old_value: &Token) {
        self.dirty = true;
    }

    fn on_pop_child_path(&mut self, _parent_path: &Path, _field: &Token, _old_value: &Path) {
        self.dirty = true;
    }
}

/// Null delegate that does nothing.
#[derive(Debug, Default)]
pub struct NullLayerStateDelegate;

impl NullLayerStateDelegate {
    /// Creates a new null delegate.
    pub fn new() -> Self {
        Self
    }
}

impl LayerStateDelegate for NullLayerStateDelegate {
    fn is_dirty(&self) -> bool {
        false
    }

    fn mark_current_state_as_clean(&mut self) {}
    fn mark_current_state_as_dirty(&mut self) {}
    fn on_set_layer(&mut self, _layer: Option<Weak<Layer>>) {}
    fn on_set_field(&mut self, _path: &Path, _field: &Token, _value: &Value) {}
    fn on_set_field_dict_value_by_key(
        &mut self,
        _path: &Path,
        _field: &Token,
        _key_path: &Token,
        _value: &Value,
    ) {
    }
    fn on_set_time_sample(&mut self, _path: &Path, _time: f64, _value: &Value) {}
    fn on_create_spec(&mut self, _path: &Path, _spec_type: SpecType, _inert: bool) {}
    fn on_delete_spec(&mut self, _path: &Path, _inert: bool) {}
    fn on_move_spec(&mut self, _old_path: &Path, _new_path: &Path) {}
    fn on_push_child_token(&mut self, _parent_path: &Path, _field: &Token, _value: &Token) {}
    fn on_push_child_path(&mut self, _parent_path: &Path, _field: &Token, _value: &Path) {}
    fn on_pop_child_token(&mut self, _parent_path: &Path, _field: &Token, _old_value: &Token) {}
    fn on_pop_child_path(&mut self, _parent_path: &Path, _field: &Token, _old_value: &Path) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_delegate_dirty() {
        let mut delegate = SimpleLayerStateDelegate::new();
        assert!(!delegate.is_dirty());

        delegate.on_set_field(&Path::empty(), &Token::empty(), &Value::default());
        assert!(delegate.is_dirty());

        delegate.mark_current_state_as_clean();
        assert!(!delegate.is_dirty());
    }

    #[test]
    fn test_null_delegate() {
        let mut delegate = NullLayerStateDelegate::new();
        assert!(!delegate.is_dirty());

        delegate.on_set_field(&Path::empty(), &Token::empty(), &Value::default());
        assert!(!delegate.is_dirty()); // Still not dirty
    }

    #[test]
    fn test_on_set_field_returning_old() {
        let mut delegate = SimpleLayerStateDelegate::new();
        // Default impl returns None
        let old =
            delegate.on_set_field_returning_old(&Path::empty(), &Token::empty(), &Value::default());
        assert!(old.is_none());
        assert!(delegate.is_dirty());
    }

    #[test]
    fn test_on_erase_time_sample() {
        let mut delegate = SimpleLayerStateDelegate::new();
        delegate.on_erase_time_sample(&Path::empty(), 1.0);
        assert!(delegate.is_dirty());
    }

    #[test]
    fn test_delegate_all_operations_dirty() {
        let mut d = SimpleLayerStateDelegate::new();
        let p = Path::empty();
        let t = Token::empty();
        let v = Value::default();

        d.on_create_spec(&p, SpecType::Prim, false);
        assert!(d.is_dirty());
        d.mark_current_state_as_clean();

        d.on_delete_spec(&p, false);
        assert!(d.is_dirty());
        d.mark_current_state_as_clean();

        d.on_move_spec(&p, &p);
        assert!(d.is_dirty());
        d.mark_current_state_as_clean();

        d.on_set_field_dict_value_by_key(&p, &t, &t, &v);
        assert!(d.is_dirty());
        d.mark_current_state_as_clean();

        d.on_push_child_token(&p, &t, &t);
        assert!(d.is_dirty());
        d.mark_current_state_as_clean();

        d.on_push_child_path(&p, &t, &p);
        assert!(d.is_dirty());
        d.mark_current_state_as_clean();

        d.on_pop_child_token(&p, &t, &t);
        assert!(d.is_dirty());
        d.mark_current_state_as_clean();

        d.on_pop_child_path(&p, &t, &p);
        assert!(d.is_dirty());
    }
}
