# usd-arch -- Architecture Abstraction

Rust port of OpenUSD `pxr/base/arch`.

Arch is the lowest-level layer of OpenUSD. It abstracts away platform differences so all higher modules work identically on Linux, macOS, and Windows:

- **Memory** -- aligned allocation (`aligned_alloc`/`aligned_free`), `AlignedBox` RAII wrapper, cache line size constants, virtual memory mapping with protection flags
- **File system** -- cross-platform file ops: memory-mapped I/O (`memmap2`), positional read/write, temp files, path normalization, `get_file_name` from fd (Linux `/proc`, macOS `fcntl`, Windows `GetFinalPathNameByHandle`)
- **Timing** -- high-resolution tick counter with platform-specific backends (RDTSC/`clock_gettime`/QPC), `IntervalTimer`, `Stopwatch`, tick quantum measurement, ns/tick conversion
- **Threads** -- main thread detection, thread naming, concurrency queries, `spin_pause()` (x86 PAUSE / ARM YIELD), critical section markers
- **Hashing** -- SpookyHash V2 (32/64/128-bit) faithful port
- **Debugger** -- attach detection (`/proc`/`sysctl`/Windows API), trap, wait, `arch_debugger_attach()` with `ARCH_DEBUGGER` env var, `arch_abort(logging)`
- **Stack traces** -- backtrace capture, callback registration, session logging, external command execution for crash reporting
- **Dynamic libraries** -- `libloading`-based open/close/symbol with platform suffix constants
- **Environment** -- get/set/unset/has, `expand_environment_variables()` (`$VAR`, `${VAR}`, `%VAR%`), `environ()` iterator
- **Regex** -- `ArchRegex` wrapper with glob-to-regex conversion
- **Daemon** -- process daemonization (Unix), `close_all_files`, `DaemonOptions` builder
- **System info** -- hostname, username, PID, executable path, page size, physical/available memory
- **Error** -- fatal/warning macros with backtrace integration
- **Math** -- bit manipulation, sin/cos, sign, byte swap, rotate, popcount, leading/trailing zeros

Reference: `_ref/OpenUSD/pxr/base/arch`

## Parity Status

All public C++ APIs have Rust equivalents. Verified header-by-header against the reference.

---

## Header-by-Header Parity

### align.h → align.rs ✓

| C++ API | Rust | Notes |
|---------|------|-------|
| ArchAlignMemorySize | align_memory_size | ✓ |
| ARCH_MAX_ALIGNMENT_INCREASE | MAX_ALIGNMENT_INCREASE | ✓ |
| ArchAlignMemory | align_memory | ✓ |
| ARCH_CACHE_LINE_SIZE | CACHE_LINE_SIZE (defines.rs) | ✓ |
| ArchAlignedAlloc | aligned_alloc | Uses posix_memalign (Unix) / _aligned_malloc (Windows) |
| ArchAlignedFree | aligned_free | Uses free (Unix) / _aligned_free (Windows) |
| AlignedBox | AlignedBox | Extra: safe RAII wrapper using std::alloc |

### attributes.h → attributes.rs ✓

| C++ API | Rust | Notes |
|---------|------|-------|
| ARCH_PRINTF_FUNCTION etc. | *_NOTE constants | Compiler attributes as doc strings |
| ARCH_CONSTRUCTOR | CONSTRUCTOR_NOTE | ✓ |
| ARCH_EMPTY_BASES | EMPTY_BASES_NOTE | ✓ |
| ARCH_FALLTHROUGH | FALLTHROUGH_NOTE | ✓ |
| ARCH_NO_SANITIZE_ADDRESS | NO_SANITIZE_ADDRESS_NOTE | ✓ |

### buildMode.h → build_mode.rs ✓

| C++ API | Rust | Notes |
|---------|------|-------|
| ArchBuildMode | ArchBuildMode enum | ✓ |
| ArchGetBuildMode | arch_build_mode | ✓ |
| ArchIsDebugBuild | arch_is_debug_build | ✓ |
| ArchIsDevBuild | arch_is_dev_build | ✓ |
| ARCH_DEV_BUILD | ARCH_DEV_BUILD | ✓ |

### daemon.h → daemon.rs ✓

| C++ API | Rust | Notes |
|---------|------|-------|
| ArchCloseAllFiles | close_all_files | ✓ Linux/macOS; Windows returns Unsupported |
| — | daemonize, DaemonOptions | Extended API: full daemonization (C++ only has close) |

### debugger.h → debugger.rs ✓

| C++ API | Rust | Notes |
|---------|------|-------|
| ArchDebuggerIsAttached | arch_debugger_is_attached | ✓ Platform-specific: /proc (Linux), sysctl (macOS), CheckRemoteDebuggerPresent (Windows) |
| ArchDebuggerTrap | arch_debugger_trap | ✓ int 3 / SIGTRAP |
| ArchDebuggerWait | arch_debugger_wait | ✓ Atomic flag spin |
| ArchDebuggerAttach | arch_debugger_attach | ✓ Reads ARCH_DEBUGGER env, double-fork exec on POSIX, DebugBreak on Windows |
| ArchAbort | arch_abort(logging) | ✓ logging=true captures backtrace, resets SIGABRT handler, then abort() |

### defines.h → defines.rs ✓

| C++ Define | Rust | Notes |
|------------|------|-------|
| ARCH_OS_LINUX/DARWIN/WINDOWS | os::IS_LINUX etc. | ✓ |
| ARCH_OS_IPHONE/OSX | os::IS_IOS, IS_MACOS | ✓ |
| ARCH_OS_WASM_VM | os::IS_WASM | ✓ |
| ARCH_CPU_INTEL/ARM | cpu::IS_INTEL, IS_ARM | ✓ |
| ARCH_BITS_32/64 | cpu::IS_32_BIT, IS_64_BIT | ✓ |
| ARCH_CACHE_LINE_SIZE | CACHE_LINE_SIZE | ✓ |
| ARCH_HAS_MMAP_MAP_POPULATE | features::HAS_MMAP_MAP_POPULATE | ✓ Linux |
| ARCH_SANITIZE_ADDRESS | features::SANITIZE_ADDRESS | ✓ cfg(sanitize=address) |
| ARCH_COMPILER_* | compiler::IS_RUSTC | Rust uses rustc |

### demangle.h → demangle.rs ✓

| C++ API | Rust | Notes |
|---------|------|-------|
| ArchDemangle | arch_demangle | ✓ C++/Rust mangled names |
| ArchGetDemangled | arch_get_demangled | ✓ |
| ArchGetDemangledTypeName | arch_get_demangled_type_name | ✓ |
| ArchGetPrettyTypeName | arch_get_pretty_type_name | ✓ |
| ArchDemangleFunctionName | arch_demangle_function_name | ✓ |

### env.h → env.rs ✓

| C++ API | Rust | Notes |
|---------|------|-------|
| ArchHasEnv | has_env | ✓ |
| ArchGetEnv | get_env | Returns Option; C++ returns "" |
| ArchSetEnv(name, val, overwrite) | set_env_with_overwrite | ✓ |
| ArchSetEnv (overwrite=true) | set_env | ✓ |
| ArchRemoveEnv | unset_env | ✓ Returns bool |
| ArchExpandEnvironmentVariables | expand_env_vars | ✓ $VAR, ${VAR}, %VAR% (Win) |
| ArchEnviron | env_vars | Iterator instead of char** |
| — | get_env_or, get_env_bool, get_env_int, get_env_uint, get_env_float | Extensions |

### errno.h → errno.rs ✓

| C++ API | Rust | Notes |
|---------|------|-------|
| ArchGetLastError | get_last_error | ✓ |
| ArchStrerror | strerror | ✓ |
| ArchGetLastErrorString | last_error_string | ✓ |
| Error codes | errno::codes | ✓ |

### error.h → error.rs ✓

| C++ API | Rust | Notes |
|---------|------|-------|
| ArchSetFatalErrorLogging | arch_set_fatal_error_logging | ✓ |
| ArchIsFatalErrorLoggingEnabled | arch_is_fatal_error_logging_enabled | ✓ |
| ARCH_ERROR | arch_error_impl | ✓ |
| ARCH_WARNING | arch_warning_impl | ✓ |
| ArchLogFatalError | arch_log_fatal_error | ✓ |
| ArchLogCurrentProcessState | arch_log_current_process_state | ✓ |

### fileSystem.h → file_system.rs ✓

| C++ API | Rust | Notes |
|---------|------|-------|
| PATH_SEP, PATH_LIST_SEP etc. | Constants | ✓ |
| ArchConstFileMapping | ConstFileMapping | ✓ |
| ArchMutableFileMapping | MutableFileMapping | ✓ |
| ArchOpenFile | open_file | ✓ |
| ArchGetFileName | get_file_name | ✓ Linux: /proc/self/fd, macOS: fcntl F_GETPATH, Windows: GetFinalPathNameByHandle |
| ArchPread/Pwrite | pread, pwrite | ✓ |
| ArchMapFileReadOnly | map_file_ro | ✓ |
| ArchMapFileReadWrite | map_file_rw | ✓ |
| ArchNormPath | norm_path | ✓ |
| ArchAbsPath | abs_path | ✓ |
| ArchGetTmpDir | get_arch_tmp_dir | ✓ |
| ArchMakeTmpFile | make_tmp_file | ✓ |
| ArchFileAdvise | file_advise | ✓ |
| ArchMemAdvise | mem_advise | ✓ |
| ArchQueryMappedMemoryResidency | query_mapped_memory_residency | ✓ |
| FileAdvice, MemAdvice | Enums | ✓ |

### function.h → function.rs ✓

| C++ API | Rust | Notes |
|---------|------|-------|
| ArchGetPrettierFunctionName | arch_get_prettier_function_name | ✓ Adapted for Rust type_name |

### functionLite.h → function_lite.rs ✓

| C++ Macro | Rust | Notes |
|-----------|------|-------|
| __ARCH_FUNCTION__ | arch_function!() | ✓ |
| __ARCH_PRETTY_FUNCTION__ | arch_pretty_function!() | ✓ |
| __ARCH_FILE__ | arch_file!() | ✓ |

### hash.h → hash.rs ✓

| C++ API | Rust | Notes |
|---------|------|-------|
| ArchHash64 | hash64, hash64_with_seed | ✓ |
| ArchHash32 | hash32, hash32_with_seed | ✓ |
| ArchHash128 | hash128 | ✓ |
| ArchSpookyHash | SpookyHasher | ✓ |

### hints.h → hints.rs ✓

| C++ API | Rust | Notes |
|---------|------|-------|
| ArchLikely | likely | ✓ |
| ArchUnlikely | unlikely | ✓ |
| ArchCold | cold | ✓ |
| ArchPrefetch | prefetch | ✓ |

### initConfig.cpp → init_config.rs ✓

| C++ Function | Rust | Notes |
|--------------|------|-------|
| Arch_SetAppLaunchTime | arch_set_app_launch_time | ✓ |
| Arch_InitTmpDir | arch_init_tmp_dir | ✓ |
| ArchSetProgramNameForErrors | arch_set_program_name_for_errors | ✓ |
| ArchGetProgramNameForErrors | arch_get_program_name_for_errors | ✓ |
| Arch_ValidateAssumptions | arch_validate_assumptions | ✓ |
| Arch_InitDebuggerAttach | arch_init_debugger_attach | ✓ |
| Arch_InitTickTimer | arch_init_tick_timer | ✓ |
| ARCH_CONSTRUCTOR(Arch_InitConfig) | arch_init_config | Manual call |
| — | arch_is_initialized | Extra |

### inttypes.h → inttypes.rs ✓

| C++ Define | Rust | Notes |
|------------|------|-------|
| INT8_MAX, INT8_MIN etc. | Constants | From Rust stdlib |

### library.h → library.rs ✓

| C++ API | Rust | Notes |
|---------|------|-------|
| ArchLibraryOpen | Library::open | ✓ |
| ArchLibraryClose | Library::close | ✓ |
| ArchLibrarySymbol | Library::symbol | ✓ |
| ArchLibraryError | library_error | ✓ |
| LIBRARY_*, PLUGIN_SUFFIX | Constants | ✓ |
| ArchMakeLibraryName | make_library_name | ✓ |

### mallocHook.h → malloc_hook.rs ✓

| C++ API | Rust | Notes |
|---------|------|-------|
| ArchSetMallocHook | set_malloc_hook | ✓ |
| ArchIsMallocHookEnabled | is_malloc_hook_enabled | ✓ |
| ArchIsPtmallocActive | is_ptmalloc_active | ✓ |
| ArchIsJemallocActive | is_jemalloc_active | ✓ |
| ArchIsStlAllocatorOff | is_stl_allocator_off | ✓ |
| InstrumentedAllocator | InstrumentedAllocator | ✓ |

### math.h → math.rs ✓

| C++ API | Rust | Notes |
|---------|------|-------|
| ARCH_MIN_FLOAT_EPS_SQR | MIN_FLOAT_EPS_SQR | ✓ |
| ARCH_PI | PI | ✓ |
| ArchFloatToBitPattern | float_to_bit_pattern | ✓ |
| ArchBitPatternToFloat | bit_pattern_to_float | ✓ |
| ArchDoubleToBitPattern | double_to_bit_pattern | ✓ |
| ArchBitPatternToDouble | bit_pattern_to_double | ✓ |
| ArchSinCos | sin_cos, sin_cos_f32 | ✓ |
| ArchSign | sign, signum | ✓ |
| ArchCountTrailingZeros | count_trailing_zeros(_64) | ✓ |
| ArchCountLeadingZeros | count_leading_zeros(_64) | ✓ |
| ArchPopcount | popcount, popcount_64 | ✓ |
| ArchNextPowerOfTwo | next_power_of_two(_64) | ✓ |
| ArchIsPowerOfTwo | is_power_of_two(_64) | ✓ |
| ArchLog2Floor | log2_floor(_64) | ✓ |
| ArchLog2Ceil | log2_ceil(_64) | ✓ |
| ArchByteSwap | byte_swap_16/32/64 | ✓ |
| ArchRotateLeft | rotate_left_32/64 | ✓ |
| ArchRotateRight | rotate_right_32/64 | ✓ |
| ArchClamp | clamp, clamp_f32, clamp_f64 | ✓ |
| ArchLerp | lerp, lerp_f32 | ✓ |

### pragmas.h → pragmas.rs ✓

| C++ API | Rust | Notes |
|---------|------|-------|
| push_pop module | push_pop | Placeholder; compiler pragmas not applicable |

### regex.h → regex.rs ✓

| C++ API | Rust | Notes |
|---------|------|-------|
| ArchRegex | ArchRegex | ✓ |
| ArchGlobToRegex | glob_to_regex | ✓ |

### stackTrace.h → stack_trace.rs ✓

| C++ API | Rust | Notes |
|---------|------|-------|
| ArchGetStackTrace | arch_get_stack_trace | ✓ |
| ArchGetStackTraceString | arch_get_stack_trace_string | ✓ |
| ArchPrintStackTrace | arch_print_stack_trace | ✓ |
| ArchSetStackTraceCallback | arch_set_stack_trace_callback | ✓ |
| ArchClearStackTraceCallback | arch_clear_stack_trace_callback | ✓ |
| ArchLogStackTrace | arch_log_stack_trace | ✓ |
| ArchSetProcessStateLogCommand | arch_set_process_state_log_command | ✓ Command + argv + fatal_argv |
| ArchLogSessionInfo | arch_log_session_info | ✓ Token substitution ($cmd, $prog, $pid, $stack) |
| ArchSetLogSession | arch_set_log_session | ✓ Command + argv + crash_argv |

### symbols.h → symbols.rs ✓

| C++ API | Rust | Notes |
|---------|------|-------|
| ArchGetAddressInfo | arch_get_address_info | ✓ |
| ArchAddressInfo | ArchAddressInfo | ✓ |
| ArchGetCurrentFunctionAddress | arch_get_current_function_address | ✓ |

### systemInfo.h → system_info.rs ✓

| C++ API | Rust | Notes |
|---------|------|-------|
| ArchGetCwd | get_cwd | ✓ |
| ArchSetCwd | set_cwd | ✓ |
| ArchGetExecutablePath | get_executable_path | ✓ |
| ArchGetExecutableDir | get_executable_dir | ✓ |
| ArchGetTempDir | get_temp_dir | ✓ |
| ArchGetHomeDir | get_home_dir | ✓ |
| ArchGetPageSize | get_page_size | ✓ |
| ArchGetPhysicalMemory | get_physical_memory | ✓ |
| ArchGetAvailableMemory | get_available_memory | ✓ |
| ArchGetHostname | get_hostname | ✓ |
| ArchGetUsername | get_username | ✓ |
| ArchGetPid | get_pid | ✓ |
| ArchGetPpid | get_ppid | ✓ |

### threads.h → threads.rs ✓

| C++ API | Rust | Notes |
|---------|------|-------|
| ArchIsMainThread | is_main_thread | ✓ |
| ArchGetCurrentThreadId | get_current_thread_id | ✓ |
| ArchGetCurrentThreadName | get_current_thread_name | ✓ |
| ArchGetConcurrency | get_concurrency | ✓ |
| ArchGetPhysicalConcurrency | get_physical_concurrency | ✓ |
| ArchYieldProcessor | yield_processor | ✓ |
| ArchSpinLoopHint | spin_loop_hint | ✓ |
| ARCH_SPIN_PAUSE | spin_pause() + spin_pause!() | ✓ x86: _mm_pause(), ARM: __yield(), others: no-op |
| ArchSetCurrentThreadName | set_current_thread_name | ✓ |
| CriticalSection | CriticalSectionMarker, Guard | ✓ |

### timing.h → timing.rs ✓

| C++ API | Rust | Notes |
|---------|------|-------|
| ArchGetTicks | get_ticks | ✓ |
| ArchTicksToNanoseconds | ticks_to_nanoseconds | ✓ |
| ArchNanosecondsToTicks | nanoseconds_to_ticks | ✓ |
| ArchGetTicksPerSecond | get_ticks_per_second | ✓ |
| ArchGetTickQuantum | get_tick_quantum | ✓ 64 trials, 5 reads each, cached in OnceLock |
| ArchGetTime | get_time, get_time_nanos | ✓ |
| ArchSleep* | sleep_nanos, sleep_ms, sleep_secs | ✓ |
| ArchStopwatch | Stopwatch | ✓ |
| ArchIntervalTimer | IntervalTimer | ✓ |

### virtualMemory.h → virtual_memory.rs ✓

| C++ API | Rust | Notes |
|---------|------|-------|
| ArchGetVmPageSize | get_vm_page_size | ✓ |
| Memory protection | MemoryProtection enum | ✓ |
| ArchMap/Unmap | map_*, unmap | ✓ |

### vsnprintf.h → vsnprintf.rs ✓

| C++ API | Rust | Notes |
|---------|------|-------|
| ArchVStringPrintf | format_safe | ✓ |

---

## Not Ported

| C++ File | Reason |
|----------|--------|
| api.h, export.h | Rust `pub` visibility |
| pch.h, module.cpp | Build infrastructure |
| testArchAbi.h, testArchUtil.h | Internal test helpers |

---

## API Differences (by design)

1. **get_env**: Returns `Option<String>` instead of empty string; use `.unwrap_or_default()` for C++ behavior.
2. **ArchEnviron**: `env_vars()` returns iterator; no raw `char**`.
3. **set_env**: Convenience wrapper; use `set_env_with_overwrite` for overwrite param.
4. **unset_env**: Returns `bool` (success) for parity with ArchRemoveEnv.
5. **daemon**: Rust adds full `daemonize()` + `DaemonOptions`; C++ only has ArchCloseAllFiles.

---

## Implementation Notes

### Debugger Attach
`arch_debugger_attach()` reads `ARCH_DEBUGGER` env var for debugger command. On POSIX, uses double-fork so the debugger child reparents to init and can attach to the original process. Sleeps 5s to give debugger time. On Windows, calls `DebugBreak()` to trigger JIT debugger.

### Spin Pause
`spin_pause()` emits CPU-specific yield hint: `_mm_pause()` on x86/x86_64 (safe intrinsic in Rust), `__yield()` on aarch64, no-op elsewhere.

### Tick Quantum
`get_tick_quantum()` measures minimum non-zero delta across 64 trials of 5 consecutive `get_ticks()` reads. Result cached in `OnceLock`.

### Environment Variable Expansion
`expand_environment_variables()` handles `${VAR}`, `$VAR` (POSIX) and `%VAR%` (Windows). Unset variables become empty strings.

### File Name from Handle
`get_file_name()` resolves file path from open handle: `/proc/self/fd/<N>` (Linux), `fcntl(F_GETPATH)` (macOS), `GetFinalPathNameByHandleW` (Windows with `\\?\` prefix stripping).

Verified 2026-02-22.
