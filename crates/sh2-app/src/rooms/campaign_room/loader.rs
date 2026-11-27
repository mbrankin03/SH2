//! Thin retail `.map`, `.cld`, `.cam`, texture, and fog asset loader for the unified
//! Campaign Game Loop (`SH2-112`).
//!
//! Consumes [`RoomLoadManifest`] produced by `sh2-core::campaign` without duplicating any
//! domain state machine logic inside `sh2-app` (`AGENTS.md` §1).

use crate::assets::{
    find_cld_file_for_room, find_map_file_for_room_for_quality, load_map_for_quality_from_roots,
    EnemyAssetSlot, MapTierAssets,
};
use crate::state::{ActiveMapSource, AppState};
use sh2_assets::formats::cld::parse_cld;
use sh2_core::campaign::RoomLoadManifest;
use sh2_core::collision::CollisionWorld;
use sh2_core::scenario::PlayableCharacter;
use sh2_renderer::fog::FogUniforms;
use sh2_ui::menu::TestRoomKind;

/// Loads the GPU `.map` geometry, single active texture tier, `.cld` collision mesh,
/// lazy-loaded enemy species, and atmospheric fog uniforms described by `manifest` into `AppState` (`SH2-181`).
///
/// For outdoor multi-tile stages (`manifest.is_outdoor`), seeds the initial tile into
/// `AppState::outdoor_tile_pool` and returns `(area, block_no)` so `OutdoorTileStreamer`
/// marks it immediately resident for that area (`SH2-183`). For indoor stages, clears the outdoor tile pool.
pub(crate) fn load_manifest_into_app_state(
    state: &mut AppState,
    manifest: &RoomLoadManifest,
) -> Option<(sh2_core::world::OutdoorArea, u16)> {
    let resolver = sh2_assets::AssetPathResolver::resolve();

    let active_tex_quality = state
        .menu_ctrl
        .options_menu
        .draft_config
        .video
        .texture_quality;

    let loaded_map = if let Some(active_map) = load_map_for_quality_from_roots(
        &manifest.map_rel_path,
        &manifest.room_id,
        active_tex_quality,
    ) {
        let active_assets = MapTierAssets::from_map(&state.render_ctx, active_map);
        state.gpu_map = active_assets.gpu_map;
        state.map_texture_bind_groups = active_assets.texture_bind_groups;
        tracing::info!(
            "[Campaign] Loaded Stage {} '{}' ({}, tier={:?}, enemies={}, pickups={})",
            manifest.stage_id,
            manifest.stage_name,
            manifest.map_rel_path,
            active_tex_quality,
            manifest.enemy_spawns.len(),
            manifest.room_pickups.len()
        );
        true
    } else {
        false
    };

    if !loaded_map {
        let fallback_room = fallback_test_room_for_stage(manifest.stage_id);
        let (fb_map, _fb_pos, _fb_yaw, fb_fog) =
            find_map_file_for_room_for_quality(fallback_room, active_tex_quality);
        let fb_assets = MapTierAssets::from_map(&state.render_ctx, fb_map);
        state.gpu_map = fb_assets.gpu_map;
        state.map_texture_bind_groups = fb_assets.texture_bind_groups;
        state.fog_uniforms = fb_fog;
    } else {
        let video = &state.menu_ctrl.options_menu.draft_config.video;
        let enable_sh2ee_world_fixes = state
            .menu_ctrl
            .options_menu
            .draft_config
            .game
            .enable_sh2ee_world_fixes;
        state.fog_uniforms = fog_for_campaign_manifest(
            manifest,
            video.fog_quality,
            video.volumetric_light,
            enable_sh2ee_world_fixes,
        );
    }

    // Load retail `.cld` collision world, falling back gracefully if absent in headless/CI builds.
    let parsed_cld = resolver
        .resolve_file(&manifest.cld_rel_path)
        .and_then(|p| std::fs::read(&p).ok())
        .and_then(|bytes| parse_cld(&bytes).ok());
    state.sim_core.collision = parsed_cld
        .as_ref()
        .map(CollisionWorld::from_cld)
        .unwrap_or_else(|| find_cld_file_for_room(fallback_test_room_for_stage(manifest.stage_id)));

    let seeded_outdoor_block = if manifest.is_outdoor {
        if let Some(area) = sh2_core::world::OutdoorArea::from_stage_id(manifest.stage_id) {
            let block_no =
                sh2_renderer::GpuMap::parse_base_room_id_from_map_name(&manifest.room_id);
            let tile_cld = parsed_cld.as_ref().map(CollisionWorld::from_cld);
            state.outdoor_tile_pool.seed_tile(
                area,
                block_no,
                std::sync::Arc::clone(&state.gpu_map),
                state.map_texture_bind_groups.clone(),
                tile_cld,
            );
            Some((area, block_no))
        } else {
            state.outdoor_tile_pool.clear();
            None
        }
    } else {
        state.outdoor_tile_pool.clear();
        None
    };

    state.render_ctx.update_fog(&state.fog_uniforms);
    state.active_maria = state.sim_core.scenario.character == PlayableCharacter::Maria;

    let ai_diff = state.sim_core.scenario.action_difficulty.into();
    state.combat_arena.ai.is_outdoor = manifest.is_outdoor;
    state
        .combat_arena
        .ai
        .spawn_from_manifest(&manifest.enemy_spawns, ai_diff);

    // Lazy-load ONLY the enemy species spawned in this room and evict unused species from
    // previous stages (`SH2-181`).
    let mut needed_slots = Vec::new();
    for enemy in &state.combat_arena.ai.enemies {
        if let Some(slot) = EnemyAssetSlot::from_species(enemy.species(), false) {
            if !needed_slots.contains(&slot) {
                needed_slots.push(slot);
            }
        }
    }
    state
        .enemy_assets
        .preload_for_slots(&state.render_ctx, &needed_slots, active_tex_quality);

    state.active_map_source = ActiveMapSource::Campaign;
    state.story_progression_room.cutaway_cache.clear();
    state.native_cutaway_surface = None;
    let ps2_room_id = sh2_core::fog::room_id_from_stage_code(&manifest.room_id);
    crate::rooms::sh2ee_glb_hooks::preload_room_glb_models(
        state,
        &resolver,
        ps2_room_id,
        sh2_core::DOG_ROOM_PRELOAD_EVENT_INDEX,
    );
    let prev_room_id = state
        .campaign_loaded_room
        .as_ref()
        .map(|(m, _)| m.room_id.clone())
        .unwrap_or_default();
    state.campaign_loaded_room = Some((manifest.clone(), active_tex_quality));
    state.apply_texture_quality(active_tex_quality);
    let persistent_blood = state
        .menu_ctrl
        .options_menu
        .draft_config
        .game
        .enable_persistent_blood_decals;
    state
        .blood_system
        .on_room_change(&prev_room_id, &manifest.room_id, persistent_blood);
    seeded_outdoor_block
}

fn fallback_test_room_for_stage(stage_id: u16) -> TestRoomKind {
    match stage_id {
        2 => TestRoomKind::RestroomStart,
        3 => TestRoomKind::ObservationDeck,
        _ => TestRoomKind::HotelBoilerRoom,
    }
}

fn fog_for_campaign_manifest(
    manifest: &RoomLoadManifest,
    fog_quality: sh2_core::config::FogQuality,
    volumetric_light: bool,
    enable_sh2ee_world_fixes: bool,
) -> FogUniforms {
    let ps2_room_id = sh2_core::fog::room_id_from_stage_code(&manifest.room_id);
    let eval = sh2_core::fog::evaluate_room_fog_with_fixes(
        &sh2_core::fog::RoomFogContext {
            room_id: ps2_room_id,
            ..Default::default()
        },
        enable_sh2ee_world_fixes,
    );
    FogUniforms::from_room_evaluation(&eval, fog_quality, volumetric_light)
}
