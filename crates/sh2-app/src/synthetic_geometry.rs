//! Synthetic test room geometry and procedural debug mesh builders.

use sh2_assets::formats::map::{BatchGroup, MapBatch, MapFile, MapMaterial};

/// Helper closure for appending quad geometry into vertex attribute buffers.
#[allow(clippy::too_many_arguments)]
pub fn append_quad(
    positions: &mut Vec<[f32; 3]>,
    normals: &mut Vec<[f32; 3]>,
    uvs: &mut Vec<[f32; 2]>,
    colors: &mut Vec<[u8; 4]>,
    indices: &mut Vec<u32>,
    p0: [f32; 3],
    p1: [f32; 3],
    p2: [f32; 3],
    p3: [f32; 3],
    normal: [f32; 3],
    col: [u8; 4],
) {
    let base_idx = positions.len() as u32;
    positions.extend_from_slice(&[p0, p1, p2, p3]);
    normals.extend_from_slice(&[normal, normal, normal, normal]);
    uvs.extend_from_slice(&[[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]);
    colors.extend_from_slice(&[col, col, col, col]);
    indices.extend_from_slice(&[
        base_idx,
        base_idx + 1,
        base_idx + 2,
        base_idx,
        base_idx + 2,
        base_idx + 3,
    ]);
}

/// Generate an authentic 3D Silent Hill enclosed room with floor, textured walls, ceiling, and columns.
pub fn create_fallback_room_geometry() -> MapFile {
    let mut map = MapFile::default();
    map.materials.push(MapMaterial {
        mode: 1,
        texture_id: 0,
        color: 0xFF888888,
        overlay: 0,
        specularity: 0.5,
    });

    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    let mut colors = Vec::new();
    let mut indices = Vec::new();

    let w = 8000.0;
    let l = 12000.0;
    let h = -2400.0; // Y down in PS2 coordinate system

    // Floor (Y = 0.0, normal points UP = -Y)
    append_quad(
        &mut positions,
        &mut normals,
        &mut uvs,
        &mut colors,
        &mut indices,
        [-w, 0.0, -l],
        [w, 0.0, -l],
        [w, 0.0, l],
        [-w, 0.0, l],
        [0.0, -1.0, 0.0],
        [160, 150, 140, 255],
    );

    // Ceiling (Y = h, normal points DOWN = +Y)
    append_quad(
        &mut positions,
        &mut normals,
        &mut uvs,
        &mut colors,
        &mut indices,
        [-w, h, l],
        [w, h, l],
        [w, h, -l],
        [-w, h, -l],
        [0.0, 1.0, 0.0],
        [110, 100, 95, 255],
    );

    // North Wall (Z = -l)
    append_quad(
        &mut positions,
        &mut normals,
        &mut uvs,
        &mut colors,
        &mut indices,
        [-w, h, -l],
        [w, h, -l],
        [w, 0.0, -l],
        [-w, 0.0, -l],
        [0.0, 0.0, 1.0],
        [180, 170, 160, 255],
    );

    // South Wall (Z = l)
    append_quad(
        &mut positions,
        &mut normals,
        &mut uvs,
        &mut colors,
        &mut indices,
        [w, h, l],
        [-w, h, l],
        [-w, 0.0, l],
        [w, 0.0, l],
        [0.0, 0.0, -1.0],
        [180, 170, 160, 255],
    );

    // West Wall (X = -w)
    append_quad(
        &mut positions,
        &mut normals,
        &mut uvs,
        &mut colors,
        &mut indices,
        [-w, h, l],
        [-w, h, -l],
        [-w, 0.0, -l],
        [-w, 0.0, l],
        [1.0, 0.0, 0.0],
        [175, 165, 155, 255],
    );

    // East Wall (X = w)
    append_quad(
        &mut positions,
        &mut normals,
        &mut uvs,
        &mut colors,
        &mut indices,
        [w, h, -l],
        [w, h, l],
        [w, 0.0, l],
        [w, 0.0, -l],
        [-1.0, 0.0, 0.0],
        [175, 165, 155, 255],
    );

    // Pillar / Column in center of room
    let pw = 600.0;
    let pz = -3000.0;
    append_quad(
        &mut positions,
        &mut normals,
        &mut uvs,
        &mut colors,
        &mut indices,
        [-pw, h, pz - pw],
        [pw, h, pz - pw],
        [pw, 0.0, pz - pw],
        [-pw, 0.0, pz - pw],
        [0.0, 0.0, -1.0],
        [220, 200, 180, 255],
    );
    append_quad(
        &mut positions,
        &mut normals,
        &mut uvs,
        &mut colors,
        &mut indices,
        [pw, h, pz + pw],
        [-pw, h, pz + pw],
        [-pw, 0.0, pz + pw],
        [pw, 0.0, pz + pw],
        [0.0, 0.0, 1.0],
        [220, 200, 180, 255],
    );
    append_quad(
        &mut positions,
        &mut normals,
        &mut uvs,
        &mut colors,
        &mut indices,
        [-pw, h, pz + pw],
        [-pw, h, pz - pw],
        [-pw, 0.0, pz - pw],
        [-pw, 0.0, pz + pw],
        [-1.0, 0.0, 0.0],
        [200, 180, 160, 255],
    );
    append_quad(
        &mut positions,
        &mut normals,
        &mut uvs,
        &mut colors,
        &mut indices,
        [pw, h, pz - pw],
        [pw, h, pz + pw],
        [pw, 0.0, pz + pw],
        [pw, 0.0, pz - pw],
        [1.0, 0.0, 0.0],
        [200, 180, 160, 255],
    );

    map.batches.push(MapBatch {
        group: BatchGroup::Opaque,
        material_index: 0,
        geo_id: 1,
        positions,
        uvs,
        colors,
        normals,
        indices,
    });

    map
}

/// Generate an extended outdoor synthetic fog calibration runway with distance markers.
pub fn create_synthetic_fog_runway_geometry() -> MapFile {
    let mut map = MapFile::default();
    map.materials.push(MapMaterial {
        mode: 1,
        texture_id: 0,
        color: 0xFF555555,
        overlay: 0,
        specularity: 0.2,
    });

    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    let mut colors = Vec::new();
    let mut indices = Vec::new();

    let w = 4000.0;
    let length = 120000.0;

    // Main runway floor
    append_quad(
        &mut positions,
        &mut normals,
        &mut uvs,
        &mut colors,
        &mut indices,
        [-w, 0.0, -5000.0],
        [w, 0.0, -5000.0],
        [w, 0.0, length],
        [-w, 0.0, length],
        [0.0, -1.0, 0.0],
        [70, 75, 80, 255],
    );

    let distance_markers = [5000.0, 10000.0, 25000.0, 50000.0, 75000.0, 100000.0];
    for &dist in &distance_markers {
        let m_thick = 300.0;
        let col = if dist == 50000.0 || dist == 100000.0 {
            [240, 220, 50, 255]
        } else {
            [220, 220, 220, 255]
        };

        append_quad(
            &mut positions,
            &mut normals,
            &mut uvs,
            &mut colors,
            &mut indices,
            [-w * 0.9, -2.0, dist - m_thick],
            [w * 0.9, -2.0, dist - m_thick],
            [w * 0.9, -2.0, dist + m_thick],
            [-w * 0.9, -2.0, dist + m_thick],
            [0.0, -1.0, 0.0],
            col,
        );

        for &side in &[-1.0f32, 1.0f32] {
            let px = side * (w + 800.0);
            let pz = dist;
            let ph = -3200.0;
            let pw = 150.0;

            append_quad(
                &mut positions,
                &mut normals,
                &mut uvs,
                &mut colors,
                &mut indices,
                [px - pw, ph, pz - pw],
                [px + pw, ph, pz - pw],
                [px + pw, 0.0, pz - pw],
                [px - pw, 0.0, pz - pw],
                [0.0, 0.0, -1.0],
                [180, 140, 60, 255],
            );
            append_quad(
                &mut positions,
                &mut normals,
                &mut uvs,
                &mut colors,
                &mut indices,
                [px + pw, ph, pz + pw],
                [px - pw, ph, pz + pw],
                [px - pw, 0.0, pz + pw],
                [px + pw, 0.0, pz + pw],
                [0.0, 0.0, 1.0],
                [180, 140, 60, 255],
            );
            let bw = 250.0;
            append_quad(
                &mut positions,
                &mut normals,
                &mut uvs,
                &mut colors,
                &mut indices,
                [px - bw, ph - bw * 2.0, pz - bw],
                [px + bw, ph - bw * 2.0, pz - bw],
                [px + bw, ph, pz - bw],
                [px - bw, ph, pz - bw],
                [0.0, 0.0, -1.0],
                [255, 230, 160, 255],
            );
        }
    }

    map.batches.push(MapBatch {
        group: BatchGroup::Opaque,
        material_index: 0,
        geo_id: 2,
        positions,
        uvs,
        colors,
        normals,
        indices,
    });

    map
}

/// Generate a calibrated neutral 3D Model Viewer pedestal, metric floor grid, and vertical height scale pole.
pub fn create_model_viewer_pedestal_geometry() -> MapFile {
    let mut map = MapFile::default();
    map.materials.push(MapMaterial {
        mode: 1,
        texture_id: 0,
        color: 0xFF666666,
        overlay: 0,
        specularity: 0.3,
    });

    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    let mut colors = Vec::new();
    let mut indices = Vec::new();

    let floor_ext = 3500.0;
    append_quad(
        &mut positions,
        &mut normals,
        &mut uvs,
        &mut colors,
        &mut indices,
        [-floor_ext, 4.0, -floor_ext],
        [floor_ext, 4.0, -floor_ext],
        [floor_ext, 4.0, floor_ext],
        [-floor_ext, 4.0, floor_ext],
        [0.0, -1.0, 0.0],
        [36, 38, 42, 255],
    );

    for step in -6..=6 {
        let coord = step as f32 * 500.0;
        let is_axis = step == 0;
        let is_meter = step % 2 == 0;
        let half_w = if is_axis {
            10.0
        } else if is_meter {
            6.0
        } else {
            3.0
        };
        let col_x = if is_axis {
            [180, 70, 70, 255]
        } else if is_meter {
            [85, 92, 102, 255]
        } else {
            [55, 60, 68, 255]
        };
        let col_z = if is_axis {
            [70, 110, 185, 255]
        } else if is_meter {
            [85, 92, 102, 255]
        } else {
            [55, 60, 68, 255]
        };

        append_quad(
            &mut positions,
            &mut normals,
            &mut uvs,
            &mut colors,
            &mut indices,
            [-3000.0, 1.5, coord - half_w],
            [3000.0, 1.5, coord - half_w],
            [3000.0, 1.5, coord + half_w],
            [-3000.0, 1.5, coord + half_w],
            [0.0, -1.0, 0.0],
            col_x,
        );
        append_quad(
            &mut positions,
            &mut normals,
            &mut uvs,
            &mut colors,
            &mut indices,
            [coord - half_w, 1.5, -3000.0],
            [coord + half_w, 1.5, -3000.0],
            [coord + half_w, 1.5, 3000.0],
            [coord - half_w, 1.5, 3000.0],
            [0.0, -1.0, 0.0],
            col_z,
        );
    }

    let segments = 16;
    let radius = 750.0f32;
    let rim_radius = 790.0f32;
    let ped_depth = 85.0f32;

    for i in 0..segments {
        let a0 = (i as f32 / segments as f32) * std::f32::consts::TAU;
        let a1 = ((i + 1) as f32 / segments as f32) * std::f32::consts::TAU;
        let (s0, c0) = a0.sin_cos();
        let (s1, c1) = a1.sin_cos();

        let top_col = if i % 2 == 0 {
            [68, 72, 80, 255]
        } else {
            [60, 64, 72, 255]
        };
        append_quad(
            &mut positions,
            &mut normals,
            &mut uvs,
            &mut colors,
            &mut indices,
            [0.0, 0.5, 0.0],
            [c0 * radius, 0.5, s0 * radius],
            [c1 * radius, 0.5, s1 * radius],
            [0.0, 0.5, 0.0],
            [0.0, -1.0, 0.0],
            top_col,
        );
        append_quad(
            &mut positions,
            &mut normals,
            &mut uvs,
            &mut colors,
            &mut indices,
            [c0 * radius, 0.5, s0 * radius],
            [c0 * rim_radius, 8.0, s0 * rim_radius],
            [c1 * rim_radius, 8.0, s1 * rim_radius],
            [c1 * radius, 0.5, s1 * radius],
            [0.0, -1.0, 0.0],
            [120, 125, 135, 255],
        );
        let nx = (c0 + c1) * 0.5;
        let nz = (s0 + s1) * 0.5;
        append_quad(
            &mut positions,
            &mut normals,
            &mut uvs,
            &mut colors,
            &mut indices,
            [c0 * rim_radius, 8.0, s0 * rim_radius],
            [c0 * rim_radius, ped_depth, s0 * rim_radius],
            [c1 * rim_radius, ped_depth, s1 * rim_radius],
            [c1 * rim_radius, 8.0, s1 * rim_radius],
            [nx, 0.0, nz],
            [48, 52, 58, 255],
        );
    }

    let post_x = -950.0;
    let post_z = -200.0;
    let post_w = 14.0;
    let post_h = -2200.0;

    append_quad(
        &mut positions,
        &mut normals,
        &mut uvs,
        &mut colors,
        &mut indices,
        [post_x - post_w, post_h, post_z],
        [post_x + post_w, post_h, post_z],
        [post_x + post_w, 0.0, post_z],
        [post_x - post_w, 0.0, post_z],
        [0.0, 0.0, 1.0],
        [140, 145, 155, 255],
    );

    let ticks: &[(f32, f32, [u8; 4])] = &[
        (-250.0, 50.0, [110, 115, 120, 255]),
        (-500.0, 95.0, [180, 185, 195, 255]),
        (-750.0, 50.0, [110, 115, 120, 255]),
        (-1000.0, 130.0, [240, 205, 55, 255]),
        (-1250.0, 50.0, [110, 115, 120, 255]),
        (-1500.0, 95.0, [180, 185, 195, 255]),
        (-1750.0, 65.0, [100, 210, 230, 255]),
        (-2000.0, 130.0, [240, 205, 55, 255]),
    ];

    for &(ty, width, col) in ticks {
        append_quad(
            &mut positions,
            &mut normals,
            &mut uvs,
            &mut colors,
            &mut indices,
            [post_x, ty - 6.0, post_z + 2.0],
            [post_x + width, ty - 6.0, post_z + 2.0],
            [post_x + width, ty + 6.0, post_z + 2.0],
            [post_x, ty + 6.0, post_z + 2.0],
            [0.0, 0.0, 1.0],
            col,
        );
    }

    map.batches.push(MapBatch {
        group: BatchGroup::Opaque,
        material_index: 0,
        geo_id: 3,
        positions,
        uvs,
        colors,
        normals,
        indices,
    });

    map
}
