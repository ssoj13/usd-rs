// Port of testenv/testArResolverContext.cpp
// Tests ArResolverContext: empty, single object, multiple objects, equality

use usd_ar::{ContextObject, ResolverContext};

// Custom context types matching C++ TestContextObject<string> and TestContextObject<int>
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
struct TestStringContext {
    data: String,
}

impl ContextObject for TestStringContext {}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
struct TestIntContext {
    data: i32,
}

impl ContextObject for TestIntContext {}

#[test]
fn test_default() {
    let ctx = ResolverContext::new();
    assert!(ctx.is_empty());
    assert!(ctx.get::<TestStringContext>().is_none());
    assert!(ctx.get::<TestIntContext>().is_none());

    let ctx2 = ResolverContext::new();
    assert!(ctx2.is_empty());
    assert!(ctx2.get::<TestStringContext>().is_none());
    assert!(ctx2.get::<TestIntContext>().is_none());
    assert_eq!(ctx, ctx2);
}

#[test]
fn test_single_context_object() {
    // Create context holding a single string context object
    let str_ctx_obj = TestStringContext {
        data: "test string".to_string(),
    };
    let ctx1 = ResolverContext::with_object(str_ctx_obj.clone());
    assert!(!ctx1.is_empty());

    let str_from_ctx = ctx1.get::<TestStringContext>();
    assert!(str_from_ctx.is_some());
    assert_eq!(str_from_ctx.expect("just checked").data, "test string");

    let int_from_ctx = ctx1.get::<TestIntContext>();
    assert!(int_from_ctx.is_none());

    // Create an equal context
    let ctx2 = ResolverContext::with_object(TestStringContext {
        data: "test string".to_string(),
    });
    assert_eq!(ctx1, ctx2);

    // Empty context is not equal
    let ctx3 = ResolverContext::new();
    assert_ne!(ctx1, ctx3);

    // Different string is not equal
    let ctx4 = ResolverContext::with_object(TestStringContext {
        data: "foo".to_string(),
    });
    assert_ne!(ctx1, ctx4);

    // Different type is not equal
    let ctx5 = ResolverContext::with_object(TestIntContext { data: 42 });
    assert_ne!(ctx1, ctx5);
}

#[test]
fn test_multiple_context_objects() {
    // Create context with both string and int context objects
    let mut context = ResolverContext::new();
    context.add(TestStringContext {
        data: "test string".to_string(),
    });
    context.add(TestIntContext { data: 42 });
    assert!(!context.is_empty());
    assert_ne!(context, ResolverContext::new());

    let str_from_context = context.get::<TestStringContext>();
    assert!(str_from_context.is_some());
    assert_eq!(str_from_context.expect("checked").data, "test string");

    let int_from_context = context.get::<TestIntContext>();
    assert!(int_from_context.is_some());
    assert_eq!(int_from_context.expect("checked").data, 42);

    // Same objects added in different order should be equal
    {
        let mut test_context = ResolverContext::new();
        test_context.add(TestIntContext { data: 42 });
        test_context.add(TestStringContext {
            data: "test string".to_string(),
        });
        assert_eq!(context, test_context);
    }

    // Context with only int is not equal
    {
        let test_context = ResolverContext::with_object(TestIntContext { data: 42 });
        assert_ne!(context, test_context);
    }

    // Context with only string (different) is not equal
    {
        let test_context = ResolverContext::with_object(TestStringContext {
            data: "foo".to_string(),
        });
        assert_ne!(context, test_context);
    }

    // Context with both but different values is not equal
    {
        let mut test_context = ResolverContext::new();
        test_context.add(TestStringContext {
            data: "foo".to_string(),
        });
        test_context.add(TestIntContext { data: 42 });
        assert_ne!(context, test_context);
    }
}

#[test]
fn test_context_hash_equality() {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut ctx1 = ResolverContext::new();
    ctx1.add(TestStringContext {
        data: "test".to_string(),
    });
    ctx1.add(TestIntContext { data: 42 });

    // Same objects, different insertion order
    let mut ctx2 = ResolverContext::new();
    ctx2.add(TestIntContext { data: 42 });
    ctx2.add(TestStringContext {
        data: "test".to_string(),
    });

    assert_eq!(ctx1, ctx2);

    let mut h1 = DefaultHasher::new();
    let mut h2 = DefaultHasher::new();
    ctx1.hash(&mut h1);
    ctx2.hash(&mut h2);
    assert_eq!(h1.finish(), h2.finish());
}
