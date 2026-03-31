//! Demo of usd_ri_pxr_imaging module usage.

use usd::usd_imaging::ri_pxr::{
    PxrAovLightAdapter, PxrCameraAPIAdapter, PxrDisplayFilterAdapter, PxrIntegratorAdapter,
    PxrSampleFilterAdapter, tokens,
};

fn main() {
    println!("UsdRiPxrImaging Demo\n");

    // Create adapters
    let camera_adapter = PxrCameraAPIAdapter::new();
    let light_adapter = PxrAovLightAdapter::new();
    let integrator_adapter = PxrIntegratorAdapter::new();
    let display_filter_adapter = PxrDisplayFilterAdapter::new();
    let sample_filter_adapter = PxrSampleFilterAdapter::new();

    println!("Created adapters:");
    println!("  - PxrCameraAPIAdapter: {:?}", camera_adapter);
    println!("  - PxrAovLightAdapter: {:?}", light_adapter);
    println!("  - PxrIntegratorAdapter: {:?}", integrator_adapter);
    println!("  - PxrDisplayFilterAdapter: {:?}", display_filter_adapter);
    println!("  - PxrSampleFilterAdapter: {:?}", sample_filter_adapter);

    // Test tokens
    println!("\nRenderMan Imaging Tokens:");
    println!("  info:source = {}", tokens::tokens::info_source());
    println!(
        "  usdVaryingExtent = {}",
        tokens::tokens::usd_varying_extent()
    );
    println!(
        "  pxrBarnLightFilter = {}",
        tokens::tokens::pxr_barn_light_filter()
    );

    println!("\nPrim Type Tokens:");
    println!("  projection = {}", tokens::prim_type_tokens::projection());

    println!("\nDemo completed successfully!");
}
