//! VFX texture system adapter — bridges `vfx_io::texture::TextureSystem` to OSL.
//!
//! This module wraps the full-featured `vfx-io` texture system (with MIP mapping,
//! anisotropic filtering, tiled caching, environment maps, and 3D textures) and
//! exposes it as functions callable from OSL's `RendererServices::texture()`,
//! `texture3d()`, and `environment()` methods.
//!
//! # Feature gate
//!
//! This module is only available when the `vfx` feature is enabled.
//! Without it, OSL falls back to the procedural stub textures in [`crate::texture`].

use std::sync::Arc;

use vfx_io::texture::{
    EnvLayout, InterpMode as VfxInterpMode, MipMode as VfxMipMode, TextureOpt as VfxTextureOpt,
    TextureOptions, TextureSystem, WrapMode as VfxWrapMode,
};

use crate::Float;
use crate::math::Vec3;
use crate::texture::{TextureInterp, TextureOpt as OslTextureOpt, TextureWrap};

/// Converts OSL texture options to vfx-io format.
fn osl_to_vfx_texture_opt(opt: &OslTextureOpt) -> VfxTextureOpt {
    VfxTextureOpt {
        firstchannel: opt.firstchannel,
        subimage: opt.subimage,
        subimagename: opt.subimagename.as_str().to_string(),
        swrap: osl_wrap_to_vfx(opt.swrap),
        twrap: osl_wrap_to_vfx(opt.twrap),
        rwrap: osl_wrap_to_vfx(opt.rwrap),
        mipmode: osl_mip_to_vfx(opt.mipmode),
        interpmode: osl_interp_to_vfx(opt.interpmode),
        conservative_filter: opt.conservative_filter,
        anisotropic: opt.anisotropic.clamp(0, u16::MAX as i32) as u16,
        sblur: opt.sblur,
        tblur: opt.tblur,
        rblur: opt.rblur,
        swidth: opt.swidth,
        twidth: opt.twidth,
        rwidth: opt.rwidth,
        fill: opt.fill,
        missingcolor: opt.missingcolor.map(|c| [c.x, c.y, c.z, 1.0]),
        rnd: -1.0,
        colortransformid: 0,
    }
}

#[inline]
fn osl_wrap_to_vfx(w: TextureWrap) -> VfxWrapMode {
    match w {
        TextureWrap::Default => VfxWrapMode::Default,
        TextureWrap::Black => VfxWrapMode::Black,
        TextureWrap::Clamp => VfxWrapMode::Clamp,
        TextureWrap::Periodic => VfxWrapMode::Periodic,
        TextureWrap::Mirror => VfxWrapMode::Mirror,
        TextureWrap::PeriodicPow2 => VfxWrapMode::PeriodicPow2,
        TextureWrap::PeriodicSharedBorder => VfxWrapMode::PeriodicSharedBorder,
    }
}

#[inline]
fn osl_interp_to_vfx(i: TextureInterp) -> VfxInterpMode {
    match i {
        TextureInterp::SmartBicubic => VfxInterpMode::SmartBicubic,
        TextureInterp::Bilinear => VfxInterpMode::Bilinear,
        TextureInterp::Bicubic => VfxInterpMode::Bicubic,
        TextureInterp::Closest => VfxInterpMode::Closest,
    }
}

#[inline]
fn osl_mip_to_vfx(m: i32) -> VfxMipMode {
    match m {
        1 => VfxMipMode::NoMIP,
        2 => VfxMipMode::OneLevel,
        3 => VfxMipMode::Trilinear,
        4 => VfxMipMode::Aniso,
        _ => VfxMipMode::Default,
    }
}

/// OSL texture system backed by `vfx-io`.
///
/// Wraps a `vfx_io::texture::TextureSystem` and provides OSL-compatible
/// texture lookup methods. Shared across threads via `Arc`.
#[derive(Clone)]
pub struct VfxTextureSystem {
    inner: Arc<TextureSystem>,
}

impl VfxTextureSystem {
    /// Create a new VFX texture system with default cache settings.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(TextureSystem::new()),
        }
    }

    /// Create from an existing `vfx_io::texture::TextureSystem`.
    pub fn from_inner(ts: Arc<TextureSystem>) -> Self {
        Self { inner: ts }
    }

    /// Access the underlying `TextureSystem`.
    pub fn inner(&self) -> &TextureSystem {
        &self.inner
    }

    /// 2D filtered texture lookup — matches `RendererServices::texture()`.
    ///
    /// Returns `Ok(())` on success (result filled), `Err` on failure.
    pub fn texture(
        &self,
        filename: &str,
        opt: &OslTextureOpt,
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
        let vfx_opt = osl_to_vfx_texture_opt(opt);
        let nch = opt.nchannels.max(nchannels) as usize;

        match self.inner.texture(
            filename, &vfx_opt, s, t, dsdx, dtdx, dsdy, dtdy, nch, result, dresultds, dresultdt,
        ) {
            Ok(true) => Ok(()),
            Ok(false) => Err(format!("texture '{}' not found", filename)),
            Err(e) => Err(e.to_string()),
        }
    }

    /// 3D filtered texture lookup — matches `RendererServices::texture3d()`.
    pub fn texture3d(
        &self,
        filename: &str,
        opt: &OslTextureOpt,
        p: &Vec3,
        _dpdx: &Vec3,
        _dpdy: &Vec3,
        _dpdz: &Vec3,
        nchannels: i32,
        result: &mut [Float],
    ) -> Result<(), String> {
        let vfx_opt = osl_to_vfx_texture_opt(opt);
        let opts = TextureOptions::from(&vfx_opt);
        let nch = opt.nchannels.max(nchannels).min(4) as usize;

        match self.inner.texture3d(filename, p.x, p.y, p.z, &opts) {
            Ok(color) => {
                let n = nch.min(result.len());
                result[..n].copy_from_slice(&color[..n]);
                Ok(())
            }
            Err(e) => Err(e.to_string()),
        }
    }

    /// Environment map lookup — matches `RendererServices::environment()`.
    pub fn environment(
        &self,
        filename: &str,
        opt: &OslTextureOpt,
        r: &Vec3,
        _drdx: &Vec3,
        _drdy: &Vec3,
        nchannels: i32,
        result: &mut [Float],
    ) -> Result<(), String> {
        let vfx_opt = osl_to_vfx_texture_opt(opt);
        let opts = TextureOptions::from(&vfx_opt);
        let dir = [r.x, r.y, r.z];
        let nch = opt.nchannels.max(nchannels).min(4) as usize;

        match self
            .inner
            .environment(filename, &dir, EnvLayout::LatLong, &opts)
        {
            Ok(color) => {
                let n = nch.min(result.len());
                result[..n].copy_from_slice(&color[..n]);
                Ok(())
            }
            Err(e) => Err(e.to_string()),
        }
    }

    /// Query texture metadata — matches `RendererServices::get_texture_info()`.
    pub fn get_texture_info(&self, filename: &str, dataname: &str) -> Option<TextureInfoValue> {
        let handle = self.inner.get_handle(filename).ok()?;
        match dataname {
            "exists" => Some(TextureInfoValue::Int(1)),
            "resolution" => Some(TextureInfoValue::IntVec(vec![
                handle.width(0) as i32,
                handle.height(0) as i32,
            ])),
            "channels" => Some(TextureInfoValue::Int(handle.channels() as i32)),
            "subimages" => Some(TextureInfoValue::Int(handle.subimages() as i32)),
            "miplevels" => Some(TextureInfoValue::Int(handle.mip_levels() as i32)),
            "type" => Some(TextureInfoValue::Str("float".to_string())),
            _ => None,
        }
    }

    /// Invalidate cached data for a texture file.
    pub fn invalidate(&self, filename: &str) {
        self.inner.invalidate(filename, true);
    }

    /// Clear all cached texture data.
    pub fn clear_cache(&self) {
        self.inner.clear();
    }
}

impl Default for VfxTextureSystem {
    fn default() -> Self {
        Self::new()
    }
}

/// Texture metadata value returned from `get_texture_info`.
#[derive(Debug, Clone)]
pub enum TextureInfoValue {
    Int(i32),
    Float(f32),
    Str(String),
    IntVec(Vec<i32>),
    FloatVec(Vec<f32>),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::texture::TextureOpt;

    #[test]
    fn test_vfx_texture_system_creation() {
        let ts = VfxTextureSystem::new();
        // Should be able to create without panic
        assert!(ts.inner().cache().size() == 0);
    }

    #[test]
    fn test_texture_missing_file() {
        let ts = VfxTextureSystem::new();
        let mut result = [0.0f32; 4];
        let opt = TextureOpt::default();
        let err = ts.texture(
            "nonexistent.exr",
            &opt,
            0.5,
            0.5,
            0.0,
            0.0,
            0.0,
            0.0,
            4,
            &mut result,
            None,
            None,
        );
        assert!(err.is_err());
    }

    #[test]
    fn test_env_missing_file() {
        let ts = VfxTextureSystem::new();
        let mut result = [0.0f32; 4];
        let opt = TextureOpt::default();
        let dir = Vec3::new(0.0, 1.0, 0.0);
        let err = ts.environment(
            "nonexistent.hdr",
            &opt,
            &dir,
            &Vec3::ZERO,
            &Vec3::ZERO,
            4,
            &mut result,
        );
        assert!(err.is_err());
    }

    #[test]
    fn test_texture_info_missing() {
        let ts = VfxTextureSystem::new();
        let info = ts.get_texture_info("nonexistent.exr", "exists");
        assert!(info.is_none());
    }
}
