//! GprimAdapter - Base adapter for USD geometry primitives.
//!
//! Port of pxr/usdImaging/usdImaging/gprimAdapter.h/cpp
//!
//! Provides base functionality for all geometry primitive adapters including:
//! - Transform handling
//! - Visibility tracking
//! - Material binding
//! - Primvar processing
//! - Extent computation

use super::data_source_gprim::DataSourceGprim;
use super::data_source_stage_globals::DataSourceStageGlobalsHandle;
use super::prim_adapter::PrimAdapter;
use super::types::PropertyInvalidationType;
use std::sync::Arc;
use usd_core::Prim;
use usd_geom::gprim::Gprim;
use usd_gf::{Matrix4d, Range3d, Vec3d, Vec3f};
use usd_hd::change_tracker::HdRprimDirtyBits;
use usd_hd::types::HdDirtyBits;
use usd_hd::{HdContainerDataSourceHandle, HdDataSourceLocatorSet};
use usd_sdf::TimeCode as SdfTimeCode;
use usd_tf::Token;

// Token constants for attribute names
mod tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    #[allow(dead_code)] // For future primvar name checking
    pub static POINTS: LazyLock<Token> = LazyLock::new(|| Token::new("points"));
    #[allow(dead_code)] // For future primvar name checking
    pub static NORMALS: LazyLock<Token> = LazyLock::new(|| Token::new("normals"));
    #[allow(dead_code)] // Used by invalidation, not by get_color() anymore
    pub static DISPLAY_COLOR: LazyLock<Token> = LazyLock::new(|| Token::new("displayColor"));
    #[allow(dead_code)] // Used by invalidation, not by get_opacity() anymore
    pub static DISPLAY_OPACITY: LazyLock<Token> = LazyLock::new(|| Token::new("displayOpacity"));
    #[allow(dead_code)] // Used by invalidation, not by get_extent() anymore
    pub static EXTENT: LazyLock<Token> = LazyLock::new(|| Token::new("extent"));
    #[allow(dead_code)] // For future visibility handling
    pub static VISIBILITY: LazyLock<Token> = LazyLock::new(|| Token::new("visibility"));
    #[allow(dead_code)] // For future purpose handling
    pub static PURPOSE: LazyLock<Token> = LazyLock::new(|| Token::new("purpose"));
    #[allow(dead_code)] // Used by invalidation, not by get_double_sided() anymore
    pub static DOUBLE_SIDED: LazyLock<Token> = LazyLock::new(|| Token::new("doubleSided"));
}

/// Base adapter for USD geometry primitives (Gprim).
///
/// This adapter provides common functionality for all geometric types:
/// - Transform inheritance and caching
/// - Visibility state
/// - Material binding resolution
/// - Display color/opacity handling
/// - Extent (bounding box) computation
/// - Double-sided rendering flag
///
/// Derived adapters (MeshAdapter, BasisCurvesAdapter, etc.) add type-specific
/// functionality.
///
/// # Example
///
/// ```ignore
/// use usd_imaging::GprimAdapter;
///
/// // GprimAdapter is typically used as a base for specific adapters:
/// struct MyMeshAdapter {
///     base: GprimAdapter,
/// }
/// ```
#[derive(Debug, Clone)]
pub struct GprimAdapter {
    /// Cached prim type for this adapter
    prim_type: Token,
}

impl GprimAdapter {
    /// Create a new gprim adapter for the given prim type.
    pub fn new(prim_type: Token) -> Self {
        Self { prim_type }
    }

    /// Get display color for a prim.
    ///
    /// Precedence: material binding color > local displayColor primvar.
    /// Returns (color_values, interpolation, optional_indices).
    ///
    /// Matches C++ `UsdImagingGprimAdapter::GetColor()`.
    pub fn get_color(
        prim: &Prim,
        time: usd_core::TimeCode,
    ) -> Option<(Vec<Vec3f>, Token, Option<Vec<i32>>)> {
        let sdf_time = usd_sdf::TimeCode::new(time.value());
        let mut result = vec![Vec3f::new(0.5, 0.5, 0.5)];
        let mut color_indices: Option<Vec<i32>> = None;
        let mut has_authored = false;

        // Read displayColor primvar from geometry prim.
        // C++ UsdImagingGprimAdapter::GetColor() reads from UsdGeomGprim::GetDisplayColorPrimvar(),
        // not from the bound material prim.
        let gprim = Gprim::new(prim.clone());
        let primvar = gprim.get_display_color_primvar();
        let color_interp = primvar.get_interpolation();

        if let Some(val) = primvar.compute_flattened(sdf_time) {
            if let Some(arr) = val.get::<Vec<Vec3f>>() {
                has_authored = true;
                result = arr.clone();
                // Truncate to 1 element if constant with multiple values
                if color_interp == "constant" && result.len() > 1 {
                    log::warn!(
                        "Prim {} has {} elements for displayColor marked constant",
                        prim.path().as_str(),
                        result.len()
                    );
                    result.truncate(1);
                }
            }
        } else if primvar.has_authored_value() {
            // Authored but ComputeFlattened returned None => empty array
            has_authored = true;
            result = Vec::new();
        }

        // Optionally read indices (non-flattened path)
        if has_authored {
            color_indices = primvar.get_indices(sdf_time);
        }

        if !has_authored {
            return None;
        }

        Some((result, color_interp, color_indices))
    }

    /// Get display opacity for a prim.
    ///
    /// Reads the displayOpacity primvar, returning the opacity values
    /// with interpolation mode.
    ///
    /// Matches C++ `UsdImagingGprimAdapter::GetOpacity()`.
    pub fn get_opacity(
        prim: &Prim,
        time: usd_core::TimeCode,
    ) -> Option<(Vec<f32>, Token, Option<Vec<i32>>)> {
        let sdf_time = usd_sdf::TimeCode::new(time.value());
        let mut result = vec![1.0f32];
        let mut opacity_indices: Option<Vec<i32>> = None;
        let mut has_authored = false;

        let gprim = Gprim::new(prim.clone());
        let primvar = gprim.get_display_opacity_primvar();
        let opacity_interp = primvar.get_interpolation();

        if let Some(val) = primvar.compute_flattened(sdf_time) {
            if let Some(arr) = val.get::<Vec<f32>>() {
                has_authored = true;
                result = arr.clone();
                if opacity_interp == "constant" && result.len() > 1 {
                    result.truncate(1);
                }
            }
        } else if primvar.has_authored_value() {
            has_authored = true;
            result = Vec::new();
        }

        if has_authored {
            opacity_indices = primvar.get_indices(sdf_time);
        }

        if !has_authored {
            return None;
        }

        Some((result, opacity_interp, opacity_indices))
    }

    /// Get extent (bounding box) for a prim.
    ///
    /// Reads the "extent" attribute (Vec3f[2]) and converts to Range3d.
    /// Returns an empty range if not authored or wrong element count.
    ///
    /// Matches C++ `UsdImagingGprimAdapter::GetExtent()`.
    pub fn get_extent(prim: &Prim, time: usd_core::TimeCode) -> Range3d {
        let gprim = Gprim::new(prim.clone());
        let attr = gprim.boundable().get_extent_attr();
        let sdf_time = usd_sdf::TimeCode::new(time.value());
        if let Some(extent) = attr.get_typed_vec::<Vec3f>(sdf_time) {
            if extent.len() == 2 {
                // USD stores extent as 2 float vecs; implicit f32->f64 conversion
                let min = Vec3d::new(extent[0].x as f64, extent[0].y as f64, extent[0].z as f64);
                let max = Vec3d::new(extent[1].x as f64, extent[1].y as f64, extent[1].z as f64);
                return Range3d::new(min, max);
            }
        }
        Range3d::default()
    }

    /// Get whether the prim is double-sided.
    ///
    /// Reads the "doubleSided" attribute and returns its value.
    /// Defaults to false if not authored.
    ///
    /// Matches C++ `UsdImagingGprimAdapter::GetDoubleSided()`.
    pub fn get_double_sided(prim: &Prim, time: usd_core::TimeCode) -> bool {
        let gprim = Gprim::new(prim.clone());
        let attr = gprim.get_double_sided_attr();
        let sdf_time = usd_sdf::TimeCode::new(time.value());
        attr.get_typed::<bool>(sdf_time).unwrap_or(false)
    }

    /// Get the basis matrix for implicit primitives.
    ///
    /// For implicit prims (capsule, cone, cylinder, plane), the spine axis
    /// may be specified. This returns a basis matrix that transforms points
    /// generated using "Z" as the spine axis to the desired axis.
    pub fn get_implicit_basis(spine_axis: &Token) -> Matrix4d {
        match spine_axis.as_str() {
            "X" => {
                // X-spine: u=Y, v=Z, spine=X  (matches C++ GetImplicitBasis: SetRow(0,Y), SetRow(1,Z), SetRow(2,X))
                // Row0=(0,1,0,0), Row1=(0,0,1,0), Row2=(1,0,0,0), Row3=(0,0,0,1)
                Matrix4d::new(
                    0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0,
                )
            }
            "Y" => {
                // Y-spine: u=Z, v=X, spine=Y  (matches C++ GetImplicitBasis: SetRow(0,Z), SetRow(1,X), SetRow(2,Y))
                // Row0=(0,0,1,0), Row1=(1,0,0,0), Row2=(0,1,0,0), Row3=(0,0,0,1)
                Matrix4d::new(
                    0.0, 0.0, 1.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0,
                )
            }
            _ => {
                // "Z" or default - identity matrix
                Matrix4d::identity()
            }
        }
    }

    /// Create the data source for this gprim.
    fn create_data_source(
        &self,
        prim: &Prim,
        stage_globals: &DataSourceStageGlobalsHandle,
    ) -> HdContainerDataSourceHandle {
        DataSourceGprim::new(prim.path().clone(), prim.clone(), stage_globals.clone())
    }
}

impl GprimAdapter {
    /// Compute time-varying dirty bits for a gprim (base implementation).
    /// Matches C++ UsdImagingGprimAdapter::TrackVariability.
    pub fn track_variability_base(prim: &Prim, _time: SdfTimeCode) -> HdDirtyBits {
        let mut bits: HdDirtyBits = 0;

        // Check local primvars for time-varying values.
        let primvars_api = usd_geom::PrimvarsAPI::new(prim.clone());
        for pv in primvars_api.get_primvars_with_values() {
            if pv.value_might_be_time_varying() {
                bits |= HdRprimDirtyBits::DIRTY_PRIMVAR;
                break;
            }
        }

        // Check extent.
        if let Some(attr) = prim.get_attribute("extent") {
            if attr.value_might_be_time_varying() {
                bits |= HdRprimDirtyBits::DIRTY_EXTENT;
            }
        }

        // Check transform: walk up hierarchy for any time-varying xformOps.
        bits |= Self::check_transform_varying(prim);

        // Check visibility.
        if let Some(attr) = prim.get_attribute("visibility") {
            if attr.value_might_be_time_varying() {
                bits |= HdRprimDirtyBits::DIRTY_VISIBILITY;
            }
        }

        // Check velocity/acceleration (affect DirtyPoints).
        for name in &["velocities", "accelerations"] {
            if let Some(attr) = prim.get_attribute(name) {
                if attr.value_might_be_time_varying() {
                    bits |= HdRprimDirtyBits::DIRTY_POINTS;
                    break;
                }
            }
        }

        // Check doubleSided.
        if let Some(attr) = prim.get_attribute("doubleSided") {
            if attr.value_might_be_time_varying() {
                bits |= HdRprimDirtyBits::DIRTY_DOUBLE_SIDED;
            }
        }

        bits
    }

    /// Check if transform is time-varying by walking up the hierarchy.
    /// Matches C++ _IsTransformVarying.
    fn check_transform_varying(prim: &Prim) -> HdDirtyBits {
        let mut current = prim.clone();
        loop {
            // Check xformOp attributes for time-varying values.
            let xformable = usd_geom::Xformable::new(current.clone());
            if xformable.transform_might_be_time_varying() {
                return HdRprimDirtyBits::DIRTY_TRANSFORM;
            }
            if xformable.get_reset_xform_stack() {
                break;
            }
            let parent = current.parent();
            if !parent.is_valid() || parent.path().is_absolute_root_path() {
                break;
            }
            current = parent;
        }
        0
    }
}

impl PrimAdapter for GprimAdapter {
    fn track_variability(&self, prim: &Prim, time: SdfTimeCode) -> HdDirtyBits {
        Self::track_variability_base(prim, time)
    }

    fn get_imaging_subprim_type(&self, _prim: &Prim, subprim: &Token) -> Token {
        if subprim.is_empty() {
            self.prim_type.clone()
        } else {
            Token::new("")
        }
    }

    fn get_imaging_subprim_data(
        &self,
        prim: &Prim,
        subprim: &Token,
        stage_globals: &DataSourceStageGlobalsHandle,
    ) -> Option<HdContainerDataSourceHandle> {
        if subprim.is_empty() {
            Some(self.create_data_source(prim, stage_globals))
        } else {
            None
        }
    }

    fn invalidate_imaging_subprim(
        &self,
        prim: &Prim,
        subprim: &Token,
        properties: &[Token],
        invalidation_type: PropertyInvalidationType,
    ) -> HdDataSourceLocatorSet {
        DataSourceGprim::invalidate(prim, subprim, properties, invalidation_type)
    }
}

/// Arc-wrapped GprimAdapter for sharing
pub type GprimAdapterHandle = Arc<GprimAdapter>;

#[cfg(test)]
mod tests {
    use super::*;
    use usd_core::Stage;

    #[test]
    fn test_gprim_adapter_creation() {
        let adapter = GprimAdapter::new(Token::new("mesh"));
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let subprim = Token::new("");

        let prim_type = adapter.get_imaging_subprim_type(&prim, &subprim);
        assert_eq!(prim_type.as_str(), "mesh");
    }

    #[test]
    fn test_implicit_basis_z() {
        let basis = GprimAdapter::get_implicit_basis(&Token::new("Z"));
        assert_eq!(basis, Matrix4d::identity());
    }

    #[test]
    fn test_implicit_basis_x() {
        let basis = GprimAdapter::get_implicit_basis(&Token::new("X"));
        // Should rotate Z axis to X axis
        assert_ne!(basis, Matrix4d::identity());
    }

    #[test]
    fn test_gprim_adapter_subprims() {
        let adapter = GprimAdapter::new(Token::new("mesh"));
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();

        let subprims = adapter.get_imaging_subprims(&prim);
        assert_eq!(subprims.len(), 1);
        assert!(subprims[0].is_empty());
    }

    #[test]
    fn test_gprim_adapter_invalidation() {
        let adapter = GprimAdapter::new(Token::new("mesh"));
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let properties = vec![Token::new("points"), Token::new("visibility")];

        let locators = adapter.invalidate_imaging_subprim(
            &prim,
            &Token::new(""),
            &properties,
            PropertyInvalidationType::PropertyChanged,
        );

        assert!(!locators.is_empty());
    }
}
