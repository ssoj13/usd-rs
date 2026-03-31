//! UsdSkelAnimQuery - class for querying skeletal animation prims.
//!
//! Port of pxr/usd/usdSkel/animQuery.h/cpp

use usd_core::{Attribute, Prim};
use usd_gf::{Matrix4d, Quatf, Vec3f, Vec3h};
use usd_sdf::TimeCode;
use usd_tf::Token;

use super::animation::SkelAnimation;

/// Class providing efficient queries of primitives that provide skel animation.
///
/// Matches C++ `UsdSkelAnimQuery`.
#[derive(Clone)]
pub struct AnimQuery {
    /// The animation prim being queried.
    anim: Option<SkelAnimation>,
    /// Cached joint order.
    joint_order: Vec<Token>,
    /// Cached blend shape order.
    blend_shape_order: Vec<Token>,
}

impl Default for AnimQuery {
    fn default() -> Self {
        Self::new()
    }
}

impl AnimQuery {
    /// Creates an invalid (empty) AnimQuery.
    pub fn new() -> Self {
        Self {
            anim: None,
            joint_order: Vec::new(),
            blend_shape_order: Vec::new(),
        }
    }

    /// Creates an AnimQuery from a prim.
    ///
    /// Returns None if the prim is not a valid animation source.
    pub fn from_prim(prim: Prim) -> Option<Self> {
        let anim = SkelAnimation::new(prim);
        if !anim.is_valid() {
            return None;
        }

        // Cache the joint and blend shape orders
        let joints_attr = anim.get_joints_attr();
        let joint_order = if joints_attr.is_valid() {
            joints_attr
                .get_typed_vec::<Token>(TimeCode::default())
                .unwrap_or_default()
        } else {
            Vec::new()
        };

        let blend_shapes_attr = anim.get_blend_shapes_attr();
        let blend_shape_order = if blend_shapes_attr.is_valid() {
            blend_shapes_attr
                .get_typed_vec::<Token>(TimeCode::default())
                .unwrap_or_default()
        } else {
            Vec::new()
        };

        Some(Self {
            anim: Some(anim),
            joint_order,
            blend_shape_order,
        })
    }

    /// Returns true if this query is valid.
    pub fn is_valid(&self) -> bool {
        self.anim.as_ref().map(|a| a.is_valid()).unwrap_or(false)
    }

    /// Returns the prim this anim query reads from.
    pub fn get_prim(&self) -> Option<Prim> {
        self.anim.as_ref().map(|a| a.prim().clone())
    }

    /// Compute joint transforms in joint-local space.
    ///
    /// Transforms are returned in the order specified by the joint ordering
    /// of the animation primitive itself.
    pub fn compute_joint_local_transforms(
        &self,
        xforms: &mut Vec<Matrix4d>,
        time: &TimeCode,
    ) -> bool {
        let Some(anim) = &self.anim else {
            return false;
        };

        // Try to compute transforms from components

        let trans_attr = anim.get_translations_attr();
        let rot_attr = anim.get_rotations_attr();
        let scale_attr = anim.get_scales_attr();

        let translations = if trans_attr.is_valid() {
            trans_attr.get_typed_vec::<Vec3f>(*time)
        } else {
            None
        };
        let rotations = if rot_attr.is_valid() {
            rot_attr.get_typed_vec::<Quatf>(*time)
        } else {
            None
        };
        let scales = if scale_attr.is_valid() {
            scale_attr.get_typed_vec::<Vec3h>(*time)
        } else {
            None
        };

        if let (Some(trans), Some(rots), Some(scls)) = (translations, rotations, scales) {
            if trans.len() == rots.len() && rots.len() == scls.len() {
                xforms.clear();
                xforms.reserve(trans.len());
                for i in 0..trans.len() {
                    xforms.push(make_transform(&trans[i], &rots[i], &scls[i]));
                }

                // Matches C++ check: xforms size must equal joint order size.
                // Empty xforms is a valid "no opinion" case (blocked/unwritten attrs).
                if xforms.len() == self.joint_order.len() {
                    return true;
                } else if xforms.is_empty() {
                    return false;
                }
                // Size mismatch — clear xforms to avoid returning partial data.
                xforms.clear();
            }
        }

        false
    }

    /// Compute translation, rotation, scale components of the joint transforms
    /// in joint-local space.
    pub fn compute_joint_local_transform_components(
        &self,
        translations: &mut Vec<Vec3f>,
        rotations: &mut Vec<Quatf>,
        scales: &mut Vec<Vec3h>,
        time: &TimeCode,
    ) -> bool {
        let Some(anim) = &self.anim else {
            return false;
        };

        let trans_attr = anim.get_translations_attr();
        let rot_attr = anim.get_rotations_attr();
        let scale_attr = anim.get_scales_attr();

        let trans = if trans_attr.is_valid() {
            trans_attr.get_typed_vec::<Vec3f>(*time)
        } else {
            None
        };
        let rots = if rot_attr.is_valid() {
            rot_attr.get_typed_vec::<Quatf>(*time)
        } else {
            None
        };
        let scls = if scale_attr.is_valid() {
            scale_attr.get_typed_vec::<Vec3h>(*time)
        } else {
            None
        };

        if let (Some(t), Some(r), Some(s)) = (trans, rots, scls) {
            *translations = t;
            *rotations = r;
            *scales = s;
            return true;
        }

        false
    }

    /// Compute blend shape weights.
    pub fn compute_blend_shape_weights(&self, weights: &mut Vec<f32>, time: &TimeCode) -> bool {
        let Some(anim) = &self.anim else {
            return false;
        };

        let attr = anim.get_blend_shape_weights_attr();
        if attr.is_valid() {
            if let Some(w) = attr.get_typed_vec::<f32>(*time) {
                *weights = w;
                return true;
            }
        }

        false
    }

    /// Get the time samples at which values contributing to joint transforms are set.
    pub fn get_joint_transform_time_samples(&self, times: &mut Vec<f64>) -> bool {
        self.get_joint_transform_time_samples_in_interval(f64::NEG_INFINITY, f64::INFINITY, times)
    }

    /// Get the time samples at which values contributing to joint transforms are set,
    /// within the given interval.
    pub fn get_joint_transform_time_samples_in_interval(
        &self,
        start: f64,
        end: f64,
        times: &mut Vec<f64>,
    ) -> bool {
        let Some(anim) = &self.anim else {
            return false;
        };

        times.clear();

        // Collect time samples from all relevant attributes
        let attrs = [
            anim.get_translations_attr(),
            anim.get_rotations_attr(),
            anim.get_scales_attr(),
        ];

        for attr in attrs.iter() {
            if attr.is_valid() {
                let samples = attr.get_time_samples_in_interval(start, end);
                for t in samples {
                    if !times.contains(&t) {
                        times.push(t);
                    }
                }
            }
        }

        times.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        true
    }

    /// Get the attributes contributing to joint transform computations.
    pub fn get_joint_transform_attributes(&self, attrs: &mut Vec<Attribute>) -> bool {
        let Some(anim) = &self.anim else {
            return false;
        };

        attrs.clear();

        let trans_attr = anim.get_translations_attr();
        if trans_attr.is_valid() {
            attrs.push(trans_attr);
        }
        let rot_attr = anim.get_rotations_attr();
        if rot_attr.is_valid() {
            attrs.push(rot_attr);
        }
        let scale_attr = anim.get_scales_attr();
        if scale_attr.is_valid() {
            attrs.push(scale_attr);
        }

        true
    }

    /// Return true if joint transforms might be time varying.
    pub fn joint_transforms_might_be_time_varying(&self) -> bool {
        let Some(anim) = &self.anim else {
            return false;
        };

        let attrs = [
            anim.get_translations_attr(),
            anim.get_rotations_attr(),
            anim.get_scales_attr(),
        ];

        for attr in attrs.iter() {
            if attr.is_valid() && attr.value_might_be_time_varying() {
                return true;
            }
        }

        false
    }

    /// Get the time samples at which blend shape weights are set.
    pub fn get_blend_shape_weight_time_samples(&self, times: &mut Vec<f64>) -> bool {
        self.get_blend_shape_weight_time_samples_in_interval(
            f64::NEG_INFINITY,
            f64::INFINITY,
            times,
        )
    }

    /// Get the time samples at which blend shape weights are set, within interval.
    pub fn get_blend_shape_weight_time_samples_in_interval(
        &self,
        start: f64,
        end: f64,
        times: &mut Vec<f64>,
    ) -> bool {
        let Some(anim) = &self.anim else {
            return false;
        };

        times.clear();

        let attr = anim.get_blend_shape_weights_attr();
        if attr.is_valid() {
            *times = attr.get_time_samples_in_interval(start, end);
            return !times.is_empty();
        }

        false
    }

    /// Get the attributes contributing to blend shape weight computations.
    pub fn get_blend_shape_weight_attributes(&self, attrs: &mut Vec<Attribute>) -> bool {
        let Some(anim) = &self.anim else {
            return false;
        };

        attrs.clear();

        let weights_attr = anim.get_blend_shape_weights_attr();
        if weights_attr.is_valid() {
            attrs.push(weights_attr);
        }

        true
    }

    /// Return true if blend shape weights might be time varying.
    pub fn blend_shape_weights_might_be_time_varying(&self) -> bool {
        let Some(anim) = &self.anim else {
            return false;
        };

        let attr = anim.get_blend_shape_weights_attr();
        if attr.is_valid() {
            return attr.value_might_be_time_varying();
        }

        false
    }

    /// Returns an array of tokens describing the ordering of joints.
    pub fn get_joint_order(&self) -> &[Token] {
        &self.joint_order
    }

    /// Returns an array of tokens describing the ordering of blend shapes.
    pub fn get_blend_shape_order(&self) -> &[Token] {
        &self.blend_shape_order
    }

    /// Get a description string.
    pub fn get_description(&self) -> String {
        if let Some(prim) = self.get_prim() {
            format!("AnimQuery for {}", prim.path().get_string())
        } else {
            "Invalid AnimQuery".to_string()
        }
    }
}

impl PartialEq for AnimQuery {
    fn eq(&self, other: &Self) -> bool {
        match (&self.anim, &other.anim) {
            (Some(a), Some(b)) => a.prim().path() == b.prim().path(),
            (None, None) => true,
            _ => false,
        }
    }
}

impl std::hash::Hash for AnimQuery {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        if let Some(anim) = &self.anim {
            anim.prim().path().hash(state);
        }
    }
}

/// Create a transform matrix from translate/rotate/scale components.
fn make_transform(translate: &Vec3f, rotate: &Quatf, scale: &Vec3h) -> Matrix4d {
    // Build matrix: T * R * S
    let sx = f64::from(scale.x);
    let sy = f64::from(scale.y);
    let sz = f64::from(scale.z);

    let tx = translate.x as f64;
    let ty = translate.y as f64;
    let tz = translate.z as f64;

    // Convert quaternion to rotation matrix elements
    // q = w + xi + yj + zk
    let w = rotate.real() as f64;
    let x = rotate.imaginary().x as f64;
    let y = rotate.imaginary().y as f64;
    let z = rotate.imaginary().z as f64;

    // Rotation matrix from quaternion (column-major like USD)
    let r00 = 1.0 - 2.0 * (y * y + z * z);
    let r01 = 2.0 * (x * y - z * w);
    let r02 = 2.0 * (x * z + y * w);
    let r10 = 2.0 * (x * y + z * w);
    let r11 = 1.0 - 2.0 * (x * x + z * z);
    let r12 = 2.0 * (y * z - x * w);
    let r20 = 2.0 * (x * z - y * w);
    let r21 = 2.0 * (y * z + x * w);
    let r22 = 1.0 - 2.0 * (x * x + y * y);

    // Compose: T * R * S (row-major storage)
    Matrix4d::new(
        r00 * sx,
        r01 * sx,
        r02 * sx,
        0.0,
        r10 * sy,
        r11 * sy,
        r12 * sy,
        0.0,
        r20 * sz,
        r21 * sz,
        r22 * sz,
        0.0,
        tx,
        ty,
        tz,
        1.0,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invalid_query() {
        let query = AnimQuery::new();
        assert!(!query.is_valid());
        assert!(query.get_prim().is_none());
    }
}
