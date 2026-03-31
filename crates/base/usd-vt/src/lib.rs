//! Value Types (vt) - Type-erased value containers.
//!
//! This module provides type-erased value containers used throughout USD:
//!
//! - `Value` - Type-erased value container (equivalent to VtValue)
//! - `ValueRef` - Non-owning reference to a value (zero-cost abstraction)
//! - `Array<T>` - Typed array with copy-on-write semantics
//! - `Dictionary` - Key-value dictionary (string to Value)
//! - `ArrayEdit<T>` - Array modification operations (critical for SDF list editing)
//! - `ArrayEditBuilder<T>` - Fluent builder for array edits
//! - `ValueVisitor` - Visitor pattern for efficient Value type inspection
//! - `visit_value()` - Dispatch Value to appropriate visitor method
//! - `ValueTransform` - Trait for value transformations
//!
//! # Examples
//!
//! ```ignore
//! use usd_vt::{Value, Array};
//!
//! // Create type-erased values
//! let v1 = Value::from(42i32);
//! let v2 = Value::from("hello".to_string());
//!
//! // Check types and extract values
//! assert!(v1.is::<i32>());
//! assert_eq!(v1.get::<i32>(), Some(&42));
//!
//! // Create typed arrays
//! let arr: Array<f32> = Array::from(vec![1.0, 2.0, 3.0]);
//! assert_eq!(arr.len(), 3);
//! ```
//!
//! # Efficient Value Access
//!
//! ```
//! use usd_vt::{Value, ValueRef};
//!
//! fn process_value(val_ref: ValueRef) {
//!     if let Some(&n) = val_ref.get::<i32>() {
//!         println!("Got integer: {}", n);
//!     }
//! }
//!
//! let value = Value::from(42i32);
//! process_value(ValueRef::from(&value)); // No cloning!
//! ```
//!
//! # Array Editing
//!
//! ```
//! use usd_vt::{Array, ArrayEditBuilder};
//!
//! let mut builder = ArrayEditBuilder::new();
//! builder.write(42, 0).append(100);
//! let edit = builder.build();
//!
//! let mut array = Array::from(vec![1, 2, 3]);
//! edit.apply(&mut array);
//! assert_eq!(array[0], 42);
//! ```
//!
//! # Value Visitor Pattern
//!
//! ```
//! use usd_vt::{Value, visit_value, ValueVisitor};
//!
//! struct IsNumeric;
//!
//! impl ValueVisitor for IsNumeric {
//!     type Output = bool;
//!
//!     fn visit_int(&mut self, _: i32) -> bool { true }
//!     fn visit_float(&mut self, _: f32) -> bool { true }
//!     fn visit_double(&mut self, _: f64) -> bool { true }
//!     fn visit_unknown(&mut self, _: &Value) -> bool { false }
//! }
//!
//! let val = Value::from(42i32);
//! assert!(visit_value(&val, &mut IsNumeric));
//! ```
//!
//! # Value Transformations
//!
//! ```
//! use usd_vt::{Value, value_transform::{ValueTransform, ScaleTransform, TransformChain}};
//!
//! let val = Value::from(10.0_f64);
//!
//! // Simple scale
//! let scale = ScaleTransform::new(2.0);
//! let result = scale.transform(&val).unwrap();
//! assert_eq!(result.get::<f64>(), Some(&20.0));
//!
//! // Chain transformations
//! use usd_vt::value_transform::{OffsetTransform, ClampTransform};
//! let mut chain = TransformChain::new();
//! chain.push(ScaleTransform::new(3.0));
//! chain.push(OffsetTransform::new(5.0));
//! chain.push(ClampTransform::new(0.0, 30.0));
//! let result = chain.transform(&val).unwrap();
//! assert_eq!(result.get::<f64>(), Some(&30.0)); // (10 * 3) + 5 = 35, clamped to 30
//! ```

pub mod array;
pub mod array_edit;
pub mod array_edit_builder;
pub mod array_edit_ops;
pub mod asset_path;
pub mod debug_codes;
pub mod dictionary;
pub mod hash;
pub mod stream_out;
pub mod time_code;
pub mod traits;
pub mod type_headers;
pub mod types;
pub mod value;
pub mod value_common;
pub mod value_compose_over;
pub mod value_ref;
pub mod value_transform;
pub mod visit_value;

#[cfg(test)]
mod array_edit_integration_tests;
#[cfg(test)]
mod test_vt_array_edit;

pub use array::{Array, DetachCallback, ForeignDataSource, MAX_OTHER_DIMS, ShapeData};
pub use array_edit::ArrayEdit;
pub use array_edit_builder::ArrayEditBuilder;
pub use array_edit_ops::{END_INDEX, EditOp};
pub use asset_path::{AssetPath, AssetPathHash, AssetPathParams, swap_asset_paths};
pub use dictionary::{
    Dictionary, dictionary_over, dictionary_over_coerce, dictionary_over_in_place,
    dictionary_over_in_place_coerce, dictionary_over_into_weak, dictionary_over_into_weak_coerce,
    dictionary_over_recursive, dictionary_over_recursive_in_place,
    dictionary_over_recursive_into_weak, get_empty_dictionary,
};
pub use hash::*;
pub use time_code::TimeCode;
pub use traits::*;
pub use types::*;
pub use value::{Value, stream_out_array, stream_out_array_shaped, stream_out_generic};
pub use value_ref::ValueRef;
pub mod spline;
pub use debug_codes::VtDebugCode;
pub use spline::{
    SplineCurveType, SplineExtrapolation, SplineInterpMode, SplineKnot, SplineLoopParams,
    SplineTangent, SplineTangentAlgorithm, SplineValue,
};
pub use stream_out::{
    StreamOutable, stream_out_bool, stream_out_double, stream_out_float,
    stream_out_generic as vt_stream_out_generic, stream_out_value,
};
pub use value_compose_over::{
    BackgroundType, VT_BACKGROUND, ValueComposable, value_can_compose_over, value_compose_over,
    value_try_compose_over, value_type_can_compose_over,
};
pub use value_transform::{
    ClampTransform, IdentityTransform, MapTransform, OffsetTransform, ScaleTransform,
    TransformChain, ValueTransform, transform_array, transform_value,
};
pub use visit_value::{
    ArraySizeVisitor, HashVisitor, PrintVisitor, TypeCollectorVisitor, ValueVisitor, visit_value,
};
