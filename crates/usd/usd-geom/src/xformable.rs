//! UsdGeomXformable - base class for all transformable prims.
//!
//! Port of pxr/usd/usdGeom/xformable.h/cpp
//!
//! Base class for all transformable prims, which allows arbitrary
//! sequences of component affine transformations to be encoded.

use super::imageable::Imageable;
use super::tokens::usd_geom_tokens;
use super::xform_op::{XformOp, XformOpPrecision, XformOpType};
use std::sync::LazyLock;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use usd_core::{Attribute, AttributeQuery, Prim};
use usd_gf::Interval;
use usd_gf::Matrix4d;
use usd_sdf::TimeCode;
use usd_tf::Token;

static DEBUG_XFORM_QUERY_CALLS: AtomicUsize = AtomicUsize::new(0);
static DEBUG_XFORM_QUERY_TOTAL_NS: AtomicU64 = AtomicU64::new(0);
static DEBUG_GET_ORDERED_XFORM_OPS_CALLS: AtomicUsize = AtomicUsize::new(0);
static DEBUG_GET_ORDERED_XFORM_OPS_TOTAL_NS: AtomicU64 = AtomicU64::new(0);
static DEBUG_GET_ORDERED_XFORM_OPS_TOTAL_OPS: AtomicUsize = AtomicUsize::new(0);
static DEBUG_GET_XFORM_OP_ORDER_VALUE_CALLS: AtomicUsize = AtomicUsize::new(0);
static DEBUG_GET_XFORM_OP_ORDER_VALUE_TOTAL_NS: AtomicU64 = AtomicU64::new(0);
static DEBUG_XFORM_QUERY_LOCAL_XFORM_CALLS: AtomicUsize = AtomicUsize::new(0);
static DEBUG_XFORM_QUERY_LOCAL_XFORM_TOTAL_NS: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, Default)]
pub struct DebugXformableStats {
    pub xform_query_calls: usize,
    pub xform_query_total_ns: u64,
    pub get_ordered_xform_ops_calls: usize,
    pub get_ordered_xform_ops_total_ns: u64,
    pub get_ordered_xform_ops_total_ops: usize,
    pub get_xform_op_order_value_calls: usize,
    pub get_xform_op_order_value_total_ns: u64,
    pub xform_query_local_xform_calls: usize,
    pub xform_query_local_xform_total_ns: u64,
}

pub fn reset_debug_xformable_stats() {
    DEBUG_XFORM_QUERY_CALLS.store(0, Ordering::Relaxed);
    DEBUG_XFORM_QUERY_TOTAL_NS.store(0, Ordering::Relaxed);
    DEBUG_GET_ORDERED_XFORM_OPS_CALLS.store(0, Ordering::Relaxed);
    DEBUG_GET_ORDERED_XFORM_OPS_TOTAL_NS.store(0, Ordering::Relaxed);
    DEBUG_GET_ORDERED_XFORM_OPS_TOTAL_OPS.store(0, Ordering::Relaxed);
    DEBUG_GET_XFORM_OP_ORDER_VALUE_CALLS.store(0, Ordering::Relaxed);
    DEBUG_GET_XFORM_OP_ORDER_VALUE_TOTAL_NS.store(0, Ordering::Relaxed);
    DEBUG_XFORM_QUERY_LOCAL_XFORM_CALLS.store(0, Ordering::Relaxed);
    DEBUG_XFORM_QUERY_LOCAL_XFORM_TOTAL_NS.store(0, Ordering::Relaxed);
}

pub fn read_debug_xformable_stats() -> DebugXformableStats {
    DebugXformableStats {
        xform_query_calls: DEBUG_XFORM_QUERY_CALLS.load(Ordering::Relaxed),
        xform_query_total_ns: DEBUG_XFORM_QUERY_TOTAL_NS.load(Ordering::Relaxed),
        get_ordered_xform_ops_calls: DEBUG_GET_ORDERED_XFORM_OPS_CALLS.load(Ordering::Relaxed),
        get_ordered_xform_ops_total_ns: DEBUG_GET_ORDERED_XFORM_OPS_TOTAL_NS
            .load(Ordering::Relaxed),
        get_ordered_xform_ops_total_ops: DEBUG_GET_ORDERED_XFORM_OPS_TOTAL_OPS
            .load(Ordering::Relaxed),
        get_xform_op_order_value_calls: DEBUG_GET_XFORM_OP_ORDER_VALUE_CALLS
            .load(Ordering::Relaxed),
        get_xform_op_order_value_total_ns: DEBUG_GET_XFORM_OP_ORDER_VALUE_TOTAL_NS
            .load(Ordering::Relaxed),
        xform_query_local_xform_calls: DEBUG_XFORM_QUERY_LOCAL_XFORM_CALLS
            .load(Ordering::Relaxed),
        xform_query_local_xform_total_ns: DEBUG_XFORM_QUERY_LOCAL_XFORM_TOTAL_NS
            .load(Ordering::Relaxed),
    }
}

fn debug_time_dirty_enabled() -> bool {
    std::env::var_os("USD_RS_DEBUG_TIME_DIRTY").is_some()
}

static RESET_XFORM_STACK_TOKEN: LazyLock<Token> = LazyLock::new(|| Token::new("!resetXformStack!"));

// ============================================================================
// Helpers
// ============================================================================

/// Returns true if two XformOps are inverses of each other.
/// They must share the same underlying attribute and differ only in is_inverse_op.
/// Matches C++ `_AreInverseXformOps(const UsdGeomXformOp&, const UsdGeomXformOp&)`.
fn are_inverse_xform_ops(a: &XformOp, b: &XformOp) -> bool {
    a.attr().name() == b.attr().name() && a.is_inverse_op() != b.is_inverse_op()
}

// ============================================================================
// Xformable
// ============================================================================

/// Base class for all transformable prims.
///
/// Base class for all transformable prims, which allows arbitrary
/// sequences of component affine transformations to be encoded.
///
/// Matches C++ `UsdGeomXformable`.
#[derive(Debug, Clone)]
pub struct Xformable {
    /// Base imageable schema.
    inner: Imageable,
}

impl Xformable {
    /// Creates a Xformable schema from a prim.
    ///
    /// Matches C++ `UsdGeomXformable(const UsdPrim& prim)`.
    pub fn new(prim: Prim) -> Self {
        Self {
            inner: Imageable::new(prim),
        }
    }

    /// Creates a Xformable schema from an Imageable schema.
    ///
    /// Matches C++ `UsdGeomXformable(const UsdSchemaBase& schemaObj)`.
    pub fn from_imageable(imageable: Imageable) -> Self {
        Self { inner: imageable }
    }

    /// Creates an invalid Xformable schema.
    pub fn invalid() -> Self {
        Self {
            inner: Imageable::invalid(),
        }
    }

    /// Returns true if this schema is valid.
    pub fn is_valid(&self) -> bool {
        self.inner.is_valid()
    }

    /// Returns the wrapped prim.
    pub fn prim(&self) -> &Prim {
        self.inner.prim()
    }

    /// Returns the imageable base.
    pub fn imageable(&self) -> &Imageable {
        &self.inner
    }

    /// Returns the schema type name.
    pub fn schema_type_name() -> Token {
        Token::new("Xformable")
    }

    // ========================================================================
    // XformOpOrder
    // ========================================================================

    /// Returns the xformOpOrder attribute.
    ///
    /// Encodes the sequence of transformation operations in the
    /// order in which they should be pushed onto a transform stack.
    ///
    /// Matches C++ `GetXformOpOrderAttr()`.
    pub fn get_xform_op_order_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().xform_op_order.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the xformOpOrder attribute.
    ///
    /// Matches C++ `CreateXformOpOrderAttr()`.
    pub fn create_xform_op_order_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        // Get or create the attribute with proper type (TokenArray) and variability (Uniform)
        if prim.has_authored_attribute(usd_geom_tokens().xform_op_order.as_str()) {
            return prim
                .get_attribute(usd_geom_tokens().xform_op_order.as_str())
                .unwrap_or_else(|| Attribute::invalid());
        }

        let registry = usd_sdf::ValueTypeRegistry::instance();
        let token_array_type = registry.find_type_by_token(&Token::new("token[]"));

        prim.create_attribute(
            usd_geom_tokens().xform_op_order.as_str(),
            &token_array_type,
            false,                                           // not custom
            Some(usd_core::attribute::Variability::Uniform), // xformOpOrder is uniform
        )
        .unwrap_or_else(Attribute::invalid)
    }

    // ========================================================================
    // Internal Helpers
    // ========================================================================

    /// Internal helper: Get xformOpOrder value.
    fn get_xform_op_order_value(&self) -> Option<Vec<Token>> {
        let debug_stats = debug_time_dirty_enabled();
        let started = debug_stats.then(std::time::Instant::now);
        if debug_stats {
            DEBUG_GET_XFORM_OP_ORDER_VALUE_CALLS.fetch_add(1, Ordering::Relaxed);
        }
        let xform_op_order_attr = self.get_xform_op_order_attr();
        if !xform_op_order_attr.is_valid() {
            if debug_stats {
                if let Some(started) = started {
                    DEBUG_GET_XFORM_OP_ORDER_VALUE_TOTAL_NS
                        .fetch_add(started.elapsed().as_nanos() as u64, Ordering::Relaxed);
                }
            }
            return None;
        }

        let result = Some(
            xform_op_order_attr
                .get_typed_vec::<Token>(TimeCode::default())
                .unwrap_or_default(),
        );
        if debug_stats {
            if let Some(started) = started {
                DEBUG_GET_XFORM_OP_ORDER_VALUE_TOTAL_NS
                    .fetch_add(started.elapsed().as_nanos() as u64, Ordering::Relaxed);
            }
        }
        result
    }

    // ========================================================================
    // XformOp Management
    // ========================================================================

    /// Add an affine transformation to the local stack represented by this
    /// Xformable.
    ///
    /// Matches C++ `AddXformOp()`.
    pub fn add_xform_op(
        &self,
        op_type: XformOpType,
        precision: XformOpPrecision,
        op_suffix: Option<&Token>,
        is_inverse_op: bool,
    ) -> XformOp {
        let mut xform_op_order = self.get_xform_op_order_value().unwrap_or_default();

        // Check if the xformOp we're about to add already exists in xformOpOrder
        let op_name = XformOp::get_op_name(op_type, op_suffix, is_inverse_op);
        if xform_op_order.contains(&op_name) {
            // Error: op already exists
            return XformOp::invalid();
        }

        let xform_op_attr_name = XformOp::get_op_name(op_type, op_suffix, false);
        let prim = self.inner.prim();

        // Try to get existing attribute (only use it if it's a real authored spec)
        let xform_op_attr = prim
            .get_attribute(xform_op_attr_name.as_str())
            .filter(|a| a.is_valid());

        let result = if let Some(attr) = xform_op_attr {
            // Check if the attribute's typeName has the requested precision level
            let attr_type_token = attr.type_name();
            let registry = usd_sdf::ValueTypeRegistry::instance();
            let attr_type = registry.find_type_by_token(&attr_type_token);
            if attr_type.is_valid() {
                let attr_precision = XformOp::get_precision_from_value_type_name(&attr_type);
                if attr_precision != precision {
                    // Precision mismatch - return invalid
                    return XformOp::invalid();
                }
            }
            XformOp::new(attr, is_inverse_op)
        } else {
            // Create new attribute with proper type name and precision
            let type_name = XformOp::get_value_type_name(op_type, precision);
            if !type_name.is_valid() {
                return XformOp::invalid();
            }

            let attr = prim.create_attribute(
                xform_op_attr_name.as_str(),
                &type_name,
                false,                                           // not custom
                Some(usd_core::attribute::Variability::Varying), // can vary over time
            );

            if let Some(attr) = attr {
                XformOp::new(attr, is_inverse_op)
            } else {
                return XformOp::invalid();
            }
        };

        if result.is_valid() {
            xform_op_order.push(result.op_name());
            let order_value = usd_vt::Value::new(xform_op_order);
            self.create_xform_op_order_attr()
                .set(order_value, TimeCode::default());
        }

        result
    }

    /// Get an affine transformation from the local stack represented by this
    /// Xformable.
    ///
    /// Matches C++ `GetXformOp()`.
    pub fn get_xform_op(
        &self,
        op_type: XformOpType,
        op_suffix: Option<&Token>,
        is_inverse_op: bool,
    ) -> XformOp {
        let xform_op_order = self.get_xform_op_order_value().unwrap_or_default();

        // Check if the xformOp exists in xformOpOrder
        let op_name = XformOp::get_op_name(op_type, op_suffix, is_inverse_op);
        if !xform_op_order.contains(&op_name) {
            return XformOp::invalid();
        }

        let xform_op_attr_name = XformOp::get_op_name(op_type, op_suffix, false);
        let prim = self.inner.prim();

        if let Some(attr) = prim.get_attribute(xform_op_attr_name.as_str()) {
            XformOp::new(attr, is_inverse_op)
        } else {
            XformOp::invalid()
        }
    }

    /// Get the ordered vector of XformOps that contribute to the local
    /// transformation of this xformable prim.
    ///
    /// Matches C++ `GetOrderedXformOps(bool *resetsXformStack)`.
    pub fn get_ordered_xform_ops(&self) -> Vec<XformOp> {
        let mut _resets = false;
        self.get_ordered_xform_ops_with_reset(&mut _resets)
    }

    /// Get the ordered vector of XformOps with reset flag.
    ///
    /// Matches C++ `GetOrderedXformOps(bool *resetsXformStack)`.
    pub fn get_ordered_xform_ops_with_reset(&self, resets_xform_stack: &mut bool) -> Vec<XformOp> {
        self.get_ordered_xform_ops_with_reset_impl(resets_xform_stack, false)
    }

    fn get_ordered_xform_ops_with_reset_impl(
        &self,
        resets_xform_stack: &mut bool,
        with_attribute_queries: bool,
    ) -> Vec<XformOp> {
        let debug_stats = debug_time_dirty_enabled();
        let started = debug_stats.then(std::time::Instant::now);
        if debug_stats {
            DEBUG_GET_ORDERED_XFORM_OPS_CALLS.fetch_add(1, Ordering::Relaxed);
        }
        *resets_xform_stack = false;
        let xform_op_order = self.get_xform_op_order_value().unwrap_or_default();
        let prim = self.inner.prim();
        let mut result = Vec::with_capacity(xform_op_order.len());

        // Pre-fetch PrimIndex once for all xformOp AttributeQuery constructions.
        // Each AttributeQuery::new → get_resolve_info does stage.get_prim_at_path
        // + prim_index_arc() — redundant when all ops share the same prim.
        let cached_prim_index = if with_attribute_queries {
            prim.prim_index_arc()
        } else {
            None
        };

        for op_name_token in xform_op_order {
            if op_name_token == *RESET_XFORM_STACK_TOKEN {
                *resets_xform_stack = true;
                result.clear();
                continue;
            }

            let mut is_inverse = false;
            if let Some(attr) = XformOp::get_xform_op_attr(&prim, &op_name_token, &mut is_inverse) {
                if with_attribute_queries {
                    // Use cached PrimIndex to skip per-attribute stage lookup.
                    let query = if let Some(ref idx) = cached_prim_index {
                        usd_core::AttributeQuery::new_with_prim_index(attr, idx)
                    } else {
                        usd_core::AttributeQuery::new(attr)
                    };
                    result.push(XformOp::new_with_query(query, is_inverse));
                } else {
                    result.push(XformOp::new(attr, is_inverse));
                }
            }
        }

        if debug_stats {
            DEBUG_GET_ORDERED_XFORM_OPS_TOTAL_OPS.fetch_add(result.len(), Ordering::Relaxed);
            if let Some(started) = started {
                DEBUG_GET_ORDERED_XFORM_OPS_TOTAL_NS
                    .fetch_add(started.elapsed().as_nanos() as u64, Ordering::Relaxed);
            }
        }
        result
    }

    /// Set the xformOpOrder to the given ordered list of xformOps.
    ///
    /// Matches C++ `SetXformOpOrder()`.
    pub fn set_xform_op_order(&self, ordered_ops: &[XformOp]) -> bool {
        self.set_xform_op_order_with_reset(ordered_ops, false)
    }

    /// Set the xformOpOrder to the given ordered list of xformOps, optionally resetting the transform stack.
    ///
    /// Matches C++ `SetXformOpOrder(const std::vector<UsdGeomXformOp> &orderedXformOps, bool resetXformStack)`.
    pub fn set_xform_op_order_with_reset(
        &self,
        ordered_ops: &[XformOp],
        reset_xform_stack: bool,
    ) -> bool {
        let mut op_names: Vec<Token> = Vec::new();

        if reset_xform_stack {
            op_names.push(Token::new("!resetXformStack!"));
        }

        // Verify all ops belong to this prim
        let prim_path = self.prim().path();
        for op in ordered_ops {
            // Check if op's attribute belongs to this prim
            // Compare paths by their string representation
            if op.attr().prim_path().get_string() != prim_path.get_string() {
                // Error: op doesn't belong to this prim
                return false;
            }
            op_names.push(op.op_name());
        }

        let order_value = usd_vt::Value::new(op_names);
        self.create_xform_op_order_attr()
            .set(order_value, TimeCode::default())
    }

    /// Set whether this prim should reset the transform stack inherited from
    /// its namespace parent.
    ///
    /// Matches C++ `SetResetXformStack()`.
    pub fn set_reset_xform_stack(&self, reset: bool) -> bool {
        let mut xform_op_order = self.get_xform_op_order_value().unwrap_or_default();
        let reset_token = Token::new("!resetXformStack!");

        if reset {
            // No-op if resetXformStack already anywhere in order.
            // C++ uses std::find across entire array, not just [0].
            if xform_op_order.iter().any(|t| *t == reset_token) {
                return true;
            }
            xform_op_order.insert(0, reset_token);
        } else {
            // C++ iterates all elements: on each resetXformStack hit,
            // clears newVec + sets found=true; after hit, pushes elements.
            // Effect: keeps only elements after the LAST resetXformStack.
            let mut new_order = Vec::new();
            let mut found = false;
            for t in &xform_op_order {
                if *t == reset_token {
                    found = true;
                    new_order.clear();
                } else if found {
                    new_order.push(t.clone());
                }
            }
            if !found {
                // Token not present — no-op, matching C++
                return true;
            }
            xform_op_order = new_order;
        }

        let order_value = usd_vt::Value::new(xform_op_order);
        self.create_xform_op_order_attr()
            .set(order_value, TimeCode::default())
    }

    /// Returns whether this prim resets the transform stack inherited from
    /// its namespace parent.
    ///
    /// Matches C++ `GetResetXformStack()`.
    pub fn get_reset_xform_stack(&self) -> bool {
        let xform_op_order = self.get_xform_op_order_value().unwrap_or_default();
        if xform_op_order.is_empty() {
            return false;
        }
        let reset_token = Token::new("!resetXformStack!");
        // C++ scans the full array, not just index [0]
        xform_op_order.iter().any(|t| t == &reset_token)
    }

    /// Compute the fully-combined, local-to-parent transformation for this prim.
    ///
    /// Matches C++ `GetLocalTransformation(GfMatrix4d *transform, bool *resetsXformStack, UsdTimeCode time)`.
    pub fn get_local_transformation_with_reset(&self, time: TimeCode) -> (Matrix4d, bool) {
        let mut resets_xform_stack = false;
        let ordered_ops = self.get_ordered_xform_ops_with_reset(&mut resets_xform_stack);
        let transform = Self::get_local_transformation_from_ops(&ordered_ops, time);
        (transform, resets_xform_stack)
    }

    /// Compute the local transformation matrix for this prim at the given time.
    ///
    /// Matches C++ `GetLocalTransformation()`.
    pub fn get_local_transformation(&self, time: TimeCode) -> Matrix4d {
        let mut resets_xform_stack = false;
        let ordered_ops = self.get_ordered_xform_ops_with_reset(&mut resets_xform_stack);
        Self::get_local_transformation_from_ops(&ordered_ops, time)
    }

    /// Static helper: Compute local transformation from ordered ops.
    ///
    /// Matches C++ `GetLocalTransformation(GfMatrix4d *transform, const std::vector<UsdGeomXformOp> &orderedXformOps, UsdTimeCode time)`.
    /// C++ iterates in REVERSE order and accumulates via `xform *= opTransform`.
    pub fn get_local_transformation_from_ops(ordered_ops: &[XformOp], time: TimeCode) -> Matrix4d {
        let mut result = Matrix4d::identity();
        let len = ordered_ops.len();

        // Iterate in reverse order, matching C++ (rbegin -> rend)
        let mut i = len;
        while i > 0 {
            i -= 1;
            let op = &ordered_ops[i];

            // Skip adjacent inverse pairs (optimization matching C++ _AreInverseXformOps).
            // When pair found: i-=1 skips partner, continue triggers top i-=1
            // to advance past both — correctly skipping exactly 2 elements.
            if i > 0 {
                let next_op = &ordered_ops[i - 1];
                if are_inverse_xform_ops(op, next_op) {
                    i -= 1; // Skip partner; loop top will skip current
                    continue;
                }
            }

            let op_transform = op.get_op_transform(time);
            // Skip identity matrices (optimization matching C++)
            if op_transform != Matrix4d::identity() {
                result *= op_transform;
            }
        }

        result
    }

    /// Return a vector of names of all pre-declared attributes for this schema
    /// class and all its ancestor classes.
    ///
    /// Matches C++ `GetSchemaAttributeNames()`.
    pub fn get_schema_attribute_names(include_inherited: bool) -> Vec<Token> {
        let local_names = vec![usd_geom_tokens().xform_op_order.clone()];

        if include_inherited {
            let mut all_names = Imageable::get_schema_attribute_names(true);
            all_names.extend(local_names);
            all_names
        } else {
            local_names
        }
    }

    // ========================================================================
    // Convenience Methods: Add*Op
    // ========================================================================

    /// Add a translation about the X-axis.
    pub fn add_translate_x_op(
        &self,
        precision: XformOpPrecision,
        op_suffix: Option<&Token>,
        is_inverse_op: bool,
    ) -> XformOp {
        self.add_xform_op(XformOpType::TranslateX, precision, op_suffix, is_inverse_op)
    }

    /// Add a translation about the Y-axis.
    pub fn add_translate_y_op(
        &self,
        precision: XformOpPrecision,
        op_suffix: Option<&Token>,
        is_inverse_op: bool,
    ) -> XformOp {
        self.add_xform_op(XformOpType::TranslateY, precision, op_suffix, is_inverse_op)
    }

    /// Add a translation about the Z-axis.
    pub fn add_translate_z_op(
        &self,
        precision: XformOpPrecision,
        op_suffix: Option<&Token>,
        is_inverse_op: bool,
    ) -> XformOp {
        self.add_xform_op(XformOpType::TranslateZ, precision, op_suffix, is_inverse_op)
    }

    /// Add a translate operation.
    pub fn add_translate_op(
        &self,
        precision: XformOpPrecision,
        op_suffix: Option<&Token>,
        is_inverse_op: bool,
    ) -> XformOp {
        self.add_xform_op(XformOpType::Translate, precision, op_suffix, is_inverse_op)
    }

    /// Add a scale operation about the X-axis.
    pub fn add_scale_x_op(
        &self,
        precision: XformOpPrecision,
        op_suffix: Option<&Token>,
        is_inverse_op: bool,
    ) -> XformOp {
        self.add_xform_op(XformOpType::ScaleX, precision, op_suffix, is_inverse_op)
    }

    /// Add a scale operation about the Y-axis.
    pub fn add_scale_y_op(
        &self,
        precision: XformOpPrecision,
        op_suffix: Option<&Token>,
        is_inverse_op: bool,
    ) -> XformOp {
        self.add_xform_op(XformOpType::ScaleY, precision, op_suffix, is_inverse_op)
    }

    /// Add a scale operation about the Z-axis.
    pub fn add_scale_z_op(
        &self,
        precision: XformOpPrecision,
        op_suffix: Option<&Token>,
        is_inverse_op: bool,
    ) -> XformOp {
        self.add_xform_op(XformOpType::ScaleZ, precision, op_suffix, is_inverse_op)
    }

    /// Add a scale operation.
    pub fn add_scale_op(
        &self,
        precision: XformOpPrecision,
        op_suffix: Option<&Token>,
        is_inverse_op: bool,
    ) -> XformOp {
        self.add_xform_op(XformOpType::Scale, precision, op_suffix, is_inverse_op)
    }

    /// Add a rotation about the X-axis.
    pub fn add_rotate_x_op(
        &self,
        precision: XformOpPrecision,
        op_suffix: Option<&Token>,
        is_inverse_op: bool,
    ) -> XformOp {
        self.add_xform_op(XformOpType::RotateX, precision, op_suffix, is_inverse_op)
    }

    /// Add a rotation about the Y-axis.
    pub fn add_rotate_y_op(
        &self,
        precision: XformOpPrecision,
        op_suffix: Option<&Token>,
        is_inverse_op: bool,
    ) -> XformOp {
        self.add_xform_op(XformOpType::RotateY, precision, op_suffix, is_inverse_op)
    }

    /// Add a rotation about the Z-axis.
    pub fn add_rotate_z_op(
        &self,
        precision: XformOpPrecision,
        op_suffix: Option<&Token>,
        is_inverse_op: bool,
    ) -> XformOp {
        self.add_xform_op(XformOpType::RotateZ, precision, op_suffix, is_inverse_op)
    }

    /// Add a rotation op with XYZ rotation order.
    pub fn add_rotate_xyz_op(
        &self,
        precision: XformOpPrecision,
        op_suffix: Option<&Token>,
        is_inverse_op: bool,
    ) -> XformOp {
        self.add_xform_op(XformOpType::RotateXYZ, precision, op_suffix, is_inverse_op)
    }

    /// Add a rotation op with XZY rotation order.
    pub fn add_rotate_xzy_op(
        &self,
        precision: XformOpPrecision,
        op_suffix: Option<&Token>,
        is_inverse_op: bool,
    ) -> XformOp {
        self.add_xform_op(XformOpType::RotateXZY, precision, op_suffix, is_inverse_op)
    }

    /// Add a rotation op with YXZ rotation order.
    pub fn add_rotate_yxz_op(
        &self,
        precision: XformOpPrecision,
        op_suffix: Option<&Token>,
        is_inverse_op: bool,
    ) -> XformOp {
        self.add_xform_op(XformOpType::RotateYXZ, precision, op_suffix, is_inverse_op)
    }

    /// Add a rotation op with YZX rotation order.
    pub fn add_rotate_yzx_op(
        &self,
        precision: XformOpPrecision,
        op_suffix: Option<&Token>,
        is_inverse_op: bool,
    ) -> XformOp {
        self.add_xform_op(XformOpType::RotateYZX, precision, op_suffix, is_inverse_op)
    }

    /// Add a rotation op with ZXY rotation order.
    pub fn add_rotate_zxy_op(
        &self,
        precision: XformOpPrecision,
        op_suffix: Option<&Token>,
        is_inverse_op: bool,
    ) -> XformOp {
        self.add_xform_op(XformOpType::RotateZXY, precision, op_suffix, is_inverse_op)
    }

    /// Add a rotation op with ZYX rotation order.
    pub fn add_rotate_zyx_op(
        &self,
        precision: XformOpPrecision,
        op_suffix: Option<&Token>,
        is_inverse_op: bool,
    ) -> XformOp {
        self.add_xform_op(XformOpType::RotateZYX, precision, op_suffix, is_inverse_op)
    }

    /// Add an orient operation (quaternion).
    pub fn add_orient_op(
        &self,
        precision: XformOpPrecision,
        op_suffix: Option<&Token>,
        is_inverse_op: bool,
    ) -> XformOp {
        self.add_xform_op(XformOpType::Orient, precision, op_suffix, is_inverse_op)
    }

    /// Add a transform operation (4x4 matrix).
    pub fn add_transform_op(
        &self,
        precision: XformOpPrecision,
        op_suffix: Option<&Token>,
        is_inverse_op: bool,
    ) -> XformOp {
        self.add_xform_op(XformOpType::Transform, precision, op_suffix, is_inverse_op)
    }

    // ========================================================================
    // Convenience Methods: Get*Op
    // ========================================================================

    /// Get a translation about the X-axis.
    pub fn get_translate_x_op(&self, op_suffix: Option<&Token>, is_inverse_op: bool) -> XformOp {
        self.get_xform_op(XformOpType::TranslateX, op_suffix, is_inverse_op)
    }

    /// Get a translation about the Y-axis.
    pub fn get_translate_y_op(&self, op_suffix: Option<&Token>, is_inverse_op: bool) -> XformOp {
        self.get_xform_op(XformOpType::TranslateY, op_suffix, is_inverse_op)
    }

    /// Get a translation about the Z-axis.
    pub fn get_translate_z_op(&self, op_suffix: Option<&Token>, is_inverse_op: bool) -> XformOp {
        self.get_xform_op(XformOpType::TranslateZ, op_suffix, is_inverse_op)
    }

    /// Get a translate operation.
    pub fn get_translate_op(&self, op_suffix: Option<&Token>, is_inverse_op: bool) -> XformOp {
        self.get_xform_op(XformOpType::Translate, op_suffix, is_inverse_op)
    }

    /// Get a scale operation about the X-axis.
    pub fn get_scale_x_op(&self, op_suffix: Option<&Token>, is_inverse_op: bool) -> XformOp {
        self.get_xform_op(XformOpType::ScaleX, op_suffix, is_inverse_op)
    }

    /// Get a scale operation about the Y-axis.
    pub fn get_scale_y_op(&self, op_suffix: Option<&Token>, is_inverse_op: bool) -> XformOp {
        self.get_xform_op(XformOpType::ScaleY, op_suffix, is_inverse_op)
    }

    /// Get a scale operation about the Z-axis.
    pub fn get_scale_z_op(&self, op_suffix: Option<&Token>, is_inverse_op: bool) -> XformOp {
        self.get_xform_op(XformOpType::ScaleZ, op_suffix, is_inverse_op)
    }

    /// Get a scale operation.
    pub fn get_scale_op(&self, op_suffix: Option<&Token>, is_inverse_op: bool) -> XformOp {
        self.get_xform_op(XformOpType::Scale, op_suffix, is_inverse_op)
    }

    /// Get a rotation about the X-axis.
    pub fn get_rotate_x_op(&self, op_suffix: Option<&Token>, is_inverse_op: bool) -> XformOp {
        self.get_xform_op(XformOpType::RotateX, op_suffix, is_inverse_op)
    }

    /// Get a rotation about the Y-axis.
    pub fn get_rotate_y_op(&self, op_suffix: Option<&Token>, is_inverse_op: bool) -> XformOp {
        self.get_xform_op(XformOpType::RotateY, op_suffix, is_inverse_op)
    }

    /// Get a rotation about the Z-axis.
    pub fn get_rotate_z_op(&self, op_suffix: Option<&Token>, is_inverse_op: bool) -> XformOp {
        self.get_xform_op(XformOpType::RotateZ, op_suffix, is_inverse_op)
    }

    /// Get a rotation op with XYZ rotation order.
    pub fn get_rotate_xyz_op(&self, op_suffix: Option<&Token>, is_inverse_op: bool) -> XformOp {
        self.get_xform_op(XformOpType::RotateXYZ, op_suffix, is_inverse_op)
    }

    /// Get a rotation op with XZY rotation order.
    pub fn get_rotate_xzy_op(&self, op_suffix: Option<&Token>, is_inverse_op: bool) -> XformOp {
        self.get_xform_op(XformOpType::RotateXZY, op_suffix, is_inverse_op)
    }

    /// Get a rotation op with YXZ rotation order.
    pub fn get_rotate_yxz_op(&self, op_suffix: Option<&Token>, is_inverse_op: bool) -> XformOp {
        self.get_xform_op(XformOpType::RotateYXZ, op_suffix, is_inverse_op)
    }

    /// Get a rotation op with YZX rotation order.
    pub fn get_rotate_yzx_op(&self, op_suffix: Option<&Token>, is_inverse_op: bool) -> XformOp {
        self.get_xform_op(XformOpType::RotateYZX, op_suffix, is_inverse_op)
    }

    /// Get a rotation op with ZXY rotation order.
    pub fn get_rotate_zxy_op(&self, op_suffix: Option<&Token>, is_inverse_op: bool) -> XformOp {
        self.get_xform_op(XformOpType::RotateZXY, op_suffix, is_inverse_op)
    }

    /// Get a rotation op with ZYX rotation order.
    pub fn get_rotate_zyx_op(&self, op_suffix: Option<&Token>, is_inverse_op: bool) -> XformOp {
        self.get_xform_op(XformOpType::RotateZYX, op_suffix, is_inverse_op)
    }

    /// Get an orient operation (quaternion).
    pub fn get_orient_op(&self, op_suffix: Option<&Token>, is_inverse_op: bool) -> XformOp {
        self.get_xform_op(XformOpType::Orient, op_suffix, is_inverse_op)
    }

    /// Get a transform operation (4x4 matrix).
    pub fn get_transform_op(&self, op_suffix: Option<&Token>, is_inverse_op: bool) -> XformOp {
        self.get_xform_op(XformOpType::Transform, op_suffix, is_inverse_op)
    }

    /// Clear the xformOpOrder attribute, removing all transform operations.
    ///
    /// Matches C++ `ClearXformOpOrder()`.
    pub fn clear_xform_op_order(&self) -> bool {
        self.set_xform_op_order(&[])
    }

    /// Clear the xformOpOrder and add a single transform op (4x4 matrix).
    ///
    /// Ref: `usd-refs/OpenUSD/pxr/usd/usdGeom/xformable.cpp`
    /// `UsdGeomXformable::MakeMatrixXform()` — after `ClearXformOpOrder()`, fails if
    /// `GetOrderedXformOps` is non-empty (not the raw token array alone).
    pub fn make_matrix_xform(&self) -> XformOp {
        if !self.clear_xform_op_order() {
            return XformOp::invalid();
        }

        let mut resets_xform_stack = false;
        let ordered_ops = self.get_ordered_xform_ops_with_reset(&mut resets_xform_stack);
        if !ordered_ops.is_empty() {
            return XformOp::invalid();
        }

        self.add_transform_op(XformOpPrecision::Double, None, false)
    }

    /// Returns whether the xform value might change over time.
    ///
    /// Matches C++ `TransformMightBeTimeVarying()`.
    pub fn transform_might_be_time_varying(&self) -> bool {
        let mut resets_xform_stack = false;
        let ordered_ops = self.get_ordered_xform_ops_with_reset(&mut resets_xform_stack);
        Self::transform_might_be_time_varying_from_ops(&ordered_ops)
    }

    /// Returns whether the xform value might change over time, using a pre-fetched list of ops.
    ///
    /// Matches C++ `TransformMightBeTimeVarying(const std::vector<UsdGeomXformOp> &ops)`.
    pub fn transform_might_be_time_varying_from_ops(ordered_ops: &[XformOp]) -> bool {
        if ordered_ops.is_empty() {
            return false;
        }

        // Check if any op is time-varying
        for op in ordered_ops {
            if op.might_be_time_varying() {
                return true;
            }
        }

        false
    }

    /// Get all time samples at which xformOps have been authored.
    ///
    /// Matches C++ `GetTimeSamples()`.
    pub fn get_time_samples(&self) -> Vec<f64> {
        let mut resets_xform_stack = false;
        let ordered_ops = self.get_ordered_xform_ops_with_reset(&mut resets_xform_stack);
        Self::get_time_samples_from_ops(&ordered_ops)
    }

    /// Static method: Get time samples from ordered ops.
    ///
    /// Matches C++ `GetTimeSamples(const std::vector<UsdGeomXformOp> &orderedXformOps, std::vector<double> *times)`.
    pub fn get_time_samples_static(ordered_ops: &[XformOp]) -> Vec<f64> {
        Self::get_time_samples_from_ops(ordered_ops)
    }

    /// Get time samples in the given interval.
    ///
    /// Matches C++ `GetTimeSamplesInInterval()`.
    pub fn get_time_samples_in_interval(&self, interval: &Interval) -> Vec<f64> {
        let mut resets_xform_stack = false;
        let ordered_ops = self.get_ordered_xform_ops_with_reset(&mut resets_xform_stack);
        Self::get_time_samples_in_interval_from_ops(&ordered_ops, interval)
    }

    /// Static method: Get time samples in interval from ordered ops.
    ///
    /// Matches C++ `GetTimeSamplesInInterval(const std::vector<UsdGeomXformOp> &orderedXformOps, const GfInterval &interval, std::vector<double> *times)`.
    pub fn get_time_samples_in_interval_static(
        ordered_ops: &[XformOp],
        interval: &Interval,
    ) -> Vec<f64> {
        Self::get_time_samples_in_interval_from_ops(ordered_ops, interval)
    }

    /// Static helper: Get time samples from ordered ops.
    ///
    /// Matches C++ `GetTimeSamples(const std::vector<UsdGeomXformOp> &orderedXformOps, std::vector<double> *times)`.
    pub fn get_time_samples_from_ops(ordered_ops: &[XformOp]) -> Vec<f64> {
        if ordered_ops.len() == 1 {
            return ordered_ops[0].get_time_samples();
        }

        let attrs: Vec<Attribute> = ordered_ops.iter().map(|op| op.attr().clone()).collect();
        Attribute::get_unioned_time_samples(&attrs)
    }

    /// Static helper: Get time samples in interval from ordered ops.
    ///
    /// Matches C++ `GetTimeSamplesInInterval(const std::vector<UsdGeomXformOp> &orderedXformOps, const GfInterval &interval, std::vector<double> *times)`.
    pub fn get_time_samples_in_interval_from_ops(
        ordered_ops: &[XformOp],
        interval: &Interval,
    ) -> Vec<f64> {
        if ordered_ops.len() == 1 {
            return ordered_ops[0].get_time_samples_in_interval(interval);
        }

        let attrs: Vec<Attribute> = ordered_ops.iter().map(|op| op.attr().clone()).collect();
        Attribute::get_unioned_time_samples_in_interval(
            &attrs,
            interval.get_min(),
            interval.get_max(),
        )
    }

    /// Return a Xformable holding the prim adhering to this schema at path on stage.
    ///
    /// Matches C++ `Get(const UsdStagePtr &stage, const SdfPath &path)`.
    pub fn get(stage: &usd_core::Stage, path: &usd_sdf::Path) -> Self {
        if let Some(prim) = stage.get_prim_at_path(path) {
            Self::new(prim)
        } else {
            Self::invalid()
        }
    }

    /// Returns true if the attribute named attrName could affect the local transformation.
    ///
    /// Matches C++ `IsTransformationAffectedByAttrNamed()`.
    pub fn is_transformation_affected_by_attr_named(attr_name: &Token) -> bool {
        *attr_name == usd_geom_tokens().xform_op_order || XformOp::is_xform_op(attr_name)
    }
}

// ============================================================================
// XformQuery
// ============================================================================

/// Helper class that caches the ordered vector of XformOps that
/// contribute to the local transformation of an xformable prim.
///
/// Matches C++ `UsdGeomXformable::XformQuery`.
#[derive(Clone)]
pub struct XformQuery {
    /// Cached copy of the vector of ordered xform ops.
    xform_ops: Vec<XformOp>,
    /// Cache whether the xformable has !resetsXformStack! in its xformOpOrder.
    resets_xform_stack: bool,
}

impl XformQuery {
    /// Creates an empty XformQuery.
    pub fn new() -> Self {
        Self {
            xform_ops: Vec::new(),
            resets_xform_stack: false,
        }
    }

    /// Constructs an XformQuery object for the given xformable prim.
    ///
    /// Matches C++ `XformQuery(const UsdGeomXformable &xformable)`.
    pub fn from_xformable(xformable: &Xformable) -> Self {
        let debug_stats = debug_time_dirty_enabled();
        let started = debug_stats.then(std::time::Instant::now);
        if debug_stats {
            DEBUG_XFORM_QUERY_CALLS.fetch_add(1, Ordering::Relaxed);
        }
        let mut resets_xform_stack = false;
        // Use non-query XformOps (matches `get_ordered_xform_ops_with_reset`): `AttributeQuery::get`
        // at `UsdTimeCode::Default()` can return `None` for authored default values when resolve
        // info is classified as time-sampled, which makes `get_op_transform` identity and breaks
        // `UsdGeomXformCache` / `BBoxCache` (see Python `GetLocalTransformation` / point bounds).
        let xform_ops =
            xformable.get_ordered_xform_ops_with_reset_impl(&mut resets_xform_stack, false);
        let result = Self {
            xform_ops,
            resets_xform_stack,
        };
        if debug_stats {
            if let Some(started) = started {
                DEBUG_XFORM_QUERY_TOTAL_NS
                    .fetch_add(started.elapsed().as_nanos() as u64, Ordering::Relaxed);
            }
        }
        result
    }

    /// Utilizes the internally cached ops to efficiently compute the transform
    /// value at the given time.
    ///
    /// Matches C++ `GetLocalTransformation()`.
    pub fn get_local_transformation(&self, time: TimeCode) -> Option<Matrix4d> {
        let debug_stats = debug_time_dirty_enabled();
        let started = debug_stats.then(std::time::Instant::now);
        if debug_stats {
            DEBUG_XFORM_QUERY_LOCAL_XFORM_CALLS.fetch_add(1, Ordering::Relaxed);
        }
        let transform = Xformable::get_local_transformation_from_ops(&self.xform_ops, time);
        if debug_stats {
            if let Some(started) = started {
                DEBUG_XFORM_QUERY_LOCAL_XFORM_TOTAL_NS
                    .fetch_add(started.elapsed().as_nanos() as u64, Ordering::Relaxed);
            }
        }
        Some(transform)
    }

    /// Returns whether the xformable resets its parent's transformation.
    pub fn get_reset_xform_stack(&self) -> bool {
        self.resets_xform_stack
    }

    /// Returns whether the xform value might change over time.
    ///
    /// Matches C++ `TransformMightBeTimeVarying()`.
    pub fn transform_might_be_time_varying(&self) -> bool {
        self.transform_might_have_effect()
            && Xformable::transform_might_be_time_varying_from_ops(&self.xform_ops)
    }

    /// Returns whether xformOpOrder is non-empty.
    ///
    /// Matches C++ `HasNonEmptyXformOpOrder()`.
    pub fn has_non_empty_xform_op_order(&self) -> bool {
        !self.xform_ops.is_empty()
    }

    /// Returns whether the authored local transform can affect the result.
    ///
    /// Matches C++ `TransformMightHaveEffect()`.
    pub fn transform_might_have_effect(&self) -> bool {
        if self.resets_xform_stack {
            return true;
        }
        if self.xform_ops.is_empty() {
            return false;
        }
        if self.xform_ops.len() == 2
            && self.xform_ops[0].op_type() == self.xform_ops[1].op_type()
            && self.xform_ops[0].is_inverse_op() != self.xform_ops[1].is_inverse_op()
            && self.xform_ops[0].op_name() == self.xform_ops[1].op_name()
        {
            return false;
        }
        true
    }

    /// Sets the vector of times at which xformOp samples have been authored.
    ///
    /// Matches C++ `GetTimeSamples()`.
    pub fn get_time_samples(&self) -> Vec<f64> {
        if self.xform_ops.len() == 1 {
            return self.xform_ops[0].get_time_samples();
        }

        let attr_queries: Vec<AttributeQuery> = self
            .xform_ops
            .iter()
            .filter_map(|op| op.attr_query().cloned())
            .collect();
        if attr_queries.len() == self.xform_ops.len() {
            return AttributeQuery::get_unioned_time_samples(&attr_queries);
        }

        Xformable::get_time_samples_from_ops(&self.xform_ops)
    }

    /// Sets the vector of times in the interval at which xformOp samples have been authored.
    ///
    /// Matches C++ `GetTimeSamplesInInterval()`.
    pub fn get_time_samples_in_interval(&self, interval: &Interval) -> Vec<f64> {
        if self.xform_ops.len() == 1 {
            return self.xform_ops[0].get_time_samples_in_interval(interval);
        }

        let attr_queries: Vec<AttributeQuery> = self
            .xform_ops
            .iter()
            .filter_map(|op| op.attr_query().cloned())
            .collect();
        if attr_queries.len() == self.xform_ops.len() {
            return AttributeQuery::get_unioned_time_samples_in_interval(&attr_queries, interval);
        }

        Xformable::get_time_samples_in_interval_from_ops(&self.xform_ops, interval)
    }

    /// Returns whether the given attribute affects the local transformation.
    ///
    /// Matches C++ `IsAttributeIncludedInLocalTransform()`.
    pub fn is_attribute_included_in_local_transform(&self, attr_name: &Token) -> bool {
        self.xform_ops
            .iter()
            .any(|op| op.attr().name() == *attr_name)
    }
}

impl Default for XformQuery {
    fn default() -> Self {
        Self::new()
    }
}

impl PartialEq for Xformable {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl Eq for Xformable {}

#[cfg(test)]
mod tests {
    use usd_gf::Matrix4d;
    use usd_gf::vec3::Vec3d;

    /// Helper: create translation matrix
    fn make_translate(x: f64, y: f64, z: f64) -> Matrix4d {
        let mut m = Matrix4d::identity();
        m.set_translate(&Vec3d::new(x, y, z));
        m
    }

    /// Helper: create rotation matrix around Z axis (angle in radians)
    fn make_rotate_z(radians: f64) -> Matrix4d {
        Matrix4d::from_rotation(Vec3d::new(0.0, 0.0, 1.0), radians)
    }

    /// Helper: create uniform scale matrix
    fn make_scale(sx: f64, sy: f64, sz: f64) -> Matrix4d {
        Matrix4d::from_scale_vec(&Vec3d::new(sx, sy, sz))
    }

    /// Verify that for non-commuting ops, T*R != R*T.
    /// C++ iterates reverse: [T, R] -> builds R * T.
    /// Our old code iterated forward: [T, R] -> built T * R (WRONG).
    #[test]
    fn test_reverse_vs_forward_order() {
        let t = make_translate(1.0, 2.0, 3.0);
        let r = make_rotate_z(std::f64::consts::FRAC_PI_2); // 90 degrees

        // Forward (WRONG): T * R
        let forward = t * r;
        // Reverse (CORRECT, C++): R * T
        let reverse = r * t;

        // These must differ for non-commuting ops
        let diff = (forward[3][0] - reverse[3][0]).abs() + (forward[3][1] - reverse[3][1]).abs();
        assert!(diff > 0.1, "T*R vs R*T must differ: diff={}", diff);
    }

    /// Verify pivot + !invert!pivot cancellation gives identity.
    /// BMW X3 body_all: [translate(0,0,-4), pivot(-2.42,1.56,4), !invert!pivot]
    /// Reverse iteration with inverse skip:
    ///   !invert!pivot + pivot = skip both -> only translate remains.
    #[test]
    fn test_pivot_inverse_cancellation() {
        let pivot = make_translate(-2.42, 1.56, 4.0);
        let inv_pivot = make_translate(2.42, -1.56, -4.0);
        let product = pivot * inv_pivot;

        // pivot * inv_pivot should be identity
        for r in 0..4 {
            for c in 0..4 {
                let expected = if r == c { 1.0 } else { 0.0 };
                assert!(
                    (product[r][c] - expected).abs() < 1e-10,
                    "pivot * inv_pivot should be identity at ({},{}): got {}",
                    r,
                    c,
                    product[r][c]
                );
            }
        }

        // Full reverse with all 3 ops should equal just translate
        let translate = make_translate(0.0, 0.0, -4.0);
        let full_reverse = inv_pivot * pivot * translate;
        for r in 0..4 {
            for c in 0..4 {
                assert!(
                    (full_reverse[r][c] - translate[r][c]).abs() < 1e-10,
                    "full_reverse should equal translate at ({},{})",
                    r,
                    c
                );
            }
        }
    }

    /// Verify SRT ordering: xformOpOrder = [T, R, S] should produce S * R * T.
    #[test]
    fn test_srt_matrix_order() {
        let t = make_translate(10.0, 0.0, 0.0);
        let r = make_rotate_z(std::f64::consts::FRAC_PI_4); // 45 degrees
        let s = make_scale(2.0, 2.0, 2.0);

        // C++ reverse iteration over [T, R, S] builds: S * R * T
        let correct = s * r * t;
        // Old forward iteration would build: T * R * S (WRONG)
        let wrong = t * r * s;

        // They must differ
        let diff = (correct[3][0] - wrong[3][0]).abs();
        assert!(
            diff > 0.01,
            "SRT vs TRS must differ in translation: diff={}",
            diff
        );
    }

    /// Diagnostic: compare xform data for flo.usda vs flo.usdc through the full Stage API.
    ///
    /// Run with: cargo test --release -p usd-geom -- diag_compare_xform --nocapture
    #[test]
    #[ignore = "diagnostic test requires local flo.usd sample assets"]
    fn diag_compare_xform() {
        use usd_gf::Matrix4d;

        // Register USDA/USDC file formats before opening stages.
        usd_sdf::init();

        let usda_path = "C:/projects/projects.rust.cg/usd-rs/data/flo.usda";
        let usdc_path = "C:/projects/projects.rust.cg/usd-rs/data/flo.usdc";

        /// Collect first `limit` Mesh prims: (path, [(op_name, op_matrix)], local_to_world)
        fn collect_mesh_xforms(
            file_path: &str,
            label: &str,
            limit: usize,
        ) -> Vec<(String, Vec<(String, Matrix4d)>, Matrix4d)> {
            use crate::xform_cache::XformCache;
            use crate::xformable::Xformable;

            let stage = usd_core::Stage::open(file_path, usd_core::common::InitialLoadSet::LoadAll)
                .unwrap_or_else(|e| panic!("[{}] failed to open '{}': {:?}", label, file_path, e));

            let pred = usd_core::prim_flags::default_predicate().into_predicate();
            let all_prims = stage.traverse_vec(pred);

            let time = usd_sdf::TimeCode::default();
            let mut results = Vec::new();

            for prim in &all_prims {
                if prim.type_name() != "Mesh" {
                    continue;
                }
                if results.len() >= limit {
                    break;
                }

                let prim_path = prim.path().get_string().to_string();
                let xformable = Xformable::new(prim.clone());

                // Collect per-op (name, matrix) pairs
                let ops = xformable.get_ordered_xform_ops();
                let op_info: Vec<(String, Matrix4d)> = ops
                    .iter()
                    .map(|op| (op.op_name().as_str().to_string(), op.get_op_transform(time)))
                    .collect();

                // Full local-to-world transform via XformCache
                let mut cache = XformCache::new(time);
                let l2w = cache.get_local_to_world_transform(prim);

                results.push((prim_path, op_info, l2w));
            }
            results
        }

        let usda_data = collect_mesh_xforms(usda_path, "USDA", 10);
        let usdc_data = collect_mesh_xforms(usdc_path, "USDC", 10);

        println!("\n=== USDA vs USDC xform diagnostic ===");
        println!(
            "USDA mesh count: {}, USDC mesh count: {}",
            usda_data.len(),
            usdc_data.len()
        );

        let n = usda_data.len().min(usdc_data.len());
        let mut any_diff = false;

        for i in 0..n {
            let (usda_prim, usda_ops, usda_l2w) = &usda_data[i];
            let (usdc_prim, usdc_ops, usdc_l2w) = &usdc_data[i];

            println!("\n--- Mesh #{} ---", i);
            println!("  USDA prim: {}", usda_prim);
            println!("  USDC prim: {}", usdc_prim);

            // Print per-op info for both formats
            println!("  USDA ops ({}):", usda_ops.len());
            for (name, m) in usda_ops {
                let t = m[3]; // translation row (row-major: row 3)
                println!("    {:40}  t=({:.4},{:.4},{:.4})", name, t[0], t[1], t[2]);
            }
            println!("  USDC ops ({}):", usdc_ops.len());
            for (name, m) in usdc_ops {
                let t = m[3];
                println!("    {:40}  t=({:.4},{:.4},{:.4})", name, t[0], t[1], t[2]);
            }

            // Local-to-world translation rows
            let ua = usda_l2w[3];
            let uc = usdc_l2w[3];
            println!(
                "  USDA L2W translate: ({:.4}, {:.4}, {:.4})",
                ua[0], ua[1], ua[2]
            );
            println!(
                "  USDC L2W translate: ({:.4}, {:.4}, {:.4})",
                uc[0], uc[1], uc[2]
            );

            // Compare op counts
            if usda_ops.len() != usdc_ops.len() {
                println!(
                    "  [DIFF] op count mismatch: USDA={} USDC={}",
                    usda_ops.len(),
                    usdc_ops.len()
                );
                any_diff = true;
            }

            // Compare per-op names and matrices
            for (j, ((ua_name, ua_m), (uc_name, uc_m))) in
                usda_ops.iter().zip(usdc_ops.iter()).enumerate()
            {
                if ua_name != uc_name {
                    println!(
                        "  [DIFF] op[{}] name: USDA='{}' USDC='{}'",
                        j, ua_name, uc_name
                    );
                    any_diff = true;
                }
                let max_delta: f64 = (0..4)
                    .flat_map(|r| (0..4).map(move |c| (ua_m[r][c] - uc_m[r][c]).abs()))
                    .fold(0.0_f64, f64::max);
                if max_delta > 1e-5 {
                    println!(
                        "  [DIFF] op[{}]='{}' matrix max_delta={:.6}",
                        j, ua_name, max_delta
                    );
                    println!(
                        "    USDA: [{:.4},{:.4},{:.4},{:.4}|{:.4},{:.4},{:.4},{:.4}|{:.4},{:.4},{:.4},{:.4}|{:.4},{:.4},{:.4},{:.4}]",
                        ua_m[0][0],
                        ua_m[0][1],
                        ua_m[0][2],
                        ua_m[0][3],
                        ua_m[1][0],
                        ua_m[1][1],
                        ua_m[1][2],
                        ua_m[1][3],
                        ua_m[2][0],
                        ua_m[2][1],
                        ua_m[2][2],
                        ua_m[2][3],
                        ua_m[3][0],
                        ua_m[3][1],
                        ua_m[3][2],
                        ua_m[3][3]
                    );
                    println!(
                        "    USDC: [{:.4},{:.4},{:.4},{:.4}|{:.4},{:.4},{:.4},{:.4}|{:.4},{:.4},{:.4},{:.4}|{:.4},{:.4},{:.4},{:.4}]",
                        uc_m[0][0],
                        uc_m[0][1],
                        uc_m[0][2],
                        uc_m[0][3],
                        uc_m[1][0],
                        uc_m[1][1],
                        uc_m[1][2],
                        uc_m[1][3],
                        uc_m[2][0],
                        uc_m[2][1],
                        uc_m[2][2],
                        uc_m[2][3],
                        uc_m[3][0],
                        uc_m[3][1],
                        uc_m[3][2],
                        uc_m[3][3]
                    );
                    any_diff = true;
                }
            }

            // Compare local-to-world matrices
            let l2w_delta: f64 = (0..4)
                .flat_map(|r| (0..4).map(move |c| (usda_l2w[r][c] - usdc_l2w[r][c]).abs()))
                .fold(0.0_f64, f64::max);
            if l2w_delta > 1e-5 {
                println!("  [DIFF] L2W matrix max_delta={:.6}", l2w_delta);
                println!(
                    "    USDA L2W: [{:.4},{:.4},{:.4},{:.4}|{:.4},{:.4},{:.4},{:.4}|{:.4},{:.4},{:.4},{:.4}|{:.4},{:.4},{:.4},{:.4}]",
                    usda_l2w[0][0],
                    usda_l2w[0][1],
                    usda_l2w[0][2],
                    usda_l2w[0][3],
                    usda_l2w[1][0],
                    usda_l2w[1][1],
                    usda_l2w[1][2],
                    usda_l2w[1][3],
                    usda_l2w[2][0],
                    usda_l2w[2][1],
                    usda_l2w[2][2],
                    usda_l2w[2][3],
                    usda_l2w[3][0],
                    usda_l2w[3][1],
                    usda_l2w[3][2],
                    usda_l2w[3][3]
                );
                println!(
                    "    USDC L2W: [{:.4},{:.4},{:.4},{:.4}|{:.4},{:.4},{:.4},{:.4}|{:.4},{:.4},{:.4},{:.4}|{:.4},{:.4},{:.4},{:.4}]",
                    usdc_l2w[0][0],
                    usdc_l2w[0][1],
                    usdc_l2w[0][2],
                    usdc_l2w[0][3],
                    usdc_l2w[1][0],
                    usdc_l2w[1][1],
                    usdc_l2w[1][2],
                    usdc_l2w[1][3],
                    usdc_l2w[2][0],
                    usdc_l2w[2][1],
                    usdc_l2w[2][2],
                    usdc_l2w[2][3],
                    usdc_l2w[3][0],
                    usdc_l2w[3][1],
                    usdc_l2w[3][2],
                    usdc_l2w[3][3]
                );
                any_diff = true;
            } else {
                println!("  [OK] L2W matrices match (max_delta={:.6})", l2w_delta);
            }
        }

        println!();
        if any_diff {
            println!("[RESULT] Differences found between USDA and USDC xforms.");
        } else {
            println!(
                "[RESULT] All {} mesh xforms match between USDA and USDC.",
                n
            );
        }
        // Diagnostic only — no assertion, just print findings.
    }

    /// Diagnostic phase 2: compare Xform prims (not just Mesh) across hierarchy.
    /// Looks at first 15 xformable prims in both USDA and USDC and prints ops + L2W.
    ///
    /// Run with: cargo test --release -p usd-geom -- diag_xform_hierarchy --nocapture
    #[test]
    #[ignore = "diagnostic test requires local flo.usd sample assets"]
    fn diag_xform_hierarchy() {
        
        

        usd_sdf::init();

        let usda_path = "C:/projects/projects.rust.cg/usd-rs/data/flo.usda";
        let usdc_path = "C:/projects/projects.rust.cg/usd-rs/data/flo.usdc";
        let time = usd_sdf::TimeCode::default();

        // Print xform info for first `limit` prims that have xformOpOrder or are Xform type
        fn print_xforms(file_path: &str, label: &str, limit: usize) {
            use crate::xform_cache::XformCache;
            use crate::xformable::Xformable;

            let stage = usd_core::Stage::open(file_path, usd_core::common::InitialLoadSet::LoadAll)
                .unwrap_or_else(|e| panic!("[{}] open failed: {:?}", label, e));

            let pred = usd_core::prim_flags::default_predicate().into_predicate();
            let all_prims = stage.traverse_vec(pred);
            let time = usd_sdf::TimeCode::default();

            println!(
                "\n=== {} ({}) — first {} xformable prims ===",
                label, file_path, limit
            );
            let mut shown = 0;
            for prim in &all_prims {
                if shown >= limit {
                    break;
                }
                let xf = Xformable::new(prim.clone());
                let ops = xf.get_ordered_xform_ops();
                // Also check property names for xformOp: prefix
                let props = prim.get_property_names();
                let xform_props: Vec<&str> = props
                    .iter()
                    .filter(|n| n.as_str().starts_with("xformOp:") || *n == "xformOpOrder")
                    .map(|n| n.as_str())
                    .collect();

                // Only print prims with xform data or that are Xform/Scope types
                let ty = prim.type_name().as_str().to_string();
                let is_xform_type = matches!(ty.as_str(), "Xform" | "Scope" | "Mesh" | "Transform");
                if xform_props.is_empty() && !is_xform_type {
                    continue;
                }

                let mut cache = XformCache::new(time);
                let local = xf.get_local_transformation(time);
                let l2w = cache.get_local_to_world_transform(prim);

                println!(
                    "  {} [{}] ops={} props={:?}",
                    prim.path().get_string(),
                    ty,
                    ops.len(),
                    xform_props
                );
                println!(
                    "    local_t=({:.4},{:.4},{:.4}) l2w_t=({:.4},{:.4},{:.4})",
                    local[3][0], local[3][1], local[3][2], l2w[3][0], l2w[3][1], l2w[3][2]
                );
                for op in &ops {
                    let m = op.get_op_transform(time);
                    println!(
                        "    op={:35} t=({:.4},{:.4},{:.4})",
                        op.op_name().as_str(),
                        m[3][0],
                        m[3][1],
                        m[3][2]
                    );
                }
                shown += 1;
            }
            println!("(showed {}/{} prims)", shown, all_prims.len());
        }

        print_xforms(usda_path, "USDA", 15);
        print_xforms(usdc_path, "USDC", 15);

        // Additionally: compare USDA vs USDC for matching prim paths — first 5 Xform prims
        fn collect_xform_prims(
            file_path: &str,
            label: &str,
            limit: usize,
        ) -> Vec<(String, usize, [f64; 3], [f64; 3])> {
            use crate::xform_cache::XformCache;
            use crate::xformable::Xformable;

            let stage = usd_core::Stage::open(file_path, usd_core::common::InitialLoadSet::LoadAll)
                .unwrap_or_else(|e| panic!("[{}] open failed: {:?}", label, e));

            let pred = usd_core::prim_flags::default_predicate().into_predicate();
            let all_prims = stage.traverse_vec(pred);
            let time = usd_sdf::TimeCode::default();

            let mut results = Vec::new();
            for prim in &all_prims {
                if results.len() >= limit {
                    break;
                }
                let xf = Xformable::new(prim.clone());
                let ops = xf.get_ordered_xform_ops();
                if ops.is_empty() {
                    continue;
                } // skip prims with no ops
                let local = xf.get_local_transformation(time);
                let mut cache = XformCache::new(time);
                let l2w = cache.get_local_to_world_transform(prim);
                results.push((
                    prim.path().get_string().to_string(),
                    ops.len(),
                    [local[3][0], local[3][1], local[3][2]],
                    [l2w[3][0], l2w[3][1], l2w[3][2]],
                ));
            }
            results
        }

        println!("\n=== USDA vs USDC Xform prim comparison (prims with ops) ===");
        let usda_xforms = collect_xform_prims(usda_path, "USDA", 10);
        let usdc_xforms = collect_xform_prims(usdc_path, "USDC", 10);

        let n = usda_xforms.len().min(usdc_xforms.len());
        let mut any_diff = false;
        for i in 0..n {
            let (ua_path, ua_ops, ua_local, ua_l2w) = &usda_xforms[i];
            let (uc_path, uc_ops, uc_local, uc_l2w) = &usdc_xforms[i];
            let path_match = ua_path == uc_path;
            let local_delta = (ua_local[0] - uc_local[0]).abs()
                + (ua_local[1] - uc_local[1]).abs()
                + (ua_local[2] - uc_local[2]).abs();
            let l2w_delta = (ua_l2w[0] - uc_l2w[0]).abs()
                + (ua_l2w[1] - uc_l2w[1]).abs()
                + (ua_l2w[2] - uc_l2w[2]).abs();

            let path_tag = if path_match { "[same]" } else { "[DIFF_PATH]" };
            println!("#{}: {} {}", i, ua_path, path_tag);
            if !path_match {
                println!("  USDC path: {}", uc_path);
            }
            println!(
                "  USDA: ops={} local_t=({:.4},{:.4},{:.4}) l2w_t=({:.4},{:.4},{:.4})",
                ua_ops, ua_local[0], ua_local[1], ua_local[2], ua_l2w[0], ua_l2w[1], ua_l2w[2]
            );
            println!(
                "  USDC: ops={} local_t=({:.4},{:.4},{:.4}) l2w_t=({:.4},{:.4},{:.4})",
                uc_ops, uc_local[0], uc_local[1], uc_local[2], uc_l2w[0], uc_l2w[1], uc_l2w[2]
            );
            if local_delta > 1e-4 {
                println!("  [DIFF] local translation delta={:.6}", local_delta);
                any_diff = true;
            }
            if l2w_delta > 1e-4 {
                println!("  [DIFF] L2W translation delta={:.6}", l2w_delta);
                any_diff = true;
            }
        }
        if any_diff {
            println!("\n[RESULT] Xform differences found.");
        } else {
            println!("\n[RESULT] All {} xform prims match.", n);
        }
        let _ = time; // suppress unused warning
    }

    /// Diagnostic phase 3: compare TimeCode::default() vs TimeCode::new(1.0) for animated prims.
    ///
    /// Run with: cargo test --release -p usd-geom -- diag_timecode --nocapture
    #[test]
    #[ignore = "diagnostic test requires local flo.usd sample assets"]
    fn diag_timecode() {
        usd_sdf::init();

        // Test with USDC (the format that has rendering issues)
        let usdc_path = "C:/projects/projects.rust.cg/usd-rs/data/flo.usdc";
        let usda_path = "C:/projects/projects.rust.cg/usd-rs/data/flo.usda";

        fn print_prim_at_times(file_path: &str, label: &str, prim_path_str: &str) {
            use crate::xform_cache::XformCache;
            use crate::xformable::Xformable;
            use usd_sdf::{Path, TimeCode};

            let stage = usd_core::Stage::open(file_path, usd_core::common::InitialLoadSet::LoadAll)
                .unwrap_or_else(|e| panic!("[{}] open failed: {:?}", label, e));

            let prim_path = Path::from_string(prim_path_str).unwrap();
            let prim = match stage.get_prim_at_path(&prim_path) {
                Some(p) => p,
                None => {
                    println!("[{}] prim not found: {}", label, prim_path_str);
                    return;
                }
            };

            let xf = Xformable::new(prim.clone());
            let ops = xf.get_ordered_xform_ops();
            println!("[{}] {} ops={}:", label, prim_path_str, ops.len());
            for op in &ops {
                let name = op.op_name().as_str().to_string();
                let v_default = op.get(TimeCode::default());
                let v_t1 = op.get(TimeCode::new(1.0));
                println!(
                    "  op={} default={:?} t=1:{:?}",
                    name,
                    v_default.as_ref().map(|v| v.type_name()),
                    v_t1.as_ref().map(|v| v.type_name()),
                );
            }
            // Compare local transform at default vs t=1
            let local_default = xf.get_local_transformation(TimeCode::default());
            let local_t1 = xf.get_local_transformation(TimeCode::new(1.0));
            let mut cache_def = XformCache::new(TimeCode::default());
            let mut cache_t1 = XformCache::new(TimeCode::new(1.0));
            let l2w_default = cache_def.get_local_to_world_transform(&prim);
            let l2w_t1 = cache_t1.get_local_to_world_transform(&prim);
            println!(
                "  local default: t=({:.4},{:.4},{:.4}) r0=({:.4},{:.4},{:.4})",
                local_default[3][0],
                local_default[3][1],
                local_default[3][2],
                local_default[0][0],
                local_default[0][1],
                local_default[0][2]
            );
            println!(
                "  local t=1:     t=({:.4},{:.4},{:.4}) r0=({:.4},{:.4},{:.4})",
                local_t1[3][0],
                local_t1[3][1],
                local_t1[3][2],
                local_t1[0][0],
                local_t1[0][1],
                local_t1[0][2]
            );
            println!(
                "  l2w default:   t=({:.4},{:.4},{:.4})",
                l2w_default[3][0], l2w_default[3][1], l2w_default[3][2]
            );
            println!(
                "  l2w t=1:       t=({:.4},{:.4},{:.4})",
                l2w_t1[3][0], l2w_t1[3][1], l2w_t1[3][2]
            );
        }

        // Check /root/flo/noga_a — has static xform ops
        println!("\n--- /root/flo/noga_a (static xform) ---");
        print_prim_at_times(usda_path, "USDA", "/root/flo/noga_a");
        print_prim_at_times(usdc_path, "USDC", "/root/flo/noga_a");

        // Check /root/flo/noga_a/noga1/noga3_001 — has animated xform ops (timeSamples)
        println!("\n--- /root/flo/noga_a/noga1/noga3_001 (animated xform) ---");
        print_prim_at_times(usda_path, "USDA", "/root/flo/noga_a/noga1/noga3_001");
        print_prim_at_times(usdc_path, "USDC", "/root/flo/noga_a/noga1/noga3_001");

        // Now compare USDA vs USDC at matching prim paths using TimeCode::new(1.0)
        println!("\n--- USDA vs USDC at TimeCode(1.0) for /root/flo/noga_a ---");
        fn compare_at_time(prim_path_str: &str, time: usd_sdf::TimeCode) {
            use crate::xform_cache::XformCache;
            use crate::xformable::Xformable;
            use usd_sdf::Path;

            let usda_path = "C:/projects/projects.rust.cg/usd-rs/data/flo.usda";
            let usdc_path = "C:/projects/projects.rust.cg/usd-rs/data/flo.usdc";

            let open = |path: &str| {
                usd_core::Stage::open(path, usd_core::common::InitialLoadSet::LoadAll).unwrap()
            };
            let usda_stage = open(usda_path);
            let usdc_stage = open(usdc_path);

            let prim_path = Path::from_string(prim_path_str).unwrap();
            let usda_prim = usda_stage.get_prim_at_path(&prim_path).unwrap();
            let usdc_prim = usdc_stage.get_prim_at_path(&prim_path).unwrap();

            let ua = Xformable::new(usda_prim.clone()).get_local_transformation(time);
            let uc = Xformable::new(usdc_prim.clone()).get_local_transformation(time);
            let mut ca = XformCache::new(time);
            let mut cb = XformCache::new(time);
            let ua_l2w = ca.get_local_to_world_transform(&usda_prim);
            let uc_l2w = cb.get_local_to_world_transform(&usdc_prim);

            let local_delta: f64 = (0..4)
                .flat_map(|r| (0..4).map(move |c| (ua[r][c] - uc[r][c]).abs()))
                .fold(0.0_f64, f64::max);
            let l2w_delta: f64 = (0..4)
                .flat_map(|r| (0..4).map(move |c| (ua_l2w[r][c] - uc_l2w[r][c]).abs()))
                .fold(0.0_f64, f64::max);

            println!(
                "  {} @ t={:.1}: local_delta={:.6} l2w_delta={:.6}",
                prim_path_str,
                time.value(),
                local_delta,
                l2w_delta
            );
            if local_delta > 1e-4 {
                println!(
                    "    [DIFF LOCAL] USDA local: r=({:.4},{:.4},{:.4}|{:.4},{:.4},{:.4}|{:.4},{:.4},{:.4}) t=({:.4},{:.4},{:.4})",
                    ua[0][0],
                    ua[0][1],
                    ua[0][2],
                    ua[1][0],
                    ua[1][1],
                    ua[1][2],
                    ua[2][0],
                    ua[2][1],
                    ua[2][2],
                    ua[3][0],
                    ua[3][1],
                    ua[3][2]
                );
                println!(
                    "    [DIFF LOCAL] USDC local: r=({:.4},{:.4},{:.4}|{:.4},{:.4},{:.4}|{:.4},{:.4},{:.4}) t=({:.4},{:.4},{:.4})",
                    uc[0][0],
                    uc[0][1],
                    uc[0][2],
                    uc[1][0],
                    uc[1][1],
                    uc[1][2],
                    uc[2][0],
                    uc[2][1],
                    uc[2][2],
                    uc[3][0],
                    uc[3][1],
                    uc[3][2]
                );
            }
            if l2w_delta > 1e-4 {
                println!("    [DIFF L2W]");
            }
        }

        for t in [0.0, 1.0, 50.0] {
            let tc = usd_sdf::TimeCode::new(t);
            compare_at_time("/root/flo/noga_a", tc);
            compare_at_time("/root/flo/noga_a/noga1/noga3_001", tc);
        }
    }
}
