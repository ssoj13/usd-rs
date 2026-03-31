//! Helpers for purpose-aware USD world bounds.
//!
//! Mirrors usdviewq behavior: bounds are evaluated against included purposes,
//! not with an empty purpose list.

use crate::data_model::ViewSettingsDataModel;
use usd_geom::bbox_cache::BBoxCache;
use usd_geom::imageable::Imageable;
use usd_geom::tokens::usd_geom_tokens;
use usd_gf::{BBox3d, Vec3d};
use usd_sdf::TimeCode;
use usd_tf::Token;

/// Build included purposes from current view settings.
///
/// Matches usdviewq default semantics: `default` is always included, and
/// `proxy`/`render`/`guide` are toggled by view settings.
pub fn included_purposes_from_view(view: &ViewSettingsDataModel) -> Vec<Token> {
    let t = usd_geom_tokens();
    let mut purposes = vec![t.default_.clone()];
    if view.display_proxy {
        purposes.push(t.proxy.clone());
    }
    if view.display_render {
        purposes.push(t.render.clone());
    }
    if view.display_guide {
        purposes.push(t.guide.clone());
    }
    purposes
}

/// Compute world bound for an imageable using explicit purposes.
pub fn compute_world_bound_for_purposes(
    imageable: &Imageable,
    time: TimeCode,
    purposes: &[Token],
) -> BBox3d {
    let t = usd_geom_tokens();
    let args: Vec<Token> = if purposes.is_empty() {
        vec![t.default_.clone()]
    } else {
        purposes.iter().take(4).cloned().collect()
    };
    imageable.compute_world_bound(time, args.get(0), args.get(1), args.get(2), args.get(3))
}

/// Compute world bound for an imageable using view-settings purposes.
///
/// Passes `use_extents_hint` from view settings to BBoxCache so that
/// authored extentsHint attributes are respected when the toggle is on.
pub fn compute_world_bound_for_view(
    imageable: &Imageable,
    time: TimeCode,
    view: &ViewSettingsDataModel,
) -> BBox3d {
    let purposes = included_purposes_from_view(view);
    let prim = imageable.prim();
    if !prim.is_valid() {
        return BBox3d::new();
    }
    let mut cache = BBoxCache::new(time, purposes, view.use_extents_hint, false);
    cache.compute_world_bound(prim)
}

/// Compute the composed scene bbox that viewer framing and clipping should use.
///
/// This intentionally goes through stage-side `BBoxCache` instead of the engine
/// render-index bbox. Real assets can distribute the same transform differently
/// across formats, for example on the mesh in `.usda` but on a wrapping `Xform`
/// in `.usdc` / `.usdz`. The composed stage bound is stable across those
/// layouts and matches the reference viewer's world-bound logic.
///
/// The pseudo-root is still the preferred whole-stage source, but real files can
/// legitimately have an empty pseudo-root range while their `defaultPrim`
/// carries the actual composed scene extent. In that case, falling back to the
/// default prim keeps framing and auto-clipping tied to the same visible scene
/// the user loaded instead of regressing to the hardcoded `1 / 2_000_000`
/// clipping defaults.
pub fn compute_stage_bbox_for_view(
    stage: &usd_core::Stage,
    time: TimeCode,
    view: &ViewSettingsDataModel,
) -> Option<(Vec3d, Vec3d)> {
    let root = stage.get_pseudo_root();
    let imageable = Imageable::new(root);
    if imageable.is_valid() {
        if let Some(bounds) =
            aligned_range_to_bounds(compute_world_bound_for_view(&imageable, time, view))
        {
            return Some(bounds);
        }
    }

    let default_prim = stage.get_default_prim();
    let imageable = Imageable::new(default_prim);
    if !imageable.is_valid() {
        return None;
    }

    aligned_range_to_bounds(compute_world_bound_for_view(&imageable, time, view))
}

/// Convert a USD bbox into a validated aligned `(min, max)` pair.
///
/// Keeping the validation in one helper ensures framing and clipping use the
/// same acceptance criteria for empty, non-finite, or degenerate bounds.
pub fn aligned_range_to_bounds(bbox: BBox3d) -> Option<(Vec3d, Vec3d)> {
    let range = bbox.compute_aligned_range();
    if range.is_empty() {
        return None;
    }

    let min = *range.min();
    let max = *range.max();
    if !is_finite_vec3(&min) || !is_finite_vec3(&max) {
        return None;
    }
    if !is_reasonable_bbox(&min, &max) {
        return None;
    }

    let diag = max - min;
    if diag.x.abs() < 1e-12 && diag.y.abs() < 1e-12 && diag.z.abs() < 1e-12 {
        return None;
    }

    Some((min, max))
}

#[inline]
pub fn is_finite_vec3(v: &Vec3d) -> bool {
    v.x.is_finite() && v.y.is_finite() && v.z.is_finite()
}

#[inline]
pub fn is_reasonable_bbox(min: &Vec3d, max: &Vec3d) -> bool {
    [min.x, min.y, min.z, max.x, max.y, max.z]
        .iter()
        .all(|v| v.is_finite() && v.abs() <= 1.0e12)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_model::ViewSettingsDataModel;

    #[test]
    fn test_included_purposes_default_matches_usdviewq_style() {
        let view = ViewSettingsDataModel::default();
        let purposes = included_purposes_from_view(&view);
        let t = usd_geom_tokens();
        assert!(purposes.contains(&t.default_));
        assert!(purposes.contains(&t.proxy));
        assert!(!purposes.contains(&t.render));
        assert!(!purposes.contains(&t.guide));
    }

    #[test]
    fn test_included_purposes_respects_toggles() {
        let mut view = ViewSettingsDataModel::default();
        view.display_render = true;
        view.display_guide = true;
        let purposes = included_purposes_from_view(&view);
        let t = usd_geom_tokens();
        assert!(purposes.contains(&t.default_));
        assert!(purposes.contains(&t.proxy));
        assert!(purposes.contains(&t.render));
        assert!(purposes.contains(&t.guide));
    }
}
