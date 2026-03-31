//! glTF test helper utilities.
//!
//! What: Builds and validates feature/metadata-heavy test scenes and meshes.
//! Why: Mirrors Draco `GltfTestHelper` used by glTF encoder/decoder tests.
//! How: Populates mesh features, structural metadata schema/tables, and
//!      property attributes with fixed reference data for deterministic checks.
//! Where used: glTF IO tests and mesh/scene parity validation.

use draco_core::attributes::geometry_attribute::GeometryAttributeType;
use draco_core::attributes::geometry_indices::{AttributeValueIndex, CornerIndex, FaceIndex};
use draco_core::attributes::point_attribute::PointAttribute;
use draco_core::core::draco_index_type_vector::IndexTypeVector;
use draco_core::core::draco_types::DataType;
use draco_core::mesh::mesh_features::MeshFeatures;
use draco_core::mesh::mesh_indices::MeshFeaturesIndex;
use draco_core::mesh::Mesh;
use draco_core::metadata::property_attribute::{
    Property as PropertyAttributeProperty, PropertyAttribute,
};
use draco_core::metadata::property_table::{Property as PropertyTableProperty, PropertyTable};
use draco_core::metadata::structural_metadata::StructuralMetadata;
use draco_core::metadata::structural_metadata_schema::{Object, StructuralMetadataSchema};
use draco_core::texture::texture_library::TextureLibrary;

use crate::scene::{MeshIndex, Scene};

/// Helper class for testing Draco glTF encoder and decoder.
pub struct GltfTestHelper;

/// Use-case flags used by glTF tests.
#[derive(Clone, Copy, Debug, Default)]
pub struct UseCase {
    pub has_draco_compression: bool,
    pub has_mesh_features: bool,
    pub has_structural_metadata: bool,
}

impl GltfTestHelper {
    /// Adds mesh feature ID sets to the box scene loaded from Box.gltf.
    pub fn add_box_meta_mesh_features(scene: &mut Scene) {
        assert_eq!(scene.num_meshes(), 1);
        let texture_library = scene.non_material_texture_library_mut();
        assert_eq!(texture_library.num_textures(), 0);

        let mesh = scene.mesh_mut(MeshIndex::from(0));
        assert_eq!(mesh.num_faces(), 12);
        assert_eq!(mesh.num_attributes(), 2);
        assert_eq!(mesh.num_points(), 24);

        let num_faces = mesh.num_faces() as usize;
        let num_corners = num_faces * 3;
        let num_vertices = mesh
            .get_named_attribute(GeometryAttributeType::Position)
            .expect("Missing POSITION attribute")
            .size();

        // Add feature ID set with per-face Uint8 attribute named _FEATURE_ID_0.
        {
            let mut pa = PointAttribute::new();
            pa.init(
                GeometryAttributeType::Generic,
                1,
                DataType::Uint8,
                false,
                num_faces,
            );
            for avi in 0..num_faces {
                let val = avi as u8;
                pa.set_attribute_value(AttributeValueIndex::from(avi as u32), &val);
            }
            let att_id = mesh.add_per_face_attribute(pa);

            let mut features = MeshFeatures::new();
            features.set_label("faces");
            features.set_feature_count(num_faces as i32);
            features.set_null_feature_id(100);
            features.set_property_table_index(0);
            features.set_attribute_index(att_id);
            mesh.add_mesh_features(Box::new(features));
        }

        // Add feature ID set with per-vertex Uint16 attribute named _FEATURE_ID_1.
        {
            let mut pa = PointAttribute::new();
            pa.init(
                GeometryAttributeType::Generic,
                1,
                DataType::Uint16,
                false,
                num_vertices,
            );
            for avi in 0..num_vertices {
                let val = avi as u16;
                pa.set_attribute_value(AttributeValueIndex::from(avi as u32), &val);
            }
            let att_id = mesh.add_per_vertex_attribute(pa);

            let mut features = MeshFeatures::new();
            features.set_label("vertices");
            features.set_feature_count(num_vertices as i32);
            features.set_null_feature_id(101);
            features.set_property_table_index(1);
            features.set_attribute_index(att_id);
            mesh.add_mesh_features(Box::new(features));
        }

        // Add feature ID set with per-corner Float attribute named _FEATURE_ID_2.
        {
            let mut pa = PointAttribute::new();
            pa.init(
                GeometryAttributeType::Generic,
                1,
                DataType::Float32,
                false,
                num_corners,
            );
            let mut corner_to_value =
                IndexTypeVector::<CornerIndex, AttributeValueIndex>::with_size_value(
                    num_corners,
                    AttributeValueIndex::from(0u32),
                );
            for avi in 0..num_corners {
                let val = avi as f32;
                pa.set_attribute_value(AttributeValueIndex::from(avi as u32), &val);
                corner_to_value[CornerIndex::from(avi as u32)] =
                    AttributeValueIndex::from(avi as u32);
            }
            let att_id = mesh.add_attribute_with_connectivity(pa, &corner_to_value);

            let mut features = MeshFeatures::new();
            features.set_feature_count(num_corners as i32);
            features.set_attribute_index(att_id);
            mesh.add_mesh_features(Box::new(features));
        }

        // Add feature ID set with the IDs stored in the R texture channel and
        // accessible via the first texture coordinate attribute.
        {
            let mut pa = PointAttribute::new();
            pa.init(
                GeometryAttributeType::TexCoord,
                2,
                DataType::Float32,
                false,
                num_vertices,
            );
            let uv: [[f32; 2]; 8] = [
                [0.0, 0.0],
                [0.0, 0.5],
                [0.0, 1.0],
                [0.5, 0.0],
                [0.5, 0.5],
                [0.5, 1.0],
                [1.0, 0.0],
                [1.0, 0.5],
            ];
            for avi in 0..num_vertices {
                let index = avi as usize;
                pa.set_attribute_value_array(AttributeValueIndex::from(avi as u32), &uv[index]);
            }
            mesh.add_per_vertex_attribute(pa);
        }

        // Add feature ID set with the IDs stored in the GBA texture channels and
        // accessible via the second texture coordinate attribute.
        {
            let mut pa = PointAttribute::new();
            pa.init(
                GeometryAttributeType::TexCoord,
                2,
                DataType::Float32,
                false,
                num_vertices,
            );
            let uv: [[f32; 2]; 8] = [
                [0.0, 0.0],
                [0.0, 0.5],
                [0.0, 1.0],
                [0.5, 0.0],
                [0.5, 0.5],
                [0.5, 1.0],
                [1.0, 0.0],
                [1.0, 0.5],
            ];
            for avi in 0..num_vertices {
                let index = avi as usize;
                pa.set_attribute_value_array(AttributeValueIndex::from(avi as u32), &uv[index]);
            }
            mesh.add_per_vertex_attribute(pa);
            assert_eq!(
                mesh.num_named_attributes(GeometryAttributeType::TexCoord),
                2
            );
        }
    }

    /// Adds structural metadata schema, property tables, and property attributes
    /// to the box scene loaded from BoxMeta.gltf.
    pub fn add_box_meta_structural_metadata(scene: &mut Scene) {
        let schema = Self::build_structural_metadata_schema();
        scene.structural_metadata_mut().set_schema(schema);

        let mut table = PropertyTable::new();
        table.set_name("Galaxy far far away.");
        table.set_class("planet");
        table.set_count(16);

        // Property: color.
        {
            let mut property = PropertyTableProperty::new();
            property.set_name("color");
            let data = property.data_mut();
            data.target = 34962;
            data.data = vec![
                94, 94, 194, 94, 145, 161, 118, 171, 91, 103, 139, 178, 83, 98, 154, 91, 177, 175,
                190, 92, 108, 72, 69, 169, 154, 90, 101, 174, 85, 175, 184, 129, 96, 185, 91, 180,
                194, 150, 83, 204, 111, 134, 182, 90, 89, 0, 0, 0,
            ];
            table.add_property(property);
        }

        // Property: name.
        {
            let mut property = PropertyTableProperty::new();
            property.set_name("name");
            let data = property.data_mut();
            data.target = 34963;
            let labels = [
                "named_class:Tatooine",
                "named_class:Corusant",
                "named_class:Naboo",
                "named_class:Alderaan",
                "named_class:Dagobah",
                "named_class:Mandalore",
                "named_class:Corellia",
                "named_class:Kamino",
                "named_class:Kashyyyk",
                "named_class:Dantooine",
                "named_class:Hoth",
                "named_class:Mustafar",
                "named_class:Bespin",
                "named_class:Yavin",
                "named_class:Geonosis",
                "UNLABELED",
            ];
            let data_string = labels.concat();
            data.data = data_string.as_bytes().to_vec();

            let string_offsets = property.string_offsets_mut();
            string_offsets.type_name = "UINT32".to_string();
            string_offsets.data.target = 34963;
            string_offsets.data.data = vec![
                0, 0, 0, 0, 20, 0, 0, 0, 40, 0, 0, 0, 57, 0, 0, 0, 77, 0, 0, 0, 96, 0, 0, 0, 117,
                0, 0, 0, 137, 0, 0, 0, 155, 0, 0, 0, 175, 0, 0, 0, 196, 0, 0, 0, 212, 0, 0, 0, 232,
                0, 0, 0, 250, 0, 0, 0, 11, 1, 0, 0, 31, 1, 0, 0, 40, 1, 0, 0,
            ];
            table.add_property(property);
        }

        // Property: sequence.
        {
            let mut property = PropertyTableProperty::new();
            property.set_name("sequence");
            let data = property.data_mut();
            data.target = 34963;
            let floats = [
                0.5f32, 1.5, 2.5, 3.5, 4.5, 5.5, 6.5, 7.5, 8.5, 9.5, 10.5, 11.5, 12.5, 13.5, 14.5,
                15.5, 16.5, 17.5, 18.5, 19.5, 20.5, 21.5, 22.5, 23.5, 24.5, 25.5, 26.5, 27.5, 28.5,
                29.5, 30.5, 31.5, 32.5, 33.5, 34.5, 35.5, 36.5, 37.5, 38.5, 39.5, 40.5,
            ];
            data.data = floats
                .iter()
                .flat_map(|value| value.to_le_bytes().to_vec())
                .collect();

            let array_offsets = property.array_offsets_mut();
            array_offsets.type_name = "UINT8".to_string();
            array_offsets.data.target = 34963;
            array_offsets.data.data = vec![
                0 * 4,
                6 * 4,
                8 * 4,
                9 * 4,
                10 * 4,
                12 * 4,
                16 * 4,
                18 * 4,
                20 * 4,
                23 * 4,
                26 * 4,
                28 * 4,
                30 * 4,
                33 * 4,
                36 * 4,
                41 * 4,
                41 * 4,
            ];
            table.add_property(property);
        }

        scene.structural_metadata_mut().add_property_table(table);

        // Property attribute.
        let mut attribute = PropertyAttribute::new();
        attribute.set_name("The movement.");
        attribute.set_class("movement");
        {
            let mut property = PropertyAttributeProperty::new();
            property.set_name("direction");
            property.set_attribute_name("_DIRECTION");
            attribute.add_property(property);
        }
        {
            let mut property = PropertyAttributeProperty::new();
            property.set_name("magnitude");
            property.set_attribute_name("_MAGNITUDE");
            attribute.add_property(property);
        }
        scene
            .structural_metadata_mut()
            .add_property_attribute(attribute);

        let mesh = scene.mesh_mut(MeshIndex::from(0));
        assert_eq!(mesh.num_faces(), 12);
        assert_eq!(mesh.num_points(), 36);
        let num_vertices = mesh
            .get_named_attribute(GeometryAttributeType::Position)
            .expect("Missing POSITION attribute")
            .size();

        // Property attribute _DIRECTION.
        {
            let mut pa = PointAttribute::new();
            pa.init(
                GeometryAttributeType::Generic,
                3,
                DataType::Float32,
                false,
                num_vertices,
            );
            for avi in 0..num_vertices {
                let val = [avi as f32 + 0.10, avi as f32 + 0.20, avi as f32 + 0.30];
                pa.set_attribute_value_array(AttributeValueIndex::from(avi as u32), &val);
            }
            let att_id = mesh.add_per_vertex_attribute(pa);
            if let Some(att) = mesh.attribute_mut(att_id) {
                att.set_name("_DIRECTION");
            }
        }

        // Property attribute _MAGNITUDE.
        {
            let mut pa = PointAttribute::new();
            pa.init(
                GeometryAttributeType::Generic,
                1,
                DataType::Float32,
                false,
                num_vertices,
            );
            for avi in 0..num_vertices {
                let val = avi as f32;
                pa.set_attribute_value(AttributeValueIndex::from(avi as u32), &val);
            }
            let att_id = mesh.add_per_vertex_attribute(pa);
            if let Some(att) = mesh.attribute_mut(att_id) {
                att.set_name("_MAGNITUDE");
            }
        }

        mesh.add_property_attributes_index(0);
    }

    /// Checks mesh features on a mesh.
    pub fn check_box_meta_mesh_features_mesh(mesh: &Mesh, use_case: &UseCase) {
        Self::check_box_meta_mesh_features(mesh, mesh.non_material_texture_library(), use_case);
    }

    /// Checks mesh features on a scene.
    pub fn check_box_meta_mesh_features_scene(scene: &Scene, use_case: &UseCase) {
        assert_eq!(scene.num_meshes(), 1);
        let mesh = scene.mesh(MeshIndex::from(0));
        Self::check_box_meta_mesh_features(mesh, scene.non_material_texture_library(), use_case);
    }

    /// Checks structural metadata on a mesh.
    pub fn check_box_meta_structural_metadata_mesh(mesh: &Mesh, use_case: &UseCase) {
        Self::check_box_meta_structural_metadata(mesh, mesh.structural_metadata(), use_case);
    }

    /// Checks structural metadata on a scene.
    pub fn check_box_meta_structural_metadata_scene(scene: &Scene, use_case: &UseCase) {
        assert_eq!(scene.num_meshes(), 1);
        let mesh = scene.mesh(MeshIndex::from(0));
        Self::check_box_meta_structural_metadata(mesh, scene.structural_metadata(), use_case);
    }

    fn check_box_meta_mesh_features(mesh: &Mesh, texture_lib: &TextureLibrary, use_case: &UseCase) {
        assert_eq!(texture_lib.num_textures(), 2);

        assert_eq!(mesh.num_mesh_features(), 5);
        assert_eq!(mesh.num_faces(), 12);
        assert_eq!(
            mesh.num_attributes(),
            if use_case.has_structural_metadata {
                9
            } else {
                7
            }
        );
        assert_eq!(mesh.num_points(), 36);
        assert_eq!(
            mesh.num_named_attributes(GeometryAttributeType::Generic),
            if use_case.has_structural_metadata {
                5
            } else {
                3
            }
        );
        assert_eq!(
            mesh.num_named_attributes(GeometryAttributeType::TexCoord),
            2
        );

        let num_faces = mesh.num_faces() as usize;
        let num_corners = num_faces * 3;
        let num_vertices = mesh
            .get_named_attribute(GeometryAttributeType::Position)
            .expect("Missing POSITION attribute")
            .size();

        // Feature set 0.
        {
            let features = mesh.mesh_features(MeshFeaturesIndex::from(0));
            assert_eq!(features.label(), "faces");
            assert_eq!(features.feature_count(), num_faces as i32);
            assert_eq!(features.null_feature_id(), 100);
            assert_eq!(features.property_table_index(), 0);
            assert_eq!(
                features.attribute_index(),
                if use_case.has_structural_metadata {
                    5
                } else {
                    4
                }
            );
            assert!(features.texture_channels().is_empty());
            assert!(features.texture_map().texture().is_none());
            assert_eq!(features.texture_map().tex_coord_index(), -1);

            let att_id = features.attribute_index();
            let att = mesh.attribute(att_id).expect("Missing attribute");
            assert_eq!(att.attribute_type(), GeometryAttributeType::Generic);
            assert_eq!(att.data_type(), DataType::Uint8);
            assert_eq!(att.num_components(), 1);
            assert_eq!(att.size(), num_faces);
            assert_eq!(att.indices_map_size(), num_corners);

            let expected_values: Vec<u8> = if use_case.has_draco_compression {
                vec![7, 11, 10, 3, 2, 5, 4, 1, 6, 9, 8, 0]
            } else {
                vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]
            };
            for i in 0..num_faces {
                let val = att.get_value_array::<u8, 1>(AttributeValueIndex::from(i as u32))[0];
                assert_eq!(val, expected_values[i]);
            }

            for i in 0..num_faces {
                let face = mesh.face(FaceIndex::from(i as u32));
                let idx0 = att.mapped_index(face[0]);
                let idx1 = att.mapped_index(face[1]);
                let idx2 = att.mapped_index(face[2]);
                assert_eq!(
                    idx0, idx1,
                    "per-face: all corners map to same attribute value"
                );
                assert_eq!(idx0, idx2);
            }
        }

        // Feature set 1.
        {
            let features = mesh.mesh_features(MeshFeaturesIndex::from(1));
            assert_eq!(features.label(), "vertices");
            assert_eq!(features.feature_count(), num_vertices as i32);
            assert_eq!(features.null_feature_id(), 101);
            assert_eq!(features.property_table_index(), 1);
            assert_eq!(
                features.attribute_index(),
                if use_case.has_structural_metadata {
                    6
                } else {
                    5
                }
            );
            assert!(features.texture_channels().is_empty());
            assert!(features.texture_map().texture().is_none());
            assert_eq!(features.texture_map().tex_coord_index(), -1);

            let att_id = features.attribute_index();
            let att = mesh.attribute(att_id).expect("Missing attribute");
            assert_eq!(att.attribute_type(), GeometryAttributeType::Generic);
            assert_eq!(att.data_type(), DataType::Uint16);
            assert_eq!(att.num_components(), 1);
            assert_eq!(att.size(), num_vertices);
            assert_eq!(att.indices_map_size(), num_corners);

            let expected_values: Vec<u16> = if use_case.has_draco_compression {
                vec![3, 6, 7, 4, 5, 0, 1, 2]
            } else {
                vec![0, 1, 2, 3, 4, 5, 6, 7]
            };
            for i in 0..num_vertices {
                let val = att.get_value_array::<u16, 1>(AttributeValueIndex::from(i as u32))[0];
                assert_eq!(val, expected_values[i]);
            }

            for i in 0..num_faces {
                let face = mesh.face(FaceIndex::from(i as u32));
                let idx0 = att.mapped_index(face[0]);
                let idx1 = att.mapped_index(face[1]);
                let idx2 = att.mapped_index(face[2]);
                assert_ne!(
                    idx0, idx1,
                    "per-vertex: corners map to different attribute values"
                );
                assert_ne!(idx1, idx2);
                assert_ne!(idx2, idx0);
            }
        }

        // Feature set 2.
        {
            let features = mesh.mesh_features(MeshFeaturesIndex::from(2));
            assert!(features.label().is_empty());
            assert_eq!(features.feature_count(), num_corners as i32);
            assert_eq!(features.null_feature_id(), -1);
            assert_eq!(features.property_table_index(), -1);
            assert_eq!(
                features.attribute_index(),
                if use_case.has_structural_metadata {
                    7
                } else {
                    6
                }
            );
            assert!(features.texture_channels().is_empty());
            assert!(features.texture_map().texture().is_none());
            assert_eq!(features.texture_map().tex_coord_index(), -1);

            let att_id = features.attribute_index();
            let att = mesh.attribute(att_id).expect("Missing attribute");
            assert_eq!(att.attribute_type(), GeometryAttributeType::Generic);
            assert_eq!(att.data_type(), DataType::Float32);
            assert_eq!(att.num_components(), 1);
            assert_eq!(att.size(), num_corners);
            assert_eq!(att.indices_map_size(), 0);
            assert!(att.is_mapping_identity());

            let expected_values: Vec<f32> = if use_case.has_draco_compression {
                vec![
                    23.0, 21.0, 22.0, 33.0, 34.0, 35.0, 31.0, 32.0, 30.0, 9.0, 10.0, 11.0, 7.0,
                    8.0, 6.0, 15.0, 16.0, 17.0, 14.0, 12.0, 13.0, 5.0, 3.0, 4.0, 19.0, 20.0, 18.0,
                    27.0, 28.0, 29.0, 26.0, 24.0, 25.0, 1.0, 2.0, 0.0,
                ]
            } else {
                (0..num_corners).map(|v| v as f32).collect()
            };
            for i in 0..num_corners {
                let val = att.get_value_array::<f32, 1>(AttributeValueIndex::from(i as u32))[0];
                assert_eq!(val, expected_values[i]);
            }

            for i in 0..num_faces {
                let face = mesh.face(FaceIndex::from(i as u32));
                let v0 = att.get_value_array::<f32, 1>(att.mapped_index(face[0]))[0];
                let v1 = att.get_value_array::<f32, 1>(att.mapped_index(face[1]))[0];
                let v2 = att.get_value_array::<f32, 1>(att.mapped_index(face[2]))[0];
                assert_eq!(v0, expected_values[3 * i + 0]);
                assert_eq!(v1, expected_values[3 * i + 1]);
                assert_eq!(v2, expected_values[3 * i + 2]);
            }
        }

        // Feature set 3.
        {
            let features = mesh.mesh_features(MeshFeaturesIndex::from(3));
            assert!(features.label().is_empty());
            assert_eq!(features.feature_count(), 6);
            assert_eq!(features.null_feature_id(), -1);
            assert_eq!(features.property_table_index(), -1);
            assert_eq!(features.attribute_index(), -1);
        }

        // Feature set 4.
        {
            let features = mesh.mesh_features(MeshFeaturesIndex::from(4));
            assert_eq!(features.label(), "water");
            assert_eq!(features.feature_count(), 2);
            assert_eq!(features.null_feature_id(), -1);
            assert_eq!(features.property_table_index(), -1);
            assert_eq!(features.attribute_index(), -1);
        }
    }

    fn check_box_meta_structural_metadata(
        mesh: &Mesh,
        structural_metadata: &StructuralMetadata,
        use_case: &UseCase,
    ) {
        // Schema checks.
        {
            let schema = structural_metadata.schema();
            assert!(!schema.is_empty());
            let json = &schema.json;
            assert_eq!(json.objects().len(), 3);
            assert_eq!(json.objects()[0].name(), "classes");
            assert_eq!(json.objects()[0].objects().len(), 2);

            // Class movement.
            {
                let item = &json.objects()[0].objects()[0];
                assert_eq!(item.name(), "movement");
                assert_eq!(item.objects().len(), 3);

                let description = &item.objects()[0];
                assert_eq!(description.name(), "description");
                assert_eq!(description.string(), "Vertex movement.");

                let name = &item.objects()[1];
                assert_eq!(name.name(), "name");
                assert_eq!(name.string(), "The movement.");

                let properties = &item.objects()[2];
                assert_eq!(properties.name(), "properties");
                assert_eq!(properties.objects().len(), 2);

                let direction = &properties.objects()[0];
                assert_eq!(direction.name(), "direction");
                assert_eq!(direction.objects().len(), 4);
                assert_eq!(direction.objects()[0].name(), "componentType");
                assert_eq!(direction.objects()[1].name(), "description");
                assert_eq!(direction.objects()[2].name(), "required");
                assert_eq!(direction.objects()[3].name(), "type");
                assert_eq!(direction.objects()[0].string(), "FLOAT32");
                assert_eq!(direction.objects()[1].string(), "Movement direction.");
                assert_eq!(direction.objects()[2].boolean(), true);
                assert_eq!(direction.objects()[3].string(), "VEC3");

                let mag = &properties.objects()[1];
                assert_eq!(mag.name(), "magnitude");
                assert_eq!(mag.objects().len(), 4);
                assert_eq!(mag.objects()[0].name(), "componentType");
                assert_eq!(mag.objects()[1].name(), "description");
                assert_eq!(mag.objects()[2].name(), "required");
                assert_eq!(mag.objects()[3].name(), "type");
                assert_eq!(mag.objects()[0].string(), "FLOAT32");
                assert_eq!(mag.objects()[1].string(), "Movement magnitude.");
                assert_eq!(mag.objects()[2].boolean(), true);
                assert_eq!(mag.objects()[3].string(), "SCALAR");
            }

            // Class planet.
            {
                let item = &json.objects()[0].objects()[1];
                assert_eq!(item.name(), "planet");
                assert_eq!(item.objects().len(), 1);

                let properties = &item.objects()[0];
                assert_eq!(properties.name(), "properties");
                assert_eq!(properties.objects().len(), 3);

                let color = &properties.objects()[0];
                assert_eq!(color.name(), "color");
                assert_eq!(color.objects().len(), 4);
                assert_eq!(color.objects()[0].name(), "componentType");
                assert_eq!(color.objects()[1].name(), "description");
                assert_eq!(color.objects()[2].name(), "required");
                assert_eq!(color.objects()[3].name(), "type");
                assert_eq!(color.objects()[0].string(), "UINT8");
                assert_eq!(color.objects()[1].string(), "The RGB color.");
                assert!(color.objects()[2].boolean());
                assert_eq!(color.objects()[3].string(), "VEC3");

                let name = &properties.objects()[1];
                assert_eq!(name.name(), "name");
                assert_eq!(name.objects().len(), 3);
                assert_eq!(name.objects()[0].name(), "description");
                assert_eq!(name.objects()[1].name(), "required");
                assert_eq!(name.objects()[2].name(), "type");
                assert_eq!(name.objects()[0].string(), "The name.");
                assert!(name.objects()[1].boolean());
                assert_eq!(name.objects()[2].string(), "STRING");

                let sequence = &properties.objects()[2];
                assert_eq!(sequence.name(), "sequence");
                assert_eq!(sequence.objects().len(), 4);
                assert_eq!(sequence.objects()[0].name(), "componentType");
                assert_eq!(sequence.objects()[1].name(), "description");
                assert_eq!(sequence.objects()[2].name(), "required");
                assert_eq!(sequence.objects()[3].name(), "type");
                assert_eq!(sequence.objects()[0].string(), "FLOAT32");
                assert_eq!(sequence.objects()[1].string(), "The number sequence.");
                assert!(!sequence.objects()[2].boolean());
                assert_eq!(sequence.objects()[3].string(), "SCALAR");
            }

            assert_eq!(json.objects()[1].name(), "enums");
            let classifications = &json.objects()[1].objects()[0];
            assert_eq!(classifications.name(), "classifications");
            assert_eq!(classifications.objects()[0].name(), "description");
            assert_eq!(
                classifications.objects()[0].string(),
                "Classifications of planets."
            );
            assert_eq!(classifications.objects()[1].name(), "name");
            assert_eq!(classifications.objects()[1].string(), "classifications");
            assert_eq!(classifications.objects()[2].name(), "values");
            let values = &classifications.objects()[2];
            assert_eq!(values.array()[0].objects()[0].name(), "name");
            assert_eq!(values.array()[1].objects()[0].name(), "name");
            assert_eq!(values.array()[2].objects()[0].name(), "name");
            assert_eq!(values.array()[3].objects()[0].name(), "name");
            assert_eq!(values.array()[4].objects()[0].name(), "name");
            assert_eq!(values.array()[0].objects()[0].string(), "Unspecified");
            assert_eq!(values.array()[1].objects()[0].string(), "Gas Giant");
            assert_eq!(values.array()[2].objects()[0].string(), "Waterworld");
            assert_eq!(values.array()[3].objects()[0].string(), "Agriworld");
            assert_eq!(values.array()[4].objects()[0].string(), "Ordnance");
            assert_eq!(values.array()[0].objects()[1].name(), "value");
            assert_eq!(values.array()[1].objects()[1].name(), "value");
            assert_eq!(values.array()[2].objects()[1].name(), "value");
            assert_eq!(values.array()[3].objects()[1].name(), "value");
            assert_eq!(values.array()[4].objects()[1].name(), "value");
            assert_eq!(values.array()[0].objects()[1].integer(), 0);
            assert_eq!(values.array()[1].objects()[1].integer(), 1);
            assert_eq!(values.array()[2].objects()[1].integer(), 2);
            assert_eq!(values.array()[3].objects()[1].integer(), 3);
            assert_eq!(values.array()[4].objects()[1].integer(), 4);

            assert_eq!(json.objects()[2].name(), "id");
            assert_eq!(json.objects()[2].string(), "galaxy");
        }

        // Property table checks.
        const K_ROWS: usize = 16;
        assert_eq!(structural_metadata.num_property_tables(), 1);
        let table = structural_metadata.property_table(0);
        assert_eq!(table.name(), "Galaxy far far away.");
        assert_eq!(table.class(), "planet");
        assert_eq!(table.count(), K_ROWS as i32);
        assert_eq!(table.num_properties(), 3);

        // Property 0: color.
        {
            let property = table.property(0);
            assert_eq!(property.name(), "color");
            assert_eq!(property.data().data.len(), K_ROWS * 3);
            assert_eq!(property.data().target, 34962);

            assert_eq!(property.data().data[0], 94);
            assert_eq!(property.data().data[1], 94);
            assert_eq!(property.data().data[2], 194);
            assert_eq!(property.data().data[18], 190);
            assert_eq!(property.data().data[19], 92);
            assert_eq!(property.data().data[20], 108);
            assert_eq!(property.data().data[45], 0);
            assert_eq!(property.data().data[46], 0);
            assert_eq!(property.data().data[47], 0);

            assert!(property.array_offsets().type_name.is_empty());
            assert!(property.array_offsets().data.data.is_empty());
            assert_eq!(property.array_offsets().data.target, 0);
            assert!(property.string_offsets().type_name.is_empty());
            assert!(property.string_offsets().data.data.is_empty());
            assert_eq!(property.string_offsets().data.target, 0);
        }

        // Property 1: name.
        {
            let property = table.property(1);
            assert_eq!(property.name(), "name");
            let data = &property.data().data;
            let offsets = &property.string_offsets().data.data;

            assert_eq!(data.len(), 296);
            assert_eq!(property.data().target, 34963);
            assert_eq!(property.string_offsets().type_name, "UINT32");
            assert_eq!(offsets.len(), 4 * (K_ROWS + 1));
            assert_eq!(property.string_offsets().data.target, 34963);

            assert_eq!(offsets[0], 0);
            assert_eq!(offsets[1], 0);
            assert_eq!(offsets[2], 0);
            assert_eq!(offsets[3], 0);
            assert_eq!(offsets[60], 31);
            assert_eq!(offsets[61], 1);
            assert_eq!(offsets[62], 0);
            assert_eq!(offsets[63], 0);
            assert_eq!(offsets[64], 40);
            assert_eq!(offsets[65], 1);
            assert_eq!(offsets[66], 0);
            assert_eq!(offsets[67], 0);

            let extract_name = |row: usize| -> String {
                let b = offsets[4 * row] as usize + 256 * offsets[4 * row + 1] as usize;
                let e = offsets[4 * (row + 1)] as usize + 256 * offsets[4 * (row + 1) + 1] as usize;
                String::from_utf8_lossy(&data[b..e]).to_string()
            };

            assert_eq!(extract_name(0), "named_class:Tatooine");
            assert_eq!(extract_name(6), "named_class:Corellia");
            assert_eq!(extract_name(12), "named_class:Bespin");
            assert_eq!(extract_name(13), "named_class:Yavin");
            assert_eq!(extract_name(14), "named_class:Geonosis");
            assert_eq!(extract_name(15), "UNLABELED");

            assert!(property.array_offsets().type_name.is_empty());
            assert!(property.array_offsets().data.data.is_empty());
            assert_eq!(property.array_offsets().data.target, 0);
        }

        // Property 2: sequence.
        {
            let property = table.property(2);
            assert_eq!(property.name(), "sequence");
            let data = &property.data().data;
            let offsets = &property.array_offsets().data.data;

            assert_eq!(data.len(), 41 * 4);
            assert_eq!(property.data().target, 34963);
            assert_eq!(property.array_offsets().type_name, "UINT8");
            assert_eq!(offsets.len(), 20);
            assert_eq!(property.array_offsets().data.target, 34963);

            assert_eq!(offsets[0], 0 * 4);
            assert_eq!(offsets[1], 6 * 4);
            assert_eq!(offsets[6], 16 * 4);
            assert_eq!(offsets[14], 36 * 4);
            assert_eq!(offsets[15], 41 * 4);
            assert_eq!(offsets[16], 41 * 4);

            let extract_sequence = |row: usize| -> Vec<f32> {
                let start = offsets[row] as usize;
                let end = offsets[row + 1] as usize;
                let count = (end - start) / 4;
                let mut result = Vec::with_capacity(count);
                for i in 0..count {
                    let offset = start + 4 * i;
                    let bytes = &data[offset..offset + 4];
                    result.push(f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]));
                }
                result
            };

            assert_eq!(extract_sequence(0), vec![0.5, 1.5, 2.5, 3.5, 4.5, 5.5]);
            assert_eq!(extract_sequence(1), vec![6.5, 7.5]);
            assert_eq!(extract_sequence(14), vec![36.5, 37.5, 38.5, 39.5, 40.5]);
            assert!(extract_sequence(15).is_empty());

            assert!(property.string_offsets().type_name.is_empty());
            assert!(property.string_offsets().data.data.is_empty());
            assert_eq!(property.string_offsets().data.target, 0);
        }

        // Property attributes in structural metadata.
        assert_eq!(structural_metadata.num_property_attributes(), 1);
        {
            let attribute = structural_metadata.property_attribute(0);
            assert_eq!(attribute.name(), "The movement.");
            assert_eq!(attribute.class(), "movement");
            assert_eq!(attribute.num_properties(), 2);

            let direction = attribute.property(0);
            assert_eq!(direction.name(), "direction");
            assert_eq!(direction.attribute_name(), "_DIRECTION");

            let magnitude = attribute.property(1);
            assert_eq!(magnitude.name(), "magnitude");
            assert_eq!(magnitude.attribute_name(), "_MAGNITUDE");
        }

        // Property attributes in mesh.
        assert_eq!(mesh.num_property_attributes_indices(), 1);
        assert_eq!(mesh.property_attributes_index(0), 0);
        assert_eq!(mesh.num_faces(), 12);
        assert_eq!(mesh.num_attributes(), 9);
        assert_eq!(mesh.num_points(), 36);
        assert_eq!(mesh.num_named_attributes(GeometryAttributeType::Generic), 5);

        let num_faces = mesh.num_faces() as usize;
        let num_corners = num_faces * 3;
        let num_vertices = mesh
            .get_named_attribute(GeometryAttributeType::Position)
            .expect("Missing POSITION attribute")
            .size();

        // _DIRECTION attribute.
        {
            let att = mesh
                .get_named_attribute_by_name(GeometryAttributeType::Generic, "_DIRECTION")
                .expect("Missing _DIRECTION attribute");
            assert_eq!(att.attribute_type(), GeometryAttributeType::Generic);
            assert_eq!(att.data_type(), DataType::Float32);
            assert_eq!(att.num_components(), 3);
            assert_eq!(att.size(), num_vertices);
            assert_eq!(att.indices_map_size(), num_corners);

            let expected_values: Vec<f32> = if use_case.has_draco_compression {
                vec![
                    3.1, 3.2, 3.3, 6.1, 6.2, 6.3, 7.1, 7.2, 7.3, 4.1, 4.2, 4.3, 5.1, 5.2, 5.3, 0.1,
                    0.2, 0.3, 1.1, 1.2, 1.3, 2.1, 2.2, 2.3,
                ]
            } else {
                vec![
                    0.1, 0.2, 0.3, 1.1, 1.2, 1.3, 2.1, 2.2, 2.3, 3.1, 3.2, 3.3, 4.1, 4.2, 4.3, 5.1,
                    5.2, 5.3, 6.1, 6.2, 6.3, 7.1, 7.2, 7.3,
                ]
            };
            for i in 0..num_vertices {
                let val = att.get_value_array::<f32, 3>(AttributeValueIndex::from(i as u32));
                assert_eq!(val[0], expected_values[3 * i + 0]);
                assert_eq!(val[1], expected_values[3 * i + 1]);
                assert_eq!(val[2], expected_values[3 * i + 2]);
            }
        }

        // _MAGNITUDE attribute.
        {
            let att = mesh
                .get_named_attribute_by_name(GeometryAttributeType::Generic, "_MAGNITUDE")
                .expect("Missing _MAGNITUDE attribute");
            assert_eq!(att.attribute_type(), GeometryAttributeType::Generic);
            assert_eq!(att.data_type(), DataType::Float32);
            assert_eq!(att.num_components(), 1);
            assert_eq!(att.size(), num_vertices);
            assert_eq!(att.indices_map_size(), num_corners);

            let expected_values: Vec<f32> = if use_case.has_draco_compression {
                vec![3.0, 6.0, 7.0, 4.0, 5.0, 0.0, 1.0, 2.0]
            } else {
                vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0]
            };
            for i in 0..num_vertices {
                let val = att.get_value_array::<f32, 1>(AttributeValueIndex::from(i as u32))[0];
                assert_eq!(val, expected_values[i]);
            }
        }
    }

    fn build_structural_metadata_schema() -> StructuralMetadataSchema {
        let mut schema = StructuralMetadataSchema::new();

        let mut classes = Object::with_name("classes");

        // Class planet.
        {
            let mut planet = Object::with_name("planet");
            let mut properties = Object::with_name("properties");

            let mut color = Object::with_name("color");
            color
                .set_objects()
                .push(Object::with_string("componentType", "UINT8"));
            color
                .set_objects()
                .push(Object::with_string("description", "The RGB color."));
            color
                .set_objects()
                .push(Object::with_boolean("required", true));
            color
                .set_objects()
                .push(Object::with_string("type", "VEC3"));

            let mut name = Object::with_name("name");
            name.set_objects()
                .push(Object::with_string("description", "The name."));
            name.set_objects()
                .push(Object::with_boolean("required", true));
            name.set_objects()
                .push(Object::with_string("type", "STRING"));

            let mut sequence = Object::with_name("sequence");
            sequence
                .set_objects()
                .push(Object::with_string("componentType", "FLOAT32"));
            sequence
                .set_objects()
                .push(Object::with_string("description", "The number sequence."));
            sequence
                .set_objects()
                .push(Object::with_boolean("required", false));
            sequence
                .set_objects()
                .push(Object::with_string("type", "SCALAR"));

            properties.set_objects().push(color);
            properties.set_objects().push(name);
            properties.set_objects().push(sequence);

            planet.set_objects().push(properties);
            classes.set_objects().push(planet);
        }

        // Class movement.
        {
            let mut movement = Object::with_name("movement");
            movement
                .set_objects()
                .push(Object::with_string("name", "The movement."));
            movement
                .set_objects()
                .push(Object::with_string("description", "Vertex movement."));
            let mut properties = Object::with_name("properties");

            let mut direction = Object::with_name("direction");
            direction
                .set_objects()
                .push(Object::with_string("componentType", "FLOAT32"));
            direction
                .set_objects()
                .push(Object::with_string("description", "Movement direction."));
            direction
                .set_objects()
                .push(Object::with_boolean("required", true));
            direction
                .set_objects()
                .push(Object::with_string("type", "VEC3"));

            let mut magnitude = Object::with_name("magnitude");
            magnitude
                .set_objects()
                .push(Object::with_string("componentType", "FLOAT32"));
            magnitude
                .set_objects()
                .push(Object::with_string("description", "Movement magnitude."));
            magnitude
                .set_objects()
                .push(Object::with_boolean("required", true));
            magnitude
                .set_objects()
                .push(Object::with_string("type", "SCALAR"));

            properties.set_objects().push(direction);
            properties.set_objects().push(magnitude);
            movement.set_objects().push(properties);
            classes.set_objects().push(movement);
        }

        let mut enums = Object::with_name("enums");
        {
            let mut classifications = Object::with_name("classifications");
            classifications.set_objects().push(Object::with_string(
                "description",
                "Classifications of planets.",
            ));
            classifications
                .set_objects()
                .push(Object::with_string("name", "classifications"));
            let mut values = Object::with_name("values");

            let mut value = Object::new();
            value
                .set_objects()
                .push(Object::with_string("name", "Unspecified"));
            value.set_objects().push(Object::with_integer("value", 0));
            values.set_array().push(value);

            let mut value = Object::new();
            value
                .set_objects()
                .push(Object::with_string("name", "Gas Giant"));
            value.set_objects().push(Object::with_integer("value", 1));
            values.set_array().push(value);

            let mut value = Object::new();
            value
                .set_objects()
                .push(Object::with_string("name", "Waterworld"));
            value.set_objects().push(Object::with_integer("value", 2));
            values.set_array().push(value);

            let mut value = Object::new();
            value
                .set_objects()
                .push(Object::with_string("name", "Agriworld"));
            value.set_objects().push(Object::with_integer("value", 3));
            values.set_array().push(value);

            let mut value = Object::new();
            value
                .set_objects()
                .push(Object::with_string("name", "Ordnance"));
            value.set_objects().push(Object::with_integer("value", 4));
            values.set_array().push(value);

            classifications.set_objects().push(values);
            enums.set_objects().push(classifications);
        }

        schema
            .json
            .set_objects()
            .push(Object::with_string("id", "galaxy"));
        schema.json.set_objects().push(classes);
        schema.json.set_objects().push(enums);

        schema
    }
}
