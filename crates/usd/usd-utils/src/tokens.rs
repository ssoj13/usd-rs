//! USD Utils token definitions.
//!
//! Defines standard tokens used throughout the usdUtils module.

use once_cell::sync::Lazy;
use usd_tf::Token;

/// Token definitions for UsdUtils module.
#[derive(Debug, Clone)]
pub struct UsdUtilsTokens {
    // Time code range tokens
    /// Token representing an empty time code range ("NONE").
    pub empty_time_code_range: Token,
    /// Token for range separator (":").
    pub range_separator: Token,
    /// Token for stride separator ("x").
    pub stride_separator: Token,

    // Pipeline tokens
    /// Default materials scope name.
    pub materials_scope_name: Token,
    /// Default primary camera name.
    pub primary_camera_name: Token,
    /// Default primary UV set name.
    pub primary_uv_set_name: Token,
    /// Default pref (reference position) name.
    pub pref_name: Token,

    // Stage stats keys
    /// Approximate memory in MB.
    pub approx_memory_in_mb: Token,
    /// Total prim count.
    pub total_prim_count: Token,
    /// Model count.
    pub model_count: Token,
    /// Instanced model count.
    pub instanced_model_count: Token,
    /// Asset count.
    pub asset_count: Token,
    /// Prototype count.
    pub prototype_count: Token,
    /// Total instance count.
    pub total_instance_count: Token,
    /// Used layer count.
    pub used_layer_count: Token,
    /// Primary stats key.
    pub primary: Token,
    /// Prototypes stats key.
    pub prototypes: Token,
    /// Prim counts stats key.
    pub prim_counts: Token,
    /// Active prim count.
    pub active_prim_count: Token,
    /// Inactive prim count.
    pub inactive_prim_count: Token,
    /// Pure over count.
    pub pure_over_count: Token,
    /// Instance count.
    pub instance_count: Token,
    /// Prim counts by type stats key.
    pub prim_counts_by_type: Token,
    /// Untyped prim count.
    pub untyped: Token,

    // Stitch value status tokens
    /// No stitched value.
    pub no_stitched_value: Token,
    /// Use default value.
    pub use_default_value: Token,
    /// Use supplied value.
    pub use_supplied_value: Token,

    // Variant set selection export policy tokens
    /// Never export selection.
    pub never: Token,
    /// Export if authored.
    pub if_authored: Token,
    /// Always export selection.
    pub always: Token,
}

impl UsdUtilsTokens {
    /// Creates a new set of UsdUtils tokens.
    pub fn new() -> Self {
        Self {
            // Time code range tokens
            empty_time_code_range: Token::from("NONE"),
            range_separator: Token::from(":"),
            stride_separator: Token::from("x"),

            // Pipeline tokens
            materials_scope_name: Token::from("Looks"),
            primary_camera_name: Token::from("main_cam"),
            primary_uv_set_name: Token::from("st"),
            pref_name: Token::from("pref"),

            // Stage stats keys
            approx_memory_in_mb: Token::from("approxMemoryInMb"),
            total_prim_count: Token::from("totalPrimCount"),
            model_count: Token::from("modelCount"),
            instanced_model_count: Token::from("instancedModelCount"),
            asset_count: Token::from("assetCount"),
            prototype_count: Token::from("prototypeCount"),
            total_instance_count: Token::from("totalInstanceCount"),
            used_layer_count: Token::from("usedLayerCount"),
            primary: Token::from("primary"),
            prototypes: Token::from("prototypes"),
            prim_counts: Token::from("primCounts"),
            active_prim_count: Token::from("activePrimCount"),
            inactive_prim_count: Token::from("inactivePrimCount"),
            pure_over_count: Token::from("pureOverCount"),
            instance_count: Token::from("instanceCount"),
            prim_counts_by_type: Token::from("primCountsByType"),
            untyped: Token::from("untyped"),

            // Stitch value status tokens
            no_stitched_value: Token::from("NoStitchedValue"),
            use_default_value: Token::from("UseDefaultValue"),
            use_supplied_value: Token::from("UseSuppliedValue"),

            // Variant set selection export policy tokens
            never: Token::from("never"),
            if_authored: Token::from("ifAuthored"),
            always: Token::from("always"),
        }
    }
}

impl Default for UsdUtilsTokens {
    fn default() -> Self {
        Self::new()
    }
}

/// Global singleton for UsdUtils tokens.
pub static TOKENS: Lazy<UsdUtilsTokens> = Lazy::new(UsdUtilsTokens::new);

/// Returns a reference to the global UsdUtils tokens.
pub fn tokens() -> &'static UsdUtilsTokens {
    &TOKENS
}
