//! State machine for `GameoverMain` (`0x001c4c70`) & `draw_opd_wowk_main` (`0x001c4de0`).

/// Delay in 30 Hz frames before fade-in begins (`DAT_0116d7d2 == 0x20 = 32` at `0x001c4de0`).
pub const GAMEOVER_DELAY_FRAMES: u32 = 32;
/// Fade-in alpha step per 30 Hz frame (`DAT_0116d7d0 += 4` at `0x001c4de0`).
pub const GAMEOVER_FADE_IN_STEP: u8 = 4;
/// Maximum PS2 GS alpha (`0x60 = 96` in 0..128 GS scale at `0x001c4de0`).
pub const GAMEOVER_MAX_PS2_ALPHA: u8 = 0x60;
/// Hold duration in 30 Hz frames at full alpha (`DAT_0116d7d2 == 0x40 = 64` at `0x001c4de0`).
pub const GAMEOVER_HOLD_FRAMES: u32 = 64;
/// Fade-out alpha step per 30 Hz frame (`DAT_0116d7d0 -= 8` at `0x001c4de0`).
pub const GAMEOVER_FADE_OUT_STEP: u8 = 8;
/// Number of background jitter strips in `draw_opd_wowk_main` (`iVar4 < 6` at `0x001c4de0`).
pub const GAMEOVER_STRIP_COUNT: usize = 6;
/// Strip vertical jitter range `rand() % 30 - 15` (`0x1e` / `-0xf` at `0x001c4de0`).
pub const GAMEOVER_STRIP_JITTER_RANGE: i32 = 15;

/// Phase of `draw_opd_wowk_main` (`DAT_0116d7d1` at `0x001c4de0`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GameOverPhase {
    /// `DAT_0116d7d1 == 0`: waiting `32` 30 Hz frames before fade-in.
    #[default]
    DelayBeforeFadeIn = 0,
    /// `DAT_0116d7d1 == 1`: fading in `alpha += 4` until `0x60`.
    FadeIn = 1,
    /// `DAT_0116d7d1 == 2`: holding for `64` 30 Hz frames with strip jitter.
    Hold = 2,
    /// `DAT_0116d7d1 == 3`: fading out `alpha -= 8` until `0`.
    FadeOut = 3,
    /// `DAT_0116d7d1 == 4`: finished, ready to return to Title.
    Done = 4,
}

/// Interactive state for the Game Over screen (`GameoverMain` `0x001c4c70`).
#[derive(Debug, Clone)]
pub struct GameOverScreen {
    /// Current phase (`DAT_0116d7d1`).
    pub phase: GameOverPhase,
    /// Current PS2 alpha `0..=0x60` (`DAT_0116d7d0`).
    pub ps2_alpha: u8,
    /// Frame counter inside `DelayBeforeFadeIn` / `Hold` (`DAT_0116d7d2`).
    pub frame_counter: u32,
    /// Accumulator for 30 Hz frame stepping from arbitrary `dt_sec`.
    pub tick_accum: f32,
    /// Deterministic LCG state for `rand() % 30 - 15` strip jitter.
    pub rng_state: u32,
    /// Cached per-strip vertical jitter offsets in PS2 pixels (`-15..=14`).
    pub strip_jitter: [f32; GAMEOVER_STRIP_COUNT],
}

impl Default for GameOverScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl GameOverScreen {
    /// Creates a new [`GameOverScreen`] at `DelayBeforeFadeIn` (`0x001c4de0`).
    #[must_use]
    pub const fn new() -> Self {
        Self {
            phase: GameOverPhase::DelayBeforeFadeIn,
            ps2_alpha: 0,
            frame_counter: 0,
            tick_accum: 0.0,
            rng_state: 0x1357_2468,
            strip_jitter: [0.0; GAMEOVER_STRIP_COUNT],
        }
    }

    /// Creates a [`GameOverScreen`] immediately at full `Hold` visibility (for preview/snapshot tests).
    #[must_use]
    pub const fn at_peak_hold() -> Self {
        Self {
            phase: GameOverPhase::Hold,
            ps2_alpha: GAMEOVER_MAX_PS2_ALPHA,
            frame_counter: 0,
            tick_accum: 0.0,
            rng_state: 0x1357_2468,
            strip_jitter: [-4.0, 6.0, -9.0, 3.0, -7.0, 5.0],
        }
    }

    /// Advances the Game Over state machine by `dt_sec` (stepped at 30 Hz PS2 rate).
    pub fn tick(&mut self, dt_sec: f32) {
        self.tick_accum += dt_sec.max(0.0);
        let step_30hz = 1.0 / 30.0;
        while self.tick_accum >= step_30hz {
            self.tick_accum -= step_30hz;
            self.step_30hz_frame();
        }
    }

    /// Advances exactly one 30 Hz frame of `draw_opd_wowk_main` (`0x001c4de0`).
    pub fn step_30hz_frame(&mut self) {
        match self.phase {
            GameOverPhase::DelayBeforeFadeIn => {
                self.frame_counter += 1;
                if self.frame_counter >= GAMEOVER_DELAY_FRAMES {
                    self.phase = GameOverPhase::FadeIn;
                    self.frame_counter = 0;
                }
            }
            GameOverPhase::FadeIn => {
                self.ps2_alpha = self.ps2_alpha.saturating_add(GAMEOVER_FADE_IN_STEP);
                if self.ps2_alpha >= GAMEOVER_MAX_PS2_ALPHA {
                    self.ps2_alpha = GAMEOVER_MAX_PS2_ALPHA;
                    self.phase = GameOverPhase::Hold;
                    self.frame_counter = 0;
                }
            }
            GameOverPhase::Hold => {
                self.frame_counter += 1;
                if self.frame_counter >= GAMEOVER_HOLD_FRAMES {
                    self.phase = GameOverPhase::FadeOut;
                    self.frame_counter = 0;
                }
            }
            GameOverPhase::FadeOut => {
                self.ps2_alpha = self.ps2_alpha.saturating_sub(GAMEOVER_FADE_OUT_STEP);
                if self.ps2_alpha == 0 {
                    self.phase = GameOverPhase::Done;
                }
            }
            GameOverPhase::Done => {}
        }

        // Update strip jitter (`rand() % 30 - 15`) when `phase >= Hold` (`0x001c4de0`).
        if (self.phase as u8) >= (GameOverPhase::Hold as u8) && self.phase != GameOverPhase::Done {
            for slot in &mut self.strip_jitter {
                self.rng_state = self
                    .rng_state
                    .wrapping_mul(1_664_525)
                    .wrapping_add(1_013_904_223);
                let val = ((self.rng_state >> 16) % 30) as i32 - GAMEOVER_STRIP_JITTER_RANGE;
                *slot = val as f32;
            }
        }
    }

    /// Allows the player to skip straight to fade-out / done on confirm input (`FUN_00497ad0`).
    pub fn request_skip(&mut self) {
        match self.phase {
            GameOverPhase::DelayBeforeFadeIn | GameOverPhase::FadeIn | GameOverPhase::Hold => {
                self.phase = GameOverPhase::FadeOut;
            }
            GameOverPhase::FadeOut | GameOverPhase::Done => {
                self.ps2_alpha = 0;
                self.phase = GameOverPhase::Done;
            }
        }
    }

    /// Converts PS2 `0..=128` GS alpha (`0x00..=0x60`) into `0..=255` RGBA alpha.
    #[must_use]
    pub fn rgba_alpha(&self) -> u8 {
        ((u16::from(self.ps2_alpha) * 255) / 128).min(255) as u8
    }

    /// Returns `true` when the Game Over sequence has completed (`GameoverMain` returns `1`).
    #[must_use]
    pub const fn is_done(&self) -> bool {
        matches!(self.phase, GameOverPhase::Done)
    }
}
