//! UsdGeomXformOp - schema wrapper for transform operation attributes.
//!
//! Port of pxr/usd/usdGeom/xformOp.h/cpp
//!
//! Schema wrapper for UsdAttribute for authoring and computing
//! transformation operations, as consumed by UsdGeomXformable schema.

use usd_core::{Attribute, AttributeQuery};
use usd_gf::{Interval, Matrix4d, Quatd, Quatf, Vec3d, Vec3f};
use usd_sdf::{TimeCode, ValueTypeName, ValueTypeRegistry};
use usd_tf::Token;
use usd_vt::Value;

mod tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    pub static XFORM_OP_PREFIX: LazyLock<Token> = LazyLock::new(|| Token::new("xformOp:"));
    pub static INVERT_PREFIX: LazyLock<Token> = LazyLock::new(|| Token::new("!invert!"));
    pub static INVERSE_XFORM_OP_PREFIX: LazyLock<Token> =
        LazyLock::new(|| Token::new("!invert!xformOp:"));
}

// ============================================================================
// XformOp Type
// ============================================================================

/// Enumerates the set of all transformation operation types.
///
/// Matches C++ `UsdGeomXformOp::Type`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum XformOpType {
    /// Represents an invalid xformOp.
    Invalid,
    /// Translation along the X-axis.
    TranslateX,
    /// Translation along the Y-axis.
    TranslateY,
    /// Translation along the Z-axis.
    TranslateZ,
    /// XYZ translation.
    Translate,
    /// Scale along the X-axis.
    ScaleX,
    /// Scale along the Y-axis.
    ScaleY,
    /// Scale along the Z-axis.
    ScaleZ,
    /// XYZ scale.
    Scale,
    /// Rotation about the X-axis, in degrees.
    RotateX,
    /// Rotation about the Y-axis, in degrees.
    RotateY,
    /// Rotation about the Z-axis, in degrees.
    RotateZ,
    /// Set of 3 canonical Euler rotations in XYZ order.
    RotateXYZ,
    /// Set of 3 canonical Euler rotations in XZY order.
    RotateXZY,
    /// Set of 3 canonical Euler rotations in YXZ order.
    RotateYXZ,
    /// Set of 3 canonical Euler rotations in YZX order.
    RotateYZX,
    /// Set of 3 canonical Euler rotations in ZXY order.
    RotateZXY,
    /// Set of 3 canonical Euler rotations in ZYX order.
    RotateZYX,
    /// Arbitrary axis/angle rotation, expressed as a quaternion.
    Orient,
    /// A 4x4 matrix transformation.
    Transform,
}

impl std::fmt::Display for XformOpType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            XformOpType::Invalid => write!(f, "invalid"),
            XformOpType::TranslateX => write!(f, "translateX"),
            XformOpType::TranslateY => write!(f, "translateY"),
            XformOpType::TranslateZ => write!(f, "translateZ"),
            XformOpType::Translate => write!(f, "translate"),
            XformOpType::ScaleX => write!(f, "scaleX"),
            XformOpType::ScaleY => write!(f, "scaleY"),
            XformOpType::ScaleZ => write!(f, "scaleZ"),
            XformOpType::Scale => write!(f, "scale"),
            XformOpType::RotateX => write!(f, "rotateX"),
            XformOpType::RotateY => write!(f, "rotateY"),
            XformOpType::RotateZ => write!(f, "rotateZ"),
            XformOpType::RotateXYZ => write!(f, "rotateXYZ"),
            XformOpType::RotateXZY => write!(f, "rotateXZY"),
            XformOpType::RotateYXZ => write!(f, "rotateYXZ"),
            XformOpType::RotateYZX => write!(f, "rotateYZX"),
            XformOpType::RotateZXY => write!(f, "rotateZXY"),
            XformOpType::RotateZYX => write!(f, "rotateZYX"),
            XformOpType::Orient => write!(f, "orient"),
            XformOpType::Transform => write!(f, "transform"),
        }
    }
}

// ============================================================================
// XformOp Precision
// ============================================================================

/// Precision of the encoded transformation operation's value.
///
/// Matches C++ `UsdGeomXformOp::Precision`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum XformOpPrecision {
    /// Double precision
    Double,
    /// Floating-point precision
    Float,
    /// Half-float precision
    Half,
}

impl std::fmt::Display for XformOpPrecision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            XformOpPrecision::Double => write!(f, "double"),
            XformOpPrecision::Float => write!(f, "float"),
            XformOpPrecision::Half => write!(f, "half"),
        }
    }
}

// ============================================================================
// XformOp
// ============================================================================

/// Schema wrapper for UsdAttribute for authoring and computing
/// transformation operations.
///
/// Matches C++ `UsdGeomXformOp`.
#[derive(Clone)]
pub struct XformOp {
    /// The underlying attribute.
    attr: Attribute,
    /// Optional cached attribute query used by `XformQuery`, matching the
    /// OpenUSD `UsdAttributeQuery`-backed hot path.
    attr_query: Option<AttributeQuery>,
    /// The operation type.
    op_type: XformOpType,
    /// Whether this is an inverse operation.
    is_inverse_op: bool,
}

impl std::fmt::Debug for XformOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("XformOp")
            .field("attr", &self.attr)
            .field("has_attr_query", &self.attr_query.is_some())
            .field("op_type", &self.op_type)
            .field("is_inverse_op", &self.is_inverse_op)
            .finish()
    }
}

impl XformOp {
    /// Creates an invalid XformOp.
    pub fn invalid() -> Self {
        Self {
            attr: Attribute::invalid(),
            attr_query: None,
            op_type: XformOpType::Invalid,
            is_inverse_op: false,
        }
    }

    /// Creates a XformOp from an attribute.
    ///
    /// Matches C++ `UsdGeomXformOp(const UsdAttribute &attr, bool isInverseOp)`.
    pub fn new(attr: Attribute, is_inverse_op: bool) -> Self {
        let op_type = if attr.is_valid() {
            Self::init_op_type_from_attr_name(&attr.name())
        } else {
            XformOpType::Invalid
        };

        Self {
            attr,
            attr_query: None,
            op_type,
            is_inverse_op,
        }
    }

    /// Creates a XformOp from an attribute query.
    ///
    /// Matches C++ `UsdGeomXformOp(UsdAttributeQuery &&query, bool isInverseOp)`.
    pub fn new_with_query(query: AttributeQuery, is_inverse_op: bool) -> Self {
        let attr = query.attribute().clone();
        let op_type = if attr.is_valid() {
            Self::init_op_type_from_attr_name(&attr.name())
        } else {
            XformOpType::Invalid
        };

        Self {
            attr,
            attr_query: Some(query),
            op_type,
            is_inverse_op,
        }
    }

    /// Returns true if this XformOp is valid.
    pub fn is_valid(&self) -> bool {
        self.attr.is_valid() && self.op_type != XformOpType::Invalid
    }

    /// Returns the operation type of this op.
    pub fn op_type(&self) -> XformOpType {
        self.op_type
    }

    /// Returns whether the xformOp represents an inverse operation.
    pub fn is_inverse_op(&self) -> bool {
        self.is_inverse_op
    }

    /// Returns the underlying attribute.
    pub fn attr(&self) -> &Attribute {
        &self.attr
    }

    pub(crate) fn attr_query(&self) -> Option<&AttributeQuery> {
        self.attr_query.as_ref()
    }

    /// Returns the opName as it appears in the xformOpOrder attribute.
    ///
    /// Matches C++ `GetOpName()`.
    pub fn op_name(&self) -> Token {
        if !self.is_valid() {
            return Token::new("");
        }

        let base_name = self.attr.name();
        if self.is_inverse_op {
            Token::new(&format!("!invert!{}", base_name.as_str()))
        } else {
            base_name
        }
    }

    /// Gets the value at the specified time.
    pub fn get(&self, time: TimeCode) -> Option<Value> {
        if let Some(query) = &self.attr_query {
            query.get(time)
        } else {
            self.attr.get(time)
        }
    }

    /// Gets the value at the specified time, with type checking.
    ///
    /// Matches C++ `Get<T>()`.
    pub fn get_typed<T: Clone + 'static>(&self, time: TimeCode) -> Option<T> {
        self.get(time).and_then(|v| v.downcast_clone::<T>())
    }

    /// Sets the value at the specified time.
    ///
    /// Matches C++ `Set<T>()`.
    /// Note: Setting a value on an inverse op is disallowed.
    pub fn set(&self, value: impl Into<Value>, time: TimeCode) -> bool {
        if self.is_inverse_op {
            // Error: Cannot set value on inverse op
            return false;
        }
        self.attr.set(value, time)
    }

    /// Return the 4x4 matrix that applies the transformation encoded
    /// in this op at the given time.
    ///
    /// Matches C++ `GetOpTransform()`.
    pub fn get_op_transform(&self, time: TimeCode) -> Matrix4d {
        if !self.is_valid() {
            return Matrix4d::identity();
        }

        // Get the value and compute transform
        if let Some(value) = self.get(time) {
            Self::get_op_transform_static(self.op_type, &value, self.is_inverse_op)
        } else {
            Matrix4d::identity()
        }
    }

    /// Determine whether there is any possibility that this op's value
    /// may vary over time.
    ///
    /// Matches C++ `MightBeTimeVarying()`.
    pub fn might_be_time_varying(&self) -> bool {
        if !self.is_valid() {
            return false;
        }
        if let Some(query) = &self.attr_query {
            query.value_might_be_time_varying()
        } else {
            self.attr.might_be_time_varying()
        }
    }

    /// Populates the list of time samples at which the associated attribute is authored.
    ///
    /// Matches C++ `GetTimeSamples()`.
    pub fn get_time_samples(&self) -> Vec<f64> {
        if !self.is_valid() {
            return Vec::new();
        }
        if let Some(query) = &self.attr_query {
            query.get_time_samples()
        } else {
            self.attr.get_time_samples()
        }
    }

    /// Populates the list of time samples within the given interval.
    ///
    /// Matches C++ `GetTimeSamplesInInterval()`.
    pub fn get_time_samples_in_interval(&self, interval: &Interval) -> Vec<f64> {
        if !self.is_valid() {
            return Vec::new();
        }
        let start = interval.get_min();
        let end = interval.get_max();
        if let Some(query) = &self.attr_query {
            query.get_time_samples_in_interval(interval)
        } else {
            self.attr.get_time_samples_in_interval(start, end)
        }
    }

    /// Returns the number of time samples authored for this xformOp.
    ///
    /// Matches C++ `GetNumTimeSamples()`.
    pub fn get_num_time_samples(&self) -> usize {
        self.get_time_samples().len()
    }

    /// Returns the name of this xformOp attribute.
    ///
    /// Matches C++ `GetName()`.
    pub fn name(&self) -> Token {
        self.attr.name()
    }

    /// Returns the precision level of the xform op.
    ///
    /// Matches C++ `GetPrecision()`.
    pub fn precision(&self) -> XformOpPrecision {
        if !self.is_valid() {
            return XformOpPrecision::Double;
        }
        // Get type name token and convert to ValueTypeName
        let type_token = self.attr.type_name();
        let registry = ValueTypeRegistry::instance();
        let type_name = registry.find_type_by_token(&type_token);
        if type_name.is_valid() {
            Self::get_precision_from_value_type_name(&type_name)
        } else {
            XformOpPrecision::Double
        }
    }

    /// Does this op have the given suffix in its name.
    ///
    /// Matches C++ `HasSuffix()`.
    pub fn has_suffix(&self, suffix: &Token) -> bool {
        if !self.is_valid() || suffix.is_empty() {
            return false;
        }
        let name = self.attr.name();
        let name_str = name.as_str();
        let suffix_str = suffix.as_str();
        // Check if name ends with :suffix
        name_str.ends_with(&format!(":{}", suffix_str))
    }

    /// Returns the base name of the attribute (without namespace).
    ///
    /// Matches C++ `GetBaseName()`.
    pub fn base_name(&self) -> Token {
        let name = self.attr.name();
        let name_str = name.as_str();
        if let Some(idx) = name_str.rfind(':') {
            Token::new(&name_str[idx + 1..])
        } else {
            name
        }
    }

    /// Returns the namespace of the attribute.
    ///
    /// Matches C++ `GetNamespace()`.
    pub fn namespace(&self) -> Token {
        let name = self.attr.name();
        let name_str = name.as_str();
        if let Some(idx) = name_str.rfind(':') {
            Token::new(&name_str[..idx])
        } else {
            Token::new("")
        }
    }

    /// Splits the attribute name into components.
    ///
    /// Matches C++ `SplitName()`.
    pub fn split_name(&self) -> Vec<String> {
        let name = self.attr.name();
        name.as_str().split(':').map(String::from).collect()
    }

    /// Returns the value type name of the attribute.
    ///
    /// Matches C++ `GetTypeName()`.
    pub fn type_name(&self) -> ValueTypeName {
        let type_token = self.attr.type_name();
        let registry = ValueTypeRegistry::instance();
        registry.find_type_by_token(&type_token)
    }

    // ========================================================================
    // Static Helper Methods
    // ========================================================================

    /// Test whether a given attribute name represents a valid XformOp.
    ///
    /// Matches C++ `IsXformOp(const TfToken &attrName)`.
    pub fn is_xform_op(attr_name: &Token) -> bool {
        attr_name
            .as_str()
            .starts_with(tokens::XFORM_OP_PREFIX.as_str())
    }

    /// Returns the TfToken used to encode the given opType.
    ///
    /// Matches C++ `GetOpTypeToken()`.
    pub fn get_op_type_token(op_type: XformOpType) -> Token {
        match op_type {
            XformOpType::Invalid => Token::new(""),
            XformOpType::TranslateX => Token::new("translateX"),
            XformOpType::TranslateY => Token::new("translateY"),
            XformOpType::TranslateZ => Token::new("translateZ"),
            XformOpType::Translate => Token::new("translate"),
            XformOpType::ScaleX => Token::new("scaleX"),
            XformOpType::ScaleY => Token::new("scaleY"),
            XformOpType::ScaleZ => Token::new("scaleZ"),
            XformOpType::Scale => Token::new("scale"),
            XformOpType::RotateX => Token::new("rotateX"),
            XformOpType::RotateY => Token::new("rotateY"),
            XformOpType::RotateZ => Token::new("rotateZ"),
            XformOpType::RotateXYZ => Token::new("rotateXYZ"),
            XformOpType::RotateXZY => Token::new("rotateXZY"),
            XformOpType::RotateYXZ => Token::new("rotateYXZ"),
            XformOpType::RotateYZX => Token::new("rotateYZX"),
            XformOpType::RotateZXY => Token::new("rotateZXY"),
            XformOpType::RotateZYX => Token::new("rotateZYX"),
            XformOpType::Orient => Token::new("orient"),
            XformOpType::Transform => Token::new("transform"),
        }
    }

    /// Returns the Type enum associated with the given opTypeToken.
    ///
    /// Matches C++ `GetOpTypeEnum()`.
    pub fn get_op_type_enum(op_type_token: &Token) -> XformOpType {
        Self::get_op_type_enum_from_str(op_type_token.as_str())
    }

    fn get_op_type_enum_from_str(op_type_token: &str) -> XformOpType {
        match op_type_token {
            "translateX" => XformOpType::TranslateX,
            "translateY" => XformOpType::TranslateY,
            "translateZ" => XformOpType::TranslateZ,
            "translate" => XformOpType::Translate,
            "scaleX" => XformOpType::ScaleX,
            "scaleY" => XformOpType::ScaleY,
            "scaleZ" => XformOpType::ScaleZ,
            "scale" => XformOpType::Scale,
            "rotateX" => XformOpType::RotateX,
            "rotateY" => XformOpType::RotateY,
            "rotateZ" => XformOpType::RotateZ,
            "rotateXYZ" => XformOpType::RotateXYZ,
            "rotateXZY" => XformOpType::RotateXZY,
            "rotateYXZ" => XformOpType::RotateYXZ,
            "rotateYZX" => XformOpType::RotateYZX,
            "rotateZXY" => XformOpType::RotateZXY,
            "rotateZYX" => XformOpType::RotateZYX,
            "orient" => XformOpType::Orient,
            "transform" => XformOpType::Transform,
            _ => XformOpType::Invalid,
        }
    }

    /// Returns the precision corresponding to the given value typeName.
    ///
    /// Matches C++ `GetPrecisionFromValueTypeName()`.
    pub fn get_precision_from_value_type_name(type_name: &ValueTypeName) -> XformOpPrecision {
        match type_name.as_token().as_str() {
            "matrix4d" | "double3" | "double" | "quatd" => XformOpPrecision::Double,
            "float3" | "float" | "quatf" => XformOpPrecision::Float,
            "half3" | "half" | "quath" => XformOpPrecision::Half,
            _ => XformOpPrecision::Double,
        }
    }

    /// Returns the value typeName token that corresponds to the given
    /// combination of opType and precision.
    ///
    /// Matches C++ `GetValueTypeName()`.
    pub fn get_value_type_name(op_type: XformOpType, precision: XformOpPrecision) -> ValueTypeName {
        let registry = ValueTypeRegistry::instance();

        // Build type name string
        let type_name_str = match op_type {
            XformOpType::TranslateX
            | XformOpType::TranslateY
            | XformOpType::TranslateZ
            | XformOpType::RotateX
            | XformOpType::RotateY
            | XformOpType::RotateZ
            | XformOpType::ScaleX
            | XformOpType::ScaleY
            | XformOpType::ScaleZ => match precision {
                XformOpPrecision::Double => "double",
                XformOpPrecision::Float => "float",
                XformOpPrecision::Half => "half",
            },
            XformOpType::Translate
            | XformOpType::Scale
            | XformOpType::RotateXYZ
            | XformOpType::RotateXZY
            | XformOpType::RotateYXZ
            | XformOpType::RotateYZX
            | XformOpType::RotateZXY
            | XformOpType::RotateZYX => match precision {
                XformOpPrecision::Double => "double3",
                XformOpPrecision::Float => "float3",
                XformOpPrecision::Half => "half3",
            },
            // Orient = quaternion: quatd/quatf/quath — NOT double4/float4/half4
            // Matches C++ GetValueTypeName() in xformOp.cpp
            XformOpType::Orient => match precision {
                XformOpPrecision::Double => "quatd",
                XformOpPrecision::Float => "quatf",
                XformOpPrecision::Half => "quath",
            },
            XformOpType::Transform => match precision {
                XformOpPrecision::Double => "matrix4d",
                XformOpPrecision::Float => "matrix4f",
                XformOpPrecision::Half => "matrix4h",
            },
            XformOpType::Invalid => return ValueTypeName::invalid(),
        };

        registry.find_type_by_token(&Token::new(type_name_str))
    }

    /// Returns the xformOp's name as it appears in xformOpOrder, given
    /// the opType, the (optional) suffix and whether it is an inverse
    /// operation.
    ///
    /// Matches C++ `GetOpName()`.
    pub fn get_op_name(op_type: XformOpType, op_suffix: Option<&Token>, inverse: bool) -> Token {
        let type_token = Self::get_op_type_token(op_type);
        if type_token.is_empty() {
            return Token::new("");
        }

        let mut name = String::from(tokens::XFORM_OP_PREFIX.as_str());
        name.push_str(type_token.as_str());

        if let Some(suffix) = op_suffix {
            if !suffix.is_empty() {
                name.push(':');
                name.push_str(suffix.as_str());
            }
        }

        if inverse {
            name.insert_str(0, tokens::INVERT_PREFIX.as_str());
        }

        Token::new(&name)
    }

    /// Internal helper matching C++ `_Init()`: take the second namespace component
    /// without allocating a split vector.
    fn init_op_type_from_attr_name(attr_name: &Token) -> XformOpType {
        let name_str = attr_name.as_str();
        if !name_str.starts_with(tokens::XFORM_OP_PREFIX.as_str()) {
            return XformOpType::Invalid;
        }
        let Some(start) = name_str.find(':').map(|idx| idx + 1) else {
            return XformOpType::Invalid;
        };
        let end = name_str[start..]
            .find(':')
            .map(|idx| start + idx)
            .unwrap_or(name_str.len());
        Self::get_op_type_enum_from_str(&name_str[start..end])
    }

    /// Matches C++ `_GetXformOpAttr(UsdPrim const&, TfToken const&, bool*)`.
    pub(crate) fn get_xform_op_attr(
        prim: &usd_core::Prim,
        op_name: &Token,
        is_inverse_op: &mut bool,
    ) -> Option<Attribute> {
        let op_name_str = op_name.as_str();
        *is_inverse_op = op_name_str.starts_with(tokens::INVERSE_XFORM_OP_PREFIX.as_str());
        let attr_name = if *is_inverse_op {
            &op_name_str[tokens::INVERT_PREFIX.as_str().len()..]
        } else {
            op_name_str
        };
        prim.get_attribute(attr_name)
    }

    /// Return the 4x4 matrix that applies the transformation encoded
    /// by op opType and data value opVal.
    ///
    /// Matches C++ static `GetOpTransform()`.
    pub fn get_op_transform_static(
        op_type: XformOpType,
        op_val: &Value,
        is_inverse_op: bool,
    ) -> Matrix4d {
        // Handle Transform type (most common case)
        if op_type == XformOpType::Transform {
            if let Some(mat) = op_val.downcast::<Matrix4d>() {
                if is_inverse_op {
                    if let Some(inv) = mat.inverse() {
                        return inv;
                    } else {
                        // Degenerate matrix - return identity
                        return Matrix4d::identity();
                    }
                }
                return *mat;
            }
            // Try Matrix4f
            if let Some(mat_f) = op_val.downcast::<usd_gf::Matrix4f>() {
                // Convert Matrix4f to Matrix4d element by element
                let mat = Matrix4d::new(
                    mat_f[0][0] as f64,
                    mat_f[0][1] as f64,
                    mat_f[0][2] as f64,
                    mat_f[0][3] as f64,
                    mat_f[1][0] as f64,
                    mat_f[1][1] as f64,
                    mat_f[1][2] as f64,
                    mat_f[1][3] as f64,
                    mat_f[2][0] as f64,
                    mat_f[2][1] as f64,
                    mat_f[2][2] as f64,
                    mat_f[2][3] as f64,
                    mat_f[3][0] as f64,
                    mat_f[3][1] as f64,
                    mat_f[3][2] as f64,
                    mat_f[3][3] as f64,
                );
                if is_inverse_op {
                    if let Some(inv) = mat.inverse() {
                        return inv;
                    }
                }
                return mat;
            }
            // Invalid type for Transform
            return Matrix4d::identity();
        }

        // Handle scalar values (single-axis operations)
        let mut double_val: Option<f64> = None;
        if let Some(&val) = op_val.downcast::<f64>() {
            double_val = Some(val);
        } else if let Some(&val) = op_val.downcast::<f32>() {
            double_val = Some(val as f64);
        }

        if let Some(val) = double_val {
            match op_type {
                XformOpType::TranslateX => {
                    // Inverse translation: negate the component
                    let v = if is_inverse_op { -val } else { val };
                    return Matrix4d::from_translation(Vec3d::new(v, 0.0, 0.0));
                }
                XformOpType::TranslateY => {
                    let v = if is_inverse_op { -val } else { val };
                    return Matrix4d::from_translation(Vec3d::new(0.0, v, 0.0));
                }
                XformOpType::TranslateZ => {
                    let v = if is_inverse_op { -val } else { val };
                    return Matrix4d::from_translation(Vec3d::new(0.0, 0.0, v));
                }
                XformOpType::ScaleX => {
                    // C++ xformOp.cpp:562-564: uniformly negates doubleVal for ALL
                    // inverse scalar ops (translate, scale, rotate) before dispatch.
                    // Negation, not reciprocal: scale(-2) is a reflection, matching
                    // the C++ behavior. Vec3 scale uses reciprocal (line 615-619).
                    let v = if is_inverse_op { -val } else { val };
                    return Matrix4d::from_scale_vec(&Vec3d::new(v, 1.0, 1.0));
                }
                XformOpType::ScaleY => {
                    let v = if is_inverse_op { -val } else { val };
                    return Matrix4d::from_scale_vec(&Vec3d::new(1.0, v, 1.0));
                }
                XformOpType::ScaleZ => {
                    let v = if is_inverse_op { -val } else { val };
                    return Matrix4d::from_scale_vec(&Vec3d::new(1.0, 1.0, v));
                }
                XformOpType::RotateX => {
                    // Inverse rotation: negate the angle
                    let v = if is_inverse_op { -val } else { val };
                    return Matrix4d::from_rotation(Vec3d::x_axis(), v.to_radians());
                }
                XformOpType::RotateY => {
                    let v = if is_inverse_op { -val } else { val };
                    return Matrix4d::from_rotation(Vec3d::y_axis(), v.to_radians());
                }
                XformOpType::RotateZ => {
                    let v = if is_inverse_op { -val } else { val };
                    return Matrix4d::from_rotation(Vec3d::z_axis(), v.to_radians());
                }
                _ => {}
            }
        }

        // Handle Vec3 values (multi-axis operations)
        let mut vec3d_val: Option<Vec3d> = None;
        if let Some(&val) = op_val.downcast::<Vec3d>() {
            vec3d_val = Some(val);
        } else if let Some(&val) = op_val.downcast::<Vec3f>() {
            vec3d_val = Some(Vec3d::new(val.x as f64, val.y as f64, val.z as f64));
        } else if let Some(val) = op_val.downcast_clone::<[f64; 3]>() {
            vec3d_val = Some(Vec3d::new(val[0], val[1], val[2]));
        } else if let Some(val) = op_val.downcast_clone::<[f32; 3]>() {
            vec3d_val = Some(Vec3d::new(val[0] as f64, val[1] as f64, val[2] as f64));
        } else if let Some(values) = op_val.downcast::<Vec<Value>>() {
            if values.len() >= 3 {
                let to_f64 = |value: &Value| {
                    value
                        .downcast::<f64>()
                        .copied()
                        .or_else(|| value.downcast::<f32>().map(|v| *v as f64))
                        .or_else(|| value.downcast::<i32>().map(|v| *v as f64))
                        .or_else(|| value.downcast::<i64>().map(|v| *v as f64))
                };
                if let (Some(x), Some(y), Some(z)) =
                    (to_f64(&values[0]), to_f64(&values[1]), to_f64(&values[2]))
                {
                    vec3d_val = Some(Vec3d::new(x, y, z));
                }
            }
        }

        if vec3d_val.is_none()
            && matches!(
                op_type,
                XformOpType::Translate
                    | XformOpType::Scale
                    | XformOpType::RotateXYZ
                    | XformOpType::RotateXZY
                    | XformOpType::RotateYXZ
                    | XformOpType::RotateYZX
                    | XformOpType::RotateZXY
                    | XformOpType::RotateZYX
            )
        {
            log::warn!(
                "XformOp: Vec3 downcast failed for {:?} (inverse={}), returning identity. \
                 Value type_name: {:?}",
                op_type,
                is_inverse_op,
                op_val.type_name()
            );
        }

        if let Some(mut vec_val) = vec3d_val {
            match op_type {
                XformOpType::Translate => {
                    if is_inverse_op {
                        vec_val = -vec_val;
                    }
                    return Matrix4d::from_translation(vec_val);
                }
                XformOpType::Scale => {
                    if is_inverse_op {
                        vec_val = Vec3d::new(1.0 / vec_val.x, 1.0 / vec_val.y, 1.0 / vec_val.z);
                    }
                    return Matrix4d::from_scale_vec(&vec_val);
                }
                XformOpType::RotateXYZ
                | XformOpType::RotateXZY
                | XformOpType::RotateYXZ
                | XformOpType::RotateYZX
                | XformOpType::RotateZXY
                | XformOpType::RotateZYX => {
                    if is_inverse_op {
                        // Inverse: negate angles AND reverse multiplication order.
                        // Inv(A*B*C) = Inv(C)*Inv(B)*Inv(A) = C(-a)*B(-b)*A(-c)
                        // Matches C++ UsdGeomXformOp::GetOpTransform (xformOp.cpp:633-656)
                        vec_val = -vec_val;
                    }
                    let x_mat = Matrix4d::from_rotation(Vec3d::x_axis(), vec_val.x.to_radians());
                    let y_mat = Matrix4d::from_rotation(Vec3d::y_axis(), vec_val.y.to_radians());
                    let z_mat = Matrix4d::from_rotation(Vec3d::z_axis(), vec_val.z.to_radians());

                    let result = if is_inverse_op {
                        // Reversed order for inverse (C++ reference behavior)
                        match op_type {
                            XformOpType::RotateXYZ => z_mat * y_mat * x_mat,
                            XformOpType::RotateXZY => y_mat * z_mat * x_mat,
                            XformOpType::RotateYXZ => z_mat * x_mat * y_mat,
                            XformOpType::RotateYZX => x_mat * z_mat * y_mat,
                            XformOpType::RotateZXY => y_mat * x_mat * z_mat,
                            XformOpType::RotateZYX => x_mat * y_mat * z_mat,
                            _ => return Matrix4d::identity(),
                        }
                    } else {
                        match op_type {
                            XformOpType::RotateXYZ => x_mat * y_mat * z_mat,
                            XformOpType::RotateXZY => x_mat * z_mat * y_mat,
                            XformOpType::RotateYXZ => y_mat * x_mat * z_mat,
                            XformOpType::RotateYZX => y_mat * z_mat * x_mat,
                            XformOpType::RotateZXY => z_mat * x_mat * y_mat,
                            XformOpType::RotateZYX => z_mat * y_mat * x_mat,
                            _ => return Matrix4d::identity(),
                        }
                    };
                    return result;
                }
                _ => {}
            }
        }

        // Handle Orient (quaternion)
        if op_type == XformOpType::Orient {
            let mut quat_val: Option<Quatd> = None;
            if let Some(&val) = op_val.downcast::<Quatd>() {
                quat_val = Some(val);
            } else if let Some(&val) = op_val.downcast::<Quatf>() {
                let imag = val.imaginary();
                quat_val = Some(Quatd::new(
                    val.real() as f64,
                    Vec3d::new(imag.x as f64, imag.y as f64, imag.z as f64),
                ));
            }

            if let Some(mut quat) = quat_val {
                if is_inverse_op {
                    // C++: quatRotation.GetInverse() then GfMatrix4d(quatRotation, GfVec3d(0.))
                    quat = quat.inverse();
                }
                // Build 4x4 matrix directly from quaternion + zero translation.
                // Matches C++: GfMatrix4d(GfRotation(quatVal), GfVec3d(0.))
                let mut mat = Matrix4d::identity();
                mat.set_rotate(&quat);
                return mat;
            }
        }

        // Invalid combination - return identity
        Matrix4d::identity()
    }
}

impl PartialEq for XformOp {
    fn eq(&self, other: &Self) -> bool {
        self.attr == other.attr
            && self.op_type == other.op_type
            && self.is_inverse_op == other.is_inverse_op
    }
}

impl Eq for XformOp {}

#[cfg(test)]
mod tests {
    use super::*;
    use usd_gf::{Matrix4d, Quatd, Vec3d};

    /// Verify orient value-type names match C++ SdfValueTypeName expectations.
    #[test]
    fn test_orient_value_type_name() {
        let vtn_d = XformOp::get_value_type_name(XformOpType::Orient, XformOpPrecision::Double);
        let vtn_f = XformOp::get_value_type_name(XformOpType::Orient, XformOpPrecision::Float);
        // C++ uses SdfValueTypeNames->Quatd / Quatf, token must be "quatd"/"quatf"
        assert!(
            vtn_d.as_token().as_str().to_lowercase().contains("quat"),
            "Orient/Double should map to quatd, got {}",
            vtn_d.as_token().as_str()
        );
        assert!(
            vtn_f.as_token().as_str().to_lowercase().contains("quat"),
            "Orient/Float should map to quatf, got {}",
            vtn_f.as_token().as_str()
        );
    }

    /// Verify that an identity quaternion produces an identity transform matrix.
    #[test]
    fn test_orient_identity_quat_gives_identity_matrix() {
        let identity_quat = Quatd::identity();
        // Quatd doesn't implement Hash so use Value::from (impl_value_from_no_hash)
        let val = Value::from(identity_quat);
        let mat = XformOp::get_op_transform_static(XformOpType::Orient, &val, false);

        // Upper-left 3x3 of an identity quat rotation should be identity.
        let expected = Matrix4d::identity();
        for row in 0..3 {
            for col in 0..3 {
                let diff = (mat[row][col] - expected[row][col]).abs();
                assert!(
                    diff < 1e-9,
                    "mat[{}][{}] = {}, expected {}",
                    row,
                    col,
                    mat[row][col],
                    expected[row][col]
                );
            }
        }
    }

    /// Verify that inverse orient quaternion gives the transposed rotation.
    #[test]
    fn test_orient_inverse() {
        // 90 degree rotation about Z axis: q = (cos45, 0, 0, sin45)
        let half = std::f64::consts::FRAC_PI_4;
        let quat = Quatd::new(half.cos(), Vec3d::new(0.0, 0.0, half.sin()));
        // Quatd doesn't implement Hash so use Value::from
        let val = Value::from(quat);

        let mat_fwd = XformOp::get_op_transform_static(XformOpType::Orient, &val, false);
        let mat_inv = XformOp::get_op_transform_static(XformOpType::Orient, &val, true);

        // Forward * Inverse should be identity
        let product = mat_fwd * mat_inv;
        for row in 0..3 {
            for col in 0..3 {
                let expected = if row == col { 1.0 } else { 0.0 };
                let diff = (product[row][col] - expected).abs();
                assert!(
                    diff < 1e-9,
                    "product[{}][{}] = {}, expected {}",
                    row,
                    col,
                    product[row][col],
                    expected
                );
            }
        }
    }

    /// Verify orient precision -> value-type name for Double and Float precisions.
    #[test]
    fn test_orient_all_precisions() {
        // quatd and quatf are registered in the ValueTypeRegistry.
        let vtn_d = XformOp::get_value_type_name(XformOpType::Orient, XformOpPrecision::Double);
        let vtn_f = XformOp::get_value_type_name(XformOpType::Orient, XformOpPrecision::Float);
        assert_eq!(
            vtn_d.as_token().as_str(),
            "quatd",
            "Orient/Double should map to quatd"
        );
        assert_eq!(
            vtn_f.as_token().as_str(),
            "quatf",
            "Orient/Float should map to quatf"
        );
        // Note: quath maps to the correct token string but may not be registered
        // in the ValueTypeRegistry in all configurations (half-float is uncommon).
        let vtn_h = XformOp::get_value_type_name(XformOpType::Orient, XformOpPrecision::Half);
        let h_token = vtn_h.as_token();
        // If registered, must be "quath"; if not registered, is an empty/invalid token.
        assert!(
            h_token.is_empty() || h_token == "quath",
            "Orient/Half should map to 'quath' or be unregistered, got '{}'",
            h_token.as_str()
        );
    }
}
