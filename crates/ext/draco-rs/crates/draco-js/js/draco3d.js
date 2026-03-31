// draco3d-style Node entrypoint for the Rust/WASM Draco bindings.

'use strict';

const createEncoderModule = require('./draco_encoder_nodejs');
const createDecoderModule = require('./draco_decoder_nodejs');

module.exports = {
  createEncoderModule,
  createDecoderModule,
};
