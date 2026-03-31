#![allow(dead_code)]

//! HdStSamplerObject - GPU sampler objects for Storm textures.
//!
//! Each sampler object type mirrors a texture object type and wraps
//! one or more HGI sampler handles with the appropriate filtering
//! and wrap mode configuration.
//!
//! Sampler types:
//! - UvSampler: For HdStUvTextureObject (single sampler)
//! - FieldSampler: For HdStFieldTextureObject (single sampler)
//! - PtexSampler: For HdStPtexTextureObject (texels + layout samplers)
//! - UdimSampler: For HdStUdimTextureObject (texels + layout samplers)
//! - CubemapSampler: For HdStCubemapTextureObject (single sampler)
//!
//! Port of pxr/imaging/hdSt/samplerObject.h

use std::sync::Arc;
use usd_hd::types::HdSamplerParameters;
use usd_hgi::{HgiSamplerDesc, HgiSamplerHandle};

/// Trait for all Storm sampler objects.
pub trait HdStSamplerObjectTrait: std::fmt::Debug + Send + Sync {
    /// Get the primary sampler handle.
    fn sampler(&self) -> &HgiSamplerHandle;

    /// Get the sampler parameters used to create this sampler.
    fn sampler_params(&self) -> &HdSamplerParameters;
}

// ---------------------------------------------------------------------------
// HdStUvSamplerObject
// ---------------------------------------------------------------------------

/// Sampler for UV (2D) textures.
///
/// Wraps a single HGI sampler with wrap/filter modes from HdSamplerParameters.
///
/// Port of HdStUvSamplerObject
#[derive(Debug, Clone)]
pub struct HdStUvSamplerObject {
    sampler: HgiSamplerHandle,
    params: HdSamplerParameters,
}

impl HdStUvSamplerObject {
    /// Create a UV sampler from parameters.
    pub fn new(params: HdSamplerParameters) -> Self {
        Self {
            sampler: HgiSamplerHandle::default(),
            params,
        }
    }

    /// Create with an existing HGI sampler handle.
    pub fn with_handle(sampler: HgiSamplerHandle, params: HdSamplerParameters) -> Self {
        Self { sampler, params }
    }

    /// Build an HGI sampler descriptor from the current parameters.
    pub fn to_sampler_desc(&self) -> HgiSamplerDesc {
        sampler_params_to_desc(&self.params)
    }
}

impl HdStSamplerObjectTrait for HdStUvSamplerObject {
    fn sampler(&self) -> &HgiSamplerHandle {
        &self.sampler
    }
    fn sampler_params(&self) -> &HdSamplerParameters {
        &self.params
    }
}

// ---------------------------------------------------------------------------
// HdStFieldSamplerObject
// ---------------------------------------------------------------------------

/// Sampler for field (3D volume) textures.
#[derive(Debug, Clone)]
pub struct HdStFieldSamplerObject {
    sampler: HgiSamplerHandle,
    params: HdSamplerParameters,
}

impl HdStFieldSamplerObject {
    pub fn new(params: HdSamplerParameters) -> Self {
        Self {
            sampler: HgiSamplerHandle::default(),
            params,
        }
    }

    pub fn with_handle(sampler: HgiSamplerHandle, params: HdSamplerParameters) -> Self {
        Self { sampler, params }
    }
}

impl HdStSamplerObjectTrait for HdStFieldSamplerObject {
    fn sampler(&self) -> &HgiSamplerHandle {
        &self.sampler
    }
    fn sampler_params(&self) -> &HdSamplerParameters {
        &self.params
    }
}

// ---------------------------------------------------------------------------
// HdStPtexSamplerObject
// ---------------------------------------------------------------------------

/// Sampler for Ptex textures (dual: texels + layout).
#[derive(Debug, Clone)]
pub struct HdStPtexSamplerObject {
    texels_sampler: HgiSamplerHandle,
    layout_sampler: HgiSamplerHandle,
    params: HdSamplerParameters,
}

impl HdStPtexSamplerObject {
    pub fn new(params: HdSamplerParameters) -> Self {
        Self {
            texels_sampler: HgiSamplerHandle::default(),
            layout_sampler: HgiSamplerHandle::default(),
            params,
        }
    }

    pub fn with_handles(
        texels: HgiSamplerHandle,
        layout: HgiSamplerHandle,
        params: HdSamplerParameters,
    ) -> Self {
        Self {
            texels_sampler: texels,
            layout_sampler: layout,
            params,
        }
    }

    pub fn texels_sampler(&self) -> &HgiSamplerHandle {
        &self.texels_sampler
    }
    pub fn layout_sampler(&self) -> &HgiSamplerHandle {
        &self.layout_sampler
    }
}

impl HdStSamplerObjectTrait for HdStPtexSamplerObject {
    fn sampler(&self) -> &HgiSamplerHandle {
        &self.texels_sampler
    }
    fn sampler_params(&self) -> &HdSamplerParameters {
        &self.params
    }
}

// ---------------------------------------------------------------------------
// HdStUdimSamplerObject
// ---------------------------------------------------------------------------

/// Sampler for UDIM textures (dual: texels + layout).
#[derive(Debug, Clone)]
pub struct HdStUdimSamplerObject {
    texels_sampler: HgiSamplerHandle,
    layout_sampler: HgiSamplerHandle,
    params: HdSamplerParameters,
}

impl HdStUdimSamplerObject {
    pub fn new(params: HdSamplerParameters) -> Self {
        Self {
            texels_sampler: HgiSamplerHandle::default(),
            layout_sampler: HgiSamplerHandle::default(),
            params,
        }
    }

    pub fn with_handles(
        texels: HgiSamplerHandle,
        layout: HgiSamplerHandle,
        params: HdSamplerParameters,
    ) -> Self {
        Self {
            texels_sampler: texels,
            layout_sampler: layout,
            params,
        }
    }

    pub fn texels_sampler(&self) -> &HgiSamplerHandle {
        &self.texels_sampler
    }
    pub fn layout_sampler(&self) -> &HgiSamplerHandle {
        &self.layout_sampler
    }
}

impl HdStSamplerObjectTrait for HdStUdimSamplerObject {
    fn sampler(&self) -> &HgiSamplerHandle {
        &self.texels_sampler
    }
    fn sampler_params(&self) -> &HdSamplerParameters {
        &self.params
    }
}

// ---------------------------------------------------------------------------
// HdStCubemapSamplerObject
// ---------------------------------------------------------------------------

/// Sampler for cubemap textures. Uses seamless cubemap sampling.
#[derive(Debug, Clone)]
pub struct HdStCubemapSamplerObject {
    sampler: HgiSamplerHandle,
    params: HdSamplerParameters,
}

impl HdStCubemapSamplerObject {
    pub fn new(params: HdSamplerParameters) -> Self {
        Self {
            sampler: HgiSamplerHandle::default(),
            params,
        }
    }

    pub fn with_handle(sampler: HgiSamplerHandle, params: HdSamplerParameters) -> Self {
        Self { sampler, params }
    }
}

impl HdStSamplerObjectTrait for HdStCubemapSamplerObject {
    fn sampler(&self) -> &HgiSamplerHandle {
        &self.sampler
    }
    fn sampler_params(&self) -> &HdSamplerParameters {
        &self.params
    }
}

// ---------------------------------------------------------------------------
// Enum wrapper
// ---------------------------------------------------------------------------

/// Sampler object enum wrapping all sampler type variants.
#[derive(Debug, Clone)]
pub enum HdStSamplerObject {
    Uv(HdStUvSamplerObject),
    Field(HdStFieldSamplerObject),
    Ptex(HdStPtexSamplerObject),
    Udim(HdStUdimSamplerObject),
    Cubemap(HdStCubemapSamplerObject),
}

impl HdStSamplerObject {
    pub fn sampler(&self) -> &HgiSamplerHandle {
        match self {
            Self::Uv(s) => s.sampler(),
            Self::Field(s) => s.sampler(),
            Self::Ptex(s) => s.sampler(),
            Self::Udim(s) => s.sampler(),
            Self::Cubemap(s) => s.sampler(),
        }
    }

    pub fn sampler_params(&self) -> &HdSamplerParameters {
        match self {
            Self::Uv(s) => s.sampler_params(),
            Self::Field(s) => s.sampler_params(),
            Self::Ptex(s) => s.sampler_params(),
            Self::Udim(s) => s.sampler_params(),
            Self::Cubemap(s) => s.sampler_params(),
        }
    }

    pub fn as_ptex(&self) -> Option<&HdStPtexSamplerObject> {
        match self {
            Self::Ptex(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_udim(&self) -> Option<&HdStUdimSamplerObject> {
        match self {
            Self::Udim(s) => Some(s),
            _ => None,
        }
    }
}

pub type HdStSamplerObjectSharedPtr = Arc<HdStSamplerObject>;

// ---------------------------------------------------------------------------
// Helper: HdSamplerParameters -> HgiSamplerDesc conversion
// ---------------------------------------------------------------------------

use usd_hd::enums::{HdCompareFunction, HdMagFilter, HdMinFilter, HdWrap};
use usd_hgi::{
    HgiBorderColor, HgiCompareFunction, HgiMipFilter, HgiSamplerAddressMode, HgiSamplerFilter,
};

/// Convert HdWrap to HgiSamplerAddressMode.
pub fn wrap_to_address_mode(wrap: HdWrap) -> HgiSamplerAddressMode {
    match wrap {
        HdWrap::Clamp => HgiSamplerAddressMode::ClampToEdge,
        HdWrap::Repeat => HgiSamplerAddressMode::Repeat,
        HdWrap::Mirror => HgiSamplerAddressMode::MirrorRepeat,
        HdWrap::Black => HgiSamplerAddressMode::ClampToBorderColor,
        HdWrap::NoOpinion | HdWrap::LegacyNoOpinionFallbackRepeat => {
            HgiSamplerAddressMode::ClampToEdge
        }
    }
}

/// Convert HdMinFilter to HgiSamplerFilter + HgiMipFilter.
pub fn min_filter_to_hgi(filter: HdMinFilter) -> (HgiSamplerFilter, HgiMipFilter) {
    match filter {
        HdMinFilter::Nearest => (HgiSamplerFilter::Nearest, HgiMipFilter::NotMipmapped),
        HdMinFilter::Linear => (HgiSamplerFilter::Linear, HgiMipFilter::NotMipmapped),
        HdMinFilter::NearestMipmapNearest => (HgiSamplerFilter::Nearest, HgiMipFilter::Nearest),
        HdMinFilter::NearestMipmapLinear => (HgiSamplerFilter::Nearest, HgiMipFilter::Linear),
        HdMinFilter::LinearMipmapNearest => (HgiSamplerFilter::Linear, HgiMipFilter::Nearest),
        HdMinFilter::LinearMipmapLinear => (HgiSamplerFilter::Linear, HgiMipFilter::Linear),
    }
}

/// Convert HdMagFilter to HgiSamplerFilter.
pub fn mag_filter_to_hgi(filter: HdMagFilter) -> HgiSamplerFilter {
    match filter {
        HdMagFilter::Nearest => HgiSamplerFilter::Nearest,
        HdMagFilter::Linear => HgiSamplerFilter::Linear,
    }
}

/// Convert HdCompareFunction to HgiCompareFunction.
pub fn compare_fn_to_hgi(func: HdCompareFunction) -> HgiCompareFunction {
    match func {
        HdCompareFunction::Never => HgiCompareFunction::Never,
        HdCompareFunction::Less => HgiCompareFunction::Less,
        HdCompareFunction::Equal => HgiCompareFunction::Equal,
        HdCompareFunction::LEqual => HgiCompareFunction::LEqual,
        HdCompareFunction::Greater => HgiCompareFunction::Greater,
        HdCompareFunction::NotEqual => HgiCompareFunction::NotEqual,
        HdCompareFunction::GEqual => HgiCompareFunction::GEqual,
        HdCompareFunction::Always => HgiCompareFunction::Always,
    }
}

/// Build HgiSamplerDesc from HdSamplerParameters.
pub fn sampler_params_to_desc(params: &HdSamplerParameters) -> HgiSamplerDesc {
    let (min_filter, mip_filter) = min_filter_to_hgi(params.min_filter);

    HgiSamplerDesc {
        debug_name: String::new(),
        mag_filter: mag_filter_to_hgi(params.mag_filter),
        min_filter,
        mip_filter,
        address_mode_u: wrap_to_address_mode(params.wrap_s),
        address_mode_v: wrap_to_address_mode(params.wrap_t),
        address_mode_w: wrap_to_address_mode(params.wrap_r),
        border_color: HgiBorderColor::TransparentBlack,
        max_anisotropy: params.max_anisotropy as u32,
        enable_compare: params.enable_compare,
        compare_function: compare_fn_to_hgi(params.compare_function),
        ..HgiSamplerDesc::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uv_sampler() {
        let params = HdSamplerParameters::default();
        let sampler = HdStUvSamplerObject::new(params.clone());
        assert_eq!(sampler.sampler_params(), &params);
    }

    #[test]
    fn test_ptex_dual_sampler() {
        let params = HdSamplerParameters::default();
        let sampler = HdStPtexSamplerObject::new(params);
        assert!(!sampler.texels_sampler().is_valid());
        assert!(!sampler.layout_sampler().is_valid());
    }

    #[test]
    fn test_wrap_conversion() {
        assert_eq!(
            wrap_to_address_mode(HdWrap::Repeat),
            HgiSamplerAddressMode::Repeat
        );
        assert_eq!(
            wrap_to_address_mode(HdWrap::Clamp),
            HgiSamplerAddressMode::ClampToEdge
        );
        assert_eq!(
            wrap_to_address_mode(HdWrap::Mirror),
            HgiSamplerAddressMode::MirrorRepeat
        );
        assert_eq!(
            wrap_to_address_mode(HdWrap::Black),
            HgiSamplerAddressMode::ClampToBorderColor
        );
    }

    #[test]
    fn test_sampler_desc_conversion() {
        let params = HdSamplerParameters::default();
        let desc = sampler_params_to_desc(&params);
        assert_eq!(desc.min_filter, HgiSamplerFilter::Nearest);
        assert_eq!(desc.mag_filter, HgiSamplerFilter::Nearest);
        assert_eq!(desc.address_mode_u, HgiSamplerAddressMode::Repeat);
    }

    #[test]
    fn test_enum_dispatch() {
        let params = HdSamplerParameters::default();
        let obj = HdStSamplerObject::Uv(HdStUvSamplerObject::new(params.clone()));
        assert_eq!(obj.sampler_params(), &params);
        assert!(obj.as_ptex().is_none());
    }
}
