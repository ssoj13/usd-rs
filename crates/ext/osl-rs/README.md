# osl-rs

Pure Rust implementation of [OpenShadingLanguage](https://github.com/AcademySoftwareFoundation/OpenShadingLanguage) (OSL).

No C++ toolchain, no LLVM, no FFI -- just `cargo build`.

## Status

**65,600+ lines of Rust**, 818 tests (743 lib + 75 e2e), 58 modules. ~98% parity with C++ reference (160+ opcodes, 60-item audit completed). Cranelift JIT backend (15-19x faster than interpreter, with compilation cache), full Dual2 derivative propagation, analytical noise derivatives (Perlin + Periodic + Simplex + Gabor), SIMD-ready batched execution with per-lane masking, OCIO color management via `vfx-ocio`, real texture sampling via `vfx-io`, USD integration bridge.

| Component | Status | Lines |
|-----------|--------|-------|
| Compiler (lexer, parser, type checker, codegen) | Complete | ~7,700 |
| OSO format (read/write) | Complete, binary-compatible with C++ oslc | ~600 |
| Runtime interpreter (160+ opcodes, struct fields, derivatives) | Complete | 4,876 |
| Cranelift JIT backend (15-19x faster, safe div/mod, 10 math trampolines) | Complete | 7,621 |
| Batched execution (per-lane masks, 20+ opcodes, SIMD-ready) | Complete | 5,753 |
| Optimizer (24+ passes, constant folding, peephole) | Complete | 4,761 |
| ShadingSystem API (groups, connections, execution, instance merging) | Complete | 3,373 |
| Noise (Perlin, Cell, Simplex, Gabor, Periodic + analytical derivs) | Complete | 1,557 |
| Color system (7 builtin spaces, blackbody, OCIO routing) | Complete | 1,385 |
| BSDFs (22 models: SPI + MaterialX + Chiang 2016 Hair) | Complete | 2,127 |
| Closures (arena allocator, tree ops, 28 closure IDs, ClosureParams) | Complete | ~900 |
| Splines (6 bases, inverse, derivatives) | Complete | ~500 |
| RendererServices trait (full: xforms, messages, texture, cache, encoded fmt) | Complete | 1,317 |
| LPE engine ({n,m} repetition, state stack) | Complete | 1,101 |
| USD integration bridge (Material/Shader -> ShaderGroup) | Complete | ~440 |
| Performance benchmarks (criterion) | Complete | ~200 |

## What is OSL?

Open Shading Language is the industry-standard shading language for physically-based rendering, developed by Sony Pictures Imageworks. Used in Arnold, RenderMan, V-Ray, Cycles, and many others.

## Quick Start

```rust
use osl_rs::prelude::*;

// Compile OSL source to IR and execute
let result = osl_rs::interp::run_shader(
    r#"
    shader simple(float Kd = 0.8, output color Cout = 0) {
        Cout = Kd * color(1, 0.5, 0.25);
    }
    "#,
);
// result["Cout"] == Color3(0.8, 0.4, 0.2)
```

### Compile .osl to .oso

```rust
use osl_rs::oslc;

let oso_bytes = oslc::compile_buffer(
    "shader diffuse(float Kd = 0.8) { Ci = Kd * diffuse(N); }",
    &oslc::CompileOptions::default(),
)?;
```

### Full shading pipeline

```rust
use osl_rs::shadingsys::*;
use osl_rs::renderer::NullRenderer;
use std::sync::Arc;

let ss = ShadingSystem::new(Arc::new(NullRenderer), None);

// Load shader from .oso
let group = ss.shader_group_begin("my_group");
ss.parameter_simple(&group, "Kd", ParamValue::Float(0.5));
ss.shader(&group, "surface", "my_shader", "layer1");
ss.shader_group_end(&group);

// Execute
let mut globals = ShaderGlobals::default();
globals.P = Vec3::new(0.5, 0.5, 0.0);
ss.execute(&group, &mut globals);
```

## Features

```toml
[features]
default = ["vfx", "capi", "jit"]

vfx  = ["dep:vfx-io", "dep:vfx-ocio"]  # Real textures + OCIO color management
capi = []                                # C-compatible FFI exports
jit  = ["dep:cranelift-*"]               # Cranelift JIT backend
```

| Feature | What it enables |
|---------|----------------|
| `vfx` | Real texture sampling (MIP, aniso, tiled cache) via `vfx-io`, OCIO color transforms via `vfx-ocio` |
| `jit` | Cranelift-based native code generation, 15-19x faster than interpreter |
| `capi` | `extern "C"` exports for embedding osl-rs as a C library |

Without `vfx`, osl-rs remains zero-dependency on external image/color libraries, using procedural texture stubs and 7 builtin color space conversions (RGB, HSV, HSL, YIQ, XYZ, xyY, sRGB).

## Architecture

```
osl-rs (65,600 lines, 58 modules)
|
|-- Compiler (frontend)
|   |-- preprocess.rs    Preprocessor (#define, #include, #if, #pragma)
|   |-- lexer.rs         Tokenizer (hand-written, 49 reserved words, metadata tokens)
|   |-- parser.rs        Recursive descent parser (bison-equivalent)
|   |-- ast.rs           Abstract syntax tree (canconstruct, negate, arg bitmasks)
|   |-- typecheck.rs     Type checker + overload resolution + printf/texture validation
|   |-- codegen.rs       AST -> ShaderIR (struct expansion, arraycopy, break/continue)
|   |-- oso.rs           OSO format reader/writer
|   +-- oslc.rs          Compiler CLI driver (%structfields hints)
|
|-- Runtime (execution)
|   |-- shadingsys.rs    ShadingSystem (groups, connections, instance merging, lifecycle)
|   |-- interp.rs        IR interpreter (160+ opcodes, safe div/mod, Dual2 derivs)
|   |-- jit.rs           Cranelift JIT (safe fdiv, 10 math trampolines, construct)
|   |-- batched_exec.rs  SIMD batched execution (mask stack, 20+ opcodes)
|   |-- batched.rs       Batched types (Wide<T>, BatchedShaderGlobals)
|   |-- optimizer.rs     Runtime optimizer (24+ passes, constant fold, peephole)
|   |-- context.rs       ShadingContext (heap, closures, layer tracking, max_warnings)
|   |-- renderer.rs      RendererServices trait (xforms, messages, texture, encoded fmt)
|   |-- encodedtypes.rs  Encoded message decoding for JIT printf/warning/error
|   |-- usd_bridge.rs    USD Material -> OSL ShaderGroup bridge
|   +-- shaderglobals.rs ShaderGlobals (repr(C), 35 fields)
|
|-- Built-in Libraries
|   |-- noise.rs         Perlin/Cell/Hash + derivatives (seeded simplex components)
|   |-- simplex.rs       Simplex noise 1D-4D
|   |-- gabor.rs         Gabor noise (iso/aniso/hybrid, weighted derivatives)
|   |-- color.rs         7 builtin spaces, blackbody, OCIO routing for all others
|   |-- spline.rs        6 basis types (Catmull-Rom, BSpline, Bezier, Hermite, Linear, Constant)
|   |-- builtins.rs      Builtin function dispatch
|   |-- stdosl.rs        Standard library declarations (closures, constants, color spaces)
|   |-- opstring.rs      String operations + regex
|   |-- matrix_ops.rs    Matrix transforms (deduplicated transform_normal)
|   |-- texture.rs       Texture sampling (blur params, procedural fallback)
|   +-- texture_vfx.rs   VFX texture system adapter (vfx-io bridge, feature-gated)
|
|-- BSDF Framework (22 models)
|   |-- bsdf.rs          Base trait, Diffuse, OrenNayar, Microfacet, Transparent
|   +-- bsdf_ext.rs      Metal, Dielectric, Sheen (Classic/LTC), Clearcoat, Volume,
|                         Hair (Chiang 2016: R/TT/TRT/residual), Phong, Ward,
|                         DeltaReflection, SpiThinLayer, Backscatter, ThinFilm,
|                         Spectral, MtxConductor, Refraction, Translucent,
|                         Subsurface, Emission
|
|-- Closures
|   |-- closure.rs       Closure color tree (repr(C), ClosureParams enum)
|   +-- closure_ops.rs   Arena allocator, 28 closure IDs (SPI + 14 MaterialX), params
|
|-- Light Path Expressions
|   |-- lpe.rs           NFA-based LPE engine ({n,m} repetition, bounded rep)
|   +-- accum.rs         Light path accumulator (state stack, push/pop)
|
|-- Support
|   |-- symbol.rs        Symbol/Opcode structs (14+ accessor methods)
|   |-- symtab.rs        Symbol table (FunctionSymbol with argcodes)
|   |-- typedesc.rs      TypeDesc (repr(C), OIIO-compatible)
|   |-- typespec.rs      Extended type system (code_from_type)
|   |-- ustring.rs       Interned strings (thread-safe, 64 shards)
|   |-- dual.rs          Dual2<T> automatic differentiation
|   |-- dual_vec.rs      Dual2<Vec3> operations
|   |-- math.rs          Vec3, Color3, Matrix44
|   |-- message.rs       Inter-shader message passing (Matrix + array types)
|   |-- hashes.rs        OIIO-compatible hashing (inthash_vec3/vec4)
|   +-- pointcloud.rs    Point cloud search (grid-based)
|
+-- Tools
    |-- oslquery.rs      Query shader parameters (with data field)
    |-- oslinfo.rs       Shader info CLI
    +-- capi.rs          C API (extern "C", attribute stubs)
```

## Key Implementation Details

### Cranelift JIT Backend (jit.rs -- 7,621 lines)

Compiles shader IR to native x86-64 code via Cranelift. Key features:
- **Safe arithmetic**: `emit_safe_fdiv()` protects 30+ division sites from producing INF/NaN
- **10 math trampolines**: cbrt, log2, log10, logb, exp2, expm1, erf, erfc, round, trunc
- **Construct opcode**: matrix from 16 floats, diagonal matrix, type casts
- **Saturating conversions**: `fcvt_to_sint_sat` prevents traps on NaN/infinity
- **Compilation cache**: Compiled function groups are cached and reused

### Batched Execution (batched_exec.rs -- 4,275 lines)

SIMD-ready execution model with per-lane masking for divergent control flow:
- **Mask stack**: Push/pop for if/else/loop with correct per-lane divergence
- **20+ opcodes**: aref, aassign, closure, spline, mxcompref, mxcompassign, arraycopy, format, regex, transformc, blackbody, raytype, messages, dict, pointcloud
- **StringArray/MatrixArray dispatch**: Full type support in array operations
- **set_masked**: Per-lane writes respecting current execution mask

### Optimizer (optimizer.rs -- 4,761 lines)

Runtime optimization of shader IR before execution:
- **Constant folding**: 40+ rules including int ops, bitwise, clamp, div/0->0, mod/0->0
- **Peephole**: sub(A,A)->0, sub(x,0)->x, div/mod by zero->0, select optimization
- **Dead code elimination**: Removes unreachable ops after constant-folded branches
- **Instance merging**: Merge identical shader instances
- **Exact comparison**: Float eq/neq uses exact comparison (not epsilon), matching C++

### Color System (color.rs -- 1,385 lines)

Seven builtin analytical color spaces: RGB, HSV, HSL, YIQ, XYZ, xyY, sRGB.
All other spaces (ACEScg, ACES AP0, NTSC, Rec2020, EBU, etc.) route through OCIO via `vfx-ocio`, matching C++ `transformc()` behavior exactly.

### Interpreter (interp.rs -- 4,876 lines)

160+ opcodes with full derivative (Dual2) support:
- **Safe division**: Uses `isfinite(a/b)` check (not `b!=0`), matching C++
- **C-convention fmod**: Rust `%` operator, not `rem_euclid`
- **Color space constructors**: `color("hsv", h, s, v)` syntax
- **Smoothstep Dual2**: Full derivative propagation through smoothstep
- **Texture derivatives**: Pass-through of `dresultds`/`dresultdt`
- **Message passing**: 4-arg getmessage with source, Matrix44 + array types

### BSDF Models (bsdf.rs + bsdf_ext.rs -- 2,127 lines)

22 physically-based BSDF models:
- **SPI standard**: Diffuse, OrenNayar, Microfacet (GGX/Beckmann), Metal, Dielectric, Transparent
- **MaterialX**: Sheen (Classic + LTC modes), Clearcoat, ThinFilm, Spectral
- **Hair**: Chiang 2016 with 4 lobes (R/TT/TRT/residual), longitudinal scattering, azimuthal functions
- **Classic**: Phong, Ward, DeltaReflection, SpiThinLayer
- **Volumetric**: Volume, Subsurface, Backscatter, Translucent, Refraction, Emission

### Closures (closure.rs + closure_ops.rs -- ~900 lines)

Arena-allocated closure color tree with parameter data:
- **ClosureParams enum**: None, Normal, NormalRoughness, NormalIor, NormalRoughnessFresnel, Custom
- **28 closure IDs**: C++-compatible numbering (EMISSION=1, BACKGROUND=2, DIFFUSE=3...)
- **Tree operations**: add, mul, component allocation with inline params

## Integration with vfx-rs

osl-rs works with [vfx-rs](../vfx-rs/), a pure Rust port of OpenImageIO + OpenColorIO + OpenEXR in the same workspace.

### What vfx-rs provides

| osl-rs need | vfx-rs solution | Crate |
|-------------|----------------|-------|
| `texture()` -- 2D texture sampling | `TextureSystem::sample()` with MIP filtering, tiled cache | `vfx-io` |
| `texture3d()` -- 3D texture lookup | `TextureSystem::texture3d()` | `vfx-io` |
| `environment()` -- env map lookup | `TextureSystem::environment()` with direction->latlong | `vfx-io` |
| Texture cache (LRU, tiled) | `ImageCache` -- 256MB default, 64x64 tiles, thread-safe | `vfx-io` |
| `transformc("sRGB", "ACEScg")` | `vfx_ocio::Config::processor().apply_rgb()` | `vfx-ocio` |
| OCIO config support | Full `.ocio` parser + built-in ACES 1.3 | `vfx-ocio` |
| EXR/PNG/TIFF/DPX read/write | `vfx_io::read()` / `vfx_io::write()` auto-detect | `vfx-io` |

### Integration architecture

```
                    Renderer
                       |
                       v
                +--------------+
                |  osl-rs       |
                |  ShadingSystem|
                +------+-------+
                       |
          +------------+------------+
          v            v            v
   +------------+ +---------+ +---------+
   | osl-rs     | | vfx-io  | | vfx-ocio|
   | Interpreter| | Texture | | Color   |
   | (160+ ops) | | System  | | Config  |
   +------------+ +----+----+ +----+----+
                       |           |
                  +----+----+     |
                  |ImageCache|     |
                  |(LRU tile)|     |
                  +----+----+     |
                       |          |
                  +----+----+ +--+---+
                  | vfx-exr | |ACES  |
                  | vfx-core| |LUTs  |
                  +---------+ +------+
```

## Intentionally Excluded

| Component | Reason |
|-----------|--------|
| LLVM JIT backend | Replaced by Cranelift (pure Rust, no C++ deps, 15-19x faster than interpreter) |
| OptiX / GPU backend | Hardware-specific, not part of core OSL |
| SIMD intrinsics (AVX2/AVX-512) | Scalar batched lanes work; intrinsics are a future optimization |
| osltoy / testrender / testshade | Test applications, not library code |
| Gabor RNG (full C++ state machine) | Simplified RNG; same visual quality |
| Simplex variant (patent-free) | Using standard simplex implementation |

## Parity with C++ Reference

~98% feature parity with C++ OpenShadingLanguage. Full 60-item audit completed.

Documentation:
- [PARITY.md](PARITY.md) -- parity status table
- [PLAN.md](PLAN.md) -- consolidated audit report (all 60 items)
- [DIAGRAMS.md](DIAGRAMS.md) -- ASCII architecture diagrams, codepaths, dataflows
- [AGENTS.md](AGENTS.md) -- module map, data flow, type hierarchy, thread model

Fully implemented:
- Complete compiler pipeline (preprocess -> lex -> parse -> typecheck -> codegen -> OSO)
- All 160+ interpreter opcodes with correct semantics
- Cranelift JIT with safe arithmetic and full opcode coverage
- Batched execution with per-lane masking
- Runtime optimizer with 40+ constant fold rules
- 28 closure types with ClosureParams data (SPI standard + MaterialX)
- 22 BSDF models (including Chiang 2016 hair)
- OCIO color management (7 builtin + unlimited OCIO spaces)
- Full RendererServices trait with encoded format methods
- LPE engine with {n,m} bounded repetition
- Struct field expansion in codegen
- Standard OSO opcodes (break, continue, arraycopy)
- USD integration bridge

Remaining architectural differences:
- Cranelift vs LLVM (different IR, same output quality)
- Procedural texture stubs when `vfx` feature disabled (vs OIIO in C++)
- Simplified Gabor RNG state

## Dependencies

```toml
# Required
dashmap = "6"          # Concurrent hash maps
parking_lot = "0.12"   # Fast mutexes
bitflags = "2"         # Bitflag types
libm = "0.2"           # Math functions (no_std compatible)
logos = "0.16"          # Lexer generator

# Optional (vfx feature)
vfx-io = { workspace = true }    # Texture system + image I/O
vfx-ocio = { workspace = true }  # OpenColorIO color management

# Optional (jit feature)
cranelift-codegen = "0.128.3"    # Code generation
cranelift-frontend = "0.128.3"   # IR builder
cranelift-jit = "0.128.3"        # JIT engine
cranelift-module = "0.128.3"     # Module linking
cranelift-native = "0.128.3"     # Native target
```

## Tests

```
818 tests (743 lib + 75 e2e), 0 failures
```

Run lib tests:
```sh
cargo test --lib
```

Run e2e tests:
```sh
cargo test --test e2e
```

Run with all features:
```sh
cargo test --all-features
```

## References

- [OSL GitHub](https://github.com/AcademySoftwareFoundation/OpenShadingLanguage)
- [OSL Specification](https://open-shading-language.readthedocs.io/)
- [OSL Language Spec PDF](https://github.com/AcademySoftwareFoundation/OpenShadingLanguage/blob/main/src/doc/osl-languagespec.pdf)
- [vfx-rs](../vfx-rs/) -- Pure Rust OIIO/OCIO/OpenEXR
- [Cranelift](https://cranelift.dev/) -- Pure Rust code generator

## License

Apache-2.0
