//! Typed data source type definitions.
//!
//! Centralized type aliases for HdTypedSampledDataSource<T> across common types.
//! Corresponds to pxr/imaging/hd/dataSourceTypeDefs.h

use crate::data_source::HdTypedSampledDataSource;
use crate::data_source::locator::HdDataSourceLocator;
use crate::types::HdFormat;
use crate::types::HdTupleType;
use std::sync::Arc;
use usd_gf::{
    Matrix3f, Matrix4d, Matrix4f, Quatf, Vec2d, Vec2f, Vec2i, Vec3d, Vec3f, Vec3i, Vec4d, Vec4f,
    Vec4i,
};
use usd_sdf::{AssetPath, Path, PathExpression};
use usd_tf::Token;
use usd_vt::Array;

// =============================================================================
// Numeric
// =============================================================================

/// Typed data source for `i32`.
pub type HdIntDataSource = dyn HdTypedSampledDataSource<i32> + Send + Sync;
/// Handle to typed data source for `i32`.
pub type HdIntDataSourceHandle = Arc<HdIntDataSource>;
/// Typed data source for `VtArray<i32>`.
pub type HdIntArrayDataSource = dyn HdTypedSampledDataSource<Array<i32>> + Send + Sync;
/// Handle to typed data source for `VtArray<i32>`.
pub type HdIntArrayDataSourceHandle = Arc<HdIntArrayDataSource>;

/// Typed data source for `usize` (C++ `size_t`).
pub type HdSizetDataSource = dyn HdTypedSampledDataSource<usize> + Send + Sync;
/// Handle to typed data source for `usize`.
pub type HdSizetDataSourceHandle = Arc<HdSizetDataSource>;

/// Typed data source for `f32`.
pub type HdFloatDataSource = dyn HdTypedSampledDataSource<f32> + Send + Sync;
/// Handle to typed data source for `f32`.
pub type HdFloatDataSourceHandle = Arc<HdFloatDataSource>;
/// Typed data source for `f64`.
pub type HdDoubleDataSource = dyn HdTypedSampledDataSource<f64> + Send + Sync;
/// Handle to typed data source for `f64`.
pub type HdDoubleDataSourceHandle = Arc<HdDoubleDataSource>;
/// Typed data source for `VtArray<f32>`.
pub type HdFloatArrayDataSource = dyn HdTypedSampledDataSource<Array<f32>> + Send + Sync;
/// Handle to typed data source for `VtArray<f32>`.
pub type HdFloatArrayDataSourceHandle = Arc<HdFloatArrayDataSource>;
/// Typed data source for `VtArray<f64>`.
pub type HdDoubleArrayDataSource = dyn HdTypedSampledDataSource<Array<f64>> + Send + Sync;
/// Handle to typed data source for `VtArray<f64>`.
pub type HdDoubleArrayDataSourceHandle = Arc<HdDoubleArrayDataSource>;

// =============================================================================
// Bool
// =============================================================================

/// Typed data source for `bool`.
pub type HdBoolDataSource = dyn HdTypedSampledDataSource<bool> + Send + Sync;
/// Handle to typed data source for `bool`.
pub type HdBoolDataSourceHandle = Arc<HdBoolDataSource>;
/// Typed data source for `VtArray<bool>`.
pub type HdBoolArrayDataSource = dyn HdTypedSampledDataSource<Array<bool>> + Send + Sync;
/// Handle to typed data source for `VtArray<bool>`.
pub type HdBoolArrayDataSourceHandle = Arc<HdBoolArrayDataSource>;

// =============================================================================
// String / Token / Path
// =============================================================================

/// Typed data source for `TfToken`.
pub type HdTokenDataSource = dyn HdTypedSampledDataSource<Token> + Send + Sync;
/// Handle to typed data source for `TfToken`.
pub type HdTokenDataSourceHandle = Arc<HdTokenDataSource>;
/// Typed data source for `VtArray<TfToken>`.
pub type HdTokenArrayDataSource = dyn HdTypedSampledDataSource<Array<Token>> + Send + Sync;
/// Handle to typed data source for `VtArray<TfToken>`.
pub type HdTokenArrayDataSourceHandle = Arc<HdTokenArrayDataSource>;

/// Typed data source for `SdfPath`.
pub type HdPathDataSource = dyn HdTypedSampledDataSource<Path> + Send + Sync;
/// Handle to typed data source for `SdfPath`.
pub type HdPathDataSourceHandle = Arc<HdPathDataSource>;
/// Typed data source for `VtArray<SdfPath>`.
pub type HdPathArrayDataSource = dyn HdTypedSampledDataSource<Array<Path>> + Send + Sync;
/// Handle to typed data source for `VtArray<SdfPath>`.
pub type HdPathArrayDataSourceHandle = Arc<HdPathArrayDataSource>;

/// Typed data source for `String`.
pub type HdStringDataSource = dyn HdTypedSampledDataSource<String> + Send + Sync;
/// Handle to typed data source for `String`.
pub type HdStringDataSourceHandle = Arc<HdStringDataSource>;
/// Typed data source for `VtArray<String>`.
pub type HdStringArrayDataSource = dyn HdTypedSampledDataSource<Array<String>> + Send + Sync;
/// Handle to typed data source for `VtArray<String>`.
pub type HdStringArrayDataSourceHandle = Arc<HdStringArrayDataSource>;

/// Typed data source for `SdfAssetPath`.
pub type HdAssetPathDataSource = dyn HdTypedSampledDataSource<AssetPath> + Send + Sync;
/// Handle to typed data source for `SdfAssetPath`.
pub type HdAssetPathDataSourceHandle = Arc<HdAssetPathDataSource>;

/// Typed data source for `SdfPathExpression`.
pub type HdPathExpressionDataSource = dyn HdTypedSampledDataSource<PathExpression> + Send + Sync;
/// Handle to typed data source for `SdfPathExpression`.
pub type HdPathExpressionDataSourceHandle = Arc<HdPathExpressionDataSource>;

// =============================================================================
// Linear algebra
// =============================================================================

/// Typed data source for `GfVec2i`.
pub type HdVec2iDataSource = dyn HdTypedSampledDataSource<Vec2i> + Send + Sync;
/// Handle to typed data source for `GfVec2i`.
pub type HdVec2iDataSourceHandle = Arc<HdVec2iDataSource>;
/// Typed data source for `VtArray<GfVec2i>`.
pub type HdVec2iArrayDataSource = dyn HdTypedSampledDataSource<Array<Vec2i>> + Send + Sync;
/// Handle to typed data source for `VtArray<GfVec2i>`.
pub type HdVec2iArrayDataSourceHandle = Arc<HdVec2iArrayDataSource>;

/// Typed data source for `GfVec2f`.
pub type HdVec2fDataSource = dyn HdTypedSampledDataSource<Vec2f> + Send + Sync;
/// Handle to typed data source for `GfVec2f`.
pub type HdVec2fDataSourceHandle = Arc<HdVec2fDataSource>;
/// Typed data source for `GfVec2d`.
pub type HdVec2dDataSource = dyn HdTypedSampledDataSource<Vec2d> + Send + Sync;
/// Handle to typed data source for `GfVec2d`.
pub type HdVec2dDataSourceHandle = Arc<HdVec2dDataSource>;
/// Typed data source for `VtArray<GfVec2f>`.
pub type HdVec2fArrayDataSource = dyn HdTypedSampledDataSource<Array<Vec2f>> + Send + Sync;
/// Handle to typed data source for `VtArray<GfVec2f>`.
pub type HdVec2fArrayDataSourceHandle = Arc<HdVec2fArrayDataSource>;
/// Typed data source for `VtArray<GfVec2d>`.
pub type HdVec2dArrayDataSource = dyn HdTypedSampledDataSource<Array<Vec2d>> + Send + Sync;
/// Handle to typed data source for `VtArray<GfVec2d>`.
pub type HdVec2dArrayDataSourceHandle = Arc<HdVec2dArrayDataSource>;

/// Typed data source for `GfVec3i`.
pub type HdVec3iDataSource = dyn HdTypedSampledDataSource<Vec3i> + Send + Sync;
/// Handle to typed data source for `GfVec3i`.
pub type HdVec3iDataSourceHandle = Arc<HdVec3iDataSource>;
/// Typed data source for `VtArray<GfVec3i>`.
pub type HdVec3iArrayDataSource = dyn HdTypedSampledDataSource<Array<Vec3i>> + Send + Sync;
/// Handle to typed data source for `VtArray<GfVec3i>`.
pub type HdVec3iArrayDataSourceHandle = Arc<HdVec3iArrayDataSource>;

/// Typed data source for `GfVec3f`.
pub type HdVec3fDataSource = dyn HdTypedSampledDataSource<Vec3f> + Send + Sync;
/// Handle to typed data source for `GfVec3f`.
pub type HdVec3fDataSourceHandle = Arc<HdVec3fDataSource>;
/// Typed data source for `VtArray<GfVec3f>`.
pub type HdVec3fArrayDataSource = dyn HdTypedSampledDataSource<Array<Vec3f>> + Send + Sync;
/// Handle to typed data source for `VtArray<GfVec3f>`.
pub type HdVec3fArrayDataSourceHandle = Arc<HdVec3fArrayDataSource>;

/// Typed data source for `GfVec3d`.
pub type HdVec3dDataSource = dyn HdTypedSampledDataSource<Vec3d> + Send + Sync;
/// Handle to typed data source for `GfVec3d`.
pub type HdVec3dDataSourceHandle = Arc<HdVec3dDataSource>;
/// Typed data source for `VtArray<GfVec3d>`.
pub type HdVec3dArrayDataSource = dyn HdTypedSampledDataSource<Array<Vec3d>> + Send + Sync;
/// Handle to typed data source for `VtArray<GfVec3d>`.
pub type HdVec3dArrayDataSourceHandle = Arc<HdVec3dArrayDataSource>;

/// Typed data source for `GfVec4i`.
pub type HdVec4iDataSource = dyn HdTypedSampledDataSource<Vec4i> + Send + Sync;
/// Handle to typed data source for `GfVec4i`.
pub type HdVec4iDataSourceHandle = Arc<HdVec4iDataSource>;
/// Typed data source for `VtArray<GfVec4i>`.
pub type HdVec4iArrayDataSource = dyn HdTypedSampledDataSource<Array<Vec4i>> + Send + Sync;
/// Handle to typed data source for `VtArray<GfVec4i>`.
pub type HdVec4iArrayDataSourceHandle = Arc<HdVec4iArrayDataSource>;

/// Typed data source for `GfVec4f`.
pub type HdVec4fDataSource = dyn HdTypedSampledDataSource<Vec4f> + Send + Sync;
/// Handle to typed data source for `GfVec4f`.
pub type HdVec4fDataSourceHandle = Arc<HdVec4fDataSource>;
/// Typed data source for `VtArray<GfVec4f>`.
pub type HdVec4fArrayDataSource = dyn HdTypedSampledDataSource<Array<Vec4f>> + Send + Sync;
/// Handle to typed data source for `VtArray<GfVec4f>`.
pub type HdVec4fArrayDataSourceHandle = Arc<HdVec4fArrayDataSource>;

/// Typed data source for `VtArray<GfVec4d>`.
pub type HdVec4dArrayDataSource = dyn HdTypedSampledDataSource<Array<Vec4d>> + Send + Sync;
/// Handle to typed data source for `VtArray<GfVec4d>`.
pub type HdVec4dArrayDataSourceHandle = Arc<HdVec4dArrayDataSource>;

/// Typed data source for `VtArray<GfQuatf>`.
pub type HdQuatfArrayDataSource = dyn HdTypedSampledDataSource<Array<Quatf>> + Send + Sync;
/// Handle to typed data source for `VtArray<GfQuatf>`.
pub type HdQuatfArrayDataSourceHandle = Arc<HdQuatfArrayDataSource>;

/// Typed data source for `GfMatrix3f`.
pub type HdMatrix3fDataSource = dyn HdTypedSampledDataSource<Matrix3f> + Send + Sync;
/// Handle to typed data source for `GfMatrix3f`.
pub type HdMatrix3fDataSourceHandle = Arc<HdMatrix3fDataSource>;
/// Typed data source for `VtArray<GfMatrix3f>`.
pub type HdMatrix3fArrayDataSource = dyn HdTypedSampledDataSource<Array<Matrix3f>> + Send + Sync;
/// Handle to typed data source for `VtArray<GfMatrix3f>`.
pub type HdMatrix3fArrayDataSourceHandle = Arc<HdMatrix3fArrayDataSource>;

/// Typed data source for `GfMatrix4f`.
pub type HdMatrix4fDataSource = dyn HdTypedSampledDataSource<Matrix4f> + Send + Sync;
/// Handle to typed data source for `GfMatrix4f`.
pub type HdMatrix4fDataSourceHandle = Arc<HdMatrix4fDataSource>;
/// Typed data source for `VtArray<GfMatrix4f>`.
pub type HdMatrix4fArrayDataSource = dyn HdTypedSampledDataSource<Array<Matrix4f>> + Send + Sync;
/// Handle to typed data source for `VtArray<GfMatrix4f>`.
pub type HdMatrix4fArrayDataSourceHandle = Arc<HdMatrix4fArrayDataSource>;

/// Typed data source for `GfMatrix4d` (default matrix type).
pub type HdMatrixDataSource = dyn HdTypedSampledDataSource<Matrix4d> + Send + Sync;
/// Handle to typed data source for `GfMatrix4d`.
pub type HdMatrixDataSourceHandle = Arc<HdMatrixDataSource>;
/// Typed data source for `VtArray<GfMatrix4d>`.
pub type HdMatrixArrayDataSource = dyn HdTypedSampledDataSource<Array<Matrix4d>> + Send + Sync;
/// Handle to typed data source for `VtArray<GfMatrix4d>`.
pub type HdMatrixArrayDataSourceHandle = Arc<HdMatrixArrayDataSource>;

// =============================================================================
// Locator
// =============================================================================

/// Typed data source for `HdDataSourceLocator`.
pub type HdLocatorDataSource = dyn HdTypedSampledDataSource<HdDataSourceLocator> + Send + Sync;
/// Handle to typed data source for `HdDataSourceLocator`.
pub type HdLocatorDataSourceHandle = Arc<HdLocatorDataSource>;

// =============================================================================
// Enum / Type descriptors
// =============================================================================

/// Typed data source for `HdFormat`.
pub type HdFormatDataSource = dyn HdTypedSampledDataSource<HdFormat> + Send + Sync;
/// Handle to typed data source for `HdFormat`.
pub type HdFormatDataSourceHandle = Arc<HdFormatDataSource>;

/// Typed data source for `HdTupleType`.
pub type HdTupleTypeDataSource = dyn HdTypedSampledDataSource<HdTupleType> + Send + Sync;
/// Handle to typed data source for `HdTupleType`.
pub type HdTupleTypeDataSourceHandle = Arc<HdTupleTypeDataSource>;

// ArResolverContext aliases live in data_source/mod.rs to avoid duplicates.
