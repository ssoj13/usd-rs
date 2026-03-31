//! Predefined string constants and their compile-time hashes.
//!
//! Mirrors OSL's `strdecls.h` — every constant string used in JITed shader
//! code or renderer code is declared here with both a lazy `UString`
//! accessor and a compile-time `UStringHash`.

/// Compile-time hash of a string literal.
macro_rules! h {
    ($s:expr) => {
        UStringHash(crate::hashes::fingerprint64($s.as_bytes()))
    };
}

/// Predefined string hashes (compile-time, no allocation).
pub mod hash_constants {
    use crate::ustring::UStringHash;

    pub const EMPTY: UStringHash = h!("");
    pub const CAMERA: UStringHash = h!("camera");
    pub const COMMON: UStringHash = h!("common");
    pub const OBJECT: UStringHash = h!("object");
    pub const SHADER: UStringHash = h!("shader");
    pub const SCREEN: UStringHash = h!("screen");
    pub const NDC: UStringHash = h!("NDC");
    pub const RGB: UStringHash = h!("rgb");
    pub const RGB_UPPER: UStringHash = h!("RGB");
    pub const HSV: UStringHash = h!("hsv");
    pub const HSL: UStringHash = h!("hsl");
    pub const YIQ: UStringHash = h!("YIQ");
    pub const XYZ_UPPER: UStringHash = h!("XYZ");
    pub const XYZ: UStringHash = h!("xyz");
    pub const XYY: UStringHash = h!("xyY");
    pub const NULL: UStringHash = h!("null");
    pub const DEFAULT: UStringHash = h!("default");
    pub const LABEL: UStringHash = h!("label");
    pub const SIDEDNESS: UStringHash = h!("sidedness");
    pub const FRONT: UStringHash = h!("front");
    pub const BACK: UStringHash = h!("back");
    pub const BOTH: UStringHash = h!("both");

    // Shader globals
    pub const P: UStringHash = h!("P");
    pub const I: UStringHash = h!("I");
    pub const N: UStringHash = h!("N");
    pub const NG: UStringHash = h!("Ng");
    pub const DPDU: UStringHash = h!("dPdu");
    pub const DPDV: UStringHash = h!("dPdv");
    pub const U: UStringHash = h!("u");
    pub const V: UStringHash = h!("v");
    pub const PS: UStringHash = h!("Ps");
    pub const TIME: UStringHash = h!("time");
    pub const DTIME: UStringHash = h!("dtime");
    pub const DPDTIME: UStringHash = h!("dPdtime");
    pub const CI: UStringHash = h!("Ci");

    // Texture options
    pub const WIDTH: UStringHash = h!("width");
    pub const SWIDTH: UStringHash = h!("swidth");
    pub const TWIDTH: UStringHash = h!("twidth");
    pub const RWIDTH: UStringHash = h!("rwidth");
    pub const BLUR: UStringHash = h!("blur");
    pub const SBLUR: UStringHash = h!("sblur");
    pub const TBLUR: UStringHash = h!("tblur");
    pub const RBLUR: UStringHash = h!("rblur");
    pub const WRAP: UStringHash = h!("wrap");
    pub const SWRAP: UStringHash = h!("swrap");
    pub const TWRAP: UStringHash = h!("twrap");
    pub const RWRAP: UStringHash = h!("rwrap");
    pub const BLACK: UStringHash = h!("black");
    pub const CLAMP: UStringHash = h!("clamp");
    pub const PERIODIC: UStringHash = h!("periodic");
    pub const MIRROR: UStringHash = h!("mirror");
    pub const FIRSTCHANNEL: UStringHash = h!("firstchannel");
    pub const FILL: UStringHash = h!("fill");
    pub const ALPHA: UStringHash = h!("alpha");
    pub const ERRORMESSAGE: UStringHash = h!("errormessage");

    // Trace
    pub const TRACE: UStringHash = h!("trace");
    pub const MINDIST: UStringHash = h!("mindist");
    pub const MAXDIST: UStringHash = h!("maxdist");
    pub const SHADE: UStringHash = h!("shade");
    pub const TRACESET: UStringHash = h!("traceset");

    // Interpolation
    pub const INTERP: UStringHash = h!("interp");
    pub const CLOSEST: UStringHash = h!("closest");
    pub const LINEAR: UStringHash = h!("linear");
    pub const CUBIC: UStringHash = h!("cubic");
    pub const CATMULLROM: UStringHash = h!("catmull-rom");
    pub const BEZIER: UStringHash = h!("bezier");
    pub const BSPLINE: UStringHash = h!("bspline");
    pub const HERMITE: UStringHash = h!("hermite");
    pub const CONSTANT: UStringHash = h!("constant");
    pub const SMARTCUBIC: UStringHash = h!("smartcubic");

    // Noise types
    pub const PERLIN: UStringHash = h!("perlin");
    pub const UPERLIN: UStringHash = h!("uperlin");
    pub const NOISE: UStringHash = h!("noise");
    pub const SNOISE: UStringHash = h!("snoise");
    pub const CELL: UStringHash = h!("cell");
    pub const CELLNOISE: UStringHash = h!("cellnoise");
    pub const PCELLNOISE: UStringHash = h!("pcellnoise");
    pub const HASH: UStringHash = h!("hash");
    pub const HASHNOISE: UStringHash = h!("hashnoise");
    pub const PHASHNOISE: UStringHash = h!("phashnoise");
    pub const PNOISE: UStringHash = h!("pnoise");
    pub const PSNOISE: UStringHash = h!("psnoise");
    pub const GENERICNOISE: UStringHash = h!("genericnoise");
    pub const GENERICPNOISE: UStringHash = h!("genericpnoise");
    pub const GABOR: UStringHash = h!("gabor");
    pub const GABORNOISE: UStringHash = h!("gabornoise");
    pub const GABORPNOISE: UStringHash = h!("gaborpnoise");
    pub const SIMPLEX: UStringHash = h!("simplex");
    pub const USIMPLEX: UStringHash = h!("usimplex");
    pub const SIMPLEXNOISE: UStringHash = h!("simplexnoise");
    pub const USIMPLEXNOISE: UStringHash = h!("usimplexnoise");

    // Gabor noise params
    pub const ANISOTROPIC: UStringHash = h!("anisotropic");
    pub const DIRECTION: UStringHash = h!("direction");
    pub const DO_FILTER: UStringHash = h!("do_filter");
    pub const BANDWIDTH: UStringHash = h!("bandwidth");
    pub const IMPULSES: UStringHash = h!("impulses");

    // Control flow
    pub const OP_DOWHILE: UStringHash = h!("dowhile");
    pub const OP_FOR: UStringHash = h!("for");
    pub const OP_WHILE: UStringHash = h!("while");
    pub const OP_EXIT: UStringHash = h!("exit");

    // Color spaces
    pub const COLOR: UStringHash = h!("color");
    pub const POINT: UStringHash = h!("point");
    pub const VECTOR: UStringHash = h!("vector");
    pub const NORMAL: UStringHash = h!("normal");
    pub const MATRIX: UStringHash = h!("matrix");
    pub const REC709: UStringHash = h!("Rec709");
    pub const SRGB: UStringHash = h!("sRGB");
    pub const WORLD: UStringHash = h!("world");
    pub const COLORSPACE: UStringHash = h!("colorspace");

    // Additional color spaces (from strdecls.h)
    pub const NTSC: UStringHash = h!("NTSC");
    pub const EBU: UStringHash = h!("EBU");
    pub const PAL: UStringHash = h!("PAL");
    pub const SECAM: UStringHash = h!("SECAM");
    pub const SMPTE: UStringHash = h!("SMPTE");
    pub const HDTV: UStringHash = h!("HDTV");
    pub const CIE: UStringHash = h!("CIE");
    pub const ADOBERGB: UStringHash = h!("AdobeRGB");
    pub const ACES2065_1: UStringHash = h!("ACES2065-1");
    pub const ACESCG: UStringHash = h!("ACEScg");
    pub const COLORSYSTEM: UStringHash = h!("colorsystem");

    // Misc
    pub const ERROR: UStringHash = h!("error");
    pub const USEPARAM: UStringHash = h!("useparam");
    pub const RAYTYPE: UStringHash = h!("raytype");
    pub const ARRAYLENGTH: UStringHash = h!("arraylength");
    pub const UNKNOWN: UStringHash = h!("unknown");
    pub const SUBIMAGE: UStringHash = h!("subimage");
    pub const SUBIMAGENAME: UStringHash = h!("subimagename");
    pub const MISSINGCOLOR: UStringHash = h!("missingcolor");
    pub const MISSINGALPHA: UStringHash = h!("missingalpha");
    pub const END: UStringHash = h!("end");
    pub const UNINITIALIZED_STRING: UStringHash = h!("!!!uninitialized!!!");
    pub const UNULL: UStringHash = h!("unull");
    pub const ERROR_COLOR_TRANSFORM: UStringHash =
        h!("ERROR: Unknown color space transformation \"%s\" -> \"%s\"\n");
}

/// Predefined `UString` constants (lazily interned on first access).
pub mod strings {
    use crate::ustring::UString;
    use std::sync::OnceLock;

    macro_rules! def_ustring {
        ($name:ident, $lit:expr) => {
            pub fn $name() -> UString {
                static CACHED: OnceLock<UString> = OnceLock::new();
                *CACHED.get_or_init(|| UString::new($lit))
            }
        };
    }

    def_ustring!(empty, "");
    def_ustring!(camera, "camera");
    def_ustring!(common, "common");
    def_ustring!(object, "object");
    def_ustring!(shader, "shader");
    def_ustring!(screen, "screen");
    def_ustring!(ndc, "NDC");
    def_ustring!(rgb, "rgb");
    def_ustring!(hsv, "hsv");
    def_ustring!(hsl, "hsl");
    def_ustring!(yiq, "YIQ");
    def_ustring!(xyz_upper, "XYZ");
    def_ustring!(xyz, "xyz");

    // Shader globals
    def_ustring!(p, "P");
    def_ustring!(i, "I");
    def_ustring!(n, "N");
    def_ustring!(ng, "Ng");
    def_ustring!(dpdu, "dPdu");
    def_ustring!(dpdv, "dPdv");
    def_ustring!(u, "u");
    def_ustring!(v, "v");
    def_ustring!(ps, "Ps");
    def_ustring!(time, "time");
    def_ustring!(dtime, "dtime");
    def_ustring!(dpdtime, "dPdtime");
    def_ustring!(ci, "Ci");

    // Noise types
    def_ustring!(perlin, "perlin");
    def_ustring!(uperlin, "uperlin");
    def_ustring!(noise, "noise");
    def_ustring!(snoise, "snoise");
    def_ustring!(cell, "cell");
    def_ustring!(cellnoise, "cellnoise");
    def_ustring!(gabor, "gabor");
    def_ustring!(simplex, "simplex");

    // Interpolation
    def_ustring!(linear, "linear");
    def_ustring!(cubic, "cubic");
    def_ustring!(catmullrom, "catmull-rom");
    def_ustring!(bspline, "bspline");
    def_ustring!(bezier, "bezier");
    def_ustring!(hermite, "hermite");
    def_ustring!(constant, "constant");

    // Color spaces
    def_ustring!(color, "color");
    def_ustring!(point, "point");
    def_ustring!(vector, "vector");
    def_ustring!(normal, "normal");
    def_ustring!(matrix, "matrix");
    def_ustring!(rec709, "Rec709");
    def_ustring!(srgb, "sRGB");
    def_ustring!(world, "world");

    // Texture options
    def_ustring!(width, "width");
    def_ustring!(swidth, "swidth");
    def_ustring!(twidth, "twidth");
    def_ustring!(rwidth, "rwidth");
    def_ustring!(blur, "blur");
    def_ustring!(sblur, "sblur");
    def_ustring!(tblur, "tblur");
    def_ustring!(rblur, "rblur");
    def_ustring!(wrap, "wrap");
    def_ustring!(swrap, "swrap");
    def_ustring!(twrap, "twrap");
    def_ustring!(rwrap, "rwrap");
    def_ustring!(black, "black");
    def_ustring!(clamp, "clamp");
    def_ustring!(periodic, "periodic");
    def_ustring!(mirror, "mirror");
    def_ustring!(firstchannel, "firstchannel");
    def_ustring!(fill, "fill");
    def_ustring!(alpha, "alpha");
    def_ustring!(errormessage, "errormessage");

    // Trace
    def_ustring!(trace, "trace");
    def_ustring!(mindist, "mindist");
    def_ustring!(maxdist, "maxdist");
    def_ustring!(shade, "shade");
    def_ustring!(traceset, "traceset");

    // Interpolation (extras)
    def_ustring!(interp, "interp");
    def_ustring!(closest, "closest");
    def_ustring!(smartcubic, "smartcubic");

    // Noise types (extras)
    def_ustring!(pcellnoise, "pcellnoise");
    def_ustring!(hash_str, "hash");
    def_ustring!(hashnoise, "hashnoise");
    def_ustring!(phashnoise, "phashnoise");
    def_ustring!(pnoise, "pnoise");
    def_ustring!(psnoise, "psnoise");
    def_ustring!(genericnoise, "genericnoise");
    def_ustring!(genericpnoise, "genericpnoise");
    def_ustring!(gabornoise, "gabornoise");
    def_ustring!(gaborpnoise, "gaborpnoise");
    def_ustring!(usimplex, "usimplex");
    def_ustring!(simplexnoise, "simplexnoise");
    def_ustring!(usimplexnoise, "usimplexnoise");

    // Gabor noise params
    def_ustring!(anisotropic, "anisotropic");
    def_ustring!(direction, "direction");
    def_ustring!(do_filter, "do_filter");
    def_ustring!(bandwidth, "bandwidth");
    def_ustring!(impulses, "impulses");

    // Control flow
    def_ustring!(op_dowhile, "dowhile");
    def_ustring!(op_for, "for");
    def_ustring!(op_while, "while");
    def_ustring!(op_exit, "exit");

    // Additional names from strdecls.h
    def_ustring!(null, "null");
    def_ustring!(default_, "default");
    def_ustring!(label, "label");
    def_ustring!(sidedness, "sidedness");
    def_ustring!(front, "front");
    def_ustring!(back, "back");
    def_ustring!(both, "both");
    def_ustring!(xyy, "xyY");
    def_ustring!(rgb_upper, "RGB");
    def_ustring!(colorspace, "colorspace");
    def_ustring!(subimage, "subimage");
    def_ustring!(subimagename, "subimagename");
    def_ustring!(missingcolor, "missingcolor");
    def_ustring!(missingalpha, "missingalpha");
    def_ustring!(end, "end");
    def_ustring!(uninitialized_string, "!!!uninitialized!!!");
    def_ustring!(unull, "unull");
    def_ustring!(raytype, "raytype");
    def_ustring!(arraylength, "arraylength");

    // Additional color spaces
    def_ustring!(ntsc, "NTSC");
    def_ustring!(ebu, "EBU");
    def_ustring!(pal, "PAL");
    def_ustring!(secam, "SECAM");
    def_ustring!(smpte, "SMPTE");
    def_ustring!(hdtv, "HDTV");
    def_ustring!(cie, "CIE");
    def_ustring!(adobergb, "AdobeRGB");
    def_ustring!(aces2065_1, "ACES2065-1");
    def_ustring!(acescg, "ACEScg");
    def_ustring!(colorsystem, "colorsystem");

    // Misc
    def_ustring!(error, "error");
    def_ustring!(unknown, "unknown");
    def_ustring!(useparam, "useparam");

    // Shader types
    def_ustring!(surface, "surface");
    def_ustring!(displacement, "displacement");
    def_ustring!(volume, "volume");
    def_ustring!(light, "light");
    def_ustring!(generic, "shader");
}
