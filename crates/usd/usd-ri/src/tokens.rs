//! UsdRi tokens.
//!
//! Static tokens for RenderMan integration schemas.
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdRi/tokens.h`

use std::sync::LazyLock;

use usd_tf::Token;

/// Token definitions for UsdRi schemas.
pub struct UsdRiTokensType {
    /// "bspline" - BSpline spline interpolation
    pub bspline: Token,
    /// "cameraVisibility" - Collection name for camera visibility
    pub camera_visibility: Token,
    /// "catmull-rom" - Catmull-Rom spline interpolation
    pub catmull_rom: Token,
    /// "constant" - Constant-value spline interpolation
    pub constant: Token,
    /// "interpolation" - Interpolation attribute name
    pub interpolation: Token,
    /// "linear" - Linear spline interpolation
    pub linear: Token,
    /// "matte" - Collection name for matte attribute
    pub matte: Token,
    /// "outputs:ri:displacement" - RiMaterialAPI displacement output
    pub outputs_ri_displacement: Token,
    /// "outputs:ri:surface" - RiMaterialAPI surface output
    pub outputs_ri_surface: Token,
    /// "outputs:ri:volume" - RiMaterialAPI volume output
    pub outputs_ri_volume: Token,
    /// "positions" - Positions attribute name
    pub positions: Token,
    /// "ri" - Render context token for UsdRi
    pub render_context: Token,
    /// "spline" - Namespace for spline attributes
    pub spline: Token,
    /// "values" - Values attribute name
    pub values: Token,
    /// "RiMaterialAPI" - Schema identifier for RiMaterialAPI
    pub ri_material_api: Token,
    /// "RiSplineAPI" - Schema identifier for RiSplineAPI
    pub ri_spline_api: Token,
    /// "StatementsAPI" - Schema identifier for StatementsAPI
    pub statements_api: Token,
    /// "ri:attributes" - Namespace prefix for Ri attributes
    pub ri_attributes: Token,
    /// "ri:coordinateSystem" - Coordinate system attribute
    pub ri_coordinate_system: Token,
    /// "ri:scopedCoordinateSystem" - Scoped coordinate system attribute
    pub ri_scoped_coordinate_system: Token,
    /// "ri:modelCoordinateSystems" - Model coordinate systems relationship
    pub ri_model_coordinate_systems: Token,
    /// "ri:modelScopedCoordinateSystems" - Model scoped coordinate systems
    pub ri_model_scoped_coordinate_systems: Token,
}

impl UsdRiTokensType {
    /// Create token instances.
    fn new() -> Self {
        Self {
            bspline: Token::new("bspline"),
            camera_visibility: Token::new("cameraVisibility"),
            catmull_rom: Token::new("catmull-rom"),
            constant: Token::new("constant"),
            interpolation: Token::new("interpolation"),
            linear: Token::new("linear"),
            matte: Token::new("matte"),
            outputs_ri_displacement: Token::new("outputs:ri:displacement"),
            outputs_ri_surface: Token::new("outputs:ri:surface"),
            outputs_ri_volume: Token::new("outputs:ri:volume"),
            positions: Token::new("positions"),
            render_context: Token::new("ri"),
            spline: Token::new("spline"),
            values: Token::new("values"),
            ri_material_api: Token::new("RiMaterialAPI"),
            ri_spline_api: Token::new("RiSplineAPI"),
            statements_api: Token::new("StatementsAPI"),
            ri_attributes: Token::new("ri:attributes"),
            ri_coordinate_system: Token::new("ri:coordinateSystem"),
            ri_scoped_coordinate_system: Token::new("ri:scopedCoordinateSystem"),
            ri_model_coordinate_systems: Token::new("ri:modelCoordinateSystems"),
            ri_model_scoped_coordinate_systems: Token::new("ri:modelScopedCoordinateSystems"),
        }
    }

    /// Get all tokens as a vector.
    pub fn all_tokens(&self) -> Vec<Token> {
        vec![
            self.bspline.clone(),
            self.camera_visibility.clone(),
            self.catmull_rom.clone(),
            self.constant.clone(),
            self.interpolation.clone(),
            self.linear.clone(),
            self.matte.clone(),
            self.outputs_ri_displacement.clone(),
            self.outputs_ri_surface.clone(),
            self.outputs_ri_volume.clone(),
            self.positions.clone(),
            self.render_context.clone(),
            self.spline.clone(),
            self.values.clone(),
            self.ri_material_api.clone(),
            self.ri_spline_api.clone(),
            self.statements_api.clone(),
        ]
    }
}

/// Global static tokens instance.
pub static USD_RI_TOKENS: LazyLock<UsdRiTokensType> = LazyLock::new(UsdRiTokensType::new);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokens() {
        assert_eq!(USD_RI_TOKENS.bspline.as_str(), "bspline");
        assert_eq!(USD_RI_TOKENS.render_context.as_str(), "ri");
        assert_eq!(USD_RI_TOKENS.ri_material_api.as_str(), "RiMaterialAPI");
    }
}
