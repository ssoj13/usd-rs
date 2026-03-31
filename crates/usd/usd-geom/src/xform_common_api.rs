//! UsdGeomXformCommonAPI - common transform API schema.
//!
//! Port of pxr/usd/usdGeom/xformCommonAPI.h/cpp
//!
//! API schema for authoring and retrieving a standard set of component transformations.

use super::xform_op::{XformOp, XformOpType};
use super::xformable::Xformable;
use usd_core::Prim;
use usd_core::schema_base::APISchemaBase;
use usd_gf::{Matrix3d, Matrix4d, Vec3d, Vec3f};
use usd_sdf::TimeCode;
use usd_tf::Token;

// ============================================================================
// RotationOrder
// ============================================================================

/// Enumerates the rotation order of the 3-angle Euler rotation.
///
/// Matches C++ `UsdGeomXformCommonAPI::RotationOrder`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RotationOrder {
    /// XYZ rotation order.
    XYZ,
    /// XZY rotation order.
    XZY,
    /// YXZ rotation order.
    YXZ,
    /// YZX rotation order.
    YZX,
    /// ZXY rotation order.
    ZXY,
    /// ZYX rotation order.
    ZYX,
}

impl std::fmt::Display for RotationOrder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RotationOrder::XYZ => write!(f, "XYZ"),
            RotationOrder::XZY => write!(f, "XZY"),
            RotationOrder::YXZ => write!(f, "YXZ"),
            RotationOrder::YZX => write!(f, "YZX"),
            RotationOrder::ZXY => write!(f, "ZXY"),
            RotationOrder::ZYX => write!(f, "ZYX"),
        }
    }
}

impl Default for RotationOrder {
    fn default() -> Self {
        RotationOrder::XYZ
    }
}

// ============================================================================
// OpFlags
// ============================================================================

/// Enumerates the categories of ops that can be handled by XformCommonAPI.
///
/// Matches C++ `UsdGeomXformCommonAPI::OpFlags`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpFlags(u8);

impl OpFlags {
    /// No operations.
    pub const NONE: OpFlags = OpFlags(0);
    /// Translation operation.
    pub const TRANSLATE: OpFlags = OpFlags(1);
    /// Pivot operation.
    pub const PIVOT: OpFlags = OpFlags(2);
    /// Rotation operation.
    pub const ROTATE: OpFlags = OpFlags(4);
    /// Scale operation.
    pub const SCALE: OpFlags = OpFlags(8);
}

impl std::ops::BitOr for OpFlags {
    type Output = Self;
    fn bitor(self, other: Self) -> Self {
        OpFlags(self.0 | other.0)
    }
}

impl std::ops::BitAnd for OpFlags {
    type Output = Self;
    fn bitand(self, other: Self) -> Self {
        OpFlags(self.0 & other.0)
    }
}

impl OpFlags {
    /// Returns true if this flags value contains the specified flags.
    pub fn contains(self, other: OpFlags) -> bool {
        (self.0 & other.0) != 0
    }
}

// ============================================================================
// Ops
// ============================================================================

/// Return type for CreateXformOps().
///
/// Stores the op of each type that is present on the prim.
///
/// Matches C++ `UsdGeomXformCommonAPI::Ops`.
#[derive(Debug, Clone)]
pub struct Ops {
    /// Translate operation.
    pub translate_op: XformOp,
    /// Pivot operation.
    pub pivot_op: XformOp,
    /// Rotate operation.
    pub rotate_op: XformOp,
    /// Scale operation.
    pub scale_op: XformOp,
    /// Inverse pivot operation.
    pub inverse_pivot_op: XformOp,
}

impl Ops {
    /// Creates an Ops struct with all invalid ops.
    pub fn invalid() -> Self {
        Self {
            translate_op: XformOp::invalid(),
            pivot_op: XformOp::invalid(),
            rotate_op: XformOp::invalid(),
            scale_op: XformOp::invalid(),
            inverse_pivot_op: XformOp::invalid(),
        }
    }
}

// ============================================================================
// XformCommonAPI
// ============================================================================

/// API schema for authoring and retrieving a standard set of component transformations.
///
/// Matches C++ `UsdGeomXformCommonAPI`.
#[derive(Debug, Clone)]
pub struct XformCommonAPI {
    /// Base API schema.
    base: APISchemaBase,
}

impl XformCommonAPI {
    /// Constructs a XformCommonAPI from a prim.
    ///
    /// Matches C++ `UsdGeomXformCommonAPI(UsdPrim)`.
    pub fn new(prim: Prim) -> Self {
        Self {
            base: APISchemaBase::new(prim),
        }
    }

    /// Constructs an invalid XformCommonAPI.
    ///
    /// Matches C++ default constructor.
    pub fn invalid() -> Self {
        Self {
            base: APISchemaBase::invalid(),
        }
    }

    /// Returns true if this schema is valid.
    pub fn is_valid(&self) -> bool {
        self.base.is_valid()
    }

    /// Returns the wrapped prim.
    pub fn prim(&self) -> &Prim {
        self.base.prim()
    }

    /// Returns the schema type name.
    pub fn schema_type_name() -> Token {
        Token::new("XformCommonAPI")
    }

    /// Gets a XformCommonAPI from a prim.
    ///
    /// Matches C++ `Get(UsdStagePtr, SdfPath)`.
    pub fn get(prim: &Prim) -> Self {
        Self::new(prim.clone())
    }

    /// Converts a RotationOrder to XformOpType.
    ///
    /// Matches C++ `ConvertRotationOrderToOpType()`.
    pub fn convert_rotation_order_to_op_type(rot_order: RotationOrder) -> XformOpType {
        match rot_order {
            RotationOrder::XYZ => XformOpType::RotateXYZ,
            RotationOrder::XZY => XformOpType::RotateXZY,
            RotationOrder::YXZ => XformOpType::RotateYXZ,
            RotationOrder::YZX => XformOpType::RotateYZX,
            RotationOrder::ZXY => XformOpType::RotateZXY,
            RotationOrder::ZYX => XformOpType::RotateZYX,
        }
    }

    /// Converts an XformOpType to RotationOrder.
    ///
    /// Matches C++ `ConvertOpTypeToRotationOrder()`.
    pub fn convert_op_type_to_rotation_order(op_type: XformOpType) -> Option<RotationOrder> {
        match op_type {
            XformOpType::RotateXYZ => Some(RotationOrder::XYZ),
            XformOpType::RotateXZY => Some(RotationOrder::XZY),
            XformOpType::RotateYXZ => Some(RotationOrder::YXZ),
            XformOpType::RotateYZX => Some(RotationOrder::YZX),
            XformOpType::RotateZXY => Some(RotationOrder::ZXY),
            XformOpType::RotateZYX => Some(RotationOrder::ZYX),
            _ => None,
        }
    }

    /// Returns whether the given op type is a three-axis rotation
    /// that can be converted to a RotationOrder.
    ///
    /// Matches C++ `CanConvertOpTypeToRotationOrder()`.
    pub fn can_convert_op_type_to_rotation_order(op_type: XformOpType) -> bool {
        matches!(
            op_type,
            XformOpType::RotateXYZ
                | XformOpType::RotateXZY
                | XformOpType::RotateYXZ
                | XformOpType::RotateYZX
                | XformOpType::RotateZXY
                | XformOpType::RotateZYX
        )
    }

    /// Returns whether the xformable resets the transform stack.
    ///
    /// Matches C++ `GetResetXformStack()`.
    pub fn get_reset_xform_stack(&self) -> bool {
        let xformable = Xformable::new(self.prim().clone());
        if !xformable.is_valid() {
            return false;
        }
        let mut resets = false;
        let _ = xformable.get_ordered_xform_ops_with_reset(&mut resets);
        resets
    }

    /// Sets whether the xformable resets the transform stack.
    ///
    /// Matches C++ `SetResetXformStack()`.
    pub fn set_reset_xform_stack(&self, reset_xform_stack: bool) -> bool {
        let xformable = Xformable::new(self.prim().clone());
        if !xformable.is_valid() {
            return false;
        }
        let ops = xformable.get_ordered_xform_ops();
        xformable.set_xform_op_order_with_reset(&ops, reset_xform_stack)
    }

    /// Creates the specified XformCommonAPI-compatible xform ops.
    ///
    /// Matches C++ `CreateXformOps()`.
    pub fn create_xform_ops(
        &self,
        rot_order: RotationOrder,
        op1: OpFlags,
        op2: OpFlags,
        op3: OpFlags,
        op4: OpFlags,
    ) -> Ops {
        let xformable = Xformable::new(self.prim().clone());
        if !xformable.is_valid() {
            return Ops::invalid();
        }

        let flags = op1 | op2 | op3 | op4;
        let create_translate = flags.contains(OpFlags::TRANSLATE);
        let create_pivot = flags.contains(OpFlags::PIVOT);
        let create_rotate = flags.contains(OpFlags::ROTATE);
        let create_scale = flags.contains(OpFlags::SCALE);

        // Get existing ops
        let mut resets = false;
        let existing_ops = xformable.get_ordered_xform_ops_with_reset(&mut resets);

        // Find existing ops and detect incompatibilities
        let mut translate_op = XformOp::invalid();
        let mut pivot_op = XformOp::invalid();
        let mut rotate_op = XformOp::invalid();
        let mut scale_op = XformOp::invalid();
        let mut inverse_pivot_op = XformOp::invalid();
        let mut has_incompatible = false;

        let translate_token = XformOp::get_op_name(XformOpType::Translate, None, false);
        let pivot_token = Token::new("pivot");
        let scale_token = XformOp::get_op_name(XformOpType::Scale, None, false);
        let _requested_rotate_type = Self::convert_rotation_order_to_op_type(rot_order);

        for op in &existing_ops {
            let op_name = op.attr().name();
            let op_name_str = op_name.as_str();
            let translate_token_str = translate_token.as_str();
            let scale_token_str = scale_token.as_str();

            match op.op_type() {
                XformOpType::Translate => {
                    if !op.is_inverse_op() {
                        if op.has_suffix(&pivot_token) {
                            if !pivot_op.is_valid() {
                                pivot_op = op.clone();
                            }
                        } else if op_name_str == translate_token_str {
                            translate_op = op.clone();
                        }
                    } else if op.has_suffix(&pivot_token) {
                        inverse_pivot_op = op.clone();
                    }
                }
                XformOpType::Scale => {
                    if !op.is_inverse_op() && op_name_str == scale_token_str {
                        scale_op = op.clone();
                    }
                }
                XformOpType::RotateXYZ
                | XformOpType::RotateXZY
                | XformOpType::RotateYXZ
                | XformOpType::RotateYZX
                | XformOpType::RotateZXY
                | XformOpType::RotateZYX => {
                    if !op.is_inverse_op() {
                        rotate_op = op.clone();
                    }
                }
                // Transform, Orient, single-axis rotates are incompatible
                XformOpType::Transform
                | XformOpType::Orient
                | XformOpType::RotateX
                | XformOpType::RotateY
                | XformOpType::RotateZ => {
                    has_incompatible = true;
                }
                _ => {}
            }
        }

        if has_incompatible {
            return Ops::invalid();
        }

        // Create missing ops
        let mut new_ops = Vec::new();
        let mut added_ops = false;

        if create_translate && !translate_op.is_valid() {
            translate_op =
                xformable.add_translate_op(super::xform_op::XformOpPrecision::Double, None, false);
            added_ops = true;
        }
        if translate_op.is_valid() {
            new_ops.push(translate_op.clone());
        }

        if create_pivot && !pivot_op.is_valid() {
            let pivot_token = Token::new("pivot");
            pivot_op = xformable.add_xform_op(
                XformOpType::Translate,
                super::xform_op::XformOpPrecision::Float,
                Some(&pivot_token),
                false,
            );
            inverse_pivot_op = xformable.add_xform_op(
                XformOpType::Translate,
                super::xform_op::XformOpPrecision::Float,
                Some(&pivot_token),
                true,
            );
            added_ops = true;
        }
        if pivot_op.is_valid() {
            new_ops.push(pivot_op.clone());
        }

        if create_rotate && !rotate_op.is_valid() {
            let rotate_op_type = Self::convert_rotation_order_to_op_type(rot_order);
            rotate_op = xformable.add_xform_op(
                rotate_op_type,
                super::xform_op::XformOpPrecision::Float,
                None,
                false,
            );
            added_ops = true;
        }
        if rotate_op.is_valid() {
            new_ops.push(rotate_op.clone());
        }

        if create_scale && !scale_op.is_valid() {
            scale_op =
                xformable.add_scale_op(super::xform_op::XformOpPrecision::Float, None, false);
            added_ops = true;
        }
        if scale_op.is_valid() {
            new_ops.push(scale_op.clone());
        }

        if inverse_pivot_op.is_valid() {
            new_ops.push(inverse_pivot_op.clone());
        }

        // Update xform op order if we added ops
        if added_ops {
            let _ = xformable.set_xform_op_order_with_reset(&new_ops, resets);
        }

        Ops {
            translate_op,
            pivot_op,
            rotate_op,
            scale_op,
            inverse_pivot_op,
        }
    }

    /// Set translation at time.
    ///
    /// Matches C++ `SetTranslate()`.
    pub fn set_translate(&self, translation: Vec3d, time: TimeCode) -> bool {
        let ops = self.create_xform_ops(
            RotationOrder::XYZ,
            OpFlags::TRANSLATE,
            OpFlags::NONE,
            OpFlags::NONE,
            OpFlags::NONE,
        );
        if !ops.translate_op.is_valid() {
            return false;
        }
        ops.translate_op
            .set(usd_vt::Value::from_no_hash(translation), time)
    }

    /// Set pivot position at time.
    ///
    /// Matches C++ `SetPivot()`.
    pub fn set_pivot(&self, pivot: Vec3f, time: TimeCode) -> bool {
        let ops = self.create_xform_ops(
            RotationOrder::XYZ,
            OpFlags::PIVOT,
            OpFlags::NONE,
            OpFlags::NONE,
            OpFlags::NONE,
        );
        if !ops.pivot_op.is_valid() {
            return false;
        }
        ops.pivot_op.set(usd_vt::Value::from_no_hash(pivot), time)
    }

    /// Set rotation at time.
    ///
    /// Matches C++ `SetRotate()`.
    pub fn set_rotate(&self, rotation: Vec3f, rot_order: RotationOrder, time: TimeCode) -> bool {
        let ops = self.create_xform_ops(
            rot_order,
            OpFlags::ROTATE,
            OpFlags::NONE,
            OpFlags::NONE,
            OpFlags::NONE,
        );
        if !ops.rotate_op.is_valid() {
            return false;
        }
        // Verify rotation order matches the existing rotate op
        let expected_type = Self::convert_rotation_order_to_op_type(rot_order);
        if ops.rotate_op.op_type() != expected_type {
            return false;
        }
        ops.rotate_op
            .set(usd_vt::Value::from_no_hash(rotation), time)
    }

    /// Set scale at time.
    ///
    /// Matches C++ `SetScale()`.
    pub fn set_scale(&self, scale: Vec3f, time: TimeCode) -> bool {
        let ops = self.create_xform_ops(
            RotationOrder::XYZ,
            OpFlags::SCALE,
            OpFlags::NONE,
            OpFlags::NONE,
            OpFlags::NONE,
        );
        if !ops.scale_op.is_valid() {
            return false;
        }
        ops.scale_op.set(usd_vt::Value::from_no_hash(scale), time)
    }

    /// Set values for the various component xformOps at a given time.
    ///
    /// Matches C++ `SetXformVectors()`.
    pub fn set_xform_vectors(
        &self,
        translation: Vec3d,
        rotation: Vec3f,
        scale: Vec3f,
        pivot: Vec3f,
        rot_order: RotationOrder,
        time: TimeCode,
    ) -> bool {
        let ops = self.create_xform_ops(
            rot_order,
            OpFlags::TRANSLATE | OpFlags::ROTATE | OpFlags::SCALE | OpFlags::PIVOT,
            OpFlags::NONE,
            OpFlags::NONE,
            OpFlags::NONE,
        );

        if !ops.translate_op.is_valid()
            || !ops.rotate_op.is_valid()
            || !ops.scale_op.is_valid()
            || !ops.pivot_op.is_valid()
        {
            return false;
        }

        // Verify rotation order matches the existing rotate op
        let expected_type = Self::convert_rotation_order_to_op_type(rot_order);
        if ops.rotate_op.op_type() != expected_type {
            return false;
        }

        ops.translate_op
            .set(usd_vt::Value::from_no_hash(translation), time)
            && ops
                .rotate_op
                .set(usd_vt::Value::from_no_hash(rotation), time)
            && ops.scale_op.set(usd_vt::Value::from_no_hash(scale), time)
            && ops.pivot_op.set(usd_vt::Value::from_no_hash(pivot), time)
    }

    /// Retrieve values of the various component xformOps at a given time.
    ///
    /// Matches C++ `GetXformVectors()`.
    pub fn get_xform_vectors(
        &self,
        translation: &mut Vec3d,
        rotation: &mut Vec3f,
        scale: &mut Vec3f,
        pivot: &mut Vec3f,
        rot_order: &mut RotationOrder,
        time: TimeCode,
    ) -> bool {
        let xformable = Xformable::new(self.prim().clone());
        if !xformable.is_valid() {
            return false;
        }

        // Get ordered ops
        let mut resets = false;
        let ops_vec = xformable.get_ordered_xform_ops_with_reset(&mut resets);

        // Find common ops
        let mut translate_op = XformOp::invalid();
        let mut pivot_op = XformOp::invalid();
        let mut rotate_op = XformOp::invalid();
        let mut scale_op = XformOp::invalid();

        let pivot_token = Token::new("pivot");
        let translate_token = XformOp::get_op_name(XformOpType::Translate, None, false);
        let scale_token = XformOp::get_op_name(XformOpType::Scale, None, false);
        let translate_token_str = translate_token.as_str();
        let scale_token_str = scale_token.as_str();

        for op in &ops_vec {
            let op_name = op.attr().name();
            let op_name_str = op_name.as_str();

            match op.op_type() {
                XformOpType::Translate => {
                    if !op.is_inverse_op() {
                        if op.has_suffix(&pivot_token) {
                            pivot_op = op.clone();
                        } else if op_name_str == translate_token_str {
                            translate_op = op.clone();
                        }
                    }
                }
                XformOpType::Scale => {
                    if !op.is_inverse_op() && op_name_str == scale_token_str {
                        scale_op = op.clone();
                    }
                }
                XformOpType::RotateXYZ
                | XformOpType::RotateXZY
                | XformOpType::RotateYXZ
                | XformOpType::RotateYZX
                | XformOpType::RotateZXY
                | XformOpType::RotateZYX => {
                    if !op.is_inverse_op() {
                        rotate_op = op.clone();
                    }
                }
                _ => {}
            }
        }

        // Extract values
        if translate_op.is_valid() {
            if let Some(val) = translate_op.get_typed::<Vec3d>(time) {
                *translation = val;
            } else {
                *translation = Vec3d::new(0.0, 0.0, 0.0);
            }
        } else {
            *translation = Vec3d::new(0.0, 0.0, 0.0);
        }

        if rotate_op.is_valid() {
            if let Some(val) = rotate_op.get_typed::<Vec3f>(time) {
                *rotation = val;
            } else {
                *rotation = Vec3f::new(0.0, 0.0, 0.0);
            }
            if let Some(order) = Self::convert_op_type_to_rotation_order(rotate_op.op_type()) {
                *rot_order = order;
            } else {
                *rot_order = RotationOrder::XYZ;
            }
        } else {
            *rotation = Vec3f::new(0.0, 0.0, 0.0);
            *rot_order = RotationOrder::XYZ;
        }

        if scale_op.is_valid() {
            if let Some(val) = scale_op.get_typed::<Vec3f>(time) {
                *scale = val;
            } else {
                *scale = Vec3f::new(1.0, 1.0, 1.0);
            }
        } else {
            *scale = Vec3f::new(1.0, 1.0, 1.0);
        }

        if pivot_op.is_valid() {
            if let Some(val) = pivot_op.get_typed::<Vec3f>(time) {
                *pivot = val;
            } else if let Some(val) = pivot_op.get_typed::<Vec3d>(time) {
                *pivot = Vec3f::new(val.x as f32, val.y as f32, val.z as f32);
            } else {
                *pivot = Vec3f::new(0.0, 0.0, 0.0);
            }
        } else {
            *pivot = Vec3f::new(0.0, 0.0, 0.0);
        }

        // If no compatible ops found but the prim has xform ops, decompose
        // the composed local transformation matrix (C++ fallback behavior).
        let has_any_common_op = translate_op.is_valid()
            || rotate_op.is_valid()
            || scale_op.is_valid()
            || pivot_op.is_valid();
        if !has_any_common_op && !ops_vec.is_empty() {
            let local_xf = xformable.get_local_transformation(time);
            *translation = local_xf.extract_translation();
            *pivot = Vec3f::new(0.0, 0.0, 0.0);
            *rot_order = RotationOrder::XYZ;

            // Extract scale from row lengths of upper-left 3x3 (row-major)
            let sx = (local_xf[0][0] * local_xf[0][0]
                + local_xf[0][1] * local_xf[0][1]
                + local_xf[0][2] * local_xf[0][2])
                .sqrt();
            let sy = (local_xf[1][0] * local_xf[1][0]
                + local_xf[1][1] * local_xf[1][1]
                + local_xf[1][2] * local_xf[1][2])
                .sqrt();
            let sz = (local_xf[2][0] * local_xf[2][0]
                + local_xf[2][1] * local_xf[2][1]
                + local_xf[2][2] * local_xf[2][2])
                .sqrt();
            *scale = Vec3f::new(sx as f32, sy as f32, sz as f32);

            // Build normalized rotation matrix (divide each row by its scale)
            let inv_sx = if sx.abs() > 1e-12 { 1.0 / sx } else { 0.0 };
            let inv_sy = if sy.abs() > 1e-12 { 1.0 / sy } else { 0.0 };
            let inv_sz = if sz.abs() > 1e-12 { 1.0 / sz } else { 0.0 };
            let rot_mat = Matrix3d::new(
                local_xf[0][0] * inv_sx,
                local_xf[0][1] * inv_sx,
                local_xf[0][2] * inv_sx,
                local_xf[1][0] * inv_sy,
                local_xf[1][1] * inv_sy,
                local_xf[1][2] * inv_sy,
                local_xf[2][0] * inv_sz,
                local_xf[2][1] * inv_sz,
                local_xf[2][2] * inv_sz,
            );
            // Build a 4x4 from the normalized rotation for Euler extraction
            let rot_4x4 = Matrix4d::new(
                rot_mat[0][0],
                rot_mat[0][1],
                rot_mat[0][2],
                0.0,
                rot_mat[1][0],
                rot_mat[1][1],
                rot_mat[1][2],
                0.0,
                rot_mat[2][0],
                rot_mat[2][1],
                rot_mat[2][2],
                0.0,
                0.0,
                0.0,
                0.0,
                1.0,
            );
            *rotation = Self::extract_euler_angles(&rot_4x4, XformOpType::RotateXYZ);
        }

        true
    }

    /// Return the 4x4 matrix that applies the rotation encoded by the
    /// given `rotation` vector using the given `rotation_order`.
    ///
    /// Matches C++ `UsdGeomXformCommonAPI::GetRotationTransform()`.
    #[deprecated(note = "Use ConvertRotationOrderToOpType + XformOp::get_op_transform_static")]
    pub fn get_rotation_transform(rotation: Vec3f, rotation_order: RotationOrder) -> Matrix4d {
        let op_type = Self::convert_rotation_order_to_op_type(rotation_order);
        XformOp::get_op_transform_static(op_type, &usd_vt::Value::from_no_hash(rotation), false)
    }

    /// Retrieve xform vectors by accumulating compatible xform ops.
    ///
    /// If the schema is compatible, delegates to `get_xform_vectors`.
    /// Otherwise tries to reduce ops by accumulating transforms of the
    /// common types (translate, rotate, scale, pivot). Falls back to
    /// full matrix decomposition if reduction fails.
    ///
    /// Matches C++ `UsdGeomXformCommonAPI::GetXformVectorsByAccumulation()`.
    pub fn get_xform_vectors_by_accumulation(
        &self,
        translation: &mut Vec3d,
        rotation: &mut Vec3f,
        scale: &mut Vec3f,
        pivot: &mut Vec3f,
        rot_order: &mut RotationOrder,
        time: TimeCode,
    ) -> bool {
        // If compatible, use the standard extraction.
        // We check compatibility by trying get_xform_vectors first.
        let xformable = Xformable::new(self.prim().clone());
        if !xformable.is_valid() {
            return false;
        }

        let mut resets = false;
        let ops = xformable.get_ordered_xform_ops_with_reset(&mut resets);

        // Detect the rotation op type present in the ops
        let rotate_op_type = Self::get_rotate_op_type(&ops);

        // Build the expected common op type order and indices
        let (
            common_op_types,
            translate_idx,
            translate_pivot_idx,
            rotate_idx,
            translate_identity_idx,
            scale_idx,
            translate_pivot_invert_idx,
        ) = Self::get_common_op_types_for_order(&ops, rotate_op_type);

        // Accumulate transforms into per-slot matrices
        let mut matrices: Vec<Matrix4d> = vec![Matrix4d::identity(); common_op_types.len()];

        // Scan backwards through ops and common types
        let mut op_idx = ops.len() as isize - 1;
        let mut common_idx = common_op_types.len() as isize - 1;

        while op_idx >= 0 && common_idx >= translate_idx as isize {
            let xform_op = &ops[op_idx as usize];
            let common_type = common_op_types[common_idx as usize];

            if xform_op.op_type() != common_type {
                common_idx -= 1;
                continue;
            }

            // Accumulate transform
            let op_xform = xform_op.get_op_transform(time);
            matrices[common_idx as usize] *= op_xform;
            op_idx -= 1;

            if common_type == rotate_op_type {
                // Don't accumulate multiple rotates
                common_idx -= 1;
            } else if common_type == XformOpType::Translate {
                if xform_op.is_inverse_op() {
                    common_idx -= 1;
                } else if common_idx as usize == translate_pivot_idx
                    && Self::matrices_are_inverses(
                        &matrices[translate_pivot_idx],
                        &matrices[translate_pivot_invert_idx],
                    )
                {
                    common_idx -= 1;
                }
            }
        }

        let mut reducible = true;

        if op_idx >= translate_idx as isize {
            reducible = false;
        }

        // Check identity constraint between rotate and scale pivots
        if let Some(ti_idx) = translate_identity_idx {
            if !Self::is_matrix_identity(&matrices[ti_idx]) {
                reducible = false;
            }
        }

        // Handle translate-only case
        if common_idx as usize == translate_pivot_invert_idx {
            matrices[translate_idx] = matrices[translate_pivot_invert_idx];
            matrices[translate_pivot_invert_idx] = Matrix4d::identity();
        }

        // Verify pivot/inverse-pivot are inverses
        if !Self::matrices_are_inverses(
            &matrices[translate_pivot_idx],
            &matrices[translate_pivot_invert_idx],
        ) {
            reducible = false;
        }

        if !reducible {
            // Fall back to matrix decomposition of the composed transform
            let local_xf = xformable.get_local_transformation(time);
            *translation = local_xf.extract_translation();
            *pivot = Vec3f::new(0.0, 0.0, 0.0);
            // Extract scale from row lengths
            let sx = (local_xf[0][0] * local_xf[0][0]
                + local_xf[0][1] * local_xf[0][1]
                + local_xf[0][2] * local_xf[0][2])
                .sqrt();
            let sy = (local_xf[1][0] * local_xf[1][0]
                + local_xf[1][1] * local_xf[1][1]
                + local_xf[1][2] * local_xf[1][2])
                .sqrt();
            let sz = (local_xf[2][0] * local_xf[2][0]
                + local_xf[2][1] * local_xf[2][1]
                + local_xf[2][2] * local_xf[2][2])
                .sqrt();
            *scale = Vec3f::new(sx as f32, sy as f32, sz as f32);
            // Build normalized rotation 3x3
            let inv_sx = if sx.abs() > 1e-12 { 1.0 / sx } else { 0.0 };
            let inv_sy = if sy.abs() > 1e-12 { 1.0 / sy } else { 0.0 };
            let inv_sz = if sz.abs() > 1e-12 { 1.0 / sz } else { 0.0 };
            let rot_4x4 = Matrix4d::new(
                local_xf[0][0] * inv_sx,
                local_xf[0][1] * inv_sx,
                local_xf[0][2] * inv_sx,
                0.0,
                local_xf[1][0] * inv_sy,
                local_xf[1][1] * inv_sy,
                local_xf[1][2] * inv_sy,
                0.0,
                local_xf[2][0] * inv_sz,
                local_xf[2][1] * inv_sz,
                local_xf[2][2] * inv_sz,
                0.0,
                0.0,
                0.0,
                0.0,
                1.0,
            );
            let actual_rot_type = if Self::can_convert_op_type_to_rotation_order(rotate_op_type) {
                rotate_op_type
            } else {
                XformOpType::RotateXYZ
            };
            *rotation = Self::extract_euler_angles(&rot_4x4, actual_rot_type);
            *rot_order = if Self::can_convert_op_type_to_rotation_order(rotate_op_type) {
                Self::convert_op_type_to_rotation_order(rotate_op_type)
                    .unwrap_or(RotationOrder::XYZ)
            } else {
                RotationOrder::XYZ
            };
            return true;
        }

        // Extract components from accumulated matrices
        *translation = matrices[translate_idx].extract_translation();

        let pivot_tr = matrices[translate_pivot_idx].extract_translation();
        *pivot = Vec3f::new(pivot_tr.x as f32, pivot_tr.y as f32, pivot_tr.z as f32);

        if let Some(r_idx) = rotate_idx {
            // Extract Euler angles directly from the rotation matrix.
            // m is the 4x4 accumulated rotation matrix (pure rotation, no scale).
            let m = &matrices[r_idx];
            *rotation = Self::extract_euler_angles(m, rotate_op_type);
        } else {
            *rotation = Vec3f::new(0.0, 0.0, 0.0);
        }

        if let Some(s_idx) = scale_idx {
            *scale = Vec3f::new(
                matrices[s_idx][0][0] as f32,
                matrices[s_idx][1][1] as f32,
                matrices[s_idx][2][2] as f32,
            );
        } else {
            *scale = Vec3f::new(1.0, 1.0, 1.0);
        }

        *rot_order = if Self::can_convert_op_type_to_rotation_order(rotate_op_type) {
            Self::convert_op_type_to_rotation_order(rotate_op_type).unwrap_or(RotationOrder::XYZ)
        } else {
            RotationOrder::XYZ
        };

        true
    }

    // ========================================================================
    // Private helpers for GetXformVectorsByAccumulation
    // ========================================================================

    /// Returns true if `op_type` is any kind of rotation.
    fn is_rotate_op_type(op_type: XformOpType) -> bool {
        matches!(
            op_type,
            XformOpType::RotateXYZ
                | XformOpType::RotateXZY
                | XformOpType::RotateYXZ
                | XformOpType::RotateYZX
                | XformOpType::RotateZXY
                | XformOpType::RotateZYX
                | XformOpType::RotateX
                | XformOpType::RotateY
                | XformOpType::RotateZ
        )
    }

    /// Finds the rotation op type from the ops list.
    fn get_rotate_op_type(ops: &[XformOp]) -> XformOpType {
        for op in ops {
            if Self::is_rotate_op_type(op.op_type()) {
                return op.op_type();
            }
        }
        XformOpType::RotateXYZ
    }

    /// Builds the expected common op type order and returns indices.
    /// Returns (common_types, translate_idx, translate_pivot_idx,
    ///          rotate_idx, translate_identity_idx, scale_idx,
    ///          translate_pivot_invert_idx).
    fn get_common_op_types_for_order(
        ops: &[XformOp],
        rotate_op_type: XformOpType,
    ) -> (
        Vec<XformOpType>,
        usize,
        usize,
        Option<usize>,
        Option<usize>,
        Option<usize>,
        usize,
    ) {
        let mut has_rotate = false;
        let mut has_scale = false;
        let mut num_inverse_translate: usize = 0;

        for op in ops {
            if Self::is_rotate_op_type(op.op_type()) {
                has_rotate = true;
            } else if op.op_type() == XformOpType::Scale {
                has_scale = true;
            } else if op.op_type() == XformOpType::Translate && op.is_inverse_op() {
                num_inverse_translate += 1;
            }
        }

        let mut common_types = Vec::new();
        let mut idx = 0usize;

        // Translate and TranslatePivot are always present
        common_types.push(XformOpType::Translate);
        let translate_idx = idx;
        idx += 1;
        common_types.push(XformOpType::Translate);
        let translate_pivot_idx = idx;
        idx += 1;

        let rotate_idx = if has_rotate {
            common_types.push(rotate_op_type);
            let ri = idx;
            idx += 1;
            Some(ri)
        } else {
            None
        };

        let translate_identity_idx = if num_inverse_translate > 1 {
            common_types.push(XformOpType::Translate);
            let ti = idx;
            idx += 1;
            Some(ti)
        } else {
            None
        };

        let scale_idx = if has_scale {
            common_types.push(XformOpType::Scale);
            let si = idx;
            idx += 1;
            Some(si)
        } else {
            None
        };

        common_types.push(XformOpType::Translate);
        let translate_pivot_invert_idx = idx;

        (
            common_types,
            translate_idx,
            translate_pivot_idx,
            rotate_idx,
            translate_identity_idx,
            scale_idx,
            translate_pivot_invert_idx,
        )
    }

    /// Returns true if two matrices are approximate inverses of each other.
    fn matrices_are_inverses(a: &Matrix4d, b: &Matrix4d) -> bool {
        let product = *a * *b;
        Self::is_matrix_identity(&product)
    }

    /// Returns true if a matrix is approximately identity.
    fn is_matrix_identity(m: &Matrix4d) -> bool {
        let id = Matrix4d::identity();
        const EPS: f64 = 1e-9;
        for r in 0..4 {
            for c in 0..4 {
                if (m[r][c] - id[r][c]).abs() > EPS {
                    return false;
                }
            }
        }
        true
    }

    /// Extracts Euler angles from a pure rotation Matrix4d (row-vector convention).
    /// Matrices are transposed relative to the standard column-vector convention:
    /// M[i][j] = R_std[j][i]. Uses M[col][row] where standard uses R[row][col].
    fn extract_euler_angles(m: &Matrix4d, op_type: XformOpType) -> Vec3f {
        let to_deg = |rad: f64| -> f32 { (rad * 180.0 / std::f64::consts::PI) as f32 };

        match op_type {
            XformOpType::RotateXYZ => {
                // Standard R = Rz*Ry*Rx; M = R^T so M[c][r] = R[r][c]
                let y = (-m[0][2]).asin();
                let cy = y.cos();
                let (x, z) = if cy.abs() > 1e-6 {
                    (m[1][2].atan2(m[2][2]), m[0][1].atan2(m[0][0]))
                } else {
                    ((-m[2][1]).atan2(m[1][1]), 0.0)
                };
                Vec3f::new(to_deg(x), to_deg(y), to_deg(z))
            }
            XformOpType::RotateXZY => {
                // Standard R = Ry*Rz*Rx; M = R^T
                let z = m[0][1].asin();
                let cz = z.cos();
                let (x, y) = if cz.abs() > 1e-6 {
                    ((-m[2][1]).atan2(m[1][1]), (-m[0][2]).atan2(m[0][0]))
                } else {
                    (m[1][2].atan2(m[2][2]), 0.0)
                };
                Vec3f::new(to_deg(x), to_deg(y), to_deg(z))
            }
            XformOpType::RotateYXZ => {
                // Standard R = Rz*Rx*Ry; M = R^T
                let x = m[1][2].asin();
                let cx = x.cos();
                let (y, z) = if cx.abs() > 1e-6 {
                    ((-m[0][2]).atan2(m[2][2]), (-m[1][0]).atan2(m[1][1]))
                } else {
                    (m[2][0].atan2(m[0][0]), 0.0)
                };
                Vec3f::new(to_deg(x), to_deg(y), to_deg(z))
            }
            XformOpType::RotateYZX => {
                // Standard R = Rx*Rz*Ry; M = R^T
                let z = (-m[1][0]).asin();
                let cz = z.cos();
                let (y, x) = if cz.abs() > 1e-6 {
                    (m[2][0].atan2(m[0][0]), m[1][2].atan2(m[1][1]))
                } else {
                    ((-m[0][2]).atan2(m[2][2]), 0.0)
                };
                Vec3f::new(to_deg(x), to_deg(y), to_deg(z))
            }
            XformOpType::RotateZXY => {
                // Standard R = Ry*Rx*Rz; M = R^T
                let x = (-m[2][1]).asin();
                let cx = x.cos();
                let (z, y) = if cx.abs() > 1e-6 {
                    (m[0][1].atan2(m[1][1]), m[2][0].atan2(m[2][2]))
                } else {
                    ((-m[1][0]).atan2(m[0][0]), 0.0)
                };
                Vec3f::new(to_deg(x), to_deg(y), to_deg(z))
            }
            XformOpType::RotateZYX => {
                // Standard R = Rx*Ry*Rz; M = R^T
                let y = m[2][0].asin();
                let cy = y.cos();
                let (z, x) = if cy.abs() > 1e-6 {
                    ((-m[1][0]).atan2(m[0][0]), (-m[2][1]).atan2(m[2][2]))
                } else {
                    (m[0][1].atan2(m[1][1]), 0.0)
                };
                Vec3f::new(to_deg(x), to_deg(y), to_deg(z))
            }
            XformOpType::RotateX => {
                let x = m[1][2].atan2(m[2][2]);
                Vec3f::new(to_deg(x), 0.0, 0.0)
            }
            XformOpType::RotateY => {
                let y = (-m[0][2]).atan2(m[2][2]);
                Vec3f::new(0.0, to_deg(y), 0.0)
            }
            XformOpType::RotateZ => {
                let z = m[0][1].atan2(m[0][0]);
                Vec3f::new(0.0, 0.0, to_deg(z))
            }
            _ => {
                // Default: XYZ
                let y = (-m[0][2]).asin();
                let cy = y.cos();
                let (x, z) = if cy.abs() > 1e-6 {
                    (m[1][2].atan2(m[2][2]), m[0][1].atan2(m[0][0]))
                } else {
                    ((-m[2][1]).atan2(m[1][1]), 0.0)
                };
                Vec3f::new(to_deg(x), to_deg(y), to_deg(z))
            }
        }
    }
}

impl PartialEq for XformCommonAPI {
    fn eq(&self, other: &Self) -> bool {
        self.base == other.base
    }
}

impl Eq for XformCommonAPI {}
