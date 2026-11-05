//! Scenario state machine and progression logic for Main Campaign and Born From a Wish.
//!
//! Ground truth references:
//! - Main Campaign (Stages 1–52): James Sunderland, *Letter from Silent Heaven*
//! - Sub-Scenario (Stages 53–58): Maria in Baldwin Mansion (`prog_wish.c`, Director's Cut)

use glam::Vec3;
use std::collections::HashSet;
use std::f32::consts::PI;

/// Active playable game scenario.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScenarioKind {
    /// Main Campaign: James Sunderland (*Letter from Silent Heaven*, Stages 1–52).
    #[default]
    LetterFromSilentHeaven,
    /// Sub-Scenario: Maria (*Born from a Wish*, Stages 53–58, Baldwin Mansion & Blue Creek).
    BornFromAWish,
}

/// Character persona currently controlled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayableCharacter {
    James,
    Maria,
}

/// Stage definition and room configuration.
#[derive(Debug, Clone, PartialEq)]
pub struct StageData {
    pub stage_id: u16,
    pub name: &'static str,
    pub spawn_pos: Vec3,
    pub spawn_yaw: f32,
}

/// Inventory item descriptor.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GameItem {
    pub id: u16,
    pub name: String,
    pub quantity: u16,
}

/// Runtime scenario state tracker.
#[derive(Debug, Clone)]
pub struct ScenarioState {
    /// Active scenario kind.
    pub scenario: ScenarioKind,
    /// Current stage ID (1..52 for James, 53..58 for Maria).
    pub current_stage: u16,
    /// Current stage name.
    pub stage_name: &'static str,
    /// Player character.
    pub character: PlayableCharacter,
    /// Player health (100.0 max).
    pub health: f32,
    /// Maximum player health.
    pub max_health: f32,
    /// Flashlight active toggle.
    pub flashlight_on: bool,
    /// Pocket radio active toggle.
    pub radio_on: bool,
    /// Inventory item collection.
    pub inventory: Vec<GameItem>,
    /// Global puzzle and progression event flags.
    pub event_flags: HashSet<u32>,
    /// Player position in PS2 world space.
    pub player_position: Vec3,
    /// Player facing yaw in radians.
    pub player_yaw: f32,
}

impl Default for ScenarioState {
    fn default() -> Self {
        Self::new(ScenarioKind::LetterFromSilentHeaven)
    }
}

impl ScenarioState {
    /// Initialize scenario state according to scenario selection.
    pub fn new(scenario: ScenarioKind) -> Self {
        match scenario {
            ScenarioKind::LetterFromSilentHeaven => Self {
                scenario,
                current_stage: 1, // Observation Deck restroom overlook
                stage_name: "Observation Deck",
                character: PlayableCharacter::James,
                health: 100.0,
                max_health: 100.0,
                flashlight_on: false, // Obtained in apartment room 205
                radio_on: false,      // Obtained in construction tunnel
                inventory: vec![
                    GameItem {
                        id: 0x01,
                        name: "Photo of Mary".into(),
                        quantity: 1,
                    },
                    GameItem {
                        id: 0x02,
                        name: "Letter from Mary".into(),
                        quantity: 1,
                    },
                ],
                event_flags: HashSet::new(),
                // Stage 2 (er01) Opening Restroom post-prologue mirror spawn: (-19606.6, 0.0, 20429.55) facing West (-PI/2)
                player_position: Vec3::new(-19606.6, 0.0, 20429.55),
                player_yaw: -std::f32::consts::FRAC_PI_2,
            },
            ScenarioKind::BornFromAWish => Self {
                scenario,
                current_stage: 53, // Heaven's Night bar
                stage_name: "Heaven's Night",
                character: PlayableCharacter::Maria,
                health: 100.0,
                max_health: 100.0,
                flashlight_on: false,
                radio_on: false,
                inventory: vec![GameItem {
                    id: 0x10,
                    name: "Revolver".into(),
                    quantity: 1,
                }],
                event_flags: HashSet::new(),
                // Stage 53 initial coordinates from prog_wish.c: (59622.3f, 0.0f, 20308.75f, -1.5707964f)
                player_position: Vec3::new(59622.3, 0.0, 20308.75),
                player_yaw: -1.5707964,
            },
        }
    }

    /// Check if an event flag is raised.
    pub fn has_flag(&self, flag: u32) -> bool {
        self.event_flags.contains(&flag)
    }

    /// Set an event flag.
    pub fn set_flag(&mut self, flag: u32) {
        self.event_flags.insert(flag);
    }

    /// Clear an event flag.
    pub fn clear_flag(&mut self, flag: u32) {
        self.event_flags.remove(&flag);
    }

    /// Add an item or increment quantity in inventory.
    pub fn add_item(&mut self, id: u16, name: impl Into<String>, qty: u16) {
        if let Some(item) = self.inventory.iter_mut().find(|i| i.id == id) {
            item.quantity = item.quantity.saturating_add(qty);
        } else {
            self.inventory.push(GameItem {
                id,
                name: name.into(),
                quantity: qty,
            });
        }
    }

    /// Transition to a new stage/room cleanly.
    pub fn transition_stage(
        &mut self,
        stage_id: u16,
        stage_name: &'static str,
        spawn_pos: Vec3,
        spawn_yaw: f32,
    ) {
        self.current_stage = stage_id;
        self.stage_name = stage_name;
        self.player_position = spawn_pos;
        self.player_yaw = spawn_yaw;
    }

    /// Advance Maria's Born from a Wish scenario stage progression.
    pub fn progress_wish_stage(&mut self, target_stage: u16) -> Result<(), &'static str> {
        if self.scenario != ScenarioKind::BornFromAWish {
            return Err("Cannot progress Born from a Wish while playing Letter from Silent Heaven");
        }

        match target_stage {
            53 => self.transition_stage(
                53,
                "Heaven's Night",
                Vec3::new(59622.3, 0.0, 20308.75),
                -1.5707964,
            ),
            54 => {
                self.transition_stage(54, "Ernest Study End", Vec3::new(32100.0, 0.0, 15400.0), PI)
            }
            55 => self.transition_stage(
                55,
                "Baldwin Mansion 1F",
                Vec3::new(24500.0, 0.0, 12000.0),
                0.0,
            ),
            56 => self.transition_stage(
                56,
                "Baldwin Mansion 2F",
                Vec3::new(24500.0, -3200.0, 12000.0),
                0.0,
            ),
            57 => self.transition_stage(
                57,
                "Blue Creek Transition",
                Vec3::new(18000.0, 0.0, 8500.0),
                1.5707964,
            ),
            58 => self.transition_stage(
                58,
                "Baldwin Mansion Attic",
                Vec3::new(24500.0, -6400.0, 12000.0),
                0.0,
            ),
            _ => return Err("Invalid stage ID for Born from a Wish (valid: 53-58)"),
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_main_scenario_initialization() {
        let state = ScenarioState::new(ScenarioKind::LetterFromSilentHeaven);
        assert_eq!(state.character, PlayableCharacter::James);
        assert_eq!(state.current_stage, 1);
        assert_eq!(state.inventory.len(), 2);
    }

    #[test]
    fn test_born_from_a_wish_stages() {
        let mut state = ScenarioState::new(ScenarioKind::BornFromAWish);
        assert_eq!(state.character, PlayableCharacter::Maria);
        assert_eq!(state.current_stage, 53);

        // Stage 53 -> 55 (Baldwin Mansion)
        assert!(state.progress_wish_stage(55).is_ok());
        assert_eq!(state.current_stage, 55);
        assert_eq!(state.stage_name, "Baldwin Mansion 1F");

        // Invalid stage check
        assert!(state.progress_wish_stage(99).is_err());
    }
}
