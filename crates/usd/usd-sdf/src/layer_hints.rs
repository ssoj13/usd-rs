//! Layer content hints for composition optimization.
//!
//! `LayerHints` contains hints about layer contents that may be used
//! to accelerate certain composition operations.

/// Contains hints about layer contents.
///
/// These hints may be used to accelerate composition operations.
/// Default constructed hints provide conservative values that ensure
/// correct behavior even if not optimal.
///
/// # Examples
///
/// ```ignore
/// use usd_sdf::LayerHints;
///
/// // Default hints are conservative (assume everything might be present)
/// let default_hints = LayerHints::default();
/// assert!(default_hints.might_have_relocates());
///
/// // Create hints with specific values
/// let hints = LayerHints::new(false);
/// assert!(!hints.might_have_relocates());
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LayerHints {
    /// If false, the layer does not contain relocates.
    /// If true, relocates may be present but are not guaranteed to exist.
    might_have_relocates: bool,
}

impl Default for LayerHints {
    /// Default hints are conservative - assume relocates might exist.
    fn default() -> Self {
        Self {
            might_have_relocates: true,
        }
    }
}

impl LayerHints {
    /// Creates new hints with specific values.
    ///
    /// # Arguments
    ///
    /// * `might_have_relocates` - Whether the layer might contain relocates
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::LayerHints;
    ///
    /// let hints = LayerHints::new(false);
    /// assert!(!hints.might_have_relocates());
    /// ```
    pub fn new(might_have_relocates: bool) -> Self {
        Self {
            might_have_relocates,
        }
    }

    /// Creates conservative hints that assume all features might be present.
    ///
    /// This is equivalent to `LayerHints::default()`.
    pub fn conservative() -> Self {
        Self {
            might_have_relocates: true,
        }
    }

    /// Returns whether the layer might contain relocates.
    ///
    /// If this returns `false`, the layer is guaranteed to not contain relocates.
    /// If this returns `true`, relocates may be present but are not guaranteed.
    #[inline]
    pub fn might_have_relocates(&self) -> bool {
        self.might_have_relocates
    }

    /// Sets whether the layer might contain relocates.
    pub fn set_might_have_relocates(&mut self, value: bool) {
        self.might_have_relocates = value;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default() {
        let hints = LayerHints::default();
        // Default is conservative - assumes relocates might exist
        assert!(hints.might_have_relocates());
    }

    #[test]
    fn test_new() {
        let hints = LayerHints::new(false);
        assert!(!hints.might_have_relocates());

        let hints2 = LayerHints::new(true);
        assert!(hints2.might_have_relocates());
    }

    #[test]
    fn test_conservative() {
        let hints = LayerHints::conservative();
        assert!(hints.might_have_relocates());
    }

    #[test]
    fn test_setter() {
        let mut hints = LayerHints::default();
        assert!(hints.might_have_relocates());

        hints.set_might_have_relocates(false);
        assert!(!hints.might_have_relocates());
    }

    #[test]
    fn test_equality() {
        let h1 = LayerHints::new(false);
        let h2 = LayerHints::new(false);
        let h3 = LayerHints::new(true);

        assert_eq!(h1, h2);
        assert_ne!(h1, h3);
    }

    #[test]
    fn test_clone() {
        let hints = LayerHints::new(false);
        let cloned = hints;
        assert_eq!(hints, cloned);
    }
}
