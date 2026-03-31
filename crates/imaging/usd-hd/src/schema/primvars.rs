//! Primvars (primitive variables) schema for Hydra.

use super::HdSchema;
use crate::data_source::HdDataSourceLocator;
use crate::data_source::{
    HdContainerDataSourceHandle, HdDataSourceBaseHandle, HdRetainedContainerDataSource,
    HdRetainedSampledDataSource, HdSampledDataSourceHandle, cast_to_container,
};
use once_cell::sync::Lazy;
use std::sync::Arc;
use usd_tf::Token;

/// Schema token: "primvars".
pub static PRIMVARS: Lazy<Token> = Lazy::new(|| Token::new("primvars"));
/// Member token: "primvarValue".
pub static PRIMVAR_VALUE: Lazy<Token> = Lazy::new(|| Token::new("primvarValue"));
/// Member token: "indexedPrimvarValue".
pub static INDEXED_PRIMVAR_VALUE: Lazy<Token> = Lazy::new(|| Token::new("indexedPrimvarValue"));
/// Member token: "indices".
pub static INDICES: Lazy<Token> = Lazy::new(|| Token::new("indices"));
/// Interpolation token: "interpolation".
pub static INTERPOLATION: Lazy<Token> = Lazy::new(|| Token::new("interpolation"));
/// Member token: "role".
pub static ROLE: Lazy<Token> = Lazy::new(|| Token::new("role"));
/// Member token: "colorSpace".
pub static COLOR_SPACE: Lazy<Token> = Lazy::new(|| Token::new("colorSpace"));
/// Member token: "elementSize".
pub static ELEMENT_SIZE: Lazy<Token> = Lazy::new(|| Token::new("elementSize"));
/// Interpolation value: per-vertex.
pub static VERTEX: Lazy<Token> = Lazy::new(|| Token::new("vertex"));
/// Interpolation value: varying (linearly interpolated).
pub static VARYING: Lazy<Token> = Lazy::new(|| Token::new("varying"));
/// Interpolation value: constant (one value per prim).
pub static CONSTANT: Lazy<Token> = Lazy::new(|| Token::new("constant"));
/// Interpolation value: uniform (one value per face).
pub static UNIFORM: Lazy<Token> = Lazy::new(|| Token::new("uniform"));
/// Interpolation value: face-varying (per face-vertex).
pub static FACE_VARYING: Lazy<Token> = Lazy::new(|| Token::new("faceVarying"));
/// Interpolation value: per-instance.
pub static INSTANCE: Lazy<Token> = Lazy::new(|| Token::new("instance"));
/// Well-known primvar: "points".
pub static POINTS: Lazy<Token> = Lazy::new(|| Token::new("points"));
/// Well-known primvar: "normals".
pub static NORMALS: Lazy<Token> = Lazy::new(|| Token::new("normals"));
/// Well-known primvar: "widths".
pub static WIDTHS: Lazy<Token> = Lazy::new(|| Token::new("widths"));
/// Member token: "transform".
pub static TRANSFORM: Lazy<Token> = Lazy::new(|| Token::new("transform"));
/// Role value: point.
pub static ROLE_POINT: Lazy<Token> = Lazy::new(|| Token::new("point"));
/// Role value: normal.
pub static ROLE_NORMAL: Lazy<Token> = Lazy::new(|| Token::new("normal"));
/// Role value: vector.
pub static ROLE_VECTOR: Lazy<Token> = Lazy::new(|| Token::new("vector"));
/// Role value: color.
pub static ROLE_COLOR: Lazy<Token> = Lazy::new(|| Token::new("color"));
/// Role value: pointIndex.
pub static ROLE_POINT_INDEX: Lazy<Token> = Lazy::new(|| Token::new("pointIndex"));
/// Role value: edgeIndex.
pub static ROLE_EDGE_INDEX: Lazy<Token> = Lazy::new(|| Token::new("edgeIndex"));
/// Role value: faceIndex.
pub static ROLE_FACE_INDEX: Lazy<Token> = Lazy::new(|| Token::new("faceIndex"));
/// Role value: textureCoordinate.
pub static ROLE_TEXTURE_COORDINATE: Lazy<Token> = Lazy::new(|| Token::new("textureCoordinate"));

/// Int data source handle alias.
pub type HdIntDataSourceHandle =
    Arc<dyn crate::data_source::HdTypedSampledDataSource<i32> + Send + Sync>;
/// Int array data source handle alias.
pub type HdIntArrayDataSourceHandle =
    Arc<dyn crate::data_source::HdTypedSampledDataSource<usd_vt::Array<i32>> + Send + Sync>;
/// Token data source handle alias.
pub type HdTokenDataSourceHandle =
    Arc<dyn crate::data_source::HdTypedSampledDataSource<Token> + Send + Sync>;

/// Schema for a single primvar (value, indexed value, indices, interpolation, role, etc).
///
/// Corresponds to pxr/imaging/hd/primvarSchema.h
#[derive(Debug, Clone)]
pub struct HdPrimvarSchema {
    schema: HdSchema,
}

impl HdPrimvarSchema {
    /// Create from container data source.
    pub fn new(container: HdContainerDataSourceHandle) -> Self {
        Self {
            schema: HdSchema::new(container),
        }
    }

    /// Get primvar value data source. Use as_sampled() on result for get_value(t).
    pub fn get_primvar_value(&self) -> Option<HdDataSourceBaseHandle> {
        self.schema.get_container()?.get(&PRIMVAR_VALUE)
    }

    /// Get indexed primvar value (unflattened) when primvar is indexed.
    pub fn get_indexed_primvar_value(&self) -> Option<HdDataSourceBaseHandle> {
        self.schema.get_container()?.get(&INDEXED_PRIMVAR_VALUE)
    }

    /// Get interpolatation (token).
    pub fn get_interpolation(
        &self,
    ) -> Option<std::sync::Arc<dyn crate::data_source::HdTypedSampledDataSource<Token> + Send + Sync>>
    {
        self.schema.get_typed_retained::<Token>(&INTERPOLATION)
    }

    /// Get role (token).
    pub fn get_role(
        &self,
    ) -> Option<std::sync::Arc<dyn crate::data_source::HdTypedSampledDataSource<Token> + Send + Sync>>
    {
        self.schema.get_typed_retained::<Token>(&ROLE)
    }

    /// Get color space (token).
    pub fn get_color_space(
        &self,
    ) -> Option<std::sync::Arc<dyn crate::data_source::HdTypedSampledDataSource<Token> + Send + Sync>>
    {
        self.schema.get_typed_retained::<Token>(&COLOR_SPACE)
    }

    /// Get element size (int) - number of values per element.
    pub fn get_element_size(
        &self,
    ) -> Option<std::sync::Arc<dyn crate::data_source::HdTypedSampledDataSource<i32> + Send + Sync>>
    {
        self.schema.get_typed_retained::<i32>(&ELEMENT_SIZE)
    }

    /// Get indices (int array) for indexed primvars.
    pub fn get_indices(
        &self,
    ) -> Option<
        std::sync::Arc<
            dyn crate::data_source::HdTypedSampledDataSource<usd_vt::Array<i32>> + Send + Sync,
        >,
    > {
        self.schema
            .get_typed_retained::<usd_vt::Array<i32>>(&INDICES)
    }

    /// Returns true if this primvar has indexed value and indices.
    pub fn is_indexed(&self) -> bool {
        if let Some(container) = self.schema.get_container() {
            container.get(&INDEXED_PRIMVAR_VALUE).is_some() && container.get(&INDICES).is_some()
        } else {
            false
        }
    }

    /// Get flattened primvar value (expands indexed primvars).
    ///
    /// If the primvar is not indexed, returns the primvarValue data source.
    /// If indexed, returns the flattened (expanded) value.
    pub fn get_flattened_primvar_value(&self) -> Option<HdDataSourceBaseHandle> {
        if self.is_indexed() {
            // For indexed primvars, return primvarValue which is the
            // flattened version (same behavior as C++ GetPrimvarValue)
            self.get_primvar_value()
        } else {
            self.schema.get_container()?.get(&PRIMVAR_VALUE)
        }
    }

    /// Build a retained container data source with all primvar fields.
    ///
    /// Parameters with None values are excluded.
    pub fn build_retained(
        primvar_value: Option<HdSampledDataSourceHandle>,
        indexed_primvar_value: Option<HdSampledDataSourceHandle>,
        indices: Option<HdIntArrayDataSourceHandle>,
        interpolation: Option<HdTokenDataSourceHandle>,
        role: Option<HdTokenDataSourceHandle>,
        color_space: Option<HdTokenDataSourceHandle>,
        element_size: Option<HdIntDataSourceHandle>,
    ) -> HdContainerDataSourceHandle {
        let mut entries: Vec<(Token, HdDataSourceBaseHandle)> = Vec::new();
        if let Some(v) = primvar_value {
            entries.push((PRIMVAR_VALUE.clone(), v));
        }
        if let Some(v) = indexed_primvar_value {
            entries.push((INDEXED_PRIMVAR_VALUE.clone(), v));
        }
        if let Some(v) = indices {
            entries.push((INDICES.clone(), v));
        }
        if let Some(v) = interpolation {
            entries.push((INTERPOLATION.clone(), v));
        }
        if let Some(v) = role {
            entries.push((ROLE.clone(), v));
        }
        if let Some(v) = color_space {
            entries.push((COLOR_SPACE.clone(), v));
        }
        if let Some(v) = element_size {
            entries.push((ELEMENT_SIZE.clone(), v));
        }
        HdRetainedContainerDataSource::from_entries(&entries)
    }

    /// Return a sampled data source for an interpolation value.
    ///
    /// Uses HdRetainedSampledDataSource for easy construction.
    pub fn build_interpolation_data_source(interpolation: &Token) -> HdSampledDataSourceHandle {
        HdRetainedSampledDataSource::new(usd_vt::Value::new(interpolation.clone()))
    }

    /// Return a sampled data source for a role value.
    ///
    /// Uses HdRetainedSampledDataSource for easy construction.
    pub fn build_role_data_source(role: &Token) -> HdSampledDataSourceHandle {
        HdRetainedSampledDataSource::new(usd_vt::Value::new(role.clone()))
    }
}

/// Builder for constructing HdPrimvarSchema containers.
///
/// Fluent API matching C++ `HdPrimvarSchema::Builder`.
#[derive(Default)]
pub struct HdPrimvarSchemaBuilder {
    primvar_value: Option<HdSampledDataSourceHandle>,
    indexed_primvar_value: Option<HdSampledDataSourceHandle>,
    indices: Option<HdIntArrayDataSourceHandle>,
    interpolation: Option<HdTokenDataSourceHandle>,
    role: Option<HdTokenDataSourceHandle>,
    color_space: Option<HdTokenDataSourceHandle>,
    element_size: Option<HdIntDataSourceHandle>,
}

impl HdPrimvarSchemaBuilder {
    /// Create new builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set primvar value.
    pub fn set_primvar_value(mut self, v: HdSampledDataSourceHandle) -> Self {
        self.primvar_value = Some(v);
        self
    }

    /// Set indexed primvar value.
    pub fn set_indexed_primvar_value(mut self, v: HdSampledDataSourceHandle) -> Self {
        self.indexed_primvar_value = Some(v);
        self
    }

    /// Set indices.
    pub fn set_indices(mut self, v: HdIntArrayDataSourceHandle) -> Self {
        self.indices = Some(v);
        self
    }

    /// Set interpolation.
    pub fn set_interpolation(mut self, v: HdTokenDataSourceHandle) -> Self {
        self.interpolation = Some(v);
        self
    }

    /// Set role.
    pub fn set_role(mut self, v: HdTokenDataSourceHandle) -> Self {
        self.role = Some(v);
        self
    }

    /// Set color space.
    pub fn set_color_space(mut self, v: HdTokenDataSourceHandle) -> Self {
        self.color_space = Some(v);
        self
    }

    /// Set element size.
    pub fn set_element_size(mut self, v: HdIntDataSourceHandle) -> Self {
        self.element_size = Some(v);
        self
    }

    /// Build the container data source.
    pub fn build(self) -> HdContainerDataSourceHandle {
        HdPrimvarSchema::build_retained(
            self.primvar_value,
            self.indexed_primvar_value,
            self.indices,
            self.interpolation,
            self.role,
            self.color_space,
            self.element_size,
        )
    }
}

/// Schema for primitive variables (primvars).
///
/// Provides access to per-primitive data attributes like points, normals,
/// colors, UVs, and custom attributes. Primvars can have different
/// interpolation modes (constant, uniform, vertex, varying, faceVarying).
#[derive(Debug, Clone)]
pub struct HdPrimvarsSchema {
    schema: HdSchema,
}

impl HdPrimvarsSchema {
    /// Creates a new primvars schema from a container data source.
    pub fn new(container: HdContainerDataSourceHandle) -> Self {
        Self {
            schema: HdSchema::new(container),
        }
    }

    /// Retrieves the schema from a parent container data source.
    ///
    /// Looks for a child container under the `primvars` token.
    pub fn get_from_parent(parent: &HdContainerDataSourceHandle) -> Self {
        if let Some(child) = parent.get(&PRIMVARS) {
            if let Some(container) = cast_to_container(&child) {
                return Self::new(container);
            }
        }
        Self {
            schema: HdSchema::empty(),
        }
    }

    /// Checks if the schema is defined (has a valid container).
    pub fn is_defined(&self) -> bool {
        self.schema.is_defined()
    }

    /// Returns the underlying container data source.
    pub fn get_container(&self) -> Option<&HdContainerDataSourceHandle> {
        self.schema.get_container()
    }

    /// Returns all primvar names available in this schema.
    pub fn get_primvar_names(&self) -> Vec<Token> {
        if let Some(container) = self.schema.get_container() {
            container.get_names()
        } else {
            Vec::new()
        }
    }

    /// Returns a specific primvar by name as container handle.
    pub fn get_primvar(&self, name: &Token) -> Option<HdContainerDataSourceHandle> {
        if let Some(container) = self.schema.get_container() {
            if let Some(child) = container.get(name) {
                return cast_to_container(&child);
            }
        }
        None
    }

    /// Returns HdPrimvarSchema for a specific primvar by name.
    pub fn get_primvar_schema(&self, name: &Token) -> HdPrimvarSchema {
        if let Some(container) = self.get_primvar(name) {
            return HdPrimvarSchema::new(container);
        }
        HdPrimvarSchema::new(crate::data_source::HdRetainedContainerDataSource::new_empty())
    }

    /// Returns the schema's identifying token.
    pub fn get_schema_token() -> &'static Lazy<Token> {
        &PRIMVARS
    }

    /// Returns the default data source locator for this schema.
    pub fn get_default_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[PRIMVARS.clone()])
    }

    /// Returns the data source locator for the points primvar.
    pub fn get_points_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[PRIMVARS.clone(), POINTS.clone()])
    }

    /// Returns the data source locator for the normals primvar.
    pub fn get_normals_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[PRIMVARS.clone(), NORMALS.clone()])
    }

    /// Returns the data source locator for the widths primvar.
    pub fn get_widths_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[PRIMVARS.clone(), WIDTHS.clone()])
    }

    /// Builds a retained container data source with the specified primvars.
    ///
    /// The `names` and `values` arrays must have the same length.
    /// Each value should be a container with primvar-specific data.
    pub fn build_retained(
        names: &[Token],
        values: &[HdDataSourceBaseHandle],
    ) -> HdContainerDataSourceHandle {
        use crate::data_source::HdRetainedContainerDataSource;

        assert_eq!(names.len(), values.len());

        let entries: Vec<(Token, HdDataSourceBaseHandle)> = names
            .iter()
            .zip(values.iter())
            .map(|(n, v)| (n.clone(), v.clone()))
            .collect();

        HdRetainedContainerDataSource::from_entries(&entries)
    }
}
