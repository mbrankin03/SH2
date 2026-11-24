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
    /// Camera mode live-cycle button (`R3` / Right Thumb) pressed this tick (`SH2-93`).
    pub camera_mode_cycle_pressed: bool,
    /// Over-the-Shoulder left/right shoulder swap button (`L3` / Left Thumb) pressed this tick (`SH2-93`).
    pub shoulder_swap_pressed: bool,
    /// Accumulated mouse horizontal yaw look delta in radians for this 60 Hz tick (`SH2-93`).
    pub mouse_look_delta_x: f32,
    /// Accumulated mouse vertical pitch look delta in radians for this 60 Hz tick (`SH2-93`).
    pub mouse_look_delta_y: f32,
    /// Accumulated gyro motion-sensor horizontal yaw delta in radians for this 60 Hz tick (`SH2-87`/`SH2-93`).
    pub gyro_look_delta_yaw: f32,
    /// Accumulated gyro motion-sensor vertical pitch delta in radians for this 60 Hz tick (`SH2-87`/`SH2-93`).
    pub gyro_look_delta_pitch: f32,
    /// Analog `R2` / Right Trigger 2 pressure (`0.0..=1.0`) for pressure-sensitive combat (`SH2-83`).
    pub r2_analog_pressure: f32,
    /// Zero-latency native split Light Attack / Quick Kick pulse (`SH2-83`).
    pub native_light_attack_pressed: bool,
    /// Zero-latency native split Heavy Attack / Two-Footed Stomp pulse (`SH2-83`).
    pub native_heavy_attack_pressed: bool,
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
    pending_camera_cycle: bool,
    pending_shoulder_swap: bool,
    pending_quick_turn: bool,
    pending_native_light_attack: bool,
    pending_native_heavy_attack: bool,
    pending_mouse_look: [f32; 2],
    pending_gyro_look: [f32; 2],
    host_active: bool,
    host_move: [f32; 2],
    host_look: [f32; 2],
    host_run: bool,
    host_ready: bool,
    host_r2_pressure: f32,
    gamepad_ready: bool,
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
            pending_camera_cycle: false,
            pending_shoulder_swap: false,
            pending_quick_turn: false,
            pending_native_light_attack: false,
            pending_native_heavy_attack: false,
            pending_mouse_look: [0.0; 2],
            pending_gyro_look: [0.0; 2],
            host_active: false,
            host_move: [0.0; 2],
            host_look: [0.0; 2],
            host_run: false,
            host_ready: false,
            host_r2_pressure: 0.0,
            gamepad_ready: false,
        }
    }

    /// Queues a single-tick `R3` camera mode cycle trigger for the next 60 Hz `update()` (`SH2-93`).
    pub fn press_camera_mode_cycle(&mut self) {
        self.pending_camera_cycle = true;
    }

    /// Queues a single-tick `L3` OTS shoulder swap trigger for the next 60 Hz `update()` (`SH2-93`).
    pub fn press_shoulder_swap(&mut self) {
        self.pending_shoulder_swap = true;
    }

    /// Queues a single-tick 180-degree quick-turn trigger for the next 60 Hz `update()` (`SH2-93`).
    pub fn press_quick_turn(&mut self) {
        self.pending_quick_turn = true;
    }

    /// Supplies held desktop keyboard/mouse axes (`WASD`, `IJKL`, `Shift`, `R`/Right-Click) in normalized
    /// `[-1.0, 1.0]` units so 60 Hz ticks merge keyboard and `gilrs` gamepad state cleanly (`SH2-93`).
    pub fn set_host_keyboard_state(
        &mut self,
        move_x: f32,
        move_y: f32,
        look_x: f32,
        look_y: f32,
        run_held: bool,
        ready_held: bool,
    ) {
        self.host_active = true;
        self.host_move = [move_x.clamp(-1.0, 1.0), move_y.clamp(-1.0, 1.0)];
        self.host_look = [look_x.clamp(-1.0, 1.0), look_y.clamp(-1.0, 1.0)];
        self.host_run = run_held;
        self.host_ready = ready_held;
    }

    /// Resets held desktop keyboard/mouse axes (e.g. on pause menu open, focus loss, or room exit) (`SH2-93`).
    pub fn clear_host_keyboard_state(&mut self) {
        self.host_move = [0.0; 2];
        self.host_look = [0.0; 2];
        self.host_run = false;
        self.host_ready = false;
    }

    /// Accumulates mouse look yaw/pitch deltas (in radians) to be consumed on the next 60 Hz `update()` (`SH2-93`).
    pub fn accumulate_mouse_look(&mut self, delta_yaw_rad: f32, delta_pitch_rad: f32) {
        self.pending_mouse_look[0] += delta_yaw_rad;
        self.pending_mouse_look[1] += delta_pitch_rad;
    }

    /// Accumulates gyro look yaw/pitch deltas (in radians) to be consumed on the next 60 Hz `update()` (`SH2-87`/`SH2-93`).
    pub fn accumulate_gyro_look(&mut self, delta_yaw_rad: f32, delta_pitch_rad: f32) {
        self.pending_gyro_look[0] += delta_yaw_rad;
        self.pending_gyro_look[1] += delta_pitch_rad;
    }

    /// Pulses a zero-latency native split Light Attack / Quick Kick input for the next `update()` (`SH2-83`).
    pub fn press_native_light_attack(&mut self) {
        self.pending_native_light_attack = true;
    }

    /// Pulses a zero-latency native split Heavy Attack / Two-Footed Stomp input for the next `update()` (`SH2-83`).
    pub fn press_native_heavy_attack(&mut self) {
        self.pending_native_heavy_attack = true;
    }

    /// Sets simulated host `R2` analog trigger pressure (`0.0..=1.0`) for testing/keyboard-mouse (`SH2-83`).
    pub fn set_host_r2_pressure(&mut self, pressure: f32) {
        self.host_r2_pressure = pressure.clamp(0.0, 1.0);
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
        // Reset single-frame pulse triggers and flush queued host mouse/gyro/R3/quick-turn inputs
        self.state.light_toggle_pressed = false;
        self.state.map_pressed = false;
        self.state.inventory_pressed = false;
        self.state.quick_turn_pressed = std::mem::take(&mut self.pending_quick_turn);
        self.state.native_light_attack_pressed =
            std::mem::take(&mut self.pending_native_light_attack);
        self.state.native_heavy_attack_pressed =
            std::mem::take(&mut self.pending_native_heavy_attack);
        self.state.camera_mode_cycle_pressed = std::mem::take(&mut self.pending_camera_cycle);
        self.state.shoulder_swap_pressed = std::mem::take(&mut self.pending_shoulder_swap);
        self.state.mouse_look_delta_x = std::mem::take(&mut self.pending_mouse_look[0]);
        self.state.mouse_look_delta_y = std::mem::take(&mut self.pending_mouse_look[1]);
        self.state.gyro_look_delta_yaw = std::mem::take(&mut self.pending_gyro_look[0]);
        self.state.gyro_look_delta_pitch = std::mem::take(&mut self.pending_gyro_look[1]);
        self.state.pending_speaker_cue = None;

        // Decrement haptic timer
        if self.state.active_haptic.duration_ticks > 0 {
            self.state.active_haptic.duration_ticks -= 1;
            if self.state.active_haptic.duration_ticks == 0 {
                self.state.active_haptic = HapticFeedback::default();
            }
        }

        let mut pad_lx = 0.0_f32;
        let mut pad_ly = 0.0_f32;
        let mut pad_rx = 0.0_f32;
        let mut pad_ry = 0.0_f32;
        let mut pad_r2 = 0.0_f32;
        let mut pad_running = false;
        let mut has_gamepad_axes = false;

        if let Some(ref mut gilrs) = self.gilrs {
            while let Some(Event { id, event, .. }) = gilrs.next_event() {
                self.active_gamepad = Some(id);
                match event {
                    EventType::ButtonPressed(Button::South, _) => self.state.action_pressed = true,
                    EventType::ButtonReleased(Button::South, _) => {
                        self.state.action_pressed = false
                    }

                    EventType::ButtonPressed(Button::East, _) => self.state.cancel_pressed = true,
                    EventType::ButtonReleased(Button::East, _) => self.state.cancel_pressed = false,

                    EventType::ButtonPressed(Button::RightTrigger2, _) => {
                        self.gamepad_ready = true;
                        self.state.ready_weapon_pressed = true;
                    }
                    EventType::ButtonReleased(Button::RightTrigger2, _) => {
                        self.gamepad_ready = false;
                        self.state.ready_weapon_pressed = self.host_active && self.host_ready;
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

                    EventType::ButtonPressed(Button::RightThumb, _) => {
                        self.state.camera_mode_cycle_pressed = true;
                    }

                    EventType::ButtonPressed(Button::LeftThumb, _) => {
                        self.state.shoulder_swap_pressed = true;
                    }

                    _ => {}
                }
            }

            if let Some(id) = self.active_gamepad {
                has_gamepad_axes = true;
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

                pad_lx = filter_deadzone(raw_lx, self.config.stick_deadzone);
                pad_ly = filter_deadzone(raw_ly, self.config.stick_deadzone);
                pad_rx = filter_deadzone(raw_rx, self.config.stick_deadzone);
                pad_ry = filter_deadzone(raw_ry, self.config.stick_deadzone);

                let stick_magnitude = (pad_lx * pad_lx + pad_ly * pad_ly).sqrt();
                pad_running = stick_magnitude >= self.config.run_threshold;
                pad_r2 = gamepad
                    .button_data(Button::RightTrigger2)
                    .map(|d| d.value().clamp(0.0, 1.0))
                    .unwrap_or_else(|| gamepad.value(Axis::RightZ).clamp(0.0, 1.0));
            }
        }

        self.state.r2_analog_pressure = pad_r2.max(self.host_r2_pressure);

        if self.host_active {
            self.state.left_stick_x = (pad_lx + self.host_move[0]).clamp(-1.0, 1.0);
            self.state.left_stick_y = (pad_ly + self.host_move[1]).clamp(-1.0, 1.0);
            self.state.right_stick_x = (pad_rx + self.host_look[0]).clamp(-1.0, 1.0);
            self.state.right_stick_y = (pad_ry + self.host_look[1]).clamp(-1.0, 1.0);
            self.state.is_running = pad_running || self.host_run;
            self.state.ready_weapon_pressed = self.gamepad_ready || self.host_ready;
        } else if has_gamepad_axes {
            self.state.left_stick_x = pad_lx;
            self.state.left_stick_y = pad_ly;
            self.state.right_stick_x = pad_rx;
            self.state.right_stick_y = pad_ry;
            self.state.is_running = pad_running;
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

        mgr.press_camera_mode_cycle();
        mgr.press_shoulder_swap();
        mgr.accumulate_mouse_look(0.15, -0.05);
        mgr.accumulate_gyro_look(0.02, 0.01);
        mgr.update();
        assert!(mgr.state.camera_mode_cycle_pressed);
        assert!(mgr.state.shoulder_swap_pressed);
        assert!((mgr.state.mouse_look_delta_x - 0.15).abs() < 1e-6);
        assert!((mgr.state.gyro_look_delta_yaw - 0.02).abs() < 1e-6);

        // Next update clears single-tick pulses and look deltas
        mgr.update();
        assert!(!mgr.state.camera_mode_cycle_pressed);
        assert!(!mgr.state.shoulder_swap_pressed);
        assert!(mgr.state.mouse_look_delta_x.abs() < 1e-6);
    }
}
