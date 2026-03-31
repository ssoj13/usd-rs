//! Standalone corpus runner for mesh_decode (no libFuzzer). Use when cargo fuzz is unavailable
//! (e.g. Windows without matching ASan DLL). Reads all files from a directory and runs the decoder.

use draco_bitstream::compression::decode::Decoder;
use draco_core::core::decoder_buffer::DecoderBuffer;
use std::env;
use std::fs;
use std::path::Path;

fn main() {
    let args: Vec<String> = env::args().collect();
    let corpus_dir = args
        .get(1)
        .map(|s| s.as_str())
        .unwrap_or("fuzz/corpus/mesh_decode");
    let path = Path::new(corpus_dir);
    if !path.is_dir() {
        eprintln!("Usage: mesh_decode_corpus [CORPUS_DIR]");
        eprintln!("  CORPUS_DIR defaults to fuzz/corpus/mesh_decode");
        std::process::exit(1);
    }
    let mut total = 0u64;
    let mut errors = 0u64;
    for entry in fs::read_dir(path).expect("read_dir") {
        let entry = entry.expect("entry");
        let p = entry.path();
        if p.is_file() {
            total += 1;
            if let Ok(data) = fs::read(&p) {
                if data.is_empty() {
                    continue;
                }
                let mut buffer = DecoderBuffer::new();
                buffer.init(&data);
                let mut decoder = Decoder::new();
                if !decoder.decode_mesh_from_buffer(&mut buffer).is_ok() {
                    errors += 1;
                }
            }
        }
    }
    eprintln!("corpus: {} files, {} decode errors", total, errors);
}
