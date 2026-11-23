//! 1:1 Silent Hill 2 Pocket Radio static modulation (`RadioNoise` & `enSetRadioVolume`, `SH2-137`).
//!
//! # Ground Truth (`SLUS_202.28` & `sh2pc.exe`)
//! - **`enSetRadioVolume`** (`SLUS_202.28` `Enemy/en_common.c` `0x00143890`):
//!   Computes each enemy's base radio emission weight (`0.0`, `0.5`, `0.8`, or `1.0`) from its
//!   `enLocalWork.kind` (`1..=15`, `enConvertId2Kind` `en_common.c` lines 455–505), `sub_type`,
//!   spawned-active flag (`+0x04 & 0x10`), alive flag (`+0x170 & 0x01`), and downed flag (`+0x170 & 0x04`).
//! - **`RadioNoise`** (`SLUS_202.28` `Event/event.c` `0x001ab300`) & **`enemy_track$2443`** (`0x002b9850`):
//!   - **Mode 0 (Normal)**: Selects up to 3 nearest enemies with `dist <= 12500.0` mm and `radio_vol != 0.0`,
//!     evaluates the cosine proximity curve `cos((dist / 12500.0) * PI / 2.0)`, mixes 4 static tracks
//!     using `enemy_track$2443[kind][track]` with contributor weights `(1.0, 1/3, 1/6)`, ramps track
//!     volumes at `±dt / 2.5` and track pans at `±dt / 2.0`, applies menu/map global ducking
//!     (`1.0` normal, `0.5` pause/inventory `mode 5|6`, `0.25` map/memo `mode 8`, ramped at `±dt`),
//!     and outputs `shSdRadio(track 1..=4, vol, pan)`.
//!   - **Mode 1 (`WorldSource`)**: World-positioned radio (before pickup), using `SeCallPosDistance(0.8, pos)`
//!     (`0x3f4ccccd`) and `SeCallPosDirection(pos)` wrapped into `[-90, 90]`, multiplied by `enemy_track$2443[1]`,
//!     pan `(pan * -60.0 + 64.0)`.
//!   - **Mode 2 (`ForcedFirstEnemy`)**: Uses the first active enemy's `radio_vol` (or `1.0` if none)
//!     multiplied by `enemy_track$2443[1]`, ramped at `±dt / 2.5`, center pan `64` (`0x40`).
//!
//! # Prototype (`SLUS_202.28`) vs. Retail PC (`sh2pc.exe`) Drift
//! - `SLUS_202.28` (`0x001ab300`) uses a single global `12500.0` mm cosine radius and 3-nearest-enemy
//!   `(1.0, 1/3, 1/6)` mixing across 4 tracks.
//! - Retail `sh2pc.exe` (`0x0048c900` in `sh2pc_00480000.c`) expanded `RadioNoise` with per-species
//!   per-track radius tables, a `1.5` inner deadzone, a threat-score threshold (`>= 9`) gating track 3,
//!   geometric `1/3^k` decay across sorted contributors, and Mode 3 (inventory volume preview).

use std::f32::consts::FRAC_PI_2;

use glam::{Mat4, Vec3, Vec4};
use serde::{Deserialize, Serialize};

use crate::ai::creatures::EnemySpecies;

/// Maximum Pocket Radio detection radius in world mm (`RadioNoise` `SLUS_202.28` `0x001ab300`).
pub const RADIO_MAX_DISTANCE_MM: f32 = 12_500.0;

/// Number of simultaneous static tracks driven by `shSdRadio` (`0x001ab300`, tracks `1..=4`).
pub const RADIO_TRACK_COUNT: usize = 4;

/// Maximum number of nearest enemies mixed in `RadioNoise` Mode 0 (`0x001ab300`, 3 slots).
pub const RADIO_MAX_NEAREST_ENEMIES: usize = 3;

/// Per-track volume ramp divisor in seconds (`dt / 2.5`, `RadioNoise` `0x001ab300`).
pub const RADIO_VOLUME_RAMP_DIVISOR: f32 = 2.5;

/// Per-track stereo pan ramp divisor in seconds (`dt / 2.0`, `RadioNoise` `0x001ab300`).
pub const RADIO_PAN_RAMP_DIVISOR: f32 = 2.0;

/// Contributor weights for the 3 nearest enemies (`1.0`, `1/3`, `1/6`, `RadioNoise` `0x001ab300`).
pub const RADIO_CONTRIB_WEIGHTS: [f32; RADIO_MAX_NEAREST_ENEMIES] = [1.0, 1.0 / 3.0, 1.0 / 6.0];

/// Base volume argument (`0x3f4ccccd` = `0.8`) passed to `SeCallPosDistance` in Mode 1 (`0x001ab300`).
pub const RADIO_MODE1_BASE_VOLUME: f32 = 0.8;

/// Maximum discrete inventory radio volume setting (`DAT_01126900`, `0..=15`, `0x001ab300`).
pub const RADIO_MAX_VOLUME_LEVEL: u8 = 15;

/// Exact 16×4 species-to-track weight matrix dumped from `SLUS_202.28` `enemy_track$2443` (`0x002b9850`).
///
/// Indexed by `enLocalWork.kind` (`0..=15`, `en_common.c` lines 455–505):
/// - `1`: Lying Figure (`0x200`) `[1.0, 0.0, 1.0, 0.0]`
/// - `2`: Mannequin (`0x201`) `[0.0, 1.0, 1.0, 0.0]`
/// - `3`: Creeper (`0x202`) `[0.6, 0.0, 0.0, 0.0]`
/// - `4`: Pyramid Head (`0x208`) `[0.4, 0.4, 0.4, 1.0]`
/// - `5`: Twin Pyramid Head (`0x209`) `[0.4, 0.4, 0.4, 1.0]`
/// - `6`: Bubble Head Nurse (`0x207`/`0x20b`) `[1.0, 1.0, 0.0, 0.0]`
/// - `7`: Flesh Lips (`0x203`) `[0.0, 0.0, 0.0, 0.0]`
/// - `8`: Abstract Daddy (`0x204`) `[0.0, 1.0, 0.8, 0.4]`
/// - `9`: Eddie (`0x205`) `[0.0, 0.0, 0.0, 0.0]`
/// - `10`: Mandarin (`0x20a`) `[1.0, 0.8, 0.0, 0.2]`
/// - `11`: Mary Boss (`0x206`) `[0.0, 0.0, 0.0, 0.0]`
/// - `13`/`14`: `ty2`/`fly` (`0x20c`/`0x20d`) `[0.6, 0.0, 0.0, 0.0]`
pub const ENEMY_RADIO_TRACK_WEIGHTS: [[f32; RADIO_TRACK_COUNT]; 16] = [
    [0.0, 0.0, 0.0, 0.0], // 0: none
    [1.0, 0.0, 1.0, 0.0], // 1: Lying Figure (scu)
    [0.0, 1.0, 1.0, 0.0], // 2: Mannequin (mkn)
    [0.6, 0.0, 0.0, 0.0], // 3: Creeper (tyu)
    [0.4, 0.4, 0.4, 1.0], // 4: Pyramid Head (red)
    [0.4, 0.4, 0.4, 1.0], // 5: Twin Pyramid Head (oni)
    [1.0, 1.0, 0.0, 0.0], // 6: Bubble Head Nurse (nse)
    [0.0, 0.0, 0.0, 0.0], // 7: Flesh Lips (ike)
    [0.0, 1.0, 0.8, 0.4], // 8: Abstract Daddy (pap)
    [0.0, 0.0, 0.0, 0.0], // 9: Eddie (edb)
    [1.0, 0.8, 0.0, 0.2], // 10: Mandarin (arm)
    [0.0, 0.0, 0.0, 0.0], // 11: Mary Boss (bos)
    [0.0, 0.0, 0.0, 0.0], // 12: nik (0x421)
    [0.6, 0.0, 0.0, 0.0], // 13: ty2 (0x20c)
    [0.6, 0.0, 0.0, 0.0], // 14: fly (0x20d)
    [0.0, 0.0, 0.0, 0.0], // 15: ins (0x20f)
];

/// Converts an [`EnemySpecies`] to its `SLUS_202.28` `enLocalWork.kind` index (`1..=15`, `en_common.c` lines 455–505).
#[inline]
pub const fn en_kind_from_species(species: EnemySpecies) -> u8 {
    match species {
        EnemySpecies::LyingFigure => 1,
        EnemySpecies::Mannequin => 2,
        EnemySpecies::Creeper => 3,
        EnemySpecies::PyramidHead => 4,
        EnemySpecies::TwinPyramidHead => 5,
        EnemySpecies::Nurse => 6,
        EnemySpecies::FleshLips => 7,
        EnemySpecies::AbstractDaddy => 8,
        EnemySpecies::Eddie => 9,
        EnemySpecies::Mandarin => 10,
        EnemySpecies::MaryBoss => 11,
        EnemySpecies::RoomInsect => 15,
    }
}

/// Evaluates `enSetRadioVolume` (`SLUS_202.28` `Enemy/en_common.c` `0x00143890`), returning the
/// enemy's base radio emission weight (`0.0`, `0.5`, `0.8`, or `1.0`).
#[inline]
pub const fn en_set_radio_volume(
    kind: u8,
    sub_type: u8,
    is_spawned_active: bool,
    is_alive: bool,
    is_downed: bool,
) -> f32 {
    if kind == 4 && sub_type == 0 {
        1.0
    } else if matches!(kind, 4 | 5 | 7 | 9 | 11) || !is_spawned_active || !is_alive {
        0.0
    } else if kind == 3 {
        0.5
    } else if !is_downed {
        1.0
    } else {
        0.8
    }
}

/// Evaluates the `RadioNoise` (`0x001ab300`) cosine proximity curve:
/// `cos((dist_mm / 12500.0) * PI / 2.0)` for `dist_mm <= 12500.0`, and `0.0` beyond.
#[inline]
pub fn radio_cosine_attenuation(dist_mm: f32) -> f32 {
    if !dist_mm.is_finite() || dist_mm < 0.0 || dist_mm > RADIO_MAX_DISTANCE_MM {
        return 0.0;
    }
    ((dist_mm / RADIO_MAX_DISTANCE_MM) * FRAC_PI_2)
        .cos()
        .max(0.0)
}

/// Pocket Radio operating mode (`_DAT_011263e4` in `RadioNoise` `0x001ab300`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub enum RadioMode {
    /// Mode 0: Normal carried pocket radio with 3-nearest-enemy 4-track mixing.
    #[default]
    Normal,
    /// Mode 1: World-positioned radio emitting directional static before pickup (`0x001ab300` lines 1871–1892).
    WorldSource { position: [f32; 3] },
    /// Mode 2: Forced static driven by the first active enemy's `radio_vol` (or `1.0` if none, lines 1893–1923).
    ForcedFirstEnemy,
}

/// Menu/map ducking state for `RadioNoise` (`_DAT_01126408`, `0x001ab300` lines 2024–2046).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum RadioDuckingState {
    /// Normal gameplay: `1.0`.
    #[default]
    Normal,
    /// Pause or Inventory (`game_mode == 5 || game_mode == 6`): `0.5`.
    PauseOrInventory,
    /// Map or Memo (`game_mode == 8`): `0.25`.
    MapOrMemo,
}

impl RadioDuckingState {
    /// Target `_DAT_01126408` ducking scalar (`0x001ab300` lines 2024–2032).
    #[inline]
    pub const fn target_gain(self) -> f32 {
        match self {
            Self::Normal => 1.0,
            Self::PauseOrInventory => 0.5,
            Self::MapOrMemo => 0.25,
        }
    }
}

/// Per-enemy snapshot fed into [`RadioNoiseEvaluator::evaluate_enemies`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RadioEnemySample {
    /// `enLocalWork.kind` (`1..=15`, see [`en_kind_from_species`]).
    pub kind: u8,
    /// Enemy subtype (`enLocalWork[5]`, used by Pyramid Head `kind == 4`).
    pub sub_type: u8,
    /// World-space position in mm (`chara + 0x20`).
    pub position: Vec3,
    /// `*(u32*)(chara + 0x04) & 0x10 != 0` (active in scene).
    pub is_spawned_active: bool,
    /// `*(u32*)(chara + 0x170) & 0x01 != 0` (alive).
    pub is_alive: bool,
    /// `*(u32*)(chara + 0x170) & 0x04 != 0` (downed / lying).
    pub is_downed: bool,
}

impl RadioEnemySample {
    /// Computes this enemy's `enSetRadioVolume` (`0x00143890`) emission weight.
    #[inline]
    pub const fn radio_volume(&self) -> f32 {
        en_set_radio_volume(
            self.kind,
            self.sub_type,
            self.is_spawned_active,
            self.is_alive,
            self.is_downed,
        )
    }
}

/// Complete 4-track Pocket Radio output computed every tick (`RadioNoise` `0x001ab300`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RadioStaticOutput {
    /// Whether the pocket radio is powered on and active (`_radio != 0`).
    pub is_active: bool,
    /// Combined scalar loudness summary (`max(track_volumes) * global_duck * (vol_level / 15.0)`) in `[0.0, 1.0]`.
    pub volume: f32,
    /// 3D Euclidean distance to the nearest emitting enemy in world mm (`f32::MAX` if none).
    pub nearest_enemy_dist: f32,
    /// Ramped per-track linear volumes (`0x011263e8`, `[0.0, 1.0]`).
    pub track_volumes: [f32; RADIO_TRACK_COUNT],
    /// Ramped per-track normalized pans (`0x011263f8`, `[-1.0, 1.0]`).
    pub track_pans: [f32; RADIO_TRACK_COUNT],
    /// Integer `shSdRadio` track volumes (`0..=127`).
    pub driver_volumes: [u8; RADIO_TRACK_COUNT],
    /// Integer `shSdRadio` track pans (`0..=127`, `64` = center).
    pub driver_pans: [u8; RADIO_TRACK_COUNT],
    /// Ramped menu/map ducking scalar (`_DAT_01126408`, `[0.0, 1.0]`).
    pub global_duck: f32,
}

impl Default for RadioStaticOutput {
    fn default() -> Self {
        Self {
            is_active: false,
            volume: 0.0,
            nearest_enemy_dist: f32::MAX,
            track_volumes: [0.0; RADIO_TRACK_COUNT],
            track_pans: [0.0; RADIO_TRACK_COUNT],
            driver_volumes: [0; RADIO_TRACK_COUNT],
            driver_pans: [64; RADIO_TRACK_COUNT],
            global_duck: 0.0,
        }
    }
}

/// Pure `sh2-core` equivalent of `SeCallPosDirection` (`0x00192990`) used by `RadioNoise` (`0x001ab300`).
fn radio_pos_direction_deg(wvs_matrix: Mat4, source_pos: Vec3) -> i32 {
    let view = wvs_matrix * Vec4::new(source_pos.x, source_pos.y, source_pos.z, 1.0);
    let vx = view.x - 750.0;
    let vz = view.z;
    let len_sq = vx * vx + vz * vz;
    let (nx, nz) = if len_sq > 0.0 {
        let inv = len_sq.sqrt().recip();
        if inv.is_finite() {
            (vx * inv, vz * inv)
        } else {
            (vx, vz)
        }
    } else {
        (vx, vz)
    };
    let neg_x = -nx;
    let sign = if neg_x > 0.0 {
        1.0
    } else if neg_x < 0.0 {
        -1.0
    } else {
        0.0
    };
    (sign * (70.0 - nz * 70.0)) as i32
}

/// Pure `sh2-core` equivalent of `SeCallPosDistance` (`0x00192a70`) used by `RadioNoise` Mode 1.
fn radio_pos_distance_byte(
    base_volume: f32,
    camera_pos: Vec3,
    player_pos: Vec3,
    src_pos: Vec3,
) -> i32 {
    let d_cam = (camera_pos - player_pos).length();
    let d_src = (src_pos - player_pos).length();
    let f_cam = ((d_cam - 1500.0) * 0.0001).clamp(0.0, 1.0);
    let f_src = ((d_src - 1500.0) * 0.0001).clamp(0.0, 1.0);
    let atten = (1.0 - f_cam) * (1.0 - f_src);
    (255.0 - base_volume.clamp(0.0, 1.0) * atten * 255.0).clamp(0.0, 255.0) as i32
}

/// Stateful 1:1 Pocket Radio evaluator (`RadioNoise` `SLUS_202.28` `0x001ab300`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RadioNoiseEvaluator {
    /// Active stream state (`_radio` at `0x011263e0`).
    pub active: bool,
    /// Operating mode (`_DAT_011263e4`: `Normal`, `WorldSource`, `ForcedFirstEnemy`).
    pub mode: RadioMode,
    /// Discrete inventory radio volume (`DAT_01126900`, `0..=15`, default `15`).
    pub volume_level: u8,
    /// Menu/map ducking state (`_DAT_010f0888` -> `1.0`/`0.5`/`0.25`).
    pub ducking_state: RadioDuckingState,
    /// Ramped per-track linear volumes (`0x011263e8`).
    pub track_volumes: [f32; RADIO_TRACK_COUNT],
    /// Ramped per-track normalized pans (`0x011263f8`).
    pub track_pans: [f32; RADIO_TRACK_COUNT],
    /// Ramped global menu/map ducking scalar (`_DAT_01126408`).
    pub global_duck: f32,
}

impl Default for RadioNoiseEvaluator {
    fn default() -> Self {
        Self {
            active: false,
            mode: RadioMode::Normal,
            volume_level: RADIO_MAX_VOLUME_LEVEL,
            ducking_state: RadioDuckingState::Normal,
            track_volumes: [0.0; RADIO_TRACK_COUNT],
            track_pans: [0.0; RADIO_TRACK_COUNT],
            global_duck: 0.0,
        }
    }
}

impl RadioNoiseEvaluator {
    /// Steps `RadioNoise` (`0x001ab300`) for the current frame with full enemy metadata, camera position,
    /// and `VbWvsMatrix`.
    pub fn evaluate_enemies(
        &mut self,
        radio_enabled: bool,
        camera_pos: Vec3,
        player_pos: Vec3,
        wvs_matrix: Mat4,
        enemies: &[RadioEnemySample],
        dt_secs: f32,
    ) -> RadioStaticOutput {
        if !radio_enabled {
            self.active = false;
            self.track_volumes = [0.0; RADIO_TRACK_COUNT];
            self.track_pans = [0.0; RADIO_TRACK_COUNT];
            self.global_duck = 0.0;
            return RadioStaticOutput::default();
        }

        if !self.active {
            self.track_volumes = [0.0; RADIO_TRACK_COUNT];
            self.track_pans = [0.0; RADIO_TRACK_COUNT];
            self.global_duck = 0.0;
            self.active = true;
        }

        let dt = dt_secs.max(0.0);
        match self.mode {
            RadioMode::WorldSource { position } => {
                let pos_v = Vec3::from_array(position);
                // Mode 1 (`0x001ab300` lines 1871–1892)
                let dist_byte =
                    radio_pos_distance_byte(RADIO_MODE1_BASE_VOLUME, camera_pos, player_pos, pos_v)
                        as f32;
                let mut dir = radio_pos_direction_deg(wvs_matrix, pos_v);
                if dir > 90 {
                    dir = 180 - dir;
                }
                if dir < -90 {
                    dir = -180 - dir;
                }
                let base_atten = (255.0 - dist_byte) / 255.0;
                let norm_dir = (dir as f32) / 90.0;
                let weights = ENEMY_RADIO_TRACK_WEIGHTS[1];
                let mut driver_volumes = [0u8; RADIO_TRACK_COUNT];
                let mut driver_pans = [64u8; RADIO_TRACK_COUNT];
                for t in 0..RADIO_TRACK_COUNT {
                    self.track_volumes[t] = base_atten * weights[t];
                    self.track_pans[t] = norm_dir * weights[t];
                    driver_volumes[t] = (self.track_volumes[t] * 127.0).clamp(0.0, 127.0) as u8;
                    driver_pans[t] = (self.track_pans[t] * -60.0 + 64.0).clamp(0.0, 127.0) as u8;
                }
                let max_vol = self.track_volumes.into_iter().fold(0.0f32, f32::max);
                RadioStaticOutput {
                    is_active: true,
                    volume: max_vol,
                    nearest_enemy_dist: (pos_v - player_pos).length(),
                    track_volumes: self.track_volumes,
                    track_pans: self.track_pans,
                    driver_volumes,
                    driver_pans,
                    global_duck: 1.0,
                }
            }
            RadioMode::ForcedFirstEnemy => {
                // Mode 2 (`0x001ab300` lines 1893–1923)
                let base_vol = enemies
                    .iter()
                    .find(|e| e.kind != 0)
                    .map(|e| e.radio_volume())
                    .unwrap_or(1.0);
                let weights = ENEMY_RADIO_TRACK_WEIGHTS[1];
                let step = dt / RADIO_VOLUME_RAMP_DIVISOR;
                let mut driver_volumes = [0u8; RADIO_TRACK_COUNT];
                for t in 0..RADIO_TRACK_COUNT {
                    let target = weights[t] * base_vol;
                    if self.track_volumes[t] < target {
                        self.track_volumes[t] = (self.track_volumes[t] + step).min(target);
                    } else if target < self.track_volumes[t] {
                        self.track_volumes[t] = (self.track_volumes[t] - step).max(target);
                    }
                    driver_volumes[t] = (self.track_volumes[t] * 127.0).clamp(0.0, 127.0) as u8;
                }
                let max_vol = self.track_volumes.into_iter().fold(0.0f32, f32::max);
                RadioStaticOutput {
                    is_active: true,
                    volume: max_vol,
                    nearest_enemy_dist: f32::MAX,
                    track_volumes: self.track_volumes,
                    track_pans: [0.0; RADIO_TRACK_COUNT],
                    driver_volumes,
                    driver_pans: [64; RADIO_TRACK_COUNT],
                    global_duck: 1.0,
                }
            }
            RadioMode::Normal => {
                // Mode 0 (`0x001ab300` lines 1924–2053): select up to 3 nearest emitting enemies <= 12500mm
                let mut nearest: [Option<(usize, f32, f32)>; RADIO_MAX_NEAREST_ENEMIES] =
                    [None; RADIO_MAX_NEAREST_ENEMIES];
                for (idx, enemy) in enemies.iter().enumerate() {
                    if enemy.kind == 0 {
                        continue;
                    }
                    let rv = enemy.radio_volume();
                    if rv == 0.0 {
                        continue;
                    }
                    let dist = (enemy.position - player_pos).length();
                    if dist > RADIO_MAX_DISTANCE_MM {
                        continue;
                    }
                    for slot in 0..RADIO_MAX_NEAREST_ENEMIES {
                        let insert_here = match nearest[slot] {
                            None => true,
                            Some((_, slot_dist, _)) => dist < slot_dist,
                        };
                        if insert_here {
                            for shift in (slot + 1..RADIO_MAX_NEAREST_ENEMIES).rev() {
                                nearest[shift] = nearest[shift - 1];
                            }
                            nearest[slot] = Some((idx, dist, rv));
                            break;
                        }
                    }
                }

                let mut cos_gains = [0.0f32; RADIO_MAX_NEAREST_ENEMIES];
                let mut dirs = [0i32; RADIO_MAX_NEAREST_ENEMIES];
                for s in 0..RADIO_MAX_NEAREST_ENEMIES {
                    if let Some((idx, dist, _)) = nearest[s] {
                        cos_gains[s] = radio_cosine_attenuation(dist);
                        dirs[s] = radio_pos_direction_deg(wvs_matrix, enemies[idx].position);
                    }
                }

                let vol_step = dt / RADIO_VOLUME_RAMP_DIVISOR;
                let pan_step = dt / RADIO_PAN_RAMP_DIVISOR;
                for t in 0..RADIO_TRACK_COUNT {
                    let mut contrib = [0.0f32; RADIO_MAX_NEAREST_ENEMIES];
                    for s in 0..RADIO_MAX_NEAREST_ENEMIES {
                        if let Some((idx, _, rv)) = nearest[s] {
                            let kind = (enemies[idx].kind as usize).min(15);
                            contrib[s] = rv * ENEMY_RADIO_TRACK_WEIGHTS[kind][t];
                        }
                    }

                    let target_vol = (cos_gains[0] * contrib[0] * RADIO_CONTRIB_WEIGHTS[0]
                        + cos_gains[1] * contrib[1] * RADIO_CONTRIB_WEIGHTS[1]
                        + cos_gains[2] * contrib[2] * RADIO_CONTRIB_WEIGHTS[2])
                        .clamp(0.0, 1.0);
                    if self.track_volumes[t] < target_vol {
                        self.track_volumes[t] = (self.track_volumes[t] + vol_step).min(target_vol);
                    } else if target_vol < self.track_volumes[t] {
                        self.track_volumes[t] = (self.track_volumes[t] - vol_step).max(target_vol);
                    }

                    let active_mask = [
                        if contrib[0] > 0.0 { 1.0 } else { 0.0 },
                        if contrib[1] > 0.0 { 1.0 } else { 0.0 },
                        if contrib[2] > 0.0 { 1.0 } else { 0.0 },
                    ];
                    let target_pan = ((active_mask[0] * (dirs[0] as f32) / 90.0)
                        * RADIO_CONTRIB_WEIGHTS[0]
                        + (active_mask[1] * (dirs[1] as f32) / 90.0) * RADIO_CONTRIB_WEIGHTS[1]
                        + (active_mask[2] * (dirs[2] as f32) / 90.0) * RADIO_CONTRIB_WEIGHTS[2])
                        .clamp(-1.0, 1.0);
                    if self.track_pans[t] < target_pan {
                        self.track_pans[t] = (self.track_pans[t] + pan_step).min(target_pan);
                    } else if target_pan < self.track_pans[t] {
                        self.track_pans[t] = (self.track_pans[t] - pan_step).max(target_pan);
                    }
                }

                // Global menu/map ducking (`_DAT_01126408`, ramped at `±dt`, lines 2024–2046)
                let target_duck = self.ducking_state.target_gain();
                if self.global_duck < target_duck {
                    self.global_duck = (self.global_duck + dt).min(target_duck);
                } else if target_duck < self.global_duck {
                    self.global_duck = (self.global_duck - dt).max(target_duck);
                }

                let level_ratio = (self.volume_level.min(RADIO_MAX_VOLUME_LEVEL) as f32)
                    / (RADIO_MAX_VOLUME_LEVEL as f32);
                let mut driver_volumes = [0u8; RADIO_TRACK_COUNT];
                let mut driver_pans = [64u8; RADIO_TRACK_COUNT];
                for t in 0..RADIO_TRACK_COUNT {
                    driver_volumes[t] =
                        (self.track_volumes[t] * 127.0 * self.global_duck * level_ratio)
                            .clamp(0.0, 127.0) as u8;
                    driver_pans[t] = (self.track_pans[t] * 32.0 + 64.0).clamp(0.0, 127.0) as u8;
                }

                let max_vol = self.track_volumes.into_iter().fold(0.0f32, f32::max)
                    * self.global_duck
                    * level_ratio;
                let nearest_enemy_dist = nearest[0].map(|(_, d, _)| d).unwrap_or(f32::MAX);

                RadioStaticOutput {
                    is_active: true,
                    volume: max_vol,
                    nearest_enemy_dist,
                    track_volumes: self.track_volumes,
                    track_pans: self.track_pans,
                    driver_volumes,
                    driver_pans,
                    global_duck: self.global_duck,
                }
            }
        }
    }

    /// Convenience wrapper for callers that only have enemy positions (assumes active standing Lying Figures,
    /// identity `VbWvsMatrix`, and `camera_pos == player_pos`).
    pub fn evaluate(
        &mut self,
        radio_enabled: bool,
        player_pos: Vec3,
        enemy_positions: &[Vec3],
        dt_secs: f32,
    ) -> RadioStaticOutput {
        let samples: Vec<RadioEnemySample> = enemy_positions
            .iter()
            .map(|&position| RadioEnemySample {
                kind: 1, // Lying Figure (`[1.0, 0.0, 1.0, 0.0]`)
                sub_type: 0,
                position,
                is_spawned_active: true,
                is_alive: true,
                is_downed: false,
            })
            .collect();
        self.evaluate_enemies(
            radio_enabled,
            player_pos,
            player_pos,
            Mat4::IDENTITY,
            &samples,
            dt_secs,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_en_set_radio_volume_species_and_state_weights() {
        // Pyramid Head (kind 4): sub_type 0 => 1.0, non-zero sub_type => 0.0
        assert_eq!(en_set_radio_volume(4, 0, false, false, false), 1.0);
        assert_eq!(en_set_radio_volume(4, 1, true, true, false), 0.0);
        // Silent bosses/enemies: Twin PH (5), Flesh Lips (7), Eddie (9), Mary (11)
        for kind in [5u8, 7, 9, 11] {
            assert_eq!(en_set_radio_volume(kind, 0, true, true, false), 0.0);
        }
        // Inactive or dead => 0.0
        assert_eq!(en_set_radio_volume(1, 0, false, true, false), 0.0);
        assert_eq!(en_set_radio_volume(1, 0, true, false, false), 0.0);
        // Creeper (kind 3) => 0.5
        assert_eq!(en_set_radio_volume(3, 0, true, true, false), 0.5);
        // Standing normal enemy => 1.0, downed => 0.8
        assert_eq!(en_set_radio_volume(1, 0, true, true, false), 1.0);
        assert_eq!(en_set_radio_volume(1, 0, true, true, true), 0.8);
    }

    #[test]
    fn test_radio_cosine_curve_and_3_enemy_4_track_mixing() {
        // Cosine boundaries: 0mm -> 1.0, 6250mm -> cos(pi/4) = sqrt(2)/2, 12500mm -> 0.0, >12500mm -> 0.0
        assert!((radio_cosine_attenuation(0.0) - 1.0).abs() < 1e-6);
        assert!((radio_cosine_attenuation(6250.0) - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-5);
        assert!(radio_cosine_attenuation(12_500.0).abs() < 1e-5);
        assert_eq!(radio_cosine_attenuation(12_501.0), 0.0);

        let mut radio = RadioNoiseEvaluator::default();
        let enemies = [
            // Nearest (0mm): Lying Figure -> [1, 0, 1, 0] * 1.0
            RadioEnemySample {
                kind: 1,
                sub_type: 0,
                position: Vec3::ZERO,
                is_spawned_active: true,
                is_alive: true,
                is_downed: false,
            },
            // 2nd nearest (6250mm): Mannequin -> [0, 1, 1, 0] * (cos(pi/4) / 3)
            RadioEnemySample {
                kind: 2,
                sub_type: 0,
                position: Vec3::new(0.0, 0.0, 6250.0),
                is_spawned_active: true,
                is_alive: true,
                is_downed: false,
            },
            // 3rd nearest (0mm): Pyramid Head (sub 0) -> [0.4, 0.4, 0.4, 1.0] * (1.0 / 6.0)
            RadioEnemySample {
                kind: 4,
                sub_type: 0,
                position: Vec3::new(0.0, 0.0, 100.0),
                is_spawned_active: true,
                is_alive: true,
                is_downed: false,
            },
        ];

        // Step 3.0s so 2.5s volume ramp & 1.0s duck ramp reach steady state
        let out =
            radio.evaluate_enemies(true, Vec3::ZERO, Vec3::ZERO, Mat4::IDENTITY, &enemies, 3.0);
        // Sorted by distance: #0 Lying Figure (0mm), #1 Pyramid Head (100mm), #2 Mannequin (6250mm)
        let cos_100 = radio_cosine_attenuation(100.0);
        let cos_6250 = radio_cosine_attenuation(6250.0);
        let exp_t0 = (1.0 + cos_100 * 0.4 / 3.0).min(1.0);
        let exp_t1 = (cos_100 * 0.4 / 3.0 + cos_6250 * 1.0 / 6.0).min(1.0);
        let exp_t2 = (1.0 + cos_100 * 0.4 / 3.0 + cos_6250 * 1.0 / 6.0).min(1.0);
        let exp_t3 = (cos_100 * 1.0 / 3.0).min(1.0);
        assert!((out.track_volumes[0] - exp_t0).abs() < 1e-4);
        assert!((out.track_volumes[1] - exp_t1).abs() < 1e-4);
        assert!((out.track_volumes[2] - exp_t2).abs() < 1e-4);
        assert!((out.track_volumes[3] - exp_t3).abs() < 1e-4);
    }

    #[test]
    fn test_radio_modes_1_and_2_and_ducking() {
        // Mode 1: WorldSource at (0, 0, 1500) -> full 0.8 base volume on tracks 0 & 2
        let mut radio = RadioNoiseEvaluator {
            mode: RadioMode::WorldSource {
                position: [0.0, 0.0, 1500.0],
            },
            ..Default::default()
        };
        let out1 = radio.evaluate_enemies(true, Vec3::ZERO, Vec3::ZERO, Mat4::IDENTITY, &[], 0.016);
        assert!((out1.track_volumes[0] - 0.8).abs() < 0.02);
        assert_eq!(out1.track_volumes[1], 0.0);
        assert!((out1.track_volumes[2] - 0.8).abs() < 0.02);

        // Mode 2: ForcedFirstEnemy with downed Lying Figure (0.8) ramps at dt/2.5
        let mut radio2 = RadioNoiseEvaluator {
            mode: RadioMode::ForcedFirstEnemy,
            ..Default::default()
        };
        let downed = [RadioEnemySample {
            kind: 1,
            sub_type: 0,
            position: Vec3::new(0.0, 0.0, 50_000.0),
            is_spawned_active: true,
            is_alive: true,
            is_downed: true,
        }];
        let out2_half =
            radio2.evaluate_enemies(true, Vec3::ZERO, Vec3::ZERO, Mat4::IDENTITY, &downed, 1.0);
        assert!((out2_half.track_volumes[0] - 0.4).abs() < 1e-5);
        assert_eq!(out2_half.driver_pans, [64; 4]);
        let out2_full =
            radio2.evaluate_enemies(true, Vec3::ZERO, Vec3::ZERO, Mat4::IDENTITY, &downed, 2.0);
        assert!((out2_full.track_volumes[0] - 0.8).abs() < 1e-5);

        // Ducking: PauseOrInventory -> 0.5, MapOrMemo -> 0.25
        let mut radio_duck = RadioNoiseEvaluator {
            ducking_state: RadioDuckingState::MapOrMemo,
            ..Default::default()
        };
        let out_duck = radio_duck.evaluate(true, Vec3::ZERO, &[Vec3::ZERO], 3.0);
        assert!((out_duck.global_duck - 0.25).abs() < 1e-5);
        assert!((out_duck.volume - 0.25).abs() < 1e-5);
    }
}
