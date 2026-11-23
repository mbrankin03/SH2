//! Unit tests for `DramaDemoPlayer`, voice buffer sync, frame snapping, and letterbox geometry.

use super::*;
use sh2_assets::formats::demo::{DemoDdsFile, DemoDdsHeader, DemoFrameRecord};

fn minimal_demo_script(total_frames: u32) -> DemoDdsFile {
    DemoDdsFile {
        header: DemoDdsHeader {
            version_a: 0.0,
            version_b: 0.0,
            total_frames,
            total_objects: 0,
            point_light_count: 0,
            spot_light_count: 0,
            inf_light_count: 0,
            character_names: Vec::new(),
        },
        records: vec![DemoFrameRecord {
            frame: 0,
            is_cut: false,
            trigger_animations: false,
            flag_14: false,
            camera_ops: Vec::new(),
            light_ops: Vec::new(),
            char_ops: Vec::new(),
        }],
        animation_trigger_frames: Vec::new(),
    }
}

#[test]
fn test_dds_play_09999_frame_boundary_snap() {
    assert_eq!(snap_demo_frame_boundary(12.99995), 13.0);
    assert_eq!(snap_demo_frame_boundary(12.9998), 12.9998);
    assert_eq!(snap_demo_frame_boundary(0.99991), 1.0);
    assert_eq!(snap_demo_frame_boundary(5.0), 5.0);
}

#[test]
fn test_drama_demo_main_voice_buffer_stall_and_reset_df() {
    let mut info = crate::cutscene::catalog::DRAMA_DEMO_CATALOG[0];
    info.stream_no = 118;
    info.stream_start = 1;
    let mut player = DramaDemoPlayer::new(info, minimal_demo_script(100), false);

    // Tick 1: `0x80` preload block fires alone (`shResetDF`); `0x100` does NOT execute on the same tick.
    let hook1 = player.tick_with_voice_buffer_sync(
        1.0 / 30.0,
        false,
        None,
        DemoVoiceBufferState::Buffering,
    );
    assert_eq!(hook1, DemoDeltaFrameHook::ResetDeltaFrame);
    assert!(player.voice_preload_requested);
    assert!(!player.voice_started);
    assert!((player.scene_frame - 1.0).abs() < 1e-4);
    assert_eq!(player.voice_stall_secs, 0.0);

    // Tick 2: `0x80` is now set and `scene_frame >= stream_start`, but buffer is still Buffering -> FreezeDeltaFrame (`shSetDFZero`)
    let hook2 = player.tick_with_voice_buffer_sync(
        1.0 / 30.0,
        false,
        None,
        DemoVoiceBufferState::Buffering,
    );
    assert_eq!(hook2, DemoDeltaFrameHook::FreezeDeltaFrame);
    assert!((player.scene_frame - 1.0).abs() < 1e-4);
    assert!(!player.voice_started);
    assert!((player.voice_stall_secs - (1.0 / 30.0)).abs() < 1e-5);

    // Tick 3: buffer becomes Ready -> ResetDeltaFrame (`shResetDF`), voice starts (`0x100`), frame advances
    let hook3 =
        player.tick_with_voice_buffer_sync(1.0 / 30.0, false, None, DemoVoiceBufferState::Ready);
    assert_eq!(hook3, DemoDeltaFrameHook::ResetDeltaFrame);
    assert!(player.voice_started);
    assert!(player.pending_voice_trigger);
    assert!((player.scene_frame - 2.0).abs() < 1e-4);
}

#[test]
fn test_drama_demo_main_preload_and_stream_start_mutually_exclusive_when_stream_start_le_1() {
    for stream_start in [0i16, 1i16] {
        let mut info = crate::cutscene::catalog::DRAMA_DEMO_CATALOG[0];
        info.stream_no = 118;
        info.stream_start = stream_start;
        let mut player = DramaDemoPlayer::new(info, minimal_demo_script(100), false);

        // Even with Ready buffer and `stream_start <= 1`, Tick 1 must ONLY run the `0x80` preload branch
        // (`Event/demoview.c:93-116` / `sh2pc.exe` `FUN_004a2110` `if / else if`).
        let hook1 = player.tick_with_voice_buffer_sync(
            1.0 / 30.0,
            false,
            None,
            DemoVoiceBufferState::Ready,
        );
        assert_eq!(hook1, DemoDeltaFrameHook::ResetDeltaFrame);
        assert!(player.voice_preload_requested);
        assert!(
            !player.voice_started,
            "0x80 preload and 0x100 stream_start must not both fire on tick 1 (stream_start={stream_start})"
        );
        assert!(!player.pending_voice_trigger);

        // Tick 2 now enters the `else if` (`0x100`) branch and triggers voice playback.
        let hook2 = player.tick_with_voice_buffer_sync(
            1.0 / 30.0,
            false,
            None,
            DemoVoiceBufferState::Ready,
        );
        assert_eq!(hook2, DemoDeltaFrameHook::ResetDeltaFrame);
        assert!(player.voice_started);
        assert!(player.pending_voice_trigger);
    }
}

#[test]
fn test_drama_demo_fade_ps2_q12_letterbox_geometry() {
    let info = crate::cutscene::catalog::DRAMA_DEMO_CATALOG[0];
    let mut player = DramaDemoPlayer::new_with_widescreen(
        info,
        minimal_demo_script(100),
        4.0 / 3.0,
        CutsceneWidescreenMode::Sh2eeZoom16x9,
    );
    player.border_mode = CutsceneBorderMode::Enabled;

    // `Check_Filter_Soft() == 0` -> authentic Q12 letterbox bars active
    let rects = player
        .ps2_letterbox_geometry(false)
        .expect("letterbox bars active in 4:3 when Check_Filter_Soft == 0");
    assert_eq!(rects.top_q12, [-4096, -4096, 4096, -3072]);
    assert_eq!(rects.bottom_q12, [-4096, 3072, 4096, 4096]);
    assert!((rects.bar_height_fraction - 0.125).abs() < 1e-6);

    // `Check_Filter_Soft() != 0` -> DramaDemoFade skips drawing bars
    assert!(player.ps2_letterbox_geometry(true).is_none());
}
