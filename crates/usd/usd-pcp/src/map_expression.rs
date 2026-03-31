//! PCP Map Expression - lazy evaluation of map functions.
//!
//! An expression that yields a PcpMapFunction value. Expressions comprise
//! constant values, variables, and operators applied to sub-expressions.
//! Expressions cache their computed values internally.
//!
//! # C++ Parity
//!
//! This is a port of `pxr/usd/pcp/mapExpression.h` (~270 lines).
//!
//! # Overview
//!
//! PcpMapExpression exists solely to support efficient incremental handling
//! of relocates edits. It represents a tree of namespace mapping operations
//! and their inputs, so we can narrowly redo the computation when one of
//! the inputs changes.

use std::sync::{Arc, RwLock, Weak};

use crate::MapFunction;
use usd_sdf::{LayerOffset, Path};

/// Operation type for map expression nodes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Operation {
    /// A constant value.
    Constant,
    /// A mutable variable.
    Variable,
    /// Inverse of a map function.
    Inverse,
    /// Composition of two map functions.
    Compose,
    /// Add root identity mapping.
    AddRootIdentity,
}

/// Internal node in the expression tree.
///
/// Mirrors C++ `PcpMapExpression::_Node` — nodes register themselves as
/// dependents of their argument nodes so that invalidation propagates
/// transitively when a Variable is mutated via `set_variable_value()`.
struct ExpressionNode {
    /// The operation for this node.
    op: Operation,
    /// First argument (for unary/binary ops).
    arg1: Option<Arc<ExpressionNode>>,
    /// Second argument (for binary ops).
    arg2: Option<Arc<ExpressionNode>>,
    /// Constant value (for Constant/Variable op).
    /// For Variable nodes this is updated atomically through `variable_value`.
    constant_value: RwLock<MapFunction>,
    /// Cached result (computed lazily, cleared on invalidation).
    cached_value: RwLock<Option<MapFunction>>,
    /// Whether expression tree always has root identity.
    always_has_identity: bool,
    /// Weak back-references to nodes that depend on this one.
    /// When this node is invalidated, all live dependents are invalidated too.
    dependents: RwLock<Vec<Weak<ExpressionNode>>>,
}

impl ExpressionNode {
    /// Registers `dependent` as a downstream node of `self`.
    fn add_dependent(self_arc: &Arc<Self>, dependent: &Arc<Self>) {
        self_arc
            .dependents
            .write()
            .expect("dependents rwlock poisoned")
            .push(Arc::downgrade(dependent));
    }

    /// Creates a constant node.
    fn constant(value: MapFunction) -> Arc<Self> {
        let always_has_identity = value.has_root_identity();
        Arc::new(Self {
            op: Operation::Constant,
            arg1: None,
            arg2: None,
            constant_value: RwLock::new(value.clone()),
            cached_value: RwLock::new(Some(value)),
            always_has_identity,
            dependents: RwLock::new(Vec::new()),
        })
    }

    /// Creates a variable node. Value is stored behind `constant_value` RwLock
    /// so it can be updated without &mut.
    fn variable(initial_value: MapFunction) -> Arc<Self> {
        let always_has_identity = initial_value.has_root_identity();
        Arc::new(Self {
            op: Operation::Variable,
            arg1: None,
            arg2: None,
            constant_value: RwLock::new(initial_value.clone()),
            cached_value: RwLock::new(Some(initial_value)),
            always_has_identity,
            dependents: RwLock::new(Vec::new()),
        })
    }

    /// Creates an inverse node and registers it as a dependent of `arg`.
    fn inverse(arg: Arc<Self>) -> Arc<Self> {
        let always_has_identity = arg.always_has_identity;
        let node = Arc::new(Self {
            op: Operation::Inverse,
            arg1: Some(arg.clone()),
            arg2: None,
            constant_value: RwLock::new(MapFunction::null()),
            cached_value: RwLock::new(None),
            always_has_identity,
            dependents: RwLock::new(Vec::new()),
        });
        Self::add_dependent(&arg, &node);
        node
    }

    /// Creates a compose node and registers it as a dependent of both args.
    fn compose(self_arg: Arc<Self>, f: Arc<Self>) -> Arc<Self> {
        let always_has_identity = self_arg.always_has_identity && f.always_has_identity;
        let node = Arc::new(Self {
            op: Operation::Compose,
            arg1: Some(self_arg.clone()),
            arg2: Some(f.clone()),
            constant_value: RwLock::new(MapFunction::null()),
            cached_value: RwLock::new(None),
            always_has_identity,
            dependents: RwLock::new(Vec::new()),
        });
        Self::add_dependent(&self_arg, &node);
        Self::add_dependent(&f, &node);
        node
    }

    /// Creates an add-root-identity node and registers it as a dependent of `arg`.
    fn add_root_identity(arg: Arc<Self>) -> Arc<Self> {
        let node = Arc::new(Self {
            op: Operation::AddRootIdentity,
            arg1: Some(arg.clone()),
            arg2: None,
            constant_value: RwLock::new(MapFunction::null()),
            cached_value: RwLock::new(None),
            always_has_identity: true,
            dependents: RwLock::new(Vec::new()),
        });
        Self::add_dependent(&arg, &node);
        node
    }

    /// Evaluates the expression and caches the result.
    fn evaluate(&self) -> MapFunction {
        // Fast path: return cached value if present.
        if let Some(cached) = self.cached_value.read().expect("rwlock poisoned").as_ref() {
            return cached.clone();
        }
        let value = self.evaluate_uncached();
        *self.cached_value.write().expect("rwlock poisoned") = Some(value.clone());
        value
    }

    /// Computes the value for this node without touching the cache.
    fn evaluate_uncached(&self) -> MapFunction {
        match self.op {
            Operation::Constant | Operation::Variable => {
                self.constant_value.read().expect("rwlock poisoned").clone()
            }
            Operation::Inverse => {
                if let Some(arg) = &self.arg1 {
                    arg.evaluate().inverse()
                } else {
                    MapFunction::null()
                }
            }
            Operation::Compose => {
                if let (Some(self_arg), Some(f)) = (&self.arg1, &self.arg2) {
                    self_arg.evaluate().compose(&f.evaluate())
                } else {
                    MapFunction::null()
                }
            }
            Operation::AddRootIdentity => {
                if let Some(arg) = &self.arg1 {
                    let value = arg.evaluate();
                    if value.has_root_identity() {
                        value
                    } else {
                        let mut map = value.source_to_target_map();
                        map.insert(Path::absolute_root(), Path::absolute_root());
                        MapFunction::create(map, *value.time_offset()).unwrap_or(value)
                    }
                } else {
                    MapFunction::null()
                }
            }
        }
    }

    /// Clears cached value and propagates invalidation to all live dependents.
    ///
    /// Matches C++ `_Node::_Invalidate()` — only propagates if cache was set
    /// (avoids redundant work when already invalid).
    fn invalidate(&self) {
        let was_cached = {
            let mut guard = self.cached_value.write().expect("rwlock poisoned");
            let had = guard.is_some();
            *guard = None;
            had
        };
        if was_cached {
            // Propagate to dependent nodes (compose/inverse/add_root_identity
            // nodes that use this node as an argument).
            let deps = self.dependents.read().expect("dependents rwlock poisoned");
            for weak in deps.iter() {
                if let Some(dep) = weak.upgrade() {
                    dep.invalidate();
                }
            }
        }
    }

    /// For Variable nodes: updates the stored value and invalidates the cache.
    ///
    /// Matches C++ `_Node::SetValueForVariable`: only invalidates if the value
    /// actually changed (optimization to avoid spurious cache busts).
    fn set_variable_value(&self, value: MapFunction) {
        debug_assert_eq!(
            self.op,
            Operation::Variable,
            "set_variable_value on non-Variable"
        );
        let mut guard = self.constant_value.write().expect("rwlock poisoned");
        if *guard != value {
            *guard = value;
            // Drop the write lock before invalidating so dependents can read.
            drop(guard);
            self.invalidate();
        }
    }
}

/// An expression that yields a PcpMapFunction value.
///
/// Expressions comprise constant values, variables, and operators applied
/// to sub-expressions. Expressions cache their computed values internally.
///
/// # Examples
///
/// ```rust,ignore
/// use usd_pcp::MapExpression;
///
/// let identity = MapExpression::identity();
/// assert!(identity.is_identity());
///
/// let composed = identity.compose(&MapExpression::identity());
/// assert!(composed.is_identity());
/// ```
#[derive(Clone)]
pub struct MapExpression {
    node: Option<Arc<ExpressionNode>>,
}

impl Default for MapExpression {
    fn default() -> Self {
        Self::null()
    }
}

impl MapExpression {
    // ========================================================================
    // Construction
    // ========================================================================

    /// Creates a null expression.
    ///
    /// For a null expression, `evaluate()` returns a null map function.
    pub fn null() -> Self {
        Self { node: None }
    }

    /// Returns an expression representing the identity map function.
    pub fn identity() -> Self {
        Self {
            node: Some(ExpressionNode::constant(MapFunction::identity().clone())),
        }
    }

    /// Creates a new constant expression.
    pub fn constant(value: MapFunction) -> Self {
        Self {
            node: Some(ExpressionNode::constant(value)),
        }
    }

    // ========================================================================
    // Query
    // ========================================================================

    /// Returns true if this is a null expression.
    #[inline]
    pub fn is_null(&self) -> bool {
        self.node.is_none()
    }

    /// Returns true if the map function is the constant identity function.
    pub fn is_constant_identity(&self) -> bool {
        if let Some(node) = &self.node {
            node.op == Operation::Constant
                && node
                    .constant_value
                    .read()
                    .expect("rwlock poisoned")
                    .is_identity()
        } else {
            false
        }
    }

    // ========================================================================
    // Evaluation
    // ========================================================================

    /// Evaluates this expression, yielding a MapFunction value.
    ///
    /// The computed result is cached internally.
    pub fn evaluate(&self) -> MapFunction {
        match &self.node {
            Some(node) => node.evaluate(),
            None => MapFunction::null(),
        }
    }

    // ========================================================================
    // Convenience API (forwards to evaluated map function)
    // ========================================================================

    /// Returns true if the evaluated map function is the identity function.
    pub fn is_identity(&self) -> bool {
        self.evaluate().is_identity()
    }

    /// Maps a path in the source namespace to the target.
    pub fn map_source_to_target(&self, path: &Path) -> Option<Path> {
        self.evaluate().map_source_to_target(path)
    }

    /// Maps a path in the target namespace to the source.
    pub fn map_target_to_source(&self, path: &Path) -> Option<Path> {
        self.evaluate().map_target_to_source(path)
    }

    /// Returns the time offset of the mapping.
    pub fn time_offset(&self) -> LayerOffset {
        *self.evaluate().time_offset()
    }

    /// Returns a string representation of this mapping.
    pub fn get_string(&self) -> String {
        self.evaluate().debug_string()
    }

    // ========================================================================
    // Composition Operations
    // ========================================================================

    /// Creates a new expression representing the application of f's value,
    /// followed by the application of this expression's value.
    pub fn compose(&self, f: &MapExpression) -> MapExpression {
        match (&self.node, &f.node) {
            (None, _) | (_, None) => MapExpression::null(),
            (Some(self_node), Some(f_node)) => MapExpression {
                node: Some(ExpressionNode::compose(self_node.clone(), f_node.clone())),
            },
        }
    }

    /// Creates a new expression representing the inverse of this expression.
    pub fn inverse(&self) -> MapExpression {
        match &self.node {
            None => MapExpression::null(),
            Some(node) => MapExpression {
                node: Some(ExpressionNode::inverse(node.clone())),
            },
        }
    }

    /// Returns a new expression representing this expression with an added
    /// mapping from </> to </>.
    pub fn add_root_identity(&self) -> MapExpression {
        match &self.node {
            None => MapExpression::null(),
            Some(node) => {
                if node.always_has_identity {
                    // Already has root identity, return self
                    self.clone()
                } else {
                    MapExpression {
                        node: Some(ExpressionNode::add_root_identity(node.clone())),
                    }
                }
            }
        }
    }

    /// Swaps the contents of this expression with another.
    pub fn swap(&mut self, other: &mut MapExpression) {
        std::mem::swap(&mut self.node, &mut other.node);
    }
}

impl std::fmt::Debug for MapExpression {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MapExpression")
            .field("is_null", &self.is_null())
            .field("value", &self.evaluate().debug_string())
            .finish()
    }
}

// ============================================================================
// Variable
// ============================================================================

/// A mutable variable that holds a MapFunction value.
///
/// Variables allow dynamic updates to map expressions. Changing a variable's
/// value invalidates dependent expressions.
pub struct MapExpressionVariable {
    node: Arc<ExpressionNode>,
}

impl MapExpressionVariable {
    /// Creates a new variable with the initial value.
    pub fn new(initial_value: MapFunction) -> Self {
        Self {
            node: ExpressionNode::variable(initial_value),
        }
    }

    /// Returns the current value.
    pub fn get_value(&self) -> MapFunction {
        self.node.evaluate()
    }

    /// Sets a new value for the variable.
    ///
    /// Updates the stored value and transitively invalidates the cached results
    /// of all dependent compose/inverse/add_root_identity expressions, matching
    /// C++ `PcpMapExpression::Variable::SetValue()` semantics.
    pub fn set_value(&self, value: MapFunction) {
        self.node.set_variable_value(value);
    }

    /// Returns an expression representing this variable.
    pub fn get_expression(&self) -> MapExpression {
        MapExpression {
            node: Some(self.node.clone()),
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn test_null_expression() {
        let null = MapExpression::null();
        assert!(null.is_null());
        assert!(!null.is_identity());

        let path = Path::from_string("/Test").unwrap();
        assert!(null.map_source_to_target(&path).is_none());
    }

    #[test]
    fn test_identity_expression() {
        let identity = MapExpression::identity();
        assert!(!identity.is_null());
        assert!(identity.is_identity());
        assert!(identity.is_constant_identity());

        let path = Path::from_string("/Test/Mesh").unwrap();
        assert_eq!(identity.map_source_to_target(&path), Some(path));
    }

    #[test]
    fn test_constant_expression() {
        let mut path_map = BTreeMap::new();
        path_map.insert(
            Path::from_string("/A").unwrap(),
            Path::from_string("/B").unwrap(),
        );
        let map_fn = MapFunction::create(path_map, LayerOffset::identity()).unwrap();

        let expr = MapExpression::constant(map_fn);
        assert!(!expr.is_null());
        assert!(!expr.is_identity());

        let a = Path::from_string("/A").unwrap();
        let b = Path::from_string("/B").unwrap();
        assert_eq!(expr.map_source_to_target(&a), Some(b));
    }

    #[test]
    fn test_compose() {
        let identity = MapExpression::identity();
        let composed = identity.compose(&MapExpression::identity());
        assert!(composed.is_identity());
    }

    #[test]
    fn test_inverse() {
        let identity = MapExpression::identity();
        let inverse = identity.inverse();
        assert!(inverse.is_identity());
    }

    #[test]
    fn test_add_root_identity() {
        let identity = MapExpression::identity();
        let with_root = identity.add_root_identity();
        assert!(with_root.evaluate().has_root_identity());
    }

    #[test]
    fn test_swap() {
        let mut expr1 = MapExpression::null();
        let mut expr2 = MapExpression::identity();

        assert!(expr1.is_null());
        assert!(expr2.is_identity());

        expr1.swap(&mut expr2);

        assert!(expr1.is_identity());
        assert!(expr2.is_null());
    }

    #[test]
    fn test_variable() {
        let var = MapExpressionVariable::new(MapFunction::identity().clone());
        let expr = var.get_expression();

        assert!(expr.is_identity());
        assert!(var.get_value().is_identity());
    }

    // =========================================================================
    // Tests for set_value() + cache invalidation
    // =========================================================================

    /// set_value() updates the variable; the shared expression sees the new value.
    #[test]
    fn test_variable_set_value_cache_invalidation() {
        let var = MapExpressionVariable::new(MapFunction::identity().clone());
        let expr = var.get_expression();

        assert!(expr.is_identity(), "initial state must be identity");

        // Update to /A -> /B
        let mut path_map = BTreeMap::new();
        path_map.insert(
            Path::from_string("/A").unwrap(),
            Path::from_string("/B").unwrap(),
        );
        let new_fn = MapFunction::create(path_map, LayerOffset::identity()).unwrap();
        var.set_value(new_fn);

        // Expression must no longer be identity after set_value
        assert!(
            !expr.is_identity(),
            "expression must not be identity after set_value"
        );
        let a = Path::from_string("/A").unwrap();
        let b = Path::from_string("/B").unwrap();
        assert_eq!(
            expr.map_source_to_target(&a),
            Some(b.clone()),
            "set_value must invalidate cache so expression maps /A -> /B"
        );
        assert_eq!(var.get_value().map_source_to_target(&a), Some(b));
    }

    /// After set_value(), a compose expression that depends on the variable
    /// must also reflect the new value (transitive evaluation).
    #[test]
    fn test_variable_set_value_visible_in_composed_expression() {
        // var: /A -> /B
        let mut init_map = BTreeMap::new();
        init_map.insert(
            Path::from_string("/A").unwrap(),
            Path::from_string("/B").unwrap(),
        );
        let var = MapExpressionVariable::new(
            MapFunction::create(init_map, LayerOffset::identity()).unwrap(),
        );
        let var_expr = var.get_expression();

        // g: / -> /, /B -> /C  (constant outer)
        let mut map_g = BTreeMap::new();
        map_g.insert(Path::absolute_root(), Path::absolute_root());
        map_g.insert(
            Path::from_string("/B").unwrap(),
            Path::from_string("/C").unwrap(),
        );
        let g_expr =
            MapExpression::constant(MapFunction::create(map_g, LayerOffset::identity()).unwrap());

        // composed = g.compose(var): /A -> /C initially
        let composed = g_expr.compose(&var_expr);
        assert_eq!(
            composed.map_source_to_target(&Path::from_string("/A").unwrap()),
            Path::from_string("/C"),
            "initial composed: /A -> /B -> /C"
        );

        // Update var to /X -> /B; now /X should chain to /C
        let mut new_map = BTreeMap::new();
        new_map.insert(
            Path::from_string("/X").unwrap(),
            Path::from_string("/B").unwrap(),
        );
        var.set_value(MapFunction::create(new_map, LayerOffset::identity()).unwrap());

        assert_eq!(
            composed.map_source_to_target(&Path::from_string("/X").unwrap()),
            Path::from_string("/C"),
            "after set_value, /X -> /B -> /C via composed expression"
        );
    }

    /// Multiple consecutive set_value() calls always show the latest value.
    #[test]
    fn test_variable_set_value_multiple_updates() {
        let var = MapExpressionVariable::new(MapFunction::identity().clone());

        for i in 0u32..3 {
            let src_str = format!("/Src{}", i);
            let dst_str = format!("/Dst{}", i);
            let mut pm = BTreeMap::new();
            pm.insert(
                Path::from_string(&src_str).unwrap(),
                Path::from_string(&dst_str).unwrap(),
            );
            var.set_value(MapFunction::create(pm, LayerOffset::identity()).unwrap());

            let got = var
                .get_value()
                .map_source_to_target(&Path::from_string(&src_str).unwrap());
            assert_eq!(
                got,
                Path::from_string(&dst_str),
                "iteration {i}: freshly set value must be readable immediately"
            );
        }
    }

    /// inverse() of a constant expression round-trips correctly.
    #[test]
    fn test_inverse_expression_round_trip() {
        let mut pm = BTreeMap::new();
        pm.insert(Path::absolute_root(), Path::absolute_root());
        pm.insert(
            Path::from_string("/M").unwrap(),
            Path::from_string("/N").unwrap(),
        );
        let expr_a =
            MapExpression::constant(MapFunction::create(pm, LayerOffset::identity()).unwrap());
        let expr_inv = expr_a.inverse();

        let n = Path::from_string("/N").unwrap();
        let m = Path::from_string("/M").unwrap();
        assert_eq!(
            expr_inv.map_source_to_target(&n),
            Some(m.clone()),
            "inverse must map /N -> /M"
        );
        // Double-inverse: expr_a(inv(N)) = expr_a(M) = N
        let back = expr_a
            .map_source_to_target(&expr_inv.map_source_to_target(&n).unwrap())
            .unwrap();
        assert_eq!(back, n, "double-inverse round-trip must be identity");
    }

    /// add_root_identity() on a constant with no root identity adds / -> /.
    #[test]
    fn test_add_root_identity_on_constant_without_root() {
        let mut pm = BTreeMap::new();
        pm.insert(
            Path::from_string("/A").unwrap(),
            Path::from_string("/B").unwrap(),
        );
        let fn_no_root = MapFunction::create(pm, LayerOffset::identity()).unwrap();
        assert!(!fn_no_root.has_root_identity());

        let expr = MapExpression::constant(fn_no_root);
        assert!(!expr.evaluate().has_root_identity());

        let with_root = expr.add_root_identity();
        assert!(
            with_root.evaluate().has_root_identity(),
            "add_root_identity() must add / -> / mapping"
        );
        // Original /A -> /B mapping is still present
        assert_eq!(
            with_root.map_source_to_target(&Path::from_string("/A").unwrap()),
            Path::from_string("/B")
        );
    }

    // =========================================================================
    // Tests ported from C++ testPcpMapExpression.cpp
    // =========================================================================

    fn arc_fn(source: &str, target: &str) -> MapFunction {
        let mut pm = BTreeMap::new();
        pm.insert(
            Path::from_string(source).unwrap(),
            Path::from_string(target).unwrap(),
        );
        MapFunction::create(pm, LayerOffset::identity()).unwrap()
    }

    /// Port of TestMapFunctionHash from testPcpMapExpression.cpp
    #[test]
    fn test_cpp_map_function_hash() {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        fn hash_of(f: &MapFunction) -> u64 {
            let mut h = DefaultHasher::new();
            f.hash(&mut h);
            h.finish()
        }

        // Null hashes equal
        assert_eq!(hash_of(&MapFunction::null()), hash_of(&MapFunction::null()));

        // Same pairs + offset hash equal
        let mut pm = BTreeMap::new();
        pm.insert(
            Path::from_string("/path/source").unwrap(),
            Path::from_string("/path/target").unwrap(),
        );
        pm.insert(
            Path::from_string("/path/source2").unwrap(),
            Path::from_string("/path/target2").unwrap(),
        );
        let f1 = MapFunction::create(pm.clone(), LayerOffset::identity()).unwrap();
        let f2 = MapFunction::create(pm.clone(), LayerOffset::identity()).unwrap();
        assert_eq!(hash_of(&f1), hash_of(&f2));

        let f3 = MapFunction::create(pm.clone(), LayerOffset::new(1.0, 2.0)).unwrap();
        let f4 = MapFunction::create(pm, LayerOffset::new(1.0, 2.0)).unwrap();
        assert_eq!(hash_of(&f3), hash_of(&f4));
    }

    /// Port of main() Null/Identity/Swap/Constant from testPcpMapExpression.cpp
    #[test]
    fn test_cpp_null_identity_swap_constant() {
        // Null
        let null_expr = MapExpression::null();
        assert!(null_expr.is_null());
        assert_eq!(null_expr.evaluate(), MapFunction::null());

        // Identity
        let identity_expr = MapExpression::identity();
        assert!(!identity_expr.is_null());
        assert_eq!(identity_expr.evaluate(), *MapFunction::identity());

        // Swap
        let mut a = MapExpression::null();
        let mut b = MapExpression::identity();
        assert!(a.is_null());
        assert!(!b.is_null());
        std::mem::swap(&mut a, &mut b);
        assert!(!a.is_null());
        assert!(b.is_null());

        // Constant (typical model reference)
        let ref_func = arc_fn("/Model", "/World/anim/Model_1");
        let ref_expr = MapExpression::constant(ref_func.clone());
        assert_eq!(ref_expr.evaluate(), ref_func);
    }

    /// Port of Inverse operation from testPcpMapExpression.cpp
    #[test]
    fn test_cpp_inverse() {
        let ref_func = arc_fn("/Model", "/World/anim/Model_1");
        let ref_expr = MapExpression::constant(ref_func.clone());
        let ref_inverse = ref_expr.inverse();
        assert!(!ref_inverse.is_null());
        assert_eq!(ref_inverse.evaluate(), ref_func.inverse());
    }

    /// Port of AddRootIdentity from testPcpMapExpression.cpp
    #[test]
    fn test_cpp_add_root_identity() {
        let ref_func = arc_fn("/Model", "/World/anim/Model_1");
        let ref_expr = MapExpression::constant(ref_func);

        // Without root identity, /Foo doesn't map
        assert!(
            ref_expr
                .map_source_to_target(&Path::from_string("/Foo").unwrap())
                .is_none()
        );

        let root_id_expr = ref_expr.add_root_identity();
        // With root identity, /Foo maps to /Foo
        assert_eq!(
            root_id_expr.map_source_to_target(&Path::from_string("/Foo").unwrap()),
            Path::from_string("/Foo")
        );
    }

    /// Port of Compose from testPcpMapExpression.cpp
    #[test]
    fn test_cpp_compose() {
        let ref_expr = MapExpression::constant(arc_fn("/Model", "/World/anim/Model_1"));
        let rig_expr = MapExpression::constant(arc_fn("/Rig", "/Model/Rig"));
        let composed = ref_expr.compose(&rig_expr);

        let expected = arc_fn("/Rig", "/World/anim/Model_1/Rig");
        assert_eq!(composed.evaluate(), expected);

        // Compose + Inverse
        let expected_inv = arc_fn("/World/anim/Model_1/Rig", "/Rig");
        assert_eq!(composed.inverse().evaluate(), expected_inv);
    }

    /// Port of Variable tests from testPcpMapExpression.cpp
    #[test]
    fn test_cpp_variable() {
        // Variable with initial null function
        let var = MapExpressionVariable::new(MapFunction::null());
        let var_expr = var.get_expression();
        assert!(!var_expr.is_null());
        assert_eq!(var_expr.evaluate(), MapFunction::null());

        // Change value
        let test_val = arc_fn("/A", "/B");
        var.set_value(test_val.clone());
        assert_eq!(var_expr.evaluate(), test_val);

        // Derived expression (inverse) tracks variable
        let inv_expr = var_expr.inverse();
        assert_eq!(inv_expr.evaluate(), test_val.inverse());

        // Change variable again — derived expressions update
        let test_val2 = arc_fn("/A2", "/B2");
        var.set_value(test_val2.clone());
        assert_eq!(var_expr.evaluate(), test_val2);
        assert_eq!(inv_expr.evaluate(), test_val2.inverse());
    }

    /// Port of semi-tricky AddRootIdentity scenario from testPcpMapExpression.cpp
    #[test]
    fn test_cpp_tricky_add_root_identity_compose() {
        let a_to_b = arc_fn("/A", "/B");
        let b_to_c = arc_fn("/B", "/C");
        let a_to_c = arc_fn("/A", "/C");

        // Compose b_to_c over (a_to_b + root identity)
        let exp = MapExpression::constant(b_to_c)
            .compose(&MapExpression::constant(a_to_b).add_root_identity());
        assert_eq!(exp.evaluate(), a_to_c);

        // AddRootIdentity on composed result should match direct construction
        let a_to_c_with_id = MapExpression::constant(a_to_c).add_root_identity();
        let exp_with_id = exp.add_root_identity();
        assert_eq!(exp_with_id.evaluate(), a_to_c_with_id.evaluate());
    }
}
