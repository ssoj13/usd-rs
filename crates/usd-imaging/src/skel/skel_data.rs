//! SkelData - Data for computing skinning transforms.
//!
//! Port of pxr/usdImaging/usdSkelImaging/skelData.h/cpp
//!
//! Holds topology, bind transforms from skeleton and skelAnimation joints.

use super::skeleton_schema::SkeletonSchema;
use usd_gf::matrix4::{Matrix4d, Matrix4f};
use usd_hd::data_source::HdContainerDataSourceHandle;
use usd_hd::scene_index::{HdSceneIndexHandle, si_ref};
use usd_sdf::Path;
use usd_skel::Topology;

/// Data necessary to compute skinning transforms of a skeleton.
///
/// From skeleton and skelAnimation's joints.
#[derive(Debug, Clone)]
pub struct SkelData {
    /// Path of deformable prim (for warnings/errors).
    pub prim_path: Path,

    /// Skeleton schema from input.
    pub skeleton_schema: SkeletonSchema,

    /// Topology from skeleton's joints.
    pub topology: Topology,

    /// Bind transforms from skeleton (converted to f32).
    pub bind_transforms: Vec<Matrix4f>,

    /// Inverse of bind transforms.
    pub inverse_bind_transforms: Vec<Matrix4f>,
}

/// Convert Matrix4d array to Matrix4f array.
fn convert_d_to_f(matrices: &[Matrix4d]) -> Vec<Matrix4f> {
    matrices
        .iter()
        .map(|m| {
            Matrix4f::new(
                m[0][0] as f32,
                m[0][1] as f32,
                m[0][2] as f32,
                m[0][3] as f32,
                m[1][0] as f32,
                m[1][1] as f32,
                m[1][2] as f32,
                m[1][3] as f32,
                m[2][0] as f32,
                m[2][1] as f32,
                m[2][2] as f32,
                m[2][3] as f32,
                m[3][0] as f32,
                m[3][1] as f32,
                m[3][2] as f32,
                m[3][3] as f32,
            )
        })
        .collect()
}

/// Invert each matrix in the array. Falls back to identity for singular matrices.
fn invert_matrices(matrices: &[Matrix4f]) -> Vec<Matrix4f> {
    matrices
        .iter()
        .map(|m| m.inverse().unwrap_or_else(Matrix4f::identity))
        .collect()
}

impl SkelData {
    /// Create from path and schema.
    pub fn new(path: Path, schema: SkeletonSchema) -> Self {
        Self {
            prim_path: path,
            skeleton_schema: schema,
            topology: Topology::new(),
            bind_transforms: Vec::new(),
            inverse_bind_transforms: Vec::new(),
        }
    }

    /// Create from path, schema, and bind transforms (Matrix4d).
    ///
    /// Converts bind transforms to f32 and computes inverses.
    /// This is the primary constructor matching C++ UsdSkelImagingSkelData.
    pub fn from_bind_xforms(
        path: Path,
        schema: SkeletonSchema,
        topology: Topology,
        bind_xforms_d: &[Matrix4d],
    ) -> Self {
        let bind_transforms = convert_d_to_f(bind_xforms_d);
        let inverse_bind_transforms = invert_matrices(&bind_transforms);
        Self {
            prim_path: path,
            skeleton_schema: schema,
            topology,
            bind_transforms,
            inverse_bind_transforms,
        }
    }
}

/// Compute SkelData for prim in scene index.
///
/// Port of C++ `UsdSkelImagingComputeSkelData`.
/// Reads skeleton schema from scene index, extracts bind transforms,
/// converts to f32 and computes inverses.
pub fn compute_skel_data(scene_index: &HdSceneIndexHandle, prim_path: &Path) -> SkelData {
    // Get prim from scene index
    let prim = si_ref(&scene_index).get_prim(prim_path);

    let Some(data_source) = prim.data_source.as_ref() else {
        return SkelData::new(prim_path.clone(), SkeletonSchema::new(None));
    };

    compute_skel_data_from_source(prim_path.clone(), data_source)
}

/// Compute SkelData from an already-resolved prim data source.
///
/// This is needed when the scene index path being evaluated is an overlay
/// wrapper (for example, a resolved skeleton prim) but we still need to read
/// the authored `SkeletonSchema` from the original source container.
pub fn compute_skel_data_from_source(
    prim_path: Path,
    data_source: &HdContainerDataSourceHandle,
) -> SkelData {
    let diag = std::env::var_os("USD_PROFILE_SKEL_DS").is_some();
    if diag {
        eprintln!("[compute_skel_data_from_source] path={} schema:start", prim_path);
    }
    // Extract skeleton schema from the provided prim data source.
    let schema = SkeletonSchema::get_from_parent(data_source)
        .unwrap_or_else(|| SkeletonSchema::new(None));
    if diag {
        eprintln!("[compute_skel_data_from_source] path={} schema:done", prim_path);
    }

    // Build topology from joint tokens (matches C++ UsdSkelTopology(joints))
    if diag {
        eprintln!("[compute_skel_data_from_source] path={} get_joints:start", prim_path);
    }
    let joints = schema.get_joints();
    if diag {
        eprintln!(
            "[compute_skel_data_from_source] path={} get_joints:done count={}",
            prim_path,
            joints.len()
        );
    }
    let topology = if !joints.is_empty() {
        Topology::from_tokens(&joints)
    } else {
        Topology::new()
    };

    // Extract bind transforms via SkeletonSchema::get_bind_transforms().
    // C++: UsdSkelImagingComputeSkelData reads bindTransforms from the schema,
    // converts d->f, then computes inverses for skinning.
    if diag {
        eprintln!(
            "[compute_skel_data_from_source] path={} get_bind_transforms:start",
            prim_path
        );
    }
    let bind_xforms_d = schema.get_bind_transforms();
    if diag {
        eprintln!(
            "[compute_skel_data_from_source] path={} get_bind_transforms:done count={}",
            prim_path,
            bind_xforms_d.len()
        );
    }
    let data = if !bind_xforms_d.is_empty() {
        // from_bind_xforms takes topology and sets bind_transforms + inverses.
        SkelData::from_bind_xforms(prim_path.clone(), schema, topology, &bind_xforms_d)
    } else {
        let mut d = SkelData::new(prim_path.clone(), schema);
        d.topology = topology;
        d
    };

    if diag {
        eprintln!("[compute_skel_data_from_source] path={} done", prim_path);
    }
    data
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skel_data_new() {
        let path = Path::from_string("/Skel").unwrap();
        let data = SkelData::new(path, SkeletonSchema::new(None));
        assert!(data.bind_transforms.is_empty());
        assert!(data.inverse_bind_transforms.is_empty());
    }

    #[test]
    fn test_convert_d_to_f() {
        let id = Matrix4d::identity();
        let result = convert_d_to_f(&[id]);
        assert_eq!(result.len(), 1);
        assert!((result[0][0][0] - 1.0f32).abs() < 1e-6);
        assert!((result[0][1][1] - 1.0f32).abs() < 1e-6);
    }

    #[test]
    fn test_invert_identity() {
        let id = Matrix4f::identity();
        let inv = invert_matrices(&[id]);
        assert_eq!(inv.len(), 1);
        // Inverse of identity is identity
        for i in 0..4 {
            for j in 0..4 {
                let expected = if i == j { 1.0f32 } else { 0.0f32 };
                assert!((inv[0][i][j] - expected).abs() < 1e-5);
            }
        }
    }

    #[test]
    fn test_from_bind_xforms() {
        let path = Path::from_string("/Skel").unwrap();
        let schema = SkeletonSchema::new(None);
        let topology = Topology::from_parent_indices(vec![-1, 0]);
        let xforms = vec![Matrix4d::identity(), Matrix4d::identity()];

        let data = SkelData::from_bind_xforms(path, schema, topology, &xforms);
        assert_eq!(data.bind_transforms.len(), 2);
        assert_eq!(data.inverse_bind_transforms.len(), 2);
    }
}
