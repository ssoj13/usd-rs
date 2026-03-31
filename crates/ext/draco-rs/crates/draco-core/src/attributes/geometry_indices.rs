//! Geometry index types.
//! Reference: `_ref/draco/src/draco/attributes/geometry_indices.h`.

use crate::core::draco_index_type::DracoIndex;

macro_rules! define_index_type_u32 {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name {
            value: u32,
        }

        impl $name {
            pub const fn new(value: u32) -> Self {
                Self { value }
            }

            pub fn value(self) -> u32 {
                self.value
            }

            pub fn value_usize(self) -> usize {
                self.value as usize
            }
        }

        impl From<u32> for $name {
            fn from(value: u32) -> Self {
                Self::new(value)
            }
        }

        impl DracoIndex for $name {
            fn to_usize(self) -> usize {
                self.value as usize
            }
        }

        impl std::ops::Add for $name {
            type Output = Self;
            fn add(self, rhs: Self) -> Self::Output {
                Self::new(self.value + rhs.value)
            }
        }

        impl std::ops::Add<u32> for $name {
            type Output = Self;
            fn add(self, rhs: u32) -> Self::Output {
                Self::new(self.value + rhs)
            }
        }

        impl std::ops::Sub for $name {
            type Output = Self;
            fn sub(self, rhs: Self) -> Self::Output {
                Self::new(self.value - rhs.value)
            }
        }

        impl std::ops::Sub<u32> for $name {
            type Output = Self;
            fn sub(self, rhs: u32) -> Self::Output {
                Self::new(self.value - rhs)
            }
        }

        impl std::ops::AddAssign for $name {
            fn add_assign(&mut self, rhs: Self) {
                self.value += rhs.value;
            }
        }

        impl std::ops::AddAssign<u32> for $name {
            fn add_assign(&mut self, rhs: u32) {
                self.value += rhs;
            }
        }

        impl std::ops::SubAssign for $name {
            fn sub_assign(&mut self, rhs: Self) {
                self.value -= rhs.value;
            }
        }

        impl std::ops::SubAssign<u32> for $name {
            fn sub_assign(&mut self, rhs: u32) {
                self.value -= rhs;
            }
        }
    };
}

define_index_type_u32!(AttributeValueIndex);
define_index_type_u32!(PointIndex);
define_index_type_u32!(VertexIndex);
define_index_type_u32!(CornerIndex);
define_index_type_u32!(FaceIndex);

pub const INVALID_ATTRIBUTE_VALUE_INDEX: AttributeValueIndex = AttributeValueIndex::new(u32::MAX);
pub const INVALID_POINT_INDEX: PointIndex = PointIndex::new(u32::MAX);
pub const INVALID_VERTEX_INDEX: VertexIndex = VertexIndex::new(u32::MAX);
pub const INVALID_CORNER_INDEX: CornerIndex = CornerIndex::new(u32::MAX);
pub const INVALID_FACE_INDEX: FaceIndex = FaceIndex::new(u32::MAX);
