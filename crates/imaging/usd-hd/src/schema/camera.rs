//! Camera schema for Hydra.
//!
//! Defines camera parameters including projection type, aperture settings,
//! focal length, clipping planes, depth of field, motion blur, and exposure.

use super::HdSchema;
use crate::data_source::{
    HdContainerDataSourceHandle, HdDataSourceLocator, HdTypedSampledDataSource, cast_to_container,
};
use once_cell::sync::Lazy;
use std::sync::Arc;
use usd_gf::{Vec2f, Vec4d};
use usd_tf::Token;

// Schema tokens
/// Camera schema token
pub static CAMERA: Lazy<Token> = Lazy::new(|| Token::new("camera"));
/// Projection type token (perspective/orthographic)
pub static PROJECTION: Lazy<Token> = Lazy::new(|| Token::new("projection"));
/// Horizontal aperture in world units
pub static HORIZONTAL_APERTURE: Lazy<Token> = Lazy::new(|| Token::new("horizontalAperture"));
/// Vertical aperture in world units
pub static VERTICAL_APERTURE: Lazy<Token> = Lazy::new(|| Token::new("verticalAperture"));
/// Horizontal aperture offset for asymmetric frustums
pub static HORIZONTAL_APERTURE_OFFSET: Lazy<Token> =
    Lazy::new(|| Token::new("horizontalApertureOffset"));
/// Vertical aperture offset for asymmetric frustums
pub static VERTICAL_APERTURE_OFFSET: Lazy<Token> =
    Lazy::new(|| Token::new("verticalApertureOffset"));
/// Focal length in tenths of a world unit
pub static FOCAL_LENGTH: Lazy<Token> = Lazy::new(|| Token::new("focalLength"));
/// Near/far clipping range as Vec2f
pub static CLIPPING_RANGE: Lazy<Token> = Lazy::new(|| Token::new("clippingRange"));
/// Additional arbitrary clipping planes as Vec4d array
pub static CLIPPING_PLANES: Lazy<Token> = Lazy::new(|| Token::new("clippingPlanes"));
/// F-stop for depth of field
pub static F_STOP: Lazy<Token> = Lazy::new(|| Token::new("fStop"));
/// Focus distance for depth of field
pub static FOCUS_DISTANCE: Lazy<Token> = Lazy::new(|| Token::new("focusDistance"));
/// Shutter open time for motion blur
pub static SHUTTER_OPEN: Lazy<Token> = Lazy::new(|| Token::new("shutterOpen"));
/// Shutter close time for motion blur
pub static SHUTTER_CLOSE: Lazy<Token> = Lazy::new(|| Token::new("shutterClose"));
/// Exposure compensation value
pub static EXPOSURE: Lazy<Token> = Lazy::new(|| Token::new("exposure"));
/// Namespaced camera properties contributed by applied API schemas.
pub static NAMESPACED_PROPERTIES: Lazy<Token> = Lazy::new(|| Token::new("namespacedProperties"));

// Projection mode tokens
/// Perspective projection token
#[allow(dead_code)]
pub static PERSPECTIVE: Lazy<Token> = Lazy::new(|| Token::new("perspective"));
/// Orthographic projection token
#[allow(dead_code)]
pub static ORTHOGRAPHIC: Lazy<Token> = Lazy::new(|| Token::new("orthographic"));

// Typed data sources
/// Data source for Token values
pub type HdTokenDataSource = dyn HdTypedSampledDataSource<Token>;
/// Arc handle to Token data source
pub type HdTokenDataSourceHandle = Arc<HdTokenDataSource>;

/// Data source for f32 values
pub type HdFloatDataSource = dyn HdTypedSampledDataSource<f32>;
/// Arc handle to f32 data source
pub type HdFloatDataSourceHandle = Arc<HdFloatDataSource>;

/// Data source for f64 values
pub type HdDoubleDataSource = dyn HdTypedSampledDataSource<f64>;
/// Arc handle to f64 data source
pub type HdDoubleDataSourceHandle = Arc<HdDoubleDataSource>;

/// Data source for Vec2f values
pub type HdVec2fDataSource = dyn HdTypedSampledDataSource<Vec2f>;
/// Arc handle to Vec2f data source
pub type HdVec2fDataSourceHandle = Arc<HdVec2fDataSource>;

/// Data source for Vec4d arrays
pub type HdVec4dArrayDataSource = dyn HdTypedSampledDataSource<Vec<Vec4d>>;
/// Arc handle to Vec4d array data source
pub type HdVec4dArrayDataSourceHandle = Arc<HdVec4dArrayDataSource>;

/// Schema representing camera parameters.
///
/// Provides access to camera settings including:
/// - `projection` - Projection type (perspective/orthographic)
/// - `horizontalAperture`, `verticalAperture` - Aperture dimensions
/// - `focalLength` - Focal length
/// - `clippingRange` - Near/far clipping planes
/// - `fStop`, `focusDistance` - Depth of field settings
/// - `shutterOpen`, `shutterClose` - Motion blur shutter interval
/// - `exposure` - Exposure value
///
/// # Location
///
/// Default locator: `camera`
#[derive(Debug, Clone)]
pub struct HdCameraSchema {
    schema: HdSchema,
}

impl HdCameraSchema {
    /// Constructs a camera schema from a container data source.
    pub fn new(container: HdContainerDataSourceHandle) -> Self {
        Self {
            schema: HdSchema::new(container),
        }
    }

    /// Retrieves camera schema from parent container at "camera" locator.
    pub fn get_from_parent(parent: &HdContainerDataSourceHandle) -> Self {
        if let Some(child) = parent.get(&CAMERA) {
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

    /// Gets projection type data source (perspective/orthographic).
    pub fn get_projection(&self) -> Option<HdTokenDataSourceHandle> {
        self.schema.get_typed(&PROJECTION)
    }

    /// Gets horizontal aperture data source in world units.
    pub fn get_horizontal_aperture(&self) -> Option<HdFloatDataSourceHandle> {
        self.schema.get_typed(&HORIZONTAL_APERTURE)
    }

    /// Gets vertical aperture data source in world units.
    pub fn get_vertical_aperture(&self) -> Option<HdFloatDataSourceHandle> {
        self.schema.get_typed(&VERTICAL_APERTURE)
    }

    /// Gets horizontal aperture offset for asymmetric frustums.
    pub fn get_horizontal_aperture_offset(&self) -> Option<HdFloatDataSourceHandle> {
        self.schema.get_typed(&HORIZONTAL_APERTURE_OFFSET)
    }

    /// Gets vertical aperture offset for asymmetric frustums.
    pub fn get_vertical_aperture_offset(&self) -> Option<HdFloatDataSourceHandle> {
        self.schema.get_typed(&VERTICAL_APERTURE_OFFSET)
    }

    /// Gets focal length data source in tenths of a world unit.
    pub fn get_focal_length(&self) -> Option<HdFloatDataSourceHandle> {
        self.schema.get_typed(&FOCAL_LENGTH)
    }

    /// Gets near/far clipping range as Vec2f.
    pub fn get_clipping_range(&self) -> Option<HdVec2fDataSourceHandle> {
        self.schema.get_typed(&CLIPPING_RANGE)
    }

    /// Gets additional clipping planes as Vec4d array.
    pub fn get_clipping_planes(&self) -> Option<HdVec4dArrayDataSourceHandle> {
        self.schema.get_typed(&CLIPPING_PLANES)
    }

    /// Gets f-stop for depth of field.
    pub fn get_f_stop(&self) -> Option<HdFloatDataSourceHandle> {
        self.schema.get_typed(&F_STOP)
    }

    /// Gets focus distance for depth of field.
    pub fn get_focus_distance(&self) -> Option<HdFloatDataSourceHandle> {
        self.schema.get_typed(&FOCUS_DISTANCE)
    }

    /// Gets shutter open time for motion blur.
    pub fn get_shutter_open(&self) -> Option<HdDoubleDataSourceHandle> {
        self.schema.get_typed(&SHUTTER_OPEN)
    }

    /// Gets shutter close time for motion blur.
    pub fn get_shutter_close(&self) -> Option<HdDoubleDataSourceHandle> {
        self.schema.get_typed(&SHUTTER_CLOSE)
    }

    /// Gets exposure compensation value.
    pub fn get_exposure(&self) -> Option<HdFloatDataSourceHandle> {
        self.schema.get_typed(&EXPOSURE)
    }

    /// Gets namespaced camera properties container.
    pub fn get_namespaced_properties(&self) -> Option<HdContainerDataSourceHandle> {
        self.get_container()
            .and_then(|container| container.get(&NAMESPACED_PROPERTIES))
            .as_ref()
            .and_then(cast_to_container)
    }

    /// Returns the schema token for camera.
    pub fn get_schema_token() -> &'static Lazy<Token> {
        &CAMERA
    }

    /// Returns the default locator for camera schema.
    pub fn get_default_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[CAMERA.clone()])
    }

    /// Returns the locator for namespaced camera properties.
    pub fn get_namespaced_properties_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[CAMERA.clone(), NAMESPACED_PROPERTIES.clone()])
    }

    /// Locator for shutterOpen (motion blur).
    pub fn get_shutter_open_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[CAMERA.clone(), SHUTTER_OPEN.clone()])
    }

    /// Locator for shutterClose (motion blur).
    pub fn get_shutter_close_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[CAMERA.clone(), SHUTTER_CLOSE.clone()])
    }

    /// Builds a retained container with camera parameters.
    ///
    /// # Parameters
    /// All camera settings as optional data source handles.
    #[allow(clippy::too_many_arguments)]
    pub fn build_retained(
        projection: Option<HdTokenDataSourceHandle>,
        horizontal_aperture: Option<HdFloatDataSourceHandle>,
        vertical_aperture: Option<HdFloatDataSourceHandle>,
        horizontal_aperture_offset: Option<HdFloatDataSourceHandle>,
        vertical_aperture_offset: Option<HdFloatDataSourceHandle>,
        focal_length: Option<HdFloatDataSourceHandle>,
        clipping_range: Option<HdVec2fDataSourceHandle>,
        clipping_planes: Option<HdVec4dArrayDataSourceHandle>,
        f_stop: Option<HdFloatDataSourceHandle>,
        focus_distance: Option<HdFloatDataSourceHandle>,
        shutter_open: Option<HdDoubleDataSourceHandle>,
        shutter_close: Option<HdDoubleDataSourceHandle>,
        exposure: Option<HdFloatDataSourceHandle>,
        namespaced_properties: Option<HdContainerDataSourceHandle>,
    ) -> HdContainerDataSourceHandle {
        use crate::data_source::{HdDataSourceBaseHandle, HdRetainedContainerDataSource};

        let mut entries = Vec::new();

        if let Some(v) = projection {
            entries.push((PROJECTION.clone(), v as HdDataSourceBaseHandle));
        }
        if let Some(v) = horizontal_aperture {
            entries.push((HORIZONTAL_APERTURE.clone(), v as HdDataSourceBaseHandle));
        }
        if let Some(v) = vertical_aperture {
            entries.push((VERTICAL_APERTURE.clone(), v as HdDataSourceBaseHandle));
        }
        if let Some(v) = horizontal_aperture_offset {
            entries.push((
                HORIZONTAL_APERTURE_OFFSET.clone(),
                v as HdDataSourceBaseHandle,
            ));
        }
        if let Some(v) = vertical_aperture_offset {
            entries.push((
                VERTICAL_APERTURE_OFFSET.clone(),
                v as HdDataSourceBaseHandle,
            ));
        }
        if let Some(v) = focal_length {
            entries.push((FOCAL_LENGTH.clone(), v as HdDataSourceBaseHandle));
        }
        if let Some(v) = clipping_range {
            entries.push((CLIPPING_RANGE.clone(), v as HdDataSourceBaseHandle));
        }
        if let Some(v) = clipping_planes {
            entries.push((CLIPPING_PLANES.clone(), v as HdDataSourceBaseHandle));
        }
        if let Some(v) = f_stop {
            entries.push((F_STOP.clone(), v as HdDataSourceBaseHandle));
        }
        if let Some(v) = focus_distance {
            entries.push((FOCUS_DISTANCE.clone(), v as HdDataSourceBaseHandle));
        }
        if let Some(v) = shutter_open {
            entries.push((SHUTTER_OPEN.clone(), v as HdDataSourceBaseHandle));
        }
        if let Some(v) = shutter_close {
            entries.push((SHUTTER_CLOSE.clone(), v as HdDataSourceBaseHandle));
        }
        if let Some(v) = exposure {
            entries.push((EXPOSURE.clone(), v as HdDataSourceBaseHandle));
        }
        if let Some(v) = namespaced_properties {
            entries.push((NAMESPACED_PROPERTIES.clone(), v as HdDataSourceBaseHandle));
        }

        HdRetainedContainerDataSource::from_entries(&entries)
    }
}

/// Builder for constructing HdCameraSchema containers.
///
/// Provides a fluent interface for setting camera parameters.
#[allow(dead_code)]
#[derive(Default)]
pub struct HdCameraSchemaBuilder {
    projection: Option<HdTokenDataSourceHandle>,
    horizontal_aperture: Option<HdFloatDataSourceHandle>,
    vertical_aperture: Option<HdFloatDataSourceHandle>,
    horizontal_aperture_offset: Option<HdFloatDataSourceHandle>,
    vertical_aperture_offset: Option<HdFloatDataSourceHandle>,
    focal_length: Option<HdFloatDataSourceHandle>,
    clipping_range: Option<HdVec2fDataSourceHandle>,
    clipping_planes: Option<HdVec4dArrayDataSourceHandle>,
    f_stop: Option<HdFloatDataSourceHandle>,
    focus_distance: Option<HdFloatDataSourceHandle>,
    shutter_open: Option<HdDoubleDataSourceHandle>,
    shutter_close: Option<HdDoubleDataSourceHandle>,
    exposure: Option<HdFloatDataSourceHandle>,
    namespaced_properties: Option<HdContainerDataSourceHandle>,
}

#[allow(dead_code)]
impl HdCameraSchemaBuilder {
    /// Creates a new empty camera schema builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets projection type (perspective/orthographic).
    pub fn set_projection(mut self, v: HdTokenDataSourceHandle) -> Self {
        self.projection = Some(v);
        self
    }

    /// Sets horizontal aperture in world units.
    pub fn set_horizontal_aperture(mut self, v: HdFloatDataSourceHandle) -> Self {
        self.horizontal_aperture = Some(v);
        self
    }

    /// Sets vertical aperture in world units.
    pub fn set_vertical_aperture(mut self, v: HdFloatDataSourceHandle) -> Self {
        self.vertical_aperture = Some(v);
        self
    }

    /// Sets horizontal aperture offset.
    pub fn set_horizontal_aperture_offset(mut self, v: HdFloatDataSourceHandle) -> Self {
        self.horizontal_aperture_offset = Some(v);
        self
    }

    /// Sets vertical aperture offset.
    pub fn set_vertical_aperture_offset(mut self, v: HdFloatDataSourceHandle) -> Self {
        self.vertical_aperture_offset = Some(v);
        self
    }

    /// Sets focal length in tenths of a world unit.
    pub fn set_focal_length(mut self, v: HdFloatDataSourceHandle) -> Self {
        self.focal_length = Some(v);
        self
    }

    /// Sets near/far clipping range.
    pub fn set_clipping_range(mut self, v: HdVec2fDataSourceHandle) -> Self {
        self.clipping_range = Some(v);
        self
    }

    /// Sets additional clipping planes.
    pub fn set_clipping_planes(mut self, v: HdVec4dArrayDataSourceHandle) -> Self {
        self.clipping_planes = Some(v);
        self
    }

    /// Sets f-stop for depth of field.
    pub fn set_f_stop(mut self, v: HdFloatDataSourceHandle) -> Self {
        self.f_stop = Some(v);
        self
    }

    /// Sets focus distance for depth of field.
    pub fn set_focus_distance(mut self, v: HdFloatDataSourceHandle) -> Self {
        self.focus_distance = Some(v);
        self
    }

    /// Sets shutter open time.
    pub fn set_shutter_open(mut self, v: HdDoubleDataSourceHandle) -> Self {
        self.shutter_open = Some(v);
        self
    }

    /// Sets shutter close time.
    pub fn set_shutter_close(mut self, v: HdDoubleDataSourceHandle) -> Self {
        self.shutter_close = Some(v);
        self
    }

    /// Sets exposure compensation value.
    pub fn set_exposure(mut self, v: HdFloatDataSourceHandle) -> Self {
        self.exposure = Some(v);
        self
    }

    /// Sets namespaced camera properties.
    pub fn set_namespaced_properties(mut self, v: HdContainerDataSourceHandle) -> Self {
        self.namespaced_properties = Some(v);
        self
    }

    /// Builds the container with all set camera parameters.
    pub fn build(self) -> HdContainerDataSourceHandle {
        HdCameraSchema::build_retained(
            self.projection,
            self.horizontal_aperture,
            self.vertical_aperture,
            self.horizontal_aperture_offset,
            self.vertical_aperture_offset,
            self.focal_length,
            self.clipping_range,
            self.clipping_planes,
            self.f_stop,
            self.focus_distance,
            self.shutter_open,
            self.shutter_close,
            self.exposure,
            self.namespaced_properties,
        )
    }
}
