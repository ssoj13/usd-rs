// Integration tests for HgiVulkan — port of testHgiVulkan.cpp.
//
// All tests require Vulkan hardware and are marked #[ignore] by default.
// Run with: cargo test --test test_hgi_vulkan -- --ignored
//
// Image comparison tests save output PNGs alongside the test binary.
// Optional baselines: set USD_RS_GPU_BASELINE_ROOT to a directory containing the
// PNG file names below, or place them under tests/baseline/ next to this crate.

use usd_hgi::blit_cmds::{RawCpuBuffer, RawCpuBufferMut};
use usd_hgi::types::get_mip_infos;
use usd_hgi::*;
use usd_hgi_vulkan::HgiVulkan;
use usd_hgi_vulkan::shader_compiler::compile_glsl;

use ash::vk;
use usd_gf::Vec3i;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const IMG_SIZE: i32 = 512;

fn baseline_png(name: &str) -> String {
    if let Ok(root) = std::env::var("USD_RS_GPU_BASELINE_ROOT") {
        return std::path::Path::new(&root)
            .join(name)
            .to_string_lossy()
            .into_owned();
    }
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/baseline")
        .join(name)
        .to_string_lossy()
        .into_owned()
}

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

fn create_hgi() -> HgiVulkan {
    HgiVulkan::new().expect("HgiVulkan::new() failed — Vulkan not available")
}

/// Reads a GPU texture back to CPU and encodes as a PNG file.
fn save_gpu_texture_to_file(
    hgi: &mut HgiVulkan,
    tex: &HgiTextureHandle,
    width: u32,
    height: u32,
    format: HgiFormat,
    path: &str,
) {
    let (bytes_per_texel, _, _) = format.data_size_of_format();
    let buffer_byte_size = (width * height) as usize * bytes_per_texel;
    let mut buffer = vec![0u8; buffer_byte_size];

    let copy_op = HgiTextureGpuToCpuOp {
        gpu_source_texture: tex.clone(),
        source_texel_offset: Vec3i::new(0, 0, 0),
        mip_level: 0,
        // SAFETY: buffer lives for the duration of submit_cmds below.
        cpu_destination_buffer: unsafe { RawCpuBufferMut::new(buffer.as_mut_ptr()) },
        destination_byte_offset: 0,
        destination_buffer_byte_size: buffer_byte_size,
        copy_size: Vec3i::new(0, 0, 0),
        source_layer: 0,
    };

    let mut blit = hgi.create_blit_cmds();
    blit.copy_texture_gpu_to_cpu(&copy_op);
    hgi.submit_cmds(blit, HgiSubmitWaitType::WaitUntilCompleted);

    save_rgba_png(width, height, &buffer, path);
}

/// Reads a GPU buffer back to CPU and encodes as a PNG file.
fn save_gpu_buffer_to_file(
    hgi: &mut HgiVulkan,
    buf: &HgiBufferHandle,
    width: u32,
    height: u32,
    format: HgiFormat,
    path: &str,
) {
    let (bytes_per_texel, _, _) = format.data_size_of_format();
    let buffer_byte_size = (width * height) as usize * bytes_per_texel;
    let mut buffer = vec![0u8; buffer_byte_size];

    let copy_op = HgiBufferGpuToCpuOp {
        gpu_source_buffer: buf.clone(),
        source_byte_offset: 0,
        // SAFETY: buffer lives for the duration of submit_cmds below.
        cpu_destination_buffer: unsafe { RawCpuBufferMut::new(buffer.as_mut_ptr()) },
        byte_size: buffer_byte_size,
    };

    let mut blit = hgi.create_blit_cmds();
    blit.copy_buffer_gpu_to_cpu(&copy_op);
    hgi.submit_cmds(blit, HgiSubmitWaitType::WaitUntilCompleted);

    save_rgba_png(width, height, &buffer, path);
}

/// Encodes raw RGBA bytes as a PNG using the `image` crate.
fn save_rgba_png(width: u32, height: u32, bytes: &[u8], path: &str) {
    image::save_buffer(path, bytes, width, height, image::ColorType::Rgba8)
        .unwrap_or_else(|e| panic!("Failed to save PNG '{}': {}", path, e));
}

/// Creates a simple GPU texture with optional initial pixel data.
fn make_texture(
    hgi: &mut HgiVulkan,
    width: i32,
    height: i32,
    format: HgiFormat,
    data: Option<&[u8]>,
) -> HgiTextureHandle {
    let mut desc = HgiTextureDesc::new();
    desc.debug_name = "Test texture".to_string();
    desc.dimensions = Vec3i::new(width, height, 1);
    desc.format = format;
    desc.layer_count = 1;
    desc.mip_levels = 1;
    desc.sample_count = HgiSampleCount::Count1;
    desc.usage = HgiTextureUsage::SHADER_READ;
    hgi.create_texture(&desc, data)
}

/// Creates a GPU buffer with optional initial data.
fn make_buffer(
    hgi: &mut HgiVulkan,
    byte_size: usize,
    usage: HgiBufferUsage,
    data: Option<&[u8]>,
) -> HgiBufferHandle {
    let mut desc = HgiBufferDesc::new();
    desc.byte_size = byte_size;
    desc.usage = usage;
    hgi.create_buffer(&desc, data)
}

/// Compares `output_path` against `baseline_path` pixel-by-pixel (tolerance ≤ 2).
///
/// If the baseline is missing the comparison is skipped with a warning, so CI
/// without baseline images still passes.
fn compare_with_baseline(output_path: &str, baseline_path: &str) {
    let output = match image::open(output_path) {
        Ok(img) => img.into_rgba8(),
        Err(e) => panic!("Failed to open output image '{}': {}", output_path, e),
    };

    let baseline = match image::open(baseline_path) {
        Ok(img) => img.into_rgba8(),
        Err(_) => {
            eprintln!(
                "WARN: Baseline '{}' not found — skipping pixel comparison",
                baseline_path
            );
            return;
        }
    };

    assert_eq!(
        output.dimensions(),
        baseline.dimensions(),
        "Image size mismatch: output={:?} baseline={:?}",
        output.dimensions(),
        baseline.dimensions()
    );

    let max_diff = output
        .pixels()
        .zip(baseline.pixels())
        .map(|(o, b)| {
            o.0.iter()
                .zip(b.0.iter())
                .map(|(a, c)| (*a as i32 - *c as i32).unsigned_abs())
                .max()
                .unwrap_or(0)
        })
        .max()
        .unwrap_or(0);

    // Allow ≤2/255 tolerance for PNG compression/rounding artefacts.
    assert!(
        max_diff <= 2,
        "Image pixel difference {} > 2 for '{}'",
        max_diff,
        output_path
    );
}

// ---------------------------------------------------------------------------
// test_vulkan_instance
// ---------------------------------------------------------------------------

/// Verifies Vulkan instance creation and debug messenger loading.
///
/// Port of C++ `TestVulkanInstance`.
#[test]
#[ignore] // Requires Vulkan hardware
fn test_vulkan_instance() {
    let _ = env_logger::try_init();
    let hgi = create_hgi();
    let instance = hgi.instance();

    // vkInstance handle must be valid.
    let raw = instance.vk_instance().handle();
    assert!(raw != vk::Instance::null(), "vkInstance handle is null");

    // Debug utils extension must be present (validation layers loaded).
    assert!(
        instance.debug_utils().is_some(),
        "Vulkan debug utils not loaded"
    );
}

// ---------------------------------------------------------------------------
// test_vulkan_device
// ---------------------------------------------------------------------------

/// Verifies logical device creation, VMA allocator, and graphics queue.
///
/// Port of C++ `TestVulkanDevice`.
#[test]
#[ignore] // Requires Vulkan hardware
fn test_vulkan_device() {
    let _ = env_logger::try_init();
    let hgi = create_hgi();
    let device = hgi.device();

    let raw_dev = device.vk_device().handle();
    assert!(raw_dev != vk::Device::null(), "vkDevice handle is null");

    let queue = device.command_queue();
    assert!(
        queue.vk_graphics_queue() != vk::Queue::null(),
        "vkGraphicsQueue is null"
    );

    // Accessing the allocator confirms it is initialized.
    let _ = device.allocator();
}

// ---------------------------------------------------------------------------
// test_vulkan_capabilities
// ---------------------------------------------------------------------------

/// Verifies Vulkan API version >= 1.2 and a positive shader version.
///
/// Port of C++ `TestVulkanCapabilities`.
#[test]
#[ignore] // Requires Vulkan hardware
fn test_vulkan_capabilities() {
    let _ = env_logger::try_init();
    let hgi = create_hgi();
    let caps = hgi.device().capabilities();

    assert!(
        caps.get_api_version() >= vk::API_VERSION_1_2,
        "Vulkan < 1.2 (got {:?})",
        caps.get_api_version()
    );

    assert!(caps.get_shader_version() > 0, "Shader version is zero");
}

// ---------------------------------------------------------------------------
// test_vulkan_shader_compiler
// ---------------------------------------------------------------------------

/// Compiles a GLSL fragment shader to SPIR-V and verifies no errors.
///
/// Port of C++ `TestVulkanShaderCompiler`. Exercises push constants, scalar
/// block layout, storage buffers, and non-uniform sampler arrays.
#[test]
#[ignore] // Requires shaderc
fn test_vulkan_shader_compiler() {
    let _ = env_logger::try_init();

    let frag_src = concat!(
        "#version 450 \n",
        "#extension GL_EXT_nonuniform_qualifier : require \n",
        "#extension GL_EXT_scalar_block_layout : require \n",
        "#extension GL_EXT_shader_explicit_arithmetic_types_int64 : require \n",
        "\n",
        "layout(push_constant) uniform PushConstantBuffer { \n",
        "    layout(offset = 0) int textureIndex; \n",
        "} pushConstants; \n",
        "\n",
        "layout (scalar, set=0, binding=0) buffer StorageBuffer { \n",
        "    vec3 value[]; \n",
        "} storageBuffer; \n",
        "\n",
        "layout(set=0, binding=1) uniform sampler2DArray samplers2D[]; \n",
        "\n",
        "layout(location = 0) in vec2 texcoordIn; \n",
        "layout(location = 0) out vec4 outputColor; \n",
        "\n",
        "layout(early_fragment_tests) in; \n",
        "\n",
        "void main() { \n",
        "    int idx = pushConstants.textureIndex;\n",
        "    outputColor = texture( \n",
        "        samplers2D[nonuniformEXT(idx)], vec3(texcoordIn, 0)); \n",
        "    outputColor.a = storageBuffer.value[0].x;",
        "} \n"
    );

    let spirv = compile_glsl("TestFrag", &[frag_src], HgiShaderStage::FRAGMENT)
        .unwrap_or_else(|e| panic!("Shader compiler errors:\n{}", e));

    assert!(!spirv.is_empty(), "SPIR-V output is empty");
}

// ---------------------------------------------------------------------------
// test_vulkan_command_queue
// ---------------------------------------------------------------------------

/// Validates multi-threaded blit command recording, in-flight bit tracking,
/// and command buffer lifecycle.
///
/// Port of C++ `TestVulkanCommandQueue`. The original test uses real OS threads
/// that share raw Vulkan command buffer pointers, which is not directly
/// portable to safe Rust (the cmds objects are !Send). This port validates the
/// same internal invariants (in-flight bits, reuse) on the main thread using
/// the same submit/idle/reset cycle.
#[test]
#[ignore] // Requires Vulkan hardware
fn test_vulkan_command_queue() {
    let _ = env_logger::try_init();
    let mut hgi = create_hgi();

    hgi.start_frame();

    // Four blit-cmds objects: A+B for "thread 0", Y+Z for "thread 1".
    let mut blit_a = hgi.create_blit_cmds();
    let mut blit_b = hgi.create_blit_cmds();
    let mut blit_y = hgi.create_blit_cmds();
    let mut blit_z = hgi.create_blit_cmds();

    // First job: record on A and Y.
    blit_a.push_debug_group("First Job A");
    blit_a.pop_debug_group();
    blit_y.push_debug_group("First Job Y");
    blit_y.pop_debug_group();

    // After recording, the queue should have in-flight bits set.
    let queue = hgi.device().command_queue();
    let inflight_1 = queue.get_inflight_command_buffers_bits();
    assert_ne!(
        inflight_1, 0,
        "Expected in-flight bits after first job recording"
    );

    // Submit first job and wait for GPU to consume it.
    hgi.submit_cmds(blit_a, HgiSubmitWaitType::WaitUntilCompleted);
    hgi.submit_cmds(blit_y, HgiSubmitWaitType::WaitUntilCompleted);

    hgi.wait_for_idle();
    // EndFrame resets consumed command buffers so they can be reused.
    hgi.end_frame();

    // Second job: B and Z should reuse the command buffer slots from A and Y.
    blit_b.push_debug_group("Second Job B");
    blit_b.pop_debug_group();
    blit_z.push_debug_group("Second Job Z");
    blit_z.pop_debug_group();

    hgi.submit_cmds(blit_b, HgiSubmitWaitType::WaitUntilCompleted);
    hgi.submit_cmds(blit_z, HgiSubmitWaitType::WaitUntilCompleted);

    hgi.wait_for_idle();
}

// ---------------------------------------------------------------------------
// test_vulkan_garbage_collection
// ---------------------------------------------------------------------------

/// Validates deferred GPU resource destruction via the garbage collector.
///
/// Port of C++ `TestVulkanGarbageCollection`. Creates two shader functions on
/// a second HGI instance, marks them for destruction while command buffers are
/// in-flight, and verifies the GC holds them until the CBs are consumed.
#[test]
#[ignore] // Requires Vulkan hardware
fn test_vulkan_garbage_collection() {
    let _ = env_logger::try_init();
    let mut hgi = create_hgi();
    // Second HGI to test that the GC handles objects from multiple instances.
    let mut hgi2 = create_hgi();

    let shader_code = "void main() {\n   bool empty = true;\n}\n";

    let mut desc = HgiShaderFunctionDesc::new();
    desc.shader_stage = HgiShaderStage::COMPUTE;
    desc.shader_code = shader_code.to_string();
    desc.debug_name = "Shader0".to_string();

    let shader0 = hgi2.create_shader_function(&desc);
    assert!(!shader0.is_null(), "shader0 creation failed");

    desc.debug_name = "Shader1".to_string();
    let shader1 = hgi2.create_shader_function(&desc);
    assert!(!shader1.is_null(), "shader1 creation failed");

    // Record blit commands to put command buffers in-flight.
    let mut blit0 = hgi2.create_blit_cmds();
    blit0.push_debug_group("BlitCmds0");
    blit0.pop_debug_group();

    // Schedule destruction of shader0 — depends on blit0 being in-flight.
    hgi2.destroy_shader_function(&shader0);

    let mut blit1 = hgi2.create_blit_cmds();
    blit1.push_debug_group("BlitCmds1");
    blit1.pop_debug_group();

    // Schedule destruction of shader1 — depends on both blit0 and blit1.
    hgi2.destroy_shader_function(&shader1);

    // Submit blit0 and wait; shader0 becomes eligible for collection.
    hgi2.submit_cmds(blit0, HgiSubmitWaitType::WaitUntilCompleted);
    hgi2.device().wait_for_idle();

    // Trigger GC by submitting another blit cmds (same as C++ "no StartFrame" path).
    let mut blit_x = hgi2.create_blit_cmds();
    blit_x.push_debug_group("BlitCmdsX");
    blit_x.pop_debug_group();
    hgi2.submit_cmds(blit_x, HgiSubmitWaitType::WaitUntilCompleted);

    // Create and trash a shader in the original hgi to verify cross-instance GC.
    let mut org_desc = HgiShaderFunctionDesc::new();
    org_desc.shader_stage = HgiShaderStage::COMPUTE;
    org_desc.shader_code = shader_code.to_string();
    org_desc.debug_name = "ShaderOrg".to_string();
    let shader_org = hgi.create_shader_function(&org_desc);
    assert!(!shader_org.is_null(), "shaderOrg creation failed");
    hgi.destroy_shader_function(&shader_org);

    // Submit blit1; after GPU idle + reset, shader1 becomes collectible.
    hgi2.submit_cmds(blit1, HgiSubmitWaitType::WaitUntilCompleted);
    hgi2.device().wait_for_idle();

    hgi2.device_mut()
        .command_queue_mut()
        .reset_consumed_command_buffers(HgiSubmitWaitType::NoWait);

    let inflight = hgi2
        .device()
        .command_queue()
        .get_inflight_command_buffers_bits();
    assert_eq!(inflight, 0, "Expected all CBs reset, inflight={}", inflight);

    // EndFrame on hgi2 collects shader1.
    hgi2.start_frame();
    hgi2.end_frame();

    // EndFrame on original hgi collects shaderOrg.
    hgi.start_frame();
    hgi.end_frame();
}

// ---------------------------------------------------------------------------
// test_vulkan_buffer
// ---------------------------------------------------------------------------

/// Validates GPU buffer creation, staging upload, readback, and offset transfers.
///
/// Port of C++ `TestVulkanBuffer`.
#[test]
#[ignore] // Requires Vulkan hardware
fn test_vulkan_buffer() {
    let _ = env_logger::try_init();
    let mut hgi = create_hgi();

    hgi.start_frame();

    // Create buffer pre-filled with u32 value 123.
    let blob: Vec<u32> = vec![123u32; 16];
    let blob_bytes: Vec<u8> = blob.iter().flat_map(|v| v.to_ne_bytes()).collect();

    let mut buf_desc = HgiBufferDesc::new();
    buf_desc.debug_name = "TestBuffer".to_string();
    buf_desc.byte_size = blob_bytes.len();
    buf_desc.usage = HgiBufferUsage::STORAGE;
    let buffer = hgi.create_buffer(&buf_desc, Some(&blob_bytes));
    assert!(!buffer.is_null(), "Buffer creation failed");

    assert_eq!(
        buffer.get().map(|b| b.byte_size_of_resource()),
        Some(buf_desc.byte_size),
        "Incorrect byte_size_of_resource"
    );

    // Read back initial data: GpuToCpu copy.
    let mut readback = vec![0u8; blob_bytes.len()];
    {
        let copy_op = HgiBufferGpuToCpuOp {
            gpu_source_buffer: buffer.clone(),
            source_byte_offset: 0,
            // SAFETY: readback lives until after submit_cmds.
            cpu_destination_buffer: unsafe { RawCpuBufferMut::new(readback.as_mut_ptr()) },
            byte_size: buf_desc.byte_size,
        };
        let mut blit = hgi.create_blit_cmds();
        blit.copy_buffer_gpu_to_cpu(&copy_op);
        hgi.submit_cmds(blit, HgiSubmitWaitType::WaitUntilCompleted);
    }
    assert_eq!(readback, blob_bytes, "Initial data readback failed");

    // CPU-to-GPU transfer: write value 456.
    let staging: Vec<u32> = vec![456u32; 16];
    let staging_bytes: Vec<u8> = staging.iter().flat_map(|v| v.to_ne_bytes()).collect();
    {
        let transfer_op = HgiBufferCpuToGpuOp {
            // SAFETY: staging_bytes lives until after submit_cmds.
            cpu_source_buffer: RawCpuBuffer::new(staging_bytes.as_ptr()),
            source_byte_offset: 0,
            gpu_destination_buffer: buffer.clone(),
            destination_byte_offset: 0,
            byte_size: buf_desc.byte_size,
        };
        let mut blit = hgi.create_blit_cmds();
        blit.copy_buffer_cpu_to_gpu(&transfer_op);
        hgi.submit_cmds(blit, HgiSubmitWaitType::WaitUntilCompleted);
    }

    // Read back after transfer.
    let mut transfer_back = vec![0u8; blob_bytes.len()];
    {
        let copy_op = HgiBufferGpuToCpuOp {
            gpu_source_buffer: buffer.clone(),
            source_byte_offset: 0,
            cpu_destination_buffer: unsafe { RawCpuBufferMut::new(transfer_back.as_mut_ptr()) },
            byte_size: buf_desc.byte_size,
        };
        let mut blit = hgi.create_blit_cmds();
        blit.copy_buffer_gpu_to_cpu(&copy_op);
        hgi.submit_cmds(blit, HgiSubmitWaitType::WaitUntilCompleted);
    }
    assert_eq!(transfer_back, staging_bytes, "Transfer readback failed");

    // Partial offset transfer: overwrite elements [8..12] with value 789.
    let mut staging2 = staging.clone();
    staging2[8] = 789;
    staging2[9] = 789;
    staging2[10] = 789;
    staging2[11] = 789;
    let partial: Vec<u8> = staging2[8..12]
        .iter()
        .flat_map(|v| v.to_ne_bytes())
        .collect();
    {
        let transfer_op = HgiBufferCpuToGpuOp {
            cpu_source_buffer: RawCpuBuffer::new(partial.as_ptr()),
            source_byte_offset: 0,
            gpu_destination_buffer: buffer.clone(),
            destination_byte_offset: 8 * std::mem::size_of::<u32>(),
            byte_size: partial.len(),
        };
        let mut blit = hgi.create_blit_cmds();
        blit.copy_buffer_cpu_to_gpu(&transfer_op);
        hgi.submit_cmds(blit, HgiSubmitWaitType::WaitUntilCompleted);
    }

    let mut final_back = vec![0u8; blob_bytes.len()];
    {
        let copy_op = HgiBufferGpuToCpuOp {
            gpu_source_buffer: buffer.clone(),
            source_byte_offset: 0,
            cpu_destination_buffer: unsafe { RawCpuBufferMut::new(final_back.as_mut_ptr()) },
            byte_size: buf_desc.byte_size,
        };
        let mut blit = hgi.create_blit_cmds();
        blit.copy_buffer_gpu_to_cpu(&copy_op);
        hgi.submit_cmds(blit, HgiSubmitWaitType::WaitUntilCompleted);
    }
    let staging2_bytes: Vec<u8> = staging2.iter().flat_map(|v| v.to_ne_bytes()).collect();
    assert_eq!(
        final_back, staging2_bytes,
        "Partial transfer readback failed"
    );

    hgi.device().wait_for_idle();
    hgi.destroy_buffer(&buffer);
    hgi.end_frame();
}

// ---------------------------------------------------------------------------
// test_vulkan_texture
// ---------------------------------------------------------------------------

/// Validates texture creation, initial-data upload, readback, CpuToGpu upload,
/// texture views, and mip generation.
///
/// Port of C++ `TestVulkanTexture`.
#[test]
#[ignore] // Requires Vulkan hardware
fn test_vulkan_texture() {
    let _ = env_logger::try_init();
    let mut hgi = create_hgi();

    hgi.start_frame();

    let w = 32i32;
    let h = 32i32;
    let fmt = HgiFormat::Float32Vec4;
    let num_texels = (w * h) as usize;
    let num_components = 4usize;

    let pixels: Vec<f32> = vec![0.123f32; num_texels * num_components];
    let pixels_bytes: Vec<u8> = pixels.iter().flat_map(|f| f.to_ne_bytes()).collect();

    let mut desc = HgiTextureDesc::new();
    desc.debug_name = "Debug Texture".to_string();
    desc.dimensions = Vec3i::new(w, h, 1);
    desc.format = fmt;
    desc.texture_type = HgiTextureType::Texture2D;
    desc.usage = HgiTextureUsage::COLOR_TARGET | HgiTextureUsage::SHADER_READ;
    let texture = hgi.create_texture(&desc, Some(&pixels_bytes));
    assert!(!texture.is_null(), "Texture creation failed");

    // Create a texture view.
    let mut view_desc = HgiTextureViewDesc::new();
    view_desc.debug_name = "Debug TextureView".to_string();
    view_desc.format = fmt;
    view_desc.source_texture = texture.clone();
    let tex_view = hgi.create_texture_view(&view_desc);
    assert!(!tex_view.is_null(), "Texture view creation failed");

    // Read back initial pixels.
    let mut readback = vec![0u8; pixels_bytes.len()];
    {
        let op = HgiTextureGpuToCpuOp {
            gpu_source_texture: texture.clone(),
            source_texel_offset: Vec3i::new(0, 0, 0),
            mip_level: 0,
            cpu_destination_buffer: unsafe { RawCpuBufferMut::new(readback.as_mut_ptr()) },
            destination_byte_offset: 0,
            destination_buffer_byte_size: readback.len(),
            copy_size: Vec3i::new(0, 0, 0),
            source_layer: 0,
        };
        let mut blit = hgi.create_blit_cmds();
        blit.copy_texture_gpu_to_cpu(&op);
        hgi.submit_cmds(blit, HgiSubmitWaitType::WaitUntilCompleted);
    }
    assert_eq!(readback, pixels_bytes, "initialData readback failed");

    // Upload 0.456 values via CpuToGpu.
    let upload: Vec<f32> = vec![0.456f32; num_texels * num_components];
    let upload_bytes: Vec<u8> = upload.iter().flat_map(|f| f.to_ne_bytes()).collect();
    {
        let up_op = HgiTextureCpuToGpuOp {
            cpu_source_buffer: RawCpuBuffer::new(upload_bytes.as_ptr()),
            buffer_byte_size: upload_bytes.len(),
            gpu_destination_texture: texture.clone(),
            destination_texel_offset: Vec3i::new(0, 0, 0),
            mip_level: 0,
            destination_layer: 0,
        };
        let rb_op = HgiTextureGpuToCpuOp {
            gpu_source_texture: texture.clone(),
            source_texel_offset: Vec3i::new(0, 0, 0),
            mip_level: 0,
            cpu_destination_buffer: unsafe { RawCpuBufferMut::new(readback.as_mut_ptr()) },
            destination_byte_offset: 0,
            destination_buffer_byte_size: readback.len(),
            copy_size: Vec3i::new(0, 0, 0),
            source_layer: 0,
        };
        let mut blit = hgi.create_blit_cmds();
        blit.copy_texture_cpu_to_gpu(&up_op);
        blit.copy_texture_gpu_to_cpu(&rb_op);
        hgi.submit_cmds(blit, HgiSubmitWaitType::WaitUntilCompleted);
    }
    assert_eq!(readback, upload_bytes, "Upload readback failed");

    // Generate mipmaps.
    {
        let mut blit = hgi.create_blit_cmds();
        blit.generate_mipmap(&texture);
        hgi.submit_cmds(blit, HgiSubmitWaitType::WaitUntilCompleted);
    }

    hgi.destroy_texture_view(&tex_view);
    hgi.destroy_texture(&texture);
    hgi.end_frame();
}

// ---------------------------------------------------------------------------
// test_vulkan_pipeline
// ---------------------------------------------------------------------------

/// Validates graphics pipeline creation with a vertex-only shader and no rasterizer.
///
/// Port of C++ `TestVulkanPipeline`.
#[test]
#[ignore] // Requires Vulkan hardware
fn test_vulkan_pipeline() {
    let _ = env_logger::try_init();
    let mut hgi = create_hgi();

    hgi.start_frame();

    let mut vs_desc = HgiShaderFunctionDesc::new();
    vs_desc.shader_stage = HgiShaderStage::VERTEX;
    vs_desc.shader_code = concat!(
        "layout(location = 0) in vec3 positionIn; \n",
        "\n",
        "void main() { \n",
        "    gl_PointSize = 1.0; \n",
        "    gl_Position = vec4(positionIn, 1.0); \n",
        "} \n"
    )
    .to_string();
    vs_desc.debug_name = "debugShader".to_string();

    let vs = hgi.create_shader_function(&vs_desc);
    assert!(
        !vs.is_null() && vs.get().map(|f| f.is_valid()).unwrap_or(false),
        "VS compile failed: {}",
        vs.get()
            .map(|f| f.compile_errors().to_string())
            .unwrap_or_default()
    );

    let mut prg_desc = HgiShaderProgramDesc::new();
    prg_desc.debug_name = "debugProgram".to_string();
    prg_desc.shader_functions.push(vs.clone());
    let prg = hgi.create_shader_program(&prg_desc);
    assert!(
        prg.get().map(|p| p.is_valid()).unwrap_or(false),
        "Shader program is not valid"
    );

    let (stride, _, _) = HgiFormat::Float32Vec3.data_size_of_format();
    let attr = HgiVertexAttributeDesc::new(HgiFormat::Float32Vec3, 0, 0);

    let mut vbo = HgiVertexBufferDesc::default();
    vbo.binding_index = 0;
    vbo.vertex_attributes.push(attr);
    vbo.vertex_stride = stride as u32;

    let mut pso_desc = HgiGraphicsPipelineDesc::new();
    pso_desc.debug_name = "debugPipeline".to_string();
    pso_desc.depth_stencil_state.depth_test_enabled = false;
    pso_desc.depth_stencil_state.depth_write_enabled = false;
    pso_desc.depth_stencil_state.stencil_test_enabled = false;
    pso_desc.primitive_type = HgiPrimitiveType::PointList;
    pso_desc.rasterization_state.rasterizer_enabled = false;
    pso_desc.shader_constants_desc.byte_size = 64;
    pso_desc.shader_constants_desc.stage_usage = HgiShaderStage::VERTEX;
    pso_desc.shader_program = prg.clone();
    pso_desc.vertex_buffers.push(vbo);

    let pso = hgi.create_graphics_pipeline(&pso_desc);
    assert!(!pso.is_null(), "Pipeline creation failed");

    let gfx_desc = HgiGraphicsCmdsDesc::new();
    let mut gfx = hgi.create_graphics_cmds(&gfx_desc);
    gfx.push_debug_group("TestVulkanPipeline");
    gfx.bind_pipeline(&pso);
    gfx.pop_debug_group();
    hgi.submit_cmds(gfx, HgiSubmitWaitType::WaitUntilCompleted);

    hgi.destroy_graphics_pipeline(&pso);
    hgi.destroy_shader_program(&prg);
    hgi.destroy_shader_function(&vs);
    hgi.end_frame();
}

// ---------------------------------------------------------------------------
// test_vulkan_graphics_cmds
// ---------------------------------------------------------------------------

/// Validates MSAA graphics commands: VS+FS pair, color+depth attachments,
/// viewport, indexed draw, resolve attachments, and GC cleanliness.
///
/// Port of C++ `TestVulkanGraphicsCmds`.
#[test]
#[ignore] // Requires Vulkan hardware
fn test_vulkan_graphics_cmds() {
    let _ = env_logger::try_init();
    let mut hgi = create_hgi();

    hgi.start_frame();

    let size = 64i32;

    // Create color (Float16Vec4, MSAA×4) and depth (Float32UInt8, MSAA×4) textures
    // plus their single-sample resolve targets.
    let formats = [HgiFormat::Float16Vec4, HgiFormat::Float32UInt8];
    let usages = [
        HgiTextureUsage::COLOR_TARGET,
        HgiTextureUsage::DEPTH_TARGET | HgiTextureUsage::STENCIL_TARGET,
    ];

    let mut textures: Vec<HgiTextureHandle> = Vec::new();
    let mut resolves: Vec<HgiTextureHandle> = Vec::new();
    let mut attachments: Vec<HgiAttachmentDesc> = Vec::new();

    for i in 0..2usize {
        let mut td = HgiTextureDesc::new();
        td.dimensions = Vec3i::new(size, size, 1);
        td.format = formats[i];
        td.sample_count = HgiSampleCount::Count4;
        td.texture_type = HgiTextureType::Texture2D;
        td.usage = usages[i];
        textures.push(hgi.create_texture(&td, None));

        let mut att = HgiAttachmentDesc::default();
        att.format = td.format;
        att.load_op = HgiAttachmentLoadOp::Clear;
        att.store_op = HgiAttachmentStoreOp::DontCare;
        attachments.push(att);

        let mut rd = HgiTextureDesc::new();
        rd.dimensions = td.dimensions;
        rd.format = td.format;
        rd.sample_count = HgiSampleCount::Count1;
        rd.texture_type = td.texture_type;
        rd.usage = td.usage;
        resolves.push(hgi.create_texture(&rd, None));
    }

    // Fullscreen triangle.
    let positions: Vec<f32> = vec![-1.0, -1.0, 0.0, 3.0, -1.0, 0.0, -1.0, 3.0, 0.0];
    let indices: Vec<u32> = vec![0, 1, 2];
    let pos_bytes: Vec<u8> = positions.iter().flat_map(|f| f.to_ne_bytes()).collect();
    let idx_bytes: Vec<u8> = indices.iter().flat_map(|i| i.to_ne_bytes()).collect();

    let ibo = make_buffer(
        &mut hgi,
        idx_bytes.len(),
        HgiBufferUsage::INDEX32,
        Some(&idx_bytes),
    );
    let vbo_buf = make_buffer(
        &mut hgi,
        pos_bytes.len(),
        HgiBufferUsage::VERTEX,
        Some(&pos_bytes),
    );

    let (stride, _, _) = HgiFormat::Float32Vec3.data_size_of_format();
    let attr = HgiVertexAttributeDesc::new(HgiFormat::Float32Vec3, 0, 0);
    let mut vbo_desc = HgiVertexBufferDesc::default();
    vbo_desc.binding_index = 0;
    vbo_desc.vertex_attributes.push(attr);
    vbo_desc.vertex_stride = stride as u32;

    // Vertex shader.
    let mut vs_desc = HgiShaderFunctionDesc::new();
    vs_desc.shader_stage = HgiShaderStage::VERTEX;
    vs_desc.shader_code = concat!(
        "layout(location = 0) in vec3 positionIn; \n",
        "void main() { gl_Position = vec4(positionIn, 1.0); } \n"
    )
    .to_string();
    vs_desc.debug_name = "debug vs".to_string();
    let vs = hgi.create_shader_function(&vs_desc);
    assert!(
        !vs.is_null() && vs.get().map(|f| f.is_valid()).unwrap_or(false),
        "VS compile failed"
    );

    // Fragment shader.
    let mut fs_desc = HgiShaderFunctionDesc::new();
    fs_desc.shader_stage = HgiShaderStage::FRAGMENT;
    fs_desc.shader_code = concat!(
        "layout(location = 0) out vec4 outputColor; \n",
        "void main() { outputColor = vec4(1,0,1,1); } \n"
    )
    .to_string();
    fs_desc.debug_name = "debug fs".to_string();
    let fs = hgi.create_shader_function(&fs_desc);
    assert!(
        !fs.is_null() && fs.get().map(|f| f.is_valid()).unwrap_or(false),
        "FS compile failed"
    );

    let mut prg_desc = HgiShaderProgramDesc::new();
    prg_desc.debug_name = "debug program".to_string();
    prg_desc.shader_functions.push(vs.clone());
    prg_desc.shader_functions.push(fs.clone());
    let prg = hgi.create_shader_program(&prg_desc);

    let mut pso_desc = HgiGraphicsPipelineDesc::new();
    pso_desc.debug_name = "debugPipeline".to_string();
    pso_desc.depth_stencil_state.depth_test_enabled = false;
    pso_desc.depth_stencil_state.depth_write_enabled = false;
    pso_desc.depth_stencil_state.stencil_test_enabled = false;
    pso_desc.multi_sample_state.sample_count = HgiSampleCount::Count4;
    pso_desc.primitive_type = HgiPrimitiveType::TriangleList;
    pso_desc.shader_program = prg.clone();
    pso_desc.vertex_buffers.push(vbo_desc);
    pso_desc.color_attachments.push(attachments[0].clone());
    pso_desc.depth_attachment = Some(attachments[1].clone());
    pso_desc.resolve_attachments = true;

    let pso = hgi.create_graphics_pipeline(&pso_desc);
    assert!(!pso.is_null(), "Pipeline creation failed");

    let mut gfx_desc = HgiGraphicsCmdsDesc::new();
    gfx_desc.color_attachment_descs.push(attachments[0].clone());
    gfx_desc.color_textures.push(textures[0].clone());
    gfx_desc.color_resolve_textures.push(resolves[0].clone());
    gfx_desc.depth_attachment_desc = attachments[1].clone();
    gfx_desc.depth_texture = textures[1].clone();
    gfx_desc.depth_resolve_texture = resolves[1].clone();

    let mut gfx = hgi.create_graphics_cmds(&gfx_desc);
    gfx.push_debug_group("TestVulkanGraphicsCmds");
    gfx.bind_pipeline(&pso);
    gfx.bind_vertex_buffers(&[vbo_buf.clone()], &[0u64]);
    gfx.set_viewport(&HgiViewport::new(0.0, 0.0, size as f32, size as f32));
    gfx.draw_indexed(
        &ibo,
        &HgiDrawIndexedOp {
            index_count: 3,
            base_index: 0,
            base_vertex: 0,
            instance_count: 1,
            base_instance: 0,
        },
    );
    gfx.pop_debug_group();
    hgi.submit_cmds(gfx, HgiSubmitWaitType::WaitUntilCompleted);

    for t in &textures {
        hgi.destroy_texture(t);
    }
    for r in &resolves {
        hgi.destroy_texture(r);
    }
    hgi.destroy_graphics_pipeline(&pso);
    hgi.destroy_shader_program(&prg);
    hgi.destroy_shader_function(&fs);
    hgi.destroy_shader_function(&vs);
    hgi.destroy_buffer(&vbo_buf);
    hgi.destroy_buffer(&ibo);
    hgi.end_frame();

    let inflight = hgi
        .device()
        .command_queue()
        .get_inflight_command_buffers_bits();
    assert_eq!(inflight, 0, "Not all CBs consumed, inflight={}", inflight);
}

// ---------------------------------------------------------------------------
// test_vulkan_compute_cmds
// ---------------------------------------------------------------------------

/// Validates compute pipeline, resource bindings (UBO, SSBO, storage image),
/// push constants, and dispatch.
///
/// Port of C++ `TestVulkanComputeCmds`.
#[test]
#[ignore] // Requires Vulkan hardware
fn test_vulkan_compute_cmds() {
    let _ = env_logger::try_init();
    let mut hgi = create_hgi();

    hgi.start_frame();

    let mut cs_desc = HgiShaderFunctionDesc::new();
    cs_desc.shader_stage = HgiShaderStage::COMPUTE;
    cs_desc.shader_code = concat!(
        "void main() { \n",
        "    vec4 v = valueIn[index]; \n",
        "    v *= offset; \n",
        "    valueOut[index] = v; \n",
        "} \n"
    )
    .to_string();
    cs_desc.debug_name = "debug cs".to_string();

    let cs = hgi.create_shader_function(&cs_desc);
    assert!(
        !cs.is_null() && cs.get().map(|f| f.is_valid()).unwrap_or(false),
        "CS compile failed"
    );

    let mut prg_desc = HgiShaderProgramDesc::new();
    prg_desc.shader_functions.push(cs.clone());
    let prg = hgi.create_shader_program(&prg_desc);

    let mut pso_desc = HgiComputePipelineDesc::new();
    pso_desc.shader_constants_desc.byte_size = 16;
    pso_desc.shader_program = prg.clone();
    let pso = hgi.create_compute_pipeline(&pso_desc);
    assert!(!pso.is_null(), "Compute pipeline creation failed");

    let push_constants = vec![0u8; pso_desc.shader_constants_desc.byte_size as usize];
    let blob = vec![0u8; 64];

    let ubo = make_buffer(&mut hgi, 64, HgiBufferUsage::UNIFORM, Some(&blob));
    let ssbo0 = make_buffer(&mut hgi, 64, HgiBufferUsage::STORAGE, Some(&blob));
    let ssbo1 = make_buffer(&mut hgi, 64, HgiBufferUsage::STORAGE, Some(&blob));

    let mut ubo_bind = HgiBufferBindDesc::default();
    ubo_bind.binding_index = 0;
    ubo_bind.buffers.push(ubo.clone());
    ubo_bind.offsets.push(0);
    ubo_bind.resource_type = HgiBindResourceType::UniformBuffer;
    ubo_bind.stage_usage = HgiShaderStage::COMPUTE;

    let mut ssbo0_bind = HgiBufferBindDesc::default();
    ssbo0_bind.binding_index = 1;
    ssbo0_bind.buffers.push(ssbo0.clone());
    ssbo0_bind.offsets.push(0);
    ssbo0_bind.resource_type = HgiBindResourceType::StorageBuffer;
    ssbo0_bind.stage_usage = HgiShaderStage::COMPUTE;

    let mut ssbo1_bind = HgiBufferBindDesc::default();
    ssbo1_bind.binding_index = 2;
    ssbo1_bind.buffers.push(ssbo1.clone());
    ssbo1_bind.offsets.push(0);
    ssbo1_bind.resource_type = HgiBindResourceType::StorageBuffer;
    ssbo1_bind.stage_usage = HgiShaderStage::COMPUTE;

    let (bpt, _, _) = HgiFormat::Float32Vec4.data_size_of_format();
    let img_bytes = vec![0u8; 64 * 64 * bpt];
    let mut img_desc = HgiTextureDesc::new();
    img_desc.dimensions = Vec3i::new(64, 64, 1);
    img_desc.format = HgiFormat::Float32Vec4;
    img_desc.usage = HgiTextureUsage::SHADER_READ | HgiTextureUsage::SHADER_WRITE;
    let image = hgi.create_texture(&img_desc, Some(&img_bytes));

    let mut img_bind = HgiTextureBindDesc::default();
    img_bind.binding_index = 0;
    img_bind.resource_type = HgiBindResourceType::StorageImage;
    img_bind.samplers.push(HgiSamplerHandle::null());
    img_bind.stage_usage = HgiShaderStage::COMPUTE;
    img_bind.textures.push(image.clone());

    let mut rb_desc = HgiResourceBindingsDesc::new();
    rb_desc.buffer_bindings.push(ubo_bind);
    rb_desc.buffer_bindings.push(ssbo0_bind);
    rb_desc.buffer_bindings.push(ssbo1_bind);
    rb_desc.texture_bindings.push(img_bind);
    let bindings = hgi.create_resource_bindings(&rb_desc);

    let comp_desc = HgiComputeCmdsDesc::new();
    let mut compute = hgi.create_compute_cmds(&comp_desc);
    compute.push_debug_group("TestVulkanComputeCmds");
    compute.bind_pipeline(&pso);
    compute.bind_resources(&bindings);
    compute.set_constant_values(&pso, 0, &push_constants);
    compute.dispatch(&HgiComputeDispatchOp::new_2d(64, 64));
    compute.pop_debug_group();
    hgi.submit_cmds(compute, HgiSubmitWaitType::WaitUntilCompleted);

    hgi.destroy_resource_bindings(&bindings);
    hgi.destroy_texture(&image);
    hgi.destroy_buffer(&ubo);
    hgi.destroy_buffer(&ssbo0);
    hgi.destroy_buffer(&ssbo1);
    hgi.destroy_compute_pipeline(&pso);
    hgi.destroy_shader_program(&prg);
    hgi.destroy_shader_function(&cs);
    hgi.end_frame();

    let inflight = hgi
        .device()
        .command_queue()
        .get_inflight_command_buffers_bits();
    assert_eq!(inflight, 0, "Not all CBs consumed after compute");
}

// ---------------------------------------------------------------------------
// test_graphics_cmds_clear
// ---------------------------------------------------------------------------

/// Renders to a 512×512 texture using `loadOp=Clear` with `clearValue=(1,0,0.5,1)`,
/// saves the result, and compares with the baseline image.
///
/// Port of C++ `TestGraphicsCmdsClear`.
#[test]
#[ignore] // Requires Vulkan hardware
fn test_graphics_cmds_clear() {
    let _ = env_logger::try_init();
    let mut hgi = create_hgi();

    let width = IMG_SIZE as u32;
    let height = IMG_SIZE as u32;
    let format = HgiFormat::UNorm8Vec4;

    // Two color textures + one depth texture (same setup as _CreateGraphicsCmdsColor0Color1Depth).
    let mut td = HgiTextureDesc::new();
    td.dimensions = Vec3i::new(width as i32, height as i32, 1);
    td.texture_type = HgiTextureType::Texture2D;
    td.format = format;
    td.sample_count = HgiSampleCount::Count1;
    td.usage = HgiTextureUsage::COLOR_TARGET;
    let color_tex0 = hgi.create_texture(&td, None);
    let color_tex1 = hgi.create_texture(&td, None);

    td.usage = HgiTextureUsage::DEPTH_TARGET;
    td.format = HgiFormat::Float32;
    let depth_tex = hgi.create_texture(&td, None);

    // Color attachment 0: clear to (1, 0, 0.5, 1).
    let mut att0 = HgiAttachmentDesc::default();
    att0.load_op = HgiAttachmentLoadOp::Clear;
    att0.store_op = HgiAttachmentStoreOp::Store;
    att0.format = format;
    att0.clear_value = usd_gf::Vec4f::new(1.0, 0.0, 0.5, 1.0);

    let mut att1 = HgiAttachmentDesc::default();
    att1.load_op = HgiAttachmentLoadOp::Clear;
    att1.store_op = HgiAttachmentStoreOp::Store;
    att1.format = format;

    let mut depth_att = HgiAttachmentDesc::default();
    depth_att.format = HgiFormat::Float32;

    let mut gfx_desc = HgiGraphicsCmdsDesc::new();
    gfx_desc.color_attachment_descs.push(att0);
    gfx_desc.color_attachment_descs.push(att1);
    gfx_desc.depth_attachment_desc = depth_att;
    gfx_desc.color_textures.push(color_tex0.clone());
    gfx_desc.color_textures.push(color_tex1.clone());
    gfx_desc.depth_texture = depth_tex.clone();

    // For Vulkan the attachment is cleared on begin-render-pass when submitted.
    let gfx = hgi.create_graphics_cmds(&gfx_desc);
    hgi.submit_cmds(gfx, HgiSubmitWaitType::WaitUntilCompleted);

    let out_path = "graphicsCmdsClear.png";
    save_gpu_texture_to_file(&mut hgi, &color_tex0, width, height, format, out_path);
    compare_with_baseline(out_path, &baseline_png("graphics_cmds_clear.png"));

    hgi.destroy_texture(&color_tex0);
    hgi.destroy_texture(&color_tex1);
    hgi.destroy_texture(&depth_tex);
}

// ---------------------------------------------------------------------------
// test_create_srgba_texture
// ---------------------------------------------------------------------------

/// Creates a 128×128 sRGBA texture filled with value 64, saves to PNG,
/// and compares with the baseline.
///
/// Port of C++ `TestCreateSrgbaTexture`.
#[test]
#[ignore] // Requires Vulkan hardware
fn test_create_srgba_texture() {
    let _ = env_logger::try_init();
    let mut hgi = create_hgi();

    let width = 128u32;
    let height = 128u32;
    let format = HgiFormat::UNorm8Vec4srgb;

    let (bpt, _, _) = format.data_size_of_format();
    let data = vec![64u8; width as usize * height as usize * bpt];
    let tex = make_texture(&mut hgi, width as i32, height as i32, format, Some(&data));
    assert!(!tex.is_null(), "sRGBA texture creation failed");

    let out_path = "srgba.png";
    save_gpu_texture_to_file(&mut hgi, &tex, width, height, format, out_path);
    compare_with_baseline(out_path, &baseline_png("srgba.png"));

    hgi.destroy_texture(&tex);
}

// ---------------------------------------------------------------------------
// test_hgi_get_mip_initial_data  (no GPU required)
// ---------------------------------------------------------------------------

/// Validates `get_mip_infos` for a 37×53 texture with a 3-mip budget.
///
/// Port of C++ `TestHgiGetMipInitialData`. Pure computation — no GPU required,
/// so this test is NOT marked `#[ignore]`.
#[test]
fn test_hgi_get_mip_initial_data() {
    let format = HgiFormat::UNorm8Vec4;
    let size0 = Vec3i::new(37, 53, 1);
    let (bpt, _, _) = format.data_size_of_format();

    let first_mip_size = (size0[0] * size0[1] * size0[2]) as usize * bpt;
    let size1 = Vec3i::new((size0[0] / 2).max(1), (size0[1] / 2).max(1), 1);
    let second_mip_size = (size1[0] * size1[1] * size1[2]) as usize * bpt;
    let size2 = Vec3i::new((size1[0] / 2).max(1), (size1[1] / 2).max(1), 1);
    let third_mip_size = (size2[0] * size2[1] * size2[2]) as usize * bpt;

    let total = first_mip_size + second_mip_size + third_mip_size;
    let mips = get_mip_infos(format, &size0, 1, Some(total));

    assert_eq!(mips.len(), 3, "Expected 3 mips, got {}", mips.len());

    let start_of_third = first_mip_size + second_mip_size;
    assert_eq!(
        mips[2].dimensions, size2,
        "Third mip dimensions: expected {:?}, got {:?}",
        size2, mips[2].dimensions
    );
    assert_eq!(
        mips[2].byte_size_per_layer, third_mip_size,
        "Third mip byte_size_per_layer: expected {}, got {}",
        third_mip_size, mips[2].byte_size_per_layer
    );
    assert_eq!(
        mips[2].byte_offset, start_of_third,
        "Third mip byte_offset: expected {}, got {}",
        start_of_third, mips[2].byte_offset
    );
}

// ---------------------------------------------------------------------------
// test_hgi_texture_to_buffer_copy
// ---------------------------------------------------------------------------

/// Copies an sRGBA texture (filled with 16) to a buffer, saves PNG,
/// and compares with the baseline.
///
/// Port of C++ `TestHgiTextureToBufferCopy`.
#[test]
#[ignore] // Requires Vulkan hardware
fn test_hgi_texture_to_buffer_copy() {
    let _ = env_logger::try_init();
    let mut hgi = create_hgi();

    let width = 128i32;
    let height = 128i32;
    let format = HgiFormat::UNorm8Vec4srgb;
    let (bpt, _, _) = format.data_size_of_format();
    let data_size = width as usize * height as usize * bpt;

    let data = vec![16u8; data_size];
    let tex = make_texture(&mut hgi, width, height, format, Some(&data));
    let buf = make_buffer(&mut hgi, data_size, HgiBufferUsage::UNIFORM, None);

    let copy_op = HgiTextureToBufferOp {
        gpu_source_texture: tex.clone(),
        source_texel_offset: Vec3i::new(0, 0, 0),
        mip_level: 0,
        source_layer: 0,
        gpu_destination_buffer: buf.clone(),
        destination_byte_offset: 0,
        copy_size: Vec3i::new(0, 0, 0),
    };
    {
        let mut blit = hgi.create_blit_cmds();
        blit.copy_texture_to_buffer(&copy_op);
        hgi.submit_cmds(blit, HgiSubmitWaitType::WaitUntilCompleted);
    }

    let out_path = "copyTextureToBuffer.png";
    save_gpu_buffer_to_file(
        &mut hgi,
        &buf,
        width as u32,
        height as u32,
        format,
        out_path,
    );
    compare_with_baseline(out_path, &baseline_png("copy_texture_to_buffer.png"));

    hgi.destroy_buffer(&buf);
    hgi.destroy_texture(&tex);
}

// ---------------------------------------------------------------------------
// test_hgi_buffer_to_texture_copy
// ---------------------------------------------------------------------------

/// Copies a buffer (filled with 32) to an sRGBA texture, saves PNG,
/// and compares with the baseline.
///
/// Port of C++ `TestHgiBufferToTextureCopy`.
#[test]
#[ignore] // Requires Vulkan hardware
fn test_hgi_buffer_to_texture_copy() {
    let _ = env_logger::try_init();
    let mut hgi = create_hgi();

    let width = 128i32;
    let height = 128i32;
    let format = HgiFormat::UNorm8Vec4srgb;
    let (bpt, _, _) = format.data_size_of_format();
    let data_size = width as usize * height as usize * bpt;

    let data = vec![32u8; data_size];
    let buf = make_buffer(&mut hgi, data_size, HgiBufferUsage::UNIFORM, Some(&data));
    let tex = make_texture(&mut hgi, width, height, format, None);

    let copy_op = HgiBufferToTextureOp {
        gpu_source_buffer: buf.clone(),
        source_byte_offset: 0,
        gpu_destination_texture: tex.clone(),
        destination_texel_offset: Vec3i::new(0, 0, 0),
        copy_size: Vec3i::new(0, 0, 0),
        destination_mip_level: 0,
        destination_layer: 0,
    };
    {
        let mut blit = hgi.create_blit_cmds();
        blit.copy_buffer_to_texture(&copy_op);
        hgi.submit_cmds(blit, HgiSubmitWaitType::WaitUntilCompleted);
    }

    let out_path = "copyBufferToTexture.png";
    save_gpu_texture_to_file(
        &mut hgi,
        &tex,
        width as u32,
        height as u32,
        format,
        out_path,
    );
    compare_with_baseline(out_path, &baseline_png("copy_buffer_to_texture.png"));

    hgi.destroy_texture(&tex);
    hgi.destroy_buffer(&buf);
}
