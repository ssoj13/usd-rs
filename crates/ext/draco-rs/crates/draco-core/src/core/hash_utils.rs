//! Hash utilities.
//! Reference: `_ref/draco/src/draco/core/hash_utils.h` + `.cc`.

/// C++-style hashing that mirrors `std::hash` behavior for primitive types.
pub trait CppHash {
    fn cpp_hash(&self) -> u64;
}

impl CppHash for i8 {
    fn cpp_hash(&self) -> u64 {
        *self as i64 as u64
    }
}
impl CppHash for u8 {
    fn cpp_hash(&self) -> u64 {
        *self as u64
    }
}
impl CppHash for i16 {
    fn cpp_hash(&self) -> u64 {
        *self as i64 as u64
    }
}
impl CppHash for u16 {
    fn cpp_hash(&self) -> u64 {
        *self as u64
    }
}
impl CppHash for i32 {
    fn cpp_hash(&self) -> u64 {
        *self as i64 as u64
    }
}
impl CppHash for u32 {
    fn cpp_hash(&self) -> u64 {
        *self as u64
    }
}
impl CppHash for i64 {
    fn cpp_hash(&self) -> u64 {
        *self as u64
    }
}
impl CppHash for u64 {
    fn cpp_hash(&self) -> u64 {
        *self
    }
}
impl CppHash for isize {
    fn cpp_hash(&self) -> u64 {
        *self as i64 as u64
    }
}
impl CppHash for usize {
    fn cpp_hash(&self) -> u64 {
        *self as u64
    }
}
impl CppHash for bool {
    fn cpp_hash(&self) -> u64 {
        if *self {
            1
        } else {
            0
        }
    }
}
impl CppHash for f32 {
    fn cpp_hash(&self) -> u64 {
        // Match std::hash behavior: -0.0 and 0.0 must hash identically.
        if *self == 0.0 {
            0
        } else {
            self.to_bits() as u64
        }
    }
}
impl CppHash for f64 {
    fn cpp_hash(&self) -> u64 {
        // Match std::hash behavior: -0.0 and 0.0 must hash identically.
        if *self == 0.0 {
            0
        } else {
            self.to_bits()
        }
    }
}

impl CppHash for &str {
    fn cpp_hash(&self) -> u64 {
        fingerprint_string(self.as_bytes())
    }
}

impl CppHash for String {
    fn cpp_hash(&self) -> u64 {
        fingerprint_string(self.as_bytes())
    }
}

pub fn hash_combine<T1: CppHash, T2: CppHash>(a: &T1, b: &T2) -> u64 {
    (a.cpp_hash() << 2) ^ (b.cpp_hash() << 1)
}

pub fn hash_combine_with<T: CppHash>(a: &T, hash: u64) -> u64 {
    hash ^ (a.cpp_hash() + 239)
}

pub fn hash_combine_u64(a: u64, b: u64) -> u64 {
    (a + 1013) ^ ((b + 107) << 1)
}

/// Will never return 1 or 0.
pub fn fingerprint_string(bytes: &[u8]) -> u64 {
    let seed: u64 = 0x8765_4321;
    let hash_loop_count = (bytes.len() / 8) + 1;
    let mut hash = seed;

    for i in 0..hash_loop_count {
        let off = i * 8;
        let num_chars_left = bytes.len().saturating_sub(off);
        let mut new_hash = seed;

        if num_chars_left > 7 {
            let s = &bytes[off..off + 8];
            new_hash = (s[0] as u64) << 56
                | (s[1] as u64) << 48
                | (s[2] as u64) << 40
                | (s[3] as u64) << 32
                | (s[4] as u64) << 24
                | (s[5] as u64) << 16
                | (s[6] as u64) << 8
                | (s[7] as u64);
        } else {
            for j in 0..num_chars_left {
                new_hash |= (bytes[off + j] as u64) << (64 - ((num_chars_left - j) * 8));
            }
        }

        hash = hash_combine_u64(new_hash, hash);
    }

    if hash < u64::MAX - 1 {
        hash += 2;
    }
    hash
}

pub struct HashArray;

impl HashArray {
    pub fn hash<T: CppHash, const N: usize>(arr: &[T; N]) -> u64 {
        let mut hash = 79u64;
        for item in arr.iter() {
            let value_hash = item.cpp_hash();
            hash = hash_combine_with(&value_hash, hash);
        }
        hash
    }
}
