//! Background & Stay Stage Object Controllers (`Chacter/bg_obj_stay.c`, `Chacter/m3_bgobj.c`,
//! `Chacter/bg_chara.c`, `Chacter/chara_bound.c`).
//!
//! Verified 1:1 against PS2 prototype `SLUS_202.28` (and E3 `SLPM_123.45` drift):
//! - `StayObjectFunction` (`SLUS_202.28` `0x00102eb0`): initializes static stage `.mdl` props
//!   with `shadow_y = world_y - 150.0` (`0xa4`, `0xa8`). (`SLPM_123.45` `0x00103ad0` added
//!   `+FRAC_PI_2` to `rot_z` and did not subtract `150.0`; `SLUS_202.28` removed both.)
//! - `ObjectFunction` (`SLUS_202.28` `0x00103f50`): animatable stage props (`0x04xx`),
//!   `shadow_y = world_y - 100.0` in step `1`, or `ObjectInit` (`anime_type = 0`) during demo mode.
//! - `shCharacterObjectAnimeSetP` (`SLUS_202.28` `0x00104140`): special-cases `0x040b` with
//!   `rot = [0.0, PI, 0.0]` (`0x40490fdb` at `0x34`).
//! - `RObjectInit` / `RObjectFunction` (`SLUS_202.28` `0x00104250..0x001042f0`): mirror reflection
//!   objects across X (`room 0x01`, `wall_pos = -20000.0`) or Z (`room 0x24`, `wall_pos = -99995.0`).
//! - `BgCharaMan` (`SLUS_202.28` `0x00253920`) & `get_block_num` (`0x002532b0`): 2-slot outdoor
//!   background character loader (`0x40000` = 256 KiB blocks, `BgCharaIsId` `kind >> 8 == 3`).
//! - `CharaGetBoundR` (`SLUS_202.28` `0x002758a0`): default `2500.0 mm` (`0x451c4000`) + `8000.0 mm`
//!   spawn/cull margin (`ConnectCharaWorkAdminOut` `0x001cafe0`).

use std::f32::consts::PI;

/// Shadow Y offset for static stay objects (`StayObjectFunction` `SLUS_202.28` `0x00102eb0`: `-150.0`).
pub const STAY_OBJECT_SHADOW_Y_OFFSET_MM: f32 = -150.0;

/// Shadow Y offset for animated stage objects (`ObjectFunction` `SLUS_202.28` `0x00103f50`: `-100.0`).
pub const BG_OBJECT_SHADOW_Y_OFFSET_MM: f32 = -100.0;

/// Special-cased animated prop model ID in `shCharacterObjectAnimeSetP` (`SLUS_202.28` `0x00104140`).
pub const MODEL_ID_SPECIAL_PI_YAW: u16 = 0x040b;

/// Default character/prop bounding radius returned by `CharaGetBoundR` (`SLUS_202.28` `0x002758a0`,
/// IEEE-754 `0x451c4000` = `2500.0 mm`).
pub const DEFAULT_CHARA_BOUND_R_MM: f32 = 2_500.0;

/// Outdoor spawn/cull radius margin added to `CharaGetBoundR` in `ConnectCharaWorkAdminOut`
/// (`SLUS_202.28` `0x001cafe0`: `fVar14 + 8000.0`).
pub const OUTDOOR_PROP_CULL_MARGIN_MM: f32 = 8_000.0;

/// Outdoor `BgChara` streaming block unit (`0x40000` = 256 KiB, `get_block_num` `SLUS_202.28` `0x002532b0`).
pub const BG_CHARA_BLOCK_SIZE_BYTES: usize = 0x4_0000;

/// Mirror wall X coordinate for `RoomNameJms() == 0x01` (`RObjectInit` `SLUS_202.28` `0x00104250`,
/// `0xc69c4000` = `-20000.0 mm`).
pub const MIRROR_WALL_POS_ROOM_01_X_MM: f32 = -20_000.0;

/// Mirror wall Z coordinate for `RoomNameJms() == 0x24` (`RObjectInit` `SLUS_202.28` `0x00104250`,
/// `0xc7c34d80` = `-99995.0 mm`).
pub const MIRROR_WALL_POS_ROOM_24_Z_MM: f32 = -99_995.0;

/// Lifecycle state for a static `StayObject` (`bg_obj_stay.c` `0x00102eb0`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StayObjectState {
    /// Internal step (`0` = uninitialized, `1` = initialized).
    pub step: u32,
    /// `SCStayModelSwitch(1)` flag.
    pub stay_model_enabled: bool,
    /// `shCharacterWorldScreenObjectSetNew(1.0)` flag.
    pub world_screen_enabled: bool,
    /// Ground/shadow Y coordinate (`0xa4`/`0xa8`), initialized to `world_y - 150.0`.
    pub shadow_y_mm: f32,
}

impl Default for StayObjectState {
    fn default() -> Self {
        Self {
            step: 0,
            stay_model_enabled: false,
            world_screen_enabled: false,
            shadow_y_mm: 0.0,
        }
    }
}

impl StayObjectState {
    /// Executes one tick of `StayObjectFunction` (`SLUS_202.28` `0x00102eb0`).
    pub fn step(&mut self, world_y_mm: f32) {
        if self.step == 0 {
            self.stay_model_enabled = true;
            self.world_screen_enabled = true;
            self.shadow_y_mm = world_y_mm + STAY_OBJECT_SHADOW_Y_OFFSET_MM;
            self.step = 1;
        }
    }
}

/// Lifecycle state for an animatable stage `Object` (`m3_bgobj.c` `0x00103f50`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BgObjectState {
    /// Model ID (`0x04xx` for animatable stage props).
    pub model_id: u16,
    /// Internal step (`0` = uninitialized, `1` = active).
    pub step: u32,
    /// `SCStayModelSwitch(1)` flag.
    pub stay_model_enabled: bool,
    /// `shCharacterWorldScreenObjectSetNew(1.0)` flag.
    pub world_screen_enabled: bool,
    /// `SCAnimeTypeSwitch` (`0` = drama/init, `1` = playable).
    pub anime_type: u8,
    /// Ground/shadow Y coordinate (`0xa4`/`0xa8`).
    pub shadow_y_mm: f32,
    /// Current Euler rotation `[pitch_x, yaw_y, roll_z]` in radians.
    pub rotation_rad: [f32; 3],
}

impl BgObjectState {
    /// Creates a new [`BgObjectState`] for `model_id` with initial `rotation_rad`.
    #[must_use]
    pub const fn new(model_id: u16, rotation_rad: [f32; 3]) -> Self {
        Self {
            model_id,
            step: 0,
            stay_model_enabled: false,
            world_screen_enabled: false,
            anime_type: 0,
            shadow_y_mm: 0.0,
            rotation_rad,
        }
    }

    /// Executes one tick of `ObjectFunction` (`SLUS_202.28` `0x00103f50`).
    pub fn step(&mut self, world_y_mm: f32, in_demo_event_mode: bool) {
        if self.step == 1 {
            self.shadow_y_mm = world_y_mm + BG_OBJECT_SHADOW_Y_OFFSET_MM;
        } else if self.step == 0 {
            if !in_demo_event_mode {
                self.stay_model_enabled = true;
                self.world_screen_enabled = true;
                self.shadow_y_mm = world_y_mm;
            } else {
                self.anime_type = 0;
            }
            self.step = 1;
        }
    }

    /// Applies playable background object animation setup (`shCharacterObjectAnimeSetP`,
    /// `SLUS_202.28` `0x00104140`), returning `true` if `model_id >> 8 == 4`.
    pub fn set_playable_anim(&mut self) -> bool {
        self.anime_type = 1;
        if self.model_id == MODEL_ID_SPECIAL_PI_YAW {
            self.rotation_rad = [0.0, PI, 0.0];
        }
        (self.model_id >> 8) == 4
    }
}

/// Mirror reflection axis used by `RObjectInit` / `RObjectFunction` (`SLUS_202.28` `0x00104250`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MirrorMode {
    /// `0`: Mirror across X plane (`x' = 2 * wall_pos - x`, `yaw' = -yaw`).
    AxisX = 0,
    /// `1`: Mirror across Y plane (`y' = 2 * wall_pos - y`, `roll' = PI`).
    AxisY = 1,
    /// `2`: Mirror across Z plane (`z' = 2 * wall_pos - z`, `yaw' = ±PI - yaw`).
    AxisZ = 2,
}

/// Mirror reflection parameters initialized by `RObjectInit` (`SLUS_202.28` `0x00104250`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MirrorPlane {
    /// Active reflection axis.
    pub mode: MirrorMode,
    /// Reflection plane coordinate in PS2 mm.
    pub wall_pos_mm: f32,
}

impl MirrorPlane {
    /// Resolves `RObjectInit` (`SLUS_202.28` `0x00104250`) for `room_id`.
    #[must_use]
    pub const fn for_room(room_id: u32) -> Option<Self> {
        match room_id {
            0x24 => Some(Self {
                mode: MirrorMode::AxisZ,
                wall_pos_mm: MIRROR_WALL_POS_ROOM_24_Z_MM,
            }),
            0x01 => Some(Self {
                mode: MirrorMode::AxisX,
                wall_pos_mm: MIRROR_WALL_POS_ROOM_01_X_MM,
            }),
            _ => None,
        }
    }

    /// Computes mirrored `(position_mm, rotation_rad)` from the source character (`model_id - 0x20`)
    /// per `RObjectFunction` (`SLUS_202.28` `0x001042f0`).
    #[must_use]
    pub fn mirror_transform(self, pos: [f32; 3], rot: [f32; 3]) -> ([f32; 3], [f32; 3]) {
        match self.mode {
            MirrorMode::AxisZ => {
                let z = self.wall_pos_mm + (self.wall_pos_mm - pos[2]);
                let yaw = if rot[1] >= 0.0 {
                    PI - rot[1]
                } else {
                    -PI - rot[1]
                };
                ([pos[0], pos[1], z], [rot[0], yaw, rot[2]])
            }
            MirrorMode::AxisY => {
                let y = self.wall_pos_mm + (self.wall_pos_mm - pos[1]);
                ([pos[0], y, pos[2]], [rot[0], rot[1], PI])
            }
            MirrorMode::AxisX => {
                let x = self.wall_pos_mm + (self.wall_pos_mm - pos[0]);
                ([x, pos[1], pos[2]], [rot[0], -rot[1], rot[2]])
            }
        }
    }
}

/// Returns `true` if `model_id` is an outdoor `BgChara` ID (`BgCharaIsId` `SLUS_202.28` `0x00253790`:
/// `model_id >> 8 == 3`).
#[inline]
#[must_use]
pub const fn bg_chara_is_id(model_id: u16) -> bool {
    (model_id >> 8) == 3
}

/// Computes the 256 KiB block count for an outdoor `BgChara` file (`get_block_num` `SLUS_202.28`
/// `0x002532b0`), returning `None` when `file_size == 0` (`0xffffffff` in C).
#[inline]
#[must_use]
pub const fn bg_chara_block_count(file_size: usize) -> Option<usize> {
    if file_size == 0 {
        None
    } else {
        Some((file_size + (BG_CHARA_BLOCK_SIZE_BYTES - 1)) >> 18)
    }
}

/// 2-slot outdoor `BgChara` residency tracker (`BgCharaMan` `SLUS_202.28` `0x002538f0..0x00253ae0`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BgCharaSlots {
    /// Loaded model IDs in the 2 outdoor background character slots (`0` = empty).
    pub slot_model_ids: [u16; 2],
}

impl BgCharaSlots {
    /// Clears all slots (`BgCharaManInit` `0x002538f0`).
    pub fn init(&mut self) {
        self.slot_model_ids = [0, 0];
    }

    /// Clears a single slot (`BgCharaDelSlot` `0x00253a80`).
    pub fn delete_slot(&mut self, slot: usize) {
        if let Some(entry) = self.slot_model_ids.get_mut(slot) {
            *entry = 0;
        }
    }

    /// Returns the model ID loaded in `slot` (`BgCharaGetSlotId` `0x00253ae0`).
    #[must_use]
    pub const fn get_slot_id(&self, slot: usize) -> u16 {
        if slot < 2 {
            self.slot_model_ids[slot]
        } else {
            0
        }
    }
}

/// Returns the bounding radius in PS2 mm for `model_id` (`CharaGetBoundR` `SLUS_202.28` `0x002758a0`).
#[must_use]
pub const fn chara_bound_radius_mm(_model_id: u16) -> f32 {
    DEFAULT_CHARA_BOUND_R_MM
}

/// Returns `true` if a prop at `prop_pos_mm` is within the `ConnectCharaWorkAdminOut`
/// (`SLUS_202.28` `0x001cafe0`) spawn/keep distance from `player_pos_mm`:
/// `dx^2 + dz^2 < (CharaGetBoundR(model_id) + 8000.0)^2`.
#[must_use]
pub fn is_within_outdoor_prop_range(
    model_id: u16,
    prop_pos_mm: [f32; 3],
    player_pos_mm: [f32; 3],
) -> bool {
    let radius = chara_bound_radius_mm(model_id) + OUTDOOR_PROP_CULL_MARGIN_MM;
    let dx = prop_pos_mm[0] - player_pos_mm[0];
    let dz = prop_pos_mm[2] - player_pos_mm[2];
    if dx.abs() >= radius || dz.abs() >= radius {
        return false;
    }
    dx * dx + dz * dz < radius * radius
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stay_object_and_bg_object_shadow_offsets_match_slus() {
        let mut stay = StayObjectState::default();
        stay.step(120.0);
        assert_eq!(stay.step, 1);
        assert!(stay.stay_model_enabled);
        assert!((stay.shadow_y_mm - (-30.0)).abs() < 1e-4);

        let mut obj = BgObjectState::new(0x040b, [0.0; 3]);
        obj.step(200.0, false);
        assert_eq!(obj.step, 1);
        assert!((obj.shadow_y_mm - 200.0).abs() < 1e-4);
        obj.step(200.0, false);
        assert!((obj.shadow_y_mm - 100.0).abs() < 1e-4);
        assert!(obj.set_playable_anim());
        assert!((obj.rotation_rad[1] - PI).abs() < 1e-5);
    }

    #[test]
    fn robject_mirror_matches_slus_constants() {
        let m01 = MirrorPlane::for_room(0x01).expect("room 0x01 mirror");
        let (p01, r01) = m01.mirror_transform([-19_500.0, 10.0, 500.0], [0.0, 0.75, 0.0]);
        assert!((p01[0] - (-20_500.0)).abs() < 1e-3);
        assert!((r01[1] - (-0.75)).abs() < 1e-5);

        let m24 = MirrorPlane::for_room(0x24).expect("room 0x24 mirror");
        let (p24, r24) = m24.mirror_transform([0.0, 0.0, -99_000.0], [0.0, 0.5, 0.0]);
        assert!((p24[2] - (-100_990.0)).abs() < 1e-3);
        assert!((r24[1] - (PI - 0.5)).abs() < 1e-5);
    }

    #[test]
    fn bg_chara_blocks_and_outdoor_cull_match_slus() {
        assert!(bg_chara_is_id(0x0305));
        assert!(!bg_chara_is_id(0x0700));
        assert_eq!(bg_chara_block_count(0), None);
        assert_eq!(bg_chara_block_count(1), Some(1));
        assert_eq!(bg_chara_block_count(0x4_0000), Some(1));
        assert_eq!(bg_chara_block_count(0x4_0001), Some(2));
        assert!(is_within_outdoor_prop_range(
            0x0700,
            [10_499.0, 0.0, 0.0],
            [0.0, 0.0, 0.0]
        ));
        assert!(!is_within_outdoor_prop_range(
            0x0700,
            [10_500.0, 0.0, 0.0],
            [0.0, 0.0, 0.0]
        ));
    }
}
