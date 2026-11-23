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

/// Maria companion follower speed multiplier (`0x54ef10`):
/// When Maria acts as an NPC companion follower following James, she moves at 80% speed.
/// When Maria is the active player (e.g. *Born From a Wish* scenario), full 1.0x player speed is used.
pub const MARIA_COMPANION_SPEED_MULTIPLIER: f32 = 0.8;

/// Quick turn 180° angular velocity in radians per second ($5.4977875\text{ rad/s}$, completing $180^\circ$ in $\approx 0.57\text{ s}$).
///
/// Ground truth: `SLUS_202.28` `lower_lturn180_3d` (`0x001196c0`) / `lower_rturn180_3d` (`0x00119860`).
pub const QUICK_TURN_SPEED_RAD_PER_SEC: f32 = 5.497_787_5;

/// Quick turn 180° angular velocity when carrying the Great Knife (`weapon == 8`, `4.712389 rad/s` = `3*PI/2`).
///
/// Ground truth: `SLUS_202.28` `lower_lturn180_3d` (`0x001196c0`) / `lower_rturn180_3d` (`0x00119860`).
pub const GREAT_KNIFE_QUICK_TURN_SPEED_RAD_PER_SEC: f32 = 4.712_389;

/// Water wading linear speed multiplier (`0.65x`).
///
/// Ground truth: `SLUS_202.28` `PlayerUpdatePosition3D` (`0x0011c340`) & `PlayerUpdatePosition2D` (`0x001153d0`).
pub const WATER_SPEED_MULTIPLIER: f32 = 0.65;

/// Water wading turning yaw-rate multiplier (`1.3x`).
///
/// Ground truth: `SLUS_202.28` `lower_walk_3d` (`0x00116630`) & `lower_back_3d` (`0x00116c70`).
pub const WATER_TURN_MULTIPLIER: f32 = 1.3;

/// Direction-offset easing rate (`2.0 rad/s`) for `Walk`, `StrafeWalk`, and `StrafeRun`.
///
/// Ground truth: `SLUS_202.28` `lower_walk_3d` (`0x00116630`), `lower_lswalk_3d` (`0x00117320`),
/// `lower_rswalk_3d` (`0x001178f0`), `lower_lsrun_3d` (`0x00118f20`), `lower_rsrun_3d` (`0x001192f0`).
pub const DIRECTION_OFFSET_EASE_WALK_RAD_PER_SEC: f32 = 2.0;

/// Direction-offset easing rate (`4.0 rad/s`) for `Run1`, `Run2`, and `Run3`.
///
/// Ground truth: `SLUS_202.28` `lower_run1_3d` (`0x00117ec0`), `lower_run2_3d` (`0x001183a0`),
/// `lower_run3_3d` (`0x001189d0`).
pub const DIRECTION_OFFSET_EASE_RUN_RAD_PER_SEC: f32 = 4.0;

/// `WallF` recoil trigger dot-product threshold (`<= -0.85`).
///
/// Ground truth: `SLUS_202.28` `PlayerUpdateStatusLower3D` (`0x0011a430` at line 2416).
pub const WALL_F_NORMAL_DOT_THRESHOLD: f32 = -0.85;

/// `WallF` recoil trigger minimum running speed (`>= 3.0 m/s`).
///
/// Ground truth: `SLUS_202.28` `PlayerUpdateStatusLower3D` (`0x0011a430` at line 2417).
pub const WALL_F_MIN_SPEED_MPS: f32 = 3.0;

/// Great Knife (`weapon == 8`) 24-frame forward walk speed table (`m/s`).
///
/// Ground truth: `SLUS_202.28` `lower_walk_3d_nata` (`0x00115d30`, `.rodata` at `0x002a9060`).
pub const GREAT_KNIFE_WALK_SPEEDS: [f32; 24] = [
    0.9, 1.05, 0.45, 0.045, 0.3, 0.9, 1.05, 1.2, 1.425, 1.5, 1.425, 1.35, 0.975, 0.6, 0.15, 0.045,
    0.3, 0.6, 0.9, 0.975, 1.05, 1.2, 1.35, 1.05,
];

/// Great Knife (`weapon == 8`) 24-frame backward walk speed table (`m/s`).
///
/// Ground truth: `SLUS_202.28` `lower_back_3d_nata` (`0x00115e50`, `.rodata` at `0x002a90f0`).
pub const GREAT_KNIFE_BACK_SPEEDS: [f32; 24] = [
    0.9, 1.05, 1.2, 0.0, 0.3, 0.9, 1.05, 1.2, 1.35, 1.425, 1.35, 1.2, 0.975, 0.75, 0.525, 0.075,
    0.0, 0.3, 0.45, 0.675, 1.35, 1.2, 1.35, 1.05,
];

/// Great Knife (`weapon == 8`) 24-frame strafe walk speed table (`m/s`).
///
/// Ground truth: `SLUS_202.28` `lower_lswalk_3d_nata` (`0x00115f70`, `0x002a9180`) &
/// `lower_rswalk_3d_nata` (`0x00116090`, `0x002a9210`).
pub const GREAT_KNIFE_STRAFE_SPEEDS: [f32; 24] = [
    0.9, 1.05, 0.45, 0.15, 0.0, 0.9, 1.05, 1.2, 1.425, 1.5, 1.425, 1.35, 0.975, 0.75, 0.525, 0.075,
    0.0, 0.0, 0.225, 0.675, 0.9, 1.2, 1.35, 1.05,
];

/// Minimum time interval in seconds between consecutive footstep triggers (`SH2:EE DoubleFootstepFix.cpp` mitigation).
///
/// Prevents the double-footstep glitch when transitioning rapidly between `Stand`, `Walk`, and `Run`
/// or when gait phase crosses a threshold on adjacent animation transition frames.
pub const MIN_FOOTSTEP_INTERVAL_SECS: f32 = 0.20;

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
    /// Wall collision recoil stumble after sprinting head-on into a wall (`lower_wall_f_3d` `0x0011a170`).
    WallF = 17,
    /// Freefall / drop-off state triggered when floor drops > 250 mm (`lower_fall` `0x0021f5c0`).
    Fall = 18,
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
        LocomotionState::WallF | LocomotionState::Fall => state_bit(LocomotionState::Stand),
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
    /// Minimum inter-step debounce timer in seconds (`SH2:EE DoubleFootstepFix.cpp` mitigation).
    footstep_cooldown: f32,
    /// Elapsed 30 fps frame counter while in `LocomotionState::WallF` (`lower_wall_f_3d` `0x0011a170`).
    wall_f_frame: f32,
    /// Elapsed 30 fps frame counter while in `LocomotionState::Fall` (`lower_fall` `0x0021f5c0`).
    fall_frame: f32,
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
            // Remain in Fall until animation completes (`< 21` frames at 30 fps) or `trigger_fall` clears
            if self.fall_frame < 21.0 {
                desired_state = Some(LocomotionState::Fall);
            } else {
                desired_state = Some(LocomotionState::Stand);
            }
        } else if self.state == LocomotionState::WallF {
            // Remain in WallF recoil stumble until 15 frames (30 fps) elapse (`lower_wall_f_3d` 0x0011a170)
            if self.wall_f_frame < 15.0 {
                desired_state = Some(LocomotionState::WallF);
            } else {
                desired_state = Some(LocomotionState::Stand);
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
        let target_offset;
        let mut offset_ease_rate = DIRECTION_OFFSET_EASE_WALK_RAD_PER_SEC;
        let mut snap_offset_directly = false;
        let mut yaw_rate = 0.0f32;
        let mut effective_speed_mps;
        let frame_idx = (input.anim_frame as usize) % 24;

        match self.state {
            LocomotionState::Stand
            | LocomotionState::Relax
            | LocomotionState::Tired
            | LocomotionState::Alert => {
                self.still_duration += dt;
                self.stamina_frames = (self.stamina_frames - dt * 30.0).max(0.0);
                // Halting deceleration (`PlayerSpeedDownToStand` 0x0021c560): 20 m/s^2 post-run, 10 m/s^2 normally
                let decel = if self.prev_state.is_running() {
                    20.0
                } else {
                    10.0
                };
                self.speed_mps = (self.speed_mps - decel * dt).max(0.0);
                effective_speed_mps = self.speed_mps;
                target_offset = self.direction_offset;
            }
            LocomotionState::TurnLeft | LocomotionState::TurnRight => {
                let decel = if self.prev_state.is_running() {
                    20.0
                } else {
                    10.0
                };
                self.speed_mps = (self.speed_mps - decel * dt).max(0.0);
                effective_speed_mps = self.speed_mps;
                // `lower_lround_3d` (0x00116330) / `lower_lround_3d_nata` (0x00115c10)
                if input.is_great_knife {
                    yaw_rate = if input.is_digital_input { 1.0 } else { tx };
                } else {
                    let base = if input.is_digital_input {
                        2.4
                    } else {
                        2.4 * tx
                    };
                    let run_multiplier = if run_held { 1.5 } else { 1.0 };
                    yaw_rate = base * run_multiplier;
                }
                target_offset = self.direction_offset;
            }
            LocomotionState::Walk => {
                // `lower_walk_3d` (0x00116630) / `lower_walk_3d_nata` (0x00115d30)
                if input.is_great_knife {
                    self.speed_mps = GREAT_KNIFE_WALK_SPEEDS[frame_idx];
                } else if input.is_digital_input {
                    if self.speed_mps > 0.0 && self.prev_state == LocomotionState::Back {
                        self.speed_mps = (self.speed_mps - 7.5 * dt).max(0.0);
                    } else {
                        self.ramp_speed(1.5, 2.5, 5.0, dt);
                    }
                } else {
                    self.speed_mps = ty * 1.5;
                }

                // Aim + Run speed boost (`0x00116630` lines 831-848: 1.5x capped at 2.25 m/s)
                effective_speed_mps = if input.is_aiming && run_held {
                    (self.speed_mps * 1.5).min(2.25)
                } else {
                    self.speed_mps
                };

                if !input.is_aiming {
                    yaw_rate = if input.is_great_knife {
                        if self.speed_mps > 0.5 {
                            0.875
                        } else {
                            0.0
                        }
                    } else if input.is_digital_input {
                        1.9
                    } else {
                        1.9 * tx
                    };
                    if input.in_water {
                        yaw_rate *= WATER_TURN_MULTIPLIER;
                    }
                }
                target_offset = 0.0;
                offset_ease_rate = DIRECTION_OFFSET_EASE_WALK_RAD_PER_SEC;
                self.stamina_frames = (self.stamina_frames - dt * 15.0).max(0.0);
            }
            LocomotionState::Back => {
                // `lower_back_3d` (0x00116c70) / `lower_back_3d_nata` (0x00115e50)
                if input.is_great_knife {
                    self.speed_mps = GREAT_KNIFE_BACK_SPEEDS[frame_idx];
                } else if input.is_digital_input {
                    if self.speed_mps > 0.0 && self.prev_state != LocomotionState::Back {
                        self.speed_mps = (self.speed_mps - 10.0 * dt).max(0.0);
                        if self.speed_mps == 0.0 {
                            self.prev_state = LocomotionState::Back;
                        }
                    } else {
                        self.ramp_speed(1.2, 5.0, 5.0, dt);
                    }
                } else {
                    self.speed_mps = ty * 1.2;
                }

                // Back + Run speed boost (`0x00116c70` lines 977-992: 1.5x capped at 1.8 m/s)
                effective_speed_mps = if run_held {
                    (self.speed_mps * 1.5).min(1.800_000_1)
                } else {
                    self.speed_mps
                };

                if !input.is_aiming {
                    yaw_rate = if input.is_great_knife {
                        if self.speed_mps > 0.5 {
                            0.65
                        } else {
                            0.0
                        }
                    } else if input.is_digital_input {
                        1.3
                    } else {
                        1.3 * tx
                    };
                    if input.in_water {
                        yaw_rate *= WATER_TURN_MULTIPLIER;
                    }
                }
                target_offset = -PI;
                snap_offset_directly = true;
                self.stamina_frames = (self.stamina_frames - dt * 15.0).max(0.0);
            }
            LocomotionState::StrafeLeftWalk | LocomotionState::StrafeRightWalk => {
                // `lower_lswalk_3d` (0x00117320) / `lower_rswalk_3d` (0x001178f0) / `*_nata`
                if input.is_great_knife {
                    self.speed_mps = GREAT_KNIFE_STRAFE_SPEEDS[frame_idx];
                } else if self.speed_mps > 0.0 && self.prev_state == LocomotionState::Back {
                    // Reversal deceleration from Back (`0x00117320` line 1097: -5.1 m/s^2)
                    self.speed_mps = (self.speed_mps - 5.100_000_4 * dt).max(0.0);
                } else {
                    self.ramp_speed(1.3, 1.7, 3.4, dt);
                }

                // Aim + Run speed boost (`0x00117320` lines 1105-1116: 1.5x capped at 1.8 m/s)
                effective_speed_mps = if input.is_aiming && run_held {
                    (self.speed_mps * 1.5).min(1.800_000_1)
                } else {
                    self.speed_mps
                };

                if !input.is_aiming {
                    yaw_rate = if input.is_great_knife {
                        if self.speed_mps > 0.5 {
                            0.875
                        } else {
                            0.0
                        }
                    } else if input.is_digital_input {
                        1.9
                    } else {
                        1.9 * tx
                    };
                }
                target_offset = if self.state == LocomotionState::StrafeLeftWalk {
                    -PI * 0.5
                } else {
                    PI * 0.5
                };
                offset_ease_rate = DIRECTION_OFFSET_EASE_WALK_RAD_PER_SEC;
            }
            LocomotionState::Run1 | LocomotionState::Run2 | LocomotionState::Run3 => {
                // `lower_run1_3d` (0x00117ec0), `lower_run2_3d` (0x001183a0), `lower_run3_3d` (0x001189d0)
                let top_speeds = [3.5f32, 4.0f32, 4.8f32];
                let idx = match self.state {
                    LocomotionState::Run1 => 0,
                    LocomotionState::Run2 => 1,
                    _ => 2,
                };
                let target = if input.is_digital_input {
                    top_speeds[idx]
                } else {
                    top_speeds[idx] * ty
                };

                if self.state == LocomotionState::Run1 {
                    // Run1 (`0x00117ec0`): ZERO stamina drain!
                    self.ramp_speed(target, 3.0, 3.0, dt);
                    if !input.is_digital_input {
                        // Analog Run1 floor 2.0, then capped at target (`0x00117ec0` lines 1328-1335)
                        if self.speed_mps < 2.0 {
                            self.speed_mps = 2.0;
                        }
                        if self.speed_mps > target {
                            self.speed_mps = target;
                        }
                    }
                    if self.speed_mps >= 3.5 {
                        self.allowed_states = (self.allowed_states
                            | state_bit(LocomotionState::Run2))
                            & !state_bit(LocomotionState::Run1);
                    }
                } else if self.state == LocomotionState::Run2 {
                    // Run2 (`0x001183a0`): drains stamina only when Ampoule is NOT active
                    if !input.ampoule_active {
                        self.stamina_frames += dt * 30.0;
                    }
                    self.ramp_speed(target, 3.0, 3.0, dt);
                    // Analog speed-drop downgrade Run2 -> Run1 (`0x001183a0` line 1465)
                    if !input.is_digital_input && self.speed_mps <= 3.5 {
                        self.allowed_states = (self.allowed_states
                            | state_bit(LocomotionState::Run1))
                            & !state_bit(LocomotionState::Run2);
                    }
                    // Upgrade Run2 -> Run3 (`0x001183a0` lines 1469, 1504): requires `!in_water`
                    if self.speed_mps >= 4.0
                        && self.stamina_frames < stamina_limit
                        && !input.in_water
                    {
                        self.allowed_states = (self.allowed_states
                            | state_bit(LocomotionState::Run3))
                            & !state_bit(LocomotionState::Run2);
                    }
                } else {
                    // Run3 (`0x001189d0`): drains stamina only when Ampoule is NOT active
                    if !input.ampoule_active {
                        self.stamina_frames += dt * 30.0;
                    }
                    self.ramp_speed(target, 3.0, 3.0, dt);
                    // Downgrade Run3 -> Run2 on stamina exhaustion OR analog speed drop <= 4.0 (`0x001189d0` lines 1591, 1615)
                    if self.stamina_frames >= stamina_limit
                        || (!input.is_digital_input && self.speed_mps <= 4.0)
                    {
                        self.allowed_states = (self.allowed_states
                            | state_bit(LocomotionState::Run2))
                            & !state_bit(LocomotionState::Run3);
                    }
                }

                effective_speed_mps = self.speed_mps;
                if !input.is_aiming {
                    yaw_rate = if input.is_digital_input {
                        1.7
                    } else {
                        1.7 * tx
                    };
                }
                target_offset = 0.0;
                offset_ease_rate = DIRECTION_OFFSET_EASE_RUN_RAD_PER_SEC;
            }
            LocomotionState::StrafeLeftRun | LocomotionState::StrafeRightRun => {
                // `lower_lsrun_3d` (0x00118f20) / `lower_rsrun_3d` (0x001192f0)
                if !input.ampoule_active {
                    self.stamina_frames += dt * 30.0;
                }
                self.ramp_speed(3.8, 6.0, 6.0, dt);
                effective_speed_mps = self.speed_mps;
                if !input.is_aiming {
                    yaw_rate = if input.is_digital_input {
                        1.7
                    } else {
                        1.7 * tx
                    };
                }
                target_offset = if self.state == LocomotionState::StrafeLeftRun {
                    -PI * 0.5
                } else {
                    PI * 0.5
                };
                offset_ease_rate = DIRECTION_OFFSET_EASE_WALK_RAD_PER_SEC;
            }
            LocomotionState::QuickTurnLeft | LocomotionState::QuickTurnRight => {
                // `lower_lturn180_3d` (0x001196c0) / `lower_rturn180_3d` (0x00119860)
                self.speed_mps = 0.0;
                effective_speed_mps = 0.0;
                let qt_speed = if input.is_great_knife {
                    GREAT_KNIFE_QUICK_TURN_SPEED_RAD_PER_SEC
                } else {
                    QUICK_TURN_SPEED_RAD_PER_SEC
                };
                let step = qt_speed * dt;
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
                snap_offset_directly = true;
            }
            LocomotionState::WallF => {
                // `lower_wall_f_3d` (`0x0011a170`):
                // Frame 4 or > 9 -> speed = 0.0; Frames 5..=9 -> recoil opposite entry at +5.0 m/s^2 up to 1.2 m/s.
                let wf_frame = self.wall_f_frame as u32;
                if wf_frame == 4 || wf_frame > 9 {
                    self.speed_mps = 0.0;
                    target_offset = self.direction_offset;
                } else if (5..=9).contains(&wf_frame) {
                    target_offset = match self.prev_state {
                        LocomotionState::StrafeLeftRun => PI * 0.5,
                        LocomotionState::StrafeRightRun => -PI * 0.5,
                        _ => -PI,
                    };
                    snap_offset_directly = true;
                    self.speed_mps = (self.speed_mps + 5.0 * dt).min(1.2);
                } else {
                    target_offset = self.direction_offset;
                }
                effective_speed_mps = self.speed_mps;
                self.wall_f_frame += dt * 30.0;
            }
            LocomotionState::Fall => {
                // `lower_fall` (`0x0021f5c0`): frames < 21 ramp toward 0.9 m/s (+2.5 m/s^2) or decel if > 0.9
                if (self.fall_frame as u32) < 21 {
                    if self.speed_mps > 0.9 {
                        let decel = if self.prev_state.is_running() {
                            20.0
                        } else {
                            10.0
                        };
                        self.speed_mps = (self.speed_mps - decel * dt).max(0.0);
                    } else {
                        self.speed_mps = (self.speed_mps + 2.5 * dt).min(0.9);
                    }
                } else {
                    self.speed_mps = 0.0;
                }
                effective_speed_mps = self.speed_mps;
                target_offset = self.direction_offset;
                self.fall_frame += dt * 30.0;
            }
        }

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

        // 3. Playable Maria vs Follower Maria speed test
        controller.reset();
        input.stick_y = 1.0;
        input.is_maria_follower = false;
        let out_maria_playable = controller.update(&input, &mut yaw, dt, true);
        assert_eq!(out_maria_playable.state, LocomotionState::Walk);
        // Playable Maria has full 1.0x player speed (750 units/s)
        assert!((out_maria_playable.linear_speed - 750.0).abs() < 1e-3);

        // Follower Maria has 0.8x companion multiplier (600 units/s)
        controller.reset();
        input.is_maria_follower = true;
        let out_maria_follower = controller.update(&input, &mut yaw, dt, true);
        assert_eq!(out_maria_follower.state, LocomotionState::Walk);
        assert!((out_maria_follower.linear_speed - 600.0).abs() < 1e-3);
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

        // First tick enters Run1 and clamps speed to the 2.0 m/s PS2 floor (0x54cfd0)
        let out = controller.update(&input_run, &mut yaw, dt, true);
        assert_eq!(out.state, LocomotionState::Run1);
        assert!((controller.speed_mps - 2.0).abs() < 1e-3);
        assert!((out.linear_speed - 1000.0).abs() < 1e-3);

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

    #[test]
    fn test_sh2_61_double_footstep_glitch_mitigation() {
        let mut controller = PlayerLocomotionController::new();
        let mut yaw = 0.0f32;
        let dt = 1.0 / 60.0;

        // Simulate rapid toggling between Walk and Run1 while moving forward
        let mut last_step_tick: Option<u32> = None;
        let mut step_count = 0u32;

        for tick in 0..180u32 {
            let input = LocomotionInput {
                stick_y: 1.0,
                run_held: (tick / 3) % 2 == 0,
                ..Default::default()
            };
            let out = controller.update(&input, &mut yaw, dt, true);
            if out.footstep_triggered {
                if let Some(prev) = last_step_tick {
                    let elapsed_secs = (tick - prev) as f32 * dt;
                    assert!(
                        elapsed_secs + 1e-4 >= MIN_FOOTSTEP_INTERVAL_SECS,
                        "Double-footstep glitch detected! Two footsteps triggered {}s apart (< {}s)",
                        elapsed_secs,
                        MIN_FOOTSTEP_INTERVAL_SECS
                    );
                }
                last_step_tick = Some(tick);
                step_count += 1;
            }
        }
        assert!(
            step_count >= 5,
            "Expected footsteps during 3s walk/run movement"
        );
    }

    #[test]
    fn test_sh2_61_authentic_walk_and_run_footstep_cadence() {
        let mut controller = PlayerLocomotionController::new();
        let mut yaw = 0.0f32;
        let dt = 1.0 / 60.0;

        // 1. Full-speed forward Walk (1.5 m/s -> 20.625 FPS on 24-frame cycle = 1.71875 steps/sec, ~0.582s interval).
        // Over 600 ticks (10.0 seconds), we expect ~17 footsteps (specifically 17 steps: 10.0 * 20.625 / 12.0 = 17.1875).
        let walk_input = LocomotionInput {
            stick_y: 1.0,
            run_held: false,
            ..Default::default()
        };
        let mut walk_steps = Vec::new();
        for tick in 0..600u32 {
            let out = controller.update(&walk_input, &mut yaw, dt, true);
            if out.footstep_triggered {
                walk_steps.push(tick as f32 * dt);
            }
        }
        assert!(
            (16..=18).contains(&walk_steps.len()),
            "Full walk over 10s should trigger ~17 footsteps (1.72 steps/s), got {}",
            walk_steps.len()
        );
        for pair in walk_steps.windows(2) {
            let interval = pair[1] - pair[0];
            assert!(
                (interval - (12.0 / 20.625)).abs() < 0.03,
                "Walk step interval should be ~0.582s (12 frames @ 20.625 FPS), got {interval:.3}s"
            );
        }

        // 2. Half-speed forward Walk (stick_y = 0.5 -> slower cadence than full walk)
        controller.reset();
        let slow_walk_input = LocomotionInput {
            stick_y: 0.5,
            run_held: false,
            ..Default::default()
        };
        let mut slow_walk_steps = 0usize;
        for _ in 0..600u32 {
            let out = controller.update(&slow_walk_input, &mut yaw, dt, true);
            if out.footstep_triggered {
                slow_walk_steps += 1;
            }
        }
        assert!(
            slow_walk_steps < walk_steps.len(),
            "Half-tilt walk ({slow_walk_steps} steps) must have slower cadence than full walk ({} steps)",
            walk_steps.len()
        );
    }

    #[test]
    fn test_sh2_132_direction_offset_easing_run1_stamina_and_downgrades() {
        let mut ctrl = PlayerLocomotionController::new();
        let mut yaw = 0.0f32;
        let dt = 1.0 / 60.0;

        // 1. Run1 has ZERO stamina drain (`lower_run1_3d` 0x00117ec0)
        let run_in = LocomotionInput {
            stick_y: 1.0,
            run_held: true,
            ..Default::default()
        };
        for _ in 0..25 {
            ctrl.update(&run_in, &mut yaw, dt, true);
        }
        assert_eq!(ctrl.state, LocomotionState::Run1);
        assert_eq!(
            ctrl.stamina_frames, 0.0,
            "Run1 must NOT drain stamina (SLUS 0x00117ec0)"
        );

        // 2. Continue accelerating into Run2 -> drains stamina unless Ampoule is active
        let ampoule_run = LocomotionInput {
            ampoule_active: true,
            ..run_in
        };
        for _ in 0..40 {
            ctrl.update(&ampoule_run, &mut yaw, dt, true);
        }
        assert!(matches!(
            ctrl.state,
            LocomotionState::Run2 | LocomotionState::Run3
        ));
        assert_eq!(
            ctrl.stamina_frames, 0.0,
            "Ampoule must prevent stamina drain in Run2/Run3"
        );

        // 3. Analog speed-drop downgrade Run3 -> Run2 -> Run1
        let half_run = LocomotionInput {
            stick_y: 0.5,
            run_held: true,
            ..Default::default()
        };
        for _ in 0..60 {
            ctrl.update(&half_run, &mut yaw, dt, true);
        }
        assert_eq!(
            ctrl.state,
            LocomotionState::Run1,
            "Analog speed drop below 3.5 m/s must downgrade Run3 -> Run2 -> Run1"
        );

        // 4. Direction-offset easing: Walk eases at 2.0 rad/s (vs 4.0 rad/s in Run1)
        ctrl.reset();
        ctrl.update(
            &LocomotionInput {
                strafe_right: true,
                ..Default::default()
            },
            &mut yaw,
            dt,
            true,
        );
        assert!((ctrl.direction_offset - PI * 0.5).abs() < 1e-4);
        ctrl.update(
            &LocomotionInput {
                stick_y: 1.0,
                ..Default::default()
            },
            &mut yaw,
            dt,
            true,
        );
        let expected_walk_offset = PI * 0.5 - 2.0 * dt;
        assert!(
            (ctrl.direction_offset - expected_walk_offset).abs() < 1e-4,
            "Walk must ease direction_offset at 2.0 rad/s; got {}",
            ctrl.direction_offset
        );
    }

    #[test]
    fn test_sh2_132_strafe_reversal_digital_back_run_and_aim_run() {
        let mut ctrl = PlayerLocomotionController::new();
        let mut yaw = 0.0f32;
        let dt = 1.0 / 60.0;

        // 1. Back + Run boosts output speed to 1.8 m/s (900 units/s)
        let back_run = LocomotionInput {
            stick_y: -1.0,
            run_held: true,
            ..Default::default()
        };
        let out_br = ctrl.update(&back_run, &mut yaw, dt, true);
        assert_eq!(out_br.state, LocomotionState::Back);
        assert!((out_br.linear_speed - 900.0).abs() < 1e-2);

        // 2. Aim + Run in Walk boosts output speed to 2.25 m/s (1125 units/s)
        ctrl.reset();
        let aim_walk_run = LocomotionInput {
            stick_y: 1.0,
            run_held: true,
            is_aiming: true,
            ..Default::default()
        };
        let out_awr = ctrl.update(&aim_walk_run, &mut yaw, dt, true);
        assert_eq!(out_awr.state, LocomotionState::Walk);
        assert!((out_awr.linear_speed - 1125.0).abs() < 1e-2);

        // 3. StrafeWalk reversal from Back decelerates at 5.1 m/s^2 (`0x00117320`)
        ctrl.reset();
        ctrl.state = LocomotionState::StrafeLeftWalk;
        ctrl.prev_state = LocomotionState::Back;
        ctrl.speed_mps = 1.0;
        ctrl.update(
            &LocomotionInput {
                strafe_left: true,
                ..Default::default()
            },
            &mut yaw,
            dt,
            true,
        );
        assert!((ctrl.speed_mps - (1.0 - 5.100_000_4 * dt)).abs() < 1e-4);

        // 4. Digital Walk ramps at +2.5 m/s^2 from 0.6 entry speed instead of instant 1.5
        ctrl.reset();
        ctrl.update(
            &LocomotionInput {
                stick_y: 1.0,
                is_digital_input: true,
                ..Default::default()
            },
            &mut yaw,
            dt,
            true,
        );
        assert!((ctrl.speed_mps - (0.6 + 2.5 * dt)).abs() < 1e-4);
    }

    #[test]
    fn test_sh2_132_water_slope_great_knife_and_wall_f_recoil() {
        let mut ctrl = PlayerLocomotionController::new();
        let mut yaw = 0.0f32;
        let dt = 1.0 / 60.0;

        // 1. Water wading: 0.65x speed (`PlayerUpdatePosition3D`) and 1.3x turn rate (`lower_walk_3d`)
        let water_walk = LocomotionInput {
            stick_y: 1.0,
            in_water: true,
            ..Default::default()
        };
        let out_w = ctrl.update(&water_walk, &mut yaw, dt, true);
        assert!((out_w.linear_speed - (750.0 * 0.65)).abs() < 1e-2);

        let water_turn = LocomotionInput {
            stick_y: 1.0,
            stick_x: 1.0,
            in_water: true,
            is_digital_input: true,
            ..Default::default()
        };
        let out_wt = ctrl.update(&water_turn, &mut yaw, dt, true);
        assert!((out_wt.turn_speed_rad_per_sec - (1.9 * 1.3)).abs() < 1e-2);

        // 2. Slope cos^2 scaling (`PlayerUpdatePosition3D` 0x0011c340)
        ctrl.reset();
        yaw = 0.0;
        let slope_angle = 0.5f32;
        let out_slope = ctrl.update(
            &LocomotionInput {
                stick_y: 1.0,
                floor_normal: Vec3::new(0.0, -slope_angle.cos(), slope_angle),
                ..Default::default()
            },
            &mut yaw,
            dt,
            true,
        );
        let expected_dz = 750.0 * dt * slope_angle.cos().powi(2);
        assert!((out_slope.delta_position.z - expected_dz).abs() < 1e-2);

        // 3. Great Knife (`*_nata`) uses 24-frame speed table, blocks Run, and uses 4.712389 rad/s quick turn
        ctrl.reset();
        yaw = 0.0;
        let nata_walk = LocomotionInput {
            stick_y: 1.0,
            run_held: true,
            is_great_knife: true,
            anim_frame: 3, // GREAT_KNIFE_WALK_SPEEDS[3] == 0.045 m/s
            ..Default::default()
        };
        let out_nata = ctrl.update(&nata_walk, &mut yaw, dt, true);
        assert_eq!(out_nata.state, LocomotionState::Walk);
        assert!((ctrl.speed_mps - 0.045).abs() < 1e-5);

        // 4. WallF recoil triggers when sprinting (>= 3.0 m/s) head-on (dot <= -0.85) into a wall
        ctrl.reset();
        ctrl.state = LocomotionState::Run2;
        ctrl.speed_mps = 3.8;
        let out_wf = ctrl.update(
            &LocomotionInput {
                stick_y: 1.0,
                run_held: true,
                wall_head_hit: true,
                wall_normal_dot: -0.95,
                ..Default::default()
            },
            &mut yaw,
            dt,
            true,
        );
        assert_eq!(out_wf.state, LocomotionState::WallF);
    }
}
