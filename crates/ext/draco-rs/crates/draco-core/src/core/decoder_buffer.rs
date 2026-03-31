//! Decoder buffer utilities.
//! Reference: `_ref/draco/src/draco/core/decoder_buffer.h` + `.cc`.

use crate::core::varint_decoding::decode_varint;

/// Converts Draco major/minor into a bitstream version (same as DRACO_BITSTREAM_VERSION).
#[inline]
fn bitstream_version(major: u8, minor: u8) -> u16 {
    ((major as u16) << 8) | minor as u16
}

const BACKWARDS_COMPATIBILITY_SUPPORTED: bool = true;

pub struct DecoderBuffer<'a> {
    data: &'a [u8],
    pos: usize,
    bit_decoder: BitDecoder<'a>,
    bit_mode: bool,
    bitstream_version: u16,
}

impl<'a> DecoderBuffer<'a> {
    pub fn new() -> Self {
        Self {
            data: &[],
            pos: 0,
            bit_decoder: BitDecoder::new(),
            bit_mode: false,
            bitstream_version: 0,
        }
    }

    /// Sets the buffer's internal data. No copy is made.
    pub fn init(&mut self, data: &'a [u8]) {
        self.init_with_version(data, self.bitstream_version);
    }

    /// Sets the buffer's internal data with a bitstream version.
    pub fn init_with_version(&mut self, data: &'a [u8], version: u16) {
        self.data = data;
        self.pos = 0;
        self.bitstream_version = version;
    }

    /// Starts decoding a bit sequence.
    pub fn start_bit_decoding(&mut self, decode_size: bool, out_size: &mut u64) -> bool {
        if decode_size {
            if BACKWARDS_COMPATIBILITY_SUPPORTED && self.bitstream_version < bitstream_version(2, 2)
            {
                if !self.decode(out_size) {
                    return false;
                }
            } else {
                if !decode_varint(out_size, self) {
                    return false;
                }
            }
        }
        self.bit_mode = true;
        let head = self.data_head();
        self.bit_decoder.reset(head);
        true
    }

    /// Ends the decoding of the bit sequence and return to byte-aligned decoding.
    pub fn end_bit_decoding(&mut self) {
        self.bit_mode = false;
        let bits_decoded = self.bit_decoder.bits_decoded();
        let bytes_decoded = (bits_decoded + 7) / 8;
        self.pos = self.pos.saturating_add(bytes_decoded as usize);
    }

    /// Decodes up to 32 bits into out_val. Only valid in bit decoding mode.
    pub fn decode_least_significant_bits32(&mut self, nbits: u32, out_value: &mut u32) -> bool {
        if !self.bit_decoder_active() {
            return false;
        }
        self.bit_decoder.get_bits(nbits, out_value)
    }

    /// Decodes a POD value.
    pub fn decode<T: Copy>(&mut self, out_val: &mut T) -> bool {
        if !self.peek(out_val) {
            return false;
        }
        self.pos += std::mem::size_of::<T>();
        true
    }

    pub fn decode_bytes(&mut self, out_data: &mut [u8]) -> bool {
        let size_to_decode = out_data.len();
        if self.pos + size_to_decode > self.data.len() {
            return false;
        }
        out_data.copy_from_slice(&self.data[self.pos..self.pos + size_to_decode]);
        self.pos += size_to_decode;
        true
    }

    /// Peeks a POD value without advancing.
    pub fn peek<T: Copy>(&self, out_val: &mut T) -> bool {
        let size_to_decode = std::mem::size_of::<T>();
        if self.pos + size_to_decode > self.data.len() {
            return false;
        }
        let ptr = unsafe { self.data.as_ptr().add(self.pos) as *const T };
        unsafe {
            *out_val = ptr.read_unaligned();
        }
        true
    }

    pub fn peek_bytes(&self, out_data: &mut [u8]) -> bool {
        let size_to_peek = out_data.len();
        if self.pos + size_to_peek > self.data.len() {
            return false;
        }
        out_data.copy_from_slice(&self.data[self.pos..self.pos + size_to_peek]);
        true
    }

    /// Advances (or rewinds if negative) the buffer position by `bytes`.
    pub fn advance(&mut self, bytes: i64) {
        self.pos = (self.pos as i64 + bytes) as usize;
    }

    /// Moves the parsing position to a specific offset.
    pub fn start_decoding_from(&mut self, offset: i64) {
        if offset < 0 {
            return;
        }
        self.pos = offset as usize;
    }

    pub fn set_bitstream_version(&mut self, version: u16) {
        self.bitstream_version = version;
    }

    pub fn data_head(&self) -> &'a [u8] {
        if self.pos >= self.data.len() {
            return &[];
        }
        &self.data[self.pos..]
    }

    pub fn data(&self) -> &'a [u8] {
        self.data
    }

    pub fn position(&self) -> usize {
        self.pos
    }

    pub fn remaining_size(&self) -> i64 {
        self.data.len().saturating_sub(self.pos) as i64
    }

    pub fn decoded_size(&self) -> i64 {
        self.pos as i64
    }

    pub fn bit_decoder_active(&self) -> bool {
        self.bit_mode
    }

    pub fn bitstream_version(&self) -> u16 {
        self.bitstream_version
    }
}

impl Default for DecoderBuffer<'_> {
    fn default() -> Self {
        Self::new()
    }
}

/// Internal helper class to decode bits from a bit buffer.
pub(crate) struct BitDecoder<'a> {
    bit_buffer: &'a [u8],
    bit_offset: usize,
}

impl<'a> BitDecoder<'a> {
    pub(crate) fn new() -> Self {
        Self {
            bit_buffer: &[],
            bit_offset: 0,
        }
    }

    pub(crate) fn reset(&mut self, data: &'a [u8]) {
        self.bit_buffer = data;
        self.bit_offset = 0;
    }

    pub(crate) fn bits_decoded(&self) -> u64 {
        self.bit_offset as u64
    }

    fn avail_bits(&self) -> u64 {
        (self.bit_buffer.len() as u64 * 8).saturating_sub(self.bit_offset as u64)
    }

    fn ensure_bits(&self, k: u32) -> u32 {
        debug_assert!(k <= 24);
        debug_assert!((k as u64) <= self.avail_bits());
        let mut buf = 0u32;
        for i in 0..k {
            buf |= (self.peek_bit(i as usize) as u32) << i;
        }
        buf
    }

    fn consume_bits(&mut self, k: usize) {
        self.bit_offset = self.bit_offset.saturating_add(k);
    }

    pub(crate) fn get_bits(&mut self, nbits: u32, out_value: &mut u32) -> bool {
        if nbits > 32 {
            return false;
        }
        if nbits == 0 {
            *out_value = 0;
            return true;
        }
        if nbits <= 24 {
            if (nbits as u64) > self.avail_bits() {
                return false;
            }
            let value = self.ensure_bits(nbits);
            self.consume_bits(nbits as usize);
            *out_value = value;
            return true;
        }
        let mut value = 0u32;
        for bit in 0..nbits {
            value |= (self.get_bit() as u32) << bit;
        }
        *out_value = value;
        true
    }

    fn get_bit(&mut self) -> u8 {
        let off = self.bit_offset;
        let byte_offset = off >> 3;
        let bit_shift = (off & 0x7) as u8;
        if byte_offset < self.bit_buffer.len() {
            let bit = (self.bit_buffer[byte_offset] >> bit_shift) & 1;
            self.bit_offset = off + 1;
            return bit;
        }
        0
    }

    fn peek_bit(&self, offset: usize) -> u8 {
        let off = self.bit_offset.saturating_add(offset);
        let byte_offset = off >> 3;
        let bit_shift = (off & 0x7) as u8;
        if byte_offset < self.bit_buffer.len() {
            return (self.bit_buffer[byte_offset] >> bit_shift) & 1;
        }
        0
    }
}
