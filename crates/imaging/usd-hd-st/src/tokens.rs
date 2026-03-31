
//! hdSt tokens - Storm render delegate token definitions.

use once_cell::sync::Lazy;
use usd_tf::Token;

// GLSL program tokens

/// Smooth normals computation: float input to float output.
pub static SMOOTH_NORMALS_FLOAT_TO_FLOAT: Lazy<Token> =
    Lazy::new(|| Token::new("smoothNormalsFloatToFloat"));
/// Smooth normals computation: float input to packed output.
pub static SMOOTH_NORMALS_FLOAT_TO_PACKED: Lazy<Token> =
    Lazy::new(|| Token::new("smoothNormalsFloatToPacked"));
/// Smooth normals computation: double input to double output.
pub static SMOOTH_NORMALS_DOUBLE_TO_DOUBLE: Lazy<Token> =
    Lazy::new(|| Token::new("smoothNormalsDoubleToDouble"));
/// Smooth normals computation: double input to packed output.
pub static SMOOTH_NORMALS_DOUBLE_TO_PACKED: Lazy<Token> =
    Lazy::new(|| Token::new("smoothNormalsDoubleToPacked"));
/// Flat normals for triangles: float input to float output.
pub static FLAT_NORMALS_TRI_FLOAT_TO_FLOAT: Lazy<Token> =
    Lazy::new(|| Token::new("flatNormalsTriFloatToFloat"));
/// Flat normals for triangles: float input to packed output.
pub static FLAT_NORMALS_TRI_FLOAT_TO_PACKED: Lazy<Token> =
    Lazy::new(|| Token::new("flatNormalsTriFloatToPacked"));
/// Flat normals for triangles: double input to double output.
pub static FLAT_NORMALS_TRI_DOUBLE_TO_DOUBLE: Lazy<Token> =
    Lazy::new(|| Token::new("flatNormalsTriDoubleToDouble"));
/// Flat normals for triangles: double input to packed output.
pub static FLAT_NORMALS_TRI_DOUBLE_TO_PACKED: Lazy<Token> =
    Lazy::new(|| Token::new("flatNormalsTriDoubleToPacked"));
/// Flat normals for quads: float input to float output.
pub static FLAT_NORMALS_QUAD_FLOAT_TO_FLOAT: Lazy<Token> =
    Lazy::new(|| Token::new("flatNormalsQuadFloatToFloat"));
/// Flat normals for quads: float input to packed output.
pub static FLAT_NORMALS_QUAD_FLOAT_TO_PACKED: Lazy<Token> =
    Lazy::new(|| Token::new("flatNormalsQuadFloatToPacked"));
/// Flat normals for quads: double input to double output.
pub static FLAT_NORMALS_QUAD_DOUBLE_TO_DOUBLE: Lazy<Token> =
    Lazy::new(|| Token::new("flatNormalsQuadDoubleToDouble"));
/// Flat normals for quads: double input to packed output.
pub static FLAT_NORMALS_QUAD_DOUBLE_TO_PACKED: Lazy<Token> =
    Lazy::new(|| Token::new("flatNormalsQuadDoubleToPacked"));
/// Flat normals for mixed tri/quad meshes: float input to float output.
pub static FLAT_NORMALS_TRI_QUAD_FLOAT_TO_FLOAT: Lazy<Token> =
    Lazy::new(|| Token::new("flatNormalsTriQuadFloatToFloat"));
/// Flat normals for mixed tri/quad meshes: float input to packed output.
pub static FLAT_NORMALS_TRI_QUAD_FLOAT_TO_PACKED: Lazy<Token> =
    Lazy::new(|| Token::new("flatNormalsTriQuadFloatToPacked"));
/// Flat normals for mixed tri/quad meshes: double input to double output.
pub static FLAT_NORMALS_TRI_QUAD_DOUBLE_TO_DOUBLE: Lazy<Token> =
    Lazy::new(|| Token::new("flatNormalsTriQuadDoubleToDouble"));
/// Flat normals for mixed tri/quad meshes: double input to packed output.
pub static FLAT_NORMALS_TRI_QUAD_DOUBLE_TO_PACKED: Lazy<Token> =
    Lazy::new(|| Token::new("flatNormalsTriQuadDoubleToPacked"));
/// Quadrangulation computation for float precision.
pub static QUADRANGULATE_FLOAT: Lazy<Token> = Lazy::new(|| Token::new("quadrangulateFloat"));
/// Quadrangulation computation for double precision.
pub static QUADRANGULATE_DOUBLE: Lazy<Token> = Lazy::new(|| Token::new("quadrangulateDouble"));

// General hdSt tokens

/// Constant (unlit) lighting mode.
pub static CONSTANT_LIGHTING: Lazy<Token> = Lazy::new(|| Token::new("constantLighting"));
/// Packed representation of smooth normals.
pub static PACKED_SMOOTH_NORMALS: Lazy<Token> = Lazy::new(|| Token::new("packedSmoothNormals"));
/// Smooth (vertex-averaged) normals.
pub static SMOOTH_NORMALS: Lazy<Token> = Lazy::new(|| Token::new("smoothNormals"));
/// Packed representation of flat normals.
pub static PACKED_FLAT_NORMALS: Lazy<Token> = Lazy::new(|| Token::new("packedFlatNormals"));
/// Flat (face) normals.
pub static FLAT_NORMALS: Lazy<Token> = Lazy::new(|| Token::new("flatNormals"));
/// Scale transform component.
pub static SCALE: Lazy<Token> = Lazy::new(|| Token::new("scale"));
/// Bias value.
pub static BIAS: Lazy<Token> = Lazy::new(|| Token::new("bias"));
/// Rotation transform component.
pub static ROTATION: Lazy<Token> = Lazy::new(|| Token::new("rotation"));
/// Translation transform component.
pub static TRANSLATION: Lazy<Token> = Lazy::new(|| Token::new("translation"));
/// sRGB color space.
pub static SRGB: Lazy<Token> = Lazy::new(|| Token::new("sRGB"));
/// Raw (linear) color space.
pub static RAW: Lazy<Token> = Lazy::new(|| Token::new("raw"));
/// Double precision floating-point type.
pub static DOUBLE: Lazy<Token> = Lazy::new(|| Token::new("double"));
/// Single precision floating-point type.
pub static FLOAT: Lazy<Token> = Lazy::new(|| Token::new("float"));
/// Integer type.
pub static INT: Lazy<Token> = Lazy::new(|| Token::new("int"));
/// Automatic color space detection.
pub static COLOR_SPACE_AUTO: Lazy<Token> = Lazy::new(|| Token::new("auto"));
/// Face-varying primitive variable indices.
pub static FVAR_INDICES: Lazy<Token> = Lazy::new(|| Token::new("fvarIndices"));
/// Face-varying patch parameter.
pub static FVAR_PATCH_PARAM: Lazy<Token> = Lazy::new(|| Token::new("fvarPatchParam"));
/// Index of coarse face in subdivision surface.
pub static COARSE_FACE_INDEX: Lazy<Token> = Lazy::new(|| Token::new("coarseFaceIndex"));
/// Processed mesh face counts.
pub static PROCESSED_FACE_COUNTS: Lazy<Token> = Lazy::new(|| Token::new("processedFaceCounts"));
/// Processed mesh face indices.
pub static PROCESSED_FACE_INDICES: Lazy<Token> = Lazy::new(|| Token::new("processedFaceIndices"));
/// Geometry subset face indices.
pub static GEOM_SUBSET_FACE_INDICES: Lazy<Token> =
    Lazy::new(|| Token::new("geomSubsetFaceIndices"));
/// Scale factor for point size.
pub static POINT_SIZE_SCALE: Lazy<Token> = Lazy::new(|| Token::new("pointSizeScale"));
/// Screen-space curve widths.
pub static SCREEN_SPACE_WIDTHS: Lazy<Token> = Lazy::new(|| Token::new("screenSpaceWidths"));
/// Minimum screen-space curve widths.
pub static MIN_SCREEN_SPACE_WIDTHS: Lazy<Token> = Lazy::new(|| Token::new("minScreenSpaceWidths"));
/// Shadow comparison texture samplers.
pub static SHADOW_COMPARE_TEXTURES: Lazy<Token> = Lazy::new(|| Token::new("shadowCompareTextures"));
/// Storm render delegate identifier.
pub static STORM: Lazy<Token> = Lazy::new(|| Token::new("storm"));

// Texture tokens

/// Texture wrap mode in S (U) direction.
pub static WRAP_S: Lazy<Token> = Lazy::new(|| Token::new("wrapS"));
/// Texture wrap mode in T (V) direction.
pub static WRAP_T: Lazy<Token> = Lazy::new(|| Token::new("wrapT"));
/// Texture wrap mode in R (W) direction for 3D textures.
pub static WRAP_R: Lazy<Token> = Lazy::new(|| Token::new("wrapR"));
/// Black border color wrap mode.
pub static BLACK: Lazy<Token> = Lazy::new(|| Token::new("black"));
/// Clamp to edge wrap mode.
pub static CLAMP: Lazy<Token> = Lazy::new(|| Token::new("clamp"));
/// Mirror repeat wrap mode.
pub static MIRROR: Lazy<Token> = Lazy::new(|| Token::new("mirror"));
/// Repeat wrap mode.
pub static REPEAT: Lazy<Token> = Lazy::new(|| Token::new("repeat"));
/// Use texture metadata for sampling parameters.
pub static USE_METADATA: Lazy<Token> = Lazy::new(|| Token::new("useMetadata"));
/// Texture minification filter.
pub static MIN_FILTER: Lazy<Token> = Lazy::new(|| Token::new("minFilter"));
/// Texture magnification filter.
pub static MAG_FILTER: Lazy<Token> = Lazy::new(|| Token::new("magFilter"));
/// Linear filtering mode.
pub static LINEAR: Lazy<Token> = Lazy::new(|| Token::new("linear"));
/// Nearest (point) filtering mode.
pub static NEAREST: Lazy<Token> = Lazy::new(|| Token::new("nearest"));
/// Linear mipmap filtering with linear interpolation between levels.
pub static LINEAR_MIPMAP_LINEAR: Lazy<Token> = Lazy::new(|| Token::new("linearMipmapLinear"));
/// Linear mipmap filtering with nearest level selection.
pub static LINEAR_MIPMAP_NEAREST: Lazy<Token> = Lazy::new(|| Token::new("linearMipmapNearest"));
/// Nearest mipmap filtering with linear interpolation between levels.
pub static NEAREST_MIPMAP_LINEAR: Lazy<Token> = Lazy::new(|| Token::new("nearestMipmapLinear"));
/// Nearest mipmap filtering with nearest level selection.
pub static NEAREST_MIPMAP_NEAREST: Lazy<Token> = Lazy::new(|| Token::new("nearestMipmapNearest"));

// Render buffer tokens

/// Storm MSAA sample count render buffer setting.
pub static STORM_MSAA_SAMPLE_COUNT: Lazy<Token> = Lazy::new(|| Token::new("storm:msaaSampleCount"));

// Render settings tokens

/// Enable culling of sub-pixel primitives.
pub static ENABLE_TINY_PRIM_CULLING: Lazy<Token> =
    Lazy::new(|| Token::new("enableTinyPrimCulling"));
/// Volume raymarching step size.
pub static VOLUME_RAYMARCHING_STEP_SIZE: Lazy<Token> =
    Lazy::new(|| Token::new("volumeRaymarchingStepSize"));
/// Volume raymarching step size for lighting computations.
pub static VOLUME_RAYMARCHING_STEP_SIZE_LIGHTING: Lazy<Token> =
    Lazy::new(|| Token::new("volumeRaymarchingStepSizeLighting"));
/// Maximum GPU texture memory per volume field.
pub static VOLUME_MAX_TEXTURE_MEMORY_PER_FIELD: Lazy<Token> =
    Lazy::new(|| Token::new("volumeMaxTextureMemoryPerField"));
/// Maximum number of lights to process.
pub static MAX_LIGHTS: Lazy<Token> = Lazy::new(|| Token::new("maxLights"));
/// Target memory size for dome light cubemaps.
pub static DOME_LIGHT_CUBEMAP_TARGET_MEMORY: Lazy<Token> =
    Lazy::new(|| Token::new("domeLightCubemapTargetMemory"));

// Material tag tokens - used for bucketing prims into draw queues.
// Tags supported by Storm:
// - defaultMaterialTag: opaque geometry
// - masked: opaque geometry with cutout masks (e.g., foliage)
// - displayInOverlay: geometry drawn on top (e.g., guides)
// - translucentToSelection: opaque geometry allowing occluded selection to show through
// - additive: transparent geometry (cheap OIT without sorting)

/// Default material tag for opaque geometry.
pub static DEFAULT_MATERIAL_TAG: Lazy<Token> = Lazy::new(|| Token::new("defaultMaterialTag"));
/// Masked material tag for opaque geometry with cutout masks.
pub static MASKED: Lazy<Token> = Lazy::new(|| Token::new("masked"));
/// Display in overlay material tag for geometry drawn on top.
pub static DISPLAY_IN_OVERLAY: Lazy<Token> = Lazy::new(|| Token::new("displayInOverlay"));
/// Translucent to selection material tag for opaque geometry with selection show-through.
pub static TRANSLUCENT_TO_SELECTION: Lazy<Token> =
    Lazy::new(|| Token::new("translucentToSelection"));
/// Additive material tag for transparent geometry without sorting.
pub static ADDITIVE: Lazy<Token> = Lazy::new(|| Token::new("additive"));
/// Translucent material tag for transparent geometry with sorted fragment lists (OIT).
pub static TRANSLUCENT: Lazy<Token> = Lazy::new(|| Token::new("translucent"));
/// Volume material tag for raymarched transparent geometry.
pub static VOLUME: Lazy<Token> = Lazy::new(|| Token::new("volume"));

// SDR metadata tokens

/// Swizzle pattern for texture channel remapping.
pub static SWIZZLE: Lazy<Token> = Lazy::new(|| Token::new("swizzle"));

// GLSL program tokens (HDST_GLSL_PROGRAM_TOKENS from tokens.h)

/// Smooth normals: double to double GLSL program.
pub static SMOOTH_NORMALS_DOUBLE_TO_DOUBLE_GLSL: Lazy<Token> =
    Lazy::new(|| Token::new("HdSt_SmoothNormalsComputeDoubleToDouble"));
/// Flat normals: tri double to double GLSL program.
pub static FLAT_NORMALS_TRI_DOUBLE_TO_DOUBLE_GLSL: Lazy<Token> =
    Lazy::new(|| Token::new("HdSt_FlatNormalsComputeTriDoubleToDouble"));
/// Mipmap generation GLSL program.
pub static GENERATE_MIPMAPS: Lazy<Token> = Lazy::new(|| Token::new("HdSt_GenerateMipmaps"));
/// Quadrangulate GLSL program.
pub static QUADRANGULATE_COMPUTE: Lazy<Token> = Lazy::new(|| Token::new("HdSt_Quadrangulate"));
/// Subdivision OSD GLSL program.
pub static SUBDIVISION_OSD: Lazy<Token> = Lazy::new(|| Token::new("HdSt_OsdRefine"));
/// Basis curves ribbon GLSL program.
pub static BASIS_CURVES_LINEAR_PATCHES: Lazy<Token> =
    Lazy::new(|| Token::new("HdSt_BasisCurvesLinearPatches"));
/// Basis curves cubic patches GLSL program.
pub static BASIS_CURVES_CUBIC_PATCHES: Lazy<Token> =
    Lazy::new(|| Token::new("HdSt_BasisCurvesCubicPatches"));
/// Basis curves cubic vertex GLSL program.
pub static BASIS_CURVES_CUBIC_VERTEX: Lazy<Token> =
    Lazy::new(|| Token::new("HdSt_BasisCurvesCubicVertex"));
/// Basis curves linear vertex GLSL program.
pub static BASIS_CURVES_LINEAR_VERTEX: Lazy<Token> =
    Lazy::new(|| Token::new("HdSt_BasisCurvesLinearVertex"));
/// Point count computation GLSL program.
pub static POINT_ID: Lazy<Token> = Lazy::new(|| Token::new("HdSt_PointId"));
/// Selection highlight GLSL program.
pub static SELECTION_HIGHLIGHT: Lazy<Token> = Lazy::new(|| Token::new("HdSt_SelectionHighlight"));
/// Shadow computation GLSL program.
pub static SHADOW_COMPUTATION: Lazy<Token> = Lazy::new(|| Token::new("HdSt_ShadowComputation"));

// Additional render buffer tokens (HDST_RENDER_BUFFER_TOKENS)

/// Number of MSAA samples in a render buffer (integer).
pub static MULTI_SAMPLE_ANTI_ALIASING_SAMPLE_COUNT: Lazy<Token> =
    Lazy::new(|| Token::new("storm:multiSampleCount"));

// Additional render settings tokens (HDST_RENDER_SETTINGS_TOKENS)

/// Maximum number of shadow maps.
pub static SHADOW_MAP_COUNT: Lazy<Token> = Lazy::new(|| Token::new("shadowMapCount"));
/// Enable shadow computation.
pub static ENABLE_SHADOWS: Lazy<Token> = Lazy::new(|| Token::new("enableShadows"));
/// Enable camera light.
pub static ENABLE_CAMERA_LIGHT: Lazy<Token> = Lazy::new(|| Token::new("enableCameraLight"));
/// Step size for transparency BSDF integration.
pub static TRANSPARENCY_STEP_SIZE: Lazy<Token> = Lazy::new(|| Token::new("transparencyStepSize"));
/// Complexity level for tessellation.
pub static COMPLEXITY: Lazy<Token> = Lazy::new(|| Token::new("complexity"));

// Performance counter tokens

/// GPU-to-GPU buffer copy operation.
pub static PERF_COPY_BUFFER_GPU_TO_GPU: Lazy<Token> =
    Lazy::new(|| Token::new("copyBufferGpuToGpu"));
/// CPU-to-GPU buffer copy operation.
pub static PERF_COPY_BUFFER_CPU_TO_GPU: Lazy<Token> =
    Lazy::new(|| Token::new("copyBufferCpuToGpu"));
/// Draw items cache hit.
pub static PERF_DRAW_ITEMS_CACHE_HIT: Lazy<Token> = Lazy::new(|| Token::new("drawItemsCacheHit"));
/// Draw items cache miss.
pub static PERF_DRAW_ITEMS_CACHE_MISS: Lazy<Token> = Lazy::new(|| Token::new("drawItemsCacheMiss"));
/// Draw items cache stale entry.
pub static PERF_DRAW_ITEMS_CACHE_STALE: Lazy<Token> =
    Lazy::new(|| Token::new("drawItemsCacheStale"));
/// Draw items fetched from render index.
pub static PERF_DRAW_ITEMS_FETCHED: Lazy<Token> = Lazy::new(|| Token::new("drawItemsFetched"));
