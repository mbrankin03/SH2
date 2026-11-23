//! Room forbidden areas (`enRoomForbiddenArea` `SLUS 0x00147000` / `sh2pc 0x004a84a0`)
//! and inflated AABB ray clipping (`enCheckForbiddenAreaSub` `SLUS 0x00147420`).

use glam::Vec3;
use serde::{Deserialize, Serialize};

/// Axis-aligned XZ rectangle in PS2 world mm (`enRoomForbiddenArea` `SLUS 0x00147000` / `sh2pc 0x004a84a0`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ForbiddenRect {
    pub min_x: f32,
    pub min_z: f32,
    pub max_x: f32,
    pub max_z: f32,
}

impl ForbiddenRect {
    /// Create a forbidden XZ rectangle.
    pub const fn new(min_x: f32, min_z: f32, max_x: f32, max_z: f32) -> Self {
        Self {
            min_x,
            min_z,
            max_x,
            max_z,
        }
    }

    /// Returns `true` if `pos.(x, z)` lies inside this rectangle (`enExecTask` `SLUS 0x001413f0`).
    pub fn contains_xz(&self, pos: Vec3) -> bool {
        pos.x >= self.min_x && pos.x <= self.max_x && pos.z >= self.min_z && pos.z <= self.max_z
    }
}

/// Room-level AI flags computed by `enRoomForbiddenArea` (`_DAT_0055244c` in `SLUS 0x00147000` / `_DAT_01f7cdec` in `sh2pc 0x004a84a0`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ForbiddenAreaFlags {
    /// Bit 0 (`0x01`): player is currently standing inside an active forbidden area.
    pub player_in_forbidden: bool,
    /// Bit 1 (`0x02`): special boss/encounter rooms (`0x30, 0x65, 0xbb`, plus `0xb8` on retail PC).
    pub special_room: bool,
    /// Bit 2 (`0x04`): tight-corridor down-radius flag (`sh2pc 0x004a84a0`: `150.0` vs `225.0` mm in `enSCUCtrlDown`).
    pub narrow_down_radius: bool,
    /// Bit 4 (`0x10`): room `0x27` flag (`sh2pc 0x004a84a0`).
    pub room_0x27_flag: bool,
}

/// Active forbidden rectangles and flags for a `(stage_id, room_id)` pair (`SLUS 0x00147000` / `sh2pc 0x004a84a0`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RoomForbiddenAreas {
    pub rects: Vec<ForbiddenRect>,
    pub flags: ForbiddenAreaFlags,
}

/// Resolve the active forbidden areas and room flags for `(stage_id, room_id)`.
///
/// - When `retail_pc` is `true`, uses the final retail `sh2pc.exe` (`0x004a84a0`) table.
/// - When `retail_pc` is `false`, uses the exact `SLUS_202.28` (`0x00147000`) prototype table.
pub fn en_room_forbidden_area(stage_id: u32, room_id: u32, retail_pc: bool) -> RoomForbiddenAreas {
    let mut out = RoomForbiddenAreas::default();

    if stage_id == 3 {
        if retail_pc {
            out.rects
                .push(ForbiddenRect::new(-20_500.0, 86_000.0, -15_000.0, 92_500.0));
        }
        out.rects
            .push(ForbiddenRect::new(-20_000.0, 40_000.0, -4_000.0, 57_000.0));
        out.rects.push(ForbiddenRect::new(
            -100_000.0, 60_000.0, -80_000.0, 100_000.0,
        ));
    } else if retail_pc && stage_id == 0x10 {
        out.flags.narrow_down_radius = true;
        match room_id {
            0xc8 => out.rects.push(ForbiddenRect::new(
                -63_500.0, 103_300.0, -61_500.0, 104_700.0,
            )),
            0xcc => out
                .rects
                .push(ForbiddenRect::new(58_000.0, -59_800.0, 58_800.0, -56_000.0)),
            _ => {}
        }
    } else {
        match room_id {
            0x07 => {
                if retail_pc {
                    out.rects
                        .push(ForbiddenRect::new(82_000.0, -71_300.0, 85_000.0, -68_000.0));
                } else {
                    out.rects
                        .push(ForbiddenRect::new(83_500.0, -71_300.0, 85_000.0, -69_600.0));
                }
            }
            0x15 if retail_pc => {
                out.rects.push(ForbiddenRect::new(
                    -103_400.0, 8_700.0, -102_200.0, 10_800.0,
                ));
            }
            0x1f if retail_pc => {
                out.rects
                    .push(ForbiddenRect::new(55_500.0, 8_000.0, 57_200.0, 12_000.0));
            }
            0x83 if retail_pc => {
                out.rects.push(ForbiddenRect::new(
                    56_400.0, -101_000.0, 57_400.0, -99_000.0,
                ));
                out.rects.push(ForbiddenRect::new(
                    62_600.0, -101_000.0, 63_600.0, -99_000.0,
                ));
            }
            0x84 if retail_pc => {
                out.rects
                    .push(ForbiddenRect::new(60_000.0, 99_000.0, 65_000.0, 100_400.0));
            }
            0x92 => {
                out.rects
                    .push(ForbiddenRect::new(-20_800.0, 18_400.0, -19_200.0, 22_800.0));
            }
            0x99 => {
                out.rects
                    .push(ForbiddenRect::new(-64_000.0, 20_800.0, -63_200.0, 23_500.0));
                out.rects
                    .push(ForbiddenRect::new(-60_750.0, 20_750.0, -59_250.0, 25_600.0));
            }
            0xa0 | 0xb2 => {
                out.rects
                    .push(ForbiddenRect::new(60_000.0, -19_400.0, 60_750.0, -16_800.0));
                out.rects
                    .push(ForbiddenRect::new(61_200.0, -19_400.0, 62_000.0, -16_800.0));
            }
            0xa3 | 0xb5 => {
                out.rects
                    .push(ForbiddenRect::new(16_000.0, 20_000.0, 16_800.0, 22_600.0));
            }
            0xa8 => {
                out.rects.push(ForbiddenRect::new(
                    -20_800.0, -60_000.0, -19_200.0, -55_600.0,
                ));
                if retail_pc {
                    out.rects.push(ForbiddenRect::new(
                        -16_000.0, -59_200.0, -14_700.0, -58_000.0,
                    ));
                }
            }
            0x30 | 0x65 | 0xbb => {
                out.flags.special_room = true;
            }
            0xb8 if retail_pc => {
                out.flags.special_room = true;
            }
            _ => {}
        }

        if retail_pc {
            if matches!(
                room_id,
                0x17 | 0x18 | 0x1a | 0x39 | 0x3a | 0x4b | 0x4c | 0x51 | 0x68
            ) {
                out.flags.narrow_down_radius = true;
            }
            if room_id == 0x27 {
                out.flags.room_0x27_flag = true;
            }
        }
    }

    out
}

/// Clip segment `from -> to` against an inflated forbidden rectangle (`enCheckForbiddenAreaSub` `SLUS 0x00147420`).
///
/// Inflates `rect` by `radius` on all 4 sides and tests each face independently only when `from`
/// approaches from outside (within `2.0 * radius` entry band or outside the face), matching `SLUS 0x00147420`
/// so an enemy already deep inside the inflated rect is not frozen at `t = 0`.
pub fn en_check_forbidden_area_sub(
    radius: f32,
    rect: ForbiddenRect,
    from: Vec3,
    to: &mut Vec3,
) -> bool {
    let min_x = rect.min_x - radius;
    let max_x = rect.max_x + radius;
    let min_z = rect.min_z - radius;
    let max_z = rect.max_z + radius;

    if (from.x < min_x && to.x < min_x)
        || (from.x > max_x && to.x > max_x)
        || (from.z < min_z && to.z < min_z)
        || (from.z > max_z && to.z > max_z)
    {
        return false;
    }

    let dx = to.x - from.x;
    let dz = to.z - from.z;
    let mut best_t: Option<f32> = None;
    let band = 2.0 * radius;

    // West face (`x = min_x`, entering from left)
    if dx > 1e-6 && from.x <= min_x + band && to.x >= min_x {
        let t = (min_x - from.x) / dx;
        if (0.0..=1.0).contains(&t) {
            let iz = from.z + dz * t;
            if iz >= min_z && iz <= max_z {
                best_t = Some(best_t.map_or(t, |b| b.min(t)));
            }
        }
    }
    // East face (`x = max_x`, entering from right)
    if dx < -1e-6 && from.x >= max_x - band && to.x <= max_x {
        let t = (max_x - from.x) / dx;
        if (0.0..=1.0).contains(&t) {
            let iz = from.z + dz * t;
            if iz >= min_z && iz <= max_z {
                best_t = Some(best_t.map_or(t, |b| b.min(t)));
            }
        }
    }
    // South face (`z = min_z`, entering from -Z)
    if dz > 1e-6 && from.z <= min_z + band && to.z >= min_z {
        let t = (min_z - from.z) / dz;
        if (0.0..=1.0).contains(&t) {
            let ix = from.x + dx * t;
            if ix >= min_x && ix <= max_x {
                best_t = Some(best_t.map_or(t, |b| b.min(t)));
            }
        }
    }
    // North face (`z = max_z`, entering from +Z)
    if dz < -1e-6 && from.z >= max_z - band && to.z <= max_z {
        let t = (max_z - from.z) / dz;
        if (0.0..=1.0).contains(&t) {
            let ix = from.x + dx * t;
            if ix >= min_x && ix <= max_x {
                best_t = Some(best_t.map_or(t, |b| b.min(t)));
            }
        }
    }

    if let Some(t) = best_t {
        to.x = from.x + dx * t;
        to.z = from.z + dz * t;
        true
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_en_room_forbidden_area_slus_vs_pc_and_inside_escape() {
        let slus_stage3 = en_room_forbidden_area(3, 0, false);
        let pc_stage3 = en_room_forbidden_area(3, 0, true);
        assert_eq!(slus_stage3.rects.len(), 2);
        assert_eq!(pc_stage3.rects.len(), 3);

        // Clip segment entering room 0x07 forbidden rect from outside.
        let r = en_room_forbidden_area(1, 0x07, false).rects[0];
        let from = Vec3::new(82_000.0, 0.0, -70_500.0);
        let mut to = Vec3::new(84_000.0, 0.0, -70_500.0);
        assert!(en_check_forbidden_area_sub(150.0, r, from, &mut to));
        assert!((to.x - (83_500.0 - 150.0)).abs() < 1e-3);

        // Enemy already inside rect moving outward is NOT frozen (`SLUS 0x00147420`).
        let inside_from = Vec3::new(84_200.0, 0.0, -70_500.0);
        let mut outside_to = Vec3::new(86_000.0, 0.0, -70_500.0);
        assert!(!en_check_forbidden_area_sub(
            150.0,
            r,
            inside_from,
            &mut outside_to
        ));
    }
}
