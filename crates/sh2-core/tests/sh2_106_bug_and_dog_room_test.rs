use sh2_assets::formats::{
    parse_glb, parse_mdl, GlbNodeLocalTransform, GLB_CHUNK_BIN, GLB_CHUNK_JSON, GLB_MAGIC,
    GLB_VERSION,
};
use sh2_assets::AssetPathResolver;
use sh2_core::config::{AppConfig, EnhancedPreset};
use sh2_core::{
    evaluate_roach_jiggle, CockroachDrawAction, CockroachSwarmState, DogRoomLeversRenderCmd,
    DogRoomLeversState, DOG_CUTSCENE_TIMER_HZ, DOG_MODEL_OFFSET_TABLE,
    DOG_ROOM_PRELOAD_EVENT_INDEX, MAX_COCKROACH_TIMERS, R_BUG_RM_ID, R_END_DOG_RM_ID,
    STAGE_END_DOG_ID, TY2_BODY_NUM_VERTICES, TY2_BODY_PRIM_COUNT, TY2_LIMB_PRIM_COUNT,
    TY3_BODY_NUM_VERTICES, TY3_BODY_PRIM_COUNT, TY3_LIMB_PRIM_COUNTS, TY3_SHADOW_NUM_VERTICES,
    TY3_SHADOW_PRIM_COUNT, VANILLA_ROACH_JIGGLE_POS_MAX, VANILLA_ROACH_JIGGLE_POS_MIN,
    VANILLA_ROACH_JIGGLE_YAW_MAX, VANILLA_ROACH_JIGGLE_YAW_MIN,
};

fn build_test_glb() -> Vec<u8> {
    // 3 vertices (36 bytes) + 3 u16 indices (6 bytes + 2 pad) + 2 timestamps (8 bytes) + 2 translations (24 bytes) = 76 bytes
    let mut bin = Vec::new();
    let positions: [[f32; 3]; 3] = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
    for p in positions {
        for c in p {
            bin.extend_from_slice(&c.to_le_bytes());
        }
    }
    for idx in [0u16, 1, 2] {
        bin.extend_from_slice(&idx.to_le_bytes());
    }
    bin.extend_from_slice(&[0u8; 2]); // 4-byte align
    for t in [0.0f32, 2.0f32] {
        bin.extend_from_slice(&t.to_le_bytes());
    }
    for tr in [[0.0f32, 0.0, 0.0], [10.0f32, 20.0, 30.0]] {
        for c in tr {
            bin.extend_from_slice(&c.to_le_bytes());
        }
    }

    let json = format!(
        r#"{{
  "asset": {{ "version": "2.0" }},
  "scene": 0,
  "scenes": [{{ "nodes": [0] }}],
  "nodes": [{{ "mesh": 0 }}],
  "meshes": [{{
    "primitives": [{{
      "attributes": {{ "POSITION": 0 }},
      "indices": 1
    }}]
  }}],
  "animations": [{{
    "channels": [{{ "sampler": 0, "target": {{ "node": 0, "path": "translation" }} }}],
    "samplers": [{{ "input": 2, "output": 3, "interpolation": "LINEAR" }}]
  }}],
  "buffers": [{{ "byteLength": {} }}],
  "bufferViews": [
    {{ "buffer": 0, "byteOffset": 0, "byteLength": 36 }},
    {{ "buffer": 0, "byteOffset": 36, "byteLength": 6 }},
    {{ "buffer": 0, "byteOffset": 44, "byteLength": 8 }},
    {{ "buffer": 0, "byteOffset": 52, "byteLength": 24 }}
  ],
  "accessors": [
    {{ "bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3" }},
    {{ "bufferView": 1, "componentType": 5123, "count": 3, "type": "SCALAR" }},
    {{ "bufferView": 2, "componentType": 5126, "count": 2, "type": "SCALAR" }},
    {{ "bufferView": 3, "componentType": 5126, "count": 2, "type": "VEC3" }}
  ]
}}"#,
        bin.len()
    );
    let mut json_bytes = json.into_bytes();
    while json_bytes.len() % 4 != 0 {
        json_bytes.push(b' ');
    }
    while bin.len() % 4 != 0 {
        bin.push(0);
    }

    let total_len = 12 + 8 + json_bytes.len() + 8 + bin.len();
    let mut out = Vec::with_capacity(total_len);
    out.extend_from_slice(&GLB_MAGIC);
    out.extend_from_slice(&GLB_VERSION.to_le_bytes());
    out.extend_from_slice(&(total_len as u32).to_le_bytes());
    out.extend_from_slice(&(json_bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(&GLB_CHUNK_JSON.to_le_bytes());
    out.extend_from_slice(&json_bytes);
    out.extend_from_slice(&(bin.len() as u32).to_le_bytes());
    out.extend_from_slice(&GLB_CHUNK_BIN.to_le_bytes());
    out.extend_from_slice(&bin);
    out
}

#[test]
fn cockroach_jiggle_and_draw_classification_respect_vanilla_ps2_preset() {
    let mut cfg = AppConfig::default();
    cfg.apply_enhanced_preset(EnhancedPreset::Ps2Authentic);
    assert!(!cfg.game.enable_sh2ee_world_fixes);

    // Vanilla PS2/PC jiggle bounds from sh2pc.exe FUN_004ae350 (0x004AE37C, 0x004AE3BB)
    let vanilla_jiggle = evaluate_roach_jiggle(cfg.game.enable_sh2ee_world_fixes, true);
    assert_eq!(
        vanilla_jiggle.pos_range,
        (VANILLA_ROACH_JIGGLE_POS_MIN, VANILLA_ROACH_JIGGLE_POS_MAX)
    );
    assert_eq!(
        vanilla_jiggle.yaw_range,
        (VANILLA_ROACH_JIGGLE_YAW_MIN, VANILLA_ROACH_JIGGLE_YAW_MAX)
    );

    // SH2:EE preset zeroes jiggle when ty3.glb is available
    cfg.apply_enhanced_preset(EnhancedPreset::Sh2ee);
    assert!(cfg.game.enable_sh2ee_world_fixes);
    let sh2ee_jiggle = evaluate_roach_jiggle(cfg.game.enable_sh2ee_world_fixes, true);
    assert_eq!(sh2ee_jiggle.pos_range, (0.0, 0.0));
    assert_eq!(sh2ee_jiggle.yaw_range, (0.0, 0.0));

    // Missing ty3.glb falls back to vanilla jiggle even when SH2:EE fixes are enabled
    let fallback_jiggle = evaluate_roach_jiggle(cfg.game.enable_sh2ee_world_fixes, false);
    assert_eq!(fallback_jiggle, vanilla_jiggle);

    // Draw call classification
    let mut swarm = CockroachSwarmState::default();
    assert_eq!(swarm.update_timers.len(), MAX_COCKROACH_TIMERS);
    assert!(swarm.update_timers.iter().all(|&t| (0.0..8.0).contains(&t)));

    swarm.begin_frame();
    // Vanilla mode never replaces or suppresses
    assert_eq!(
        swarm.classify_indexed_draw(
            R_BUG_RM_ID,
            true,
            TY3_BODY_NUM_VERTICES,
            TY3_BODY_PRIM_COUNT,
            true,
            false
        ),
        CockroachDrawAction::RenderVanilla
    );
    // SH2:EE mode replaces ty3 & ty2 bodies, suppresses ty3 shadow & limbs
    assert_eq!(
        swarm.classify_indexed_draw(
            R_BUG_RM_ID,
            true,
            TY3_BODY_NUM_VERTICES,
            TY3_BODY_PRIM_COUNT,
            true,
            true
        ),
        CockroachDrawAction::ReplaceWithTy3Glb { timer_slot: 0 }
    );
    assert_eq!(
        swarm.classify_indexed_draw(
            R_BUG_RM_ID,
            true,
            TY2_BODY_NUM_VERTICES,
            TY2_BODY_PRIM_COUNT,
            true,
            true
        ),
        CockroachDrawAction::ReplaceWithTy3Glb { timer_slot: 1 }
    );
    assert_eq!(
        swarm.classify_indexed_draw(
            R_BUG_RM_ID,
            true,
            TY3_SHADOW_NUM_VERTICES,
            TY3_SHADOW_PRIM_COUNT,
            true,
            true
        ),
        CockroachDrawAction::SuppressVanillaLimbOrShadow
    );
    assert_eq!(
        CockroachSwarmState::classify_limb_draw(
            R_BUG_RM_ID,
            true,
            0,
            TY3_LIMB_PRIM_COUNTS[0],
            false,
            true,
            true
        ),
        CockroachDrawAction::SuppressVanillaLimbOrShadow
    );
    assert_eq!(
        CockroachSwarmState::classify_limb_draw(
            R_BUG_RM_ID,
            true,
            0,
            TY2_LIMB_PRIM_COUNT,
            true,
            true,
            true
        ),
        CockroachDrawAction::SuppressVanillaLimbOrShadow
    );
    assert_eq!(
        CockroachSwarmState::classify_limb_draw(
            R_BUG_RM_ID,
            true,
            0,
            TY2_LIMB_PRIM_COUNT,
            false,
            true,
            true
        ),
        CockroachDrawAction::RenderVanilla
    );
}

#[test]
fn dog_room_levers_state_and_30hz_timer_respect_vanilla_ps2_preset() {
    assert_eq!(STAGE_END_DOG_ID, 52);
    assert!(DogRoomLeversState::should_preload(
        R_END_DOG_RM_ID,
        DOG_ROOM_PRELOAD_EVENT_INDEX,
        true,
        true
    ));
    assert!(!DogRoomLeversState::should_preload(
        R_END_DOG_RM_ID,
        DOG_ROOM_PRELOAD_EVENT_INDEX,
        true,
        false
    ));

    let mut state = DogRoomLeversState::default();
    // Vanilla PS2 mode: never draws mon.glb levers overlay
    assert_eq!(
        state.evaluate_draw(R_END_DOG_RM_ID, true, 30.0, true, false),
        DogRoomLeversRenderCmd::VanillaOnly
    );
    assert!(!state.active);

    // SH2:EE mode: initializes on first dog draw at 30 Hz
    let first = state.evaluate_draw(R_END_DOG_RM_ID, true, 30.0, true, true);
    assert_eq!(
        first,
        DogRoomLeversRenderCmd::DrawLeversOverlay {
            anim_timer_sec: 0.0,
            delta_sec: 0.0
        }
    );
    assert!(state.active);

    // Advance by 15 cutscene ticks = 0.5 seconds at 30 Hz
    let second = state.evaluate_draw(
        R_END_DOG_RM_ID,
        true,
        30.0 + (DOG_CUTSCENE_TIMER_HZ as f32) * 0.5,
        true,
        true,
    );
    match second {
        DogRoomLeversRenderCmd::DrawLeversOverlay {
            anim_timer_sec,
            delta_sec,
        } => {
            assert!((anim_timer_sec - 0.5).abs() < 1e-5);
            assert!((delta_sec - 0.5).abs() < 1e-5);
        }
        DogRoomLeversRenderCmd::VanillaOnly => panic!("expected DrawLeversOverlay"),
    }

    // Leaving R_END_DOG_RM resets active flag
    assert_eq!(
        state.evaluate_draw(R_BUG_RM_ID, true, 60.0, true, true),
        DogRoomLeversRenderCmd::VanillaOnly
    );
    assert!(!state.active);
}

#[test]
fn retail_bug_and_dog_room_mdl_assets_and_glb_parser() {
    let glb_bytes = build_test_glb();
    let glb = parse_glb(&glb_bytes).expect("parse synthetic GLB");
    assert_eq!(glb.meshes.len(), 1);
    assert_eq!(glb.meshes[0].primitives[0].vertices.len(), 3);
    assert_eq!(glb.meshes[0].primitives[0].indices, vec![0, 1, 2]);
    let locals = glb.evaluate_node_locals(1.0, true);
    match locals[0] {
        GlbNodeLocalTransform::Trs { translation, .. } => {
            assert!((translation[0] - 5.0).abs() < 1e-4);
            assert!((translation[1] - 10.0).abs() < 1e-4);
            assert!((translation[2] - 15.0).abs() < 1e-4);
        }
        GlbNodeLocalTransform::Matrix(_) => panic!("expected TRS local transform"),
    }

    let resolver = AssetPathResolver::resolve();
    let item_dir = resolver.data_dir.join("chr/item");
    if !item_dir.is_dir() {
        return;
    }

    for name in ["ty2.mdl", "ty3.mdl", "inu.mdl", "rinu.mdl"] {
        let path = item_dir.join(name);
        let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("read {name}: {e}"));
        let mdl = parse_mdl(&bytes).unwrap_or_else(|e| panic!("parse {name}: {e}"));
        assert!(mdl.bone_count > 0, "{name} should have bones");

        if name == "inu.mdl" {
            let mut tbl = [0i32; 20];
            for (i, slot) in tbl.iter_mut().enumerate() {
                let off = 0x40 + i * 4;
                *slot = i32::from_le_bytes(bytes[off..off + 4].try_into().unwrap());
            }
            assert!(DogRoomLeversState::is_dog_model_table(&tbl));
            assert_eq!(tbl, DOG_MODEL_OFFSET_TABLE);
        }
    }
}
