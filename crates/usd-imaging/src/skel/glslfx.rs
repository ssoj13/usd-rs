//! Minimal GLSLFX parser for skinning compute kernels.
//!
//! Port of pxr/imaging/hio/glslfx - simplified for UsdSkelImaging skinning.glslfx.
//! Parses glslfx format: sections (-- glslfx, -- configuration, -- glsl SectionId)
//! and returns concatenated GLSL source for a given kernel key.

use std::collections::HashMap;

const SKINNING_GLSLFX: &str = include_str!("shaders/skinning.glslfx");

/// Parsed glslfx document - source map and config for technique lookup.
pub struct Glslfx {
    source_map: HashMap<String, String>,
    source_key_map: HashMap<String, Vec<String>>,
}

impl Glslfx {
    /// Parse glslfx content. Returns None if parsing fails.
    pub fn parse(source: &str) -> Option<Self> {
        let mut source_map: HashMap<String, String> = HashMap::new();
        let mut config_json = String::new();
        let mut current_section_type: Option<&str> = None;
        let mut current_section_id = String::new();

        for line in source.lines() {
            let line = line.trim_end();
            if line.is_empty() {
                if let (Some("glsl"), ref id) =
                    (current_section_type.as_deref(), &current_section_id)
                {
                    if !id.is_empty() {
                        source_map.get_mut(id.as_str()).map(|s| s.push('\n'));
                    }
                }
                continue;
            }
            if line.starts_with("---") {
                continue;
            }
            if line.starts_with("-- ") {
                let rest = line[3..].trim_start();
                let mut tokens = rest.split_whitespace();
                let section_type = tokens.next()?;
                let section_id = tokens.next().unwrap_or("").to_string();

                current_section_type = Some(section_type);
                current_section_id = section_id.clone();

                if section_type == "configuration" {
                    config_json.clear();
                    continue;
                }
                if section_type == "glsl" && !section_id.is_empty() {
                    source_map.insert(section_id.clone(), String::new());
                    continue;
                }
                continue;
            }

            if let Some(st) = current_section_type {
                if st == "configuration" {
                    config_json.push_str(line);
                    config_json.push('\n');
                } else if st == "glsl" && !current_section_id.is_empty() {
                    if let Some(content) = source_map.get_mut(&current_section_id) {
                        if !content.is_empty() {
                            content.push('\n');
                        }
                        content.push_str(line);
                    }
                }
            }
        }

        let source_key_map = parse_source_key_map(&config_json)?;

        Some(Self {
            source_map,
            source_key_map,
        })
    }

    /// Get concatenated GLSL source for the given kernel key (e.g. skinPointsLBSKernel).
    pub fn get_source(&self, kernel_key: &str) -> Option<String> {
        let keys = self.source_key_map.get(kernel_key)?;
        let mut result = String::new();
        for key in keys {
            let section = self.source_map.get(key)?;
            if !result.is_empty() {
                result.push('\n');
            }
            result.push_str(section);
        }
        if result.is_empty() {
            None
        } else {
            Some(result)
        }
    }
}

fn parse_source_key_map(config_json: &str) -> Option<HashMap<String, Vec<String>>> {
    let config: serde_json::Value = serde_json::from_str(config_json).ok()?;
    let techniques = config.get("techniques")?.as_object()?;
    let default_technique = techniques.get("default")?.as_object()?;

    let mut source_key_map = HashMap::new();
    for (kernel_key, kernel_config) in default_technique {
        let source_array = kernel_config.get("source")?.as_array()?;
        let keys: Vec<String> = source_array
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect();
        if !keys.is_empty() {
            source_key_map.insert(kernel_key.clone(), keys);
        }
    }
    Some(source_key_map)
}

/// Load the skinning glslfx and return GLSL source for the given kernel key.
pub fn load_skinning_kernel(kernel_key: &str) -> Option<String> {
    let gfx = Glslfx::parse(SKINNING_GLSLFX)?;
    gfx.get_source(kernel_key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_skinning_glslfx() {
        let gfx = Glslfx::parse(SKINNING_GLSLFX).expect("parse skinning.glslfx");
        assert!(gfx.get_source("skinPointsLBSKernel").is_some());
        assert!(gfx.get_source("skinPointsDQSKernel").is_some());
        assert!(gfx.get_source("skinNormalsLBSKernel").is_some());
        assert!(gfx.get_source("skinNormalsDQSKernel").is_some());
    }
}
