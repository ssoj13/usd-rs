//! Render product schema for Hydra.
//!
//! Defines an output render product (image or AOV) including resolution,
//! camera, pixel aspect ratio, and render variables.

use super::HdSchema;
use crate::data_source::{
    HdContainerDataSourceHandle, HdDataSourceLocator, HdTypedSampledDataSource, cast_to_container,
};
use once_cell::sync::Lazy;
use std::sync::Arc;
use usd_gf::{Vec2f, Vec2i, Vec4f};
use usd_sdf::Path;
use usd_tf::Token;

// Schema tokens
/// Render product schema token
pub static RENDER_PRODUCT: Lazy<Token> = Lazy::new(|| Token::new("renderProduct"));
/// Output file path token
pub static PATH: Lazy<Token> = Lazy::new(|| Token::new("path"));
/// Product type token
pub static TYPE: Lazy<Token> = Lazy::new(|| Token::new("type"));
/// Product name token
pub static NAME: Lazy<Token> = Lazy::new(|| Token::new("name"));
/// Output resolution token
pub static RESOLUTION: Lazy<Token> = Lazy::new(|| Token::new("resolution"));
/// Render variables (AOVs) token
pub static RENDER_VARS: Lazy<Token> = Lazy::new(|| Token::new("renderVars"));
/// Camera prim path token
pub static CAMERA_PRIM: Lazy<Token> = Lazy::new(|| Token::new("cameraPrim"));
/// Pixel aspect ratio token
pub static PIXEL_ASPECT_RATIO: Lazy<Token> = Lazy::new(|| Token::new("pixelAspectRatio"));
/// Aspect ratio conform policy token
pub static ASPECT_RATIO_CONFORM_POLICY: Lazy<Token> =
    Lazy::new(|| Token::new("aspectRatioConformPolicy"));
/// Camera aperture size override token
pub static APERTURE_SIZE: Lazy<Token> = Lazy::new(|| Token::new("apertureSize"));
/// Data window in NDC space token
pub static DATA_WINDOW_NDC: Lazy<Token> = Lazy::new(|| Token::new("dataWindowNDC"));
/// Disable motion blur flag token
pub static DISABLE_MOTION_BLUR: Lazy<Token> = Lazy::new(|| Token::new("disableMotionBlur"));
/// Disable depth of field flag token
pub static DISABLE_DEPTH_OF_FIELD: Lazy<Token> = Lazy::new(|| Token::new("disableDepthOfField"));
/// Renderer-specific settings token
pub static NAMESPACED_SETTINGS: Lazy<Token> = Lazy::new(|| Token::new("namespacedSettings"));

// Typed data sources
/// Data source for Path values
pub type HdPathDataSource = dyn HdTypedSampledDataSource<Path>;
/// Arc handle to Path data source
pub type HdPathDataSourceHandle = Arc<HdPathDataSource>;

/// Data source for Token values
pub type HdTokenDataSource = dyn HdTypedSampledDataSource<Token>;
/// Arc handle to Token data source
pub type HdTokenDataSourceHandle = Arc<HdTokenDataSource>;

/// Data source for Vec2i values
pub type HdVec2iDataSource = dyn HdTypedSampledDataSource<Vec2i>;
/// Arc handle to Vec2i data source
pub type HdVec2iDataSourceHandle = Arc<HdVec2iDataSource>;

/// Data source for Vec2f values
pub type HdVec2fDataSource = dyn HdTypedSampledDataSource<Vec2f>;
/// Arc handle to Vec2f data source
pub type HdVec2fDataSourceHandle = Arc<HdVec2fDataSource>;

/// Data source for Vec4f values
pub type HdVec4fDataSource = dyn HdTypedSampledDataSource<Vec4f>;
/// Arc handle to Vec4f data source
pub type HdVec4fDataSourceHandle = Arc<HdVec4fDataSource>;

/// Data source for f32 values
pub type HdFloatDataSource = dyn HdTypedSampledDataSource<f32>;
/// Arc handle to f32 data source
pub type HdFloatDataSourceHandle = Arc<HdFloatDataSource>;

/// Data source for bool values
pub type HdBoolDataSource = dyn HdTypedSampledDataSource<bool>;
/// Arc handle to bool data source
pub type HdBoolDataSourceHandle = Arc<HdBoolDataSource>;

/// Schema representing a render product (output image/AOV).
///
/// Provides access to:
/// - `path` - Output file path
/// - `type` - Product type
/// - `name` - Product name
/// - `resolution` - Output resolution
/// - `renderVars` - Vector of RenderVar schemas (AOVs)
/// - `cameraPrim` - Path to camera prim
/// - `pixelAspectRatio` - Pixel aspect ratio
/// - `aspectRatioConformPolicy` - How to handle aspect ratio
/// - `apertureSize` - Camera aperture size override
/// - `dataWindowNDC` - Data window in NDC space
/// - `disableMotionBlur` - Disable motion blur for this product
/// - `disableDepthOfField` - Disable depth of field for this product
/// - `namespacedSettings` - Renderer-specific settings
///
/// # Location
///
/// Default locator: `renderProduct`
#[derive(Debug, Clone)]
pub struct HdRenderProductSchema {
    schema: HdSchema,
}

impl HdRenderProductSchema {
    /// Constructs a render product schema from a container data source.
    pub fn new(container: HdContainerDataSourceHandle) -> Self {
        Self {
            schema: HdSchema::new(container),
        }
    }

    /// Retrieves render product schema from parent container.
    pub fn get_from_parent(parent: &HdContainerDataSourceHandle) -> Self {
        if let Some(child) = parent.get(&RENDER_PRODUCT) {
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

    /// Gets output file path.
    pub fn get_path(&self) -> Option<HdPathDataSourceHandle> {
        self.schema.get_typed(&PATH)
    }

    /// Gets product type.
    pub fn get_type(&self) -> Option<HdTokenDataSourceHandle> {
        self.schema.get_typed(&TYPE)
    }

    /// Gets product name.
    pub fn get_name(&self) -> Option<HdTokenDataSourceHandle> {
        self.schema.get_typed(&NAME)
    }

    /// Gets output resolution as Vec2i.
    pub fn get_resolution(&self) -> Option<HdVec2iDataSourceHandle> {
        self.schema.get_typed(&RESOLUTION)
    }

    /// Gets render variables (AOVs) container.
    pub fn get_render_vars(&self) -> Option<HdContainerDataSourceHandle> {
        if let Some(container) = self.get_container() {
            if let Some(child) = container.get(&RENDER_VARS) {
                return cast_to_container(&child);
            }
        }
        None
    }

    /// Gets camera prim path.
    pub fn get_camera_prim(&self) -> Option<HdPathDataSourceHandle> {
        self.schema.get_typed(&CAMERA_PRIM)
    }

    /// Gets pixel aspect ratio.
    pub fn get_pixel_aspect_ratio(&self) -> Option<HdFloatDataSourceHandle> {
        self.schema.get_typed(&PIXEL_ASPECT_RATIO)
    }

    /// Gets aspect ratio conform policy.
    pub fn get_aspect_ratio_conform_policy(&self) -> Option<HdTokenDataSourceHandle> {
        self.schema.get_typed(&ASPECT_RATIO_CONFORM_POLICY)
    }

    /// Gets camera aperture size override.
    pub fn get_aperture_size(&self) -> Option<HdVec2fDataSourceHandle> {
        self.schema.get_typed(&APERTURE_SIZE)
    }

    /// Gets data window in NDC space as Vec4f.
    pub fn get_data_window_ndc(&self) -> Option<HdVec4fDataSourceHandle> {
        self.schema.get_typed(&DATA_WINDOW_NDC)
    }

    /// Gets flag to disable motion blur for this product.
    pub fn get_disable_motion_blur(&self) -> Option<HdBoolDataSourceHandle> {
        self.schema.get_typed(&DISABLE_MOTION_BLUR)
    }

    /// Gets flag to disable depth of field for this product.
    pub fn get_disable_depth_of_field(&self) -> Option<HdBoolDataSourceHandle> {
        self.schema.get_typed(&DISABLE_DEPTH_OF_FIELD)
    }

    /// Gets renderer-specific settings container.
    pub fn get_namespaced_settings(&self) -> Option<HdContainerDataSourceHandle> {
        if let Some(container) = self.get_container() {
            if let Some(child) = container.get(&NAMESPACED_SETTINGS) {
                return cast_to_container(&child);
            }
        }
        None
    }

    /// Returns the schema token for render product.
    pub fn get_schema_token() -> &'static Lazy<Token> {
        &RENDER_PRODUCT
    }

    /// Returns the default locator for render product schema.
    pub fn get_default_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[RENDER_PRODUCT.clone()])
    }

    /// Builds a retained container with render product parameters.
    ///
    /// # Parameters
    /// All render product settings as optional data source handles.
    #[allow(clippy::too_many_arguments)]
    pub fn build_retained(
        path: Option<HdPathDataSourceHandle>,
        type_: Option<HdTokenDataSourceHandle>,
        name: Option<HdTokenDataSourceHandle>,
        resolution: Option<HdVec2iDataSourceHandle>,
        render_vars: Option<HdContainerDataSourceHandle>,
        camera_prim: Option<HdPathDataSourceHandle>,
        pixel_aspect_ratio: Option<HdFloatDataSourceHandle>,
        aspect_ratio_conform_policy: Option<HdTokenDataSourceHandle>,
        aperture_size: Option<HdVec2fDataSourceHandle>,
        data_window_ndc: Option<HdVec4fDataSourceHandle>,
        disable_motion_blur: Option<HdBoolDataSourceHandle>,
        disable_depth_of_field: Option<HdBoolDataSourceHandle>,
        namespaced_settings: Option<HdContainerDataSourceHandle>,
    ) -> HdContainerDataSourceHandle {
        use crate::data_source::{HdDataSourceBaseHandle, HdRetainedContainerDataSource};

        let mut entries = Vec::new();

        if let Some(v) = path {
            entries.push((PATH.clone(), v as HdDataSourceBaseHandle));
        }
        if let Some(v) = type_ {
            entries.push((TYPE.clone(), v as HdDataSourceBaseHandle));
        }
        if let Some(v) = name {
            entries.push((NAME.clone(), v as HdDataSourceBaseHandle));
        }
        if let Some(v) = resolution {
            entries.push((RESOLUTION.clone(), v as HdDataSourceBaseHandle));
        }
        if let Some(v) = render_vars {
            entries.push((RENDER_VARS.clone(), v as HdDataSourceBaseHandle));
        }
        if let Some(v) = camera_prim {
            entries.push((CAMERA_PRIM.clone(), v as HdDataSourceBaseHandle));
        }
        if let Some(v) = pixel_aspect_ratio {
            entries.push((PIXEL_ASPECT_RATIO.clone(), v as HdDataSourceBaseHandle));
        }
        if let Some(v) = aspect_ratio_conform_policy {
            entries.push((
                ASPECT_RATIO_CONFORM_POLICY.clone(),
                v as HdDataSourceBaseHandle,
            ));
        }
        if let Some(v) = aperture_size {
            entries.push((APERTURE_SIZE.clone(), v as HdDataSourceBaseHandle));
        }
        if let Some(v) = data_window_ndc {
            entries.push((DATA_WINDOW_NDC.clone(), v as HdDataSourceBaseHandle));
        }
        if let Some(v) = disable_motion_blur {
            entries.push((DISABLE_MOTION_BLUR.clone(), v as HdDataSourceBaseHandle));
        }
        if let Some(v) = disable_depth_of_field {
            entries.push((DISABLE_DEPTH_OF_FIELD.clone(), v as HdDataSourceBaseHandle));
        }
        if let Some(v) = namespaced_settings {
            entries.push((NAMESPACED_SETTINGS.clone(), v as HdDataSourceBaseHandle));
        }

        HdRetainedContainerDataSource::from_entries(&entries)
    }
}

/// Builder for constructing HdRenderProductSchema containers.
///
/// Provides a fluent interface for setting render product parameters.
#[allow(dead_code)]
#[derive(Default)]
pub struct HdRenderProductSchemaBuilder {
    path: Option<HdPathDataSourceHandle>,
    type_: Option<HdTokenDataSourceHandle>,
    name: Option<HdTokenDataSourceHandle>,
    resolution: Option<HdVec2iDataSourceHandle>,
    render_vars: Option<HdContainerDataSourceHandle>,
    camera_prim: Option<HdPathDataSourceHandle>,
    pixel_aspect_ratio: Option<HdFloatDataSourceHandle>,
    aspect_ratio_conform_policy: Option<HdTokenDataSourceHandle>,
    aperture_size: Option<HdVec2fDataSourceHandle>,
    data_window_ndc: Option<HdVec4fDataSourceHandle>,
    disable_motion_blur: Option<HdBoolDataSourceHandle>,
    disable_depth_of_field: Option<HdBoolDataSourceHandle>,
    namespaced_settings: Option<HdContainerDataSourceHandle>,
}

#[allow(dead_code)]
impl HdRenderProductSchemaBuilder {
    /// Creates a new empty render product schema builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets output file path.
    pub fn set_path(mut self, v: HdPathDataSourceHandle) -> Self {
        self.path = Some(v);
        self
    }

    /// Sets product type.
    pub fn set_type(mut self, v: HdTokenDataSourceHandle) -> Self {
        self.type_ = Some(v);
        self
    }

    /// Sets product name.
    pub fn set_name(mut self, v: HdTokenDataSourceHandle) -> Self {
        self.name = Some(v);
        self
    }

    /// Sets output resolution.
    pub fn set_resolution(mut self, v: HdVec2iDataSourceHandle) -> Self {
        self.resolution = Some(v);
        self
    }

    /// Sets render variables (AOVs).
    pub fn set_render_vars(mut self, v: HdContainerDataSourceHandle) -> Self {
        self.render_vars = Some(v);
        self
    }

    /// Sets camera prim path.
    pub fn set_camera_prim(mut self, v: HdPathDataSourceHandle) -> Self {
        self.camera_prim = Some(v);
        self
    }

    /// Sets pixel aspect ratio.
    pub fn set_pixel_aspect_ratio(mut self, v: HdFloatDataSourceHandle) -> Self {
        self.pixel_aspect_ratio = Some(v);
        self
    }

    /// Sets aspect ratio conform policy.
    pub fn set_aspect_ratio_conform_policy(mut self, v: HdTokenDataSourceHandle) -> Self {
        self.aspect_ratio_conform_policy = Some(v);
        self
    }

    /// Sets camera aperture size override.
    pub fn set_aperture_size(mut self, v: HdVec2fDataSourceHandle) -> Self {
        self.aperture_size = Some(v);
        self
    }

    /// Sets data window in NDC space.
    pub fn set_data_window_ndc(mut self, v: HdVec4fDataSourceHandle) -> Self {
        self.data_window_ndc = Some(v);
        self
    }

    /// Sets flag to disable motion blur.
    pub fn set_disable_motion_blur(mut self, v: HdBoolDataSourceHandle) -> Self {
        self.disable_motion_blur = Some(v);
        self
    }

    /// Sets flag to disable depth of field.
    pub fn set_disable_depth_of_field(mut self, v: HdBoolDataSourceHandle) -> Self {
        self.disable_depth_of_field = Some(v);
        self
    }

    /// Sets renderer-specific settings.
    pub fn set_namespaced_settings(mut self, v: HdContainerDataSourceHandle) -> Self {
        self.namespaced_settings = Some(v);
        self
    }

    /// Builds the container with all set render product parameters.
    pub fn build(self) -> HdContainerDataSourceHandle {
        HdRenderProductSchema::build_retained(
            self.path,
            self.type_,
            self.name,
            self.resolution,
            self.render_vars,
            self.camera_prim,
            self.pixel_aspect_ratio,
            self.aspect_ratio_conform_policy,
            self.aperture_size,
            self.data_window_ndc,
            self.disable_motion_blur,
            self.disable_depth_of_field,
            self.namespaced_settings,
        )
    }
}
