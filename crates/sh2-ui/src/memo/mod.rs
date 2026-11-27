//! Authentic 1:1 PS2 / SH2:EE Collected Memo List & Multi-Page Reader (`Event/memo.c`).
//!
//! # Ground Truth References
//! - **PS2 Prototype (`SLUS_202.28`) & Retail PC (`sh2pc.exe`):**
//!   - `MemoMain` (`0x002304e0` / `0x0049abd0`), `MemoCommandCheck` (`0x00230570` / `0x00498de0`),
//!     `MemoInit` (`0x002305f0` / `0x00498e50`), `MemoSelect` (`0x00230810` / `0x0049a0c0`),
//!     `MemoDisplay` (`0x00230c70` / `0x0049a750`), `MemoEnd` (`0x00231320` / `0x00498fa0`),
//!     `MemoPictureLoad` (`0x00231390` / `0x00499000`), `MemoSelectBarDraw` (`0x002315c0` / `0x00499180`),
//!     `MemoPictureLayerDraw*` (`0x002316d0..0x002321ef` / `0x00499250..0x004999af`),
//!     `MemoMessageWallet` (`0x002321f0` / `0x004999b0`).
//!   - `DAT_0122b313` / `DAT_01dbbf80._3_1_`: last-selected catalog index persisted across opens.
//! - **SH2:EE Patches:**
//!   - `MemoFix.cpp` (`0x0049914d`): synchronizes both bottom-layer and top-layer backdrop RGB
//!     brightness (`MemoBottomBrightnessAddr` & `MemoTopBrightnessAddr`) during dim transitions.

pub mod catalog;
pub mod overlays;
pub mod overlays_retail;
pub mod render;

pub use catalog::{
    collected_memos, memo_command_check, resolve_memo_textures, MemoDisplayMode, MemoEntry,
    DEFAULT_MEMO_BACKDROP_TEX, MEMO_CATALOG, MEMO_CATALOG_COUNT, STATUE_BOX_TEX,
};
pub use overlays::{
    apply_memo_dynamic_codes, build_angel_ring_quads, build_guruguru_quads, build_hair_quad,
    build_memo_layer_quads, build_safe_lock_quad, build_tablet_quads, format_memo_keypad_code,
    format_memo_wallet_codes, MemoLayerQuad, GURUGURU_POS_Q4, GURUGURU_TEX_UVS,
};
pub use render::{
    draw_memo_reader, memo_render_fingerprint, MemoRenderContext, MemoRenderTextures,
};

use crate::menu::{MenuNavAction, MenuSoundCue};
use sh2_assets::formats::mes::MessageTable;
use sh2_core::events::EventFlags;

/// Maximum non-scrolling rows (`count < 12`, `MemoSelect` `0x00230810` line 211).
pub const MEMO_LIST_MAX_STATIC_ROWS: usize = 11;
/// Visible circular rows when `count >= 12` (`MemoSelect` `0x00230810` line 221: `0xd`).
pub const MEMO_LIST_SCROLL_ROWS: usize = 13;
/// Backdrop dimming rate in `MemoDisplay` (`0x00230c70` line 355: `_alpha_563 -= dt`, `1.0 / s`).
pub const MEMO_DIM_RATE: f32 = 1.0;

/// Sub-phase of the Memo Reader (`MemoSelect` `0x00230810` vs 3-mode `MemoDisplay` `0x00230c70`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MemoPhase {
    /// Browsing the chronological list of collected memos (`MemoSelect`).
    #[default]
    List,
    /// Viewing the full-brightness (`0x80`) still + overlays (`MemoDisplay` step `3`, modes `0` & `2`).
    ViewingImage,
    /// Reading multi-page `.mes` text over the dimmed (`0x80 -> 0x40` at `1.0/s`) backdrop (`step 5`).
    ViewingText,
}

/// Single visible row computed by [`MemoReader::visible_list_rows`] (`MemoSelect` `0x00230810`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoListRow {
    /// Index into `MemoReader::entries`.
    pub entry_idx: usize,
    /// Top Y coordinate in PS2 `512x512` virtual space (`0x30 + row*0x20` or `256 - count*16 + i*32`).
    pub y_ps2: i32,
    /// PS2 `0..=0x80` font alpha (`0x10`, `0x20`, `0x40`, or `0x80`).
    pub alpha_ps2: u8,
    /// Whether this row is the active selection (`MemoSelectBarDraw`).
    pub is_selected: bool,
}

/// Action emitted by [`MemoReader`] to `sh2-app`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoAction {
    /// Stay in Memo Reader.
    None,
    /// Close Memo Reader and return to Inventory.
    Close,
    /// Active memo's backdrop and optional overlay layer `.tex` paths changed; `sh2-app` should load them.
    TexturesChanged {
        /// Base backdrop `.tex` path (`p_memo.tex` or entry-specific still).
        backdrop_path: &'static str,
        /// Optional overlay layer `.tex` path (for SafeLock, Guruguru, Hair, AngelRing, Tablet).
        layer_path: Option<&'static str>,
    },
}

/// Authentic PS2 / SH2:EE Memo List & Multi-Page Reader state machine (`Event/memo.c`).
#[derive(Debug, Clone)]
pub struct MemoReader {
    /// Whether the Memo Reader overlay is currently open.
    pub is_open: bool,
    /// Active phase (`List`, `ViewingImage`, or `ViewingText`).
    pub phase: MemoPhase,
    /// Chronological list of collected memos from `EventFlags`.
    pub entries: Vec<&'static MemoEntry>,
    /// Selected index in `entries` (`_list_point` in `MemoSelect` `0x00230810`).
    pub selected_index: usize,
    /// Visual scroll offset (`_disp_point` in `-3..=3` in `MemoSelect` `0x00230810`).
    pub disp_point: i32,
    /// Persisted last-selected catalog index (`DAT_0122b313` in `MemoInit`/`MemoEnd`).
    pub last_selected_catalog_index: u8,
    /// Active page index (`0..page_count`) when `phase == MemoPhase::ViewingText`.
    pub current_page: usize,
    /// Backdrop dimming interpolation `_alpha_563` (`1.0` = `0x80`, `0.0` = `0x40`, `MemoDisplay` `0x00230c70`).
    pub dim_alpha: f32,
    /// Fullscreen fade-in scrim alpha (`1.0` = solid black, `0.0` = visible; `Event/memo.c:130`
    /// `ScreenEffectFadeStart(0, 4)`).
    pub screen_fade_alpha: f32,
}

impl Default for MemoReader {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoReader {
    /// Creates a closed `MemoReader`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            is_open: false,
            phase: MemoPhase::List,
            entries: Vec::new(),
            selected_index: 0,
            disp_point: 0,
            last_selected_catalog_index: 0,
            current_page: 0,
            dim_alpha: 1.0,
            screen_fade_alpha: 0.0,
        }
    }

    /// Starts the `Event/memo.c:130` (`ScreenEffectFadeStart(0, 4)`) `0.5` s fade-in from black.
    pub fn start_fade_in(&mut self) {
        self.screen_fade_alpha = 1.0;
    }

    /// Opens the Memo Reader matching `MemoInit` (`0x002305f0` / `0x00498e50`):
    /// restores `_list_point` from `last_selected_catalog_index` (`DAT_0122b313`) by counting
    /// collected entries up to that catalog index, and resets `_disp_point = 0`.
    pub fn open(&mut self, flags: &EventFlags) {
        self.is_open = true;
        self.phase = MemoPhase::List;
        self.entries = collected_memos(flags);
        let saved = self.last_selected_catalog_index as usize;
        let mut list_point = saved as i32;
        for entry in MEMO_CATALOG
            .iter()
            .take(saved.min(MEMO_CATALOG_COUNT.saturating_sub(1)) + 1)
        {
            if !flags.get(entry.flag_id as usize) {
                list_point -= 1;
            }
        }
        self.selected_index = if self.entries.is_empty() {
            0
        } else {
            (list_point.max(0) as usize).min(self.entries.len() - 1)
        };
        self.disp_point = 0;
        self.current_page = 0;
        self.dim_alpha = 1.0;
    }

    /// Closes the Memo Reader matching `MemoEnd` (`0x00231320` / `0x00498fa0`), persisting
    /// `last_selected_catalog_index` (`DAT_0122b313`).
    pub fn close(&mut self) {
        if let Some(entry) = self.selected_entry() {
            self.last_selected_catalog_index = entry.index as u8;
        }
        self.is_open = false;
        self.phase = MemoPhase::List;
        self.current_page = 0;
    }

    /// Returns the currently highlighted [`MemoEntry`], if any.
    #[must_use]
    pub fn selected_entry(&self) -> Option<&'static MemoEntry> {
        self.entries.get(self.selected_index).copied()
    }

    /// Returns the current PS2 `0..=0x80` backdrop/layer brightness byte:
    /// - `0x60` in `MemoSelect` (`0x00230810` line 209)
    /// - `(dim_alpha * 64.0) as u8 + 0x40` in `MemoDisplay` (`0x00230c70` line 294: `0x80` -> `0x40`).
    #[must_use]
    pub fn backdrop_brightness_ps2(&self) -> u8 {
        match self.phase {
            MemoPhase::List => 0x60,
            MemoPhase::ViewingImage | MemoPhase::ViewingText => {
                ((self.dim_alpha.clamp(0.0, 1.0) * 64.0) as u8).saturating_add(0x40)
            }
        }
    }

    /// Computes the visible rows for `MemoSelect` (`0x00230810` lines 211-248):
    /// - `count < 12`: `count` rows centered at `y = 256 - count * 16 + i * 32`, alpha `0x80`.
    /// - `count >= 12`: 13 circular rows at `y = 48 + row * 32`, edge fade `0x10/0x20/0x40/0x80`.
    #[must_use]
    pub fn visible_list_rows(&self) -> Vec<MemoListRow> {
        let count = self.entries.len();
        if count == 0 {
            return Vec::new();
        }
        if count < 12 {
            (0..count)
                .map(|i| MemoListRow {
                    entry_idx: i,
                    y_ps2: 0x100 - (count as i32) * 0x10 + (i as i32) * 0x20,
                    alpha_ps2: 0x80,
                    is_selected: i == self.selected_index,
                })
                .collect()
        } else {
            let count_i = count as i32;
            (0..MEMO_LIST_SCROLL_ROWS)
                .map(|row| {
                    let mut idx = (row as i32) + (self.selected_index as i32) + self.disp_point - 6;
                    if idx < 0 {
                        idx += count_i;
                    }
                    if idx >= count_i {
                        idx -= count_i;
                    }
                    let entry_idx = idx.rem_euclid(count_i) as usize;
                    let alpha_ps2 = match row {
                        0 | 12 => 0x10,
                        1 | 11 => 0x20,
                        2 | 10 => 0x40,
                        _ => 0x80,
                    };
                    MemoListRow {
                        entry_idx,
                        y_ps2: 0x30 + (row as i32) * 0x20,
                        alpha_ps2,
                        is_selected: entry_idx == self.selected_index,
                    }
                })
                .collect()
        }
    }

    /// Resolves the localized title for `entry` from `mes_table`.
    #[must_use]
    pub fn entry_title<'a>(
        entry: &'static MemoEntry,
        mes_table: Option<&'a MessageTable>,
    ) -> &'a str {
        mes_table
            .and_then(|t| t.get_text(entry.title_msg_idx as usize))
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("Memo")
    }

    /// Resolves the multi-page body text for `entry` from `mes_table` (`MemoDisplay` `0x00230c70`).
    #[must_use]
    pub fn entry_pages(entry: &'static MemoEntry, mes_table: Option<&MessageTable>) -> Vec<String> {
        if let Some(msg) = mes_table.and_then(|t| t.get(entry.body_msg_idx as usize)) {
            if !msg.pages.is_empty() {
                return msg.pages.clone();
            }
            if !msg.text.trim().is_empty() {
                return vec![msg.text.clone()];
            }
        }
        vec![String::new()]
    }

    /// Advances the `MemoDisplay` (`0x00230c70` lines 354-358) `0x80 -> 0x40` backdrop dimming
    /// ramp at exact **`1.0 / s`** while `phase == MemoPhase::ViewingText`.
    pub fn tick(&mut self, dt: f32) {
        if !self.is_open {
            return;
        }
        let dt = dt.clamp(0.0, 0.1);
        if self.screen_fade_alpha > 0.0 {
            self.screen_fade_alpha = (self.screen_fade_alpha - dt * 2.0).max(0.0);
        }
        if self.phase == MemoPhase::ViewingText {
            self.dim_alpha = (self.dim_alpha - dt * MEMO_DIM_RATE).max(0.0);
        }
    }

    /// Handles directional, confirm, and cancel navigation (`MemoSelect` & 3-mode `MemoDisplay`).
    pub fn handle_nav(
        &mut self,
        nav: MenuNavAction,
        flags: &EventFlags,
        mes_table: Option<&MessageTable>,
    ) -> (MenuSoundCue, MemoAction) {
        if !self.is_open {
            return (MenuSoundCue::None, MemoAction::None);
        }

        match self.phase {
            MemoPhase::List => match nav {
                MenuNavAction::Up => {
                    if !self.entries.is_empty() {
                        if self.selected_index == 0 {
                            self.selected_index = self.entries.len() - 1;
                        } else {
                            self.selected_index -= 1;
                        }
                        self.disp_point = (self.disp_point + 1).min(3);
                        (MenuSoundCue::CursorMove, MemoAction::None)
                    } else {
                        (MenuSoundCue::None, MemoAction::None)
                    }
                }
                MenuNavAction::Down => {
                    if !self.entries.is_empty() {
                        self.selected_index = (self.selected_index + 1) % self.entries.len();
                        self.disp_point = (self.disp_point - 1).max(-3);
                        (MenuSoundCue::CursorMove, MemoAction::None)
                    } else {
                        (MenuSoundCue::None, MemoAction::None)
                    }
                }
                MenuNavAction::Confirm => {
                    if let Some(entry) = self.selected_entry() {
                        self.last_selected_catalog_index = entry.index as u8;
                        self.current_page = 0;
                        let (backdrop_path, layer_path) = resolve_memo_textures(entry, flags);
                        match entry.display_mode {
                            MemoDisplayMode::TextImmediate => {
                                self.dim_alpha = 0.0;
                                self.phase = MemoPhase::ViewingText;
                            }
                            MemoDisplayMode::ImageOnly | MemoDisplayMode::ImageThenText => {
                                self.dim_alpha = 1.0;
                                self.phase = MemoPhase::ViewingImage;
                            }
                        }
                        (
                            MenuSoundCue::Confirm,
                            MemoAction::TexturesChanged {
                                backdrop_path,
                                layer_path,
                            },
                        )
                    } else {
                        (MenuSoundCue::Cancel, MemoAction::None)
                    }
                }
                MenuNavAction::Cancel => {
                    self.close();
                    (MenuSoundCue::Cancel, MemoAction::Close)
                }
                MenuNavAction::Left | MenuNavAction::Right => {
                    (MenuSoundCue::None, MemoAction::None)
                }
            },
            MemoPhase::ViewingImage => match nav {
                MenuNavAction::Confirm | MenuNavAction::Cancel => {
                    let mode = self
                        .selected_entry()
                        .map_or(MemoDisplayMode::ImageOnly, |e| e.display_mode);
                    if mode == MemoDisplayMode::ImageThenText {
                        // MemoDisplay step 3 -> step 4: dim 0x80 -> 0x40 at 1.0/s and show text
                        self.phase = MemoPhase::ViewingText;
                        self.current_page = 0;
                        (MenuSoundCue::Confirm, MemoAction::None)
                    } else {
                        self.phase = MemoPhase::List;
                        (
                            MenuSoundCue::Cancel,
                            MemoAction::TexturesChanged {
                                backdrop_path: DEFAULT_MEMO_BACKDROP_TEX,
                                layer_path: None,
                            },
                        )
                    }
                }
                _ => (MenuSoundCue::None, MemoAction::None),
            },
            MemoPhase::ViewingText => {
                let page_count = self
                    .selected_entry()
                    .map(|e| Self::entry_pages(e, mes_table).len().max(1))
                    .unwrap_or(1);
                match nav {
                    MenuNavAction::Confirm | MenuNavAction::Right | MenuNavAction::Down => {
                        if self.current_page + 1 < page_count {
                            self.current_page += 1;
                            (MenuSoundCue::CursorMove, MemoAction::None)
                        } else {
                            self.phase = MemoPhase::List;
                            self.current_page = 0;
                            (
                                MenuSoundCue::Cancel,
                                MemoAction::TexturesChanged {
                                    backdrop_path: DEFAULT_MEMO_BACKDROP_TEX,
                                    layer_path: None,
                                },
                            )
                        }
                    }
                    MenuNavAction::Left | MenuNavAction::Up => {
                        if self.current_page > 0 {
                            self.current_page -= 1;
                            (MenuSoundCue::CursorMove, MemoAction::None)
                        } else {
                            (MenuSoundCue::None, MemoAction::None)
                        }
                    }
                    MenuNavAction::Cancel => {
                        self.phase = MemoPhase::List;
                        self.current_page = 0;
                        (
                            MenuSoundCue::Cancel,
                            MemoAction::TexturesChanged {
                                backdrop_path: DEFAULT_MEMO_BACKDROP_TEX,
                                layer_path: None,
                            },
                        )
                    }
                }
            }
        }
    }
}
