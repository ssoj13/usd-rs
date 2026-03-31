//! Unit, UnitDef, UnitTypeDef -- unit system.
//!
//! Provides UnitConverter trait, LinearUnitConverter (scale-based),
//! and UnitConverterRegistry for managing converters per unit type.
//! Matches C++ MaterialXCore/Unit.h.

use std::any::Any;
use std::collections::HashMap;

use crate::core::element::{ElementPtr, category};

/// Unit scale entry parsed from a UnitDef element.
#[derive(Clone, Debug)]
pub struct UnitScale {
    pub unit_name: String,
    pub scale: f32,
}

/// Get unit scales from a UnitDef element (child "unit" elements).
pub fn get_unit_scales(unit_def: &ElementPtr) -> Vec<UnitScale> {
    let mut result = vec![];
    for child in unit_def.borrow().get_children() {
        if child.borrow().get_category() == category::UNIT {
            let name = child.borrow().get_name().to_string();
            let scale: f32 = child
                .borrow()
                .get_attribute("scale")
                .and_then(|s| s.parse().ok())
                .unwrap_or(1.0);
            result.push(UnitScale {
                unit_name: name,
                scale,
            });
        }
    }
    result
}

/// Get unit scales from UnitTypeDef (all UnitDefs with matching unittype).
/// UnitTypeDef and UnitDefs are siblings under document root.
pub fn get_unit_scales_from_typedef(unit_type_def: &ElementPtr) -> Vec<UnitScale> {
    let type_name = unit_type_def.borrow().get_name().to_string();
    let mut result = vec![];
    let doc_root = unit_type_def
        .borrow()
        .get_parent()
        .expect("UnitTypeDef has document parent");
    for child in doc_root.borrow().get_children() {
        if child.borrow().get_category() == category::UNIT_DEF {
            if child
                .borrow()
                .get_attribute("unittype")
                .map(|s| s == type_name)
                .unwrap_or(false)
            {
                result.extend(get_unit_scales(&child));
            }
        }
    }
    result
}

// ---------------------------------------------------------------------------
// UnitConverter trait
// ---------------------------------------------------------------------------

/// Abstract unit converter. Each instance handles one unit type (e.g. "distance").
/// Matches C++ UnitConverter.
pub trait UnitConverter: Any {
    /// Convert scalar value from `input_unit` to `output_unit`.
    fn convert_float(&self, input: f32, input_unit: &str, output_unit: &str) -> f32;

    /// Convert vec2 value.
    fn convert_vec2(&self, input: [f32; 2], input_unit: &str, output_unit: &str) -> [f32; 2];

    /// Convert vec3 value.
    fn convert_vec3(&self, input: [f32; 3], input_unit: &str, output_unit: &str) -> [f32; 3];

    /// Convert vec4 value.
    fn convert_vec4(&self, input: [f32; 4], input_unit: &str, output_unit: &str) -> [f32; 4];

    /// Map unit name to integer index. Returns None if unknown.
    fn get_unit_as_integer(&self, _unit_name: &str) -> Option<u32> {
        None
    }

    /// Map integer index back to unit name. Returns None if unknown.
    fn get_unit_from_integer(&self, _index: u32) -> Option<&str> {
        None
    }

    /// Return the unit type string (e.g. "distance", "angle").
    fn get_unit_type(&self) -> &str;

    /// Upcast to Any for downcasting to concrete type.
    fn as_any(&self) -> &dyn Any;
}

// ---------------------------------------------------------------------------
// LinearUnitConverter
// ---------------------------------------------------------------------------

/// Linear unit converter: conversion = input * (from_scale / to_scale).
/// Matches C++ LinearUnitConverter. Handles distance, angle, etc.
#[derive(Clone, Debug)]
pub struct LinearUnitConverter {
    /// Map from unit name to its scale factor.
    unit_scale: HashMap<String, f32>,
    /// Map from unit name to integer enumeration index.
    unit_enumeration: HashMap<String, u32>,
    /// The unit type name (e.g. "distance").
    unit_type: String,
}

impl LinearUnitConverter {
    /// Create from a UnitTypeDef element (reads all UnitDefs + their unit children).
    /// Matches C++ LinearUnitConverter::create(UnitTypeDefPtr).
    pub fn create(unit_type_def: &ElementPtr) -> Self {
        let scales = get_unit_scales_from_typedef(unit_type_def);
        let unit_type = unit_type_def.borrow().get_name().to_string();

        let mut unit_scale = HashMap::new();
        let mut unit_enumeration = HashMap::new();
        for (i, us) in scales.iter().enumerate() {
            unit_scale.insert(us.unit_name.clone(), us.scale);
            unit_enumeration.insert(us.unit_name.clone(), i as u32);
        }

        Self {
            unit_scale,
            unit_enumeration,
            unit_type,
        }
    }

    /// Create from explicit scale map (for testing / manual setup).
    pub fn from_scales(unit_type: impl Into<String>, scales: &[(&str, f32)]) -> Self {
        let mut unit_scale = HashMap::new();
        let mut unit_enumeration = HashMap::new();
        for (i, &(name, scale)) in scales.iter().enumerate() {
            unit_scale.insert(name.to_string(), scale);
            unit_enumeration.insert(name.to_string(), i as u32);
        }
        Self {
            unit_scale,
            unit_enumeration,
            unit_type: unit_type.into(),
        }
    }

    /// Get the scale map (unit name -> scale factor).
    pub fn get_unit_scale(&self) -> &HashMap<String, f32> {
        &self.unit_scale
    }

    /// Compute ratio: from_scale / to_scale. Returns None if either unit unknown.
    pub fn conversion_ratio(&self, input_unit: &str, output_unit: &str) -> Option<f32> {
        let from = self.unit_scale.get(input_unit)?;
        let to = self.unit_scale.get(output_unit)?;
        Some(from / to)
    }
}

impl UnitConverter for LinearUnitConverter {
    fn convert_float(&self, input: f32, input_unit: &str, output_unit: &str) -> f32 {
        if input_unit == output_unit {
            return input;
        }
        match self.conversion_ratio(input_unit, output_unit) {
            Some(ratio) => input * ratio,
            None => input,
        }
    }

    fn convert_vec2(&self, input: [f32; 2], input_unit: &str, output_unit: &str) -> [f32; 2] {
        if input_unit == output_unit {
            return input;
        }
        match self.conversion_ratio(input_unit, output_unit) {
            Some(ratio) => [input[0] * ratio, input[1] * ratio],
            None => input,
        }
    }

    fn convert_vec3(&self, input: [f32; 3], input_unit: &str, output_unit: &str) -> [f32; 3] {
        if input_unit == output_unit {
            return input;
        }
        match self.conversion_ratio(input_unit, output_unit) {
            Some(ratio) => [input[0] * ratio, input[1] * ratio, input[2] * ratio],
            None => input,
        }
    }

    fn convert_vec4(&self, input: [f32; 4], input_unit: &str, output_unit: &str) -> [f32; 4] {
        if input_unit == output_unit {
            return input;
        }
        match self.conversion_ratio(input_unit, output_unit) {
            Some(ratio) => [
                input[0] * ratio,
                input[1] * ratio,
                input[2] * ratio,
                input[3] * ratio,
            ],
            None => input,
        }
    }

    fn get_unit_as_integer(&self, unit_name: &str) -> Option<u32> {
        self.unit_enumeration.get(unit_name).copied()
    }

    fn get_unit_from_integer(&self, index: u32) -> Option<&str> {
        self.unit_enumeration
            .iter()
            .find(|(_, v)| **v == index)
            .map(|(k, _)| k.as_str())
    }

    fn get_unit_type(&self) -> &str {
        &self.unit_type
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

// ---------------------------------------------------------------------------
// UnitConverterRegistry
// ---------------------------------------------------------------------------

/// Registry mapping unit type names to their converters.
/// Matches C++ UnitConverterRegistry.
#[derive(Default)]
pub struct UnitConverterRegistry {
    /// Map from unit type name (e.g. "distance") to converter.
    converters: HashMap<String, Box<dyn UnitConverter>>,
}

impl UnitConverterRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add converter for a unit type def. Returns false if already registered.
    pub fn add_converter(
        &mut self,
        unit_type_def: &ElementPtr,
        converter: Box<dyn UnitConverter>,
    ) -> bool {
        let name = unit_type_def.borrow().get_name().to_string();
        self.add_converter_by_name(name, converter)
    }

    /// Add converter by unit type name directly. Returns false if already registered.
    pub fn add_converter_by_name(
        &mut self,
        name: impl Into<String>,
        converter: Box<dyn UnitConverter>,
    ) -> bool {
        let name = name.into();
        if self.converters.contains_key(&name) {
            return false;
        }
        self.converters.insert(name, converter);
        true
    }

    /// Remove converter by unit type name. Returns false if not found.
    pub fn remove_converter_by_name(&mut self, name: &str) -> bool {
        self.converters.remove(name).is_some()
    }

    /// Remove converter for a unit type def. Returns false if not found.
    pub fn remove_converter(&mut self, unit_type_def: &ElementPtr) -> bool {
        let name = unit_type_def.borrow().get_name().to_string();
        self.remove_converter_by_name(&name)
    }

    /// Get converter for a unit type def. Returns None if not found.
    pub fn get_converter(&self, unit_type_def: &ElementPtr) -> Option<&dyn UnitConverter> {
        let name = unit_type_def.borrow().get_name().to_string();
        self.get_converter_by_name(&name)
    }

    /// Get converter by unit type name directly.
    pub fn get_converter_by_name(&self, name: &str) -> Option<&dyn UnitConverter> {
        self.converters.get(name).map(|b| b.as_ref())
    }

    /// Clear all converters.
    pub fn clear(&mut self) {
        self.converters.clear();
    }

    /// Look up unit name across all converters. Returns the integer mapping or None.
    pub fn get_unit_as_integer(&self, unit_name: &str) -> Option<u32> {
        for conv in self.converters.values() {
            if let Some(v) = conv.get_unit_as_integer(unit_name) {
                return Some(v);
            }
        }
        None
    }

    /// Downcast a converter to LinearUnitConverter if applicable.
    /// Useful for emitting shader code that needs the scale lookup table.
    pub fn get_linear_converter(&self, unit_type: &str) -> Option<&LinearUnitConverter> {
        let conv = self.converters.get(unit_type)?;
        conv.as_any().downcast_ref::<LinearUnitConverter>()
    }
}

impl std::fmt::Debug for UnitConverterRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UnitConverterRegistry")
            .field("unit_types", &self.converters.keys().collect::<Vec<_>>())
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linear_converter_from_scales() {
        let conv = LinearUnitConverter::from_scales(
            "distance",
            &[
                ("meter", 1.0),
                ("centimeter", 0.01),
                ("kilometer", 1000.0),
                ("inch", 0.0254),
                ("foot", 0.3048),
            ],
        );

        assert_eq!(conv.get_unit_type(), "distance");

        // meter -> centimeter: 1.0 / 0.01 = 100.0
        let ratio = conv.conversion_ratio("meter", "centimeter").unwrap();
        assert!((ratio - 100.0).abs() < 1e-6);

        // Convert 2.5 meters to centimeters
        let result = conv.convert_float(2.5, "meter", "centimeter");
        assert!((result - 250.0).abs() < 1e-3);

        // Same unit -> no conversion
        assert_eq!(conv.convert_float(5.0, "meter", "meter"), 5.0);

        // Unknown unit -> passthrough
        assert_eq!(conv.convert_float(5.0, "meter", "parsec"), 5.0);

        // Vec3 conversion
        let v = conv.convert_vec3([1.0, 2.0, 3.0], "meter", "centimeter");
        assert!((v[0] - 100.0).abs() < 1e-3);
        assert!((v[1] - 200.0).abs() < 1e-3);
        assert!((v[2] - 300.0).abs() < 1e-3);
    }

    #[test]
    fn test_unit_enumeration() {
        let conv = LinearUnitConverter::from_scales(
            "distance",
            &[("meter", 1.0), ("centimeter", 0.01), ("foot", 0.3048)],
        );

        assert_eq!(conv.get_unit_as_integer("meter"), Some(0));
        assert_eq!(conv.get_unit_as_integer("centimeter"), Some(1));
        assert_eq!(conv.get_unit_as_integer("foot"), Some(2));
        assert_eq!(conv.get_unit_as_integer("parsec"), None);

        assert_eq!(conv.get_unit_from_integer(0), Some("meter"));
        assert_eq!(conv.get_unit_from_integer(1), Some("centimeter"));
        assert_eq!(conv.get_unit_from_integer(99), None);
    }

    #[test]
    fn test_registry() {
        let mut reg = UnitConverterRegistry::new();

        let dist =
            LinearUnitConverter::from_scales("distance", &[("meter", 1.0), ("centimeter", 0.01)]);
        let angle =
            LinearUnitConverter::from_scales("angle", &[("degree", 1.0), ("radian", 57.2957795)]);

        assert!(reg.add_converter_by_name("distance", Box::new(dist)));
        assert!(reg.add_converter_by_name("angle", Box::new(angle)));

        // Duplicate should fail
        let dup = LinearUnitConverter::from_scales("distance", &[("meter", 1.0)]);
        assert!(!reg.add_converter_by_name("distance", Box::new(dup)));

        // Lookup
        assert!(reg.get_converter_by_name("distance").is_some());
        assert!(reg.get_converter_by_name("angle").is_some());
        assert!(reg.get_converter_by_name("mass").is_none());

        // get_unit_as_integer across all converters
        assert_eq!(reg.get_unit_as_integer("meter"), Some(0));
        assert_eq!(reg.get_unit_as_integer("degree"), Some(0));
        assert_eq!(reg.get_unit_as_integer("parsec"), None);

        // Downcast to LinearUnitConverter
        let linear = reg.get_linear_converter("distance").unwrap();
        let ratio = linear.conversion_ratio("meter", "centimeter").unwrap();
        assert!((ratio - 100.0).abs() < 1e-6);

        // Remove
        assert!(reg.remove_converter_by_name("distance"));
        assert!(reg.get_converter_by_name("distance").is_none());

        // Clear
        reg.clear();
        assert!(reg.get_converter_by_name("angle").is_none());
    }
}
