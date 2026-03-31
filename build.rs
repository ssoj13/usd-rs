//! Build script for usd-rs.
//!
//! Declares custom cfg names for conditional compilation (e.g. sanitize_address
//! used when building with -Z sanitizer=address).

fn main() {
    println!("cargo::rustc-check-cfg=cfg(sanitize_address)");
}
