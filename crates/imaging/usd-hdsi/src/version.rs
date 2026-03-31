
//! HDSI API version.
//!
//! Port of pxr/imaging/hdsi/version.h
//!
//! Version history:
//! - 10 -> 11: Adding HdsiPrimManagingSceneIndexObserver and HdsiPrimTypeNoticeBatchingSceneIndex
//! - 11 -> 12: Adding HdsiPrimManagingSceneIndexObserver::GetTypedPrim
//! - 12 -> 13: Adding HdsiLightLinkingSceneIndex
//! - 13 -> 14: Add utilities for evaluating expressions on pruning collections
//! - 14 -> 15: Fix VelocityMotionResolvingSceneIndex's handling of instance scales
//! - 15 -> 16: Introducing HdsiDomeLightCameraVisibilitySceneIndex
//! - 16 -> 17: Introducing HdsiMaterialRenderContextFilteringSceneIndex
//! - 17 -> 18: Introducing ComposeFn in HdsiMaterialPrimvarTransferSceneIndex

/// HDSI API version constant.
pub const HDSI_API_VERSION: u32 = 18;
