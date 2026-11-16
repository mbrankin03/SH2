//! Head-bone anchored First-Person (FPS) camera controller (`SH2-93`).
//!
//! Anchors the gameplay camera to James's (`bone 15`, eyes `20`/`22` in `lll_jms`/`hll_jms`/`hhh_jms`)
//! or Maria's (`bone 9` in `lll_mar`, eyes `23`/`25` in `hhh_mar`) skeletal head rig (`SH2-8`/`SH2-12`),
//! with clean fallback eye height when bone pose matrices are unavailable, clamped pitch/yaw look,
//! configurable vertical FOV (`55..=100 deg`), tuned near-clip distance (`8.0` SH2 units = `1.6 cm`),
//! subtle toggleable footstep/head-bob sway, `.cld` room bounds containment, typed
//! [`PlayerMeshVisibility`] (`FullBody` vs `HideHeadOnly`), and a natural forward neck-tilt with a
//! chest-clear envelope (`head_rig.rs`) so looking down shows the outside of the jacket, arms and legs.

use glam::Vec3;
use serde::{Deserialize, Serialize};

use crate::camera::CameraSnapshot;
use crate::collision::CollisionWorld;
use crate::scenario::PlayableCharacter;

use super::frame::{ModernCameraTickInput, QuickTurnPhase};
use super::head_rig::{model_to_world, CharacterHeadRigSpec};
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
    /// Hide **only** the foreground cranial head subtree (skull, face, eyes, jaw, hair) in `FirstPerson`,
    /// keeping neck stump, collar, torso, arms, hands, legs and shoes visible; planar mirror
    /// reflections still draw the full body including head and hair.
    HideHeadOnly,
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
            // `1.10 rad` = `63.0 deg`: natural chin-to-chest limit; the sightline grazes the jacket front.
            max_pitch_radians: 1.10,
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
    /// Cached world-space head joint (`head_bone_index` translation) from external skeletal pose matrices.
    pub external_head_joint: Option<Vec3>,
    /// Active 180-degree quick-turn angular state (`QUICK_TURN_SPEED_RAD_PER_SEC`).
    pub quick_turn: QuickTurnPhase,
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
            external_head_joint: None,
            quick_turn: QuickTurnPhase::Idle,
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
        let spec = CharacterHeadRigSpec::for_character(character);
        self.external_bone_eye = pose_matrices
            .map(|mats| spec.resolve_world_eye_anchor(player_pos, player_yaw, Some(mats)));
        self.external_head_joint = pose_matrices
            .map(|mats| spec.resolve_world_head_joint(player_pos, player_yaw, Some(mats)));
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
        self.external_head_joint = None;
        self.quick_turn = QuickTurnPhase::Idle;
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

    /// Unit horizontal (`Y = 0`) look direction for the current `look_yaw`.
    #[inline]
    pub fn look_forward_xz(&self) -> Vec3 {
        let (sy, cy) = self.look_yaw.sin_cos();
        Vec3::new(sy, 0.0, cy)
    }

    /// Steps the First-Person camera for one 60 Hz tick, returning `(snapshot, aim_origin, aim_dir, reticle)`.
    ///
    /// `input` must already be gated by any cutscene/cutaway lock (see [`super::ModernCameraRig::tick`]).
    pub fn tick(
        &mut self,
        input: &ModernCameraTickInput,
        collision: &CollisionWorld,
        spring_cfg: &SpringArmConfig,
    ) -> (CameraSnapshot, Vec3, Vec3, OtsReticleState) {
        let dt = 1.0 / 60.0;
        let spec = CharacterHeadRigSpec::for_character(input.character);
        self.step_look_angles(input, dt);

        let raw_anchor = self.external_bone_eye.unwrap_or_else(|| {
            spec.resolve_world_eye_anchor(input.player_pos, input.player_yaw, None)
        });
        self.smoothed_eye = self.smoothed_eye.lerp(
            raw_anchor,
            self.config.eye_anchor_smoothing.clamp(0.05, 1.0),
        );
        let head_joint = self.external_head_joint.unwrap_or_else(|| {
            spec.resolve_world_head_joint(input.player_pos, input.player_yaw, None)
        });

        let speed_norm = (input.linear_speed / 1750.0).clamp(0.0, 1.0);
        let sway_offset = self.step_head_bob(speed_norm, dt);
        let base_fov = self.config.fov_y_degrees.clamp(55.0, 100.0).to_radians();
        let fov_y = if input.ready_weapon {
            base_fov * 0.86
        } else {
            base_fov
        };

        // Natural chin-forward neck tilt, then push the eye clear of the neck opening along the view.
        let tilt = model_to_world(
            Vec3::ZERO,
            input.player_yaw,
            spec.neck_tilt_model_offset(self.look_pitch),
        );
        let desired = self.clear_neck_opening(
            &spec,
            self.smoothed_eye + tilt + sway_offset,
            head_joint,
            0.5 * fov_y,
        );
        let constrained_eye =
            constrain_eye_in_room(input.player_pos, desired, collision, spring_cfg);
        // If `.cld` walls shoved the eye back toward the chest, cap pitch so the view never dips inside.
        self.cap_pitch_for_clearance(&spec, constrained_eye, head_joint, 0.5 * fov_y);

        let forward = self.view_forward();
        let reticle = match self.reticle_mode {
            ReticleMode::Off => OtsReticleState::Hidden,
            ReticleMode::AimOnly if input.ready_weapon => OtsReticleState::SubtleDot {
                opacity: 0.72,
                spread: speed_norm * 0.4,
            },
            ReticleMode::AimOnly => OtsReticleState::Hidden,
            ReticleMode::AlwaysSubtle => OtsReticleState::SubtleDot {
                opacity: if input.ready_weapon { 0.75 } else { 0.25 },
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

    /// Applies quick-turn, right-stick, and mouse/gyro deltas to `(look_yaw, look_pitch)`.
    fn step_look_angles(&mut self, input: &ModernCameraTickInput, dt: f32) {
        if input.quick_turn_pressed {
            self.quick_turn.trigger(self.look_yaw);
        }
        let is_quick_turning = self.quick_turn.step(&mut self.look_yaw, dt);

        let aim_scale = if input.ready_weapon { 0.70 } else { 1.0 };
        let sens = input.sensitivity.clamp(0.2, 4.0) * aim_scale;
        if !is_quick_turning {
            let yaw_step =
                input.right_stick_x * self.config.stick_yaw_speed_rad_per_sec * sens * dt
                    + input.extra_yaw_delta_rad;
            self.look_yaw = (self.look_yaw + yaw_step).rem_euclid(std::f32::consts::TAU);
            if self.look_yaw > std::f32::consts::PI {
                self.look_yaw -= std::f32::consts::TAU;
            }
        }
        let pitch_step =
            -input.right_stick_y * self.config.stick_pitch_speed_rad_per_sec * sens * dt
                + input.extra_pitch_delta_rad;
        self.look_pitch = (self.look_pitch + pitch_step)
            .clamp(self.config.min_pitch_radians, self.config.max_pitch_radians);
    }

    /// Advances subtle stride head-bob & footstep sway, returning the world offset (zero when `Disabled`).
    fn step_head_bob(&mut self, speed_norm: f32, dt: f32) -> Vec3 {
        match self.head_bob_mode {
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
        }
    }

    /// Pushes `desired_eye` forward along the horizontal look direction until the bottom frustum
    /// edge (`look_pitch + half_fov_y`) passes in front of the neck opening's front lip.
    fn clear_neck_opening(
        &self,
        spec: &CharacterHeadRigSpec,
        desired_eye: Vec3,
        head_joint: Vec3,
        half_fov_y: f32,
    ) -> Vec3 {
        let fwd_xz = self.look_forward_xz();
        let drop = spec.neck_lip_world_y(head_joint) - desired_eye.y;
        let need = spec.min_clear_forward(drop, self.look_pitch + half_fov_y);
        let have = (desired_eye - head_joint).dot(fwd_xz);
        if have < need {
            desired_eye + fwd_xz * (need - have)
        } else {
            desired_eye
        }
    }

    /// Lowers `look_pitch` (never below level) when the room-constrained eye no longer clears the neck opening.
    fn cap_pitch_for_clearance(
        &mut self,
        spec: &CharacterHeadRigSpec,
        eye: Vec3,
        head_joint: Vec3,
        half_fov_y: f32,
    ) {
        let forward = (eye - head_joint).dot(self.look_forward_xz());
        let drop = spec.neck_lip_world_y(head_joint) - eye.y;
        let cap = spec.max_clear_pitch(forward, drop, half_fov_y).max(0.0);
        if self.look_pitch > cap {
            self.look_pitch = cap;
        }
    }
}
