//! 2D HUD telemetry & hotkey legend rendering for the 3D Model & Bestiary Viewer (`SH2-36` / `SH2-37`).

use crate::state::AppState;
use sh2_debug::{TextureSourceMode, BESTIARY_CATALOG};

/// Renders the Bestiary Model Viewer telemetry header, equipment/pose status, and hotkey legend.
pub(super) fn render_model_viewer_hud(
    state: &mut AppState,
    encoder: &mut wgpu::CommandEncoder,
    surface_view: &wgpu::TextureView,
) {
    state.begin_hud_overlay();

    let white = [225, 225, 225, 255];
    let yellow = [245, 210, 65, 255];
    let cyan = [105, 215, 235, 255];
    let green = [120, 230, 150, 255];
    let muted = [165, 165, 170, 255];

    let entry = state.model_viewer.current_entry();
    let cat_str = entry.category.label();
    let total_models = BESTIARY_CATALOG.len();
    let cur_idx = state.model_viewer.selected_model_index + 1;

    state.menu_rasterizer.draw_text(
        "3D MODEL, ANIMATION & TEXTURE VIEWER [BESTIARY DEBUG]",
        24,
        18,
        2,
        yellow,
        true,
    );
    state.menu_rasterizer.draw_text(
        &format!(
            "MODEL [{}/{}] ({}) : {}",
            cur_idx, total_models, cat_str, entry.display_name
        ),
        24,
        46,
        2,
        white,
        true,
    );

    if let Some(ref c) = state.viewer_asset {
        let active_submeshes = c.gpu_mesh.primitives.iter().filter(|p| p.is_active).count();
        let active_indices: u32 = c
            .gpu_mesh
            .primitives
            .iter()
            .filter(|p| p.is_active)
            .map(|p| p.index_count)
            .sum();
        let active_tris = (active_indices / 3) as usize;
        let presets = sh2_renderer::discover_pose_presets(&c.model);
        let preset_idx = state
            .model_viewer
            .selected_pose_preset_index
            .min(presets.len().saturating_sub(1));
        let preset_label = presets
            .get(preset_idx)
            .map(|p| p.label)
            .unwrap_or("DEFAULT");

        state.menu_rasterizer.draw_text(
            &format!(
                "MESH STATS : {} BONES | {} VERTS | {} ACTIVE TRIS | {}/{} SUBMESHES | {} TEX | POSE [J] [{}/{}]: {}",
                c.model.bone_count,
                c.model.vertices.positions.len(),
                active_tris,
                active_submeshes,
                c.model.primitives.len(),
                c.model.textures.len(),
                preset_idx + 1,
                presets.len().max(1),
                preset_label
            ),
            24,
            72,
            1,
            cyan,
            true,
        );
    }

    let anm_name = if entry.anm_paths.is_empty() {
        "STATIC BIND POSE"
    } else {
        entry.anm_paths[state.model_viewer.selected_anm_index % entry.anm_paths.len()]
    };
    let play_status = if state.model_viewer.is_paused {
        "PAUSED"
    } else {
        "PLAYING"
    };
    let subclips = state.model_viewer.available_subclips();
    let subclip_info = if let Some(sc) = state.model_viewer.active_subclip() {
        let idx = state.model_viewer.selected_subclip_index.unwrap_or(0) + 1;
        format!(
            "SUB-CLIP [{}/{}] {} ({}..={} @ {:.2} FPS)",
            idx,
            subclips.len(),
            sc.label,
            sc.clip.start_frame,
            sc.clip.end_frame,
            sc.clip.fps
        )
    } else if !subclips.is_empty() {
        format!(
            "SUB-CLIP [ALL/{}] FULL TRACK (PRESS [C] FOR PS2 SUB-CLIPS)",
            subclips.len()
        )
    } else {
        "SUB-CLIP [FULL TRACK]".to_string()
    };
    state.menu_rasterizer.draw_text(
        &format!(
            "ANIM CLIP [{}/{}] : {} | {} | FRAME {:03.0}/{:03} | SPEED {:.1}X",
            state.model_viewer.selected_anm_index + 1,
            entry.anm_paths.len().max(1),
            anm_name,
            play_status,
            state.model_viewer.current_frame.floor(),
            state.model_viewer.total_frames,
            state.model_viewer.playback_speed
        ),
        24,
        92,
        1,
        green,
        true,
    );
    state
        .menu_rasterizer
        .draw_text(&subclip_info, 24, 110, 1, yellow, true);

    state.menu_rasterizer.draw_text(
        &format!(
            "SHADING : {} | OVERLAYS: BONES={} WIRE={}",
            state.model_viewer.render_mode.label(),
            if state.model_viewer.skeleton_overlay {
                "ON"
            } else {
                "OFF"
            },
            if state.model_viewer.wireframe_overlay {
                "ON"
            } else {
                "OFF"
            }
        ),
        24,
        128,
        1,
        white,
        true,
    );

    let tex_color = match state.model_viewer.texture_source {
        TextureSourceMode::Ai4kMaster => green,
        TextureSourceMode::EnhancedHd => cyan,
        TextureSourceMode::VanillaPs2 => yellow,
    };
    state.menu_rasterizer.draw_text(
        &format!(
            "TEXTURES [1: 4K AI | 2: PS2 | 3: SH2:EE | H: Cycle | U: A/B] : {} | LIGHT : {} | SPIN : {}",
            state.model_viewer.texture_source.label(),
            state.model_viewer.lighting_rig.label(),
            if state.model_viewer.turntable_enabled {
                "ON"
            } else {
                "OFF"
            }
        ),
        24,
        146,
        1,
        tex_color,
        true,
    );

    if let Some(ref c) = state.viewer_asset {
        if !c.model.clusters.is_empty() {
            let cls_summary = if let Some(ref cls) = c.cls {
                let active_count = c
                    .cluster_weights_scratch
                    .iter()
                    .filter(|&&w| w.abs() > 0.01)
                    .count();
                let max_w = c
                    .cluster_weights_scratch
                    .iter()
                    .fold(0.0f32, |acc, &w| acc.max(w.abs()));
                format!(
                    "FACIAL CLUSTERS (.CLS) : {} TARGETS | {} BLOCKS ({} FRAMES) | ACTIVE: {}/{} (PEAK WEIGHT: {:.2})",
                    c.model.clusters.len(),
                    cls.blocks.len(),
                    cls.total_frames(),
                    active_count,
                    c.cluster_weights_scratch.len(),
                    max_w
                )
            } else {
                format!(
                    "FACIAL CLUSTERS (.MDL) : {} MORPH TARGETS READY (SELECT DEMO .ANM WITH [M/Tab] FOR LIVE .CLS FACIAL SYNC)",
                    c.model.clusters.len()
                )
            };
            state
                .menu_rasterizer
                .draw_text(&cls_summary, 24, 164, 1, yellow, true);
        }
    }

    // Bottom hotkey legend
    state.menu_rasterizer.draw_text(
        "[N/P or PgDn/PgUp] Next/Prev Model  [M/Tab] Next .ANM  [C] Sub-Clip  [J] Pose/Equip  [Space] Play/Pause  [[ / ]] Step  [-/+] Speed  [R] Rev",
        24,
        666,
        1,
        muted,
        true,
    );
    state.menu_rasterizer.draw_text(
        "[V] Render Mode  [B] Skeleton  [G] Wireframe  [1/2/3 or H/U] Texture Quality  [L] Light  [T] Turntable  [WASD/Mouse] Orbit  [Q/E] Zoom  [Esc] Exit",
        24,
        688,
        1,
        muted,
        true,
    );

    state.finish_hud_overlay(encoder, surface_view);
}
