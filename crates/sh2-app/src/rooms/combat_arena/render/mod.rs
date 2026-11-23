//! 60 Hz simulation step, skeletal skinning, cutscene camera/lighting overrides, and 3D scene rendering
//! for 3D Stage Exploration & Combat Arena (`SH2-13` / `SH2-57` / `SH2-63`).

mod atmosphere;
mod cutscene;
mod scene;
mod simulation;
mod skinning;

use std::time::Duration;

use crate::state::AppState;

pub(crate) use atmosphere::update_atmosphere_fog_uniforms;

pub(super) fn tick_and_render_combat_arena_3d(
    state: &mut AppState,
    dt: Duration,
    _aspect: f32,
    is_paused: bool,
    is_in_game: bool,
) {
    // 1. Sync host keyboard/mouse axes + handoffs & advance 60 Hz simulation, combat, and BGM
    let ticks_processed = if !is_paused {
        super::camera_wiring::sync_host_input_and_handoffs(state);
        super::camera_wiring::sync_player_pose_and_visibility(state);
        simulation::step_simulation_and_audio(state, dt, is_in_game)
    } else {
        0
    };

    // 2. Advance active in-engine 3D cutscene (`DramaDemoPlayer`, `SH2-63` / `SH2-77`)
    let alpha = state.sim_core.interpolation_alpha();
    let (cutscene_camera_override, cutscene_light_override) =
        cutscene::step_cutscene_player(state, dt, is_paused);

    // 3. Sample player, cutscene actor, & enemy skeletal animations at sub-frame alpha,
    //    then feed animated `.mdl` head-bone pose & submesh visibility into `sim_core` / `gpu_mesh`
    let (player_pos, player_yaw) = state.sim_core.interpolated_player_pose(alpha);
    skinning::evaluate_player_skinning(
        state,
        alpha,
        is_paused,
        ticks_processed,
        player_pos,
        player_yaw,
    );
    super::camera_wiring::sync_player_pose_and_visibility(state);
    skinning::evaluate_cutscene_actors_skinning(state);
    let active_enemy_snapshot = skinning::evaluate_enemy_skinning(state, alpha);

    // 4. Upload active camera (PS2 Rail / OTS / First-Person / Cutscene), look-coupled flashlight, & fog
    super::camera_wiring::update_camera_uniforms(state, alpha, cutscene_camera_override);
    let cs_spot = cutscene_light_override
        .map(|l| cutscene::build_cutscene_spotlight_uniforms(state, l, player_pos));
    super::camera_wiring::update_spotlight_uniforms(state, alpha, cs_spot);
    update_atmosphere_fog_uniforms(state, alpha);

    // 5. Submit 3D map, skeletal meshes (plus full-body planar mirror reflections in RestroomStart), and optional collision wireframe overlay
    scene::submit_scene_draw(state, is_in_game, player_pos, active_enemy_snapshot);
}
