//! Save slot repository and Clear Data NG+ profile persistence (`MC/mc_menu.c` & `MC/savedata.c`).

use super::{
    validate_save_data, CampaignSaveSnapshot, PS2_MAX_FILES_PER_CARD, PS2_MEMORY_CARD_COUNT,
};
use crate::config::{ActionDifficulty, RiddleDifficulty};
use crate::events::ClearData;
use crate::scenario::ScenarioKind;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Filename for persistent Clear Data NG+ profile (`prog_clear_get` / `prog_clear_set`).
pub const CLEAR_DATA_FILENAME: &str = "clear_data.json";
/// Filename for native-upgrade Quicksave slot.
pub const QUICKSAVE_FILENAME: &str = "quicksave.json";
/// Filename for native-upgrade Autosave slot.
pub const AUTOSAVE_FILENAME: &str = "autosave.json";

/// Slot classification for save/load UI presentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SaveSlotKind {
    /// Standard PS2 memory card file (`0..15`) or numbered native save slot.
    #[default]
    Manual,
    /// Native-upgrade Quicksave slot (`enable_quicksave_autosave`).
    Quicksave,
    /// Native-upgrade Autosave slot (`enable_quicksave_autosave`).
    Autosave,
}

/// Persistent Clear Data NG+ profile carried across playthroughs (`GameSavePreviousTotalRank`
/// `0x00290bf0` & `GameSaveSprayPower` `0x00290c20`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ClearDataProfile {
    /// Core Team Silent `ClearData` bitmasks (`cleared_levels`, `endings_seen`, `clear_count`).
    pub clear_data: ClearData,
    /// Previous total star rank (`0..=100`, `DAT_0122b316`).
    pub previous_total_rank: u8,
    /// Hyper Spray power tier (`-1` Purple, `0` White, `1` Yellow, `2` Green, `DAT_0122b319`).
    pub spray_pow: i8,
}

impl ClearDataProfile {
    /// Updates profile upon completing an ending and viewing Results (`ResultMain` `0x00293e40`).
    pub fn record_completion(
        &mut self,
        ending_bit: u8,
        riddle: RiddleDifficulty,
        action: ActionDifficulty,
        total_rank: u8,
        spray_pow: i8,
    ) {
        self.clear_data.record_ending(ending_bit, riddle, action);
        self.previous_total_rank = total_rank;
        self.spray_pow = spray_pow;
    }

    /// Loads `clear_data.json` from `dir`, returning `Default` if absent or unreadable.
    pub fn load_from_dir<P: AsRef<Path>>(dir: P) -> Self {
        let path = dir.as_ref().join(CLEAR_DATA_FILENAME);
        fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    /// Writes `clear_data.json` atomically to `dir`.
    pub fn save_to_dir<P: AsRef<Path>>(&self, dir: P) -> std::io::Result<()> {
        let dir = dir.as_ref();
        fs::create_dir_all(dir)?;
        let path = dir.join(CLEAR_DATA_FILENAME);
        let tmp = dir.join("clear_data.json.tmp");
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        fs::write(&tmp, json)?;
        fs::rename(&tmp, path)
    }
}

/// Summary metadata + payload for a single save slot in the Save/Load menu (`mcDrawSlot` `0x00245e30`).
#[derive(Debug, Clone, PartialEq)]
pub struct SaveSlotEntry {
    /// Memory card index (`0` = Slot 1, `1` = Slot 2).
    pub card_index: usize,
    /// File index on the card (`0..15` in PS2 mode, `0..` in unlimited native mode).
    pub file_index: usize,
    /// Slot kind (`Manual`, `Quicksave`, `Autosave`).
    pub kind: SaveSlotKind,
    /// `true` if `CheckSaveData` (`0x00247bb0`) failed (bit `0x80` corrupted flag in `mcDrawSlot`).
    pub is_corrupted: bool,
    /// Underlying snapshot (if valid).
    pub snapshot: CampaignSaveSnapshot,
}

impl SaveSlotEntry {
    /// Ending bitmask (`0..=6`) from `ClearData::endings_seen` (`mcDrawAddInfo` `0x00246910`).
    #[must_use]
    pub const fn endings_seen_mask(&self) -> u8 {
        self.snapshot.clear_data.endings_seen
    }

    /// Scenario (`DAT_0122b396`: Main vs Born from a Wish).
    #[must_use]
    pub const fn scenario(&self) -> ScenarioKind {
        self.snapshot.scenario
    }
}

/// Directory-backed repository for PS2-style 2×15 save slots and native-upgrade unlimited/quick/auto saves.
#[derive(Debug, Clone)]
pub struct SaveSlotRepository {
    pub root_dir: PathBuf,
}

impl SaveSlotRepository {
    /// Creates a repository rooted at `root_dir`.
    pub fn new<P: AsRef<Path>>(root_dir: P) -> Self {
        Self {
            root_dir: root_dir.as_ref().to_path_buf(),
        }
    }

    /// Canonical path for a manual PS2/native slot `(card_index, file_index)`.
    #[must_use]
    pub fn slot_path(&self, card_index: usize, file_index: usize) -> PathBuf {
        if card_index == 0 && file_index == 0 {
            let legacy = self.root_dir.join("save_slot0.json");
            if legacy.is_file() {
                return legacy;
            }
        }
        self.root_dir
            .join(format!("save_card{card_index}_file{file_index:02}.json"))
    }

    /// Scans `root_dir` for all save slots. When `unlimited` is `false`, restricts to
    /// `PS2_MEMORY_CARD_COUNT` (`2`) × `PS2_MAX_FILES_PER_CARD` (`15`) and omits quick/auto slots.
    pub fn scan_slots(&self, unlimited: bool, include_quick_auto: bool) -> Vec<SaveSlotEntry> {
        let mut entries = Vec::new();
        let max_cards = if unlimited { 1 } else { PS2_MEMORY_CARD_COUNT };
        let max_files = if unlimited {
            64
        } else {
            PS2_MAX_FILES_PER_CARD
        };

        if include_quick_auto {
            for (kind, fname, idx) in [
                (SaveSlotKind::Autosave, AUTOSAVE_FILENAME, 9000usize),
                (SaveSlotKind::Quicksave, QUICKSAVE_FILENAME, 9001usize),
            ] {
                let path = self.root_dir.join(fname);
                if path.is_file() {
                    if let Ok(snap) = CampaignSaveSnapshot::load_from_file(&path) {
                        let corrupted = validate_save_data(&snap).is_err();
                        entries.push(SaveSlotEntry {
                            card_index: 0,
                            file_index: idx,
                            kind,
                            is_corrupted: corrupted,
                            snapshot: snap,
                        });
                    }
                }
            }
        }

        for card in 0..max_cards {
            for file in 0..max_files {
                let path = self.slot_path(card, file);
                if !path.is_file() {
                    continue;
                }
                if let Ok(snap) = CampaignSaveSnapshot::load_from_file(&path) {
                    let corrupted = validate_save_data(&snap).is_err();
                    entries.push(SaveSlotEntry {
                        card_index: card,
                        file_index: file,
                        kind: SaveSlotKind::Manual,
                        is_corrupted: corrupted,
                        snapshot: snap,
                    });
                }
            }
        }
        entries
    }

    /// Writes a save snapshot to `(card_index, file_index)` or quick/auto slot.
    pub fn write_slot(
        &self,
        card_index: usize,
        file_index: usize,
        kind: SaveSlotKind,
        snapshot: &CampaignSaveSnapshot,
    ) -> std::io::Result<PathBuf> {
        let path = match kind {
            SaveSlotKind::Manual => self.slot_path(card_index, file_index),
            SaveSlotKind::Quicksave => self.root_dir.join(QUICKSAVE_FILENAME),
            SaveSlotKind::Autosave => self.root_dir.join(AUTOSAVE_FILENAME),
        };
        snapshot.save_to_file(&path)?;
        Ok(path)
    }

    /// Deletes a manual save slot (`mcDeleteData` in `mcSaveMenu` `0x00242c70` step 10).
    pub fn delete_slot(&self, card_index: usize, file_index: usize) -> std::io::Result<()> {
        let path = self.slot_path(card_index, file_index);
        if path.is_file() {
            fs::remove_file(path)?;
        }
        Ok(())
    }
}
