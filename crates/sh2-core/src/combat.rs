//! Authentic Silent Hill 2 combat and weapon mechanics (PS2/PC 1:1 parity).
//!
//! # Parity & Ground Truth References:
//! - PlayStation 2 prototype `sh_character_battle.c`, `clani.c`, `combat.c`, `combat.h`, `ENEMIES.md`
//! - PC executable `sh2pc.exe` Director's Cut:
//!   - `0x88d2f0`: Weapon file table (grip IDs 0..10).
//!   - `0x534240`: Weapon grip to item ID mapping.
//!   - `0x5569b0`: `PlayerRequestAttack`: attack ID calculation from weapon grip, button tap/hold, and downed targets.
//!   - `0x52fae0` / `0x531a00`: Upper/lower attack state machine and animation mapping.
//!   - `0x558bd0`: Firearm reload cycling and ammo transfer frame offsets.
//!   - `0x543600` / `0x543810`: Gunshot raycasting (single ray for Handgun/Rifle, 10-pellet 30° cone for Shotgun).
//!   - `0x6ec568`: `sh2_attack_list` table containing AP (Attack Power), SP (Stamina/Stagger Power), and hit windows.
//!   - `0x476730`: Low-health flashing red cross indicator (threshold 50% HP, pulsing alpha formula).
//!   - `0x533d00` / `0x5565c0`: Grab escape struggle counter (`combat_grab_james`).
//! - Silent Hill 2: Enhanced Edition:
//!   - `Patches/HoldToStomp.cpp`: Digital button timing (0.2s hold for stomp vs tap for kick).
//!   - `Patches/SwapLightHeavyAttack.cpp`: 0.2s hold for heavy downswing vs tap for light swing.

use crate::ai::creatures::anim::CreatureAnimationController;
use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::f32::consts::PI;

/// Weapon grip classification matching PS2 `gWeaponHandGrip` (`0x88d2f0`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[repr(u8)]
pub enum WeaponGrip {
    #[default]
    None = 0,
    Handgun = 1,
    Shotgun = 2,
    HuntingRifle = 3,
    HyperSpray = 4,
    WoodenPlank = 5,
    SteelPipe = 6,
    Chainsaw = 7,
    GreatKnife = 8,
    MariaRevolver = 9,
    MariaChineseCleaver = 10,
}

impl WeaponGrip {
    /// Return the corresponding authentic item ID (`0x534240`).
    pub const fn item_id(&self) -> Option<u32> {
        match self {
            Self::None => None,
            Self::Handgun => Some(4),
            Self::Shotgun => Some(6),
            Self::HuntingRifle => Some(8),
            Self::HyperSpray => Some(12),
            Self::WoodenPlank => Some(13),
            Self::SteelPipe => Some(14),
            Self::GreatKnife => Some(15),
            Self::Chainsaw => Some(16),
            Self::MariaRevolver => Some(10),
            Self::MariaChineseCleaver => Some(17),
        }
    }

    /// Return the corresponding authentic ammo item ID if firearm.
    pub const fn ammo_item_id(&self) -> Option<u32> {
        match self {
            Self::Handgun => Some(5),        // Handgun bullets
            Self::Shotgun => Some(7),        // Shotgun shells
            Self::HuntingRifle => Some(9),   // Rifle shells
            Self::MariaRevolver => Some(11), // Revolver bullets
            _ => None,
        }
    }

    /// Magazine / cylinder capacity.
    pub const fn magazine_capacity(&self) -> u32 {
        match self {
            Self::Handgun => 10,
            Self::Shotgun => 6,
            Self::HuntingRifle => 4,
            Self::MariaRevolver => 10,
            _ => 0,
        }
    }

    /// Whether this weapon is a firearm.
    pub const fn is_firearm(&self) -> bool {
        matches!(
            self,
            Self::Handgun | Self::Shotgun | Self::HuntingRifle | Self::MariaRevolver
        )
    }

    /// Whether this weapon is a melee weapon.
    pub const fn is_melee(&self) -> bool {
        matches!(
            self,
            Self::WoodenPlank
                | Self::SteelPipe
                | Self::Chainsaw
                | Self::GreatKnife
                | Self::MariaChineseCleaver
        )
    }

    /// Default kick/stomp variation index (`0x53b190`).
    /// 0: bare hands / handgun / spray / maria knife
    /// 1: long guns (shotgun, rifle)
    /// 2: plank and pipe
    /// 3: great knife
    /// 4: chainsaw
    pub const fn kick_type(&self) -> usize {
        match self {
            Self::None
            | Self::Handgun
            | Self::HyperSpray
            | Self::MariaRevolver
            | Self::MariaChineseCleaver => 0,
            Self::Shotgun | Self::HuntingRifle => 1,
            Self::WoodenPlank | Self::SteelPipe => 2,
            Self::GreatKnife => 3,
            Self::Chainsaw => 4,
        }
    }
}

/// Attack definition from `sh2_attack_list` (`0x6ec568`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AttackSpec {
    pub id: u32,
    pub kind: u8,
    pub ap: f32, // Attack Power (health damage)
    pub sp: f32, // Stagger Power (endurance damage)
    pub min_range: f32,
    pub max_range: f32,
    pub first_hit_frame: u8,
    pub last_hit_frame: u8,
    pub sfx_id: u32,
}

/// Authentic PS2 / PC Kick attack specs (IDs 31..35).
pub const ATK_KICKS: [AttackSpec; 5] = [
    AttackSpec {
        id: 31,
        kind: 1,
        ap: 9999.9,
        sp: 0.1,
        min_range: 0.0,
        max_range: 350.0,
        first_hit_frame: 7,
        last_hit_frame: 10,
        sfx_id: 11040,
    },
    AttackSpec {
        id: 32,
        kind: 1,
        ap: 9999.9,
        sp: 0.1,
        min_range: 0.0,
        max_range: 350.0,
        first_hit_frame: 7,
        last_hit_frame: 10,
        sfx_id: 11040,
    },
    AttackSpec {
        id: 33,
        kind: 1,
        ap: 9999.9,
        sp: 0.1,
        min_range: 0.0,
        max_range: 350.0,
        first_hit_frame: 7,
        last_hit_frame: 10,
        sfx_id: 11040,
    },
    AttackSpec {
        id: 34,
        kind: 1,
        ap: 9999.9,
        sp: 0.1,
        min_range: 0.0,
        max_range: 350.0,
        first_hit_frame: 9,
        last_hit_frame: 9,
        sfx_id: 11040,
    },
    AttackSpec {
        id: 35,
        kind: 1,
        ap: 9999.9,
        sp: 0.1,
        min_range: 0.0,
        max_range: 350.0,
        first_hit_frame: 7,
        last_hit_frame: 11,
        sfx_id: 11040,
    },
];

/// Authentic PS2 / PC Stomp attack specs (IDs 36..40).
pub const ATK_STOMPS: [AttackSpec; 5] = [
    AttackSpec {
        id: 36,
        kind: 1,
        ap: 9999.9,
        sp: 0.1,
        min_range: 0.0,
        max_range: 350.0,
        first_hit_frame: 3,
        last_hit_frame: 9,
        sfx_id: 11040,
    },
    AttackSpec {
        id: 37,
        kind: 1,
        ap: 9999.9,
        sp: 0.1,
        min_range: 0.0,
        max_range: 350.0,
        first_hit_frame: 3,
        last_hit_frame: 9,
        sfx_id: 11040,
    },
    AttackSpec {
        id: 38,
        kind: 1,
        ap: 9999.9,
        sp: 0.1,
        min_range: 0.0,
        max_range: 350.0,
        first_hit_frame: 3,
        last_hit_frame: 9,
        sfx_id: 11040,
    },
    AttackSpec {
        id: 39,
        kind: 1,
        ap: 9999.9,
        sp: 0.1,
        min_range: 0.0,
        max_range: 350.0,
        first_hit_frame: 3,
        last_hit_frame: 9,
        sfx_id: 11040,
    },
    AttackSpec {
        id: 40,
        kind: 1,
        ap: 9999.9,
        sp: 0.1,
        min_range: 0.0,
        max_range: 350.0,
        first_hit_frame: 3,
        last_hit_frame: 9,
        sfx_id: 11040,
    },
];

/// Authentic Plank attacks (`sh2_attack_list`):
/// 12: Light swing (ap 100, sp 80)
/// 13: Combo swing (ap 80, sp 80)
/// 14: Downward heavy swing (ap 140, sp 80)
pub const ATK_PLANK_LIGHT: AttackSpec = AttackSpec {
    id: 12,
    kind: 2,
    ap: 100.0,
    sp: 80.0,
    min_range: -150.0,
    max_range: 350.0,
    first_hit_frame: 12,
    last_hit_frame: 15,
    sfx_id: 11032,
};
pub const ATK_PLANK_COMBO: AttackSpec = AttackSpec {
    id: 13,
    kind: 2,
    ap: 80.0,
    sp: 80.0,
    min_range: -150.0,
    max_range: 350.0,
    first_hit_frame: 8,
    last_hit_frame: 11,
    sfx_id: 11032,
};
pub const ATK_PLANK_HEAVY: AttackSpec = AttackSpec {
    id: 14,
    kind: 2,
    ap: 140.0,
    sp: 80.0,
    min_range: -150.0,
    max_range: 350.0,
    first_hit_frame: 8,
    last_hit_frame: 10,
    sfx_id: 11032,
};

/// Steel pipe attacks (`sh2_attack_list`):
/// 15: Swing (ap 120, sp 100)
/// 16: Short swing combo (ap 96, sp 100)
/// 17: Overhead heavy swing (ap 160, sp 100)
/// 18: Lunge forward thrust (ap 130, sp 50)
pub const ATK_PIPE_SWING: AttackSpec = AttackSpec {
    id: 15,
    kind: 2,
    ap: 120.0,
    sp: 100.0,
    min_range: -150.0,
    max_range: 540.0,
    first_hit_frame: 10,
    last_hit_frame: 12,
    sfx_id: 11040,
};
pub const ATK_PIPE_COMBO: AttackSpec = AttackSpec {
    id: 16,
    kind: 2,
    ap: 96.0,
    sp: 100.0,
    min_range: -150.0,
    max_range: 540.0,
    first_hit_frame: 4,
    last_hit_frame: 6,
    sfx_id: 11040,
};
pub const ATK_PIPE_OVERHEAD: AttackSpec = AttackSpec {
    id: 17,
    kind: 2,
    ap: 160.0,
    sp: 100.0,
    min_range: -150.0,
    max_range: 540.0,
    first_hit_frame: 15,
    last_hit_frame: 17,
    sfx_id: 11040,
};
pub const ATK_PIPE_LUNGE: AttackSpec = AttackSpec {
    id: 18,
    kind: 2,
    ap: 130.0,
    sp: 50.0,
    min_range: -150.0,
    max_range: 540.0,
    first_hit_frame: 8,
    last_hit_frame: 10,
    sfx_id: 11040,
};

/// Firearm attack specs (`0x543600`, `0x543810`):
pub const ATK_HANDGUN: AttackSpec = AttackSpec {
    id: 2,
    kind: 0,
    ap: 120.0,
    sp: 60.0,
    min_range: -100.0,
    max_range: 7500.0,
    first_hit_frame: 0,
    last_hit_frame: 10,
    sfx_id: 11048,
};
pub const ATK_SHOTGUN: AttackSpec = AttackSpec {
    id: 4,
    kind: 0,
    ap: 100.0,
    sp: 120.0,
    min_range: -100.0,
    max_range: 5000.0,
    first_hit_frame: 0,
    last_hit_frame: 10,
    sfx_id: 11051,
};
pub const ATK_HUNTING_RIFLE: AttackSpec = AttackSpec {
    id: 6,
    kind: 0,
    ap: 500.0,
    sp: 150.0,
    min_range: -350.0,
    max_range: 9000.0,
    first_hit_frame: 0,
    last_hit_frame: 10,
    sfx_id: 11048,
};

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
    Shot,    // Gun discharging
    Reload,  // Gun reloading animation
    Stomp,   // Stomping / kicking downed enemy
    Hurt,    // Taking damage / flinching
    Held,    // Grabbed by enemy (struggling)
    Dying,   // Dying scream / collapse
    Dead,    // Fully dead
}

/// Attack button tap-versus-hold timing state (Enhanced Edition `HoldToStomp` and `SwapLightHeavyAttack`).
pub const HOLD_TIMER_THRESHOLD: f32 = 0.2; // 200 ms threshold

/// Player combat state machine and inventory ammunition tracking.
#[derive(Debug, Clone)]
pub struct CombatController {
    /// Equipped weapon.
    pub weapon: WeaponGrip,
    /// Weapon shoulder side (0 = right shoulder, 1 = left shoulder).
    pub shoulder_side: usize,
    /// Current combat action mode.
    pub mode: CombatMode,
    /// Current animation frame within active combat clip.
    pub anim_frame: f32,
    /// Total duration/length of current combat animation clip in frames.
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
    /// Rounds currently loaded in firearms.
    pub loaded_rounds: [u32; 4], // [Handgun, Shotgun, Rifle, Revolver]
    /// Spare inventory rounds.
    pub spare_rounds: [u32; 4], // [Handgun, Shotgun, Rifle, Revolver]
    /// Frame at which rounds transfer during reload cycle.
    pub reload_transfer_frame: f32,
    /// Whether rounds have transferred during this reload.
    pub reload_transferred: bool,
    /// Player current health (max 100.0 James on Normal/Hard, 200.0 on Easy, 80/160 Maria).
    pub health: f32,
    pub max_health: f32,
    /// Gradual health recovery pool from healing items.
    pub health_recovery_pool: f32,
    /// Temporary invulnerability timer after getting hit (seconds).
    pub invulnerability_timer: f32,
    /// Grab struggle counter meter (`combat_grab_james`, in seconds of frame time).
    pub struggle_meter: f32,
    /// Low health red cross flashing pulse timer (0.0..1.0).
    pub low_health_pulse: f32,
    pub low_health_pulse_speed: f32,
    /// God mode cheat (blows take no damage).
    pub god_mode: bool,
    /// Muzzle flash duration timer.
    pub muzzle_flash_timer: f32,
    /// Enable Hold-To-Stomp (Enhanced Edition option).
    pub enable_hold_to_stomp: bool,
    /// Enable Swap-Light-Heavy-Attack (Enhanced Edition option).
    pub enable_swap_light_heavy: bool,
    /// Last audio cue emitted by combat state machine.
    pub pending_sfx: Option<u32>,
    /// Shotgun ray pellets count (10).
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
        } else {
            if is_easy_diff {
                200.0
            } else {
                100.0
            }
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
            loaded_rounds: [10, 6, 4, 10],  // Full clips for each gun
            spare_rounds: [30, 18, 12, 30], // Starting spare reserves
            reload_transfer_frame: 21.0,
            reload_transferred: false,
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
            pending_sfx: None,
            shotgun_pellets: 10,
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

    /// Perform ammo transfer for reload.
    pub fn execute_reload(&mut self) -> u32 {
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
        self.invulnerability_timer = 1.0; // 1.0 second authentic invulnerability window

        if self.health <= 0.0 {
            self.mode = CombatMode::Dying;
            self.anim_frame = 0.0;
            self.clip_length = 25.0; // Death collapse animation
            self.pending_sfx = Some(11024); // Dying scream
            return;
        }

        // Acid spray burns without dropping guard if ready
        if is_fog_spray && (self.mode == CombatMode::Ready || self.mode == CombatMode::Raise) {
            self.pending_sfx = Some(11020);
            return;
        }

        self.mode = CombatMode::Hurt;
        self.anim_frame = 0.0;
        self.clip_length = 18.0; // Flinch animation
        self.pending_sfx = Some(11020 + (self.health as u32 % 4));
    }

    /// Enemy initiates grab on player (`combat_grab_james`).
    pub fn apply_grab(&mut self, meter: f32) {
        if self.god_mode || self.mode == CombatMode::Dead || self.mode == CombatMode::Dying {
            return;
        }
        self.mode = CombatMode::Held;
        self.struggle_meter = meter;
        self.charge_timer = -1.0;
        self.anim_frame = 0.0;
    }

    /// Process struggle escape input during grab (`0x5565c0` / `shBattleHuggedHuman` 0x00124db0).
    /// In authentic PS2, button presses and stick inputs shave ~0.12 - 0.25 s per input from the 4.0s hold meter,
    /// allowing ~15-20 rapid mashing presses to break free within ~1-2 seconds,
    /// while passive time decay ensures release after 3.5-4.0s even with zero input.
    pub fn process_struggle_inputs(&mut self, button_press_count: u32, dt: f32, is_beginner: bool) {
        if self.mode != CombatMode::Held {
            return;
        }

        let drain_from_presses = (button_press_count as f32) * 0.135;
        let passive_rate = if is_beginner { 1.5 } else { 1.0 };
        let passive_drain = dt * passive_rate;
        self.struggle_meter = (self.struggle_meter - (drain_from_presses + passive_drain)).max(0.0);

        if self.struggle_meter <= 0.0 {
            self.mode = CombatMode::Hurt;
            self.invulnerability_timer = 1.0;
            self.anim_frame = 0.0;
            self.clip_length = 18.0;
        }
    }

    /// Initiate weapon attack logic based on player input and target environment.
    /// `ready_held`: R1/R2 ready button held
    /// `action_pressed`: X/Square attack button pressed this frame
    /// `action_held`: Attack button held
    /// `has_downed_enemy_nearby`: whether a fallen monster is at James' feet
    pub fn handle_combat_inputs(
        &mut self,
        ready_held: bool,
        action_pressed: bool,
        action_held: bool,
        has_downed_enemy_nearby: bool,
        dt: f32,
    ) {
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
                        self.clip_length = 12.0; // Raise weapon clip
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

                if action_pressed {
                    let can_stomp = self.weapon == WeaponGrip::None
                        || self.weapon.is_firearm()
                        || self.weapon == WeaponGrip::SteelPipe
                        || self.weapon == WeaponGrip::GreatKnife
                        || self.weapon == WeaponGrip::Chainsaw;

                    if can_stomp && has_downed_enemy_nearby {
                        if self.enable_hold_to_stomp {
                            self.charge_timer = 0.0;
                            self.charge_is_stomp = true;
                        } else {
                            self.start_stomp(true);
                        }
                    } else if self.weapon.is_melee() {
                        if has_downed_enemy_nearby && self.weapon == WeaponGrip::WoodenPlank {
                            // Downed targets automatically get the downward heavy swing
                            self.start_melee_attack(true, false);
                        } else if self.enable_swap_light_heavy {
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
                            self.pending_sfx = Some(11030); // Dry-fire hammer click
                        }
                    }
                } else if self.charge_timer >= 0.0 {
                    // Handle tap vs hold charge timer (Enhanced Edition HoldToStomp & SwapLightHeavyAttack)
                    self.charge_timer += dt;
                    let threshold_reached = self.charge_timer >= HOLD_TIMER_THRESHOLD;
                    let button_released = !action_held;

                    if threshold_reached || button_released {
                        let is_hold = threshold_reached || action_held;
                        self.charge_timer = -1.0;

                        if self.charge_is_stomp {
                            self.start_stomp(is_hold);
                        } else {
                            self.start_melee_attack(is_hold, false);
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
                        self.shoulder_side ^= 1; // Swap shoulder for combo swing
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
                if self.anim_ended {
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
                    self.pending_sfx = Some(11029); // Reload sound
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

    fn start_stomp(&mut self, is_stomp: bool) {
        let kick_idx = self.weapon.kick_type();
        self.mode = CombatMode::Stomp;
        self.anim_frame = 0.0;
        self.anim_ended = false;
        self.is_heavy_attack = is_stomp;

        if is_stomp {
            self.active_attack = Some(ATK_STOMPS[kick_idx]);
            self.clip_length = 18.0; // Stomp anim length
            self.pending_sfx = Some(11040);
        } else {
            self.active_attack = Some(ATK_KICKS[kick_idx]);
            self.clip_length = 20.0; // Kick anim length
            self.pending_sfx = Some(11040);
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
                self.pending_sfx = Some(11027); // Plank swish sound
            }
            WeaponGrip::SteelPipe => {
                if is_heavy {
                    self.active_attack = Some(ATK_PIPE_OVERHEAD);
                    self.clip_length = 34.0;
                } else if is_combo {
                    self.active_attack = Some(ATK_PIPE_COMBO);
                    self.clip_length = 18.0;
                } else {
                    self.active_attack = Some(ATK_PIPE_SWING);
                    self.clip_length = 24.0;
                }
                self.pending_sfx = Some(11027);
            }
            _ => {
                self.active_attack = Some(ATK_PLANK_LIGHT);
                self.clip_length = 22.0;
                self.pending_sfx = Some(11027);
            }
        }
    }

    fn start_firearm_shot(&mut self) {
        if !self.consume_ammo() {
            return;
        }

        self.mode = CombatMode::Shot;
        self.anim_frame = 0.0;
        self.anim_ended = false;
        self.muzzle_flash_timer = 0.06; // 60 ms muzzle flash

        match self.weapon {
            WeaponGrip::Handgun | WeaponGrip::MariaRevolver => {
                self.active_attack = Some(ATK_HANDGUN);
                self.clip_length = 15.0;
                self.pending_sfx = Some(11048);
            }
            WeaponGrip::Shotgun => {
                self.active_attack = Some(ATK_SHOTGUN);
                self.clip_length = 50.0;
                self.pending_sfx = Some(11051);
            }
            WeaponGrip::HuntingRifle => {
                self.active_attack = Some(ATK_HUNTING_RIFLE);
                self.clip_length = 30.0;
                self.pending_sfx = Some(11048);
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
            _ => {}
        }
    }
}

/// 3D Collision Hitbox Capsule / Cylinder for characters and enemies (`clDbgDrawHitPolygon`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CombatHitbox {
    pub center_x: f32,
    pub center_z: f32,
    pub radius: f32,
    pub top_y: f32,    // In PS2 world units, Y is negative-up (-1800.0 is head level)
    pub bottom_y: f32, // Floor level (0.0)
    pub is_downed: bool,
}

impl CombatHitbox {
    /// Create vertical character hitbox standing at (x, y, z).
    pub fn new_standing(pos: Vec3, radius: f32, height: f32) -> Self {
        Self {
            center_x: pos.x,
            center_z: pos.z,
            radius,
            top_y: pos.y - height,
            bottom_y: pos.y,
            is_downed: false,
        }
    }

    /// Create downed / fallen character hitbox (wide horizontal torso cylinder).
    pub fn new_downed(pos: Vec3, radius: f32, height: f32) -> Self {
        Self {
            center_x: pos.x,
            center_z: pos.z,
            radius: radius * 1.5,
            top_y: pos.y - height * 0.3,
            bottom_y: pos.y,
            is_downed: true,
        }
    }

    /// Test if a 2D line segment (e.g. plank swing from hand root to tip) intersects this cylinder.
    pub fn intersects_segment(&self, a: Vec3, b: Vec3) -> bool {
        let seg_y_min = a.y.min(b.y);
        let seg_y_max = a.y.max(b.y);
        let cyl_y_min = self.top_y.min(self.bottom_y);
        let cyl_y_max = self.top_y.max(self.bottom_y);
        if seg_y_max < cyl_y_min || seg_y_min > cyl_y_max {
            return false;
        }

        // Distance from point to segment in XZ plane
        let vx = b.x - a.x;
        let vz = b.z - a.z;
        let wx = self.center_x - a.x;
        let wz = self.center_z - a.z;
        let l_sq = vx * vx + vz * vz;

        let t = if l_sq > 0.0 {
            ((wx * vx + wz * vz) / l_sq).clamp(0.0, 1.0)
        } else {
            0.0
        };

        let proj_x = a.x + vx * t;
        let proj_z = a.z + vz * t;
        let dx = proj_x - self.center_x;
        let dz = proj_z - self.center_z;

        (dx * dx + dz * dz) <= (self.radius * self.radius)
    }

    /// Test if a ray (e.g. gunshot) intersects this cylinder between min_dist and max_dist.
    /// Returns distance `t` along ray direction if hit occurs.
    pub fn intersects_ray(
        &self,
        origin: Vec3,
        dir: Vec3,
        min_dist: f32,
        max_dist: f32,
    ) -> Option<f32> {
        let hx = dir.x;
        let hz = dir.z;
        let h2 = hx * hx + hz * hz;
        if h2 < 1e-6 {
            return None;
        }

        let wx = self.center_x - origin.x;
        let wz = self.center_z - origin.z;
        let t = (wx * hx + wz * hz) / h2;

        let px = origin.x + hx * t - self.center_x;
        let pz = origin.z + hz * t - self.center_z;
        let miss2 = px * px + pz * pz;

        if miss2 > self.radius * self.radius {
            return None;
        }

        let back = ((self.radius * self.radius - miss2) / h2).sqrt();
        let t_enter = (t - back).max(min_dist);

        if t_enter > max_dist {
            return None;
        }

        let y_at_hit = origin.y + dir.y * t_enter;
        let cyl_y_min = self.top_y.min(self.bottom_y);
        let cyl_y_max = self.top_y.max(self.bottom_y);
        if y_at_hit < cyl_y_min - 50.0 || y_at_hit > cyl_y_max + 50.0 {
            return None;
        }

        Some(t_enter)
    }
}

/// Simulated monster/enemy in combat room for testing attack interactions.
#[derive(Debug, Clone)]
pub struct CombatTargetEnemy {
    pub id: u32,
    pub name: &'static str,
    pub position: Vec3,
    pub yaw: f32,
    pub spawn_origin: Vec3,
    pub aggro_radius: f32,
    pub is_aggroed: bool,
    pub health: f32,
    pub max_health: f32,
    pub endurance: f32,
    pub max_endurance: f32,
    pub is_downed: bool,
    pub is_dead: bool,
    pub flinch_timer: f32,
    pub down_timer: f32,
    pub attack_cooldown: f32,
    pub attack_timer: f32,
    pub is_attacking: bool,
    pub is_kick_next: bool,
    pub is_grappling_player: bool,
    pub hitbox_radius: f32,
    pub hitbox_height: f32,
    pub anim: CreatureAnimationController,
}

impl CombatTargetEnemy {
    /// Create a Lying Figure (scu) test target with authentic Normal difficulty values.
    /// Health: 700, Endurance: 285 (`0x4c46c0`).
    pub fn new_lying_figure(id: u32, position: Vec3) -> Self {
        Self {
            id,
            name: "Lying Figure (scu)",
            position,
            yaw: 0.0,
            spawn_origin: position,
            aggro_radius: 1800.0,
            is_aggroed: false,
            health: 700.0,
            max_health: 700.0,
            endurance: 285.0,
            max_endurance: 285.0,
            is_downed: false,
            is_dead: false,
            flinch_timer: 0.0,
            down_timer: 0.0,
            attack_cooldown: 0.5,
            attack_timer: 0.0,
            is_attacking: false,
            is_kick_next: false,
            is_grappling_player: false,
            hitbox_radius: 350.0,
            hitbox_height: 1500.0,
            anim: CreatureAnimationController::new_lying_figure(),
        }
    }

    /// Create a Mannequin (mkn) test target with authentic Normal difficulty values.
    /// Health: 490 (700 * 0.7), Endurance: 200 (`0x4c15c0`).
    pub fn new_mannequin(id: u32, position: Vec3) -> Self {
        Self {
            id,
            name: "Mannequin (mkn)",
            position,
            yaw: 0.0,
            spawn_origin: position,
            aggro_radius: 1800.0,
            is_aggroed: false,
            health: 490.0,
            max_health: 490.0,
            endurance: 200.0,
            max_endurance: 200.0,
            is_downed: false,
            is_dead: false,
            flinch_timer: 0.0,
            down_timer: 0.0,
            attack_cooldown: 0.8,
            attack_timer: 0.0,
            is_attacking: false,
            is_kick_next: false,
            is_grappling_player: false,
            hitbox_radius: 300.0,
            hitbox_height: 1600.0,
            anim: CreatureAnimationController::new_mannequin(),
        }
    }

    /// Create a Bubble Head Nurse (nse) test target with authentic Normal difficulty values.
    /// Health: 600, Endurance: 260.
    pub fn new_nurse(id: u32, position: Vec3) -> Self {
        Self {
            id,
            name: "Bubble Head Nurse (nse)",
            position,
            yaw: 0.0,
            spawn_origin: position,
            aggro_radius: 1800.0,
            is_aggroed: false,
            health: 600.0,
            max_health: 600.0,
            endurance: 260.0,
            max_endurance: 260.0,
            is_downed: false,
            is_dead: false,
            flinch_timer: 0.0,
            down_timer: 0.0,
            attack_cooldown: 0.6,
            attack_timer: 0.0,
            is_attacking: false,
            is_kick_next: false,
            is_grappling_player: false,
            hitbox_radius: 320.0,
            hitbox_height: 1600.0,
            anim: CreatureAnimationController::new_nurse(),
        }
    }

    /// Create a Pyramid Head (red) boss target.
    /// Health: 2000, Endurance: 1000.
    pub fn new_pyramid_head(id: u32, position: Vec3) -> Self {
        Self {
            id,
            name: "Pyramid Head (red)",
            position,
            yaw: 0.0,
            spawn_origin: position,
            aggro_radius: 2000.0,
            is_aggroed: false,
            health: 2000.0,
            max_health: 2000.0,
            endurance: 1000.0,
            max_endurance: 1000.0,
            is_downed: false,
            is_dead: false,
            flinch_timer: 0.0,
            down_timer: 0.0,
            attack_cooldown: 1.0,
            attack_timer: 0.0,
            is_attacking: false,
            is_kick_next: false,
            is_grappling_player: false,
            hitbox_radius: 450.0,
            hitbox_height: 2200.0,
            anim: CreatureAnimationController::new_pyramid_head(),
        }
    }

    /// Create an Abstract Daddy (pap) boss target.
    /// Health: 1800, Endurance: 800.
    pub fn new_abstract_daddy(id: u32, position: Vec3) -> Self {
        Self {
            id,
            name: "Abstract Daddy (pap)",
            position,
            yaw: 0.0,
            spawn_origin: position,
            aggro_radius: 2000.0,
            is_aggroed: false,
            health: 1800.0,
            max_health: 1800.0,
            endurance: 800.0,
            max_endurance: 800.0,
            is_downed: false,
            is_dead: false,
            flinch_timer: 0.0,
            down_timer: 0.0,
            attack_cooldown: 0.8,
            attack_timer: 0.0,
            is_attacking: false,
            is_kick_next: false,
            is_grappling_player: false,
            hitbox_radius: 500.0,
            hitbox_height: 1800.0,
            anim: CreatureAnimationController::new_abstract_daddy(),
        }
    }

    /// Create a Flesh Lips (ike) boss target.
    /// Health: 1200, Endurance: 600.
    pub fn new_flesh_lips(id: u32, position: Vec3) -> Self {
        Self {
            id,
            name: "Flesh Lips (ike)",
            position,
            yaw: 0.0,
            spawn_origin: position,
            aggro_radius: 2000.0,
            is_aggroed: false,
            health: 1200.0,
            max_health: 1200.0,
            endurance: 600.0,
            max_endurance: 600.0,
            is_downed: false,
            is_dead: false,
            flinch_timer: 0.0,
            down_timer: 0.0,
            attack_cooldown: 1.0,
            attack_timer: 0.0,
            is_attacking: false,
            is_kick_next: false,
            is_grappling_player: false,
            hitbox_radius: 400.0,
            hitbox_height: 1800.0,
            anim: CreatureAnimationController::new_flesh_lips(),
        }
    }

    /// Create a Mandarin (arm) target placed above ground for animation visibility.
    /// Health: 500, Endurance: 250.
    pub fn new_mandarin(id: u32, position: Vec3) -> Self {
        Self {
            id,
            name: "Mandarin (arm)",
            position,
            yaw: 0.0,
            spawn_origin: position,
            aggro_radius: 1800.0,
            is_aggroed: false,
            health: 500.0,
            max_health: 500.0,
            endurance: 250.0,
            max_endurance: 250.0,
            is_downed: false,
            is_dead: false,
            flinch_timer: 0.0,
            down_timer: 0.0,
            attack_cooldown: 0.7,
            attack_timer: 0.0,
            is_attacking: false,
            is_kick_next: false,
            is_grappling_player: false,
            hitbox_radius: 350.0,
            hitbox_height: 1400.0,
            anim: CreatureAnimationController::new_mandarin(),
        }
    }

    /// Create a Creeper (tyu) insect swarm target.
    /// Health: 50, Endurance: 20 (dies in 1 stomp or plank hit).
    pub fn new_creeper(id: u32, position: Vec3) -> Self {
        Self {
            id,
            name: "Creeper (tyu)",
            position,
            yaw: 0.0,
            spawn_origin: position,
            aggro_radius: 1600.0,
            is_aggroed: false,
            health: 50.0,
            max_health: 50.0,
            endurance: 20.0,
            max_endurance: 20.0,
            is_downed: false,
            is_dead: false,
            flinch_timer: 0.0,
            down_timer: 0.0,
            attack_cooldown: 0.4,
            attack_timer: 0.0,
            is_attacking: false,
            is_kick_next: false,
            is_grappling_player: false,
            hitbox_radius: 150.0,
            hitbox_height: 200.0,
            anim: CreatureAnimationController::new_creeper(),
        }
    }

    /// Create a Mary / Maria Final Boss (bos) target.
    /// Health: 2500, Endurance: 1200.
    pub fn new_mary_boss(id: u32, position: Vec3) -> Self {
        Self {
            id,
            name: "Mary Final Boss (bos)",
            position,
            yaw: 0.0,
            spawn_origin: position,
            aggro_radius: 2200.0,
            is_aggroed: false,
            health: 2500.0,
            max_health: 2500.0,
            endurance: 1200.0,
            max_endurance: 1200.0,
            is_downed: false,
            is_dead: false,
            flinch_timer: 0.0,
            down_timer: 0.0,
            attack_cooldown: 1.2,
            attack_timer: 0.0,
            is_attacking: false,
            is_kick_next: false,
            is_grappling_player: false,
            hitbox_radius: 500.0,
            hitbox_height: 2400.0,
            anim: CreatureAnimationController::new_mary_boss(),
        }
    }

    /// Get current active collision hitbox.
    pub fn hitbox(&self) -> CombatHitbox {
        if self.is_downed {
            CombatHitbox::new_downed(self.position, self.hitbox_radius, self.hitbox_height)
        } else {
            CombatHitbox::new_standing(self.position, self.hitbox_radius, self.hitbox_height)
        }
    }

    /// Apply weapon damage (AP) and stagger (SP) (`0x4c65d0`).
    pub fn receive_hit(&mut self, atk: &AttackSpec) {
        if self.is_dead {
            return;
        }

        self.health = (self.health - atk.ap).max(0.0);
        self.endurance = (self.endurance - atk.sp).max(0.0);
        self.is_attacking = false;
        self.attack_timer = 0.0;
        self.is_aggroed = true; // Retaliate immediately if struck

        if self.is_downed {
            // Attacks against downed enemies finish them off or trigger death twitch
            if self.health <= 0.0 || atk.ap >= 9999.0 {
                self.is_dead = true;
            }
            return;
        }

        if self.health <= 0.0 || self.endurance <= 0.0 {
            self.is_downed = true;
            self.down_timer = 10.0; // Stays down for 10 seconds (300 ticks)
        } else {
            self.flinch_timer = 0.5; // Flinch stagger for 0.5s
        }

        // Sync creature animation state
        if self.id == 1 {
            let scu_state = if self.is_dead || self.is_downed {
                crate::ai::creatures::lying_figure::ScuState::Down
            } else if self.flinch_timer > 0.0 {
                crate::ai::creatures::lying_figure::ScuState::Damage
            } else if self.is_attacking {
                crate::ai::creatures::lying_figure::ScuState::Attack
            } else {
                crate::ai::creatures::lying_figure::ScuState::Chase
            };
            self.anim.sync_lying_figure(scu_state);
        } else {
            let mkn_state = if self.is_dead || self.is_downed {
                crate::ai::creatures::mannequin::MknState::Down
            } else if self.flinch_timer > 0.0 {
                crate::ai::creatures::mannequin::MknState::Damage
            } else if self.is_attacking {
                crate::ai::creatures::mannequin::MknState::Attack
            } else {
                crate::ai::creatures::mannequin::MknState::Chase
            };
            self.anim.sync_mannequin(mkn_state, self.is_kick_next);
        }
    }

    /// Update enemy timers, attack logic, and navigation towards player.
    pub fn update(&mut self, player_pos: Vec3, dt: f32) {
        if self.is_dead {
            self.anim.update(dt, self.yaw);
            return;
        }
        if self.flinch_timer > 0.0 {
            self.flinch_timer = (self.flinch_timer - dt).max(0.0);
        }
        if self.down_timer > 0.0 {
            self.down_timer -= dt;
            if self.down_timer <= 0.0 && self.health > 0.0 {
                // Recover from floor
                self.is_downed = false;
                self.endurance = self.max_endurance * 0.5;
            }
        }
        if self.attack_cooldown > 0.0 {
            self.attack_cooldown = (self.attack_cooldown - dt).max(0.0);
        }

        if !self.is_downed && self.flinch_timer <= 0.0 {
            let dx = player_pos.x - self.position.x;
            let dz = player_pos.z - self.position.z;
            let dist = (dx * dx + dz * dz).sqrt();

            // Aggro threshold checking:
            // Monster activates if player enters its section territory (aggro_radius)
            // or if it was struck by an attack.
            if !self.is_aggroed && dist <= self.aggro_radius {
                self.is_aggroed = true;
            }

            if self.is_aggroed {
                let desired_yaw = dx.atan2(dz);

                // Smoothly turn towards player
                let mut diff = desired_yaw - self.yaw;
                while diff > PI {
                    diff -= 2.0 * PI;
                }
                while diff < -PI {
                    diff += 2.0 * PI;
                }
                self.yaw += diff.clamp(-3.0 * dt, 3.0 * dt);

                if self.is_grappling_player {
                    // Holding player in grapple lock (0x4b7160 / 0x4c00a0):
                    // Attack timer does not expire and walk root motion is suspended
                    self.is_attacking = true;
                    self.attack_timer = 0.0;
                } else if self.is_attacking {
                    self.attack_timer += dt;

                    // Forward lunge root motion during initial attack swing/lunge frames:
                    // Authentic PS2 pap_attack (enemy.c:3275-3312 frames 0..19) advances towards James to close the gap
                    let lunge_speed = match self.id {
                        5 => 500.0, // Abstract Daddy lunges forward to grapple James
                        4 => 250.0, // Pyramid Head forward momentum
                        _ => 0.0,
                    };
                    if lunge_speed > 0.0 && dist > 150.0 && self.attack_timer < 0.45 {
                        self.position.x += self.yaw.sin() * lunge_speed * dt;
                        self.position.z += self.yaw.cos() * lunge_speed * dt;
                    }

                    // Attack animation duration ~1.0 - 1.2s
                    let max_atk_time = if self.id == 1 { 1.2 } else { 1.0 };
                    if self.attack_timer >= max_atk_time {
                        self.is_attacking = false;
                        self.attack_timer = 0.0;
                        self.attack_cooldown = 1.5;
                        if self.id != 1 {
                            self.is_kick_next = !self.is_kick_next;
                        }
                    }
                } else {
                    let (attack_range, max_angle) = match self.id {
                        1 => (1200.0, 0.6), // Lying figure acid spray
                        2 => (650.0, 0.6),  // Mannequin
                        3 => (800.0, 0.6),  // Nurse pipe swing
                        4 => (900.0, 0.6),  // Pyramid Head
                        // Authentic PS2 pap_sense (enemy.c:3166): e->p_dist < 130.0 + 405.0 = 535.0, angle < 0.1745 (10 deg)
                        5 => (535.0, 0.35), // Abstract Daddy lunge grab
                        6 => (650.0, 0.6),  // Flesh Lips choke descent
                        7 => (600.0, 0.6),  // Mandarin tentacle strike
                        8 => (200.0, 0.6),  // Creeper bite
                        9 => (1400.0, 0.6), // Mary final boss moth swarm
                        _ => (700.0, 0.6),
                    };
                    if dist <= attack_range && self.attack_cooldown <= 0.0 && diff.abs() < max_angle
                    {
                        // Trigger creature attack
                        self.is_attacking = true;
                        self.attack_timer = 0.0;
                    } else if dist > 150.0 {
                        // Move towards player using authentic walk root motion
                        let walk_speed = match self.id {
                            1 => 402.0,  // scu
                            2 => 576.0,  // mkn
                            3 => 480.0,  // nse
                            4 => 330.0,  // red
                            5 => 540.0,  // pap
                            6 => 360.0,  // ike
                            7 => 450.0,  // arm
                            8 => 1200.0, // tyu
                            9 => 420.0,  // bos
                            _ => 400.0,
                        };
                        self.position.x += self.yaw.sin() * walk_speed * dt;
                        self.position.z += self.yaw.cos() * walk_speed * dt;
                    }
                }
            } else {
                // Return smoothly towards home anchor if drifted, or idle in place
                let home_dx = self.spawn_origin.x - self.position.x;
                let home_dz = self.spawn_origin.z - self.position.z;
                let home_dist = (home_dx * home_dx + home_dz * home_dz).sqrt();
                if home_dist > 50.0 {
                    self.yaw = home_dx.atan2(home_dz);
                    self.position.x += self.yaw.sin() * 200.0 * dt;
                    self.position.z += self.yaw.cos() * 200.0 * dt;
                }
            }
        }

        // Sync and advance creature animation
        match self.id {
            1 => {
                let scu_state = if self.is_dead || self.is_downed {
                    crate::ai::creatures::lying_figure::ScuState::Down
                } else if self.flinch_timer > 0.0 {
                    crate::ai::creatures::lying_figure::ScuState::Damage
                } else if self.is_attacking {
                    crate::ai::creatures::lying_figure::ScuState::Attack
                } else if self.is_aggroed {
                    crate::ai::creatures::lying_figure::ScuState::Chase
                } else {
                    crate::ai::creatures::lying_figure::ScuState::Wander
                };
                self.anim.sync_lying_figure(scu_state);
            }
            2 => {
                let mkn_state = if self.is_dead || self.is_downed {
                    crate::ai::creatures::mannequin::MknState::Down
                } else if self.flinch_timer > 0.0 {
                    crate::ai::creatures::mannequin::MknState::Damage
                } else if self.is_attacking {
                    crate::ai::creatures::mannequin::MknState::Attack
                } else if self.is_aggroed {
                    crate::ai::creatures::mannequin::MknState::Chase
                } else {
                    crate::ai::creatures::mannequin::MknState::Event
                };
                self.anim.sync_mannequin(mkn_state, self.is_kick_next);
            }
            3 => {
                let nse_state = if self.is_dead || self.is_downed {
                    crate::ai::creatures::nurse::NseState::Down
                } else if self.flinch_timer > 0.0 {
                    crate::ai::creatures::nurse::NseState::Damage
                } else if self.is_attacking {
                    crate::ai::creatures::nurse::NseState::Attack
                } else if self.is_aggroed {
                    crate::ai::creatures::nurse::NseState::Chase
                } else {
                    crate::ai::creatures::nurse::NseState::Confuse
                };
                self.anim.sync_nurse(nse_state);
            }
            4 => {
                let red_state = if self.is_grappling_player {
                    crate::ai::creatures::pyramid_head::RedState::Grab
                } else if self.is_attacking {
                    crate::ai::creatures::pyramid_head::RedState::Attack
                } else if self.is_aggroed {
                    crate::ai::creatures::pyramid_head::RedState::Chase
                } else {
                    crate::ai::creatures::pyramid_head::RedState::Stand
                };
                self.anim.sync_pyramid_head(red_state);
            }
            5 => {
                // Abstract Daddy (pap)
                use crate::ai::creatures::anim::{AbstractDaddyAnimClips, CreatureAnimStateKind};
                if self.is_dead || self.is_downed {
                    self.anim.transition_to(
                        CreatureAnimStateKind::Knockdown,
                        AbstractDaddyAnimClips::KNOCKDOWN,
                        0.0,
                        None,
                        None,
                    );
                } else if self.flinch_timer > 0.0 {
                    self.anim.transition_to(
                        CreatureAnimStateKind::Flinch,
                        AbstractDaddyAnimClips::FLINCH,
                        0.0,
                        None,
                        None,
                    );
                } else if self.is_grappling_player {
                    // Authentic PS2 PAP_ANIM[3] looping strangle hold while smothering James
                    self.anim.transition_to(
                        CreatureAnimStateKind::AttackSecondary,
                        AbstractDaddyAnimClips::STRUGGLE_HOLD,
                        0.0,
                        None,
                        None,
                    );
                } else if self.is_attacking {
                    self.anim.transition_to(
                        CreatureAnimStateKind::AttackPrimary,
                        AbstractDaddyAnimClips::LUNGE_GRAB,
                        0.0,
                        Some((
                            AbstractDaddyAnimClips::ATTACK_HIT_START,
                            AbstractDaddyAnimClips::ATTACK_HIT_END,
                        )),
                        None,
                    );
                } else if self.is_aggroed {
                    self.anim.transition_to(
                        CreatureAnimStateKind::Locomotion,
                        AbstractDaddyAnimClips::WALK,
                        AbstractDaddyAnimClips::WALK_ROOT_MOTION_PER_FRAME,
                        None,
                        None,
                    );
                } else {
                    self.anim.transition_to(
                        CreatureAnimStateKind::Idle,
                        AbstractDaddyAnimClips::STAND,
                        0.0,
                        None,
                        None,
                    );
                }
            }
            6 => {
                // Flesh Lips (ike)
                use crate::ai::creatures::anim::{CreatureAnimStateKind, FleshLipsAnimClips};
                if self.is_dead {
                    self.anim.transition_to(
                        CreatureAnimStateKind::Knockdown,
                        FleshLipsAnimClips::FALL_DEATH,
                        0.0,
                        None,
                        None,
                    );
                } else if self.flinch_timer > 0.0 {
                    self.anim.transition_to(
                        CreatureAnimStateKind::Flinch,
                        FleshLipsAnimClips::FLINCH,
                        0.0,
                        None,
                        None,
                    );
                } else if self.is_grappling_player {
                    // Authentic PS2 IKE_ANIM[4] looping choke hold while straddling and strangling James
                    self.anim.transition_to(
                        CreatureAnimStateKind::AttackSecondary,
                        FleshLipsAnimClips::CHOKE_HOLD,
                        0.0,
                        None,
                        None,
                    );
                } else if self.is_attacking {
                    self.anim.transition_to(
                        CreatureAnimStateKind::AttackPrimary,
                        FleshLipsAnimClips::GRAB_DESCEND,
                        0.0,
                        Some((
                            FleshLipsAnimClips::ATTACK_HIT_START,
                            FleshLipsAnimClips::ATTACK_HIT_END,
                        )),
                        None,
                    );
                } else if self.is_aggroed {
                    self.anim.transition_to(
                        CreatureAnimStateKind::Locomotion,
                        FleshLipsAnimClips::GLIDE_ADVANCE,
                        FleshLipsAnimClips::SWING_ROOT_MOTION_PER_FRAME,
                        None,
                        None,
                    );
                } else {
                    self.anim.transition_to(
                        CreatureAnimStateKind::Idle,
                        FleshLipsAnimClips::HANG_IDLE,
                        0.0,
                        None,
                        None,
                    );
                }
            }
            7 => {
                // Mandarin (arm)
                use crate::ai::creatures::anim::{CreatureAnimStateKind, MandarinAnimClips};
                if self.is_dead || self.is_downed {
                    self.anim.transition_to(
                        CreatureAnimStateKind::Knockdown,
                        MandarinAnimClips::KNOCKDOWN,
                        0.0,
                        None,
                        None,
                    );
                } else if self.flinch_timer > 0.0 {
                    self.anim.transition_to(
                        CreatureAnimStateKind::Flinch,
                        MandarinAnimClips::FLINCH,
                        0.0,
                        None,
                        None,
                    );
                } else if self.is_attacking {
                    self.anim.transition_to(
                        CreatureAnimStateKind::AttackPrimary,
                        MandarinAnimClips::TENTACLE_STRIKE,
                        0.0,
                        Some((
                            MandarinAnimClips::ATTACK_HIT_START,
                            MandarinAnimClips::ATTACK_HIT_END,
                        )),
                        None,
                    );
                } else if self.is_aggroed {
                    self.anim.transition_to(
                        CreatureAnimStateKind::Locomotion,
                        MandarinAnimClips::CRAWL,
                        MandarinAnimClips::CRAWL_ROOT_MOTION_PER_FRAME,
                        None,
                        None,
                    );
                } else {
                    self.anim.transition_to(
                        CreatureAnimStateKind::Idle,
                        MandarinAnimClips::HANG_IDLE,
                        0.0,
                        None,
                        None,
                    );
                }
            }
            8 => {
                // Creeper (tyu)
                use crate::ai::creatures::anim::{CreatureAnimStateKind, CreeperAnimClips};
                if self.is_attacking || self.is_grappling_player {
                    self.anim.transition_to(
                        CreatureAnimStateKind::AttackPrimary,
                        CreeperAnimClips::BITE,
                        0.0,
                        None,
                        None,
                    );
                } else if self.is_aggroed {
                    self.anim.transition_to(
                        CreatureAnimStateKind::Locomotion,
                        CreeperAnimClips::SCUTTLE,
                        CreeperAnimClips::SCUTTLE_ROOT_MOTION_PER_FRAME,
                        None,
                        None,
                    );
                } else {
                    self.anim.transition_to(
                        CreatureAnimStateKind::Idle,
                        CreeperAnimClips::SCUTTLE,
                        0.0,
                        None,
                        None,
                    );
                }
            }
            9 => {
                // Mary Final Boss (bos)
                use crate::ai::creatures::anim::{CreatureAnimStateKind, MaryBossAnimClips};
                if self.is_dead {
                    self.anim.transition_to(
                        CreatureAnimStateKind::Knockdown,
                        MaryBossAnimClips::DEATH_FALL,
                        0.0,
                        None,
                        None,
                    );
                } else if self.flinch_timer > 0.0 {
                    self.anim.transition_to(
                        CreatureAnimStateKind::Flinch,
                        MaryBossAnimClips::FLINCH,
                        0.0,
                        None,
                        None,
                    );
                } else if self.is_grappling_player {
                    // Authentic PS2 BOS_ANIM[4] looping choke hold while strangling James
                    self.anim.transition_to(
                        CreatureAnimStateKind::AttackSecondary,
                        MaryBossAnimClips::CHOKE_HOLD,
                        0.0,
                        None,
                        None,
                    );
                } else if self.is_attacking {
                    self.anim.transition_to(
                        CreatureAnimStateKind::AttackPrimary,
                        MaryBossAnimClips::LUNGE_CATCH,
                        0.0,
                        Some((
                            MaryBossAnimClips::ATTACK_HIT_START,
                            MaryBossAnimClips::ATTACK_HIT_END,
                        )),
                        None,
                    );
                } else if self.is_aggroed {
                    self.anim.transition_to(
                        CreatureAnimStateKind::Locomotion,
                        MaryBossAnimClips::HOVER_IDLE,
                        MaryBossAnimClips::HOVER_ROOT_MOTION_PER_FRAME,
                        None,
                        None,
                    );
                } else {
                    self.anim.transition_to(
                        CreatureAnimStateKind::Idle,
                        MaryBossAnimClips::HOVER_IDLE,
                        0.0,
                        None,
                        None,
                    );
                }
            }
            _ => {}
        }
        self.anim.update(dt, self.yaw);
    }
}

/// Interactive Combat Testing Room / Arena containing player and spawned target enemies.
#[derive(Debug, Clone)]
pub struct CombatArena {
    pub player: CombatController,
    pub enemies: Vec<CombatTargetEnemy>,
    pub player_position: Vec3,
    pub player_yaw: f32,
    pub attack_hit_registered: bool,
    /// Index of enemy currently holding James in a grapple lock (`0x533d00` / `0x4b7160`)
    pub grappling_enemy_idx: Option<usize>,
}

impl Default for CombatArena {
    fn default() -> Self {
        Self::new()
    }
}

impl CombatArena {
    /// Initialize a combat test arena with all monsters and bosses separated into
    /// distinct thematic sections across the arena map:
    /// - Front Courtyard: Lying Figure (`[0, 0, 1500]`), Mannequin (`[2200, 0, 1500]`), Creeper (`[-2200, 0, 1500]`)
    /// - Middle Wing (Hanging/Corridor): Nurse (`[-3000, 0, 3600]`), Mandarin (`[0, 0, 3600]`), Flesh Lips (`[3000, 0, 3600]`)
    /// - Boss Sanctuary (Deep North): Abstract Daddy (`[-2600, 0, 5800]`), Pyramid Head (`[0, 0, 5800]`), Mary Final Boss (`[2600, 0, 5800]`)
    pub fn new() -> Self {
        let player = CombatController::new(false, false);
        let enemies = vec![
            // Section 1: Front Entry Courtyard (Z ~ 1500, active when approaching)
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
            grappling_enemy_idx: None,
        }
    }

    /// Check if any enemy is currently downed at the player's feet (within 500 units).
    pub fn has_downed_enemy_at_feet(&self) -> bool {
        let forward = Vec3::new(self.player_yaw.sin(), 0.0, self.player_yaw.cos());
        let feet_check = self.player_position + forward * 250.0;

        self.enemies.iter().any(|e| {
            if !e.is_downed || e.is_dead {
                return false;
            }
            let dist = (Vec3::new(e.position.x, 0.0, e.position.z) - feet_check).length();
            dist <= (e.hitbox_radius + 350.0)
        })
    }

    /// Execute simulation tick (60 Hz).
    pub fn tick(&mut self, ready_held: bool, action_pressed: bool, action_held: bool, dt: f32) {
        let has_downed = self.has_downed_enemy_at_feet();
        self.player
            .handle_combat_inputs(ready_held, action_pressed, action_held, has_downed, dt);

        // Check if an attack was triggered this tick and perform hit detection
        if let Some(atk) = self.player.active_attack {
            let hit_frame = self.player.anim_frame as u8;
            if !self.attack_hit_registered
                && hit_frame >= atk.first_hit_frame
                && hit_frame <= atk.last_hit_frame
            {
                self.attack_hit_registered = true;
                self.resolve_attack_hits(&atk);
            }
        } else {
            self.attack_hit_registered = false;
        }

        // Update target enemies and check for grapple/hold interactions with James
        let player_p = self.player_position;
        let is_god_mode = self.player.god_mode;
        let is_dying_or_dead =
            self.player.mode == CombatMode::Dead || self.player.mode == CombatMode::Dying;

        for (idx, enemy) in self.enemies.iter_mut().enumerate() {
            enemy.update(player_p, dt);

            // Grapple / Grab Attack Resolution (1:1 with authentic PS2 en_red, en_pap, en_ike, en_bos)
            // When an attacking enemy with a grapple move connects with player within grapple range:
            if !is_god_mode && !is_dying_or_dead && enemy.is_attacking {
                let dx = enemy.position.x - player_p.x;
                let dz = enemy.position.z - player_p.z;
                let dist = (dx * dx + dz * dz).sqrt();

                // Authentic PS2 shBattleHuggedHuman (0x00124db0):
                // Hold timer is 240 frames @ 60Hz = 4.0 seconds (reduced by player struggle button presses)
                let (grab_range, meter) = match enemy.id {
                    4 => (400.0, 4.0), // Pyramid Head one-handed choke lift
                    // Authentic PS2 swing_hit (enemy.c:3094, 3305): reach 400.0 + James 130.0 = 530.0
                    5 => (530.0, 4.0), // Abstract Daddy strangle grapple
                    6 => (450.0, 4.0), // Flesh Lips ceiling choke descent
                    8 => (220.0, 3.0), // Creeper latch / bite
                    9 => (550.0, 4.0), // Mary Final Boss tentacle grapple
                    _ => (0.0, 0.0),
                };

                if grab_range > 0.0
                    && dist <= grab_range
                    && self.player.mode != CombatMode::Held
                    && self.player.invulnerability_timer <= 0.0
                {
                    self.player.apply_grab(meter);
                    // Bind creature to grapple state (e->slv = 2 in PS2 enemy.c)
                    enemy.is_grappling_player = true;
                    self.grappling_enemy_idx = Some(idx);
                    // Authentic PS2 orientation: snap James to face the grabbing enemy
                    self.player_yaw = dx.atan2(dz);
                }
            }
        }

        // When held in grapple: maintain creature anchor, apply authentic hold damage over time (0.2 * 30 DPS = 6.0 DPS),
        // and handle grapple lifecycle & struggle breakout
        if self.player.mode == CombatMode::Held {
            // Validate grappler presence: if the grappler died, flinched, got knocked down, or was removed, release James!
            let mut should_release = false;
            if let Some(g_idx) = self.grappling_enemy_idx {
                if g_idx < self.enemies.len() {
                    let g = &mut self.enemies[g_idx];
                    if g.is_dead || g.is_downed || g.flinch_timer > 0.0 {
                        should_release = true;
                    } else {
                        // Authentic PS2 grapple coupling (enemy.c:3309, 4342):
                        // Monster slides onto James and stays anchored in front of him
                        let forward = Vec3::new(self.player_yaw.sin(), 0.0, self.player_yaw.cos());
                        let anchor_dist = match g.id {
                            5 => 380.0, // Abstract Daddy standing grapple distance (in front of James, not clipping into him)
                            6 => 40.0, // Flesh Lips hangs directly overhead / straddles James's neck
                            8 => 80.0, // Creeper bites at feet
                            9 => 220.0, // Mary Final Boss hovers close-range with tentacles wrapped around James
                            _ => 250.0,
                        };
                        g.position = self.player_position + forward * anchor_dist;
                        g.yaw = self.player_yaw + std::f32::consts::PI;
                        g.is_grappling_player = true;
                    }
                } else {
                    should_release = true;
                }
            } else {
                // Ghost hold failsafe: if no enemy is bound, release James immediately
                should_release = true;
            }

            if should_release {
                if let Some(g_idx) = self.grappling_enemy_idx.take() {
                    if g_idx < self.enemies.len() {
                        self.enemies[g_idx].is_grappling_player = false;
                        self.enemies[g_idx].is_attacking = false;
                    }
                }
                self.player.mode = CombatMode::Free;
                self.player.struggle_meter = 0.0;
                self.player.invulnerability_timer = 0.5;
            } else {
                let hold_dps = 0.2 * 30.0;
                self.player.health = (self.player.health - hold_dps * dt).max(0.0);
                if self.player.health <= 0.0 {
                    self.player.mode = CombatMode::Dying;
                    self.player.anim_frame = 0.0;
                    self.player.clip_length = 24.0;
                    self.player.pending_sfx = Some(11050); // Death scream
                    if let Some(g_idx) = self.grappling_enemy_idx.take() {
                        if g_idx < self.enemies.len() {
                            self.enemies[g_idx].is_grappling_player = false;
                            self.enemies[g_idx].is_attacking = false;
                        }
                    }
                }
            }
        } else if let Some(g_idx) = self.grappling_enemy_idx.take() {
            // James broke free from struggle or took damage (PS2 enemy.c:3325 recoil breakout)
            if g_idx < self.enemies.len() {
                let g = &mut self.enemies[g_idx];
                g.is_grappling_player = false;
                g.is_attacking = false;
                g.attack_cooldown = 2.0;
                // Authentic PS2 recoil: push creature backward away from James
                let backward = Vec3::new(self.player_yaw.sin(), 0.0, self.player_yaw.cos());
                g.position += backward * 250.0;
                g.flinch_timer = 0.5;
            }
        }

        // Mutual character & enemy cylinder collision separation (authentic PS2 clCheckColumn2ColumnHit)
        self.resolve_character_separations();
    }

    /// Execute simulation tick isolating updates to only one active enemy index.
    /// Inactive enemies are kept dormant off-screen so only the targeted enemy is active.
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
            if e.is_downed && !e.is_dead {
                let forward = Vec3::new(self.player_yaw.sin(), 0.0, self.player_yaw.cos());
                let feet_check = self.player_position + forward * 250.0;
                let dist = (Vec3::new(e.position.x, 0.0, e.position.z) - feet_check).length();
                dist <= (e.hitbox_radius + 350.0)
            } else {
                false
            }
        };

        self.player
            .handle_combat_inputs(ready_held, action_pressed, action_held, has_downed, dt);

        if let Some(atk) = self.player.active_attack {
            let hit_frame = self.player.anim_frame as u8;
            if !self.attack_hit_registered
                && hit_frame >= atk.first_hit_frame
                && hit_frame <= atk.last_hit_frame
            {
                self.attack_hit_registered = true;
                let forward = Vec3::new(self.player_yaw.sin(), 0.0, self.player_yaw.cos());
                let target_enemy = &mut self.enemies[isolated_idx];
                if !target_enemy.is_dead {
                    if atk.kind == 0 {
                        let muzzle =
                            self.player_position + Vec3::new(0.0, -1100.0, 0.0) + forward * 200.0;
                        if target_enemy
                            .hitbox()
                            .intersects_ray(muzzle, forward, 0.0, atk.max_range)
                            .is_some()
                        {
                            target_enemy.receive_hit(&atk);
                        }
                    } else if atk.kind == 1 {
                        let feet_check = self.player_position + forward * 250.0;
                        if target_enemy.is_downed {
                            let dist =
                                (Vec3::new(target_enemy.position.x, 0.0, target_enemy.position.z)
                                    - feet_check)
                                    .length();
                            if dist <= (target_enemy.hitbox_radius * 1.5 + atk.max_range) {
                                target_enemy.receive_hit(&atk);
                            }
                        }
                    } else {
                        let root = self.player_position + Vec3::new(0.0, -1000.0, 0.0);
                        let tip = root + forward * 1600.0;
                        if target_enemy.hitbox().intersects_segment(root, tip) {
                            target_enemy.receive_hit(&atk);
                        }
                    }
                }
            }
        } else {
            self.attack_hit_registered = false;
        }

        let player_p = self.player_position;
        let is_god_mode = self.player.god_mode;
        let is_dying_or_dead =
            self.player.mode == CombatMode::Dead || self.player.mode == CombatMode::Dying;

        let enemy = &mut self.enemies[isolated_idx];
        enemy.update(player_p, dt);

        if !is_god_mode && !is_dying_or_dead && enemy.is_attacking {
            let dx = enemy.position.x - player_p.x;
            let dz = enemy.position.z - player_p.z;
            let dist = (dx * dx + dz * dz).sqrt();

            // Authentic PS2 shBattleHuggedHuman (0x00124db0):
            // Hold timer is 240 frames @ 60Hz = 4.0 seconds (reduced by player struggle button presses)
            let (grab_range, meter) = match enemy.id {
                4 => (400.0, 4.0),
                5 => (530.0, 4.0),
                6 => (450.0, 4.0),
                8 => (220.0, 3.0),
                9 => (550.0, 4.0),
                _ => (0.0, 0.0),
            };

            if grab_range > 0.0
                && dist <= grab_range
                && self.player.mode != CombatMode::Held
                && self.player.invulnerability_timer <= 0.0
            {
                self.player.apply_grab(meter);
                enemy.is_grappling_player = true;
                self.grappling_enemy_idx = Some(isolated_idx);
                self.player_yaw = dx.atan2(dz);
            }
        }

        if self.player.mode == CombatMode::Held {
            let mut should_release = false;
            if let Some(g_idx) = self.grappling_enemy_idx {
                if g_idx == isolated_idx {
                    let g = &mut self.enemies[isolated_idx];
                    if g.is_dead || g.is_downed || g.flinch_timer > 0.0 {
                        should_release = true;
                    } else {
                        let forward = Vec3::new(self.player_yaw.sin(), 0.0, self.player_yaw.cos());
                        let anchor_dist = match g.id {
                            5 => 380.0, // Abstract Daddy standing grapple distance (in front of James, not clipping into him)
                            6 => 40.0, // Flesh Lips hangs directly overhead / straddles James's neck
                            8 => 80.0, // Creeper bites at feet
                            9 => 220.0, // Mary Boss tentacle reach distance (shBattleAttackHitCheckToHuman 0x123170)
                            _ => 250.0,
                        };
                        g.position = self.player_position + forward * anchor_dist;
                        g.yaw = self.player_yaw + std::f32::consts::PI;
                        g.is_grappling_player = true;
                    }
                } else {
                    should_release = true;
                }
            } else {
                should_release = true;
            }

            if should_release {
                if let Some(g_idx) = self.grappling_enemy_idx.take() {
                    if g_idx < self.enemies.len() {
                        self.enemies[g_idx].is_grappling_player = false;
                        self.enemies[g_idx].is_attacking = false;
                    }
                }
                self.player.mode = CombatMode::Free;
                self.player.struggle_meter = 0.0;
                self.player.invulnerability_timer = 0.5;
            } else {
                let hold_dps = 0.2 * 30.0;
                self.player.health = (self.player.health - hold_dps * dt).max(0.0);
                if self.player.health <= 0.0 {
                    self.player.mode = CombatMode::Dying;
                    self.player.anim_frame = 0.0;
                    self.player.clip_length = 24.0;
                    self.player.pending_sfx = Some(11050);
                    if let Some(g_idx) = self.grappling_enemy_idx.take() {
                        if g_idx < self.enemies.len() {
                            self.enemies[g_idx].is_grappling_player = false;
                            self.enemies[g_idx].is_attacking = false;
                        }
                    }
                }
            }
        } else if let Some(g_idx) = self.grappling_enemy_idx.take() {
            if g_idx < self.enemies.len() {
                let g = &mut self.enemies[g_idx];
                g.is_grappling_player = false;
                g.is_attacking = false;
                g.attack_cooldown = 2.0;
                let backward = Vec3::new(self.player_yaw.sin(), 0.0, self.player_yaw.cos());
                g.position += backward * 250.0;
                g.flinch_timer = 0.5;
            }
        }

        // Mutual collision separation with the active enemy
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

    /// Resolve mutual character-to-enemy and enemy-to-enemy cylinder collision separations.
    ///
    /// Upright enemies cannot be walked through by James, and upright enemies push apart from one another.
    /// Follows authentic PS2 `clCheckColumn2ColumnHit` / `enemy.c:1494`:
    /// `mind = 130.0 + enemy_size * 0.8` (where upright enemy size is ~150-200, giving `mind ~ 250..290mm`).
    pub fn resolve_character_separations(&mut self) {
        let is_held = self.player.mode == CombatMode::Held;

        // 1. Player vs Upright Enemies (only when not actively held in grab lock)
        if !is_held {
            for enemy in &mut self.enemies {
                // Downed enemies and creepers (id 8) can be stepped over / stomped at feet
                if enemy.is_dead || enemy.is_downed || enemy.id == 8 {
                    continue;
                }
                // Air/ceiling monsters (Flesh Lips 6, Mandarin 7, Mary 9) do not block floor walking
                if enemy.id == 6 || enemy.id == 7 || enemy.id == 9 {
                    continue;
                }

                let dx = self.player_position.x - enemy.position.x;
                let dz = self.player_position.z - enemy.position.z;
                let dist_sq = dx * dx + dz * dz;

                // Authentic PS2 collision cylinder radius (`enemy.c:1494`):
                // James collision radius is 130 mm (`s_col 130.0f`).
                // Upright enemy radius is `size * 0.8` (scu/mkn/nse: 150*0.8=120; bosses: 200..320*0.8=160..256).
                // Total minimum contact separation is 250.0 - 320.0 mm.
                let enemy_col_radius = match enemy.id {
                    4 => 200.0, // Pyramid Head (red)
                    5 => 220.0, // Abstract Daddy (pap)
                    _ => 130.0, // Standard humanoid monsters (scu, mkn, nse)
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

                    // Push player out from enemy (heavy bosses push player 100%, lighter enemies 60/40)
                    let (push_player, push_enemy) = match enemy.id {
                        4 | 5 => (penetration, 0.0), // Pyramid Head & Abstract Daddy are immovable
                        _ => (penetration * 0.6, penetration * 0.4),
                    };

                    self.player_position.x += nx * push_player;
                    self.player_position.z += nz * push_player;
                    enemy.position.x -= nx * push_enemy;
                    enemy.position.z -= nz * push_enemy;
                }
            }
        }

        // 2. Enemy vs Enemy cylinder separation
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

    /// Test active attack collision against all enemies in arena.
    fn resolve_attack_hits(&mut self, atk: &AttackSpec) {
        let forward = Vec3::new(self.player_yaw.sin(), 0.0, self.player_yaw.cos());

        if atk.kind == 0 {
            // Gunshot raycast from weapon muzzle
            let muzzle = self.player_position + Vec3::new(0.0, -1100.0, 0.0) + forward * 200.0;

            if self.player.weapon == WeaponGrip::Shotgun {
                // Shotgun fires 10 pellets in 30° cone
                for pellet in 0..self.player.shotgun_pellets {
                    let angle_offset = ((pellet as f32 / 10.0) - 0.5) * (PI / 6.0);
                    let pellet_yaw = self.player_yaw + angle_offset;
                    let pellet_dir = Vec3::new(pellet_yaw.sin(), 0.0, pellet_yaw.cos());

                    let mut closest_enemy: Option<(usize, f32)> = None;
                    for (idx, enemy) in self.enemies.iter().enumerate() {
                        if enemy.is_dead {
                            continue;
                        }
                        if let Some(t) =
                            enemy
                                .hitbox()
                                .intersects_ray(muzzle, pellet_dir, 0.0, atk.max_range)
                        {
                            if closest_enemy.is_none() || t < closest_enemy.unwrap().1 {
                                closest_enemy = Some((idx, t));
                            }
                        }
                    }

                    if let Some((idx, _)) = closest_enemy {
                        self.enemies[idx].receive_hit(atk);
                    }
                }
            } else {
                // Single ray for Handgun / Rifle
                let mut closest_enemy: Option<(usize, f32)> = None;
                for (idx, enemy) in self.enemies.iter().enumerate() {
                    if enemy.is_dead {
                        continue;
                    }
                    if let Some(t) =
                        enemy
                            .hitbox()
                            .intersects_ray(muzzle, forward, 0.0, atk.max_range)
                    {
                        if closest_enemy.is_none() || t < closest_enemy.unwrap().1 {
                            closest_enemy = Some((idx, t));
                        }
                    }
                }

                if let Some((idx, _)) = closest_enemy {
                    self.enemies[idx].receive_hit(atk);
                }
            }
        } else if atk.kind == 1 {
            // Stomp / Kick against downed enemy at feet
            let feet_check = self.player_position + forward * 250.0;
            for enemy in &mut self.enemies {
                if !enemy.is_downed || enemy.is_dead {
                    continue;
                }
                let dist =
                    (Vec3::new(enemy.position.x, 0.0, enemy.position.z) - feet_check).length();
                if dist <= (enemy.hitbox_radius * 1.5 + atk.max_range) {
                    enemy.receive_hit(atk);
                }
            }
        } else {
            // Melee swing segment test (hand root to plank tip)
            let root = self.player_position + Vec3::new(0.0, -1000.0, 0.0);
            let tip = root + forward * 1600.0; // Reach in front of player

            for enemy in &mut self.enemies {
                if enemy.is_dead {
                    continue;
                }
                if enemy.hitbox().intersects_segment(root, tip) {
                    enemy.receive_hit(atk);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_combat_controller_initialization() {
        let combat = CombatController::new(false, false);
        assert_eq!(combat.health, 100.0);
        assert_eq!(combat.max_health, 100.0);
        assert_eq!(combat.weapon, WeaponGrip::WoodenPlank);
        assert_eq!(combat.mode, CombatMode::Free);
        assert_eq!(combat.loaded_rounds[0], 10);
        assert_eq!(combat.loaded_rounds[1], 6);
        assert_eq!(combat.loaded_rounds[2], 4);
    }

    #[test]
    fn test_ammo_consumption_and_reload() {
        let mut combat = CombatController::new(false, false);
        combat.equip_weapon(WeaponGrip::Handgun);
        assert_eq!(combat.loaded_ammo(), 10);
        assert_eq!(combat.spare_ammo(), 30);

        for _ in 0..10 {
            assert!(combat.consume_ammo());
        }
        assert_eq!(combat.loaded_ammo(), 0);
        assert!(!combat.consume_ammo()); // Empty clip

        let reloaded = combat.execute_reload();
        assert_eq!(reloaded, 10);
        assert_eq!(combat.loaded_ammo(), 10);
        assert_eq!(combat.spare_ammo(), 20);
    }

    #[test]
    fn test_low_health_indicator_formula() {
        let mut combat = CombatController::new(false, false);
        let dt = 1.0 / 60.0;

        // Above 50% HP -> alpha is 0
        assert_eq!(combat.update_low_health_alpha(dt), 0.0);

        // Reduced to 25% HP -> alpha pulses
        combat.health = 25.0;
        let alpha = combat.update_low_health_alpha(dt);
        assert!(alpha > 0.5 && alpha <= 1.0);
    }

    #[test]
    fn test_grab_struggle_meter_decay() {
        let mut combat = CombatController::new(false, false);
        combat.apply_grab(1.0); // 1.0 second grab struggle meter
        assert_eq!(combat.mode, CombatMode::Held);

        // 30 button presses should drain 1.0 second
        combat.process_struggle_inputs(30, 1.0 / 60.0, false);
        assert_eq!(combat.struggle_meter, 0.0);
        assert_eq!(combat.mode, CombatMode::Hurt); // Escaped
    }

    #[test]
    fn test_tap_vs_hold_attack_states() {
        let mut combat = CombatController::new(false, false);
        let dt = 1.0 / 60.0;

        // 1. Ready weapon
        combat.handle_combat_inputs(true, false, false, false, dt);
        combat.mode = CombatMode::Ready;

        // 2. Quick tap of attack button (< 0.2s) -> Light swing
        combat.handle_combat_inputs(true, true, true, false, dt);
        combat.handle_combat_inputs(true, false, false, false, dt); // released next frame
        assert_eq!(combat.mode, CombatMode::Swing);
        assert!(!combat.is_heavy_attack);
        assert_eq!(combat.active_attack.unwrap().id, ATK_PLANK_LIGHT.id);

        // 3. Reset to Ready and test 0.2s hold -> Heavy downswing
        combat.mode = CombatMode::Ready;
        combat.handle_combat_inputs(true, true, true, false, dt);
        // Simulate holding for 15 frames (~0.25s)
        for _ in 0..15 {
            combat.handle_combat_inputs(true, false, true, false, dt);
        }
        assert_eq!(combat.mode, CombatMode::Swing);
        assert!(combat.is_heavy_attack);
        assert_eq!(combat.active_attack.unwrap().id, ATK_PLANK_HEAVY.id);
    }

    #[test]
    fn test_combat_arena_melee_and_gun_hit_resolution() {
        let mut arena = CombatArena::new();
        let dt = 1.0 / 60.0;

        // Verify initial state
        assert_eq!(arena.enemies.len(), 9);
        assert_eq!(arena.enemies[0].health, 700.0);

        // 1. Ready handgun and fire raycast at Lying Figure in front
        arena.player.equip_weapon(WeaponGrip::Handgun);
        arena.player.mode = CombatMode::Ready;

        arena.tick(true, true, true, dt);
        assert_eq!(arena.player.mode, CombatMode::Shot);

        // Advance shot to hit frame and register hit
        arena.player.anim_frame = 2.0;
        arena.tick(true, false, false, dt);

        // Handgun deals 120 AP damage
        assert_eq!(arena.enemies[0].health, 580.0);

        // Finish shot animation and return to Ready
        arena.player.anim_ended = true;
        arena.tick(true, false, false, dt);
        assert_eq!(arena.player.mode, CombatMode::Ready);

        // 2. Equip plank and execute heavy downswing to knock down enemy
        arena.player.equip_weapon(WeaponGrip::WoodenPlank);
        arena.player.mode = CombatMode::Ready;

        // Hold attack button for 0.25s to trigger heavy swing
        arena.tick(true, true, true, dt);
        for _ in 0..15 {
            arena.tick(true, false, true, dt);
        }
        assert_eq!(arena.player.mode, CombatMode::Swing);
        assert!(arena.player.is_heavy_attack);

        // Advance to hit frame (8)
        arena.player.anim_frame = 8.0;
        arena.attack_hit_registered = false;
        arena.tick(true, false, false, dt);
        // Heavy plank deals 140 AP damage: 580 - 140 = 440
        assert_eq!(arena.enemies[0].health, 440.0);
    }

    #[test]
    fn test_downed_enemy_stomp_finish() {
        let mut arena = CombatArena::new();
        let dt = 1.0 / 60.0;

        // Manually place downed enemy at James' feet
        arena.enemies[0].is_downed = true;
        arena.enemies[0].down_timer = 10.0;
        arena.enemies[0].position = Vec3::new(0.0, 0.0, 300.0);
        assert!(arena.has_downed_enemy_at_feet());

        arena.player.equip_weapon(WeaponGrip::Handgun);
        arena.player.mode = CombatMode::Ready;

        // Press attack at feet -> triggers stomp charge / attack
        arena.tick(true, true, true, dt);
        // Hold for stomp
        for _ in 0..15 {
            arena.tick(true, false, true, dt);
        }
        assert_eq!(arena.player.mode, CombatMode::Stomp);
        assert!(arena.player.active_attack.is_some());

        // Advance to hit frame (3)
        arena.player.anim_frame = 3.0;
        arena.attack_hit_registered = false;
        arena.tick(true, false, true, dt);

        // 9999 AP stomp finishes downed enemy immediately
        assert!(arena.enemies[0].is_dead);
    }

    #[test]
    fn test_enemy_player_and_enemy_enemy_collision_separation() {
        let mut arena = CombatArena::new();
        // Place James at origin
        arena.player_position = Vec3::ZERO;
        // Place Lying Figure overlapping James at (100, 0, 0)
        arena.enemies[0].position = Vec3::new(100.0, 0.0, 0.0);
        let initial_player_x = arena.player_position.x;
        let initial_enemy_x = arena.enemies[0].position.x;

        arena.resolve_character_separations();

        // Authentic PS2 min contact distance is 130 + 130 = 260.0.
        // Initial dist was 100 -> they must push apart!
        let separation = (arena.player_position.x - arena.enemies[0].position.x).abs();
        assert!(
            separation >= 259.0,
            "Player and enemy must separate; got {}",
            separation
        );
        assert!(arena.player_position.x < initial_player_x);
        assert!(arena.enemies[0].position.x > initial_enemy_x);

        // Test two upright enemies overlapping: place enemy 1 (Mannequin) at (1000, 0, 0) and enemy 3 (Nurse) at (1100, 0, 0)
        // Note: enemies[2] is Creeper (id 8) which is exempt from standing body collision
        arena.enemies[1].position = Vec3::new(1000.0, 0.0, 0.0);
        arena.enemies[3].position = Vec3::new(1100.0, 0.0, 0.0);
        arena.resolve_character_separations();

        let enemy_sep = (arena.enemies[1].position.x - arena.enemies[3].position.x).abs();
        assert!(
            enemy_sep >= 259.0,
            "Enemies must push each other apart; got {}",
            enemy_sep
        );
    }

    #[test]
    fn test_grapple_binding_anchor_and_breakout_recoil() {
        let mut arena = CombatArena::new();
        let dt = 1.0 / 60.0;

        // Abstract Daddy is enemies[6] (id 5)
        let pap_idx = 6;
        assert_eq!(arena.enemies[pap_idx].id, 5);

        // Position James at origin facing +Z
        arena.player_position = Vec3::ZERO;
        arena.player_yaw = 0.0;

        // Place Abstract Daddy within grab reach (400 mm along +Z) and attacking
        arena.enemies[pap_idx].position = Vec3::new(0.0, 0.0, 400.0);
        arena.enemies[pap_idx].is_attacking = true;

        arena.tick(false, false, false, dt);

        // 1. Grab must connect and bind
        assert_eq!(arena.player.mode, CombatMode::Held);
        assert_eq!(arena.grappling_enemy_idx, Some(pap_idx));
        assert!(arena.enemies[pap_idx].is_grappling_player);

        // 2. Creature must be anchored 380mm in front of James (clear of mesh collision)
        let pap_z = arena.enemies[pap_idx].position.z;
        assert!((pap_z - 380.0).abs() < 1e-2, "Pap Z was {}", pap_z);

        // 3. Struggle escape: simulate 30 button presses to drain meter
        arena.player.process_struggle_inputs(30, dt, false);
        assert_eq!(arena.player.mode, CombatMode::Hurt);

        // Next arena tick processes the breakout & recoil
        arena.tick(false, false, false, dt);

        assert!(arena.grappling_enemy_idx.is_none());
        assert!(!arena.enemies[pap_idx].is_grappling_player);
        assert!(!arena.enemies[pap_idx].is_attacking);
        // Recoil pushback: creature pushed backward by 250mm
        assert!(arena.enemies[pap_idx].position.z > 380.0 + 200.0);
        assert!(arena.enemies[pap_idx].flinch_timer > 0.0);
    }

    #[test]
    fn test_all_grappling_enemies_grab_and_release() {
        let dt = 1.0 / 60.0;
        // Grappling enemies in SH2 bestiary:
        // id 4: Pyramid Head (idx 7, reach 400.0)
        // id 5: Abstract Daddy (idx 6, reach 530.0)
        // id 6: Flesh Lips (idx 5, reach 450.0)
        // id 8: Creeper (idx 2, reach 220.0)
        // id 9: Mary Boss (idx 8, reach 550.0)
        let test_cases = [
            (7, 4, 300.0), // Pyramid Head
            (6, 5, 450.0), // Abstract Daddy
            (5, 6, 350.0), // Flesh Lips
            (2, 8, 180.0), // Creeper
            (8, 9, 450.0), // Mary Boss
        ];

        for (enemy_idx, expected_id, spawn_dist) in test_cases {
            let mut arena = CombatArena::new();
            assert_eq!(
                arena.enemies[enemy_idx].id, expected_id,
                "Mismatch for enemy index {}",
                enemy_idx
            );

            arena.player_position = Vec3::ZERO;
            arena.player_yaw = 0.0;
            arena.player.mode = CombatMode::Free;

            // Spawn enemy in front of James within its grab reach
            arena.enemies[enemy_idx].position = Vec3::new(0.0, 0.0, spawn_dist);
            arena.enemies[enemy_idx].yaw = std::f32::consts::PI;
            arena.enemies[enemy_idx].is_attacking = true;

            arena.tick(false, false, false, dt);

            assert_eq!(
                arena.player.mode,
                CombatMode::Held,
                "Enemy id {} failed to trigger grab hold at distance {}",
                expected_id,
                spawn_dist
            );
            assert_eq!(
                arena.grappling_enemy_idx,
                Some(enemy_idx),
                "Enemy id {} failed to bind grappling_enemy_idx",
                expected_id
            );
            assert!(
                arena.enemies[enemy_idx].is_grappling_player,
                "Enemy id {} did not enter is_grappling_player state",
                expected_id
            );

            // Break free with struggle
            arena.player.process_struggle_inputs(40, dt, false);
            arena.tick(false, false, false, dt);

            assert_eq!(
                arena.player.mode,
                CombatMode::Hurt,
                "James did not enter Hurt mode upon struggle escape from enemy id {}",
                expected_id
            );
            assert!(
                arena.grappling_enemy_idx.is_none(),
                "grappling_enemy_idx not cleared after escape from enemy id {}",
                expected_id
            );
            assert!(
                !arena.enemies[enemy_idx].is_grappling_player,
                "Enemy id {} remained grappling after escape",
                expected_id
            );
        }
    }
}
