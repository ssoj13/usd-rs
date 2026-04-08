//! USD Shade ConnectableAPIBehavior - defines compatibility and behavior for ConnectableAPI.
//!
//! Port of pxr/usd/usdShade/connectableAPIBehavior.h and connectableAPIBehavior.cpp
//!
//! UsdShadeConnectableAPIBehavior defines the compatibility and behavior
//! of UsdShadeConnectableAPI when applied to a particular prim type.

use super::connectable_api::ConnectableAPI;
use super::input::Input;
use super::output::Output;
use super::tokens::tokens;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use usd_core::attribute::Attribute;
use usd_core::prim::Prim;

/// An enum describing the types of connectable nodes which will govern what
/// connectibility rule is invoked for these.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectableNodeTypes {
    /// Shader, NodeGraph
    BasicNodes,
    /// Material, etc
    DerivedContainerNodes,
}

/// UsdShadeConnectableAPIBehavior defines the compatibility and behavior
/// of UsdShadeConnectableAPI when applied to a particular prim type.
///
/// This enables schema libraries to enable UsdShadeConnectableAPI for
/// their prim types and define its behavior.
pub trait ConnectableAPIBehavior: Send + Sync {
    /// Returns true if the connection is allowed, false otherwise.
    ///
    /// The prim owning the input is guaranteed to be of the type this
    /// behavior was registered with. The function must be thread-safe.
    fn can_connect_input_to_source(
        &self,
        input: &Input,
        source: &Attribute,
        reason: &mut Option<String>,
    ) -> bool;

    /// Returns true if the connection is allowed, false otherwise.
    ///
    /// The prim owning the output is guaranteed to be of the type this
    /// behavior was registered with. The function must be thread-safe.
    fn can_connect_output_to_source(
        &self,
        output: &Output,
        source: &Attribute,
        reason: &mut Option<String>,
    ) -> bool;

    /// Returns true if the associated prim type is considered a "container" for connected nodes.
    fn is_container(&self) -> bool;

    /// Determines if the behavior should respect container encapsulation rules.
    fn requires_encapsulation(&self) -> bool;
}

/// Default implementation of ConnectableAPIBehavior.
///
/// By default we want a connectableBehavior to not exhibit a container like
/// behavior. And we want encapsulation behavior enabled by default.
pub struct DefaultConnectableAPIBehavior {
    is_container: bool,
    requires_encapsulation: bool,
}

impl DefaultConnectableAPIBehavior {
    /// Creates a new default behavior.
    pub fn new() -> Self {
        Self {
            is_container: false,
            requires_encapsulation: true,
        }
    }

    /// Creates a new behavior with specified container and encapsulation settings.
    pub fn with_settings(is_container: bool, requires_encapsulation: bool) -> Self {
        Self {
            is_container,
            requires_encapsulation,
        }
    }
}

impl ConnectableAPIBehavior for DefaultConnectableAPIBehavior {
    fn can_connect_input_to_source(
        &self,
        input: &Input,
        source: &Attribute,
        reason: &mut Option<String>,
    ) -> bool {
        Self::_can_connect_input_to_source(
            input,
            source,
            reason,
            ConnectableNodeTypes::BasicNodes,
            self.requires_encapsulation,
        )
    }

    fn can_connect_output_to_source(
        &self,
        output: &Output,
        source: &Attribute,
        reason: &mut Option<String>,
    ) -> bool {
        Self::_can_connect_output_to_source(
            output,
            source,
            reason,
            ConnectableNodeTypes::BasicNodes,
            self.requires_encapsulation,
        )
    }

    fn is_container(&self) -> bool {
        self.is_container
    }

    fn requires_encapsulation(&self) -> bool {
        self.requires_encapsulation
    }
}

impl DefaultConnectableAPIBehavior {
    /// Helper function to separate and share special connectivity logic.
    fn _can_connect_input_to_source(
        input: &Input,
        source: &Attribute,
        reason: &mut Option<String>,
        node_type: ConnectableNodeTypes,
        requires_encapsulation: bool,
    ) -> bool {
        if !input.is_defined() {
            if let Some(r) = reason {
                *r = format!("Invalid input: {}", input.as_attribute().path());
            }
            return false;
        }

        if !source.is_valid() {
            if let Some(r) = reason {
                *r = format!("Invalid source: {}", source.path());
            }
            return false;
        }

        // Ensure that the source prim is the closest ancestor container of the
        // NodeGraph owning the input.
        let encapsulation_check_for_input_sources = |input: &Input,
                                                     source: &Attribute,
                                                     reason: &mut Option<String>|
         -> bool {
            let input_prim = input.get_prim();
            let input_prim_path = input_prim.path();
            let _source_prim_path = source.path().get_prim_path();

            let source_prim_path = source.path().get_prim_path();
            let Some(stage) = source.stage() else {
                if let Some(r) = reason {
                    *r = format!("Cannot get stage for source attribute: {}", source.path());
                }
                return false;
            };
            let Some(source_prim) = stage.get_prim_at_path(&source_prim_path) else {
                if let Some(r) = reason {
                    *r = format!("Cannot get prim for source attribute: {}", source.path());
                }
                return false;
            };
            let source_connectable = ConnectableAPI::new(source_prim);
            if !source_connectable.is_container() {
                if let Some(r) = reason {
                    *r = format!(
                        "Encapsulation check failed - prim '{}' owning the input source '{}' is not a container.",
                        source_prim_path,
                        source.name().as_str()
                    );
                }
                return false;
            }

            // C++ connectableAPIBehavior.cpp:91:
            //   inputPrimPath.GetParentPath() != sourcePrimPath
            // The source prim must BE the parent of the input prim (parent-child
            // relationship), NOT share the same parent (sibling relationship).
            if input_prim_path.get_parent_path() != source_prim_path {
                if let Some(r) = reason {
                    *r = format!(
                        "Encapsulation check failed - input source prim '{}' is not the closest ancestor container of the NodeGraph '{}' owning the input attribute '{}'.",
                        source_prim_path,
                        input_prim_path,
                        input.get_full_name().as_str()
                    );
                }
                return false;
            }

            true
        };

        // Ensure that the source prim and input prim are contained by the same
        // inner most container for all nodes, other than DerivedContainerNodes.
        let encapsulation_check_for_output_sources = |input: &Input,
                                                      source: &Attribute,
                                                      node_type: ConnectableNodeTypes,
                                                      reason: &mut Option<String>|
         -> bool {
            let input_prim = input.get_prim();
            let input_prim_path = input_prim.path();
            let source_prim_path = source.path().get_prim_path();

            match node_type {
                ConnectableNodeTypes::DerivedContainerNodes => {
                    let input_connectable = ConnectableAPI::new(input_prim.clone());
                    if !input_connectable.is_container() {
                        if let Some(r) = reason {
                            *r = format!(
                                "Encapsulation check failed - For input's prim type '{}', prim owning the input '{}' is not a container.",
                                input_prim.type_name().as_str(),
                                input.as_attribute().path()
                            );
                        }
                        return false;
                    }
                    // C++: sourcePrimPath.GetParentPath() != inputPrimPath
                    // Source must be a direct child of the input's prim
                    if &source_prim_path.get_parent_path() != input_prim_path {
                        if let Some(r) = reason {
                            *r = format!(
                                "Encapsulation check failed - For input's prim type '{}', Output source's prim '{}' is not an immediate descendent of the input's prim '{}'.",
                                input_prim.type_name().as_str(),
                                source_prim_path,
                                input_prim_path
                            );
                        }
                        return false;
                    }
                    true
                }
                ConnectableNodeTypes::BasicNodes => {
                    let input_parent_path = input_prim_path.get_parent_path();
                    if input_parent_path.is_empty() {
                        if let Some(r) = reason {
                            *r = format!(
                                "Encapsulation check failed - Input prim '{}' has no parent.",
                                input_prim_path
                            );
                        }
                        return false;
                    }

                    let parent_prim = input_prim.parent();
                    if !parent_prim.is_valid() {
                        if let Some(r) = reason {
                            *r = format!(
                                "Encapsulation check failed - Cannot get parent prim for input prim '{}'.",
                                input_prim_path
                            );
                        }
                        return false;
                    }

                    let parent_connectable = ConnectableAPI::new(parent_prim);
                    if !parent_connectable.is_container() {
                        if let Some(r) = reason {
                            *r = format!(
                                "Encapsulation check failed - For input's prim type '{}', Immediate ancestor '{}' for the prim owning the output source '{}' is not a container.",
                                input_prim.type_name().as_str(),
                                input_parent_path,
                                source.path()
                            );
                        }
                        return false;
                    }

                    let input_parent = input_prim_path.get_parent_path();
                    let source_parent = source_prim_path.get_parent_path();
                    if input_parent != source_parent {
                        if let Some(r) = reason {
                            *r = format!(
                                "Encapsulation check failed - For input's prim type '{}', Input's prim '{}' and source's prim '{}' are not contained by the same container prim.",
                                input_prim.type_name().as_str(),
                                input_prim_path,
                                source_prim_path
                            );
                        }
                        return false;
                    }
                    true
                }
            }
        };

        let input_connectability = input.get_connectability();

        if input_connectability == tokens().full {
            if Input::is_input(source) {
                if !requires_encapsulation
                    || encapsulation_check_for_input_sources(input, source, reason)
                {
                    return true;
                }
                return false;
            }
            // source is an output - allow connection
            if !requires_encapsulation
                || encapsulation_check_for_output_sources(input, source, node_type, reason)
            {
                return true;
            }
            false
        } else if input_connectability == tokens().interface_only {
            if Input::is_input(source) {
                let source_input = Input::from_attribute(source.clone());
                let source_connectability = source_input.get_connectability();
                if source_connectability == tokens().interface_only {
                    if !requires_encapsulation
                        || encapsulation_check_for_input_sources(input, source, reason)
                    {
                        return true;
                    }
                    false
                } else {
                    if let Some(r) = reason {
                        *r = "Input connectability is 'interfaceOnly' and source does not have 'interfaceOnly' connectability.".to_string();
                    }
                    false
                }
            } else {
                if let Some(r) = reason {
                    *r = "Input connectability is 'interfaceOnly' but source is not an input"
                        .to_string();
                }
                false
            }
        } else {
            if let Some(r) = reason {
                *r = "Input connectability is unspecified".to_string();
            }
            false
        }
    }

    /// Helper function for output connections.
    fn _can_connect_output_to_source(
        output: &Output,
        source: &Attribute,
        reason: &mut Option<String>,
        node_type: ConnectableNodeTypes,
        requires_encapsulation: bool,
    ) -> bool {
        // Nodegraphs allow connections to their outputs, but only from internal nodes.
        if !output.is_defined() {
            if let Some(r) = reason {
                *r = "Invalid output".to_string();
            }
            return false;
        }
        if !source.is_valid() {
            if let Some(r) = reason {
                *r = "Invalid source".to_string();
            }
            return false;
        }

        let source_prim_path = source.path().get_prim_path();
        let output_prim = output.get_prim();
        let output_prim_path = output_prim.path();

        if Input::is_input(source) {
            // passthrough usage is not allowed for DerivedContainerNodes
            if node_type == ConnectableNodeTypes::DerivedContainerNodes {
                if let Some(r) = reason {
                    *r = format!(
                        "Encapsulation check failed - passthrough usage is not allowed for output prim '{}' of type '{}'.",
                        output_prim_path,
                        output_prim.type_name().as_str()
                    );
                }
                return false;
            }
            // output can connect to an input of the same container as a passthrough.
            if source_prim_path != *output_prim_path {
                if let Some(r) = reason {
                    if let Some(attr) = output.get_attr() {
                        *r = format!(
                            "Encapsulation check failed - output '{}' and input source '{}' must be encapsulated by the same container prim",
                            attr.path(),
                            source.path()
                        );
                    } else {
                        *r = format!(
                            "Encapsulation check failed - output and input source '{}' must be encapsulated by the same container prim",
                            source.path()
                        );
                    }
                }
                return false;
            }
            true
        } else {
            // Source is an output
            // output can connect to other node's output directly encapsulated by
            // it, unless explicitly marked to ignore encapsulation rule.

            // C++: sourcePrimPath.GetParentPath() != outputPrimPath
            // Source must be a direct child of the output's prim
            let source_parent = source_prim_path.get_parent_path();
            if requires_encapsulation && &source_parent != output_prim_path {
                if let Some(r) = reason {
                    if let Some(attr) = output.get_attr() {
                        *r = format!(
                            "Encapsulation check failed - prim owning the output '{}' is not an immediate descendent of the prim owning the output source '{}'.",
                            attr.path(),
                            source.path()
                        );
                    } else {
                        *r = format!(
                            "Encapsulation check failed - prim owning the output is not an immediate descendent of the prim owning the output source '{}'.",
                            source.path()
                        );
                    }
                }
                return false;
            }

            true
        }
    }
}

/// Shader-specific ConnectableAPIBehavior.
///
/// Per C++ shader.cpp:121-131: Shader outputs are NOT connectable.
/// isContainer=false, requiresEncapsulation=true (defaults).
pub struct ShaderConnectableAPIBehavior;

impl ConnectableAPIBehavior for ShaderConnectableAPIBehavior {
    fn can_connect_input_to_source(
        &self,
        input: &Input,
        source: &Attribute,
        reason: &mut Option<String>,
    ) -> bool {
        DefaultConnectableAPIBehavior::_can_connect_input_to_source(
            input,
            source,
            reason,
            ConnectableNodeTypes::BasicNodes,
            true, // requiresEncapsulation
        )
    }

    /// Shader outputs are not connectable (C++ returns false unconditionally).
    fn can_connect_output_to_source(
        &self,
        _output: &Output,
        _source: &Attribute,
        reason: &mut Option<String>,
    ) -> bool {
        if let Some(r) = reason {
            *r = "Shader outputs are not connectable".to_string();
        }
        false
    }

    fn is_container(&self) -> bool {
        false
    }

    fn requires_encapsulation(&self) -> bool {
        true
    }
}

/// Material-specific ConnectableAPIBehavior.
///
/// Per C++ material.cpp:726-751: isContainer=true, requiresEncapsulation=true.
/// Both input and output connections use DerivedContainerNodes rules:
/// - Passthrough (output->input on same container) is BLOCKED.
/// - Input source must be a direct descendant of the material prim.
pub struct MaterialConnectableAPIBehavior;

impl ConnectableAPIBehavior for MaterialConnectableAPIBehavior {
    fn can_connect_input_to_source(
        &self,
        input: &Input,
        source: &Attribute,
        reason: &mut Option<String>,
    ) -> bool {
        DefaultConnectableAPIBehavior::_can_connect_input_to_source(
            input,
            source,
            reason,
            ConnectableNodeTypes::DerivedContainerNodes,
            true, // requiresEncapsulation
        )
    }

    fn can_connect_output_to_source(
        &self,
        output: &Output,
        source: &Attribute,
        reason: &mut Option<String>,
    ) -> bool {
        DefaultConnectableAPIBehavior::_can_connect_output_to_source(
            output,
            source,
            reason,
            ConnectableNodeTypes::DerivedContainerNodes,
            true, // requiresEncapsulation
        )
    }

    fn is_container(&self) -> bool {
        true
    }

    fn requires_encapsulation(&self) -> bool {
        true
    }
}

/// NodeGraph-specific ConnectableAPIBehavior.
///
/// Per C++ nodeGraph.cpp:360-386: isContainer=true, requiresEncapsulation=true.
/// CanConnectOutputToSource delegates to _CanConnectOutputToSource (standard logic).
pub struct NodeGraphConnectableAPIBehavior;

impl ConnectableAPIBehavior for NodeGraphConnectableAPIBehavior {
    fn can_connect_input_to_source(
        &self,
        input: &Input,
        source: &Attribute,
        reason: &mut Option<String>,
    ) -> bool {
        DefaultConnectableAPIBehavior::_can_connect_input_to_source(
            input,
            source,
            reason,
            ConnectableNodeTypes::BasicNodes,
            true,
        )
    }

    fn can_connect_output_to_source(
        &self,
        output: &Output,
        source: &Attribute,
        reason: &mut Option<String>,
    ) -> bool {
        DefaultConnectableAPIBehavior::_can_connect_output_to_source(
            output,
            source,
            reason,
            ConnectableNodeTypes::BasicNodes,
            true,
        )
    }

    fn is_container(&self) -> bool {
        true
    }

    fn requires_encapsulation(&self) -> bool {
        true
    }
}

// ============================================================================
// LightFilterConnectableAPIBehavior
// ============================================================================

/// ConnectableAPI behavior for UsdLuxLightFilter.
///
/// isContainer=true, requiresEncapsulation=false, but output connections
/// are always disallowed.  Matches C++ lightFilter.cpp.
struct LightFilterConnectableAPIBehavior;

impl ConnectableAPIBehavior for LightFilterConnectableAPIBehavior {
    fn can_connect_input_to_source(
        &self,
        input: &Input,
        source: &Attribute,
        reason: &mut Option<String>,
    ) -> bool {
        // Same as LightAPI — no encapsulation
        DefaultConnectableAPIBehavior::_can_connect_input_to_source(
            input,
            source,
            reason,
            ConnectableNodeTypes::BasicNodes,
            false,
        )
    }

    fn can_connect_output_to_source(
        &self,
        _output: &Output,
        _source: &Attribute,
        _reason: &mut Option<String>,
    ) -> bool {
        // C++ lightFilter.cpp: CanConnectOutputToSource always returns false
        false
    }

    fn is_container(&self) -> bool {
        true
    }

    fn requires_encapsulation(&self) -> bool {
        false
    }
}

// ============================================================================
// Behavior Registry
// ============================================================================

/// Registry for ConnectableAPIBehavior instances.
///
/// This registry maps prim types to their behavior implementations.
struct BehaviorRegistry {
    /// Cache of behaviors by prim type name.
    behavior_cache: RwLock<HashMap<String, Arc<dyn ConnectableAPIBehavior>>>,
    /// Initialization flag.
    initialized: AtomicBool,
}

impl BehaviorRegistry {
    /// Get the singleton instance.
    fn get_instance() -> &'static BehaviorRegistry {
        static INSTANCE: std::sync::OnceLock<BehaviorRegistry> = std::sync::OnceLock::new();
        INSTANCE.get_or_init(|| {
            let registry = BehaviorRegistry {
                behavior_cache: RwLock::new(HashMap::new()),
                initialized: AtomicBool::new(true),
            };
            // Register default behaviors for known types
            registry.register_default_behaviors();
            registry
        })
    }

    /// Register default behaviors for known types.
    fn register_default_behaviors(&self) {
        // Register Shader behavior: outputs NOT connectable (C++ shader.cpp:121-131)
        self.register_behavior_for_type(
            "Shader",
            Arc::new(ShaderConnectableAPIBehavior) as Arc<dyn ConnectableAPIBehavior>,
        );

        // Register NodeGraph behavior: container=true, encapsulation=true (C++ nodeGraph.cpp:360-386)
        self.register_behavior_for_type(
            "NodeGraph",
            Arc::new(NodeGraphConnectableAPIBehavior) as Arc<dyn ConnectableAPIBehavior>,
        );

        // Register Material behavior: DerivedContainerNodes rules (C++ material.cpp:726-751)
        self.register_behavior_for_type(
            "Material",
            Arc::new(MaterialConnectableAPIBehavior) as Arc<dyn ConnectableAPIBehavior>,
        );

        // Register behavior for LightAPI: isContainer=true, requiresEncapsulation=false.
        // Per C++ lightAPI.cpp:326-360: lights are containers but do NOT enforce encapsulation,
        // allowing connections across scopes.
        self.register_behavior_for_type(
            "LightAPI",
            Arc::new(DefaultConnectableAPIBehavior::with_settings(true, false))
                as Arc<dyn ConnectableAPIBehavior>,
        );

        // Register behavior for LightFilter: isContainer=true, requiresEncapsulation=false,
        // but CanConnectOutputToSource always returns false (light filter outputs are not
        // connectable).  Matches C++ lightFilter.cpp:187-192.
        self.register_behavior_for_type(
            "LightFilter",
            Arc::new(LightFilterConnectableAPIBehavior) as Arc<dyn ConnectableAPIBehavior>,
        );

        // Concrete light types also need container=true, requiresEncapsulation=false.
        for light_type in &[
            "RectLight",
            "SphereLight",
            "DiskLight",
            "CylinderLight",
            "DistantLight",
            "DomeLight",
            "DomeLight_1",
            "GeometryLight",
            "PortalLight",
            "PluginLight",
        ] {
            self.register_behavior_for_type(
                light_type,
                Arc::new(DefaultConnectableAPIBehavior::with_settings(true, false))
                    as Arc<dyn ConnectableAPIBehavior>,
            );
        }

        // Schema-defined property names and types for light types are
        // registered in register_builtin_schemas() (usd-core), which runs
        // at Stage creation time. No need to duplicate here.
    }

    /// Register a behavior for a prim type.
    fn register_behavior_for_type(
        &self,
        type_name: &str,
        behavior: Arc<dyn ConnectableAPIBehavior>,
    ) {
        let mut cache = self.behavior_cache.write().expect("rwlock poisoned");
        cache.insert(type_name.to_string(), behavior);
    }

    /// Get behavior for a prim.
    pub fn get_behavior(&self, prim: &Prim) -> Option<Arc<dyn ConnectableAPIBehavior>> {
        if !self.initialized.load(Ordering::Acquire) {
            return None;
        }

        let type_name_token = prim.type_name();
        let type_name = type_name_token.as_str().to_string();
        let cache = self.behavior_cache.read().expect("rwlock poisoned");
        cache.get(&type_name).cloned()
    }

    /// Check if a type has a registered behavior.
    pub fn has_behavior_for_type(&self, type_name: &str) -> bool {
        if !self.initialized.load(Ordering::Acquire) {
            return false;
        }

        let cache = self.behavior_cache.read().expect("rwlock poisoned");
        cache.contains_key(type_name)
    }
}

/// Register a ConnectableAPIBehavior for a prim type.
pub fn register_connectable_api_behavior(
    type_name: &str,
    behavior: Arc<dyn ConnectableAPIBehavior>,
) {
    BehaviorRegistry::get_instance().register_behavior_for_type(type_name, behavior);
}

/// Get the behavior for a prim.
pub fn get_behavior(prim: &Prim) -> Option<Arc<dyn ConnectableAPIBehavior>> {
    BehaviorRegistry::get_instance().get_behavior(prim)
}

/// Check if a type has a registered behavior.
pub fn has_behavior_for_type(type_name: &str) -> bool {
    BehaviorRegistry::get_instance().has_behavior_for_type(type_name)
}
