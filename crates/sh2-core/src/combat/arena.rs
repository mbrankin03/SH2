//! Combat arena orchestration wiring `CombatController`, `CombatTargetEnemy`, hitboxes,
//! auto-aim/LOS stomp targeting, and grapple lifecycle.
//!
//! # Ground Truth & Binary References
//! - **`shBattleExecAttackQueue` (`SLUS_202.28` `0x00123760` / `sh2pc.exe` `0x00544f00`):**
//!   Dispatches attacks by `AttackDispatchKind` (GunshotTypeA, GunshotTypeB 10-pellet 3D cone,
//!   HumanFog 5-ray cone, HumanFight swept blade quad, HumanFinish stomp/kick).
//! - **`shBattleAttackByHumanFightType` (`SLUS_202.28` `0x00121370` / `sh2pc.exe` `0x00543ec0`):**
//!   Tracks previous weapon tip (`prev_blade_tip`) to sweep a blade quad over `min_range..max_range`
//!   and evaluates multi-hit frames for Great Knife (`19..=22`), Chainsaw (`23..=24`), and
//!   Maria Chinese Cleaver (`28..=30`).
//! - **`shBattleGetTargetEnemy` (`0x00124390`), `shBattleChangeTargetEnemy` (`0x001244d0`),
//!   `shBattleGetNearDeadlyTargetEnemy` (`0x00124800`, `600.0` mm LOS-gated stomp).**

use super::attack_list::{AttackDispatchKind, AttackSpec};
use super::controller::{CombatController, CombatMode};
use super::enemy::CombatTargetEnemy;
use super::grapple::GrappleSession;
use super::hitboxes::{
    generate_hyper_spray_rays, generate_shotgun_cone_rays, hyper_spray_effective_range, CombatRng,
    SweptBladeQuad,
};
use super::targeting::{
    aim_turn_toward_target, cycle_auto_aim_target, find_near_deadly_target, select_auto_aim_target,
    TargetCandidate, STOMP_TARGET_MAX_DIST_MM,
};
use glam::Vec3;

/// Complete combat arena controller managing player, enemies, hit resolution, targeting, and grapples.
#[derive(Debug, Clone)]
pub struct CombatArena {
    pub player: CombatController,
    pub enemies: Vec<CombatTargetEnemy>,
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
}

impl Default for CombatArena {
    fn default() -> Self {
        Self::new()
    }
}

impl CombatArena {
    /// Initialize a combat test arena with all monsters and bosses separated into
    /// distinct thematic sections across the arena map.
    pub fn new() -> Self {
        let player = CombatController::new(false, false);
        let enemies = vec![
            // Section 1: Front Entry Courtyard (Z ~ 1500)
            CombatTargetEnemy::new_lying_figure(1, Vec3::new(0.0, 0.0, 1500.0)),
            CombatTargetEnemy::new_mannequin(2, Vec3::new(2200.0, 0.0, 1500.0)),
            CombatTargetEnemy::new_creeper(8, Vec3::new(-2200.0, 0.0, 1500.0)),
            // Section 2: Middle Gallery & Grate Chambers (Z ~ 3600)
            CombatTargetEnemy::new_nurse(3, Vec3::new(-3000.0, 0.0, 3600.0)),
            CombatTargetEnemy::new_mandarin(7, Vec3::new(0.0, 0.0, 3600.0)),
            CombatTargetEnemy::new_flesh_lips(6, Vec3::new(3000.0, 0.0, 3600.0)),
            // Section 3: Deep Boss Sanctuary (Z ~ 5800)
            CombatTargetEnemy::new_abstract_daddy(5, Vec3::new(-2600.0, 0.0, 5800.0)),
            CombatTargetEnemy::new_pyramid_head(4, Vec3::new(0.0, 0.0, 5800.0)),
            CombatTargetEnemy::new_mary_boss(9, Vec3::new(2600.0, 0.0, 5800.0)),
        ];

        Self {
            player,
            enemies,
            player_position: Vec3::ZERO,
            player_yaw: 0.0,
            attack_hit_registered: false,
            last_multi_hit_frame: None,
            prev_blade_tip: None,
            rng: CombatRng::default(),
            locked_target_idx: None,
            grappling_enemy_idx: None,
            grapple_session: None,
        }
    }

    fn build_target_candidates(&self) -> Vec<TargetCandidate> {
        self.enemies
            .iter()
            .enumerate()
            .map(|(index, e)| TargetCandidate {
                index,
                position: e.position,
                is_downed: e.is_downed,
                is_dead: e.is_dead,
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
    /// `0x00124800`) assuming unobstructed arena LOS.
    pub fn has_downed_enemy_at_feet(&self) -> bool {
        self.find_stomp_target_with_los(|_| true).is_some()
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
            if let Some(enemy) = self.enemies.get(idx) {
                if !enemy.is_dead {
                    self.player_yaw = aim_turn_toward_target(
                        self.player_position,
                        self.player_yaw,
                        enemy.position,
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

        if let Some(enemy) = self.enemies.get_mut(enemy_idx) {
            let player_p = self.player_position;
            if let Some(session) = GrappleSession::try_initiate(
                enemy_idx,
                enemy,
                &mut self.player,
                player_p,
                &mut self.player_yaw,
                2, // Normal difficulty
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
                &mut self.enemies,
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

    /// Execute simulation tick (60 Hz).
    pub fn tick(&mut self, ready_held: bool, action_pressed: bool, action_held: bool, dt: f32) {
        let has_downed = self.has_downed_enemy_at_feet();
        self.player
            .handle_combat_inputs(ready_held, action_pressed, action_held, has_downed, dt);

        self.step_active_attack_hits(None);

        let player_p = self.player_position;
        let is_in_grapple = self.grapple_session.is_some() || self.player.mode == CombatMode::Held;

        for idx in 0..self.enemies.len() {
            self.enemies[idx].update(player_p, dt);
            if !is_in_grapple {
                self.check_enemy_grapple_initiation(idx);
            }
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
        if self.enemies.is_empty() {
            return;
        }
        let isolated_idx = active_enemy_idx % self.enemies.len();

        let has_downed = {
            let e = &self.enemies[isolated_idx];
            e.is_downed
                && !e.is_dead
                && (e.position - self.player_position).length() <= STOMP_TARGET_MAX_DIST_MM
        };

        self.player
            .handle_combat_inputs(ready_held, action_pressed, action_held, has_downed, dt);

        self.step_active_attack_hits(Some(isolated_idx));

        let player_p = self.player_position;
        let is_in_grapple = self.grapple_session.is_some() || self.player.mode == CombatMode::Held;

        let enemy = &mut self.enemies[isolated_idx];
        enemy.update(player_p, dt);

        if !is_in_grapple {
            self.check_enemy_grapple_initiation(isolated_idx);
        }

        self.process_grapple_lifecycle(dt);

        if self.player.mode != CombatMode::Held {
            let enemy = &mut self.enemies[isolated_idx];
            if !enemy.is_dead
                && !enemy.is_downed
                && enemy.id != 8
                && enemy.id != 6
                && enemy.id != 7
                && enemy.id != 9
            {
                let dx = self.player_position.x - enemy.position.x;
                let dz = self.player_position.z - enemy.position.z;
                let dist_sq = dx * dx + dz * dz;

                let enemy_col_radius = match enemy.id {
                    4 => 200.0,
                    5 => 220.0,
                    _ => 130.0,
                };
                let min_dist = 130.0 + enemy_col_radius;

                if dist_sq < min_dist * min_dist {
                    let dist = dist_sq.sqrt();
                    let (nx, nz) = if dist > 1e-4 {
                        (dx / dist, dz / dist)
                    } else {
                        (1.0, 0.0)
                    };
                    let penetration = min_dist - dist;
                    let (push_player, push_enemy) = match enemy.id {
                        4 | 5 => (penetration, 0.0),
                        _ => (penetration * 0.6, penetration * 0.4),
                    };

                    self.player_position.x += nx * push_player;
                    self.player_position.z += nz * push_player;
                    enemy.position.x -= nx * push_enemy;
                    enemy.position.z -= nz * push_enemy;
                }
            }
        }
    }

    fn step_active_attack_hits(&mut self, isolated_idx: Option<usize>) {
        let Some(atk) = self.player.active_attack else {
            self.attack_hit_registered = false;
            self.last_multi_hit_frame = None;
            self.prev_blade_tip = None;
            return;
        };

        let hit_frame = self.player.anim_frame as u8;
        let in_window = hit_frame >= atk.first_hit_frame && hit_frame <= atk.last_hit_frame;

        if atk.is_multi_hit() {
            // Multi-hit weapons (Great Knife 19..=22, Chainsaw 23..=24, Maria Cleaver 28..=30,
            // Hyper Spray 8..=9) evaluate once per distinct 30 Hz frame in the hit window.
            if in_window && self.last_multi_hit_frame != Some(hit_frame) {
                self.last_multi_hit_frame = Some(hit_frame);
                self.resolve_attack_hits(&atk, isolated_idx);
            }
        } else if !self.attack_hit_registered && in_window {
            self.attack_hit_registered = true;
            self.resolve_attack_hits(&atk, isolated_idx);
        }
    }

    /// Resolve mutual character-to-enemy and enemy-to-enemy cylinder collision separations.
    pub fn resolve_character_separations(&mut self) {
        let is_held = self.player.mode == CombatMode::Held;

        if !is_held {
            for enemy in &mut self.enemies {
                if enemy.is_dead || enemy.is_downed || enemy.id == 8 {
                    continue;
                }
                if enemy.id == 6 || enemy.id == 7 || enemy.id == 9 {
                    continue;
                }

                let dx = self.player_position.x - enemy.position.x;
                let dz = self.player_position.z - enemy.position.z;
                let dist_sq = dx * dx + dz * dz;

                let enemy_col_radius = match enemy.id {
                    4 => 200.0,
                    5 => 220.0,
                    _ => 130.0,
                };
                let min_dist = 130.0 + enemy_col_radius;

                if dist_sq < min_dist * min_dist {
                    let dist = dist_sq.sqrt();
                    let (nx, nz) = if dist > 1e-4 {
                        (dx / dist, dz / dist)
                    } else {
                        (1.0, 0.0)
                    };
                    let penetration = min_dist - dist;

                    let (push_player, push_enemy) = match enemy.id {
                        4 | 5 => (penetration, 0.0),
                        _ => (penetration * 0.6, penetration * 0.4),
                    };

                    self.player_position.x += nx * push_player;
                    self.player_position.z += nz * push_player;
                    enemy.position.x -= nx * push_enemy;
                    enemy.position.z -= nz * push_enemy;
                }
            }
        }

        let enemy_count = self.enemies.len();
        for i in 0..enemy_count {
            if self.enemies[i].is_dead || self.enemies[i].is_downed || self.enemies[i].id == 8 {
                continue;
            }
            if self.enemies[i].id == 6 || self.enemies[i].id == 7 || self.enemies[i].id == 9 {
                continue;
            }

            for j in (i + 1)..enemy_count {
                if self.enemies[j].is_dead || self.enemies[j].is_downed || self.enemies[j].id == 8 {
                    continue;
                }
                if self.enemies[j].id == 6 || self.enemies[j].id == 7 || self.enemies[j].id == 9 {
                    continue;
                }

                let dx = self.enemies[i].position.x - self.enemies[j].position.x;
                let dz = self.enemies[i].position.z - self.enemies[j].position.z;
                let dist_sq = dx * dx + dz * dz;

                let r_i = match self.enemies[i].id {
                    4 => 200.0,
                    5 => 220.0,
                    _ => 130.0,
                };
                let r_j = match self.enemies[j].id {
                    4 => 200.0,
                    5 => 220.0,
                    _ => 130.0,
                };
                let min_dist = r_i + r_j;

                if dist_sq < min_dist * min_dist {
                    let dist = dist_sq.sqrt();
                    let (nx, nz) = if dist > 1e-4 {
                        (dx / dist, dz / dist)
                    } else {
                        (1.0, 0.0)
                    };
                    let penetration = min_dist - dist;
                    let half_pen = penetration * 0.5;

                    self.enemies[i].position.x += nx * half_pen;
                    self.enemies[i].position.z += nz * half_pen;
                    self.enemies[j].position.x -= nx * half_pen;
                    self.enemies[j].position.z -= nz * half_pen;
                }
            }
        }
    }

    /// Test active attack collision against enemies in the arena (`shBattleExecAttackQueue` `0x00123760`).
    fn resolve_attack_hits(&mut self, atk: &AttackSpec, isolated_idx: Option<usize>) {
        let forward = Vec3::new(self.player_yaw.sin(), 0.0, self.player_yaw.cos());

        match atk.dispatch_kind() {
            AttackDispatchKind::GunshotTypeA => {
                // Single ray from muzzle (`shBattleAttackByHumanGunshotTypeA` `0x00120cf0`)
                let muzzle = self.player_position + Vec3::new(0.0, -1100.0, 0.0) + forward * 200.0;
                if let Some(hit_idx) = self.find_closest_ray_hit(
                    muzzle,
                    forward,
                    atk.min_range,
                    atk.max_range,
                    isolated_idx,
                ) {
                    self.enemies[hit_idx].receive_hit(atk);
                }
            }
            AttackDispatchKind::GunshotTypeB => {
                // 10 randomized 3D cone pellets (`shBattleAttackByHumanGunshotTypeB` `0x00120f30`)
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
                        self.enemies[hit_idx].receive_hit(atk);
                    }
                }
            }
            AttackDispatchKind::HumanFog => {
                // 5-ray Hyper Spray cone (`shGetHumanAttackSprayPos` `0x00121630` + `0x00121a40`)
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
                let mut hit_mask = vec![false; self.enemies.len()];

                for ray_dir in rays {
                    if let Some(hit_idx) =
                        self.find_closest_ray_hit(muzzle, ray_dir, min_r, max_r, isolated_idx)
                    {
                        if !hit_mask[hit_idx] {
                            hit_mask[hit_idx] = true;
                            self.enemies[hit_idx].receive_hyper_spray_hit(atk, color);
                        }
                    }
                }
            }
            AttackDispatchKind::HumanFight => {
                // Swept melee blade quad (`shBattleAttackByHumanFightType` `0x00121370`)
                // Weapon hand root sits ~200mm forward of torso center at shoulder/chest height.
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

                for (idx, enemy) in self.enemies.iter_mut().enumerate() {
                    if isolated_idx.is_some_and(|only| only != idx) || enemy.is_dead {
                        continue;
                    }
                    if enemy.hitbox().intersects_swept_quad(&quad) {
                        enemy.receive_hit(atk);
                    }
                }
            }
            AttackDispatchKind::HumanFinish => {
                // Stomp / Kick against downed enemy (`shBattleAttackByHumanFinish` `0x00121f40`)
                let feet_check = self.player_position + forward * 250.0;
                for (idx, enemy) in self.enemies.iter_mut().enumerate() {
                    if isolated_idx.is_some_and(|only| only != idx)
                        || !enemy.is_downed
                        || enemy.is_dead
                    {
                        continue;
                    }
                    let dist =
                        (Vec3::new(enemy.position.x, 0.0, enemy.position.z) - feet_check).length();
                    if dist <= (enemy.hitbox_radius * 1.5 + atk.max_range) {
                        enemy.receive_hit(atk);
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
        for (idx, enemy) in self.enemies.iter().enumerate() {
            if isolated_idx.is_some_and(|only| only != idx) || enemy.is_dead {
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
