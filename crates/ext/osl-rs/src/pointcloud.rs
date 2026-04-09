//! Pointcloud operations matching OSL's `pointcloud.cpp`.
//!
//! Provides pointcloud_search, pointcloud_get, and pointcloud_write.
//!
//! # In-memory only
//!
//! Unlike C++ OSL which delegates to Partio for on-disk `.bgeo`/`.ptc` files,
//! this implementation stores points entirely in memory. There is no file I/O;
//! points must be added via `pointcloud_write` at runtime. This is sufficient
//! for procedural point generation and shader-to-shader communication but
//! cannot load pre-existing particle caches from disk.
//!
//! # Performance
//!
//! Uses a grid-based spatial hash for O(1) average-case nearest-neighbor
//! queries. Call [`PointCloud::build_grid`] before searching for best
//! performance. Falls back to O(N) brute-force when the grid is not built.

use crate::Float;
use crate::math::Vec3;
use crate::ustring::UString;
use std::collections::HashMap;

/// A single point in the point cloud.
#[derive(Debug, Clone)]
pub struct CloudPoint {
    pub position: Vec3,
    pub attributes: HashMap<UString, PointData>,
}

/// Data stored per point per attribute.
#[derive(Debug, Clone)]
pub enum PointData {
    Float(f32),
    FloatArray(Vec<f32>),
    Int(i32),
    String(UString),
    Vec3(Vec3),
}

impl PointData {
    /// Number of float components.
    pub fn float_count(&self) -> usize {
        match self {
            PointData::Float(_) => 1,
            PointData::FloatArray(v) => v.len(),
            PointData::Int(_) => 1,
            PointData::String(_) => 0,
            PointData::Vec3(_) => 3,
        }
    }
}

/// Grid-based spatial hash for O(1) average nearest-neighbor lookups.
#[derive(Debug, Clone)]
pub struct SpatialGrid {
    cell_size: f32,
    inv_cell: f32,
    cells: HashMap<(i32, i32, i32), Vec<usize>>,
}

impl SpatialGrid {
    /// Build a spatial grid from point positions.
    pub fn build(points: &[CloudPoint], cell_size: f32) -> Self {
        let cell_size = cell_size.max(f32::EPSILON);
        let inv_cell = 1.0 / cell_size;
        let mut cells: HashMap<(i32, i32, i32), Vec<usize>> = HashMap::new();
        for (i, pt) in points.iter().enumerate() {
            let key = Self::cell_key(pt.position, inv_cell);
            cells.entry(key).or_default().push(i);
        }
        Self {
            cell_size,
            inv_cell,
            cells,
        }
    }

    /// Auto-select cell_size from point density (2x average spacing).
    pub fn build_auto(points: &[CloudPoint]) -> Self {
        let cell_size = Self::estimate_cell_size(points);
        Self::build(points, cell_size)
    }

    /// Find all point indices within `radius` of `center`.
    pub fn query_radius(&self, points: &[CloudPoint], center: Vec3, radius: f32) -> Vec<usize> {
        let radius_sq = radius * radius;
        let r = radius;
        // Cell range to check
        let min_c = Self::cell_key(
            Vec3::new(center.x - r, center.y - r, center.z - r),
            self.inv_cell,
        );
        let max_c = Self::cell_key(
            Vec3::new(center.x + r, center.y + r, center.z + r),
            self.inv_cell,
        );
        let mut result = Vec::new();
        for cx in min_c.0..=max_c.0 {
            for cy in min_c.1..=max_c.1 {
                for cz in min_c.2..=max_c.2 {
                    if let Some(indices) = self.cells.get(&(cx, cy, cz)) {
                        for &i in indices {
                            let diff = points[i].position - center;
                            if diff.dot(diff) <= radius_sq {
                                result.push(i);
                            }
                        }
                    }
                }
            }
        }
        result
    }

    /// Find up to `k` nearest neighbors within `max_radius`.
    /// Returns (index, squared_distance) pairs sorted by distance.
    pub fn query_nearest(
        &self,
        points: &[CloudPoint],
        center: Vec3,
        k: usize,
        max_radius: f32,
    ) -> Vec<(usize, f32)> {
        let indices = self.query_radius(points, center, max_radius);
        let mut dists: Vec<(usize, f32)> = indices
            .into_iter()
            .map(|i| {
                let diff = points[i].position - center;
                (i, diff.dot(diff))
            })
            .collect();
        dists.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        dists.truncate(k);
        dists
    }

    /// Number of occupied cells.
    pub fn cell_count(&self) -> usize {
        self.cells.len()
    }

    /// Cell size used by this grid.
    pub fn cell_size(&self) -> f32 {
        self.cell_size
    }

    #[inline]
    fn cell_key(pos: Vec3, inv_cell: f32) -> (i32, i32, i32) {
        (
            (pos.x * inv_cell).floor() as i32,
            (pos.y * inv_cell).floor() as i32,
            (pos.z * inv_cell).floor() as i32,
        )
    }

    /// Estimate cell size from bounding box and point count.
    fn estimate_cell_size(points: &[CloudPoint]) -> f32 {
        if points.len() < 2 {
            return 1.0;
        }
        let mut min = points[0].position;
        let mut max = points[0].position;
        for pt in &points[1..] {
            min.x = min.x.min(pt.position.x);
            min.y = min.y.min(pt.position.y);
            min.z = min.z.min(pt.position.z);
            max.x = max.x.max(pt.position.x);
            max.y = max.y.max(pt.position.y);
            max.z = max.z.max(pt.position.z);
        }
        let extent = Vec3::new(max.x - min.x, max.y - min.y, max.z - min.z);
        // Average extent per axis, divided by cbrt(n), times 2
        let avg_extent = (extent.x + extent.y + extent.z) / 3.0;
        let spacing = avg_extent / (points.len() as f32).cbrt();
        (spacing * 2.0).max(f32::EPSILON)
    }
}

/// A point cloud -- collection of points with named attributes.
#[derive(Debug, Clone, Default)]
pub struct PointCloud {
    pub points: Vec<CloudPoint>,
    grid: Option<SpatialGrid>,
}

impl PointCloud {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a point to the cloud. Invalidates the spatial grid.
    pub fn add_point(&mut self, position: Vec3, attributes: HashMap<UString, PointData>) {
        self.points.push(CloudPoint {
            position,
            attributes,
        });
        // Grid is stale after adding points
        self.grid = None;
    }

    /// Build spatial grid with explicit cell size.
    pub fn build_grid(&mut self, cell_size: f32) {
        self.grid = Some(SpatialGrid::build(&self.points, cell_size));
    }

    /// Build spatial grid with auto-estimated cell size from point density.
    pub fn build_grid_auto(&mut self) {
        self.grid = Some(SpatialGrid::build_auto(&self.points));
    }

    /// Access the spatial grid, if built.
    pub fn grid(&self) -> Option<&SpatialGrid> {
        self.grid.as_ref()
    }

    /// Number of points.
    pub fn len(&self) -> usize {
        self.points.len()
    }

    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }
}

/// Result of a point cloud search.
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// Indices of found points (sorted by distance).
    pub indices: Vec<usize>,
    /// Squared distances to found points.
    pub distances_sq: Vec<f32>,
}

/// Search for nearest points within a radius.
///
/// Returns up to `max_points` nearest points to `center` within `max_dist`.
pub fn pointcloud_search(
    cloud: &PointCloud,
    center: Vec3,
    max_dist: Float,
    max_points: usize,
    sort: bool,
) -> SearchResult {
    let max_dist_sq = max_dist * max_dist;
    let mut candidates: Vec<(usize, f32)> = Vec::new();

    // Use spatial grid if available, otherwise fall back to brute-force
    if let Some(grid) = cloud.grid.as_ref() {
        let indices = grid.query_radius(&cloud.points, center, max_dist);
        for i in indices {
            let diff = cloud.points[i].position - center;
            candidates.push((i, diff.dot(diff)));
        }
    } else {
        // O(N) brute-force fallback
        for (i, pt) in cloud.points.iter().enumerate() {
            let diff = pt.position - center;
            let dist_sq = diff.dot(diff);
            if dist_sq <= max_dist_sq {
                candidates.push((i, dist_sq));
            }
        }
    }

    // Sort by distance
    candidates.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

    // Truncate to max_points
    candidates.truncate(max_points);

    if !sort {
        // Restore original order (not sorted by distance)
        candidates.sort_by_key(|&(i, _)| i);
    }

    SearchResult {
        indices: candidates.iter().map(|&(i, _)| i).collect(),
        distances_sq: candidates.iter().map(|&(_, d)| d).collect(),
    }
}

/// Get attribute values for previously found points.
///
/// Returns a vector of PointData for each found point index.
pub fn pointcloud_get<'a>(
    cloud: &'a PointCloud,
    indices: &[usize],
    attr_name: UString,
) -> Vec<Option<&'a PointData>> {
    indices
        .iter()
        .map(|&i| {
            cloud
                .points
                .get(i)
                .and_then(|pt| pt.attributes.get(&attr_name))
        })
        .collect()
}

/// Write a point to a point cloud.
pub fn pointcloud_write(
    cloud: &mut PointCloud,
    position: Vec3,
    attributes: HashMap<UString, PointData>,
) -> bool {
    cloud.add_point(position, attributes);
    true
}

/// Point cloud manager — caches loaded point clouds.
#[derive(Default)]
pub struct PointCloudManager {
    clouds: HashMap<UString, PointCloud>,
}

impl PointCloudManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Get or create a named point cloud.
    pub fn get_or_create(&mut self, name: &str) -> &mut PointCloud {
        let uname = UString::new(name);
        self.clouds.entry(uname).or_default()
    }

    /// Get a point cloud by name (read-only).
    pub fn get(&self, name: &str) -> Option<&PointCloud> {
        let uname = UString::new(name);
        self.clouds.get(&uname)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_cloud() -> PointCloud {
        let mut cloud = PointCloud::new();
        for i in 0..10 {
            let pos = Vec3::new(i as f32, 0.0, 0.0);
            let mut attrs = HashMap::new();
            attrs.insert(UString::new("id"), PointData::Int(i));
            attrs.insert(UString::new("value"), PointData::Float(i as f32 * 0.1));
            cloud.add_point(pos, attrs);
        }
        cloud
    }

    #[test]
    fn test_search() {
        let cloud = make_cloud();
        let result = pointcloud_search(&cloud, Vec3::new(2.5, 0.0, 0.0), 2.0, 10, true);
        assert!(!result.indices.is_empty());
        // Points at x=1,2,3,4 are within distance 2.0 of x=2.5
        assert!(result.indices.contains(&1));
        assert!(result.indices.contains(&2));
        assert!(result.indices.contains(&3));
        assert!(result.indices.contains(&4));
    }

    #[test]
    fn test_search_max_points() {
        let cloud = make_cloud();
        let result = pointcloud_search(&cloud, Vec3::new(5.0, 0.0, 0.0), 100.0, 3, true);
        assert_eq!(result.indices.len(), 3);
        // Should be the 3 nearest: indices 4, 5, 6
        assert!(result.distances_sq[0] <= result.distances_sq[1]);
    }

    #[test]
    fn test_get_attribute() {
        let cloud = make_cloud();
        let result = pointcloud_search(&cloud, Vec3::ZERO, 1.5, 10, true);
        let vals = pointcloud_get(&cloud, &result.indices, UString::new("id"));
        assert!(!vals.is_empty());
        for v in &vals {
            assert!(v.is_some());
        }
    }

    #[test]
    fn test_write() {
        let mut cloud = PointCloud::new();
        let mut attrs = HashMap::new();
        attrs.insert(UString::new("test"), PointData::Float(42.0));
        assert!(pointcloud_write(
            &mut cloud,
            Vec3::new(1.0, 2.0, 3.0),
            attrs
        ));
        assert_eq!(cloud.len(), 1);
    }

    #[test]
    fn test_manager() {
        let mut mgr = PointCloudManager::new();
        let cloud = mgr.get_or_create("test");
        cloud.add_point(Vec3::ZERO, HashMap::new());
        assert_eq!(mgr.get("test").unwrap().len(), 1);
        assert!(mgr.get("nonexistent").is_none());
    }

    // --- SpatialGrid tests ---

    fn make_cloud_with_grid() -> PointCloud {
        let mut cloud = make_cloud();
        cloud.build_grid_auto();
        cloud
    }

    #[test]
    fn test_grid_build() {
        let cloud = make_cloud_with_grid();
        let grid = cloud.grid().unwrap();
        assert!(grid.cell_count() > 0);
        assert!(grid.cell_size() > 0.0);
    }

    #[test]
    fn test_grid_build_explicit_cell_size() {
        let mut cloud = make_cloud();
        cloud.build_grid(2.0);
        let grid = cloud.grid().unwrap();
        assert!((grid.cell_size() - 2.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_grid_invalidated_on_add() {
        let mut cloud = make_cloud_with_grid();
        assert!(cloud.grid().is_some());
        cloud.add_point(Vec3::new(99.0, 0.0, 0.0), HashMap::new());
        assert!(cloud.grid().is_none());
    }

    #[test]
    fn test_grid_query_radius() {
        let cloud = make_cloud_with_grid();
        let grid = cloud.grid().unwrap();
        let found = grid.query_radius(&cloud.points, Vec3::new(2.5, 0.0, 0.0), 2.0);
        // x=1,2,3,4 are within distance 2.0 of x=2.5
        assert!(found.contains(&1));
        assert!(found.contains(&2));
        assert!(found.contains(&3));
        assert!(found.contains(&4));
        // x=0 is at distance 2.5 => outside
        assert!(!found.contains(&0));
    }

    #[test]
    fn test_grid_query_radius_empty() {
        let cloud = make_cloud_with_grid();
        let grid = cloud.grid().unwrap();
        // Search far away from all points
        let found = grid.query_radius(&cloud.points, Vec3::new(100.0, 100.0, 100.0), 1.0);
        assert!(found.is_empty());
    }

    #[test]
    fn test_grid_query_nearest() {
        let cloud = make_cloud_with_grid();
        let grid = cloud.grid().unwrap();
        let nearest = grid.query_nearest(&cloud.points, Vec3::new(4.5, 0.0, 0.0), 3, 100.0);
        assert_eq!(nearest.len(), 3);
        // Should be sorted by distance
        assert!(nearest[0].1 <= nearest[1].1);
        assert!(nearest[1].1 <= nearest[2].1);
        // Closest are indices 4 and 5 (distance 0.5 each)
        assert!(nearest[0].0 == 4 || nearest[0].0 == 5);
    }

    #[test]
    fn test_grid_query_nearest_fewer_than_k() {
        let cloud = make_cloud_with_grid();
        let grid = cloud.grid().unwrap();
        // Only 1 point within radius 0.5 of x=3.0
        let nearest = grid.query_nearest(&cloud.points, Vec3::new(3.0, 0.0, 0.0), 5, 0.5);
        assert_eq!(nearest.len(), 1);
        assert_eq!(nearest[0].0, 3);
    }

    #[test]
    fn test_search_with_grid_matches_brute_force() {
        // Compare grid-accelerated vs brute-force results
        let brute = make_cloud();
        let mut grid_cloud = make_cloud();
        grid_cloud.build_grid(2.0);

        let center = Vec3::new(3.5, 0.0, 0.0);
        let r_brute = pointcloud_search(&brute, center, 2.5, 10, true);
        let r_grid = pointcloud_search(&grid_cloud, center, 2.5, 10, true);

        assert_eq!(r_brute.indices, r_grid.indices);
        assert_eq!(r_brute.distances_sq.len(), r_grid.distances_sq.len());
        for (a, b) in r_brute.distances_sq.iter().zip(&r_grid.distances_sq) {
            assert!((a - b).abs() < 1e-6);
        }
    }

    #[test]
    fn test_search_with_grid_max_points() {
        let mut cloud = make_cloud();
        cloud.build_grid(2.0);
        let result = pointcloud_search(&cloud, Vec3::new(5.0, 0.0, 0.0), 100.0, 3, true);
        assert_eq!(result.indices.len(), 3);
        assert!(result.distances_sq[0] <= result.distances_sq[1]);
    }

    #[test]
    fn test_grid_3d_points() {
        // Test with points in all 3 dimensions
        let mut cloud = PointCloud::new();
        let positions = [
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 1.0, 1.0),
            Vec3::new(2.0, 2.0, 2.0),
            Vec3::new(-1.0, -1.0, -1.0),
            Vec3::new(0.5, 0.5, 0.5),
        ];
        for pos in &positions {
            cloud.add_point(*pos, HashMap::new());
        }
        cloud.build_grid(2.0);
        let grid = cloud.grid().unwrap();

        // Query around origin, radius ~1.0 => should find (0,0,0) and (0.5,0.5,0.5)
        let found = grid.query_radius(&cloud.points, Vec3::ZERO, 1.0);
        assert!(found.contains(&0)); // (0,0,0) dist=0
        assert!(found.contains(&4)); // (0.5,0.5,0.5) dist=0.866
        assert!(!found.contains(&1)); // (1,1,1) dist=1.73 > 1.0
    }

    #[test]
    fn test_grid_negative_coords() {
        // Points with negative coordinates
        let mut cloud = PointCloud::new();
        cloud.add_point(Vec3::new(-5.0, -5.0, -5.0), HashMap::new());
        cloud.add_point(Vec3::new(-4.0, -5.0, -5.0), HashMap::new());
        cloud.add_point(Vec3::new(5.0, 5.0, 5.0), HashMap::new());
        cloud.build_grid(2.0);

        let grid = cloud.grid().unwrap();
        let found = grid.query_radius(&cloud.points, Vec3::new(-4.5, -5.0, -5.0), 1.0);
        assert_eq!(found.len(), 2);
        assert!(found.contains(&0));
        assert!(found.contains(&1));
    }

    #[test]
    fn test_grid_single_point() {
        let mut cloud = PointCloud::new();
        cloud.add_point(Vec3::new(1.0, 2.0, 3.0), HashMap::new());
        cloud.build_grid_auto();
        let grid = cloud.grid().unwrap();
        let found = grid.query_radius(&cloud.points, Vec3::new(1.0, 2.0, 3.0), 0.1);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0], 0);
    }

    #[test]
    fn test_grid_empty_cloud() {
        let mut cloud = PointCloud::new();
        cloud.build_grid(1.0);
        let grid = cloud.grid().unwrap();
        assert_eq!(grid.cell_count(), 0);
        let found = grid.query_radius(&cloud.points, Vec3::ZERO, 10.0);
        assert!(found.is_empty());
    }

    #[test]
    fn test_grid_clone() {
        let cloud = make_cloud_with_grid();
        let cloned = cloud.clone();
        assert!(cloned.grid().is_some());
        let grid = cloned.grid().unwrap();
        let found = grid.query_radius(&cloned.points, Vec3::new(5.0, 0.0, 0.0), 1.5);
        assert!(!found.is_empty());
    }

    #[test]
    fn test_estimate_cell_size() {
        // Uniformly distributed points => cell size should be reasonable
        let mut cloud = PointCloud::new();
        for x in 0..10 {
            for y in 0..10 {
                for z in 0..10 {
                    cloud.add_point(Vec3::new(x as f32, y as f32, z as f32), HashMap::new());
                }
            }
        }
        cloud.build_grid_auto();
        let grid = cloud.grid().unwrap();
        // 1000 points in 9x9x9 box, avg_extent=9, cbrt(1000)~=10, spacing~=0.9, cell~=1.8
        assert!(grid.cell_size() > 0.5);
        assert!(grid.cell_size() < 5.0);
    }

    #[test]
    fn test_grid_search_unsorted() {
        let mut cloud = make_cloud();
        cloud.build_grid(2.0);
        let result = pointcloud_search(&cloud, Vec3::new(5.0, 0.0, 0.0), 100.0, 5, false);
        assert_eq!(result.indices.len(), 5);
        // When unsorted, indices should be in original order
        for i in 1..result.indices.len() {
            assert!(result.indices[i - 1] < result.indices[i]);
        }
    }
}
