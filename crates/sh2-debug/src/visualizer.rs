//! Collision, hitbox, and skeletal bone visualizer definitions for debugging physics hulls and armatures.
//!
//! # Debug Visualizers
//! - **Collision / Hitbox Debugger**: Visualizes `.cld` convex collision hulls, enemy attack bounding boxes,
//!   and interaction trigger volumes rendered as wireframe lines overlaying the 3D scene.
//! - **Bone Skeleton / Joint Axes Visualizer**: Extracts bone parent-child links and local XYZ joint coordinate
//!   axes from evaluated skeletal poses (`evaluate_pose`) for real-time armature inspection in the Model Viewer.

use bytemuck::{Pod, Zeroable};
use glam::Vec3;

/// Axis-aligned or oriented bounding box debug visualizer.
#[derive(Debug, Clone)]
pub struct HitboxDebugDraw {
    /// Bounding box center coordinates $(X, Y, Z)$.
    pub center: Vec3,
    /// Half-extents along each coordinate axis $(dX, dY, dZ)$.
    pub half_extents: Vec3,
    /// Wireframe display color in RGBA format.
    pub color: [f32; 4],
}

/// GPU-ready 3D colored line vertex for debug wireframe, bone skeleton, and joint axis rendering.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct DebugLineVertex {
    /// World-space 3D position $(X, Y, Z)$.
    pub position: [f32; 3],
    /// Linear RGBA color $(R, G, B, A)$.
    pub color: [f32; 4],
}

impl DebugLineVertex {
    /// Constructs a new colored 3D line vertex.
    pub const fn new(position: [f32; 3], color: [f32; 4]) -> Self {
        Self { position, color }
    }
}

/// Generates 3D line segments (`LineList` vertex pairs) representing a character's bone hierarchy
/// and local joint coordinate axes $(X=\text{Red}, Y=\text{Green}, Z=\text{Blue})$.
///
/// # Arguments
/// - `parents`: Parent bone index array (`255` indicates a root bone).
/// - `world_pose`: Evaluated row-major $4 \times 4$ model-space bone transform matrices.
/// - `model_world_mat`: Row-major $4 \times 4$ transform matrix placing the model in world space.
/// - `axis_length`: Length of each joint's RGB orientation axes in world units (e.g. `45.0` mm).
pub fn build_skeleton_debug_lines(
    parents: &[u8],
    world_pose: &[[[f32; 4]; 4]],
    model_world_mat: &[[f32; 4]; 4],
    axis_length: f32,
) -> Vec<DebugLineVertex> {
    let bone_count = parents.len().min(world_pose.len());
    let mut lines = Vec::with_capacity(bone_count * 8);

    // Transform each bone's model-space matrix by `model_world_mat`
    let mut global_mats = Vec::with_capacity(bone_count);
    for m in world_pose.iter().take(bone_count) {
        let mut combined = [[0.0f32; 4]; 4];
        for r in 0..4 {
            for c in 0..4 {
                combined[r][c] = model_world_mat[r][0] * m[0][c]
                    + model_world_mat[r][1] * m[1][c]
                    + model_world_mat[r][2] * m[2][c]
                    + model_world_mat[r][3] * m[3][c];
            }
        }
        global_mats.push(combined);
    }

    let bone_link_color = [1.0, 0.85, 0.2, 1.0]; // Golden yellow bone links
    let root_marker_color = [0.2, 0.95, 1.0, 1.0]; // Cyan root joint diamond
    let axis_x_color = [1.0, 0.2, 0.2, 1.0]; // Red +X
    let axis_y_color = [0.2, 1.0, 0.2, 1.0]; // Green +Y
    let axis_z_color = [0.2, 0.5, 1.0, 1.0]; // Blue +Z

    for i in 0..bone_count {
        let m = &global_mats[i];
        let pos = [m[0][3], m[1][3], m[2][3]];

        // 1. Bone link from parent joint to child joint
        let p = parents[i];
        if p != 255 && (p as usize) < bone_count {
            let pm = &global_mats[p as usize];
            let p_pos = [pm[0][3], pm[1][3], pm[2][3]];
            lines.push(DebugLineVertex::new(p_pos, bone_link_color));
            lines.push(DebugLineVertex::new(pos, bone_link_color));
        } else {
            // Root bone marker cross
            let r_ext = axis_length * 0.5;
            lines.push(DebugLineVertex::new(
                [pos[0] - r_ext, pos[1], pos[2]],
                root_marker_color,
            ));
            lines.push(DebugLineVertex::new(
                [pos[0] + r_ext, pos[1], pos[2]],
                root_marker_color,
            ));
        }

        // 2. Local joint orientation axes (columns 0, 1, 2 of upper 3x3 rotation matrix)
        if axis_length > 0.0 {
            let x_dir = Vec3::new(m[0][0], m[1][0], m[2][0]).normalize_or_zero() * axis_length;
            let y_dir = Vec3::new(m[0][1], m[1][1], m[2][1]).normalize_or_zero() * axis_length;
            let z_dir = Vec3::new(m[0][2], m[1][2], m[2][2]).normalize_or_zero() * axis_length;

            lines.push(DebugLineVertex::new(pos, axis_x_color));
            lines.push(DebugLineVertex::new(
                [pos[0] + x_dir.x, pos[1] + x_dir.y, pos[2] + x_dir.z],
                axis_x_color,
            ));

            lines.push(DebugLineVertex::new(pos, axis_y_color));
            lines.push(DebugLineVertex::new(
                [pos[0] + y_dir.x, pos[1] + y_dir.y, pos[2] + y_dir.z],
                axis_y_color,
            ));

            lines.push(DebugLineVertex::new(pos, axis_z_color));
            lines.push(DebugLineVertex::new(
                [pos[0] + z_dir.x, pos[1] + z_dir.y, pos[2] + z_dir.z],
                axis_z_color,
            ));
        }
    }

    lines
}

/// Generates wireframe line segments for an indexed triangle mesh (`[u32]` triples).
///
/// Transforms each vertex by its weighted skinning matrix on the CPU or uses bind-space positions
/// when inspecting static/skinned wireframe overlays.
pub fn build_wireframe_lines(
    positions: &[[f32; 3]],
    bone_indices: &[[u16; 4]],
    weights: &[[f32; 4]],
    skin_matrices: &[[[f32; 4]; 4]],
    triangles: &[[u16; 3]],
    color: [f32; 4],
) -> Vec<DebugLineVertex> {
    let v_count = positions.len();
    let mut skinned_pos = Vec::with_capacity(v_count);

    for (i, &p) in positions.iter().enumerate() {
        if skin_matrices.is_empty() {
            skinned_pos.push(p);
            continue;
        }
        let b = bone_indices.get(i).copied().unwrap_or([0; 4]);
        let w = weights.get(i).copied().unwrap_or([1.0, 0.0, 0.0, 0.0]);

        let mut out = [0.0f32; 3];
        for k in 0..4 {
            let weight = w[k];
            if weight <= 0.0 {
                continue;
            }
            let b_idx = (b[k] as usize).min(skin_matrices.len() - 1);
            let m = &skin_matrices[b_idx];
            let tx = m[0][0] * p[0] + m[0][1] * p[1] + m[0][2] * p[2] + m[0][3];
            let ty = m[1][0] * p[0] + m[1][1] * p[1] + m[1][2] * p[2] + m[1][3];
            let tz = m[2][0] * p[0] + m[2][1] * p[1] + m[2][2] * p[2] + m[2][3];
            out[0] += tx * weight;
            out[1] += ty * weight;
            out[2] += tz * weight;
        }
        skinned_pos.push(out);
    }

    let mut lines = Vec::with_capacity(triangles.len() * 6);
    for &[i0, i1, i2] in triangles {
        let (u0, u1, u2) = (i0 as usize, i1 as usize, i2 as usize);
        if u0 < v_count && u1 < v_count && u2 < v_count {
            let p0 = skinned_pos[u0];
            let p1 = skinned_pos[u1];
            let p2 = skinned_pos[u2];

            lines.push(DebugLineVertex::new(p0, color));
            lines.push(DebugLineVertex::new(p1, color));

            lines.push(DebugLineVertex::new(p1, color));
            lines.push(DebugLineVertex::new(p2, color));

            lines.push(DebugLineVertex::new(p2, color));
            lines.push(DebugLineVertex::new(p0, color));
        }
    }

    lines
}

/// Generates debug wireframe lines for background collision geometry (`clDbgDrawBGColiPoly`).
///
/// Floors are drawn with `floor_color` (e.g. green/cyan) and walls are drawn as vertical quad frames
/// with `wall_color` (e.g. red/orange).
pub fn build_collision_debug_lines(
    floors: &[[[f32; 3]; 3]],
    walls: &[([f32; 2], [f32; 2], f32, f32)],
    floor_color: [f32; 4],
    wall_color: [f32; 4],
) -> Vec<DebugLineVertex> {
    let mut lines = Vec::with_capacity(floors.len() * 6 + walls.len() * 8);
    build_collision_debug_lines_iter(
        floors.iter().copied(),
        walls.iter().copied(),
        floor_color,
        wall_color,
        &mut lines,
    );
    lines
}

/// Zero-allocation line vertex builder receiving iterators directly into a pre-allocated vector.
pub fn build_collision_debug_lines_iter<F, W>(
    floors: F,
    walls: W,
    floor_color: [f32; 4],
    wall_color: [f32; 4],
    lines: &mut Vec<DebugLineVertex>,
) where
    F: IntoIterator<Item = [[f32; 3]; 3]>,
    W: IntoIterator<Item = ([f32; 2], [f32; 2], f32, f32)>,
{
    // 1. Floor triangles
    for tri in floors {
        let p0 = tri[0];
        let p1 = tri[1];
        let p2 = tri[2];

        lines.push(DebugLineVertex::new(p0, floor_color));
        lines.push(DebugLineVertex::new(p1, floor_color));

        lines.push(DebugLineVertex::new(p1, floor_color));
        lines.push(DebugLineVertex::new(p2, floor_color));

        lines.push(DebugLineVertex::new(p2, floor_color));
        lines.push(DebugLineVertex::new(p0, floor_color));
    }

    // 2. Wall segment vertical quads: (start, end, y_top, y_bottom)
    for (start, end, y_top, y_bottom) in walls {
        let top_0 = [start[0], y_top, start[1]];
        let top_1 = [end[0], y_top, end[1]];
        let bot_0 = [start[0], y_bottom, start[1]];
        let bot_1 = [end[0], y_bottom, end[1]];

        // Top line
        lines.push(DebugLineVertex::new(top_0, wall_color));
        lines.push(DebugLineVertex::new(top_1, wall_color));

        // Bottom line
        lines.push(DebugLineVertex::new(bot_0, wall_color));
        lines.push(DebugLineVertex::new(bot_1, wall_color));

        // Left vertical
        lines.push(DebugLineVertex::new(top_0, wall_color));
        lines.push(DebugLineVertex::new(bot_0, wall_color));

        // Right vertical
        lines.push(DebugLineVertex::new(top_1, wall_color));
        lines.push(DebugLineVertex::new(bot_1, wall_color));
    }
}

/// Generates debug wireframe lines for character collision cylinders (`clDbgDrawCharaColi`).
///
/// Draws top and bottom circular rings connected by vertical boundary lines.
pub fn build_chara_collision_debug_lines(
    pos: [f32; 3],
    radius: f32,
    height: f32,
    segments: usize,
    color: [f32; 4],
) -> Vec<DebugLineVertex> {
    let segs = segments.max(8);
    let mut lines = Vec::with_capacity(segs * 4 + 8);
    let y_feet = pos[1];
    let y_head = pos[1] - height;

    for i in 0..segs {
        let theta0 = (i as f32) * std::f32::consts::TAU / (segs as f32);
        let theta1 = ((i + 1) as f32) * std::f32::consts::TAU / (segs as f32);

        let cos0 = theta0.cos() * radius;
        let sin0 = theta0.sin() * radius;
        let cos1 = theta1.cos() * radius;
        let sin1 = theta1.sin() * radius;

        // Feet ring
        lines.push(DebugLineVertex::new(
            [pos[0] + cos0, y_feet, pos[2] + sin0],
            color,
        ));
        lines.push(DebugLineVertex::new(
            [pos[0] + cos1, y_feet, pos[2] + sin1],
            color,
        ));

        // Head ring
        lines.push(DebugLineVertex::new(
            [pos[0] + cos0, y_head, pos[2] + sin0],
            color,
        ));
        lines.push(DebugLineVertex::new(
            [pos[0] + cos1, y_head, pos[2] + sin1],
            color,
        ));

        // 4 Vertical cardinal pillars
        if i % (segs / 4) == 0 {
            lines.push(DebugLineVertex::new(
                [pos[0] + cos0, y_feet, pos[2] + sin0],
                color,
            ));
            lines.push(DebugLineVertex::new(
                [pos[0] + cos0, y_head, pos[2] + sin0],
                color,
            ));
        }
    }

    lines
}
