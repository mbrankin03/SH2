//! Dynamic PS2 room clear color (`SLUS_202.28` `kari_sh2setGS_clearcolor` `0x00175930` &
//! `SetupDrawEnvData` `0x00251370`).
//!
//! Ground-truth references (`SLUS_202.28`):
//! - `kari_sh2setGS_clearcolor` (`0x00175930` / `FUN_001759a8` `0x001759a8`): writes the 8-bit
//!   `(r, g, b, a)` clear RGBA to all 3 frame-buffer clear packets (`0x2ab990 + i * 0x2a0`).
//! - `SetupDrawEnvData` (`0x00251370`, `GFW/sh2_DrawEnvData.c` lines 357-399 & 429):
//!   - Day (`night_or_day == 0`) with `*(f32*)(env + 0x9c) == 0.0`: default `(0x46, 0x46, 0x46, 0x40)`.
//!   - Night (`night_or_day != 0`) with `*(f32*)(env + 0x9c) == 0.0`: default `(0x00, 0x00, 0x00, 0x7c)`.
//!   - Otherwise: room's `clear_color` from `env + 0x1a0`.

/// Default Day room GS clear color RGBA (`SLUS_202.28` `SetupDrawEnvData` `0x00251370` lines 358-361).
pub const PS2_DEFAULT_DAY_CLEAR_RGBA: [u8; 4] = [0x46, 0x46, 0x46, 0x40];

/// Default Night room GS clear color RGBA (`SLUS_202.28` `SetupDrawEnvData` `0x00251370` lines 386-389).
pub const PS2_DEFAULT_NIGHT_CLEAR_RGBA: [u8; 4] = [0x00, 0x00, 0x00, 0x7c];

/// Resolves the PS2 GS room clear color (`SetupDrawEnvData` `0x00251370` + `kari_sh2setGS_clearcolor`
/// `0x00175930`) into 8-bit RGBA bytes.
///
/// When `room_clear_rgba` is `None` (corresponding to `*(f32*)(env + 0x9c) == 0.0`), returns
/// [`PS2_DEFAULT_DAY_CLEAR_RGBA`] (`[0x46, 0x46, 0x46, 0x40]`) for Day or
/// [`PS2_DEFAULT_NIGHT_CLEAR_RGBA`] (`[0x00, 0x00, 0x00, 0x7c]`) for Night.
#[must_use]
pub const fn resolve_ps2_room_clear_rgba(
    is_night: bool,
    room_clear_rgba: Option<[u8; 4]>,
) -> [u8; 4] {
    match room_clear_rgba {
        Some(rgba) => rgba,
        None if is_night => PS2_DEFAULT_NIGHT_CLEAR_RGBA,
        None => PS2_DEFAULT_DAY_CLEAR_RGBA,
    }
}

/// Converts a PS2 GS 8-bit clear RGBA (`kari_sh2setGS_clearcolor` `0x00175930`) to a `wgpu::Color`.
#[must_use]
pub fn ps2_clear_rgba_to_wgpu(rgba: [u8; 4]) -> wgpu::Color {
    wgpu::Color {
        r: (rgba[0] as f64) / 255.0,
        g: (rgba[1] as f64) / 255.0,
        b: (rgba[2] as f64) / 255.0,
        a: 1.0,
    }
}

/// Packs `(r, g, b, a)` into the 64-bit PS2 GS `RGBAQ` clear register written by
/// `kari_sh2setGS_clearcolor` / `FUN_001759a8` (`0x001759a8` lines 73-76: `Q = 1.0 = 0x3f800000`).
#[must_use]
pub const fn pack_gs_clearcolor_rgbaq(rgba: [u8; 4]) -> u64 {
    (rgba[0] as u64)
        | ((rgba[1] as u64) << 8)
        | ((rgba[2] as u64) << 16)
        | ((rgba[3] as u64) << 24)
        | 0x3f80_0000_0000_0000
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn room_clear_color_matches_slus_00251370_and_00175930() {
        let day = resolve_ps2_room_clear_rgba(false, None);
        assert_eq!(day, [0x46, 0x46, 0x46, 0x40]);
        assert_eq!(pack_gs_clearcolor_rgbaq(day), 0x3f80_0000_4046_4646);

        let night = resolve_ps2_room_clear_rgba(true, None);
        assert_eq!(night, [0x00, 0x00, 0x00, 0x7c]);
        assert_eq!(pack_gs_clearcolor_rgbaq(night), 0x3f80_0000_7c00_0000);

        let custom = resolve_ps2_room_clear_rgba(true, Some([0x10, 0x20, 0x30, 0x80]));
        assert_eq!(custom, [0x10, 0x20, 0x30, 0x80]);
        let w = ps2_clear_rgba_to_wgpu(custom);
        assert!((w.r - 16.0 / 255.0).abs() < 1e-6);
        assert!((w.g - 32.0 / 255.0).abs() < 1e-6);
        assert!((w.b - 48.0 / 255.0).abs() < 1e-6);
        assert_eq!(w.a, 1.0);
    }
}
