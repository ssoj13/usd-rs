# Animation and Time Samples

USD supports time-varying data through **time samples** -- discrete values
keyed at specific times on an attribute.

## Time Codes

A `TimeCode` represents a moment in time. The special value `TimeCode::default()`
refers to the attribute's default (non-time-sampled) value.

```rust
use usd::TimeCode;

let default_time = TimeCode::default();
let frame_1 = TimeCode::from(1.0);
let frame_24 = TimeCode::from(24.0);
```

## Writing Time Samples

```rust
use usd::{Stage, InitialLoadSet, Path, TimeCode};
use usd::tf::Token;
use usd::vt::Value;

let stage = Stage::create_in_memory(InitialLoadSet::All)?;
let prim = stage.define_prim(&Path::from("/Cube"), &Token::from("Xform"));

if let Some(attr) = prim.get_attribute(&"xformOp:translate".into()) {
    // Keyframe at frame 1
    attr.set(
        Value::from([0.0f64, 0.0, 0.0]),
        TimeCode::from(1.0),
    );
    // Keyframe at frame 24
    attr.set(
        Value::from([10.0f64, 0.0, 0.0]),
        TimeCode::from(24.0),
    );
}
```

## Reading Time Samples

```rust
let attr = prim.get_attribute(&"xformOp:translate".into()).unwrap();

// Query all sample times
let times = attr.get_time_samples();
println!("Sample times: {:?}", times);  // [1.0, 24.0]

// Query sample count
println!("Count: {}", attr.get_num_time_samples());

// Query samples in a range
let range_samples = attr.get_time_samples_in_interval(1.0, 12.0);

// Check if attribute varies over time
if attr.might_be_time_varying() {
    println!("Animated attribute");
}
```

## Interpolation

When querying a value between authored time samples, USD interpolates:

| Mode | Behavior |
|------|----------|
| `Linear` | Linear interpolation between neighboring samples |
| `Held` | Return the value of the preceding sample (step) |

The interpolation mode is set per-stage:

```rust
use usd::usd::InterpolationType;

stage.set_interpolation_type(InterpolationType::Linear);
// or
stage.set_interpolation_type(InterpolationType::Held);
```

## Value Clips

Value clips allow splitting time-sampled data across multiple files. This is
useful for:
- Per-frame geometry caches (e.g., fluid simulations)
- Reducing file sizes by isolating animation data
- Non-destructive time remapping

Clips are authored via `ClipsAPI`:

```
def "Sim" (
    clips = {
        dictionary default = {
            asset[] assetPaths = [
                @./sim.001.usdc@,
                @./sim.002.usdc@,
                @./sim.003.usdc@
            ]
            double[] times = [1, 2, 3]
            double[] active = [0, 1, 2]
            asset manifestAssetPath = @./sim.manifest.usdc@
            string primPath = "/Sim"
        }
    }
)
```

## Stage Time Metadata

Stages carry metadata that defines the time range and playback rate:

| Metadata | Meaning |
|----------|---------|
| `startTimeCode` | First frame of the stage |
| `endTimeCode` | Last frame of the stage |
| `timeCodesPerSecond` | Frames per second (default: 24) |
| `framesPerSecond` | Intended playback rate |

```rust
let start = stage.get_start_time_code();
let end = stage.get_end_time_code();
let fps = stage.get_time_codes_per_second();
println!("Range: {} - {} @ {} fps", start, end, fps);
```

## Splines (`usd-ts`)

The `usd-ts` crate provides spline/curve types for smooth animation
interpolation beyond simple linear/held sampling. This corresponds to the
C++ `TsSpline` facility.

## Skeletal Animation

For character animation, see the `usd-skel` crate which provides:
- `Skeleton` -- joint hierarchy definition
- `SkelAnimation` -- per-joint transforms over time
- `BlendShape` -- morph targets
- `SkelBindingAPI` -- binding geometry to skeleton

The imaging pipeline handles skinning deformation at render time.
