//! Texture loading helpers for the UsdImagingGL engine.
//!
//! Contains free functions for Value conversion, format mapping, path resolution,
//! and HGI texture upload. All wgpu-specific functions are gated on `#[cfg(feature = "wgpu")]`.

use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

use usd_sdf::Path;

#[cfg(feature = "wgpu")]
use usd_hgi::{
    enums::{
        HgiMipFilter, HgiSamplerAddressMode, HgiSamplerFilter, HgiSubmitWaitType as WgpuSubmitWait,
        HgiTextureUsage,
    },
    sampler::HgiSamplerDesc,
    texture::{HgiTextureDesc, HgiTextureHandle},
    types::HgiFormat,
};
#[cfg(feature = "wgpu")]
use usd_hgi_wgpu::HgiWgpu;

use usd_hd_st::draw_item::MaterialTextureHandles;

// ---------------------------------------------------------------------------
// Texture loading helpers (wgpu feature only)
// ---------------------------------------------------------------------------

/// USD surface input name -> @group(3) slot index mapping.
///
/// Matches the wgsl_code_gen layout:
///   slot 0 = diffuse, 1 = normal, 2 = roughness, 3 = metallic,
///   4 = opacity, 5 = emissive, 6 = occlusion
#[cfg(feature = "wgpu")]
pub(super) fn usd_input_to_tex_slot(input_name: &str) -> Option<usize> {
    match input_name {
        "diffuseColor" | "baseColor" => Some(0),
        "normal" | "normalMap" => Some(1),
        "roughness" => Some(2),
        "metallic" => Some(3),
        "opacity" => Some(4),
        "emissiveColor" => Some(5),
        "occlusion" => Some(6),
        _ => None,
    }
}

/// Compute the number of mip levels for a given 2D texture size.
///
/// Returns floor(log2(max(w, h))) + 1, which is the standard full mip chain count.
/// Minimum 1 (no mipmapping for 1x1 textures, or degenerate zero-size textures).
#[cfg(feature = "wgpu")]
pub(super) fn calc_mip_levels(w: u32, h: u32) -> u32 {
    if w == 0 || h == 0 {
        return 1;
    }
    (w.max(h) as f32).log2().floor() as u32 + 1
}

/// Convert HioFormat + sRGB flag to HgiFormat for GPU upload.
#[cfg(feature = "wgpu")]
pub(super) fn hio_format_to_hgi(hio: usd_hio::types::HioFormat, is_srgb: bool) -> HgiFormat {
    use usd_hio::types::HioFormat as Hio;
    match hio {
        Hio::UNorm8 => HgiFormat::UNorm8,
        Hio::UNorm8Vec2 => HgiFormat::UNorm8Vec2,
        Hio::UNorm8Vec3 | Hio::UNorm8Vec4 => {
            if is_srgb {
                HgiFormat::UNorm8Vec4srgb
            } else {
                HgiFormat::UNorm8Vec4
            }
        }
        Hio::Float16Vec4 => HgiFormat::Float16Vec4,
        Hio::Float16Vec3 => HgiFormat::Float16Vec3,
        Hio::Float16Vec2 => HgiFormat::Float16Vec2,
        Hio::Float16 => HgiFormat::Float16,
        Hio::Float32Vec4 => HgiFormat::Float32Vec4,
        Hio::Float32Vec3 => HgiFormat::Float32Vec3,
        Hio::Float32Vec2 => HgiFormat::Float32Vec2,
        Hio::Float32 => HgiFormat::Float32,
        _ => HgiFormat::UNorm8Vec4,
    }
}

/// Resolve a texture asset path string relative to a USD stage's root layer.
///
/// Handles relative paths by joining with the stage's root layer directory.
#[cfg(feature = "wgpu")]
pub(super) fn resolve_tex_path(
    raw_path: &str,
    stage: &Arc<usd_core::stage::Stage>,
) -> Option<String> {
    if raw_path.is_empty() {
        return None;
    }
    // Try as-is first (absolute or already exists)
    if std::path::Path::new(raw_path).exists() {
        return Some(raw_path.to_string());
    }
    // Resolve relative to root layer directory
    {
        let root_layer = stage.get_root_layer();
        let layer_path = root_layer.identifier().to_string();
        if !layer_path.is_empty() {
            let layer_dir = std::path::Path::new(&layer_path)
                .parent()
                .unwrap_or(std::path::Path::new("."));
            let resolved = layer_dir.join(raw_path);
            if resolved.exists() {
                return Some(resolved.to_string_lossy().into_owned());
            }
        }
    }
    // Return raw path as last resort (may fail to load, logged by caller)
    Some(raw_path.to_string())
}

/// Load all textures for a mesh from texture_paths, upload to HGI, return handles.
///
/// For each (slot_name -> file_path) entry:
///   1. Resolve path relative to stage root layer
///   2. Read pixels via HIO
///   3. Create HGI texture + sampler
///   4. Store in MaterialTextureHandles at the correct slot index
#[cfg(feature = "wgpu")]
pub(super) fn load_mesh_textures(
    tex_paths: &std::collections::HashMap<usd_tf::Token, String>,
    hgi_arc: &Arc<RwLock<HgiWgpu>>,
    prim_id: &Path,
    stage: &Arc<usd_core::stage::Stage>,
    texture_cache: &mut HashMap<String, (HgiTextureHandle, usd_hgi::sampler::HgiSamplerHandle)>,
) -> MaterialTextureHandles {
    use usd_hgi::hgi::Hgi;
    use usd_hio::image::SourceColorSpace;
    use usd_hio::image_reader::read_image_data;

    let mut handles = MaterialTextureHandles::new();
    let mut hgi = hgi_arc.write();

    for (input_name, raw_path) in tex_paths {
        let Some(slot) = usd_input_to_tex_slot(input_name.as_str()) else {
            log::debug!("[engine] no slot for input '{}' on {}", input_name, prim_id);
            continue;
        };

        // Resolve path
        let resolved = match resolve_tex_path(raw_path, stage) {
            Some(p) => p,
            None => {
                log::warn!(
                    "[engine] cannot resolve texture '{}' for {}",
                    raw_path,
                    prim_id
                );
                continue;
            }
        };

        // Diffuse textures are sRGB, others linear
        let is_diffuse = slot == 0;
        let color_space = if is_diffuse {
            SourceColorSpace::SRGB
        } else {
            SourceColorSpace::Raw
        };
        let cache_key = if is_diffuse {
            format!("{resolved}|srgb")
        } else {
            format!("{resolved}|raw")
        };

        if let Some((tex, smp)) = texture_cache.get(&cache_key) {
            handles.set_slot(slot, tex.clone(), smp.clone());
            continue;
        }

        // Load pixels via HIO
        let Some(img) = read_image_data(&resolved, color_space, false, false) else {
            log::warn!(
                "[engine] failed to load texture '{}' for slot {} on {}",
                resolved,
                slot,
                prim_id
            );
            continue;
        };

        let hgi_fmt = hio_format_to_hgi(img.format, img.is_srgb);

        // Full mip chain for trilinear filtering quality
        let mip_levels = calc_mip_levels(img.width as u32, img.height as u32);

        // SHADER_WRITE is required so the compute mip generator can bind individual
        // mip levels as storage textures (used by generate_mipmap internally).
        let tex_usage = if mip_levels > 1 {
            HgiTextureUsage::SHADER_READ | HgiTextureUsage::SHADER_WRITE
        } else {
            HgiTextureUsage::SHADER_READ
        };

        let desc = HgiTextureDesc::new()
            .with_debug_name(&resolved)
            .with_format(hgi_fmt)
            .with_usage(tex_usage)
            .with_mip_levels(mip_levels as u16)
            .with_dimensions(usd_gf::Vec3i::new(img.width, img.height, 1));

        let tex_handle = hgi.create_texture(&desc, Some(&img.pixels));

        // Generate mipmaps via compute if format is supported and we have > 1 level
        if mip_levels > 1 {
            let mut blit = hgi.create_blit_cmds();
            blit.generate_mipmap(&tex_handle);
            hgi.submit_cmds(blit, WgpuSubmitWait::NoWait);
        }

        // Create a basic linear/clamp sampler for this texture
        let smp_desc = HgiSamplerDesc {
            debug_name: resolved.clone(),
            mag_filter: HgiSamplerFilter::Linear,
            min_filter: HgiSamplerFilter::Linear,
            mip_filter: HgiMipFilter::Linear,
            address_mode_u: HgiSamplerAddressMode::ClampToEdge,
            address_mode_v: HgiSamplerAddressMode::ClampToEdge,
            address_mode_w: HgiSamplerAddressMode::ClampToEdge,
            ..HgiSamplerDesc::default()
        };
        let smp_handle = hgi.create_sampler(&smp_desc);
        texture_cache.insert(cache_key, (tex_handle.clone(), smp_handle.clone()));

        handles.set_slot(slot, tex_handle, smp_handle);
        log::info!(
            "[engine] loaded texture slot {} '{}' ({}x{} {:?}) for {}",
            slot,
            resolved,
            img.width,
            img.height,
            hgi_fmt,
            prim_id
        );
    }

    handles
}
