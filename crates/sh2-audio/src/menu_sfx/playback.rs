//! 2D SFX, surface-material footsteps, flashlight clicks, and cut/unused audio preview playback
//! on [`MenuAudioBank`] (`SH2-61`, `SH2-62`).

use kira::sound::static_sound::StaticSoundHandle;
use sh2_assets::formats::sound::slice_wav_time_range;
use sh2_core::SurfaceMaterial;

use super::{synthesize_fallback_cue, MenuAudioBank, SoundCueKind};
use crate::beta_audio_catalog::{
    CutSfxCatalogEntry, LAURA_LETTER_VOICEOVER_SLICES, VOICE_AFS_LAURA_LETTER_INDEX,
};
use crate::sfx_bank::footstep_sddata_indices;
use crate::{AudioEngine, AudioError};

impl MenuAudioBank {
    /// Plays a sound effect by its 0-based `sddata.bin` index (`0..417`) at 2D `volume`.
    pub fn play_sddata_index(
        &self,
        engine: &mut AudioEngine,
        index: usize,
        volume: f32,
    ) -> Result<Option<StaticSoundHandle>, AudioError> {
        if let Some(prepared) = self.get_or_prepare_sddata(index) {
            let handle = engine.play_prepared_sfx_2d(&prepared, volume)?;
            Ok(Some(handle))
        } else {
            synthesize_fallback_cue(engine, SoundCueKind::Footstep)
        }
    }

    /// Plays a game sound effect by its `se_id` (`0x5147E0` / `0x5148B0`, e.g., `11048` Handgun, `11051` Shotgun, `11032` Plank, `11040` Pipe/Kick).
    pub fn play_se_id(
        &self,
        engine: &mut AudioEngine,
        se_id: u32,
        volume: f32,
    ) -> Result<Option<StaticSoundHandle>, AudioError> {
        if let Some(prepared) = self.resolve_prepared_se_id(se_id) {
            let handle = engine.play_prepared_sfx_2d(&prepared, volume)?;
            Ok(Some(handle))
        } else {
            synthesize_fallback_cue(engine, SoundCueKind::Confirm)
        }
    }

    /// Plays a surface-material-specific footstep for James or Maria, alternating left/right step samples (`SH2-61`).
    ///
    /// Also enforces `sh2pc.exe` `0x518270` voice-replacement behavior: if the previous footstep is still
    /// ringing, its tail is faded out (`25 ms`) before the new step triggers so footsteps never stack.
    ///
    /// Returns the `sddata.bin` index that was triggered.
    pub fn play_footstep(
        &mut self,
        engine: &mut AudioEngine,
        is_maria: bool,
        material: SurfaceMaterial,
        is_running: bool,
        volume: f32,
    ) -> Result<(usize, Option<StaticSoundHandle>), AudioError> {
        let (left_idx, right_idx) = footstep_sddata_indices(is_maria, material, is_running);
        let chosen_idx = if self.step_parity {
            right_idx
        } else {
            left_idx
        };
        self.step_parity = !self.step_parity;

        // `0x518270`: Stop previous footstep instance if still playing so tails do not overlap
        if let Some(mut prev_handle) = self.last_footstep_handle.take() {
            prev_handle.stop(kira::tween::Tween {
                duration: std::time::Duration::from_millis(25),
                easing: kira::tween::Easing::Linear,
                ..Default::default()
            });
        }

        if let Some(prepared) = self.get_or_prepare_sddata(chosen_idx).or_else(|| {
            self.footstep
                .as_ref()
                .and_then(|s| self.prepare_source_ref(s))
        }) {
            let handle = engine.play_prepared_sfx_2d(&prepared, volume)?;
            self.last_footstep_handle = Some(handle);
            Ok((chosen_idx, None))
        } else {
            let handle = synthesize_fallback_cue(engine, SoundCueKind::Footstep)?;
            self.last_footstep_handle = handle;
            Ok((chosen_idx, None))
        }
    }

    /// Plays a flashlight toggle click (`FlashlightOn` when `turned_on == true`, `FlashlightOff` when `false`).
    pub fn play_flashlight_toggle(
        &self,
        engine: &mut AudioEngine,
        turned_on: bool,
    ) -> Result<Option<StaticSoundHandle>, AudioError> {
        self.play(
            engine,
            if turned_on {
                SoundCueKind::FlashlightOn
            } else {
                SoundCueKind::FlashlightOff
            },
        )
    }

    /// Plays a sound cue through the audio engine using authentic audio.
    pub fn play(
        &self,
        engine: &mut AudioEngine,
        cue: SoundCueKind,
    ) -> Result<Option<StaticSoundHandle>, AudioError> {
        let entry = match cue {
            SoundCueKind::CursorMove => self.cursor_move.as_ref(),
            SoundCueKind::Confirm => self.confirm.as_ref(),
            SoundCueKind::Cancel => self.cancel.as_ref(),
            SoundCueKind::GameStartTransition => self.game_start.as_ref(),
            SoundCueKind::SavePoint => self.save_sound.as_ref(),
            SoundCueKind::FlashlightOn => self.flashlight_on.as_ref(),
            SoundCueKind::FlashlightOff => self.flashlight_off.as_ref(),
            SoundCueKind::Footstep => self.footstep.as_ref(),
        };

        if let Some(prepared) = entry.and_then(|src| self.prepare_source_ref(src)) {
            let handle = engine.play_prepared_sfx_2d(&prepared, 1.0)?;
            Ok(Some(handle))
        } else {
            // Fallback synthetic generator if asset was unavailable
            synthesize_fallback_cue(engine, cue)
        }
    }

    /// Stops any currently playing cut/beta audio or voiceover slice (`SH2-62`).
    pub fn stop_active_cut_audio(&mut self) {
        if let Some(mut handle) = self.active_cut_audio_handle.take() {
            handle.stop(kira::tween::Tween {
                duration: std::time::Duration::from_millis(60),
                easing: kira::tween::Easing::Linear,
                ..Default::default()
            });
        }
    }

    /// Plays one of the 11 timed voiceover slices (`page_index` `0..=10`) of Mary's unused letter
    /// to Laura (`voice.afs[119]` / `SCENE_45` in `PlayUnusedAudio.cpp`) (`SH2-62`).
    pub fn play_laura_letter_slice(
        &mut self,
        engine: &mut AudioEngine,
        page_index: usize,
        volume: f32,
    ) -> Result<Option<StaticSoundHandle>, AudioError> {
        self.stop_active_cut_audio();
        let slice_meta = LAURA_LETTER_VOICEOVER_SLICES
            .get(page_index)
            .copied()
            .unwrap_or(LAURA_LETTER_VOICEOVER_SLICES[0]);

        if let Some(full_wav) = self.load_voice_afs_entry(VOICE_AFS_LAURA_LETTER_INDEX) {
            let sliced = slice_wav_time_range(&full_wav, slice_meta.start_sec, slice_meta.end_sec);
            if !sliced.data.is_empty() {
                let handle = engine.play_sfx_2d(&sliced, volume)?;
                self.active_cut_audio_handle = Some(handle);
                return Ok(None);
            }
        }

        let handle = synthesize_fallback_cue(engine, SoundCueKind::Confirm)?;
        self.active_cut_audio_handle = handle;
        Ok(None)
    }

    /// Resolves and plays any [`CutSfxCatalogEntry`] (`sddata.bin` unreferenced/3D cue, `voice.afs` stream,
    /// or `sh2e/sound/extra/*.wav` bonus cue), stopping any previously auditioned cut audio first (`SH2-62`).
    pub fn play_cut_catalog_entry(
        &mut self,
        engine: &mut AudioEngine,
        entry: &CutSfxCatalogEntry,
        volume: f32,
    ) -> Result<Option<StaticSoundHandle>, AudioError> {
        self.stop_active_cut_audio();

        if let Some(sd_idx) = entry.sddata_index {
            if let Some(prepared) = self.get_or_prepare_sddata(sd_idx) {
                let handle = engine.play_prepared_sfx_2d(&prepared, volume)?;
                self.active_cut_audio_handle = Some(handle);
                return Ok(None);
            }
        }

        if let Some(afs_idx) = entry.voice_afs_index {
            // For `SCENE_45` (119), play the opening slice (0..11.7s) when auditioned from the catalog
            if afs_idx == VOICE_AFS_LAURA_LETTER_INDEX {
                if let Some(full_wav) = self.load_voice_afs_entry(afs_idx) {
                    let preview = slice_wav_time_range(&full_wav, 0.0, 11.7);
                    let handle = engine.play_sfx_2d(&preview, volume)?;
                    self.active_cut_audio_handle = Some(handle);
                    return Ok(None);
                }
            } else if let Some(wav) = self.load_voice_afs_entry(afs_idx) {
                let handle = engine.play_sfx_2d(&wav, volume)?;
                self.active_cut_audio_handle = Some(handle);
                return Ok(None);
            }
        }

        if let Some(rel) = entry.extra_rel_path {
            let wav_opt = if rel.ends_with("sankaku_toujyou_2.wav") {
                self.closet_bonus_audio.clone()
            } else if rel.ends_with("machi.wav") {
                self.machi_steps_audio.clone()
            } else {
                let mut slot = None;
                if let Some(ref dr) = self.data_root {
                    Self::load_extra_wav_override(
                        dr,
                        self.ee_root.as_deref(),
                        true,
                        rel,
                        &mut slot,
                    );
                }
                slot
            };
            if let Some(wav) = wav_opt {
                let handle = engine.play_sfx_2d(&wav, volume)?;
                self.active_cut_audio_handle = Some(handle);
                return Ok(None);
            }
        }

        let handle = synthesize_fallback_cue(engine, SoundCueKind::Confirm)?;
        self.active_cut_audio_handle = handle;
        Ok(None)
    }
}
