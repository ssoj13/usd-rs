# usd-tf -- Tools Foundation

Rust port of OpenUSD `pxr/base/tf`.

TF is the foundational layer of OpenUSD. It provides core infrastructure that every other USD module depends on:

- **Token** -- interned strings for attribute names, prim paths, and identifiers. O(1) equality comparison via pointer interning across 128 shards
- **TfType** -- runtime type registry with C3 MRO inheritance, factory pattern, and cross-module type discovery
- **Diagnostics** -- structured error/warning/status reporting with pluggable delegates and RAII error marks
- **Notice** -- observer pattern for decoupled event delivery with thread-local blocking
- **Smart pointers** -- intrusive ref-counted (`RefPtr`) and weak (`WeakPtr`) pointers with debug tracking
- **Registry** -- type-keyed function registry with subscribe/notify for plugin-style extensibility
- **String utilities** -- 40+ functions: printf-style formatting, tokenization, glob-to-regex, XML escaping, dictionary comparison
- **Environment** -- typed env settings with file overrides and non-default value warnings
- **Containers** -- dense hash maps/sets, small vectors, compressed bit arrays, span views
- **Synchronization** -- spin mutexes, sharded RW locks, scope guards
- **Debug** -- symbol-based conditional debug output with runtime enable/disable

Reference: `_ref/OpenUSD/pxr/base/tf`

## Parity Status

All public C++ APIs have Rust equivalents. Verified header-by-header against the reference.

Python bindings (`py*.h`, `wrap*.cpp`) excluded by design -- not applicable to Rust.

C++-only idioms replaced with Rust equivalents:
- `TF_BAD_SIZE_T` -> `usize::MAX`
- `TfAbs/TfMin/TfMax` -> `i32::abs()`, `std::cmp::min/max`
- `TfDeleter` -> `Drop`
- `TF_UNUSED(x)` -> `_` prefix
- `CastToAncestor/CastFromAncestor` -> trait objects (no `void*` casts)
- `TfVStringPrintf` (va_list) -> `format!` macro

---

## Module Map

### Core Types

| C++ Header | Rust Module | Notes |
|---|---|---|
| token.h | token.rs | Interned strings, 128-shard RwLock, O(1) equality via pointer compare |
| type.h / type_Impl.h | type_info.rs | Runtime type registry. C3 MRO for `get_all_ancestor_types`. `is_a_type::<T>()` generic shorthand. `get_factory_as::<T>()` typed factory |
| typeFunctions.h | type_functions.rs | `GetRawPtr`/`IsNull` traits for `Box`, `Arc`, `Rc`, `Option` |
| typeNotice.h | type_notice.rs | Type declaration/change notices |
| typeInfoMap.h | type_info_map.rs | `TypeId`-keyed concurrent map with aliases |

### Smart Pointers

| C++ Header | Rust Module | Notes |
|---|---|---|
| refBase.h | ref_base.rs | Intrusive ref-count base |
| refPtr.h | ref_ptr.rs | Intrusive reference-counted pointer |
| refPtrTracker.h | ref_ptr_tracker.rs | Debug tracking for ref-counted objects |
| weakBase.h | weak_base.rs | Weak reference base |
| weakPtr.h / weakPtrFacade.h | weak_ptr.rs | Weak pointer with facade merged |
| anyUniquePtr.h | any_unique_ptr.rs | Type-erased owning pointer |
| anyWeakPtr.h | any_weak_ptr.rs | Type-erased weak pointer |
| delegatedCountPtr.h | delegated_count_ptr.rs | External ref-count delegation |
| declarePtrs.h | N/A | Macro declarations -- not needed in Rust |

### Diagnostics

| C++ Header | Rust Module | Notes |
|---|---|---|
| diagnostic.h / diagnosticBase.h / diagnosticHelper.h | diagnostic.rs, diagnostic_base.rs | `tf_error!`, `tf_warn!`, `tf_coding_error!`, `tf_fatal_error!` macros |
| diagnosticMgr.h | diagnostic_mgr.rs | Full delegate system: `DiagnosticDelegate` trait with 4 callbacks (`issue_error`, `issue_fatal_error`, `issue_status`, `issue_warning`), add/remove delegate, multi-delegate dispatch, thread-safe via RwLock |
| diagnosticLite.h | diagnostic_lite.rs | Lightweight diagnostic helpers |
| error.h | error.rs | `TfError` type |
| warning.h | warning.rs | `TfWarning` type |
| status.h | status.rs | `TfStatus` type |
| errorMark.h | error_mark.rs | RAII error scope |
| errorTransport.h | error_transport.rs | Cross-thread error transfer |
| exception.h | exception.rs | Exception handling bridge |

### Debug System

| C++ Header | Rust Module | Notes |
|---|---|---|
| debug.h | debug.rs | `Debug::register`/`enable`/`disable`, `tf_debug_msg!` macro |
| debugCodes.h | debug_codes.rs | Standard debug symbol codes |
| debugNotice.h | debug_notice.rs | Notices for debug symbol changes |

### String & Path Utilities

| C++ Header | Rust Module | Notes |
|---|---|---|
| stringUtils.h | string_utils.rs | 40+ functions: `string_printf!`, `string_to_double`, `glob_to_regex`, `matched_string_tokenize` (with escape char), `quoted_string_tokenize`, `dictionary_less_than`, etc. |
| pathUtils.h | path_utils.rs | Path manipulation |
| unicodeUtils.h | unicode_utils.rs | XID_Start/XID_Continue via `unicode-xid` crate (UAX #31) |
| unicodeCharacterClasses.h | unicode_utils.rs | Merged into unicode_utils |
| patternMatcher.h | pattern_matcher.rs | Glob-style pattern matching |

### Environment

| C++ Header | Rust Module | Notes |
|---|---|---|
| getenv.h | getenv.rs | Thread-safe env access |
| setenv.h | setenv.rs | Thread-safe env modification |
| envSetting.h | env_setting.rs | `define_env_setting!` macro, file overrides via `PIXAR_TF_ENV_SETTING_FILE`, non-default value warnings |

### Containers

| C++ Header | Rust Module | Notes |
|---|---|---|
| hashmap.h | hashmap.rs | HashMap wrapper |
| hashset.h | hashset.rs | HashSet wrapper |
| denseHashMap.h | dense_hashmap.rs | Dense hash map |
| denseHashSet.h | dense_hashset.rs | Dense hash set |
| smallVector.h | small_vector.rs | Stack-allocated small buffer optimization |
| span.h | span.rs | Non-owning view |
| stl.h | stl.rs, stl_utils.rs | `map_lookup` -> `Option<&V>`, `map_lookup_by_value` |

### Bit Operations

| C++ Header | Rust Module | Notes |
|---|---|---|
| bits.h | bits.rs | Bit set operations |
| bitUtils.h | bit_utils.rs | Bit manipulation utilities |
| compressedBits.h | compressed_bits.rs | Run-length compressed bit arrays |
| pointerAndBits.h | pointer_and_bits.rs | Pointer with embedded flag bits |

### Synchronization

| C++ Header | Rust Module | Notes |
|---|---|---|
| spinMutex.h | spin_mutex.rs | Spin-lock mutex |
| spinRWMutex.h | spin_rw_mutex.rs | Spin-lock reader-writer mutex |
| bigRWMutex.h | big_rw_mutex.rs | Sharded reader-writer mutex |

### Compression

| C++ Header | Rust Module | Notes |
|---|---|---|
| fastCompression.h | fast_compression.rs | LZ4 via `lz4_flex` crate |

### File Operations

| C++ Header | Rust Module | Notes |
|---|---|---|
| fileUtils.h | file_utils.rs | File system utilities |
| atomicOfstreamWrapper.h | atomic_ofstream_wrapper.rs | Atomic file writes via temp + rename |
| atomicRenameUtil.h | atomic_rename.rs | Cross-platform atomic rename |
| safeOutputFile.h | safe_output_file.rs | Safe output with backup |

### Utilities

| C++ Header | Rust Module | Notes |
|---|---|---|
| singleton.h | singleton.rs | Thread-safe singleton |
| stopwatch.h | stopwatch.rs | High-resolution timer |
| scoped.h | scoped.rs | RAII scope guard |
| scopeDescription.h | scope_description.rs | Nested scope descriptions for diagnostics |
| stacked.h | stacked.rs | Thread-local stack |
| staticData.h | static_data.rs | Lazily initialized static data |
| staticTokens.h | static_tokens.rs | Compile-time token definitions |
| hash.h | hash.rs | Hash utilities |
| iterator.h | iterator.rs | Iterator adapters |
| meta.h | meta.rs | Metaprogramming helpers |
| callContext.h | call_context.rs | Source location capture (`file!`, `line!`, `column!`) |
| functionRef.h | function_ref.rs | Non-owning function reference |
| functionTraits.h | function_traits.rs | Function type introspection |
| dl.h | dl.rs | Dynamic library loading (`dlopen`/`dlclose`), `dlopen_is_active()`/`dlclose_is_active()` flags |
| enum.h | enum_type.rs | Runtime enum type with `is_a::<T>()` |
| registryManager.h | registry_manager.rs | Type registry with subscribe/register. All functions called on post-subscribe register |
| notice.h / noticeRegistry.h | notice.rs | Observer pattern with thread-local blocking (`NoticeBlock`) |
| mallocTag.h | malloc_tag.rs | Memory allocation tagging |
| stackTrace.h | stack_trace.rs | Stack trace capture |
| regTest.h | reg_test.rs | Test registration |
| expiryNotifier.h | expiry_notifier.rs | Expiry notification callbacks |
| scriptModuleLoader.h | script_module_loader.rs | Module loading orchestration |
| safeTypeCompare.h | safe_type_compare.rs | Cross-DSO type comparison |
| cxxCast.h | cxx_cast.rs | Safe type casting (replaces C++ `CastToAncestor`/`CastFromAncestor`) |
| nullPtr.h | null_ptr.rs | Null pointer sentinel |
| ostreamMethods.h | ostream_methods.rs | Stream output via `Display` trait |

---

## Not Ported (not needed in Rust)

| C++ File | Reason |
|---|---|
| api.h | `pub` visibility in Rust |
| tf.h / module.cpp | Module init macros (`TF_MAX_ARITY`, `TF_DEV_BUILD`) -- Rust uses `cfg` |
| pch.h | Precompiled headers |
| preprocessorUtilsLite.h | C++ preprocessor macros |
| instantiate*.h | C++ template instantiation |
| py*.h/cpp (30+ files) | Python bindings -- not applicable |
| makePyConstructor.h | Python constructors |
| wrap*.cpp | Python wrappers |
| pxrCLI11/* | CLI parser -- use `clap` crate |
| pxrDoubleConversion/* | Double conversion -- Rust stdlib |
| pxrLZ4/* | LZ4 -- use `lz4_flex` crate |
| pxrTslRobinMap/* | Robin hood hash map -- Rust stdlib `HashMap` |
| unicode/* | Unicode tables -- use `unicode-xid` crate |
| overview.dox / *Overview.dox | Doxygen docs |

---

## API Differences from C++

| C++ API | Rust Equivalent | Rationale |
|---|---|---|
| `TfType::Declare<T, Bases<...>>()` | `declare_with_bases::<T>(name, &[TypeId])` | Rust has no variadic templates |
| `TfType::Define<T, B>()` | `declare<T>` + `declare_with_bases` | Declaration and definition unified |
| `TfType::GetCanonicalTypeName(type_info)` | `canonical_type_name::<T>()` | Uses `std::any::type_name` |
| `base.GetAliases(derived)` | `get_aliases_for_derived(derived_type)` | Same semantics, different name |
| `TfMapLookup(map, key, &val)` | `map_lookup()` -> `Option<&V>` | Rust idiom: `Option` instead of bool + out-param |
| `TfToken::Set` | `BTreeSet<Token>` / `HashSet<Token>` | Standard Rust collections |
| `TfToTokenVector` / `TfToStringVector` | `.iter().map(Token::new).collect()` | Rust iterators |
| `TF_REGISTRY_FUNCTION(KEY)` | `registry_function!(KeyType, { ... })` | Different macro syntax |
| `TfRegistryManager::GetInstance()` | `RegistryManager::instance()` | Unit struct with static methods |
| `TfDlopen(file, flag, err, loadScriptBindings)` | `dlopen(file, flags)` -> `Result` | No script bindings (Python-only), `Result` for errors |

---

## Implementation Notes

### Token Internment
128-shard RwLock pool. O(1) equality via pointer comparison. Optional `Token::gc()` for reclaiming unused tokens (C++ does not have GC).

### TfType: C3 MRO
`get_all_ancestor_types()` uses true C3 linearization (`c3_linearize` + `c3_merge`), matching C++ `_MergeAncestors` in `type.cpp`.

### TfNotice: Thread-Local Blocking
`NoticeBlock` uses `thread_local! { Cell<u64> }` -- blocks only the current thread, matching C++ thread-local semantics.

### Unicode (XID_Start / XID_Continue)
Uses `unicode-xid` crate (UAX #31). Unicode version may differ from OpenUSD's embedded UCD snapshot.

### EnvSetting: File Overrides
`PIXAR_TF_ENV_SETTING_FILE` supports KEY=VALUE format. Priority: env var > file > default. Warns on non-default values (controlled by `TF_ENV_SETTING_ALERTS_ENABLED`).

Verified 2026-02-22.
