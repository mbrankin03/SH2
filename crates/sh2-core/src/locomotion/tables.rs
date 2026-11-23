//! Locomotion parity constants, Great Knife (`*_nata`) `.rodata` speed tables,
//! [`LocomotionState`] enum, and PS2 transition permission bitmasks (`0x6cf878`).

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
pub(crate) const fn state_bit(s: LocomotionState) -> u32 {
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

pub(crate) const fn get_allowed_transitions(state: LocomotionState) -> u32 {
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
