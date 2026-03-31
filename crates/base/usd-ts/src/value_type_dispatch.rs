//! Value type dispatch for splines.
//!
//! Port of pxr/base/ts/valueTypeDispatch.h
//!
//! Provides runtime dispatch to type-specific code based on value type.

use super::knot_data::KnotValueType;

/// Trait for operations that can be dispatched by value type.
///
/// Implement this trait to create operations that work with
/// different spline value types at runtime.
pub trait ValueTypeOperation {
    /// The output type of the operation.
    type Output;

    /// Execute the operation for f64 values.
    fn execute_f64(&mut self) -> Self::Output;

    /// Execute the operation for f32 values.
    fn execute_f32(&mut self) -> Self::Output;

    /// Execute the operation for Half values (placeholder).
    fn execute_half(&mut self) -> Self::Output;
}

/// Dispatches to the appropriate method based on value type.
///
/// # Example
///
/// ```ignore
/// struct MyOp { knot: &Knot, result: bool }
///
/// impl ValueTypeOperation for MyOp {
///     type Output = bool;
///     fn execute_f64(&mut self) -> bool { ... }
///     fn execute_f32(&mut self) -> bool { ... }
///     fn execute_half(&mut self) -> bool { ... }
/// }
///
/// let result = dispatch_to_value_type(KnotValueType::Double, &mut my_op);
/// ```
pub fn dispatch_to_value_type<Op: ValueTypeOperation>(
    value_type: KnotValueType,
    op: &mut Op,
) -> Op::Output {
    match value_type {
        KnotValueType::Double => op.execute_f64(),
        KnotValueType::Float => op.execute_f32(),
        KnotValueType::Half => op.execute_half(),
    }
}

/// Dispatches a closure to the appropriate value type.
///
/// This is a simpler alternative to implementing ValueTypeOperation
/// when you just need to run different code for different types.
pub fn dispatch_with<F64, F32, FH, O>(
    value_type: KnotValueType,
    f64_fn: F64,
    f32_fn: F32,
    half_fn: FH,
) -> O
where
    F64: FnOnce() -> O,
    F32: FnOnce() -> O,
    FH: FnOnce() -> O,
{
    match value_type {
        KnotValueType::Double => f64_fn(),
        KnotValueType::Float => f32_fn(),
        KnotValueType::Half => half_fn(),
    }
}

/// Generic visitor trait for type-erased operations.
///
/// Use when you need to visit spline data of any value type.
pub trait ValueTypeVisitor {
    /// Visit f64 data.
    fn visit_f64(&mut self);

    /// Visit f32 data.
    fn visit_f32(&mut self);

    /// Visit Half data.
    fn visit_half(&mut self);
}

/// Dispatches a visitor based on value type.
pub fn dispatch_visitor(value_type: KnotValueType, visitor: &mut dyn ValueTypeVisitor) {
    match value_type {
        KnotValueType::Double => visitor.visit_f64(),
        KnotValueType::Float => visitor.visit_f32(),
        KnotValueType::Half => visitor.visit_half(),
    }
}

/// Macro for creating type-dispatched operations.
///
/// This macro helps reduce boilerplate when implementing
/// operations that need to work with multiple value types.
///
/// # Example
///
/// ```ignore
/// dispatch_by_type!(value_type, {
///     Double => { println!("f64"); },
///     Float => { println!("f32"); },
///     Half => { println!("half"); },
/// });
/// ```
#[macro_export]
macro_rules! dispatch_by_type {
    ($value_type:expr, {
        Double => $double_block:block,
        Float => $float_block:block,
        Half => $half_block:block $(,)?
    }) => {
        match $value_type {
            $crate::KnotValueType::Double => $double_block,
            $crate::KnotValueType::Float => $float_block,
            $crate::KnotValueType::Half => $half_block,
        }
    };
}

/// Type-erased function for processing values.
pub type ValueProcessor<T> = Box<dyn Fn(T) -> T + Send + Sync>;

/// Creates a type-appropriate processor based on value type.
pub fn create_processor<F>(
    _value_type: KnotValueType,
    f: F,
) -> Box<dyn Fn(f64) -> f64 + Send + Sync>
where
    F: Fn(f64) -> f64 + Send + Sync + 'static,
{
    // For now, all processing happens in f64 space
    // Type conversion happens at the boundary
    Box::new(f)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestOp {
        value: i32,
    }

    impl ValueTypeOperation for TestOp {
        type Output = i32;

        fn execute_f64(&mut self) -> i32 {
            self.value = 64;
            64
        }

        fn execute_f32(&mut self) -> i32 {
            self.value = 32;
            32
        }

        fn execute_half(&mut self) -> i32 {
            self.value = 16;
            16
        }
    }

    #[test]
    fn test_dispatch_operation() {
        let mut op = TestOp { value: 0 };

        let result = dispatch_to_value_type(KnotValueType::Double, &mut op);
        assert_eq!(result, 64);
        assert_eq!(op.value, 64);

        let result = dispatch_to_value_type(KnotValueType::Float, &mut op);
        assert_eq!(result, 32);
        assert_eq!(op.value, 32);

        let result = dispatch_to_value_type(KnotValueType::Half, &mut op);
        assert_eq!(result, 16);
        assert_eq!(op.value, 16);
    }

    #[test]
    fn test_dispatch_with_closures() {
        let result: &str = dispatch_with(KnotValueType::Double, || "double", || "float", || "half");
        assert_eq!(result, "double");

        let result: &str = dispatch_with(KnotValueType::Float, || "double", || "float", || "half");
        assert_eq!(result, "float");
    }

    struct TestVisitor {
        visited: String,
    }

    impl ValueTypeVisitor for TestVisitor {
        fn visit_f64(&mut self) {
            self.visited = "f64".to_string();
        }

        fn visit_f32(&mut self) {
            self.visited = "f32".to_string();
        }

        fn visit_half(&mut self) {
            self.visited = "half".to_string();
        }
    }

    #[test]
    fn test_visitor() {
        let mut visitor = TestVisitor {
            visited: String::new(),
        };

        dispatch_visitor(KnotValueType::Double, &mut visitor);
        assert_eq!(visitor.visited, "f64");

        dispatch_visitor(KnotValueType::Float, &mut visitor);
        assert_eq!(visitor.visited, "f32");

        dispatch_visitor(KnotValueType::Half, &mut visitor);
        assert_eq!(visitor.visited, "half");
    }
}
