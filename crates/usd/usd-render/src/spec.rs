//! Render Specification utilities.
//!
//! Provides self-contained specification structures for render settings
//! that can be computed from USD prims. These structures aggregate all
//! render settings data in a convenient form for render delegates.
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdRender/spec.h` and `spec.cpp`
//!
//! # Key Functions
//!
//! - `compute_spec()` - Computes complete render specification from settings prim
//! - `compute_namespaced_settings()` - Extracts custom namespaced attributes

use std::collections::HashMap;

use usd_gf::{Range2f, Vec2f, Vec2i, Vec4f};
use usd_sdf::{Path, TimeCode};
use usd_shade::output::Output as ShadeOutput;
use usd_shade::utils::Utils as ShadeUtils;
use usd_tf::Token;
use usd_vt::Value;

use super::product::RenderProduct;
use super::settings::RenderSettings;
use super::settings_base::RenderSettingsBase;
use super::tokens::USD_RENDER_TOKENS;
use super::var::RenderVar as RenderVarSchema;

// ============================================================================
// Render Specification Structures
// ============================================================================

/// A self-contained specification of render settings.
///
/// This aggregates all render configuration into a convenient structure
/// for consumption by render delegates.
#[derive(Debug, Clone, Default)]
pub struct RenderSpec {
    /// The full list of products requested by this render.
    pub products: Vec<Product>,
    /// The full list of render vars requested by products in this render.
    pub render_vars: Vec<RenderVarSpec>,
    /// List of purposes to use to filter scene contents.
    pub included_purposes: Vec<Token>,
    /// List of material binding purposes.
    pub material_binding_purposes: Vec<Token>,
    /// Any extra settings values discovered in requested namespaces.
    pub namespaced_settings: HashMap<String, Value>,
}

/// Specification of a render product.
///
/// Contains all settings needed to configure a render output.
#[derive(Debug, Clone)]
pub struct Product {
    /// The path of this product, which uniquely identifies it.
    pub render_product_path: Path,
    /// The type of product, ex: "raster".
    pub product_type: Token,
    /// The name of the product.
    pub name: Token,
    /// Path to the primary UsdGeomCamera to use for this product.
    pub camera_path: Path,
    /// If set to true, disables motion blur.
    pub disable_motion_blur: bool,
    /// If set to true, disables depth of field.
    pub disable_depth_of_field: bool,
    /// The pixel resolution of the product.
    pub resolution: Vec2i,
    /// The pixel aspect ratio as adjusted by aspectRatioConformPolicy.
    pub pixel_aspect_ratio: f32,
    /// The policy that was applied to conform aspect ratio.
    pub aspect_ratio_conform_policy: Token,
    /// The camera aperture size as adjusted by aspectRatioConformPolicy.
    pub aperture_size: Vec2f,
    /// The data window, in NDC terms relative to the aperture.
    /// (0,0) corresponds to bottom-left and (1,1) corresponds to top-right.
    pub data_window_ndc: Range2f,
    /// The render vars used by this product, as indices into render_vars.
    pub render_var_indices: Vec<usize>,
    /// Any extra settings values discovered in requested namespaces.
    pub namespaced_settings: HashMap<String, Value>,
}

impl Default for Product {
    fn default() -> Self {
        Self {
            render_product_path: Path::default(),
            product_type: Token::default(),
            name: Token::default(),
            camera_path: Path::default(),
            disable_motion_blur: false,
            disable_depth_of_field: false,
            resolution: Vec2i::new(2048, 1080),
            pixel_aspect_ratio: 1.0,
            aspect_ratio_conform_policy: Token::default(),
            aperture_size: Vec2f::new(0.0, 0.0),
            data_window_ndc: Range2f::new(Vec2f::new(0.0, 0.0), Vec2f::new(1.0, 1.0)),
            render_var_indices: Vec::new(),
            namespaced_settings: HashMap::new(),
        }
    }
}

/// Specification of a render variable (AOV).
///
/// Describes a data channel to be rendered.
#[derive(Debug, Clone, Default)]
pub struct RenderVarSpec {
    /// The path of this render var, which uniquely identifies it.
    pub render_var_path: Path,
    /// The value data type of the variable, as a USD type name.
    pub data_type: Token,
    /// Source name for the renderer to look for.
    pub source_name: String,
    /// Source type (raw, primvar, lpe, intrinsic).
    pub source_type: Token,
    /// Any extra settings values discovered in requested namespaces.
    pub namespaced_settings: HashMap<String, Value>,
}

// ============================================================================
// Internal Helper Functions
// ============================================================================

/// Return the outermost namespace of an attribute name.
///
/// For "ri:attributes:foo", returns "ri".
/// For "foo", returns empty string.
fn get_attr_namespace(name: &str) -> String {
    if let Some(pos) = name.find(':') {
        name[..pos].to_string()
    } else {
        String::new()
    }
}

/// Check if attribute name starts with "outputs:" prefix (UsdShade output).
fn is_shade_output(name: &str) -> bool {
    name.starts_with("outputs:")
}

/// Strip "outputs:" prefix if present, returning base name.
fn get_shade_output_basename(name: &str) -> &str {
    name.strip_prefix("outputs:").unwrap_or(name)
}

/// Read namespaced settings from a prim into a dictionary.
///
/// Iterates all authored attributes and relationships, filtering by namespace.
/// If namespaces is empty, collects all custom (namespaced) attributes.
fn read_namespaced_settings(
    prim: &usd_core::Prim,
    requested_namespaces: &[Token],
    namespaced_settings: &mut HashMap<String, Value>,
) {
    // Process authored attributes
    for attr_name in prim.get_attribute_names() {
        let name_str = attr_name.as_str();

        // Use shade output basename for namespace check
        let basename = get_shade_output_basename(name_str);
        let attr_namespace = get_attr_namespace(basename);

        // Only collect namespaced settings
        if attr_namespace.is_empty() {
            continue;
        }

        // If specific namespaces requested, require a match
        if !requested_namespaces.is_empty() {
            let ns_token = Token::new(&attr_namespace);
            if !requested_namespaces.contains(&ns_token) {
                continue;
            }
        }

        // Get attribute and read value
        if let Some(attr) = prim.get_attribute(name_str) {
            // Connections are stronger than values authored on the attribute,
            // so check for connections first (UsdShade outputs).
            if is_shade_output(name_str) {
                let shade_output = ShadeOutput::from_attribute(attr.clone());
                if shade_output.is_valid() {
                    // Use GetValueProducingAttributes to resolve connected sources
                    let targets =
                        ShadeUtils::get_value_producing_attributes_output(&shade_output, false);
                    if !targets.is_empty() {
                        // C++ stores connected prim paths
                        let output_paths: Vec<String> = targets
                            .iter()
                            .map(|a| a.prim_path().get_string().to_string())
                            .collect();
                        namespaced_settings.insert(name_str.to_string(), Value::from(output_paths));
                        continue;
                    }
                }
            }

            // Base case: use the attribute value
            if let Some(val) = attr.get(TimeCode::default()) {
                namespaced_settings.insert(name_str.to_string(), val);
            }
        }
    }

    // Process authored relationships
    for rel_name in prim.get_relationship_names() {
        let name_str = rel_name.as_str();
        let rel_namespace = get_attr_namespace(name_str);

        // Only collect namespaced relationships
        if rel_namespace.is_empty() {
            continue;
        }

        // If specific namespaces requested, require a match
        if !requested_namespaces.is_empty() {
            let ns_token = Token::new(&rel_namespace);
            if !requested_namespaces.contains(&ns_token) {
                continue;
            }
        }

        if let Some(rel) = prim.get_relationship(name_str) {
            let targets = rel.get_targets();
            // Store paths as strings since Value doesn't directly support Vec<Path>
            let target_strs: Vec<String> =
                targets.iter().map(|p| p.get_string().to_string()).collect();
            namespaced_settings.insert(name_str.to_string(), Value::from(target_strs));
        }
    }
}

/// Helper to get attribute value, optionally requiring authored value.
fn get_attr_value<T: Clone + 'static>(
    attr: &usd_core::Attribute,
    get_default_value: bool,
) -> Option<T> {
    if get_default_value || attr.has_authored_value() {
        attr.get_typed::<T>(TimeCode::default())
    } else {
        None
    }
}

/// Read settings from RenderSettingsBase into a Product.
///
/// Reads camera, resolution, pixelAspectRatio, aspectRatioConformPolicy,
/// dataWindowNDC, disableMotionBlur, and disableDepthOfField.
fn read_settings_base(rs_base: &RenderSettingsBase, product: &mut Product, get_default: bool) {
    // Camera relationship
    if let Some(camera_rel) = rs_base.get_camera_rel() {
        let targets = camera_rel.get_forwarded_targets();
        if let Some(first_target) = targets.first() {
            product.camera_path = first_target.clone();
        }
    }

    // Resolution
    if let Some(attr) = rs_base.get_resolution_attr() {
        if let Some(res) = get_attr_value::<Vec2i>(&attr, get_default) {
            product.resolution = res;
        }
    }

    // Pixel aspect ratio
    if let Some(attr) = rs_base.get_pixel_aspect_ratio_attr() {
        if let Some(par) = get_attr_value::<f32>(&attr, get_default) {
            product.pixel_aspect_ratio = par;
        }
    }

    // Aspect ratio conform policy
    if let Some(attr) = rs_base.get_aspect_ratio_conform_policy_attr() {
        if let Some(policy) = get_attr_value::<Token>(&attr, get_default) {
            product.aspect_ratio_conform_policy = policy;
        }
    }

    // Data window NDC (stored as vec4, convert to Range2f)
    if let Some(attr) = rs_base.get_data_window_ndc_attr() {
        if let Some(vec4) = get_attr_value::<Vec4f>(&attr, get_default) {
            product.data_window_ndc =
                Range2f::new(Vec2f::new(vec4.x, vec4.y), Vec2f::new(vec4.z, vec4.w));
        }
    }

    // Disable motion blur
    if let Some(attr) = rs_base.get_disable_motion_blur_attr() {
        if let Some(disable) = get_attr_value::<bool>(&attr, get_default) {
            product.disable_motion_blur = disable;
        }
    }

    // For backwards compatibility: instantaneousShutter disables motion blur
    if let Some(attr) = rs_base.get_instantaneous_shutter_attr() {
        if let Some(instantaneous) = get_attr_value::<bool>(&attr, get_default) {
            if instantaneous {
                product.disable_motion_blur = true;
            }
        }
    }

    // Disable depth of field
    if let Some(attr) = rs_base.get_disable_depth_of_field_attr() {
        if let Some(disable) = get_attr_value::<bool>(&attr, get_default) {
            product.disable_depth_of_field = disable;
        }
    }
}

/// Apply aspect ratio conform policy to adjust aperture size.
///
/// Adjusts the product's aperture_size based on the aspect_ratio_conform_policy
/// to match the image aspect ratio.
fn apply_aspect_ratio_policy(product: &mut Product) {
    let res = product.resolution;
    let size = product.aperture_size;

    // Validate dimensions
    if res.x <= 0 || res.y <= 0 || size.x <= 0.0 || size.y <= 0.0 {
        return;
    }

    // Compute aspect ratios
    let res_aspect_ratio = res.x as f32 / res.y as f32;
    let image_aspect_ratio = product.pixel_aspect_ratio * res_aspect_ratio;
    if image_aspect_ratio <= 0.0 {
        return;
    }
    let aperture_aspect_ratio = size.x / size.y;

    // Determine adjustment based on policy
    #[derive(PartialEq)]
    enum Adjust {
        Width,
        Height,
        None,
    }

    let policy = &product.aspect_ratio_conform_policy;
    let adjust = if policy == &USD_RENDER_TOKENS.adjust_pixel_aspect_ratio {
        product.pixel_aspect_ratio = aperture_aspect_ratio / res_aspect_ratio;
        Adjust::None
    } else if policy == &USD_RENDER_TOKENS.adjust_aperture_height {
        Adjust::Height
    } else if policy == &USD_RENDER_TOKENS.adjust_aperture_width {
        Adjust::Width
    } else if policy == &USD_RENDER_TOKENS.expand_aperture {
        if aperture_aspect_ratio > image_aspect_ratio {
            Adjust::Height
        } else {
            Adjust::Width
        }
    } else if policy == &USD_RENDER_TOKENS.crop_aperture {
        if aperture_aspect_ratio > image_aspect_ratio {
            Adjust::Width
        } else {
            Adjust::Height
        }
    } else {
        Adjust::None
    };

    // Apply adjustment so that size.x / size.y == image_aspect_ratio
    match adjust {
        Adjust::Width => {
            product.aperture_size = Vec2f::new(size.y * image_aspect_ratio, size.y);
        }
        Adjust::Height => {
            product.aperture_size = Vec2f::new(size.x, size.x / image_aspect_ratio);
        }
        Adjust::None => {}
    }
}

// ============================================================================
// Public API Functions
// ============================================================================

/// Computes the specification of the render settings.
///
/// For each product, applies the aspectRatioConformPolicy
/// and computes a final screenWindow and pixelAspectRatio.
///
/// Any other attributes encountered are returned in namespaced_settings.
/// If a non-empty list of namespaces is provided, only attributes
/// within those namespaces are returned.
///
/// # Arguments
///
/// * `settings` - The RenderSettings prim to compute from
/// * `namespaces` - List of namespaces to filter settings (empty = all custom attrs)
///
/// # Returns
///
/// A fully populated RenderSpec structure.
pub fn compute_spec(settings: &RenderSettings, namespaces: &[Token]) -> RenderSpec {
    let mut spec = RenderSpec::default();

    if !settings.is_valid() {
        return spec;
    }

    let rs_prim = settings.get_prim();
    let Some(stage) = rs_prim.stage() else {
        return spec;
    };

    // Read shared base settings as a "base product"
    // This excludes namespaced attributes that are gathered separately
    let mut base_product = Product::default();
    read_settings_base(&settings.as_settings_base(), &mut base_product, true);

    // Process products relationship
    if let Some(products_rel) = settings.get_products_rel() {
        let targets = products_rel.get_forwarded_targets();

        for target in targets {
            // Get the RenderProduct prim
            let Some(prim) = stage.get_prim_at_path(&target) else {
                continue;
            };

            // Check if it's a valid RenderProduct
            if !prim.is_a(&Token::new(RenderProduct::SCHEMA_TYPE_NAME)) {
                continue;
            }

            let rp_prim = RenderProduct::new(prim.clone());

            // Initialize render spec product with base settings
            let mut rp_spec = base_product.clone();
            rp_spec.render_product_path = target.clone();

            // Read product-specific overrides (only authored values)
            read_settings_base(&rp_prim.as_settings_base(), &mut rp_spec, false);

            // Read camera aperture and apply aspectRatioConformPolicy
            // Use camera path from rpSpec if authored, otherwise from base
            let cam_path = if rp_spec.camera_path.is_empty() {
                &base_product.camera_path
            } else {
                &rp_spec.camera_path
            };

            if !cam_path.is_empty() {
                // Try to get camera prim and read aperture
                if let Some(cam_prim) = stage.get_prim_at_path(cam_path) {
                    // Read horizontal and vertical aperture
                    if let Some(h_attr) = cam_prim.get_attribute("horizontalAperture") {
                        if let Some(h_aperture) = h_attr.get_typed::<f32>(TimeCode::default()) {
                            rp_spec.aperture_size = Vec2f::new(h_aperture, rp_spec.aperture_size.y);
                        }
                    }
                    if let Some(v_attr) = cam_prim.get_attribute("verticalAperture") {
                        if let Some(v_aperture) = v_attr.get_typed::<f32>(TimeCode::default()) {
                            rp_spec.aperture_size = Vec2f::new(rp_spec.aperture_size.x, v_aperture);
                        }
                    }

                    // Apply aspect ratio policy
                    apply_aspect_ratio_policy(&mut rp_spec);
                }
            }

            // Read product-only settings
            if let Some(attr) = rp_prim.get_product_type_attr() {
                if let Some(ptype) = attr.get_typed::<Token>(TimeCode::default()) {
                    rp_spec.product_type = ptype;
                }
            }
            if let Some(attr) = rp_prim.get_product_name_attr() {
                if let Some(pname) = attr.get_typed::<Token>(TimeCode::default()) {
                    rp_spec.name = pname;
                }
            }

            // Read render vars
            if let Some(vars_rel) = rp_prim.get_ordered_vars_rel() {
                let render_var_paths = vars_rel.get_forwarded_targets();

                for render_var_path in render_var_paths {
                    // Check if this render var already exists in our list
                    let existing_index = spec
                        .render_vars
                        .iter()
                        .position(|rv| rv.render_var_path == render_var_path);

                    if let Some(idx) = existing_index {
                        // Reuse existing render var
                        rp_spec.render_var_indices.push(idx);
                    } else {
                        // Check if it's a valid RenderVar prim
                        if let Some(rv_prim) = stage.get_prim_at_path(&render_var_path) {
                            if rv_prim.is_a(&Token::new(RenderVarSchema::SCHEMA_TYPE_NAME)) {
                                let rv_schema = RenderVarSchema::new(rv_prim.clone());
                                let mut rv_spec = RenderVarSpec::default();

                                // Store schema-defined attributes
                                rv_spec.render_var_path = render_var_path.clone();

                                if let Some(attr) = rv_schema.get_data_type_attr() {
                                    if let Some(dt) = attr.get_typed::<Token>(TimeCode::default()) {
                                        rv_spec.data_type = dt;
                                    }
                                }
                                if let Some(attr) = rv_schema.get_source_name_attr() {
                                    if let Some(sn) = attr.get_typed::<String>(TimeCode::default())
                                    {
                                        rv_spec.source_name = sn;
                                    }
                                }
                                if let Some(attr) = rv_schema.get_source_type_attr() {
                                    if let Some(st) = attr.get_typed::<Token>(TimeCode::default()) {
                                        rv_spec.source_type = st;
                                    }
                                }

                                // Read namespaced settings for render var
                                read_namespaced_settings(
                                    &rv_prim,
                                    namespaces,
                                    &mut rv_spec.namespaced_settings,
                                );

                                // Record new render var
                                rp_spec.render_var_indices.push(spec.render_vars.len());
                                spec.render_vars.push(rv_spec);
                            }
                        }
                    }
                }
            }

            // Read namespaced settings for product
            read_namespaced_settings(&prim, namespaces, &mut rp_spec.namespaced_settings);

            spec.products.push(rp_spec);
        }
    }

    // Scene configuration from RenderSettings
    if let Some(attr) = settings.get_material_binding_purposes_attr() {
        if let Some(purposes) = attr.get_typed::<Vec<Token>>(TimeCode::default()) {
            spec.material_binding_purposes = purposes;
        }
    }
    if let Some(attr) = settings.get_included_purposes_attr() {
        if let Some(purposes) = attr.get_typed::<Vec<Token>>(TimeCode::default()) {
            spec.included_purposes = purposes;
        }
    }

    // Read namespaced settings for render settings prim
    read_namespaced_settings(rs_prim, namespaces, &mut spec.namespaced_settings);

    spec
}

/// Returns a dictionary populated with attributes filtered by namespaces.
///
/// If a non-empty list of namespaces is provided, only authored attributes
/// within those namespaces are returned.
/// If an empty list of namespaces is provided, all custom (non-schema)
/// attributes are returned.
///
/// # Arguments
///
/// * `prim` - The prim to read namespaced settings from
/// * `namespaces` - List of namespaces to filter (empty = all custom attrs)
///
/// # Returns
///
/// A map of attribute/relationship names to their values.
pub fn compute_namespaced_settings(
    prim: &usd_core::Prim,
    namespaces: &[Token],
) -> HashMap<String, Value> {
    let mut result = HashMap::new();
    read_namespaced_settings(prim, namespaces, &mut result);
    result
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_product_default() {
        let product = Product::default();
        assert_eq!(product.resolution, Vec2i::new(2048, 1080));
        assert_eq!(product.pixel_aspect_ratio, 1.0);
        assert!(!product.disable_motion_blur);
        assert!(!product.disable_depth_of_field);
    }

    #[test]
    fn test_render_var_spec_default() {
        let rv = RenderVarSpec::default();
        assert!(rv.source_name.is_empty());
        assert!(rv.namespaced_settings.is_empty());
    }

    #[test]
    fn test_render_spec_default() {
        let spec = RenderSpec::default();
        assert!(spec.products.is_empty());
        assert!(spec.render_vars.is_empty());
        assert!(spec.included_purposes.is_empty());
    }

    #[test]
    fn test_get_attr_namespace() {
        assert_eq!(get_attr_namespace("ri:attributes:foo"), "ri");
        assert_eq!(get_attr_namespace("arnold:global:foo"), "arnold");
        assert_eq!(get_attr_namespace("nonamespace"), "");
        assert_eq!(get_attr_namespace(""), "");
    }

    #[test]
    fn test_is_shade_output() {
        assert!(is_shade_output("outputs:surface"));
        assert!(is_shade_output("outputs:displacement"));
        assert!(!is_shade_output("inputs:diffuseColor"));
        assert!(!is_shade_output("someAttr"));
    }

    #[test]
    fn test_get_shade_output_basename() {
        assert_eq!(get_shade_output_basename("outputs:surface"), "surface");
        assert_eq!(get_shade_output_basename("someAttr"), "someAttr");
    }

    #[test]
    fn test_apply_aspect_ratio_policy_expand() {
        let mut product = Product {
            resolution: Vec2i::new(1920, 1080),
            pixel_aspect_ratio: 1.0,
            aperture_size: Vec2f::new(36.0, 24.0), // 1.5 aspect ratio
            aspect_ratio_conform_policy: USD_RENDER_TOKENS.expand_aperture.clone(),
            ..Default::default()
        };

        // Image aspect ratio = 1.0 * (1920/1080) = 1.777...
        // Aperture aspect ratio = 36/24 = 1.5
        // Since aperture_ar < image_ar and policy is expand, adjust width
        apply_aspect_ratio_policy(&mut product);

        // New width should be height * image_aspect_ratio = 24 * 1.777... = 42.666...
        assert!((product.aperture_size.x - 42.666666).abs() < 0.001);
        assert_eq!(product.aperture_size.y, 24.0);
    }

    #[test]
    fn test_apply_aspect_ratio_policy_crop() {
        let mut product = Product {
            resolution: Vec2i::new(1920, 1080),
            pixel_aspect_ratio: 1.0,
            aperture_size: Vec2f::new(36.0, 18.0), // 2.0 aspect ratio
            aspect_ratio_conform_policy: USD_RENDER_TOKENS.crop_aperture.clone(),
            ..Default::default()
        };

        // Image aspect ratio = 1.0 * (1920/1080) = 1.777...
        // Aperture aspect ratio = 36/18 = 2.0
        // Since aperture_ar > image_ar and policy is crop, adjust width
        apply_aspect_ratio_policy(&mut product);

        // New width should be height * image_aspect_ratio = 18 * 1.777... = 32.0
        assert!((product.aperture_size.x - 32.0).abs() < 0.001);
        assert_eq!(product.aperture_size.y, 18.0);
    }
}
