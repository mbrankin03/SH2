//! Pure PS2 enemy-proximity evaluation, `.cam` flag filtering, and stage-event road priority
//! rules (`SH2-52`).
//!
//! Ground-truth PS2 `SLUS_202.28` references:
//! - `vcSetAllNpcDeadTimer` (`0x001962a0`): post-death enemy timer (`0.0..=10.0` s)
//! - `vcSetNearestEnemyDataInVC_WORK` (`0x00196f20`): `7500.0` mm XZ radius, `1.5` s dead grace,
//!   and alerted-over-passive candidate selection
//! - `vcSetNearRoadAryByCharaPos` (`0x00197310`, `0x19738c..0x1973b8`): `0x0020` / `0x0040` filtering
//! - `vcRetRoadUsePriority` (`0x00197a10`) & `vcRetPrioStgEvnt` (`0x00294ab0`): stage-event priority
//! - `vcAdvantageDistOfOldCurRoad` (`0x00198ae0`): per-area hysteresis boundary width

use glam::Vec3;
use sh2_assets::formats::cam::{
    AreaSizeType, CamFile, RoadRecord, RoadType, VC_RD_NO_EXTRA_AREA_F, VC_RD_USE_NEAR_ENEMY_F,
    VC_RD_USE_NO_ENEMY_F,
};

/// Maximum horizontal XZ enemy detection radius in PS2 world millimeters (`0x45ea6000` in
/// `vcSetNearestEnemyDataInVC_WORK` at `0x00196f20`).
pub const VC_NEAR_ENEMY_MAX_DIST_MM: f32 = 7500.0;

/// Grace duration in seconds during which a newly killed enemy continues to hold `NEAR_ENEMY`
/// combat framing (`0x3fc00000` in `vcSetNearestEnemyDataInVC_WORK` at `0x00196fd4`).
pub const VC_DEAD_ENEMY_GRACE_SECS: f32 = 1.5;

/// Upper clamp in seconds for an enemy's accumulated post-death timer (`0x41200000` in
/// `vcSetAllNpcDeadTimer` at `0x0019630c`).
pub const VC_DEAD_ENEMY_TIMER_MAX_SECS: f32 = 10.0;

/// Maximum number of character target slots scanned per tick (`0x14` in `vcSetNearestEnemyDataInVC_WORK`
/// at `0x00197120` and `shCameraGetNearTarget` at `0x00124770`).
pub const VC_MAX_CAMERA_TARGETS: usize = 20;

/// Extra switch-area hysteresis padding in PS2 world millimeters indexed by `AreaSizeType`
/// (`extra_boundary_width` at `0x00390600`, read by `vcAdvantageDistOfOldCurRoad` at `0x00198ae0`).
pub const VC_EXTRA_BOUNDARY_WIDTH_MM: [f32; 4] = [75.0, 125.0, 175.0, 250.0];

/// Hysteresis advantage in PS2 world millimeters granted to active `Effect` / `Event` roads
/// (`0x42480000` in `vcAdvantageDistOfOldCurRoad` at `0x00198b7c`).
pub const VC_EVENT_EFFECT_ADVANTAGE_MM: f32 = 50.0;

/// Alertness tier used by `vcSetNearestEnemyDataInVC_WORK` (`0x00196f20`) when ranking near enemies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnemyCameraAlertState {
    /// Actively hostile/engaged enemy (`(chara->flags_170 & 0x04) == 0` at `0x001970a8`),
    /// preferred over passive enemies within `7500.0` mm.
    Alerted,
    /// Unalerted or stalker-dormant enemy (`$16` fallback candidate at `0x00197158`).
    Passive,
}

/// Lifecycle state of an enemy slot for camera framing (`vcSetAllNpcDeadTimer` at `0x001962a0`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EnemyLifeCameraState {
    /// Living targetable enemy (`dead_timer == 0.0` s).
    Alive,
    /// Defeated enemy accumulating post-death seconds (`0.0..=10.0` s at `+0x134`); remains eligible
    /// while `dead_timer_secs <= 1.5` s (`0x00196fe0`).
    DyingOrDead {
        /// Elapsed seconds since death (`+0x134` in `SubCharacter`).
        dead_timer_secs: f32,
    },
    /// Explicitly excluded from camera tracking (`(chara->flags_170 & 0x20) != 0` at `0x00197004`).
    CameraIgnored,
}

impl EnemyLifeCameraState {
    /// Advances the post-death timer by `dt_secs` seconds (`vcSetAllNpcDeadTimer` at `0x001962a0`).
    #[inline]
    pub fn step_dead_timer(&mut self, dt_secs: f32) {
        if let Self::DyingOrDead { dead_timer_secs } = self {
            *dead_timer_secs =
                (*dead_timer_secs + dt_secs.max(0.0)).min(VC_DEAD_ENEMY_TIMER_MAX_SECS);
        }
    }

    /// Returns `true` if this enemy currently qualifies for `vcSetNearestEnemyDataInVC_WORK` (`0x00196fd0`).
    #[inline]
    pub fn is_camera_eligible(self) -> bool {
        match self {
            Self::Alive => true,
            Self::DyingOrDead { dead_timer_secs } => dead_timer_secs <= VC_DEAD_ENEMY_GRACE_SECS,
            Self::CameraIgnored => false,
        }
    }
}

/// Descriptor for a single enemy candidate evaluated by `vcSetNearestEnemyDataInVC_WORK` (`0x00196f20`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EnemyCameraTarget {
    /// Slot identifier (`0..20` in `shCameraGetNearTarget` at `0x00124770`).
    pub slot_id: u16,
    /// World-space position in PS2 millimeters (Y-down).
    pub position: Vec3,
    /// Alertness tier (`Alerted` prioritized over `Passive`).
    pub alert_state: EnemyCameraAlertState,
    /// Living / post-death grace / ignored state.
    pub life_state: EnemyLifeCameraState,
}

/// Player status gate checked at the entry of `vcSetNearestEnemyDataInVC_WORK` (`0x00196f60`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlayerCameraSuppression {
    /// Normal gameplay; enemy proximity tracking is active.
    #[default]
    Active,
    /// Player dying or event-suppressed (`(shPlayerWork.flags_170 & 0x10) != 0`), forcing
    /// `nearest_enemy_p = NULL` and `nearest_enemy_xz_dist = 7500.0` mm.
    SuppressedByDyingOrEvent,
}

/// Result of `vcSetNearestEnemyDataInVC_WORK` (`0x00196f20`, stored at `vcWork+0xab80..0xab88`).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum EnemyProximityState {
    /// No qualifying enemy within `7500.0` mm (`nearest_enemy_p == NULL`).
    #[default]
    Clear,
    /// At least one qualifying enemy lies within `7500.0` mm XZ of the player.
    NearEnemy {
        /// Winning enemy slot identifier.
        slot_id: u16,
        /// Horizontal XZ Euclidean distance in PS2 millimeters (`< 7500.0` mm).
        xz_distance_mm: f32,
        /// Alertness tier of the winning enemy.
        alert_state: EnemyCameraAlertState,
    },
}

impl EnemyProximityState {
    /// Returns the PS2 `nearest_enemy_xz_dist` scalar in millimeters (`7500.0` when `Clear`).
    #[inline]
    pub const fn xz_distance_or_max_mm(self) -> f32 {
        match self {
            Self::Clear => VC_NEAR_ENEMY_MAX_DIST_MM,
            Self::NearEnemy { xz_distance_mm, .. } => xz_distance_mm,
        }
    }
}

/// Stage event completion gate controlling `RoadType::Event` and `RoadType::Effect` priority
/// (`vcRetRoadUsePriority` at `0x00197a10` + `vcRetPrioStgEvnt` at `0x00294ab0`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StageEventRoadGate {
    /// Stage event is pending (`vcRetPrioStgEvnt == 0`): `Event` has priority `6`, `Effect` has `5`.
    #[default]
    Active,
    /// Stage event flag has completed (`vcRetPrioStgEvnt != 0`): `Event` and `Effect` drop to `0`.
    Completed,
}

/// Self-view search button state controlling `RoadType::SelfViewOnly` priority (`0x00197ac0`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SelfViewGate {
    /// Standard camera view (`(vcWork.flags & VC_PRS_F_VIEW_F) == 0`): `SelfViewOnly` priority is `0`.
    #[default]
    Inactive,
    /// Search view held (`(vcWork.flags & VC_PRS_F_VIEW_F) != 0`): `SelfViewOnly` priority is `4`.
    Active,
}

/// Evaluates whether a `.cam` road's `flags` pass the enemy-proximity filter in
/// `vcSetNearRoadAryByCharaPos` (`SLUS_202.28` `0x0019738c..0x1973b8`).
#[inline]
pub const fn is_road_eligible_for_enemy(flags: u16, proximity: EnemyProximityState) -> bool {
    match proximity {
        EnemyProximityState::NearEnemy { .. } => (flags & VC_RD_USE_NO_ENEMY_F) == 0,
        EnemyProximityState::Clear => (flags & VC_RD_USE_NEAR_ENEMY_F) == 0,
    }
}

/// Computes the effective road priority matching `vcRetRoadUsePriority` (`SLUS_202.28` `0x00197a10`).
#[inline]
pub const fn effective_road_priority(
    rec: &RoadRecord,
    event_gate: StageEventRoadGate,
    self_view: SelfViewGate,
) -> i32 {
    match rec.rd_type {
        RoadType::Road => 2,
        RoadType::RoadPrioLow => 1,
        RoadType::RoadPrioHigh => 3,
        RoadType::Effect => match event_gate {
            StageEventRoadGate::Active => 5,
            StageEventRoadGate::Completed => 0,
        },
        RoadType::Event => match event_gate {
            StageEventRoadGate::Active => 6,
            StageEventRoadGate::Completed => 0,
        },
        RoadType::SelfViewOnly => match self_view {
            SelfViewGate::Active => 4,
            SelfViewGate::Inactive => 0,
        },
    }
}

/// Computes the boundary hysteresis advantage in PS2 world millimeters granted to the currently
/// active road (`vcAdvantageDistOfOldCurRoad` at `SLUS_202.28` `0x00198ae0`).
#[inline]
pub const fn old_road_advantage_mm(rec: &RoadRecord) -> f32 {
    if (rec.flags & VC_RD_NO_EXTRA_AREA_F) != 0 {
        return 0.0;
    }
    match rec.rd_type {
        RoadType::Effect | RoadType::Event => VC_EVENT_EFFECT_ADVANTAGE_MM,
        RoadType::Road
        | RoadType::RoadPrioLow
        | RoadType::RoadPrioHigh
        | RoadType::SelfViewOnly => match rec.area_size_type {
            AreaSizeType::Tiny => VC_EXTRA_BOUNDARY_WIDTH_MM[0],
            AreaSizeType::Small => VC_EXTRA_BOUNDARY_WIDTH_MM[1],
            AreaSizeType::Wide => VC_EXTRA_BOUNDARY_WIDTH_MM[2],
            AreaSizeType::Outdoor => VC_EXTRA_BOUNDARY_WIDTH_MM[3],
        },
    }
}

/// Evaluates the nearest qualifying enemy within `7500.0` mm XZ of `player_pos`, reproducing
/// `vcSetNearestEnemyDataInVC_WORK` (`SLUS_202.28` `0x00196f20`).
pub fn evaluate_nearest_enemy(
    player_pos: Vec3,
    player_suppression: PlayerCameraSuppression,
    enemies: &[EnemyCameraTarget],
) -> EnemyProximityState {
    if player_suppression == PlayerCameraSuppression::SuppressedByDyingOrEvent {
        return EnemyProximityState::Clear;
    }

    let mut best_alerted: Option<(u16, f32)> = None;
    let mut best_any: Option<(u16, f32, EnemyCameraAlertState)> = None;

    for enemy in enemies.iter().take(VC_MAX_CAMERA_TARGETS) {
        if !enemy.life_state.is_camera_eligible() {
            continue;
        }
        let dx = enemy.position.x - player_pos.x;
        let dz = enemy.position.z - player_pos.z;
        if dx.abs() >= VC_NEAR_ENEMY_MAX_DIST_MM || dz.abs() >= VC_NEAR_ENEMY_MAX_DIST_MM {
            continue;
        }
        let dist_xz = (dx * dx + dz * dz).sqrt();
        if dist_xz < best_any.map_or(VC_NEAR_ENEMY_MAX_DIST_MM, |(_, d, _)| d) {
            best_any = Some((enemy.slot_id, dist_xz, enemy.alert_state));
        }
        if enemy.alert_state == EnemyCameraAlertState::Alerted
            && dist_xz < best_alerted.map_or(VC_NEAR_ENEMY_MAX_DIST_MM, |(_, d)| d)
        {
            best_alerted = Some((enemy.slot_id, dist_xz));
        }
    }

    if let Some((slot_id, xz_distance_mm)) = best_alerted {
        EnemyProximityState::NearEnemy {
            slot_id,
            xz_distance_mm,
            alert_state: EnemyCameraAlertState::Alerted,
        }
    } else if let Some((slot_id, xz_distance_mm, alert_state)) = best_any {
        EnemyProximityState::NearEnemy {
            slot_id,
            xz_distance_mm,
            alert_state,
        }
    } else {
        EnemyProximityState::Clear
    }
}

/// Selects the canonical road index in `cam_file.records` for `player_pos` (in PS2 mm), applying
/// `VC_RD_USE_NEAR_ENEMY_F` (`0x0040`), `VC_RD_USE_NO_ENEMY_F` (`0x0020`), dynamic priority, and
/// old-road hysteresis (`vcSetNearRoadAryByCharaPos` + `vcGetBestNewCurNearRoad` `0x00198260`).
pub fn select_best_road_index(
    cam_file: &CamFile,
    player_pos: Vec3,
    margin_mm: f32,
    prev_road_index: Option<usize>,
    proximity: EnemyProximityState,
    event_gate: StageEventRoadGate,
    self_view: SelfViewGate,
) -> Option<usize> {
    let pos = [player_pos.x, player_pos.y, player_pos.z];
    let mut best_idx: Option<usize> = None;
    let mut best_dist = f32::MAX;
    let mut best_prio = -1;

    for (idx, rec) in cam_file.records.iter().enumerate() {
        if !is_road_eligible_for_enemy(rec.flags, proximity) {
            continue;
        }
        let prio = effective_road_priority(rec, event_gate, self_view);
        if prio <= 0 {
            continue;
        }
        let extra = if prev_road_index == Some(idx) {
            old_road_advantage_mm(rec)
        } else {
            0.0
        };
        let inside = rec.lim_sw.contains_point(pos, margin_mm + extra);
        let dist = if inside {
            -extra
        } else {
            (rec.lim_sw.distance_to_xz(pos[0], pos[2]) - extra).max(0.0)
        };

        if inside {
            if prio > best_prio || (prio == best_prio && dist < best_dist) {
                best_idx = Some(idx);
                best_prio = prio;
                best_dist = dist;
            }
        } else if best_prio <= 0 && dist < best_dist {
            best_idx = Some(idx);
            best_dist = dist;
        }
    }

    best_idx
}
