# Python API Parity — Current State

## What I was doing
Adding CamelCase #[pyo3(name = "...")] aliases to ALL methods in geom.rs
Using mcp__filesystem__bulk_edits for mass renaming — works great.

## Test counts (last measured):
- base/gf: 126/147 (86%)
- base/vt: 24/28 (86%)
- usd/sdf: 38/210 (18%)
- usd/usd: 36/527 (7%)
- usd/ar: 36/52 (69%)
- usd/pcp: 0/111 (0%)
- usd/usdGeom: 0/206 → should improve after CamelCase rename
- usd/usdSkel: 4/33 (12%)

## What's done this session
- geom.rs: bulk CamelCase rename for all 39 schema classes (Get/Define/Apply/GetPrim/GetPath/attr methods)
- usd.rs: Prim 30+ new methods, Stage 15 missing methods, GetPath→PyPath, Tf.Notice.Register
- sdf.rs: Layer missing methods (GetFileFormat, Clear*, GetExternalReferences, etc.), Sdf.Find
- pcp.rs: PyPrimIndex.from_index constructor

## What's next
1. Finish CamelCase rename in geom.rs — remaining snake_case methods
2. Add remaining missing methods per C++ reference for each module
3. Focus: usd/usd (527 tests), usd/sdf (210 tests), usd/usdGeom (206 tests) — biggest impact
4. NEVER run tests — operator does it. NEVER use sed. Use Edit tool and filesystem MCP bulk_edits.
5. NEVER launch long-running agents. Do work directly.
