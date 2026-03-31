
//! HdDrawItem - Lightweight representation of rprim resources for rendering.
//!
//! Corresponds to pxr/imaging/hd/drawItem.h.
//! Created by HdRprim for each HdRepr; consumed by HdRenderPass.

use super::drawing_coord::HdDrawingCoord;
use super::rprim_shared_data::HdRprimSharedData;
use std::sync::Arc;
use usd_gf::{BBox3d, Matrix4d, Range3d};
use usd_sdf::Path as SdfPath;
use usd_tf::Token;

/// Trait for draw items (base + backend specializations like HdStDrawItem).
///
/// C++ uses inheritance; Rust uses trait for polymorphism.
pub trait HdDrawItemTrait: Send + Sync {
    /// Get the rprim ID.
    fn get_rprim_id(&self) -> &SdfPath;
    /// Get bounds.
    fn get_bounds(&self) -> &BBox3d;
    /// Get extent (axis-aligned range).
    fn get_extent(&self) -> &Range3d;
    /// Get transform matrix.
    fn get_matrix(&self) -> &Matrix4d;
    /// Get authored visibility.
    fn get_visible(&self) -> bool;
    /// Get material tag.
    fn get_material_tag(&self) -> &Token;
}

/// Lightweight representation of an HdRprim's resources for rendering.
///
/// Corresponds to C++ `HdDrawItem`.
/// A repr may have multiple draw items. Backends may extend/specialize (e.g. HdStDrawItem).
pub struct HdDrawItem {
    drawing_coord: HdDrawingCoord,
    shared_data: Arc<HdRprimSharedData>,
    material_tag: Token,
}

impl HdDrawItem {
    /// Create a new draw item with shared data.
    pub fn new(shared_data: Arc<HdRprimSharedData>) -> Self {
        Self {
            drawing_coord: HdDrawingCoord::new(),
            shared_data,
            material_tag: Token::default(),
        }
    }

    /// Get the rprim ID.
    pub fn get_rprim_id(&self) -> &SdfPath {
        &self.shared_data.rprim_id
    }

    /// Get bounds.
    pub fn get_bounds(&self) -> &BBox3d {
        &self.shared_data.bounds
    }

    /// Get extent (axis-aligned range from bounds).
    pub fn get_extent(&self) -> &Range3d {
        self.shared_data.bounds.range()
    }

    /// Get transform matrix.
    pub fn get_matrix(&self) -> &Matrix4d {
        self.shared_data.bounds.matrix()
    }

    /// Get drawing coordinate.
    pub fn get_drawing_coord(&mut self) -> &mut HdDrawingCoord {
        &mut self.drawing_coord
    }

    /// Get drawing coordinate (const).
    pub fn get_drawing_coord_ref(&self) -> &HdDrawingCoord {
        &self.drawing_coord
    }

    /// Get authored visibility.
    pub fn get_visible(&self) -> bool {
        self.shared_data.visible
    }

    /// Get material tag.
    pub fn get_material_tag(&self) -> &Token {
        &self.material_tag
    }

    /// Set material tag.
    pub fn set_material_tag(&mut self, tag: Token) {
        self.material_tag = tag;
    }

    /// Get shared data (for backend use).
    pub fn get_shared_data(&self) -> &Arc<HdRprimSharedData> {
        &self.shared_data
    }
}

impl HdDrawItemTrait for HdDrawItem {
    fn get_rprim_id(&self) -> &SdfPath {
        &self.shared_data.rprim_id
    }
    fn get_bounds(&self) -> &BBox3d {
        &self.shared_data.bounds
    }
    fn get_extent(&self) -> &Range3d {
        self.shared_data.bounds.range()
    }
    fn get_matrix(&self) -> &Matrix4d {
        self.shared_data.bounds.matrix()
    }
    fn get_visible(&self) -> bool {
        self.shared_data.visible
    }
    fn get_material_tag(&self) -> &Token {
        &self.material_tag
    }
}

impl std::fmt::Debug for HdDrawItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HdDrawItem")
            .field("rprim_id", &self.shared_data.rprim_id)
            .field("material_tag", &self.material_tag)
            .finish()
    }
}
