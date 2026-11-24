//! Engine and game configuration settings with persistent JSON serialization.

pub mod audio;
pub mod controls;
pub mod game;
pub mod video;

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

pub use audio::{
    ps2_level_to_sd_arg, ps2_level_to_volume_gain, volume_gain_to_ps2_level, AudioOptions,
    AudioPackDetectionStatus, AudioPackPreference, AudioPackSource, SpatialAudioMode,
    SpeakerConfiguration, PS2_VOLUME_MAX_LEVEL,
};
pub use controls::{
    ButtonGlyphStyle, CameraCyclePreset, ControlOptions, ControllerPreset, GameplayCameraMode,
    OtsShoulderSide, Ps2ControllerBindings, ReticleMode,
};
pub use game::{
    ActionDifficulty, BloodColor, BulletMultiplier, DebugOptions, EnhancedPreset, GameLanguage,
    GameOptions, RetreatTurnMode, RiddleDifficulty, ViewControlMode, WalkRunControlMode,
    PS2_BRIGHTNESS_MULTIPLIERS, PS2_DEFAULT_BRIGHTNESS_LEVEL, PS2_MAX_BRIGHTNESS_LEVEL,
};
pub use video::{
    FogQuality, FrameRateCap, InternalResolution, LightingProfile, OutdoorStreamingMode,
    ShadowQualityMode, VideoOptions, WindowDisplayMode, WorldTextureQuality,
};

/// Complete game configuration structure.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct AppConfig {
    pub video: VideoOptions,
    pub audio: AudioOptions,
    pub controls: ControlOptions,
    pub game: GameOptions,
    #[serde(default)]
    pub debug: DebugOptions,
}

impl AppConfig {
    /// Applies one of the 3 full version presets (`Ps2Authentic`, `Sh2ee`, `NativeEnhanced`) across
    /// all enhanced video, audio, and camera toggles in a single atomic operation.
    pub fn apply_enhanced_preset(&mut self, preset: EnhancedPreset) {
        match preset {
            EnhancedPreset::Ps2Authentic => {
                self.video.texture_quality = WorldTextureQuality::OriginalPs2;
                self.video.fog_quality = FogQuality::ClassicPs2;
                self.video.volumetric_light = false;
                self.video.shadow_quality = ShadowQualityMode::ClassicPs2Stencil;
                self.video.lighting_profile = LightingProfile::Ps2Authentic;
                self.video.outdoor_streaming_mode = OutdoorStreamingMode::ClassicPs2LoadBg2x2;
                self.video.aces_tonemapping = false;
                self.video.grain_intensity = 100;
                self.audio.spatial_audio_mode = SpatialAudioMode::ClassicPs2Stereo;
                self.audio.enable_acoustic_occlusion = false;
                self.audio.enable_dynamic_room_acoustics = false;
                self.audio.enable_restored_cut_audio = false;
                self.audio.audio_pack_source = AudioPackPreference::Ps2Original;
                self.controls.default_camera_mode = GameplayCameraMode::ClassicPs2Rail;
                self.game.enable_delayed_fade_in = false;
                self.game.enable_secondary_physics = false;
                self.game.enable_sh2ee_world_fixes = false;
            }
            EnhancedPreset::Sh2ee => {
                self.video.texture_quality = WorldTextureQuality::Sh2eeHd;
                self.video.fog_quality = FogQuality::ClassicPs2;
                self.video.volumetric_light = false;
                self.video.shadow_quality = ShadowQualityMode::EnhancedEditionSoft;
                self.video.lighting_profile = LightingProfile::EnhancedEdition;
                self.video.outdoor_streaming_mode = OutdoorStreamingMode::ClassicPs2LoadBg2x2;
                self.video.aces_tonemapping = false;
                self.audio.spatial_audio_mode = SpatialAudioMode::ClassicPs2Stereo;
                self.audio.enable_acoustic_occlusion = false;
                self.audio.enable_dynamic_room_acoustics = false;
                self.audio.enable_restored_cut_audio = true;
                self.audio.audio_pack_source = AudioPackPreference::Sh2eeLossless;
                self.controls.default_camera_mode = GameplayCameraMode::ClassicPs2Rail;
                self.game.enable_delayed_fade_in = true;
                self.game.enable_secondary_physics = false;
                self.game.enable_sh2ee_world_fixes = true;
            }
            EnhancedPreset::NativeEnhanced => {
                self.video.texture_quality = WorldTextureQuality::Ai4kMaster;
                self.video.fog_quality = FogQuality::ModernVolumetric;
                self.video.volumetric_light = true;
                self.video.shadow_quality = ShadowQualityMode::NativePcss;
                self.video.lighting_profile = LightingProfile::NativeEnhanced;
                self.video.outdoor_streaming_mode = OutdoorStreamingMode::NativeWholeTown;
                self.video.aces_tonemapping = true;
                self.audio.spatial_audio_mode = SpatialAudioMode::BinauralHrtf;
                self.audio.enable_acoustic_occlusion = true;
                self.audio.enable_dynamic_room_acoustics = true;
                self.audio.enable_restored_cut_audio = true;
                self.audio.audio_pack_source = AudioPackPreference::Auto;
                self.controls.default_camera_mode = GameplayCameraMode::OverTheShoulder;
                self.game.enable_delayed_fade_in = true;
                self.game.enable_secondary_physics = true;
                self.game.enable_sh2ee_world_fixes = true;
            }
            EnhancedPreset::Custom => {}
        }
    }

    /// Inspects all enhanced toggles and returns the matching [`EnhancedPreset`], or
    /// [`EnhancedPreset::Custom`] if the user has mixed individual settings.
    #[must_use]
    pub fn detect_enhanced_preset(&self) -> EnhancedPreset {
        let is_ps2 = self.video.texture_quality == WorldTextureQuality::OriginalPs2
            && self.video.fog_quality == FogQuality::ClassicPs2
            && !self.video.volumetric_light
            && self.video.shadow_quality == ShadowQualityMode::ClassicPs2Stencil
            && self.video.lighting_profile == LightingProfile::Ps2Authentic
            && self.video.outdoor_streaming_mode == OutdoorStreamingMode::ClassicPs2LoadBg2x2
            && !self.video.aces_tonemapping
            && self.audio.spatial_audio_mode == SpatialAudioMode::ClassicPs2Stereo
            && !self.audio.enable_acoustic_occlusion
            && !self.audio.enable_dynamic_room_acoustics
            && !self.audio.enable_restored_cut_audio
            && self.controls.default_camera_mode == GameplayCameraMode::ClassicPs2Rail
            && !self.game.enable_delayed_fade_in
            && !self.game.enable_secondary_physics
            && !self.game.enable_sh2ee_world_fixes;
        if is_ps2 {
            return EnhancedPreset::Ps2Authentic;
        }

        let is_sh2ee = self.video.texture_quality == WorldTextureQuality::Sh2eeHd
            && self.video.fog_quality == FogQuality::ClassicPs2
            && !self.video.volumetric_light
            && self.video.shadow_quality == ShadowQualityMode::EnhancedEditionSoft
            && self.video.lighting_profile == LightingProfile::EnhancedEdition
            && self.video.outdoor_streaming_mode == OutdoorStreamingMode::ClassicPs2LoadBg2x2
            && !self.video.aces_tonemapping
            && self.audio.spatial_audio_mode == SpatialAudioMode::ClassicPs2Stereo
            && !self.audio.enable_acoustic_occlusion
            && !self.audio.enable_dynamic_room_acoustics
            && self.audio.enable_restored_cut_audio
            && self.controls.default_camera_mode == GameplayCameraMode::ClassicPs2Rail
            && self.game.enable_delayed_fade_in
            && !self.game.enable_secondary_physics
            && self.game.enable_sh2ee_world_fixes;
        if is_sh2ee {
            return EnhancedPreset::Sh2ee;
        }

        let is_native = self.video.texture_quality == WorldTextureQuality::Ai4kMaster
            && self.video.fog_quality == FogQuality::ModernVolumetric
            && self.video.volumetric_light
            && self.video.shadow_quality == ShadowQualityMode::NativePcss
            && self.video.lighting_profile == LightingProfile::NativeEnhanced
            && self.video.outdoor_streaming_mode == OutdoorStreamingMode::NativeWholeTown
            && self.video.aces_tonemapping
            && self.audio.spatial_audio_mode == SpatialAudioMode::BinauralHrtf
            && self.audio.enable_acoustic_occlusion
            && self.audio.enable_dynamic_room_acoustics
            && self.audio.enable_restored_cut_audio
            && self.controls.default_camera_mode == GameplayCameraMode::OverTheShoulder
            && self.game.enable_delayed_fade_in
            && self.game.enable_secondary_physics
            && self.game.enable_sh2ee_world_fixes;
        if is_native {
            return EnhancedPreset::NativeEnhanced;
        }

        EnhancedPreset::Custom
    }
    /// Serialize and write configuration to disk atomically.
    ///
    /// Writes to a temporary `.tmp` sibling file, flushes to disk, and renames over the target path.
    /// This prevents configuration corruption or zero-byte file writes if the application terminates
    /// or crashes during saving.
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> std::io::Result<()> {
        let serialized = serde_json::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        let target = path.as_ref();
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        let tmp_path = target.with_extension(format!(
            "tmp.{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        fs::write(&tmp_path, serialized)?;
        fs::rename(&tmp_path, target)?;
        Ok(())
    }

    /// Load configuration from disk, falling back to default if file is missing or invalid.
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Self {
        if let Ok(data) = fs::read_to_string(path) {
            if let Ok(config) = serde_json::from_str(&data) {
                return config;
            }
        }
        Self::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_defaults_and_cycling() {
        let mut cfg = AppConfig::default();
        assert_eq!(cfg.video.resolution, InternalResolution::Native);
        assert_eq!(cfg.video.resolution.next(), InternalResolution::Res1080p);
        assert_eq!(cfg.video.resolution.prev(), InternalResolution::Res4k);

        assert_eq!(cfg.video.display_mode.next(), WindowDisplayMode::Windowed);
        assert_eq!(cfg.video.fog_quality.toggle(), FogQuality::ClassicPs2);

        assert_eq!(
            cfg.audio.speaker_config.next(),
            SpeakerConfiguration::Headphones
        );
        assert_eq!(cfg.controls.glyph_style.next(), ButtonGlyphStyle::Xbox);
        assert_eq!(cfg.game.action_difficulty.next(), ActionDifficulty::Hard);
        assert_eq!(cfg.game.riddle_difficulty.next(), RiddleDifficulty::Hard);
        assert_eq!(cfg.game.language.next(), GameLanguage::Japanese);
        assert_eq!(cfg.game.language.code(), 'e');
        assert_eq!(cfg.game.bullet_multiplier.next(), BulletMultiplier::Two);

        cfg.video.grain_intensity = 80;
        cfg.audio.sfx_volume = 0.75;
        assert_eq!(cfg.video.grain_intensity, 80);
        assert_eq!(cfg.audio.sfx_volume, 0.75);

        // Verify SH2-56 FrameRateCap cycle: 60 -> 120 -> 144 -> 240 -> Uncapped -> 30 -> 60
        let cap0 = FrameRateCap::default();
        assert_eq!(cap0, FrameRateCap::Fps60);
        assert_eq!(cap0.target_fps(), Some(60));
        let cap1 = cap0.next();
        assert_eq!(cap1, FrameRateCap::Fps120);
        assert_eq!(cap1.target_fps(), Some(120));
        let cap2 = cap1.next();
        assert_eq!(cap2, FrameRateCap::Fps144);
        assert_eq!(cap2.target_fps(), Some(144));
        let cap3 = cap2.next();
        assert_eq!(cap3, FrameRateCap::Fps240);
        assert_eq!(cap3.target_fps(), Some(240));
        let cap4 = cap3.next();
        assert_eq!(cap4, FrameRateCap::Uncapped);
        assert_eq!(cap4.target_fps(), None);
        let cap5 = cap4.next();
        assert_eq!(cap5, FrameRateCap::Fps30);
        assert_eq!(cap5.target_fps(), Some(30));
        assert_eq!(cap5.next(), FrameRateCap::Fps60);

        // Verify SH2-59 AudioPackPreference cycle: Auto -> Sh2eeLossless -> Ps2Original -> Auto
        let ap0 = AudioPackPreference::default();
        assert_eq!(ap0, AudioPackPreference::Auto);
        let ap1 = ap0.next();
        assert_eq!(ap1, AudioPackPreference::Sh2eeLossless);
        let ap2 = ap1.next();
        assert_eq!(ap2, AudioPackPreference::Ps2Original);
        assert_eq!(ap2.next(), AudioPackPreference::Auto);
        assert_eq!(ap0.prev(), AudioPackPreference::Ps2Original);

        // Verify AudioPackDetectionStatus resolution logic
        let both = AudioPackDetectionStatus {
            sh2ee_lossless_present: true,
            ps2_original_present: true,
            sh2ee_extra_present: true,
        };
        assert!(both.both_detected());
        assert_eq!(
            both.resolve_source(AudioPackPreference::Auto),
            AudioPackPreference::Sh2eeLossless
        );
        assert_eq!(
            both.resolve_source(AudioPackPreference::Ps2Original),
            AudioPackPreference::Ps2Original
        );

        let only_ps2 = AudioPackDetectionStatus {
            sh2ee_lossless_present: false,
            ps2_original_present: true,
            sh2ee_extra_present: false,
        };
        assert!(!only_ps2.both_detected());
        assert_eq!(
            only_ps2.resolve_source(AudioPackPreference::Auto),
            AudioPackPreference::Ps2Original
        );
        assert_eq!(
            only_ps2.resolve_source(AudioPackPreference::Sh2eeLossless),
            AudioPackPreference::Ps2Original
        );

        let only_ee = AudioPackDetectionStatus {
            sh2ee_lossless_present: true,
            ps2_original_present: false,
            sh2ee_extra_present: true,
        };
        assert!(!only_ee.both_detected());
        assert_eq!(
            only_ee.resolve_source(AudioPackPreference::Auto),
            AudioPackPreference::Sh2eeLossless
        );
        assert_eq!(
            only_ee.resolve_source(AudioPackPreference::Ps2Original),
            AudioPackPreference::Sh2eeLossless
        );
    }

    #[test]
    fn test_config_serialization_roundtrip() {
        let mut original = AppConfig::default();
        original.video.resolution = InternalResolution::Res1440p;
        original.video.grain_intensity = 45;
        original.audio.master_volume = 0.8;
        original.controls.tank_controls = false;
        original.controls.default_camera_mode = GameplayCameraMode::OverTheShoulder;
        original.controls.camera_cycle_preset = CameraCyclePreset::ClassicAndOts;
        original.controls.ots_shoulder = OtsShoulderSide::Left;
        original.controls.ots_reticle = ReticleMode::AlwaysSubtle;
        original.controls.first_person_fov_deg = 85;
        original.controls.first_person_head_bob = false;
        original.game.language = GameLanguage::Spanish;

        let json = serde_json::to_string(&original).expect("serialize config");
        let decoded: AppConfig = serde_json::from_str(&json).expect("deserialize config");

        assert_eq!(original, decoded);
    }
}
