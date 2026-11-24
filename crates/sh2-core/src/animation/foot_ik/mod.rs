//! Analytical 2-bone Foot IK on stairs and slopes (`SH2-85`).
//!
//! # Ground Truth & Binary References
//! - **`PlayerCheckFootLine` (`SLUS_202.28` `Chacter/m3_play.c` `0x001078f0`):**
//!   - Casts vertical floor probes from James's shin/knee bones (`0x0d = 13`, `0x0e = 14` in
//!     `hll_jms.mdl` / `hhh_jms.mdl`) using `clCheckHitEyesOnlyFloor` (`0x00138f10`) across
//!     `[foot_y - 250.0, foot_y + 750.0]` mm (`+Y` down).
//! - **`shGetJamesFootPos` (`SLUS_202.28` `Chacter/m3_play.c` `0x00110b60`):**
//!   - Reads James's ankle (`0x1a = 26`) and toe (`0x1e = 30`) world positions.
//! - **`MariaCheckFootLine` (`SLUS_202.28` `Chacter/m3_maria_sub.c` `0x002262f0`):**
//!   - Casts identical `[-250.0, +750.0]` mm `clCheckHitEyesOnlyFloor` probes from Maria's
//!     leg bones (`7`, `8` in `lll_mar.mdl`).
//! - **Native-Upgrade Gating (`AGENTS.md` Rule 3.4):**
//!   - Strictly gated behind `GameOptions::enable_foot_ik` (default `false` in `Ps2Authentic`
//!     and `Sh2ee`, `true` in `NativeEnhanced`) so 1:1 PS2 keyframed animations remain untouched
//!     when disabled.

mod math;
#[cfg(test)]
mod tests;

pub use math::{
    clamped_normal_alignment, model_to_world_pos, solve_two_bone_ik_angles, world_to_model_dir,
};

use glam::Vec3;

use crate::collision::{CollisionController, CollisionWorld};
use math::{bone_translation, solve_single_leg};

/// Upward floor-probe offset in SH2 world mm (`-250.0` along `+Y`-down,
/// `PlayerCheckFootLine` `0x001078f0` / `MariaCheckFootLine` `0x002262f0`).
pub const FOOT_PROBE_UP_MM: f32 = 250.0;

/// Downward floor-probe offset in SH2 world mm (`+750.0` along `+Y`-down,
/// `PlayerCheckFootLine` `0x001078f0` / `MariaCheckFootLine` `0x002262f0`).
pub const FOOT_PROBE_DOWN_MM: f32 = 750.0;

/// Maximum vertical foot adjustment in mm, matching `PlayerSetHeight` (`0x00107aa0`)
/// step/freefall threshold (`250.0` mm).
pub const MAX_FOOT_IK_STEP_MM: f32 = 250.0;

/// Maximum ankle surface-normal tilt (`pi / 4` = 45 degrees) to prevent hyper-flexion on walls.
pub const MAX_ANKLE_TILT_RAD: f32 = std::f32::consts::FRAC_PI_4;

/// Exponential smoothing slew rate (`20.0` s^-1, `0x41a00000`, matching
/// `PlayerCheckNeckAngle` `0x00108e10`).
pub const FOOT_IK_SMOOTH_SPEED: f32 = 20.0;

/// Bone index chain for one character leg in `.mdl` skeleton hierarchy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LegChainSpec {
    /// Upper-leg hip/thigh bone index (`parent == lower_root`).
    pub hip: usize,
    /// Upper-knee bone index (`parent == hip`).
    pub knee: usize,
    /// Lower-knee/shin bone index (`parent == knee`, probed by `PlayerCheckFootLine`).
    pub shin: usize,
    /// Ankle joint bone index (`parent == shin`).
    pub ankle: usize,
    /// Toe/sole tip bone index (`parent == ankle`).
    pub toe: usize,
}

/// Complete two-leg IK rig specification for a character skeleton.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FootIkRigSpec {
    /// Upper-body root bone (`parent == 255`, `0` in `hll_jms.mdl` & `lll_mar.mdl`).
    pub upper_root: usize,
    /// Lower-body pelvis root bone (`parent == 255`, `1` in `hll_jms.mdl` & `lll_mar.mdl`).
    pub lower_root: usize,
    /// Left leg bone chain.
    pub left: LegChainSpec,
    /// Right leg bone chain.
    pub right: LegChainSpec,
}

impl FootIkRigSpec {
    /// James Sunderland rig verified from `hll_jms.mdl` (41 bones) & `hhh_jms.mdl` (71 bones),
    /// matching `PlayerCheckFootLine` (`0x001078f0`) and `shGetJamesFootPos` (`0x00110b60`).
    pub const JAMES: Self = Self {
        upper_root: 0,
        lower_root: 1,
        left: LegChainSpec {
            hip: 4,
            knee: 6,
            shin: 13,
            ankle: 25,
            toe: 29,
        },
        right: LegChainSpec {
            hip: 5,
            knee: 7,
            shin: 14,
            ankle: 26,
            toe: 30,
        },
    };

    /// Maria rig verified from `lll_mar.mdl` (36 bones), matching `MariaCheckFootLine` (`0x002262f0`).
    pub const MARIA: Self = Self {
        upper_root: 0,
        lower_root: 1,
        left: LegChainSpec {
            hip: 3,
            knee: 7,
            shin: 10,
            ankle: 20,
            toe: 24,
        },
        right: LegChainSpec {
            hip: 5,
            knee: 8,
            shin: 15,
            ankle: 22,
            toe: 26,
        },
    };

    /// Returns `true` if `parents` matches both left and right leg chains of this rig.
    #[must_use]
    pub fn matches_parents(&self, parents: &[u8]) -> bool {
        let chain_ok = |c: LegChainSpec| {
            let max_b = c.hip.max(c.knee).max(c.shin).max(c.ankle).max(c.toe);
            if max_b >= parents.len() {
                return false;
            }
            parents[c.hip] as usize == self.lower_root
                && parents[c.knee] as usize == c.hip
                && parents[c.shin] as usize == c.knee
                && parents[c.ankle] as usize == c.shin
                && parents[c.toe] as usize == c.ankle
        };
        self.upper_root < parents.len()
            && self.lower_root < parents.len()
            && chain_ok(self.left)
            && chain_ok(self.right)
    }
}

/// Output of a single floor probe at a foot's world XZ position.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FootGroundSample {
    /// Vertical offset relative to character `player_pos.y` (`+Y` down, clamped to `[-250, +250]` mm).
    pub delta_y_mm: f32,
    /// Upward-pointing surface normal in world space (`normal.y <= 0.0` in SH2 `+Y`-down coords).
    pub world_normal: Vec3,
}

/// Probes the `.cld` floor directly beneath a foot's world XZ coordinate using
/// `clCheckHitEyesOnlyFloor` (`SLUS_202.28` `0x00138f10`) across
/// `[player_pos.y - 250.0, player_pos.y + 750.0]` (`PlayerCheckFootLine` `0x001078f0`).
#[must_use]
pub fn probe_foot_ground(
    world: &CollisionWorld,
    foot_world_xz: Vec3,
    player_pos_y: f32,
) -> Option<FootGroundSample> {
    let origin = Vec3::new(
        foot_world_xz.x,
        player_pos_y - FOOT_PROBE_UP_MM,
        foot_world_xz.z,
    );
    let target = Vec3::new(
        foot_world_xz.x,
        player_pos_y + FOOT_PROBE_DOWN_MM,
        foot_world_xz.z,
    );
    let hit = world.check_hit_eyes_only_floor(origin, target)?;
    let delta_y_mm = (hit.point.y - player_pos_y).clamp(-MAX_FOOT_IK_STEP_MM, MAX_FOOT_IK_STEP_MM);
    let mut normal = hit.normal.normalize_or_zero();
    if normal.length_squared() < 1e-6 {
        normal = Vec3::NEG_Y;
    } else if normal.y > 0.0 {
        normal = -normal;
    }
    Some(FootGroundSample {
        delta_y_mm,
        world_normal: normal,
    })
}

/// Stateful analytical 2-bone Foot IK solver with exponential smoothing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FootIkSolver {
    /// Target character leg skeleton specification.
    pub rig: FootIkRigSpec,
    /// Smoothed vertical pelvis offset in mm (`>= 0.0` in `+Y` down).
    pub smoothed_pelvis_y: f32,
    /// Smoothed left foot target vertical delta in mm.
    pub smoothed_left_y: f32,
    /// Smoothed right foot target vertical delta in mm.
    pub smoothed_right_y: f32,
    /// Smoothed left floor normal in world space (`Vec3::NEG_Y` when flat).
    pub smoothed_left_normal: Vec3,
    /// Smoothed right floor normal in world space (`Vec3::NEG_Y` when flat).
    pub smoothed_right_normal: Vec3,
}

impl FootIkSolver {
    /// Creates a new Foot IK solver for the given [`FootIkRigSpec`].
    #[must_use]
    pub const fn new(rig: FootIkRigSpec) -> Self {
        Self {
            rig,
            smoothed_pelvis_y: 0.0,
            smoothed_left_y: 0.0,
            smoothed_right_y: 0.0,
            smoothed_left_normal: Vec3::NEG_Y,
            smoothed_right_normal: Vec3::NEG_Y,
        }
    }

    /// Creates a Foot IK solver configured for James Sunderland.
    #[must_use]
    pub const fn for_james() -> Self {
        Self::new(FootIkRigSpec::JAMES)
    }

    /// Creates a Foot IK solver configured for Maria.
    #[must_use]
    pub const fn for_maria() -> Self {
        Self::new(FootIkRigSpec::MARIA)
    }

    /// Resets smoothed pelvis and foot offsets to flat rest state.
    pub fn reset(&mut self) {
        self.smoothed_pelvis_y = 0.0;
        self.smoothed_left_y = 0.0;
        self.smoothed_right_y = 0.0;
        self.smoothed_left_normal = Vec3::NEG_Y;
        self.smoothed_right_normal = Vec3::NEG_Y;
    }

    /// Applies Foot IK to `pose_world` only when `enabled` is `true` (`GameOptions::enable_foot_ik`).
    /// When `false`, resets state and leaves `pose_world` untouched for 1:1 PS2 parity.
    /// When `dt <= 0.0` (e.g. paused), holds the current smoothed offsets without snapping.
    #[allow(clippy::too_many_arguments)]
    pub fn solve_if_enabled(
        &mut self,
        enabled: bool,
        world: &CollisionWorld,
        player_pos: Vec3,
        player_yaw: f32,
        dt: f32,
        parents: &[u8],
        pose_world: &mut [[[f32; 4]; 4]],
    ) {
        if !enabled {
            self.reset();
            return;
        }
        self.solve_into_world(world, player_pos, player_yaw, dt, parents, pose_world);
    }

    /// Solves analytical 2-bone Foot IK against a [`CollisionController`].
    pub fn solve_into(
        &mut self,
        collision: &CollisionController,
        player_pos: Vec3,
        player_yaw: f32,
        dt: f32,
        parents: &[u8],
        pose_world: &mut [[[f32; 4]; 4]],
    ) {
        self.solve_into_world(
            &collision.world,
            player_pos,
            player_yaw,
            dt,
            parents,
            pose_world,
        );
    }

    /// Solves analytical 2-bone Foot IK against a [`CollisionWorld`].
    ///
    /// When `dt <= 0.0` (paused), `alpha = 0.0` so the current smoothed pose is held cleanly.
    pub fn solve_into_world(
        &mut self,
        world: &CollisionWorld,
        player_pos: Vec3,
        player_yaw: f32,
        dt: f32,
        parents: &[u8],
        pose_world: &mut [[[f32; 4]; 4]],
    ) {
        if pose_world.len() < parents.len() || !self.rig.matches_parents(parents) {
            return;
        }

        let left_ankle_orig = bone_translation(&pose_world[self.rig.left.ankle]);
        let right_ankle_orig = bone_translation(&pose_world[self.rig.right.ankle]);

        if dt > 0.0 {
            let left_world = model_to_world_pos(left_ankle_orig, player_pos, player_yaw);
            let right_world = model_to_world_pos(right_ankle_orig, player_pos, player_yaw);

            let left_hit = probe_foot_ground(world, left_world, player_pos.y);
            let right_hit = probe_foot_ground(world, right_world, player_pos.y);

            let target_left_y = left_hit.map_or(0.0, |h| h.delta_y_mm);
            let target_right_y = right_hit.map_or(0.0, |h| h.delta_y_mm);
            let target_left_n = left_hit.map_or(Vec3::NEG_Y, |h| h.world_normal);
            let target_right_n = right_hit.map_or(Vec3::NEG_Y, |h| h.world_normal);

            let target_pelvis_y = target_left_y.max(target_right_y).max(0.0);
            let alpha = (dt * FOOT_IK_SMOOTH_SPEED).clamp(0.0, 1.0);

            self.smoothed_pelvis_y += (target_pelvis_y - self.smoothed_pelvis_y) * alpha;
            self.smoothed_left_y += (target_left_y - self.smoothed_left_y) * alpha;
            self.smoothed_right_y += (target_right_y - self.smoothed_right_y) * alpha;
            self.smoothed_left_normal = self
                .smoothed_left_normal
                .lerp(target_left_n, alpha)
                .normalize_or_zero();
            self.smoothed_right_normal = self
                .smoothed_right_normal
                .lerp(target_right_n, alpha)
                .normalize_or_zero();
        }

        if self.smoothed_pelvis_y.abs() > 1e-4 {
            for slot in pose_world.iter_mut() {
                slot[1][3] += self.smoothed_pelvis_y;
            }
        }

        let left_model_n = world_to_model_dir(self.smoothed_left_normal, player_yaw);
        let right_model_n = world_to_model_dir(self.smoothed_right_normal, player_yaw);

        solve_single_leg(
            pose_world,
            parents,
            self.rig.left,
            left_ankle_orig.y + self.smoothed_left_y,
            left_model_n,
        );
        solve_single_leg(
            pose_world,
            parents,
            self.rig.right,
            right_ankle_orig.y + self.smoothed_right_y,
            right_model_n,
        );
    }
}
