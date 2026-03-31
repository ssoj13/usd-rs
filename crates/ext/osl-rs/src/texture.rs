//! Texture operations — texture(), texture3d(), environment(), gettextureinfo().
//!
//! Port of `optexture.cpp`. Provides the interface and default implementations
//! for texture sampling operations. Actual texture I/O is delegated to
//! the RendererServices trait.

use crate::Float;
use crate::math::{Color3, Vec3};
use crate::renderer::RendererServices;
use crate::shaderglobals::ShaderGlobals;
use crate::ustring::{UString, UStringHash};

/// Texture wrapping mode.
/// Matches OIIO `TextureOpt::Wrap`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum TextureWrap {
    Default = 0,
    Black = 1,
    Clamp = 2,
    Periodic = 3,
    Mirror = 4,
    PeriodicPow2 = 5,
    PeriodicSharedBorder = 6,
}

impl TextureWrap {
    pub fn from_str(s: &str) -> Self {
        match s {
            "black" => TextureWrap::Black,
            "clamp" => TextureWrap::Clamp,
            "periodic" => TextureWrap::Periodic,
            "mirror" => TextureWrap::Mirror,
            "periodic_pow2" => TextureWrap::PeriodicPow2,
            "periodic_sharedborder" => TextureWrap::PeriodicSharedBorder,
            _ => TextureWrap::Default,
        }
    }
}

/// Texture interpolation mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum TextureInterp {
    SmartBicubic = 0,
    Bilinear = 1,
    Bicubic = 2,
    Closest = 3,
}

/// Texture lookup options.
/// Matches OIIO `TextureOpt` + OSL extensions.
#[derive(Debug, Clone)]
pub struct TextureOpt {
    pub firstchannel: i32,
    pub nchannels: i32,
    pub swrap: TextureWrap,
    pub twrap: TextureWrap,
    pub rwrap: TextureWrap,
    pub mipmode: i32,
    pub interpmode: TextureInterp,
    pub anisotropic: i32,
    pub conservative_filter: bool,
    pub sblur: Float,
    pub tblur: Float,
    /// Blur in the r (3D depth) direction. Used by texture3d.
    pub rblur: Float,
    pub swidth: Float,
    pub twidth: Float,
    /// Filter width in the r (3D depth) direction. Used by texture3d.
    pub rwidth: Float,
    pub fill: Float,
    pub missingcolor: Option<Color3>,
    pub time: Float,
    pub subimage: i32,
    pub subimagename: UString,
}

impl Default for TextureOpt {
    fn default() -> Self {
        Self {
            firstchannel: 0,
            nchannels: 3,
            swrap: TextureWrap::Default,
            twrap: TextureWrap::Default,
            rwrap: TextureWrap::Default,
            mipmode: 0,
            interpmode: TextureInterp::SmartBicubic,
            anisotropic: 32,
            conservative_filter: true,
            sblur: 0.0,
            tblur: 0.0,
            rblur: 0.0,
            swidth: 1.0,
            twidth: 1.0,
            rwidth: 1.0,
            fill: 0.0,
            missingcolor: None,
            time: 0.0,
            subimage: 0,
            subimagename: UString::empty(),
        }
    }
}

impl TextureInterp {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "linear" | "bilinear" => TextureInterp::Bilinear,
            "cubic" | "bicubic" => TextureInterp::Bicubic,
            "smartcubic" | "smart bicubic" => TextureInterp::SmartBicubic,
            "closest" | "point" => TextureInterp::Closest,
            _ => TextureInterp::SmartBicubic,
        }
    }
}

/// Value for texture optional args (string name + int/float/string value).
#[derive(Clone)]
pub enum TextureOptArg {
    Int(i32),
    Float(Float),
    Str(String),
}

impl TextureOptArg {
    fn as_int(&self) -> Option<i32> {
        match self {
            TextureOptArg::Int(i) => Some(*i),
            TextureOptArg::Float(f) => Some(*f as i32),
            _ => None,
        }
    }
    fn as_float(&self) -> Option<Float> {
        match self {
            TextureOptArg::Float(f) => Some(*f),
            TextureOptArg::Int(i) => Some(*i as Float),
            _ => None,
        }
    }
    fn as_str(&self) -> Option<&str> {
        match self {
            TextureOptArg::Str(s) => Some(s),
            _ => None,
        }
    }
}

/// Parse optional texture arguments from name-value pairs.
/// Matches C++ llvm_gen_texture_options (swrap, twrap, rwrap, fill, firstchannel, subimage, interp, etc.).
pub fn parse_texture_options<I>(pairs: I) -> TextureOpt
where
    I: IntoIterator<Item = (String, TextureOptArg)>,
{
    let mut opt = TextureOpt::default();
    for (name, val) in pairs {
        let name = name.to_lowercase();
        match name.as_str() {
            "swrap" => {
                if let Some(s) = val.as_str() {
                    opt.swrap = TextureWrap::from_str(s);
                }
            }
            "twrap" => {
                if let Some(s) = val.as_str() {
                    opt.twrap = TextureWrap::from_str(s);
                }
            }
            "rwrap" => {
                if let Some(s) = val.as_str() {
                    opt.rwrap = TextureWrap::from_str(s);
                }
            }
            "wrap" => {
                if let Some(s) = val.as_str() {
                    let w = TextureWrap::from_str(s);
                    opt.swrap = w;
                    opt.twrap = w;
                    opt.rwrap = w;
                }
            }
            "fill" => {
                if let Some(f) = val.as_float() {
                    opt.fill = f;
                }
            }
            "firstchannel" => {
                if let Some(i) = val.as_int() {
                    opt.firstchannel = i;
                }
            }
            "subimage" => {
                if let Some(i) = val.as_int() {
                    opt.subimage = i;
                }
            }
            "interp" => {
                if let Some(s) = val.as_str() {
                    opt.interpmode = TextureInterp::from_str(s);
                }
            }
            "swidth" => {
                if let Some(f) = val.as_float() {
                    opt.swidth = f;
                    // C++ only sets swidth here, NOT twidth
                }
            }
            "twidth" => {
                if let Some(f) = val.as_float() {
                    opt.twidth = f;
                }
            }
            "rwidth" => {
                if let Some(f) = val.as_float() {
                    opt.rwidth = f;
                }
            }
            "width" => {
                if let Some(f) = val.as_float() {
                    opt.swidth = f;
                    opt.twidth = f;
                    opt.rwidth = f;
                }
            }
            "blur" => {
                if let Some(f) = val.as_float() {
                    opt.sblur = f;
                    opt.tblur = f;
                    opt.rblur = f;
                }
            }
            "sblur" => {
                if let Some(f) = val.as_float() {
                    opt.sblur = f;
                }
            }
            "tblur" => {
                if let Some(f) = val.as_float() {
                    opt.tblur = f;
                }
            }
            "rblur" => {
                if let Some(f) = val.as_float() {
                    opt.rblur = f;
                }
            }
            "subimagename" => {
                if let Some(s) = val.as_str() {
                    opt.subimagename = UString::new(s);
                }
            }
            "missingcolor" => {
                // TODO: parse missingcolor from float[3] value
            }
            _ => {}
        }
    }
    opt
}

/// Texture handle — opaque reference used by RendererServices.
pub type TextureHandle = *const std::ffi::c_void;

/// Result of a texture lookup, including optional screen-space derivatives.
///
/// When derivatives are available (from RendererServices + chain rule),
/// `dcolor_dx`/`dcolor_dy` carry dResult/dx and dResult/dy in screen space.
#[derive(Debug, Clone)]
pub struct TextureResult {
    pub color: Color3,
    pub alpha: Float,
    pub ok: bool,
    /// dColor/dx in screen space (chain rule: dresultds * dsdx + dresultdt * dtdx).
    pub dcolor_dx: Color3,
    /// dColor/dy in screen space (chain rule: dresultds * dsdy + dresultdt * dtdy).
    pub dcolor_dy: Color3,
    /// dAlpha/dx in screen space.
    pub dalpha_dx: Float,
    /// dAlpha/dy in screen space.
    pub dalpha_dy: Float,
}

impl Default for TextureResult {
    fn default() -> Self {
        Self {
            color: Color3::ZERO,
            alpha: 1.0,
            ok: true,
            dcolor_dx: Color3::ZERO,
            dcolor_dy: Color3::ZERO,
            dalpha_dx: 0.0,
            dalpha_dy: 0.0,
        }
    }
}

/// Compute MIP level from texture-space derivatives (per-pixel footprint).
///
/// Matches OIIO/OpenGL style: `level = log2(rho)` where `rho` is the maximum
/// axis-aligned footprint in texels. Used by texture systems for trilinear filtering.
///
/// When derivatives are zero or very small, returns 0 (finest level).
#[inline]
pub fn mip_level_from_derivs(
    dsdx: Float,
    dtdx: Float,
    dsdy: Float,
    dtdy: Float,
    width: i32,
    height: i32,
) -> Float {
    if width <= 0 || height <= 0 {
        return 0.0;
    }
    let w = width as Float;
    let h = height as Float;
    // Footprint in texels: (dsdx*w, dtdx*h) and (dsdy*w, dtdy*h)
    let rho_x = (dsdx * dsdx * w * w + dtdx * dtdx * h * h).sqrt();
    let rho_y = (dsdy * dsdy * w * w + dtdy * dtdy * h * h).sqrt();
    let rho = rho_x.max(rho_y).max(1e-8);
    rho.log2().max(0.0)
}

/// Texture lookup for 2D textures.
///
/// This is the default software implementation. In production,
/// the RendererServices trait provides the actual texture lookup.
/// Uses MIP level from derivatives for procedural checker blur when footprint is large.
pub fn texture_2d(
    _filename: &str,
    s: Float,
    t: Float,
    dsdx: Float,
    dtdx: Float,
    dsdy: Float,
    dtdy: Float,
    opt: &TextureOpt,
) -> TextureResult {
    // Wrap coordinates
    let s = wrap_coord(s, opt.swrap);
    let t = wrap_coord(t, opt.twrap);

    // MIP-aware procedural checker: blur when derivatives indicate large footprint
    let (width, height) = (256, 256); // match get_texture_info "resolution"
    let mip_level = mip_level_from_derivs(dsdx, dtdx, dsdy, dtdy, width, height);
    let freq = (8.0 / (1.0 + mip_level)).max(1.0);
    let checker = ((s * freq).floor() as i32 + (t * freq).floor() as i32) & 1;
    let val = if checker == 0 { 0.2 } else { 0.8 };

    TextureResult {
        color: Color3::new(val, val, val),
        alpha: 1.0,
        ok: true,
        ..Default::default()
    }
}

/// Texture lookup for 3D textures.
pub fn texture_3d(
    _filename: &str,
    p: Vec3,
    _dpdx: Vec3,
    _dpdy: Vec3,
    _dpdz: Vec3,
    _opt: &TextureOpt,
) -> TextureResult {
    // Default: 3D procedural noise-like pattern
    let val = ((p.x * 4.0).sin() * (p.y * 4.0).sin() * (p.z * 4.0).sin() + 1.0) * 0.5;

    TextureResult {
        color: Color3::new(val, val, val),
        alpha: 1.0,
        ok: true,
        ..Default::default()
    }
}

/// Environment map lookup.
pub fn environment(
    _filename: &str,
    r: Vec3,
    _drdx: Vec3,
    _drdy: Vec3,
    _opt: &TextureOpt,
) -> TextureResult {
    // Default: simple sky dome gradient
    let up = r.normalize().y;
    let sky = Color3::new(0.3, 0.5, 0.8);
    let ground = Color3::new(0.4, 0.35, 0.3);
    let t = (up * 0.5 + 0.5).clamp(0.0, 1.0);
    let c = Color3::new(
        ground.x * (1.0 - t) + sky.x * t,
        ground.y * (1.0 - t) + sky.y * t,
        ground.z * (1.0 - t) + sky.z * t,
    );

    TextureResult {
        color: c,
        alpha: 1.0,
        ok: true,
        ..Default::default()
    }
}

/// Get information about a texture file.
pub fn gettextureinfo(_filename: &str, dataname: &str) -> Option<TextureInfo> {
    // This is a stub — actual implementation would query the texture system
    match dataname {
        "exists" => Some(TextureInfo::Int(0)), // file doesn't exist
        "resolution" => Some(TextureInfo::IntVec(vec![0, 0])),
        "channels" => Some(TextureInfo::Int(0)),
        "type" => Some(TextureInfo::Str("unknown".to_string())),
        "subimages" => Some(TextureInfo::Int(0)),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Dispatch functions: try RendererServices first, fall back to procedural.
// ---------------------------------------------------------------------------

/// 2D texture lookup via RendererServices with procedural fallback.
///
/// If `renderer` implements texture(), delegates there. Otherwise falls
/// back to the built-in procedural checker pattern.
pub fn texture_lookup(
    renderer: Option<&dyn RendererServices>,
    sg: &ShaderGlobals,
    filename: &str,
    s: Float,
    t: Float,
    dsdx: Float,
    dtdx: Float,
    dsdy: Float,
    dtdy: Float,
    opt: &TextureOpt,
) -> TextureResult {
    if let Some(rs) = renderer {
        let fh = UStringHash::from_str(filename);
        let nch = opt.nchannels.max(3);
        let mut buf = vec![0.0f32; nch as usize];
        if rs
            .texture(
                fh,
                std::ptr::null_mut(),
                sg,
                opt,
                s,
                t,
                dsdx,
                dtdx,
                dsdy,
                dtdy,
                nch,
                &mut buf,
                None,
                None,
            )
            .is_ok()
        {
            return TextureResult {
                color: Color3::new(
                    buf[0],
                    if nch > 1 { buf[1] } else { buf[0] },
                    if nch > 2 { buf[2] } else { buf[0] },
                ),
                alpha: if nch > 3 { buf[3] } else { 1.0 },
                ok: true,
                ..Default::default()
            };
        }
    }
    // Procedural fallback
    texture_2d(filename, s, t, dsdx, dtdx, dsdy, dtdy, opt)
}

/// Apply chain rule to convert texture-space color derivatives to screen-space.
///
/// dresult/d{x|y} = dresult/ds * ds/d{x|y} + dresult/dt * dt/d{x|y}
#[inline]
#[allow(dead_code)] // Used by 3D variant; 2D texture calls chain_rule_color_3d directly
fn chain_rule_color(
    dresultds: &[Float],
    dresultdt: &[Float],
    nch: i32,
    ds: Float,
    dt: Float,
) -> Color3 {
    let r = dresultds[0] * ds + dresultdt[0] * dt;
    let g = if nch > 1 {
        dresultds[1] * ds + dresultdt[1] * dt
    } else {
        r
    };
    let b = if nch > 2 {
        dresultds[2] * ds + dresultdt[2] * dt
    } else {
        r
    };
    Color3::new(r, g, b)
}

/// Apply chain rule for 3D texture: str texture-space to xyz screen-space.
#[inline]
fn chain_rule_color_3d(
    dresultds: &[Float],
    dresultdt: &[Float],
    dresultdr: &[Float],
    nch: i32,
    dp: Vec3,
) -> Color3 {
    let r = dresultds[0] * dp.x + dresultdt[0] * dp.y + dresultdr[0] * dp.z;
    let g = if nch > 1 {
        dresultds[1] * dp.x + dresultdt[1] * dp.y + dresultdr[1] * dp.z
    } else {
        r
    };
    let b = if nch > 2 {
        dresultds[2] * dp.x + dresultdt[2] * dp.y + dresultdr[2] * dp.z
    } else {
        r
    };
    Color3::new(r, g, b)
}

/// 3D texture lookup via RendererServices with procedural fallback.
///
/// Applies the chain rule for 3D: str texture-space to xyz screen-space.
///   dresult/dx = dresult/ds * dPdx.x + dresult/dt * dPdx.y + dresult/dr * dPdx.z
///   dresult/dy = dresult/ds * dPdy.x + dresult/dt * dPdy.y + dresult/dr * dPdy.z
pub fn texture3d_lookup(
    renderer: Option<&dyn RendererServices>,
    sg: &ShaderGlobals,
    filename: &str,
    p: Vec3,
    dpdx: Vec3,
    dpdy: Vec3,
    dpdz: Vec3,
    opt: &TextureOpt,
) -> TextureResult {
    if let Some(rs) = renderer {
        let fh = UStringHash::from_str(filename);
        let nch = opt.nchannels.max(3);
        let mut buf = vec![0.0f32; nch as usize];
        let mut dresultds = vec![0.0f32; nch as usize];
        let mut dresultdt = vec![0.0f32; nch as usize];
        let mut dresultdr = vec![0.0f32; nch as usize];
        if rs
            .texture3d(
                fh,
                std::ptr::null_mut(),
                sg,
                opt,
                &p,
                &dpdx,
                &dpdy,
                &dpdz,
                nch,
                &mut buf,
                Some(&mut dresultds),
                Some(&mut dresultdt),
                Some(&mut dresultdr),
            )
            .is_ok()
        {
            let color = Color3::new(
                buf[0],
                if nch > 1 { buf[1] } else { buf[0] },
                if nch > 2 { buf[2] } else { buf[0] },
            );
            let alpha = if nch > 3 { buf[3] } else { 1.0 };

            // Chain rule: str texture-space -> xyz screen-space
            let dcolor_dx = chain_rule_color_3d(&dresultds, &dresultdt, &dresultdr, nch, dpdx);
            let dcolor_dy = chain_rule_color_3d(&dresultds, &dresultdt, &dresultdr, nch, dpdy);
            let dalpha_dx = if nch > 3 {
                dresultds[3] * dpdx.x + dresultdt[3] * dpdx.y + dresultdr[3] * dpdx.z
            } else {
                0.0
            };
            let dalpha_dy = if nch > 3 {
                dresultds[3] * dpdy.x + dresultdt[3] * dpdy.y + dresultdr[3] * dpdy.z
            } else {
                0.0
            };

            return TextureResult {
                color,
                alpha,
                ok: true,
                dcolor_dx,
                dcolor_dy,
                dalpha_dx,
                dalpha_dy,
            };
        }
    }
    texture_3d(filename, p, dpdx, dpdy, dpdz, opt)
}

/// Environment map lookup via RendererServices with procedural fallback.
pub fn environment_lookup(
    renderer: Option<&dyn RendererServices>,
    sg: &ShaderGlobals,
    filename: &str,
    r: Vec3,
    drdx: Vec3,
    drdy: Vec3,
    opt: &TextureOpt,
) -> TextureResult {
    if let Some(rs) = renderer {
        let fh = UStringHash::from_str(filename);
        let nch = opt.nchannels.max(3);
        let mut buf = vec![0.0f32; nch as usize];
        if rs
            .environment(
                fh,
                std::ptr::null_mut(),
                sg,
                opt,
                &r,
                &drdx,
                &drdy,
                nch,
                &mut buf,
                None,
                None,
            )
            .is_ok()
        {
            return TextureResult {
                color: Color3::new(
                    buf[0],
                    if nch > 1 { buf[1] } else { buf[0] },
                    if nch > 2 { buf[2] } else { buf[0] },
                ),
                alpha: if nch > 3 { buf[3] } else { 1.0 },
                ok: true,
                ..Default::default()
            };
        }
    }
    environment(filename, r, drdx, drdy, opt)
}

/// Query texture info via RendererServices with stub fallback.
pub fn gettextureinfo_lookup(
    renderer: Option<&dyn RendererServices>,
    sg: &ShaderGlobals,
    filename: &str,
    dataname: &str,
) -> Option<TextureInfo> {
    if let Some(rs) = renderer {
        // Intern strings so UString::from_hash can resolve them back
        let fus = UString::new(filename);
        let dus = UString::new(dataname);
        let fh = UStringHash::from_hash(fus.hash());
        let dh = UStringHash::from_hash(dus.hash());
        // Try int query first
        let mut ival = 0i32;
        if rs
            .get_texture_info(
                fh,
                std::ptr::null_mut(),
                sg,
                0,
                dh,
                crate::typedesc::TypeDesc::INT,
                &mut ival as *mut _ as *mut std::ffi::c_void,
            )
            .is_ok()
        {
            return Some(TextureInfo::Int(ival));
        }
        // Try float query
        let mut fval = 0.0f32;
        if rs
            .get_texture_info(
                fh,
                std::ptr::null_mut(),
                sg,
                0,
                dh,
                crate::typedesc::TypeDesc::FLOAT,
                &mut fval as *mut _ as *mut std::ffi::c_void,
            )
            .is_ok()
        {
            return Some(TextureInfo::Float(fval));
        }
        // Try string query
        let mut sval = UString::empty();
        if rs
            .get_texture_info(
                fh,
                std::ptr::null_mut(),
                sg,
                0,
                dh,
                crate::typedesc::TypeDesc::STRING,
                &mut sval as *mut _ as *mut std::ffi::c_void,
            )
            .is_ok()
        {
            return Some(TextureInfo::Str(sval.as_str().to_string()));
        }
    }
    // Stub fallback
    gettextureinfo(filename, dataname)
}

/// Info returned from gettextureinfo.
#[derive(Debug, Clone)]
pub enum TextureInfo {
    Int(i32),
    Float(f32),
    Str(String),
    IntVec(Vec<i32>),
    FloatVec(Vec<f32>),
}

/// Apply wrapping to a texture coordinate.
fn wrap_coord(x: Float, wrap: TextureWrap) -> Float {
    match wrap {
        TextureWrap::Clamp => x.clamp(0.0, 1.0),
        TextureWrap::Periodic | TextureWrap::PeriodicPow2 | TextureWrap::PeriodicSharedBorder => {
            x - x.floor()
        }
        TextureWrap::Mirror => {
            let t = x - 2.0 * (x * 0.5).floor();
            1.0 - (t - 1.0).abs()
        }
        TextureWrap::Black => x, // Out-of-range returns black (handled by caller)
        TextureWrap::Default => x.clamp(0.0, 1.0),
    }
}

/// A cache entry for texture handles.
#[derive(Debug)]
pub struct TextureCache {
    entries: std::collections::HashMap<UString, CachedTexture>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct CachedTexture {
    filename: UString,
    resolution: [i32; 2],
    channels: i32,
}

impl TextureCache {
    pub fn new() -> Self {
        Self {
            entries: std::collections::HashMap::new(),
        }
    }

    /// Look up a cached texture entry by filename.
    pub fn lookup(&self, filename: UString) -> Option<&CachedTexture> {
        self.entries.get(&filename)
    }

    pub fn insert(&mut self, filename: UString, resolution: [i32; 2], channels: i32) {
        self.entries.insert(
            filename,
            CachedTexture {
                filename,
                resolution,
                channels,
            },
        );
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wrap_coord_clamp() {
        assert_eq!(wrap_coord(-0.5, TextureWrap::Clamp), 0.0);
        assert_eq!(wrap_coord(1.5, TextureWrap::Clamp), 1.0);
        assert_eq!(wrap_coord(0.5, TextureWrap::Clamp), 0.5);
    }

    #[test]
    fn test_wrap_coord_periodic() {
        let w = wrap_coord(1.3, TextureWrap::Periodic);
        assert!((w - 0.3).abs() < 1e-5);
        let w2 = wrap_coord(-0.7, TextureWrap::Periodic);
        assert!((w2 - 0.3).abs() < 1e-5);
    }

    #[test]
    fn test_texture_2d_default() {
        let opt = TextureOpt::default();
        let result = texture_2d("test.exr", 0.5, 0.5, 0.0, 0.0, 0.0, 0.0, &opt);
        assert!(result.ok);
    }

    #[test]
    fn test_environment_default() {
        let opt = TextureOpt::default();
        let result = environment(
            "env.hdr",
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::ZERO,
            Vec3::ZERO,
            &opt,
        );
        assert!(result.ok);
        assert!(result.color.y > 0.4); // Looking up: more sky-blue
    }

    #[test]
    fn test_texture_cache() {
        let mut cache = TextureCache::new();
        let name = UString::new("test.exr");
        cache.insert(name, [1024, 1024], 4);
        assert!(cache.lookup(name).is_some());
    }

    // --- Dispatch function tests ---

    #[test]
    fn test_texture_lookup_no_renderer() {
        // Without renderer, falls back to procedural checker
        let sg = ShaderGlobals::default();
        let opt = TextureOpt::default();
        let r = texture_lookup(None, &sg, "test.exr", 0.5, 0.5, 0.0, 0.0, 0.0, 0.0, &opt);
        assert!(r.ok);
        // Procedural checker returns 0.2 or 0.8
        assert!(r.color.x == 0.2 || r.color.x == 0.8);
    }

    #[test]
    fn test_texture_lookup_with_basic_renderer() {
        use crate::renderer::BasicRenderer;
        let br = BasicRenderer::new();
        let sg = ShaderGlobals::default();
        let opt = TextureOpt::default();
        let r = texture_lookup(
            Some(&br),
            &sg,
            "test.exr",
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            &opt,
        );
        assert!(r.ok);
        // BasicRenderer checker: (0*8).floor() + (0*8).floor() = 0, val=0.0
        assert_eq!(r.color.x, 0.0);
    }

    #[test]
    fn test_texture3d_lookup_no_renderer() {
        let sg = ShaderGlobals::default();
        let opt = TextureOpt::default();
        let r = texture3d_lookup(
            None,
            &sg,
            "vol.exr",
            Vec3::new(0.5, 0.5, 0.5),
            Vec3::ZERO,
            Vec3::ZERO,
            Vec3::ZERO,
            &opt,
        );
        assert!(r.ok);
    }

    #[test]
    fn test_texture3d_lookup_with_basic_renderer() {
        use crate::renderer::BasicRenderer;
        let br = BasicRenderer::new();
        let sg = ShaderGlobals::default();
        let opt = TextureOpt::default();
        let r = texture3d_lookup(
            Some(&br),
            &sg,
            "vol.exr",
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::ZERO,
            Vec3::ZERO,
            Vec3::ZERO,
            &opt,
        );
        assert!(r.ok);
    }

    #[test]
    fn test_environment_lookup_no_renderer() {
        let sg = ShaderGlobals::default();
        let opt = TextureOpt::default();
        let r = environment_lookup(
            None,
            &sg,
            "env.hdr",
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::ZERO,
            Vec3::ZERO,
            &opt,
        );
        assert!(r.ok);
        // Looking up -> sky color
        assert!(r.color.y > 0.4);
    }

    #[test]
    fn test_environment_lookup_with_basic_renderer() {
        use crate::renderer::BasicRenderer;
        let br = BasicRenderer::new();
        let sg = ShaderGlobals::default();
        let opt = TextureOpt::default();
        let r = environment_lookup(
            Some(&br),
            &sg,
            "env.hdr",
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::ZERO,
            Vec3::ZERO,
            &opt,
        );
        assert!(r.ok);
        // BasicRenderer sky gradient: +Y -> blueish
        assert!(r.color.z > r.color.x);
    }

    #[test]
    fn test_gettextureinfo_lookup_no_renderer() {
        let sg = ShaderGlobals::default();
        let info = gettextureinfo_lookup(None, &sg, "test.exr", "exists");
        assert!(info.is_some());
        match info.unwrap() {
            TextureInfo::Int(v) => assert_eq!(v, 0), // stub says not found
            _ => panic!("expected Int"),
        }
    }

    #[test]
    fn test_gettextureinfo_lookup_with_basic_renderer() {
        use crate::renderer::BasicRenderer;
        let br = BasicRenderer::new();
        let sg = ShaderGlobals::default();
        // BasicRenderer reports exists=1 for any texture
        let info = gettextureinfo_lookup(Some(&br), &sg, "test.exr", "exists");
        assert!(info.is_some());
        match info.unwrap() {
            TextureInfo::Int(v) => assert_eq!(v, 1),
            _ => panic!("expected Int"),
        }
    }

    #[test]
    fn test_gettextureinfo_lookup_channels() {
        use crate::renderer::BasicRenderer;
        let br = BasicRenderer::new();
        let sg = ShaderGlobals::default();
        let info = gettextureinfo_lookup(Some(&br), &sg, "test.exr", "channels");
        assert!(info.is_some());
        match info.unwrap() {
            TextureInfo::Int(v) => assert_eq!(v, 3),
            _ => panic!("expected Int"),
        }
    }

    // --- New wrap mode tests ---

    #[test]
    fn test_wrap_periodic_pow2() {
        let w = wrap_coord(1.3, TextureWrap::PeriodicPow2);
        assert!((w - 0.3).abs() < 1e-5);
    }

    #[test]
    fn test_wrap_periodic_shared_border() {
        let w = wrap_coord(2.7, TextureWrap::PeriodicSharedBorder);
        assert!((w - 0.7).abs() < 1e-5);
    }

    #[test]
    fn test_wrap_from_str_new_modes() {
        assert_eq!(
            TextureWrap::from_str("periodic_pow2"),
            TextureWrap::PeriodicPow2
        );
        assert_eq!(
            TextureWrap::from_str("periodic_sharedborder"),
            TextureWrap::PeriodicSharedBorder
        );
    }

    #[test]
    fn test_texture_opt_rblur_rwidth() {
        let opt = TextureOpt::default();
        assert_eq!(opt.rblur, 0.0);
        assert_eq!(opt.rwidth, 1.0);
    }

    #[test]
    fn test_mip_level_from_derivs() {
        // Zero derivs -> level 0
        let mip = mip_level_from_derivs(0.0, 0.0, 0.0, 0.0, 256, 256);
        assert!((mip - 0.0).abs() < 1e-6);
        // Large derivs -> higher level
        let mip_large = mip_level_from_derivs(0.1, 0.1, 0.1, 0.1, 256, 256);
        assert!(mip_large > 2.0);
    }

    #[test]
    fn test_parse_texture_options() {
        use super::TextureOptArg;
        let opt = parse_texture_options(vec![
            ("swrap".to_string(), TextureOptArg::Str("clamp".to_string())),
            (
                "twrap".to_string(),
                TextureOptArg::Str("periodic".to_string()),
            ),
            ("fill".to_string(), TextureOptArg::Float(0.5)),
            ("firstchannel".to_string(), TextureOptArg::Int(1)),
        ]);
        assert_eq!(opt.swrap, TextureWrap::Clamp);
        assert_eq!(opt.twrap, TextureWrap::Periodic);
        assert!((opt.fill - 0.5).abs() < 1e-6);
        assert_eq!(opt.firstchannel, 1);
        // wrap sets all three
        let opt2 = parse_texture_options(vec![(
            "wrap".to_string(),
            TextureOptArg::Str("mirror".to_string()),
        )]);
        assert_eq!(opt2.swrap, TextureWrap::Mirror);
        assert_eq!(opt2.twrap, TextureWrap::Mirror);
        assert_eq!(opt2.rwrap, TextureWrap::Mirror);
    }
}
