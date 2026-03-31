
//! HdUnitTestDelegate - Simple scene delegate for unit tests.
//!
//! Corresponds to pxr/imaging/hd/unitTestDelegate.h.
//! Minimal implementation for tests that need a scene delegate.

use crate::HdInterpolation;
use crate::prim::HdSceneDelegate;
use crate::scene_delegate::{HdDisplayStyle, HdPrimvarDescriptor};
use crate::tokens;
use crate::types::HdDirtyBits;
use std::collections::HashMap;
use usd_sdf::Path as SdfPath;
use usd_tf::Token;

/// Simple delegate for unit tests.
///
/// Provides minimal HdSceneDelegate implementation.
pub struct HdUnitTestDelegate {
    _delegate_id: SdfPath,
    dirty_bits: HashMap<SdfPath, HdDirtyBits>,
    visibility: HashMap<SdfPath, bool>,
}

impl HdUnitTestDelegate {
    /// Create new unit test delegate.
    pub fn new(delegate_id: SdfPath) -> Self {
        Self {
            _delegate_id: delegate_id,
            dirty_bits: HashMap::new(),
            visibility: HashMap::new(),
        }
    }

    /// Set visibility for a prim.
    pub fn set_visibility(&mut self, id: SdfPath, vis: bool) {
        self.visibility.insert(id, vis);
    }

    /// Add a mesh prim (minimal - just registers the path).
    pub fn add_mesh(&mut self, id: SdfPath) {
        self.dirty_bits.insert(id.clone(), HdDirtyBits::MAX);
        self.visibility.insert(id, true);
    }
}

impl HdSceneDelegate for HdUnitTestDelegate {
    fn get_dirty_bits(&self, id: &SdfPath) -> HdDirtyBits {
        self.dirty_bits.get(id).copied().unwrap_or(0)
    }

    fn mark_clean(&mut self, id: &SdfPath, bits: HdDirtyBits) {
        if let Some(b) = self.dirty_bits.get_mut(id) {
            *b &= !bits;
        }
    }

    fn get_instancer_id(&self, _prim_id: &SdfPath) -> SdfPath {
        SdfPath::default()
    }

    fn get_visible(&self, id: &SdfPath) -> bool {
        self.visibility.get(id).copied().unwrap_or(true)
    }

    fn get_display_style(&self, _id: &SdfPath) -> HdDisplayStyle {
        HdDisplayStyle::default()
    }

    fn get_primvar_descriptors(
        &self,
        _id: &SdfPath,
        _interp: HdInterpolation,
    ) -> Vec<HdPrimvarDescriptor> {
        vec![
            HdPrimvarDescriptor {
                name: tokens::DISPLAY_COLOR.clone(),
                interpolation: HdInterpolation::Varying,
                role: Token::default(),
                indexed: false,
            },
            HdPrimvarDescriptor {
                name: tokens::POINTS.clone(),
                interpolation: HdInterpolation::Vertex,
                role: Token::default(),
                indexed: false,
            },
        ]
    }
}
