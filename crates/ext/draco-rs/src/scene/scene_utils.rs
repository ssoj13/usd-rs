//! Scene utilities.
//!
//! What: Helper algorithms for scene graphs, mesh instances, and cleanup.
//! Why: Mirrors Draco `scene_utils` and supports glTF IO parity.
//! How: Traverses scene hierarchy, aggregates instances, and updates metadata.
//! Where used: glTF encoder/decoder tests and scene processing.

use std::collections::{HashMap, HashSet};

use draco_core::attributes::geometry_attribute::GeometryAttributeType;
use draco_core::core::bounding_box::BoundingBox;
use draco_core::core::draco_index_type_vector::IndexTypeVector;
use draco_core::core::status::{Status, StatusCode};
use draco_core::core::status_or::StatusOr;
use draco_core::mesh::mesh_splitter::MeshSplitter;
use draco_core::mesh::mesh_utils::{Matrix4d, MeshUtils};
use draco_core::mesh::Mesh;

use crate::scene::mesh_group::{MeshGroup, MeshInstance as GroupMeshInstance};
use crate::scene::scene_indices::{
    MeshGroupIndex, MeshIndex, MeshInstanceIndex, SceneNodeIndex, INVALID_MESH_GROUP_INDEX,
    INVALID_MESH_INDEX, INVALID_SCENE_NODE_INDEX,
};
use crate::scene::Scene;

/// Mesh instance with transform in scene space.
#[derive(Clone, Debug)]
pub struct SceneMeshInstance {
    pub mesh_index: MeshIndex,
    pub scene_node_index: SceneNodeIndex,
    pub mesh_group_mesh_index: i32,
    pub transform: Matrix4d,
}

/// Cleanup options for scene graphs.
#[derive(Clone, Copy, Debug)]
pub struct CleanupOptions {
    pub remove_invalid_mesh_instances: bool,
    pub remove_unused_mesh_groups: bool,
    pub remove_unused_meshes: bool,
    pub remove_unused_nodes: bool,
    pub remove_unused_tex_coords: bool,
    pub remove_unused_materials: bool,
}

impl Default for CleanupOptions {
    fn default() -> Self {
        Self {
            remove_invalid_mesh_instances: true,
            remove_unused_mesh_groups: true,
            remove_unused_meshes: true,
            remove_unused_nodes: false,
            remove_unused_tex_coords: false,
            remove_unused_materials: true,
        }
    }
}

/// Helper class containing various utility functions operating on draco::Scene.
pub struct SceneUtils;

impl SceneUtils {
    /// Computes all mesh instances in the scene.
    pub fn compute_all_instances(
        scene: &Scene,
    ) -> IndexTypeVector<MeshInstanceIndex, SceneMeshInstance> {
        let mut instances = IndexTypeVector::new();
        for i in 0..scene.num_root_nodes() {
            let node_instances =
                Self::compute_all_instances_from_node(scene, scene.root_node_index(i));
            let old_size = instances.size();
            instances.resize_with_value(
                old_size + node_instances.size(),
                SceneMeshInstance {
                    mesh_index: INVALID_MESH_INDEX,
                    scene_node_index: INVALID_SCENE_NODE_INDEX,
                    mesh_group_mesh_index: -1,
                    transform: Matrix4d::identity(),
                },
            );
            for mii in 0..node_instances.size() {
                instances[MeshInstanceIndex::from((old_size + mii) as u32)] =
                    node_instances[MeshInstanceIndex::from(mii as u32)].clone();
            }
        }
        instances
    }

    /// Computes all mesh instances under a node, relative to that node.
    pub fn compute_all_instances_from_node(
        scene: &Scene,
        node_index: SceneNodeIndex,
    ) -> IndexTypeVector<MeshInstanceIndex, SceneMeshInstance> {
        let mut instances = IndexTypeVector::new();
        let mut stack = vec![NodeTraversal {
            scene_node_index: node_index,
            transform: Matrix4d::identity(),
        }];

        while let Some(node) = stack.pop() {
            let scene_node = scene.node(node.scene_node_index);
            let combined_transform = node
                .transform
                .mul(&scene_node.trs_matrix().compute_transformation_matrix());

            let mesh_group_index = scene_node.mesh_group_index();
            if mesh_group_index != INVALID_MESH_GROUP_INDEX {
                let mesh_group = scene.mesh_group(mesh_group_index);
                for i in 0..mesh_group.num_mesh_instances() {
                    let mesh_index = mesh_group.mesh_instance(i).mesh_index;
                    if mesh_index != INVALID_MESH_INDEX {
                        instances.push_back(SceneMeshInstance {
                            mesh_index,
                            scene_node_index: node.scene_node_index,
                            mesh_group_mesh_index: i,
                            transform: combined_transform,
                        });
                    }
                }
            }

            for i in 0..scene_node.num_children() {
                stack.push(NodeTraversal {
                    scene_node_index: scene_node.child(i),
                    transform: combined_transform,
                });
            }
        }

        instances
    }

    /// Computes global transform matrix of a scene node.
    pub fn compute_global_node_transform(scene: &Scene, mut index: SceneNodeIndex) -> Matrix4d {
        let mut transform = Matrix4d::identity();
        while index != INVALID_SCENE_NODE_INDEX {
            let node = scene.node(index);
            transform = node
                .trs_matrix()
                .compute_transformation_matrix()
                .mul(&transform);
            index = if node.num_parents() == 1 {
                node.parent(0)
            } else {
                INVALID_SCENE_NODE_INDEX
            };
        }
        transform
    }

    /// Returns mesh instance counts for all base meshes.
    pub fn num_mesh_instances(scene: &Scene) -> IndexTypeVector<MeshIndex, i32> {
        let instances = Self::compute_all_instances(scene);
        let mut counts = IndexTypeVector::with_size_value(scene.num_meshes() as usize, 0);
        for i in 0..instances.size() {
            let instance = &instances[MeshInstanceIndex::from(i as u32)];
            counts[instance.mesh_index] += 1;
        }
        counts
    }

    /// Returns the material index for a mesh instance.
    pub fn get_mesh_instance_material_index(scene: &Scene, instance: &SceneMeshInstance) -> i32 {
        let node = scene.node(instance.scene_node_index);
        scene
            .mesh_group(node.mesh_group_index())
            .mesh_instance(instance.mesh_group_mesh_index)
            .material_index
    }

    /// Returns total faces on base meshes.
    pub fn num_faces_on_base_meshes(scene: &Scene) -> i32 {
        let mut num_faces = 0;
        for i in 0..scene.num_meshes() {
            num_faces += scene.mesh(MeshIndex::from(i as u32)).num_faces() as i32;
        }
        num_faces
    }

    /// Returns total faces including instances.
    pub fn num_faces_on_instanced_meshes(scene: &Scene) -> i32 {
        let instances = Self::compute_all_instances(scene);
        let mut num_faces = 0;
        for i in 0..instances.size() {
            let instance = &instances[MeshInstanceIndex::from(i as u32)];
            num_faces += scene.mesh(instance.mesh_index).num_faces() as i32;
        }
        num_faces
    }

    /// Returns total points on base meshes.
    pub fn num_points_on_base_meshes(scene: &Scene) -> i32 {
        let mut num_points = 0;
        for i in 0..scene.num_meshes() {
            num_points += scene.mesh(MeshIndex::from(i as u32)).num_points() as i32;
        }
        num_points
    }

    /// Returns total points including instances.
    pub fn num_points_on_instanced_meshes(scene: &Scene) -> i32 {
        let instances = Self::compute_all_instances(scene);
        let mut num_points = 0;
        for i in 0..instances.size() {
            let instance = &instances[MeshInstanceIndex::from(i as u32)];
            num_points += scene.mesh(instance.mesh_index).num_points() as i32;
        }
        num_points
    }

    /// Returns total attribute entries on base meshes.
    pub fn num_att_entries_on_base_meshes(scene: &Scene, att_type: GeometryAttributeType) -> i32 {
        let mut num_entries = 0;
        for i in 0..scene.num_meshes() {
            let mesh = scene.mesh(MeshIndex::from(i as u32));
            if let Some(att) = mesh.get_named_attribute(att_type) {
                num_entries += att.size() as i32;
            }
        }
        num_entries
    }

    /// Returns total attribute entries including instances.
    pub fn num_att_entries_on_instanced_meshes(
        scene: &Scene,
        att_type: GeometryAttributeType,
    ) -> i32 {
        let instances = Self::compute_all_instances(scene);
        let mut num_entries = 0;
        for i in 0..instances.size() {
            let instance = &instances[MeshInstanceIndex::from(i as u32)];
            let mesh = scene.mesh(instance.mesh_index);
            if let Some(att) = mesh.get_named_attribute(att_type) {
                num_entries += att.size() as i32;
            }
        }
        num_entries
    }

    /// Returns the bounding box of the scene.
    pub fn compute_bounding_box(scene: &Scene) -> BoundingBox {
        let instances = Self::compute_all_instances(scene);
        let mut bbox = BoundingBox::default();
        for i in 0..instances.size() {
            let instance = &instances[MeshInstanceIndex::from(i as u32)];
            let mesh_bbox = Self::compute_mesh_instance_bounding_box(scene, instance);
            bbox.update_box(&mesh_bbox);
        }
        bbox
    }

    /// Returns the bounding box of a mesh instance.
    pub fn compute_mesh_instance_bounding_box(
        scene: &Scene,
        instance: &SceneMeshInstance,
    ) -> BoundingBox {
        let mesh = scene.mesh(instance.mesh_index);
        let pos_att = match mesh.get_named_attribute(GeometryAttributeType::Position) {
            Some(att) => att,
            None => return BoundingBox::default(),
        };
        let mut bbox = BoundingBox::default();
        let mut position = [0.0_f64; 3];
        for i in 0..pos_att.size() {
            pos_att.convert_value(
                draco_core::attributes::geometry_indices::AttributeValueIndex::from(i as u32),
                3,
                &mut position,
            );
            let transformed =
                instance
                    .transform
                    .mul_vec4([position[0], position[1], position[2], 1.0]);
            bbox.update_point(&draco_core::core::vector_d::Vector3f::new3(
                transformed[0] as f32,
                transformed[1] as f32,
                transformed[2] as f32,
            ));
        }
        bbox
    }

    /// Prints info about input and simplified scenes.
    pub fn print_info(input: &Scene, simplified: &Scene, verbose: bool) {
        let print_instanced_info = {
            let input_instances = Self::num_mesh_instances(input);
            let simplified_instances = Self::num_mesh_instances(simplified);
            if input_instances.size() != simplified_instances.size() {
                true
            } else {
                let mut any_instanced = false;
                for i in 0..input_instances.size() {
                    if input_instances[MeshIndex::from(i as u32)] != 1
                        || simplified_instances[MeshIndex::from(i as u32)] != 1
                    {
                        any_instanced = true;
                        break;
                    }
                }
                any_instanced
            }
        };

        println!("\n");
        if print_instanced_info {
            println!("{:>21} |   geometry:         base    instanced", "");
        } else {
            println!("{:>21} |   geometry:         base", "");
        }

        let print_row = |label: &str,
                         count_input_base: i32,
                         count_input_instanced: i32,
                         count_simplified_base: i32,
                         count_simplified_instanced: i32| {
            if count_input_base == 0 && count_input_instanced == 0 {
                return;
            }
            if print_instanced_info {
                println!("  -----------------------------------------------------------");
                println!(
                    "{:>21} |      input: {:12} {:12}",
                    label, count_input_base, count_input_instanced
                );
                println!(
                    "{:>21} | simplified: {:12} {:12}",
                    "", count_simplified_base, count_simplified_instanced
                );
            } else {
                println!("  ----------------------------------------------");
                println!("{:>21} |      input: {:12}", label, count_input_base);
                println!("{:>21} | simplified: {:12}", "", count_simplified_base);
            }
        };

        let print_att_row = |label: &str, att_type: GeometryAttributeType| {
            print_row(
                label,
                Self::num_att_entries_on_base_meshes(input, att_type),
                Self::num_att_entries_on_instanced_meshes(input, att_type),
                Self::num_att_entries_on_base_meshes(simplified, att_type),
                Self::num_att_entries_on_instanced_meshes(simplified, att_type),
            );
        };

        if verbose {
            let num_meshes_input_base = input.num_meshes();
            let num_meshes_simplified_base = simplified.num_meshes();
            let num_meshes_input_instanced = Self::compute_all_instances(input).size() as i32;
            let num_meshes_simplified_instanced =
                Self::compute_all_instances(simplified).size() as i32;
            print_row(
                "Number of meshes",
                num_meshes_input_base,
                num_meshes_input_instanced,
                num_meshes_simplified_base,
                num_meshes_simplified_instanced,
            );
        }

        print_row(
            "Number of faces",
            Self::num_faces_on_base_meshes(input),
            Self::num_faces_on_instanced_meshes(input),
            Self::num_faces_on_base_meshes(simplified),
            Self::num_faces_on_instanced_meshes(simplified),
        );

        if verbose {
            print_row(
                "Number of points",
                Self::num_points_on_base_meshes(input),
                Self::num_points_on_instanced_meshes(input),
                Self::num_points_on_base_meshes(simplified),
                Self::num_points_on_instanced_meshes(simplified),
            );
            print_att_row("Number of positions", GeometryAttributeType::Position);
            print_att_row("Number of normals", GeometryAttributeType::Normal);
            print_att_row("Number of colors", GeometryAttributeType::Color);
            print_row(
                "Number of materials",
                input.material_library().num_materials() as i32,
                input.material_library().num_materials() as i32,
                simplified.material_library().num_materials() as i32,
                simplified.material_library().num_materials() as i32,
            );
        }
    }

    /// Converts a draco::Mesh into a draco::Scene.
    pub fn mesh_to_scene(mut mesh: Box<Mesh>, deduplicate_vertices: bool) -> StatusOr<Box<Scene>> {
        let num_mesh_materials = mesh.material_library().num_materials();
        let mut scene = Box::new(Scene::new());
        if num_mesh_materials > 0 {
            scene
                .material_library_mut()
                .copy_from(mesh.material_library());
            mesh.material_library_mut().clear();
        } else {
            scene.material_library_mut().mutable_material(0);
        }

        scene
            .structural_metadata_mut()
            .copy_from(mesh.structural_metadata());

        scene
            .non_material_texture_library_mut()
            .copy_from(mesh.non_material_texture_library());

        let old_texture_to_index_map = mesh
            .non_material_texture_library()
            .compute_texture_to_index_map();

        let scene_node_index = scene.add_node();
        let mesh_group_index = scene.add_mesh_group();

        if num_mesh_materials <= 1 {
            let mesh_index = scene.add_mesh(mesh);
            if mesh_index == INVALID_MESH_INDEX {
                return StatusOr::new_status(Status::new(
                    StatusCode::DracoError,
                    "Could not add Draco mesh to scene.",
                ));
            }
            scene
                .mesh_group_mut(mesh_group_index)
                .add_mesh_instance(GroupMeshInstance::new(mesh_index, 0));

            update_mesh_features_textures_on_mesh(
                &old_texture_to_index_map,
                scene.as_mut(),
                mesh_index,
            );

            scene
                .mesh_mut(mesh_index)
                .non_material_texture_library_mut()
                .clear();
        } else {
            let mat_att_id = mesh.get_named_attribute_id(GeometryAttributeType::Material);
            if mat_att_id == -1 {
                return StatusOr::new_status(Status::new(
                    StatusCode::DracoError,
                    "Internal error in MeshToScene: GetNamedAttributeId(MATERIAL) returned -1",
                ));
            }
            let mat_att = mesh.get_named_attribute(GeometryAttributeType::Material);
            if mat_att.is_none() {
                return StatusOr::new_status(Status::new(
                    StatusCode::DracoError,
                    "Internal error in MeshToScene: GetNamedAttribute(MATERIAL) returned nullptr",
                ));
            }
            let mat_att = mat_att.unwrap();

            let mut splitter = MeshSplitter::new();
            splitter.set_deduplicate_vertices(deduplicate_vertices);
            let split_meshes_or = splitter.split_mesh(&mesh, mat_att_id as u32);
            if !split_meshes_or.is_ok() {
                return StatusOr::new_status(split_meshes_or.status().clone());
            }
            let split_meshes = split_meshes_or.into_value();

            for (i, split_mesh) in split_meshes.into_iter().enumerate() {
                if split_mesh.is_none() {
                    continue;
                }
                let mesh_index = scene.add_mesh(split_mesh.unwrap());
                if mesh_index == INVALID_MESH_INDEX {
                    return StatusOr::new_status(Status::new(
                        StatusCode::DracoError,
                        "Could not add Draco mesh to scene.",
                    ));
                }

                let mut mat_value = [0u32; 1];
                let _ = mat_att.get_value_array_into(
                    draco_core::attributes::geometry_indices::AttributeValueIndex::from(i as u32),
                    &mut mat_value,
                );
                let material_index = mat_value[0] as i32;
                scene
                    .mesh_group_mut(mesh_group_index)
                    .add_mesh_instance(GroupMeshInstance::new(mesh_index, material_index));

                {
                    let scene_mesh = scene.mesh_mut(mesh_index);
                    Mesh::copy_mesh_features_for_material(&mesh, scene_mesh, material_index);

                    for mfi in 0..scene_mesh.num_mesh_features() {
                        let idx =
                            draco_core::mesh::mesh_indices::MeshFeaturesIndex::from(mfi as u32);
                        let mesh_features = scene_mesh.mesh_features_mut(idx);
                        if mesh_features.attribute_index() != -1 {
                            let new_index = splitter
                                .get_split_mesh_attribute_index(mesh_features.attribute_index());
                            mesh_features.set_attribute_index(new_index);
                        }
                    }
                }

                update_mesh_features_textures_on_mesh(
                    &old_texture_to_index_map,
                    scene.as_mut(),
                    mesh_index,
                );

                {
                    let scene_mesh = scene.mesh_mut(mesh_index);
                    Mesh::copy_property_attributes_indices_for_material(
                        &mesh,
                        scene_mesh,
                        material_index,
                    );

                    scene_mesh.non_material_texture_library_mut().clear();
                }
            }
        }

        scene
            .node_mut(scene_node_index)
            .set_mesh_group_index(mesh_group_index);
        scene.add_root_node_index(scene_node_index);
        StatusOr::new_value(scene)
    }

    /// Creates a mesh according to mesh instance in scene.
    pub fn instantiate_mesh(scene: &Scene, instance: &SceneMeshInstance) -> StatusOr<Box<Mesh>> {
        if scene.num_meshes() <= instance.mesh_index.value() as i32 {
            return StatusOr::new_status(Status::new(
                StatusCode::DracoError,
                "Scene has no corresponding base mesh.",
            ));
        }

        let base_mesh = scene.mesh(instance.mesh_index);
        let pos_id = base_mesh.get_named_attribute_id(GeometryAttributeType::Position);
        let pos_att = base_mesh.attribute(pos_id);
        let pos_att = match pos_att {
            Some(att) => att,
            None => {
                return StatusOr::new_status(Status::new(
                    StatusCode::DracoError,
                    "Mesh has no positions.",
                ))
            }
        };
        if pos_att.data_type() != draco_core::core::draco_types::DataType::Float32
            || pos_att.num_components() != 3
        {
            return StatusOr::new_status(Status::new(
                StatusCode::DracoError,
                "Mesh has invalid positions.",
            ));
        }

        let mut mesh = Box::new(Mesh::new());
        mesh.copy_from(base_mesh);

        if instance.transform != Matrix4d::identity() {
            MeshUtils::transform_mesh(&instance.transform, &mut mesh);
        }
        StatusOr::new_value(mesh)
    }

    /// Cleans up a scene using default options.
    pub fn cleanup(scene: &mut Scene) {
        Self::cleanup_with_options(scene, &CleanupOptions::default());
    }

    /// Cleans up a scene using provided options.
    pub fn cleanup_with_options(scene: &mut Scene, options: &CleanupOptions) {
        if options.remove_invalid_mesh_instances {
            for i in 0..scene.num_mesh_groups() {
                scene
                    .mesh_group_mut(MeshGroupIndex::from(i as u32))
                    .remove_mesh_instances(INVALID_MESH_INDEX);
            }
        }

        let mut is_mesh_group_referenced = vec![false; scene.num_mesh_groups() as usize];
        for i in 0..scene.num_nodes() {
            let node = scene.node(SceneNodeIndex::from(i as u32));
            let mesh_group_index = node.mesh_group_index();
            if mesh_group_index != INVALID_MESH_GROUP_INDEX {
                is_mesh_group_referenced[mesh_group_index.value() as usize] = true;
            }
        }

        let mut is_base_mesh_referenced = vec![false; scene.num_meshes() as usize];
        let mut is_mesh_group_empty = vec![false; scene.num_mesh_groups() as usize];
        for i in 0..scene.num_mesh_groups() {
            let mgi = MeshGroupIndex::from(i as u32);
            if !is_mesh_group_referenced[i as usize] {
                continue;
            }
            let mesh_group = scene.mesh_group(mgi);
            let mut mesh_group_is_empty = true;
            for j in 0..mesh_group.num_mesh_instances() {
                let mesh_index = mesh_group.mesh_instance(j).mesh_index;
                mesh_group_is_empty = false;
                is_base_mesh_referenced[mesh_index.value() as usize] = true;
            }
            if mesh_group_is_empty {
                is_mesh_group_empty[i as usize] = true;
            }
        }

        if options.remove_unused_meshes {
            for i in (0..scene.num_meshes()).rev() {
                let mi = MeshIndex::from(i as u32);
                if !is_base_mesh_referenced[mi.value() as usize] {
                    let _ = scene.remove_mesh(mi);
                }
            }
        }

        if options.remove_unused_mesh_groups {
            for i in (0..scene.num_mesh_groups()).rev() {
                let mgi = MeshGroupIndex::from(i as u32);
                if is_mesh_group_empty[mgi.value() as usize]
                    || !is_mesh_group_referenced[mgi.value() as usize]
                {
                    let _ = scene.remove_mesh_group(mgi);
                }
            }
        }

        let num_materials = scene.material_library().num_materials();
        let mut material_meshes: Vec<HashSet<MeshIndex>> = vec![HashSet::new(); num_materials];
        let mut mesh_materials: IndexTypeVector<MeshIndex, HashSet<i32>> =
            IndexTypeVector::with_size_value(scene.num_meshes() as usize, HashSet::new());
        let mut tex_coord_referenced: IndexTypeVector<MeshIndex, HashSet<i32>> =
            IndexTypeVector::with_size_value(scene.num_meshes() as usize, HashSet::new());

        for mgi in 0..scene.num_mesh_groups() {
            let mesh_group = scene.mesh_group(MeshGroupIndex::from(mgi as u32));
            for mi in 0..mesh_group.num_mesh_instances() {
                let mesh_index = mesh_group.mesh_instance(mi).mesh_index;
                let material_index = mesh_group.mesh_instance(mi).material_index;
                if material_index == -1 {
                    continue;
                }
                material_meshes[material_index as usize].insert(mesh_index);
                if let Some(set) = mesh_materials.get_mut(mesh_index) {
                    set.insert(material_index);
                }

                if let Some(material) = scene.material_library().material(material_index) {
                    for ti in 0..material.num_texture_maps() {
                        let texture_map = material
                            .texture_map_by_index(ti as i32)
                            .expect("Texture map missing");
                        let tex_coord_index = texture_map.tex_coord_index();
                        if let Some(set) = tex_coord_referenced.get_mut(mesh_index) {
                            set.insert(tex_coord_index);
                        }
                    }
                }
            }
        }

        if options.remove_unused_tex_coords {
            for mi in 0..scene.num_meshes() {
                let mesh_index = MeshIndex::from(mi as u32);
                let mut remove_tex_coord = true;
                if let Some(materials) = mesh_materials.get(mesh_index) {
                    for material_index in materials.iter() {
                        if material_meshes[*material_index as usize].len() != 1 {
                            remove_tex_coord = false;
                            break;
                        }
                    }
                }
                if !remove_tex_coord {
                    continue;
                }

                let tex_coord_count = scene
                    .mesh(mesh_index)
                    .num_named_attributes(GeometryAttributeType::TexCoord);
                for tci in (0..tex_coord_count).rev() {
                    if let Some(set) = tex_coord_referenced.get(mesh_index) {
                        if set.contains(&tci) {
                            continue;
                        }
                    }

                    {
                        let mesh = scene.mesh_mut(mesh_index);
                        let att_id = mesh
                            .get_named_attribute_id_by_index(GeometryAttributeType::TexCoord, tci);
                        if att_id != -1 {
                            mesh.delete_attribute(att_id);
                        }
                    }

                    let material_indices: Vec<i32> = mesh_materials
                        .get(mesh_index)
                        .map(|set| set.iter().copied().collect())
                        .unwrap_or_default();
                    for material_index in material_indices {
                        if let Some(material) = scene
                            .material_library_mut()
                            .mutable_material(material_index)
                        {
                            for ti in 0..material.num_texture_maps() {
                                if let Some(texture_map) =
                                    material.texture_map_by_index_mut(ti as i32)
                                {
                                    if texture_map.tex_coord_index() > tci {
                                        texture_map.set_properties_with_tex_coord(
                                            texture_map.map_type(),
                                            texture_map.tex_coord_index() - 1,
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        if options.remove_unused_materials {
            for i in (0..num_materials).rev() {
                if material_meshes[i].is_empty() {
                    let _ = scene.remove_material(i as i32);
                }
            }
        }

        if options.remove_unused_nodes {
            let mut remover = SceneUnusedNodeRemover::new();
            remover.remove_unused_nodes(scene);
        }
    }

    /// Removes mesh instances from scene.
    pub fn remove_mesh_instances(instances: &[SceneMeshInstance], scene: &mut Scene) {
        for instance in instances {
            let mgi = scene.node(instance.scene_node_index).mesh_group_index();
            let mesh_group_snapshot = scene.mesh_group(mgi).clone();
            let new_mesh_group_index = scene.add_mesh_group();
            let new_mesh_group = scene.mesh_group_mut(new_mesh_group_index);
            new_mesh_group.copy_from(&mesh_group_snapshot);
            new_mesh_group.remove_mesh_instances(instance.mesh_index);
            scene
                .node_mut(instance.scene_node_index)
                .set_mesh_group_index(new_mesh_group_index);
        }

        Self::deduplicate_mesh_groups(scene);
    }

    /// Removes duplicate mesh groups with identical contents.
    pub fn deduplicate_mesh_groups(scene: &mut Scene) {
        if scene.num_mesh_groups() <= 1 {
            return;
        }

        let mut parent_mesh_group: IndexTypeVector<MeshGroupIndex, MeshGroupIndex> =
            IndexTypeVector::with_size_value(
                scene.num_mesh_groups() as usize,
                INVALID_MESH_GROUP_INDEX,
            );
        let mut unique_groups: Vec<MeshGroupIndex> = Vec::new();

        for i in 0..scene.num_mesh_groups() {
            let mgi = MeshGroupIndex::from(i as u32);
            let mesh_group = scene.mesh_group(mgi);
            let mut duplicate = None;
            for &unique in &unique_groups {
                if mesh_group_equal(mesh_group, scene.mesh_group(unique)) {
                    duplicate = Some(unique);
                    break;
                }
            }
            if let Some(parent) = duplicate {
                parent_mesh_group[mgi] = parent;
            } else {
                parent_mesh_group[mgi] = INVALID_MESH_GROUP_INDEX;
                unique_groups.push(mgi);
            }
        }

        for i in 0..scene.num_nodes() {
            let sni = SceneNodeIndex::from(i as u32);
            let mgi = scene.node(sni).mesh_group_index();
            if mgi == INVALID_MESH_GROUP_INDEX {
                continue;
            }
            let parent = parent_mesh_group[mgi];
            if parent != INVALID_MESH_GROUP_INDEX {
                scene.node_mut(sni).set_mesh_group_index(parent);
            }
        }

        Self::cleanup(scene);
    }

    /// Enables geometry compression and sets compression options on all meshes.
    pub fn set_draco_compression_options(
        options: Option<
            &draco_core::compression::draco_compression_options::DracoCompressionOptions,
        >,
        scene: &mut Scene,
    ) {
        for i in 0..scene.num_meshes() {
            let mesh = scene.mesh_mut(MeshIndex::from(i as u32));
            if let Some(opts) = options {
                mesh.set_compression_enabled(true);
                mesh.set_compression_options(opts.clone());
            } else {
                mesh.set_compression_enabled(false);
            }
        }
    }

    /// Returns true if compression is enabled for any scene mesh.
    pub fn is_draco_compression_enabled(scene: &Scene) -> bool {
        for i in 0..scene.num_meshes() {
            if scene
                .mesh(MeshIndex::from(i as u32))
                .is_compression_enabled()
            {
                return true;
            }
        }
        false
    }

    /// Returns a transform for each base mesh corresponding to largest scale instance.
    pub fn find_largest_base_mesh_transforms(
        scene: &Scene,
    ) -> IndexTypeVector<MeshIndex, Matrix4d> {
        let mut transforms =
            IndexTypeVector::with_size_value(scene.num_meshes() as usize, Matrix4d::identity());
        let mut transform_scale =
            IndexTypeVector::with_size_value(scene.num_meshes() as usize, 0.0f32);

        let instances = Self::compute_all_instances(scene);
        for i in 0..instances.size() {
            let instance = &instances[MeshInstanceIndex::from(i as u32)];
            let scale_vec = [
                column_norm(&instance.transform, 0),
                column_norm(&instance.transform, 1),
                column_norm(&instance.transform, 2),
            ];
            let max_scale = scale_vec[0].max(scale_vec[1]).max(scale_vec[2]) as f32;

            if transform_scale[instance.mesh_index] < max_scale {
                transform_scale[instance.mesh_index] = max_scale;
                transforms[instance.mesh_index] = instance.transform;
            }
        }

        transforms
    }
}

struct NodeTraversal {
    scene_node_index: SceneNodeIndex,
    transform: Matrix4d,
}

fn update_mesh_features_textures_on_mesh(
    texture_to_index_map: &HashMap<*const draco_core::texture::texture::Texture, i32>,
    scene: &mut Scene,
    mesh_index: MeshIndex,
) {
    // SAFETY: Scene stores meshes and non-material texture library in distinct
    // fields. We only hold mutable references within this block and do not
    // alias them elsewhere.
    let scene_ptr = scene as *mut Scene;
    unsafe {
        let new_texture_library = (*scene_ptr).non_material_texture_library_mut();
        let mesh = (*scene_ptr).mesh_mut(mesh_index);
        for mfi in 0..mesh.num_mesh_features() {
            let idx = draco_core::mesh::mesh_indices::MeshFeaturesIndex::from(mfi as u32);
            let features = mesh.mesh_features_mut(idx);
            Mesh::update_mesh_features_texture_pointer(
                texture_to_index_map,
                new_texture_library,
                features,
            );
        }
    }
}

fn mesh_group_equal(a: &MeshGroup, b: &MeshGroup) -> bool {
    if a.name() != b.name() {
        return false;
    }
    if a.num_mesh_instances() != b.num_mesh_instances() {
        return false;
    }
    for i in 0..a.num_mesh_instances() {
        if a.mesh_instance(i) != b.mesh_instance(i) {
            return false;
        }
    }
    true
}

fn column_norm(matrix: &Matrix4d, column: usize) -> f64 {
    let x = matrix.m[0][column];
    let y = matrix.m[1][column];
    let z = matrix.m[2][column];
    (x * x + y * y + z * z).sqrt()
}

struct SceneUnusedNodeRemover {
    node_map: IndexTypeVector<SceneNodeIndex, SceneNodeIndex>,
}

impl SceneUnusedNodeRemover {
    fn new() -> Self {
        Self {
            node_map: IndexTypeVector::new(),
        }
    }

    fn remove_unused_nodes(&mut self, scene: &mut Scene) {
        let num_unused = self.find_unused_nodes(scene);
        if num_unused == 0 {
            return;
        }
        self.update_node_indices(scene);
        self.remove_unused_nodes_from_scene(scene);
    }

    fn find_unused_nodes(&mut self, scene: &Scene) -> i32 {
        self.node_map =
            IndexTypeVector::with_size_value(scene.num_nodes() as usize, INVALID_SCENE_NODE_INDEX);
        for sni in 0..scene.num_nodes() {
            let index = SceneNodeIndex::from(sni as u32);
            if scene.node(index).mesh_group_index() != INVALID_MESH_GROUP_INDEX {
                self.node_map[index] = index;
            }
        }

        for i in 0..scene.num_animations() {
            let animation =
                scene.animation(crate::scene::scene_indices::AnimationIndex::from(i as u32));
            for channel_i in 0..animation.num_channels() {
                let channel = animation
                    .channel(channel_i)
                    .expect("Missing animation channel");
                let node_index = SceneNodeIndex::from(channel.target_index as u32);
                self.node_map[node_index] = node_index;
            }
        }

        for i in 0..scene.num_skins() {
            let skin = scene.skin(crate::scene::scene_indices::SkinIndex::from(i as u32));
            for j in 0..skin.num_joints() {
                let node_index = skin.joint(j);
                self.node_map[node_index] = node_index;
            }
            let root_index = skin.joint_root();
            if root_index != INVALID_SCENE_NODE_INDEX {
                self.node_map[root_index] = root_index;
            }
        }

        for r in 0..scene.num_root_nodes() {
            self.update_used_nodes_from_scene_graph(scene, scene.root_node_index(r));
        }

        let mut num_valid_nodes = 0;
        for sni in 0..scene.num_nodes() {
            let index = SceneNodeIndex::from(sni as u32);
            if self.node_map[index] != INVALID_SCENE_NODE_INDEX {
                self.node_map[index] = SceneNodeIndex::from(num_valid_nodes as u32);
                num_valid_nodes += 1;
            }
        }

        scene.num_nodes() - num_valid_nodes
    }

    fn update_used_nodes_from_scene_graph(&mut self, scene: &Scene, sni: SceneNodeIndex) -> bool {
        let node = scene.node(sni);
        let mut any_child_used = false;
        for c in 0..node.num_children() {
            let cni = node.child(c);
            if self.update_used_nodes_from_scene_graph(scene, cni) {
                any_child_used = true;
            }
        }
        if any_child_used {
            self.node_map[sni] = sni;
        }
        self.node_map[sni] != INVALID_SCENE_NODE_INDEX
    }

    fn update_node_indices(&self, scene: &mut Scene) {
        let mut indices: Vec<SceneNodeIndex>;
        for sni in 0..scene.num_nodes() {
            let idx = SceneNodeIndex::from(sni as u32);
            indices = scene.node(idx).children().clone();
            scene.node_mut(idx).remove_all_children();
            for child in indices.drain(..) {
                let new_sni = self.node_map[child];
                if new_sni != INVALID_SCENE_NODE_INDEX {
                    scene.node_mut(idx).add_child_index(new_sni);
                }
            }

            indices = scene.node(idx).parents().clone();
            scene.node_mut(idx).remove_all_parents();
            for parent in indices.drain(..) {
                let new_sni = self.node_map[parent];
                if new_sni != INVALID_SCENE_NODE_INDEX {
                    scene.node_mut(idx).add_parent_index(new_sni);
                }
            }
        }

        indices = scene.root_node_indices().clone();
        scene.remove_all_root_node_indices();
        for root in indices {
            let new_rni = self.node_map[root];
            if new_rni != INVALID_SCENE_NODE_INDEX {
                scene.add_root_node_index(new_rni);
            }
        }

        for i in 0..scene.num_animations() {
            let animation =
                scene.animation_mut(crate::scene::scene_indices::AnimationIndex::from(i as u32));
            for channel_i in 0..animation.num_channels() {
                let channel = animation
                    .channel_mut(channel_i)
                    .expect("Missing animation channel");
                let node_index = SceneNodeIndex::from(channel.target_index as u32);
                channel.target_index = self.node_map[node_index].value() as i32;
            }
        }

        for i in 0..scene.num_skins() {
            let skin = scene.skin_mut(crate::scene::scene_indices::SkinIndex::from(i as u32));
            for j in 0..skin.num_joints() {
                let node_index = skin.joint(j);
                *skin.joint_mut(j) = self.node_map[node_index];
            }
            let root_index = skin.joint_root();
            if root_index != INVALID_SCENE_NODE_INDEX {
                skin.set_joint_root(self.node_map[root_index]);
            }
        }
    }

    fn remove_unused_nodes_from_scene(&self, scene: &mut Scene) {
        let mut num_valid_nodes = 0;
        for sni in 0..scene.num_nodes() {
            let index = SceneNodeIndex::from(sni as u32);
            let new_sni = self.node_map[index];
            if new_sni == INVALID_SCENE_NODE_INDEX {
                continue;
            }
            num_valid_nodes += 1;
            if index != new_sni {
                let node = scene.node(index).clone();
                scene.node_mut(new_sni).copy_from(&node);
            }
        }
        scene.resize_nodes(num_valid_nodes);
    }
}
