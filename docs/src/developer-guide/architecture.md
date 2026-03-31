# Architecture Overview

usd-rs mirrors the layered architecture of C++ OpenUSD, organized into four
major tiers: **Base**, **USD Core**, **Schemas**, and **Imaging**.

## High-Level Architecture

```mermaid
graph TB
    subgraph "Application Layer"
        Viewer["usd-view<br/>(egui viewer)"]
        CLI["usd CLI<br/>(cat, tree, dump...)"]
        App["Your Application"]
    end

    subgraph "Imaging Layer"
        UsdImaging["usd-imaging<br/>(Stage → Hydra)"]
        Engine["Engine<br/>(HDX + Storm)"]

        subgraph "Hydra"
            HD["usd-hd<br/>(Hydra core)"]
            HdSt["usd-hd-st<br/>(Storm renderer)"]
            HdSI["usd-hdsi<br/>(Scene index plugins)"]
            HDX["usd-hdx<br/>(Task controller)"]
        end

        subgraph "GPU"
            HGI["usd-hgi<br/>(GPU abstraction)"]
            WGPU["usd-hgi-wgpu<br/>(wgpu backend)"]
        end
    end

    subgraph "Schema Layer"
        Geom["usd-geom"]
        Shade["usd-shade"]
        Lux["usd-lux"]
        Skel["usd-skel"]
        More["usd-vol, usd-physics,<br/>usd-render, ..."]
    end

    subgraph "USD Core Layer"
        Core["usd-core<br/>(Stage, Prim, Attribute)"]
        PCP["usd-pcp<br/>(composition engine)"]
        SDF["usd-sdf<br/>(layers, paths, specs)"]
        AR["usd-ar<br/>(asset resolution)"]
    end

    subgraph "Base Layer"
        TF["usd-tf<br/>(tokens, diagnostics)"]
        GF["usd-gf<br/>(math)"]
        VT["usd-vt<br/>(value types)"]
        Arch["usd-arch"]
        Work["usd-work"]
        JS["usd-js"]
        Plug["usd-plug"]
        Trace["usd-trace"]
        TS["usd-ts"]
    end

    Viewer --> Engine
    CLI --> Core
    App --> Core
    App --> Geom
    Engine --> UsdImaging
    UsdImaging --> HD
    UsdImaging --> Core
    HD --> HdSt
    HD --> HdSI
    HD --> HDX
    HdSt --> HGI
    HGI --> WGPU
    Geom --> Core
    Shade --> Core
    Lux --> Core
    Skel --> Core
    Core --> PCP
    PCP --> SDF
    SDF --> AR
    Core --> TF
    Core --> GF
    Core --> VT
    SDF --> TF
    SDF --> VT
    PCP --> TF
    AR --> TF
    AR --> Plug
```

## Design Principles

### Pure Rust, No Bindings

Every module is implemented from scratch in Rust. There is no C/C++ dependency,
no FFI layer, and no unsafe code in the core USD pipeline. The C++ OpenUSD
source serves as the behavioral reference, not as linked code.

### Arc-Based Ownership

Stages, layers, and other long-lived objects use `Arc<T>` for shared ownership.
This enables safe multi-threaded access and matches the reference-counted
semantics of the C++ `TfRefPtr`/`SdfLayerRefPtr`.

### Interior Mutability

USD's data model requires mutation through shared references (e.g., setting
attribute values on a composed stage). This is handled through `RwLock` and
`Mutex` where needed, keeping the public API ergonomic.

### Error Propagation

All fallible operations return `Result<T, Error>` instead of panicking or
silently failing. This replaces the C++ pattern of `TF_CODING_ERROR` macros
with idiomatic Rust error handling.

### Token Interning

Frequently-used strings (type names, attribute names, schema tokens) are
interned as `Token` values (from `usd-tf`). Token comparison is O(1) pointer
equality rather than string comparison.

## Key Differences from C++ OpenUSD

| Area | C++ | Rust |
|------|-----|------|
| Memory management | `TfRefPtr`, raw pointers | `Arc<T>`, ownership |
| GPU backend | OpenGL (Storm) | wgpu (Vulkan/Metal/DX12) |
| UI framework | Qt (usdview) | egui (usd-view) |
| Build system | CMake + Boost | Cargo |
| Plugin system | Dynamic `.so`/`.dll` loading | Static registration via `usd-plug` |
| Thread pool | TBB | Rayon + `usd-work` |
| Logging | TF_STATUS/WARN/ERROR | `tracing` + `log` crates |
| Python bindings | Built-in (Boost.Python) | Not yet available |

## Data Flow: Opening a Stage

```mermaid
sequenceDiagram
    participant App
    participant Stage as usd-core::Stage
    participant PCP as usd-pcp::Cache
    participant SDF as usd-sdf::Layer
    participant AR as usd-ar::Resolver
    participant IO as File I/O

    App->>Stage: Stage::open("scene.usda")
    Stage->>AR: Resolve asset path
    AR-->>Stage: /abs/path/scene.usda
    Stage->>SDF: Layer::find_or_open()
    SDF->>IO: Read + parse file
    IO-->>SDF: Layer data
    SDF-->>Stage: Arc<Layer>
    Stage->>PCP: Build prim index
    PCP->>SDF: Compose sublayers
    PCP->>SDF: Resolve references/payloads
    PCP-->>Stage: Composed prim cache
    Stage-->>App: Arc<Stage>
```
