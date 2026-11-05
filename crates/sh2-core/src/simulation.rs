//! Simulation engine orchestrating fixed 60 Hz ticks, timing constants, camera, scenario, and input.

use glam::Vec3;
use std::time::Duration;

use crate::animation::{
    AnimationStateKind, CharacterAnimationController, CharacterLocomotionClips,
};
use crate::camera::Ps2CameraController;
use crate::clock::EngineClock;
use crate::input::{ControlScheme, HapticFeedback, InputManager};
use crate::locomotion::{
    LocomotionInput, LocomotionOutput, LocomotionState, PlayerLocomotionController,
};
use crate::scenario::{ScenarioKind, ScenarioState};
use crate::timing::TimingConstants;

/// Unified Silent Hill 2 simulation core.
pub struct SimulationCore {
    /// Fixed-step decoupled simulation clock.
    pub clock: EngineClock,
    /// Authentic PS2 camera controller.
    pub camera: Ps2CameraController,
    /// Active scenario state (James / Maria).
    pub scenario: ScenarioState,
    /// Controller and gamepad manager.
    pub input: InputManager,
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
        let mut camera = Ps2CameraController::new();
        camera.reset_to_player(scenario.player_position, scenario.player_yaw);

        let locomotion_clips = match scenario.character {
            crate::scenario::PlayableCharacter::James => CharacterLocomotionClips::JAMES,
            crate::scenario::PlayableCharacter::Maria => CharacterLocomotionClips::MARIA,
        };

        Self {
            clock: EngineClock::new(60),
            camera,
            scenario,
            input: InputManager::new(Default::default()),
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
        self.camera
            .reset_to_player(self.scenario.player_position, self.scenario.player_yaw);
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
            is_maria: self.scenario.character == crate::scenario::PlayableCharacter::Maria,
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
        self.scenario.player_position += out.delta_position;
        self.last_locomotion = out;

        // 3b. Update animation locomotion state machine based on calibrated locomotion state
        match out.state {
            LocomotionState::Stand | LocomotionState::Relax | LocomotionState::Alert => {
                self.animation
                    .transition_to(AnimationStateKind::Idle, self.locomotion_clips.idle);
            }
            LocomotionState::Walk
            | LocomotionState::StrafeLeftWalk
            | LocomotionState::StrafeRightWalk => {
                self.animation
                    .transition_to(AnimationStateKind::Walk, self.locomotion_clips.walk);
            }
            LocomotionState::Back => {
                self.animation
                    .transition_to(AnimationStateKind::Back, self.locomotion_clips.back);
            }
            LocomotionState::Run1
            | LocomotionState::Run2
            | LocomotionState::Run3
            | LocomotionState::StrafeLeftRun
            | LocomotionState::StrafeRightRun => {
                self.animation
                    .transition_to(AnimationStateKind::Run, self.locomotion_clips.run);
            }
            LocomotionState::TurnLeft | LocomotionState::TurnRight => {
                self.animation
                    .transition_to(AnimationStateKind::Walk, self.locomotion_clips.walk);
            }
            LocomotionState::QuickTurnLeft | LocomotionState::QuickTurnRight => {
                self.animation
                    .transition_to(AnimationStateKind::Walk, self.locomotion_clips.walk);
            }
            LocomotionState::Tired => {
                self.animation
                    .transition_to(AnimationStateKind::Idle, self.locomotion_clips.idle);
            }
        }

        // Fixed tick dt is 1/60s (approx 16.666 ms)
        self.animation.update(Duration::from_nanos(16_666_667));

        // 4. Update authentic PS2 camera
        self.camera.tick(
            self.scenario.player_position,
            self.scenario.player_yaw,
            self.input.state.right_stick_x,
            self.input.state.right_stick_y,
        );

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
}
