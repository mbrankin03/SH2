//! Platform gamepad polling and event translation using `gilrs` (`SH2-145`).
//!
//! Translates hardware `gilrs` events and axis readings into backend-agnostic
//! [`sh2_core::GamepadSnapshot`] values consumed by `sh2-core`.

use gilrs::{Axis, Button, Event, EventType, GamepadId, Gilrs};
use sh2_core::GamepadSnapshot;

/// Tracked held-button state across `gilrs` event polls.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct HeldGamepadButtons {
    /// Action / Inspect / Attack button (`South`).
    pub action_pressed: bool,
    /// Cancel / Walk modifier button (`East`).
    pub cancel_pressed: bool,
    /// Ready weapon / Aim button (`RightTrigger2`).
    pub ready_weapon_pressed: bool,
    /// Strafe left button (`LeftTrigger`).
    pub strafe_left: bool,
    /// Strafe right button (`RightTrigger`).
    pub strafe_right: bool,
}

/// Translates a single `gilrs` [`EventType`] into held-button state and edge-triggered
/// [`GamepadSnapshot`] flags, preserving 1:1 mapping parity with the PS2/PC controller layout.
pub fn apply_gilrs_event(
    held: &mut HeldGamepadButtons,
    snapshot: &mut GamepadSnapshot,
    event: &EventType,
) {
    match event {
        EventType::ButtonPressed(button, _) => {
            apply_gilrs_button(held, snapshot, *button, true);
        }
        EventType::ButtonReleased(button, _) => {
            apply_gilrs_button(held, snapshot, *button, false);
        }
        _ => {}
    }
}

/// Applies a pressed or released `gilrs` [`Button`] to held-button state and [`GamepadSnapshot`].
pub fn apply_gilrs_button(
    held: &mut HeldGamepadButtons,
    snapshot: &mut GamepadSnapshot,
    button: Button,
    pressed: bool,
) {
    match (button, pressed) {
        (Button::South, _) => {
            held.action_pressed = pressed;
            snapshot.action_pressed = pressed;
        }
        (Button::East, _) => {
            held.cancel_pressed = pressed;
            snapshot.cancel_pressed = pressed;
        }
        (Button::RightTrigger2, _) => {
            held.ready_weapon_pressed = pressed;
            snapshot.ready_weapon_pressed = pressed;
        }
        (Button::North, true) => {
            snapshot.light_toggle_pressed = true;
        }
        (Button::Select, true) => {
            snapshot.map_pressed = true;
        }
        (Button::Start, true) => {
            snapshot.inventory_pressed = true;
        }
        (Button::DPadDown, true) => {
            snapshot.quick_turn_pressed = true;
        }
        (Button::LeftTrigger, _) => {
            held.strafe_left = pressed;
            snapshot.strafe_left = pressed;
        }
        (Button::RightTrigger, _) => {
            held.strafe_right = pressed;
            snapshot.strafe_right = pressed;
        }
        (Button::RightThumb, true) => {
            snapshot.camera_mode_cycle_pressed = true;
        }
        (Button::LeftThumb, true) => {
            snapshot.shoulder_swap_pressed = true;
        }
        _ => {}
    }
}

/// Host-side `gilrs` gamepad poller and active-device tracker.
pub struct GilrsGamepadPoller {
    gilrs: Option<Gilrs>,
    active_gamepad: Option<GamepadId>,
    held: HeldGamepadButtons,
}

impl Default for GilrsGamepadPoller {
    fn default() -> Self {
        Self::new()
    }
}

impl GilrsGamepadPoller {
    /// Initializes `gilrs` gamepad polling. Safely falls back if no gamepad driver is present.
    pub fn new() -> Self {
        Self {
            gilrs: Gilrs::new().ok(),
            active_gamepad: None,
            held: HeldGamepadButtons::default(),
        }
    }

    /// Drains pending `gilrs` events and samples the active gamepad's raw analog axes.
    pub fn poll(&mut self) -> GamepadSnapshot {
        let mut snapshot = GamepadSnapshot {
            action_pressed: self.held.action_pressed,
            cancel_pressed: self.held.cancel_pressed,
            ready_weapon_pressed: self.held.ready_weapon_pressed,
            strafe_left: self.held.strafe_left,
            strafe_right: self.held.strafe_right,
            ..Default::default()
        };

        if let Some(ref mut gilrs) = self.gilrs {
            while let Some(Event { id, event, .. }) = gilrs.next_event() {
                self.active_gamepad = Some(id);
                apply_gilrs_event(&mut self.held, &mut snapshot, &event);
            }

            if let Some(id) = self.active_gamepad {
                let gamepad = gilrs.gamepad(id);
                snapshot.connected = true;
                snapshot.raw_left_stick_x = gamepad.value(Axis::LeftStickX);
                snapshot.raw_left_stick_y = gamepad.value(Axis::LeftStickY);
                snapshot.raw_right_stick_x = gamepad.value(Axis::RightStickX);
                snapshot.raw_right_stick_y = gamepad.value(Axis::RightStickY);
            }
        }

        snapshot
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apply_gilrs_button_translates_held_and_edge_buttons() {
        let mut held = HeldGamepadButtons::default();
        let mut snap = GamepadSnapshot::default();

        for button in [
            Button::South,
            Button::East,
            Button::RightTrigger2,
            Button::LeftTrigger,
            Button::RightTrigger,
            Button::North,
            Button::Select,
            Button::Start,
            Button::DPadDown,
            Button::RightThumb,
            Button::LeftThumb,
        ] {
            apply_gilrs_button(&mut held, &mut snap, button, true);
        }

        assert!(snap.action_pressed);
        assert!(snap.cancel_pressed);
        assert!(snap.ready_weapon_pressed);
        assert!(snap.strafe_left);
        assert!(snap.strafe_right);
        assert!(snap.light_toggle_pressed);
        assert!(snap.map_pressed);
        assert!(snap.inventory_pressed);
        assert!(snap.quick_turn_pressed);
        assert!(snap.camera_mode_cycle_pressed);
        assert!(snap.shoulder_swap_pressed);

        for button in [
            Button::South,
            Button::East,
            Button::RightTrigger2,
            Button::LeftTrigger,
            Button::RightTrigger,
        ] {
            apply_gilrs_button(&mut held, &mut snap, button, false);
        }

        assert!(!held.action_pressed);
        assert!(!held.cancel_pressed);
        assert!(!held.ready_weapon_pressed);
        assert!(!held.strafe_left);
        assert!(!held.strafe_right);
        assert!(!snap.action_pressed);
        assert!(!snap.cancel_pressed);
        assert!(!snap.ready_weapon_pressed);
        assert!(!snap.strafe_left);
        assert!(!snap.strafe_right);
    }
}
