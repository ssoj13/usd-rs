//! Attribute transform type.
//! Reference: `_ref/draco/src/draco/attributes/attribute_transform_type.h`.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum AttributeTransformType {
    InvalidTransform = -1,
    NoTransform = 0,
    QuantizationTransform = 1,
    OctahedronTransform = 2,
}

impl Default for AttributeTransformType {
    fn default() -> Self {
        Self::InvalidTransform
    }
}
