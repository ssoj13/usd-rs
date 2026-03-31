//! Material utilities.
//!
//! What: Equivalence helpers for Draco materials.
//! Why: Mesh cleanup and IO need a consistent way to verify default/unused materials.
//! How: Compares scalar properties and texture maps by value (including textures).
//! Where used: Mesh cleanup/tests and glTF material workflows.

use crate::material::material::Material;
use crate::texture::texture_map::TextureMap;

/// Material helper utilities mirroring Draco's `MaterialUtils`.
pub struct MaterialUtils;

impl MaterialUtils {
    /// Returns true when two materials are equivalent by value.
    pub fn are_materials_equivalent(lhs: &Material, rhs: &Material) -> bool {
        if lhs.name() != rhs.name()
            || lhs.color_factor() != rhs.color_factor()
            || lhs.metallic_factor() != rhs.metallic_factor()
            || lhs.roughness_factor() != rhs.roughness_factor()
            || lhs.emissive_factor() != rhs.emissive_factor()
            || lhs.double_sided() != rhs.double_sided()
            || lhs.transparency_mode() != rhs.transparency_mode()
            || lhs.alpha_cutoff() != rhs.alpha_cutoff()
            || lhs.normal_texture_scale() != rhs.normal_texture_scale()
            || lhs.unlit() != rhs.unlit()
            || lhs.has_sheen() != rhs.has_sheen()
            || lhs.sheen_color_factor() != rhs.sheen_color_factor()
            || lhs.sheen_roughness_factor() != rhs.sheen_roughness_factor()
            || lhs.has_transmission() != rhs.has_transmission()
            || lhs.transmission_factor() != rhs.transmission_factor()
            || lhs.has_clearcoat() != rhs.has_clearcoat()
            || lhs.clearcoat_factor() != rhs.clearcoat_factor()
            || lhs.clearcoat_roughness_factor() != rhs.clearcoat_roughness_factor()
            || lhs.has_volume() != rhs.has_volume()
            || lhs.thickness_factor() != rhs.thickness_factor()
            || lhs.attenuation_distance() != rhs.attenuation_distance()
            || lhs.attenuation_color() != rhs.attenuation_color()
            || lhs.has_ior() != rhs.has_ior()
            || lhs.ior() != rhs.ior()
            || lhs.has_specular() != rhs.has_specular()
            || lhs.specular_factor() != rhs.specular_factor()
            || lhs.specular_color_factor() != rhs.specular_color_factor()
        {
            return false;
        }

        if lhs.num_texture_maps() != rhs.num_texture_maps() {
            return false;
        }

        for i in 0..lhs.num_texture_maps() {
            let lhs_map = match lhs.texture_map_by_index(i as i32) {
                Some(map) => map,
                None => return false,
            };
            let rhs_map = match rhs.texture_map_by_type(lhs_map.map_type()) {
                Some(map) => map,
                None => return false,
            };
            if !texture_map_equivalent(lhs_map, rhs_map) {
                return false;
            }
        }

        true
    }
}

fn texture_map_equivalent(lhs: &TextureMap, rhs: &TextureMap) -> bool {
    if lhs.map_type() != rhs.map_type()
        || lhs.wrapping_mode() != rhs.wrapping_mode()
        || lhs.tex_coord_index() != rhs.tex_coord_index()
        || lhs.min_filter() != rhs.min_filter()
        || lhs.mag_filter() != rhs.mag_filter()
        || lhs.texture_transform() != rhs.texture_transform()
    {
        return false;
    }

    match (lhs.texture(), rhs.texture()) {
        (None, None) => true,
        (Some(tex_a), Some(tex_b)) => tex_a == tex_b,
        _ => false,
    }
}
