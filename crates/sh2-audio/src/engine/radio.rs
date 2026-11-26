//! 4-track Pocket Radio static playback (`0x2B22` / `shSdRadio` `0x00291420`, `SH2-167`).
//!
//! # Ground Truth (`SLUS_202.28` & `sh2pc.exe`)
//! - `SLUS_202.28` `Event/event.c` `RadioNoise` (`0x001ab300`) & `sound/sh_sd_call.c` `shSdRadio` (`0x00291420`):
//!   Starts the 4-track radio static stream via `shSdCall(0x2b22)`, updates tracks `1..=4` via
//!   `shSdRadio(track, vol_0_127, pan_0_127)`, and stops all tracks via `shSdCall(0x406)`.
//! - `sh2pc.exe` `FUN_00518680` (`0x00518680`) & `DAT_008a67cc..0x008a67dc`:
//!   Preloads the 4 looping radio static waveforms from `sddata.bin` driver IDs `5360..=5363`
//!   (`0x14f0..=0x14f3`), corresponding to 0-based `sddata.bin` indices `[360, 361, 362, 363]`.
//! - `sh2pc.exe` `FUN_005186d0` (`0x005186d0`) & `FUN_00518770` (`0x00518770`):
//!   Applies per-track volume (`0..=127`) and pan (`0..=127`, `64` = center), and silences all
//!   tracks on stop.

use std::sync::Arc;
use std::time::Duration;

use kira::{
    sound::{
        static_sound::{StaticSoundData, StaticSoundSettings},
        EndPosition, PlaybackPosition, PlaybackState, Region,
    },
    tween::{Easing, Tween},
    Volume,
};
use sh2_assets::WavEntry;

use super::{wav_entry_to_frames_48k, AudioEngine, AudioError, LoopRegionSamples};

/// 0-based `sddata.bin` indices for the 4 simultaneous Pocket Radio static tracks
/// (`sh2pc.exe` `FUN_00518680` `0x00518680` / `DAT_008a67cc..0x008a67dc`: driver IDs `5360..=5363`).
pub const RADIO_STATIC_SDDATA_INDICES: [usize; 4] = [360, 361, 362, 363];

/// Converts `shSdRadio` (`0x00291420`) driver `(volume, pan)` bytes (`0..=127`, `64` = center)
/// to normalized `(linear_gain, kira_pan_01)` in `[0.0, 1.0]`.
#[inline]
pub fn radio_driver_to_volume_pan(driver_vol: u8, driver_pan: u8) -> (f32, f32) {
    let gain = (f32::from(driver_vol.min(127)) / 127.0).clamp(0.0, 1.0);
    let pan_01 = (f32::from(driver_pan.min(127)) / 127.0).clamp(0.0, 1.0);
    (gain, pan_01)
}

impl AudioEngine {
    /// Starts all 4 looping Pocket Radio static tracks (`shSdCall(0x2b22)` / `FUN_00518680` `0x00518680`)
    /// at initial volume `0.0` and center pan `0.5`.
    pub fn start_radio_tracks(&mut self, tracks: [&WavEntry; 4]) -> Result<(), AudioError> {
        self.stop_radio_tracks();

        let manager = self
            .manager
            .as_mut()
            .ok_or_else(|| AudioError::InitError("No manager".into()))?;
        let sfx_track = self
            .sfx_track
            .as_ref()
            .ok_or_else(|| AudioError::InitError("No sfx track".into()))?;

        let mut handles: [Option<kira::sound::static_sound::StaticSoundHandle>; 4] =
            [None, None, None, None];
        for (idx, wav) in tracks.iter().enumerate() {
            let (sample_rate, frames) = wav_entry_to_frames_48k(wav);
            let mut settings = StaticSoundSettings::new()
                .output_destination(sfx_track)
                .volume(Volume::Amplitude(0.0))
                .panning(0.5);

            if wav.loops {
                if let Some(region) = LoopRegionSamples::from_source_samples(
                    wav.loop_start,
                    wav.loop_end,
                    wav.sample_rate,
                    sample_rate,
                    frames.len(),
                ) {
                    settings = settings.loop_region(region.to_kira_region());
                } else {
                    settings = settings.loop_region(Region {
                        start: PlaybackPosition::Seconds(0.0),
                        end: EndPosition::EndOfAudio,
                    });
                }
            } else {
                settings = settings.loop_region(Region {
                    start: PlaybackPosition::Seconds(0.0),
                    end: EndPosition::EndOfAudio,
                });
            }

            let sound_data = StaticSoundData {
                sample_rate,
                frames: Arc::from(frames),
                settings,
                slice: None,
            };

            let handle = manager
                .play(sound_data)
                .map_err(|e| AudioError::PlaybackError(e.to_string()))?;
            handles[idx] = Some(handle);
        }

        self.active_radio_tracks = handles;
        Ok(())
    }

    /// Updates the 4 active Pocket Radio static tracks with `shSdRadio` (`0x00291420` / `FUN_005186d0`)
    /// driver volumes (`0..=127`) and pans (`0..=127`, `64` = center).
    pub fn update_radio_tracks(&mut self, driver_volumes: [u8; 4], driver_pans: [u8; 4]) {
        let tween = Tween {
            duration: Duration::from_millis(16),
            easing: Easing::Linear,
            ..Default::default()
        };
        let scale = (self.sfx_volume * self.master_volume) as f64;
        for (idx, slot) in self.active_radio_tracks.iter_mut().enumerate() {
            if let Some(ref mut handle) = slot {
                let (gain, pan_01) =
                    radio_driver_to_volume_pan(driver_volumes[idx], driver_pans[idx]);
                handle.set_volume(Volume::Amplitude(gain as f64 * scale), tween);
                handle.set_panning(pan_01 as f64, tween);
            }
        }
    }

    /// Stops all 4 Pocket Radio static tracks (`shSdCall(0x406)` / `FUN_00518770`).
    pub fn stop_radio_tracks(&mut self) {
        let tween = Tween {
            duration: Duration::from_millis(40),
            easing: Easing::Linear,
            ..Default::default()
        };
        for slot in &mut self.active_radio_tracks {
            if let Some(mut handle) = slot.take() {
                handle.stop(tween);
            }
        }
    }

    /// Returns `true` if any of the 4 Pocket Radio static tracks is currently active.
    pub fn is_radio_playing(&self) -> bool {
        self.active_radio_tracks.iter().any(|slot| {
            slot.as_ref()
                .is_some_and(|h| h.state() != PlaybackState::Stopped)
        })
    }
}
