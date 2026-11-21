//! Spatial trigger shape decoding (`EvPosShape`), area containment (`EventCheckIn`),
//! and destination placement (`EventPositionSet`, `EventResultMovePosition`).
//!
//! Verified 1:1 against `SLUS_202.28` (`src/Event/event.c`: `EventCheckLook` `0x001a8780`,
//! `EventCheckIn` `0x001a8d70`, `EventPositionSet` `0x001a9570`, `EventResultMovePosition`
//! `0x001a9730`; `shOuterXZ` `0x001b0d50`) and retail PC `sh2pc.exe` (`0x0048d4d0`,
//! `0x0048da90`, `0x0048b610`, `0x0048b780`).

use std::f32::consts::{FRAC_PI_2, PI};

use crate::types::{
    char_to_float2, char_to_float4, wrap_angle, EventDestShapeKind, EventShapeKind,
};

use super::EVENT_IGNORE_HEIGHT_Y;

/// Decoded spatial trigger shape from a stage's raw `ev_pos` byte array.
///
/// Matches the 10-byte header `[f32 x0, f16 y, f32 z0]` in `SLUS_202.28` (`0x001a8780`,
/// `0x001a8d70`) and `sh2pc.exe` (`0x0048d4d0`, `0x0048da90`). There is no per-shape radius
/// field in `ev_pos`; look checks use the fixed `400.0` mm radius (`0x001a8b00`).
#[derive(Debug, Clone, PartialEq)]
pub enum EvPosShape {
    /// Shape `0`: Point `(x, y, z)` in PS2 world millimeters.
    Point { x: f32, y: f32, z: f32 },
    /// Shapes `1..=10`: One or more 2D line segments at height `y` in PS2 world millimeters.
    Polyline { y: f32, vertices: Vec<[f32; 2]> },
    /// Shape `11`: Axis-aligned rectangle `(x..=x+w, z..=z+d)` at height `y`.
    Rectangle {
        x: f32,
        y: f32,
        z: f32,
        w: f32,
        d: f32,
    },
    /// Shape `12`: 2D triangle at height `y`.
    Triangle { y: f32, vertices: [[f32; 2]; 3] },
    /// Shape `13`: Convex 2D quad at height `y`.
    Quad { y: f32, vertices: [[f32; 2]; 4] },
}

/// Reads a 2-byte Team Silent half-float (`CharToFloat2`, `SLUS_202.28` `0x001a9280`).
#[inline]
pub(crate) fn read_f16(ev_pos: &[u8], byte_offset: usize) -> Option<f32> {
    let b0 = *ev_pos.get(byte_offset)?;
    let b1 = *ev_pos.get(byte_offset + 1)?;
    Some(char_to_float2([b0, b1]))
}

/// Reads an unaligned 4-byte IEEE-754 float (`CharToFloat4`, `SLUS_202.28` `0x001a92e0`).
#[inline]
pub(crate) fn read_f32(ev_pos: &[u8], byte_offset: usize) -> Option<f32> {
    let slice = ev_pos.get(byte_offset..byte_offset.checked_add(4)?)?;
    Some(char_to_float4([slice[0], slice[1], slice[2], slice[3]]))
}

/// Decodes an [`EvPosShape`] from `ev_pos` starting at `offset`.
///
/// Exact 1:1 parity with `EventCheckLook` (`SLUS_202.28` `0x001a8780`, `sh2pc.exe` `0x0048d4d0`)
/// and `EventCheckIn` (`SLUS_202.28` `0x001a8d70`, `sh2pc.exe` `0x0048da90`):
/// - Header (`10` bytes): `[0..4] f32 x0`, `[4..6] f16 y`, `[6..10] f32 z0`.
/// - Shapes `1..=4` (`12` bytes): `f16 len` at `+10` along `1=+X, 2=-X, 3=-Z, 4=+Z`.
/// - Shapes `5..=10` (`10 + (shape - 4) * 4` bytes): relative `(f16 dx, f16 dz)` deltas added to `(x0, z0)`.
/// - Shape `11` (`14` bytes): `f16 w` at `+10`, `f16 d` at `+12`.
/// - Shape `12` (`18` bytes) & Shape `13` (`22` bytes): relative `(f16 dx, f16 dz)` deltas added to `(x0, z0)`.
pub fn decode_ev_pos_shape(
    ev_pos: &[u8],
    offset: usize,
    shape: EventShapeKind,
) -> Option<EvPosShape> {
    if offset.checked_add(shape.byte_len())? > ev_pos.len() {
        return None;
    }
    let x0 = read_f32(ev_pos, offset)?;
    let y = read_f16(ev_pos, offset + 4)?;
    let z0 = read_f32(ev_pos, offset + 6)?;

    match shape {
        EventShapeKind::Point => Some(EvPosShape::Point { x: x0, y, z: z0 }),
        EventShapeKind::LinePosX => {
            let len = read_f16(ev_pos, offset + 10)?;
            Some(EvPosShape::Polyline {
                y,
                vertices: vec![[x0, z0], [x0 + len, z0]],
            })
        }
        EventShapeKind::LineNegX => {
            let len = read_f16(ev_pos, offset + 10)?;
            Some(EvPosShape::Polyline {
                y,
                vertices: vec![[x0, z0], [x0 - len, z0]],
            })
        }
        EventShapeKind::LineNegZ => {
            let len = read_f16(ev_pos, offset + 10)?;
            Some(EvPosShape::Polyline {
                y,
                vertices: vec![[x0, z0], [x0, z0 - len]],
            })
        }
        EventShapeKind::LinePosZ => {
            let len = read_f16(ev_pos, offset + 10)?;
            Some(EvPosShape::Polyline {
                y,
                vertices: vec![[x0, z0], [x0, z0 + len]],
            })
        }
        EventShapeKind::Polyline2
        | EventShapeKind::Polyline3
        | EventShapeKind::Polyline4
        | EventShapeKind::Polyline5
        | EventShapeKind::Polyline6
        | EventShapeKind::Polyline7 => {
            let extra_vertices = (shape as usize) - 4;
            let mut vertices = Vec::with_capacity(extra_vertices + 1);
            vertices.push([x0, z0]);
            for i in 0..extra_vertices {
                let dx = read_f16(ev_pos, offset + 10 + i * 4)?;
                let dz = read_f16(ev_pos, offset + 12 + i * 4)?;
                vertices.push([x0 + dx, z0 + dz]);
            }
            Some(EvPosShape::Polyline { y, vertices })
        }
        EventShapeKind::Rectangle => {
            let w = read_f16(ev_pos, offset + 10)?;
            let d = read_f16(ev_pos, offset + 12)?;
            Some(EvPosShape::Rectangle {
                x: x0,
                y,
                z: z0,
                w,
                d,
            })
        }
        EventShapeKind::Triangle => {
            let dx1 = read_f16(ev_pos, offset + 10)?;
            let dz1 = read_f16(ev_pos, offset + 12)?;
            let dx2 = read_f16(ev_pos, offset + 14)?;
            let dz2 = read_f16(ev_pos, offset + 16)?;
            Some(EvPosShape::Triangle {
                y,
                vertices: [[x0, z0], [x0 + dx1, z0 + dz1], [x0 + dx2, z0 + dz2]],
            })
        }
        EventShapeKind::Quad => {
            let dx1 = read_f16(ev_pos, offset + 10)?;
            let dz1 = read_f16(ev_pos, offset + 12)?;
            let dx2 = read_f16(ev_pos, offset + 14)?;
            let dz2 = read_f16(ev_pos, offset + 16)?;
            let dx3 = read_f16(ev_pos, offset + 18)?;
            let dz3 = read_f16(ev_pos, offset + 20)?;
            Some(EvPosShape::Quad {
                y,
                vertices: [
                    [x0, z0],
                    [x0 + dx1, z0 + dz1],
                    [x0 + dx2, z0 + dz2],
                    [x0 + dx3, z0 + dz3],
                ],
            })
        }
    }
}

/// Checks whether the event's `y` coordinate is within the vertical window relative to `player_y`
/// (`y >= 45000.0 || (player_y - 500.0 <= y && y <= player_y + 100.0)`).
///
/// Matches `EventCheckLook` (`SLUS_202.28` `0x001a8780`, `sh2pc.exe` `0x0048d4d0`) and
/// `EventCheckIn` (`SLUS_202.28` `0x001a8d70`, `sh2pc.exe` `0x0048da90`).
#[inline]
pub fn player_height_matches_event_y(player_y: f32, event_y: f32) -> bool {
    event_y >= EVENT_IGNORE_HEIGHT_Y || (player_y - 500.0 <= event_y && event_y <= player_y + 100.0)
}

/// 2D XZ outer product (`shOuterXZ` in `SLUS_202.28` `Event/vu0_func.c` `0x001b0d50`,
/// inlined in `sh2pc.exe` `EventCheckIn` `0x0048da90`):
/// `(px - ax) * (bz - az) - (pz - az) * (bx - ax)`.
#[inline]
fn sh_outer_xz(px: f32, pz: f32, ax: f32, az: f32, bx: f32, bz: f32) -> f32 {
    (px - ax) * (bz - az) - (pz - az) * (bx - ax)
}

/// Exact `EventCheckIn` convex polygon containment (`SLUS_202.28` `0x001a8d70` lines 885-900,
/// `sh2pc.exe` `0x0048da90` lines 8502-8514): returns `true` iff `shOuterXZ >= 0.0` for every edge.
#[inline]
fn point_in_convex_poly_xz(px: f32, pz: f32, vertices: &[[f32; 2]]) -> bool {
    let n = vertices.len();
    for i in 0..n {
        let [ax, az] = vertices[i];
        let [bx, bz] = vertices[(i + 1) % n];
        if sh_outer_xz(px, pz, ax, az, bx, bz) < 0.0 {
            return false;
        }
    }
    true
}

/// Evaluates `EventCheckIn` (`SLUS_202.28` `0x001a8d70`, `sh2pc.exe` `0x0048da90`) for area
/// shapes `11..=13` (Rectangle, Triangle, Quad).
pub fn event_check_in(
    ev_pos: &[u8],
    offset: usize,
    shape: EventShapeKind,
    player_pos: [f32; 3],
) -> bool {
    let Some(decoded) = decode_ev_pos_shape(ev_pos, offset, shape) else {
        return false;
    };
    let [px, py, pz] = player_pos;
    match decoded {
        EvPosShape::Rectangle { x, y, z, w, d } => {
            if !player_height_matches_event_y(py, y) {
                return false;
            }
            px >= x && px <= x + w && pz >= z && pz <= z + d
        }
        EvPosShape::Triangle { y, vertices } => {
            if !player_height_matches_event_y(py, y) {
                return false;
            }
            point_in_convex_poly_xz(px, pz, &vertices)
        }
        EvPosShape::Quad { y, vertices } => {
            if !player_height_matches_event_y(py, y) {
                return false;
            }
            point_in_convex_poly_xz(px, pz, &vertices)
        }
        _ => false,
    }
}

/// Computes the representative 3D world position of an `ev_pos` shape (`EventPositionSet` at
/// `SLUS_202.28` `0x001a9570`, `sh2pc.exe` `0x0048b610`), used for positioning 3D door sound
/// effects (`pos.y -= 500.0`).
///
/// Prototype vs retail PC drift: `sh2pc.exe` (`0x0048b610`) clamps `y` to `0.0` when
/// `y > 45000.0` (`EVENT_IGNORE_HEIGHT_Y`) so height-ignored doors still emit 3D audio at floor
/// level instead of `y = 45000+`.
pub fn event_position_set(ev_pos: &[u8], offset: usize, shape: EventShapeKind) -> Option<[f32; 3]> {
    let x0 = read_f32(ev_pos, offset)?;
    let raw_y = read_f16(ev_pos, offset + 4)?;
    let z0 = read_f32(ev_pos, offset + 6)?;
    let y = if raw_y > EVENT_IGNORE_HEIGHT_Y {
        0.0
    } else {
        raw_y
    };

    match shape {
        EventShapeKind::Point => Some([x0, y, z0]),
        EventShapeKind::LinePosX => {
            let len = read_f16(ev_pos, offset + 10)?;
            Some([x0 + len * 0.5, y, z0])
        }
        EventShapeKind::LineNegX => {
            let len = read_f16(ev_pos, offset + 10)?;
            Some([x0 - len * 0.5, y, z0])
        }
        EventShapeKind::LineNegZ => {
            let len = read_f16(ev_pos, offset + 10)?;
            Some([x0, y, z0 - len * 0.5])
        }
        EventShapeKind::LinePosZ => {
            let len = read_f16(ev_pos, offset + 10)?;
            Some([x0, y, z0 + len * 0.5])
        }
        EventShapeKind::Polyline2 => {
            let dx = read_f16(ev_pos, offset + 10)?;
            let dz = read_f16(ev_pos, offset + 12)?;
            Some([x0 + dx * 0.5, y, z0 + dz * 0.5])
        }
        _ => Some([x0, y, z0]),
    }
}

/// Computes the destination player position and yaw angle (`rot_y` in radians) for `EventExecMove`
/// (`EventResultMovePosition` at `SLUS_202.28` `0x001a9730`, `sh2pc.exe` `0x0048b780`).
///
/// `current_rot_y` is James's current yaw (`_sh2jms + 0x34` / `DAT_01fb7c80 + 0x30`), which is
/// preserved verbatim when `dest_shape == EventDestShapeKind::Point` (`case 0`).
pub fn event_result_move_position(
    ev_pos: &[u8],
    dest_offset: usize,
    dest_shape: EventDestShapeKind,
    current_rot_y: f32,
) -> Option<([f32; 3], f32)> {
    let x0 = read_f32(ev_pos, dest_offset)?;
    let y = read_f16(ev_pos, dest_offset + 4)?;
    let z0 = read_f32(ev_pos, dest_offset + 6)?;

    // Exact 1:1 parity with `EventResultMovePosition` (`SLUS_202.28` `0x001a9730`,
    // `sh2pc.exe` `0x0048b780`).
    let (pos, angle) = match dest_shape {
        // Case 0: preserves James's current `rot_y` (`_sh2jms + 0x34` / `DAT_01fb7c80 + 0x30`).
        EventDestShapeKind::Point => ([x0, y, z0], current_rot_y),
        // Case 1 (`0x001a97f0` / `0x0048b855`): x = x0 + len/2, z = z0 + 250, rot_y = 0.0
        EventDestShapeKind::DoorLinePosX => {
            let len = read_f16(ev_pos, dest_offset + 10)?;
            ([x0 + len * 0.5, y, z0 + 250.0], 0.0)
        }
        // Case 2 (`0x001a983c` / `0x0048b888`): x = x0 - len/2, z = z0 - 250, rot_y = PI
        EventDestShapeKind::DoorLineNegX => {
            let len = read_f16(ev_pos, dest_offset + 10)?;
            ([x0 - len * 0.5, y, z0 - 250.0], PI)
        }
        // Case 3 (`0x001a9890` / `0x0048b8bb`): x = x0 + 250, z = z0 - len/2, rot_y = +PI/2
        EventDestShapeKind::DoorLineNegZ => {
            let len = read_f16(ev_pos, dest_offset + 10)?;
            ([x0 + 250.0, y, z0 - len * 0.5], FRAC_PI_2)
        }
        // Case 4 (`0x001a98e4` / `0x0048b8ee`): x = x0 - 250, z = z0 + len/2, rot_y = -PI/2
        EventDestShapeKind::DoorLinePosZ => {
            let len = read_f16(ev_pos, dest_offset + 10)?;
            ([x0 - 250.0, y, z0 + len * 0.5], -FRAC_PI_2)
        }
        // Case 5 (`0x001a9938` / `0x0048b921`): midpoint + 250 along normal.
        // Note: `shAtan2(dz, dx)` (`sh2pc.exe` `FUN_004482d0`) computes `atan2(dx, dz)`.
        EventDestShapeKind::DoorSegment => {
            let dx = read_f16(ev_pos, dest_offset + 10)?;
            let dz = read_f16(ev_pos, dest_offset + 12)?;
            let mid_x = x0 + dx * 0.5;
            let mid_z = z0 + dz * 0.5;
            let angle = dx.atan2(dz) - FRAC_PI_2;
            (
                [mid_x + angle.sin() * 250.0, y, mid_z + angle.cos() * 250.0],
                angle,
            )
        }
    };

    Some((pos, wrap_angle(angle)))
}
