//! Persistent mod loader configuration (`SH2-29`).

use serde::{Deserialize, Serialize};

/// Persisted enablement and load-order priority for a single mod in `mods/`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModEntryConfig {
    /// Stable mod identifier (directory name or `.zip` archive stem).
    pub id: String,
    /// Whether this mod is currently enabled in the VFS overlay.
    pub enabled: bool,
    /// Load-order priority (higher values override lower values).
    pub priority: i32,
}

/// Top-level mod loader configuration stored in `config.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ModsConfig {
    /// Ordered list of known mod states.
    #[serde(default)]
    pub entries: Vec<ModEntryConfig>,
}

impl ModsConfig {
    /// Converts persisted entries into `(id, enabled, priority)` tuples for `sh2-assets::discover_mods`.
    #[must_use]
    pub fn as_discovery_tuples(&self) -> Vec<(String, bool, i32)> {
        self.entries
            .iter()
            .map(|e| (e.id.clone(), e.enabled, e.priority))
            .collect()
    }

    /// Sets or updates the enabled state and priority for `id`.
    pub fn set_mod_state(&mut self, id: impl Into<String>, enabled: bool, priority: i32) {
        let id_str = id.into();
        if let Some(existing) = self.entries.iter_mut().find(|e| e.id == id_str) {
            existing.enabled = enabled;
            existing.priority = priority;
        } else {
            self.entries.push(ModEntryConfig {
                id: id_str,
                enabled,
                priority,
            });
        }
    }
}
