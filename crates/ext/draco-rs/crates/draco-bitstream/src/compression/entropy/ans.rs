//! Asymmetric Numeral Systems (rANS) coding utilities.
//! Reference: `_ref/draco/src/draco/compression/entropy/ans.h`.
//!
//! This module provides low-level rANS/ANS primitives used by symbol encoders
//! and bit coders throughout the compression pipeline.

use draco_core::core::divide::fastdiv;
use draco_core::{draco_dcheck, draco_dcheck_ge, draco_dcheck_lt};

pub const DRACO_ANS_DIVIDE_BY_MULTIPLY: bool = true;

pub type AnsP8 = u8;
pub const DRACO_ANS_P8_PRECISION: u32 = 256;
pub const DRACO_ANS_L_BASE: u32 = 4096;
pub const DRACO_ANS_IO_BASE: u32 = 256;

#[inline]
fn ans_divrem(dividend: u32, divisor: u32) -> (u32, u32) {
    if DRACO_ANS_DIVIDE_BY_MULTIPLY {
        let quotient = fastdiv(dividend, divisor as usize);
        let remainder = dividend - quotient * divisor;
        (quotient, remainder)
    } else {
        (dividend / divisor, dividend % divisor)
    }
}

#[inline]
fn ans_div(dividend: u32, divisor: u32) -> u32 {
    if DRACO_ANS_DIVIDE_BY_MULTIPLY {
        fastdiv(dividend, divisor as usize)
    } else {
        dividend / divisor
    }
}

#[inline]
unsafe fn mem_get_le16(ptr: *const u8) -> u32 {
    let b0 = *ptr as u32;
    let b1 = *ptr.add(1) as u32;
    (b1 << 8) | b0
}

#[inline]
unsafe fn mem_get_le24(ptr: *const u8) -> u32 {
    let b0 = *ptr as u32;
    let b1 = *ptr.add(1) as u32;
    let b2 = *ptr.add(2) as u32;
    (b2 << 16) | (b1 << 8) | b0
}

#[inline]
unsafe fn mem_get_le32(ptr: *const u8) -> u32 {
    let b0 = *ptr as u32;
    let b1 = *ptr.add(1) as u32;
    let b2 = *ptr.add(2) as u32;
    let b3 = *ptr.add(3) as u32;
    (b3 << 24) | (b2 << 16) | (b1 << 8) | b0
}

#[inline]
unsafe fn mem_put_le16(ptr: *mut u8, val: u32) {
    *ptr = (val & 0xFF) as u8;
    *ptr.add(1) = ((val >> 8) & 0xFF) as u8;
}

#[inline]
unsafe fn mem_put_le24(ptr: *mut u8, val: u32) {
    *ptr = (val & 0xFF) as u8;
    *ptr.add(1) = ((val >> 8) & 0xFF) as u8;
    *ptr.add(2) = ((val >> 16) & 0xFF) as u8;
}

#[inline]
unsafe fn mem_put_le32(ptr: *mut u8, val: u32) {
    *ptr = (val & 0xFF) as u8;
    *ptr.add(1) = ((val >> 8) & 0xFF) as u8;
    *ptr.add(2) = ((val >> 16) & 0xFF) as u8;
    *ptr.add(3) = ((val >> 24) & 0xFF) as u8;
}

#[derive(Clone, Copy, Debug)]
pub struct AnsCoder {
    pub buf: *mut u8,
    pub buf_offset: i32,
    pub state: u32,
}

impl AnsCoder {
    pub fn new() -> Self {
        Self {
            buf: std::ptr::null_mut(),
            buf_offset: 0,
            state: 0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct AnsDecoder {
    pub buf: *const u8,
    pub buf_offset: i32,
    pub state: u32,
}

impl AnsDecoder {
    pub fn new() -> Self {
        Self {
            buf: std::ptr::null(),
            buf_offset: 0,
            state: 0,
        }
    }
}

#[inline]
pub fn ans_write_init(ans: &mut AnsCoder, buf: *mut u8) {
    ans.buf = buf;
    ans.buf_offset = 0;
    ans.state = DRACO_ANS_L_BASE;
}

#[inline]
pub fn ans_write_end(ans: &mut AnsCoder) -> i32 {
    let state = ans.state - DRACO_ANS_L_BASE;
    draco_dcheck_ge!(ans.state, DRACO_ANS_L_BASE);
    draco_dcheck_lt!(ans.state, DRACO_ANS_L_BASE * DRACO_ANS_IO_BASE);
    unsafe {
        if state < (1 << 6) {
            *ans.buf.add(ans.buf_offset as usize) = ((0x00 << 6) + state) as u8;
            ans.buf_offset + 1
        } else if state < (1 << 14) {
            mem_put_le16(
                ans.buf.add(ans.buf_offset as usize),
                ((0x01 << 14) + state) as u32,
            );
            ans.buf_offset + 2
        } else if state < (1 << 22) {
            mem_put_le24(
                ans.buf.add(ans.buf_offset as usize),
                ((0x02 << 22) + state) as u32,
            );
            ans.buf_offset + 3
        } else {
            draco_dcheck!(false);
            ans.buf_offset
        }
    }
}

#[inline]
pub fn ans_read_init(ans: &mut AnsDecoder, buf: *const u8, offset: i32) -> i32 {
    if offset < 1 {
        return 1;
    }
    ans.buf = buf;
    unsafe {
        let x = *buf.add((offset - 1) as usize) >> 6;
        if x == 0 {
            ans.buf_offset = offset - 1;
            ans.state = (*buf.add((offset - 1) as usize) & 0x3F) as u32;
        } else if x == 1 {
            if offset < 2 {
                return 1;
            }
            ans.buf_offset = offset - 2;
            ans.state = mem_get_le16(buf.add((offset - 2) as usize)) & 0x3FFF;
        } else if x == 2 {
            if offset < 3 {
                return 1;
            }
            ans.buf_offset = offset - 3;
            ans.state = mem_get_le24(buf.add((offset - 3) as usize)) & 0x3FFFFF;
        } else if x == 3 {
            ans.buf_offset = offset - 4;
            ans.state = mem_get_le32(buf.add((offset - 4) as usize)) & 0x3FFFFFFF;
        } else {
            return 1;
        }
    }
    ans.state += DRACO_ANS_L_BASE;
    if ans.state >= DRACO_ANS_L_BASE * DRACO_ANS_IO_BASE {
        return 1;
    }
    0
}

#[inline]
pub fn ans_read_end(ans: &mut AnsDecoder) -> i32 {
    if ans.state == DRACO_ANS_L_BASE {
        1
    } else {
        0
    }
}

#[inline]
pub fn ans_reader_has_error(ans: &AnsDecoder) -> i32 {
    if ans.state < DRACO_ANS_L_BASE && ans.buf_offset == 0 {
        1
    } else {
        0
    }
}

// rABS with descending spread.
#[inline]
pub fn rabs_desc_write(ans: &mut AnsCoder, val: i32, p0: AnsP8) {
    let p = DRACO_ANS_P8_PRECISION - p0 as u32;
    let l_s = if val != 0 { p } else { p0 as u32 };
    if ans.state >= DRACO_ANS_L_BASE / DRACO_ANS_P8_PRECISION * DRACO_ANS_IO_BASE * l_s {
        unsafe {
            *ans.buf.add(ans.buf_offset as usize) = (ans.state % DRACO_ANS_IO_BASE) as u8;
        }
        ans.buf_offset += 1;
        ans.state /= DRACO_ANS_IO_BASE;
    }
    let (quot, rem) = ans_divrem(ans.state, l_s);
    ans.state = quot * DRACO_ANS_P8_PRECISION + rem + if val != 0 { 0 } else { p };
}

#[inline]
pub fn rabs_desc_read(ans: &mut AnsDecoder, p0: AnsP8) -> i32 {
    let p = DRACO_ANS_P8_PRECISION - p0 as u32;
    if ans.state < DRACO_ANS_L_BASE && ans.buf_offset > 0 {
        ans.buf_offset -= 1;
        unsafe {
            ans.state =
                ans.state * DRACO_ANS_IO_BASE + *ans.buf.add(ans.buf_offset as usize) as u32;
        }
    }
    let x = ans.state;
    let quot = x / DRACO_ANS_P8_PRECISION;
    let rem = x % DRACO_ANS_P8_PRECISION;
    let xn = quot * p;
    let val = rem < p;
    if val {
        ans.state = xn + rem;
        1
    } else {
        ans.state = x - xn - p;
        0
    }
}

// rABS with ascending spread.
#[inline]
pub fn rabs_asc_write(ans: &mut AnsCoder, val: i32, p0: AnsP8) {
    let p = DRACO_ANS_P8_PRECISION - p0 as u32;
    let l_s = if val != 0 { p } else { p0 as u32 };
    if ans.state >= DRACO_ANS_L_BASE / DRACO_ANS_P8_PRECISION * DRACO_ANS_IO_BASE * l_s {
        unsafe {
            *ans.buf.add(ans.buf_offset as usize) = (ans.state % DRACO_ANS_IO_BASE) as u8;
        }
        ans.buf_offset += 1;
        ans.state /= DRACO_ANS_IO_BASE;
    }
    let (quot, rem) = ans_divrem(ans.state, l_s);
    ans.state = quot * DRACO_ANS_P8_PRECISION + rem + if val != 0 { p0 as u32 } else { 0 };
}

#[inline]
pub fn rabs_asc_read(ans: &mut AnsDecoder, p0: AnsP8) -> i32 {
    let p = DRACO_ANS_P8_PRECISION - p0 as u32;
    if ans.state < DRACO_ANS_L_BASE {
        ans.buf_offset -= 1;
        unsafe {
            ans.state =
                ans.state * DRACO_ANS_IO_BASE + *ans.buf.add(ans.buf_offset as usize) as u32;
        }
    }
    let x = ans.state;
    let quot = x / DRACO_ANS_P8_PRECISION;
    let rem = x % DRACO_ANS_P8_PRECISION;
    let xn = quot * p;
    let val = rem >= p0 as u32;
    if val {
        ans.state = xn + rem - p0 as u32;
        1
    } else {
        ans.state = x - xn;
        0
    }
}

#[inline]
pub fn rabs_write(ans: &mut AnsCoder, val: i32, p0: AnsP8) {
    rabs_desc_write(ans, val, p0);
}

#[inline]
pub fn rabs_read(ans: &mut AnsDecoder, p0: AnsP8) -> i32 {
    rabs_desc_read(ans, p0)
}

// uABS with normalization.
#[inline]
pub fn uabs_write(ans: &mut AnsCoder, val: i32, p0: AnsP8) {
    let p = DRACO_ANS_P8_PRECISION - p0 as u32;
    let l_s = if val != 0 { p } else { p0 as u32 };
    while ans.state >= DRACO_ANS_L_BASE / DRACO_ANS_P8_PRECISION * DRACO_ANS_IO_BASE * l_s {
        unsafe {
            *ans.buf.add(ans.buf_offset as usize) = (ans.state % DRACO_ANS_IO_BASE) as u8;
        }
        ans.buf_offset += 1;
        ans.state /= DRACO_ANS_IO_BASE;
    }
    if val == 0 {
        ans.state = ans_div(ans.state * DRACO_ANS_P8_PRECISION, p0 as u32);
    } else {
        ans.state = ans_div((ans.state + 1) * DRACO_ANS_P8_PRECISION + p - 1, p) - 1;
    }
}

#[inline]
pub fn uabs_read(ans: &mut AnsDecoder, p0: AnsP8) -> i32 {
    let p = DRACO_ANS_P8_PRECISION - p0 as u32;
    let mut state = ans.state;
    while state < DRACO_ANS_L_BASE && ans.buf_offset > 0 {
        ans.buf_offset -= 1;
        unsafe {
            state = state * DRACO_ANS_IO_BASE + *ans.buf.add(ans.buf_offset as usize) as u32;
        }
    }
    let sp = state * p;
    let xp = sp / DRACO_ANS_P8_PRECISION;
    let s = (sp & 0xFF) >= p0 as u32;
    if s {
        ans.state = xp;
        1
    } else {
        ans.state = state - xp;
        0
    }
}

#[inline]
pub fn uabs_read_bit(ans: &mut AnsDecoder) -> i32 {
    let mut state = ans.state;
    while state < DRACO_ANS_L_BASE && ans.buf_offset > 0 {
        ans.buf_offset -= 1;
        unsafe {
            state = state * DRACO_ANS_IO_BASE + *ans.buf.add(ans.buf_offset as usize) as u32;
        }
    }
    let s = (state & 1) as i32;
    ans.state = state >> 1;
    s
}

#[derive(Clone, Copy, Debug, Default)]
pub struct RansSym {
    pub prob: u32,
    pub cum_prob: u32,
}

pub struct RAnsEncoder<const RANS_PRECISION_BITS: u32> {
    ans: AnsCoder,
}

impl<const RANS_PRECISION_BITS: u32> RAnsEncoder<RANS_PRECISION_BITS> {
    const RANS_PRECISION: u32 = 1u32 << RANS_PRECISION_BITS;
    const L_RANS_BASE: u32 = Self::RANS_PRECISION * 4;

    pub fn new() -> Self {
        Self {
            ans: AnsCoder::new(),
        }
    }

    pub fn write_init(&mut self, buf: *mut u8) {
        self.ans.buf = buf;
        self.ans.buf_offset = 0;
        self.ans.state = Self::L_RANS_BASE;
    }

    pub fn write_end(&mut self) -> i32 {
        let state = self.ans.state - Self::L_RANS_BASE;
        draco_dcheck_ge!(self.ans.state, Self::L_RANS_BASE);
        draco_dcheck_lt!(self.ans.state, Self::L_RANS_BASE * DRACO_ANS_IO_BASE);
        unsafe {
            if state < (1 << 6) {
                *self.ans.buf.add(self.ans.buf_offset as usize) = ((0x00 << 6) + state) as u8;
                self.ans.buf_offset + 1
            } else if state < (1 << 14) {
                mem_put_le16(
                    self.ans.buf.add(self.ans.buf_offset as usize),
                    ((0x01 << 14) + state) as u32,
                );
                self.ans.buf_offset + 2
            } else if state < (1 << 22) {
                mem_put_le24(
                    self.ans.buf.add(self.ans.buf_offset as usize),
                    ((0x02 << 22) + state) as u32,
                );
                self.ans.buf_offset + 3
            } else if state < (1 << 30) {
                mem_put_le32(
                    self.ans.buf.add(self.ans.buf_offset as usize),
                    ((0x03u32 << 30) + state) as u32,
                );
                self.ans.buf_offset + 4
            } else {
                draco_dcheck!(false);
                self.ans.buf_offset
            }
        }
    }

    pub fn rans_write(&mut self, sym: &RansSym) {
        let p = sym.prob;
        while self.ans.state >= Self::L_RANS_BASE / Self::RANS_PRECISION * DRACO_ANS_IO_BASE * p {
            unsafe {
                *self.ans.buf.add(self.ans.buf_offset as usize) =
                    (self.ans.state % DRACO_ANS_IO_BASE) as u8;
            }
            self.ans.buf_offset += 1;
            self.ans.state /= DRACO_ANS_IO_BASE;
        }
        self.ans.state =
            (self.ans.state / p) * Self::RANS_PRECISION + self.ans.state % p + sym.cum_prob;
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct RansDecSym {
    pub val: u32,
    pub prob: u32,
    pub cum_prob: u32,
}

pub struct RAnsDecoder<const RANS_PRECISION_BITS: u32> {
    lut_table: Vec<u32>,
    probability_table: Vec<RansSym>,
    ans: AnsDecoder,
}

impl<const RANS_PRECISION_BITS: u32> RAnsDecoder<RANS_PRECISION_BITS> {
    const RANS_PRECISION: u32 = 1u32 << RANS_PRECISION_BITS;
    const L_RANS_BASE: u32 = Self::RANS_PRECISION * 4;

    pub fn new() -> Self {
        Self {
            lut_table: Vec::new(),
            probability_table: Vec::new(),
            ans: AnsDecoder::new(),
        }
    }

    pub fn read_init(&mut self, buf: *const u8, offset: i32) -> i32 {
        if offset < 1 {
            return 1;
        }
        self.ans.buf = buf;
        unsafe {
            let x = *buf.add((offset - 1) as usize) >> 6;
            if x == 0 {
                self.ans.buf_offset = offset - 1;
                self.ans.state = (*buf.add((offset - 1) as usize) & 0x3F) as u32;
            } else if x == 1 {
                if offset < 2 {
                    return 1;
                }
                self.ans.buf_offset = offset - 2;
                self.ans.state = mem_get_le16(buf.add((offset - 2) as usize)) & 0x3FFF;
            } else if x == 2 {
                if offset < 3 {
                    return 1;
                }
                self.ans.buf_offset = offset - 3;
                self.ans.state = mem_get_le24(buf.add((offset - 3) as usize)) & 0x3FFFFF;
            } else if x == 3 {
                self.ans.buf_offset = offset - 4;
                self.ans.state = mem_get_le32(buf.add((offset - 4) as usize)) & 0x3FFFFFFF;
            } else {
                return 1;
            }
        }
        self.ans.state += Self::L_RANS_BASE;
        if self.ans.state >= Self::L_RANS_BASE * DRACO_ANS_IO_BASE {
            return 1;
        }
        0
    }

    pub fn read_end(&self) -> i32 {
        if self.ans.state == Self::L_RANS_BASE {
            1
        } else {
            0
        }
    }

    pub fn reader_has_error(&self) -> i32 {
        if self.ans.state < Self::L_RANS_BASE && self.ans.buf_offset == 0 {
            1
        } else {
            0
        }
    }

    pub fn rans_read(&mut self) -> u32 {
        while self.ans.state < Self::L_RANS_BASE && self.ans.buf_offset > 0 {
            self.ans.buf_offset -= 1;
            unsafe {
                self.ans.state = self.ans.state * DRACO_ANS_IO_BASE
                    + *self.ans.buf.add(self.ans.buf_offset as usize) as u32;
            }
        }
        let quo = self.ans.state / Self::RANS_PRECISION;
        let rem = self.ans.state % Self::RANS_PRECISION;
        let sym = self.fetch_sym(rem);
        self.ans.state = quo * sym.prob + rem - sym.cum_prob;
        sym.val
    }

    pub fn rans_build_look_up_table(&mut self, token_probs: &[u32], num_symbols: u32) -> bool {
        self.lut_table.resize(Self::RANS_PRECISION as usize, 0);
        self.probability_table
            .resize(num_symbols as usize, RansSym::default());
        let mut cum_prob: u32 = 0;
        let mut act_prob: u32 = 0;
        for i in 0..num_symbols {
            let prob = token_probs[i as usize];
            self.probability_table[i as usize].prob = prob;
            self.probability_table[i as usize].cum_prob = cum_prob;
            cum_prob = cum_prob.wrapping_add(prob);
            if cum_prob > Self::RANS_PRECISION {
                return false;
            }
            for j in act_prob..cum_prob {
                self.lut_table[j as usize] = i;
            }
            act_prob = cum_prob;
        }
        if cum_prob != Self::RANS_PRECISION {
            return false;
        }
        true
    }

    fn fetch_sym(&self, rem: u32) -> RansDecSym {
        let symbol = self.lut_table[rem as usize];
        let prob = self.probability_table[symbol as usize].prob;
        let cum_prob = self.probability_table[symbol as usize].cum_prob;
        RansDecSym {
            val: symbol,
            prob,
            cum_prob,
        }
    }
}
