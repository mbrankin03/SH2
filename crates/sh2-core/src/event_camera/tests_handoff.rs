//! Retail `.dds` cutscene, hybrid sequence chain (`SEQ_DEAI`), and Bink FMV camera handoff
//! integration tests for `SH2-52`.

use super::*;
use crate::camera::Ps2CameraController;
use crate::cutscene::{find_drama_demo_by_no, CutsceneSequenceStep, DramaDemoPlayer};
use glam::Vec3;
use sh2_assets::formats::bink::parse_bink_file_header;
use sh2_assets::formats::demo::parse_demo_dds;
use sh2_assets::AssetPathResolver;

#[test]
fn test_retail_dds_cutscene_and_bink_fmv_camera_handoffs() {
    let resolver = AssetPathResolver::resolve();
    let mut sys = EventCameraSystem::new();
    let mut cam = Ps2CameraController::new();
    let has_ca11 = cam.load_room_cam_with_resolver(&resolver, "ca11", false);
    let pre_pos = Vec3::new(8_500.0, 0.0, -10_500.0);
    let pre_yaw = 0.35;
    cam.reset_to_player(pre_pos, pre_yaw);
    let pre_snap = cam.curr_snapshot;

    // 1. Standalone FMV (`murder.bik`) with `RestoreExactSavedSnapshot`: warp-safe restore.
    let murder_path = resolver.game_root.join("data/movie/murder.bik");
    let murder_dur = if murder_path.exists() {
        let hdr = parse_bink_file_header(&murder_path).expect("murder.bik header must parse");
        (hdr.frame_count as f32) / ((hdr.fps_num as f32) / (hdr.fps_den.max(1) as f32))
    } else {
        12.0
    };
    assert!(murder_dur > 1.0);

    sys.cutscene_handoff.begin_fmv(
        &mut cam,
        pre_pos,
        pre_yaw,
        "murder",
        murder_dur,
        PostSequenceRestorePolicy::RestoreExactSavedSnapshot,
    );
    assert_eq!(sys.active_authority(), EventCameraAuthority::CutsceneOrFmv);
    let out = sys.tick(
        &mut cam,
        pre_pos,
        pre_yaw,
        1.0,
        -1.0,
        0.5,
        PlayerCameraSuppression::Active,
        &[],
        None,
    );
    assert_eq!(out, HandoffStepOutcome::RunningFmv { movie_id: "murder" });
    assert_eq!(cam.prev_snapshot, pre_snap);

    let skip_out = sys.cutscene_handoff.tick_fmv(
        &mut cam,
        0.0,
        FmvAdvanceSignal::Skip,
        None,
        Some(&mut sys.enemy_framing),
        &[],
    );
    assert_eq!(
        skip_out,
        HandoffStepOutcome::RestoredToGameplay {
            player_pos: pre_pos,
            player_yaw: pre_yaw,
        }
    );
    assert_eq!(cam.curr_snapshot, pre_snap);
    assert_eq!(cam.prev_snapshot, pre_snap);

    // 2. Real retail `.dds` cutscene (`haka_agl.dds`, `demo_no == 3` Angela cemetery).
    if !has_ca11 {
        return;
    }
    let haka_info = find_drama_demo_by_no(3).expect("demo_no 3 (haka_agl) in catalog");
    let haka_dds_path = resolver.game_root.join(haka_info.dds_path);
    if haka_dds_path.exists() {
        let haka_bytes = std::fs::read(&haka_dds_path).expect("read haka_agl.dds");
        let haka_script = parse_demo_dds(&haka_bytes).expect("parse haka_agl.dds");
        let mut demo = DramaDemoPlayer::new(*haka_info, haka_script, true);

        sys.cutscene_handoff.begin_drama_demo(
            &mut cam,
            pre_pos,
            pre_yaw,
            &demo,
            PostSequenceRestorePolicy::ReevaluateAtPlayerPose,
        );
        assert_eq!(cam.prev_snapshot, cam.curr_snapshot);
        assert!((cam.curr_snapshot.eye - demo.camera.eye).length() < 1e-2);

        demo.tick(1.0 / 60.0, true);
        let step_out = sys.tick(
            &mut cam,
            pre_pos,
            pre_yaw,
            0.0,
            0.0,
            1.0 / 60.0,
            PlayerCameraSuppression::Active,
            &[],
            Some(&demo),
        );
        assert!(matches!(
            step_out,
            HandoffStepOutcome::RunningDemo { demo_no: 3, .. }
        ));

        // Skip cutscene (`DramaDemoSkipLast` `0x001b2f10`) -> commits `shCharacterSetPosAfterDemo` (`0x0011f770`).
        demo.skip();
        let expected_pose = demo
            .player_handoff_transform()
            .unwrap_or((pre_pos, pre_yaw));
        let done_out = sys.tick(
            &mut cam,
            pre_pos,
            pre_yaw,
            0.0,
            0.0,
            1.0 / 60.0,
            PlayerCameraSuppression::Active,
            &[],
            Some(&demo),
        );
        assert_eq!(
            done_out,
            HandoffStepOutcome::RestoredToGameplay {
                player_pos: expected_pose.0,
                player_yaw: expected_pose.1,
            }
        );
        assert_eq!(cam.prev_snapshot, cam.curr_snapshot);
        assert!(cam.interpolated_view_matrix(0.5).is_finite());
    }

    // 3. Hybrid sequence chain `SEQ_DEAI` (`demo 21` -> `FmvMovie("deai")` -> `demo 22` -> `SmoothBlendToGameplay`).
    let deai_c_info = find_drama_demo_by_no(21).expect("demo 21 in catalog");
    let deai_e_info = find_drama_demo_by_no(22).expect("demo 22 in catalog");
    let deai_c_path = resolver.game_root.join(deai_c_info.dds_path);
    let deai_e_path = resolver.game_root.join(deai_e_info.dds_path);
    if deai_c_path.exists() && deai_e_path.exists() {
        let mut demo_c = DramaDemoPlayer::new(
            *deai_c_info,
            parse_demo_dds(&std::fs::read(&deai_c_path).expect("read mar_deai_c")).expect("parse"),
            true,
        );
        let mut demo_e = DramaDemoPlayer::new(
            *deai_e_info,
            parse_demo_dds(&std::fs::read(&deai_e_path).expect("read mar_deai_e")).expect("parse"),
            true,
        );

        sys.cutscene_handoff.begin_drama_demo(
            &mut cam,
            pre_pos,
            pre_yaw,
            &demo_c,
            PostSequenceRestorePolicy::SmoothBlendToGameplay { blend_ticks: 6 },
        );
        demo_c.skip();
        let step1 = sys.tick(
            &mut cam,
            pre_pos,
            pre_yaw,
            0.0,
            0.0,
            1.0 / 60.0,
            PlayerCameraSuppression::Active,
            &[],
            Some(&demo_c),
        );
        assert_eq!(
            step1,
            HandoffStepOutcome::TransitionedChainStep {
                next_step: CutsceneSequenceStep::FmvMovie { movie_id: "deai" },
            }
        );

        let step2 = sys.cutscene_handoff.tick_fmv(
            &mut cam,
            0.0,
            FmvAdvanceSignal::Skip,
            None,
            Some(&mut sys.enemy_framing),
            &[],
        );
        assert_eq!(
            step2,
            HandoffStepOutcome::TransitionedChainStep {
                next_step: CutsceneSequenceStep::InEngineDemo { demo_no: 22 },
            }
        );

        sys.cutscene_handoff.attach_chained_demo(&mut cam, &demo_e);
        demo_e.skip();
        let step3 = sys.tick(
            &mut cam,
            pre_pos,
            pre_yaw,
            0.0,
            0.0,
            1.0 / 60.0,
            PlayerCameraSuppression::Active,
            &[],
            Some(&demo_e),
        );
        assert_eq!(
            step3,
            HandoffStepOutcome::BlendingOut { remaining_ticks: 6 }
        );

        for rem in (1..=5).rev() {
            let b = sys.tick(
                &mut cam,
                pre_pos,
                pre_yaw,
                0.0,
                0.0,
                1.0 / 60.0,
                PlayerCameraSuppression::Active,
                &[],
                None,
            );
            assert_eq!(
                b,
                HandoffStepOutcome::BlendingOut {
                    remaining_ticks: rem
                }
            );
        }
        let final_out = sys.tick(
            &mut cam,
            pre_pos,
            pre_yaw,
            0.0,
            0.0,
            1.0 / 60.0,
            PlayerCameraSuppression::Active,
            &[],
            None,
        );
        assert!(matches!(
            final_out,
            HandoffStepOutcome::RestoredToGameplay { .. }
        ));
        assert_eq!(sys.active_authority(), EventCameraAuthority::GameplayRail);
    }
}
