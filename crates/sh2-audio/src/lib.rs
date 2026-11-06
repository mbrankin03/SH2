//! sh2-audio: High-definition streaming sound engine using kira.

pub mod cutscene_bgm;
pub mod engine;
pub mod menu_sfx;

pub use cutscene_bgm::{
    find_cutscene_audio_mapping, master_volume_for_start_id, resolve_bgm_stream_path,
    CutsceneAudioMapping, CutsceneBgmAction, CUTSCENE_AUDIO_MAP,
};
pub use engine::{ActiveDialogue, AudioEngine, AudioError, EnvironmentalPreset};
pub use menu_sfx::{play_menu_sound, MenuAudioBank, SoundCueKind};
