//! HdStLight - Storm light implementation.
//!
//! Manages lighting for Storm rendering. Converts USD light types
//! (sphere, rect, disk, cylinder, distant, dome) into GlfSimpleLight
//! representations for the simple lighting pipeline.
//!
//! Port of pxr/imaging/hdSt/light.cpp from OpenUSD.

use usd_gf::{Matrix4d, Vec3f, Vec4f};
use usd_glf::GlfSimpleLight;
use usd_sdf::Path as SdfPath;
use usd_tf::Token;
use usd_vt::Value;

use std::collections::HashMap;
use std::sync::{Arc, LazyLock};

// ---------------------------------------------------------------------------
// Blackbody color temperature table
// ---------------------------------------------------------------------------

/// Blackbody RGB lookup table from John Walker's specrend.c.
///
/// Covers 1000K..10000K in 500K steps, Rec709/sRGB chromaticity.
/// Repeated boundary knots for Catmull-Rom boundary handling.
static BLACKBODY_RGB: &[Vec3f] = &[
    Vec3f {
        x: 1.000000,
        y: 0.027490,
        z: 0.000000,
    }, // 1000 K (Approximation)
    Vec3f {
        x: 1.000000,
        y: 0.027490,
        z: 0.000000,
    }, // 1000 K (Approximation)
    Vec3f {
        x: 1.000000,
        y: 0.149664,
        z: 0.000000,
    }, // 1500 K (Approximation)
    Vec3f {
        x: 1.000000,
        y: 0.256644,
        z: 0.008095,
    }, // 2000 K
    Vec3f {
        x: 1.000000,
        y: 0.372033,
        z: 0.067450,
    }, // 2500 K
    Vec3f {
        x: 1.000000,
        y: 0.476725,
        z: 0.153601,
    }, // 3000 K
    Vec3f {
        x: 1.000000,
        y: 0.570376,
        z: 0.259196,
    }, // 3500 K
    Vec3f {
        x: 1.000000,
        y: 0.653480,
        z: 0.377155,
    }, // 4000 K
    Vec3f {
        x: 1.000000,
        y: 0.726878,
        z: 0.501606,
    }, // 4500 K
    Vec3f {
        x: 1.000000,
        y: 0.791543,
        z: 0.628050,
    }, // 5000 K
    Vec3f {
        x: 1.000000,
        y: 0.848462,
        z: 0.753228,
    }, // 5500 K
    Vec3f {
        x: 1.000000,
        y: 0.898581,
        z: 0.874905,
    }, // 6000 K
    Vec3f {
        x: 1.000000,
        y: 0.942771,
        z: 0.991642,
    }, // 6500 K
    Vec3f {
        x: 0.906947,
        y: 0.890456,
        z: 1.000000,
    }, // 7000 K
    Vec3f {
        x: 0.828247,
        y: 0.841838,
        z: 1.000000,
    }, // 7500 K
    Vec3f {
        x: 0.765791,
        y: 0.801896,
        z: 1.000000,
    }, // 8000 K
    Vec3f {
        x: 0.715255,
        y: 0.768579,
        z: 1.000000,
    }, // 8500 K
    Vec3f {
        x: 0.673683,
        y: 0.740423,
        z: 1.000000,
    }, // 9000 K
    Vec3f {
        x: 0.638992,
        y: 0.716359,
        z: 1.000000,
    }, // 9500 K
    Vec3f {
        x: 0.609681,
        y: 0.695588,
        z: 1.000000,
    }, // 10000 K
    Vec3f {
        x: 0.609681,
        y: 0.695588,
        z: 1.000000,
    }, // 10000 K
    Vec3f {
        x: 0.609681,
        y: 0.695588,
        z: 1.000000,
    }, // 10000 K
];

/// Catmull-Rom basis matrix coefficients (4x4).
const CR_BASIS: [[f32; 4]; 4] = [
    [-0.5, 1.5, -1.5, 0.5],
    [1.0, -2.5, 2.0, -0.5],
    [-0.5, 0.0, 0.5, 0.0],
    [0.0, 1.0, 0.0, 0.0],
];

/// Rec709 luma coefficients.
const REC709_LUMA: Vec3f = Vec3f {
    x: 0.2126,
    y: 0.7152,
    z: 0.0722,
};

fn vec3f_dot(a: Vec3f, b: Vec3f) -> f32 {
    a.x * b.x + a.y * b.y + a.z * b.z
}

fn vec3f_comp_mult(a: Vec3f, b: Vec3f) -> Vec3f {
    Vec3f {
        x: a.x * b.x,
        y: a.y * b.y,
        z: a.z * b.z,
    }
}

/// Convert a blackbody color temperature (Kelvin) to an RGB color.
///
/// Uses Catmull-Rom interpolation of the blackbody lookup table,
/// normalized to Rec709 luminance so it doesn't affect overall brightness.
fn blackbody_temp_as_rgb(temp: f32) -> Vec3f {
    let num_knots = BLACKBODY_RGB.len();
    let u_spline = ((temp - 1000.0) / 9000.0).clamp(0.0, 1.0);
    let num_segs = (num_knots - 4) as f32;
    let x = u_spline * num_segs;
    let seg = x.floor() as usize;
    let u_seg = x - seg as f32;

    let k0 = BLACKBODY_RGB[seg];
    let k1 = BLACKBODY_RGB[seg + 1];
    let k2 = BLACKBODY_RGB[seg + 2];
    let k3 = BLACKBODY_RGB[seg + 3];

    // Compute Catmull-Rom cubic coefficients
    let coeff_k = |row: usize| {
        CR_BASIS[row][0] * k0.x
            + CR_BASIS[row][1] * k1.x
            + CR_BASIS[row][2] * k2.x
            + CR_BASIS[row][3] * k3.x
    };
    let coeff_ky = |row: usize| {
        CR_BASIS[row][0] * k0.y
            + CR_BASIS[row][1] * k1.y
            + CR_BASIS[row][2] * k2.y
            + CR_BASIS[row][3] * k3.y
    };
    let coeff_kz = |row: usize| {
        CR_BASIS[row][0] * k0.z
            + CR_BASIS[row][1] * k1.z
            + CR_BASIS[row][2] * k2.z
            + CR_BASIS[row][3] * k3.z
    };

    let eval_cubic = |a: f32, b: f32, c: f32, d: f32| ((a * u_seg + b) * u_seg + c) * u_seg + d;

    let rgb = Vec3f {
        x: eval_cubic(coeff_k(0), coeff_k(1), coeff_k(2), coeff_k(3)),
        y: eval_cubic(coeff_ky(0), coeff_ky(1), coeff_ky(2), coeff_ky(3)),
        z: eval_cubic(coeff_kz(0), coeff_kz(1), coeff_kz(2), coeff_kz(3)),
    };

    // Normalize to Rec709 luma so color temperature doesn't scale brightness
    let luma = vec3f_dot(rgb, REC709_LUMA);
    let scale = if luma > 1e-6 { 1.0 / luma } else { 1.0 };
    Vec3f {
        x: (rgb.x * scale).max(0.0),
        y: (rgb.y * scale).max(0.0),
        z: (rgb.z * scale).max(0.0),
    }
}

// ---------------------------------------------------------------------------
// Token constants
// ---------------------------------------------------------------------------

// Light parameter tokens (HdLightTokens equivalents)
static TOK_PARAMS: LazyLock<Token> = LazyLock::new(|| Token::new("params"));
static TOK_TRANSFORM: LazyLock<Token> = LazyLock::new(|| Token::new("transform"));
// Used for light filter dependency tracking (future shadow collection support)
#[allow(dead_code)]
static TOK_FILTERS: LazyLock<Token> = LazyLock::new(|| Token::new("filters"));
static TOK_SHADOW_PARAMS: LazyLock<Token> = LazyLock::new(|| Token::new("shadowParams"));
// Used for shadow collection change tracking (future support)
#[allow(dead_code)]
static TOK_SHADOW_COLLECTION: LazyLock<Token> = LazyLock::new(|| Token::new("shadowCollection"));

// Light parameter query tokens
static TOK_COLOR: LazyLock<Token> = LazyLock::new(|| Token::new("color"));
static TOK_INTENSITY: LazyLock<Token> = LazyLock::new(|| Token::new("intensity"));
static TOK_EXPOSURE: LazyLock<Token> = LazyLock::new(|| Token::new("exposure"));
static TOK_DIFFUSE: LazyLock<Token> = LazyLock::new(|| Token::new("diffuse"));
static TOK_SPECULAR: LazyLock<Token> = LazyLock::new(|| Token::new("specular"));
static TOK_AMBIENT: LazyLock<Token> = LazyLock::new(|| Token::new("ambient"));
static TOK_NORMALIZE: LazyLock<Token> = LazyLock::new(|| Token::new("normalize"));
static TOK_ENABLE_COLOR_TEMP: LazyLock<Token> =
    LazyLock::new(|| Token::new("enableColorTemperature"));
static TOK_COLOR_TEMP: LazyLock<Token> = LazyLock::new(|| Token::new("colorTemperature"));
static TOK_HAS_SHADOW: LazyLock<Token> = LazyLock::new(|| Token::new("hasShadow"));
static TOK_RADIUS: LazyLock<Token> = LazyLock::new(|| Token::new("radius"));
static TOK_WIDTH: LazyLock<Token> = LazyLock::new(|| Token::new("width"));
static TOK_HEIGHT: LazyLock<Token> = LazyLock::new(|| Token::new("height"));
static TOK_LENGTH: LazyLock<Token> = LazyLock::new(|| Token::new("length"));
static TOK_ANGLE: LazyLock<Token> = LazyLock::new(|| Token::new("angle"));
static TOK_TEXTURE_FILE: LazyLock<Token> = LazyLock::new(|| Token::new("texture:file"));
static TOK_SHAPING_CONE_ANGLE: LazyLock<Token> = LazyLock::new(|| Token::new("shaping:cone:angle"));
static TOK_SHAPING_FOCUS: LazyLock<Token> = LazyLock::new(|| Token::new("shaping:focus"));
static TOK_DOME_OFFSET: LazyLock<Token> = LazyLock::new(|| Token::new("domeOffset"));

// Shadow sub-parameter tokens (HdLightTokens shadow:* sub-namespace)
static TOK_SHADOW_ENABLE: LazyLock<Token> = LazyLock::new(|| Token::new("shadow:enable"));
static TOK_SHADOW_RESOLUTION: LazyLock<Token> = LazyLock::new(|| Token::new("shadow:resolution"));
static TOK_SHADOW_BLUR: LazyLock<Token> = LazyLock::new(|| Token::new("shadow:blur"));
static TOK_SHADOW_BIAS: LazyLock<Token> = LazyLock::new(|| Token::new("shadow:bias"));

// Light type tokens (HdPrimTypeTokens equivalents)
static LTYPE_SIMPLE: LazyLock<Token> = LazyLock::new(|| Token::new("simpleLight"));
static LTYPE_DOME: LazyLock<Token> = LazyLock::new(|| Token::new("domeLight"));
static LTYPE_DISTANT: LazyLock<Token> = LazyLock::new(|| Token::new("distantLight"));
static LTYPE_SPHERE: LazyLock<Token> = LazyLock::new(|| Token::new("sphereLight"));
static LTYPE_RECT: LazyLock<Token> = LazyLock::new(|| Token::new("rectLight"));
static LTYPE_DISK: LazyLock<Token> = LazyLock::new(|| Token::new("diskLight"));
static LTYPE_CYLINDER: LazyLock<Token> = LazyLock::new(|| Token::new("cylinderLight"));

// ---------------------------------------------------------------------------
// Dirty bit flags (match C++ HdLight dirty bits)
// ---------------------------------------------------------------------------

pub type HdDirtyBits = u32;

pub const DIRTY_TRANSFORM: HdDirtyBits = 1 << 0;
pub const DIRTY_PARAMS: HdDirtyBits = 1 << 1;
pub const DIRTY_SHADOW_PARAMS: HdDirtyBits = 1 << 2;
pub const DIRTY_COLLECTION: HdDirtyBits = 1 << 3;
pub const ALL_DIRTY: HdDirtyBits =
    DIRTY_TRANSFORM | DIRTY_PARAMS | DIRTY_SHADOW_PARAMS | DIRTY_COLLECTION;
pub const CLEAN: HdDirtyBits = 0;

// ---------------------------------------------------------------------------
// HdSceneDelegate interface (minimal, matches C++ usage in Sync)
// ---------------------------------------------------------------------------

/// Minimal scene delegate interface needed by HdStLight::sync.
///
/// Matches the subset of HdSceneDelegate used in C++ light.cpp.
pub trait LightSceneDelegate: Send + Sync {
    /// Get the named value for a light param.
    fn get_light_param_value(&self, id: &SdfPath, param: &Token) -> Value;
    /// Get the world transform for a prim.
    fn get_transform(&self, id: &SdfPath) -> Matrix4d;
    /// Get the visibility state.
    fn get_visible(&self, id: &SdfPath) -> bool;
    /// Get named value (for HdTokens->params, HdTokens->filters, etc.).
    fn get(&self, id: &SdfPath, key: &Token) -> Value;
}

// ---------------------------------------------------------------------------
// HdStShadowMap
// ---------------------------------------------------------------------------

/// Shadow map descriptor for a Storm light.
#[derive(Debug, Clone)]
pub struct HdStShadowMap {
    /// Shadow map resolution
    pub resolution: (u32, u32),
    /// Shadow map texture handle
    pub texture_handle: u64,
    /// Shadow bias
    pub bias: f32,
    /// Whether shadow map is allocated
    pub allocated: bool,
}

impl HdStShadowMap {
    /// Create a new shadow map descriptor.
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            resolution: (width, height),
            texture_handle: 0,
            bias: 0.005,
            allocated: false,
        }
    }

    /// Allocate shadow map resources.
    pub fn allocate(&mut self) {
        // Note: full impl would create GPU texture via Hgi
        self.texture_handle = 1; // placeholder
        self.allocated = true;
    }

    /// Free shadow map resources.
    pub fn free(&mut self) {
        self.texture_handle = 0;
        self.allocated = false;
    }
}

// ---------------------------------------------------------------------------
// HdStLight
// ---------------------------------------------------------------------------

/// Storm light representation.
///
/// Manages light parameters by syncing from the scene delegate and caching
/// everything as `GlfSimpleLight` in the params map. Area lights (sphere,
/// rect, disk, cylinder, distant) are approximated as simple point/directional
/// sources. Dome lights are passed through with their texture path for IBL.
///
/// Port of pxr/imaging/hdSt/light.h/cpp.
#[derive(Debug)]
pub struct HdStLight {
    /// Prim path
    path: SdfPath,
    /// Light type token (domeLight, distantLight, sphereLight, etc.)
    light_type: Token,
    /// Cached parameters: maps token -> VtValue (GlfSimpleLight, Matrix4d, etc.)
    params: HashMap<Token, Value>,
    /// Optional shadow map
    shadow_map: Option<HdStShadowMap>,
    /// Shadow parameters for this light (bias, blur, matrix).
    /// Populated during sync() when DIRTY_SHADOW_PARAMS is set.
    shadow_params: crate::lighting::ShadowParams,
}

impl HdStLight {
    /// Create a new HdStLight.
    pub fn new(path: SdfPath, light_type: Token) -> Self {
        Self {
            path,
            light_type,
            params: HashMap::new(),
            shadow_map: None,
            shadow_params: crate::lighting::ShadowParams::default(),
        }
    }

    /// Create a distant (directional) light.
    pub fn new_distant(path: SdfPath) -> Self {
        Self::new(path, LTYPE_DISTANT.clone())
    }

    /// Create a dome (environment) light.
    pub fn new_dome(path: SdfPath) -> Self {
        Self::new(path, LTYPE_DOME.clone())
    }

    /// Create a sphere (point) light.
    pub fn new_sphere(path: SdfPath) -> Self {
        Self::new(path, LTYPE_SPHERE.clone())
    }

    /// Create a rect (area) light.
    pub fn new_rect(path: SdfPath) -> Self {
        Self::new(path, LTYPE_RECT.clone())
    }

    // -----------------------------------------------------------------------
    // Accessors
    // -----------------------------------------------------------------------

    /// Get prim path.
    pub fn get_path(&self) -> &SdfPath {
        &self.path
    }

    /// Get light type token.
    pub fn get_light_type(&self) -> &Token {
        &self.light_type
    }

    /// Get cached param value (mirrors C++ `Get()`).
    pub fn get(&self, token: &Token) -> Value {
        self.params.get(token).cloned().unwrap_or_default()
    }

    /// Get all cached params.
    pub fn get_params(&self) -> &HashMap<Token, Value> {
        &self.params
    }

    /// Get the GlfSimpleLight cached in params (result of Sync).
    pub fn get_simple_light(&self) -> Option<GlfSimpleLight> {
        self.params
            .get(&TOK_PARAMS)?
            .get::<GlfSimpleLight>()
            .cloned()
    }

    /// Get shadow map.
    pub fn get_shadow_map(&self) -> Option<&HdStShadowMap> {
        self.shadow_map.as_ref()
    }

    /// Get shadow parameters for this light.
    pub fn get_shadow_params(&self) -> &crate::lighting::ShadowParams {
        &self.shadow_params
    }

    /// Set shadow parameters for this light.
    pub fn set_shadow_params(&mut self, params: crate::lighting::ShadowParams) {
        self.shadow_params = params;
    }

    /// Returns initial dirty bits mask.
    ///
    /// SimpleLight and DistantLight get ALL dirty bits on first sync.
    /// Other lights only need params and transform initially.
    pub fn get_initial_dirty_bits_mask(&self) -> HdDirtyBits {
        if self.light_type == *LTYPE_SIMPLE || self.light_type == *LTYPE_DISTANT {
            ALL_DIRTY
        } else {
            DIRTY_PARAMS | DIRTY_TRANSFORM
        }
    }

    // -----------------------------------------------------------------------
    // Private helpers: light type conversion
    // -----------------------------------------------------------------------

    /// Read common color + color temperature from delegate, return final RGB.
    fn read_color(&self, id: &SdfPath, delegate: &dyn LightSceneDelegate) -> Vec3f {
        let hdc = delegate
            .get_light_param_value(id, &TOK_COLOR)
            .get::<Vec3f>()
            .copied()
            .unwrap_or(Vec3f {
                x: 1.0,
                y: 1.0,
                z: 1.0,
            });

        // Apply color temperature if enabled
        let enable_ct = delegate
            .get_light_param_value(id, &TOK_ENABLE_COLOR_TEMP)
            .get::<bool>()
            .copied()
            .unwrap_or(false);

        if enable_ct {
            if let Some(temp) = delegate
                .get_light_param_value(id, &TOK_COLOR_TEMP)
                .get::<f32>()
                .copied()
            {
                return vec3f_comp_mult(hdc, blackbody_temp_as_rgb(temp));
            }
        }
        hdc
    }

    /// Compute effective intensity = intensity * 2^clamp(exposure, -50, 50).
    fn read_intensity(&self, id: &SdfPath, delegate: &dyn LightSceneDelegate) -> f32 {
        let intensity = delegate
            .get_light_param_value(id, &TOK_INTENSITY)
            .get::<f32>()
            .copied()
            .unwrap_or(1.0);
        let exposure = delegate
            .get_light_param_value(id, &TOK_EXPOSURE)
            .get::<f32>()
            .copied()
            .unwrap_or(0.0);
        intensity * 2.0f32.powf(exposure.clamp(-50.0, 50.0))
    }

    /// Convert area light (sphere/rect/disk/cylinder/distant) to GlfSimpleLight.
    ///
    /// Port of C++ `HdStLight::_ApproximateAreaLight`.
    fn approximate_area_light(
        &self,
        id: &SdfPath,
        delegate: &dyn LightSceneDelegate,
    ) -> GlfSimpleLight {
        // Invisible light: return black light with no intensity
        if !delegate.get_visible(id) {
            let mut l = GlfSimpleLight::default();
            l.set_ambient(Vec4f::new(0.0, 0.0, 0.0, 0.0));
            l.set_diffuse(Vec4f::new(0.0, 0.0, 0.0, 0.0));
            l.set_specular(Vec4f::new(0.0, 0.0, 0.0, 0.0));
            l.set_has_intensity(false);
            return l;
        }

        let hdc = self.read_color(id, delegate);
        let mut intensity = self.read_intensity(id, delegate);

        // If not normalizing: scale intensity by effective facing area
        let normalize = delegate
            .get_light_param_value(id, &TOK_NORMALIZE)
            .get::<bool>()
            .copied()
            .unwrap_or(false);

        if !normalize {
            let area = self.compute_area(id, delegate);
            intensity *= area;
        }

        let c = Vec4f::new(hdc.x * intensity, hdc.y * intensity, hdc.z * intensity, 1.0);

        let ambient_mul = delegate
            .get_light_param_value(id, &TOK_AMBIENT)
            .get::<f32>()
            .copied()
            .unwrap_or(0.0);
        let diffuse_mul = delegate
            .get_light_param_value(id, &TOK_DIFFUSE)
            .get::<f32>()
            .copied()
            .unwrap_or(1.0);
        let specular_mul = delegate
            .get_light_param_value(id, &TOK_SPECULAR)
            .get::<f32>()
            .copied()
            .unwrap_or(1.0);

        let shaping_cone = delegate
            .get_light_param_value(id, &TOK_SHAPING_CONE_ANGLE)
            .get::<f32>()
            .copied()
            .unwrap_or(90.0);
        let shaping_focus = delegate
            .get_light_param_value(id, &TOK_SHAPING_FOCUS)
            .get::<f32>()
            .copied()
            .unwrap_or(0.0);

        let has_shadow = delegate
            .get_light_param_value(id, &TOK_HAS_SHADOW)
            .get::<bool>()
            .copied()
            .unwrap_or(false);

        let mut l = GlfSimpleLight::default();
        l.set_has_intensity(intensity != 0.0);
        l.set_ambient(scale_vec4(ambient_mul, c));
        l.set_diffuse(scale_vec4(diffuse_mul, c));
        l.set_specular(scale_vec4(specular_mul, c));
        l.set_has_shadow(has_shadow);

        // Spot shaping for rect/disk lights
        if self.light_type == *LTYPE_RECT || self.light_type == *LTYPE_DISK {
            l.set_spot_cutoff(shaping_cone);
            l.set_spot_falloff(shaping_focus.max(0.0));
        }

        // Attenuation: distant lights have none; all others use 1/d^2
        if self.light_type == *LTYPE_DISTANT {
            l.set_attenuation(Vec3f::new(0.0, 0.0, 0.0));
        } else {
            l.set_attenuation(Vec3f::new(0.0, 0.0, 1.0));
        }

        l
    }

    /// Compute the effective facing-area for non-normalized area lights.
    fn compute_area(&self, id: &SdfPath, delegate: &dyn LightSceneDelegate) -> f32 {
        let get_f32 = |tok: &Token| {
            delegate
                .get_light_param_value(id, tok)
                .get::<f32>()
                .copied()
        };

        if self.light_type == *LTYPE_SPHERE {
            // Sphere surface area = 4 * pi * r^2 (not pi * r^2 which is disk area).
            if let Some(r) = get_f32(&TOK_RADIUS) {
                return 4.0 * r * r * std::f32::consts::PI;
            }
        } else if self.light_type == *LTYPE_DISK {
            // Disk area = pi * r^2
            if let Some(r) = get_f32(&TOK_RADIUS) {
                return r * r * std::f32::consts::PI;
            }
        } else if self.light_type == *LTYPE_RECT {
            // width * height
            let w = get_f32(&TOK_WIDTH).unwrap_or(1.0);
            let h = get_f32(&TOK_HEIGHT).unwrap_or(1.0);
            return w * h;
        } else if self.light_type == *LTYPE_CYLINDER {
            // 2 * pi * r * length  (lateral surface)
            let r = get_f32(&TOK_RADIUS).unwrap_or(1.0);
            let len = get_f32(&TOK_LENGTH).unwrap_or(1.0);
            return 2.0 * std::f32::consts::PI * r * len;
        } else if self.light_type == *LTYPE_DISTANT {
            // USD `angle` attribute is the full cone angle (apex-to-apex), so half_angle = angle/2.
            // Solid angle Omega = 2*pi*(1 - cos(half_angle)).
            // C++ HdStLight uses the same convention (UsdLuxDistantLight::GetAngleAttr).
            if let Some(angle_deg) = get_f32(&TOK_ANGLE) {
                let half_angle_rad = (angle_deg / 2.0) / 180.0 * std::f32::consts::PI;
                let solid_angle = 2.0 * std::f32::consts::PI * (1.0 - half_angle_rad.cos());
                return solid_angle;
            }
        }
        1.0 // default area = 1 (no scaling)
    }

    /// Prepare a dome/IBL light from delegate params.
    ///
    /// Port of C++ `HdStLight::_PrepareDomeLight`.
    fn prepare_dome_light(
        &self,
        id: &SdfPath,
        delegate: &dyn LightSceneDelegate,
    ) -> GlfSimpleLight {
        let mut l = GlfSimpleLight::default();
        l.set_has_shadow(false);
        l.set_is_dome_light(true);
        l.set_attenuation(Vec3f::new(0.0, 0.0, 0.0));

        // Invisible or zero intensity: return with has_intensity=false
        if !delegate.get_visible(id) {
            l.set_has_intensity(false);
            return l;
        }
        let raw_intensity = delegate
            .get_light_param_value(id, &TOK_INTENSITY)
            .get::<f32>()
            .copied()
            .unwrap_or(1.0);
        if raw_intensity == 0.0 {
            l.set_has_intensity(false);
            return l;
        }

        let hdc = self.read_color(id, delegate);
        let intensity = self.read_intensity(id, delegate);

        let c = Vec4f::new(hdc.x * intensity, hdc.y * intensity, hdc.z * intensity, 1.0);

        let diffuse_mul = delegate
            .get_light_param_value(id, &TOK_DIFFUSE)
            .get::<f32>()
            .copied()
            .unwrap_or(1.0);
        let specular_mul = delegate
            .get_light_param_value(id, &TOK_SPECULAR)
            .get::<f32>()
            .copied()
            .unwrap_or(1.0);

        l.set_has_intensity(intensity != 0.0);
        l.set_diffuse(scale_vec4(diffuse_mul, c));
        l.set_specular(scale_vec4(specular_mul, c));

        // Dome light texture file
        if let Some(path) = delegate
            .get_light_param_value(id, &TOK_TEXTURE_FILE)
            .get::<usd_sdf::asset_path::AssetPath>()
            .cloned()
        {
            l.set_dome_light_texture_file(path);
        }

        l
    }

    /// Pass-through for simple lights (from GlfSimpleLight params directly).
    ///
    /// Scales diffuse/specular by pi to match expected Lambertian output.
    /// Port of C++ `HdStLight::_PrepareSimpleLight`.
    fn prepare_simple_light(
        &self,
        id: &SdfPath,
        delegate: &dyn LightSceneDelegate,
    ) -> GlfSimpleLight {
        let v = delegate.get(id, &TOK_PARAMS);
        if let Some(l) = v.get::<GlfSimpleLight>() {
            let mut light = l.clone();
            light.set_diffuse(scale_vec4_pi(*light.get_diffuse()));
            light.set_specular(scale_vec4_pi(*light.get_specular()));
            light
        } else {
            GlfSimpleLight::default()
        }
    }

    // -----------------------------------------------------------------------
    // Sync (main update from scene delegate)
    // -----------------------------------------------------------------------

    /// Sync light state from the scene delegate.
    ///
    /// Port of C++ `HdStLight::Sync`. Updates internal params cache with
    /// - `HdTokens->transform`: Matrix4d
    /// - `HdLightTokens->params`: GlfSimpleLight
    /// - `HdLightTokens->shadowParams`: shadow params
    /// - `HdLightTokens->shadowCollection`: HdRprimCollection
    ///
    /// Also applies world transform (position/direction) into the light struct
    /// after params have been built.
    pub fn sync(&mut self, delegate: &dyn LightSceneDelegate, dirty_bits: &mut HdDirtyBits) {
        let id = &self.path.clone();

        // -- Transform --
        if *dirty_bits & DIRTY_TRANSFORM != 0 {
            let xform = delegate.get_transform(id);
            self.params
                .insert(TOK_TRANSFORM.clone(), Value::from(xform));
        }

        // -- Light params -> GlfSimpleLight --
        if *dirty_bits & DIRTY_PARAMS != 0 {
            let glf_light = if self.light_type == *LTYPE_SIMPLE {
                self.prepare_simple_light(id, delegate)
            } else if self.light_type == *LTYPE_DOME {
                self.prepare_dome_light(id, delegate)
            } else {
                // All area lights (sphere/rect/disk/cylinder/distant)
                self.approximate_area_light(id, delegate)
            };
            self.params
                .insert(TOK_PARAMS.clone(), Value::from_no_hash(glf_light));
        }

        // -- Apply transform to the cached GlfSimpleLight --
        if *dirty_bits & (DIRTY_TRANSFORM | DIRTY_PARAMS) != 0 {
            let transform = self
                .params
                .get(&TOK_TRANSFORM)
                .and_then(|v| v.get::<Matrix4d>().copied())
                .unwrap_or_else(Matrix4d::identity);

            if self.light_type == *LTYPE_DOME {
                // Apply optional dome offset rotation before world transform
                let final_transform = if let Some(dome_offset) = delegate
                    .get_light_param_value(id, &TOK_DOME_OFFSET)
                    .get::<Matrix4d>()
                    .copied()
                {
                    dome_offset * transform
                } else {
                    transform
                };

                if let Some(mut light) = self.get_simple_light() {
                    light.set_transform(final_transform);
                    self.params
                        .insert(TOK_PARAMS.clone(), Value::from_no_hash(light));
                }
            } else if self.light_type != *LTYPE_SIMPLE {
                // Area lights: extract position/direction from transform
                if let Some(mut light) = self.get_simple_light() {
                    // Translation = row 3, first 3 elements (row-vector convention)
                    let t = transform.extract_translation();
                    let mut pos = Vec4f::new(t.x as f32, t.y as f32, t.z as f32, 1.0);

                    // Z-axis direction = row 2 (convention: lights emit along -Z)
                    let row2 = transform.row(2);
                    let z_dir = Vec4f::new(row2.x as f32, row2.y as f32, row2.z as f32, 0.0);

                    if self.light_type == *LTYPE_RECT || self.light_type == *LTYPE_DISK {
                        // Spot direction = -Z axis of transform (emit direction)
                        light.set_spot_direction(Vec3f::new(-z_dir.x, -z_dir.y, -z_dir.z));
                    } else if self.light_type == *LTYPE_DISTANT {
                        // Distant light at infinity: use +Z as homogeneous direction (w=0)
                        pos = Vec4f::new(z_dir.x, z_dir.y, z_dir.z, 0.0);
                        // Emit direction = -Z axis, same convention as spot/area lights
                        light.set_spot_direction(Vec3f::new(-z_dir.x, -z_dir.y, -z_dir.z));
                    }
                    light.set_position(pos);
                    self.params
                        .insert(TOK_PARAMS.clone(), Value::from_no_hash(light));
                }
            }
        }

        // -- Shadow params --
        // C++ stores the raw VtValue(HdxShadowParams) in _params[shadowParams].
        // We don't have that C++ type; instead read individual shadow sub-params and
        // populate our crate::lighting::ShadowParams directly.
        if *dirty_bits & DIRTY_SHADOW_PARAMS != 0 {
            // Cache raw value in params map for downstream consumers that query it
            let raw = delegate.get_light_param_value(id, &TOK_SHADOW_PARAMS);
            self.params.insert(TOK_SHADOW_PARAMS.clone(), raw);

            // Extract individual shadow fields via standard HdLightTokens names.
            // Mirrors C++ HdxShadowParams members: enable, resolution, blur, bias.
            let enabled = delegate
                .get_light_param_value(id, &TOK_SHADOW_ENABLE)
                .get::<bool>()
                .copied()
                // Fall back to hasShadow if shadow:enable is absent (older schema)
                .unwrap_or_else(|| {
                    delegate
                        .get_light_param_value(id, &TOK_HAS_SHADOW)
                        .get::<bool>()
                        .copied()
                        .unwrap_or(false)
                });

            let resolution = delegate
                .get_light_param_value(id, &TOK_SHADOW_RESOLUTION)
                .get::<i32>()
                .copied()
                .map(|v| v as u32)
                .unwrap_or(1024);

            let blur = delegate
                .get_light_param_value(id, &TOK_SHADOW_BLUR)
                .get::<f32>()
                .copied()
                .unwrap_or(0.0);

            let bias = delegate
                .get_light_param_value(id, &TOK_SHADOW_BIAS)
                .get::<f32>()
                .copied()
                .unwrap_or(0.001);

            self.shadow_params = crate::lighting::ShadowParams {
                enabled,
                blur,
                bias,
                // shadow matrix is computed later by the shadow render pass;
                // leave as identity here (matches C++ deferred matrix computation)
                matrix: crate::shadow::MAT4_IDENTITY,
            };

            // Also sync the shadow map resolution if we have one allocated
            if let Some(ref mut sm) = self.shadow_map {
                sm.resolution = (resolution, resolution);
                sm.bias = bias;
            }

            log::debug!(
                "HdStLight::sync shadow_params {}: enabled={} res={} blur={:.4} bias={:.6}",
                self.path,
                enabled,
                resolution,
                blur,
                bias
            );
        }

        *dirty_bits = CLEAN;

        log::debug!("HdStLight::sync: {} (type: {})", self.path, self.light_type);
    }
}

// ---------------------------------------------------------------------------
// Helper: scalar * Vec4f
// ---------------------------------------------------------------------------

/// Scale a Vec4f by a scalar, preserving alpha = 1.0.
fn scale_vec4(s: f32, c: Vec4f) -> Vec4f {
    Vec4f::new(s * c.x, s * c.y, s * c.z, c.w)
}

/// Scale RGB channels of Vec4f by pi (for simple light Lambertian correction).
fn scale_vec4_pi(c: Vec4f) -> Vec4f {
    Vec4f::new(
        std::f32::consts::PI * c.x,
        std::f32::consts::PI * c.y,
        std::f32::consts::PI * c.z,
        1.0,
    )
}

// ---------------------------------------------------------------------------
// Shared pointer alias
// ---------------------------------------------------------------------------

/// Arc-wrapped HdStLight.
pub type HdStLightSharedPtr = Arc<HdStLight>;

// ---------------------------------------------------------------------------
// Re-exports for convenience
// ---------------------------------------------------------------------------

pub use usd_glf::GlfSimpleLight as SimpleLight;
pub use usd_glf::GlfSimpleLightVector;

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // Blackbody tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_blackbody_6500k_near_white() {
        // D65 is close to white but not exact on the Planckian locus
        let rgb = blackbody_temp_as_rgb(6500.0);
        // Should be roughly white (all channels close to 1)
        assert!(rgb.x > 0.8, "R should be > 0.8, got {}", rgb.x);
        assert!(rgb.y > 0.8, "G should be > 0.8, got {}", rgb.y);
        assert!(rgb.z > 0.8, "B should be > 0.8, got {}", rgb.z);
    }

    #[test]
    fn test_blackbody_1000k_is_red() {
        let rgb = blackbody_temp_as_rgb(1000.0);
        assert!(rgb.x > rgb.z, "At 1000K, R should exceed B");
        assert!(rgb.z < 0.1, "At 1000K, B should be near 0, got {}", rgb.z);
    }

    #[test]
    fn test_blackbody_10000k_is_blue() {
        let rgb = blackbody_temp_as_rgb(10000.0);
        assert!(rgb.z > rgb.x, "At 10000K, B should exceed R");
    }

    #[test]
    fn test_blackbody_luma_normalized() {
        // All samples should have luma ~= 1.0 after normalization
        for temp in [2000.0f32, 4000.0, 6500.0, 8000.0, 10000.0] {
            let rgb = blackbody_temp_as_rgb(temp);
            let luma = vec3f_dot(rgb, REC709_LUMA);
            assert!(
                (luma - 1.0).abs() < 0.01,
                "Luma at {}K = {}, expected ~1.0",
                temp,
                luma
            );
        }
    }

    // -----------------------------------------------------------------------
    // HdStLight creation tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_light_creation() {
        let path = SdfPath::from_string("/lights/key").unwrap();
        let light = HdStLight::new_distant(path.clone());

        assert_eq!(light.get_path(), &path);
        assert_eq!(light.get_light_type().as_str(), "distantLight");
        assert!(light.get_simple_light().is_none()); // not synced yet
    }

    #[test]
    fn test_light_types() {
        let path = SdfPath::from_string("/light").unwrap();
        assert_eq!(
            HdStLight::new_dome(path.clone()).get_light_type().as_str(),
            "domeLight"
        );
        assert_eq!(
            HdStLight::new_sphere(path.clone())
                .get_light_type()
                .as_str(),
            "sphereLight"
        );
        assert_eq!(
            HdStLight::new_rect(path).get_light_type().as_str(),
            "rectLight"
        );
    }

    #[test]
    fn test_initial_dirty_bits_distant() {
        let path = SdfPath::from_string("/light").unwrap();
        let light = HdStLight::new_distant(path);
        // Distant light should return ALL dirty bits initially
        assert_eq!(light.get_initial_dirty_bits_mask(), ALL_DIRTY);
    }

    #[test]
    fn test_initial_dirty_bits_sphere() {
        let path = SdfPath::from_string("/light").unwrap();
        let light = HdStLight::new_sphere(path);
        // Area light: only params + transform initially
        let bits = light.get_initial_dirty_bits_mask();
        assert_ne!(bits & DIRTY_PARAMS, 0);
        assert_ne!(bits & DIRTY_TRANSFORM, 0);
        assert_eq!(bits & DIRTY_SHADOW_PARAMS, 0);
    }

    // -----------------------------------------------------------------------
    // Mock delegate for unit tests
    // -----------------------------------------------------------------------

    struct MockDelegate {
        params: HashMap<String, Value>,
        visible: bool,
        transform: Matrix4d,
    }

    impl MockDelegate {
        fn new() -> Self {
            Self {
                params: HashMap::new(),
                visible: true,
                transform: Matrix4d::identity(),
            }
        }
        fn set(&mut self, key: &str, val: Value) {
            self.params.insert(key.to_string(), val);
        }
    }

    impl LightSceneDelegate for MockDelegate {
        fn get_light_param_value(&self, _id: &SdfPath, param: &Token) -> Value {
            self.params.get(param.as_str()).cloned().unwrap_or_default()
        }
        fn get_transform(&self, _id: &SdfPath) -> Matrix4d {
            self.transform
        }
        fn get_visible(&self, _id: &SdfPath) -> bool {
            self.visible
        }
        fn get(&self, _id: &SdfPath, key: &Token) -> Value {
            self.params.get(key.as_str()).cloned().unwrap_or_default()
        }
    }

    // -----------------------------------------------------------------------
    // Sync tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_sync_sphere_light_defaults() {
        let path = SdfPath::from_string("/light").unwrap();
        let mut light = HdStLight::new_sphere(path);
        let mut delegate = MockDelegate::new();
        delegate.set("color", Value::from(Vec3f::new(1.0, 1.0, 1.0)));
        delegate.set("intensity", Value::from(1.0f32));
        delegate.set("exposure", Value::from(0.0f32));
        delegate.set("normalize", Value::from(true)); // normalized so no area scaling

        let mut bits = DIRTY_PARAMS | DIRTY_TRANSFORM;
        light.sync(&delegate, &mut bits);

        assert_eq!(bits, CLEAN);
        let glf = light
            .get_simple_light()
            .expect("should have GlfSimpleLight after sync");
        assert!(glf.has_intensity());
        // diffuse should be white * 1.0 = (1,1,1,1) * diffuse_mul(1.0)
        let d = glf.get_diffuse();
        assert!((d.x - 1.0).abs() < 0.01);
        assert!((d.y - 1.0).abs() < 0.01);
        assert!((d.z - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_sync_invisible_light() {
        let path = SdfPath::from_string("/light").unwrap();
        let mut light = HdStLight::new_sphere(path);
        let mut delegate = MockDelegate::new();
        delegate.visible = false;

        let mut bits = DIRTY_PARAMS | DIRTY_TRANSFORM;
        light.sync(&delegate, &mut bits);

        let glf = light
            .get_simple_light()
            .expect("should have GlfSimpleLight");
        assert!(!glf.has_intensity());
        assert_eq!(*glf.get_diffuse(), Vec4f::new(0.0, 0.0, 0.0, 0.0));
    }

    #[test]
    fn test_sync_dome_light() {
        let path = SdfPath::from_string("/domeLight").unwrap();
        let mut light = HdStLight::new_dome(path);
        let mut delegate = MockDelegate::new();
        delegate.set("color", Value::from(Vec3f::new(1.0, 1.0, 1.0)));
        delegate.set("intensity", Value::from(1.0f32));
        delegate.set("exposure", Value::from(0.0f32));

        let mut bits = DIRTY_PARAMS | DIRTY_TRANSFORM;
        light.sync(&delegate, &mut bits);

        let glf = light.get_simple_light().expect("GlfSimpleLight");
        assert!(glf.is_dome_light());
        assert!(!glf.has_shadow());
    }

    #[test]
    fn test_sync_exposure_doubles_intensity() {
        let path = SdfPath::from_string("/light").unwrap();
        let mut light = HdStLight::new_sphere(path);
        let mut delegate = MockDelegate::new();
        delegate.set("color", Value::from(Vec3f::new(1.0, 1.0, 1.0)));
        delegate.set("intensity", Value::from(1.0f32));
        delegate.set("exposure", Value::from(1.0f32)); // 2^1 = 2x multiplier
        delegate.set("normalize", Value::from(true));

        let mut bits = DIRTY_PARAMS | DIRTY_TRANSFORM;
        light.sync(&delegate, &mut bits);

        let glf = light.get_simple_light().unwrap();
        let d = glf.get_diffuse();
        // intensity = 1.0 * 2^1 = 2.0; diffuse_mul=1; ambient_mul=0
        assert!((d.x - 2.0).abs() < 0.01, "Expected ~2.0, got {}", d.x);
    }

    #[test]
    fn test_sync_area_scaling_rect_light() {
        let path = SdfPath::from_string("/rect").unwrap();
        let mut light = HdStLight::new_rect(path);
        let mut delegate = MockDelegate::new();
        delegate.set("color", Value::from(Vec3f::new(1.0, 1.0, 1.0)));
        delegate.set("intensity", Value::from(1.0f32));
        delegate.set("exposure", Value::from(0.0f32));
        delegate.set("normalize", Value::from(false)); // area scaling enabled
        delegate.set("width", Value::from(2.0f32));
        delegate.set("height", Value::from(3.0f32)); // area = 6

        let mut bits = DIRTY_PARAMS | DIRTY_TRANSFORM;
        light.sync(&delegate, &mut bits);

        let glf = light.get_simple_light().unwrap();
        let d = glf.get_diffuse();
        // intensity = 1.0 * area(6) = 6.0
        assert!((d.x - 6.0).abs() < 0.01, "Expected ~6.0, got {}", d.x);
    }

    #[test]
    fn test_sync_distant_light_attenuation() {
        let path = SdfPath::from_string("/distant").unwrap();
        let mut light = HdStLight::new_distant(path);
        let mut delegate = MockDelegate::new();
        delegate.set("color", Value::from(Vec3f::new(1.0, 1.0, 1.0)));
        delegate.set("intensity", Value::from(1.0f32));
        delegate.set("exposure", Value::from(0.0f32));
        delegate.set("normalize", Value::from(true));

        let mut bits = ALL_DIRTY;
        light.sync(&delegate, &mut bits);

        let glf = light.get_simple_light().unwrap();
        // Distant light: zero attenuation
        assert_eq!(*glf.get_attenuation(), Vec3f::new(0.0, 0.0, 0.0));
    }

    #[test]
    fn test_compute_area_sphere() {
        let path = SdfPath::from_string("/sphere").unwrap();
        let light = HdStLight::new_sphere(path);
        let mut delegate = MockDelegate::new();
        delegate.set("radius", Value::from(2.0f32));

        let area = light.compute_area(&SdfPath::from_string("/sphere").unwrap(), &delegate);
        // Sphere surface area = 4 * pi * r^2 = 4 * pi * 4 ≈ 50.27 for r=2. (P1-7 fix)
        let expected = 4.0 * std::f32::consts::PI * 4.0;
        assert!(
            (area - expected).abs() < 0.01,
            "Expected {}, got {}",
            expected,
            area
        );
    }

    #[test]
    fn test_compute_area_cylinder() {
        let path = SdfPath::from_string("/cyl").unwrap();
        let light = HdStLight::new(path, Token::new("cylinderLight"));
        let mut delegate = MockDelegate::new();
        delegate.set("radius", Value::from(1.0f32));
        delegate.set("length", Value::from(2.0f32)); // 2*pi*1*2 = ~12.57

        let area = light.compute_area(&SdfPath::from_string("/cyl").unwrap(), &delegate);
        let expected = 2.0 * std::f32::consts::PI * 1.0 * 2.0;
        assert!((area - expected).abs() < 0.01);
    }

    #[test]
    fn test_color_temperature_tints_warm() {
        let path = SdfPath::from_string("/light").unwrap();
        let light = HdStLight::new_sphere(path.clone());
        let mut delegate = MockDelegate::new();
        delegate.set("color", Value::from(Vec3f::new(1.0, 1.0, 1.0)));
        delegate.set("enableColorTemperature", Value::from(true));
        delegate.set("colorTemperature", Value::from(2000.0f32));

        let hdc = light.read_color(&path, &delegate);
        // At 2000K, green and blue are much lower than red
        assert!(
            hdc.x > hdc.z,
            "R ({}) should exceed B ({}) at 2000K",
            hdc.x,
            hdc.z
        );
    }
}
