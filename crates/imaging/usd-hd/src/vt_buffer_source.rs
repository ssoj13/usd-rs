
//! VtValue-backed buffer source.
//!
//! Corresponds to pxr/imaging/hd/vtBufferSource.h.
//! Buffer source where data comes from a Value (VtValue equivalent).

use crate::resource::buffer_source::{
    HdBufferSource, HdBufferSourceBase, HdBufferSourceState, HdResolvedBufferSource,
};
use crate::resource::buffer_spec::{HdBufferSpec, HdBufferSpecVector};
use crate::types::{HdTupleType, HdType};
use usd_gf::matrix4::{Matrix4d, Matrix4f};
use usd_tf::Token;
use usd_vt::Value;

/// Returns raw pointer to Value's data for Hd-compatible types.
///
/// Corresponds to C++ `HdGetValueData`. Returns null if type is not supported.
pub fn hd_get_value_data(value: &Value) -> *const u8 {
    macro_rules! try_ptr {
        ($t:ty) => {
            if let Some(r) = value.get::<$t>() {
                return (r as *const $t).cast::<u8>();
            }
        };
    }
    try_ptr!(f32);
    try_ptr!(f64);
    try_ptr!(i32);
    try_ptr!(u32);
    try_ptr!(i16);
    try_ptr!(u16);
    try_ptr!(i8);
    try_ptr!(u8);
    try_ptr!(bool);
    try_ptr!([f32; 2]);
    try_ptr!([f32; 3]);
    try_ptr!([f32; 4]);
    try_ptr!([f64; 2]);
    try_ptr!([f64; 3]);
    try_ptr!([f64; 4]);
    try_ptr!(Vec<f32>);
    try_ptr!(Vec<[f32; 2]>);
    try_ptr!(Vec<[f32; 3]>);
    try_ptr!(Vec<[f32; 4]>);
    try_ptr!(Vec<Matrix4d>);
    try_ptr!(Vec<Matrix4f>);
    try_ptr!(Matrix4d);
    try_ptr!(Matrix4f);
    std::ptr::null()
}

/// Returns HdTupleType for Value's held type.
///
/// Corresponds to C++ `HdGetValueTupleType`. Returns Invalid type if unsupported.
pub fn hd_get_value_tuple_type(value: &Value) -> HdTupleType {
    macro_rules! try_tuple {
        ($t:ty, $hd:expr, $count:expr) => {
            if value.is::<$t>() {
                return HdTupleType::new($hd, $count);
            }
        };
    }
    try_tuple!(f32, HdType::Float, 1);
    try_tuple!(f64, HdType::Double, 1);
    try_tuple!(i32, HdType::Int32, 1);
    try_tuple!(u32, HdType::UInt32, 1);
    try_tuple!(i16, HdType::Int16, 1);
    try_tuple!(u16, HdType::UInt16, 1);
    try_tuple!(i8, HdType::Int8, 1);
    try_tuple!(u8, HdType::UInt8, 1);
    try_tuple!(bool, HdType::Bool, 1);
    try_tuple!([f32; 2], HdType::FloatVec2, 1);
    try_tuple!([f32; 3], HdType::FloatVec3, 1);
    try_tuple!([f32; 4], HdType::FloatVec4, 1);
    try_tuple!([f64; 2], HdType::DoubleVec2, 1);
    try_tuple!([f64; 3], HdType::DoubleVec3, 1);
    try_tuple!([f64; 4], HdType::DoubleVec4, 1);
    try_tuple!(Matrix4d, HdType::DoubleMat4, 1);
    try_tuple!(Matrix4f, HdType::FloatMat4, 1);
    if value.is::<Vec<f32>>() {
        return HdTupleType::new(HdType::Float, 1);
    }
    if value.is::<Vec<[f32; 2]>>() {
        return HdTupleType::new(HdType::FloatVec2, 1);
    }
    if value.is::<Vec<[f32; 3]>>() {
        return HdTupleType::new(HdType::FloatVec3, 1);
    }
    if value.is::<Vec<[f32; 4]>>() {
        return HdTupleType::new(HdType::FloatVec4, 1);
    }
    if value.is::<Vec<Matrix4d>>() {
        return HdTupleType::new(HdType::DoubleMat4, 1);
    }
    if value.is::<Vec<Matrix4f>>() {
        return HdTupleType::new(HdType::FloatMat4, 1);
    }
    HdTupleType::new(HdType::Invalid, 0)
}

/// Default matrix type for GPU (FloatMat4 unless double matrix enabled).
#[inline]
pub fn hd_get_default_matrix_type() -> HdType {
    HdType::FloatMat4
}

/// VtValue-backed buffer source.
///
/// Corresponds to C++ `HdVtBufferSource`.
/// Implements HdResolvedBufferSource for data that needs no preprocessing.
pub struct HdVtBufferSource {
    base: HdBufferSourceBase,
    name: Token,
    value: Value,
    tuple_type: HdTupleType,
    num_elements: usize,
}

impl HdVtBufferSource {
    /// Create from Value. array_size is per-element component count (default 1).
    /// allow_doubles: if false, double types are converted to float.
    pub fn new(name: Token, value: Value, array_size: usize, allow_doubles: bool) -> Self {
        let (value, tuple_type, num_elements) = Self::set_value(&value, array_size, allow_doubles);
        Self {
            base: HdBufferSourceBase::new(),
            name,
            value,
            tuple_type,
            num_elements,
        }
    }

    /// Create from a single matrix.
    pub fn from_matrix(name: Token, matrix: Matrix4d, allow_doubles: bool) -> Self {
        let tuple_type = if allow_doubles {
            HdTupleType::new(HdType::DoubleMat4, 1)
        } else {
            HdTupleType::new(HdType::FloatMat4, 1)
        };
        let value = if allow_doubles {
            Value::from_no_hash(matrix)
        } else {
            let mf = Matrix4f::new(
                matrix[0][0] as f32,
                matrix[0][1] as f32,
                matrix[0][2] as f32,
                matrix[0][3] as f32,
                matrix[1][0] as f32,
                matrix[1][1] as f32,
                matrix[1][2] as f32,
                matrix[1][3] as f32,
                matrix[2][0] as f32,
                matrix[2][1] as f32,
                matrix[2][2] as f32,
                matrix[2][3] as f32,
                matrix[3][0] as f32,
                matrix[3][1] as f32,
                matrix[3][2] as f32,
                matrix[3][3] as f32,
            );
            Value::from_no_hash(mf)
        };
        let num_elements = 1;
        Self {
            base: HdBufferSourceBase::new(),
            name,
            value,
            tuple_type,
            num_elements,
        }
    }

    /// Create from matrix array.
    pub fn from_matrices(
        name: Token,
        matrices: Vec<Matrix4d>,
        array_size: usize,
        allow_doubles: bool,
    ) -> Self {
        let tuple_type = if allow_doubles {
            HdTupleType::new(HdType::DoubleMat4, array_size.max(1))
        } else {
            HdTupleType::new(HdType::FloatMat4, array_size.max(1))
        };
        let value = if allow_doubles {
            Value::from_no_hash(matrices)
        } else {
            let mf: Vec<Matrix4f> = matrices
                .into_iter()
                .map(|m| {
                    Matrix4f::new(
                        m[0][0] as f32,
                        m[0][1] as f32,
                        m[0][2] as f32,
                        m[0][3] as f32,
                        m[1][0] as f32,
                        m[1][1] as f32,
                        m[1][2] as f32,
                        m[1][3] as f32,
                        m[2][0] as f32,
                        m[2][1] as f32,
                        m[2][2] as f32,
                        m[2][3] as f32,
                        m[3][0] as f32,
                        m[3][1] as f32,
                        m[3][2] as f32,
                        m[3][3] as f32,
                    )
                })
                .collect();
            Value::from_no_hash(mf)
        };
        let num_elements = Self::compute_num_elements(&value);
        Self {
            base: HdBufferSourceBase::new(),
            name,
            value,
            tuple_type,
            num_elements,
        }
    }

    fn set_value(
        value: &Value,
        array_size: usize,
        _allow_doubles: bool,
    ) -> (Value, HdTupleType, usize) {
        let mut tuple_type = hd_get_value_tuple_type(value);
        if tuple_type.type_ == HdType::Invalid {
            return (Value::empty(), HdTupleType::new(HdType::Invalid, 0), 0);
        }
        if array_size > 1 {
            tuple_type.count = array_size;
        }
        let num_elements = if value.is_empty() {
            0
        } else if value.is_array_valued() {
            Self::compute_num_elements(value)
        } else {
            1
        };
        let value = value.clone();
        (value, tuple_type, num_elements)
    }

    fn compute_num_elements(value: &Value) -> usize {
        if value.is::<Vec<f32>>() {
            value.get::<Vec<f32>>().map(|v| v.len()).unwrap_or(0)
        } else if value.is::<Vec<[f32; 2]>>() {
            value.get::<Vec<[f32; 2]>>().map(|v| v.len()).unwrap_or(0)
        } else if value.is::<Vec<[f32; 3]>>() {
            value.get::<Vec<[f32; 3]>>().map(|v| v.len()).unwrap_or(0)
        } else if value.is::<Vec<[f32; 4]>>() {
            value.get::<Vec<[f32; 4]>>().map(|v| v.len()).unwrap_or(0)
        } else if value.is::<Vec<Matrix4d>>() {
            value.get::<Vec<Matrix4d>>().map(|v| v.len()).unwrap_or(0)
        } else if value.is::<Vec<Matrix4f>>() {
            value.get::<Vec<Matrix4f>>().map(|v| v.len()).unwrap_or(0)
        } else {
            0
        }
    }

    /// Truncate to given number of elements.
    pub fn truncate(&mut self, num_elements: usize) {
        if num_elements <= self.num_elements {
            self.num_elements = num_elements;
        }
    }

    /// Get underlying data pointer (for Debug).
    pub fn get_data_ptr(&self) -> *const u8 {
        hd_get_value_data(&self.value)
    }
}

impl HdBufferSource for HdVtBufferSource {
    fn get_name(&self) -> &Token {
        &self.name
    }

    fn add_buffer_specs(&self, specs: &mut HdBufferSpecVector) {
        specs.push(HdBufferSpec::new(self.name.clone(), self.tuple_type));
    }

    fn resolve(&self) -> bool {
        if !self.try_lock() {
            return false;
        }
        self.set_resolved();
        true
    }

    fn get_data(&self) -> Option<*const u8> {
        if self.base.get_state() != HdBufferSourceState::Resolved {
            return None;
        }
        let ptr = hd_get_value_data(&self.value);
        if ptr.is_null() { None } else { Some(ptr) }
    }

    fn get_tuple_type(&self) -> HdTupleType {
        self.tuple_type
    }

    fn get_num_elements(&self) -> usize {
        self.num_elements
    }

    fn get_state(&self) -> HdBufferSourceState {
        self.base.get_state()
    }

    fn set_state(&self, state: HdBufferSourceState) {
        self.base.set_state(state);
    }

    fn get_state_atomic(&self) -> &std::sync::atomic::AtomicU8 {
        self.base.state_atomic()
    }

    fn check_valid(&self) -> bool {
        !self.value.is_empty() && hd_get_value_data(&self.value) != std::ptr::null()
    }
}

impl HdResolvedBufferSource for HdVtBufferSource {}
