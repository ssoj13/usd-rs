//! Material library.
//!
//! What: Stores materials and shared texture library.
//! Why: Mirrors Draco `MaterialLibrary` for scene/mesh assets.
//! How: Owns materials, texture library, and variants.
//! Where used: glTF transcoding and texture utilities.

use std::collections::BTreeMap;
use std::ptr::NonNull;

use crate::material::material::Material;
use crate::texture::texture_library::TextureLibrary;
use crate::texture::texture_map::TextureMap;

/// Container for materials applied to a model.
#[derive(Default, Debug)]
pub struct MaterialLibrary {
    materials: Vec<Box<Material>>,
    materials_variants_names: Vec<String>,
    texture_library: TextureLibrary,
}

impl MaterialLibrary {
    pub fn new() -> Self {
        Self::default()
    }

    /// Copies the source library into this one.
    pub fn copy_from(&mut self, src: &MaterialLibrary) {
        self.clear();
        self.append(src);
    }

    /// Appends materials from `src`, copying materials and textures.
    pub fn append(&mut self, src: &MaterialLibrary) {
        let old_num_materials = self.materials.len();
        let library_ptr = NonNull::from(&mut self.texture_library);
        self.materials
            .resize_with(old_num_materials + src.materials.len(), || {
                Box::new(Material::with_texture_library(Some(library_ptr)))
            });
        for (i, material) in src.materials.iter().enumerate() {
            self.materials[old_num_materials + i].copy_from(material);
        }

        let old_num_textures = self.texture_library.num_textures();
        self.texture_library.append(&src.texture_library);
        for name in &src.materials_variants_names {
            self.materials_variants_names.push(name.clone());
        }

        let texture_map_to_index =
            self.compute_texture_map_to_texture_index_mapping(&src.texture_library);
        for (texture_map_ptr, index) in texture_map_to_index {
            let texture_index = old_num_textures + index as usize;
            if let Some(texture) = self.texture_library.texture_mut(texture_index as i32) {
                unsafe {
                    (*texture_map_ptr).set_texture_ptr(texture as *mut _);
                }
            }
        }
    }

    /// Clears all materials and textures.
    pub fn clear(&mut self) {
        self.materials.clear();
        self.texture_library.clear();
        self.materials_variants_names.clear();
    }

    pub fn num_materials(&self) -> usize {
        self.materials.len()
    }

    pub fn material(&self, index: i32) -> Option<&Material> {
        if index < 0 || index as usize >= self.materials.len() {
            return None;
        }
        self.materials.get(index as usize).map(|m| m.as_ref())
    }

    pub fn mutable_material(&mut self, index: i32) -> Option<&mut Material> {
        if index < 0 {
            return None;
        }
        let index = index as usize;
        if self.materials.len() <= index {
            let old_size = self.materials.len();
            let library_ptr = NonNull::from(&mut self.texture_library);
            self.materials.resize_with(index + 1, || {
                Box::new(Material::with_texture_library(Some(library_ptr)))
            });
            // Ensure newly created materials are initialized (constructor already does).
            for i in old_size..=index {
                let _ = &self.materials[i];
            }
        }
        self.materials.get_mut(index).map(|m| m.as_mut())
    }

    pub fn remove_material(&mut self, index: i32) -> Option<Box<Material>> {
        if index < 0 || index as usize >= self.materials.len() {
            return None;
        }
        Some(self.materials.remove(index as usize))
    }

    pub fn texture_library(&self) -> &TextureLibrary {
        &self.texture_library
    }

    pub fn texture_library_mut(&mut self) -> &mut TextureLibrary {
        &mut self.texture_library
    }

    /// Removes textures that are not referenced by any texture map.
    pub fn remove_unused_textures(&mut self) {
        let texture_map_to_index =
            self.compute_texture_map_to_texture_index_mapping(&self.texture_library);
        let mut is_texture_used = vec![false; self.texture_library.num_textures()];
        for (_map, index) in texture_map_to_index {
            if index >= 0 {
                is_texture_used[index as usize] = true;
            }
        }

        let mut i = self.texture_library.num_textures();
        while i > 0 {
            i -= 1;
            if !is_texture_used[i] {
                let _ = self.texture_library.remove_texture(i as i32);
            }
        }
    }

    /// Returns a map between each texture map and the texture index in `library`.
    pub fn compute_texture_map_to_texture_index_mapping(
        &self,
        library: &TextureLibrary,
    ) -> BTreeMap<*mut TextureMap, i32> {
        let mut map_to_index: BTreeMap<*mut TextureMap, i32> = BTreeMap::new();
        for material in &self.materials {
            for i in 0..material.num_texture_maps() {
                if let Some(texture_map) = material.texture_map_by_index(i as i32) {
                    let texture_map_ptr = texture_map as *const TextureMap as *mut TextureMap;
                    for tli in 0..library.num_textures() {
                        if let Some(tex) = library.texture(tli as i32) {
                            if let Some(map_tex) = texture_map.texture() {
                                if std::ptr::eq(tex as *const _, map_tex as *const _) {
                                    map_to_index.insert(texture_map_ptr, tli as i32);
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }
        map_to_index
    }

    pub fn add_materials_variant(&mut self, name: &str) -> i32 {
        self.materials_variants_names.push(name.to_string());
        (self.materials_variants_names.len() - 1) as i32
    }

    pub fn num_materials_variants(&self) -> i32 {
        self.materials_variants_names.len() as i32
    }

    pub fn materials_variant_name(&self, index: i32) -> &str {
        &self.materials_variants_names[index as usize]
    }
}
