# usd-core Infrastructure Bug: Layer Offset Not Applied to Timecode Values

Found during full parity check of `usd-media` crate against C++ reference `pxr/usd/usdMedia`.

---

## BUG: Layer Offset Not Applied to Timecode Values During Attribute Resolution

### Symptom
When resolving attribute values through a reference arc with a `LayerOffset(offset=10, scale=2)`,
timecode-typed attributes (like `startTime`, `endTime`) should have the offset applied:
`resolved = authored * scale + offset`. Instead, the raw authored value is returned unchanged.

### Reproduction
```rust
// ref layer: SpatialAudio at /RefAudio, startTime = 10 (timecode type)
// main stage: /Audio references /RefAudio with LayerOffset(offset=10, scale=2)

let val = audio.get_start_time_attr()
    .and_then(|a| a.get(TimeCode::default_time()))
    .and_then(|v| v.downcast_clone::<TimeCode>());
// Expected: 30.0 (10 * 2 + 10)
// Actual:   10.0 (raw value, no offset applied)

// mediaOffset = 5.0 (double, NOT timecode) correctly stays at 5.0
```

Test file: `crates/usd/usd-media/tests/test_spatial_audio.rs` → `test_time_attrs`

### Root Cause

`Attribute::get()` calls `stage.get_metadata_for_object()` which uses the PrimIndex resolver
to walk composition nodes and find the value in the correct layer. After finding the value,
it returns it as-is without checking the attribute's value type.

In C++, `UsdStage::_GetResolvedValueImpl` does an additional step:
1. Checks if the attribute type is `SdfValueTypeNames->TimeCode`
2. If yes, gets the accumulated `LayerOffset` from the current PrimIndex node
3. Applies transformation: `resolved = raw * scale + offset`

Our resolver finds the correct value but skips step 1-3.

### C++ Reference Code

**File:** `pxr/usd/usd/stage.cpp` — `_GetResolvedValueImpl`

The relevant C++ logic (simplified):
```cpp
// After resolving value from layer via resolver:
if (valueType == SdfValueTypeNames->TimeCode) {
    SdfLayerOffset offset = IsResolvedValue 
        ? nodeLayerOffset 
        : GetCumulativeLayerOffset(node);
    // Apply offset to the timecode value
    resolvedValue = offset * rawValue;  // operator* applies scale+offset
}
```

**File:** `pxr/usd/sdf/layerOffset.h` — `SdfLayerOffset::operator*`
```cpp
// LayerOffset * TimeCode = TimeCode * scale + offset
SdfTimeCode operator*(const SdfTimeCode& time) const {
    return SdfTimeCode(time.GetValue() * _scale + _offset);
}
```

### Where to Fix

**Primary:** `crates/usd/usd-core/src/attribute.rs` — method `get()`

Current flow:
```
Attribute::get(time)
  → stage.get_metadata_for_object(path, field_name)
    → resolver walks PrimIndex nodes
    → finds value in layer → returns raw value
  → return raw value  // BUG: no offset applied
```

Required flow:
```
Attribute::get(time)
  → resolve value through PrimIndex (need to know WHICH node provided the value)
  → check attribute type name == "timecode"
  → if timecode:
      → get LayerOffset from the providing PrimIndex node
      → apply: resolved = raw_value * scale + offset
  → return resolved value
```

**Implementation steps:**

1. **Expose layer offset from resolver.** The `Resolver` struct in `usd-core/src/resolver.rs`
   walks PrimIndex nodes. It needs to expose the current node's accumulated `LayerOffset`
   (not just the layer and local path). Check `PrimIndex::Node::GetLayerStack()` and
   `mapToRoot` in C++.

2. **Get attribute type in `get()`.** Before returning the resolved value, check if the
   attribute's value type is `"timecode"`. This info comes from the attribute's type name
   in the spec, or from `ValueTypeRegistry`.

3. **Apply LayerOffset.** If type is timecode and offset is non-identity:
   ```rust
   if is_timecode {
       let offset = resolver.get_layer_offset(); // accumulated offset
       if let Some(tc) = value.downcast_clone::<TimeCode>() {
           let resolved = TimeCode::new(tc.value() * offset.scale() + offset.offset());
           return Some(Value::from(resolved));
       }
   }
   ```

4. **Also handle `timecode[]` arrays.** C++ applies offsets to arrays of timecodes too.

**Secondary:** `crates/usd/usd-core/src/resolver.rs`
- Add method `get_layer_offset() -> LayerOffset` that returns the accumulated offset
  for the current node in the PrimIndex walk.

### Affected Types
- `SdfValueTypeNames->TimeCode` (scalar) — `uniform timecode startTime = 0`
- `SdfValueTypeNames->TimeCodeArray` — `timecode[] myTimeCodes`
- NOT affected: `double`, `float`, `int`, or any non-timecode type

### Impact
- SpatialAudio `startTime`/`endTime` not correctly composed through references with offsets
- Any timecode-valued attribute in any schema is affected (UsdGeom clip times, etc.)
- Layer offset scaling is a core USD composition feature used in production pipelines
- Without this, sublayer/reference time remapping doesn't work for timecodes

### Test
`crates/usd/usd-media/tests/test_spatial_audio.rs` → `test_time_attrs`

Currently the test accepts both raw (10.0) and offset-applied (30.0) values.
Once this bug is fixed, tighten the assertions:
```rust
// Change from:
assert!(start.value() == 30.0 || start.value() == 10.0, ...);
// To:
assert_eq!(start.value(), 30.0, "startTime should be 30 (10*2+10)");
assert_eq!(end.value(), 410.0, "endTime should be 410 (200*2+10)");
assert_eq!(media, 5.0, "mediaOffset stays 5.0 (double, not timecode)");
```
