# usd-rs

![usd-rs viewer](data/usdview.jpg)

Pure Rust port of OpenUSD:

- This repository is a pure experimental ground-up rewrite of Pixar's OpenUSD architecture in Rust.
- It is not a binding layer, but a pure Rust implementation.
- The C++ reference lives at [PixarAnimationStudios/OpenUSD](https://github.com/PixarAnimationStudios/OpenUSD) and remains the behavior target for composition, imaging, Hydra, and viewer semantics.
- This repo is large and still under active parity work against OpenUSD.
- For architectural details and crate mapping, see [`STRUCTURE.md`](./STRUCTURE.md).
- It's not production ready and is not supposed to be used by anyone.
- Sudden changes and API rewrites are possible at any moment.


## Workspace

- Core USD crates live under [`crates/usd/`](./crates/usd/)
- Hydra and imaging crates live under [`crates/imaging/`](./crates/imaging/)
- The USD scene delegate lives under [`crates/usd-imaging/`](./crates/usd-imaging/)
- The viewer app lives under [`crates/usd-view/`](./crates/usd-view/)

## Current Viewer Status

Recent work focused on `usd-view` correctness and parity on heavy real-world files.

- `flo.usdz` hierarchical xform animation now propagates time dirties correctly through Hydra.
- Hover/orbit stutter was removed by keeping hover on the fast GPU picking path and avoiding the catastrophic fallback on passive motion.
- Picking correctness was tightened after fixing coordinate/readback issues and selection-tracker churn.
- Viewer first-load framing and free-camera clipping now use composed stage bounds from `BBoxCache` instead of render-index bookkeeping.
- Manual free-camera near/far values are treated as explicit overrides only, not as the default runtime projection policy.
- `bmw_x3.usdc` / `bmw_x3.usdz` camera handling was further tightened so packaged tiny-scene assets use scene-aware free-camera clipping during load and camera motion.
- Workspace compiler warnings were cleaned up and remaining scene-index `unsafe` sites were documented and consolidated where possible.

## Diagnostics

- `usd view <file>` launches the viewer.
- `usd meshdump <file> <primPath> [time]` dumps composed mesh/xform/bounds diagnostics for investigation.
- [`profile_flo_usdz.cmd`](./profile_flo_usdz.cmd) runs a profiled release viewer session for `data/flo.usdz`.

## Validation

Typical checks used during current work:

```powershell
cargo check --quiet -p usd-view
cargo check --quiet -p usd-imaging --lib
cargo check --quiet -p usd-hd-st --lib
```

## References:
  - [ssoj13/usd-refs](https://github.com/ssoj13/usd-refs)
