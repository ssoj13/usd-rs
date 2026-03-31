//! Implementation factory — registry for ShaderNodeImpl creators.

use std::collections::HashMap;
use std::sync::Arc;

use super::shader_node_impl::ShaderNodeImpl;

/// Creator function for ShaderNodeImpl
pub type ShaderNodeImplCreator = Arc<dyn Fn() -> Box<dyn ShaderNodeImpl> + Send + Sync>;

/// Factory for creating ShaderNodeImpl by implementation element name.
#[derive(Default)]
pub struct ImplementationFactory {
    creators: HashMap<String, ShaderNodeImplCreator>,
}

impl ImplementationFactory {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a ShaderNodeImpl creator for the given implementation name.
    pub fn register(&mut self, name: impl Into<String>, creator: ShaderNodeImplCreator) {
        self.creators.insert(name.into(), creator);
    }

    /// Register for multiple names (e.g. aliases).
    pub fn register_multi(
        &mut self,
        names: impl IntoIterator<Item = impl Into<String>>,
        creator: ShaderNodeImplCreator,
    ) {
        for name in names {
            self.creators.insert(name.into(), Arc::clone(&creator));
        }
    }

    /// Check if an implementation is registered for the given name.
    pub fn is_registered(&self, name: &str) -> bool {
        self.creators.contains_key(name)
    }

    /// Create a ShaderNodeImpl for the given implementation name. Returns None if not registered.
    pub fn create(&self, name: &str) -> Option<Box<dyn ShaderNodeImpl>> {
        self.creators.get(name).map(|f| (f.as_ref())())
    }
}
