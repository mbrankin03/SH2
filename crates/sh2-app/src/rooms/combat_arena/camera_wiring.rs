//! Thin camera, skeletal head-bone anchor, look-coupled flashlight, and submesh visibility
//! wiring between `AppState`, `SimulationCore`, and `RenderContext` (`SH2-93`).

use crate::state::AppState;
use sh2_core::modern_camera::{
    ExternalCameraHandoff, PlayerMeshVisibility, STANDARD_NEAR_CLIP_UNITS,
};
use sh2_core::scenario::PlayableCharacter;
use sh2_core::CutsceneCameraSnapshot;
use sh2_renderer::light::SpotlightUniforms;
use sh2_ui::menu::TestRoomKind;

/// Synchronizes held desktop keyboard/mouse axes (`WASD`, `IJKL`, `Shift`, `R`/Right-Click) and
/// `.dds` cutscene / 2D cutaway handoffs into `state.sim_core` prior to 60 Hz stepping (`SH2-93`).
pub(crate) fn sync_host_input_and_handoffs(state: &mut AppState) {
    let cutscene_locked = state
        .drama_demo_session
        .as_ref()
        .is_some_and(|s| s.player.locks_gameplay_input());
    let combat_locked = state.active_test_room == TestRoomKind::CombatArena
        && state.combat_arena.player.mode != sh2_core::combat::CombatMode::Free;

    let (move_x, move_y) = if cutscene_locked || combat_locked {
        (0.0, 0.0)
    } else {
        (
            (state.key_right as i8 - state.key_left as i8) as f32,
            (state.key_forward as i8 - state.key_backward as i8) as f32,
        )
    };
    let (look_x, look_y) = if cutscene_locked {
        (0.0, 0.0)
    } else {
        (
            (state.key_look_right as i8 - state.key_look_left as i8) as f32,
            (state.key_look_up as i8 - state.key_look_down as i8) as f32,
        )
    };

    state.sim_core.scenario.character = if state.active_maria {
        PlayableCharacter::Maria
    } else {
        PlayableCharacter::James
    };
    state.sim_core.input.set_host_keyboard_state(
        move_x,
        move_y,
        look_x,
        look_y,
        state.key_run_held && !cutscene_locked,
        state.key_ready_held && !cutscene_locked,
    );
    state
        .sim_core
        .sync_cutscene_handoff(state.drama_demo_session.as_ref().map(|s| &s.player));
    let cutaway_clone = state.sim_core.scenario.cutaway_overlay.clone();
    state.sim_core.sync_cutaway_handoff(&cutaway_clone);
}

/// Feeds evaluated model-space skeletal bone matrices (`James` / `Maria`) into `SimulationCore`
/// for First-Person head-bone eye anchoring and updates foreground `PlayerMeshVisibility` (`SH2-93`).
pub(crate) fn sync_player_pose_and_visibility(state: &mut AppState) {
    let visibility = state.sim_core.player_mesh_visibility();
    if let Some(ref mut jms) = state.character {
        let vis = if state.active_maria && state.maria.is_some() {
            PlayerMeshVisibility::FullBody
        } else {
            visibility
        };
        jms.gpu_mesh.set_player_visibility(vis);
    }
    if let Some(ref mut mar) = state.maria {
        let vis = if state.active_maria {
            visibility
        } else {
            PlayerMeshVisibility::FullBody
        };
        mar.gpu_mesh.set_player_visibility(vis);
    }

    let active_char = if state.active_maria {
        state.maria.as_ref().or(state.character.as_ref())
    } else {
        state.character.as_ref().or(state.maria.as_ref())
    };
    let pose_slice = active_char.map(|c| {
        if !c.pose_scratch.is_empty() {
            c.pose_scratch.as_slice()
        } else {
            c.model.initial_matrices.as_slice()
        }
    });
    state.sim_core.set_player_skeleton_pose(pose_slice);
}

/// Uploads the active sub-frame view/projection matrices, FOV, eye position, and perspective
/// near-clip (`8.0` FPS vs `25.0` Rail/OTS) to `RenderContext` (`SH2-93`).
pub(crate) fn update_camera_uniforms(
    state: &mut AppState,
    alpha: f32,
    cutscene_camera_override: Option<CutsceneCameraSnapshot>,
) {
    let (view, fov, cam_pos, near_clip) = match cutscene_camera_override {
        Some(cs_cam)
            if matches!(
                state.sim_core.modern_camera.handoff,
                ExternalCameraHandoff::CutsceneLocked { .. }
            ) =>
        {
            (
                cs_cam.view_matrix(),
                cs_cam.fov_y_radians,
                cs_cam.eye,
                STANDARD_NEAR_CLIP_UNITS,
            )
        }
        _ => (
            state.sim_core.active_view_matrix(alpha),
            state.sim_core.active_fov_y(alpha),
            state.sim_core.active_camera_eye(alpha),
            state.sim_core.active_near_clip(alpha),
        ),
    };
    let (proj, inv_proj) =
        state
            .render_ctx
            .perspective_projection(fov, near_clip, crate::rooms::DEFAULT_CAMERA_Z_FAR);
    state
        .render_ctx
        .update_camera(proj * view, inv_proj, cam_pos);
}

/// Uploads the active look-coupled pocket/eye flashlight ray (or `.dds` cutscene spotlight override)
/// into `RenderContext` (`SH2-93`).
pub(crate) fn update_spotlight_uniforms(
    state: &mut AppState,
    alpha: f32,
    cutscene_spotlight_override: Option<SpotlightUniforms>,
) {
    let (spot_pos, light_dir) = state.sim_core.interpolated_flashlight_ray(alpha);
    let active_demo = state
        .drama_demo_session
        .as_ref()
        .filter(|s| s.player.locks_gameplay_input());
    let ambient_tint = active_demo
        .map(|s| s.player.atmosphere.ambient_tint_rgba)
        .unwrap_or([1.0, 1.0, 1.0, 1.0]);

    let spot_uniforms = if active_demo.is_some_and(|s| s.player.closet_fix.flashlight_suppressed) {
        SpotlightUniforms {
            position: [spot_pos.x, spot_pos.y, spot_pos.z, 0.0],
            direction_inner: [light_dir.x, light_dir.y, light_dir.z, 0.90],
            color_outer: [0.0, 0.0, 0.0, 0.0],
            ambient_tint,
        }
    } else if let Some(cs_spot) = cutscene_spotlight_override {
        cs_spot
    } else if active_demo.is_some_and(|s| s.player.info.stage_name == "er01")
        || (state.active_test_room == TestRoomKind::RestroomStart
            && !state.sim_core.scenario.flashlight_on)
    {
        SpotlightUniforms {
            position: [-19200.0, -1950.0, 20429.5, 1.0],
            direction_inner: [-0.65, 0.76, 0.0, 0.35],
            color_outer: [1.85, 1.90, 1.80, -0.25],
            ambient_tint,
        }
    } else if let Some(session) = active_demo {
        let key_pos = session.player.camera.eye + glam::Vec3::new(0.0, -600.0, 0.0);
        let safe_dir = (session.player.camera.interest - key_pos)
            .normalize_or(glam::Vec3::new(0.0, 0.5, 0.86));
        SpotlightUniforms {
            position: [key_pos.x, key_pos.y, key_pos.z, 1.0],
            direction_inner: [safe_dir.x, safe_dir.y, safe_dir.z, 0.25],
            color_outer: [1.75, 1.72, 1.68, -0.50],
            ambient_tint,
        }
    } else {
        let rgb = if state.sim_core.scenario.flashlight_on {
            [2.5, 2.3, 1.8]
        } else {
            [0.0, 0.0, 0.0]
        };
        SpotlightUniforms {
            position: [spot_pos.x, spot_pos.y, spot_pos.z, 1.0],
            direction_inner: [light_dir.x, light_dir.y, light_dir.z, 0.90],
            color_outer: [rgb[0], rgb[1], rgb[2], 0.70],
            ambient_tint,
        }
    };
    state.render_ctx.update_spotlight(&spot_uniforms);
}
