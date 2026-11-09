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
    SubtitleWindow, DRAMA_DEMO_CATALOG, SUBTITLES_BP_EDI_1, SUBTITLES_BP_EDI_2, SUBTITLES_DEAI,
    SUBTITLES_DEAI_C, SUBTITLES_FIRE_AGL, SUBTITLES_FIRST_BATTLE, SUBTITLES_FIRST_TOILET,
    SUBTITLES_FUKATSU_A, SUBTITLES_GEKIJOU_A, SUBTITLES_GERO_EDI, SUBTITLES_HAKA_AGL,
    SUBTITLES_HEI_LAU, SUBTITLES_KAGIKERI, SUBTITLES_KILLER_EDI, SUBTITLES_KNIFE_AGL,
    SUBTITLES_MAR_DEAI_FULL, SUBTITLES_MAR_SAIKAI, SUBTITLES_MRY_YARINAOSHI_A, SUBTITLES_PAPA_AGL,
    SUBTITLES_PIANO_LAU, SUBTITLES_PIANO_LAU_E, SUBTITLES_RADIO_GERO, SUBTITLES_TENBOU,
    SUBTITLES_TOMATTA, SUBTITLES_VTR,
};
pub use player::{CutsceneActorRuntime, CutsceneCameraSnapshot, DramaDemoPlayer, DramaDemoStep};

/// Native keyframe rate of Silent Hill 2 `.dds`, `.anm`, and `.cls` cutscene data (30.0 FPS).
pub const CUTSCENE_NATIVE_FPS: f32 = 30.0;

/// Base stream number offset for `data/sound/adx/voice/voice.afs` (`stream_no = 60000 + afs_index` in 0-based AFS indexing).
pub const VOICE_AFS_STREAM_BASE: i32 = 60000;

#[cfg(test)]
mod tests {
    use super::*;
    use sh2_assets::formats::demo::parse_demo_dds;

    #[test]
    fn test_drama_demo_anim_info_table_integrity() {
        assert_eq!(DRAMA_DEMO_ANIM_INFO.len(), 84);
        let jms = find_drama_demo_anim_info("hhh_jms_pos").expect("hhh_jms_pos must exist");
        assert_eq!(jms.kind_id, 0x0103);
        assert_eq!(jms.model_id, 0);
        assert_eq!(jms.start_aid, 1000);
        assert_eq!(jms.end_aid, 1165);

        let agl = find_drama_demo_anim_info("agl_pos").expect("agl_pos must exist");
        assert_eq!(agl.kind_id, 0x0107);
        assert_eq!(agl.start_aid, 3500);
        assert_eq!(agl.end_aid, 3531);

        let (anm, cls) =
            derive_demo_actor_anm_and_cls_paths("data/demo/haka_agl/haka_agl.dds", "agl_pos");
        assert_eq!(anm, "data/demo/haka_agl/agl.anm");
        assert_eq!(cls.as_deref(), Some("data/demo/haka_agl/agl.cls"));

        let (hei_anm, hei_cls) =
            derive_demo_actor_anm_and_cls_paths("data/demo/hei_lau/hei_lau_a.dds", "hhl_jms_pos");
        assert_eq!(hei_anm, "data/demo/hei_lau/hhl_jms.anm");
        assert_eq!(hei_cls.as_deref(), Some("data/demo/hei_lau/hhl_jms.cls"));
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
        assert_eq!(info.voice_afs_index(), Some(118));
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
        assert_eq!(player.active_subtitle_msg_id, Some(7));
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

            let map_full = format!("{root}/data/{}", entry.stage_map_path);
            assert!(
                std::path::Path::new(&map_full).exists(),
                "Missing stage .map for {}: {}",
                entry.id,
                map_full
            );
        }

        // Verify multi-segment `.anm`/`.cls` frame offsets (`mar_deai_e` = 1546, `papa_agl` = 438,
        // `piano_lau` Laura = 226, `piano_lau_e` James = 1679 & Laura = 1905)
        let deai_e = find_drama_demo_by_id("deai").expect("deai in catalog");
        assert_eq!(deai_e.anm_frame_offset, 1546);

        let papa_b = find_drama_demo_by_id("papa_agl").expect("papa_agl in catalog");
        assert_eq!(papa_b.anm_frame_offset, 438);

        let piano_b = find_drama_demo_by_id("piano_lau").expect("piano_lau in catalog");
        let piano_b_dds =
            parse_demo_dds(&std::fs::read(format!("{root}/{}", piano_b.dds_path)).unwrap())
                .unwrap();
        let piano_b_player = DramaDemoPlayer::new(*piano_b, piano_b_dds, false);
        let jms_b = piano_b_player
            .actors
            .iter()
            .find(|a| a.actor_name == "hhh_jms_pos")
            .unwrap();
        let lau_b = piano_b_player
            .actors
            .iter()
            .find(|a| a.actor_name == "lau_pos")
            .unwrap();
        assert_eq!(jms_b.anm_frame_offset, 0);
        assert_eq!(lau_b.anm_frame_offset, 226);

        let piano_e = find_drama_demo_by_id("piano_lau_e").expect("piano_lau_e in catalog");
        let piano_e_dds =
            parse_demo_dds(&std::fs::read(format!("{root}/{}", piano_e.dds_path)).unwrap())
                .unwrap();
        let piano_e_player = DramaDemoPlayer::new(*piano_e, piano_e_dds, false);
        let jms_e = piano_e_player
            .actors
            .iter()
            .find(|a| a.actor_name == "hhh_jms_pos")
            .unwrap();
        let lau_e = piano_e_player
            .actors
            .iter()
            .find(|a| a.actor_name == "lau_pos")
            .unwrap();
        assert_eq!(jms_e.anm_frame_offset, 1679);
        assert_eq!(lau_e.anm_frame_offset, 1905);
        assert_eq!(lau_e.cls_frame_offset, 1905);

        // Verify NPC `.anm` gameplay base-clip prefix vs `.cls` facial morph offset separation:
        // In `haka_agl`, Angela's `agl.anm` prepends 156 gameplay frames (`4169 - 4013 = 156`),
        // whereas `agl.cls` starts at frame 0 (`4013` frames).
        let haka = find_drama_demo_by_id("haka_agl").expect("haka_agl in catalog");
        let haka_dds =
            parse_demo_dds(&std::fs::read(format!("{root}/{}", haka.dds_path)).unwrap()).unwrap();
        let haka_player = DramaDemoPlayer::new(*haka, haka_dds, false);
        let agl_haka = haka_player
            .actors
            .iter()
            .find(|a| a.actor_name == "agl_pos")
            .unwrap();
        assert_eq!(agl_haka.anm_frame_offset, 156);
        assert_eq!(agl_haka.cls_frame_offset, 0);

        // In `papa_agl` (Part 2, starting after 438-frame `papa_agl_a`), Angela's `agl.anm` has
        // `438 + 69 = 507` offset, while `agl.cls` has `438` offset.
        let papa_dds =
            parse_demo_dds(&std::fs::read(format!("{root}/{}", papa_b.dds_path)).unwrap()).unwrap();
        let papa_player = DramaDemoPlayer::new(*papa_b, papa_dds, false);
        let agl_papa = papa_player
            .actors
            .iter()
            .find(|a| a.actor_name == "agl_pos")
            .unwrap();
        assert_eq!(agl_papa.anm_frame_offset, 507);
        assert_eq!(agl_papa.cls_frame_offset, 438);
    }
}
