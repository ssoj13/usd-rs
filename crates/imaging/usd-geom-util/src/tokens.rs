// Geometry utility interpolation tokens

/// Interpolation tokens for geometry normals
pub struct InterpolationTokens;

impl InterpolationTokens {
    /// Constant interpolation - single value for entire primitive
    pub const CONSTANT: &'static str = "constant";

    /// Uniform interpolation - one value per face
    pub const UNIFORM: &'static str = "uniform";

    /// Vertex interpolation - one value per vertex
    pub const VERTEX: &'static str = "vertex";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interpolation_tokens() {
        assert_eq!(InterpolationTokens::CONSTANT, "constant");
        assert_eq!(InterpolationTokens::UNIFORM, "uniform");
        assert_eq!(InterpolationTokens::VERTEX, "vertex");
    }
}
