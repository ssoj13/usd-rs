//! Material test ports from the Draco C++ reference.
//!
//! What: Re-implements `material_test.cc` and `material_library_test.cc`.
//! Why: Confirms parity for material properties, texture maps, and libraries.
//! Where used: Runs under `draco-rs` tests; relies on `crates/draco-rs/test`.

use std::env;
use std::path::PathBuf;

use crate::core::vector_d::{Vector3f, Vector4f};
use crate::io::texture_io;
use crate::material::{Material, MaterialLibrary, MaterialTransparencyMode};
use crate::texture::{Texture, TextureMapType, TextureUtils};

const TEXTURE_MAP_TYPES: [TextureMapType; 19] = [
    TextureMapType::Generic,
    TextureMapType::Color,
    TextureMapType::Opacity,
    TextureMapType::Metallic,
    TextureMapType::Roughness,
    TextureMapType::MetallicRoughness,
    TextureMapType::NormalObjectSpace,
    TextureMapType::NormalTangentSpace,
    TextureMapType::AmbientOcclusion,
    TextureMapType::Emissive,
    TextureMapType::SheenColor,
    TextureMapType::SheenRoughness,
    TextureMapType::Transmission,
    TextureMapType::Clearcoat,
    TextureMapType::ClearcoatRoughness,
    TextureMapType::ClearcoatNormal,
    TextureMapType::Thickness,
    TextureMapType::Specular,
    TextureMapType::SpecularColor,
];

fn test_data_dir() -> PathBuf {
    if let Ok(dir) = env::var("DRACO_RS_TEST_DATA_DIR") {
        return PathBuf::from(dir);
    }
    if let Ok(dir) = env::var("DRACO_TEST_DATA_DIR") {
        return PathBuf::from(dir);
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("test")
}

fn read_texture_from_test_file(file_name: &str) -> Box<Texture> {
    let path = test_data_dir()
        .join(file_name)
        .to_string_lossy()
        .into_owned();
    let status_or = texture_io::read_texture_from_file(&path);
    assert!(
        status_or.is_ok(),
        "{}",
        status_or.status().error_msg_string()
    );
    status_or.into_value()
}

#[test]
fn material_access() {
    let mut material = Material::new();

    material.set_name("Superalloy");
    assert_eq!(material.name(), "Superalloy");
    material.set_color_factor(Vector4f::new4(1.0, 0.2, 0.1, 0.9));
    assert_eq!(material.color_factor(), Vector4f::new4(1.0, 0.2, 0.1, 0.9));
    material.set_metallic_factor(0.3);
    assert_eq!(material.metallic_factor(), 0.3);
    material.set_roughness_factor(0.2);
    assert_eq!(material.roughness_factor(), 0.2);
    material.set_emissive_factor(Vector3f::new3(0.2, 0.0, 0.1));
    assert_eq!(material.emissive_factor(), Vector3f::new3(0.2, 0.0, 0.1));

    material.set_unlit(true);
    assert!(material.unlit());
    material.set_has_sheen(true);
    assert!(material.has_sheen());
    material.set_sheen_color_factor(Vector3f::new3(0.4, 0.2, 0.8));
    assert_eq!(material.sheen_color_factor(), Vector3f::new3(0.4, 0.2, 0.8));
    material.set_sheen_roughness_factor(0.428);
    assert_eq!(material.sheen_roughness_factor(), 0.428);
    material.set_has_transmission(true);
    assert!(material.has_transmission());
    material.set_transmission_factor(0.5);
    assert_eq!(material.transmission_factor(), 0.5);
    material.set_has_clearcoat(true);
    assert!(material.has_clearcoat());
    material.set_clearcoat_factor(0.6);
    assert_eq!(material.clearcoat_factor(), 0.6);
    material.set_clearcoat_roughness_factor(0.7);
    assert_eq!(material.clearcoat_roughness_factor(), 0.7);
    material.set_has_volume(true);
    assert!(material.has_volume());
    material.set_thickness_factor(0.8);
    assert_eq!(material.thickness_factor(), 0.8);
    material.set_attenuation_distance(0.9);
    assert_eq!(material.attenuation_distance(), 0.9);
    material.set_attenuation_color(Vector3f::new3(0.2, 0.5, 0.8));
    assert_eq!(material.attenuation_color(), Vector3f::new3(0.2, 0.5, 0.8));
    material.set_has_ior(true);
    assert!(material.has_ior());
    material.set_ior(1.1);
    assert_eq!(material.ior(), 1.1);
    material.set_has_specular(true);
    assert!(material.has_specular());
    material.set_specular_factor(0.01);
    assert_eq!(material.specular_factor(), 0.01);
    material.set_specular_color_factor(Vector3f::new3(0.4, 1.0, 1.0));
    assert_eq!(
        material.specular_color_factor(),
        Vector3f::new3(0.4, 1.0, 1.0)
    );

    assert!(material
        .texture_map_by_type(TextureMapType::Color)
        .is_none());
    assert_eq!(material.num_texture_maps(), 0);

    let texture = read_texture_from_test_file("test.png");
    material.set_texture_map(texture, TextureMapType::Color, 0);

    assert!(material
        .texture_map_by_type(TextureMapType::Color)
        .is_some());
    assert_eq!(material.num_texture_maps(), 1);
    assert_eq!(
        material.texture_map_by_index(0).unwrap() as *const _,
        material.texture_map_by_type(TextureMapType::Color).unwrap() as *const _
    );

    let texture2 = read_texture_from_test_file("test.png");
    material.set_texture_map(texture2, TextureMapType::Emissive, 1);

    assert!(material
        .texture_map_by_type(TextureMapType::Emissive)
        .is_some());
    assert_eq!(
        material
            .texture_map_by_type(TextureMapType::Emissive)
            .unwrap()
            .tex_coord_index(),
        1
    );
    assert_eq!(material.num_texture_maps(), 2);

    let texture3 = read_texture_from_test_file("test.png");
    material.set_texture_map(texture3, TextureMapType::Emissive, 2);
    assert_eq!(material.num_texture_maps(), 2);
    assert_eq!(
        material
            .texture_map_by_type(TextureMapType::Emissive)
            .unwrap()
            .tex_coord_index(),
        2
    );

    let texture4 = read_texture_from_test_file("test.png");
    material.set_texture_map(texture4, TextureMapType::Roughness, 0);
    assert_eq!(material.num_texture_maps(), 3);
    assert!(material
        .texture_map_by_type(TextureMapType::Roughness)
        .is_some());

    material.set_transparency_mode(MaterialTransparencyMode::Blend);
    assert_eq!(
        material.transparency_mode(),
        MaterialTransparencyMode::Blend
    );
    material.set_alpha_cutoff(0.2);
    assert_eq!(material.alpha_cutoff(), 0.2);
    material.set_normal_texture_scale(0.75);
    assert_eq!(material.normal_texture_scale(), 0.75);

    material.clear_texture_maps();
    assert_eq!(material.num_texture_maps(), 0);
    assert!(material
        .texture_map_by_type(TextureMapType::Color)
        .is_none());

    assert_eq!(material.metallic_factor(), 0.3);

    material.clear();
    assert_ne!(material.metallic_factor(), 0.3);

    assert!(!material.double_sided());
    material.set_double_sided(true);
    assert!(material.double_sided());
}

#[test]
fn material_copy() {
    let mut material = Material::new();
    material.set_name("Antimatter");
    material.set_color_factor(Vector4f::new4(0.3, 0.2, 0.4, 0.9));
    material.set_metallic_factor(0.2);
    material.set_roughness_factor(0.4);
    material.set_emissive_factor(Vector3f::new3(0.3, 0.1, 0.2));
    material.set_transparency_mode(MaterialTransparencyMode::Mask);
    material.set_alpha_cutoff(0.25);
    material.set_double_sided(true);
    material.set_normal_texture_scale(0.75);

    material.set_unlit(true);
    material.set_has_sheen(true);
    material.set_sheen_color_factor(Vector3f::new3(0.4, 0.2, 0.8));
    material.set_sheen_roughness_factor(0.428);
    material.set_has_transmission(true);
    material.set_transmission_factor(0.5);
    material.set_has_clearcoat(true);
    material.set_clearcoat_factor(0.6);
    material.set_clearcoat_roughness_factor(0.7);
    material.set_has_volume(true);
    material.set_thickness_factor(0.8);
    material.set_attenuation_distance(0.9);
    material.set_attenuation_color(Vector3f::new3(0.2, 0.5, 0.8));
    material.set_has_ior(true);
    material.set_ior(1.1);
    material.set_has_specular(true);
    material.set_specular_factor(0.01);
    material.set_specular_color_factor(Vector3f::new3(0.4, 1.0, 1.0));

    let texture = read_texture_from_test_file("test.png");
    material.set_texture_map(texture, TextureMapType::Emissive, 2);

    let mut new_material = Material::new();
    new_material.copy_from(&material);

    assert_eq!(material.name(), new_material.name());
    assert_eq!(material.color_factor(), new_material.color_factor());
    assert_eq!(material.metallic_factor(), new_material.metallic_factor());
    assert_eq!(material.roughness_factor(), new_material.roughness_factor());
    assert_eq!(material.emissive_factor(), new_material.emissive_factor());
    assert_eq!(
        material.transparency_mode(),
        new_material.transparency_mode()
    );
    assert_eq!(material.alpha_cutoff(), new_material.alpha_cutoff());
    assert_eq!(material.double_sided(), new_material.double_sided());
    assert_eq!(
        material.normal_texture_scale(),
        new_material.normal_texture_scale()
    );

    assert_eq!(material.unlit(), new_material.unlit());
    assert_eq!(material.has_sheen(), new_material.has_sheen());
    assert_eq!(
        material.sheen_color_factor(),
        new_material.sheen_color_factor()
    );
    assert_eq!(
        material.sheen_roughness_factor(),
        new_material.sheen_roughness_factor()
    );
    assert_eq!(material.has_transmission(), new_material.has_transmission());
    assert_eq!(
        material.transmission_factor(),
        new_material.transmission_factor()
    );
    assert_eq!(material.has_clearcoat(), new_material.has_clearcoat());
    assert_eq!(material.clearcoat_factor(), new_material.clearcoat_factor());
    assert_eq!(
        material.clearcoat_roughness_factor(),
        new_material.clearcoat_roughness_factor()
    );
    assert_eq!(material.has_volume(), new_material.has_volume());
    assert_eq!(material.thickness_factor(), new_material.thickness_factor());
    assert_eq!(
        material.attenuation_distance(),
        new_material.attenuation_distance()
    );
    assert_eq!(
        material.attenuation_color(),
        new_material.attenuation_color()
    );
    assert_eq!(material.has_ior(), new_material.has_ior());
    assert_eq!(material.ior(), new_material.ior());
    assert_eq!(material.has_specular(), new_material.has_specular());
    assert_eq!(material.specular_factor(), new_material.specular_factor());
    assert_eq!(
        material.specular_color_factor(),
        new_material.specular_color_factor()
    );

    for texture_map_type in TEXTURE_MAP_TYPES {
        if material.texture_map_by_type(texture_map_type).is_none() {
            assert!(new_material.texture_map_by_type(texture_map_type).is_none());
            continue;
        }
        if material
            .texture_map_by_type(texture_map_type)
            .unwrap()
            .texture()
            .is_none()
        {
            assert!(new_material
                .texture_map_by_type(texture_map_type)
                .unwrap()
                .texture()
                .is_none());
        } else {
            assert!(new_material
                .texture_map_by_type(texture_map_type)
                .unwrap()
                .texture()
                .is_some());
            assert_eq!(
                material
                    .texture_map_by_type(texture_map_type)
                    .unwrap()
                    .tex_coord_index(),
                new_material
                    .texture_map_by_type(texture_map_type)
                    .unwrap()
                    .tex_coord_index()
            );
        }
    }
}

#[test]
fn material_library_materials() {
    let mut library = MaterialLibrary::new();
    assert_eq!(library.num_materials(), 0);

    {
        let new_mat = library.mutable_material(0);
        assert!(new_mat.is_some());
    }
    assert_eq!(library.num_materials(), 1);

    let new_mat2_ptr = {
        let new_mat2 = library.mutable_material(2);
        assert!(new_mat2.is_some());
        new_mat2.unwrap() as *mut Material
    };
    assert_eq!(library.num_materials(), 3);
    assert!(std::ptr::eq(library.material(2).unwrap(), unsafe {
        &*new_mat2_ptr
    }));

    for i in 0..library.num_materials() {
        assert!(library.mutable_material(i as i32).is_some());
    }
    assert_eq!(library.num_materials(), 3);

    library.add_materials_variant("Milk Truck");
    library.add_materials_variant("Ice Cream Truck");
    assert_eq!(library.num_materials_variants(), 2);
    assert_eq!(library.materials_variant_name(0), "Milk Truck");
    assert_eq!(library.materials_variant_name(1), "Ice Cream Truck");

    library.clear();
    assert_eq!(library.num_materials(), 0);
    assert_eq!(library.num_materials_variants(), 0);
}

#[test]
fn material_library_copy() {
    let mut library = MaterialLibrary::new();
    library
        .mutable_material(0)
        .unwrap()
        .set_metallic_factor(2.4);
    library
        .mutable_material(3)
        .unwrap()
        .set_roughness_factor(1.2);
    library.add_materials_variant("Milk Truck");
    library.add_materials_variant("Ice Cream Truck");

    let mut new_library = MaterialLibrary::new();
    new_library.copy_from(&library);
    assert_eq!(library.num_materials(), new_library.num_materials());
    assert_eq!(
        library.material(0).unwrap().metallic_factor(),
        new_library.material(0).unwrap().metallic_factor()
    );
    assert_eq!(
        library.material(3).unwrap().roughness_factor(),
        new_library.material(3).unwrap().roughness_factor()
    );
    assert_eq!(new_library.num_materials_variants(), 2);
    assert_eq!(new_library.materials_variant_name(0), "Milk Truck");
    assert_eq!(new_library.materials_variant_name(1), "Ice Cream Truck");
}

#[test]
fn material_library_texture_library_updates() {
    let texture_0 = Box::new(Texture::new());
    let texture_1 = Box::new(Texture::new());

    let mut library = MaterialLibrary::new();
    library
        .mutable_material(0)
        .unwrap()
        .set_texture_map(texture_0, TextureMapType::Color, 0);
    assert_eq!(library.texture_library().num_textures(), 1);
    library
        .mutable_material(3)
        .unwrap()
        .set_texture_map(texture_1, TextureMapType::Color, 0);
    assert_eq!(library.texture_library().num_textures(), 2);
}

#[test]
fn material_library_remove_unused_textures() {
    let texture_0 = Box::new(Texture::new());
    let texture_1 = Box::new(Texture::new());
    let texture_2 = Box::new(Texture::new());

    let mut library = MaterialLibrary::new();
    library
        .mutable_material(0)
        .unwrap()
        .set_texture_map(texture_0, TextureMapType::Color, 0);
    library.mutable_material(0).unwrap().set_texture_map(
        texture_1,
        TextureMapType::MetallicRoughness,
        0,
    );
    library
        .mutable_material(1)
        .unwrap()
        .set_texture_map(texture_2, TextureMapType::Color, 0);

    assert_eq!(library.texture_library().num_textures(), 3);

    library.remove_unused_textures();
    assert_eq!(library.texture_library().num_textures(), 3);

    library
        .mutable_material(0)
        .unwrap()
        .remove_texture_map_by_type(TextureMapType::MetallicRoughness);
    library.remove_unused_textures();
    assert_eq!(library.texture_library().num_textures(), 2);

    library
        .mutable_material(1)
        .unwrap()
        .remove_texture_map_by_type(TextureMapType::Color);
    library.remove_unused_textures();
    assert_eq!(library.texture_library().num_textures(), 1);

    library
        .mutable_material(0)
        .unwrap()
        .remove_texture_map_by_type(TextureMapType::Color);
    library.remove_unused_textures();
    assert_eq!(library.texture_library().num_textures(), 0);
}

#[test]
fn material_library_remove_material() {
    let mut library = MaterialLibrary::new();
    library
        .mutable_material(0)
        .unwrap()
        .set_metallic_factor(0.0);
    library
        .mutable_material(1)
        .unwrap()
        .set_metallic_factor(1.0);
    library
        .mutable_material(2)
        .unwrap()
        .set_metallic_factor(2.0);
    library
        .mutable_material(3)
        .unwrap()
        .set_metallic_factor(3.0);

    assert_eq!(library.num_materials(), 4);

    assert_eq!(library.remove_material(0).unwrap().metallic_factor(), 0.0);
    assert_eq!(library.num_materials(), 3);

    assert_eq!(library.remove_material(1).unwrap().metallic_factor(), 2.0);
    assert_eq!(library.num_materials(), 2);

    assert_eq!(library.remove_material(1).unwrap().metallic_factor(), 3.0);
    assert_eq!(library.num_materials(), 1);

    assert_eq!(library.remove_material(0).unwrap().metallic_factor(), 1.0);
    assert_eq!(library.num_materials(), 0);
}

#[test]
fn texture_utils_compute_required_num_channels() {
    let texture0 = read_texture_from_test_file("fully_transparent.png");
    let texture0_ptr: *const Texture = texture0.as_ref();
    let texture1 = read_texture_from_test_file("squares.png");
    let texture1_ptr: *const Texture = texture1.as_ref();

    let mut library = MaterialLibrary::new();
    {
        let material0 = library.mutable_material(0).unwrap();
        material0.set_texture_map(texture0, TextureMapType::AmbientOcclusion, 0);
    }
    assert_eq!(
        TextureUtils::compute_required_num_channels(unsafe { &*texture0_ptr }, &library),
        1
    );

    {
        let material1 = library.mutable_material(1).unwrap();
        material1.set_texture_map(texture1, TextureMapType::MetallicRoughness, 0);
    }
    assert_eq!(
        TextureUtils::compute_required_num_channels(unsafe { &*texture0_ptr }, &library),
        1
    );

    assert_eq!(
        TextureUtils::compute_required_num_channels(unsafe { &*texture1_ptr }, &library),
        3
    );

    {
        let material2 = library.mutable_material(2).unwrap();
        let status = material2.set_texture_map_existing(
            texture0_ptr as *mut Texture,
            TextureMapType::MetallicRoughness,
            0,
        );
        assert!(status.is_ok(), "{}", status.error_msg_string());
    }
    assert_eq!(
        TextureUtils::compute_required_num_channels(unsafe { &*texture0_ptr }, &library),
        3
    );
}
