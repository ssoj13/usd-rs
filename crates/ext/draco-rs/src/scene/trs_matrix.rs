//! TRS matrix storage.
//!
//! What: Stores translation/rotation/scale vectors or a 4x4 matrix.
//! Why: glTF nodes store either TRS or matrix; we need parity with Draco.
//! How: Keeps flags for which components are set and can compute a matrix.
//! Where used: Scene nodes, GPU instancing, glTF IO.

use draco_core::core::status::{Status, StatusCode};
use draco_core::core::status_or::StatusOr;
use draco_core::mesh::mesh_utils::{Matrix3d, Matrix4d};

/// 3D vector (f64) used for TRS fields.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vector3d {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Vector3d {
    pub fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }
}

/// Quaternion (w, x, y, z).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quaterniond {
    pub w: f64,
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Quaterniond {
    pub fn new(w: f64, x: f64, y: f64, z: f64) -> Self {
        Self { w, x, y, z }
    }

    pub fn normalized(self) -> Self {
        let len = (self.w * self.w + self.x * self.x + self.y * self.y + self.z * self.z).sqrt();
        if len == 0.0 {
            return Self::new(1.0, 0.0, 0.0, 0.0);
        }
        Self::new(self.w / len, self.x / len, self.y / len, self.z / len)
    }

    pub fn to_rotation_matrix(self) -> Matrix3d {
        let q = self.normalized();
        let (w, x, y, z) = (q.w, q.x, q.y, q.z);
        let xx = x * x;
        let yy = y * y;
        let zz = z * z;
        let xy = x * y;
        let xz = x * z;
        let yz = y * z;
        let wx = w * x;
        let wy = w * y;
        let wz = w * z;

        Matrix3d {
            m: [
                [1.0 - 2.0 * (yy + zz), 2.0 * (xy - wz), 2.0 * (xz + wy)],
                [2.0 * (xy + wz), 1.0 - 2.0 * (xx + zz), 2.0 * (yz - wx)],
                [2.0 * (xz - wy), 2.0 * (yz + wx), 1.0 - 2.0 * (xx + yy)],
            ],
        }
    }
}

/// TRS matrix container.
#[derive(Clone, Debug)]
pub struct TrsMatrix {
    matrix: Matrix4d,
    translation: Vector3d,
    rotation: Quaterniond,
    scale: Vector3d,
    matrix_set: bool,
    translation_set: bool,
    rotation_set: bool,
    scale_set: bool,
}

impl Default for TrsMatrix {
    fn default() -> Self {
        Self {
            matrix: Matrix4d::identity(),
            translation: Vector3d::new(0.0, 0.0, 0.0),
            rotation: Quaterniond::new(1.0, 0.0, 0.0, 0.0),
            scale: Vector3d::new(1.0, 1.0, 1.0),
            matrix_set: false,
            translation_set: false,
            rotation_set: false,
            scale_set: false,
        }
    }
}

impl TrsMatrix {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn copy_from(&mut self, src: &TrsMatrix) {
        self.matrix = src.matrix;
        self.translation = src.translation;
        self.rotation = src.rotation;
        self.scale = src.scale;
        self.matrix_set = src.matrix_set;
        self.translation_set = src.translation_set;
        self.rotation_set = src.rotation_set;
        self.scale_set = src.scale_set;
    }

    pub fn set_matrix(&mut self, matrix: Matrix4d) -> &mut Self {
        self.matrix = matrix;
        self.matrix_set = true;
        self
    }

    pub fn matrix_set(&self) -> bool {
        self.matrix_set
    }

    pub fn matrix(&self) -> StatusOr<Matrix4d> {
        if !self.matrix_set {
            return StatusOr::new_status(Status::new(StatusCode::DracoError, "Matrix is not set."));
        }
        StatusOr::new_value(self.matrix)
    }

    pub fn set_translation(&mut self, translation: Vector3d) -> &mut Self {
        self.translation = translation;
        self.translation_set = true;
        self
    }

    pub fn translation_set(&self) -> bool {
        self.translation_set
    }

    pub fn translation(&self) -> StatusOr<Vector3d> {
        if !self.translation_set {
            return StatusOr::new_status(Status::new(
                StatusCode::DracoError,
                "Translation is not set.",
            ));
        }
        StatusOr::new_value(self.translation)
    }

    pub fn set_rotation(&mut self, rotation: Quaterniond) -> &mut Self {
        self.rotation = rotation;
        self.rotation_set = true;
        self
    }

    pub fn rotation_set(&self) -> bool {
        self.rotation_set
    }

    pub fn rotation(&self) -> StatusOr<Quaterniond> {
        if !self.rotation_set {
            return StatusOr::new_status(Status::new(
                StatusCode::DracoError,
                "Rotation is not set.",
            ));
        }
        StatusOr::new_value(self.rotation)
    }

    pub fn set_scale(&mut self, scale: Vector3d) -> &mut Self {
        self.scale = scale;
        self.scale_set = true;
        self
    }

    pub fn scale_set(&self) -> bool {
        self.scale_set
    }

    pub fn scale(&self) -> StatusOr<Vector3d> {
        if !self.scale_set {
            return StatusOr::new_status(Status::new(StatusCode::DracoError, "Scale is not set."));
        }
        StatusOr::new_value(self.scale)
    }

    pub fn is_matrix_identity(&self) -> bool {
        if !self.matrix_set {
            return true;
        }
        self.matrix == Matrix4d::identity()
    }

    pub fn is_matrix_translation_only(&self) -> bool {
        if !self.matrix_set {
            return false;
        }
        let mut translation_check = self.matrix;
        translation_check.m[0][3] = 0.0;
        translation_check.m[1][3] = 0.0;
        translation_check.m[2][3] = 0.0;
        translation_check == Matrix4d::identity()
    }

    pub fn compute_transformation_matrix(&self) -> Matrix4d {
        if self.matrix_set {
            return self.matrix;
        }

        let mut translation_matrix = Matrix4d::identity();
        translation_matrix.m[0][3] = self.translation.x;
        translation_matrix.m[1][3] = self.translation.y;
        translation_matrix.m[2][3] = self.translation.z;

        let rotation_matrix_3 = self.rotation.to_rotation_matrix();
        let mut rotation_matrix = Matrix4d::identity();
        rotation_matrix.set_block_3x3(rotation_matrix_3);

        let mut scale_matrix = Matrix4d::identity();
        scale_matrix.m[0][0] = self.scale.x;
        scale_matrix.m[1][1] = self.scale.y;
        scale_matrix.m[2][2] = self.scale.z;

        translation_matrix.mul(&rotation_matrix).mul(&scale_matrix)
    }

    pub fn transform_set(&self) -> bool {
        self.matrix_set || self.translation_set || self.rotation_set || self.scale_set
    }
}

impl PartialEq for TrsMatrix {
    fn eq(&self, other: &Self) -> bool {
        if self.matrix_set != other.matrix_set
            || self.translation_set != other.translation_set
            || self.rotation_set != other.rotation_set
            || self.scale_set != other.scale_set
        {
            return false;
        }
        if self.matrix_set && self.matrix != other.matrix {
            return false;
        }
        if self.translation_set && self.translation != other.translation {
            return false;
        }
        if self.rotation_set && self.rotation != other.rotation {
            return false;
        }
        // Fixed: C++ reference omits scale comparison (bug in operator==)
        if self.scale_set && self.scale != other.scale {
            return false;
        }
        true
    }
}
