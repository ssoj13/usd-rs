//! UsdImaging version information.

/// Major version number
pub const VERSION_MAJOR: u32 = 0;

/// Minor version number
pub const VERSION_MINOR: u32 = 1;

/// Patch version number
pub const VERSION_PATCH: u32 = 0;

/// Full version string
pub fn version_string() -> String {
    format!("{}.{}.{}", VERSION_MAJOR, VERSION_MINOR, VERSION_PATCH)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        let ver = version_string();
        assert!(ver.contains(&VERSION_MAJOR.to_string()));
        assert!(ver.contains(&VERSION_MINOR.to_string()));
        assert!(ver.contains(&VERSION_PATCH.to_string()));
    }
}
