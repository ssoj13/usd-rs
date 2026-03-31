//! Mesh group.
//!
//! What: Groups mesh instances with material bindings.
//! Why: Matches glTF mesh groups and KHR_materials_variants.
//! How: Stores instances and optional variants mappings.
//! Where used: Scene graphs and glTF IO.

use crate::scene::scene_indices::MeshIndex;

/// Mapping from material index to materials variants.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MaterialsVariantsMapping {
    pub material: i32,
    pub variants: Vec<i32>,
}

impl MaterialsVariantsMapping {
    pub fn new(material: i32, variants: &[i32]) -> Self {
        Self {
            material,
            variants: variants.to_vec(),
        }
    }
}

/// Mesh instance descriptor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MeshInstance {
    pub mesh_index: MeshIndex,
    pub material_index: i32,
    pub materials_variants_mappings: Vec<MaterialsVariantsMapping>,
}

impl MeshInstance {
    pub fn new(mesh_index: MeshIndex, material_index: i32) -> Self {
        Self {
            mesh_index,
            material_index,
            materials_variants_mappings: Vec::new(),
        }
    }

    pub fn with_variants(
        mesh_index: MeshIndex,
        material_index: i32,
        mappings: &[MaterialsVariantsMapping],
    ) -> Self {
        Self {
            mesh_index,
            material_index,
            materials_variants_mappings: mappings.to_vec(),
        }
    }
}

/// Mesh group containing ordered mesh instances.
#[derive(Clone, Debug, Default)]
pub struct MeshGroup {
    name: String,
    mesh_instances: Vec<MeshInstance>,
}

impl MeshGroup {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn copy_from(&mut self, src: &MeshGroup) {
        self.name = src.name.clone();
        self.mesh_instances = src.mesh_instances.clone();
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn set_name(&mut self, name: &str) {
        self.name = name.to_string();
    }

    pub fn add_mesh_instance(&mut self, instance: MeshInstance) {
        self.mesh_instances.push(instance);
    }

    pub fn set_mesh_instance(&mut self, index: i32, instance: MeshInstance) {
        self.mesh_instances[index as usize] = instance;
    }

    pub fn mesh_instance(&self, index: i32) -> &MeshInstance {
        &self.mesh_instances[index as usize]
    }

    pub fn mesh_instance_mut(&mut self, index: i32) -> &mut MeshInstance {
        &mut self.mesh_instances[index as usize]
    }

    pub fn num_mesh_instances(&self) -> i32 {
        self.mesh_instances.len() as i32
    }

    pub fn remove_mesh_instances(&mut self, mesh_index: MeshIndex) {
        let mut i = 0;
        while i < self.mesh_instances.len() {
            if self.mesh_instances[i].mesh_index == mesh_index {
                self.mesh_instances.remove(i);
            } else {
                i += 1;
            }
        }
    }
}
