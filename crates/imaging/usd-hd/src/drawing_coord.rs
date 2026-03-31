
//! HdDrawingCoord - Indirection mapping from conceptual resources to BAR indices.
//!
//! Corresponds to pxr/imaging/hd/drawingCoord.h.
//! Maps topology, primvars, instancing slots to indices in HdBufferArrayRangeContainer.

/// Unassigned slot value.
pub const HD_DRAWING_COORD_UNASSIGNED: i32 = -1;

/// Index where custom slots begin.
pub const HD_DRAWING_COORD_CUSTOM_SLOTS_BEGIN: i32 = 8;

/// Default number of slots (constant, vertex, topology).
pub const HD_DRAWING_COORD_DEFAULT_NUM_SLOTS: i32 = 3;

/// Drawing coordinate mapping from conceptual slots to BAR container indices.
///
/// Corresponds to C++ `HdDrawingCoord`.
#[derive(Debug, Clone, Default)]
pub struct HdDrawingCoord {
    topology: i16,
    instance_primvar: i16,
    constant_primvar: i8,
    vertex_primvar: i8,
    element_primvar: i8,
    instance_index: i8,
    face_varying_primvar: i8,
    topology_visibility: i8,
    varying_primvar: i8,
}

impl HdDrawingCoord {
    /// Create default drawing coord with standard slot layout.
    pub fn new() -> Self {
        Self {
            topology: 2,
            instance_primvar: HD_DRAWING_COORD_UNASSIGNED as i16,
            constant_primvar: 0,
            vertex_primvar: 1,
            element_primvar: 3,
            instance_index: 4,
            face_varying_primvar: 5,
            topology_visibility: 6,
            varying_primvar: 7,
        }
    }

    /// BAR index for constant (uniform per-prim) primvar.
    pub fn get_constant_primvar_index(&self) -> i32 {
        self.constant_primvar as i32
    }
    /// Set BAR index for constant primvar slot.
    pub fn set_constant_primvar_index(&mut self, slot: i32) {
        self.constant_primvar = slot as i8;
    }
    /// BAR index for vertex primvar.
    pub fn get_vertex_primvar_index(&self) -> i32 {
        self.vertex_primvar as i32
    }
    /// Set BAR index for vertex primvar slot.
    pub fn set_vertex_primvar_index(&mut self, slot: i32) {
        self.vertex_primvar = slot as i8;
    }
    /// BAR index for topology data.
    pub fn get_topology_index(&self) -> i32 {
        self.topology as i32
    }
    /// Set BAR index for topology slot.
    pub fn set_topology_index(&mut self, slot: i32) {
        self.topology = slot as i16;
    }
    /// BAR index for element (per-face) primvar.
    pub fn get_element_primvar_index(&self) -> i32 {
        self.element_primvar as i32
    }
    /// Set BAR index for element primvar slot.
    pub fn set_element_primvar_index(&mut self, slot: i32) {
        self.element_primvar = slot as i8;
    }
    /// BAR index for instance index data.
    pub fn get_instance_index_index(&self) -> i32 {
        self.instance_index as i32
    }
    /// Set BAR index for instance index slot.
    pub fn set_instance_index_index(&mut self, slot: i32) {
        self.instance_index = slot as i8;
    }
    /// BAR index for face-varying primvar.
    pub fn get_face_varying_primvar_index(&self) -> i32 {
        self.face_varying_primvar as i32
    }
    /// Set BAR index for face-varying primvar slot.
    pub fn set_face_varying_primvar_index(&mut self, slot: i32) {
        self.face_varying_primvar = slot as i8;
    }
    /// BAR index for topology visibility data.
    pub fn get_topology_visibility_index(&self) -> i32 {
        self.topology_visibility as i32
    }
    /// Set BAR index for topology visibility slot.
    pub fn set_topology_visibility_index(&mut self, slot: i32) {
        self.topology_visibility = slot as i8;
    }
    /// BAR index for varying primvar.
    pub fn get_varying_primvar_index(&self) -> i32 {
        self.varying_primvar as i32
    }
    /// Set BAR index for varying primvar slot.
    pub fn set_varying_primvar_index(&mut self, slot: i32) {
        self.varying_primvar = slot as i8;
    }

    /// Set base BAR index for instance primvar levels.
    pub fn set_instance_primvar_base_index(&mut self, slot: i32) {
        self.instance_primvar = slot as i16;
    }
    /// BAR index for instance primvar at the given nesting level.
    pub fn get_instance_primvar_index(&self, level: i32) -> i32 {
        debug_assert!(self.instance_primvar != HD_DRAWING_COORD_UNASSIGNED as i16);
        self.instance_primvar as i32 + level
    }
}
