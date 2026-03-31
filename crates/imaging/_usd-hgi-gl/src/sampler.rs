//! OpenGL sampler implementation

#[cfg(feature = "opengl")]
use super::conversions::*;
use usd_hgi::*;

/// OpenGL sampler object
///
/// Wraps an OpenGL sampler object that encapsulates texture sampling parameters
#[derive(Debug)]
pub struct HgiGLSampler {
    /// OpenGL sampler object name
    gl_id: u32,

    /// Sampler descriptor
    desc: HgiSamplerDesc,
}

impl HgiGLSampler {
    /// Create a new OpenGL sampler
    pub fn new(desc: &HgiSamplerDesc) -> Self {
        let gl_id = Self::create_gl_sampler(desc);

        Self {
            gl_id,
            desc: desc.clone(),
        }
    }

    /// Create OpenGL sampler object
    #[cfg(feature = "opengl")]
    fn create_gl_sampler(desc: &HgiSamplerDesc) -> u32 {
        use gl::types::*;

        let mut sampler_id: GLuint = 0;

        unsafe {
            gl::CreateSamplers(1, &mut sampler_id);

            if sampler_id == 0 {
                log::error!("Failed to create OpenGL sampler");
                return 0;
            }

            // Set minification filter (combines base filter with mip filter)
            let min_filter = hgi_sampler_filter_to_gl_min_filter(desc.min_filter, desc.mip_filter);
            gl::SamplerParameteri(sampler_id, gl::TEXTURE_MIN_FILTER, min_filter as GLint);

            // Set magnification filter
            let mag_filter = hgi_sampler_filter_to_gl_mag_filter(desc.mag_filter);
            gl::SamplerParameteri(sampler_id, gl::TEXTURE_MAG_FILTER, mag_filter as GLint);

            // Set wrap/address modes
            let wrap_s = hgi_address_mode_to_gl_wrap(desc.address_mode_u);
            let wrap_t = hgi_address_mode_to_gl_wrap(desc.address_mode_v);
            let wrap_r = hgi_address_mode_to_gl_wrap(desc.address_mode_w);
            gl::SamplerParameteri(sampler_id, gl::TEXTURE_WRAP_S, wrap_s as GLint);
            gl::SamplerParameteri(sampler_id, gl::TEXTURE_WRAP_T, wrap_t as GLint);
            gl::SamplerParameteri(sampler_id, gl::TEXTURE_WRAP_R, wrap_r as GLint);

            // Set border color if using clamp to border
            if desc.address_mode_u == HgiSamplerAddressMode::ClampToBorderColor
                || desc.address_mode_v == HgiSamplerAddressMode::ClampToBorderColor
                || desc.address_mode_w == HgiSamplerAddressMode::ClampToBorderColor
            {
                let border = hgi_border_color_to_gl(desc.border_color);
                gl::SamplerParameterfv(sampler_id, gl::TEXTURE_BORDER_COLOR, border.as_ptr());
            }

            // Set LOD parameters
            gl::SamplerParameterf(sampler_id, gl::TEXTURE_MIN_LOD, desc.min_lod);
            gl::SamplerParameterf(sampler_id, gl::TEXTURE_MAX_LOD, desc.max_lod);

            // Set comparison mode if enabled (for shadow mapping)
            if desc.enable_compare {
                gl::SamplerParameteri(
                    sampler_id,
                    gl::TEXTURE_COMPARE_MODE,
                    gl::COMPARE_REF_TO_TEXTURE as GLint,
                );
                let compare_func = hgi_compare_func_to_gl(desc.compare_function);
                gl::SamplerParameteri(sampler_id, gl::TEXTURE_COMPARE_FUNC, compare_func as GLint);
            }

            // Set debug label if provided
            if !desc.debug_name.is_empty() {
                gl::ObjectLabel(
                    gl::SAMPLER,
                    sampler_id,
                    desc.debug_name.len() as GLsizei,
                    desc.debug_name.as_ptr() as *const GLchar,
                );
            }
        }

        sampler_id
    }

    #[cfg(not(feature = "opengl"))]
    fn create_gl_sampler(_desc: &HgiSamplerDesc) -> u32 {
        0
    }

    /// Get the OpenGL sampler object name
    pub fn gl_id(&self) -> u32 {
        self.gl_id
    }

    /// Get the sampler descriptor
    pub fn descriptor(&self) -> &HgiSamplerDesc {
        &self.desc
    }
}

impl HgiSampler for HgiGLSampler {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn descriptor(&self) -> &HgiSamplerDesc {
        &self.desc
    }

    fn raw_resource(&self) -> u64 {
        self.gl_id as u64
    }
}

impl Drop for HgiGLSampler {
    #[cfg(feature = "opengl")]
    fn drop(&mut self) {
        if self.gl_id != 0 {
            unsafe {
                gl::DeleteSamplers(1, &self.gl_id);
            }
        }
    }

    #[cfg(not(feature = "opengl"))]
    fn drop(&mut self) {}
}

/// Bind sampler to a texture unit
#[cfg(feature = "opengl")]
pub fn bind_sampler(sampler: &HgiGLSampler, unit: u32) {
    if sampler.gl_id() != 0 {
        unsafe {
            gl::BindSampler(unit, sampler.gl_id());
        }
    }
}

/// Bind sampler to a texture unit (stub when opengl feature disabled)
#[cfg(not(feature = "opengl"))]
pub fn bind_sampler(_sampler: &HgiGLSampler, _unit: u32) {}

/// Unbind sampler from a texture unit
#[cfg(feature = "opengl")]
pub fn unbind_sampler(unit: u32) {
    unsafe {
        gl::BindSampler(unit, 0);
    }
}

/// Unbind sampler from a texture unit (stub when opengl feature disabled)
#[cfg(not(feature = "opengl"))]
pub fn unbind_sampler(_unit: u32) {}

#[cfg(all(test, feature = "opengl"))]
pub(crate) fn run_gl_tests() {
    use super::*;

    let desc = HgiSamplerDesc::new()
        .with_mag_filter(HgiSamplerFilter::Linear)
        .with_min_filter(HgiSamplerFilter::Linear)
        .with_mip_filter(HgiMipFilter::Linear)
        .with_address_mode_u(HgiSamplerAddressMode::Repeat)
        .with_address_mode_v(HgiSamplerAddressMode::Repeat)
        .with_address_mode_w(HgiSamplerAddressMode::ClampToEdge);

    let sampler = HgiGLSampler::new(&desc);
    assert_eq!(sampler.descriptor().mag_filter, HgiSamplerFilter::Linear);
    assert_eq!(
        sampler.descriptor().address_mode_u,
        HgiSamplerAddressMode::Repeat
    );

    let desc = HgiSamplerDesc::new().with_compare(HgiCompareFunction::Less);

    let sampler = HgiGLSampler::new(&desc);
    assert!(sampler.descriptor().enable_compare);
    assert_eq!(
        sampler.descriptor().compare_function,
        HgiCompareFunction::Less
    );
}
