
//! HdStTextureBinder - Texture binding utilities for Storm.
//!
//! Manages binding of texture handles to shader texture units.
//! Provides utilities for batch binding/unbinding and tracking of
//! currently bound textures.
//!
//! Port of pxr/imaging/hdSt/textureBinder.h

use super::texture_handle::HdStTextureHandleSharedPtr;
use std::collections::HashMap;
use usd_hgi::HgiTextureHandle;
use usd_tf::Token;

/// Texture binding entry.
///
/// Associates a texture unit with a texture handle and name.
#[derive(Debug, Clone)]
pub struct TextureBinding {
    /// Shader texture unit index
    unit: u32,

    /// Texture handle to bind
    handle: HdStTextureHandleSharedPtr,

    /// Name of texture in shader
    name: Token,
}

impl TextureBinding {
    /// Create a new texture binding.
    ///
    /// # Arguments
    /// * `unit` - Texture unit index (0-based)
    /// * `handle` - Texture handle to bind
    /// * `name` - Texture name in shader
    pub fn new(unit: u32, handle: HdStTextureHandleSharedPtr, name: Token) -> Self {
        Self { unit, handle, name }
    }

    /// Get texture unit.
    pub fn unit(&self) -> u32 {
        self.unit
    }

    /// Get texture handle.
    pub fn handle(&self) -> &HdStTextureHandleSharedPtr {
        &self.handle
    }

    /// Get texture name.
    pub fn name(&self) -> &Token {
        &self.name
    }

    /// Check if binding is valid (handle is valid).
    pub fn is_valid(&self) -> bool {
        self.handle.is_valid()
    }

    /// Get HGI texture handle if valid.
    pub fn hgi_texture(&self) -> Option<&HgiTextureHandle> {
        self.handle.hgi_texture()
    }
}

/// Texture binder for managing texture bindings.
///
/// Provides utilities for binding textures to shader units and tracking
/// currently bound textures. Ensures efficient binding by avoiding
/// redundant bind operations.
///
/// # Usage
///
/// ```ignore
/// use usd_hd_st::*;
///
/// let mut binder = HdStTextureBinder::new();
///
/// // Bind textures
/// binder.bind(0, diffuse_handle, Token::new("diffuseTexture"));
/// binder.bind(1, normal_handle, Token::new("normalTexture"));
///
/// // Commit all bindings
/// binder.bind_all();
///
/// // ... rendering ...
///
/// // Unbind all
/// binder.unbind_all();
/// ```
///
/// # Reference
/// Port of HdStTextureBinder from pxr/imaging/hdSt/textureBinder.h
#[derive(Debug, Clone)]
pub struct HdStTextureBinder {
    /// Current texture bindings by unit
    bindings: HashMap<u32, TextureBinding>,

    /// Bindings that need to be committed
    pending_bindings: Vec<TextureBinding>,
}

impl HdStTextureBinder {
    /// Create a new texture binder.
    pub fn new() -> Self {
        Self {
            bindings: HashMap::new(),
            pending_bindings: Vec::new(),
        }
    }

    /// Bind a texture to a specific unit.
    ///
    /// The binding is not applied immediately but queued for batch binding.
    ///
    /// # Arguments
    /// * `unit` - Texture unit index
    /// * `handle` - Texture handle to bind
    /// * `name` - Texture name in shader
    pub fn bind(&mut self, unit: u32, handle: HdStTextureHandleSharedPtr, name: Token) {
        let binding = TextureBinding::new(unit, handle, name);
        self.pending_bindings.push(binding);
    }

    /// Bind a texture with automatic unit assignment.
    ///
    /// Assigns the next available texture unit.
    ///
    /// # Returns
    /// The assigned texture unit
    pub fn bind_auto(&mut self, handle: HdStTextureHandleSharedPtr, name: Token) -> u32 {
        let unit = self.next_available_unit();
        self.bind(unit, handle, name);
        unit
    }

    /// Get the next available texture unit.
    fn next_available_unit(&self) -> u32 {
        let max_used = self.bindings.keys().copied().max().unwrap_or(0);

        let max_pending = self
            .pending_bindings
            .iter()
            .map(|b| b.unit())
            .max()
            .unwrap_or(0);

        max_used.max(max_pending) + 1
    }

    /// Unbind texture from a specific unit.
    pub fn unbind(&mut self, unit: u32) {
        self.bindings.remove(&unit);
    }

    /// Unbind all textures.
    pub fn unbind_all(&mut self) {
        self.bindings.clear();
        self.pending_bindings.clear();
    }

    /// Commit pending bindings.
    ///
    /// Applies all queued texture bindings. Validity of the underlying
    /// texture handle is checked at draw time, not at bind time — matching
    /// the C++ `HdSt_TextureBinder::BindResources` pattern where all named
    /// texture handles are processed regardless of loaded state.
    pub fn bind_all(&mut self) {
        for binding in self.pending_bindings.drain(..) {
            self.bindings.insert(binding.unit(), binding);
        }
    }

    /// Get currently bound texture at a unit.
    pub fn get_binding(&self, unit: u32) -> Option<&TextureBinding> {
        self.bindings.get(&unit)
    }

    /// Get all current bindings.
    pub fn bindings(&self) -> &HashMap<u32, TextureBinding> {
        &self.bindings
    }

    /// Get number of bound textures.
    pub fn num_bindings(&self) -> usize {
        self.bindings.len()
    }

    /// Check if a unit is bound.
    pub fn is_bound(&self, unit: u32) -> bool {
        self.bindings.contains_key(&unit)
    }

    /// Get all bound texture units (sorted).
    pub fn bound_units(&self) -> Vec<u32> {
        let mut units: Vec<u32> = self.bindings.keys().copied().collect();
        units.sort_unstable();
        units
    }

    /// Get binding by texture name.
    pub fn get_binding_by_name(&self, name: &Token) -> Option<&TextureBinding> {
        self.bindings.values().find(|b| b.name() == name)
    }

    /// Clear all bindings (both current and pending).
    pub fn clear(&mut self) {
        self.bindings.clear();
        self.pending_bindings.clear();
    }

    /// Get number of pending bindings.
    pub fn num_pending(&self) -> usize {
        self.pending_bindings.len()
    }

    /// Check if there are pending bindings.
    pub fn has_pending(&self) -> bool {
        !self.pending_bindings.is_empty()
    }
}

impl Default for HdStTextureBinder {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for constructing texture bindings.
///
/// Provides a fluent interface for building multiple texture bindings.
#[derive(Debug)]
pub struct TextureBinderBuilder {
    binder: HdStTextureBinder,
}

impl TextureBinderBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self {
            binder: HdStTextureBinder::new(),
        }
    }

    /// Add a texture binding.
    pub fn with_texture(
        mut self,
        unit: u32,
        handle: HdStTextureHandleSharedPtr,
        name: Token,
    ) -> Self {
        self.binder.bind(unit, handle, name);
        self
    }

    /// Add a texture binding with automatic unit assignment.
    pub fn with_texture_auto(mut self, handle: HdStTextureHandleSharedPtr, name: Token) -> Self {
        self.binder.bind_auto(handle, name);
        self
    }

    /// Build and return the configured binder.
    pub fn build(mut self) -> HdStTextureBinder {
        self.binder.bind_all();
        self.binder
    }
}

impl Default for TextureBinderBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::texture_handle::HdStTextureHandle;
    use crate::texture_identifier::HdStTextureIdentifier;
    use std::sync::Arc;
    use usd_sdf::AssetPath;

    fn create_test_handle(name: &str) -> HdStTextureHandleSharedPtr {
        let path = AssetPath::new(name);
        let id = HdStTextureIdentifier::from_path(path);
        let handle = HdStTextureHandle::with_defaults(id);
        Arc::new(handle)
    }

    #[test]
    fn test_texture_binding_creation() {
        let handle = create_test_handle("test.png");
        let binding = TextureBinding::new(0, handle, Token::new("diffuse"));

        assert_eq!(binding.unit(), 0);
        assert_eq!(binding.name().as_str(), "diffuse");
    }

    #[test]
    fn test_binder_basic_binding() {
        let mut binder = HdStTextureBinder::new();
        let handle = create_test_handle("test.png");

        binder.bind(0, handle, Token::new("diffuse"));
        assert_eq!(binder.num_pending(), 1);
        assert_eq!(binder.num_bindings(), 0);

        binder.bind_all();
        assert_eq!(binder.num_pending(), 0);
        assert_eq!(binder.num_bindings(), 1);
        assert!(binder.is_bound(0));
    }

    #[test]
    fn test_binder_multiple_bindings() {
        let mut binder = HdStTextureBinder::new();
        let diffuse = create_test_handle("diffuse.png");
        let normal = create_test_handle("normal.png");
        let specular = create_test_handle("specular.png");

        binder.bind(0, diffuse, Token::new("diffuse"));
        binder.bind(1, normal, Token::new("normal"));
        binder.bind(2, specular, Token::new("specular"));
        binder.bind_all();

        assert_eq!(binder.num_bindings(), 3);
        assert_eq!(binder.bound_units(), vec![0, 1, 2]);
    }

    #[test]
    fn test_binder_unbind() {
        let mut binder = HdStTextureBinder::new();
        let handle = create_test_handle("test.png");

        binder.bind(0, handle, Token::new("texture"));
        binder.bind_all();
        assert!(binder.is_bound(0));

        binder.unbind(0);
        assert!(!binder.is_bound(0));
        assert_eq!(binder.num_bindings(), 0);
    }

    #[test]
    fn test_binder_unbind_all() {
        let mut binder = HdStTextureBinder::new();
        let handle1 = create_test_handle("tex1.png");
        let handle2 = create_test_handle("tex2.png");

        binder.bind(0, handle1, Token::new("tex1"));
        binder.bind(1, handle2, Token::new("tex2"));
        binder.bind_all();

        assert_eq!(binder.num_bindings(), 2);

        binder.unbind_all();
        assert_eq!(binder.num_bindings(), 0);
    }

    #[test]
    fn test_binder_get_binding() {
        let mut binder = HdStTextureBinder::new();
        let handle = create_test_handle("test.png");

        binder.bind(5, handle, Token::new("myTexture"));
        binder.bind_all();

        let binding = binder.get_binding(5);
        assert!(binding.is_some());
        assert_eq!(binding.unwrap().name().as_str(), "myTexture");

        assert!(binder.get_binding(10).is_none());
    }

    #[test]
    fn test_binder_get_by_name() {
        let mut binder = HdStTextureBinder::new();
        let handle = create_test_handle("test.png");

        binder.bind(0, handle, Token::new("normalMap"));
        binder.bind_all();

        let binding = binder.get_binding_by_name(&Token::new("normalMap"));
        assert!(binding.is_some());
        assert_eq!(binding.unwrap().unit(), 0);
    }

    #[test]
    fn test_binder_auto_unit() {
        let mut binder = HdStTextureBinder::new();
        let handle1 = create_test_handle("tex1.png");
        let handle2 = create_test_handle("tex2.png");
        let handle3 = create_test_handle("tex3.png");

        let unit1 = binder.bind_auto(handle1, Token::new("tex1"));
        let unit2 = binder.bind_auto(handle2, Token::new("tex2"));
        let unit3 = binder.bind_auto(handle3, Token::new("tex3"));

        assert_eq!(unit1, 1);
        assert_eq!(unit2, 2);
        assert_eq!(unit3, 3);

        binder.bind_all();
        assert_eq!(binder.num_bindings(), 3);
    }

    #[test]
    fn test_builder() {
        let handle1 = create_test_handle("diffuse.png");
        let handle2 = create_test_handle("normal.png");

        let binder = TextureBinderBuilder::new()
            .with_texture(0, handle1, Token::new("diffuse"))
            .with_texture(1, handle2, Token::new("normal"))
            .build();

        assert_eq!(binder.num_bindings(), 2);
        assert!(binder.is_bound(0));
        assert!(binder.is_bound(1));
    }

    #[test]
    fn test_builder_auto() {
        let handle1 = create_test_handle("tex1.png");
        let handle2 = create_test_handle("tex2.png");

        let binder = TextureBinderBuilder::new()
            .with_texture_auto(handle1, Token::new("tex1"))
            .with_texture_auto(handle2, Token::new("tex2"))
            .build();

        assert_eq!(binder.num_bindings(), 2);
    }
}
