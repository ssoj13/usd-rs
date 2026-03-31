//! Integration tests for HgiWgpu -- ported from C++ testHgiVulkan.cpp & testHgiCommand.cpp.
//!
//! Tests are adapted for the wgpu backend: Vulkan-specific internals (instance fn ptrs,
//! command queue threading, garbage collector internals) are replaced with wgpu-equivalent
//! smoke tests that exercise the same logical path through the Hgi trait.
//!
//! All tests guard against missing GPU with `try_create_hgi()` -- they skip gracefully
//! in headless CI environments.

#[allow(unsafe_code)]
use usd_gf::Vec3i;
use usd_hgi::blit_cmds::*;
use usd_hgi::enums::*;
use usd_hgi::*;
use usd_hgi_wgpu::*;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn try_create_hgi() -> Option<HgiWgpu> {
    let _ = env_logger::builder().is_test(true).try_init();
    create_hgi_wgpu()
}

// ===========================================================================
// 1. Instance / device (port of TestVulkanInstance + TestVulkanDevice)
// ===========================================================================

#[test]
fn test_wgpu_instance() {
    let Some(hgi) = try_create_hgi() else { return };
    assert!(hgi.is_backend_supported());
    assert_eq!(hgi.get_api_name(), "wgpu");
}

#[test]
fn test_wgpu_device() {
    let Some(hgi) = try_create_hgi() else { return };
    let _device = hgi.device();
    let _queue = hgi.queue();
    let _adapter = hgi.adapter();
}

#[test]
fn test_wgpu_capabilities() {
    let Some(hgi) = try_create_hgi() else { return };
    let _caps = hgi.capabilities();
}

// ===========================================================================
// 2. Unique ID
// ===========================================================================

#[test]
fn test_wgpu_unique_id() {
    let Some(mut hgi) = try_create_hgi() else {
        return;
    };
    let id1 = hgi.unique_id();
    let id2 = hgi.unique_id();
    let id3 = hgi.unique_id();
    assert!(id2 > id1);
    assert!(id3 > id2);
}

// ===========================================================================
// 3. Buffer creation (port of TestVulkanBuffer)
// ===========================================================================

#[test]
fn test_wgpu_buffer_create() {
    let Some(mut hgi) = try_create_hgi() else {
        return;
    };
    hgi.start_frame();

    let data: Vec<u32> = vec![123u32; 16];
    let byte_size = data.len() * std::mem::size_of::<u32>();
    let data_bytes: &[u8] = bytemuck::cast_slice(&data);

    let desc = HgiBufferDesc {
        debug_name: "TestBuffer".into(),
        byte_size,
        usage: HgiBufferUsage::STORAGE,
        vertex_stride: 0,
    };

    let buffer = hgi.create_buffer(&desc, Some(data_bytes));
    assert!(buffer.get().is_some());
    assert_eq!(buffer.get().unwrap().byte_size_of_resource(), byte_size);

    hgi.destroy_buffer(&buffer);
    hgi.end_frame();
}

#[test]
fn test_wgpu_buffer_vertex_index() {
    let Some(mut hgi) = try_create_hgi() else {
        return;
    };
    hgi.start_frame();

    let vertices: [f32; 18] = [
        -0.25, 0.25, 0.0, 0.25, 0.0, 1.0, -0.25, -0.25, 0.0, 0.25, 0.0, 0.0, 0.25, -0.25, 0.0,
        0.25, 0.25, 0.0,
    ];
    let vbo = hgi.create_buffer(
        &HgiBufferDesc {
            debug_name: "VertexBuffer".into(),
            byte_size: std::mem::size_of_val(&vertices),
            usage: HgiBufferUsage::VERTEX,
            vertex_stride: 24, // 6 * f32
        },
        Some(bytemuck::cast_slice(&vertices)),
    );
    assert!(vbo.get().is_some());

    let indices: [u32; 3] = [0, 1, 2];
    let ibo = hgi.create_buffer(
        &HgiBufferDesc {
            debug_name: "IndexBuffer".into(),
            byte_size: std::mem::size_of_val(&indices),
            usage: HgiBufferUsage::INDEX32,
            vertex_stride: 0,
        },
        Some(bytemuck::cast_slice(&indices)),
    );
    assert!(ibo.get().is_some());

    hgi.destroy_buffer(&vbo);
    hgi.destroy_buffer(&ibo);
    hgi.end_frame();
}

// ===========================================================================
// 4. Texture creation + view (port of TestVulkanTexture)
// ===========================================================================

#[test]
fn test_wgpu_texture_create() {
    let Some(mut hgi) = try_create_hgi() else {
        return;
    };
    hgi.start_frame();

    let (width, height) = (32i32, 32i32);
    let num_texels = (width * height) as usize;
    let pixels: Vec<f32> = vec![0.123f32; num_texels * 4];

    let desc = HgiTextureDesc {
        debug_name: "Debug Texture".into(),
        dimensions: Vec3i::new(width, height, 1),
        format: HgiFormat::Float32Vec4,
        texture_type: HgiTextureType::Texture2D,
        usage: HgiTextureUsage::COLOR_TARGET | HgiTextureUsage::SHADER_READ,
        ..Default::default()
    };

    let texture = hgi.create_texture(&desc, Some(bytemuck::cast_slice(&pixels)));
    assert!(texture.get().is_some());

    let view_desc = HgiTextureViewDesc {
        debug_name: "Debug TextureView".into(),
        format: HgiFormat::Float32Vec4,
        source_texture: texture.clone(),
        ..Default::default()
    };
    let view = hgi.create_texture_view(&view_desc);
    assert!(view.get().is_some());

    hgi.destroy_texture_view(&view);
    hgi.destroy_texture(&texture);
    hgi.end_frame();
}

// ===========================================================================
// 5. Shader creation (port of TestVulkanPipeline shader part)
// ===========================================================================

#[test]
fn test_wgpu_shader_creation() {
    let Some(mut hgi) = try_create_hgi() else {
        return;
    };
    hgi.start_frame();

    let vs = hgi.create_shader_function(&HgiShaderFunctionDesc {
        debug_name: "test_vertex".into(),
        shader_stage: HgiShaderStage::VERTEX,
        shader_code: concat!(
            "@vertex\n",
            "fn vs_main(@location(0) pos: vec3<f32>) -> @builtin(position) vec4<f32> {\n",
            "    return vec4<f32>(pos, 1.0);\n",
            "}\n",
        )
        .into(),
        ..Default::default()
    });
    assert!(vs.get().is_some());

    let prg = hgi.create_shader_program(&HgiShaderProgramDesc {
        debug_name: "test_program".into(),
        shader_functions: vec![vs.clone()],
        ..Default::default()
    });
    assert!(prg.get().is_some());

    hgi.destroy_shader_program(&prg);
    hgi.destroy_shader_function(&vs);
    hgi.end_frame();
}

// ===========================================================================
// 6. Graphics pipeline (port of TestVulkanPipeline)
// ===========================================================================

#[test]
fn test_wgpu_graphics_pipeline() {
    let Some(mut hgi) = try_create_hgi() else {
        return;
    };
    hgi.start_frame();

    let vs = hgi.create_shader_function(&HgiShaderFunctionDesc {
        debug_name: "pipeline_vs".into(),
        shader_stage: HgiShaderStage::VERTEX,
        shader_code: concat!(
            "@vertex\n",
            "fn vs_main(@location(0) pos: vec3<f32>) -> @builtin(position) vec4<f32> {\n",
            "    return vec4<f32>(pos, 1.0);\n",
            "}\n",
        )
        .into(),
        ..Default::default()
    });
    let prg = hgi.create_shader_program(&HgiShaderProgramDesc {
        debug_name: "pipeline_program".into(),
        shader_functions: vec![vs.clone()],
        ..Default::default()
    });

    let vbo_desc = HgiVertexBufferDesc {
        binding_index: 0,
        vertex_attributes: vec![HgiVertexAttributeDesc {
            format: HgiFormat::Float32Vec3,
            offset: 0,
            shader_binding_location: 0,
        }],
        vertex_stride: 12,
        step_function: HgiVertexBufferStepFunction::PerVertex,
    };

    let pso = hgi.create_graphics_pipeline(&HgiGraphicsPipelineDesc {
        debug_name: "test_pipeline".into(),
        depth_stencil_state: HgiDepthStencilState {
            depth_test_enabled: false,
            depth_write_enabled: false,
            stencil_test_enabled: false,
            ..Default::default()
        },
        primitive_type: HgiPrimitiveType::PointList,
        rasterization_state: HgiRasterizationState {
            rasterizer_enabled: false,
            ..Default::default()
        },
        shader_program: prg.clone(),
        vertex_buffers: vec![vbo_desc],
        ..Default::default()
    });
    assert!(pso.get().is_some());

    hgi.destroy_graphics_pipeline(&pso);
    hgi.destroy_shader_program(&prg);
    hgi.destroy_shader_function(&vs);
    hgi.end_frame();
}

// ===========================================================================
// 7. Compute pipeline (port of TestVulkanComputeCmds)
// ===========================================================================

#[test]
fn test_wgpu_compute_pipeline() {
    let Some(mut hgi) = try_create_hgi() else {
        return;
    };
    hgi.start_frame();

    let cs = hgi.create_shader_function(&HgiShaderFunctionDesc {
        debug_name: "test_compute".into(),
        shader_stage: HgiShaderStage::COMPUTE,
        shader_code: concat!(
            "@compute @workgroup_size(1)\n",
            "fn cs_main(@builtin(global_invocation_id) id: vec3<u32>) {}\n",
        )
        .into(),
        ..Default::default()
    });
    let prg = hgi.create_shader_program(&HgiShaderProgramDesc {
        debug_name: "compute_program".into(),
        shader_functions: vec![cs.clone()],
        ..Default::default()
    });
    let pso = hgi.create_compute_pipeline(&HgiComputePipelineDesc {
        debug_name: "test_compute_pipeline".into(),
        shader_program: prg.clone(),
        ..Default::default()
    });
    assert!(pso.get().is_some());

    hgi.destroy_compute_pipeline(&pso);
    hgi.destroy_shader_program(&prg);
    hgi.destroy_shader_function(&cs);
    hgi.end_frame();
}

// ===========================================================================
// 8. Buffer readback (port of TestVulkanBuffer readback)
// ===========================================================================

#[test]
fn test_wgpu_buffer_readback() {
    let Some(mut hgi) = try_create_hgi() else {
        return;
    };
    hgi.start_frame();

    let data: Vec<u32> = vec![42u32; 16];
    let byte_size = data.len() * std::mem::size_of::<u32>();

    let buffer = hgi.create_buffer(
        &HgiBufferDesc {
            debug_name: "ReadbackBuf".into(),
            byte_size,
            usage: HgiBufferUsage::STORAGE,
            vertex_stride: 0,
        },
        Some(bytemuck::cast_slice(&data)),
    );

    let mut readback: Vec<u32> = vec![0u32; 16];
    let readback_bytes: &mut [u8] = bytemuck::cast_slice_mut(&mut readback);

    let copy_op = HgiBufferGpuToCpuOp {
        gpu_source_buffer: buffer.clone(),
        source_byte_offset: 0,
        #[allow(unsafe_code)]
        cpu_destination_buffer: unsafe { RawCpuBufferMut::new(readback_bytes.as_mut_ptr()) },
        byte_size,
    };

    let mut blit = hgi.create_blit_cmds();
    blit.copy_buffer_gpu_to_cpu(&copy_op);
    hgi.submit_cmds(blit, HgiSubmitWaitType::WaitUntilCompleted);

    assert_eq!(readback, data, "Buffer readback should match initial data");

    hgi.destroy_buffer(&buffer);
    hgi.end_frame();
}

// ===========================================================================
// 9. Buffer CPU->GPU transfer + readback (port of staging copy test)
// ===========================================================================

#[test]
fn test_wgpu_buffer_cpu_to_gpu_transfer() {
    let Some(mut hgi) = try_create_hgi() else {
        return;
    };
    hgi.start_frame();

    let count = 16usize;
    let byte_size = count * std::mem::size_of::<u32>();

    let buffer = hgi.create_buffer(
        &HgiBufferDesc {
            debug_name: "TransferBuf".into(),
            byte_size,
            usage: HgiBufferUsage::STORAGE,
            vertex_stride: 0,
        },
        None,
    );

    // Upload
    let upload_data: Vec<u32> = vec![789u32; count];
    let upload_bytes: &[u8] = bytemuck::cast_slice(&upload_data);

    let upload_op = HgiBufferCpuToGpuOp {
        cpu_source_buffer: RawCpuBuffer::new(upload_bytes.as_ptr()),
        source_byte_offset: 0,
        gpu_destination_buffer: buffer.clone(),
        destination_byte_offset: 0,
        byte_size,
    };
    let mut blit_up = hgi.create_blit_cmds();
    blit_up.copy_buffer_cpu_to_gpu(&upload_op);
    hgi.submit_cmds(blit_up, HgiSubmitWaitType::WaitUntilCompleted);

    // Readback
    let mut readback: Vec<u32> = vec![0u32; count];
    let readback_bytes: &mut [u8] = bytemuck::cast_slice_mut(&mut readback);
    let read_op = HgiBufferGpuToCpuOp {
        gpu_source_buffer: buffer.clone(),
        source_byte_offset: 0,
        #[allow(unsafe_code)]
        cpu_destination_buffer: unsafe { RawCpuBufferMut::new(readback_bytes.as_mut_ptr()) },
        byte_size,
    };
    let mut blit_rd = hgi.create_blit_cmds();
    blit_rd.copy_buffer_gpu_to_cpu(&read_op);
    hgi.submit_cmds(blit_rd, HgiSubmitWaitType::WaitUntilCompleted);

    assert_eq!(readback, upload_data, "Transfer readback must match");

    hgi.destroy_buffer(&buffer);
    hgi.end_frame();
}

// ===========================================================================
// 10. Texture readback (port of TestVulkanTexture readback)
// ===========================================================================

#[test]
fn test_wgpu_texture_readback() {
    let Some(mut hgi) = try_create_hgi() else {
        return;
    };
    hgi.start_frame();

    let (w, h) = (8i32, 8i32);
    let num_texels = (w * h) as usize;
    let num_floats = num_texels * 4;
    let pixel_bytes = num_floats * std::mem::size_of::<f32>();

    let pixels: Vec<f32> = vec![0.5f32; num_floats];
    let texture = hgi.create_texture(
        &HgiTextureDesc {
            debug_name: "ReadbackTex".into(),
            dimensions: Vec3i::new(w, h, 1),
            format: HgiFormat::Float32Vec4,
            texture_type: HgiTextureType::Texture2D,
            usage: HgiTextureUsage::COLOR_TARGET | HgiTextureUsage::SHADER_READ,
            ..Default::default()
        },
        Some(bytemuck::cast_slice(&pixels)),
    );

    let mut readback: Vec<f32> = vec![0.0f32; num_floats];
    let rb_bytes: &mut [u8] = bytemuck::cast_slice_mut(&mut readback);

    let read_op = HgiTextureGpuToCpuOp {
        gpu_source_texture: texture.clone(),
        source_texel_offset: Vec3i::new(0, 0, 0),
        mip_level: 0,
        #[allow(unsafe_code)]
        cpu_destination_buffer: unsafe { RawCpuBufferMut::new(rb_bytes.as_mut_ptr()) },
        destination_byte_offset: 0,
        destination_buffer_byte_size: pixel_bytes,
        copy_size: Vec3i::new(w, h, 1),
        source_layer: 0,
    };

    let mut blit = hgi.create_blit_cmds();
    blit.copy_texture_gpu_to_cpu(&read_op);
    hgi.submit_cmds(blit, HgiSubmitWaitType::WaitUntilCompleted);

    assert_eq!(readback, pixels, "Texture readback must match");

    hgi.destroy_texture(&texture);
    hgi.end_frame();
}

// ===========================================================================
// 11. Garbage collection (port of TestVulkanGarbageCollection)
// ===========================================================================

#[test]
fn test_wgpu_garbage_collection() {
    let Some(mut hgi) = try_create_hgi() else {
        return;
    };
    hgi.start_frame();

    let buffer = hgi.create_buffer(
        &HgiBufferDesc {
            debug_name: "GcBuffer".into(),
            byte_size: 64,
            usage: HgiBufferUsage::STORAGE,
            vertex_stride: 0,
        },
        None,
    );
    let texture = hgi.create_texture(
        &HgiTextureDesc {
            debug_name: "GcTexture".into(),
            dimensions: Vec3i::new(4, 4, 1),
            format: HgiFormat::UNorm8Vec4,
            texture_type: HgiTextureType::Texture2D,
            usage: HgiTextureUsage::COLOR_TARGET,
            ..Default::default()
        },
        None,
    );

    hgi.destroy_buffer(&buffer);
    hgi.destroy_texture(&texture);
    hgi.end_frame();

    // Second cycle should be clean
    hgi.start_frame();
    hgi.end_frame();
}

// ===========================================================================
// 12. Frame lifecycle
// ===========================================================================

#[test]
fn test_wgpu_frame_lifecycle() {
    let Some(mut hgi) = try_create_hgi() else {
        return;
    };
    // Nested frames
    hgi.start_frame();
    hgi.start_frame();
    hgi.end_frame();
    hgi.end_frame();
    // Single frame
    hgi.start_frame();
    hgi.end_frame();
}

// ===========================================================================
// 13. Sampler
// ===========================================================================

#[test]
fn test_wgpu_sampler_create() {
    let Some(mut hgi) = try_create_hgi() else {
        return;
    };
    let sampler = hgi.create_sampler(&HgiSamplerDesc {
        debug_name: "TestSampler".into(),
        ..Default::default()
    });
    assert!(sampler.get().is_some());
    hgi.destroy_sampler(&sampler);
}

// ===========================================================================
// 14. Resource bindings (port of TestVulkanComputeCmds resource part)
// ===========================================================================

#[test]
fn test_wgpu_resource_bindings() {
    let Some(mut hgi) = try_create_hgi() else {
        return;
    };
    hgi.start_frame();

    let blob = vec![0u8; 64];
    let ubo = hgi.create_buffer(
        &HgiBufferDesc {
            debug_name: "Ubo".into(),
            byte_size: 64,
            usage: HgiBufferUsage::UNIFORM,
            vertex_stride: 0,
        },
        Some(&blob),
    );
    let ssbo = hgi.create_buffer(
        &HgiBufferDesc {
            debug_name: "Ssbo".into(),
            byte_size: 64,
            usage: HgiBufferUsage::STORAGE,
            vertex_stride: 0,
        },
        Some(&blob),
    );

    let rb_desc = HgiResourceBindingsDesc {
        buffer_bindings: vec![
            HgiBufferBindDesc {
                binding_index: 0,
                buffers: vec![ubo.clone()],
                offsets: vec![0],
                resource_type: HgiBindResourceType::UniformBuffer,
                stage_usage: HgiShaderStage::COMPUTE,
                ..Default::default()
            },
            HgiBufferBindDesc {
                binding_index: 1,
                buffers: vec![ssbo.clone()],
                offsets: vec![0],
                resource_type: HgiBindResourceType::StorageBuffer,
                stage_usage: HgiShaderStage::COMPUTE,
                ..Default::default()
            },
        ],
        ..Default::default()
    };

    let bindings = hgi.create_resource_bindings(&rb_desc);
    assert!(bindings.get().is_some());

    hgi.destroy_resource_bindings(&bindings);
    hgi.destroy_buffer(&ubo);
    hgi.destroy_buffer(&ssbo);
    hgi.end_frame();
}

// ===========================================================================
// 15. Texture-to-buffer copy (port of TestHgiTextureToBufferCopy)
// ===========================================================================

#[test]
fn test_wgpu_texture_to_buffer_copy() {
    let Some(mut hgi) = try_create_hgi() else {
        return;
    };
    hgi.start_frame();

    let (w, h) = (16i32, 16i32);
    let byte_size = (w * h) as usize * 4; // UNorm8Vec4

    let texture = hgi.create_texture(
        &HgiTextureDesc {
            debug_name: "CopySrcTex".into(),
            dimensions: Vec3i::new(w, h, 1),
            format: HgiFormat::UNorm8Vec4,
            texture_type: HgiTextureType::Texture2D,
            usage: HgiTextureUsage::SHADER_READ,
            ..Default::default()
        },
        Some(&vec![64u8; byte_size]),
    );
    let buffer = hgi.create_buffer(
        &HgiBufferDesc {
            debug_name: "CopyDstBuf".into(),
            byte_size,
            usage: HgiBufferUsage::STORAGE,
            vertex_stride: 0,
        },
        None,
    );

    let copy_op = HgiTextureToBufferOp {
        gpu_source_texture: texture.clone(),
        source_texel_offset: Vec3i::new(0, 0, 0),
        mip_level: 0,
        source_layer: 0,
        gpu_destination_buffer: buffer.clone(),
        destination_byte_offset: 0,
        copy_size: Vec3i::new(w, h, 1),
    };
    let mut blit = hgi.create_blit_cmds();
    blit.copy_texture_to_buffer(&copy_op);
    hgi.submit_cmds(blit, HgiSubmitWaitType::WaitUntilCompleted);

    hgi.destroy_buffer(&buffer);
    hgi.destroy_texture(&texture);
    hgi.end_frame();
}

// ===========================================================================
// 16. Buffer-to-texture copy (port of TestHgiBufferToTextureCopy)
// ===========================================================================

#[test]
fn test_wgpu_buffer_to_texture_copy() {
    let Some(mut hgi) = try_create_hgi() else {
        return;
    };
    hgi.start_frame();

    let (w, h) = (16i32, 16i32);
    let byte_size = (w * h) as usize * 4;

    let buffer = hgi.create_buffer(
        &HgiBufferDesc {
            debug_name: "CopySrcBuf".into(),
            byte_size,
            usage: HgiBufferUsage::STORAGE,
            vertex_stride: 0,
        },
        Some(&vec![32u8; byte_size]),
    );
    let texture = hgi.create_texture(
        &HgiTextureDesc {
            debug_name: "CopyDstTex".into(),
            dimensions: Vec3i::new(w, h, 1),
            format: HgiFormat::UNorm8Vec4,
            texture_type: HgiTextureType::Texture2D,
            usage: HgiTextureUsage::SHADER_READ,
            ..Default::default()
        },
        None,
    );

    let copy_op = HgiBufferToTextureOp {
        gpu_source_buffer: buffer.clone(),
        source_byte_offset: 0,
        gpu_destination_texture: texture.clone(),
        destination_texel_offset: Vec3i::new(0, 0, 0),
        copy_size: Vec3i::new(w, h, 1),
        destination_mip_level: 0,
        destination_layer: 0,
    };
    let mut blit = hgi.create_blit_cmds();
    blit.copy_buffer_to_texture(&copy_op);
    hgi.submit_cmds(blit, HgiSubmitWaitType::WaitUntilCompleted);

    hgi.destroy_texture(&texture);
    hgi.destroy_buffer(&buffer);
    hgi.end_frame();
}

// ===========================================================================
// 17. Mipmap generation (port of TestVulkanTexture mipmap path)
// ===========================================================================

#[test]
fn test_wgpu_mipmap_generation() {
    let Some(mut hgi) = try_create_hgi() else {
        return;
    };
    hgi.start_frame();

    let (w, h) = (32i32, 32i32);
    let num_floats = (w * h) as usize * 4;
    let pixels: Vec<f32> = vec![1.0f32; num_floats];

    let texture = hgi.create_texture(
        &HgiTextureDesc {
            debug_name: "MipTex".into(),
            dimensions: Vec3i::new(w, h, 1),
            format: HgiFormat::Float32Vec4,
            texture_type: HgiTextureType::Texture2D,
            usage: HgiTextureUsage::COLOR_TARGET | HgiTextureUsage::SHADER_READ,
            ..Default::default()
        },
        Some(bytemuck::cast_slice(&pixels)),
    );

    let mut blit = hgi.create_blit_cmds();
    blit.generate_mipmap(&texture);
    hgi.submit_cmds(blit, HgiSubmitWaitType::WaitUntilCompleted);

    hgi.destroy_texture(&texture);
    hgi.end_frame();
}

// ===========================================================================
// 18. Wait for idle
// ===========================================================================

#[test]
fn test_wgpu_wait_for_idle() {
    let Some(mut hgi) = try_create_hgi() else {
        return;
    };
    hgi.wait_for_idle();
}

// ===========================================================================
// 19. Device identity
// ===========================================================================

#[test]
fn test_wgpu_device_identity() {
    let Some(hgi) = try_create_hgi() else { return };
    let id = hgi.device_identity();
    assert_ne!(id, 0);
    assert_eq!(id, hgi.device_identity());
}

// ===========================================================================
// 20. Backend support (static)
// ===========================================================================

#[test]
fn test_wgpu_backend_support() {
    let _supported = HgiWgpu::check_backend_support();
}

// ===========================================================================
// 21. Multi-frame resource cycling (stress test)
// ===========================================================================

#[test]
fn test_wgpu_multi_frame_resources() {
    let Some(mut hgi) = try_create_hgi() else {
        return;
    };
    for frame in 0..3 {
        hgi.start_frame();
        let buffer = hgi.create_buffer(
            &HgiBufferDesc {
                debug_name: format!("Frame{frame}Buf"),
                byte_size: 256,
                usage: HgiBufferUsage::UNIFORM,
                vertex_stride: 0,
            },
            None,
        );
        let texture = hgi.create_texture(
            &HgiTextureDesc {
                debug_name: format!("Frame{frame}Tex"),
                dimensions: Vec3i::new(8, 8, 1),
                format: HgiFormat::UNorm8Vec4,
                texture_type: HgiTextureType::Texture2D,
                usage: HgiTextureUsage::COLOR_TARGET,
                ..Default::default()
            },
            None,
        );
        hgi.destroy_buffer(&buffer);
        hgi.destroy_texture(&texture);
        hgi.end_frame();
    }
}

// ===========================================================================
// 22. Fill buffer
// ===========================================================================

#[test]
fn test_wgpu_fill_buffer() {
    let Some(mut hgi) = try_create_hgi() else {
        return;
    };
    hgi.start_frame();

    let buffer = hgi.create_buffer(
        &HgiBufferDesc {
            debug_name: "FillBuf".into(),
            byte_size: 256,
            usage: HgiBufferUsage::STORAGE,
            vertex_stride: 0,
        },
        None,
    );

    let mut blit = hgi.create_blit_cmds();
    blit.fill_buffer(&buffer, 0xFF);
    hgi.submit_cmds(blit, HgiSubmitWaitType::WaitUntilCompleted);

    hgi.destroy_buffer(&buffer);
    hgi.end_frame();
}
