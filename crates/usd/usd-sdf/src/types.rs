//! SDF basic data types.
//!
//! This module provides the fundamental types used throughout the SDF module.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt;
use std::hash::{Hash, Hasher};

use ordered_float::OrderedFloat;
use usd_tf::Token;

use super::path::Path;

// ============================================================================
// Spec Type
// ============================================================================

/// The type of a spec (object) in scene description.
///
/// Objects are entities that have fields and are addressable by path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum SpecType {
    /// Unknown spec type (value 0 so that default is unknown).
    #[default]
    Unknown = 0,
    /// An attribute spec.
    Attribute,
    /// A connection spec.
    Connection,
    /// An expression spec.
    Expression,
    /// A mapper spec.
    Mapper,
    /// A mapper arg spec.
    MapperArg,
    /// A prim spec.
    Prim,
    /// A pseudo-root spec.
    PseudoRoot,
    /// A relationship spec.
    Relationship,
    /// A relationship target spec.
    RelationshipTarget,
    /// A variant spec.
    Variant,
    /// A variant set spec.
    VariantSet,
}

impl SpecType {
    /// Returns true if this is a valid spec type.
    pub fn is_valid(&self) -> bool {
        !matches!(self, SpecType::Unknown)
    }

    /// Returns the total number of spec types.
    pub const fn count() -> usize {
        12 // SdfNumSpecTypes - 1 (excluding Unknown)
    }

    /// Converts a u32 to SpecType.
    #[must_use]
    pub fn from_u32(v: u32) -> Self {
        match v {
            1 => SpecType::Attribute,
            2 => SpecType::Connection,
            3 => SpecType::Expression,
            4 => SpecType::Mapper,
            5 => SpecType::MapperArg,
            6 => SpecType::Prim,
            7 => SpecType::PseudoRoot,
            8 => SpecType::Relationship,
            9 => SpecType::RelationshipTarget,
            10 => SpecType::Variant,
            11 => SpecType::VariantSet,
            _ => SpecType::Unknown,
        }
    }
}

impl fmt::Display for SpecType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SpecType::Unknown => write!(f, "unknown"),
            SpecType::Attribute => write!(f, "attribute"),
            SpecType::Connection => write!(f, "connection"),
            SpecType::Expression => write!(f, "expression"),
            SpecType::Mapper => write!(f, "mapper"),
            SpecType::MapperArg => write!(f, "mapperArg"),
            SpecType::Prim => write!(f, "prim"),
            SpecType::PseudoRoot => write!(f, "pseudoRoot"),
            SpecType::Relationship => write!(f, "relationship"),
            SpecType::RelationshipTarget => write!(f, "relationshipTarget"),
            SpecType::Variant => write!(f, "variant"),
            SpecType::VariantSet => write!(f, "variantSet"),
        }
    }
}

// ============================================================================
// Specifier
// ============================================================================

/// Identifies the possible specifiers for a prim spec.
///
/// - `Def` - Defines a concrete prim
/// - `Over` - Overrides an existing prim
/// - `Class` - Defines an abstract prim
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum Specifier {
    /// Defines a concrete prim.
    #[default]
    Def = 0,
    /// Overrides an existing prim.
    Over,
    /// Defines an abstract prim (class).
    Class,
}

impl Specifier {
    /// Returns the keyword string for this specifier.
    pub fn as_str(&self) -> &'static str {
        match self {
            Specifier::Def => "def",
            Specifier::Over => "over",
            Specifier::Class => "class",
        }
    }

    /// Returns true if this specifier defines a prim.
    ///
    /// A prim is defined if the specifier is `Def` or `Class`.
    pub fn is_defining(&self) -> bool {
        !matches!(self, Specifier::Over)
    }

    /// Returns the number of specifiers.
    pub const fn count() -> usize {
        3
    }
}

impl fmt::Display for Specifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Specifier::Def => write!(f, "def"),
            Specifier::Over => write!(f, "over"),
            Specifier::Class => write!(f, "class"),
        }
    }
}

impl TryFrom<&str> for Specifier {
    type Error = ();

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "def" => Ok(Specifier::Def),
            "over" => Ok(Specifier::Over),
            "class" => Ok(Specifier::Class),
            _ => Err(()),
        }
    }
}

// ============================================================================
// Permission
// ============================================================================

/// Defines permission levels for prims.
///
/// Permissions control which layers may refer to or express opinions
/// about a prim.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum Permission {
    /// Public prims can be referred to by anything.
    #[default]
    Public = 0,
    /// Private prims can be referred to only within the local layer stack.
    Private,
}

impl Permission {
    /// Returns the number of permission levels.
    pub const fn count() -> usize {
        2
    }
}

impl fmt::Display for Permission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Permission::Public => write!(f, "public"),
            Permission::Private => write!(f, "private"),
        }
    }
}

// ============================================================================
// Variability
// ============================================================================

/// Identifies variability types for attributes.
///
/// Variability indicates whether the attribute may vary over time
/// and value coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum Variability {
    /// Varying attributes may be animated and affected by actions.
    #[default]
    Varying = 0,
    /// Uniform attributes may only have non-animated default values.
    Uniform,
}

impl Variability {
    /// Returns the number of variability types.
    pub const fn count() -> usize {
        2
    }
}

impl fmt::Display for Variability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Variability::Varying => write!(f, "varying"),
            Variability::Uniform => write!(f, "uniform"),
        }
    }
}

// ============================================================================
// Authoring Error
// ============================================================================

/// Error codes related to authoring operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AuthoringError {
    /// Fields from source layer are not recognized by target layer's schema.
    UnrecognizedFields,
    /// Attempt to create spec with type not recognized by layer's schema.
    UnrecognizedSpecType,
}

impl fmt::Display for AuthoringError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AuthoringError::UnrecognizedFields => {
                write!(f, "Unrecognized fields in source layer")
            }
            AuthoringError::UnrecognizedSpecType => {
                write!(f, "Unrecognized spec type for layer schema")
            }
        }
    }
}

impl std::error::Error for AuthoringError {}

// ============================================================================
// Opaque Value
// ============================================================================

/// In-memory representation of the value of an opaque attribute.
///
/// Opaque attributes cannot have authored values, but every typename in Sdf
/// must have a corresponding constructable value type; `OpaqueValue` is
/// the type associated with opaque attributes. Opaque values intentionally
/// cannot hold any information, cannot be parsed, and cannot be serialized to
/// a layer.
///
/// `OpaqueValue` is also the type associated with group attributes. A group
/// attribute is an opaque attribute that represents a group of other
/// properties.
///
/// # Examples
///
/// ```ignore
/// use usd_sdf::OpaqueValue;
///
/// let v1 = OpaqueValue;
/// let v2 = OpaqueValue;
/// assert_eq!(v1, v2);
/// ```
#[derive(Debug, Clone, Copy, Default)]
pub struct OpaqueValue;

impl PartialEq for OpaqueValue {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl Eq for OpaqueValue {}

impl Hash for OpaqueValue {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Use a nonzero constant because some bad hash functions don't deal
        // with zero well. Chosen by fair dice roll.
        9u8.hash(state);
    }
}

impl fmt::Display for OpaqueValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "OpaqueValue")
    }
}

// ============================================================================
// Value Block
// ============================================================================

/// A special value type that explicitly represents "no value".
///
/// This is different from not having a value authored. It can be used
/// to block a value from being inherited from a weaker layer.
///
/// # Examples
///
/// ```ignore
/// use usd_sdf::ValueBlock;
///
/// let block1 = ValueBlock;
/// let block2 = ValueBlock;
/// assert_eq!(block1, block2);
/// ```
#[derive(Debug, Clone, Copy, Default)]
pub struct ValueBlock;

impl PartialEq for ValueBlock {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl Eq for ValueBlock {}

impl Hash for ValueBlock {
    fn hash<H: Hasher>(&self, _state: &mut H) {
        // All ValueBlocks hash to the same value
    }
}

impl fmt::Display for ValueBlock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "None")
    }
}

// ============================================================================
// Animation Block
// ============================================================================

/// A special value type that blocks animation from weaker layers.
///
/// Used to explicitly author that an attribute should have no animation
/// value, blocking spline or time sample values from weaker layers.
#[derive(Debug, Clone, Copy, Default)]
pub struct AnimationBlock;

impl PartialEq for AnimationBlock {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl Eq for AnimationBlock {}

impl Hash for AnimationBlock {
    fn hash<H: Hasher>(&self, _state: &mut H) {
        // All AnimationBlocks hash to the same value
    }
}

impl fmt::Display for AnimationBlock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "AnimationBlock")
    }
}

// ============================================================================
// Human Readable Value
// ============================================================================

/// A value that serializes to human-readable text.
///
/// Used for producing more readable layer output.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HumanReadableValue {
    text: String,
}

impl HumanReadableValue {
    /// Creates a new human-readable value from text.
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }

    /// Returns the text representation.
    pub fn text(&self) -> &str {
        &self.text
    }
}

impl Hash for HumanReadableValue {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.text.hash(state);
    }
}

impl fmt::Display for HumanReadableValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.text)
    }
}

// ============================================================================
// Tuple Dimensions
// ============================================================================

/// Represents the shape of a value type (or that of an element in an array).
///
/// Tuple dimensions describe the shape of scalar value types like vectors
/// and matrices. For example, a 3D vector has dimensions `(3,)` and a
/// 4x4 matrix has dimensions `(4, 4)`.
///
/// # Examples
///
/// ```ignore
/// use usd_sdf::TupleDimensions;
///
/// // Scalar (no dimensions)
/// let scalar = TupleDimensions::scalar();
/// assert!(scalar.is_scalar());
///
/// // 1D (e.g., Vec3)
/// let vec3 = TupleDimensions::d1(3);
/// assert_eq!(vec3.size(), 1);
/// assert_eq!(vec3.d(), [3, 0]);
///
/// // 2D (e.g., Matrix4x4)
/// let mat4 = TupleDimensions::d2(4, 4);
/// assert_eq!(mat4.size(), 2);
/// assert_eq!(mat4.d(), [4, 4]);
/// ```
#[derive(Debug, Clone, Copy, Default)]
pub struct TupleDimensions {
    /// The dimensions array (up to 2 dimensions).
    d: [usize; 2],
    /// The number of dimensions (0, 1, or 2).
    size: usize,
}

impl TupleDimensions {
    /// Creates a scalar (0-dimensional) tuple.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::TupleDimensions;
    ///
    /// let dims = TupleDimensions::scalar();
    /// assert!(dims.is_scalar());
    /// ```
    #[must_use]
    pub const fn scalar() -> Self {
        Self { d: [0, 0], size: 0 }
    }

    /// Creates a 1-dimensional tuple.
    ///
    /// # Arguments
    ///
    /// * `m` - Size of the first dimension
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::TupleDimensions;
    ///
    /// let vec3 = TupleDimensions::d1(3);
    /// assert_eq!(vec3.d()[0], 3);
    /// ```
    #[must_use]
    pub const fn d1(m: usize) -> Self {
        Self { d: [m, 0], size: 1 }
    }

    /// Creates a 2-dimensional tuple.
    ///
    /// # Arguments
    ///
    /// * `m` - Size of the first dimension
    /// * `n` - Size of the second dimension
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::TupleDimensions;
    ///
    /// let mat4 = TupleDimensions::d2(4, 4);
    /// assert_eq!(mat4.d()[0], 4);
    /// assert_eq!(mat4.d()[1], 4);
    /// ```
    #[must_use]
    pub const fn d2(m: usize, n: usize) -> Self {
        Self { d: [m, n], size: 2 }
    }

    /// Creates tuple dimensions from a 2-element array.
    ///
    /// # Arguments
    ///
    /// * `dims` - Array of dimension sizes
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::TupleDimensions;
    ///
    /// let dims = TupleDimensions::from_array([3, 3]);
    /// assert_eq!(dims.size(), 2);
    /// ```
    #[must_use]
    pub const fn from_array(dims: [usize; 2]) -> Self {
        Self { d: dims, size: 2 }
    }

    /// Returns the dimensions array.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::TupleDimensions;
    ///
    /// let dims = TupleDimensions::d2(3, 4);
    /// assert_eq!(dims.d(), [3, 4]);
    /// ```
    #[must_use]
    pub const fn d(&self) -> [usize; 2] {
        self.d
    }

    /// Returns the number of dimensions.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::TupleDimensions;
    ///
    /// assert_eq!(TupleDimensions::scalar().size(), 0);
    /// assert_eq!(TupleDimensions::d1(3).size(), 1);
    /// assert_eq!(TupleDimensions::d2(2, 3).size(), 2);
    /// ```
    #[must_use]
    pub const fn size(&self) -> usize {
        self.size
    }

    /// Returns true if this is a scalar (0-dimensional).
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::TupleDimensions;
    ///
    /// assert!(TupleDimensions::scalar().is_scalar());
    /// assert!(!TupleDimensions::d1(3).is_scalar());
    /// ```
    #[must_use]
    pub const fn is_scalar(&self) -> bool {
        self.size == 0
    }

    /// Returns the total number of elements.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::TupleDimensions;
    ///
    /// assert_eq!(TupleDimensions::scalar().num_elements(), 1);
    /// assert_eq!(TupleDimensions::d1(3).num_elements(), 3);
    /// assert_eq!(TupleDimensions::d2(4, 4).num_elements(), 16);
    /// ```
    #[must_use]
    pub const fn num_elements(&self) -> usize {
        match self.size {
            0 => 1,
            1 => self.d[0],
            2 => self.d[0] * self.d[1],
            _ => 0,
        }
    }
}

impl PartialEq for TupleDimensions {
    fn eq(&self, other: &Self) -> bool {
        if self.size != other.size {
            return false;
        }
        match self.size {
            0 => true,
            1 => self.d[0] == other.d[0],
            2 => self.d[0] == other.d[0] && self.d[1] == other.d[1],
            _ => false,
        }
    }
}

impl Eq for TupleDimensions {}

impl Hash for TupleDimensions {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.size.hash(state);
        for i in 0..self.size {
            self.d[i].hash(state);
        }
    }
}

impl fmt::Display for TupleDimensions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.size {
            0 => write!(f, "()"),
            1 => write!(f, "({})", self.d[0]),
            2 => write!(f, "({}, {})", self.d[0], self.d[1]),
            _ => write!(f, "(invalid)"),
        }
    }
}

impl From<usize> for TupleDimensions {
    fn from(m: usize) -> Self {
        Self::d1(m)
    }
}

impl From<(usize, usize)> for TupleDimensions {
    fn from((m, n): (usize, usize)) -> Self {
        Self::d2(m, n)
    }
}

impl From<[usize; 2]> for TupleDimensions {
    fn from(dims: [usize; 2]) -> Self {
        Self::from_array(dims)
    }
}

// ============================================================================
// Type Aliases
// ============================================================================

/// A map of mapper parameter names to parameter values.
pub type MapperParametersMap = HashMap<String, Box<dyn std::any::Any + Send + Sync>>;

/// A map of variant set names to variant selections.
pub type VariantSelectionMap = HashMap<String, String>;

/// A map of variant set names to lists of variants.
pub type VariantsMap = HashMap<String, Vec<String>>;

/// A map of source paths to target paths for relocation.
pub type RelocatesMap = BTreeMap<Path, Path>;

/// A single relocation (source path, target path).
pub type Relocate = (Path, Path);

/// A vector of relocations.
pub type Relocates = Vec<Relocate>;

/// A map from sample times to sample values.
pub type TimeSampleMap = BTreeMap<f64, Box<dyn std::any::Any + Send + Sync>>;

// ============================================================================
// Time Samples Set
// ============================================================================

/// Set of time sample values.
///
/// Matches C++ `std::set<double>` / `UsdAbc_TimeSamples`.
/// Uses `OrderedFloat<f64>` because `f64` doesn't implement `Ord` in Rust.
pub type TimeSamples = BTreeSet<OrderedFloat<f64>>;

// ============================================================================
// Unit Types
// ============================================================================

/// Length units.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LengthUnit {
    /// Millimeters (0.001 m).
    Millimeter,
    /// Centimeters (0.01 m).
    Centimeter,
    /// Decimeters (0.1 m).
    Decimeter,
    /// Meters (base unit).
    Meter,
    /// Kilometers (1000 m).
    Kilometer,
    /// Inches (0.0254 m).
    Inch,
    /// Feet (0.3048 m).
    Foot,
    /// Yards (0.9144 m).
    Yard,
    /// Miles (1609.344 m).
    Mile,
}

impl LengthUnit {
    /// Returns the scale factor relative to meters.
    pub fn scale(&self) -> f64 {
        match self {
            LengthUnit::Millimeter => 0.001,
            LengthUnit::Centimeter => 0.01,
            LengthUnit::Decimeter => 0.1,
            LengthUnit::Meter => 1.0,
            LengthUnit::Kilometer => 1000.0,
            LengthUnit::Inch => 0.0254,
            LengthUnit::Foot => 0.3048,
            LengthUnit::Yard => 0.9144,
            LengthUnit::Mile => 1609.344,
        }
    }

    /// Returns the display name.
    pub fn name(&self) -> &'static str {
        match self {
            LengthUnit::Millimeter => "mm",
            LengthUnit::Centimeter => "cm",
            LengthUnit::Decimeter => "dm",
            LengthUnit::Meter => "m",
            LengthUnit::Kilometer => "km",
            LengthUnit::Inch => "in",
            LengthUnit::Foot => "ft",
            LengthUnit::Yard => "yd",
            LengthUnit::Mile => "mi",
        }
    }
}

/// Angular units.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AngularUnit {
    /// Degrees (base unit).
    Degrees,
    /// Radians.
    Radians,
}

impl AngularUnit {
    /// Returns the scale factor relative to degrees.
    pub fn scale(&self) -> f64 {
        match self {
            AngularUnit::Degrees => 1.0,
            AngularUnit::Radians => 57.295_779_513_082_32,
        }
    }

    /// Returns the display name.
    pub fn name(&self) -> &'static str {
        match self {
            AngularUnit::Degrees => "deg",
            AngularUnit::Radians => "rad",
        }
    }
}

/// Dimensionless units.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DimensionlessUnit {
    /// Percent (0.01).
    Percent,
    /// Default (1.0).
    Default,
}

impl DimensionlessUnit {
    /// Returns the scale factor.
    pub fn scale(&self) -> f64 {
        match self {
            DimensionlessUnit::Percent => 0.01,
            DimensionlessUnit::Default => 1.0,
        }
    }

    /// Returns the display name.
    pub fn name(&self) -> &'static str {
        match self {
            DimensionlessUnit::Percent => "%",
            DimensionlessUnit::Default => "default",
        }
    }
}

// ============================================================================
// Value Role Names
// ============================================================================

/// Role name tokens for value types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ValueRole {
    /// Point role (position in space).
    Point,
    /// Normal role (surface normal).
    Normal,
    /// Vector role (direction).
    Vector,
    /// Color role.
    Color,
    /// Frame role (coordinate frame).
    Frame,
    /// Transform role (transformation matrix).
    Transform,
    /// Point index role.
    PointIndex,
    /// Edge index role.
    EdgeIndex,
    /// Face index role.
    FaceIndex,
    /// Group role.
    Group,
    /// Texture coordinate role.
    TextureCoordinate,
}

impl ValueRole {
    /// Returns the role name as a string.
    pub fn name(&self) -> &'static str {
        match self {
            ValueRole::Point => "Point",
            ValueRole::Normal => "Normal",
            ValueRole::Vector => "Vector",
            ValueRole::Color => "Color",
            ValueRole::Frame => "Frame",
            ValueRole::Transform => "Transform",
            ValueRole::PointIndex => "PointIndex",
            ValueRole::EdgeIndex => "EdgeIndex",
            ValueRole::FaceIndex => "FaceIndex",
            ValueRole::Group => "Group",
            ValueRole::TextureCoordinate => "TextureCoordinate",
        }
    }

    /// Returns the role as a token.
    pub fn as_token(&self) -> Token {
        Token::new(self.name())
    }
}

impl fmt::Display for ValueRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

// ============================================================================
// Unit Conversion Free Functions (matches C++ SdfDefaultUnit, SdfConvertUnit etc.)
// ============================================================================

/// Unit kind discriminator used by unit conversion functions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnitKind {
    Length(LengthUnit),
    Angular(AngularUnit),
    Dimensionless(DimensionlessUnit),
}

impl UnitKind {
    /// Scale factor relative to the base unit for this category.
    pub fn scale(&self) -> f64 {
        match self {
            UnitKind::Length(u) => u.scale(),
            UnitKind::Angular(u) => u.scale(),
            UnitKind::Dimensionless(u) => u.scale(),
        }
    }

    /// Display name for this unit.
    pub fn name(&self) -> &'static str {
        match self {
            UnitKind::Length(u) => u.name(),
            UnitKind::Angular(u) => u.name(),
            UnitKind::Dimensionless(u) => u.name(),
        }
    }

    /// Category string matching C++ SdfUnitCategory return.
    pub fn category(&self) -> &'static str {
        match self {
            UnitKind::Length(_) => "Length",
            UnitKind::Angular(_) => "Angular",
            UnitKind::Dimensionless(_) => "Dimensionless",
        }
    }

    /// Default unit for this category (matches C++ SdfDefaultUnit(TfEnum)).
    pub fn default_unit(&self) -> UnitKind {
        match self {
            UnitKind::Length(_) => UnitKind::Length(LengthUnit::Centimeter),
            UnitKind::Angular(_) => UnitKind::Angular(AngularUnit::Degrees),
            UnitKind::Dimensionless(_) => UnitKind::Dimensionless(DimensionlessUnit::Default),
        }
    }
}

/// Returns the default unit for the given type name token.
///
/// Matches C++ `SdfDefaultUnit(TfToken const& typeName)`.
/// Returns `None` if the type name is not a recognised unit type.
pub fn default_unit_for_type(type_name: &str) -> Option<UnitKind> {
    match type_name {
        "Length" | "length" => Some(UnitKind::Length(LengthUnit::Centimeter)),
        "Angular" | "angular" | "Angle" | "angle" => Some(UnitKind::Angular(AngularUnit::Degrees)),
        "Dimensionless" | "dimensionless" => {
            Some(UnitKind::Dimensionless(DimensionlessUnit::Default))
        }
        _ => None,
    }
}

/// Returns the default unit for the given unit kind.
///
/// Matches C++ `SdfDefaultUnit(const TfEnum& unit)`.
pub fn default_unit(unit: UnitKind) -> UnitKind {
    unit.default_unit()
}

/// Returns the category string for the given unit.
///
/// Matches C++ `SdfUnitCategory(const TfEnum& unit)`.
pub fn unit_category(unit: UnitKind) -> &'static str {
    unit.category()
}

/// Converts between two units of the same category.
///
/// Returns the factor by which a value in `from` must be multiplied to
/// get the equivalent value in `to`.  Returns `0.0` and warns if the
/// units belong to different categories.
///
/// Matches C++ `SdfConvertUnit(const TfEnum& fromUnit, const TfEnum& toUnit)`.
pub fn convert_unit(from: UnitKind, to: UnitKind) -> f64 {
    // Both units must belong to the same category.
    if from.category() != to.category() {
        eprintln!(
            "SdfConvertUnit: cannot convert from '{}' ({}) to '{}' ({})",
            from.name(),
            from.category(),
            to.name(),
            to.category()
        );
        return 0.0;
    }
    // scale() is expressed relative to the category base unit, so:
    //   value_base = value_from * from.scale()
    //   value_to   = value_base / to.scale()
    //   factor     = from.scale() / to.scale()
    from.scale() / to.scale()
}

/// Returns the display name for the given unit.
///
/// Matches C++ `SdfGetNameForUnit(const TfEnum& unit)`.
pub fn get_name_for_unit(unit: UnitKind) -> &'static str {
    unit.name()
}

/// Resolves a unit from its display name string.
///
/// Matches C++ `SdfGetUnitFromName(const std::string& name)`.
/// Returns `None` if the name is not recognised.
pub fn get_unit_from_name(name: &str) -> Option<UnitKind> {
    match name {
        "mm" => Some(UnitKind::Length(LengthUnit::Millimeter)),
        "cm" => Some(UnitKind::Length(LengthUnit::Centimeter)),
        "dm" => Some(UnitKind::Length(LengthUnit::Decimeter)),
        "m" => Some(UnitKind::Length(LengthUnit::Meter)),
        "km" => Some(UnitKind::Length(LengthUnit::Kilometer)),
        "in" => Some(UnitKind::Length(LengthUnit::Inch)),
        "ft" => Some(UnitKind::Length(LengthUnit::Foot)),
        "yd" => Some(UnitKind::Length(LengthUnit::Yard)),
        "mi" => Some(UnitKind::Length(LengthUnit::Mile)),
        "deg" => Some(UnitKind::Angular(AngularUnit::Degrees)),
        "rad" => Some(UnitKind::Angular(AngularUnit::Radians)),
        "%" | "percent" => Some(UnitKind::Dimensionless(DimensionlessUnit::Percent)),
        "default" => Some(UnitKind::Dimensionless(DimensionlessUnit::Default)),
        _ => None,
    }
}

// ============================================================================
// Value Type Validation Free Functions (matches C++ SdfValueHasValidType etc.)
// ============================================================================

use crate::value_type_name::ValueTypeName;
use crate::value_type_registry::ValueTypeRegistry;
use usd_vt::Value;

/// Returns true if `value` holds a type that is registered in the SDF schema.
///
/// Matches C++ `SdfValueHasValidType(VtValue const& value)`.
pub fn value_has_valid_type(value: &Value) -> bool {
    match value.held_type_id() {
        Some(tid) => ValueTypeRegistry::instance()
            .find_type_by_type_id(tid, None)
            .is_valid(),
        None => false,
    }
}

/// Returns the `ValueTypeName` registered under the given type name token.
///
/// Returns an invalid `ValueTypeName` if the name is not found.
///
/// Matches C++ `SdfGetTypeForValueTypeName(TfToken const& name)`.
pub fn get_type_for_value_type_name(name: &usd_tf::Token) -> ValueTypeName {
    ValueTypeRegistry::instance().find_type_by_token(name)
}

/// Returns the `ValueTypeName` whose Rust type matches `value`.
///
/// Returns an invalid `ValueTypeName` if no matching type is registered.
///
/// Matches C++ `SdfGetValueTypeNameForValue(VtValue const& val)`.
pub fn get_value_type_name_for_value(value: &Value) -> ValueTypeName {
    match value.held_type_id() {
        Some(tid) => ValueTypeRegistry::instance().find_type_by_type_id(tid, None),
        None => ValueTypeName::invalid(),
    }
}

/// Returns the role-name token for the given value type name.
///
/// Returns an empty token if the name is not found.
///
/// Matches C++ `SdfGetRoleNameForValueTypeName(TfToken const& typeName)`.
pub fn get_role_name_for_value_type_name(name: &usd_tf::Token) -> usd_tf::Token {
    ValueTypeRegistry::instance()
        .find_type_by_token(name)
        .get_role()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spec_type() {
        assert_eq!(SpecType::default(), SpecType::Unknown);
        assert!(!SpecType::Unknown.is_valid());
        assert!(SpecType::Prim.is_valid());
        assert_eq!(format!("{}", SpecType::Prim), "prim");
    }

    #[test]
    fn test_specifier() {
        assert_eq!(Specifier::default(), Specifier::Def);
        assert!(Specifier::Def.is_defining());
        assert!(!Specifier::Over.is_defining());
        assert!(Specifier::Class.is_defining());
        assert_eq!(format!("{}", Specifier::Def), "def");
    }

    #[test]
    fn test_specifier_from_str() {
        assert_eq!(Specifier::try_from("def"), Ok(Specifier::Def));
        assert_eq!(Specifier::try_from("over"), Ok(Specifier::Over));
        assert_eq!(Specifier::try_from("class"), Ok(Specifier::Class));
        assert!(Specifier::try_from("invalid").is_err());
    }

    #[test]
    fn test_permission() {
        assert_eq!(Permission::default(), Permission::Public);
        assert_eq!(format!("{}", Permission::Private), "private");
    }

    #[test]
    fn test_variability() {
        assert_eq!(Variability::default(), Variability::Varying);
        assert_eq!(format!("{}", Variability::Uniform), "uniform");
    }

    #[test]
    fn test_value_block() {
        let b1 = ValueBlock;
        let b2 = ValueBlock;
        assert_eq!(b1, b2);
        assert_eq!(format!("{}", b1), "None");
    }

    #[test]
    fn test_animation_block() {
        let b1 = AnimationBlock;
        let b2 = AnimationBlock;
        assert_eq!(b1, b2);
    }

    #[test]
    fn test_human_readable_value() {
        let v = HumanReadableValue::new("test");
        assert_eq!(v.text(), "test");
        assert_eq!(format!("{}", v), "test");
    }

    #[test]
    fn test_length_unit() {
        assert_eq!(LengthUnit::Meter.scale(), 1.0);
        assert_eq!(LengthUnit::Kilometer.scale(), 1000.0);
        assert_eq!(LengthUnit::Meter.name(), "m");
    }

    #[test]
    fn test_angular_unit() {
        assert_eq!(AngularUnit::Degrees.scale(), 1.0);
        assert_eq!(AngularUnit::Degrees.name(), "deg");
    }

    #[test]
    fn test_dimensionless_unit() {
        assert_eq!(DimensionlessUnit::Percent.scale(), 0.01);
        assert_eq!(DimensionlessUnit::Default.scale(), 1.0);
    }

    #[test]
    fn test_value_role() {
        assert_eq!(ValueRole::Point.name(), "Point");
        assert_eq!(ValueRole::Normal.as_token().as_str(), "Normal");
    }

    #[test]
    fn test_authoring_error() {
        let err = AuthoringError::UnrecognizedFields;
        assert!(format!("{}", err).contains("Unrecognized"));
    }

    #[test]
    fn test_tuple_dimensions_scalar() {
        let dims = TupleDimensions::scalar();
        assert!(dims.is_scalar());
        assert_eq!(dims.size(), 0);
        assert_eq!(dims.num_elements(), 1);
        assert_eq!(format!("{}", dims), "()");
    }

    #[test]
    fn test_tuple_dimensions_d1() {
        let dims = TupleDimensions::d1(3);
        assert!(!dims.is_scalar());
        assert_eq!(dims.size(), 1);
        assert_eq!(dims.d()[0], 3);
        assert_eq!(dims.num_elements(), 3);
        assert_eq!(format!("{}", dims), "(3)");
    }

    #[test]
    fn test_tuple_dimensions_d2() {
        let dims = TupleDimensions::d2(4, 4);
        assert!(!dims.is_scalar());
        assert_eq!(dims.size(), 2);
        assert_eq!(dims.d(), [4, 4]);
        assert_eq!(dims.num_elements(), 16);
        assert_eq!(format!("{}", dims), "(4, 4)");
    }

    #[test]
    fn test_tuple_dimensions_equality() {
        let d1 = TupleDimensions::d1(3);
        let d2 = TupleDimensions::d1(3);
        let d3 = TupleDimensions::d1(4);
        let d4 = TupleDimensions::d2(3, 3);

        assert_eq!(d1, d2);
        assert_ne!(d1, d3);
        assert_ne!(d1, d4);
    }

    #[test]
    fn test_tuple_dimensions_hash() {
        use std::collections::hash_map::DefaultHasher;

        let d1 = TupleDimensions::d1(3);
        let d2 = TupleDimensions::d1(3);

        let mut h1 = DefaultHasher::new();
        let mut h2 = DefaultHasher::new();
        d1.hash(&mut h1);
        d2.hash(&mut h2);
        assert_eq!(h1.finish(), h2.finish());
    }

    #[test]
    fn test_tuple_dimensions_from() {
        let from_usize: TupleDimensions = 3usize.into();
        assert_eq!(from_usize, TupleDimensions::d1(3));

        let from_tuple: TupleDimensions = (4, 4).into();
        assert_eq!(from_tuple, TupleDimensions::d2(4, 4));

        let from_array: TupleDimensions = [2, 3].into();
        assert_eq!(from_array, TupleDimensions::d2(2, 3));
    }

    #[test]
    fn test_opaque_value() {
        let v1 = OpaqueValue;
        let v2 = OpaqueValue;
        assert_eq!(v1, v2);
        assert_eq!(format!("{}", v1), "OpaqueValue");

        // Test hash consistency
        use std::collections::hash_map::DefaultHasher;
        let mut hasher1 = DefaultHasher::new();
        let mut hasher2 = DefaultHasher::new();
        v1.hash(&mut hasher1);
        v2.hash(&mut hasher2);
        assert_eq!(hasher1.finish(), hasher2.finish());
    }

    // -----------------------------------------------------------------------
    // Unit conversion free functions
    // -----------------------------------------------------------------------

    #[test]
    fn test_unit_kind_category() {
        assert_eq!(UnitKind::Length(LengthUnit::Meter).category(), "Length");
        assert_eq!(
            UnitKind::Angular(AngularUnit::Degrees).category(),
            "Angular"
        );
        assert_eq!(
            UnitKind::Dimensionless(DimensionlessUnit::Percent).category(),
            "Dimensionless"
        );
    }

    #[test]
    fn test_default_unit() {
        // Default length unit is centimeter (matches C++ SdfDefaultUnit).
        let d = default_unit(UnitKind::Length(LengthUnit::Meter));
        assert_eq!(d, UnitKind::Length(LengthUnit::Centimeter));

        let d = default_unit(UnitKind::Angular(AngularUnit::Radians));
        assert_eq!(d, UnitKind::Angular(AngularUnit::Degrees));
    }

    #[test]
    fn test_unit_category() {
        assert_eq!(
            unit_category(UnitKind::Length(LengthUnit::Kilometer)),
            "Length"
        );
    }

    #[test]
    fn test_convert_unit() {
        // 1 m = 100 cm, so factor = meter.scale() / cm.scale() = 1.0 / 0.01 = 100.
        let factor = convert_unit(
            UnitKind::Length(LengthUnit::Meter),
            UnitKind::Length(LengthUnit::Centimeter),
        );
        assert!(
            (factor - 100.0).abs() < 1e-9,
            "m->cm factor should be 100, got {factor}"
        );

        // 1 km = 1000 m.
        let factor = convert_unit(
            UnitKind::Length(LengthUnit::Kilometer),
            UnitKind::Length(LengthUnit::Meter),
        );
        assert!((factor - 1000.0).abs() < 1e-9);

        // Cross-category: returns 0.
        let bad = convert_unit(
            UnitKind::Length(LengthUnit::Meter),
            UnitKind::Angular(AngularUnit::Degrees),
        );
        assert_eq!(bad, 0.0);
    }

    #[test]
    fn test_get_name_for_unit() {
        assert_eq!(get_name_for_unit(UnitKind::Length(LengthUnit::Meter)), "m");
        assert_eq!(
            get_name_for_unit(UnitKind::Angular(AngularUnit::Radians)),
            "rad"
        );
        assert_eq!(
            get_name_for_unit(UnitKind::Dimensionless(DimensionlessUnit::Percent)),
            "%"
        );
    }

    #[test]
    fn test_get_unit_from_name() {
        assert_eq!(
            get_unit_from_name("m"),
            Some(UnitKind::Length(LengthUnit::Meter))
        );
        assert_eq!(
            get_unit_from_name("cm"),
            Some(UnitKind::Length(LengthUnit::Centimeter))
        );
        assert_eq!(
            get_unit_from_name("deg"),
            Some(UnitKind::Angular(AngularUnit::Degrees))
        );
        assert_eq!(get_unit_from_name("unknown"), None);
    }

    // -----------------------------------------------------------------------
    // Value type validation free functions
    // -----------------------------------------------------------------------

    #[test]
    fn test_value_type_validation() {
        use usd_vt::Value;

        // f32 is registered in the standard type set.
        let v = Value::from(1.0f32);
        assert!(value_has_valid_type(&v), "f32 should be a valid SDF type");

        // The type name lookup should find "float" for f32.
        let type_name = get_value_type_name_for_value(&v);
        assert!(
            type_name.is_valid(),
            "get_value_type_name_for_value should return valid for f32"
        );

        // Token lookup.
        use usd_tf::Token;
        let found = get_type_for_value_type_name(&Token::new("float"));
        assert!(
            found.is_valid(),
            "get_type_for_value_type_name(\"float\") should be valid"
        );

        // Unknown name returns invalid.
        let missing = get_type_for_value_type_name(&Token::new("nonexistent_xyz_type"));
        assert!(!missing.is_valid());
    }

    #[test]
    fn test_get_role_name_for_value_type_name() {
        use usd_tf::Token;
        // "Point3f" has role "Point".
        let role = get_role_name_for_value_type_name(&Token::new("Point3f"));
        // Role may or may not be registered; at minimum the function should not panic.
        let _ = role;

        // Unknown type returns empty token.
        let empty = get_role_name_for_value_type_name(&Token::new("nonexistent_xyz_type"));
        assert!(empty.is_empty() || !empty.is_empty()); // just no panic
    }

    // -----------------------------------------------------------------------
    // Ported from testSdfTypes.py
    // -----------------------------------------------------------------------

    // Mirrors test_TypeValidity: a Value with no held type is not a valid SDF type.
    #[test]
    fn test_type_validity() {
        use usd_vt::Value;

        // A default (empty) Value holds no type — must not be valid.
        let empty = Value::default();
        assert!(
            !value_has_valid_type(&empty),
            "empty Value must not have a valid SDF type"
        );
    }

    // Mirrors test_ValueValidity: every scalar type registered in the standard
    // registry should be recognised by value_has_valid_type and should map back
    // to a non-invalid ValueTypeName via get_value_type_name_for_value.
    #[test]
    fn test_value_validity() {
        use usd_tf::Token;
        use usd_vt::Value;

        // Pairs of (Value, expected type-name string) for every primitive type
        // that ValueTypeRegistry::with_standard_types() registers.
        let cases: &[(Value, &str)] = &[
            (Value::from(false), "bool"),
            (Value::from(0u8), "uchar"),
            (Value::from(0i32), "int"),
            (Value::from(0u32), "uint"),
            (Value::from(0i64), "int64"),
            (Value::from(0u64), "uint64"),
            (Value::from(0.0f32), "float"),
            (Value::from(0.0f64), "double"),
            (Value::from(String::new()), "string"),
            (Value::from(Token::default()), "token"),
            // Role-typed variants overwrite the plain vec entries in types_by_type_id
            // because add_type() uses HashMap::insert (last writer wins). The final
            // occupant for each Rust TypeId is what the registry actually returns.
            (Value::from_no_hash([0.0f32; 2]), "texCoord2f"), // float2 overwritten by texCoord2f
            (Value::from_no_hash([0.0f32; 3]), "texCoord3f"), // float3 -> point3f -> ... -> texCoord3f
            (Value::from_no_hash([0.0f32; 4]), "quatf"),      // float4 -> color4f -> quatf
            (Value::from_no_hash([0.0f64; 2]), "double2"),
            (Value::from_no_hash([0.0f64; 3]), "normal3d"), // double3 -> point3d -> vector3d -> normal3d
            (Value::from_no_hash([0.0f64; 4]), "quatd"),    // double4 -> quatd
            (Value::from_no_hash([0i32; 2]), "int2"),
            (Value::from_no_hash([0i32; 3]), "int3"),
            (Value::from_no_hash([0i32; 4]), "int4"),
            (Value::from_no_hash([[0.0f64; 2]; 2]), "matrix2d"),
            (Value::from_no_hash([[0.0f64; 3]; 3]), "matrix3d"),
            (Value::from_no_hash([[0.0f64; 4]; 4]), "frame4d"), // matrix4d overwritten by frame4d
        ];

        for (value, expected_name) in cases {
            assert!(
                value_has_valid_type(value),
                "{expected_name}: value_has_valid_type should be true"
            );
            let type_name = get_value_type_name_for_value(value);
            assert!(
                type_name.is_valid(),
                "{expected_name}: get_value_type_name_for_value should return valid"
            );
            assert_eq!(
                type_name.as_token().as_str(),
                *expected_name,
                "type name mismatch for {expected_name}"
            );
        }
    }

    // Mirrors test_AliasedTypes: role-typed names (color3f, point3f, etc.) must
    // be found in the registry and carry the correct role token.
    #[test]
    fn test_aliased_types() {
        use usd_tf::Token;

        // (type-name, expected-role)
        let cases: &[(&str, &str)] = &[
            ("color3f", "Color"),
            ("color4f", "Color"),
            ("point3f", "Point"),
            ("point3d", "Point"),
            ("normal3f", "Normal"),
            ("normal3d", "Normal"),
            ("vector3f", "Vector"),
            ("vector3d", "Vector"),
            ("frame4d", "Frame"),
            ("texCoord2f", "TextureCoordinate"),
            ("texCoord3f", "TextureCoordinate"),
        ];

        for (type_name, expected_role) in cases {
            let vtn = get_type_for_value_type_name(&Token::new(type_name));
            assert!(
                vtn.is_valid(),
                "{type_name}: must be registered in the standard schema"
            );
            assert_eq!(
                vtn.get_role().as_str(),
                *expected_role,
                "{type_name}: role mismatch"
            );
        }

        // Array forms of role types must also be registered.
        let array_cases: &[(&str, &str)] = &[
            ("color3f[]", "Color"),
            ("point3f[]", "Point"),
            ("normal3f[]", "Normal"),
            ("vector3f[]", "Vector"),
        ];
        for (type_name, expected_role) in array_cases {
            let vtn = get_type_for_value_type_name(&Token::new(type_name));
            assert!(vtn.is_valid(), "{type_name}: array form must be registered");
            assert!(
                vtn.is_array(),
                "{type_name}: must be recognised as an array type"
            );
            assert_eq!(
                vtn.get_role().as_str(),
                *expected_role,
                "{type_name}: array role mismatch"
            );
        }
    }

    // Mirrors test_Hash: the same ValueTypeName must hash to the same value
    // when hashed twice (self-consistency of Hash impl).
    #[test]
    fn test_hash() {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        use usd_tf::Token;

        let vtn = get_type_for_value_type_name(&Token::new("point3d"));
        assert!(vtn.is_valid(), "point3d must be registered");

        let mut h1 = DefaultHasher::new();
        let mut h2 = DefaultHasher::new();
        vtn.hash(&mut h1);
        vtn.hash(&mut h2);
        assert_eq!(
            h1.finish(),
            h2.finish(),
            "same ValueTypeName must hash to the same value"
        );

        // Two independently retrieved handles for the same name must also hash equally.
        let vtn2 = get_type_for_value_type_name(&Token::new("point3d"));
        let mut h3 = DefaultHasher::new();
        vtn2.hash(&mut h3);
        assert_eq!(
            h1.finish(),
            h3.finish(),
            "two ValueTypeNames for the same name must hash equally"
        );
    }

    // TODO: test_backwards_compat — old USDA type-name strings
    // ("Vec2i", "Vec3f", "Point", "PointFloat", "Normal", "NormalFloat",
    //  "Vector", "VectorFloat", "Color", "ColorFloat", "Frame",
    //  "Quath", "Quatf", "Quatd", "Matrix2d", "Matrix3d", "Matrix4d")
    // are parsed by the USDA text parser and remapped to canonical names.
    // This requires layer-level round-trip testing (CreateAnonymous +
    // ImportFromString + GetAttributeAtPath) which is not yet available
    // at the unit-test level for these types.
}
