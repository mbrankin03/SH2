//! Look-cone and proximity weight evaluators (`EventCheckLook`, `EventCheckLookPoint`,
//! `EventCheckLookLine`, `ItemCheckLookPoint` in `SLUS_202.28` `src/Event/event.c`).

use std::f32::consts::{FRAC_PI_2, FRAC_PI_3, FRAC_PI_4};

use crate::types::{wrap_angle, EventShapeKind, ItemListEntry};

use super::shapes::{decode_ev_pos_shape, player_height_matches_event_y, EvPosShape};
use super::EVENT_IGNORE_HEIGHT_Y;

/// Evaluates look/proximity weight against a 2D point `(tx, tz)` with `radius`.
///
/// Matches `EventCheckLookPoint` (`SLUS_202.28` `0x001a8a30`):
/// - Rejects if `dist_sq > radius * radius` or `radius <= 0.0`.
/// - Computes `da = wrap_angle(atan2(tx - px, tz - pz) - look_rot_y)`.
/// - Computes `w = (1.0 - |da| / (PI / 4)) * (1.0 - dist / radius)`.
/// - Returns `w` (`> 0.0`), or `0.0` if outside the `±45°` cone or radius.
pub fn event_check_look_point(
    px: f32,
    pz: f32,
    look_rot_y: f32,
    tx: f32,
    tz: f32,
    radius: f32,
) -> f32 {
    if radius <= 0.0 {
        return 0.0;
    }
    let dx = tx - px;
    let dz = tz - pz;
    let dist_sq = dx * dx + dz * dz;
    if dist_sq > radius * radius {
        return 0.0;
    }
    let target_angle = dx.atan2(dz);
    let da = wrap_angle(target_angle - look_rot_y).abs();
    let angle_weight = 1.0 - da / FRAC_PI_4;
    let dist_weight = 1.0 - dist_sq.sqrt() / radius;
    let weight = angle_weight * dist_weight;
    if weight > 0.0 {
        weight
    } else {
        0.0
    }
}

/// Evaluates look/proximity weight against a directed 2D line segment `(x1, z1) -> (x2, z2)` with `radius`.
///
/// Matches `EventCheckLookLine` (`SLUS_202.28` `0x001a8b10`):
/// 1. Front-side facing check: the segment's outward normal is `atan2(x2 - x1, z2 - z1) - PI / 2`;
///    rejects if `|wrap_angle(normal - look_rot_y)| > PI / 3` (`60°`).
/// 2. Projects `(px, pz)` onto the segment `(x1, z1)..(x2, z2)`, clamping to the nearest endpoint
///    when `t < 0.0` or `t > 1.0`, and delegates to [`event_check_look_point`].
#[allow(clippy::too_many_arguments)]
pub fn event_check_look_line(
    px: f32,
    pz: f32,
    look_rot_y: f32,
    x1: f32,
    z1: f32,
    x2: f32,
    z2: f32,
    radius: f32,
) -> f32 {
    let seg_dx = x2 - x1;
    let seg_dz = z2 - z1;
    let normal_angle = seg_dx.atan2(seg_dz) - FRAC_PI_2;
    if wrap_angle(normal_angle - look_rot_y).abs() > FRAC_PI_3 {
        return 0.0;
    }

    let seg_len_sq = seg_dx * seg_dx + seg_dz * seg_dz;
    if seg_len_sq <= f32::EPSILON {
        return event_check_look_point(px, pz, look_rot_y, x1, z1, radius);
    }

    let t = ((px - x1) * seg_dx + (pz - z1) * seg_dz) / seg_len_sq;
    if t < 0.0 {
        event_check_look_point(px, pz, look_rot_y, x1, z1, radius)
    } else if t > 1.0 {
        event_check_look_point(px, pz, look_rot_y, x2, z2, radius)
    } else {
        let proj_x = x1 + t * seg_dx;
        let proj_z = z1 + t * seg_dz;
        event_check_look_point(px, pz, look_rot_y, proj_x, proj_z, radius)
    }
}

/// Evaluates `EventCheckLook` (`SLUS_202.28` `0x001a86d0`) for shapes `0..=10`.
///
/// When `require_look` is `true` (`mode != 0`), uses `player_rot_y`.
/// When `require_look` is `false` (`mode == 0`), substitutes the ideal facing angle (`atan2(dx, dz)`
/// for points, or `atan2(x2 - x1, z2 - z1) - PI / 2` for segments) so only radial distance is weighted.
pub fn event_check_look(
    ev_pos: &[u8],
    offset: usize,
    shape: EventShapeKind,
    player_pos: [f32; 3],
    player_rot_y: f32,
    require_look: bool,
) -> f32 {
    let Some(decoded) = decode_ev_pos_shape(ev_pos, offset, shape) else {
        return 0.0;
    };
    let [px, py, pz] = player_pos;
    match decoded {
        EvPosShape::Point { x, y, z, radius } => {
            if !player_height_matches_event_y(py, y) {
                return 0.0;
            }
            let rot = if require_look {
                player_rot_y
            } else {
                (x - px).atan2(z - pz)
            };
            event_check_look_point(px, pz, rot, x, z, radius)
        }
        EvPosShape::Polyline {
            y,
            radius,
            vertices,
        } => {
            if !player_height_matches_event_y(py, y) {
                return 0.0;
            }
            let mut best = 0.0f32;
            for seg in vertices.windows(2) {
                let [x1, z1] = seg[0];
                let [x2, z2] = seg[1];
                let rot = if require_look {
                    player_rot_y
                } else {
                    (x2 - x1).atan2(z2 - z1) - FRAC_PI_2
                };
                let w = event_check_look_line(px, pz, rot, x1, z1, x2, z2, radius);
                if w > best {
                    best = w;
                }
            }
            best
        }
        _ => 0.0,
    }
}

/// Evaluates `ItemCheckLookPoint` (`SLUS_202.28` `0x001a93a0`) for a stage `Item_List` pickup.
///
/// Uses a `[-800.0, +100.0]` height window (unless `item_y >= 45000.0`), an `800.0` unit radius,
/// and scales the resulting look weight by `0.5`.
pub fn item_check_look_point(item: &ItemListEntry, player_pos: [f32; 3], player_rot_y: f32) -> f32 {
    let [ix, iy, iz] = item.position();
    let [px, py, pz] = player_pos;
    if iy < EVENT_IGNORE_HEIGHT_Y && (iy < py - 800.0 || iy > py + 100.0) {
        return 0.0;
    }
    event_check_look_point(px, pz, player_rot_y, ix, iz, 800.0) * 0.5
}
