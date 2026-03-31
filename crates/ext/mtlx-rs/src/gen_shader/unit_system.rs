//! UnitSystem -- unit conversion support for shader generation.
//!
//! Matches C++ MaterialXGenShader/UnitSystem.h.
//! - `UnitSystem` trait: get_name, load_library, supports_transform, create_node
//! - `DefaultUnitSystem`: default implementation using multiply nodedef + registry
//! - `UnitTransform`: source/target unit + type descriptor

use crate::core::{Document, LinearUnitConverter, UnitConverterRegistry};

use super::ShaderGraphCreateContext;
use super::shader_node::ShaderNode;
use super::type_desc::TypeDesc;

/// Unit transform: source unit, target unit, type, unit type.
/// Matches C++ UnitTransform.
#[derive(Clone, Debug)]
pub struct UnitTransform {
    pub source_unit: String,
    pub target_unit: String,
    pub type_desc: TypeDesc,
    pub unit_type: String,
}

impl UnitTransform {
    pub fn new(
        source_unit: impl Into<String>,
        target_unit: impl Into<String>,
        type_desc: TypeDesc,
        unit_type: impl Into<String>,
    ) -> Self {
        Self {
            source_unit: source_unit.into(),
            target_unit: target_unit.into(),
            type_desc,
            unit_type: unit_type.into(),
        }
    }
}

impl PartialEq for UnitTransform {
    fn eq(&self, other: &Self) -> bool {
        self.source_unit == other.source_unit
            && self.target_unit == other.target_unit
            && self.type_desc.get_name() == other.type_desc.get_name()
            && self.unit_type == other.unit_type
    }
}

/// Unit system -- transforms between unit spaces (distance, angle, etc.).
/// Matches C++ UnitSystem base class.
pub trait UnitSystem {
    /// Return the UnitSystem name.
    fn get_name(&self) -> &str;

    /// Load unit implementations from document.
    fn load_library(&mut self, _document: Document) {}

    /// Assign a UnitConverterRegistry, replacing any previous one.
    fn set_unit_converter_registry(&mut self, _registry: UnitConverterRegistry) {}

    /// Get a reference to the current UnitConverterRegistry, if set.
    fn get_unit_converter_registry(&self) -> Option<&UnitConverterRegistry> {
        None
    }

    /// Return true if this system supports the given transform.
    fn supports_transform(&self, transform: &UnitTransform) -> bool;

    /// Create unit transform node (matches C++ UnitSystem::createNode).
    /// Returns None if conversion ratio unavailable or NodeDef not found.
    fn create_node(
        &self,
        transform: &UnitTransform,
        name: &str,
        doc: &Document,
        context: &dyn ShaderGraphCreateContext,
    ) -> Option<ShaderNode>;
}

/// Default unit system. Matches C++ UnitSystem (the concrete class).
#[derive(Debug)]
pub struct DefaultUnitSystem {
    #[allow(dead_code)]
    target: String,
    document: Option<Document>,
    /// Optional registry for looking up converters by unit type.
    unit_registry: Option<UnitConverterRegistry>,
}

impl DefaultUnitSystem {
    pub const UNITSYSTEM_NAME: &'static str = "default_unit_system";

    pub fn new(target: impl Into<String>) -> Self {
        Self {
            target: target.into(),
            document: None,
            unit_registry: None,
        }
    }

    pub fn create(target: impl Into<String>) -> Box<dyn UnitSystem> {
        Box::new(Self::new(target))
    }

    /// Look up NodeDef for transform. C++ finds multiply nodedef with in1=transform.type, in2=float.
    /// Matches C++ UnitSystem::getNodeDef(const UnitTransform&).
    pub fn get_node_def(&self, transform: &UnitTransform) -> Option<crate::core::ElementPtr> {
        let doc = self.document.as_ref()?;
        let type_name = transform.type_desc.get_name();

        for child in doc.get_root().borrow().get_children() {
            let cat = child.borrow().get_category().to_string();
            if cat != crate::core::element::category::NODEDEF {
                continue;
            }
            let nd_name = child.borrow().get_name().to_string();
            if !nd_name.contains("multiply") {
                continue;
            }
            // Check inputs: must be exactly 2, with in1=type and in2=float
            let inputs = crate::core::get_active_inputs(&child);
            if inputs.len() != 2 {
                continue;
            }
            let in1_type = inputs[0]
                .borrow()
                .get_type()
                .map(|s| s.to_string())
                .unwrap_or_default();
            let in2_type = inputs[1]
                .borrow()
                .get_type()
                .map(|s| s.to_string())
                .unwrap_or_default();
            if in1_type == type_name && in2_type == "float" {
                return Some(child.clone());
            }
            // Fallback: ND_multiply_float has (float, float)
            if type_name == "float" && in1_type == "float" && in2_type == "float" {
                return Some(child.clone());
            }
        }
        None
    }

    /// Get conversion ratio via registry (preferred) or fallback to document lookup.
    fn get_conversion_ratio(
        &self,
        doc: &Document,
        unit_type: &str,
        source_unit: &str,
        target_unit: &str,
    ) -> Option<f32> {
        // Try registry first -- LinearUnitConverter has the scale table
        if let Some(registry) = &self.unit_registry {
            if let Some(linear) = registry.get_linear_converter(unit_type) {
                return linear.conversion_ratio(source_unit, target_unit);
            }
        }

        // Fallback: read scales directly from document UnitTypeDef
        let unit_type_def = doc.get_unit_type_def(unit_type)?;
        let scales = crate::core::get_unit_scales_from_typedef(&unit_type_def);
        let from_scale = scales.iter().find(|s| s.unit_name == source_unit)?.scale;
        let to_scale = scales.iter().find(|s| s.unit_name == target_unit)?.scale;
        Some(from_scale / to_scale)
    }
}

impl UnitSystem for DefaultUnitSystem {
    fn get_name(&self) -> &str {
        Self::UNITSYSTEM_NAME
    }

    fn load_library(&mut self, document: Document) {
        self.document = Some(document);
    }

    fn set_unit_converter_registry(&mut self, registry: UnitConverterRegistry) {
        self.unit_registry = Some(registry);
    }

    fn get_unit_converter_registry(&self) -> Option<&UnitConverterRegistry> {
        self.unit_registry.as_ref()
    }

    fn supports_transform(&self, transform: &UnitTransform) -> bool {
        // C++: only float and vectors
        let name = transform.type_desc.get_name();
        if name != "float" && name != "vector2" && name != "vector3" && name != "vector4" {
            return false;
        }
        self.get_node_def(transform).is_some()
    }

    fn create_node(
        &self,
        transform: &UnitTransform,
        name: &str,
        doc: &Document,
        context: &dyn ShaderGraphCreateContext,
    ) -> Option<ShaderNode> {
        let node_def = self.get_node_def(transform)?;
        let mut shader_node =
            super::shader_node_factory::create_node_from_nodedef(name, &node_def, doc, context)
                .ok()?;
        // Set in2 to conversion ratio (C++ UnitSystem::createNode ~line 193)
        let ratio = self.get_conversion_ratio(
            doc,
            &transform.unit_type,
            &transform.source_unit,
            &transform.target_unit,
        )?;
        if let Some(in2) = shader_node.inputs.get_mut("in2") {
            in2.port_mut()
                .set_value(Some(crate::core::Value::Float(ratio)), false);
        }
        Some(shader_node)
    }
}

// ---------------------------------------------------------------------------
// Helper: build a default registry from a document's UnitTypeDefs
// ---------------------------------------------------------------------------

/// Build a UnitConverterRegistry from all UnitTypeDefs in a document.
/// Creates a LinearUnitConverter for each UnitTypeDef found.
pub fn build_registry_from_document(doc: &Document) -> UnitConverterRegistry {
    let mut registry = UnitConverterRegistry::new();
    for child in doc.get_root().borrow().get_children() {
        if child.borrow().get_category() == crate::core::element::category::UNIT_TYPEDEF {
            let conv = LinearUnitConverter::create(&child);
            let name = child.borrow().get_name().to_string();
            registry.add_converter_by_name(name, Box::new(conv));
        }
    }
    registry
}
