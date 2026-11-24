//! Rowboat input processing (`Chacter/m3_boat.c` `BoatCheckKeyInput` `0x0026c420`,
//! `sh2pc.exe` `0x0053eb10`).
//!
//! Supports:
//! - Classic PS2/PC 3D dual-stick circular rowing (`DAT_0122b384 != 0`)
//! - 2D / Beginner single-stick rowing (`DAT_0122b384 == 0`)
//! - Retail PC digital/keyboard ±7 input mapping (`0x0053eb10`)
//! - Toggleable `native-upgrade` L2/R2 analog trigger rowing (`enable_analog_trigger_rowing`)

use std::f32::consts::PI;

/// Stick angular history buffer length (`0x14` = `20` frames at `0x0026c170` / `0x0026c420`).
pub const STICK_HISTORY_LEN: usize = 20;

/// Maximum per-tick stick power in 3D dual-stick circular mode (`±0.5` at `0x0026c420`).
pub const DUAL_STICK_POWER_CLAMP: f32 = 0.5;

/// Single-stick forward/backward projection weight (`0.5` at `0x0026c420` / `0x0053eb10`).
pub const SINGLE_STICK_COS_WEIGHT: f32 = 0.5;

/// Single-stick turn projection weight (`0.25` = `0.5 * 0.5` at `0x0026c420` / `0x0053eb10`).
pub const SINGLE_STICK_SIN_WEIGHT: f32 = 0.25;

/// Retail PC boat power multiplier when playing a valid rowing animation (`1.8` at `0x0053eb10`).
///
/// Prototype drift: `SLUS_202.28` (`0x0026c420`) uses `1.0` directly without the `1.8` scale.
pub const PC_ROWING_POWER_SCALE: f32 = 1.8;

/// Retail PC minimum boat animation ID for active rowing input (`0x11` = `17` at `0x0053eb10`).
pub const PC_BOAT_ANIM_MIN: i16 = 0x11;

/// Retail PC maximum boat animation ID for active rowing input (`0x4f` = `79` at `0x0053eb10`).
pub const PC_BOAT_ANIM_MAX: i16 = 0x4f;

/// Rotational power multiplier (`round_power = (l - r) * 2.0` at `0x0026c420` / `0x0053eb10`).
pub const ROUND_POWER_SCALE: f32 = 2.0;

/// Straight propulsion multiplier (`straight_power = (l + r) * 0.5` at `0x0026c420` / `0x0053eb10`).
pub const STRAIGHT_POWER_SCALE: f32 = 0.5;

/// Forward speed threshold forcing boat animation status to `Both` (`2.0` at `0x0026c420` / `0x0053eb10`).
pub const BOAT_STATUS_FORCE_BOTH_SPEED: f32 = 2.0;

/// Cooldown duration (seconds) after changing boat rowing animation status (`1.5` at `0x0026c420` / `0x0053eb10`).
pub const BOAT_STATUS_COOLDOWN_SECS: f32 = 1.5;

/// 3x3 rowing status transition table (`auStack_8` at `0x0026c420` / `local_24` at `0x0053eb10`).
///
/// Indexed by `[left_dir][right_dir]` where `0 = idle`, `1 = forward`, `2 = reverse`.
/// Outputs `0 = Both`, `1 = Right`, `2 = Left`.
pub const BOAT_STATUS_TABLE: [[u8; 3]; 3] = [[0, 1, 1], [2, 0, 1], [2, 1, 0]];

/// Rowing control scheme (`DAT_0122b384` in `m3_boat.c`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RowboatControlMode {
    /// Classic 3D dual-stick circular rowing (`DAT_0122b384 != 0`).
    #[default]
    DualStick3D,
    /// 2D / Beginner single-stick directional rowing (`DAT_0122b384 == 0`).
    SingleStick2D,
}

/// Active rowing animation mode (`DAT_01202554` / `DAT_01f96804`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BoatRowMode {
    /// Both oars rowing straight (`0`, `0x39a61c` / James `0x4e24`).
    #[default]
    Both = 0,
    /// Right oar turn (`1`, `0x39a628` / James `0x4e25`).
    Right = 1,
    /// Left oar turn (`2`, `0x39a634` / James `0x4e26`).
    Left = 2,
}

impl BoatRowMode {
    /// Converts a raw `0..=2` mode index from [`BOAT_STATUS_TABLE`].
    #[inline]
    pub const fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::Right,
            2 => Self::Left,
            _ => Self::Both,
        }
    }

    /// Corresponding James boat animation ID in `BoatPlayerCheckAnime` (`0x0010ec40`, `m3_play.c`).
    #[inline]
    pub const fn james_anim_id(self) -> u16 {
        match self {
            Self::Both => 0x4e24,
            Self::Right => 0x4e25,
            Self::Left => 0x4e26,
        }
    }
}

/// Per-frame raw input fed into [`RowboatInputProcessor`].
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct RowboatInput {
    /// Left analog stick X (`-1.0..=1.0`).
    pub left_stick_x: f32,
    /// Left analog stick Y (`-1.0..=1.0`, positive = up/forward).
    pub left_stick_y: f32,
    /// Right analog stick X (`-1.0..=1.0`).
    pub right_stick_x: f32,
    /// Right analog stick Y (`-1.0..=1.0`, positive = up/forward).
    pub right_stick_y: f32,
    /// Left analog trigger L2 (`0.0..=1.0`, used when `enable_analog_trigger_rowing` is true).
    pub left_trigger: f32,
    /// Right analog trigger R2 (`0.0..=1.0`, used when `enable_analog_trigger_rowing` is true).
    pub right_trigger: f32,
    /// Reverse modifier for analog trigger rowing (e.g. Cancel/L1/R1 held).
    pub trigger_reverse: bool,
    /// Control mode (`DualStick3D` vs `SingleStick2D`).
    pub control_mode: RowboatControlMode,
    /// Native-upgrade toggle: allow L2/R2 analog triggers to drive oars directly.
    pub enable_analog_trigger_rowing: bool,
    /// Retail PC `DAT_01fb7f0d == 0` half-power flag (default `true` = full power).
    pub full_power_mode: bool,
    /// Current boat animation ID for retail PC `0x11..=0x4f` gate (default `0x20`).
    pub current_boat_anim_id: i16,
}

/// Normalizes an angle in radians to `(-PI, PI]` (`shAngleRegulate`).
#[inline]
pub fn regulate_angle(mut angle: f32) -> f32 {
    while angle > PI {
        angle -= std::f32::consts::TAU;
    }
    while angle < -PI {
        angle += std::f32::consts::TAU;
    }
    angle
}

/// `BoatCheckKeyInputStickDir` (`0x0026c2e0`): `shAtan2(-y, x)`.
#[inline]
pub fn stick_dir(x: f32, y: f32) -> f32 {
    (-y).atan2(x)
}

/// `BoatCheckKeyInputStickPow` (`0x0026c310` / `0x0053ea70`): projected magnitude clamped to `[0, 1]`.
#[inline]
pub fn stick_pow(x: f32, y: f32, dir: f32) -> f32 {
    let s = dir.sin();
    let c = dir.cos();
    let mag = ((-y * c) * (-y * c) + (x * s) * (x * s)).sqrt();
    mag.clamp(0.0, 1.0)
}

/// Stateful 20-frame stick history & oar power processor (`BoatCheckKeyInput`).
#[derive(Debug, Clone)]
pub struct RowboatInputProcessor {
    pub lstick_buf: [f32; STICK_HISTORY_LEN],
    pub rstick_buf: [f32; STICK_HISTORY_LEN],
    /// Left oar direction (`0 = idle, 1 = forward, 2 = reverse`).
    pub left_dir: u8,
    /// Right oar direction (`0 = idle, 1 = forward, 2 = reverse`).
    pub right_dir: u8,
    /// Left oar stick status bitmask (`1 = active, 2 = idle, 4 = fwd/cw, 8 = rev/ccw`).
    pub lstick_status: u8,
    /// Right oar stick status bitmask.
    pub rstick_status: u8,
    /// Computed left oar power (`_lstick_power`).
    pub lstick_power: f32,
    /// Computed right oar power (`_rstick_power`).
    pub rstick_power: f32,
    /// Turn power (`_round_power = (l - r) * 2.0`).
    pub round_power: f32,
    /// Forward/reverse power (`_straight_power = (l + r) * 0.5`).
    pub straight_power: f32,
    /// Requested rowing status (`DAT_01202565` / `DAT_01f96819`).
    pub pending_row_mode: BoatRowMode,
    /// Previous requested rowing status (`DAT_01202566` / `DAT_01f9681a`).
    pub prev_pending_row_mode: BoatRowMode,
    /// Cooldown timer before another mode switch (`_DAT_01202568` / `_DAT_01f9681c`).
    pub mode_cooldown: f32,
}

impl Default for RowboatInputProcessor {
    fn default() -> Self {
        Self::new()
    }
}

impl RowboatInputProcessor {
    /// Initializes input state (`HumanBOTInit` `0x0026c170`).
    pub fn new() -> Self {
        Self {
            lstick_buf: [0.0; STICK_HISTORY_LEN],
            rstick_buf: [0.0; STICK_HISTORY_LEN],
            left_dir: 0,
            right_dir: 0,
            lstick_status: 0,
            rstick_status: 0,
            lstick_power: 0.0,
            rstick_power: 0.0,
            round_power: 0.0,
            straight_power: 0.0,
            pending_row_mode: BoatRowMode::Both,
            prev_pending_row_mode: BoatRowMode::Both,
            mode_cooldown: 0.0,
        }
    }

    /// Evaluates one tick of `BoatCheckKeyInput` (`0x0026c420` SLUS / `0x0053eb10` PC).
    pub fn process(&mut self, input: &RowboatInput, forward_speed: f32) {
        if input.enable_analog_trigger_rowing
            && (input.left_trigger > 0.05 || input.right_trigger > 0.05)
        {
            let sign = if input.trigger_reverse { -1.0 } else { 1.0 };
            self.lstick_power = (input.left_trigger.clamp(0.0, 1.0) * 0.5) * sign;
            self.rstick_power = (input.right_trigger.clamp(0.0, 1.0) * 0.5) * sign;
            self.left_dir = dir_from_power(self.lstick_power);
            self.right_dir = dir_from_power(self.rstick_power);
            self.lstick_status = status_from_dir(self.left_dir);
            self.rstick_status = status_from_dir(self.right_dir);
        } else {
            let l_dir = stick_dir(input.left_stick_x, input.left_stick_y);
            let r_dir = stick_dir(input.right_stick_x, input.right_stick_y);
            let l_pow = stick_pow(input.left_stick_x, input.left_stick_y, l_dir);
            let r_pow = stick_pow(input.right_stick_x, input.right_stick_y, r_dir);
            let _ = r_pow;

            match input.control_mode {
                RowboatControlMode::SingleStick2D => {
                    let c = l_dir.cos();
                    let s = l_dir.sin();
                    self.lstick_power =
                        SINGLE_STICK_COS_WEIGHT * l_pow * c + SINGLE_STICK_SIN_WEIGHT * l_pow * s;
                    self.rstick_power =
                        SINGLE_STICK_COS_WEIGHT * l_pow * c - SINGLE_STICK_SIN_WEIGHT * l_pow * s;
                    if !input.full_power_mode {
                        self.lstick_power *= 0.5;
                        self.rstick_power *= 0.5;
                    }
                    self.left_dir = if self.lstick_power >= 0.0 { 1 } else { 2 };
                    self.right_dir = if self.rstick_power >= 0.0 { 1 } else { 2 };
                    self.lstick_status = status_from_dir(self.left_dir);
                    self.rstick_status = status_from_dir(self.right_dir);
                }
                RowboatControlMode::DualStick3D => {
                    let dl = regulate_angle(l_dir - self.lstick_buf[0]);
                    let dr = regulate_angle(r_dir - self.rstick_buf[0]);

                    self.lstick_status = 1;
                    if self.lstick_buf.iter().all(|&v| v == l_dir) {
                        self.lstick_power = 0.0;
                        self.lstick_status |= 2;
                        self.left_dir = 0;
                    } else {
                        self.lstick_power = -dl;
                        if dl > 0.0 {
                            self.lstick_status |= 4;
                            self.left_dir = 2;
                        } else if dl < 0.0 {
                            self.lstick_status |= 8;
                            self.left_dir = 1;
                        } else {
                            self.lstick_status |= status_bit_for_dir(self.left_dir, true);
                        }
                    }
                    self.lstick_buf.copy_within(0..STICK_HISTORY_LEN - 1, 1);
                    self.lstick_buf[0] = l_dir;
                    self.lstick_power = self
                        .lstick_power
                        .clamp(-DUAL_STICK_POWER_CLAMP, DUAL_STICK_POWER_CLAMP);

                    self.rstick_status = 1;
                    if self.rstick_buf.iter().all(|&v| v == r_dir) {
                        self.rstick_power = 0.0;
                        self.rstick_status |= 2;
                        self.right_dir = 0;
                    } else {
                        self.rstick_power = dr;
                        if dr > 0.0 {
                            self.rstick_status |= 4;
                            self.right_dir = 1;
                        } else if dr < 0.0 {
                            self.rstick_status |= 8;
                            self.right_dir = 2;
                        } else {
                            self.rstick_status |= status_bit_for_dir(self.right_dir, false);
                        }
                    }
                    self.rstick_buf.copy_within(0..STICK_HISTORY_LEN - 1, 1);
                    self.rstick_buf[0] = r_dir;
                    self.rstick_power = self
                        .rstick_power
                        .clamp(-DUAL_STICK_POWER_CLAMP, DUAL_STICK_POWER_CLAMP);
                }
            }
        }

        // Retail PC animation gate & 1.8x power multiplier (`0x0053eb10`).
        let (eff_l, eff_r) = if input.current_boat_anim_id < PC_BOAT_ANIM_MIN
            || input.current_boat_anim_id > PC_BOAT_ANIM_MAX
        {
            (0.0, 0.0)
        } else {
            (
                self.lstick_power * PC_ROWING_POWER_SCALE,
                self.rstick_power * PC_ROWING_POWER_SCALE,
            )
        };

        self.round_power = (eff_l - eff_r) * ROUND_POWER_SCALE;
        self.straight_power = (eff_l + eff_r) * STRAIGHT_POWER_SCALE;

        if self.mode_cooldown == 0.0 {
            let prev = self.pending_row_mode;
            self.prev_pending_row_mode = prev;
            let raw =
                BOAT_STATUS_TABLE[self.left_dir.min(2) as usize][self.right_dir.min(2) as usize];
            let next = if forward_speed >= BOAT_STATUS_FORCE_BOTH_SPEED {
                BoatRowMode::Both
            } else {
                BoatRowMode::from_u8(raw)
            };
            self.pending_row_mode = next;
            if prev != next {
                self.mode_cooldown = BOAT_STATUS_COOLDOWN_SECS;
            }
        }
    }
}

#[inline]
const fn dir_from_power(p: f32) -> u8 {
    if p > 0.0 {
        1
    } else if p < 0.0 {
        2
    } else {
        0
    }
}

#[inline]
const fn status_from_dir(dir: u8) -> u8 {
    match dir {
        1 => 1 | 4,
        2 => 1 | 8,
        _ => 1 | 2,
    }
}

#[inline]
const fn status_bit_for_dir(dir: u8, is_left: bool) -> u8 {
    match (dir, is_left) {
        (0, _) => 2,
        (1, true) | (2, false) => 8,
        _ => 4,
    }
}
