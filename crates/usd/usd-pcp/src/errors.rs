//! PCP error types.
//!
//! Errors that can occur during prim index composition.
//!
//! # C++ Parity
//!
//! This is a port of `pxr/usd/pcp/errors.h` and `errors.cpp`.
//!
//! # Error Types
//!
//! Composition errors fall into several categories:
//!
//! - **Cycle errors**: Circular dependencies between arcs or sublayers
//! - **Permission errors**: Accessing private prims, properties, or targets
//! - **Path errors**: Invalid prim paths, asset paths, or target paths
//! - **Offset errors**: Invalid layer offsets on references/sublayers
//! - **Capacity errors**: Exceeded composition limits
//! - **Consistency errors**: Conflicting property definitions
//! - **Relocation errors**: Invalid relocates operations

use std::fmt;
use std::sync::Arc;

use crate::{ArcType, Site, SiteTracker};
use usd_sdf::{LayerHandle, LayerOffset, Path, SpecType, Variability};

// ============================================================================
// Error Type Enum
// ============================================================================

/// Enum to indicate the type represented by a PCP error.
///
/// Each error type corresponds to a specific composition problem that
/// can occur when building a prim index.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ErrorType {
    /// Arcs between nodes that form a cycle.
    #[default]
    ArcCycle = 0,

    /// Arcs not made due to permission restrictions.
    ArcPermissionDenied = 1,

    /// Arcs to prohibited child prims due to relocations.
    ArcToProhibitedChild = 2,

    /// Exceeded the prim index capacity.
    IndexCapacityExceeded = 3,

    /// Exceeded the arc capacity at a single site.
    ArcCapacityExceeded = 4,

    /// Exceeded the namespace depth capacity.
    ArcNamespaceDepthCapacityExceeded = 5,

    /// Properties with conflicting spec types.
    InconsistentPropertyType = 6,

    /// Attributes with conflicting value types.
    InconsistentAttributeType = 7,

    /// Attributes with conflicting variability.
    InconsistentAttributeVariability = 8,

    /// Internal asset path (should be external).
    InternalAssetPath = 9,

    /// Invalid prim path in reference/payload.
    InvalidPrimPath = 10,

    /// Invalid asset path in reference/payload.
    InvalidAssetPath = 11,

    /// Invalid target path pointing to an instance.
    InvalidInstanceTargetPath = 12,

    /// Invalid target path pointing outside scope.
    InvalidExternalTargetPath = 13,

    /// Invalid target or connection path.
    InvalidTargetPath = 14,

    /// Invalid layer offset on reference/payload.
    InvalidReferenceOffset = 15,

    /// Invalid layer offset on sublayer.
    InvalidSublayerOffset = 16,

    /// Sibling sublayers with same owner.
    InvalidSublayerOwnership = 17,

    /// Invalid sublayer path.
    InvalidSublayerPath = 18,

    /// Invalid variant selection.
    InvalidVariantSelection = 19,

    /// Muted asset path in reference/payload.
    MutedAssetPath = 20,

    /// Invalid authored relocation.
    InvalidAuthoredRelocation = 21,

    /// Relocation conflicts with another.
    InvalidConflictingRelocation = 22,

    /// Multiple relocations with same target.
    InvalidSameTargetRelocations = 23,

    /// Opinions at relocation source path.
    OpinionAtRelocationSource = 24,

    /// Illegal opinions about private prims.
    PrimPermissionDenied = 25,

    /// Illegal opinions about private properties.
    PropertyPermissionDenied = 26,

    /// Sublayer cycle detected.
    SublayerCycle = 27,

    /// Illegal opinions about private targets.
    TargetPermissionDenied = 28,

    /// Prim path could not be resolved.
    UnresolvedPrimPath = 29,

    /// Error evaluating a variable expression.
    VariableExpressionError = 30,
}

impl ErrorType {
    /// Returns the total number of error types.
    pub const COUNT: usize = 31;

    /// Returns the error type as a string.
    pub fn as_str(&self) -> &'static str {
        match self {
            ErrorType::ArcCycle => "ArcCycle",
            ErrorType::ArcPermissionDenied => "ArcPermissionDenied",
            ErrorType::ArcToProhibitedChild => "ArcToProhibitedChild",
            ErrorType::IndexCapacityExceeded => "IndexCapacityExceeded",
            ErrorType::ArcCapacityExceeded => "ArcCapacityExceeded",
            ErrorType::ArcNamespaceDepthCapacityExceeded => "ArcNamespaceDepthCapacityExceeded",
            ErrorType::InconsistentPropertyType => "InconsistentPropertyType",
            ErrorType::InconsistentAttributeType => "InconsistentAttributeType",
            ErrorType::InconsistentAttributeVariability => "InconsistentAttributeVariability",
            ErrorType::InternalAssetPath => "InternalAssetPath",
            ErrorType::InvalidPrimPath => "InvalidPrimPath",
            ErrorType::InvalidAssetPath => "InvalidAssetPath",
            ErrorType::InvalidInstanceTargetPath => "InvalidInstanceTargetPath",
            ErrorType::InvalidExternalTargetPath => "InvalidExternalTargetPath",
            ErrorType::InvalidTargetPath => "InvalidTargetPath",
            ErrorType::InvalidReferenceOffset => "InvalidReferenceOffset",
            ErrorType::InvalidSublayerOffset => "InvalidSublayerOffset",
            ErrorType::InvalidSublayerOwnership => "InvalidSublayerOwnership",
            ErrorType::InvalidSublayerPath => "InvalidSublayerPath",
            ErrorType::InvalidVariantSelection => "InvalidVariantSelection",
            ErrorType::MutedAssetPath => "MutedAssetPath",
            ErrorType::InvalidAuthoredRelocation => "InvalidAuthoredRelocation",
            ErrorType::InvalidConflictingRelocation => "InvalidConflictingRelocation",
            ErrorType::InvalidSameTargetRelocations => "InvalidSameTargetRelocations",
            ErrorType::OpinionAtRelocationSource => "OpinionAtRelocationSource",
            ErrorType::PrimPermissionDenied => "PrimPermissionDenied",
            ErrorType::PropertyPermissionDenied => "PropertyPermissionDenied",
            ErrorType::SublayerCycle => "SublayerCycle",
            ErrorType::TargetPermissionDenied => "TargetPermissionDenied",
            ErrorType::UnresolvedPrimPath => "UnresolvedPrimPath",
            ErrorType::VariableExpressionError => "VariableExpressionError",
        }
    }

    /// Returns the display name for this error type.
    pub fn display_name(&self) -> &'static str {
        match self {
            ErrorType::ArcCycle => "arc cycle",
            ErrorType::ArcPermissionDenied => "arc permission denied",
            ErrorType::ArcToProhibitedChild => "arc to prohibited child",
            ErrorType::IndexCapacityExceeded => "index capacity exceeded",
            ErrorType::ArcCapacityExceeded => "arc capacity exceeded",
            ErrorType::ArcNamespaceDepthCapacityExceeded => "arc namespace depth capacity exceeded",
            ErrorType::InconsistentPropertyType => "inconsistent property type",
            ErrorType::InconsistentAttributeType => "inconsistent attribute type",
            ErrorType::InconsistentAttributeVariability => "inconsistent attribute variability",
            ErrorType::InternalAssetPath => "internal asset path",
            ErrorType::InvalidPrimPath => "invalid prim path",
            ErrorType::InvalidAssetPath => "invalid asset path",
            ErrorType::InvalidInstanceTargetPath => "invalid instance target path",
            ErrorType::InvalidExternalTargetPath => "invalid external target path",
            ErrorType::InvalidTargetPath => "invalid target path",
            ErrorType::InvalidReferenceOffset => "invalid reference offset",
            ErrorType::InvalidSublayerOffset => "invalid sublayer offset",
            ErrorType::InvalidSublayerOwnership => "invalid sublayer ownership",
            ErrorType::InvalidSublayerPath => "invalid sublayer path",
            ErrorType::InvalidVariantSelection => "invalid variant selection",
            ErrorType::MutedAssetPath => "muted asset path",
            ErrorType::InvalidAuthoredRelocation => "invalid authored relocation",
            ErrorType::InvalidConflictingRelocation => "invalid conflicting relocation",
            ErrorType::InvalidSameTargetRelocations => "invalid same target relocations",
            ErrorType::OpinionAtRelocationSource => "opinion at relocation source",
            ErrorType::PrimPermissionDenied => "prim permission denied",
            ErrorType::PropertyPermissionDenied => "property permission denied",
            ErrorType::SublayerCycle => "sublayer cycle",
            ErrorType::TargetPermissionDenied => "target permission denied",
            ErrorType::UnresolvedPrimPath => "unresolved prim path",
            ErrorType::VariableExpressionError => "variable expression error",
        }
    }

    /// Returns true if this is a cycle-related error.
    pub fn is_cycle_error(&self) -> bool {
        matches!(self, ErrorType::ArcCycle | ErrorType::SublayerCycle)
    }

    /// Returns true if this is a permission-related error.
    pub fn is_permission_error(&self) -> bool {
        matches!(
            self,
            ErrorType::ArcPermissionDenied
                | ErrorType::PrimPermissionDenied
                | ErrorType::PropertyPermissionDenied
                | ErrorType::TargetPermissionDenied
        )
    }

    /// Returns true if this is a path-related error.
    pub fn is_path_error(&self) -> bool {
        matches!(
            self,
            ErrorType::InvalidPrimPath
                | ErrorType::InvalidAssetPath
                | ErrorType::InvalidInstanceTargetPath
                | ErrorType::InvalidExternalTargetPath
                | ErrorType::InvalidTargetPath
                | ErrorType::InvalidSublayerPath
                | ErrorType::UnresolvedPrimPath
                | ErrorType::InternalAssetPath
                | ErrorType::MutedAssetPath
        )
    }

    /// Returns true if this is an offset-related error.
    pub fn is_offset_error(&self) -> bool {
        matches!(
            self,
            ErrorType::InvalidReferenceOffset | ErrorType::InvalidSublayerOffset
        )
    }

    /// Returns true if this is a capacity-related error.
    pub fn is_capacity_error(&self) -> bool {
        matches!(
            self,
            ErrorType::IndexCapacityExceeded
                | ErrorType::ArcCapacityExceeded
                | ErrorType::ArcNamespaceDepthCapacityExceeded
        )
    }

    /// Returns true if this is a consistency-related error.
    pub fn is_consistency_error(&self) -> bool {
        matches!(
            self,
            ErrorType::InconsistentPropertyType
                | ErrorType::InconsistentAttributeType
                | ErrorType::InconsistentAttributeVariability
        )
    }

    /// Returns true if this is a relocation-related error.
    pub fn is_relocation_error(&self) -> bool {
        matches!(
            self,
            ErrorType::ArcToProhibitedChild
                | ErrorType::InvalidAuthoredRelocation
                | ErrorType::InvalidConflictingRelocation
                | ErrorType::InvalidSameTargetRelocations
                | ErrorType::OpinionAtRelocationSource
        )
    }
}

impl fmt::Display for ErrorType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// ============================================================================
// Error Base Trait
// ============================================================================

/// Base trait for all PCP errors.
pub trait PcpError: fmt::Debug + Send + Sync {
    /// Returns the error type.
    fn error_type(&self) -> ErrorType;

    /// Returns the root site of the composed prim/property being computed.
    fn root_site(&self) -> &Site;

    /// Returns a mutable reference to the root site.
    fn root_site_mut(&mut self) -> &mut Site;

    /// Converts error to string message.
    fn to_string(&self) -> String;
}

/// Shared pointer to a PCP error.
pub type PcpErrorBasePtr = Arc<dyn PcpError>;

/// Vector of PCP errors.
pub type PcpErrorVector = Vec<PcpErrorBasePtr>;

// ============================================================================
// Arc Cycle Error
// ============================================================================

/// Arcs between PcpNodes that form a cycle.
#[derive(Debug, Clone, Default)]
pub struct ErrorArcCycle {
    /// The root site of the composed prim being computed.
    pub root_site: Site,
    /// The cycle of sites forming the cycle.
    pub cycle: SiteTracker,
}

impl ErrorArcCycle {
    /// Creates a new arc cycle error.
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }
}

impl PcpError for ErrorArcCycle {
    fn error_type(&self) -> ErrorType {
        ErrorType::ArcCycle
    }

    fn root_site(&self) -> &Site {
        &self.root_site
    }

    fn root_site_mut(&mut self) -> &mut Site {
        &mut self.root_site
    }

    fn to_string(&self) -> String {
        if self.cycle.is_empty() {
            return String::new();
        }

        let mut msg = String::from("Cycle detected:\n");
        for (i, segment) in self.cycle.iter().enumerate() {
            if i > 0 {
                if i + 1 < self.cycle.len() {
                    msg.push_str(match segment.arc_type {
                        ArcType::Inherit => "inherits from:\n",
                        ArcType::Relocate => "is relocated from:\n",
                        ArcType::Variant => "uses variant:\n",
                        ArcType::Reference => "references:\n",
                        ArcType::Payload => "gets payload from:\n",
                        _ => "refers to:\n",
                    });
                } else {
                    msg.push_str("CANNOT ");
                    msg.push_str(match segment.arc_type {
                        ArcType::Inherit => "inherit from:\n",
                        ArcType::Relocate => "be relocated from:\n",
                        ArcType::Variant => "use variant:\n",
                        ArcType::Reference => "reference:\n",
                        ArcType::Payload => "get payload from:\n",
                        _ => "refer to:\n",
                    });
                }
            }
            msg.push_str(&format!("{}\n", segment.site));
            if i > 0 && i + 1 < self.cycle.len() {
                msg.push_str("which ");
            }
        }
        msg
    }
}

// ============================================================================
// Arc Permission Denied Error
// ============================================================================

/// Arcs not made between PcpNodes because of permission restrictions.
#[derive(Debug, Clone, Default)]
pub struct ErrorArcPermissionDenied {
    /// The root site.
    pub root_site: Site,
    /// The site where the invalid arc was expressed.
    pub site: Site,
    /// The private, invalid target of the arc.
    pub private_site: Site,
    /// The type of arc.
    pub arc_type: ArcType,
}

impl ErrorArcPermissionDenied {
    /// Creates a new arc permission denied error.
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }
}

impl PcpError for ErrorArcPermissionDenied {
    fn error_type(&self) -> ErrorType {
        ErrorType::ArcPermissionDenied
    }

    fn root_site(&self) -> &Site {
        &self.root_site
    }

    fn root_site_mut(&mut self) -> &mut Site {
        &mut self.root_site
    }

    fn to_string(&self) -> String {
        let action = match self.arc_type {
            ArcType::Inherit => "inherit from",
            ArcType::Relocate => "be relocated from",
            ArcType::Variant => "use variant",
            ArcType::Reference => "reference",
            ArcType::Payload => "get payload from",
            _ => "refer to",
        };
        format!(
            "{}\nCANNOT {}:\n{}\nwhich is private.",
            self.site, action, self.private_site
        )
    }
}

// ============================================================================
// Arc To Prohibited Child Error
// ============================================================================

/// Arcs not made because the target is a prohibited child prim due to relocations.
#[derive(Debug, Clone, Default)]
pub struct ErrorArcToProhibitedChild {
    /// The root site.
    pub root_site: Site,
    /// The site where the invalid arc was expressed.
    pub site: Site,
    /// The target site of the invalid arc which is a prohibited child.
    pub target_site: Site,
    /// The site of the node that is a relocation source.
    pub relocation_source_site: Site,
    /// The type of arc.
    pub arc_type: ArcType,
}

impl ErrorArcToProhibitedChild {
    /// Creates a new arc to prohibited child error.
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }
}

impl PcpError for ErrorArcToProhibitedChild {
    fn error_type(&self) -> ErrorType {
        ErrorType::ArcToProhibitedChild
    }

    fn root_site(&self) -> &Site {
        &self.root_site
    }

    fn root_site_mut(&mut self) -> &mut Site {
        &mut self.root_site
    }

    fn to_string(&self) -> String {
        let action = match self.arc_type {
            ArcType::Inherit => "inherit from",
            ArcType::Relocate => "be relocated from",
            ArcType::Variant => "use variant",
            ArcType::Reference => "reference",
            ArcType::Payload => "get payload from",
            _ => "refer to",
        };
        format!(
            "{}\nCANNOT {}:\n{}\nwhich is a prohibited child of its parent \
             because it would require allowing opinions from the source of a \
             relocation at {}.",
            self.site, action, self.target_site, self.relocation_source_site
        )
    }
}

// ============================================================================
// Capacity Exceeded Error
// ============================================================================

/// Exceeded the capacity for composition arcs at a single site.
#[derive(Debug, Clone, Default)]
pub struct ErrorCapacityExceeded {
    /// The root site.
    pub root_site: Site,
    /// The specific error type (IndexCapacity, ArcCapacity, or ArcNamespaceDepthCapacity).
    pub error_type: ErrorType,
}

impl ErrorCapacityExceeded {
    /// Creates a new capacity exceeded error.
    pub fn new(error_type: ErrorType) -> Arc<Self> {
        Arc::new(Self {
            root_site: Site::default(),
            error_type,
        })
    }
}

impl PcpError for ErrorCapacityExceeded {
    fn error_type(&self) -> ErrorType {
        self.error_type
    }

    fn root_site(&self) -> &Site {
        &self.root_site
    }

    fn root_site_mut(&mut self) -> &mut Site {
        &mut self.root_site
    }

    fn to_string(&self) -> String {
        format!(
            "Composition graph capacity exceeded: {}",
            self.error_type.display_name()
        )
    }
}

// ============================================================================
// Inconsistent Property Type Error
// ============================================================================

/// Properties that have specs with conflicting definitions.
#[derive(Debug, Clone, Default)]
pub struct ErrorInconsistentPropertyType {
    /// The root site.
    pub root_site: Site,
    /// The identifier of the layer with the defining property spec.
    pub defining_layer_identifier: String,
    /// The path of the defining property spec.
    pub defining_spec_path: Path,
    /// The type of the defining spec.
    pub defining_spec_type: SpecType,
    /// The identifier of the layer with the conflicting property spec.
    pub conflicting_layer_identifier: String,
    /// The path of the conflicting property spec.
    pub conflicting_spec_path: Path,
    /// The type of the conflicting spec.
    pub conflicting_spec_type: SpecType,
}

impl ErrorInconsistentPropertyType {
    /// Creates a new inconsistent property type error.
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }
}

impl PcpError for ErrorInconsistentPropertyType {
    fn error_type(&self) -> ErrorType {
        ErrorType::InconsistentPropertyType
    }

    fn root_site(&self) -> &Site {
        &self.root_site
    }

    fn root_site_mut(&mut self) -> &mut Site {
        &mut self.root_site
    }

    fn to_string(&self) -> String {
        let defining_type = if self.defining_spec_type == SpecType::Attribute {
            "an attribute"
        } else {
            "a relationship"
        };
        let conflicting_type = if self.conflicting_spec_type == SpecType::Attribute {
            "an attribute"
        } else {
            "a relationship"
        };
        format!(
            "The property <{}> has inconsistent spec types. \
             The defining spec is @{}@<{}> and is {} spec. \
             The conflicting spec is @{}@<{}> and is {} spec. \
             The conflicting spec will be ignored.",
            self.root_site.path.as_str(),
            self.defining_layer_identifier,
            self.defining_spec_path.as_str(),
            defining_type,
            self.conflicting_layer_identifier,
            self.conflicting_spec_path.as_str(),
            conflicting_type
        )
    }
}

// ============================================================================
// Inconsistent Attribute Type Error
// ============================================================================

/// Attributes that have specs with conflicting definitions.
#[derive(Debug, Clone, Default)]
pub struct ErrorInconsistentAttributeType {
    /// The root site.
    pub root_site: Site,
    /// The identifier of the layer with the defining attribute spec.
    pub defining_layer_identifier: String,
    /// The path of the defining attribute spec.
    pub defining_spec_path: Path,
    /// The value type from the defining spec.
    pub defining_value_type: String,
    /// The identifier of the layer with the conflicting attribute spec.
    pub conflicting_layer_identifier: String,
    /// The path of the conflicting attribute spec.
    pub conflicting_spec_path: Path,
    /// The value type from the conflicting spec.
    pub conflicting_value_type: String,
}

impl ErrorInconsistentAttributeType {
    /// Creates a new inconsistent attribute type error.
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }
}

impl PcpError for ErrorInconsistentAttributeType {
    fn error_type(&self) -> ErrorType {
        ErrorType::InconsistentAttributeType
    }

    fn root_site(&self) -> &Site {
        &self.root_site
    }

    fn root_site_mut(&mut self) -> &mut Site {
        &mut self.root_site
    }

    fn to_string(&self) -> String {
        format!(
            "The attribute <{}> has specs with inconsistent value types. \
             The defining spec is @{}@<{}> with value type '{}'. \
             The conflicting spec is @{}@<{}> with value type '{}'. \
             The conflicting spec will be ignored.",
            self.root_site.path.as_str(),
            self.defining_layer_identifier,
            self.defining_spec_path.as_str(),
            self.defining_value_type,
            self.conflicting_layer_identifier,
            self.conflicting_spec_path.as_str(),
            self.conflicting_value_type
        )
    }
}

// ============================================================================
// Inconsistent Attribute Variability Error
// ============================================================================

/// Attributes that have specs with conflicting variability.
#[derive(Debug, Clone, Default)]
pub struct ErrorInconsistentAttributeVariability {
    /// The root site.
    pub root_site: Site,
    /// The identifier of the layer with the defining attribute spec.
    pub defining_layer_identifier: String,
    /// The path of the defining attribute spec.
    pub defining_spec_path: Path,
    /// The variability of the defining spec.
    pub defining_variability: Variability,
    /// The identifier of the layer with the conflicting attribute spec.
    pub conflicting_layer_identifier: String,
    /// The path of the conflicting attribute spec.
    pub conflicting_spec_path: Path,
    /// The variability of the conflicting spec.
    pub conflicting_variability: Variability,
}

impl ErrorInconsistentAttributeVariability {
    /// Creates a new inconsistent attribute variability error.
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }
}

impl PcpError for ErrorInconsistentAttributeVariability {
    fn error_type(&self) -> ErrorType {
        ErrorType::InconsistentAttributeVariability
    }

    fn root_site(&self) -> &Site {
        &self.root_site
    }

    fn root_site_mut(&mut self) -> &mut Site {
        &mut self.root_site
    }

    fn to_string(&self) -> String {
        format!(
            "The attribute <{}> has specs with inconsistent variability. \
             The defining spec is @{}@<{}> with variability '{}'. \
             The conflicting spec is @{}@<{}> with variability '{}'. \
             The conflicting variability will be ignored.",
            self.root_site.path.as_str(),
            self.defining_layer_identifier,
            self.defining_spec_path.as_str(),
            self.defining_variability,
            self.conflicting_layer_identifier,
            self.conflicting_spec_path.as_str(),
            self.conflicting_variability
        )
    }
}

// ============================================================================
// Invalid Prim Path Error
// ============================================================================

/// Invalid prim paths used by references or payloads.
#[derive(Debug, Clone, Default)]
pub struct ErrorInvalidPrimPath {
    /// The root site.
    pub root_site: Site,
    /// The site where the invalid arc was expressed.
    pub site: Site,
    /// The target prim path of the arc that is invalid.
    pub prim_path: Path,
    /// The source layer of the spec that caused this arc.
    pub source_layer: LayerHandle,
    /// The arc type.
    pub arc_type: ArcType,
}

impl ErrorInvalidPrimPath {
    /// Creates a new invalid prim path error.
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }
}

impl PcpError for ErrorInvalidPrimPath {
    fn error_type(&self) -> ErrorType {
        ErrorType::InvalidPrimPath
    }

    fn root_site(&self) -> &Site {
        &self.root_site
    }

    fn root_site_mut(&mut self) -> &mut Site {
        &mut self.root_site
    }

    fn to_string(&self) -> String {
        let layer_id = self
            .source_layer
            .upgrade()
            .map(|l| l.identifier().to_string())
            .unwrap_or_else(|| "<expired>".to_string());
        format!(
            "Invalid {} path <{}> introduced by @{}@<{}> \
             -- must be an absolute prim path with no variant selections.",
            self.arc_type.display_name(),
            self.prim_path.as_str(),
            layer_id,
            self.site.path.as_str()
        )
    }
}

// ============================================================================
// Invalid Asset Path Error
// ============================================================================

/// Invalid asset paths used by references or payloads.
#[derive(Debug, Clone, Default)]
pub struct ErrorInvalidAssetPath {
    /// The root site.
    pub root_site: Site,
    /// The site where the invalid arc was expressed.
    pub site: Site,
    /// The target prim path of the arc.
    pub target_path: Path,
    /// The target asset path of the arc as authored.
    pub asset_path: String,
    /// The resolved target asset path of the arc.
    pub resolved_asset_path: String,
    /// The source layer of the spec that caused this arc.
    pub source_layer: LayerHandle,
    /// The arc type.
    pub arc_type: ArcType,
    /// Additional provided error information.
    pub messages: String,
}

impl ErrorInvalidAssetPath {
    /// Creates a new invalid asset path error.
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }
}

impl PcpError for ErrorInvalidAssetPath {
    fn error_type(&self) -> ErrorType {
        ErrorType::InvalidAssetPath
    }

    fn root_site(&self) -> &Site {
        &self.root_site
    }

    fn root_site_mut(&mut self) -> &mut Site {
        &mut self.root_site
    }

    fn to_string(&self) -> String {
        let layer_id = self
            .source_layer
            .upgrade()
            .map(|l| l.identifier().to_string())
            .unwrap_or_else(|| "<expired>".to_string());
        let msg_part = if self.messages.is_empty() {
            String::new()
        } else {
            format!(" -- {}", self.messages)
        };
        format!(
            "Could not open asset @{}@ for {} introduced by @{}@<{}>{}.",
            self.resolved_asset_path,
            self.arc_type.display_name(),
            layer_id,
            self.site.path.as_str(),
            msg_part
        )
    }
}

// ============================================================================
// Muted Asset Path Error
// ============================================================================

/// Muted asset paths used by references or payloads.
#[derive(Debug, Clone, Default)]
pub struct ErrorMutedAssetPath {
    /// The root site.
    pub root_site: Site,
    /// The site where the arc was expressed.
    pub site: Site,
    /// The target prim path of the arc.
    pub target_path: Path,
    /// The target asset path of the arc as authored.
    pub asset_path: String,
    /// The resolved target asset path of the arc.
    pub resolved_asset_path: String,
    /// The source layer of the spec that caused this arc.
    pub source_layer: LayerHandle,
    /// The arc type.
    pub arc_type: ArcType,
}

impl ErrorMutedAssetPath {
    /// Creates a new muted asset path error.
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }
}

impl PcpError for ErrorMutedAssetPath {
    fn error_type(&self) -> ErrorType {
        ErrorType::MutedAssetPath
    }

    fn root_site(&self) -> &Site {
        &self.root_site
    }

    fn root_site_mut(&mut self) -> &mut Site {
        &mut self.root_site
    }

    fn to_string(&self) -> String {
        let layer_id = self
            .source_layer
            .upgrade()
            .map(|l| l.identifier().to_string())
            .unwrap_or_else(|| "<expired>".to_string());
        format!(
            "Asset @{}@ was muted for {} introduced by @{}@<{}>.",
            self.resolved_asset_path,
            self.arc_type.display_name(),
            layer_id,
            self.site.path.as_str()
        )
    }
}

// ============================================================================
// Invalid Instance Target Path Error
// ============================================================================

/// Invalid target or connection path authored in an inherited class
/// that points to an instance of that class.
#[derive(Debug, Clone, Default)]
pub struct ErrorInvalidInstanceTargetPath {
    /// The root site.
    pub root_site: Site,
    /// The invalid target or connection path that was authored.
    pub target_path: Path,
    /// The path to the property where the target was authored.
    pub owning_path: Path,
    /// The spec type of the property where the target was authored.
    pub owner_spec_type: SpecType,
    /// The layer containing the property where the target was authored.
    pub layer: LayerHandle,
    /// The target or connection path in the composed scene.
    pub composed_target_path: Path,
}

impl ErrorInvalidInstanceTargetPath {
    /// Creates a new invalid instance target path error.
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }
}

impl PcpError for ErrorInvalidInstanceTargetPath {
    fn error_type(&self) -> ErrorType {
        ErrorType::InvalidInstanceTargetPath
    }

    fn root_site(&self) -> &Site {
        &self.root_site
    }

    fn root_site_mut(&mut self) -> &mut Site {
        &mut self.root_site
    }

    fn to_string(&self) -> String {
        let prop_type = if self.owner_spec_type == SpecType::Attribute {
            "attribute connection"
        } else {
            "relationship target"
        };
        let layer_id = self
            .layer
            .upgrade()
            .map(|l| l.identifier().to_string())
            .unwrap_or_else(|| "<expired>".to_string());
        format!(
            "The {} <{}> from <{}> in layer @{}@ is authored in a class \
             but refers to an instance of that class. Ignoring.",
            prop_type,
            self.target_path.as_str(),
            self.owning_path.as_str(),
            layer_id
        )
    }
}

// ============================================================================
// Invalid External Target Path Error
// ============================================================================

/// Invalid target or connection path in some scope that points to
/// an object outside of that scope.
#[derive(Debug, Clone, Default)]
pub struct ErrorInvalidExternalTargetPath {
    /// The root site.
    pub root_site: Site,
    /// The invalid target or connection path that was authored.
    pub target_path: Path,
    /// The path to the property where the target was authored.
    pub owning_path: Path,
    /// The spec type of the property where the target was authored.
    pub owner_spec_type: SpecType,
    /// The layer containing the property where the target was authored.
    pub layer: LayerHandle,
    /// The target or connection path in the composed scene.
    pub composed_target_path: Path,
    /// The arc type of the owning property.
    pub owner_arc_type: ArcType,
    /// The introduction path of the owner.
    pub owner_intro_path: Path,
}

impl ErrorInvalidExternalTargetPath {
    /// Creates a new invalid external target path error.
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }
}

impl PcpError for ErrorInvalidExternalTargetPath {
    fn error_type(&self) -> ErrorType {
        ErrorType::InvalidExternalTargetPath
    }

    fn root_site(&self) -> &Site {
        &self.root_site
    }

    fn root_site_mut(&mut self) -> &mut Site {
        &mut self.root_site
    }

    fn to_string(&self) -> String {
        let prop_type = if self.owner_spec_type == SpecType::Attribute {
            "attribute connection"
        } else {
            "relationship target"
        };
        let layer_id = self
            .layer
            .upgrade()
            .map(|l| l.identifier().to_string())
            .unwrap_or_else(|| "<expired>".to_string());
        format!(
            "The {} <{}> from <{}> in layer @{}@ refers to a path outside \
             the scope of the {} from <{}>. Ignoring.",
            prop_type,
            self.target_path.as_str(),
            self.owning_path.as_str(),
            layer_id,
            self.owner_arc_type.display_name(),
            self.owner_intro_path.as_str()
        )
    }
}

// ============================================================================
// Invalid Target Path Error
// ============================================================================

/// Invalid target or connection path.
#[derive(Debug, Clone, Default)]
pub struct ErrorInvalidTargetPath {
    /// The root site.
    pub root_site: Site,
    /// The invalid target or connection path that was authored.
    pub target_path: Path,
    /// The path to the property where the target was authored.
    pub owning_path: Path,
    /// The spec type of the property where the target was authored.
    pub owner_spec_type: SpecType,
    /// The layer containing the property where the target was authored.
    pub layer: LayerHandle,
    /// The target or connection path in the composed scene.
    pub composed_target_path: Path,
}

impl ErrorInvalidTargetPath {
    /// Creates a new invalid target path error.
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }
}

impl PcpError for ErrorInvalidTargetPath {
    fn error_type(&self) -> ErrorType {
        ErrorType::InvalidTargetPath
    }

    fn root_site(&self) -> &Site {
        &self.root_site
    }

    fn root_site_mut(&mut self) -> &mut Site {
        &mut self.root_site
    }

    fn to_string(&self) -> String {
        let prop_type = if self.owner_spec_type == SpecType::Attribute {
            "attribute connection"
        } else {
            "relationship target"
        };
        let layer_id = self
            .layer
            .upgrade()
            .map(|l| l.identifier().to_string())
            .unwrap_or_else(|| "<expired>".to_string());
        format!(
            "The {} <{}> from <{}> in layer @{}@ is invalid. This may be \
             because the path is the pre-relocated source path of a \
             relocated prim. Ignoring.",
            prop_type,
            self.target_path.as_str(),
            self.owning_path.as_str(),
            layer_id
        )
    }
}

// ============================================================================
// Invalid Sublayer Offset Error
// ============================================================================

/// Sublayers that use invalid layer offsets.
#[derive(Debug, Clone, Default)]
pub struct ErrorInvalidSublayerOffset {
    /// The root site.
    pub root_site: Site,
    /// The parent layer.
    pub layer: LayerHandle,
    /// The sublayer with the invalid offset.
    pub sublayer: LayerHandle,
    /// The invalid offset.
    pub offset: LayerOffset,
}

impl ErrorInvalidSublayerOffset {
    /// Creates a new invalid sublayer offset error.
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }
}

impl PcpError for ErrorInvalidSublayerOffset {
    fn error_type(&self) -> ErrorType {
        ErrorType::InvalidSublayerOffset
    }

    fn root_site(&self) -> &Site {
        &self.root_site
    }

    fn root_site_mut(&mut self) -> &mut Site {
        &mut self.root_site
    }

    fn to_string(&self) -> String {
        let layer_id = self
            .layer
            .upgrade()
            .map(|l| l.identifier().to_string())
            .unwrap_or_else(|| "<expired>".to_string());
        let sublayer_id = self
            .sublayer
            .upgrade()
            .map(|l| l.identifier().to_string())
            .unwrap_or_else(|| "<expired>".to_string());
        format!(
            "Invalid sublayer offset {} in sublayer @{}@ of layer @{}@. \
             Using no offset instead.",
            self.offset, sublayer_id, layer_id
        )
    }
}

// ============================================================================
// Invalid Reference Offset Error
// ============================================================================

/// References or payloads that use invalid layer offsets.
#[derive(Debug, Clone, Default)]
pub struct ErrorInvalidReferenceOffset {
    /// The root site.
    pub root_site: Site,
    /// The source layer of the spec that caused this arc.
    pub source_layer: LayerHandle,
    /// The source path of the spec that caused this arc.
    pub source_path: Path,
    /// Target asset path of the arc.
    pub asset_path: String,
    /// Target prim path of the arc.
    pub target_path: Path,
    /// The invalid layer offset expressed on the arc.
    pub offset: LayerOffset,
    /// The arc type.
    pub arc_type: ArcType,
}

impl ErrorInvalidReferenceOffset {
    /// Creates a new invalid reference offset error.
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }
}

impl PcpError for ErrorInvalidReferenceOffset {
    fn error_type(&self) -> ErrorType {
        ErrorType::InvalidReferenceOffset
    }

    fn root_site(&self) -> &Site {
        &self.root_site
    }

    fn root_site_mut(&mut self) -> &mut Site {
        &mut self.root_site
    }

    fn to_string(&self) -> String {
        let layer_id = self
            .source_layer
            .upgrade()
            .map(|l| l.identifier().to_string())
            .unwrap_or_else(|| "<expired>".to_string());
        format!(
            "Invalid {} offset {} for @{}@<{}> introduced by @{}@<{}>. \
             Using no offset instead.",
            self.arc_type.display_name(),
            self.offset,
            self.asset_path,
            self.target_path.as_str(),
            layer_id,
            self.source_path.as_str()
        )
    }
}

// ============================================================================
// Invalid Sublayer Ownership Error
// ============================================================================

/// Sibling layers that have the same owner.
#[derive(Debug, Clone, Default)]
pub struct ErrorInvalidSublayerOwnership {
    /// The root site.
    pub root_site: Site,
    /// The owner string.
    pub owner: String,
    /// The parent layer.
    pub layer: LayerHandle,
    /// The sublayers with the same owner.
    pub sublayers: Vec<LayerHandle>,
}

impl ErrorInvalidSublayerOwnership {
    /// Creates a new invalid sublayer ownership error.
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }
}

impl PcpError for ErrorInvalidSublayerOwnership {
    fn error_type(&self) -> ErrorType {
        ErrorType::InvalidSublayerOwnership
    }

    fn root_site(&self) -> &Site {
        &self.root_site
    }

    fn root_site_mut(&mut self) -> &mut Site {
        &mut self.root_site
    }

    fn to_string(&self) -> String {
        let layer_id = self
            .layer
            .upgrade()
            .map(|l| l.identifier().to_string())
            .unwrap_or_else(|| "<expired>".to_string());
        let sublayer_strs: Vec<String> = self
            .sublayers
            .iter()
            .map(|sl| {
                sl.upgrade()
                    .map(|l| format!("@{}@", l.identifier()))
                    .unwrap_or_else(|| "@<expired>@".to_string())
            })
            .collect();
        format!(
            "The following sublayers for layer @{}@ have the same owner '{}': {}",
            layer_id,
            self.owner,
            sublayer_strs.join(", ")
        )
    }
}

// ============================================================================
// Invalid Sublayer Path Error
// ============================================================================

/// Asset paths that could not be both resolved and loaded.
#[derive(Debug, Clone, Default)]
pub struct ErrorInvalidSublayerPath {
    /// The root site.
    pub root_site: Site,
    /// The parent layer.
    pub layer: LayerHandle,
    /// The sublayer path that could not be loaded.
    pub sublayer_path: String,
    /// Additional error messages.
    pub messages: String,
}

impl ErrorInvalidSublayerPath {
    /// Creates a new invalid sublayer path error.
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }
}

impl PcpError for ErrorInvalidSublayerPath {
    fn error_type(&self) -> ErrorType {
        ErrorType::InvalidSublayerPath
    }

    fn root_site(&self) -> &Site {
        &self.root_site
    }

    fn root_site_mut(&mut self) -> &mut Site {
        &mut self.root_site
    }

    fn to_string(&self) -> String {
        let layer_id = self
            .layer
            .upgrade()
            .map(|l| l.identifier().to_string())
            .unwrap_or_else(|| "<NULL>".to_string());
        let msg_part = if self.messages.is_empty() {
            String::new()
        } else {
            format!(" -- {}", self.messages)
        };
        format!(
            "Could not load sublayer @{}@ of layer @{}@{}; skipping.",
            self.sublayer_path, layer_id, msg_part
        )
    }
}

// ============================================================================
// Invalid Authored Relocation Error
// ============================================================================

/// Invalid authored relocation found in a relocates field.
#[derive(Debug, Clone, Default)]
pub struct ErrorInvalidAuthoredRelocation {
    /// The root site.
    pub root_site: Site,
    /// The source path of the invalid relocation.
    pub source_path: Path,
    /// The target path of the invalid relocation.
    pub target_path: Path,
    /// The layer containing the authored relocates.
    pub layer: LayerHandle,
    /// The path to the prim where the relocates is authored.
    pub owning_path: Path,
    /// Additional messages about the error.
    pub messages: String,
}

impl ErrorInvalidAuthoredRelocation {
    /// Creates a new invalid authored relocation error.
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }
}

impl PcpError for ErrorInvalidAuthoredRelocation {
    fn error_type(&self) -> ErrorType {
        ErrorType::InvalidAuthoredRelocation
    }

    fn root_site(&self) -> &Site {
        &self.root_site
    }

    fn root_site_mut(&mut self) -> &mut Site {
        &mut self.root_site
    }

    fn to_string(&self) -> String {
        let layer_id = self
            .layer
            .upgrade()
            .map(|l| l.identifier().to_string())
            .unwrap_or_else(|| "<expired>".to_string());
        format!(
            "Relocation from <{}> to <{}> authored at @{}@<{}> is invalid \
             and will be ignored: {}",
            self.source_path.as_str(),
            self.target_path.as_str(),
            layer_id,
            self.owning_path.as_str(),
            self.messages
        )
    }
}

// ============================================================================
// Invalid Conflicting Relocation Error
// ============================================================================

/// Enumeration of reasons a relocate can be in conflict with another relocate.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ConflictReason {
    /// The target of a relocate is the source of another relocate.
    #[default]
    TargetIsConflictSource,
    /// The source of a relocate is the target of another relocate.
    SourceIsConflictTarget,
    /// The target of a relocate is a descendant of the source of another relocate.
    TargetIsConflictSourceDescendant,
    /// The source of a relocate is a descendant of the source of another relocate.
    SourceIsConflictSourceDescendant,
}

impl ConflictReason {
    /// Returns the reason as a string.
    pub fn as_str(&self) -> &'static str {
        match self {
            ConflictReason::TargetIsConflictSource => {
                "The target of a relocate cannot be the source of another relocate in the same layer stack."
            }
            ConflictReason::SourceIsConflictTarget => {
                "The source of a relocate cannot be the target of another relocate in the same layer stack."
            }
            ConflictReason::TargetIsConflictSourceDescendant => {
                "The target of a relocate cannot be a descendant of the source of another relocate."
            }
            ConflictReason::SourceIsConflictSourceDescendant => {
                "The source of a relocate cannot be a descendant of the source of another relocate."
            }
        }
    }
}

/// Relocation conflicts with another relocation in the layer stack.
#[derive(Debug, Clone, Default)]
pub struct ErrorInvalidConflictingRelocation {
    /// The root site.
    pub root_site: Site,
    /// The source path of the invalid relocation.
    pub source_path: Path,
    /// The target path of the invalid relocation.
    pub target_path: Path,
    /// The layer containing the authored relocates.
    pub layer: LayerHandle,
    /// The path to the prim where the relocates is authored.
    pub owning_path: Path,
    /// The source path of the relocation this conflicts with.
    pub conflict_source_path: Path,
    /// The target path of the relocation this conflicts with.
    pub conflict_target_path: Path,
    /// The layer containing the authored relocation this conflicts with.
    pub conflict_layer: LayerHandle,
    /// The path to the prim where the relocation this conflicts with is authored.
    pub conflict_owning_path: Path,
    /// The reason the relocate is a conflict.
    pub conflict_reason: ConflictReason,
}

impl ErrorInvalidConflictingRelocation {
    /// Creates a new invalid conflicting relocation error.
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }
}

impl PcpError for ErrorInvalidConflictingRelocation {
    fn error_type(&self) -> ErrorType {
        ErrorType::InvalidConflictingRelocation
    }

    fn root_site(&self) -> &Site {
        &self.root_site
    }

    fn root_site_mut(&mut self) -> &mut Site {
        &mut self.root_site
    }

    fn to_string(&self) -> String {
        let layer_id = self
            .layer
            .upgrade()
            .map(|l| l.identifier().to_string())
            .unwrap_or_else(|| "<expired>".to_string());
        let conflict_layer_id = self
            .conflict_layer
            .upgrade()
            .map(|l| l.identifier().to_string())
            .unwrap_or_else(|| "<expired>".to_string());
        format!(
            "Relocation from <{}> to <{}> authored at @{}@<{}> conflicts with \
             another relocation from <{}> to <{}> authored at @{}@<{}> and will \
             be ignored: {}",
            self.source_path.as_str(),
            self.target_path.as_str(),
            layer_id,
            self.owning_path.as_str(),
            self.conflict_source_path.as_str(),
            self.conflict_target_path.as_str(),
            conflict_layer_id,
            self.conflict_owning_path.as_str(),
            self.conflict_reason.as_str()
        )
    }
}

// ============================================================================
// Invalid Same Target Relocations Error
// ============================================================================

/// Info about each relocate source that has the same target path.
#[derive(Debug, Clone, Default)]
pub struct RelocationSource {
    /// The source path of the invalid relocation.
    pub source_path: Path,
    /// The layer containing the authored relocates.
    pub layer: LayerHandle,
    /// The path to the prim where the relocates is authored.
    pub owning_path: Path,
}

/// Multiple relocations in the layer stack have the same target.
#[derive(Debug, Clone, Default)]
pub struct ErrorInvalidSameTargetRelocations {
    /// The root site.
    pub root_site: Site,
    /// The target path of the multiple invalid relocations.
    pub target_path: Path,
    /// The sources of all relocates that relocate to the target path.
    pub sources: Vec<RelocationSource>,
}

impl ErrorInvalidSameTargetRelocations {
    /// Creates a new invalid same target relocations error.
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }
}

impl PcpError for ErrorInvalidSameTargetRelocations {
    fn error_type(&self) -> ErrorType {
        ErrorType::InvalidSameTargetRelocations
    }

    fn root_site(&self) -> &Site {
        &self.root_site
    }

    fn root_site_mut(&mut self) -> &mut Site {
        &mut self.root_site
    }

    fn to_string(&self) -> String {
        if self.sources.is_empty() {
            return String::new();
        }

        let sources_str: Vec<String> = self
            .sources
            .iter()
            .map(|s| {
                let layer_id = s
                    .layer
                    .upgrade()
                    .map(|l| l.identifier().to_string())
                    .unwrap_or_else(|| "<expired>".to_string());
                format!(
                    "relocation from <{}> authored at @{}@<{}>",
                    s.source_path.as_str(),
                    layer_id,
                    s.owning_path.as_str()
                )
            })
            .collect();

        format!(
            "The path <{}> is the target of multiple relocations from different \
             sources. The following relocates to this target are invalid and will \
             be ignored: {}.",
            self.target_path.as_str(),
            sources_str.join("; ")
        )
    }
}

// ============================================================================
// Opinion At Relocation Source Error
// ============================================================================

/// Opinions were found at a relocation source path.
#[derive(Debug, Clone, Default)]
pub struct ErrorOpinionAtRelocationSource {
    /// The root site.
    pub root_site: Site,
    /// The layer with the opinion.
    pub layer: LayerHandle,
    /// The path of the opinion.
    pub path: Path,
}

impl ErrorOpinionAtRelocationSource {
    /// Creates a new opinion at relocation source error.
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }
}

impl PcpError for ErrorOpinionAtRelocationSource {
    fn error_type(&self) -> ErrorType {
        ErrorType::OpinionAtRelocationSource
    }

    fn root_site(&self) -> &Site {
        &self.root_site
    }

    fn root_site_mut(&mut self) -> &mut Site {
        &mut self.root_site
    }

    fn to_string(&self) -> String {
        let layer_id = self
            .layer
            .upgrade()
            .map(|l| l.identifier().to_string())
            .unwrap_or_else(|| "<expired>".to_string());
        format!(
            "The layer @{}@ has an invalid opinion at the relocation source \
             path <{}>, which will be ignored.",
            layer_id,
            self.path.as_str()
        )
    }
}

// ============================================================================
// Prim Permission Denied Error
// ============================================================================

/// Layers with illegal opinions about private prims.
#[derive(Debug, Clone, Default)]
pub struct ErrorPrimPermissionDenied {
    /// The root site.
    pub root_site: Site,
    /// The site where the invalid arc was expressed.
    pub site: Site,
    /// The private, invalid target of the arc.
    pub private_site: Site,
}

impl ErrorPrimPermissionDenied {
    /// Creates a new prim permission denied error.
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }
}

impl PcpError for ErrorPrimPermissionDenied {
    fn error_type(&self) -> ErrorType {
        ErrorType::PrimPermissionDenied
    }

    fn root_site(&self) -> &Site {
        &self.root_site
    }

    fn root_site_mut(&mut self) -> &mut Site {
        &mut self.root_site
    }

    fn to_string(&self) -> String {
        format!(
            "{}\nwill be ignored because:\n{}\nis private and overrides its opinions.",
            self.site, self.private_site
        )
    }
}

// ============================================================================
// Property Permission Denied Error
// ============================================================================

/// Layers with illegal opinions about private properties.
#[derive(Debug, Clone, Default)]
pub struct ErrorPropertyPermissionDenied {
    /// The root site.
    pub root_site: Site,
    /// The property path.
    pub prop_path: Path,
    /// The property spec type.
    pub prop_type: SpecType,
    /// The layer path.
    pub layer_path: String,
}

impl ErrorPropertyPermissionDenied {
    /// Creates a new property permission denied error.
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }
}

impl PcpError for ErrorPropertyPermissionDenied {
    fn error_type(&self) -> ErrorType {
        ErrorType::PropertyPermissionDenied
    }

    fn root_site(&self) -> &Site {
        &self.root_site
    }

    fn root_site_mut(&mut self) -> &mut Site {
        &mut self.root_site
    }

    fn to_string(&self) -> String {
        let prop_type = if self.prop_type == SpecType::Attribute {
            "an attribute"
        } else {
            "a relationship"
        };
        format!(
            "The layer at @{}@ has an illegal opinion about {} <{}> which is \
             private across a reference, inherit, or variant. Ignoring.",
            self.layer_path,
            prop_type,
            self.prop_path.as_str()
        )
    }
}

// ============================================================================
// Sublayer Cycle Error
// ============================================================================

/// Layers that recursively sublayer themselves.
#[derive(Debug, Clone, Default)]
pub struct ErrorSublayerCycle {
    /// The root site.
    pub root_site: Site,
    /// The root layer of the sublayer hierarchy.
    pub layer: LayerHandle,
    /// The sublayer that was seen for the second time.
    pub sublayer: LayerHandle,
}

impl ErrorSublayerCycle {
    /// Creates a new sublayer cycle error.
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }
}

impl PcpError for ErrorSublayerCycle {
    fn error_type(&self) -> ErrorType {
        ErrorType::SublayerCycle
    }

    fn root_site(&self) -> &Site {
        &self.root_site
    }

    fn root_site_mut(&mut self) -> &mut Site {
        &mut self.root_site
    }

    fn to_string(&self) -> String {
        let layer_id = self
            .layer
            .upgrade()
            .map(|l| l.identifier().to_string())
            .unwrap_or_else(|| "<expired>".to_string());
        let sublayer_id = self
            .sublayer
            .upgrade()
            .map(|l| l.identifier().to_string())
            .unwrap_or_else(|| "<expired>".to_string());
        format!(
            "Sublayer hierarchy with root layer @{}@ has cycles. Detected when \
             layer @{}@ was seen in the layer stack for the second time.",
            layer_id, sublayer_id
        )
    }
}

// ============================================================================
// Target Permission Denied Error
// ============================================================================

/// Paths with illegal opinions about private targets.
#[derive(Debug, Clone, Default)]
pub struct ErrorTargetPermissionDenied {
    /// The root site.
    pub root_site: Site,
    /// The target path.
    pub target_path: Path,
    /// The path to the property where the target was authored.
    pub owning_path: Path,
    /// The spec type of the property where the target was authored.
    pub owner_spec_type: SpecType,
    /// The layer containing the property where the target was authored.
    pub layer: LayerHandle,
    /// The target path in the composed scene.
    pub composed_target_path: Path,
}

impl ErrorTargetPermissionDenied {
    /// Creates a new target permission denied error.
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }
}

impl PcpError for ErrorTargetPermissionDenied {
    fn error_type(&self) -> ErrorType {
        ErrorType::TargetPermissionDenied
    }

    fn root_site(&self) -> &Site {
        &self.root_site
    }

    fn root_site_mut(&mut self) -> &mut Site {
        &mut self.root_site
    }

    fn to_string(&self) -> String {
        let (prop_type, conn_type) = if self.owner_spec_type == SpecType::Attribute {
            ("attribute connection", "connection")
        } else {
            ("relationship target", "target")
        };
        let layer_id = self
            .layer
            .upgrade()
            .map(|l| l.identifier().to_string())
            .unwrap_or_else(|| "<expired>".to_string());
        format!(
            "The {} <{}> from <{}> in layer @{}@ targets an object that is \
             private on the far side of a reference or inherit. This {} will \
             be ignored.",
            prop_type,
            self.target_path.as_str(),
            self.owning_path.as_str(),
            layer_id,
            conn_type
        )
    }
}

// ============================================================================
// Unresolved Prim Path Error
// ============================================================================

/// Asset paths that could not be both resolved and loaded.
#[derive(Debug, Clone, Default)]
pub struct ErrorUnresolvedPrimPath {
    /// The root site.
    pub root_site: Site,
    /// The site where the invalid arc was expressed.
    pub site: Site,
    /// The source layer of the spec that caused this arc.
    pub source_layer: LayerHandle,
    /// The target layer of the arc.
    pub target_layer: LayerHandle,
    /// The prim path that cannot be resolved on the target layer stack.
    pub unresolved_path: Path,
    /// The arc type.
    pub arc_type: ArcType,
}

impl ErrorUnresolvedPrimPath {
    /// Creates a new unresolved prim path error.
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }
}

impl PcpError for ErrorUnresolvedPrimPath {
    fn error_type(&self) -> ErrorType {
        ErrorType::UnresolvedPrimPath
    }

    fn root_site(&self) -> &Site {
        &self.root_site
    }

    fn root_site_mut(&mut self) -> &mut Site {
        &mut self.root_site
    }

    fn to_string(&self) -> String {
        let source_layer_id = self
            .source_layer
            .upgrade()
            .map(|l| l.identifier().to_string())
            .unwrap_or_else(|| "<expired>".to_string());
        let target_layer_id = self
            .target_layer
            .upgrade()
            .map(|l| l.identifier().to_string())
            .unwrap_or_else(|| "<expired>".to_string());
        format!(
            "Unresolved {} prim path @{}@<{}> introduced by @{}@<{}>",
            self.arc_type.display_name(),
            target_layer_id,
            self.unresolved_path.as_str(),
            source_layer_id,
            self.site.path.as_str()
        )
    }
}

// ============================================================================
// Variable Expression Error
// ============================================================================

/// Error when evaluating a variable expression.
#[derive(Debug, Clone, Default)]
pub struct ErrorVariableExpressionError {
    /// The root site.
    pub root_site: Site,
    /// The expression that was evaluated.
    pub expression: String,
    /// The error generated during evaluation.
    pub expression_error: String,
    /// The context where the expression was authored (e.g., "sublayer", "reference").
    pub context: String,
    /// The source layer where the expression was authored.
    pub source_layer: LayerHandle,
    /// The source path where the expression was authored.
    pub source_path: Path,
}

impl ErrorVariableExpressionError {
    /// Creates a new variable expression error.
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }
}

impl PcpError for ErrorVariableExpressionError {
    fn error_type(&self) -> ErrorType {
        ErrorType::VariableExpressionError
    }

    fn root_site(&self) -> &Site {
        &self.root_site
    }

    fn root_site_mut(&mut self) -> &mut Site {
        &mut self.root_site
    }

    fn to_string(&self) -> String {
        let layer_id = self
            .source_layer
            .upgrade()
            .map(|l| l.identifier().to_string())
            .unwrap_or_else(|| "<expired>".to_string());
        let source_str = if self.source_path.is_absolute_root_path() {
            format!("in @{}@", layer_id)
        } else {
            format!("at {} in @{}@", self.source_path.as_str(), layer_id)
        };
        // Truncate expression to 32 chars for readability
        let expr_preview: String = self.expression.chars().take(32).collect();
        format!(
            "Error evaluating expression {} for {} {}: {}",
            expr_preview, self.context, source_str, self.expression_error
        )
    }
}

// ============================================================================
// Raise Errors Function
// ============================================================================

/// Raise the given errors as runtime errors (prints them).
pub fn raise_errors(errors: &PcpErrorVector) {
    for error in errors {
        eprintln!("PCP Error: {}", error.to_string());
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_type_count() {
        assert_eq!(ErrorType::COUNT, 31);
    }

    #[test]
    fn test_error_type_as_str() {
        assert_eq!(ErrorType::ArcCycle.as_str(), "ArcCycle");
        assert_eq!(ErrorType::InvalidAssetPath.as_str(), "InvalidAssetPath");
        assert_eq!(ErrorType::SublayerCycle.as_str(), "SublayerCycle");
    }

    #[test]
    fn test_error_type_display() {
        assert_eq!(format!("{}", ErrorType::ArcCycle), "ArcCycle");
        assert_eq!(format!("{}", ErrorType::InvalidPrimPath), "InvalidPrimPath");
    }

    #[test]
    fn test_is_cycle_error() {
        assert!(ErrorType::ArcCycle.is_cycle_error());
        assert!(ErrorType::SublayerCycle.is_cycle_error());
        assert!(!ErrorType::InvalidAssetPath.is_cycle_error());
    }

    #[test]
    fn test_is_permission_error() {
        assert!(ErrorType::ArcPermissionDenied.is_permission_error());
        assert!(ErrorType::PrimPermissionDenied.is_permission_error());
        assert!(ErrorType::PropertyPermissionDenied.is_permission_error());
        assert!(ErrorType::TargetPermissionDenied.is_permission_error());
        assert!(!ErrorType::ArcCycle.is_permission_error());
    }

    #[test]
    fn test_is_path_error() {
        assert!(ErrorType::InvalidPrimPath.is_path_error());
        assert!(ErrorType::InvalidAssetPath.is_path_error());
        assert!(ErrorType::InvalidTargetPath.is_path_error());
        assert!(ErrorType::UnresolvedPrimPath.is_path_error());
        assert!(!ErrorType::ArcCycle.is_path_error());
    }

    #[test]
    fn test_error_arc_cycle_empty() {
        let err = ErrorArcCycle::default();
        assert!(err.to_string().is_empty());
    }

    #[test]
    fn test_error_arc_permission_denied() {
        use crate::LayerStackIdentifier;
        let mut err = ErrorArcPermissionDenied::default();
        err.arc_type = ArcType::Reference;
        err.site = Site::new(
            LayerStackIdentifier::new("test.usda"),
            Path::from_string("/A").unwrap(),
        );
        err.private_site = Site::new(
            LayerStackIdentifier::new("test.usda"),
            Path::from_string("/B").unwrap(),
        );
        let msg = err.to_string();
        assert!(msg.contains("CANNOT reference"));
        assert!(msg.contains("which is private"));
    }

    #[test]
    fn test_error_capacity_exceeded() {
        let err = ErrorCapacityExceeded {
            root_site: Site::default(),
            error_type: ErrorType::ArcCapacityExceeded,
        };
        let msg = err.to_string();
        assert!(msg.contains("Composition graph capacity exceeded"));
    }

    #[test]
    fn test_error_inconsistent_property_type() {
        use crate::LayerStackIdentifier;
        let mut err = ErrorInconsistentPropertyType::default();
        err.root_site = Site::new(
            LayerStackIdentifier::new("root.usda"),
            Path::from_string("/A.prop").unwrap(),
        );
        err.defining_layer_identifier = "layer1.usda".to_string();
        err.defining_spec_path = Path::from_string("/A.prop").unwrap();
        err.defining_spec_type = SpecType::Attribute;
        err.conflicting_layer_identifier = "layer2.usda".to_string();
        err.conflicting_spec_path = Path::from_string("/A.prop").unwrap();
        err.conflicting_spec_type = SpecType::Relationship;
        let msg = err.to_string();
        assert!(msg.contains("inconsistent spec types"));
        assert!(msg.contains("an attribute"));
        assert!(msg.contains("a relationship"));
    }

    #[test]
    fn test_conflict_reason() {
        assert_eq!(
            ConflictReason::TargetIsConflictSource.as_str(),
            "The target of a relocate cannot be the source of another relocate in the same layer stack."
        );
    }

    #[test]
    fn test_error_sublayer_cycle() {
        let err = ErrorSublayerCycle::default();
        let msg = err.to_string();
        assert!(msg.contains("has cycles"));
    }

    #[test]
    fn test_error_variable_expression() {
        let mut err = ErrorVariableExpressionError::default();
        err.expression = "${FOO}".to_string();
        err.expression_error = "undefined variable".to_string();
        err.context = "sublayer".to_string();
        err.source_path = Path::absolute_root();
        let msg = err.to_string();
        assert!(msg.contains("Error evaluating expression"));
        assert!(msg.contains("sublayer"));
    }
}
