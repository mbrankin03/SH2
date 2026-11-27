//! Sensory awareness, `.cld` line-of-sight, and HP-scaled walk speed model for Silent Hill 2
//! enemies (1:1 PS2/PC parity).
//!
//! # Ground Truth & Binary References
//! - **`SLUS_202.28` (`docs/decomp/slus/Enemy/`):**
//!   - `enGetWorldCondition` (`0x00141ca0`): 4 world-lighting states (`1` Day, `2` NightOutdoorLightOn,
//!     `3` NightIndoorLightOn, `4` NightLightOff, plus `0` Lit override).
//!   - `enCheckHitEyes` (`0x00145540`) & `enCheckHitEyes2` (`0x00145770`): `.cld` wall LOS raycast
//!     (negative distance = clear LOS, positive distance = wall hit distance).
//!   - `enSCUCanSeePlayer` (`0x00163b80`), `enSCUGetFeelRange` (`0x001647f0`),
//!     `enSCUGetAttackRange` (`0x00164860`), `enSCUGetAttackAngle` (`0x001648e0`),
//!     `enSCUGetWalkSpeed` (`0x001646a0`), `enSCUGetCrawlSpeed` (`0x00164740`).
//!   - `enMKNCanSeePlayer` (`0x00156800`), `enMKNGetFeelRange` (`0x00157330`),
//!     `enMKNGetWalkSpeed` (`0x00157290`).
//!   - `enNSECanSeePlayer` (`0x00159400`, dual cones `1500mm @ 5°` + `900mm @ 30°`),
//!     `enNSEGetFeelRange` (`0x00159ba0`), `enNSEGetWalkSpeed` (`0x00159b00`).
//!   - `enTYUCanSeePlayer` (`0x001679c0`), `enTYUGetSpeed` (`0x00167bf0`).
//!   - `enREDCanSeePlayer` (`0x0015f530`), `enREDGetFeelRange` (`0x001600e0`),
//!     `enREDGetSpeed` (`0x0015ff60`), `enREDGetWalkSpeed` (`0x0015ffa0`).
//!   - `enONICanSeePlayer` (`0x0022b520`), `enONIGetFeelRange` (`0x0022be10`),
//!     `enONIGetSpeed` (`0x0022bc90`), `enONIGetWalkSpeed` (`0x0022bcd0`).
//!   - `enPAPGetWalkSpeed` (`0x0015cc70`), `enIKEGetMoveSpeed` (`0x00153280`),
//!     `enARMGetMoveSpeed` (`0x0023ab30`).
//! - **`sh_character_status.c`:** `shBattleListenHumanSound` (`0x00124b00`),
//!   `shBattleSeeHumanLight` (`0x00124b20`).

use crate::collision::raycast::intersect_segment_xz_t;
use crate::collision::CollisionWorld;
use glam::Vec3;
use serde::{Deserialize, Serialize};
use sh2_assets::formats::cld::WallSegment;
use std::f32::consts::{FRAC_PI_3, FRAC_PI_6, PI};

/// World lighting / visibility condition returned by `enGetWorldCondition`
/// (`SLUS_202.28` `0x00141ca0`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[repr(u8)]
pub enum RoomLighting {
    /// `0`: Unshadowed / fully lit override (`5000.0` mm sight for SCU/MKN).
    #[default]
    Lit = 0,
    /// `1`: Daytime (`sh2gfw_Get_NightOrDay() == 0`).
    Day = 1,
    /// `2`: Nighttime outdoor with player flashlight ON (`BgIsOut() != 0`, `DAT_011268fe != 0`).
    NightOutdoorLightOn = 2,
    /// `3`: Nighttime indoor with player flashlight ON (`BgIsOut() == 0`, `DAT_011268fe != 0`).
    NightIndoorLightOn = 3,
    /// `4`: Nighttime with player flashlight OFF (`DAT_011268fe == 0`, near-blind).
    NightLightOff = 4,
}

impl RoomLighting {
    /// Evaluate `enGetWorldCondition` (`SLUS_202.28` `0x00141ca0`).
    pub const fn from_world_condition(
        is_night: bool,
        is_outdoor: bool,
        is_flashlight_on: bool,
    ) -> Self {
        if !is_night {
            Self::Day
        } else if !is_flashlight_on {
            Self::NightLightOff
        } else if is_outdoor {
            Self::NightOutdoorLightOn
        } else {
            Self::NightIndoorLightOn
        }
    }

    /// Returns `true` for the dark/unlit state (`enGetWorldCondition() == 4`).
    #[inline]
    pub const fn is_dark_no_light(self) -> bool {
        matches!(self, Self::NightLightOff)
    }

    /// Returns `true` when nighttime (`enGetWorldCondition() >= 2`, enables flashlight beam alert).
    #[inline]
    pub const fn is_night(self) -> bool {
        matches!(
            self,
            Self::NightOutdoorLightOn | Self::NightIndoorLightOn | Self::NightLightOff
        )
    }
}

/// Difficulty level influencing perception, HP, endurance, and movement speed (`enGetMode()` `0..=3`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum DifficultyLevel {
    Beginner,
    Easy,
    #[default]
    Normal,
    Hard,
}

impl DifficultyLevel {
    /// Returns the 0-based difficulty table index (`enGetMode()` `0..=3`: Beginner..Hard).
    pub const fn as_index(self) -> usize {
        match self {
            Self::Beginner => 0,
            Self::Easy => 1,
            Self::Normal => 2,
            Self::Hard => 3,
        }
    }
}

impl From<crate::config::ActionDifficulty> for DifficultyLevel {
    fn from(diff: crate::config::ActionDifficulty) -> Self {
        match diff {
            crate::config::ActionDifficulty::Beginner => Self::Beginner,
            crate::config::ActionDifficulty::Easy => Self::Easy,
            crate::config::ActionDifficulty::Normal => Self::Normal,
            crate::config::ActionDifficulty::Hard => Self::Hard,
        }
    }
}

/// Information about a perceivable character (James or Maria) passed to sensory routines.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PerceivableTarget {
    pub position: Vec3,
    pub yaw: f32,
    pub is_flashlight_on: bool,
    pub is_running: bool,
    pub is_alive: bool,
}

impl PerceivableTarget {
    pub fn new(position: Vec3, yaw: f32, is_flashlight_on: bool, is_running: bool) -> Self {
        Self {
            position,
            yaw,
            is_flashlight_on,
            is_running,
            is_alive: true,
        }
    }
}

/// Sensory check output flags / state (`en*CanSeePlayer` return codes `0..=3`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SensoryNotice {
    /// `0`: Target is completely undetected.
    None,
    /// `1`: Target detected by sight (within vision cone and distance) or flashlight beam.
    VisualContact,
    /// Target detected by hearing / proximity feel range (`en*GetFeelRange`).
    AuditoryContact,
    /// `2`: Target is in primary close-combat attack range/cone.
    InAttackRange,
    /// `3`: Target is in Nurse's long narrow lunge cone (`1500mm @ 5°`, `enNSECanSeePlayer` `0x00159400`).
    InLungeRange,
}

/// Sensory configuration profile per enemy species (`SLUS_202.28` `Enemy/en_*.c`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SensoryProfile {
    /// Max sight distance for `RoomLighting::Lit` (`0`, units: mm).
    pub sight_lit: f32,
    /// Max sight distance for `RoomLighting::Day` (`1`, units: mm).
    pub sight_day: f32,
    /// Max sight distance for `RoomLighting::NightOutdoorLightOn` (`2`) & `NightIndoorLightOn` (`3`).
    pub sight_night_light_on: f32,
    /// Max sight distance for `RoomLighting::NightLightOff` (`4`, units: mm).
    pub sight_night_light_off: f32,
    /// Field of view half-angle in radians.
    pub view_half_angle: f32,
    /// Difficulty-indexed feel / hearing ranges `[Beginner, Easy, Normal, Hard]` (`en*GetFeelRange`).
    pub hearing_ranges: [f32; 4],
    /// Optional `NightLightOff` (`4`) fixed override for feel/hearing range (e.g. `500.0` SCU/MKN, `750.0` NSE).
    pub hearing_dark_override: Option<f32>,
    /// Multiplier applied to `hearing_ranges[diff]` in `NightLightOff` when `hearing_dark_override` is `None` (e.g. `0.75` RED/ONI).
    pub hearing_dark_scale: f32,
    /// Primary attack range threshold by difficulty `[Beginner, Easy, Normal, Hard]` (mm).
    pub attack_ranges: [f32; 4],
    /// Primary attack half-angle threshold by difficulty `[Beginner, Easy, Normal, Hard]` (rad).
    pub attack_half_angles: [f32; 4],
    /// Optional secondary long narrow lunge cone `(range_mm, half_angle_rad)` (Nurse `1500mm @ 5°`).
    pub lunge_cone: Option<(f32, f32)>,
}

impl SensoryProfile {
    /// Lying Figure (`SCU`, `en_scu.c` `0x00163b80`, `0x001647f0`, `0x00164860`, `0x001648e0`).
    pub const fn lying_figure() -> Self {
        Self {
            sight_lit: 5000.0,
            sight_day: 3000.0,
            sight_night_light_on: 4000.0,
            sight_night_light_off: 1000.0,
            view_half_angle: 70.0 * PI / 180.0, // 1.22173 rad
            // `0x2ab150`: [400.0, 750.0, 1000.0, 1500.0]; NightLightOff -> 500.0 (`0x43fa0000`)
            hearing_ranges: [400.0, 750.0, 1000.0, 1500.0],
            hearing_dark_override: Some(500.0),
            hearing_dark_scale: 1.0,
            // `0x2ab170`: [750.0, 900.0, 1200.0, 1500.0]
            attack_ranges: [750.0, 900.0, 1200.0, 1500.0],
            // `0x2ab190`: [5°, 8°, 10°, 20°]
            attack_half_angles: [0.087_266_46, 0.139_626_34, 0.174_532_92, 0.349_065_84],
            lunge_cone: None,
        }
    }

    /// Mannequin (`MKN`, `en_mkn.c` `0x00156800`, `0x00157330`).
    pub const fn mannequin() -> Self {
        Self {
            sight_lit: 5000.0,
            sight_day: 2000.0,
            sight_night_light_on: 4000.0,
            sight_night_light_off: 1100.0,
            view_half_angle: 60.0 * PI / 180.0, // 1.0471976 rad
            // `0x2aa560`: [400.0, 750.0, 1000.0, 1500.0]; NightLightOff -> 500.0 (`0x43fa0000`)
            hearing_ranges: [400.0, 750.0, 1000.0, 1500.0],
            hearing_dark_override: Some(500.0),
            hearing_dark_scale: 1.0,
            attack_ranges: [1100.0; 4],
            attack_half_angles: [0.261_799_4; 4], // 15°
            lunge_cone: None,
        }
    }

    /// Bubble Head Nurse (`NSE`, `en_nse.c` `0x00159400`, `0x00159ba0`).
    ///
    /// Dual attack cones:
    /// - Lunge (`InLungeRange`, code `3`): `1500.0` mm @ `5°` (`0.08726646` rad), disabled in `NightLightOff`.
    /// - Strike (`InAttackRange`, code `2`): `900.0` mm @ `30°` (`0.5235988` rad).
    pub const fn nurse() -> Self {
        Self {
            sight_lit: 4000.0,
            sight_day: 4000.0,
            sight_night_light_on: 4000.0,
            sight_night_light_off: 1000.0,
            view_half_angle: 1.396_263_4, // 80°
            // `0x2aa720`: [1000.0, 1250.0, 1500.0, 1500.0]; NightLightOff -> 750.0 (`0x443b8000`)
            hearing_ranges: [1000.0, 1250.0, 1500.0, 1500.0],
            hearing_dark_override: Some(750.0),
            hearing_dark_scale: 1.0,
            attack_ranges: [900.0; 4],
            attack_half_angles: [FRAC_PI_6; 4], // 30° (`0.5235988`)
            lunge_cone: Some((1500.0, 0.087_266_46)), // 1500mm @ 5°
        }
    }

    /// Pyramid Head (`RED`, `en_red.c` `0x0015f530`, `0x001600e0`).
    pub const fn pyramid_head() -> Self {
        Self {
            sight_lit: 2500.0,
            sight_day: 2500.0,
            sight_night_light_on: 2500.0,
            sight_night_light_off: 1875.0,
            view_half_angle: PI, // 360° feel range gated in enREDCanSeePlayer
            // `0x2aab40`: [1000.0, 1500.0, 2000.0, 2500.0]; NightLightOff -> * 0.75
            hearing_ranges: [1000.0, 1500.0, 2000.0, 2500.0],
            hearing_dark_override: None,
            hearing_dark_scale: 0.75,
            attack_ranges: [900.0; 4],
            attack_half_angles: [0.174_532_92; 4],    // 10°
            lunge_cone: Some((1500.0, 0.122_173_05)), // 1500mm @ 7° (overhead cleave)
        }
    }

    /// Creeper (`TYU`, `en_tyu.c` `0x001679c0`, `0x2ab330`).
    pub const fn creeper() -> Self {
        Self {
            sight_lit: 750.0,
            sight_day: 750.0,
            sight_night_light_on: 750.0,
            sight_night_light_off: 500.0,
            view_half_angle: PI,
            // `0x2ab330`: [0.0, 500.0, 600.0, 750.0]
            hearing_ranges: [0.0, 500.0, 600.0, 750.0],
            hearing_dark_override: None,
            hearing_dark_scale: 1.0,
            attack_ranges: [150.0; 4],
            attack_half_angles: [FRAC_PI_3; 4], // 60° (`1.0471976`)
            lunge_cone: None,
        }
    }

    /// Compute effective sight cutoff distance in mm for `lighting` (`en*CanSeePlayer`).
    #[inline]
    pub const fn sight_range(&self, lighting: RoomLighting) -> f32 {
        match lighting {
            RoomLighting::Lit => self.sight_lit,
            RoomLighting::Day => self.sight_day,
            RoomLighting::NightOutdoorLightOn | RoomLighting::NightIndoorLightOn => {
                self.sight_night_light_on
            }
            RoomLighting::NightLightOff => self.sight_night_light_off,
        }
    }

    /// Compute difficulty- and lighting-indexed feel/hearing range in mm (`en*GetFeelRange`).
    #[inline]
    pub fn feel_range(&self, difficulty: DifficultyLevel, lighting: RoomLighting) -> f32 {
        if lighting.is_dark_no_light() {
            if let Some(override_mm) = self.hearing_dark_override {
                return override_mm;
            }
            return self.hearing_ranges[difficulty.as_index()] * self.hearing_dark_scale;
        }
        self.hearing_ranges[difficulty.as_index()]
    }
}

/// Normalizes an angle into (-PI, PI] range (`shAngleRegulate`).
pub fn normalize_angle(mut rad: f32) -> f32 {
    while rad > PI {
        rad -= 2.0 * PI;
    }
    while rad <= -PI {
        rad += 2.0 * PI;
    }
    rad
}

/// Evaluates whether the target is directly illuminating the enemy with their flashlight
/// (`shBattleSeeHumanLight` `SLUS_202.28` `0x00124b20` / `sh2pc.exe` `0x00542cd0`).
pub fn is_lit_by_flashlight(
    enemy_pos: Vec3,
    target_pos: Vec3,
    target_yaw: f32,
    target_light_on: bool,
) -> bool {
    if !target_light_on {
        return false;
    }
    let dx = enemy_pos.x - target_pos.x;
    let dz = enemy_pos.z - target_pos.z;
    let dist_sq = dx * dx + dz * dz;
    if dist_sq > 6000.0 * 6000.0 {
        return false;
    }

    let angle_to_enemy = dx.atan2(dz);
    let diff = (normalize_angle(angle_to_enemy - target_yaw)).abs();
    diff < 0.42
}

/// `.cld` wall line-of-sight check matching `enCheckHitEyes` (`SLUS_202.28` `0x00145540`) /
/// `enCheckHitEyes2` (`0x00145770`).
///
/// Casts a horizontal segment from `eye_from` to `eye_to` against `.cld` `walls` spanning the
/// ray's Y elevation:
/// - Returns **negative Euclidean distance** (`-dist`) when line-of-sight is **clear** (`< 0.0`).
/// - Returns **positive hit distance** (`+hit_dist >= 0.0`) when occluded by a `.cld` wall.
pub fn check_hit_eyes(eye_from: Vec3, eye_to: Vec3, walls: &[WallSegment]) -> f32 {
    let total_dist = (eye_to - eye_from).length();
    if total_dist <= 1e-4 {
        return -total_dist;
    }

    let ray_y = (eye_from.y + eye_to.y) * 0.5;
    let mut best_t: Option<f32> = None;

    for w in walls {
        // Group 2 = ceiling/non-blocking wall planes in `.cld`
        if w.group == 2 {
            continue;
        }
        // Check vertical span (Y-down PS2 coordinates: y_top <= ray_y <= y_bottom)
        if !(w.y_top <= ray_y && ray_y <= w.y_bottom) {
            continue;
        }
        if let Some(t) = intersect_segment_xz_t(eye_from.x, eye_from.z, eye_to.x, eye_to.z, w) {
            if t > 0.0 && t < 1.0 && best_t.is_none_or(|b| t < b) {
                best_t = Some(t);
            }
        }
    }

    match best_t {
        Some(t) => total_dist * t,
        None => -total_dist,
    }
}

/// `SpatialGrid`-accelerated `.cld` wall line-of-sight check matching `enCheckHitEyes`
/// (`SLUS_202.28` `0x00145540`) / `enCheckHitEyes2` (`0x00145770`).
#[inline]
pub fn check_hit_eyes_world(eye_from: Vec3, eye_to: Vec3, world: &CollisionWorld) -> f32 {
    world.check_ai_los(eye_from, eye_to)
}

/// Evaluates sensory contact of an enemy observing a target (`en*CanSeePlayer` + `en*GetFeelRange`),
/// without `.cld` wall occlusion (for unit tests or open arenas).
#[allow(clippy::too_many_arguments)]
pub fn evaluate_sensory(
    profile: &SensoryProfile,
    enemy_pos: Vec3,
    enemy_yaw: f32,
    alert_timer: f32,
    target: &PerceivableTarget,
    lighting: RoomLighting,
    difficulty: DifficultyLevel,
    dt: f32,
) -> (SensoryNotice, f32) {
    evaluate_sensory_with_los(
        profile,
        enemy_pos,
        enemy_yaw,
        alert_timer,
        target,
        lighting,
        difficulty,
        &[],
        dt,
    )
}

/// Wall obstacle source for AI LOS (`enCheckHitEyes` `0x00145540`) and pathfinding (`enSetPath`).
///
/// Uses `SpatialGrid`-accelerated [`CollisionWorld::check_ai_los`] when a [`CollisionWorld`] is
/// available, falling back to a flat [`WallSegment`] slice for unit tests.
#[derive(Debug, Clone, Copy)]
pub enum AiWallSource<'a> {
    /// Flat wall slice (linear scan with `intersect_segment_xz_t`).
    Slice(&'a [WallSegment]),
    /// Full [`CollisionWorld`] using per-sector `SpatialGrid` cell traversal.
    World(&'a CollisionWorld),
}

impl AiWallSource<'_> {
    /// Returns `true` when there are no static, extra, or dynamic walls to test.
    #[inline]
    pub fn is_empty(self) -> bool {
        match self {
            Self::Slice(w) => w.is_empty(),
            Self::World(c) => {
                c.walls.is_empty() && c.extra_walls.is_empty() && c.dynamic_walls.is_empty()
            }
        }
    }

    /// Evaluates `enCheckHitEyes` (`0x00145540`) against this wall source.
    #[inline]
    pub fn check_hit_eyes(self, eye_from: Vec3, eye_to: Vec3) -> f32 {
        match self {
            Self::Slice(w) => check_hit_eyes(eye_from, eye_to, w),
            Self::World(c) => c.check_ai_los(eye_from, eye_to),
        }
    }
}

/// Evaluates sensory contact with full `.cld` wall LOS occlusion (`enCheckHitEyes2` `0x00145770`),
/// 4-state `enGetWorldCondition` (`0x00141ca0`), Nurse dual attack cones (`enNSECanSeePlayer`
/// `0x00159400`), and exact difficulty-indexed `en*GetFeelRange` hearing (no `* 1.5` run hack).
#[allow(clippy::too_many_arguments)]
pub fn evaluate_sensory_with_los(
    profile: &SensoryProfile,
    enemy_pos: Vec3,
    enemy_yaw: f32,
    alert_timer: f32,
    target: &PerceivableTarget,
    lighting: RoomLighting,
    difficulty: DifficultyLevel,
    walls: &[WallSegment],
    dt: f32,
) -> (SensoryNotice, f32) {
    evaluate_sensory_with_wall_source(
        profile,
        enemy_pos,
        enemy_yaw,
        alert_timer,
        target,
        lighting,
        difficulty,
        AiWallSource::Slice(walls),
        dt,
    )
}

/// Evaluates sensory contact using [`AiWallSource`] (`SpatialGrid`-accelerated when backed by a
/// [`CollisionWorld`]).
#[allow(clippy::too_many_arguments)]
pub fn evaluate_sensory_with_wall_source(
    profile: &SensoryProfile,
    enemy_pos: Vec3,
    enemy_yaw: f32,
    mut alert_timer: f32,
    target: &PerceivableTarget,
    lighting: RoomLighting,
    difficulty: DifficultyLevel,
    wall_source: AiWallSource<'_>,
    dt: f32,
) -> (SensoryNotice, f32) {
    if !target.is_alive {
        return (SensoryNotice::None, 0.0);
    }

    let dx = target.position.x - enemy_pos.x;
    let dz = target.position.z - enemy_pos.z;
    let dist = (dx * dx + dz * dz).sqrt();

    let angle_to_target = dx.atan2(dz);
    let facing_diff = (normalize_angle(angle_to_target - enemy_yaw)).abs();

    let is_lit = is_lit_by_flashlight(
        enemy_pos,
        target.position,
        target.yaw,
        target.is_flashlight_on,
    );

    if is_lit {
        alert_timer = (alert_timer + dt).min(3.0);
    } else {
        alert_timer = (alert_timer - dt).max(0.0);
    }

    let los_occluded =
        !wall_source.is_empty() && wall_source.check_hit_eyes(enemy_pos, target.position) >= 0.0;
    if los_occluded {
        // Behind a wall: only alerted if illuminated by flashlight at night (`en*CanSeePlayer`
        // `if (lVar2 >= 2 && enCheckSeeLight()) return 1`) or within auditory feel range.
        if lighting.is_night() && is_lit {
            return (SensoryNotice::VisualContact, alert_timer);
        }
        let hear_range = profile.feel_range(difficulty, lighting);
        if dist <= hear_range {
            return (SensoryNotice::AuditoryContact, alert_timer);
        }
        return (SensoryNotice::None, alert_timer);
    }

    let diff_idx = difficulty.as_index();
    let sight_max = profile.sight_range(lighting);
    let hear_range = profile.feel_range(difficulty, lighting);

    // 1. Optional long narrow lunge cone (e.g. Nurse 1500mm @ 5° in `enNSECanSeePlayer` `0x00159400`;
    //    disabled in `NightLightOff`).
    if let Some((lunge_dist, lunge_angle)) = profile.lunge_cone {
        if !lighting.is_dark_no_light() && dist < lunge_dist && facing_diff < lunge_angle {
            return (SensoryNotice::InLungeRange, alert_timer);
        }
    }

    // 2. Primary close-combat attack cone (`en*CanSeePlayer` code `2`).
    if dist < profile.attack_ranges[diff_idx] && facing_diff < profile.attack_half_angles[diff_idx]
    {
        return (SensoryNotice::InAttackRange, alert_timer);
    }

    // 3. Visual contact within lighting-dependent sight range & FOV cone (or flashlight alert).
    if dist <= sight_max
        && (facing_diff <= profile.view_half_angle || (lighting.is_night() && is_lit))
    {
        return (SensoryNotice::VisualContact, alert_timer);
    }

    // 4. Difficulty-indexed omnidirectional feel/hearing range (`en*GetFeelRange`, no `* 1.5` run hack).
    if dist <= hear_range {
        return (SensoryNotice::AuditoryContact, alert_timer);
    }

    (SensoryNotice::None, alert_timer)
}

// ============================================================================
// HP- & Endurance-Scaled Walk/Move Speed Formulas (`SLUS_202.28` Enemy/en_*.c)
// ============================================================================

/// Lying Figure (`SCU`) HP-scaled walk speed multiplier (`enSCUGetWalkSpeed` `0x001646a0`,
/// table `0x2ab110`: `[0.7, 0.8, 1.0, 1.2] * (hp_rate / 250.0 + 0.6)`).
#[inline]
pub fn scu_walk_speed_scale(difficulty: DifficultyLevel, hp_rate: f32) -> f32 {
    const TABLE: [f32; 4] = [0.7, 0.8, 1.0, 1.2];
    TABLE[difficulty.as_index()] * (hp_rate / 250.0 + 0.6)
}

/// Lying Figure (`SCU`) HP-scaled crawl speed in mm/s (`enSCUGetCrawlSpeed` `0x00164740`,
/// table `0x2ab130`: `[1.0, 1.5, 2.0, 2.0] * (hp_rate / 400.0 + 0.75) * 1500.0`).
#[inline]
pub fn scu_crawl_speed_mm_s(difficulty: DifficultyLevel, hp_rate: f32) -> f32 {
    const TABLE: [f32; 4] = [1.0, 1.5, 2.0, 2.0];
    TABLE[difficulty.as_index()] * (hp_rate / 400.0 + 0.75) * 1500.0
}

/// Mannequin (`MKN`) HP-scaled walk speed multiplier (`enMKNGetWalkSpeed` `0x00157290`,
/// table `0x2aa540`: `[0.7, 0.8, 1.0, 1.2] * (hp_rate / 200.0 + 0.5)`).
#[inline]
pub fn mkn_walk_speed_scale(difficulty: DifficultyLevel, hp_rate: f32) -> f32 {
    const TABLE: [f32; 4] = [0.7, 0.8, 1.0, 1.2];
    TABLE[difficulty.as_index()] * (hp_rate / 200.0 + 0.5)
}

/// Bubble Head Nurse (`NSE`) HP-scaled walk speed multiplier (`enNSEGetWalkSpeed` `0x00159b00`,
/// table `0x2aa700`: `[0.7, 0.8, 1.0, 1.2] * (hp_rate / 200.0 + 0.5)`).
#[inline]
pub fn nse_walk_speed_scale(difficulty: DifficultyLevel, hp_rate: f32) -> f32 {
    const TABLE: [f32; 4] = [0.7, 0.8, 1.0, 1.2];
    TABLE[difficulty.as_index()] * (hp_rate / 200.0 + 0.5)
}

/// Abstract Daddy (`PAP`) HP-scaled walk speed multiplier (`enPAPGetWalkSpeed` `0x0015cc70`,
/// table `0x2aa920`: `[0.7, 0.8, 1.0, 1.2] * (hp_rate / 200.0 + 0.5)`, `× 0.5` for Boss subtype 6).
#[inline]
pub fn pap_walk_speed_scale(difficulty: DifficultyLevel, hp_rate: f32, is_boss: bool) -> f32 {
    const TABLE: [f32; 4] = [0.7, 0.8, 1.0, 1.2];
    let base = TABLE[difficulty.as_index()] * (hp_rate / 200.0 + 0.5);
    if is_boss {
        base * 0.5
    } else {
        base
    }
}

/// Flesh Lips (`IKE`) HP-scaled move speed in mm/s (`enIKEGetMoveSpeed` `0x00153280`,
/// table `0x2aa3e0`: `[0.5, 0.8, 1.0, 1.2] * (hp_rate / 500.0 + 0.8) * 500.0`).
#[inline]
pub fn ike_move_speed_mm_s(difficulty: DifficultyLevel, hp_rate: f32) -> f32 {
    const TABLE: [f32; 4] = [0.5, 0.8, 1.0, 1.2];
    TABLE[difficulty.as_index()] * (hp_rate / 500.0 + 0.8) * 500.0
}

/// Mandarin (`ARM`) HP-scaled move speed multiplier (`enARMGetMoveSpeed` `0x0023ab30`,
/// table `0x34ea60`: `[0.7, 0.8, 1.0, 1.2] * (hp_rate / 250.0 + 0.6)`, `× 4.0` if `dist > 5000.0`).
#[inline]
pub fn arm_move_speed_scale(difficulty: DifficultyLevel, hp_rate: f32, dist_mm: f32) -> f32 {
    const TABLE: [f32; 4] = [0.7, 0.8, 1.0, 1.2];
    let mut s = TABLE[difficulty.as_index()] * (hp_rate / 250.0 + 0.6);
    if dist_mm > 5000.0 {
        s *= 4.0;
    }
    s
}

/// Pyramid Head (`RED`) endurance-scaled walk speed multiplier (`enREDGetSpeed` `0x0015ff60` +
/// `enREDGetWalkSpeed` `0x0015ffa0`, table `0x2aab00`: `[0.3, 0.5, 0.8, 1.0] * (end/max * 0.75 + 0.25)`).
#[inline]
pub fn red_walk_speed_scale(
    difficulty: DifficultyLevel,
    endurance: f32,
    max_endurance: f32,
    is_stair_chase: bool,
) -> f32 {
    const TABLE: [f32; 4] = [0.3, 0.5, 0.8, 1.0];
    let ratio = if max_endurance > 0.0 {
        (endurance / max_endurance).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let mut s = TABLE[difficulty.as_index()] * (ratio * 0.75 + 0.25);
    if is_stair_chase {
        s *= 1.2;
    }
    s
}

/// Twin Pyramid Heads (`ONI`) endurance-scaled walk speed multiplier (`enONIGetSpeed` `0x0022bc90` +
/// `enONIGetWalkSpeed` `0x0022bcd0`, table `0x34c860`: `[0.5, 0.8, 1.0, 1.1] * (end/max * 0.75 + 0.25)`).
#[inline]
pub fn oni_walk_speed_scale(
    difficulty: DifficultyLevel,
    endurance: f32,
    max_endurance: f32,
    is_event_chase: bool,
) -> f32 {
    const TABLE: [f32; 4] = [0.5, 0.8, 1.0, 1.1];
    let ratio = if max_endurance > 0.0 {
        (endurance / max_endurance).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let mut s = TABLE[difficulty.as_index()] * (ratio * 0.75 + 0.25);
    if is_event_chase {
        s *= 1.5;
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flashlight_beam_detection() {
        let player_pos = Vec3::new(0.0, 0.0, 0.0);
        let player_yaw = 0.0;
        let enemy_pos_ahead = Vec3::new(0.0, 0.0, 2000.0);
        let enemy_pos_behind = Vec3::new(0.0, 0.0, -2000.0);

        assert!(is_lit_by_flashlight(
            enemy_pos_ahead,
            player_pos,
            player_yaw,
            true
        ));
        assert!(!is_lit_by_flashlight(
            enemy_pos_ahead,
            player_pos,
            player_yaw,
            false
        ));
        assert!(!is_lit_by_flashlight(
            enemy_pos_behind,
            player_pos,
            player_yaw,
            true
        ));
    }

    #[test]
    fn test_world_condition_4_states() {
        assert_eq!(
            RoomLighting::from_world_condition(false, true, false),
            RoomLighting::Day
        );
        assert_eq!(
            RoomLighting::from_world_condition(true, true, true),
            RoomLighting::NightOutdoorLightOn
        );
        assert_eq!(
            RoomLighting::from_world_condition(true, false, true),
            RoomLighting::NightIndoorLightOn
        );
        assert_eq!(
            RoomLighting::from_world_condition(true, false, false),
            RoomLighting::NightLightOff
        );
    }

    #[test]
    fn test_sensory_scu_dark_vs_lit() {
        let profile = SensoryProfile::lying_figure();
        let enemy_pos = Vec3::ZERO;
        let enemy_yaw = 0.0;
        let target = PerceivableTarget::new(Vec3::new(0.0, 0.0, 2500.0), PI, false, false);

        let (notice_lit, _) = evaluate_sensory(
            &profile,
            enemy_pos,
            enemy_yaw,
            0.0,
            &target,
            RoomLighting::Lit,
            DifficultyLevel::Normal,
            0.016,
        );
        assert_eq!(notice_lit, SensoryNotice::VisualContact);

        // NightLightOff: sight 1000mm, feel_range override 500mm -> 2500mm is undetected
        let (notice_dark, _) = evaluate_sensory(
            &profile,
            enemy_pos,
            enemy_yaw,
            0.0,
            &target,
            RoomLighting::NightLightOff,
            DifficultyLevel::Normal,
            0.016,
        );
        assert_eq!(notice_dark, SensoryNotice::None);
    }

    #[test]
    fn test_sensory_hearing_difficulty_indexed_no_run_hack() {
        let profile = SensoryProfile::mannequin();
        let enemy_pos = Vec3::ZERO;
        let enemy_yaw = 0.0;
        // Behind enemy at 1200mm: Normal feel range is 1000mm (no *1.5 run hack), Hard is 1500mm.
        let running_target = PerceivableTarget::new(Vec3::new(0.0, 0.0, -1200.0), 0.0, false, true);

        let (notice_normal, _) = evaluate_sensory(
            &profile,
            enemy_pos,
            enemy_yaw,
            0.0,
            &running_target,
            RoomLighting::Day,
            DifficultyLevel::Normal,
            0.016,
        );
        assert_eq!(notice_normal, SensoryNotice::None);

        let (notice_hard, _) = evaluate_sensory(
            &profile,
            enemy_pos,
            enemy_yaw,
            0.0,
            &running_target,
            RoomLighting::Day,
            DifficultyLevel::Hard,
            0.016,
        );
        assert_eq!(notice_hard, SensoryNotice::AuditoryContact);
    }

    #[test]
    fn test_nurse_dual_attack_cones() {
        let profile = SensoryProfile::nurse();
        // 1300mm straight ahead (0 deg < 5 deg) in lit room -> InLungeRange (code 3)
        let lunge_target = PerceivableTarget::new(Vec3::new(0.0, 0.0, 1300.0), PI, false, false);
        let (notice_lunge, _) = evaluate_sensory(
            &profile,
            Vec3::ZERO,
            0.0,
            0.0,
            &lunge_target,
            RoomLighting::NightIndoorLightOn,
            DifficultyLevel::Normal,
            0.016,
        );
        assert_eq!(notice_lunge, SensoryNotice::InLungeRange);

        // Same lunge target in NightLightOff -> lunge disabled, > 1000mm sight -> None
        let (notice_dark, _) = evaluate_sensory(
            &profile,
            Vec3::ZERO,
            0.0,
            0.0,
            &lunge_target,
            RoomLighting::NightLightOff,
            DifficultyLevel::Normal,
            0.016,
        );
        assert_eq!(notice_dark, SensoryNotice::None);

        // 700mm at 20 deg (< 30 deg, < 900mm) -> InAttackRange (code 2)
        let angle = 20.0_f32.to_radians();
        let close_target = PerceivableTarget::new(
            Vec3::new(700.0 * angle.sin(), 0.0, 700.0 * angle.cos()),
            PI,
            false,
            false,
        );
        let (notice_close, _) = evaluate_sensory(
            &profile,
            Vec3::ZERO,
            0.0,
            0.0,
            &close_target,
            RoomLighting::NightLightOff,
            DifficultyLevel::Normal,
            0.016,
        );
        assert_eq!(notice_close, SensoryNotice::InAttackRange);
    }

    #[test]
    fn test_check_hit_eyes_cld_los() {
        let wall = WallSegment {
            start: [-500.0, 1000.0],
            end: [500.0, 1000.0],
            y_top: -2000.0,
            y_bottom: 200.0,
            group: 1,
            material: 0,
        };
        let eye_from = Vec3::new(0.0, -750.0, 0.0);
        let eye_to = Vec3::new(0.0, -750.0, 2000.0);

        // Clear LOS returns -2000.0
        assert!((check_hit_eyes(eye_from, eye_to, &[]) + 2000.0).abs() < 1e-3);
        // Wall at Z=1000 returns +1000.0
        assert!((check_hit_eyes(eye_from, eye_to, &[wall]) - 1000.0).abs() < 1e-3);
    }
}
