//! Deterministic 64-bit FNV-1a UI state fingerprinting for zero-copy static frame skipping (`SH2-177`).
//!
//! Each UI screen hashes all state that affects its rasterized pixels — visible selections,
//! active animation phases/timers, viewport dimensions, texture identities, and mouse cursor
//! coordinates — so `sh2-app` only clears, re-rasterizes, and uploads the `1920x1440` (11.06 MB)
//! RGBA texture when the fingerprint changes.

/// Lightweight, allocation-free deterministic 64-bit FNV-1a hasher for UI dirty-checking.
#[derive(Debug, Clone, Copy)]
pub struct UiFingerprintHasher {
    state: u64,
}

impl Default for UiFingerprintHasher {
    fn default() -> Self {
        Self::new()
    }
}

impl UiFingerprintHasher {
    /// FNV-1a 64-bit offset basis.
    const OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    /// FNV-1a 64-bit prime.
    const PRIME: u64 = 0x0100_0000_01b3;

    /// Creates a new hasher seeded with the FNV-1a 64-bit offset basis.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            state: Self::OFFSET_BASIS,
        }
    }

    /// Creates a new hasher seeded with a screen-specific domain tag so distinct screens
    /// never collide even if their internal states happen to be identical.
    #[must_use]
    pub const fn with_tag(tag: u64) -> Self {
        let mut h = Self::new();
        h.state ^= tag;
        h.state = h.state.wrapping_mul(Self::PRIME);
        h
    }

    /// Mixes a byte slice into the hash state.
    #[inline]
    pub fn write_bytes(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.state ^= u64::from(b);
            self.state = self.state.wrapping_mul(Self::PRIME);
        }
    }

    /// Mixes a single `u8` into the hash state.
    #[inline]
    pub fn write_u8(&mut self, v: u8) {
        self.state ^= u64::from(v);
        self.state = self.state.wrapping_mul(Self::PRIME);
    }

    /// Mixes a `bool` into the hash state.
    #[inline]
    pub fn write_bool(&mut self, v: bool) {
        self.write_u8(u8::from(v));
    }

    /// Mixes a `u16` (little-endian) into the hash state.
    #[inline]
    pub fn write_u16(&mut self, v: u16) {
        self.write_bytes(&v.to_le_bytes());
    }

    /// Mixes a `u32` (little-endian) into the hash state.
    #[inline]
    pub fn write_u32(&mut self, v: u32) {
        self.write_bytes(&v.to_le_bytes());
    }

    /// Mixes an `i32` (little-endian) into the hash state.
    #[inline]
    pub fn write_i32(&mut self, v: i32) {
        self.write_bytes(&v.to_le_bytes());
    }

    /// Mixes a `u64` (little-endian) into the hash state.
    #[inline]
    pub fn write_u64(&mut self, v: u64) {
        self.write_bytes(&v.to_le_bytes());
    }

    /// Mixes a `usize` (cast to `u64`) into the hash state.
    #[inline]
    pub fn write_usize(&mut self, v: usize) {
        self.write_u64(v as u64);
    }

    /// Mixes an `f32` via canonical IEEE-754 bits (normalizing `-0.0` to `+0.0`).
    #[inline]
    pub fn write_f32(&mut self, v: f32) {
        let bits = if v == 0.0 { 0u32 } else { v.to_bits() };
        self.write_u32(bits);
    }

    /// Mixes a length-prefixed UTF-8 string slice into the hash state.
    #[inline]
    pub fn write_str(&mut self, s: &str) {
        self.write_usize(s.len());
        self.write_bytes(s.as_bytes());
    }

    /// Mixes an optional texture identity `(pointer address, width, height)` into the hash state.
    #[inline]
    pub fn write_opt_texture(&mut self, tex: Option<(&[u8], usize, usize)>) {
        match tex {
            Some((rgba, w, h)) => {
                self.write_u8(1);
                self.write_usize(rgba.as_ptr() as usize);
                self.write_usize(rgba.len());
                self.write_usize(w);
                self.write_usize(h);
            }
            None => self.write_u8(0),
        }
    }

    /// Mixes an optional mouse cursor position (quantized to integer screen pixels) into the hash state.
    #[inline]
    pub fn write_opt_cursor(&mut self, cursor: Option<(f64, f64)>) {
        match cursor {
            Some((mx, my)) => {
                self.write_u8(1);
                self.write_i32(mx.round() as i32);
                self.write_i32(my.round() as i32);
            }
            None => self.write_u8(0),
        }
    }

    /// Returns the finalized 64-bit fingerprint.
    #[must_use]
    #[inline]
    pub const fn finish(&self) -> u64 {
        self.state
    }
}
