//! 1:1 PS2 / SH2:EE Fullscreen Paper Map viewer state machine & renderer (`Event/chizu.c`).
//!
//! # Ground Truth References
//! - `ChizuMain` (`0x001c6b90`), `ChizuControl` (`0x001c8360`):
//!   - `chizu_disp_step` (`0x0116dbb0`): `0` = FullSheet, `1` = ZoomingIn, `2` = ZoomedPan, `3` = ZoomingOut.
//!   - Analog pan deadzone: `0x9b` (`155` / `255` -> `0.21` normalized stick threshold, `0x001c8508`).
//! - `MapTranscription.cpp` (`0x0049c644`):
//!   - Exact 60 FPS transcription state durations:
//!     - State 0 init pause: `0x02` ticks (`1/30` s at 60 Hz)
//!     - State 1 pre-zoom pause: `0x22` (`34`) ticks (`17/30` s at 60 Hz)
//!     - State 2 zoom-complete check pause: `0x02` ticks (`1/30` s at 60 Hz)
//!     - State 3 fade-in alpha: `+4` (`delta_frame << 2`) per 60 Hz tick up to `0x80` (`128`)
//!     - State 4 post-fade hold: `0x32` (`50`) ticks (`5/6` s at 60 Hz).

use crate::atlas_font::{AtlasFont, Sh2TextColor};
use crate::font::TextRasterizer;
use crate::map::tables::{
    annotations_for_map, chizu_connect_neighbor, map_spec, MapAnnotation, MapDirection,
    MapMarkerKind,
};
use crate::map::{MapCanvasClipper, MapPlayerMarker};
use crate::menu::{MenuNavAction, MenuSoundCue};
use sh2_core::events::EventFlags;

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
    /// Active `map_id` changed; `sh2-app` should load the new sheet `.tex`.
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
    /// Current zoom state (`chizu_disp_step` `0x0116dbb0`).
    pub zoom_step: MapZoomStep,
    /// Continuous zoom interpolation factor (`0.0` = `1.0x` full sheet, `1.0` = `2.0x` zoomed).
    pub zoom_t: f32,
    /// Normalized pan center X (`0.0..=1.0`, `chizu_center_x` `0x0116dbc0`).
    pub center_x: f32,
    /// Normalized pan center Y (`0.0..=1.0`, `chizu_center_y` `0x0116dbb8`).
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
    pub fn new() -> Self {
        Self {
            is_open: false,
            opened_from_inventory: false,
            current_map_id: 1,
            zoom_step: MapZoomStep::FullSheet,
            zoom_t: 0.0,
            center_x: 0.5,
            center_y: 0.5,
            transcription_phase: MapTranscriptionPhase::Idle,
            transcribing_flag_id: None,
            transcription_ticks: 0,
            transcription_alpha: TRANSCRIBE_FADE_TARGET_ALPHA,
        }
    }

    /// Opens the fullscreen paper map on `map_id` (`1..=36`).
    pub fn open(&mut self, map_id: u8, opened_from_inventory: bool) {
        self.is_open = true;
        self.opened_from_inventory = opened_from_inventory;
        self.current_map_id = map_id.clamp(1, 36);
        self.zoom_step = MapZoomStep::FullSheet;
        self.zoom_t = 0.0;
        self.center_x = 0.5;
        self.center_y = 0.5;
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

    /// Advances smooth zoom interpolation and `MapTranscription.cpp` 60 Hz frame counters.
    pub fn tick(&mut self, dt: f32) {
        if !self.is_open {
            return;
        }
        let dt = dt.clamp(0.0, 0.1);
        match self.zoom_step {
            MapZoomStep::ZoomingIn => {
                self.zoom_t = (self.zoom_t + dt * 3.5).min(1.0);
                if self.zoom_t >= 0.99999 {
                    self.zoom_t = 1.0;
                    self.zoom_step = MapZoomStep::ZoomedPan;
                }
            }
            MapZoomStep::ZoomingOut => {
                self.zoom_t = (self.zoom_t - dt * 3.5).max(0.0);
                if self.zoom_t <= 0.00001 {
                    self.zoom_t = 0.0;
                    self.center_x = 0.5;
                    self.center_y = 0.5;
                    self.zoom_step = MapZoomStep::FullSheet;
                }
            }
            _ => {}
        }

        // Advance `MapTranscription.cpp` 60 Hz animation ticks
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

    /// Handles directional, zoom toggle, and cancel inputs (`ChizuControl` @ `0x001c8360`).
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
                // `ChizuControl` (`0x001c8394..0x001c84ec`): toggle ZoomingIn <-> ZoomingOut
                self.zoom_step = match self.zoom_step {
                    MapZoomStep::FullSheet | MapZoomStep::ZoomingOut => MapZoomStep::ZoomingIn,
                    MapZoomStep::ZoomingIn | MapZoomStep::ZoomedPan => MapZoomStep::ZoomingOut,
                };
                (MenuSoundCue::Confirm, PaperMapAction::None)
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
                    if let Some(next_id) = chizu_connect_neighbor(self.current_map_id, dir, flags) {
                        self.current_map_id = next_id;
                        (
                            MenuSoundCue::CursorMove,
                            PaperMapAction::MapSheetChanged(next_id),
                        )
                    } else {
                        (MenuSoundCue::None, PaperMapAction::None)
                    }
                } else {
                    // Pan inside zoomed map (`chizu_center_x` / `chizu_center_y`)
                    const PAN_STEP: f32 = 0.08;
                    match nav {
                        MenuNavAction::Up => {
                            self.center_y = (self.center_y - PAN_STEP).clamp(0.25, 0.75)
                        }
                        MenuNavAction::Down => {
                            self.center_y = (self.center_y + PAN_STEP).clamp(0.25, 0.75)
                        }
                        MenuNavAction::Left => {
                            self.center_x = (self.center_x - PAN_STEP).clamp(0.25, 0.75)
                        }
                        MenuNavAction::Right => {
                            self.center_x = (self.center_x + PAN_STEP).clamp(0.25, 0.75)
                        }
                        _ => {}
                    }
                    (MenuSoundCue::None, PaperMapAction::None)
                }
            }
        }
    }

    /// Projects a normalized sheet coordinate `(nx, ny)` in `0.0..=1.0` to viewport pixels
    /// inside `clipper`, accounting for active zoom (`1.0x..=2.0x`) and pan `(center_x, center_y)`.
    pub fn project_to_viewport(&self, nx: f32, ny: f32, clipper: &MapCanvasClipper) -> (f32, f32) {
        let zoom = 1.0 + self.zoom_t;
        let cw = clipper.layout.canvas_right - clipper.layout.canvas_left;
        let ch = clipper.layout.canvas_bottom - clipper.layout.canvas_top;
        let cx = clipper.layout.canvas_left + cw * 0.5;
        let cy = clipper.layout.canvas_top + ch * 0.5;
        let px = cx + (nx - self.center_x) * cw * zoom;
        let py = cy + (ny - self.center_y) * ch * zoom;
        (px, py)
    }
}

/// Renders the authentic `4:3` pillarboxed Fullscreen Paper Map with:
/// - Loaded retail `data/pic/map/*.tex` RGBA sheet (or aged paper fallback if missing),
/// - `EventFlags`-driven red marker pen annotations (`ChizuMarkerDraw`),
/// - Clipped green player heading triangle (`MapCanvasClipper::clip_player_marker`),
/// - Pure black widescreen/ultrawide pillarboxes (`SetBlackPillarBoxes`).
#[allow(clippy::too_many_arguments)]
pub fn draw_paper_map(
    viewer: &PaperMapViewer,
    flags: &EventFlags,
    player_norm_pos: (f32, f32),
    player_yaw_rad: f32,
    sheet_rgba: Option<(&[u8], usize, usize)>,
    font: &AtlasFont,
    rasterizer: &mut TextRasterizer,
) {
    if !viewer.is_open {
        return;
    }
    let vw = rasterizer.width as f32;
    let vh = rasterizer.height as f32;
    let clipper = MapCanvasClipper::new_4x3(vw, vh);

    // 1. Clear full screen to black
    rasterizer.fill_rect(0, 0, rasterizer.width, rasterizer.height, [0, 0, 0, 255]);

    // 2. Draw paper map sheet inside active 4:3 canvas (with zoom & pan)
    let (x0, y0) = viewer.project_to_viewport(0.0, 0.0, &clipper);
    let (x1, y1) = viewer.project_to_viewport(1.0, 1.0, &clipper);
    if let Some((cx0, cy0, cx1, cy1, cu0, cv0, cu1, cv1)) =
        clipper.clip_decal_quad(x0, y0, x1, y1, 0.0, 0.0, 1.0, 1.0)
    {
        let dx = cx0.floor().max(0.0) as usize;
        let dy = cy0.floor().max(0.0) as usize;
        let dw = (cx1 - cx0).ceil().max(1.0) as usize;
        let dh = (cy1 - cy0).ceil().max(1.0) as usize;
        if let Some((rgba, sw, sh)) = sheet_rgba {
            let tris = [
                (cx0, cy0, cu0, cv0),
                (cx1, cy0, cu1, cv0),
                (cx1, cy1, cu1, cv1),
                (cx0, cy0, cu0, cv0),
                (cx1, cy1, cu1, cv1),
                (cx0, cy1, cu0, cv1),
            ];
            rasterizer.blit_overlay_triangles(&tris, rgba, sw, sh, 1.0);
        } else {
            // Aged parchment paper fallback when retail `.tex` is unavailable
            rasterizer.fill_rect(dx, dy, dw, dh, [186, 172, 142, 255]);
            rasterizer.draw_rect_outline(
                dx + 8,
                dy + 8,
                dw.saturating_sub(16),
                dh.saturating_sub(16),
                2,
                [92, 76, 56, 220],
            );
        }
    }

    // 3. Draw `EventFlags`-driven red marker pen annotations (`ChizuMarkerDraw`)
    let annotations = annotations_for_map(viewer.current_map_id, flags);
    for mark in &annotations {
        draw_red_pen_annotation(viewer, mark, &clipper, font, rasterizer);
    }

    // 4. Draw clipped green player heading triangle (`MapCanvasClipper`)
    let (px, py) = viewer.project_to_viewport(player_norm_pos.0, player_norm_pos.1, &clipper);
    let marker = MapPlayerMarker {
        x: px,
        y: py,
        yaw_radians: player_yaw_rad,
        radius: 9.0 * (1.0 + viewer.zoom_t * 0.35),
    };
    if let Some(verts) = clipper.clip_player_marker(&marker) {
        rasterizer.fill_triangle(verts, [32, 224, 72, 245]);
    }

    // 5. Enforce pure black widescreen/ultrawide pillarboxes (`SetBlackPillarBoxes`)
    clipper.draw_black_pillarbox_bars(rasterizer);

    // 6. Subtle header & controls bar inside canvas bounds
    let left_px = clipper.layout.canvas_left.round().max(0.0) as usize;
    let cw_px = (clipper.layout.canvas_right - clipper.layout.canvas_left)
        .round()
        .max(1.0) as usize;
    let title = map_spec(viewer.current_map_id)
        .map(|s| s.title)
        .unwrap_or("Silent Hill Map");
    let zoom_label = if viewer.zoom_step == MapZoomStep::FullSheet {
        "[Confirm] Zoom In   [Arrows] Floors/Areas   [Cancel/M] Close"
    } else {
        "[Confirm] Zoom Out   [Arrows] Pan   [Cancel/M] Close"
    };
    rasterizer.fill_rect(left_px, 0, cw_px, 34, [0, 0, 0, 175]);
    font.draw_line_centered(
        rasterizer,
        title,
        (left_px + cw_px / 2, 8),
        16,
        Sh2TextColor::GoldSelected,
        true,
    );
    let bottom_y = rasterizer.height.saturating_sub(28);
    rasterizer.fill_rect(left_px, bottom_y, cw_px, 28, [0, 0, 0, 175]);
    font.draw_line_centered(
        rasterizer,
        zoom_label,
        (left_px + cw_px / 2, bottom_y + 6),
        13,
        Sh2TextColor::White,
        true,
    );
}

fn draw_red_pen_annotation(
    viewer: &PaperMapViewer,
    mark: &MapAnnotation,
    clipper: &MapCanvasClipper,
    font: &AtlasFont,
    rasterizer: &mut TextRasterizer,
) {
    let (mx, my) = viewer.project_to_viewport(mark.norm_x, mark.norm_y, clipper);
    if mx < clipper.layout.canvas_left
        || mx > clipper.layout.canvas_right
        || my < clipper.layout.canvas_top
        || my > clipper.layout.canvas_bottom
    {
        return;
    }
    let alpha = if viewer.transcribing_flag_id == Some(mark.flag_id) {
        ((viewer.transcription_alpha as u32) * 255 / (TRANSCRIBE_FADE_TARGET_ALPHA as u32)).min(255)
            as u8
    } else {
        225
    };
    let red = [195, 24, 24, alpha];
    let ix = mx.round() as i32;
    let iy = my.round() as i32;

    match mark.kind {
        MapMarkerKind::LockedDoor => {
            rasterizer.draw_line(ix - 8, iy, ix + 8, iy, 3, red);
        }
        MapMarkerKind::JammedDoor => {
            rasterizer.draw_line(ix - 7, iy - 7, ix + 7, iy + 7, 2, red);
            rasterizer.draw_line(ix + 7, iy - 7, ix - 7, iy + 7, 2, red);
        }
        MapMarkerKind::UnlockedDoor => {
            rasterizer.draw_line(ix - 6, iy + 1, ix - 1, iy + 6, 2, red);
            rasterizer.draw_line(ix - 1, iy + 6, ix + 7, iy - 6, 2, red);
        }
        MapMarkerKind::CircledObjective => {
            rasterizer.draw_rect_outline(
                (ix - 10).max(0) as usize,
                (iy - 10).max(0) as usize,
                20,
                20,
                2,
                red,
            );
        }
        MapMarkerKind::Arrow => {
            rasterizer.draw_line(ix - 8, iy, ix + 8, iy, 2, red);
            rasterizer.draw_line(ix + 3, iy - 5, ix + 8, iy, 2, red);
            rasterizer.draw_line(ix + 3, iy + 5, ix + 8, iy, 2, red);
        }
        MapMarkerKind::QuestionMark | MapMarkerKind::TranscriptionNote => {
            font.draw_line(
                rasterizer,
                "?",
                ((ix - 4).max(0) as usize, (iy - 8).max(0) as usize),
                16,
                red,
                false,
            );
        }
    }

    if let Some(caption) = mark.caption {
        font.draw_line(
            rasterizer,
            caption,
            ((ix + 10).max(0) as usize, (iy - 6).max(0) as usize),
            12,
            red,
            true,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_paper_map_zoom_pan_and_transcription_timing() {
        let mut flags = EventFlags::new();
        flags.set(24, true); // Town Map
        let mut viewer = PaperMapViewer::new();
        viewer.open(1, false);
        assert_eq!(viewer.zoom_step, MapZoomStep::FullSheet);

        // Confirm toggles into ZoomingIn -> reaches ZoomedPan
        viewer.handle_nav(MenuNavAction::Confirm, &flags);
        assert_eq!(viewer.zoom_step, MapZoomStep::ZoomingIn);
        for _ in 0..30 {
            viewer.tick(1.0 / 60.0);
        }
        assert_eq!(viewer.zoom_step, MapZoomStep::ZoomedPan);

        // Pan while zoomed
        let before_x = viewer.center_x;
        viewer.handle_nav(MenuNavAction::Right, &flags);
        assert!(viewer.center_x > before_x);

        // Test MapTranscription 60 Hz state machine
        viewer.start_transcription(0x04e9);
        assert_eq!(viewer.transcription_phase, MapTranscriptionPhase::WaitInit);
        for _ in 0..160 {
            viewer.tick(1.0 / 60.0);
        }
        assert_eq!(viewer.transcription_phase, MapTranscriptionPhase::Idle);
        assert_eq!(viewer.transcription_alpha, TRANSCRIBE_FADE_TARGET_ALPHA);
    }
}
