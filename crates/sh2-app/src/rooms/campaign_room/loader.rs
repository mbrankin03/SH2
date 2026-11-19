//! Thin retail `.map`, `.cld`, `.cam`, texture, and fog asset loader for the unified
//! Campaign Game Loop (`SH2-112`).
//!
//! Consumes [`RoomLoadManifest`] produced by `sh2-core::campaign` without duplicating any
//! domain state machine logic inside `sh2-app` (`AGENTS.md` §1).

use std::path::PathBuf;

use crate::assets::{
    build_map_texture_bind_groups, find_cld_file_for_room, find_map_file_for_room,
};
use crate::state::AppState;
use sh2_assets::formats::cld::parse_cld;
use sh2_assets::formats::TextureLoader;
use sh2_core::campaign::RoomLoadManifest;
use sh2_core::collision::CollisionWorld;
use sh2_core::scenario::PlayableCharacter;
use sh2_renderer::fog::FogUniforms;
use sh2_renderer::GpuMap;
use sh2_ui::menu::TestRoomKind;

/// Loads the GPU `.map` geometry, multi-tier textures (4K / SH2:EE / Vanilla), `.cld` collision
/// mesh, and atmospheric fog uniforms described by `manifest` into `AppState`.
pub(crate) fn load_manifest_into_app_state(state: &mut AppState, manifest: &RoomLoadManifest) {
    let resolver = sh2_assets::AssetPathResolver::resolve();
    let base_roots = [
        resolver.game_root.to_string_lossy().to_string(),
        "Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2".to_string(),
    ];

    let mut loaded_map = false;
    for base in &base_roots {
        let loader = TextureLoader::new(
            PathBuf::from(base).join("data"),
            Some(PathBuf::from(base).join("sh2e")),
        );
        if let Ok(hd_map) =
            loader.load_map_with_hd_overrides(&manifest.map_rel_path, &manifest.room_id)
        {
            state.gpu_map = GpuMap::from_map(&state.render_ctx.device, &hd_map);
            let hd_bgs = build_map_texture_bind_groups(&state.render_ctx, &hd_map);
            let sh2e_bgs = loader
                .load_sh2e_map(&manifest.map_rel_path, &manifest.room_id)
                .ok()
                .map(|m| build_map_texture_bind_groups(&state.render_ctx, &m))
                .unwrap_or_else(|| hd_bgs.clone());
            let van_bgs = loader
                .load_vanilla_map(&manifest.map_rel_path, &manifest.room_id)
                .ok()
                .map(|m| build_map_texture_bind_groups(&state.render_ctx, &m))
                .unwrap_or_else(|| sh2e_bgs.clone());
            state.map_hd_texture_bind_groups = hd_bgs;
            state.map_sh2e_texture_bind_groups = sh2e_bgs;
            state.map_vanilla_texture_bind_groups = van_bgs;
            loaded_map = true;
            tracing::info!(
                "[Campaign] Loaded Stage {} '{}' ({}, enemies={}, pickups={})",
                manifest.stage_id,
                manifest.stage_name,
                manifest.map_rel_path,
                manifest.enemy_spawns.len(),
                manifest.room_pickups.len()
            );
            break;
        }
    }

    if !loaded_map {
        let fallback_room = fallback_test_room_for_stage(manifest.stage_id);
        let (fb_map, _fb_pos, _fb_yaw, fb_fog) = find_map_file_for_room(fallback_room);
        state.gpu_map = GpuMap::from_map(&state.render_ctx.device, &fb_map);
        let bgs = build_map_texture_bind_groups(&state.render_ctx, &fb_map);
        state.map_hd_texture_bind_groups = bgs.clone();
        state.map_sh2e_texture_bind_groups = bgs.clone();
        state.map_vanilla_texture_bind_groups = bgs;
        state.fog_uniforms = fb_fog;
    } else {
        state.fog_uniforms = fog_for_campaign_manifest(manifest);
    }

    // Load retail `.cld` collision world, falling back gracefully if absent in headless/CI builds.
    state.sim_core.collision = resolver
        .resolve_file(&manifest.cld_rel_path)
        .and_then(|p| std::fs::read(&p).ok())
        .and_then(|bytes| parse_cld(&bytes).ok())
        .map(|cld| CollisionWorld::from_cld(&cld))
        .unwrap_or_else(|| find_cld_file_for_room(fallback_test_room_for_stage(manifest.stage_id)));

    state.render_ctx.update_fog(&state.fog_uniforms);
    state.active_maria = state.sim_core.scenario.character == PlayableCharacter::Maria;

    let active_tex_quality = state
        .menu_ctrl
        .options_menu
        .draft_config
        .video
        .texture_quality;
    state.apply_texture_quality(active_tex_quality);
}

fn fallback_test_room_for_stage(stage_id: u16) -> TestRoomKind {
    match stage_id {
        2 => TestRoomKind::RestroomStart,
        3 => TestRoomKind::ObservationDeck,
        _ => TestRoomKind::HotelBoilerRoom,
    }
}

fn fog_for_campaign_manifest(manifest: &RoomLoadManifest) -> FogUniforms {
    if manifest.is_outdoor {
        FogUniforms {
            fog_color: [0.58, 0.62, 0.66, 1.0],
            fog_near: 1_500.0,
            fog_far: 18_000.0,
            density: 0.00018,
            ..FogUniforms::default()
        }
    } else {
        FogUniforms {
            fog_color: [0.06, 0.07, 0.08, 1.0],
            fog_near: 800.0,
            fog_far: 9_500.0,
            density: 0.00032,
            ..FogUniforms::default()
        }
    }
}
