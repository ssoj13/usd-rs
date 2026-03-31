//! Task graph for hierarchical structured parallelism.
//!
//! Mirrors C++ `WorkTaskGraph` / `WorkTaskGraph_DefaultImpl`.
//!
//! Tasks are heap-allocated (`Box::into_raw`) and freed by the executor
//! (mirroring C++ `delete this`). Each task embeds a [`TaskBase`] with:
//! - `task_graph`  -- back-pointer to the owning [`TaskGraph`]
//! - `parent`      -- erased fat-pointer to the parent/successor task
//! - `child_count` -- atomic ref-count of pending children
//! - `recycle`     -- set inside `execute()` for continuation-passing
//!
//! Completion (mirrors C++ `operator()`):
//! - If recycled: decrement `child_count`; if 0 re-execute self.
//! - If not recycled: drop task; if parent recycled and hits 0, re-execute parent.
//! - If `execute()` returned a continuation, run it inline (depth-bounded).

#![allow(unsafe_op_in_unsafe_fn, unsafe_code)]

use std::sync::atomic::{AtomicI32, Ordering};

use crate::Dispatcher;

const DEPTH_CUTOFF: i32 = 50;

// ---- Send-safe raw pointer wrappers ----------------------------------------

#[derive(Copy, Clone)]
struct GraphPtr(*mut TaskGraph);
unsafe impl Send for GraphPtr {}
unsafe impl Sync for GraphPtr {}

#[derive(Copy, Clone)]
struct TaskRawPtr(*mut dyn BaseTask);
unsafe impl Send for TaskRawPtr {}
unsafe impl Sync for TaskRawPtr {}

// ---- Fat pointer split/join -------------------------------------------------

#[repr(C)]
struct FatPtr {
    data: *mut (),
    vtbl: *const (),
}

unsafe fn encode_fat(p: *mut dyn BaseTask) -> (*mut (), *const ()) {
    let fp = std::mem::transmute::<*mut dyn BaseTask, FatPtr>(p);
    (fp.data, fp.vtbl)
}

unsafe fn decode_fat(data: *mut (), vtbl: *const ()) -> *mut dyn BaseTask {
    std::mem::transmute::<FatPtr, *mut dyn BaseTask>(FatPtr { data, vtbl })
}

// ---- TaskBase ---------------------------------------------------------------

/// Plumbing fields embedded in every task struct.
pub struct TaskBase {
    pub(crate) task_graph: *mut TaskGraph,
    pub(crate) parent_data: *mut (),
    pub(crate) parent_vtbl: *const (),
    pub(crate) child_count: AtomicI32,
    pub(crate) recycle: bool,
}

unsafe impl Send for TaskBase {}
unsafe impl Sync for TaskBase {}

impl TaskBase {
    pub fn new() -> Self {
        Self {
            task_graph: std::ptr::null_mut(),
            parent_data: std::ptr::null_mut(),
            parent_vtbl: std::ptr::null(),
            child_count: AtomicI32::new(0),
            recycle: false,
        }
    }

    /// Mirrors C++ `AddChildReference()`.
    #[inline]
    pub fn add_child_reference(&self) {
        self.child_count.fetch_add(1, Ordering::Acquire);
    }

    /// Mirrors C++ `RemoveChildReference()`. Returns the new count.
    #[inline]
    pub fn remove_child_reference(&self) -> i32 {
        self.child_count.fetch_sub(1, Ordering::Release) - 1
    }

    /// Call inside `execute()` to recycle as continuation.
    /// Mirrors C++ `_RecycleAsContinuation()`.
    #[inline]
    pub fn recycle_as_continuation(&mut self) {
        self.recycle = true;
    }
}

impl Default for TaskBase {
    fn default() -> Self {
        Self::new()
    }
}

// ---- RawTask ----------------------------------------------------------------

/// Send-safe, type-erased pointer to a heap-allocated [`BaseTask`].
#[derive(Copy, Clone)]
pub struct RawTask(TaskRawPtr);

impl RawTask {
    /// # Safety: ptr must be a valid heap-allocated `dyn BaseTask`.
    #[inline]
    pub unsafe fn new(ptr: *mut dyn BaseTask) -> Self {
        Self(TaskRawPtr(ptr))
    }
    #[inline]
    pub fn as_ptr(self) -> *mut dyn BaseTask {
        self.0.0
    }
    #[inline]
    pub fn is_null(self) -> bool {
        self.0.0.is_null()
    }
}

// ---- BaseTask trait ---------------------------------------------------------

/// Base trait for all task-graph tasks.
///
/// # Example
///
/// ```
/// use usd_work::task_graph::{BaseTask, RawTask, TaskBase, TaskGraph, FnTask};
/// use std::sync::atomic::{AtomicI32, Ordering};
/// use std::sync::Arc;
///
/// struct MyTask { base: TaskBase, counter: Arc<AtomicI32> }
/// impl BaseTask for MyTask {
///     fn base(&self)         -> &TaskBase        { &self.base }
///     fn base_mut(&mut self) -> &mut TaskBase    { &mut self.base }
///     fn execute(&mut self) -> Option<RawTask> {
///         self.counter.fetch_add(1, Ordering::SeqCst);
///         None
///     }
/// }
///
/// let counter = Arc::new(AtomicI32::new(0));
/// let graph = TaskGraph::new();
/// let c = counter.clone();
/// let task = graph.allocate_task(MyTask { base: TaskBase::new(), counter: c });
/// graph.run_task(task);
/// graph.wait();
/// assert_eq!(counter.load(Ordering::SeqCst), 1);
/// ```
pub trait BaseTask: Send {
    fn base(&self) -> &TaskBase;
    fn base_mut(&mut self) -> &mut TaskBase;
    fn execute(&mut self) -> Option<RawTask>;
}

// ---- TaskGraph --------------------------------------------------------------

/// Thread-local list of raw task pointers pending submission.
pub type TaskList = Vec<RawTask>;

/// Directed graph of parallel tasks with continuation passing and recycling.
///
/// # Examples
///
/// ```
/// use usd_work::task_graph::{TaskGraph, FnTask};
/// use std::sync::atomic::{AtomicI32, Ordering};
/// use std::sync::Arc;
///
/// let counter = Arc::new(AtomicI32::new(0));
/// let graph = TaskGraph::new();
/// let c = counter.clone();
/// let task = graph.allocate_task(FnTask::new(move || {
///     c.fetch_add(1, Ordering::SeqCst);
/// }));
/// graph.run_task(task);
/// graph.wait();
/// assert_eq!(counter.load(Ordering::SeqCst), 1);
/// ```
pub struct TaskGraph {
    dispatcher: Dispatcher,
}

impl TaskGraph {
    #[must_use]
    pub fn new() -> Self {
        Self {
            dispatcher: Dispatcher::new(),
        }
    }

    /// Allocate a task on the heap. Caller owns it until `run_task`.
    #[must_use]
    pub fn allocate_task<F: BaseTask + 'static>(&self, task: F) -> RawTask {
        let ptr: *mut dyn BaseTask = Box::into_raw(Box::new(task));
        unsafe { RawTask::new(ptr) }
    }

    /// Submit a task for concurrent execution. Transfers ownership.
    pub fn run_task(&self, task: RawTask) {
        let tptr = task.as_ptr();
        unsafe { (*tptr).base_mut().task_graph = self as *const Self as *mut Self };

        let tp = TaskRawPtr(tptr);
        let gp = GraphPtr(self as *const Self as *mut Self);
        self.dispatcher.run(move || {
            unsafe { invoke_task(tp, 0, gp) };
        });
    }

    /// Submit all tasks from thread-local lists.
    /// Mirrors C++ `WorkTaskGraph::RunLists()` which uses `WorkParallelForTBBRange`.
    pub fn run_lists(&self, task_lists: &[TaskList]) {
        crate::parallel_for_each(task_lists, |list| {
            for &task in list {
                self.run_task(task);
            }
        });
    }

    pub fn wait(&self) {
        self.dispatcher.wait();
    }
}

impl Default for TaskGraph {
    fn default() -> Self {
        Self::new()
    }
}

// ---- Executor internals -----------------------------------------------------

unsafe fn run_or_invoke(tp: TaskRawPtr, depth: i32, gp: GraphPtr) {
    if depth < DEPTH_CUTOFF {
        invoke_task(tp, depth + 1, gp);
    } else {
        (*gp.0).dispatcher.run(move || {
            invoke_task(tp, 0, gp);
        });
    }
}

unsafe fn invoke_task(tp: TaskRawPtr, depth: i32, gp: GraphPtr) {
    let task = tp.0;
    let graph = gp.0;

    (*task).base_mut().recycle = false;
    let next: Option<RawTask> = (*task).execute();

    if (*task).base().recycle {
        if (*task).base().remove_child_reference() == 0 {
            run_or_invoke(TaskRawPtr(task), depth, GraphPtr(graph));
        }
    } else {
        let (par_data, par_vtbl) = {
            let b = (*task).base();
            (b.parent_data, b.parent_vtbl)
        };

        drop(Box::from_raw(task));

        if !par_data.is_null() {
            let parent = decode_fat(par_data, par_vtbl);
            if (*parent).base().remove_child_reference() == 0 && (*parent).base().recycle {
                run_or_invoke(TaskRawPtr(parent), depth, GraphPtr(graph));
            }
        }
    }

    if let Some(next_raw) = next {
        if !next_raw.is_null() {
            run_or_invoke(next_raw.0, depth, GraphPtr(graph));
        }
    }
}

// ---- Child allocation -------------------------------------------------------

/// Allocate a child, link to parent, increment parent ref-count.
/// Mirrors C++ `BaseTask::AllocateChild<F>(args...)`.
///
/// # Safety: parent must outlive the child.
pub unsafe fn allocate_child<F: BaseTask + 'static>(
    parent: *mut dyn BaseTask,
    child: F,
) -> RawTask {
    (*parent).base().add_child_reference();
    let child_raw: *mut dyn BaseTask = Box::into_raw(Box::new(child));
    let (pd, pv) = encode_fat(parent);
    (*child_raw).base_mut().parent_data = pd;
    (*child_raw).base_mut().parent_vtbl = pv;
    RawTask::new(child_raw)
}

// ---- Built-in task types ----------------------------------------------------

/// Task backed by a `FnMut` closure that may return a continuation.
pub struct SimpleTask {
    pub base: TaskBase,
    func: Option<Box<dyn FnMut() -> Option<RawTask> + Send>>,
}

impl SimpleTask {
    pub fn new<F: FnMut() -> Option<RawTask> + Send + 'static>(f: F) -> Self {
        Self {
            base: TaskBase::new(),
            func: Some(Box::new(f)),
        }
    }
}

impl BaseTask for SimpleTask {
    fn base(&self) -> &TaskBase {
        &self.base
    }
    fn base_mut(&mut self) -> &mut TaskBase {
        &mut self.base
    }
    fn execute(&mut self) -> Option<RawTask> {
        self.func.as_mut().and_then(|f| f())
    }
}

/// Fire-once `FnOnce` task with no continuation.
pub struct FnTask {
    pub base: TaskBase,
    func: Option<Box<dyn FnOnce() + Send>>,
}

impl FnTask {
    pub fn new<F: FnOnce() + Send + 'static>(f: F) -> Self {
        Self {
            base: TaskBase::new(),
            func: Some(Box::new(f)),
        }
    }
}

impl BaseTask for FnTask {
    fn base(&self) -> &TaskBase {
        &self.base
    }
    fn base_mut(&mut self) -> &mut TaskBase {
        &mut self.base
    }
    fn execute(&mut self) -> Option<RawTask> {
        if let Some(f) = self.func.take() {
            f();
        }
        None
    }
}

/// Chains a `FnOnce` with an optional continuation `RawTask`.
pub struct ChainedTask {
    pub base: TaskBase,
    first: Option<Box<dyn FnOnce() -> Option<RawTask> + Send>>,
    then: Option<RawTask>,
}

impl ChainedTask {
    pub fn new<F: FnOnce() -> Option<RawTask> + Send + 'static>(
        first: F,
        then: Option<RawTask>,
    ) -> Self {
        Self {
            base: TaskBase::new(),
            first: Some(Box::new(first)),
            then,
        }
    }
}

impl BaseTask for ChainedTask {
    fn base(&self) -> &TaskBase {
        &self.base
    }
    fn base_mut(&mut self) -> &mut TaskBase {
        &mut self.base
    }
    fn execute(&mut self) -> Option<RawTask> {
        let cont = self.first.take().and_then(|f| f());
        cont.or(self.then.take())
    }
}

// ---- Compat: RefCountedTask -------------------------------------------------

/// Standalone ref-count wrapper. API compat; not used by the executor.
pub struct RefCountedTask<T> {
    pub inner: T,
    child_refs: AtomicI32,
}

impl<T: Send> RefCountedTask<T> {
    pub fn new(task: T) -> Self {
        Self {
            inner: task,
            child_refs: AtomicI32::new(0),
        }
    }
    pub fn add_child_reference(&self) {
        self.child_refs.fetch_add(1, Ordering::Acquire);
    }
    pub fn remove_child_reference(&self) -> i32 {
        self.child_refs.fetch_sub(1, Ordering::Release) - 1
    }
    pub fn child_count(&self) -> i32 {
        self.child_refs.load(Ordering::Relaxed)
    }
}

// ---- Tests ------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicI32, AtomicUsize, Ordering},
    };

    struct CounterTask {
        base: TaskBase,
        counter: Arc<AtomicI32>,
        delta: i32,
    }
    impl CounterTask {
        fn new(counter: Arc<AtomicI32>, delta: i32) -> Self {
            Self {
                base: TaskBase::new(),
                counter,
                delta,
            }
        }
    }
    impl BaseTask for CounterTask {
        fn base(&self) -> &TaskBase {
            &self.base
        }
        fn base_mut(&mut self) -> &mut TaskBase {
            &mut self.base
        }
        fn execute(&mut self) -> Option<RawTask> {
            self.counter.fetch_add(self.delta, Ordering::SeqCst);
            None
        }
    }

    #[test]
    fn test_task_graph_new() {
        let _g = TaskGraph::new();
    }

    #[test]
    fn test_single_counter_task() {
        let counter = Arc::new(AtomicI32::new(0));
        let graph = TaskGraph::new();
        let task = graph.allocate_task(CounterTask::new(counter.clone(), 42));
        graph.run_task(task);
        graph.wait();
        assert_eq!(counter.load(Ordering::SeqCst), 42);
    }

    #[test]
    fn test_multiple_tasks() {
        let counter = Arc::new(AtomicI32::new(0));
        let graph = TaskGraph::new();
        for i in 1..=10 {
            graph.run_task(graph.allocate_task(CounterTask::new(counter.clone(), i)));
        }
        graph.wait();
        assert_eq!(counter.load(Ordering::SeqCst), 55);
    }

    #[test]
    fn test_fn_task() {
        let counter = Arc::new(AtomicI32::new(0));
        let graph = TaskGraph::new();
        let c = counter.clone();
        graph.run_task(graph.allocate_task(FnTask::new(move || {
            c.fetch_add(100, Ordering::SeqCst);
        })));
        graph.wait();
        assert_eq!(counter.load(Ordering::SeqCst), 100);
    }

    #[test]
    fn test_continuation() {
        let counter = Arc::new(AtomicI32::new(0));
        let graph = TaskGraph::new();
        let c2 = counter.clone();
        let cont = graph.allocate_task(SimpleTask::new(move || {
            c2.fetch_add(1, Ordering::SeqCst);
            None
        }));
        let c1 = counter.clone();
        let task = graph.allocate_task(SimpleTask::new(move || {
            c1.fetch_add(1, Ordering::SeqCst);
            Some(cont)
        }));
        graph.run_task(task);
        graph.wait();
        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn test_deep_chain_no_stack_overflow() {
        let counter = Arc::new(AtomicI32::new(0));
        let graph = TaskGraph::new();
        const N: i32 = 200;
        fn make_chain(graph: &TaskGraph, counter: Arc<AtomicI32>, remaining: i32) -> RawTask {
            let c = counter.clone();
            if remaining == 0 {
                graph.allocate_task(SimpleTask::new(move || {
                    c.fetch_add(1, Ordering::SeqCst);
                    None
                }))
            } else {
                let next = make_chain(graph, counter.clone(), remaining - 1);
                graph.allocate_task(SimpleTask::new(move || {
                    c.fetch_add(1, Ordering::SeqCst);
                    Some(next)
                }))
            }
        }
        let head = make_chain(&graph, counter.clone(), N - 1);
        graph.run_task(head);
        graph.wait();
        assert_eq!(counter.load(Ordering::SeqCst), N);
    }

    #[test]
    fn test_parallel_100_tasks() {
        let counter = Arc::new(AtomicI32::new(0));
        let graph = TaskGraph::new();
        for _ in 0..100 {
            let c = counter.clone();
            graph.run_task(graph.allocate_task(FnTask::new(move || {
                c.fetch_add(1, Ordering::Relaxed);
            })));
        }
        graph.wait();
        assert_eq!(counter.load(Ordering::SeqCst), 100);
    }

    #[test]
    fn test_empty_wait() {
        let graph = TaskGraph::new();
        graph.wait();
    }

    #[test]
    fn test_accumulate_order_independent() {
        let seen = Arc::new(Mutex::new(Vec::<i32>::new()));
        let graph = TaskGraph::new();
        for i in 0..20 {
            let s = seen.clone();
            graph.run_task(graph.allocate_task(FnTask::new(move || {
                s.lock().unwrap().push(i);
            })));
        }
        graph.wait();
        let mut v = seen.lock().unwrap().clone();
        v.sort();
        assert_eq!(v, (0..20).collect::<Vec<_>>());
    }

    #[test]
    fn test_task_base_child_refs() {
        let base = TaskBase::new();
        assert_eq!(base.child_count.load(Ordering::Relaxed), 0);
        base.add_child_reference();
        base.add_child_reference();
        assert_eq!(base.child_count.load(Ordering::Relaxed), 2);
        assert_eq!(base.remove_child_reference(), 1);
        assert_eq!(base.remove_child_reference(), 0);
    }

    #[test]
    fn test_ref_counted_task() {
        let rc = RefCountedTask::new(42i32);
        assert_eq!(rc.child_count(), 0);
        rc.add_child_reference();
        rc.add_child_reference();
        assert_eq!(rc.child_count(), 2);
        assert_eq!(rc.remove_child_reference(), 1);
    }

    #[test]
    fn test_default() {
        let _g = TaskGraph::default();
    }

    #[test]
    fn test_large_parallel_sum() {
        let counter = Arc::new(AtomicUsize::new(0));
        let graph = TaskGraph::new();
        const N: usize = 1000;
        for _ in 0..N {
            let c = counter.clone();
            graph.run_task(graph.allocate_task(FnTask::new(move || {
                c.fetch_add(1, Ordering::Relaxed);
            })));
        }
        graph.wait();
        assert_eq!(counter.load(Ordering::SeqCst), N);
    }

    #[test]
    fn test_run_lists() {
        let counter = Arc::new(AtomicI32::new(0));
        let graph = TaskGraph::new();
        let mut lists: Vec<TaskList> = vec![vec![], vec![]];
        for i in 0..5 {
            let c = counter.clone();
            lists[0].push(graph.allocate_task(FnTask::new(move || {
                c.fetch_add(i, Ordering::Relaxed);
            })));
        }
        for i in 5..10 {
            let c = counter.clone();
            lists[1].push(graph.allocate_task(FnTask::new(move || {
                c.fetch_add(i, Ordering::Relaxed);
            })));
        }
        graph.run_lists(&lists);
        graph.wait();
        assert_eq!(counter.load(Ordering::SeqCst), 45); // 0+1+...+9
    }

    struct FanParent {
        pub base: TaskBase,
        counter: Arc<AtomicI32>,
        n_children: i32,
        // Prevent re-spawning when re-invoked as continuation after children complete.
        spawned: bool,
    }
    struct FanChild {
        pub base: TaskBase,
        counter: Arc<AtomicI32>,
    }

    impl BaseTask for FanChild {
        fn base(&self) -> &TaskBase {
            &self.base
        }
        fn base_mut(&mut self) -> &mut TaskBase {
            &mut self.base
        }
        fn execute(&mut self) -> Option<RawTask> {
            self.counter.fetch_add(1, Ordering::SeqCst);
            None
        }
    }

    impl BaseTask for FanParent {
        fn base(&self) -> &TaskBase {
            &self.base
        }
        fn base_mut(&mut self) -> &mut TaskBase {
            &mut self.base
        }
        fn execute(&mut self) -> Option<RawTask> {
            if self.spawned {
                // Second invocation (continuation after all children done): just finish.
                return None;
            }
            self.spawned = true;
            let graph_ptr = self.base.task_graph;
            let this_ptr: *mut dyn BaseTask = self as *mut FanParent;

            // Safe-continuation extra ref + mark for recycling.
            self.base.add_child_reference();
            self.base.recycle = true;

            for _ in 0..self.n_children {
                let child_raw = unsafe {
                    allocate_child(
                        this_ptr,
                        FanChild {
                            base: TaskBase::new(),
                            counter: self.counter.clone(),
                        },
                    )
                };
                unsafe { (*graph_ptr).run_task(child_raw) };
            }
            None
        }
    }

    #[test]
    fn test_fan_out_children() {
        let counter = Arc::new(AtomicI32::new(0));
        let graph = TaskGraph::new();
        let task = graph.allocate_task(FanParent {
            base: TaskBase::new(),
            counter: counter.clone(),
            n_children: 10,
            spawned: false,
        });
        graph.run_task(task);
        graph.wait();
        assert_eq!(counter.load(Ordering::SeqCst), 10);
    }

    #[test]
    fn test_chained_task() {
        let counter = Arc::new(AtomicI32::new(0));
        let graph = TaskGraph::new();
        let c2 = counter.clone();
        let then_task = graph.allocate_task(FnTask::new(move || {
            c2.fetch_add(20, Ordering::SeqCst);
        }));
        let c1 = counter.clone();
        let task = graph.allocate_task(ChainedTask::new(
            move || {
                c1.fetch_add(10, Ordering::SeqCst);
                None
            },
            Some(then_task),
        ));
        graph.run_task(task);
        graph.wait();
        assert_eq!(counter.load(Ordering::SeqCst), 30);
    }

    #[test]
    fn test_multiple_waits() {
        let counter = Arc::new(AtomicI32::new(0));
        let graph = TaskGraph::new();
        for _ in 0..5 {
            let c = counter.clone();
            graph.run_task(graph.allocate_task(FnTask::new(move || {
                c.fetch_add(1, Ordering::Relaxed);
            })));
            graph.wait();
        }
        assert_eq!(counter.load(Ordering::SeqCst), 5);
    }
}
