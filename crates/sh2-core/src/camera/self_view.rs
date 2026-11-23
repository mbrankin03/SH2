//! Authentic PS2 Mode 3 (`SELF_VIEW`) and `FarWatch` search-view camera algorithms (`SH2-134`).
//!
//! Ground truth:
//! - PS2 `SLUS_202.28` (`view/vc_main.c`, `view/vc_newest.c`, `view/vc_play.c`):
//!   - `vcMakeIdealCamPosByHeadPos` (`0x0019a3c0`)
//!   - `vcRetFarWatchRate` (`0x00196700`)
//!   - `vcRetSelfViewEffectRate` (`0x001968f0`)
//!   - `vcMakeFarWatchTgtPos` (`0x00199760`)
//!   - `vcRetCirRadiusReduction` (`0x00252b90`)
//!   - `vcMixSelfViewEffectToWatchTgtPos` (`0x00199080`)
//!   - `vcAdjustWatchYLimitHighWhenFarView` (`0x00199dd0`)
//! - Retail PC `sh2pc.exe`:
//!   - `0x0051b030` (`vcMakeIdealCamPosByHeadPos`)
//!   - `0x00519e90` (`vcRetFarWatchRate`)
//!   - `0x00519ff0` (`vcRetSelfViewEffectRate`)
//!   - `0x0051aa20` (`vcMakeFarWatchTgtPos`, boat radius `10200.0` vs SLUS `7200.0`, added stage `0x10`)
//!   - `0x006c8370` (`far_tgt_watch_cir_r` retail `[1600, 2800, 4200, 5000]` vs SLUS `0x00399f40` `[1600, 2800, 5000, 6000]`)
//!   - `0x0051a5f0` (`vcMixSelfViewEffectToWatchTgtPos`)
//!   - `0x0051ae60` (`vcAdjustWatchYLimitHighWhenFarView`)

use crate::camera::locus_door::regulate_angle;
use glam::Vec3;
use sh2_assets::formats::cam::{AreaSizeType, CamMovementType, VC_RD_INVALID_SV_F};
use std::f32::consts::{FRAC_PI_6, PI};

/// `VC_WORK.flags` mask disabling far-watch evaluation (`0x0102`, `SLUS_202.28` `0x00196700`).
pub const VC_FLAG_FAR_WATCH_INHIBIT_MASK: u32 = 0x0102;
/// `VC_WORK.flags` warp-watch bit switching `vcMakeIdealCamPosByHeadPos` to character center (`0x0008`, `0x0019a3c0`).
pub const VC_FLAG_WARP_WATCH: u32 = 0x0008;

/// Head-relative azimuth offset in radians (`170 deg`) for `vcMakeIdealCamPosByHeadPos` (`SLUS_202.28` `0x0019a3c0`; `sh2pc.exe` `0x0051b030` uses `2.9670599`).
pub const SELF_VIEW_HEAD_AZIMUTH_OFS_RAD: f32 = 2.967_059_6;
/// Head-relative horizontal pullback radius in PS2 mm (`SLUS_202.28` `0x0019a3c0`).
pub const SELF_VIEW_HEAD_HORIZ_RADIUS_MM: f32 = 90.0;
/// Head-relative vertical offset in PS2 mm, Y-down (`SLUS_202.28` `0x0019a3c0`).
pub const SELF_VIEW_HEAD_Y_OFS_MM: f32 = 50.0;

/// Maximum `ThroughDoor` far-watch blend rate when search button is not held (`SLUS_202.28` `0x00196700`).
pub const THROUGH_DOOR_FAR_WATCH_MAX_RATE: f32 = 0.9;
/// Distance span in PS2 mm over which `ThroughDoor` far-watch rate decays to zero (`SLUS_202.28` `0x00196700`).
pub const THROUGH_DOOR_FAR_WATCH_DIST_SPAN_MM: f32 = 800.0;
/// Distance threshold in PS2 mm above which `ThroughDoor` far-watch rate applies turnback attenuation (`SLUS_202.28` `0x00196700`).
pub const THROUGH_DOOR_FAR_WATCH_TURN_DIST_MM: f32 = 250.0;
/// Maximum character-vs-doorway angle (`70 deg` in radians) for `ThroughDoor` far-watch attenuation (`SLUS_202.28` `0x00196700`).
pub const THROUGH_DOOR_FAR_WATCH_MAX_ANG_RAD: f32 = 1.221_730_5;

/// Peak self-view effect rate for `ThroughDoor` (Mode 5) (`SLUS_202.28` `0x001968f0`).
pub const SELF_VIEW_EFFECT_THROUGH_DOOR_MAX: f32 = 0.35;
/// Camera-to-head distance in PS2 mm below which self-view effect is at full strength (`SLUS_202.28` `0x001968f0`).
pub const SELF_VIEW_EFFECT_HEAD_NEAR_MM: f32 = 250.0;
/// Camera-to-head distance in PS2 mm above which self-view effect reaches zero (`SLUS_202.28` `0x001968f0`).
pub const SELF_VIEW_EFFECT_HEAD_FAR_MM: f32 = 600.0;
/// Nearest-enemy distance in PS2 mm below which self-view effect is suppressed (`SLUS_202.28` `0x001968f0`).
pub const SELF_VIEW_EFFECT_ENEMY_NEAR_MM: f32 = 1000.0;
/// Nearest-enemy distance in PS2 mm above which self-view effect is unattenuated (`SLUS_202.28` `0x001968f0`).
pub const SELF_VIEW_EFFECT_ENEMY_FAR_MM: f32 = 2000.0;

/// Retail PC `far_tgt_watch_cir_r` table in PS2 mm indexed by `AreaSizeType` (`sh2pc.exe` `0x006c8370`).
///
/// Prototype-vs-retail drift: `SLUS_202.28` (`0x00399f40`) used `[1600.0, 2800.0, 5000.0, 6000.0]`;
/// retail reduced `Wide` (`5000 -> 4200`) and `Outdoor` (`6000 -> 5000`).
pub const FAR_TGT_WATCH_CIR_R_RETAIL: [f32; 4] = [1600.0, 2800.0, 4200.0, 5000.0];
/// PS2 prototype `far_tgt_watch_cir_r` table in PS2 mm (`SLUS_202.28` `0x00399f40`).
pub const FAR_TGT_WATCH_CIR_R_PROTO: [f32; 4] = [1600.0, 2800.0, 5000.0, 6000.0];
/// Retail PC far-watch radius in PS2 mm while rowing the Toluca Lake boat (`sh2pc.exe` `0x0051aa20`).
///
/// Prototype-vs-retail drift: `SLUS_202.28` (`0x00199760`) used `7200.0`.
pub const FAR_WATCH_BOAT_RADIUS_RETAIL_MM: f32 = 10_200.0;
/// PS2 prototype far-watch radius in PS2 mm while rowing the boat (`SLUS_202.28` `0x00199760`).
pub const FAR_WATCH_BOAT_RADIUS_PROTO_MM: f32 = 7200.0;
/// Far-watch radius scale factor in Stage 1 (`observation/forest`) when `player_x >= -20000.0` (`SLUS_202.28` `0x00199760`).
pub const FAR_WATCH_STAGE1_SCALE: f32 = 1.75;
/// Stage 1 X threshold in PS2 mm above which `FAR_WATCH_STAGE1_SCALE` applies (`SLUS_202.28` `0x00199760`).
pub const FAR_WATCH_STAGE1_X_THRESHOLD_MM: f32 = -20_000.0;

/// Ceiling height reference in PS2 mm (Y-down) for `vcAdjustWatchYLimitHighWhenFarView` (`SLUS_202.28` `0x00199dd0`).
pub const FAR_VIEW_CEILING_Y_MM: f32 = -2500.0;
/// Horizontal projection reference in PS2 mm (`0x45cb2000`) for `vcAdjustWatchYLimitHighWhenFarView` (`SLUS_202.28` `0x00199dd0`).
pub const FAR_VIEW_CEILING_DIST_MM: f32 = 6500.0;
/// Stage 9 (`ap`) upper pitch clamp (`-22.5 deg` = `-PI / 8`, `0xbec90fdb`) (`SLUS_202.28` `0x00199dd0`).
pub const FAR_VIEW_STAGE9_MAX_PITCH_RAD: f32 = f32::from_bits(0xbec9_0fdb);

/// Base look-pitch clamp (`70 deg` in radians) in `vcMixSelfViewEffectToWatchTgtPos` (`SLUS_202.28` `0x00199080`).
pub const SELF_VIEW_BASE_PITCH_CLAMP_RAD: f32 = 1.221_730_5;
/// Final mixed pitch clamp (`80 deg` in radians) in `vcMixSelfViewEffectToWatchTgtPos` (`SLUS_202.28` `0x00199080`).
pub const SELF_VIEW_FINAL_PITCH_CLAMP_RAD: f32 = 1.396_263_4;

/// Computes Mode 3 (`SELF_VIEW`) ideal camera eye position in PS2 mm, Y-down.
///
/// Direct 1:1 port of `vcMakeIdealCamPosByHeadPos` (`SLUS_202.28` `0x0019a3c0`, `sh2pc.exe` `0x0051b030`).
pub fn make_ideal_cam_pos_by_head_pos(
    vc_work_flags: u32,
    chara_pos: Vec3,
    chara_center_y: f32,
    head_pos: Vec3,
    chara_mv_ang_y: f32,
) -> Vec3 {
    if (vc_work_flags & VC_FLAG_WARP_WATCH) == 0 {
        let ang = regulate_angle(chara_mv_ang_y + SELF_VIEW_HEAD_AZIMUTH_OFS_RAD);
        let (sin_a, cos_a) = ang.sin_cos();
        Vec3::new(
            head_pos.x + sin_a * SELF_VIEW_HEAD_HORIZ_RADIUS_MM,
            head_pos.y + SELF_VIEW_HEAD_Y_OFS_MM,
            head_pos.z + cos_a * SELF_VIEW_HEAD_HORIZ_RADIUS_MM,
        )
    } else {
        Vec3::new(chara_pos.x, chara_center_y, chara_pos.z)
    }
}

/// Output of `vcRetFarWatchRate` (`SLUS_202.28` `0x00196700`, `sh2pc.exe` `0x00519e90`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FarWatchRateResult {
    /// Blend weight in `[0.0, 1.0]` between normal watch target and far-watch target.
    pub rate: f32,
    /// `true` when Mode 2 (`FixedAngle`) indoor search requests `vcWarpForFixAngCam` (`0x00252980`).
    pub trigger_fix_ang_warp: bool,
}

/// Computes the far-watch blend rate in `[0.0, 1.0]`.
///
/// Direct 1:1 port of `vcRetFarWatchRate` (`SLUS_202.28` `0x00196700`, `sh2pc.exe` `0x00519e90`).
#[allow(clippy::too_many_arguments)]
pub fn ret_far_watch_rate(
    search_button_pressed: bool,
    mv_type: CamMovementType,
    vc_work_flags: u32,
    cur_road_flags: u16,
    is_outdoor: bool,
    through_door_dist_mm: f32,
    through_door_sta_pos: Vec3,
    player_pos: Vec3,
    player_yaw: f32,
) -> FarWatchRateResult {
    if (vc_work_flags & VC_FLAG_FAR_WATCH_INHIBIT_MASK) != 0
        || (cur_road_flags & VC_RD_INVALID_SV_F) != 0
    {
        return FarWatchRateResult {
            rate: 0.0,
            trigger_fix_ang_warp: false,
        };
    }

    match mv_type {
        CamMovementType::Chase | CamMovementType::Settle | CamMovementType::LocusCircle => {
            FarWatchRateResult {
                rate: if search_button_pressed { 1.0 } else { 0.0 },
                trigger_fix_ang_warp: false,
            }
        }
        CamMovementType::FixedAngle => FarWatchRateResult {
            rate: 0.0,
            trigger_fix_ang_warp: search_button_pressed && !is_outdoor,
        },
        CamMovementType::SelfView => FarWatchRateResult {
            rate: 0.0,
            trigger_fix_ang_warp: false,
        },
        CamMovementType::ThroughDoor => {
            if search_button_pressed {
                return FarWatchRateResult {
                    rate: 1.0,
                    trigger_fix_ang_warp: false,
                };
            }
            let mut rate = (THROUGH_DOOR_FAR_WATCH_MAX_RATE
                - (through_door_dist_mm * THROUGH_DOOR_FAR_WATCH_MAX_RATE)
                    / THROUGH_DOOR_FAR_WATCH_DIST_SPAN_MM)
                .max(0.0);
            if through_door_dist_mm > THROUGH_DOOR_FAR_WATCH_TURN_DIST_MM {
                let ang_to_chara = (player_pos.x - through_door_sta_pos.x)
                    .atan2(player_pos.z - through_door_sta_pos.z);
                let ang_diff = regulate_angle(player_yaw - ang_to_chara).abs();
                rate = ((rate * (THROUGH_DOOR_FAR_WATCH_MAX_ANG_RAD - ang_diff))
                    / THROUGH_DOOR_FAR_WATCH_MAX_ANG_RAD)
                    .max(0.0);
            }
            FarWatchRateResult {
                rate,
                trigger_fix_ang_warp: false,
            }
        }
    }
}

/// Computes the head-coupled self-view effect blend weight in `[0.0, 1.0]`.
///
/// Direct 1:1 port of `vcRetSelfViewEffectRate` (`SLUS_202.28` `0x001968f0`, `sh2pc.exe` `0x00519ff0`).
pub fn ret_self_view_effect_rate(
    mv_type: CamMovementType,
    cam_pos: Vec3,
    head_pos: Vec3,
    nearest_enemy_dist_mm: f32,
) -> f32 {
    let max_rate = match mv_type {
        CamMovementType::SelfView => 1.0,
        CamMovementType::ThroughDoor => SELF_VIEW_EFFECT_THROUGH_DOOR_MAX,
        _ => return 0.0,
    };

    let head_dist = (cam_pos - head_pos).length();
    let head_factor = if head_dist < SELF_VIEW_EFFECT_HEAD_NEAR_MM {
        max_rate
    } else if head_dist > SELF_VIEW_EFFECT_HEAD_FAR_MM {
        0.0
    } else {
        (max_rate * (SELF_VIEW_EFFECT_HEAD_FAR_MM - head_dist))
            / (SELF_VIEW_EFFECT_HEAD_FAR_MM - SELF_VIEW_EFFECT_HEAD_NEAR_MM)
    };

    let enemy_factor = if nearest_enemy_dist_mm > SELF_VIEW_EFFECT_ENEMY_FAR_MM {
        1.0
    } else if nearest_enemy_dist_mm < SELF_VIEW_EFFECT_ENEMY_NEAR_MM {
        0.0
    } else {
        (nearest_enemy_dist_mm - SELF_VIEW_EFFECT_ENEMY_NEAR_MM)
            / (SELF_VIEW_EFFECT_ENEMY_FAR_MM - SELF_VIEW_EFFECT_ENEMY_NEAR_MM)
    };

    (head_factor * enemy_factor).clamp(0.0, max_rate)
}

/// Computes the reduced far-watch circle radius for indoor tight stages (`9, 12, 13, 16`).
///
/// Direct 1:1 port of `vcRetCirRadiusReduction` (`SLUS_202.28` `0x00252b90`, `sh2pc.exe` `0x00522260`).
pub fn ret_cir_radius_reduction(
    area_size_type: AreaSizeType,
    cam_pos: Vec3,
    chara_pos: Vec3,
    chara_yaw: f32,
) -> f32 {
    let half_dist = (cam_pos.x - chara_pos.x).hypot(cam_pos.z - chara_pos.z) * 0.5;
    let cam2chr_ang = (chara_pos.x - cam_pos.x).atan2(chara_pos.z - cam_pos.z);
    let base_r = FAR_TGT_WATCH_CIR_R_RETAIL[area_size_type as usize];
    let vx = base_r * chara_yaw.sin() + half_dist * cam2chr_ang.sin();
    let vz = base_r * chara_yaw.cos() + half_dist * cam2chr_ang.cos();
    let ang_diff = regulate_angle(chara_yaw - cam2chr_ang);
    vx.hypot(vz) * (1.0 - (ang_diff / PI).abs())
}

/// Input parameters for `vcMakeFarWatchTgtPos` (`SLUS_202.28` `0x00199760`, `sh2pc.exe` `0x0051aa20`).
#[derive(Debug, Clone, Copy)]
pub struct FarWatchInput {
    /// Active stage ID (`_stage->id`, e.g. `1` = observation/forest, `9` = apartments, `12`/`13`/`16`).
    pub stage_id: i32,
    /// Room area size category (`_VC_ROAD_DATA.area_size_type`).
    pub area_size_type: AreaSizeType,
    /// `true` when James is rowing the Toluca Lake boat (`_sh2jms->flags & 0x20000`).
    pub on_boat: bool,
    /// Current camera eye position in PS2 mm, Y-down (`vcWork.cam_pos`).
    pub cam_pos: Vec3,
    /// Character world position in PS2 mm, Y-down (`vcWork.chara_pos`).
    pub chara_pos: Vec3,
    /// Character facing yaw in radians (`vcWork.chara_mv_ang_y`).
    pub chara_yaw: f32,
    /// Character center Y in PS2 mm, Y-down (`_DAT_012027f8 = *(float*)(_sh2jms + 0xa4)` set in `vcPreSetCharaDataForCamera` `0x0026f780`, `view/vc_play.c:36`).
    pub chara_center_y: f32,
    /// Player neck pitch in radians (`PlayerGetNeckAngleX`, `_DAT_012027f0` / `DAT_01f80d20`).
    pub neck_ang_x: f32,
    /// Player neck yaw in radians (`PlayerGetNeckAngleY`, `_DAT_012027f4` / `_DAT_01f80d24`).
    pub neck_ang_y: f32,
    /// Optional nearest enemy `(xz_dist_mm, target_center_y_mm)` (takes priority over nearest item).
    pub nearest_enemy: Option<(f32, f32)>,
    /// Optional nearest item `(xz_dist_mm, target_center_y_mm)` used when no enemy is in range.
    pub nearest_item: Option<(f32, f32)>,
}

/// Computes the far-watch look target position in PS2 mm, Y-down.
///
/// Direct 1:1 port of `vcMakeFarWatchTgtPos` (`SLUS_202.28` `0x00199760`, `sh2pc.exe` `0x0051aa20`),
/// using retail `far_tgt_watch_cir_r` (`0x006c8370`), retail boat radius (`10200.0` mm), and retail
/// stage `0x10` radius reduction.
pub fn make_far_watch_tgt_pos(input: &FarWatchInput) -> Vec3 {
    let mut radius = if input.stage_id == 1 && input.chara_pos.x >= FAR_WATCH_STAGE1_X_THRESHOLD_MM
    {
        FAR_TGT_WATCH_CIR_R_RETAIL[input.area_size_type as usize] * FAR_WATCH_STAGE1_SCALE
    } else if matches!(input.stage_id, 9 | 12 | 13 | 16) {
        ret_cir_radius_reduction(
            input.area_size_type,
            input.cam_pos,
            input.chara_pos,
            input.chara_yaw,
        )
    } else if input.on_boat {
        FAR_WATCH_BOAT_RADIUS_RETAIL_MM
    } else {
        FAR_TGT_WATCH_CIR_R_RETAIL[input.area_size_type as usize]
    };

    let base_y = if let Some((tgt_dist, tgt_center_y)) = input.nearest_enemy.or(input.nearest_item)
    {
        let pullback = if tgt_dist < 850.0 {
            (tgt_dist * -350.0) / 850.0
        } else {
            -350.0
        };
        if tgt_dist + pullback < radius {
            radius = tgt_dist + pullback;
        }
        if radius < 200.0 {
            radius = 200.0;
        }
        let max_y = input.chara_pos.y - 250.0 + tgt_dist * 0.5;
        tgt_center_y.min(max_y)
    } else {
        input.chara_pos.y - 450.0
    };

    let neck_yaw = regulate_angle(input.chara_yaw + input.neck_ang_y);
    let init_x = input.chara_pos.x + radius * neck_yaw.sin();
    let init_z = input.chara_pos.z + radius * neck_yaw.cos();

    // `shAtanV(cam_pos - init_pos)` -> `atan2(dx, dz)`, then project opposite (`+ PI`) from chara_pos.
    let cam_to_init_ang = (input.cam_pos.x - init_x).atan2(input.cam_pos.z - init_z);
    let opposite_ang = cam_to_init_ang + PI;
    let out_x = input.chara_pos.x + radius * opposite_ang.sin();
    let out_z = input.chara_pos.z + radius * opposite_ang.cos();

    let dy_head = base_y - input.chara_center_y;
    let horiz_dist = (input.cam_pos.x - out_x).hypot(input.cam_pos.z - out_z);
    let ang0 = (input.cam_pos.y - base_y).atan2(horiz_dist);
    let pitch_dy = -(radius.hypot(dy_head)) * input.neck_ang_x.sin();
    let ang1 = (input.cam_pos.y - (base_y + pitch_dy)).atan2(horiz_dist);

    let dist_3d = (input.cam_pos - Vec3::new(out_x, base_y, out_z)).length();
    let out_y = base_y - dist_3d * (ang1 - ang0).sin();

    Vec3::new(out_x, out_y, out_z)
}

/// Blends head-bone YXZ orientation into the camera watch target and roll angle (`ang_z`).
///
/// Direct 1:1 port of `vcMixSelfViewEffectToWatchTgtPos` (`SLUS_202.28` `0x00199080`, `sh2pc.exe` `0x0051a5f0`).
///
/// - `head_yxz`: `[pitch_x, yaw_y, roll_z]` in radians extracted from James's head bone matrix (`vwMatrixToAngleYXZ`).
/// - `player_anim_kind`: James's current motion kind (`DAT_003c7f51` in `SLUS_202.28`, `in_EAX` in `sh2pc.exe`).
/// - Returns `(blended_watch_tgt_pos, blended_ang_z)`.
#[allow(clippy::too_many_arguments)]
pub fn mix_self_view_effect_to_watch_tgt_pos(
    self_view_rate: f32,
    watch_tgt_pos: Vec3,
    watch_tgt_ang_z: f32,
    cam_pos: Vec3,
    player_yaw: f32,
    head_yxz: [f32; 3],
    player_anim_kind: u8,
    has_nearest_enemy: bool,
) -> (Vec3, f32) {
    let delta = watch_tgt_pos - cam_pos;
    let dist_3d = delta.length();
    let dist_xz = delta.x.hypot(delta.z);
    let base_pitch = (-delta.y).atan2(dist_xz).clamp(
        -SELF_VIEW_BASE_PITCH_CLAMP_RAD,
        SELF_VIEW_BASE_PITCH_CLAMP_RAD,
    );

    let mut head_x = head_yxz[0];
    let mut head_y = head_yxz[1];
    let mut head_z = head_yxz[2];

    let rel_head_y = regulate_angle(head_y - player_yaw);

    // Roll attenuation (`param_6` / `player_anim_kind`)
    if player_anim_kind == 1 {
        if has_nearest_enemy {
            head_z = 0.0;
        }
    } else if !matches!(player_anim_kind, 0x0c..=0x0e) {
        head_z *= 0.5;
    }

    // Yaw blending (`fStack_2c`)
    match player_anim_kind {
        6 | 7 | 9 | 0x0c | 0x0d | 0x0e => {
            let clamped = (rel_head_y / 8.0).clamp(-0.174_532_92, 0.174_532_92);
            head_y = player_yaw + clamped;
        }
        3 => {
            let abs_rel = rel_head_y.abs();
            let adj = if abs_rel > 0.069_813_17 {
                let mag = (abs_rel - 0.069_813_17) / 8.0 + 0.069_813_17;
                if rel_head_y < 0.0 {
                    -mag
                } else {
                    mag
                }
            } else {
                rel_head_y
            };
            head_y = player_yaw + adj;
        }
        0x18 => {}
        1 if !has_nearest_enemy => {
            head_y += FRAC_PI_6;
        }
        _ => {
            head_y = player_yaw;
        }
    }

    // Pitch offset (`fStack_30`)
    match player_anim_kind {
        6 | 7 => {
            let yaw_sector = ((player_yaw as i32) >> 7) & 0x0f;
            if yaw_sector == 0 || yaw_sector == 5 {
                head_x -= 0.017_453_292;
            }
            head_x -= 0.104_719_76;
        }
        9 | 0x0c | 0x0d | 0x0e => {}
        1 => {
            if !has_nearest_enemy {
                head_x -= 0.139_626_34;
            } else {
                head_x = -0.122_173_05;
            }
        }
        3 => {
            head_x -= 0.139_626_34;
        }
        _ => {}
    }

    // Pitch blend with base watch pitch
    head_x = if matches!(player_anim_kind, 1 | 3 | 6 | 7 | 9 | 0x0c | 0x0d | 0x0e) {
        head_x + base_pitch * 0.5
    } else {
        base_pitch * 0.7
    }
    .clamp(
        -SELF_VIEW_FINAL_PITCH_CLAMP_RAD,
        SELF_VIEW_FINAL_PITCH_CLAMP_RAD,
    );

    let blended_ang_z = watch_tgt_ang_z + regulate_angle(head_z - watch_tgt_ang_z) * self_view_rate;

    let (sin_p, cos_p) = head_x.sin_cos();
    let (sin_y, cos_y) = head_y.sin_cos();
    let self_view_tgt = Vec3::new(
        cam_pos.x + cos_p * dist_3d * sin_y,
        cam_pos.y - dist_3d * sin_p,
        cam_pos.z + cos_p * dist_3d * cos_y,
    );
    let blended_tgt = watch_tgt_pos + (self_view_tgt - watch_tgt_pos) * self_view_rate;

    (blended_tgt, blended_ang_z)
}

/// Clamps the upper elevation of `watch_tgt_pos.y` (PS2 mm, Y-down) so far-view does not look above
/// the room ceiling (`-2500.0` mm at `6500.0` mm distance) or above `-22.5 deg` in Stage 9 (`ap`).
///
/// Direct 1:1 port of `vcAdjustWatchYLimitHighWhenFarView` (`SLUS_202.28` `0x00199dd0`, `sh2pc.exe` `0x0051ae60`).
pub fn adjust_watch_y_limit_high_when_far_view(
    mut watch_tgt_pos: Vec3,
    cam_pos: Vec3,
    scr_z: f32,
    scr_sy: f32,
    stage_id: i32,
) -> Vec3 {
    let ceil_ang = (-(FAR_VIEW_CEILING_Y_MM - cam_pos.y)).atan2(FAR_VIEW_CEILING_DIST_MM);
    let half_fov_y = (scr_sy * 0.5).atan2(scr_z.max(1.0));
    let limit_ang = ceil_ang - half_fov_y;

    let horiz_dist = (watch_tgt_pos.x - cam_pos.x).hypot(watch_tgt_pos.z - cam_pos.z);
    let cur_elev = (-(watch_tgt_pos.y - cam_pos.y)).atan2(horiz_dist);

    if cur_elev > limit_ang {
        watch_tgt_pos.y = cam_pos.y - (horiz_dist * limit_ang.sin()) / limit_ang.cos();
    }
    if stage_id == 9 && cur_elev > FAR_VIEW_STAGE9_MAX_PITCH_RAD {
        watch_tgt_pos.y = cam_pos.y
            - (horiz_dist * FAR_VIEW_STAGE9_MAX_PITCH_RAD.sin())
                / FAR_VIEW_STAGE9_MAX_PITCH_RAD.cos();
    }
    watch_tgt_pos
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_make_ideal_cam_pos_by_head_pos_normal_and_warp() {
        let chara = Vec3::new(1000.0, 0.0, 2000.0);
        let head = Vec3::new(1000.0, -1650.0, 2000.0);
        let pos = make_ideal_cam_pos_by_head_pos(0, chara, -850.0, head, 0.0);
        let expected_ang = regulate_angle(SELF_VIEW_HEAD_AZIMUTH_OFS_RAD);
        assert!((pos.x - (1000.0 + expected_ang.sin() * 90.0)).abs() < 1e-3);
        assert!((pos.y - (-1600.0)).abs() < 1e-3);
        assert!((pos.z - (2000.0 + expected_ang.cos() * 90.0)).abs() < 1e-3);

        let warp_pos = make_ideal_cam_pos_by_head_pos(VC_FLAG_WARP_WATCH, chara, -850.0, head, 0.0);
        assert_eq!(warp_pos, Vec3::new(1000.0, -850.0, 2000.0));
    }

    #[test]
    fn test_ret_far_watch_rate_golden_vectors() {
        // Chase with search pressed -> 1.0; inhibited by VC_RD_INVALID_SV_F -> 0.0
        let r1 = ret_far_watch_rate(
            true,
            CamMovementType::Chase,
            0,
            0,
            false,
            0.0,
            Vec3::ZERO,
            Vec3::ZERO,
            0.0,
        );
        assert_eq!(r1.rate, 1.0);
        assert!(!r1.trigger_fix_ang_warp);

        let r_inhibit = ret_far_watch_rate(
            true,
            CamMovementType::Chase,
            0,
            VC_RD_INVALID_SV_F,
            false,
            0.0,
            Vec3::ZERO,
            Vec3::ZERO,
            0.0,
        );
        assert_eq!(r_inhibit.rate, 0.0);

        // FixedAngle indoor search triggers vcWarpForFixAngCam
        let r_fix = ret_far_watch_rate(
            true,
            CamMovementType::FixedAngle,
            0,
            0,
            false,
            0.0,
            Vec3::ZERO,
            Vec3::ZERO,
            0.0,
        );
        assert_eq!(r_fix.rate, 0.0);
        assert!(r_fix.trigger_fix_ang_warp);

        // ThroughDoor at dist=0 -> 0.9; at dist=400 facing along +Z -> 0.45
        let r_td0 = ret_far_watch_rate(
            false,
            CamMovementType::ThroughDoor,
            0,
            0,
            false,
            0.0,
            Vec3::ZERO,
            Vec3::ZERO,
            0.0,
        );
        assert!((r_td0.rate - 0.9).abs() < 1e-5);

        let r_td400 = ret_far_watch_rate(
            false,
            CamMovementType::ThroughDoor,
            0,
            0,
            false,
            400.0,
            Vec3::ZERO,
            Vec3::new(0.0, 0.0, 400.0),
            0.0,
        );
        assert!((r_td400.rate - 0.45).abs() < 1e-5);
    }

    #[test]
    fn test_ret_self_view_effect_rate_knees() {
        let head = Vec3::new(0.0, -1600.0, 0.0);
        // Mode 3, head_dist=200 (<250), enemy=3000 (>2000) -> 1.0
        assert!(
            (ret_self_view_effect_rate(
                CamMovementType::SelfView,
                Vec3::new(0.0, -1600.0, -200.0),
                head,
                3000.0
            ) - 1.0)
                .abs()
                < 1e-5
        );
        // Mode 3, head_dist=425 (midpoint of 250..600 -> 0.5), enemy=1500 (midpoint of 1000..2000 -> 0.5) -> 0.25
        assert!(
            (ret_self_view_effect_rate(
                CamMovementType::SelfView,
                Vec3::new(0.0, -1600.0, -425.0),
                head,
                1500.0
            ) - 0.25)
                .abs()
                < 1e-5
        );
        // Mode 5 caps at 0.35
        assert!(
            (ret_self_view_effect_rate(
                CamMovementType::ThroughDoor,
                Vec3::new(0.0, -1600.0, -100.0),
                head,
                3000.0
            ) - 0.35)
                .abs()
                < 1e-5
        );
        // Enemy < 1000 mm suppresses to 0.0
        assert_eq!(
            ret_self_view_effect_rate(CamMovementType::SelfView, head, head, 999.0),
            0.0
        );
    }

    #[test]
    fn test_make_far_watch_tgt_pos_boat_and_enemy_pullback() {
        let boat_in = FarWatchInput {
            stage_id: 2,
            area_size_type: AreaSizeType::Outdoor,
            on_boat: true,
            cam_pos: Vec3::new(0.0, -1400.0, -2000.0),
            chara_pos: Vec3::ZERO,
            chara_yaw: 0.0,
            chara_center_y: -468.75,
            neck_ang_x: 0.0,
            neck_ang_y: 0.0,
            nearest_enemy: None,
            nearest_item: None,
        };
        let boat_tgt = make_far_watch_tgt_pos(&boat_in);
        assert!((boat_tgt.z - FAR_WATCH_BOAT_RADIUS_RETAIL_MM).abs() < 1.0);
        assert!((boat_tgt.y - (-450.0)).abs() < 1.0);

        let enemy_in = FarWatchInput {
            on_boat: false,
            nearest_enemy: Some((1000.0, -800.0)),
            ..boat_in
        };
        let enemy_tgt = make_far_watch_tgt_pos(&enemy_in);
        // r = 1000 - 350 = 650 mm
        assert!((enemy_tgt.z - 650.0).abs() < 1.0);
        assert!((enemy_tgt.y - (-800.0)).abs() < 1.0);
    }

    #[test]
    fn test_mix_self_view_effect_and_adjust_watch_y_limit() {
        let cam = Vec3::new(0.0, -1400.0, -1000.0);
        let tgt = Vec3::new(0.0, -1400.0, 1000.0);
        let (mixed, ang_z) = mix_self_view_effect_to_watch_tgt_pos(
            0.5,
            tgt,
            0.0,
            cam,
            0.0,
            [0.0, 0.0, 0.2],
            0,
            false,
        );
        assert!(mixed.is_finite());
        // Default anim kind halves roll (0.1), then blends by 0.5 -> 0.05
        assert!((ang_z - 0.05).abs() < 1e-5);

        // Looking straight up in Stage 9 must be clamped downward (larger Y in Y-down space)
        let high_tgt = Vec3::new(0.0, -5000.0, 1000.0);
        let clamped = adjust_watch_y_limit_high_when_far_view(high_tgt, cam, 448.0, 448.0, 9);
        assert!(clamped.y > high_tgt.y);
    }
}
