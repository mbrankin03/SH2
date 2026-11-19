//! Per-room PS2 `DrawEnvData` lighting & atmosphere lookup (`GFW/sh2_DrawEnvData.c`,
//! `SLUS_202.28` `Set_DrawEnvData` @ `0x00251060`, `SetupDrawEnvData` @ `0x00251370`,
//! `Get_NowDrawEnvData` @ `0x00250e30`).

use super::draw_env_tables::ALL_STAGE_DRAW_ENV_TABLES;

/// Event flag index (`625`) indicating the Wood Side Room 205 mannequin flashlight pickup
/// (`sh2pc.exe` `0x50a010` / `env_flash_off_level`).
pub const FLAG_WOODSIDE_FLASHLIGHT_PICKUP: u16 = 625;

/// Room ambient scale before picking up the flashlight in Wood Side Apartments Room 205 (`0x50a010`).
pub const ENV_FLASH_OFF_PRE_PICKUP: f32 = 1.0;

/// Room ambient scale after picking up the flashlight (or when Flag `625` is set) (`0x50a010`).
pub const ENV_FLASH_OFF_POST_PICKUP: f32 = 0.3;

/// One of the 3 directional lights embedded in `DrawEnvData` (`+0x10..+0x6f`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DrawEnvDirectionalLight {
    /// Light direction vector in PS2 world space (Y-down).
    pub direction: [f32; 3],
    /// Linear RGB light color.
    pub color: [f32; 3],
}

/// One of the up to 4 room-local point/fake lights embedded in `DrawEnvData` (`+0xb0..+0x1af`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DrawEnvPointLight {
    /// Linear RGB light color.
    pub color: [f32; 3],
    /// Near attenuation distance in PS2 mm.
    pub near_dist: f32,
    /// Far attenuation cutoff distance in PS2 mm.
    pub far_dist: f32,
    /// World-space light position in PS2 mm (Y-down).
    pub position: [f32; 3],
}

impl DrawEnvPointLight {
    /// Returns `true` when this point light slot has non-zero color and a positive far radius.
    #[inline]
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.far_dist > 1.0
            && (self.color[0].abs() > 1e-4
                || self.color[1].abs() > 1e-4
                || self.color[2].abs() > 1e-4)
    }
}

/// Authentic PS2 `DrawEnvData` (`0x1f0` = 496 bytes in `SLUS_202.28` `GFW/sh2_DrawEnvData.c`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DrawEnvData {
    /// First room index covered by this entry (`0xFFFF` = wildcard/default for the stage).
    pub room_start: u16,
    /// Last room index covered by this entry (`0xFFFF` = wildcard/default).
    pub room_end: u16,
    /// Stage environment flags (`+0x04`).
    pub flags: u32,
    /// Room shadow darkness byte (`+0x09`; `0` triggers floor drop-shadow flattening instead of
    /// spotlight stencil shadow volumes, `sh2pc.exe` `0x5a7c90`).
    pub shadow_darkness: u8,
    /// Framebuffer clear color RGB (`+0x08..+0x0b`).
    pub clear_color_rgb: [u8; 3],
    /// 3 directional lights (`+0x10..+0x6f`).
    pub directional_lights: [DrawEnvDirectionalLight; 3],
    /// Flashlight spotlight RGB color (`+0x70..+0x7b`).
    pub flashlight_color: [f32; 3],
    /// Flashlight inner attenuation distance in PS2 mm (`+0x7c`, typically `1500.0`).
    pub flashlight_inner_dist: f32,
    /// Flashlight start distance in PS2 mm (`+0x80`, typically `1500.0`).
    pub flashlight_start_dist: f32,
    /// Flashlight maximum reach in PS2 mm (`+0x88`, `6000.0` on PS2, `8500.0` with SH2:EE).
    pub flashlight_range: f32,
    /// Flashlight outer cone half-angle in radians (`+0x8c`, typically `0.785` = $\pi/4$).
    pub flashlight_outer_rad: f32,
    /// Camera near clip plane in PS2 mm (`+0x98`).
    pub clip_near: f32,
    /// Camera far clip plane in PS2 mm (`+0x94`).
    pub clip_far: f32,
    /// Fog near distance in PS2 mm (`+0xa4`).
    pub fog_near: f32,
    /// Fog far distance in PS2 mm (`+0xa0`).
    pub fog_far: f32,
    /// Up to 4 room-local point/fake lights (`+0xb0..+0x1af`).
    pub point_lights: [DrawEnvPointLight; 4],
    /// Room ambient color RGB (`+0x1b0..+0x1bb`, e.g. `0.9` in OB, `0.65` in ER, `0.15` in AP).
    pub ambient: [f32; 3],
    /// Ambient scale / fog parameter (`+0x1bc`).
    pub ambient_scale: f32,
    /// Base ambient floor color RGB (`+0x1c0..+0x1cb`, typically `0.045`).
    pub base_ambient: [f32; 3],
    /// Base world color RGB (`+0x1d0..+0x1db`, typically `22.4`).
    pub base_color: [f32; 3],
    /// Vertex ambient multiplier RGB (`+0x1e0..+0x1eb`, `4.0` default; patched by `CatacombsMeatRoomFix`).
    pub vertex_ambient: [f32; 3],
    /// Vertex ambient W / alpha (`+0x1ec`).
    pub vertex_ambient_w: f32,
}

impl DrawEnvData {
    /// Returns `true` when this room uses flattened floor drop shadows rather than spotlight
    /// stencil volumes (`shadow_darkness == 0`, `sh2pc.exe` `0x5a7c90`).
    #[inline]
    #[must_use]
    pub const fn uses_drop_shadow_mode(&self) -> bool {
        self.shadow_darkness == 0
    }
}

/// Evaluates `env_flash_off_level` (`sh2pc.exe` `0x50a010`):
/// - Returns `1.0` before James picks up the flashlight in Wood Side Apartments Room 205.
/// - Returns `0.3` once the flashlight has been acquired or event Flag `625` is set.
#[inline]
#[must_use]
pub const fn env_flash_off_level(has_flashlight: bool, flag_625_set: bool) -> f32 {
    if has_flashlight || flag_625_set {
        ENV_FLASH_OFF_POST_PICKUP
    } else {
        ENV_FLASH_OFF_PRE_PICKUP
    }
}

/// Looks up the matching [`DrawEnvData`] entry for a 2-letter PS2 stage code (e.g. `"OB"`, `"ER"`,
/// `"AP"`, `"HP"`, `"PS"`, `"RR"`) and optional room index (`SLUS_202.28` `Set_DrawEnvData` @ `0x00251060`).
///
/// Matches exact room range (`room_start <= room_idx <= room_end`) first, then falls back to
/// wildcard (`0xFFFF`) or the first entry in that stage table.
#[must_use]
pub fn lookup_draw_env(stage_code: &str, room_index: u16) -> Option<&'static DrawEnvData> {
    let table = ALL_STAGE_DRAW_ENV_TABLES
        .iter()
        .find(|t| t.stage_code.eq_ignore_ascii_case(stage_code))?;

    // 1. Exact room-range match (excluding 0xFFFF wildcard)
    if let Some(exact) = table.entries.iter().find(|e| {
        if e.room_start == 0xFFFF {
            return false;
        }
        let lo = e.room_start.min(e.room_end);
        let hi = e.room_start.max(e.room_end);
        room_index >= lo && room_index <= hi
    }) {
        return Some(exact);
    }

    // 2. Stage-wide wildcard (0xFFFF) or default entry 0
    table
        .entries
        .iter()
        .find(|e| e.room_start == 0xFFFF)
        .or_else(|| table.entries.first())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_flash_off_level_matches_0x50a010() {
        assert_eq!(env_flash_off_level(false, false), 1.0);
        assert_eq!(env_flash_off_level(true, false), 0.3);
        assert_eq!(env_flash_off_level(false, true), 0.3);
        assert_eq!(env_flash_off_level(true, true), 0.3);
    }

    #[test]
    fn lookup_draw_env_resolves_ob_er_ap_from_slus_202_28() {
        let ob = lookup_draw_env("OB", 0).expect("OB_DrawEnvData");
        assert!((ob.ambient[0] - 0.9).abs() < 1e-4);
        assert!((ob.flashlight_range - 6000.0).abs() < 1e-2);
        assert!((ob.fog_far - 33000.0).abs() < 1e-2);

        let er = lookup_draw_env("ER", 6).expect("ER_DrawEnvData");
        assert!((er.ambient[0] - 0.65).abs() < 1e-4);
        assert!(er.point_lights[0].is_active());

        let ap = lookup_draw_env("AP", 12).expect("AP_DrawEnvData");
        assert!((ap.ambient[0] - 0.15).abs() < 1e-4);
        assert_eq!(ap.shadow_darkness, 1);
    }
}
