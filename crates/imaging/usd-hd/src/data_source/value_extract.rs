//! Value extraction trait and typed adapter for Hydra data sources.
//!
//! # Problem
//!
//! In C++, `HdTypedSampledDataSource<T>` is a virtual interface. Any data source
//! implementing `GetTypedValue()` works through vtable dispatch — including
//! `UsdImagingDataSourceAttribute<T>`, `HdRetainedTypedSampledDataSource<T>`, etc.
//! Schema accessors like `HdMeshTopologySchema::GetFaceVertexCounts()` use
//! `dynamic_pointer_cast` to obtain `HdTypedSampledDataSource<VtIntArray>::Handle`
//! from `HdDataSourceBaseHandle`.
//!
//! In Rust, trait objects don't support cross-trait downcasting. We can't cast
//! `dyn HdDataSourceBase` → `dyn HdTypedSampledDataSource<T>`. This breaks
//! typed schema accessors for any non-retained data source (e.g. attribute-backed
//! data sources from usd-imaging).
//!
//! # Solution
//!
//! Two components:
//!
//! - [`HdValueExtract`] — trait for extracting a concrete type `T` from an untyped
//!   [`Value`]. This is the Rust analogue of C++ `VtValue::Get<T>()`.
//!
//! - [`SampledToTypedAdapter`] — wraps any [`HdSampledDataSource`] and exposes it
//!   as [`HdTypedSampledDataSource<T>`] by extracting `T` from `Value` via
//!   `HdValueExtract`. This is the Rust analogue of C++ `dynamic_pointer_cast`.
//!
//! Together, these let `HdSchema::get_typed_retained()` fall back to the untyped
//! sampled path when the concrete-type downcast fails — making typed schema
//! accessors work uniformly for all data source implementations.
//!
//! # Performance
//!
//! The adapter adds no real overhead: our USD attribute system returns `Value`
//! (untyped) regardless, so the extraction cost is identical to what a direct
//! `DataSourceAttribute<T>::get_typed_value()` would do.

use super::base::{HdDataSourceBase, HdDataSourceBaseHandle};
use super::sampled::{HdSampledDataSource, HdSampledDataSourceTime};
use super::typed::HdTypedSampledDataSource;
use std::fmt;
use std::marker::PhantomData;
use std::sync::Arc;
use usd_gf::{
    Matrix4, Matrix4f, Quatf, Vec2d, Vec2f, Vec2i, Vec3d, Vec3f, Vec3h, Vec3i, Vec4f, Vec4i,
};
use usd_tf::Token;
use usd_vt::{Array, Value};

// ---------------------------------------------------------------------------
// HdValueExtract
// ---------------------------------------------------------------------------

/// Trait for extracting a concrete type from an untyped [`Value`].
///
/// In C++, `VtValue::Get<T>()` and `UsdAttribute::Get<T>()` are templates
/// that return the typed value directly. In Rust, attribute access always
/// returns `Value`, so we need this trait to recover the concrete type.
///
/// Implementations handle multiple storage representations that may appear
/// at runtime (e.g. `Array<i32>`, `Vec<i32>`, or `Vec<Value>` for int arrays).
pub trait HdValueExtract: Clone + Default + Send + Sync + 'static {
    /// Attempt to extract `Self` from an untyped `Value`.
    ///
    /// Returns `None` if the value cannot be converted to the target type.
    fn extract(value: &Value) -> Option<Self>;
}

// --- Scalar implementations ------------------------------------------------

impl HdValueExtract for bool {
    fn extract(value: &Value) -> Option<Self> {
        value.get::<bool>().copied()
    }
}

impl HdValueExtract for i32 {
    fn extract(value: &Value) -> Option<Self> {
        value.get::<i32>().copied()
    }
}

impl HdValueExtract for i64 {
    fn extract(value: &Value) -> Option<Self> {
        value.get::<i64>().copied()
    }
}

impl HdValueExtract for u32 {
    fn extract(value: &Value) -> Option<Self> {
        value.get::<u32>().copied()
    }
}

impl HdValueExtract for u64 {
    fn extract(value: &Value) -> Option<Self> {
        value.get::<u64>().copied()
    }
}

impl HdValueExtract for usize {
    fn extract(value: &Value) -> Option<Self> {
        value.get::<usize>().copied()
    }
}

impl HdValueExtract for f32 {
    fn extract(value: &Value) -> Option<Self> {
        value
            .get::<f32>()
            .copied()
            .or_else(|| value.get::<f64>().map(|v| *v as f32))
            .or_else(|| value.get::<i32>().map(|v| *v as f32))
            .or_else(|| value.get::<i64>().map(|v| *v as f32))
    }
}

impl HdValueExtract for f64 {
    fn extract(value: &Value) -> Option<Self> {
        value
            .get::<f64>()
            .copied()
            .or_else(|| value.get::<f32>().map(|v| *v as f64))
            .or_else(|| value.get::<i32>().map(|v| *v as f64))
            .or_else(|| value.get::<i64>().map(|v| *v as f64))
    }
}

impl HdValueExtract for String {
    fn extract(value: &Value) -> Option<Self> {
        value.get::<String>().cloned()
    }
}

impl HdValueExtract for Token {
    fn extract(value: &Value) -> Option<Self> {
        value
            .get::<Token>()
            .cloned()
            .or_else(|| value.get::<String>().map(|s| Token::new(s)))
    }
}

impl HdValueExtract for usd_sdf::Path {
    fn extract(value: &Value) -> Option<Self> {
        value.get::<usd_sdf::Path>().cloned()
    }
}

// --- Value identity (untyped fallback) ------------------------------------

impl HdValueExtract for Value {
    /// Identity extraction — always succeeds. Used by `DataSourceAttribute<Value>`
    /// for untyped attribute access.
    fn extract(value: &Value) -> Option<Self> {
        Some(value.clone())
    }
}

// --- Math type implementations --------------------------------------------

impl HdValueExtract for Matrix4<f64> {
    fn extract(value: &Value) -> Option<Self> {
        value.get::<Matrix4<f64>>().cloned()
    }
}

impl HdValueExtract for Matrix4f {
    fn extract(value: &Value) -> Option<Self> {
        value.get::<Matrix4f>().cloned().or_else(|| {
            value.get::<Matrix4<f64>>().map(|m| {
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
        })
    }
}

impl HdValueExtract for Vec2f {
    fn extract(value: &Value) -> Option<Self> {
        value
            .get::<Vec2f>()
            .copied()
            .or_else(|| value.get::<[f32; 2]>().map(|v| Vec2f::new(v[0], v[1])))
            .or_else(|| {
                value
                    .get::<[f64; 2]>()
                    .map(|v| Vec2f::new(v[0] as f32, v[1] as f32))
            })
            .or_else(|| {
                value.get::<Vec<Value>>().and_then(|values| {
                    let x = f32::extract(values.first()?)?;
                    let y = f32::extract(values.get(1)?)?;
                    Some(Vec2f::new(x, y))
                })
            })
    }
}

impl HdValueExtract for Vec2d {
    fn extract(value: &Value) -> Option<Self> {
        value
            .get::<Vec2d>()
            .copied()
            .or_else(|| value.get::<[f64; 2]>().map(|v| Vec2d::new(v[0], v[1])))
            .or_else(|| {
                value
                    .get::<[f32; 2]>()
                    .map(|v| Vec2d::new(v[0] as f64, v[1] as f64))
            })
            .or_else(|| {
                value.get::<Vec<Value>>().and_then(|values| {
                    let x = f64::extract(values.first()?)?;
                    let y = f64::extract(values.get(1)?)?;
                    Some(Vec2d::new(x, y))
                })
            })
    }
}

impl HdValueExtract for Vec3f {
    fn extract(value: &Value) -> Option<Self> {
        value
            .get::<Vec3f>()
            .copied()
            .or_else(|| {
                value
                    .get::<[f32; 3]>()
                    .map(|v| Vec3f::new(v[0], v[1], v[2]))
            })
            .or_else(|| {
                value
                    .get::<[f64; 3]>()
                    .map(|v| Vec3f::new(v[0] as f32, v[1] as f32, v[2] as f32))
            })
            .or_else(|| {
                value.get::<Vec<Value>>().and_then(|values| {
                    let x = f32::extract(values.first()?)?;
                    let y = f32::extract(values.get(1)?)?;
                    let z = f32::extract(values.get(2)?)?;
                    Some(Vec3f::new(x, y, z))
                })
            })
    }
}

impl HdValueExtract for Vec3d {
    fn extract(value: &Value) -> Option<Self> {
        value
            .get::<Vec3d>()
            .copied()
            .or_else(|| {
                value
                    .get::<[f64; 3]>()
                    .map(|v| Vec3d::new(v[0], v[1], v[2]))
            })
            .or_else(|| {
                value
                    .get::<[f32; 3]>()
                    .map(|v| Vec3d::new(v[0] as f64, v[1] as f64, v[2] as f64))
            })
            .or_else(|| {
                value.get::<Vec<Value>>().and_then(|values| {
                    let x = f64::extract(values.first()?)?;
                    let y = f64::extract(values.get(1)?)?;
                    let z = f64::extract(values.get(2)?)?;
                    Some(Vec3d::new(x, y, z))
                })
            })
    }
}

impl HdValueExtract for Vec2i {
    fn extract(value: &Value) -> Option<Self> {
        value
            .get::<Vec2i>()
            .copied()
            .or_else(|| value.get::<[i32; 2]>().map(|v| Vec2i::new(v[0], v[1])))
            .or_else(|| {
                value.get::<Vec<Value>>().and_then(|values| {
                    let x = i32::extract(values.first()?)?;
                    let y = i32::extract(values.get(1)?)?;
                    Some(Vec2i::new(x, y))
                })
            })
    }
}

impl HdValueExtract for Vec4f {
    fn extract(value: &Value) -> Option<Self> {
        value
            .get::<Vec4f>()
            .copied()
            .or_else(|| {
                value
                    .get::<[f32; 4]>()
                    .map(|v| Vec4f::new(v[0], v[1], v[2], v[3]))
            })
            .or_else(|| {
                value
                    .get::<[f64; 4]>()
                    .map(|v| Vec4f::new(v[0] as f32, v[1] as f32, v[2] as f32, v[3] as f32))
            })
            .or_else(|| {
                value.get::<Vec<Value>>().and_then(|values| {
                    let x = f32::extract(values.first()?)?;
                    let y = f32::extract(values.get(1)?)?;
                    let z = f32::extract(values.get(2)?)?;
                    let w = f32::extract(values.get(3)?)?;
                    Some(Vec4f::new(x, y, z, w))
                })
            })
    }
}

// --- Array implementations ------------------------------------------------
//
// USDA-parsed data may arrive as Vec<Value> (heterogeneous) rather than
// Array<T> or Vec<T>. We handle all three representations, matching the
// logic in the old delegate path (delegate.rs: attr_get_i32_vec, etc.).

impl HdValueExtract for Array<i32> {
    fn extract(value: &Value) -> Option<Self> {
        // Fast path: already Array<i32>
        if let Some(arr) = value.get::<Array<i32>>() {
            return Some(arr.clone());
        }
        // Vec<i32> → Array
        if let Some(vec) = value.get::<Vec<i32>>() {
            return Some(Array::from(vec.clone()));
        }
        // Vec<Value> → per-element extraction (USDA-backed scene index)
        if let Some(values) = value.get::<Vec<Value>>() {
            let mut result = Vec::with_capacity(values.len());
            for v in values {
                let scalar = v
                    .get::<i32>()
                    .copied()
                    .or_else(|| v.get::<i64>().map(|x| *x as i32))
                    .or_else(|| v.get::<f64>().map(|x| *x as i32))?;
                result.push(scalar);
            }
            return Some(Array::from(result));
        }
        None
    }
}

impl HdValueExtract for Array<f32> {
    fn extract(value: &Value) -> Option<Self> {
        if let Some(arr) = value.get::<Array<f32>>() {
            return Some(arr.clone());
        }
        if let Some(vec) = value.get::<Vec<f32>>() {
            return Some(Array::from(vec.clone()));
        }
        if let Some(values) = value.get::<Vec<Value>>() {
            let mut result = Vec::with_capacity(values.len());
            for v in values {
                let scalar = v
                    .get::<f32>()
                    .copied()
                    .or_else(|| v.get::<f64>().map(|x| *x as f32))?;
                result.push(scalar);
            }
            return Some(Array::from(result));
        }
        None
    }
}

impl HdValueExtract for Array<f64> {
    fn extract(value: &Value) -> Option<Self> {
        if let Some(arr) = value.get::<Array<f64>>() {
            return Some(arr.clone());
        }
        if let Some(vec) = value.get::<Vec<f64>>() {
            return Some(Array::from(vec.clone()));
        }
        if let Some(values) = value.get::<Vec<Value>>() {
            let mut result = Vec::with_capacity(values.len());
            for v in values {
                let scalar = v
                    .get::<f64>()
                    .copied()
                    .or_else(|| v.get::<f32>().map(|x| *x as f64))?;
                result.push(scalar);
            }
            return Some(Array::from(result));
        }
        None
    }
}

impl HdValueExtract for Array<Token> {
    fn extract(value: &Value) -> Option<Self> {
        if let Some(arr) = value.get::<Array<Token>>() {
            return Some(arr.clone());
        }
        if let Some(vec) = value.get::<Vec<Token>>() {
            return Some(Array::from(vec.clone()));
        }
        if let Some(values) = value.get::<Vec<Value>>() {
            let mut result = Vec::with_capacity(values.len());
            for v in values {
                let token = v
                    .get::<Token>()
                    .cloned()
                    .or_else(|| v.get::<String>().map(|s| Token::new(s)))?;
                result.push(token);
            }
            return Some(Array::from(result));
        }
        None
    }
}

impl HdValueExtract for Vec<Token> {
    fn extract(value: &Value) -> Option<Self> {
        if let Some(vec) = value.get::<Vec<Token>>() {
            return Some(vec.clone());
        }
        if let Some(arr) = value.get::<Array<Token>>() {
            return Some(arr.to_vec());
        }
        if let Some(values) = value.get::<Vec<Value>>() {
            let mut result = Vec::with_capacity(values.len());
            for v in values {
                result.push(Token::extract(v)?);
            }
            return Some(result);
        }
        None
    }
}

impl HdValueExtract for Vec<f32> {
    fn extract(value: &Value) -> Option<Self> {
        if let Some(vec) = value.get::<Vec<f32>>() {
            return Some(vec.clone());
        }
        if let Some(arr) = value.get::<Array<f32>>() {
            return Some(arr.to_vec());
        }
        if let Some(values) = value.get::<Vec<Value>>() {
            let mut result = Vec::with_capacity(values.len());
            for v in values {
                result.push(f32::extract(v)?);
            }
            return Some(result);
        }
        None
    }
}

impl HdValueExtract for Vec<Matrix4<f64>> {
    fn extract(value: &Value) -> Option<Self> {
        if let Some(vec) = value.get::<Vec<Matrix4<f64>>>() {
            return Some(vec.clone());
        }
        if let Some(arr) = value.get::<Array<Matrix4<f64>>>() {
            return Some(arr.to_vec());
        }
        if let Some(values) = value.get::<Vec<Value>>() {
            let mut result = Vec::with_capacity(values.len());
            for v in values {
                result.push(Matrix4::<f64>::extract(v)?);
            }
            return Some(result);
        }
        None
    }
}

impl HdValueExtract for Vec<Vec2f> {
    fn extract(value: &Value) -> Option<Self> {
        if let Some(vec) = value.get::<Vec<Vec2f>>() {
            return Some(vec.clone());
        }
        if let Some(arr) = value.get::<Array<Vec2f>>() {
            return Some(arr.to_vec());
        }
        if let Some(values) = value.get::<Vec<Value>>() {
            return values.iter().map(Vec2f::extract).collect();
        }
        None
    }
}

impl HdValueExtract for Vec<Vec2d> {
    fn extract(value: &Value) -> Option<Self> {
        if let Some(vec) = value.get::<Vec<Vec2d>>() {
            return Some(vec.clone());
        }
        if let Some(arr) = value.get::<Array<Vec2d>>() {
            return Some(arr.to_vec());
        }
        if let Some(values) = value.get::<Vec<Value>>() {
            return values.iter().map(Vec2d::extract).collect();
        }
        None
    }
}

impl HdValueExtract for Vec<Vec3f> {
    fn extract(value: &Value) -> Option<Self> {
        if let Some(vec) = value.get::<Vec<Vec3f>>() {
            return Some(vec.clone());
        }
        if let Some(arr) = value.get::<Array<Vec3f>>() {
            return Some(arr.to_vec());
        }
        if let Some(values) = value.get::<Vec<Value>>() {
            return values.iter().map(Vec3f::extract).collect();
        }
        None
    }
}

impl HdValueExtract for Vec<Vec3d> {
    fn extract(value: &Value) -> Option<Self> {
        if let Some(vec) = value.get::<Vec<Vec3d>>() {
            return Some(vec.clone());
        }
        if let Some(arr) = value.get::<Array<Vec3d>>() {
            return Some(arr.to_vec());
        }
        if let Some(values) = value.get::<Vec<Value>>() {
            return values.iter().map(Vec3d::extract).collect();
        }
        None
    }
}

impl HdValueExtract for Vec<Vec3h> {
    fn extract(value: &Value) -> Option<Self> {
        if let Some(vec) = value.get::<Vec<Vec3h>>() {
            return Some(vec.clone());
        }
        if let Some(arr) = value.get::<Array<Vec3h>>() {
            return Some(arr.to_vec());
        }
        if let Some(values) = value.get::<Vec<Value>>() {
            let mut result = Vec::with_capacity(values.len());
            for v in values {
                let coords = v.get::<Vec<Value>>()?;
                let x = f32::extract(coords.first()?)?;
                let y = f32::extract(coords.get(1)?)?;
                let z = f32::extract(coords.get(2)?)?;
                result.push(Vec3h::new(x.into(), y.into(), z.into()));
            }
            return Some(result);
        }
        None
    }
}

impl HdValueExtract for Vec<Quatf> {
    fn extract(value: &Value) -> Option<Self> {
        if let Some(vec) = value.get::<Vec<Quatf>>() {
            return Some(vec.clone());
        }
        if let Some(arr) = value.get::<Array<Quatf>>() {
            return Some(arr.to_vec());
        }
        if let Some(values) = value.get::<Vec<Value>>() {
            let mut result = Vec::with_capacity(values.len());
            for v in values {
                let coords = v.get::<Vec<Value>>()?;
                let real = f32::extract(coords.first()?)?;
                let i = f32::extract(coords.get(1)?)?;
                let j = f32::extract(coords.get(2)?)?;
                let k = f32::extract(coords.get(3)?)?;
                result.push(Quatf::from_components(real, i, j, k));
            }
            return Some(result);
        }
        None
    }
}

impl HdValueExtract for Vec<Matrix4f> {
    fn extract(value: &Value) -> Option<Self> {
        if let Some(vec) = value.get::<Vec<Matrix4f>>() {
            return Some(vec.clone());
        }
        if let Some(arr) = value.get::<Array<Matrix4f>>() {
            return Some(arr.to_vec());
        }
        if let Some(values) = value.get::<Vec<Value>>() {
            let mut result = Vec::with_capacity(values.len());
            for v in values {
                let m = Matrix4f::extract(v)?;
                result.push(m);
            }
            return Some(result);
        }
        None
    }
}

impl HdValueExtract for Vec<Vec2i> {
    fn extract(value: &Value) -> Option<Self> {
        if let Some(vec) = value.get::<Vec<Vec2i>>() {
            return Some(vec.clone());
        }
        if let Some(arr) = value.get::<Array<Vec2i>>() {
            return Some(arr.to_vec());
        }
        if let Some(values) = value.get::<Vec<Value>>() {
            return values.iter().map(Vec2i::extract).collect();
        }
        None
    }
}

impl HdValueExtract for Array<Vec2d> {
    fn extract(value: &Value) -> Option<Self> {
        if let Some(arr) = value.get::<Array<Vec2d>>() {
            return Some(arr.clone());
        }
        if let Some(vec) = value.get::<Vec<Vec2d>>() {
            return Some(Array::from(vec.clone()));
        }
        if let Some(values) = value.get::<Vec<Value>>() {
            return Some(Array::from(
                values
                    .iter()
                    .map(Vec2d::extract)
                    .collect::<Option<Vec<_>>>()?,
            ));
        }
        None
    }
}

impl HdValueExtract for Array<Vec3d> {
    fn extract(value: &Value) -> Option<Self> {
        if let Some(arr) = value.get::<Array<Vec3d>>() {
            return Some(arr.clone());
        }
        if let Some(vec) = value.get::<Vec<Vec3d>>() {
            return Some(Array::from(vec.clone()));
        }
        if let Some(values) = value.get::<Vec<Value>>() {
            return Some(Array::from(
                values
                    .iter()
                    .map(Vec3d::extract)
                    .collect::<Option<Vec<_>>>()?,
            ));
        }
        None
    }
}

impl HdValueExtract for std::vec::Vec<Vec3i> {
    fn extract(value: &Value) -> Option<Self> {
        if let Some(vec) = value.get::<Vec<Vec3i>>() {
            return Some(vec.clone());
        }
        if let Some(arr) = value.get::<Array<Vec3i>>() {
            return Some(arr.to_vec());
        }
        if let Some(values) = value.get::<Vec<Value>>() {
            let mut result = Vec::with_capacity(values.len());
            for v in values {
                let coords = v.get::<Vec<Value>>()?;
                let x = i32::extract(coords.first()?)?;
                let y = i32::extract(coords.get(1)?)?;
                let z = i32::extract(coords.get(2)?)?;
                result.push(Vec3i::new(x, y, z));
            }
            return Some(result);
        }
        None
    }
}

impl HdValueExtract for std::vec::Vec<Vec4i> {
    fn extract(value: &Value) -> Option<Self> {
        if let Some(vec) = value.get::<Vec<Vec4i>>() {
            return Some(vec.clone());
        }
        if let Some(arr) = value.get::<Array<Vec4i>>() {
            return Some(arr.to_vec());
        }
        if let Some(values) = value.get::<Vec<Value>>() {
            let mut result = Vec::with_capacity(values.len());
            for v in values {
                let coords = v.get::<Vec<Value>>()?;
                let x = i32::extract(coords.first()?)?;
                let y = i32::extract(coords.get(1)?)?;
                let z = i32::extract(coords.get(2)?)?;
                let w = i32::extract(coords.get(3)?)?;
                result.push(Vec4i::new(x, y, z, w));
            }
            return Some(result);
        }
        None
    }
}

// ---------------------------------------------------------------------------
// SampledToTypedAdapter
// ---------------------------------------------------------------------------

/// Adapts any [`HdSampledDataSource`] into a [`HdTypedSampledDataSource<T>`].
///
/// This is the Rust equivalent of C++ `dynamic_pointer_cast<HdTypedSampledDataSource<T>>`.
/// In C++, RTTI allows zero-cost casting between trait object types. In Rust,
/// we wrap the untyped data source and extract `T` from `Value` on each access.
///
/// Used by [`HdSchema::get_typed_retained`](crate::schema::HdSchema::get_typed_retained)
/// as a fallback when the concrete-type downcast (to `HdRetainedTypedSampledDataSource<T>`)
/// fails — enabling typed schema accessors to work with any data source that
/// implements `as_sampled()`.
pub struct SampledToTypedAdapter<T: HdValueExtract> {
    /// The wrapped data source (owns the Arc so the adapter is self-contained).
    inner: HdDataSourceBaseHandle,
    _phantom: PhantomData<T>,
}

impl<T: HdValueExtract> SampledToTypedAdapter<T> {
    /// Create an adapter from a data source base handle.
    ///
    /// The handle must return `Some` from `as_sampled()`, otherwise
    /// `get_typed_value` will return `T::default()`.
    pub fn new(inner: HdDataSourceBaseHandle) -> Arc<Self> {
        Arc::new(Self {
            inner,
            _phantom: PhantomData,
        })
    }
}

impl<T: HdValueExtract> fmt::Debug for SampledToTypedAdapter<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SampledToTypedAdapter")
            .field("inner", &self.inner)
            .field("type", &std::any::type_name::<T>())
            .finish()
    }
}

impl<T: HdValueExtract> Clone for SampledToTypedAdapter<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            _phantom: PhantomData,
        }
    }
}

impl<T: HdValueExtract + fmt::Debug> HdDataSourceBase for SampledToTypedAdapter<T> {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(self.clone())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_sampled(&self) -> Option<&dyn HdSampledDataSource> {
        Some(self)
    }
}

impl<T: HdValueExtract + fmt::Debug> HdSampledDataSource for SampledToTypedAdapter<T> {
    fn get_value(&self, shutter_offset: HdSampledDataSourceTime) -> Value {
        if let Some(sampled) = self.inner.as_sampled() {
            sampled.get_value(shutter_offset)
        } else {
            Value::empty()
        }
    }

    fn get_contributing_sample_times(
        &self,
        start_time: HdSampledDataSourceTime,
        end_time: HdSampledDataSourceTime,
        out_sample_times: &mut Vec<HdSampledDataSourceTime>,
    ) -> bool {
        if let Some(sampled) = self.inner.as_sampled() {
            sampled.get_contributing_sample_times(start_time, end_time, out_sample_times)
        } else {
            false
        }
    }
}

impl<T: HdValueExtract + fmt::Debug> HdTypedSampledDataSource<T> for SampledToTypedAdapter<T> {
    fn get_typed_value(&self, shutter_offset: HdSampledDataSourceTime) -> T {
        if let Some(sampled) = self.inner.as_sampled() {
            let value = sampled.get_value(shutter_offset);
            T::extract(&value).unwrap_or_default()
        } else {
            log::debug!(
                "[SampledToTypedAdapter] as_sampled=None type={}",
                std::any::type_name::<T>()
            );
            T::default()
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_source::retained::HdRetainedSampledDataSource;

    #[test]
    fn test_extract_i32() {
        let v = Value::new(42i32);
        assert_eq!(i32::extract(&v), Some(42));
    }

    #[test]
    fn test_extract_token() {
        let v = Value::new(Token::new("hello"));
        assert_eq!(Token::extract(&v), Some(Token::new("hello")));
    }

    #[test]
    fn test_extract_token_from_string() {
        let v = Value::new("world".to_string());
        assert_eq!(Token::extract(&v), Some(Token::new("world")));
    }

    #[test]
    fn test_extract_int_array_from_vec_value() {
        // Simulates USDA-backed scene index: array stored as Vec<Value>
        let values: Vec<Value> = vec![Value::new(3i32), Value::new(4i32), Value::new(3i32)];
        let v = Value::new(values);
        let arr = Array::<i32>::extract(&v).expect("should extract");
        assert_eq!(arr.as_slice(), &[3, 4, 3]);
    }

    #[test]
    fn test_extract_int_array_from_native() {
        let arr = Array::from(vec![1i32, 2, 3]);
        let v = Value::new(arr.clone());
        let extracted = Array::<i32>::extract(&v).expect("should extract");
        assert_eq!(extracted.as_slice(), &[1, 2, 3]);
    }

    #[test]
    fn test_adapter_with_retained_sampled() {
        // Wrap a retained sampled data source in the adapter
        let ds = HdRetainedSampledDataSource::new(Value::new(42i32));
        let adapter = SampledToTypedAdapter::<i32>::new(ds as HdDataSourceBaseHandle);
        assert_eq!(adapter.get_typed_value(0.0), 42);
    }

    #[test]
    fn test_adapter_with_array_value() {
        let values: Vec<Value> = vec![Value::new(1i32), Value::new(2i32), Value::new(3i32)];
        let ds = HdRetainedSampledDataSource::new(Value::new(values));
        let adapter = SampledToTypedAdapter::<Array<i32>>::new(ds as HdDataSourceBaseHandle);
        let result = adapter.get_typed_value(0.0);
        assert_eq!(result.as_slice(), &[1, 2, 3]);
    }

    #[test]
    fn test_adapter_returns_default_on_mismatch() {
        let ds = HdRetainedSampledDataSource::new(Value::new("not an int".to_string()));
        let adapter = SampledToTypedAdapter::<i32>::new(ds as HdDataSourceBaseHandle);
        assert_eq!(adapter.get_typed_value(0.0), 0); // i32::default()
    }

    #[test]
    fn test_extract_bool() {
        let v = Value::new(true);
        assert_eq!(bool::extract(&v), Some(true));
    }

    #[test]
    fn test_extract_f32_from_f64() {
        let v = Value::from_no_hash(3.14f64);
        let result = f32::extract(&v).expect("should extract");
        assert!((result - 3.14f32).abs() < 0.001);
    }

    #[test]
    fn test_extract_vec3f_array_from_nested_vec_value() {
        let values = vec![
            Value::new(vec![
                Value::from_no_hash(0.0f64),
                Value::from_no_hash(1.0f64),
                Value::from_no_hash(2.0f64),
            ]),
            Value::new(vec![
                Value::from_no_hash(3.0f64),
                Value::from_no_hash(4.0f64),
                Value::from_no_hash(5.0f64),
            ]),
        ];
        let extracted = Vec::<Vec3f>::extract(&Value::new(values)).expect("should extract");
        assert_eq!(extracted[0], Vec3f::new(0.0, 1.0, 2.0));
        assert_eq!(extracted[1], Vec3f::new(3.0, 4.0, 5.0));
    }
}
