//! Texture map description.
//!
//! What: Describes how a texture is applied to a mesh.
//! Why: Mirrors Draco `TextureMap` used by materials and mesh features.
//! How: Stores mapping type, wrapping/filtering, UV set index, and texture.
//! Where used: `Material`, `MeshFeatures`, and glTF IO.

use std::ptr::NonNull;

use crate::texture::texture::Texture;
use crate::texture::texture_transform::TextureTransform;

/// Mapping type for a texture map (GLTF-aligned).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum TextureMapType {
    Generic = 0,
    Color = 1,
    Opacity = 2,
    Metallic = 3,
    Roughness = 4,
    MetallicRoughness = 5,
    NormalObjectSpace = 6,
    NormalTangentSpace = 7,
    AmbientOcclusion = 8,
    Emissive = 9,
    SheenColor = 10,
    SheenRoughness = 11,
    Transmission = 12,
    Clearcoat = 13,
    ClearcoatRoughness = 14,
    ClearcoatNormal = 15,
    Thickness = 16,
    Specular = 17,
    SpecularColor = 18,
    TextureTypesCount = 19,
}

/// Axis wrapping modes for texture coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum TextureMapAxisWrappingMode {
    ClampToEdge = 0,
    MirroredRepeat = 1,
    Repeat = 2,
}

/// Combined wrapping mode for S/T axes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextureMapWrappingMode {
    pub s: TextureMapAxisWrappingMode,
    pub t: TextureMapAxisWrappingMode,
}

impl TextureMapWrappingMode {
    pub fn new(mode: TextureMapAxisWrappingMode) -> Self {
        Self { s: mode, t: mode }
    }
}

/// Texture filtering modes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum TextureMapFilterType {
    Unspecified = 0,
    Nearest = 1,
    Linear = 2,
    NearestMipmapNearest = 3,
    LinearMipmapNearest = 4,
    NearestMipmapLinear = 5,
    LinearMipmapLinear = 6,
}

/// Mapping of a texture to mesh geometry.
#[derive(Clone, Debug)]
pub struct TextureMap {
    map_type: TextureMapType,
    wrapping_mode: TextureMapWrappingMode,
    tex_coord_index: i32,
    min_filter: TextureMapFilterType,
    mag_filter: TextureMapFilterType,
    owned_texture: Option<Box<Texture>>,
    texture_ptr: Option<NonNull<Texture>>,
    texture_transform: TextureTransform,
}

impl TextureMap {
    /// Creates an empty texture map.
    pub fn new() -> Self {
        Self {
            map_type: TextureMapType::Generic,
            wrapping_mode: TextureMapWrappingMode::new(TextureMapAxisWrappingMode::ClampToEdge),
            tex_coord_index: -1,
            min_filter: TextureMapFilterType::Unspecified,
            mag_filter: TextureMapFilterType::Unspecified,
            owned_texture: None,
            texture_ptr: None,
            texture_transform: TextureTransform::new(),
        }
    }

    /// Copies texture map data from `src`.
    pub fn copy_from(&mut self, src: &TextureMap) {
        self.map_type = src.map_type;
        self.wrapping_mode = src.wrapping_mode;
        self.tex_coord_index = src.tex_coord_index;
        self.min_filter = src.min_filter;
        self.mag_filter = src.mag_filter;
        if src.owned_texture.is_none() {
            self.owned_texture = None;
            self.texture_ptr = src.texture_ptr;
        } else {
            let mut new_texture = Box::new(Texture::new());
            new_texture.copy_from(src.owned_texture.as_ref().expect("texture"));
            let ptr = NonNull::from(new_texture.as_mut());
            self.owned_texture = Some(new_texture);
            self.texture_ptr = Some(ptr);
        }
        self.texture_transform.copy_from(&src.texture_transform);
    }

    /// Sets texture map properties with default wrapping and filters.
    pub fn set_properties(&mut self, map_type: TextureMapType) {
        self.set_properties_with_wrapping(
            map_type,
            TextureMapWrappingMode::new(TextureMapAxisWrappingMode::ClampToEdge),
            0,
        );
    }

    /// Sets texture map properties with default wrapping and filters.
    pub fn set_properties_with_tex_coord(
        &mut self,
        map_type: TextureMapType,
        tex_coord_index: i32,
    ) {
        self.set_properties_with_wrapping(
            map_type,
            TextureMapWrappingMode::new(TextureMapAxisWrappingMode::ClampToEdge),
            tex_coord_index,
        );
    }

    /// Sets texture map properties with wrapping mode.
    pub fn set_properties_with_wrapping(
        &mut self,
        map_type: TextureMapType,
        wrapping_mode: TextureMapWrappingMode,
        tex_coord_index: i32,
    ) {
        self.set_properties_full(
            map_type,
            wrapping_mode,
            tex_coord_index,
            TextureMapFilterType::Unspecified,
            TextureMapFilterType::Unspecified,
        );
    }

    /// Sets texture map properties with wrapping and filtering.
    pub fn set_properties_full(
        &mut self,
        map_type: TextureMapType,
        wrapping_mode: TextureMapWrappingMode,
        tex_coord_index: i32,
        min_filter: TextureMapFilterType,
        mag_filter: TextureMapFilterType,
    ) {
        self.map_type = map_type;
        self.wrapping_mode = wrapping_mode;
        self.tex_coord_index = tex_coord_index;
        self.min_filter = min_filter;
        self.mag_filter = mag_filter;
    }

    /// Sets an owned texture (transfers ownership).
    pub fn set_texture_owned(&mut self, texture: Box<Texture>) {
        let mut texture = texture;
        let ptr = NonNull::from(texture.as_mut());
        self.owned_texture = Some(texture);
        self.texture_ptr = Some(ptr);
    }

    /// Sets a non-owned texture pointer.
    pub fn set_texture_ptr(&mut self, texture: *mut Texture) {
        self.owned_texture = None;
        self.texture_ptr = NonNull::new(texture);
    }

    /// Sets a texture transform.
    pub fn set_transform(&mut self, transform: &TextureTransform) {
        self.texture_transform.copy_from(transform);
    }

    pub fn texture_transform(&self) -> &TextureTransform {
        &self.texture_transform
    }

    pub fn texture(&self) -> Option<&Texture> {
        self.texture_ptr.map(|ptr| unsafe { ptr.as_ref() })
    }

    pub fn texture_mut(&mut self) -> Option<&mut Texture> {
        self.texture_ptr.map(|mut ptr| unsafe { ptr.as_mut() })
    }

    pub fn map_type(&self) -> TextureMapType {
        self.map_type
    }

    pub fn wrapping_mode(&self) -> TextureMapWrappingMode {
        self.wrapping_mode
    }

    pub fn tex_coord_index(&self) -> i32 {
        self.tex_coord_index
    }

    pub fn min_filter(&self) -> TextureMapFilterType {
        self.min_filter
    }

    pub fn mag_filter(&self) -> TextureMapFilterType {
        self.mag_filter
    }
}

impl Default for TextureMap {
    fn default() -> Self {
        Self::new()
    }
}
