//! Thin retail `.map`, `.cld`, `.cam`, texture, and fog asset loader for the unified
//! Campaign Game Loop (`SH2-112`).
//!
//! Consumes [`RoomLoadManifest`] produced by `sh2-core::campaign` without duplicating any
//! domain state machine logic inside `sh2-app` (`AGENTS.md` §1).

use std::path::PathBuf;

use crate::assets::{find_cld_file_for_room, find_map_file_for_room, MapTierAssets};
use crate::state::AppState;
use sh2_assets::formats::cld::parse_cld;
use sh2_assets::formats::TextureLoader;
use sh2_core::campaign::RoomLoadManifest;
use sh2_core::collision::CollisionWorld;
use sh2_core::scenario::PlayableCharacter;
use sh2_renderer::fog::FogUniforms;
use sh2_ui::menu::TestRoomKind;

/// Loads the GPU `.map` geometry, multi-tier textures (4K / SH2:EE / Vanilla), `.cld` collision
/// mesh, and atmospheric fog uniforms described by `manifest` into `AppState`.
pub(crate) fn load_manifest_into_app_state(state: &mut AppState, manifest: &RoomLoadManifest) {
    let resolver = sh2_assets::AssetPathResolver::resolve();
    let base_roots = [
        resolver.game_root.to_string_lossy().to_string(),
        "Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2".to_string(),
    ];

    let active_tex_quality = state
        .menu_ctrl
        .options_menu
        .draft_config
        .video
        .texture_quality;

    let mut loaded_map = false;
    for base in &base_roots {
        let loader = TextureLoader::new(
            PathBuf::from(base).join("data"),
            Some(PathBuf::from(base).join("sh2e")),
        );
        // Load ONLY the active `WorldTextureQuality` tier synchronously to avoid parsing the `.map`
        // and decoding/uploading all 3 full texture tiers (including 4K AI Master PNGs) on every
        // room transition.
        let (prefer_sh2ee, apply_4k) = match active_tex_quality {
            sh2_core::config::WorldTextureQuality::Ai4kMaster => (true, true),
            sh2_core::config::WorldTextureQuality::Sh2eeHd => (true, false),
            sh2_core::config::WorldTextureQuality::OriginalPs2 => (false, false),
        };
        let loaded = loader
            .load_map_for_enhancement(
                &manifest.map_rel_path,
                &manifest.room_id,
                prefer_sh2ee,
                apply_4k,
            )
            .or_else(|err| match active_tex_quality {
                sh2_core::config::WorldTextureQuality::Sh2eeHd => {
                    loader.load_map_with_hd_overrides(&manifest.map_rel_path, &manifest.room_id)
                }
                sh2_core::config::WorldTextureQuality::OriginalPs2 => {
                    loader.load_sh2e_map(&manifest.map_rel_path, &manifest.room_id)
                }
                sh2_core::config::WorldTextureQuality::Ai4kMaster => Err(err),
            });
        if let Ok(active_map) = loaded {
            let active_assets = MapTierAssets::from_map(&state.render_ctx, &active_map);
            state.map_hd_assets = active_assets.clone();
            state.map_sh2e_assets = match active_tex_quality {
                sh2_core::config::WorldTextureQuality::Sh2eeHd => Some(active_assets.clone()),
                _ => None,
            };
            state.map_vanilla_assets = match active_tex_quality {
                sh2_core::config::WorldTextureQuality::OriginalPs2 => Some(active_assets),
                _ => None,
            };
            loaded_map = true;
            tracing::info!(
                "[Campaign] Loaded Stage {} '{}' ({}, tier={:?}, enemies={}, pickups={})",
                manifest.stage_id,
                manifest.stage_name,
                manifest.map_rel_path,
                active_tex_quality,
                manifest.enemy_spawns.len(),
                manifest.room_pickups.len()
            );
            break;
        }
    }

    if !loaded_map {
        let fallback_room = fallback_test_room_for_stage(manifest.stage_id);
        let (fb_map, _fb_pos, _fb_yaw, fb_fog) = find_map_file_for_room(fallback_room);
        let fb_assets = MapTierAssets::from_map(&state.render_ctx, &fb_map);
        state.map_hd_assets = fb_assets.clone();
        state.map_sh2e_assets = Some(fb_assets.clone());
        state.map_vanilla_assets = Some(fb_assets);
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
    state.sim_core.collision = resolver
        .resolve_file(&manifest.cld_rel_path)
        .and_then(|p| std::fs::read(&p).ok())
        .and_then(|bytes| parse_cld(&bytes).ok())
        .map(|cld| CollisionWorld::from_cld(&cld))
        .unwrap_or_else(|| find_cld_file_for_room(fallback_test_room_for_stage(manifest.stage_id)));

    state.render_ctx.update_fog(&state.fog_uniforms);
    state.active_maria = state.sim_core.scenario.character == PlayableCharacter::Maria;

    let ai_diff = state.sim_core.scenario.action_difficulty.into();
    state.combat_arena.ai.is_outdoor = manifest.is_outdoor;
    state
        .combat_arena
        .ai
        .spawn_from_manifest(&manifest.enemy_spawns, ai_diff);

    state.apply_texture_quality(active_tex_quality);
    state.campaign_loaded_room = Some((manifest.clone(), active_tex_quality));
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
