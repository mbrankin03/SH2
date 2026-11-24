//! Boss completion signals and [`AiSubsystem`] hit/overflow helpers.
//!
//! # Ground Truth:
//! - `enONISetDamage` (`0x0022b370` / `sh2pc 0x004bc8b0`)
//! - `enREDCtrlBattleEnd` (`0x0015eeb0`), `enIKECtrlDead` (`0x001522b0`),
//!   `enPAPCtrlDown` (`0x0015baf0`), `enEDBCtrlEscape2` (`0x0014c2b0`),
//!   `enEDBCtrlDead` (`0x0014c750`), `enONICtrlSuicide` (`0x0022ad20`),
//!   `enBOSCtrlDead` (`0x00140820`).

use crate::ai::active_enemy::ActiveEnemy;
use crate::ai::AiSubsystem;

/// Boss completion signals emitted by active boss controllers for consumption by stage events
/// and inventory (`sh2-core::events`, `sh2-core::campaign`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AiBossEvents {
    /// Pyramid Head 1st encounter retreated down flooded stairwell (`enREDCtrlBattleEnd` `0x0015eeb0`).
    pub ph_battle_end_complete: bool,
    /// Last Flesh Lips defeated (`enIKECtrlDead` `0x001522b0`).
    pub flesh_lips_battle_cleared: bool,
    /// Abstract Daddy boss defeated (`enPAPCtrlDown` `0x0015baf0`).
    pub abstract_daddy_defeated: bool,
    /// Eddie Room 1 escaped (`enEDBCtrlEscape2` `0x0014c2b0`).
    pub eddie_room1_escaped: bool,
    /// Eddie Room 2 defeated (`enEDBCtrlDead` `0x0014c750`).
    pub eddie_room2_dead: bool,
    /// Twin Pyramid Heads reached suicide pose (`enONICtrlSuicide` `0x0022ad20`).
    pub twin_ph_suicide_ready: bool,
    /// Mary Boss Phase 1 cleared (`enBOSCtrlDead` `0x00140820`).
    pub mary_phase1_cleared: bool,
    /// Mary Boss Phase 2 defeated / ending triggered (`enBOSCtrlDead` `0x00140820`).
    pub mary_ending_triggered: bool,
}

impl AiSubsystem {
    /// Apply a player weapon hit to `enemies[enemy_idx]` and transfer any `TwinPyramidHead`
    /// HP overflow (`(-next_hp) * 0.75`) to its partner (`enONISetDamage` `0x0022b370` / `sh2pc 0x004bc8b0`).
    pub fn apply_hit_to_enemy(
        &mut self,
        enemy_idx: usize,
        atk: &crate::combat::attack_list::AttackSpec,
        attack_dir_xz: [f32; 2],
    ) {
        let Some(enemy) = self.enemies.get_mut(enemy_idx) else {
            return;
        };
        if let Some((hit_id, overflow)) = enemy.receive_hit_with_overflow(atk, attack_dir_xz) {
            for other in &mut self.enemies {
                if let ActiveEnemy::TwinPyramidHead(partner) = &mut other.controller {
                    if partner.id != hit_id {
                        partner.stats.health = (partner.stats.health - overflow).max(0.0);
                        break;
                    }
                }
            }
        }
    }

    /// Apply a Hyper Spray aerosol hit to `enemies[enemy_idx]`.
    pub fn apply_hyper_spray_hit_to_enemy(
        &mut self,
        enemy_idx: usize,
        atk: &crate::combat::attack_list::AttackSpec,
        color: crate::combat::attack_list::HyperSprayColor,
    ) -> bool {
        let Some(enemy) = self.enemies.get_mut(enemy_idx) else {
            return false;
        };
        enemy.receive_hyper_spray_hit(atk, color)
    }

    /// Poll and clear one-shot boss completion events emitted by active boss controllers.
    pub fn consume_boss_events(&mut self) -> AiBossEvents {
        let mut events = AiBossEvents::default();
        for enemy in &mut self.enemies {
            match &mut enemy.controller {
                ActiveEnemy::PyramidHead(e) if e.battle_end_complete => {
                    events.ph_battle_end_complete = true;
                }
                ActiveEnemy::FleshLips(e) if e.battle_cleared_event => {
                    events.flesh_lips_battle_cleared = true;
                    e.battle_cleared_event = false;
                }
                ActiveEnemy::AbstractDaddy(e) if e.boss_defeated_event => {
                    events.abstract_daddy_defeated = true;
                    e.boss_defeated_event = false;
                }
                ActiveEnemy::Eddie(e) => {
                    if e.room1_escaped_event {
                        events.eddie_room1_escaped = true;
                        e.room1_escaped_event = false;
                    }
                    if e.room2_dead_event {
                        events.eddie_room2_dead = true;
                        e.room2_dead_event = false;
                    }
                }
                ActiveEnemy::TwinPyramidHead(e) if e.suicide_cutscene_ready => {
                    events.twin_ph_suicide_ready = true;
                    e.suicide_cutscene_ready = false;
                }
                ActiveEnemy::MaryBoss(e) => {
                    if e.phase1_cleared_event {
                        events.mary_phase1_cleared = true;
                        e.phase1_cleared_event = false;
                    }
                    if e.ending_triggered_event {
                        events.mary_ending_triggered = true;
                        e.ending_triggered_event = false;
                    }
                }
                _ => {}
            }
        }
        events
    }
}
