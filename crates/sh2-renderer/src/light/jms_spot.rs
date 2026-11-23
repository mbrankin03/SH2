//! 1:1 port of `Chacter_Draw/sh2_JmsSpot_Man.c` (`SLUS_202.28` `0x0028e0b0..0x0028eaf0`):
//! James's flashlight turn-on/off ramp, LCG flicker, gun muzzle flash propagation,
//! and `DARK_Hosei` ambient trim.

/// Ambient dark-trim multiplier `DARK_Hosei` (`SLUS_202.28` `0x002ab710` = `0.5f`).
///
/// Used in `sh2gfw_setparal3LGT_blk` (`0x0017d7f0`) and `sh2gfw_setsp1para2LGT_blk` (`0x0017ddd0`):
/// `world_ambient += DARK_Hosei * 4.0 * dark_trim_factor * base_ambient`.
pub const DARK_HOSEI: f32 = 0.5;

/// VU1 world-lighting scale factor (`0x40800000` = `4.0f`) applied to parallel light colors and
/// world ambient in `sh2gfw_setparal3LGT_blk` (`0x0017d7f0`) and `sh2gfw_setsp1para2LGT_blk` (`0x0017ddd0`).
pub const PS2_VU1_WORLD_LIGHT_SCALE: f32 = 4.0;

/// Gun muzzle flash ambient multiplier in VU1 4×-scaled lighting space
/// (`sh2gfw_setparal3LGT_blk` `0x0017d7f0` & `sh2gfw_setsp1para2LGT_blk` `0x0017ddd0`:
/// `fVar2 = sh2gfw_Get_GunLight() * 2.0`).
pub const GUN_LIGHT_AMBIENT_MULTIPLIER: f32 = 2.0;

/// Normalized (1.0-ambient-space) gun muzzle flash ambient boost multiplier
/// (`GUN_LIGHT_AMBIENT_MULTIPLIER / PS2_VU1_WORLD_LIGHT_SCALE = 2.0 / 4.0 = 0.5`), matching how
/// `DARK_Hosei * 4.0 * dark_trim_factor` factors out `4.0` in `0x0017d7f0` / `0x0017ddd0`.
pub const GUN_LIGHT_NORMALIZED_AMBIENT_MULTIPLIER: f32 =
    GUN_LIGHT_AMBIENT_MULTIPLIER / PS2_VU1_WORLD_LIGHT_SCALE;

/// Daytime reflection highlight scale factor in `Programed_Light_Set` (`SLUS_202.28` `0x0028eaf0`,
/// `sceVu0ScaleVector(0x3f333333, ...)` = `0.7f` applied to `0x0037b570`).
pub const DAYTIME_REFLECTION_HIGHLIGHT_SCALE: f32 = 0.7;

/// Base reflection highlight RGB color in `Programed_Light_Set` (`SLUS_202.28` `0x0037b570` =
/// `[1.0, 1.0, 1.0, 0.0]`).
pub const BASE_REFLECTION_HIGHLIGHT_RGB: [f32; 3] = [1.0, 1.0, 1.0];

/// Initial gun muzzle flash intensity written by `sh2gfw_Set_JmsGunLight` (`0x0028e220`, `0x3f400000`).
pub const GUN_LIGHT_INITIAL_INTENSITY: f32 = 0.75;

/// Per-frame gun muzzle flash decay in `sh2gfw_Propagate_JmsGunLight` (`0x0028e1b0`).
pub const GUN_LIGHT_DECAY_PER_TICK: f32 = 0.15;

/// Constant floor added by `sh2gfw_Get_JmsGunLight` (`0x0028e240`).
pub const GUN_LIGHT_JMS_BASE_OFFSET: f32 = 0.075;

/// LCG multiplier in `Flicker_Jms_Spot` (`0x0028e7e0`, Numerical Recipes LCG `0x19660d`).
pub const SPOT_LCG_MULTIPLIER: u32 = 0x0019_660d;

/// LCG increment in `Flicker_Jms_Spot` (`0x0028e7e0`, Numerical Recipes LCG `0x3c6ef35f`).
pub const SPOT_LCG_INCREMENT: u32 = 0x3c6e_f35f;

/// Minimum residual spotlight color written by `sh2gfw_TrueOff_JmsSPOT` (`0x0028e500`, `0x3a83126f`).
pub const SPOT_MIN_COLOR_OFF: f32 = 0.001;

/// Normalized intensity cutoff in `Calculate_Jms_Spot` (`0x0028e940`, `0x3fc999999999999a` = `0.2`).
pub const SPOT_OFF_THRESHOLD: f64 = 0.2;

/// Default max dark-trim scale when James has the flashlight (`sh2gfw_Get_MaxDarkScaleFactor` `0x0028e360`, `0x3e99999a`).
pub const DARK_ROOM_MAX_SCALE_DEFAULT: f32 = 0.3;

/// Max dark-trim scale before James acquires the flashlight (`sh2gfw_Get_MaxDarkScaleFactor` `0x0028e360`, `0x3f800000`).
pub const DARK_ROOM_MAX_SCALE_NO_FLASHLIGHT: f32 = 1.0;

/// Packed map IDs where `sh2gfw_Get_MaxDarkScaleFactor` (`0x0028e360`) forces pitch-black (`0.0`).
pub const PITCH_BLACK_MAP_IDS: [u32; 4] = [0x000a_00e6, 0x000a_0091, 0x000a_007d, 0x000a_003d];

/// Exact port of `LinearTrim` (`Chacter_Draw/sh2_JmsSpot_Man.c` `0x0028e7b0`).
///
/// Maps `x` linearly from `[min_in, max_in]` to `[min_out, max_out]`.
#[must_use]
#[inline]
pub fn linear_trim(max_out: f32, min_out: f32, max_in: f32, min_in: f32, x: f32) -> f32 {
    min_out + ((max_out - min_out) * (x - min_in)) / (max_in - min_in)
}

/// 1:1 state machine matching `JmsSpotMan` (`0x0037b510`, 68 bytes) in `Chacter_Draw/sh2_JmsSpot_Man.c`.
#[derive(Debug, Clone, PartialEq)]
pub struct JmsSpotManager {
    /// 1-frame trigger latch for `sh2gfw_Set_JmsGunLight` (`JmsSpotMan +0x00`).
    pub gun_trigger: bool,
    /// Flashlight currently active (`DAT_0037b512`).
    pub spot_on: bool,
    /// Current ramp counter (`DAT_0037b518`).
    pub ramp_count: i16,
    /// Ramp upper bound (`DAT_0037b51a`, default `60` = `0x3c`).
    pub ramp_max: i16,
    /// Ramp lower bound (`DAT_0037b51c`, default `1`).
    pub ramp_min: i16,
    /// Per-tick ramp delta (`DAT_0037b51e`: `+2` on, `-10` / `0xfff6` off, `0` idle).
    pub ramp_step: i16,
    /// Dark-trim ramp duration (`DAT_0037b520`, default `450` = `0x1c2`).
    pub dark_ramp_max: i16,
    /// Dark-trim ramp step (`DAT_0037b522`, `1` after `TrueOff`, `0` idle).
    pub dark_ramp_step: i16,
    /// Dark-trim ramp counter (`DAT_0037b524`).
    pub dark_ramp_count: i16,
    /// Flicker amplitude (`DAT_0037b528`, default `3`).
    pub flicker_amp: i16,
    /// Active flicker frame counter (`DAT_0037b52a`, `0` = not flickering).
    pub flicker_tick: i16,
    /// Flicker total duration in frames (`DAT_0037b52c`, default `100`).
    pub flicker_duration: i16,
    /// End state after flicker finishes (`DAT_0037b52e`: `false` = `ForceOff`, `true` = `ForceOn`).
    pub flicker_end_on: bool,
    /// Gun muzzle flash intensity (`DAT_0037b530`, decays by `0.15`/tick from `0.75`).
    pub gun_light: f32,
    /// Maximum spotlight color multiplier (`DAT_0037b534`, default `7.0` = `0x40e00000`).
    pub spot_max_color: f32,
    /// Minimum spotlight color multiplier (`DAT_0037b538`, default `0.001` = `0x3a83126f`).
    pub spot_min_color: f32,
    /// Current evaluated spotlight scalar intensity (`DAT_0037b53c`).
    pub spot_intensity: f32,
    /// Current evaluated spotlight RGB channel value (`DAT_002ad940..002ad948`).
    pub spot_rgb_scalar: f32,
    /// Current evaluated spotlight distance parameter in PS2 mm (`DAT_0037b540`, default `1500.0` = `0x44bb8000`).
    pub spot_dist_mm: f32,
    /// Shadow color scale factor (`DAT_0037b54c`, default `1.0`).
    pub shadow_scale_factor: f32,
    /// Dark ambient trim factor (`DAT_0037b550`, default `1.0`).
    pub dark_trim_factor: f32,
    /// Numerical Recipes LCG state (`DAT_002adaf0`, initial seed `1`).
    pub lcg_seed: u32,
}

impl Default for JmsSpotManager {
    fn default() -> Self {
        Self::new()
    }
}

impl JmsSpotManager {
    /// Initializes all fields identically to `sh2gfw_ALLInit_SpotMan` (`0x0028e0b0`).
    #[must_use]
    pub fn new() -> Self {
        Self {
            gun_trigger: false,
            spot_on: false,
            ramp_count: 0,
            ramp_max: 60,       // 0x3c
            ramp_min: 1,        // 1
            ramp_step: 0,       // 0
            dark_ramp_max: 450, // 0x1c2
            dark_ramp_step: 0,  // 0
            dark_ramp_count: 0, // 0
            flicker_amp: 3,     // 3
            flicker_tick: 0,    // 0
            flicker_duration: 100,
            flicker_end_on: false,
            gun_light: 0.0,
            spot_max_color: 7.0, // 0x40e00000
            spot_min_color: SPOT_MIN_COLOR_OFF,
            spot_intensity: 0.0,
            spot_rgb_scalar: SPOT_MIN_COLOR_OFF,
            spot_dist_mm: 1500.0, // 0x44bb8000
            shadow_scale_factor: 1.0,
            dark_trim_factor: 1.0,
            lcg_seed: 1, // DAT_002adaf0 initial value in SLUS_202.28
        }
    }

    /// Fires a gun muzzle flash (`sh2gfw_Set_JmsGunLight` `0x0028e220`).
    pub fn trigger_gun_light(&mut self) {
        self.gun_light = GUN_LIGHT_INITIAL_INTENSITY;
        self.gun_trigger = true;
    }

    /// Advances gun muzzle flash decay by one frame (`sh2gfw_Propagate_JmsGunLight` `0x0028e1b0`).
    pub fn propagate_gun_light(&mut self) {
        if !self.gun_trigger {
            if self.gun_light > GUN_LIGHT_DECAY_PER_TICK {
                self.gun_light -= GUN_LIGHT_DECAY_PER_TICK;
            } else {
                self.gun_light = 0.0;
            }
        } else {
            self.gun_trigger = false;
        }
    }

    /// Returns world-pass gun muzzle flash intensity (`sh2gfw_Get_GunLight` `0x0028e290`).
    #[must_use]
    pub fn gun_light(&self) -> f32 {
        if self.gun_trigger {
            0.0
        } else {
            self.gun_light
        }
    }

    /// Returns James character-pass gun muzzle flash intensity (`sh2gfw_Get_JmsGunLight` `0x0028e240`).
    #[must_use]
    pub fn jms_gun_light(&self) -> f32 {
        if self.gun_trigger {
            GUN_LIGHT_JMS_BASE_OFFSET
        } else {
            self.gun_light + GUN_LIGHT_JMS_BASE_OFFSET
        }
    }

    /// Returns current dark-trim factor (`sh2gfw_Get_DarkTrimFactor` `0x0028e350`).
    #[must_use]
    pub fn dark_trim_factor(&self) -> f32 {
        self.dark_trim_factor
    }

    /// Returns the normalized spotlight intensity in `[0.0, 1.0]` (`spot_rgb_scalar / spot_max_color`),
    /// unifying `Calculate_Jms_Spot` (`0x0028e940`) 30-frame turn-on / 6-frame turn-off ramps and
    /// `Flicker_Jms_Spot` (`0x0028e7e0`) LCG flicker.
    #[must_use]
    #[inline]
    pub fn normalized_spot_intensity(&self) -> f32 {
        if !self.spot_on || self.spot_max_color <= 1e-6 {
            0.0
        } else {
            (self.spot_rgb_scalar / self.spot_max_color).clamp(0.0, 1.0)
        }
    }

    /// Evaluates `sh2gfw_Get_MaxDarkScaleFactor` (`0x0028e360`).
    #[must_use]
    pub fn max_dark_scale_factor(has_flashlight: bool, packed_map_id: u32) -> f32 {
        if !has_flashlight {
            DARK_ROOM_MAX_SCALE_NO_FLASHLIGHT
        } else if PITCH_BLACK_MAP_IDS.contains(&packed_map_id) {
            0.0
        } else {
            DARK_ROOM_MAX_SCALE_DEFAULT
        }
    }

    /// Begins smooth flashlight turn-on ramp (`sh2gfw_On_JmsSPOT` `0x0028e410`).
    pub fn turn_on(&mut self) {
        self.spot_on = true;
        self.ramp_step = 2;
        self.ramp_max = 60;
    }

    /// Begins fast flashlight turn-off ramp (`sh2gfw_Off_JmsSPOT` `0x0028e680`, step `-10`).
    pub fn turn_off(&mut self) {
        self.ramp_count = self.ramp_max;
        self.ramp_step = -10; // 0xfff6 as i16
    }

    /// Finalizes flashlight off state (`sh2gfw_TrueOff_JmsSPOT` `0x0028e500`).
    pub fn true_off(&mut self) {
        self.spot_rgb_scalar = SPOT_MIN_COLOR_OFF;
        self.spot_dist_mm = 1500.0;
        self.spot_on = false;
        self.dark_ramp_step = 1;
        self.shadow_scale_factor = 0.0;
    }

    /// Immediately forces flashlight off (`sh2gfw_ForceOff_JmsSPOT` `0x0028e6e0`).
    pub fn force_off(&mut self) {
        self.ramp_count = 0;
        self.ramp_step = -1;
        self.true_off();
        self.dark_ramp_step = 0;
        self.ramp_step = 0;
    }

    /// Immediately forces flashlight on (`sh2gfw_ForceOn_JmsSPOT` `0x0028e750`).
    pub fn force_on(&mut self) {
        self.turn_on();
        self.ramp_count = self.ramp_max;
        self.shadow_scale_factor = 1.0;
    }

    /// Starts a scripted LCG flicker sequence (`Flicker_Jms_Spot` `0x0028e7e0`).
    pub fn start_flicker(&mut self, duration_frames: i16, amplitude: i16, end_on: bool) {
        self.flicker_tick = 1;
        self.flicker_duration = duration_frames.max(1);
        self.flicker_amp = amplitude;
        self.flicker_end_on = end_on;
    }

    /// Steps one frame of `Flicker_Jms_Spot` (`0x0028e7e0`).
    pub fn step_flicker(&mut self) {
        self.flicker_tick = self.flicker_tick.saturating_add(1);
        if self.flicker_tick < self.flicker_duration {
            self.lcg_seed = self
                .lcg_seed
                .wrapping_mul(SPOT_LCG_MULTIPLIER)
                .wrapping_add(SPOT_LCG_INCREMENT);
            // Exact divisor `4.2949673e+09` (`2^32` as f32) from `0x0028e7e0`
            self.shadow_scale_factor = (self.lcg_seed as f32) / 4_294_967_296.0_f32;
            self.spot_intensity =
                self.spot_max_color - f32::from(self.flicker_amp) * self.shadow_scale_factor;
        } else {
            self.flicker_tick = 0;
            if !self.flicker_end_on {
                self.force_off();
            } else {
                self.force_on();
            }
        }
        self.spot_rgb_scalar = self.spot_intensity;
    }

    /// Steps one frame of `Calculate_Jms_Spot` (`0x0028e940`).
    pub fn step_ramp(&mut self) {
        self.ramp_count = self.ramp_count.saturating_add(self.ramp_step);
        if self.ramp_step >= 1 && self.ramp_count >= self.ramp_max {
            self.ramp_count = self.ramp_max;
            self.ramp_step = 0;
        } else if self.ramp_step < 0 && self.ramp_count <= self.ramp_min {
            self.ramp_count = self.ramp_min;
            self.ramp_step = 0;
        }

        self.spot_intensity = linear_trim(
            1.0,
            0.0,
            f32::from(self.ramp_max),
            f32::from(self.ramp_min),
            f32::from(self.ramp_count),
        );
        self.spot_rgb_scalar = self.spot_max_color * self.spot_intensity;
        self.shadow_scale_factor = self.spot_intensity;

        if f64::from(self.spot_intensity) <= SPOT_OFF_THRESHOLD && self.ramp_step == 0 {
            self.true_off();
        }
    }

    /// Advances the full per-frame spotlight & dark-trim update (`Programed_Light_Set` `0x0028eaf0`
    /// lines 490..528).
    pub fn tick(&mut self, is_night: bool, has_flashlight: bool, packed_map_id: u32) {
        self.propagate_gun_light();

        if self.flicker_tick == 0 {
            if self.ramp_step != 0 {
                self.step_ramp();
            }
        } else {
            self.step_flicker();
        }

        let max_dark = Self::max_dark_scale_factor(has_flashlight, packed_map_id);
        if max_dark > 0.0 && is_night {
            if !self.spot_on {
                if self.dark_ramp_step == 0 || self.dark_ramp_count > self.dark_ramp_max {
                    self.dark_ramp_step = 0;
                    self.dark_ramp_count = 0;
                    self.dark_trim_factor = max_dark;
                } else {
                    self.dark_ramp_count = self.dark_ramp_count.saturating_add(self.dark_ramp_step);
                    self.dark_trim_factor = linear_trim(
                        max_dark,
                        0.0,
                        f32::from(self.dark_ramp_max),
                        0.0,
                        f32::from(self.dark_ramp_count),
                    );
                }
                self.shadow_scale_factor = self.dark_trim_factor;
            } else if self.ramp_step > 0 {
                let next_trim = self.dark_trim_factor - self.shadow_scale_factor;
                if next_trim >= 0.001 {
                    self.dark_trim_factor = next_trim;
                }
                self.dark_ramp_count = 0;
            }
        }
    }
}
