//! 1:1 Silent Hill 2 Pocket Radio static modulation (`RadioNoise` & `enSetRadioVolume`, `SH2-137`).
//!
//! # Ground Truth (`SLUS_202.28` & `sh2pc.exe`)
//! - **`enSetRadioVolume`** (`SLUS_202.28` `Enemy/en_common.c` `0x00143890`):
//!   Computes each enemy's base radio emission weight (`0.0`, `0.5`, `0.8`, or `1.0`) from its
//!   `enLocalWork.kind` (`0..=14`, `enTransID` `0x00141b30` in `Enemy/en_common.c:452`), `sub_type`,
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
//! - Retail `sh2pc.exe` (`0x0048c710` / `FUN_0048c710` in `sh2pc_00480000.c:7663`) expanded `RadioNoise`
//!   with per-species per-track radius tables, a `1.5` inner deadzone, a threat-score threshold (`>= 9`)
//!   gating track 3, geometric `1/3^k` decay across sorted contributors, and Mode 3 (inventory volume preview).

pub mod tables;
#[cfg(test)]
mod tests;

pub use tables::{
    en_kind_from_species, en_set_radio_volume, radio_cosine_attenuation, RadioDuckingState,
    RadioEnemySample, RadioMode, RadioStaticOutput, ENEMY_RADIO_TRACK_WEIGHTS,
    RADIO_CONTRIB_WEIGHTS, RADIO_MAX_DISTANCE_MM, RADIO_MAX_NEAREST_ENEMIES,
    RADIO_MAX_VOLUME_LEVEL, RADIO_MODE1_BASE_VOLUME, RADIO_PAN_RAMP_DIVISOR, RADIO_TRACK_COUNT,
    RADIO_VOLUME_RAMP_DIVISOR,
};

use glam::{Mat4, Vec3, Vec4};
use serde::{Deserialize, Serialize};

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
                kind: 1,
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
