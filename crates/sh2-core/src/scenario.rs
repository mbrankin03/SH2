//! Scenario state machine and progression logic for Main Campaign and Born From a Wish.
//!
//! Ground truth references:
//! - Main Campaign (Stages 1–52): James Sunderland, *Letter from Silent Heaven*
//! - Sub-Scenario (Stages 53–58): Maria in Baldwin Mansion (`prog_wish.c`, Director's Cut)

use crate::config::{ActionDifficulty, RiddleDifficulty};
use crate::cutaway::CutawayOverlayEngine;
use crate::events::{
    init_new_game, init_new_game_wish, light_spot_on_off_check, ClearData, EventFlags, PuzzleState,
    Sh2Rng, StageProgramRunner,
};
use crate::inventory::InventoryManager;
use crate::lighting::{FLAG_WOODSIDE_FLASHLIGHT_PICKUP, ITEM_ID_FLASHLIGHT};
use crate::room_loader::default_room_id_for_stage;
use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::f32::consts::PI;

/// Active playable game scenario.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
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
    /// Active room map/camera identifier (e.g. `"er01"`, `"ob01"`, `"rr21"`, `"th09"`, `"ma61"`) (`SH2-50`).
    pub current_room: String,
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
    /// Full 85-item catalog inventory manager (`SH2-15`).
    pub inventory_mgr: InventoryManager,
    /// 16,384-bit stage event condition flag block (`0x1f7d5e0`).
    pub flags: EventFlags,
    /// 32-byte randomized puzzle state block (`0x1f7da60..0x1f7da7f`).
    pub puzzle_state: PuzzleState,
    /// Step-based `ev_prog` stage program runner (`EventExecProgram`).
    pub prog_runner: StageProgramRunner,
    /// Pre-rendered cutaway still and interactive puzzle sprite overlay engine (`EvSubPictureDisplay`).
    pub cutaway_overlay: CutawayOverlayEngine,
    /// Active riddle difficulty (`Easy`, `Normal`, `Hard`, `Extra`).
    pub riddle_difficulty: RiddleDifficulty,
    /// Active action difficulty (`Easy`, `Normal`, `Hard`).
    pub action_difficulty: ActionDifficulty,
    /// Accumulated boss completion signals emitted by [`crate::ai::AiSubsystem::consume_boss_events`].
    pub boss_events: crate::ai::AiBossEvents,
    /// Authentic `Playing_Info` runtime telemetry (`SLUS_202.28` `0x0122b310` / `sh2pc.exe` `0x01dbbf80`).
    pub playing_info: crate::endings::PlayingInfo,
    /// Live ending-point telemetry (`sh2pc.exe` `0x01f7adcc..0x01f7ae7b` / `EvProgEndingJudge` `0x00576490`).
    pub ending_tracker: crate::endings::EndingPointTracker,
    /// Active multi-floor elevator button panel (`FUN_004a2a50` `0x004a2e29`), if open.
    pub active_elevator: Option<crate::world::MultiFloorElevatorPanel>,
    /// Per-frame `EventMain` `sys.flag` word (`SLUS_202.28` `0x010f08a4` / `sh2pc.exe` `DAT_00932194`).
    pub event_main_sys_flags: u32,
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
            ScenarioKind::LetterFromSilentHeaven => {
                let mut inventory_mgr = InventoryManager::new();
                inventory_mgr.item_get_now(20); // Photo of Mary
                inventory_mgr.item_get_now(21); // Letter from Mary

                let mut flags = EventFlags::new();
                let mut puzzle_state = PuzzleState::new();
                let mut rng = Sh2Rng::new(0x1999_0901);
                let riddle_difficulty = init_new_game(
                    &mut flags,
                    &mut puzzle_state,
                    &mut rng,
                    RiddleDifficulty::Normal,
                    ActionDifficulty::Normal,
                    ClearData::default(),
                );

                Self {
                    scenario,
                    current_stage: 1, // Observation Deck restroom overlook
                    stage_name: "Observation Deck",
                    current_room: "er01".to_string(),
                    character: PlayableCharacter::James,
                    health: 100.0,
                    max_health: 100.0,
                    flashlight_on: false, // Obtained in apartment room 205
                    radio_on: false,      // Obtained in construction tunnel
                    inventory: vec![
                        GameItem {
                            id: 20,
                            name: "Photo of Mary".into(),
                            quantity: 1,
                        },
                        GameItem {
                            id: 21,
                            name: "Letter from Mary".into(),
                            quantity: 1,
                        },
                    ],
                    event_flags: HashSet::new(),
                    // Stage 2 (er01) Opening Restroom post-prologue mirror spawn: (-19606.6, 0.0, 20429.55) facing West (-PI/2)
                    player_position: Vec3::new(-19606.6, 0.0, 20429.55),
                    player_yaw: -std::f32::consts::FRAC_PI_2,
                    inventory_mgr,
                    flags,
                    puzzle_state,
                    prog_runner: StageProgramRunner::new(),
                    cutaway_overlay: CutawayOverlayEngine::new(),
                    riddle_difficulty,
                    action_difficulty: ActionDifficulty::Normal,
                    boss_events: crate::ai::AiBossEvents::default(),
                    playing_info: crate::endings::PlayingInfo::default(),
                    ending_tracker: crate::endings::EndingPointTracker::default(),
                    active_elevator: None,
                    event_main_sys_flags: 0,
                }
            }
            ScenarioKind::BornFromAWish => {
                let mut inventory_mgr = InventoryManager::new();
                inventory_mgr.item_get_now(10); // Revolver
                inventory_mgr.item_get_now(11); // Revolver bullets x2 boxes
                inventory_mgr.item_get_now(11);
                inventory_mgr.item_get_now(2); // First-aid kit

                let mut flags = EventFlags::new();
                let mut puzzle_state = PuzzleState::new();
                init_new_game_wish(&mut flags, &mut puzzle_state);

                let mut event_flags = HashSet::new();
                event_flags.insert(5);
                event_flags.insert(24);

                Self {
                    scenario,
                    current_stage: 53, // Heaven's Night bar
                    stage_name: "Heaven's Night",
                    current_room: "th09".to_string(),
                    character: PlayableCharacter::Maria,
                    health: 100.0,
                    max_health: 100.0,
                    flashlight_on: false,
                    radio_on: false,
                    inventory: vec![GameItem {
                        id: 0x0a,
                        name: "Revolver".into(),
                        quantity: 1,
                    }],
                    event_flags,
                    // Stage 53 initial coordinates from prog_wish.c: (59622.3f, 0.0f, 20308.75f, -1.5707964f)
                    player_position: Vec3::new(59622.3, 0.0, 20308.75),
                    player_yaw: -1.5707964,
                    inventory_mgr,
                    flags,
                    puzzle_state,
                    prog_runner: StageProgramRunner::new(),
                    cutaway_overlay: CutawayOverlayEngine::new(),
                    riddle_difficulty: RiddleDifficulty::Normal,
                    action_difficulty: ActionDifficulty::Normal,
                    boss_events: crate::ai::AiBossEvents::default(),
                    playing_info: crate::endings::PlayingInfo::default(),
                    ending_tracker: crate::endings::EndingPointTracker::default(),
                    active_elevator: None,
                    event_main_sys_flags: 0,
                }
            }
        }
    }

    /// Consume boss completion signals from [`crate::ai::AiSubsystem::consume_boss_events`]
    /// into stage `EventFlags` and `Inventory` (e.g. PH stairwell siren clear + water drain,
    /// Dual PH Rust-Colored & Scarlet Eggs).
    pub fn apply_boss_events(&mut self, events: &crate::ai::AiBossEvents) {
        if events.ph_battle_end_complete && !self.boss_events.ph_battle_end_complete {
            crate::events::stages_act1::act1_apply_ph_battle_end(&mut self.flags);
            self.event_flags.insert(148);
            self.event_flags.insert(526);
            self.boss_events.ph_battle_end_complete = true;
        }
        if events.flesh_lips_battle_cleared {
            self.boss_events.flesh_lips_battle_cleared = true;
        }
        if events.abstract_daddy_defeated {
            self.boss_events.abstract_daddy_defeated = true;
        }
        if events.eddie_room1_escaped {
            self.boss_events.eddie_room1_escaped = true;
        }
        if events.eddie_room2_dead {
            self.boss_events.eddie_room2_dead = true;
        }
        if events.twin_ph_suicide_ready && !self.boss_events.twin_ph_suicide_ready {
            // `EvProgEggHoleOnDoor` (`0x005768c0`): Rust-colored egg (`72`) & Scarlet egg (`73`).
            if !self.inventory_mgr.has_item(72) {
                self.add_item(72, "Rust-colored egg", 1);
            }
            if !self.inventory_mgr.has_item(73) {
                self.add_item(73, "Scarlet egg", 1);
            }
            self.boss_events.twin_ph_suicide_ready = true;
        }
        if events.mary_phase1_cleared {
            self.boss_events.mary_phase1_cleared = true;
        }
        if events.mary_ending_triggered {
            self.boss_events.mary_ending_triggered = true;
        }
    }

    /// Check if an event flag is raised.
    pub fn has_flag(&self, flag: u32) -> bool {
        self.event_flags.contains(&flag) || self.flags.get(flag as usize)
    }

    /// Set an event flag.
    pub fn set_flag(&mut self, flag: u32) {
        self.event_flags.insert(flag);
        self.flags.set(flag as usize, true);
    }

    /// Clear an event flag.
    pub fn clear_flag(&mut self, flag: u32) {
        self.event_flags.remove(&flag);
        self.flags.set(flag as usize, false);
    }

    /// Add an item or increment quantity in inventory.
    pub fn add_item(&mut self, id: u16, name: impl Into<String>, qty: u16) {
        if let Ok(id_u8) = u8::try_from(id) {
            self.inventory_mgr.give_raw(id_u8, qty as i32);
        }
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

    /// Synchronizes the legacy `inventory: Vec<GameItem>` view from `inventory_mgr`.
    pub fn sync_legacy_views(&mut self) {
        self.inventory = self
            .inventory_mgr
            .owned_items()
            .into_iter()
            .map(|(id, qty)| {
                let name = crate::inventory::item_metadata(id)
                    .map(|m| m.name)
                    .unwrap_or("Unknown")
                    .to_string();
                GameItem {
                    id: id as u16,
                    name,
                    quantity: qty,
                }
            })
            .collect();
    }

    /// Returns `true` when the player possesses the Flashlight (`item 0x12` = 18,
    /// `sh2pc.exe` `0x50a48e`: `0x01f7a7e0 & 0x40000`, acquired in Wood Side Room 205).
    #[inline]
    #[must_use]
    pub fn has_flashlight(&self) -> bool {
        self.inventory_mgr.has_item(ITEM_ID_FLASHLIGHT)
    }

    /// Evaluates `LightSpotOnOffCheck` (`SLUS_202.28` `0x001aad70`): returns `true` only when
    /// the player owns the Flashlight (`item 0x12`), the power switch is ON, and Event Flag `272`
    /// (dead battery / disabled flashlight) is NOT set.
    #[inline]
    #[must_use]
    pub fn is_flashlight_active(&self) -> bool {
        light_spot_on_off_check(self.has_flashlight(), self.flashlight_on, &self.flags)
    }

    /// Grants the Flashlight (`item 0x12`) to the player's inventory, raises Event Flag `625`
    /// (`FLAG_WOODSIDE_FLASHLIGHT_PICKUP`), and optionally switches it on (`SH2-33`).
    pub fn acquire_flashlight(&mut self, turn_on: bool) {
        if !self.has_flashlight() {
            self.inventory_mgr.item_get_now(ITEM_ID_FLASHLIGHT);
            self.sync_legacy_views();
        }
        self.set_flag(u32::from(FLAG_WOODSIDE_FLASHLIGHT_PICKUP));
        self.flashlight_on = turn_on && !self.flags.get(272);
    }

    /// Removes the Flashlight (`item 0x12`), clears Event Flag `625`, and switches the beam off,
    /// restoring the authentic pre-Room-205 pickup state (`s_flash_level = 1.0`, `SH2-33`).
    pub fn revoke_flashlight(&mut self) {
        if self.has_flashlight() {
            self.inventory_mgr.item_lose(ITEM_ID_FLASHLIGHT);
            self.sync_legacy_views();
        }
        self.clear_flag(u32::from(FLAG_WOODSIDE_FLASHLIGHT_PICKUP));
        self.flashlight_on = false;
    }

    /// Toggles the flashlight power switch if the player owns the Flashlight (`item 0x12`) and
    /// its battery is not depleted (`!flags.get(272)`). Returns the resulting active beam state.
    pub fn toggle_flashlight(&mut self) -> bool {
        if !self.has_flashlight() || self.flags.get(272) {
            self.flashlight_on = false;
            return false;
        }
        self.flashlight_on = !self.flashlight_on;
        self.is_flashlight_active()
    }

    /// Toggles the pocket radio power state if owned (`ITEM_ID_RADIO = 19`).
    pub fn toggle_radio(&mut self) -> bool {
        self.radio_on = !self.radio_on;
        self.radio_on
    }

    /// Returns `true` when the active stage is an indoor/night area requiring the pocket
    /// flashlight to read the paper map (`EventExecChizuFail` @ `SLUS_202.28` `0x001aae40`).
    /// Bright daytime outdoor stages (Stages `1..=6`: Restroom, Observation Deck, Forest, Town Day)
    /// return `false`; indoor/night stages (Apartments, Hospital, Prison, Labyrinth, Hotel, Mansion)
    /// return `true`.
    pub fn is_current_stage_dark(&self) -> bool {
        !matches!(self.current_stage, 0..=6)
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
        if let Some(default_room) = default_room_id_for_stage(stage_id) {
            self.current_room = default_room.to_string();
        }
        self.player_position = spawn_pos;
        self.player_yaw = spawn_yaw;
    }

    /// Transition to a specific room identifier (`"er01"`, `"ob01"`, `"rr21"`, `"ap01"`, `"ma61"`)
    /// within the current or target stage.
    pub fn set_current_room(&mut self, room_id: impl Into<String>) {
        let raw = room_id.into();
        if let Some((canonical, _)) = sh2_assets::formats::cam::normalize_room_cam_rel_path(&raw) {
            self.current_room = canonical;
        } else {
            self.current_room = raw;
        }
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

    #[test]
    fn test_flashlight_acquisition_and_toggle_gating() {
        let mut state = ScenarioState::new(ScenarioKind::LetterFromSilentHeaven);
        // Before Room 205 pickup: James does not own item 0x12 and cannot toggle flashlight ON
        assert!(!state.has_flashlight());
        assert!(!state.is_flashlight_active());
        assert!(!state.toggle_flashlight());
        assert!(!state.flashlight_on);

        // Acquire flashlight in Room 205 (item 0x12 + Flag 625)
        state.acquire_flashlight(true);
        assert!(state.has_flashlight());
        assert!(state.has_flag(u32::from(FLAG_WOODSIDE_FLASHLIGHT_PICKUP)));
        assert!(state.is_flashlight_active());

        // Toggle OFF and back ON
        assert!(!state.toggle_flashlight());
        assert!(!state.is_flashlight_active());
        assert!(state.toggle_flashlight());
        assert!(state.is_flashlight_active());

        // Dead battery (Flag 272) suppresses beam and prevents toggle
        state.set_flag(272);
        assert!(!state.is_flashlight_active());
        assert!(!state.toggle_flashlight());

        // Revoking flashlight restores authentic pre-pickup state
        state.clear_flag(272);
        state.revoke_flashlight();
        assert!(!state.has_flashlight());
        assert!(!state.has_flag(u32::from(FLAG_WOODSIDE_FLASHLIGHT_PICKUP)));
        assert!(!state.is_flashlight_active());
    }
}
