//! Strongly typed index utilities.
//! Reference: `_ref/draco/src/draco/core/draco_index_type.h`.

use std::fmt;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct IndexType<ValueTypeT, TagT> {
    value: ValueTypeT,
    _tag: PhantomData<TagT>,
}

pub trait DracoIndex {
    fn to_usize(self) -> usize;
}

impl<ValueTypeT: Copy + Default, TagT> IndexType<ValueTypeT, TagT> {
    pub fn new(value: ValueTypeT) -> Self {
        Self {
            value,
            _tag: PhantomData,
        }
    }

    pub fn value(self) -> ValueTypeT {
        self.value
    }
}

impl<ValueTypeT, TagT> DracoIndex for IndexType<ValueTypeT, TagT>
where
    ValueTypeT: Copy + Into<usize>,
{
    fn to_usize(self) -> usize {
        self.value.into()
    }
}

impl<ValueTypeT: Copy + Default, TagT> From<ValueTypeT> for IndexType<ValueTypeT, TagT> {
    fn from(value: ValueTypeT) -> Self {
        Self::new(value)
    }
}

impl<ValueTypeT: Copy + Default, TagT> std::ops::Add for IndexType<ValueTypeT, TagT>
where
    ValueTypeT: std::ops::Add<Output = ValueTypeT>,
{
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        Self::new(self.value + rhs.value)
    }
}

impl<ValueTypeT: Copy + Default, TagT> std::ops::Add<ValueTypeT> for IndexType<ValueTypeT, TagT>
where
    ValueTypeT: std::ops::Add<Output = ValueTypeT>,
{
    type Output = Self;
    fn add(self, rhs: ValueTypeT) -> Self::Output {
        Self::new(self.value + rhs)
    }
}

impl<ValueTypeT: Copy + Default, TagT> std::ops::Sub for IndexType<ValueTypeT, TagT>
where
    ValueTypeT: std::ops::Sub<Output = ValueTypeT>,
{
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output {
        Self::new(self.value - rhs.value)
    }
}

impl<ValueTypeT: Copy + Default, TagT> std::ops::Sub<ValueTypeT> for IndexType<ValueTypeT, TagT>
where
    ValueTypeT: std::ops::Sub<Output = ValueTypeT>,
{
    type Output = Self;
    fn sub(self, rhs: ValueTypeT) -> Self::Output {
        Self::new(self.value - rhs)
    }
}

impl<ValueTypeT: Copy + Default, TagT> std::ops::AddAssign for IndexType<ValueTypeT, TagT>
where
    ValueTypeT: std::ops::AddAssign,
{
    fn add_assign(&mut self, rhs: Self) {
        self.value += rhs.value;
    }
}

impl<ValueTypeT: Copy + Default, TagT> std::ops::SubAssign for IndexType<ValueTypeT, TagT>
where
    ValueTypeT: std::ops::SubAssign,
{
    fn sub_assign(&mut self, rhs: Self) {
        self.value -= rhs.value;
    }
}

impl<ValueTypeT: Copy + Default, TagT> std::ops::AddAssign<ValueTypeT>
    for IndexType<ValueTypeT, TagT>
where
    ValueTypeT: std::ops::AddAssign,
{
    fn add_assign(&mut self, rhs: ValueTypeT) {
        self.value += rhs;
    }
}

impl<ValueTypeT: Copy + Default, TagT> std::ops::SubAssign<ValueTypeT>
    for IndexType<ValueTypeT, TagT>
where
    ValueTypeT: std::ops::SubAssign,
{
    fn sub_assign(&mut self, rhs: ValueTypeT) {
        self.value -= rhs;
    }
}

impl<ValueTypeT: Copy + Default + fmt::Display, TagT> fmt::Display for IndexType<ValueTypeT, TagT> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.value)
    }
}

impl<ValueTypeT: Copy + Default + Hash, TagT> Hash for IndexType<ValueTypeT, TagT> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.value.hash(state);
    }
}

#[macro_export]
macro_rules! define_new_draco_index_type {
    ($value_type:ty, $name:ident) => {
        #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
        pub struct $name {
            value: $value_type,
        }

        impl $name {
            pub const fn new(value: $value_type) -> Self {
                Self { value }
            }

            pub fn value(self) -> $value_type {
                self.value
            }
        }

        impl From<$value_type> for $name {
            fn from(value: $value_type) -> Self {
                Self::new(value)
            }
        }

        impl $crate::core::draco_index_type::DracoIndex for $name
        where
            $value_type: Copy + Into<usize>,
        {
            fn to_usize(self) -> usize {
                self.value.into()
            }
        }

        impl std::ops::Add for $name
        where
            $value_type: std::ops::Add<Output = $value_type>,
        {
            type Output = Self;
            fn add(self, rhs: Self) -> Self::Output {
                Self::new(self.value + rhs.value)
            }
        }

        impl std::ops::Add<$value_type> for $name
        where
            $value_type: std::ops::Add<Output = $value_type>,
        {
            type Output = Self;
            fn add(self, rhs: $value_type) -> Self::Output {
                Self::new(self.value + rhs)
            }
        }

        impl std::ops::Sub for $name
        where
            $value_type: std::ops::Sub<Output = $value_type>,
        {
            type Output = Self;
            fn sub(self, rhs: Self) -> Self::Output {
                Self::new(self.value - rhs.value)
            }
        }

        impl std::ops::Sub<$value_type> for $name
        where
            $value_type: std::ops::Sub<Output = $value_type>,
        {
            type Output = Self;
            fn sub(self, rhs: $value_type) -> Self::Output {
                Self::new(self.value - rhs)
            }
        }

        impl std::ops::AddAssign for $name
        where
            $value_type: std::ops::AddAssign,
        {
            fn add_assign(&mut self, rhs: Self) {
                self.value += rhs.value;
            }
        }

        impl std::ops::AddAssign<$value_type> for $name
        where
            $value_type: std::ops::AddAssign,
        {
            fn add_assign(&mut self, rhs: $value_type) {
                self.value += rhs;
            }
        }

        impl std::ops::SubAssign for $name
        where
            $value_type: std::ops::SubAssign,
        {
            fn sub_assign(&mut self, rhs: Self) {
                self.value -= rhs.value;
            }
        }

        impl std::ops::SubAssign<$value_type> for $name
        where
            $value_type: std::ops::SubAssign,
        {
            fn sub_assign(&mut self, rhs: $value_type) {
                self.value -= rhs;
            }
        }
    };
}
