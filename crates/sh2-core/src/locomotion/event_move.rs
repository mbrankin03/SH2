//! Authentic PS2/PC `PlayerEventMove` (`SLUS_202.28` `Chacter/m3_play_event.c` `0x00283d10`,
//! `sh2pc.exe` `0x0053b3c0`) scripted auto-turn and walk-back controller.
//!
//! # Ground Truth Reference
//! - `SLUS_202.28` `Chacter/m3_play_event.c` / `sh2pc.exe`:
//!   - `PlayerEventMove` (`0x00283d10` / `0x0053b3c0`): selects `Run` (`> 3500.0 mm`), `Walk`
//!     (`> 87.5 mm`), or `Stand` (`<= 87.5 mm`) and dispatches through `func_list_event` (`0x008ba558`).
//!   - `event_jms_stand` (`0x00283a40` / `0x0053b210`): calls `PlayerSpeedDownToStand`
//!     (`0x0021c560` / `0x00556b60`) to decelerate at `10.0 m/s^2` (or `20.0 m/s^2` from run).
//!   - `event_jms_walk` (`0x00283a70` / `0x0053b220`): ramps speed to `1.5 m/s` (`+2.5 m/s^2` /
//!     `-5.0 m/s^2`), computes `shAtan2(dz, dx) = atan2(dx, dz)`, and smoothly rotates `rot_y` via
//!     `close_to_angle_target(..., -PI, PI, 12.0)`.
//!   - `event_jms_run` (`0x00283c00` / `0x0053b310`): ramps speed to `4.0 m/s` (`+3.0 m/s^2`),
//!     rotates `rot_y` via `close_to_angle_target(..., -PI, PI, 8.0)`.
//!   - `PlayerEventMoveIsEnd` (`0x00283ed0` / `0x0053b6d0`) & `PlayerEventMoveCancel`
//!     (`0x00283f50` / `0x0053b4a0`).
//!   - `close_to_angle_target` (`SLUS_202.28` `0x00111160`, `sh2pc.exe` `FUN_00534790` `0x00534790`).
//!   - `PlayerCheckAnimeUpper` (`0x00109f90`) / `PlayerCheckAnimeLower` (`0x0010c600`): maps
//!     `Stand -> 0x65` (101), `Walk -> 0x68` (104), `Run -> 0x6b` (107).
//!
//! # PS2 vs. PC Drift Notes
//! - In `SLUS_202.28` (`0x00283d10`), the stop-radius threshold uses double literal
//!   `0x4055dffff9c00000` (`~87.4999985 mm`); in retail `sh2pc.exe` (`0x0053b410`), it is exact
//!   `87.5 mm`.
//! - In `SLUS_202.28` (`0x00111160`), `close_to_angle_target` divides by `3.0915928f` (`PI - 0.05`);
//!   in retail `sh2pc.exe` (`0x00534790`), the compiler pre-reciprocated it to
//!   `0.3234578690569946` (`f64`), which matches `1.0 / 3.0915928` to single-precision accuracy.

use glam::Vec3;
use std::f32::consts::PI;

use super::{
    wrap_angle, LocomotionOutput, LocomotionState, MIN_FOOTSTEP_INTERVAL_SECS,
    SH2_WORLD_UNIT_SCALE, WATER_SPEED_MULTIPLIER,
};

/// Retail boundary turn-back offset distance in PS2 mm (`sh2pc.exe` `0x0062d230 = 500.0f`).
///
/// Used by all four progression-blocking boundary programs in `sh2pc.exe`:
/// - Stage 3 (`stg_ovservation.c`) `ev_prog[4]` (`0x005a0cb0`): `-500.0 mm` along Z
/// - Stage 3 (`stg_ovservation.c`) `ev_prog[5]` (`0x005a0de0`): `-500.0 mm` along X
/// - Stage 5 (`stg_town_east.c`) `ev_prog[4]` (`0x0059d720`): `+500.0 mm` along X
/// - Stage 14 (`stg_town_west.c`) `ev_prog[4]` (`0x00592550`): `+500.0 mm` along Z
pub const BOUNDARY_WALKBACK_DISTANCE_MM: f32 = 500.0;

/// Distance threshold above which `PlayerEventMove` selects `Run` (`SLUS_202.28` `0x00283d10`,
/// `sh2pc.exe` `0x0053b404`: `3500.0 mm`).
pub const EVENT_MOVE_RUN_THRESHOLD_MM: f32 = 3500.0;

/// Arrival stop radius in PS2 mm where `PlayerEventMove` transitions to `Stand`
/// (`sh2pc.exe` `0x0053b410`: `87.5 mm`; `SLUS_202.28` `0x00283d10`: `~87.5 mm`).
pub const EVENT_MOVE_STOP_RADIUS_MM: f32 = 87.5;

/// Target walk speed in m/s during `event_jms_walk` (`SLUS_202.28` `0x00283a70`,
/// `sh2pc.exe` `0x0053b220` via `0x0069f33c = 1.5f`).
pub const EVENT_WALK_TARGET_SPEED_MPS: f32 = 1.5;

/// Walk acceleration in m/s^2 during `event_jms_walk` (`SLUS_202.28` `0x00283a70`,
/// `sh2pc.exe` `0x0053b265` via `0x00638850 = 2.5f`).
pub const EVENT_WALK_ACCEL_MPS2: f32 = 2.5;

/// Walk deceleration in m/s^2 when slowing from run into `event_jms_walk`
/// (`SLUS_202.28` `0x00283a70`, `sh2pc.exe` `0x0053b23f` via `0x006318d8 = 5.0f`).
pub const EVENT_WALK_DECEL_MPS2: f32 = 5.0;

/// Yaw interpolation gain passed to `close_to_angle_target` in `event_jms_walk`
/// (`SLUS_202.28` `0x00283a70` `0x41400000`, `sh2pc.exe` `0x0053b2c4` `0x41400000` = `12.0`).
pub const EVENT_WALK_TURN_GAIN: f32 = 12.0;

/// Target run speed in m/s during `event_jms_run` (`SLUS_202.28` `0x00283c00`,
/// `sh2pc.exe` `0x0053b310`: `4.0`).
pub const EVENT_RUN_TARGET_SPEED_MPS: f32 = 4.0;

/// Run acceleration in m/s^2 during `event_jms_run` (`SLUS_202.28` `0x00283c00`,
/// `sh2pc.exe` `0x0053b310`: `3.0`).
pub const EVENT_RUN_ACCEL_MPS2: f32 = 3.0;

/// Yaw interpolation gain passed to `close_to_angle_target` in `event_jms_run`
/// (`SLUS_202.28` `0x00283c00` `0x41000000`, `sh2pc.exe` `0x0053b310` `0x41000000` = `8.0`).
pub const EVENT_RUN_TURN_GAIN: f32 = 8.0;

/// Halting deceleration in m/s^2 from walk (`PlayerSpeedDownToStand` `SLUS_202.28` `0x0021c560`,
/// `sh2pc.exe` `0x00556b60`: `10.0`).
pub const EVENT_STAND_DECEL_WALK_MPS2: f32 = 10.0;

/// Halting deceleration in m/s^2 from run (`PlayerSpeedDownToStand` `SLUS_202.28` `0x0021c560`,
/// `sh2pc.exe` `0x00556b60`: `20.0`).
pub const EVENT_STAND_DECEL_RUN_MPS2: f32 = 20.0;

/// Reciprocal angle normalization constant in `close_to_angle_target`
/// (`sh2pc.exe` `0x00534790`: `0.3234578690569946`, equivalent to `1.0 / 3.0915928` in
/// `SLUS_202.28` `0x00111160`).
pub const CLOSE_TO_ANGLE_INV_DENOM: f32 = 0.323_457_87;

/// James animation clip ID for `PlayerEventMove` `Stand` (`SLUS_202.28` `0x00109f90` / `0x0010c600`: `0x65`).
pub const EVENT_MOVE_ANIM_STAND: u16 = 0x65;
/// James animation clip ID for `PlayerEventMove` `Walk` (`SLUS_202.28` `0x00109f90` / `0x0010c600`: `0x68`).
pub const EVENT_MOVE_ANIM_WALK: u16 = 0x68;
/// James animation clip ID for `PlayerEventMove` `Run` (`SLUS_202.28` `0x00109f90` / `0x0010c600`: `0x6b`).
pub const EVENT_MOVE_ANIM_RUN: u16 = 0x6b;

/// Authentic `close_to_angle_target` (`SLUS_202.28` `Chacter/m3_play.c` `0x00111160`,
/// `sh2pc.exe` `FUN_00534790` `0x00534790`).
///
/// Smoothly rotates `*current` (radians in `[-PI, PI]`) toward `target` (clamped to `[min, max]`)
/// using exponential proportional damping `step = wrap_angle(*current - target) * 0.32345787 * gain * dt`,
/// snapping cleanly to `target` without overshoot.
pub fn close_to_angle_target(
    target: f32,
    min: f32,
    max: f32,
    gain: f32,
    current: &mut f32,
    dt: f32,
) {
    let clamped_target = target.clamp(min, max);
    let diff = wrap_angle(*current - clamped_target);
    let step = diff * CLOSE_TO_ANGLE_INV_DENOM * gain * dt;
    let remaining = diff - step;

    if (diff >= 0.0 && remaining <= 0.0) || (diff < 0.0 && remaining >= 0.0) {
        *current = wrap_angle(clamped_target);
    } else {
        *current = wrap_angle(*current - step);
    }
}

/// Sub-state of `PlayerEventMove` (`DAT_003c7f5a` in `SLUS_202.28`, `DAT_01fb7cf8` in `sh2pc.exe`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum EventMoveState {
    /// `0xff`: Inactive / not yet initialized (`PlayerEventMoveCancel`).
    #[default]
    Idle = 0xff,
    /// `0`: Reached `<= 87.5 mm` of target; decelerating via `event_jms_stand` (`0x65`).
    Stand = 0,
    /// `1`: `87.5 mm < dist <= 3500.0 mm`; walking via `event_jms_walk` (`0x68`).
    Walk = 1,
    /// `2`: `dist > 3500.0 mm`; running via `event_jms_run` (`0x6b`).
    Run = 2,
}

/// Stateful controller executing `PlayerEventMove` (`0x00283d10` / `0x0053b3c0`) across frames
/// while player input is locked (`JmsEventMode(jms, 1)`).
#[derive(Debug, Clone, Default)]
pub struct PlayerEventMoveController {
    /// `DAT_003c7f5c` / `DAT_01fb7cfa`: whether scripted event movement is active.
    active: bool,
    /// `DAT_003c7f5a` / `DAT_01fb7cf8`: current event-move sub-state.
    state: EventMoveState,
    /// `DAT_003c7f5b` / `DAT_01fb7cf9`: previous event-move sub-state.
    prev_state: EventMoveState,
    /// Target world position in PS2 mm `[x, y, z]`.
    target_pos: Vec3,
    /// Current forward speed in m/s (`chara + 0xac` / `chara + 0xa8`).
    speed_mps: f32,
    /// Tracks whether `Stand` was entered from `Run` so `PlayerSpeedDownToStand` uses `20.0 m/s^2`.
    came_from_run: bool,
    /// Normalized 24-frame walk/run gait phase for footstep triggers during auto-walk.
    footstep_phase: f32,
    /// Inter-step debounce timer in seconds (`SH2:EE DoubleFootstepFix.cpp`).
    footstep_cooldown: f32,
}

impl PlayerEventMoveController {
    /// Creates an inactive `PlayerEventMoveController`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Starts a new `PlayerEventMove` sequence toward `target_pos` (in PS2 mm), resetting speed
    /// to `0.0 m/s` to match James standing still during `PlayerEventAnimeSet(0x65)` prior to
    /// `PlayerEventMove`.
    pub fn start(&mut self, target_pos: Vec3) {
        self.active = true;
        self.state = EventMoveState::Idle;
        self.prev_state = EventMoveState::Idle;
        self.target_pos = target_pos;
        self.speed_mps = 0.0;
        self.came_from_run = false;
        self.footstep_phase = 0.0;
        self.footstep_cooldown = 0.0;
    }

    /// Aborts/clears active event movement (`PlayerEventMoveCancel` `0x00283f50` / `0x0053b4a0`).
    pub fn cancel(&mut self) {
        self.active = false;
        self.state = EventMoveState::Idle;
        self.prev_state = EventMoveState::Idle;
        self.speed_mps = 0.0;
        self.came_from_run = false;
        self.footstep_phase = 0.0;
        self.footstep_cooldown = 0.0;
    }

    /// Returns `true` while `PlayerEventMove` is actively driving James.
    #[inline]
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Returns `true` when `PlayerEventMoveIsEnd` (`0x00283ed0` / `0x0053b6d0`) has completed.
    #[inline]
    pub fn is_end(&self) -> bool {
        !self.active || (self.state == EventMoveState::Stand && self.speed_mps <= 0.0)
    }

    /// Current `EventMoveState` sub-state (`Stand` / `Walk` / `Run` / `Idle`).
    #[inline]
    pub fn state(&self) -> EventMoveState {
        self.state
    }

    /// Current linear speed in m/s (`chara + 0xac`).
    #[inline]
    pub fn speed_mps(&self) -> f32 {
        self.speed_mps
    }

    /// Target world position in PS2 mm.
    #[inline]
    pub fn target_pos(&self) -> Vec3 {
        self.target_pos
    }

    /// Advances `PlayerEventMove` by `dt` seconds, smoothly rotating `*current_yaw` toward
    /// `target_pos` via `close_to_angle_target` and returning a [`LocomotionOutput`] suitable for
    /// collision resolution and `LocomotionAnimationSync`.
    pub fn step(
        &mut self,
        current_pos: Vec3,
        current_yaw: &mut f32,
        in_water: bool,
        floor_normal: Vec3,
        dt: f32,
    ) -> LocomotionOutput {
        if !self.active || dt <= 0.0 {
            return LocomotionOutput {
                delta_position: Vec3::ZERO,
                turn_speed_rad_per_sec: 0.0,
                state: LocomotionState::Stand,
                linear_speed: 0.0,
                footstep_triggered: false,
                footstep_volume: 0.5,
            };
        }

        let initial_yaw = *current_yaw;
        let dx = self.target_pos.x - current_pos.x;
        let dz = self.target_pos.z - current_pos.z;
        let distance = (dx * dx + dz * dz).sqrt();

        // `PlayerEventMove` (`SLUS_202.28` `0x00283d10`, `sh2pc.exe` `0x0053b3c0`):
        // Once `state == Stand`, it stays in `Stand` until `PlayerEventMoveIsEnd` finishes.
        if self.state != EventMoveState::Stand {
            let next_state = if distance > EVENT_MOVE_RUN_THRESHOLD_MM {
                EventMoveState::Run
            } else if distance > EVENT_MOVE_STOP_RADIUS_MM {
                EventMoveState::Walk
            } else {
                EventMoveState::Stand
            };
            if next_state != self.state {
                if next_state == EventMoveState::Stand {
                    self.came_from_run = self.state == EventMoveState::Run;
                }
                self.prev_state = self.state;
                self.state = next_state;
            }
        }

        // Dispatch `func_list_event[state]` (`0x008ba558`):
        match self.state {
            EventMoveState::Stand | EventMoveState::Idle => {
                // `event_jms_stand` (`0x00283a40` / `0x0053b210`) -> `PlayerSpeedDownToStand`
                let decel = if self.came_from_run {
                    EVENT_STAND_DECEL_RUN_MPS2
                } else {
                    EVENT_STAND_DECEL_WALK_MPS2
                };
                self.speed_mps = (self.speed_mps - decel * dt).max(0.0);
            }
            EventMoveState::Walk => {
                // `event_jms_walk` (`0x00283a70` / `0x0053b220`)
                if self.speed_mps > EVENT_WALK_TARGET_SPEED_MPS {
                    self.speed_mps = (self.speed_mps - EVENT_WALK_DECEL_MPS2 * dt)
                        .max(EVENT_WALK_TARGET_SPEED_MPS);
                } else {
                    self.speed_mps = (self.speed_mps + EVENT_WALK_ACCEL_MPS2 * dt)
                        .min(EVENT_WALK_TARGET_SPEED_MPS);
                }
                // `shAtan2(dz, dx)` (`sh2pc.exe` `0x004482d0`: `fpatan(dx, dz)`)
                let target_yaw = dx.atan2(dz);
                close_to_angle_target(target_yaw, -PI, PI, EVENT_WALK_TURN_GAIN, current_yaw, dt);
            }
            EventMoveState::Run => {
                // `event_jms_run` (`0x00283c00` / `0x0053b310`)
                self.speed_mps =
                    (self.speed_mps + EVENT_RUN_ACCEL_MPS2 * dt).min(EVENT_RUN_TARGET_SPEED_MPS);
                let target_yaw = dx.atan2(dz);
                close_to_angle_target(target_yaw, -PI, PI, EVENT_RUN_TURN_GAIN, current_yaw, dt);
            }
        }

        // `PlayerUpdatePosition3D` (`0x0011c340`): world displacement along `current_yaw`
        // (`direction_offset == 0.0` during `event_jms_walk` / `event_jms_run`).
        let mut effective_speed_mps = self.speed_mps;
        if in_water {
            effective_speed_mps *= WATER_SPEED_MULTIPLIER;
        }
        let linear_speed = effective_speed_mps * SH2_WORLD_UNIT_SCALE;
        let cos_nx = floor_normal.x.cos();
        let cos_nz = floor_normal.z.cos();
        let delta_x = current_yaw.sin() * linear_speed * dt * cos_nx * cos_nx;
        let delta_z = current_yaw.cos() * linear_speed * dt * cos_nz * cos_nz;

        // Footstep phase sync (24-frame gait cycle, heel strikes at frames 5 & 17).
        self.footstep_cooldown = (self.footstep_cooldown - dt).max(0.0);
        let mut footstep_triggered = false;
        let loco_state = match self.state {
            EventMoveState::Run => LocomotionState::Run2,
            EventMoveState::Walk => LocomotionState::Walk,
            EventMoveState::Stand | EventMoveState::Idle => LocomotionState::Stand,
        };
        let footstep_volume = if self.state == EventMoveState::Run {
            0.8
        } else {
            0.5
        };

        if matches!(self.state, EventMoveState::Walk | EventMoveState::Run) && self.speed_mps > 0.1
        {
            let effective_anim_fps = if self.state == EventMoveState::Run {
                26.25
            } else {
                let ty = (self.speed_mps / EVENT_WALK_TARGET_SPEED_MPS).clamp(0.0, 1.0);
                (512.0 + 896.0 * ty) / 2048.0 * 30.0
            };
            let prev_phase = self.footstep_phase;
            self.footstep_phase += dt * (effective_anim_fps / 24.0);
            const HEEL_STRIKE_1: f32 = 5.0 / 24.0;
            const HEEL_STRIKE_2: f32 = 17.0 / 24.0;
            let crossed = (prev_phase < HEEL_STRIKE_1 && self.footstep_phase >= HEEL_STRIKE_1)
                || (prev_phase < HEEL_STRIKE_2 && self.footstep_phase >= HEEL_STRIKE_2)
                || (self.footstep_phase >= 1.0 + HEEL_STRIKE_1);
            if self.footstep_phase >= 1.0 {
                self.footstep_phase %= 1.0;
            }
            if crossed && self.footstep_cooldown <= 0.0 {
                footstep_triggered = true;
                self.footstep_cooldown = MIN_FOOTSTEP_INTERVAL_SECS;
            }
        } else {
            self.footstep_phase = 0.0;
        }

        // `PlayerEventMoveIsEnd` (`0x00283ed0` / `0x0053b6d0`): once in `Stand` and speed reaches
        // zero, invoke `PlayerEventMoveCancel` (`0x00283f50` / `0x0053b4a0`).
        if self.state == EventMoveState::Stand && self.speed_mps <= 0.0 {
            self.cancel();
        }

        let turn_speed_rad_per_sec = wrap_angle(*current_yaw - initial_yaw) / dt;

        LocomotionOutput {
            delta_position: Vec3::new(delta_x, 0.0, delta_z),
            turn_speed_rad_per_sec,
            state: loco_state,
            linear_speed,
            footstep_triggered,
            footstep_volume,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn close_to_angle_target_matches_slus_00111160_and_never_overshoots() {
        let mut yaw = 0.0_f32;
        let dt = 1.0 / 60.0;
        close_to_angle_target(PI * 0.5, -PI, PI, EVENT_WALK_TURN_GAIN, &mut yaw, dt);
        // First step: diff = -PI/2, step = -PI/2 * 0.32345787 * 12.0 / 60.0 = -0.101617 rad
        let expected = (PI * 0.5) * CLOSE_TO_ANGLE_INV_DENOM * EVENT_WALK_TURN_GAIN * dt;
        assert!((yaw - expected).abs() < 1e-5);

        // Large dt must clamp cleanly to target without overshooting.
        close_to_angle_target(PI * 0.5, -PI, PI, EVENT_WALK_TURN_GAIN, &mut yaw, 5.0);
        assert!((yaw - PI * 0.5).abs() < 1e-5);
    }

    #[test]
    fn player_event_move_smoothly_turns_180_and_walks_back_without_jutting_forward() {
        let mut ctrl = PlayerEventMoveController::new();
        let mut pos = Vec3::new(0.0, 0.0, 0.0);
        let mut yaw = 0.0_f32; // Facing +Z (toward blocked stairs)
        let target = Vec3::new(0.0, 0.0, -BOUNDARY_WALKBACK_DISTANCE_MM);
        ctrl.start(target);

        let dt = 1.0 / 60.0;
        let mut max_forward_z = 0.0_f32;
        let mut saw_walk = false;
        let mut saw_stand_decel = false;
        let mut frames = 0_u32;

        while !ctrl.is_end() && frames < 240 {
            let out = ctrl.step(pos, &mut yaw, false, Vec3::new(0.0, -1.0, 0.0), dt);
            pos += out.delta_position;
            max_forward_z = max_forward_z.max(pos.z);
            if out.state == LocomotionState::Walk {
                saw_walk = true;
            }
            if out.state == LocomotionState::Stand && frames > 10 {
                saw_stand_decel = true;
            }
            frames += 1;
        }

        assert!(ctrl.is_end(), "PlayerEventMove should finish within 4s");
        assert!(saw_walk, "James must enter Walk state (clip 0x68)");
        assert!(
            saw_stand_decel,
            "James must decelerate into Stand (clip 0x65)"
        );
        // Smooth turn-around takes ~65-75 frames (~1.15s) at 60 Hz, not 1-frame snap!
        assert!(
            (40..=120).contains(&frames),
            "expected ~69 frames for 500mm 180-deg turn-around, got {frames}"
        );
        // Peak forward excursion during the initial pivot is ~7.3 mm (< 15 mm), never jutting forward.
        assert!(
            max_forward_z < 15.0,
            "James must not jut forward during turn-around; max_forward_z = {max_forward_z} mm"
        );
        // Final position is near target (-500 mm along Z, within arrival + decel margin).
        assert!(
            pos.z < -400.0 && pos.z > -560.0,
            "expected final Z near -500 mm, got {}",
            pos.z
        );
        // Final yaw faces away from +Z (within ~45 deg of ±PI).
        assert!(
            yaw.abs() > PI * 0.75,
            "expected final yaw facing away from blocked +Z boundary, got {} deg",
            yaw.to_degrees()
        );
    }

    #[test]
    fn player_event_move_selects_run_above_3500mm_then_transitions_to_walk_and_stand() {
        let mut ctrl = PlayerEventMoveController::new();
        let mut pos = Vec3::ZERO;
        let mut yaw = 0.0_f32;
        ctrl.start(Vec3::new(0.0, 0.0, 4200.0));

        let dt = 1.0 / 60.0;
        let mut saw_run = false;
        let mut saw_walk = false;

        for _ in 0..360 {
            if ctrl.is_end() {
                break;
            }
            let out = ctrl.step(pos, &mut yaw, false, Vec3::new(0.0, -1.0, 0.0), dt);
            pos += out.delta_position;
            if out.state == LocomotionState::Run2 {
                saw_run = true;
            } else if out.state == LocomotionState::Walk {
                saw_walk = true;
            }
        }

        assert!(saw_run, "distance > 3500 mm must enter Run (clip 0x6b)");
        assert!(
            saw_walk,
            "distance <= 3500 mm must transition to Walk (clip 0x68)"
        );
        assert!(ctrl.is_end());
        assert!((pos.z - 4200.0).abs() < 120.0);
    }
}
