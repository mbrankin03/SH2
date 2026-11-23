//! BGM stream resolution, ADX/AIX decoding, playback start, and debug HUD telemetry.

use sh2_assets::{decode_adx, decode_aix, DecodedAdx};
use sh2_core::AudioPackSource;
use std::path::{Path, PathBuf};

use crate::bgm_data::find_bgm_file_by_load_id;
use crate::cutscene_bgm::resolve_bgm_stream_path_for_source;
use crate::engine::{AudioEngine, AudioError};

use super::types::BGM_MAX_TRACKS;
use super::BgmManager;

impl BgmManager {
    /// Resolves, decodes, and starts playback of the `.adx` or `.aix` file corresponding to `load_id`.
    pub(super) fn load_and_start_stream(
        &mut self,
        load_id: i32,
        engine: Option<&mut AudioEngine>,
    ) -> Result<(), AudioError> {
        let Some(file_entry) = find_bgm_file_by_load_id(load_id) else {
            return Err(AudioError::AssetError(format!(
                "Unknown BGM load_id: {load_id}"
            )));
        };

        self.loaded_file_id = load_id;
        self.loaded_rel_path = Some(file_entry.path);
        self.loaded_is_adx = file_entry.is_adx;

        let Some(full_path) = resolve_bgm_stream_path_for_source(
            &self.game_root,
            file_entry.path,
            self.audio_pack_source,
        ) else {
            // In headless unit tests without retail assets on disk, still record metadata state cleanly
            self.loaded_full_path = None;
            self.loaded_layer_count = if file_entry.is_adx { 1 } else { BGM_MAX_TRACKS };
            self.loaded_sample_rate = match self.audio_pack_source {
                AudioPackSource::Auto | AudioPackSource::Sh2eeLossless => 48000,
                AudioPackSource::Ps2Original => 24000,
            };
            return Ok(());
        };

        let bytes = std::fs::read(&full_path).map_err(|e| {
            AudioError::AssetError(format!(
                "Failed to read BGM file {}: {e}",
                full_path.display()
            ))
        })?;

        let layers: Vec<DecodedAdx> = if file_entry.is_adx {
            let decoded = decode_adx(&bytes).map_err(|e| {
                AudioError::AssetError(format!("Failed to decode ADX {}: {e}", full_path.display()))
            })?;
            vec![decoded]
        } else {
            decode_aix(&bytes).map_err(|e| {
                AudioError::AssetError(format!("Failed to decode AIX {}: {e}", full_path.display()))
            })?
        };

        self.loaded_full_path = Some(full_path);
        self.loaded_layer_count = layers.len();
        self.loaded_sample_rate = layers.first().map(|l| l.header.sample_rate).unwrap_or(0);

        if let Some(eng) = engine {
            let mut initial_gains = [0.0f32; BGM_MAX_TRACKS];
            for (i, gain) in initial_gains.iter_mut().enumerate() {
                let track_vol = if file_entry.is_adx && i == 0 {
                    1.0
                } else {
                    self.tracks[i].volume
                };
                *gain = track_vol * self.file_master_volume * self.global_bgm_fade;
            }
            // AIX streams in Silent Hill 2 loop seamlessly from end-of-stream back to sample 0
            let loop_whole_stream = !file_entry.is_adx;
            eng.play_bgm_layers(&layers, &initial_gains[..layers.len()], loop_whole_stream)?;
        }

        Ok(())
    }

    /// Formats a concise telemetry summary line for the in-game debug HUD.
    pub fn hud_summary(&self) -> String {
        let file_label = self.loaded_rel_path.unwrap_or("None (Silence)");
        let pack_label = match self.audio_pack_source {
            AudioPackSource::Auto => "Auto (SH2:EE 48kHz)",
            AudioPackSource::Sh2eeLossless => "SH2:EE 48kHz",
            AudioPackSource::Ps2Original => "PS2/PC Orig",
        };
        let sit_label = match self.situation_page {
            0 => "0:Calm",
            1 => "1:Event1",
            2 => "2:LowHP",
            3 => "3:EnemyNear",
            4 => "4:Event4",
            5 => "5:Event5",
            6..=13 => "6..13:MusicBox",
            14 => "14:Mute",
            15 => "15:Event15",
            _ => "Other",
        };
        let vols: Vec<String> = self
            .tracks
            .iter()
            .take(self.loaded_layer_count.clamp(1, BGM_MAX_TRACKS))
            .enumerate()
            .map(|(i, t)| format!("T{}:{:.0}%", i, t.volume * 100.0))
            .collect();
        format!(
            "BGM [{} | {}Hz] Page:{} (Room:0x{:02X}) Sit:{} | {} | [{}]",
            pack_label,
            self.loaded_sample_rate,
            self.current_page,
            self.room_id,
            sit_label,
            file_label,
            vols.join(" ")
        )
    }
}

/// Decodes a BGM `.adx` or `.aix` file from `game_root` using `source` into its constituent stereo layers.
pub fn decode_bgm_layers_from_disk(
    game_root: impl AsRef<Path>,
    rel_bgm_path: &str,
    source: AudioPackSource,
) -> Result<(PathBuf, Vec<DecodedAdx>), AudioError> {
    let path =
        resolve_bgm_stream_path_for_source(game_root, rel_bgm_path, source).ok_or_else(|| {
            AudioError::AssetError(format!("Could not resolve BGM path: {rel_bgm_path}"))
        })?;
    let bytes = std::fs::read(&path)
        .map_err(|e| AudioError::AssetError(format!("Failed to read {}: {e}", path.display())))?;
    let is_aix = rel_bgm_path.to_ascii_lowercase().ends_with(".aix")
        || (bytes.len() >= 4 && &bytes[0..4] == b"AIXF");
    let layers = if is_aix {
        decode_aix(&bytes).map_err(|e| AudioError::AssetError(e.to_string()))?
    } else {
        vec![decode_adx(&bytes).map_err(|e| AudioError::AssetError(e.to_string()))?]
    };
    Ok((path, layers))
}
