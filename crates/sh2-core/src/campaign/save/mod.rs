//! Persistent campaign save-game snapshot, PS2 memory-card slot metadata, `CheckSaveData`
//! validation, and Clear Data NG+ persistence.
//!
//! # Ground Truth References
//! - **PS2 Prototype (`SLUS_202.28`):**
//!   - `MC/savedata.c`: `SetSavePointName` (`0x002479d0`), `SetSaveData` (`0x002479e0`, `_SaveDataAll = 4`),
//!     `ExtGameData` (`0x00247ae0`), `CheckSaveData` (`0x00247bb0`).
//!   - `MC/mc_menu.c`: `mcSaveMenu` (`0x00242c70`, 15 files per card `0xf`), `mcDrawAddInfo`
//!     (`0x00246910`, `599999` s max display time, 7-bit ending stars), `mcDrawStatus` (`0x00246c30`).
//! - **Retail PC (`sh2pc.exe`):**
//!   - `FUN_00407300` (save-anywhere pause hook), `0x0048d330` (`prog_clear_record`),
//!     `0x0048aed0` (`FlagInit` Clear Data NG+ unlocks & Extra Riddle promotion).

pub mod repository;

pub use repository::{
    ClearDataProfile, SaveSlotEntry, SaveSlotKind, SaveSlotRepository, AUTOSAVE_FILENAME,
    CLEAR_DATA_FILENAME, QUICKSAVE_FILENAME,
};

use crate::config::{ActionDifficulty, RiddleDifficulty};
use crate::events::ClearData;
use crate::scenario::ScenarioKind;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// Save format version written by `SetSaveData` (`SLUS_202.28` `0x002479e0`, `_SaveDataAll = 4`).
pub const SAVE_FORMAT_VERSION: u32 = 4;
/// Legacy save format version also accepted by `CheckSaveData` (`SLUS_202.28` `0x00247bb0`, `_SaveDataAll == 3`).
pub const LEGACY_SAVE_FORMAT_VERSION: u32 = 3;
/// Number of PS2 memory card slots (`Slot 1`, `Slot 2`, `mcSelectData` `0x00244a50`).
pub const PS2_MEMORY_CARD_COUNT: usize = 2;
/// Maximum save files per PS2 memory card (`0xf = 15`, `mcSaveMenu` `0x00242c70`).
pub const PS2_MAX_FILES_PER_CARD: usize = 15;
/// Maximum save-time display value in seconds (`599999` s = `9999:59`, `mcDrawAddInfo` `0x00246910`).
pub const MAX_DISPLAY_PLAY_TIME_SECS: u32 = 599_999;
/// Maximum retail stage ID (`58` in retail PC Director's Cut; `0x34 = 52` in `SLUS_202.28` `CheckSaveData`).
pub const MAX_RETAIL_STAGE_ID: u16 = 58;
/// Magic header for portable binary save export (`"SH2S"`).
pub const BINARY_SAVE_MAGIC: [u8; 4] = *b"SH2S";

/// Validation error returned by [`validate_save_data`] (`CheckSaveData` `0x00247bb0`).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SaveValidationError {
    #[error("unsupported save version {0} (expected 3 or 4)")]
    UnsupportedVersion(u32),
    #[error("invalid stage id {0} (expected 1..={MAX_RETAIL_STAGE_ID})")]
    InvalidStageId(u16),
    #[error("negative play time {0}")]
    NegativePlayTime(String),
    #[error("invalid health values")]
    InvalidHealth,
    #[error("invalid binary save header")]
    InvalidBinaryHeader,
}

/// Serializable campaign save snapshot persisted to disk on Save and restored on Load / Title `CONTINUE`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CampaignSaveSnapshot {
    /// Active scenario (`LetterFromSilentHeaven` or `BornFromAWish`).
    pub scenario: ScenarioKind,
    /// 1-based retail stage ID (`1..=58`, e.g. `2` = `stg_toilet`).
    pub stage_id: u16,
    /// Active room identifier (e.g. `"er01"`, `"ob01"`, `"ap34"`).
    pub room_id: String,
    /// Player world position in PS2 millimeters `[x, y, z]`.
    pub player_position: [f32; 3],
    /// Player facing yaw in radians.
    pub player_yaw: f32,
    /// Combat action difficulty.
    pub action_difficulty: ActionDifficulty,
    /// Puzzle riddle difficulty.
    pub riddle_difficulty: RiddleDifficulty,
    /// Elapsed campaign play time in seconds (`GameTimerCountUp` `0x002902d0`).
    pub play_time_secs: f64,
    /// Cumulative number of saves performed in this playthrough.
    pub save_count: u32,
    /// Save format version (`SAVE_FORMAT_VERSION = 4`, `SetSaveData` `0x002479e0`).
    #[serde(default = "default_save_version")]
    pub save_version: u32,
    /// Save-point location message ID (`SetSavePointName` `0x002479d0`, `m_card_msg_e.mes`).
    #[serde(default)]
    pub save_point_name_id: u16,
    /// Secondary timer (`_mcw + 0x0c` in `mcDrawAddInfo` `0x00246910`).
    #[serde(default)]
    pub secondary_timer_secs: f64,
    /// Player current HP at save time (`CheckSaveData` `0x00247bb0`).
    #[serde(default = "default_hp")]
    pub player_hp: f32,
    /// Player max HP at save time.
    #[serde(default = "default_hp")]
    pub player_max_hp: f32,
    /// Clear Data carried into this playthrough (`ClearData`, `FlagInit` `0x001a6e80`).
    #[serde(default)]
    pub clear_data: ClearData,
    /// Previous total rank (`GameSavePreviousTotalRank` `0x00290bf0`).
    #[serde(default)]
    pub previous_total_rank: u8,
    /// Hyper Spray power tier (`GameSaveSprayPower` `0x00290c20`: `-1` Purple, `0` White, `1` Yellow, `2` Green).
    #[serde(default)]
    pub spray_pow: i8,
    /// Optional packed event flags for full state restoration.
    #[serde(default)]
    pub event_flags_words: Vec<u32>,
    /// Optional inventory items `(item_id, count)` for full state restoration.
    #[serde(default)]
    pub inventory_items: Vec<(u8, u16)>,
    /// Optional native-upgrade live 3D thumbnail `(width, height, rgba)`.
    #[serde(default)]
    pub thumbnail_rgba: Option<(u16, u16, Vec<u8>)>,
}

const fn default_save_version() -> u32 {
    SAVE_FORMAT_VERSION
}

const fn default_hp() -> f32 {
    100.0
}

impl CampaignSaveSnapshot {
    /// Validates this snapshot using `CheckSaveData` (`SLUS_202.28` `0x00247bb0`) invariants.
    pub fn validate(&self) -> Result<(), SaveValidationError> {
        validate_save_data(self)
    }

    /// Serializes and writes this save snapshot atomically to `path` via a temp file + rename.
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> std::io::Result<()> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)?;
            }
        }
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        let tmp_path = path.with_extension("json.tmp");
        fs::write(&tmp_path, json)?;
        fs::rename(&tmp_path, path)
    }

    /// Reads and deserializes a [`CampaignSaveSnapshot`] from `path`.
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> std::io::Result<Self> {
        let content = fs::read_to_string(path)?;
        serde_json::from_str(&content)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }

    /// Encodes this snapshot into a portable binary blob (`"SH2S"` magic + version + JSON payload)
    /// for native-upgrade binary save export.
    pub fn to_binary_bytes(&self) -> Result<Vec<u8>, SaveValidationError> {
        let json =
            serde_json::to_vec(self).map_err(|_| SaveValidationError::InvalidBinaryHeader)?;
        let mut out = Vec::with_capacity(12 + json.len());
        out.extend_from_slice(&BINARY_SAVE_MAGIC);
        out.extend_from_slice(&self.save_version.to_le_bytes());
        out.extend_from_slice(&(json.len() as u32).to_le_bytes());
        out.extend_from_slice(&json);
        Ok(out)
    }

    /// Decodes a portable binary blob created by [`Self::to_binary_bytes`].
    pub fn from_binary_bytes(bytes: &[u8]) -> Result<Self, SaveValidationError> {
        if bytes.len() < 12 || bytes[0..4] != BINARY_SAVE_MAGIC {
            return Err(SaveValidationError::InvalidBinaryHeader);
        }
        let len = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]) as usize;
        if bytes.len() < 12 + len {
            return Err(SaveValidationError::InvalidBinaryHeader);
        }
        let snap: Self = serde_json::from_slice(&bytes[12..12 + len])
            .map_err(|_| SaveValidationError::InvalidBinaryHeader)?;
        snap.validate()?;
        Ok(snap)
    }

    /// Returns `true` if a readable save snapshot file exists at `path`.
    #[must_use]
    pub fn exists_at<P: AsRef<Path>>(path: P) -> bool {
        path.as_ref().is_file()
    }
}

/// Validates save data fields against `CheckSaveData` (`SLUS_202.28` `0x00247bb0`).
pub fn validate_save_data(snap: &CampaignSaveSnapshot) -> Result<(), SaveValidationError> {
    if snap.save_version != SAVE_FORMAT_VERSION && snap.save_version != LEGACY_SAVE_FORMAT_VERSION {
        return Err(SaveValidationError::UnsupportedVersion(snap.save_version));
    }
    if snap.stage_id == 0 || snap.stage_id > MAX_RETAIL_STAGE_ID {
        return Err(SaveValidationError::InvalidStageId(snap.stage_id));
    }
    if !snap.play_time_secs.is_finite() || snap.play_time_secs < 0.0 {
        return Err(SaveValidationError::NegativePlayTime(format!(
            "{}",
            snap.play_time_secs
        )));
    }
    if !snap.player_hp.is_finite()
        || !snap.player_max_hp.is_finite()
        || snap.player_hp < 0.0
        || snap.player_max_hp <= 0.0
    {
        return Err(SaveValidationError::InvalidHealth);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_snapshot() -> CampaignSaveSnapshot {
        CampaignSaveSnapshot {
            scenario: ScenarioKind::LetterFromSilentHeaven,
            stage_id: 2,
            room_id: "er01".to_string(),
            player_position: [1200.0, 0.0, -3400.0],
            player_yaw: 1.57,
            action_difficulty: ActionDifficulty::Beginner,
            riddle_difficulty: RiddleDifficulty::Hard,
            play_time_secs: 425.5,
            save_count: 3,
            save_version: SAVE_FORMAT_VERSION,
            save_point_name_id: 4,
            secondary_timer_secs: 0.0,
            player_hp: 100.0,
            player_max_hp: 100.0,
            clear_data: ClearData::default(),
            previous_total_rank: 0,
            spray_pow: 0,
            event_flags_words: Vec::new(),
            inventory_items: Vec::new(),
            thumbnail_rgba: None,
        }
    }

    #[test]
    fn test_campaign_save_snapshot_roundtrip() {
        let snap = sample_snapshot();
        let dir = std::env::temp_dir().join("sh2_save_snapshot_test");
        let path = dir.join("save_slot0.json");
        let _ = fs::remove_file(&path);

        assert!(!CampaignSaveSnapshot::exists_at(&path));
        snap.save_to_file(&path).expect("save should succeed");
        assert!(CampaignSaveSnapshot::exists_at(&path));

        let loaded = CampaignSaveSnapshot::load_from_file(&path).expect("load should succeed");
        assert_eq!(loaded, snap);

        let bin = snap.to_binary_bytes().expect("binary encode");
        let decoded = CampaignSaveSnapshot::from_binary_bytes(&bin).expect("binary decode");
        assert_eq!(decoded, snap);

        let _ = fs::remove_file(&path);
        let _ = fs::remove_dir(&dir);
    }

    #[test]
    fn test_check_save_data_validation() {
        let mut snap = sample_snapshot();
        assert!(snap.validate().is_ok());

        snap.save_version = 99;
        assert_eq!(
            snap.validate(),
            Err(SaveValidationError::UnsupportedVersion(99))
        );

        snap.save_version = SAVE_FORMAT_VERSION;
        snap.stage_id = 0;
        assert_eq!(snap.validate(), Err(SaveValidationError::InvalidStageId(0)));
    }
}
