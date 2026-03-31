//! Generates tests/box_sparse.bin and tests/box_sparse.glb from tests/box_sparse.gltf.
//! Run: cargo run --example gen_box_sparse_glb

use std::fs;
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let tests_dir = Path::new("tests");

    // Create box_sparse.bin: 352 bytes
    let mut bin = Vec::with_capacity(352);

    // 36 x u32 indices
    let indices: [u32; 36] = [
        0, 1, 2, 2, 3, 0, 4, 5, 6, 6, 7, 4, 0, 4, 5, 5, 1, 0, 2, 6, 7, 7, 3, 2, 0, 2, 6, 6, 4, 0,
        1, 5, 7, 7, 3, 1,
    ];
    for &i in &indices {
        bin.extend_from_slice(&i.to_le_bytes());
    }

    // 8 x vec3 positions
    let positions: [[f32; 3]; 8] = [
        [-0.5, -0.5, -0.5],
        [0.5, -0.5, -0.5],
        [0.5, 0.5, -0.5],
        [-0.5, 0.5, -0.5],
        [-0.5, -0.5, 0.5],
        [0.5, -0.5, 0.5],
        [0.5, 0.5, 0.5],
        [-0.5, 0.5, 0.5],
    ];
    for p in &positions {
        for &v in p {
            bin.extend_from_slice(&v.to_le_bytes());
        }
    }

    bin.extend_from_slice(&1u32.to_le_bytes());
    bin.extend_from_slice(&1.0f32.to_le_bytes());
    bin.extend_from_slice(&0.0f32.to_le_bytes());
    bin.extend_from_slice(&1.0f32.to_le_bytes());
    for _ in 0..24 {
        bin.extend_from_slice(&0.0f32.to_le_bytes());
    }

    while bin.len() < 352 {
        bin.push(0);
    }
    bin.truncate(352);

    fs::write(tests_dir.join("box_sparse.bin"), &bin)?;
    println!("Wrote tests/box_sparse.bin ({} bytes)", bin.len());

    let gltf_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(tests_dir.join("box_sparse.gltf"))?)?;
    let mut gltf_obj = gltf_json.as_object().unwrap().clone();
    if let Some(buffers) = gltf_obj.get_mut("buffers").and_then(|b| b.as_array_mut()) {
        if let Some(buf) = buffers.get_mut(0).and_then(|b| b.as_object_mut()) {
            buf.remove("uri");
        }
    }
    let json_str = serde_json::to_string(&gltf_obj)?;

    let mut json_bytes = json_str.into_bytes();
    while json_bytes.len() % 4 != 0 {
        json_bytes.push(0x20);
    }

    let mut bin_padded = bin.clone();
    while bin_padded.len() % 4 != 0 {
        bin_padded.push(0);
    }

    let total_len = 12 + 8 + json_bytes.len() + 8 + bin_padded.len();
    let mut glb = Vec::with_capacity(total_len);

    glb.extend_from_slice(b"glTF");
    glb.extend_from_slice(&2u32.to_le_bytes());
    glb.extend_from_slice(&(total_len as u32).to_le_bytes());
    glb.extend_from_slice(&(json_bytes.len() as u32).to_le_bytes());
    glb.extend_from_slice(b"JSON");
    glb.extend_from_slice(&json_bytes);
    glb.extend_from_slice(&(bin_padded.len() as u32).to_le_bytes());
    glb.extend_from_slice(b"BIN\0");
    glb.extend_from_slice(&bin_padded);

    fs::write(tests_dir.join("box_sparse.glb"), &glb)?;
    println!("Wrote tests/box_sparse.glb ({} bytes)", glb.len());

    Ok(())
}
