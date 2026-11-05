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
