//! Asset resolver notices.
//!
//! Notices are sent when resolver state changes that may affect
//! asset resolution results.

use std::sync::Arc;

use usd_tf::notice::Notice;

use super::resolver_context::ResolverContext;

/// Type alias for context filter functions.
pub type ContextFilter = Arc<dyn Fn(&ResolverContext) -> bool + Send + Sync>;

/// Notice sent when asset paths may resolve to different paths.
///
/// This notice is sent when the resolver state has changed in a way
/// that may cause previously resolved paths to resolve differently.
///
/// # Examples
///
/// ```
/// use usd_ar::{ResolverContext, ResolverChangedNotice};
///
/// // Create a notice that affects all contexts
/// let notice = ResolverChangedNotice::new();
/// assert!(notice.affects_context(&ResolverContext::new()));
///
/// // Create a notice that only affects empty contexts
/// let notice = ResolverChangedNotice::with_filter(|ctx| ctx.is_empty());
/// assert!(notice.affects_context(&ResolverContext::new()));
/// ```
pub struct ResolverChangedNotice {
    /// Optional filter function to determine affected contexts.
    affects: Option<ContextFilter>,
}

impl ResolverChangedNotice {
    /// Creates a notice that affects all contexts.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_ar::{ResolverChangedNotice, ResolverContext};
    ///
    /// let notice = ResolverChangedNotice::new();
    /// assert!(notice.affects_context(&ResolverContext::new()));
    /// ```
    pub fn new() -> Self {
        Self { affects: None }
    }

    /// Creates a notice with a filter function.
    ///
    /// The filter function determines which contexts are affected by
    /// this resolver change. If the function returns `true` for a
    /// context, that context is affected.
    ///
    /// # Arguments
    ///
    /// * `affects` - Function that returns true if a context is affected
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_ar::{ResolverChangedNotice, ResolverContext};
    ///
    /// let notice = ResolverChangedNotice::with_filter(|ctx| ctx.is_empty());
    /// assert!(notice.affects_context(&ResolverContext::new()));
    /// ```
    pub fn with_filter<F>(affects: F) -> Self
    where
        F: Fn(&ResolverContext) -> bool + Send + Sync + 'static,
    {
        Self {
            affects: Some(Arc::new(affects)),
        }
    }

    /// Creates a notice that affects contexts containing a specific object.
    ///
    /// # Arguments
    ///
    /// * `context_obj` - The context object to match
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_ar::{ResolverChangedNotice, ResolverContext, DefaultResolverContext};
    ///
    /// let search_ctx = DefaultResolverContext::new(vec!["/path".into()]);
    /// let notice = ResolverChangedNotice::affecting_context(search_ctx.clone());
    ///
    /// let ctx = ResolverContext::with_object(search_ctx);
    /// assert!(notice.affects_context(&ctx));
    /// ```
    pub fn affecting_context<T>(context_obj: T) -> Self
    where
        T: super::resolver_context::ContextObject,
    {
        Self::with_filter(move |ctx: &ResolverContext| {
            ctx.get::<T>()
                .map(|obj| *obj == context_obj)
                .unwrap_or(false)
        })
    }

    /// Returns true if the given context is affected by this notice.
    ///
    /// # Arguments
    ///
    /// * `context` - The context to check
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_ar::{ResolverChangedNotice, ResolverContext};
    ///
    /// let notice = ResolverChangedNotice::new();
    /// assert!(notice.affects_context(&ResolverContext::new()));
    /// ```
    pub fn affects_context(&self, context: &ResolverContext) -> bool {
        match &self.affects {
            Some(filter) => filter(context),
            None => true, // Affects all contexts
        }
    }
}

impl Default for ResolverChangedNotice {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for ResolverChangedNotice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResolverChangedNotice")
            .field("has_filter", &self.affects.is_some())
            .finish()
    }
}

impl Clone for ResolverChangedNotice {
    fn clone(&self) -> Self {
        Self {
            affects: self.affects.clone(),
        }
    }
}

impl Notice for ResolverChangedNotice {
    fn notice_type_name() -> &'static str {
        "ResolverChangedNotice"
    }
}

#[cfg(test)]
mod tests {
    use super::super::resolver_context::DefaultResolverContext;
    use super::*;

    #[test]
    fn test_resolver_changed_notice_new() {
        let notice = ResolverChangedNotice::new();
        assert!(notice.affects_context(&ResolverContext::new()));
    }

    #[test]
    fn test_resolver_changed_notice_default() {
        let notice = ResolverChangedNotice::default();
        assert!(notice.affects_context(&ResolverContext::new()));
    }

    #[test]
    fn test_resolver_changed_notice_with_filter() {
        let notice = ResolverChangedNotice::with_filter(|ctx| ctx.is_empty());

        // Empty context should be affected
        assert!(notice.affects_context(&ResolverContext::new()));

        // Non-empty context should not be affected
        let ctx = ResolverContext::with_object(DefaultResolverContext::empty());
        assert!(!notice.affects_context(&ctx));
    }

    #[test]
    fn test_resolver_changed_notice_affecting_context() {
        let search_ctx = DefaultResolverContext::new(vec!["/path".into()]);
        let notice = ResolverChangedNotice::affecting_context(search_ctx.clone());

        // Matching context should be affected
        let ctx = ResolverContext::with_object(search_ctx.clone());
        assert!(notice.affects_context(&ctx));

        // Different context should not be affected
        let other_ctx =
            ResolverContext::with_object(DefaultResolverContext::new(vec!["/other".into()]));
        assert!(!notice.affects_context(&other_ctx));

        // Empty context should not be affected
        assert!(!notice.affects_context(&ResolverContext::new()));
    }

    #[test]
    fn test_resolver_changed_notice_clone() {
        let notice = ResolverChangedNotice::with_filter(|ctx| ctx.is_empty());
        let cloned = notice.clone();

        assert!(cloned.affects_context(&ResolverContext::new()));
    }

    #[test]
    fn test_resolver_changed_notice_debug() {
        let notice = ResolverChangedNotice::new();
        let debug = format!("{:?}", notice);
        assert!(debug.contains("ResolverChangedNotice"));
        assert!(debug.contains("has_filter"));

        let notice = ResolverChangedNotice::with_filter(|_| true);
        let debug = format!("{:?}", notice);
        assert!(debug.contains("true")); // has_filter: true
    }
}
