// Dome Light IBL Compute Shaders - Combined reference file
//
// Copyright 2025 Pixar (WGSL port)
//
// This file contains all 4 dome light IBL compute kernels for reference.
// At runtime, each kernel is compiled as a separate WGSL module because
// they use different binding layouts at the same group/binding indices.
// See dome_light_computations.rs for the split WGSL source strings.
//
// Kernels:
//   1. latlong_to_cubemap  - equirectangular -> cubemap
//   2. irradiance_conv     - cubemap -> diffuse irradiance cubemap
//   3. prefilter_ggx       - cubemap -> specular prefilter (per-roughness mip)
//   4. brdf_integration    - NdotV x roughness -> BRDF split-sum LUT
//
// Cubemap layout: texture_2d_array<f32> with 6 layers.
// Workgroup size: 8x8 threads.
// OpenGL left-handed cubemap convention (matches C++ reference).
//
// NOTE: This file is NOT compiled directly. It exists for documentation.
//       The actual per-kernel WGSL sources are in dome_light_computations.rs.
