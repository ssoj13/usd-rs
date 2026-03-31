
//! Command API for render delegates.
//!
//! Corresponds to pxr/imaging/hd/command.h.
//! Describes invokable commands and their arguments.

use std::collections::HashMap;
use usd_tf::Token;
use usd_vt::Value;

/// Command argument descriptor.
///
/// Corresponds to C++ `HdCommandArgDescriptor`.
#[derive(Debug, Clone)]
pub struct HdCommandArgDescriptor {
    /// Argument name.
    pub arg_name: Token,
    /// Default value for the argument.
    pub default_value: Value,
}

impl HdCommandArgDescriptor {
    /// Create a new command argument descriptor.
    pub fn new(arg_name: Token, default_value: Value) -> Self {
        Self {
            arg_name,
            default_value,
        }
    }
}

/// List of command argument descriptors.
pub type HdCommandArgDescriptors = Vec<HdCommandArgDescriptor>;

/// Command arguments as token -> value map.
///
/// Corresponds to C++ `HdCommandArgs` = VtDictionary.
pub type HdCommandArgs = HashMap<Token, Value>;

/// Descriptor for an invokable command.
///
/// Corresponds to C++ `HdCommandDescriptor`.
#[derive(Debug, Clone)]
pub struct HdCommandDescriptor {
    /// Token representing the command.
    pub command_name: Token,
    /// Human-readable description for UI.
    pub command_description: String,
    /// List of supported arguments.
    pub command_args: HdCommandArgDescriptors,
}

impl HdCommandDescriptor {
    /// Create a new command descriptor.
    pub fn new(name: Token, description: impl Into<String>, args: HdCommandArgDescriptors) -> Self {
        Self {
            command_name: name,
            command_description: description.into(),
            command_args: args,
        }
    }
}

/// List of command descriptors.
pub type HdCommandDescriptors = Vec<HdCommandDescriptor>;
