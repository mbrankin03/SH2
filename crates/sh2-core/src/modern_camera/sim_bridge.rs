//! Bridge methods and locomotion/flashlight coupling connecting [`ModernCameraRig`] to [`SimulationCore`] (`SH2-93`).

use glam::{Mat4, Vec3};

use crate::camera::CameraSnapshot;
use crate::config::ControlOptions;
use crate::cutaway::CutawayOverlayEngine;
use crate::cutscene::DramaDemoPlayer;
use crate::input::ControlScheme;
use crate::locomotion::{LocomotionInput, LocomotionOutput};
use crate::simulation::SimulationCore;
use crate::timing::lerp_angle_shortest;

use super::{
    beam_quat_from_direction, ExternalCameraHandoff, GameplayCameraMode, ModeTransitionState,
    ModernCameraTickInput, OtsReticleState, PlayerMeshVisibility,
};

/// Mouse motion conversion scalar in radians per raw pointer dot at `1.0x` sensitivity (`SH2-93`).
pub const MOUSE_LOOK_RAD_PER_DOT: f32 = 0.0022;

impl SimulationCore {
    /// Builds per-tick `(LocomotionInput, is_tank)` and pre-aligns `player_yaw` for `FirstPerson`
    /// and weapon-readied `OverTheShoulder` so strafe and backpedal never fight free-look (`SH2-93`).
    pub(crate) fn prepare_locomotion_for_camera(&mut self) -> (LocomotionInput, bool) {
        if self.input.state.camera_mode_cycle_pressed {
            self.modern_camera.cycle_mode_r3(
                &self.camera.curr_snapshot,
                self.scenario.character,
                self.scenario.player_position,
                self.scenario.player_yaw,
            );
        }

        let mode = self.modern_camera.active_mode;
        let body_locked_strafe = mode == GameplayCameraMode::FirstPerson
            || (mode == GameplayCameraMode::OverTheShoulder
                && self.input.state.ready_weapon_pressed);

        let lx = self.input.state.left_stick_x;
        let ly = self.input.state.left_stick_y;

        if body_locked_strafe
            && !self.locomotion.state.is_quick_turn()
            && !self.input.state.quick_turn_pressed
        {
            if mode == GameplayCameraMode::FirstPerson {
                let diag_offset = if ly > 0.21 && lx.abs() > 0.21 {
                    lx.atan2(ly)
                } else if ly < -0.21 && lx.abs() > 0.21 {
                    -lx.atan2(-ly)
                } else {
                    0.0
                };
                self.scenario.player_yaw = self.modern_camera.first_person.look_yaw + diag_offset;
            } else {
                self.scenario.player_yaw = lerp_angle_shortest(
                    self.scenario.player_yaw,
                    self.modern_camera.ots.orbit_yaw,
                    0.35,
                );
            }
        }

        if body_locked_strafe {
            let dominant_lateral = lx.abs() > 0.21 && lx.abs() > ly.abs();
            let stick_y = if dominant_lateral { 0.0 } else { ly };
            let strafe_left = if dominant_lateral {
                lx < 0.0
            } else {
                self.input.state.strafe_left
            };
            let strafe_right = if dominant_lateral {
                lx > 0.0
            } else {
                self.input.state.strafe_right
            };

            let input = LocomotionInput {
                stick_x: 0.0,
                stick_y,
                run_held: self.input.state.is_running,
                strafe_left,
                strafe_right,
                quick_turn_pressed: self.input.state.quick_turn_pressed,
                wall_ahead: false,
                enemy_near: false,
                health_fraction: self.scenario.health / 100.0,
                is_maria_follower: false,
                camera_yaw: self.scenario.player_yaw,
            };
            (input, true)
        } else {
            // In OverTheShoulder exploration, WASD / left stick always moves relative to `ots.orbit_yaw`
            // and refreshes `cam_locked` each tick so continuous mouse/stick steering curves smoothly.
            let is_tank = match mode {
                GameplayCameraMode::ClassicPs2Rail => {
                    self.input.config.scheme == ControlScheme::Tank
                }
                GameplayCameraMode::OverTheShoulder | GameplayCameraMode::FirstPerson => {
                    self.locomotion.cam_locked = false;
                    false
                }
            };
            let camera_yaw = match mode {
                GameplayCameraMode::ClassicPs2Rail => self
                    .camera
                    .curr_snapshot
                    .eye
                    .x
                    .atan2(self.camera.curr_snapshot.eye.z),
                GameplayCameraMode::OverTheShoulder => self.modern_camera.ots.orbit_yaw,
                GameplayCameraMode::FirstPerson => self.modern_camera.first_person.look_yaw,
            };
            let input = LocomotionInput {
                stick_x: lx,
                stick_y: ly,
                run_held: self.input.state.is_running,
                strafe_left: self.input.state.strafe_left,
                strafe_right: self.input.state.strafe_right,
                quick_turn_pressed: self.input.state.quick_turn_pressed,
                wall_ahead: false,
                enemy_near: false,
                health_fraction: self.scenario.health / 100.0,
                is_maria_follower: false,
                camera_yaw,
            };
            (input, is_tank)
        }
    }

    /// Steps [`super::ModernCameraRig`] after the PS2 rail tick and couples the flashlight beam (`SH2-93`).
    pub(crate) fn step_modern_camera_after_ps2_rail(&mut self, locomotion_out: &LocomotionOutput) {
        let mode = self.modern_camera.active_mode;
        let extra_yaw = self.input.state.mouse_look_delta_x + self.input.state.gyro_look_delta_yaw;
        let extra_pitch =
            self.input.state.mouse_look_delta_y + self.input.state.gyro_look_delta_pitch;
        let modern_in = ModernCameraTickInput {
            character: self.scenario.character,
            player_pos: self.scenario.player_position,
            player_yaw: self.scenario.player_yaw,
            linear_speed: locomotion_out.linear_speed,
            right_stick_x: self.input.state.right_stick_x,
            right_stick_y: self.input.state.right_stick_y,
            extra_yaw_delta_rad: extra_yaw,
            extra_pitch_delta_rad: extra_pitch,
            sensitivity: self.stick_sensitivity,
            ready_weapon: self.input.state.ready_weapon_pressed,
            quick_turn_pressed: self.input.state.quick_turn_pressed,
            shoulder_swap_pressed: self.input.state.shoulder_swap_pressed,
            in_bug_room: false,
        };
        let frame = self.modern_camera.tick(
            &modern_in,
            &self.camera.curr_snapshot,
            &self.collision,
            &self.timing,
        );
        if mode == GameplayCameraMode::FirstPerson && !self.locomotion.state.is_quick_turn() {
            self.scenario.player_yaw = self.modern_camera.first_person.look_yaw;
        }
        if mode.is_modern() {
            self.flashlight_sweep.curr_origin = frame.flashlight_origin;
            self.flashlight_sweep.curr_orientation =
                beam_quat_from_direction(frame.flashlight_dir, self.scenario.player_yaw);
        }
    }

    /// Synchronizes input sensitivity, control scheme, and modern camera settings from [`ControlOptions`] (`SH2-93`).
    pub fn apply_control_options(&mut self, opts: &ControlOptions) {
        self.input.config.scheme = if opts.tank_controls {
            ControlScheme::Tank
        } else {
            ControlScheme::AnalogDirectional
        };
        self.input.config.stick_deadzone = opts.stick_deadzone.clamp(0.0, 0.5);
        self.input.config.vibration_intensity = opts.vibration_intensity.clamp(0.0, 1.0);
        self.stick_sensitivity = opts.stick_sensitivity.clamp(0.2, 4.0);
        self.modern_camera.apply_control_options(
            opts,
            &self.camera.curr_snapshot,
            self.scenario.character,
            self.scenario.player_position,
            self.scenario.player_yaw,
        );
    }

    /// Returns the currently active gameplay camera mode (`SH2-93`).
    #[inline]
    pub fn active_camera_mode(&self) -> GameplayCameraMode {
        self.modern_camera.active_mode
    }

    /// Smoothly switches to `mode` (`ClassicPs2Rail`, `OverTheShoulder`, or `FirstPerson`) (`SH2-93`).
    pub fn set_camera_mode(&mut self, mode: GameplayCameraMode) {
        self.modern_camera.set_mode_smooth(
            mode,
            &self.camera.curr_snapshot,
            self.scenario.character,
            self.scenario.player_position,
            self.scenario.player_yaw,
        );
    }

    /// Supplies evaluated character model-space bone matrices (`SH2-8`/`SH2-12`) to anchor First-Person view (`SH2-93`).
    pub fn set_player_skeleton_pose(&mut self, pose_matrices: Option<&[[[f32; 4]; 4]]>) {
        self.modern_camera.first_person.update_from_skeleton_pose(
            self.scenario.character,
            self.scenario.player_position,
            self.scenario.player_yaw,
            pose_matrices,
        );
    }

    /// Returns the current foreground character mesh visibility directive (`FullBody` vs `HideHeadAndUpperBody`) (`SH2-93`).
    #[inline]
    pub fn player_mesh_visibility(&self) -> PlayerMeshVisibility {
        self.modern_camera.curr_frame.player_visibility
    }

    /// Synchronizes scripted `.dds` cutscene camera blending/lock (`SH2-63` / `SH2-93`).
    pub fn sync_cutscene_handoff(&mut self, demo_player: Option<&DramaDemoPlayer>) {
        self.modern_camera.sync_cutscene(demo_player);
    }

    /// Synchronizes 2D interactive puzzle/memo cutaway camera lock (`SH2-15` / `SH2-93`).
    pub fn sync_cutaway_handoff(&mut self, cutaway: &CutawayOverlayEngine) {
        self.modern_camera.sync_cutaway(cutaway);
    }

    /// Converts raw desktop mouse motion `(dx_dots, dy_dots)` into sensitivity-scaled yaw/pitch radians
    /// when an active modern camera (`OverTheShoulder` or `FirstPerson`) is unlocked (`SH2-93`).
    pub fn accumulate_mouse_motion_dots(&mut self, dx_dots: f32, dy_dots: f32) {
        if !self.modern_camera.active_mode.is_modern()
            || self.modern_camera.handoff.locks_camera_controls()
        {
            return;
        }
        let aim_scale = if self.input.state.ready_weapon_pressed {
            0.68
        } else {
            1.0
        };
        let rad_per_dot =
            MOUSE_LOOK_RAD_PER_DOT * self.stick_sensitivity.clamp(0.2, 4.0) * aim_scale;
        self.input
            .accumulate_mouse_look(dx_dots * rad_per_dot, dy_dots * rad_per_dot);
    }

    /// Resets both `Ps2CameraController` and `ModernCameraRig` immediately to `(pos, yaw)` on room cuts
    /// or debug teleports (`SH2-93`).
    pub fn reset_cameras_to_player(&mut self, pos: Vec3, yaw: f32) {
        self.camera.reset_to_player(pos, yaw);
        self.modern_camera.reset_to_player(
            self.scenario.character,
            pos,
            yaw,
            &self.camera.curr_snapshot,
        );
    }

    /// Snaps the player to `(pos, yaw)` at the end of a `.dds` cutscene while smoothly blending
    /// `ModernCameraRig` out of the final cutscene shot back into `active_mode` (`SH2-63` / `SH2-93`).
    pub fn handoff_player_after_cutscene(&mut self, pos: Vec3, yaw: f32) {
        self.snap_player_transform(pos, yaw);
        self.camera.reset_to_player(pos, yaw);
        self.modern_camera.ots.reset_to_player(pos, yaw);
        self.modern_camera
            .first_person
            .reset_to_player(self.scenario.character, pos, yaw);
        self.modern_camera.sync_cutscene(None);
    }

    /// Evaluates the active sub-frame [`CameraSnapshot`] across `ClassicPs2Rail`, `OverTheShoulder`,
    /// `FirstPerson`, crossfades, and external handoffs (`SH2-93`).
    #[inline]
    pub fn active_camera_snapshot(&self, alpha: f32) -> CameraSnapshot {
        if self.modern_camera.active_mode == GameplayCameraMode::ClassicPs2Rail
            && matches!(self.modern_camera.transition, ModeTransitionState::Settled)
            && matches!(
                self.modern_camera.handoff,
                ExternalCameraHandoff::GameplayActive
            )
        {
            let t = alpha.clamp(0.0, 1.0);
            let prev = &self.camera.prev_snapshot;
            let curr = &self.camera.curr_snapshot;
            CameraSnapshot {
                eye: prev.eye.lerp(curr.eye, t),
                target: prev.target.lerp(curr.target, t),
                up: prev
                    .up
                    .lerp(curr.up, t)
                    .normalize_or(Vec3::new(0.0, -1.0, 0.0)),
                fov_y: prev.fov_y + (curr.fov_y - prev.fov_y) * t,
            }
        } else {
            self.modern_camera.interpolated_snapshot(alpha)
        }
    }

    /// Evaluates the active sub-frame camera eye position in SH2 world units (`SH2-93`).
    #[inline]
    pub fn active_camera_eye(&self, alpha: f32) -> Vec3 {
        self.active_camera_snapshot(alpha).eye
    }

    /// Evaluates the active sub-frame view matrix across `ClassicPs2Rail`, `OverTheShoulder`, `FirstPerson`, and handoffs (`SH2-93`).
    #[inline]
    pub fn active_view_matrix(&self, alpha: f32) -> Mat4 {
        if self.modern_camera.active_mode == GameplayCameraMode::ClassicPs2Rail
            && matches!(self.modern_camera.transition, ModeTransitionState::Settled)
            && matches!(
                self.modern_camera.handoff,
                ExternalCameraHandoff::GameplayActive
            )
        {
            self.camera.interpolated_view_matrix(alpha)
        } else {
            self.modern_camera.interpolated_view_matrix(alpha)
        }
    }

    /// Evaluates the active sub-frame vertical FOV in radians (`SH2-93`).
    #[inline]
    pub fn active_fov_y(&self, alpha: f32) -> f32 {
        if self.modern_camera.active_mode == GameplayCameraMode::ClassicPs2Rail
            && matches!(self.modern_camera.transition, ModeTransitionState::Settled)
            && matches!(
                self.modern_camera.handoff,
                ExternalCameraHandoff::GameplayActive
            )
        {
            self.camera.interpolated_fov(alpha)
        } else {
            self.modern_camera.interpolated_fov(alpha)
        }
    }

    /// Evaluates the active sub-frame perspective near-clip distance in SH2 world units (`SH2-93`).
    #[inline]
    pub fn active_near_clip(&self, alpha: f32) -> f32 {
        self.modern_camera.interpolated_near_clip(alpha)
    }

    /// Returns the active ranged weapon aiming `(origin, unit_direction)` in SH2 world space (`SH2-93`).
    #[inline]
    pub fn active_aim_ray(&self) -> (Vec3, Vec3) {
        (
            self.modern_camera.curr_frame.aim_origin,
            self.modern_camera.curr_frame.aim_dir,
        )
    }

    /// Returns the active subtle aiming reticle state (`SH2-93`).
    #[inline]
    pub fn active_reticle_state(&self) -> OtsReticleState {
        self.modern_camera.curr_frame.reticle
    }
}

#[cfg(test)]
mod sim_bridge_tests {
    use glam::Vec3;

    use crate::config::ControlOptions;
    use crate::input::ControlScheme;
    use crate::modern_camera::{GameplayCameraMode, OtsShoulderSide, PlayerMeshVisibility};
    use crate::scenario::ScenarioKind;
    use crate::simulation::SimulationCore;

    #[test]
    fn test_sh2_93_simulation_r3_ots_fps_flashlight_and_visibility() {
        let mut sim = SimulationCore::new(ScenarioKind::LetterFromSilentHeaven);
        assert_eq!(sim.active_camera_mode(), GameplayCameraMode::ClassicPs2Rail);
        assert_eq!(sim.player_mesh_visibility(), PlayerMeshVisibility::FullBody);

        sim.input.press_camera_mode_cycle();
        sim.input.accumulate_mouse_look(0.25, -0.10);
        sim.input.state.ready_weapon_pressed = true;
        sim.tick();
        assert_eq!(
            sim.active_camera_mode(),
            GameplayCameraMode::OverTheShoulder
        );
        for _ in 0..20 {
            sim.tick();
        }
        let (_fl_orig, fl_dir) = sim.interpolated_flashlight_ray(1.0);
        let (_aim_orig, aim_dir) = sim.active_aim_ray();
        assert!((fl_dir - aim_dir).length() < 1e-3);
        assert!(sim.active_view_matrix(0.5).is_finite());

        sim.input.press_camera_mode_cycle();
        for _ in 0..22 {
            sim.tick();
        }
        assert_eq!(sim.active_camera_mode(), GameplayCameraMode::FirstPerson);
        assert_eq!(
            sim.player_mesh_visibility(),
            PlayerMeshVisibility::HideHeadAndUpperBody
        );
        assert!((sim.active_near_clip(1.0) - 8.0).abs() < 1e-3);

        sim.input.press_camera_mode_cycle();
        for _ in 0..22 {
            sim.tick();
        }
        assert_eq!(sim.active_camera_mode(), GameplayCameraMode::ClassicPs2Rail);
        assert_eq!(sim.player_mesh_visibility(), PlayerMeshVisibility::FullBody);
    }

    #[test]
    fn test_sh2_93_locomotion_directions_in_2d_ots_and_first_person() {
        let mut sim = SimulationCore::new(ScenarioKind::LetterFromSilentHeaven);
        sim.collision = crate::collision::CollisionWorld::new();
        sim.input
            .set_control_scheme(ControlScheme::AnalogDirectional);

        // 1. 2D OverTheShoulder at yaw = 0.0: +stick_y -> +Z, -stick_y -> -Z
        sim.snap_player_transform(Vec3::ZERO, 0.0);
        sim.modern_camera.active_mode = GameplayCameraMode::OverTheShoulder;
        sim.modern_camera.reset_to_player(
            sim.scenario.character,
            Vec3::ZERO,
            0.0,
            &sim.camera.curr_snapshot,
        );

        sim.input.state.left_stick_x = 0.0;
        sim.input.state.left_stick_y = 1.0;
        sim.tick();
        assert!(
            sim.last_locomotion.delta_position.z > 0.0,
            "2D OTS stick_y = +1.0 at yaw 0 must move +Z, got {:?}",
            sim.last_locomotion.delta_position
        );

        sim.snap_player_transform(Vec3::ZERO, std::f32::consts::PI);
        sim.modern_camera.ots.orbit_yaw = 0.0;
        sim.input.state.left_stick_y = -1.0;
        for _ in 0..5 {
            sim.tick();
        }
        assert!(
            sim.last_locomotion.delta_position.z < 0.0,
            "2D OTS stick_y = -1.0 at orbit_yaw 0 must move -Z, got {:?}",
            sim.last_locomotion.delta_position
        );

        // 2. FirstPerson at yaw = 0.0: +stick_y -> +Z, -stick_y -> -Z (backpedal), +stick_x -> +X (strafe), preserving player_yaw == 0.0
        sim.locomotion = crate::locomotion::PlayerLocomotionController::new();
        sim.snap_player_transform(Vec3::ZERO, 0.0);
        sim.modern_camera.active_mode = GameplayCameraMode::FirstPerson;
        sim.modern_camera.reset_to_player(
            sim.scenario.character,
            Vec3::ZERO,
            0.0,
            &sim.camera.curr_snapshot,
        );

        sim.input.state.left_stick_x = 0.0;
        sim.input.state.left_stick_y = 1.0;
        sim.tick();
        assert!(sim.last_locomotion.delta_position.z > 0.0);
        assert!(sim.scenario.player_yaw.abs() < 1e-4);

        sim.input.state.left_stick_x = 0.0;
        sim.input.state.left_stick_y = -1.0;
        for _ in 0..4 {
            sim.tick();
        }
        assert!(
            sim.last_locomotion.delta_position.z < 0.0,
            "FirstPerson stick_y = -1.0 must backpedal along -Z without fighting look_yaw, got {:?}",
            sim.last_locomotion.delta_position
        );
        assert!(sim.scenario.player_yaw.abs() < 1e-4);

        sim.input.state.left_stick_y = 0.0;
        sim.input.state.left_stick_x = 1.0;
        for _ in 0..4 {
            sim.tick();
        }
        assert!(
            sim.last_locomotion.delta_position.x > 0.0,
            "FirstPerson stick_x = +1.0 must strafe right (+X), got {:?}",
            sim.last_locomotion.delta_position
        );
        assert!(sim.scenario.player_yaw.abs() < 1e-4);
    }

    #[test]
    fn test_sh2_93_host_wasd_mouse_ijkl_and_options_across_all_three_modes() {
        let mut sim = SimulationCore::new(ScenarioKind::LetterFromSilentHeaven);
        sim.collision = crate::collision::CollisionWorld::new();
        // Default ControlOptions has `tank_controls = true`
        sim.apply_control_options(&ControlOptions::default());
        assert_eq!(sim.input.config.scheme, ControlScheme::Tank);

        // 1. ClassicPs2Rail + Tank: 'A' (`move_x = -1`) turns left; 'W' (`move_y = +1`) walks along player_yaw
        sim.snap_player_transform(Vec3::ZERO, 0.0);
        sim.reset_cameras_to_player(Vec3::ZERO, 0.0);
        sim.input
            .set_host_keyboard_state(-1.0, 0.0, 0.0, 0.0, false, false);
        sim.tick();
        assert!(sim.scenario.player_yaw < 0.0);

        // 2. Press 'V' -> OverTheShoulder: even with `tank_controls = true`, WASD moves relative to `ots.orbit_yaw`
        // and continuous mouse motion (`accumulate_mouse_motion_dots`) steers mid-walk!
        sim.snap_player_transform(Vec3::ZERO, 0.0);
        sim.input.press_camera_mode_cycle();
        sim.input
            .set_host_keyboard_state(0.0, 0.0, 0.0, 0.0, false, false);
        sim.tick();
        assert_eq!(
            sim.active_camera_mode(),
            GameplayCameraMode::OverTheShoulder
        );

        // Orbit OTS camera +90 deg (+X) via mouse motion dots while James still faces +Z (0.0)
        sim.accumulate_mouse_motion_dots(
            std::f32::consts::FRAC_PI_2 / super::MOUSE_LOOK_RAD_PER_DOT,
            0.0,
        );
        sim.tick();
        assert!((sim.modern_camera.ots.orbit_yaw - std::f32::consts::FRAC_PI_2).abs() < 1e-2);

        // Hold 'W' (`move_y = +1.0`): James must turn and walk toward +X (OTS look direction)!
        sim.input
            .set_host_keyboard_state(0.0, 1.0, 0.0, 0.0, true, false);
        for _ in 0..12 {
            sim.tick();
        }
        assert!(
            sim.scenario.player_position.x > 50.0,
            "OTS 'W' at orbit_yaw=+PI/2 must move +X, got {:?}",
            sim.scenario.player_position
        );

        // Press 'X' (`press_shoulder_swap`) -> Left Shoulder
        sim.input.press_shoulder_swap();
        sim.tick();
        assert_eq!(sim.modern_camera.ots.shoulder_side, OtsShoulderSide::Left);

        // 3. Press 'V' -> FirstPerson (inherits `ots.orbit_yaw == +PI/2` East!):
        //    At look_yaw = +PI/2, diagonal W+D (`+3*PI/4`) moves (+X, -Z), while IJKL (`look_x=1, look_y=0.5`) pans right & pitches up!
        sim.snap_player_transform(Vec3::ZERO, 0.0);
        sim.input.press_camera_mode_cycle();
        sim.input
            .set_host_keyboard_state(1.0, 1.0, 1.0, 0.5, false, false);
        for _ in 0..8 {
            sim.tick();
        }
        assert_eq!(sim.active_camera_mode(), GameplayCameraMode::FirstPerson);
        assert!(
            sim.scenario.player_position.x > 5.0 && sim.scenario.player_position.z < -5.0,
            "FPS W+D from inherited orbit_yaw=+PI/2 must move (+X, -Z), got {:?}",
            sim.scenario.player_position
        );
        assert!(sim.modern_camera.first_person.look_yaw > std::f32::consts::FRAC_PI_2 + 0.05);
        assert!(sim.modern_camera.first_person.look_pitch < 0.0);
    }
}
