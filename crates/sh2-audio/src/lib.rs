//! sh2-audio: High-definition streaming sound engine using kira.

pub mod engine;
pub mod menu_sfx;

pub use engine::{ActiveDialogue, AudioEngine, AudioError, EnvironmentalPreset};
pub use menu_sfx::{play_menu_sound, MenuAudioBank, SoundCueKind};
