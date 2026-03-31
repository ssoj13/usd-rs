//! Shannon entropy estimation utilities.
//! Reference: `_ref/draco/src/draco/compression/entropy/shannon_entropy.h|cc`.
//!
//! Provides entropy estimation used to pick symbol coding strategies.

use crate::compression::entropy::rans_symbol_coding::approximate_rans_frequency_table_bits;

pub fn compute_shannon_entropy(
    symbols: &[u32],
    num_symbols: i32,
    max_value: i32,
    out_num_unique_symbols: Option<&mut i32>,
) -> i64 {
    let mut num_unique_symbols = 0;
    let mut frequencies = vec![0i32; (max_value + 1).max(0) as usize];
    for i in 0..num_symbols {
        let symbol = symbols[i as usize] as usize;
        frequencies[symbol] += 1;
    }
    let mut total_bits = 0.0f64;
    let num_symbols_d = num_symbols as f64;
    for i in 0..=max_value {
        let freq = frequencies[i as usize];
        if freq > 0 {
            num_unique_symbols += 1;
            total_bits += (freq as f64) * ((freq as f64) / num_symbols_d).log2();
        }
    }
    if let Some(out) = out_num_unique_symbols {
        *out = num_unique_symbols;
    }
    (-total_bits) as i64
}

pub fn compute_binary_shannon_entropy(num_values: u32, num_true_values: u32) -> f64 {
    if num_values == 0 {
        return 0.0;
    }
    if num_true_values == 0 || num_values == num_true_values {
        return 0.0;
    }
    let true_freq = num_true_values as f64 / num_values as f64;
    let false_freq = 1.0 - true_freq;
    -(true_freq * true_freq.log2() + false_freq * false_freq.log2())
}

#[derive(Clone, Copy, Debug)]
pub struct EntropyData {
    pub entropy_norm: f64,
    pub num_values: i32,
    pub max_symbol: i32,
    pub num_unique_symbols: i32,
}

impl Default for EntropyData {
    fn default() -> Self {
        Self {
            entropy_norm: 0.0,
            num_values: 0,
            max_symbol: 0,
            num_unique_symbols: 0,
        }
    }
}

pub struct ShannonEntropyTracker {
    frequencies: Vec<i32>,
    entropy_data: EntropyData,
}

impl ShannonEntropyTracker {
    pub fn new() -> Self {
        Self {
            frequencies: Vec::new(),
            entropy_data: EntropyData::default(),
        }
    }

    pub fn peek(&mut self, symbols: &[u32], num_symbols: i32) -> EntropyData {
        self.update_symbols(symbols, num_symbols, false)
    }

    pub fn push(&mut self, symbols: &[u32], num_symbols: i32) -> EntropyData {
        self.update_symbols(symbols, num_symbols, true)
    }

    pub fn get_number_of_data_bits(entropy_data: &EntropyData) -> i64 {
        if entropy_data.num_values < 2 {
            return 0;
        }
        ((entropy_data.num_values as f64) * (entropy_data.num_values as f64).log2()
            - entropy_data.entropy_norm)
            .ceil() as i64
    }

    /// Returns the number of data bits for the stream pushed so far (ref: GetNumberOfDataBits).
    pub fn current_number_of_data_bits(&self) -> i64 {
        Self::get_number_of_data_bits(&self.entropy_data)
    }

    pub fn get_number_of_rans_table_bits(entropy_data: &EntropyData) -> i64 {
        approximate_rans_frequency_table_bits(
            entropy_data.max_symbol + 1,
            entropy_data.num_unique_symbols,
        )
    }

    fn update_symbols(
        &mut self,
        symbols: &[u32],
        num_symbols: i32,
        push_changes: bool,
    ) -> EntropyData {
        let mut ret_data = self.entropy_data;
        ret_data.num_values += num_symbols;
        for i in 0..num_symbols {
            let symbol = symbols[i as usize] as usize;
            if self.frequencies.len() <= symbol {
                self.frequencies.resize(symbol + 1, 0);
            }
            let frequency = &mut self.frequencies[symbol];
            let mut old_symbol_entropy_norm = 0.0f64;
            if *frequency > 1 {
                old_symbol_entropy_norm = (*frequency as f64) * (*frequency as f64).log2();
            } else if *frequency == 0 {
                ret_data.num_unique_symbols += 1;
                if symbol as i32 > ret_data.max_symbol {
                    ret_data.max_symbol = symbol as i32;
                }
            }
            *frequency += 1;
            let new_symbol_entropy_norm = (*frequency as f64) * (*frequency as f64).log2();
            ret_data.entropy_norm += new_symbol_entropy_norm - old_symbol_entropy_norm;
        }
        if push_changes {
            self.entropy_data = ret_data;
        } else {
            for i in 0..num_symbols {
                let symbol = symbols[i as usize] as usize;
                self.frequencies[symbol] -= 1;
            }
        }
        ret_data
    }
}
