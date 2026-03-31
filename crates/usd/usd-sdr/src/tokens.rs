//! SDR Tokens - Static token definitions for the Shader Definition Registry.
//!
//! Port of pxr/usd/sdr tokens (shaderNodeMetadata.h, shaderPropertyMetadata.h, shaderNode.h, shaderProperty.h)
//!
//! This module provides all token constants used throughout SDR:
//! - Node metadata tokens (label, category, role, etc.)
//! - Node context tokens (pattern, surface, volume, etc.)
//! - Node role tokens (primvar, texture, field, math)
//! - Property metadata tokens (label, help, page, widget, etc.)
//! - Property type tokens (int, float, color, vector, etc.)
//! - Node field key tokens (identifier, name, family, sourceType)

use once_cell::sync::Lazy;
use usd_tf::Token;

/// Token definitions for SDR node field keys.
/// These are used internally for node data access via `get_data_for_key()`.
#[derive(Debug, Clone)]
pub struct SdrNodeFieldKeyTokens {
    /// Key for node identifier field ("_identifier").
    pub identifier: Token,
    /// Key for node name field ("_name").
    pub name: Token,
    /// Key for node family field ("_family").
    pub family: Token,
    /// Key for node source type field ("_sourceType").
    pub source_type: Token,
}

impl SdrNodeFieldKeyTokens {
    fn new() -> Self {
        Self {
            identifier: Token::new("_identifier"),
            name: Token::new("_name"),
            family: Token::new("_family"),
            source_type: Token::new("_sourceType"),
        }
    }
}

/// Token definitions for SDR node metadata.
/// Metadata keys used in shader node definitions.
#[derive(Debug, Clone)]
pub struct SdrNodeMetadataTokens {
    /// Category grouping for the node ("category").
    pub category: Token,
    /// Role annotation for the node ("role").
    pub role: Token,
    /// Departments associated with the node ("departments").
    pub departments: Token,
    /// Help text for the node ("help").
    pub help: Token,
    /// Display label for the node ("label").
    pub label: Token,
    /// Property page groupings ("pages").
    pub pages: Token,
    /// Pages that are open by default ("openPages").
    pub open_pages: Token,
    /// Conditional visibility expressions for pages ("pagesShownIf").
    pub pages_shown_if: Token,
    /// Required primvars ("primvars").
    pub primvars: Token,
    /// Implementation function name ("__SDR__implementationName").
    pub implementation_name: Token,
    /// Target renderer ("__SDR__target").
    pub target: Token,
    /// USD encoding version ("sdrUsdEncodingVersion").
    pub sdr_usd_encoding_version: Token,
    /// Fallback prefix for definition names ("sdrDefinitionNameFallbackPrefix").
    pub sdr_definition_name_fallback_prefix: Token,
}

impl SdrNodeMetadataTokens {
    fn new() -> Self {
        Self {
            category: Token::new("category"),
            role: Token::new("role"),
            departments: Token::new("departments"),
            help: Token::new("help"),
            label: Token::new("label"),
            pages: Token::new("pages"),
            open_pages: Token::new("openPages"),
            pages_shown_if: Token::new("pagesShownIf"),
            primvars: Token::new("primvars"),
            implementation_name: Token::new("__SDR__implementationName"),
            target: Token::new("__SDR__target"),
            sdr_usd_encoding_version: Token::new("sdrUsdEncodingVersion"),
            sdr_definition_name_fallback_prefix: Token::new("sdrDefinitionNameFallbackPrefix"),
        }
    }
}

/// Token definitions for SDR node context.
/// Contexts describe what role a shader node plays in rendering.
#[derive(Debug, Clone)]
pub struct SdrNodeContextTokens {
    /// Pattern evaluation context ("pattern").
    pub pattern: Token,
    /// Surface BXDF context ("surface").
    pub surface: Token,
    /// Volume shader context ("volume").
    pub volume: Token,
    /// Displacement shader context ("displacement").
    pub displacement: Token,
    /// Light shader context ("light").
    pub light: Token,
    /// Display filter context ("displayFilter").
    pub display_filter: Token,
    /// Light filter context ("lightFilter").
    pub light_filter: Token,
    /// Pixel filter context ("pixelFilter").
    pub pixel_filter: Token,
    /// Sample filter context ("sampleFilter").
    pub sample_filter: Token,
}

impl SdrNodeContextTokens {
    fn new() -> Self {
        Self {
            pattern: Token::new("pattern"),
            surface: Token::new("surface"),
            volume: Token::new("volume"),
            displacement: Token::new("displacement"),
            light: Token::new("light"),
            display_filter: Token::new("displayFilter"),
            light_filter: Token::new("lightFilter"),
            pixel_filter: Token::new("pixelFilter"),
            sample_filter: Token::new("sampleFilter"),
        }
    }
}

/// Token definitions for SDR node roles.
/// Roles annotate the function of a shader within a network.
#[derive(Debug, Clone)]
pub struct SdrNodeRoleTokens {
    /// Reads primvar data ("primvar").
    pub primvar: Token,
    /// Reads texture data ("texture").
    pub texture: Token,
    /// Reads volume field data ("field").
    pub field: Token,
    /// Mathematical operation ("math").
    pub math: Token,
}

impl SdrNodeRoleTokens {
    fn new() -> Self {
        Self {
            primvar: Token::new("primvar"),
            texture: Token::new("texture"),
            field: Token::new("field"),
            math: Token::new("math"),
        }
    }
}

/// Token definitions for SDR property metadata.
/// Metadata keys used in shader property definitions.
#[derive(Debug, Clone)]
pub struct SdrPropertyMetadataTokens {
    /// Display label ("label").
    pub label: Token,
    /// Help text ("help").
    pub help: Token,
    /// UI page grouping ("page").
    pub page: Token,
    /// Render type hint ("renderType").
    pub render_type: Token,
    /// Property role ("role").
    pub role: Token,
    /// UI widget type ("widget").
    pub widget: Token,
    /// UI hints ("hints").
    pub hints: Token,
    /// Enum options ("options").
    pub options: Token,
    /// Dynamic array flag ("isDynamicArray").
    pub is_dynamic_array: Token,
    /// Tuple size for arrays ("tupleSize").
    pub tuple_size: Token,
    /// Connection allowed flag ("connectable").
    pub connectable: Token,
    /// Tag metadata ("tag").
    pub tag: Token,
    /// Conditional visibility expression ("shownIf").
    pub shown_if: Token,
    /// Valid connection type list ("validConnectionTypes").
    pub valid_connection_types: Token,
    /// VStruct parent name ("vstructMemberOf").
    pub vstruct_member_of: Token,
    /// VStruct member name ("vstructMemberName").
    pub vstruct_member_name: Token,
    /// VStruct conditional expression ("vstructConditionalExpr").
    pub vstruct_conditional_expr: Token,
    /// Asset identifier flag ("__SDR__isAssetIdentifier").
    pub is_asset_identifier: Token,
    /// Implementation parameter name ("__SDR__implementationName").
    pub implementation_name: Token,
    /// USD definition type override ("sdrUsdDefinitionType").
    pub sdr_usd_definition_type: Token,
    /// Default input flag ("__SDR__defaultinput").
    pub default_input: Token,
    /// Target renderer ("__SDR__target").
    pub target: Token,
    /// Colorspace annotation ("__SDR__colorspace").
    pub colorspace: Token,
}

impl SdrPropertyMetadataTokens {
    fn new() -> Self {
        Self {
            label: Token::new("label"),
            help: Token::new("help"),
            page: Token::new("page"),
            render_type: Token::new("renderType"),
            role: Token::new("role"),
            widget: Token::new("widget"),
            hints: Token::new("hints"),
            options: Token::new("options"),
            is_dynamic_array: Token::new("isDynamicArray"),
            tuple_size: Token::new("tupleSize"),
            connectable: Token::new("connectable"),
            tag: Token::new("tag"),
            shown_if: Token::new("shownIf"),
            valid_connection_types: Token::new("validConnectionTypes"),
            vstruct_member_of: Token::new("vstructMemberOf"),
            vstruct_member_name: Token::new("vstructMemberName"),
            vstruct_conditional_expr: Token::new("vstructConditionalExpr"),
            is_asset_identifier: Token::new("__SDR__isAssetIdentifier"),
            implementation_name: Token::new("__SDR__implementationName"),
            sdr_usd_definition_type: Token::new("sdrUsdDefinitionType"),
            default_input: Token::new("__SDR__defaultinput"),
            target: Token::new("__SDR__target"),
            colorspace: Token::new("__SDR__colorspace"),
        }
    }
}

/// Token definitions for SDR property roles.
#[derive(Debug, Clone)]
pub struct SdrPropertyRoleTokens {
    /// No specific role ("none").
    pub none: Token,
}

impl SdrPropertyRoleTokens {
    fn new() -> Self {
        Self {
            none: Token::new("none"),
        }
    }
}

/// Token definitions for SDR property type tokens.
/// These define the types that shader properties can have.
#[derive(Debug, Clone)]
pub struct SdrPropertyTypeTokens {
    /// Integer type ("int").
    pub int: Token,
    /// String type ("string").
    pub string: Token,
    /// Float type ("float").
    pub float: Token,
    /// RGB color type ("color").
    pub color: Token,
    /// RGBA color type ("color4").
    pub color4: Token,
    /// 3D point type ("point").
    pub point: Token,
    /// 3D normal type ("normal").
    pub normal: Token,
    /// 3D vector type ("vector").
    pub vector: Token,
    /// 4x4 matrix type ("matrix").
    pub matrix: Token,
    /// Struct composite type ("struct").
    pub struct_type: Token,
    /// Connection-only terminal type ("terminal").
    pub terminal: Token,
    /// Virtual struct type ("vstruct").
    pub vstruct: Token,
    /// Unknown/unsupported type ("unknown").
    pub unknown: Token,
}

impl SdrPropertyTypeTokens {
    fn new() -> Self {
        Self {
            int: Token::new("int"),
            string: Token::new("string"),
            float: Token::new("float"),
            color: Token::new("color"),
            color4: Token::new("color4"),
            point: Token::new("point"),
            normal: Token::new("normal"),
            vector: Token::new("vector"),
            matrix: Token::new("matrix"),
            struct_type: Token::new("struct"),
            terminal: Token::new("terminal"),
            vstruct: Token::new("vstruct"),
            unknown: Token::new("unknown"),
        }
    }
}

/// Token definitions for SDR misc property tokens.
#[derive(Debug, Clone)]
pub struct SdrPropertyTokensStruct {
    /// Page hierarchy delimiter (":").
    pub page_delimiter: Token,
}

impl SdrPropertyTokensStruct {
    fn new() -> Self {
        Self {
            page_delimiter: Token::new(":"),
        }
    }
}

/// All SDR tokens combined for convenient access.
#[derive(Debug, Clone)]
pub struct SdrTokens {
    /// Node field key tokens for data access.
    pub node_field_key: SdrNodeFieldKeyTokens,
    /// Node metadata tokens.
    pub node_metadata: SdrNodeMetadataTokens,
    /// Node context tokens.
    pub node_context: SdrNodeContextTokens,
    /// Node role tokens.
    pub node_role: SdrNodeRoleTokens,
    /// Property metadata tokens.
    pub property_metadata: SdrPropertyMetadataTokens,
    /// Property role tokens.
    pub property_role: SdrPropertyRoleTokens,
    /// Property type tokens.
    pub property_types: SdrPropertyTypeTokens,
    /// Miscellaneous property tokens.
    pub property_tokens: SdrPropertyTokensStruct,
}

impl SdrTokens {
    fn new() -> Self {
        Self {
            node_field_key: SdrNodeFieldKeyTokens::new(),
            node_metadata: SdrNodeMetadataTokens::new(),
            node_context: SdrNodeContextTokens::new(),
            node_role: SdrNodeRoleTokens::new(),
            property_metadata: SdrPropertyMetadataTokens::new(),
            property_role: SdrPropertyRoleTokens::new(),
            property_types: SdrPropertyTypeTokens::new(),
            property_tokens: SdrPropertyTokensStruct::new(),
        }
    }
}

/// Global static instance of SDR tokens.
static SDR_TOKENS: Lazy<SdrTokens> = Lazy::new(SdrTokens::new);

/// Returns reference to the global SDR tokens instance.
pub fn tokens() -> &'static SdrTokens {
    &SDR_TOKENS
}
