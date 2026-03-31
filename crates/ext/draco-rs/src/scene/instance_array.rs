//! Instance array (EXT_mesh_gpu_instancing).
//!
//! What: Stores per-instance TRS data for mesh groups.
//! Why: glTF GPU instancing uses this data for repeated meshes.
//! How: Holds instances with TRS only (no matrix allowed).
//! Where used: Scene graphs and glTF IO.

use draco_core::core::status::{ok_status, Status, StatusCode};

use crate::scene::trs_matrix::TrsMatrix;

/// Instance attributes for a mesh group.
#[derive(Clone, Debug, Default)]
pub struct Instance {
    pub trs: TrsMatrix,
}

/// Instance array for GPU instancing.
#[derive(Clone, Debug, Default)]
pub struct InstanceArray {
    instances: Vec<Instance>,
}

impl InstanceArray {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn copy_from(&mut self, src: &InstanceArray) {
        self.instances.clear();
        self.instances
            .resize(src.instances.len(), Instance::default());
        for (i, inst) in src.instances.iter().enumerate() {
            self.instances[i].trs.copy_from(&inst.trs);
        }
    }

    pub fn add_instance(&mut self, instance: &Instance) -> Status {
        if instance.trs.matrix_set() {
            return Status::new(StatusCode::DracoError, "Instance must have no matrix set.");
        }
        self.instances.push(instance.clone());
        ok_status()
    }

    pub fn num_instances(&self) -> i32 {
        self.instances.len() as i32
    }

    pub fn instance(&self, index: i32) -> &Instance {
        &self.instances[index as usize]
    }
}
