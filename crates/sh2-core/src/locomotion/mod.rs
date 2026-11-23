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

mod states;
mod tables;
#[cfg(test)]
mod tests;

use glam::Vec3;
use std::f32::consts::{FRAC_PI_6, PI};

use tables::{get_allowed_transitions, state_bit};
pub use tables::{
    LocomotionState, DIRECTION_OFFSET_EASE_RUN_RAD_PER_SEC, DIRECTION_OFFSET_EASE_WALK_RAD_PER_SEC,
    GREAT_KNIFE_BACK_SPEEDS, GREAT_KNIFE_QUICK_TURN_SPEED_RAD_PER_SEC, GREAT_KNIFE_STRAFE_SPEEDS,
    GREAT_KNIFE_WALK_SPEEDS, MARIA_COMPANION_SPEED_MULTIPLIER, MIN_FOOTSTEP_INTERVAL_SECS,
    QUICK_TURN_SPEED_RAD_PER_SEC, SH2_WORLD_UNIT_SCALE, WALL_F_MIN_SPEED_MPS,
    WALL_F_NORMAL_DOT_THRESHOLD, WATER_SPEED_MULTIPLIER, WATER_TURN_MULTIPLIER,
};

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
    /// Wall obstacle detected immediately ahead (< 250 units): prevents sprint/run (`DAT_003c8405`).
    pub wall_ahead: bool,
    /// Head-height forward probe hit a wall (`PlayerCheckStraightLine` `0x00110890`).
    pub wall_head_hit: bool,
    /// Dot product of sloped forward ray direction with hit wall normal (`_DAT_003c7f34`; `<= -0.85` triggers `WallF`).
    pub wall_normal_dot: f32,
    /// Enemy detected nearby (triggers alert idle rather than relax).
    pub enemy_near: bool,
    /// Normalized character health `(hp / max_hp)` in `[0.0, 1.0]`.
    pub health_fraction: f32,
    /// Maria companion follower flag: applies 0.8x speed scalar when following James as NPC.
    /// In playable scenario (*Born From a Wish*), Maria uses standard 1.0x player speed.
    pub is_maria_follower: bool,
    /// Camera yaw in world space (used for Directional / 2D movement mode).
    pub camera_yaw: f32,
    /// Digital D-pad/keyboard input mode (`DAT_003c836a == 0`) vs analog stick (`1`, default `false`).
    pub is_digital_input: bool,
    /// Weapon readied / aiming flag (`DAT_003c8403`: enables Aim+Run 2.25/1.8 m/s & suppresses tank turn).
    pub is_aiming: bool,
    /// Ampoule infinite-stamina efficacy active (`ItemAmpolueEfficacy() != 0.0`).
    pub ampoule_active: bool,
    /// Standing/wading in water (`PlayerWaterRoadIsOn` / `DAT_003c7f44 == 2`: 0.65x speed, 1.3x turn, no Run3).
    pub in_water: bool,
    /// Carrying the Great Knife (`DAT_003c8408 == 8`: enables `*_nata` speed/turn overrides & disables Run).
    pub is_great_knife: bool,
    /// Current lower-body animation frame index (`0..24`), used by Great Knife `*_nata` speed tables.
    pub anim_frame: u8,
    /// Normalized floor surface normal from `PlayerCheckFootLine` (`_DAT_003c8140..48`) for `cos^2` slope scaling.
    pub floor_normal: Vec3,
    /// External `PlayerSetHeight` drop trigger (`> 250 mm`) requesting transition to `LocomotionState::Fall`.
    pub trigger_fall: bool,
    /// Lower-body animation completion flag (`_DAT_003c7f70 & 1`), used by `lower_wall_f_3d` (`0x0011a170`)
    /// and `lower_fall` (`0x0021f5c0`) to re-enable transition back to `LocomotionState::Stand`.
    pub lower_anim_finished: bool,
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
            wall_head_hit: false,
            wall_normal_dot: 0.0,
            enemy_near: false,
            health_fraction: 1.0,
            is_maria_follower: false,
            camera_yaw: 0.0,
            is_digital_input: false,
            is_aiming: false,
            ampoule_active: false,
            in_water: false,
            is_great_knife: false,
            anim_frame: 0,
            floor_normal: Vec3::new(0.0, -1.0, 0.0),
            trigger_fall: false,
            lower_anim_finished: false,
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
    pub(crate) allowed_states: u32,
    /// Footstep phase counter (0.0 to 1.0 cycle).
    footstep_phase: f32,
    /// Minimum inter-step debounce timer in seconds (`SH2:EE DoubleFootstepFix.cpp` mitigation).
    footstep_cooldown: f32,
    /// Elapsed 30 fps frame counter while in `LocomotionState::WallF` (`lower_wall_f_3d` `0x0011a170`).
    pub(crate) wall_f_frame: f32,
    /// Elapsed 30 fps frame counter while in `LocomotionState::Fall` (`lower_fall` `0x0021f5c0`).
    pub(crate) fall_frame: f32,
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
            footstep_cooldown: 0.0,
            wall_f_frame: 0.0,
            fall_frame: 0.0,
        }
    }

    /// Reset internal controller state.
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Check if transition to target state is currently valid (`lower_flg_on` `0x00106400`).
    #[inline]
    pub fn can_transition_to(&self, target: LocomotionState) -> bool {
        (self.allowed_states & state_bit(target)) != 0
    }

    /// Enter a new locomotion state and initialize starting entry velocity (`0x54e0f0`).
    pub(crate) fn enter_state(&mut self, new_state: LocomotionState) {
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
            LocomotionState::WallF => {
                self.wall_f_frame = 0.0;
            }
            LocomotionState::Fall => {
                self.fall_frame = 0.0;
            }
            _ => {}
        }
    }

    /// Accelerate or decelerate `speed_mps` toward target at specified rates.
    pub(crate) fn ramp_speed(&mut self, target: f32, accel: f32, decel: f32, dt: f32) {
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

        // 2. State Resolution & Permissions (`PlayerUpdateStatusLower3D` 0x0011a430)
        let sl = input.strafe_left;
        let sr = input.strafe_right;
        let quick_turn = input.quick_turn_pressed || (sl && sr);
        let can_run = run_held && !input.wall_ahead && !input.is_great_knife && !input.is_aiming;

        let mut desired_state = None;

        // `PlayerSetHeight` (`0x00107aa0`) Fall trigger (> 250 mm drop)
        if input.trigger_fall && self.state != LocomotionState::Fall {
            desired_state = Some(LocomotionState::Fall);
        } else if self.state == LocomotionState::Fall {
            // `lower_fall` (`0x0021f5c0`): exits to Stand when `(_DAT_003c7f70 & 1) != 0` (`lower_anim_finished`)
            if input.lower_anim_finished {
                desired_state = Some(LocomotionState::Stand);
            } else {
                desired_state = Some(LocomotionState::Fall);
            }
        } else if self.state == LocomotionState::WallF {
            // `lower_wall_f_3d` (`0x0011a170`): exits to Stand when `(_DAT_003c7f70 & 1) != 0` (`lower_anim_finished`)
            if input.lower_anim_finished {
                desired_state = Some(LocomotionState::Stand);
            } else {
                desired_state = Some(LocomotionState::WallF);
            }
        } else if matches!(
            self.state,
            LocomotionState::Run2
                | LocomotionState::Run3
                | LocomotionState::StrafeLeftRun
                | LocomotionState::StrafeRightRun
        ) && input.wall_head_hit
            && input.wall_normal_dot <= WALL_F_NORMAL_DOT_THRESHOLD
            && self.speed_mps >= WALL_F_MIN_SPEED_MPS
        {
            // Head-on sprint collision -> WallF recoil (`0x0011a430` line 2416)
            desired_state = Some(LocomotionState::WallF);
        } else if self.state.is_quick_turn() {
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
            if can_run {
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
            if can_run && self.can_transition_to(run_s) {
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

        // 3. Execute State Dynamics & Speeds (`m3_play_3d.c` / `m3_play_common.c`)
        let dyn_out =
            self.execute_state_dynamics(input, current_yaw, dt, tx, ty, run_held, stamina_limit);
        let target_offset = dyn_out.target_offset;
        let offset_ease_rate = dyn_out.offset_ease_rate;
        let snap_offset_directly = dyn_out.snap_offset_directly;
        let yaw_rate = dyn_out.yaw_rate;
        let mut effective_speed_mps = dyn_out.effective_speed_mps;

        // Apply turning rate in Tank controls
        if is_tank && turn_dir != 0.0 && !self.state.is_quick_turn() {
            *current_yaw += turn_dir * yaw_rate * dt;
        }

        // Direction-offset easing: 2.0 rad/s for walk/strafe, 4.0 rad/s for Run1/2/3 (`m3_play_3d.c`)
        if snap_offset_directly {
            self.direction_offset = target_offset;
        } else if self.state.is_moving() && self.prev_state.is_moving() {
            let diff = wrap_angle(target_offset - self.direction_offset);
            let max_ease = offset_ease_rate * dt;
            self.direction_offset += diff.clamp(-max_ease, max_ease);
        } else if self.state.is_moving() {
            self.direction_offset = target_offset;
        }

        *current_yaw = wrap_angle(*current_yaw);

        // Water wading 0.65x speed scalar (`PlayerUpdatePosition3D` 0x0011c340 & `2D` 0x001153d0)
        if input.in_water {
            effective_speed_mps *= WATER_SPEED_MULTIPLIER;
        }

        // Calculate final translation velocity vector in world space
        let speed_multiplier = if input.is_maria_follower {
            MARIA_COMPANION_SPEED_MULTIPLIER
        } else {
            1.0
        };
        let final_speed_units_per_sec =
            effective_speed_mps * SH2_WORLD_UNIT_SCALE * speed_multiplier;
        let move_angle = *current_yaw + self.direction_offset;

        // Slope cos^2 scaling (`PlayerUpdatePosition3D` 0x0011c340 lines 3039-3042)
        let cos_nx = input.floor_normal.x.cos();
        let cos_nz = input.floor_normal.z.cos();
        let delta_x = move_angle.sin() * final_speed_units_per_sec * dt * cos_nx * cos_nx;
        let delta_z = move_angle.cos() * final_speed_units_per_sec * dt * cos_nz * cos_nz;
        let delta_position = Vec3::new(delta_x, 0.0, delta_z);

        // Footstep timing & acoustic volume (`sh2pc.exe` 0x54A9A0 / 0x5495C0 24-frame gait cycle
        // with heel strikes at relative frames 5 and 17, plus SH2:EE DoubleFootstepFix.cpp debounce)
        self.footstep_cooldown = (self.footstep_cooldown - dt).max(0.0);
        let mut footstep_triggered = false;
        let footstep_volume = if self.state.is_running() { 0.8 } else { 0.5 };

        if self.state.is_moving() && self.speed_mps > 0.1 {
            // Compute effective playback FPS matching `LocomotionAnimationSync` (`0x54c186` / `0x6ceac4`).
            // Every movement clip (`Walk`, `Back`, `StrafeWalk`, `Run1..3`, `StrafeRun`) is a 24-frame
            // full stride cycle with two heel strikes per cycle (at frames 5 and 17, i.e., every 12 frames).
            let effective_anim_fps = match self.state {
                LocomotionState::Walk => {
                    let ty = (self.speed_mps / 1.5).clamp(0.0, 1.0);
                    (512.0 + 896.0 * ty) / 2048.0 * 30.0
                }
                LocomotionState::Back => {
                    let ty = (self.speed_mps / 1.2).clamp(0.0, 1.0);
                    (512.0 + 1024.0 * ty) / 2048.0 * 30.0
                }
                LocomotionState::StrafeLeftWalk | LocomotionState::StrafeRightWalk => 22.5,
                LocomotionState::Run1 => 22.5,
                LocomotionState::Run2 => 26.25,
                LocomotionState::Run3
                | LocomotionState::StrafeLeftRun
                | LocomotionState::StrafeRightRun => 30.0,
                _ => 20.625,
            };
            let stride_freq = effective_anim_fps / 24.0;
            let prev_phase = self.footstep_phase;
            self.footstep_phase += dt * stride_freq;

            // Trigger footsteps at the two authentic heel-strike phases of the 24-frame cycle:
            // frame 5 / 24 (0.20833) and frame 17 / 24 (0.70833).
            const HEEL_STRIKE_1: f32 = 5.0 / 24.0;
            const HEEL_STRIKE_2: f32 = 17.0 / 24.0;

            let crossed_heel_strike = (prev_phase < HEEL_STRIKE_1
                && self.footstep_phase >= HEEL_STRIKE_1)
                || (prev_phase < HEEL_STRIKE_2 && self.footstep_phase >= HEEL_STRIKE_2)
                || (self.footstep_phase >= 1.0 + HEEL_STRIKE_1);

            if self.footstep_phase >= 1.0 {
                self.footstep_phase %= 1.0;
            }

            if crossed_heel_strike && self.footstep_cooldown <= 0.0 {
                footstep_triggered = true;
                self.footstep_cooldown = MIN_FOOTSTEP_INTERVAL_SECS;
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
