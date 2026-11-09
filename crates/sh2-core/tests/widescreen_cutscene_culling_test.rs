//! Integration and unit tests for `SH2-77`:
//! - Aspect-aware cutscene FOV (`4:3`, `16:9`, `20:9`, `21:9`, `32:9`) across [`CutsceneWidescreenMode`] variants.
//! - Smart off-screen `4:3` parked actor frustum culling at `16:9`, `21:9`, and `32:9`.
//! - Scene-specific widescreen/ultrawide fixes:
//!   - `ClosetCutsceneFix` (`sankaku1` / `wp_closet`, `demo_no == 14`, `CS_APT_RPT_CLOSET`)
//!   - `PistonRoomFix` (`papa_agl` / `ug_angela`, `demo_no == 68`, `CS_PS_ANGELA_BOSS_FINISH`)
//!   - `FixFinalBossRoom` (`R_FINAL_BOSS_RM == 0xBB` / `ru73`)
//!   - `ObservationDeckFogFix` (`ob01` / `tenbou`, `demo_no == 2`) & `MeetingMariaCutsceneFog` (`deai`, `demo_no == 22`)
//! - `DisableCutsceneBorders = Auto` & `SetBlackPillarBoxes` 2D/map clipping on `21:9` and `32:9`.

use glam::Vec3;
use sh2_assets::formats::demo::{
    parse_demo_dds, DemoCameraOp, DemoCharOp, DemoDdsFile, DemoDdsHeader, DemoFrameRecord,
    DemoKeyframeState,
};
use sh2_core::{
    evaluate_meeting_maria_fog_counters, find_drama_demo_by_id,
    observation_deck_border_alpha_factor, observation_deck_fog_bounds_x,
    observation_deck_wrap_particle_x, ClosetCutsceneFixState, CutsceneBorderBars,
    CutsceneBorderMode, CutsceneWidescreenMode, DramaDemoPlayInfo, DramaDemoPlayer,
    FinalBossRoomFixState, OverlaySpriteQuad, PillarboxViewportLayout, PistonRoomFixState,
    AUTHORED_4X3_ASPECT, CLOSET_BLURRED_BARS_FIXED_DESPAWN_FRAME, CLOSET_BLURRED_BARS_HIDE_CAM_X,
    CLOSET_DISABLE_FLASHLIGHT_CAM_X, CLOSET_MANNEQUIN_FIXED_DESPAWN_FRAME,
    CS_APT_RPT_CLOSET_DEMO_NO, CS_PS_ANGELA_BOSS_FINISH_DEMO_NO, CS_TOWN_WEST_MARIA_MEET_DEMO_NO,
    FINAL_BOSS_EXTENDED_DRAW_DISTANCE, FINAL_BOSS_LOWERED_SPAWN_Y, PISTON_DOORWAY_PROP_OFFSET,
    PISTON_ROOM_DOOR_OPEN_CAM_X, R_FINAL_BOSS_RM_ID, STANDARD_16X9_ASPECT,
    SUPER_ULTRAWIDE_32X9_ASPECT, ULTRAWIDE_21X9_ASPECT,
};

#[test]
fn test_aspect_aware_cutscene_fov_4x3_16x9_20x9_21x9_32x9() {
    let state = DemoKeyframeState {
        camera_plane: 320.0,
        ..Default::default()
    };

    // 1. UltrawideHorPlusSmartCull preserves the exact authored vertical FOV across 4:3, 16:9, 20:9, 21:9, 32:9
    let fov_4_3 = state.vertical_fov_radians(
        1.0,
        AUTHORED_4X3_ASPECT,
        CutsceneWidescreenMode::UltrawideHorPlusSmartCull,
    );
    let fov_16_9 = state.vertical_fov_radians(
        1.0,
        STANDARD_16X9_ASPECT,
        CutsceneWidescreenMode::UltrawideHorPlusSmartCull,
    );
    let fov_20_9 = state.vertical_fov_radians(
        1.0,
        20.0 / 9.0,
        CutsceneWidescreenMode::UltrawideHorPlusSmartCull,
    );
    let fov_21_9 = state.vertical_fov_radians(
        1.0,
        ULTRAWIDE_21X9_ASPECT,
        CutsceneWidescreenMode::UltrawideHorPlusSmartCull,
    );
    let fov_32_9 = state.vertical_fov_radians(
        1.0,
        SUPER_ULTRAWIDE_32X9_ASPECT,
        CutsceneWidescreenMode::UltrawideHorPlusSmartCull,
    );
    assert!((fov_4_3 - fov_16_9).abs() < 1e-6);
    assert!((fov_4_3 - fov_20_9).abs() < 1e-6);
    assert!((fov_4_3 - fov_21_9).abs() < 1e-6);
    assert!((fov_4_3 - fov_32_9).abs() < 1e-6);

    // 2. Sh2eeZoom16x9 scales scr_z smoothly up to 1.125x at 16:9 and clamps at >= 16:9 (20:9, 21:9, 32:9)
    let zoom_4_3 = state.vertical_fov_radians(
        1.0,
        AUTHORED_4X3_ASPECT,
        CutsceneWidescreenMode::Sh2eeZoom16x9,
    );
    let zoom_16_9 = state.vertical_fov_radians(
        1.0,
        STANDARD_16X9_ASPECT,
        CutsceneWidescreenMode::Sh2eeZoom16x9,
    );
    let zoom_20_9 =
        state.vertical_fov_radians(1.0, 20.0 / 9.0, CutsceneWidescreenMode::Sh2eeZoom16x9);
    let zoom_21_9 = state.vertical_fov_radians(
        1.0,
        ULTRAWIDE_21X9_ASPECT,
        CutsceneWidescreenMode::Sh2eeZoom16x9,
    );
    assert!((zoom_4_3 - fov_4_3).abs() < 1e-6);
    assert!(
        zoom_16_9 < zoom_4_3,
        "1.125x scr_z zoom at 16:9 must narrow vertical FOV slightly"
    );
    assert!((zoom_20_9 - zoom_16_9).abs() < 1e-6);
    assert!((zoom_21_9 - zoom_16_9).abs() < 1e-6);
}

#[test]
fn test_smart_offscreen_4x3_parked_actor_frustum_culling_at_16x9_21x9_32x9() {
    // Camera at origin (0, -1000, 0) looking along +Z (0, -1000, 2000), plane = 300.0
    // At Z = 2000, half_tan_y = 224 / (300 * 1.14702) = 0.65096 -> half_x_tan = 0.65096 * (4/3) = 0.86795
    // So the 4:3 horizontal frustum edge at Z = 2000 is X = ±1735.9 mm.
    // - Actor 0 (`hhh_jms_pos`): centered at X = 0, Z = 2000 (inside 4:3 cone) -> ALWAYS VISIBLE
    // - Actor 1 (`mkn_1_pos`): stationary/parked at X = 2350, Z = 2000 (outside 4:3 cone, inside 16:9/21:9/32:9 margin) -> CULLED
    // - Actor 2 (`agl_pos`): actively walking into the 4:3 cone from X = 2120 -> X = 2060 at Z = 2000 -> VISIBLE (entering)
    let script = DemoDdsFile {
        header: DemoDdsHeader {
            version_a: 0.61,
            version_b: 0.61,
            total_frames: 30,
            total_objects: 4,
            point_light_count: 0,
            spot_light_count: 0,
            inf_light_count: 0,
            character_names: vec![
                "hhh_jms_pos".to_string(),
                "mkn_1_pos".to_string(),
                "agl_pos".to_string(),
            ],
        },
        records: vec![
            DemoFrameRecord {
                frame: 0,
                is_cut: true,
                trigger_animations: true,
                flag_14: false,
                camera_ops: vec![
                    DemoCameraOp::Position {
                        values: [0.0, -1000.0, 0.0],
                        is_raw: true,
                    },
                    DemoCameraOp::Interest {
                        values: [0.0, -1000.0, 2000.0],
                        is_raw: true,
                    },
                    DemoCameraOp::Plane(300.0),
                ],
                light_ops: vec![],
                char_ops: vec![
                    (0, DemoCharOp::Visible(true)),
                    (
                        0,
                        DemoCharOp::Position {
                            values: [0.0, -150.0, 2000.0],
                            is_raw: true,
                        },
                    ),
                    (1, DemoCharOp::Visible(true)),
                    (
                        1,
                        DemoCharOp::Position {
                            values: [2650.0, -550.0, 2000.0],
                            is_raw: true,
                        },
                    ),
                    (2, DemoCharOp::Visible(true)),
                    (
                        2,
                        DemoCharOp::Position {
                            values: [2500.0, -550.0, 2000.0],
                            is_raw: true,
                        },
                    ),
                ],
            },
            DemoFrameRecord {
                frame: 1,
                is_cut: false,
                trigger_animations: false,
                flag_14: false,
                camera_ops: vec![],
                light_ops: vec![],
                char_ops: vec![
                    // Actor 1 (`mkn_1_pos`) remains stationary (delta = 0)
                    // Actor 2 (`agl_pos`) walks -60 mm toward X = 0 (entering the 4:3 frame)
                    (
                        2,
                        DemoCharOp::Position {
                            values: [-60.0, 0.0, 0.0],
                            is_raw: false,
                        },
                    ),
                ],
            },
        ],
        animation_trigger_frames: vec![0],
    };

    let info = DramaDemoPlayInfo {
        demo_no: 99,
        id: "test_cull",
        title: "Test Off-Screen 4:3 Culling",
        dds_path: "data/demo/tenbou/tenbou.dds",
        stage_map_path: "bg/ob/ob01.map",
        stage_name: "ob01",
        mes_path: "data/etc/message/stage_observation_msg_e.mes",
        msg_start: 0,
        subtitle_windows: &[],
        stream_no: 0,
        stream_start: 0,
        anm_frame_offset: 0,
        offset_x: 0.0,
        offset_z: 0.0,
        sequence_chain: &[],
    };

    for &aspect in &[
        STANDARD_16X9_ASPECT,
        ULTRAWIDE_21X9_ASPECT,
        SUPER_ULTRAWIDE_32X9_ASPECT,
    ] {
        let mut player = DramaDemoPlayer::new_with_widescreen(
            info,
            script.clone(),
            aspect,
            CutsceneWidescreenMode::UltrawideHorPlusSmartCull,
        );
        player.seek_to_frame(0.5, false);

        // Actor 0 (James inside 4:3 cone) is visible
        assert!(player.actors[0].script_visible);
        assert!(!player.actors[0].culled_offscreen_4x3);
        assert!(player.actors[0].visible);

        // Actor 1 (Mannequin stationary outside 4:3 cone in widescreen margin) is culled!
        assert!(player.actors[1].script_visible);
        assert!(
            player.actors[1].culled_offscreen_4x3,
            "Stationary actor outside 4:3 cone must be culled at aspect {aspect:.2}"
        );
        assert!(!player.actors[1].visible);

        // Actor 2 (Angela actively walking into 4:3 cone near border) remains visible!
        assert!(player.actors[2].script_visible);
        assert!(
            !player.actors[2].culled_offscreen_4x3,
            "Entering actor walking into 4:3 cone must remain visible at aspect {aspect:.2}"
        );
        assert!(player.actors[2].visible);

        // Switching to Pillarbox4x3 disables widescreen frustum culling (since 4:3 pillarbars mask the sides)
        player.set_widescreen_config(aspect, CutsceneWidescreenMode::Pillarbox4x3);
        assert!(!player.actors[1].culled_offscreen_4x3);
        assert!(player.actors[1].visible);
    }
}

#[test]
fn test_room_307_closet_cutscene_fix_sankaku1_wp_closet() {
    let info = find_drama_demo_by_id("wp_closet").expect("wp_closet alias resolves to sankaku1");
    assert_eq!(info.demo_no, CS_APT_RPT_CLOSET_DEMO_NO);
    assert_eq!(info.id, "sankaku1");

    // 1. Verify Mannequin despawn threshold (359.0 instead of 360.0)
    let fix_358 = ClosetCutsceneFixState::evaluate(
        CS_APT_RPT_CLOSET_DEMO_NO,
        358.5,
        -20500.0,
        ULTRAWIDE_21X9_ASPECT,
    );
    assert!(!fix_358.mannequin_despawned);
    assert!(!fix_358.should_hide_actor("mkn_1_pos"));

    let fix_359 = ClosetCutsceneFixState::evaluate(
        CS_APT_RPT_CLOSET_DEMO_NO,
        CLOSET_MANNEQUIN_FIXED_DESPAWN_FRAME,
        -20500.0,
        ULTRAWIDE_21X9_ASPECT,
    );
    assert!(fix_359.mannequin_despawned);
    assert!(fix_359.should_hide_actor("mkn_1_pos"));
    assert!(fix_359.should_hide_actor("mkn_2_pos"));

    // 2. Verify Blurred Bars despawn at frame 1695.0 and at camera_pos_x == -21376.56445,
    // plus first-person blur ultrawide stretch and grain-above-blur ordering
    let fix_bars_cam = ClosetCutsceneFixState::evaluate(
        CS_APT_RPT_CLOSET_DEMO_NO,
        800.0,
        CLOSET_BLURRED_BARS_HIDE_CAM_X,
        SUPER_ULTRAWIDE_32X9_ASPECT,
    );
    assert!(fix_bars_cam.blurred_bars_hidden);
    assert!(fix_bars_cam.should_hide_actor("b_doo_pos"));
    assert!(fix_bars_cam.first_person_blur_overlay_active);
    assert!(
        (fix_bars_cam.blur_overlay_width_scale
            - (SUPER_ULTRAWIDE_32X9_ASPECT / AUTHORED_4X3_ASPECT))
            .abs()
            < 1e-4
    );
    assert!(fix_bars_cam.render_grain_above_blur);

    let fix_1695 = ClosetCutsceneFixState::evaluate(
        CS_APT_RPT_CLOSET_DEMO_NO,
        CLOSET_BLURRED_BARS_FIXED_DESPAWN_FRAME,
        -20500.0,
        ULTRAWIDE_21X9_ASPECT,
    );
    assert!(fix_1695.blurred_bars_hidden);
    assert!(fix_1695.should_hide_actor("b_doo_pos"));

    // 3. Verify Flashlight suppression at camera_pos_x == -22256.61133
    let fix_flash = ClosetCutsceneFixState::evaluate(
        CS_APT_RPT_CLOSET_DEMO_NO,
        500.0,
        CLOSET_DISABLE_FLASHLIGHT_CAM_X,
        STANDARD_16X9_ASPECT,
    );
    assert!(fix_flash.flashlight_suppressed);

    // 4. Verify against retail `sankaku.dds` if present
    let sankaku_path =
        "../../Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2/data/demo/sankaku1/sankaku.dds";
    if std::path::Path::new(sankaku_path).exists() {
        let dds = parse_demo_dds(&std::fs::read(sankaku_path).unwrap()).unwrap();
        let mut player = DramaDemoPlayer::new_with_widescreen(
            *info,
            dds,
            ULTRAWIDE_21X9_ASPECT,
            CutsceneWidescreenMode::UltrawideHorPlusSmartCull,
        );
        player.seek_to_frame(359.2, false);
        assert!(player.closet_fix.mannequin_despawned);
        for actor in &player.actors {
            if actor.actor_name == "mkn_1_pos" || actor.actor_name == "mkn_2_pos" {
                assert!(
                    !actor.visible,
                    "Mannequin must be despawned at frame 359.2 in sankaku1"
                );
            }
        }
    }
}

#[test]
fn test_labyrinth_piston_room_fix_papa_agl_ug_angela() {
    let info = find_drama_demo_by_id("ug_angela").expect("ug_angela alias resolves to papa_agl");
    assert_eq!(info.demo_no, CS_PS_ANGELA_BOSS_FINISH_DEMO_NO);

    let mut piston_fix = PistonRoomFixState::default();
    piston_fix.update(CS_PS_ANGELA_BOSS_FINISH_DEMO_NO, 19500.0);
    assert!(piston_fix.active);
    assert!(!piston_fix.doorway_piston_culled);
    assert_eq!(piston_fix.doorway_piston_offset, 0.0);

    // When Angela opens the door and camera X hits 20278.414 (`0x469E6CD4`), doorway piston culls and latches
    piston_fix.update(
        CS_PS_ANGELA_BOSS_FINISH_DEMO_NO,
        PISTON_ROOM_DOOR_OPEN_CAM_X,
    );
    assert!(piston_fix.doorway_piston_culled);
    assert!((piston_fix.doorway_piston_offset - PISTON_DOORWAY_PROP_OFFSET).abs() < 1e-5);

    // Remains latched for subsequent camera shots until cutscene ends
    piston_fix.update(CS_PS_ANGELA_BOSS_FINISH_DEMO_NO, 19800.0);
    assert!(piston_fix.doorway_piston_culled);

    // Verify against retail `papa_agl.dds` if present
    let papa_path =
        "../../Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2/data/demo/papa_agl/papa_agl.dds";
    if std::path::Path::new(papa_path).exists() {
        let dds = parse_demo_dds(&std::fs::read(papa_path).unwrap()).unwrap();
        let mut player = DramaDemoPlayer::new_with_widescreen(
            *info,
            dds,
            ULTRAWIDE_21X9_ASPECT,
            CutsceneWidescreenMode::UltrawideHorPlusSmartCull,
        );
        // Scrub through the end of papa_agl where Angela opens the door
        player.seek_to_frame((player.total_frames() - 10) as f32, false);
        assert!(
            player.piston_fix.doorway_piston_culled,
            "Doorway piston must be culled after Angela opens the door in papa_agl"
        );
    }
}

#[test]
fn test_final_boss_room_fix_and_observation_deck_maria_fog_bounds() {
    // 1. Final Boss Room (`R_FINAL_BOSS_RM == 0xBB`)
    let boss_high = FinalBossRoomFixState::evaluate(R_FINAL_BOSS_RM_ID, -15000.0);
    assert!(boss_high.in_final_boss_room);
    assert_eq!(boss_high.bottom_walkway_spawn_y, FINAL_BOSS_LOWERED_SPAWN_Y);
    assert_eq!(boss_high.bottom_floor_spawn_y, FINAL_BOSS_LOWERED_SPAWN_Y);
    assert_eq!(boss_high.black_box_spawn_y, FINAL_BOSS_LOWERED_SPAWN_Y);
    assert_eq!(boss_high.draw_distance, FINAL_BOSS_EXTENDED_DRAW_DISTANCE);

    // 2. Observation Deck (`ob01` / `tenbou`) horizontal fog bounds & particle wrap/fade
    let (min_x, max_x) = observation_deck_fog_bounds_x();
    assert_eq!((min_x, max_x), (-24_500.0, 24_500.0));
    let wrapped = observation_deck_wrap_particle_x(24_600.0);
    assert!((wrapped - (-24_400.0)).abs() < 1e-3);
    assert!((observation_deck_border_alpha_factor(0.0, 0.0, -5000.0, 5000.0) - 1.0).abs() < 1e-5);
    assert!(
        (observation_deck_border_alpha_factor(24_350.0, 0.0, -5000.0, 5000.0) - 0.5).abs() < 1e-4
    );

    // 3. Meeting Maria (`demo_no == 22`, `deai`) widescreen fog counters (`965.5` and `1460.0`)
    assert_eq!(
        evaluate_meeting_maria_fog_counters(CS_TOWN_WEST_MARIA_MEET_DEMO_NO, 964.0),
        (false, false)
    );
    assert_eq!(
        evaluate_meeting_maria_fog_counters(CS_TOWN_WEST_MARIA_MEET_DEMO_NO, 966.0),
        (true, false)
    );
    assert_eq!(
        evaluate_meeting_maria_fog_counters(CS_TOWN_WEST_MARIA_MEET_DEMO_NO, 1461.0),
        (true, true)
    );
}

#[test]
fn test_disable_cutscene_borders_auto_and_set_black_pillarboxes_21x9_32x9() {
    // 1. DisableCutsceneBorders = Auto hides top/bottom letterbox bars on 16:9, 21:9, and 32:9
    let bars_21_9 = CutsceneBorderBars::evaluate(
        3440.0,
        1440.0,
        CutsceneWidescreenMode::UltrawideHorPlusSmartCull,
        CutsceneBorderMode::Auto,
    );
    assert_eq!(bars_21_9.letterbox_f0_f1, (0.5, 0.5));
    assert!(!bars_21_9.letterbox_bars[0].is_non_empty());
    assert!(!bars_21_9.pillarbox_bars[0].is_non_empty());

    // In Pillarbox4x3 mode on 21:9 (3440x1440), left/right black pillarboxes are emitted
    let bars_pillar = CutsceneBorderBars::evaluate(
        3440.0,
        1440.0,
        CutsceneWidescreenMode::Pillarbox4x3,
        CutsceneBorderMode::Auto,
    );
    assert!(bars_pillar.pillarbox_bars[0].is_non_empty());
    assert!(bars_pillar.pillarbox_bars[1].is_non_empty());
    assert!((bars_pillar.pillarbox_bars[0].width() - 760.0).abs() < 1.0);

    // 2. SetBlackPillarBoxes (`PillarboxViewportLayout`) on 32:9 (5120x1440) for 16:9 and 4:3 2D media
    let layout_32_9 = PillarboxViewportLayout::for_aspect(5120.0, 1440.0, STANDARD_16X9_ASPECT);
    // 16:9 canvas on 5120x1440 is 2560px wide -> [1280.0 .. 3840.0]
    assert!((layout_32_9.canvas_left - 1280.0).abs() < 1e-3);
    assert!((layout_32_9.canvas_right - 3840.0).abs() < 1e-3);

    // Quad crossing the left pillarbox boundary (x: 1000..1560, u: 0.0..1.0) is clipped to 1280..1560 with u0 = 0.5
    let clipped = layout_32_9
        .clip_axis_aligned_quad(1000.0, 200.0, 1560.0, 400.0, 0.0, 0.0, 1.0, 1.0)
        .expect("partially visible quad");
    assert!((clipped.0 - 1280.0).abs() < 1e-3);
    assert!((clipped.4 - 0.5).abs() < 1e-3);

    // Quad completely inside the left pillarbox (x: 200..800) is discarded
    assert!(layout_32_9
        .clip_axis_aligned_quad(200.0, 200.0, 800.0, 400.0, 0.0, 0.0, 1.0, 1.0)
        .is_none());

    // Green player map marker inside active canvas is preserved; marker panned into pillarbox is culled
    let marker_in = [(2500.0, 700.0), (2510.0, 715.0), (2490.0, 715.0)];
    assert!(layout_32_9.clip_map_marker_triangle(marker_in).is_some());
    let marker_out = [(600.0, 700.0), (610.0, 715.0), (590.0, 715.0)];
    assert!(layout_32_9.clip_map_marker_triangle(marker_out).is_none());

    // OverlaySpriteQuad protruding past -256..256 is clipped to the active canvas
    let over_quad = OverlaySpriteQuad {
        sheet_name: "out/p_incar_map",
        vertices: [
            (-300.0, -100.0, 0.0, 0.0),
            (100.0, -100.0, 1.0, 0.0),
            (100.0, 100.0, 1.0, 1.0),
            (-300.0, 100.0, 0.0, 1.0),
        ],
        alpha: 1.0,
    };
    let clipped_tris = layout_32_9
        .map_and_clip_overlay_quad(&over_quad)
        .expect("protruding overlay quad clipped");
    for &(vx, _vy, u, _v) in &clipped_tris {
        assert!(vx >= layout_32_9.canvas_left - 1e-3 && vx <= layout_32_9.canvas_right + 1e-3);
        assert!((0.0..=1.0).contains(&u));
    }
}

#[test]
fn test_hei_lau_smooth_widescreen_entry_and_catalog_onscreen_actors_preserved() {
    // 1. Verify `sankaku1` (`demo_no == 14`) uses Wood Side Apt Room 307 (`bg/ap/ap18.map`)
    let sankaku_info = find_drama_demo_by_id("sankaku1").expect("sankaku1 in catalog");
    assert_eq!(sankaku_info.stage_map_path, "bg/ap/ap18.map");
    assert_eq!(sankaku_info.stage_name, "ap18");

    let base = "../../Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2";
    let hei_path = format!("{base}/data/demo/hei_lau/hei_lau_a.dds");
    if !std::path::Path::new(&hei_path).exists() {
        return;
    }

    // 2. Verify `sankaku1` second mannequin (`mkn_2_pos`) has 2576-frame subclip offset in `mkn.anm`
    let sankaku_dds =
        parse_demo_dds(&std::fs::read(format!("{base}/{}", sankaku_info.dds_path)).unwrap())
            .unwrap();
    let sankaku_player = DramaDemoPlayer::new(*sankaku_info, sankaku_dds, false);
    let mkn1 = sankaku_player
        .actors
        .iter()
        .find(|a| a.actor_name == "mkn_1_pos")
        .unwrap();
    let mkn2 = sankaku_player
        .actors
        .iter()
        .find(|a| a.actor_name == "mkn_2_pos")
        .unwrap();
    assert_eq!(mkn1.anm_frame_offset, 0);
    assert_eq!(mkn2.anm_frame_offset, 2576);

    // 3. Verify `hei_lau` (`demo_no == 20`) smooth widescreen entry walk for James (`hhl_jms_pos`)
    let hei_info = find_drama_demo_by_id("hei_lau").expect("hei_lau in catalog");
    let hei_dds = parse_demo_dds(&std::fs::read(&hei_path).unwrap()).unwrap();
    let mut hei_player = DramaDemoPlayer::new_with_widescreen(
        *hei_info,
        hei_dds,
        ULTRAWIDE_21X9_ASPECT,
        CutsceneWidescreenMode::UltrawideHorPlusSmartCull,
    );

    // Before frame 172.0, James is parked off-screen left and hidden, while Laura on the wall is visible
    hei_player.seek_to_frame(100.0, false);
    assert!(!hei_player.actors[0].visible);
    assert!(hei_player.actors[0].culled_offscreen_4x3);
    assert!(hei_player.actors[1].visible);

    // During frames 172.0..240.0, James is visible and walking smoothly from Z = 45620.3 to Z = 46492.07
    hei_player.seek_to_frame(172.0, false);
    assert!(hei_player.actors[0].visible);
    assert!((hei_player.actors[0].position.z - 45_620.31).abs() < 1.0);

    hei_player.seek_to_frame(206.0, false);
    assert!(hei_player.actors[0].visible);
    assert!((hei_player.actors[0].position.z - 46_056.19).abs() < 1.0);
    let walk_f = hei_player.actors[0].global_anm_frame(206.0);
    assert!(
        (240.0..=257.0).contains(&walk_f),
        "Pre-roll walk frame {walk_f} must oscillate within [240..257]"
    );

    // C^0 position and animation pose continuity across the 239.9 -> 240.0 hand-off
    hei_player.seek_to_frame(239.9, false);
    let pos_239_9 = hei_player.actors[0].position;
    let anm_239_9 = hei_player.actors[0].global_anm_frame(239.9);
    hei_player.seek_to_frame(240.0, false);
    let pos_240_0 = hei_player.actors[0].position;
    let anm_240_0 = hei_player.actors[0].global_anm_frame(240.0);
    assert!((pos_240_0 - pos_239_9).length() < 2.0);
    assert!((anm_240_0 - anm_239_9).abs() < 0.2);

    // 4. Verify on-screen main characters are never falsely culled in close-up / over-the-shoulder shots
    for (id, frame) in [
        ("gero_edi", 300.0),
        ("knife_agl", 200.0),
        ("bp_edi_1", 100.0),
        ("mar_saikai", 500.0),
        ("papa_agl_a", 50.0),
        ("papa_agl_a", 350.0),
        ("killer_edi", 400.0),
        ("radio_gero", 200.0),
        ("haka_agl", 2300.0),
    ] {
        let info = find_drama_demo_by_id(id).unwrap();
        let dds =
            parse_demo_dds(&std::fs::read(format!("{base}/{}", info.dds_path)).unwrap()).unwrap();
        let mut player = DramaDemoPlayer::new_with_widescreen(
            *info,
            dds,
            ULTRAWIDE_21X9_ASPECT,
            CutsceneWidescreenMode::UltrawideHorPlusSmartCull,
        );
        player.seek_to_frame(frame, false);
        for actor in &player.actors {
            if actor.script_visible
                && !actor.actor_name.starts_with("i_")
                && !actor.actor_name.starts_with("b_")
                && !actor.actor_name.starts_with("pap_")
            {
                assert!(
                    actor.visible && !actor.culled_offscreen_4x3,
                    "On-screen actor {} in {id} at frame {frame} must not be culled (pos={:?}, eye={:?}, interest={:?})",
                    actor.actor_name,
                    actor.position,
                    player.camera.eye,
                    player.camera.interest,
                );
            }
        }
    }

    // 5. Verify genuinely parked off-screen actors (e.g. Laura in `bp_edi_2` & `vtr`, Boss Mary in `mry_yarinaoshi_a`) ARE culled
    for (id, frame, parked_offscreen_actor) in [
        ("bp_edi_2", 200.0, "lau_pos"),
        ("vtr_s", 100.0, "lau_pos"),
        ("piano_lau", 10.0, "hhh_jms_pos"),
        ("mry_yarinaoshi_a", 100.0, "bos_pos"),
    ] {
        let info = find_drama_demo_by_id(id).unwrap();
        let dds =
            parse_demo_dds(&std::fs::read(format!("{base}/{}", info.dds_path)).unwrap()).unwrap();
        let mut player = DramaDemoPlayer::new_with_widescreen(
            *info,
            dds,
            ULTRAWIDE_21X9_ASPECT,
            CutsceneWidescreenMode::UltrawideHorPlusSmartCull,
        );
        player.seek_to_frame(frame, false);
        let actor = player
            .actors
            .iter()
            .find(|a| a.actor_name == parked_offscreen_actor)
            .unwrap();
        assert!(
            actor.culled_offscreen_4x3 && !actor.visible,
            "Parked off-screen actor {parked_offscreen_actor} in {id} at frame {frame} must be culled"
        );
    }

    let _ = Vec3::ZERO;
}

#[test]
fn test_maria_and_eddie_softimage_locator_visibility_and_model_anm_cls_evaluation() {
    let base = "../../Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2";
    let deai_e_path = format!("{base}/data/demo/mar_deai/mar_deai_e.dds");
    if !std::path::Path::new(&deai_e_path).exists() {
        return;
    }

    // 1. In `SLUS_202.28` (`DdsPlay` at `0x1b2348`) and `sh2pc.exe` (`0x4a1ca3`), `DdsPlayCharacter`
    // sub_key `c == 2` (`DemoCharOp::Visible(false)`) only records whether the `*_pos` null locator
    // was hidden in the Softimage 3D viewport when exported (`0x11337d0` PS2 / `0x94dfe4` PC is never
    // read by `DdsPlay`). Verify Maria (`hhh_mar_pos`) and James (`hhh_jms_pos`) are both visible
    // throughout `deai_c` (`mar_deai_c.dds`), `deai` (`mar_deai_e.dds`, including frame 78),
    // `mar_deai_full` (`mar_deai.dds`), and `mar_saikai` (`mar_saikai.dds`).
    for (id, frames) in [
        ("deai_c", &[0.0f32, 78.0, 200.0, 500.0, 730.0, 966.0, 1500.0][..]),
        ("deai", &[0.0f32, 78.0, 200.0, 500.0, 730.0, 966.0, 1500.0][..]),
        ("mar_saikai", &[100.0f32, 500.0, 1000.0][..]),
    ] {
        let info = find_drama_demo_by_id(id).expect("Maria cutscene in catalog");
        let dds =
            parse_demo_dds(&std::fs::read(format!("{base}/{}", info.dds_path)).unwrap()).unwrap();
        let mut player = DramaDemoPlayer::new_with_widescreen(
            *info,
            dds,
            STANDARD_16X9_ASPECT,
            CutsceneWidescreenMode::UltrawideHorPlusSmartCull,
        );
        for &frame in frames {
            player.seek_to_frame(frame, false);
            let jms = player
                .actors
                .iter()
                .find(|a| a.actor_name == "hhh_jms_pos")
                .expect("hhh_jms_pos in Maria cutscene");
            let mar = player
                .actors
                .iter()
                .find(|a| a.actor_name == "hhh_mar_pos")
                .expect("hhh_mar_pos in Maria cutscene");
            assert!(
                jms.script_visible && jms.visible,
                "James must be visible in {id} at frame {frame}"
            );
            assert!(
                mar.script_visible && mar.visible && !mar.culled_offscreen_4x3,
                "Maria (hhh_mar_pos) must be visible in {id} at frame {frame} (pos={:?}, eye={:?}, interest={:?})",
                mar.position,
                player.camera.eye,
                player.camera.interest,
            );
        }
    }

    // 2. Verify Eddie (`hhh_edi_pos`) and his pizza box (`b_piz_pos`) are visible in `bp_edi_1` and `bp_edi_2`
    for (id, frame) in [("bp_edi_1", 200.0f32), ("bp_edi_2", 100.0f32)] {
        let info = find_drama_demo_by_id(id).unwrap();
        let dds =
            parse_demo_dds(&std::fs::read(format!("{base}/{}", info.dds_path)).unwrap()).unwrap();
        let mut player = DramaDemoPlayer::new_with_widescreen(
            *info,
            dds,
            STANDARD_16X9_ASPECT,
            CutsceneWidescreenMode::UltrawideHorPlusSmartCull,
        );
        player.seek_to_frame(frame, false);
        for actor_name in ["hhh_edi_pos", "b_piz_pos"] {
            let actor = player
                .actors
                .iter()
                .find(|a| a.actor_name == actor_name)
                .unwrap();
            assert!(
                actor.script_visible && actor.visible,
                "{actor_name} must be visible in {id} at frame {frame}"
            );
        }
    }

    // 3. Verify unplaced handheld items parked at raw (0.0, 0.0, 0.0) in Softimage stay hidden
    let sankaku_info = find_drama_demo_by_id("sankaku1").unwrap();
    let sankaku_dds =
        parse_demo_dds(&std::fs::read(format!("{base}/{}", sankaku_info.dds_path)).unwrap())
            .unwrap();
    let mut sankaku_player = DramaDemoPlayer::new(*sankaku_info, sankaku_dds, false);
    sankaku_player.seek_to_frame(100.0, false);
    for unplaced_item in ["i_handgun_pos", "i_magazine_pos"] {
        let item = sankaku_player
            .actors
            .iter()
            .find(|a| a.actor_name == unplaced_item)
            .unwrap();
        assert!(
            !item.script_visible && !item.visible,
            "Unplaced item {unplaced_item} at raw (0,0,0) in sankaku1 must remain hidden"
        );
    }

    // 4. Verify Maria's 3D model (`data/chr/mar/hhh_mar.mdl`), cutscene skeletal animation
    // (`data/demo/mar_deai/hhh_mar.anm`), and facial morph clusters (`data/demo/mar_deai/hhh_mar.cls`)
    // load, match bone count, and evaluate cleanly at Frame 78 of `deai` (`mar_deai_e`).
    let mar_mdl_bytes =
        std::fs::read(format!("{base}/data/chr/mar/hhh_mar.mdl")).expect("read hhh_mar.mdl");
    let mar_mdl = sh2_assets::formats::parse_mdl(&mar_mdl_bytes).expect("parse hhh_mar.mdl");
    let mar_anm_bytes = std::fs::read(format!("{base}/data/demo/mar_deai/hhh_mar.anm"))
        .expect("read mar_deai/hhh_mar.anm");
    let mar_anm = sh2_assets::formats::parse_anm(
        &mar_anm_bytes,
        mar_mdl.bone_count,
        &mar_mdl.parents,
    )
    .expect("parse mar_deai/hhh_mar.anm");
    let mar_cls_bytes = std::fs::read(format!("{base}/data/demo/mar_deai/hhh_mar.cls"))
        .expect("read mar_deai/hhh_mar.cls");
    let mar_cls =
        sh2_assets::formats::parse_cls(&mar_cls_bytes).expect("parse mar_deai/hhh_mar.cls");

    let deai_info = find_drama_demo_by_id("deai").unwrap();
    let deai_dds = parse_demo_dds(&std::fs::read(&deai_e_path).unwrap()).unwrap();
    let mut deai_player = DramaDemoPlayer::new(*deai_info, deai_dds, false);
    deai_player.seek_to_frame(78.0, false);
    let mar_actor = deai_player
        .actors
        .iter()
        .find(|a| a.actor_name == "hhh_mar_pos")
        .unwrap();
    assert_eq!(mar_actor.anm_frame_offset, 1546 + 15);
    assert_eq!(mar_actor.cls_frame_offset, 1546);
    let (f0, f1, frac) = mar_anm.sample_span_at(mar_actor.global_anm_frame(78.0));
    assert!(f0 < mar_anm.frames.len() && f1 < mar_anm.frames.len());
    assert!((0.0..=1.0).contains(&frac));

    let mut weights = Vec::new();
    mar_cls.evaluate_global_frame_into(mar_actor.global_cls_frame(78.0), false, &mut weights);
    assert!(!weights.is_empty());
}
