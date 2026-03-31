
//! Core enumerations for Hydra.
//!
//! This module defines fundamental enums used throughout Hydra for graphics
//! operations, interpolation modes, and rendering styles.

/// Compare function for depth/stencil tests.
///
/// Abstraction of graphics API compare functions used in depth and stencil testing.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HdCompareFunction {
    /// Never passes
    Never = 0,
    /// Passes if source < destination
    Less,
    /// Passes if source == destination
    Equal,
    /// Passes if source <= destination
    LEqual,
    /// Passes if source > destination
    Greater,
    /// Passes if source != destination
    NotEqual,
    /// Passes if source >= destination
    GEqual,
    /// Always passes
    Always,
}

/// Stencil test operation.
///
/// Defines what happens to stencil buffer values during stencil testing.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HdStencilOp {
    /// Keep current stencil value
    Keep = 0,
    /// Set stencil value to 0
    Zero,
    /// Replace stencil value with reference value
    Replace,
    /// Increment stencil value, clamp to maximum
    Increment,
    /// Increment stencil value, wrap to 0 on overflow
    IncrementWrap,
    /// Decrement stencil value, clamp to 0
    Decrement,
    /// Decrement stencil value, wrap to maximum on underflow
    DecrementWrap,
    /// Bitwise invert stencil value
    Invert,
}

/// Blend operation for color blending.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HdBlendOp {
    /// Source + destination
    Add = 0,
    /// Source - destination
    Subtract,
    /// Destination - source
    ReverseSubtract,
    /// min(source, destination)
    Min,
    /// max(source, destination)
    Max,
}

/// Blend factor for color blending.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HdBlendFactor {
    /// (0, 0, 0, 0)
    Zero = 0,
    /// (1, 1, 1, 1)
    One,
    /// (Rs, Gs, Bs, As)
    SrcColor,
    /// (1-Rs, 1-Gs, 1-Bs, 1-As)
    OneMinusSrcColor,
    /// (Rd, Gd, Bd, Ad)
    DstColor,
    /// (1-Rd, 1-Gd, 1-Bd, 1-Ad)
    OneMinusDstColor,
    /// (As, As, As, As)
    SrcAlpha,
    /// (1-As, 1-As, 1-As, 1-As)
    OneMinusSrcAlpha,
    /// (Ad, Ad, Ad, Ad)
    DstAlpha,
    /// (1-Ad, 1-Ad, 1-Ad, 1-Ad)
    OneMinusDstAlpha,
    /// (Rc, Gc, Bc, Ac)
    ConstantColor,
    /// (1-Rc, 1-Gc, 1-Bc, 1-Ac)
    OneMinusConstantColor,
    /// (Ac, Ac, Ac, Ac)
    ConstantAlpha,
    /// (1-Ac, 1-Ac, 1-Ac, 1-Ac)
    OneMinusConstantAlpha,
    /// (f, f, f, 1) where f = min(As, 1-Ad)
    SrcAlphaSaturate,
    /// (Rs1, Gs1, Bs1, As1) from second source
    Src1Color,
    /// (1-Rs1, 1-Gs1, 1-Bs1, 1-As1)
    OneMinusSrc1Color,
    /// (As1, As1, As1, As1)
    Src1Alpha,
    /// (1-As1, 1-As1, 1-As1, 1-As1)
    OneMinusSrc1Alpha,
}

/// Face culling style.
///
/// Controls which faces are culled during rasterization.
///
/// - `DontCare`: No opinion, defers to viewer settings
/// - `Nothing`: No culling
/// - `Back`: Cull back-facing triangles
/// - `Front`: Cull front-facing triangles
/// - `BackUnlessDoubleSided`: Cull back faces unless marked double-sided
/// - `FrontUnlessDoubleSided`: Cull front faces unless marked double-sided
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HdCullStyle {
    /// No opinion, defer to viewer
    DontCare = 0,
    /// No culling
    Nothing,
    /// Cull back faces
    Back,
    /// Cull front faces
    Front,
    /// Cull back faces unless marked double-sided
    BackUnlessDoubleSided,
    /// Cull front faces unless marked double-sided
    FrontUnlessDoubleSided,
}

impl HdCullStyle {
    /// Returns the opposite cull style.
    ///
    /// Swaps front/back culling. Returns self for `DontCare` and `Nothing`.
    pub fn invert(self) -> Self {
        match self {
            Self::DontCare => Self::DontCare,
            Self::Nothing => Self::Nothing,
            Self::Back => Self::Front,
            Self::Front => Self::Back,
            Self::BackUnlessDoubleSided => Self::FrontUnlessDoubleSided,
            Self::FrontUnlessDoubleSided => Self::BackUnlessDoubleSided,
        }
    }
}

/// Polygon rasterization mode.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HdPolygonMode {
    /// Fill polygons
    Fill = 0,
    /// Draw polygon edges as lines
    Line,
}

/// Mesh geometry style for rendering.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HdMeshGeomStyle {
    /// Invalid/uninitialized style
    Invalid = 0,
    /// Render surface (refined if subdivision)
    Surf,
    /// Render edges only
    EdgeOnly,
    /// Render edges on surface
    EdgeOnSurf,
    /// Render subdivision hull (coarse mesh)
    Hull,
    /// Render hull edges only
    HullEdgeOnly,
    /// Render hull edges on hull surface
    HullEdgeOnSurf,
    /// Render vertices as points
    Points,
}

/// Basis curves geometry style.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HdBasisCurvesGeomStyle {
    /// Invalid/uninitialized style
    Invalid = 0,
    /// Render as wire (line segments)
    Wire,
    /// Render as patches (ribbons/tubes)
    Patch,
    /// Render control points as points
    Points,
}

/// Points geometry style.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HdPointsGeomStyle {
    /// Invalid/uninitialized style
    Invalid = 0,
    /// Render as points
    Points,
}

/// Primvar interpolation mode.
///
/// Defines how primvar (primitive variable) values are interpolated
/// across geometry:
///
/// - `Constant`: One value for entire primitive
/// - `Uniform`: One value per face/patch
/// - `Varying`: Four values per face, bilinearly interpolated
/// - `Vertex`: Interpolated between vertices using basis function
/// - `FaceVarying`: Four values per face for polygons/subdiv
/// - `Instance`: One value per instance
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HdInterpolation {
    /// One value for entire primitive
    Constant = 0,
    /// One value per face/patch
    Uniform,
    /// Four values per face, bilinearly interpolated
    Varying,
    /// Interpolated between vertices using basis function
    Vertex,
    /// Four values per face for polygons/subdiv, bilinearly interpolated
    FaceVarying,
    /// One value per instance
    Instance,
}

impl HdInterpolation {
    /// Get string representation for debugging.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Constant => "constant",
            Self::Uniform => "uniform",
            Self::Varying => "varying",
            Self::Vertex => "vertex",
            Self::FaceVarying => "faceVarying",
            Self::Instance => "instance",
        }
    }
}

/// Depth priority for depth-based operations.
///
/// Controls whether nearer or farther objects have priority.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HdDepthPriority {
    /// Prioritize objects nearest to camera
    Nearest = 0,
    /// Prioritize objects farthest from camera
    Farthest,
}

/// Wrapping mode for texture sampling.
///
/// Corresponds to HdWrap in types.h for texture sampling,
/// but may also be used for other wrapping contexts.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HdWrap {
    /// Clamp coordinate to range [1/(2N), 1-1/(2N)] where N is texture size
    Clamp = 0,
    /// Creates a repeating pattern
    Repeat,
    /// Clamp coordinate to range [-1/(2N), 1+1/(2N)] where N is texture size
    Black,
    /// Creates a mirrored repeating pattern
    Mirror,
    /// No opinion, texture can define its own wrap mode (fallback: Black)
    NoOpinion,
    /// (deprecated) No opinion with Repeat fallback instead of Black
    LegacyNoOpinionFallbackRepeat,
}

/// Minification filter for texture sampling.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HdMinFilter {
    /// Nearest texel to pixel center
    Nearest = 0,
    /// Weighted average of four closest texels
    Linear,
    /// Nearest texel from nearest mipmap level
    NearestMipmapNearest,
    /// Weighted average from nearest mipmap level
    LinearMipmapNearest,
    /// Nearest texel, interpolated between two nearest mipmaps
    NearestMipmapLinear,
    /// Weighted average, interpolated between two nearest mipmaps
    LinearMipmapLinear,
}

/// Magnification filter for texture sampling.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HdMagFilter {
    /// Nearest texel to pixel center
    Nearest = 0,
    /// Weighted average of four closest texels
    Linear,
}

/// Border color for clamped texture values.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HdBorderColor {
    /// (0, 0, 0, 0)
    TransparentBlack = 0,
    /// (0, 0, 0, 1)
    OpaqueBlack,
    /// (1, 1, 1, 1)
    OpaqueWhite,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cull_style_invert() {
        assert_eq!(HdCullStyle::Back.invert(), HdCullStyle::Front);
        assert_eq!(HdCullStyle::Front.invert(), HdCullStyle::Back);
        assert_eq!(HdCullStyle::Nothing.invert(), HdCullStyle::Nothing);
        assert_eq!(HdCullStyle::DontCare.invert(), HdCullStyle::DontCare);
        assert_eq!(
            HdCullStyle::BackUnlessDoubleSided.invert(),
            HdCullStyle::FrontUnlessDoubleSided
        );
    }

    #[test]
    fn test_interpolation_strings() {
        assert_eq!(HdInterpolation::Constant.as_str(), "constant");
        assert_eq!(HdInterpolation::Vertex.as_str(), "vertex");
        assert_eq!(HdInterpolation::FaceVarying.as_str(), "faceVarying");
    }

    #[test]
    fn test_enum_sizes() {
        use std::mem::size_of;

        // All enums should be C-compatible size
        assert_eq!(size_of::<HdCompareFunction>(), size_of::<i32>());
        assert_eq!(size_of::<HdCullStyle>(), size_of::<i32>());
        assert_eq!(size_of::<HdInterpolation>(), size_of::<i32>());
    }
}
