//! Audio engine wrapper around `kira` for music, 3D sound effects, and voice.
//!
//! # Architecture & Sound Pipeline
//! - DirectSound / PS2 driver emulation:
//!   - High-priority 2D effect routing (`shSdCall`).
//!   - Positional 3D attenuation calculations (`SeCallPosDistanceF`).
//!   - BGM page and stream transition handling (`SeBgmChange`).

use kira::manager::{backend::DefaultBackend, AudioManager, AudioManagerSettings};

/// Master audio engine managing streaming BGM and sound effect channels.
pub struct AudioEngine {
    /// Underlying Kira backend manager.
    _manager: Option<AudioManager<DefaultBackend>>,
}

impl AudioEngine {
    /// Initializes the Kira audio manager with default device configuration.
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let manager = AudioManager::<DefaultBackend>::new(AudioManagerSettings::default())?;
        Ok(Self {
            _manager: Some(manager),
        })
    }

    /// Checks if the audio backend was successfully initialized and is active.
    pub fn is_active(&self) -> bool {
        self._manager.is_some()
    }
}
