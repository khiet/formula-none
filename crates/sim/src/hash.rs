//! A platform-independent hash of sim state, so the native and wasm builds
//! can be compared (ADR 0002). `std`'s default hasher is
//! deliberately unstable across versions, which would make every fixture a
//! moving target; FNV-1a over the raw bytes never changes.

use std::hash::Hasher;

/// 64-bit FNV-1a. Floats are fed as their IEEE bit patterns, so two states
/// hash equal exactly when every float is bit-identical: `-0.0` and `0.0`
/// differ, as do two NaNs with different payloads. That strictness is the
/// point; a renderer-invisible difference still means the platforms diverged.
pub(crate) struct Fnv1a(u64);

impl Fnv1a {
    const OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;

    pub(crate) fn new() -> Self {
        Self(Self::OFFSET_BASIS)
    }

    pub(crate) fn write_f32(&mut self, value: f32) {
        self.write_u32(value.to_bits());
    }

    pub(crate) fn write_f32s(&mut self, values: impl IntoIterator<Item = f32>) {
        for value in values {
            self.write_f32(value);
        }
    }
}

impl Hasher for Fnv1a {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.0 ^= u64::from(byte);
            self.0 = self.0.wrapping_mul(Self::PRIME);
        }
    }
}

/// A number in [0, 1) that only `seed` and `mixed` decide: an integer hash,
/// so it is the same natively and in wasm and draws nothing from the race's
/// item generator. `mixed` packs whatever else picks the draw.
pub(crate) fn unit(seed: u64, mixed: u64) -> f32 {
    let mut hash = seed ^ mixed.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    // The SplitMix64 finaliser.
    hash = (hash ^ (hash >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    hash = (hash ^ (hash >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    hash ^= hash >> 31;
    // The top 24 bits, which an f32 holds exactly.
    (hash >> 40) as f32 / (1u32 << 24) as f32
}
