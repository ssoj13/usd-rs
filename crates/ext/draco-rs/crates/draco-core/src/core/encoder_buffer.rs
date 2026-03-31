//! Encoder buffer utilities.
//! Reference: `_ref/draco/src/draco/core/encoder_buffer.h` + `.cc`.

use crate::core::varint_encoding::encode_varint;

pub struct EncoderBuffer {
    buffer: Vec<u8>,
    bit_encoder: Option<BitEncoder>,
    bit_encoder_reserved_bytes: i64,
    encode_bit_sequence_size: bool,
}

impl EncoderBuffer {
    pub fn new() -> Self {
        Self {
            buffer: Vec::new(),
            bit_encoder: None,
            bit_encoder_reserved_bytes: 0,
            encode_bit_sequence_size: false,
        }
    }

    pub fn clear(&mut self) {
        self.buffer.clear();
        self.bit_encoder_reserved_bytes = 0;
        self.bit_encoder = None;
    }

    pub fn resize(&mut self, nbytes: i64) {
        if nbytes < 0 {
            return;
        }
        self.buffer.resize(nbytes as usize, 0);
    }

    /// Start encoding a bit sequence. The maximum size must be known upfront.
    pub fn start_bit_encoding(&mut self, required_bits: i64, encode_size: bool) -> bool {
        if self.bit_encoder_active() {
            return false;
        }
        if required_bits <= 0 {
            return false;
        }
        self.encode_bit_sequence_size = encode_size;
        let required_bytes = (required_bits + 7) / 8;
        self.bit_encoder_reserved_bytes = required_bytes;

        let mut buffer_start_size = self.buffer.len() as i64;
        if encode_size {
            buffer_start_size += std::mem::size_of::<u64>() as i64;
        }
        let new_size = buffer_start_size + required_bytes;
        self.buffer.resize(new_size as usize, 0);

        // SAFETY: `bit_encoder_data` is a raw pointer into `self.buffer`.
        // It remains valid only because:
        // 1. `bit_encoder_active` prevents `encode()` calls that could reallocate
        // 2. No other mutations occur between start/end_bit_encoding
        // If the buffer were to reallocate, this pointer would dangle.
        let data_ptr = unsafe { self.buffer.as_mut_ptr().add(buffer_start_size as usize) };
        self.bit_encoder = Some(BitEncoder::new(data_ptr));
        true
    }

    /// Returns the active BitEncoder. Call only when bit_encoder_active() is true.
    #[inline]
    fn active_bit_encoder(&self) -> &BitEncoder {
        self.bit_encoder
            .as_ref()
            .expect("invariant: bit_encoder_active implies Some")
    }

    /// Returns the active BitEncoder mutably. Call only when bit_encoder_active() is true.
    #[inline]
    fn active_bit_encoder_mut(&mut self) -> &mut BitEncoder {
        self.bit_encoder
            .as_mut()
            .expect("invariant: bit_encoder_active implies Some")
    }

    /// End bit encoding and return to byte-aligned mode.
    pub fn end_bit_encoding(&mut self) {
        if !self.bit_encoder_active() {
            return;
        }
        let encoded_bits = self.active_bit_encoder().bits();
        let encoded_bytes = (encoded_bits + 7) / 8;

        if self.encode_bit_sequence_size {
            let buffer_len = self.buffer.len();
            let total_reserved =
                self.bit_encoder_reserved_bytes as usize + std::mem::size_of::<u64>();
            let out_mem_start = buffer_len - total_reserved;

            let mut var_size_buffer = EncoderBuffer::new();
            encode_varint(encoded_bytes as u64, &mut var_size_buffer);
            let size_len = var_size_buffer.size() as usize;

            let src_start = out_mem_start + std::mem::size_of::<u64>();
            let src_end = src_start + encoded_bytes as usize;
            let dst_start = out_mem_start + size_len;
            self.buffer.copy_within(src_start..src_end, dst_start);

            let size_bytes = var_size_buffer.data();
            self.buffer[out_mem_start..out_mem_start + size_len].copy_from_slice(size_bytes);

            self.bit_encoder_reserved_bytes += std::mem::size_of::<u64>() as i64 - size_len as i64;
        }

        let new_size = (self.buffer.len() as i64 - self.bit_encoder_reserved_bytes
            + encoded_bytes as i64) as usize;
        self.buffer.truncate(new_size);
        self.bit_encoder_reserved_bytes = 0;
        self.bit_encoder = None;
    }

    pub fn encode_least_significant_bits32(&mut self, nbits: i32, value: u32) -> bool {
        if !self.bit_encoder_active() {
            return false;
        }
        self.active_bit_encoder_mut().put_bits(value, nbits);
        true
    }

    pub fn encode<T: Copy>(&mut self, data: T) -> bool {
        if self.bit_encoder_active() {
            return false;
        }
        let ptr = &data as *const T as *const u8;
        let slice = unsafe { std::slice::from_raw_parts(ptr, std::mem::size_of::<T>()) };
        self.buffer.extend_from_slice(slice);
        true
    }

    pub fn encode_bytes(&mut self, data: &[u8]) -> bool {
        if self.bit_encoder_active() {
            return false;
        }
        self.buffer.extend_from_slice(data);
        true
    }

    pub fn bit_encoder_active(&self) -> bool {
        self.bit_encoder_reserved_bytes > 0
    }

    pub fn data(&self) -> &[u8] {
        &self.buffer
    }

    pub fn size(&self) -> usize {
        self.buffer.len()
    }

    pub fn buffer(&mut self) -> &mut Vec<u8> {
        assert!(
            !self.bit_encoder_active(),
            "Cannot access buffer while bit encoder is active"
        );
        &mut self.buffer
    }
}

impl Default for EncoderBuffer {
    fn default() -> Self {
        Self::new()
    }
}

pub(crate) struct BitEncoder {
    bit_buffer: *mut u8,
    bit_offset: usize,
}

impl BitEncoder {
    pub(crate) fn new(bit_buffer: *mut u8) -> Self {
        Self {
            bit_buffer,
            bit_offset: 0,
        }
    }

    pub(crate) fn put_bits(&mut self, data: u32, nbits: i32) {
        debug_assert!(nbits >= 0);
        debug_assert!(nbits <= 32);
        for bit in 0..nbits {
            let value = ((data >> bit) & 1) as u8;
            self.put_bit(value);
        }
    }

    pub(crate) fn bits(&self) -> u64 {
        self.bit_offset as u64
    }

    fn put_bit(&mut self, value: u8) {
        let byte_size = 8u64;
        let off = self.bit_offset as u64;
        let byte_offset = (off / byte_size) as isize;
        let bit_shift = (off % byte_size) as u8;
        unsafe {
            let cell = self.bit_buffer.offset(byte_offset);
            *cell &= !(1 << bit_shift);
            *cell |= value << bit_shift;
        }
        self.bit_offset += 1;
    }
}
