//! Runtime wiring for optional SH2:EE `.glb` 3D replacement models (`SH2-106`):
//! - `sh2e/model/ty3.glb` (`Patches/CockroachesReplacement.cpp` in `R_BUG_RM = 0x65`)
//! - `sh2e/model/mon.glb` (`Patches/DogRoomEnhanced.cpp` in `R_END_DOG_RM = 0xBD`).

use glam::Mat4;
use sh2_assets::formats::parse_glb;
use sh2_assets::AssetPathResolver;
use sh2_core::{
    evaluate_roach_jiggle, DogRoomLeversRenderCmd, DogRoomLeversState,
    DOG_ROOM_PRELOAD_EVENT_INDEX, R_BUG_RM_ID, R_END_DOG_RM_ID,
};
use sh2_renderer::GlbGpuModel;

use crate::state::AppState;

fn load_glb_if_present(
    state: &AppState,
    resolver: &AssetPathResolver,
    filename: &str,
    label: &str,
) -> Option<GlbGpuModel> {
    let path = resolver.resolve_sh2e_model(filename)?;
    let bytes = std::fs::read(&path).ok()?;
    let glb = parse_glb(&bytes).ok()?;
    Some(GlbGpuModel::from_glb(&state.render_ctx, glb, label))
}

/// Preloads optional `sh2e/model/ty3.glb` or `sh2e/model/mon.glb` on room load when
/// `enable_sh2ee_world_fixes` is enabled (`DogRoomEnhanced.cpp:142-147`).
pub(crate) fn preload_room_glb_models(
    state: &mut AppState,
    resolver: &AssetPathResolver,
    room_id: u32,
    event_index: u32,
) {
    let enable_fixes = state
        .menu_ctrl
        .options_menu
        .draft_config
        .game
        .enable_sh2ee_world_fixes;
    if !enable_fixes {
        return;
    }

    if room_id == R_BUG_RM_ID && state.sh2ee_ty3_glb.is_none() {
        state.sh2ee_ty3_glb = load_glb_if_present(state, resolver, "ty3.glb", "ty3_glb");
    }

    let has_mon = resolver.has_sh2e_model("mon.glb");
    if DogRoomLeversState::should_preload(room_id, event_index, has_mon, enable_fixes)
        && state.sh2ee_mon_glb.is_none()
    {
        state.sh2ee_mon_glb = load_glb_if_present(state, resolver, "mon.glb", "mon_glb");
    }
}

/// Updates and renders active SH2:EE `.glb` overlays for the current campaign room,
/// leaving strict 1:1 Vanilla PS2 mode (`enable_sh2ee_world_fixes == false`) untouched.
pub(crate) fn render_room_glb_replacements(state: &mut AppState, dt_sec: f32) {
    let enable_fixes = state
        .menu_ctrl
        .options_menu
        .draft_config
        .game
        .enable_sh2ee_world_fixes;

    let room_id = state
        .campaign_loaded_room
        .as_ref()
        .map(|(m, _)| sh2_core::fog::room_id_from_stage_code(&m.room_id))
        .unwrap_or(0);

    let _jiggle = evaluate_roach_jiggle(enable_fixes, state.sh2ee_ty3_glb.is_some());
    state.cockroach_swarm.begin_frame();
    let ty3_active = enable_fixes && room_id == R_BUG_RM_ID && state.sh2ee_ty3_glb.is_some();
    if ty3_active {
        let timer = state.cockroach_swarm.advance_timer(0, dt_sec);
        if let Some(ref ty3) = state.sh2ee_ty3_glb {
            ty3.update_skin_matrices(&state.render_ctx.queue, Mat4::IDENTITY, timer, true);
        }
    }

    let cutscene_ticks = state
        .drama_demo_session
        .as_ref()
        .map(|s| s.player.scene_frame)
        .unwrap_or(0.0);
    let mon_cmd = state.dog_room_levers.evaluate_draw(
        room_id,
        room_id == R_END_DOG_RM_ID,
        cutscene_ticks,
        state.sh2ee_mon_glb.is_some(),
        enable_fixes,
    );
    let mon_active = match mon_cmd {
        DogRoomLeversRenderCmd::DrawLeversOverlay { anim_timer_sec, .. } => {
            if let Some(ref mon) = state.sh2ee_mon_glb {
                mon.update_skin_matrices(
                    &state.render_ctx.queue,
                    Mat4::IDENTITY,
                    anim_timer_sec,
                    true,
                );
                true
            } else {
                false
            }
        }
        DogRoomLeversRenderCmd::VanillaOnly => false,
    };

    let _preload_hint = DOG_ROOM_PRELOAD_EVENT_INDEX;
    let mut overlays: Vec<&GlbGpuModel> = Vec::new();
    if ty3_active {
        if let Some(ref ty3) = state.sh2ee_ty3_glb {
            overlays.push(ty3);
        }
    }
    if mon_active {
        if let Some(ref mon) = state.sh2ee_mon_glb {
            overlays.push(mon);
        }
    }
    state.render_ctx.render_glb_overlays(&overlays);
}
