use std::thread;
use usd_tf::scope_description::{ScopeDescription, get_scope_stack, scope_depth};
use usd_tf::tf_describe_scope;

// Helper: push a deeply-nested set of scopes then let them all drop.
fn push_pop_stack_descriptions(i: i32) {
    let _d1 = ScopeDescription::new(format!("Description {} 1", i));
    let _d3 = ScopeDescription::new(format!("Description {} 3", i));
    let _dm = ScopeDescription::new("=== Intermission ===");
    let _d5 = ScopeDescription::new(format!("Description {} 5", i));
    let _d6 = ScopeDescription::new(format!("Description {} 6", i));
    let _d7 = ScopeDescription::new(format!("Description {} 7", i));
    let _d8 = ScopeDescription::new(format!("Description {} 8", i));
    let _df = ScopeDescription::new("!!! Finale !!!");
    let _d10 = ScopeDescription::new(format!("Description {} 10", i));
}

#[test]
fn test_basics() {
    // Stack must be empty before we push anything.
    assert!(get_scope_stack().is_empty());

    {
        let _one = ScopeDescription::new("one");

        let stack = get_scope_stack();
        assert_eq!(stack.len(), 1);
        assert_eq!(stack.last().unwrap(), "one");

        {
            let _two = ScopeDescription::new("two");

            let stack = get_scope_stack();
            assert_eq!(stack.len(), 2);
            assert_eq!(stack.last().unwrap(), "two");
        }

        // "two" was dropped — only "one" should remain.
        let stack = get_scope_stack();
        assert_eq!(stack.len(), 1);
        assert_eq!(stack.last().unwrap(), "one");

        {
            // Equivalent to TF_DESCRIBE_SCOPE("%s", "three").
            let _three = ScopeDescription::new(format!("{}", "three"));

            let stack = get_scope_stack();
            assert_eq!(stack.len(), 2);
            assert_eq!(stack.last().unwrap(), "three");
        }

        let stack = get_scope_stack();
        assert_eq!(stack.len(), 1);
        assert_eq!(stack.last().unwrap(), "one");
    }

    // All scopes dropped — stack must be empty again.
    assert!(get_scope_stack().is_empty());
}

#[test]
fn test_threads() {
    // Spawn 64 threads that each push/pop nested descriptions in a loop for
    // ~1 second (we cap the iterations so CI stays fast).
    const NTHREADS: usize = 64;
    const ITERS: usize = 100;

    let handles: Vec<_> = (0..NTHREADS)
        .map(|i| {
            thread::spawn(move || {
                for _ in 0..ITERS {
                    push_pop_stack_descriptions(i as i32);
                    // Stack must be empty between calls.
                    assert!(get_scope_stack().is_empty());
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().expect("thread panicked");
    }
}

#[test]
fn test_macro_basic() {
    assert_eq!(scope_depth(), 0);

    {
        tf_describe_scope!("macro test");
        assert_eq!(scope_depth(), 1);
        assert_eq!(get_scope_stack().last().unwrap(), "macro test");
    }

    assert_eq!(scope_depth(), 0);
}

#[test]
fn test_macro_with_format() {
    {
        tf_describe_scope!("item {}", 42);
        assert_eq!(get_scope_stack().last().unwrap(), "item 42");
    }
}

#[test]
fn test_set_description() {
    let desc = ScopeDescription::new("initial");
    assert_eq!(get_scope_stack().last().unwrap(), "initial");

    desc.set_description("updated");
    assert_eq!(get_scope_stack().last().unwrap(), "updated");
}

#[test]
fn test_set_description_outer_with_nested() {
    // set_description on the outer scope must not overwrite the inner one.
    let outer = ScopeDescription::new("outer initial");
    let _inner = ScopeDescription::new("inner");

    outer.set_description("outer updated");

    let stack = get_scope_stack();
    assert_eq!(stack[0], "outer updated");
    assert_eq!(stack[1], "inner");
}

#[test]
fn test_nested_depth() {
    let _d1 = ScopeDescription::new("a");
    let _d2 = ScopeDescription::new("b");
    let _d3 = ScopeDescription::new("c");

    assert_eq!(scope_depth(), 3);

    let stack = get_scope_stack();
    assert_eq!(stack, vec!["a", "b", "c"]);
}
