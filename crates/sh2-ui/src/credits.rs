//! Epilogue Mary's Letter Overlay & Per-Ending Staff Credits Roll (`sh2-ui::credits`).
//!
//! Consumes [`sh2_core::endings`] epilogue/credits descriptors (`0x0043ed20..0x005a12d0` in `sh2pc.exe`)
//! and renders Mary's full epilogue letter reading, scrolling Team Silent staff credits, Dog ending
//! blooper-reel credits, UFO ending silent-film slides (`hj01..07.tex` -> `staff01..07.tex`), and
//! *Born from a Wish* credits (*Love Psalm*).

#![forbid(unsafe_code)]

use sh2_core::endings::{
    active_mary_letter_stanza, credits_descriptor_for_ending, CreditsMusicTheme,
    CreditsPresentationKind, EndingCreditsDescriptor, GameEnding, StaffCreditsSection,
    UfoSlideShowPlayer, MARY_FULL_LETTER_STANZAS, TEAM_SILENT_STAFF_SECTIONS,
};

use crate::atlas_font::{AtlasFont, Sh2TextColor};
use crate::font::TextRasterizer;

/// Total duration of Mary's complete epilogue letter reading (`196.0` s in `voice.afs[97]`).
pub const MARY_LETTER_TOTAL_DURATION_SECS: f32 = 196.0;
/// Total duration of the scrolling Silent Hill 2 staff credits roll (`150.0` s in `credits.bik`).
pub const STAFF_CREDITS_SCROLL_DURATION_SECS: f32 = 150.0;
/// Total duration of the Dog ending comedic control-room / blooper FMV (`125.0` s in `end_dog.bik`).
pub const DOG_ENDING_BLOOPER_DURATION_SECS: f32 = 125.0;
/// Total duration of the *Born from a Wish* epilogue FMV (`90.0` s in `end_wish.bik`).
pub const BORN_FROM_A_WISH_EPILOGUE_SECS: f32 = 90.0;
/// Total duration of the *Rebirth* boat ritual FMV (`85.0` s in `end_rbrt.bik`).
pub const REBIRTH_RITUAL_FMV_DURATION_SECS: f32 = 85.0;
/// Total duration of the *In Water* car plunge prelude (`18.0` s in `water.bik` at `0x0043efa9`).
pub const IN_WATER_PRELUDE_DURATION_SECS: f32 = 18.0;

/// Lifecycle phase of an ending's epilogue & staff credits presentation (`0x0043ed20..0x005a12d0`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CreditsSequencePhase {
    /// Inactive prior to ending trigger.
    Idle,
    /// Optional pre-letter FMV (`"water.bik"` on *In Water*).
    PreludeFmv {
        /// Elapsed playback time in seconds.
        elapsed_secs: f32,
        /// Target duration in seconds.
        duration_secs: f32,
    },
    /// Full Mary's letter voiceover & stanza overlay (*Leave*, *Maria*, *In Water*).
    MaryLetterReading {
        /// Elapsed reading time in seconds (`0.0..MARY_LETTER_TOTAL_DURATION_SECS`).
        elapsed_secs: f32,
    },
    /// Non-letter epilogue FMV (*Rebirth* `end_rbrt.bik`, *Dog* `end_dog.bik`, *Born from a Wish* `end_wish.bik`).
    EpilogueScene {
        /// Elapsed playback time in seconds.
        elapsed_secs: f32,
        /// Scene duration in seconds.
        duration_secs: f32,
    },
    /// Vertical scrolling staff credits roll (`credits.bik` / `end.bik` + UI crawl).
    ScrollingStaffRoll {
        /// Elapsed scroll time in seconds (`0.0..STAFF_CREDITS_SCROLL_DURATION_SECS`).
        elapsed_secs: f32,
    },
    /// UFO ending 35-slide comic & `staff01..07.tex` sequence (`EvProgUfoMado` `0x00578c50`).
    UfoSlideShow(UfoSlideShowPlayer),
    /// Credits finished; ready to transition to the 10-Star Result Screen (`0x004474d0`).
    Finished,
}

/// Stateful UI controller for Silent Hill 2 epilogue letter readings and per-ending staff credits.
#[derive(Debug, Clone, PartialEq)]
pub struct EndingCreditsSequence {
    /// Active ending descriptor.
    pub descriptor: EndingCreditsDescriptor,
    /// Current playback phase.
    pub phase: CreditsSequencePhase,
}

impl EndingCreditsSequence {
    /// Starts the epilogue and staff credits sequence for `ending`.
    pub fn start(ending: GameEnding) -> Self {
        let descriptor = credits_descriptor_for_ending(ending);
        let phase = match descriptor.presentation {
            CreditsPresentationKind::MaryLetterThenScrollingRoll => {
                if descriptor.prelude_fmv.is_some() {
                    CreditsSequencePhase::PreludeFmv {
                        elapsed_secs: 0.0,
                        duration_secs: IN_WATER_PRELUDE_DURATION_SECS,
                    }
                } else {
                    CreditsSequencePhase::MaryLetterReading { elapsed_secs: 0.0 }
                }
            }
            CreditsPresentationKind::RitualSceneThenScrollingRoll => {
                CreditsSequencePhase::EpilogueScene {
                    elapsed_secs: 0.0,
                    duration_secs: REBIRTH_RITUAL_FMV_DURATION_SECS,
                }
            }
            CreditsPresentationKind::DogBlooperReel => CreditsSequencePhase::EpilogueScene {
                elapsed_secs: 0.0,
                duration_secs: DOG_ENDING_BLOOPER_DURATION_SECS,
            },
            CreditsPresentationKind::UfoSilentFilmSlides => {
                CreditsSequencePhase::UfoSlideShow(UfoSlideShowPlayer::new())
            }
            CreditsPresentationKind::BornFromAWishRoll => CreditsSequencePhase::EpilogueScene {
                elapsed_secs: 0.0,
                duration_secs: BORN_FROM_A_WISH_EPILOGUE_SECS,
            },
        };
        Self { descriptor, phase }
    }

    /// Returns the active [`CreditsMusicTheme`] paired with this ending's credits roll.
    pub const fn music_theme(&self) -> CreditsMusicTheme {
        self.descriptor.music_theme
    }

    /// Returns `true` when the entire epilogue + credits sequence has finished.
    pub const fn is_finished(&self) -> bool {
        matches!(self.phase, CreditsSequencePhase::Finished)
    }

    /// Returns the currently visible Mary letter stanza text during [`CreditsSequencePhase::MaryLetterReading`].
    pub fn current_letter_text(&self) -> Option<&'static str> {
        match self.phase {
            CreditsSequencePhase::MaryLetterReading { elapsed_secs } => {
                active_mary_letter_stanza(elapsed_secs).map(|s| s.text)
            }
            _ => None,
        }
    }

    /// Normalized `[0.0, 1.0]` scroll progress during [`CreditsSequencePhase::ScrollingStaffRoll`].
    pub fn scroll_progress(&self) -> Option<f32> {
        match self.phase {
            CreditsSequencePhase::ScrollingStaffRoll { elapsed_secs } => {
                Some((elapsed_secs / STAFF_CREDITS_SCROLL_DURATION_SECS).clamp(0.0, 1.0))
            }
            _ => None,
        }
    }

    /// Advances the epilogue / credits state machine (`skip_phase` skips the current sub-phase on Start/Confirm,
    /// matching `0x0043ee00` / `0x0059f9b7` / `0x00578c53`).
    pub fn tick(&mut self, dt_secs: f32, skip_phase: bool) {
        let dt = dt_secs.max(0.0);
        match &mut self.phase {
            CreditsSequencePhase::Idle | CreditsSequencePhase::Finished => {}
            CreditsSequencePhase::PreludeFmv {
                elapsed_secs,
                duration_secs,
            } => {
                *elapsed_secs += dt;
                if skip_phase || *elapsed_secs >= *duration_secs {
                    self.phase = CreditsSequencePhase::MaryLetterReading { elapsed_secs: 0.0 };
                }
            }
            CreditsSequencePhase::MaryLetterReading { elapsed_secs } => {
                *elapsed_secs += dt;
                let end_secs = MARY_FULL_LETTER_STANZAS
                    .last()
                    .map(|s| s.end_secs)
                    .unwrap_or(MARY_LETTER_TOTAL_DURATION_SECS);
                if skip_phase || *elapsed_secs >= end_secs {
                    self.phase = CreditsSequencePhase::ScrollingStaffRoll { elapsed_secs: 0.0 };
                }
            }
            CreditsSequencePhase::EpilogueScene {
                elapsed_secs,
                duration_secs,
            } => {
                *elapsed_secs += dt;
                if skip_phase || *elapsed_secs >= *duration_secs {
                    self.phase = CreditsSequencePhase::ScrollingStaffRoll { elapsed_secs: 0.0 };
                }
            }
            CreditsSequencePhase::ScrollingStaffRoll { elapsed_secs } => {
                *elapsed_secs += dt;
                if skip_phase || *elapsed_secs >= STAFF_CREDITS_SCROLL_DURATION_SECS {
                    self.phase = CreditsSequencePhase::Finished;
                }
            }
            CreditsSequencePhase::UfoSlideShow(player) => {
                player.tick(dt, skip_phase);
                if player.finished {
                    self.phase = CreditsSequencePhase::Finished;
                }
            }
        }
    }

    /// Renders the current epilogue letter stanza or scrolling staff credits overlay into `rast`.
    pub fn render(&self, rast: &mut TextRasterizer, font: &AtlasFont) {
        match &self.phase {
            CreditsSequencePhase::MaryLetterReading { elapsed_secs } => {
                if let Some(stanza) = active_mary_letter_stanza(*elapsed_secs) {
                    draw_multiline_centered(
                        rast,
                        font,
                        stanza.text,
                        rast.height * 2 / 3,
                        22,
                        Sh2TextColor::White,
                    );
                }
            }
            CreditsSequencePhase::ScrollingStaffRoll { elapsed_secs } => {
                render_scrolling_staff_sections(
                    rast,
                    font,
                    &TEAM_SILENT_STAFF_SECTIONS,
                    self.descriptor.music_theme.title(),
                    *elapsed_secs,
                );
            }
            CreditsSequencePhase::UfoSlideShow(player) => {
                let caption = player.current_texture_path();
                font.draw_line_centered(
                    rast,
                    caption,
                    (rast.width / 2, rast.height.saturating_sub(36)),
                    18,
                    Sh2TextColor::MutedFooter,
                    true,
                );
            }
            CreditsSequencePhase::Idle
            | CreditsSequencePhase::PreludeFmv { .. }
            | CreditsSequencePhase::EpilogueScene { .. }
            | CreditsSequencePhase::Finished => {}
        }
    }
}

fn draw_multiline_centered(
    rast: &mut TextRasterizer,
    font: &AtlasFont,
    text: &str,
    start_y: usize,
    px_height: usize,
    color: Sh2TextColor,
) {
    let line_step = px_height + 8;
    for (idx, line) in text.lines().enumerate() {
        let y = start_y.saturating_add(idx * line_step);
        if y + px_height <= rast.height {
            font.draw_line_centered(rast, line, (rast.width / 2, y), px_height, color, true);
        }
    }
}

fn render_scrolling_staff_sections(
    rast: &mut TextRasterizer,
    font: &AtlasFont,
    sections: &[StaffCreditsSection],
    music_title: &str,
    elapsed_secs: f32,
) {
    let progress = (elapsed_secs / STAFF_CREDITS_SCROLL_DURATION_SECS).clamp(0.0, 1.0);
    let total_virtual_h = (rast.height + sections.len() * 110 + 160) as f32;
    let scroll_px = (progress * total_virtual_h) as isize;
    let center_x = rast.width / 2;

    let mut cursor_y = rast.height as isize - scroll_px;
    for section in sections {
        if cursor_y >= 0 && (cursor_y as usize) + 20 <= rast.height {
            font.draw_line_centered(
                rast,
                section.role,
                (center_x, cursor_y as usize),
                18,
                Sh2TextColor::AmberHeader,
                true,
            );
        }
        cursor_y += 28;
        for name in section.names {
            if cursor_y >= 0 && (cursor_y as usize) + 22 <= rast.height {
                font.draw_line_centered(
                    rast,
                    name,
                    (center_x, cursor_y as usize),
                    22,
                    Sh2TextColor::White,
                    true,
                );
            }
            cursor_y += 28;
        }
        cursor_y += 28;
    }

    if cursor_y >= 0 && (cursor_y as usize) + 20 <= rast.height {
        font.draw_line_centered(
            rast,
            music_title,
            (center_x, cursor_y as usize),
            18,
            Sh2TextColor::SilverUnselected,
            true,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_water_runs_prelude_then_mary_letter_then_angels_thanatos_credits() {
        let mut seq = EndingCreditsSequence::start(GameEnding::InWater);
        assert_eq!(seq.music_theme(), CreditsMusicTheme::AngelsThanatos);
        assert!(matches!(seq.phase, CreditsSequencePhase::PreludeFmv { .. }));

        seq.tick(20.0, false);
        assert!(matches!(
            seq.phase,
            CreditsSequencePhase::MaryLetterReading { .. }
        ));
        assert!(seq
            .current_letter_text()
            .expect("stanza active")
            .contains("In my restless dreams"));

        let font = AtlasFont::load_embedded();
        let mut rast = TextRasterizer::new(640, 480);
        seq.render(&mut rast, &font);
        assert!(rast.pixels.iter().any(|&b| b != 0));

        seq.tick(200.0, false);
        assert!(matches!(
            seq.phase,
            CreditsSequencePhase::ScrollingStaffRoll { .. }
        ));
        seq.tick(30.0, false);
        let mut credits_rast = TextRasterizer::new(640, 480);
        seq.render(&mut credits_rast, &font);
        assert!(credits_rast.pixels.iter().any(|&b| b != 0));

        seq.tick(130.0, false);
        assert!(seq.is_finished());
    }

    #[test]
    fn ufo_and_dog_and_wish_sequences_pair_correct_themes_and_complete() {
        let mut ufo = EndingCreditsSequence::start(GameEnding::Ufo);
        assert_eq!(
            ufo.music_theme(),
            CreditsMusicTheme::SilentHillUfoArrangement
        );
        ufo.tick(90.0, false);
        assert!(ufo.is_finished());

        let mut dog = EndingCreditsSequence::start(GameEnding::Dog);
        assert_eq!(dog.music_theme(), CreditsMusicTheme::DogBlooperPolka);
        dog.tick(5.0, true);
        assert!(matches!(
            dog.phase,
            CreditsSequencePhase::ScrollingStaffRoll { .. }
        ));
        dog.tick(1.0, true);
        assert!(dog.is_finished());

        let wish = EndingCreditsSequence::start(GameEnding::BornFromAWish);
        assert_eq!(wish.music_theme(), CreditsMusicTheme::LovePsalm);
    }
}
