//! Procedural head & neck bone tracking toward nearby pickup items, doors, Maria,
//! or enemies (`SLUS_202.28` `PlayerCheckNeckAngle` `0x00108e10`, `sh2pc.exe` `FUN_005424a0`
//! / `FUN_005426b0` `0x005424a0..0x00542910`, and `Patches/TeddyBearLookFix.cpp`).
//!
//! # Ground Truth & Binary References
//! - **`PlayerCheckNeckAngle` (`SLUS_202.28` `Chacter/m3_play.c` `0x00108e10`):**
//!   - Pitch (`param_1 + 0x230`): clamped to `[-0.4, +0.5]` rad (`0xbecccccd..0x3f000000`).
//!   - Yaw (`param_1 + 0x234`): clamped to `[-0.8, +0.8]` rad (`0xbf4ccccd..0x3f4ccccd`).
//!   - Angular slew speed: `20.0` rad/s (`0x41a00000`) via `close_to_angle_target`.
//!   - Upper-spine / neck distribution (`0x250 = 0x230 * 0.2`, `0x254 = 0x234 * 0.2`):
//!     20% on the neck/chest parent bone and 80% on the cranial head bone.
//! - **`FUN_005424a0` / `FUN_005426b0` (`sh2pc.exe` `0x005424a0..0x00542910`):**
//!   - Scans object list for `kind >> 8` in `{1 (NPC/Maria), 2 (Enemy), 7 (Item/Door prop)}`,
//!     filtering items (`0x700`) to `dy in [-600.0, +1000.0]` mm and sorting by Euclidean distance.
//! - **`TeddyBearLookFix` (`Patches/TeddyBearLookFix.cpp`, `0x00542769`):**
//!   - Ignores `kind == 0x0727` (`i_bear`) once game flag `0xAA` (`170`, `Bent needle` acquired) is set.

use glam::{Quat, Vec3};
use sh2_assets::formats::anm::{mat4_inverse, mat4_mul, quat_to_matrix};

use crate::events::{
    act1_should_james_look_at_prop, Act1VanillaFixesConfig, EventFlags, ACT1_PROP_KIND_TEDDY_BEAR,
};

/// Minimum neck/head pitch in radians (`0xbecccccd` = `-0.4` rad, `PlayerCheckNeckAngle` `0x00108e10`).
pub const NECK_PITCH_MIN_RAD: f32 = -0.4;
/// Maximum neck/head pitch in radians (`0x3f000000` = `+0.5` rad, `PlayerCheckNeckAngle` `0x00108e10`).
pub const NECK_PITCH_MAX_RAD: f32 = 0.5;
/// Minimum neck/head yaw in radians (`0xbf4ccccd` = `-0.8` rad, `PlayerCheckNeckAngle` `0x00108e10`).
pub const NECK_YAW_MIN_RAD: f32 = -0.8;
/// Maximum neck/head yaw in radians (`0x3f4ccccd` = `+0.8` rad, `PlayerCheckNeckAngle` `0x00108e10`).
pub const NECK_YAW_MAX_RAD: f32 = 0.8;
/// Neck/head angular slew speed (`0x41a00000` = `20.0`, `PlayerCheckNeckAngle` `0x00108e10`).
pub const NECK_TURN_SPEED: f32 = 20.0;
/// Fraction of neck rotation applied to the upper-spine/neck bone (`0.2`, `m3_play.c:1578-1579`).
pub const NECK_UPPER_SPINE_SHARE: f32 = 0.2;
/// Remaining fraction of neck rotation applied to the cranial head bone (`0.8 = 1.0 - 0.2`).
pub const NECK_CRANIAL_SHARE: f32 = 1.0 - NECK_UPPER_SPINE_SHARE;

/// Minimum vertical delta (`-600.0` mm) for `0x700` item/prop look targets (`sh2pc.exe` `FUN_005424a0` `0x005424a0`).
pub const LOOK_ITEM_MIN_DY_MM: f32 = -600.0;
/// Maximum vertical delta (`+1000.0` mm) for `0x700` item/prop look targets (`sh2pc.exe` `FUN_005424a0` `0x005424a0`).
pub const LOOK_ITEM_MAX_DY_MM: f32 = 1000.0;
/// Default search-area look radius (`2000.0` mm) when `PlayerSetSearchArea` (`0x00108390`) is active.
pub const DEFAULT_LOOK_SEARCH_RADIUS_MM: f32 = 2000.0;

/// James's neck/upper-spine bone index (`lll_jms.mdl` / `hll_jms.mdl` / `hhh_jms.mdl`, `body_region.rs`).
pub const JAMES_NECK_BONE_INDEX: usize = 8;
/// James's cranial head bone index (`lll_jms.mdl` / `hll_jms.mdl` / `hhh_jms.mdl`, `CharacterHeadRigSpec::JAMES`).
pub const JAMES_HEAD_BONE_INDEX: usize = 15;
/// Maria's neck/upper-spine bone index (`lll_mar.mdl` / `hhh_mar.mdl`, `parents[9] == 6`, parent of cranial bone `9`).
pub const MARIA_NECK_BONE_INDEX: usize = 6;
/// Maria's cranial head bone index (`lll_mar.mdl` / `hhh_mar.mdl`, `CharacterHeadRigSpec::MARIA`).
pub const MARIA_HEAD_BONE_INDEX: usize = 9;

/// Classification of a candidate look target (`kind >> 8` in `sh2pc.exe` `FUN_005426b0` `0x005426b0`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LookTargetKind {
    /// Friendly NPC (e.g., Maria, `kind >> 8 == 1`).
    Npc,
    /// Hostile creature (`kind >> 8 == 2`, requires active/visible flag `0x400`).
    Enemy,
    /// Pickup item or interactive prop (`kind >> 8 == 7`, e.g. `0x0727` `i_bear`).
    ItemOrProp,
    /// Interactive door / passage trigger.
    Door,
}

/// Candidate world-space look target evaluated by [`select_head_look_target`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LookTargetCandidate {
    /// Target classification (`Npc`, `Enemy`, `ItemOrProp`, `Door`).
    pub kind: LookTargetKind,
    /// Raw 16-bit `kind` ID (`0x0727` for `i_bear` teddy bear in Brookhaven 2F locker room).
    pub prop_kind: i16,
    /// World-space target position in SH2 mm (`+Y` down).
    pub position: Vec3,
    /// Whether this candidate is currently the locked combat/aim target (`_DAT_003c83c4`).
    pub is_combat_target: bool,
    /// Whether this candidate is active/visible (`(flags & 0x400) != 0` for enemies).
    pub is_active: bool,
}

/// Stateful procedural head/neck look-at tracker (`PlayerCheckNeckAngle` `0x00108e10`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeadLookTracker {
    /// Current smoothed pitch (`+0x230`, radians, clamped to `[-0.4, +0.5]`).
    pub pitch_rad: f32,
    /// Current smoothed yaw (`+0x234`, radians, clamped to `[-0.8, +0.8]`).
    pub yaw_rad: f32,
    /// Previous-tick pitch for sub-frame interpolation.
    pub prev_pitch_rad: f32,
    /// Previous-tick yaw for sub-frame interpolation.
    pub prev_yaw_rad: f32,
}

impl Default for HeadLookTracker {
    fn default() -> Self {
        Self {
            pitch_rad: 0.0,
            yaw_rad: 0.0,
            prev_pitch_rad: 0.0,
            prev_yaw_rad: 0.0,
        }
    }
}

impl HeadLookTracker {
    /// Steps pitch and yaw toward `target_world_pos` (or back to `0.0` when `None` or suppressed)
    /// using `PlayerCheckNeckAngle` (`0x00108e10`) clamps and slew speed.
    pub fn step(
        &mut self,
        player_eye_pos: Vec3,
        player_yaw: f32,
        target_world_pos: Option<Vec3>,
        suppressed: bool,
        dt: f32,
    ) {
        self.prev_pitch_rad = self.pitch_rad;
        self.prev_yaw_rad = self.yaw_rad;

        let (desired_pitch, desired_yaw) = if suppressed {
            (0.0, 0.0)
        } else if let Some(target) = target_world_pos {
            compute_desired_neck_angles(player_eye_pos, player_yaw, target)
        } else {
            (0.0, 0.0)
        };

        self.pitch_rad = close_to_angle_target(
            desired_pitch,
            NECK_PITCH_MIN_RAD,
            NECK_PITCH_MAX_RAD,
            NECK_TURN_SPEED,
            self.pitch_rad,
            dt,
        );
        self.yaw_rad = close_to_angle_target(
            desired_yaw,
            NECK_YAW_MIN_RAD,
            NECK_YAW_MAX_RAD,
            NECK_TURN_SPEED,
            self.yaw_rad,
            dt,
        );
    }

    /// Applies the 20% neck / 80% head rotational split (`m3_play.c:1578-1579`) to row-major local
    /// bone matrices `locals` in-place without transposing or rotating local translation (`M_new[0..3][0..3] = M[0..3][0..3] * R_delta`,
    /// matching `shCharacterAnimePartsControl` `0x00100a40`, `Chacter/anime.c:415-429`).
    pub fn apply_to_locals(
        &self,
        locals: &mut [[[f32; 4]; 4]],
        neck_bone: usize,
        head_bone: usize,
        alpha: f32,
    ) {
        let t = alpha.clamp(0.0, 1.0);
        let pitch = self.prev_pitch_rad + (self.pitch_rad - self.prev_pitch_rad) * t;
        let yaw = self.prev_yaw_rad + (self.yaw_rad - self.prev_yaw_rad) * t;

        if pitch.abs() < 1e-5 && yaw.abs() < 1e-5 {
            return;
        }

        apply_bone_yaw_pitch(
            locals,
            neck_bone,
            yaw * NECK_UPPER_SPINE_SHARE,
            pitch * NECK_UPPER_SPINE_SHARE,
        );
        apply_bone_yaw_pitch(
            locals,
            head_bone,
            yaw * NECK_CRANIAL_SHARE,
            pitch * NECK_CRANIAL_SHARE,
        );
    }

    /// Applies the 20% neck / 80% head rotational split (`PlayerCheckNeckAngle` `0x00108e10`,
    /// `shCharacterAnimePartsControl` `0x00100a40`) **after** `evaluate_pose_into` on evaluated
    /// model-space row-major matrices `pose_world`, propagating each bone's pivot rotation to all
    /// descendant bones in `parents` without overwriting keyframed rotations or corrupting translations.
    pub fn apply_to_pose(
        &self,
        pose_world: &mut [[[f32; 4]; 4]],
        parents: &[u8],
        neck_bone: usize,
        head_bone: usize,
        alpha: f32,
    ) {
        let t = alpha.clamp(0.0, 1.0);
        let pitch = self.prev_pitch_rad + (self.pitch_rad - self.prev_pitch_rad) * t;
        let yaw = self.prev_yaw_rad + (self.yaw_rad - self.prev_yaw_rad) * t;

        if pitch.abs() < 1e-5 && yaw.abs() < 1e-5 {
            return;
        }

        apply_pose_bone_yaw_pitch(
            pose_world,
            parents,
            neck_bone,
            yaw * NECK_UPPER_SPINE_SHARE,
            pitch * NECK_UPPER_SPINE_SHARE,
        );
        apply_pose_bone_yaw_pitch(
            pose_world,
            parents,
            head_bone,
            yaw * NECK_CRANIAL_SHARE,
            pitch * NECK_CRANIAL_SHARE,
        );
    }
}

/// Computes `(desired_pitch_rad, desired_yaw_rad)` matching `PlayerCheckNeckAngle`
/// (`0x00108e10` lines 1539-1559).
#[must_use]
pub fn compute_desired_neck_angles(
    player_eye_pos: Vec3,
    player_yaw: f32,
    target_pos: Vec3,
) -> (f32, f32) {
    let dx = target_pos.x - player_eye_pos.x;
    let dy = target_pos.y - player_eye_pos.y;
    let dz = target_pos.z - player_eye_pos.z;
    let horiz = (dx * dx + dz * dz).sqrt();

    // `shAtan2(dx, dz)` (`rot_y = 0` faces `+Z`).
    let world_yaw = dx.atan2(dz);
    let yaw = wrap_angle_pi(world_yaw - player_yaw);
    // `+Y` is down in SH2 world space: target above eye (`dy < 0`) gives positive upward pitch.
    let pitch = wrap_angle_pi(-dy.atan2(horiz));
    (pitch, yaw)
}

/// Selects the highest-priority look target from `candidates` (`sh2pc.exe` `FUN_005424a0` / `FUN_005426b0`
/// + `Patches/TeddyBearLookFix.cpp` `0x00542769`).
///
/// Priority:
/// 1. Active combat/aim target (`is_combat_target`, `_DAT_003c83c4` in `PlayerCheckNeckAngle`).
/// 2. Nearest valid candidate within `max_radius_mm` passing `0x700` Y-band (`[-600, +1000]` mm)
///    and `TeddyBearLookFix` (`0x0727` skipped when `fixes.teddy_bear_look_fix && flag(0xAA)`).
#[must_use]
pub fn select_head_look_target(
    player_pos: Vec3,
    candidates: &[LookTargetCandidate],
    max_radius_mm: f32,
    flags: &EventFlags,
    enable_sh2ee_world_fixes: bool,
) -> Option<LookTargetCandidate> {
    let fixes = if enable_sh2ee_world_fixes {
        Act1VanillaFixesConfig::enhanced()
    } else {
        Act1VanillaFixesConfig::vanilla_ps2()
    };

    let max_dist_sq = max_radius_mm * max_radius_mm;
    let mut best_combat: Option<(f32, LookTargetCandidate)> = None;
    let mut best_general: Option<(f32, LookTargetCandidate)> = None;

    for &cand in candidates {
        if !cand.is_active {
            continue;
        }
        if cand.prop_kind == ACT1_PROP_KIND_TEDDY_BEAR
            && !act1_should_james_look_at_prop(cand.prop_kind, flags, fixes)
        {
            continue;
        }
        if cand.kind == LookTargetKind::ItemOrProp {
            let dy = cand.position.y - player_pos.y;
            if !(LOOK_ITEM_MIN_DY_MM..=LOOK_ITEM_MAX_DY_MM).contains(&dy) {
                continue;
            }
        }
        let dist_sq = cand.position.distance_squared(player_pos);
        if dist_sq > max_dist_sq {
            continue;
        }
        if cand.is_combat_target {
            if best_combat.is_none_or(|(d, _)| dist_sq < d) {
                best_combat = Some((dist_sq, cand));
            }
        } else if best_general.is_none_or(|(d, _)| dist_sq < d) {
            best_general = Some((dist_sq, cand));
        }
    }

    best_combat.or(best_general).map(|(_, c)| c)
}

fn close_to_angle_target(
    target: f32,
    min_rad: f32,
    max_rad: f32,
    speed: f32,
    current: f32,
    dt: f32,
) -> f32 {
    let clamped_target = target.clamp(min_rad, max_rad);
    let step = speed * dt.max(0.0);
    let diff = wrap_angle_pi(clamped_target - current);
    if diff.abs() <= step {
        clamped_target
    } else {
        (current + diff.signum() * step).clamp(min_rad, max_rad)
    }
}

fn wrap_angle_pi(mut a: f32) -> f32 {
    while a > std::f32::consts::PI {
        a -= std::f32::consts::TAU;
    }
    while a < -std::f32::consts::PI {
        a += std::f32::consts::TAU;
    }
    a
}

/// Composes a row-major 3x3 rotation `rot_3x3` into the upper-left 3x3 block of `mat`
/// (`M_new[0..3][0..3] = M[0..3][0..3] * rot_3x3`), preserving translation `M[0..3][3]` verbatim
/// (`shCharacterAnimePartsControl` `0x00100a40`, `Chacter/anime.c:415-429`).
#[inline]
fn compose_row_major_rot3(mat: &mut [[f32; 4]; 4], rot_3x3: [[f32; 3]; 3]) {
    let orig = *mat;
    for r in 0..3 {
        for c in 0..3 {
            mat[r][c] = orig[r][0] * rot_3x3[0][c]
                + orig[r][1] * rot_3x3[1][c]
                + orig[r][2] * rot_3x3[2][c];
        }
    }
}

#[inline]
fn is_descendant_of(mut idx: usize, ancestor: usize, parents: &[u8]) -> bool {
    let mut depth = 0usize;
    while idx < parents.len() && depth < parents.len() {
        let p = parents[idx];
        if p == 255 {
            return false;
        }
        let p_usize = p as usize;
        if p_usize == ancestor {
            return true;
        }
        idx = p_usize;
        depth += 1;
    }
    false
}

fn apply_bone_yaw_pitch(locals: &mut [[[f32; 4]; 4]], bone: usize, yaw: f32, pitch: f32) {
    let Some(slot) = locals.get_mut(bone) else {
        return;
    };
    let rot_delta = Quat::from_euler(glam::EulerRot::YXZ, yaw, pitch, 0.0);
    let rot_3x3 = quat_to_matrix(rot_delta.to_array());
    compose_row_major_rot3(slot, rot_3x3);
}

fn apply_pose_bone_yaw_pitch(
    pose_world: &mut [[[f32; 4]; 4]],
    parents: &[u8],
    bone: usize,
    yaw: f32,
    pitch: f32,
) {
    if bone >= pose_world.len() {
        return;
    }
    let w_old = pose_world[bone];
    let mut w_new = w_old;
    let rot_delta = Quat::from_euler(glam::EulerRot::YXZ, yaw, pitch, 0.0);
    let rot_3x3 = quat_to_matrix(rot_delta.to_array());
    compose_row_major_rot3(&mut w_new, rot_3x3);

    let pivot_xform = mat4_mul(w_new, mat4_inverse(w_old));
    pose_world[bone] = w_new;

    for (idx, slot) in pose_world.iter_mut().enumerate() {
        if idx != bone && is_descendant_of(idx, bone, parents) {
            *slot = mat4_mul(pivot_xform, *slot);
        }
    }
}
