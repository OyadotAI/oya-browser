/**
 * Seeded randomness for persona devices: the same LCG and string hash as Oya's
 * (server/src/modules/personas/prng.ts), so one seed gives one device in both. JavaScript runs
 * the LCG in exact doubles masked to 32 bits; the product stays under 2^53, so wrapping u32
 * arithmetic yields the identical low 32 bits.
 */
use crate::constants::{HASH_SHIFT, LCG_INCREMENT, LCG_MULTIPLIER, UINT32_MAX};

/// A seeded LCG returning floats in [0, 1].
pub struct Prng {
    /// The generator's state, as JavaScript's `>>> 0` reads it.
    state: u32,
}

impl Prng {
    /// A generator positioned at `seed`.
    pub fn new(seed: u32) -> Self {
        Self { state: seed }
    }

    /// The next value: the same float JavaScript's `(s >>> 0) / 0xffffffff` gives.
    pub fn next_f64(&mut self) -> f64 {
        self.state = self
            .state
            .wrapping_mul(LCG_MULTIPLIER)
            .wrapping_add(LCG_INCREMENT);
        f64::from(self.state) / UINT32_MAX
    }

    /// One element of `items`, chosen as JavaScript's `arr[Math.floor(rng() * arr.length)]`.
    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        // ponytail: rng() may return exactly 1.0 (state 0xffffffff), which indexes past the end
        // in JavaScript too (undefined); clamp so Rust never panics where Oya returns undefined.
        let index = (self.next_f64() * items.len() as f64).floor() as usize;
        &items[index.min(items.len() - 1)]
    }
}

/// A 32-bit unsigned seed hashed from a string's UTF-16 units, as Oya's `seedFromString`.
pub fn seed_from_string(text: &str) -> u32 {
    text.encode_utf16().fold(0i32, |hash, unit| {
        (hash << HASH_SHIFT)
            .wrapping_sub(hash)
            .wrapping_add(i32::from(unit))
    }) as u32
}
