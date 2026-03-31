//! HwImplementation — base for HW node implementations.
//! By ref MaterialX HwImplementation.h/.cpp

use std::collections::HashSet;

/// Coordinate space identifiers matching stdlib enum order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum Space {
    Model = 0,
    Object = 1,
    World = 2,
}

/// Internal string constants used by HW implementations.
pub const SPACE: &str = "space";
pub const INDEX: &str = "index";
pub const GEOMPROP: &str = "geomprop";

/// Input names whose modification requires shader recompilation (not editable at runtime).
fn immutable_inputs() -> HashSet<&'static str> {
    ["index", "space", "attrname"].into_iter().collect()
}

/// Returns true if the input is editable (runtime-modifiable).
/// Matches C++ HwImplementation::isEditable.
pub fn is_editable_hw(input_name: &str) -> bool {
    !immutable_inputs().contains(input_name)
}
