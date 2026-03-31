//! Layer stitching utilities.
//!
//! Provides utilities for merging layers together, with the convention of
//! a strong and weak layer where the strong layer has precedence.

use std::sync::Arc;

use usd_sdf::Specifier;
use usd_sdf::layer::Layer;
use usd_sdf::path::Path;
use usd_sdf::prim_spec::PrimSpec;
use usd_tf::Token;
use usd_vt::value::Value;

/// Status indicating the desired value stitching behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StitchValueStatus {
    /// Don't stitch values for this field.
    NoStitchedValue,
    /// Use the default stitching behavior for this field.
    UseDefaultValue,
    /// Use the value supplied in the stitched_value parameter.
    UseSuppliedValue,
}

/// Callback for customizing how values are stitched together.
pub type StitchValueFn = dyn Fn(
        &Token,      // field
        &Path,       // path
        &Arc<Layer>, // strong_layer
        bool,        // field_in_strong_layer
        &Arc<Layer>, // weak_layer
        bool,        // field_in_weak_layer
    ) -> (StitchValueStatus, Option<Value>)
    + Send
    + Sync;

/// A boxed stitch value function.
pub type BoxedStitchValueFn = Box<StitchValueFn>;

/// Merges all scene description from a weak layer into a strong layer.
pub fn stitch_layers(strong_layer: &Arc<Layer>, weak_layer: &Arc<Layer>) {
    stitch_layers_with_fn(strong_layer, weak_layer, None);
}

/// Advanced version of [`stitch_layers`] with a custom stitch function.
pub fn stitch_layers_with_fn(
    strong_layer: &Arc<Layer>,
    weak_layer: &Arc<Layer>,
    stitch_value_fn: Option<&BoxedStitchValueFn>,
) {
    stitch_layer_metadata(strong_layer, weak_layer, stitch_value_fn);

    // Get root prims from weak layer and process recursively
    let weak_prims = weak_layer.root_prims();

    for weak_spec in weak_prims {
        let prim_path = weak_spec.path().clone();
        let strong_spec = strong_layer.get_prim_at_path(&prim_path);

        if let Some(strong) = strong_spec {
            stitch_prim_spec(&strong, &weak_spec, stitch_value_fn);
        } else {
            copy_prim_spec(strong_layer, &prim_path, &weak_spec);
        }
    }
}

/// Merges scene description for a weak spec into a strong spec.
pub fn stitch_info(strong_obj: &PrimSpec, weak_obj: &PrimSpec) {
    stitch_info_with_fn(strong_obj, weak_obj, None);
}

/// Advanced version of [`stitch_info`] with a custom stitch function.
pub fn stitch_info_with_fn(
    strong_obj: &PrimSpec,
    weak_obj: &PrimSpec,
    stitch_value_fn: Option<&BoxedStitchValueFn>,
) {
    stitch_prim_spec(strong_obj, weak_obj, stitch_value_fn);
}

/// Stitches layer metadata from weak to strong layer.
fn stitch_layer_metadata(
    strong_layer: &Arc<Layer>,
    weak_layer: &Arc<Layer>,
    _stitch_value_fn: Option<&BoxedStitchValueFn>,
) {
    // Merge time code ranges - take min of starts and max of ends
    let strong_has_start = strong_layer.has_start_time_code();
    let weak_has_start = weak_layer.has_start_time_code();

    if strong_has_start && weak_has_start {
        let strong_start = strong_layer.get_start_time_code();
        let weak_start = weak_layer.get_start_time_code();
        strong_layer.set_start_time_code(strong_start.min(weak_start));
    } else if weak_has_start {
        strong_layer.set_start_time_code(weak_layer.get_start_time_code());
    }

    let strong_has_end = strong_layer.has_end_time_code();
    let weak_has_end = weak_layer.has_end_time_code();

    if strong_has_end && weak_has_end {
        let strong_end = strong_layer.get_end_time_code();
        let weak_end = weak_layer.get_end_time_code();
        strong_layer.set_end_time_code(strong_end.max(weak_end));
    } else if weak_has_end {
        strong_layer.set_end_time_code(weak_layer.get_end_time_code());
    }
}

/// Stitches two prim specs together, merging weak into strong.
///
/// Follows C++ _MergeValueFn semantics:
/// - Fields only in weak: copy to strong
/// - Fields in both: merge based on field type (time samples, list ops, dicts)
/// - Specifier: 'over' in strong takes weak's specifier
/// - TimeSamples: non-overwriting merge (only add new time samples)
/// - StartTimeCode: take min
/// - EndTimeCode: take max
/// - FramesPerSecond/TimeCodesPerSecond: warn on mismatch, keep strong
fn stitch_prim_spec(
    strong_spec: &PrimSpec,
    weak_spec: &PrimSpec,
    stitch_value_fn: Option<&BoxedStitchValueFn>,
) {
    let strong_layer = strong_spec.layer();
    let weak_layer = weak_spec.layer();
    let strong_path = strong_spec.path();
    let weak_path = weak_spec.path();

    let strong_arc = match strong_layer.upgrade() {
        Some(l) => l,
        None => return,
    };
    let weak_arc = match weak_layer.upgrade() {
        Some(l) => l,
        None => return,
    };

    // Iterate weak fields and merge into strong
    let weak_fields = weak_arc.list_fields(&weak_path);
    for field in &weak_fields {
        let field_in_strong = strong_arc.has_field(&strong_path, field);
        let field_in_weak = true;

        // Check custom stitch function first
        if let Some(sfn) = stitch_value_fn {
            let (status, supplied_value) = sfn(
                field,
                &strong_path,
                &strong_arc,
                field_in_strong,
                &weak_arc,
                field_in_weak,
            );
            match status {
                StitchValueStatus::NoStitchedValue => continue,
                StitchValueStatus::UseSuppliedValue => {
                    if let Some(val) = supplied_value {
                        strong_arc.set_field(&strong_path, field, val);
                    }
                    continue;
                }
                StitchValueStatus::UseDefaultValue => {} // fall through
            }
        }

        if !field_in_strong {
            // Field only in weak - copy it over
            if let Some(val) = weak_arc.get_field(&weak_path, field) {
                strong_arc.set_field(&strong_path, field, val);
            }
            continue;
        }

        // Both have the field - merge based on field name/type
        let field_str = field.as_str();

        if field_str == "specifier" {
            // If strong specifier is 'over', take weak's value
            if strong_spec.specifier() == Specifier::Over {
                strong_spec.clone().set_specifier(weak_spec.specifier());
            }
        } else if field_str == "timeSamples" {
            // Non-overwriting merge: only add time samples from weak
            // that don't exist in strong
            let weak_times = weak_arc.list_time_samples_for_path(&weak_path);
            for time in weak_times {
                if strong_arc.query_time_sample(&strong_path, time).is_none() {
                    if let Some(val) = weak_arc.query_time_sample(&weak_path, time) {
                        strong_arc.set_time_sample(&strong_path, time, val);
                    }
                }
            }
        } else if field_str == "startTimeCode" {
            // Take minimum
            if let (Some(strong_val), Some(weak_val)) = (
                strong_arc
                    .get_field(&strong_path, field)
                    .and_then(|v| v.get::<f64>().copied()),
                weak_arc
                    .get_field(&weak_path, field)
                    .and_then(|v| v.get::<f64>().copied()),
            ) {
                strong_arc.set_field(&strong_path, field, Value::from(strong_val.min(weak_val)));
            }
        } else if field_str == "endTimeCode" {
            // Take maximum
            if let (Some(strong_val), Some(weak_val)) = (
                strong_arc
                    .get_field(&strong_path, field)
                    .and_then(|v| v.get::<f64>().copied()),
                weak_arc
                    .get_field(&weak_path, field)
                    .and_then(|v| v.get::<f64>().copied()),
            ) {
                strong_arc.set_field(&strong_path, field, Value::from(strong_val.max(weak_val)));
            }
        } else if field_str == "framesPerSecond"
            || field_str == "timeCodesPerSecond"
            || field_str == "framePrecision"
        {
            // Validate match, keep strong value
            if let (Some(strong_val), Some(weak_val)) = (
                strong_arc
                    .get_field(&strong_path, field)
                    .and_then(|v| v.get::<f64>().copied()),
                weak_arc
                    .get_field(&weak_path, field)
                    .and_then(|v| v.get::<f64>().copied()),
            ) {
                if (strong_val - weak_val).abs() > f64::EPSILON {
                    eprintln!("Warning: Mismatched {} in stitched layers", field_str);
                }
            }
        } else if field_str == "customData" || field_str == "assetInfo" {
            // Merge dictionaries recursively (strong wins on conflicts)
            merge_dict_field(&strong_arc, &strong_path, &weak_arc, &weak_path, field);
        } else if field_str == "variantSelection" {
            // Merge variant selections (strong wins on conflicts)
            let strong_sel = strong_spec.variant_selections();
            let weak_sel = weak_spec.variant_selections();
            let mut merged = weak_sel;
            merged.extend(strong_sel);
            for (vset, vname) in &merged {
                strong_spec.clone().set_variant_selection(vset, vname);
            }
        }
        // Other fields: keep strong value (no copy needed)
    }

    // Recursively stitch children that exist in both
    let weak_children = weak_spec.name_children();
    for weak_child in &weak_children {
        let child_name = weak_child.name();
        let child_path = strong_path.append_child(child_name.as_str());
        let child_path = match child_path {
            Some(p) => p,
            None => continue,
        };

        if let Some(strong_child) = strong_arc.get_prim_at_path(&child_path) {
            stitch_prim_spec(&strong_child, weak_child, stitch_value_fn);
        } else {
            copy_prim_spec(&strong_arc, &child_path, weak_child);
        }
    }

    // Stitch properties
    for weak_prop in weak_spec.properties() {
        let prop_name = weak_prop.name();
        let prop_path = strong_path.append_property(prop_name.as_str());
        let prop_path = match prop_path {
            Some(p) => p,
            None => continue,
        };

        if strong_arc.get_property_at_path(&prop_path).is_some() {
            // Property exists in both - merge fields
            let weak_prop_path = weak_prop.spec().path();
            let prop_fields = weak_arc.list_fields(&weak_prop_path);
            for field in &prop_fields {
                if !strong_arc.has_field(&prop_path, field) {
                    if let Some(val) = weak_arc.get_field(&weak_prop_path, field) {
                        strong_arc.set_field(&prop_path, field, val);
                    }
                } else if field == "timeSamples" {
                    // Non-overwriting time sample merge
                    let weak_times = weak_arc.list_time_samples_for_path(&weak_prop_path);
                    for time in weak_times {
                        if strong_arc.query_time_sample(&prop_path, time).is_none() {
                            if let Some(val) = weak_arc.query_time_sample(&weak_prop_path, time) {
                                strong_arc.set_time_sample(&prop_path, time, val);
                            }
                        }
                    }
                }
            }
        } else {
            // Property only in weak - copy all fields
            let weak_prop_path = weak_prop.spec().path();
            let prop_fields = weak_arc.list_fields(&weak_prop_path);
            // Create the spec first
            strong_arc.create_spec(&prop_path, weak_arc.get_spec_type(&weak_prop_path));
            for field in &prop_fields {
                if let Some(val) = weak_arc.get_field(&weak_prop_path, field) {
                    strong_arc.set_field(&prop_path, field, val);
                }
            }
        }
    }
}

/// Merges dictionary fields from weak into strong (strong wins on conflicts).
fn merge_dict_field(
    strong_layer: &Arc<Layer>,
    strong_path: &Path,
    weak_layer: &Arc<Layer>,
    weak_path: &Path,
    field: &Token,
) {
    let strong_dict = strong_layer
        .get_field(strong_path, field)
        .and_then(|v| v.as_dictionary())
        .unwrap_or_default();
    let weak_dict = weak_layer
        .get_field(weak_path, field)
        .and_then(|v| v.as_dictionary())
        .unwrap_or_default();

    // Merge: weak provides base, strong overwrites
    let mut merged = weak_dict;
    merged.extend(strong_dict);
    strong_layer.set_field(strong_path, field, Value::from_dictionary(merged));
}

/// Copies a prim spec (and all its children, properties, metadata) to dest layer.
fn copy_prim_spec(dest_layer: &Arc<Layer>, path: &Path, source_spec: &PrimSpec) {
    let source_layer = match source_spec.layer().upgrade() {
        Some(l) => l,
        None => return,
    };
    let source_path = source_spec.path();

    // Create the prim spec in destination
    let specifier = source_spec.specifier();
    let type_name = source_spec.type_name();
    let _ = dest_layer.create_prim_spec(path, specifier, type_name.as_str());

    // Copy all fields from source to dest
    let fields = source_layer.list_fields(&source_path);
    for field in &fields {
        if let Some(val) = source_layer.get_field(&source_path, field) {
            dest_layer.set_field(path, field, val);
        }
    }

    // Copy time samples
    let times = source_layer.list_time_samples_for_path(&source_path);
    for time in times {
        if let Some(val) = source_layer.query_time_sample(&source_path, time) {
            dest_layer.set_time_sample(path, time, val);
        }
    }

    // Copy properties
    for prop in source_spec.properties() {
        let prop_name = prop.name();
        let dest_prop_path = match path.append_property(prop_name.as_str()) {
            Some(p) => p,
            None => continue,
        };
        let source_prop_path = prop.spec().path();

        // Create spec and copy fields
        dest_layer.create_spec(
            &dest_prop_path,
            source_layer.get_spec_type(&source_prop_path),
        );
        let prop_fields = source_layer.list_fields(&source_prop_path);
        for field in &prop_fields {
            if let Some(val) = source_layer.get_field(&source_prop_path, field) {
                dest_layer.set_field(&dest_prop_path, field, val);
            }
        }

        // Copy property time samples
        let prop_times = source_layer.list_time_samples_for_path(&source_prop_path);
        for time in prop_times {
            if let Some(val) = source_layer.query_time_sample(&source_prop_path, time) {
                dest_layer.set_time_sample(&dest_prop_path, time, val);
            }
        }
    }

    // Recursively copy children
    for child in source_spec.name_children() {
        let child_name = child.name();
        if let Some(child_path) = path.append_child(child_name.as_str()) {
            copy_prim_spec(dest_layer, &child_path, &child);
        }
    }
}

/// Creates a default stitch function that uses default behavior for all fields.
pub fn default_stitch_value_fn() -> BoxedStitchValueFn {
    Box::new(|_field, _path, _strong, _in_strong, _weak, _in_weak| {
        (StitchValueStatus::UseDefaultValue, None)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stitch_value_status() {
        assert_ne!(
            StitchValueStatus::NoStitchedValue,
            StitchValueStatus::UseDefaultValue,
        );
        assert_ne!(
            StitchValueStatus::UseDefaultValue,
            StitchValueStatus::UseSuppliedValue,
        );
    }
}
