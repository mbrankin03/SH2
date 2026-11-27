//! State machine for `mcSaveMenu` (`0x00242c70`), `mcLoadMenu` (`0x00243e20`), and `mcSelectData` (`0x00244a50`).

use crate::menu::MenuNavAction;
use sh2_core::campaign::{
    SaveSlotEntry, SaveSlotKind, MAX_DISPLAY_PLAY_TIME_SECS, PS2_MAX_FILES_PER_CARD,
    PS2_MEMORY_CARD_COUNT,
};
use sh2_core::scenario::ScenarioKind;

/// Number of visible save rows in the PS2 Save/Load window (`mcDrawSlot` `0x00245e30`: `iVar8 < 5`).
pub const MC_VISIBLE_ROWS: usize = 5;

/// Whether the screen is opened in Save (`mcSaveMenu`) or Load (`mcLoadMenu`) mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveLoadMode {
    /// Saving at a Red Square or Pause Menu (`mcSaveMenu` `0x00242c70`).
    Save,
    /// Loading from Title Screen or Pause Menu (`mcLoadMenu` `0x00243e20`).
    Load,
}

/// Active step inside `mcSaveMenu` / `mcLoadMenu`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SaveLoadStep {
    #[default]
    SelectSlot,
    ConfirmOverwrite,
    ConfirmDelete,
    SaveDone,
    DeleteDone,
    LoadDone,
    LoadError,
}

/// Yes/No confirmation cursor (`mcTellYesNo` `0x00244940`: `1 = Yes`, `2 = No`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum YesNoChoice {
    Yes = 1,
    #[default]
    No = 2,
}

/// Right-column info display mode cycled by Square / Info button (`mcDrawAddInfo` `0x00246910`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SaveAddInfoMode {
    /// `_mcw + 0x411d == 0`: Play time (`"%4d:%02d"`).
    #[default]
    PlayTime = 0,
    /// `_mcw + 0x411d == 1`: Secondary timer (`"%4d:%02d"` in yellow-green).
    SecondaryTime = 1,
    /// `_mcw + 0x411d == 2`: 7-bit Endings Cleared icons (`mcDrawAddInfo` `0x00246910`).
    EndingsCleared = 2,
}

impl SaveAddInfoMode {
    /// Cycles to the next info mode (`0 -> 1 -> 2 -> 0`, `mcSelectData` `0x00244a50` line 1017).
    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::PlayTime => Self::SecondaryTime,
            Self::SecondaryTime => Self::EndingsCleared,
            Self::EndingsCleared => Self::PlayTime,
        }
    }
}

/// Native-upgrade feature flags passed from `GameOptions` (`AGENTS.md` Rule 3.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SaveLoadConfig {
    /// `false` = strict 1:1 PS2 2 cards × 15 files; `true` = unlimited native slots + export.
    pub unlimited_slots: bool,
    /// `false` = manual slots only; `true` = show Quicksave / Autosave slots.
    pub quicksave_autosave: bool,
    /// `false` = 1:1 PS2 `savebg` + big font; `true` = live 3D thumbnail & Ending Tracker.
    pub save_thumbnails: bool,
}

/// Action emitted by [`SaveLoadScreen`] for `sh2-app` to execute on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SaveLoadAction {
    None,
    Close,
    WriteSlot {
        card_index: usize,
        file_index: usize,
        kind: SaveSlotKind,
    },
    LoadSlot {
        card_index: usize,
        file_index: usize,
        kind: SaveSlotKind,
    },
    DeleteSlot {
        card_index: usize,
        file_index: usize,
    },
    ExportJson {
        card_index: usize,
        file_index: usize,
    },
    ExportBinary {
        card_index: usize,
        file_index: usize,
    },
}

/// Interactive state for the Save/Load Screen (`MC/mc_menu.c`).
#[derive(Debug, Clone)]
pub struct SaveLoadScreen {
    pub mode: SaveLoadMode,
    pub step: SaveLoadStep,
    pub active_card: usize,
    pub cursor_row: usize,
    pub scroll_top: usize,
    pub add_info_mode: SaveAddInfoMode,
    pub yes_no: YesNoChoice,
    pub scenario: ScenarioKind,
    pub config: SaveLoadConfig,
    /// Blink phase `0.0..1.0` (`_mcw + 0x4128` in `mcMenuControl` `0x00244710`).
    pub blink_phase: f32,
    /// Smooth vertical scroll offset `-1.0..1.0` (`_mcw + 0x4124` in `mcMenuControl` `0x00244710`).
    pub scroll_anim: f32,
    /// Status timer in 60 Hz frames (`_mcw + 0x411e = 0x5a = 90` in `mcSaveMenu` `0x00242c70`).
    pub status_timer_frames: i32,
    /// Status banner message for native export or save/load feedback.
    pub status_message: Option<String>,
    /// Loaded slot entries for the active card/view.
    pub slots: Vec<SaveSlotEntry>,
}

impl SaveLoadScreen {
    /// Creates a new [`SaveLoadScreen`] in `mode`.
    #[must_use]
    pub fn new(mode: SaveLoadMode, scenario: ScenarioKind, config: SaveLoadConfig) -> Self {
        Self {
            mode,
            step: SaveLoadStep::SelectSlot,
            active_card: 0,
            cursor_row: 0,
            scroll_top: 0,
            add_info_mode: SaveAddInfoMode::PlayTime,
            yes_no: YesNoChoice::No,
            scenario,
            config,
            blink_phase: 0.0,
            scroll_anim: 0.0,
            status_timer_frames: 0,
            status_message: None,
            slots: Vec::new(),
        }
    }

    /// Updates the slot list and clamps cursor/scroll bounds.
    pub fn set_slots(&mut self, slots: Vec<SaveSlotEntry>) {
        self.slots = slots;
        let max_rows = self.total_selectable_rows();
        if max_rows == 0 {
            self.cursor_row = 0;
            self.scroll_top = 0;
        } else {
            self.cursor_row = self.cursor_row.min(max_rows - 1);
            let max_top = max_rows.saturating_sub(MC_VISIBLE_ROWS);
            self.scroll_top = self.scroll_top.min(max_top);
        }
    }

    /// Zero-allocation iterator over entries visible on the currently selected card (`active_card`).
    pub fn active_card_slots_iter(&self) -> impl Iterator<Item = &SaveSlotEntry> {
        let unlimited = self.config.unlimited_slots;
        let active_card = self.active_card;
        self.slots
            .iter()
            .filter(move |s| unlimited || s.card_index == active_card)
    }

    /// Returns the `row`-th entry on the currently selected card without allocating a `Vec`.
    #[must_use]
    pub fn active_card_slot(&self, row: usize) -> Option<&SaveSlotEntry> {
        if self.config.unlimited_slots {
            self.slots.get(row)
        } else {
            self.active_card_slots_iter().nth(row)
        }
    }

    /// Number of existing save entries on the currently selected card without allocating a `Vec`.
    #[must_use]
    pub fn active_card_slot_count(&self) -> usize {
        if self.config.unlimited_slots {
            self.slots.len()
        } else {
            self.active_card_slots_iter().count()
        }
    }

    /// Entries visible on the currently selected card (`active_card`).
    #[must_use]
    pub fn active_card_slots(&self) -> Vec<&SaveSlotEntry> {
        self.active_card_slots_iter().collect()
    }

    /// Total selectable rows (existing files + 1 `"New Save"` row in `Save` mode when under cap).
    #[must_use]
    pub fn total_selectable_rows(&self) -> usize {
        let count = self.active_card_slot_count();
        if self.mode == SaveLoadMode::Save {
            if self.config.unlimited_slots || count < PS2_MAX_FILES_PER_CARD {
                count + 1
            } else {
                count
            }
        } else {
            count
        }
    }

    /// Advances blink and scroll animations (`mcMenuControl` `0x00244710`).
    pub fn tick(&mut self, dt_sec: f32) {
        if self.scroll_anim < 0.0 {
            self.scroll_anim = (self.scroll_anim + dt_sec * 5.0).min(0.0);
        } else if self.scroll_anim > 0.0 {
            self.scroll_anim = (self.scroll_anim - dt_sec * 5.0).max(0.0);
        }
        self.blink_phase = (self.blink_phase + dt_sec) % 1.0;

        if self.status_timer_frames > 0 {
            self.status_timer_frames -= (dt_sec * 60.0).round().max(1.0) as i32;
            if self.status_timer_frames <= 0 {
                self.status_timer_frames = 0;
                if matches!(
                    self.step,
                    SaveLoadStep::SaveDone | SaveLoadStep::DeleteDone | SaveLoadStep::LoadError
                ) {
                    self.step = SaveLoadStep::SelectSlot;
                }
            }
        }
    }

    /// Switches memory card slot (`Slot 1` <-> `Slot 2`, `mcSelectData` `0x00244a50`).
    pub fn switch_card(&mut self, card: usize) {
        if !self.config.unlimited_slots && card < PS2_MEMORY_CARD_COUNT && card != self.active_card
        {
            self.active_card = card;
            self.cursor_row = 0;
            self.scroll_top = 0;
            self.scroll_anim = 0.0;
        }
    }

    /// Cycles `SaveAddInfoMode` (`mcSelectData` `0x00244a50` line 1011).
    pub fn cycle_add_info(&mut self) {
        self.add_info_mode = self.add_info_mode.next();
    }

    /// Requests deletion of the selected existing slot (`mcSaveMenu` step 9).
    pub fn request_delete(&mut self) {
        if self.step == SaveLoadStep::SelectSlot && self.cursor_row < self.active_card_slots().len()
        {
            self.yes_no = YesNoChoice::No;
            self.step = SaveLoadStep::ConfirmDelete;
        }
    }

    /// Requests JSON or binary export for the selected slot (gated by `unlimited_slots`).
    pub fn request_export(&mut self, binary: bool) -> SaveLoadAction {
        if !self.config.unlimited_slots {
            return SaveLoadAction::None;
        }
        if let Some(entry) = self.active_card_slots().get(self.cursor_row) {
            if binary {
                SaveLoadAction::ExportBinary {
                    card_index: entry.card_index,
                    file_index: entry.file_index,
                }
            } else {
                SaveLoadAction::ExportJson {
                    card_index: entry.card_index,
                    file_index: entry.file_index,
                }
            }
        } else {
            SaveLoadAction::None
        }
    }

    /// Processes directional / confirm / cancel navigation (`mcSelectData` & `mcTellYesNo`).
    pub fn handle_nav(&mut self, nav: MenuNavAction) -> SaveLoadAction {
        match self.step {
            SaveLoadStep::ConfirmOverwrite | SaveLoadStep::ConfirmDelete => match nav {
                MenuNavAction::Left | MenuNavAction::Right => {
                    self.yes_no = match self.yes_no {
                        YesNoChoice::Yes => YesNoChoice::No,
                        YesNoChoice::No => YesNoChoice::Yes,
                    };
                    SaveLoadAction::None
                }
                MenuNavAction::Cancel => {
                    self.step = SaveLoadStep::SelectSlot;
                    SaveLoadAction::None
                }
                MenuNavAction::Confirm => {
                    if self.yes_no == YesNoChoice::No {
                        self.step = SaveLoadStep::SelectSlot;
                        return SaveLoadAction::None;
                    }
                    let is_delete = self.step == SaveLoadStep::ConfirmDelete;
                    let card_slots = self.active_card_slots();
                    if let Some(entry) = card_slots.get(self.cursor_row).copied().cloned() {
                        if is_delete {
                            self.step = SaveLoadStep::DeleteDone;
                            self.status_timer_frames = 90;
                            SaveLoadAction::DeleteSlot {
                                card_index: entry.card_index,
                                file_index: entry.file_index,
                            }
                        } else {
                            self.step = SaveLoadStep::SaveDone;
                            self.status_timer_frames = 90;
                            SaveLoadAction::WriteSlot {
                                card_index: entry.card_index,
                                file_index: entry.file_index,
                                kind: entry.kind,
                            }
                        }
                    } else {
                        self.step = SaveLoadStep::SelectSlot;
                        SaveLoadAction::None
                    }
                }
                _ => SaveLoadAction::None,
            },
            SaveLoadStep::SaveDone | SaveLoadStep::DeleteDone | SaveLoadStep::LoadError => {
                if matches!(nav, MenuNavAction::Confirm | MenuNavAction::Cancel) {
                    self.step = SaveLoadStep::SelectSlot;
                    self.status_timer_frames = 0;
                }
                SaveLoadAction::None
            }
            SaveLoadStep::LoadDone => SaveLoadAction::Close,
            SaveLoadStep::SelectSlot => {
                let total = self.total_selectable_rows();
                match nav {
                    MenuNavAction::Up => {
                        if total > 1 {
                            if self.cursor_row == 0 {
                                self.cursor_row = total - 1;
                                self.scroll_top = total.saturating_sub(MC_VISIBLE_ROWS);
                                self.scroll_anim = -1.0;
                            } else {
                                self.cursor_row -= 1;
                                if self.cursor_row < self.scroll_top {
                                    self.scroll_top = self.cursor_row;
                                }
                                self.scroll_anim = 1.0;
                            }
                        }
                        SaveLoadAction::None
                    }
                    MenuNavAction::Down => {
                        if total > 1 {
                            if self.cursor_row + 1 >= total {
                                self.cursor_row = 0;
                                self.scroll_top = 0;
                                self.scroll_anim = 1.0;
                            } else {
                                self.cursor_row += 1;
                                if self.cursor_row >= self.scroll_top + MC_VISIBLE_ROWS {
                                    self.scroll_top = self.cursor_row + 1 - MC_VISIBLE_ROWS;
                                }
                                self.scroll_anim = -1.0;
                            }
                        }
                        SaveLoadAction::None
                    }
                    MenuNavAction::Left => {
                        self.switch_card(0);
                        SaveLoadAction::None
                    }
                    MenuNavAction::Right => {
                        self.switch_card(1);
                        SaveLoadAction::None
                    }
                    MenuNavAction::Cancel => SaveLoadAction::Close,
                    MenuNavAction::Confirm => self.confirm_selected_slot(),
                }
            }
        }
    }

    fn confirm_selected_slot(&mut self) -> SaveLoadAction {
        let card_slots = self.active_card_slots();
        if let Some(entry) = card_slots.get(self.cursor_row).copied().cloned() {
            if entry.is_corrupted {
                self.step = SaveLoadStep::LoadError;
                self.status_timer_frames = 90;
                return SaveLoadAction::None;
            }
            match self.mode {
                SaveLoadMode::Save => {
                    self.yes_no = YesNoChoice::No;
                    self.step = SaveLoadStep::ConfirmOverwrite;
                    SaveLoadAction::None
                }
                SaveLoadMode::Load => {
                    self.step = SaveLoadStep::LoadDone;
                    SaveLoadAction::LoadSlot {
                        card_index: entry.card_index,
                        file_index: entry.file_index,
                        kind: entry.kind,
                    }
                }
            }
        } else if self.mode == SaveLoadMode::Save {
            let next_file = self.next_free_file_index();
            self.step = SaveLoadStep::SaveDone;
            self.status_timer_frames = 90;
            SaveLoadAction::WriteSlot {
                card_index: self.active_card,
                file_index: next_file,
                kind: SaveSlotKind::Manual,
            }
        } else {
            SaveLoadAction::None
        }
    }

    fn next_free_file_index(&self) -> usize {
        let used: Vec<usize> = self
            .active_card_slots()
            .iter()
            .filter(|s| s.kind == SaveSlotKind::Manual)
            .map(|s| s.file_index)
            .collect();
        (0..100).find(|i| !used.contains(i)).unwrap_or(used.len())
    }
}

/// Computes `mcGetBlinkAlpha` (`SLUS_202.28` `0x00245440`):
/// triangle wave `0.0..0.5 -> 0..32`, `0.5..1.0 -> 32..0`.
#[must_use]
pub fn mc_get_blink_alpha(blink_phase: f32) -> i32 {
    let mut f = blink_phase.clamp(0.0, 1.0);
    if f > 0.5 {
        f = 1.0 - f;
    }
    ((f / 0.5) * 32.0) as i32
}

/// Formats play time in `mcDrawAddInfo` (`0x00246910`): `"%4d:%02d"` clamped to `599999` seconds.
#[must_use]
pub fn format_mc_time(secs: f64) -> String {
    let total = (secs.max(0.0) as u32).min(MAX_DISPLAY_PLAY_TIME_SECS);
    format!("{:4}:{:02}", total / 60, total % 60)
}

/// Maps `save_point_name_id` (`SetSavePointName` `0x002479d0`) to an authentic location label.
#[must_use]
pub fn save_point_location_name(id: u16, room_id: &str) -> &'static str {
    match id {
        1 => "Observation Deck",
        2 => "Wood Side Apartments 1F",
        3 => "Wood Side Apartments 2F",
        4 => "Blue Creek Apartments 1F",
        5 => "Pete's Bowl-O-Rama",
        6 => "Brookhaven Hospital 1F",
        7 => "Brookhaven Hospital 2F",
        8 => "Brookhaven Hospital 3F",
        9 => "Historical Society",
        10 => "Toluca Prison",
        11 => "Labyrinth",
        12 => "Lakeview Hotel 1F",
        13 => "Lakeview Hotel 2F",
        14 => "Lakeview Hotel 3F",
        _ => match room_id {
            "er01" => "Restroom",
            "ob01" => "Observation Deck",
            _ => "Silent Hill",
        },
    }
}
