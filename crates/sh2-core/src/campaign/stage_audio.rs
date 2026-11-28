//! Unified game-wide runtime stage ambient & horror SFX coordinator (`EvSoundCallAfterLoad` +
//! `EvAllTimeFunc` across all 58 retail stages).
//!
//! Pure simulation logic with zero `kira`, `sh2-audio`, `wgpu`, or `winit` dependencies
//! (`AGENTS.md` §1). Every SE ID, flag index, world coordinate (PS2 mm), and volume constant
//! is verified 1:1 against `sh2pc.exe` (`Stage_Data::sound_call_after_load_fn` & `all_time_fn`).

use crate::events::horror_triggers::{
    SpatialSoundCommand, StageEventDispatcher, StageHookContext, StageId,
};
use crate::events::{
    stage4_eval_sound_call_after_load, EventFlags, ForestAmbientState, Sh2Rng,
    STG4_FLAG_AMBIENT_MUTE, STG4_SE_AMBIENT_CROW, STG4_SE_AMBIENT_PASSER, STG4_SE_AMBIENT_RUSTLE,
    STG4_SE_FOREST_WIND,
};

use super::io::{CampaignEffects, StagePositionalSound};

// ============================================================================
// Verified Retail `EvSoundCallAfterLoad` Constants (`sh2pc.exe`)
// ============================================================================

/// PS2 `SeCallPos` flag `0x08` (`SE_FLAG_MANAGED_2D`: persistent looping managed ambient slot).
const SE_FLAG_MANAGED_2D: u32 = 0x08;
/// PS2 `SeCallPos` flag `0x0c` (`SE_FLAG_NO_DISTANCE | SE_FLAG_MANAGED_2D`).
const SE_FLAG_NO_DISTANCE_MANAGED_2D: u32 = 0x0c;
/// PS2 `SeCallPos` flag `0x02` (3D non-managed one-shot ambient emitter, e.g. Forest `0x9c86..0x9c88`).
const SE_FLAG_3D_ONESHOT: u32 = 0x02;

/// Stage 14 (`stg_town_west.c` `0x00593ac0`) ambient SE `0x4269` (`voice.afs[33]` / `sddata[359]`).
pub const SE_TOWN_WEST_AMBIENT: u32 = 0x4269;
/// Stage 14 (`0x00593acc` -> `0x008ddd28`) world anchor for `0x4269` in PS2 mm.
pub const TOWN_WEST_AMBIENT_POS_MM: [f32; 3] = [-70_640.0, -250.0, 61_850.0];
/// Stage 14 (`0x00593ac0`: `0x1f7a9f4 & 0x100`) flag `168` inhibiting `0x4269`.
pub const FLAG_TOWN_WEST_AMBIENT_MUTE: usize = 168;

/// Stage 22 (`stg_hospital_1fw_b.c` `0x0058be20`) & Stages 48–51 (`0x00440250`, `0x00440fd0`,
/// `0x0043f400`, `0x00574b70`) outdoor rain loop SE `0x4db8` (`voice.afs[13]`).
pub const SE_RAIN_AMBIENT: u32 = 0x4db8;
/// Stage 22 (`0x0058be25`) Hospital Otherworld Courtyard room ID (`0x49`).
pub const ROOM_HOSPITAL_1FW_B_COURTYARD: u16 = 0x49;
/// Stage 22 (`0x0058be2a`: `0x1f7a9f8 & 0x0100_0000`) flag `216` courtyard rain already heard.
pub const FLAG_HOSPITAL_COURTYARD_RAIN_SEEN: usize = 216;
/// Stages 48–51 (`0x00440250`: `0x1f7aa1c & 0x0200_0000`) flag `505` inhibiting ending rain loop.
pub const FLAG_ENDING_RAIN_MUTE: usize = 505;

/// Stage 34 (`stg_labyrinth_w.c` `0x00582d10`) Room `0x88` ambient SE `0x465a` (`voice.afs[22]`).
pub const SE_LAB_W_ROOM88_AMBIENT: u32 = 0x465a;
/// Stage 34 (`0x00582d1e` -> `0x008d16f8`) world anchor for `0x465a` in PS2 mm.
pub const LAB_W_ROOM88_AMBIENT_POS_MM: [f32; 3] = [20_000.0, -2_500.0, -100_000.0];
/// Stage 34 (`stg_labyrinth_w.c` `0x00582d36`) Room `0x87` ambient SE `0x42cb` (`sddata[416]`).
pub const SE_LAB_W_ROOM87_AMBIENT: u32 = 0x42cb;
/// Stage 34 (`0x00582d48` -> `0x008d1708`) world anchor for `0x42cb` in PS2 mm.
pub const LAB_W_ROOM87_AMBIENT_POS_MM: [f32; 3] = [19_656.1, -602.5, -25_420.5];
/// Stage 34 (`0x00582d3d`: `0x1f7aa08 & 0x80`) flag `327` enabling Room `0x87` SE `0x42cb`.
pub const FLAG_LAB_W_ROOM87_AMBIENT: usize = 327;

/// Stage 38 (`stg_boat.c` `0x0057e670`) Toluca Lake ambient loop SE `0x3cf0` (`voice.afs[30]`).
pub const SE_BOAT_LAKE_AMBIENT: u32 = 0x3cf0;

/// Stage 39 (`stg_hotel_1f_f.c` `0x0057d8e0`) Hotel 1F Rooms `0xa7`/`0xa9` ambient SE `0x44c6`
/// (`voice.afs[14]` / `sddata[350]`).
pub const SE_HOTEL_1F_AMBIENT: u32 = 0x44c6;
/// Stage 39 Room `0xa7` (`0x0057d8ee` -> `0x008cc6e8`) world anchor for `0x44c6` in PS2 mm.
pub const HOTEL_1F_ROOM_A7_AMBIENT_POS_MM: [f32; 3] = [-100_000.0, -500.0, -60_500.0];
/// Stage 39 Room `0xa9` (`0x0057d90f` -> `0x008cc6f8`) world anchor for `0x44c6` in PS2 mm.
pub const HOTEL_1F_ROOM_A9_AMBIENT_POS_MM: [f32; 3] = [-58_009.996, -500.0, -63_187.5];
/// Stage 39 Room `0xa9` (`0x0057d914`: `0x3e4ccccd` = `0.2f`) base volume for `0x44c6`.
pub const HOTEL_1F_ROOM_A9_AMBIENT_VOL: f32 = 0.2;

/// Unified runtime coordinator for `EvSoundCallAfterLoad` and `EvAllTimeFunc` stage ambient
/// & horror SFX across all 58 stages (`CampaignOrchestrator::stage_audio`).
#[derive(Debug, Clone, Default)]
pub struct StageAudioCoordinator {
    /// Currently initialized stage ID (`None` before first stage load or after reset).
    pub active_stage_id: Option<u16>,
    /// Currently initialized numeric room ID (`RoomName` / `room_id_from_stage_code`).
    pub active_room_id: Option<u16>,
    /// Persistent BSS state for Stage 4 (`stg_forest.c` `0x0059fcf0`).
    pub forest: ForestAmbientState,
}

impl StageAudioCoordinator {
    /// Creates a fresh coordinator with no active stage.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Resets tracked stage/room state (e.g. on New Game or returning to the Title Screen).
    pub fn reset(&mut self) {
        self.active_stage_id = None;
        self.active_room_id = None;
        self.forest = ForestAmbientState::default();
    }

    /// Evaluates `EvSoundCallAfterLoad` (`Stage_Data::sound_call_after_load_fn`) whenever a stage
    /// or room finishes loading. If transitioning from a previous stage/room, marks
    /// `effects.stop_all_stage_ambient_se = true` so previous looping 2D & 3D ambient SEs fade out.
    pub fn on_stage_or_room_enter(
        &mut self,
        stage_id: u16,
        room_id: u16,
        flags: &mut EventFlags,
        horror_triggers: &mut StageEventDispatcher,
        rng: &mut Sh2Rng,
        effects: &mut CampaignEffects,
    ) {
        let prev = (self.active_stage_id, self.active_room_id);
        let next = (Some(stage_id), Some(room_id));
        if prev != next && self.active_stage_id.is_some() {
            effects.stop_all_stage_ambient_se = true;
        }
        self.active_stage_id = Some(stage_id);
        self.active_room_id = Some(room_id);

        match stage_id {
            // Stage 4 (`stg_forest.c` `0x0059fc70..0x0059fce3`):
            // Wind loop `0x2742` (`pos = NULL`, initial vol `0.0`, modulated by `EvAllTime`) and
            // NG+ chainsaw idle loop `0x4268` at `STG4_CHAINSAW_SE_POS`.
            4 => {
                self.forest = ForestAmbientState::init_from_rng(rng);
                let (se_2d, se_pos) = stage4_eval_sound_call_after_load(flags);
                for se_id in se_2d {
                    let initial_vol = if se_id == STG4_SE_FOREST_WIND {
                        0.0
                    } else {
                        1.0
                    };
                    effects.looping_se_starts.push((se_id, initial_vol));
                }
                for (se_id, pos) in se_pos {
                    effects.stage_positional_sounds.push(StagePositionalSound {
                        se_id,
                        pos: Some(pos),
                        volume: 1.0,
                        flags: SE_FLAG_MANAGED_2D,
                    });
                }
            }
            // Stages 6..=13, 28, 31..=33 (`StageEventDispatcher::on_sound_call_after_load`):
            // ApartE1f `0x3e90` (`0x0059cfe0`), ApartE2f `0x3e80` (`0x0059ba80`), etc.
            6..=13 | 28 | 31..=33 => {
                if let Some(sid) = StageId::from_raw(stage_id as u8) {
                    let out = horror_triggers.on_sound_call_after_load(sid, room_id as u8, flags);
                    emit_spatial_sound_commands(&out.sounds, effects);
                }
            }
            // Stage 14 (`stg_town_west.c` `0x00593ac0`): `0x4269` at `0x008ddd28` when `!flag(168)`.
            14 if !flags.get(FLAG_TOWN_WEST_AMBIENT_MUTE) => {
                effects.stage_positional_sounds.push(StagePositionalSound {
                    se_id: SE_TOWN_WEST_AMBIENT,
                    pos: Some(TOWN_WEST_AMBIENT_POS_MM),
                    volume: 1.0,
                    flags: SE_FLAG_MANAGED_2D,
                });
            }
            // Stage 22 (`stg_hospital_1fw_b.c` `0x0058be20`): Room `0x49` courtyard rain `0x4db8`.
            22 if room_id == ROOM_HOSPITAL_1FW_B_COURTYARD => {
                let vol = if flags.get(FLAG_HOSPITAL_COURTYARD_RAIN_SEEN) {
                    1.0
                } else {
                    flags.set(FLAG_HOSPITAL_COURTYARD_RAIN_SEEN, true);
                    0.0
                };
                effects.looping_se_starts.push((SE_RAIN_AMBIENT, vol));
            }
            // Stage 34 (`stg_labyrinth_w.c` `0x00582d10`): Room `0x88` `0x465a` & Room `0x87` `0x42cb`.
            34 => {
                if room_id == 0x88 {
                    effects.stage_positional_sounds.push(StagePositionalSound {
                        se_id: SE_LAB_W_ROOM88_AMBIENT,
                        pos: Some(LAB_W_ROOM88_AMBIENT_POS_MM),
                        volume: 1.0,
                        flags: SE_FLAG_MANAGED_2D,
                    });
                } else if room_id == 0x87 && flags.get(FLAG_LAB_W_ROOM87_AMBIENT) {
                    effects.stage_positional_sounds.push(StagePositionalSound {
                        se_id: SE_LAB_W_ROOM87_AMBIENT,
                        pos: Some(LAB_W_ROOM87_AMBIENT_POS_MM),
                        volume: 1.0,
                        flags: SE_FLAG_MANAGED_2D,
                    });
                }
            }
            // Stage 38 (`stg_boat.c` `0x0057e670`): Toluca Lake ambient loop `0x3cf0` routed
            // through `SePoolManager::slots_2d` (`pos = None`, `flags = 0x0c`) so retail
            // `Se2dManager` (`FUN_00516b90` `0x00516b90` lines 5066–5082) applies the
            // `player_pos.z >= -10000.0` spatial override and pause/demo ducking.
            38 => {
                effects.stage_positional_sounds.push(StagePositionalSound {
                    se_id: SE_BOAT_LAKE_AMBIENT,
                    pos: None,
                    volume: 1.0,
                    flags: SE_FLAG_NO_DISTANCE_MANAGED_2D,
                });
            }
            // Stage 39 (`stg_hotel_1f_f.c` `0x0057d8e0`): Room `0xa7` & `0xa9` ambient `0x44c6`.
            39 => {
                if room_id == 0xa7 {
                    effects.stage_positional_sounds.push(StagePositionalSound {
                        se_id: SE_HOTEL_1F_AMBIENT,
                        pos: Some(HOTEL_1F_ROOM_A7_AMBIENT_POS_MM),
                        volume: 1.0,
                        flags: SE_FLAG_NO_DISTANCE_MANAGED_2D,
                    });
                } else if room_id == 0xa9 {
                    effects.stage_positional_sounds.push(StagePositionalSound {
                        se_id: SE_HOTEL_1F_AMBIENT,
                        pos: Some(HOTEL_1F_ROOM_A9_AMBIENT_POS_MM),
                        volume: HOTEL_1F_ROOM_A9_AMBIENT_VOL,
                        flags: SE_FLAG_MANAGED_2D,
                    });
                }
            }
            // Stages 48–51 (`0x00440250`, `0x00440fd0`, `0x0043f400`, `0x00574b70`):
            // Ending rain loop `0x4db8` when `!flag(505)`.
            48..=51 if !flags.get(FLAG_ENDING_RAIN_MUTE) => {
                effects.looping_se_starts.push((SE_RAIN_AMBIENT, 1.0));
            }
            _ => {}
        }
    }

    /// Evaluates per-frame `EvAllTimeFunc` ambient & horror SFX for the active stage and room.
    ///
    /// Automatically initializes the stage via [`Self::on_stage_or_room_enter`] if `(stage_id,
    /// room_id)` changed (e.g. direct test-room / stage-select entry into Stage 4 `ca11`).
    #[allow(clippy::too_many_arguments)]
    pub fn tick_all_time(
        &mut self,
        stage_id: u16,
        room_id: u16,
        player_pos: [f32; 3],
        player_yaw: f32,
        player_speed: f32,
        dt: f32,
        demo_active: bool,
        active_sounds: &[u32],
        flags: &mut EventFlags,
        horror_triggers: &mut StageEventDispatcher,
        rng: &mut Sh2Rng,
        effects: &mut CampaignEffects,
    ) {
        if self.active_stage_id != Some(stage_id) || self.active_room_id != Some(room_id) {
            self.on_stage_or_room_enter(stage_id, room_id, flags, horror_triggers, rng, effects);
        }

        if dt <= 0.0 {
            return;
        }

        match stage_id {
            // Stage 4 (`stg_forest.c` `0x0059fcf0..0x005a025a`):
            // Wind gust oscillator (`0x2742`), distant crow (`0x9c88`), behind-James rustle
            // (`0x9c86`), and distant passing footsteps (`0x9c87`).
            4 => {
                self.forest.sync_active_sounds(
                    active_sounds.contains(&STG4_SE_AMBIENT_CROW),
                    active_sounds.contains(&STG4_SE_AMBIENT_RUSTLE),
                    active_sounds.contains(&STG4_SE_AMBIENT_PASSER),
                );
                let out = self.forest.tick_all_time(
                    player_pos,
                    player_yaw,
                    player_speed,
                    dt,
                    demo_active,
                    flags,
                    rng,
                );
                if flags.get(STG4_FLAG_AMBIENT_MUTE) {
                    effects.stop_sound_effects.push(STG4_SE_FOREST_WIND);
                } else if let Some(vol) = out.wind_volume {
                    effects.looping_se_volumes.push((STG4_SE_FOREST_WIND, vol));
                }
                for (se_id, pos) in out.spawned_positional_se {
                    effects.stage_positional_sounds.push(StagePositionalSound {
                        se_id,
                        pos: Some(pos),
                        volume: 1.0,
                        flags: SE_FLAG_3D_ONESHOT,
                    });
                }
                for (se_id, pos) in out.updated_positional_se {
                    effects
                        .updated_positional_sounds
                        .push(StagePositionalSound {
                            se_id,
                            pos: Some(pos),
                            volume: 1.0,
                            flags: SE_FLAG_3D_ONESHOT,
                        });
                }
                effects.stop_sound_effects.extend(out.stopped_se);
            }
            // Stages 6..=13, 28, 31..=33 (`StageEventDispatcher::on_all_time`):
            // Apartments Room 209 whisper (`0x9c70`), Room 205 TV (`0x3e80`), 3F footsteps
            // (`0x9c51`), Staircase drain (`0x2716`), Historical Society (`0x9c73`, `0x4982`),
            // Toluca Prison (`0x427d`, `0x4682`).
            6..=13 | 28 | 31..=33 => {
                if let Some(sid) = StageId::from_raw(stage_id as u8) {
                    let mut active_arr = [0u32; 4];
                    for (dst, &src) in active_arr.iter_mut().zip(active_sounds.iter()) {
                        *dst = src;
                    }
                    let ctx = StageHookContext {
                        stage: sid,
                        room_id: room_id as u8,
                        dt,
                        player_pos,
                        player_rot_y: player_yaw,
                        active_sounds: active_arr,
                    };
                    let out = horror_triggers.on_all_time(&ctx, flags, rng);
                    emit_spatial_sound_commands(&out.sounds, effects);
                }
            }
            _ => {}
        }
    }
}

/// Translates [`SpatialSoundCommand`]s from [`StageEventDispatcher`] into [`CampaignEffects`].
fn emit_spatial_sound_commands(commands: &[SpatialSoundCommand], effects: &mut CampaignEffects) {
    for &cmd in commands {
        match cmd {
            SpatialSoundCommand::CallPos {
                se_id,
                volume,
                pos,
                channel,
            } => {
                effects.stage_positional_sounds.push(StagePositionalSound {
                    se_id,
                    pos: Some(pos),
                    volume,
                    flags: u32::from(channel),
                });
            }
            SpatialSoundCommand::PosMove { se_id, volume, pos } => {
                effects
                    .updated_positional_sounds
                    .push(StagePositionalSound {
                        se_id,
                        pos: Some(pos),
                        volume,
                        flags: 0,
                    });
            }
            SpatialSoundCommand::PosSet {
                se_id,
                volume,
                pos,
                channel,
            } => {
                effects
                    .updated_positional_sounds
                    .push(StagePositionalSound {
                        se_id,
                        pos: Some(pos),
                        volume,
                        flags: u32::from(channel),
                    });
            }
            SpatialSoundCommand::SetVolume { se_id, volume } => {
                effects.looping_se_volumes.push((se_id, volume));
            }
            SpatialSoundCommand::Call2D { se_id, .. } => {
                effects.sound_effects.push(se_id);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::campaign::{CampaignOrchestrator, CampaignPhase, CampaignTickInput};
    use crate::config::OutdoorStreamingMode;
    use crate::events::horror_triggers::{
        FLAG_APART_ROOM209_WHISPER_DONE, FLAG_PRISON_BATHROOM_JUMPSCARE_DONE,
        FLAG_PRISON_BATHROOM_KNOCKED, SE_APART_ROOM205_TV_STATIC, SE_APART_ROOM209_WHISPER,
        SE_PRISON_BATHROOM_STALL_BANG, SE_PRISON_INVISIBLE_PRISONER, SE_SOCIETY_STAIRCASE_AMBIENT,
    };
    use crate::events::{STG4_FLAG_NG_PLUS, STG4_SE_CHAINSAW_IDLE};
    use crate::simulation::SimulationCore;
    use glam::Vec3;

    fn advance_connect_to_gameplay(
        orch: &mut CampaignOrchestrator,
        sim: &mut SimulationCore,
    ) -> Vec<CampaignEffects> {
        let mut all = Vec::new();
        for _ in 0..24 {
            let eff = orch.step(sim, &CampaignTickInput::default(), 1.0 / 60.0);
            if eff.load_room.is_some() {
                orch.notify_room_assets_loaded();
            }
            all.push(eff);
            if matches!(orch.phase, CampaignPhase::ActiveGameplay) {
                break;
            }
        }
        assert!(
            matches!(orch.phase, CampaignPhase::ActiveGameplay),
            "expected ActiveGameplay, got {:?}",
            orch.phase
        );
        all
    }

    #[test]
    fn stage4_forest_wind_crow_rustle_passer_and_stage_exit_via_orchestrator() {
        let mut sim = SimulationCore::default();
        sim.scenario.flags.set(STG4_FLAG_NG_PLUS, true);
        let mut orch = CampaignOrchestrator::from_saved_stage(
            4,
            Some("ca11".to_string()),
            [1000.0, 0.0, -40_000.0],
            -1.5,
            OutdoorStreamingMode::default(),
            false,
        );

        let load_effects = advance_connect_to_gameplay(&mut orch, &mut sim);
        // `EvSoundCallAfterLoad` (`0x0059fc70`) must start looping wind `0x2742` and NG+ chainsaw `0x4268`.
        assert!(
            load_effects.iter().any(|e| e
                .looping_se_starts
                .iter()
                .any(|&(id, _)| id == STG4_SE_FOREST_WIND)),
            "Forest Trail must start looping wind 0x2742 on load"
        );
        assert!(
            load_effects.iter().any(|e| e
                .stage_positional_sounds
                .iter()
                .any(|p| p.se_id == STG4_SE_CHAINSAW_IDLE)),
            "Forest Trail NG+ must start chainsaw idle 0x4268 on load"
        );

        // 1. Live wind volume modulation + distant crow `0x9c88` spawn (`x > 0, -90000 < z < -30000`).
        orch.stage_audio.forest.crow_timer = 0.01;
        let eff = orch.step(&mut sim, &CampaignTickInput::default(), 1.0 / 60.0);
        assert!(
            eff.looping_se_volumes
                .iter()
                .any(|&(id, vol)| id == STG4_SE_FOREST_WIND && vol > 0.0),
            "Forest wind 0x2742 volume must modulate every tick"
        );
        assert!(
            eff.stage_positional_sounds
                .iter()
                .any(|p| p.se_id == STG4_SE_AMBIENT_CROW),
            "Distant crow 0x9c88 must spawn in (-90000 < z < -30000, x > 0)"
        );
        assert!(orch.stage_audio.forest.crow_playing);

        // 2. When `active_stage_se_ids` no longer reports `0x9c88`, `crow_playing` must clear (`SeCheck`).
        let _ = orch.step(&mut sim, &CampaignTickInput::default(), 1.0 / 60.0);
        assert!(
            !orch.stage_audio.forest.crow_playing,
            "crow_playing must clear once 0x9c88 finishes playing in se_pool"
        );

        // 3. Behind-James rustle `0x9c86`: spawns when `x < -20000` & yaw in `(-3π/4, -π/4)`,
        //    updates while moving (`speed >= 1.2`), and stops immediately when James stops (`speed < 1.2`).
        sim.snap_player_transform(Vec3::new(-30_000.0, 0.0, -40_000.0), -1.5);
        sim.locomotion.speed_mps = 2.0;
        orch.stage_audio.forest.rustle_timer = 0.01;
        let rustle_spawn = orch.step(&mut sim, &CampaignTickInput::default(), 1.0 / 60.0);
        assert!(
            rustle_spawn
                .stage_positional_sounds
                .iter()
                .any(|p| p.se_id == STG4_SE_AMBIENT_RUSTLE),
            "Behind-James rustle 0x9c86 must spawn"
        );
        assert!(
            rustle_spawn
                .updated_positional_sounds
                .iter()
                .any(|p| p.se_id == STG4_SE_AMBIENT_RUSTLE),
            "Behind-James rustle 0x9c86 must update position while moving"
        );

        // Stop moving while `0x9c86` is reported active -> must emit `stop_sound_effects` for `0x9c86`.
        sim.locomotion.speed_mps = 0.0;
        let mut stop_input = CampaignTickInput::default();
        stop_input.active_stage_se_ids.push(STG4_SE_AMBIENT_RUSTLE);
        let rustle_stop = orch.step(&mut sim, &stop_input, 1.0 / 60.0);
        assert!(
            rustle_stop
                .stop_sound_effects
                .contains(&STG4_SE_AMBIENT_RUSTLE),
            "Behind-James rustle 0x9c86 must stop immediately when speed < 1.2"
        );

        // 4. Distant passing footsteps `0x9c87` (`x > 0, z < -25000`).
        sim.snap_player_transform(Vec3::new(1000.0, 0.0, -40_000.0), -1.5);
        orch.stage_audio.forest.passer_timer = 0.01;
        let passer_eff = orch.step(&mut sim, &CampaignTickInput::default(), 1.0 / 60.0);
        assert!(
            passer_eff
                .stage_positional_sounds
                .iter()
                .any(|p| p.se_id == STG4_SE_AMBIENT_PASSER),
            "Distant passing footsteps 0x9c87 must spawn"
        );
        assert!(
            passer_eff
                .updated_positional_sounds
                .iter()
                .any(|p| p.se_id == STG4_SE_AMBIENT_PASSER),
            "Distant passing footsteps 0x9c87 must sweep X via updated_positional_sounds"
        );

        // 5. Transitioning from Stage 4 to Stage 5 (`TownEast`) must emit `stop_all_stage_ambient_se`.
        let mut exit_effects = CampaignEffects::default();
        orch.stage_audio.on_stage_or_room_enter(
            5,
            0x06,
            &mut sim.scenario.flags,
            &mut orch.horror_triggers,
            &mut orch.rng,
            &mut exit_effects,
        );
        assert!(
            exit_effects.stop_all_stage_ambient_se,
            "Leaving Stage 4 for Stage 5 must stop all previous looping/spatial ambient SEs"
        );
    }

    #[test]
    fn apartments_society_prison_and_gamewide_stages_trigger_cleanly() {
        let mut coord = StageAudioCoordinator::new();
        let mut horror = StageEventDispatcher::new();
        let mut flags = EventFlags::new();
        let mut rng = Sh2Rng::new(0x1234_5678);

        // 1. Wood Side Apartments E 2F Room `0x18` (Room 205/208 TV static `0x3E80` when `flag(69)`).
        flags.set(69, true);
        let mut eff = CampaignEffects::default();
        coord.on_stage_or_room_enter(7, 0x18, &mut flags, &mut horror, &mut rng, &mut eff);
        assert!(
            eff.stage_positional_sounds
                .iter()
                .any(|p| p.se_id == SE_APART_ROOM205_TV_STATIC && (p.volume - 0.3).abs() < 1e-4),
            "ApartE2f Room 0x18 must start TV static 0x3E80 at vol 0.3"
        );
        assert!(flags.get(70));

        // 2. Blue Creek Apartments W 2F Room `0x28` (Room 209 orbiting whisper `0x9C70`).
        flags.set(FLAG_APART_ROOM209_WHISPER_DONE, false);
        horror.on_room_init(StageId::ApartW2f, 0x28, &mut flags, &mut rng);
        let mut eff = CampaignEffects::default();
        coord.tick_all_time(
            11,
            0x28,
            [19_000.0, 0.0, 0.0],
            0.0,
            0.0,
            1.0 / 60.0,
            false,
            &[],
            &mut flags,
            &mut horror,
            &mut rng,
            &mut eff,
        );
        assert!(
            eff.stop_all_stage_ambient_se,
            "Stage change stops previous TV static"
        );
        assert!(
            eff.stage_positional_sounds
                .iter()
                .any(|p| p.se_id == SE_APART_ROOM209_WHISPER),
            "Room 209 whisper 0x9C70 must spawn"
        );
        assert!(
            eff.updated_positional_sounds
                .iter()
                .any(|p| p.se_id == SE_APART_ROOM209_WHISPER),
            "Room 209 whisper 0x9C70 must immediately orbit via updated_positional_sounds"
        );

        // 3. Historical Society Room `0x5d` descending staircase ambient `0x9C73`.
        horror.society_staircase_timer = 0.01;
        let mut eff = CampaignEffects::default();
        coord.tick_all_time(
            28,
            0x5d,
            [-11_520.0, 20_000.0, 40_151.7],
            0.0,
            0.0,
            0.05,
            false,
            &[],
            &mut flags,
            &mut horror,
            &mut rng,
            &mut eff,
        );
        assert!(
            eff.stage_positional_sounds
                .iter()
                .any(|p| p.se_id == SE_SOCIETY_STAIRCASE_AMBIENT),
            "Historical Society Room 0x5d must trigger 0x9C73"
        );

        // 4. Toluca Prison North Room `0x73` bathroom stall bang `0x427D`.
        flags.set(FLAG_PRISON_BATHROOM_KNOCKED, true);
        flags.set(FLAG_PRISON_BATHROOM_JUMPSCARE_DONE, false);
        horror.prison_n_bathroom_z_threshold = -20_000.0;
        let mut fired_bang = false;
        for _ in 0..16 {
            flags.set(FLAG_PRISON_BATHROOM_KNOCKED, true);
            let mut eff = CampaignEffects::default();
            coord.tick_all_time(
                31,
                0x73,
                [-138_700.0, 0.0, -19_000.0],
                0.0,
                0.0,
                1.0 / 60.0,
                false,
                &[],
                &mut flags,
                &mut horror,
                &mut rng,
                &mut eff,
            );
            if eff
                .stage_positional_sounds
                .iter()
                .any(|p| p.se_id == SE_PRISON_BATHROOM_STALL_BANG)
            {
                fired_bang = true;
                break;
            }
        }
        assert!(
            fired_bang,
            "Toluca Prison North Room 0x73 must trigger 0x427D"
        );

        // 5. Toluca Prison South Room `0x72` invisible prisoner `0x4682`.
        horror.prison_s_prisoner_timer = 0.01;
        let mut eff = CampaignEffects::default();
        coord.tick_all_time(
            32,
            0x72,
            [-140_000.0, 0.0, 20_000.0],
            0.0,
            0.0,
            0.05,
            false,
            &[],
            &mut flags,
            &mut horror,
            &mut rng,
            &mut eff,
        );
        assert!(
            eff.stage_positional_sounds
                .iter()
                .any(|p| p.se_id == SE_PRISON_INVISIBLE_PRISONER),
            "Toluca Prison South Room 0x72 must trigger 0x4682"
        );

        // 6. Audited Act 1/Act 2 `EvSoundCallAfterLoad` stages: TownWest `0x4269`, Boat `0x3CF0`,
        //    Hotel1fF `0x44C6`, EndLeave `0x4DB8`.
        let mut eff = CampaignEffects::default();
        coord.on_stage_or_room_enter(14, 0, &mut flags, &mut horror, &mut rng, &mut eff);
        assert!(eff
            .stage_positional_sounds
            .iter()
            .any(|p| p.se_id == SE_TOWN_WEST_AMBIENT));

        let mut eff = CampaignEffects::default();
        coord.on_stage_or_room_enter(38, 0, &mut flags, &mut horror, &mut rng, &mut eff);
        assert!(eff.stage_positional_sounds.iter().any(|p| {
            p.se_id == SE_BOAT_LAKE_AMBIENT
                && p.pos.is_none()
                && p.flags == SE_FLAG_NO_DISTANCE_MANAGED_2D
        }));

        let mut eff = CampaignEffects::default();
        coord.on_stage_or_room_enter(39, 0xa7, &mut flags, &mut horror, &mut rng, &mut eff);
        assert!(eff
            .stage_positional_sounds
            .iter()
            .any(|p| p.se_id == SE_HOTEL_1F_AMBIENT));

        let mut eff = CampaignEffects::default();
        coord.on_stage_or_room_enter(48, 0, &mut flags, &mut horror, &mut rng, &mut eff);
        assert!(eff
            .looping_se_starts
            .iter()
            .any(|&(id, _)| id == SE_RAIN_AMBIENT));
    }
}
