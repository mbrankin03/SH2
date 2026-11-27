//! High-level Enemy AI subsystem orchestrating all creature instances, sensory perception,
//! radio noise modulation, and the Maria companion system (1:1 PS2/PC parity).
//!
//! # Ground Truth:
//! - PS2 prototype unstripped: `src/Enemy/en_common.c`, `src/Chacter/m3_maria_sub.c`
//! - PC executable `sh2pc.exe`:
//!   - `0x4a88e0`: `enExecTask` - global enemy update loop across active slots.
//!   - `0x4aa5e0`: `enInitData` - spawn management.
//!   - `0x542ad0`: Companion targeting arbitration.

pub mod active_enemy;
pub mod boss_events;
pub mod creatures;
pub mod enemy_actor;
pub mod maria;
pub mod npcs;
pub mod pathfinding;
pub mod radio;
pub mod sensory;
pub mod spawn;

pub use active_enemy::{ActiveEnemy, EnemyUpdateContext};
pub use boss_events::AiBossEvents;
pub use enemy_actor::EnemyActor;
pub use spawn::EnemySpawnVariant;

use crate::ai::creatures::creeper::TyuState;
use crate::ai::creatures::flesh_lips::IkeState;
use crate::ai::creatures::insects::InsectState;
use crate::ai::creatures::lying_figure::ScuState;
use crate::ai::creatures::mannequin::MknState;
use crate::ai::creatures::twin_pyramid_head::TwinPartnerSnapshot;
use crate::ai::creatures::{
    en_check_sleep_in, en_check_sleep_out, EnemySpecies, StagePlace, TribeCommunicationBus,
};
use crate::ai::maria::MariaCompanionController;
use crate::ai::pathfinding::{en_room_forbidden_area, RoomForbiddenAreas};
use crate::ai::radio::{
    en_kind_from_species, RadioEnemySample, RadioNoiseEvaluator, RadioStaticOutput,
};
use crate::ai::sensory::{DifficultyLevel, PerceivableTarget, RoomLighting};
use glam::Vec3;

/// Global AI and Companion subsystem (`en_common.c` / `m3_maria_sub.c`).
#[derive(Debug, Clone)]
pub struct AiSubsystem {
    /// Active enemies currently spawned in the stage/room (max 24-32 slots).
    pub enemies: Vec<EnemyActor>,
    /// Maria companion controller.
    pub maria: MariaCompanionController,
    /// Radio static evaluator.
    pub radio_evaluator: RadioNoiseEvaluator,
    /// Latest radio output state.
    pub radio_output: RadioStaticOutput,
    /// Active camera position (`vcWork.cam_pos` `0x010e5b40`) fed into `RadioNoise` (`0x001ab300`).
    pub radio_camera_pos: Option<Vec3>,
    /// Active PS2 `VbWvsMatrix` (`0x010e59d0` / `0x01f80e00`) fed into `RadioNoise` (`0x001ab300`).
    pub radio_wvs_matrix: Option<glam::Mat4>,
    /// Current room lighting condition (`enGetWorldCondition` `0x00141ca0`).
    pub lighting: RoomLighting,
    /// Game difficulty setting (`enGetMode()` `0..=3`).
    pub difficulty: DifficultyLevel,
    /// Stage progression tier for `enSetHP` (`0x001420c0`) multipliers.
    pub stage_place: StagePlace,
    /// True when the active stage is outdoor (`BgIsOut()`, controls sleep culling & acid fog reach).
    pub is_outdoor: bool,
    /// 8-slot tribe alert communication bus (`enSetCommunication` / `enCommunicateTribe`).
    pub tribe_bus: TribeCommunicationBus,
    /// Active room forbidden areas & flags (`enRoomForbiddenArea` `0x00147000` / `sh2pc 0x004a84a0`).
    pub forbidden_areas: RoomForbiddenAreas,
    /// True when the player has 0 firearm ammo (`enCheckPlayerBulletEmpty` for Mary Boss pity timer).
    pub player_ammo_empty: bool,
    /// True while the player is actively firing Hyper Spray (`enCheckPlayerSprayNow` for Moth Swarm escape).
    pub player_spraying: bool,
    /// Player collision radius in mm (`enGetPlayerSize()`, `200.0` mm for James).
    pub player_radius_mm: f32,
    /// Deterministic LCG state (`shRandI` equivalent for `TwinPyramidHead` spin-loop rolls).
    rng_state: u32,
    /// Accumulated phase for `MaryBoss` hover sway (`shSway1f(-250.0, 0.1)` `enBOSMoveExec` `0x00140df0`).
    sway_phase: f32,
    /// Next synthetic ID assigned to dynamically spawned Mary moth swarm insects.
    next_dynamic_id: u32,
    /// Reusable scratch buffers to keep `update_filtered` 100% zero-allocation per tick.
    scratch_hostile_positions: Vec<Vec3>,
    scratch_radio_samples: Vec<RadioEnemySample>,
    scratch_twin_snapshots: Vec<(u32, TwinPartnerSnapshot)>,
    scratch_flesh_lips_info: Vec<(u32, bool, bool)>,
    scratch_mandarin_positions: Vec<(u32, Vec3)>,
}

impl Default for AiSubsystem {
    fn default() -> Self {
        Self {
            enemies: Vec::with_capacity(32),
            maria: MariaCompanionController::default(),
            radio_evaluator: RadioNoiseEvaluator::default(),
            radio_output: RadioStaticOutput::default(),
            radio_camera_pos: None,
            radio_wvs_matrix: None,
            lighting: RoomLighting::Lit,
            difficulty: DifficultyLevel::Normal,
            stage_place: StagePlace::Default,
            is_outdoor: false,
            tribe_bus: TribeCommunicationBus::default(),
            forbidden_areas: RoomForbiddenAreas::default(),
            player_ammo_empty: false,
            player_spraying: false,
            player_radius_mm: 200.0,
            rng_state: 0x1999_0901,
            sway_phase: 0.0,
            next_dynamic_id: 10_000,
            scratch_hostile_positions: Vec::with_capacity(32),
            scratch_radio_samples: Vec::with_capacity(32),
            scratch_twin_snapshots: Vec::with_capacity(4),
            scratch_flesh_lips_info: Vec::with_capacity(8),
            scratch_mandarin_positions: Vec::with_capacity(8),
        }
    }
}

impl AiSubsystem {
    /// Load active room forbidden rectangles and room flags for `(stage_id, room_id)`
    /// (`enRoomForbiddenArea` `SLUS_202.28` `0x00147000` / `sh2pc.exe` `0x004a84a0`).
    pub fn set_room_forbidden_areas(&mut self, stage_id: u32, room_id: u32, retail_pc: bool) {
        self.forbidden_areas = en_room_forbidden_area(stage_id, room_id, retail_pc);
    }

    /// Primary 60 Hz AI simulation update loop (`enExecTask` `SLUS_202.28` `0x001413f0`).
    pub fn update(
        &mut self,
        player_pos: Vec3,
        player_yaw: f32,
        is_flashlight_on: bool,
        is_running: bool,
        radio_enabled: bool,
        dt: f32,
    ) {
        self.update_filtered(
            player_pos,
            player_yaw,
            is_flashlight_on,
            is_running,
            radio_enabled,
            None,
            dt,
        );
    }

    /// Update AI simulation, optionally isolating execution to a single enemy index
    /// (used by the Combat Arena single-target test mode).
    #[allow(clippy::too_many_arguments)]
    pub fn update_filtered(
        &mut self,
        player_pos: Vec3,
        player_yaw: f32,
        is_flashlight_on: bool,
        is_running: bool,
        radio_enabled: bool,
        only_enemy_idx: Option<usize>,
        dt: f32,
    ) {
        self.update_filtered_with_collision(
            player_pos,
            player_yaw,
            is_flashlight_on,
            is_running,
            radio_enabled,
            only_enemy_idx,
            None,
            dt,
        );
    }

    /// Update AI simulation with optional `.cld` [`crate::collision::CollisionWorld`] floor probing
    /// (for Mandarin `enCheckFloor` grating checks) and cross-entity loopbacks.
    #[allow(clippy::too_many_arguments)]
    pub fn update_filtered_with_collision(
        &mut self,
        player_pos: Vec3,
        player_yaw: f32,
        is_flashlight_on: bool,
        is_running: bool,
        radio_enabled: bool,
        only_enemy_idx: Option<usize>,
        collision: Option<&crate::collision::CollisionWorld>,
        dt: f32,
    ) {
        // Collect active enemy positions for radio and companion awareness (zero-alloc scratch)
        self.scratch_hostile_positions.clear();
        for e in &self.enemies {
            if !e.is_dead() {
                self.scratch_hostile_positions.push(e.position());
            }
        }

        // 1. Update Maria companion
        self.maria
            .update(player_pos, player_yaw, &self.scratch_hostile_positions, dt);

        // 2. Target arbitration and enemy updates (`0x542ad0`)
        let player_target =
            PerceivableTarget::new(player_pos, player_yaw, is_flashlight_on, is_running);

        let maria_target =
            PerceivableTarget::new(self.maria.position, self.maria.yaw, false, false);

        // Pre-capture cross-entity snapshots into reusable scratch buffers (zero heap allocation).
        self.scratch_twin_snapshots.clear();
        self.scratch_flesh_lips_info.clear();
        self.scratch_mandarin_positions.clear();
        let mut insect_leader: Option<(u32, InsectState, Vec3)> = None;
        let mut has_alive_insects = false;

        for e in &self.enemies {
            match &e.controller {
                ActiveEnemy::TwinPyramidHead(t) => {
                    self.scratch_twin_snapshots
                        .push((t.id, t.snapshot(player_pos)));
                }
                ActiveEnemy::FleshLips(f) => {
                    self.scratch_flesh_lips_info.push((
                        f.id,
                        f.stats.health > 0.0,
                        matches!(f.state, IkeState::Attack | IkeState::Attack2),
                    ));
                }
                ActiveEnemy::Mandarin(m) if m.stats.health > 0.0 => {
                    self.scratch_mandarin_positions.push((m.id, m.position));
                }
                ActiveEnemy::RoomInsect(i) => {
                    if i.stats.health > 0.0 {
                        has_alive_insects = true;
                    }
                    if insect_leader.is_none()
                        && matches!(
                            i.state,
                            InsectState::Approach | InsectState::Attack | InsectState::Escape
                        )
                    {
                        insect_leader = Some((i.id, i.state, i.position));
                    }
                }
                _ => {}
            }
        }

        // Advance Mary hover sway (`shSway1f(-250.0, 0.1)` `enBOSMoveExec` `0x00140df0`).
        self.sway_phase += dt * 60.0 * 0.1;
        let mary_sway_y_sample = self.sway_phase.sin() * 250.0;
        let path_sway_sample = (self.sway_phase.sin() * 0.5).clamp(-0.5, 0.5);
        let has_collision_floors = collision.is_some_and(|c| c.all_floors().next().is_some());
        let walls = collision.map_or(&[][..], |c| c.walls.as_slice());

        // Update bit 0 of room forbidden-area flags (`enExecTask` `SLUS 0x001413f0`):
        // true when player is standing inside an active forbidden rectangle.
        self.forbidden_areas.flags.player_in_forbidden = self
            .forbidden_areas
            .rects
            .iter()
            .any(|r| r.contains_xz(player_pos));

        let mut mary_swarm_spawn: Option<(Vec3, f32)> = None;
        let mut insects_despawn_all = false;
        let mut pending_tribe_posts: Vec<(f32, u32, u32, Vec3, i32)> = Vec::new();

        let twin_snapshots = &self.scratch_twin_snapshots;
        let flesh_lips_info = &self.scratch_flesh_lips_info;
        let mandarin_positions = &self.scratch_mandarin_positions;

        for idx in 0..self.enemies.len() {
            if only_enemy_idx.is_some_and(|only| only != idx) {
                continue;
            }

            self.rng_state = self
                .rng_state
                .wrapping_mul(1_664_525)
                .wrapping_add(1_013_904_223);
            let rng_bits = self.rng_state;
            let enemy = &mut self.enemies[idx];
            enemy.record_prev_pose();

            // Outdoor sleep culling (`enCheckSleepIn` 15000mm / `enCheckSleepOut` 7500mm)
            let dist_to_player = (enemy.position() - player_pos).length();
            if enemy.is_sleeping {
                if en_check_sleep_out(dist_to_player, self.is_outdoor, true) {
                    enemy.is_sleeping = false;
                } else {
                    continue;
                }
            } else if en_check_sleep_in(dist_to_player, self.is_outdoor, enemy.is_dead(), true) {
                enemy.is_sleeping = true;
                continue;
            }

            // Drive loopback fields from EnemyActor animation & grapple state (`*(i32*)(work+0x14) == -1`, `enCheckHuggedPlayer`).
            let clip_finished = enemy.anim().is_clip_finished();
            let is_grappling = enemy.is_grappling_player;
            match &mut enemy.controller {
                ActiveEnemy::Mannequin(e) => {
                    e.anim_finished |= clip_finished;
                }
                ActiveEnemy::PyramidHead(e) => {
                    e.anim_finished |= clip_finished;
                    e.is_hugging_player = is_grappling;
                }
                ActiveEnemy::AbstractDaddy(e) => {
                    e.anim_finished |= clip_finished;
                    e.is_hugging_player = is_grappling;
                }
                ActiveEnemy::FleshLips(e) => {
                    e.anim_finished |= clip_finished;
                    e.is_hugging_player = is_grappling;
                }
                ActiveEnemy::Mandarin(e) => {
                    e.anim_finished |= clip_finished;
                }
                ActiveEnemy::Eddie(e) => {
                    e.anim_finished |= clip_finished;
                }
                ActiveEnemy::TwinPyramidHead(e) => {
                    e.anim_finished |= clip_finished;
                    e.spin_loop_rand_bits = rng_bits;
                }
                ActiveEnemy::MaryBoss(e) => {
                    e.anim_finished |= clip_finished;
                    e.is_hugging_player = is_grappling;
                }
                _ => {}
            }

            // While grappling James, `GrappleSession` owns enemy anchoring & pose.
            if enemy.is_grappling_player {
                enemy.tick_fallback_anim(dt);
                continue;
            }

            let target = if self
                .maria
                .should_enemy_target_maria(enemy.position(), player_pos)
            {
                &maria_target
            } else {
                &player_target
            };

            let tribe_id = match &enemy.controller {
                ActiveEnemy::LyingFigure(_) => Some(1),
                ActiveEnemy::Mannequin(_) => Some(2),
                ActiveEnemy::Creeper(_) => Some(3),
                _ => None,
            };
            let tribe_alert_pos = tribe_id
                .and_then(|tid| self.tribe_bus.query_nearest(tid, enemy.position()))
                .map(|a| a.position);

            let prev_scu_state = match &enemy.controller {
                ActiveEnemy::LyingFigure(e) => Some(e.state),
                _ => None,
            };
            let prev_mkn_state = match &enemy.controller {
                ActiveEnemy::Mannequin(e) => Some(e.state),
                _ => None,
            };
            let prev_tyu_state = match &enemy.controller {
                ActiveEnemy::Creeper(e) => Some(e.state),
                _ => None,
            };

            let mut ctx = EnemyUpdateContext {
                player_radius_mm: self.player_radius_mm,
                player_ammo_empty: self.player_ammo_empty,
                player_spraying: self.player_spraying,
                mary_sway_y_sample,
                walls,
                forbidden_areas: (!self.forbidden_areas.rects.is_empty())
                    .then_some(&self.forbidden_areas),
                tribe_alert_pos,
                path_sway_sample,
                ..EnemyUpdateContext::default()
            };

            match &enemy.controller {
                ActiveEnemy::TwinPyramidHead(t) => {
                    ctx.twin_partner = twin_snapshots
                        .iter()
                        .find(|(id, _)| *id != t.id)
                        .map(|(_, snap)| *snap);
                }
                ActiveEnemy::FleshLips(f) => {
                    ctx.other_flesh_lips_alive = flesh_lips_info
                        .iter()
                        .filter(|(id, alive, _)| *id != f.id && *alive)
                        .count();
                    ctx.other_flesh_lips_attacking = flesh_lips_info
                        .iter()
                        .any(|(id, _, attacking)| *id != f.id && *attacking);
                }
                ActiveEnemy::Mandarin(m) => {
                    ctx.mandarin_near_other_turn = mandarin_positions
                        .iter()
                        .filter(|(id, _)| *id != m.id)
                        .find_map(|(_, pos)| m.check_near_other(*pos));
                    if has_collision_floors {
                        if let Some(world) = collision {
                            let probe = m.forward_grating_probe_pos();
                            let hit = world
                                .sample_floor_surface(probe.x, probe.z, probe.y, 1000.0, 2000.0);
                            // `enCheckFloor` (`0x00145a80`): returns `2` for material `9` or `0xb`, `0` for none, `1` for solid.
                            ctx.mandarin_over_grating =
                                hit.is_none_or(|h| matches!(h.raw_material & 0x0F, 9 | 11));
                        }
                    }
                }
                ActiveEnemy::RoomInsect(i) => {
                    ctx.insect_leader = insect_leader
                        .filter(|(id, _, _)| *id != i.id)
                        .map(|(_, st, pos)| (st, pos));
                }
                _ => {}
            }

            enemy
                .controller
                .update_with_context(target, self.lighting, self.difficulty, ctx, dt);
            enemy.step_hitback();
            enemy.tick_fallback_anim(dt);

            // Post tribe communication alerts on aggro/wake transitions
            // (`enSetCommunication` `SLUS 0x00146d80`: SCU `en_scu.c:572` 4000mm/120t,
            // MKN `en_mkn.c:344,382` 1000mm/2t, TYU `en_tyu.c:821` 5000mm/300t).
            match &enemy.controller {
                ActiveEnemy::LyingFigure(e)
                    if e.state == ScuState::Chase && prev_scu_state != Some(ScuState::Chase) =>
                {
                    pending_tribe_posts.push((4000.0, 1, 1, e.position, 120));
                }
                ActiveEnemy::Mannequin(e)
                    if prev_mkn_state == Some(MknState::Stay) && e.state != MknState::Stay =>
                {
                    pending_tribe_posts.push((1000.0, 2, 1, e.position, 2));
                }
                ActiveEnemy::Creeper(e)
                    if e.state == TyuState::Attack && prev_tyu_state != Some(TyuState::Attack) =>
                {
                    pending_tribe_posts.push((5000.0, 3, 1, e.position, 300));
                }
                ActiveEnemy::MaryBoss(m)
                    if m.moth_swarm_active && !has_alive_insects && mary_swarm_spawn.is_none() =>
                {
                    let to_player = player_pos - m.position;
                    let yaw = to_player.x.atan2(to_player.z);
                    mary_swarm_spawn = Some((m.position, yaw));
                }
                ActiveEnemy::RoomInsect(i) if i.despawn_all => {
                    insects_despawn_all = true;
                }
                _ => {}
            }
        }

        // Decay existing tribe alert TTLs (`enExecTask` `0x001413f0` lines 272–282), then post
        // newly emitted alerts so same-frame listeners can observe them on the next tick.
        self.tribe_bus.tick();
        for (radius, tribe_id, msg_type, pos, ttl) in pending_tribe_posts {
            self.tribe_bus.post(radius, tribe_id, msg_type, pos, ttl);
        }

        if insects_despawn_all {
            self.enemies
                .retain(|e| !matches!(e.controller, ActiveEnemy::RoomInsect(_)));
            for e in &mut self.enemies {
                if let ActiveEnemy::MaryBoss(m) = &mut e.controller {
                    m.moth_swarm_active = false;
                }
            }
        } else if let Some((spawn_pos, spawn_yaw)) = mary_swarm_spawn {
            // `enBOSCtrlGenerate` (`0x00140090`): spawn RoomInsect swarm (leader + followers up to 32 slots).
            let available_slots = 32usize.saturating_sub(self.enemies.len());
            let spawn_count = available_slots.min(16);
            for _ in 0..spawn_count {
                let id = self.next_dynamic_id;
                self.next_dynamic_id = self.next_dynamic_id.wrapping_add(1);
                self.spawn_enemy(EnemySpecies::RoomInsect, id, spawn_pos, spawn_yaw);
            }
        }

        // 3. Update Pocket Radio proximity static (`RadioNoise` `0x001ab300` & `enSetRadioVolume` `0x00143890`)
        self.step_radio_only(radio_enabled, player_pos, dt);
    }

    /// Updates the live camera position (`vcWork.cam_pos`) and PS2 `VbWvsMatrix` (`0x010e59d0`)
    /// consumed by [`Self::radio_evaluator`] (`RadioNoise` `0x001ab300`, `SH2-167`).
    #[inline]
    pub fn set_radio_listener(&mut self, camera_pos: Vec3, wvs_matrix: glam::Mat4) {
        self.radio_camera_pos = Some(camera_pos);
        self.radio_wvs_matrix = Some(wvs_matrix);
    }

    /// Evaluates Pocket Radio proximity static (`RadioNoise` `0x001ab300` & `enSetRadioVolume`
    /// `0x00143890`) without advancing enemy AI, allowing menu/map ducking (`0.5` / `0.25`) to
    /// ramp smoothly while the game is paused (`SH2-167`).
    pub fn step_radio_only(&mut self, radio_enabled: bool, player_pos: Vec3, dt: f32) {
        self.scratch_radio_samples.clear();
        for e in &self.enemies {
            self.scratch_radio_samples.push(RadioEnemySample {
                kind: en_kind_from_species(e.species()),
                sub_type: e.sub_type,
                position: e.position(),
                is_spawned_active: !e.is_sleeping,
                is_alive: !e.is_dead(),
                is_downed: e.controller.is_downed(),
            });
        }
        let camera_pos = self.radio_camera_pos.unwrap_or(player_pos);
        let wvs_matrix = self.radio_wvs_matrix.unwrap_or(glam::Mat4::IDENTITY);
        self.radio_output = self.radio_evaluator.evaluate_enemies(
            radio_enabled,
            camera_pos,
            player_pos,
            wvs_matrix,
            &self.scratch_radio_samples,
            dt,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ai_subsystem_lifecycle_and_radio() {
        let mut ai = AiSubsystem::default();
        let player_pos = Vec3::new(0.0, 0.0, 0.0);

        // Spawn Lying Figure 1000 units away
        ai.spawn_enemy(
            EnemySpecies::LyingFigure,
            1,
            Vec3::new(0.0, 0.0, 1000.0),
            std::f32::consts::PI,
        );
        assert_eq!(ai.enemies.len(), 1);

        // Tick AI enough frames for the 2.5s `RadioNoise` (`0x001ab300`) volume ramp to settle
        for _ in 0..200 {
            ai.update(player_pos, 0.0, true, false, true, 0.016);
        }

        // Radio static should be active and audible at 1000 units (`cos(1000/12500 * pi/2) ≈ 0.992`)
        assert!(ai.radio_output.volume > 0.8);
        assert!(ai.radio_output.nearest_enemy_dist < 12_500.0);
    }

    #[test]
    fn test_sh2_167_ai_radio_subtype_ducking_volume_and_wvs() {
        use crate::ai::radio::RadioDuckingState;
        use crate::camera::ps2_wvs_matrix_from_look_at;

        let mut ai = AiSubsystem::default();
        // Spawn Blue Creek Pyramid Head (`sub_type == 6` -> silent in `enSetRadioVolume` `0x00143890`)
        ai.spawn_enemy_with_variant(
            EnemySpecies::PyramidHead,
            1,
            Vec3::new(0.0, 0.0, 1000.0),
            0.0,
            EnemySpawnVariant {
                sub_type: 6,
                is_dark_nurse: false,
            },
        );
        let cam_pos = Vec3::new(0.0, -1600.0, -2000.0);
        let wvs = ps2_wvs_matrix_from_look_at(cam_pos, Vec3::new(0.0, -1600.0, 0.0), -Vec3::Y);
        ai.set_radio_listener(cam_pos, wvs);
        for _ in 0..60 {
            ai.step_radio_only(true, Vec3::ZERO, 1.0 / 60.0);
        }
        assert_eq!(
            ai.radio_output.volume, 0.0,
            "Pyramid Head sub_type == 6 must not emit radio static"
        );

        // Now spawn a Lying Figure straight ahead (+Z) and verify ducking (0.5 / 0.25) & volume_level scaling.
        ai.enemies.clear();
        ai.spawn_enemy(
            EnemySpecies::LyingFigure,
            2,
            Vec3::new(750.0, 0.0, 1000.0),
            0.0,
        );
        ai.radio_evaluator.ducking_state = RadioDuckingState::MapOrMemo;
        ai.radio_evaluator.volume_level = 15;
        for _ in 0..200 {
            ai.step_radio_only(true, Vec3::ZERO, 0.05);
        }
        assert!(
            (ai.radio_output.global_duck - 0.25).abs() < 1e-4,
            "MapOrMemo radio ducking must settle at 0.25, got {}",
            ai.radio_output.global_duck
        );
        let vol_15 = ai.radio_output.driver_volumes[0];
        assert!(vol_15 > 20);
        // Forward sound with PS2 WVS matrix must pan to center (driver pan 64).
        assert_eq!(ai.radio_output.driver_pans[0], 64);

        ai.radio_evaluator.volume_level = 0;
        ai.step_radio_only(true, Vec3::ZERO, 0.016);
        assert_eq!(ai.radio_output.driver_volumes[0], 0);
    }

    #[test]
    fn test_sh2_176_mandarin_floor_probe_passes_x_z_y_order() {
        use crate::collision::CollisionWorld;
        use sh2_assets::FloorTriangle;

        let push_floor_quad =
            |world: &mut CollisionWorld, cx: f32, cz: f32, y: f32, half: f32, material: u32| {
                let p0 = [cx - half, y, cz - half];
                let p1 = [cx + half, y, cz - half];
                let p2 = [cx + half, y, cz + half];
                let p3 = [cx - half, y, cz + half];
                world.floors.push(FloorTriangle {
                    v: [p0, p1, p2],
                    material,
                });
                world.floors.push(FloorTriangle {
                    v: [p0, p2, p3],
                    material,
                });
            };

        // Place Mandarin far from z == 0 (z = 5000.0, y = 0.0) facing +Z (yaw = 0.0)
        // so swapping (y, z) would probe z = 0.0 and miss the floor at z = 5000 + forward_offset.
        let mandarin_pos = Vec3::new(0.0, 0.0, 5000.0);
        let player_pos = Vec3::new(0.0, 0.0, 5500.0);

        // 1. Solid concrete floor (material = 1) at probe XZ -> over_grating must be false -> sub_state == 2 (not 3 attack)
        let mut solid_world = CollisionWorld::new();
        push_floor_quad(&mut solid_world, 0.0, 5500.0, 0.0, 1000.0, 1);

        let mut ai_solid = AiSubsystem::default();
        ai_solid.spawn_enemy(EnemySpecies::Mandarin, 1, mandarin_pos, 0.0);
        ai_solid.update_filtered_with_collision(
            player_pos,
            0.0,
            true,
            false,
            false,
            None,
            Some(&solid_world),
            1.0 / 60.0,
        );
        match &ai_solid.enemies[0].controller {
            ActiveEnemy::Mandarin(m) => {
                assert_ne!(
                    m.sub_state, 3,
                    "Solid floor at probe XZ must prevent Mandarin grating attack (sub_state 3)"
                );
                assert_eq!(m.sub_state, 2);
            }
            _ => panic!("Expected Mandarin"),
        }

        // 2. Grating floor (material = 9) at the exact same probe XZ -> over_grating == true -> sub_state == 3 (attack)
        let mut grating_world = CollisionWorld::new();
        push_floor_quad(&mut grating_world, 0.0, 5500.0, 0.0, 1000.0, 9);

        let mut ai_grating = AiSubsystem::default();
        ai_grating.spawn_enemy(EnemySpecies::Mandarin, 2, mandarin_pos, 0.0);
        ai_grating.update_filtered_with_collision(
            player_pos,
            0.0,
            true,
            false,
            false,
            None,
            Some(&grating_world),
            1.0 / 60.0,
        );
        match &ai_grating.enemies[0].controller {
            ActiveEnemy::Mandarin(m) => {
                assert_eq!(
                    m.sub_state, 3,
                    "Grating floor (material 9) at probe XZ must permit Mandarin attack (sub_state 3)"
                );
            }
            _ => panic!("Expected Mandarin"),
        }
    }
}
