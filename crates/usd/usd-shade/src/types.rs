//! USD Shade types - type definitions for usdShade module.
//!
//! Port of pxr/usd/usdShade/types.h

use std::collections::HashMap;
use std::fmt;
use usd_core::attribute::Attribute;
use usd_tf::Token;

/// Specifies the type of a shading attribute.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AttributeType {
    /// Invalid attribute type.
    Invalid,
    /// Input attribute.
    Input,
    /// Output attribute.
    Output,
}

impl fmt::Display for AttributeType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AttributeType::Invalid => write!(f, "Invalid"),
            AttributeType::Input => write!(f, "Input"),
            AttributeType::Output => write!(f, "Output"),
        }
    }
}

impl Default for AttributeType {
    fn default() -> Self {
        AttributeType::Invalid
    }
}

/// Choice when creating a single connection with the ConnectToSource method
/// for a shading attribute. The new connection can replace any existing
/// connections or be added to the list of existing connections.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConnectionModification {
    /// Replace any existing connections.
    Replace,
    /// Prepend to the list of existing connections.
    Prepend,
    /// Append to the list of existing connections.
    Append,
}

impl fmt::Display for ConnectionModification {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConnectionModification::Replace => write!(f, "Replace"),
            ConnectionModification::Prepend => write!(f, "Prepend"),
            ConnectionModification::Append => write!(f, "Append"),
        }
    }
}

impl Default for ConnectionModification {
    fn default() -> Self {
        ConnectionModification::Replace
    }
}

/// Type alias for SdrTokenMap - a map from Token to String.
/// Used for shader registry metadata.
pub type SdrTokenMap = HashMap<Token, String>;

/// Small vector optimized for single-element case (matches TfSmallVector<UsdAttribute, 1>).
/// Used for reporting attributes where single connection is common.
/// Using Vec for now - can optimize later with smallvec if needed.
pub type AttributeVector = Vec<Attribute>;

/// Small vector optimized for single-element case (matches TfSmallVector<UsdShadeConnectionSourceInfo, 1>).
/// Full definition of ConnectionSourceInfo lives in connectable_api.rs.
/// Using Vec for now - can optimize later with smallvec if needed.
pub type SourceInfoVector = Vec<super::connectable_api::ConnectionSourceInfo>;
