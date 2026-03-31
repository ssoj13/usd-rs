//! ShaderMetadataRegistry — metadata for shader params (по рефу MaterialX ShaderMetadataRegistry).
//! Holds entries for UI attributes (uiname, uifolder, etc.) with optional OSL name remapping.

use std::collections::HashMap;

/// MaterialX standard UI attribute names (по рефу ValueElement).
pub mod ui_attr {
    pub const UI_NAME: &str = "uiname";
    pub const UI_FOLDER: &str = "uifolder";
    pub const UI_MIN: &str = "uimin";
    pub const UI_MAX: &str = "uimax";
    pub const UI_SOFT_MIN: &str = "uisoftmin";
    pub const UI_SOFT_MAX: &str = "uisoftmax";
    pub const UI_STEP: &str = "uistep";
    pub const UI_ADVANCED: &str = "uiadvanced";
    pub const DOC: &str = "doc";
    pub const UNIT: &str = "unit";
    pub const COLOR_SPACE: &str = "colorspace";
}

/// OSL metadata attribute names (по рефу OslShaderGenerator::registerShaderMetadata nameRemapping).
pub mod osl_attr {
    pub const LABEL: &str = "label";
    pub const PAGE: &str = "page";
    pub const MIN: &str = "min";
    pub const MAX: &str = "max";
    pub const SLIDERMIN: &str = "slidermin";
    pub const SLIDERMAX: &str = "slidermax";
    pub const SENSITIVITY: &str = "sensitivity";
    pub const HELP: &str = "help";
}

/// Metadata entry in the registry (по рефу ShaderMetadata).
#[derive(Debug, Clone)]
pub struct ShaderMetadataEntry {
    /// Export name (after remapping, e.g. "label" for OSL)
    pub name: String,
    /// Type for value formatting ("string", "float", "integer", "boolean")
    pub type_name: String,
    /// Optional default value
    pub default_value: Option<String>,
}

/// Metadata on a ShaderPort (name, value) for emit.
#[derive(Debug, Clone)]
pub struct ShaderPortMetadata {
    pub name: String,
    pub value: String,
}

/// Registry of metadata for shader parameters (по рефу ShaderMetadataRegistry).
/// find_metadata looks up by source attr name (uiname, uifolder); entry.name is export name.
#[derive(Debug, Default)]
pub struct ShaderMetadataRegistry {
    /// Source attr name → entry (entry.name may be remapped for OSL)
    entries: HashMap<String, ShaderMetadataEntry>,
}

impl ShaderMetadataRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add metadata entry (по рефу addMetadata). Lookup by name.
    pub fn add_metadata(
        &mut self,
        name: impl Into<String>,
        type_name: impl Into<String>,
        default_value: Option<impl Into<String>>,
    ) {
        let name = name.into();
        if !self.entries.contains_key(&name) {
            let key = name.clone();
            self.entries.insert(
                key,
                ShaderMetadataEntry {
                    name,
                    type_name: type_name.into(),
                    default_value: default_value.map(|v| v.into()),
                },
            );
        }
    }

    /// Find entry by source attr name (по рефу findMetadata).
    pub fn find_metadata(&self, name: &str) -> Option<&ShaderMetadataEntry> {
        self.entries.get(name)
    }

    /// Find entry mutably (for OSL remapping of entry.name).
    pub fn find_metadata_mut(&mut self, name: &str) -> Option<&mut ShaderMetadataEntry> {
        self.entries.get_mut(name)
    }

    /// Add default entries (по рефу ShaderGenerator::registerShaderMetadata DEFAULT_METADATA).
    pub fn add_default_entries(&mut self) {
        self.add_metadata(ui_attr::UI_NAME, "string", None::<&str>);
        self.add_metadata(ui_attr::UI_FOLDER, "string", None::<&str>);
        self.add_metadata(ui_attr::UI_MIN, "float", None::<&str>);
        self.add_metadata(ui_attr::UI_MAX, "float", None::<&str>);
        self.add_metadata(ui_attr::UI_SOFT_MIN, "float", None::<&str>);
        self.add_metadata(ui_attr::UI_SOFT_MAX, "float", None::<&str>);
        self.add_metadata(ui_attr::UI_STEP, "float", None::<&str>);
        self.add_metadata(ui_attr::UI_ADVANCED, "boolean", None::<&str>);
        self.add_metadata(ui_attr::DOC, "string", None::<&str>);
        self.add_metadata(ui_attr::UNIT, "string", None::<&str>);
        self.add_metadata(ui_attr::COLOR_SPACE, "string", None::<&str>);
    }

    /// Check if registry has any entries.
    pub fn has_metadata(&self) -> bool {
        !self.entries.is_empty()
    }

    /// Get the target name for a source attribute (backward compat).
    pub fn get_target_name(&self, source: &str) -> String {
        self.find_metadata(source)
            .map(|e| e.name.clone())
            .unwrap_or_else(|| source.to_string())
    }

    /// Apply OSL name remapping (по рефу OslShaderGenerator::registerShaderMetadata).
    pub fn apply_osl_remapping(&mut self) {
        let mappings: &[(&str, &str)] = &[
            (ui_attr::UI_NAME, osl_attr::LABEL),
            (ui_attr::UI_FOLDER, osl_attr::PAGE),
            (ui_attr::UI_MIN, osl_attr::MIN),
            (ui_attr::UI_MAX, osl_attr::MAX),
            (ui_attr::UI_SOFT_MIN, osl_attr::SLIDERMIN),
            (ui_attr::UI_SOFT_MAX, osl_attr::SLIDERMAX),
            (ui_attr::UI_STEP, osl_attr::SENSITIVITY),
            (ui_attr::DOC, osl_attr::HELP),
        ];
        for (src, tgt) in mappings {
            if let Some(e) = self.find_metadata_mut(src) {
                e.name = tgt.to_string();
            }
        }
    }
}
