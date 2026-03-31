//! D-dimensional vector class and helpers.
//! Reference: `_ref/draco/src/draco/core/vector_d.h`.

use std::cmp::Ordering;
use std::fmt;
use std::ops::{Add, AddAssign, Div, Index, IndexMut, Mul, MulAssign, Neg, Sub, SubAssign};

/// Trait for absolute value and max value used by `abs_sum`.
pub trait DracoAbs: Copy + PartialOrd + Add<Output = Self> + Sub<Output = Self> {
    const MAX: Self;
    fn abs(self) -> Self;
}

/// Convert numeric types into f64 for generic conversions.
pub trait DracoToF64: Copy {
    fn to_f64(self) -> f64;
}

/// Convert f64 into numeric types for generic conversions.
pub trait DracoFromF64: Copy + Sized {
    fn from_f64(v: f64) -> Self;
}

/// Convert numeric types into f32 (for C++ float parity in interpolation).
pub trait DracoToF32: Copy {
    fn to_f32(self) -> f32;
}

/// Convert f32 into numeric types (for C++ float parity in interpolation).
pub trait DracoFromF32: Copy + Sized {
    fn from_f32(v: f32) -> Self;
}

macro_rules! impl_draco_abs_signed {
    ($t:ty) => {
        impl DracoAbs for $t {
            const MAX: Self = <$t>::MAX;
            #[inline]
            fn abs(self) -> Self {
                self.abs()
            }
        }
    };
}

macro_rules! impl_draco_abs_unsigned {
    ($t:ty) => {
        impl DracoAbs for $t {
            const MAX: Self = <$t>::MAX;
            #[inline]
            fn abs(self) -> Self {
                self
            }
        }
    };
}

impl_draco_abs_signed!(f32);
impl_draco_abs_signed!(f64);
impl_draco_abs_signed!(i8);
impl_draco_abs_signed!(i16);
impl_draco_abs_signed!(i32);
impl_draco_abs_signed!(i64);
impl_draco_abs_signed!(isize);
impl_draco_abs_unsigned!(u8);
impl_draco_abs_unsigned!(u16);
impl_draco_abs_unsigned!(u32);
impl_draco_abs_unsigned!(u64);
impl_draco_abs_unsigned!(usize);

macro_rules! impl_draco_to_from_f64_signed {
    ($t:ty) => {
        impl DracoToF64 for $t {
            fn to_f64(self) -> f64 {
                self as f64
            }
        }
        impl DracoFromF64 for $t {
            fn from_f64(v: f64) -> Self {
                v as $t
            }
        }
    };
}

macro_rules! impl_draco_to_from_f64_unsigned {
    ($t:ty) => {
        impl DracoToF64 for $t {
            fn to_f64(self) -> f64 {
                self as f64
            }
        }
        impl DracoFromF64 for $t {
            fn from_f64(v: f64) -> Self {
                v as $t
            }
        }
    };
}

impl_draco_to_from_f64_signed!(f32);
impl_draco_to_from_f64_signed!(f64);
impl_draco_to_from_f64_signed!(i8);
impl_draco_to_from_f64_signed!(i16);
impl_draco_to_from_f64_signed!(i32);
impl_draco_to_from_f64_signed!(i64);
impl_draco_to_from_f64_signed!(isize);
impl_draco_to_from_f64_unsigned!(u8);
impl_draco_to_from_f64_unsigned!(u16);
impl_draco_to_from_f64_unsigned!(u32);
impl_draco_to_from_f64_unsigned!(u64);
impl_draco_to_from_f64_unsigned!(usize);

macro_rules! impl_draco_to_from_f32_signed {
    ($t:ty) => {
        impl DracoToF32 for $t {
            fn to_f32(self) -> f32 {
                self as f32
            }
        }
        impl DracoFromF32 for $t {
            fn from_f32(v: f32) -> Self {
                v as $t
            }
        }
    };
}
macro_rules! impl_draco_to_from_f32_unsigned {
    ($t:ty) => {
        impl DracoToF32 for $t {
            fn to_f32(self) -> f32 {
                self as f32
            }
        }
        impl DracoFromF32 for $t {
            fn from_f32(v: f32) -> Self {
                v as $t
            }
        }
    };
}
impl_draco_to_from_f32_signed!(f32);
impl_draco_to_from_f32_signed!(f64);
impl_draco_to_from_f32_signed!(i8);
impl_draco_to_from_f32_signed!(i16);
impl_draco_to_from_f32_signed!(i32);
impl_draco_to_from_f32_signed!(i64);
impl_draco_to_from_f32_signed!(isize);
impl_draco_to_from_f32_unsigned!(u8);
impl_draco_to_from_f32_unsigned!(u16);
impl_draco_to_from_f32_unsigned!(u32);
impl_draco_to_from_f32_unsigned!(u64);
impl_draco_to_from_f32_unsigned!(usize);

impl DracoToF64 for bool {
    fn to_f64(self) -> f64 {
        if self {
            1.0
        } else {
            0.0
        }
    }
}
impl DracoFromF64 for bool {
    fn from_f64(v: f64) -> Self {
        v != 0.0
    }
}

/// Trait for normalization (sqrt) used by `normalize`.
pub trait DracoFloat:
    Copy + PartialEq + Div<Output = Self> + Mul<Output = Self> + Add<Output = Self>
{
    fn sqrt(self) -> Self;
    fn zero() -> Self;
}

impl DracoFloat for f32 {
    #[inline]
    fn sqrt(self) -> Self {
        f32::sqrt(self)
    }
    #[inline]
    fn zero() -> Self {
        0.0
    }
}

impl DracoFloat for f64 {
    #[inline]
    fn sqrt(self) -> Self {
        f64::sqrt(self)
    }
    #[inline]
    fn zero() -> Self {
        0.0
    }
}

/// Marker trait for signed scalar types used by cross product.
pub trait DracoSignedScalar: Copy {}
impl DracoSignedScalar for f32 {}
impl DracoSignedScalar for f64 {}
impl DracoSignedScalar for i8 {}
impl DracoSignedScalar for i16 {}
impl DracoSignedScalar for i32 {}
impl DracoSignedScalar for i64 {}
impl DracoSignedScalar for isize {}

/// D-dimensional vector class with basic operations.
#[derive(Clone, Copy, Debug)]
pub struct VectorD<Scalar, const N: usize> {
    v: [Scalar; N],
}

impl<Scalar: Copy + Default, const N: usize> Default for VectorD<Scalar, N> {
    fn default() -> Self {
        Self {
            v: [Scalar::default(); N],
        }
    }
}

impl<Scalar: Copy + Default, const N: usize> VectorD<Scalar, N> {
    pub const DIMENSION: usize = N;

    /// Constructs from a 2D vector. Debug-asserts that `N == 2`.
    pub fn new2(c0: Scalar, c1: Scalar) -> Self {
        debug_assert_eq!(N, 2);
        let mut v = [Scalar::default(); N];
        v[0] = c0;
        v[1] = c1;
        Self { v }
    }

    /// Constructs from a 3D vector. Debug-asserts that `N == 3`.
    pub fn new3(c0: Scalar, c1: Scalar, c2: Scalar) -> Self {
        debug_assert_eq!(N, 3);
        let mut v = [Scalar::default(); N];
        v[0] = c0;
        v[1] = c1;
        v[2] = c2;
        Self { v }
    }

    /// Constructs from a 4D vector. Debug-asserts that `N == 4`.
    pub fn new4(c0: Scalar, c1: Scalar, c2: Scalar, c3: Scalar) -> Self {
        debug_assert_eq!(N, 4);
        let mut v = [Scalar::default(); N];
        v[0] = c0;
        v[1] = c1;
        v[2] = c2;
        v[3] = c3;
        Self { v }
    }

    /// Constructs from a 5D vector. Debug-asserts that `N == 5`.
    pub fn new5(c0: Scalar, c1: Scalar, c2: Scalar, c3: Scalar, c4: Scalar) -> Self {
        debug_assert_eq!(N, 5);
        let mut v = [Scalar::default(); N];
        v[0] = c0;
        v[1] = c1;
        v[2] = c2;
        v[3] = c3;
        v[4] = c4;
        Self { v }
    }

    /// Constructs from a 6D vector. Debug-asserts that `N == 6`.
    pub fn new6(c0: Scalar, c1: Scalar, c2: Scalar, c3: Scalar, c4: Scalar, c5: Scalar) -> Self {
        debug_assert_eq!(N, 6);
        let mut v = [Scalar::default(); N];
        v[0] = c0;
        v[1] = c1;
        v[2] = c2;
        v[3] = c3;
        v[4] = c4;
        v[5] = c5;
        Self { v }
    }

    /// Constructs from a 7D vector. Debug-asserts that `N == 7`.
    pub fn new7(
        c0: Scalar,
        c1: Scalar,
        c2: Scalar,
        c3: Scalar,
        c4: Scalar,
        c5: Scalar,
        c6: Scalar,
    ) -> Self {
        debug_assert_eq!(N, 7);
        let mut v = [Scalar::default(); N];
        v[0] = c0;
        v[1] = c1;
        v[2] = c2;
        v[3] = c3;
        v[4] = c4;
        v[5] = c5;
        v[6] = c6;
        Self { v }
    }

    /// Constructs the vector from another vector with a different scalar type
    /// or dimension. Extra components are truncated; missing components are 0.
    pub fn from_vector<OtherScalar, const M: usize>(src: VectorD<OtherScalar, M>) -> Self
    where
        Scalar: DracoFromF64 + Default + Copy,
        OtherScalar: DracoToF64 + Copy,
    {
        let mut v = [Scalar::default(); N];
        let limit = if M < N { M } else { N };
        for i in 0..limit {
            v[i] = Scalar::from_f64(src[i].to_f64());
        }
        Self { v }
    }

    /// Returns a pointer to the underlying data.
    pub fn data(&self) -> *const Scalar {
        self.v.as_ptr()
    }

    /// Returns a mutable pointer to the underlying data.
    pub fn data_mut(&mut self) -> *mut Scalar {
        self.v.as_mut_ptr()
    }
}

impl<Scalar, const N: usize> Index<usize> for VectorD<Scalar, N> {
    type Output = Scalar;
    fn index(&self, index: usize) -> &Self::Output {
        &self.v[index]
    }
}

impl<Scalar, const N: usize> IndexMut<usize> for VectorD<Scalar, N> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.v[index]
    }
}

impl<Scalar: Copy + Neg<Output = Scalar>, const N: usize> Neg for VectorD<Scalar, N> {
    type Output = Self;
    fn neg(self) -> Self::Output {
        let mut out = self;
        for i in 0..N {
            out[i] = -out[i];
        }
        out
    }
}

impl<Scalar: Copy + Add<Output = Scalar>, const N: usize> Add for VectorD<Scalar, N> {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        let mut out = self;
        for i in 0..N {
            out[i] = out[i] + rhs[i];
        }
        out
    }
}

impl<Scalar: Copy + Sub<Output = Scalar>, const N: usize> Sub for VectorD<Scalar, N> {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output {
        let mut out = self;
        for i in 0..N {
            out[i] = out[i] - rhs[i];
        }
        out
    }
}

impl<Scalar: Copy + Mul<Output = Scalar>, const N: usize> Mul for VectorD<Scalar, N> {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self::Output {
        let mut out = self;
        for i in 0..N {
            out[i] = out[i] * rhs[i];
        }
        out
    }
}

impl<Scalar: Copy + AddAssign, const N: usize> AddAssign for VectorD<Scalar, N> {
    fn add_assign(&mut self, rhs: Self) {
        for i in 0..N {
            self[i] += rhs[i];
        }
    }
}

impl<Scalar: Copy + SubAssign, const N: usize> SubAssign for VectorD<Scalar, N> {
    fn sub_assign(&mut self, rhs: Self) {
        for i in 0..N {
            self[i] -= rhs[i];
        }
    }
}

impl<Scalar: Copy + MulAssign, const N: usize> MulAssign for VectorD<Scalar, N> {
    fn mul_assign(&mut self, rhs: Self) {
        for i in 0..N {
            self[i] *= rhs[i];
        }
    }
}

impl<Scalar: Copy + Mul<Output = Scalar>, const N: usize> Mul<Scalar> for VectorD<Scalar, N> {
    type Output = Self;
    fn mul(self, rhs: Scalar) -> Self::Output {
        let mut out = self;
        for i in 0..N {
            out[i] = out[i] * rhs;
        }
        out
    }
}

impl<Scalar: Copy + Div<Output = Scalar>, const N: usize> Div<Scalar> for VectorD<Scalar, N> {
    type Output = Self;
    fn div(self, rhs: Scalar) -> Self::Output {
        let mut out = self;
        for i in 0..N {
            out[i] = out[i] / rhs;
        }
        out
    }
}

impl<Scalar: Copy + Add<Output = Scalar>, const N: usize> Add<Scalar> for VectorD<Scalar, N> {
    type Output = Self;
    fn add(self, rhs: Scalar) -> Self::Output {
        let mut out = self;
        for i in 0..N {
            out[i] = out[i] + rhs;
        }
        out
    }
}

impl<Scalar: Copy + Sub<Output = Scalar>, const N: usize> Sub<Scalar> for VectorD<Scalar, N> {
    type Output = Self;
    fn sub(self, rhs: Scalar) -> Self::Output {
        let mut out = self;
        for i in 0..N {
            out[i] = out[i] - rhs;
        }
        out
    }
}

impl<Scalar: Copy + PartialEq, const N: usize> PartialEq for VectorD<Scalar, N> {
    fn eq(&self, other: &Self) -> bool {
        for i in 0..N {
            if self[i] != other[i] {
                return false;
            }
        }
        true
    }
}

impl<Scalar: Copy + PartialOrd, const N: usize> PartialOrd for VectorD<Scalar, N> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        if self.lex_less_than(other) {
            return Some(Ordering::Less);
        }
        if other.lex_less_than(self) {
            return Some(Ordering::Greater);
        }
        Some(Ordering::Equal)
    }
}

impl<Scalar: Copy + PartialOrd, const N: usize> VectorD<Scalar, N> {
    /// Lexicographic comparison (equivalent to C++ operator<).
    pub fn lex_less_than(&self, other: &Self) -> bool {
        if N == 0 {
            return false;
        }
        for i in 0..(N - 1) {
            if self.v[i] < other.v[i] {
                return true;
            }
            if self.v[i] > other.v[i] {
                return false;
            }
        }
        self.v[N - 1] < other.v[N - 1]
    }
}

impl<Scalar: Copy + DracoAbs, const N: usize> VectorD<Scalar, N> {
    pub fn squared_norm(&self) -> Scalar
    where
        Scalar: Add<Output = Scalar> + Mul<Output = Scalar>,
    {
        self.dot(self)
    }

    /// Computes L1, the sum of absolute values of all entries.
    pub fn abs_sum(&self) -> Scalar {
        let mut result = self.v[0].abs();
        for i in 1..N {
            let next_value = self.v[i].abs();
            if result > <Scalar as DracoAbs>::MAX - next_value {
                return <Scalar as DracoAbs>::MAX;
            }
            result = result + next_value;
        }
        result
    }

    pub fn dot(&self, other: &Self) -> Scalar
    where
        Scalar: Add<Output = Scalar> + Mul<Output = Scalar>,
    {
        let mut ret = self.v[0] * other.v[0];
        for i in 1..N {
            ret = ret + (self.v[i] * other.v[i]);
        }
        ret
    }

    pub fn max_coeff(&self) -> Scalar {
        let mut max_v = self.v[0];
        for i in 1..N {
            if self.v[i] > max_v {
                max_v = self.v[i];
            }
        }
        max_v
    }

    pub fn min_coeff(&self) -> Scalar {
        let mut min_v = self.v[0];
        for i in 1..N {
            if self.v[i] < min_v {
                min_v = self.v[i];
            }
        }
        min_v
    }
}

impl<Scalar: Copy + DracoFloat + DracoAbs, const N: usize> VectorD<Scalar, N> {
    pub fn normalize(&mut self)
    where
        Scalar: Add<Output = Scalar> + Mul<Output = Scalar>,
    {
        let magnitude = self.squared_norm().sqrt();
        if magnitude == <Scalar as DracoFloat>::zero() {
            return;
        }
        for i in 0..N {
            self.v[i] = self.v[i] / magnitude;
        }
    }

    pub fn get_normalized(&self) -> Self
    where
        Scalar: Add<Output = Scalar> + Mul<Output = Scalar>,
    {
        let mut ret = *self;
        ret.normalize();
        ret
    }
}

/// Scalar multiplication from the other side (concrete scalars to satisfy orphan rules).
macro_rules! impl_scalar_mul_vec {
    ($($t:ty),+ $(,)?) => {
        $(
            impl<const N: usize> Mul<VectorD<$t, N>> for $t {
                type Output = VectorD<$t, N>;
                fn mul(self, rhs: VectorD<$t, N>) -> Self::Output {
                    rhs * self
                }
            }
        )+
    };
}

impl_scalar_mul_vec!(f32, f64, i8, u8, i16, u16, i32, u32, i64, u64, isize, usize);

/// Calculates the squared distance between two points.
pub fn squared_distance<Scalar, const N: usize>(
    v1: &VectorD<Scalar, N>,
    v2: &VectorD<Scalar, N>,
) -> Scalar
where
    Scalar: Copy + PartialOrd + Sub<Output = Scalar> + Add<Output = Scalar> + Mul<Output = Scalar>,
{
    let mut squared_distance = if v1[0] >= v2[0] {
        let difference = v1[0] - v2[0];
        difference * difference
    } else {
        let difference = v2[0] - v1[0];
        difference * difference
    };

    for i in 1..N {
        let difference = if v1[i] >= v2[i] {
            v1[i] - v2[i]
        } else {
            v2[i] - v1[i]
        };
        squared_distance = squared_distance + (difference * difference);
    }
    squared_distance
}

/// Cross product of two 3D vectors (signed scalar types only).
pub fn cross_product<Scalar>(u: &VectorD<Scalar, 3>, v: &VectorD<Scalar, 3>) -> VectorD<Scalar, 3>
where
    Scalar: DracoSignedScalar + Sub<Output = Scalar> + Mul<Output = Scalar>,
{
    let mut r = *u;
    r[0] = (u[1] * v[2]) - (u[2] * v[1]);
    r[1] = (u[2] * v[0]) - (u[0] * v[2]);
    r[2] = (u[0] * v[1]) - (u[1] * v[0]);
    r
}

impl<Scalar: fmt::Display, const N: usize> fmt::Display for VectorD<Scalar, N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if N == 0 {
            return Ok(());
        }
        for i in 0..(N - 1) {
            write!(f, "{} ", self.v[i])?;
        }
        write!(f, "{}", self.v[N - 1])
    }
}

pub type Vector2f = VectorD<f32, 2>;
pub type Vector3f = VectorD<f32, 3>;
pub type Vector4f = VectorD<f32, 4>;
pub type Vector5f = VectorD<f32, 5>;
pub type Vector6f = VectorD<f32, 6>;
pub type Vector7f = VectorD<f32, 7>;

pub type Vector2ui = VectorD<u32, 2>;
pub type Vector3ui = VectorD<u32, 3>;
pub type Vector4ui = VectorD<u32, 4>;
pub type Vector5ui = VectorD<u32, 5>;
pub type Vector6ui = VectorD<u32, 6>;
pub type Vector7ui = VectorD<u32, 7>;
