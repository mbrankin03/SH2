//! Retail-asset integration tests for `SH2-78`:
//! - Dynamic map part switches (`stageparts` + `GpuMap` sub-mesh visibility filtering)
//! - Animated stage-placed models (`stagemdl` + `data/chr/item/org.mdl`)
//! - Multi-tile outdoor `.map` background streaming (`OutdoorTileStreamer` + `GpuOutdoorTilePool`).

use std::time::{Duration, Instant};

use sh2_assets::formats::map::parse_map;
use sh2_assets::formats::mdls::parse_mdl;
use sh2_assets::AssetPathResolver;
use sh2_core::config::OutdoorStreamingMode;
use sh2_core::events::stagemdl::{
    StageModelManager, StageModelTickInput, MODEL_KIND_HOTEL_MUSIC_BOX,
};
use sh2_core::events::stageparts::{
    stageparts_tick, StagePartsContext, StagePartsState, FLAG_WOODSIDE_GATE_UNLOCKED,
};
use sh2_core::events::EventFlags;
use sh2_core::world::OutdoorTileStreamer;
use sh2_renderer::{GpuMap, GpuOutdoorTilePool, RenderContext, RendererConfig};

#[test]
fn test_retail_cb46_and_cb29_gpu_map_stageparts_visibility_filtering() {
    let resolver = AssetPathResolver::resolve();
    let cb29_path = resolver.data_dir.join("bg/cb/cb29.map");
    let cb46_path = resolver.data_dir.join("bg/cb/cb46.map");
    if !cb29_path.exists() || !cb46_path.exists() {
        return;
    }

    let Some(ctx) = RenderContext::new_headless(RendererConfig::default()) else {
        return;
    };

    // 1. Town East traffic light tile cb29.map (room 0x1d = 29)
    let cb29_bytes = std::fs::read(&cb29_path).expect("read cb29.map");
    let cb29_map = parse_map(&cb29_bytes, "cb29").expect("parse cb29.map");
    let mut cb29_gpu = GpuMap::from_map_with_room_id(&ctx.device, &cb29_map, 0x1d);
    let total_cb29_batches = cb29_gpu.batches.len();
    assert!(total_cb29_batches > 0);
    assert!(
        cb29_gpu.batches.iter().any(|b| b.variant_index > 0),
        "cb29.map must contain switchable traffic light variant batches"
    );

    let mut sp_state = StagePartsState::default();
    let mut flags = EventFlags::new();
    let out = stageparts_tick(
        &mut sp_state,
        &StagePartsContext {
            stage_id: 5,
            room_id: 0x1d,
            cutscene_id: 0,
            dt_secs: 0.0,
            flags: &flags,
            has_blue_gem: false,
            enable_sh2ee_fixes: true,
        },
    );
    cb29_gpu.set_visibility_filter(Some(out.visibility.clone()));
    let visible_cb29 = cb29_gpu.visible_batch_count();
    assert!(
        visible_cb29 > 0 && visible_cb29 < total_cb29_batches,
        "Expected traffic light filter to hide inactive signal bulbs ({visible_cb29}/{total_cb29_batches})"
    );

    // 2. Wood Side gate tile cb46.map (room 0x2e = 46) — SH2:EE FixTownGateEvents
    let cb46_bytes = std::fs::read(&cb46_path).expect("read cb46.map");
    let cb46_map = parse_map(&cb46_bytes, "cb46").expect("parse cb46.map");
    let mut cb46_gpu = GpuMap::from_map_with_room_id(&ctx.device, &cb46_map, 0x2e);

    flags.set(FLAG_WOODSIDE_GATE_UNLOCKED, true);
    let out_open = stageparts_tick(
        &mut sp_state,
        &StagePartsContext {
            stage_id: 5,
            room_id: 0x2e,
            cutscene_id: 0,
            dt_secs: 0.0,
            flags: &flags,
            has_blue_gem: false,
            enable_sh2ee_fixes: true,
        },
    );
    cb46_gpu.set_visibility_filter(Some(out_open.visibility));

    // Broken lock variant (variant_index == 2 -> bit 1) must be hidden after Flag 251 with SH2:EE fix.
    for batch in &cb46_gpu.batches {
        if batch.variant_index == 2 {
            assert!(!cb46_gpu.is_batch_visible(batch));
        }
    }
}

#[test]
fn test_retail_music_box_org_mdl_matches_stagemdl_figurine_bones() {
    let resolver = AssetPathResolver::resolve();
    let org_path = resolver.data_dir.join("chr/item/org.mdl");
    if !org_path.exists() {
        return;
    }

    let bytes = std::fs::read(&org_path).expect("read org.mdl");
    let model = parse_mdl(&bytes).expect("parse org.mdl");
    // org.mdl skeleton must include figurine bones 6, 7, 8 cited from sh2pc.exe 0x0057b646..0x0057b713.
    assert!(
        model.bone_count > 8,
        "org.mdl skeleton expected > 8 bones, found {}",
        model.bone_count
    );

    let mut mgr = StageModelManager::new();
    let flags = EventFlags::new();
    mgr.start_music_box(8.0);
    let out = mgr.tick(&StageModelTickInput {
        stage_id: 40,
        dt_secs: 0.5,
        flags: &flags,
        enable_sh2ee_fixes: true,
        sixty_fps_mode: true,
    });
    assert_eq!(out.instances[0].model_kind_id, MODEL_KIND_HOTEL_MUSIC_BOX);
    for ov in &out.instances[0].bone_overrides {
        assert!((ov.bone_index as usize) < model.bone_count);
    }
}

#[test]
fn test_retail_outdoor_tile_pool_streams_ps2_2x2_tiles_on_background_thread() {
    let resolver = AssetPathResolver::resolve();
    if !resolver.data_dir.join("bg/cb/cb28.map").exists() {
        return;
    }

    let Some(ctx) = RenderContext::new_headless(RendererConfig::default()) else {
        return;
    };
    let mut streamer = OutdoorTileStreamer::new(OutdoorStreamingMode::ClassicPs2LoadBg2x2);
    let mut pool = GpuOutdoorTilePool::new(resolver.data_dir.clone());

    let plan = streamer.plan(5, -48_000.0, -8_000.0);
    assert_eq!(plan.priority_gb_cam, Some("bg/cb/cbGB.cam"));
    assert!(resolver.data_dir.join("bg/cb/cbGB.cam").exists());

    let queued = pool.apply_plan(&plan);
    assert!(queued > 0);

    let deadline = Instant::now() + Duration::from_secs(5);
    while pool.resident_count() < queued && Instant::now() < deadline {
        for block_no in pool.poll_uploads(&ctx.device, 4) {
            streamer.mark_resident(block_no);
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    assert_eq!(pool.resident_count(), queued);
    assert!(pool.get(28).is_some(), "cb28 must be resident in GPU pool");
    assert!(pool.total_gpu_bytes() > 0);
}
