//! Toggleable Over-the-Shoulder (OTS) 3D & First-Person (FPS) camera subsystem (`SH2-93`).
//!
//! Operates alongside the untouched `Ps2CameraController` (`camera.rs`), providing:
//! - [`GameplayCameraMode`]: `ClassicPs2Rail`, `OverTheShoulder`, and `FirstPerson`.
//! - [`OtsCameraRig`] (`ots.rs`): Damped right/left-shoulder 3D orbit with look-coupled flashlight & weapon aiming.
//! - [`FirstPersonCameraRig`] (`first_person.rs`): Head-bone (`SH2-8`/`SH2-12`) eye anchoring, clamped pitch,
//!   configurable FOV (`55..=100 deg`), tuned near-clip (`8.0` units), subtle toggleable sway, and [`PlayerMeshVisibility`].
//! - [`SpringArmConfig`] (`spring_arm.rs`): `.cld` wall/ceiling/floor raycast boom & eye bounds containment.
//! - [`ExternalCameraHandoff`] (`handoff.rs`): Smooth `R3` cross-mode blending and `.dds` cutscene / 2D cutaway handoffs.

pub mod first_person;
pub mod frame;
pub mod handoff;
pub mod ots;
pub mod spring_arm;
#[cfg(test)]
mod tests;

use glam::{Mat4, Vec3};

use crate::camera::CameraSnapshot;
use crate::collision::CollisionWorld;
use crate::config::ControlOptions;
use crate::cutaway::CutawayOverlayEngine;
use crate::cutscene::DramaDemoPlayer;
use crate::scenario::PlayableCharacter;
use crate::timing::TimingConstants;

pub use first_person::{
    CharacterHeadRigSpec, FirstPersonCameraRig, FirstPersonConfig, HeadBobMode,
    PlayerMeshVisibility, FIRST_PERSON_NEAR_CLIP_UNITS, STANDARD_NEAR_CLIP_UNITS,
};
pub use frame::{ModernCameraFrame, ModernCameraTickInput};
pub use handoff::{
    blend_snapshots, smoothstep01, snapshot_from_cutscene, CameraCyclePreset,
    ExternalCameraHandoff, GameplayCameraMode, ModeTransitionState,
};
pub use ots::{
    beam_quat_from_direction, OtsAimPhase, OtsCameraConfig, OtsCameraRig, OtsReticleState,
    OtsShoulderSide, ReticleMode,
};
pub use spring_arm::{
    constrain_eye_in_room, point_to_wall_distance_xz, sample_local_ceiling_y, solve_ots_spring_arm,
    sweep_ray_against_cld, OtsBoomQuery, SpringArmConfig, SpringArmState,
};

/// Unified stateful controller orchestrating `ClassicPs2Rail`, `OverTheShoulder`, and `FirstPerson` (`SH2-93`).
#[derive(Debug, Clone)]
pub struct ModernCameraRig {
    /// Selected gameplay camera mode (`ClassicPs2Rail`, `OverTheShoulder`, or `FirstPerson`).
    pub active_mode: GameplayCameraMode,
    /// `R3` live camera cycle subset preset.
    pub cycle_preset: CameraCyclePreset,
    /// Cross-mode smoothstep blend state.
    pub transition: ModeTransitionState,
    /// Scripted `.dds` cutscene & 2D puzzle cutaway handoff state machine.
    pub handoff: ExternalCameraHandoff,
    /// Over-the-Shoulder 3D camera rig.
    pub ots: OtsCameraRig,
    /// Head-bone First-Person camera rig.
    pub first_person: FirstPersonCameraRig,
    /// `.cld` spring-arm and room bounds collision tuning.
    pub spring_config: SpringArmConfig,
    /// Cross-mode transition duration in 60 Hz ticks (`18` = `0.30 s`).
    pub mode_blend_ticks: u16,
    /// Cutscene / cutaway handoff blend duration in 60 Hz ticks (`20` = `0.33 s`).
    pub handoff_blend_ticks: u16,
    /// Bug-room flashlight oscillation accumulator in radians.
    pub bug_room_phase: f32,
    /// Previous 60 Hz tick output frame for 120–240 Hz sub-frame interpolation.
    pub prev_frame: ModernCameraFrame,
    /// Current 60 Hz tick output frame.
    pub curr_frame: ModernCameraFrame,
}

impl Default for ModernCameraRig {
    fn default() -> Self {
        Self::new()
    }
}

impl ModernCameraRig {
    /// Creates a new modern camera orchestrator defaulting to `ClassicPs2Rail`.
    pub fn new() -> Self {
        let initial = ModernCameraFrame::default();
        Self {
            active_mode: GameplayCameraMode::ClassicPs2Rail,
            cycle_preset: CameraCyclePreset::AllThree,
            transition: ModeTransitionState::Settled,
            handoff: ExternalCameraHandoff::GameplayActive,
            ots: OtsCameraRig::default(),
            first_person: FirstPersonCameraRig::default(),
            spring_config: SpringArmConfig::default(),
            mode_blend_ticks: 18,
            handoff_blend_ticks: 20,
            bug_room_phase: 0.0,
            prev_frame: initial,
            curr_frame: initial,
        }
    }

    /// Synchronizes camera mode, `R3` preset, shoulder side, reticle, FPS FOV, and sway from [`ControlOptions`].
    pub fn apply_control_options(
        &mut self,
        opts: &ControlOptions,
        rail_snapshot: &CameraSnapshot,
        character: PlayableCharacter,
        player_pos: Vec3,
        player_yaw: f32,
    ) {
        self.cycle_preset = opts.camera_cycle_preset;
        self.ots.shoulder_side = opts.ots_shoulder;
        self.ots.reticle_mode = opts.ots_reticle;
        self.first_person.reticle_mode = opts.ots_reticle;
        self.first_person.config.fov_y_degrees =
            (opts.first_person_fov_deg as f32).clamp(55.0, 100.0);
        self.first_person.head_bob_mode = HeadBobMode::from_enabled(opts.first_person_head_bob);
        if opts.default_camera_mode != self.active_mode {
            self.set_mode_smooth(
                opts.default_camera_mode,
                rail_snapshot,
                character,
                player_pos,
                player_yaw,
            );
        }
    }

    /// Smoothly transitions to `target_mode`, seeding OTS/FPS look yaw from the outgoing pose.
    pub fn set_mode_smooth(
        &mut self,
        target_mode: GameplayCameraMode,
        rail_snapshot: &CameraSnapshot,
        character: PlayableCharacter,
        player_pos: Vec3,
        player_yaw: f32,
    ) {
        if target_mode == self.active_mode {
            return;
        }
        let outgoing = if self.active_mode == GameplayCameraMode::ClassicPs2Rail
            && matches!(self.transition, ModeTransitionState::Settled)
        {
            *rail_snapshot
        } else {
            self.curr_frame.snapshot
        };
        let from_near = self.curr_frame.near_clip;

        match target_mode {
            GameplayCameraMode::OverTheShoulder => {
                self.ots.reset_to_player(player_pos, player_yaw);
            }
            GameplayCameraMode::FirstPerson => {
                let seed_yaw = if self.active_mode == GameplayCameraMode::OverTheShoulder {
                    self.ots.orbit_yaw
                } else {
                    player_yaw
                };
                self.first_person
                    .reset_to_player(character, player_pos, seed_yaw);
            }
            GameplayCameraMode::ClassicPs2Rail => {}
        }

        self.active_mode = target_mode;
        self.transition = ModeTransitionState::Crossfading {
            from_snapshot: outgoing,
            from_near_clip: from_near,
            elapsed_ticks: 0,
            duration_ticks: self.mode_blend_ticks.max(1),
        };
    }

    /// Cycles the gameplay camera mode via `R3` unless locked inside a cutscene or 2D puzzle cutaway.
    pub fn cycle_mode_r3(
        &mut self,
        rail_snapshot: &CameraSnapshot,
        character: PlayableCharacter,
        player_pos: Vec3,
        player_yaw: f32,
    ) -> GameplayCameraMode {
        if self.handoff.locks_camera_controls() {
            return self.active_mode;
        }
        let next = self.active_mode.cycle_with_preset(self.cycle_preset);
        self.set_mode_smooth(next, rail_snapshot, character, player_pos, player_yaw);
        self.active_mode
    }

    /// Immediately snaps all camera rigs to `(player_pos, player_yaw)` (used on stage/room cuts).
    pub fn reset_to_player(
        &mut self,
        character: PlayableCharacter,
        player_pos: Vec3,
        player_yaw: f32,
        rail_snapshot: &CameraSnapshot,
    ) {
        self.ots.reset_to_player(player_pos, player_yaw);
        self.first_person
            .reset_to_player(character, player_pos, player_yaw);
        self.transition = ModeTransitionState::Settled;
        self.handoff = ExternalCameraHandoff::GameplayActive;
        let (sy, cy) = player_yaw.sin_cos();
        let fwd = Vec3::new(sy, 0.0, cy);
        let snap = match self.active_mode {
            GameplayCameraMode::ClassicPs2Rail => *rail_snapshot,
            GameplayCameraMode::OverTheShoulder => CameraSnapshot {
                eye: player_pos + Vec3::new(0.0, self.ots.config.shoulder_height_y, 0.0)
                    - fwd * self.ots.config.hip_boom_distance,
                target: player_pos
                    + Vec3::new(0.0, self.ots.config.shoulder_height_y, 0.0)
                    + fwd * 1000.0,
                up: Vec3::new(0.0, -1.0, 0.0),
                fov_y: self.ots.config.hip_fov_y_radians,
            },
            GameplayCameraMode::FirstPerson => CameraSnapshot {
                eye: self.first_person.smoothed_eye,
                target: self.first_person.smoothed_eye + fwd * 1000.0,
                up: Vec3::new(0.0, -1.0, 0.0),
                fov_y: self.first_person.config.fov_y_degrees.to_radians(),
            },
        };
        let frame = ModernCameraFrame {
            active_mode: self.active_mode,
            snapshot: snap,
            near_clip: if self.active_mode == GameplayCameraMode::FirstPerson {
                self.first_person.config.near_clip_units
            } else {
                STANDARD_NEAR_CLIP_UNITS
            },
            player_visibility: if self.active_mode == GameplayCameraMode::FirstPerson {
                PlayerMeshVisibility::HideHeadAndUpperBody
            } else {
                PlayerMeshVisibility::FullBody
            },
            reticle: OtsReticleState::Hidden,
            aim_origin: player_pos + Vec3::new(0.0, -700.0, 0.0),
            aim_dir: fwd,
            flashlight_origin: player_pos + Vec3::new(sy * 45.0, -700.0, cy * 45.0),
            flashlight_dir: fwd,
        };
        self.prev_frame = frame;
        self.curr_frame = frame;
    }

    /// Updates scripted `.dds` cutscene camera handoff (`SH2-63`) and restores `active_mode` on completion.
    pub fn sync_cutscene(&mut self, demo_player: Option<&DramaDemoPlayer>) {
        self.handoff.sync_cutscene(
            demo_player,
            self.curr_frame.snapshot,
            self.handoff_blend_ticks,
        );
    }

    /// Updates 2D puzzle/memo cutaway handoff (`SH2-15`) and smoothly blends back on overlay close.
    pub fn sync_cutaway(&mut self, cutaway: &CutawayOverlayEngine) {
        self.handoff
            .sync_cutaway(cutaway, self.curr_frame.snapshot, self.handoff_blend_ticks);
    }

    /// Evaluates one deterministic 60 Hz tick across the active camera mode, `.cld` collision, and handoffs.
    pub fn tick(
        &mut self,
        input: &ModernCameraTickInput,
        rail_snapshot: &CameraSnapshot,
        collision: &CollisionWorld,
        timing: &TimingConstants,
    ) -> ModernCameraFrame {
        self.prev_frame = self.curr_frame;
        let locked = self.handoff.locks_camera_controls();
        let rx = if locked { 0.0 } else { input.right_stick_x };
        let ry = if locked { 0.0 } else { input.right_stick_y };
        let dyaw = if locked {
            0.0
        } else {
            input.extra_yaw_delta_rad
        };
        let dpitch = if locked {
            0.0
        } else {
            input.extra_pitch_delta_rad
        };

        let (mut base_snap, mut near_clip, aim_origin, aim_dir, mut reticle) =
            match self.active_mode {
                GameplayCameraMode::ClassicPs2Rail => {
                    let (sy, cy) = input.player_yaw.sin_cos();
                    let fwd = Vec3::new(sy, 0.0, cy);
                    let chest = input.player_pos + Vec3::new(sy * 45.0, -700.0, cy * 45.0);
                    (
                        *rail_snapshot,
                        STANDARD_NEAR_CLIP_UNITS,
                        chest,
                        fwd,
                        OtsReticleState::Hidden,
                    )
                }
                GameplayCameraMode::OverTheShoulder => {
                    let (snap, orig, dir, ret) = self.ots.tick(
                        input.player_pos,
                        input.player_yaw,
                        input.linear_speed,
                        rx,
                        ry,
                        dyaw,
                        dpitch,
                        input.sensitivity,
                        input.ready_weapon && !locked,
                        input.quick_turn_pressed && !locked,
                        input.shoulder_swap_pressed && !locked,
                        collision,
                        &self.spring_config,
                    );
                    (snap, STANDARD_NEAR_CLIP_UNITS, orig, dir, ret)
                }
                GameplayCameraMode::FirstPerson => {
                    let (snap, orig, dir, ret) = self.first_person.tick(
                        input.character,
                        input.player_pos,
                        input.player_yaw,
                        input.linear_speed,
                        rx,
                        ry,
                        dyaw,
                        dpitch,
                        input.sensitivity,
                        input.ready_weapon && !locked,
                        input.quick_turn_pressed && !locked,
                        collision,
                        &self.spring_config,
                    );
                    (
                        snap,
                        self.first_person.config.near_clip_units,
                        orig,
                        dir,
                        ret,
                    )
                }
            };

        self.transition.step(&mut base_snap, &mut near_clip);

        let (final_snap, suppress_hud_and_near, snap_prev_cut) = self.handoff.step(base_snap);
        if suppress_hud_and_near {
            reticle = OtsReticleState::Hidden;
            if matches!(
                self.handoff,
                ExternalCameraHandoff::BlendingIntoCutscene { .. }
                    | ExternalCameraHandoff::CutsceneLocked { .. }
            ) {
                near_clip = STANDARD_NEAR_CLIP_UNITS;
            }
        }

        let head_anchor = CharacterHeadRigSpec::for_character(input.character)
            .resolve_world_eye_anchor(input.player_pos, input.player_yaw, None);
        let player_visibility = if matches!(
            self.handoff,
            ExternalCameraHandoff::BlendingIntoCutscene { .. }
                | ExternalCameraHandoff::CutsceneLocked { .. }
        ) {
            PlayerMeshVisibility::FullBody
        } else if self.active_mode == GameplayCameraMode::FirstPerson
            && final_snap.eye.distance(head_anchor) < 180.0
        {
            PlayerMeshVisibility::HideHeadAndUpperBody
        } else {
            PlayerMeshVisibility::FullBody
        };

        let mut beam_dir = aim_dir;
        if input.in_bug_room {
            self.bug_room_phase =
                (self.bug_room_phase + timing.bug_room_flashlight_rate) % std::f32::consts::TAU;
            beam_dir = (beam_dir + Vec3::new(0.0, self.bug_room_phase.sin() * 0.035, 0.0))
                .normalize_or(aim_dir);
        }
        let (sy, cy) = input.player_yaw.sin_cos();
        let chest_pocket = input.player_pos + Vec3::new(sy * 45.0, -700.0, cy * 45.0);
        let flashlight_origin = match self.active_mode {
            GameplayCameraMode::FirstPerson => final_snap.eye.lerp(chest_pocket, 0.35),
            _ => chest_pocket,
        };

        let frame = ModernCameraFrame {
            active_mode: self.active_mode,
            snapshot: final_snap,
            near_clip,
            player_visibility,
            reticle,
            aim_origin,
            aim_dir,
            flashlight_origin,
            flashlight_dir: beam_dir,
        };
        self.curr_frame = frame;
        if snap_prev_cut {
            self.prev_frame = frame;
        }
        frame
    }

    /// Sub-frame `alpha` (`0.0..=1.0`) interpolated [`CameraSnapshot`] for 120–240 Hz+ rendering.
    #[inline]
    pub fn interpolated_snapshot(&self, alpha: f32) -> CameraSnapshot {
        self.curr_frame
            .interpolate_snapshot_from(&self.prev_frame, alpha)
    }

    /// Sub-frame `alpha` (`0.0..=1.0`) right-handed view matrix (`Mat4::look_at_rh`).
    #[inline]
    pub fn interpolated_view_matrix(&self, alpha: f32) -> Mat4 {
        self.curr_frame
            .interpolate_view_from(&self.prev_frame, alpha)
    }

    /// Sub-frame `alpha` (`0.0..=1.0`) vertical field-of-view in radians.
    #[inline]
    pub fn interpolated_fov(&self, alpha: f32) -> f32 {
        let t = alpha.clamp(0.0, 1.0);
        self.prev_frame.snapshot.fov_y
            + (self.curr_frame.snapshot.fov_y - self.prev_frame.snapshot.fov_y) * t
    }

    /// Sub-frame `alpha` (`0.0..=1.0`) perspective near-clip plane in SH2 world units.
    #[inline]
    pub fn interpolated_near_clip(&self, alpha: f32) -> f32 {
        let t = alpha.clamp(0.0, 1.0);
        self.prev_frame.near_clip + (self.curr_frame.near_clip - self.prev_frame.near_clip) * t
    }
}
