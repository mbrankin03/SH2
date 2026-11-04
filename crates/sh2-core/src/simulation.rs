//! Simulation engine orchestrating fixed 60 Hz ticks, timing constants, camera, scenario, and input.

use glam::Vec3;
use std::time::Duration;

use crate::animation::{
    AnimationStateKind, CharacterAnimationController, CharacterLocomotionClips,
};
use crate::camera::Ps2CameraController;
use crate::clock::EngineClock;
use crate::input::{ControlScheme, HapticFeedback, InputManager};
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
    /// Character skeletal animation controller.
    pub animation: CharacterAnimationController,
    /// Active character locomotion clips.
    pub locomotion_clips: CharacterLocomotionClips,
    /// 60 FPS timing constants.
    pub timing: TimingConstants,
    /// Cumulative simulation ticks elapsed.
    pub tick_counter: u64,
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
            animation: CharacterAnimationController::new(
                AnimationStateKind::Idle,
                locomotion_clips.idle,
            ),
            locomotion_clips,
            timing: TimingConstants::default(),
            tick_counter: 0,
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

        // 3. Update character position based on control scheme
        let is_moving;
        let forward_speed = if self.input.state.is_running {
            65.0
        } else {
            25.0
        };
        let turn_speed = 0.05; // radians per tick

        match self.input.config.scheme {
            ControlScheme::Tank => {
                // Classic Tank controls: Left stick Y = forward/backward, Left stick X = rotate yaw
                self.scenario.player_yaw += self.input.state.left_stick_x * turn_speed;
                let forward = Vec3::new(
                    self.scenario.player_yaw.sin(),
                    0.0,
                    self.scenario.player_yaw.cos(),
                );
                self.scenario.player_position +=
                    forward * (self.input.state.left_stick_y * forward_speed);
                is_moving = self.input.state.left_stick_y.abs() > 0.1;
            }
            ControlScheme::AnalogDirectional => {
                // 2D/3D camera-relative directional movement
                if self.input.state.left_stick_x.abs() > 0.01
                    || self.input.state.left_stick_y.abs() > 0.01
                {
                    let move_dir = Vec3::new(
                        self.input.state.left_stick_x,
                        0.0,
                        self.input.state.left_stick_y,
                    )
                    .normalize_or_zero();
                    self.scenario.player_yaw = move_dir.x.atan2(move_dir.z);
                    self.scenario.player_position += move_dir * forward_speed;
                    is_moving = true;
                } else {
                    is_moving = false;
                }
            }
        }

        // 3b. Update animation locomotion state machine based on movement & input
        if is_moving {
            if self.input.state.is_running {
                self.animation
                    .transition_to(AnimationStateKind::Run, self.locomotion_clips.run);
            } else {
                self.animation
                    .transition_to(AnimationStateKind::Walk, self.locomotion_clips.walk);
            }
        } else {
            self.animation
                .transition_to(AnimationStateKind::Idle, self.locomotion_clips.idle);
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
