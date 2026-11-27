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
    /// Target room number (`+0x00`, or `0xFFFF` for stage-wide wildcard/default).
    pub room_no: u16,
    /// PS2 stage identifier (`+0x02`, e.g. `6` = ER, `9` = AP, `0xFFFF` = wildcard).
    pub stage_id: u16,
    /// Stage environment flags (`+0x04..+0x06`).
    pub flags: u32,
    /// Number of populated room point lights in [`Self::point_lights`] (`+0x07`, `0..=5`).
    pub point_light_count: u8,
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
    /// Flashlight outer cone half-angle in radians (`+0x8c`, typically `0.785398` = $\pi/4$).
    pub flashlight_outer_rad: f32,
    /// Camera near clip plane in PS2 mm (`+0x98`).
    pub clip_near: f32,
    /// Camera far clip plane in PS2 mm (`+0x94`).
    pub clip_far: f32,
    /// Fog near distance in PS2 mm (`+0xa4`).
    pub fog_near: f32,
    /// Fog far distance in PS2 mm (`+0xa0`).
    pub fog_far: f32,
    /// Fog maximum alpha/density floor in PS2 GS 0..255 units (`+0xa8`, `DAT_002adac8`,
    /// `SetupDrawEnvData` `0x00251370:354,382`; Day default `20.0`, Night default `100.0`).
    pub fog_max: f32,
    /// Fog minimum alpha/density ceiling in PS2 GS 0..255 units (`+0xac`, `DAT_002adacc`,
    /// `SetupDrawEnvData` `0x00251370:355,383`; Day default `235.0`, Night default `255.0`).
    pub fog_min: f32,
    /// Up to 5 room-local point/fake lights (`+0xb0, +0xe0, +0x110, +0x140, +0x170`, stride `0x30` = 48 bytes).
    pub point_lights: [DrawEnvPointLight; 5],
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

    /// Returns the slice of active room point lights declared by `point_light_count` (`+0x07`).
    #[inline]
    #[must_use]
    pub fn active_point_lights(&self) -> &[DrawEnvPointLight] {
        let count = usize::from(self.point_light_count).min(self.point_lights.len());
        &self.point_lights[..count]
    }

    /// Resolves `(fog_far, fog_near, fog_max, fog_min)` matching `SLUS_202.28` `SetupDrawEnvData`
    /// (`0x00251370`, `GFW/sh2_DrawEnvData.c:344-385`).
    ///
    /// When `self.fog_far == 0.0`, returns the PS2 Day (`5500.0, 3000.0, 20.0, 235.0`) or Night
    /// (`8000.0, 3000.0, 100.0, 255.0`) defaults. Otherwise returns the verbatim per-room table values.
    #[inline]
    #[must_use]
    pub const fn effective_fog_bounds(&self, is_night: bool) -> (f32, f32, f32, f32) {
        if self.fog_far == 0.0 {
            if is_night {
                (8000.0, 3000.0, 100.0, 255.0)
            } else {
                (5500.0, 3000.0, 20.0, 235.0)
            }
        } else {
            (self.fog_far, self.fog_near, self.fog_max, self.fog_min)
        }
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

/// Stage-wide Daytime wildcard `room_no` (`0xFFFE` / `65534`, packed `0xFFFFFFFE`) used by
/// `CB_DrawEnvData` (`0x00361ef0`) and `CC_DrawEnvData` (`0x003622d0`) in `SLUS_202.28`
/// `Set_DrawEnvData` (`0x00251060`, `GFW/sh2_DrawEnvData.c:220-231`) and `sh2pc.exe`
/// `FUN_0047c2d0` (`0x0047c2d0`, `sh2pc_00470000.c:9270-9298`).
pub const DRAW_ENV_ROOM_DAYTIME_WILDCARD: u16 = 0xFFFE;

/// Stage-wide default / Nighttime wildcard `room_no` (`0xFFFF` / `65535`, packed `0xFFFFFFFF`)
/// in `SLUS_202.28` `Set_DrawEnvData` (`0x00251060`) and `sh2pc.exe` `FUN_0047c2d0` (`0x0047c2d0`).
pub const DRAW_ENV_ROOM_WILDCARD: u16 = 0xFFFF;

/// Looks up the matching [`DrawEnvData`] entry for a 2-letter PS2 stage code (e.g. `"OB"`, `"ER"`,
/// `"CA"`, `"CB"`, `"AP"`, `"HP"`, `"PS"`, `"RR"`), room index, and daytime/nighttime state
/// (`SLUS_202.28` `Set_DrawEnvData` @ `0x00251060`, `sh2pc.exe` `FUN_0047c2d0` @ `0x0047c2d0`).
///
/// - For Town East (`"CB"`, `uVar3 == 2`) and Town West (`"CC"`, `uVar3 == 3`):
///   - Daytime (`!is_night`, `(_DAT_0112644c >> 0x1b & 1) == 0` / `(DAT_01f7a9fc & 0x8000000) == 0`):
///     matches the first `room_no == 0xFFFE` (`0xFFFFFFFE`) entry (`ambient = 0.65`, `shadow_darkness = 0`).
///   - Nighttime (`is_night`): matches `room_no == 0xFFFF` (`0xFFFFFFFF`) (`ambient = 0.15`, `shadow_darkness = 1`).
/// - For all other stages (including `"CA"` Forest Trail): matches exact `room_no == room_index`
///   (excluding `0xFFFE`/`0xFFFF`), then falls back to `room_no == 0xFFFF` or entry `0`.
#[must_use]
pub fn lookup_draw_env_for_time(
    stage_code: &str,
    room_index: u16,
    is_night: bool,
) -> Option<&'static DrawEnvData> {
    let table = ALL_STAGE_DRAW_ENV_TABLES
        .iter()
        .find(|t| t.stage_code.eq_ignore_ascii_case(stage_code))?;

    // `SLUS_202.28` `Set_DrawEnvData` (`0x00251060:220-231`) & `sh2pc.exe` `FUN_0047c2d0`
    // (`0x0047c2d0:9270-9298`): `CB` (2) and `CC` (3) select `0xFFFE` for Day and `0xFFFF` for Night.
    if stage_code.eq_ignore_ascii_case("CB") || stage_code.eq_ignore_ascii_case("CC") {
        if !is_night {
            if let Some(day) = table
                .entries
                .iter()
                .find(|e| e.room_no == DRAW_ENV_ROOM_DAYTIME_WILDCARD)
            {
                return Some(day);
            }
        }
        return table
            .entries
            .iter()
            .find(|e| e.room_no == DRAW_ENV_ROOM_WILDCARD)
            .or_else(|| table.entries.first());
    }

    // 1. Exact room_no match (excluding 0xFFFE / 0xFFFF wildcards)
    if let Some(exact) = table.entries.iter().find(|e| {
        e.room_no != DRAW_ENV_ROOM_DAYTIME_WILDCARD
            && e.room_no != DRAW_ENV_ROOM_WILDCARD
            && e.room_no == room_index
    }) {
        return Some(exact);
    }

    // 2. Stage-wide wildcard (0xFFFF) or default entry 0
    table
        .entries
        .iter()
        .find(|e| e.room_no == DRAW_ENV_ROOM_WILDCARD)
        .or_else(|| table.entries.first())
}

/// Looks up the matching [`DrawEnvData`] entry for a 2-letter PS2 stage code and room index
/// in daytime mode (`is_night = false`). See [`lookup_draw_env_for_time`] for nighttime town stages.
#[must_use]
pub fn lookup_draw_env(stage_code: &str, room_index: u16) -> Option<&'static DrawEnvData> {
    lookup_draw_env_for_time(stage_code, room_index, false)
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
        // `SetupDrawEnvData` (`0x00251370:344-385`): `OB_DrawEnvData` fog_max (+0xa8) = 40.0, fog_min (+0xac) = 230.0
        assert!((ob.fog_max - 40.0).abs() < 1e-4);
        assert!((ob.fog_min - 230.0).abs() < 1e-4);
        let (_eff_far, _eff_near, eff_max, eff_min) = ob.effective_fog_bounds(false);
        assert!((eff_max - 40.0).abs() < 1e-4);
        assert!((eff_min - 230.0).abs() < 1e-4);

        // ER_DrawEnvData[0] is room_no=9, stage_id=6, point_light_count=1
        let er = lookup_draw_env("ER", 9).expect("ER_DrawEnvData room 9");
        assert_eq!(er.room_no, 9);
        assert_eq!(er.stage_id, 6);
        assert!((er.ambient[0] - 0.65).abs() < 1e-4);
        assert_eq!(er.active_point_lights().len(), 1);
        assert!(er.active_point_lights()[0].is_active());

        // AP_DrawEnvData[0] is room_no=18, stage_id=9; room 12 must NOT match room 18!
        let ap18 = lookup_draw_env("AP", 18).expect("AP_DrawEnvData room 18");
        assert_eq!(ap18.room_no, 18);
        assert!((ap18.ambient[0] - 0.15).abs() < 1e-4);
        assert_eq!(ap18.shadow_darkness, 1);

        let ap12 = lookup_draw_env("AP", 12).expect("AP_DrawEnvData fallback");
        assert_ne!(ap12.room_no, 18);

        // `CA_DrawEnvData` (`0x00361b10` / `0x007f41d0`): ca14 exact vs wildcard ca11/ca20
        let ca14 = lookup_draw_env("CA", 14).expect("CA_DrawEnvData room 14");
        assert_eq!(ca14.room_no, 14);
        assert!((ca14.ambient[0] - 0.9).abs() < 1e-4);
        assert!((ca14.fog_near - 10_000.0).abs() < 1e-2);
        assert!((ca14.fog_far - 17_635.0).abs() < 1e-2);

        let ca11 = lookup_draw_env("CA", 11).expect("CA_DrawEnvData wildcard");
        assert_eq!(ca11.room_no, DRAW_ENV_ROOM_WILDCARD);
        assert!((ca11.ambient[0] - 0.65).abs() < 1e-4);
        assert_eq!(ca11.shadow_darkness, 0);
        assert_eq!(
            ca11.effective_fog_bounds(false),
            (5500.0, 3000.0, 20.0, 235.0)
        );

        // `CB_DrawEnvData` (`0x00361ef0` / `0x007f45b0`): Daytime `0xFFFE` vs Nighttime `0xFFFF`
        let cb54_day = lookup_draw_env("CB", 54).expect("CB_DrawEnvData daytime");
        assert_eq!(cb54_day.room_no, DRAW_ENV_ROOM_DAYTIME_WILDCARD);
        assert!((cb54_day.ambient[0] - 0.65).abs() < 1e-4);
        assert_eq!(cb54_day.shadow_darkness, 0);

        let cb54_night = lookup_draw_env_for_time("CB", 54, true).expect("CB_DrawEnvData night");
        assert_eq!(cb54_night.room_no, DRAW_ENV_ROOM_WILDCARD);
        assert!((cb54_night.ambient[0] - 0.15).abs() < 1e-4);
        assert_eq!(cb54_night.shadow_darkness, 1);
    }
}
