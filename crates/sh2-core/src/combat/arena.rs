//! Combat arena orchestration wiring `CombatController`, unified `AiSubsystem`,
//! hitboxes, `shBattleAttackByEnemy*` dispatchers, auto-aim/LOS stomp targeting, and grapples.
//!
//! # Ground Truth & Binary References
//! - **`shBattleExecAttackQueue` (`SLUS_202.28` `0x00123760` / `sh2pc.exe` `0x00544f90`):**
//!   Dispatches player attacks (`GunshotTypeA`, `GunshotTypeB` 10-pellet 3D cone, `HumanFog`
//!   5-ray cone, `HumanFight` swept blade quad, `HumanFinish` stomp/kick) and enemy attacks
//!   (`shBattleAttackByEnemy{Slash,Strike,Fog,Hug,Needle,Shot}` `0x00122310..0x00122eb0`).
//! - **`enExecTask` (`SLUS_202.28` `0x001413f0`):** All enemy state machines, stats, hitback,
//!   sensory perception, sleep culling, and tribe alerts execute inside [`AiSubsystem`].
//! - **`shBattleGetTargetEnemy` (`0x00124390`), `shBattleChangeTargetEnemy` (`0x001244d0`),
//!   `shBattleGetNearDeadlyTargetEnemy` (`0x00124800`, `600.0` mm LOS-gated stomp).**

use super::attack_list::{get_attack_spec, AttackDispatchKind, AttackSpec};
use super::controller::{CombatController, CombatMode};
use super::enemy_attack::dispatch_enemy_attack;
use super::grapple::GrappleSession;
use super::hitboxes::{
    generate_hyper_spray_rays, generate_shotgun_cone_rays, hyper_spray_effective_range, CombatRng,
    SweptBladeQuad,
};
use super::targeting::{
    aim_turn_toward_target, cycle_auto_aim_target, find_near_deadly_target, select_auto_aim_target,
    TargetCandidate, STOMP_TARGET_MAX_DIST_MM,
};
use crate::ai::creatures::EnemySpecies;
use crate::ai::AiSubsystem;
use glam::Vec3;

/// Complete combat arena controller managing player, unified [`AiSubsystem`], hit resolution,
/// enemy attack dispatch, targeting, and grapples.
#[derive(Debug, Clone)]
pub struct CombatArena {
    pub player: CombatController,
    /// Unified AI subsystem owning all active enemy actors (`enExecTask` `0x001413f0`).
    pub ai: AiSubsystem,
    pub player_position: Vec3,
    pub player_yaw: f32,
    pub attack_hit_registered: bool,
    /// Last 30 Hz animation frame evaluated for multi-hit attacks (Great Knife, Chainsaw, Cleaver).
    pub last_multi_hit_frame: Option<u8>,
    /// Previous frame melee weapon tip in world mm (`chara + 0x150` / `sh2pc +0x128` in `0x00121370`).
    pub prev_blade_tip: Option<Vec3>,
    /// Deterministic PRNG for 3D cone Shotgun pellet spread (`0x00120f30`).
    pub rng: CombatRng,
    /// Currently locked auto-aim target index (`_DAT_003c83c4` in `PlayerGetTarget` `0x0010ffa0`).
    pub locked_target_idx: Option<usize>,
    /// Index of enemy currently holding James in a grapple lock (`0x533d00` / `0x4b7160`).
    pub grappling_enemy_idx: Option<usize>,
    /// Modular authentic PS2/PC grapple session state machine.
    pub grapple_session: Option<GrappleSession>,
    /// Pending enemy attack SFX ID emitted by `shBattleAttackByEnemy*` (`0x00122310..0x00122eb0`).
    pub pending_enemy_sfx: Option<u32>,
    /// Reusable scratch buffer for `HumanFog` 5-ray deduplication (zero heap allocation per tick).
    scratch_spray_hit_mask: Vec<bool>,
}

impl Default for CombatArena {
    fn default() -> Self {
        Self::new()
    }
}

impl CombatArena {
    /// Initialize a combat test arena with all monsters and bosses spawned into [`AiSubsystem`]
    /// using authentic `SLUS_202.28` / `sh2pc.exe` stats and `enSetSize` collision radii.
    pub fn new() -> Self {
        let player = CombatController::new(false, false);
        let mut ai = AiSubsystem::default();

        // Section 1: Front Entry Courtyard (Z ~ 1500)
        ai.spawn_enemy(
            EnemySpecies::LyingFigure,
            1,
            Vec3::new(0.0, 0.0, 1500.0),
            0.0,
        );
        ai.spawn_enemy(
            EnemySpecies::Mannequin,
            2,
            Vec3::new(2200.0, 0.0, 1500.0),
            0.0,
        );
        ai.spawn_enemy(
            EnemySpecies::Creeper,
            8,
            Vec3::new(-2200.0, 0.0, 1500.0),
            0.0,
        );
        // Section 2: Middle Gallery & Grate Chambers (Z ~ 3600)
        ai.spawn_enemy(EnemySpecies::Nurse, 3, Vec3::new(-3000.0, 0.0, 3600.0), 0.0);
        ai.spawn_enemy(EnemySpecies::Mandarin, 7, Vec3::new(0.0, 0.0, 3600.0), 0.0);
        ai.spawn_enemy(
            EnemySpecies::FleshLips,
            6,
            Vec3::new(3000.0, 0.0, 3600.0),
            0.0,
        );
        // Section 3: Deep Boss Sanctuary (Z ~ 5800)
        ai.spawn_enemy(
            EnemySpecies::AbstractDaddy,
            5,
            Vec3::new(-2600.0, 0.0, 5800.0),
            0.0,
        );
        ai.spawn_enemy(
            EnemySpecies::PyramidHead,
            4,
            Vec3::new(0.0, 0.0, 5800.0),
            0.0,
        );
        ai.spawn_enemy(
            EnemySpecies::MaryBoss,
            9,
            Vec3::new(2600.0, 0.0, 5800.0),
            0.0,
        );

        Self {
            player,
            ai,
            player_position: Vec3::ZERO,
            player_yaw: 0.0,
            attack_hit_registered: false,
            last_multi_hit_frame: None,
            prev_blade_tip: None,
            rng: CombatRng::default(),
            locked_target_idx: None,
            grappling_enemy_idx: None,
            grapple_session: None,
            pending_enemy_sfx: None,
            scratch_spray_hit_mask: Vec::with_capacity(32),
        }
    }

    fn build_target_candidates(&self) -> Vec<TargetCandidate> {
        self.ai
            .enemies
            .iter()
            .enumerate()
            .map(|(index, e)| TargetCandidate {
                index,
                position: e.position(),
                is_downed: e.is_downed(),
                is_dead: e.is_dead(),
            })
            .collect()
    }

    /// Find the nearest downed enemy eligible for stomp/kick (`shBattleGetNearDeadlyTargetEnemy`,
    /// `SLUS_202.28` `0x00124800`, `600.0` mm max distance + caller-supplied LOS predicate).
    pub fn find_stomp_target_with_los<F>(&self, has_los: F) -> Option<usize>
    where
        F: FnMut(Vec3) -> bool,
    {
        let candidates = self.build_target_candidates();
        find_near_deadly_target(self.player_position, &candidates, has_los)
    }

    /// Check if any enemy is currently downed within `600.0` mm (`shBattleGetNearDeadlyTargetEnemy`
    /// `0x00124800`) assuming unobstructed arena LOS (zero heap allocations per tick).
    pub fn has_downed_enemy_at_feet(&self) -> bool {
        self.ai.enemies.iter().any(|e| {
            e.is_downed()
                && !e.is_dead()
                && (e.position() - self.player_position).length() <= STOMP_TARGET_MAX_DIST_MM
        })
    }

    /// Acquire the primary auto-aim target (`shBattleGetTargetEnemy`, `0x00124390`).
    pub fn acquire_auto_aim_target<F>(&mut self, max_range: f32, has_los: F) -> Option<usize>
    where
        F: FnMut(Vec3) -> bool,
    {
        let candidates = self.build_target_candidates();
        let target = select_auto_aim_target(self.player_position, &candidates, max_range, has_los);
        self.locked_target_idx = target;
        target
    }

    /// Cycle auto-aim target left (`-1`) or right (`+1`) (`shBattleChangeTargetEnemy`, `0x001244d0`).
    pub fn cycle_auto_aim_target<F>(
        &mut self,
        direction: i8,
        max_range: f32,
        has_los: F,
    ) -> Option<usize>
    where
        F: FnMut(Vec3) -> bool,
    {
        let candidates = self.build_target_candidates();
        let next = cycle_auto_aim_target(
            self.player_position,
            self.player_yaw,
            self.locked_target_idx,
            direction,
            &candidates,
            max_range,
            has_los,
        );
        if next.is_some() {
            self.locked_target_idx = next;
        }
        next
    }

    /// Smoothly rotate `player_yaw` toward the currently locked auto-aim target
    /// (`PlayerCheckAimingToEnemy`, `SLUS_202.28` `0x0021c7a0`).
    pub fn step_auto_aim_turn(&mut self, dt: f32) {
        if let Some(idx) = self.locked_target_idx {
            if let Some(enemy) = self.ai.enemies.get(idx) {
                if !enemy.is_dead() {
                    self.player_yaw = aim_turn_toward_target(
                        self.player_position,
                        self.player_yaw,
                        enemy.position(),
                        dt,
                    );
                    return;
                }
            }
            self.locked_target_idx = None;
        }
    }

    /// Query the authentic animation clip James should play if currently grappled.
    #[inline]
    pub fn active_grapple_player_clip(&self) -> Option<crate::animation::AnimationClip> {
        self.grapple_session
            .as_ref()
            .map(|s| s.active_player_clip())
    }

    /// Query the stage ANM overlay relative path James should sample from if currently grappled.
    #[inline]
    pub fn active_grapple_james_anm_overlay(&self) -> Option<&'static str> {
        self.grapple_session
            .as_ref()
            .and_then(|s| s.spec.james_anm_overlay)
    }

    /// Register button mashing or stick flicks to accelerate breakout.
    #[inline]
    pub fn register_grapple_struggle_input(&mut self, button_presses: u32) {
        if let Some(session) = &mut self.grapple_session {
            session.register_struggle_input(button_presses);
            self.player.struggle_meter = session.struggle_gauge;
        }
    }

    /// Check if a specific enemy can initiate an authentic grapple attack on James.
    pub fn check_enemy_grapple_initiation(&mut self, enemy_idx: usize) -> bool {
        let is_god_mode = self.player.god_mode;
        let is_dying_or_dead =
            self.player.mode == CombatMode::Dead || self.player.mode == CombatMode::Dying;
        if is_god_mode || is_dying_or_dead || self.grapple_session.is_some() {
            return false;
        }

        let diff_idx = self.ai.difficulty.as_index();
        if let Some(enemy) = self.ai.enemies.get_mut(enemy_idx) {
            let player_p = self.player_position;
            if let Some(session) = GrappleSession::try_initiate(
                enemy_idx,
                enemy,
                &mut self.player,
                player_p,
                &mut self.player_yaw,
                diff_idx,
            ) {
                self.grappling_enemy_idx = Some(enemy_idx);
                self.grapple_session = Some(session);
                return true;
            }
        }
        false
    }

    /// Advance active grapple session lifecycle and maintain creature anchoring.
    pub fn process_grapple_lifecycle(&mut self, dt: f32) {
        if let Some(session) = &mut self.grapple_session {
            session.update(
                dt,
                &mut self.player,
                &mut self.ai.enemies,
                self.player_position,
                self.player_yaw,
            );
            self.grappling_enemy_idx = Some(session.enemy_idx);
            if session.is_finished() {
                self.grapple_session = None;
                self.grappling_enemy_idx = None;
            }
        } else {
            self.grappling_enemy_idx = None;
        }
    }

    /// Execute simulation tick (60 Hz) routing all enemies through [`AiSubsystem`].
    pub fn tick(&mut self, ready_held: bool, action_pressed: bool, action_held: bool, dt: f32) {
        self.tick_with_collision(ready_held, action_pressed, action_held, None, dt);
    }

    /// Execute simulation tick (60 Hz) with optional `.cld` [`crate::collision::CollisionWorld`]
    /// floor probing for Mandarin grating checks.
    pub fn tick_with_collision(
        &mut self,
        ready_held: bool,
        action_pressed: bool,
        action_held: bool,
        collision: Option<&crate::collision::CollisionWorld>,
        dt: f32,
    ) {
        self.tick_with_collision_input(
            crate::combat::CombatInputSnapshot {
                ready_held,
                action_pressed,
                action_held,
                ..Default::default()
            },
            collision,
            dt,
        );
    }

    /// Execute simulation tick (60 Hz) with full [`crate::combat::CombatInputSnapshot`]
    /// (PS2 pressure, `R2` analog trigger, native split bindings, running override) and optional
    /// `.cld` collision probing (`SH2-83`).
    pub fn tick_with_collision_input(
        &mut self,
        mut input: crate::combat::CombatInputSnapshot,
        collision: Option<&crate::collision::CollisionWorld>,
        dt: f32,
    ) {
        input.has_downed_enemy_nearby = self.has_downed_enemy_at_feet();
        self.player.handle_combat_inputs_ex(input, dt);

        self.step_active_attack_hits(None);

        let is_in_grapple = self.grapple_session.is_some() || self.player.mode == CombatMode::Held;
        self.ai.update_filtered_with_collision(
            self.player_position,
            self.player_yaw,
            true,
            false,
            true,
            None,
            collision,
            dt,
        );

        for idx in 0..self.ai.enemies.len() {
            if !is_in_grapple {
                self.check_enemy_grapple_initiation(idx);
            }
            self.step_enemy_attack_dispatch(idx);
        }

        self.process_grapple_lifecycle(dt);
        self.resolve_character_separations();
    }

    /// Execute simulation tick isolating updates to only one active enemy index.
    pub fn tick_isolated(
        &mut self,
        ready_held: bool,
        action_pressed: bool,
        action_held: bool,
        dt: f32,
        active_enemy_idx: usize,
    ) {
        self.tick_isolated_input(
            crate::combat::CombatInputSnapshot {
                ready_held,
                action_pressed,
                action_held,
                ..Default::default()
            },
            dt,
            active_enemy_idx,
        );
    }

    /// Execute isolated-enemy simulation tick with full [`crate::combat::CombatInputSnapshot`] (`SH2-83`).
    pub fn tick_isolated_input(
        &mut self,
        mut input: crate::combat::CombatInputSnapshot,
        dt: f32,
        active_enemy_idx: usize,
    ) {
        if self.ai.enemies.is_empty() {
            return;
        }
        let isolated_idx = active_enemy_idx % self.ai.enemies.len();

        input.has_downed_enemy_nearby = {
            let e = &self.ai.enemies[isolated_idx];
            e.is_downed()
                && !e.is_dead()
                && (e.position() - self.player_position).length() <= STOMP_TARGET_MAX_DIST_MM
        };

        self.player.handle_combat_inputs_ex(input, dt);

        self.step_active_attack_hits(Some(isolated_idx));

        let is_in_grapple = self.grapple_session.is_some() || self.player.mode == CombatMode::Held;
        self.ai.update_filtered(
            self.player_position,
            self.player_yaw,
            true,
            false,
            true,
            Some(isolated_idx),
            dt,
        );

        if !is_in_grapple {
            self.check_enemy_grapple_initiation(isolated_idx);
        }
        self.step_enemy_attack_dispatch(isolated_idx);

        self.process_grapple_lifecycle(dt);
        self.resolve_single_enemy_player_separation(isolated_idx);
    }

    /// Dispatch an active enemy attack against James via `shBattleAttackByEnemy{Slash,Strike,Fog,Hug,Needle,Shot}`
    /// (`SLUS_202.28` `0x00122310..0x00122eb0`).
    fn step_enemy_attack_dispatch(&mut self, enemy_idx: usize) {
        if self.player.god_mode
            || self.player.mode == CombatMode::Held
            || self.player.mode == CombatMode::Dying
            || self.player.mode == CombatMode::Dead
            || self.player.invulnerability_timer > 0.0
        {
            return;
        }

        let Some(atk_id) = self.ai.enemies[enemy_idx].active_attack_id() else {
            return;
        };
        let Some(atk) = get_attack_spec(atk_id as usize) else {
            return;
        };

        let is_outdoor = self.ai.is_outdoor;
        let difficulty = self.ai.difficulty.as_index() as u8;
        let enemy = &mut self.ai.enemies[enemy_idx];
        let species = enemy.species();
        let e_pos = enemy.position();
        let e_yaw = enemy.yaw();
        let anim_frame = (enemy.anim().clip_time() * 30.0) as u8;

        let outcome = dispatch_enemy_attack(
            species,
            atk,
            &mut enemy.attack_work,
            anim_frame,
            e_pos,
            e_yaw,
            self.player_position,
            self.player_yaw,
            &mut self.player.guard_state,
            difficulty,
            is_outdoor,
        );

        if let Some(sfx) = outcome.sfx_id {
            self.pending_enemy_sfx = Some(sfx);
        }

        if outcome.hit_player {
            if outcome.is_hug {
                self.check_enemy_grapple_initiation(enemy_idx);
            } else if !outcome.guarded && outcome.damage_ap > 0.0 {
                self.player.apply_damage(outcome.damage_ap, outcome.is_fog);
            }
        }
    }

    fn step_active_attack_hits(&mut self, isolated_idx: Option<usize>) {
        let Some(atk) = self.player.active_attack else {
            self.attack_hit_registered = false;
            self.last_multi_hit_frame = None;
            self.prev_blade_tip = None;
            for enemy in &mut self.ai.enemies {
                enemy.stats_mut().reset_multi_hit_if_idle();
            }
            return;
        };

        let hit_frame = self.player.anim_frame as u8;
        let in_window = hit_frame >= atk.first_hit_frame && hit_frame <= atk.last_hit_frame;

        if atk.is_multi_hit() {
            if in_window && self.last_multi_hit_frame != Some(hit_frame) {
                self.last_multi_hit_frame = Some(hit_frame);
                self.resolve_attack_hits(&atk, isolated_idx);
            }
        } else if !self.attack_hit_registered && in_window {
            self.attack_hit_registered = true;
            self.resolve_attack_hits(&atk, isolated_idx);
        }
    }

    fn resolve_single_enemy_player_separation(&mut self, idx: usize) {
        if self.player.mode == CombatMode::Held {
            return;
        }
        let enemy = &mut self.ai.enemies[idx];
        if enemy.is_dead() || enemy.is_downed() {
            return;
        }
        if matches!(
            enemy.species(),
            EnemySpecies::Creeper
                | EnemySpecies::FleshLips
                | EnemySpecies::Mandarin
                | EnemySpecies::MaryBoss
                | EnemySpecies::RoomInsect
        ) {
            return;
        }

        let e_pos = enemy.position();
        let dx = self.player_position.x - e_pos.x;
        let dz = self.player_position.z - e_pos.z;
        let dist_sq = dx * dx + dz * dz;

        // Authentic `enSetSize` (`0x001426e0`) body radius + James collision radius (200mm)
        let enemy_col_radius = enemy.size().radius;
        let min_dist = self.ai.player_radius_mm + enemy_col_radius;

        if dist_sq < min_dist * min_dist {
            let dist = dist_sq.sqrt();
            let (nx, nz) = if dist > 1e-4 {
                (dx / dist, dz / dist)
            } else {
                (1.0, 0.0)
            };
            let penetration = min_dist - dist;
            let (push_player, push_enemy) = match enemy.species() {
                EnemySpecies::PyramidHead
                | EnemySpecies::TwinPyramidHead
                | EnemySpecies::AbstractDaddy => (penetration, 0.0),
                _ => (penetration * 0.6, penetration * 0.4),
            };

            self.player_position.x += nx * push_player;
            self.player_position.z += nz * push_player;
            enemy.set_position(Vec3::new(
                e_pos.x - nx * push_enemy,
                e_pos.y,
                e_pos.z - nz * push_enemy,
            ));
        }
    }

    /// Resolve mutual character-to-enemy and enemy-to-enemy cylinder collision separations
    /// using authentic `enSetSize` (`0x001426e0`) radii.
    pub fn resolve_character_separations(&mut self) {
        for idx in 0..self.ai.enemies.len() {
            self.resolve_single_enemy_player_separation(idx);
        }

        let enemy_count = self.ai.enemies.len();
        for i in 0..enemy_count {
            if self.ai.enemies[i].is_dead()
                || self.ai.enemies[i].is_downed()
                || matches!(
                    self.ai.enemies[i].species(),
                    EnemySpecies::Creeper
                        | EnemySpecies::FleshLips
                        | EnemySpecies::Mandarin
                        | EnemySpecies::MaryBoss
                        | EnemySpecies::RoomInsect
                )
            {
                continue;
            }

            for j in (i + 1)..enemy_count {
                if self.ai.enemies[j].is_dead()
                    || self.ai.enemies[j].is_downed()
                    || matches!(
                        self.ai.enemies[j].species(),
                        EnemySpecies::Creeper
                            | EnemySpecies::FleshLips
                            | EnemySpecies::Mandarin
                            | EnemySpecies::MaryBoss
                            | EnemySpecies::RoomInsect
                    )
                {
                    continue;
                }

                let pi = self.ai.enemies[i].position();
                let pj = self.ai.enemies[j].position();
                let dx = pi.x - pj.x;
                let dz = pi.z - pj.z;
                let dist_sq = dx * dx + dz * dz;

                let min_dist = self.ai.enemies[i].size().radius + self.ai.enemies[j].size().radius;

                if dist_sq < min_dist * min_dist {
                    let dist = dist_sq.sqrt();
                    let (nx, nz) = if dist > 1e-4 {
                        (dx / dist, dz / dist)
                    } else {
                        (1.0, 0.0)
                    };
                    let half_pen = (min_dist - dist) * 0.5;

                    self.ai.enemies[i].set_position(Vec3::new(
                        pi.x + nx * half_pen,
                        pi.y,
                        pi.z + nz * half_pen,
                    ));
                    self.ai.enemies[j].set_position(Vec3::new(
                        pj.x - nx * half_pen,
                        pj.y,
                        pj.z - nz * half_pen,
                    ));
                }
            }
        }
    }

    /// Test active player attack collision against enemies in [`AiSubsystem`]
    /// (`shBattleExecAttackQueue` `0x00123760`).
    fn resolve_attack_hits(&mut self, atk: &AttackSpec, isolated_idx: Option<usize>) {
        let forward = Vec3::new(self.player_yaw.sin(), 0.0, self.player_yaw.cos());
        let dir_xz = [forward.x, forward.z];

        match atk.dispatch_kind() {
            AttackDispatchKind::GunshotTypeA => {
                let muzzle = self.player_position + Vec3::new(0.0, -1100.0, 0.0) + forward * 200.0;
                if let Some(hit_idx) = self.find_closest_ray_hit(
                    muzzle,
                    forward,
                    atk.min_range,
                    atk.max_range,
                    isolated_idx,
                ) {
                    self.ai.apply_hit_to_enemy(hit_idx, atk, dir_xz);
                }
            }
            AttackDispatchKind::GunshotTypeB => {
                let muzzle = self.player_position + Vec3::new(0.0, -1100.0, 0.0) + forward * 200.0;
                let rays = generate_shotgun_cone_rays(forward, &mut self.rng);
                for pellet_dir in rays {
                    if let Some(hit_idx) = self.find_closest_ray_hit(
                        muzzle,
                        pellet_dir,
                        atk.min_range,
                        atk.max_range,
                        isolated_idx,
                    ) {
                        self.ai
                            .apply_hit_to_enemy(hit_idx, atk, [pellet_dir.x, pellet_dir.z]);
                    }
                }
            }
            AttackDispatchKind::HumanFog => {
                let (min_r, max_r) = hyper_spray_effective_range(
                    self.player.hyper_spray_charge,
                    atk.min_range,
                    atk.max_range,
                );
                if max_r <= 0.0 {
                    return;
                }
                let muzzle = self.player_position + Vec3::new(0.0, -1100.0, 0.0) + forward * 200.0;
                let rays = generate_hyper_spray_rays(forward);
                let color = self.player.hyper_spray_color;
                self.scratch_spray_hit_mask.clear();
                self.scratch_spray_hit_mask
                    .resize(self.ai.enemies.len(), false);

                for ray_dir in rays {
                    if let Some(hit_idx) =
                        self.find_closest_ray_hit(muzzle, ray_dir, min_r, max_r, isolated_idx)
                    {
                        if !self.scratch_spray_hit_mask[hit_idx] {
                            self.scratch_spray_hit_mask[hit_idx] = true;
                            self.ai.apply_hyper_spray_hit_to_enemy(hit_idx, atk, color);
                        }
                    }
                }
            }
            AttackDispatchKind::HumanFight => {
                let weapon_origin =
                    self.player_position + Vec3::new(0.0, -1000.0, 0.0) + forward * 200.0;
                let quad = SweptBladeQuad::new(
                    weapon_origin,
                    forward,
                    atk.min_range,
                    atk.max_range,
                    self.prev_blade_tip,
                );
                self.prev_blade_tip = Some(quad.curr_tip);

                for idx in 0..self.ai.enemies.len() {
                    if isolated_idx.is_some_and(|only| only != idx) {
                        continue;
                    }
                    let hit = {
                        let enemy = &self.ai.enemies[idx];
                        !enemy.is_dead() && enemy.hitbox().intersects_swept_quad(&quad)
                    };
                    if hit {
                        self.ai.apply_hit_to_enemy(idx, atk, dir_xz);
                    }
                }
            }
            AttackDispatchKind::HumanFinish => {
                let feet_check = self.player_position + forward * 250.0;
                for idx in 0..self.ai.enemies.len() {
                    if isolated_idx.is_some_and(|only| only != idx) {
                        continue;
                    }
                    let hit = {
                        let enemy = &self.ai.enemies[idx];
                        if !enemy.is_downed() || enemy.is_dead() {
                            false
                        } else {
                            let pos = enemy.position();
                            let dist = (Vec3::new(pos.x, 0.0, pos.z) - feet_check).length();
                            let reach =
                                enemy.size().battle_radius(enemy.species(), true) + atk.max_range;
                            dist <= reach
                        }
                    };
                    if hit {
                        self.ai.apply_hit_to_enemy(idx, atk, dir_xz);
                    }
                }
            }
            AttackDispatchKind::None | AttackDispatchKind::Enemy => {}
        }
    }

    fn find_closest_ray_hit(
        &self,
        origin: Vec3,
        dir: Vec3,
        min_range: f32,
        max_range: f32,
        isolated_idx: Option<usize>,
    ) -> Option<usize> {
        let mut closest: Option<(usize, f32)> = None;
        for (idx, enemy) in self.ai.enemies.iter().enumerate() {
            if isolated_idx.is_some_and(|only| only != idx) || enemy.is_dead() {
                continue;
            }
            if let Some(t) = enemy
                .hitbox()
                .intersects_ray(origin, dir, min_range, max_range)
            {
                if closest.is_none_or(|(_, best_t)| t < best_t) {
                    closest = Some((idx, t));
                }
            }
        }
        closest.map(|(idx, _)| idx)
    }
}
