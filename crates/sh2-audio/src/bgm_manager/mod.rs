//! 1:1 Silent Hill 2 BGM Streaming & Multi-Channel AIX Spatial Mixing Engine (`SH2-60`).
//!
//! Reproduces Team Silent's BGM subsystem from the PS2 Debug Prototype (`SLUS_202.28`:
//! `sound/sh_sound.c` and `sound/snd_select.c`) and Retail PC Binary (`sh2pc.exe`):
//! - **`SeBgmChange`** (`0x515C20`, PS2 `0x193c20`): Resolves the active BGM page (`0..=47`)
//!   from the current stage (`0..=58`), room ID (`bgm_room_id` / `0x494DB0`), and game event flags.
//! - **`BgmPageSet`** (`0x515690`, PS2 `0x194b80`): Evaluates the dynamic situation page:
//!   - Stage `EvBgmControl` override (`4..=15`) when active.
//!   - `3` (`EnemyNear`) when an active enemy (`0x200` type, alive, `0x400` active flag) is within
//!     `6000.0` world units (`dist_xz^2 < 36,000,000.0` at `0x6C5F5C`).
//!   - `2` (`LowHealth`) when player health ratio `< 0.20` (`0x63345C`).
//!   - Stage `EvBgmControl` low-priority return (`0..=3`), or `0` (`Calm`).
//! - **`sndGetSoundAryByCharaPos`** (`0x5187E0`, PS2 `0x221540`): Evaluates up to 8 overlapping
//!   `_SOUND_DATA` spatial volume bounding boxes (with `HALF_W = 50.0` X/Z margin and `[min_y, max_y]`
//!   vertical bounds, falling back to record `0` default when no area box matches), or situation
//!   preset table `0x8A9EB8` when `situation_page >= 6`.
//! - **`SeBgmManager`** (`0x516080`, PS2 `0x194300`): Drives the 7-track fade state machine
//!   (`TR_OFF = 0`, `TR_IN = 1`, `TR_ON = 2`, `TR_OUT = 3`) with priority-based fade-in/fade-out
//!   ramps (`FADE_PRIO = [0, 2, 3, 1, 4]`, `FADE_TIME = [0.0, 3.0, 1.5, 6.0, 0.0]` seconds),
//!   3.0-second page transition crossfades (`0x5155E0`), and per-file master volume scaling (`0x8A73B4`).

mod areas;
mod ducking;
mod selection;
mod streaming;
mod types;

pub use areas::{
    apply_music_box_volume_patch, evaluate_sound_areas, evaluate_sound_areas_with_restoration,
    extract_record_track_fades,
};
pub use ducking::{
    bgm_ducking_mode_from_game_state, is_bgm_demo_duck_exempt, step_global_bgm_fade,
    BgmDuckingMode, BGM_DUCK_DEMO, BGM_DUCK_NORMAL, BGM_DUCK_PAUSED, BGM_DUCK_RAMP_DIVISOR,
};
pub use selection::{eval_stage_ev_bgm_control, evaluate_situation_page, pick_bgm_page};
pub use streaming::decode_bgm_layers_from_disk;
pub use types::{
    BgmEventFlags, BgmTrackState, BgmTrackStatus, SoundAreaEvaluation, BGM_MAX_TRACKS,
    ENEMY_NEAR_DIST_SQ, FADE_PRIO, FADE_TIME_SECS, LOW_HEALTH_RATIO_THRESHOLD,
    MAX_MATCHED_SOUND_AREAS, SITUATION_PRESETS_6_TO_15, SOUND_AREA_HALF_WIDTH,
};

use glam::Vec3;
use sh2_core::AudioPackSource;
use std::path::PathBuf;
use std::time::Duration;

use crate::bgm_data::{bgm_page_entry, bgm_room_id, stage_glb_crd};
use crate::cutscene_bgm::master_volume_for_start_id;
use crate::engine::{AudioEngine, AudioError};

/// Runtime state machine for Silent Hill 2's BGM streaming & multi-channel AIX mixing (`SeBgmManager`).
#[derive(Debug, Clone)]
pub struct BgmManager {
    /// Game root directory containing `sh2e/sound/adx/` and `data/sound/adx/`.
    pub game_root: PathBuf,
    /// Selected audio source pack (`EnhancedLossless` 48 kHz vs `OriginalPs2` 24/44.1 kHz).
    pub audio_pack_source: AudioPackSource,
    /// Current stage ID (`0..=58`, `bgm.room` at `0x1F837CC`).
    pub stage_id: u32,
    /// Current room ID resolved from `bgm_room_id(glb_crd, x, z)`.
    pub room_id: u32,
    /// Current active BGM page (`0..=47`).
    pub current_page: u32,
    /// Target BGM page requested by `request_page` / `SeBgmChange`.
    pub target_page: u32,
    /// Currently playing `start_id` (`50000..=50130`, `bgm.play` at `0x1F837C0`).
    pub current_play_id: i32,
    /// Next queued `start_id` (`bgm.next` at `0x1F837C4`).
    pub next_play_id: i32,
    /// Next queued `load_id` (`5030..=5083`, `bgm.load` at `0x1F837C8`).
    pub pending_load_id: i32,
    /// Loaded `load_id` currently resident in the audio engine (`5030..=5083`, or `0`).
    pub loaded_file_id: i32,
    /// Relative path of the currently loaded `.adx` or `.aix` file.
    pub loaded_rel_path: Option<&'static str>,
    /// Resolved filesystem path of the currently playing `.adx` or `.aix` file.
    pub loaded_full_path: Option<PathBuf>,
    /// Whether the currently loaded stream is a single-track `.adx` (`true`) or multi-layer `.aix` (`false`).
    pub loaded_is_adx: bool,
    /// Sample rate (Hz) of the currently loaded stream (e.g., `48000` for SH2:EE, `24000`/`44100` for PS2/PC).
    pub loaded_sample_rate: u32,
    /// Number of decoded stereo layers in the active stream (`1` for ADX, `2..=7` for AIX).
    pub loaded_layer_count: usize,
    /// Per-file master volume scalar (`0x8A73B4`, `0.25..=0.90`, default `0.40`).
    pub file_master_volume: f32,
    /// Global BGM ducking/fade scalar (`bgm.vol` at `0x010e5890` / `0x1F837D4`, `0.5..=1.0`, default `1.0`).
    pub global_bgm_fade: f32,
    /// Active ducking mode (`Normal` `1.0`, `DramaDemo` `0.7`, `PausedOrMenu` `0.5`, `SeBgmManager` `0x00194300`).
    pub ducking_mode: BgmDuckingMode,
    /// Current situation page (`0` = Calm, `2` = Low Health, `3` = Enemy Near, `4..=15` = Event).
    pub situation_page: u32,
    /// Last `_SOUND_DATA` spatial evaluation snapshot.
    pub last_area_eval: SoundAreaEvaluation,
    /// Per-track state machine for the 7 AIX layers (`0x1F837D8`).
    pub tracks: [BgmTrackState; BGM_MAX_TRACKS],
    /// Event/progression flags affecting BGM page and situation selection.
    pub event_flags: BgmEventFlags,
}

impl BgmManager {
    /// Creates a new `BgmManager` rooted at `game_root` with the specified [`AudioPackSource`].
    pub fn new(game_root: impl Into<PathBuf>, audio_pack_source: AudioPackSource) -> Self {
        Self {
            game_root: game_root.into(),
            audio_pack_source,
            stage_id: 0,
            room_id: 0,
            current_page: 0,
            target_page: 0,
            current_play_id: 0,
            next_play_id: 0,
            pending_load_id: 0,
            loaded_file_id: 0,
            loaded_rel_path: None,
            loaded_full_path: None,
            loaded_is_adx: false,
            loaded_sample_rate: 0,
            loaded_layer_count: 0,
            file_master_volume: 0.40,
            global_bgm_fade: BGM_DUCK_NORMAL,
            ducking_mode: BgmDuckingMode::Normal,
            situation_page: 0,
            last_area_eval: SoundAreaEvaluation {
                matched_box_count: 0,
                matched_record_indices: Vec::new(),
                used_default_record: true,
                track_fades: [(0, 0); BGM_MAX_TRACKS],
            },
            tracks: [BgmTrackState::default(); BGM_MAX_TRACKS],
            event_flags: BgmEventFlags::default(),
        }
    }

    /// Switches between `AudioPackSource::EnhancedLossless` (`sh2e/sound/adx/`) and
    /// `AudioPackSource::OriginalPs2` (`data/sound/adx/`). If a stream is currently playing,
    /// reloads the active stream from the new pack while preserving all 7 track volumes and states.
    pub fn set_audio_pack_source(
        &mut self,
        source: AudioPackSource,
        engine: Option<&mut AudioEngine>,
    ) -> Result<(), AudioError> {
        if self.audio_pack_source == source {
            return Ok(());
        }
        self.audio_pack_source = source;
        if self.loaded_file_id > 0 {
            let load_id = self.loaded_file_id;
            self.load_and_start_stream(load_id, engine)?;
        }
        Ok(())
    }

    /// Sets the active BGM global ducking mode (`SeBgmManager` `0x00194300` / `0x00516430`).
    #[inline]
    pub fn set_ducking_mode(&mut self, mode: BgmDuckingMode) {
        self.ducking_mode = mode;
    }

    /// Requests a transition to BGM `page` (`0..=47`), reproducing `SeBgmCall`
    /// (`SLUS_202.28` `0x00194220` / `sh2pc.exe` `0x00515990`):
    /// - Looks up `(load_id, start_id)` in `BGM_PAGES[page]`.
    /// - If `start_id != current_play_id`:
    ///   - Sets `next_play_id = start_id` and `pending_load_id = load_id`.
    ///   - For every active track (`status != Off`):
    ///     - If already `FadingOut`, ONLY accelerate slow `6.0s` (`fade_out == 3`) to `3.0s` (`1`);
    ///       never overwrite fast `1.5s` (`2`) or instant (`4`) fade-outs.
    ///     - Otherwise (`FadingIn` or `On`), transition to `FadingOut` with `fade_out = 1` (`3.0s`).
    pub fn request_page(&mut self, page: u32) {
        let clamped_page = page.min(47);
        self.target_page = clamped_page;
        let entry = bgm_page_entry(clamped_page);

        if entry.start_id != self.current_play_id {
            self.next_play_id = entry.start_id;
            self.pending_load_id = entry.load_id;

            for track in &mut self.tracks {
                match track.status {
                    BgmTrackStatus::Off => {}
                    BgmTrackStatus::FadingOut => {
                        if track.fade_out == 3 {
                            track.fade_out = 1;
                        }
                    }
                    BgmTrackStatus::FadingIn | BgmTrackStatus::On => {
                        track.status = BgmTrackStatus::FadingOut;
                        track.fade_out = 1; // 3.0s fade-out before loading next page
                    }
                }
            }
        }
    }

    /// Evaluates `SeBgmChange` (`0x515C20`) for `stage_id` and `player_pos`, and requests the resolved BGM page.
    pub fn update_stage_and_position(&mut self, stage_id: u32, player_pos: Vec3) -> u32 {
        self.stage_id = stage_id;
        let glb = stage_glb_crd(stage_id);
        self.room_id = bgm_room_id(glb, player_pos.x, player_pos.z);
        let page = pick_bgm_page(stage_id, self.room_id, &self.event_flags);
        self.request_page(page);
        page
    }

    /// Immediately stops all BGM tracks and resets state (`0x515640` in `sh2pc.exe`).
    pub fn stop_all(&mut self, engine: Option<&mut AudioEngine>) {
        self.current_page = 0;
        self.target_page = 0;
        self.current_play_id = 0;
        self.next_play_id = 0;
        self.pending_load_id = 0;
        self.loaded_file_id = 0;
        self.loaded_rel_path = None;
        self.loaded_full_path = None;
        self.loaded_layer_count = 0;
        self.loaded_sample_rate = 0;
        self.tracks = [BgmTrackState::default(); BGM_MAX_TRACKS];
        if let Some(eng) = engine {
            eng.stop_bgm(Duration::from_millis(100));
        }
    }

    /// Steps the `SeBgmManager` (`0x516080`) state machine by `dt_secs`:
    /// 1. Resolves room ID and `SeBgmChange` target BGM page for `(stage_id, player_pos)`.
    /// 2. Handles page transition fade-out completion (`all tracks == Off`) and loads `pending_load_id`.
    /// 3. Starts `next_play_id` and sets `file_master_volume` (`0x8A73B4`) and initial track states:
    ///    - For single-stream `.adx`: Track 0 starts immediately at `On` (`vol = 1.0`), matching `SeBgmCall` (`0x194220`).
    ///    - For multi-layer `.aix`: All 7 tracks start at `Off` (`vol = 0.0`) and are driven by `_SOUND_DATA` area mixing.
    /// 4. Evaluates `BgmPageSet` situation page and `sndGetSoundAryByCharaPos` `_SOUND_DATA` area fades.
    /// 5. Advances per-track `FadingIn` / `FadingOut` volume ramps and updates `AudioEngine` layer gains.
    pub fn tick(
        &mut self,
        stage_id: u32,
        player_pos: Vec3,
        player_hp_ratio: f32,
        min_enemy_dist_xz: Option<f32>,
        dt_secs: f32,
        mut engine: Option<&mut AudioEngine>,
    ) -> Result<(), AudioError> {
        self.update_stage_and_position(stage_id, player_pos);

        // 1. Check if a page transition is waiting for active tracks to finish fading out (`0x516089..0x5160F3`)
        let mut waiting_for_fadeout = false;
        if self.current_play_id != self.next_play_id && self.next_play_id != 0 {
            let all_off = self.tracks.iter().all(|t| t.status == BgmTrackStatus::Off);
            if all_off {
                if let Some(eng) = engine.as_deref_mut() {
                    eng.stop_bgm(Duration::from_millis(20));
                }
                if self.pending_load_id > 0 {
                    self.load_and_start_stream(self.pending_load_id, engine.as_deref_mut())?;
                } else {
                    self.loaded_file_id = 0;
                    self.loaded_rel_path = None;
                    self.loaded_full_path = None;
                    self.loaded_layer_count = 0;
                    self.loaded_sample_rate = 0;
                }
                self.current_play_id = self.next_play_id;
                self.current_page = self.target_page;
                self.pending_load_id = 0;

                if self.current_play_id > 0 {
                    self.file_master_volume =
                        master_volume_for_start_id(self.current_play_id as u32);
                    if self.loaded_is_adx {
                        self.tracks[0] = BgmTrackState {
                            status: BgmTrackStatus::On,
                            fade_in: 4,
                            fade_out: 1,
                            volume: 1.0,
                        };
                    }
                } else {
                    // Page 0 (-1, -1) silence reached
                    self.current_play_id = 0;
                    self.next_play_id = 0;
                }
            } else {
                waiting_for_fadeout = true;
            }
        } else if self.pending_load_id != 0 {
            // Immediate first load when `current_play_id == 0` (`0x5160F4..0x516299`)
            let load_id = self.pending_load_id;
            self.pending_load_id = 0;
            self.current_play_id = self.next_play_id;
            self.current_page = self.target_page;

            if self.current_play_id > 0 && load_id > 0 {
                self.file_master_volume = master_volume_for_start_id(self.current_play_id as u32);
                self.tracks = [BgmTrackState::default(); BGM_MAX_TRACKS];
                self.load_and_start_stream(load_id, engine.as_deref_mut())?;
                if self.loaded_is_adx {
                    self.tracks[0] = BgmTrackState {
                        status: BgmTrackStatus::On,
                        fade_in: 4,
                        fade_out: 1,
                        volume: 1.0,
                    };
                }
            } else {
                self.stop_all(engine.as_deref_mut());
            }
        }

        // 2. Evaluate `_SOUND_DATA` spatial volume areas when an AIX stream is active and not mid-page-transition (`0x51629A..0x516401`)
        if self.current_play_id > 0 && self.pending_load_id == 0 && !waiting_for_fadeout {
            self.situation_page = evaluate_situation_page(
                self.stage_id,
                self.room_id,
                player_hp_ratio,
                min_enemy_dist_xz,
                &self.event_flags,
            );
            let eval = evaluate_sound_areas_with_restoration(
                self.stage_id,
                player_pos,
                self.situation_page,
                self.event_flags.enable_restored_cut_audio,
            );

            if !self.loaded_is_adx {
                for (i, &(in_type, out_type)) in eval.track_fades.iter().enumerate() {
                    if in_type != 0 {
                        if self.tracks[i].status == BgmTrackStatus::Off
                            || self.tracks[i].status == BgmTrackStatus::FadingOut
                        {
                            self.tracks[i].status = BgmTrackStatus::FadingIn;
                            self.tracks[i].fade_in = in_type;
                            self.tracks[i].fade_out = out_type.max(1);
                        }
                    } else if self.tracks[i].status != BgmTrackStatus::Off {
                        self.tracks[i].status = BgmTrackStatus::FadingOut;
                        if self.tracks[i].fade_out == 0 {
                            self.tracks[i].fade_out = 1;
                        }
                    }
                }
            }
            self.last_area_eval = eval;
        }

        // 3. Step `global_bgm_fade` pause/demo ducking ramp (`SeBgmManager` `0x00194300` / `0x00516430`)
        //    and per-track volume ramps (`0x516402..0x51650B`).
        let dt = dt_secs.max(0.0);
        self.global_bgm_fade = step_global_bgm_fade(self.global_bgm_fade, self.ducking_mode, dt);
        for (i, track) in self.tracks.iter_mut().enumerate() {
            match track.status {
                BgmTrackStatus::FadingIn => {
                    if track.fade_in == 4 {
                        track.volume = 1.0;
                        track.status = BgmTrackStatus::On;
                    } else {
                        let dur = FADE_TIME_SECS
                            .get(track.fade_in as usize)
                            .copied()
                            .unwrap_or(3.0);
                        if dur <= 0.0 {
                            track.volume = 1.0;
                            track.status = BgmTrackStatus::On;
                        } else {
                            track.volume += dt / dur;
                            if track.volume >= 1.0 {
                                track.volume = 1.0;
                                track.status = BgmTrackStatus::On;
                            }
                        }
                    }
                }
                BgmTrackStatus::FadingOut => {
                    if track.fade_out == 4 {
                        track.volume = 0.0;
                        track.status = BgmTrackStatus::Off;
                    } else {
                        let dur = FADE_TIME_SECS
                            .get(track.fade_out as usize)
                            .copied()
                            .unwrap_or(3.0);
                        if dur <= 0.0 {
                            track.volume = 0.0;
                            track.status = BgmTrackStatus::Off;
                        } else {
                            track.volume -= dt / dur;
                            if track.volume <= 0.0 {
                                track.volume = 0.0;
                                track.status = BgmTrackStatus::Off;
                            }
                        }
                    }
                }
                BgmTrackStatus::On => {
                    track.volume = 1.0;
                }
                BgmTrackStatus::Off => {
                    track.volume = 0.0;
                }
            }

            if let Some(eng) = engine.as_deref_mut() {
                let effective_gain = track.volume * self.file_master_volume * self.global_bgm_fade;
                eng.set_bgm_layer_gain(i, effective_gain, Duration::from_millis(16));
            }
        }

        Ok(())
    }
}
