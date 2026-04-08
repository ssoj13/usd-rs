//! Token definitions for Hydra.
//!
//! This module defines all the standard tokens used throughout Hydra for
//! identifying prims, properties, and various attributes.

use once_cell::sync::Lazy;
use usd_tf::Token;

// Core property tokens
/// Point acceleration attribute.
pub static ACCELERATIONS: Lazy<Token> = Lazy::new(|| Token::new("accelerations"));
/// Mesh face adjacency information.
pub static ADJACENCY: Lazy<Token> = Lazy::new(|| Token::new("adjacency"));
/// Angular velocity attribute for motion blur.
pub static ANGULAR_VELOCITIES: Lazy<Token> = Lazy::new(|| Token::new("angularVelocities"));
/// Bounding box attribute.
pub static BBOX: Lazy<Token> = Lazy::new(|| Token::new("bbox"));
/// Local space minimum bound.
pub static BBOX_LOCAL_MIN: Lazy<Token> = Lazy::new(|| Token::new("bboxLocalMin"));
/// Local space maximum bound.
pub static BBOX_LOCAL_MAX: Lazy<Token> = Lazy::new(|| Token::new("bboxLocalMax"));
/// Bezier curve basis type.
pub static BEZIER: Lazy<Token> = Lazy::new(|| Token::new("bezier"));
/// B-spline curve basis type.
pub static BSPLINE: Lazy<Token> = Lazy::new(|| Token::new("bspline"));
/// Motion blur scale factor.
pub static BLUR_SCALE: Lazy<Token> = Lazy::new(|| Token::new("blurScale"));
/// Camera prim type.
pub static CAMERA: Lazy<Token> = Lazy::new(|| Token::new("camera"));
/// Catmull-Rom curve basis type.
pub static CATMULL_ROM: Lazy<Token> = Lazy::new(|| Token::new("catmullRom"));
/// Centripetal Catmull-Rom spline type.
pub static CENTRIPETAL_CATMULL_ROM: Lazy<Token> = Lazy::new(|| Token::new("centripetalCatmullRom"));
/// Collection prim type.
pub static COLLECTION: Lazy<Token> = Lazy::new(|| Token::new("collection"));
/// Compute shader resource type.
pub static COMPUTE_SHADER: Lazy<Token> = Lazy::new(|| Token::new("computeShader"));
/// Coordinate system bindings.
pub static COORD_SYS_BINDINGS: Lazy<Token> = Lazy::new(|| Token::new("coordSysBindings"));
/// Cubic interpolation type.
pub static CUBIC: Lazy<Token> = Lazy::new(|| Token::new("cubic"));
/// Face culling style attribute.
pub static CULL_STYLE: Lazy<Token> = Lazy::new(|| Token::new("cullStyle"));
/// Display color primvar.
pub static DISPLAY_COLOR: Lazy<Token> = Lazy::new(|| Token::new("displayColor"));
/// Display opacity primvar.
pub static DISPLAY_OPACITY: Lazy<Token> = Lazy::new(|| Token::new("displayOpacity"));
/// Display style render attribute.
pub static DISPLAY_STYLE: Lazy<Token> = Lazy::new(|| Token::new("displayStyle"));
/// Double-sided surface attribute.
pub static DOUBLE_SIDED: Lazy<Token> = Lazy::new(|| Token::new("doubleSided"));
/// Drawing coordinate channel 0.
pub static DRAWING_COORD_0: Lazy<Token> = Lazy::new(|| Token::new("drawingCoord0"));
/// Drawing coordinate channel 1.
pub static DRAWING_COORD_1: Lazy<Token> = Lazy::new(|| Token::new("drawingCoord1"));
/// Drawing coordinate channel 2.
pub static DRAWING_COORD_2: Lazy<Token> = Lazy::new(|| Token::new("drawingCoord2"));
/// Instance drawing coordinate index.
pub static DRAWING_COORD_I: Lazy<Token> = Lazy::new(|| Token::new("drawingCoordI"));
/// Bounding extent attribute.
pub static EXTENT: Lazy<Token> = Lazy::new(|| Token::new("extent"));
/// Per-face color attribute.
pub static FACE_COLORS: Lazy<Token> = Lazy::new(|| Token::new("faceColors"));
/// Geometry data category.
pub static GEOMETRY: Lazy<Token> = Lazy::new(|| Token::new("geometry"));
/// Hermite curve basis type.
pub static HERMITE: Lazy<Token> = Lazy::new(|| Token::new("hermite"));
/// Subdivision hull face indices.
pub static HULL_INDICES: Lazy<Token> = Lazy::new(|| Token::new("hullIndices"));
/// Vertex or face indices array.
pub static INDICES: Lazy<Token> = Lazy::new(|| Token::new("indices"));
/// Surface orientation flip flag.
pub static IS_FLIPPED: Lazy<Token> = Lazy::new(|| Token::new("isFlipped"));
/// Left-handed coordinate system.
pub static LEFT_HANDED: Lazy<Token> = Lazy::new(|| Token::new("leftHanded"));
/// Linear interpolation type.
pub static LINEAR: Lazy<Token> = Lazy::new(|| Token::new("linear"));
/// Material shader parameters.
pub static MATERIAL_PARAMS: Lazy<Token> = Lazy::new(|| Token::new("materialParams"));
/// Vertex normal vectors.
pub static NORMALS: Lazy<Token> = Lazy::new(|| Token::new("normals"));
/// Generic parameter attribute.
pub static PARAMS: Lazy<Token> = Lazy::new(|| Token::new("params"));
/// Subdivision patch parameterization.
pub static PATCH_PARAM: Lazy<Token> = Lazy::new(|| Token::new("patchParam"));
/// Periodic wrap mode.
pub static PERIODIC: Lazy<Token> = Lazy::new(|| Token::new("periodic"));
/// Nonperiodic wrap mode (curve endpoints not joined).
pub static NONPERIODIC: Lazy<Token> = Lazy::new(|| Token::new("nonperiodic"));
/// Pinned wrap mode (endpoints replicated for smooth tangent).
pub static PINNED: Lazy<Token> = Lazy::new(|| Token::new("pinned"));
/// Vertex position array.
pub static POINTS: Lazy<Token> = Lazy::new(|| Token::new("points"));
/// Point cloud index array.
pub static POINTS_INDICES: Lazy<Token> = Lazy::new(|| Token::new("pointsIndices"));
/// Per-point visibility flags.
pub static POINTS_VISIBILITY: Lazy<Token> = Lazy::new(|| Token::new("pointsVisibility"));
/// Preview render mode.
pub static PREVIEW: Lazy<Token> = Lazy::new(|| Token::new("preview"));
/// Primitive identifier.
pub static PRIM_ID: Lazy<Token> = Lazy::new(|| Token::new("primID"));
/// Primvar data category.
pub static PRIMVAR: Lazy<Token> = Lazy::new(|| Token::new("primvar"));
/// Primitive parameterization data.
pub static PRIMITIVE_PARAM: Lazy<Token> = Lazy::new(|| Token::new("primitiveParam"));
/// Dispatch count for compute shaders.
pub static DISPATCH_COUNT: Lazy<Token> = Lazy::new(|| Token::new("dispatchCount"));
/// Draw dispatch token.
pub static DRAW_DISPATCH: Lazy<Token> = Lazy::new(|| Token::new("drawDispatch"));
/// Drawing shader token.
pub static DRAWING_SHADER: Lazy<Token> = Lazy::new(|| Token::new("drawingShader"));
/// Edge indices.
pub static EDGE_INDICES: Lazy<Token> = Lazy::new(|| Token::new("edgeIndices"));
/// Element count.
pub static ELEMENT_COUNT: Lazy<Token> = Lazy::new(|| Token::new("elementCount"));
/// Elements visibility.
pub static ELEMENTS_VISIBILITY: Lazy<Token> = Lazy::new(|| Token::new("elementsVisibility"));
/// Filters token.
pub static FILTERS: Lazy<Token> = Lazy::new(|| Token::new("filters"));
/// Full quality/purpose.
pub static FULL: Lazy<Token> = Lazy::new(|| Token::new("full"));
/// Items drawn count.
pub static ITEMS_DRAWN: Lazy<Token> = Lazy::new(|| Token::new("itemsDrawn"));
/// Layout token.
pub static LAYOUT: Lazy<Token> = Lazy::new(|| Token::new("layout"));
/// Light link collection.
pub static LIGHT_LINK: Lazy<Token> = Lazy::new(|| Token::new("lightLink"));
/// Filter link collection.
pub static FILTER_LINK: Lazy<Token> = Lazy::new(|| Token::new("filterLink"));
/// Light filter link.
pub static LIGHT_FILTER_LINK: Lazy<Token> = Lazy::new(|| Token::new("lightFilterLink"));
/// Light filter type.
pub static LIGHT_FILTER_TYPE: Lazy<Token> = Lazy::new(|| Token::new("lightFilterType"));
/// Mesh light.
pub static MESH_LIGHT: Lazy<Token> = Lazy::new(|| Token::new("meshLight"));
/// Material sync mode.
pub static MATERIAL_SYNC_MODE: Lazy<Token> = Lazy::new(|| Token::new("materialSyncMode"));
/// Portals token.
pub static PORTALS: Lazy<Token> = Lazy::new(|| Token::new("portals"));
/// Power token.
pub static POWER: Lazy<Token> = Lazy::new(|| Token::new("power"));
/// Tessellation factors.
pub static TESS_FACTORS: Lazy<Token> = Lazy::new(|| Token::new("tessFactors"));
/// Quad info.
pub static QUAD_INFO: Lazy<Token> = Lazy::new(|| Token::new("quadInfo"));
/// Task state.
pub static TASK_STATE: Lazy<Token> = Lazy::new(|| Token::new("taskState"));
/// Task params.
pub static TASK_PARAMS: Lazy<Token> = Lazy::new(|| Token::new("taskParams"));
/// Total item count.
pub static TOTAL_ITEM_COUNT: Lazy<Token> = Lazy::new(|| Token::new("totalItemCount"));
/// Is light flag.
pub static IS_LIGHT: Lazy<Token> = Lazy::new(|| Token::new("isLight"));
/// Legacy bSpline alias (same as bspline).
pub static BSPLINE_LEGACY: Lazy<Token> = Lazy::new(|| Token::new("bspline"));
/// Drivers token.
pub static DRIVERS: Lazy<Token> = Lazy::new(|| Token::new("drivers"));
/// Shadow link collection.
pub static SHADOW_LINK: Lazy<Token> = Lazy::new(|| Token::new("shadowLink"));
/// Render pass tags.
pub static RENDER_TAGS: Lazy<Token> = Lazy::new(|| Token::new("renderTags"));
/// Subdivision refine level.
pub static REFINE_LEVEL: Lazy<Token> = Lazy::new(|| Token::new("refineLevel"));
/// Right-handed coordinate system.
pub static RIGHT_HANDED: Lazy<Token> = Lazy::new(|| Token::new("rightHanded"));
/// Segmented curve wrap type.
pub static SEGMENTED: Lazy<Token> = Lazy::new(|| Token::new("segmented"));
/// Subdivision surface tags.
pub static SUBDIV_TAGS: Lazy<Token> = Lazy::new(|| Token::new("subdivTags"));
/// Mesh topology data.
pub static TOPOLOGY: Lazy<Token> = Lazy::new(|| Token::new("topology"));
/// Per-face visibility flags.
pub static TOPOLOGY_VISIBILITY: Lazy<Token> = Lazy::new(|| Token::new("topologyVisibility"));
/// Local to world transform.
pub static TRANSFORM: Lazy<Token> = Lazy::new(|| Token::new("transform"));
/// World to local transform.
pub static TRANSFORM_INVERSE: Lazy<Token> = Lazy::new(|| Token::new("transformInverse"));
/// Velocity vectors for motion blur.
pub static VELOCITIES: Lazy<Token> = Lazy::new(|| Token::new("velocities"));
/// Nonlinear sample count for motion blur (UsdGeomMotionAPI).
pub static NONLINEAR_SAMPLE_COUNT: Lazy<Token> = Lazy::new(|| Token::new("nonlinearSampleCount"));
/// Prim visibility flag.
pub static VISIBILITY: Lazy<Token> = Lazy::new(|| Token::new("visibility"));
/// Curve or point width attribute.
pub static WIDTHS: Lazy<Token> = Lazy::new(|| Token::new("widths"));

// Instancer tokens
/// Indices of culled instances.
pub static CULLED_INSTANCE_INDICES: Lazy<Token> = Lazy::new(|| Token::new("culledInstanceIndices"));
/// Instancer prim type.
pub static INSTANCER: Lazy<Token> = Lazy::new(|| Token::new("instancer"));
/// Instancer's local to world transform.
pub static INSTANCER_TRANSFORM: Lazy<Token> = Lazy::new(|| Token::new("instancerTransform"));
/// Instancer's world to local transform.
pub static INSTANCER_TRANSFORM_INVERSE: Lazy<Token> =
    Lazy::new(|| Token::new("instancerTransformInverse"));
/// Per-instance index array.
pub static INSTANCE_INDICES: Lazy<Token> = Lazy::new(|| Token::new("instanceIndices"));
/// Base offset for instance indices.
pub static INSTANCE_INDEX_BASE: Lazy<Token> = Lazy::new(|| Token::new("instanceIndexBase"));
/// Per-instance transform matrices.
pub static INSTANCE_TRANSFORMS: Lazy<Token> = Lazy::new(|| Token::new("hydra:instanceTransforms"));
/// Per-instance rotation quaternions.
pub static INSTANCE_ROTATIONS: Lazy<Token> = Lazy::new(|| Token::new("hydra:instanceRotations"));
/// Per-instance scale vectors.
pub static INSTANCE_SCALES: Lazy<Token> = Lazy::new(|| Token::new("hydra:instanceScales"));
/// Per-instance translation vectors.
pub static INSTANCE_TRANSLATIONS: Lazy<Token> =
    Lazy::new(|| Token::new("hydra:instanceTranslations"));

// Repr tokens
/// Disabled representation.
pub static REPR_DISABLED: Lazy<Token> = Lazy::new(|| Token::new("disabled"));
/// Coarse subdivision hull.
pub static REPR_HULL: Lazy<Token> = Lazy::new(|| Token::new("hull"));
/// Point cloud representation.
pub static REPR_POINTS: Lazy<Token> = Lazy::new(|| Token::new("points"));
/// Smooth shaded hull.
pub static REPR_SMOOTH_HULL: Lazy<Token> = Lazy::new(|| Token::new("smoothHull"));
/// Refined subdivision surface.
pub static REPR_REFINED: Lazy<Token> = Lazy::new(|| Token::new("refined"));
/// Refined wireframe representation.
pub static REPR_REFINED_WIRE: Lazy<Token> = Lazy::new(|| Token::new("refinedWire"));
/// Refined wireframe on surface.
pub static REPR_REFINED_WIRE_ON_SURF: Lazy<Token> = Lazy::new(|| Token::new("refinedWireOnSurf"));
/// Refined solid with wireframe overlay.
pub static REPR_REFINED_SOLID_WIRE_ON_SURF: Lazy<Token> =
    Lazy::new(|| Token::new("refinedSolidWireOnSurf"));
/// Wireframe representation.
pub static REPR_WIRE: Lazy<Token> = Lazy::new(|| Token::new("wire"));
/// Wireframe on shaded surface.
pub static REPR_WIRE_ON_SURF: Lazy<Token> = Lazy::new(|| Token::new("wireOnSurf"));
/// Solid with wireframe overlay.
pub static REPR_SOLID_WIRE_ON_SURF: Lazy<Token> = Lazy::new(|| Token::new("solidWireOnSurf"));

// Cull style tokens
/// No culling preference.
pub static CULL_STYLE_DONT_CARE: Lazy<Token> = Lazy::new(|| Token::new("dontCare"));
/// Disable face culling.
pub static CULL_STYLE_NOTHING: Lazy<Token> = Lazy::new(|| Token::new("nothing"));
/// Cull back-facing polygons.
pub static CULL_STYLE_BACK: Lazy<Token> = Lazy::new(|| Token::new("back"));
/// Cull front-facing polygons.
pub static CULL_STYLE_FRONT: Lazy<Token> = Lazy::new(|| Token::new("front"));
/// Cull back faces unless double-sided flag set.
pub static CULL_STYLE_BACK_UNLESS_DOUBLE_SIDED: Lazy<Token> =
    Lazy::new(|| Token::new("backUnlessDoubleSided"));
/// Cull front faces unless double-sided flag set.
pub static CULL_STYLE_FRONT_UNLESS_DOUBLE_SIDED: Lazy<Token> =
    Lazy::new(|| Token::new("frontUnlessDoubleSided"));

// Rprim type tokens
/// Capsule primitive type.
pub static RPRIM_CAPSULE: Lazy<Token> = Lazy::new(|| Token::new("capsule"));
/// Cone primitive type.
pub static RPRIM_CONE: Lazy<Token> = Lazy::new(|| Token::new("cone"));
/// Cube primitive type.
pub static RPRIM_CUBE: Lazy<Token> = Lazy::new(|| Token::new("cube"));
/// Cylinder primitive type.
pub static RPRIM_CYLINDER: Lazy<Token> = Lazy::new(|| Token::new("cylinder"));
/// Geometry subset primitive type.
pub static RPRIM_GEOM_SUBSET: Lazy<Token> = Lazy::new(|| Token::new("geomSubset"));
/// Polygon mesh primitive type.
pub static RPRIM_MESH: Lazy<Token> = Lazy::new(|| Token::new("mesh"));
/// Tetrahedral mesh primitive type.
pub static RPRIM_TET_MESH: Lazy<Token> = Lazy::new(|| Token::new("tetMesh"));
/// NURBS patch primitive type.
pub static RPRIM_NURBS_PATCH: Lazy<Token> = Lazy::new(|| Token::new("nurbsPatch"));
/// Basis curves primitive type.
pub static RPRIM_BASIS_CURVES: Lazy<Token> = Lazy::new(|| Token::new("basisCurves"));
/// NURBS curves primitive type.
pub static RPRIM_NURBS_CURVES: Lazy<Token> = Lazy::new(|| Token::new("nurbsCurves"));
/// Infinite plane primitive type.
pub static RPRIM_PLANE: Lazy<Token> = Lazy::new(|| Token::new("plane"));
/// Point cloud primitive type.
pub static RPRIM_POINTS: Lazy<Token> = Lazy::new(|| Token::new("points"));
/// Sphere primitive type.
pub static RPRIM_SPHERE: Lazy<Token> = Lazy::new(|| Token::new("sphere"));
/// Volume primitive type.
pub static RPRIM_VOLUME: Lazy<Token> = Lazy::new(|| Token::new("volume"));
/// Model reference primitive type.
pub static RPRIM_MODEL: Lazy<Token> = Lazy::new(|| Token::new("model"));

// Light type tokens
/// Cylinder area light type.
pub static LIGHT_CYLINDER: Lazy<Token> = Lazy::new(|| Token::new("cylinderLight"));
/// Disk area light type.
pub static LIGHT_DISK: Lazy<Token> = Lazy::new(|| Token::new("diskLight"));
/// Distant directional light type.
pub static LIGHT_DISTANT: Lazy<Token> = Lazy::new(|| Token::new("distantLight"));
/// Dome environment light type.
pub static LIGHT_DOME: Lazy<Token> = Lazy::new(|| Token::new("domeLight"));
/// Generic light prim type.
pub static LIGHT: Lazy<Token> = Lazy::new(|| Token::new("light"));
/// Mesh emissive light type.
pub static LIGHT_MESH: Lazy<Token> = Lazy::new(|| Token::new("meshLight"));
/// Plugin-defined light type.
pub static LIGHT_PLUGIN: Lazy<Token> = Lazy::new(|| Token::new("pluginLight"));
/// Rectangular area light type.
pub static LIGHT_RECT: Lazy<Token> = Lazy::new(|| Token::new("rectLight"));
/// Simple point light type.
pub static LIGHT_SIMPLE: Lazy<Token> = Lazy::new(|| Token::new("simpleLight"));
/// Spherical area light type.
pub static LIGHT_SPHERE: Lazy<Token> = Lazy::new(|| Token::new("sphereLight"));
/// Light filter sprim type (HD_LIGHT_FILTER_TYPE_TOKENS).
pub static LIGHT_FILTER: Lazy<Token> = Lazy::new(|| Token::new("lightFilter"));

// Sprim type tokens
/// Camera state primitive type.
pub static SPRIM_CAMERA: Lazy<Token> = Lazy::new(|| Token::new("camera"));
/// Draw target state primitive type.
pub static SPRIM_DRAW_TARGET: Lazy<Token> = Lazy::new(|| Token::new("drawTarget"));
/// External computation state primitive type.
pub static SPRIM_EXT_COMPUTATION: Lazy<Token> = Lazy::new(|| Token::new("extComputation"));
/// Material state primitive type.
pub static SPRIM_MATERIAL: Lazy<Token> = Lazy::new(|| Token::new("material"));
/// Render settings primitive type.
pub static SPRIM_RENDER_SETTINGS: Lazy<Token> = Lazy::new(|| Token::new("renderSettings"));
/// Render pass primitive type.
pub static SPRIM_RENDER_PASS: Lazy<Token> = Lazy::new(|| Token::new("renderPass"));
/// Coordinate system primitive type.
pub static SPRIM_COORD_SYS: Lazy<Token> = Lazy::new(|| Token::new("coordSys"));
/// Integrator sprim type.
pub static SPRIM_INTEGRATOR: Lazy<Token> = Lazy::new(|| Token::new("integrator"));
/// Sample filter sprim type.
pub static SPRIM_SAMPLE_FILTER: Lazy<Token> = Lazy::new(|| Token::new("sampleFilter"));
/// Display filter sprim type.
pub static SPRIM_DISPLAY_FILTER: Lazy<Token> = Lazy::new(|| Token::new("displayFilter"));
/// Image shader sprim type.
pub static SPRIM_IMAGE_SHADER: Lazy<Token> = Lazy::new(|| Token::new("imageShader"));
/// Instance sprim type.
pub static SPRIM_INSTANCE: Lazy<Token> = Lazy::new(|| Token::new("instance"));
/// Task prim type (scene-index-only).
pub static TASK: Lazy<Token> = Lazy::new(|| Token::new("task"));

// Bprim type tokens
/// Render buffer primitive type.
pub static BPRIM_RENDER_BUFFER: Lazy<Token> = Lazy::new(|| Token::new("renderBuffer"));
/// Render settings bprim type.
pub static BPRIM_RENDER_SETTINGS: Lazy<Token> = Lazy::new(|| Token::new("renderSettings"));

// Model draw mode tokens
/// Inherited draw mode.
pub static MODEL_DRAWMODE_INHERITED: Lazy<Token> = Lazy::new(|| Token::new("inherited"));
/// Origin draw mode.
pub static MODEL_DRAWMODE_ORIGIN: Lazy<Token> = Lazy::new(|| Token::new("origin"));
/// Bounds draw mode.
pub static MODEL_DRAWMODE_BOUNDS: Lazy<Token> = Lazy::new(|| Token::new("bounds"));
/// Cards draw mode.
pub static MODEL_DRAWMODE_CARDS: Lazy<Token> = Lazy::new(|| Token::new("cards"));
/// Default draw mode.
pub static MODEL_DRAWMODE_DEFAULT: Lazy<Token> = Lazy::new(|| Token::new("default"));
/// Cross card geometry.
pub static MODEL_DRAWMODE_CROSS: Lazy<Token> = Lazy::new(|| Token::new("cross"));
/// Box card geometry.
pub static MODEL_DRAWMODE_BOX: Lazy<Token> = Lazy::new(|| Token::new("box"));
/// From texture card geometry.
pub static MODEL_DRAWMODE_FROM_TEXTURE: Lazy<Token> = Lazy::new(|| Token::new("fromTexture"));

// Option tokens
/// Parallel rprim sync option.
pub static OPTION_PARALLEL_RPRIM_SYNC: Lazy<Token> = Lazy::new(|| Token::new("parallelRprimSync"));

// Primvar role none (empty string)
/// No primvar role.
pub static PRIMVAR_ROLE_NONE: Lazy<Token> = Lazy::new(|| Token::default());

// AOV tokens
/// Color AOV.
pub static AOV_COLOR: Lazy<Token> = Lazy::new(|| Token::new("color"));
/// Depth AOV.
pub static AOV_DEPTH: Lazy<Token> = Lazy::new(|| Token::new("depth"));
/// Depth stencil AOV.
pub static AOV_DEPTH_STENCIL: Lazy<Token> = Lazy::new(|| Token::new("depthStencil"));
/// Camera depth AOV.
pub static AOV_CAMERA_DEPTH: Lazy<Token> = Lazy::new(|| Token::new("cameraDepth"));
/// Prim ID AOV.
pub static AOV_PRIM_ID: Lazy<Token> = Lazy::new(|| Token::new("primId"));
/// Instance ID AOV.
pub static AOV_INSTANCE_ID: Lazy<Token> = Lazy::new(|| Token::new("instanceId"));
/// Element ID AOV.
pub static AOV_ELEMENT_ID: Lazy<Token> = Lazy::new(|| Token::new("elementId"));
/// Edge ID AOV.
pub static AOV_EDGE_ID: Lazy<Token> = Lazy::new(|| Token::new("edgeId"));
/// Point ID AOV.
pub static AOV_POINT_ID: Lazy<Token> = Lazy::new(|| Token::new("pointId"));
/// Eye-space position AOV (Peye).
pub static AOV_PEYE: Lazy<Token> = Lazy::new(|| Token::new("Peye"));
/// Eye-space normal AOV (Neye).
pub static AOV_NEYE: Lazy<Token> = Lazy::new(|| Token::new("Neye"));
/// Patch coordinate AOV.
pub static AOV_PATCH_COORD: Lazy<Token> = Lazy::new(|| Token::new("patchCoord"));
/// Primitive parameter AOV.
pub static AOV_PRIMITIVE_PARAM: Lazy<Token> = Lazy::new(|| Token::new("primitiveParam"));
/// Normal AOV.
pub static AOV_NORMAL: Lazy<Token> = Lazy::new(|| Token::new("normal"));
/// Primvars prefix for AOV names.
pub static AOV_PRIMVARS_PREFIX: Lazy<Token> = Lazy::new(|| Token::new("primvars:"));
/// Light path expression prefix for AOV names.
pub static AOV_LPE_PREFIX: Lazy<Token> = Lazy::new(|| Token::new("lpe:"));
/// Shader prefix for AOV names.
pub static AOV_SHADER_PREFIX: Lazy<Token> = Lazy::new(|| Token::new("shader:"));

// Perf tokens (HD_PERF_TOKENS)
/// Adjacency buffer size.
pub static PERF_ADJACENCY_BUF_SIZE: Lazy<Token> = Lazy::new(|| Token::new("adjacencyBufSize"));
/// Basis curves topology.
pub static PERF_BASIS_CURVES_TOPOLOGY: Lazy<Token> =
    Lazy::new(|| Token::new("basisCurvesTopology"));
/// Buffer sources resolved.
pub static PERF_BUFFER_SOURCES_RESOLVED: Lazy<Token> =
    Lazy::new(|| Token::new("bufferSourcesResolved"));
/// Buffer array range migrated.
pub static PERF_BUFFER_ARRAY_RANGE_MIGRATED: Lazy<Token> =
    Lazy::new(|| Token::new("bufferArrayRangeMigrated"));
/// Buffer array range container resized.
pub static PERF_BUFFER_ARRAY_RANGE_CONTAINER_RESIZED: Lazy<Token> =
    Lazy::new(|| Token::new("bufferArrayRangeContainerResized"));
/// Committed.
pub static PERF_COMMITTED: Lazy<Token> = Lazy::new(|| Token::new("committed"));
/// Computations committed.
pub static PERF_COMPUTATIONS_COMMITTED: Lazy<Token> =
    Lazy::new(|| Token::new("computationsCommitted"));
/// Draw batches.
pub static PERF_DRAW_BATCHES: Lazy<Token> = Lazy::new(|| Token::new("drawBatches"));
/// Draw calls.
pub static PERF_DRAW_CALLS: Lazy<Token> = Lazy::new(|| Token::new("drawCalls"));
/// Dirty lists.
pub static PERF_DIRTY_LISTS: Lazy<Token> = Lazy::new(|| Token::new("dirtyLists"));
/// Dirty lists rebuilt.
pub static PERF_DIRTY_LISTS_REBUILT: Lazy<Token> = Lazy::new(|| Token::new("dirtyListsRebuilt"));
/// Garbage collected.
pub static PERF_GARBAGE_COLLECTED: Lazy<Token> = Lazy::new(|| Token::new("garbageCollected"));
/// Garbage collected SSBO.
pub static PERF_GARBAGE_COLLECTED_SSBO: Lazy<Token> =
    Lazy::new(|| Token::new("garbageCollectedSsbo"));
/// Garbage collected UBO.
pub static PERF_GARBAGE_COLLECTED_UBO: Lazy<Token> =
    Lazy::new(|| Token::new("garbageCollectedUbo"));
/// Garbage collected VBO.
pub static PERF_GARBAGE_COLLECTED_VBO: Lazy<Token> =
    Lazy::new(|| Token::new("garbageCollectedVbo"));
/// GPU memory used.
pub static PERF_GPU_MEMORY_USED: Lazy<Token> = Lazy::new(|| Token::new("gpuMemoryUsed"));
/// Inst basis curves topology.
pub static PERF_INST_BASIS_CURVES_TOPOLOGY: Lazy<Token> =
    Lazy::new(|| Token::new("instBasisCurvesTopology"));
/// Inst basis curves topology range.
pub static PERF_INST_BASIS_CURVES_TOPOLOGY_RANGE: Lazy<Token> =
    Lazy::new(|| Token::new("instBasisCurvesTopologyRange"));
/// Inst ext computation data range.
pub static PERF_INST_EXT_COMPUTATION_DATA_RANGE: Lazy<Token> =
    Lazy::new(|| Token::new("instExtComputationDataRange"));
/// Inst mesh topology.
pub static PERF_INST_MESH_TOPOLOGY: Lazy<Token> = Lazy::new(|| Token::new("instMeshTopology"));
/// Inst mesh topology range.
pub static PERF_INST_MESH_TOPOLOGY_RANGE: Lazy<Token> =
    Lazy::new(|| Token::new("instMeshTopologyRange"));
/// Inst primvar range.
pub static PERF_INST_PRIMVAR_RANGE: Lazy<Token> = Lazy::new(|| Token::new("instPrimvarRange"));
/// Inst vertex adjacency.
pub static PERF_INST_VERTEX_ADJACENCY: Lazy<Token> =
    Lazy::new(|| Token::new("instVertexAdjacency"));
/// Mesh topology.
pub static PERF_MESH_TOPOLOGY: Lazy<Token> = Lazy::new(|| Token::new("meshTopology"));
/// Non-uniform size.
pub static PERF_NON_UNIFORM_SIZE: Lazy<Token> = Lazy::new(|| Token::new("nonUniformSize"));
/// Num completed samples.
pub static PERF_NUM_COMPLETED_SAMPLES: Lazy<Token> =
    Lazy::new(|| Token::new("numCompletedSamples"));
/// Quadrangulate CPU.
pub static PERF_QUADRANGULATE_CPU: Lazy<Token> = Lazy::new(|| Token::new("quadrangulateCPU"));
/// Quadrangulate GPU.
pub static PERF_QUADRANGULATE_GPU: Lazy<Token> = Lazy::new(|| Token::new("quadrangulateGPU"));
/// Quadrangulate face varying.
pub static PERF_QUADRANGULATE_FACE_VARYING: Lazy<Token> =
    Lazy::new(|| Token::new("quadrangulateFaceVarying"));
/// Quadrangulated verts.
pub static PERF_QUADRANGULATED_VERTS: Lazy<Token> = Lazy::new(|| Token::new("quadrangulatedVerts"));
/// Rebuild batches.
pub static PERF_REBUILD_BATCHES: Lazy<Token> = Lazy::new(|| Token::new("rebuildBatches"));
/// Single buffer size.
pub static PERF_SINGLE_BUFFER_SIZE: Lazy<Token> = Lazy::new(|| Token::new("singleBufferSize"));
/// SSBO size.
pub static PERF_SSBO_SIZE: Lazy<Token> = Lazy::new(|| Token::new("ssboSize"));
/// Skip invisible rprim sync.
pub static PERF_SKIP_INVISIBLE_RPRIM_SYNC: Lazy<Token> =
    Lazy::new(|| Token::new("skipInvisibleRprimSync"));
/// Sources committed.
pub static PERF_SOURCES_COMMITTED: Lazy<Token> = Lazy::new(|| Token::new("sourcesCommitted"));
/// Subdivision refine CPU.
pub static PERF_SUBDIVISION_REFINE_CPU: Lazy<Token> =
    Lazy::new(|| Token::new("subdivisionRefineCPU"));
/// Subdivision refine GPU.
pub static PERF_SUBDIVISION_REFINE_GPU: Lazy<Token> =
    Lazy::new(|| Token::new("subdivisionRefineGPU"));
/// Texture memory.
pub static PERF_TEXTURE_MEMORY: Lazy<Token> = Lazy::new(|| Token::new("textureMemory"));
/// Triangulate face varying.
pub static PERF_TRIANGULATE_FACE_VARYING: Lazy<Token> =
    Lazy::new(|| Token::new("triangulateFaceVarying"));
/// UBO size.
pub static PERF_UBO_SIZE: Lazy<Token> = Lazy::new(|| Token::new("uboSize"));
/// VBO relocated.
pub static PERF_VBO_RELOCATED: Lazy<Token> = Lazy::new(|| Token::new("vboRelocated"));

// Shader tokens (HD_SHADER_TOKENS)
/// Alpha threshold.
pub static SHADER_ALPHA_THRESHOLD: Lazy<Token> = Lazy::new(|| Token::new("alphaThreshold"));
/// Clip planes.
pub static SHADER_CLIP_PLANES: Lazy<Token> = Lazy::new(|| Token::new("clipPlanes"));
/// Common shader source.
pub static SHADER_COMMON_SHADER_SOURCE: Lazy<Token> =
    Lazy::new(|| Token::new("commonShaderSource"));
/// Draw range.
pub static SHADER_DRAW_RANGE: Lazy<Token> = Lazy::new(|| Token::new("drawRange"));
/// Environment map.
pub static SHADER_ENVIRONMENT_MAP: Lazy<Token> = Lazy::new(|| Token::new("environmentMap"));
/// Linear exposure.
pub static SHADER_LINEAR_EXPOSURE: Lazy<Token> = Lazy::new(|| Token::new("linearExposure"));
/// Displacement shader.
pub static SHADER_DISPLACEMENT_SHADER: Lazy<Token> = Lazy::new(|| Token::new("displacementShader"));
/// Fragment shader.
pub static SHADER_FRAGMENT_SHADER: Lazy<Token> = Lazy::new(|| Token::new("fragmentShader"));
/// Geometry shader.
pub static SHADER_GEOMETRY_SHADER: Lazy<Token> = Lazy::new(|| Token::new("geometryShader"));
/// Image to world matrix.
pub static SHADER_IMAGE_TO_WORLD_MATRIX: Lazy<Token> =
    Lazy::new(|| Token::new("imageToWorldMatrix"));
/// Image to horizontally normalized filmback.
pub static SHADER_IMAGE_TO_HORIZONTALLY_NORMALIZED_FILMBACK: Lazy<Token> =
    Lazy::new(|| Token::new("imageToHorizontallyNormalizedFilmback"));
/// Indicator color.
pub static SHADER_INDICATOR_COLOR: Lazy<Token> = Lazy::new(|| Token::new("indicatorColor"));
/// Lighting blend amount.
pub static SHADER_LIGHTING_BLEND_AMOUNT: Lazy<Token> =
    Lazy::new(|| Token::new("lightingBlendAmount"));
/// Override color.
pub static SHADER_OVERRIDE_COLOR: Lazy<Token> = Lazy::new(|| Token::new("overrideColor"));
/// Mask color.
pub static SHADER_MASK_COLOR: Lazy<Token> = Lazy::new(|| Token::new("maskColor"));
/// Projection matrix.
pub static SHADER_PROJECTION_MATRIX: Lazy<Token> = Lazy::new(|| Token::new("projectionMatrix"));
/// Point color.
pub static SHADER_POINT_COLOR: Lazy<Token> = Lazy::new(|| Token::new("pointColor"));
/// Point size.
pub static SHADER_POINT_SIZE: Lazy<Token> = Lazy::new(|| Token::new("pointSize"));
/// Point selected size.
pub static SHADER_POINT_SELECTED_SIZE: Lazy<Token> = Lazy::new(|| Token::new("pointSelectedSize"));
/// Material tag.
pub static SHADER_MATERIAL_TAG: Lazy<Token> = Lazy::new(|| Token::new("materialTag"));
/// Num clip planes.
pub static SHADER_NUM_CLIP_PLANES: Lazy<Token> = Lazy::new(|| Token::new("numClipPlanes"));
/// Tess control shader.
pub static SHADER_TESS_CONTROL_SHADER: Lazy<Token> = Lazy::new(|| Token::new("tessControlShader"));
/// Tess eval shader.
pub static SHADER_TESS_EVAL_SHADER: Lazy<Token> = Lazy::new(|| Token::new("tessEvalShader"));
/// Post tess control shader.
pub static SHADER_POST_TESS_CONTROL_SHADER: Lazy<Token> =
    Lazy::new(|| Token::new("postTessControlShader"));
/// Post tess vertex shader.
pub static SHADER_POST_TESS_VERTEX_SHADER: Lazy<Token> =
    Lazy::new(|| Token::new("postTessVertexShader"));
/// Tess level.
pub static SHADER_TESS_LEVEL: Lazy<Token> = Lazy::new(|| Token::new("tessLevel"));
/// Viewport.
pub static SHADER_VIEWPORT: Lazy<Token> = Lazy::new(|| Token::new("viewport"));
/// Vertex shader.
pub static SHADER_VERTEX_SHADER: Lazy<Token> = Lazy::new(|| Token::new("vertexShader"));
/// Wireframe color.
pub static SHADER_WIREFRAME_COLOR: Lazy<Token> = Lazy::new(|| Token::new("wireframeColor"));
/// World to view matrix.
pub static SHADER_WORLD_TO_VIEW_MATRIX: Lazy<Token> = Lazy::new(|| Token::new("worldToViewMatrix"));
/// World to view inverse matrix.
pub static SHADER_WORLD_TO_VIEW_INVERSE_MATRIX: Lazy<Token> =
    Lazy::new(|| Token::new("worldToViewInverseMatrix"));
/// Step size.
pub static SHADER_STEP_SIZE: Lazy<Token> = Lazy::new(|| Token::new("stepSize"));
/// Step size lighting.
pub static SHADER_STEP_SIZE_LIGHTING: Lazy<Token> = Lazy::new(|| Token::new("stepSizeLighting"));
/// Multisample count.
pub static SHADER_MULTISAMPLE_COUNT: Lazy<Token> = Lazy::new(|| Token::new("multisampleCount"));

// Render settings tokens
/// Enable shadows.
pub static RENDER_SETTINGS_ENABLE_SHADOWS: Lazy<Token> = Lazy::new(|| Token::new("enableShadows"));
/// Enable scene materials.
pub static RENDER_SETTINGS_ENABLE_SCENE_MATERIALS: Lazy<Token> =
    Lazy::new(|| Token::new("enableSceneMaterials"));
/// Enable scene lights.
pub static RENDER_SETTINGS_ENABLE_SCENE_LIGHTS: Lazy<Token> =
    Lazy::new(|| Token::new("enableSceneLights"));
/// Enable exposure compensation.
pub static RENDER_SETTINGS_ENABLE_EXPOSURE_COMPENSATION: Lazy<Token> =
    Lazy::new(|| Token::new("enableExposureCompensation"));
/// Dome light camera visibility.
pub static RENDER_SETTINGS_DOME_LIGHT_CAMERA_VISIBILITY: Lazy<Token> =
    Lazy::new(|| Token::new("domeLightCameraVisibility"));
/// Converged variance.
pub static RENDER_SETTINGS_CONVERGED_VARIANCE: Lazy<Token> =
    Lazy::new(|| Token::new("convergedVariance"));
/// Converged samples per pixel.
pub static RENDER_SETTINGS_CONVERGED_SAMPLES_PER_PIXEL: Lazy<Token> =
    Lazy::new(|| Token::new("convergedSamplesPerPixel"));
/// Thread limit.
pub static RENDER_SETTINGS_THREAD_LIMIT: Lazy<Token> = Lazy::new(|| Token::new("threadLimit"));
/// Enable interactive.
pub static RENDER_SETTINGS_ENABLE_INTERACTIVE: Lazy<Token> =
    Lazy::new(|| Token::new("enableInteractive"));
/// Renderer create args.
pub static RENDER_SETTINGS_RENDERER_CREATE_ARGS: Lazy<Token> =
    Lazy::new(|| Token::new("rendererCreateArgs"));

// Render settings prim tokens
/// Active render settings.
pub static RENDER_SETTINGS_PRIM_ACTIVE: Lazy<Token> = Lazy::new(|| Token::new("active"));
/// Namespaced settings.
pub static RENDER_SETTINGS_PRIM_NAMESPACED: Lazy<Token> =
    Lazy::new(|| Token::new("namespacedSettings"));
/// Render products.
pub static RENDER_SETTINGS_PRIM_RENDER_PRODUCTS: Lazy<Token> =
    Lazy::new(|| Token::new("renderProducts"));
/// Included purposes.
pub static RENDER_SETTINGS_PRIM_INCLUDED_PURPOSES: Lazy<Token> =
    Lazy::new(|| Token::new("includedPurposes"));
/// Material binding purposes.
pub static RENDER_SETTINGS_PRIM_MATERIAL_BINDING_PURPOSES: Lazy<Token> =
    Lazy::new(|| Token::new("materialBindingPurposes"));
/// Rendering color space.
pub static RENDER_SETTINGS_PRIM_RENDERING_COLOR_SPACE: Lazy<Token> =
    Lazy::new(|| Token::new("renderingColorSpace"));
/// Shutter interval.
pub static RENDER_SETTINGS_PRIM_SHUTTER_INTERVAL: Lazy<Token> =
    Lazy::new(|| Token::new("shutterInterval"));

// Aspect ratio conform policy tokens
/// Adjust aperture width.
pub static ASPECT_RATIO_ADJUST_APERTURE_WIDTH: Lazy<Token> =
    Lazy::new(|| Token::new("adjustApertureWidth"));
/// Adjust aperture height.
pub static ASPECT_RATIO_ADJUST_APERTURE_HEIGHT: Lazy<Token> =
    Lazy::new(|| Token::new("adjustApertureHeight"));
/// Expand aperture.
pub static ASPECT_RATIO_EXPAND_APERTURE: Lazy<Token> = Lazy::new(|| Token::new("expandAperture"));
/// Crop aperture.
pub static ASPECT_RATIO_CROP_APERTURE: Lazy<Token> = Lazy::new(|| Token::new("cropAperture"));
/// Adjust pixel aspect ratio.
pub static ASPECT_RATIO_ADJUST_PIXEL_ASPECT_RATIO: Lazy<Token> =
    Lazy::new(|| Token::new("adjustPixelAspectRatio"));

// Collection emulation tokens (light linking)
/// Light link collection name.
pub static COLLECTION_LIGHT_LINK: Lazy<Token> = Lazy::new(|| Token::new("lightLink"));
/// Shadow link collection name.
pub static COLLECTION_SHADOW_LINK: Lazy<Token> = Lazy::new(|| Token::new("shadowLink"));
/// Filter link collection name.
pub static COLLECTION_FILTER_LINK: Lazy<Token> = Lazy::new(|| Token::new("filterLink"));
/// Light link collection membership expression.
pub static COLLECTION_LIGHT_LINK_MEMBERSHIP_EXPRESSION: Lazy<Token> =
    Lazy::new(|| Token::new("lightLinkCollectionMembershipExpression"));
/// Shadow link collection membership expression.
pub static COLLECTION_SHADOW_LINK_MEMBERSHIP_EXPRESSION: Lazy<Token> =
    Lazy::new(|| Token::new("shadowLinkCollectionMembershipExpression"));
/// Filter link collection membership expression.
pub static COLLECTION_FILTER_LINK_MEMBERSHIP_EXPRESSION: Lazy<Token> =
    Lazy::new(|| Token::new("filterLinkCollectionMembershipExpression"));

// Skinning input tokens
/// Skinning transforms.
pub static SKINNING_SKINNING_XFORMS: Lazy<Token> = Lazy::new(|| Token::new("hydra:skinningXforms"));
/// Skinning dual quaternions.
pub static SKINNING_SKINNING_DUAL_QUATS: Lazy<Token> =
    Lazy::new(|| Token::new("hydra:skinningDualQuats"));
/// Skinning scale transforms.
pub static SKINNING_SKINNING_SCALE_XFORMS: Lazy<Token> =
    Lazy::new(|| Token::new("hydra:skinningScaleXforms"));
/// Blend shape weights.
pub static SKINNING_BLEND_SHAPE_WEIGHTS: Lazy<Token> =
    Lazy::new(|| Token::new("hydra:blendShapeWeights"));
/// Skel local to common space.
pub static SKINNING_SKEL_LOCAL_TO_COMMON_SPACE: Lazy<Token> =
    Lazy::new(|| Token::new("hydra:skelLocalToWorld"));
/// Common space to prim local.
pub static SKINNING_COMMON_SPACE_TO_PRIM_LOCAL: Lazy<Token> =
    Lazy::new(|| Token::new("hydra:primWorldToLocal"));
/// Blend shape offsets.
pub static SKINNING_BLEND_SHAPE_OFFSETS: Lazy<Token> =
    Lazy::new(|| Token::new("hydra:blendShapeOffsets"));
/// Blend shape offset ranges.
pub static SKINNING_BLEND_SHAPE_OFFSET_RANGES: Lazy<Token> =
    Lazy::new(|| Token::new("hydra:blendShapeOffsetRanges"));
/// Num blend shape offset ranges.
pub static SKINNING_NUM_BLEND_SHAPE_OFFSET_RANGES: Lazy<Token> =
    Lazy::new(|| Token::new("hydra:numBlendShapeOffsetRanges"));
/// Has constant influences.
pub static SKINNING_HAS_CONSTANT_INFLUENCES: Lazy<Token> =
    Lazy::new(|| Token::new("hydra:hasConstantInfluences"));
/// Num influences per component.
pub static SKINNING_NUM_INFLUENCES_PER_COMPONENT: Lazy<Token> =
    Lazy::new(|| Token::new("hydra:numInfluencesPerComponent"));
/// Influences.
pub static SKINNING_INFLUENCES: Lazy<Token> = Lazy::new(|| Token::new("hydra:influences"));
/// Num skinning method.
pub static SKINNING_NUM_SKINNING_METHOD: Lazy<Token> =
    Lazy::new(|| Token::new("hydra:numSkinningMethod"));
/// Num joints.
pub static SKINNING_NUM_JOINTS: Lazy<Token> = Lazy::new(|| Token::new("hydra:numJoints"));
/// Num blend shape weights.
pub static SKINNING_NUM_BLEND_SHAPE_WEIGHTS: Lazy<Token> =
    Lazy::new(|| Token::new("hydra:numBlendShapeWeights"));

// Skinning skel input tokens
/// Geom bind transform.
pub static SKINNING_GEOM_BIND_TRANSFORM: Lazy<Token> =
    Lazy::new(|| Token::new("skel:geomBindTransform"));

// Resource type tokens
/// Texture resource type.
pub static RESOURCE_TYPE_TEXTURE: Lazy<Token> = Lazy::new(|| Token::new("texture"));
/// Shader file resource type.
pub static RESOURCE_TYPE_SHADER_FILE: Lazy<Token> = Lazy::new(|| Token::new("shaderFile"));

// Material tag tokens
/// Default material tag.
pub static MATERIAL_TAG_DEFAULT: Lazy<Token> = Lazy::new(|| Token::new("defaultMaterialTag"));

// Render context tokens
/// Universal render context (empty string).
pub static RENDER_CONTEXT_UNIVERSAL: Lazy<Token> = Lazy::new(|| Token::default());

// Scene index emulation
/// Scene delegate emulation.
pub static SCENE_INDEX_EMULATION_DELEGATE: Lazy<Token> = Lazy::new(|| Token::new("sceneDelegate"));

// Material terminal tokens
/// Surface material terminal.
pub static MATERIAL_SURFACE: Lazy<Token> = Lazy::new(|| Token::new("surface"));
/// Displacement material terminal.
pub static MATERIAL_DISPLACEMENT: Lazy<Token> = Lazy::new(|| Token::new("displacement"));
/// Volume material terminal.
pub static MATERIAL_VOLUME: Lazy<Token> = Lazy::new(|| Token::new("volume"));
/// Light shader material terminal.
pub static MATERIAL_LIGHT: Lazy<Token> = Lazy::new(|| Token::new("light"));
/// Light filter material terminal.
pub static MATERIAL_LIGHT_FILTER: Lazy<Token> = Lazy::new(|| Token::new("lightFilter"));
/// Image shader material terminal.
pub static MATERIAL_IMAGE_SHADER: Lazy<Token> = Lazy::new(|| Token::new("imageShader"));

// Render tag tokens
/// Standard geometry render tag.
pub static RENDER_TAG_GEOMETRY: Lazy<Token> = Lazy::new(|| Token::new("geometry"));
/// Guide/manipulator render tag.
pub static RENDER_TAG_GUIDE: Lazy<Token> = Lazy::new(|| Token::new("guide"));
/// Hidden geometry render tag.
pub static RENDER_TAG_HIDDEN: Lazy<Token> = Lazy::new(|| Token::new("hidden"));
/// Proxy geometry render tag.
pub static RENDER_TAG_PROXY: Lazy<Token> = Lazy::new(|| Token::new("proxy"));
/// Final render geometry tag.
pub static RENDER_TAG_RENDER: Lazy<Token> = Lazy::new(|| Token::new("render"));

// Primvar role tokens
/// Point position primvar role.
pub static PRIMVAR_ROLE_POINT: Lazy<Token> = Lazy::new(|| Token::new("point"));
/// Normal vector primvar role.
pub static PRIMVAR_ROLE_NORMAL: Lazy<Token> = Lazy::new(|| Token::new("normal"));
/// Generic vector primvar role.
pub static PRIMVAR_ROLE_VECTOR: Lazy<Token> = Lazy::new(|| Token::new("vector"));
/// Color primvar role.
pub static PRIMVAR_ROLE_COLOR: Lazy<Token> = Lazy::new(|| Token::new("color"));
/// Texture coordinate primvar role.
pub static PRIMVAR_ROLE_TEXTURE_COORDINATE: Lazy<Token> =
    Lazy::new(|| Token::new("textureCoordinate"));

// Prim type helper functions (correspond to HdPrimTypeIsGprim, etc. in tokens.cpp)

/// Returns true if the prim type is a gprim (renderable geometry).
///
/// Gprims: mesh, basisCurves, points, volume.
pub fn hd_prim_type_is_gprim(prim_type: &Token) -> bool {
    let s = prim_type.as_str();
    s == "mesh" || s == "basisCurves" || s == "points" || s == "volume"
}

/// Returns true if the prim type is a light (HD_LIGHT_TYPE_TOKENS only, not lightFilter).
pub fn hd_prim_type_is_light(prim_type: &Token) -> bool {
    let s = prim_type.as_str();
    matches!(
        s,
        "cylinderLight"
            | "diskLight"
            | "distantLight"
            | "domeLight"
            | "light"
            | "meshLight"
            | "pluginLight"
            | "rectLight"
            | "simpleLight"
            | "sphereLight"
    )
}

/// Returns true if the prim type supports geom subsets.
///
/// Supported: mesh, basisCurves (tetMesh not yet in C++).
pub fn hd_prim_type_supports_geom_subsets(prim_type: &Token) -> bool {
    let s = prim_type.as_str();
    s == "mesh" || s == "basisCurves"
}

/// Light prim type tokens as a vector (for iteration).
///
/// Corresponds to C++ `HdLightPrimTypeTokens()`.
pub fn hd_light_prim_type_tokens() -> Vec<Token> {
    vec![
        LIGHT_CYLINDER.clone(),
        LIGHT_DISK.clone(),
        LIGHT_DISTANT.clone(),
        LIGHT_DOME.clone(),
        LIGHT.clone(),
        LIGHT_MESH.clone(),
        LIGHT_PLUGIN.clone(),
        LIGHT_RECT.clone(),
        LIGHT_SIMPLE.clone(),
        LIGHT_SPHERE.clone(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_creation() {
        assert_eq!(POINTS.as_str(), "points");
        assert_eq!(NORMALS.as_str(), "normals");
        assert_eq!(TRANSFORM.as_str(), "transform");
    }

    #[test]
    fn test_rprim_tokens() {
        assert_eq!(RPRIM_MESH.as_str(), "mesh");
        assert_eq!(RPRIM_CUBE.as_str(), "cube");
        assert_eq!(RPRIM_SPHERE.as_str(), "sphere");
    }

    #[test]
    fn test_light_tokens() {
        assert_eq!(LIGHT_DOME.as_str(), "domeLight");
        assert_eq!(LIGHT_RECT.as_str(), "rectLight");
    }

    #[test]
    fn test_repr_tokens() {
        assert_eq!(REPR_HULL.as_str(), "hull");
        assert_eq!(REPR_REFINED.as_str(), "refined");
        assert_eq!(REPR_WIRE.as_str(), "wire");
    }

    #[test]
    fn test_instancer_tokens() {
        assert_eq!(INSTANCE_TRANSFORMS.as_str(), "hydra:instanceTransforms");
        assert_eq!(INSTANCE_ROTATIONS.as_str(), "hydra:instanceRotations");
    }

    #[test]
    fn test_material_tokens() {
        assert_eq!(MATERIAL_SURFACE.as_str(), "surface");
        assert_eq!(MATERIAL_DISPLACEMENT.as_str(), "displacement");
    }
}
