//! Texture library container.
//!
//! What: Stores textures in an indexed list.
//! Why: Mirrors Draco `TextureLibrary` used by materials and scenes.
//! How: Owns textures and provides index/pointer lookups.
//! Where used: `MaterialLibrary` and mesh feature textures.

use std::collections::HashMap;

use crate::texture::texture::Texture;

/// Container for textures in an indexed list.
#[derive(Default, Debug)]
pub struct TextureLibrary {
    textures: Vec<Box<Texture>>,
}

impl TextureLibrary {
    /// Creates an empty texture library.
    pub fn new() -> Self {
        Self::default()
    }

    /// Copies textures from `src` into this library.
    pub fn copy_from(&mut self, src: &TextureLibrary) {
        self.clear();
        self.append(src);
    }

    /// Appends textures from `src` into this library.
    pub fn append(&mut self, src: &TextureLibrary) {
        let old_num_textures = self.textures.len();
        self.textures
            .resize_with(old_num_textures + src.textures.len(), || {
                Box::new(Texture::new())
            });
        for (i, texture) in src.textures.iter().enumerate() {
            let dst = &mut self.textures[old_num_textures + i];
            dst.copy_from(texture);
        }
    }

    /// Removes all textures.
    pub fn clear(&mut self) {
        self.textures.clear();
    }

    /// Pushes a new texture and returns its index.
    pub fn push_texture(&mut self, texture: Box<Texture>) -> i32 {
        self.textures.push(texture);
        (self.textures.len() - 1) as i32
    }

    pub fn num_textures(&self) -> usize {
        self.textures.len()
    }

    pub fn texture(&self, index: i32) -> Option<&Texture> {
        self.textures.get(index as usize).map(|t| t.as_ref())
    }

    pub fn texture_mut(&mut self, index: i32) -> Option<&mut Texture> {
        self.textures.get_mut(index as usize).map(|t| t.as_mut())
    }

    /// Returns the index of a texture by identity (uses pointer equality).
    pub fn index_of(&self, texture: &Texture) -> Option<i32> {
        for (i, t) in self.textures.iter().enumerate() {
            if std::ptr::eq(t.as_ref(), texture) {
                return Some(i as i32);
            }
        }
        None
    }

    /// Returns a map from texture pointer to index.
    pub fn compute_texture_to_index_map(&self) -> HashMap<*const Texture, i32> {
        let mut ret = HashMap::new();
        for (i, texture) in self.textures.iter().enumerate() {
            ret.insert(texture.as_ref() as *const Texture, i as i32);
        }
        ret
    }

    /// Removes and returns a texture by index.
    pub fn remove_texture(&mut self, index: i32) -> Option<Box<Texture>> {
        if index < 0 || index as usize >= self.textures.len() {
            return None;
        }
        Some(self.textures.remove(index as usize))
    }
}
