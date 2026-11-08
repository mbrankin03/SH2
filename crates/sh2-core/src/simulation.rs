//! Simulation engine orchestrating fixed 60 Hz ticks, timing constants, camera, scenario, and input.

use glam::Vec3;
use std::time::Duration;

use crate::animation::{
    AnimationStateKind, CharacterAnimationController, CharacterLocomotionClips,
    LocomotionAnimationSync,
};
use crate::camera::Ps2CameraController;
use crate::clock::EngineClock;
use crate::collision::{CollisionConfig, CollisionWorld};
use crate::input::{ControlScheme, HapticFeedback, InputManager};
use crate::locomotion::{
    LocomotionInput, LocomotionOutput, LocomotionState, PlayerLocomotionController,
};
use crate::scenario::{ScenarioKind, ScenarioState};
use crate::timing::{
    lerp_angle_shortest, FlashlightSweepState, RowboatController, TimingConstants,
};

/// Unified Silent Hill 2 simulation core.
pub struct SimulationCore {
    /// Fixed-step decoupled simulation clock.
    pub clock: EngineClock,
    /// Authentic PS2 camera controller.
    pub camera: Ps2CameraController,
    /// Active scenario state (James / Maria).
    pub scenario: ScenarioState,
    /// Previous 60 Hz tick player world position for sub-frame rendering interpolation.
    pub prev_player_position: Vec3,
    /// Previous 60 Hz tick player yaw angle in radians for sub-frame rendering interpolation.
    pub prev_player_yaw: f32,
    /// Deterministic 60 Hz pocket flashlight sweep and toggle state.
    pub flashlight_sweep: FlashlightSweepState,
    /// Deterministic 60 Hz Toluca Lake rowboat state.
    pub rowboat: RowboatController,
    /// Controller and gamepad manager.
    pub input: InputManager,
    /// Collision detection and response environment.
    pub collision: CollisionWorld,
    /// Collision kinematics tuning parameters.
    pub collision_config: CollisionConfig,
    /// Authentic 1:1 PS2/PC player locomotion and turning controller.
    pub locomotion: PlayerLocomotionController,
    /// Character skeletal animation controller.
    pub animation: CharacterAnimationController,
    /// Active character locomotion clips.
    pub locomotion_clips: CharacterLocomotionClips,
    /// 60 FPS timing constants.
    pub timing: TimingConstants,
    /// Cumulative simulation ticks elapsed.
    pub tick_counter: u64,
    /// Last tick's locomotion output.
    pub last_locomotion: LocomotionOutput,
    /// When true, locomotion state changes do not overwrite the active animation clip.
    pub animation_locked: bool,
}

impl Default for SimulationCore {
    fn default() -> Self {
        Self::new(ScenarioKind::LetterFromSilentHeaven)
    }
}

impl SimulationCore {
    /// Create a new simulation core configured for 60 Hz fixed logic rate.
    pub fn new(scenario_kind: ScenarioKind) -> Self {
        let scenario = ScenarioState::new(scenario_kind);
        let init_pos = scenario.player_position;
        let init_yaw = scenario.player_yaw;
        let mut camera = Ps2CameraController::new();
        camera.reset_to_player(init_pos, init_yaw);

        let locomotion_clips = match scenario.character {
            crate::scenario::PlayableCharacter::James => CharacterLocomotionClips::JAMES,
            crate::scenario::PlayableCharacter::Maria => CharacterLocomotionClips::MARIA,
        };

        Self {
            clock: EngineClock::new(60),
            camera,
            scenario,
            prev_player_position: init_pos,
            prev_player_yaw: init_yaw,
            flashlight_sweep: FlashlightSweepState::new(init_pos, init_yaw),
            rowboat: RowboatController::new(init_pos, init_yaw),
            input: InputManager::new(Default::default()),
            collision: CollisionWorld::new(),
            collision_config: CollisionConfig::default(),
            locomotion: PlayerLocomotionController::new(),
            animation: CharacterAnimationController::new(
                AnimationStateKind::Idle,
                locomotion_clips.idle,
            ),
            locomotion_clips,
            timing: TimingConstants::default(),
            tick_counter: 0,
            last_locomotion: LocomotionOutput {
                delta_position: Vec3::ZERO,
                turn_speed_rad_per_sec: 0.0,
                state: LocomotionState::Stand,
                linear_speed: 0.0,
                footstep_triggered: false,
                footstep_volume: 0.5,
            },
            animation_locked: false,
        }
    }

    /// Set or update the active character's locomotion clip configuration.
    pub fn set_locomotion_clips(&mut self, clips: CharacterLocomotionClips) {
        self.locomotion_clips = clips;
        self.animation
            .transition_to(AnimationStateKind::Idle, clips.idle);
    }

    /// Change or restart game scenario.
    pub fn select_scenario(&mut self, scenario_kind: ScenarioKind) {
        self.scenario = ScenarioState::new(scenario_kind);
        self.snap_player_transform(self.scenario.player_position, self.scenario.player_yaw);
        self.camera
            .reset_to_player(self.scenario.player_position, self.scenario.player_yaw);
    }

    /// Immediately teleports the player to `position` and `yaw`, resetting previous-tick
    /// snapshots and flashlight sweep state so sub-frame interpolation does not smear across room cuts.
    pub fn snap_player_transform(&mut self, position: Vec3, yaw: f32) {
        self.scenario.player_position = position;
        self.scenario.player_yaw = yaw;
        self.prev_player_position = position;
        self.prev_player_yaw = yaw;
        self.flashlight_sweep.reset(position, yaw);
    }

    /// Advance real delta time and consume all ready 60 Hz simulation ticks.
    ///
    /// Returns the number of fixed ticks processed.
    pub fn update(&mut self, delta_time: Duration) -> u32 {
        self.clock.advance(delta_time);
        let mut ticks_processed = 0;

        while self.clock.step() {
            self.tick();
            ticks_processed += 1;
        }

        ticks_processed
    }

    /// Execute a single fixed 60 Hz simulation tick.
    pub fn tick(&mut self) {
        self.tick_counter += 1;
        self.prev_player_position = self.scenario.player_position;
        self.prev_player_yaw = self.scenario.player_yaw;

        // 1. Poll and update gamepad state
        self.input.update();

        // 2. Handle flashlight toggle
        if self.input.state.light_toggle_pressed {
            self.scenario.flashlight_on = !self.scenario.flashlight_on;
        }

        // 3. Update character position and turning via authentic PS2 locomotion controller
        let dt = 1.0 / 60.0;
        let is_tank = self.input.config.scheme == ControlScheme::Tank;

        let locomotion_in = LocomotionInput {
            stick_x: self.input.state.left_stick_x,
            stick_y: self.input.state.left_stick_y,
            run_held: self.input.state.is_running,
            strafe_left: self.input.state.strafe_left,
            strafe_right: self.input.state.strafe_right,
            quick_turn_pressed: self.input.state.quick_turn_pressed,
            wall_ahead: false, // In free roam / room boundaries
            enemy_near: false,
            health_fraction: self.scenario.health / 100.0,
            is_maria_follower: false, // Active player 1 always has 1.0x speed, even in Born From a Wish (Maria)
            camera_yaw: self
                .camera
                .curr_snapshot
                .eye
                .x
                .atan2(self.camera.curr_snapshot.eye.z),
        };

        let out =
            self.locomotion
                .update(&locomotion_in, &mut self.scenario.player_yaw, dt, is_tank);

        // Apply locomotion delta with collision wall pushback and floor sampling
        let desired_pos = self.scenario.player_position + out.delta_position;
        self.scenario.player_position = self.collision.resolve_character_movement(
            self.scenario.player_position,
            desired_pos,
            &self.collision_config,
        );
        self.last_locomotion = out;

        // 3b. Synchronize character animation playback rate and gait phase with locomotion output (SH2-26 / 0x531a00)
        if !self.animation_locked {
            let cmd = LocomotionAnimationSync::resolve(
                self.animation.state(),
                &out,
                &self.locomotion_clips,
            );
            self.animation.apply_locomotion_command(cmd);
        } else {
            self.animation.set_playback_rate_scale(1.0);
        }

        // Fixed tick dt is 1/60s (approx 16.666 ms)
        self.animation.update(Duration::from_nanos(16_666_667));

        // 3c. Step pocket flashlight sweep at 60 Hz
        self.flashlight_sweep.tick(
            self.scenario.player_position,
            self.scenario.player_yaw,
            false,
            &self.timing,
        );

        // 4. Update authentic PS2 camera with raycast wall clamping (clCheckHitEyes)
        self.camera.tick(
            self.scenario.player_position,
            self.scenario.player_yaw,
            self.input.state.right_stick_x,
            self.input.state.right_stick_y,
        );

        let eye_target = self.scenario.player_position + Vec3::new(0.0, -1200.0, 0.0);
        let filter = crate::collision::RaycastFilter::default();
        self.camera.curr_snapshot.eye =
            self.collision
                .clamp_camera(eye_target, self.camera.curr_snapshot.eye, &filter);

        // 5. DualSense health heartbeat haptics
        if self.scenario.health <= 25.0 {
            // Pulsate heartbeat through DualSense voice-coil haptics every 40 ticks (~0.66s)
            if self.tick_counter.is_multiple_of(40) {
                self.input.trigger_haptics(HapticFeedback {
                    heavy_rumble: 0.2,
                    light_rumble: 0.0,
                    heartbeat_pulse: 1.0,
                    duration_ticks: 12,
                });
            }
        }
    }

    /// Sub-frame alpha factor (0.0 .. 1.0) for smooth rendering interpolation.
    #[inline]
    pub fn interpolation_alpha(&self) -> f32 {
        self.clock.alpha()
    }

    /// Evaluates the sub-frame interpolated player `(position, yaw)` between the previous and
    /// current 60 Hz simulation ticks for 120–240 Hz+ rendering.
    #[inline]
    pub fn interpolated_player_pose(&self, alpha: f32) -> (Vec3, f32) {
        let t = alpha.clamp(0.0, 1.0);
        let pos = self
            .prev_player_position
            .lerp(self.scenario.player_position, t);
        let yaw = lerp_angle_shortest(self.prev_player_yaw, self.scenario.player_yaw, t);
        (pos, yaw)
    }

    /// Evaluates the sub-frame Slerp-interpolated pocket flashlight `(origin, unit_direction)`.
    #[inline]
    pub fn interpolated_flashlight_ray(&self, alpha: f32) -> (Vec3, Vec3) {
        self.flashlight_sweep.interpolated_ray(alpha)
    }

    /// Evaluates deterministic fog flow time in seconds from elapsed 60 Hz ticks and sub-frame `alpha`.
    #[inline]
    pub fn deterministic_fog_time(&self, alpha: f32) -> f32 {
        self.timing.deterministic_fog_time(self.tick_counter, alpha)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simulation_core_dual_scenarios() {
        let mut sim = SimulationCore::new(ScenarioKind::LetterFromSilentHeaven);
        assert_eq!(
            sim.scenario.character,
            crate::scenario::PlayableCharacter::James
        );

        // Advance 1/30 second -> 2 ticks at 60 Hz
        let ticks = sim.update(Duration::from_millis(34));
        assert!(ticks >= 2);
        assert_eq!(sim.tick_counter, ticks as u64);

        // Switch to Maria's Born from a Wish
        sim.select_scenario(ScenarioKind::BornFromAWish);
        assert_eq!(
            sim.scenario.character,
            crate::scenario::PlayableCharacter::Maria
        );
        assert_eq!(sim.scenario.current_stage, 53);
    }

    #[test]
    fn test_sh2_26_simulation_locomotion_animation_sync() {
        let mut sim = SimulationCore::new(ScenarioKind::LetterFromSilentHeaven);

        // 1. Half analog stick tilt walking (`stick_y = 0.595` -> after 0x52e110 deadzone filter `/ 0.79`, ~0.75 tilt)
        sim.input.state.left_stick_y = 0.395; // ~0.5 tilt after `/ 0.79`
        sim.tick();
        assert_eq!(sim.animation.state(), AnimationStateKind::Walk);
        let half_rate = sim.animation.effective_fps();

        // 2. Full analog stick tilt walking (`stick_y = 1.0` -> `1.0` tilt, `750.0` units/s)
        // PS2 fixed-point walk speed: 512 + 896 * ty (960 @ ty=0.5 -> 14.0625 fps; 1408 @ ty=1.0 -> 20.625 fps)
        sim.input.state.left_stick_y = 1.0;
        sim.tick();
        assert_eq!(sim.animation.state(), AnimationStateKind::Walk);
        let full_rate = sim.animation.effective_fps();
        assert!((half_rate - 14.0625).abs() < 1e-2);
        assert!((full_rate - 20.625).abs() < 1e-3);

        // Step 15 ticks in Walk to build up non-zero stride phase
        for _ in 0..15 {
            sim.tick();
        }
        let walk_rel_phase = sim.animation.relative_frame_offset();
        assert!(walk_rel_phase > 2.0);

        // 3. Engage Run (`is_running = true`) -> transitions to Run 1 with Mode 10 gait phase sync
        sim.input.state.is_running = true;
        sim.tick();
        assert_eq!(sim.animation.state(), AnimationStateKind::Run);
        assert!(sim.animation.is_gait_phase_synced_blend());
        assert!((sim.animation.effective_fps() - 22.5).abs() < 1e-3);
        // Relative frame offset advances smoothly from `walk_rel_phase` rather than resetting to 0.0
        assert!(sim.animation.relative_frame_offset() > walk_rel_phase);

        // 4. Accelerate through Run 1 -> Run 2 (26.25 fps) -> Run 3 (30.0 fps)
        for _ in 0..90 {
            sim.tick();
        }
        assert!(matches!(
            sim.animation.state(),
            AnimationStateKind::Run2 | AnimationStateKind::Run3
        ));
        if sim.animation.state() == AnimationStateKind::Run3 {
            assert!((sim.animation.effective_fps() - 30.0).abs() < 1e-3);
        }
    }
}
