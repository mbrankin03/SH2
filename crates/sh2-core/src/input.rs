//! Backend-agnostic gamepad and controller input subsystem.
//!
//! Provides support for:
//! - Tank controls (classic Silent Hill 2) vs 2D / 3D camera-relative directional controls.
//! - Deadzone calibration and analog walk/run thresholds.
//! - DualSense PS5 controller feature routing (speaker audio cues, voice-coil haptics/rumble).
//! - Clean merging of host keyboard/mouse axes with platform-polled gamepad snapshots (`SH2-93`, `SH2-145`).

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
    /// Active controller speaker cue.
    pub pending_speaker_cue: Option<ControllerSpeakerCue>,
    /// Active haptic feedback command.
    pub active_haptic: HapticFeedback,
}

/// Backend-agnostic raw gamepad snapshot produced by the host platform layer (`sh2-app`)
/// and consumed by [`InputManager`] during 60 Hz simulation ticks (`SH2-145`).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct GamepadSnapshot {
    /// Whether an active gamepad is connected and providing axis readings.
    pub connected: bool,
    /// Raw left stick X axis (`-1.0..=1.0`, before deadzone filtering).
    pub raw_left_stick_x: f32,
    /// Raw left stick Y axis (`-1.0..=1.0`, before deadzone filtering).
    pub raw_left_stick_y: f32,
    /// Raw right stick X axis (`-1.0..=1.0`, before deadzone filtering).
    pub raw_right_stick_x: f32,
    /// Raw right stick Y axis (`-1.0..=1.0`, before deadzone filtering).
    pub raw_right_stick_y: f32,
    /// Action / Inspect / Attack button currently held (`South`).
    pub action_pressed: bool,
    /// Cancel / Walk modifier button currently held (`East`).
    pub cancel_pressed: bool,
    /// Ready weapon / Aim button currently held (`RightTrigger2`).
    pub ready_weapon_pressed: bool,
    /// Strafe left button currently held (`LeftTrigger`).
    pub strafe_left: bool,
    /// Strafe right button currently held (`RightTrigger`).
    pub strafe_right: bool,
    /// Flashlight toggle edge-triggered press (`North`).
    pub light_toggle_pressed: bool,
    /// Map button edge-triggered press (`Select`).
    pub map_pressed: bool,
    /// Inventory button edge-triggered press (`Start`).
    pub inventory_pressed: bool,
    /// Quick turn (180 degrees) edge-triggered press (`DPadDown`).
    pub quick_turn_pressed: bool,
    /// Camera mode live-cycle edge-triggered press (`RightThumb`).
    pub camera_mode_cycle_pressed: bool,
    /// Over-the-Shoulder left/right shoulder swap edge-triggered press (`LeftThumb`).
    pub shoulder_swap_pressed: bool,
}

/// Applies symmetric radial-axis deadzone filtering with linear rescaling to `[-1.0, 1.0]`.
#[inline]
pub fn filter_deadzone(val: f32, deadzone: f32) -> f32 {
    if val.abs() < deadzone {
        0.0
    } else {
        let sign = val.signum();
        (val.abs() - deadzone) / (1.0 - deadzone) * sign
    }
}

/// Backend-agnostic input manager that merges host keyboard/mouse/gamepad snapshots into [`GamepadInputState`].
pub struct InputManager {
    pub config: ControllerConfig,
    pub state: GamepadInputState,
    pending_light_toggle: bool,
    pending_map: bool,
    pending_inventory: bool,
    pending_camera_cycle: bool,
    pending_shoulder_swap: bool,
    pending_quick_turn: bool,
    pending_mouse_look: [f32; 2],
    pending_gyro_look: [f32; 2],
    host_active: bool,
    host_move: [f32; 2],
    host_look: [f32; 2],
    host_run: bool,
    host_ready: bool,
    host_gamepad_active: bool,
    host_gamepad: GamepadSnapshot,
    gamepad_ready: bool,
}

impl InputManager {
    /// Create a new backend-agnostic input manager.
    pub fn new(config: ControllerConfig) -> Self {
        Self {
            config,
            state: GamepadInputState::default(),
            pending_light_toggle: false,
            pending_map: false,
            pending_inventory: false,
            pending_camera_cycle: false,
            pending_shoulder_swap: false,
            pending_quick_turn: false,
            pending_mouse_look: [0.0; 2],
            pending_gyro_look: [0.0; 2],
            host_active: false,
            host_move: [0.0; 2],
            host_look: [0.0; 2],
            host_run: false,
            host_ready: false,
            host_gamepad_active: false,
            host_gamepad: GamepadSnapshot::default(),
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
    /// `[-1.0, 1.0]` units so 60 Hz ticks merge keyboard and gamepad state cleanly (`SH2-93`, `SH2-145`).
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

    /// Supplies a backend-agnostic gamepad snapshot polled by the host platform layer (`SH2-145`).
    ///
    /// Edge-triggered button presses are latched until the next 60 Hz `update()` tick so they are
    /// never dropped on sub-tick frames or duplicated on multi-tick catch-up frames.
    pub fn set_host_gamepad_state(&mut self, snapshot: GamepadSnapshot) {
        if snapshot.light_toggle_pressed {
            self.pending_light_toggle = true;
        }
        if snapshot.map_pressed {
            self.pending_map = true;
        }
        if snapshot.inventory_pressed {
            self.pending_inventory = true;
        }
        if snapshot.quick_turn_pressed {
            self.pending_quick_turn = true;
        }
        if snapshot.camera_mode_cycle_pressed {
            self.pending_camera_cycle = true;
        }
        if snapshot.shoulder_swap_pressed {
            self.pending_shoulder_swap = true;
        }
        self.gamepad_ready = snapshot.ready_weapon_pressed;
        self.host_gamepad = snapshot;
        self.host_gamepad_active = true;
    }

    /// Update controller input states from queued host keyboard, mouse, gyro, and gamepad snapshots.
    pub fn update(&mut self) {
        // Reset single-frame pulse triggers and flush queued host mouse/gyro/gamepad inputs
        self.state.light_toggle_pressed = std::mem::take(&mut self.pending_light_toggle);
        self.state.map_pressed = std::mem::take(&mut self.pending_map);
        self.state.inventory_pressed = std::mem::take(&mut self.pending_inventory);
        self.state.quick_turn_pressed = std::mem::take(&mut self.pending_quick_turn);
        self.state.camera_mode_cycle_pressed = std::mem::take(&mut self.pending_camera_cycle);
        self.state.shoulder_swap_pressed = std::mem::take(&mut self.pending_shoulder_swap);
        self.state.mouse_look_delta_x = std::mem::take(&mut self.pending_mouse_look[0]);
        self.state.mouse_look_delta_y = std::mem::take(&mut self.pending_mouse_look[1]);
        self.state.gyro_look_delta_yaw = std::mem::take(&mut self.pending_gyro_look[0]);
        self.state.gyro_look_delta_pitch = std::mem::take(&mut self.pending_gyro_look[1]);
        self.state.pending_speaker_cue =
            if self.state.light_toggle_pressed && self.config.dualsense_speaker_enabled {
                Some(ControllerSpeakerCue::FlashlightClick)
            } else {
                None
            };

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
        let mut pad_running = false;
        let mut has_gamepad_axes = false;

        if self.host_gamepad_active {
            self.state.action_pressed = self.host_gamepad.action_pressed;
            self.state.cancel_pressed = self.host_gamepad.cancel_pressed;
            self.state.strafe_left = self.host_gamepad.strafe_left;
            self.state.strafe_right = self.host_gamepad.strafe_right;
            self.state.ready_weapon_pressed =
                self.gamepad_ready || (self.host_active && self.host_ready);

            if self.host_gamepad.connected {
                has_gamepad_axes = true;
                pad_lx = filter_deadzone(
                    self.host_gamepad.raw_left_stick_x,
                    self.config.stick_deadzone,
                );
                pad_ly = filter_deadzone(
                    self.host_gamepad.raw_left_stick_y,
                    self.config.stick_deadzone,
                );
                pad_rx = filter_deadzone(
                    self.host_gamepad.raw_right_stick_x,
                    self.config.stick_deadzone,
                );
                pad_ry = filter_deadzone(
                    self.host_gamepad.raw_right_stick_y,
                    self.config.stick_deadzone,
                );

                let stick_magnitude = (pad_lx * pad_lx + pad_ly * pad_ly).sqrt();
                pad_running = stick_magnitude >= self.config.run_threshold;
            }
        }

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

    #[test]
    fn test_filter_deadzone_and_run_threshold() {
        assert!((filter_deadzone(0.10, 0.15)).abs() < f32::EPSILON);
        assert!((filter_deadzone(-0.14, 0.15)).abs() < f32::EPSILON);
        assert!((filter_deadzone(1.0, 0.15) - 1.0).abs() < 1e-6);
        assert!((filter_deadzone(-1.0, 0.15) + 1.0).abs() < 1e-6);

        let mid = filter_deadzone(0.575, 0.15);
        assert!((mid - 0.5).abs() < 1e-6);

        let mut mgr = InputManager::new(ControllerConfig::default());
        mgr.set_host_gamepad_state(GamepadSnapshot {
            connected: true,
            raw_left_stick_x: 0.10,
            raw_left_stick_y: 0.575,
            ..Default::default()
        });
        mgr.update();
        assert!(mgr.state.left_stick_x.abs() < f32::EPSILON);
        assert!((mgr.state.left_stick_y - 0.5).abs() < 1e-6);
        assert!(!mgr.state.is_running);

        mgr.set_host_gamepad_state(GamepadSnapshot {
            connected: true,
            raw_left_stick_y: 0.90,
            ..Default::default()
        });
        mgr.update();
        assert!(mgr.state.is_running);
    }

    #[test]
    fn test_gamepad_snapshot_buttons_and_keyboard_merge() {
        let mut mgr = InputManager::new(ControllerConfig::default());
        mgr.set_host_keyboard_state(0.5, 0.0, 0.0, 0.0, false, true);
        mgr.set_host_gamepad_state(GamepadSnapshot {
            connected: true,
            raw_left_stick_x: 1.0,
            raw_left_stick_y: 0.0,
            action_pressed: true,
            cancel_pressed: true,
            strafe_left: true,
            strafe_right: false,
            ready_weapon_pressed: false,
            light_toggle_pressed: true,
            map_pressed: true,
            inventory_pressed: true,
            quick_turn_pressed: true,
            camera_mode_cycle_pressed: true,
            shoulder_swap_pressed: true,
            ..Default::default()
        });

        mgr.update();
        assert!((mgr.state.left_stick_x - 1.0).abs() < 1e-6);
        assert!(mgr.state.action_pressed);
        assert!(mgr.state.cancel_pressed);
        assert!(mgr.state.strafe_left);
        assert!(!mgr.state.strafe_right);
        assert!(mgr.state.ready_weapon_pressed);
        assert!(mgr.state.light_toggle_pressed);
        assert!(mgr.state.map_pressed);
        assert!(mgr.state.inventory_pressed);
        assert!(mgr.state.quick_turn_pressed);
        assert!(mgr.state.camera_mode_cycle_pressed);
        assert!(mgr.state.shoulder_swap_pressed);
        assert_eq!(
            mgr.state.pending_speaker_cue,
            Some(ControllerSpeakerCue::FlashlightClick)
        );

        // Next poll with no edge presses clears single-tick pulses
        mgr.set_host_gamepad_state(GamepadSnapshot {
            connected: true,
            ..Default::default()
        });
        mgr.update();
        assert!(!mgr.state.light_toggle_pressed);
        assert!(!mgr.state.map_pressed);
        assert!(!mgr.state.inventory_pressed);
        assert!(!mgr.state.quick_turn_pressed);
        assert!(!mgr.state.camera_mode_cycle_pressed);
        assert!(!mgr.state.shoulder_swap_pressed);
        assert_eq!(mgr.state.pending_speaker_cue, None);
    }
}
