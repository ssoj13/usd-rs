//! Port of C++ testArURIResolver.cpp
//!
//! Tests URI resolver dispatch: resolve with context, create context from
//! string with URI schemes, create default context (aggregated), create
//! default context for asset.

use std::sync::Arc;

use usd_ar::define_resolver::{ResolverMeta, define_resolver_with_meta};
use usd_ar::resolver_context::ResolverContext;
use usd_ar::{
    Asset, AssetInfo, ContextObject, DispatchingResolver, ResolvedPath, Resolver, Timestamp,
    WritableAsset, WriteMode,
};
use usd_vt::Value;

// ── _TestURIResolverContext (C++ TestArURIResolver_plugin.h) ────────────────

/// Context object for test URI resolver.
/// Matches C++ `_TestURIResolverContext`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct TestURIResolverContext {
    data: String,
}

impl TestURIResolverContext {
    fn new(data: &str) -> Self {
        Self {
            data: data.to_string(),
        }
    }
}

impl ContextObject for TestURIResolverContext {}

// ── _TestURIResolver (C++ TestArURIResolver_plugin.cpp) ────────────────────

/// Test URI resolver that handles "test://..." paths.
/// Matches C++ `_TestURIResolver`.
#[derive(Default)]
struct TestURIResolverForRegistration;

impl Resolver for TestURIResolverForRegistration {
    fn create_identifier(&self, asset_path: &str, _anchor: Option<&ResolvedPath>) -> String {
        asset_path.to_string()
    }

    fn create_identifier_for_new_asset(
        &self,
        asset_path: &str,
        _anchor: Option<&ResolvedPath>,
    ) -> String {
        asset_path.to_string()
    }

    fn resolve(&self, asset_path: &str) -> ResolvedPath {
        // C++ _Resolve: append context data as "?data" if context is bound
        let ctx = self.get_current_context();
        if let Some(uri_ctx) = ctx.get::<TestURIResolverContext>() {
            if !uri_ctx.data.is_empty() {
                return ResolvedPath::new(format!("{}?{}", asset_path, uri_ctx.data));
            }
        }
        ResolvedPath::new(asset_path)
    }

    fn resolve_for_new_asset(&self, asset_path: &str) -> ResolvedPath {
        self.resolve(asset_path)
    }

    fn bind_context(&self, _context: &ResolverContext) -> Option<Value> {
        None
    }

    fn unbind_context(&self, _context: &ResolverContext, _binding_data: Option<Value>) {}

    fn create_default_context(&self) -> ResolverContext {
        // C++ _CreateDefaultContext: returns TestURIResolverContext("CreateDefaultContext")
        ResolverContext::with_object(TestURIResolverContext::new("CreateDefaultContext"))
    }

    fn create_default_context_for_asset(&self, asset_path: &str) -> ResolverContext {
        // C++ _CreateDefaultContextForAsset: returns context with abs path
        let abs = if std::path::Path::new(asset_path).is_relative() {
            std::env::current_dir()
                .map(|cwd| cwd.join(asset_path).to_string_lossy().into_owned())
                .unwrap_or_else(|_| asset_path.to_string())
        } else {
            asset_path.to_string()
        };
        ResolverContext::with_object(TestURIResolverContext::new(&abs))
    }

    fn create_context_from_string(&self, context_str: &str) -> ResolverContext {
        ResolverContext::with_object(TestURIResolverContext::new(context_str))
    }

    fn create_context_from_string_with_scheme(
        &self,
        _uri_scheme: &str,
        context_str: &str,
    ) -> ResolverContext {
        self.create_context_from_string(context_str)
    }

    fn create_context_from_strings(&self, context_strings: &[(String, String)]) -> ResolverContext {
        let mut contexts = Vec::new();
        for (_, s) in context_strings {
            let ctx = self.create_context_from_string(s);
            if !ctx.is_empty() {
                contexts.push(ctx);
            }
        }
        ResolverContext::from_contexts(contexts)
    }

    fn refresh_context(&self, _context: &ResolverContext) {}

    fn get_current_context(&self) -> ResolverContext {
        // Read from the dispatching resolver's internal context stack
        usd_ar::dispatching_resolver::get_internally_managed_current_context().unwrap_or_default()
    }

    fn is_context_dependent_path(&self, _asset_path: &str) -> bool {
        false
    }

    fn get_extension(&self, asset_path: &str) -> String {
        std::path::Path::new(asset_path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_string()
    }

    fn get_asset_info(&self, _: &str, _: &ResolvedPath) -> AssetInfo {
        AssetInfo::default()
    }

    fn get_modification_timestamp(&self, _: &str, _: &ResolvedPath) -> Timestamp {
        Timestamp::invalid()
    }

    fn open_asset(&self, _: &ResolvedPath) -> Option<Arc<dyn Asset>> {
        None
    }

    fn open_asset_for_write(
        &self,
        _: &ResolvedPath,
        _: WriteMode,
    ) -> Option<Arc<dyn WritableAsset + Send + Sync>> {
        None
    }

    fn can_write_asset_to_path(&self, _: &ResolvedPath, _: Option<&mut String>) -> bool {
        false
    }

    fn begin_cache_scope(&self) -> Option<Value> {
        None
    }

    fn end_cache_scope(&self, _: Option<Value>) {}

    fn is_repository_path(&self, _: &str) -> bool {
        false
    }
}

// ── Test setup ─────────────────────────────────────────────────────────────

fn setup_test_uri_resolver() {
    use std::sync::Once;
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        // Register the test URI resolver with "test" URI scheme
        // Matches C++ TestArURIResolver_plugInfo.json:
        //   _TestURIResolverBase: implementsContexts=true
        //   _TestURIResolver: uriSchemes=["test"]
        define_resolver_with_meta::<TestURIResolverForRegistration>(
            "_TestURIResolver",
            ResolverMeta {
                uri_schemes: vec!["test".to_string()],
                implements_contexts: true,
                implements_scoped_caches: false,
            },
        );
    });
}

// ── Tests (C++ testArURIResolver.cpp) ──────────────────────────────────────

#[test]
fn test_dispatching_resolver_uri_creation() {
    // Verify that DispatchingResolver discovers the test URI resolver
    setup_test_uri_resolver();
    let dr = DispatchingResolver::new();
    let schemes = dr.get_uri_schemes();
    assert!(
        schemes.contains(&"test".to_string()),
        "Expected 'test' in URI schemes, got: {:?}",
        schemes
    );
}

#[test]
fn test_resolve_with_uri_scheme() {
    // Verify basic URI resolution dispatches to URI resolver
    setup_test_uri_resolver();
    let dr = DispatchingResolver::new();
    let result = dr.resolve("test://foo");
    // Without context, should just return the path as-is
    assert_eq!(result.as_str(), "test://foo");
}

#[test]
fn test_resolve_with_context() {
    // C++ TestResolveWithContext
    setup_test_uri_resolver();
    let dr = DispatchingResolver::new();

    // Bind a context with the test URI resolver context
    let ctx = ResolverContext::with_object(TestURIResolverContext::new("context"));
    dr.bind_context(&ctx);

    let result = dr.resolve("test://foo");
    assert_eq!(
        result.as_str(),
        "test://foo?context",
        "URI resolver should append context data"
    );

    // Bind another context — should override
    {
        let ctx2 = ResolverContext::with_object(TestURIResolverContext::new("context2"));
        dr.bind_context(&ctx2);
        let result2 = dr.resolve("test://foo");
        assert_eq!(result2.as_str(), "test://foo?context2");
        dr.unbind_context(&ctx2, None);
    }

    // After unbind, original context should be active again
    let result3 = dr.resolve("test://foo");
    assert_eq!(result3.as_str(), "test://foo?context");

    dr.unbind_context(&ctx, None);
}

#[test]
fn test_create_context_from_string_with_scheme() {
    // C++ TestCreateContextFromString
    setup_test_uri_resolver();
    let dr = DispatchingResolver::new();

    // Empty scheme = primary resolver
    let ctx1 = dr.create_context_from_string_with_scheme("", "/a;/b");
    // Primary resolver (DefaultResolver) creates DefaultResolverContext
    assert!(!ctx1.is_empty());

    // Bogus scheme = empty context
    let ctx_bogus = dr.create_context_from_string_with_scheme("bogus", "context string");
    assert!(ctx_bogus.is_empty());

    // "test" scheme should create TestURIResolverContext
    let ctx_test = dr.create_context_from_string_with_scheme("test", "context string");
    assert!(!ctx_test.is_empty());
    let uri_ctx = ctx_test.get::<TestURIResolverContext>();
    assert!(uri_ctx.is_some());
    assert_eq!(uri_ctx.unwrap().data, "context string");
}

#[test]
fn test_create_context_from_strings() {
    // C++ TestCreateContextFromString — CreateContextFromStrings part
    setup_test_uri_resolver();
    let dr = DispatchingResolver::new();

    // Single URI scheme entry
    let ctx = dr.create_context_from_strings(&[("test".to_string(), "context string".to_string())]);
    assert!(!ctx.is_empty());

    // Mixed entries: primary + test + bogus
    let ctx2 = dr.create_context_from_strings(&[
        ("".to_string(), "/a;/b".to_string()),
        ("test".to_string(), "context string".to_string()),
        ("bogus".to_string(), "ignored".to_string()),
    ]);
    assert!(!ctx2.is_empty());
}

#[test]
fn test_create_default_context() {
    // C++ TestCreateDefaultContext
    // CreateDefaultContext should aggregate from primary + URI resolvers
    setup_test_uri_resolver();
    let dr = DispatchingResolver::new();

    let default_ctx = dr.create_default_context();

    // Should contain TestURIResolverContext("CreateDefaultContext")
    // from the URI resolver
    let uri_ctx = default_ctx.get::<TestURIResolverContext>();
    assert!(
        uri_ctx.is_some(),
        "Default context should contain TestURIResolverContext"
    );
    assert_eq!(uri_ctx.unwrap().data, "CreateDefaultContext");
}

#[test]
fn test_create_default_context_for_asset() {
    // C++ TestCreateDefaultContextForAsset
    setup_test_uri_resolver();
    let dr = DispatchingResolver::new();

    let ctx = dr.create_default_context_for_asset("test/test.file");

    // URI resolver should have contributed a context with the abs path
    let uri_ctx = ctx.get::<TestURIResolverContext>();
    assert!(uri_ctx.is_some(), "Should contain URI resolver context");
    // The data should be an absolute path (since test resolver calls abs_path)
}

// ── URI scheme validation tests (C++ _ValidateResourceIdentifierScheme) ────

#[test]
fn test_validate_uri_scheme_valid() {
    assert!(usd_ar::validate_uri_scheme("http").is_ok());
    assert!(usd_ar::validate_uri_scheme("test").is_ok());
    assert!(usd_ar::validate_uri_scheme("test-other").is_ok());
    assert!(usd_ar::validate_uri_scheme("a1b2c3").is_ok());
    assert!(usd_ar::validate_uri_scheme("x+y.z-w").is_ok());
}

#[test]
fn test_validate_uri_scheme_invalid_empty() {
    assert!(usd_ar::validate_uri_scheme("").is_err());
}

#[test]
fn test_validate_uri_scheme_invalid_numeric_prefix() {
    // C++ _TestInvalidNumericPrefixResolver: "113-test"
    assert!(usd_ar::validate_uri_scheme("113-test").is_err());
}

#[test]
fn test_validate_uri_scheme_invalid_underscore() {
    // C++ _TestInvalidUnderbarURIResolver: "test_other"
    assert!(usd_ar::validate_uri_scheme("test_other").is_err());
}

#[test]
fn test_validate_uri_scheme_invalid_colon() {
    // C++ _TestInvalidColonURIResolver: "other:test"
    assert!(usd_ar::validate_uri_scheme("other:test").is_err());
}
