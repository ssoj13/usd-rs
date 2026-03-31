// Port of C++ testenv/notice.cpp — TfNotice register/send/revoke tests.
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use usd_tf::notice::{ListenerKey, Notice, NoticeRegistry, SenderId};

// ============================================================
// Notice type hierarchy (mirrors C++ TestNotice / BaseNotice /
// MainNotice / WorkerNotice)
// ============================================================

/// Equivalent to C++ TestNotice : public TfNotice.
#[derive(Clone)]
struct TestNotice {
    what: String,
}

impl Notice for TestNotice {
    fn notice_type_name() -> &'static str {
        "TestNotice"
    }
}

/// C++ BaseNotice : public TfNotice
#[derive(Clone)]
#[allow(dead_code)]
struct BaseNotice {
    what: String,
}

impl Notice for BaseNotice {
    fn notice_type_name() -> &'static str {
        "BaseNotice"
    }
}

/// C++ MainNotice : public BaseNotice
#[derive(Clone)]
#[allow(dead_code)]
struct MainNotice {
    what: String,
}

impl Notice for MainNotice {
    fn notice_type_name() -> &'static str {
        "MainNotice"
    }
}

/// C++ WorkerNotice : public BaseNotice
#[derive(Clone)]
#[allow(dead_code)]
struct WorkerNotice {
    what: String,
}

impl Notice for WorkerNotice {
    fn notice_type_name() -> &'static str {
        "WorkerNotice"
    }
}

// ============================================================
// Test: Register two listeners, send, both receive.
// C++: l1/l2 registered for TfNotice and TestNotice, first .Send() call.
// ============================================================
#[test]
fn test_register_and_send_basic() {
    let registry = NoticeRegistry::new();
    let hits = Arc::new(AtomicUsize::new(0));

    // l1: registered for TestNotice (process_notice equivalent)
    let h1 = hits.clone();
    let _k1 = registry.register_global::<TestNotice, _>(move |_| {
        h1.fetch_add(1, Ordering::SeqCst);
    });

    // l2: also registered for TestNotice
    let h2 = hits.clone();
    let _k2 = registry.register_global::<TestNotice, _>(move |_| {
        h2.fetch_add(1, Ordering::SeqCst);
    });

    let count = registry.send(&TestNotice {
        what: "first".into(),
    });
    assert_eq!(count, 2, "both listeners must receive the notice");
    assert_eq!(hits.load(Ordering::SeqCst), 2);
}

// ============================================================
// Test: Revoke one key — the revoked listener stops receiving.
// C++: TfNotice::Revoke(l2Key2) then TestNotice("third").Send(wl1).
// ============================================================
#[test]
fn test_revoke_stops_delivery() {
    let registry = NoticeRegistry::new();
    let count1 = Arc::new(AtomicUsize::new(0));
    let count2 = Arc::new(AtomicUsize::new(0));

    let c1 = count1.clone();
    let _k1 = registry.register_global::<TestNotice, _>(move |_| {
        c1.fetch_add(1, Ordering::SeqCst);
    });

    let c2 = count2.clone();
    let k2 = registry.register_global::<TestNotice, _>(move |_| {
        c2.fetch_add(1, Ordering::SeqCst);
    });

    registry.send(&TestNotice {
        what: "before revoke".into(),
    });
    assert_eq!(count1.load(Ordering::SeqCst), 1);
    assert_eq!(count2.load(Ordering::SeqCst), 1);

    // Revoke k2 (mirrors C++ TfNotice::Revoke(l2Key2))
    assert!(registry.revoke(k2));

    registry.send(&TestNotice {
        what: "after revoke".into(),
    });
    assert_eq!(count1.load(Ordering::SeqCst), 2, "l1 must still receive");
    assert_eq!(
        count2.load(Ordering::SeqCst),
        1,
        "l2 must NOT receive after revoke"
    );
}

// ============================================================
// Test: Double revoke is a no-op (key becomes invalid).
// ============================================================
#[test]
fn test_double_revoke_is_noop() {
    let registry = NoticeRegistry::new();
    let count = Arc::new(AtomicUsize::new(0));
    let c = count.clone();

    let key = registry.register_global::<TestNotice, _>(move |_| {
        c.fetch_add(1, Ordering::SeqCst);
    });

    assert!(key.is_valid());
    assert!(registry.revoke(key.clone()), "first revoke must succeed");
    assert!(!key.is_valid());
    assert!(!registry.revoke(key), "second revoke must return false");

    // No delivery after revoke.
    registry.send(&TestNotice {
        what: "after double revoke".into(),
    });
    assert_eq!(count.load(Ordering::SeqCst), 0);
}

// ============================================================
// Test: Per-sender registration — only matching sender fires the callback.
// C++: Register(wl2, &ProcessMyTestNotice, wl2) — sender-filtered listener.
// ============================================================
#[test]
fn test_per_sender_registration() {
    let registry = NoticeRegistry::new();
    let sender1 = SenderId::new();
    let sender2 = SenderId::new();

    let global_hits = Arc::new(AtomicUsize::new(0));
    let sender1_hits = Arc::new(AtomicUsize::new(0));

    let gh = global_hits.clone();
    let _global_key = registry.register_global::<TestNotice, _>(move |_| {
        gh.fetch_add(1, Ordering::SeqCst);
    });

    let sh = sender1_hits.clone();
    let _sender_key = registry.register_for_sender::<TestNotice, _>(sender1, move |_| {
        sh.fetch_add(1, Ordering::SeqCst);
    });

    // Send from sender1 — global listener + sender-specific listener both fire.
    registry.send_from(
        &TestNotice {
            what: "from sender1".into(),
        },
        sender1,
    );
    assert_eq!(global_hits.load(Ordering::SeqCst), 1);
    assert_eq!(sender1_hits.load(Ordering::SeqCst), 1);

    // Send from sender2 — only global listener fires.
    registry.send_from(
        &TestNotice {
            what: "from sender2".into(),
        },
        sender2,
    );
    assert_eq!(global_hits.load(Ordering::SeqCst), 2);
    assert_eq!(
        sender1_hits.load(Ordering::SeqCst),
        1,
        "sender-filtered listener must not fire for wrong sender"
    );

    // Global send (no sender) — only global listener fires.
    registry.send(&TestNotice {
        what: "global send".into(),
    });
    assert_eq!(global_hits.load(Ordering::SeqCst), 3);
    assert_eq!(sender1_hits.load(Ordering::SeqCst), 1);
}

// ============================================================
// Test: Notice block — delivery is suppressed while the guard is held.
// C++: TfNotice::Block noticeBlock; ... assert hits unchanged.
// ============================================================
#[test]
fn test_notice_block() {
    let registry = NoticeRegistry::new();
    let hits = Arc::new(AtomicUsize::new(0));
    let h = hits.clone();

    let _key = registry.register_global::<TestNotice, _>(move |_| {
        h.fetch_add(1, Ordering::SeqCst);
    });

    // Before block: delivery works.
    registry.send(&TestNotice {
        what: "not blocked".into(),
    });
    assert_eq!(hits.load(Ordering::SeqCst), 1);

    {
        let _block = registry.block();
        assert!(registry.is_blocked());

        registry.send(&TestNotice {
            what: "blocked 1".into(),
        });
        registry.send(&TestNotice {
            what: "blocked 2".into(),
        });
        // Hits must not change while blocked.
        assert_eq!(
            hits.load(Ordering::SeqCst),
            1,
            "delivery must be suppressed during block"
        );
    }

    // After block drops: delivery resumes.
    assert!(!registry.is_blocked());
    registry.send(&TestNotice {
        what: "after block".into(),
    });
    assert_eq!(hits.load(Ordering::SeqCst), 2);
}

// ============================================================
// Test: Nested blocks — only the outermost drop unblocks delivery.
// C++: _TestNoticeBlock uses a nested scope with two Block objects.
// ============================================================
#[test]
fn test_nested_notice_blocks() {
    let registry = NoticeRegistry::new();
    let hits = Arc::new(AtomicUsize::new(0));
    let h = hits.clone();

    let _key = registry.register_global::<TestNotice, _>(move |_| {
        h.fetch_add(1, Ordering::SeqCst);
    });

    {
        let _outer = registry.block();
        {
            let _inner = registry.block();
            registry.send(&TestNotice {
                what: "inner blocked".into(),
            });
            // Still blocked by both guards.
            assert_eq!(hits.load(Ordering::SeqCst), 0);
        }
        // Inner dropped, outer still active.
        registry.send(&TestNotice {
            what: "outer still blocked".into(),
        });
        assert_eq!(hits.load(Ordering::SeqCst), 0);
    }

    // Both guards dropped — delivery resumes.
    registry.send(&TestNotice {
        what: "unblocked".into(),
    });
    assert_eq!(hits.load(Ordering::SeqCst), 1);
}

// ============================================================
// Test: Listener deleted (dropped) — no delivery after drop.
// C++: delete l2; TestNotice("seventh").Send(wl2) → only #1 ProcessTestNotice.
//      delete l1; TestNotice("error!").Send() → nothing.
// ============================================================
#[test]
fn test_listener_revoked_on_drop() {
    let registry = NoticeRegistry::new();
    let count1 = Arc::new(AtomicUsize::new(0));
    let count2 = Arc::new(AtomicUsize::new(0));

    let c1 = count1.clone();
    let k1 = registry.register_global::<TestNotice, _>(move |_| {
        c1.fetch_add(1, Ordering::SeqCst);
    });

    let c2 = count2.clone();
    let k2 = registry.register_global::<TestNotice, _>(move |_| {
        c2.fetch_add(1, Ordering::SeqCst);
    });

    registry.send(&TestNotice {
        what: "both live".into(),
    });
    assert_eq!(count1.load(Ordering::SeqCst), 1);
    assert_eq!(count2.load(Ordering::SeqCst), 1);

    // Simulate "delete l2" by revoking k2.
    registry.revoke(k2);
    registry.send(&TestNotice {
        what: "l2 dead".into(),
    });
    assert_eq!(count1.load(Ordering::SeqCst), 2, "l1 must still fire");
    assert_eq!(
        count2.load(Ordering::SeqCst),
        1,
        "l2 must not fire after revoke"
    );

    // Simulate "delete l1".
    registry.revoke(k1);
    registry.send(&TestNotice {
        what: "all dead".into(),
    });
    assert_eq!(count1.load(Ordering::SeqCst), 2);
    assert_eq!(count2.load(Ordering::SeqCst), 1);
}

// ============================================================
// Test: RevokeAndWait — after return no in-flight callbacks reference the listener.
// C++: _TestRevokeAndWait stress test.
// ============================================================
#[test]
fn test_revoke_and_wait() {
    let registry = Arc::new(NoticeRegistry::new());
    let hits = Arc::new(AtomicUsize::new(0));
    let h = hits.clone();

    let key = registry.register_global::<TestNotice, _>(move |_| {
        h.fetch_add(1, Ordering::SeqCst);
    });

    registry.send(&TestNotice {
        what: "pre-revoke".into(),
    });
    assert_eq!(hits.load(Ordering::SeqCst), 1);

    // After RevokeAndWait returns, the listener must never fire again.
    assert!(registry.revoke_and_wait(key));

    registry.send(&TestNotice {
        what: "post-revoke".into(),
    });
    assert_eq!(
        hits.load(Ordering::SeqCst),
        1,
        "must not fire after revoke_and_wait"
    );
}

// ============================================================
// Test: Threaded notice delivery — worker and main thread listeners
// both receive their respective notices concurrently.
// C++: _TestThreadedNotices — MainListener + WorkListener across threads.
// ============================================================
#[test]
fn test_threaded_notice_delivery() {
    let registry = Arc::new(NoticeRegistry::new());

    let main_hits = Arc::new(AtomicUsize::new(0));
    let worker_hits = Arc::new(AtomicUsize::new(0));

    // Main listener: receives MainNotice globally.
    let mh = main_hits.clone();
    let _main_key = registry.register_global::<MainNotice, _>(move |_| {
        mh.fetch_add(1, Ordering::SeqCst);
    });

    // Worker thread sends WorkerNotice and receives it.
    let reg_worker = registry.clone();
    let wh = worker_hits.clone();
    let worker = std::thread::spawn(move || {
        let local_hits = Arc::new(AtomicUsize::new(0));
        let lh = local_hits.clone();
        let key = reg_worker.register_global::<WorkerNotice, _>(move |_| {
            lh.fetch_add(1, Ordering::SeqCst);
        });

        reg_worker.send(&WorkerNotice {
            what: "WorkerNotice 1".into(),
        });

        let count = local_hits.load(Ordering::SeqCst);
        wh.fetch_add(count, Ordering::SeqCst);

        reg_worker.revoke(key);

        // After revoke: sending again must not increment.
        reg_worker.send(&WorkerNotice {
            what: "WorkerNotice 2".into(),
        });
        let count_after = local_hits.load(Ordering::SeqCst);
        assert_eq!(count_after, 1, "worker listener must not fire after revoke");
    });

    // Main thread sends MainNotice while worker is running.
    registry.send(&MainNotice {
        what: "Main notice 1".into(),
    });

    worker.join().expect("worker thread panicked");

    // Worker received one WorkerNotice before revoking.
    assert_eq!(worker_hits.load(Ordering::SeqCst), 1);
    // Main received one MainNotice.
    assert_eq!(main_hits.load(Ordering::SeqCst), 1);
}

// ============================================================
// Test: Multiple listeners, send returns correct delivery count.
// ============================================================
#[test]
fn test_send_delivery_count() {
    let registry = NoticeRegistry::new();
    let _k1 = registry.register_global::<TestNotice, _>(|_| {});
    let _k2 = registry.register_global::<TestNotice, _>(|_| {});
    let _k3 = registry.register_global::<TestNotice, _>(|_| {});

    let delivered = registry.send(&TestNotice {
        what: "count test".into(),
    });
    assert_eq!(delivered, 3, "all three listeners must be counted");
}

// ============================================================
// Test: listener_count tracks registrations and revocations.
// ============================================================
#[test]
fn test_listener_count() {
    let registry = NoticeRegistry::new();
    assert_eq!(registry.listener_count::<TestNotice>(), 0);

    let k1 = registry.register_global::<TestNotice, _>(|_| {});
    assert_eq!(registry.listener_count::<TestNotice>(), 1);

    let sender = SenderId::new();
    let _k2 = registry.register_for_sender::<TestNotice, _>(sender, |_| {});
    assert_eq!(registry.listener_count::<TestNotice>(), 2);

    registry.revoke(k1);
    assert_eq!(registry.listener_count::<TestNotice>(), 1);
}

// ============================================================
// Test: Different notice types are independent.
// ============================================================
#[test]
fn test_different_notice_types_are_independent() {
    let registry = NoticeRegistry::new();
    let test_hits = Arc::new(AtomicUsize::new(0));
    let main_hits = Arc::new(AtomicUsize::new(0));

    let th = test_hits.clone();
    let _tk = registry.register_global::<TestNotice, _>(move |_| {
        th.fetch_add(1, Ordering::SeqCst);
    });

    let mh = main_hits.clone();
    let _mk = registry.register_global::<MainNotice, _>(move |_| {
        mh.fetch_add(1, Ordering::SeqCst);
    });

    registry.send(&TestNotice {
        what: "test".into(),
    });
    assert_eq!(test_hits.load(Ordering::SeqCst), 1);
    assert_eq!(main_hits.load(Ordering::SeqCst), 0);

    registry.send(&MainNotice {
        what: "main".into(),
    });
    assert_eq!(test_hits.load(Ordering::SeqCst), 1);
    assert_eq!(main_hits.load(Ordering::SeqCst), 1);
}

// ============================================================
// Test: Concurrent sends from multiple threads — no data races.
// C++: _TestThreadedNotices implicitly tests this with the thread
//      running WorkTask while main sends MainNotice.
// ============================================================
#[test]
fn test_concurrent_sends_no_races() {
    let registry = Arc::new(NoticeRegistry::new());
    let total = Arc::new(AtomicUsize::new(0));

    let t = total.clone();
    let _key = registry.register_global::<TestNotice, _>(move |_| {
        t.fetch_add(1, Ordering::SeqCst);
    });

    let mut handles = Vec::new();
    for _ in 0..4 {
        let reg = registry.clone();
        handles.push(std::thread::spawn(move || {
            for i in 0..25u32 {
                reg.send(&TestNotice {
                    what: format!("notice {i}"),
                });
            }
        }));
    }
    for h in handles {
        h.join().expect("thread panicked");
    }

    // 4 threads × 25 sends = 100 deliveries.
    assert_eq!(total.load(Ordering::SeqCst), 100);
}

// ============================================================
// Test: revoke_all clears a batch of keys at once.
// ============================================================
#[test]
fn test_revoke_all() {
    let registry = NoticeRegistry::new();
    let hits = Arc::new(AtomicUsize::new(0));
    let mut keys: Vec<ListenerKey> = Vec::new();

    for _ in 0..3 {
        let h = hits.clone();
        keys.push(registry.register_global::<TestNotice, _>(move |_| {
            h.fetch_add(1, Ordering::SeqCst);
        }));
    }

    registry.send(&TestNotice {
        what: "before revoke_all".into(),
    });
    assert_eq!(hits.load(Ordering::SeqCst), 3);

    registry.revoke_all(&mut keys);
    assert!(keys.is_empty(), "revoke_all must drain the vec");

    registry.send(&TestNotice {
        what: "after revoke_all".into(),
    });
    // Hits must not increase.
    assert_eq!(hits.load(Ordering::SeqCst), 3);
}

// ============================================================
// Test: send_from delivers to both global and per-sender listeners,
// but NOT to per-sender listeners registered for a different sender.
// Mirrors C++ spoofed-notices test intent (matching vs. non-matching sender).
// ============================================================
#[test]
fn test_send_from_selectivity() {
    let registry = NoticeRegistry::new();
    let sender_a = SenderId::new();
    let sender_b = SenderId::new();

    let global_hits = Arc::new(AtomicUsize::new(0));
    let a_hits = Arc::new(AtomicUsize::new(0));
    let b_hits = Arc::new(AtomicUsize::new(0));

    let gh = global_hits.clone();
    let _gk = registry.register_global::<TestNotice, _>(move |_| {
        gh.fetch_add(1, Ordering::SeqCst);
    });

    let ah = a_hits.clone();
    let _ak = registry.register_for_sender::<TestNotice, _>(sender_a, move |_| {
        ah.fetch_add(1, Ordering::SeqCst);
    });

    let bh = b_hits.clone();
    let _bk = registry.register_for_sender::<TestNotice, _>(sender_b, move |_| {
        bh.fetch_add(1, Ordering::SeqCst);
    });

    // Send from sender_a: global + a_listener
    registry.send_from(
        &TestNotice {
            what: "from a".into(),
        },
        sender_a,
    );
    assert_eq!(global_hits.load(Ordering::SeqCst), 1);
    assert_eq!(a_hits.load(Ordering::SeqCst), 1);
    assert_eq!(b_hits.load(Ordering::SeqCst), 0);

    // Send from sender_b: global + b_listener
    registry.send_from(
        &TestNotice {
            what: "from b".into(),
        },
        sender_b,
    );
    assert_eq!(global_hits.load(Ordering::SeqCst), 2);
    assert_eq!(a_hits.load(Ordering::SeqCst), 1);
    assert_eq!(b_hits.load(Ordering::SeqCst), 1);

    // Global send: only global listener
    registry.send(&TestNotice {
        what: "global".into(),
    });
    assert_eq!(global_hits.load(Ordering::SeqCst), 3);
    assert_eq!(a_hits.load(Ordering::SeqCst), 1);
    assert_eq!(b_hits.load(Ordering::SeqCst), 1);
}

// ============================================================
// Test: Notice payload is passed correctly to callbacks.
// ============================================================
#[test]
fn test_notice_payload_delivered() {
    let registry = NoticeRegistry::new();
    let received: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let rv = received.clone();

    let _key = registry.register_global::<TestNotice, _>(move |n: &TestNotice| {
        rv.lock().unwrap().push(n.what.clone());
    });

    registry.send(&TestNotice {
        what: "first".into(),
    });
    registry.send(&TestNotice {
        what: "second".into(),
    });

    let log = received.lock().unwrap();
    assert_eq!(log.as_slice(), &["first", "second"][..]);
}
