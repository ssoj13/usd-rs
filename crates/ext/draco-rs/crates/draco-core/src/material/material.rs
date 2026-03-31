//! Material definition.
//!
//! What: Represents GLTF-style PBR material with texture maps.
//! Why: Mirrors Draco `Material` for transcoding and mesh features.
//! How: Stores factors, extensions, and texture maps with ownership rules.
//! Where used: `MaterialLibrary`, texture utilities, and mesh features.

use std::collections::HashMap;
use std::ptr::NonNull;

use crate::core::status::{ok_status, Status, StatusCode};
use crate::core::vector_d::{Vector3f, Vector4f};
use crate::texture::texture::Texture;
use crate::texture::texture_library::TextureLibrary;
use crate::texture::texture_map::{
    TextureMap, TextureMapFilterType, TextureMapType, TextureMapWrappingMode,
};
use crate::texture::texture_transform::TextureTransform;

/// Material transparency modes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum MaterialTransparencyMode {
    Opaque = 0,
    Mask = 1,
    Blend = 2,
}

/// Material specification for Draco geometry.
#[derive(Debug)]
pub struct Material {
    name: String,
    color_factor: Vector4f,
    metallic_factor: f32,
    roughness_factor: f32,
    emissive_factor: Vector3f,
    double_sided: bool,
    transparency_mode: MaterialTransparencyMode,
    alpha_cutoff: f32,
    normal_texture_scale: f32,

    // KHR_materials_unlit.
    unlit: bool,

    // KHR_materials_sheen.
    has_sheen: bool,
    sheen_color_factor: Vector3f,
    sheen_roughness_factor: f32,

    // KHR_materials_transmission.
    has_transmission: bool,
    transmission_factor: f32,

    // KHR_materials_clearcoat.
    has_clearcoat: bool,
    clearcoat_factor: f32,
    clearcoat_roughness_factor: f32,

    // KHR_materials_volume.
    has_volume: bool,
    thickness_factor: f32,
    attenuation_distance: f32,
    attenuation_color: Vector3f,

    // KHR_materials_ior.
    has_ior: bool,
    ior: f32,

    // KHR_materials_specular.
    has_specular: bool,
    specular_factor: f32,
    specular_color_factor: Vector3f,

    texture_maps: Vec<Box<TextureMap>>,
    texture_map_type_to_index: HashMap<TextureMapType, usize>,
    texture_library: Option<NonNull<TextureLibrary>>,
}

impl Material {
    /// Creates a material without an associated texture library.
    pub fn new() -> Self {
        Self::with_texture_library(None)
    }

    /// Creates a material tied to a texture library.
    pub fn with_texture_library(texture_library: Option<NonNull<TextureLibrary>>) -> Self {
        let mut material = Self {
            name: String::new(),
            color_factor: Vector4f::new4(1.0, 1.0, 1.0, 1.0),
            metallic_factor: 1.0,
            roughness_factor: 1.0,
            emissive_factor: Vector3f::new3(0.0, 0.0, 0.0),
            double_sided: false,
            transparency_mode: MaterialTransparencyMode::Opaque,
            alpha_cutoff: 0.5,
            normal_texture_scale: 1.0,
            unlit: false,
            has_sheen: false,
            sheen_color_factor: Vector3f::new3(0.0, 0.0, 0.0),
            sheen_roughness_factor: 0.0,
            has_transmission: false,
            transmission_factor: 0.0,
            has_clearcoat: false,
            clearcoat_factor: 0.0,
            clearcoat_roughness_factor: 0.0,
            has_volume: false,
            thickness_factor: 0.0,
            attenuation_distance: f32::MAX,
            attenuation_color: Vector3f::new3(1.0, 1.0, 1.0),
            has_ior: false,
            ior: 1.5,
            has_specular: false,
            specular_factor: 1.0,
            specular_color_factor: Vector3f::new3(1.0, 1.0, 1.0),
            texture_maps: Vec::new(),
            texture_map_type_to_index: HashMap::new(),
            texture_library,
        };
        material.clear();
        material
    }

    /// Copies all material data from `src`.
    pub fn copy_from(&mut self, src: &Material) {
        self.name = src.name.clone();
        self.color_factor = src.color_factor;
        self.metallic_factor = src.metallic_factor;
        self.roughness_factor = src.roughness_factor;
        self.emissive_factor = src.emissive_factor;
        self.transparency_mode = src.transparency_mode;
        self.alpha_cutoff = src.alpha_cutoff;
        self.double_sided = src.double_sided;
        self.normal_texture_scale = src.normal_texture_scale;

        self.unlit = src.unlit;
        self.has_sheen = src.has_sheen;
        self.sheen_color_factor = src.sheen_color_factor;
        self.sheen_roughness_factor = src.sheen_roughness_factor;
        self.has_transmission = src.has_transmission;
        self.transmission_factor = src.transmission_factor;
        self.has_clearcoat = src.has_clearcoat;
        self.clearcoat_factor = src.clearcoat_factor;
        self.clearcoat_roughness_factor = src.clearcoat_roughness_factor;
        self.has_volume = src.has_volume;
        self.thickness_factor = src.thickness_factor;
        self.attenuation_distance = src.attenuation_distance;
        self.attenuation_color = src.attenuation_color;
        self.has_ior = src.has_ior;
        self.ior = src.ior;
        self.has_specular = src.has_specular;
        self.specular_factor = src.specular_factor;
        self.specular_color_factor = src.specular_color_factor;

        self.texture_map_type_to_index = src.texture_map_type_to_index.clone();
        self.texture_maps
            .resize_with(src.texture_maps.len(), || Box::new(TextureMap::new()));
        for (i, map) in src.texture_maps.iter().enumerate() {
            self.texture_maps[i].copy_from(map);
        }
    }

    /// Resets the material to defaults and clears texture maps.
    pub fn clear(&mut self) {
        self.clear_texture_maps();

        self.name.clear();
        self.color_factor = Vector4f::new4(1.0, 1.0, 1.0, 1.0);
        self.metallic_factor = 1.0;
        self.roughness_factor = 1.0;
        self.emissive_factor = Vector3f::new3(0.0, 0.0, 0.0);
        self.transparency_mode = MaterialTransparencyMode::Opaque;
        self.alpha_cutoff = 0.5;
        self.double_sided = false;
        self.normal_texture_scale = 1.0;

        self.unlit = false;
        self.has_sheen = false;
        self.sheen_color_factor = Vector3f::new3(0.0, 0.0, 0.0);
        self.sheen_roughness_factor = 0.0;
        self.has_transmission = false;
        self.transmission_factor = 0.0;
        self.has_clearcoat = false;
        self.clearcoat_factor = 0.0;
        self.clearcoat_roughness_factor = 0.0;
        self.has_volume = false;
        self.thickness_factor = 0.0;
        self.attenuation_distance = f32::MAX;
        self.attenuation_color = Vector3f::new3(1.0, 1.0, 1.0);
        self.has_ior = false;
        self.ior = 1.5;
        self.has_specular = false;
        self.specular_factor = 1.0;
        self.specular_color_factor = Vector3f::new3(1.0, 1.0, 1.0);
    }

    /// Removes all texture maps but keeps other properties.
    pub fn clear_texture_maps(&mut self) {
        self.texture_maps.clear();
        self.texture_map_type_to_index.clear();
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn set_name(&mut self, name: &str) {
        self.name = name.to_string();
    }

    pub fn color_factor(&self) -> Vector4f {
        self.color_factor
    }

    pub fn set_color_factor(&mut self, value: Vector4f) {
        self.color_factor = value;
    }

    pub fn metallic_factor(&self) -> f32 {
        self.metallic_factor
    }

    pub fn set_metallic_factor(&mut self, value: f32) {
        self.metallic_factor = value;
    }

    pub fn roughness_factor(&self) -> f32 {
        self.roughness_factor
    }

    pub fn set_roughness_factor(&mut self, value: f32) {
        self.roughness_factor = value;
    }

    pub fn emissive_factor(&self) -> Vector3f {
        self.emissive_factor
    }

    pub fn set_emissive_factor(&mut self, value: Vector3f) {
        self.emissive_factor = value;
    }

    pub fn double_sided(&self) -> bool {
        self.double_sided
    }

    pub fn set_double_sided(&mut self, value: bool) {
        self.double_sided = value;
    }

    pub fn transparency_mode(&self) -> MaterialTransparencyMode {
        self.transparency_mode
    }

    pub fn set_transparency_mode(&mut self, mode: MaterialTransparencyMode) {
        self.transparency_mode = mode;
    }

    pub fn alpha_cutoff(&self) -> f32 {
        self.alpha_cutoff
    }

    pub fn set_alpha_cutoff(&mut self, value: f32) {
        self.alpha_cutoff = value;
    }

    pub fn normal_texture_scale(&self) -> f32 {
        self.normal_texture_scale
    }

    pub fn set_normal_texture_scale(&mut self, value: f32) {
        self.normal_texture_scale = value;
    }

    pub fn unlit(&self) -> bool {
        self.unlit
    }

    pub fn set_unlit(&mut self, value: bool) {
        self.unlit = value;
    }

    pub fn has_sheen(&self) -> bool {
        self.has_sheen
    }

    pub fn set_has_sheen(&mut self, value: bool) {
        self.has_sheen = value;
    }

    pub fn sheen_color_factor(&self) -> Vector3f {
        self.sheen_color_factor
    }

    pub fn set_sheen_color_factor(&mut self, value: Vector3f) {
        self.sheen_color_factor = value;
    }

    pub fn sheen_roughness_factor(&self) -> f32 {
        self.sheen_roughness_factor
    }

    pub fn set_sheen_roughness_factor(&mut self, value: f32) {
        self.sheen_roughness_factor = value;
    }

    pub fn has_transmission(&self) -> bool {
        self.has_transmission
    }

    pub fn set_has_transmission(&mut self, value: bool) {
        self.has_transmission = value;
    }

    pub fn transmission_factor(&self) -> f32 {
        self.transmission_factor
    }

    pub fn set_transmission_factor(&mut self, value: f32) {
        self.transmission_factor = value;
    }

    pub fn has_clearcoat(&self) -> bool {
        self.has_clearcoat
    }

    pub fn set_has_clearcoat(&mut self, value: bool) {
        self.has_clearcoat = value;
    }

    pub fn clearcoat_factor(&self) -> f32 {
        self.clearcoat_factor
    }

    pub fn set_clearcoat_factor(&mut self, value: f32) {
        self.clearcoat_factor = value;
    }

    pub fn clearcoat_roughness_factor(&self) -> f32 {
        self.clearcoat_roughness_factor
    }

    pub fn set_clearcoat_roughness_factor(&mut self, value: f32) {
        self.clearcoat_roughness_factor = value;
    }

    pub fn has_volume(&self) -> bool {
        self.has_volume
    }

    pub fn set_has_volume(&mut self, value: bool) {
        self.has_volume = value;
    }

    pub fn thickness_factor(&self) -> f32 {
        self.thickness_factor
    }

    pub fn set_thickness_factor(&mut self, value: f32) {
        self.thickness_factor = value;
    }

    pub fn attenuation_distance(&self) -> f32 {
        self.attenuation_distance
    }

    pub fn set_attenuation_distance(&mut self, value: f32) {
        self.attenuation_distance = value;
    }

    pub fn attenuation_color(&self) -> Vector3f {
        self.attenuation_color
    }

    pub fn set_attenuation_color(&mut self, value: Vector3f) {
        self.attenuation_color = value;
    }

    pub fn has_ior(&self) -> bool {
        self.has_ior
    }

    pub fn set_has_ior(&mut self, value: bool) {
        self.has_ior = value;
    }

    pub fn ior(&self) -> f32 {
        self.ior
    }

    pub fn set_ior(&mut self, value: f32) {
        self.ior = value;
    }

    pub fn has_specular(&self) -> bool {
        self.has_specular
    }

    pub fn set_has_specular(&mut self, value: bool) {
        self.has_specular = value;
    }

    pub fn specular_factor(&self) -> f32 {
        self.specular_factor
    }

    pub fn set_specular_factor(&mut self, value: f32) {
        self.specular_factor = value;
    }

    pub fn specular_color_factor(&self) -> Vector3f {
        self.specular_color_factor
    }

    pub fn set_specular_color_factor(&mut self, value: Vector3f) {
        self.specular_color_factor = value;
    }

    pub fn num_texture_maps(&self) -> usize {
        self.texture_maps.len()
    }

    pub fn texture_map_by_index(&self, index: i32) -> Option<&TextureMap> {
        self.texture_maps.get(index as usize).map(|m| m.as_ref())
    }

    pub fn texture_map_by_index_mut(&mut self, index: i32) -> Option<&mut TextureMap> {
        self.texture_maps
            .get_mut(index as usize)
            .map(|m| m.as_mut())
    }

    pub fn texture_map_by_type(&self, texture_type: TextureMapType) -> Option<&TextureMap> {
        self.texture_map_type_to_index
            .get(&texture_type)
            .and_then(|index| self.texture_map_by_index(*index as i32))
    }

    pub fn texture_map_by_type_mut(
        &mut self,
        texture_type: TextureMapType,
    ) -> Option<&mut TextureMap> {
        let index = *self.texture_map_type_to_index.get(&texture_type)? as i32;
        self.texture_map_by_index_mut(index)
    }

    /// Adds a new texture map with an owned texture.
    pub fn set_texture_map(
        &mut self,
        texture: Box<Texture>,
        texture_map_type: TextureMapType,
        tex_coord_index: i32,
    ) {
        self.set_texture_map_with_wrapping(
            texture,
            texture_map_type,
            TextureMapWrappingMode::new(
                crate::texture::texture_map::TextureMapAxisWrappingMode::ClampToEdge,
            ),
            tex_coord_index,
        );
    }

    /// Adds a new texture map with an owned texture and wrapping mode.
    pub fn set_texture_map_with_wrapping(
        &mut self,
        texture: Box<Texture>,
        texture_map_type: TextureMapType,
        wrapping_mode: TextureMapWrappingMode,
        tex_coord_index: i32,
    ) {
        let mut texture_map = Box::new(TextureMap::new());
        texture_map.set_properties_with_wrapping(texture_map_type, wrapping_mode, tex_coord_index);

        if let Some(mut library_ptr) = self.texture_library {
            let library = unsafe { library_ptr.as_mut() };
            let index = library.push_texture(texture);
            if let Some(texture_ref) = library.texture_mut(index) {
                texture_map.set_texture_ptr(texture_ref as *mut Texture);
            }
        } else {
            texture_map.set_texture_owned(texture);
        }
        self.set_texture_map_internal(texture_map);
    }

    pub fn set_texture_map_existing(
        &mut self,
        texture: *mut Texture,
        texture_map_type: TextureMapType,
        tex_coord_index: i32,
    ) -> Status {
        self.set_texture_map_existing_full(
            texture,
            texture_map_type,
            TextureMapWrappingMode::new(
                crate::texture::texture_map::TextureMapAxisWrappingMode::ClampToEdge,
            ),
            TextureMapFilterType::Unspecified,
            TextureMapFilterType::Unspecified,
            None,
            tex_coord_index,
        )
    }

    pub fn set_texture_map_existing_with_wrapping(
        &mut self,
        texture: *mut Texture,
        texture_map_type: TextureMapType,
        wrapping_mode: TextureMapWrappingMode,
        tex_coord_index: i32,
    ) -> Status {
        self.set_texture_map_existing_full(
            texture,
            texture_map_type,
            wrapping_mode,
            TextureMapFilterType::Unspecified,
            TextureMapFilterType::Unspecified,
            None,
            tex_coord_index,
        )
    }

    pub fn set_texture_map_existing_with_filters(
        &mut self,
        texture: *mut Texture,
        texture_map_type: TextureMapType,
        wrapping_mode: TextureMapWrappingMode,
        min_filter: TextureMapFilterType,
        mag_filter: TextureMapFilterType,
        tex_coord_index: i32,
    ) -> Status {
        self.set_texture_map_existing_full(
            texture,
            texture_map_type,
            wrapping_mode,
            min_filter,
            mag_filter,
            None,
            tex_coord_index,
        )
    }

    pub fn set_texture_map_existing_with_transform(
        &mut self,
        texture: *mut Texture,
        texture_map_type: TextureMapType,
        wrapping_mode: TextureMapWrappingMode,
        min_filter: TextureMapFilterType,
        mag_filter: TextureMapFilterType,
        transform: &TextureTransform,
        tex_coord_index: i32,
    ) -> Status {
        self.set_texture_map_existing_full(
            texture,
            texture_map_type,
            wrapping_mode,
            min_filter,
            mag_filter,
            Some(transform),
            tex_coord_index,
        )
    }

    /// Sets texture map by library index (no raw pointer). Use when texture is in material's library.
    pub fn set_texture_map_existing_by_index_with_transform(
        &mut self,
        texture_index: i32,
        texture_map_type: TextureMapType,
        wrapping_mode: TextureMapWrappingMode,
        min_filter: TextureMapFilterType,
        mag_filter: TextureMapFilterType,
        transform: &TextureTransform,
        tex_coord_index: i32,
    ) -> Status {
        let texture_ptr = self
            .texture_library
            .and_then(|mut lib| unsafe { lib.as_mut() }.texture_mut(texture_index))
            .map(|t| t as *mut Texture)
            .unwrap_or(std::ptr::null_mut());
        self.set_texture_map_existing_full(
            texture_ptr,
            texture_map_type,
            wrapping_mode,
            min_filter,
            mag_filter,
            Some(transform),
            tex_coord_index,
        )
    }

    /// Sets texture map by library index (no raw pointer).
    pub fn set_texture_map_existing_by_index_with_filters(
        &mut self,
        texture_index: i32,
        texture_map_type: TextureMapType,
        wrapping_mode: TextureMapWrappingMode,
        min_filter: TextureMapFilterType,
        mag_filter: TextureMapFilterType,
        tex_coord_index: i32,
    ) -> Status {
        let texture_ptr = self
            .texture_library
            .and_then(|mut lib| unsafe { lib.as_mut() }.texture_mut(texture_index))
            .map(|t| t as *mut Texture)
            .unwrap_or(std::ptr::null_mut());
        self.set_texture_map_existing_full(
            texture_ptr,
            texture_map_type,
            wrapping_mode,
            min_filter,
            mag_filter,
            None,
            tex_coord_index,
        )
    }

    fn set_texture_map_existing_full(
        &mut self,
        texture: *mut Texture,
        texture_map_type: TextureMapType,
        wrapping_mode: TextureMapWrappingMode,
        min_filter: TextureMapFilterType,
        mag_filter: TextureMapFilterType,
        transform: Option<&TextureTransform>,
        tex_coord_index: i32,
    ) -> Status {
        if texture.is_null() || !self.is_texture_owned(texture) {
            return Status::new(
                StatusCode::DracoError,
                "Provided texture is not owned by the material.",
            );
        }
        let mut texture_map = Box::new(TextureMap::new());
        if let Some(transform) = transform {
            texture_map.set_transform(transform);
        }
        texture_map.set_properties_full(
            texture_map_type,
            wrapping_mode,
            tex_coord_index,
            min_filter,
            mag_filter,
        );
        texture_map.set_texture_ptr(texture);
        self.set_texture_map_internal(texture_map);
        ok_status()
    }

    fn set_texture_map_internal(&mut self, texture_map: Box<TextureMap>) {
        let map_type = texture_map.map_type();
        match self.texture_map_type_to_index.get(&map_type) {
            None => {
                self.texture_maps.push(texture_map);
                self.texture_map_type_to_index
                    .insert(map_type, self.texture_maps.len() - 1);
            }
            Some(&index) => {
                self.texture_maps[index] = texture_map;
            }
        }
    }

    fn is_texture_owned(&self, texture: *mut Texture) -> bool {
        if let Some(library_ptr) = self.texture_library {
            let library = unsafe { library_ptr.as_ref() };
            for i in 0..library.num_textures() {
                if let Some(candidate) = library.texture(i as i32) {
                    if std::ptr::eq(candidate as *const Texture, texture as *const Texture) {
                        return true;
                    }
                }
            }
            return false;
        }

        for map in &self.texture_maps {
            if let Some(tex) = map.texture() {
                if std::ptr::eq(tex as *const Texture, texture as *const Texture) {
                    return true;
                }
            }
        }
        false
    }

    pub fn remove_texture_map_by_index(&mut self, index: i32) -> Option<Box<TextureMap>> {
        if index < 0 || index as usize >= self.texture_maps.len() {
            return None;
        }
        let removed = self.texture_maps.remove(index as usize);
        for i in index as usize..self.texture_maps.len() {
            let map_type = self.texture_maps[i].map_type();
            self.texture_map_type_to_index.insert(map_type, i);
        }
        self.texture_map_type_to_index.remove(&removed.map_type());
        Some(removed)
    }

    pub fn remove_texture_map_by_type(
        &mut self,
        texture_type: TextureMapType,
    ) -> Option<Box<TextureMap>> {
        let index = *self.texture_map_type_to_index.get(&texture_type)? as i32;
        self.remove_texture_map_by_index(index)
    }
}

impl Default for Material {
    fn default() -> Self {
        Self::new()
    }
}
