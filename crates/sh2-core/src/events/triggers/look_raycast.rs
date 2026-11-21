//! Exact 1:1 boolean look-cone and 2D ray-segment intersection evaluators
//! (`EventCheckLook`, `EventCheckLookPoint`, `EventCheckLookLine`, `ItemCheckLookPoint`
//! in `SLUS_202.28` `src/Event/event.c` and retail PC `sh2pc.exe`).

use std::f32::consts::{FRAC_PI_6, PI};

use crate::types::{wrap_angle, EventShapeKind, ItemListEntry};

use super::shapes::{decode_ev_pos_shape, player_height_matches_event_y, EvPosShape};

/// Fixed horizontal look radius (`400.0` mm) used by `EventCheck` (`SLUS_202.28` `0x001a7ed0`),
/// `EventCheckLookPoint` (`0x001a8b00`, `sh2pc.exe` `0x0048afe0` / `0x006a91e8`),
/// `EventCheckLookLine` (`0x001a8c80`), and `ItemCheckLookPoint` (`0x001a93b0`, `0x0048b4d0`).
pub const EVENT_LOOK_RADIUS_MM: f32 = 400.0;

/// Squared horizontal look radius (`160_000.0` mm², `SLUS_202.28` `0x001a8b00` & `0x001a93b0`,
/// `sh2pc.exe` `0x0048afe0` / `0x006a91e4`).
pub const EVENT_LOOK_RADIUS_SQ_MM2: f32 = 160_000.0;

/// Half-angle (`30° = PI / 6 = 0.5235988` rad, `0x3f060a92`) of the look cone in
/// `EventCheckLookPoint` (`SLUS_202.28` `0x001a8b00`, `sh2pc.exe` `0x0048afe0`) and
/// `ItemCheckLookPoint` (`0x001a93b0`, `0x0048b4d0`).
pub const EVENT_LOOK_HALF_ANGLE_RAD: f32 = FRAC_PI_6;

/// Backward offset (`50.0` mm) behind James for the look-cone apex in `EventCheck`
/// (`SLUS_202.28` `0x001a7ed0` lines 498-501).
pub const EVENT_LOOK_BACK_OFFSET_MM: f32 = 50.0;

/// Minimum vertical delta (`-750.0` mm) for `ItemCheckLookPoint` (`SLUS_202.28` `0x001a93b0`,
/// `sh2pc.exe` `0x0048b4d0`).
pub const ITEM_LOOK_MIN_DY_MM: f32 = -750.0;

/// Maximum vertical delta (`+100.0` mm) for `ItemCheckLookPoint` (`SLUS_202.28` `0x001a93b0`,
/// `sh2pc.exe` `0x0048b4d0`).
pub const ITEM_LOOK_MAX_DY_MM: f32 = 100.0;

/// Evaluates whether James is within `400.0` mm and looking within `±30°` (`0.5235988` rad) of
/// a 2D point `(tx, tz)`, with cone apex `50.0` mm behind James.
///
/// Exact 1:1 parity with `EventCheckLookPoint` (`SLUS_202.28` `0x001a8b00`,
/// `sh2pc.exe` `0x0048afe0`). Returns `true` (`1`) on hit, `false` (`0`) otherwise.
pub fn event_check_look_point(px: f32, pz: f32, look_rot_y: f32, tx: f32, tz: f32) -> bool {
    let dx = tx - px;
    let dz = tz - pz;
    if dx.abs() > EVENT_LOOK_RADIUS_MM || dz.abs() > EVENT_LOOK_RADIUS_MM {
        return false;
    }
    if dx * dx + dz * dz > EVENT_LOOK_RADIUS_SQ_MM2 {
        return false;
    }
    // Apex 50.0 mm behind James (`EventCheck` `0x001a7ed0`: `pos + sin/cos(rot_y + PI) * 50.0`).
    let back_x = px + (look_rot_y + PI).sin() * EVENT_LOOK_BACK_OFFSET_MM;
    let back_z = pz + (look_rot_y + PI).cos() * EVENT_LOOK_BACK_OFFSET_MM;
    // `shAtan2(tz - back_z, tx - back_x)` (`sh2pc.exe` `FUN_004482d0`) computes `atan2(dx, dz)`.
    let target_angle = (tx - back_x).atan2(tz - back_z);
    let da = wrap_angle(look_rot_y - target_angle).abs();
    da <= EVENT_LOOK_HALF_ANGLE_RAD
}

/// Evaluates exact 2D ray-segment intersection between James's `400.0` mm forward look ray
/// `(px, pz) -> (px + sin(rot_y)*400, pz + cos(rot_y)*400)` and directed segment `(x1, z1) -> (x2, z2)`.
///
/// Exact 1:1 parity with `EventCheckLookLine` (`SLUS_202.28` `0x001a8c80`, `sh2pc.exe` `0x0048b0c0`):
/// 1. Retail `sh2pc.exe` (`0x0048b0c0`) AABB overlap precheck rejecting collinear non-overlapping segments.
/// 2. Exact 4-cross-product intersection test (`0x001a8c80` lines 829-831 & `0x0048b0c0` lines 6837-6842):
///    - `ax * look_dz - az * look_dx >= 0.0`
///    - `bx * look_dz - bz * look_dx <= 0.0`
///    - `ax * ez - az * ex >= 0.0`
///    - `(look_dx - ax) * ez - (look_dz - az) * ex >= 0.0`
#[allow(clippy::too_many_arguments)]
pub fn event_check_look_line(
    px: f32,
    pz: f32,
    look_rot_y: f32,
    x1: f32,
    z1: f32,
    x2: f32,
    z2: f32,
) -> bool {
    let look_dx = look_rot_y.sin() * EVENT_LOOK_RADIUS_MM;
    let look_dz = look_rot_y.cos() * EVENT_LOOK_RADIUS_MM;

    let ax = x1 - px;
    let az = z1 - pz;
    let bx = x2 - px;
    let bz = z2 - pz;

    // Retail PC (`sh2pc.exe` `0x0048b0c0`) bounding-box overlap guard rejecting collinear segments
    // outside the `[0, look_dx] x [0, look_dz]` ray extent.
    if look_dx.min(0.0) > ax.max(bx)
        || ax.min(bx) > look_dx.max(0.0)
        || look_dz.min(0.0) > az.max(bz)
        || az.min(bz) > look_dz.max(0.0)
    {
        return false;
    }

    let ex = bx - ax;
    let ez = bz - az;

    let c1 = ax * look_dz - az * look_dx;
    let c2 = bx * look_dz - bz * look_dx;
    let c3 = ax * ez - az * ex;
    let c4 = (look_dx - ax) * ez - (look_dz - az) * ex;

    c1 >= 0.0 && c2 <= 0.0 && c3 >= 0.0 && c4 >= 0.0
}

/// Evaluates `EventCheckLook` (`SLUS_202.28` `0x001a8780`, `sh2pc.exe` `0x0048d4d0`) for
/// shapes `0..=10` (Point, Cardinal Lines `1..=4`, and Polylines `5..=10`).
pub fn event_check_look(
    ev_pos: &[u8],
    offset: usize,
    shape: EventShapeKind,
    player_pos: [f32; 3],
    player_rot_y: f32,
) -> bool {
    let Some(decoded) = decode_ev_pos_shape(ev_pos, offset, shape) else {
        return false;
    };
    let [px, py, pz] = player_pos;
    match decoded {
        EvPosShape::Point { x, y, z } => {
            if !player_height_matches_event_y(py, y) {
                return false;
            }
            event_check_look_point(px, pz, player_rot_y, x, z)
        }
        EvPosShape::Polyline { y, vertices } => {
            if !player_height_matches_event_y(py, y) {
                return false;
            }
            vertices.windows(2).any(|seg| {
                let [x1, z1] = seg[0];
                let [x2, z2] = seg[1];
                event_check_look_line(px, pz, player_rot_y, x1, z1, x2, z2)
            })
        }
        _ => false,
    }
}

/// Evaluates `ItemCheckLookPoint` (`SLUS_202.28` `0x001a93b0`, `sh2pc.exe` `0x0048b4d0`) for a
/// stage `Item_List` pickup.
///
/// Uses a strict `[-750.0, +100.0]` mm height window (with **no** `45000.0` bypass), fixed
/// `400.0` mm radius, and `±30°` (`0.5235988` rad) cone from `50.0` mm behind James.
pub fn item_check_look_point(
    item: &ItemListEntry,
    player_pos: [f32; 3],
    player_rot_y: f32,
) -> bool {
    let [ix, iy, iz] = item.position();
    let [px, py, pz] = player_pos;
    if iy > py + ITEM_LOOK_MAX_DY_MM || iy < py + ITEM_LOOK_MIN_DY_MM {
        return false;
    }
    event_check_look_point(px, pz, player_rot_y, ix, iz)
}
