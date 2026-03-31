//! UsdSkelBlendShapeQuery - helper for resolving blend shape weights.
//!
//! Port of pxr/usd/usdSkel/blendShapeQuery.h/cpp

use super::binding_api::BindingAPI;
use super::blend_shape::BlendShape;
use super::inbetween_shape::InbetweenShape;
use usd_core::Prim;
use usd_gf::{Vec2i, Vec3f, Vec4f};
use usd_sdf::TimeCode;

/// Object identifying a general sub-shape (either primary or inbetween).
#[derive(Clone, Debug)]
struct SubShape {
    /// Index of the blend shape this sub-shape belongs to.
    blend_shape_index: u32,
    /// Index of the inbetween within the blend shape (-1 for primary shape).
    inbetween_index: i32,
    /// The weight at which this sub-shape is fully active.
    weight: f32,
}

impl SubShape {
    fn new(blend_shape_index: u32, inbetween_index: i32, weight: f32) -> Self {
        Self {
            blend_shape_index,
            inbetween_index,
            weight,
        }
    }

    fn get_blend_shape_index(&self) -> usize {
        self.blend_shape_index as usize
    }

    fn get_inbetween_index(&self) -> i32 {
        self.inbetween_index
    }

    fn is_inbetween(&self) -> bool {
        self.inbetween_index >= 0
    }

    fn is_null_shape(&self) -> bool {
        self.weight == 0.0
    }

    #[allow(dead_code)] // C++ parity - blend shape weight classification
    fn is_primary_shape(&self) -> bool {
        self.weight == 1.0
    }

    fn get_weight(&self) -> f32 {
        self.weight
    }
}

/// Internal blend shape data.
#[derive(Clone, Debug)]
struct BlendShapeData {
    /// The blend shape prim.
    shape: BlendShape,
    /// Index of the first sub-shape in the sub-shapes array.
    first_sub_shape: usize,
    /// Number of sub-shapes (including primary and inbetweens).
    num_sub_shapes: usize,
}

/// Helper class used to resolve blend shape weights, including inbetweens.
///
/// Matches C++ `UsdSkelBlendShapeQuery`.
#[derive(Clone)]
pub struct BlendShapeQuery {
    /// The prim the blend shapes apply to.
    prim: Option<Prim>,
    /// Sub-shapes (primary shapes and inbetweens).
    sub_shapes: Vec<SubShape>,
    /// Blend shape data.
    blend_shapes: Vec<BlendShapeData>,
    /// All inbetween shapes.
    inbetweens: Vec<InbetweenShape>,
}

impl Default for BlendShapeQuery {
    fn default() -> Self {
        Self::new()
    }
}

impl BlendShapeQuery {
    /// Create an invalid blend shape query.
    pub fn new() -> Self {
        Self {
            prim: None,
            sub_shapes: Vec::new(),
            blend_shapes: Vec::new(),
            inbetweens: Vec::new(),
        }
    }

    /// Create a blend shape query from a binding API.
    pub fn from_binding(binding: &BindingAPI) -> Self {
        let prim = binding.prim().clone();
        if !prim.is_valid() {
            return Self::new();
        }

        // Get blend shape targets from the binding
        let blend_shape_targets = binding
            .get_blend_shape_targets_rel()
            .map(|rel| rel.get_targets())
            .unwrap_or_default();

        if blend_shape_targets.is_empty() {
            return Self::new();
        }

        let mut query = Self {
            prim: Some(prim.clone()),
            sub_shapes: Vec::new(),
            blend_shapes: Vec::with_capacity(blend_shape_targets.len()),
            inbetweens: Vec::new(),
        };

        // Process each blend shape target
        for (blend_shape_idx, target_path) in blend_shape_targets.iter().enumerate() {
            let stage = prim.stage().expect("prim has stage");
            let Some(target_prim) = stage.get_prim_at_path(target_path) else {
                continue;
            };

            let blend_shape = BlendShape::new(target_prim);
            if !blend_shape.is_valid() {
                continue;
            }

            let first_sub_shape = query.sub_shapes.len();

            // Add null shape (weight 0)
            query.sub_shapes.push(SubShape::new(
                blend_shape_idx as u32,
                -1, // Not an inbetween
                0.0,
            ));

            // Get inbetweens for this blend shape
            let inbetweens = blend_shape.get_inbetweens();
            let mut sorted_inbetweens: Vec<(InbetweenShape, f32)> = inbetweens
                .into_iter()
                .filter_map(|ib| {
                    if let Some(weight) = ib.get_weight() {
                        if weight > 0.0 && weight < 1.0 {
                            return Some((ib, weight));
                        }
                    }
                    None
                })
                .collect::<Vec<_>>();

            // Sort by weight
            sorted_inbetweens
                .sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

            // Add inbetween sub-shapes
            for (inbetween, weight) in sorted_inbetweens.iter() {
                query.sub_shapes.push(SubShape::new(
                    blend_shape_idx as u32,
                    query.inbetweens.len() as i32,
                    *weight,
                ));
                query.inbetweens.push(inbetween.clone());
            }

            // Add primary shape (weight 1)
            query.sub_shapes.push(SubShape::new(
                blend_shape_idx as u32,
                -1, // Primary shape, not inbetween
                1.0,
            ));

            query.blend_shapes.push(BlendShapeData {
                shape: blend_shape,
                first_sub_shape,
                num_sub_shapes: query.sub_shapes.len() - first_sub_shape,
            });
        }

        query
    }

    /// Return true if this query is valid.
    pub fn is_valid(&self) -> bool {
        self.prim.is_some()
    }

    /// Returns the prim the blend shapes apply to.
    pub fn get_prim(&self) -> Option<&Prim> {
        self.prim.as_ref()
    }

    /// Returns the blend shape corresponding to the given index.
    pub fn get_blend_shape(&self, blend_shape_index: usize) -> Option<&BlendShape> {
        self.blend_shapes.get(blend_shape_index).map(|bs| &bs.shape)
    }

    /// Returns the inbetween shape corresponding to the given sub-shape index.
    pub fn get_inbetween(&self, sub_shape_index: usize) -> Option<&InbetweenShape> {
        let sub_shape = self.sub_shapes.get(sub_shape_index)?;
        if sub_shape.is_inbetween() {
            self.inbetweens
                .get(sub_shape.get_inbetween_index() as usize)
        } else {
            None
        }
    }

    /// Returns the blend shape index for the given sub-shape index.
    pub fn get_blend_shape_index(&self, sub_shape_index: usize) -> Option<usize> {
        self.sub_shapes
            .get(sub_shape_index)
            .map(|ss| ss.get_blend_shape_index())
    }

    /// Get the number of blend shapes.
    pub fn get_num_blend_shapes(&self) -> usize {
        self.blend_shapes.len()
    }

    /// Get the number of sub-shapes.
    pub fn get_num_sub_shapes(&self) -> usize {
        self.sub_shapes.len()
    }

    /// Compute point indices for all blend shapes.
    pub fn compute_blend_shape_point_indices(&self) -> Vec<Vec<i32>> {
        self.blend_shapes
            .iter()
            .map(|bs| {
                if bs.shape.get_point_indices_attr().is_valid() {
                    bs.shape
                        .get_point_indices_attr()
                        .get_typed_vec::<i32>(TimeCode::default())
                        .unwrap_or_default()
                } else {
                    Vec::new()
                }
            })
            .collect()
    }

    /// Compute point offsets for all sub-shapes.
    pub fn compute_sub_shape_point_offsets(&self) -> Vec<Vec<Vec3f>> {
        self.sub_shapes
            .iter()
            .map(|ss| {
                if ss.is_null_shape() {
                    // Null shape has no offsets
                    Vec::new()
                } else if ss.is_inbetween() {
                    // Get inbetween offsets
                    let ib_idx = ss.get_inbetween_index() as usize;
                    if let Some(ib) = self.inbetweens.get(ib_idx) {
                        ib.get_offsets().unwrap_or_default()
                    } else {
                        Vec::new()
                    }
                } else {
                    // Primary shape - get from blend shape
                    let bs_idx = ss.get_blend_shape_index();
                    if bs_idx >= self.blend_shapes.len() {
                        return Vec::new();
                    }
                    let bs = &self.blend_shapes[bs_idx];
                    if bs.shape.get_offsets_attr().is_valid() {
                        bs.shape
                            .get_offsets_attr()
                            .get_typed_vec::<Vec3f>(TimeCode::default())
                            .unwrap_or_default()
                    } else {
                        Vec::new()
                    }
                }
            })
            .collect()
    }

    /// Compute normal offsets for all sub-shapes.
    ///
    /// Matches C++ `ComputeSubShapeNormalOffsets()` — includes normals for both
    /// primary shapes and inbetween shapes. Null shapes return empty arrays.
    pub fn compute_sub_shape_normal_offsets(&self) -> Vec<Vec<Vec3f>> {
        self.sub_shapes
            .iter()
            .map(|ss| {
                if ss.is_null_shape() {
                    // Null shape has no offsets
                    Vec::new()
                } else if ss.is_inbetween() {
                    // Inbetween shapes can have normal offsets
                    let ib_idx = ss.get_inbetween_index() as usize;
                    if let Some(ib) = self.inbetweens.get(ib_idx) {
                        ib.get_normal_offsets().unwrap_or_default()
                    } else {
                        Vec::new()
                    }
                } else {
                    // Primary shape - get from blend shape
                    let bs_idx = ss.get_blend_shape_index();
                    if bs_idx >= self.blend_shapes.len() {
                        return Vec::new();
                    }
                    let bs = &self.blend_shapes[bs_idx];
                    if bs.shape.get_normal_offsets_attr().is_valid() {
                        bs.shape
                            .get_normal_offsets_attr()
                            .get_typed_vec::<Vec3f>(TimeCode::default())
                            .unwrap_or_default()
                    } else {
                        Vec::new()
                    }
                }
            })
            .collect()
    }

    /// Compute resolved weights for all sub-shapes.
    ///
    /// The `weights` are initial weight values ordered according to blend shape targets.
    /// Returns computed sub-shape weights, blend shape indices, and sub-shape indices.
    /// Matches C++ `ComputeSubShapeWeights()` using upper_bound search.
    pub fn compute_sub_shape_weights(
        &self,
        weights: &[f32],
        sub_shape_weights: &mut Vec<f32>,
        blend_shape_indices: &mut Vec<u32>,
        sub_shape_indices: &mut Vec<u32>,
    ) -> bool {
        sub_shape_weights.clear();
        blend_shape_indices.clear();
        sub_shape_indices.clear();

        if weights.len() != self.blend_shapes.len() {
            eprintln!(
                "Size of weights [{}] != number of blend shapes [{}]",
                weights.len(),
                self.blend_shapes.len()
            );
            return false;
        }

        sub_shape_weights.reserve(weights.len() * 2);
        blend_shape_indices.reserve(weights.len() * 2);
        sub_shape_indices.reserve(weights.len() * 2);

        const EPS: f32 = 1e-6;

        for (bs_idx, bs_data) in self.blend_shapes.iter().enumerate() {
            let sub_shapes = &self.sub_shapes[bs_data.first_sub_shape..][..bs_data.num_sub_shapes];

            // Fast path: no inbetweens (only null + primary = 2 sub-shapes)
            if bs_data.num_sub_shapes < 3 {
                // The second sub-shape should be the primary shape (weight=1)
                let global_idx = bs_data.first_sub_shape + 1;
                sub_shape_weights.push(weights[bs_idx]);
                blend_shape_indices.push(bs_idx as u32);
                sub_shape_indices.push(global_idx as u32);
                continue;
            }

            let w = weights[bs_idx];

            // upper_bound: find first sub-shape with weight > w
            let upper_pos = sub_shapes.iter().position(|ss| ss.get_weight() > w);

            let (lower, upper, lower_global, upper_global) = match upper_pos {
                Some(pos) if pos > 0 => (
                    &sub_shapes[pos - 1],
                    &sub_shapes[pos],
                    bs_data.first_sub_shape + pos - 1,
                    bs_data.first_sub_shape + pos,
                ),
                Some(_) => {
                    // w is below the first sub-shape weight
                    (
                        &sub_shapes[0],
                        &sub_shapes[1],
                        bs_data.first_sub_shape,
                        bs_data.first_sub_shape + 1,
                    )
                }
                None => {
                    // w is above or equal to last sub-shape weight
                    let n = sub_shapes.len();
                    (
                        &sub_shapes[n - 2],
                        &sub_shapes[n - 1],
                        bs_data.first_sub_shape + n - 2,
                        bs_data.first_sub_shape + n - 1,
                    )
                }
            };

            let weight_delta = upper.get_weight() - lower.get_weight();

            if weight_delta > EPS {
                // Compute normalized position between shapes
                let alpha = (w - lower.get_weight()) / weight_delta;

                if !lower.is_null_shape() && (alpha - 1.0).abs() > EPS {
                    sub_shape_weights.push(1.0 - alpha);
                    blend_shape_indices.push(bs_idx as u32);
                    sub_shape_indices.push(lower_global as u32);
                }
                if !upper.is_null_shape() && alpha.abs() > EPS {
                    sub_shape_weights.push(alpha);
                    blend_shape_indices.push(bs_idx as u32);
                    sub_shape_indices.push(upper_global as u32);
                }
            }
        }

        true
    }

    /// Compute flattened array of weights for all sub-shapes.
    ///
    /// Creates a dense array of size `get_num_sub_shapes()` with weights scattered
    /// by sub-shape index. Matches C++ `ComputeFlattenedSubShapeWeights()`.
    pub fn compute_flattened_sub_shape_weights(
        &self,
        weights: &[f32],
        sub_shape_weights: &mut Vec<f32>,
    ) -> bool {
        let mut sparse_weights = Vec::new();
        let mut blend_shape_indices = Vec::new();
        let mut sparse_sub_shape_indices = Vec::new();

        if !self.compute_sub_shape_weights(
            weights,
            &mut sparse_weights,
            &mut blend_shape_indices,
            &mut sparse_sub_shape_indices,
        ) {
            return false;
        }

        // Build dense array of size num_sub_shapes, scatter sparse weights by index
        let num_sub_shapes = self.sub_shapes.len();
        sub_shape_weights.clear();
        sub_shape_weights.resize(num_sub_shapes, 0.0f32);
        for i in 0..sparse_weights.len() {
            let ss_idx = sparse_sub_shape_indices[i] as usize;
            if ss_idx < num_sub_shapes {
                sub_shape_weights[ss_idx] = sparse_weights[i];
            }
        }
        true
    }

    /// Deform points using resolved sub-shapes.
    pub fn compute_deformed_points(
        &self,
        sub_shape_weights: &[f32],
        blend_shape_indices: &[u32],
        sub_shape_indices: &[u32],
        blend_shape_point_indices: &[Vec<i32>],
        sub_shape_point_offsets: &[Vec<Vec3f>],
        points: &mut [Vec3f],
    ) -> bool {
        if sub_shape_weights.len() != blend_shape_indices.len()
            || sub_shape_weights.len() != sub_shape_indices.len()
        {
            return false;
        }

        for i in 0..sub_shape_weights.len() {
            let weight = sub_shape_weights[i];
            let bs_idx = blend_shape_indices[i] as usize;
            let ss_idx = sub_shape_indices[i] as usize;

            if weight.abs() < f32::EPSILON {
                continue;
            }

            let point_indices = blend_shape_point_indices.get(bs_idx);
            let offsets = match sub_shape_point_offsets.get(ss_idx) {
                Some(o) => o,
                None => continue,
            };

            if let Some(indices) = point_indices {
                // Sparse offsets
                for (offset_idx, &point_idx) in indices.iter().enumerate() {
                    if point_idx >= 0 && (point_idx as usize) < points.len() {
                        if let Some(offset) = offsets.get(offset_idx) {
                            let pt = &mut points[point_idx as usize];
                            *pt += *offset * weight;
                        }
                    }
                }
            } else if !offsets.is_empty() {
                // Dense offsets
                for (point_idx, offset) in offsets.iter().enumerate() {
                    if point_idx < points.len() {
                        let pt = &mut points[point_idx];
                        *pt += *offset * weight;
                    }
                }
            }
        }

        true
    }

    /// Deform normals using resolved sub-shapes (normalizes after deformation).
    pub fn compute_deformed_normals(
        &self,
        sub_shape_weights: &[f32],
        blend_shape_indices: &[u32],
        sub_shape_indices: &[u32],
        blend_shape_point_indices: &[Vec<i32>],
        sub_shape_normal_offsets: &[Vec<Vec3f>],
        normals: &mut [Vec3f],
    ) -> bool {
        if !self.compute_deformed_points(
            sub_shape_weights,
            blend_shape_indices,
            sub_shape_indices,
            blend_shape_point_indices,
            sub_shape_normal_offsets,
            normals,
        ) {
            return false;
        }

        // Normalize all normals
        for normal in normals.iter_mut() {
            let len = normal.length();
            if len > f32::EPSILON {
                *normal /= len;
            }
        }

        true
    }

    /// Pack all blend shape offsets into a GPU-friendly flat table.
    ///
    /// `offsets` receives Vec4f entries where xyz = point offset and w = sub-shape index.
    /// `ranges` receives Vec2i per point where [0] = start index and [1] = end index
    /// into the offsets table.
    ///
    /// Matches C++ `ComputePackedShapeTable()`.
    pub fn compute_packed_shape_table(
        &self,
        offsets: &mut Vec<Vec4f>,
        ranges: &mut Vec<Vec2i>,
    ) -> bool {
        let indices_per_bs = self.compute_blend_shape_point_indices();
        let offsets_per_ss = self.compute_sub_shape_point_offsets();

        // Compute approximate number of points
        let num_points = {
            let mut max_index: i32 = 0;
            for indices in &indices_per_bs {
                for &idx in indices {
                    max_index = max_index.max(idx);
                }
            }
            // Also consider non-sparse shapes (offset array sizes)
            for off in &offsets_per_ss {
                max_index = max_index.max(off.len() as i32);
            }
            if max_index > 0 {
                (max_index + 1) as usize
            } else {
                0
            }
        };

        if num_points == 0 {
            offsets.clear();
            ranges.clear();
            return true;
        }

        // Count non-null sub-shapes per blend shape
        let mut num_ss_per_bs = vec![0u32; self.blend_shapes.len()];
        for ss in &self.sub_shapes {
            if !ss.is_null_shape() {
                let bs_idx = ss.get_blend_shape_index();
                if bs_idx >= self.blend_shapes.len() {
                    continue;
                }
                num_ss_per_bs[bs_idx] += 1;
            }
        }

        // Count offsets per point
        let mut num_offsets_per_point = vec![0u32; num_points];
        for (bs_idx, _) in self.blend_shapes.iter().enumerate() {
            let n = num_ss_per_bs[bs_idx];
            let indices = &indices_per_bs[bs_idx];
            if indices.is_empty() {
                // Non-sparse: every point gets these sub-shapes
                for count in num_offsets_per_point.iter_mut() {
                    *count += n;
                }
            } else {
                for &idx in indices {
                    if idx >= 0 && (idx as usize) < num_points {
                        num_offsets_per_point[idx as usize] += n;
                    }
                }
            }
        }

        // Compute ranges from counts
        ranges.resize(num_points, Vec2i::new(0, 0));
        let mut start = 0u32;
        for (i, &count) in num_offsets_per_point.iter().enumerate() {
            ranges[i] = Vec2i::new(start as i32, (start + count) as i32);
            start += count;
        }
        let total_offsets = start as usize;

        // Track next write position per point
        let mut next_offset_per_point: Vec<u32> = ranges.iter().map(|r| r.x as u32).collect();

        // Fill packed offset table
        offsets.resize(total_offsets, Vec4f::new(0.0, 0.0, 0.0, 0.0));

        for (ss_idx, ss) in self.sub_shapes.iter().enumerate() {
            if ss.is_null_shape() {
                continue;
            }

            let ss_offsets = match offsets_per_ss.get(ss_idx) {
                Some(o) if !o.is_empty() => o,
                _ => continue,
            };

            let bs_idx = ss.get_blend_shape_index();
            if bs_idx >= indices_per_bs.len() {
                continue;
            }
            let bs_indices = &indices_per_bs[bs_idx];
            let ss_index_f = ss_idx as f32;

            if bs_indices.is_empty() {
                // Non-sparse: fill for all points
                for (pi, offset) in ss_offsets.iter().enumerate() {
                    if pi < num_points {
                        let oi = next_offset_per_point[pi] as usize;
                        offsets[oi] = Vec4f::new(offset.x, offset.y, offset.z, ss_index_f);
                        next_offset_per_point[pi] += 1;
                    }
                }
            } else {
                // Sparse: use indices
                for (j, &point_idx) in bs_indices.iter().enumerate() {
                    if point_idx >= 0 && (point_idx as usize) < num_points {
                        if let Some(offset) = ss_offsets.get(j) {
                            let pi = point_idx as usize;
                            let oi = next_offset_per_point[pi] as usize;
                            offsets[oi] = Vec4f::new(offset.x, offset.y, offset.z, ss_index_f);
                            next_offset_per_point[pi] += 1;
                        }
                    }
                }
            }
        }

        true
    }

    /// Get a description string.
    pub fn get_description(&self) -> String {
        if let Some(ref prim) = self.prim {
            format!(
                "BlendShapeQuery for {} ({} blend shapes, {} sub-shapes)",
                prim.path().get_string(),
                self.blend_shapes.len(),
                self.sub_shapes.len()
            )
        } else {
            "Invalid BlendShapeQuery".to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invalid_query() {
        let query = BlendShapeQuery::new();
        assert!(!query.is_valid());
        assert_eq!(query.get_num_blend_shapes(), 0);
        assert_eq!(query.get_num_sub_shapes(), 0);
    }
}
