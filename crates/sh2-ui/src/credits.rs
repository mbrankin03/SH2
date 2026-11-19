//! Epilogue Mary's Letter Upward Scroll & Per-Ending Staff Credits Roll (`sh2-ui::credits`).
//!
//! Consumes [`sh2_core::endings`] epilogue/credits descriptors (`0x0043ed20..0x005a12d0` and
//! `0x00482f60..0x004832e4` in `sh2pc.exe`) with exact retail `.bik` frame/FPS durations and
//! authentic `letter*_msg_e.mes` (`0xf8XX` spacing @ `16.5` px/s) upward scroll presentation.

#![forbid(unsafe_code)]

use sh2_core::endings::{
    credits_descriptor_for_ending, letter_scroll_total_duration_secs, visible_letter_entries_at,
    CreditsMusicTheme, CreditsPresentationKind, EndingCreditsDescriptor, GameEnding,
    MaryLetterScrollEntry, StaffCreditsSection, UfoSlideShowPlayer, LETTER_MSG_E_ENTRIES,
    LETTER_W1_MSG_E_ENTRIES, LETTER_W2_MSG_E_ENTRIES, MARIA_LETTER_VOICE_DURATION_SECS,
    TEAM_SILENT_STAFF_SECTIONS,
};
pub use sh2_core::endings::{
    CREDITS_BIK_DURATION_SECS, END_BIK_DURATION_SECS, END_DOG_BIK_DURATION_SECS,
    END_WISH_BIK_DURATION_SECS, IN_WATER_BEDSIDE_DURATION_SECS,
    IN_WATER_CLOSING_SCROLL_DURATION_SECS, WATER_BIK_DURATION_SECS,
};

use crate::atlas_font::{AtlasFont, Sh2TextColor};
use crate::font::TextRasterizer;

/// Lifecycle phase of an ending's epilogue & staff credits presentation (`0x0043ed20..0x005a12d0`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CreditsSequencePhase {
    /// Inactive prior to ending trigger.
    Idle,
    /// *In Water* `36.0` s (`1080.0` ticks @ 30 Hz `[0x006388a4]`) bedside scene (`0x0043ee27..0x0043ef9c`).
    InWaterBedside {
        /// Elapsed bedside time in seconds.
        elapsed_secs: f32,
    },
    /// *In Water* `water.bik` (`8700` frames @ `30000/1001` fps = `290.290` s at `0x0043efa9`) with `letter_w1_msg_e.mes` overlay.
    PreludeFmv {
        /// Elapsed playback time in seconds.
        elapsed_secs: f32,
        /// Exact `water.bik` duration (`WATER_BIK_DURATION_SECS`).
        duration_secs: f32,
    },
    /// Upward `.mes` letter scroll (`16.5` px/s at `0x006388a0`, `0x00482f60..0x004832e4`) paired with `voice.afs` stream.
    MaryLetterScroll {
        /// Active `.mes` entries (`LETTER_MSG_E_ENTRIES`, `LETTER_W1_MSG_E_ENTRIES`, or `LETTER_W2_MSG_E_ENTRIES`).
        entries: &'static [MaryLetterScrollEntry],
        /// Elapsed scroll time in seconds.
        elapsed_secs: f32,
        /// Phase completion duration in seconds.
        duration_secs: f32,
        /// Optional `voice.afs` stream ID (`0xEAE1`, `0xEA64`, or `0xEAB2`).
        voice_stream_id: Option<u32>,
        /// Trigger timestamp (`0.8` s Leave, `0.0` s Maria, `15.0` s In Water closing scroll).
        voice_trigger_secs: f32,
        /// Latched `true` once `voice_stream_id` has fired.
        voice_triggered: bool,
    },
    /// Epilogue Bink FMV (`end.bik` tracks `0..=3` `230.230` s, `end_dog.bik` `139.138` s, `end_wish.bik` `187.187` s).
    EpilogueScene {
        /// Elapsed playback time in seconds.
        elapsed_secs: f32,
        /// Exact retail `.bik` duration in seconds.
        duration_secs: f32,
    },
    /// Vertical scrolling staff credits roll (`credits.bik`, `2788` frames @ `2997/100` fps = `93.02636` s).
    ScrollingStaffRoll {
        /// Elapsed scroll time in seconds (`0.0..CREDITS_BIK_DURATION_SECS`).
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
    /// Most recently triggered `voice.afs` stream ID (`0xEAE1`, `0xEA64`, `0xEAB2`).
    pub last_triggered_voice_stream: Option<u32>,
}

impl EndingCreditsSequence {
    /// Starts the epilogue and staff credits sequence for `ending`.
    pub fn start(ending: GameEnding) -> Self {
        let descriptor = credits_descriptor_for_ending(ending);
        let phase = match descriptor.presentation {
            CreditsPresentationKind::MaryLetterThenScrollingRoll => match ending {
                GameEnding::InWater => CreditsSequencePhase::InWaterBedside { elapsed_secs: 0.0 },
                GameEnding::Maria => CreditsSequencePhase::MaryLetterScroll {
                    entries: LETTER_W1_MSG_E_ENTRIES,
                    elapsed_secs: 0.0,
                    duration_secs: MARIA_LETTER_VOICE_DURATION_SECS,
                    voice_stream_id: descriptor.letter_voice_stream_id,
                    voice_trigger_secs: descriptor.letter_voice_delay_secs,
                    voice_triggered: false,
                },
                _ => CreditsSequencePhase::MaryLetterScroll {
                    entries: &LETTER_MSG_E_ENTRIES,
                    elapsed_secs: 0.0,
                    duration_secs: letter_scroll_total_duration_secs(&LETTER_MSG_E_ENTRIES),
                    voice_stream_id: descriptor.letter_voice_stream_id,
                    voice_trigger_secs: descriptor.letter_voice_delay_secs,
                    voice_triggered: false,
                },
            },
            CreditsPresentationKind::RitualSceneThenScrollingRoll => {
                CreditsSequencePhase::EpilogueScene {
                    elapsed_secs: 0.0,
                    duration_secs: END_BIK_DURATION_SECS,
                }
            }
            CreditsPresentationKind::DogBlooperReel => CreditsSequencePhase::EpilogueScene {
                elapsed_secs: 0.0,
                duration_secs: END_DOG_BIK_DURATION_SECS,
            },
            CreditsPresentationKind::UfoSilentFilmSlides => {
                CreditsSequencePhase::UfoSlideShow(UfoSlideShowPlayer::new())
            }
            CreditsPresentationKind::BornFromAWishRoll => CreditsSequencePhase::EpilogueScene {
                elapsed_secs: 0.0,
                duration_secs: END_WISH_BIK_DURATION_SECS,
            },
        };
        Self {
            descriptor,
            phase,
            last_triggered_voice_stream: None,
        }
    }

    /// Returns the active [`CreditsMusicTheme`] paired with this ending's credits roll.
    pub const fn music_theme(&self) -> CreditsMusicTheme {
        self.descriptor.music_theme
    }

    /// Returns `true` when the entire epilogue + credits sequence has finished.
    pub const fn is_finished(&self) -> bool {
        matches!(self.phase, CreditsSequencePhase::Finished)
    }

    /// Returns the top-most visible `.mes` stanza text during letter scroll / `water.bik` overlay (`512` px viewport).
    pub fn current_letter_text(&self) -> Option<&'static str> {
        match self.phase {
            CreditsSequencePhase::PreludeFmv { elapsed_secs, .. } => {
                visible_letter_entries_at(LETTER_W1_MSG_E_ENTRIES, elapsed_secs, 512)
                    .first()
                    .map(|(_, e)| e.text)
            }
            CreditsSequencePhase::MaryLetterScroll {
                entries,
                elapsed_secs,
                ..
            } => visible_letter_entries_at(entries, elapsed_secs, 512)
                .first()
                .map(|(_, e)| e.text),
            _ => None,
        }
    }

    /// Normalized `[0.0, 1.0]` scroll progress during [`CreditsSequencePhase::ScrollingStaffRoll`].
    pub fn scroll_progress(&self) -> Option<f32> {
        match self.phase {
            CreditsSequencePhase::ScrollingStaffRoll { elapsed_secs } => {
                Some((elapsed_secs / CREDITS_BIK_DURATION_SECS).clamp(0.0, 1.0))
            }
            _ => None,
        }
    }

    /// Advances the epilogue / credits state machine (`skip_phase` skips the current sub-phase on Start/Confirm,
    /// matching `0x0043ee00` / `0x0059f880` / `0x005a10dd` / `0x00578c53`).
    pub fn tick(&mut self, dt_secs: f32, skip_phase: bool) {
        let dt = dt_secs.max(0.0);
        match &mut self.phase {
            CreditsSequencePhase::Idle | CreditsSequencePhase::Finished => {}
            CreditsSequencePhase::InWaterBedside { elapsed_secs } => {
                *elapsed_secs += dt;
                if skip_phase || *elapsed_secs >= IN_WATER_BEDSIDE_DURATION_SECS {
                    self.phase = CreditsSequencePhase::PreludeFmv {
                        elapsed_secs: 0.0,
                        duration_secs: WATER_BIK_DURATION_SECS,
                    };
                }
            }
            CreditsSequencePhase::PreludeFmv {
                elapsed_secs,
                duration_secs,
            } => {
                *elapsed_secs += dt;
                if skip_phase || *elapsed_secs >= *duration_secs {
                    self.phase = CreditsSequencePhase::MaryLetterScroll {
                        entries: &LETTER_W2_MSG_E_ENTRIES,
                        elapsed_secs: 0.0,
                        duration_secs: IN_WATER_CLOSING_SCROLL_DURATION_SECS,
                        voice_stream_id: self.descriptor.letter_voice_stream_id,
                        voice_trigger_secs: self.descriptor.letter_voice_delay_secs,
                        voice_triggered: false,
                    };
                }
            }
            CreditsSequencePhase::MaryLetterScroll {
                elapsed_secs,
                duration_secs,
                voice_stream_id,
                voice_trigger_secs,
                voice_triggered,
                ..
            } => {
                *elapsed_secs += dt;
                if !*voice_triggered && *elapsed_secs >= *voice_trigger_secs {
                    *voice_triggered = true;
                    if let Some(stream_id) = *voice_stream_id {
                        self.last_triggered_voice_stream = Some(stream_id);
                    }
                }
                if skip_phase || *elapsed_secs >= *duration_secs {
                    self.phase = CreditsSequencePhase::EpilogueScene {
                        elapsed_secs: 0.0,
                        duration_secs: END_BIK_DURATION_SECS,
                    };
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
                if skip_phase || *elapsed_secs >= CREDITS_BIK_DURATION_SECS {
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

    /// Renders the upward-scrolling `.mes` letter stanzas or scrolling staff credits overlay into `rast`.
    pub fn render(&self, rast: &mut TextRasterizer, font: &AtlasFont) {
        match &self.phase {
            CreditsSequencePhase::PreludeFmv { elapsed_secs, .. } => {
                render_letter_scroll(rast, font, LETTER_W1_MSG_E_ENTRIES, *elapsed_secs);
            }
            CreditsSequencePhase::MaryLetterScroll {
                entries,
                elapsed_secs,
                ..
            } => {
                render_letter_scroll(rast, font, entries, *elapsed_secs);
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
                font.draw_line_centered(
                    rast,
                    player.current_texture_path(),
                    (rast.width / 2, rast.height.saturating_sub(36)),
                    18,
                    Sh2TextColor::MutedFooter,
                    true,
                );
            }
            CreditsSequencePhase::Idle
            | CreditsSequencePhase::InWaterBedside { .. }
            | CreditsSequencePhase::EpilogueScene { .. }
            | CreditsSequencePhase::Finished => {}
        }
    }
}

fn render_letter_scroll(
    rast: &mut TextRasterizer,
    font: &AtlasFont,
    entries: &'static [MaryLetterScrollEntry],
    elapsed_secs: f32,
) {
    let viewport_h = rast.height.max(1) as i32;
    let y_scale = (rast.height as f32) / 512.0;
    for (virtual_y, entry) in visible_letter_entries_at(entries, elapsed_secs, 512) {
        let base_y = (virtual_y as f32 * y_scale).round() as isize;
        for (line_idx, line) in entry.text.lines().enumerate() {
            let y = base_y + (line_idx as isize) * 28;
            if y >= 0 && (y as usize) + 22 <= rast.height && (virtual_y < viewport_h + 64) {
                font.draw_line_centered(
                    rast,
                    line,
                    (rast.width / 2, y as usize),
                    22,
                    Sh2TextColor::White,
                    true,
                );
            }
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
    let progress = (elapsed_secs / CREDITS_BIK_DURATION_SECS).clamp(0.0, 1.0);
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
    use sh2_core::endings::{
        SE_IN_WATER_DROWNING_STREAM, SE_LEAVE_LETTER_VOICEOVER, SE_MARIA_LETTER_VOICEOVER,
    };

    use super::*;

    #[test]
    fn leave_maria_and_in_water_trigger_authentic_voice_streams_and_bik_durations() {
        let mut leave = EndingCreditsSequence::start(GameEnding::Leave);
        leave.tick(0.5, false);
        assert_eq!(leave.last_triggered_voice_stream, None);
        leave.tick(3.0, false);
        assert_eq!(
            leave.last_triggered_voice_stream,
            Some(SE_LEAVE_LETTER_VOICEOVER)
        );
        assert!(leave
            .current_letter_text()
            .expect("letter visible")
            .contains("In my restless dreams"));

        let font = AtlasFont::load_embedded();
        let mut rast = TextRasterizer::new(640, 480);
        leave.render(&mut rast, &font);
        assert!(rast.pixels.iter().any(|&b| b != 0));

        let mut maria = EndingCreditsSequence::start(GameEnding::Maria);
        maria.tick(0.1, false);
        assert_eq!(
            maria.last_triggered_voice_stream,
            Some(SE_MARIA_LETTER_VOICEOVER)
        );

        let mut water = EndingCreditsSequence::start(GameEnding::InWater);
        assert!(matches!(
            water.phase,
            CreditsSequencePhase::InWaterBedside { .. }
        ));
        water.tick(IN_WATER_BEDSIDE_DURATION_SECS + 0.1, false);
        assert!(matches!(
            water.phase,
            CreditsSequencePhase::PreludeFmv { .. }
        ));
        water.tick(WATER_BIK_DURATION_SECS + 0.1, false);
        water.tick(15.5, false);
        assert_eq!(
            water.last_triggered_voice_stream,
            Some(SE_IN_WATER_DROWNING_STREAM)
        );
        water.tick(10.0, false);
        water.tick(END_BIK_DURATION_SECS + 0.1, false);
        water.tick(30.0, false);
        let mut credits_rast = TextRasterizer::new(640, 480);
        water.render(&mut credits_rast, &font);
        assert!(credits_rast.pixels.iter().any(|&b| b != 0));
        water.tick(CREDITS_BIK_DURATION_SECS, false);
        assert!(water.is_finished());
    }

    #[test]
    fn ufo_and_dog_and_wish_sequences_pair_exact_bik_durations_and_complete() {
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
        dog.tick(1.0, true);
        assert!(dog.is_finished());

        let wish = EndingCreditsSequence::start(GameEnding::BornFromAWish);
        assert_eq!(wish.music_theme(), CreditsMusicTheme::LovePsalm);
    }
}
