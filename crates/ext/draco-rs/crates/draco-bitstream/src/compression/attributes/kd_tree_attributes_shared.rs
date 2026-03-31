//! Shared constants for kd-tree attribute encoding.
//! Reference: `_ref/draco/src/draco/compression/attributes/kd_tree_attributes_shared.h`.

/// Defines types of kD-tree compression (legacy bitstreams).
#[repr(i8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KdTreeAttributesEncodingMethod {
    KdTreeQuantizationEncoding = 0,
    KdTreeIntegerEncoding = 1,
}
