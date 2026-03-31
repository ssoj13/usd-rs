// Port of testenv/testArNotice.cpp
// Tests ArNotice::ResolverChanged with filters and context matching

use usd_ar::{ContextObject, ResolverChangedNotice, ResolverContext};

// Custom context types matching C++ TestResolverContext<int> and TestResolverContext<string>
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
struct IntContext {
    data: i32,
}

impl ContextObject for IntContext {}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
struct StringContext {
    data: String,
}

impl ContextObject for StringContext {}

#[test]
fn test_resolver_changed_notice_affects_all() {
    // ArNotice::ResolverChanged() -- no filter, affects everything
    let notice = ResolverChangedNotice::new();
    assert!(notice.affects_context(&ResolverContext::new()));

    let mut ctx = ResolverContext::new();
    ctx.add(IntContext { data: 0 });
    ctx.add(StringContext {
        data: "s".to_string(),
    });
    assert!(notice.affects_context(&ctx));
}

#[test]
fn test_resolver_changed_notice_affects_specific_context() {
    // ArNotice::ResolverChanged(IntContext(0)) -- only affects contexts containing IntContext(0)
    let notice = ResolverChangedNotice::affecting_context(IntContext { data: 0 });

    // Empty context -- not affected
    assert!(!notice.affects_context(&ResolverContext::new()));

    // Context with IntContext(1) -- not affected
    let ctx_1 = ResolverContext::with_object(IntContext { data: 1 });
    assert!(!notice.affects_context(&ctx_1));

    // Context with IntContext(0) -- affected
    let ctx_0 = ResolverContext::with_object(IntContext { data: 0 });
    assert!(notice.affects_context(&ctx_0));

    // Context with IntContext(0) + StringContext("s") -- affected
    let mut ctx_both = ResolverContext::new();
    ctx_both.add(IntContext { data: 0 });
    ctx_both.add(StringContext {
        data: "s".to_string(),
    });
    assert!(notice.affects_context(&ctx_both));
}

#[test]
fn test_resolver_changed_notice_with_lambda_filter() {
    // ArNotice::ResolverChanged([](ctx) { ... }) -- custom filter
    let notice = ResolverChangedNotice::with_filter(|ctx: &ResolverContext| {
        ctx.get::<StringContext>()
            .map(|s| s.data.contains("needle"))
            .unwrap_or(false)
    });

    // Empty context -- not affected
    assert!(!notice.affects_context(&ResolverContext::new()));

    // IntContext only -- not affected
    let ctx_int = ResolverContext::with_object(IntContext { data: 0 });
    assert!(!notice.affects_context(&ctx_int));

    // StringContext("s") -- not affected (no "needle")
    let mut ctx_no_needle = ResolverContext::new();
    ctx_no_needle.add(IntContext { data: 0 });
    ctx_no_needle.add(StringContext {
        data: "s".to_string(),
    });
    assert!(!notice.affects_context(&ctx_no_needle));

    // StringContext("test-needle") -- affected
    let ctx_needle = ResolverContext::with_object(StringContext {
        data: "test-needle".to_string(),
    });
    assert!(notice.affects_context(&ctx_needle));

    // IntContext(0) + StringContext("test-needle") -- affected
    let mut ctx_both_needle = ResolverContext::new();
    ctx_both_needle.add(IntContext { data: 0 });
    ctx_both_needle.add(StringContext {
        data: "test-needle".to_string(),
    });
    assert!(notice.affects_context(&ctx_both_needle));
}
