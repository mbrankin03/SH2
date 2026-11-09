//! Spatial trigger shape decoding (`EvPosShape`), area containment (`EventCheckIn`),
//! and destination placement (`EventPositionSet`, `EventResultMovePosition`).

use std::f32::consts::{FRAC_PI_2, PI};

use crate::types::{char_to_float2, wrap_angle, EventDestShapeKind, EventShapeKind};

use super::EVENT_IGNORE_HEIGHT_Y;

/// Decoded spatial trigger shape from a stage's raw `ev_pos` byte array.
#[derive(Debug, Clone, PartialEq)]
pub enum EvPosShape {
    /// Shape `0`: Point `(x, y, z)` with interaction `radius`.
    Point { x: f32, y: f32, z: f32, radius: f32 },
    /// Shapes `1..=10`: One or more 2D line segments at height `y` with interaction `radius`.
    Polyline {
        y: f32,
        radius: f32,
        vertices: Vec<[f32; 2]>,
    },
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
    /// Shape `13`: Convex 2D quad at height `y` (evaluated as triangles `(0,1,2)` and `(0,2,3)`).
    Quad { y: f32, vertices: [[f32; 2]; 4] },
}

#[inline]
pub(crate) fn read_f32_i16(ev_pos: &[u8], byte_offset: usize) -> Option<f32> {
    let b0 = *ev_pos.get(byte_offset)?;
    let b1 = *ev_pos.get(byte_offset + 1)?;
    Some(char_to_float2([b0, b1]))
}

/// Decodes an [`EvPosShape`] from `ev_pos` starting at `offset` (`EventCheckLook` / `EventCheckIn`).
pub fn decode_ev_pos_shape(
    ev_pos: &[u8],
    offset: usize,
    shape: EventShapeKind,
) -> Option<EvPosShape> {
    if offset.checked_add(shape.byte_len())? > ev_pos.len() {
        return None;
    }
    let x0 = read_f32_i16(ev_pos, offset)?;
    let y = read_f32_i16(ev_pos, offset + 2)?;
    let z0 = read_f32_i16(ev_pos, offset + 4)?;

    match shape {
        EventShapeKind::Point => {
            let radius = read_f32_i16(ev_pos, offset + 6)?;
            Some(EvPosShape::Point {
                x: x0,
                y,
                z: z0,
                radius,
            })
        }
        EventShapeKind::LineNegZ => {
            let radius = read_f32_i16(ev_pos, offset + 6)?;
            let len = read_f32_i16(ev_pos, offset + 8)?;
            Some(EvPosShape::Polyline {
                y,
                radius,
                vertices: vec![[x0, z0], [x0, z0 - len]],
            })
        }
        EventShapeKind::LinePosX => {
            let radius = read_f32_i16(ev_pos, offset + 6)?;
            let len = read_f32_i16(ev_pos, offset + 8)?;
            Some(EvPosShape::Polyline {
                y,
                radius,
                vertices: vec![[x0, z0], [x0 + len, z0]],
            })
        }
        EventShapeKind::LinePosZ => {
            let radius = read_f32_i16(ev_pos, offset + 6)?;
            let len = read_f32_i16(ev_pos, offset + 8)?;
            Some(EvPosShape::Polyline {
                y,
                radius,
                vertices: vec![[x0, z0], [x0, z0 + len]],
            })
        }
        EventShapeKind::LineNegX => {
            let radius = read_f32_i16(ev_pos, offset + 6)?;
            let len = read_f32_i16(ev_pos, offset + 8)?;
            Some(EvPosShape::Polyline {
                y,
                radius,
                vertices: vec![[x0, z0], [x0 - len, z0]],
            })
        }
        EventShapeKind::Polyline2
        | EventShapeKind::Polyline3
        | EventShapeKind::Polyline4
        | EventShapeKind::Polyline5
        | EventShapeKind::Polyline6
        | EventShapeKind::Polyline7 => {
            let radius = read_f32_i16(ev_pos, offset + 6)?;
            let seg_count = (shape as usize) - 4;
            let mut vertices = Vec::with_capacity(seg_count + 1);
            vertices.push([x0, z0]);
            for i in 0..seg_count {
                let vx = read_f32_i16(ev_pos, offset + 8 + i * 4)?;
                let vz = read_f32_i16(ev_pos, offset + 10 + i * 4)?;
                vertices.push([vx, vz]);
            }
            Some(EvPosShape::Polyline {
                y,
                radius,
                vertices,
            })
        }
        EventShapeKind::Rectangle => {
            let w = read_f32_i16(ev_pos, offset + 6)?;
            let d = read_f32_i16(ev_pos, offset + 8)?;
            Some(EvPosShape::Rectangle {
                x: x0,
                y,
                z: z0,
                w,
                d,
            })
        }
        EventShapeKind::Triangle => {
            let x1 = read_f32_i16(ev_pos, offset + 6)?;
            let z1 = read_f32_i16(ev_pos, offset + 8)?;
            let x2 = read_f32_i16(ev_pos, offset + 10)?;
            let z2 = read_f32_i16(ev_pos, offset + 12)?;
            Some(EvPosShape::Triangle {
                y,
                vertices: [[x0, z0], [x1, z1], [x2, z2]],
            })
        }
        EventShapeKind::Quad => {
            let x1 = read_f32_i16(ev_pos, offset + 6)?;
            let z1 = read_f32_i16(ev_pos, offset + 8)?;
            let x2 = read_f32_i16(ev_pos, offset + 10)?;
            let z2 = read_f32_i16(ev_pos, offset + 12)?;
            let x3 = read_f32_i16(ev_pos, offset + 14)?;
            let z3 = read_f32_i16(ev_pos, offset + 16)?;
            Some(EvPosShape::Quad {
                y,
                vertices: [[x0, z0], [x1, z1], [x2, z2], [x3, z3]],
            })
        }
    }
}

/// Checks whether the event's `y` coordinate is within the vertical window relative to `player_y`
/// (`y >= 45000.0 || (player_y - 500.0 <= y && y <= player_y + 100.0)`).
///
/// Matches `EventCheckLook` (`0x001a86f0..0x001a8740`) and `EventCheckIn` (`0x001a8cf0..0x001a8d38`).
#[inline]
pub fn player_height_matches_event_y(player_y: f32, event_y: f32) -> bool {
    event_y >= EVENT_IGNORE_HEIGHT_Y || (player_y - 500.0 <= event_y && event_y <= player_y + 100.0)
}

#[inline]
fn point_in_triangle_2d(px: f32, pz: f32, v: [[f32; 2]; 3]) -> bool {
    let mut pos_count = 0u8;
    let mut neg_count = 0u8;
    for i in 0..3 {
        let [x1, z1] = v[i];
        let [x2, z2] = v[(i + 1) % 3];
        let cross = (x2 - x1) * (pz - z1) - (px - x1) * (z2 - z1);
        if cross >= 0.0 {
            pos_count += 1;
        }
        if cross <= 0.0 {
            neg_count += 1;
        }
    }
    pos_count == 3 || neg_count == 3
}

/// Evaluates `EventCheckIn` (`SLUS_202.28` `0x001a8cd0`) for area shapes `11..=13`
/// (Rectangle, Triangle, Quad).
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
            point_in_triangle_2d(px, pz, vertices)
        }
        EvPosShape::Quad { y, vertices } => {
            if !player_height_matches_event_y(py, y) {
                return false;
            }
            point_in_triangle_2d(px, pz, [vertices[0], vertices[1], vertices[2]])
                || point_in_triangle_2d(px, pz, [vertices[0], vertices[2], vertices[3]])
        }
        _ => false,
    }
}

/// Computes the representative 3D world position of an `ev_pos` shape (`EventPositionSet` at
/// `SLUS_202.28` `0x001a9570`), used for positioning 3D door sound effects (`pos.y -= 500.0`).
pub fn event_position_set(ev_pos: &[u8], offset: usize, shape: EventShapeKind) -> Option<[f32; 3]> {
    let x0 = read_f32_i16(ev_pos, offset)?;
    let y = read_f32_i16(ev_pos, offset + 2)?;
    let z0 = read_f32_i16(ev_pos, offset + 4)?;

    match shape {
        EventShapeKind::Point => Some([x0, y, z0]),
        EventShapeKind::LineNegZ => {
            let len = read_f32_i16(ev_pos, offset + 8)?;
            Some([x0, y, z0 - len * 0.5])
        }
        EventShapeKind::LinePosX => {
            let len = read_f32_i16(ev_pos, offset + 8)?;
            Some([x0 + len * 0.5, y, z0])
        }
        EventShapeKind::LinePosZ => {
            let len = read_f32_i16(ev_pos, offset + 8)?;
            Some([x0, y, z0 + len * 0.5])
        }
        EventShapeKind::LineNegX => {
            let len = read_f32_i16(ev_pos, offset + 8)?;
            Some([x0 - len * 0.5, y, z0])
        }
        EventShapeKind::Polyline2 => {
            let dx = read_f32_i16(ev_pos, offset + 8)?;
            let dz = read_f32_i16(ev_pos, offset + 10)?;
            Some([x0 + dx * 0.5, y, z0 + dz * 0.5])
        }
        _ => Some([x0, y, z0]),
    }
}

/// Computes the destination player position and yaw angle (`rot_y` in radians) for `EventExecMove`
/// (`EventResultMovePosition` at `SLUS_202.28` `0x001a9730`).
pub fn event_result_move_position(
    ev_pos: &[u8],
    dest_offset: usize,
    dest_shape: EventDestShapeKind,
) -> Option<([f32; 3], f32)> {
    let x0 = read_f32_i16(ev_pos, dest_offset)?;
    let y = read_f32_i16(ev_pos, dest_offset + 2)?;
    let z0 = read_f32_i16(ev_pos, dest_offset + 4)?;

    let (pos, angle) = match dest_shape {
        EventDestShapeKind::PointWithAngle => {
            let deg = read_f32_i16(ev_pos, dest_offset + 6)?;
            ([x0, y, z0], deg * PI / 180.0)
        }
        EventDestShapeKind::DoorLineNegZ => {
            let len = read_f32_i16(ev_pos, dest_offset + 8)?;
            ([x0 - 250.0, y, z0 - len * 0.5], -FRAC_PI_2)
        }
        EventDestShapeKind::DoorLinePosX => {
            let len = read_f32_i16(ev_pos, dest_offset + 8)?;
            ([x0 + len * 0.5, y, z0 - 250.0], PI)
        }
        EventDestShapeKind::DoorLinePosZ => {
            let len = read_f32_i16(ev_pos, dest_offset + 8)?;
            ([x0 + 250.0, y, z0 + len * 0.5], FRAC_PI_2)
        }
        EventDestShapeKind::DoorLineNegX => {
            let len = read_f32_i16(ev_pos, dest_offset + 8)?;
            ([x0 - len * 0.5, y, z0 + 250.0], 0.0)
        }
        EventDestShapeKind::DoorSegment => {
            let dx = read_f32_i16(ev_pos, dest_offset + 10)?;
            let dz = read_f32_i16(ev_pos, dest_offset + 12)?;
            let mid_x = x0 + dx * 0.5;
            let mid_z = z0 + dz * 0.5;
            let angle = dz.atan2(dx) - FRAC_PI_2;
            (
                [mid_x + angle.sin() * 250.0, y, mid_z + angle.cos() * 250.0],
                angle,
            )
        }
    };

    Some((pos, wrap_angle(angle)))
}
