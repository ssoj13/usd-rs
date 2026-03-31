// Port of testenv/fastCompression.cpp

use usd_tf::fast_compression::FastCompression;

// C++ uses { 'a', 'b', 'c', 'd' } with index formula (i ^ (i >> 3)) & 3
fn make_source(size: usize) -> Vec<u8> {
    let values: [u8; 4] = [b'a', b'b', b'c', b'd'];
    (0..size).map(|i| values[(i ^ (i >> 3)) & 3]).collect()
}

/// Compress `src`, decompress back, assert byte-for-byte equality.
/// Mirrors C++ `testRoundTrip(sz)`.
fn round_trip(size: usize) {
    let src = make_source(size);

    // Compress
    let compressed =
        FastCompression::compress(&src).unwrap_or_else(|e| panic!("compress({size}) failed: {e}"));

    // Decompress
    let decompressed = FastCompression::decompress(&compressed, size)
        .unwrap_or_else(|e| panic!("decompress({size}) failed: {e}"));

    assert_eq!(
        decompressed.len(),
        size,
        "decompressed size mismatch for input size {size}"
    );
    assert_eq!(
        src, decompressed,
        "round-trip content mismatch for input size {size}"
    );
}

// ---------------------------------------------------------------------------
// Small sizes — always fast, safe to run in unit tests
// ---------------------------------------------------------------------------

#[test]
fn roundtrip_3_bytes() {
    round_trip(3);
}

#[test]
fn roundtrip_3_plus_2_bytes() {
    round_trip(3 + 2);
}

#[test]
fn roundtrip_3_kib() {
    round_trip(3 * 1024);
}

#[test]
fn roundtrip_3_kib_plus_2267_bytes() {
    round_trip(3 * 1024 + 2267);
}

// ---------------------------------------------------------------------------
// Medium sizes — a few MiB, still quick
// ---------------------------------------------------------------------------

#[test]
fn roundtrip_3_mib() {
    round_trip(3 * 1024 * 1024);
}

#[test]
fn roundtrip_3_mib_plus_514229_bytes() {
    round_trip(3 * 1024 * 1024 + 514229);
}

#[test]
fn roundtrip_7_mib() {
    round_trip(7 * 1024 * 1024);
}

#[test]
fn roundtrip_7_mib_plus_514229_bytes() {
    round_trip(7 * 1024 * 1024 + 514229);
}

// ---------------------------------------------------------------------------
// Large sizes — skipped by default to keep CI fast; run with --ignored
// ---------------------------------------------------------------------------

/// C++ test: 2008 MiB — skipped unless explicitly requested.
#[test]
#[ignore]
fn roundtrip_2008_mib() {
    round_trip(2008 * 1024 * 1024);
}

#[test]
#[ignore]
fn roundtrip_2008_mib_plus_514229_bytes() {
    round_trip(2008 * 1024 * 1024 + 514229);
}

/// C++ test: 3 GiB — skipped unless explicitly requested.
#[test]
#[ignore]
fn roundtrip_3_gib() {
    round_trip(3 * 1024 * 1024 * 1024);
}

#[test]
#[ignore]
fn roundtrip_3_gib_plus_178656871_bytes() {
    round_trip(3 * 1024 * 1024 * 1024 + 178_656_871);
}
