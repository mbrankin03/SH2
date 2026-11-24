//! Player combat state machine (`CombatController` & `CombatMode`).
//!
//! # Ground Truth & Binary References
//! - **`PlayerRequestAttack` (`SLUS_202.28` `0x00110530` / `sh2pc.exe` `0x005343a0`):**
//!   Weapon attack request dispatch (Firearm dry-fire SFX `11030`/`11052`/`11049`, Plank light/heavy,
//!   Steel Pipe lunge/overhead/swing, Great Knife horizontal/overhead, Chainsaw swing/thrust,
//!   Hyper Spray burst/sustain, Maria Revolver, Maria Chinese Cleaver).
//! - **`PlayerCheckAttack` (`SLUS_202.28` `0x0021c920`):**
//!   Maps weapon grip + request code to `sh2_attack_list` IDs (`1..=40`).
//! - **`upper_attack` (`SLUS_202.28` `0x0021ef50` lines 1422–1427):**
//!   Hyper Spray shake recharge (`DAT_003c8416` starts at `200`, decrements by 1 per shake,
//!   sets charge to `(shake_count * 20.0) / 200.0`).
//! - **`PlayerCheckHuggingButton` (`SLUS_202.28` `0x0021c360`):**
//!   Grapple escape gauge drains ONLY on triggered button/stick inputs (`presses * dt`), with
//!   **zero** passive time decay.
//! - **`PlayerChectGuardSuccess` (`SLUS_202.28` `0x001101d0` / `sh2pc.exe` `0x00534210`):**
//!   Guard/Block evaluation gated by difficulty, attack ID, player mode, and facing angle.

use super::attack_list::{
    AttackSpec, HyperSprayColor, WeaponGrip, ATK_CHAINSAW_LIGHT, ATK_CHAINSAW_THRUST,
    ATK_GREAT_KNIFE_OVERHEAD_1, ATK_GREAT_KNIFE_OVERHEAD_2, ATK_GREAT_KNIFE_SWING_1,
    ATK_GREAT_KNIFE_SWING_2, ATK_HANDGUN, ATK_HUNTING_RIFLE, ATK_HYPER_SPRAY_1, ATK_HYPER_SPRAY_2,
    ATK_KICKS, ATK_MARIA_CLEAVER_COMBO, ATK_MARIA_CLEAVER_HEAVY, ATK_MARIA_CLEAVER_LIGHT,
    ATK_MARIA_REVOLVER_1, ATK_MARIA_REVOLVER_2, ATK_PIPE_COMBO, ATK_PIPE_LUNGE, ATK_PIPE_OVERHEAD,
    ATK_PIPE_SWING, ATK_PLANK_COMBO, ATK_PLANK_HEAVY, ATK_PLANK_LIGHT, ATK_SHOTGUN, ATK_STOMPS,
    SE_CHAINSAW_ATTACK, SE_FIREARM_RELOAD, SE_HANDGUN_SHOT, SE_HYPER_SPRAY_SHOT, SE_MELEE_SWISH,
    SE_PIPE_OR_FINISH, SE_RIFLE_SHOT, SE_SHOTGUN_SHOT,
};
use super::hitboxes::{
    HYPER_SPRAY_INITIAL_SHAKES, HYPER_SPRAY_MAX_CHARGE_SECS, SHOTGUN_PELLET_COUNT,
};
use super::targeting::{check_guard_success, GuardState};
use crate::config::{CombatInputMode, ControlOptions, PS2_HEAVY_PRESSURE_THRESHOLD_NORM};

/// Current combat posture / action mode of the player.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CombatMode {
    #[default]
    Free, // Normal locomotion / unready
    Raise,   // Raising weapon to aim posture
    Ready,   // Aiming / ready to attack
    Swing,   // Melee attack in progress
    Rebound, // Melee attack hit rebound recoil
    Recover, // Melee attack follow-through recovery
    Lower,   // Lowering weapon back to free
    Shot,    // Gun discharging (or Hyper Spray spraying)
    Reload,  // Gun reloading / Hyper Spray shaking animation
    Stomp,   // Stomping / kicking downed enemy
    Hurt,    // Taking damage / flinching
    Held,    // Grabbed by enemy (struggling)
    Dying,   // Dying scream / collapse
    Dead,    // Fully dead
}

/// Attack button tap-versus-hold timing threshold in seconds
/// (Enhanced Edition `HoldToStomp` and `SwapLightHeavyAttack`).
pub const HOLD_TIMER_THRESHOLD: f32 = 0.2;

/// Minimum PS2/PC DualShock 2 button pressure (`0xC1 = 193`, i.e., `0xC0 < bVar4`)
/// required to trigger Heavy melee swings and Two-Footed Stomps in `sh2pc.exe`
/// `FUN_005343a0` (`0x005343a0`) and `FUN_005345c0` (`0x005345c0`).
pub const PS2_PRESSURE_HEAVY_THRESHOLD: u8 = 0xC1;

/// Per-tick combat input snapshot supporting Vanilla PS2 pressure, SH2:EE tap/hold,
/// and Native zero-latency split bindings / `R2` analog trigger thresholds (`SH2-83`).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct CombatInputSnapshot {
    /// Weapon ready / aim button held (`R2` / `R1`).
    pub ready_held: bool,
    /// Primary attack button rising edge this tick.
    pub action_pressed: bool,
    /// Primary attack button currently held.
    pub action_held: bool,
    /// Whether a downed enemy is within stomp/kick range (`_DAT_003c83fc != 0`).
    pub has_downed_enemy_nearby: bool,
    /// Whether the player is currently running (`param_2 != 0` in `PlayerRequestAttack` `0x00110530` / `0x005343a0`).
    /// Forces light swing on `WoodenPlank` and `MariaChineseCleaver`.
    pub is_running: bool,
    /// Optional PS2 analog button pressure (`0..=255`) or `R2` trigger pressure (`r2 * 255.0`).
    pub attack_pressure: Option<u8>,
    /// Native zero-latency dedicated Light Attack / Quick Kick pulse (`CombatInputMode::NativeZeroLatency`).
    pub native_light_pressed: bool,
    /// Native zero-latency dedicated Heavy Attack / Two-Footed Stomp pulse (`CombatInputMode::NativeZeroLatency`).
    pub native_heavy_pressed: bool,
}

/// Player combat state machine and inventory ammunition tracking.
#[derive(Debug, Clone)]
pub struct CombatController {
    /// Equipped weapon.
    pub weapon: WeaponGrip,
    /// Weapon shoulder side (0 = right shoulder, 1 = left shoulder).
    pub shoulder_side: usize,
    /// Current combat action mode.
    pub mode: CombatMode,
    /// Current animation frame within active combat clip (30 Hz frames).
    pub anim_frame: f32,
    /// Total duration/length of current combat animation clip in 30 Hz frames.
    pub clip_length: f32,
    /// Speed of animation playback in PS2 fixed-point scale (2048 = 1.0x / 30fps).
    pub anim_speed: f32,
    /// Whether the current animation clip reached its end.
    pub anim_ended: bool,
    /// Active attack specification being executed.
    pub active_attack: Option<AttackSpec>,
    /// Whether the active attack was a heavy strike.
    pub is_heavy_attack: bool,
    /// Whether attack button is currently being charged for tap vs hold choice.
    pub charge_timer: f32,
    /// Whether pending charge choice is a stomp/kick (true) or melee light/heavy (false).
    pub charge_is_stomp: bool,
    /// Buffered attack chain / combo request.
    pub combo_requested: bool,
    /// Rounds currently loaded in firearms `[Handgun, Shotgun, Rifle, MariaRevolver]`.
    pub loaded_rounds: [u32; 4],
    /// Spare inventory rounds `[Handgun, Shotgun, Rifle, MariaRevolver]`.
    pub spare_rounds: [u32; 4],
    /// Frame at which rounds transfer during reload cycle.
    pub reload_transfer_frame: f32,
    /// Whether rounds have transferred during this reload.
    pub reload_transferred: bool,
    /// Remaining Hyper Spray pressurized charge in seconds (`0.0..=20.0`, `_DAT_003c83ec`).
    pub hyper_spray_charge: f32,
    /// Remaining Hyper Spray canister shakes (`0..=200`, `DAT_003c8416` in `0x0021ef50`).
    pub hyper_spray_shakes: u8,
    /// Unlocked Hyper Spray color/power tier (`DAT_0122b319` in `GameSaveSprayPower` `0x00290c20`).
    pub hyper_spray_color: HyperSprayColor,
    /// Persistent Guard/Block latch state (`DAT_003c83b7` / `DAT_01fb80b7`).
    pub guard_state: GuardState,
    /// Player current health (max 100.0 James Normal/Hard, 200.0 Easy; 80.0/160.0 Maria).
    pub health: f32,
    pub max_health: f32,
    /// Gradual health recovery pool from healing items.
    pub health_recovery_pool: f32,
    /// Temporary invulnerability timer after getting hit (seconds).
    pub invulnerability_timer: f32,
    /// Grab struggle counter meter (`_DAT_003c83d0`, in seconds).
    pub struggle_meter: f32,
    /// Low health red cross flashing pulse timer (0.0..1.0).
    pub low_health_pulse: f32,
    pub low_health_pulse_speed: f32,
    /// God mode cheat (blows take no damage).
    pub god_mode: bool,
    /// Muzzle flash duration timer (seconds).
    pub muzzle_flash_timer: f32,
    /// Enable Hold-To-Stomp (Enhanced Edition option).
    pub enable_hold_to_stomp: bool,
    /// Enable Swap-Light-Heavy-Attack (Enhanced Edition option).
    pub enable_swap_light_heavy: bool,
    /// Active melee/stomp dispatch mode (`Ps2Pressure`, `Sh2eeTapHold`, `NativeZeroLatency`) (`SH2-83`).
    pub combat_input_mode: CombatInputMode,
    /// Normalized `R2` trigger threshold (`0.0..=1.0`, default `193.0 / 255.0`) (`SH2-83`).
    pub r2_heavy_pressure_threshold: f32,
    /// Last audio cue emitted by combat state machine.
    pub pending_sfx: Option<u32>,
    /// Shotgun ray pellets count (`10` in `shBattleAttackByHumanGunshotTypeB` `0x00120f30`).
    pub shotgun_pellets: u32,
}

impl Default for CombatController {
    fn default() -> Self {
        Self::new(false, false)
    }
}

impl CombatController {
    /// Create new combat controller.
    /// `is_easy_diff`: whether action difficulty is Beginner or Easy.
    /// `is_maria`: whether active player is Maria.
    pub fn new(is_easy_diff: bool, is_maria: bool) -> Self {
        let max_hp = if is_maria {
            if is_easy_diff {
                160.0
            } else {
                80.0
            }
        } else if is_easy_diff {
            200.0
        } else {
            100.0
        };

        Self {
            weapon: if is_maria {
                WeaponGrip::MariaChineseCleaver
            } else {
                WeaponGrip::WoodenPlank
            },
            shoulder_side: 0,
            mode: CombatMode::Free,
            anim_frame: 0.0,
            clip_length: 30.0,
            anim_speed: 2048.0,
            anim_ended: false,
            active_attack: None,
            is_heavy_attack: false,
            charge_timer: -1.0,
            charge_is_stomp: false,
            combo_requested: false,
            loaded_rounds: [10, 6, 4, 10],
            spare_rounds: [30, 18, 12, 30],
            reload_transfer_frame: 21.0,
            reload_transferred: false,
            hyper_spray_charge: HYPER_SPRAY_MAX_CHARGE_SECS,
            hyper_spray_shakes: HYPER_SPRAY_INITIAL_SHAKES,
            hyper_spray_color: HyperSprayColor::White,
            guard_state: GuardState::default(),
            health: max_hp,
            max_health: max_hp,
            health_recovery_pool: 0.0,
            invulnerability_timer: 0.0,
            struggle_meter: 0.0,
            low_health_pulse: 1.0,
            low_health_pulse_speed: -0.03,
            god_mode: false,
            muzzle_flash_timer: 0.0,
            enable_hold_to_stomp: true,
            enable_swap_light_heavy: true,
            combat_input_mode: CombatInputMode::Sh2eeTapHold,
            r2_heavy_pressure_threshold: PS2_HEAVY_PRESSURE_THRESHOLD_NORM,
            pending_sfx: None,
            shotgun_pellets: SHOTGUN_PELLET_COUNT as u32,
        }
    }

    /// Index into gun ammo arrays for a weapon grip.
    fn gun_index(grip: WeaponGrip) -> Option<usize> {
        match grip {
            WeaponGrip::Handgun => Some(0),
            WeaponGrip::Shotgun => Some(1),
            WeaponGrip::HuntingRifle => Some(2),
            WeaponGrip::MariaRevolver => Some(3),
            _ => None,
        }
    }

    /// Rounds currently in magazine.
    pub fn loaded_ammo(&self) -> u32 {
        Self::gun_index(self.weapon).map_or(0, |idx| self.loaded_rounds[idx])
    }

    /// Spare ammo available.
    pub fn spare_ammo(&self) -> u32 {
        Self::gun_index(self.weapon).map_or(0, |idx| self.spare_rounds[idx])
    }

    /// Consume one round from active firearm. Returns true if round was fired.
    pub fn consume_ammo(&mut self) -> bool {
        if let Some(idx) = Self::gun_index(self.weapon) {
            if self.loaded_rounds[idx] > 0 {
                self.loaded_rounds[idx] -= 1;
                return true;
            }
        }
        false
    }

    /// Perform ammo transfer for reload from combat reserve (`spare_rounds`), or Hyper Spray
    /// canister shake recharge (`m3_play_common.c` `0x0021ef50` lines 1422–1427).
    pub fn execute_reload(&mut self) -> u32 {
        if self.weapon == WeaponGrip::HyperSpray {
            self.shake_hyper_spray();
            return 0;
        }
        if let Some(idx) = Self::gun_index(self.weapon) {
            let cap = self.weapon.magazine_capacity();
            let needed = cap.saturating_sub(self.loaded_rounds[idx]);
            let transfer = needed.min(self.spare_rounds[idx]);
            self.loaded_rounds[idx] += transfer;
            self.spare_rounds[idx] -= transfer;
            return transfer;
        }
        0
    }

    /// Shake the Hyper Spray canister to recharge pressure (`upper_attack`, `SLUS_202.28`
    /// `0x0021ef50` lines 1422–1427).
    ///
    /// Decrements `hyper_spray_shakes` by 1 (saturating at `0`) and recomputes
    /// `hyper_spray_charge = (hyper_spray_shakes as f32 * 20.0) / 200.0`.
    pub fn shake_hyper_spray(&mut self) -> f32 {
        if self.hyper_spray_shakes != 0 {
            self.hyper_spray_shakes -= 1;
        }
        self.hyper_spray_charge = (self.hyper_spray_shakes as f32 * HYPER_SPRAY_MAX_CHARGE_SECS)
            / (HYPER_SPRAY_INITIAL_SHAKES as f32);
        self.hyper_spray_charge
    }

    /// Directly loads up to `available_rounds` from the inventory into `grip`'s magazine
    /// without altering the currently equipped weapon or touching `spare_rounds`.
    pub fn load_magazine_rounds(&mut self, grip: WeaponGrip, available_rounds: u32) -> u32 {
        let Some(idx) = Self::gun_index(grip) else {
            return 0;
        };
        let cap = grip.magazine_capacity();
        let needed = cap.saturating_sub(self.loaded_rounds[idx]);
        let loaded = needed.min(available_rounds);
        self.loaded_rounds[idx] += loaded;
        loaded
    }

    /// Maps a Director's Cut inventory item ID (`ITEM_CATALOG` `sh2pc.exe` `0x892640`) to `WeaponGrip`.
    pub const fn weapon_grip_from_item_id(item_id: u8) -> Option<WeaponGrip> {
        match item_id {
            4 => Some(WeaponGrip::Handgun),
            6 => Some(WeaponGrip::Shotgun),
            8 => Some(WeaponGrip::HuntingRifle),
            10 => Some(WeaponGrip::MariaRevolver),
            12 => Some(WeaponGrip::HyperSpray),
            13 => Some(WeaponGrip::WoodenPlank),
            14 => Some(WeaponGrip::SteelPipe),
            15 => Some(WeaponGrip::GreatKnife),
            16 => Some(WeaponGrip::Chainsaw),
            17 => Some(WeaponGrip::MariaChineseCleaver),
            _ => None,
        }
    }

    /// Maps a `WeaponGrip` back to its canonical Director's Cut inventory item ID.
    pub const fn weapon_grip_to_item_id(grip: WeaponGrip) -> Option<u8> {
        match grip {
            WeaponGrip::None => None,
            WeaponGrip::Handgun => Some(4),
            WeaponGrip::Shotgun => Some(6),
            WeaponGrip::HuntingRifle => Some(8),
            WeaponGrip::MariaRevolver => Some(10),
            WeaponGrip::HyperSpray => Some(12),
            WeaponGrip::WoodenPlank => Some(13),
            WeaponGrip::SteelPipe => Some(14),
            WeaponGrip::GreatKnife => Some(15),
            WeaponGrip::Chainsaw => Some(16),
            WeaponGrip::MariaChineseCleaver => Some(17),
        }
    }

    /// Equip specified weapon.
    pub fn equip_weapon(&mut self, weapon: WeaponGrip) {
        if self.mode == CombatMode::Free || self.mode == CombatMode::Ready {
            let was_ready = self.mode == CombatMode::Ready;
            self.weapon = weapon;
            self.mode = if was_ready {
                CombatMode::Ready
            } else {
                CombatMode::Free
            };
            self.anim_frame = 0.0;
            self.active_attack = None;
            self.anim_ended = false;
            self.charge_timer = -1.0;
        }
    }

    /// Evaluate Guard/Block against an incoming enemy attack (`PlayerChectGuardSuccess`,
    /// `SLUS_202.28` `0x001101d0` / `sh2pc.exe` `0x00534210`).
    #[allow(clippy::too_many_arguments)]
    pub fn try_guard_incoming_attack(
        &mut self,
        enemy_attack_active: bool,
        difficulty: u8,
        guard_triggered: bool,
        guard_held: bool,
        incoming_attack_id: u32,
        player_yaw: f32,
        attacker_yaw: f32,
    ) -> bool {
        let player_can_guard = !matches!(
            self.mode,
            CombatMode::Hurt
                | CombatMode::Held
                | CombatMode::Dying
                | CombatMode::Dead
                | CombatMode::Stomp
        );
        check_guard_success(
            &mut self.guard_state,
            enemy_attack_active,
            difficulty,
            guard_triggered,
            guard_held,
            player_can_guard,
            incoming_attack_id,
            player_yaw,
            attacker_yaw,
        )
    }

    /// Calculate low health red cross HUD alpha (authentic PC formula `0x476730`).
    /// If HP >= 50%, alpha is 0.0. Below 50%, alpha pulses smoothly between 0.1 and 1.0.
    pub fn update_low_health_alpha(&mut self, dt: f32) -> f32 {
        if self.health >= self.max_health * 0.5 {
            self.low_health_pulse = 1.0;
            return 0.0;
        }

        self.low_health_pulse += self.low_health_pulse_speed * dt * 30.0;
        if self.low_health_pulse < 0.1 {
            self.low_health_pulse = 0.1;
            self.low_health_pulse_speed = -self.low_health_pulse_speed;
        } else if self.low_health_pulse > 1.0 {
            self.low_health_pulse = 1.0;
            self.low_health_pulse_speed = -self.low_health_pulse_speed;
        }

        let fraction_lost = (self.max_health - self.health) / self.max_health;
        (fraction_lost * self.low_health_pulse).clamp(0.0, 1.0)
    }

    /// Advance gradual health recovery from first aid / health drinks (`recover_update`).
    pub fn update_health_recovery(&mut self, dt: f32) {
        if self.health_recovery_pool <= 0.0 || self.health <= 0.0 {
            return;
        }
        let rate = 30.0 * dt; // 30 HP per second
        let step = rate.min(self.health_recovery_pool);
        self.health_recovery_pool -= step;
        self.health = (self.health + step).min(self.max_health);
        if self.health >= self.max_health {
            self.health_recovery_pool = 0.0;
        }
    }

    /// Player receives attack damage from enemy.
    pub fn apply_damage(&mut self, ap: f32, is_fog_spray: bool) {
        if self.god_mode
            || self.invulnerability_timer > 0.0
            || self.mode == CombatMode::Dead
            || self.mode == CombatMode::Dying
        {
            return;
        }

        self.health = (self.health - ap).max(0.0);
        self.invulnerability_timer = 1.0;

        if self.health <= 0.0 {
            self.mode = CombatMode::Dying;
            self.anim_frame = 0.0;
            self.clip_length = 25.0;
            self.pending_sfx = Some(11024);
            return;
        }

        if is_fog_spray && (self.mode == CombatMode::Ready || self.mode == CombatMode::Raise) {
            self.pending_sfx = Some(11020);
            return;
        }

        self.mode = CombatMode::Hurt;
        self.anim_frame = 0.0;
        self.clip_length = 18.0;
        self.pending_sfx = Some(11020 + (self.health as u32 % 4));
    }

    /// Enemy initiates grab on player (`PlayerCheckHuggingAttack` `0x00110070` / `0x005340b0`).
    pub fn apply_grab(&mut self, meter: f32) {
        if self.god_mode || self.mode == CombatMode::Dead || self.mode == CombatMode::Dying {
            return;
        }
        self.mode = CombatMode::Held;
        self.struggle_meter = meter;
        self.charge_timer = -1.0;
        self.anim_frame = 0.0;
    }

    /// Process struggle escape input during grab (`PlayerCheckHuggingButton`, `SLUS_202.28` `0x0021c360`).
    ///
    /// Authentic 1:1 behavior: each triggered button/stick input subtracts `dt` (`shGetDT()`) from
    /// `struggle_meter`. There is **zero** passive decay when `button_press_count == 0`.
    pub fn process_struggle_inputs(
        &mut self,
        button_press_count: u32,
        dt: f32,
        _is_beginner: bool,
    ) {
        if self.mode != CombatMode::Held {
            return;
        }

        if button_press_count > 0 {
            let drain_from_presses = (button_press_count as f32) * dt;
            self.struggle_meter = (self.struggle_meter - drain_from_presses).max(0.0);
        }

        if self.struggle_meter <= 0.0 {
            self.mode = CombatMode::Hurt;
            self.invulnerability_timer = 1.0;
            self.anim_frame = 0.0;
            self.clip_length = 18.0;
        }
    }

    /// Synchronizes combat input mode, tap/hold toggles, and `R2` threshold from [`ControlOptions`] (`SH2-83`).
    pub fn apply_control_options(&mut self, opts: &ControlOptions) {
        self.combat_input_mode = opts.combat_input_mode;
        self.r2_heavy_pressure_threshold = opts.r2_heavy_pressure_threshold.clamp(0.05, 1.0);
        match opts.combat_input_mode {
            CombatInputMode::Ps2Pressure => {
                self.enable_hold_to_stomp = false;
                self.enable_swap_light_heavy = false;
            }
            CombatInputMode::Sh2eeTapHold | CombatInputMode::NativeZeroLatency => {
                self.enable_hold_to_stomp = true;
                self.enable_swap_light_heavy = true;
            }
        }
    }

    /// Initiate weapon attack logic based on player input and target environment.
    pub fn handle_combat_inputs(
        &mut self,
        ready_held: bool,
        action_pressed: bool,
        action_held: bool,
        has_downed_enemy_nearby: bool,
        dt: f32,
    ) {
        self.handle_combat_inputs_ex(
            CombatInputSnapshot {
                ready_held,
                action_pressed,
                action_held,
                has_downed_enemy_nearby,
                ..CombatInputSnapshot::default()
            },
            dt,
        );
    }

    /// Extended combat input handler supporting PS2 analog pressure (`0xC0 < p`),
    /// SH2:EE tap/hold (`0.2s`), running light-attack override (`0x005343a0`), and
    /// Native zero-latency split bindings / `R2` trigger thresholds (`SH2-83`).
    pub fn handle_combat_inputs_ex(&mut self, input: CombatInputSnapshot, dt: f32) {
        let ready_held = input.ready_held;
        let action_pressed = input.action_pressed;
        let action_held = input.action_held;
        let has_downed_enemy_nearby = input.has_downed_enemy_nearby;
        self.pending_sfx = None;

        if self.invulnerability_timer > 0.0 {
            self.invulnerability_timer = (self.invulnerability_timer - dt).max(0.0);
        }
        if self.muzzle_flash_timer > 0.0 {
            self.muzzle_flash_timer = (self.muzzle_flash_timer - dt).max(0.0);
        }

        // Advance animation frame if in active combat clip
        if self.mode != CombatMode::Free
            && self.mode != CombatMode::Ready
            && self.mode != CombatMode::Held
        {
            self.anim_frame += dt * 30.0 * (self.anim_speed / 2048.0);
            if self.anim_frame >= self.clip_length {
                self.anim_frame = self.clip_length;
                self.anim_ended = true;
            }
        }

        match self.mode {
            CombatMode::Free => {
                if ready_held {
                    if self.weapon == WeaponGrip::None {
                        self.mode = CombatMode::Ready;
                    } else {
                        self.mode = CombatMode::Raise;
                        self.anim_frame = 0.0;
                        self.clip_length = 12.0;
                        self.anim_ended = false;
                    }
                }
            }
            CombatMode::Raise => {
                if !ready_held {
                    self.mode = CombatMode::Lower;
                    self.anim_frame = 0.0;
                    self.clip_length = 12.0;
                    self.anim_ended = false;
                } else if self.anim_ended {
                    self.mode = CombatMode::Ready;
                }
            }
            CombatMode::Ready => {
                if !ready_held {
                    self.charge_timer = -1.0;
                    if self.weapon == WeaponGrip::None {
                        self.mode = CombatMode::Free;
                    } else {
                        self.mode = CombatMode::Lower;
                        self.anim_frame = 0.0;
                        self.clip_length = 12.0;
                        self.anim_ended = false;
                    }
                    return;
                }

                // 1. Native zero-latency split bindings (0 ms frame-0 dispatch when enabled)
                if self.combat_input_mode == CombatInputMode::NativeZeroLatency
                    && (input.native_light_pressed || input.native_heavy_pressed)
                {
                    let want_heavy = input.native_heavy_pressed;
                    let can_stomp = self.weapon == WeaponGrip::None
                        || self.weapon.is_firearm()
                        || self.weapon == WeaponGrip::HyperSpray
                        || self.weapon == WeaponGrip::SteelPipe
                        || self.weapon == WeaponGrip::GreatKnife
                        || self.weapon == WeaponGrip::Chainsaw;

                    if can_stomp && has_downed_enemy_nearby {
                        self.start_stomp(want_heavy);
                        return;
                    } else if self.weapon.is_melee() {
                        let force_light = input.is_running
                            && matches!(
                                self.weapon,
                                WeaponGrip::WoodenPlank | WeaponGrip::MariaChineseCleaver
                            );
                        self.start_melee_attack(want_heavy && !force_light, false);
                        return;
                    }
                }

                if action_pressed {
                    let can_stomp = self.weapon == WeaponGrip::None
                        || self.weapon.is_firearm()
                        || self.weapon == WeaponGrip::HyperSpray
                        || self.weapon == WeaponGrip::SteelPipe
                        || self.weapon == WeaponGrip::GreatKnife
                        || self.weapon == WeaponGrip::Chainsaw;

                    // Pressure threshold check (`sh2pc.exe` `0x005343a0` / `0x005345c0`: `0xC0 < pressure`)
                    let pressure_threshold =
                        if self.combat_input_mode == CombatInputMode::NativeZeroLatency {
                            (self.r2_heavy_pressure_threshold * 255.0)
                                .round()
                                .clamp(1.0, 255.0) as u8
                        } else {
                            PS2_PRESSURE_HEAVY_THRESHOLD
                        };
                    let pressure_is_heavy = input.attack_pressure.map(|p| p >= pressure_threshold);

                    if can_stomp && has_downed_enemy_nearby {
                        if let Some(is_heavy) = pressure_is_heavy {
                            if self.combat_input_mode != CombatInputMode::Sh2eeTapHold {
                                self.start_stomp(is_heavy);
                                return;
                            }
                        }
                        if self.enable_hold_to_stomp
                            && self.combat_input_mode != CombatInputMode::Ps2Pressure
                        {
                            self.charge_timer = 0.0;
                            self.charge_is_stomp = true;
                        } else {
                            self.start_stomp(pressure_is_heavy.unwrap_or(true));
                        }
                    } else if self.weapon == WeaponGrip::HyperSpray {
                        if self.hyper_spray_charge > 0.0 {
                            self.start_hyper_spray(false);
                        } else if self.hyper_spray_shakes > 0 {
                            self.start_reload();
                        }
                    } else if self.weapon.is_melee() {
                        let force_light = input.is_running
                            && matches!(
                                self.weapon,
                                WeaponGrip::WoodenPlank | WeaponGrip::MariaChineseCleaver
                            );
                        if force_light {
                            self.start_melee_attack(false, false);
                        } else if has_downed_enemy_nearby && self.weapon == WeaponGrip::WoodenPlank
                        {
                            self.start_melee_attack(true, false);
                        } else if let Some(is_heavy) = pressure_is_heavy {
                            if self.combat_input_mode != CombatInputMode::Sh2eeTapHold {
                                self.start_melee_attack(is_heavy, false);
                                return;
                            }
                            if self.enable_swap_light_heavy {
                                self.charge_timer = 0.0;
                                self.charge_is_stomp = false;
                            } else {
                                self.start_melee_attack(is_heavy, false);
                            }
                        } else if self.enable_swap_light_heavy
                            && self.combat_input_mode != CombatInputMode::Ps2Pressure
                        {
                            self.charge_timer = 0.0;
                            self.charge_is_stomp = false;
                        } else {
                            self.start_melee_attack(true, false);
                        }
                    } else if self.weapon.is_firearm() {
                        if self.loaded_ammo() > 0 {
                            self.start_firearm_shot();
                        } else if self.spare_ammo() > 0 {
                            self.start_reload();
                        } else {
                            self.pending_sfx = self.weapon.dry_fire_sfx();
                        }
                    }
                } else if self.charge_timer >= 0.0 {
                    self.charge_timer += dt;
                    let threshold_reached = self.charge_timer >= HOLD_TIMER_THRESHOLD;
                    let button_released = !action_held;

                    if threshold_reached || button_released {
                        let is_hold = threshold_reached || action_held;
                        self.charge_timer = -1.0;

                        if self.charge_is_stomp {
                            self.start_stomp(is_hold);
                        } else {
                            let force_light = input.is_running
                                && matches!(
                                    self.weapon,
                                    WeaponGrip::WoodenPlank | WeaponGrip::MariaChineseCleaver
                                );
                            self.start_melee_attack(is_hold && !force_light, false);
                        }
                    }
                }
            }
            CombatMode::Swing => {
                if action_pressed {
                    self.combo_requested = true;
                }
                if self.anim_ended {
                    self.active_attack = None;
                    if self.combo_requested && ready_held {
                        self.combo_requested = false;
                        self.shoulder_side ^= 1;
                        self.start_melee_attack(false, true);
                    } else {
                        self.mode = CombatMode::Recover;
                        self.anim_frame = 0.0;
                        self.clip_length = 10.0;
                        self.anim_ended = false;
                    }
                }
            }
            CombatMode::Recover => {
                if action_pressed && ready_held {
                    self.shoulder_side ^= 1;
                    self.start_melee_attack(false, true);
                    return;
                }
                if self.anim_ended {
                    self.shoulder_side ^= 1;
                    self.mode = if ready_held {
                        CombatMode::Ready
                    } else {
                        CombatMode::Free
                    };
                }
            }
            CombatMode::Shot => {
                if self.weapon == WeaponGrip::HyperSpray {
                    // Drain Hyper Spray charge by dt (`shBattleAttackByHumanFog` `0x00121a40` line 913)
                    self.hyper_spray_charge = (self.hyper_spray_charge - dt).max(0.0);
                    if self.anim_ended {
                        if action_held && ready_held && self.hyper_spray_charge > 0.0 {
                            self.start_hyper_spray(true);
                        } else {
                            self.active_attack = None;
                            self.mode = if ready_held {
                                CombatMode::Ready
                            } else {
                                CombatMode::Free
                            };
                        }
                    }
                } else if self.anim_ended {
                    self.active_attack = None;
                    self.mode = if ready_held {
                        CombatMode::Ready
                    } else {
                        CombatMode::Free
                    };
                }
            }
            CombatMode::Reload => {
                if !self.reload_transferred && self.anim_frame >= self.reload_transfer_frame {
                    self.reload_transferred = true;
                    self.execute_reload();
                    self.pending_sfx = Some(SE_FIREARM_RELOAD);
                }
                if self.anim_ended {
                    self.mode = if ready_held {
                        CombatMode::Ready
                    } else {
                        CombatMode::Free
                    };
                }
            }
            CombatMode::Stomp => {
                if self.anim_ended {
                    self.active_attack = None;
                    self.mode = if ready_held {
                        CombatMode::Ready
                    } else {
                        CombatMode::Free
                    };
                }
            }
            CombatMode::Lower | CombatMode::Rebound | CombatMode::Hurt => {
                if self.anim_ended {
                    self.mode = CombatMode::Free;
                }
            }
            CombatMode::Dying => {
                if self.anim_ended {
                    self.mode = CombatMode::Dead;
                }
            }
            CombatMode::Dead | CombatMode::Held => {}
        }
    }

    /// Explicitly dispatch a Steel Pipe forward lunge thrust (`ATK_PIPE_LUNGE` ID 18,
    /// `PlayerRequestAttack` `SLUS_202.28` `0x00110530` case 6 -> `0x05` / `PlayerCheckAttack` `0x0021c920` line 521).
    pub fn start_pipe_lunge(&mut self) {
        if self.weapon != WeaponGrip::SteelPipe {
            return;
        }
        self.mode = CombatMode::Swing;
        self.anim_frame = 0.0;
        self.anim_ended = false;
        self.is_heavy_attack = false;
        self.active_attack = Some(ATK_PIPE_LUNGE);
        self.clip_length = 22.0;
        self.pending_sfx = Some(SE_PIPE_OR_FINISH);
    }

    fn start_stomp(&mut self, is_stomp: bool) {
        let kick_idx = self.weapon.kick_type();
        self.mode = CombatMode::Stomp;
        self.anim_frame = 0.0;
        self.anim_ended = false;
        self.is_heavy_attack = is_stomp;

        if is_stomp {
            self.active_attack = Some(ATK_STOMPS[kick_idx]);
            self.clip_length = 18.0;
            self.pending_sfx = Some(SE_PIPE_OR_FINISH);
        } else {
            self.active_attack = Some(ATK_KICKS[kick_idx]);
            self.clip_length = 20.0;
            self.pending_sfx = Some(SE_PIPE_OR_FINISH);
        }
    }

    fn start_melee_attack(&mut self, is_heavy: bool, is_combo: bool) {
        self.mode = CombatMode::Swing;
        self.anim_frame = 0.0;
        self.anim_ended = false;
        self.is_heavy_attack = is_heavy;

        match self.weapon {
            WeaponGrip::WoodenPlank => {
                if is_heavy {
                    self.active_attack = Some(ATK_PLANK_HEAVY);
                    self.clip_length = 28.0;
                } else if is_combo {
                    self.active_attack = Some(ATK_PLANK_COMBO);
                    self.clip_length = 18.0;
                } else {
                    self.active_attack = Some(ATK_PLANK_LIGHT);
                    self.clip_length = 22.0;
                }
                self.pending_sfx = Some(SE_MELEE_SWISH);
            }
            WeaponGrip::SteelPipe => {
                // `PlayerRequestAttack` (0x00110530 case 6) + `PlayerCheckAttack` (0x0021c920 lines 518-536):
                // - Stationary light tap -> Pipe Lunge (ID 18)
                // - Heavy hold -> Overhead swing (ID 17)
                // - Combo follow-up -> Pipe Swing / Combo (ID 15 / 16)
                if is_heavy {
                    self.active_attack = Some(ATK_PIPE_OVERHEAD);
                    self.clip_length = 34.0;
                } else if is_combo {
                    self.active_attack = Some(if self.shoulder_side == 0 {
                        ATK_PIPE_SWING
                    } else {
                        ATK_PIPE_COMBO
                    });
                    self.clip_length = 18.0;
                } else {
                    self.active_attack = Some(ATK_PIPE_LUNGE);
                    self.clip_length = 22.0;
                }
                self.pending_sfx = Some(SE_MELEE_SWISH);
            }
            WeaponGrip::GreatKnife => {
                // `PlayerCheckAttack` (0x0021c920 case 8 lines 569-588):
                // - Horizontal swing 1/2: IDs 19/20 (600 AP, 0..725 mm, multi-hit)
                // - Overhead chop 1/2: IDs 21/22 (1000 AP, 0..725 mm, multi-hit)
                if is_heavy {
                    self.active_attack = Some(if is_combo {
                        ATK_GREAT_KNIFE_OVERHEAD_2
                    } else {
                        ATK_GREAT_KNIFE_OVERHEAD_1
                    });
                    self.clip_length = 36.0;
                } else {
                    self.active_attack = Some(if is_combo {
                        ATK_GREAT_KNIFE_SWING_2
                    } else {
                        ATK_GREAT_KNIFE_SWING_1
                    });
                    self.clip_length = 28.0;
                }
                self.pending_sfx = Some(SE_MELEE_SWISH);
            }
            WeaponGrip::Chainsaw => {
                // `PlayerCheckAttack` (0x0021c920 case 7 lines 546-554):
                // - Light swing: ID 23 (250 AP, 0..525 mm, frames 12..=27, multi-hit)
                // - Heavy thrust: ID 24 (300 AP, 0..525 mm, frames 6..=13, multi-hit)
                if is_heavy {
                    self.active_attack = Some(ATK_CHAINSAW_THRUST);
                    self.clip_length = 26.0;
                } else {
                    self.active_attack = Some(ATK_CHAINSAW_LIGHT);
                    self.clip_length = 34.0;
                }
                self.pending_sfx = Some(SE_CHAINSAW_ATTACK);
            }
            WeaponGrip::MariaChineseCleaver => {
                // `sh2pc.exe` `sh2_attack_list[28..=30]` (`0x00629000`, multi-hit in `0x00543ec0`):
                // - Light: ID 28 (60 AP), Combo: ID 29 (35 AP), Heavy: ID 30 (60 AP)
                if is_heavy {
                    self.active_attack = Some(ATK_MARIA_CLEAVER_HEAVY);
                    self.clip_length = 24.0;
                } else if is_combo {
                    self.active_attack = Some(ATK_MARIA_CLEAVER_COMBO);
                    self.clip_length = 16.0;
                } else {
                    self.active_attack = Some(ATK_MARIA_CLEAVER_LIGHT);
                    self.clip_length = 20.0;
                }
                self.pending_sfx = Some(SE_MELEE_SWISH);
            }
            _ => {
                self.active_attack = Some(ATK_PLANK_LIGHT);
                self.clip_length = 22.0;
                self.pending_sfx = Some(SE_MELEE_SWISH);
            }
        }
    }

    fn start_hyper_spray(&mut self, is_sustained: bool) {
        self.mode = CombatMode::Shot;
        self.anim_frame = 0.0;
        self.anim_ended = false;
        self.active_attack = Some(if is_sustained {
            ATK_HYPER_SPRAY_2
        } else {
            ATK_HYPER_SPRAY_1
        });
        self.clip_length = if is_sustained { 20.0 } else { 26.0 };
        self.pending_sfx = Some(SE_HYPER_SPRAY_SHOT);
    }

    fn start_firearm_shot(&mut self) {
        if !self.consume_ammo() {
            return;
        }

        self.mode = CombatMode::Shot;
        self.anim_frame = 0.0;
        self.anim_ended = false;
        self.muzzle_flash_timer = 0.06;

        match self.weapon {
            WeaponGrip::Handgun => {
                self.active_attack = Some(ATK_HANDGUN);
                self.clip_length = 15.0;
                self.pending_sfx = Some(SE_HANDGUN_SHOT);
            }
            WeaponGrip::MariaRevolver => {
                self.active_attack = Some(if self.shoulder_side == 0 {
                    ATK_MARIA_REVOLVER_2
                } else {
                    ATK_MARIA_REVOLVER_1
                });
                self.clip_length = 15.0;
                self.pending_sfx = Some(SE_HANDGUN_SHOT);
            }
            WeaponGrip::Shotgun => {
                self.active_attack = Some(ATK_SHOTGUN);
                self.clip_length = 50.0;
                self.pending_sfx = Some(SE_SHOTGUN_SHOT);
            }
            WeaponGrip::HuntingRifle => {
                self.active_attack = Some(ATK_HUNTING_RIFLE);
                self.clip_length = 30.0;
                self.pending_sfx = Some(SE_RIFLE_SHOT);
            }
            _ => {}
        }
    }

    fn start_reload(&mut self) {
        self.mode = CombatMode::Reload;
        self.anim_frame = 0.0;
        self.anim_ended = false;
        self.reload_transferred = false;

        match self.weapon {
            WeaponGrip::Handgun | WeaponGrip::MariaRevolver => {
                self.clip_length = 40.0;
                self.reload_transfer_frame = 21.0;
            }
            WeaponGrip::Shotgun => {
                self.clip_length = 40.0;
                self.reload_transfer_frame = 12.0;
            }
            WeaponGrip::HuntingRifle => {
                self.clip_length = 40.0;
                self.reload_transfer_frame = 17.0;
            }
            WeaponGrip::HyperSpray => {
                // `sh2_attack_list[10]` standing shake clip (frames 0..30); recharge transfer occurs
                // at frame `7` (`0x1fe` standing shake; `3` for moving `0x1ff`) in `upper_attack`
                // (`SLUS_202.28` `0x0021ef50` / `sh2pc.exe` `sh2pc_00540000.c` `case '\x04'`).
                self.clip_length = 30.0;
                self.reload_transfer_frame = 7.0;
            }
            _ => {}
        }
    }
}
