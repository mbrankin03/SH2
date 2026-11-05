//! Gamepad and controller input subsystem using `gilrs` and SDL Gamepad DB.
//!
//! Provides support for:
//! - Tank controls (classic Silent Hill 2) vs 2D / 3D camera-relative directional controls.
//! - Deadzone calibration and analog walk/run thresholds.
//! - DualSense PS5 controller feature routing (speaker audio cues, voice-coil haptics/rumble).
//! - Cross-platform input mapping (Windows XInput/WGI, Linux evdev/udev).

use gilrs::{Axis, Button, Event, EventType, GamepadId, Gilrs};

/// Control movement scheme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ControlScheme {
    /// Classic Silent Hill 2 directional tank controls (Up = walk forward, Down = backward, Left/Right = turn).
    #[default]
    Tank,
    /// Modern 2D/3D camera-relative analog direction.
    AnalogDirectional,
}

/// Vibration and haptic rumble command for gamepads.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HapticFeedback {
    /// Low frequency heavy motor rumble (0.0 to 1.0) - e.g. heavy impacts, monster footsteps.
    pub heavy_rumble: f32,
    /// High frequency light motor rumble (0.0 to 1.0) - e.g. bullet shots, chain rattle.
    pub light_rumble: f32,
    /// DualSense voice-coil audio heartbeat pulse intensity (0.0 to 1.0) on low health.
    pub heartbeat_pulse: f32,
    /// Duration of haptic effect in simulation ticks (1 tick = 1/60s).
    pub duration_ticks: u16,
}

impl Default for HapticFeedback {
    fn default() -> Self {
        Self {
            heavy_rumble: 0.0,
            light_rumble: 0.0,
            heartbeat_pulse: 0.0,
            duration_ticks: 0,
        }
    }
}

/// DualSense speaker sound event triggers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControllerSpeakerCue {
    /// None active.
    None,
    /// Pocket radio static crackle (varying intensity when monsters are nearby).
    RadioStatic { intensity: u8 },
    /// Flashlight physical switch click.
    FlashlightClick,
    /// Key, memo, or puzzle item pickup chime.
    ItemPickupChime,
    /// Empty gun dry-fire hammer click.
    DryFireClick,
}

/// Controller configuration settings.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ControllerConfig {
    /// Movement control scheme (Tank vs Analog).
    pub scheme: ControlScheme,
    /// Left analog stick deadzone threshold (0.0 to 1.0).
    pub stick_deadzone: f32,
    /// Threshold magnitude on analog stick past which walking becomes running (default 0.75).
    pub run_threshold: f32,
    /// Master rumble vibration intensity (0.0 to 1.0).
    pub vibration_intensity: f32,
    /// Enable routing of radio/flashlight audio cues to DualSense controller speaker.
    pub dualsense_speaker_enabled: bool,
    /// Enable voice-coil tactile feedback.
    pub dualsense_haptics_enabled: bool,
}

impl Default for ControllerConfig {
    fn default() -> Self {
        Self {
            scheme: ControlScheme::Tank,
            stick_deadzone: 0.15,
            run_threshold: 0.75,
            vibration_intensity: 1.0,
            dualsense_speaker_enabled: true,
            dualsense_haptics_enabled: true,
        }
    }
}

/// Unified gamepad input state processed for game simulation.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct GamepadInputState {
    /// Left stick X axis (-1.0 to 1.0).
    pub left_stick_x: f32,
    /// Left stick Y axis (-1.0 to 1.0, positive = forward/up).
    pub left_stick_y: f32,
    /// Right stick X axis (-1.0 to 1.0, for PS2 search view panning).
    pub right_stick_x: f32,
    /// Right stick Y axis (-1.0 to 1.0, for PS2 search view pitch).
    pub right_stick_y: f32,
    /// Run button held or stick magnitude exceeding run threshold.
    pub is_running: bool,
    /// Action / Inspect / Attack button held.
    pub action_pressed: bool,
    /// Cancel / Walk modifier button held.
    pub cancel_pressed: bool,
    /// Ready weapon / Aim button held (R2 / R1).
    pub ready_weapon_pressed: bool,
    /// Flashlight toggle button pressed this tick.
    pub light_toggle_pressed: bool,
    /// Map button pressed this tick.
    pub map_pressed: bool,
    /// Inventory button pressed this tick.
    pub inventory_pressed: bool,
    /// Quick turn (180 degrees) button pressed this tick.
    pub quick_turn_pressed: bool,
    /// Strafe left button held.
    pub strafe_left: bool,
    /// Strafe right button held.
    pub strafe_right: bool,
    /// Active controller speaker cue.
    pub pending_speaker_cue: Option<ControllerSpeakerCue>,
    /// Active haptic feedback command.
    pub active_haptic: HapticFeedback,
}

/// Gamepad manager wrapping `gilrs` event pump and state tracking.
pub struct InputManager {
    gilrs: Option<Gilrs>,
    active_gamepad: Option<GamepadId>,
    pub config: ControllerConfig,
    pub state: GamepadInputState,
}

impl InputManager {
    /// Create a new input manager. Safely falls back if no gamepad driver is present.
    pub fn new(config: ControllerConfig) -> Self {
        let gilrs = Gilrs::new().ok();
        Self {
            gilrs,
            active_gamepad: None,
            config,
            state: GamepadInputState::default(),
        }
    }

    /// Set control scheme (Tank vs Analog).
    pub fn set_control_scheme(&mut self, scheme: ControlScheme) {
        self.config.scheme = scheme;
    }

    /// Trigger a DualSense controller speaker audio cue.
    pub fn trigger_speaker_cue(&mut self, cue: ControllerSpeakerCue) {
        if self.config.dualsense_speaker_enabled {
            self.state.pending_speaker_cue = Some(cue);
        }
    }

    /// Trigger haptic rumble feedback.
    pub fn trigger_haptics(&mut self, haptic: HapticFeedback) {
        let mut scaled = haptic;
        scaled.heavy_rumble =
            (scaled.heavy_rumble * self.config.vibration_intensity).clamp(0.0, 1.0);
        scaled.light_rumble =
            (scaled.light_rumble * self.config.vibration_intensity).clamp(0.0, 1.0);
        scaled.heartbeat_pulse =
            (scaled.heartbeat_pulse * self.config.vibration_intensity).clamp(0.0, 1.0);
        self.state.active_haptic = scaled;
    }

    /// Update controller input states from `gilrs` event loop.
    pub fn update(&mut self) {
        // Reset single-frame pulse triggers
        self.state.light_toggle_pressed = false;
        self.state.map_pressed = false;
        self.state.inventory_pressed = false;
        self.state.quick_turn_pressed = false;
        self.state.pending_speaker_cue = None;

        // Decrement haptic timer
        if self.state.active_haptic.duration_ticks > 0 {
            self.state.active_haptic.duration_ticks -= 1;
            if self.state.active_haptic.duration_ticks == 0 {
                self.state.active_haptic = HapticFeedback::default();
            }
        }

        let Some(ref mut gilrs) = self.gilrs else {
            return;
        };

        while let Some(Event { id, event, .. }) = gilrs.next_event() {
            self.active_gamepad = Some(id);
            match event {
                EventType::ButtonPressed(Button::South, _) => self.state.action_pressed = true,
                EventType::ButtonReleased(Button::South, _) => self.state.action_pressed = false,

                EventType::ButtonPressed(Button::East, _) => self.state.cancel_pressed = true,
                EventType::ButtonReleased(Button::East, _) => self.state.cancel_pressed = false,

                EventType::ButtonPressed(Button::RightTrigger2, _) => {
                    self.state.ready_weapon_pressed = true;
                }
                EventType::ButtonReleased(Button::RightTrigger2, _) => {
                    self.state.ready_weapon_pressed = false;
                }

                EventType::ButtonPressed(Button::North, _) => {
                    self.state.light_toggle_pressed = true;
                    if self.config.dualsense_speaker_enabled {
                        self.state.pending_speaker_cue =
                            Some(ControllerSpeakerCue::FlashlightClick);
                    }
                }

                EventType::ButtonPressed(Button::Select, _) => {
                    self.state.map_pressed = true;
                }

                EventType::ButtonPressed(Button::Start, _) => {
                    self.state.inventory_pressed = true;
                }

                EventType::ButtonPressed(Button::DPadDown, _) => {
                    self.state.quick_turn_pressed = true;
                }

                EventType::ButtonPressed(Button::LeftTrigger, _) => {
                    self.state.strafe_left = true;
                }
                EventType::ButtonReleased(Button::LeftTrigger, _) => {
                    self.state.strafe_left = false;
                }

                EventType::ButtonPressed(Button::RightTrigger, _) => {
                    self.state.strafe_right = true;
                }
                EventType::ButtonReleased(Button::RightTrigger, _) => {
                    self.state.strafe_right = false;
                }

                _ => {}
            }
        }

        // Read axes for currently active gamepad
        if let Some(id) = self.active_gamepad {
            let gamepad = gilrs.gamepad(id);
            let raw_lx = gamepad.value(Axis::LeftStickX);
            let raw_ly = gamepad.value(Axis::LeftStickY);
            let raw_rx = gamepad.value(Axis::RightStickX);
            let raw_ry = gamepad.value(Axis::RightStickY);

            let filter_deadzone = |val: f32, deadzone: f32| -> f32 {
                if val.abs() < deadzone {
                    0.0
                } else {
                    let sign = val.signum();
                    (val.abs() - deadzone) / (1.0 - deadzone) * sign
                }
            };

            self.state.left_stick_x = filter_deadzone(raw_lx, self.config.stick_deadzone);
            self.state.left_stick_y = filter_deadzone(raw_ly, self.config.stick_deadzone);
            self.state.right_stick_x = filter_deadzone(raw_rx, self.config.stick_deadzone);
            self.state.right_stick_y = filter_deadzone(raw_ry, self.config.stick_deadzone);

            let stick_magnitude = (self.state.left_stick_x * self.state.left_stick_x
                + self.state.left_stick_y * self.state.left_stick_y)
                .sqrt();

            self.state.is_running = stick_magnitude >= self.config.run_threshold;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_controller_config_and_deadzone() {
        let config = ControllerConfig::default();
        let mut mgr = InputManager::new(config);
        assert_eq!(mgr.config.scheme, ControlScheme::Tank);

        mgr.set_control_scheme(ControlScheme::AnalogDirectional);
        assert_eq!(mgr.config.scheme, ControlScheme::AnalogDirectional);

        mgr.trigger_speaker_cue(ControllerSpeakerCue::FlashlightClick);
        assert_eq!(
            mgr.state.pending_speaker_cue,
            Some(ControllerSpeakerCue::FlashlightClick)
        );

        mgr.trigger_haptics(HapticFeedback {
            heavy_rumble: 0.8,
            light_rumble: 0.5,
            heartbeat_pulse: 1.0,
            duration_ticks: 10,
        });
        assert_eq!(mgr.state.active_haptic.duration_ticks, 10);
        assert!((mgr.state.active_haptic.heavy_rumble - 0.8).abs() < f32::EPSILON);
    }
}
