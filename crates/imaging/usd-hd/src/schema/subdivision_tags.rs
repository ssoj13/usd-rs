#![allow(dead_code)]
//! Subdivision tags schema for Hydra.
//!
//! Defines subdivision tags including creases, corners, and interpolation rules.

use super::HdSchema;
use crate::data_source::{
    HdContainerDataSourceHandle, HdDataSourceBaseHandle, HdDataSourceLocator,
    HdRetainedContainerDataSource, HdTypedSampledDataSource, cast_to_container,
};
use std::sync::Arc;
use std::sync::LazyLock;
use usd_tf::Token;

/// Subdivision tags schema token
pub static SUBDIVISION_TAGS: LazyLock<Token> = LazyLock::new(|| Token::new("subdivisionTags"));
/// Face varying linear interpolation token
pub static FACE_VARYING_LINEAR_INTERPOLATION: LazyLock<Token> =
    LazyLock::new(|| Token::new("faceVaryingLinearInterpolation"));
/// Interpolate boundary token
pub static INTERPOLATE_BOUNDARY: LazyLock<Token> =
    LazyLock::new(|| Token::new("interpolateBoundary"));
/// Triangle subdivision rule token
pub static TRIANGLE_SUBDIVISION_RULE: LazyLock<Token> =
    LazyLock::new(|| Token::new("triangleSubdivisionRule"));
/// Corner indices token
pub static CORNER_INDICES: LazyLock<Token> = LazyLock::new(|| Token::new("cornerIndices"));
/// Corner sharpnesses token
pub static CORNER_SHARPNESSES: LazyLock<Token> = LazyLock::new(|| Token::new("cornerSharpnesses"));
/// Crease indices token
pub static CREASE_INDICES: LazyLock<Token> = LazyLock::new(|| Token::new("creaseIndices"));
/// Crease lengths token
pub static CREASE_LENGTHS: LazyLock<Token> = LazyLock::new(|| Token::new("creaseLengths"));
/// Crease sharpnesses token
pub static CREASE_SHARPNESSES: LazyLock<Token> = LazyLock::new(|| Token::new("creaseSharpnesses"));

/// Data source for Token values
pub type HdTokenDataSource = dyn HdTypedSampledDataSource<Token>;
/// Arc handle to Token data source
pub type HdTokenDataSourceHandle = Arc<HdTokenDataSource>;
/// Data source for int array values
pub type HdIntArrayDataSource = dyn HdTypedSampledDataSource<Vec<i32>>;
/// Arc handle to int array data source
pub type HdIntArrayDataSourceHandle = Arc<HdIntArrayDataSource>;
/// Data source for float array values
pub type HdFloatArrayDataSource = dyn HdTypedSampledDataSource<Vec<f32>>;
/// Arc handle to float array data source
pub type HdFloatArrayDataSourceHandle = Arc<HdFloatArrayDataSource>;

/// Schema representing subdivision tags.
///
/// Provides access to:
/// - `faceVaryingLinearInterpolation` - Face varying interpolation mode
/// - `interpolateBoundary` - Boundary interpolation mode
/// - `triangleSubdivisionRule` - Triangle subdivision rule
/// - `cornerIndices` - Indices of corner vertices
/// - `cornerSharpnesses` - Sharpness values for corners
/// - `creaseIndices` - Indices of crease edges
/// - `creaseLengths` - Lengths of crease chains
/// - `creaseSharpnesses` - Sharpness values for creases
///
/// # Location
///
/// Default locator: `subdivisionTags`
#[derive(Debug, Clone)]
pub struct HdSubdivisionTagsSchema {
    schema: HdSchema,
}

impl HdSubdivisionTagsSchema {
    /// Constructs a subdivision tags schema from a container data source.
    pub fn new(container: HdContainerDataSourceHandle) -> Self {
        Self {
            schema: HdSchema::new(container),
        }
    }

    /// Retrieves subdivision tags schema from parent container at "subdivisionTags" locator.
    pub fn get_from_parent(parent: &HdContainerDataSourceHandle) -> Self {
        if let Some(child) = parent.get(&SUBDIVISION_TAGS) {
            if let Some(container) = cast_to_container(&child) {
                return Self::new(container);
            }
        }
        Self {
            schema: HdSchema::empty(),
        }
    }

    /// Returns true if the schema is non-empty.
    pub fn is_defined(&self) -> bool {
        self.schema.is_defined()
    }

    /// Gets the underlying container data source.
    pub fn get_container(&self) -> Option<&HdContainerDataSourceHandle> {
        self.schema.get_container()
    }

    /// Gets face varying linear interpolation mode.
    pub fn get_face_varying_linear_interpolation(&self) -> Option<HdTokenDataSourceHandle> {
        self.schema.get_typed(&FACE_VARYING_LINEAR_INTERPOLATION)
    }

    /// Gets interpolate boundary mode.
    pub fn get_interpolate_boundary(&self) -> Option<HdTokenDataSourceHandle> {
        self.schema.get_typed(&INTERPOLATE_BOUNDARY)
    }

    /// Gets triangle subdivision rule.
    pub fn get_triangle_subdivision_rule(&self) -> Option<HdTokenDataSourceHandle> {
        self.schema.get_typed(&TRIANGLE_SUBDIVISION_RULE)
    }

    /// Gets corner indices.
    pub fn get_corner_indices(&self) -> Option<HdIntArrayDataSourceHandle> {
        self.schema.get_typed(&CORNER_INDICES)
    }

    /// Gets corner sharpnesses.
    pub fn get_corner_sharpnesses(&self) -> Option<HdFloatArrayDataSourceHandle> {
        self.schema.get_typed(&CORNER_SHARPNESSES)
    }

    /// Gets crease indices.
    pub fn get_crease_indices(&self) -> Option<HdIntArrayDataSourceHandle> {
        self.schema.get_typed(&CREASE_INDICES)
    }

    /// Gets crease lengths.
    pub fn get_crease_lengths(&self) -> Option<HdIntArrayDataSourceHandle> {
        self.schema.get_typed(&CREASE_LENGTHS)
    }

    /// Gets crease sharpnesses.
    pub fn get_crease_sharpnesses(&self) -> Option<HdFloatArrayDataSourceHandle> {
        self.schema.get_typed(&CREASE_SHARPNESSES)
    }

    /// Returns the schema token for subdivision tags.
    pub fn get_schema_token() -> &'static LazyLock<Token> {
        &SUBDIVISION_TAGS
    }

    /// Returns the default locator for subdivision tags schema.
    pub fn get_default_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[SUBDIVISION_TAGS.clone()])
    }

    /// Returns the locator for face varying linear interpolation.
    pub fn get_face_varying_linear_interpolation_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[
            SUBDIVISION_TAGS.clone(),
            FACE_VARYING_LINEAR_INTERPOLATION.clone(),
        ])
    }

    /// Returns the locator for interpolate boundary.
    pub fn get_interpolate_boundary_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[SUBDIVISION_TAGS.clone(), INTERPOLATE_BOUNDARY.clone()])
    }

    /// Returns the locator for triangle subdivision rule.
    pub fn get_triangle_subdivision_rule_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[SUBDIVISION_TAGS.clone(), TRIANGLE_SUBDIVISION_RULE.clone()])
    }

    /// Returns the locator for corner indices.
    pub fn get_corner_indices_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[SUBDIVISION_TAGS.clone(), CORNER_INDICES.clone()])
    }

    /// Returns the locator for corner sharpnesses.
    pub fn get_corner_sharpnesses_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[SUBDIVISION_TAGS.clone(), CORNER_SHARPNESSES.clone()])
    }

    /// Returns the locator for crease indices.
    pub fn get_crease_indices_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[SUBDIVISION_TAGS.clone(), CREASE_INDICES.clone()])
    }

    /// Returns the locator for crease lengths.
    pub fn get_crease_lengths_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[SUBDIVISION_TAGS.clone(), CREASE_LENGTHS.clone()])
    }

    /// Returns the locator for crease sharpnesses.
    pub fn get_crease_sharpnesses_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[SUBDIVISION_TAGS.clone(), CREASE_SHARPNESSES.clone()])
    }

    /// Builds a retained container with subdivision tags parameters.
    ///
    /// # Parameters
    /// All subdivision tag settings as optional data source handles.
    #[allow(clippy::too_many_arguments)]
    pub fn build_retained(
        face_varying_linear_interpolation: Option<HdTokenDataSourceHandle>,
        interpolate_boundary: Option<HdTokenDataSourceHandle>,
        triangle_subdivision_rule: Option<HdTokenDataSourceHandle>,
        corner_indices: Option<HdIntArrayDataSourceHandle>,
        corner_sharpnesses: Option<HdFloatArrayDataSourceHandle>,
        crease_indices: Option<HdIntArrayDataSourceHandle>,
        crease_lengths: Option<HdIntArrayDataSourceHandle>,
        crease_sharpnesses: Option<HdFloatArrayDataSourceHandle>,
    ) -> HdContainerDataSourceHandle {
        let mut entries = Vec::new();

        if let Some(fvli) = face_varying_linear_interpolation {
            entries.push((
                FACE_VARYING_LINEAR_INTERPOLATION.clone(),
                fvli as HdDataSourceBaseHandle,
            ));
        }
        if let Some(ib) = interpolate_boundary {
            entries.push((INTERPOLATE_BOUNDARY.clone(), ib as HdDataSourceBaseHandle));
        }
        if let Some(tsr) = triangle_subdivision_rule {
            entries.push((
                TRIANGLE_SUBDIVISION_RULE.clone(),
                tsr as HdDataSourceBaseHandle,
            ));
        }
        if let Some(ci) = corner_indices {
            entries.push((CORNER_INDICES.clone(), ci as HdDataSourceBaseHandle));
        }
        if let Some(cs) = corner_sharpnesses {
            entries.push((CORNER_SHARPNESSES.clone(), cs as HdDataSourceBaseHandle));
        }
        if let Some(ci) = crease_indices {
            entries.push((CREASE_INDICES.clone(), ci as HdDataSourceBaseHandle));
        }
        if let Some(cl) = crease_lengths {
            entries.push((CREASE_LENGTHS.clone(), cl as HdDataSourceBaseHandle));
        }
        if let Some(cs) = crease_sharpnesses {
            entries.push((CREASE_SHARPNESSES.clone(), cs as HdDataSourceBaseHandle));
        }

        HdRetainedContainerDataSource::from_entries(&entries)
    }
}

/// Builder for HdSubdivisionTagsSchema.
///
/// Provides a fluent interface for constructing subdivision tags schemas.
pub struct HdSubdivisionTagsSchemaBuilder {
    face_varying_linear_interpolation: Option<HdTokenDataSourceHandle>,
    interpolate_boundary: Option<HdTokenDataSourceHandle>,
    triangle_subdivision_rule: Option<HdTokenDataSourceHandle>,
    corner_indices: Option<HdIntArrayDataSourceHandle>,
    corner_sharpnesses: Option<HdFloatArrayDataSourceHandle>,
    crease_indices: Option<HdIntArrayDataSourceHandle>,
    crease_lengths: Option<HdIntArrayDataSourceHandle>,
    crease_sharpnesses: Option<HdFloatArrayDataSourceHandle>,
}

impl HdSubdivisionTagsSchemaBuilder {
    /// Creates a new builder.
    pub fn new() -> Self {
        Self {
            face_varying_linear_interpolation: None,
            interpolate_boundary: None,
            triangle_subdivision_rule: None,
            corner_indices: None,
            corner_sharpnesses: None,
            crease_indices: None,
            crease_lengths: None,
            crease_sharpnesses: None,
        }
    }

    /// Sets the face varying linear interpolation.
    pub fn set_face_varying_linear_interpolation(mut self, value: HdTokenDataSourceHandle) -> Self {
        self.face_varying_linear_interpolation = Some(value);
        self
    }

    /// Sets the interpolate boundary.
    pub fn set_interpolate_boundary(mut self, value: HdTokenDataSourceHandle) -> Self {
        self.interpolate_boundary = Some(value);
        self
    }

    /// Sets the triangle subdivision rule.
    pub fn set_triangle_subdivision_rule(mut self, value: HdTokenDataSourceHandle) -> Self {
        self.triangle_subdivision_rule = Some(value);
        self
    }

    /// Sets the corner indices.
    pub fn set_corner_indices(mut self, value: HdIntArrayDataSourceHandle) -> Self {
        self.corner_indices = Some(value);
        self
    }

    /// Sets the corner sharpnesses.
    pub fn set_corner_sharpnesses(mut self, value: HdFloatArrayDataSourceHandle) -> Self {
        self.corner_sharpnesses = Some(value);
        self
    }

    /// Sets the crease indices.
    pub fn set_crease_indices(mut self, value: HdIntArrayDataSourceHandle) -> Self {
        self.crease_indices = Some(value);
        self
    }

    /// Sets the crease lengths.
    pub fn set_crease_lengths(mut self, value: HdIntArrayDataSourceHandle) -> Self {
        self.crease_lengths = Some(value);
        self
    }

    /// Sets the crease sharpnesses.
    pub fn set_crease_sharpnesses(mut self, value: HdFloatArrayDataSourceHandle) -> Self {
        self.crease_sharpnesses = Some(value);
        self
    }

    /// Builds the container data source.
    pub fn build(self) -> HdContainerDataSourceHandle {
        HdSubdivisionTagsSchema::build_retained(
            self.face_varying_linear_interpolation,
            self.interpolate_boundary,
            self.triangle_subdivision_rule,
            self.corner_indices,
            self.corner_sharpnesses,
            self.crease_indices,
            self.crease_lengths,
            self.crease_sharpnesses,
        )
    }
}

impl Default for HdSubdivisionTagsSchemaBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subdivision_tags_schema_empty() {
        let empty_container: HdContainerDataSourceHandle =
            HdRetainedContainerDataSource::from_entries(&[]);
        let schema = HdSubdivisionTagsSchema::get_from_parent(&empty_container);
        assert!(!schema.is_defined());
    }

    #[test]
    fn test_subdivision_tags_schema_tokens() {
        assert_eq!(SUBDIVISION_TAGS.as_str(), "subdivisionTags");
        assert_eq!(
            FACE_VARYING_LINEAR_INTERPOLATION.as_str(),
            "faceVaryingLinearInterpolation"
        );
        assert_eq!(INTERPOLATE_BOUNDARY.as_str(), "interpolateBoundary");
        assert_eq!(
            TRIANGLE_SUBDIVISION_RULE.as_str(),
            "triangleSubdivisionRule"
        );
        assert_eq!(CORNER_INDICES.as_str(), "cornerIndices");
        assert_eq!(CORNER_SHARPNESSES.as_str(), "cornerSharpnesses");
        assert_eq!(CREASE_INDICES.as_str(), "creaseIndices");
        assert_eq!(CREASE_LENGTHS.as_str(), "creaseLengths");
        assert_eq!(CREASE_SHARPNESSES.as_str(), "creaseSharpnesses");
    }

    #[test]
    fn test_subdivision_tags_schema_locators() {
        let default_loc = HdSubdivisionTagsSchema::get_default_locator();
        assert_eq!(default_loc.elements().len(), 1);

        let corner_loc = HdSubdivisionTagsSchema::get_corner_indices_locator();
        assert_eq!(corner_loc.elements().len(), 2);
    }
}
