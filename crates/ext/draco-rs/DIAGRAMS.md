# DRACO-RS Diagrams

This file is the visual companion to `README.md`, `AGENTS.md`, and `DRACO_PLAN.md`.

It describes the current reference-aligned architecture of `draco-rs` after the completed parity audit.

## Workspace Topology
```mermaid
flowchart TD
    Root[draco-rs root] --> Core[crates/draco-core]
    Root --> Bitstream[crates/draco-bitstream]
    Root --> Cli[crates/draco-cli]
    Root --> Js[crates/draco-js]
    Root --> Maya[crates/draco-maya]
    Root --> Unity[crates/draco-unity]
    Root --> App[src high-level modules]

    Core --> CoreTypes[core types]
    Core --> CoreAttr[attributes]
    Core --> CorePc[point_cloud]
    Core --> CoreMesh[mesh]
    Core --> CoreMeta[metadata]
    Core --> CoreMatTex[material + texture]

    Bitstream --> BDecode[decode]
    Bitstream --> BEncode[encode + expert_encode]
    Bitstream --> BMesh[mesh codecs]
    Bitstream --> BPc[point-cloud codecs]
    Bitstream --> BAttr[attribute codecs]

    App --> AppIo[src/io]
    App --> AppScene[src/scene]
    App --> AppAnim[src/animation]
    App --> AppTools[src/tools]
```

## Layering
```mermaid
flowchart TD
    UserAPI[draco-rs public API] --> HighLevel[src/io + src/scene + src/animation + src/tools]
    UserAPI --> Bindings[js / maya / unity wrappers]
    HighLevel --> Codec[draco-bitstream codec layer]
    Bindings --> Codec
    Codec --> Core[draco-core data model]
    HighLevel --> Core
```

## Draco Decode Pipeline
```mermaid
flowchart TD
    Input[.drc bytes] --> Buffer[DecoderBuffer]
    Buffer --> Decode[draco-bitstream::compression::decode]
    Decode --> Header[header parse]
    Header --> Kind{geometry type}

    Kind -->|Mesh| MeshDecoder[Mesh decoder]
    Kind -->|PointCloud| PcDecoder[Point-cloud decoder]

    MeshDecoder --> Connectivity[connectivity reconstruction]
    MeshDecoder --> AttrDecode[attribute decode]
    PcDecoder --> AttrDecode

    Connectivity --> MeshObj[draco-core Mesh]
    AttrDecode --> MeshObj
    AttrDecode --> PcObj[draco-core PointCloud]

    MeshObj --> MeshIo[src/io/mesh_io]
    MeshObj --> GltfDecode[src/io/gltf_decoder]
    MeshObj --> BindingMesh[js / maya / unity]

    PcObj --> PcIo[src/io/point_cloud_io]
    PcObj --> GltfDecode
    PcObj --> BindingPc[js / maya / unity]
```

## Draco Encode Pipeline
```mermaid
flowchart TD
    InMesh[Mesh] --> Expert[ExpertEncoder]
    InPc[PointCloud] --> Expert

    Expert --> Validate[input validation]
    Validate --> Select{encoding method}

    Select -->|Mesh| MeshEncoder[mesh encoder]
    Select -->|PointCloud| PcEncoder[point-cloud encoder]

    MeshEncoder --> ConnEncode[connectivity encoding]
    MeshEncoder --> AttrEncode[attribute encoding]
    PcEncoder --> AttrEncode

    AttrEncode --> Buffer[EncoderBuffer]
    ConnEncode --> Buffer
    Buffer --> Output[.drc bytes]
```

## File IO Dispatch
```mermaid
flowchart TD
    File[filename or stream] --> Ext[file extension or explicit path]
    Ext --> MeshIO[src/io/mesh_io.rs]
    Ext --> PointCloudIO[src/io/point_cloud_io.rs]
    Ext --> SceneIO[src/io/scene_io.rs]

    MeshIO --> ObjMesh[OBJ decode/encode]
    MeshIO --> PlyMesh[PLY decode/encode]
    MeshIO --> StlMesh[STL decode/encode]
    MeshIO --> DracoMesh[raw Draco mesh decode/encode]
    MeshIO --> GltfMesh[glTF mesh decode/encode]

    PointCloudIO --> ObjPc[OBJ point cloud]
    PointCloudIO --> PlyPc[PLY point cloud]
    PointCloudIO --> DracoPc[raw Draco point cloud]

    SceneIO --> GltfScene[glTF / GLB scene IO]
```

## Reference-Sensitive Decode Semantics
```mermaid
flowchart LR
    RawDrc[raw .drc mesh decode] --> DirectMesh[return decoded mesh directly]
    ObjDecode[OBJ decode] --> ObjDedup[post-decode attribute and point dedup]
    PlyDecode[PLY decode] --> PlyDedup[post-decode attribute and point dedup]
    SoupFinalize[TriangleSoupMeshBuilder::Finalize] --> SoupDedup[dedup before returning mesh]
```

## Mesh and Point-Cloud Point Deduplication
```mermaid
flowchart TD
    Mesh[Mesh::deduplicate_point_ids] --> Shared[build_point_deduplication_map]
    PointCloud[PointCloud::deduplicate_point_ids] --> Shared

    Shared --> Signature[point signature from mapped attribute ids]
    Signature --> IndexMap[index remap]
    IndexMap --> ApplyMesh[remap faces and attribute point maps]
    IndexMap --> ApplyPc[remap attribute point maps]
```

## Point Signature Rule
```mermaid
flowchart LR
    Point[PointIndex] --> Attr0[mapped_index on attribute 0]
    Point --> Attr1[mapped_index on attribute 1]
    Point --> AttrN[mapped_index on attribute N]
    Attr0 --> Signature[Vec<u32> signature]
    Attr1 --> Signature
    AttrN --> Signature
    Signature --> Equality[two points are equal iff signatures match]
```

## glTF Scene Flow
```mermaid
flowchart TD
    GltfIn[.gltf / .glb] --> Preflight[required-extension preflight]
    Preflight --> Buffers[JSON + BIN buffers]
    Buffers --> Nodes[node hierarchy]
    Buffers --> Meshes[primitive decode]
    Buffers --> Materials[materials + textures]
    Buffers --> Anim[animations + skins]
    Buffers --> Meta[structural metadata + mesh features]

    Meshes --> Scene[Scene]
    Nodes --> Scene
    Materials --> Scene
    Anim --> Scene
    Meta --> Scene

    Scene --> GltfEncoder[src/io/gltf_encoder.rs]
    GltfEncoder --> Accessors[accessors + buffer views]
    GltfEncoder --> DracoExt[KHR_draco_mesh_compression path]
    GltfEncoder --> GltfOut[JSON + BIN / GLB]
```

## glTF Extension Support Gate
```mermaid
flowchart LR
    Asset[glTF asset] --> Required[extensionsRequired]
    Required --> Registry[shared required-extension registry]
    Registry --> Supported{supported?}
    Supported -->|yes| Continue[decode continues]
    Supported -->|no| Reject[unsupported feature error]
    Continue --> SceneOrMesh[Scene / Mesh / PointCloud output]
```

## Scene Material Removal
```mermaid
flowchart TD
    Remove[Scene::remove_material] --> Validate[validate all references first]
    Validate -->|still referenced| Error[return error with no mutation]
    Validate -->|unused| Delete[remove from material library]
    Delete --> Reindex[reindex remaining material references]
    Reindex --> Done[consistent scene state]
```

## Metadata Dataflow
```mermaid
flowchart TD
    RawName[MetadataName raw bytes] --> CoreMeta[core metadata structures]
    RawString[MetadataString raw bytes] --> CoreMeta

    CoreMeta --> JsText[JS adapter converts to text when needed]
    CoreMeta --> GltfText[glTF encoder converts to text when needed]
    CoreMeta --> ObjText[OBJ encoder converts to text when needed]

    CoreMeta --> CoreRoundtrip[core roundtrip preserves bytes]
```

## Binding Ownership Model
```mermaid
flowchart TD
    Decode[decode mesh / point cloud] --> MeshHandle[owned decoded object]
    MeshHandle --> ExportAttr[export attribute handle]
    ExportAttr --> Snapshot[heap-owned attribute snapshot]
    Snapshot --> Read[GetAttributeData / materialize values]
    MeshHandle --> Release[release original mesh handle]
    Release --> SafeRead[attribute handle remains valid because it owns snapshot state]
```

## FFI Error Translation
```mermaid
flowchart LR
    DecodeStatus[Status / StatusOr from Rust core] --> Translate[translate to explicit ABI result]
    Translate --> ErrorCode[numeric error code / null / false]
    Translate --> Output[successful exported object]
```

## Validation Workflow
```mermaid
flowchart TD
    Start[workspace root] --> Meta[cargo test -p draco-core metadata_ --lib]
    Meta --> Build[cargo test -p draco-rs --no-run]
    Build --> Gltf[cargo test -p draco-rs gltf_decoder_test_ -- --test-threads=1]
    Gltf --> Full[cargo test -p draco-rs -- --test-threads=1]
    Full --> Green[current verified state]
```

## Isolated Target Directory
```mermaid
flowchart LR
    SharedWorkspace[shared usd-rs workspace] --> Contention[target lock contention]
    Contention --> Symptoms[timeouts / misleading hangs]
    Isolated[CARGO_TARGET_DIR=crates/ext/draco-rs/target-draco-verify] --> CleanRuns[isolated draco-rs builds and tests]
    CleanRuns --> Reliable[reliable validation signal]
```

## Document Map
```mermaid
flowchart TD
    README[README.md] --> UserGuide[user-facing overview]
    AGENTS[AGENTS.md] --> MaintainerGuide[maintainer and agent invariants]
    PLAN[DRACO_PLAN.md] --> AuditLedger[full parity ledger]
    DIAGRAMS[DIAGRAMS.md] --> VisualMap[visual dataflows and codepaths]
    Checkpoints[plan1.md / plan2.md / plan3.md] --> Milestones[historical checkpoints]
    Reports[md/agent_*.md + md/README.md] --> Evidence[narrow audit evidence]
```
