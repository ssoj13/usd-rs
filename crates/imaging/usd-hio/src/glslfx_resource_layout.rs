//! HioGlslfxResourceLayout - Shader resource layout parser.
//!
//! Port of pxr/imaging/hio/glslfxResourceLayout.h/cpp
//!
//! Provides an intermediate representation for shader resources (buffers,
//! textures, input/output variables, interface blocks) parsed from GLSLFX
//! resource layout dictionaries.

use super::types::HioFormat;
use usd_tf::Token;
use usd_vt::{Dictionary, Value};

// ============================================================================
// InOut
// ============================================================================

/// Whether a resource element is a shader input, output, or neither.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InOut {
    /// Neither input nor output (e.g., uniform buffer, texture).
    None,
    /// Shader stage input.
    StageIn,
    /// Shader stage output.
    StageOut,
}

// ============================================================================
// Kind
// ============================================================================

/// The kind of resource element.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    /// No kind assigned.
    None,
    /// Single value (in/out variable).
    Value,
    /// Interface block.
    Block,
    /// Layout qualifier (e.g., early_fragment_tests).
    Qualifier,
    /// Uniform value.
    UniformValue,
    /// Uniform block.
    UniformBlock,
    /// Uniform block with constant parameters.
    UniformBlockConstantParams,
    /// Read-only buffer (SSBO).
    BufferReadOnly,
    /// Read-write buffer (SSBO).
    BufferReadWrite,
}

// ============================================================================
// Member
// ============================================================================

/// A member of an aggregate resource element (block/buffer).
#[derive(Debug, Clone)]
pub struct Member {
    /// Data type (e.g., "vec4", "mat4", "float").
    pub data_type: Token,
    /// Member name.
    pub name: Token,
    /// Array size token (empty if not an array).
    pub array_size: Token,
    /// Interpolation/storage qualifiers (e.g., "flat", "centroid").
    pub qualifiers: Token,
}

impl Member {
    /// Create a new member.
    pub fn new(data_type: Token, name: Token) -> Self {
        Self {
            data_type,
            name,
            array_size: Token::empty(),
            qualifiers: Token::empty(),
        }
    }

    /// Create a member with array size.
    pub fn with_array_size(mut self, array_size: Token) -> Self {
        self.array_size = array_size;
        self
    }

    /// Create a member with qualifiers.
    pub fn with_qualifiers(mut self, qualifiers: Token) -> Self {
        self.qualifiers = qualifiers;
        self
    }
}

/// A vector of members.
pub type MemberVector = Vec<Member>;

// ============================================================================
// Element
// ============================================================================

/// A resource element in the shader pipeline layout.
#[derive(Debug, Clone)]
pub struct Element {
    /// Whether this is a stage input, output, or neither.
    pub in_out: InOut,
    /// The kind of resource.
    pub kind: Kind,
    /// Binding location (-1 if not specified).
    pub location: i32,
    /// Data type token.
    pub data_type: Token,
    /// Element name.
    pub name: Token,
    /// Qualifiers.
    pub qualifiers: Token,
    /// Array size token.
    pub array_size: Token,
    /// Aggregate/block type name.
    pub aggregate_name: Token,
    /// Block/buffer members.
    pub members: MemberVector,
}

impl Element {
    /// Create a new element with default values.
    pub fn new() -> Self {
        Self {
            in_out: InOut::None,
            kind: Kind::None,
            location: -1,
            data_type: Token::new("unknown"),
            name: Token::new("unknown"),
            qualifiers: Token::empty(),
            array_size: Token::empty(),
            aggregate_name: Token::empty(),
            members: Vec::new(),
        }
    }

    /// Create element with full parameters.
    pub fn with_params(in_out: InOut, kind: Kind, data_type: Token, name: Token) -> Self {
        Self {
            in_out,
            kind,
            location: -1,
            data_type,
            name,
            qualifiers: Token::empty(),
            array_size: Token::empty(),
            aggregate_name: Token::empty(),
            members: Vec::new(),
        }
    }
}

impl Default for Element {
    fn default() -> Self {
        Self::new()
    }
}

/// A vector of elements.
pub type ElementVector = Vec<Element>;

// ============================================================================
// TextureType
// ============================================================================

/// The type of a texture element.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextureType {
    /// Standard texture.
    Texture,
    /// Shadow texture.
    ShadowTexture,
    /// Array texture (1D/2D array).
    ArrayTexture,
    /// Cubemap texture.
    CubemapTexture,
}

// ============================================================================
// TextureElement
// ============================================================================

/// A texture resource element.
#[derive(Debug, Clone)]
pub struct TextureElement {
    /// Texture name.
    pub name: Token,
    /// Texture dimensionality (1, 2, 3).
    pub dim: i32,
    /// Binding index.
    pub binding_index: i32,
    /// Pixel format.
    pub format: HioFormat,
    /// Texture type.
    pub texture_type: TextureType,
    /// Array size (0 = not array).
    pub array_size: i32,
}

impl TextureElement {
    /// Create a new texture element.
    pub fn new(name: Token, dim: i32, binding_index: i32) -> Self {
        Self {
            name,
            dim,
            binding_index,
            format: HioFormat::Float32Vec4,
            texture_type: TextureType::Texture,
            array_size: 0,
        }
    }

    /// Set format.
    pub fn with_format(mut self, format: HioFormat) -> Self {
        self.format = format;
        self
    }

    /// Set texture type.
    pub fn with_texture_type(mut self, texture_type: TextureType) -> Self {
        self.texture_type = texture_type;
        self
    }

    /// Set array size.
    pub fn with_array_size(mut self, array_size: i32) -> Self {
        self.array_size = array_size;
        self
    }
}

/// A vector of texture elements.
pub type TextureElementVector = Vec<TextureElement>;

// ============================================================================
// Layout tokens
// ============================================================================

/// Well-known tokens for GLSLFX resource layout parsing.
pub mod layout_tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    /// Layout token set.
    pub struct LayoutTokens {
        pub unknown: Token,
        pub block: Token,
        pub in_value: Token,
        pub out_value: Token,
        pub in_block: Token,
        pub out_block: Token,
        pub in_value_array: Token,
        pub out_value_array: Token,
        pub in_block_array: Token,
        pub out_block_array: Token,
        pub uniform_block: Token,
        pub buffer_read_only: Token,
        pub buffer_read_write: Token,
        pub centroid: Token,
        pub sample: Token,
        pub smooth: Token,
        pub flat: Token,
        pub noperspective: Token,
    }

    static TOKENS: LazyLock<LayoutTokens> = LazyLock::new(|| LayoutTokens {
        unknown: Token::new("unknown"),
        block: Token::new("block"),
        in_value: Token::new("in"),
        out_value: Token::new("out"),
        in_block: Token::new("in block"),
        out_block: Token::new("out block"),
        in_value_array: Token::new("in array"),
        out_value_array: Token::new("out array"),
        in_block_array: Token::new("in block array"),
        out_block_array: Token::new("out block array"),
        uniform_block: Token::new("uniform block"),
        buffer_read_only: Token::new("buffer readOnly"),
        buffer_read_write: Token::new("buffer readWrite"),
        centroid: Token::new("centroid"),
        sample: Token::new("sample"),
        smooth: Token::new("smooth"),
        flat: Token::new("flat"),
        noperspective: Token::new("noperspective"),
    });

    /// Get the layout tokens singleton.
    pub fn tokens() -> &'static LayoutTokens {
        &TOKENS
    }
}

// ============================================================================
// HioGlslfxResourceLayout
// ============================================================================

/// Shader resource layout parser.
///
/// Matches C++ `HioGlslfxResourceLayout`.
///
/// Parses resource layout definitions from GLSLFX layout dictionaries
/// into an intermediate representation of Elements and TextureElements.
pub struct HioGlslfxResourceLayout;

impl HioGlslfxResourceLayout {
    /// Parse layout elements from a GLSLFX layout dictionary.
    ///
    /// Matches C++ `HioGlslfxResourceLayout::ParseLayout()`.
    pub fn parse_layout(
        result: &mut ElementVector,
        shader_stage: &Token,
        layout_dict: &Dictionary,
    ) {
        if let Some(per_stage) = layout_dict.get(shader_stage.as_str()) {
            parse_per_stage_layout(result, per_stage);
        }
    }
}

// ============================================================================
// Private parsing helpers
// ============================================================================

/// Check if a token is a member qualifier (interpolation/storage).
fn is_member_qualifier(token: &str) -> bool {
    let t = layout_tokens::tokens();
    let tok = Token::new(token);
    tok == t.centroid
        || tok == t.sample
        || tok == t.flat
        || tok == t.noperspective
        || tok == t.smooth
}

/// Get a Token from a Value (defaulting to "unknown").
fn token_from_value(v: &Value) -> Token {
    v.get::<String>()
        .map(|s| Token::new(s))
        .unwrap_or_else(|| Token::new("unknown"))
}

/// Get a vector of string entries from a Value.
///
/// Resource layout entries are stored as Vec<String> (serialized JSON arrays).
fn get_value_vec(v: &Value) -> Vec<Value> {
    // Try Vec<String> first (our JSON parser stores arrays this way)
    if let Some(strings) = v.get::<Vec<String>>() {
        return strings.iter().map(|s| Value::from(s.clone())).collect();
    }
    // Try Vec<f32>
    if let Some(floats) = v.get::<Vec<f32>>() {
        return floats.iter().map(|f| Value::from(*f as f64)).collect();
    }
    Vec::new()
}

/// Parse members from input values starting at `from_element`.
fn parse_members(input: &[Value], from_element: usize) -> MemberVector {
    let mut result = MemberVector::new();

    for item in input.iter().skip(from_element) {
        let member_input = get_value_vec(item);
        let n = member_input.len();
        if !(2..=4).contains(&n) {
            continue;
        }

        let mut member = Member::new(
            token_from_value(&member_input[0]),
            token_from_value(&member_input[1]),
        );

        if n == 3 {
            let tok = token_from_value(&member_input[2]);
            if is_member_qualifier(tok.as_str()) {
                member.qualifiers = tok;
            } else {
                member.array_size = tok;
            }
        } else if n == 4 {
            member.array_size = token_from_value(&member_input[2]);
            member.qualifiers = token_from_value(&member_input[3]);
        }

        result.push(member);
    }

    result
}

/// Try to parse as a simple value: ["in", "vec3", "color"] or with qualifier.
fn try_parse_value(input: &[Value], element: &mut Element) -> bool {
    if input.len() != 3 && input.len() != 4 {
        return false;
    }
    let t = layout_tokens::tokens();
    let tag = token_from_value(&input[0]);

    let in_out = if tag == t.in_value {
        InOut::StageIn
    } else if tag == t.out_value {
        InOut::StageOut
    } else {
        return false;
    };

    *element = Element::with_params(
        in_out,
        Kind::Value,
        token_from_value(&input[1]),
        token_from_value(&input[2]),
    );
    if input.len() == 4 {
        element.qualifiers = token_from_value(&input[3]);
    }
    true
}

/// Try to parse as value array: ["in array", "vec3", "color", "NUM_VERTS"].
fn try_parse_value_array(input: &[Value], element: &mut Element) -> bool {
    if input.len() != 4 {
        return false;
    }
    let t = layout_tokens::tokens();
    let tag = token_from_value(&input[0]);

    let in_out = if tag == t.in_value_array {
        InOut::StageIn
    } else if tag == t.out_value_array {
        InOut::StageOut
    } else {
        return false;
    };

    *element = Element::with_params(
        in_out,
        Kind::Value,
        token_from_value(&input[1]),
        token_from_value(&input[2]),
    );
    element.array_size = token_from_value(&input[3]);
    true
}

/// Try to parse as block: ["in block", "VertexData", "inData", members...].
fn try_parse_block(input: &[Value], element: &mut Element) -> bool {
    if input.len() < 4 {
        return false;
    }
    let t = layout_tokens::tokens();
    let tag = token_from_value(&input[0]);

    let in_out = if tag == t.in_block {
        InOut::StageIn
    } else if tag == t.out_block {
        InOut::StageOut
    } else {
        return false;
    };

    *element = Element::with_params(
        in_out,
        Kind::Block,
        t.block.clone(),
        token_from_value(&input[2]),
    );
    element.aggregate_name = token_from_value(&input[1]);
    element.members = parse_members(input, 3);
    true
}

/// Try to parse as block array: ["in block array", "VertexData", "inData", "N", members...].
fn try_parse_block_array(input: &[Value], element: &mut Element) -> bool {
    if input.len() < 5 {
        return false;
    }
    let t = layout_tokens::tokens();
    let tag = token_from_value(&input[0]);

    let in_out = if tag == t.in_block_array {
        InOut::StageIn
    } else if tag == t.out_block_array {
        InOut::StageOut
    } else {
        return false;
    };

    *element = Element::with_params(
        in_out,
        Kind::Block,
        t.block.clone(),
        token_from_value(&input[2]),
    );
    element.array_size = token_from_value(&input[3]);
    element.aggregate_name = token_from_value(&input[1]);
    element.members = parse_members(input, 4);
    true
}

/// Try to parse as qualifier: ["in", "early_fragment_tests"].
fn try_parse_qualifier(input: &[Value], element: &mut Element) -> bool {
    if input.len() != 2 {
        return false;
    }
    let t = layout_tokens::tokens();
    let tag = token_from_value(&input[0]);

    if tag == t.in_value {
        *element = Element::with_params(
            InOut::StageIn,
            Kind::Qualifier,
            Token::empty(),
            Token::empty(),
        );
        element.qualifiers = token_from_value(&input[1]);
        return true;
    } else if tag == t.out_value {
        *element = Element::with_params(
            InOut::StageOut,
            Kind::Qualifier,
            Token::empty(),
            Token::empty(),
        );
        element.qualifiers = token_from_value(&input[1]);
        return true;
    }
    false
}

/// Try to parse as uniform block: ["uniform block", "Uniforms", "params", members...].
fn try_parse_uniform_block(input: &[Value], element: &mut Element) -> bool {
    if input.len() < 4 {
        return false;
    }
    let t = layout_tokens::tokens();
    let tag = token_from_value(&input[0]);

    if tag != t.uniform_block {
        return false;
    }

    *element = Element::with_params(
        InOut::None,
        Kind::UniformBlockConstantParams,
        t.uniform_block.clone(),
        token_from_value(&input[2]),
    );
    element.aggregate_name = token_from_value(&input[1]);
    element.members = parse_members(input, 3);
    true
}

/// Try to parse as buffer: ["buffer readOnly", "Name", "name", members...].
fn try_parse_buffer(input: &[Value], element: &mut Element) -> bool {
    if input.len() < 4 {
        return false;
    }
    let t = layout_tokens::tokens();
    let tag = token_from_value(&input[0]);

    let kind = if tag == t.buffer_read_only {
        Kind::BufferReadOnly
    } else if tag == t.buffer_read_write {
        Kind::BufferReadWrite
    } else {
        return false;
    };

    *element = Element::with_params(InOut::None, kind, tag, token_from_value(&input[2]));
    element.aggregate_name = token_from_value(&input[1]);
    element.members = parse_members(input, 3);
    true
}

/// Parse per-stage layout from a Value containing nested arrays.
fn parse_per_stage_layout(result: &mut ElementVector, per_stage: &Value) {
    let snippets = get_value_vec(per_stage);
    for snippet in &snippets {
        let decls = get_value_vec(snippet);
        for decl in &decls {
            let input = get_value_vec(decl);
            let mut element = Element::new();

            if try_parse_value(&input, &mut element)
                || try_parse_value_array(&input, &mut element)
                || try_parse_block(&input, &mut element)
                || try_parse_block_array(&input, &mut element)
                || try_parse_qualifier(&input, &mut element)
                || try_parse_uniform_block(&input, &mut element)
                || try_parse_buffer(&input, &mut element)
            {
                result.push(element);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_element_default() {
        let elem = Element::new();
        assert_eq!(elem.in_out, InOut::None);
        assert_eq!(elem.kind, Kind::None);
        assert_eq!(elem.location, -1);
    }

    #[test]
    fn test_member_creation() {
        let member = Member::new(Token::new("vec4"), Token::new("color"))
            .with_qualifiers(Token::new("flat"));
        assert_eq!(member.data_type.as_str(), "vec4");
        assert_eq!(member.name.as_str(), "color");
        assert_eq!(member.qualifiers.as_str(), "flat");
    }

    #[test]
    fn test_texture_element() {
        let tex = TextureElement::new(Token::new("diffuseMap"), 2, 0)
            .with_texture_type(TextureType::Texture)
            .with_format(HioFormat::UNorm8Vec4);
        assert_eq!(tex.name.as_str(), "diffuseMap");
        assert_eq!(tex.dim, 2);
        assert_eq!(tex.binding_index, 0);
    }

    #[test]
    fn test_is_member_qualifier() {
        assert!(is_member_qualifier("flat"));
        assert!(is_member_qualifier("centroid"));
        assert!(is_member_qualifier("smooth"));
        assert!(!is_member_qualifier("vec3"));
        assert!(!is_member_qualifier("myVar"));
    }
}
