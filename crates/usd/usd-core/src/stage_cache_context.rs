//! UsdStageCacheContext - thread-local stacked context for stage cache binding.
//!
//! Port of pxr/usd/usd/stageCacheContext.h/cpp
//!
//! Provides RAII-based context that binds a StageCache to the current thread,
//! allowing UsdStage::Open() to find/insert stages in the cache.

use super::stage_cache::StageCache;
use std::cell::RefCell;

/// Block type for cache contexts.
///
/// Matches C++ `UsdStageCacheContextBlockType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StageCacheContextBlockType {
    /// Ignore all currently bound caches (no reads or writes).
    BlockStageCaches,
    /// Ignore all writable caches (reads still work, no writes).
    BlockStageCachePopulation,
    /// No blocking (default).
    NoBlock,
}

/// The internal binding stored in the thread-local stack.
#[derive(Debug)]
enum CacheBinding {
    /// Read-write cache binding.
    ReadWrite(*const StageCache),
    /// Read-only cache binding.
    ReadOnly(*const StageCache),
    /// Blocking context (no cache pointer needed).
    Block(StageCacheContextBlockType),
}

// SAFETY: CacheBinding contains raw pointers to StageCache, but is only stored in
// thread-local storage (CONTEXT_STACK). The pointers are never accessed across threads.
// StageCache is internally synchronized with RwLock, ensuring thread safety when accessed.
#[allow(unsafe_code)]
unsafe impl Send for CacheBinding {}

thread_local! {
    /// Thread-local stack of cache context bindings.
    static CONTEXT_STACK: RefCell<Vec<CacheBinding>> = RefCell::new(Vec::new());
}

/// A context object that binds a StageCache to the current scope.
///
/// Matches C++ `UsdStageCacheContext`.
///
/// When a `StageCacheContext` is in scope, stage open operations can find
/// stages in the bound cache or insert newly opened stages into it.
///
/// Contexts are stacked per-thread. The most recently created context is
/// examined first when looking for stages.
///
/// # Examples
///
/// ```
/// use usd_core::{StageCache, StageCacheContext, StageCacheContextBlockType};
///
/// let cache = StageCache::new();
///
/// // Bind cache for read+write within this scope
/// let _ctx = StageCacheContext::new(&cache);
///
/// // Within this scope, stage open would check `cache`
/// // When _ctx drops, the binding is removed
/// ```
pub struct StageCacheContext {
    _private: (), // prevents external construction
}

impl StageCacheContext {
    /// Bind a cache for read+write access.
    ///
    /// Matches C++ `UsdStageCacheContext(UsdStageCache &cache)`.
    pub fn new(cache: &StageCache) -> Self {
        CONTEXT_STACK.with(|stack| {
            stack
                .borrow_mut()
                .push(CacheBinding::ReadWrite(cache as *const StageCache));
        });
        Self { _private: () }
    }

    /// Bind a cache for read-only access.
    ///
    /// Matches C++ `UsdStageCacheContext(Usd_NonPopulatingStageCacheWrapper)`.
    pub fn read_only(cache: &StageCache) -> Self {
        CONTEXT_STACK.with(|stack| {
            stack
                .borrow_mut()
                .push(CacheBinding::ReadOnly(cache as *const StageCache));
        });
        Self { _private: () }
    }

    /// Create a blocking context that disables cache use.
    ///
    /// Matches C++ `UsdStageCacheContext(UsdStageCacheContextBlockType)`.
    pub fn blocking(block_type: StageCacheContextBlockType) -> Self {
        CONTEXT_STACK.with(|stack| {
            stack.borrow_mut().push(CacheBinding::Block(block_type));
        });
        Self { _private: () }
    }

    /// Returns readable caches from the context stack (both RO and RW).
    ///
    /// Walks the stack from most-recent to least-recent, stopping at
    /// a BlockStageCaches entry.
    ///
    /// Matches C++ `_GetReadableCaches()`.
    pub fn get_readable_caches() -> Vec<*const StageCache> {
        CONTEXT_STACK.with(|stack| {
            let stack = stack.borrow();
            let mut caches = Vec::new();
            for binding in stack.iter().rev() {
                match binding {
                    CacheBinding::Block(StageCacheContextBlockType::BlockStageCaches) => break,
                    CacheBinding::Block(StageCacheContextBlockType::BlockStageCachePopulation) => {
                        continue;
                    }
                    CacheBinding::Block(StageCacheContextBlockType::NoBlock) => continue,
                    CacheBinding::ReadWrite(ptr) => caches.push(*ptr),
                    CacheBinding::ReadOnly(ptr) => caches.push(*ptr),
                }
            }
            caches
        })
    }

    /// Returns writable caches from the context stack (RW only).
    ///
    /// Walks the stack from most-recent to least-recent, stopping at
    /// a BlockStageCaches or BlockStageCachePopulation entry.
    ///
    /// Matches C++ `_GetWritableCaches()`.
    pub fn get_writable_caches() -> Vec<*const StageCache> {
        CONTEXT_STACK.with(|stack| {
            let stack = stack.borrow();
            let mut caches = Vec::new();
            for binding in stack.iter().rev() {
                match binding {
                    CacheBinding::Block(StageCacheContextBlockType::BlockStageCaches)
                    | CacheBinding::Block(StageCacheContextBlockType::BlockStageCachePopulation) => {
                        break
                    }
                    CacheBinding::Block(StageCacheContextBlockType::NoBlock) => continue,
                    CacheBinding::ReadWrite(ptr) => caches.push(*ptr),
                    CacheBinding::ReadOnly(_) => {} // skip read-only
                }
            }
            caches
        })
    }

    /// Returns read-only caches from the context stack.
    ///
    /// Matches C++ `_GetReadOnlyCaches()`.
    pub fn get_read_only_caches() -> Vec<*const StageCache> {
        CONTEXT_STACK.with(|stack| {
            let stack = stack.borrow();
            let mut caches = Vec::new();
            for binding in stack.iter().rev() {
                match binding {
                    CacheBinding::Block(StageCacheContextBlockType::BlockStageCaches) => break,
                    CacheBinding::Block(StageCacheContextBlockType::BlockStageCachePopulation) => {
                        continue;
                    }
                    CacheBinding::Block(StageCacheContextBlockType::NoBlock) => continue,
                    CacheBinding::ReadOnly(ptr) => caches.push(*ptr),
                    CacheBinding::ReadWrite(_) => {} // skip read-write
                }
            }
            caches
        })
    }

    /// Returns the current stack depth (useful for testing).
    pub fn stack_depth() -> usize {
        CONTEXT_STACK.with(|stack| stack.borrow().len())
    }
}

impl Drop for StageCacheContext {
    fn drop(&mut self) {
        CONTEXT_STACK.with(|stack| {
            stack.borrow_mut().pop();
        });
    }
}

/// Helper to create a read-only cache binding.
///
/// Matches C++ `UsdUseButDoNotPopulateCache()`.
///
/// # Examples
///
/// ```
/// use usd_core::{StageCache, StageCacheContext, use_but_do_not_populate_cache};
///
/// let cache = StageCache::new();
/// let _ctx = use_but_do_not_populate_cache(&cache);
/// ```
pub fn use_but_do_not_populate_cache(cache: &StageCache) -> StageCacheContext {
    StageCacheContext::read_only(cache)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_context() {
        let cache = StageCache::new();
        assert_eq!(StageCacheContext::stack_depth(), 0);

        {
            let _ctx = StageCacheContext::new(&cache);
            assert_eq!(StageCacheContext::stack_depth(), 1);

            let readable = StageCacheContext::get_readable_caches();
            assert_eq!(readable.len(), 1);

            let writable = StageCacheContext::get_writable_caches();
            assert_eq!(writable.len(), 1);
        }

        assert_eq!(StageCacheContext::stack_depth(), 0);
    }

    #[test]
    fn test_read_only_context() {
        let cache = StageCache::new();

        {
            let _ctx = use_but_do_not_populate_cache(&cache);
            assert_eq!(StageCacheContext::stack_depth(), 1);

            // Readable should include read-only caches
            let readable = StageCacheContext::get_readable_caches();
            assert_eq!(readable.len(), 1);

            // Writable should not include read-only caches
            let writable = StageCacheContext::get_writable_caches();
            assert_eq!(writable.len(), 0);

            // Read-only list should have it
            let ro = StageCacheContext::get_read_only_caches();
            assert_eq!(ro.len(), 1);
        }

        assert_eq!(StageCacheContext::stack_depth(), 0);
    }

    #[test]
    fn test_nested_contexts() {
        let cache1 = StageCache::new();
        let cache2 = StageCache::new();

        {
            let _ctx1 = StageCacheContext::new(&cache1);
            {
                let _ctx2 = StageCacheContext::new(&cache2);
                assert_eq!(StageCacheContext::stack_depth(), 2);

                let readable = StageCacheContext::get_readable_caches();
                assert_eq!(readable.len(), 2);
            }
            assert_eq!(StageCacheContext::stack_depth(), 1);
        }
        assert_eq!(StageCacheContext::stack_depth(), 0);
    }

    #[test]
    fn test_block_all_caches() {
        let cache = StageCache::new();

        {
            let _ctx = StageCacheContext::new(&cache);
            {
                // Block all cache access
                let _block =
                    StageCacheContext::blocking(StageCacheContextBlockType::BlockStageCaches);
                assert_eq!(StageCacheContext::stack_depth(), 2);

                // Should find nothing - blocked
                let readable = StageCacheContext::get_readable_caches();
                assert_eq!(readable.len(), 0);

                let writable = StageCacheContext::get_writable_caches();
                assert_eq!(writable.len(), 0);
            }

            // After block dropped, cache is visible again
            let readable = StageCacheContext::get_readable_caches();
            assert_eq!(readable.len(), 1);
        }
    }

    #[test]
    fn test_block_population() {
        let cache = StageCache::new();

        {
            let _ctx = StageCacheContext::new(&cache);
            {
                // Block writes only
                let _block = StageCacheContext::blocking(
                    StageCacheContextBlockType::BlockStageCachePopulation,
                );

                // Readable should still find cache (reads pass through)
                // Note: BlockStageCachePopulation causes _GetReadableCaches to skip
                // the blocking entry via `continue`, so the outer RW cache is still found
                let readable = StageCacheContext::get_readable_caches();
                assert_eq!(readable.len(), 1);

                // But writable stops at the block
                let writable = StageCacheContext::get_writable_caches();
                assert_eq!(writable.len(), 0);
            }
        }
    }

    #[test]
    fn test_mixed_rw_ro_contexts() {
        let rw_cache = StageCache::new();
        let ro_cache = StageCache::new();

        {
            let _rw_ctx = StageCacheContext::new(&rw_cache);
            let _ro_ctx = use_but_do_not_populate_cache(&ro_cache);

            // Both should be readable
            let readable = StageCacheContext::get_readable_caches();
            assert_eq!(readable.len(), 2);

            // Only the RW cache should be writable
            let writable = StageCacheContext::get_writable_caches();
            assert_eq!(writable.len(), 1);

            // Only the RO cache should be in read-only list
            let ro = StageCacheContext::get_read_only_caches();
            assert_eq!(ro.len(), 1);
        }
    }
}
