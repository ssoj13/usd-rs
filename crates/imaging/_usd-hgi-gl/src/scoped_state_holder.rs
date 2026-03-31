//! OpenGL scoped state save/restore
//!
//! Matches C++ `HgiGL_ScopedStateHolder` — captures ~40 GL state variables
//! on construction and restores them on drop.

/// RAII guard that saves the current OpenGL state on creation and restores it
/// on drop. Used to bracket HGI command execution so that the surrounding
/// application's GL state is not disturbed.
///
/// Matches C++ `HgiGL_ScopedStateHolder` from `hgiGL/scopedStateHolder.h`.
#[derive(Debug)]
#[allow(dead_code)]
pub struct HgiGLScopedStateHolder {
    /// GL_RENDERBUFFER_BINDING
    restore_render_buffer: i32,
    /// GL_VERTEX_ARRAY_BINDING
    restore_vao: i32,

    // Depth state
    restore_depth_test: bool,
    restore_depth_write_mask: bool,
    restore_depth_func: i32,
    restore_depth_bias: bool,
    restore_depth_bias_constant_factor: f32,
    restore_depth_bias_slope_factor: f32,
    restore_depth_clamp: bool,
    depth_range: [f32; 2],

    // Stencil state (front=0, back=1)
    restore_stencil_test: bool,
    restore_stencil_compare_fn: [i32; 2],
    restore_stencil_reference_value: [i32; 2],
    restore_stencil_read_mask: [i32; 2],
    restore_stencil_write_mask: [i32; 2],
    restore_stencil_fail: [i32; 2],
    restore_stencil_depth_fail: [i32; 2],
    restore_stencil_depth_pass: [i32; 2],

    // Viewport
    restore_viewport: [i32; 4],

    // Blend state
    restore_blend_enabled: bool,
    restore_color_op: i32,
    restore_alpha_op: i32,
    restore_color_src_fn: i32,
    restore_alpha_src_fn: i32,
    restore_color_dst_fn: i32,
    restore_alpha_dst_fn: i32,
    restore_blend_color: [f32; 4],
    restore_alpha_to_coverage: bool,
    restore_sample_alpha_to_one: bool,

    // Raster state
    line_width: f32,
    cull_face: bool,
    cull_mode: i32,
    front_face: i32,
    rasterizer_discard: bool,
    restore_framebuffer_srgb: bool,
    restore_multisample: bool,
    restore_point_smooth: bool,
    restore_point_sprite: bool,

    // Clip distances
    restore_clip_distances: Vec<bool>,

    // Pixel store
    restore_unpack_alignment: i32,
    restore_pack_alignment: i32,

    // Miscellaneous
    restore_cube_map_seamless: bool,
}

impl HgiGLScopedStateHolder {
    /// Capture current GL state. Matches C++ constructor body.
    #[cfg(feature = "opengl")]
    pub fn new() -> Self {
        let mut s = Self::zeroed();
        unsafe {
            // Framebuffer / VAO
            gl::GetIntegerv(gl::RENDERBUFFER_BINDING, &mut s.restore_render_buffer);
            gl::GetIntegerv(gl::VERTEX_ARRAY_BINDING, &mut s.restore_vao);

            // Depth
            let mut b: u8 = 0;
            gl::GetBooleanv(gl::DEPTH_TEST, &mut b);
            s.restore_depth_test = b != 0;

            gl::GetBooleanv(gl::DEPTH_WRITEMASK, &mut b);
            s.restore_depth_write_mask = b != 0;

            gl::GetIntegerv(gl::DEPTH_FUNC, &mut s.restore_depth_func);

            gl::GetBooleanv(gl::POLYGON_OFFSET_FILL, &mut b);
            s.restore_depth_bias = b != 0;
            gl::GetFloatv(
                gl::POLYGON_OFFSET_UNITS,
                &mut s.restore_depth_bias_constant_factor,
            );
            gl::GetFloatv(
                gl::POLYGON_OFFSET_FACTOR,
                &mut s.restore_depth_bias_slope_factor,
            );

            gl::GetBooleanv(gl::DEPTH_CLAMP, &mut b);
            s.restore_depth_clamp = b != 0;
            gl::GetFloatv(gl::DEPTH_RANGE, s.depth_range.as_mut_ptr());

            // Stencil (front)
            gl::GetBooleanv(gl::STENCIL_TEST, &mut b);
            s.restore_stencil_test = b != 0;
            gl::GetIntegerv(gl::STENCIL_FUNC, &mut s.restore_stencil_compare_fn[0]);
            gl::GetIntegerv(gl::STENCIL_REF, &mut s.restore_stencil_reference_value[0]);
            gl::GetIntegerv(gl::STENCIL_VALUE_MASK, &mut s.restore_stencil_read_mask[0]);
            gl::GetIntegerv(gl::STENCIL_FAIL, &mut s.restore_stencil_fail[0]);
            gl::GetIntegerv(
                gl::STENCIL_PASS_DEPTH_FAIL,
                &mut s.restore_stencil_depth_fail[0],
            );
            gl::GetIntegerv(
                gl::STENCIL_PASS_DEPTH_PASS,
                &mut s.restore_stencil_depth_pass[0],
            );
            gl::GetIntegerv(gl::STENCIL_WRITEMASK, &mut s.restore_stencil_write_mask[0]);

            // Stencil (back)
            gl::GetIntegerv(gl::STENCIL_BACK_FUNC, &mut s.restore_stencil_compare_fn[1]);
            gl::GetIntegerv(
                gl::STENCIL_BACK_REF,
                &mut s.restore_stencil_reference_value[1],
            );
            gl::GetIntegerv(
                gl::STENCIL_BACK_VALUE_MASK,
                &mut s.restore_stencil_read_mask[1],
            );
            gl::GetIntegerv(gl::STENCIL_BACK_FAIL, &mut s.restore_stencil_fail[1]);
            gl::GetIntegerv(
                gl::STENCIL_BACK_PASS_DEPTH_FAIL,
                &mut s.restore_stencil_depth_fail[1],
            );
            gl::GetIntegerv(
                gl::STENCIL_BACK_PASS_DEPTH_PASS,
                &mut s.restore_stencil_depth_pass[1],
            );
            gl::GetIntegerv(
                gl::STENCIL_BACK_WRITEMASK,
                &mut s.restore_stencil_write_mask[1],
            );

            // Viewport
            gl::GetIntegerv(gl::VIEWPORT, s.restore_viewport.as_mut_ptr());

            // Blend
            gl::GetBooleanv(gl::BLEND, &mut b);
            s.restore_blend_enabled = b != 0;
            gl::GetIntegerv(gl::BLEND_EQUATION_RGB, &mut s.restore_color_op);
            gl::GetIntegerv(gl::BLEND_EQUATION_ALPHA, &mut s.restore_alpha_op);
            gl::GetIntegerv(gl::BLEND_SRC_RGB, &mut s.restore_color_src_fn);
            gl::GetIntegerv(gl::BLEND_SRC_ALPHA, &mut s.restore_alpha_src_fn);
            gl::GetIntegerv(gl::BLEND_DST_RGB, &mut s.restore_color_dst_fn);
            gl::GetIntegerv(gl::BLEND_DST_ALPHA, &mut s.restore_alpha_dst_fn);
            gl::GetFloatv(gl::BLEND_COLOR, s.restore_blend_color.as_mut_ptr());
            gl::GetBooleanv(gl::SAMPLE_ALPHA_TO_COVERAGE, &mut b);
            s.restore_alpha_to_coverage = b != 0;
            gl::GetBooleanv(gl::SAMPLE_ALPHA_TO_ONE, &mut b);
            s.restore_sample_alpha_to_one = b != 0;

            // Raster
            gl::GetFloatv(gl::LINE_WIDTH, &mut s.line_width);
            gl::GetBooleanv(gl::CULL_FACE, &mut b);
            s.cull_face = b != 0;
            gl::GetIntegerv(gl::CULL_FACE_MODE, &mut s.cull_mode);
            gl::GetIntegerv(gl::FRONT_FACE, &mut s.front_face);
            gl::GetBooleanv(gl::RASTERIZER_DISCARD, &mut b);
            s.rasterizer_discard = b != 0;

            // Framebuffer sRGB
            gl::GetBooleanv(gl::FRAMEBUFFER_SRGB, &mut b);
            s.restore_framebuffer_srgb = b != 0;

            // Clip distances
            let mut max_clip: i32 = 0;
            gl::GetIntegerv(gl::MAX_CLIP_PLANES, &mut max_clip);
            s.restore_clip_distances.resize(max_clip as usize, false);
            for i in 0..max_clip {
                gl::GetBooleanv(gl::CLIP_DISTANCE0 + i as u32, &mut b);
                s.restore_clip_distances[i as usize] = b != 0;
            }

            // Misc
            gl::GetBooleanv(gl::MULTISAMPLE, &mut b);
            s.restore_multisample = b != 0;
            gl::GetBooleanv(gl::POINT_SMOOTH, &mut b);
            s.restore_point_smooth = b != 0;
            gl::GetBooleanv(gl::POINT_SPRITE, &mut b);
            s.restore_point_sprite = b != 0;

            gl::GetIntegerv(gl::UNPACK_ALIGNMENT, &mut s.restore_unpack_alignment);
            gl::GetIntegerv(gl::PACK_ALIGNMENT, &mut s.restore_pack_alignment);

            gl::GetBooleanv(gl::TEXTURE_CUBE_MAP_SEAMLESS, &mut b);
            s.restore_cube_map_seamless = b != 0;
        }
        s
    }

    /// Stub when opengl feature is disabled
    #[cfg(not(feature = "opengl"))]
    pub fn new() -> Self {
        Self::zeroed()
    }

    fn zeroed() -> Self {
        Self {
            restore_render_buffer: 0,
            restore_vao: 0,
            restore_depth_test: false,
            restore_depth_write_mask: true,
            restore_depth_func: 0x0201, // GL_LESS
            restore_depth_bias: false,
            restore_depth_bias_constant_factor: 0.0,
            restore_depth_bias_slope_factor: 0.0,
            restore_depth_clamp: false,
            depth_range: [0.0, 1.0],
            restore_stencil_test: false,
            restore_stencil_compare_fn: [0; 2],
            restore_stencil_reference_value: [0; 2],
            restore_stencil_read_mask: [0; 2],
            restore_stencil_write_mask: [0; 2],
            restore_stencil_fail: [0; 2],
            restore_stencil_depth_fail: [0; 2],
            restore_stencil_depth_pass: [0; 2],
            restore_viewport: [0; 4],
            restore_blend_enabled: false,
            restore_color_op: 0x8006, // GL_FUNC_ADD
            restore_alpha_op: 0x8006,
            restore_color_src_fn: 1, // GL_ONE
            restore_alpha_src_fn: 1,
            restore_color_dst_fn: 0, // GL_ZERO
            restore_alpha_dst_fn: 0,
            restore_blend_color: [0.0; 4],
            restore_alpha_to_coverage: false,
            restore_sample_alpha_to_one: false,
            line_width: 1.0,
            cull_face: false,
            cull_mode: 0x0405,  // GL_BACK
            front_face: 0x0901, // GL_CCW
            rasterizer_discard: false,
            restore_framebuffer_srgb: false,
            restore_multisample: false,
            restore_point_smooth: false,
            restore_point_sprite: false,
            restore_clip_distances: Vec::new(),
            restore_unpack_alignment: 4,
            restore_pack_alignment: 4,
            restore_cube_map_seamless: false,
        }
    }
}

impl Drop for HgiGLScopedStateHolder {
    /// Restore all saved GL state. Matches C++ destructor body.
    #[cfg(feature = "opengl")]
    fn drop(&mut self) {
        unsafe {
            // Depth
            Self::enable_disable(gl::DEPTH_TEST, self.restore_depth_test);
            gl::DepthMask(self.restore_depth_write_mask as u8);
            gl::DepthFunc(self.restore_depth_func as u32);

            Self::enable_disable(gl::POLYGON_OFFSET_FILL, self.restore_depth_bias);
            gl::PolygonOffset(
                self.restore_depth_bias_slope_factor,
                self.restore_depth_bias_constant_factor,
            );

            // Stencil
            Self::enable_disable(gl::STENCIL_TEST, self.restore_stencil_test);
            gl::StencilFuncSeparate(
                gl::FRONT,
                self.restore_stencil_compare_fn[0] as u32,
                self.restore_stencil_reference_value[0],
                self.restore_stencil_read_mask[0] as u32,
            );
            gl::StencilOpSeparate(
                gl::FRONT,
                self.restore_stencil_fail[0] as u32,
                self.restore_stencil_depth_fail[0] as u32,
                self.restore_stencil_depth_pass[0] as u32,
            );
            gl::StencilMaskSeparate(gl::FRONT, self.restore_stencil_write_mask[0] as u32);

            gl::StencilFuncSeparate(
                gl::BACK,
                self.restore_stencil_compare_fn[1] as u32,
                self.restore_stencil_reference_value[1],
                self.restore_stencil_read_mask[1] as u32,
            );
            gl::StencilOpSeparate(
                gl::BACK,
                self.restore_stencil_fail[1] as u32,
                self.restore_stencil_depth_fail[1] as u32,
                self.restore_stencil_depth_pass[1] as u32,
            );
            gl::StencilMaskSeparate(gl::BACK, self.restore_stencil_write_mask[1] as u32);

            // Alpha to coverage / sample alpha to one
            Self::enable_disable(gl::SAMPLE_ALPHA_TO_COVERAGE, self.restore_alpha_to_coverage);
            Self::enable_disable(gl::SAMPLE_ALPHA_TO_ONE, self.restore_sample_alpha_to_one);

            // Blend
            gl::BlendFuncSeparate(
                self.restore_color_src_fn as u32,
                self.restore_color_dst_fn as u32,
                self.restore_alpha_src_fn as u32,
                self.restore_alpha_dst_fn as u32,
            );
            gl::BlendEquationSeparate(self.restore_color_op as u32, self.restore_alpha_op as u32);
            gl::BlendColor(
                self.restore_blend_color[0],
                self.restore_blend_color[1],
                self.restore_blend_color[2],
                self.restore_blend_color[3],
            );
            Self::enable_disable(gl::BLEND, self.restore_blend_enabled);

            // Viewport / VAO / renderbuffer
            gl::Viewport(
                self.restore_viewport[0],
                self.restore_viewport[1],
                self.restore_viewport[2],
                self.restore_viewport[3],
            );
            gl::BindVertexArray(self.restore_vao as u32);
            gl::BindRenderbuffer(gl::RENDERBUFFER, self.restore_render_buffer as u32);

            // Raster
            gl::LineWidth(self.line_width);
            Self::enable_disable(gl::CULL_FACE, self.cull_face);
            gl::CullFace(self.cull_mode as u32);
            gl::FrontFace(self.front_face as u32);
            Self::enable_disable(gl::RASTERIZER_DISCARD, self.rasterizer_discard);
            Self::enable_disable(gl::DEPTH_CLAMP, self.restore_depth_clamp);
            gl::DepthRangef(self.depth_range[0], self.depth_range[1]);

            // Framebuffer sRGB
            Self::enable_disable(gl::FRAMEBUFFER_SRGB, self.restore_framebuffer_srgb);

            // Clip distances
            for (i, &enabled) in self.restore_clip_distances.iter().enumerate() {
                Self::enable_disable(gl::CLIP_DISTANCE0 + i as u32, enabled);
            }

            // Misc
            Self::enable_disable(gl::MULTISAMPLE, self.restore_multisample);
            Self::enable_disable(gl::POINT_SMOOTH, self.restore_point_smooth);
            Self::enable_disable(gl::POINT_SPRITE, self.restore_point_sprite);

            gl::PixelStorei(gl::UNPACK_ALIGNMENT, self.restore_unpack_alignment);
            gl::PixelStorei(gl::PACK_ALIGNMENT, self.restore_pack_alignment);

            Self::enable_disable(
                gl::TEXTURE_CUBE_MAP_SEAMLESS,
                self.restore_cube_map_seamless,
            );

            // Unbind all samplers (units 0-7) and the program.
            // Matches C++ `glBindSamplers(0, 8, zeros); glUseProgram(0);`
            let zeros: [u32; 8] = [0; 8];
            gl::BindSamplers(0, 8, zeros.as_ptr());
            gl::UseProgram(0);
        }
    }

    #[cfg(not(feature = "opengl"))]
    fn drop(&mut self) {}
}

impl HgiGLScopedStateHolder {
    #[cfg(feature = "opengl")]
    #[inline]
    unsafe fn enable_disable(cap: u32, enable: bool) {
        if enable {
            gl::Enable(cap);
        } else {
            gl::Disable(cap);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zeroed_defaults() {
        let s = HgiGLScopedStateHolder::zeroed();
        assert_eq!(s.line_width, 1.0);
        assert_eq!(s.depth_range, [0.0, 1.0]);
        assert!(!s.restore_depth_test);
        assert!(s.restore_depth_write_mask);
        assert!(!s.restore_blend_enabled);
    }
}
