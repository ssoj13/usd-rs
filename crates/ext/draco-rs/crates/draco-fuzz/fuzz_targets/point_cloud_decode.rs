//! Fuzz target: decode arbitrary bytes as Draco point cloud (ref: draco_pc_decoder_fuzzer.cc).
#![no_main]

use draco_bitstream::compression::decode::Decoder;
use draco_core::core::decoder_buffer::DecoderBuffer;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if data.is_empty() {
        return;
    }
    let mut buffer = DecoderBuffer::new();
    buffer.init(data);
    let mut decoder = Decoder::new();
    let _ = decoder.decode_point_cloud_from_buffer(&mut buffer);
});
