//! 1:1 PS2 / SH2:EE Fullscreen Paper Map state machine (`Event/chizu.c`).
//!
//! # Ground Truth References
//! - `ChizuMain` (`0x001c6b90` / `sh2pc.exe` `0x0049e2e0`):
//!   - Opens centered at `(px * 2.0, py * 2.0)` when `current_map_id == chizu_select(...)`, else `(0, 0)`.
//!   - Emits `SeCall(1.0, 0x2b17)` paper rustle SFX on sheet open/switch.
//! - `ChizuDisplay` (`0x001c7ba0` / `sh2pc.exe` `0x0049de60`):
//!   - `disp_rate` advances at **`1.0 / s`** (`disp_rate += dt` / `disp_rate -= dt`).
//!   - Non-linear sheet zoom factor: `2.0 / (2.0 - disp_rate)` (`1.0x` at `0.0` -> `2.0x` at `1.0`).
//!   - Marker scale factor: `1.0 / (2.0 - disp_rate)` (`0.5x` at `0.0` -> `1.0x` at `1.0` in 512×512).
//! - `ChizuControl` (`0x001c8360` / `sh2pc.exe` `0x0049cb40`):
//!   - `chizu_disp_step` (`0x0116dbb0`): `0` = FullSheet, `1` = ZoomingIn, `2` = ZoomedPan, `3` = ZoomingOut.
//!   - Continuous pan rates: **`307.2` px/s** horizontal, **`384.0` px/s** vertical, clamped to `[-256.0, +256.0]`.
//!   - `(center_x, center_y)` is **preserved** when zooming out (never reset to `(0, 0)`).
//! - `MapTranscription.cpp` (`0x0049c644`):
//!   - State 0 init pause: `0x02` ticks (`1/30` s at 60 Hz)
//!   - State 1 pre-zoom pause: `0x22` (`34`) ticks (`17/30` s at 60 Hz)
//!   - State 2 zoom-complete check pause: `0x02` ticks (`1/30` s at 60 Hz)
//!   - State 3 fade-in alpha: `+4` (`delta_frame << 2`) per 60 Hz tick up to `0x80` (`128`)
//!   - State 4 post-fade hold: `0x32` (`50`) ticks (`5/6` s at 60 Hz).

use crate::map::tables::{chizu_connect_check, MapDirection};
use crate::map::MapCanvasClipper;
use crate::menu::{MenuNavAction, MenuSoundCue};
use sh2_core::events::EventFlags;

/// Exact zoom rate in `disp_rate` units per second (`ChizuDisplay` `0x001c7ba0`: `disp_rate += dt`).
pub const CHIZU_ZOOM_RATE: f32 = 1.0;
/// Exact horizontal pan rate in PS2 virtual px/s (`ChizuControl` `0x001c8360`: `307.2`).
pub const CHIZU_PAN_SPEED_X: f32 = 307.2;
/// Exact vertical pan rate in PS2 virtual px/s (`ChizuControl` `0x001c8360`: `384.0`).
pub const CHIZU_PAN_SPEED_Y: f32 = 384.0;
/// Exact `chizu_center_x` / `chizu_center_y` clamp bound (`ChizuControl` `0x001c8360`: `±256.0`).
pub const CHIZU_CENTER_LIMIT: f32 = 256.0;

/// State 0 init pause in 60 Hz frames (`MapTranscription.cpp:52`, `push 0x02`).
pub const TRANSCRIBE_WAIT_INIT_TICKS: u32 = 0x02;
/// State 1 pre-zoom pause in 60 Hz frames (`MapTranscription.cpp:63`, `push 0x22`).
pub const TRANSCRIBE_WAIT_BEFORE_PAN_TICKS: u32 = 0x22;
/// State 2 zoom-check pause in 60 Hz frames (`MapTranscription.cpp:80`, `push 0x02`).
pub const TRANSCRIBE_WAIT_ZOOM_CHECK_TICKS: u32 = 0x02;
/// State 3 fade-in alpha target (`MapTranscription.cpp:107`, `cmp bl, 0x80`).
pub const TRANSCRIBE_FADE_TARGET_ALPHA: u8 = 0x80;
/// State 4 post-fade hold in 60 Hz frames (`MapTranscription.cpp:119`, `push 0x32`).
pub const TRANSCRIBE_WAIT_AFTER_FADE_TICKS: u32 = 0x32;
/// Zoom/pan timer completion cap (`MapTranscription.cpp:32`, `ZoomPanTimerMaxValue = 0.99999f`).
pub const ZOOM_PAN_TIMER_MAX: f32 = 0.99999;

/// Zoom state machine matching `chizu_disp_step` (`0x0116dbb0` in `ChizuControl` `0x001c8360`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MapZoomStep {
    /// Full `4:3` sheet view (`0`); directional keys switch floors/wings.
    #[default]
    FullSheet = 0,
    /// Smoothly zooming into `2.0x` detail view (`1`).
    ZoomingIn = 1,
    /// Zoomed-in panning view (`2`); directional keys pan `(center_x, center_y)`.
    ZoomedPan = 2,
    /// Smoothly zooming back out to full sheet (`3`).
    ZoomingOut = 3,
}

/// Active red-pen transcription animation phase (`MapTranscription.cpp:35..126`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MapTranscriptionPhase {
    /// No active transcription animation.
    #[default]
    Idle,
    /// State 0: `1/30` s initial wait (`0x02` frames at 60 Hz).
    WaitInit,
    /// State 1: `17/30` s pause before zooming/panning (`0x22` frames at 60 Hz).
    WaitBeforePan,
    /// State 2: `1/30` s wait for zoom-in completion (`0x02` frames at 60 Hz).
    WaitZoomComplete,
    /// State 3: Fade in new red marker pen stroke (`alpha += 4` per 60 Hz frame up to `0x80`).
    FadeInMarker,
    /// State 4: `5/6` s hold after transcription completes (`0x32` frames at 60 Hz).
    HoldAfterFade,
}

/// Action emitted by [`PaperMapViewer`] to `sh2-app`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaperMapAction {
    /// Stay in the paper map viewer.
    None,
    /// Close the paper map and return to gameplay (or Inventory if opened from Inventory).
    Close,
    /// Active `map_id` changed; `sh2-app` should load the new sheet `.tex` and play `0x2b17`.
    MapSheetChanged(u8),
}

/// Authentic PS2 / SH2:EE Fullscreen Paper Map viewer (`Event/chizu.c`).
#[derive(Debug, Clone)]
pub struct PaperMapViewer {
    /// Whether the fullscreen paper map is currently open.
    pub is_open: bool,
    /// Whether the map was opened from the Inventory `MAP` button (returns to Inventory on Cancel).
    pub opened_from_inventory: bool,
    /// Active 1-based map ID (`1..=36`, `chizu_crnt` `0x0116db80`).
    pub current_map_id: u8,
    /// Player's current-room map ID from `ChizuSelect` (`0x001c7040`).
    pub selected_map_id: u8,
    /// Whether Maria's *Born From a Wish* scenario is active (`DAT_01dbc00c == 1`).
    pub is_maria_scenario: bool,
    /// Current zoom state (`chizu_disp_step` `0x0116dbb0`).
    pub zoom_step: MapZoomStep,
    /// Continuous zoom interpolation factor `disp_rate` (`0.0` = `1.0x`, `1.0` = `2.0x`).
    pub zoom_t: f32,
    /// Pan center X in PS2 virtual pixels (`-256.0..=+256.0`, `chizu_center_x` `0x0116dbc0`).
    pub center_x: f32,
    /// Pan center Y in PS2 virtual pixels (`-256.0..=+256.0`, `chizu_center_y` `0x0116dbb8`).
    pub center_y: f32,
    /// Active red-pen transcription animation state (`MapTranscription.cpp`).
    pub transcription_phase: MapTranscriptionPhase,
    /// Active marker flag ID being transcribed, if any.
    pub transcribing_flag_id: Option<u16>,
    /// Elapsed 60 Hz ticks in the current transcription wait state.
    pub transcription_ticks: u32,
    /// Current fade-in alpha (`0..=0x80`) of the newly transcribed marker.
    pub transcription_alpha: u8,
}

impl Default for PaperMapViewer {
    fn default() -> Self {
        Self::new()
    }
}

impl PaperMapViewer {
    /// Creates a closed `PaperMapViewer`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            is_open: false,
            opened_from_inventory: false,
            current_map_id: 1,
            selected_map_id: 1,
            is_maria_scenario: false,
            zoom_step: MapZoomStep::FullSheet,
            zoom_t: 0.0,
            center_x: 0.0,
            center_y: 0.0,
            transcription_phase: MapTranscriptionPhase::Idle,
            transcribing_flag_id: None,
            transcription_ticks: 0,
            transcription_alpha: TRANSCRIBE_FADE_TARGET_ALPHA,
        }
    }

    /// Opens the fullscreen paper map on `map_id` (`1..=36`) centered at `(0.0, 0.0)`.
    pub fn open(&mut self, map_id: u8, opened_from_inventory: bool) {
        self.open_at_player(map_id, map_id, None, false, opened_from_inventory);
    }

    /// Opens the fullscreen paper map matching `ChizuMain` (`0x001c6b90` lines 104-119):
    /// when `map_id == selected_map_id` and `player_ps2_pos` is `Some((px, py))`, initializes
    /// `(center_x, center_y) = (px * 2.0, py * 2.0)` clamped to `±256.0`; otherwise `(0.0, 0.0)`.
    pub fn open_at_player(
        &mut self,
        map_id: u8,
        selected_map_id: u8,
        player_ps2_pos: Option<(f32, f32)>,
        is_maria_scenario: bool,
        opened_from_inventory: bool,
    ) {
        let clamped = map_id.clamp(1, 36);
        self.is_open = true;
        self.opened_from_inventory = opened_from_inventory;
        self.current_map_id = clamped;
        self.selected_map_id = selected_map_id;
        self.is_maria_scenario = is_maria_scenario;
        self.zoom_step = MapZoomStep::FullSheet;
        self.zoom_t = 0.0;
        if clamped == selected_map_id {
            if let Some((px, py)) = player_ps2_pos {
                self.center_x = (px * 2.0).clamp(-CHIZU_CENTER_LIMIT, CHIZU_CENTER_LIMIT);
                self.center_y = (py * 2.0).clamp(-CHIZU_CENTER_LIMIT, CHIZU_CENTER_LIMIT);
            } else {
                self.center_x = 0.0;
                self.center_y = 0.0;
            }
        } else {
            self.center_x = 0.0;
            self.center_y = 0.0;
        }
        self.transcription_phase = MapTranscriptionPhase::Idle;
        self.transcribing_flag_id = None;
        self.transcription_ticks = 0;
        self.transcription_alpha = TRANSCRIBE_FADE_TARGET_ALPHA;
    }

    /// Closes the fullscreen paper map.
    pub fn close(&mut self) {
        self.is_open = false;
        self.zoom_step = MapZoomStep::FullSheet;
        self.zoom_t = 0.0;
        self.transcription_phase = MapTranscriptionPhase::Idle;
    }

    /// Triggers a 60 FPS red-pen transcription animation for `flag_id` (`MapTranscription.cpp`).
    pub fn start_transcription(&mut self, flag_id: u16) {
        self.transcribing_flag_id = Some(flag_id);
        self.transcription_phase = MapTranscriptionPhase::WaitInit;
        self.transcription_ticks = 0;
        self.transcription_alpha = 0;
    }

    /// Non-linear sheet zoom magnification `2.0 / (2.0 - disp_rate)` (`ChizuDisplay` `0x001c7ba0`).
    #[must_use]
    pub fn zoom_factor(&self) -> f32 {
        2.0 / (2.0 - self.zoom_t)
    }

    /// Non-linear marker scale factor `1.0 / (2.0 - disp_rate)` (`ChizuMarkerDraw` `0x001c7e90`).
    #[must_use]
    pub fn marker_scale(&self) -> f32 {
        1.0 / (2.0 - self.zoom_t)
    }

    /// Projection magnification delta `disp_rate / (2.0 - disp_rate)` (`ChizuMarkerDraw` `0x001c81a0`).
    #[must_use]
    pub fn zoom_delta(&self) -> f32 {
        self.zoom_t / (2.0 - self.zoom_t)
    }

    /// Computes the exact normalized `[u0, v0, u1, v1]` crop of the `1024x1024` base map sheet
    /// (`ChizuDisplay` `0x001c7ba0` lines 419-435).
    #[must_use]
    pub fn sheet_uv_rect(&self) -> (f32, f32, f32, f32) {
        let u0 = ((self.center_x + 256.0) * self.zoom_t).max(0.0);
        let v0 = ((self.center_y + 256.0) * self.zoom_t).max(0.0);
        let span = (2.0 - self.zoom_t) * 512.0;
        let u1 = (u0 + span).min(1023.0);
        let v1 = (v0 + span).min(1023.0);
        (u0 / 1024.0, v0 / 1024.0, u1 / 1024.0, v1 / 1024.0)
    }

    /// Applies continuous directional panning (`ChizuControl` `0x001c8360` lines 693-736) when
    /// zoomed (`zoom_step != FullSheet`). `axis_x` / `axis_y` are in `-1.0..=+1.0`.
    pub fn pan_continuous(&mut self, axis_x: f32, axis_y: f32, dt: f32) {
        if !self.is_open || self.zoom_step == MapZoomStep::FullSheet {
            return;
        }
        let dt = dt.clamp(0.0, 0.1);
        self.center_x = (self.center_x + axis_x * CHIZU_PAN_SPEED_X * dt)
            .clamp(-CHIZU_CENTER_LIMIT, CHIZU_CENTER_LIMIT);
        self.center_y = (self.center_y + axis_y * CHIZU_PAN_SPEED_Y * dt)
            .clamp(-CHIZU_CENTER_LIMIT, CHIZU_CENTER_LIMIT);
    }

    /// Advances smooth `1.0 / s` zoom interpolation (`ChizuDisplay` `0x001c7ba0`) and
    /// `MapTranscription.cpp` 60 Hz frame counters. Preserves `(center_x, center_y)` on zoom-out.
    pub fn tick(&mut self, dt: f32) {
        if !self.is_open {
            return;
        }
        let dt = dt.clamp(0.0, 0.1);
        match self.zoom_step {
            MapZoomStep::ZoomingIn => {
                self.zoom_t += dt * CHIZU_ZOOM_RATE;
                if self.zoom_t >= ZOOM_PAN_TIMER_MAX {
                    self.zoom_t = 1.0;
                    self.zoom_step = MapZoomStep::ZoomedPan;
                }
            }
            MapZoomStep::ZoomingOut => {
                self.zoom_t -= dt * CHIZU_ZOOM_RATE;
                if self.zoom_t <= 1.0 - ZOOM_PAN_TIMER_MAX {
                    self.zoom_t = 0.0;
                    self.zoom_step = MapZoomStep::FullSheet;
                }
            }
            _ => {}
        }

        let delta_frames = ((dt * 60.0).round() as u32).max(1);
        match self.transcription_phase {
            MapTranscriptionPhase::Idle => {}
            MapTranscriptionPhase::WaitInit => {
                self.transcription_ticks += delta_frames;
                if self.transcription_ticks >= TRANSCRIBE_WAIT_INIT_TICKS {
                    self.transcription_ticks = 0;
                    self.transcription_phase = MapTranscriptionPhase::WaitBeforePan;
                }
            }
            MapTranscriptionPhase::WaitBeforePan => {
                self.transcription_ticks += delta_frames;
                if self.transcription_ticks >= TRANSCRIBE_WAIT_BEFORE_PAN_TICKS {
                    self.transcription_ticks = 0;
                    self.zoom_step = MapZoomStep::ZoomingIn;
                    self.transcription_phase = MapTranscriptionPhase::WaitZoomComplete;
                }
            }
            MapTranscriptionPhase::WaitZoomComplete => {
                self.transcription_ticks += delta_frames;
                if self.transcription_ticks >= TRANSCRIBE_WAIT_ZOOM_CHECK_TICKS
                    && self.zoom_step == MapZoomStep::ZoomedPan
                {
                    self.transcription_ticks = 0;
                    self.transcription_phase = MapTranscriptionPhase::FadeInMarker;
                }
            }
            MapTranscriptionPhase::FadeInMarker => {
                let step = (delta_frames << 2).min(255) as u8;
                self.transcription_alpha = self
                    .transcription_alpha
                    .saturating_add(step)
                    .min(TRANSCRIBE_FADE_TARGET_ALPHA);
                if self.transcription_alpha >= TRANSCRIBE_FADE_TARGET_ALPHA {
                    self.transcription_ticks = 0;
                    self.transcription_phase = MapTranscriptionPhase::HoldAfterFade;
                }
            }
            MapTranscriptionPhase::HoldAfterFade => {
                self.transcription_ticks += delta_frames;
                if self.transcription_ticks >= TRANSCRIBE_WAIT_AFTER_FADE_TICKS {
                    self.transcription_phase = MapTranscriptionPhase::Idle;
                    self.transcribing_flag_id = None;
                }
            }
        }
    }

    /// Handles discrete trigger inputs (`ChizuControl` @ `0x001c8360`):
    /// - `Confirm`: toggles zoom state (`0/3 -> 1`, `1/2 -> 3`).
    /// - Directional triggers in `FullSheet`: switches sheets via [`chizu_connect_check`].
    /// - `Cancel`: closes the map.
    pub fn handle_nav(
        &mut self,
        nav: MenuNavAction,
        flags: &EventFlags,
    ) -> (MenuSoundCue, PaperMapAction) {
        if !self.is_open {
            return (MenuSoundCue::None, PaperMapAction::None);
        }

        match nav {
            MenuNavAction::Cancel => {
                self.close();
                (MenuSoundCue::Cancel, PaperMapAction::Close)
            }
            MenuNavAction::Confirm => {
                self.zoom_step = match self.zoom_step {
                    MapZoomStep::FullSheet | MapZoomStep::ZoomingOut => MapZoomStep::ZoomingIn,
                    MapZoomStep::ZoomingIn | MapZoomStep::ZoomedPan => MapZoomStep::ZoomingOut,
                };
                (MenuSoundCue::None, PaperMapAction::None)
            }
            MenuNavAction::Up
            | MenuNavAction::Down
            | MenuNavAction::Left
            | MenuNavAction::Right => {
                if self.zoom_step == MapZoomStep::FullSheet {
                    let dir = match nav {
                        MenuNavAction::Up => MapDirection::Up,
                        MenuNavAction::Down => MapDirection::Down,
                        MenuNavAction::Left => MapDirection::Left,
                        MenuNavAction::Right => MapDirection::Right,
                        _ => unreachable!(),
                    };
                    if let Some(next_id) = chizu_connect_check(
                        self.current_map_id,
                        dir,
                        flags,
                        self.selected_map_id,
                        self.is_maria_scenario,
                    ) {
                        self.current_map_id = next_id;
                        self.center_x = 0.0;
                        self.center_y = 0.0;
                        (MenuSoundCue::None, PaperMapAction::MapSheetChanged(next_id))
                    } else {
                        (MenuSoundCue::None, PaperMapAction::None)
                    }
                } else {
                    (MenuSoundCue::None, PaperMapAction::None)
                }
            }
        }
    }

    /// Projects a PS2 centered `[-256..+256]` virtual coordinate `(x_ps2, y_ps2)` into viewport
    /// pixels inside `clipper` using the exact non-linear `ChizuMarkerDraw` formula:
    /// `p_screen = p + (disp_rate / (2.0 - disp_rate)) * (p - center)`.
    #[must_use]
    pub fn project_ps2_to_viewport(
        &self,
        x_ps2: f32,
        y_ps2: f32,
        clipper: &MapCanvasClipper,
    ) -> (f32, f32) {
        let f = self.zoom_delta();
        let sx = x_ps2 + f * (x_ps2 - self.center_x);
        let sy = y_ps2 + f * (y_ps2 - self.center_y);
        let cw = clipper.layout.canvas_right - clipper.layout.canvas_left;
        let ch = clipper.layout.canvas_bottom - clipper.layout.canvas_top;
        let vx = clipper.layout.canvas_left + ((sx + 256.0) / 512.0) * cw;
        let vy = clipper.layout.canvas_top + ((sy + 256.0) / 512.0) * ch;
        (vx, vy)
    }

    /// Projects a normalized sheet coordinate `(nx, ny)` in `0.0..=1.0` to viewport pixels
    /// inside `clipper` via [`Self::project_ps2_to_viewport`].
    #[must_use]
    pub fn project_to_viewport(&self, nx: f32, ny: f32, clipper: &MapCanvasClipper) -> (f32, f32) {
        self.project_ps2_to_viewport(nx * 512.0 - 256.0, ny * 512.0 - 256.0, clipper)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_paper_map_zoom_pan_and_transcription_timing() {
        let mut flags = EventFlags::new();
        flags.set(24, true);
        let mut viewer = PaperMapViewer::new();
        viewer.open(1, false);
        assert_eq!(viewer.zoom_step, MapZoomStep::FullSheet);
        assert!((viewer.zoom_factor() - 1.0).abs() < 1e-6);
        assert!((viewer.marker_scale() - 0.5).abs() < 1e-6);

        // Zoom in at exact 1.0 / s: 30 frames = 0.5 s -> disp_rate = 0.5, zoom = 2/(2-0.5) = 4/3
        viewer.handle_nav(MenuNavAction::Confirm, &flags);
        assert_eq!(viewer.zoom_step, MapZoomStep::ZoomingIn);
        for _ in 0..30 {
            viewer.tick(1.0 / 60.0);
        }
        assert!((viewer.zoom_t - 0.5).abs() < 1e-4);
        assert!((viewer.zoom_factor() - (4.0 / 3.0)).abs() < 1e-4);
        assert!((viewer.marker_scale() - (2.0 / 3.0)).abs() < 1e-4);

        // Another 30 frames -> disp_rate = 1.0, ZoomedPan, zoom = 2.0, marker_scale = 1.0
        for _ in 0..30 {
            viewer.tick(1.0 / 60.0);
        }
        assert_eq!(viewer.zoom_step, MapZoomStep::ZoomedPan);
        assert!((viewer.zoom_factor() - 2.0).abs() < 1e-6);
        assert!((viewer.marker_scale() - 1.0).abs() < 1e-6);

        // Continuous pan at 307.2 / 384.0 px/s (5 steps of 0.1 s = 0.5 s)
        for _ in 0..5 {
            viewer.pan_continuous(1.0, -1.0, 0.1);
        }
        assert!((viewer.center_x - 153.6).abs() < 1e-3);
        assert!((viewer.center_y - (-192.0)).abs() < 1e-3);

        // Zoom out preserves (center_x, center_y)
        viewer.handle_nav(MenuNavAction::Confirm, &flags);
        assert_eq!(viewer.zoom_step, MapZoomStep::ZoomingOut);
        for _ in 0..60 {
            viewer.tick(1.0 / 60.0);
        }
        assert_eq!(viewer.zoom_step, MapZoomStep::FullSheet);
        assert!((viewer.center_x - 153.6).abs() < 1e-3);
        assert!((viewer.center_y - (-192.0)).abs() < 1e-3);

        // Transcription timing (60 Hz ticks + 1.0/s zoom)
        viewer.start_transcription(0x05ce);
        assert_eq!(viewer.transcription_phase, MapTranscriptionPhase::WaitInit);
        for _ in 0..220 {
            viewer.tick(1.0 / 60.0);
        }
        assert_eq!(viewer.transcription_phase, MapTranscriptionPhase::Idle);
        assert_eq!(viewer.transcription_alpha, TRANSCRIBE_FADE_TARGET_ALPHA);
    }
}
