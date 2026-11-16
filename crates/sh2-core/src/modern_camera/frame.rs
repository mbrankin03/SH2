//! Per-tick input and evaluated output frame types for [`super::ModernCameraRig`] (`SH2-93`).

use glam::{Mat4, Vec3};

use crate::camera::CameraSnapshot;
use crate::scenario::PlayableCharacter;

use super::first_person::{PlayerMeshVisibility, STANDARD_NEAR_CLIP_UNITS};
use super::handoff::GameplayCameraMode;
use super::ots::OtsReticleState;

/// Per-tick simulation inputs consumed by [`super::ModernCameraRig::tick`] at 60 Hz (`SH2-93`).
#[derive(Debug, Clone, Copy)]
pub struct ModernCameraTickInput {
    /// Active playable protagonist (`James` vs `Maria`).
    pub character: PlayableCharacter,
    /// Player world position in SH2 units (`+Y` down).
    pub player_pos: Vec3,
    /// Player horizontal facing yaw in radians (`0.0` = `+Z`).
    pub player_yaw: f32,
    /// Player planar locomotion speed in SH2 world units/sec.
    pub linear_speed: f32,
    /// Filtered right-stick X (`-1.0..=1.0`).
    pub right_stick_x: f32,
    /// Filtered right-stick Y (`-1.0..=1.0`, positive = stick up).
    pub right_stick_y: f32,
    /// Accumulated mouse + gyro (`SH2-87`) yaw delta in radians for this tick.
    pub extra_yaw_delta_rad: f32,
    /// Accumulated mouse + gyro (`SH2-87`) pitch delta in radians for this tick.
    pub extra_pitch_delta_rad: f32,
    /// User look sensitivity multiplier (`0.5..=3.0`).
    pub sensitivity: f32,
    /// Whether Ready Weapon (`R2`) is held.
    pub ready_weapon: bool,
    /// Whether Quick Turn (`180 deg`) triggered on this tick.
    pub quick_turn_pressed: bool,
    /// Whether OTS shoulder swap (`L3`) triggered on this tick.
    pub shoulder_swap_pressed: bool,
    /// Whether the player is inside the Toluca Prison Bug Room (flashlight flicker).
    pub in_bug_room: bool,
}

/// Evaluated 60 Hz output frame from [`super::ModernCameraRig`] (`SH2-93`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ModernCameraFrame {
    /// Active gameplay camera mode.
    pub active_mode: GameplayCameraMode,
    /// Evaluated camera eye, look-at target, up vector, and vertical FOV in radians.
    pub snapshot: CameraSnapshot,
    /// Recommended perspective near-clip distance in SH2 world units (`8.0` FPS, `25.0` Rail/OTS).
    pub near_clip: f32,
    /// Character submesh visibility directive (`FullBody` vs `HideHeadAndUpperBody`).
    pub player_visibility: PlayerMeshVisibility,
    /// Optional subtle diegetic reticle state.
    pub reticle: OtsReticleState,
    /// World-space ranged weapon aim ray origin in SH2 units.
    pub aim_origin: Vec3,
    /// Unit ranged weapon aim ray direction in SH2 world space.
    pub aim_dir: Vec3,
    /// World-space flashlight emitter origin in SH2 units.
    pub flashlight_origin: Vec3,
    /// Unit flashlight beam direction in SH2 world space.
    pub flashlight_dir: Vec3,
}

impl Default for ModernCameraFrame {
    fn default() -> Self {
        Self {
            active_mode: GameplayCameraMode::ClassicPs2Rail,
            snapshot: CameraSnapshot::default(),
            near_clip: STANDARD_NEAR_CLIP_UNITS,
            player_visibility: PlayerMeshVisibility::FullBody,
            reticle: OtsReticleState::Hidden,
            aim_origin: Vec3::new(0.0, -700.0, 0.0),
            aim_dir: Vec3::Z,
            flashlight_origin: Vec3::new(0.0, -700.0, 45.0),
            flashlight_dir: Vec3::Z,
        }
    }
}

impl ModernCameraFrame {
    /// Interpolates sub-frame [`CameraSnapshot`] between `prev` and `self` at `alpha` (`0.0..=1.0`).
    #[inline]
    pub fn interpolate_snapshot_from(&self, prev: &Self, alpha: f32) -> CameraSnapshot {
        let t = alpha.clamp(0.0, 1.0);
        CameraSnapshot {
            eye: prev.snapshot.eye.lerp(self.snapshot.eye, t),
            target: prev.snapshot.target.lerp(self.snapshot.target, t),
            up: prev
                .snapshot
                .up
                .lerp(self.snapshot.up, t)
                .normalize_or(Vec3::new(0.0, -1.0, 0.0)),
            fov_y: prev.snapshot.fov_y + (self.snapshot.fov_y - prev.snapshot.fov_y) * t,
        }
    }

    /// Computes right-handed view matrix (`Mat4::look_at_rh`) between `prev` and `self` at `alpha`.
    #[inline]
    pub fn interpolate_view_from(&self, prev: &Self, alpha: f32) -> Mat4 {
        let s = self.interpolate_snapshot_from(prev, alpha);
        Mat4::look_at_rh(s.eye, s.target, s.up)
    }
}
