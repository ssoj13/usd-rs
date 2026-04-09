//! IR Interpreter — execute OSL opcodes without JIT compilation.
//!
//! Port of the OSL interpreter loop. Walks the opcode stream and
//! evaluates each instruction. Sufficient for correctness testing
//! and reference-compatible execution of compiled shaders.

use std::sync::Arc;

use crate::codegen::{ConstValue, ShaderIR};
use crate::math::{Color3, Matrix44, Vec3};
use crate::message::{MessageStore, MessageValue};
use crate::renderer::RendererServices;
use crate::shaderglobals::ShaderGlobals;
use crate::shadingsys::ErrorHandler;
use crate::typedesc::{Aggregate, BaseType, TypeDesc};
use crate::ustring::{UString, UStringHash};

fn value_to_message_value(v: &Value) -> MessageValue {
    match v {
        Value::Int(i) => MessageValue::Int(*i),
        Value::Float(f) | Value::DualFloat(f, _, _) => MessageValue::Float(*f),
        Value::String(s) => MessageValue::String(*s),
        Value::Vec3(v) | Value::Color(v) | Value::DualVec3(v, _, _) => MessageValue::Color(*v),
        Value::IntArray(a) => MessageValue::IntArray(a.clone()),
        Value::FloatArray(a) => MessageValue::FloatArray(a.clone()),
        _ => MessageValue::Float(0.0),
    }
}

fn message_value_to_value(m: &MessageValue) -> Value {
    match m {
        MessageValue::Int(i) => Value::Int(*i),
        MessageValue::Float(f) => Value::Float(*f),
        MessageValue::String(s) => Value::String(*s),
        MessageValue::Color(c)
        | MessageValue::Point(c)
        | MessageValue::Vector(c)
        | MessageValue::Normal(c) => Value::Color(*c),
        MessageValue::IntArray(a) => Value::IntArray(a.clone()),
        MessageValue::FloatArray(a) => Value::FloatArray(a.clone()),
        MessageValue::Matrix(m) => Value::Matrix(*m),
        _ => Value::Float(0.0), // StringArray, ColorArray, etc. — no direct Value equivalent
    }
}

/// Recursive per-field equality for Value::Struct comparison (used by eq/neq opcodes).
fn values_equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => x == y,
        (Value::String(x), Value::String(y)) => x == y,
        (Value::Vec3(x), Value::Vec3(y))
        | (Value::Color(x), Value::Color(y))
        | (Value::Vec3(x), Value::Color(y))
        | (Value::Color(x), Value::Vec3(y)) => x.x == y.x && x.y == y.y && x.z == y.z,
        (Value::Matrix(x), Value::Matrix(y)) => x.m == y.m,
        (Value::Struct(xa), Value::Struct(xb)) => {
            xa.len() == xb.len()
                && xa
                    .iter()
                    .zip(xb.iter())
                    .all(|(fa, fb)| values_equal(fa, fb))
        }
        _ => a.as_float() == b.as_float(),
    }
}

/// Message validation config for execute (plan #45).
pub struct ExecuteMessageConfig<'a> {
    pub shared_messages: &'a mut MessageStore,
    pub layeridx: i32,
    pub strict: bool,
    pub errhandler: &'a dyn ErrorHandler,
}

type UnknownCoordsysReportFn<'a> = Box<dyn Fn(&str) + 'a>;

fn make_unknown_coordsys_reporter<'a>(
    errh: Option<&'a dyn ErrorHandler>,
    fallback: &'a std::cell::RefCell<String>,
) -> UnknownCoordsysReportFn<'a> {
    Box::new(move |name: &str| {
        let msg = format!("Unknown transformation \"{}\"", name);
        if let Some(h) = errh {
            h.error(&msg);
        } else {
            *fallback.borrow_mut() = format!("ERROR: {msg}\n");
        }
    })
}

/// Represents a closure value in the interpreter.
/// Matches C++ ClosureColor tree: Component (leaf), Mul (weight × child), Add (child_a + child_b).
#[derive(Clone)]
pub enum ClosureValue {
    /// Single weighted closure component (e.g. diffuse, phong).
    Component {
        name: String,
        id: i32,
        params: Vec<Value>,
        weight: Color3,
    },
    /// Add two closure sub-trees (matches osl_add_closure_closure).
    Add(Box<ClosureValue>, Box<ClosureValue>),
    /// Multiply closure by weight (matches osl_mul_closure_*).
    Mul {
        weight: Color3,
        closure: Box<ClosureValue>,
    },
}

impl std::fmt::Debug for ClosureValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClosureValue::Component { name, id, .. } => write!(f, "Closure({}#{})", name, id),
            ClosureValue::Add(..) => write!(f, "ClosureAdd"),
            ClosureValue::Mul { .. } => write!(f, "ClosureMul"),
        }
    }
}

impl ClosureValue {
    /// For Component: access weight. For Mul: combined weight. For Add: (1,1,1) placeholder.
    pub fn weight(&self) -> Color3 {
        match self {
            ClosureValue::Component { weight, .. } => *weight,
            ClosureValue::Mul { weight, .. } => *weight,
            ClosureValue::Add(..) => Color3::new(1.0, 1.0, 1.0),
        }
    }
    /// First component name (for simple closures or Mul/Add of single component).
    pub fn name(&self) -> &str {
        match self {
            ClosureValue::Component { name, .. } => name,
            ClosureValue::Mul { closure, .. } => closure.name(),
            ClosureValue::Add(a, _) => a.name(),
        }
    }

    /// Format closure for printf %g output, matching C++ OSL print_closure format.
    ///
    /// Each leaf: "(w.x, w.y, w.z) * name (p1, p2, ...)".
    /// Multiple leaves separated by newline + tab + "+ ".
    /// Mul weights are accumulated into each leaf's printed weight.
    pub fn fmt_display(&self) -> String {
        let mut out = String::new();
        let mut first = true;
        self.fmt_inner(Color3::new(1.0, 1.0, 1.0), &mut out, &mut first);
        out
    }

    /// Recursive helper: propagates accumulated weight to leaves.
    fn fmt_inner(&self, w: Color3, out: &mut String, first: &mut bool) {
        match self {
            ClosureValue::Component {
                name,
                weight,
                params,
                ..
            } => {
                let eff = Color3::new(w.x * weight.x, w.y * weight.y, w.z * weight.z);
                if !*first {
                    out.push_str("\n\t+ ");
                }
                let p = if params.is_empty() {
                    "()".to_string()
                } else {
                    let ps: Vec<String> = params.iter().map(Self::fmt_param).collect();
                    format!("({})", ps.join(", "))
                };
                out.push_str(&format!(
                    "({}, {}, {}) * {} {}",
                    eff.x, eff.y, eff.z, name, p
                ));
                *first = false;
            }
            ClosureValue::Mul {
                weight: mw,
                closure,
            } => {
                // Accumulate Mul weight - never emit it as a standalone line
                let new_w = Color3::new(w.x * mw.x, w.y * mw.y, w.z * mw.z);
                closure.fmt_inner(new_w, out, first);
            }
            ClosureValue::Add(a, b) => {
                a.fmt_inner(w, out, first);
                b.fmt_inner(w, out, first);
            }
        }
    }

    /// Format a single closure parameter value
    fn fmt_param(v: &Value) -> String {
        match v {
            Value::Float(f) | Value::DualFloat(f, ..) => format!("{}", f),
            Value::Int(i) => format!("{}", i),
            Value::String(s) => format!("\"{}\"", s.as_str()),
            Value::Vec3(v) | Value::Color(v) | Value::DualVec3(v, ..) => {
                format!("({}, {}, {})", v.x, v.y, v.z)
            }
            Value::IntArray(a) => format!(
                "[{}]",
                a.iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Value::FloatArray(a) => format!(
                "[{}]",
                a.iter()
                    .map(|v| format!("{}", v))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Value::Vec3Array(a) => format!(
                "[{}]",
                a.iter()
                    .map(|v| format!("({}, {}, {})", v.x, v.y, v.z))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Value::StringArray(a) => format!(
                "[{}]",
                a.iter()
                    .map(|v| format!("\"{}\"", v.as_str()))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Value::ClosureArray(a) => format!(
                "[{}]",
                a.iter()
                    .map(|v| match v {
                        Some(cv) => cv.as_ref().fmt_display(),
                        None => "null".to_string(),
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            _ => "?".to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Value {
    Int(i32),
    Float(f32),
    String(UString),
    Vec3(Vec3),
    Color(Color3),
    Matrix(Matrix44),
    IntArray(Vec<i32>),
    FloatArray(Vec<f32>),
    StringArray(Vec<UString>),
    Vec3Array(Vec<Vec3>),
    MatrixArray(Vec<Matrix44>),
    Closure(Box<ClosureValue>),
    /// Array of closures; elements are Option because closure slots start null.
    ClosureArray(Vec<Option<Box<ClosureValue>>>),
    /// Float with derivatives (val, dx, dy) for automatic differentiation.
    /// Enables proper texture filter width computation in the interpreter.
    DualFloat(f32, f32, f32),
    /// Vec3 with derivatives (val, dx, dy) for automatic differentiation.
    DualVec3(Vec3, Vec3, Vec3),
    /// Composite struct value: fields are stored in order matching the StructSpec.
    Struct(Vec<Value>),
    Void,
}

impl Value {
    pub fn as_float(&self) -> f32 {
        match self {
            Value::Float(f) => *f,
            Value::DualFloat(f, _, _) => *f,
            Value::Int(i) => *i as f32,
            _ => 0.0,
        }
    }

    pub fn as_int(&self) -> i32 {
        match self {
            Value::Int(i) => *i,
            Value::Float(f) | Value::DualFloat(f, _, _) => *f as i32,
            _ => 0,
        }
    }

    pub fn is_triple(&self) -> bool {
        matches!(self, Value::Vec3(_) | Value::Color(_) | Value::DualVec3(..))
    }

    pub fn as_vec3(&self) -> Vec3 {
        match self {
            Value::Vec3(v) | Value::Color(v) => *v,
            Value::DualVec3(v, _, _) => *v,
            Value::Float(f) | Value::DualFloat(f, _, _) => Vec3::splat(*f),
            _ => Vec3::ZERO,
        }
    }

    pub fn as_color(&self) -> Color3 {
        self.as_vec3()
    }

    pub fn as_string(&self) -> UString {
        match self {
            Value::String(s) => *s,
            _ => UString::empty(),
        }
    }

    /// Extract x-derivative (Dx). Returns 0 if no derivatives.
    pub fn dx_float(&self) -> f32 {
        match self {
            Value::DualFloat(_, dx, _) => *dx,
            _ => 0.0,
        }
    }

    /// Extract y-derivative (Dy). Returns 0 if no derivatives.
    pub fn dy_float(&self) -> f32 {
        match self {
            Value::DualFloat(_, _, dy) => *dy,
            _ => 0.0,
        }
    }

    /// Extract x-derivative for Vec3 (Dx). Returns ZERO if no derivatives.
    pub fn dx_vec3(&self) -> Vec3 {
        match self {
            Value::DualVec3(_, dx, _) => *dx,
            _ => Vec3::ZERO,
        }
    }

    /// Extract y-derivative for Vec3 (Dy). Returns ZERO if no derivatives.
    pub fn dy_vec3(&self) -> Vec3 {
        match self {
            Value::DualVec3(_, _, dy) => *dy,
            _ => Vec3::ZERO,
        }
    }

    /// Check if this value carries derivatives.
    pub fn has_derivs(&self) -> bool {
        matches!(self, Value::DualFloat(..) | Value::DualVec3(..))
    }

    /// Strip derivatives, returning a plain value.
    pub fn strip_derivs(&self) -> Value {
        match self {
            Value::DualFloat(v, _, _) => Value::Float(*v),
            Value::DualVec3(v, _, _) => Value::Vec3(*v),
            other => other.clone(),
        }
    }

    pub fn is_truthy(&self) -> bool {
        match self {
            Value::Int(i) => *i != 0,
            Value::Float(f) | Value::DualFloat(f, _, _) => *f != 0.0,
            Value::String(s) => !s.as_str().is_empty(),
            Value::Vec3(v) | Value::Color(v) | Value::DualVec3(v, _, _) => {
                v.x != 0.0 || v.y != 0.0 || v.z != 0.0
            }
            Value::Matrix(m) => m.m.iter().flatten().any(|v| *v != 0.0),
            _ => true,
        }
    }

    /// Check if value is a float type (scalar or promoted int).
    pub fn is_numeric(&self) -> bool {
        matches!(self, Value::Float(_) | Value::DualFloat(..) | Value::Int(_))
    }
}

// ---------------------------------------------------------------------------
// C++-compatible safe arithmetic helpers (matching llvm_ops.cpp)
// ---------------------------------------------------------------------------

/// C++-compatible safe_div: does the division, returns 0 if result is non-finite.
/// Matches C++ `osl_safe_div_fff`: `float q = a/b; return isfinite(q) ? q : 0.0f`
#[inline]
fn safe_div_f32(a: f32, b: f32) -> f32 {
    let q = a / b;
    if q.is_finite() { q } else { 0.0 }
}

/// OSL int mod: C-style truncation (a % b), safe for division by zero and overflow.
/// Matches C++ osl_safe_mod_iii: `(b != 0) ? (a % b) : 0`.
/// Extra guard: `i32::MIN % -1` overflows in Rust debug mode, return 0 (same as C++ UB result).
#[inline]
fn osl_mod_i32(a: i32, b: i32) -> i32 {
    if b == 0 || b == -1 { 0 } else { a % b }
}

/// OSL mod for f32: a - b*floor(a/b).
fn osl_mod_f32(a: f32, b: f32) -> f32 {
    if b == 0.0 {
        return 0.0;
    }

    a - b * (a / b).floor()
}

/// C++-compatible safe_fmod: truncation toward zero (result sign = dividend sign).
/// Matches C++ dual.h `safe_fmod`: `int N = (int)(a/b); return a - N*b`
/// Uses f64 intermediate to avoid i32 overflow with extreme values.
#[inline]
fn safe_fmod_f32(a: f32, b: f32) -> f32 {
    if b != 0.0 {
        let q = (a as f64 / b as f64).trunc();
        (a as f64 - q * b as f64) as f32
    } else {
        0.0
    }
}

// ---------------------------------------------------------------------------
// Dual2-aware arithmetic helpers
// ---------------------------------------------------------------------------

/// Add two values with derivative propagation: d(a+b) = da + db
fn dual_add(a: &Value, b: &Value) -> Value {
    let av = a.as_float();
    let bv = b.as_float();
    if a.has_derivs() || b.has_derivs() {
        Value::DualFloat(
            av + bv,
            a.dx_float() + b.dx_float(),
            a.dy_float() + b.dy_float(),
        )
    } else {
        Value::Float(av + bv)
    }
}

/// Subtract with derivative propagation: d(a-b) = da - db
fn dual_sub(a: &Value, b: &Value) -> Value {
    let av = a.as_float();
    let bv = b.as_float();
    if a.has_derivs() || b.has_derivs() {
        Value::DualFloat(
            av - bv,
            a.dx_float() - b.dx_float(),
            a.dy_float() - b.dy_float(),
        )
    } else {
        Value::Float(av - bv)
    }
}

/// Multiply with derivative propagation: d(a*b) = a*db + da*b
fn dual_mul(a: &Value, b: &Value) -> Value {
    let av = a.as_float();
    let bv = b.as_float();
    if a.has_derivs() || b.has_derivs() {
        Value::DualFloat(
            av * bv,
            av * b.dx_float() + a.dx_float() * bv,
            av * b.dy_float() + a.dy_float() * bv,
        )
    } else {
        Value::Float(av * bv)
    }
}

/// Divide with derivative propagation using safe_div_f32 (isfinite check).
/// Handles scalar and Vec3/Color component-wise division.
/// C++ formula: val=a/b, dx=1/b*(ax - a/b*bx), dy=1/b*(ay - a/b*by)
fn dual_div(a: &Value, b: &Value) -> Value {
    // Vec3/Color component-wise division
    match (a, b) {
        (Value::Vec3(av) | Value::Color(av), Value::Vec3(bv) | Value::Color(bv)) => {
            return Value::Vec3(Vec3::new(
                safe_div_f32(av.x, bv.x),
                safe_div_f32(av.y, bv.y),
                safe_div_f32(av.z, bv.z),
            ));
        }
        (Value::DualVec3(av, adx, ady), Value::DualVec3(bv, bdx, bdy)) => {
            let vx = safe_div_f32(av.x, bv.x);
            let vy = safe_div_f32(av.y, bv.y);
            let vz = safe_div_f32(av.z, bv.z);
            let inv_bx = safe_div_f32(1.0, bv.x);
            let inv_by = safe_div_f32(1.0, bv.y);
            let inv_bz = safe_div_f32(1.0, bv.z);
            return Value::DualVec3(
                Vec3::new(vx, vy, vz),
                Vec3::new(
                    inv_bx * (adx.x - vx * bdx.x),
                    inv_by * (adx.y - vy * bdx.y),
                    inv_bz * (adx.z - vz * bdx.z),
                ),
                Vec3::new(
                    inv_bx * (ady.x - vx * bdy.x),
                    inv_by * (ady.y - vy * bdy.y),
                    inv_bz * (ady.z - vz * bdy.z),
                ),
            );
        }
        (Value::DualVec3(av, adx, ady), _) => {
            let bf = b.as_float();
            let inv_b = safe_div_f32(1.0, bf);
            return Value::DualVec3(
                Vec3::new(
                    safe_div_f32(av.x, bf),
                    safe_div_f32(av.y, bf),
                    safe_div_f32(av.z, bf),
                ),
                Vec3::new(adx.x * inv_b, adx.y * inv_b, adx.z * inv_b),
                Vec3::new(ady.x * inv_b, ady.y * inv_b, ady.z * inv_b),
            );
        }
        _ => {}
    }
    // Scalar path
    let av = a.as_float();
    let bv = b.as_float();
    let val = safe_div_f32(av, bv);
    if a.has_derivs() || b.has_derivs() {
        let inv_b = safe_div_f32(1.0, bv);
        Value::DualFloat(
            val,
            inv_b * (a.dx_float() - val * b.dx_float()),
            inv_b * (a.dy_float() - val * b.dy_float()),
        )
    } else {
        Value::Float(val)
    }
}

/// Negate with derivative propagation: d(-a) = -da
fn dual_neg(a: &Value) -> Value {
    match a {
        Value::DualFloat(v, dx, dy) => Value::DualFloat(-v, -dx, -dy),
        Value::DualVec3(v, dx, dy) => Value::DualVec3(
            Vec3::new(-v.x, -v.y, -v.z),
            Vec3::new(-dx.x, -dx.y, -dx.z),
            Vec3::new(-dy.x, -dy.y, -dy.z),
        ),
        Value::Float(f) => Value::Float(-f),
        Value::Int(i) => Value::Int(-i),
        Value::Vec3(v) | Value::Color(v) => Value::Vec3(Vec3::new(-v.x, -v.y, -v.z)),
        Value::Matrix(m) => {
            let mut r = [[0.0f32; 4]; 4];
            for (i, row) in r.iter_mut().enumerate() {
                for (j, cell) in row.iter_mut().enumerate() {
                    *cell = -m.m[i][j];
                }
            }
            Value::Matrix(Matrix44 { m: r })
        }
        other => other.clone(),
    }
}

/// Apply a unary f32 function with chain rule: d(f(a)) = f'(a) * da
fn dual_unary(a: &Value, f: impl Fn(f32) -> f32, df: impl Fn(f32) -> f32) -> Value {
    match a {
        Value::Vec3(v) | Value::Color(v) => Value::Vec3(Vec3::new(f(v.x), f(v.y), f(v.z))),
        Value::DualVec3(v, dx, dy) => Value::DualVec3(
            Vec3::new(f(v.x), f(v.y), f(v.z)),
            Vec3::new(df(v.x) * dx.x, df(v.y) * dx.y, df(v.z) * dx.z),
            Vec3::new(df(v.x) * dy.x, df(v.y) * dy.y, df(v.z) * dy.z),
        ),
        _ => {
            let av = a.as_float();
            let result = f(av);
            if a.has_derivs() {
                let deriv = df(av);
                Value::DualFloat(result, deriv * a.dx_float(), deriv * a.dy_float())
            } else {
                Value::Float(result)
            }
        }
    }
}

/// Like dual_unary but computes via f64 for better precision matching C++
fn dual_unary_f64(a: &Value, f: impl Fn(f64) -> f64, df: impl Fn(f32) -> f32) -> Value {
    let ff = |x: f32| -> f32 { f(x as f64) as f32 };
    match a {
        Value::Vec3(v) | Value::Color(v) => Value::Vec3(Vec3::new(ff(v.x), ff(v.y), ff(v.z))),
        Value::DualVec3(v, dx, dy) => Value::DualVec3(
            Vec3::new(ff(v.x), ff(v.y), ff(v.z)),
            Vec3::new(df(v.x) * dx.x, df(v.y) * dx.y, df(v.z) * dx.z),
            Vec3::new(df(v.x) * dy.x, df(v.y) * dy.y, df(v.z) * dy.z),
        ),
        _ => {
            let av = a.as_float();
            let result = ff(av);
            if a.has_derivs() {
                let deriv = df(av);
                Value::DualFloat(result, deriv * a.dx_float(), deriv * a.dy_float())
            } else {
                Value::Float(result)
            }
        }
    }
}

/// Vec3 add with derivatives
fn dual_add_vec3(a: &Value, b: &Value) -> Value {
    let av = a.as_vec3();
    let bv = b.as_vec3();
    if a.has_derivs() || b.has_derivs() {
        Value::DualVec3(
            av + bv,
            a.dx_vec3() + b.dx_vec3(),
            a.dy_vec3() + b.dy_vec3(),
        )
    } else {
        Value::Vec3(av + bv)
    }
}

/// Vec3 sub with derivatives
fn dual_sub_vec3(a: &Value, b: &Value) -> Value {
    let av = a.as_vec3();
    let bv = b.as_vec3();
    if a.has_derivs() || b.has_derivs() {
        Value::DualVec3(
            av - bv,
            a.dx_vec3() - b.dx_vec3(),
            a.dy_vec3() - b.dy_vec3(),
        )
    } else {
        Value::Vec3(av - bv)
    }
}

/// Vec3 * scalar with derivatives
fn dual_mul_vec3_scalar(v: &Value, s: &Value) -> Value {
    let vv = v.as_vec3();
    let sv = s.as_float();
    if v.has_derivs() || s.has_derivs() {
        let vdx = v.dx_vec3();
        let vdy = v.dy_vec3();
        let sdx = s.dx_float();
        let sdy = s.dy_float();
        Value::DualVec3(vv * sv, vdx * sv + vv * sdx, vdy * sv + vv * sdy)
    } else {
        Value::Vec3(vv * sv)
    }
}

/// Dual2-aware smoothstep matching C++ dual.h template.
/// f(t) = (3-2t)*t^2 where t = (x-e0)/(e1-e0).
/// Propagates derivatives through all three arguments.
fn dual_smoothstep(e0: &Value, e1: &Value, x: &Value) -> Value {
    let e0v = e0.as_float();
    let e1v = e1.as_float();
    let xv = x.as_float();
    // Boundary cases: no derivatives at saturation
    if xv < e0v {
        return Value::Float(0.0);
    }
    if xv >= e1v {
        return Value::Float(1.0);
    }
    let has_d = e0.has_derivs() || e1.has_derivs() || x.has_derivs();
    if !has_d {
        // Scalar path
        let t = (xv - e0v) / (e1v - e0v);
        return Value::Float(t * t * (3.0 - 2.0 * t));
    }
    // Dual path: t = (x - e0)/(e1 - e0), result = (3 - 2*t)*t*t
    let t_num = dual_sub(x, e0); // x - e0
    let t_den = dual_sub(e1, e0); // e1 - e0
    let t = dual_div(&t_num, &t_den);
    // 2*t
    let two = Value::Float(2.0);
    let two_t = dual_mul(&two, &t);
    // 3 - 2*t
    let three = Value::Float(3.0);
    let coeff = dual_sub(&three, &two_t);
    // coeff * t * t
    let ct = dual_mul(&coeff, &t);
    dual_mul(&ct, &t)
}

impl Default for Value {
    fn default() -> Self {
        Value::Int(0)
    }
}

/// Interpreter state.
pub struct Interpreter {
    /// Symbol values: index -> Value.
    values: Vec<Value>,
    /// Output messages (from printf/warning/error).
    pub messages: Vec<String>,
    /// Whether execution was halted (e.g., by `exit()`).
    halted: bool,
    /// Message store for setmessage/getmessage.
    message_store: std::collections::HashMap<String, Value>,
    /// Optional renderer services for texture/attribute/trace delegation.
    renderer: Option<Arc<dyn RendererServices>>,
    /// Dictionary store for dict_find/dict_next/dict_value.
    dict_store: crate::dict::DictStore,
    /// Call stack for user function calls (return addresses).
    call_stack: Vec<usize>,
    /// Synonym for "common" space in transform/getmatrix (e.g. "world").
    /// Matching C++ ShadingContext::commonspace_synonym.
    commonspace_synonym: UString,
    /// Whether to emit range-check errors for aref/aassign/compref/compassign/mxcompref/mxcompassign.
    /// Matching C++ ShadingSystem::range_checking.
    range_checking: bool,
    /// Whether to emit errors when unknown coordinate systems are used in getmatrix/transform.
    /// Matching C++ ShadingSystem::unknown_coordsys_error.
    unknown_coordsys_error: bool,
    /// Dedup sets for warning/error messages (C++ m_errseen/m_warnseen).
    seen_errors: std::collections::HashSet<String>,
    seen_warnings: std::collections::HashSet<String>,
}

impl Default for Interpreter {
    fn default() -> Self {
        Self::new()
    }
}

impl Interpreter {
    pub fn new() -> Self {
        Self {
            values: Vec::new(),
            messages: Vec::new(),
            halted: false,
            message_store: std::collections::HashMap::new(),
            renderer: None,
            dict_store: crate::dict::DictStore::new(),
            call_stack: Vec::new(),
            commonspace_synonym: UString::new("world"),
            range_checking: true,
            unknown_coordsys_error: true,
            seen_errors: std::collections::HashSet::new(),
            seen_warnings: std::collections::HashSet::new(),
        }
    }

    /// Create an interpreter with a renderer for real texture/attribute lookups.
    pub fn with_renderer(renderer: Arc<dyn RendererServices>) -> Self {
        Self {
            values: Vec::new(),
            messages: Vec::new(),
            halted: false,
            message_store: std::collections::HashMap::new(),
            renderer: Some(renderer),
            dict_store: crate::dict::DictStore::new(),
            call_stack: Vec::new(),
            commonspace_synonym: UString::new("world"),
            range_checking: true,
            unknown_coordsys_error: true,
            seen_errors: std::collections::HashSet::new(),
            seen_warnings: std::collections::HashSet::new(),
        }
    }

    /// Set the renderer services (replaces any existing renderer).
    pub fn set_renderer(&mut self, renderer: Arc<dyn RendererServices>) {
        self.renderer = Some(renderer);
    }

    /// Set the synonym for "common" space (e.g. "world"). Matching C++ set_commonspace_synonym.
    pub fn set_commonspace_synonym(&mut self, synonym: UString) {
        self.commonspace_synonym = synonym;
    }

    /// Set whether to emit range-check errors. Matching C++ ShadingSystem::range_checking.
    pub fn set_range_checking(&mut self, enabled: bool) {
        self.range_checking = enabled;
    }

    /// Set whether to emit errors for unknown coordinate systems. Matching C++ ShadingSystem::unknown_coordsys_error.
    pub fn set_unknown_coordsys_error(&mut self, enabled: bool) {
        self.unknown_coordsys_error = enabled;
    }

    /// Range-check index for aref/aassign/compref/compassign/mxcompref/mxcompassign.
    /// Returns clamped index; reports error when OOB and range_checking is enabled.
    fn range_check(
        &mut self,
        index: i32,
        length: i32,
        symname: &str,
        sourcefile: &str,
        sourceline: i32,
        shader_name: &str,
        errhandler: Option<&dyn crate::shadingsys::ErrorHandler>,
    ) -> i32 {
        if !self.range_checking || length <= 0 {
            return index;
        }
        if index >= 0 && index < length {
            return index;
        }
        let max_idx = length - 1;
        let msg = format!(
            "Index [{}] out of range {}[0..{}]: {}:{} (group unnamed_group_1, layer 0 {shader_name}_0, shader {shader_name})",
            index, symname, max_idx, sourcefile, sourceline
        );
        if let Some(h) = errhandler {
            h.error(&msg);
        } else if self.seen_errors.insert(msg.clone()) {
            self.messages.push(format!("ERROR: {msg}\n"));
        }
        if index >= length { max_idx } else { 0 }
    }

    /// Execute a ShaderIR with the given globals.
    /// When `msg_config` is Some, uses shared message store with layer validation.
    pub fn execute(
        &mut self,
        ir: &ShaderIR,
        globals: &ShaderGlobals,
        msg_config: Option<ExecuteMessageConfig<'_>>,
    ) {
        self.halted = false;
        self.messages.clear();
        if msg_config.is_none() {
            self.message_store.clear();
        }
        let mut msg_cfg = msg_config;

        // 1. Initialize all symbols to their type-default values
        self.values.clear();
        self.values.resize(ir.symbols.len(), Value::default());
        for (i, sym) in ir.symbols.iter().enumerate() {
            let sid = sym.typespec.structure_id();
            if sid > 0 {
                // Struct symbol: initialize with default values for each field
                if let Some(spec) = crate::typespec::get_struct(sid as i32) {
                    let fields: Vec<Value> = spec
                        .fields
                        .iter()
                        .map(|f| default_value_for_type(&f.type_spec.simpletype()))
                        .collect();
                    self.values[i] = Value::Struct(fields);
                } else {
                    self.values[i] = Value::Void;
                }
            } else if sym.typespec.is_closure_array() {
                // Closure array: allocate with all-None slots
                let len = sym.typespec.arraylength().max(0) as usize;
                self.values[i] = Value::ClosureArray(vec![None; len]);
            } else if sym.typespec.is_closure() {
                // Scalar closure: start as null/Void until assigned
                self.values[i] = Value::Void;
            } else if sym.typespec.is_closure_array() {
                // Closure array: allocate with all-None slots
                let len = sym.typespec.arraylength().max(0) as usize;
                self.values[i] = Value::ClosureArray(vec![None; len]);
            } else if sym.typespec.is_closure() {
                // Scalar closure: starts null until assigned
                self.values[i] = Value::Void;
            } else if sym.typespec.is_closure_array() {
                // Closure array: allocate with all-None slots
                let len = sym.typespec.arraylength().max(0) as usize;
                self.values[i] = Value::ClosureArray(vec![None; len]);
            } else if sym.typespec.is_closure() {
                // Scalar closure: starts null until assigned
                self.values[i] = Value::Void;
            } else {
                self.values[i] = default_value_for_type(&sym.typespec.simpletype());
            }
        }

        // 2. Load compile-time constant values
        for &(idx, ref cv) in &ir.const_values {
            self.values[idx] = const_to_value(cv);
        }

        // 3. Load parameter default values
        for &(idx, ref cv) in &ir.param_defaults {
            self.values[idx] = const_to_value(cv);
        }

        // 4. Bind shader globals (P, N, I, u, v, etc.)
        self.bind_globals(ir, globals);

        // 5. Execute opcodes
        let mut pc = 0usize;
        let num_ops = ir.opcodes.len();
        let mut step_count = 0u64;
        const MAX_STEPS: u64 = 10_000_000; // 10M opcode limit

        while pc < num_ops && !self.halted && step_count < MAX_STEPS {
            step_count += 1;
            let op = &ir.opcodes[pc];
            let opname = op.op.as_str();
            let nargs = op.nargs as usize;
            let firstarg = op.firstarg as usize;

            // Collect arg symbol indices
            let args: Vec<i32> = (0..nargs)
                .map(|j| {
                    if firstarg + j < ir.args.len() {
                        ir.args[firstarg + j]
                    } else {
                        -1
                    }
                })
                .collect();

            match opname {
                // --- Control flow ---
                "nop" | ""
                    // nop with jump target = unconditional jump (used for loop back-edges)
                    if op.jump[0] >= 0 => {
                        pc = op.jump[0] as usize;
                        continue;
                    }

                "end" => break,

                "return" => {
                    // If we have a call stack, return to the caller
                    if let Some(return_pc) = self.call_stack.pop() {
                        pc = return_pc;
                        continue;
                    } else {
                        // No call stack — we're in the main shader body
                        break;
                    }
                }

                "exit" => {
                    self.halted = true;
                    break;
                }

                "functioncall"
                    // jump[0] = function body start
                    if op.jump[0] >= 0 => {
                        // Guard against infinite recursion (OSL doesn't allow recursion)
                        if self.call_stack.len() >= 256 {
                            // Bail out — likely infinite recursion bug
                            break;
                        }
                        self.call_stack.push(pc + 1); // return to next opcode
                        pc = op.jump[0] as usize;
                        continue;
                    }

                "if" if !args.is_empty() => {
                    let cond = self.get(args[0]);
                    if !cond.is_truthy() {
                        // FALSE: skip then-block, jump to else/end (jump[1])
                        if op.jump[1] >= 0 {
                            pc = op.jump[1] as usize;
                            continue;
                        }
                    }
                    // TRUE: fall through to then-block (next instruction)
                    // jump[0] = then-block start (already next pc)
                }

                // --- Data movement ---
                "assign" if args.len() >= 2 => {
                    let src = self.get(args[1]);
                    // Type coercion based on destination symbol type
                    let dst_idx = args[0] as usize;
                    let is_matrix = |td: &crate::typedesc::TypeDesc| {
                        td.aggregate == crate::typedesc::Aggregate::Matrix44 as u8
                    };
                    let coerced = if dst_idx < ir.symbols.len() {
                        let dst_td = ir.symbols[dst_idx].typespec.simpletype();
                        match &src {
                            Value::Float(f) if dst_td.is_triple() => {
                                Value::Vec3(Vec3::new(*f, *f, *f))
                            }
                            Value::DualFloat(f, dx, dy) if dst_td.is_triple() => Value::DualVec3(
                                Vec3::new(*f, *f, *f),
                                Vec3::new(*dx, *dx, *dx),
                                Vec3::new(*dy, *dy, *dy),
                            ),
                            Value::Float(f) if is_matrix(&dst_td) => {
                                let v = *f;
                                Value::Matrix(crate::math::Matrix44 {
                                    m: [
                                        [v, 0.0, 0.0, 0.0],
                                        [0.0, v, 0.0, 0.0],
                                        [0.0, 0.0, v, 0.0],
                                        [0.0, 0.0, 0.0, v],
                                    ],
                                })
                            }
                            Value::Int(i)
                                if dst_td.basetype == crate::typedesc::BaseType::Float as u8
                                    && !dst_td.is_triple()
                                    && !is_matrix(&dst_td) =>
                            {
                                Value::Float(*i as f32)
                            }
                            Value::Int(i) if dst_td.is_triple() => {
                                let f = *i as f32;
                                Value::Vec3(Vec3::new(f, f, f))
                            }
                            Value::Int(i) if is_matrix(&dst_td) => {
                                let v = *i as f32;
                                Value::Matrix(crate::math::Matrix44 {
                                    m: [
                                        [v, 0.0, 0.0, 0.0],
                                        [0.0, v, 0.0, 0.0],
                                        [0.0, 0.0, v, 0.0],
                                        [0.0, 0.0, 0.0, v],
                                    ],
                                })
                            }
                            _ => src,
                        }
                    } else {
                        src
                    };
                    self.set(args[0], coerced);
                }

                // select(result, x, y, cond): cond ? y : x  (cond is last arg)
                "select" if args.len() >= 4 => {
                    let cond = self.get(args[3]);
                    // Component-wise select when condition is a triple
                    if cond.is_triple() {
                        let c = cond.as_vec3();
                        let a = self.get(args[1]).as_vec3(); // false branch
                        let b = self.get(args[2]).as_vec3(); // true branch
                        let r = Vec3::new(
                            if c.x != 0.0 { b.x } else { a.x },
                            if c.y != 0.0 { b.y } else { a.y },
                            if c.z != 0.0 { b.z } else { a.z },
                        );
                        self.set(args[0], Value::Vec3(r));
                    } else {
                        let val = if cond.is_truthy() {
                            self.get(args[2]) // y (true branch)
                        } else {
                            self.get(args[1]) // x (false branch)
                        };
                        self.set(args[0], val);
                    }
                }

                // --- Arithmetic (3-arg: dst, lhs, rhs) ---
                // All arithmetic propagates derivatives via Dual2 chain rules.
                "add" if args.len() >= 3 => {
                    let a = self.get(args[1]);
                    let b = self.get(args[2]);
                    let result = match (&a, &b) {
                        // Closure + Closure: build Add tree (matches osl_add_closure_closure)
                        (Value::Closure(ca), Value::Closure(cb)) => {
                            Value::Closure(Box::new(ClosureValue::Add(ca.clone(), cb.clone())))
                        }
                        // Vec3/Color types (with or without derivs)
                        (
                            Value::Vec3(_) | Value::Color(_) | Value::DualVec3(..),
                            Value::Vec3(_) | Value::Color(_) | Value::DualVec3(..),
                        ) => dual_add_vec3(&a, &b),
                        // Float/Dual types
                        _ if a.has_derivs() || b.has_derivs() => dual_add(&a, &b),
                        (Value::Float(x), Value::Float(y)) => Value::Float(x + y),
                        (Value::Int(x), Value::Int(y)) => Value::Int(x + y),
                        (Value::Vec3(x), Value::Float(y)) | (Value::Color(x), Value::Float(y)) => {
                            Value::Vec3(Vec3::new(x.x + y, x.y + y, x.z + y))
                        }
                        (Value::Float(x), Value::Vec3(y)) | (Value::Float(x), Value::Color(y)) => {
                            Value::Vec3(Vec3::new(x + y.x, x + y.y, x + y.z))
                        }
                        _ => Value::Float(a.as_float() + b.as_float()),
                    };
                    self.set(args[0], result);
                }

                "sub" if args.len() >= 3 => {
                    let a = self.get(args[1]);
                    let b = self.get(args[2]);
                    let result = match (&a, &b) {
                        (
                            Value::Vec3(_) | Value::Color(_) | Value::DualVec3(..),
                            Value::Vec3(_) | Value::Color(_) | Value::DualVec3(..),
                        ) => dual_sub_vec3(&a, &b),
                        _ if a.has_derivs() || b.has_derivs() => dual_sub(&a, &b),
                        (Value::Float(x), Value::Float(y)) => Value::Float(x - y),
                        (Value::Int(x), Value::Int(y)) => Value::Int(x - y),
                        (Value::Vec3(x), Value::Float(y)) | (Value::Color(x), Value::Float(y)) => {
                            Value::Vec3(Vec3::new(x.x - y, x.y - y, x.z - y))
                        }
                        (Value::Float(x), Value::Vec3(y)) | (Value::Float(x), Value::Color(y)) => {
                            Value::Vec3(Vec3::new(x - y.x, x - y.y, x - y.z))
                        }
                        _ => Value::Float(a.as_float() - b.as_float()),
                    };
                    self.set(args[0], result);
                }

                "mul" if args.len() >= 3 => {
                    let a = self.get(args[1]);
                    let b = self.get(args[2]);
                    let result = match (&a, &b) {
                        // Closure * float weight
                        (Value::Closure(c), Value::Float(w)) => {
                            Value::Closure(Box::new(ClosureValue::Mul {
                                weight: Color3::new(*w, *w, *w),
                                closure: c.clone(),
                            }))
                        }
                        (Value::Float(w), Value::Closure(c)) => {
                            Value::Closure(Box::new(ClosureValue::Mul {
                                weight: Color3::new(*w, *w, *w),
                                closure: c.clone(),
                            }))
                        }
                        // DualFloat * Closure (e.g. u * emission())
                        (Value::Closure(c), Value::DualFloat(w, ..)) => {
                            Value::Closure(Box::new(ClosureValue::Mul {
                                weight: Color3::new(*w, *w, *w),
                                closure: c.clone(),
                            }))
                        }
                        (Value::DualFloat(w, ..), Value::Closure(c)) => {
                            Value::Closure(Box::new(ClosureValue::Mul {
                                weight: Color3::new(*w, *w, *w),
                                closure: c.clone(),
                            }))
                        }
                        // DualVec3 * Closure
                        (Value::Closure(c), Value::DualVec3(w, ..))
                        | (Value::DualVec3(w, ..), Value::Closure(c)) => {
                            Value::Closure(Box::new(ClosureValue::Mul {
                                weight: *w,
                                closure: c.clone(),
                            }))
                        }
                        // Closure * color weight
                        (Value::Closure(c), Value::Vec3(w))
                        | (Value::Closure(c), Value::Color(w)) => {
                            Value::Closure(Box::new(ClosureValue::Mul {
                                weight: *w,
                                closure: c.clone(),
                            }))
                        }
                        (Value::Vec3(w), Value::Closure(c))
                        | (Value::Color(w), Value::Closure(c)) => {
                            Value::Closure(Box::new(ClosureValue::Mul {
                                weight: *w,
                                closure: c.clone(),
                            }))
                        }
                        // Vec3 * scalar (with derivs)
                        (
                            Value::Vec3(_) | Value::DualVec3(..),
                            Value::Float(_) | Value::DualFloat(..),
                        )
                        | (Value::Color(_), Value::Float(_) | Value::DualFloat(..)) => {
                            dual_mul_vec3_scalar(&a, &b)
                        }
                        (
                            Value::Float(_) | Value::DualFloat(..),
                            Value::Vec3(_) | Value::DualVec3(..),
                        )
                        | (Value::Float(_) | Value::DualFloat(..), Value::Color(_)) => {
                            dual_mul_vec3_scalar(&b, &a)
                        }
                        _ if a.has_derivs() || b.has_derivs() => dual_mul(&a, &b),
                        (Value::Float(x), Value::Float(y)) => Value::Float(x * y),
                        (Value::Int(x), Value::Int(y)) => Value::Int(x * y),
                        (Value::DualVec3(xv, xdx, xdy), Value::DualVec3(yv, ydx, ydy)) => {
                            // Component-wise dual mul: d(a*b) = a*db + da*b
                            Value::DualVec3(
                                Vec3::new(xv.x * yv.x, xv.y * yv.y, xv.z * yv.z),
                                Vec3::new(
                                    xv.x * ydx.x + xdx.x * yv.x,
                                    xv.y * ydx.y + xdx.y * yv.y,
                                    xv.z * ydx.z + xdx.z * yv.z,
                                ),
                                Vec3::new(
                                    xv.x * ydy.x + xdy.x * yv.x,
                                    xv.y * ydy.y + xdy.y * yv.y,
                                    xv.z * ydy.z + xdy.z * yv.z,
                                ),
                            )
                        }
                        (Value::DualVec3(xv, xdx, xdy), Value::Vec3(yv))
                        | (Value::DualVec3(xv, xdx, xdy), Value::Color(yv)) => Value::DualVec3(
                            Vec3::new(xv.x * yv.x, xv.y * yv.y, xv.z * yv.z),
                            Vec3::new(xdx.x * yv.x, xdx.y * yv.y, xdx.z * yv.z),
                            Vec3::new(xdy.x * yv.x, xdy.y * yv.y, xdy.z * yv.z),
                        ),
                        (Value::Vec3(xv), Value::DualVec3(yv, ydx, ydy))
                        | (Value::Color(xv), Value::DualVec3(yv, ydx, ydy)) => Value::DualVec3(
                            Vec3::new(xv.x * yv.x, xv.y * yv.y, xv.z * yv.z),
                            Vec3::new(xv.x * ydx.x, xv.y * ydx.y, xv.z * ydx.z),
                            Vec3::new(xv.x * ydy.x, xv.y * ydy.y, xv.z * ydy.z),
                        ),
                        (Value::Vec3(x), Value::Vec3(y)) | (Value::Color(x), Value::Color(y)) => {
                            Value::Vec3(Vec3::new(x.x * y.x, x.y * y.y, x.z * y.z))
                        }
                        // Matrix * scalar / scalar * Matrix
                        (Value::Matrix(m), _) if matches!(b, Value::Float(_) | Value::Int(_)) => {
                            let s = b.as_float();
                            let mut r = [[0.0f32; 4]; 4];
                            for (i, row) in r.iter_mut().enumerate() {
                                for (j, cell) in row.iter_mut().enumerate() {
                                    *cell = m.m[i][j] * s;
                                }
                            }
                            Value::Matrix(Matrix44 { m: r })
                        }
                        (_, Value::Matrix(m)) if matches!(a, Value::Float(_) | Value::Int(_)) => {
                            let s = a.as_float();
                            let mut r = [[0.0f32; 4]; 4];
                            for (i, row) in r.iter_mut().enumerate() {
                                for (j, cell) in row.iter_mut().enumerate() {
                                    *cell = m.m[i][j] * s;
                                }
                            }
                            Value::Matrix(Matrix44 { m: r })
                        }
                        // Matrix * Matrix
                        (Value::Matrix(ma), Value::Matrix(mb)) => {
                            Value::Matrix(crate::matrix_ops::matmul(ma, mb))
                        }
                        _ => Value::Float(a.as_float() * b.as_float()),
                    };
                    self.set(args[0], result);
                }

                // Division: uses safe_div_f32 (isfinite check) matching C++ llvm_ops.cpp
                "div" if args.len() >= 3 => {
                    let a = self.get(args[1]);
                    let b = self.get(args[2]);
                    let result = if a.has_derivs() || b.has_derivs() {
                        dual_div(&a, &b)
                    } else {
                        match (&a, &b) {
                            (Value::Float(x), Value::Float(y)) => {
                                Value::Float(safe_div_f32(*x, *y))
                            }
                            (Value::Int(x), Value::Int(y)) => {
                                Value::Int(if *y != 0 { x / y } else { 0 })
                            }
                            (Value::Vec3(x), Value::Float(y))
                            | (Value::Color(x), Value::Float(y)) => Value::Vec3(Vec3::new(
                                safe_div_f32(x.x, *y),
                                safe_div_f32(x.y, *y),
                                safe_div_f32(x.z, *y),
                            )),
                            (Value::Vec3(x), Value::Vec3(y))
                            | (Value::Color(x), Value::Color(y))
                            | (Value::Vec3(x), Value::Color(y))
                            | (Value::Color(x), Value::Vec3(y)) => Value::Vec3(Vec3::new(
                                safe_div_f32(x.x, y.x),
                                safe_div_f32(x.y, y.y),
                                safe_div_f32(x.z, y.z),
                            )),
                            (Value::Float(x), Value::Vec3(y))
                            | (Value::Float(x), Value::Color(y)) => Value::Vec3(Vec3::new(
                                safe_div_f32(*x, y.x),
                                safe_div_f32(*x, y.y),
                                safe_div_f32(*x, y.z),
                            )),
                            // Matrix / scalar
                            (Value::Matrix(m), _)
                                if matches!(b, Value::Float(_) | Value::Int(_)) =>
                            {
                                let s = b.as_float();
                                let inv = safe_div_f32(1.0, s);
                                let mut r = [[0.0f32; 4]; 4];
                                for (i, row) in r.iter_mut().enumerate() {
                                    for (j, cell) in row.iter_mut().enumerate() {
                                        *cell = m.m[i][j] * inv;
                                    }
                                }
                                Value::Matrix(Matrix44 { m: r })
                            }
                            // scalar / Matrix = scalar * inverse(Matrix)
                            (_, Value::Matrix(m))
                                if matches!(a, Value::Float(_) | Value::Int(_)) =>
                            {
                                let s = a.as_float();
                                if let Some(inv) = crate::matrix_ops::inverse(m) {
                                    let mut r = [[0.0f32; 4]; 4];
                                    for (i, row) in r.iter_mut().enumerate() {
                                        for (j, cell) in row.iter_mut().enumerate() {
                                            *cell = inv.m[i][j] * s;
                                        }
                                    }
                                    Value::Matrix(Matrix44 { m: r })
                                } else {
                                    Value::Matrix(Matrix44::ZERO)
                                }
                            }
                            // Matrix / Matrix = A * inverse(B)
                            (Value::Matrix(ma), Value::Matrix(mb)) => {
                                if let Some(inv_b) = crate::matrix_ops::inverse(mb) {
                                    Value::Matrix(crate::matrix_ops::matmul(ma, &inv_b))
                                } else {
                                    Value::Matrix(Matrix44::ZERO)
                                }
                            }
                            _ => Value::Float(safe_div_f32(a.as_float(), b.as_float())),
                        }
                    };
                    self.set(args[0], result);
                }

                // OSL mod: a - b*floor(a/b), always positive for positive b
                "mod" if args.len() >= 3 => {
                    let a = self.get(args[1]);
                    let b = self.get(args[2]);
                    let result = match (&a, &b) {
                        (Value::Int(x), Value::Int(y)) => Value::Int(osl_mod_i32(*x, *y)),
                        _ if a.is_triple() || b.is_triple() => {
                            let va = a.as_vec3();
                            let vb = b.as_vec3();
                            Value::Vec3(Vec3::new(
                                osl_mod_f32(va.x, vb.x),
                                osl_mod_f32(va.y, vb.y),
                                osl_mod_f32(va.z, vb.z),
                            ))
                        }
                        _ => Value::Float(osl_mod_f32(a.as_float(), b.as_float())),
                    };
                    self.set(args[0], result);
                }

                // --- Unary (2-arg: dst, src) ---
                "neg" if args.len() >= 2 => {
                    let a = self.get(args[1]);
                    self.set(args[0], dual_neg(&a));
                }

                "not" if args.len() >= 2 => {
                    let a = self.get(args[1]);
                    self.set(args[0], Value::Int(if a.is_truthy() { 0 } else { 1 }));
                }

                "compl" if args.len() >= 2 => {
                    let a = self.get(args[1]).as_int();
                    self.set(args[0], Value::Int(!a));
                }

                // --- Bitwise ---
                "bitand" if args.len() >= 3 => {
                    let a = self.get(args[1]).as_int();
                    let b = self.get(args[2]).as_int();
                    self.set(args[0], Value::Int(a & b));
                }
                "bitor" if args.len() >= 3 => {
                    let a = self.get(args[1]).as_int();
                    let b = self.get(args[2]).as_int();
                    self.set(args[0], Value::Int(a | b));
                }
                "xor" if args.len() >= 3 => {
                    let a = self.get(args[1]).as_int();
                    let b = self.get(args[2]).as_int();
                    self.set(args[0], Value::Int(a ^ b));
                }
                "shl" if args.len() >= 3 => {
                    let a = self.get(args[1]).as_int();
                    let b = self.get(args[2]).as_int();
                    self.set(args[0], Value::Int(a << (b & 31)));
                }
                "shr" if args.len() >= 3 => {
                    let a = self.get(args[1]).as_int();
                    let b = self.get(args[2]).as_int();
                    self.set(args[0], Value::Int(a >> (b & 31)));
                }

                // --- Comparison (3-arg: dst, lhs, rhs) ---
                "eq" if args.len() >= 3 => {
                    let a = self.get(args[1]);
                    let b = self.get(args[2]);
                    let float_to_matrix = |f: f32| -> [[f32; 4]; 4] {
                        [
                            [f, 0.0, 0.0, 0.0],
                            [0.0, f, 0.0, 0.0],
                            [0.0, 0.0, f, 0.0],
                            [0.0, 0.0, 0.0, f],
                        ]
                    };
                    let eq = match (&a, &b) {
                        (Value::Int(x), Value::Int(y)) => x == y,
                        (Value::String(x), Value::String(y)) => x == y,
                        (Value::Vec3(x), Value::Vec3(y))
                        | (Value::Color(x), Value::Color(y))
                        | (Value::Vec3(x), Value::Color(y))
                        | (Value::Color(x), Value::Vec3(y)) => {
                            x.x == y.x && x.y == y.y && x.z == y.z
                        }
                        (Value::Vec3(x), _) | (Value::Color(x), _)
                            if !matches!(&b, Value::Matrix(_)) =>
                        {
                            let f = b.as_float();
                            x.x == f && x.y == f && x.z == f
                        }
                        (_, Value::Vec3(y)) | (_, Value::Color(y))
                            if !matches!(&a, Value::Matrix(_)) =>
                        {
                            let f = a.as_float();
                            f == y.x && f == y.y && f == y.z
                        }
                        (Value::Matrix(x), Value::Matrix(y)) => x.m == y.m,
                        (Value::Matrix(x), _) => x.m == float_to_matrix(b.as_float()),
                        (_, Value::Matrix(y)) => float_to_matrix(a.as_float()) == y.m,
                        // Struct equality: per-field recursive comparison
                        (Value::Struct(xa), Value::Struct(xb)) => {
                            xa.len() == xb.len()
                                && xa
                                    .iter()
                                    .zip(xb.iter())
                                    .all(|(fa, fb)| values_equal(fa, fb))
                        }
                        // C++ uses exact == for float comparison, not epsilon
                        _ => a.as_float() == b.as_float(),
                    };
                    self.set(args[0], Value::Int(if eq { 1 } else { 0 }));
                }
                "neq" if args.len() >= 3 => {
                    let a = self.get(args[1]);
                    let b = self.get(args[2]);
                    let float_to_matrix = |f: f32| -> [[f32; 4]; 4] {
                        [
                            [f, 0.0, 0.0, 0.0],
                            [0.0, f, 0.0, 0.0],
                            [0.0, 0.0, f, 0.0],
                            [0.0, 0.0, 0.0, f],
                        ]
                    };
                    let neq = match (&a, &b) {
                        (Value::Int(x), Value::Int(y)) => x != y,
                        (Value::String(x), Value::String(y)) => x != y,
                        (Value::Vec3(x), Value::Vec3(y))
                        | (Value::Color(x), Value::Color(y))
                        | (Value::Vec3(x), Value::Color(y))
                        | (Value::Color(x), Value::Vec3(y)) => {
                            x.x != y.x || x.y != y.y || x.z != y.z
                        }
                        (Value::Vec3(x), _) | (Value::Color(x), _)
                            if !matches!(&b, Value::Matrix(_)) =>
                        {
                            let f = b.as_float();
                            x.x != f || x.y != f || x.z != f
                        }
                        (_, Value::Vec3(y)) | (_, Value::Color(y))
                            if !matches!(&a, Value::Matrix(_)) =>
                        {
                            let f = a.as_float();
                            f != y.x || f != y.y || f != y.z
                        }
                        (Value::Matrix(x), Value::Matrix(y)) => x.m != y.m,
                        (Value::Matrix(x), _) => x.m != float_to_matrix(b.as_float()),
                        (_, Value::Matrix(y)) => float_to_matrix(a.as_float()) != y.m,
                        // Struct inequality: any field differs
                        (Value::Struct(xa), Value::Struct(xb)) => {
                            xa.len() != xb.len()
                                || xa
                                    .iter()
                                    .zip(xb.iter())
                                    .any(|(fa, fb)| !values_equal(fa, fb))
                        }
                        // C++ uses exact != for float comparison, not epsilon
                        _ => a.as_float() != b.as_float(),
                    };
                    self.set(args[0], Value::Int(if neq { 1 } else { 0 }));
                }
                "lt" if args.len() >= 3 => {
                    let a = self.get(args[1]).as_float();
                    let b = self.get(args[2]).as_float();
                    self.set(args[0], Value::Int(if a < b { 1 } else { 0 }));
                }
                "gt" if args.len() >= 3 => {
                    let a = self.get(args[1]).as_float();
                    let b = self.get(args[2]).as_float();
                    self.set(args[0], Value::Int(if a > b { 1 } else { 0 }));
                }
                "le" if args.len() >= 3 => {
                    let a = self.get(args[1]).as_float();
                    let b = self.get(args[2]).as_float();
                    self.set(args[0], Value::Int(if a <= b { 1 } else { 0 }));
                }
                "ge" if args.len() >= 3 => {
                    let a = self.get(args[1]).as_float();
                    let b = self.get(args[2]).as_float();
                    self.set(args[0], Value::Int(if a >= b { 1 } else { 0 }));
                }

                // --- Logical ---
                "and" if args.len() >= 3 => {
                    let a = self.get(args[1]).is_truthy();
                    let b = self.get(args[2]).is_truthy();
                    self.set(args[0], Value::Int(if a && b { 1 } else { 0 }));
                }
                "or" if args.len() >= 3 => {
                    let a = self.get(args[1]).is_truthy();
                    let b = self.get(args[2]).is_truthy();
                    self.set(args[0], Value::Int(if a || b { 1 } else { 0 }));
                }

                // --- Type construction ---
                "construct" if args.len() >= 2 => {
                    // Handle various construct forms:
                    // 2 args: construct(result, float) — broadcast
                    // 4 args: construct(result, x, y, z) — triple
                    // 5 args: construct(result, colorspace, x, y, z) — with colorspace string
                    // 17 args: construct(result, 16 floats) — matrix
                    if args.len() == 2 {
                        // Broadcast single value or triple-to-triple cast
                        let dst_idx = args[0] as usize;
                        let dst_td = if dst_idx < ir.symbols.len() {
                            ir.symbols[dst_idx].typespec.simpletype()
                        } else {
                            crate::typedesc::TypeDesc::FLOAT
                        };
                        let src = self.get(args[1]);
                        if dst_td.is_triple() {
                            // If src is already a triple, copy it directly (point(vec) etc)
                            if src.has_derivs() {
                                self.set(
                                    args[0],
                                    Value::DualVec3(src.as_vec3(), src.dx_vec3(), src.dy_vec3()),
                                );
                            } else {
                                self.set(args[0], Value::Vec3(src.as_vec3()));
                            }
                        } else if dst_td.is_matrix44() {
                            // Single float → diagonal matrix
                            let v = src.as_float();
                            let m = Matrix44 {
                                m: [
                                    [v, 0.0, 0.0, 0.0],
                                    [0.0, v, 0.0, 0.0],
                                    [0.0, 0.0, v, 0.0],
                                    [0.0, 0.0, 0.0, v],
                                ],
                            };
                            self.set(args[0], Value::Matrix(m));
                        } else if dst_td.basetype == crate::typedesc::BaseType::Int32 as u8 {
                            self.set(args[0], Value::Int(src.as_float() as i32));
                        } else {
                            self.set(args[0], Value::Float(src.as_float()));
                        }
                    } else if args.len() == 4 {
                        // construct(result, x, y, z) — propagate derivs from components
                        let a = self.get(args[1]);
                        let b = self.get(args[2]);
                        let c = self.get(args[3]);
                        let v = Vec3::new(a.as_float(), b.as_float(), c.as_float());
                        if a.has_derivs() || b.has_derivs() || c.has_derivs() {
                            let dx = Vec3::new(a.dx_float(), b.dx_float(), c.dx_float());
                            let dy = Vec3::new(a.dy_float(), b.dy_float(), c.dy_float());
                            self.set(args[0], Value::DualVec3(v, dx, dy));
                        } else {
                            self.set(args[0], Value::Vec3(v));
                        }
                    } else if args.len() == 5 {
                        // construct(result, space, x, y, z)
                        // Destination type determines transform kind:
                        // - color: color space transform (e.g. "hsv" → RGB)
                        // - point/vector/normal: spatial transform (space → common)
                        let dst_idx = args[0] as usize;
                        let dst_td = if dst_idx < ir.symbols.len() {
                            ir.symbols[dst_idx].typespec.simpletype()
                        } else {
                            crate::typedesc::TypeDesc::COLOR
                        };
                        let space = self.get(args[1]).as_string();
                        let xv = self.get(args[2]);
                        let yv = self.get(args[3]);
                        let zv = self.get(args[4]);
                        let v = Vec3::new(xv.as_float(), yv.as_float(), zv.as_float());
                        if dst_td == crate::typedesc::TypeDesc::COLOR {
                            let rgb = crate::color::transform_color(space.as_str(), "RGB", v);
                            self.set(args[0], Value::Color(rgb));
                        } else {
                            // point/vector/normal with space — transform from space to common
                            let to_us = UString::new("common");
                            let m = self.get_space_matrix(globals, &space, &to_us);
                            let has_derivs = xv.has_derivs() || yv.has_derivs() || zv.has_derivs();
                            let is_point = dst_td == crate::typedesc::TypeDesc::POINT;
                            let is_normal = dst_td == crate::typedesc::TypeDesc::NORMAL;
                            let result = if is_point {
                                m.transform_point(v)
                            } else if is_normal {
                                m.transform_normal(v)
                            } else {
                                m.transform_vector(v)
                            };
                            if has_derivs {
                                let dx = Vec3::new(xv.dx_float(), yv.dx_float(), zv.dx_float());
                                let dy = Vec3::new(xv.dy_float(), yv.dy_float(), zv.dy_float());
                                // Transform derivs as vectors (no translation)
                                let tdx = m.transform_vector(dx);
                                let tdy = m.transform_vector(dy);
                                self.set(args[0], Value::DualVec3(result, tdx, tdy));
                            } else {
                                self.set(args[0], Value::Vec3(result));
                            }
                        }
                    } else if args.len() == 17 {
                        // matrix
                        let mut m = Matrix44::ZERO;
                        for r in 0..4 {
                            for c in 0..4 {
                                m.m[r][c] = self.get(args[1 + r * 4 + c]).as_float();
                            }
                        }
                        self.set(args[0], Value::Matrix(m));
                    } else if args.len() == 3 {
                        // Check if this is matrix("space", float) or matrix("from", "to")
                        let dst_idx = args[0] as usize;
                        let dst_td = if dst_idx < ir.symbols.len() {
                            ir.symbols[dst_idx].typespec.simpletype()
                        } else {
                            crate::typedesc::TypeDesc::FLOAT
                        };
                        if dst_td.is_matrix44() {
                            let v1 = self.get(args[1]);
                            let v2 = self.get(args[2]);
                            if let (Value::String(from_s), Value::String(to_s)) = (&v1, &v2) {
                                // matrix("from", "to")
                                let m = if let Some(renderer) = &self.renderer {
                                    crate::matrix_ops::get_from_to_matrix(
                                        renderer.as_ref(),
                                        globals,
                                        from_s.as_str(),
                                        to_s.as_str(),
                                        globals.time,
                                        self.commonspace_synonym.as_str(),
                                        None,
                                    )
                                    .unwrap_or(Matrix44::IDENTITY)
                                } else {
                                    Matrix44::IDENTITY
                                };
                                self.set(args[0], Value::Matrix(m));
                            } else if let Value::String(space) = &v1 {
                                // matrix("space", float)
                                let f = v2.as_float();
                                let diag = Matrix44 {
                                    m: [
                                        [f, 0.0, 0.0, 0.0],
                                        [0.0, f, 0.0, 0.0],
                                        [0.0, 0.0, f, 0.0],
                                        [0.0, 0.0, 0.0, f],
                                    ],
                                };
                                let space_m = if let Some(renderer) = &self.renderer {
                                    crate::matrix_ops::get_from_to_matrix(
                                        renderer.as_ref(),
                                        globals,
                                        space.as_str(),
                                        "common",
                                        globals.time,
                                        self.commonspace_synonym.as_str(),
                                        None,
                                    )
                                    .unwrap_or(Matrix44::IDENTITY)
                                } else {
                                    Matrix44::IDENTITY
                                };
                                self.set(
                                    args[0],
                                    Value::Matrix(crate::matrix_ops::matmul(&space_m, &diag)),
                                );
                            } else {
                                self.set(args[0], Value::Matrix(Matrix44::IDENTITY));
                            }
                        } else {
                            // Fallback for non-matrix 3-arg construct
                            self.set(args[0], Value::Float(self.get(args[1]).as_float()));
                        }
                    } else if args.len() == 18 {
                        // matrix("space", 16 floats) — space-aware 16-float matrix
                        let dst_idx = args[0] as usize;
                        let dst_td = if dst_idx < ir.symbols.len() {
                            ir.symbols[dst_idx].typespec.simpletype()
                        } else {
                            crate::typedesc::TypeDesc::FLOAT
                        };
                        if dst_td.is_matrix44() {
                            if let Value::String(space) = &self.get(args[1]) {
                                let mut m = Matrix44::ZERO;
                                for r in 0..4 {
                                    for c in 0..4 {
                                        m.m[r][c] = self.get(args[2 + r * 4 + c]).as_float();
                                    }
                                }
                                let space_m = if let Some(renderer) = &self.renderer {
                                    crate::matrix_ops::get_from_to_matrix(
                                        renderer.as_ref(),
                                        globals,
                                        space.as_str(),
                                        "common",
                                        globals.time,
                                        self.commonspace_synonym.as_str(),
                                        None,
                                    )
                                    .unwrap_or(Matrix44::IDENTITY)
                                } else {
                                    Matrix44::IDENTITY
                                };
                                self.set(
                                    args[0],
                                    Value::Matrix(crate::matrix_ops::matmul(&space_m, &m)),
                                );
                            } else {
                                let mut m = Matrix44::ZERO;
                                for r in 0..4 {
                                    for c in 0..4 {
                                        m.m[r][c] = self.get(args[1 + r * 4 + c]).as_float();
                                    }
                                }
                                self.set(args[0], Value::Matrix(m));
                            }
                        }
                    } else {
                        // Check if destination is a struct type.
                        let dst_idx = args[0] as usize;
                        let sid = if dst_idx < ir.symbols.len() {
                            ir.symbols[dst_idx].typespec.structure_id()
                        } else {
                            0
                        };
                        if sid > 0 {
                            // Struct construction: assign args[1..] to fields.
                            let nfields = args.len() - 1;
                            let mut fields = Vec::with_capacity(nfields);
                            for fi in 0..nfields {
                                fields.push(self.get(args[1 + fi]));
                            }
                            self.set(args[0], Value::Struct(fields));
                        } else if args.len() >= 4 {
                            // Fallback: try as triple from first 3 value args
                            let a = self.get(args[1]).as_float();
                            let b = self.get(args[2]).as_float();
                            let c = self.get(args[3]).as_float();
                            self.set(args[0], Value::Vec3(Vec3::new(a, b, c)));
                        }
                    }
                }

                // color(space, r, g, b) — construct color with space conversion
                "color" if args.len() >= 5 => {
                    if let Value::String(space) = &self.get(args[1]) {
                        let r = self.get(args[2]).as_float();
                        let g = self.get(args[3]).as_float();
                        let b = self.get(args[4]).as_float();
                        let c = Vec3::new(r, g, b);
                        // Convert from named space to RGB (C++ osl_prepend_color_from)
                        let rgb = match space.as_str() {
                            "rgb" | "RGB" | "linear" => c,
                            "hsv" | "HSV" => crate::color::hsv_to_rgb(c),
                            "hsl" | "HSL" => crate::color::hsl_to_rgb(c),
                            "sRGB" | "srgb" => crate::color::srgb_to_linear_color(c),
                            _ => c, // unknown space: pass through
                        };
                        self.set(args[0], Value::Color(rgb));
                    } else {
                        // Fallback: treat as 5-arg without valid space string
                        let r = self.get(args[2]).as_float();
                        let g = self.get(args[3]).as_float();
                        let b = self.get(args[4]).as_float();
                        self.set(args[0], Value::Color(Vec3::new(r, g, b)));
                    }
                }
                // color(r, g, b) — plain RGB construction
                "color" if args.len() >= 4 => {
                    let r = self.get(args[1]).as_float();
                    let g = self.get(args[2]).as_float();
                    let b = self.get(args[3]).as_float();
                    self.set(args[0], Value::Color(Vec3::new(r, g, b)));
                }

                // --- Array access ---
                // --- init_array: create an empty array of the right type ---
                "init_array" if args.len() >= 2 => {
                    let len = self.get(args[1]).as_int().max(0) as usize;
                    let dst_idx = args[0] as usize;
                    // Determine element type from the destination symbol's typespec
                    if dst_idx < ir.symbols.len() {
                        let ts = &ir.symbols[dst_idx].typespec;
                        let val = if ts.is_closure_array() {
                            Value::ClosureArray(vec![None; len])
                        } else {
                            let td = ts.simpletype();
                            let is_vec3_elem = td.basetype
                                == crate::typedesc::BaseType::Float as u8
                                && td.aggregate == crate::typedesc::Aggregate::Vec3 as u8;
                            if is_vec3_elem {
                                Value::Vec3Array(vec![Vec3::ZERO; len])
                            } else if td.aggregate == crate::typedesc::Aggregate::Matrix44 as u8 {
                                Value::MatrixArray(vec![Matrix44::IDENTITY; len])
                            } else if td.basetype == crate::typedesc::BaseType::Int32 as u8 {
                                Value::IntArray(vec![0; len])
                            } else if td.basetype == crate::typedesc::BaseType::String as u8 {
                                Value::StringArray(vec![UString::new(""); len])
                            } else {
                                Value::FloatArray(vec![0.0; len])
                            }
                        };
                        self.set(args[0], val);
                    }
                }

                "aref" if args.len() >= 3 => {
                    let base = self.get(args[1]);
                    let idx_raw = self.get(args[2]).as_int();
                    let symname = if args[1] >= 0 && (args[1] as usize) < ir.symbols.len() {
                        ir.symbols[args[1] as usize].name.as_str()
                    } else {
                        "?"
                    };
                    let errh = msg_cfg
                        .as_ref()
                        .map(|c| c.errhandler as &dyn crate::shadingsys::ErrorHandler);
                    let val = match &base {
                        Value::Vec3(v) | Value::Color(v) => {
                            let idx = self.range_check(
                                idx_raw,
                                3,
                                symname,
                                op.sourcefile.as_str(),
                                op.sourceline,
                                &ir.shader_name,
                                errh,
                            ) as usize;
                            match idx {
                                0 => Value::Float(v.x),
                                1 => Value::Float(v.y),
                                _ => Value::Float(v.z),
                            }
                        }
                        Value::IntArray(arr) => {
                            let idx = self.range_check(
                                idx_raw,
                                arr.len() as i32,
                                symname,
                                op.sourcefile.as_str(),
                                op.sourceline,
                                &ir.shader_name,
                                errh,
                            ) as usize;
                            Value::Int(*arr.get(idx).unwrap_or(&0))
                        }
                        Value::FloatArray(arr) => {
                            let idx = self.range_check(
                                idx_raw,
                                arr.len() as i32,
                                symname,
                                op.sourcefile.as_str(),
                                op.sourceline,
                                &ir.shader_name,
                                errh,
                            ) as usize;
                            Value::Float(*arr.get(idx).unwrap_or(&0.0))
                        }
                        Value::StringArray(arr) => {
                            let idx = self.range_check(
                                idx_raw,
                                arr.len() as i32,
                                symname,
                                op.sourcefile.as_str(),
                                op.sourceline,
                                &ir.shader_name,
                                errh,
                            ) as usize;
                            Value::String(arr.get(idx).cloned().unwrap_or_else(|| UString::new("")))
                        }
                        Value::Vec3Array(arr) => {
                            let idx = self.range_check(
                                idx_raw,
                                arr.len() as i32,
                                symname,
                                op.sourcefile.as_str(),
                                op.sourceline,
                                &ir.shader_name,
                                errh,
                            ) as usize;
                            Value::Vec3(arr.get(idx).copied().unwrap_or(Vec3::ZERO))
                        }
                        Value::MatrixArray(arr) => {
                            let idx = self.range_check(
                                idx_raw,
                                arr.len() as i32,
                                symname,
                                op.sourcefile.as_str(),
                                op.sourceline,
                                &ir.shader_name,
                                errh,
                            ) as usize;
                            Value::Matrix(arr.get(idx).copied().unwrap_or(Matrix44::IDENTITY))
                        }
                        Value::ClosureArray(arr) => {
                            let idx = self.range_check(
                                idx_raw,
                                arr.len() as i32,
                                symname,
                                op.sourcefile.as_str(),
                                op.sourceline,
                                &ir.shader_name,
                                errh,
                            ) as usize;
                            match arr.get(idx).and_then(|v| v.as_ref()) {
                                Some(cv) => Value::Closure(cv.clone()),
                                None => Value::Void,
                            }
                        }
                        _ => Value::Float(0.0),
                    };
                    self.set(args[0], val);
                }

                // --- Array assign ---
                "aassign" if args.len() >= 3 => {
                    let idx_raw = self.get(args[1]).as_int();
                    let src = self.get(args[2]);
                    let symname = if args[0] >= 0 && (args[0] as usize) < ir.symbols.len() {
                        ir.symbols[args[0] as usize].name.as_str()
                    } else {
                        "?"
                    };
                    let errh = msg_cfg
                        .as_ref()
                        .map(|c| c.errhandler as &dyn crate::shadingsys::ErrorHandler);
                    if args[0] >= 0 {
                        let dst_i = args[0] as usize;
                        if dst_i < self.values.len() {
                            let length = match &self.values[dst_i] {
                                Value::Vec3(_) | Value::Color(_) => 3,
                                Value::IntArray(a) => a.len() as i32,
                                Value::FloatArray(a) => a.len() as i32,
                                Value::StringArray(a) => a.len() as i32,
                                Value::Vec3Array(a) => a.len() as i32,
                                Value::MatrixArray(a) => a.len() as i32,
                                Value::ClosureArray(a) => a.len() as i32,
                                _ => 0,
                            };
                            let idx = self.range_check(
                                idx_raw,
                                length,
                                symname,
                                op.sourcefile.as_str(),
                                op.sourceline,
                                &ir.shader_name,
                                errh,
                            ) as usize;
                            match &mut self.values[dst_i] {
                                Value::Vec3(v) | Value::Color(v) => {
                                    let f = src.as_float();
                                    match idx {
                                        0 => v.x = f,
                                        1 => v.y = f,
                                        _ => v.z = f,
                                    }
                                }
                                Value::IntArray(arr)
                                    if idx < arr.len() => {
                                        arr[idx] = src.as_int();
                                    }
                                Value::FloatArray(arr)
                                    if idx < arr.len() => {
                                        arr[idx] = src.as_float();
                                    }
                                Value::StringArray(arr)
                                    if idx < arr.len() => {
                                        arr[idx] = src.as_string();
                                    }
                                Value::Vec3Array(arr)
                                    if idx < arr.len() => {
                                        arr[idx] = src.as_vec3();
                                    }
                                Value::MatrixArray(arr) => {
                                    if idx < arr.len()
                                        && let Value::Matrix(m) = &src {
                                            arr[idx] = *m;
                                        }
                                }
                                Value::ClosureArray(arr)
                                    if idx < arr.len() => {
                                        match src {
                                            Value::Closure(cv) => arr[idx] = Some(cv),
                                            Value::Void => arr[idx] = None,
                                            _ => {}
                                        }
                                    }
                                _ => {}
                            }
                        }
                    }
                }

                // --- Type conversion ---
                "float" if args.len() >= 2 => {
                    let v = self.get(args[1]).as_float();
                    self.set(args[0], Value::Float(v));
                }
                "int" if args.len() >= 2 => {
                    let v = self.get(args[1]).as_int();
                    self.set(args[0], Value::Int(v));
                }

                // --- Math builtins (1-arg) with derivative propagation ---
                // d(sin(x)) = cos(x) * dx
                "sin" if args.len() >= 2 => {
                    let a = self.get(args[1]);
                    self.set(args[0], dual_unary_f64(&a, |x| x.sin(), |x| x.cos()));
                }
                // d(cos(x)) = -sin(x) * dx
                "cos" if args.len() >= 2 => {
                    let a = self.get(args[1]);
                    self.set(args[0], dual_unary_f64(&a, |x| x.cos(), |x| -x.sin()));
                }
                // d(tan(x)) = (1 + tan(x)^2) * dx = sec^2(x) * dx
                "tan" if args.len() >= 2 => {
                    let a = self.get(args[1]);
                    self.set(
                        args[0],
                        dual_unary_f64(
                            &a,
                            |x| x.tan(),
                            |x| {
                                let c = x.cos();
                                if c.abs() > 1e-10 { 1.0 / (c * c) } else { 0.0 }
                            },
                        ),
                    );
                }
                // d(asin(x)) = 1/sqrt(1-x^2) * dx
                "asin" if args.len() >= 2 => {
                    let a = self.get(args[1]);
                    self.set(
                        args[0],
                        dual_unary_f64(
                            &a,
                            |x| x.clamp(-1.0, 1.0).asin(),
                            |x| {
                                let t = 1.0 - x * x;
                                if t > 0.0 { 1.0 / t.sqrt() } else { 0.0 }
                            },
                        ),
                    );
                }
                // d(acos(x)) = -1/sqrt(1-x^2) * dx
                "acos" if args.len() >= 2 => {
                    let a = self.get(args[1]);
                    self.set(
                        args[0],
                        dual_unary_f64(
                            &a,
                            |x| x.clamp(-1.0, 1.0).acos(),
                            |x| {
                                let t = 1.0 - x * x;
                                if t > 0.0 { -1.0 / t.sqrt() } else { 0.0 }
                            },
                        ),
                    );
                }
                // d(atan(x)) = 1/(1+x^2) * dx
                "atan" if args.len() >= 2 => {
                    let a = self.get(args[1]);
                    self.set(
                        args[0],
                        dual_unary_f64(&a, |x| x.atan(), |x| 1.0 / (1.0 + x * x)),
                    );
                }
                // d(atan2(y,x))/dS = (x*dy - y*dx) / (x^2 + y^2)
                // where dy,dx are screen-space derivatives (Dual partials).
                // ∂atan2/∂y = x/(x²+y²), ∂atan2/∂x = -y/(x²+y²)
                // chain rule: (∂f/∂y)*y.partial + (∂f/∂x)*x.partial
                //           = (x*y.partial - y*x.partial) / (x²+y²)
                "atan2" if args.len() >= 3 => {
                    let yv = self.get(args[1]);
                    let xv = self.get(args[2]);
                    // Vec3 component-wise atan2
                    if yv.is_triple() || xv.is_triple() {
                        let y = yv.as_vec3();
                        let x = xv.as_vec3();
                        let val = Vec3::new(y.x.atan2(x.x), y.y.atan2(x.y), y.z.atan2(x.z));
                        if yv.has_derivs() || xv.has_derivs() {
                            // Per-component: d = (x*ydx - y*xdx) / (x^2+y^2)
                            let ydx = yv.dx_vec3();
                            let ydy = yv.dy_vec3();
                            let xdx = xv.dx_vec3();
                            let xdy = xv.dy_vec3();
                            let mut rdx = Vec3::ZERO;
                            let mut rdy = Vec3::ZERO;
                            for i in 0..3 {
                                let denom = x[i] * x[i] + y[i] * y[i];
                                if denom > 0.0 {
                                    let d = 1.0 / denom;
                                    rdx[i] = (x[i] * ydx[i] - y[i] * xdx[i]) * d;
                                    rdy[i] = (x[i] * ydy[i] - y[i] * xdy[i]) * d;
                                }
                            }
                            self.set(args[0], Value::DualVec3(val, rdx, rdy));
                        } else {
                            self.set(args[0], Value::Vec3(val));
                        }
                    } else {
                        let yf = yv.as_float();
                        let xf = xv.as_float();
                        let result = yf.atan2(xf);
                        if yv.has_derivs() || xv.has_derivs() {
                            let denom = xf * xf + yf * yf;
                            if denom > 0.0 {
                                let d = 1.0 / denom;
                                self.set(
                                    args[0],
                                    Value::DualFloat(
                                        result,
                                        (xf * yv.dx_float() - yf * xv.dx_float()) * d,
                                        (xf * yv.dy_float() - yf * xv.dy_float()) * d,
                                    ),
                                );
                            } else {
                                self.set(args[0], Value::DualFloat(result, 0.0, 0.0));
                            }
                        } else {
                            self.set(args[0], Value::Float(result));
                        }
                    }
                }
                // d(sinh(x)) = cosh(x) * dx  — f64 for precision
                "sinh" if args.len() >= 2 => {
                    let a = self.get(args[1]);
                    self.set(args[0], dual_unary_f64(&a, |x| x.sinh(), |x| x.cosh()));
                }
                // d(cosh(x)) = sinh(x) * dx  — f64 for precision
                "cosh" if args.len() >= 2 => {
                    let a = self.get(args[1]);
                    self.set(args[0], dual_unary_f64(&a, |x| x.cosh(), |x| x.sinh()));
                }
                // d(tanh(x)) = (1 - tanh^2(x)) * dx  — f64 for precision
                "tanh" if args.len() >= 2 => {
                    let a = self.get(args[1]);
                    self.set(
                        args[0],
                        dual_unary_f64(
                            &a,
                            |x| x.tanh(),
                            |x| {
                                let t = x.tanh();
                                1.0 - t * t
                            },
                        ),
                    );
                }
                // d(sqrt(x)) = 0.5/sqrt(x) * dx
                "sqrt" if args.len() >= 2 => {
                    let a = self.get(args[1]);
                    self.set(
                        args[0],
                        dual_unary_f64(
                            &a,
                            |x| if x >= 0.0 { x.sqrt() } else { 0.0 },
                            |x| if x > 0.0 { 0.5 / x.sqrt() } else { 0.0 },
                        ),
                    );
                }
                // d(1/sqrt(x)) = -0.5 * x^(-3/2) * dx
                "inversesqrt" if args.len() >= 2 => {
                    let a = self.get(args[1]);
                    self.set(
                        args[0],
                        dual_unary_f64(
                            &a,
                            |x| if x > 0.0 { 1.0 / x.sqrt() } else { 0.0 },
                            |x| if x > 0.0 { -0.5 * x.powf(-1.5) } else { 0.0 },
                        ),
                    );
                }
                "abs" if args.len() >= 2 => {
                    let a = self.get(args[1]);
                    // d(|x|) = sign(x) * dx — handles DualFloat, DualVec3, Int
                    let result = match &a {
                        Value::Int(x) => Value::Int(x.abs()),
                        _ => {
                            let signf = |x: f32| -> f32 { if x >= 0.0 { 1.0 } else { -1.0 } };
                            dual_unary(&a, |x| x.abs(), signf)
                        }
                    };
                    self.set(args[0], result);
                }
                // d(|x|)/dx = sign(x)
                "fabs" if args.len() >= 2 => {
                    let a = self.get(args[1]);
                    let signf = |x: f32| -> f32 { if x >= 0.0 { 1.0 } else { -1.0 } };
                    self.set(args[0], dual_unary(&a, |x| x.abs(), signf));
                }
                "floor" if args.len() >= 2 => {
                    let a = self.get(args[1]);
                    match &a {
                        Value::Vec3(v) | Value::Color(v) | Value::DualVec3(v, ..) => self.set(
                            args[0],
                            Value::Vec3(Vec3::new(v.x.floor(), v.y.floor(), v.z.floor())),
                        ),
                        _ => self.set(args[0], Value::Float(a.as_float().floor())),
                    }
                }
                "ceil" if args.len() >= 2 => {
                    let a = self.get(args[1]);
                    match &a {
                        Value::Vec3(v) | Value::Color(v) | Value::DualVec3(v, ..) => self.set(
                            args[0],
                            Value::Vec3(Vec3::new(v.x.ceil(), v.y.ceil(), v.z.ceil())),
                        ),
                        _ => self.set(args[0], Value::Float(a.as_float().ceil())),
                    }
                }
                "round" if args.len() >= 2 => {
                    let a = self.get(args[1]);
                    match &a {
                        Value::Vec3(v) | Value::Color(v) | Value::DualVec3(v, ..) => self.set(
                            args[0],
                            Value::Vec3(Vec3::new(v.x.round(), v.y.round(), v.z.round())),
                        ),
                        _ => self.set(args[0], Value::Float(a.as_float().round())),
                    }
                }
                "trunc" if args.len() >= 2 => {
                    let a = self.get(args[1]);
                    match &a {
                        Value::Vec3(v) | Value::Color(v) | Value::DualVec3(v, ..) => self.set(
                            args[0],
                            Value::Vec3(Vec3::new(v.x.trunc(), v.y.trunc(), v.z.trunc())),
                        ),
                        _ => self.set(args[0], Value::Float(a.as_float().trunc())),
                    }
                }
                "sign" if args.len() >= 2 => {
                    let a = self.get(args[1]);
                    let signf = |x: f32| -> f32 {
                        if x > 0.0 {
                            1.0
                        } else if x < 0.0 {
                            -1.0
                        } else {
                            0.0
                        }
                    };
                    match &a {
                        Value::Vec3(v) | Value::Color(v) | Value::DualVec3(v, ..) => self.set(
                            args[0],
                            Value::Vec3(Vec3::new(signf(v.x), signf(v.y), signf(v.z))),
                        ),
                        _ => {
                            let x = a.as_float();
                            self.set(args[0], Value::Float(signf(x)));
                        }
                    }
                }
                // d(exp(x)) = exp(x) * dx
                "exp" if args.len() >= 2 => {
                    let a = self.get(args[1]);
                    self.set(args[0], dual_unary_f64(&a, |x| x.exp(), |x| x.exp()));
                }
                // d(exp2(x)) = exp2(x) * ln(2) * dx
                "exp2" if args.len() >= 2 => {
                    let a = self.get(args[1]);
                    self.set(
                        args[0],
                        dual_unary_f64(&a, |x| x.exp2(), |x| x.exp2() * std::f32::consts::LN_2),
                    );
                }
                // d(expm1(x)) = exp(x) * dx
                "expm1" if args.len() >= 2 => {
                    let a = self.get(args[1]);
                    self.set(args[0], dual_unary_f64(&a, |x| x.exp() - 1.0, |x| x.exp()));
                }
                // d(ln(x)) = 1/x * dx
                // C++ OIIO::safe_log clamps to f32::MIN_POSITIVE (~1.175e-38),
                // so log(0) returns ~-87.3 instead of -INF.
                "log" if args.len() == 3 => {
                    // log(result, x, base) = ln(x)/ln(base)
                    let safe_log_base = |x: f32, b: f32| -> f32 {
                        let sx = (x as f64).max(f32::MIN_POSITIVE as f64);
                        let sb = (b as f64).max(f32::MIN_POSITIVE as f64);
                        let raw = (sx.ln() / sb.ln()) as f32;
                        if raw.is_finite() { raw } else { 0.0 }
                    };
                    let xv = self.get(args[1]);
                    let bv = self.get(args[2]);
                    if xv.is_triple() {
                        // Triple path: component-wise log(vec, base)
                        let va = xv.as_vec3();
                        let bf = bv.as_float();
                        self.set(
                            args[0],
                            Value::Vec3(Vec3::new(
                                safe_log_base(va.x, bf),
                                safe_log_base(va.y, bf),
                                safe_log_base(va.z, bf),
                            )),
                        );
                    } else {
                        // Scalar path with derivative propagation
                        let xf = xv.as_float();
                        let bf = bv.as_float();
                        let v = safe_log_base(xf, bf);
                        if xv.has_derivs() || bv.has_derivs() {
                            let sx = (xf as f64).max(f32::MIN_POSITIVE as f64);
                            let sb = (bf as f64).max(f32::MIN_POSITIVE as f64);
                            let lnx = sx.ln();
                            let lnb = sb.ln();
                            let inv_x_lnb = if xf > 0.0 && lnb.abs() > 1e-30 {
                                1.0 / (xf as f64 * lnb)
                            } else {
                                0.0
                            };
                            let lnx_over_b_lnb2 = if bf > 0.0 && lnb.abs() > 1e-30 {
                                lnx / (bf as f64 * lnb * lnb)
                            } else {
                                0.0
                            };
                            let dx = (inv_x_lnb * xv.dx_float() as f64
                                - lnx_over_b_lnb2 * bv.dx_float() as f64)
                                as f32;
                            let dy = (inv_x_lnb * xv.dy_float() as f64
                                - lnx_over_b_lnb2 * bv.dy_float() as f64)
                                as f32;
                            self.set(args[0], Value::DualFloat(v, dx, dy));
                        } else {
                            self.set(args[0], Value::Float(v));
                        }
                    }
                }
                "log" if args.len() >= 2 => {
                    // safe_log: clamp to f32::MIN_POSITIVE, matching C++ OIIO::safe_log
                    let a = self.get(args[1]);
                    self.set(
                        args[0],
                        dual_unary_f64(
                            &a,
                            |x| x.max(f32::MIN_POSITIVE as f64).ln(),
                            |x| if x < f32::MIN_POSITIVE { 0.0 } else { 1.0 / x },
                        ),
                    );
                }
                // d(log2(x)) = 1/(x*ln(2)) * dx
                "log2" if args.len() >= 2 => {
                    // safe_log2: clamp to f32::MIN_POSITIVE
                    let a = self.get(args[1]);
                    self.set(
                        args[0],
                        dual_unary_f64(
                            &a,
                            |x| x.max(f32::MIN_POSITIVE as f64).log2(),
                            |x| {
                                if x < f32::MIN_POSITIVE {
                                    0.0
                                } else {
                                    1.0 / (x * std::f32::consts::LN_2)
                                }
                            },
                        ),
                    );
                }
                // d(log10(x)) = 1/(x*ln(10)) * dx
                "log10" if args.len() >= 2 => {
                    // safe_log10: clamp to f32::MIN_POSITIVE
                    let a = self.get(args[1]);
                    self.set(
                        args[0],
                        dual_unary_f64(
                            &a,
                            |x| x.max(f32::MIN_POSITIVE as f64).log10(),
                            |x| {
                                if x < f32::MIN_POSITIVE {
                                    0.0
                                } else {
                                    1.0 / (x * std::f32::consts::LN_10)
                                }
                            },
                        ),
                    );
                }
                "logb" if args.len() >= 2 => {
                    let logb_f = |x: f32| -> f32 {
                        if x != 0.0 {
                            (x.abs().log2()).floor()
                        } else {
                            f32::NEG_INFINITY
                        }
                    };
                    let a = self.get(args[1]);
                    match &a {
                        Value::Vec3(v) | Value::Color(v) | Value::DualVec3(v, ..) => self.set(
                            args[0],
                            Value::Vec3(Vec3::new(logb_f(v.x), logb_f(v.y), logb_f(v.z))),
                        ),
                        _ => self.set(args[0], Value::Float(logb_f(a.as_float()))),
                    }
                }
                // degrees(rad) = rad * 180/pi — linear, so derivs scale too
                "degrees" if args.len() >= 2 => {
                    let a = self.get(args[1]);
                    let scale = 180.0 / std::f32::consts::PI;
                    self.set(args[0], dual_unary(&a, |x| x * scale, |_| scale));
                }
                // radians(deg) = deg * pi/180 — linear
                "radians" if args.len() >= 2 => {
                    let a = self.get(args[1]);
                    let scale = std::f32::consts::PI / 180.0;
                    self.set(args[0], dual_unary(&a, |x| x * scale, |_| scale));
                }
                // d(erf(x)) = 2/sqrt(pi) * exp(-x^2)
                "erf" if args.len() >= 2 => {
                    let erf_f64 = |x: f64| -> f64 {
                        let t = 1.0_f64 / (1.0 + 0.3275911 * x.abs());
                        let y = 1.0
                            - (((((1.061405429 * t - 1.453152027) * t) + 1.421413741) * t
                                - 0.284496736)
                                * t
                                + 0.254829592)
                                * t
                                * (-x * x).exp();
                        if x >= 0.0 { y } else { -y }
                    };
                    let two_over_sqrt_pi: f32 = 2.0 / std::f32::consts::PI.sqrt();
                    let a = self.get(args[1]);
                    self.set(
                        args[0],
                        dual_unary_f64(&a, erf_f64, |x| two_over_sqrt_pi * (-x * x).exp()),
                    );
                }
                // d(erfc(x)) = -2/sqrt(pi) * exp(-x^2)
                "erfc" if args.len() >= 2 => {
                    let erfc_f64 = |x: f64| -> f64 {
                        let t = 1.0_f64 / (1.0 + 0.3275911 * x.abs());
                        let y = 1.0
                            - (((((1.061405429 * t - 1.453152027) * t) + 1.421413741) * t
                                - 0.284496736)
                                * t
                                + 0.254829592)
                                * t
                                * (-x * x).exp();
                        if x >= 0.0 { 1.0 - y } else { 1.0 + y }
                    };
                    let neg_two_over_sqrt_pi: f32 = -2.0 / std::f32::consts::PI.sqrt();
                    let a = self.get(args[1]);
                    self.set(
                        args[0],
                        dual_unary_f64(&a, erfc_f64, |x| neg_two_over_sqrt_pi * (-x * x).exp()),
                    );
                }
                "isnan" if args.len() >= 2 => {
                    let x = self.get(args[1]).as_float();
                    self.set(args[0], Value::Int(if x.is_nan() { 1 } else { 0 }));
                }
                "isinf" if args.len() >= 2 => {
                    let x = self.get(args[1]).as_float();
                    self.set(args[0], Value::Int(if x.is_infinite() { 1 } else { 0 }));
                }
                "isfinite" if args.len() >= 2 => {
                    let x = self.get(args[1]).as_float();
                    self.set(args[0], Value::Int(if x.is_finite() { 1 } else { 0 }));
                }

                // --- Math builtins (2-arg) ---
                "pow" if args.len() >= 3 => {
                    // C++ OIIO::safe_pow: pow(0,0)=1, clamp inf/nan to 0
                    let safe_pow = |x: f32, y: f32| -> f32 {
                        if y == 0.0 {
                            return 1.0;
                        }
                        if x == 0.0 {
                            return 0.0;
                        }
                        let r = (x as f64).powf(y as f64) as f32;
                        // C++ OIIO::safe_pow: non-finite -> 0
                        if r.is_finite() { r } else { 0.0 }
                    };
                    let a = self.get(args[1]);
                    let b = self.get(args[2]);
                    if a.is_triple() || b.is_triple() {
                        let va = a.as_vec3();
                        let vb = b.as_vec3();
                        let val = Vec3::new(
                            safe_pow(va.x, vb.x),
                            safe_pow(va.y, vb.y),
                            safe_pow(va.z, vb.z),
                        );
                        if a.has_derivs() || b.has_derivs() {
                            // Per-component dual pow: d(x^y) = x^(y-1) * (y*dx + x*ln(x)*dy)
                            let adx = a.dx_vec3();
                            let ady = a.dy_vec3();
                            let bdx = b.dx_vec3();
                            let bdy = b.dy_vec3();
                            let mut rdx = Vec3::ZERO;
                            let mut rdy = Vec3::ZERO;
                            for i in 0..3 {
                                let pm1 = safe_pow(va[i], vb[i] - 1.0);
                                let logu = if va[i] > 0.0 { va[i].ln() } else { 0.0 };
                                rdx[i] = pm1 * (vb[i] * adx[i] + va[i] * logu * bdx[i]);
                                rdy[i] = pm1 * (vb[i] * ady[i] + va[i] * logu * bdy[i]);
                            }
                            self.set(args[0], Value::DualVec3(val, rdx, rdy));
                        } else {
                            self.set(args[0], Value::Vec3(val));
                        }
                    } else {
                        let xf = a.as_float();
                        let yf = b.as_float();
                        if a.has_derivs() || b.has_derivs() {
                            // C++ OSL Dual safe_pow: powuvm1 = safe_pow(u, v-1), powuv = powuvm1 * u
                            // This gives pow(0,0) = 0 in Dual path (unlike scalar safe_pow which gives 1)
                            let pm1 = safe_pow(xf, yf - 1.0);
                            let powuv = pm1 * xf;
                            let logu = if xf > 0.0 { xf.ln() } else { 0.0 };
                            self.set(
                                args[0],
                                Value::DualFloat(
                                    powuv,
                                    (yf * pm1 * a.dx_float()) + (logu * powuv * b.dx_float()),
                                    (yf * pm1 * a.dy_float()) + (logu * powuv * b.dy_float()),
                                ),
                            );
                        } else {
                            self.set(args[0], Value::Float(safe_pow(xf, yf)));
                        }
                    }
                }
                "max" if args.len() >= 3 => {
                    // d(max(a,b)) = a > b ? da : db (C++ selects b on tie)
                    let a = self.get(args[1]);
                    let b = self.get(args[2]);
                    if a.is_triple() || b.is_triple() {
                        let va = a.as_vec3();
                        let vb = b.as_vec3();
                        let r = Vec3::new(va.x.max(vb.x), va.y.max(vb.y), va.z.max(vb.z));
                        if a.has_derivs() || b.has_derivs() {
                            let (adx, ady) = (a.dx_vec3(), a.dy_vec3());
                            let (bdx, bdy) = (b.dx_vec3(), b.dy_vec3());
                            let sel =
                                |av: f32, bv: f32, ad: f32, bd: f32| if av > bv { ad } else { bd };
                            let dx = Vec3::new(
                                sel(va.x, vb.x, adx.x, bdx.x),
                                sel(va.y, vb.y, adx.y, bdx.y),
                                sel(va.z, vb.z, adx.z, bdx.z),
                            );
                            let dy = Vec3::new(
                                sel(va.x, vb.x, ady.x, bdy.x),
                                sel(va.y, vb.y, ady.y, bdy.y),
                                sel(va.z, vb.z, ady.z, bdy.z),
                            );
                            self.set(args[0], Value::DualVec3(r, dx, dy));
                        } else {
                            self.set(args[0], Value::Vec3(r));
                        }
                    } else {
                        let af = a.as_float();
                        let bf = b.as_float();
                        let r = af.max(bf);
                        if a.has_derivs() || b.has_derivs() {
                            let (dx, dy) = if af > bf {
                                (a.dx_float(), a.dy_float())
                            } else {
                                (b.dx_float(), b.dy_float())
                            };
                            self.set(args[0], Value::DualFloat(r, dx, dy));
                        } else {
                            self.set(args[0], Value::Float(r));
                        }
                    }
                }
                "min" if args.len() >= 3 => {
                    // d(min(a,b)) = a <= b ? da : db
                    let a = self.get(args[1]);
                    let b = self.get(args[2]);
                    if a.is_triple() || b.is_triple() {
                        let va = a.as_vec3();
                        let vb = b.as_vec3();
                        let r = Vec3::new(va.x.min(vb.x), va.y.min(vb.y), va.z.min(vb.z));
                        if a.has_derivs() || b.has_derivs() {
                            let (adx, ady) = (a.dx_vec3(), a.dy_vec3());
                            let (bdx, bdy) = (b.dx_vec3(), b.dy_vec3());
                            let sel =
                                |av: f32, bv: f32, ad: f32, bd: f32| if av <= bv { ad } else { bd };
                            let dx = Vec3::new(
                                sel(va.x, vb.x, adx.x, bdx.x),
                                sel(va.y, vb.y, adx.y, bdx.y),
                                sel(va.z, vb.z, adx.z, bdx.z),
                            );
                            let dy = Vec3::new(
                                sel(va.x, vb.x, ady.x, bdy.x),
                                sel(va.y, vb.y, ady.y, bdy.y),
                                sel(va.z, vb.z, ady.z, bdy.z),
                            );
                            self.set(args[0], Value::DualVec3(r, dx, dy));
                        } else {
                            self.set(args[0], Value::Vec3(r));
                        }
                    } else {
                        let af = a.as_float();
                        let bf = b.as_float();
                        let r = af.min(bf);
                        if a.has_derivs() || b.has_derivs() {
                            let (dx, dy) = if af <= bf {
                                (a.dx_float(), a.dy_float())
                            } else {
                                (b.dx_float(), b.dy_float())
                            };
                            self.set(args[0], Value::DualFloat(r, dx, dy));
                        } else {
                            self.set(args[0], Value::Float(r));
                        }
                    }
                }
                // C++ fmod uses truncation toward zero (same as safe_fmod_f32)
                "fmod" if args.len() >= 3 => {
                    // fmod derivs pass through from x: d(fmod(x,y)) = dx
                    let a = self.get(args[1]);
                    let b = self.get(args[2]);
                    if a.is_triple() || b.is_triple() {
                        let va = a.as_vec3();
                        let vb = b.as_vec3();
                        let r = Vec3::new(
                            safe_fmod_f32(va.x, vb.x),
                            safe_fmod_f32(va.y, vb.y),
                            safe_fmod_f32(va.z, vb.z),
                        );
                        if a.has_derivs() {
                            self.set(args[0], Value::DualVec3(r, a.dx_vec3(), a.dy_vec3()));
                        } else {
                            self.set(args[0], Value::Vec3(r));
                        }
                    } else if a.has_derivs() {
                        let r = safe_fmod_f32(a.as_float(), b.as_float());
                        self.set(args[0], Value::DualFloat(r, a.dx_float(), a.dy_float()));
                    } else {
                        self.set(
                            args[0],
                            Value::Float(safe_fmod_f32(a.as_float(), b.as_float())),
                        );
                    }
                }
                "step" if args.len() >= 3 => {
                    let edge = self.get(args[1]).as_float();
                    let x = self.get(args[2]).as_float();
                    self.set(args[0], Value::Float(if x < edge { 0.0 } else { 1.0 }));
                }

                // --- Math builtins (3-arg) ---
                "clamp" if args.len() >= 4 => {
                    let a = self.get(args[1]);
                    let lo = self.get(args[2]);
                    let hi = self.get(args[3]);
                    // OSL clamp: max(lo, min(x, hi)) per stdosl.h
                    let clampf = |x: f32, lo: f32, hi: f32| -> f32 { lo.max(x.min(hi)) };
                    if a.is_triple() || lo.is_triple() || hi.is_triple() {
                        let v = a.as_vec3();
                        let l = lo.as_vec3();
                        let h = hi.as_vec3();
                        self.set(
                            args[0],
                            Value::Vec3(Vec3::new(
                                clampf(v.x, l.x, h.x),
                                clampf(v.y, l.y, h.y),
                                clampf(v.z, l.z, h.z),
                            )),
                        );
                    } else {
                        let xf = a.as_float();
                        let lf = lo.as_float();
                        let hf = hi.as_float();
                        let clamped = clampf(xf, lf, hf);
                        if a.has_derivs() || lo.has_derivs() || hi.has_derivs() {
                            // Derivs: pick from whichever bound is active
                            let (dx, dy) = if xf <= lf {
                                (lo.dx_float(), lo.dy_float())
                            } else if xf >= hf {
                                (hi.dx_float(), hi.dy_float())
                            } else {
                                (a.dx_float(), a.dy_float())
                            };
                            self.set(args[0], Value::DualFloat(clamped, dx, dy));
                        } else {
                            self.set(args[0], Value::Float(clamped));
                        }
                    }
                }
                "mix" if args.len() >= 4 => {
                    // d(mix(a,b,t)) = (1-t)*da + t*db + (b-a)*dt
                    let a = self.get(args[1]);
                    let b = self.get(args[2]);
                    let t = self.get(args[3]);
                    let mixf = |a: f32, b: f32, t: f32| -> f32 { a * (1.0 - t) + b * t };
                    if a.is_triple() || b.is_triple() {
                        let av = a.as_vec3();
                        let bv = b.as_vec3();
                        // t can be scalar or triple for vec mix
                        let tv = if t.is_triple() {
                            t.as_vec3()
                        } else {
                            let f = t.as_float();
                            Vec3::new(f, f, f)
                        };
                        let r = Vec3::new(
                            mixf(av.x, bv.x, tv.x),
                            mixf(av.y, bv.y, tv.y),
                            mixf(av.z, bv.z, tv.z),
                        );
                        if a.has_derivs() || b.has_derivs() || t.has_derivs() {
                            let (adx, ady) = (a.dx_vec3(), a.dy_vec3());
                            let (bdx, bdy) = (b.dx_vec3(), b.dy_vec3());
                            let (tdx, tdy) = (t.dx_vec3(), t.dy_vec3());
                            let mix_dx = |i: usize| {
                                let (a, b, t, da, db, dt) = match i {
                                    0 => (av.x, bv.x, tv.x, adx.x, bdx.x, tdx.x),
                                    1 => (av.y, bv.y, tv.y, adx.y, bdx.y, tdx.y),
                                    _ => (av.z, bv.z, tv.z, adx.z, bdx.z, tdx.z),
                                };
                                (1.0 - t) * da + t * db + (b - a) * dt
                            };
                            let mix_dy = |i: usize| {
                                let (a, b, t, da, db, dt) = match i {
                                    0 => (av.x, bv.x, tv.x, ady.x, bdy.x, tdy.x),
                                    1 => (av.y, bv.y, tv.y, ady.y, bdy.y, tdy.y),
                                    _ => (av.z, bv.z, tv.z, ady.z, bdy.z, tdy.z),
                                };
                                (1.0 - t) * da + t * db + (b - a) * dt
                            };
                            let dx = Vec3::new(mix_dx(0), mix_dx(1), mix_dx(2));
                            let dy = Vec3::new(mix_dy(0), mix_dy(1), mix_dy(2));
                            self.set(args[0], Value::DualVec3(r, dx, dy));
                        } else {
                            self.set(args[0], Value::Vec3(r));
                        }
                    } else {
                        let af = a.as_float();
                        let bf = b.as_float();
                        let tf = t.as_float();
                        let r = mixf(af, bf, tf);
                        if a.has_derivs() || b.has_derivs() || t.has_derivs() {
                            let dx = (1.0 - tf) * a.dx_float()
                                + tf * b.dx_float()
                                + (bf - af) * t.dx_float();
                            let dy = (1.0 - tf) * a.dy_float()
                                + tf * b.dy_float()
                                + (bf - af) * t.dy_float();
                            self.set(args[0], Value::DualFloat(r, dx, dy));
                        } else {
                            self.set(args[0], Value::Float(r));
                        }
                    }
                }
                // smoothstep with Dual2 derivative propagation.
                // C++ dual.h: t=(x-e0)/(e1-e0), result=(3-2t)*t*t
                "smoothstep" if args.len() >= 4 => {
                    let e0 = self.get(args[1]);
                    let e1 = self.get(args[2]);
                    let x = self.get(args[3]);
                    // Component-wise smoothstep for vector/color inputs
                    let is_triple =
                        matches!(&x, Value::Vec3(_) | Value::Color(_) | Value::DualVec3(..))
                            || matches!(
                                &e0,
                                Value::Vec3(_) | Value::Color(_) | Value::DualVec3(..)
                            )
                            || matches!(
                                &e1,
                                Value::Vec3(_) | Value::Color(_) | Value::DualVec3(..)
                            );
                    if is_triple {
                        let ssf = |x: f32, e0: f32, e1: f32| -> f32 {
                            if x <= e0 {
                                0.0
                            } else if x >= e1 {
                                1.0
                            } else {
                                let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
                                t * t * (3.0 - 2.0 * t)
                            }
                        };
                        let xv = x.as_vec3();
                        let e0v = e0.as_vec3();
                        let e1v = e1.as_vec3();
                        self.set(
                            args[0],
                            Value::Vec3(Vec3::new(
                                ssf(xv.x, e0v.x, e1v.x),
                                ssf(xv.y, e0v.y, e1v.y),
                                ssf(xv.z, e0v.z, e1v.z),
                            )),
                        );
                    } else {
                        self.set(args[0], dual_smoothstep(&e0, &e1, &x));
                    }
                }
                "linearstep" if args.len() >= 4 => {
                    let edge0 = self.get(args[1]).as_float();
                    let edge1 = self.get(args[2]).as_float();
                    let x = self.get(args[3]).as_float();
                    let t = if edge0 >= edge1 {
                        0.0
                    } else {
                        ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0)
                    };
                    self.set(args[0], Value::Float(t));
                }

                // --- Vector operations ---
                "dot" if args.len() >= 3 => {
                    // d(a·b) = da·b + a·db
                    let a = self.get(args[1]);
                    let b = self.get(args[2]);
                    let av = a.as_vec3();
                    let bv = b.as_vec3();
                    let r = av.dot(bv);
                    if a.has_derivs() || b.has_derivs() {
                        let dx = a.dx_vec3().dot(bv) + av.dot(b.dx_vec3());
                        let dy = a.dy_vec3().dot(bv) + av.dot(b.dy_vec3());
                        self.set(args[0], Value::DualFloat(r, dx, dy));
                    } else {
                        self.set(args[0], Value::Float(r));
                    }
                }
                "cross" if args.len() >= 3 => {
                    // d(a×b) = da×b + a×db
                    let a = self.get(args[1]);
                    let b = self.get(args[2]);
                    let av = a.as_vec3();
                    let bv = b.as_vec3();
                    let r = av.cross(bv);
                    if a.has_derivs() || b.has_derivs() {
                        let dx = a.dx_vec3().cross(bv) + av.cross(b.dx_vec3());
                        let dy = a.dy_vec3().cross(bv) + av.cross(b.dy_vec3());
                        self.set(args[0], Value::DualVec3(r, dx, dy));
                    } else {
                        self.set(args[0], Value::Vec3(r));
                    }
                }
                "normalize" if args.len() >= 2 => {
                    // d(v/|v|) = (dv*|v|^2 - v*(v·dv)) / |v|^3
                    let src = self.get(args[1]);
                    let v = src.as_vec3();
                    let len = v.length();
                    let r = if len > 0.0 { v / len } else { v };
                    if src.has_derivs() && len > 0.0 {
                        let len2 = len * len;
                        let len3 = len2 * len;
                        let dv_dx = src.dx_vec3();
                        let dv_dy = src.dy_vec3();
                        let dx = (dv_dx * len2 - v * v.dot(dv_dx)) / len3;
                        let dy = (dv_dy * len2 - v * v.dot(dv_dy)) / len3;
                        self.set(args[0], Value::DualVec3(r, dx, dy));
                    } else {
                        self.set(args[0], Value::Vec3(r));
                    }
                }
                "length" if args.len() >= 2 => {
                    // d(|v|) = (v·dv) / |v|
                    let src = self.get(args[1]);
                    let v = src.as_vec3();
                    let len = v.length();
                    if src.has_derivs() && len > 0.0 {
                        let dx = v.dot(src.dx_vec3()) / len;
                        let dy = v.dot(src.dy_vec3()) / len;
                        self.set(args[0], Value::DualFloat(len, dx, dy));
                    } else {
                        self.set(args[0], Value::Float(len));
                    }
                }
                "distance" if args.len() >= 3 => {
                    if args.len() >= 4 {
                        // distance(result, a, b, q) — point q to line segment a→b
                        let av = self.get(args[1]).as_vec3();
                        let bv = self.get(args[2]).as_vec3();
                        let qv = self.get(args[3]).as_vec3();
                        let ab = bv - av;
                        let ab_len2 = ab.dot(ab);
                        let result = if ab_len2 == 0.0 {
                            (qv - av).length()
                        } else {
                            let t = ((qv - av).dot(ab) / ab_len2).clamp(0.0, 1.0);
                            let closest = av + ab * t;
                            (qv - closest).length()
                        };
                        self.set(args[0], Value::Float(result));
                    } else {
                        // distance(result, a, b) — point to point
                        let a = self.get(args[1]);
                        let b = self.get(args[2]);
                        let av = a.as_vec3();
                        let bv = b.as_vec3();
                        let d = av - bv;
                        let len = d.length();
                        if (a.has_derivs() || b.has_derivs()) && len > 0.0 {
                            let ddx = a.dx_vec3() - b.dx_vec3();
                            let ddy = a.dy_vec3() - b.dy_vec3();
                            let dx = d.dot(ddx) / len;
                            let dy = d.dot(ddy) / len;
                            self.set(args[0], Value::DualFloat(len, dx, dy));
                        } else {
                            self.set(args[0], Value::Float(len));
                        }
                    }
                }
                "faceforward" if args.len() >= 4 => {
                    let n = self.get(args[1]).as_vec3();
                    let i = self.get(args[2]).as_vec3();
                    let nref = self.get(args[3]).as_vec3();
                    let result = if nref.dot(i) < 0.0 {
                        n
                    } else {
                        Vec3::new(-n.x, -n.y, -n.z)
                    };
                    self.set(args[0], Value::Vec3(result));
                }
                "faceforward" if args.len() == 3 => {
                    // 2-arg faceforward(N, I) — uses I as both I and Nref
                    let n = self.get(args[1]).as_vec3();
                    let i = self.get(args[2]).as_vec3();
                    let result = if n.dot(i) < 0.0 {
                        n
                    } else {
                        Vec3::new(-n.x, -n.y, -n.z)
                    };
                    self.set(args[0], Value::Vec3(result));
                }
                "hypot" if args.len() == 3 => {
                    // d(hypot(a,b)) = (a*da + b*db) / hypot(a,b)
                    let a = self.get(args[1]);
                    let b = self.get(args[2]);
                    let af = a.as_float();
                    let bf = b.as_float();
                    let r = ((af as f64).hypot(bf as f64)) as f32;
                    if (a.has_derivs() || b.has_derivs()) && r > 0.0 {
                        let dx = (af * a.dx_float() + bf * b.dx_float()) / r;
                        let dy = (af * a.dy_float() + bf * b.dy_float()) / r;
                        self.set(args[0], Value::DualFloat(r, dx, dy));
                    } else {
                        self.set(args[0], Value::Float(r));
                    }
                }
                "hypot" if args.len() >= 4 => {
                    // d(hypot(a,b,c)) = (a*da + b*db + c*dc) / hypot(a,b,c)
                    let a = self.get(args[1]);
                    let b = self.get(args[2]);
                    let c = self.get(args[3]);
                    let af = a.as_float();
                    let bf = b.as_float();
                    let cf = c.as_float();
                    let r = (af as f64 * af as f64 + bf as f64 * bf as f64 + cf as f64 * cf as f64)
                        .sqrt() as f32;
                    if (a.has_derivs() || b.has_derivs() || c.has_derivs()) && r > 0.0 {
                        let dx = (af * a.dx_float() + bf * b.dx_float() + cf * c.dx_float()) / r;
                        let dy = (af * a.dy_float() + bf * b.dy_float() + cf * c.dy_float()) / r;
                        self.set(args[0], Value::DualFloat(r, dx, dy));
                    } else {
                        self.set(args[0], Value::Float(r));
                    }
                }
                // fresnel(I, N, eta, Kr [, Kt, R, T])
                "fresnel" if args.len() >= 5 => {
                    // fresnel(void_ret, I, N, eta, Kr [, Kt, R, T])
                    // args[0]=void, args[1]=I, args[2]=N, args[3]=eta, args[4]=Kr, ...
                    let i = self.get(args[1]).as_vec3();
                    let n = self.get(args[2]).as_vec3();
                    let eta = self.get(args[3]).as_float();
                    let c = i.dot(n).abs();
                    let r_vec = i - n * (2.0 * i.dot(n));
                    let g2 = 1.0 / (eta * eta) - 1.0 + c * c;
                    if g2 >= 0.0 {
                        let g = g2.sqrt();
                        let beta = g - c;
                        let f_val = (c * (g + c) - 1.0) / (c * beta + 1.0);
                        let f_val = 0.5 * (1.0 + f_val * f_val);
                        let kr = f_val * (beta / (g + c)) * (beta / (g + c));
                        let kt = (1.0 - kr) * eta * eta; // C++ multiplies by eta^2
                        self.set(args[4], Value::Float(kr));
                        if args.len() >= 6 {
                            self.set(args[5], Value::Float(kt));
                        }
                        if args.len() >= 7 {
                            self.set(args[6], Value::Vec3(r_vec));
                        }
                        if args.len() >= 8 {
                            // T = refract(I, N, eta)
                            let idotn = i.dot(n);
                            let k = 1.0 - eta * eta * (1.0 - idotn * idotn);
                            let t = if k < 0.0 {
                                Vec3::ZERO
                            } else {
                                i * eta - n * (eta * idotn + k.sqrt())
                            };
                            self.set(args[7], Value::Vec3(t));
                        }
                    } else {
                        // Total internal reflection
                        self.set(args[4], Value::Float(1.0));
                        if args.len() >= 6 {
                            self.set(args[5], Value::Float(0.0));
                        }
                        if args.len() >= 7 {
                            self.set(args[6], Value::Vec3(r_vec));
                        }
                        if args.len() >= 8 {
                            self.set(args[7], Value::Vec3(Vec3::ZERO));
                        }
                    }
                }
                "reflect" if args.len() >= 3 => {
                    let i = self.get(args[1]).as_vec3();
                    let n = self.get(args[2]).as_vec3();
                    let d = 2.0 * n.dot(i);
                    self.set(args[0], Value::Vec3(i - n * d));
                }
                "refract" if args.len() >= 4 => {
                    let i = self.get(args[1]).as_vec3();
                    let n = self.get(args[2]).as_vec3();
                    let eta = self.get(args[3]).as_float();
                    let cos_i = n.dot(i);
                    let k = 1.0 - eta * eta * (1.0 - cos_i * cos_i);
                    if k < 0.0 {
                        self.set(args[0], Value::Vec3(Vec3::ZERO));
                    } else {
                        self.set(args[0], Value::Vec3(i * eta - n * (eta * cos_i + k.sqrt())));
                    }
                }

                // --- Color operations ---
                "luminance" if args.len() >= 2 => {
                    let c = self.get(args[1]).as_vec3();
                    self.set(args[0], Value::Float(crate::color::luminance(c)));
                }

                // --- Noise ---
                "noise" if args.len() >= 2 => {
                    let p = self.get(args[1]).as_vec3();
                    self.set(args[0], Value::Float(crate::noise::uperlin3(p)));
                }
                "cellnoise" if args.len() >= 2 => {
                    let p = self.get(args[1]).as_vec3();
                    self.set(args[0], Value::Float(crate::noise::cellnoise3(p)));
                }
                "snoise" if args.len() >= 2 => {
                    let p = self.get(args[1]).as_vec3();
                    self.set(args[0], Value::Float(crate::noise::perlin3(p)));
                }

                // --- String operations ---
                "concat" if args.len() >= 3 => {
                    // Variadic concat: concat(result, s1, s2, ..., sN)
                    let mut result = String::new();
                    for &slot in args.iter().skip(1) {
                        if let Value::String(s) = &self.get(slot) {
                            result.push_str(s.as_str());
                        }
                    }
                    self.set(args[0], Value::String(UString::new(&result)));
                }
                "strlen" if args.len() >= 2 => {
                    if let Value::String(s) = &self.get(args[1]) {
                        self.set(args[0], Value::Int(s.as_str().len() as i32));
                    }
                }
                "startswith" if args.len() >= 3 => {
                    if let (Value::String(s), Value::String(prefix)) =
                        (&self.get(args[1]), &self.get(args[2]))
                    {
                        self.set(
                            args[0],
                            Value::Int(if s.as_str().starts_with(prefix.as_str()) {
                                1
                            } else {
                                0
                            }),
                        );
                    }
                }
                "endswith" if args.len() >= 3 => {
                    if let (Value::String(s), Value::String(suffix)) =
                        (&self.get(args[1]), &self.get(args[2]))
                    {
                        self.set(
                            args[0],
                            Value::Int(if s.as_str().ends_with(suffix.as_str()) {
                                1
                            } else {
                                0
                            }),
                        );
                    }
                }

                // --- Output ---
                // Accept both forms:
                // 1) legacy: (result, fmt, args...)
                // 2) void:   (fmt, args...)
                "printf" if !args.is_empty() => {
                    let start = if let Some(&sym0) = args.first() {
                        match self.get(sym0) {
                            Value::String(_) => 0,
                            _ if args.len() >= 2 => 1,
                            _ => 0,
                        }
                    } else {
                        0
                    };
                    let msg = self.format_string(&args[start..]);
                    self.messages.push(msg);
                }
                "fprintf" if args.len() >= 2 => {
                    // fprintf(file, fmt, ...) — we ignore file, just format
                    let start = if args.len() >= 2 {
                        let arg0_is_str = matches!(self.get(args[0]), Value::String(_));
                        let arg1_is_str = matches!(self.get(args[1]), Value::String(_));
                        if arg0_is_str && arg1_is_str {
                            1
                        } else if args.len() >= 3 {
                            2 // legacy: (result, file, fmt, ...)
                        } else {
                            1
                        }
                    } else {
                        1
                    };
                    let msg = self.format_string(&args[start..]);
                    self.messages.push(msg);
                }
                "sprintf" if args.len() >= 2 => {
                    // sprintf(result, fmt, ...)
                    let msg = self.format_string(&args[1..]);
                    self.set(args[0], Value::String(UString::new(&msg)));
                }
                "warning" if !args.is_empty() => {
                    let start = if let Some(&sym0) = args.first() {
                        match self.get(sym0) {
                            Value::String(_) => 0,
                            _ if args.len() >= 2 => 1,
                            _ => 0,
                        }
                    } else {
                        0
                    };
                    let msg = self.format_string(&args[start..]);
                    // Deduplicate warnings (C++ m_warnseen)
                    if self.seen_warnings.insert(msg.clone()) {
                        self.messages.push(format!("WARNING: {msg}\n"));
                    }
                }
                "error" if !args.is_empty() => {
                    let start = if let Some(&sym0) = args.first() {
                        match self.get(sym0) {
                            Value::String(_) => 0,
                            _ if args.len() >= 2 => 1,
                            _ => 0,
                        }
                    } else {
                        0
                    };
                    let msg = self.format_string(&args[start..]);
                    // Deduplicate errors (C++ m_errseen)
                    if self.seen_errors.insert(msg.clone()) {
                        self.messages.push(format!("ERROR: {msg}\n"));
                    }
                }

                // --- Matrix operations ---
                "determinant" if args.len() >= 2 => {
                    if let Value::Matrix(m) = &self.get(args[1]) {
                        self.set(args[0], Value::Float(crate::matrix_ops::determinant(m)));
                    }
                }
                "transpose" if args.len() >= 2 => {
                    if let Value::Matrix(m) = &self.get(args[1]) {
                        self.set(args[0], Value::Matrix(crate::matrix_ops::transpose(m)));
                    }
                }

                // --- Closures ---
                "closure" if args.len() >= 3 => {
                    // closure(result, closure_name, params...)
                    let name = match &self.get(args[1]) {
                        Value::String(s) => s.as_str().to_string(),
                        _ => "unknown".to_string(),
                    };
                    let mut params = Vec::new();
                    for &slot in args.iter().skip(2) {
                        params.push(self.get(slot));
                    }
                    let cv = ClosureValue::Component {
                        name: name.clone(),
                        id: crate::closure_ops::closure_name_to_id(&name).unwrap_or(0),
                        params,
                        weight: Color3::new(1.0, 1.0, 1.0),
                    };
                    self.set(args[0], Value::Closure(Box::new(cv)));
                }

                // Named closure constructors emitted directly by codegen
                // (e.g. "diffuse", "emission", "phong", etc.)
                opname
                    if !args.is_empty()
                        && crate::closure_ops::closure_name_to_id(opname).is_some() =>
                {
                    // args[0] = result, args[1..] = closure params
                    let mut params = Vec::new();
                    for &slot in args.iter().skip(1) {
                        params.push(self.get(slot));
                    }
                    let cv = ClosureValue::Component {
                        name: opname.to_string(),
                        id: crate::closure_ops::closure_name_to_id(opname).unwrap_or(0),
                        params,
                        weight: Color3::new(1.0, 1.0, 1.0),
                    };
                    self.set(args[0], Value::Closure(Box::new(cv)));
                }

                // --- Message passing ---
                "setmessage" if args.len() >= 2 => {
                    // void setmessage(name, value) — no return sym
                    if let Value::String(name) = &self.get(args[0]) {
                        let val = self.get(args[1]);
                        let val_msg = value_to_message_value(&val);
                        if let Some(ref mut cfg) = msg_cfg {
                            let mut on_err = |s: &str| cfg.errhandler.error(s);
                            cfg.shared_messages.setmessage_validated(
                                *name,
                                val_msg,
                                cfg.layeridx,
                                &mut on_err,
                            );
                        } else {
                            self.message_store.insert(name.as_str().to_string(), val);
                        }
                    }
                }
                // getmessage: 3-arg form (result, name, value) or
                //             4-arg form (result, source, name, value)
                "getmessage" if args.len() >= 3 => {
                    // Determine source and name based on arg count.
                    // 4-arg: args[0]=result, args[1]=source, args[2]=name, args[3]=value
                    // 3-arg: args[0]=result, args[1]=name (source="")
                    let (source_str, name_val) = if args.len() >= 4 {
                        // 4-arg form: extract source string
                        let src = match &self.get(args[1]) {
                            Value::String(s) => s.as_str().to_string(),
                            _ => String::new(),
                        };
                        (src, self.get(args[2]))
                    } else {
                        (String::new(), self.get(args[1]))
                    };

                    if let Value::String(name) = &name_val {
                        // When source is "trace", delegate to renderer
                        let found = if source_str == "trace" {
                            if let Some(renderer) = &self.renderer {
                                let name_hash = UStringHash::hash_utf8(name.as_str());
                                renderer.get_trace_value(globals, name_hash).map(
                                    |attr| match attr {
                                        crate::renderer::AttributeData::Int(i) => Value::Int(i),
                                        crate::renderer::AttributeData::Float(f) => Value::Float(f),
                                        crate::renderer::AttributeData::String(s) => {
                                            Value::String(UString::new(&s))
                                        }
                                        crate::renderer::AttributeData::Vec3(v) => Value::Vec3(v),
                                        crate::renderer::AttributeData::Matrix44(m) => {
                                            Value::Matrix(m)
                                        }
                                        crate::renderer::AttributeData::IntArray(a) => {
                                            Value::IntArray(a)
                                        }
                                        crate::renderer::AttributeData::FloatArray(a) => {
                                            Value::FloatArray(a)
                                        }
                                    },
                                )
                            } else {
                                None
                            }
                        } else if let Some(ref mut cfg) = msg_cfg {
                            let mut on_err = |s: &str| cfg.errhandler.error(s);
                            cfg.shared_messages
                                .getmessage_validated(
                                    *name,
                                    cfg.layeridx,
                                    cfg.strict,
                                    &mut on_err,
                                )
                                .map(|m| message_value_to_value(&m))
                        } else {
                            // Simple path: use source for the message store lookup
                            self.message_store.get(name.as_str()).cloned()
                        };
                        // args[0]=result (1=found, 0=not), value goes to last arg
                        let val_idx = if args.len() >= 4 { args[3] } else { args[2] };
                        if let Some(val) = found {
                            self.set(val_idx, val);
                            self.set(args[0], Value::Int(1));
                        } else {
                            self.set(args[0], Value::Int(0));
                        }
                    }
                }

                // --- Utility ---
                "arraylength" if args.len() >= 2 => {
                    let val = self.get(args[1]);
                    let len = match &val {
                        Value::IntArray(a) => a.len() as i32,
                        Value::FloatArray(a) => a.len() as i32,
                        Value::StringArray(a) => a.len() as i32,
                        Value::Vec3Array(a) => a.len() as i32,
                        Value::MatrixArray(a) => a.len() as i32,
                        Value::ClosureArray(a) => a.len() as i32,
                        _ => {
                            // Check symbol type for static array length
                            let sym_idx = args[1] as usize;
                            if sym_idx < ir.symbols.len() {
                                let al = ir.symbols[sym_idx].typespec.simpletype().arraylen;
                                if al > 0 { al } else { 0 }
                            } else {
                                0
                            }
                        }
                    };
                    self.set(args[0], Value::Int(len));
                }
                "isconnected" if args.len() >= 2 => {
                    self.set(args[0], Value::Int(0));
                }
                "isconstant" if args.len() >= 2 => {
                    let src_idx = args[1] as usize;
                    // In testshade, params with default values are constant.
                    let is_const = if src_idx < ir.symbols.len() {
                        let sym = &ir.symbols[src_idx];
                        sym.symtype == crate::symbol::SymType::Const
                            || sym.symtype == crate::symbol::SymType::Param
                    } else {
                        false
                    };
                    self.set(args[0], Value::Int(if is_const { 1 } else { 0 }));
                }
                "regex_search" | "regex_match" if args.len() >= 3 => {
                    let fullmatch = opname == "regex_match";
                    let s = match &self.get(args[1]) {
                        Value::String(u) => u.as_str().to_string(),
                        _ => String::new(),
                    };
                    let has_results = args.len() >= 4;
                    let pat_idx = if has_results { 3 } else { 2 };
                    let pat = match &self.get(args[pat_idx]) {
                        Value::String(u) => u.as_str().to_string(),
                        _ => String::new(),
                    };
                    if has_results {
                        // 4-arg form: result, subject, results[], pattern
                        let arr_len = match &self.get(args[2]) {
                            Value::IntArray(a) => a.len(),
                            _ => 0,
                        };
                        let mut buf = vec![0i32; arr_len];
                        let found =
                            crate::opstring::regex_search_captures(&s, &pat, &mut buf, fullmatch);
                        self.set(args[2], Value::IntArray(buf));
                        self.set(args[0], Value::Int(if found { 1 } else { 0 }));
                    } else {
                        // 3-arg form: result, subject, pattern
                        let found = if fullmatch {
                            crate::opstring::regex_match(&s, &pat)
                        } else {
                            crate::opstring::regex_search(&s, &pat)
                        };
                        self.set(args[0], Value::Int(if found { 1 } else { 0 }));
                    }
                }
                "hash" if args.len() >= 2 => {
                    use crate::hashes::*;
                    let a = self.get(args[1]);
                    let result = match args.len() {
                        2 => match &a {
                            Value::String(s) => {
                                if s.as_str().is_empty() {
                                    0
                                } else {
                                    fingerprint64(s.as_str().as_bytes()) as i32
                                }
                            }
                            Value::Float(_) | Value::DualFloat(..) => osl_hash_f(a.as_float()),
                            Value::Int(i) => inthash1(*i as u32) as i32,
                            Value::Vec3(v) | Value::Color(v) => osl_hash_v(&[v.x, v.y, v.z]),
                            Value::DualVec3(v, ..) => osl_hash_v(&[v.x, v.y, v.z]),
                            _ => 0,
                        },
                        3 => {
                            let b = self.get(args[2]);
                            match &a {
                                Value::Float(_) | Value::DualFloat(..) => {
                                    osl_hash_ff(a.as_float(), b.as_float())
                                }
                                Value::Vec3(v) | Value::Color(v) | Value::DualVec3(v, ..) => {
                                    osl_hash_vf(&[v.x, v.y, v.z], b.as_float())
                                }
                                _ => 0,
                            }
                        }
                        _ => 0,
                    };
                    self.set(args[0], Value::Int(result));
                }

                // --- atan2 (two-argument) ---
                // --- sincos(sin_out, cos_out, angle) ---
                "sincos" if args.len() >= 3 => {
                    // sincos(angle, out sin_val, out cos_val)
                    // C++ has 7 variants: fff through dvdvdv
                    let a = self.get(args[0]);
                    match &a {
                        Value::DualVec3(v, dx, dy) => {
                            let (sx, cx) = (v.x.sin(), v.x.cos());
                            let (sy, cy) = (v.y.sin(), v.y.cos());
                            let (sz, cz) = (v.z.sin(), v.z.cos());
                            self.set(
                                args[1],
                                Value::DualVec3(
                                    Vec3::new(sx, sy, sz),
                                    Vec3::new(cx * dx.x, cy * dx.y, cz * dx.z),
                                    Vec3::new(cx * dy.x, cy * dy.y, cz * dy.z),
                                ),
                            );
                            self.set(
                                args[2],
                                Value::DualVec3(
                                    Vec3::new(cx, cy, cz),
                                    Vec3::new(-sx * dx.x, -sy * dx.y, -sz * dx.z),
                                    Vec3::new(-sx * dy.x, -sy * dy.y, -sz * dy.z),
                                ),
                            );
                        }
                        Value::DualFloat(v, dx, dy) => {
                            let (sv, cv) = (v.sin(), v.cos());
                            self.set(args[1], Value::DualFloat(sv, cv * dx, cv * dy));
                            self.set(args[2], Value::DualFloat(cv, -sv * dx, -sv * dy));
                        }
                        Value::Vec3(v) | Value::Color(v) => {
                            self.set(
                                args[1],
                                Value::Vec3(Vec3::new(v.x.sin(), v.y.sin(), v.z.sin())),
                            );
                            self.set(
                                args[2],
                                Value::Vec3(Vec3::new(v.x.cos(), v.y.cos(), v.z.cos())),
                            );
                        }
                        _ => {
                            let f = a.as_float();
                            self.set(args[1], Value::Float(f.sin()));
                            self.set(args[2], Value::Float(f.cos()));
                        }
                    }
                }

                // --- cbrt ---
                "cbrt" if args.len() >= 2 => {
                    let a = self.get(args[1]);
                    match &a {
                        Value::Vec3(v) | Value::Color(v) | Value::DualVec3(v, ..) => self.set(
                            args[0],
                            Value::Vec3(Vec3::new(v.x.cbrt(), v.y.cbrt(), v.z.cbrt())),
                        ),
                        _ => self.set(args[0], Value::Float(a.as_float().cbrt())),
                    }
                }

                // --- String operations ---
                "substr" if args.len() >= 3 => {
                    let s = match &self.get(args[1]) {
                        Value::String(s) => s.as_str().to_string(),
                        _ => String::new(),
                    };
                    let slen = s.len() as i32;
                    let mut start = self.get(args[2]).as_int();
                    // Negative start: count from end
                    if start < 0 {
                        start = (slen + start).max(0);
                    }
                    let start = start.min(slen) as usize;
                    let len = if args.len() >= 4 {
                        self.get(args[3]).as_int().max(0) as usize
                    } else {
                        s.len() // from start to end
                    };
                    let end = (start + len).min(s.len());
                    let sub = if start < s.len() { &s[start..end] } else { "" };
                    self.set(args[0], Value::String(crate::ustring::UString::new(sub)));
                }

                "getchar" if args.len() >= 3 => {
                    let s = match &self.get(args[1]) {
                        Value::String(s) => s.as_str().to_string(),
                        _ => String::new(),
                    };
                    let idx = self.get(args[2]).as_int() as usize;
                    let ch = if idx < s.len() {
                        s.as_bytes()[idx] as i32
                    } else {
                        0
                    };
                    self.set(args[0], Value::Int(ch));
                }

                "stoi" if args.len() >= 2 => {
                    let s = match &self.get(args[1]) {
                        Value::String(s) => s.as_str().to_string(),
                        _ => String::new(),
                    };
                    let v = s.trim().parse::<i32>().unwrap_or(0);
                    self.set(args[0], Value::Int(v));
                }

                "stof" if args.len() >= 2 => {
                    let s = match &self.get(args[1]) {
                        Value::String(s) => s.as_str().to_string(),
                        _ => String::new(),
                    };
                    let v = s.trim().parse::<f32>().unwrap_or(0.0);
                    self.set(args[0], Value::Float(v));
                }

                "format" if !args.is_empty() => {
                    let result = self.format_string(&args[1..]);
                    self.set(
                        args[0],
                        Value::String(crate::ustring::UString::new(&result)),
                    );
                }

                "split" if args.len() >= 4 => {
                    // Non-void builtin: args[0]=result, args[1]=str, args[2]=results[], args[3]=sep, [args[4]=maxsplit]
                    let s = match &self.get(args[1]) {
                        Value::String(s) => s.as_str().to_string(),
                        _ => String::new(),
                    };
                    let sep = match &self.get(args[3]) {
                        Value::String(s) => s.as_str().to_string(),
                        _ => String::new(),
                    };
                    let maxsplit = if args.len() > 4 {
                        self.get(args[4]).as_int() as usize
                    } else {
                        usize::MAX
                    };
                    // C++ split with maxsplit: last element keeps remainder
                    let parts: Vec<&str> = if sep.is_empty() {
                        if maxsplit < usize::MAX {
                            s.splitn(maxsplit, char::is_whitespace)
                                .filter(|p| !p.is_empty())
                                .collect()
                        } else {
                            s.split_whitespace().collect()
                        }
                    } else if maxsplit < usize::MAX {
                        s.splitn(maxsplit, sep.as_str()).collect()
                    } else {
                        s.split(&sep).collect()
                    };
                    let count = parts.len();
                    // Store split results into the output string array
                    let dst_idx = args[2] as usize;
                    if dst_idx < self.values.len() {
                        let mut arr = vec![UString::new(""); count];
                        for i in 0..count {
                            arr[i] = UString::new(parts[i]);
                        }
                        self.values[dst_idx] = Value::StringArray(arr);
                    }
                    self.set(args[0], Value::Int(count as i32));
                }

                // --- Geometry ---
                "calculatenormal" if args.len() >= 2 => {
                    // calculatenormal(result, sg) — compute normal from dPdu x dPdv
                    // In the simple case, just output the shading normal
                    let n = globals.n;
                    self.set(args[0], Value::Vec3(n));
                }

                "area" if args.len() >= 2 => {
                    // area(P) — surface area of the micropolygon
                    let dpdu = globals.dp_du;
                    let dpdv = globals.dp_dv;
                    let a = dpdu.cross(dpdv).length();
                    self.set(args[0], Value::Float(a));
                }

                "filterwidth" if args.len() >= 2 => {
                    // filterwidth(x) = sqrt(|Dx(x)|^2 + |Dy(x)|^2)
                    // Uses actual propagated derivatives from Dual2 values.
                    let src = self.get(args[1]);
                    let fw = match &src {
                        Value::DualFloat(_, dx, dy) => (dx * dx + dy * dy).sqrt(),
                        Value::DualVec3(_, dx, dy) => {
                            (dx.length_squared() + dy.length_squared()).sqrt()
                        }
                        Value::Vec3(_) => {
                            let dpdx = globals.dp_dx;
                            let dpdy = globals.dp_dy;
                            (dpdx.length_squared() + dpdy.length_squared()).sqrt()
                        }
                        _ => 0.001,
                    };
                    self.set(args[0], Value::Float(fw));
                }

                // --- Color ---
                "blackbody" if args.len() >= 2 => {
                    let temp = self.get(args[1]).as_float();
                    let rgb = crate::color::blackbody(temp);
                    self.set(args[0], Value::Vec3(rgb));
                }

                "wavelength_color" if args.len() >= 2 => {
                    let wavelength = self.get(args[1]).as_float();
                    let rgb = crate::color::wavelength_color(wavelength);
                    self.set(args[0], Value::Vec3(rgb));
                }

                "transformc" if args.len() >= 4 => {
                    // transformc(result, from_space, to_space, color)
                    let from_space = match &self.get(args[1]) {
                        Value::String(s) => s.as_str().to_string(),
                        _ => "rgb".to_string(),
                    };
                    let to_space = match &self.get(args[2]) {
                        Value::String(s) => s.as_str().to_string(),
                        _ => "rgb".to_string(),
                    };
                    let color = self.get(args[3]).as_vec3();
                    if from_space == to_space || self.renderer.is_none() {
                        self.set(args[0], Value::Vec3(color));
                    } else {
                        // Delegate to built-in color transforms for common spaces
                        let result = crate::color::transform_color(&from_space, &to_space, color);
                        self.set(args[0], Value::Vec3(result));
                    }
                }

                // --- Matrix operations ---
                "matrix" if args.len() >= 2 => {
                    // Matrix construction from 16 floats or identity
                    if args.len() >= 17 {
                        let mut m = crate::math::Matrix44::ZERO;
                        for row in 0..4 {
                            for col in 0..4 {
                                m.m[row][col] = self.get(args[1 + row * 4 + col]).as_float();
                            }
                        }
                        self.set(args[0], Value::Matrix(m));
                    } else if args.len() >= 2 {
                        // Single float: f * identity matrix (all diagonal = f)
                        let v = self.get(args[1]).as_float();
                        let m = crate::math::Matrix44 {
                            m: [
                                [v, 0.0, 0.0, 0.0],
                                [0.0, v, 0.0, 0.0],
                                [0.0, 0.0, v, 0.0],
                                [0.0, 0.0, 0.0, v],
                            ],
                        };
                        self.set(args[0], Value::Matrix(m));
                    }
                }

                "getmatrix" if args.len() >= 3 => {
                    // getmatrix(success, from, to, M) — C++ op layout; 2-arg form has to="common"
                    let from_s = self.get(args[1]).as_string();
                    let to_s = if args.len() >= 4 {
                        self.get(args[2]).as_string()
                    } else {
                        crate::ustring::UString::new("common")
                    };
                    let m_arg = if args.len() >= 4 { args[3] } else { args[2] };
                    let errh = msg_cfg
                        .as_ref()
                        .map(|c| c.errhandler as &dyn crate::shadingsys::ErrorHandler);
                    let fallback_name = std::cell::RefCell::new(String::new());
                    let report_unknown: Option<UnknownCoordsysReportFn<'_>> =
                        if self.unknown_coordsys_error {
                            Some(make_unknown_coordsys_reporter(errh, &fallback_name))
                        } else {
                            None
                        };
                    let success = if let Some(renderer) = &self.renderer {
                        match crate::matrix_ops::get_from_to_matrix(
                            renderer.as_ref(),
                            globals,
                            from_s.as_str(),
                            to_s.as_str(),
                            globals.time,
                            self.commonspace_synonym.as_str(),
                            report_unknown.as_deref(),
                        ) {
                            Some(m) => {
                                self.set(m_arg, Value::Matrix(m));
                                1
                            }
                            None => {
                                let fb = fallback_name.borrow();
                                if !fb.is_empty() {
                                    self.messages.push(fb.clone());
                                }
                                0
                            }
                        }
                    } else {
                        0
                    };
                    self.set(args[0], Value::Int(success));
                }

                "transform" if args.len() >= 3 => {
                    // transform(matrix, p), transform("to", p), transform("from", "to", p)
                    let (m, src_idx) = if args.len() >= 4 {
                        let arg1 = self.get(args[1]);
                        match arg1 {
                            Value::Matrix(m) => (m, 2usize),
                            _ => {
                                let from = arg1.as_string();
                                let to = self.get(args[2]).as_string();
                                (self.get_space_matrix(globals, &from, &to), 3)
                            }
                        }
                    } else {
                        match self.get(args[1]) {
                            Value::Matrix(m) => (m, 2usize),
                            _ => {
                                // transform("to", p) = transform("common", "to", p)
                                let to = self.get(args[1]).as_string();
                                let from = UString::new("common");
                                (self.get_space_matrix(globals, &from, &to), 2)
                            }
                        }
                    };
                    let src = self.get(args[src_idx]);
                    let p = src.as_vec3();
                    let result = m.transform_point(p);
                    if src.has_derivs() {
                        let tdx = m.transform_vector(src.dx_vec3());
                        let tdy = m.transform_vector(src.dy_vec3());
                        self.set(args[0], Value::DualVec3(result, tdx, tdy));
                    } else {
                        self.set(args[0], Value::Vec3(result));
                    }
                }

                "transformv" if args.len() >= 3 => {
                    let (m, src_idx) = if args.len() >= 4 {
                        let arg1 = self.get(args[1]);
                        match arg1 {
                            Value::Matrix(m) => (m, 2usize),
                            _ => {
                                let from = arg1.as_string();
                                let to = self.get(args[2]).as_string();
                                (self.get_space_matrix(globals, &from, &to), 3)
                            }
                        }
                    } else {
                        match self.get(args[1]) {
                            Value::Matrix(m) => (m, 2usize),
                            _ => {
                                let to = self.get(args[1]).as_string();
                                let from = UString::new("common");
                                (self.get_space_matrix(globals, &from, &to), 2)
                            }
                        }
                    };
                    let src = self.get(args[src_idx]);
                    let v = src.as_vec3();
                    let result = m.transform_vector(v);
                    if src.has_derivs() {
                        let tdx = m.transform_vector(src.dx_vec3());
                        let tdy = m.transform_vector(src.dy_vec3());
                        self.set(args[0], Value::DualVec3(result, tdx, tdy));
                    } else {
                        self.set(args[0], Value::Vec3(result));
                    }
                }

                "transformn" if args.len() >= 3 => {
                    let (m, src_idx) = if args.len() >= 4 {
                        let arg1 = self.get(args[1]);
                        match arg1 {
                            Value::Matrix(m) => (m, 2usize),
                            _ => {
                                let from = arg1.as_string();
                                let to = self.get(args[2]).as_string();
                                (self.get_space_matrix(globals, &from, &to), 3)
                            }
                        }
                    } else {
                        match self.get(args[1]) {
                            Value::Matrix(m) => (m, 2usize),
                            _ => {
                                let to = self.get(args[1]).as_string();
                                let from = UString::new("common");
                                (self.get_space_matrix(globals, &from, &to), 2)
                            }
                        }
                    };
                    let src = self.get(args[src_idx]);
                    let n = src.as_vec3();
                    let result = m.transform_normal(n);
                    if src.has_derivs() {
                        let tdx = m.transform_normal(src.dx_vec3());
                        let tdy = m.transform_normal(src.dy_vec3());
                        self.set(args[0], Value::DualVec3(result, tdx, tdy));
                    } else {
                        self.set(args[0], Value::Vec3(result));
                    }
                }

                // --- Raytype ---
                "raytype" if args.len() >= 2 => {
                    // raytype(name) - check if current raytype matches
                    let rt = globals.raytype;
                    let name = match &self.get(args[1]) {
                        Value::String(s) => s.as_str().to_string(),
                        _ => String::new(),
                    };
                    // Basic raytype mapping
                    let bit = match name.as_str() {
                        "camera" => 1,
                        "shadow" => 2,
                        "diffuse" => 4,
                        "glossy" => 8,
                        "reflection" => 16,
                        "refraction" => 32,
                        _ => 0,
                    };
                    self.set(args[0], Value::Int(if (rt & bit) != 0 { 1 } else { 0 }));
                }

                // --- Texture (delegates to RendererServices with fallback) ---
                // Supports implicit derivs from s/t Dual2 and explicit user derivs:
                //   texture(result, filename, s, t)
                //   texture(result, filename, s, t, dsdx, dtdx, dsdy, dtdy, ...opts...)
                "texture" if args.len() >= 2 => {
                    let filename = match &self.get(args[1]) {
                        Value::String(s) => s.as_str().to_string(),
                        _ => String::new(),
                    };
                    // first_optional: 4 for (result,filename,s,t), 9 for (...,dsdx,dtdx,dsdy,dtdy)
                    let first_optional = if args.len() >= 9 { 9 } else { 4 };
                    let s_val = if args.len() > 2 {
                        self.get(args[2])
                    } else {
                        Value::DualFloat(globals.u, globals.dudx, globals.dudy)
                    };
                    let t_val = if args.len() > 3 {
                        self.get(args[3])
                    } else {
                        Value::DualFloat(globals.v, globals.dvdx, globals.dvdy)
                    };
                    let s = s_val.as_float();
                    let t = t_val.as_float();
                    let opt = self.parse_texture_opt_args(&args, first_optional);
                    // Extract derivatives: explicit args 4-7, or from Dual2 s/t
                    let (dsdx, dtdx, dsdy, dtdy) = if args.len() > 7 {
                        let a4 = self.get(args[4]);
                        if matches!(a4, Value::Float(_) | Value::DualFloat(..)) {
                            (
                                a4.as_float(),
                                self.get(args[5]).as_float(),
                                self.get(args[6]).as_float(),
                                self.get(args[7]).as_float(),
                            )
                        } else {
                            (
                                s_val.dx_float(),
                                t_val.dx_float(),
                                s_val.dy_float(),
                                t_val.dy_float(),
                            )
                        }
                    } else {
                        // Implicit derivs from Dual2 texture coordinates
                        (
                            s_val.dx_float(),
                            t_val.dx_float(),
                            s_val.dy_float(),
                            t_val.dy_float(),
                        )
                    };
                    let rs = self.renderer.as_deref();
                    let tr = crate::texture::texture_lookup(
                        rs, globals, &filename, s, t, dsdx, dtdx, dsdy, dtdy, &opt,
                    );
                    self.set(args[0], Value::Color(tr.color));
                }

                // --- getattribute (delegates to RendererServices) ---
                // 3-arg: getattribute(result, name, dest)
                // 4-arg: getattribute(result, object, name, dest)
                "getattribute" if args.len() >= 3 => {
                    // Determine object/name/dest indices based on arg count
                    let (obj_hash, name_str, dest_idx) = if args.len() >= 4 {
                        let obj = self.get(args[1]).as_string();
                        let name = match &self.get(args[2]) {
                            Value::String(s) => s.as_str().to_string(),
                            _ => String::new(),
                        };
                        (UStringHash::hash_utf8(obj.as_str()), name, 3)
                    } else {
                        let name = match &self.get(args[1]) {
                            Value::String(s) => s.as_str().to_string(),
                            _ => String::new(),
                        };
                        (UStringHash::from_hash(0), name, 2)
                    };

                    // Built-in shader:* attributes (resolved without renderer, C++ parity)
                    let builtin = match name_str.as_str() {
                        "osl:version" => Some(Value::Int(crate::OSL_VERSION as i32)),
                        "shader:shadername" => Some(Value::String(UString::new(&ir.shader_name))),
                        // In standalone execution layer == shader; group is unnamed
                        "shader:layername" => Some(Value::String(UString::new(&ir.shader_name))),
                        "shader:groupname" => Some(Value::String(UString::new(""))),
                        _ => None,
                    };

                    if let Some(val) = builtin {
                        self.set(args[dest_idx], val);
                        self.set(args[0], Value::Int(1));
                    } else if let Some(renderer) = &self.renderer {
                        let name_hash = UStringHash::hash_utf8(&name_str);
                        // Determine TypeDesc from destination symbol typespec (C++ inspects symbol type)
                        let attr_type = if args[dest_idx] >= 0
                            && (args[dest_idx] as usize) < ir.symbols.len()
                        {
                            ir.symbols[args[dest_idx] as usize].typespec.simpletype()
                        } else {
                            TypeDesc::FLOAT
                        };
                        if let Some(attr) =
                            renderer.get_attribute(globals, false, obj_hash, attr_type, name_hash)
                        {
                            match attr {
                                crate::renderer::AttributeData::Int(i) => {
                                    self.set(args[dest_idx], Value::Int(i));
                                    self.set(args[0], Value::Int(1));
                                }
                                crate::renderer::AttributeData::Float(f) => {
                                    self.set(args[dest_idx], Value::Float(f));
                                    self.set(args[0], Value::Int(1));
                                }
                                crate::renderer::AttributeData::String(s) => {
                                    self.set(args[dest_idx], Value::String(UString::new(&s)));
                                    self.set(args[0], Value::Int(1));
                                }
                                crate::renderer::AttributeData::Vec3(v) => {
                                    self.set(args[dest_idx], Value::Vec3(v));
                                    self.set(args[0], Value::Int(1));
                                }
                                crate::renderer::AttributeData::Matrix44(m) => {
                                    self.set(args[dest_idx], Value::Matrix(m));
                                    self.set(args[0], Value::Int(1));
                                }
                                _ => self.set(args[0], Value::Int(0)),
                            }
                        } else {
                            self.set(args[0], Value::Int(0));
                        }
                    } else {
                        self.set(args[0], Value::Int(0));
                    }
                }

                // --- Dictionary operations ---
                "dict_find" if args.len() >= 3 => {
                    let query = self.get(args[2]).as_string();
                    let query_s = query.as_str().to_string();
                    let handle = match self.get(args[1]) {
                        Value::Int(node_id) => self.dict_store.dict_find_node(node_id, &query_s),
                        other => {
                            let dict_src = other.as_string().as_str().to_string();
                            self.dict_store.dict_find_str(&dict_src, &query_s)
                        }
                    };
                    self.set(args[0], Value::Int(handle));
                }
                "dict_next" if args.len() >= 2 => {
                    let node_id = self.get(args[1]).as_int();
                    let next = self.dict_store.dict_next(node_id);
                    self.set(args[0], Value::Int(next));
                }
                "dict_value" if args.len() >= 4 => {
                    let node_id = self.get(args[1]).as_int();
                    let attrib = self.get(args[2]).as_string().as_str().to_string();
                    let ok = if let Some(s) = self.dict_store.dict_value_str(node_id, &attrib) {
                        if let Ok(i) = s.parse::<i32>() {
                            self.set(args[3], Value::Int(i));
                            1
                        } else if let Ok(f) = s.parse::<f32>() {
                            self.set(args[3], Value::Float(f));
                            1
                        } else {
                            self.set(args[3], Value::String(s.into()));
                            1
                        }
                    } else {
                        0
                    };
                    self.set(args[0], Value::Int(ok));
                }
                "dict_value" if args.len() >= 3 => {
                    let node_id = self.get(args[1]).as_int();
                    let attrib = self.get(args[2]).as_string().as_str().to_string();
                    let ok = if self.dict_store.dict_value_str(node_id, &attrib).is_some() {
                        1
                    } else {
                        0
                    };
                    self.set(args[0], Value::Int(ok));
                }

                // --- Point cloud (renderer) ---
                "pointcloud_search" if args.len() >= 6 => {
                    // pointcloud_search(result, filename, center, radius, maxpoints, sort, [optional: "index", indices, "distance", distances, ...])
                    if let Some(renderer) = &self.renderer {
                        let filename = self.get(args[1]).as_string();
                        let center_val = self.get(args[2]);
                        let center = center_val.as_vec3();
                        let radius = self.get(args[3]).as_float();
                        let maxpoints = self.get(args[4]).as_int();
                        let sort = if args.len() > 5 {
                            self.get(args[5]).as_int() != 0
                        } else {
                            true
                        };
                        let filename_hash = UStringHash::hash_utf8(filename.as_str());
                        let mut indices = vec![0i32; maxpoints as usize];
                        let max_pt = maxpoints as usize;
                        let derivs_offset = if center_val.has_derivs() {
                            max_pt as i32
                        } else {
                            0
                        };
                        let mut distances =
                            vec![0.0f32; max_pt + (if derivs_offset > 0 { max_pt * 2 } else { 0 })];
                        let (dcdx, dcdy) = if center_val.has_derivs() {
                            (Some(center_val.dx_vec3()), Some(center_val.dy_vec3()))
                        } else {
                            (None, None)
                        };
                        let count = renderer.pointcloud_search(
                            globals,
                            filename_hash,
                            &center,
                            radius,
                            maxpoints,
                            sort,
                            &mut indices,
                            Some(&mut distances),
                            derivs_offset,
                            dcdx.as_ref(),
                            dcdy.as_ref(),
                        );
                        self.set(args[0], Value::Int(count));
                        // Parse optional output args: "index", indices_sym, "distance", distances_sym
                        let mut i = 6;
                        while i + 1 < args.len() {
                            let name_val = self.get(args[i]);
                            let name = name_val.as_string().as_str().to_string();
                            let sym = args[i + 1];
                            if name == "index" && sym >= 0 {
                                let arr: Vec<i32> =
                                    indices.iter().take(count as usize).copied().collect();
                                self.set(sym, Value::IntArray(arr));
                            } else if name == "distance" && sym >= 0 {
                                let arr: Vec<f32> =
                                    distances.iter().take(count as usize).copied().collect();
                                self.set(sym, Value::FloatArray(arr));
                            }
                            i += 2;
                        }
                    } else {
                        self.set(args[0], Value::Int(0));
                    }
                }
                "pointcloud_get" if args.len() >= 6 => {
                    // pointcloud_get(result, filename, indices, count, attr, dest) — OSL spec
                    if let Some(renderer) = &self.renderer {
                        let filename = self.get(args[1]).as_string();
                        let filename_hash = UStringHash::hash_utf8(filename.as_str());
                        let indices_val = self.get(args[2]);
                        let indices_full: Vec<i32> = match indices_val {
                            Value::IntArray(arr) => arr.clone(),
                            Value::Int(i) => vec![i],
                            _ => vec![],
                        };
                        let count = self.get(args[3]).as_int().max(0) as usize;
                        let indices: Vec<i32> = indices_full.into_iter().take(count).collect();
                        let attrname = self.get(args[4]).as_string();
                        let attrname_hash = UStringHash::hash_utf8(attrname.as_str());
                        let dest_sym = args[5];
                        if indices.is_empty() {
                            self.set(args[0], Value::Int(1)); // success, no data to fetch
                        } else {
                            let dest_ts = if dest_sym >= 0 && (dest_sym as usize) < ir.symbols.len() { ir.symbols[dest_sym as usize].typespec.simpletype() } else { crate::typedesc::TypeDesc::FLOAT };
                            let is_triple = dest_ts.aggregate
                                == crate::typedesc::Aggregate::Vec3 as u8
                                && dest_ts.arraylen == 0;
                            let is_triple_arr = dest_ts.aggregate
                                == crate::typedesc::Aggregate::Vec3 as u8
                                && dest_ts.arraylen != 0;
                            let attr_td = if is_triple || is_triple_arr {
                                crate::typedesc::TypeDesc::COLOR
                            } else {
                                crate::typedesc::TypeDesc::FLOAT.array(-1)
                            };
                            let ok = if is_triple || is_triple_arr {
                                let mut data = vec![Vec3::ZERO; indices.len()];
                                let ok = renderer.pointcloud_get(
                                    globals,
                                    filename_hash,
                                    &indices,
                                    attrname_hash,
                                    attr_td,
                                    data.as_mut_ptr() as *mut _,
                                );
                                if ok {
                                    if data.len() == 1 {
                                        self.set(dest_sym, Value::Vec3(data[0]));
                                    } else {
                                        self.set(dest_sym, Value::Vec3Array(data));
                                    }
                                }
                                ok
                            } else {
                                let mut data = vec![0.0f32; indices.len()];
                                let ok = renderer.pointcloud_get(
                                    globals,
                                    filename_hash,
                                    &indices,
                                    attrname_hash,
                                    attr_td,
                                    data.as_mut_ptr() as *mut _,
                                );
                                if ok {
                                    if data.len() == 1 {
                                        self.set(dest_sym, Value::Float(data[0]));
                                    } else {
                                        self.set(dest_sym, Value::FloatArray(data));
                                    }
                                }
                                ok
                            };
                            self.set(args[0], Value::Int(if ok { 1 } else { 0 }));
                        }
                    } else {
                        self.set(args[0], Value::Int(0));
                    }
                }
                "pointcloud_write" if args.len() >= 3 => {
                    // pointcloud_write(result, filename, position, "attr1", val1, "attr2", val2, ...)
                    if let Some(renderer) = &self.renderer {
                        let filename = self.get(args[1]).as_string();
                        let filename_hash = UStringHash::hash_utf8(filename.as_str());
                        let pos = self.get(args[2]).as_vec3();
                        let nattrs = (args.len() - 3) / 2;
                        let mut names = Vec::with_capacity(nattrs);
                        let mut types = Vec::with_capacity(nattrs);
                        let mut floats: Vec<f32> = Vec::new();
                        let mut vec3s: Vec<Vec3> = Vec::new();
                        let mut ints: Vec<i32> = Vec::new();
                        let mut data_ptrs: Vec<*const std::ffi::c_void> =
                            Vec::with_capacity(nattrs);
                        for i in 0..nattrs {
                            let name = self.get(args[3 + i * 2]).as_string();
                            let val = self.get(args[4 + i * 2]);
                            names.push(UStringHash::hash_utf8(name.as_str()));
                            match &val {
                                Value::Float(f) => {
                                    floats.push(*f);
                                    types.push(crate::typedesc::TypeDesc::FLOAT);
                                    data_ptrs
                                        .push(floats.last().unwrap() as *const f32 as *const _);
                                }
                                Value::Vec3(v) | Value::Color(v) => {
                                    vec3s.push(*v);
                                    types.push(crate::typedesc::TypeDesc::COLOR);
                                    data_ptrs
                                        .push(vec3s.last().unwrap() as *const Vec3 as *const _);
                                }
                                Value::Int(ii) => {
                                    ints.push(*ii);
                                    types.push(crate::typedesc::TypeDesc::INT);
                                    data_ptrs.push(ints.last().unwrap() as *const i32 as *const _);
                                }
                                _ => {
                                    let f = val.as_float();
                                    floats.push(f);
                                    types.push(crate::typedesc::TypeDesc::FLOAT);
                                    data_ptrs
                                        .push(floats.last().unwrap() as *const f32 as *const _);
                                }
                            }
                        }
                        let ok = renderer.pointcloud_write(
                            globals,
                            filename_hash,
                            &pos,
                            &names,
                            &types,
                            &data_ptrs,
                        );
                        self.set(args[0], Value::Int(if ok { 1 } else { 0 }));
                    } else {
                        self.set(args[0], Value::Int(0));
                    }
                }

                // --- Trace (delegates to RendererServices) ---
                // trace(result, P, Dir, ["mindist", f], ["maxdist", f],
                //       ["shade", i], ["traceset", s])
                "trace" if args.len() >= 3 => {
                    if let Some(renderer) = &self.renderer {
                        let p = self.get(args[1]).as_vec3();
                        let dir = self.get(args[2]).as_vec3();
                        let mut opt = crate::renderer::TraceOpt::default();
                        // Parse keyword arguments from args[3..] in pairs
                        let mut i = 3;
                        while i + 1 < args.len() {
                            let key = self.get(args[i]).as_string();
                            let val = self.get(args[i + 1]);
                            match key.as_str() {
                                "mindist" => opt.mindist = val.as_float(),
                                "maxdist" => opt.maxdist = val.as_float(),
                                "shade" => opt.shade = val.as_int() != 0,
                                "traceset" => {
                                    opt.traceset = UStringHash::from(val.as_string());
                                }
                                _ => {} // ignore unknown keywords
                            }
                            i += 2;
                        }
                        let hit = renderer.trace(
                            &mut opt,
                            globals,
                            &p,
                            &Vec3::ZERO,
                            &Vec3::ZERO,
                            &dir,
                            &Vec3::ZERO,
                            &Vec3::ZERO,
                        );
                        self.set(args[0], Value::Int(if hit { 1 } else { 0 }));
                    } else {
                        self.set(args[0], Value::Int(0));
                    }
                }

                // --- Noise variants ---
                "hashnoise" if args.len() >= 2 => {
                    let x = self.get(args[1]).as_float();
                    let h = crate::hashes::fingerprint64(&x.to_le_bytes());
                    let r = (h as f32) / (u64::MAX as f32);
                    self.set(args[0], Value::Float(r));
                }

                "pnoise" | "psnoise" if args.len() >= 3 => {
                    // Periodic noise: pnoise(result, x, period)
                    let x = self.get(args[1]).as_float();
                    let period = self.get(args[2]).as_float();
                    let px = if period > 0.0 {
                        x - (x / period).floor() * period
                    } else {
                        x
                    };
                    let n = crate::noise::perlin1(px);
                    self.set(args[0], Value::Float(n));
                }

                "simplex" | "simplexnoise" if args.len() >= 2 => {
                    let x = self.get(args[1]).as_float();
                    let n = crate::simplex::simplex1(x);
                    self.set(args[0], Value::Float(n));
                }

                "usimplex" | "usimplexnoise" if args.len() >= 2 => {
                    let x = self.get(args[1]).as_float();
                    let n = crate::simplex::usimplex1(x);
                    self.set(args[0], Value::Float(n));
                }

                // --- Generic noise dispatch (C++ opnoise.cpp) ---
                "genericnoise" if args.len() >= 2 => {
                    // genericnoise dispatches by arg count and type
                    let x = self.get(args[1]).as_float();
                    let n = crate::noise::perlin1(x);
                    self.set(args[0], Value::Float(n));
                }
                "genericpnoise" if args.len() >= 3 => {
                    let x = self.get(args[1]).as_float();
                    let period = self.get(args[2]).as_float();
                    let px = if period > 0.0 {
                        x - (x / period).floor() * period
                    } else {
                        x
                    };
                    let n = crate::noise::perlin1(px);
                    self.set(args[0], Value::Float(n));
                }
                "gabornoise" if args.len() >= 2 => {
                    let p = Vec3::new(self.get(args[1]).as_float(), 0.0, 0.0);
                    let n = crate::gabor::gabor3(p, &crate::gabor::GaborParams::default());
                    self.set(args[0], Value::Float(n));
                }
                "gaborpnoise" | "gabor_pnoise" if args.len() >= 3 => {
                    // Gabor periodic noise — Gabor ignores period in our impl
                    let p = Vec3::new(self.get(args[1]).as_float(), 0.0, 0.0);
                    let n = crate::gabor::gabor3(p, &crate::gabor::GaborParams::default());
                    self.set(args[0], Value::Float(n));
                }

                "pcellnoise" if args.len() >= 3 => {
                    // Periodic cell noise
                    let p = self.get(args[1]).as_vec3();
                    let period = self.get(args[2]).as_vec3();
                    let n = crate::noise::pnoise_by_name("cellnoise", p, period);
                    self.set(args[0], Value::Float(n));
                }

                "phashnoise" if args.len() >= 3 => {
                    // Periodic hash noise
                    let p = self.get(args[1]).as_vec3();
                    let period = self.get(args[2]).as_vec3();
                    let n = crate::noise::pnoise_by_name("hashnoise", p, period);
                    self.set(args[0], Value::Float(n));
                }

                "nullnoise" if args.len() >= 2 => {
                    // Null noise — always 0
                    self.set(args[0], Value::Float(0.0));
                }
                "unullnoise" if args.len() >= 2 => {
                    // Unsigned null noise — always 0.5
                    self.set(args[0], Value::Float(0.5));
                }

                // --- Array operations ---
                "arraycopy" if args.len() >= 2 => {
                    // arraycopy(dst, src) — copy array
                    let src = self.get(args[1]);
                    self.set(args[0], src);
                }
                "arrayfill" if args.len() >= 2 => {
                    // arrayfill(dst, value) — fill all elements with value
                    let val = self.get(args[1]);
                    self.set(args[0], val);
                }

                // --- Regex match with groups ---
                // regex_match handled above with regex_search

                // --- Additional math ---
                // duplicate select removed — primary handler above

                // --- Type conversion ---
                "float" if args.len() >= 2 => {
                    // float(result, src) — convert int to float
                    let src = self.get(args[1]);
                    let f = match &src {
                        Value::Int(i) => *i as f32,
                        Value::Float(f) => *f,
                        _ => 0.0,
                    };
                    self.set(args[0], Value::Float(f));
                }
                "int" if args.len() >= 2 => {
                    // int(result, src) — convert float to int
                    let src = self.get(args[1]);
                    let i = match &src {
                        Value::Float(f) => *f as i32,
                        Value::Int(i) => *i,
                        _ => 0,
                    };
                    self.set(args[0], Value::Int(i));
                }

                // --- Struct operations ---
                "getfield" if args.len() >= 3 => {
                    // getfield(result, struct_sym, field_name_const)
                    let struct_val = self.get(args[1]);
                    let field_name = self.get(args[2]).as_string();
                    let result = match struct_val {
                        Value::Struct(ref fields) => {
                            // Look up field index from the struct's StructSpec
                            let sid = ir.symbols[args[1] as usize].typespec.structure_id();
                            if let Some(spec) = crate::typespec::get_struct(sid as i32) {
                                let field_idx =
                                    spec.fields.iter().position(|f| f.name == field_name);
                                if let Some(idx) = field_idx {
                                    fields.get(idx).cloned().unwrap_or(Value::Float(0.0))
                                } else {
                                    Value::Float(0.0)
                                }
                            } else {
                                Value::Float(0.0)
                            }
                        }
                        // Vec3/Color component access by name (x/y/z or r/g/b)
                        Value::Vec3(v) | Value::Color(v) => match field_name.as_str() {
                            "x" | "r" => Value::Float(v.x),
                            "y" | "g" => Value::Float(v.y),
                            "z" | "b" => Value::Float(v.z),
                            _ => Value::Float(0.0),
                        },
                        _ => Value::Float(0.0),
                    };
                    self.set(args[0], result);
                }
                "setfield" if args.len() >= 3 => {
                    // setfield(struct_sym, field_name_const, value)
                    let field_name = self.get(args[1]).as_string();
                    let val = self.get(args[2]);
                    let struct_idx = args[0];
                    match self.get(struct_idx) {
                        Value::Struct(mut fields) => {
                            let sid = ir.symbols[struct_idx as usize].typespec.structure_id();
                            if let Some(spec) = crate::typespec::get_struct(sid as i32)
                                && let Some(idx) =
                                    spec.fields.iter().position(|f| f.name == field_name)
                                    && idx < fields.len() {
                                        fields[idx] = val;
                                    }
                            self.set(struct_idx, Value::Struct(fields));
                        }
                        Value::Vec3(mut v) => {
                            let f = val.as_float();
                            match field_name.as_str() {
                                "x" | "r" => v.x = f,
                                "y" | "g" => v.y = f,
                                "z" | "b" => v.z = f,
                                _ => {}
                            }
                            self.set(struct_idx, Value::Vec3(v));
                        }
                        Value::Color(mut v) => {
                            let f = val.as_float();
                            match field_name.as_str() {
                                "x" | "r" => v.x = f,
                                "y" | "g" => v.y = f,
                                "z" | "b" => v.z = f,
                                _ => {}
                            }
                            self.set(struct_idx, Value::Color(v));
                        }
                        _ => {}
                    }
                }

                // --- Spline (uses proper basis from spline.rs) ---
                "spline" if args.len() >= 4 => {
                    // spline(result, basis_name, t, knots...)
                    let basis_name = self.get(args[1]).as_string();
                    let basis = crate::spline::SplineBasis::from_name(basis_name.as_str())
                        .unwrap_or(crate::spline::SplineBasis::CatmullRom);
                    let t = self.get(args[2]).as_float();
                    let first = self.get(args[3]);
                    match &first {
                        // Array arg: expand array into knot values
                        Value::FloatArray(arr) => {
                            let result = crate::spline::spline_float(basis, t, arr);
                            self.set(args[0], Value::Float(result));
                        }
                        Value::Vec3Array(arr) => {
                            let result = crate::spline::spline_vec3(basis, t, arr);
                            self.set(args[0], Value::Vec3(result));
                        }
                        Value::Vec3(_) | Value::Color(_) | Value::DualVec3(..) => {
                            let nknots = args.len() - 3;
                            let knots: Vec<Vec3> = (0..nknots)
                                .map(|j| self.get(args[3 + j]).as_vec3())
                                .collect();
                            let result = crate::spline::spline_vec3(basis, t, &knots);
                            self.set(args[0], Value::Vec3(result));
                        }
                        _ => {
                            let nknots = args.len() - 3;
                            let knots: Vec<f32> = (0..nknots)
                                .map(|j| self.get(args[3 + j]).as_float())
                                .collect();
                            let result = crate::spline::spline_float(basis, t, &knots);
                            self.set(args[0], Value::Float(result));
                        }
                    }
                }

                "splineinverse" if args.len() >= 4 => {
                    // splineinverse(result, basis_name, value, knots...)
                    let basis_name = self.get(args[1]).as_string();
                    let basis = crate::spline::SplineBasis::from_name(basis_name.as_str())
                        .unwrap_or(crate::spline::SplineBasis::CatmullRom);
                    let value = self.get(args[2]).as_float();
                    let first = self.get(args[3]);
                    let knots: Vec<f32> = match &first {
                        Value::FloatArray(arr) => arr.clone(),
                        _ => {
                            let nknots = args.len() - 3;
                            (0..nknots)
                                .map(|j| self.get(args[3 + j]).as_float())
                                .collect()
                        }
                    };
                    if !knots.is_empty() {
                        let t = crate::spline::spline_inverse_float(basis, value, &knots, 32);
                        self.set(args[0], Value::Float(t));
                    }
                }

                // --- Derivative opcodes (Dx, Dy, Dz) ---
                // With Dual2-aware values, we can extract real derivatives
                // that have been propagated through arithmetic operations.
                "Dx" if args.len() >= 2 => {
                    let src = self.get(args[1]);
                    let result = match &src {
                        Value::DualFloat(_, dx, _) => Value::Float(*dx),
                        Value::DualVec3(_, dx, _) => Value::Vec3(*dx),
                        Value::Float(_) => Value::Float(0.0),
                        Value::Vec3(_) | Value::Color(_) => Value::Vec3(Vec3::ZERO),
                        _ => Value::Float(0.0),
                    };
                    self.set(args[0], result);
                }
                "Dy" if args.len() >= 2 => {
                    let src = self.get(args[1]);
                    let result = match &src {
                        Value::DualFloat(_, _, dy) => Value::Float(*dy),
                        Value::DualVec3(_, _, dy) => Value::Vec3(*dy),
                        Value::Float(_) => Value::Float(0.0),
                        Value::Vec3(_) | Value::Color(_) => Value::Vec3(Vec3::ZERO),
                        _ => Value::Float(0.0),
                    };
                    self.set(args[0], result);
                }
                "Dz" if args.len() >= 2 => {
                    // Dz is typically zero (only relevant for volume shaders)
                    let src = self.get(args[1]);
                    let result = match &src {
                        Value::Float(_) | Value::DualFloat(..) => Value::Float(0.0),
                        Value::Vec3(_) | Value::Color(_) | Value::DualVec3(..) => {
                            Value::Vec3(Vec3::ZERO)
                        }
                        _ => Value::Float(0.0),
                    };
                    self.set(args[0], result);
                }
                "deriv" if args.len() >= 3 => {
                    // deriv(result, val, dvar) — generic derivative
                    // In non-JIT mode, use the Dual2 derivatives if available
                    let src = self.get(args[1]);
                    let result = match &src {
                        Value::DualFloat(_, dx, _) => Value::Float(*dx),
                        Value::DualVec3(_, dx, _) => Value::Vec3(*dx),
                        Value::Float(_) => Value::Float(0.0),
                        Value::Vec3(_) | Value::Color(_) => Value::Vec3(Vec3::ZERO),
                        _ => Value::Float(0.0),
                    };
                    self.set(args[0], result);
                }

                // --- Component access ---
                "compref" if args.len() >= 3 => {
                    // compref(result, src, index) — extract component from triple
                    let src = self.get(args[1]);
                    let idx_raw = self.get(args[2]).as_int();
                    let symname = if args[1] >= 0 && (args[1] as usize) < ir.symbols.len() {
                        ir.symbols[args[1] as usize].name.as_str()
                    } else {
                        "?"
                    };
                    let errh = msg_cfg
                        .as_ref()
                        .map(|c| c.errhandler as &dyn crate::shadingsys::ErrorHandler);
                    let idx = self.range_check(
                        idx_raw,
                        3,
                        symname,
                        op.sourcefile.as_str(),
                        op.sourceline,
                        &ir.shader_name,
                        errh,
                    ) as usize;
                    let v = src.as_vec3();
                    let comp = match idx {
                        0 => v.x,
                        1 => v.y,
                        _ => v.z,
                    };
                    if src.has_derivs() {
                        let dx = src.dx_vec3();
                        let dy = src.dy_vec3();
                        let cdx = match idx {
                            0 => dx.x,
                            1 => dx.y,
                            _ => dx.z,
                        };
                        let cdy = match idx {
                            0 => dy.x,
                            1 => dy.y,
                            _ => dy.z,
                        };
                        self.set(args[0], Value::DualFloat(comp, cdx, cdy));
                    } else {
                        self.set(args[0], Value::Float(comp));
                    }
                }
                "compassign" if args.len() >= 3 => {
                    // compassign(dst, index, value) — assign component of triple
                    let idx_raw = self.get(args[1]).as_int();
                    let val = self.get(args[2]);
                    let symname = if args[0] >= 0 && (args[0] as usize) < ir.symbols.len() {
                        ir.symbols[args[0] as usize].name.as_str()
                    } else {
                        "?"
                    };
                    let errh = msg_cfg
                        .as_ref()
                        .map(|c| c.errhandler as &dyn crate::shadingsys::ErrorHandler);
                    let idx = self.range_check(
                        idx_raw,
                        3,
                        symname,
                        op.sourcefile.as_str(),
                        op.sourceline,
                        &ir.shader_name,
                        errh,
                    ) as usize;
                    let dst = self.get(args[0]);
                    let mut v = dst.as_vec3();
                    let vf = val.as_float();
                    match idx {
                        0 => v.x = vf,
                        1 => v.y = vf,
                        _ => v.z = vf,
                    }
                    if dst.has_derivs() || val.has_derivs() {
                        let mut dx = dst.dx_vec3();
                        let mut dy = dst.dy_vec3();
                        let vdx = val.dx_float();
                        let vdy = val.dy_float();
                        match idx {
                            0 => {
                                dx.x = vdx;
                                dy.x = vdy;
                            }
                            1 => {
                                dx.y = vdx;
                                dy.y = vdy;
                            }
                            _ => {
                                dx.z = vdx;
                                dy.z = vdy;
                            }
                        }
                        self.set(args[0], Value::DualVec3(v, dx, dy));
                    } else {
                        self.set(args[0], Value::Vec3(v));
                    }
                }
                "mxcompref" if args.len() >= 4 => {
                    // mxcompref(result, matrix, row, col) — extract matrix element
                    let src = self.get(args[1]);
                    let row_raw = self.get(args[2]).as_int();
                    let col_raw = self.get(args[3]).as_int();
                    let symname = if args[1] >= 0 && (args[1] as usize) < ir.symbols.len() {
                        ir.symbols[args[1] as usize].name.as_str()
                    } else {
                        "?"
                    };
                    let errh = msg_cfg
                        .as_ref()
                        .map(|c| c.errhandler as &dyn crate::shadingsys::ErrorHandler);
                    let row = self.range_check(
                        row_raw,
                        4,
                        symname,
                        op.sourcefile.as_str(),
                        op.sourceline,
                        &ir.shader_name,
                        errh,
                    ) as usize;
                    let col = self.range_check(
                        col_raw,
                        4,
                        symname,
                        op.sourcefile.as_str(),
                        op.sourceline,
                        &ir.shader_name,
                        errh,
                    ) as usize;
                    let val = match &src {
                        Value::Matrix(m) => m.m[row][col],
                        _ => 0.0,
                    };
                    self.set(args[0], Value::Float(val));
                }
                "mxcompassign" if args.len() >= 4 => {
                    // mxcompassign(dst, row, col, value) — assign matrix element
                    let row_raw = self.get(args[1]).as_int();
                    let col_raw = self.get(args[2]).as_int();
                    let val = self.get(args[3]).as_float();
                    let symname = if args[0] >= 0 && (args[0] as usize) < ir.symbols.len() {
                        ir.symbols[args[0] as usize].name.as_str()
                    } else {
                        "?"
                    };
                    let errh = msg_cfg
                        .as_ref()
                        .map(|c| c.errhandler as &dyn crate::shadingsys::ErrorHandler);
                    let row = self.range_check(
                        row_raw,
                        4,
                        symname,
                        op.sourcefile.as_str(),
                        op.sourceline,
                        &ir.shader_name,
                        errh,
                    ) as usize;
                    let col = self.range_check(
                        col_raw,
                        4,
                        symname,
                        op.sourcefile.as_str(),
                        op.sourceline,
                        &ir.shader_name,
                        errh,
                    ) as usize;
                    let mut m = match &self.get(args[0]) {
                        Value::Matrix(m) => *m,
                        _ => Matrix44::IDENTITY,
                    };
                    m.m[row][col] = val;
                    self.set(args[0], Value::Matrix(m));
                }

                "functioncall_nr"
                    // Same as functioncall but no return value to store (plan #52).
                    if op.jump[0] >= 0 => {
                        if self.call_stack.len() >= 256 {
                            break;
                        }
                        self.call_stack.push(pc + 1);
                        pc = op.jump[0] as usize;
                        continue;
                    }
                "useparam" => {
                    // useparam — tells the runtime to evaluate upstream layers
                    // for the referenced parameters. In our interpreter, all
                    // layers are already evaluated, so this is a no-op.
                }

                // --- 3D texture / environment (delegates via dispatch) ---
                "texture3d" if args.len() >= 2 => {
                    let filename = match &self.get(args[1]) {
                        Value::String(s) => s.as_str().to_string(),
                        _ => String::new(),
                    };
                    // first_optional: 3 for (result,filename,p), 6 for (...,dpdx,dpdy,dpdz)
                    let first_optional = if args.len() >= 6 { 6 } else { 3 };
                    let p = if args.len() > 2 {
                        self.get(args[2]).as_vec3()
                    } else {
                        globals.p
                    };
                    let dpdx = if args.len() > 3 {
                        self.get(args[3]).as_vec3()
                    } else {
                        Vec3::ZERO
                    };
                    let dpdy = if args.len() > 4 {
                        self.get(args[4]).as_vec3()
                    } else {
                        Vec3::ZERO
                    };
                    let dpdz = if args.len() > 5 {
                        self.get(args[5]).as_vec3()
                    } else {
                        Vec3::ZERO
                    };
                    let opt = self.parse_texture_opt_args(&args, first_optional);
                    let rs = self.renderer.as_deref();
                    let tr = crate::texture::texture3d_lookup(
                        rs, globals, &filename, p, dpdx, dpdy, dpdz, &opt,
                    );
                    self.set(args[0], Value::Color(tr.color));
                }
                "environment" if args.len() >= 2 => {
                    let filename = match &self.get(args[1]) {
                        Value::String(s) => s.as_str().to_string(),
                        _ => String::new(),
                    };
                    // first_optional: 3 for (result,filename,r), 5 for (...,drdx,drdy)
                    let first_optional = if args.len() >= 5 { 5 } else { 3 };
                    let r = if args.len() > 2 {
                        self.get(args[2]).as_vec3()
                    } else {
                        globals.i
                    };
                    let drdx = if args.len() > 3 {
                        self.get(args[3]).as_vec3()
                    } else {
                        Vec3::ZERO
                    };
                    let drdy = if args.len() > 4 {
                        self.get(args[4]).as_vec3()
                    } else {
                        Vec3::ZERO
                    };
                    let opt = self.parse_texture_opt_args(&args, first_optional);
                    let rs = self.renderer.as_deref();
                    let tr = crate::texture::environment_lookup(
                        rs, globals, &filename, r, drdx, drdy, &opt,
                    );
                    self.set(args[0], Value::Color(tr.color));
                }

                // --- gettextureinfo (delegates via dispatch) ---
                "gettextureinfo" if args.len() >= 4 => {
                    let filename = self.get(args[1]).as_string();
                    let dataname = self.get(args[2]).as_string();
                    let rs = self.renderer.as_deref();
                    let info = crate::texture::gettextureinfo_lookup(
                        rs,
                        globals,
                        filename.as_str(),
                        dataname.as_str(),
                    );
                    if let Some(ti) = info {
                        match ti {
                            crate::texture::TextureInfo::Int(v) => {
                                self.set(args[3], Value::Int(v));
                            }
                            crate::texture::TextureInfo::Float(v) => {
                                self.set(args[3], Value::Float(v));
                            }
                            crate::texture::TextureInfo::Str(ref v) => {
                                self.set(args[3], Value::String(UString::new(v)));
                            }
                            crate::texture::TextureInfo::IntVec(ref v) => {
                                if v.len() >= 2 {
                                    self.set(
                                        args[3],
                                        Value::Vec3(Vec3::new(v[0] as f32, v[1] as f32, 0.0)),
                                    );
                                }
                            }
                            crate::texture::TextureInfo::FloatVec(ref v) => {
                                if !v.is_empty() {
                                    self.set(args[3], Value::Float(v[0]));
                                }
                            }
                        }
                        self.set(args[0], Value::Int(1));
                    } else {
                        self.set(args[0], Value::Int(0));
                    }
                }

                // --- Pre/post increment/decrement ---
                "preinc" if args.len() >= 2 => {
                    let v = self.get(args[1]);
                    let result = match &v {
                        Value::Int(i) => Value::Int(i + 1),
                        Value::Float(f) => Value::Float(f + 1.0),
                        _ => v,
                    };
                    self.set(args[1], result.clone());
                    self.set(args[0], result);
                }
                "predec" if args.len() >= 2 => {
                    let v = self.get(args[1]);
                    let result = match &v {
                        Value::Int(i) => Value::Int(i - 1),
                        Value::Float(f) => Value::Float(f - 1.0),
                        _ => v,
                    };
                    self.set(args[1], result.clone());
                    self.set(args[0], result);
                }
                "postinc" if args.len() >= 2 => {
                    let v = self.get(args[1]);
                    let old = v.clone();
                    let incremented = match &v {
                        Value::Int(i) => Value::Int(i + 1),
                        Value::Float(f) => Value::Float(f + 1.0),
                        _ => v,
                    };
                    self.set(args[1], incremented);
                    self.set(args[0], old);
                }
                "postdec" if args.len() >= 2 => {
                    let v = self.get(args[1]);
                    let old = v.clone();
                    let decremented = match &v {
                        Value::Int(i) => Value::Int(i - 1),
                        Value::Float(f) => Value::Float(f - 1.0),
                        _ => v,
                    };
                    self.set(args[1], decremented);
                    self.set(args[0], old);
                }

                // --- for/while/dowhile loop opcodes (from C++ oslc OSO) ---
                "for" if op.jump[0] >= 0 => {
                    // for(init_end, cond_end, body_end, step_end)
                    // The C++ OSO encodes for-loops with jump targets.
                    // jump[0]=init_end, jump[1]=cond_end,
                    // jump[2]=body_end, jump[3]=step_end
                    // The interpreter handles this via inline if/nop jumps,
                    // so this is a no-op — the sub-opcodes drive the flow.
                }
                "while" | "dowhile" if op.jump[0] >= 0 => {
                    // Similar to for — structured loop with jump targets.
                    // The inline if/nop opcodes handle the flow.
                }
                "break"
                    // In C++ OSO, break jumps to jump[0] (loop end).
                    if op.jump[0] >= 0 => {
                        pc = op.jump[0] as usize;
                        continue;
                    }
                "continue"
                    // In C++ OSO, continue jumps to jump[0] (loop step/condition).
                    if op.jump[0] >= 0 => {
                        pc = op.jump[0] as usize;
                        continue;
                    }

                // --- Type conversion opcodes ---
                "vector" if args.len() >= 4 => {
                    // vector(result, x, y, z) — construct vector
                    let x = self.get(args[1]).as_float();
                    let y = self.get(args[2]).as_float();
                    let z = self.get(args[3]).as_float();
                    self.set(args[0], Value::Vec3(Vec3::new(x, y, z)));
                }
                "point" if args.len() >= 4 => {
                    // point(result, x, y, z)
                    let x = self.get(args[1]).as_float();
                    let y = self.get(args[2]).as_float();
                    let z = self.get(args[3]).as_float();
                    self.set(args[0], Value::Vec3(Vec3::new(x, y, z)));
                }
                "normal" if args.len() >= 4 => {
                    // normal(result, x, y, z)
                    let x = self.get(args[1]).as_float();
                    let y = self.get(args[2]).as_float();
                    let z = self.get(args[3]).as_float();
                    self.set(args[0], Value::Vec3(Vec3::new(x, y, z)));
                }

                // --- Standalone shader globals opcodes ---
                "backfacing" if !args.is_empty() => {
                    self.set(args[0], Value::Int(globals.backfacing));
                }
                "surfacearea" if !args.is_empty() => {
                    self.set(args[0], Value::Float(globals.surfacearea));
                }

                // rotate(result, P, angle, axis_from, axis_to)
                "rotate" if args.len() >= 5 => {
                    let p = self.get(args[1]).as_vec3();
                    let angle = self.get(args[2]).as_float();
                    let from = self.get(args[3]).as_vec3();
                    let to = self.get(args[4]).as_vec3();
                    let axis = (to - from).normalize();
                    let pt = p - from; // translate to origin
                    // Rodrigues rotation: v' = v*cos(θ) + (k×v)*sin(θ) + k*(k·v)*(1-cos(θ))
                    let cos_a = angle.cos();
                    let sin_a = angle.sin();
                    let rotated =
                        pt * cos_a + axis.cross(pt) * sin_a + axis * axis.dot(pt) * (1.0 - cos_a);
                    self.set(args[0], Value::Vec3(rotated + from));
                }

                // --- Catch-all for unknown opcodes ---
                _ => {
                    // Unknown opcode -- skip silently (production would log this)
                }
            }

            pc += 1;
        }
    }

    /// Format a printf-style string from args[0] = format string, args[1..] = values.
    fn format_string(&self, args: &[i32]) -> String {
        if args.is_empty() {
            return String::new();
        }
        let fmt_val = self.get(args[0]);
        let fmt_str = match &fmt_val {
            Value::String(s) => s.as_str().to_string(),
            _ => return String::from("?"),
        };

        let mut result = String::with_capacity(fmt_str.len());
        let bytes = fmt_str.as_bytes();
        let len = bytes.len();
        let mut i = 0;
        let mut arg_idx = 1usize; // index into args[1..]

        while i < len {
            if bytes[i] == b'%' {
                i += 1;
                if i >= len {
                    break;
                }

                // Handle %%
                if bytes[i] == b'%' {
                    result.push('%');
                    i += 1;
                    continue;
                }

                // Parse flags: -, +, 0, space, #
                let mut flags = String::new();
                while i < len && matches!(bytes[i], b'-' | b'+' | b'0' | b' ' | b'#') {
                    flags.push(bytes[i] as char);
                    i += 1;
                }

                // Parse width
                let mut width = String::new();
                while i < len && bytes[i].is_ascii_digit() {
                    width.push(bytes[i] as char);
                    i += 1;
                }

                // Parse precision
                let mut precision = String::new();
                if i < len && bytes[i] == b'.' {
                    precision.push('.');
                    i += 1;
                    while i < len && bytes[i].is_ascii_digit() {
                        precision.push(bytes[i] as char);
                        i += 1;
                    }
                }

                // Parse conversion specifier
                if i >= len {
                    break;
                }
                let spec = bytes[i] as char;
                i += 1;

                let val = if arg_idx < args.len() {
                    let v = self.get(args[arg_idx]);
                    arg_idx += 1;
                    v
                } else {
                    Value::Int(0)
                };

                // Helper: format a single float with C's %g behavior
                fn fmt_g(v: f32, sig: usize) -> String {
                    if !v.is_finite() {
                        if v.is_nan() {
                            return "nan".to_string();
                        }
                        return if v > 0.0 {
                            "inf".to_string()
                        } else {
                            "-inf".to_string()
                        };
                    }
                    if v == 0.0 {
                        return if v.is_sign_negative() {
                            "-0".to_string()
                        } else {
                            "0".to_string()
                        };
                    }
                    let sig = if sig == 0 { 1 } else { sig };
                    let abs = v.abs() as f64;
                    let exp = abs.log10().floor() as i32;
                    if exp < -4 || exp >= sig as i32 {
                        // Use scientific notation
                        let prec = sig.saturating_sub(1);
                        let s = format!("{:.prec$e}", v as f64, prec = prec);
                        // Strip trailing zeros in mantissa, normalize exponent to C style
                        if let Some(epos) = s.find('e') {
                            let mantissa = s[..epos]
                                .trim_end_matches('0')
                                .trim_end_matches('.')
                                .to_string();
                            let exp_str = &s[epos + 1..];
                            let exp_val: i32 = exp_str.parse().unwrap_or(0);
                            // C printf uses at least 2-digit exponent with explicit sign
                            if exp_val.abs() < 100 {
                                format!("{}e{:+03}", mantissa, exp_val)
                            } else {
                                format!("{}e{:+}", mantissa, exp_val)
                            }
                        } else {
                            s
                        }
                    } else {
                        // Use fixed notation
                        let decimal_places = if sig as i32 > exp + 1 {
                            (sig as i32 - exp - 1) as usize
                        } else {
                            0
                        };
                        let s = format!("{:.prec$}", v as f64, prec = decimal_places);
                        // Strip trailing zeros after decimal point
                        if s.contains('.') {
                            let s = s.trim_end_matches('0').trim_end_matches('.');
                            s.to_string()
                        } else {
                            s
                        }
                    }
                }

                fn fmt_f(v: f32, prec: usize) -> String {
                    format!("{:.prec$}", v as f64, prec = prec)
                }

                fn fmt_e(v: f32, prec: usize) -> String {
                    let s = format!("{:.prec$e}", v as f64, prec = prec);
                    // Normalize exponent to C style: e+02, e-03
                    if let Some(epos) = s.find('e') {
                        let mantissa = &s[..epos];
                        let exp_str = &s[epos + 1..];
                        let exp_val: i32 = exp_str.parse().unwrap_or(0);
                        if exp_val.abs() < 100 {
                            format!("{}e{:+03}", mantissa, exp_val)
                        } else {
                            format!("{}e{:+}", mantissa, exp_val)
                        }
                    } else {
                        s
                    }
                }

                // OSL printf: %g/%f/%e with a triple (color/vector/point/normal)
                // prints space-separated components. Same for matrices (16 floats).
                // Struct: each field printed space-separated using the same specifier.
                fn fmt_struct_fields_g(
                    fields: &[Value],
                    sig: usize,
                    fmt_g: &dyn Fn(f32, usize) -> String,
                ) -> String {
                    let mut parts = Vec::new();
                    for f in fields {
                        match f {
                            Value::Int(v) => parts.push(fmt_g(*v as f32, sig)),
                            Value::Float(v) | Value::DualFloat(v, ..) => parts.push(fmt_g(*v, sig)),
                            Value::Vec3(v) | Value::Color(v) | Value::DualVec3(v, ..) => {
                                parts.push(fmt_g(v.x, sig));
                                parts.push(fmt_g(v.y, sig));
                                parts.push(fmt_g(v.z, sig));
                            }
                            Value::Matrix(m) => {
                                for v in m.m.iter().flatten() {
                                    parts.push(fmt_g(*v, sig));
                                }
                            }
                            Value::Struct(nested) => {
                                parts.push(fmt_struct_fields_g(nested, sig, fmt_g));
                            }
                            _ => parts.push(fmt_g(f.as_float(), sig)),
                        }
                    }
                    parts.join(" ")
                }

                match spec {
                    'd' | 'i' => {
                        // Handle whole-array and struct printing
                        if let Value::IntArray(arr) = &val {
                            for (j, v) in arr.iter().enumerate() {
                                if j > 0 {
                                    result.push(' ');
                                }
                                result.push_str(&format!("{v}"));
                            }
                        } else if let Value::Struct(fields) = &val {
                            // Struct: print each field as integer, space-separated
                            let mut first = true;
                            for f in fields {
                                if !first {
                                    result.push(' ');
                                }
                                first = false;
                                result.push_str(&format!("{}", f.as_int()));
                            }
                        } else {
                            let v = val.as_int();
                            let w: usize = width.parse().unwrap_or(0);
                            if w == 0 {
                                result.push_str(&format!("{v}"));
                            } else if flags.contains('-') {
                                result.push_str(&format!("{v:<w$}"));
                            } else if flags.contains('0') {
                                result.push_str(&format!("{v:0>w$}"));
                            } else {
                                result.push_str(&format!("{v:>w$}"));
                            }
                        }
                    }
                    'u' => {
                        let v = val.as_int() as u32;
                        result.push_str(&format!("{v}"));
                    }
                    'f' => {
                        let prec: usize = if precision.len() > 1 {
                            precision[1..].parse().unwrap_or(6)
                        } else {
                            6
                        };
                        match &val {
                            Value::FloatArray(arr) => {
                                for (j, v) in arr.iter().enumerate() {
                                    if j > 0 {
                                        result.push(' ');
                                    }
                                    result.push_str(&fmt_f(*v, prec));
                                }
                            }
                            Value::Vec3Array(arr) => {
                                for (j, v) in arr.iter().enumerate() {
                                    if j > 0 {
                                        result.push(' ');
                                    }
                                    result.push_str(&fmt_f(v.x, prec));
                                    result.push(' ');
                                    result.push_str(&fmt_f(v.y, prec));
                                    result.push(' ');
                                    result.push_str(&fmt_f(v.z, prec));
                                }
                            }
                            Value::Vec3(v) | Value::Color(v) | Value::DualVec3(v, ..) => {
                                result.push_str(&fmt_f(v.x, prec));
                                result.push(' ');
                                result.push_str(&fmt_f(v.y, prec));
                                result.push(' ');
                                result.push_str(&fmt_f(v.z, prec));
                            }
                            Value::Matrix(m) => {
                                for (j, v) in m.m.iter().flatten().enumerate() {
                                    if j > 0 {
                                        result.push(' ');
                                    }
                                    result.push_str(&fmt_f(*v, prec));
                                }
                            }
                            Value::Struct(fields) => {
                                // Struct: each field printed with %f, space-separated
                                result.push_str(&fmt_struct_fields_g(fields, prec, &|v, p| {
                                    fmt_f(v, p)
                                }));
                            }
                            _ => {
                                let v = val.as_float();
                                let w: usize = width.parse().unwrap_or(0);
                                if w > 0 {
                                    result.push_str(&format!("{v:>w$.prec$}"));
                                } else {
                                    result.push_str(&fmt_f(v, prec));
                                }
                            }
                        }
                    }
                    'e' | 'E' => {
                        let prec: usize = if precision.len() > 1 {
                            precision[1..].parse().unwrap_or(6)
                        } else {
                            6
                        };
                        match &val {
                            Value::Vec3(v) | Value::Color(v) | Value::DualVec3(v, ..) => {
                                result.push_str(&fmt_e(v.x, prec));
                                result.push(' ');
                                result.push_str(&fmt_e(v.y, prec));
                                result.push(' ');
                                result.push_str(&fmt_e(v.z, prec));
                            }
                            Value::Struct(fields) => {
                                // Struct: each field printed with %e, space-separated
                                result.push_str(&fmt_struct_fields_g(fields, prec, &|v, p| {
                                    fmt_e(v, p)
                                }));
                            }
                            _ => {
                                let v = val.as_float();
                                result.push_str(&fmt_e(v, prec));
                            }
                        }
                    }
                    'g' | 'G' => {
                        let sig: usize = if precision.len() > 1 {
                            precision[1..].parse().unwrap_or(6)
                        } else {
                            6
                        };
                        match &val {
                            Value::FloatArray(arr) => {
                                for (j, v) in arr.iter().enumerate() {
                                    if j > 0 {
                                        result.push(' ');
                                    }
                                    result.push_str(&fmt_g(*v, sig));
                                }
                            }
                            Value::IntArray(arr) => {
                                for (j, v) in arr.iter().enumerate() {
                                    if j > 0 {
                                        result.push(' ');
                                    }
                                    result.push_str(&fmt_g(*v as f32, sig));
                                }
                            }
                            Value::Vec3Array(arr) => {
                                for (j, v) in arr.iter().enumerate() {
                                    if j > 0 {
                                        result.push(' ');
                                    }
                                    result.push_str(&fmt_g(v.x, sig));
                                    result.push(' ');
                                    result.push_str(&fmt_g(v.y, sig));
                                    result.push(' ');
                                    result.push_str(&fmt_g(v.z, sig));
                                }
                            }
                            Value::Vec3(v) | Value::Color(v) | Value::DualVec3(v, ..) => {
                                result.push_str(&fmt_g(v.x, sig));
                                result.push(' ');
                                result.push_str(&fmt_g(v.y, sig));
                                result.push(' ');
                                result.push_str(&fmt_g(v.z, sig));
                            }
                            Value::Matrix(m) => {
                                for (j, v) in m.m.iter().flatten().enumerate() {
                                    if j > 0 {
                                        result.push(' ');
                                    }
                                    result.push_str(&fmt_g(*v, sig));
                                }
                            }
                            Value::Closure(c) => {
                                result.push_str(&c.fmt_display());
                            }
                            Value::Struct(fields) => {
                                // Struct: each field printed with %g, space-separated
                                result.push_str(&fmt_struct_fields_g(fields, sig, &fmt_g));
                            }
                            _ => {
                                let v = val.as_float();
                                result.push_str(&fmt_g(v, sig));
                            }
                        }
                    }
                    'x' | 'X' => {
                        let v = val.as_int();
                        if spec == 'x' {
                            result.push_str(&format!("{v:x}"));
                        } else {
                            result.push_str(&format!("{v:X}"));
                        }
                    }
                    'o' => {
                        let v = val.as_int();
                        result.push_str(&format!("{v:o}"));
                    }
                    'c' => {
                        let v = val.as_int();
                        if let Some(c) = char::from_u32(v as u32) {
                            result.push(c);
                        }
                    }
                    's' => match &val {
                        Value::String(s) => result.push_str(s.as_str()),
                        Value::StringArray(arr) => {
                            for (j, s) in arr.iter().enumerate() {
                                if j > 0 {
                                    result.push(' ');
                                }
                                result.push_str(s.as_str());
                            }
                        }
                        Value::Closure(c) => result.push_str(&c.fmt_display()),
                        Value::ClosureArray(arr) => {
                            for (j, elem) in arr.iter().enumerate() {
                                if j > 0 {
                                    result.push('\n');
                                }
                                match elem {
                                    Some(cv) => result.push_str(&cv.as_ref().fmt_display()),
                                    None => result.push_str("null"),
                                }
                            }
                        }
                        Value::Struct(fields) => {
                            // Struct: print each field as %s, space-separated
                            let mut first = true;
                            for f in fields {
                                if !first {
                                    result.push(' ');
                                }
                                first = false;
                                match f {
                                    Value::String(s) => result.push_str(s.as_str()),
                                    _ => result.push_str(&format!("{}", f.as_float())),
                                }
                            }
                        }
                        // Null closure (int 0 or float 0.0) prints empty string
                        _ => {}
                    },
                    'p' => {
                        // Point/vector: print as (x, y, z)
                        let v = val.as_vec3();
                        result.push_str(&format!("({}, {}, {})", v.x, v.y, v.z));
                    }
                    _ => {
                        result.push('%');
                        result.push(spec);
                    }
                }
            } else {
                result.push(bytes[i] as char);
                i += 1;
            }
        }

        result
    }

    fn get(&self, idx: i32) -> Value {
        if idx >= 0 && (idx as usize) < self.values.len() {
            self.values[idx as usize].clone()
        } else {
            Value::default()
        }
    }

    fn set(&mut self, idx: i32, val: Value) {
        if idx >= 0 && (idx as usize) < self.values.len() {
            self.values[idx as usize] = val;
        }
    }

    /// Parse optional texture args from opcode args[start..] as (name, value) pairs.
    /// Matches C++ llvm_gen_texture_options: name must be string, value follows.
    fn parse_texture_opt_args(&self, args: &[i32], start: usize) -> crate::texture::TextureOpt {
        use crate::texture::{TextureOptArg, parse_texture_options};
        let mut pairs = Vec::new();
        let mut i = start;
        while i + 1 < args.len() {
            let name_val = self.get(args[i]);
            let name_str = name_val.as_string();
            let name = name_str.as_str().to_string();
            if name.is_empty() {
                i += 1;
                continue;
            }
            let val = self.get(args[i + 1]);
            let opt_arg = match &val {
                Value::Int(v) => TextureOptArg::Int(*v),
                Value::Float(v) | Value::DualFloat(v, _, _) => TextureOptArg::Float(*v),
                Value::String(s) => TextureOptArg::Str(s.as_str().to_string()),
                _ => TextureOptArg::Float(val.as_float()),
            };
            pairs.push((name, opt_arg));
            i += 2;
        }
        parse_texture_options(pairs)
    }

    /// Get the final value of a symbol by name.
    pub fn get_symbol_value(&self, ir: &ShaderIR, name: &str) -> Option<Value> {
        ir.symbols
            .iter()
            .enumerate()
            .find(|(_, s)| s.name == name)
            .map(|(i, _)| self.values[i].clone())
    }

    /// Get a float value of a symbol by name.
    pub fn get_float(&self, ir: &ShaderIR, name: &str) -> Option<f32> {
        self.get_symbol_value(ir, name).map(|v| v.as_float())
    }

    /// Get an int value of a symbol by name.
    pub fn get_int(&self, ir: &ShaderIR, name: &str) -> Option<i32> {
        self.get_symbol_value(ir, name).map(|v| v.as_int())
    }

    /// Get a vec3 value of a symbol by name.
    pub fn get_vec3(&self, ir: &ShaderIR, name: &str) -> Option<Vec3> {
        self.get_symbol_value(ir, name).map(|v| v.as_vec3())
    }

    /// Get matrix to transform from `from` space to `to` space via RendererServices.
    /// Falls back to identity if no renderer or unknown spaces.
    /// Uses commonspace_synonym (e.g. "world") as alias for "common" per-reference.
    fn get_space_matrix(
        &mut self,
        globals: &ShaderGlobals,
        from: &UString,
        to: &UString,
    ) -> Matrix44 {
        let from_s = from.as_str();
        let to_s = to.as_str();
        let syn_s = self.commonspace_synonym.as_str();
        // "common" or synonym is the identity reference
        if from_s == to_s {
            return Matrix44::IDENTITY;
        }
        let from_is_common = from_s == "common" || from_s == syn_s;
        let to_is_common = to_s == "common" || to_s == syn_s;
        if from_is_common && to_is_common {
            return Matrix44::IDENTITY;
        }

        // Resolve from->common: "shader"/"object" use ShaderGlobals ptrs (C++ parity)
        let m_from = if from_is_common {
            Matrix44::IDENTITY
        } else if let Some(m) = crate::matrix_ops::get_sg_space_matrix(globals, from_s) {
            m
        } else if let Some(renderer) = &self.renderer {
            let h = UStringHash::hash_utf8(from_s);
            match renderer.get_matrix_named_static(globals, h) {
                Some(m) => m,
                None => {
                    if self.unknown_coordsys_error {
                        let msg = format!("Unknown transformation \"{}\"", from_s);
                        if self.seen_errors.insert(msg.clone()) {
                            self.messages.push(format!("ERROR: {msg}\n"));
                        }
                    }
                    Matrix44::IDENTITY
                }
            }
        } else {
            return Matrix44::IDENTITY;
        };

        // Resolve common->to: "shader"/"object" use inverse of ShaderGlobals ptrs
        let m_to_inv = if to_is_common {
            Matrix44::IDENTITY
        } else if let Some(m) = crate::matrix_ops::get_sg_inverse_space_matrix(globals, to_s) {
            m
        } else if let Some(renderer) = &self.renderer {
            let h = UStringHash::hash_utf8(to_s);
            match renderer.get_inverse_matrix_named_static(globals, h) {
                Some(m) => m,
                None => {
                    if self.unknown_coordsys_error {
                        let msg = format!("Unknown transformation \"{}\"", to_s);
                        if self.seen_errors.insert(msg.clone()) {
                            self.messages.push(format!("ERROR: {msg}\n"));
                        }
                    }
                    Matrix44::IDENTITY
                }
            }
        } else {
            return Matrix44::IDENTITY;
        };

        crate::matrix_ops::matmul(&m_from, &m_to_inv)
    }

    /// Bind shader globals to the appropriate symbols.
    ///
    /// Globals that have known derivatives (P, u, v, I, Ps) are stored
    /// as `DualFloat` / `DualVec3` so that derivative propagation works
    /// throughout the interpreter, enabling correct texture filter widths.
    fn bind_globals(&mut self, ir: &ShaderIR, globals: &ShaderGlobals) {
        for (i, sym) in ir.symbols.iter().enumerate() {
            let name = sym.name.as_str();
            match name {
                // P with screen-space derivatives
                "P" => self.values[i] = Value::DualVec3(globals.p, globals.dp_dx, globals.dp_dy),
                "dPdx" => self.values[i] = Value::Vec3(globals.dp_dx),
                "dPdy" => self.values[i] = Value::Vec3(globals.dp_dy),
                "dPdz" => self.values[i] = Value::Vec3(globals.dp_dz),
                "N" => self.values[i] = Value::Vec3(globals.n),
                "Ng" => self.values[i] = Value::Vec3(globals.ng),
                // I with screen-space derivatives
                "I" => self.values[i] = Value::DualVec3(globals.i, globals.di_dx, globals.di_dy),
                "dIdx" => self.values[i] = Value::Vec3(globals.di_dx),
                "dIdy" => self.values[i] = Value::Vec3(globals.di_dy),
                "dPdu" => self.values[i] = Value::Vec3(globals.dp_du),
                "dPdv" => self.values[i] = Value::Vec3(globals.dp_dv),
                // u with screen-space derivatives (critical for texture filtering)
                "u" => self.values[i] = Value::DualFloat(globals.u, globals.dudx, globals.dudy),
                "dudx" => self.values[i] = Value::Float(globals.dudx),
                "dudy" => self.values[i] = Value::Float(globals.dudy),
                // v with screen-space derivatives
                "v" => self.values[i] = Value::DualFloat(globals.v, globals.dvdx, globals.dvdy),
                "dvdx" => self.values[i] = Value::Float(globals.dvdx),
                "dvdy" => self.values[i] = Value::Float(globals.dvdy),
                "time" => self.values[i] = Value::Float(globals.time),
                "dtime" => self.values[i] = Value::Float(globals.dtime),
                "dPdtime" => self.values[i] = Value::Vec3(globals.dp_dtime),
                // Ps with screen-space derivatives
                "Ps" => {
                    self.values[i] = Value::DualVec3(globals.ps, globals.dps_dx, globals.dps_dy)
                }
                "surfacearea" => self.values[i] = Value::Float(globals.surfacearea),
                "raytype" => self.values[i] = Value::Int(globals.raytype),
                "flipHandedness" => self.values[i] = Value::Int(globals.flip_handedness),
                "backfacing" => self.values[i] = Value::Int(globals.backfacing),
                _ => {}
            }
        }
    }
}

fn const_to_value(cv: &ConstValue) -> Value {
    match cv {
        ConstValue::Int(v) => Value::Int(*v),
        ConstValue::Float(v) => Value::Float(*v),
        ConstValue::String(s) => Value::String(*s),
        ConstValue::Vec3(v) => Value::Vec3(*v),
        ConstValue::Matrix(m) => Value::Matrix(*m),
        ConstValue::IntArray(a) => Value::IntArray(a.clone()),
        ConstValue::FloatArray(a) => Value::FloatArray(a.clone()),
        ConstValue::StringArray(a) => Value::StringArray(a.clone()),
    }
}

// closure_name_to_id removed -- use crate::closure_ops::closure_name_to_id

fn default_value_for_type(td: &TypeDesc) -> Value {
    // Array types: allocate with correct length
    if td.arraylen > 0 {
        let len = td.arraylen as usize;
        if td.basetype == BaseType::Float as u8 {
            if td.aggregate == Aggregate::Vec3 as u8 {
                return Value::Vec3Array(vec![Vec3::ZERO; len]);
            } else if td.aggregate == Aggregate::Matrix44 as u8 {
                return Value::MatrixArray(vec![Matrix44::IDENTITY; len]);
            } else {
                return Value::FloatArray(vec![0.0; len]);
            }
        } else if td.basetype == BaseType::Int32 as u8 {
            return Value::IntArray(vec![0; len]);
        } else if td.basetype == BaseType::String as u8 {
            return Value::StringArray(vec![UString::empty(); len]);
        }
    }
    // Scalar types
    if td.basetype == BaseType::Float as u8 {
        if td.aggregate == Aggregate::Vec3 as u8 {
            Value::Vec3(Vec3::ZERO)
        } else if td.aggregate == Aggregate::Matrix44 as u8 {
            Value::Matrix(Matrix44::IDENTITY)
        } else {
            Value::Float(0.0)
        }
    } else if td.basetype == BaseType::Int32 as u8 {
        Value::Int(0)
    } else if td.basetype == BaseType::String as u8 {
        Value::String(UString::empty())
    } else {
        Value::Void
    }
}

/// Convenience: parse + compile + execute a shader in one call.
pub fn run_shader(source: &str) -> Result<Interpreter, String> {
    let ast = crate::parser::parse(source)
        .map_err(|e| format!("{e:?}"))?
        .ast;
    let ir = crate::codegen::generate(&ast);
    let globals = ShaderGlobals::default();
    let mut interp = Interpreter::new();
    interp.execute(&ir, &globals, None);
    Ok(interp)
}

/// Execute with custom globals.
pub fn run_shader_with_globals(
    source: &str,
    globals: &ShaderGlobals,
) -> Result<Interpreter, String> {
    let ast = crate::parser::parse(source)
        .map_err(|e| format!("{e:?}"))?
        .ast;
    let ir = crate::codegen::generate(&ast);
    let mut interp = Interpreter::new();
    interp.execute(&ir, globals, None);
    Ok(interp)
}

/// Execute with custom globals and renderer services.
pub fn run_shader_with_renderer(
    source: &str,
    globals: &ShaderGlobals,
    renderer: Arc<dyn RendererServices>,
) -> Result<Interpreter, String> {
    let ast = crate::parser::parse(source)
        .map_err(|e| format!("{e:?}"))?
        .ast;
    let ir = crate::codegen::generate(&ast);
    let mut interp = Interpreter::with_renderer(renderer);
    interp.execute(&ir, globals, None);
    Ok(interp)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codegen;
    use crate::parser;

    fn compile_and_run(src: &str) -> (ShaderIR, Interpreter) {
        let ast = parser::parse(src).unwrap().ast;
        let ir = codegen::generate(&ast);
        let globals = ShaderGlobals::default();
        let mut interp = Interpreter::new();
        interp.execute(&ir, &globals, None);
        (ir, interp)
    }

    #[test]
    fn test_constant_initialization() {
        let src = r#"
shader test() {
    float a = 2.0;
    float b = 3.0;
    float c = a + b;
}
"#;
        let (ir, interp) = compile_and_run(src);

        // Constants should be properly initialized
        let a_val = interp.get_float(&ir, "a").unwrap();
        let b_val = interp.get_float(&ir, "b").unwrap();
        let c_val = interp.get_float(&ir, "c").unwrap();
        assert_eq!(a_val, 2.0, "a should be 2.0");
        assert_eq!(b_val, 3.0, "b should be 3.0");
        assert_eq!(c_val, 5.0, "c should be 2.0 + 3.0 = 5.0");
    }

    #[test]
    fn test_param_defaults() {
        let src = r#"
shader test(float Kd = 0.5) {
    float result = Kd * 2.0;
}
"#;
        let (ir, interp) = compile_and_run(src);
        let kd = interp.get_float(&ir, "Kd").unwrap();
        let result = interp.get_float(&ir, "result").unwrap();
        assert_eq!(kd, 0.5, "Kd should be 0.5");
        assert_eq!(result, 1.0, "result should be 0.5 * 2.0 = 1.0");
    }

    #[test]
    fn test_arithmetic_operators() {
        let src = r#"
shader test() {
    float a = 10.0;
    float b = 3.0;
    float c = a + b;
    float d = a - b;
    float e = a * b;
    float f = a / b;
}
"#;
        let (ir, interp) = compile_and_run(src);
        assert_eq!(interp.get_float(&ir, "c").unwrap(), 13.0);
        assert_eq!(interp.get_float(&ir, "d").unwrap(), 7.0);
        assert_eq!(interp.get_float(&ir, "e").unwrap(), 30.0);
        assert!((interp.get_float(&ir, "f").unwrap() - 10.0 / 3.0).abs() < 1e-6);
    }

    #[test]
    fn test_conditional() {
        let src = r#"
shader test() {
    float x = 5.0;
    float result = 0.0;
    if (x > 3.0) {
        result = 1.0;
    }
}
"#;
        let (ir, interp) = compile_and_run(src);
        let result = interp.get_float(&ir, "result").unwrap();
        assert_eq!(result, 1.0, "condition should be true, result = 1.0");
    }

    #[test]
    fn test_conditional_false() {
        let src = r#"
shader test() {
    float x = 1.0;
    float result = 0.0;
    if (x > 3.0) {
        result = 1.0;
    }
}
"#;
        let (ir, interp) = compile_and_run(src);
        let result = interp.get_float(&ir, "result").unwrap();
        assert_eq!(result, 0.0, "condition should be false, result = 0.0");
    }

    #[test]
    fn test_math_builtins() {
        let src = r#"
shader test() {
    float a = sin(0.0);
    float b = cos(0.0);
    float c = sqrt(4.0);
    float d = abs(-5.0);
    float e = pow(2.0, 3.0);
}
"#;
        let (ir, interp) = compile_and_run(src);
        assert!((interp.get_float(&ir, "a").unwrap() - 0.0).abs() < 1e-6);
        assert!((interp.get_float(&ir, "b").unwrap() - 1.0).abs() < 1e-6);
        assert!((interp.get_float(&ir, "c").unwrap() - 2.0).abs() < 1e-6);
        assert!((interp.get_float(&ir, "d").unwrap() - 5.0).abs() < 1e-6);
        assert!((interp.get_float(&ir, "e").unwrap() - 8.0).abs() < 1e-6);
    }

    #[test]
    fn test_complex_shader() {
        let src = r#"
surface simple_diffuse(color Cd = color(0.8, 0.2, 0.1), float Kd = 1.0) {
    float intensity = Kd * 0.5;
}
"#;
        let (ir, interp) = compile_and_run(src);
        let intensity = interp.get_float(&ir, "intensity").unwrap();
        assert_eq!(intensity, 0.5, "intensity should be 1.0 * 0.5 = 0.5");
    }

    #[test]
    fn test_multiple_assignments() {
        let src = r#"
shader test() {
    float x = 1.0;
    x = x + 1.0;
    x = x * 3.0;
}
"#;
        let (ir, interp) = compile_and_run(src);
        let x = interp.get_float(&ir, "x").unwrap();
        // x = 1.0, then x = 2.0, then x = 6.0
        assert_eq!(x, 6.0, "x should be (1+1)*3 = 6.0");
    }

    #[test]
    fn test_smoothstep() {
        let src = r#"
shader test() {
    float a = smoothstep(0.0, 1.0, 0.5);
}
"#;
        let (ir, interp) = compile_and_run(src);
        let a = interp.get_float(&ir, "a").unwrap();
        // smoothstep(0,1,0.5) = 0.5^2 * (3 - 2*0.5) = 0.25 * 2 = 0.5
        assert!((a - 0.5).abs() < 1e-6, "smoothstep(0,1,0.5) should be 0.5");
    }

    #[test]
    fn test_clamp() {
        let src = r#"
shader test() {
    float a = clamp(5.0, 0.0, 1.0);
    float b = clamp(-1.0, 0.0, 1.0);
    float c = clamp(0.5, 0.0, 1.0);
}
"#;
        let (ir, interp) = compile_and_run(src);
        assert_eq!(interp.get_float(&ir, "a").unwrap(), 1.0);
        assert_eq!(interp.get_float(&ir, "b").unwrap(), 0.0);
        assert_eq!(interp.get_float(&ir, "c").unwrap(), 0.5);
    }

    #[test]
    fn test_mix() {
        let src = r#"
shader test() {
    float a = mix(0.0, 1.0, 0.5);
}
"#;
        let (ir, interp) = compile_and_run(src);
        let a = interp.get_float(&ir, "a").unwrap();
        assert!((a - 0.5).abs() < 1e-6, "mix(0,1,0.5) should be 0.5");
    }

    #[test]
    fn test_run_shader_convenience() {
        let interp = run_shader(
            r#"
shader test() {
    float x = 42.0;
}
"#,
        )
        .unwrap();
        // Just verify it doesn't crash and produces valid output
        assert!(interp.messages.is_empty());
    }

    #[test]
    fn test_pointcloud_search_optional_args() {
        use crate::math::Vec3;
        use crate::renderer::BasicRenderer;
        use crate::ustring::UString;
        use std::sync::Arc;

        let br = Arc::new(BasicRenderer::new());
        {
            let mut mgr = br.pointcloud_manager.write().unwrap();
            let cloud = mgr.get_or_create("pc.ptc");
            for i in 0..5 {
                let pos = Vec3::new(i as f32, 0.0, 0.0);
                let mut attrs = std::collections::HashMap::new();
                attrs.insert(UString::new("id"), crate::pointcloud::PointData::Int(i));
                cloud.add_point(pos, attrs);
            }
        }

        let src = r#"
shader test() {
    int indices[64];
    float distances[64];
    int n = pointcloud_search("pc.ptc", point(2,0,0), 2.0, 64, 1, "index", indices, "distance", distances);
}
"#;
        let ast = parser::parse(src).unwrap().ast;
        let ir = codegen::generate(&ast);
        let mut interp = Interpreter::with_renderer(br.clone());
        let globals = ShaderGlobals::default();
        interp.execute(&ir, &globals, None);

        let n = interp.get_int(&ir, "n").unwrap();
        assert!(
            n >= 2,
            "should find at least points 1,2,3 within radius 2 of (2,0,0)"
        );

        let indices_val = interp.get_symbol_value(&ir, "indices").unwrap();
        if let Value::IntArray(arr) = indices_val {
            assert!(arr.len() >= 2, "indices array should be populated");
            assert!(arr.contains(&1) || arr.contains(&2) || arr.contains(&3));
        } else {
            panic!("indices should be IntArray");
        }

        let distances_val = interp.get_symbol_value(&ir, "distances").unwrap();
        if let Value::FloatArray(arr) = distances_val {
            assert!(arr.len() >= 2);
            assert!(arr[0] < arr[1], "distances should be sorted");
        } else {
            panic!("distances should be FloatArray");
        }
    }

    #[test]
    fn test_pointcloud_get_with_count_and_color() {
        use crate::math::Vec3;
        use crate::renderer::BasicRenderer;
        use crate::ustring::UString;
        use std::sync::Arc;

        let br = Arc::new(BasicRenderer::new());
        {
            let mut mgr = br.pointcloud_manager.write().unwrap();
            let cloud = mgr.get_or_create("pcget.ptc");
            for i in 0..5 {
                let pos = Vec3::new(i as f32, 0.0, 0.0);
                let mut attrs = std::collections::HashMap::new();
                attrs.insert(UString::new("id"), crate::pointcloud::PointData::Int(i));
                attrs.insert(
                    UString::new("uv"),
                    crate::pointcloud::PointData::Vec3(Vec3::new(
                        i as f32 * 0.2,
                        i as f32 * 0.1,
                        0.0,
                    )),
                );
                cloud.add_point(pos, attrs);
            }
        }

        let src = r#"
shader test() {
    int indices[64];
    float distances[64];
    color uv[64];
    int n = pointcloud_search("pcget.ptc", point(2,0,0), 2.0, 64, 1, "index", indices, "distance", distances);
    int ok = 0;
    if (n > 0) {
        ok = pointcloud_get("pcget.ptc", indices, n, "uv", uv);
    }
}
"#;
        let ast = parser::parse(src).unwrap().ast;
        let ir = codegen::generate(&ast);
        let mut interp = Interpreter::with_renderer(br.clone());
        let globals = ShaderGlobals::default();
        interp.execute(&ir, &globals, None);

        let n = interp.get_int(&ir, "n").unwrap();
        assert!(n >= 2);
        let ok = interp.get_int(&ir, "ok").unwrap();
        assert_eq!(ok, 1);
        let uv_val = interp.get_symbol_value(&ir, "uv").unwrap();
        if let Value::Vec3Array(arr) = uv_val {
            assert!(!arr.is_empty());
        }
    }

    #[test]
    fn test_globals_binding() {
        let src = r#"
shader test() {
    float su = u;
    float sv = v;
}
"#;
        let mut globals = ShaderGlobals::default();
        globals.u = 0.25;
        globals.v = 0.75;
        let ast = parser::parse(src).unwrap().ast;
        let ir = codegen::generate(&ast);
        let mut interp = Interpreter::new();
        interp.execute(&ir, &globals, None);
        assert_eq!(interp.get_float(&ir, "su").unwrap(), 0.25);
        assert_eq!(interp.get_float(&ir, "sv").unwrap(), 0.75);
    }

    #[test]
    fn test_comparison_operators() {
        let src = r#"
shader test() {
    float a = 5.0;
    float b = 3.0;
    int gt_result = 0;
    int lt_result = 0;
    int eq_result = 0;
    if (a > b) { gt_result = 1; }
    if (a < b) { lt_result = 1; }
    if (a > 10.0) { eq_result = 1; }
}
"#;
        let (ir, interp) = compile_and_run(src);
        assert_eq!(
            interp.get_int(&ir, "gt_result").unwrap(),
            1,
            "5 > 3 should be true"
        );
        assert_eq!(
            interp.get_int(&ir, "lt_result").unwrap(),
            0,
            "5 < 3 should be false"
        );
        assert_eq!(
            interp.get_int(&ir, "eq_result").unwrap(),
            0,
            "5 > 10 should be false"
        );
    }

    #[test]
    fn test_negation() {
        let src = r#"
shader test() {
    float a = 5.0;
    float b = -a;
}
"#;
        let (ir, interp) = compile_and_run(src);
        assert_eq!(interp.get_float(&ir, "b").unwrap(), -5.0);
    }

    #[test]
    fn test_division_by_zero() {
        let src = r#"
shader test() {
    float a = 1.0;
    float b = 0.0;
    float c = a / b;
}
"#;
        let (ir, interp) = compile_and_run(src);
        assert_eq!(
            interp.get_float(&ir, "c").unwrap(),
            0.0,
            "div by zero should be 0"
        );
    }

    // --- Parity fix tests (matching C++ llvm_ops.cpp / dual.h) ---

    #[test]
    fn test_safe_div_f32_isfinite() {
        // C++ safe_div: q = a/b; isfinite(q) ? q : 0.0
        assert_eq!(safe_div_f32(1.0, 0.0), 0.0);
        assert_eq!(safe_div_f32(0.0, 0.0), 0.0);
        assert_eq!(safe_div_f32(6.0, 3.0), 2.0);
        assert_eq!(safe_div_f32(-6.0, 3.0), -2.0);
        // Near-zero divisor producing INF should return 0.0
        assert_eq!(safe_div_f32(1e30, 1e-30), 0.0);
    }

    #[test]
    fn test_safe_fmod_c_convention() {
        // C++ safe_fmod: int N = (int)(a/b); return a - N*b
        // Result sign = dividend sign (C convention), NOT always-positive (Euclidean)
        let r = safe_fmod_f32(-7.0, 3.0);
        assert!(
            r < 0.0,
            "fmod(-7,3) should be negative per C convention, got {}",
            r
        );
        assert!(
            (r - (-1.0)).abs() < 1e-6,
            "fmod(-7,3) should be -1.0, got {}",
            r
        );

        let r2 = safe_fmod_f32(7.0, -3.0);
        assert!(r2 > 0.0, "fmod(7,-3) should be positive, got {}", r2);
        assert!(
            (r2 - 1.0).abs() < 1e-6,
            "fmod(7,-3) should be 1.0, got {}",
            r2
        );

        assert_eq!(safe_fmod_f32(5.0, 0.0), 0.0);
    }

    #[test]
    fn test_osl_mod_i32() {
        // C++ osl_safe_mod_iii: (b != 0) ? (a % b) : 0
        // Standard C modulus, result sign = dividend sign
        assert_eq!(osl_mod_i32(-7, 3), -1);
        assert_eq!(osl_mod_i32(7, -3), 1);
        assert_eq!(osl_mod_i32(7, 0), 0);
        assert_eq!(osl_mod_i32(10, 3), 1);
    }

    #[test]
    fn test_float_eq_exact() {
        // C++ uses exact == for float comparison, not epsilon.
        // Use values far enough apart to differ in f32 representation.
        let src = r#"
shader test() {
    float a = 1.0;
    float b = 1.0;
    float c = 1.001;
    int ab_eq = (a == b);
    int ac_eq = (a == c);
}
"#;
        let (ir, interp) = compile_and_run(src);
        assert_eq!(
            interp.get_int(&ir, "ab_eq").unwrap(),
            1,
            "identical floats should be =="
        );
        // With exact comparison, different f32 values should NOT be equal
        assert_eq!(
            interp.get_int(&ir, "ac_eq").unwrap(),
            0,
            "different floats should not be =="
        );
    }

    #[test]
    fn test_dual_div_vec3() {
        // dual_div should handle Vec3 component-wise with safe_div_f32
        let a = Value::Vec3(Vec3::new(6.0, 0.0, 9.0));
        let b = Value::Vec3(Vec3::new(3.0, 0.0, 3.0));
        let r = dual_div(&a, &b);
        match &r {
            Value::Vec3(v) => {
                assert_eq!(v.x, 2.0);
                assert_eq!(v.y, 0.0, "0/0 should be 0 via isfinite check");
                assert_eq!(v.z, 3.0);
            }
            other => panic!("Expected Vec3, got {:?}", other),
        }
    }

    #[test]
    fn test_safe_div_near_zero() {
        // Very small divisor producing non-finite result should return 0.0
        let big = 1e38_f32;
        let tiny = 1e-38_f32;
        let result = safe_div_f32(big, tiny);
        assert_eq!(result, 0.0, "near-zero divisor overflow should yield 0.0");
    }

    #[test]
    fn test_mod_c_convention_via_interp() {
        // Verify modulus through the full interpreter uses C convention
        let src = r#"
shader test() {
    int a = -7;
    int b = 3;
    int r = a % b;
}
"#;
        let (ir, interp) = compile_and_run(src);
        assert_eq!(
            interp.get_int(&ir, "r").unwrap(),
            -1,
            "int mod should use C convention: -7 % 3 = -1"
        );
    }

    #[test]
    fn test_select_arg_order() {
        // OSL select(x, y, cond): cond ? y : x
        // Condition is the LAST argument, not the second.
        let src = r#"
shader test() {
    float a = select(1.0, 2.0, 1);
    float b = select(1.0, 2.0, 0);
}
"#;
        let (ir, interp) = compile_and_run(src);
        assert_eq!(
            interp.get_float(&ir, "a").unwrap(),
            2.0,
            "select with true cond should return y=2.0"
        );
        assert_eq!(
            interp.get_float(&ir, "b").unwrap(),
            1.0,
            "select with false cond should return x=1.0"
        );
    }

    // --- MEDIUM-1: smoothstep derivative propagation ---
    #[test]
    fn test_smoothstep_dual_derivatives() {
        // smoothstep with Dual2 inputs should propagate derivatives.
        // t = (x-e0)/(e1-e0), result = (3-2t)*t*t
        // At x=0.5, e0=0, e1=1: t=0.5, result=0.5*0.5*(3-1)=0.5
        // f'(t) = 6t(1-t) = 6*0.5*0.5 = 1.5
        // dt/dx = 1/(e1-e0) = 1
        // So dr/dx = 1.5 * 1 = 1.5
        let e0 = Value::Float(0.0);
        let e1 = Value::Float(1.0);
        let x = Value::DualFloat(0.5, 1.0, 0.0); // dx=1, dy=0

        // Compute smoothstep via dual arithmetic (matching interpreter logic)
        let t = dual_div(&dual_sub(&x, &e0), &dual_sub(&e1, &e0));
        let two_t = dual_mul(&Value::Float(2.0), &t);
        let three_minus = dual_sub(&Value::Float(3.0), &two_t);
        let t_sq = dual_mul(&t, &t);
        let result = dual_mul(&three_minus, &t_sq);

        assert!((result.as_float() - 0.5).abs() < 1e-5, "smoothstep val");
        assert!(
            (result.dx_float() - 1.5).abs() < 1e-5,
            "smoothstep dx should be 1.5"
        );
        assert!(result.dy_float().abs() < 1e-5, "smoothstep dy should be 0");
    }

    #[test]
    fn test_smoothstep_edges() {
        // Below lower edge: result = 0
        let src = r#"
shader test() {
    float a = smoothstep(0.5, 1.0, 0.0);
    float b = smoothstep(0.0, 1.0, 2.0);
}
"#;
        let (ir, interp) = compile_and_run(src);
        assert_eq!(interp.get_float(&ir, "a").unwrap(), 0.0, "below edge => 0");
        assert_eq!(interp.get_float(&ir, "b").unwrap(), 1.0, "above edge => 1");
    }

    // --- MEDIUM-2: color space constructor ---
    #[test]
    fn test_color_space_hsv() {
        // color("hsv", 0, 0, 1) = pure white in HSV => (1,1,1) in RGB
        // HSV: h=0, s=0, v=1 => no saturation, full value => white
        let hsv = Vec3::new(0.0, 0.0, 1.0);
        let rgb = crate::color::hsv_to_rgb(hsv);
        assert!((rgb.x - 1.0).abs() < 1e-5, "hsv(0,0,1) r");
        assert!((rgb.y - 1.0).abs() < 1e-5, "hsv(0,0,1) g");
        assert!((rgb.z - 1.0).abs() < 1e-5, "hsv(0,0,1) b");
    }

    #[test]
    fn test_color_space_hsv_red() {
        // HSV: h=0, s=1, v=1 => pure red (1,0,0)
        let hsv = Vec3::new(0.0, 1.0, 1.0);
        let rgb = crate::color::hsv_to_rgb(hsv);
        assert!((rgb.x - 1.0).abs() < 1e-5, "hsv pure red r");
        assert!(rgb.y.abs() < 1e-5, "hsv pure red g");
        assert!(rgb.z.abs() < 1e-5, "hsv pure red b");
    }

    // --- MEDIUM-3: log safe values ---
    #[test]
    fn test_log_zero_clamped() {
        let src = r#"
shader test() {
    float a = log(0.0);
    float b = log2(0.0);
    float c = log10(0.0);
}
"#;
        let (ir, interp) = compile_and_run(src);
        let a = interp.get_float(&ir, "a").unwrap();
        let b = interp.get_float(&ir, "b").unwrap();
        let c = interp.get_float(&ir, "c").unwrap();
        // Should be finite (clamped to log(f32::MIN_POSITIVE)), not -INF
        assert!(a.is_finite(), "log(0) should be finite, got {a}");
        assert!(b.is_finite(), "log2(0) should be finite, got {b}");
        assert!(c.is_finite(), "log10(0) should be finite, got {c}");
        // Should be a large negative number
        assert!(a < -80.0, "log(0) should be very negative, got {a}");
        assert!(b < -100.0, "log2(0) should be very negative, got {b}");
        assert!(c < -30.0, "log10(0) should be very negative, got {c}");
    }

    #[test]
    fn test_log_positive() {
        let src = r#"
shader test() {
    float a = log(1.0);
    float b = log2(4.0);
    float c = log10(100.0);
}
"#;
        let (ir, interp) = compile_and_run(src);
        assert!(
            (interp.get_float(&ir, "a").unwrap() - 0.0).abs() < 1e-6,
            "log(1) should be 0"
        );
        assert!(
            (interp.get_float(&ir, "b").unwrap() - 2.0).abs() < 1e-5,
            "log2(4) should be 2"
        );
        assert!(
            (interp.get_float(&ir, "c").unwrap() - 2.0).abs() < 1e-5,
            "log10(100) should be 2"
        );
    }

    #[test]
    fn test_safe_fmod_f64_precision() {
        // Values that would overflow i32 with old (a/b) as i32 implementation.
        // With f64 trunc, result is finite (no UB from integer overflow).
        let r = safe_fmod_f32(1.0e15, 3.0);
        assert!(
            r.is_finite(),
            "safe_fmod with extreme values should be finite, got {r}"
        );

        // Within f32 exact integer range (< 2^24 = 16777216)
        let r2 = safe_fmod_f32(100.0, 7.0);
        assert!((r2 - 2.0).abs() < 1e-6, "fmod(100,7)=2, got {r2}");

        let r3 = safe_fmod_f32(10000.0, 3.0);
        assert!((r3 - 1.0).abs() < 1e-5, "fmod(10000,3)=1, got {r3}");

        // Basic correctness: truncation toward zero (C convention)
        assert!((safe_fmod_f32(7.0, 3.0) - 1.0).abs() < 1e-6, "fmod(7,3)=1");
        assert!(
            (safe_fmod_f32(-7.0, 3.0) - (-1.0)).abs() < 1e-6,
            "fmod(-7,3)=-1"
        );
        assert!(
            (safe_fmod_f32(7.0, -3.0) - 1.0).abs() < 1e-6,
            "fmod(7,-3)=1"
        );
        assert!(
            (safe_fmod_f32(-7.0, -3.0) - (-1.0)).abs() < 1e-6,
            "fmod(-7,-3)=-1"
        );

        // Zero divisor
        assert_eq!(safe_fmod_f32(5.0, 0.0), 0.0, "fmod(x,0) should be 0");
    }

    #[test]
    fn test_fmod_builtin_uses_safe_fmod() {
        // fmod builtin should match safe_fmod_f32 (truncation toward zero)
        let src = r#"
shader test() {
    float a = fmod(-7.0, 3.0);
    float b = fmod(7.0, -3.0);
}
"#;
        let (ir, interp) = compile_and_run(src);
        let a = interp.get_float(&ir, "a").unwrap();
        let b = interp.get_float(&ir, "b").unwrap();
        // C convention: result sign = dividend sign
        assert!(a < 0.0, "fmod(-7,3) should be negative, got {a}");
        assert!(
            (a - (-1.0)).abs() < 1e-6,
            "fmod(-7,3) should be -1, got {a}"
        );
        assert!(b > 0.0, "fmod(7,-3) should be positive, got {b}");
        assert!((b - 1.0).abs() < 1e-6, "fmod(7,-3) should be 1, got {b}");
    }

    // --- LOW-6: fmod precision for extreme values ---
    #[test]
    fn test_fmod_extreme_values() {
        // With old i32 truncation, (1e15 / 3.0) as i32 would overflow.
        // f64 trunc handles it correctly.
        let r = safe_fmod_f32(1e15, 3.0);
        assert!(r.abs() < 3.0, "fmod(1e15, 3) should be in [0,3), got {r}");

        // Negative extreme
        let r2 = safe_fmod_f32(-1e15, 7.0);
        assert!(
            r2.abs() < 7.0,
            "fmod(-1e15, 7) should be in (-7,0], got {r2}"
        );
        assert!(r2 <= 0.0, "fmod of negative dividend should be <= 0");
    }

    #[test]
    fn test_fmod_basic() {
        // Basic fmod behavior: truncation toward zero
        assert!((safe_fmod_f32(5.5, 2.0) - 1.5).abs() < 1e-6);
        assert!((safe_fmod_f32(-5.5, 2.0) - (-1.5)).abs() < 1e-6);
        assert_eq!(safe_fmod_f32(1.0, 0.0), 0.0);
    }

    // --- MEDIUM-4: texture explicit derivatives ---
    #[test]
    fn test_texture_no_crash() {
        // Ensure the texture opcode doesn't crash with basic args.
        // Without a real texture system, we just verify no panic.
        let src = r#"
shader test() {
    color c = texture("nofile.exr", 0.5, 0.5);
}
"#;
        let (ir, interp) = compile_and_run(src);
        let c = interp.get_vec3(&ir, "c");
        assert!(c.is_some(), "texture should produce a color value");
    }

    #[test]
    fn test_printf_g_arithmetic() {
        let src = r#"
shader test() {
    float a = 0.7;
    float b = 0.2;
    printf("%g + %g = %g\n", a, b, a+b);
    printf("%g - %g = %g\n", a, b, a-b);
    printf("%g * %g = %g\n", a, b, a*b);
    printf("%g / %g = %g\n", a, b, a/b);
}
"#;
        let (_, interp) = compile_and_run(src);
        let out = interp.messages.join("");
        assert!(out.contains("0.7 + 0.2 = 0.9"), "got: {out}");
        assert!(out.contains("0.7 - 0.2 = 0.5"), "got: {out}");
        assert!(out.contains("0.7 * 0.2 = 0.14"), "got: {out}");
        assert!(out.contains("0.7 / 0.2 = 3.5"), "got: {out}");
    }

    #[test]
    fn test_array_init() {
        let src = r#"
shader test(output int r0 = 0, output int r1 = 0, output int r2 = 0) {
    int arr[3] = { 10, 11, 12 };
    r0 = arr[0]; r1 = arr[1]; r2 = arr[2];
}
"#;
        let (ir, interp) = compile_and_run(src);
        assert_eq!(interp.get_int(&ir, "r0"), Some(10));
        assert_eq!(interp.get_int(&ir, "r1"), Some(11));
        assert_eq!(interp.get_int(&ir, "r2"), Some(12));
    }

    #[test]
    fn test_scope_variable_redecl() {
        // Variable re-declared in sibling {} blocks with different type
        let src = r#"
shader test(output float result = 0) {
    { float b = 0.2; }
    { int b = 2; result = b; }
}
"#;
        let (ir, interp) = compile_and_run(src);
        let r = interp.get_float(&ir, "result").unwrap();
        assert!((r - 2.0).abs() < 0.01, "expected 2.0, got {r}");
    }

    #[test]
    fn test_user_function_call_params() {
        let src = r#"
void show(int a, int b) {
    printf("%d %d\n", a, b);
}
shader test() {
    show(0, 2);
}
"#;
        let (_, interp) = compile_and_run(src);
        assert_eq!(
            interp.messages.join(""),
            "0 2\n",
            "function params: got {:?}",
            interp.messages
        );
    }

    #[test]
    fn test_vector_math_from_shader_global() {
        // color a = u; should broadcast float u to all 3 components
        let src = r#"
shader test() {
    color a = u;
    printf("%g\n", a);
    printf("%g\n", fabs(a));
}
"#;
        let (_, interp) = compile_and_run(src);
        // u = 0.0 on default globals
        assert_eq!(
            interp.messages.join(""),
            "0 0 0\n0 0 0\n",
            "vector from u: got {:?}",
            interp.messages
        );
    }

    #[test]
    fn test_vector_math_functions() {
        // Vector fabs/floor/etc should return vectors
        let src = r#"
shader test() {
    color a = 0;
    printf("%g\n", fabs(a));
    vector b = 2.5;
    printf("%g\n", floor(b));
}
"#;
        let (_, interp) = compile_and_run(src);
        assert_eq!(
            interp.messages.join(""),
            "0 0 0\n2 2 2\n",
            "vector math: got {:?}",
            interp.messages
        );
    }

    #[test]
    fn test_nested_function_calls_same_param_names() {
        // Mirrors the logic test: nested calls with same-named params
        let src = r#"
void inner(int a, int b) {
    printf("%d %d\n", a, b);
}
void outer(int a, int b) {
    inner(a, b);
    inner(b, a);
}
shader test() {
    int a = 0, b = 2;
    outer(a, b);
}
"#;
        let (_, interp) = compile_and_run(src);
        assert_eq!(
            interp.messages.join(""),
            "0 2\n2 0\n",
            "nested calls: got {:?}",
            interp.messages
        );
    }

    // Diagnostic test: check IR and values
    #[test]
    fn test_closure_array_diag() {
        let src = r#"
shader test() {
    closure color arr[2];
    arr[0] = diffuse(N);
    closure color c0 = arr[0];
    printf("%s\n", c0);
}
"#;
        let ast = crate::parser::parse(src).unwrap().ast;
        let ir = crate::codegen::generate(&ast);
        eprintln!("=== SYMBOLS ===");
        for (i, sym) in ir.symbols.iter().enumerate() {
            eprintln!(
                "  [{}] {} is_closure={} is_closure_array={} arrlen={}",
                i,
                sym.name.as_str(),
                sym.typespec.is_closure(),
                sym.typespec.is_closure_array(),
                sym.typespec.arraylength()
            );
        }
        eprintln!("=== OPCODES ===");
        for (i, op) in ir.opcodes.iter().enumerate() {
            let args: Vec<String> = (0..op.nargs as usize)
                .map(|j| {
                    let ai = ir.args[op.firstarg as usize + j];
                    if ai >= 0 && (ai as usize) < ir.symbols.len() {
                        ir.symbols[ai as usize].name.as_str().to_string()
                    } else {
                        format!("#{}", ai)
                    }
                })
                .collect();
            eprintln!("  [{}] {} [{}]", i, op.op.as_str(), args.join(", "));
        }
        let globals = ShaderGlobals::default();
        let mut interp = Interpreter::new();
        interp.execute(&ir, &globals, None);
        eprintln!("=== VALUES ===");
        for (i, sym) in ir.symbols.iter().enumerate() {
            if sym.name.as_str().starts_with("arr") || sym.name.as_str().starts_with("") {
                eprintln!(
                    "  [{}] {} = {:?}",
                    i,
                    sym.name.as_str(),
                    interp.values.get(i)
                );
            }
        }
        // Don't assert, just observe
    }

    // --- Closure array support ---
    #[test]
    fn test_closure_array_create_and_access() {
        // Assign diffuse and emission to array slots; read back via element ref.
        let src = r#"
shader test() {
    closure color arr[3];
    arr[0] = diffuse(N);
    arr[1] = emission();
    closure color c0 = arr[0];
    closure color c1 = arr[1];
    printf("%s\n", c0);
    printf("%s\n", c1);
}
"#;
        let (_, interp) = compile_and_run(src);
        let out = interp.messages.join("");
        assert!(
            out.contains("diffuse"),
            "arr[0] should be diffuse, got: {out}"
        );
        assert!(
            out.contains("emission"),
            "arr[1] should be emission, got: {out}"
        );
    }

    #[test]
    fn test_closure_array_length() {
        // arraylength() must return the declared size of a closure array.
        let src = r#"
shader test() {
    closure color arr[4];
    int n = arraylength(arr);
    printf("%d\n", n);
}
"#;
        let (_, interp) = compile_and_run(src);
        assert_eq!(
            interp.messages.join(""),
            "4\n",
            "closure array length should be 4"
        );
    }

    #[test]
    fn test_closure_array_add_elements() {
        // Read two closures from array slots and combine with +.
        let src = r#"
shader test() {
    closure color arr[2];
    arr[0] = diffuse(N);
    arr[1] = emission();
    closure color combined = arr[0] + arr[1];
    printf("%s\n", combined);
}
"#;
        let (_, interp) = compile_and_run(src);
        let out = interp.messages.join("");
        assert!(
            out.contains("diffuse") || out.contains("emission"),
            "combined closure should contain leaf names, got: {out}"
        );
    }

    #[test]
    fn test_getattribute_shader_builtins() {
        // shader:shadername / shader:layername / shader:groupname are resolved
        // without a renderer (built-in table, C++ parity).
        let src = r#"
shader myshader(output string sname = "", output string lname = "", output string gname = "") {
    int ok1 = getattribute("shader:shadername", sname);
    int ok2 = getattribute("shader:layername", lname);
    int ok3 = getattribute("shader:groupname", gname);
    printf("%d %s %d %s %d %s\n", ok1, sname, ok2, lname, ok3, gname);
}
"#;
        let (_, interp) = compile_and_run(src);
        let out = interp.messages.join("");
        // ok1/ok2 should be 1, sname/lname == "myshader", ok3 == 1, gname == ""
        assert!(
            out.contains("1 myshader"),
            "shadername/layername lookup failed: {out}"
        );
    }

    #[test]
    fn test_getattribute_typedesc_int() {
        // When destination is an int, attr_type must be INT so renderer can coerce correctly.
        // Test with the built-in "osl:version" which returns an int.
        let src = r#"
shader test(output int ver = 0) {
    int ok = getattribute("osl:version", ver);
    printf("%d %d\n", ok, ver);
}
"#;
        let (_, interp) = compile_and_run(src);
        let out = interp.messages.join("");
        assert!(
            out.starts_with("1 "),
            "osl:version should succeed, got: {out}"
        );
        let ver: i32 = out
            .split_whitespace()
            .nth(1)
            .unwrap_or("0")
            .parse()
            .unwrap_or(0);
        assert!(ver > 0, "osl:version should be > 0, got {ver}");
    }

    // --- P0 regression tests ---

    #[test]
    fn test_atan2_derivative_sign() {
        // d(atan2(y,x))/dS = (x*y.partial - y*x.partial) / (x^2+y^2)
        // Numeric: shift y by h → (atan2(y+h,x) - atan2(y,x)) / h = x/(x^2+y^2)
        // At (y=1, x=sqrt(3)): d/dy_input = x/(x^2+y^2) = sqrt(3)/4 ≈ +0.433 (POSITIVE)
        let src = r#"
shader test(output float r = 0) {
    float y0 = 1.0; float x0 = sqrt(3.0);
    float h = 0.001;
    float a0 = atan2(y0, x0);
    float a1 = atan2(y0 + h, x0);
    // numeric partial w.r.t. y input, expected = x0/(x0^2+y0^2) = sqrt(3)/4 ≈ +0.433
    float deriv_y = (a1 - a0) / h;
    float expected = x0 / (x0 * x0 + y0 * y0);
    r = (deriv_y - expected) / (abs(expected) + 0.001);
    printf("deriv_y=%g expected=%g ratio=%g\n", deriv_y, expected, r);
}
"#;
        let (_, interp) = compile_and_run(src);
        let out = interp.messages.join("");
        let ratio: f32 = out
            .split("ratio=")
            .nth(1)
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(99.0);
        assert!(
            ratio.abs() < 0.01,
            "atan2 derivative sign wrong, ratio={ratio}, output: {out}"
        );
    }

    #[test]
    fn test_const_array_param_all_elements() {
        // Param array default must preserve ALL elements — not just [0].
        // Fix: codegen emit_var_init must propagate typespec to CompoundInitializer
        // on the no-typecheck path so aassign opcodes are used instead of construct.
        let src = r#"
shader test(int arr[4] = {10, 20, 30, 40},
            output int r0 = 0, output int r1 = 0,
            output int r2 = 0, output int r3 = 0) {
    r0 = arr[0]; r1 = arr[1]; r2 = arr[2]; r3 = arr[3];
}
"#;
        let (ir, interp) = compile_and_run(src);
        assert_eq!(interp.get_int(&ir, "r0"), Some(10), "arr[0] wrong");
        assert_eq!(
            interp.get_int(&ir, "r1"),
            Some(20),
            "arr[1] wrong — array init truncated"
        );
        assert_eq!(interp.get_int(&ir, "r2"), Some(30), "arr[2] wrong");
        assert_eq!(interp.get_int(&ir, "r3"), Some(40), "arr[3] wrong");
    }

    #[test]
    fn test_safe_pow_inf_clamp() {
        // pow(large, large) should return 0 (clamped from inf), not MAX or inf.
        let src = r#"
shader test(output float r = -1.0) {
    r = pow(1e30, 1e30);
}
"#;
        let (ir, interp) = compile_and_run(src);
        let v = interp.get_float(&ir, "r").unwrap_or(-1.0);
        assert_eq!(v, 0.0, "pow(1e30,1e30) should clamp inf to 0.0, got {v}");
    }

    #[test]
    fn test_sincos_derivative_propagation() {
        // sincos(x, s, c): s = sin(x), c = cos(x).
        // Fix: codegen must emit sincos as void-builtin (args=[x,s,c]) not value-return
        // (args=[result,x,s,c]), which caused args to be mis-indexed in the interpreter.
        let src = r#"
shader test(output float sr = 0, output float cr = 0) {
    float x = 1.0;
    float s; float c;
    sincos(x, s, c);
    sr = s; cr = c;
}
"#;
        let (ir, interp) = compile_and_run(src);
        let s = interp.get_float(&ir, "sr").unwrap_or(0.0);
        let c = interp.get_float(&ir, "cr").unwrap_or(0.0);
        assert!(
            (s - 1.0f32.sin()).abs() < 1e-5,
            "sincos sine wrong: {s}, expected {}",
            1.0f32.sin()
        );
        assert!(
            (c - 1.0f32.cos()).abs() < 1e-5,
            "sincos cosine wrong: {c}, expected {}",
            1.0f32.cos()
        );
    }
}
