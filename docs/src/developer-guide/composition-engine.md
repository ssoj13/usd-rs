# Composition Engine (SDF / PCP / USD)

The composition engine is the heart of USD. It takes raw layer data (SDF) and
produces a composed view of the scene (USD Stage) through the PCP indexing
process.

## The Three Layers of Composition

```mermaid
graph TB
    subgraph "USD (usd-core)"
        Stage["Stage"]
        Prim["Prim"]
        Attr["Attribute"]
    end

    subgraph "PCP (usd-pcp)"
        Cache["PcpCache"]
        PrimIdx["PrimIndex"]
        PropIdx["PropertyIndex"]
        LayerStack["LayerStack"]
        Node["PcpNode (arc tree)"]
    end

    subgraph "SDF (usd-sdf)"
        Layer["Layer"]
        Spec["PrimSpec / PropertySpec"]
        Path["Path"]
        Data["AbstractData"]
    end

    Stage --> Cache
    Prim --> PrimIdx
    Attr --> PropIdx
    Cache --> LayerStack
    PrimIdx --> Node
    Node --> Layer
    LayerStack --> Layer
    Layer --> Data
    Data --> Spec
```

### SDF: Scene Description Foundation

SDF is the lowest level. It deals with individual layers and their raw
(un-composed) content:

- **Layer** -- a single file's worth of scene data, stored as a hierarchy of
  specs
- **Spec** -- a namespace entry (PrimSpec, AttributeSpec, RelationshipSpec)
  with fields (key-value pairs)
- **Path** -- a hierarchical name (`/World/Mesh.points`)
- **AbstractData** -- the backing store for spec fields; swappable between
  in-memory hash maps (for USDA) and memory-mapped crate data (for USDC)

SDF does no composition. It only knows about a single layer at a time.

### PCP: Prim Cache Population

PCP is the composition engine. Given a root layer and composition arcs, it
builds a **PrimIndex** for each prim that records all contributing opinions
from all layers, ordered by strength.

Key concepts:

- **PcpCache** -- the main composition cache, one per stage
- **LayerStack** -- an ordered set of sublayers sharing the same composition
  context
- **PrimIndex** -- the composed index for one prim path, containing a tree of
  PcpNodes
- **PcpNode** -- one node in the index tree, representing a single arc
  (reference, payload, inherit, etc.)
- **MapFunction** -- translates paths between different namespace contexts
  (e.g., the path inside a referenced file vs. the path on the stage)

### USD: Composed Stage

The USD layer (`usd-core`) builds on PCP to provide the familiar Stage/Prim
API. It:

1. Maintains a `PcpCache` for composition
2. Populates `PrimData` for each composed prim
3. Resolves attribute values by walking the PrimIndex in strength order
4. Handles instancing, value clips, and schema resolution

## Composition Walk-Through

When you call `Stage::open("scene.usda")`:

```mermaid
sequenceDiagram
    participant S as Stage
    participant PCP as PcpCache
    participant LS as LayerStack
    participant IDX as PrimIndex
    participant SDF as SdfLayer

    S->>SDF: Open root layer
    S->>PCP: Create cache with root layer
    PCP->>LS: Build root layer stack (root + sublayers)

    Note over PCP: For each prim path...

    PCP->>IDX: Compute prim index for /World
    IDX->>SDF: Check local opinions
    IDX->>SDF: Follow reference arcs
    IDX->>SDF: Follow inherit arcs
    IDX->>SDF: Follow payload arcs
    IDX-->>PCP: PrimIndex with all nodes

    PCP-->>S: Composed prim data

    Note over S: Prim.get_attribute().get(time)<br/>walks PrimIndex strongest→weakest
```

## PrimIndex Arc Tree

A PrimIndex is a tree of nodes, one per composition arc:

```mermaid
graph TD
    Root["Root node<br/>(local layer stack)"]
    Ref1["Reference<br/>chair.usda:/Chair"]
    Ref2["Reference<br/>legs.usda:/Legs"]
    Inherit["Inherit<br/>/_class_Furniture"]
    Payload["Payload<br/>geo.usdc:/Chair/Geo"]
    Specialize["Specialize<br/>/_base_Chair"]

    Root --> Inherit
    Root --> Ref1
    Ref1 --> Ref2
    Ref1 --> Payload
    Root --> Specialize
```

Each node carries:
- The layer stack contributing opinions
- A map function for path translation
- Arc type (reference, payload, inherit, specialize, variant)
- Permission and restrictions

## Value Resolution

When `Attribute::get(time)` is called:

1. Walk the PrimIndex from strongest to weakest node
2. In each node's layer stack, check each layer (strongest sublayer first)
3. Return the first authored opinion found
4. If no authored opinion, return the schema fallback value

For time-sampled attributes, the resolution also considers:
- Sublayer time offsets (`LayerOffset`)
- Value clips (if authored)
- Interpolation mode (linear or held)

## Instancing

PCP detects **instanceable** prims -- prims whose composition graphs are
structurally identical. These share a single **prototype** PrimIndex, saving
memory and composition time.

The `InstanceCache` in `usd-core` manages the mapping between instance prims
and their shared prototypes. Instance proxies allow traversal into prototype
children through the instance namespace.

## Namespace Editing

PCP supports namespace operations (rename, reparent, remove) that correctly
update all composition arcs referencing the affected paths. The
`PcpNamespaceEdits` module computes the minimal set of layer edits needed to
maintain consistency.

## Change Processing

When a layer is modified, PCP's change processing system:
1. Identifies which PrimIndexes are affected
2. Invalidates stale composition results
3. Recomputes only the affected indices
4. Notifies the Stage via `PcpChanges`

The Stage then updates its PrimData cache and emits USD notices to downstream
consumers (e.g., Hydra).
