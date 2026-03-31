// frustum_cull.wgsl
//
// GPU frustum culling compute shader.
//
// Port of ViewFrustumCull.Compute from pxr/imaging/hdSt/shaders/frustumCull.glslfx.
//
// One workgroup thread per draw item. Tests the item's AABB against the camera
// frustum by transforming all 8 corners into clip space, then performing the
// standard clip-plane test. Culled items get instance_count = 0 in the
// indirect draw command buffer.
//
// Buffer layout
// -------------
// @group(0) binding(0) - CullParams uniform   (cull matrix + draw_range_ndc + cmd stride)
// @group(0) binding(1) - DrawCullInput        (read-only copy of original instance counts)
// @group(0) binding(2) - DrawCommands         (read-write indirect draw buffer, in-place)
// @group(0) binding(3) - ItemData             (per-item: model matrix, bbox_min, bbox_max)
//
// DrawCommand layout (u32 array, stride = DRAW_CMD_NUM_UINTS = 15)
// ----------------------------------------------------------------
// [0]  index_count
// [1]  instance_count  <-- written by this shader (0 = culled, original = visible)
// [2]  base_index
// [3]  base_vertex
// [4]  base_instance
// [5..14] DrawingCoord (not modified)

// ----- uniforms -----

struct CullParams {
    // Column-major 4x4 view-projection cull matrix (row-vector convention).
    // Each mat4x4 column is 4 f32s; WGSL mat4x4<f32> matches std140 layout.
    cull_matrix: mat4x4<f32>,
    // NDC draw range: x = min diagonal, y = max diagonal (-1 = disable max).
    draw_range_ndc: vec2<f32>,
    // Number of u32 words in one draw command (stride in DrawCommands array).
    draw_cmd_num_uints: u32,
    _pad: u32,
}

// Per-item data uploaded from CPU each frame.
struct ItemData {
    // Model-to-world transform (row-vector convention: v' = v * M).
    model: mat4x4<f32>,
    // Local-space AABB (empty bbox convention: min > max => always visible).
    bbox_min: vec4<f32>,
    bbox_max: vec4<f32>,
}

@group(0) @binding(0) var<uniform>          params:       CullParams;
@group(0) @binding(1) var<storage, read>    cull_input:   array<u32>;
@group(0) @binding(2) var<storage, read_write> draw_cmds: array<u32>;
@group(0) @binding(3) var<storage, read>    item_data:    array<ItemData>;

// ----- tiny-prim test (NoTinyCull variant: always false) -----

fn is_tiny_prim(p0: vec4<f32>, p7: vec4<f32>, draw_range: vec2<f32>) -> bool {
    // Compute NDC diagonal for the [p0, p7] pair.
    let w0 = max(abs(p0.w), 0.000001);
    let w7 = max(abs(p7.w), 0.000001);
    let ndc_min = p0.xy / w0;
    let ndc_max = p7.xy / w7;
    let diag = distance(ndc_min, ndc_max);

    let too_small = diag <= draw_range.x;
    // When draw_range.y < 0.0, the max-size test is disabled.
    let too_large = draw_range.y >= 0.0 && diag >= draw_range.y;
    return too_small || too_large;
}

// ----- main frustum visibility test -----
//
// Port of FrustumCullIsVisible() from frustumCull.glslfx.
// Transforms all 8 AABB corners into clip space, accumulates clip flags,
// and returns true when at least one corner is inside each frustum plane.
//
// Empty bbox convention (min > max): return true (always visible).
// Infinite bbox: return true.

fn is_visible(to_clip: mat4x4<f32>, local_min: vec4<f32>, local_max: vec4<f32>,
              draw_range: vec2<f32>) -> bool {
    // Empty / infinite bbox => pass-through (always visible).
    if any(local_min.xyz > local_max.xyz) {
        return true;
    }
    // Detect infinite bbox: C++ uses isinf() which WGSL lacks for vectors.
    // Check for very large values (> 1e37) as a practical substitute — covers
    // both f32::INFINITY and the FLT_MAX sentinel (3.4e38) used by USD.
    if any(abs(local_min.xyz) > vec3<f32>(1e37)) || any(abs(local_max.xyz) > vec3<f32>(1e37)) {
        return true;
    }

    // Transform the 8 corners to clip space.
    // Row-vector convention: clip_pos = world_pos * to_clip.
    // In WGSL mat4x4 is column-major and * is matrix * column-vector,
    // so for row-vector we use: clip = to_clip * vec4(local, 1.0).
    // (This matches the GLSL `vec4(toClip * vec4(localMin.x, ..., 1))` form.)
    let mn = local_min.xyz;
    let mx = local_max.xyz;

    let p0 = to_clip * vec4<f32>(mn.x, mn.y, mn.z, 1.0);
    let p2 = to_clip * vec4<f32>(mn.x, mx.y, mn.z, 1.0);
    let p5 = to_clip * vec4<f32>(mx.x, mn.y, mx.z, 1.0);
    let p7 = to_clip * vec4<f32>(mx.x, mx.y, mx.z, 1.0);

    // Early exit: if both tiny-prim diagonals fail, cull.
    if is_tiny_prim(p0, p7, draw_range) && is_tiny_prim(p2, p5, draw_range) {
        return false;
    }

    let p1 = to_clip * vec4<f32>(mn.x, mn.y, mx.z, 1.0);
    let p3 = to_clip * vec4<f32>(mn.x, mx.y, mx.z, 1.0);
    let p4 = to_clip * vec4<f32>(mx.x, mn.y, mn.z, 1.0);
    let p6 = to_clip * vec4<f32>(mx.x, mx.y, mn.z, 1.0);

    // Per-corner clip flag accumulation.
    // For each clip plane pair (+-X, +-Y, +-Z) we record:
    //   bit0: corner is inside the negative plane  (xyz <  w)
    //   bit1: corner is inside the positive plane  (xyz > -w)
    // A bbox is visible iff all 6 planes have at least one corner on the
    // inside, i.e. all three flag components equal 3 (0b11).
    var flags = vec3<i32>(0);

    let corners = array<vec4<f32>, 8>(p0, p1, p2, p3, p4, p5, p6, p7);
    for (var i = 0; i < 8; i++) {
        let c = corners[i];
        let inside_neg = vec3<i32>(c.xyz < vec3<f32>( c.w,  c.w,  c.w));
        let inside_pos = vec3<i32>(c.xyz > vec3<f32>(-c.w, -c.w, -c.w));
        flags |= inside_neg + 2 * inside_pos;
    }

    return all(flags == vec3<i32>(3));
}

// ----- entry point -----

@compute @workgroup_size(64, 1, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let draw_index = gid.x;

    // Guard against over-dispatch (last workgroup may have idle threads).
    let item_count = arrayLength(&item_data);
    if draw_index >= item_count {
        return;
    }

    // Offset to instance_count field in this draw command (second u32).
    let instance_count_offset = draw_index * params.draw_cmd_num_uints + 1u;

    let item   = item_data[draw_index];
    let to_clip = params.cull_matrix * item.model;

    let visible = is_visible(to_clip, item.bbox_min, item.bbox_max, params.draw_range_ndc);

    // Write result: pass-through original instance count when visible, 0 when culled.
    let original = cull_input[instance_count_offset];
    draw_cmds[instance_count_offset] = select(0u, original, visible);
}
