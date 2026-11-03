//! Audio engine wrapper around kira.

use kira::manager::{AudioManager, AudioManagerSettings, backend::DefaultBackend};

pub struct AudioEngine {
    _manager: Option<AudioManager<DefaultBackend>>,
}

impl AudioEngine {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let manager = AudioManager::<DefaultBackend>::new(AudioManagerSettings::default())?;
        Ok(Self {
            _manager: Some(manager),
        })
    }

    pub fn is_active(&self) -> bool {
        self._manager.is_some()
    }
}
