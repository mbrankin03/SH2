//! Authentic Silent Hill 2 locomotion dynamics, turning speeds, and state machine (PS2 1:1 parity).
//!
//! # Parity & Ground Truth Reference
//! - Derived directly from PlayStation 2 / PC `sh2pc.exe` Director's Cut (`move.c`, `move.h`):
//!   - `0x52e110`: Analog cross dead-zone filter and tilt thresholds.
//!   - `0x54e0f0`: Lower-body movement state selection & transition permissions (`STATE_NEXT`).
//!   - `0x8bf6d8`: State movers (turn on spot, walk, back, strafe, run 1/2/3, strafe run, quick 180° turn).
//!   - `0x54ef10`: World velocity scaling: $500\text{ units/s}$ base scalar, angle offset easing at $4\text{ rad/s}$, Maria at $0.8\times$.
//!   - `0x531a00`: Locomotion animation clip mapping and crossfading.
//!   - `0x533ad0`: Stamina counter: 600 frames at full HP, run 3 stamina limit, stamina recovery when standing/walking.
//!   - `0x5567b0`: Halting deceleration ($10\text{ m/s}^2$ normal, $20\text{ m/s}^2$ post-run).

use glam::Vec3;
use std::f32::consts::{FRAC_PI_6, PI};

/// Conversion factor from game velocity units (m/s) to Silent Hill 2 world units per second.
///
/// In PS2/PC SH2 engine (`0x54ef10`), $1\text{ m/s} = 500\text{ world units/s}$.
pub const SH2_WORLD_UNIT_SCALE: f32 = 500.0;

/// Maria velocity multiplier (`0x54ef10`): Maria moves at 80% of James's movement speed.
pub const MARIA_SPEED_MULTIPLIER: f32 = 0.8;

/// Quick turn 180° angular velocity in radians per second ($5.5\text{ rad/s}$, completing $180^\circ$ in $\approx 0.57\text{ s}$).
pub const QUICK_TURN_SPEED_RAD_PER_SEC: f32 = 5.497_787_5;

/// Locomotion state flags matching PS2 lower-body state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum LocomotionState {
    /// Idle stationary standing.
    #[default]
    Stand = 0,
    /// Subtle breathing / relax animation.
    Relax = 1,
    /// Heavy panting / stamina exhausted.
    Tired = 2,
    /// Turn left on spot.
    TurnLeft = 3,
    /// Turn right on spot.
    TurnRight = 4,
    /// Backward walking step.
    Back = 5,
    /// Forward walking.
    Walk = 6,
    /// Left strafe walk.
    StrafeLeftWalk = 7,
    /// Right strafe walk.
    StrafeRightWalk = 8,
    /// Running stage 1 (up to 3.5 m/s = 1750 units/s).
    Run1 = 9,
    /// Running stage 2 (up to 4.0 m/s = 2000 units/s).
    Run2 = 10,
    /// Running stage 3 (sprint at high stamina, up to 4.8 m/s = 2400 units/s).
    Run3 = 11,
    /// Left strafe run (up to 3.8 m/s = 1900 units/s).
    StrafeLeftRun = 12,
    /// Right strafe run (up to 3.8 m/s = 1900 units/s).
    StrafeRightRun = 13,
    /// Quick 180° turn left.
    QuickTurnLeft = 14,
    /// Quick 180° turn right.
    QuickTurnRight = 15,
    /// Alert posture (enemy detected nearby).
    Alert = 16,
}

impl LocomotionState {
    /// Whether this state is any of the running states.
    #[inline]
    pub fn is_running(&self) -> bool {
        matches!(
            self,
            Self::Run1 | Self::Run2 | Self::Run3 | Self::StrafeLeftRun | Self::StrafeRightRun
        )
    }

    /// Whether this state involves translation movement across the floor.
    #[inline]
    pub fn is_moving(&self) -> bool {
        matches!(
            self,
            Self::Back
                | Self::Walk
                | Self::StrafeLeftWalk
                | Self::StrafeRightWalk
                | Self::Run1
                | Self::Run2
                | Self::Run3
                | Self::StrafeLeftRun
                | Self::StrafeRightRun
        )
    }

    /// Whether this state is an active quick 180° turn.
    #[inline]
    pub fn is_quick_turn(&self) -> bool {
        matches!(self, Self::QuickTurnLeft | Self::QuickTurnRight)
    }
}

/// Bitmask helper for valid state transitions matching `0x6cf878` in `move.c`.
const fn state_bit(s: LocomotionState) -> u32 {
    1u32 << (s as u8)
}

const FREE_GO: u32 = state_bit(LocomotionState::TurnLeft)
    | state_bit(LocomotionState::TurnRight)
    | state_bit(LocomotionState::Back)
    | state_bit(LocomotionState::Walk)
    | state_bit(LocomotionState::StrafeLeftWalk)
    | state_bit(LocomotionState::StrafeRightWalk)
    | state_bit(LocomotionState::Run1)
    | state_bit(LocomotionState::StrafeLeftRun)
    | state_bit(LocomotionState::StrafeRightRun)
    | state_bit(LocomotionState::QuickTurnLeft)
    | state_bit(LocomotionState::QuickTurnRight);

const fn get_allowed_transitions(state: LocomotionState) -> u32 {
    match state {
        LocomotionState::Stand => {
            FREE_GO
                | state_bit(LocomotionState::Relax)
                | state_bit(LocomotionState::Tired)
                | state_bit(LocomotionState::Alert)
        }
        LocomotionState::Relax | LocomotionState::Tired | LocomotionState::Alert => {
            FREE_GO | state_bit(LocomotionState::Stand)
        }
        LocomotionState::TurnLeft | LocomotionState::TurnRight => {
            FREE_GO | state_bit(LocomotionState::Stand)
        }
        LocomotionState::Back => {
            state_bit(LocomotionState::Stand)
                | state_bit(LocomotionState::Back)
                | state_bit(LocomotionState::QuickTurnLeft)
                | state_bit(LocomotionState::QuickTurnRight)
        }
        LocomotionState::Walk => {
            state_bit(LocomotionState::Stand)
                | state_bit(LocomotionState::Walk)
                | state_bit(LocomotionState::StrafeLeftWalk)
                | state_bit(LocomotionState::StrafeRightWalk)
                | state_bit(LocomotionState::Run1)
                | state_bit(LocomotionState::Run2)
                | state_bit(LocomotionState::StrafeLeftRun)
                | state_bit(LocomotionState::StrafeRightRun)
                | state_bit(LocomotionState::QuickTurnLeft)
                | state_bit(LocomotionState::QuickTurnRight)
        }
        LocomotionState::StrafeLeftWalk => {
            state_bit(LocomotionState::Stand)
                | state_bit(LocomotionState::Walk)
                | state_bit(LocomotionState::StrafeLeftWalk)
                | state_bit(LocomotionState::Run1)
                | state_bit(LocomotionState::StrafeLeftRun)
                | state_bit(LocomotionState::QuickTurnLeft)
                | state_bit(LocomotionState::QuickTurnRight)
        }
        LocomotionState::StrafeRightWalk => {
            state_bit(LocomotionState::Stand)
                | state_bit(LocomotionState::Walk)
                | state_bit(LocomotionState::StrafeRightWalk)
                | state_bit(LocomotionState::Run1)
                | state_bit(LocomotionState::StrafeRightRun)
                | state_bit(LocomotionState::QuickTurnLeft)
                | state_bit(LocomotionState::QuickTurnRight)
        }
        LocomotionState::Run1 => {
            state_bit(LocomotionState::Stand)
                | state_bit(LocomotionState::Walk)
                | state_bit(LocomotionState::StrafeLeftWalk)
                | state_bit(LocomotionState::StrafeRightWalk)
                | state_bit(LocomotionState::Run1)
                | state_bit(LocomotionState::StrafeLeftRun)
                | state_bit(LocomotionState::StrafeRightRun)
        }
        LocomotionState::Run2 => {
            state_bit(LocomotionState::Stand)
                | state_bit(LocomotionState::Walk)
                | state_bit(LocomotionState::StrafeLeftWalk)
                | state_bit(LocomotionState::StrafeRightWalk)
                | state_bit(LocomotionState::Run2)
                | state_bit(LocomotionState::StrafeLeftRun)
                | state_bit(LocomotionState::StrafeRightRun)
        }
        LocomotionState::Run3 => {
            state_bit(LocomotionState::Stand)
                | state_bit(LocomotionState::Walk)
                | state_bit(LocomotionState::StrafeLeftWalk)
                | state_bit(LocomotionState::StrafeRightWalk)
                | state_bit(LocomotionState::Run3)
                | state_bit(LocomotionState::StrafeLeftRun)
                | state_bit(LocomotionState::StrafeRightRun)
        }
        LocomotionState::StrafeLeftRun => {
            state_bit(LocomotionState::Stand)
                | state_bit(LocomotionState::Walk)
                | state_bit(LocomotionState::StrafeLeftWalk)
                | state_bit(LocomotionState::Run1)
                | state_bit(LocomotionState::StrafeLeftRun)
        }
        LocomotionState::StrafeRightRun => {
            state_bit(LocomotionState::Stand)
                | state_bit(LocomotionState::Walk)
                | state_bit(LocomotionState::StrafeRightWalk)
                | state_bit(LocomotionState::Run1)
                | state_bit(LocomotionState::StrafeRightRun)
        }
        LocomotionState::QuickTurnLeft | LocomotionState::QuickTurnRight => 0, // Quick turn locks out interruptions until finished
    }
}

/// Movement input parameters provided per tick to the locomotion controller.
#[derive(Debug, Clone, Copy)]
pub struct LocomotionInput {
    /// Left analog stick X axis (-1.0 to 1.0, positive = right).
    pub stick_x: f32,
    /// Left analog stick Y axis (-1.0 to 1.0, positive = forward/up).
    pub stick_y: f32,
    /// Run button held or analog trigger running threshold reached.
    pub run_held: bool,
    /// Strafe left button held.
    pub strafe_left: bool,
    /// Strafe right button held.
    pub strafe_right: bool,
    /// Quick turn 180° button pressed this frame.
    pub quick_turn_pressed: bool,
    /// Wall obstacle detected immediately ahead (< 250 units): prevents sprint/run.
    pub wall_ahead: bool,
    /// Enemy detected nearby (triggers alert idle rather than relax).
    pub enemy_near: bool,
    /// Normalized character health `(hp / max_hp)` in `[0.0, 1.0]`.
    pub health_fraction: f32,
    /// Maria active flag: applies 0.8x speed scalar.
    pub is_maria: bool,
    /// Camera yaw in world space (used for Directional / 2D movement mode).
    pub camera_yaw: f32,
}

impl Default for LocomotionInput {
    fn default() -> Self {
        Self {
            stick_x: 0.0,
            stick_y: 0.0,
            run_held: false,
            strafe_left: false,
            strafe_right: false,
            quick_turn_pressed: false,
            wall_ahead: false,
            enemy_near: false,
            health_fraction: 1.0,
            is_maria: false,
            camera_yaw: 0.0,
        }
    }
}

/// Results and movement delta produced by a single locomotion update tick.
#[derive(Debug, Clone, Copy)]
pub struct LocomotionOutput {
    /// Displacement vector in world space for this tick (world units).
    pub delta_position: Vec3,
    /// Instantaneous turning yaw speed in radians per second.
    pub turn_speed_rad_per_sec: f32,
    /// Current active locomotion state.
    pub state: LocomotionState,
    /// Current scalar movement speed in world units per second.
    pub linear_speed: f32,
    /// Footstep acoustic trigger for this tick (true if foot struck ground).
    pub footstep_triggered: bool,
    /// Footstep audio playback volume (0.5 for walk, 0.8 for run).
    pub footstep_volume: f32,
}

/// Full PS2/PC 1:1 authentic player locomotion and turning controller.
#[derive(Debug, Clone)]
pub struct PlayerLocomotionController {
    /// Active locomotion state.
    pub state: LocomotionState,
    /// Previous tick locomotion state.
    pub prev_state: LocomotionState,
    /// Current speed in m/s (multiply by `SH2_WORLD_UNIT_SCALE` for units/s).
    pub speed_mps: f32,
    /// Direction offset relative to character facing yaw (radians).
    pub direction_offset: f32,
    /// Running stamina accumulated (measured in 30 fps frame equivalents, up to 600).
    pub stamina_frames: f32,
    /// Duration in seconds standing completely still (triggers relax after 10.0s).
    pub still_duration: f32,
    /// Quick 180° turn progress angle in radians.
    pub quick_turn_progress: f32,
    /// Target yaw for quick turn completion.
    pub quick_turn_target_yaw: f32,
    /// Camera lock yaw for 2D/Directional control across camera transitions.
    pub cam_lock_yaw: f32,
    /// Stick angle at camera lock.
    pub stick_at_lock: f32,
    /// Whether 2D camera lock is currently engaged.
    pub cam_locked: bool,
    /// Dynamic allowed state transition bitmask.
    allowed_states: u32,
    /// Footstep phase counter (0.0 to 1.0 cycle).
    footstep_phase: f32,
}

impl Default for PlayerLocomotionController {
    fn default() -> Self {
        Self::new()
    }
}

impl PlayerLocomotionController {
    /// Creates a new player locomotion controller initialized to standing state.
    pub fn new() -> Self {
        Self {
            state: LocomotionState::Stand,
            prev_state: LocomotionState::Stand,
            speed_mps: 0.0,
            direction_offset: 0.0,
            stamina_frames: 0.0,
            still_duration: 0.0,
            quick_turn_progress: 0.0,
            quick_turn_target_yaw: 0.0,
            cam_lock_yaw: 0.0,
            stick_at_lock: 0.0,
            cam_locked: false,
            allowed_states: get_allowed_transitions(LocomotionState::Stand),
            footstep_phase: 0.0,
        }
    }

    /// Reset internal controller state.
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Check if transition to target state is currently valid.
    #[inline]
    pub fn can_transition_to(&self, target: LocomotionState) -> bool {
        target == self.state || (self.allowed_states & state_bit(target)) != 0
    }

    /// Enter a new locomotion state and initialize starting entry velocity (`0x54e0f0`).
    fn enter_state(&mut self, new_state: LocomotionState) {
        if new_state == self.state {
            return;
        }

        self.prev_state = self.state;
        self.state = new_state;
        self.still_duration = 0.0;
        self.allowed_states = get_allowed_transitions(new_state);

        // PS2 authentic entry velocities (m/s)
        match new_state {
            LocomotionState::Back => {
                self.speed_mps = 0.6;
            }
            LocomotionState::Walk
            | LocomotionState::StrafeLeftWalk
            | LocomotionState::StrafeRightWalk => {
                if self.speed_mps < 0.6 {
                    self.speed_mps = 0.6;
                }
            }
            LocomotionState::Run1 => {
                if self.speed_mps < 1.5 {
                    self.speed_mps = 1.5;
                }
            }
            LocomotionState::Run2 => {
                if self.speed_mps < 2.5 {
                    self.speed_mps = 2.5;
                }
            }
            LocomotionState::Run3 => {
                if self.speed_mps < 3.0 {
                    self.speed_mps = 3.0;
                }
            }
            LocomotionState::StrafeLeftRun | LocomotionState::StrafeRightRun => {
                self.speed_mps = 2.0;
            }
            _ => {}
        }
    }

    /// Accelerate or decelerate `speed_mps` toward target at specified rates.
    fn ramp_speed(&mut self, target: f32, accel: f32, decel: f32, dt: f32) {
        if self.speed_mps < target {
            self.speed_mps += accel * dt;
            if self.speed_mps > target {
                self.speed_mps = target;
            }
        } else if self.speed_mps > target {
            self.speed_mps -= decel * dt;
            if self.speed_mps < target {
                self.speed_mps = target;
            }
        }
    }

    /// Step player locomotion and turning dynamics by delta time `dt` (seconds).
    ///
    /// Modifies `current_yaw` in place and returns position displacement and metadata.
    pub fn update(
        &mut self,
        input: &LocomotionInput,
        current_yaw: &mut f32,
        dt: f32,
        is_tank: bool,
    ) -> LocomotionOutput {
        let initial_yaw = *current_yaw;
        let hp_clamped = input.health_fraction.clamp(0.0, 1.0);
        let stamina_limit = 600.0 * hp_clamped;

        // 1. Analog Deadzone & Cross-Filtering (PS2 Director's Cut 0x52e110)
        let ax = input.stick_x.abs();
        let ay = input.stick_y.abs();
        let dzx = if ay * 0.5 > 0.244 { ay * 0.5 } else { 0.0 };
        let dzy = if ax * 0.5 > 0.244 { ax * 0.5 } else { 0.0 };

        let mut tx = if ax > dzx {
            (ax - dzx) / (0.67 - dzx).max(0.05)
        } else {
            0.0
        };
        let mut ty = if ay > dzy { (ay - dzy) / 0.79 } else { 0.0 };
        tx = tx.clamp(0.0, 1.0);
        ty = ty.clamp(0.0, 1.0);

        let turn_dir = if tx > 0.0 {
            if input.stick_x > 0.0 {
                1.0
            } else {
                -1.0
            }
        } else {
            0.0
        };

        let mut fwd = ty > 0.0 && input.stick_y > 0.0;
        let mut back = ty > 0.0 && input.stick_y < 0.0;
        let run_held = input.run_held;

        // Directional / 2D Control Scheme handling (PC 0x547bf0 / 0x546760)
        if !is_tank {
            let mx = input.stick_x;
            let my = input.stick_y;
            let mag = (mx * mx + my * my).sqrt();
            back = false;
            tx = 0.0;
            fwd = mag > 0.2;

            if !fwd {
                self.cam_locked = false;
            } else {
                let stick_angle = mx.atan2(my);
                if !self.cam_locked
                    || wrap_angle(stick_angle - self.stick_at_lock).abs() >= FRAC_PI_6
                {
                    self.cam_lock_yaw = input.camera_yaw;
                    self.stick_at_lock = stick_angle;
                    self.cam_locked = true;
                }
                let target_yaw = self.cam_lock_yaw + stick_angle;
                let diff = wrap_angle(target_yaw - *current_yaw);
                let turn_rate = 20.0 * 0.323_457_87 * dt;
                *current_yaw += diff * turn_rate.clamp(0.0, 1.0);
                ty = (mag / 0.79).clamp(0.0, 1.0);
            }
        }

        // 2. State Resolution & Permissions (0x54e0f0)
        let sl = input.strafe_left;
        let sr = input.strafe_right;
        let quick_turn = input.quick_turn_pressed || (sl && sr);

        let mut desired_state = None;

        if self.state.is_quick_turn() {
            desired_state = Some(self.state);
        } else if quick_turn && self.can_transition_to(LocomotionState::QuickTurnLeft) {
            desired_state = Some(if turn_dir > 0.0 {
                LocomotionState::QuickTurnRight
            } else {
                LocomotionState::QuickTurnLeft
            });
            self.quick_turn_progress = 0.0;
            self.quick_turn_target_yaw = wrap_angle(*current_yaw + PI);
        } else if fwd {
            if run_held && !input.wall_ahead {
                if self.can_transition_to(LocomotionState::Run3) {
                    desired_state = Some(LocomotionState::Run3);
                } else if self.can_transition_to(LocomotionState::Run2) {
                    desired_state = Some(LocomotionState::Run2);
                } else if self.can_transition_to(LocomotionState::Run1) {
                    desired_state = Some(LocomotionState::Run1);
                }
            }
            if desired_state.is_none() && self.can_transition_to(LocomotionState::Walk) {
                desired_state = Some(LocomotionState::Walk);
            }
        } else if back && self.can_transition_to(LocomotionState::Back) {
            desired_state = Some(LocomotionState::Back);
        } else if sl != sr {
            let walk_s = if sl {
                LocomotionState::StrafeLeftWalk
            } else {
                LocomotionState::StrafeRightWalk
            };
            let run_s = if sl {
                LocomotionState::StrafeLeftRun
            } else {
                LocomotionState::StrafeRightRun
            };
            if run_held && !input.wall_ahead && self.can_transition_to(run_s) {
                desired_state = Some(run_s);
            } else if self.can_transition_to(walk_s) {
                desired_state = Some(walk_s);
            }
        }

        if desired_state.is_none() && turn_dir != 0.0 && !back {
            let turn_state = if turn_dir < 0.0 {
                LocomotionState::TurnLeft
            } else {
                LocomotionState::TurnRight
            };
            if self.can_transition_to(turn_state) {
                desired_state = Some(turn_state);
            }
        }

        if desired_state.is_none() {
            if (self.state == LocomotionState::Relax || self.state == LocomotionState::Alert)
                && self.still_duration < 3.0
            {
                desired_state = Some(self.state);
            } else if self.stamina_frames > stamina_limit / 3.0 {
                desired_state = Some(LocomotionState::Tired);
            } else if self.state == LocomotionState::Stand && self.still_duration >= 10.0 {
                desired_state = Some(if input.enemy_near {
                    LocomotionState::Alert
                } else {
                    LocomotionState::Relax
                });
            } else {
                desired_state = Some(LocomotionState::Stand);
            }
        }

        let chosen_state = desired_state.unwrap_or(LocomotionState::Stand);
        self.enter_state(chosen_state);

        // 3. Execute State Dynamics & Speeds
        let target_offset;
        let mut yaw_rate = 0.0f32;

        match self.state {
            LocomotionState::Stand
            | LocomotionState::Relax
            | LocomotionState::Tired
            | LocomotionState::Alert => {
                self.still_duration += dt;
                self.stamina_frames = (self.stamina_frames - dt * 30.0).max(0.0);
                // Halting deceleration: 20 m/s^2 after running, 10 m/s^2 normally
                let decel = if self.prev_state.is_running() {
                    20.0
                } else {
                    10.0
                };
                self.speed_mps = (self.speed_mps - decel * dt).max(0.0);
                target_offset = self.direction_offset;
            }
            LocomotionState::TurnLeft | LocomotionState::TurnRight => {
                self.speed_mps = (self.speed_mps - 10.0 * dt).max(0.0);
                // Turn on spot: 2.4 rad/s * tilt (x1.5 with run held)
                let run_multiplier = if run_held { 1.5 } else { 1.0 };
                yaw_rate = 2.4 * tx * run_multiplier;
                target_offset = self.direction_offset;
            }
            LocomotionState::Walk => {
                // Walk: up to 1.5 m/s * tilt (750 units/s)
                self.speed_mps = ty * 1.5;
                yaw_rate = 1.9 * tx;
                target_offset = 0.0;
                self.stamina_frames = (self.stamina_frames - dt * 15.0).max(0.0);
            }
            LocomotionState::Back => {
                // Back: up to 1.2 m/s * tilt (600 units/s)
                self.speed_mps = ty * 1.2;
                yaw_rate = 1.3 * tx;
                target_offset = PI;
                self.stamina_frames = (self.stamina_frames - dt * 15.0).max(0.0);
            }
            LocomotionState::StrafeLeftWalk | LocomotionState::StrafeRightWalk => {
                // Walk strafe: ramp to 1.3 m/s (650 units/s)
                self.ramp_speed(1.3, 1.7, 3.4, dt);
                yaw_rate = 1.9 * tx;
                target_offset = if self.state == LocomotionState::StrafeLeftWalk {
                    -PI * 0.5
                } else {
                    PI * 0.5
                };
            }
            LocomotionState::Run1 | LocomotionState::Run2 | LocomotionState::Run3 => {
                // Run 1: 3.5 m/s (1750 u/s), Run 2: 4.0 m/s (2000 u/s), Run 3: 4.8 m/s (2400 u/s)
                let top_speeds = [3.5f32, 4.0f32, 4.8f32];
                let idx = match self.state {
                    LocomotionState::Run1 => 0,
                    LocomotionState::Run2 => 1,
                    _ => 2,
                };
                let mut target = top_speeds[idx] * ty;
                if self.state == LocomotionState::Run1 && target < 2.0 {
                    target = 2.0;
                }
                // Accelerates at 3.0 m/s^2
                self.ramp_speed(target, 3.0, 3.0, dt);
                yaw_rate = 1.7 * tx;
                self.stamina_frames += dt * 30.0;

                // Stage advancement conditions (0x54cbb0, 0x54cf10, 0x54d300)
                if self.state == LocomotionState::Run1 && self.speed_mps >= 3.5 {
                    self.allowed_states = (self.allowed_states | state_bit(LocomotionState::Run2))
                        & !state_bit(LocomotionState::Run1);
                }
                if self.state == LocomotionState::Run2
                    && self.speed_mps >= 4.0
                    && self.stamina_frames < stamina_limit
                {
                    self.allowed_states = (self.allowed_states | state_bit(LocomotionState::Run3))
                        & !state_bit(LocomotionState::Run2);
                }
                if self.state == LocomotionState::Run3 && self.stamina_frames >= stamina_limit {
                    self.allowed_states = (self.allowed_states | state_bit(LocomotionState::Run2))
                        & !state_bit(LocomotionState::Run3);
                }
                target_offset = 0.0;
            }
            LocomotionState::StrafeLeftRun | LocomotionState::StrafeRightRun => {
                // Strafe run: ramp to 3.8 m/s (1900 units/s)
                self.ramp_speed(3.8, 6.0, 6.0, dt);
                yaw_rate = 1.7 * tx;
                self.stamina_frames += dt * 30.0;
                target_offset = if self.state == LocomotionState::StrafeLeftRun {
                    -PI * 0.5
                } else {
                    PI * 0.5
                };
            }
            LocomotionState::QuickTurnLeft | LocomotionState::QuickTurnRight => {
                self.speed_mps = 0.0;
                let step = QUICK_TURN_SPEED_RAD_PER_SEC * dt;
                self.quick_turn_progress += step;
                if self.quick_turn_progress >= PI {
                    *current_yaw = self.quick_turn_target_yaw;
                    self.enter_state(LocomotionState::Stand);
                } else if self.state == LocomotionState::QuickTurnLeft {
                    *current_yaw -= step;
                } else {
                    *current_yaw += step;
                }
                target_offset = 0.0;
                self.direction_offset = 0.0;
            }
        }

        // Apply turning rate in Tank controls
        if is_tank && turn_dir != 0.0 && !self.state.is_quick_turn() {
            *current_yaw += turn_dir * yaw_rate * dt;
        }

        // Offset easing at 4.0 rad/s (0x54ef10)
        if self.state.is_moving() && self.prev_state.is_moving() {
            let diff = wrap_angle(target_offset - self.direction_offset);
            let max_ease = 4.0 * dt;
            self.direction_offset += diff.clamp(-max_ease, max_ease);
        } else if self.state.is_moving() {
            self.direction_offset = target_offset;
        }

        *current_yaw = wrap_angle(*current_yaw);

        // Calculate final translation velocity vector in world space
        let maria_mult = if input.is_maria {
            MARIA_SPEED_MULTIPLIER
        } else {
            1.0
        };
        let final_speed_units_per_sec = self.speed_mps * SH2_WORLD_UNIT_SCALE * maria_mult;
        let move_angle = *current_yaw + self.direction_offset;

        let delta_x = move_angle.sin() * final_speed_units_per_sec * dt;
        let delta_z = move_angle.cos() * final_speed_units_per_sec * dt;
        let delta_position = Vec3::new(delta_x, 0.0, delta_z);

        // Footstep timing & acoustic volume
        let mut footstep_triggered = false;
        let footstep_volume = if self.state.is_running() { 0.8 } else { 0.5 };

        if self.state.is_moving() && self.speed_mps > 0.1 {
            let stride_freq = if self.state.is_running() { 2.4 } else { 1.6 };
            self.footstep_phase += dt * stride_freq;
            if self.footstep_phase >= 0.5 {
                self.footstep_phase -= 0.5;
                footstep_triggered = true;
            }
        } else {
            self.footstep_phase = 0.0;
        }

        let turn_speed = if dt > 0.0 {
            wrap_angle(*current_yaw - initial_yaw) / dt
        } else {
            0.0
        };

        LocomotionOutput {
            delta_position,
            turn_speed_rad_per_sec: turn_speed,
            state: self.state,
            linear_speed: final_speed_units_per_sec,
            footstep_triggered,
            footstep_volume,
        }
    }
}

/// Helper to wrap angles within `[-PI, PI]`.
#[inline]
pub fn wrap_angle(mut angle: f32) -> f32 {
    while angle > PI {
        angle -= 2.0 * PI;
    }
    while angle < -PI {
        angle += 2.0 * PI;
    }
    angle
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ps2_movement_speeds_and_parity_constants() {
        let mut controller = PlayerLocomotionController::new();
        let mut yaw = 0.0f32;

        // 1. Walk test at full stick tilt (1.0)
        let mut input = LocomotionInput {
            stick_y: 1.0,
            ..Default::default()
        };
        let dt = 1.0 / 60.0;

        let out = controller.update(&input, &mut yaw, dt, true);
        assert_eq!(out.state, LocomotionState::Walk);
        // Speed should be exactly 1.5 m/s * 500 = 750 units/s
        assert!((controller.speed_mps - 1.5).abs() < 1e-3);
        assert!((out.linear_speed - 750.0).abs() < 1e-3);
        // Per-tick displacement at 60 Hz = 750 / 60 = 12.5 units
        assert!((out.delta_position.z - 12.5).abs() < 1e-2);

        // 2. Backward walk test
        controller.reset();
        input.stick_y = -1.0;
        let out_back = controller.update(&input, &mut yaw, dt, true);
        assert_eq!(out_back.state, LocomotionState::Back);
        // Speed should be 1.2 m/s * 500 = 600 units/s (~10.0 units/tick)
        assert!((controller.speed_mps - 1.2).abs() < 1e-3);
        assert!((out_back.linear_speed - 600.0).abs() < 1e-3);
        assert!((out_back.delta_position.z - (-10.0)).abs() < 1e-2);

        // 3. Maria 0.8x multiplier test
        controller.reset();
        input.stick_y = 1.0;
        input.is_maria = true;
        let out_maria = controller.update(&input, &mut yaw, dt, true);
        assert_eq!(out_maria.state, LocomotionState::Walk);
        assert!((out_maria.linear_speed - 600.0).abs() < 1e-3); // 750 * 0.8 = 600
    }

    #[test]
    fn test_ps2_turning_speeds_and_quick_turn() {
        let mut controller = PlayerLocomotionController::new();
        let mut yaw = 0.0f32;
        let dt = 1.0 / 60.0;

        // Spot turn test (stick_x = 1.0) -> 2.4 rad/s
        let input_turn = LocomotionInput {
            stick_x: 1.0,
            ..Default::default()
        };
        let out_turn = controller.update(&input_turn, &mut yaw, dt, true);
        assert_eq!(out_turn.state, LocomotionState::TurnRight);
        // 2.4 rad/s * (1/60s) = 0.04 rad/tick
        assert!((yaw - 0.04).abs() < 1e-3);

        // Spot turn with run held -> 2.4 * 1.5 = 3.6 rad/s
        yaw = 0.0;
        let input_turn_run = LocomotionInput {
            stick_x: 1.0,
            run_held: true,
            ..Default::default()
        };
        controller.update(&input_turn_run, &mut yaw, dt, true);
        assert!((yaw - 0.06).abs() < 1e-3); // 3.6 / 60 = 0.06 rad/tick

        // Quick turn test (180° = PI rad at 5.4977875 rad/s)
        controller.reset();
        yaw = 0.0;
        let input_quick = LocomotionInput {
            quick_turn_pressed: true,
            ..Default::default()
        };
        let out_qt = controller.update(&input_quick, &mut yaw, dt, true);
        assert!(out_qt.state.is_quick_turn());

        // Step until complete (PI / 5.4977875 s = ~0.5714 s -> ~35 ticks)
        let mut ticks = 0;
        while controller.state.is_quick_turn() {
            controller.update(&LocomotionInput::default(), &mut yaw, dt, true);
            ticks += 1;
        }
        assert!((34..=36).contains(&ticks));
        assert!((yaw.abs() - PI).abs() < 1e-2);
    }

    #[test]
    fn test_ps2_running_acceleration_and_deceleration() {
        let mut controller = PlayerLocomotionController::new();
        let mut yaw = 0.0f32;
        let dt = 1.0 / 60.0;

        let input_run = LocomotionInput {
            stick_y: 1.0,
            run_held: true,
            ..Default::default()
        };

        // First tick enters Run1 at 1.5 m/s
        let out = controller.update(&input_run, &mut yaw, dt, true);
        assert_eq!(out.state, LocomotionState::Run1);
        assert!(controller.speed_mps >= 1.5);

        // Advance 1 second (60 ticks): acceleration at 3.0 m/s^2 progresses through Run1 (3.5 m/s) and Run2 (4.0 m/s) to Run3
        for _ in 0..60 {
            controller.update(&input_run, &mut yaw, dt, true);
        }
        assert!(controller.speed_mps >= 3.5);
        assert!(controller.state.is_running());

        // Let go of stick -> deceleration at 20 m/s^2 post-run
        let top_speed = controller.speed_mps;
        let out_stop = controller.update(&LocomotionInput::default(), &mut yaw, dt, true);
        assert_eq!(out_stop.state, LocomotionState::Stand);
        let speed_after_1_tick = controller.speed_mps;
        assert!(speed_after_1_tick < top_speed);
        // After 20 ticks (~0.33s), speed should have dropped by 20.0 * 0.33 = 6.6 m/s -> fully stopped (0.0)
        for _ in 0..20 {
            controller.update(&LocomotionInput::default(), &mut yaw, dt, true);
        }
        assert_eq!(controller.speed_mps, 0.0);
    }
}
