//! RendererServices — abstract interface for renderer callbacks.
//!
//! This trait mirrors the C++ `OSL::RendererServices` class. The renderer
//! implements these methods to provide OSL with scene data and capabilities
//! such as coordinate transformations, texture lookups, attribute queries,
//! ray tracing, and point cloud access.

use std::ffi::c_void;

use crate::Float;
use crate::context::ShadingContext;
use crate::encodedtypes::EncodedType;
use crate::math::{Matrix44, Vec3};
use crate::shaderglobals::ShaderGlobals;
use crate::typedesc::{TypeDesc, VecSemantics};
use crate::ustring::{UString, UStringHash};

// ---------------------------------------------------------------------------
// Attribute getter specs — Rust equivalents of C++ AttributeGetterSpec /
// InterpolatedGetterSpec (FunctionSpec<...>).  These are opaque "compiled
// getter" descriptors that a renderer fills in during shader optimisation.
// An empty `fn_name` means "attribute not available at compile time".
// ---------------------------------------------------------------------------

/// Spec for a compiled attribute getter, mirrors C++ `AttributeGetterSpec`.
/// The renderer fills this in during `build_attribute_getter`.
#[derive(Debug, Clone, Default)]
pub struct AttributeGetterSpec {
    /// Mangled function name that the JIT should call.  Empty = not available.
    pub fn_name: String,
}

/// Spec for a compiled interpolated getter, mirrors C++ `InterpolatedGetterSpec`.
/// The renderer fills this in during `build_interpolated_getter`.
#[derive(Debug, Clone, Default)]
pub struct InterpolatedGetterSpec {
    /// Mangled function name that the JIT should call.  Empty = not available.
    pub fn_name: String,
}

/// Opaque texture system handle — renderers that wrap OIIO can return a real
/// pointer; the default is `None` (no texture system available).
pub struct TextureSystem; // opaque placeholder; renderers may define their own

/// Opaque handle to a texture managed by the renderer's texture system.
pub type TextureHandle = *mut c_void;

/// Per-thread opaque data for the texture system.
pub type TexturePerthread = *mut c_void;

/// Options for trace() calls, binary-compatible with `OSL::TraceOpt`.
#[repr(C)]
#[derive(Debug, Clone)]
pub struct TraceOpt {
    pub mindist: Float,
    pub maxdist: Float,
    pub shade: bool,
    pub traceset: UStringHash,
}

impl Default for TraceOpt {
    fn default() -> Self {
        Self {
            mindist: 0.0,
            maxdist: 1.0e30,
            shade: false,
            traceset: UStringHash::EMPTY,
        }
    }
}

/// Options for noise calls, mirroring `RendererServices::NoiseOpt`.
#[repr(C)]
#[derive(Debug, Clone)]
pub struct NoiseOpt {
    pub anisotropic: i32,
    pub do_filter: i32,
    pub direction: Vec3,
    pub bandwidth: Float,
    pub impulses: Float,
}

impl Default for NoiseOpt {
    fn default() -> Self {
        Self {
            anisotropic: 0,
            do_filter: 1,
            direction: Vec3::new(1.0, 0.0, 0.0),
            bandwidth: 1.0,
            impulses: 16.0,
        }
    }
}

/// Opaque pointer to whatever the renderer uses to represent a
/// (potentially motion-blurred) coordinate transformation.
pub type TransformationPtr = *const c_void;

/// The abstract renderer interface that OSL calls back into.
///
/// All methods have default implementations that return `false` / no-op.
/// A renderer overrides the methods it supports.
#[allow(unused_variables)]
pub trait RendererServices: Send + Sync {
    /// Return whether this renderer supports a named feature.
    /// Known features: "OptiX", "build_attribute_getter",
    /// "build_interpolated_getter".
    fn supports(&self, feature: &str) -> bool {
        false
    }

    // -- Coordinate transformations ----------------------------------------

    /// Get the 4×4 matrix for a transformation at a given time.
    fn get_matrix_xform(
        &self,
        sg: &ShaderGlobals,
        xform: TransformationPtr,
        time: Float,
    ) -> Option<Matrix44> {
        None
    }

    /// Get the inverse 4×4 matrix for a transformation at a given time.
    /// Default: calls `get_matrix_xform` and inverts.
    fn get_inverse_matrix_xform(
        &self,
        sg: &ShaderGlobals,
        xform: TransformationPtr,
        time: Float,
    ) -> Option<Matrix44> {
        self.get_matrix_xform(sg, xform, time)
            .and_then(|m| m.inverse())
    }

    /// Get the 4x4 matrix for a static (non-time-varying) transformation.
    /// Return None if time-varying or unknown.
    /// Matches C++ `get_matrix(sg, result, xform)` (no time param).
    fn get_matrix_xform_static(
        &self,
        sg: &ShaderGlobals,
        xform: TransformationPtr,
    ) -> Option<Matrix44> {
        None
    }

    /// Get the inverse of a static transformation.
    /// Default: calls `get_matrix_xform_static` and inverts.
    fn get_inverse_matrix_xform_static(
        &self,
        sg: &ShaderGlobals,
        xform: TransformationPtr,
    ) -> Option<Matrix44> {
        self.get_matrix_xform_static(sg, xform)
            .and_then(|m| m.inverse())
    }

    /// Get the 4×4 matrix that transforms points from the named
    /// 'from' coordinate system to "common" space at the given time.
    fn get_matrix_named(
        &self,
        sg: &ShaderGlobals,
        from: UStringHash,
        time: Float,
    ) -> Option<Matrix44> {
        None
    }

    /// Get the inverse matrix from "common" space to the named space.
    fn get_inverse_matrix_named(
        &self,
        sg: &ShaderGlobals,
        to: UStringHash,
        time: Float,
    ) -> Option<Matrix44> {
        self.get_matrix_named(sg, to, time)
            .and_then(|m| m.inverse())
    }

    /// Get the matrix from named space to "common", no time (static).
    fn get_matrix_named_static(&self, sg: &ShaderGlobals, from: UStringHash) -> Option<Matrix44> {
        None
    }

    /// Get the inverse static matrix.
    fn get_inverse_matrix_named_static(
        &self,
        sg: &ShaderGlobals,
        to: UStringHash,
    ) -> Option<Matrix44> {
        self.get_matrix_named_static(sg, to)
            .and_then(|m| m.inverse())
    }

    /// Transform points between named coordinate systems.
    /// Return `true` if a nonlinear transformation was applied.
    fn transform_points(
        &self,
        sg: &ShaderGlobals,
        from: UStringHash,
        to: UStringHash,
        time: Float,
        pin: &[Vec3],
        pout: &mut [Vec3],
        vectype: VecSemantics,
    ) -> bool {
        false
    }

    // -- Attributes --------------------------------------------------------

    /// Get a named attribute value.
    ///
    /// Returns `Some(AttributeData)` if the attribute was found, `None`
    /// otherwise. This is the safe version of the C++ API which wrote
    /// through a `void*` — we return the value directly instead.
    fn get_attribute(
        &self,
        _sg: &ShaderGlobals,
        _derivatives: bool,
        _object: UStringHash,
        _type_desc: TypeDesc,
        _name: UStringHash,
    ) -> Option<AttributeData> {
        None
    }

    /// Get an element of a named attribute array.
    fn get_array_attribute(
        &self,
        _sg: &ShaderGlobals,
        _derivatives: bool,
        _object: UStringHash,
        _type_desc: TypeDesc,
        _name: UStringHash,
        _index: i32,
    ) -> Option<AttributeData> {
        None
    }

    /// Get a trace/ray-hit attribute value by name.
    /// Used when source is "trace" in getattribute.
    fn get_trace_value(&self, _sg: &ShaderGlobals, _name: UStringHash) -> Option<AttributeData> {
        None
    }

    // -- User data ---------------------------------------------------------

    /// Get named user-data from the current object.
    fn get_userdata(
        &self,
        _derivatives: bool,
        _name: UStringHash,
        _type_desc: TypeDesc,
        _sg: &ShaderGlobals,
    ) -> Option<AttributeData> {
        None
    }

    // -- Textures ----------------------------------------------------------

    /// Get a texture handle from a filename.
    ///
    /// Matches C++ `RendererServices::get_texture_handle(ustring filename, ShadingContext* ctx)`.
    /// `ctx` is optional; renderers that don't use it can ignore it.
    /// Default delegates to `get_texture_handle_simple` for backward compatibility.
    fn get_texture_handle(
        &self,
        filename: UStringHash,
        _ctx: Option<&ShadingContext>,
    ) -> TextureHandle {
        self.get_texture_handle_simple(filename)
    }

    /// Simplified texture handle lookup without a shading context.
    /// Override this if you don't need the context parameter.
    fn get_texture_handle_simple(&self, _filename: UStringHash) -> TextureHandle {
        std::ptr::null_mut()
    }

    /// Check if a texture handle is valid.
    fn texture_handle_is_valid(&self, handle: TextureHandle) -> bool {
        !handle.is_null()
    }

    /// Check if a texture handle is UDIM.
    fn is_udim(&self, handle: TextureHandle) -> bool {
        false
    }

    /// 2D filtered texture lookup.
    /// Matches C++ RendererServices::texture(..., TextureOpt& options, ...).
    fn texture(
        &self,
        filename: UStringHash,
        handle: TextureHandle,
        sg: &ShaderGlobals,
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
        let _ = (
            filename, handle, sg, opt, s, t, dsdx, dtdx, dsdy, dtdy, nchannels, result, dresultds,
            dresultdt,
        );
        Err("texture not implemented".into())
    }

    /// 3D filtered texture lookup.
    /// Matches C++ RendererServices::texture3d(..., TextureOpt& options, ...).
    fn texture3d(
        &self,
        filename: UStringHash,
        handle: TextureHandle,
        sg: &ShaderGlobals,
        opt: &crate::texture::TextureOpt,
        p: &Vec3,
        dpdx: &Vec3,
        dpdy: &Vec3,
        dpdz: &Vec3,
        nchannels: i32,
        result: &mut [Float],
        dresultds: Option<&mut [Float]>,
        dresultdt: Option<&mut [Float]>,
        dresultdr: Option<&mut [Float]>,
    ) -> Result<(), String> {
        let _ = (
            filename, handle, sg, opt, p, dpdx, dpdy, dpdz, nchannels, result, dresultds,
            dresultdt, dresultdr,
        );
        Err("texture3d not implemented".into())
    }

    /// Environment map lookup.
    /// Matches C++ RendererServices::environment(..., TextureOpt& options, ...).
    fn environment(
        &self,
        filename: UStringHash,
        handle: TextureHandle,
        sg: &ShaderGlobals,
        opt: &crate::texture::TextureOpt,
        r: &Vec3,
        drdx: &Vec3,
        drdy: &Vec3,
        nchannels: i32,
        result: &mut [Float],
        dresultds: Option<&mut [Float]>,
        dresultdt: Option<&mut [Float]>,
    ) -> Result<(), String> {
        let _ = (
            filename, handle, sg, opt, r, drdx, drdy, nchannels, result, dresultds, dresultdt,
        );
        Err("environment not implemented".into())
    }

    /// Get texture metadata / info.
    fn get_texture_info(
        &self,
        filename: UStringHash,
        handle: TextureHandle,
        sg: &ShaderGlobals,
        subimage: i32,
        dataname: UStringHash,
        datatype: TypeDesc,
        data: *mut c_void,
    ) -> Result<(), String> {
        Err("get_texture_info not implemented".into())
    }

    // -- Point cloud -------------------------------------------------------

    /// Search for nearest points in a point cloud.
    ///
    /// When `derivs_offset > 0`, the caller must provide `dcenter_dx` and `dcenter_dy`
    /// (screen-space derivatives of center). The out_distances layout becomes:
    /// `[dist0..distN, d_distance_dx[0]..[N], d_distance_dy[0]..[N]]`, so the buffer
    /// must have at least `max_points * (1 + 2 * derivs_offset)` elements when derivs_offset>=1.
    /// (derivs_offset is the stride: distance block size = derivs_offset when 1, meaning 3 blocks.)
    fn pointcloud_search(
        &self,
        sg: &ShaderGlobals,
        filename: UStringHash,
        center: &Vec3,
        radius: Float,
        max_points: i32,
        sort: bool,
        out_indices: &mut [i32],
        out_distances: Option<&mut [Float]>,
        derivs_offset: i32,
        _dcenter_dx: Option<&Vec3>,
        _dcenter_dy: Option<&Vec3>,
    ) -> i32 {
        0
    }

    /// Retrieve attributes for a set of point cloud indices.
    fn pointcloud_get(
        &self,
        sg: &ShaderGlobals,
        filename: UStringHash,
        indices: &[i32],
        attr_name: UStringHash,
        attr_type: TypeDesc,
        out_data: *mut c_void,
    ) -> bool {
        false
    }

    /// Write a point to a named point cloud.
    fn pointcloud_write(
        &self,
        sg: &ShaderGlobals,
        filename: UStringHash,
        pos: &Vec3,
        attrib_names: &[UStringHash],
        attrib_types: &[TypeDesc],
        attrib_data: &[*const c_void],
    ) -> bool {
        false
    }

    // -- Ray tracing -------------------------------------------------------

    /// Trace a ray. Returns `true` if anything was hit.
    fn trace(
        &self,
        options: &mut TraceOpt,
        sg: &ShaderGlobals,
        p: &Vec3,
        dpdx: &Vec3,
        dpdy: &Vec3,
        r: &Vec3,
        drdx: &Vec3,
        drdy: &Vec3,
    ) -> bool {
        false
    }

    // -- Messages ----------------------------------------------------------

    /// Get a named message from the renderer (for "sourced" messages).
    fn getmessage(
        &self,
        sg: &ShaderGlobals,
        source: UStringHash,
        name: UStringHash,
        type_desc: TypeDesc,
        val: *mut c_void,
        derivatives: bool,
    ) -> bool {
        false
    }

    // -- Device memory (GPU) -----------------------------------------------

    /// Allocate memory on the shader execution device.
    fn device_alloc(&self, size: usize) -> *mut c_void {
        std::ptr::null_mut()
    }

    /// Free device memory.
    fn device_free(&self, ptr: *mut c_void) {}

    /// Copy bytes from host to device.
    fn copy_to_device(
        &self,
        dst_device: *mut c_void,
        src_host: *const c_void,
        size: usize,
    ) -> *mut c_void {
        std::ptr::null_mut()
    }

    // -- Closure registration callbacks ------------------------------------

    /// Prepare closure data before execution.
    fn prepare_closure(&self, id: i32, data: *mut c_void) {}

    /// Set up closure data.
    fn setup_closure(&self, id: i32, data: *mut c_void) {}

    // -- Attribute getter builders (for optimization) ---------------------

    /// Build a compiled attribute getter for a named attribute.
    ///
    /// Called at shader *compile* time (not execution time). The renderer
    /// fills `spec` with a function name the JIT can call directly. Leave
    /// `spec.fn_name` empty to signal that the attribute is not available.
    ///
    /// Matches C++ `RendererServices::build_attribute_getter`:
    ///   `void build_attribute_getter(group, is_object_lookup, object_name,
    ///       attribute_name, is_array_lookup, array_index, type, derivatives, spec)`
    ///
    /// * `is_object_lookup` — true when an object name was specified, even
    ///   if its value is not known at compile time.
    /// * `object_name`  — `Some` when the object name is known at compile
    ///   time, `None` otherwise.
    /// * `attribute_name` — `Some` when the attribute name is known at
    ///   compile time, `None` otherwise.
    /// * `is_array_lookup` — true when an array index is provided.
    /// * `array_index`  — `Some` when the array index is known at compile
    ///   time, `None` otherwise.
    fn build_attribute_getter(
        &self,
        _group: *const c_void,
        _is_object_lookup: bool,
        _object_name: Option<&UString>,
        _attribute_name: Option<&UString>,
        _is_array_lookup: bool,
        _array_index: Option<i32>,
        _type_desc: TypeDesc,
        _derivatives: bool,
        _spec: &mut AttributeGetterSpec,
    ) {
        // Default: leave spec.fn_name empty (attribute not specialisable).
    }

    /// Build a compiled interpolated getter for a named user-data parameter.
    ///
    /// Called at shader *compile* time. Fill `spec.fn_name` with a callable
    /// function; leave empty when not available.
    ///
    /// Matches C++ `RendererServices::build_interpolated_getter`:
    ///   `void build_interpolated_getter(group, param_name, type, derivatives, spec)`
    fn build_interpolated_getter(
        &self,
        _group: *const c_void,
        _param_name: &UString,
        _type_desc: TypeDesc,
        _derivatives: bool,
        _spec: &mut InterpolatedGetterSpec,
    ) {
        // Default: leave spec.fn_name empty.
    }

    // -- Texture system access ---------------------------------------------

    /// Return a reference to the renderer's texture system, if any.
    ///
    /// Matches C++ `RendererServices::texturesys() const`.
    /// Default returns `None`; renderers wrapping OIIO override this.
    fn texturesys(&self) -> Option<&TextureSystem> {
        None
    }

    // -- Batched rendering support -----------------------------------------

    /// Return true if this renderer supports batched (SIMD) shader execution.
    ///
    /// Minimal placeholder matching the intent of C++ `batched(WidthOf<N>)`.
    /// Renderers that implement `BatchedRendererServices` should override this
    /// and return `true`.
    fn supports_batched(&self) -> bool {
        false
    }

    // -- Error / warning / print reporting --------------------------------

    /// Report an error message from within a shader (pre-decoded string).
    /// Matches C++ `RendererServices::errorfmt` (decoded path).
    fn errorfmt(&self, sg: &ShaderGlobals, msg: &str) {
        eprintln!("[OSL error] {}", msg);
    }

    /// Report a warning message from within a shader (pre-decoded string).
    /// Matches C++ `RendererServices::warningfmt` (decoded path).
    fn warningfmt(&self, sg: &ShaderGlobals, msg: &str) {
        eprintln!("[OSL warning] {}", msg);
    }

    /// Print output from within a shader (pre-decoded string).
    /// Matches C++ `RendererServices::printfmt` (decoded path).
    fn printfmt(&self, sg: &ShaderGlobals, msg: &str) {
        print!("{}", msg);
    }

    /// Print to a named file from within a shader (pre-decoded string).
    /// Matches C++ `RendererServices::filefmt` (decoded path).
    fn filefmt(&self, sg: &ShaderGlobals, filename: &str, msg: &str) {}

    /// Report an error with raw encoded-arg format (JIT path).
    /// Default decodes and delegates to `errorfmt`.
    /// Matches C++ `RendererServices::errorfmt(sg, fmt_spec, arg_count, arg_types, arg_values_size, arg_values)`.
    fn errorfmt_encoded(
        &self,
        sg: &ShaderGlobals,
        fmt_hash: UStringHash,
        arg_count: i32,
        arg_types: &[EncodedType],
        arg_values: &[u8],
    ) {
        let msg =
            crate::encodedtypes::decode_message(fmt_hash.hash(), arg_count, arg_types, arg_values);
        self.errorfmt(sg, &msg);
    }

    /// Report a warning with raw encoded-arg format (JIT path).
    /// Default decodes and delegates to `warningfmt`.
    fn warningfmt_encoded(
        &self,
        sg: &ShaderGlobals,
        fmt_hash: UStringHash,
        arg_count: i32,
        arg_types: &[EncodedType],
        arg_values: &[u8],
    ) {
        let msg =
            crate::encodedtypes::decode_message(fmt_hash.hash(), arg_count, arg_types, arg_values);
        self.warningfmt(sg, &msg);
    }

    /// Print with raw encoded-arg format (JIT path).
    /// Default decodes and delegates to `printfmt`.
    fn printfmt_encoded(
        &self,
        sg: &ShaderGlobals,
        fmt_hash: UStringHash,
        arg_count: i32,
        arg_types: &[EncodedType],
        arg_values: &[u8],
    ) {
        let msg =
            crate::encodedtypes::decode_message(fmt_hash.hash(), arg_count, arg_types, arg_values);
        self.printfmt(sg, &msg);
    }

    /// Print to file with raw encoded-arg format (JIT path).
    /// Default decodes and delegates to `filefmt`.
    fn filefmt_encoded(
        &self,
        sg: &ShaderGlobals,
        filename_hash: UStringHash,
        fmt_hash: UStringHash,
        arg_count: i32,
        arg_types: &[EncodedType],
        arg_values: &[u8],
    ) {
        let msg =
            crate::encodedtypes::decode_message(fmt_hash.hash(), arg_count, arg_types, arg_values);
        // Resolve filename from hash via UString lookup (C++ uses c_str())
        let filename = filename_hash
            .resolve()
            .map(|u| u.as_str().to_string())
            .unwrap_or_else(|| format!("{}", filename_hash.hash()));
        self.filefmt(sg, &filename, &msg);
    }

    // -- Caching API (for OptiX PTX) ---------------------------------------

    /// Insert a value into a named cache.
    /// Matches C++ `RendererServices::cache_insert`.
    fn cache_insert(&self, cachename: &str, key: &str, value: &str) {}

    /// Get a value from a named cache. Returns None if not found.
    /// Matches C++ `RendererServices::cache_get`.
    fn cache_get(&self, cachename: &str, key: &str) -> Option<String> {
        None
    }

    // -- Renderer info ---------------------------------------------------

    /// Return a human-readable name for this renderer.
    fn renderer_name(&self) -> &str {
        "unknown"
    }
}

/// A default "null" renderer that returns false for everything.
/// Useful for testing and as a fallback.
pub struct NullRenderer;

impl RendererServices for NullRenderer {
    fn renderer_name(&self) -> &str {
        "null"
    }
}

/// A basic renderer implementation that provides common coordinate spaces,
/// attribute queries, and in-memory point clouds. Useful for testing and simple integrations.
pub struct BasicRenderer {
    /// Named coordinate spaces: "world", "camera", "object", etc.
    pub transforms: std::collections::HashMap<String, Matrix44>,
    /// Named attributes: "object:name", "geom:uv", etc.
    pub attributes: std::collections::HashMap<String, AttributeData>,
    /// Camera-to-world matrix.
    pub camera_to_world: Matrix44,
    /// World-to-camera matrix.
    pub world_to_camera: Matrix44,
    /// In-memory point clouds (filename → PointCloud).
    pub pointcloud_manager: std::sync::RwLock<crate::pointcloud::PointCloudManager>,
}

/// Attribute data that can be queried from the renderer.
#[derive(Debug, Clone)]
pub enum AttributeData {
    Int(i32),
    Float(Float),
    String(String),
    Vec3(Vec3),
    Matrix44(Matrix44),
    IntArray(Vec<i32>),
    FloatArray(Vec<Float>),
}

impl BasicRenderer {
    /// Create a BasicRenderer with identity transforms.
    pub fn new() -> Self {
        let mut transforms = std::collections::HashMap::new();
        transforms.insert("common".to_string(), Matrix44::IDENTITY);
        transforms.insert("world".to_string(), Matrix44::IDENTITY);
        transforms.insert("camera".to_string(), Matrix44::IDENTITY);
        transforms.insert("screen".to_string(), Matrix44::IDENTITY);
        transforms.insert("NDC".to_string(), Matrix44::IDENTITY);
        transforms.insert("raster".to_string(), Matrix44::IDENTITY);
        transforms.insert("object".to_string(), Matrix44::IDENTITY);
        transforms.insert("shader".to_string(), Matrix44::IDENTITY);

        Self {
            transforms,
            attributes: std::collections::HashMap::new(),
            camera_to_world: Matrix44::IDENTITY,
            world_to_camera: Matrix44::IDENTITY,
            pointcloud_manager: std::sync::RwLock::new(crate::pointcloud::PointCloudManager::new()),
        }
    }

    /// Get write access to the point cloud manager (register clouds via get_or_create).
    pub fn pointcloud_manager_write(
        &self,
    ) -> std::sync::RwLockWriteGuard<'_, crate::pointcloud::PointCloudManager> {
        self.pointcloud_manager.write().unwrap()
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
}

impl Default for BasicRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl RendererServices for BasicRenderer {
    fn supports(&self, feature: &str) -> bool {
        matches!(
            feature,
            "get_matrix" | "get_attribute" | "get_userdata" | "build_attribute_getter"
        )
    }

    fn renderer_name(&self) -> &str {
        "basic"
    }

    fn get_matrix_named(
        &self,
        _sg: &ShaderGlobals,
        from: UStringHash,
        _time: Float,
    ) -> Option<Matrix44> {
        // Resolve UStringHash back to a string
        if let Some(us) = crate::ustring::UString::from_hash(from.hash()) {
            if let Some(mat) = self.transforms.get(us.as_str()) {
                return Some(*mat);
            }
        }
        None
    }

    fn get_matrix_named_static(&self, _sg: &ShaderGlobals, from: UStringHash) -> Option<Matrix44> {
        if let Some(us) = crate::ustring::UString::from_hash(from.hash()) {
            if let Some(mat) = self.transforms.get(us.as_str()) {
                return Some(*mat);
            }
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
        // Build the attribute key: "object:name" or just "name".
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
        _filename: UStringHash,
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
        _dresultds: Option<&mut [Float]>,
        _dresultdt: Option<&mut [Float]>,
    ) -> Result<(), String> {
        let nch = opt.nchannels.max(nchannels).min(result.len() as i32);
        // Procedural checkerboard with MIP chain: use derivatives for footprint.
        let mip = crate::texture::mip_level_from_derivs(dsdx, dtdx, dsdy, dtdy, 256, 256);
        let freq = (8.0 / (1.0 + mip)).max(1.0);
        let check = ((s * freq).floor() as i32 + (t * freq).floor() as i32) & 1;
        let val = if check != 0 { 1.0 } else { 0.0 };
        for i in 0..nch as usize {
            result[i] = val;
        }
        Ok(())
    }

    fn texture3d(
        &self,
        _filename: UStringHash,
        _handle: TextureHandle,
        _sg: &ShaderGlobals,
        opt: &crate::texture::TextureOpt,
        p: &Vec3,
        _dpdx: &Vec3,
        _dpdy: &Vec3,
        _dpdz: &Vec3,
        nchannels: i32,
        result: &mut [Float],
        _dresultds: Option<&mut [Float]>,
        _dresultdt: Option<&mut [Float]>,
        _dresultdr: Option<&mut [Float]>,
    ) -> Result<(), String> {
        let _ = opt;
        // Procedural 3D checker: alternating cubes.
        let freq = 4.0;
        let check = ((p.x * freq).floor() as i32
            + (p.y * freq).floor() as i32
            + (p.z * freq).floor() as i32)
            & 1;
        let val = if check != 0 { 1.0 } else { 0.0 };
        for i in 0..nchannels.min(result.len() as i32) as usize {
            result[i] = val;
        }
        Ok(())
    }

    fn environment(
        &self,
        _filename: UStringHash,
        _handle: TextureHandle,
        _sg: &ShaderGlobals,
        opt: &crate::texture::TextureOpt,
        r: &Vec3,
        _drdx: &Vec3,
        _drdy: &Vec3,
        nchannels: i32,
        result: &mut [Float],
        _dresultds: Option<&mut [Float]>,
        _dresultdt: Option<&mut [Float]>,
    ) -> Result<(), String> {
        let _ = opt;
        // Procedural sky-ground gradient based on Y direction.
        // +Y → sky color (0.5, 0.7, 1.0), -Y → ground (0.3, 0.3, 0.3).
        let up = r.y / (r.x * r.x + r.y * r.y + r.z * r.z).sqrt().max(1e-8);
        let blend = up * 0.5 + 0.5; // map [-1,1] → [0,1]
        let sky = [0.5, 0.7, 1.0];
        let ground = [0.3, 0.3, 0.3];
        for i in 0..nchannels.min(result.len() as i32) as usize {
            let s = if i < 3 { sky[i] } else { 1.0 };
            let g = if i < 3 { ground[i] } else { 1.0 };
            result[i] = g + blend * (s - g);
        }
        Ok(())
    }

    fn get_texture_info(
        &self,
        _filename: UStringHash,
        _handle: TextureHandle,
        _sg: &ShaderGlobals,
        _subimage: i32,
        dataname: UStringHash,
        _datatype: TypeDesc,
        data: *mut std::ffi::c_void,
    ) -> Result<(), String> {
        // Report that procedural textures "exist" and have known properties.
        let name = crate::ustring::UString::from_hash(dataname.hash())
            .map(|u| u.as_str().to_string())
            .unwrap_or_default();
        if data.is_null() {
            return Err("null data pointer".into());
        }
        match name.as_str() {
            "exists" => {
                // SAFETY: caller guarantees data points to valid i32 storage
                unsafe {
                    *(data as *mut i32) = 1;
                }
                Ok(())
            }
            "resolution" => {
                // SAFETY: caller guarantees data points to valid [i32; 2] storage
                unsafe {
                    let p = data as *mut i32;
                    *p = 256;
                    *p.add(1) = 256;
                }
                Ok(())
            }
            "channels" => {
                unsafe {
                    *(data as *mut i32) = 3;
                }
                Ok(())
            }
            _ => Err("unknown texture info query".into()),
        }
    }

    fn pointcloud_search(
        &self,
        _sg: &ShaderGlobals,
        filename: UStringHash,
        center: &Vec3,
        radius: Float,
        max_points: i32,
        sort: bool,
        out_indices: &mut [i32],
        out_distances: Option<&mut [Float]>,
        derivs_offset: i32,
        dcenter_dx: Option<&Vec3>,
        dcenter_dy: Option<&Vec3>,
    ) -> i32 {
        let name = crate::ustring::UString::from_hash(filename.hash())
            .map(|u| u.as_str().to_string())
            .unwrap_or_default();
        if name.is_empty() {
            return 0;
        }
        let mgr = self.pointcloud_manager.read().unwrap();
        let cloud = mgr.get(&name);
        if let Some(cloud) = cloud {
            let max_pt = max_points.max(0) as usize;
            let sr = crate::pointcloud::pointcloud_search(cloud, *center, radius, max_pt, sort);
            let n = sr.indices.len().min(out_indices.len());
            for (i, &idx) in sr.indices.iter().take(n).enumerate() {
                out_indices[i] = idx as i32;
            }
            if let Some(dist_buf) = out_distances {
                let need_derivs = derivs_offset > 0 && dcenter_dx.is_some() && dcenter_dy.is_some();
                let stride = (derivs_offset as usize).max(1);
                for (i, &d2) in sr.distances_sq.iter().take(n).enumerate() {
                    if i < dist_buf.len() {
                        let dist = d2.sqrt();
                        dist_buf[i] = dist;
                        if need_derivs && dist > 1e-10 {
                            let pos = &cloud.points[sr.indices[i]].position;
                            let dcdx = dcenter_dx.unwrap();
                            let dcdy = dcenter_dy.unwrap();
                            let inv_dist = 1.0 / dist;
                            let dx_val = inv_dist
                                * ((center.x - pos.x) * dcdx.x
                                    + (center.y - pos.y) * dcdx.y
                                    + (center.z - pos.z) * dcdx.z);
                            let dy_val = inv_dist
                                * ((center.x - pos.x) * dcdy.x
                                    + (center.y - pos.y) * dcdy.y
                                    + (center.z - pos.z) * dcdy.z);
                            if stride + i < dist_buf.len() {
                                dist_buf[stride + i] = dx_val;
                            }
                            if stride * 2 + i < dist_buf.len() {
                                dist_buf[stride * 2 + i] = dy_val;
                            }
                        }
                    }
                }
            }
            n as i32
        } else {
            0
        }
    }

    fn pointcloud_get(
        &self,
        _sg: &ShaderGlobals,
        filename: UStringHash,
        indices: &[i32],
        attr_name: UStringHash,
        attr_type: TypeDesc,
        out_data: *mut c_void,
    ) -> bool {
        if out_data.is_null() || indices.is_empty() {
            return false;
        }
        let name = crate::ustring::UString::from_hash(filename.hash())
            .map(|u| u.as_str().to_string())
            .unwrap_or_default();
        let attr_str = crate::ustring::UString::from_hash(attr_name.hash())
            .map(|u| u.as_str().to_string())
            .unwrap_or_default();
        let mgr = self.pointcloud_manager.read().unwrap();
        let cloud = mgr.get(&name);
        if let Some(cloud) = cloud {
            let indices_usize: Vec<usize> = indices.iter().map(|&i| i as usize).collect();
            let vals = crate::pointcloud::pointcloud_get(
                cloud,
                &indices_usize,
                crate::ustring::UString::new(&attr_str),
            );
            if attr_type.is_triple()
                || (attr_type.aggregate == crate::typedesc::Aggregate::Vec3 as u8
                    && attr_type.arraylen != 0)
            {
                for (i, v) in vals.iter().enumerate() {
                    if i < indices.len() {
                        let vec = match v {
                            Some(crate::pointcloud::PointData::Vec3(v)) => *v,
                            Some(crate::pointcloud::PointData::Float(f)) => Vec3::new(*f, *f, *f),
                            _ => Vec3::ZERO,
                        };
                        unsafe {
                            let ptr = (out_data as *mut Vec3).add(i);
                            *ptr = vec;
                        }
                    }
                }
            } else if attr_type.is_float() {
                for (i, v) in vals.iter().enumerate() {
                    if i < indices.len() {
                        if let Some(crate::pointcloud::PointData::Float(f)) = v {
                            unsafe {
                                *((out_data as *mut Float).add(i)) = *f;
                            }
                        }
                    }
                }
            }
            true
        } else {
            false
        }
    }

    fn pointcloud_write(
        &self,
        _sg: &ShaderGlobals,
        filename: UStringHash,
        pos: &Vec3,
        attrib_names: &[UStringHash],
        attrib_types: &[TypeDesc],
        attrib_data: &[*const c_void],
    ) -> bool {
        let name = crate::ustring::UString::from_hash(filename.hash())
            .map(|u| u.as_str().to_string())
            .unwrap_or_default();
        if name.is_empty() {
            return false;
        }
        let mut mgr = self.pointcloud_manager.write().unwrap();
        let cloud = mgr.get_or_create(&name);
        let mut attrs = std::collections::HashMap::new();
        for (i, &nh) in attrib_names.iter().enumerate() {
            let attr_str = crate::ustring::UString::from_hash(nh.hash())
                .map(|u| u.as_str().to_string())
                .unwrap_or_default();
            if i < attrib_data.len() && !attrib_data[i].is_null() {
                let pd = if i < attrib_types.len() && attrib_types[i].is_triple() {
                    let v = unsafe { *(attrib_data[i] as *const Vec3) };
                    crate::pointcloud::PointData::Vec3(v)
                } else if i < attrib_types.len()
                    && attrib_types[i].basetype == crate::typedesc::BaseType::Int32 as u8
                {
                    let ii = unsafe { *(attrib_data[i] as *const i32) };
                    crate::pointcloud::PointData::Int(ii)
                } else {
                    let f = unsafe { *(attrib_data[i] as *const Float) };
                    crate::pointcloud::PointData::Float(f)
                };
                attrs.insert(crate::ustring::UString::new(&attr_str), pd);
            }
        }
        crate::pointcloud::pointcloud_write(cloud, *pos, attrs);
        true
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
                false // linear transform
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_null_renderer() {
        let r = NullRenderer;
        assert!(!r.supports("anything"));
        let sg = ShaderGlobals::default();
        assert!(r.get_matrix_named(&sg, UStringHash::EMPTY, 0.0).is_none());
    }

    #[test]
    fn test_trace_opt_default() {
        let opt = TraceOpt::default();
        assert_eq!(opt.mindist, 0.0);
        assert!(!opt.shade);
    }

    #[test]
    fn test_noise_opt_default() {
        let opt = NoiseOpt::default();
        assert_eq!(opt.anisotropic, 0);
        assert_eq!(opt.bandwidth, 1.0);
        assert_eq!(opt.impulses, 16.0);
    }

    // -- Static xform tests ------------------------------------------------

    #[test]
    fn test_null_renderer_static_xform() {
        let r = NullRenderer;
        let sg = ShaderGlobals::default();
        assert!(r.get_matrix_xform_static(&sg, std::ptr::null()).is_none());
        assert!(
            r.get_inverse_matrix_xform_static(&sg, std::ptr::null())
                .is_none()
        );
    }

    #[test]
    fn test_basic_renderer_named_static() {
        let mut br = BasicRenderer::new();
        let mat = Matrix44::scale(Vec3::new(2.0, 3.0, 4.0));
        br.set_transform("myspace", mat);
        let sg = ShaderGlobals::default();
        let us = crate::ustring::UString::new("myspace");
        let hash = UStringHash::from_hash(us.hash());
        let result = br.get_matrix_named_static(&sg, hash);
        assert!(result.is_some());
        let m = result.unwrap();
        assert!((m.m[0][0] - 2.0).abs() < 1e-6);
        assert!((m.m[1][1] - 3.0).abs() < 1e-6);
        assert!((m.m[2][2] - 4.0).abs() < 1e-6);
    }

    // -- Error / warning / print tests ------------------------------------

    #[test]
    fn test_null_renderer_errorfmt() {
        // errorfmt/warningfmt/printfmt have default impls -- just verify no panic
        let r = NullRenderer;
        let sg = ShaderGlobals::default();
        r.errorfmt(&sg, "test error");
        r.warningfmt(&sg, "test warning");
        r.printfmt(&sg, "test print");
        r.filefmt(&sg, "/dev/null", "test file output");
    }

    // -- Cache tests ------------------------------------------------------

    #[test]
    fn test_null_renderer_cache() {
        let r = NullRenderer;
        r.cache_insert("ptx_cache", "key1", "value1");
        // Default impl returns None
        assert!(r.cache_get("ptx_cache", "key1").is_none());
    }

    // -- BasicRenderer coverage -------------------------------------------

    #[test]
    fn test_basic_renderer_supports() {
        let br = BasicRenderer::new();
        assert!(br.supports("get_matrix"));
        assert!(br.supports("get_attribute"));
        assert!(br.supports("get_userdata"));
        assert!(br.supports("build_attribute_getter"));
        assert!(!br.supports("OptiX"));
    }

    #[test]
    fn test_basic_renderer_name() {
        let br = BasicRenderer::new();
        assert_eq!(br.renderer_name(), "basic");
    }

    #[test]
    fn test_basic_renderer_camera() {
        let mut br = BasicRenderer::new();
        let cam = Matrix44::translate(Vec3::new(1.0, 2.0, 3.0));
        br.set_camera(cam);
        assert!((br.camera_to_world.m[0][3] - 1.0).abs() < 1e-6);
        assert!((br.camera_to_world.m[1][3] - 2.0).abs() < 1e-6);
        assert!((br.camera_to_world.m[2][3] - 3.0).abs() < 1e-6);
        // world_to_camera should be inverse
        let inv = br.world_to_camera;
        assert!((inv.m[0][3] - (-1.0)).abs() < 1e-6);
    }

    #[test]
    fn test_basic_renderer_get_attribute() {
        let mut br = BasicRenderer::new();
        br.set_attribute("test_val", AttributeData::Float(3.14));
        let sg = ShaderGlobals::default();
        let name = crate::ustring::UString::new("test_val");
        let result = br.get_attribute(
            &sg,
            false,
            UStringHash::EMPTY,
            TypeDesc::FLOAT,
            UStringHash::from_hash(name.hash()),
        );
        assert!(result.is_some());
        match result.unwrap() {
            AttributeData::Float(v) => assert!((v - 3.14).abs() < 1e-4),
            _ => panic!("expected Float"),
        }
    }

    #[test]
    fn test_basic_renderer_transform_points() {
        let br = BasicRenderer::new();
        let sg = ShaderGlobals::default();
        let from = crate::ustring::UString::new("world");
        let to = crate::ustring::UString::new("world");
        let pin = [Vec3::new(1.0, 2.0, 3.0)];
        let mut pout = [Vec3::ZERO];
        br.transform_points(
            &sg,
            UStringHash::from_hash(from.hash()),
            UStringHash::from_hash(to.hash()),
            0.0,
            &pin,
            &mut pout,
            VecSemantics::Point,
        );
        // world->world is identity, so pout == pin
        assert!((pout[0].x - 1.0).abs() < 1e-6);
        assert!((pout[0].y - 2.0).abs() < 1e-6);
        assert!((pout[0].z - 3.0).abs() < 1e-6);
    }

    #[test]
    fn test_basic_renderer_texture_checker() {
        use crate::texture::TextureOpt;
        let br = BasicRenderer::new();
        let sg = ShaderGlobals::default();
        let opt = TextureOpt::default();
        let mut result = [0.0f32; 3];
        // s=0, t=0 -> floor(0)=0, 0+0=0 even -> val=0.0
        let ok = br.texture(
            UStringHash::EMPTY,
            std::ptr::null_mut(),
            &sg,
            &opt,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            3,
            &mut result,
            None,
            None,
        );
        assert!(ok.is_ok());
        assert_eq!(result[0], 0.0);
        // s=0.13 (floor(0.13*8)=1), t=0 (floor(0)=0), 1+0=1 odd -> val=1.0
        let ok2 = br.texture(
            UStringHash::EMPTY,
            std::ptr::null_mut(),
            &sg,
            &opt,
            0.13,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            3,
            &mut result,
            None,
            None,
        );
        assert!(ok2.is_ok());
        assert_eq!(result[0], 1.0);
    }

    #[test]
    fn test_basic_renderer_get_texture_info_exists() {
        let br = BasicRenderer::new();
        let sg = ShaderGlobals::default();
        let dn = crate::ustring::UString::new("exists");
        let mut val: i32 = 0;
        let ok = br.get_texture_info(
            UStringHash::EMPTY,
            std::ptr::null_mut(),
            &sg,
            0,
            UStringHash::from_hash(dn.hash()),
            TypeDesc::INT,
            &mut val as *mut _ as *mut std::ffi::c_void,
        );
        assert!(ok.is_ok());
        assert_eq!(val, 1);
    }

    #[test]
    fn test_basic_renderer_default() {
        let br = BasicRenderer::default();
        assert_eq!(br.renderer_name(), "basic");
        assert!(br.transforms.contains_key("world"));
        assert!(br.transforms.contains_key("camera"));
    }

    #[test]
    fn test_null_renderer_name() {
        let r = NullRenderer;
        assert_eq!(r.renderer_name(), "null");
    }

    #[test]
    fn test_basic_renderer_pointcloud_defaults() {
        let br = BasicRenderer::new();
        let sg = ShaderGlobals::default();
        let center = Vec3::ZERO;
        let mut indices = [0i32; 4];
        let n = br.pointcloud_search(
            &sg,
            UStringHash::EMPTY,
            &center,
            1.0,
            4,
            false,
            &mut indices,
            None,
            0,
            None,
            None,
        );
        assert_eq!(n, 0);
        let ok = br.pointcloud_get(
            &sg,
            UStringHash::EMPTY,
            &[],
            UStringHash::EMPTY,
            TypeDesc::FLOAT,
            std::ptr::null_mut(),
        );
        assert!(!ok);
        let ok = br.pointcloud_write(&sg, UStringHash::EMPTY, &center, &[], &[], &[]);
        assert!(!ok);
    }

    #[test]
    fn test_basic_renderer_pointcloud_search_with_cloud() {
        use crate::math::Vec3;
        use crate::ustring::UString;

        let br = BasicRenderer::new();
        {
            let mut mgr = br.pointcloud_manager.write().unwrap();
            let cloud = mgr.get_or_create("test.ptc");
            for i in 0..5 {
                let pos = Vec3::new(i as f32, 0.0, 0.0);
                let mut attrs = std::collections::HashMap::new();
                attrs.insert(
                    UString::new("val"),
                    crate::pointcloud::PointData::Float(i as f32 * 0.5),
                );
                cloud.add_point(pos, attrs);
            }
        }

        let sg = ShaderGlobals::default();
        let center = Vec3::new(2.0, 0.0, 0.0);
        let mut indices = [0i32; 10];
        let mut distances = [0.0f32; 10];
        let name = UString::new("test.ptc");
        let n = br.pointcloud_search(
            &sg,
            UStringHash::from_hash(name.hash()),
            &center,
            2.0,
            10,
            true,
            &mut indices,
            Some(&mut distances),
            0,
            None,
            None,
        );
        assert!(n >= 2); // at least points 1,2,3 within radius 2 of center (2,0,0)
        assert!(distances[0] < distances[1]);

        // Test derivs_offset: d_distance_dx, d_distance_dy written per C++ pointcloud.cpp
        let br2 = BasicRenderer::new();
        {
            let mut mgr = br2.pointcloud_manager.write().unwrap();
            let cloud = mgr.get_or_create("test2.ptc");
            cloud.add_point(Vec3::new(2.0, 0.0, 0.0), {
                let mut m = std::collections::HashMap::new();
                m.insert(UString::new("v"), crate::pointcloud::PointData::Float(1.0));
                m
            });
        }
        // center (1.5,0,0), point (2,0,0) => dist=0.5, (center-point)/dist = (-1,0,0)
        // d_distance_dx = (-1,0,0)·dCdx = -1*0.1 = -0.1
        let center = Vec3::new(1.5, 0.0, 0.0);
        let dcdx = Vec3::new(0.1, 0.0, 0.0);
        let dcdy = Vec3::new(0.0, 0.1, 0.0);
        let mut indices2 = [0i32; 10];
        let mut dists2 = [0.0f32; 30];
        let n2 = br2.pointcloud_search(
            &sg,
            UStringHash::from_hash(UString::new("test2.ptc").hash()),
            &center,
            2.0,
            10,
            true,
            &mut indices2,
            Some(&mut dists2),
            10,
            Some(&dcdx),
            Some(&dcdy),
        );
        assert!(n2 >= 1, "should find the point");
        assert!(
            (dists2[0] - 0.5).abs() < 0.001,
            "dist to (2,0,0) from (1.5,0,0) = 0.5"
        );
        assert!(
            (dists2[10] - (-0.1)).abs() < 0.01,
            "d_distance_dx = (c-p)/dist · dCdx"
        );

        let mut out_val = 0.0f32;
        let ok = br.pointcloud_get(
            &sg,
            UStringHash::from_hash(name.hash()),
            &indices[..n as usize],
            UStringHash::from_hash(UString::new("val").hash()),
            TypeDesc::FLOAT,
            &mut out_val as *mut _ as *mut std::ffi::c_void,
        );
        assert!(ok);
    }
}
