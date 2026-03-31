//! Texture transform values.
//!
//! What: Stores UV transform (offset/scale/rotation/tex_coord).
//! Why: Mirrors Draco `TextureTransform` used by `TextureMap`.
//! How: Simple POD with helpers for defaults and equality.
//! Where used: Texture maps in materials and mesh features.

/// Texture transform parameters (KHR_texture_transform).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextureTransform {
    offset: [f64; 2],
    rotation: f64,
    scale: [f64; 2],
    tex_coord: i32,
}

impl TextureTransform {
    /// Creates a transform initialized to default values.
    pub fn new() -> Self {
        let mut tt = Self {
            offset: Self::default_offset(),
            rotation: Self::default_rotation(),
            scale: Self::default_scale(),
            tex_coord: Self::default_tex_coord(),
        };
        tt.clear();
        tt
    }

    /// Resets all values to defaults.
    pub fn clear(&mut self) {
        self.offset = Self::default_offset();
        self.rotation = Self::default_rotation();
        self.scale = Self::default_scale();
        self.tex_coord = Self::default_tex_coord();
    }

    /// Copies values from `src`.
    pub fn copy_from(&mut self, src: &TextureTransform) {
        self.offset = src.offset;
        self.rotation = src.rotation;
        self.scale = src.scale;
        self.tex_coord = src.tex_coord;
    }

    /// Returns true if `tt` equals the default transform.
    pub fn is_default(tt: &TextureTransform) -> bool {
        let defaults = TextureTransform::new();
        *tt == defaults
    }

    pub fn is_offset_set(&self) -> bool {
        self.offset != Self::default_offset()
    }

    pub fn is_rotation_set(&self) -> bool {
        self.rotation != Self::default_rotation()
    }

    pub fn is_scale_set(&self) -> bool {
        self.scale != Self::default_scale()
    }

    pub fn is_tex_coord_set(&self) -> bool {
        self.tex_coord != Self::default_tex_coord()
    }

    pub fn set_offset(&mut self, offset: [f64; 2]) {
        self.offset = offset;
    }

    pub fn offset(&self) -> [f64; 2] {
        self.offset
    }

    pub fn set_scale(&mut self, scale: [f64; 2]) {
        self.scale = scale;
    }

    pub fn scale(&self) -> [f64; 2] {
        self.scale
    }

    pub fn set_rotation(&mut self, rotation: f64) {
        self.rotation = rotation;
    }

    pub fn rotation(&self) -> f64 {
        self.rotation
    }

    pub fn set_tex_coord(&mut self, tex_coord: i32) {
        self.tex_coord = tex_coord;
    }

    pub fn tex_coord(&self) -> i32 {
        self.tex_coord
    }

    fn default_offset() -> [f64; 2] {
        [0.0, 0.0]
    }

    fn default_rotation() -> f64 {
        0.0
    }

    fn default_scale() -> [f64; 2] {
        [0.0, 0.0]
    }

    fn default_tex_coord() -> i32 {
        -1
    }
}

impl Default for TextureTransform {
    fn default() -> Self {
        Self::new()
    }
}
