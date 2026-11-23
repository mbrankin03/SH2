//! Authentic PS2 2D post-process filter algorithms & constants (`SLUS_202.28` `GFW/sh2gfw_2d_filters.c`,
//! `GFW/sh2_set_drawenv.c`).
//!
//! Ground-truth references (`SLUS_202.28`):
//! - `sh2gfw_Set_Brightness` (`0x001757b0`, jump table `0x0038f060`): 8-level GS brightness LUT
//!   `[0x4b, 0x5a, 0x6e, 0x80, 0x8c, 0x9b, 0xb4, 0xdc]` (default/OOB `0x80`).
//!   Note: Retail PC `sh2pc.exe` `FUN_0047b770` (`0x0047b770`) retuned these to
//!   `[0.5, 0.6, 0.7, 1.0, 1.2, 1.4, 1.7, 2.0]`.
//! - `sh2gfw_test_MakeNoise` (`0x00177c50`) & `sh2gfw_SendDraw_Noise` (`0x00177f30`):
//!   128×128 LCG (`state * 0x19660d + 0x3c6ef35f`) 8-bit noise texture tiled 4×4 across screen.
//! - `sh2gfw_Swap_Soft` (`0x001785e0`): N×N ping-pong DoF blur with 3 GS `TEST_1` modes.
//! - `sh2gfw_Swap_GlowSoft` (`0x00178d60`): N×N ping-pong additive bloom (`ALPHA_1 = 0x68`, `TEST_1 = 0x3800b`).
//! - `sh2gfw_Filter_Retain` (`0x00179b10`), `sh2gfw_Set_PauseRetain` (`0x0017ae90`),
//!   `sh2gfw_Reset_PauseRetain` (`0x0017aee0`): command `0x17` feedback / pause freeze.
//! - `sh2gfw_Fade2` (`0x00179d60`), `sh2gfw_Fade3` (`0x0017a2e0`), `sh2gfw_FadeOut_Retain` (`0x001795c0`):
//!   Black / White / Red fade commands (`0x0e..=0x15`).

/// Exact 8-level PS2 GS brightness RGB LUT (`SLUS_202.28` `sh2gfw_Set_Brightness` `0x001757b0`,
/// jump table `0x0038f060`). Level `3` (`0x80 = 128`) is neutral `1.0x` in GS modulation.
///
/// Prototype vs. Retail PC drift:
/// - `SLUS_202.28`: `[75, 90, 110, 128, 140, 155, 180, 220] / 128.0`
///   = `[0.5859375, 0.703125, 0.859375, 1.0, 1.09375, 1.2109375, 1.40625, 1.71875]`.
/// - `sh2pc.exe` `FUN_0047b770` (`0x0047b770`): `[0.5, 0.6, 0.7, 1.0, 1.2, 1.4, 1.7, 2.0]`.
pub const PS2_BRIGHTNESS_LUT_U8: [u8; 8] = [0x4b, 0x5a, 0x6e, 0x80, 0x8c, 0x9b, 0xb4, 0xdc];

/// Out-of-range fallback brightness byte (`0x80 = 128`, `SLUS_202.28` `0x001757b0` lines 35-37).
pub const PS2_BRIGHTNESS_NEUTRAL_U8: u8 = 0x80;

/// Returns the raw PS2 GS brightness byte (`0x4b..=0xdc`, or `0x80` if `level >= 8`)
/// matching `sh2gfw_Set_Brightness` (`0x001757b0`).
#[must_use]
pub const fn ps2_brightness_byte(level: u8) -> u8 {
    if (level as usize) < PS2_BRIGHTNESS_LUT_U8.len() {
        PS2_BRIGHTNESS_LUT_U8[level as usize]
    } else {
        PS2_BRIGHTNESS_NEUTRAL_U8
    }
}

/// Returns the normalized PS2 brightness scale (`byte / 128.0`) matching `sh2gfw_Set_Brightness` (`0x001757b0`).
#[must_use]
pub fn ps2_brightness_scale(level: u8) -> f32 {
    (ps2_brightness_byte(level) as f32) / (PS2_BRIGHTNESS_NEUTRAL_U8 as f32)
}

/// Numerical Recipes LCG multiplier used by `sh2gfw_test_MakeNoise` (`0x00177c50`).
pub const PS2_NOISE_LCG_MUL: u32 = 0x0019_660d;

/// Numerical Recipes LCG increment used by `sh2gfw_test_MakeNoise` (`0x00177c50`).
pub const PS2_NOISE_LCG_INC: u32 = 0x3c6e_f35f;

/// PS2 noise texture width and height in texels (`0x80 = 128`, `sh2gfw_SendDraw_Noise` `0x00177f30`).
pub const PS2_NOISE_TEX_SIZE: usize = 128;

/// Number of noise tiles along each axis (`4x4` grid covering `512x512` in `sh2gfw_SendDraw_Noise` `0x00177f30`).
pub const PS2_NOISE_TILE_GRID: usize = 4;

/// Stateful 128×128 LCG film-grain generator matching `SLUS_202.28` `sh2gfw_test_MakeNoise` (`0x00177c50`).
#[derive(Debug, Clone)]
pub struct Ps2NoiseGenerator {
    /// Current 32-bit LCG seed (`DAT_002adaf0`).
    pub seed: u32,
}

impl Ps2NoiseGenerator {
    /// Creates a new generator with the given initial LCG seed (`DAT_002adaf0`).
    #[must_use]
    pub const fn new(seed: u32) -> Self {
        Self { seed }
    }

    /// Advances the LCG by one step (`state * 0x19660d + 0x3c6ef35f`, `0x00177c50`).
    #[inline]
    pub fn next_u32(&mut self) -> u32 {
        self.seed = self
            .seed
            .wrapping_mul(PS2_NOISE_LCG_MUL)
            .wrapping_add(PS2_NOISE_LCG_INC);
        self.seed
    }

    /// Generates one `128x128` 8-bit noise texture (`16,384` bytes = `4,096` `u32` words filled from
    /// index `0xfff` down to `0`, matching `sh2gfw_test_MakeNoise` `0x00177c50` lines 503-510).
    #[must_use]
    pub fn generate_128x128(&mut self) -> Vec<u8> {
        let mut words = vec![0u32; (PS2_NOISE_TEX_SIZE * PS2_NOISE_TEX_SIZE) / 4];
        for i in (0..words.len()).rev() {
            let u1 = self.next_u32();
            let u2 = self.next_u32();
            let u3 = self.next_u32();
            let u4 = self.next_u32();
            words[i] = ((u1 >> 24) << 24) | ((u2 >> 24) << 16) | ((u3 >> 24) << 8) | (u4 >> 24);
        }
        let mut bytes = Vec::with_capacity(PS2_NOISE_TEX_SIZE * PS2_NOISE_TEX_SIZE);
        for w in words {
            bytes.extend_from_slice(&w.to_le_bytes());
        }
        bytes
    }
}

/// Samples a `128x128` noise texture tiled `4x4` across a `(width, height)` framebuffer
/// (`sh2gfw_SendDraw_Noise` `0x00177f30`, `iVar7 < 4`, `iVar8 < 4`).
#[must_use]
pub fn sample_ps2_noise_tiled_4x4(
    noise_128x128: &[u8],
    px: u32,
    py: u32,
    width: u32,
    height: u32,
) -> u8 {
    if noise_128x128.len() != PS2_NOISE_TEX_SIZE * PS2_NOISE_TEX_SIZE || width == 0 || height == 0 {
        return 0;
    }
    let u = ((px as f32) / (width as f32)) * (PS2_NOISE_TILE_GRID as f32);
    let v = ((py as f32) / (height as f32)) * (PS2_NOISE_TILE_GRID as f32);
    let tx = ((u.fract() * (PS2_NOISE_TEX_SIZE as f32)) as usize).min(PS2_NOISE_TEX_SIZE - 1);
    let ty = ((v.fract() * (PS2_NOISE_TEX_SIZE as f32)) as usize).min(PS2_NOISE_TEX_SIZE - 1);
    noise_128x128[ty * PS2_NOISE_TEX_SIZE + tx]
}

/// Computes the exact 1D tap offsets in PS2 12.4 fixed-point sub-pixels (`16 == 1.0 px`) used by
/// both `sh2gfw_Swap_Soft` (`0x001785e0` lines 852-882) and `sh2gfw_Swap_GlowSoft` (`0x00178d60`
/// lines 1064-1094) for an `n × n` blur kernel with step parameter `step_param`.
#[must_use]
pub fn compute_swap_tap_offsets_12_4(n: u32, step_param: i32) -> Vec<i16> {
    if n == 0 {
        return Vec::new();
    }
    let half = (n / 2) as i16;
    let step = (step_param * 4) as i16;
    let mut out = Vec::with_capacity(n as usize);
    if (n & 1) == 0 {
        let half_step = (step_param * 4) / 2;
        let hs = half_step as i16;
        for i in 0..n {
            let off = if (i as i16) < half {
                (step * ((i as i16) - half) - hs) - 4
            } else {
                hs + step * ((i as i16) - half) - 4
            };
            out.push(off);
        }
    } else {
        for i in 0..n {
            out.push(step * ((i as i16) - half) - 4);
        }
    }
    out
}

/// GS `TEST_1` alpha-test mode for `sh2gfw_Swap_Soft` (`0x001785e0` lines 822-833).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwapSoftTestMode {
    /// `param_7 == 0`: `TEST_1 = (aref << 4) | 0x3800a` (`ATE = 0`, alpha test disabled).
    All = 0,
    /// `param_7 == 1`: `TEST_1 = (aref << 4) | 0x3800d` (`ATE = 1`, `ATST = 6 (GREATER)`, near DoF pass).
    Near = 1,
    /// `param_7 == 2`: `TEST_1 = (aref << 4) | 0x38007` (`ATE = 1`, `ATST = 3 (LEQUAL)`, far DoF pass).
    Far = 2,
}

impl SwapSoftTestMode {
    /// Encodes the exact 64-bit PS2 GS `TEST_1` register value (`0x001785e0`).
    #[must_use]
    pub const fn gs_test_1(self, aref: u8) -> u64 {
        let base = match self {
            Self::All => 0x3800a_u64,
            Self::Near => 0x3800d_u64,
            Self::Far => 0x38007_u64,
        };
        ((aref as u64) << 4) | base
    }
}

/// Parameters for `sh2gfw_Swap_Soft` (`0x001785e0`) N×N screen-space DoF blur.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SwapSoftParams {
    /// Kernel grid dimension `N` (`N × N` taps).
    pub grid_n: u32,
    /// Step size parameter (multiplied by `4` in 12.4 fixed-point -> `0.25 px` units).
    pub step_param: i32,
    /// GS `ALPHA_1` FIX weight (`0..=0x80`, encoded as `(alpha << 32) | 0x64`).
    pub blend_alpha: u8,
    /// GS `TEST_1` alpha reference (`0..=0xff`).
    pub aref: u8,
    /// Near / Far / All alpha-test mode (`param_7`).
    pub test_mode: SwapSoftTestMode,
}

impl SwapSoftParams {
    /// Returns the exact PS2 GS `ALPHA_1` register (`(blend_alpha << 32) | 0x64`, `0x001785e0` line 843).
    #[must_use]
    pub const fn gs_alpha_1(self) -> u64 {
        ((self.blend_alpha as u64) << 32) | 0x64
    }

    /// Returns the exact PS2 GS `TEST_1` register (`0x001785e0` lines 822-833).
    #[must_use]
    pub const fn gs_test_1(self) -> u64 {
        self.test_mode.gs_test_1(self.aref)
    }
}

/// Parameters for `sh2gfw_Swap_GlowSoft` (`0x00178d60`) N×N additive bloom filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SwapGlowSoftParams {
    /// Kernel grid dimension `N` (`N × N` taps).
    pub grid_n: u32,
    /// Additive GS `ALPHA_1` FIX weight (`0..=0x80`, encoded as `(alpha << 32) | 0x68`).
    pub blend_alpha: u8,
    /// Bloom luminance/alpha threshold `AREF` (`TEST_1 = (aref << 4) | 0x3800b`).
    pub aref: u8,
    /// Step size parameter (multiplied by `4` in 12.4 fixed-point -> `0.25 px` units).
    pub step_param: i32,
}

impl SwapGlowSoftParams {
    /// Returns the exact PS2 GS `ALPHA_1` register (`(blend_alpha << 32) | 0x68`, `0x00178d60` line 1055).
    #[must_use]
    pub const fn gs_alpha_1(self) -> u64 {
        ((self.blend_alpha as u64) << 32) | 0x68
    }

    /// Returns the exact PS2 GS `TEST_1` register (`(aref << 4) | 0x3800b`, `0x00178d60` line 1045).
    #[must_use]
    pub const fn gs_test_1(self) -> u64 {
        ((self.aref as u64) << 4) | 0x3800b
    }
}

/// PS2 GS `ALPHA_1` register for `sh2gfw_Filter_Retain` (`0x00179b10` line 1382).
pub const GS_FILTER_RETAIN_ALPHA_1: u64 = 0x0000_0080_0000_0062;

/// Filter command ID for `sh2gfw_Set_PauseRetain` (`0x0017ae90`).
pub const PS2_FILTER_CMD_PAUSE_RETAIN: u32 = 0x17;

/// Stateful `sh2gfw_Filter_Retain` (`0x00179b10`) / `sh2gfw_Set_PauseRetain` (`0x0017ae90`)
/// feedback / pause-freeze controller.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FilterRetainState {
    /// Active filter command (`_shGsFilterWork`).
    pub active_cmd: u32,
    /// Saved previous filter command (`_DAT_00616a28`).
    pub saved_cmd: u32,
    /// Number of retained frames elapsed (`*(short*)(params + 4)`).
    pub retain_frame: u16,
}

impl FilterRetainState {
    /// Activates pause-freeze retain (`sh2gfw_Set_PauseRetain` `0x0017ae90`).
    pub fn set_pause_retain(&mut self) {
        if self.active_cmd != PS2_FILTER_CMD_PAUSE_RETAIN {
            self.saved_cmd = self.active_cmd;
            self.active_cmd = PS2_FILTER_CMD_PAUSE_RETAIN;
            self.retain_frame = 0;
        }
    }

    /// Restores the saved filter command (`sh2gfw_Reset_PauseRetain` `0x0017aee0`).
    pub fn reset_pause_retain(&mut self) {
        if self.active_cmd == PS2_FILTER_CMD_PAUSE_RETAIN {
            self.active_cmd = self.saved_cmd;
            self.saved_cmd = PS2_FILTER_CMD_PAUSE_RETAIN;
            self.retain_frame = 0;
        }
    }

    /// Returns `true` while pause-freeze retain (`0x17`) is active.
    #[must_use]
    pub const fn is_frozen(self) -> bool {
        self.active_cmd == PS2_FILTER_CMD_PAUSE_RETAIN
    }

    /// Steps one frame of `sh2gfw_Filter_Retain` (`0x00179b10` lines 1383-1421), returning the
    /// GS modulation RGB bytes: brightness LUT on frame 0, then neutral `[0x80, 0x80, 0x80]`.
    pub fn step_retain_tint(&mut self, brightness_level: u8) -> [u8; 3] {
        let tint = if self.retain_frame == 0 {
            let b = ps2_brightness_byte(brightness_level);
            [b, b, b]
        } else {
            [0x80, 0x80, 0x80]
        };
        self.retain_frame = self.retain_frame.saturating_add(1);
        tint
    }
}

/// PS2 2D screen fade commands (`SLUS_202.28` `Make_Filter_Packet` `0x0017b280` cases `0x0e..=0x15`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ps2FadeKind {
    /// Command `0x0e`: `sh2gfw_Set_FadeOut_Black` (`0x0017a8f0`) -> `Fade2(dir=0, color=1)`.
    FadeOutBlack = 0x0e,
    /// Command `0x0f`: `sh2gfw_Set_FadeOut_White` (`0x0017ab40`) -> `Fade2(dir=0, color=0)`.
    FadeOutWhite = 0x0f,
    /// Command `0x10`: `sh2gfw_Set_FadeOutRetain_Red` (`0x0017aa80`) -> `FadeOut_Retain(2)`.
    FadeOutRetainRed = 0x10,
    /// Command `0x11`: `sh2gfw_Set_FadeOutRetain_Black` (`0x0017a830`) -> `FadeOut_Retain(1)`.
    FadeOutRetainBlack = 0x11,
    /// Command `0x12`: `sh2gfw_Set_FadeOutRetain_White` (`0x0017a9c0`) -> `FadeOut_Retain(0)`.
    FadeOutRetainWhite = 0x12,
    /// Command `0x13`: `sh2gfw_Set_FadeIn_Black` (`0x0017ac10`) -> `Fade2(dir=1, color=1)`.
    FadeInBlack = 0x13,
    /// Command `0x14`: `sh2gfw_Set_FadeIn_White` (`0x0017acd0`) -> `Fade2(dir=1, color=0)`.
    FadeInWhite = 0x14,
    /// Command `0x15`: `sh2gfw_Set_FadeIn_Red` (`0x0017ad90`) -> `Fade3`.
    FadeInRed = 0x15,
}

impl Ps2FadeKind {
    /// Returns the target fade overlay RGB color in `[0.0, 1.0]`.
    #[must_use]
    pub const fn color_rgb(self) -> [f32; 3] {
        match self {
            Self::FadeOutBlack | Self::FadeOutRetainBlack | Self::FadeInBlack => [0.0, 0.0, 0.0],
            Self::FadeOutWhite | Self::FadeOutRetainWhite | Self::FadeInWhite => [1.0, 1.0, 1.0],
            Self::FadeOutRetainRed | Self::FadeInRed => [1.0, 0.0, 0.0],
        }
    }

    /// Evaluates the exact PS2 GS fade alpha in `0..=0x80` (`sh2gfw_Fade2` `0x00179d60` lines
    /// 1493-1508 and `sh2gfw_Fade3` `0x0017a2e0` line 1643) for `(frame, total_frames)`.
    #[must_use]
    pub fn evaluate_alpha_u8(self, frame: i16, total_frames: i16) -> u8 {
        if total_frames <= 0 {
            return match self {
                Self::FadeInBlack | Self::FadeInWhite | Self::FadeInRed => 0,
                _ => 0x80,
            };
        }
        let clamped = frame.clamp(0, total_frames) as i32;
        let prog = ((clamped << 7) / (total_frames as i32)).clamp(0, 0x80);
        let alpha = match self {
            Self::FadeInBlack | Self::FadeInWhite | Self::FadeInRed => 0x80 - prog,
            _ => prog,
        };
        alpha as u8
    }

    /// Evaluates the normalized `[r, g, b, alpha]` fade overlay (`alpha` in `[0.0, 1.0]`).
    #[must_use]
    pub fn evaluate_rgba(self, frame: i16, total_frames: i16) -> [f32; 4] {
        let rgb = self.color_rgb();
        let a = (self.evaluate_alpha_u8(frame, total_frames) as f32) / 128.0;
        [rgb[0], rgb[1], rgb[2], a]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brightness_lut_matches_slus_001757b0() {
        let expected = [0x4b, 0x5a, 0x6e, 0x80, 0x8c, 0x9b, 0xb4, 0xdc];
        for (lvl, &byte) in expected.iter().enumerate() {
            assert_eq!(ps2_brightness_byte(lvl as u8), byte);
        }
        // Out of range defaults to 0x80 (1.0)
        assert_eq!(ps2_brightness_byte(8), 0x80);
        assert_eq!(ps2_brightness_byte(255), 0x80);
        assert!((ps2_brightness_scale(3) - 1.0).abs() < 1e-6);
        assert!((ps2_brightness_scale(0) - 75.0 / 128.0).abs() < 1e-6);
        assert!((ps2_brightness_scale(7) - 220.0 / 128.0).abs() < 1e-6);
    }

    #[test]
    fn lcg_noise_128x128_matches_slus_00177c50() {
        let mut gen = Ps2NoiseGenerator::new(0);
        let buf = gen.generate_128x128();
        assert_eq!(buf.len(), 128 * 128);
        // Verify exact first 4 LCG steps written into word 0xfff (last 4 bytes of buffer):
        let u1 = 0u32.wrapping_mul(0x19660d).wrapping_add(0x3c6ef35f);
        let u2 = u1.wrapping_mul(0x19660d).wrapping_add(0x3c6ef35f);
        let u3 = u2.wrapping_mul(0x19660d).wrapping_add(0x3c6ef35f);
        let u4 = u3.wrapping_mul(0x19660d).wrapping_add(0x3c6ef35f);
        let expected_word =
            ((u1 >> 24) << 24) | ((u2 >> 24) << 16) | ((u3 >> 24) << 8) | (u4 >> 24);
        let last_word = u32::from_le_bytes(buf[buf.len() - 4..].try_into().unwrap());
        assert_eq!(last_word, expected_word);

        // Verify 4x4 tiling repeats across quadrants of a 512x512 screen
        let q0 = sample_ps2_noise_tiled_4x4(&buf, 17, 23, 512, 512);
        let q1 = sample_ps2_noise_tiled_4x4(&buf, 17 + 128, 23, 512, 512);
        let q2 = sample_ps2_noise_tiled_4x4(&buf, 17, 23 + 128, 512, 512);
        assert_eq!(q0, q1);
        assert_eq!(q0, q2);
    }

    #[test]
    fn swap_soft_and_glow_soft_offsets_and_gs_regs() {
        // Odd 3x3 with step_param = 2 -> step = 8: [-12, -4, +4]
        assert_eq!(compute_swap_tap_offsets_12_4(3, 2), vec![-12, -4, 4]);
        // Even 2x2 with step_param = 2 -> step = 8, half_step = 4: [-16, 0]
        assert_eq!(compute_swap_tap_offsets_12_4(2, 2), vec![-16, 0]);

        let soft = SwapSoftParams {
            grid_n: 3,
            step_param: 2,
            blend_alpha: 0x40,
            aref: 0x20,
            test_mode: SwapSoftTestMode::Near,
        };
        assert_eq!(soft.gs_alpha_1(), 0x0000_0040_0000_0064);
        assert_eq!(soft.gs_test_1(), 0x0003_820d);

        let glow = SwapGlowSoftParams {
            grid_n: 3,
            blend_alpha: 0x30,
            aref: 0x60,
            step_param: 2,
        };
        assert_eq!(glow.gs_alpha_1(), 0x0000_0030_0000_0068);
        assert_eq!(glow.gs_test_1(), 0x0003_860b);
    }

    #[test]
    fn filter_retain_pause_freeze_and_fades() {
        let mut retain = FilterRetainState {
            active_cmd: 6,
            saved_cmd: 0,
            retain_frame: 0,
        };
        retain.set_pause_retain();
        assert!(retain.is_frozen());
        assert_eq!(retain.saved_cmd, 6);
        // Frame 0 uses brightness LUT (level 5 -> 0x9b), frame 1 uses neutral 0x80
        assert_eq!(retain.step_retain_tint(5), [0x9b, 0x9b, 0x9b]);
        assert_eq!(retain.step_retain_tint(5), [0x80, 0x80, 0x80]);
        retain.reset_pause_retain();
        assert!(!retain.is_frozen());
        assert_eq!(retain.active_cmd, 6);

        // Black fade out & red fade in golden vectors
        assert_eq!(Ps2FadeKind::FadeOutBlack.evaluate_alpha_u8(0, 30), 0);
        assert_eq!(Ps2FadeKind::FadeOutBlack.evaluate_alpha_u8(15, 30), 64);
        assert_eq!(Ps2FadeKind::FadeOutBlack.evaluate_alpha_u8(30, 30), 128);
        assert_eq!(Ps2FadeKind::FadeInRed.evaluate_alpha_u8(0, 30), 128);
        assert_eq!(Ps2FadeKind::FadeInRed.evaluate_alpha_u8(30, 30), 0);
        assert_eq!(Ps2FadeKind::FadeInRed.color_rgb(), [1.0, 0.0, 0.0]);
        assert_eq!(Ps2FadeKind::FadeOutWhite.color_rgb(), [1.0, 1.0, 1.0]);
    }
}
