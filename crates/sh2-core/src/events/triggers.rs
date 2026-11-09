//! Spatial Event Trigger Volumes, Look/Touch Geometry Evaluators, and `EventCheck` Selection
//! (`SLUS_202.28` `src/Event/event.c` at `0x001a7dc0..0x001a9be0`).

use std::f32::consts::{FRAC_PI_2, FRAC_PI_3, FRAC_PI_4, PI};

use crate::config::ActionDifficulty;
use crate::types::{
    char_to_float2, wrap_angle, EventDestShapeKind, EventListEntry, EventShapeKind,
    EventTriggerKind, ItemListEntry,
};

use super::EventFlags;

/// Maximum number of `Event_List` entries tracked per stage for rising-edge triggers (`cond = 1` / `7`).
pub const MAX_STAGE_EVENT_ENTRIES: usize = 256;

/// Index offset in `ev_work_no` (`0x01126380`) distinguishing `Item_List` pickups (`>= 0x400`)
/// from `Event_List` entries (`0..0x400`).
pub const EVENT_ITEM_INDEX_BASE: usize = 0x400;

/// Sentinel Y threshold (`45000.0`, `0x472fc800`): any `ev_pos` or `Item_List` with `y >= 45000.0`
/// ignores the vertical floor height window check.
pub const EVENT_IGNORE_HEIGHT_Y: f32 = 45_000.0;

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
fn read_f32_i16(ev_pos: &[u8], byte_offset: usize) -> Option<f32> {
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

/// Applies the secondary flag toggle side effects of `EventExecSubFlagSet`
/// (`SLUS_202.28` `0x001a9a90`).
pub fn event_exec_sub_flag_set(entry: &EventListEntry, flags: &mut EventFlags) {
    let f0 = entry.flag_0() as usize;
    if f0 != 0 && entry.flag_0_toggle_on_exec() {
        flags.set(f0, !entry.flag_0_inverted());
    }
    let f1 = entry.flag_1() as usize;
    if f1 != 0 && entry.flag_1_toggle_on_exec() {
        flags.set(f1, !entry.flag_1_inverted());
    }
}

/// Evaluates whether the two flag conditions (`flag_0` and `flag_1`) on `entry` are satisfied
/// (`EventCheck` at `SLUS_202.28` `0x001a7e60..0x001a7fb0`).
pub fn check_event_flag_conditions(entry: &EventListEntry, flags: &EventFlags) -> bool {
    let f0 = entry.flag_0() as usize;
    if f0 != 0 {
        let is_set = flags.get(f0);
        if entry.flag_0_inverted() {
            if is_set {
                return false;
            }
        } else if !is_set {
            return false;
        }
    }

    let f1 = entry.flag_1() as usize;
    if f1 != 0 {
        let is_set = flags.get(f1);
        if entry.flag_1_inverted() {
            if is_set {
                return false;
            }
        } else if !is_set {
            return false;
        }
    }

    true
}

/// Evaluates `EventItemConditionCheck` (`SLUS_202.28` `0x001ab1a0`) for a stage `Item_List` entry.
pub fn event_item_condition_check(
    item: &ItemListEntry,
    action_difficulty: ActionDifficulty,
    flags: &EventFlags,
) -> bool {
    let cond_diff = item.difficulty_condition();
    let diff_blocked = match action_difficulty {
        ActionDifficulty::Easy => matches!(cond_diff, 2 | 3 | 6),
        ActionDifficulty::Normal => matches!(cond_diff, 1 | 3 | 5),
        ActionDifficulty::Hard => matches!(cond_diff, 1 | 2 | 4),
    };
    if diff_blocked {
        return false;
    }

    match item.extra_condition() {
        1 => flags.get(219),
        2 => flags.get(108),
        3 => flags.get(208),
        _ => true,
    }
}

/// Per-frame input context for `EventCheck` (`SLUS_202.28` `0x001a7dc0`).
#[derive(Debug, Clone, Copy)]
pub struct EventEvalInput {
    pub player_pos: [f32; 3],
    pub player_rot_y: f32,
    /// `ev_active` (`0` = idle, `1` = non-blocking background event, `2` = blocking event).
    pub ev_active: u8,
    /// `true` if the Action/Search button (`pad->trg & 0x4000`) was pressed this frame.
    pub action_pressed: bool,
    /// `true` if the Map button was pressed this frame while map viewing is unavailable.
    pub map_disabled_pressed: bool,
    /// `true` on the first frame after entering a new room (`RoomChange != 0`).
    pub room_changed: bool,
    /// `true` on the first frame after loading a new stage (`StageChange != 0`).
    pub stage_changed: bool,
    /// Currently used inventory item ID (`use_item_id != 0` when using an item from the menu).
    pub used_item_id: Option<u8>,
    /// `true` if the game is currently running an automated attract/demo sequence.
    pub is_demo_mode: bool,
    /// Current action difficulty (for `Item_List` filtering).
    pub action_difficulty: ActionDifficulty,
    /// `true` if player controls/stamina allow search interactions.
    pub player_can_search: bool,
}

impl Default for EventEvalInput {
    fn default() -> Self {
        Self {
            player_pos: [0.0, 0.0, 0.0],
            player_rot_y: 0.0,
            ev_active: 0,
            action_pressed: false,
            map_disabled_pressed: false,
            room_changed: false,
            stage_changed: false,
            used_item_id: None,
            is_demo_mode: false,
            action_difficulty: ActionDifficulty::Normal,
            player_can_search: true,
        }
    }
}

/// Persistent rising-edge state for `cond = 1` (`TouchEdge`, `0x011264b0`) and
/// `cond = 7` (`ZoneEnterEdge`, `0x011266b0`).
#[derive(Debug, Clone)]
pub struct EventTriggerState {
    touch_edge_latched: [bool; MAX_STAGE_EVENT_ENTRIES],
    zone_edge_latched: [bool; MAX_STAGE_EVENT_ENTRIES],
}

impl Default for EventTriggerState {
    fn default() -> Self {
        Self::new()
    }
}

impl EventTriggerState {
    pub const fn new() -> Self {
        Self {
            touch_edge_latched: [false; MAX_STAGE_EVENT_ENTRIES],
            zone_edge_latched: [false; MAX_STAGE_EVENT_ENTRIES],
        }
    }

    pub fn reset(&mut self) {
        self.touch_edge_latched = [false; MAX_STAGE_EVENT_ENTRIES];
        self.zone_edge_latched = [false; MAX_STAGE_EVENT_ENTRIES];
    }
}

/// Evaluates a single `EventListEntry` at `index` and returns its trigger weight (`> 0.0` if triggered).
pub fn evaluate_event_entry(
    index: usize,
    entry: &EventListEntry,
    ev_pos: &[u8],
    flags: &EventFlags,
    input: &EventEvalInput,
    trigger_state: &mut EventTriggerState,
) -> f32 {
    if entry.is_sentinel() {
        return 0.0;
    }
    if input.is_demo_mode && entry.disabled_in_demo() {
        return 0.0;
    }
    if !check_event_flag_conditions(entry, flags) {
        return 0.0;
    }

    let Some(trigger_kind) = entry.trigger_kind() else {
        return 0.0;
    };
    let offset = entry.ev_pos_offset();

    match trigger_kind {
        EventTriggerKind::TouchContinuous => {
            if input.ev_active == 2 {
                return 0.0;
            }
            let Some(shape) = entry.shape_kind() else {
                return 0.0;
            };
            if (shape as u8) <= 10 {
                event_check_look(
                    ev_pos,
                    offset,
                    shape,
                    input.player_pos,
                    input.player_rot_y,
                    false,
                )
            } else if event_check_in(ev_pos, offset, shape, input.player_pos) {
                1.0
            } else {
                0.0
            }
        }
        EventTriggerKind::TouchEdge => {
            if input.ev_active == 2 {
                return 0.0;
            }
            let Some(shape) = entry.shape_kind() else {
                return 0.0;
            };
            let w = if (shape as u8) <= 10 {
                event_check_look(
                    ev_pos,
                    offset,
                    shape,
                    input.player_pos,
                    input.player_rot_y,
                    false,
                )
            } else if event_check_in(ev_pos, offset, shape, input.player_pos) {
                1.0
            } else {
                0.0
            };
            let inside = w > 0.0;
            let latched = trigger_state
                .touch_edge_latched
                .get(index)
                .copied()
                .unwrap_or(false);
            if let Some(slot) = trigger_state.touch_edge_latched.get_mut(index) {
                *slot = inside;
            }
            if inside && !latched {
                w
            } else {
                0.0
            }
        }
        EventTriggerKind::ActionLook => {
            if input.ev_active != 0 || !input.action_pressed || !input.player_can_search {
                return 0.0;
            }
            let Some(shape) = entry.shape_kind() else {
                return 0.0;
            };
            if (shape as u8) <= 10 {
                event_check_look(
                    ev_pos,
                    offset,
                    shape,
                    input.player_pos,
                    input.player_rot_y,
                    true,
                )
            } else if event_check_in(ev_pos, offset, shape, input.player_pos) {
                0.01
            } else {
                0.0
            }
        }
        EventTriggerKind::ActionTouch => {
            if input.ev_active != 0 || !input.action_pressed || !input.player_can_search {
                return 0.0;
            }
            let Some(shape) = entry.shape_kind() else {
                return 0.0;
            };
            if (shape as u8) <= 10 {
                event_check_look(
                    ev_pos,
                    offset,
                    shape,
                    input.player_pos,
                    input.player_rot_y,
                    false,
                )
            } else if event_check_in(ev_pos, offset, shape, input.player_pos) {
                0.01
            } else {
                0.0
            }
        }
        EventTriggerKind::RoomInit => {
            if input.ev_active != 2 && input.room_changed {
                100.0
            } else {
                0.0
            }
        }
        EventTriggerKind::StageInit => {
            if input.ev_active != 2 && input.stage_changed {
                100.0
            } else {
                0.0
            }
        }
        EventTriggerKind::UseItemLook => {
            if input.ev_active != 0 || !input.player_can_search {
                return 0.0;
            }
            if input.used_item_id != Some(entry.required_item_id()) {
                return 0.0;
            }
            let Some(shape) = entry.shape_kind() else {
                return 0.0;
            };
            if (shape as u8) <= 10 {
                event_check_look(
                    ev_pos,
                    offset,
                    shape,
                    input.player_pos,
                    input.player_rot_y,
                    true,
                )
            } else if event_check_in(ev_pos, offset, shape, input.player_pos) {
                0.01
            } else {
                0.0
            }
        }
        EventTriggerKind::ZoneEnterEdge => {
            if input.ev_active != 0 {
                return 0.0;
            }
            let Some(shape) = entry.shape_kind() else {
                return 0.0;
            };
            let w = if (shape as u8) <= 10 {
                event_check_look(
                    ev_pos,
                    offset,
                    shape,
                    input.player_pos,
                    input.player_rot_y,
                    false,
                )
            } else if event_check_in(ev_pos, offset, shape, input.player_pos) {
                1.0
            } else {
                0.0
            };
            let inside = w > 0.0;
            let latched = trigger_state
                .zone_edge_latched
                .get(index)
                .copied()
                .unwrap_or(false);
            if let Some(slot) = trigger_state.zone_edge_latched.get_mut(index) {
                *slot = inside;
            }
            if inside && !latched {
                w
            } else {
                0.0
            }
        }
        EventTriggerKind::UseItemTouch => {
            if input.ev_active != 0 || !input.player_can_search {
                return 0.0;
            }
            if input.used_item_id != Some(entry.required_item_id()) {
                return 0.0;
            }
            let Some(shape) = entry.shape_kind() else {
                return 0.0;
            };
            if (shape as u8) <= 10 {
                event_check_look(
                    ev_pos,
                    offset,
                    shape,
                    input.player_pos,
                    input.player_rot_y,
                    false,
                )
            } else if event_check_in(ev_pos, offset, shape, input.player_pos) {
                0.01
            } else {
                0.0
            }
        }
        EventTriggerKind::Always => {
            if input.ev_active == 0 {
                100.0
            } else {
                0.0
            }
        }
        EventTriggerKind::MapDisabled => {
            if input.ev_active == 0 && input.map_disabled_pressed {
                100.0
            } else {
                0.0
            }
        }
    }
}

/// Evaluates `EventCheck` (`SLUS_202.28` `0x001a7dc0`) across the stage's `Event_List` and `Item_List`.
///
/// Returns `Some(work_no)` for the highest-weight matching candidate:
/// - `0..0x400`: index into `ev_list`
/// - `0x400..`: `0x400 + index` into `item_list`
pub fn event_check(
    ev_list: &[EventListEntry],
    item_list: &[ItemListEntry],
    ev_pos: &[u8],
    flags: &EventFlags,
    input: &EventEvalInput,
    trigger_state: &mut EventTriggerState,
) -> Option<usize> {
    let mut best_weight = 0.0f32;
    let mut best_index = None;

    for (idx, entry) in ev_list.iter().enumerate() {
        if entry.is_sentinel() {
            break;
        }
        let weight = evaluate_event_entry(idx, entry, ev_pos, flags, input, trigger_state);
        if weight > best_weight {
            best_weight = weight;
            best_index = Some(idx);
        }
    }

    if input.ev_active == 0 && input.action_pressed && input.player_can_search {
        for (idx, item) in item_list.iter().enumerate() {
            if item.is_sentinel() {
                break;
            }
            if !event_item_condition_check(item, input.action_difficulty, flags) {
                continue;
            }
            let base_flag = item.flag_id() as usize;
            // Already picked up if flag (base_flag + 1) is set
            if flags.get(base_flag + 1) {
                continue;
            }
            let vis_mode = item.visibility_mode();
            if vis_mode == 1 && !flags.get(base_flag) {
                continue;
            }
            if vis_mode == 2 && flags.get(base_flag) {
                continue;
            }
            let weight = item_check_look_point(item, input.player_pos, input.player_rot_y);
            if weight > best_weight {
                best_weight = weight;
                best_index = Some(EVENT_ITEM_INDEX_BASE + idx);
            }
        }
    }

    best_index
}
