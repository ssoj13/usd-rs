# Work Module - Parallel Execution

Rust port of OpenUSD `pxr/base/work`. Multi-threaded task execution using Rayon backend.

## Parity Status: 100%

Every public C++ API has a Rust equivalent. Verified header-by-header against `_ref/OpenUSD/pxr/base/work/*.h` on 2026-02-08.

---

### Core Components

| C++ Header | Rust File | Status |
|---|---|---|
| dispatcher.h | dispatcher.rs | 100% — task dispatcher |
| loops.h | loops.rs | 100% — parallel for loops |
| reduce.h | reduce.rs | 100% — parallel reduction |
| sort.h | sort.rs | 100% — parallel sort |
| threadLimits.h | thread_limits.rs | 100% — thread pool config |

### Task Types

| C++ Header | Rust File | Status |
|---|---|---|
| detachedTask.h | detached_task.rs | 100% — fire-and-forget |
| singularTask.h | singular_task.rs | 100% — single-execution |
| taskGraph.h | task_graph.rs | 100% — dependency graph |

### Advanced Features

| C++ Header | Rust File | Status |
|---|---|---|
| withScopedParallelism.h | scoped_parallelism.rs | 100% |
| isolatingDispatcher.h | isolating_dispatcher.rs | 100% — isolated arena |
| utils.h | utils.rs | 100% |
| zeroAllocator.h | zero_allocator.rs | 100% |

### Backend

Uses **Rayon** for parallel execution (replaces C++ Intel TBB):
- Work-stealing thread pool
- Parallel iterators
- Scoped threads
- Join/spawn primitives

### Not Ported (not needed in Rust)

| C++ File | Reason |
|---|---|
| api.h | Rust `pub` visibility |
| module.cpp / pch.h | Module init |
| impl.h.in | Build-system template |
| taskGraph_defaultImpl.h | Default impl detail |
| workTBB/* (10 files) | TBB backend — replaced by Rayon |
| overview.dox | Doxygen docs |

---

## Summary

**Work module: 100% API parity with OpenUSD C++ reference.**

All 12 public C++ headers fully covered. 14 Rust source files. 0 API gaps.

Verified 2026-02-08.
