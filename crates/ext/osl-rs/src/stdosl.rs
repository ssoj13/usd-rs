//! stdosl.h — the standard OSL library header content.
//!
//! This module contains the text of `stdosl.h` and associated type
//! definitions that are automatically included in every OSL shader.
//! It defines macros, built-in types, and function declarations.

/// The full stdosl.h text, ready to be prepended to any OSL source.
pub const STDOSL_H: &str = r#"
// stdosl.h -- Standard OSL header file
// Copyright Contributors to the Open Shading Language project.
// SPDX-License-Identifier: BSD-3-Clause

#ifndef STDOSL_H
#define STDOSL_H

// Define OSL version
#define OSL_VERSION_MAJOR 1
#define OSL_VERSION_MINOR 14
#define OSL_VERSION (OSL_VERSION_MAJOR * 10000 + OSL_VERSION_MINOR * 100)

// Type aliases
#define color  color
#define point  point
#define vector vector
#define normal normal
#define matrix matrix

// Math constants
#define M_PI       3.14159265358979323846
#define M_PI_2     1.57079632679489661923
#define M_PI_4     0.78539816339744830961
#define M_2_PI     0.63661977236758134308
#define M_2PI      6.28318530717958647693
#define M_4PI     12.56637061435917295385
#define M_1_PI     0.31830988618379067154
#define M_2_SQRTPI 1.12837916709551257390
#define M_E        2.71828182845904523536
#define M_LN2      0.69314718055994530942
#define M_LN10     2.30258509299404568402
#define M_LOG2E    1.44269504088896340736
#define M_LOG10E   0.43429448190325182765
#define M_SQRT2    1.41421356237309504880
#define M_SQRT1_2  0.70710678118654752440

// Math functions (declarations)
float abs(float x);
int   abs(int x);
float ceil(float x);
float floor(float x);
float round(float x);
float trunc(float x);
float sign(float x);
float min(float a, float b);
float max(float a, float b);
int   min(int a, int b);
int   max(int a, int b);
float clamp(float x, float lo, float hi);
int   clamp(int x, int lo, int hi);
float mix(float a, float b, float t);
color mix(color a, color b, float t);
float step(float edge, float x);
float smoothstep(float edge0, float edge1, float x);
float linearstep(float edge0, float edge1, float x);
float mod(float a, float b);
int   mod(int a, int b);
float fmod(float a, float b);

float sqrt(float x);
float inversesqrt(float x);
float cbrt(float x);
float pow(float x, float y);
float exp(float x);
float exp2(float x);
float expm1(float x);
float log(float x);
float log2(float x);
float log10(float x);
float log(float x, float base);
float erf(float x);
float erfc(float x);

float sin(float x);
float cos(float x);
float tan(float x);
float asin(float x);
float acos(float x);
float atan(float y);
float atan2(float y, float x);
float sinh(float x);
float cosh(float x);
float tanh(float x);
void  sincos(float x, output float sinval, output float cosval);
float radians(float deg);
float degrees(float rad);

// Geometry
float dot(vector a, vector b);
vector cross(vector a, vector b);
float length(vector v);
float distance(point a, point b);
vector normalize(vector v);
vector faceforward(vector N, vector I, vector Nref);
vector faceforward(vector N, vector I);
vector reflect(vector I, vector N);
vector refract(vector I, vector N, float eta);
void fresnel(vector I, normal N, float eta, output float Kr, output float Kt, output vector R, output vector T);
void fresnel(vector I, normal N, float eta, output float Kr, output float Kt);
float hypot(float a, float b);
float hypot(float a, float b, float c);
point  rotate(point p, float angle, point a, point b);
float  area(point p);
vector calculatenormal(point p);

// Color
float luminance(color c);
color blackbody(float temperature);
color wavelength_color(float wavelength);
color transformc(string from, string to, color c);
color transformc(string to, color c);

// Matrix
float determinant(matrix m);
matrix transpose(matrix m);
point  transform(string from, string to, point p);
point  transform(string to, point p);
point  transform(matrix M, point p);
vector transform(string from, string to, vector v);
vector transform(string to, vector v);
vector transform(matrix M, vector v);
normal transform(string from, string to, normal n);
normal transform(string to, normal n);
normal transform(matrix M, normal n);
float  transformu(string from, string to, float x);

// Strings
string concat(string a, string b);
int    strlen(string s);
int    startswith(string s, string prefix);
int    endswith(string s, string suffix);
int    stoi(string s);
float  stof(string s);
string substr(string s, int start, int len);
int    getchar(string s, int index);
int    regex_search(string subject, string pattern);
int    regex_match(string subject, string pattern);
int    hash(string s);
string format(string fmt, ...);
void   printf(string fmt, ...);
void   fprintf(string filename, string fmt, ...);
void   error(string fmt, ...);
void   warning(string fmt, ...);

// Noise
float  noise(string noisetype, float x);
float  noise(string noisetype, float x, float y);
float  noise(string noisetype, point p);
float  noise(string noisetype, point p, float t);
float  noise(float x);
float  noise(float x, float y);
float  noise(point p);
float  noise(point p, float t);
color  noise(string noisetype, float x);
color  noise(string noisetype, point p);
float  snoise(float x);
float  snoise(point p);
float  pnoise(string noisetype, point p, point period);
float  cellnoise(float x);
float  cellnoise(point p);
float  hashnoise(float x);
float  hashnoise(point p);

// Texture
color  texture(string filename, float s, float t, ...);
color  texture3d(string filename, point p, ...);
color  environment(string filename, vector R, ...);
int    gettextureinfo(string filename, string dataname, output int val);
int    gettextureinfo(string filename, string dataname, output float val);
int    gettextureinfo(string filename, string dataname, output string val);

// Closures
closure color diffuse(normal N);
closure color oren_nayar(normal N, float sigma);
closure color phong(normal N, float exponent);
closure color ward(normal N, vector T, float ax, float ay);
closure color microfacet(string dist, normal N, float alpha, float eta, int refract);
closure color reflection(normal N);
closure color refraction(normal N, float eta);
closure color transparent();
closure color translucent(normal N);
closure color emission();
closure color background();
closure color holdout();
closure color debug(string tag);
closure color subsurface(float eta, float g, color mfp, color albedo);

// MaterialX closures
closure color oren_nayar_diffuse_bsdf(normal N, color albedo, float roughness);
closure color burley_diffuse_bsdf(normal N, color albedo, float roughness);
closure color dielectric_bsdf(normal N, vector U, color reflection_tint, color transmission_tint, float roughness_x, float roughness_y, float ior, string distribution);
closure color conductor_bsdf(normal N, vector U, float roughness_x, float roughness_y, color ior, color extinction, string distribution);
closure color generalized_schlick_bsdf(normal N, vector U, color reflection_tint, color transmission_tint, float roughness_x, float roughness_y, color f0, color f90, float exponent, string distribution);
closure color translucent_bsdf(normal N, color albedo);
closure color transparent_bsdf();
closure color subsurface_bssrdf(normal N, color albedo, color radius, float anisotropy);
closure color sheen_bsdf(normal N, color albedo, float roughness);
closure color uniform_edf(color emittance);
closure color anisotropic_vdf(color albedo, color extinction, float anisotropy);
closure color medium_vdf(color albedo, float transmission_depth, color transmission_color, float anisotropy, float ior, int priority);
closure color layer(closure color top, closure color base);
closure color chiang_hair_bsdf(normal N, vector curve_direction, color tint_R, color tint_TT, color tint_TRT, float ior, float longitudual_roughness_R, float longitudual_roughness_TT, float longitudual_roughness_TRT, float azimuthal_roughness_R, float azimuthal_roughness_TT, float azimuthal_roughness_TRT, float cuticle_angle, color absorption_coefficient);

// Derivatives
float  Dx(float x);
float  Dy(float x);
float  Dz(float x);
vector Dx(vector x);
vector Dy(vector x);
float  filterwidth(float x);
vector filterwidth(vector x);

// Attributes
int    getattribute(string name, output int val);
int    getattribute(string name, output float val);
int    getattribute(string name, output string val);
int    getattribute(string name, output color val);
int    getattribute(string name, output point val);
int    getattribute(string name, output vector val);
int    getattribute(string name, output normal val);
int    getattribute(string name, output matrix val);
int    getattribute(string obj, string name, output int val);
int    getattribute(string obj, string name, output float val);

// Messages
void   setmessage(string name, int value);
void   setmessage(string name, float value);
void   setmessage(string name, string value);
void   setmessage(string name, color value);
int    getmessage(string source, string name, output int val);
int    getmessage(string source, string name, output float val);
int    getmessage(string source, string name, output string val);
int    getmessage(string source, string name, output color val);

// Misc
int    isconnected(float param);
int    isconstant(float expr);
int    arraylength(float arr[]);
int    raytype(string name);
int    backfacing();
float  surfacearea();
int    trace(point P, vector R, ...);
void   exit();

// Spline
float  spline(string basis, float x, float knots[]);
color  spline(string basis, float x, color knots[]);
float  splineinverse(string basis, float y, float knots[]);

// Dict
int    dict_find(string dictionary, string query);
int    dict_find(int nodeID, string query);
int    dict_next(int nodeID);
int    dict_value(int nodeID, string attribname, output float value);
int    dict_value(int nodeID, string attribname, output int value);
int    dict_value(int nodeID, string attribname, output string value);

// Pointcloud
int    pointcloud_search(string filename, point center, float radius, int maxpoints, ...);
int    pointcloud_get(string filename, int indices[], int count, string attr, output float data[]);
int    pointcloud_get(string filename, int indices[], int count, string attr, output color data[]);
int    pointcloud_get(string filename, int indices[], int count, string attr, output point data[]);
int    pointcloud_get(string filename, int indices[], int count, string attr, output vector data[]);
int    pointcloud_write(string filename, point P, ...);

#endif // STDOSL_H
"#;

/// Color space names.
pub const COLOR_SPACES: &[&str] = &[
    "rgb",
    "RGB",
    "hsv",
    "hsl",
    "YIQ",
    "xyz",
    "XYZ",
    "xyY",
    "linear",
    "Rec709",
    "sRGB",
    "NTSC",
    "EBU",
    "PAL",
    "SECAM",
    "SMPTE",
    "HDTV",
    "CIE",
    "AdobeRGB",
    "ACES2065-1",
    "ACEScg",
];

/// Standard shader variable names.
pub const SHADER_GLOBALS: &[(&str, &str)] = &[
    ("P", "point"),
    ("I", "vector"),
    ("N", "normal"),
    ("Ng", "normal"),
    ("u", "float"),
    ("v", "float"),
    ("dPdu", "vector"),
    ("dPdv", "vector"),
    ("time", "float"),
    ("dtime", "float"),
    ("dPdtime", "vector"),
    ("Ps", "point"),
    ("Ci", "color"),
];

/// Standard closure parameter types.
pub const CLOSURE_PARAMS: &[(&str, &[&str])] = &[
    ("diffuse", &["normal"]),
    ("oren_nayar", &["normal", "float"]),
    ("phong", &["normal", "float"]),
    ("ward", &["normal", "vector", "float", "float"]),
    ("microfacet", &["string", "normal", "float", "float", "int"]),
    ("reflection", &["normal"]),
    ("refraction", &["normal", "float"]),
    ("transparent", &[]),
    ("translucent", &["normal"]),
    ("emission", &[]),
    ("background", &[]),
    ("holdout", &[]),
    ("debug", &["string"]),
    ("subsurface", &["float", "float", "color", "color"]),
    // MaterialX closures
    ("oren_nayar_diffuse_bsdf", &["normal", "color", "float"]),
    ("burley_diffuse_bsdf", &["normal", "color", "float"]),
    (
        "dielectric_bsdf",
        &[
            "normal", "vector", "color", "color", "float", "float", "float", "string",
        ],
    ),
    (
        "conductor_bsdf",
        &[
            "normal", "vector", "float", "float", "color", "color", "string",
        ],
    ),
    (
        "generalized_schlick_bsdf",
        &[
            "normal", "vector", "color", "color", "float", "float", "color", "color", "float",
            "string",
        ],
    ),
    ("translucent_bsdf", &["normal", "color"]),
    ("transparent_bsdf", &[]),
    ("subsurface_bssrdf", &["normal", "color", "color", "float"]),
    ("sheen_bsdf", &["normal", "color", "float"]),
    ("uniform_edf", &["color"]),
    ("anisotropic_vdf", &["color", "color", "float"]),
    (
        "medium_vdf",
        &["color", "float", "color", "float", "float", "int"],
    ),
    ("layer", &["closure", "closure"]),
    (
        "chiang_hair_bsdf",
        &[
            "normal", "vector", "color", "color", "color", "float", "float", "float", "float",
            "float", "float", "float", "float", "color",
        ],
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stdosl_h_content() {
        assert!(STDOSL_H.contains("#define M_PI"));
        assert!(STDOSL_H.contains("#define M_2PI"));
        assert!(STDOSL_H.contains("#define M_4PI"));
        assert!(STDOSL_H.contains("float sin(float x)"));
        assert!(STDOSL_H.contains("closure color diffuse(normal N)"));
        assert!(STDOSL_H.contains("closure color subsurface("));
        assert!(STDOSL_H.contains("closure color chiang_hair_bsdf("));
        assert!(STDOSL_H.contains("#endif"));
    }

    #[test]
    fn test_stdosl_h_materialx_closures() {
        assert!(STDOSL_H.contains("oren_nayar_diffuse_bsdf"));
        assert!(STDOSL_H.contains("burley_diffuse_bsdf"));
        assert!(STDOSL_H.contains("dielectric_bsdf"));
        assert!(STDOSL_H.contains("conductor_bsdf"));
        assert!(STDOSL_H.contains("generalized_schlick_bsdf"));
        assert!(STDOSL_H.contains("translucent_bsdf"));
        assert!(STDOSL_H.contains("transparent_bsdf"));
        assert!(STDOSL_H.contains("subsurface_bssrdf"));
        assert!(STDOSL_H.contains("sheen_bsdf"));
        assert!(STDOSL_H.contains("uniform_edf"));
        assert!(STDOSL_H.contains("anisotropic_vdf"));
        assert!(STDOSL_H.contains("medium_vdf"));
        assert!(STDOSL_H.contains("closure color layer("));
    }

    #[test]
    fn test_color_spaces() {
        // All C++ opcolor.cpp color spaces must be present
        assert!(COLOR_SPACES.contains(&"rgb"));
        assert!(COLOR_SPACES.contains(&"RGB"));
        assert!(COLOR_SPACES.contains(&"hsv"));
        assert!(COLOR_SPACES.contains(&"hsl"));
        assert!(COLOR_SPACES.contains(&"YIQ"));
        assert!(COLOR_SPACES.contains(&"XYZ"));
        assert!(COLOR_SPACES.contains(&"xyY"));
        assert!(COLOR_SPACES.contains(&"linear"));
        assert!(COLOR_SPACES.contains(&"Rec709"));
        assert!(COLOR_SPACES.contains(&"sRGB"));
        assert!(COLOR_SPACES.contains(&"NTSC"));
        assert!(COLOR_SPACES.contains(&"EBU"));
        assert!(COLOR_SPACES.contains(&"PAL"));
        assert!(COLOR_SPACES.contains(&"SECAM"));
        assert!(COLOR_SPACES.contains(&"SMPTE"));
        assert!(COLOR_SPACES.contains(&"HDTV"));
        assert!(COLOR_SPACES.contains(&"CIE"));
        assert!(COLOR_SPACES.contains(&"AdobeRGB"));
        assert!(COLOR_SPACES.contains(&"ACES2065-1"));
        assert!(COLOR_SPACES.contains(&"ACEScg"));
        assert_eq!(COLOR_SPACES.len(), 21);
    }

    #[test]
    fn test_shader_globals() {
        assert!(SHADER_GLOBALS.iter().any(|(n, _)| *n == "P"));
        assert!(SHADER_GLOBALS.iter().any(|(n, _)| *n == "N"));
        assert!(SHADER_GLOBALS.iter().any(|(n, _)| *n == "Ci"));
    }

    #[test]
    fn test_closure_params() {
        let diffuse = CLOSURE_PARAMS.iter().find(|(n, _)| *n == "diffuse");
        assert!(diffuse.is_some());
        assert_eq!(diffuse.unwrap().1, &["normal"]);

        // subsurface
        let sub = CLOSURE_PARAMS.iter().find(|(n, _)| *n == "subsurface");
        assert!(sub.is_some());
        assert_eq!(sub.unwrap().1, &["float", "float", "color", "color"]);
    }

    #[test]
    fn test_closure_params_materialx() {
        // All MaterialX closures must be in CLOSURE_PARAMS
        let mtlx_names = [
            "oren_nayar_diffuse_bsdf",
            "burley_diffuse_bsdf",
            "dielectric_bsdf",
            "conductor_bsdf",
            "generalized_schlick_bsdf",
            "translucent_bsdf",
            "transparent_bsdf",
            "subsurface_bssrdf",
            "sheen_bsdf",
            "uniform_edf",
            "anisotropic_vdf",
            "medium_vdf",
            "layer",
            "chiang_hair_bsdf",
        ];
        for name in &mtlx_names {
            assert!(
                CLOSURE_PARAMS.iter().any(|(n, _)| n == name),
                "Missing closure: {name}"
            );
        }
        // Total: 13 std + 1 subsurface + 14 MaterialX = 28
        assert_eq!(CLOSURE_PARAMS.len(), 28);
    }
}
