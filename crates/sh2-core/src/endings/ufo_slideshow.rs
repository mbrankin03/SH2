//! Director's Cut UFO Ending 35-Slide Comic & Silent-Film Staff Credits (`EvProgUfoMado` `0x00578c50`).
//!
//! Verified 1:1 against `sh2pc.exe` (`0x008c95a8` 26 `.tex` table, `0x008c9490..0x008c95a8` 35-slide table,
//! and `0x00578400..0x00578f63` playback state machine).

/// Authentic 26 UFO ending `.tex` asset paths loaded by `EvProgUfoMado` (`0x008c95a8` in `sh2pc.exe`).
pub const UFO_TEXTURE_PATHS: [&str; 26] = [
    "data/pic/ufo/ufomado01.tex",
    "data/pic/ufo/ufomado02.tex",
    "data/pic/ufo/ufomado03.tex",
    "data/pic/ufo/hj01.tex",
    "data/pic/ufo/hj02.tex",
    "data/pic/ufo/hj03.tex",
    "data/pic/ufo/hj04.tex",
    "data/pic/ufo/hj05.tex",
    "data/pic/ufo/hj06.tex",
    "data/pic/ufo/hj07.tex",
    "data/pic/ufo/words01.tex",
    "data/pic/ufo/words02.tex",
    "data/pic/ufo/words03.tex",
    "data/pic/ufo/words04.tex",
    "data/pic/ufo/words05.tex",
    "data/pic/ufo/words06.tex",
    "data/pic/ufo/words07.tex",
    "data/pic/ufo/hj032.tex",
    "data/pic/ufo/words062.tex",
    "data/pic/ufo/staff01.tex",
    "data/pic/ufo/staff02.tex",
    "data/pic/ufo/staff03.tex",
    "data/pic/ufo/staff04.tex",
    "data/pic/ufo/staff05.tex",
    "data/pic/ufo/staff06.tex",
    "data/pic/ufo/staff07.tex",
];

/// Single slide entry in `UFO_SLIDE_TABLE` (`8` bytes per record at `0x008c9490..0x008c95a8` in `sh2pc.exe`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UfoSlideEntry {
    /// Slide hold duration in seconds (`-1.0` on final slide `34`, holding until confirm/fade-out).
    pub duration_secs: f32,
    /// Index into [`UFO_TEXTURE_PATHS`] (`3..=25`).
    pub texture_index: u8,
    /// `hj07.tex` abduction beam overlay mode (`0` = none, `1` = beam sweep in, `2` = beam hold at `0x00578500`).
    pub overlay_mode: u8,
    /// Stage `.mes` dialogue subtitle index (`53..=60`, or `0` when silent; `0x00578675`).
    pub subtitle_msg_index: u8,
}

/// All 35 comic + staff-credit slides from `0x008c9490..0x008c95a8` in `sh2pc.exe`.
pub const UFO_SLIDE_TABLE: [UfoSlideEntry; 35] = [
    UfoSlideEntry {
        duration_secs: 4.0,
        texture_index: 3,
        overlay_mode: 0,
        subtitle_msg_index: 0,
    },
    UfoSlideEntry {
        duration_secs: 2.5,
        texture_index: 10,
        overlay_mode: 0,
        subtitle_msg_index: 53,
    },
    UfoSlideEntry {
        duration_secs: 2.5,
        texture_index: 11,
        overlay_mode: 0,
        subtitle_msg_index: 54,
    },
    UfoSlideEntry {
        duration_secs: 3.0,
        texture_index: 4,
        overlay_mode: 0,
        subtitle_msg_index: 0,
    },
    UfoSlideEntry {
        duration_secs: 4.0,
        texture_index: 12,
        overlay_mode: 0,
        subtitle_msg_index: 55,
    },
    UfoSlideEntry {
        duration_secs: 2.5,
        texture_index: 13,
        overlay_mode: 0,
        subtitle_msg_index: 56,
    },
    UfoSlideEntry {
        duration_secs: 3.0,
        texture_index: 4,
        overlay_mode: 0,
        subtitle_msg_index: 0,
    },
    UfoSlideEntry {
        duration_secs: 2.5,
        texture_index: 14,
        overlay_mode: 0,
        subtitle_msg_index: 57,
    },
    UfoSlideEntry {
        duration_secs: 2.0,
        texture_index: 15,
        overlay_mode: 0,
        subtitle_msg_index: 58,
    },
    UfoSlideEntry {
        duration_secs: 3.5,
        texture_index: 18,
        overlay_mode: 0,
        subtitle_msg_index: 59,
    },
    UfoSlideEntry {
        duration_secs: 2.0,
        texture_index: 5,
        overlay_mode: 0,
        subtitle_msg_index: 0,
    },
    UfoSlideEntry {
        duration_secs: 1.5,
        texture_index: 16,
        overlay_mode: 0,
        subtitle_msg_index: 60,
    },
    UfoSlideEntry {
        duration_secs: 0.4,
        texture_index: 5,
        overlay_mode: 0,
        subtitle_msg_index: 0,
    },
    UfoSlideEntry {
        duration_secs: 0.4,
        texture_index: 17,
        overlay_mode: 0,
        subtitle_msg_index: 0,
    },
    UfoSlideEntry {
        duration_secs: 0.4,
        texture_index: 5,
        overlay_mode: 0,
        subtitle_msg_index: 0,
    },
    UfoSlideEntry {
        duration_secs: 0.4,
        texture_index: 17,
        overlay_mode: 0,
        subtitle_msg_index: 0,
    },
    UfoSlideEntry {
        duration_secs: 0.4,
        texture_index: 5,
        overlay_mode: 0,
        subtitle_msg_index: 0,
    },
    UfoSlideEntry {
        duration_secs: 0.4,
        texture_index: 17,
        overlay_mode: 0,
        subtitle_msg_index: 0,
    },
    UfoSlideEntry {
        duration_secs: 0.4,
        texture_index: 5,
        overlay_mode: 0,
        subtitle_msg_index: 0,
    },
    UfoSlideEntry {
        duration_secs: 0.4,
        texture_index: 17,
        overlay_mode: 0,
        subtitle_msg_index: 0,
    },
    UfoSlideEntry {
        duration_secs: 0.4,
        texture_index: 5,
        overlay_mode: 0,
        subtitle_msg_index: 0,
    },
    UfoSlideEntry {
        duration_secs: 0.4,
        texture_index: 17,
        overlay_mode: 0,
        subtitle_msg_index: 0,
    },
    UfoSlideEntry {
        duration_secs: 3.5,
        texture_index: 6,
        overlay_mode: 0,
        subtitle_msg_index: 0,
    },
    UfoSlideEntry {
        duration_secs: 3.5,
        texture_index: 7,
        overlay_mode: 0,
        subtitle_msg_index: 0,
    },
    UfoSlideEntry {
        duration_secs: 4.0,
        texture_index: 8,
        overlay_mode: 1,
        subtitle_msg_index: 0,
    },
    UfoSlideEntry {
        duration_secs: 1.0,
        texture_index: 8,
        overlay_mode: 2,
        subtitle_msg_index: 0,
    },
    UfoSlideEntry {
        duration_secs: 4.0,
        texture_index: 8,
        overlay_mode: 0,
        subtitle_msg_index: 0,
    },
    UfoSlideEntry {
        duration_secs: 3.0,
        texture_index: 19,
        overlay_mode: 0,
        subtitle_msg_index: 0,
    },
    UfoSlideEntry {
        duration_secs: 3.0,
        texture_index: 20,
        overlay_mode: 0,
        subtitle_msg_index: 0,
    },
    UfoSlideEntry {
        duration_secs: 3.0,
        texture_index: 21,
        overlay_mode: 0,
        subtitle_msg_index: 0,
    },
    UfoSlideEntry {
        duration_secs: 3.0,
        texture_index: 22,
        overlay_mode: 0,
        subtitle_msg_index: 0,
    },
    UfoSlideEntry {
        duration_secs: 3.0,
        texture_index: 23,
        overlay_mode: 0,
        subtitle_msg_index: 0,
    },
    UfoSlideEntry {
        duration_secs: 3.0,
        texture_index: 24,
        overlay_mode: 0,
        subtitle_msg_index: 0,
    },
    UfoSlideEntry {
        duration_secs: 3.0,
        texture_index: 25,
        overlay_mode: 0,
        subtitle_msg_index: 0,
    },
    UfoSlideEntry {
        duration_secs: -1.0,
        texture_index: 25,
        overlay_mode: 0,
        subtitle_msg_index: 0,
    },
];

/// Runtime player for the 35-slide UFO Ending comic & staff roll (`EvProgUfoMado` `0x00578400..0x00578f63`).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct UfoSlideShowPlayer {
    /// Current slide index (`0..35`, `0x01d8319c`).
    pub slide_index: usize,
    /// Elapsed time on the current slide in seconds (`0x01f7a964`).
    pub slide_elapsed_secs: f32,
    /// Whether the sequence has completed or been skipped (`0x005786cf` / `0x00578c61`).
    pub finished: bool,
}

impl UfoSlideShowPlayer {
    /// Creates a new UFO comic-slide player starting at `slide[0]` (`hj01.tex`).
    pub const fn new() -> Self {
        Self {
            slide_index: 0,
            slide_elapsed_secs: 0.0,
            finished: false,
        }
    }

    /// Current [`UfoSlideEntry`] descriptor.
    pub const fn current_slide(&self) -> UfoSlideEntry {
        let idx = if self.slide_index < UFO_SLIDE_TABLE.len() {
            self.slide_index
        } else {
            UFO_SLIDE_TABLE.len() - 1
        };
        UFO_SLIDE_TABLE[idx]
    }

    /// Current `.tex` asset path from [`UFO_TEXTURE_PATHS`].
    pub const fn current_texture_path(&self) -> &'static str {
        let tex_idx = self.current_slide().texture_index as usize;
        UFO_TEXTURE_PATHS[tex_idx]
    }

    /// Returns `true` once playback reaches the 7 silent-film staff credit slides (`slide[27..=34]`, `staff01..07.tex`).
    pub const fn is_in_staff_credits(&self) -> bool {
        self.slide_index >= 27
    }

    /// Advances the UFO slideshow by `dt_secs` (`0x0057868d..0x00578721`).
    pub fn tick(&mut self, dt_secs: f32, skip_requested: bool) {
        if self.finished {
            return;
        }
        if skip_requested {
            self.finished = true;
            return;
        }
        let mut remaining = dt_secs.max(0.0);
        while remaining > 0.0 && !self.finished {
            let entry = self.current_slide();
            if entry.duration_secs < 0.0 {
                self.finished = true;
                break;
            }
            let needed = (entry.duration_secs - self.slide_elapsed_secs).max(0.0);
            if remaining >= needed {
                remaining -= needed;
                self.slide_elapsed_secs = 0.0;
                if self.slide_index + 1 < UFO_SLIDE_TABLE.len() {
                    self.slide_index += 1;
                    if self.current_slide().duration_secs < 0.0 {
                        self.finished = true;
                        break;
                    }
                } else {
                    self.finished = true;
                    break;
                }
            } else {
                self.slide_elapsed_secs += remaining;
                remaining = 0.0;
            }
        }
    }
}
