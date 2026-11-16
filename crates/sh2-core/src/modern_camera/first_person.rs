//! Head-bone anchored First-Person (FPS) camera controller (`SH2-93`).
//!
//! Anchors the gameplay camera to James's (`bone 15`, eyes `20`/`22` in `lll_jms`/`hll_jms`/`hhh_jms`)
//! or Maria's (`bone 9` in `lll_mar`, eyes `23`/`25` in `hhh_mar`) skeletal head rig (`SH2-8`/`SH2-12`),
//! with clean fallback eye height when bone pose matrices are unavailable, clamped pitch/yaw look,
//! configurable vertical FOV (`55..=100 deg`), tuned near-clip distance (`8.0` SH2 units = `1.6 cm`),
//! subtle toggleable footstep/head-bob sway, `.cld` room bounds containment, and typed
//! [`PlayerMeshVisibility`] (`FullBody` vs `HideHeadAndUpperBody`) to eliminate near-plane mesh clipping.

use glam::Vec3;
use serde::{Deserialize, Serialize};

use crate::camera::CameraSnapshot;
use crate::collision::CollisionWorld;
use crate::scenario::PlayableCharacter;
use crate::timing::lerp_angle_shortest;

use super::ots::{OtsReticleState, ReticleMode};
use super::spring_arm::{constrain_eye_in_room, SpringArmConfig};

/// Default First-Person near-clip plane in SH2 world units (`8.0` units = `1.6 cm` at `500 units/m`).
pub const FIRST_PERSON_NEAR_CLIP_UNITS: f32 = 8.0;

/// Standard Third-Person / PS2 Rail near-clip plane in SH2 world units (`25.0` units = `5.0 cm`).
pub const STANDARD_NEAR_CLIP_UNITS: f32 = 25.0;

/// Typed character mesh visibility command preventing near-plane head/collar clipping in First-Person (`SH2-93`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum PlayerMeshVisibility {
    /// Draw all active character submeshes (`ClassicPs2Rail`, `OverTheShoulder`, cutscenes, and planar mirrors).
    #[default]
    FullBody,
    /// Hide foreground head, hair, neck, and upper-torso submeshes in `FirstPerson` while preserving lower body/hands and mirror reflections.
    HideHeadAndUpperBody,
}

/// Toggleable gait sway / head-bob mode for First-Person locomotion (`SH2-93`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum HeadBobMode {
    /// Subtle vertical and lateral stride sway coupled to locomotion speed.
    #[default]
    Enabled,
    /// Perfectly steady horizon (accessibility / motion-comfort option).
    Disabled,
}

impl HeadBobMode {
    /// Converts from boolean configuration toggle.
    #[inline]
    pub const fn from_enabled(enabled: bool) -> Self {
        if enabled {
            Self::Enabled
        } else {
            Self::Disabled
        }
    }

    /// Returns `true` if subtle head-bob sway is active.
    #[inline]
    pub const fn is_enabled(self) -> bool {
        matches!(self, Self::Enabled)
    }
}

/// Ground-truth skeletal head rig specification extracted from retail `.mdl` bind poses (`SH2-8`/`SH2-12`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CharacterHeadRigSpec {
    /// Skeleton head bone index (`15` for James, `9` for Maria).
    pub head_bone_index: usize,
    /// Left eye bone index (`20` for James, `23` for high-LOD Maria).
    pub left_eye_bone_index: usize,
    /// Right eye bone index (`22` for James, `25` for high-LOD Maria).
    pub right_eye_bone_index: usize,
    /// Minimum skeleton `bone_count` required for `left_eye_bone_index`/`right_eye_bone_index` to be valid eye bones
    /// (`41` on `lll_jms`/`hll_jms`/`hhh_jms`; `73` on `hhh_mar`, since 36-bone `lll_mar` reuses `23`/`25` for arms).
    pub min_eye_rig_bones: usize,
    /// Eye midpoint offset `[x, y, z]` in head-bone local coordinates (SH2 units, `+Y` down, `+Z` forward).
    pub head_local_eye_offset: Vec3,
    /// Fallback model-space eye `(x, y, z)` when skeletal pose matrices have not been supplied (`+Y` down).
    pub fallback_model_eye: Vec3,
}

impl CharacterHeadRigSpec {
    /// Ground-truth rig spec for James Sunderland (`lll_jms.mdl` / `hll_jms.mdl` 41 bones & `hhh_jms.mdl` 71 bones).
    /// Head bone `15` sits at `(0.0, -809.1, -26.6)` with bone-local X along `-Y` neck; eyes `20`/`22` sit at `(±15.9, -844.6, +23.4)`.
    pub const JAMES: Self = Self {
        head_bone_index: 15,
        left_eye_bone_index: 20,
        right_eye_bone_index: 22,
        min_eye_rig_bones: 41,
        head_local_eye_offset: Vec3::new(38.1311, 0.0, 47.9967),
        fallback_model_eye: Vec3::new(0.0, -844.6, 23.4),
    };

    /// Ground-truth rig spec for Maria (`lll_mar.mdl` 36 bones & `hhh_mar.mdl` 73 bones).
    /// Head bone `9` sits at `(0.0, -794.4, -15.6)` (`M_init[9]^-1 * eye = (45.3310, 0.0019, 34.3863)`); eyes `23`/`25` in `hhh_mar` sit at `(±15.4, -825.6, +32.0)`.
    pub const MARIA: Self = Self {
        head_bone_index: 9,
        left_eye_bone_index: 23,
        right_eye_bone_index: 25,
        min_eye_rig_bones: 73,
        head_local_eye_offset: Vec3::new(45.3310, 0.0019, 34.3863),
        fallback_model_eye: Vec3::new(0.0, -825.6, 32.0),
    };

    /// Returns the skeletal head rig specification for `character`.
    #[inline]
    pub const fn for_character(character: PlayableCharacter) -> Self {
        match character {
            PlayableCharacter::James => Self::JAMES,
            PlayableCharacter::Maria => Self::MARIA,
        }
    }

    /// Evaluates the character's 3D world-space eye position in SH2 world units (`+Y` down)
    /// from evaluated row-major model-space bone matrices (`evaluate_pose_into`), or falls back
    /// cleanly to `fallback_model_eye` rotated by `(player_pos, player_yaw)`.
    pub fn resolve_world_eye_anchor(
        &self,
        player_pos: Vec3,
        player_yaw: f32,
        pose_matrices: Option<&[[[f32; 4]; 4]]>,
    ) -> Vec3 {
        let model_eye = if let Some(mats) = pose_matrices {
            if mats.len() >= self.min_eye_rig_bones
                && self.left_eye_bone_index < mats.len()
                && self.right_eye_bone_index < mats.len()
            {
                let ml = &mats[self.left_eye_bone_index];
                let mr = &mats[self.right_eye_bone_index];
                Vec3::new(
                    0.5 * (ml[0][3] + mr[0][3]),
                    0.5 * (ml[1][3] + mr[1][3]),
                    0.5 * (ml[2][3] + mr[2][3]),
                )
            } else if let Some(mh) = mats.get(self.head_bone_index) {
                let off = self.head_local_eye_offset;
                Vec3::new(
                    mh[0][0] * off.x + mh[0][1] * off.y + mh[0][2] * off.z + mh[0][3],
                    mh[1][0] * off.x + mh[1][1] * off.y + mh[1][2] * off.z + mh[1][3],
                    mh[2][0] * off.x + mh[2][1] * off.y + mh[2][2] * off.z + mh[2][3],
                )
            } else {
                self.fallback_model_eye
            }
        } else {
            self.fallback_model_eye
        };

        let (s, c) = player_yaw.sin_cos();
        Vec3::new(
            c * model_eye.x + s * model_eye.z + player_pos.x,
            model_eye.y + player_pos.y,
            -s * model_eye.x + c * model_eye.z + player_pos.z,
        )
    }
}

/// Configuration parameters for the First-Person camera (`SH2-93`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FirstPersonConfig {
    /// Vertical field-of-view in degrees (`55.0..=100.0`, default `75.0`).
    pub fov_y_degrees: f32,
    /// Near-clip distance in SH2 world units (`8.0` = `1.6 cm`).
    pub near_clip_units: f32,
    /// Minimum pitch angle in radians (negative looks upward `-Y`).
    pub min_pitch_radians: f32,
    /// Maximum pitch angle in radians (positive looks downward `+Y`).
    pub max_pitch_radians: f32,
    /// Right-stick horizontal yaw rate in radians per second at 1.0x sensitivity.
    pub stick_yaw_speed_rad_per_sec: f32,
    /// Right-stick vertical pitch rate in radians per second at 1.0x sensitivity.
    pub stick_pitch_speed_rad_per_sec: f32,
    /// Subtle vertical head-bob amplitude in SH2 world units (`5.5` = `1.1 cm`).
    pub bob_vertical_amplitude_units: f32,
    /// Subtle lateral head-sway amplitude in SH2 world units (`3.5` = `0.7 cm`).
    pub bob_lateral_amplitude_units: f32,
    /// Smoothing factor (`0.0..=1.0`) per 60 Hz tick applied to animated head-bone jitter.
    pub eye_anchor_smoothing: f32,
}

impl Default for FirstPersonConfig {
    fn default() -> Self {
        Self {
            fov_y_degrees: 75.0,
            near_clip_units: FIRST_PERSON_NEAR_CLIP_UNITS,
            min_pitch_radians: -1.30,
            max_pitch_radians: 1.25,
            stick_yaw_speed_rad_per_sec: 2.85,
            stick_pitch_speed_rad_per_sec: 2.05,
            bob_vertical_amplitude_units: 5.5,
            bob_lateral_amplitude_units: 3.5,
            eye_anchor_smoothing: 0.42,
        }
    }
}

/// Stateful head-bone First-Person camera rig (`SH2-93`).
#[derive(Debug, Clone)]
pub struct FirstPersonCameraRig {
    /// Tunable First-Person FOV, near-clip, pitch bounds, and sway amplitudes.
    pub config: FirstPersonConfig,
    /// Toggleable gait head-bob / footstep sway setting.
    pub head_bob_mode: HeadBobMode,
    /// Reticle visibility preference.
    pub reticle_mode: ReticleMode,
    /// Horizontal look yaw in radians (`0.0` = `+Z`).
    pub look_yaw: f32,
    /// Vertical look pitch in radians (`< 0.0` up `-Y`, `> 0.0` down `+Y`).
    pub look_pitch: f32,
    /// Continuous gait sway phase in radians `[0, TAU)`.
    pub bob_phase: f32,
    /// Smoothed world-space eye anchor in SH2 units.
    pub smoothed_eye: Vec3,
    /// Cached world-space eye anchor updated when external skeletal pose matrices are supplied.
    pub external_bone_eye: Option<Vec3>,
}

impl Default for FirstPersonCameraRig {
    fn default() -> Self {
        Self::new(FirstPersonConfig::default())
    }
}

impl FirstPersonCameraRig {
    /// Creates a new First-Person camera rig with default 75 deg vertical FOV and subtle sway.
    pub fn new(config: FirstPersonConfig) -> Self {
        Self {
            config,
            head_bob_mode: HeadBobMode::Enabled,
            reticle_mode: ReticleMode::AimOnly,
            look_yaw: 0.0,
            look_pitch: 0.0,
            bob_phase: 0.0,
            smoothed_eye: CharacterHeadRigSpec::JAMES.fallback_model_eye,
            external_bone_eye: None,
        }
    }

    /// Updates the cached world-space eye position from evaluated skeletal `pose_matrices` (`SH2-8`/`SH2-12`).
    pub fn update_from_skeleton_pose(
        &mut self,
        character: PlayableCharacter,
        player_pos: Vec3,
        player_yaw: f32,
        pose_matrices: Option<&[[[f32; 4]; 4]]>,
    ) {
        self.external_bone_eye = pose_matrices.map(|mats| {
            CharacterHeadRigSpec::for_character(character).resolve_world_eye_anchor(
                player_pos,
                player_yaw,
                Some(mats),
            )
        });
    }

    /// Snaps the First-Person rig immediately to `(player_pos, player_yaw)` for `character`.
    pub fn reset_to_player(
        &mut self,
        character: PlayableCharacter,
        player_pos: Vec3,
        player_yaw: f32,
    ) {
        self.look_yaw = player_yaw;
        self.look_pitch = 0.0;
        self.bob_phase = 0.0;
        self.external_bone_eye = None;
        self.smoothed_eye = CharacterHeadRigSpec::for_character(character)
            .resolve_world_eye_anchor(player_pos, player_yaw, None);
    }

    /// Unit forward view vector in `+Y`-down world space for current `(look_yaw, look_pitch)`.
    #[inline]
    pub fn view_forward(&self) -> Vec3 {
        let (sy, cy) = self.look_yaw.sin_cos();
        let (sp, cp) = self.look_pitch.sin_cos();
        Vec3::new(sy * cp, sp, cy * cp).normalize_or(Vec3::Z)
    }

    /// Steps the First-Person camera for one 60 Hz tick, returning `(snapshot, aim_origin, aim_dir, reticle)`.
    #[allow(clippy::too_many_arguments)]
    pub fn tick(
        &mut self,
        character: PlayableCharacter,
        player_pos: Vec3,
        player_yaw: f32,
        linear_speed: f32,
        right_stick_x: f32,
        right_stick_y: f32,
        extra_yaw_delta_rad: f32,
        extra_pitch_delta_rad: f32,
        sensitivity: f32,
        ready_weapon: bool,
        quick_turn_pressed: bool,
        collision: &CollisionWorld,
        spring_cfg: &SpringArmConfig,
    ) -> (CameraSnapshot, Vec3, Vec3, OtsReticleState) {
        let dt = 1.0 / 60.0;
        if quick_turn_pressed {
            self.look_yaw =
                lerp_angle_shortest(self.look_yaw, self.look_yaw + std::f32::consts::PI, 1.0);
        }

        let aim_scale = if ready_weapon { 0.70 } else { 1.0 };
        let sens = sensitivity.clamp(0.2, 4.0) * aim_scale;
        let yaw_step = right_stick_x * self.config.stick_yaw_speed_rad_per_sec * sens * dt
            + extra_yaw_delta_rad;
        let pitch_step = -right_stick_y * self.config.stick_pitch_speed_rad_per_sec * sens * dt
            + extra_pitch_delta_rad;

        self.look_yaw = (self.look_yaw + yaw_step).rem_euclid(std::f32::consts::TAU);
        if self.look_yaw > std::f32::consts::PI {
            self.look_yaw -= std::f32::consts::TAU;
        }
        self.look_pitch = (self.look_pitch + pitch_step)
            .clamp(self.config.min_pitch_radians, self.config.max_pitch_radians);

        let raw_anchor = self.external_bone_eye.unwrap_or_else(|| {
            CharacterHeadRigSpec::for_character(character)
                .resolve_world_eye_anchor(player_pos, player_yaw, None)
        });
        self.smoothed_eye = self.smoothed_eye.lerp(
            raw_anchor,
            self.config.eye_anchor_smoothing.clamp(0.05, 1.0),
        );

        // Subtle stride head-bob & footstep sway (cleanly zero when Disabled)
        let speed_norm = (linear_speed / 1750.0).clamp(0.0, 1.0);
        let sway_offset = match self.head_bob_mode {
            HeadBobMode::Enabled if speed_norm > 0.03 => {
                let freq_hz = 1.45 + 1.35 * speed_norm;
                self.bob_phase =
                    (self.bob_phase + std::f32::consts::TAU * freq_hz * dt) % std::f32::consts::TAU;
                let (sy, cy) = self.look_yaw.sin_cos();
                let right = Vec3::new(cy, 0.0, -sy);
                let bob_y = (self.bob_phase * 2.0).sin()
                    * self.config.bob_vertical_amplitude_units
                    * speed_norm;
                let sway_lat =
                    self.bob_phase.sin() * self.config.bob_lateral_amplitude_units * speed_norm;
                right * sway_lat + Vec3::new(0.0, bob_y, 0.0)
            }
            _ => {
                self.bob_phase *= 0.85;
                Vec3::ZERO
            }
        };

        let constrained_eye = constrain_eye_in_room(
            player_pos,
            self.smoothed_eye + sway_offset,
            collision,
            spring_cfg,
        );
        let forward = self.view_forward();
        let base_fov = self.config.fov_y_degrees.clamp(55.0, 100.0).to_radians();
        let fov_y = if ready_weapon {
            base_fov * 0.86
        } else {
            base_fov
        };

        let reticle = match self.reticle_mode {
            ReticleMode::Off => OtsReticleState::Hidden,
            ReticleMode::AimOnly if ready_weapon => OtsReticleState::SubtleDot {
                opacity: 0.72,
                spread: speed_norm * 0.4,
            },
            ReticleMode::AimOnly => OtsReticleState::Hidden,
            ReticleMode::AlwaysSubtle => OtsReticleState::SubtleDot {
                opacity: if ready_weapon { 0.75 } else { 0.25 },
                spread: speed_norm * 0.45,
            },
        };

        let snapshot = CameraSnapshot {
            eye: constrained_eye,
            target: constrained_eye + forward * 1000.0,
            up: Vec3::new(0.0, -1.0, 0.0),
            fov_y,
        };
        (snapshot, constrained_eye, forward, reticle)
    }
}
