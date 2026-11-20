//! Boot Logo / Warning Splash Sequence (`SLUS_202.28` `logo.c` `0x0020c250..0x0020d6a0`
//! and retail PC `sh2pc.exe` `0x00405c50..0x004062a0`).
//!
//! Prototype (`SLUS_202.28`) vs Retail PC (`sh2pc.exe`) drift:
//! - `SLUS_202.28` (`logoDrawWarningCESA` `0x0020c910`, `logoDrawWarningSCE` `0x0020cda0`,
//!   `logoDrawKonamiLogo` `0x0020d220`): displays CESA -> SCE -> Konami (`cesa.tex`, `sce.tex`,
//!   `konami_r.tex`) with `0.3s` fade-in (`0x3e99999a`), `3.0s` hold, `0.3s` fade-out, unskippable.
//! - `sh2pc.exe` (`0x00405e80`, `0x00406280`, `0x004062a0`): skips CESA/SCE and displays
//!   Konami (`konami2048x2048.tex`) -> Splash (`splash_logo.tex`) with `1.0s` fade-in, `2.0s` hold,
//!   `1.0s` fade-out (`0.5s` fast fade-out when skipped during hold).

use serde::{Deserialize, Serialize};

/// PS2 `logo.c` (`0x0020c910..0x0020d220`) fade duration in seconds (`0x3e99999a` = `0.3f`).
pub const PS2_LOGO_FADE_SECS: f32 = 0.3;

/// PS2 `logo.c` (`0x0020c910..0x0020d220`) hold duration in seconds (`3.0f`).
pub const PS2_LOGO_HOLD_SECS: f32 = 3.0;

/// Retail PC `sh2pc.exe` (`0x00405e80`) fade-in and normal fade-out duration (`1.0f`).
pub const PC_LOGO_FADE_SECS: f32 = 1.0;

/// Retail PC `sh2pc.exe` (`0x00405fcb`) hold duration (`2.0f`).
pub const PC_LOGO_HOLD_SECS: f32 = 2.0;

/// Retail PC `sh2pc.exe` (`0x00405e80`) fast fade-out duration when skipped during hold (`0.5f`).
pub const PC_LOGO_SKIP_FADE_SECS: f32 = 0.5;

/// Boot sequence profile selecting between PS2 original (`logo.c`) and Retail PC (`sh2pc.exe`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum BootProfile {
    /// Strict 1:1 PS2 `logo.c` sequence: CESA -> SCE -> Konami (`3.0s` hold, `0.3s` fade).
    #[default]
    Ps2Original,
    /// Retail PC `sh2pc.exe` (`0x00405e80`) sequence: Konami -> Splash (`2.0s` hold, `1.0s` fade, skippable).
    RetailPc,
}

/// Individual boot logo screens in `logo.c` / `sh2pc.exe`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BootLogoKind {
    /// `logoDrawWarningCESA` (`0x0020c910`): `data/pic/etc/cesa.tex`.
    CesaWarning,
    /// `logoDrawWarningSCE` (`0x0020cda0`): `data/pic/etc/sce.tex`.
    SceWarning,
    /// `logoDrawKonamiLogo` (`0x0020d220`) / `sh2pc` `0x00406280`: `konami_r.tex` / `konami2048x2048.tex`.
    KonamiLogo,
    /// `sh2pc` `0x004062a0`: `data/pic/etc/splash_logo.tex` (KCET / Creature Labs / Ratings).
    SplashLogo,
}

impl BootLogoKind {
    /// Retail asset relative path under `data/pic/etc/`.
    #[must_use]
    pub const fn asset_path(self) -> &'static str {
        match self {
            Self::CesaWarning => "pic/etc/cesa.tex",
            Self::SceWarning => "pic/etc/sce.tex",
            Self::KonamiLogo => "pic/etc/konami2048x2048.tex",
            Self::SplashLogo => "pic/etc/splash_logo.tex",
        }
    }

    /// Fallback display caption when `.tex` artwork is not loaded.
    #[must_use]
    pub const fn fallback_title(self) -> &'static str {
        match self {
            Self::CesaWarning => "THIS GAME CONTAINS SCENES OF EXPLICIT VIOLENCE AND GORE.",
            Self::SceWarning => "SONY COMPUTER ENTERTAINMENT",
            Self::KonamiLogo => "KONAMI",
            Self::SplashLogo => "KONAMI COMPUTER ENTERTAINMENT TOKYO",
        }
    }
}

/// Per-logo fade/hold sub-step matching `logo.c` `_DAT_010f0888` (`1..=4`) and `sh2pc` `0x00405e80`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BootLogoStage {
    /// Fading in from black (`ScreenEffectFadeStart(fade_secs, 4)`).
    #[default]
    FadeIn,
    /// Holding at full opacity (`_logoDispTimer` countdown).
    Hold,
    /// Fading out to black (`ScreenEffectFadeStart(fade_out_secs, 1|2)`).
    FadeOut,
    /// All boot logos finished; ready to enter `TitleMain`.
    Complete,
}

/// Deterministic state machine for the boot logo sequence (`logo.c` / `sh2pc` `0x00405e80`).
#[derive(Debug, Clone, PartialEq)]
pub struct BootSequenceState {
    pub profile: BootProfile,
    pub logo_index: usize,
    pub stage: BootLogoStage,
    /// Elapsed seconds within the current [`BootLogoStage`].
    pub stage_timer: f32,
    /// Current fade opacity in `[0.0, 1.0]`.
    pub alpha: f32,
    /// Active fade-out duration (supports PC `0.5s` fast skip fade-out).
    pub active_fade_out_secs: f32,
}

impl Default for BootSequenceState {
    fn default() -> Self {
        Self::new(BootProfile::Ps2Original)
    }
}

impl BootSequenceState {
    /// Creates a new boot sequence starting at logo 0 in [`BootLogoStage::FadeIn`].
    #[must_use]
    pub fn new(profile: BootProfile) -> Self {
        let fade_out = match profile {
            BootProfile::Ps2Original => PS2_LOGO_FADE_SECS,
            BootProfile::RetailPc => PC_LOGO_FADE_SECS,
        };
        Self {
            profile,
            logo_index: 0,
            stage: BootLogoStage::FadeIn,
            stage_timer: 0.0,
            alpha: 0.0,
            active_fade_out_secs: fade_out,
        }
    }

    /// Ordered logos for the active [`BootProfile`].
    #[must_use]
    pub const fn sequence(profile: BootProfile) -> &'static [BootLogoKind] {
        match profile {
            BootProfile::Ps2Original => &[
                BootLogoKind::CesaWarning,
                BootLogoKind::SceWarning,
                BootLogoKind::KonamiLogo,
            ],
            BootProfile::RetailPc => &[BootLogoKind::KonamiLogo, BootLogoKind::SplashLogo],
        }
    }

    /// Currently active [`BootLogoKind`], or `None` once [`BootLogoStage::Complete`].
    #[must_use]
    pub fn current_logo(&self) -> Option<BootLogoKind> {
        if self.stage == BootLogoStage::Complete {
            return None;
        }
        Self::sequence(self.profile).get(self.logo_index).copied()
    }

    /// Fade-in duration in seconds for the active profile.
    #[must_use]
    pub const fn fade_in_secs(&self) -> f32 {
        match self.profile {
            BootProfile::Ps2Original => PS2_LOGO_FADE_SECS,
            BootProfile::RetailPc => PC_LOGO_FADE_SECS,
        }
    }

    /// Hold duration in seconds for the active profile.
    #[must_use]
    pub const fn hold_secs(&self) -> f32 {
        match self.profile {
            BootProfile::Ps2Original => PS2_LOGO_HOLD_SECS,
            BootProfile::RetailPc => PC_LOGO_HOLD_SECS,
        }
    }

    /// Advances the boot logo state machine by `dt` seconds. Returns `true` when completed.
    pub fn tick(&mut self, dt: f32) -> bool {
        if self.stage == BootLogoStage::Complete {
            return true;
        }
        let dt = dt.max(0.0);
        self.stage_timer += dt;

        match self.stage {
            BootLogoStage::FadeIn => {
                let dur = self.fade_in_secs().max(f32::EPSILON);
                self.alpha = (self.stage_timer / dur).clamp(0.0, 1.0);
                if self.stage_timer >= dur {
                    self.stage = BootLogoStage::Hold;
                    self.stage_timer = 0.0;
                    self.alpha = 1.0;
                }
            }
            BootLogoStage::Hold => {
                self.alpha = 1.0;
                if self.stage_timer >= self.hold_secs() {
                    self.stage = BootLogoStage::FadeOut;
                    self.stage_timer = 0.0;
                    self.active_fade_out_secs = self.fade_in_secs();
                }
            }
            BootLogoStage::FadeOut => {
                let dur = self.active_fade_out_secs.max(f32::EPSILON);
                self.alpha = (1.0 - self.stage_timer / dur).clamp(0.0, 1.0);
                if self.stage_timer >= dur {
                    self.advance_to_next_logo();
                }
            }
            BootLogoStage::Complete => {}
        }

        self.stage == BootLogoStage::Complete
    }

    /// Handles a skip button press (`sh2pc.exe` `0x00405e80` fast skip vs PS2 `logo.c`).
    ///
    /// In `RetailPc` mode, matches `0x00405e80`: during `FadeIn`/`FadeOut` snaps fade immediately;
    /// during `Hold` triggers `0.5s` fast fade-out. In `Ps2Original` mode, returns `false` unless
    /// `allow_ps2_skip` is set.
    pub fn handle_skip(&mut self, allow_ps2_skip: bool) -> bool {
        if self.stage == BootLogoStage::Complete {
            return false;
        }
        if self.profile == BootProfile::Ps2Original && !allow_ps2_skip {
            return false;
        }
        match self.stage {
            BootLogoStage::FadeIn => {
                self.stage = BootLogoStage::Hold;
                self.stage_timer = 0.0;
                self.alpha = 1.0;
                true
            }
            BootLogoStage::Hold => {
                self.stage = BootLogoStage::FadeOut;
                self.stage_timer = 0.0;
                self.active_fade_out_secs = if self.profile == BootProfile::RetailPc {
                    PC_LOGO_SKIP_FADE_SECS
                } else {
                    PS2_LOGO_FADE_SECS
                };
                true
            }
            BootLogoStage::FadeOut => {
                self.advance_to_next_logo();
                true
            }
            BootLogoStage::Complete => false,
        }
    }

    fn advance_to_next_logo(&mut self) {
        self.logo_index += 1;
        if self.logo_index >= Self::sequence(self.profile).len() {
            self.stage = BootLogoStage::Complete;
            self.alpha = 0.0;
            self.stage_timer = 0.0;
        } else {
            self.stage = BootLogoStage::FadeIn;
            self.stage_timer = 0.0;
            self.alpha = 0.0;
            self.active_fade_out_secs = self.fade_in_secs();
        }
    }
}
