//! Pre-rendered Cutaway Still & Interactive Puzzle Overlay Subsystem (`EvSubPictureDisplay` / `prog_overlay.c`).
//!
//! Renders 2D cutaway stills (`data/pic` or `sh2e/pic`) and generates animated/rotatable overlay
//! sprite quads in the PC's `-256..256` sprite coordinate space using the authentic binary tables
//! extracted from `sh2pc.exe` (Director's Cut).

use crate::events::{EventFlags, PuzzleState};

mod draw;
mod tables;
pub use tables::{
    original_sheet_dimensions, OV_BOX, OV_CLOCK_UV, OV_COIN, OV_DRUM_ABC, OV_DRUM_POS, OV_ELEVATOR,
    OV_ELEVATOR2, OV_MARKER, OV_TEIEN, OV_VIDEO, OV_WALL, SHEET_ORIGINAL_DIMENSIONS,
};

/// Interactive puzzle or cutaway still overlay kind (`prog_overlay.c`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayKind {
    /// No sprite sheet overlay over the active still.
    None,
    /// Room 208 closed clock face (`apt_clock_close` + `apt/clock_hari`).
    ClockClosed,
    /// Room 208 open clock face (`apt_clock_open` + `apt/clock_hari`).
    ClockOpen,
    /// Room 203 / W2F safe dial (`apt_p_safe_close` + `apt/p_safe_close2`).
    Safe,
    /// Room 105 five-slot coin desk (`apt_p_desk_coin` + `apt/p_desk_coin_coin`).
    Coins,
    /// Hospital 2F locked box (`hsp_p_box` + `hsp/pboxkey01`).
    Box,
    /// Hospital 3F wall symbol blood scrawl (`hsp_p_boxnumber` + `hsp/p_boxnumber_2`).
    BoxWall,
    /// Hospital Trick or Treat quiz box (`hsp_p_quiz` + `hsp/p_quiz_btn`).
    Quiz,
    /// Hotel cloakroom briefcase letter drums (`htl_p_trank` + `htl/dram`).
    Drums,
    /// Hotel 3F doors with Scarlet / Rust-colored eggs (`htl_p_egghollow` + `htl/egg`).
    Egg,
    /// Baldwin Mansion garden tombstone board puzzle (`add_p_teien` + `add/teien2`).
    Tombstone,
    /// Hotel Room 312 VCR (`htl_p_video` + `htl/p_video02`).
    Video,
    /// Hotel Room 202 open briefcase with marker (`htl_p_marker` + `htl/maker`).
    Marker,
    /// Hospital elevator button panel (`hsp_p_h_elevator` + `hsp/p_h_elevator_botan`).
    Elevator,
    /// Otherworld Hospital elevator button panel (`hsp_p_h_elevator_2` + `hsp/p_h_elevator_botan`).
    Elevator2,
    /// Labyrinth catacombs manacles (`dls_p_manacles_on` + `dls/p_manacles_off`).
    Manacles,
    /// Observation deck car seat Silent Hill map (`out_p_incar` + `out/p_incar_map`).
    InCar,
    /// Memo examine overlay (`0x12`, `0x20`, `0x21`, `0x23`, `0x29`, `0x35`, `0x36`).
    Memo(u8),
}

/// Single textured sprite quad emitted by `CutawayOverlayEngine` over the active cutaway still.
#[derive(Debug, Clone, PartialEq)]
pub struct OverlaySpriteQuad {
    /// Sprite sheet path under `pic/` (e.g. `"out/p_incar_map"`, `"apt/clock_hari"`).
    pub sheet_name: &'static str,
    /// Four corners `(x, y, u, v)` in PC screen space (`x: -256..256`, `y: -256..256`) and
    /// normalized texture coordinates (`u, v: 0.0..=1.0`).
    pub vertices: [(f32, f32, f32, f32); 4],
    /// Opacity in `0.0..=1.0`.
    pub alpha: f32,
}

impl OverlaySpriteQuad {
    /// Converts the four PC sprite-space vertices (`-256..256`) into a 6-vertex triangle pair
    /// `(x, y, u, v)` inside a target viewport rectangle `(vp_x, vp_y, vp_w, vp_h)`.
    pub fn to_viewport_triangles(
        &self,
        vp_x: f32,
        vp_y: f32,
        vp_w: f32,
        vp_h: f32,
    ) -> [(f32, f32, f32, f32); 6] {
        let map_pt = |(px, py, u, v): (f32, f32, f32, f32)| -> (f32, f32, f32, f32) {
            let sx = vp_x + (px + 256.0) * (vp_w / 512.0);
            let sy = vp_y + (py + 256.0) * (vp_h / 512.0);
            (sx, sy, u, v)
        };
        let p0 = map_pt(self.vertices[0]);
        let p1 = map_pt(self.vertices[1]);
        let p2 = map_pt(self.vertices[2]);
        let p3 = map_pt(self.vertices[3]);
        [p0, p1, p2, p0, p2, p3]
    }
}

/// Stateful cutaway still & interactive puzzle sprite overlay engine (`prog_overlay.c`).
#[derive(Debug, Clone)]
pub struct CutawayOverlayEngine {
    /// Active still picture identifier (e.g. `"out_p_incar"`, `"apt_clock_open"`), if any.
    active_picture: Option<String>,
    /// Active overlay kind derived from `active_picture`.
    kind: OverlayKind,
    /// Active sprite sheet name (`"out/p_incar_map"`, `"apt/clock_hari"`, etc.), if any.
    sheet_name: Option<&'static str>,
    /// Alpha fade for `dls_p_manacles_on` (`0.0..=1.0`).
    manacles_alpha: f32,
    /// Alpha fade for `out_p_incar` car seat map (`1.0` -> `0.0` over 1.0s when taken).
    incar_alpha: f32,
    /// Sprite quads generated for the current frame.
    pub(super) frame_sprites: Vec<OverlaySpriteQuad>,
}

impl Default for CutawayOverlayEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl CutawayOverlayEngine {
    /// Creates an idle cutaway overlay engine.
    pub fn new() -> Self {
        Self {
            active_picture: None,
            kind: OverlayKind::None,
            sheet_name: None,
            manacles_alpha: 0.0,
            incar_alpha: 1.0,
            frame_sprites: Vec::new(),
        }
    }

    /// Returns the currently displayed base cutaway still identifier, if any.
    pub fn active_picture(&self) -> Option<&str> {
        self.active_picture.as_deref()
    }

    /// Returns the active overlay kind.
    pub fn kind(&self) -> OverlayKind {
        self.kind
    }

    /// Returns the active sprite sheet identifier, if any.
    pub fn sheet_name(&self) -> Option<&'static str> {
        self.sheet_name
    }

    /// Returns the sprite quads generated on the most recent frame.
    pub fn frame_sprites(&self) -> &[OverlaySpriteQuad] {
        &self.frame_sprites
    }

    /// Sets the alpha of the open manacles overlay (`0x57fd50`).
    pub fn set_manacles_alpha(&mut self, alpha: f32) {
        self.manacles_alpha = alpha.clamp(0.0, 1.0);
    }

    /// Sets the alpha of the car seat map overlay (`0x5a0b80`).
    pub fn set_incar_alpha(&mut self, alpha: f32) {
        self.incar_alpha = alpha.clamp(0.0, 1.0);
    }

    /// Returns the current car seat map overlay alpha (`0.0..=1.0`).
    pub fn incar_alpha(&self) -> f32 {
        self.incar_alpha
    }

    /// Returns the current open manacles overlay alpha (`0.0..=1.0`).
    pub fn manacles_alpha(&self) -> f32 {
        self.manacles_alpha
    }

    /// Advances active overlay alpha transitions (`out_p_incar` map fade and `dls_p_manacles_on` fade).
    pub fn tick(&mut self, dt: f32, flags: &EventFlags) {
        if dt <= 0.0 {
            return;
        }
        match self.kind {
            OverlayKind::InCar => {
                if flags.get(28) {
                    self.incar_alpha = (self.incar_alpha - dt).max(0.0);
                } else {
                    self.incar_alpha = 1.0;
                }
            }
            OverlayKind::Manacles => {
                if flags.get(277) || flags.get(304) {
                    self.manacles_alpha = (self.manacles_alpha + dt).min(1.0);
                } else {
                    self.manacles_alpha = 0.0;
                }
            }
            _ => {}
        }
    }

    /// Activates a cutaway still (`prog_overlay_picture`), automatically binding the matching
    /// interactive puzzle sprite sheet (`SHEETS`) when applicable. Pass `None` to hide the still.
    pub fn set_picture(&mut self, pic: Option<&str>, flags: &EventFlags) {
        self.frame_sprites.clear();
        let Some(pic_str) = pic.filter(|s| !s.is_empty()) else {
            self.active_picture = None;
            self.kind = OverlayKind::None;
            self.sheet_name = None;
            return;
        };

        self.active_picture = Some(pic_str.to_string());
        let (kind, sheet) = match pic_str {
            "apt_clock_close" => (OverlayKind::ClockClosed, Some("apt/clock_hari")),
            "apt_clock_open" => (OverlayKind::ClockOpen, Some("apt/clock_hari")),
            "apt_p_safe_close" => (OverlayKind::Safe, Some("apt/p_safe_close2")),
            "apt_p_desk_coin" => (OverlayKind::Coins, Some("apt/p_desk_coin_coin")),
            "hsp_p_box" => (OverlayKind::Box, Some("hsp/pboxkey01")),
            "hsp_p_boxnumber" => (OverlayKind::BoxWall, Some("hsp/p_boxnumber_2")),
            "hsp_p_quiz" => (OverlayKind::Quiz, Some("hsp/p_quiz_btn")),
            "htl_p_trank" => (OverlayKind::Drums, Some("htl/dram")),
            "htl_p_egghollow" => (OverlayKind::Egg, Some("htl/egg")),
            "add_p_teien" => (OverlayKind::Tombstone, Some("add/teien2")),
            "htl_p_video" => (OverlayKind::Video, Some("htl/p_video02")),
            "htl_p_marker" => (OverlayKind::Marker, Some("htl/maker")),
            "hsp_p_h_elevator" => (OverlayKind::Elevator, Some("hsp/p_h_elevator_botan")),
            "hsp_p_h_elevator_2" => (OverlayKind::Elevator2, Some("hsp/p_h_elevator_botan")),
            "dls_p_manacles_on" => {
                self.manacles_alpha = if flags.get(277) { 1.0 } else { 0.0 };
                (OverlayKind::Manacles, Some("dls/p_manacles_off"))
            }
            "out_p_incar" => {
                self.incar_alpha = if flags.get(28) { 0.0 } else { 1.0 };
                (OverlayKind::InCar, Some("out/p_incar_map"))
            }
            _ => (OverlayKind::None, None),
        };
        self.kind = kind;
        self.sheet_name = sheet;
    }

    /// Evaluates and emits the sprite quads for the current frame (`prog_overlay_frame`).
    ///
    /// `extra` carries context-specific state (e.g. the memo index for `OverlayKind::Memo`, or the
    /// current elevator floor bitmask `1..=8` for `OverlayKind::Elevator` / `Elevator2`).
    pub fn evaluate_frame(
        &mut self,
        flags: &EventFlags,
        puzzle: &PuzzleState,
        riddle_difficulty: u8,
        extra: i32,
    ) -> &[OverlaySpriteQuad] {
        self.frame_sprites.clear();

        if self.kind == OverlayKind::None {
            // Memo overlays when viewing specific stills in the memo screen (0x4997c0..0x499d00)
            match extra {
                0x12 => {
                    if !flags.get(120) {
                        self.emit_rect(
                            "apt/p_dust_out_coin",
                            24.0,
                            37.0,
                            56.0,
                            69.0,
                            0.0,
                            0.0,
                            32.0,
                            32.0,
                            1.0,
                        );
                    }
                }
                0x20 => {
                    if !flags.get(186) {
                        self.emit_rect(
                            "hsp/p_hair_hair",
                            -89.0,
                            12.0,
                            57.0,
                            50.0,
                            0.0,
                            0.0,
                            146.0,
                            38.0,
                            1.0,
                        );
                    }
                }
                0x21 => {
                    if !flags.get(187) {
                        self.emit_rect(
                            "hsp/p_female_ring",
                            -21.0,
                            -65.0,
                            5.0,
                            -17.0,
                            0.0,
                            0.0,
                            26.0,
                            48.0,
                            1.0,
                        );
                    }
                }
                0x23 => {
                    for i in 0..3usize {
                        if flags.get(278 + i) {
                            let x = [-31.0f32, -89.0, 29.0][i];
                            let u = [0.0f32, 64.0, 128.0][i];
                            let v = [0.0f32, 0.0, 128.0][i];
                            self.emit_rect(
                                "dls/p_tablet_plate",
                                x,
                                56.0,
                                x + 60.0,
                                122.0,
                                u,
                                v,
                                u + 60.0,
                                v + 66.0,
                                1.0,
                            );
                        }
                    }
                }
                0x29 => {
                    for i in 0..3usize {
                        if flags.get(382 + i) {
                            let x = [-225.0f32, -45.0, 134.0][i];
                            self.emit_rect(
                                "htl/egg",
                                x,
                                -123.0,
                                x + 48.0,
                                -68.0,
                                0.0,
                                0.0,
                                48.0,
                                55.0,
                                1.0,
                            );
                        }
                    }
                }
                0x35 => {
                    let prev_sheet = self.sheet_name;
                    self.sheet_name = Some("htl/p_video02");
                    self.draw_video(flags);
                    self.sheet_name = prev_sheet;
                }
                0x36 => {
                    let prev_sheet = self.sheet_name;
                    self.sheet_name = Some("htl/maker");
                    self.draw_marker(flags, riddle_difficulty);
                    self.sheet_name = prev_sheet;
                }
                _ => {}
            }
            return &self.frame_sprites;
        }

        match self.kind {
            OverlayKind::ClockClosed => self.draw_clock(puzzle, false),
            OverlayKind::ClockOpen => self.draw_clock(puzzle, true),
            OverlayKind::Safe => self.draw_safe(flags),
            OverlayKind::Coins => self.draw_coins(flags),
            OverlayKind::Box => self.draw_box(flags, puzzle),
            OverlayKind::BoxWall => self.draw_box_wall(puzzle),
            OverlayKind::Quiz => self.draw_quiz(flags),
            OverlayKind::Drums => self.draw_drums(flags, puzzle),
            OverlayKind::Egg => self.draw_egg(flags),
            OverlayKind::Tombstone => self.draw_tombstone(flags),
            OverlayKind::Video => self.draw_video(flags),
            OverlayKind::Marker => self.draw_marker(flags, riddle_difficulty),
            OverlayKind::Elevator => self.draw_elevator(&OV_ELEVATOR, extra),
            OverlayKind::Elevator2 => self.draw_elevator(&OV_ELEVATOR2, extra),
            OverlayKind::Manacles => {
                if self.manacles_alpha > 0.0 {
                    self.emit_rect(
                        "dls/p_manacles_off",
                        -256.0,
                        -256.0,
                        256.0,
                        256.0,
                        0.0,
                        0.0,
                        512.0,
                        512.0,
                        self.manacles_alpha,
                    );
                }
            }
            OverlayKind::InCar => {
                if self.incar_alpha > 0.0 {
                    // 0x5a0b80: (-127, -234) size (256, 256)
                    self.emit_rect(
                        "out/p_incar_map",
                        -127.0,
                        -234.0,
                        129.0,
                        22.0,
                        0.0,
                        0.0,
                        256.0,
                        256.0,
                        self.incar_alpha,
                    );
                }
            }
            OverlayKind::None | OverlayKind::Memo(_) => {}
        }

        &self.frame_sprites
    }
}
