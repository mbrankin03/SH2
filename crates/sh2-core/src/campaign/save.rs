//! Persistent campaign save-game snapshot for Pause Menu Save/Load (`sh2pc.exe` `FUN_00407300`
//! / `mcSaveMenu` `0x0020ba00`) and Title Screen `LOAD` / `CONTINUE` availability checks
//! (`titleCreateMainMenu` `0x001c1a90` / `sh2pc.exe` `0x00495520`).

use crate::config::{ActionDifficulty, RiddleDifficulty};
use crate::scenario::ScenarioKind;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// Serializable campaign save snapshot persisted to disk on `PauseAction::Save` and restored
/// on `PauseAction::Load` or Title `CONTINUE`.
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
}

impl CampaignSaveSnapshot {
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

    /// Returns `true` if a readable save snapshot file exists at `path`.
    #[must_use]
    pub fn exists_at<P: AsRef<Path>>(path: P) -> bool {
        path.as_ref().is_file()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_campaign_save_snapshot_roundtrip() {
        let snap = CampaignSaveSnapshot {
            scenario: ScenarioKind::LetterFromSilentHeaven,
            stage_id: 2,
            room_id: "er01".to_string(),
            player_position: [1200.0, 0.0, -3400.0],
            player_yaw: 1.57,
            action_difficulty: ActionDifficulty::Beginner,
            riddle_difficulty: RiddleDifficulty::Hard,
            play_time_secs: 425.5,
            save_count: 3,
        };
        let dir = std::env::temp_dir().join("sh2_save_snapshot_test");
        let path = dir.join("save_slot0.json");
        let _ = fs::remove_file(&path);

        assert!(!CampaignSaveSnapshot::exists_at(&path));
        snap.save_to_file(&path).expect("save should succeed");
        assert!(CampaignSaveSnapshot::exists_at(&path));

        let loaded = CampaignSaveSnapshot::load_from_file(&path).expect("load should succeed");
        assert_eq!(loaded, snap);

        let _ = fs::remove_file(&path);
        let _ = fs::remove_dir(&dir);
    }
}
