//! Shadow map array management.
//!
//! Port of pxr/imaging/glf/simpleShadowArray.h

use usd_gf::{Matrix4d, Vec2i};

/// Type alias for 4x4 matrix matching USD naming convention.
pub type GfMatrix4d = Matrix4d;
/// Type alias for 2D integer vector matching USD naming convention.
pub type GfVec2i = Vec2i;

/// Shadow map array for managing multiple shadow maps.
///
/// Each shadow map corresponds to a shadow-casting light and stores
/// depth information for shadow computation.
#[derive(Debug)]
pub struct GlfSimpleShadowArray {
    /// Shadow map resolutions
    resolutions: Vec<GfVec2i>,
    /// View matrices for shadow generation passes
    view_matrices: Vec<GfMatrix4d>,
    /// Projection matrices for shadow generation passes
    projection_matrices: Vec<GfMatrix4d>,
    /// GL texture IDs for shadow maps
    shadow_map_textures: Vec<u32>,
    /// GL framebuffer object ID (used only with opengl feature)
    #[cfg_attr(not(feature = "opengl"), allow(dead_code))]
    framebuffer: u32,
    /// GL depth sampler ID
    depth_sampler: u32,
    /// GL comparison sampler ID
    compare_sampler: u32,
    /// Saved viewport for restoration (used only with opengl feature)
    #[cfg_attr(not(feature = "opengl"), allow(dead_code))]
    saved_viewport: [i32; 4],
    /// Saved draw FBO for restoration in end_capture (matches C++ _unbindRestoreDrawFramebuffer)
    #[cfg_attr(not(feature = "opengl"), allow(dead_code))]
    saved_draw_fbo: u32,
    /// Saved read FBO for restoration in end_capture (matches C++ _unbindRestoreReadFramebuffer)
    #[cfg_attr(not(feature = "opengl"), allow(dead_code))]
    saved_read_fbo: u32,
    /// Whether shadow map textures were allocated externally (SetShadowMapTextureArray).
    /// Used in C++ to skip FreeTextures() when externally managed; kept here for API parity.
    #[allow(dead_code)]
    textures_allocated_externally: bool,
}

impl GlfSimpleShadowArray {
    /// Creates a new shadow array.
    pub fn new() -> Self {
        Self {
            resolutions: Vec::new(),
            view_matrices: Vec::new(),
            projection_matrices: Vec::new(),
            shadow_map_textures: Vec::new(),
            framebuffer: 0,
            depth_sampler: 0,
            compare_sampler: 0,
            saved_viewport: [0; 4],
            saved_draw_fbo: 0,
            saved_read_fbo: 0,
            textures_allocated_externally: false,
        }
    }

    /// Returns the GL texture id of the shadow texture at the given index.
    ///
    /// # Stub Implementation
    /// Returns 0.
    pub fn get_shadow_map_texture(&self, shadow_index: usize) -> u32 {
        self.shadow_map_textures
            .get(shadow_index)
            .copied()
            .unwrap_or(0)
    }

    /// Returns the GL sampler id of the sampler object used to read raw depth values.
    pub fn get_shadow_map_depth_sampler(&self) -> u32 {
        self.depth_sampler
    }

    /// Returns the GL sampler id of the sampler object used for depth comparison.
    pub fn get_shadow_map_compare_sampler(&self) -> u32 {
        self.compare_sampler
    }

    /// Returns all shadow map texture IDs.
    pub fn get_textures(&self) -> &[u32] {
        &self.shadow_map_textures
    }

    /// Sets the resolutions of all shadow maps.
    ///
    /// No-ops if the resolution list is unchanged (matches C++ equality check).
    /// Only frees/reallocates textures when not externally managed.
    pub fn set_shadow_map_resolutions(&mut self, resolutions: Vec<GfVec2i>) {
        // Skip reallocate if nothing changed (C++ early-return on equality)
        if self.resolutions == resolutions {
            return;
        }
        self.resolutions = resolutions;
        let n = self.resolutions.len();
        if self.view_matrices.len() != n {
            self.view_matrices.resize(n, GfMatrix4d::identity());
        }
        if self.projection_matrices.len() != n {
            self.projection_matrices.resize(n, GfMatrix4d::identity());
        }
        // Textures are always re-allocated here (GlfSimpleShadowArray owns them)
        self.allocate_shadow_maps();
    }

    /// Returns the number of shadow map generation passes required.
    ///
    /// Currently one per shadow map (corresponding to a shadow casting light).
    pub fn get_num_shadow_map_passes(&self) -> usize {
        self.resolutions.len()
    }

    /// Returns the shadow map resolution for a given pass.
    pub fn get_shadow_map_size(&self, pass: usize) -> Option<GfVec2i> {
        self.resolutions.get(pass).copied()
    }

    /// Gets the view matrix for a shadow map generation pass.
    pub fn get_view_matrix(&self, index: usize) -> Option<&GfMatrix4d> {
        self.view_matrices.get(index)
    }

    /// Sets the view matrix for a shadow map generation pass.
    pub fn set_view_matrix(&mut self, index: usize, matrix: GfMatrix4d) {
        if index < self.view_matrices.len() {
            self.view_matrices[index] = matrix;
        }
    }

    /// Gets the projection matrix for a shadow map generation pass.
    pub fn get_projection_matrix(&self, index: usize) -> Option<&GfMatrix4d> {
        self.projection_matrices.get(index)
    }

    /// Sets the projection matrix for a shadow map generation pass.
    pub fn set_projection_matrix(&mut self, index: usize, matrix: GfMatrix4d) {
        if index < self.projection_matrices.len() {
            self.projection_matrices[index] = matrix;
        }
    }

    /// Gets the world-to-shadow transform with NDC-to-UV bias applied.
    ///
    /// Matches C++ `GetWorldToShadowMatrix()`: `view * proj * scale(0.5) * translate(0.5)`.
    /// Transforms world-space positions into `[0,1]` texture/depth space directly —
    /// `(X,Y)` is the shadow map texture coordinate, `Z` is the compare value.
    ///
    /// USD uses Imath **row-vector** convention: `v' = v * M`.
    /// The combined bias matrix in row-major layout (scale then translate):
    ///
    /// ```text
    /// | 0.5  0    0    0 |
    /// | 0    0.5  0    0 |
    /// | 0    0    0.5  0 |
    /// | 0.5  0.5  0.5  1 |
    /// ```
    pub fn get_world_to_shadow_matrix(&self, index: usize) -> GfMatrix4d {
        if let (Some(view), Some(proj)) = (
            self.view_matrices.get(index),
            self.projection_matrices.get(index),
        ) {
            // C++: GetViewMatrix(index) * GetProjectionMatrix(index) * size * center
            // where size = scale(0.5,0.5,0.5), center = translate(0.5,0.5,0.5)
            // Combined bias maps NDC [-1,1] -> texture [0,1].
            #[rustfmt::skip]
            let bias = GfMatrix4d::new(
                0.5, 0.0, 0.0, 0.0,
                0.0, 0.5, 0.0, 0.0,
                0.0, 0.0, 0.5, 0.0,
                0.5, 0.5, 0.5, 1.0,
            );
            *view * *proj * bias
        } else {
            GfMatrix4d::identity()
        }
    }

    /// Gets the raw (unbiased) world-to-shadow clip transform: `view * proj`.
    ///
    /// Returns NDC coordinates `[-1, 1]` without the NDC-to-UV remap.
    /// Provided for cases that need the raw clip-space transform.
    pub fn get_world_to_shadow_matrix_unbiased(&self, index: usize) -> GfMatrix4d {
        if let (Some(view), Some(proj)) = (
            self.view_matrices.get(index),
            self.projection_matrices.get(index),
        ) {
            *view * *proj
        } else {
            GfMatrix4d::identity()
        }
    }

    /// Returns the number of shadow-casting lights (= shadow pass count).
    ///
    /// Alias of `get_num_shadow_map_passes()` for shorter calling convention.
    pub fn get_num_shadows(&self) -> usize {
        self.resolutions.len()
    }

    /// Binds necessary resources for a shadow map generation pass.
    ///
    /// Mirrors C++ `BeginCapture()`: binds the FBO, sets the viewport,
    /// sets `glDepthRange(0, 0.99999)` and enables `GL_DEPTH_CLAMP`
    /// (depth=1.0 means "infinity / no occluder").
    #[cfg(feature = "opengl")]
    pub fn begin_capture(&mut self, index: usize, clear: bool) {
        if index >= self.resolutions.len() {
            return;
        }

        let resolution = self.resolutions[index];
        let texture = self.shadow_map_textures.get(index).copied().unwrap_or(0);

        unsafe {
            // C++ _BindFramebuffer: save current draw+read FBOs before rebinding.
            // Mirrors: glGetIntegerv(GL_DRAW_FRAMEBUFFER_BINDING, &_unbindRestoreDrawFramebuffer)
            //          glGetIntegerv(GL_READ_FRAMEBUFFER_BINDING, &_unbindRestoreReadFramebuffer)
            let mut draw_fbo: i32 = 0;
            let mut read_fbo: i32 = 0;
            gl::GetIntegerv(gl::DRAW_FRAMEBUFFER_BINDING, &mut draw_fbo);
            gl::GetIntegerv(gl::READ_FRAMEBUFFER_BINDING, &mut read_fbo);
            self.saved_draw_fbo = draw_fbo as u32;
            self.saved_read_fbo = read_fbo as u32;

            // Bind shadow FBO and attach the shadow depth texture.
            gl::BindFramebuffer(gl::FRAMEBUFFER, self.framebuffer);
            gl::FramebufferTexture2D(
                gl::FRAMEBUFFER,
                gl::DEPTH_ATTACHMENT,
                gl::TEXTURE_2D,
                texture,
                0,
            );

            // Clear depth buffer (C++: done immediately after FBO bind).
            if clear {
                gl::Clear(gl::DEPTH_BUFFER_BIT);
            }

            // Save current viewport for restoration in end_capture.
            gl::GetIntegerv(gl::VIEWPORT, self.saved_viewport.as_mut_ptr());

            // Set viewport to shadow map size.
            gl::Viewport(0, 0, resolution.x, resolution.y);

            // depth=1.0 == infinity (no occluders); clamp prevents near-clip artefacts.
            gl::DepthRange(0.0, 0.99999);
            gl::Enable(gl::DEPTH_CLAMP);
        }
    }

    /// No-op when OpenGL feature is disabled.
    #[cfg(not(feature = "opengl"))]
    pub fn begin_capture(&mut self, _index: usize, _clear: bool) {}

    /// Unbinds resources after a shadow map generation pass.
    ///
    /// Mirrors C++ `EndCapture()` + `_UnbindFramebuffer()`:
    /// - Restores `glDepthRange(0, 1)` and disables `GL_DEPTH_CLAMP`.
    /// - Restores saved draw and read FBOs (saved in `begin_capture`).
    /// - Restores the viewport.
    ///
    /// C++ `_UnbindFramebuffer` binds draw and read targets separately:
    /// `glBindFramebuffer(GL_DRAW_FRAMEBUFFER, _unbindRestoreDrawFramebuffer)` and
    /// `glBindFramebuffer(GL_READ_FRAMEBUFFER, _unbindRestoreReadFramebuffer)`.
    #[cfg(feature = "opengl")]
    pub fn end_capture(&mut self, _index: usize) {
        unsafe {
            // Restore GL defaults for depth range and depth clamp.
            gl::DepthRange(0.0, 1.0);
            gl::Disable(gl::DEPTH_CLAMP);

            // Restore saved draw and read framebuffers separately.
            // C++: glBindFramebuffer(GL_DRAW_FRAMEBUFFER, _unbindRestoreDrawFramebuffer)
            //      glBindFramebuffer(GL_READ_FRAMEBUFFER, _unbindRestoreReadFramebuffer)
            gl::BindFramebuffer(gl::DRAW_FRAMEBUFFER, self.saved_draw_fbo);
            gl::BindFramebuffer(gl::READ_FRAMEBUFFER, self.saved_read_fbo);

            // Restore viewport.
            gl::Viewport(
                self.saved_viewport[0],
                self.saved_viewport[1],
                self.saved_viewport[2],
                self.saved_viewport[3],
            );
        }
    }

    /// No-op when OpenGL feature is disabled.
    #[cfg(not(feature = "opengl"))]
    pub fn end_capture(&mut self, _index: usize) {}

    /// Sets the GL texture ids of shadow textures.
    ///
    /// As opposed to creating them internally. Used when shadow maps
    /// are managed externally.
    pub fn set_shadow_map_textures(&mut self, textures: Vec<u32>) {
        self.shadow_map_textures = textures;
    }

    /// Sets the depth sampler ID.
    pub fn set_depth_sampler(&mut self, sampler: u32) {
        self.depth_sampler = sampler;
    }

    /// Sets the comparison sampler ID.
    pub fn set_compare_sampler(&mut self, sampler: u32) {
        self.compare_sampler = sampler;
    }

    /// Allocate shadow samplers explicitly (depth + comparison).
    ///
    /// Mirrors C++ `AllocSamplers()`.  Creates GL sampler objects for reading
    /// raw depth values and for PCF shadow comparison.  Both samplers use
    /// LINEAR filtering and CLAMP_TO_BORDER with a white (1,1,1,1) border so
    /// that lookups outside the shadow map report "fully lit".
    #[cfg(feature = "opengl")]
    pub fn alloc_samplers(&mut self) {
        unsafe {
            // Border color (1,1,1,1) = no shadow outside map extents.
            let border: [f32; 4] = [1.0, 1.0, 1.0, 1.0];

            // Depth sampler — raw depth reads, no comparison.
            if self.depth_sampler == 0 {
                gl::GenSamplers(1, &mut self.depth_sampler);
                gl::SamplerParameteri(
                    self.depth_sampler,
                    gl::TEXTURE_MIN_FILTER,
                    gl::LINEAR as i32,
                );
                gl::SamplerParameteri(
                    self.depth_sampler,
                    gl::TEXTURE_MAG_FILTER,
                    gl::LINEAR as i32,
                );
                gl::SamplerParameteri(
                    self.depth_sampler,
                    gl::TEXTURE_WRAP_S,
                    gl::CLAMP_TO_BORDER as i32,
                );
                gl::SamplerParameteri(
                    self.depth_sampler,
                    gl::TEXTURE_WRAP_T,
                    gl::CLAMP_TO_BORDER as i32,
                );
                gl::SamplerParameterfv(
                    self.depth_sampler,
                    gl::TEXTURE_BORDER_COLOR,
                    border.as_ptr(),
                );
            }

            // Compare sampler — PCF LEQUAL, also LINEAR + CLAMP_TO_BORDER.
            if self.compare_sampler == 0 {
                gl::GenSamplers(1, &mut self.compare_sampler);
                gl::SamplerParameteri(
                    self.compare_sampler,
                    gl::TEXTURE_MIN_FILTER,
                    gl::LINEAR as i32,
                );
                gl::SamplerParameteri(
                    self.compare_sampler,
                    gl::TEXTURE_MAG_FILTER,
                    gl::LINEAR as i32,
                );
                gl::SamplerParameteri(
                    self.compare_sampler,
                    gl::TEXTURE_WRAP_S,
                    gl::CLAMP_TO_BORDER as i32,
                );
                gl::SamplerParameteri(
                    self.compare_sampler,
                    gl::TEXTURE_WRAP_T,
                    gl::CLAMP_TO_BORDER as i32,
                );
                gl::SamplerParameterfv(
                    self.compare_sampler,
                    gl::TEXTURE_BORDER_COLOR,
                    border.as_ptr(),
                );
                gl::SamplerParameteri(
                    self.compare_sampler,
                    gl::TEXTURE_COMPARE_MODE,
                    gl::COMPARE_REF_TO_TEXTURE as i32,
                );
                gl::SamplerParameteri(
                    self.compare_sampler,
                    gl::TEXTURE_COMPARE_FUNC,
                    gl::LEQUAL as i32,
                );
            }
        }
    }

    /// No-op stub when the `opengl` feature is disabled.
    #[cfg(not(feature = "opengl"))]
    pub fn alloc_samplers(&mut self) {}

    // Internal methods
    #[cfg(feature = "opengl")]
    fn allocate_shadow_maps(&mut self) {
        // Free existing resources first
        self.free_resources();

        if self.resolutions.is_empty() {
            return;
        }

        unsafe {
            // Create framebuffer
            gl::GenFramebuffers(1, &mut self.framebuffer);

            // Allocate textures as DEPTH_COMPONENT32F (matches C++ _AllocTextures).
            // Sampler state is handled separately via alloc_samplers().
            let count = self.resolutions.len();
            self.shadow_map_textures.resize(count, 0);
            gl::GenTextures(count as i32, self.shadow_map_textures.as_mut_ptr());

            for (i, &resolution) in self.resolutions.iter().enumerate() {
                let tex = self.shadow_map_textures[i];
                gl::BindTexture(gl::TEXTURE_2D, tex);
                gl::TexImage2D(
                    gl::TEXTURE_2D,
                    0,
                    gl::DEPTH_COMPONENT32F as i32, // C++: GL_DEPTH_COMPONENT32F
                    resolution.x,
                    resolution.y,
                    0,
                    gl::DEPTH_COMPONENT,
                    gl::FLOAT,
                    std::ptr::null(),
                );
            }
            gl::BindTexture(gl::TEXTURE_2D, 0);

            // Allocate samplers (LINEAR + CLAMP_TO_BORDER + compare mode)
            self.alloc_samplers();
        }
    }

    #[cfg(not(feature = "opengl"))]
    fn allocate_shadow_maps(&mut self) {
        self.shadow_map_textures = vec![0; self.resolutions.len()];
    }

    /// Frees all shadow map resources.
    #[cfg(feature = "opengl")]
    pub fn free_resources(&mut self) {
        unsafe {
            if self.framebuffer != 0 {
                gl::DeleteFramebuffers(1, &self.framebuffer);
                self.framebuffer = 0;
            }
            if !self.shadow_map_textures.is_empty() {
                gl::DeleteTextures(
                    self.shadow_map_textures.len() as i32,
                    self.shadow_map_textures.as_ptr(),
                );
                self.shadow_map_textures.clear();
            }
            if self.depth_sampler != 0 {
                gl::DeleteSamplers(1, &self.depth_sampler);
                self.depth_sampler = 0;
            }
            if self.compare_sampler != 0 {
                gl::DeleteSamplers(1, &self.compare_sampler);
                self.compare_sampler = 0;
            }
        }
    }

    /// No-op when OpenGL feature is disabled.
    #[cfg(not(feature = "opengl"))]
    pub fn free_resources(&mut self) {
        self.shadow_map_textures.clear();
        self.depth_sampler = 0;
        self.compare_sampler = 0;
    }
}

impl Default for GlfSimpleShadowArray {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for GlfSimpleShadowArray {
    fn drop(&mut self) {
        self.free_resources();
    }
}

#[cfg(all(test, feature = "opengl"))]
pub(crate) fn run_gl_tests() {
    use super::*;

    let array = GlfSimpleShadowArray::new();
    assert_eq!(array.get_num_shadow_map_passes(), 0);

    let mut array = GlfSimpleShadowArray::default();
    let resolutions = vec![GfVec2i::new(512, 512), GfVec2i::new(1024, 1024)];

    array.set_shadow_map_resolutions(resolutions.clone());
    assert_eq!(array.get_num_shadow_map_passes(), 2);
    assert_eq!(array.get_shadow_map_size(0), Some(GfVec2i::new(512, 512)));
    assert_eq!(array.get_shadow_map_size(1), Some(GfVec2i::new(1024, 1024)));

    let mut array = GlfSimpleShadowArray::new();
    array.set_shadow_map_resolutions(vec![GfVec2i::new(512, 512)]);

    let view = GfMatrix4d::identity();
    let proj = GfMatrix4d::identity();

    array.set_view_matrix(0, view);
    array.set_projection_matrix(0, proj);

    assert_eq!(array.get_view_matrix(0), Some(&GfMatrix4d::identity()));
    assert_eq!(
        array.get_projection_matrix(0),
        Some(&GfMatrix4d::identity())
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // Basic lifecycle
    // -----------------------------------------------------------------------

    #[test]
    fn test_new_empty() {
        let array = GlfSimpleShadowArray::new();
        assert_eq!(array.get_num_shadow_map_passes(), 0);
        assert_eq!(array.get_num_shadows(), 0);
    }

    #[test]
    fn test_set_resolutions_and_count() {
        let mut array = GlfSimpleShadowArray::new();
        array.set_shadow_map_resolutions(vec![GfVec2i::new(512, 512), GfVec2i::new(1024, 1024)]);
        assert_eq!(array.get_num_shadow_map_passes(), 2);
        assert_eq!(array.get_num_shadows(), 2);
        assert_eq!(array.get_shadow_map_size(0), Some(GfVec2i::new(512, 512)));
        assert_eq!(array.get_shadow_map_size(1), Some(GfVec2i::new(1024, 1024)));
        assert_eq!(array.get_shadow_map_size(2), None);
    }

    #[test]
    fn test_set_same_resolutions_is_noop() {
        let res = vec![GfVec2i::new(512, 512)];
        let mut array = GlfSimpleShadowArray::new();
        array.set_shadow_map_resolutions(res.clone());
        // Setting the same list should not crash and should preserve count
        array.set_shadow_map_resolutions(res.clone());
        assert_eq!(array.get_num_shadows(), 1);
    }

    // -----------------------------------------------------------------------
    // Matrix accessors
    // -----------------------------------------------------------------------

    #[test]
    fn test_view_projection_roundtrip() {
        let mut array = GlfSimpleShadowArray::new();
        array.set_shadow_map_resolutions(vec![GfVec2i::new(512, 512)]);

        let view = GfMatrix4d::identity();
        let proj = GfMatrix4d::identity();
        array.set_view_matrix(0, view);
        array.set_projection_matrix(0, proj);

        assert_eq!(array.get_view_matrix(0), Some(&GfMatrix4d::identity()));
        assert_eq!(
            array.get_projection_matrix(0),
            Some(&GfMatrix4d::identity())
        );
    }

    #[test]
    fn test_out_of_bounds_matrix_returns_none() {
        let array = GlfSimpleShadowArray::new();
        assert!(array.get_view_matrix(99).is_none());
        assert!(array.get_projection_matrix(99).is_none());
    }

    // -----------------------------------------------------------------------
    // get_world_to_shadow_matrix — now includes NDC-to-UV bias (C++ parity)
    // -----------------------------------------------------------------------

    #[test]
    fn test_world_to_shadow_includes_bias() {
        // With identity view+proj the result equals the bias matrix itself.
        // NDC corner (-1,-1,-1) must map to (0,0,0) in texture space.
        let mut array = GlfSimpleShadowArray::new();
        array.set_shadow_map_resolutions(vec![GfVec2i::new(512, 512)]);
        array.set_view_matrix(0, GfMatrix4d::identity());
        array.set_projection_matrix(0, GfMatrix4d::identity());

        let m = array.get_world_to_shadow_matrix(0);

        // Row-vector convention: v' = v * M
        let ndc_min = [-1.0_f64, -1.0, -1.0, 1.0];
        let mut uv = [0.0_f64; 4];
        for col in 0..4 {
            for row in 0..4 {
                uv[col] += ndc_min[row] * m[row][col];
            }
        }
        let eps = 1e-9;
        assert!((uv[0] - 0.0).abs() < eps, "u expected 0, got {}", uv[0]);
        assert!((uv[1] - 0.0).abs() < eps, "v expected 0, got {}", uv[1]);
        assert!((uv[2] - 0.0).abs() < eps, "depth expected 0, got {}", uv[2]);

        // NDC corner (1,1,1) must map to (1,1,1)
        let ndc_max = [1.0_f64, 1.0, 1.0, 1.0];
        let mut uv_max = [0.0_f64; 4];
        for col in 0..4 {
            for row in 0..4 {
                uv_max[col] += ndc_max[row] * m[row][col];
            }
        }
        assert!((uv_max[0] - 1.0).abs() < eps);
        assert!((uv_max[1] - 1.0).abs() < eps);
        assert!((uv_max[2] - 1.0).abs() < eps);
    }

    #[test]
    fn test_world_to_shadow_origin_maps_to_half() {
        // NDC origin (0,0,0) must map to (0.5, 0.5, 0.5).
        let mut array = GlfSimpleShadowArray::new();
        array.set_shadow_map_resolutions(vec![GfVec2i::new(512, 512)]);
        array.set_view_matrix(0, GfMatrix4d::identity());
        array.set_projection_matrix(0, GfMatrix4d::identity());

        let m = array.get_world_to_shadow_matrix(0);

        let ndc_origin = [0.0_f64, 0.0, 0.0, 1.0];
        let mut uv = [0.0_f64; 4];
        for col in 0..4 {
            for row in 0..4 {
                uv[col] += ndc_origin[row] * m[row][col];
            }
        }
        let eps = 1e-9;
        assert!((uv[0] - 0.5).abs() < eps, "u expected 0.5, got {}", uv[0]);
        assert!((uv[1] - 0.5).abs() < eps, "v expected 0.5, got {}", uv[1]);
        assert!(
            (uv[2] - 0.5).abs() < eps,
            "depth expected 0.5, got {}",
            uv[2]
        );
    }

    #[test]
    fn test_world_to_shadow_out_of_bounds_is_identity() {
        let array = GlfSimpleShadowArray::new();
        assert_eq!(array.get_world_to_shadow_matrix(0), GfMatrix4d::identity());
    }

    // -----------------------------------------------------------------------
    // get_world_to_shadow_matrix_unbiased — raw clip-space transform
    // -----------------------------------------------------------------------

    #[test]
    fn test_unbiased_identity_returns_identity() {
        let mut array = GlfSimpleShadowArray::new();
        array.set_shadow_map_resolutions(vec![GfVec2i::new(512, 512)]);
        array.set_view_matrix(0, GfMatrix4d::identity());
        array.set_projection_matrix(0, GfMatrix4d::identity());
        // identity * identity = identity (no bias)
        assert_eq!(
            array.get_world_to_shadow_matrix_unbiased(0),
            GfMatrix4d::identity()
        );
    }
}
