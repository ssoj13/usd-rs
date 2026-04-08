//! HF diagnostic codes and validation utilities.
//!
//! Provides macros for issuing validation warnings and errors specific
//! to Hydra primitives and scene description.

/// Issues a validation warning for an invalid Hydra primitive.
///
/// This macro is a placeholder for future validation system integration.
/// Currently logs a warning message identifying the invalid prim.
///
/// # Arguments
///
/// * `id` - The identifier of the invalid prim (must have `.as_str()` method)
/// * `fmt` - Format string for the error message
/// * `args` - Format arguments
///
/// # Example
///
/// ```ignore
/// use usd_hf::hf_validation_warn;
/// use usd_sdf::Path;
///
/// let prim_path = Path::new("/World/InvalidMesh");
/// hf_validation_warn!(prim_path, "Missing required attribute: {}", "points");
/// ```
#[macro_export]
macro_rules! hf_validation_warn {
    ($id:expr, $fmt:literal $(, $args:expr)*) => {
        log::warn!(
            "Invalid Hydra prim '{}': {}",
            $id.as_str(),
            format!($fmt $(, $args)*)
        );
    };
}

// Re-export at module level
pub use hf_validation_warn;

#[cfg(test)]
mod tests {

    struct MockPath {
        path: String,
    }

    impl MockPath {
        fn new(path: &str) -> Self {
            Self {
                path: path.to_string(),
            }
        }

        fn as_str(&self) -> &str {
            &self.path
        }
    }

    #[test]
    fn test_validation_warn_compiles() {
        let path = MockPath::new("/World/TestPrim");

        // Should compile without errors
        hf_validation_warn!(path, "Test warning: {}", "missing data");
    }

    #[test]
    fn test_validation_warn_with_multiple_args() {
        let path = MockPath::new("/World/Mesh");

        hf_validation_warn!(path, "Invalid topology: {} vertices, {} faces", 100, 50);
    }

    #[test]
    fn test_validation_warn_simple() {
        let path = MockPath::new("/Root");

        hf_validation_warn!(path, "Simple error message");
    }
}
