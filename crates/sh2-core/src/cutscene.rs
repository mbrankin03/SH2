//! In-Engine Real-Time 3D Cutscene Player (`Event/demoview.c`, `Chacter_Draw/clani.c`) (`SH2-63`).
//!
//! Modularized into:
//! - [`anim_info`]: 84-entry `DramaDemo_AnimInfo` table (`0x2ba080` PS2 / `0x88ed20` PC), light distance decay ramp (`0x50cff0`), and per-demo FOV scales.
//! - [`catalog`]: Curated `DRAMA_DEMO_CATALOG`, `DramaDemoPlayInfo`, `SubtitleWindow` (`adr_msg_time`), and `CutsceneSequenceStep`.
//! - [`player`]: Stateful `DramaDemoPlayer`, `CutsceneCameraSnapshot`, and `CutsceneActorRuntime`.

mod anim_info;
mod catalog;
mod player;

pub use anim_info::{
    demo_fov_scale_for_demo_no, demo_light_decay_bounds, derive_demo_actor_anm_and_cls_paths,
    find_drama_demo_anim_info, DramaDemoAnimInfo, DEMO_LIGHT_DECAY_RAMP, DRAMA_DEMO_ANIM_INFO,
};
pub use catalog::{
    find_drama_demo_by_id, find_drama_demo_by_no, CutsceneSequenceStep, DramaDemoPlayInfo,
    SubtitleWindow, DRAMA_DEMO_CATALOG, SUBTITLES_BP_EDI_2, SUBTITLES_DEAI, SUBTITLES_FIRE_AGL,
    SUBTITLES_FIRST_BATTLE, SUBTITLES_FUKATSU_A, SUBTITLES_GEKIJOU_A, SUBTITLES_GERO_EDI,
    SUBTITLES_HAKA_AGL, SUBTITLES_HEI_LAU, SUBTITLES_KILLER_EDI, SUBTITLES_KNIFE_AGL,
    SUBTITLES_MAR_SAIKAI, SUBTITLES_MRY_YARINAOSHI_A, SUBTITLES_PAPA_AGL, SUBTITLES_PIANO_LAU,
    SUBTITLES_RADIO_GERO, SUBTITLES_TENBOU, SUBTITLES_VTR,
};
pub use player::{CutsceneActorRuntime, CutsceneCameraSnapshot, DramaDemoPlayer, DramaDemoStep};

/// Native keyframe rate of Silent Hill 2 `.dds`, `.anm`, and `.cls` cutscene data (30.0 FPS).
pub const CUTSCENE_NATIVE_FPS: f32 = 30.0;

/// Base stream number offset for `data/sound/adx/voice/voice.afs` (`stream_no = 60001 + afs_index` in 0-based AFS indexing).
pub const VOICE_AFS_STREAM_BASE: i32 = 60000;

#[cfg(test)]
mod tests {
    use super::*;
    use sh2_assets::formats::demo::parse_demo_dds;

    #[test]
    fn test_drama_demo_anim_info_table_integrity() {
        assert_eq!(DRAMA_DEMO_ANIM_INFO.len(), 84);
        let jms = find_drama_demo_anim_info("hhh_jms_pos").expect("hhh_jms_pos must exist");
        assert_eq!(jms.kind_id, 0x0100);
        assert_eq!(jms.model_id, 2);
        assert_eq!(jms.start_aid, 1501);
        assert_eq!(jms.end_aid, 1859);

        let agl = find_drama_demo_anim_info("agl_pos").expect("agl_pos must exist");
        assert_eq!(agl.kind_id, 0x0D00);
        assert_eq!(agl.start_aid, 3501);
        assert_eq!(agl.end_aid, 3531);

        let (anm, cls) =
            derive_demo_actor_anm_and_cls_paths("data/demo/haka_agl/haka_agl.dds", "agl_pos");
        assert_eq!(anm, "data/demo/haka_agl/agl.anm");
        assert_eq!(cls.as_deref(), Some("data/demo/haka_agl/agl.cls"));

        let (vtr_anm, vtr_cls) =
            derive_demo_actor_anm_and_cls_paths("data/demo/vtr/vtr.dds", "hhl_jms_pos");
        assert_eq!(vtr_anm, "data/demo/vtr/hhh_jms.anm");
        assert_eq!(vtr_cls.as_deref(), Some("data/demo/vtr/hhh_jms.cls"));
    }

    #[test]
    fn test_light_decay_ramp_and_fov_scales() {
        assert_eq!(demo_light_decay_bounds(800.0), (500.0, 2000.0));
        assert_eq!(demo_light_decay_bounds(1800.0), (1000.0, 3000.0));
        assert_eq!(demo_light_decay_bounds(20000.0), (3000.0, 7500.0));

        assert!((demo_fov_scale_for_demo_no(1) - 1.0).abs() < 1e-6);
        assert!((demo_fov_scale_for_demo_no(7) - 1.25).abs() < 1e-6);
        assert!((demo_fov_scale_for_demo_no(93) - 1.4).abs() < 1e-6);
        assert!((demo_fov_scale_for_demo_no(100) - 1.35).abs() < 1e-6);
    }

    #[test]
    fn test_drama_demo_player_with_retail_tenbou_and_haka_agl() {
        let root = "../../Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2";
        let tenbou_path = format!("{root}/data/demo/tenbou/tenbou.dds");
        if !std::path::Path::new(&tenbou_path).exists() {
            return;
        }

        let info = find_drama_demo_by_id("tenbou").expect("tenbou in catalog");
        assert_eq!(info.voice_afs_index(), Some(0));
        let dds_bytes = std::fs::read(&tenbou_path).expect("read tenbou.dds");
        let dds = parse_demo_dds(&dds_bytes).expect("parse tenbou.dds");

        let mut player = DramaDemoPlayer::new(*info, dds, false);
        assert!(player.locks_gameplay_input());
        assert_eq!(player.actors.len(), 1);
        assert_eq!(player.actors[0].actor_name, "hhh_jms_pos");
        assert!(!player.voice_started);

        player.tick(750.0 / 30.0, true);
        assert!(player.voice_started);
        assert!(player.pending_voice_trigger);
        assert_eq!(player.active_subtitle_msg_id, Some(15));
        assert!(player.camera.fov_y_radians > 0.1 && player.camera.fov_y_radians < 2.5);
        let view = player.camera.view_matrix();
        assert!(!view.is_nan());

        player.skip();
        assert_eq!(player.step, DramaDemoStep::Finished);
        assert!(!player.locks_gameplay_input());
        assert!(player.pending_voice_stop);
        let (final_pos, final_yaw) = player
            .player_handoff_transform()
            .expect("James handoff transform");
        assert!(final_pos.is_finite());
        assert!((final_yaw - std::f32::consts::PI).abs() < 1e-5);
    }

    #[test]
    fn test_all_drama_demo_catalog_assets_exist_and_parse() {
        let root = "../../Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2";
        if !std::path::Path::new(&format!("{root}/data/demo/tenbou/tenbou.dds")).exists() {
            return;
        }

        for entry in DRAMA_DEMO_CATALOG {
            let dds_full = format!("{root}/{}", entry.dds_path);
            let dds_bytes = std::fs::read(&dds_full)
                .unwrap_or_else(|e| panic!("Missing .dds for {}: {} ({e})", entry.id, dds_full));
            let dds = parse_demo_dds(&dds_bytes)
                .unwrap_or_else(|e| panic!("Failed to parse {}: {e}", entry.dds_path));
            assert!(dds.header.total_frames > 0, "{} has 0 frames", entry.id);

            let mes_full = format!("{root}/{}", entry.mes_path);
            let mes_bytes = std::fs::read(&mes_full)
                .unwrap_or_else(|e| panic!("Missing .mes for {}: {} ({e})", entry.id, mes_full));
            let mes = sh2_assets::formats::mes::parse_mes(&mes_bytes)
                .unwrap_or_else(|e| panic!("Failed to parse {}: {e}", entry.mes_path));
            assert!(!mes.messages.is_empty(), "{} has empty .mes", entry.id);
        }
    }
}
