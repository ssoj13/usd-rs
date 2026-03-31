//! SkeletonSchema - Hydra schema for skeleton data.
//!
//! Port of pxr/usdImaging/usdSkelImaging/skeletonSchema.h
//!
//! Provides data source schema for skeleton data in Hydra.

use std::sync::Arc;
use super::data_source_utils::{
    get_typed_value_from_container_vec_mat4d, get_typed_value_from_container_vec_token,
};
use usd_gf::matrix4::{Matrix4d, Matrix4f};
use usd_hd::{
    HdContainerDataSourceHandle, HdDataSourceBaseHandle, HdDataSourceLocator, cast_to_container,
};
use usd_hd::data_source::{HdTypedSampledDataSource, HdValueExtract, SampledToTypedAdapter};
use usd_tf::Token;
use usd_vt::{Array, Value};

fn value_to_f64(value: &Value) -> Option<f64> {
    value
        .get::<f64>()
        .copied()
        .or_else(|| value.get::<f32>().map(|v| *v as f64))
        .or_else(|| value.get::<i64>().map(|v| *v as f64))
        .or_else(|| value.get::<i32>().map(|v| *v as f64))
        .or_else(|| value.get::<u64>().map(|v| *v as f64))
        .or_else(|| value.get::<u32>().map(|v| *v as f64))
}

fn value_to_matrix4d(value: &Value) -> Option<Matrix4d> {
    if let Some(matrix) = value.get::<Matrix4d>() {
        return Some(*matrix);
    }
    if let Some(matrix) = value.get::<Matrix4f>() {
        return Some(Matrix4d::from(*matrix));
    }
    if let Some(values) = value.as_vec_clone::<Value>() {
        if values.len() == 16 {
            let scalars: Option<Vec<f64>> = values.iter().map(value_to_f64).collect();
            if let Some(scalars) = scalars {
                return Some(Matrix4d::new(
                    scalars[0], scalars[1], scalars[2], scalars[3], scalars[4], scalars[5],
                    scalars[6], scalars[7], scalars[8], scalars[9], scalars[10], scalars[11],
                    scalars[12], scalars[13], scalars[14], scalars[15],
                ));
            }
        }
        if values.len() == 4 {
            let mut flat = Vec::with_capacity(16);
            for row in &values {
                let row_values = row.as_vec_clone::<Value>()?;
                if row_values.len() != 4 {
                    return None;
                }
                for scalar in &row_values {
                    flat.push(value_to_f64(scalar)?);
                }
            }
            return Some(Matrix4d::new(
                flat[0], flat[1], flat[2], flat[3], flat[4], flat[5], flat[6], flat[7], flat[8],
                flat[9], flat[10], flat[11], flat[12], flat[13], flat[14], flat[15],
            ));
        }
    }
    None
}

fn extract_matrix4d_vec(value: &Value) -> Option<Vec<Matrix4d>> {
    if let Some(mats) = value.get::<Vec<Matrix4d>>() {
        return Some(mats.clone());
    }
    if let Some(mats) = value.get::<Array<Matrix4d>>() {
        return Some(mats.iter().cloned().collect());
    }
    if let Some(mats) = value.get::<Vec<Matrix4f>>() {
        return Some(mats.iter().map(|matrix| Matrix4d::from(*matrix)).collect());
    }
    if let Some(mats) = value.get::<Array<Matrix4f>>() {
        return Some(mats.iter().map(|matrix| Matrix4d::from(*matrix)).collect());
    }
    let items = value.as_vec_clone::<Value>()?;
    items.iter().map(value_to_matrix4d).collect()
}

// Token constants
mod tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    pub static SKELETON: LazyLock<Token> = LazyLock::new(|| Token::new("skeleton"));
    pub static JOINTS: LazyLock<Token> = LazyLock::new(|| Token::new("joints"));
    pub static JOINT_NAMES: LazyLock<Token> = LazyLock::new(|| Token::new("jointNames"));
    pub static BIND_TRANSFORMS: LazyLock<Token> = LazyLock::new(|| Token::new("bindTransforms"));
    pub static REST_TRANSFORMS: LazyLock<Token> = LazyLock::new(|| Token::new("restTransforms"));
}

// ============================================================================
// SkeletonSchema
// ============================================================================

/// Schema for skeleton data in Hydra.
///
/// Corresponds to UsdSkelSkeleton. Contains joint hierarchy,
/// bind transforms, and rest transforms.
#[derive(Debug, Clone)]
pub struct SkeletonSchema {
    container: Option<HdContainerDataSourceHandle>,
}

impl SkeletonSchema {
    /// Create schema from container.
    pub fn new(container: Option<HdContainerDataSourceHandle>) -> Self {
        Self { container }
    }

    /// Check if this schema is defined.
    pub fn is_defined(&self) -> bool {
        self.container.is_some()
    }

    pub fn get_container(&self) -> Option<&HdContainerDataSourceHandle> {
        self.container.as_ref()
    }

    /// Get the schema token.
    pub fn get_schema_token() -> Token {
        tokens::SKELETON.clone()
    }

    /// Get the default locator for this schema.
    pub fn get_default_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_token(tokens::SKELETON.clone())
    }

    /// Get the joints locator.
    pub fn get_joints_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_tokens_2(tokens::SKELETON.clone(), tokens::JOINTS.clone())
    }

    /// Get the joint names locator.
    pub fn get_joint_names_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_tokens_2(tokens::SKELETON.clone(), tokens::JOINT_NAMES.clone())
    }

    /// Get the bind transforms locator.
    pub fn get_bind_transforms_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_tokens_2(
            tokens::SKELETON.clone(),
            tokens::BIND_TRANSFORMS.clone(),
        )
    }

    /// Get the rest transforms locator.
    pub fn get_rest_transforms_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_tokens_2(
            tokens::SKELETON.clone(),
            tokens::REST_TRANSFORMS.clone(),
        )
    }

    /// Get schema from parent container (looks for "skeleton" child).
    pub fn get_from_parent(parent: &HdContainerDataSourceHandle) -> Option<Self> {
        let ds = parent.get(&tokens::SKELETON)?;
        let container = cast_to_container(&ds)?;
        Some(Self {
            container: Some(container),
        })
    }

    /// Get joint names (skeleton order) from schema.
    pub fn get_joints(&self) -> Vec<Token> {
        self.container
            .as_ref()
            .and_then(|c| get_typed_value_from_container_vec_token(c, &tokens::JOINTS))
            .unwrap_or_default()
    }

    /// Get bind transforms from schema (world-space bind poses, double precision).
    pub fn get_bind_transforms(&self) -> Vec<Matrix4d> {
        let diag = std::env::var_os("USD_PROFILE_SKEL_DS").is_some();
        let Some(container) = self.container.as_ref() else {
            return Vec::new();
        };
        if diag {
            eprintln!("[SkeletonSchema] get_bind_transforms container.get:start");
        }
        let Some(child) = container.get(&tokens::BIND_TRANSFORMS) else {
            return Vec::new();
        };
        if diag {
            eprintln!("[SkeletonSchema] get_bind_transforms container.get:done");
            eprintln!("[SkeletonSchema] get_bind_transforms as_sampled:start");
        }
        let Some(sampled) = child.as_sampled() else {
            return Vec::new();
        };
        if diag {
            eprintln!("[SkeletonSchema] get_bind_transforms as_sampled:done");
            eprintln!("[SkeletonSchema] get_bind_transforms get_value:start");
        }
        let value = sampled.get_value(0.0);
        if diag {
            eprintln!("[SkeletonSchema] get_bind_transforms get_value:done");
            eprintln!(
                "[SkeletonSchema] get_bind_transforms value_type={:?}",
                value.type_name()
            );
            if let Some(items) = value.as_vec_clone::<usd_vt::Value>() {
                eprintln!(
                    "[SkeletonSchema] get_bind_transforms vec_len={} first_type={:?}",
                    items.len(),
                    items.first().and_then(|item| item.type_name())
                );
            }
            eprintln!("[SkeletonSchema] get_bind_transforms extract:start");
        }
        let result = Vec::<Matrix4d>::extract(&value)
            .or_else(|| extract_matrix4d_vec(&value))
            .or_else(|| get_typed_value_from_container_vec_mat4d(container, &tokens::BIND_TRANSFORMS))
            .unwrap_or_default();
        if diag {
            eprintln!(
                "[SkeletonSchema] get_bind_transforms extract:done count={}",
                result.len()
            );
        }
        result
    }

    pub fn get_rest_transforms_data_source(
        &self,
    ) -> Option<Arc<dyn HdTypedSampledDataSource<Vec<Matrix4d>> + Send + Sync>> {
        let child = self.container.as_ref()?.get(&tokens::REST_TRANSFORMS)?;
        Some(SampledToTypedAdapter::<Vec<Matrix4d>>::new(child))
    }
}

// ============================================================================
// SkeletonSchemaBuilder
// ============================================================================

/// Builder for SkeletonSchema data sources.
#[derive(Debug, Default)]
pub struct SkeletonSchemaBuilder {
    joints: Option<HdDataSourceBaseHandle>,
    joint_names: Option<HdDataSourceBaseHandle>,
    bind_transforms: Option<HdDataSourceBaseHandle>,
    rest_transforms: Option<HdDataSourceBaseHandle>,
}

impl SkeletonSchemaBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the joints data source.
    pub fn set_joints(mut self, joints: HdDataSourceBaseHandle) -> Self {
        self.joints = Some(joints);
        self
    }

    /// Set the joint names data source.
    pub fn set_joint_names(mut self, names: HdDataSourceBaseHandle) -> Self {
        self.joint_names = Some(names);
        self
    }

    /// Set the bind transforms data source.
    pub fn set_bind_transforms(mut self, transforms: HdDataSourceBaseHandle) -> Self {
        self.bind_transforms = Some(transforms);
        self
    }

    /// Set the rest transforms data source.
    pub fn set_rest_transforms(mut self, transforms: HdDataSourceBaseHandle) -> Self {
        self.rest_transforms = Some(transforms);
        self
    }

    /// Build the container data source from set fields.
    ///
    /// Matches C++ BuildRetained: only includes non-None fields.
    pub fn build(self) -> HdContainerDataSourceHandle {
        let mut entries: Vec<(Token, HdDataSourceBaseHandle)> = Vec::with_capacity(4);
        if let Some(v) = self.joints {
            entries.push((tokens::JOINTS.clone(), v));
        }
        if let Some(v) = self.joint_names {
            entries.push((tokens::JOINT_NAMES.clone(), v));
        }
        if let Some(v) = self.bind_transforms {
            entries.push((tokens::BIND_TRANSFORMS.clone(), v));
        }
        if let Some(v) = self.rest_transforms {
            entries.push((tokens::REST_TRANSFORMS.clone(), v));
        }
        usd_hd::HdRetainedContainerDataSource::from_entries(&entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_token() {
        assert_eq!(SkeletonSchema::get_schema_token().as_str(), "skeleton");
    }

    #[test]
    fn test_joints_locator() {
        let locator = SkeletonSchema::get_joints_locator();
        assert!(locator.first_element().is_some());
    }

    #[test]
    fn test_bind_transforms_locator() {
        let locator = SkeletonSchema::get_bind_transforms_locator();
        assert!(locator.first_element().is_some());
    }

    #[test]
    fn test_builder() {
        let _schema = SkeletonSchemaBuilder::new().build();
    }
}
