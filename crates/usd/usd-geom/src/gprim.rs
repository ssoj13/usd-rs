//! UsdGeomGprim - base class for all geometric primitives.
//!
//! Port of pxr/usd/usdGeom/gprim.h/cpp
//!
//! Gprim encodes basic graphical properties such as doubleSided and
//! orientation, and provides primvars for "display color" and "display
//! opacity".

use super::boundable::Boundable;
use super::primvar::Primvar;
use super::primvars_api::PrimvarsAPI;
use super::tokens::usd_geom_tokens;
use usd_core::{Attribute, Prim};
use usd_tf::Token;

// ============================================================================
// Gprim
// ============================================================================

/// Base class for all geometric primitives.
///
/// Gprim encodes basic graphical properties such as doubleSided and
/// orientation, and provides primvars for "display color" and "display
/// opacity".
///
/// Matches C++ `UsdGeomGprim`.
#[derive(Debug, Clone)]
pub struct Gprim {
    /// Base boundable schema.
    inner: Boundable,
}

impl Gprim {
    /// Creates a Gprim schema from a prim.
    ///
    /// Matches C++ `UsdGeomGprim(const UsdPrim& prim)`.
    pub fn new(prim: Prim) -> Self {
        Self {
            inner: Boundable::new(prim),
        }
    }

    /// Creates a Gprim schema from a Boundable schema.
    ///
    /// Matches C++ `UsdGeomGprim(const UsdSchemaBase& schemaObj)`.
    pub fn from_boundable(boundable: Boundable) -> Self {
        Self { inner: boundable }
    }

    /// Creates an invalid Gprim schema.
    pub fn invalid() -> Self {
        Self {
            inner: Boundable::invalid(),
        }
    }

    /// Returns true if this schema is valid.
    pub fn is_valid(&self) -> bool {
        self.inner.is_valid()
    }

    /// Returns the wrapped prim.
    pub fn prim(&self) -> &Prim {
        self.inner.prim()
    }

    /// Returns the boundable base.
    pub fn boundable(&self) -> &Boundable {
        &self.inner
    }

    /// Returns the schema type name.
    pub fn schema_type_name() -> Token {
        Token::new("Gprim")
    }

    // ========================================================================
    // DisplayColor
    // ========================================================================

    /// Returns the displayColor attribute.
    ///
    /// It is useful to have an "official" colorSet that can be used
    /// as a display or modeling color, even in the absence of any specified
    /// shader for a gprim.
    ///
    /// Matches C++ `GetDisplayColorAttr()`.
    pub fn get_display_color_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().primvars_display_color.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the displayColor attribute.
    ///
    /// Matches C++ `CreateDisplayColorAttr()`.
    pub fn create_display_color_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        // Get or create the attribute with proper type (Color3fArray) and variability (Varying)
        if prim.has_authored_attribute(usd_geom_tokens().primvars_display_color.as_str()) {
            return prim
                .get_attribute(usd_geom_tokens().primvars_display_color.as_str())
                .unwrap_or_else(|| Attribute::invalid());
        }

        let registry = usd_sdf::ValueTypeRegistry::instance();
        let color3f_array_type = registry.find_type_by_token(&Token::new("color3f[]"));

        prim.create_attribute(
            usd_geom_tokens().primvars_display_color.as_str(),
            &color3f_array_type,
            false,                                           // not custom
            Some(usd_core::attribute::Variability::Varying), // can vary over time
        )
        .unwrap_or_else(Attribute::invalid)
    }

    // ========================================================================
    // DisplayOpacity
    // ========================================================================

    /// Returns the displayOpacity attribute.
    ///
    /// Companion to displayColor that specifies opacity, broken
    /// out as an independent attribute rather than an rgba color.
    ///
    /// Matches C++ `GetDisplayOpacityAttr()`.
    pub fn get_display_opacity_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().primvars_display_opacity.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the displayOpacity attribute.
    ///
    /// Matches C++ `CreateDisplayOpacityAttr()`.
    pub fn create_display_opacity_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        // Get or create the attribute with proper type (FloatArray) and variability (Varying)
        if prim.has_authored_attribute(usd_geom_tokens().primvars_display_opacity.as_str()) {
            return prim
                .get_attribute(usd_geom_tokens().primvars_display_opacity.as_str())
                .unwrap_or_else(|| Attribute::invalid());
        }

        let registry = usd_sdf::ValueTypeRegistry::instance();
        let float_array_type = registry.find_type_by_token(&Token::new("float[]"));

        prim.create_attribute(
            usd_geom_tokens().primvars_display_opacity.as_str(),
            &float_array_type,
            false,                                           // not custom
            Some(usd_core::attribute::Variability::Varying), // can vary over time
        )
        .unwrap_or_else(Attribute::invalid)
    }

    // ========================================================================
    // DoubleSided
    // ========================================================================

    /// Returns the doubleSided attribute.
    ///
    /// Setting a gprim's doubleSided attribute to true instructs all
    /// renderers to disable optimizations such as backface culling for the gprim.
    ///
    /// Matches C++ `GetDoubleSidedAttr()`.
    pub fn get_double_sided_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().double_sided.as_str())
            .or_else(|| prim.get_attribute_handle(usd_geom_tokens().double_sided.as_str()))
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the doubleSided attribute.
    ///
    /// Matches C++ `CreateDoubleSidedAttr()`.
    pub fn create_double_sided_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        // Get or create the attribute with proper type (Bool) and variability (Uniform)
        if prim.has_authored_attribute(usd_geom_tokens().double_sided.as_str()) {
            return prim
                .get_attribute(usd_geom_tokens().double_sided.as_str())
                .unwrap_or_else(|| Attribute::invalid());
        }

        let registry = usd_sdf::ValueTypeRegistry::instance();
        let bool_type = registry.find_type_by_token(&Token::new("bool"));

        prim.create_attribute(
            usd_geom_tokens().double_sided.as_str(),
            &bool_type,
            false,                                           // not custom
            Some(usd_core::attribute::Variability::Uniform), // doubleSided is uniform
        )
        .unwrap_or_else(Attribute::invalid)
    }

    // ========================================================================
    // Orientation
    // ========================================================================

    /// Returns the orientation attribute.
    ///
    /// Orientation specifies whether the gprim's surface normal
    /// should be computed using the right hand rule, or the left hand rule.
    ///
    /// Matches C++ `GetOrientationAttr()`.
    pub fn get_orientation_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        if let Some(attr) = prim.get_attribute(usd_geom_tokens().orientation.as_str()) {
            attr
        } else {
            self.create_orientation_attr()
        }
    }

    /// Creates the orientation attribute.
    ///
    /// Matches C++ `CreateOrientationAttr()`.
    pub fn create_orientation_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        // Get or create the attribute with proper type (Token) and variability (Uniform)
        if prim.has_authored_attribute(usd_geom_tokens().orientation.as_str()) {
            return prim
                .get_attribute(usd_geom_tokens().orientation.as_str())
                .unwrap_or_else(|| Attribute::invalid());
        }

        let registry = usd_sdf::ValueTypeRegistry::instance();
        let token_type = registry.find_type_by_token(&Token::new("token"));

        prim.create_attribute(
            usd_geom_tokens().orientation.as_str(),
            &token_type,
            false,                                           // not custom
            Some(usd_core::attribute::Variability::Uniform), // orientation is uniform
        )
        .unwrap_or_else(Attribute::invalid)
    }

    /// Get the displayColor primvar.
    ///
    /// Matches C++ `GetDisplayColorPrimvar()`.
    pub fn get_display_color_primvar(&self) -> Primvar {
        let prim = self.inner.prim();
        let primvars_api = PrimvarsAPI::new(prim.clone());
        primvars_api.get_primvar(&usd_geom_tokens().display_color)
    }

    /// Create the displayColor primvar.
    ///
    /// Matches C++ `CreateDisplayColorPrimvar()`.
    pub fn create_display_color_primvar(
        &self,
        interpolation: &Token,
        element_size: i32,
    ) -> Primvar {
        let prim = self.inner.prim();
        let primvars_api = PrimvarsAPI::new(prim.clone());
        let registry = usd_sdf::ValueTypeRegistry::instance();
        let type_name = registry.find_type_by_token(&Token::new("color3f[]"));
        primvars_api.create_primvar(
            &usd_geom_tokens().display_color,
            &type_name,
            Some(interpolation),
            element_size,
        )
    }

    /// Get the displayOpacity primvar.
    ///
    /// Matches C++ `GetDisplayOpacityPrimvar()`.
    pub fn get_display_opacity_primvar(&self) -> Primvar {
        let prim = self.inner.prim();
        let primvars_api = PrimvarsAPI::new(prim.clone());
        primvars_api.get_primvar(&usd_geom_tokens().display_opacity)
    }

    /// Create the displayOpacity primvar.
    ///
    /// Matches C++ `CreateDisplayOpacityPrimvar()`.
    pub fn create_display_opacity_primvar(
        &self,
        interpolation: &Token,
        element_size: i32,
    ) -> Primvar {
        let prim = self.inner.prim();
        let primvars_api = PrimvarsAPI::new(prim.clone());
        let registry = usd_sdf::ValueTypeRegistry::instance();
        let type_name = registry.find_type_by_token(&Token::new("float[]"));
        primvars_api.create_primvar(
            &usd_geom_tokens().display_opacity,
            &type_name,
            Some(interpolation),
            element_size,
        )
    }

    /// Return a vector of names of all pre-declared attributes for this schema
    /// class and all its ancestor classes.
    ///
    /// Matches C++ `GetSchemaAttributeNames()`.
    pub fn get_schema_attribute_names(include_inherited: bool) -> Vec<Token> {
        let local_names = vec![
            usd_geom_tokens().primvars_display_color.clone(),
            usd_geom_tokens().primvars_display_opacity.clone(),
            usd_geom_tokens().double_sided.clone(),
            usd_geom_tokens().orientation.clone(),
        ];

        if include_inherited {
            let mut all_names = Boundable::get_schema_attribute_names(true);
            all_names.extend(local_names);
            all_names
        } else {
            local_names
        }
    }

    /// Return a Gprim holding the prim adhering to this schema at path on stage.
    ///
    /// Matches C++ `Get(const UsdStagePtr &stage, const SdfPath &path)`.
    pub fn get(stage: &usd_core::Stage, path: &usd_sdf::Path) -> Self {
        if let Some(prim) = stage.get_prim_at_path(path) {
            Self::new(prim)
        } else {
            Self::invalid()
        }
    }
}

impl PartialEq for Gprim {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl Eq for Gprim {}
