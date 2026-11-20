//! Authentic 1:1 PS2 / SH2:EE Collected Memo List & Multi-Page Reader (`Event/memo.c`).
//!
//! # Ground Truth References
//! - **PS2 Prototype (`SLUS_202.28`):**
//!   - `MemoMain` (`0x002304e0`), `MemoCommandCheck` (`0x00230570`), `MemoInit` (`0x002305f0`),
//!     `MemoSelect` (`0x00230810`), `MemoDisplay` (`0x00230c70`), `MemoPictureLoad` (`0x00231390`).
//!   - `memo_list` (`0x0034ced0`, 45 chronological entries).
//!   - `data_etc_message_memo_msg_e_mes` (`0x003a17d0`) / `memo_x_msg_e.mes` multi-page `.mes` text.
//!   - `data_pic_etc_p_memo_tex` (`0x003a1c60`) inspection still backdrop.
//! - **SH2:EE Patches:**
//!   - `MemoFix.cpp` (`0x0049914d`): synchronizes both bottom-layer and top-layer backdrop RGB
//!     brightness (`MemoBottomBrightnessAddr` & `MemoTopBrightnessAddr`) during fade transitions.

pub mod catalog;

pub use catalog::{
    collected_memos, memo_command_check, MemoEntry, DEFAULT_MEMO_BACKDROP_TEX, MEMO_CATALOG,
    MEMO_CATALOG_COUNT,
};

use crate::atlas_font::{AtlasFont, Sh2TextColor};
use crate::font::TextRasterizer;
use crate::menu::{MenuNavAction, MenuSoundCue};
use sh2_assets::formats::mes::MessageTable;
use sh2_core::events::EventFlags;

/// Maximum visible rows in the chronological memo list before scrolling.
pub const MEMO_LIST_VISIBLE_ROWS: usize = 9;

/// Sub-phase of the Memo Reader (`MemoSelect` `0x00230810` vs `MemoDisplay` `0x00230c70`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MemoPhase {
    /// Browsing the chronological list of collected memos.
    #[default]
    List,
    /// Reading a selected memo's multi-page `.mes` text over its inspection still backdrop.
    Reading,
}

/// Action emitted by [`MemoReader`] to `sh2-app`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoAction {
    /// Stay in Memo Reader.
    None,
    /// Close Memo Reader and return to Inventory.
    Close,
    /// Active memo's backdrop `.tex` path changed; `sh2-app` should load it.
    BackdropChanged(&'static str),
}

/// Authentic PS2 / SH2:EE Memo List & Multi-Page Reader state machine (`Event/memo.c`).
#[derive(Debug, Clone)]
pub struct MemoReader {
    /// Whether the Memo Reader overlay is currently open.
    pub is_open: bool,
    /// Active phase (`List` vs `Reading`).
    pub phase: MemoPhase,
    /// Chronological list of collected memos from `EventFlags`.
    pub entries: Vec<&'static MemoEntry>,
    /// Selected index in `entries`.
    pub selected_index: usize,
    /// Top scroll offset in `entries`.
    pub scroll_offset: usize,
    /// Active page index (`0..page_count`) when `phase == MemoPhase::Reading`.
    pub current_page: usize,
    /// Synchronized top/bottom layer brightness fade (`0.0..=1.0`, `MemoFix.cpp` `0x0049914d`).
    pub layer_brightness: f32,
}

impl Default for MemoReader {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoReader {
    /// Creates a closed `MemoReader`.
    pub fn new() -> Self {
        Self {
            is_open: false,
            phase: MemoPhase::List,
            entries: Vec::new(),
            selected_index: 0,
            scroll_offset: 0,
            current_page: 0,
            layer_brightness: 1.0,
        }
    }

    /// Opens the Memo Reader, populating the chronological collected memo list from `flags`.
    pub fn open(&mut self, flags: &EventFlags) {
        self.is_open = true;
        self.phase = MemoPhase::List;
        self.entries = collected_memos(flags);
        self.selected_index = 0;
        self.scroll_offset = 0;
        self.current_page = 0;
        self.layer_brightness = 0.0;
    }

    /// Closes the Memo Reader.
    pub fn close(&mut self) {
        self.is_open = false;
        self.phase = MemoPhase::List;
        self.current_page = 0;
    }

    /// Returns the currently highlighted [`MemoEntry`], if any.
    pub fn selected_entry(&self) -> Option<&'static MemoEntry> {
        self.entries.get(self.selected_index).copied()
    }

    /// Resolves the localized title for `entry` from `mes_table`, falling back to `fallback_title`.
    pub fn entry_title<'a>(
        entry: &'static MemoEntry,
        mes_table: Option<&'a MessageTable>,
    ) -> &'a str {
        mes_table
            .and_then(|t| t.get_text(entry.title_msg_idx as usize))
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .unwrap_or(entry.fallback_title)
    }

    /// Resolves the multi-page body text for `entry` from `mes_table` (`MemoDisplay` `0x00230c70`).
    pub fn entry_pages(entry: &'static MemoEntry, mes_table: Option<&MessageTable>) -> Vec<String> {
        if let Some(msg) = mes_table.and_then(|t| t.get(entry.body_msg_idx as usize)) {
            if !msg.pages.is_empty() {
                return msg.pages.clone();
            }
            if !msg.text.trim().is_empty() {
                return vec![msg.text.clone()];
            }
        }
        vec![format!(
            "{}\n\n(Full localized text loads from data/etc/message/memo_msg_e.mes.)",
            entry.fallback_title
        )]
    }

    /// Advances the synchronized top & bottom backdrop brightness fade (`MemoFix.cpp` `0x0049914d`).
    pub fn tick(&mut self, dt: f32) {
        if !self.is_open {
            return;
        }
        self.layer_brightness = (self.layer_brightness + dt.clamp(0.0, 0.1) * 4.0).min(1.0);
    }

    /// Handles directional, confirm, and cancel navigation (`MemoSelect` & `MemoDisplay`).
    pub fn handle_nav(
        &mut self,
        nav: MenuNavAction,
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
                            self.scroll_offset =
                                self.entries.len().saturating_sub(MEMO_LIST_VISIBLE_ROWS);
                        } else {
                            self.selected_index -= 1;
                            if self.selected_index < self.scroll_offset {
                                self.scroll_offset = self.selected_index;
                            }
                        }
                        (MenuSoundCue::CursorMove, MemoAction::None)
                    } else {
                        (MenuSoundCue::None, MemoAction::None)
                    }
                }
                MenuNavAction::Down => {
                    if !self.entries.is_empty() {
                        self.selected_index = (self.selected_index + 1) % self.entries.len();
                        if self.selected_index == 0 {
                            self.scroll_offset = 0;
                        } else if self.selected_index >= self.scroll_offset + MEMO_LIST_VISIBLE_ROWS
                        {
                            self.scroll_offset = self.selected_index + 1 - MEMO_LIST_VISIBLE_ROWS;
                        }
                        (MenuSoundCue::CursorMove, MemoAction::None)
                    } else {
                        (MenuSoundCue::None, MemoAction::None)
                    }
                }
                MenuNavAction::Confirm => {
                    if let Some(entry) = self.selected_entry() {
                        self.phase = MemoPhase::Reading;
                        self.current_page = 0;
                        self.layer_brightness = 0.0;
                        (
                            MenuSoundCue::Confirm,
                            MemoAction::BackdropChanged(entry.backdrop_tex_path),
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
            MemoPhase::Reading => {
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
                            (MenuSoundCue::Cancel, MemoAction::None)
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
                        (MenuSoundCue::Cancel, MemoAction::None)
                    }
                }
            }
        }
    }
}

/// Renders the Memo List or Multi-Page Memo Reader onto `rasterizer` with synchronized
/// top/bottom layer brightness (`MemoFix.cpp` `0x0049914d`).
pub fn draw_memo_reader(
    reader: &MemoReader,
    mes_table: Option<&MessageTable>,
    backdrop_rgba: Option<(&[u8], usize, usize)>,
    font: &AtlasFont,
    rasterizer: &mut TextRasterizer,
) {
    if !reader.is_open {
        return;
    }

    let w = rasterizer.width;
    let h = rasterizer.height;
    let sx = w as f32 / 640.0;
    let sy = h as f32 / 480.0;

    // 1. Base black screen + inspection still backdrop (`p_memo.tex` / custom memo still)
    rasterizer.fill_rect(0, 0, w, h, [0, 0, 0, 255]);
    if let Some((rgba, bw, bh)) = backdrop_rgba {
        // Synchronized brightness across both bottom & top layers (`MemoFix.cpp`)
        let alpha = reader.layer_brightness.clamp(0.0, 1.0);
        rasterizer.blit_rgba_scaled(0, 0, w, h, rgba, bw, bh, alpha);
    } else {
        let shade = (28.0 * reader.layer_brightness).round() as u8;
        rasterizer.fill_rect(0, 0, w, h, [shade, shade, shade.saturating_add(4), 255]);
    }

    let title_px = ((20.0 * sy).round() as usize).clamp(14, 30);
    let body_px = ((15.0 * sy).round() as usize).clamp(11, 24);
    let small_px = ((12.0 * sy).round() as usize).clamp(10, 18);

    match reader.phase {
        MemoPhase::List => {
            // Subtle readable backdrop panel for the chronological memo list
            let px = (56.0 * sx).round() as usize;
            let py = (44.0 * sy).round() as usize;
            let pw = (528.0 * sx).round() as usize;
            let ph = (392.0 * sy).round() as usize;
            rasterizer.fill_rect(px, py, pw, ph, [8, 10, 12, 210]);
            rasterizer.draw_rect_outline(px, py, pw, ph, 2, [145, 132, 102, 230]);

            font.draw_line_centered(
                rasterizer,
                "MEMO",
                (w / 2, py + 16),
                title_px,
                Sh2TextColor::GoldSelected,
                true,
            );

            if reader.entries.is_empty() {
                font.draw_line_centered(
                    rasterizer,
                    "No memos collected yet.",
                    (w / 2, h / 2),
                    body_px,
                    Sh2TextColor::MutedFooter,
                    true,
                );
            } else {
                let row_h = ((32.0 * sy).round() as usize).max(body_px + 8);
                let list_y = py + 56;
                let end = (reader.scroll_offset + MEMO_LIST_VISIBLE_ROWS).min(reader.entries.len());
                for (vis_idx, idx) in (reader.scroll_offset..end).enumerate() {
                    let entry = reader.entries[idx];
                    let ry = list_y + vis_idx * row_h;
                    let is_sel = idx == reader.selected_index;
                    if is_sel {
                        rasterizer.fill_rect(
                            px + 12,
                            ry.saturating_sub(2),
                            pw.saturating_sub(24),
                            row_h.saturating_sub(2),
                            [68, 58, 38, 200],
                        );
                    }
                    let title = MemoReader::entry_title(entry, mes_table);
                    font.draw_line_styled(
                        rasterizer,
                        &format!("{:02}. {}", idx + 1, title),
                        (px + 24, ry + 4),
                        body_px,
                        if is_sel {
                            Sh2TextColor::GoldSelected
                        } else {
                            Sh2TextColor::White
                        },
                        true,
                    );
                }
            }

            font.draw_line_centered(
                rasterizer,
                "[Up/Down] Select   [Confirm] Read   [Cancel] Back to Inventory",
                (w / 2, py + ph.saturating_sub(small_px + 14)),
                small_px,
                Sh2TextColor::MutedFooter,
                true,
            );
        }
        MemoPhase::Reading => {
            let Some(entry) = reader.selected_entry() else {
                return;
            };
            let title = MemoReader::entry_title(entry, mes_table);
            let pages = MemoReader::entry_pages(entry, mes_table);
            let page_idx = reader.current_page.min(pages.len().saturating_sub(1));
            let page_text = &pages[page_idx];

            // Semi-transparent reading frame over the inspection still
            let px = (52.0 * sx).round() as usize;
            let py = (52.0 * sy).round() as usize;
            let pw = (536.0 * sx).round() as usize;
            let ph = (376.0 * sy).round() as usize;
            rasterizer.fill_rect(px, py, pw, ph, [6, 8, 10, 195]);
            rasterizer.draw_rect_outline(px, py, pw, ph, 2, [175, 160, 125, 220]);

            font.draw_line_centered(
                rasterizer,
                title,
                (w / 2, py + 16),
                title_px,
                Sh2TextColor::GoldSelected,
                true,
            );

            let line_y0 = py + 58;
            let line_step = body_px + 8;
            for (i, line) in page_text.lines().take(12).enumerate() {
                font.draw_line_styled(
                    rasterizer,
                    line,
                    (px + 24, line_y0 + i * line_step),
                    body_px,
                    Sh2TextColor::White,
                    true,
                );
            }

            let footer = format!(
                "Page {}/{}   [Left/Right or Confirm] Turn Page   [Cancel] Back to List",
                page_idx + 1,
                pages.len()
            );
            font.draw_line_centered(
                rasterizer,
                &footer,
                (w / 2, py + ph.saturating_sub(small_px + 14)),
                small_px,
                Sh2TextColor::MutedFooter,
                true,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sh2_assets::formats::mes::DialogueMessage;

    #[test]
    fn test_memo_catalog_and_multipage_pagination() {
        let mut flags = EventFlags::new();
        assert!(!memo_command_check(&flags));

        // Collect Mary's Letter (flag 44) and Clock Riddle (flag 74)
        flags.set(44, true);
        flags.set(74, true);
        assert!(memo_command_check(&flags));

        let mut reader = MemoReader::new();
        reader.open(&flags);
        assert_eq!(reader.entries.len(), 2);
        assert_eq!(reader.entries[0].flag_id, 44);
        assert_eq!(reader.entries[1].flag_id, 74);

        // Build mock multi-page `.mes` table for Mary's Letter (`body_msg_idx = 49`)
        let mut msgs = Vec::new();
        for id in 0..=55 {
            msgs.push(DialogueMessage {
                id,
                pages: if id == 49 {
                    vec![
                        "In my restless dreams,\nI see that town.".into(),
                        "Silent Hill.".into(),
                    ]
                } else {
                    vec![format!("Msg {id}")]
                },
                text: format!("Msg {id}"),
            });
        }
        let table = MessageTable { messages: msgs };

        // Confirm enters Reading phase on page 0
        let (cue, action) = reader.handle_nav(MenuNavAction::Confirm, Some(&table));
        assert_eq!(cue, MenuSoundCue::Confirm);
        assert_eq!(action, MemoAction::BackdropChanged("pic/etc/p_letterm.tex"));
        assert_eq!(reader.phase, MemoPhase::Reading);
        assert_eq!(reader.current_page, 0);

        // Next page -> page 1
        reader.handle_nav(MenuNavAction::Right, Some(&table));
        assert_eq!(reader.current_page, 1);

        // Confirm on last page returns to List
        reader.handle_nav(MenuNavAction::Confirm, Some(&table));
        assert_eq!(reader.phase, MemoPhase::List);
    }

    #[test]
    fn test_retail_memo_mes_and_map_tex_integration() {
        let resolver = sh2_assets::AssetPathResolver::resolve();
        let mes_path = resolver.data_dir.join("etc/message/memo_msg_e.mes");
        if !mes_path.exists() {
            return;
        }
        let bytes = std::fs::read(&mes_path).expect("read retail memo_msg_e.mes");
        let table = sh2_assets::formats::mes::parse_mes(&bytes).expect("parse memo_msg_e.mes");
        assert!(table.messages.len() >= 90);
        // Mary's Letter (`MEMO_CATALOG[8]`, flag 44, body_msg_idx 49)
        let pages = MemoReader::entry_pages(&MEMO_CATALOG[8], Some(&table));
        assert!(!pages.is_empty());

        let loader =
            sh2_assets::TextureLoader::new(&resolver.data_dir, resolver.sh2e_dir.as_deref());
        let town_map = loader
            .load_texture(crate::map::MAP_SPECS[0].base_tex_path)
            .expect("load retail outmap.tex");
        assert!(town_map.width > 0 && town_map.height > 0);
        let baldwin_map = loader
            .load_texture(crate::map::MAP_SPECS[35].base_tex_path)
            .expect("load retail mapboldwin.tex");
        assert!(baldwin_map.width > 0 && baldwin_map.height > 0);
    }
}
