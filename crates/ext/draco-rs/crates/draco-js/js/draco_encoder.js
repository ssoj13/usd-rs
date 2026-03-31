// Browser loader for Draco WASM encoder/decoder module.
// Uses the same module as the decoder; kept for parity with draco3d naming.

import DracoDecoderModule from './draco_decoder.js';

export async function DracoEncoderModule(options = {}) {
  return DracoDecoderModule(options);
}

export default DracoEncoderModule;
