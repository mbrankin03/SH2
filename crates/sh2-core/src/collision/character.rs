//! Character collision cylinders, dual `move_col`/`hit_col` columns, and
//! `PlayerSetColumn_SetTarget` / `PlayerSetColumn_CloseToTarget` interpolation.
//!
//! Ground truth from `SLUS_202.28` (`Chacter/m3_play.c`: `PlayerSetColumn_SetTarget` `0x00107d40`,
//! `PlayerSetColumn_CloseToTarget` `0x00108010`, `close_to_value` `0x001112e0`) and retail PC
//! `sh2pc.exe` (`0x0052f110`, adding playable Maria `0x10e` and Born From a Wish weapons 9-10):
//!
//! - **Vertical spans (world `+Y` down):**
//!   - `move_col` (BG wall & actor separation): `y_bottom = pos.y - 50.0`, `y_top = pos.y - 850.0` (height `800.0` mm).
//!   - `hit_col` (combat / raycast hitbox): `y_bottom = pos.y`, `y_top = pos.y - 900.0` (height `900.0` mm).
//! - **Runtime radii & forward offset (`+25.0` mm along character yaw):**
//!   - James: `move_col.radius = 175.0` (`0x432f0000`), `hit_col.radius = 150.0` (`0x43160000`).
//!   - Playable Maria (`0x10e`): `move_col.radius = 150.0` (`0x43160000`), `hit_col.radius = 125.00001` (`0x42fa0001`).
//! - **Interpolation:** `close_to_value` steps each radius and forward offset by `5.0` mm/tick (`0x40a00000`).

use glam::Vec3;

/// James base movement collision cylinder radius in mm (`PlayerSetColumn_SetTarget` `0x00107d40`: `0x432f0000`).
pub const JAMES_MOVE_RADIUS: f32 = 175.0;
/// James base combat hitbox cylinder radius in mm (`sh2pc.exe` `0x0052f110`: `0x43160000`; `SLUS_202.28`: `0x4315ffff`).
pub const JAMES_HIT_RADIUS: f32 = 150.0;
/// Playable Maria (`0x10e`) base movement collision cylinder radius in mm (`sh2pc.exe` `0x0052f110`: `0x43160000`).
pub const MARIA_MOVE_RADIUS: f32 = 150.0;
/// Playable Maria (`0x10e`) base combat hitbox cylinder radius in mm (`sh2pc.exe` `0x0052f110`: `0x42fa0001`).
pub const MARIA_HIT_RADIUS: f32 = 125.00001;
/// Base forward center offset for both `move_col` and `hit_col` in mm (`PlayerSetColumn_SetTarget` `0x00107d40`).
pub const PLAYER_BASE_FORWARD_OFFSET: f32 = 25.0;
/// `move_col` bottom Y offset relative to feet (`pos.y - 50.0`, `0x00107d40`).
pub const PLAYER_MOVE_COL_Y_BOTTOM_OFFSET: f32 = -50.0;
/// `move_col` top Y offset relative to feet (`pos.y - 850.0`, `0x00107d40`).
pub const PLAYER_MOVE_COL_Y_TOP_OFFSET: f32 = -850.0;
/// `move_col` vertical height (`800.0` mm, `[-50.0 .. -850.0]`, `0x00107d40`).
pub const PLAYER_MOVE_COL_HEIGHT: f32 = 800.0;
/// `hit_col` bottom Y offset relative to feet (`pos.y + 0.0`, `0x00107d40`).
pub const PLAYER_HIT_COL_Y_BOTTOM_OFFSET: f32 = 0.0;
/// `hit_col` top Y offset relative to feet (`pos.y - 900.0`, `0x00107d40`).
pub const PLAYER_HIT_COL_Y_TOP_OFFSET: f32 = -900.0;
/// `hit_col` vertical height (`900.0` mm, `[0.0 .. -900.0]`, `0x00107d40`).
pub const PLAYER_HIT_COL_HEIGHT: f32 = 900.0;
/// Per-tick linear convergence step for `PlayerSetColumn_CloseToTarget` (`0x00108010`: `0x40a00000` = `5.0` mm).
pub const PLAYER_COLUMN_CLOSE_STEP: f32 = 5.0;

/// Retail PC 10-weapon column scale table (`sh2pc.exe` `0x0052f110` `local_140[80]`, multiplied by `500.0`).
///
/// Indexed by `weapon_id - 1` (`0..10`). Each row holds:
/// `[atk_move_r, atk_move_fwd, atk_hit_r, atk_hit_fwd, aim_move_r, aim_move_fwd, aim_hit_r, aim_hit_fwd]`.
pub const PLAYER_WEAPON_COLUMN_TABLE: [[f32; 8]; 10] = [
    // Weapon 1 (Plank)
    [0.35, 0.05, 0.29999998, 0.05, 0.55, 0.23, 0.45, 0.23],
    // Weapon 2 (Pipe)
    [0.45, 0.05, 0.4, 0.05, 0.55, 0.05, 0.45, 0.05],
    // Weapon 3 (Great Knife)
    [0.65, 0.2, 0.5, 0.2, 0.65, 0.2, 0.5, 0.2],
    // Weapon 4 (Handgun)
    [0.4, 0.15, 0.35, 0.15, 0.45, 0.17, 0.4, 0.17],
    // Weapon 5 (Shotgun)
    [0.4, 0.2, 0.35, 0.2, 0.45, 0.2, 0.4, 0.2],
    // Weapon 6 (Rifle)
    [0.4, 0.1, 0.35, 0.1, 0.5, 0.1, 0.4, 0.1],
    // Weapon 7 (Hyper Spray)
    [0.45, 0.12, 0.4, 0.12, 0.45, 0.12, 0.4, 0.12],
    // Weapon 8 (Chainsaw)
    [0.45, 0.18, 0.4, 0.18, 0.5, 0.18, 0.4, 0.18],
    // Weapon 9 (Maria Cleaver — retail PC)
    [0.3, 0.05, 0.25, 0.05, 0.5, 0.23, 0.4, 0.23],
    // Weapon 10 (Maria Revolver — retail PC)
    [0.35000002, 0.1, 0.3, 0.1, 0.4, 0.1, 0.35000002, 0.1],
];

/// Exact 1:1 PS2 `close_to_value` (`SLUS_202.28` `0x001112e0`): moves `current` toward `target`
/// by at most `step` without overshooting.
#[inline]
pub fn close_to_value(target: f32, step: f32, current: &mut f32) {
    if *current != target {
        if target < *current {
            *current -= step;
            if *current < target {
                *current = target;
            }
        } else {
            *current += step;
            if target < *current {
                *current = target;
            }
        }
    }
}

/// Handle/index identifying a character cylinder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CharacterId(pub u32);

/// Authentic vertical collision cylinder matching `_CL_HITPOLY_COLUMN` (`cl_main.c`).
///
/// World coordinates use `+Y` pointing downward (`y_top <= y_bottom`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CollisionColumn {
    /// Center X in world mm (`+0x10`).
    pub center_x: f32,
    /// Bottom Y in world mm (`+0x14`, numerically larger in `+Y`-down).
    pub y_bottom: f32,
    /// Center Z in world mm (`+0x18`).
    pub center_z: f32,
    /// Top Y in world mm (`+0x24`, numerically smaller in `+Y`-down).
    pub y_top: f32,
    /// Cylinder radius in world mm (`+0x2c`).
    pub radius: f32,
    /// Discrete PS2 collision push weight (`_CL_HITPOLY_COLUMN.weight` at `+0x04`).
    pub weight: i32,
    /// Active flag (`radius > 0.0` in `clSetCharaHitColumn` `0x001325e0`).
    pub active: bool,
}

impl CollisionColumn {
    /// Constructs a new active collision column.
    pub const fn new(
        center_x: f32,
        y_bottom: f32,
        center_z: f32,
        y_top: f32,
        radius: f32,
        weight: i32,
    ) -> Self {
        Self {
            center_x,
            y_bottom,
            center_z,
            y_top,
            radius,
            weight,
            active: radius > 0.0,
        }
    }
}

/// Player action category used by `PlayerSetColumn_SetTarget` (`0x00107d40` / `sh2pc.exe` `0x0052f110`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlayerColumnAction {
    /// Normal locomotion / idle (base radius and `+25.0` forward offset).
    #[default]
    Normal,
    /// Ready / Guard stance (`DAT_003c7f50 == 0x19`).
    ReadyGuard {
        /// Guard active flag (`DAT_003c8410 != 0`).
        active_flag: bool,
        /// Retail PC zero-radius override when `anim_id == 0x36 || 0x3d`.
        zero_radius_anim: bool,
    },
    /// Weapon attack (`DAT_003c7f50 == 0x1a`).
    Attack {
        /// 1-based weapon ID (`1..=10`).
        weapon_id: u8,
        /// Sub-action flag (`DAT_003c840f`).
        sub_flag: u8,
    },
    /// Weapon aim (`DAT_003c7f50 == 0x1c`).
    Aim {
        /// 1-based weapon ID (`1..=10`).
        weapon_id: u8,
    },
}

/// Target radii and forward offsets produced by `PlayerSetColumn_SetTarget` (`0x00107d40`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlayerColumnTarget {
    /// Target `move_col` cylinder (`y_bottom = pos.y - 50`, `y_top = pos.y - 850`).
    pub move_col: CollisionColumn,
    /// Target `hit_col` cylinder (`y_bottom = pos.y`, `y_top = pos.y - 900`).
    pub hit_col: CollisionColumn,
    /// Target forward offset for `move_col` along player yaw (mm).
    pub move_fwd_offset: f32,
    /// Target forward offset for `hit_col` along player yaw (mm).
    pub hit_fwd_offset: f32,
}

/// Persistent dual-cylinder player collision state matching `PlayerSetColumn_SetTarget`
/// (`0x00107d40`) and `PlayerSetColumn_CloseToTarget` (`0x00108010`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlayerColumnState {
    /// `true` for playable Maria (`chara_id == 0x10e`), `false` for James.
    pub is_maria: bool,
    /// Current interpolated `move_col` cylinder (`0x003c81a0`).
    pub move_col: CollisionColumn,
    /// Current interpolated `hit_col` cylinder (`0x003c81d0`).
    pub hit_col: CollisionColumn,
    /// Current interpolated forward offset for `move_col` (`0x003c8200`).
    pub move_fwd_offset: f32,
    /// Current interpolated forward offset for `hit_col` (`0x003c8204`).
    pub hit_fwd_offset: f32,
}

impl PlayerColumnState {
    /// Initializes player dual cylinders for James (`is_maria = false`) or playable Maria (`is_maria = true`).
    pub fn new(pos: Vec3, is_maria: bool) -> Self {
        let (move_r, hit_r) = if is_maria {
            (MARIA_MOVE_RADIUS, MARIA_HIT_RADIUS)
        } else {
            (JAMES_MOVE_RADIUS, JAMES_HIT_RADIUS)
        };
        Self {
            is_maria,
            move_col: CollisionColumn::new(
                pos.x,
                pos.y + PLAYER_MOVE_COL_Y_BOTTOM_OFFSET,
                pos.z,
                pos.y + PLAYER_MOVE_COL_Y_TOP_OFFSET,
                move_r,
                1,
            ),
            hit_col: CollisionColumn::new(
                pos.x,
                pos.y + PLAYER_HIT_COL_Y_BOTTOM_OFFSET,
                pos.z,
                pos.y + PLAYER_HIT_COL_Y_TOP_OFFSET,
                hit_r,
                1,
            ),
            move_fwd_offset: PLAYER_BASE_FORWARD_OFFSET,
            hit_fwd_offset: PLAYER_BASE_FORWARD_OFFSET,
        }
    }

    /// Exact 1:1 `PlayerSetColumn_SetTarget` (`SLUS_202.28` `0x00107d40` / `sh2pc.exe` `0x0052f110`).
    pub fn compute_target(
        &self,
        pos: Vec3,
        action: PlayerColumnAction,
        override_to_base: bool,
    ) -> PlayerColumnTarget {
        let (base_move_r, base_hit_r) = if self.is_maria {
            (MARIA_MOVE_RADIUS, MARIA_HIT_RADIUS)
        } else {
            (JAMES_MOVE_RADIUS, JAMES_HIT_RADIUS)
        };

        let mut move_r = base_move_r;
        let mut hit_r = base_hit_r;
        let mut move_fwd = PLAYER_BASE_FORWARD_OFFSET;
        let mut hit_fwd = PLAYER_BASE_FORWARD_OFFSET;

        match action {
            PlayerColumnAction::Normal => {}
            PlayerColumnAction::ReadyGuard {
                active_flag,
                zero_radius_anim,
            } => {
                if active_flag {
                    move_r = if zero_radius_anim { 0.0 } else { 375.0 };
                }
            }
            PlayerColumnAction::Attack {
                weapon_id,
                sub_flag,
            } => {
                if let Some(row) = weapon_id
                    .checked_sub(1)
                    .and_then(|idx| PLAYER_WEAPON_COLUMN_TABLE.get(idx as usize))
                {
                    move_r = row[0] * 500.0;
                    move_fwd = row[1] * 500.0;
                    hit_r = row[2] * 500.0;
                    hit_fwd = row[3] * 500.0;
                }
                if (weapon_id == 1 || weapon_id == 9) && sub_flag == 1 {
                    move_fwd = 115.0;
                    move_r = 275.0;
                }
                if weapon_id == 8 {
                    if sub_flag == 0 {
                        move_fwd = 50.0;
                        hit_fwd = 0.0;
                        move_r = 265.0;
                    } else {
                        move_fwd = -50.0;
                        hit_fwd = -75.0;
                        move_r = 265.0;
                    }
                }
            }
            PlayerColumnAction::Aim { weapon_id } => {
                if let Some(row) = weapon_id
                    .checked_sub(1)
                    .and_then(|idx| PLAYER_WEAPON_COLUMN_TABLE.get(idx as usize))
                {
                    move_r = row[4] * 500.0;
                    move_fwd = row[5] * 500.0;
                    hit_r = row[6] * 500.0;
                    hit_fwd = row[7] * 500.0;
                }
            }
        }

        if override_to_base {
            move_r = base_move_r;
            hit_r = base_hit_r;
        }

        PlayerColumnTarget {
            move_col: CollisionColumn::new(
                pos.x,
                pos.y + PLAYER_MOVE_COL_Y_BOTTOM_OFFSET,
                pos.z,
                pos.y + PLAYER_MOVE_COL_Y_TOP_OFFSET,
                move_r,
                self.move_col.weight,
            ),
            hit_col: CollisionColumn::new(
                pos.x,
                pos.y + PLAYER_HIT_COL_Y_BOTTOM_OFFSET,
                pos.z,
                pos.y + PLAYER_HIT_COL_Y_TOP_OFFSET,
                hit_r,
                self.hit_col.weight,
            ),
            move_fwd_offset: move_fwd,
            hit_fwd_offset: hit_fwd,
        }
    }

    /// Exact 1:1 `PlayerSetColumn_CloseToTarget` (`SLUS_202.28` `0x00108010`):
    /// updates `move_col` and `hit_col` positions, converges radii and forward offsets by
    /// `5.0` mm/tick (`close_to_value`), and applies forward offset `(sin(yaw), cos(yaw))`.
    pub fn close_to_target(&mut self, pos: Vec3, yaw: f32, target: &PlayerColumnTarget) {
        close_to_value(
            target.move_col.radius,
            PLAYER_COLUMN_CLOSE_STEP,
            &mut self.move_col.radius,
        );
        close_to_value(
            target.hit_col.radius,
            PLAYER_COLUMN_CLOSE_STEP,
            &mut self.hit_col.radius,
        );
        close_to_value(
            target.move_fwd_offset,
            PLAYER_COLUMN_CLOSE_STEP,
            &mut self.move_fwd_offset,
        );
        close_to_value(
            target.hit_fwd_offset,
            PLAYER_COLUMN_CLOSE_STEP,
            &mut self.hit_fwd_offset,
        );

        let sin_y = yaw.sin();
        let cos_y = yaw.cos();

        self.move_col.center_x = pos.x + self.move_fwd_offset * sin_y;
        self.move_col.y_bottom = pos.y + PLAYER_MOVE_COL_Y_BOTTOM_OFFSET;
        self.move_col.center_z = pos.z + self.move_fwd_offset * cos_y;
        self.move_col.y_top = pos.y + PLAYER_MOVE_COL_Y_TOP_OFFSET;
        self.move_col.active = self.move_col.radius > 0.0;

        self.hit_col.center_x = pos.x + self.hit_fwd_offset * sin_y;
        self.hit_col.y_bottom = pos.y + PLAYER_HIT_COL_Y_BOTTOM_OFFSET;
        self.hit_col.center_z = pos.z + self.hit_fwd_offset * cos_y;
        self.hit_col.y_top = pos.y + PLAYER_HIT_COL_Y_TOP_OFFSET;
        self.hit_col.active = self.hit_col.radius > 0.0;
    }

    /// Convenience helper combining `PlayerSetColumn_SetTarget` + `PlayerSetColumn_CloseToTarget`.
    pub fn step(
        &mut self,
        pos: Vec3,
        yaw: f32,
        action: PlayerColumnAction,
        override_to_base: bool,
    ) {
        let target = self.compute_target(pos, action, override_to_base);
        self.close_to_target(pos, yaw, &target);
    }
}

/// Character collision cylinder representation matching PS2 `clCharaList` (`clSetCharaHitColumn`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CharacterCylinder {
    pub id: CharacterId,
    /// Character feet position `(x, y, z)` in world mm (`+Y` down).
    pub position: Vec3,
    /// Movement collision radius in world mm (James `175.0`, Maria `150.0`).
    pub radius: f32,
    /// Movement collision height in world mm (James `800.0`, `[-50..-850]`).
    pub height: f32,
    /// Vertical bottom offset relative to `position.y` (`-50.0` for James/Maria `move_col`).
    pub y_bottom_offset: f32,
    /// Combat hitbox radius in world mm (`hit_col`, James `150.0`, Maria `125.00001`).
    pub hit_radius: f32,
    /// Combat hitbox height in world mm (`hit_col`, James/Maria `900.0`, `[0..-900]`).
    pub hit_height: f32,
    /// Legacy floating-point mass field (preserved for callers; mapped to discrete `weight`).
    pub mass: f32,
    /// Discrete PS2 collision push weight (`_CL_HITPOLY_COLUMN.weight` at `+0x04`).
    /// Used with `clCollectCharaALL` weight-diff table `[1.0, 0.98, 0.94, 0.90, 0.86]`.
    pub weight: i32,
    pub active: bool,
}

impl CharacterCylinder {
    /// Constructs a new character cylinder with authentic default `y_bottom_offset = -50.0`,
    /// `hit_radius = radius`, and discrete `weight` derived from `mass`.
    pub const fn new(id: CharacterId, position: Vec3, radius: f32, height: f32, mass: f32) -> Self {
        let weight = if mass <= 1.0 {
            1
        } else if mass <= 2.0 {
            2
        } else if mass <= 5.0 {
            3
        } else {
            4
        };
        Self {
            id,
            position,
            radius,
            height,
            y_bottom_offset: PLAYER_MOVE_COL_Y_BOTTOM_OFFSET,
            hit_radius: radius,
            hit_height: height,
            mass,
            weight,
            active: true,
        }
    }

    /// Constructs James's authentic dual-cylinder registration (`175mm / 800mm` move, `150mm / 900mm` hit).
    pub const fn james(id: CharacterId, position: Vec3) -> Self {
        Self {
            id,
            position,
            radius: JAMES_MOVE_RADIUS,
            height: PLAYER_MOVE_COL_HEIGHT,
            y_bottom_offset: PLAYER_MOVE_COL_Y_BOTTOM_OFFSET,
            hit_radius: JAMES_HIT_RADIUS,
            hit_height: PLAYER_HIT_COL_HEIGHT,
            mass: 1.0,
            weight: 1,
            active: true,
        }
    }

    /// Constructs playable Maria's authentic dual-cylinder registration (`150mm / 800mm` move, `125mm / 900mm` hit).
    pub const fn maria(id: CharacterId, position: Vec3) -> Self {
        Self {
            id,
            position,
            radius: MARIA_MOVE_RADIUS,
            height: PLAYER_MOVE_COL_HEIGHT,
            y_bottom_offset: PLAYER_MOVE_COL_Y_BOTTOM_OFFSET,
            hit_radius: MARIA_HIT_RADIUS,
            hit_height: PLAYER_HIT_COL_HEIGHT,
            mass: 1.0,
            weight: 1,
            active: true,
        }
    }

    /// Returns the active movement [`CollisionColumn`] (`move_col`).
    #[inline]
    pub fn move_column(&self) -> CollisionColumn {
        let y_bottom = self.position.y + self.y_bottom_offset;
        let y_top = y_bottom - self.height;
        CollisionColumn {
            center_x: self.position.x,
            y_bottom,
            center_z: self.position.z,
            y_top,
            radius: self.radius,
            weight: self.weight,
            active: self.active && self.radius > 0.0,
        }
    }

    /// Returns the active combat hitbox [`CollisionColumn`] (`hit_col`).
    #[inline]
    pub fn hit_column(&self) -> CollisionColumn {
        CollisionColumn {
            center_x: self.position.x,
            y_bottom: self.position.y,
            center_z: self.position.z,
            y_top: self.position.y - self.hit_height,
            radius: self.hit_radius,
            weight: self.weight,
            active: self.active && self.hit_radius > 0.0,
        }
    }

    /// Check if two cylinders overlap along the vertical axis (world `+Y` pointing downward).
    #[inline]
    pub fn overlaps_vertical(&self, other: &Self) -> bool {
        let a = self.move_column();
        let b = other.move_column();
        a.y_bottom >= b.y_top && b.y_bottom >= a.y_top
    }

    /// Resolve mutual 2D circle-circle separation using `clCheckSubColumnToColumn` (`0x00131f90`).
    ///
    /// Returns `(delta_a, delta_b)` movement adjustments.
    pub fn resolve_separation(&self, other: &Self) -> (Vec3, Vec3) {
        if !self.active || !other.active || !self.overlaps_vertical(other) {
            return (Vec3::ZERO, Vec3::ZERO);
        }

        let dx = other.position.x - self.position.x;
        let dz = other.position.z - self.position.z;
        let dist_sq = dx * dx + dz * dz;
        let min_dist = self.radius + other.radius;

        if dist_sq >= min_dist * min_dist {
            return (Vec3::ZERO, Vec3::ZERO);
        }
        if dist_sq < 1e-6 {
            let half_pen = min_dist * 0.5;
            return (
                Vec3::new(-half_pen, 0.0, 0.0),
                Vec3::new(half_pen, 0.0, 0.0),
            );
        }

        let dist = dist_sq.sqrt();
        let penetration = (min_dist - dist) + 1.0;
        let nx = dx / dist;
        let nz = dz / dist;

        let total_mass = self.mass + other.mass;
        let weight_a = if total_mass > 0.0 {
            other.mass / total_mass
        } else {
            0.5
        };
        let weight_b = if total_mass > 0.0 {
            self.mass / total_mass
        } else {
            0.5
        };

        let delta_a = Vec3::new(
            -nx * penetration * weight_a,
            0.0,
            -nz * penetration * weight_a,
        );
        let delta_b = Vec3::new(
            nx * penetration * weight_b,
            0.0,
            nz * penetration * weight_b,
        );

        (delta_a, delta_b)
    }
}
