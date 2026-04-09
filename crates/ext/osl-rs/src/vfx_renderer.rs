//! VFX-integrated renderer — `RendererServices` backed by `vfx-io` + `vfx-ocio`.
//!
//! `VfxRenderer` is a production-quality implementation of [`RendererServices`]
//! that uses:
//! - [`VfxTextureSystem`] for real texture/environment/3D texture lookups
//! - [`VfxColorSystem`] for OCIO-backed color space transforms
//! - Named coordinate spaces and attribute queries (inherited from `BasicRenderer`)
//!
//! # Usage
//!
//! ```ignore
//! use osl_rs::vfx_renderer::VfxRenderer;
//!
//! let renderer = VfxRenderer::new();
//! // Use with ShadingSystem
//! let ss = ShadingSystem::new(Box::new(renderer));
//! ```

use std::collections::HashMap;

use crate::Float;
use crate::color_vfx::VfxColorSystem;
use crate::math::{Color3, Matrix44, Vec3};
use crate::renderer::{AttributeData, RendererServices, TextureHandle};
use crate::shaderglobals::ShaderGlobals;
use crate::texture_vfx::VfxTextureSystem;
use crate::typedesc::{TypeDesc, VecSemantics};
use crate::ustring::UStringHash;

/// Production renderer backed by vfx-rs for real texture + color support.
pub struct VfxRenderer {
    /// Named coordinate spaces.
    pub transforms: HashMap<String, Matrix44>,
    /// Named attributes.
    pub attributes: HashMap<String, AttributeData>,
    /// Camera-to-world matrix.
    pub camera_to_world: Matrix44,
    /// World-to-camera matrix.
    pub world_to_camera: Matrix44,
    /// VFX texture system for real texture I/O.
    pub texture_system: VfxTextureSystem,
    /// VFX color system for OCIO-backed transforms.
    pub color_system: VfxColorSystem,
}

impl VfxRenderer {
    /// Create a VfxRenderer with default ACES 1.3 color config.
    pub fn new() -> Self {
        let mut transforms = HashMap::new();
        transforms.insert("common".into(), Matrix44::IDENTITY);
        transforms.insert("world".into(), Matrix44::IDENTITY);
        transforms.insert("camera".into(), Matrix44::IDENTITY);
        transforms.insert("screen".into(), Matrix44::IDENTITY);
        transforms.insert("NDC".into(), Matrix44::IDENTITY);
        transforms.insert("raster".into(), Matrix44::IDENTITY);
        transforms.insert("object".into(), Matrix44::IDENTITY);
        transforms.insert("shader".into(), Matrix44::IDENTITY);

        Self {
            transforms,
            attributes: HashMap::new(),
            camera_to_world: Matrix44::IDENTITY,
            world_to_camera: Matrix44::IDENTITY,
            texture_system: VfxTextureSystem::new(),
            color_system: VfxColorSystem::new(),
        }
    }

    /// Create with a custom OCIO config file.
    pub fn with_ocio_config(ocio_path: &str) -> Result<Self, String> {
        let mut r = Self::new();
        r.color_system = VfxColorSystem::from_file(ocio_path)?;
        Ok(r)
    }

    /// Set a named coordinate space transform.
    pub fn set_transform(&mut self, name: &str, mat: Matrix44) {
        self.transforms.insert(name.to_string(), mat);
    }

    /// Set a named attribute.
    pub fn set_attribute(&mut self, name: &str, data: AttributeData) {
        self.attributes.insert(name.to_string(), data);
    }

    /// Set camera matrices.
    pub fn set_camera(&mut self, cam_to_world: Matrix44) {
        self.camera_to_world = cam_to_world;
        self.world_to_camera = cam_to_world.inverse().unwrap_or(Matrix44::IDENTITY);
        self.transforms.insert("camera".to_string(), cam_to_world);
    }

    /// Transform a color using the VFX color system (OCIO).
    pub fn transformc(&self, from: &str, to: &str, color: Color3) -> Result<Color3, String> {
        self.color_system.transformc(from, to, color)
    }
}

impl Default for VfxRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl RendererServices for VfxRenderer {
    fn supports(&self, feature: &str) -> bool {
        matches!(
            feature,
            "get_matrix"
                | "get_attribute"
                | "get_userdata"
                | "texture"
                | "texture3d"
                | "environment"
                | "transformc"
        )
    }

    fn renderer_name(&self) -> &str {
        "vfx"
    }

    fn get_matrix_named(
        &self,
        _sg: &ShaderGlobals,
        from: UStringHash,
        _time: Float,
    ) -> Option<Matrix44> {
        if let Some(us) = crate::ustring::UString::from_hash(from.hash())
            && let Some(mat) = self.transforms.get(us.as_str())
        {
            return Some(*mat);
        }
        None
    }

    fn get_matrix_named_static(&self, _sg: &ShaderGlobals, from: UStringHash) -> Option<Matrix44> {
        if let Some(us) = crate::ustring::UString::from_hash(from.hash())
            && let Some(mat) = self.transforms.get(us.as_str())
        {
            return Some(*mat);
        }
        None
    }

    fn get_attribute(
        &self,
        _sg: &ShaderGlobals,
        _derivatives: bool,
        object: UStringHash,
        _type_desc: TypeDesc,
        name: UStringHash,
    ) -> Option<AttributeData> {
        let name_str = crate::ustring::UString::from_hash(name.hash())
            .map(|u| u.as_str().to_string())
            .unwrap_or_default();
        let obj_str = crate::ustring::UString::from_hash(object.hash())
            .map(|u| u.as_str().to_string())
            .unwrap_or_default();

        let key = if obj_str.is_empty() {
            name_str.clone()
        } else {
            format!("{obj_str}:{name_str}")
        };

        self.attributes
            .get(&key)
            .or_else(|| self.attributes.get(&name_str))
            .cloned()
    }

    fn texture(
        &self,
        filename: UStringHash,
        _handle: TextureHandle,
        _sg: &ShaderGlobals,
        opt: &crate::texture::TextureOpt,
        s: Float,
        t: Float,
        dsdx: Float,
        dtdx: Float,
        dsdy: Float,
        dtdy: Float,
        nchannels: i32,
        result: &mut [Float],
        dresultds: Option<&mut [Float]>,
        dresultdt: Option<&mut [Float]>,
    ) -> Result<(), String> {
        let name = crate::ustring::UString::from_hash(filename.hash())
            .map(|u| u.as_str().to_string())
            .unwrap_or_default();

        self.texture_system.texture(
            &name, opt, s, t, dsdx, dtdx, dsdy, dtdy, nchannels, result, dresultds, dresultdt,
        )
    }

    fn texture3d(
        &self,
        filename: UStringHash,
        _handle: TextureHandle,
        _sg: &ShaderGlobals,
        opt: &crate::texture::TextureOpt,
        p: &Vec3,
        dpdx: &Vec3,
        dpdy: &Vec3,
        dpdz: &Vec3,
        nchannels: i32,
        result: &mut [Float],
        _dresultds: Option<&mut [Float]>,
        _dresultdt: Option<&mut [Float]>,
        _dresultdr: Option<&mut [Float]>,
    ) -> Result<(), String> {
        let _ = _dresultdr;
        let name = crate::ustring::UString::from_hash(filename.hash())
            .map(|u| u.as_str().to_string())
            .unwrap_or_default();

        self.texture_system
            .texture3d(&name, opt, p, dpdx, dpdy, dpdz, nchannels, result)
    }

    fn environment(
        &self,
        filename: UStringHash,
        _handle: TextureHandle,
        _sg: &ShaderGlobals,
        opt: &crate::texture::TextureOpt,
        r: &Vec3,
        drdx: &Vec3,
        drdy: &Vec3,
        nchannels: i32,
        result: &mut [Float],
        _dresultds: Option<&mut [Float]>,
        _dresultdt: Option<&mut [Float]>,
    ) -> Result<(), String> {
        let name = crate::ustring::UString::from_hash(filename.hash())
            .map(|u| u.as_str().to_string())
            .unwrap_or_default();

        self.texture_system
            .environment(&name, opt, r, drdx, drdy, nchannels, result)
    }

    fn transform_points(
        &self,
        sg: &ShaderGlobals,
        from: UStringHash,
        to: UStringHash,
        time: Float,
        pin: &[Vec3],
        pout: &mut [Vec3],
        _vectype: VecSemantics,
    ) -> bool {
        let mat_from = self.get_matrix_named(sg, from, time);
        let mat_to_inv = self.get_inverse_matrix_named(sg, to, time);

        match (mat_from, mat_to_inv) {
            (Some(mf), Some(mti)) => {
                let combined = crate::matrix_ops::matmul(&mti, &mf);
                for (i, p) in pin.iter().enumerate() {
                    if i < pout.len() {
                        pout[i] = crate::matrix_ops::transform_point(&combined, *p);
                    }
                }
                false
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vfx_renderer_creation() {
        let r = VfxRenderer::new();
        assert!(r.supports("texture"));
        assert!(r.supports("transformc"));
        assert_eq!(r.renderer_name(), "vfx");
    }

    #[test]
    fn test_vfx_renderer_transforms() {
        let mut r = VfxRenderer::new();
        let mat = Matrix44::translate(Vec3::new(1.0, 2.0, 3.0));
        r.set_transform("myspace", mat);
        assert!(r.transforms.contains_key("myspace"));
    }

    #[test]
    fn test_vfx_renderer_color() {
        let r = VfxRenderer::new();
        let gray = Color3::new(0.18, 0.18, 0.18);
        let srgb = r.transformc("ACEScg", "sRGB", gray);
        assert!(srgb.is_ok());
        let srgb = srgb.unwrap();
        assert!(srgb.x > 0.3 && srgb.x < 0.6);
    }

    #[test]
    fn test_vfx_renderer_attribute() {
        use crate::ustring::UString;

        let mut r = VfxRenderer::new();
        r.set_attribute("object:name", AttributeData::String("sphere".into()));

        let sg = ShaderGlobals::default();
        // Intern the strings first so from_hash() can resolve them
        let name_us = UString::new("name");
        let obj_us = UString::new("object");
        let name_hash = UStringHash::from_hash(name_us.hash());
        let obj_hash = UStringHash::from_hash(obj_us.hash());

        let result = r.get_attribute(&sg, false, obj_hash, TypeDesc::STRING, name_hash);
        assert!(result.is_some());
    }
}
