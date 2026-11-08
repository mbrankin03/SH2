//! Interactive FMV Cutscene Player & Theater debug controller (`SH2-10`).
//!
//! Provides a complete catalog of all 21 Silent Hill 2 Bink (`.bik`) cutscenes and trailers,
//! playback controls (play/pause, seek forward/backward, restart, next/prev movie),
//! SH2:EE 1080p60 (`sh2e/movie/*.bik`) vs Vanilla PC (`data/movie/*.bik`) source toggling,
//! aspect-ratio scaling modes (16:9 widescreen, 4:3 pillarboxed, uncropped raw frame),
//! film grain toggle, and NTSC 29.97 FPS rescaled subtitle synchronization (`Patches/FmvSubtitles.cpp`).

use serde::{Deserialize, Serialize};

/// Category classification for entries in the FMV Cutscene & Movie Theater.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FmvCategory {
    /// Opening & Prologue cinematics (`toilet.bik`, `open.bik`, `flash.bik`, `hakaba.bik`, `gero.bik`).
    Prologue,
    /// Mid-game story cutscenes (`deai.bik`, `knife.bik`, `hei.bik`, `saikai.bik`, `rouya.bik`, `korosu_a.bik`, `korosu_b.bik`, `murder.bik`).
    StoryCutscene,
    /// Scenario endings & credits (`end.bik`, `water.bik`, `end_dog.bik`, `end_wish.bik`, `credits.bik`).
    Ending,
    /// Bonus E3 & TGS exhibition trailers (`sh2e3.bik`, `sh3e3.bik`, `sh3tgs.bik`).
    BonusTrailer,
}

impl FmvCategory {
    /// Human-readable category label for the FMV Player HUD.
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Prologue => "PROLOGUE / INTRO",
            Self::StoryCutscene => "STORY CUTSCENE",
            Self::Ending => "ENDING / CREDITS",
            Self::BonusTrailer => "BONUS TRAILER",
        }
    }
}

/// Descriptor for an FMV movie entry in the Silent Hill 2 Cutscene Theater catalog.
#[derive(Debug, Clone)]
pub struct FmvCatalogEntry {
    /// Short movie stem / CLI identifier (e.g. `"open"`, `"toilet"`, `"deai"`).
    pub id: &'static str,
    /// Bink filename under `sh2e/movie/` and `data/movie/` (e.g. `"open.bik"`).
    pub filename: &'static str,
    /// Human-readable cutscene title and stage context.
    pub display_name: &'static str,
    /// Cutscene category classification.
    pub category: FmvCategory,
    /// Authentic story context / location description.
    pub description: &'static str,
}

/// Complete catalog of all 21 Silent Hill 2 `.bik` FMV movies in narrative and bonus order.
pub const FMV_CATALOG: &[FmvCatalogEntry] = &[
    // 1. Prologue & Intro Cutscenes
    FmvCatalogEntry {
        id: "open",
        filename: "open.bik",
        display_name: "Opening Attract Montage - Theme of Laura (open.bik)",
        category: FmvCategory::Prologue,
        description: "Iconic Akira Yamaoka 'Theme of Laura' attract sequence (1080p60)",
    },
    FmvCatalogEntry {
        id: "toilet",
        filename: "toilet.bik",
        display_name: "Prologue: Observation Deck Restroom Mirror (toilet.bik)",
        category: FmvCategory::Prologue,
        description: "James stares into the filthy mirror at the Toluca Lake overlook",
    },
    FmvCatalogEntry {
        id: "flash",
        filename: "flash.bik",
        display_name: "Mary's Silent Flashback (flash.bik)",
        category: FmvCategory::Prologue,
        description: "Brief memory flash of Mary lying in bed",
    },
    FmvCatalogEntry {
        id: "hakaba",
        filename: "hakaba.bik",
        display_name: "Toluca Graveyard: Meeting Angela Orosco (hakaba.bik)",
        category: FmvCategory::Prologue,
        description: "James asks Angela for directions in the fog-shrouded cemetery",
    },
    FmvCatalogEntry {
        id: "gero",
        filename: "gero.bik",
        display_name: "Wood Side Apartments: Meeting Eddie Dombrowski (gero.bik)",
        category: FmvCategory::Prologue,
        description: "James discovers Eddie vomiting in the Room 101 bathroom",
    },
    // 2. Story Cutscenes
    FmvCatalogEntry {
        id: "hei",
        filename: "hei.bik",
        display_name: "Blue Creek Apartments: Laura Steps on James's Hand (hei.bik)",
        category: FmvCategory::StoryCutscene,
        description: "Laura kicks the key away and mocks James atop the courtyard wall",
    },
    FmvCatalogEntry {
        id: "knife",
        filename: "knife.bik",
        display_name: "Blue Creek Room 109: Angela's Mirror & Knife (knife.bik)",
        category: FmvCategory::StoryCutscene,
        description: "Angela contemplates suicide before handing her knife to James",
    },
    FmvCatalogEntry {
        id: "deai",
        filename: "deai.bik",
        display_name: "Rosewater Park: First Meeting with Maria (deai.bik)",
        category: FmvCategory::StoryCutscene,
        description: "'Do I look like your girlfriend? My name... is Maria.'",
    },
    FmvCatalogEntry {
        id: "saikai",
        filename: "saikai.bik",
        display_name: "Brookhaven Hospital Basement: Reunion with Maria (saikai.bik)",
        category: FmvCategory::StoryCutscene,
        description: "Maria confronts James after Pyramid Head's elevator chase",
    },
    FmvCatalogEntry {
        id: "rouya",
        filename: "rouya.bik",
        display_name: "Labyrinth Cell: Maria Behind the Bars (rouya.bik)",
        category: FmvCategory::StoryCutscene,
        description: "Maria speaks with Mary's memories from inside the labyrinth cell",
    },
    FmvCatalogEntry {
        id: "korosu_a",
        filename: "korosu_a.bik",
        display_name: "Labyrinth Execution A: Pyramid Head & Maria (korosu_a.bik)",
        category: FmvCategory::StoryCutscene,
        description: "James reaches the cell door too late to save Maria",
    },
    FmvCatalogEntry {
        id: "korosu_b",
        filename: "korosu_b.bik",
        display_name: "Lakeview Hotel Lobby: Dual Pyramid Heads & Maria (korosu_b.bik)",
        category: FmvCategory::StoryCutscene,
        description: "Two Pyramid Heads execute Maria before the final showdown",
    },
    FmvCatalogEntry {
        id: "murder",
        filename: "murder.bik",
        display_name: "Lakeview Hotel Room 312: The Videotape Truth (murder.bik)",
        category: FmvCategory::StoryCutscene,
        description: "James watches the VHS tape in Room 312 and remembers the truth",
    },
    // 3. Endings & Credits
    FmvCatalogEntry {
        id: "end",
        filename: "end.bik",
        display_name: "Leave / Maria / Rebirth Ending Sequence (end.bik)",
        category: FmvCategory::Ending,
        description: "Main scenario ending cinematic at Toluca Lake",
    },
    FmvCatalogEntry {
        id: "water",
        filename: "water.bik",
        display_name: "In Water Ending & Mary's Full Letter (water.bik)",
        category: FmvCategory::Ending,
        description: "James drives into Toluca Lake as Mary reads her final letter",
    },
    FmvCatalogEntry {
        id: "end_dog",
        filename: "end_dog.bik",
        display_name: "Dog Ending: Mira the Shiba Inu Control Room (end_dog.bik)",
        category: FmvCategory::Ending,
        description: "Secret Joke Ending: James discovers Mira operating the levers",
    },
    FmvCatalogEntry {
        id: "end_wish",
        filename: "end_wish.bik",
        display_name: "Born from a Wish Ending: Maria's Resolve (end_wish.bik)",
        category: FmvCategory::Ending,
        description: "Sub-scenario conclusion as Maria sets out to find James",
    },
    FmvCatalogEntry {
        id: "credits",
        filename: "credits.bik",
        display_name: "Staff Roll & Ending Credits (credits.bik)",
        category: FmvCategory::Ending,
        description: "Team Silent staff credits roll with montage portraits",
    },
    // 4. Bonus Trailers
    FmvCatalogEntry {
        id: "sh2e3",
        filename: "sh2e3.bik",
        display_name: "Bonus: Silent Hill 2 E3 2001 Trailer (sh2e3.bik)",
        category: FmvCategory::BonusTrailer,
        description: "Original E3 2001 promotional trailer",
    },
    FmvCatalogEntry {
        id: "sh3e3",
        filename: "sh3e3.bik",
        display_name: "Bonus: Silent Hill 3 E3 2002 Teaser (sh3e3.bik)",
        category: FmvCategory::BonusTrailer,
        description: "Silent Hill 3 E3 teaser included on Restless Dreams disc",
    },
    FmvCatalogEntry {
        id: "sh3tgs",
        filename: "sh3tgs.bik",
        display_name: "Bonus: Silent Hill 3 TGS 2002 Trailer (sh3tgs.bik)",
        category: FmvCategory::BonusTrailer,
        description: "Silent Hill 3 Tokyo Game Show trailer",
    },
];

/// Aspect ratio and cropping mode for FMV presentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum FmvAspectMode {
    /// Fit 16:9 widescreen active region (automatically crops top/bottom 75px subtitle padding on `1920x1230` SH2:EE videos).
    #[default]
    Widescreen16x9,
    /// Fit 4:3 classic CRT pillarboxed framing.
    Classic4x3,
    /// Show uncropped full coded video frame (`1920x1230` or `640x480`) preserving native pixel aspect.
    RawUncropped,
}

impl FmvAspectMode {
    /// Cycles to the next aspect ratio presentation mode.
    pub const fn next(self) -> Self {
        match self {
            Self::Widescreen16x9 => Self::Classic4x3,
            Self::Classic4x3 => Self::RawUncropped,
            Self::RawUncropped => Self::Widescreen16x9,
        }
    }

    /// Human-readable label for the FMV Player HUD.
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Widescreen16x9 => "16:9 WIDESCREEN (AUTO-CROP 1230->1080)",
            Self::Classic4x3 => "4:3 CLASSIC CRT (PILLARBOXED)",
            Self::RawUncropped => "RAW UNCROPPED FRAME (FULL CODED HEIGHT)",
        }
    }
}

/// Cutscene video source flavor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FmvVideoSource {
    /// Locally upscaled 4K AI Master MP4 video (`tools/vsr_pipeline/output/` or `movie/*.mp4`).
    Upscaled4kMp4,
    /// SH2:EE 1080p60 AI-upscaled & motion-interpolated videos (`sh2e/movie/`).
    #[default]
    Enhanced1080p60,
    /// Authentic PS2 DVD / Ratiocinator & Angular Graphics OG HQ deinterlaced 30fps masters (`sh2_enhanced_edition_og_hq_fmvs...`).
    OgHqPs2Masters,
    /// Original 2001 PC retail Bink cutscenes (`data/movie/`).
    VanillaPc,
}

impl FmvVideoSource {
    pub const fn next(self) -> Self {
        match self {
            Self::Upscaled4kMp4 => Self::Enhanced1080p60,
            Self::Enhanced1080p60 => Self::OgHqPs2Masters,
            Self::OgHqPs2Masters => Self::VanillaPc,
            Self::VanillaPc => Self::Upscaled4kMp4,
        }
    }

    pub const fn label(&self) -> &'static str {
        match self {
            Self::Upscaled4kMp4 => "4K MASTER MP4 (LOCAL AI VSR UPSCALE)",
            Self::Enhanced1080p60 => "SH2:EE 1080P60 (AI-UPSCALED)",
            Self::OgHqPs2Masters => "OG HQ PS2 MASTERS (NATURAL 30FPS)",
            Self::VanillaPc => "VANILLA PC (640x384 / 4:3 30FPS)",
        }
    }
}

pub struct FmvViewerState {
    /// Index into [`FMV_CATALOG`].
    pub selected_movie_index: usize,
    /// Whether playback is currently paused.
    pub is_paused: bool,
    /// Whether to loop the active cutscene when it reaches the end.
    pub loop_playback: bool,
    /// Active video source tier preference.
    pub video_source: FmvVideoSource,
    /// Legacy compatibility flag (true for non-vanilla sources).
    pub prefer_hd_source: bool,
    /// Active aspect ratio and cropping mode.
    pub aspect_mode: FmvAspectMode,
    /// Whether film grain noise overlay is enabled over the FMV (drawn beneath subtitles per `Patches/FmvSubtitles.cpp`).
    pub film_grain_enabled: bool,
    /// Whether synchronized subtitles are displayed over the FMV.
    pub subtitles_enabled: bool,
    /// Whether the diagnostic HUD overlay is visible (`Tab` or `H` toggles clean theater view).
    pub show_hud: bool,
    /// Selected Bink audio track override (`None` = per-movie default, e.g. uncut murder.bik).
    pub audio_track: Option<usize>,
    /// Whether restored cut/unused audio (`VHSAudio` uncut PS2 `murder.bik` Track 1 with Mary screaming) is enabled by default (`SH2-62`).
    pub enable_restored_cut_audio: bool,
    /// Real-time sharpening level (0 = off, 1 = soft, 2 = medium, 3 = strong).
    pub sharpen_level: u8,
    /// Elapsed playback clock in seconds for audio-video master synchronization.
    pub elapsed_secs: f64,
    /// Total duration of the currently loaded movie in seconds.
    pub duration_secs: f64,
    /// Current decoded video frame index.
    pub current_frame_index: u32,
    /// Total frames in the currently loaded movie.
    pub total_frames: u32,
    /// Active video framerate (e.g. `59.94` or `29.97`).
    pub video_fps: f64,
    /// Coded width of the active movie in pixels.
    pub video_width: u32,
    /// Coded height of the active movie in pixels.
    pub video_height: u32,
}

impl Default for FmvViewerState {
    fn default() -> Self {
        Self::new()
    }
}

impl FmvViewerState {
    /// Initializes the FMV Viewer focused on the first catalog entry (`open.bik`).
    pub fn new() -> Self {
        Self {
            selected_movie_index: 0,
            is_paused: false,
            loop_playback: true,
            video_source: FmvVideoSource::Enhanced1080p60,
            prefer_hd_source: true,
            aspect_mode: FmvAspectMode::Widescreen16x9,
            film_grain_enabled: true,
            subtitles_enabled: true,
            show_hud: true,
            audio_track: None,
            enable_restored_cut_audio: true,
            sharpen_level: 1,
            elapsed_secs: 0.0,
            duration_secs: 1.0,
            current_frame_index: 0,
            total_frames: 1,
            video_fps: 59.94,
            video_width: 1920,
            video_height: 1080,
        }
    }

    /// Returns the currently selected [`FmvCatalogEntry`].
    pub fn current_entry(&self) -> &'static FmvCatalogEntry {
        &FMV_CATALOG[self.selected_movie_index % FMV_CATALOG.len()]
    }

    /// Selects a cutscene by matching its CLI stem, filename, or display name (case-insensitive).
    pub fn select_by_name(&mut self, query: &str) -> bool {
        let q = query.trim().trim_end_matches(".bik").to_ascii_lowercase();
        if q.is_empty() {
            return false;
        }

        for (idx, entry) in FMV_CATALOG.iter().enumerate() {
            if entry.id.eq_ignore_ascii_case(&q) || entry.filename.eq_ignore_ascii_case(&q) {
                self.set_movie_index(idx);
                return true;
            }
        }

        for (idx, entry) in FMV_CATALOG.iter().enumerate() {
            if entry.id.to_ascii_lowercase().contains(&q)
                || entry.display_name.to_ascii_lowercase().contains(&q)
            {
                self.set_movie_index(idx);
                return true;
            }
        }

        false
    }

    /// Sets the active movie index and resets playback timer to 0.0.
    pub fn set_movie_index(&mut self, index: usize) {
        self.selected_movie_index = index % FMV_CATALOG.len();
        self.audio_track = None;
        self.elapsed_secs = 0.0;
        self.current_frame_index = 0;
        self.is_paused = false;
    }

    /// Advances to the next cutscene in [`FMV_CATALOG`].
    pub fn next_movie(&mut self) {
        let next = (self.selected_movie_index + 1) % FMV_CATALOG.len();
        self.set_movie_index(next);
    }

    /// Returns to the previous cutscene in [`FMV_CATALOG`].
    pub fn prev_movie(&mut self) {
        let prev = if self.selected_movie_index == 0 {
            FMV_CATALOG.len() - 1
        } else {
            self.selected_movie_index - 1
        };
        self.set_movie_index(prev);
    }

    /// Updates stream metadata when a new `.bik` file is opened.
    pub fn configure_stream_meta(&mut self, width: u32, height: u32, frame_count: u32, fps: f64) {
        self.video_width = width.max(1);
        self.video_height = height.max(1);
        self.total_frames = frame_count.max(1);
        self.video_fps = if fps > 0.0 { fps } else { 29.97 };
        self.duration_secs = (self.total_frames as f64) / self.video_fps;
        self.elapsed_secs = 0.0;
        self.current_frame_index = 0;
    }

    /// Toggles pause/resume state.
    pub fn toggle_pause(&mut self) {
        self.is_paused = !self.is_paused;
    }

    /// Resolves the default Bink audio track index for `filename` (`SH2-62`):
    /// - `"open.bik"`: Track `1` (Vocal mix with Mary/Maria dialogue).
    /// - `"murder.bik"`: Track `1` (Uncut PS2 VHS with Mary's screams restored per `VHSAudio`)
    ///   when `self.enable_restored_cut_audio` is `true`, or Track `0` (Censored PC VHS static) when `false`.
    /// - All other `.bik` movies: Track `0`.
    pub fn default_audio_track_for(&self, filename: &str) -> usize {
        if filename.eq_ignore_ascii_case("open.bik") {
            1
        } else if filename.eq_ignore_ascii_case("murder.bik") {
            if self.enable_restored_cut_audio {
                1
            } else {
                0
            }
        } else {
            0
        }
    }

    /// Cycles cutscene video source: SH2:EE 1080p60 -> OG HQ PS2 Masters (30fps) -> Vanilla PC.
    /// Human-readable label for the currently active audio track.
    pub fn audio_track_label(&self, filename: &str, total_tracks: usize) -> String {
        if total_tracks <= 1 {
            return "TRACK 1/1 : STEREO 48KHZ (DEFAULT)".to_string();
        }

        let default_track = self.default_audio_track_for(filename);
        let current = self.audio_track.unwrap_or(default_track);

        match filename {
            "murder.bik" => {
                if current == 1 {
                    format!(
                        "TRACK 2/{} : UNCUT PS2 VHS (MARY SCREAMING RESTORED)",
                        total_tracks
                    )
                } else {
                    format!(
                        "TRACK 1/{} : CENSORED PC VHS (MUTED / STATIC)",
                        total_tracks
                    )
                }
            }
            "open.bik" => {
                if current == 1 {
                    format!(
                        "TRACK 2/{} : VOCAL MIX (THEME OF LAURA + MARY/MARIA)",
                        total_tracks
                    )
                } else {
                    format!(
                        "TRACK 1/{} : INSTRUMENTAL (THEME OF LAURA BGM ONLY)",
                        total_tracks
                    )
                }
            }
            "end.bik" | "ending.bik" => {
                let name = match current {
                    0 => "MARIA ENDING ('PROMISE')",
                    1 => "REBIRTH ENDING ('THE REVERSE WILL')",
                    2 => "IN WATER ENDING ('ANGEL\'S THANATOS')",
                    3 => "LEAVE ENDING ('OVERDOSE DELUSION')",
                    _ => "UNKNOWN ENDING THEME",
                };
                format!("TRACK {}/{} : {}", current + 1, total_tracks, name)
            }
            _ => {
                format!("TRACK {}/{} (STEREO 48KHZ)", current + 1, total_tracks)
            }
        }
    }

    pub fn cycle_video_source(&mut self) {
        self.video_source = self.video_source.next();
        self.prefer_hd_source = !matches!(self.video_source, FmvVideoSource::VanillaPc);
        self.elapsed_secs = 0.0;
        self.current_frame_index = 0;
    }

    /// Legacy toggle for backwards compatibility.
    pub fn toggle_hd_source(&mut self) {
        self.cycle_video_source();
    }

    /// Computes normalized UV crop `(uv_y_offset, uv_y_scale)` and target display aspect ratio
    /// for the current movie dimensions and [`FmvAspectMode`].
    pub fn compute_uv_crop_and_aspect(&self) -> (f32, f32, f32) {
        let w = self.video_width.max(1) as f32;
        let h = self.video_height.max(1) as f32;

        match self.aspect_mode {
            FmvAspectMode::Widescreen16x9 => {
                // SH2:EE 1920x1230 videos contain 1920x1080 active 16:9 image in the center
                // with 75px top and bottom padding. Vanilla 640x384 / 640x480 letterboxed videos
                // also center the 16:9 picture vertically.
                if self.video_width == 1920 && self.video_height == 1230 {
                    let y_off = 75.0 / 1230.0;
                    let y_scale = 1080.0 / 1230.0;
                    (y_off, y_scale, 16.0 / 9.0)
                } else {
                    (0.0, 1.0, 16.0 / 9.0)
                }
            }
            FmvAspectMode::Classic4x3 => {
                if self.video_width == 1920 && self.video_height == 1230 {
                    let y_off = 75.0 / 1230.0;
                    let y_scale = 1080.0 / 1230.0;
                    (y_off, y_scale, 4.0 / 3.0)
                } else {
                    (0.0, 1.0, 4.0 / 3.0)
                }
            }
            FmvAspectMode::RawUncropped => (0.0, 1.0, w / h),
        }
    }

    /// Returns formatted BGM stream label for HUD rendering.
    pub fn bgm_stream_label(&self, bgm_file: Option<&str>, description: &str) -> String {
        match bgm_file {
            Some(file) => format!("{} ({})", file, description),
            None => format!("Silent / In-Movie ({})", description),
        }
    }

    /// Advances playback clock by `dt` seconds and returns the target video frame index `(0..total_frames)`.
    pub fn tick(&mut self, dt: f64) -> u32 {
        if !self.is_paused && self.duration_secs > 0.0 {
            self.elapsed_secs += dt;
            if self.elapsed_secs >= self.duration_secs {
                if self.loop_playback {
                    self.elapsed_secs %= self.duration_secs;
                } else {
                    self.elapsed_secs = self.duration_secs;
                    self.is_paused = true;
                }
            }
        }

        let target = ((self.elapsed_secs * self.video_fps).floor() as u32)
            .min(self.total_frames.saturating_sub(1));
        self.current_frame_index = target;
        target
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fmv_catalog_and_navigation() {
        assert_eq!(FMV_CATALOG.len(), 21);
        let mut viewer = FmvViewerState::new();
        assert_eq!(viewer.current_entry().id, "open");

        assert!(viewer.select_by_name("toilet"));
        assert_eq!(viewer.current_entry().filename, "toilet.bik");
        assert_eq!(viewer.current_entry().category, FmvCategory::Prologue);

        assert!(viewer.select_by_name("deai.bik"));
        assert_eq!(viewer.current_entry().id, "deai");
        assert_eq!(viewer.current_entry().category, FmvCategory::StoryCutscene);

        assert!(viewer.select_by_name("water"));
        assert_eq!(viewer.current_entry().category, FmvCategory::Ending);

        viewer.next_movie();
        assert_eq!(viewer.current_entry().id, "end_dog");
    }

    #[test]
    fn test_fmv_timing_and_1230_crop() {
        let mut viewer = FmvViewerState::new();
        viewer.configure_stream_meta(1920, 1230, 261, 2997.0 / 50.0);

        let (y_off, y_scale, aspect) = viewer.compute_uv_crop_and_aspect();
        assert!((y_off - 75.0 / 1230.0).abs() < 1e-4);
        assert!((y_scale - 1080.0 / 1230.0).abs() < 1e-4);
        assert!((aspect - 16.0 / 9.0).abs() < 1e-4);

        // Advance 1.0 second at 59.94 fps -> frame 59
        let frame = viewer.tick(1.0);
        assert!(frame == 59 || frame == 60);
    }

    #[test]
    fn test_murder_bik_uncut_vhs_track_respects_enable_restored_cut_audio() {
        let mut viewer = FmvViewerState::new();
        assert!(viewer.enable_restored_cut_audio);
        assert_eq!(viewer.default_audio_track_for("murder.bik"), 1);
        assert_eq!(viewer.default_audio_track_for("open.bik"), 1);
        assert_eq!(viewer.default_audio_track_for("deai.bik"), 0);
        assert!(viewer
            .audio_track_label("murder.bik", 2)
            .contains("UNCUT PS2 VHS"));

        viewer.enable_restored_cut_audio = false;
        assert_eq!(viewer.default_audio_track_for("murder.bik"), 0);
        assert_eq!(viewer.default_audio_track_for("open.bik"), 1);
        assert!(viewer
            .audio_track_label("murder.bik", 2)
            .contains("CENSORED PC VHS"));
    }
}
