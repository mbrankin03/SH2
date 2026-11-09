//! 2D HUD telemetry, 7-channel AIX VU meter, and SH2-62 Cut/Rare Audio panel for the Audio Debug Room (`SH2-60` / `SH2-61` / `SH2-62`).

use crate::state::AppState;
use sh2_audio::{
    bgm_page_entry, compute_se_call_pos_distance_f, find_bgm_file_by_load_id, BGM_MAX_TRACKS,
    CUT_AND_RARE_AUDIO_CATALOG, E3_PROTOTYPE_SDB_CATALOG, FADE_TIME_SECS,
    LAURA_LETTER_VOICEOVER_SLICES, STAGE_SOUND_DATA,
};

pub(super) fn render_audio_debug_hud(
    state: &mut AppState,
    encoder: &mut wgpu::CommandEncoder,
    surface_view: &wgpu::TextureView,
) {
    state.begin_hud_overlay();

    let yellow = [245, 210, 65, 255];
    let cyan = [105, 215, 235, 255];
    let green = [120, 230, 150, 255];
    let white = [235, 235, 235, 255];
    let orange = [255, 165, 75, 255];
    let muted = [165, 165, 175, 255];

    render_header_and_stage_status(state, yellow, green, white, cyan, orange);
    render_aix_layer_vu_table(state, yellow, green, cyan, muted);
    render_sfx_and_spatial_panel(state, cyan, green, white);
    render_cut_and_prototype_panel(state, yellow, orange, cyan, green, white, muted);

    state.finish_hud_overlay(encoder, surface_view);
}

fn render_header_and_stage_status(
    state: &mut AppState,
    yellow: [u8; 4],
    green: [u8; 4],
    white: [u8; 4],
    cyan: [u8; 4],
    orange: [u8; 4],
) {
    let (stg, preset_desc) = state.audio_debug.current_stage_preset();
    let p_pos = state.sim_core.scenario.player_position;
    let mgr = &state.bgm_manager;

    let pack_str = match mgr.audio_pack_source {
        sh2_core::AudioPackPreference::Auto => "AUTO -> SH2:EE LOSSLESS 48 kHz (sh2e/sound/adx/)",
        sh2_core::AudioPackPreference::Sh2eeLossless => "SH2:EE LOSSLESS 48 kHz (sh2e/sound/adx/)",
        sh2_core::AudioPackPreference::Ps2Original => {
            "ORIGINAL PS2 / PC 24-44.1 kHz (data/sound/adx/)"
        }
    };

    state.menu_rasterizer.draw_text(
        "SILENT HILL 2 - AUDIO & MULTI-CHANNEL AIX SPATIAL MIXING STUDIO [SH2-60]",
        20,
        16,
        2,
        yellow,
        true,
    );

    state.menu_rasterizer.draw_text(
        &format!(
            "SOURCE PACK [B] : {} | SAMPLE RATE: {} Hz",
            pack_str, mgr.loaded_sample_rate
        ),
        20,
        44,
        1,
        green,
        true,
    );

    state.menu_rasterizer.draw_text(
        &format!("STAGE PRESET [N/P]: {}", preset_desc),
        20,
        62,
        1,
        white,
        true,
    );

    let page_entry = bgm_page_entry(mgr.current_page);
    let file_info = find_bgm_file_by_load_id(page_entry.load_id)
        .map(|f| {
            format!(
                "{} ({})",
                f.path,
                if f.is_adx {
                    "Single-Stream ADX"
                } else {
                    "Multi-Layer AIX"
                }
            )
        })
        .unwrap_or_else(|| "Silence (-1, -1)".to_string());

    state.menu_rasterizer.draw_text(
        &format!(
            "ACTIVE BGM PAGE [PgUp/PgDn]: Page {} (Target: {}) | load_id: {} | start_id: {} | Master Vol: {:.0}%",
            mgr.current_page,
            mgr.target_page,
            page_entry.load_id,
            page_entry.start_id,
            mgr.file_master_volume * 100.0
        ),
        20,
        80,
        1,
        cyan,
        true,
    );

    let disk_path_str = mgr
        .loaded_full_path
        .as_ref()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "None".to_string());

    state.menu_rasterizer.draw_text(
        &format!("STREAM FILE : {} -> {}", file_info, disk_path_str),
        20,
        98,
        1,
        white,
        true,
    );

    let total_boxes = STAGE_SOUND_DATA
        .get(stg as usize)
        .map(|s| s.len().saturating_sub(1))
        .unwrap_or(0);
    let matched_str = if mgr.last_area_eval.used_default_record {
        "Fallback Record #0 (Outside Boxes)".to_string()
    } else {
        format!(
            "Matched Boxes: {:?}",
            mgr.last_area_eval.matched_record_indices
        )
    };

    state.menu_rasterizer.draw_text(
        &format!(
            "PLAYER POS (WASD): X: {:.0}  Y: {:.0}  Z: {:.0} | Room ID: 0x{:02X} | Stage Boxes: {} ({})",
            p_pos.x, p_pos.y, p_pos.z, mgr.room_id, total_boxes, matched_str
        ),
        20,
        116,
        1,
        green,
        true,
    );

    state.menu_rasterizer.draw_text(
        &format!(
            "SITUATION [T]: {} -> Resolved BgmPageSet: {} | Solo Layer [1-7, 0=Auto]: {}",
            state.audio_debug.situation_mode_label(),
            mgr.situation_page,
            state
                .audio_debug
                .solo_track
                .map(|i| format!("SOLO T{}", i))
                .unwrap_or_else(|| "OFF (Spatial _SOUND_DATA Mix)".to_string())
        ),
        20,
        134,
        1,
        orange,
        true,
    );
}

fn render_aix_layer_vu_table(
    state: &mut AppState,
    yellow: [u8; 4],
    green: [u8; 4],
    cyan: [u8; 4],
    muted: [u8; 4],
) {
    let mgr = &state.bgm_manager;

    state.menu_rasterizer.draw_text(
        "--- 7-CHANNEL AIX / ADX LAYER MIXER (SeBgmManager 0x516080 & sndGetSoundAryByCharaPos 0x5187E0) ---",
        20,
        162,
        1,
        yellow,
        true,
    );

    for i in 0..BGM_MAX_TRACKS {
        let y = 182 + i * 22;
        let trk = &mgr.tracks[i];
        let (cmd_in, cmd_out) = mgr.last_area_eval.track_fades[i];
        let is_present = i < mgr.loaded_layer_count;

        let bar_blocks = ((trk.volume.clamp(0.0, 1.0) * 20.0).round() as usize).min(20);
        let mut bar = String::with_capacity(22);
        bar.push('[');
        for b in 0..20 {
            if b < bar_blocks {
                bar.push('#');
            } else {
                bar.push('.');
            }
        }
        bar.push(']');

        let fade_in_sec = FADE_TIME_SECS
            .get(trk.fade_in as usize)
            .copied()
            .unwrap_or(0.0);
        let fade_out_sec = FADE_TIME_SECS
            .get(trk.fade_out as usize)
            .copied()
            .unwrap_or(0.0);
        let eff_pct = trk.volume * mgr.file_master_volume * 100.0;

        let row_color = if !is_present {
            muted
        } else if state.audio_debug.solo_track == Some(i) {
            yellow
        } else if trk.volume > 0.01 {
            green
        } else {
            cyan
        };

        let status_tag = if !is_present {
            "N/A (Not in container)".to_string()
        } else {
            format!(
                "{:?} | AreaCmd:(in={}, out={}) | Ramp:(in={:.1}s, out={:.1}s)",
                trk.status, cmd_in, cmd_out, fade_in_sec, fade_out_sec
            )
        };

        state.menu_rasterizer.draw_text(
            &format!(
                "LAYER T{} [Key {}]: {} {:>5.1}% (Eff: {:>5.1}%) | {}",
                i,
                i + 1,
                bar,
                trk.volume * 100.0,
                eff_pct,
                status_tag
            ),
            20,
            y,
            1,
            row_color,
            true,
        );
    }
}

fn render_sfx_and_spatial_panel(
    state: &mut AppState,
    cyan: [u8; 4],
    green: [u8; 4],
    white: [u8; 4],
) {
    let p_pos = state.sim_core.scenario.player_position;
    let eng_status = if let Some(ref eng) = state.audio_engine {
        format!(
            "ACTIVE (Kira Playing Layers: {})",
            eng.active_bgm_layer_count()
        )
    } else {
        "OFFLINE (No Audio Output Device)".to_string()
    };

    state.menu_rasterizer.draw_text(
        &format!(
            "AUDIO ENGINE : {} | DSP REVERB [E]: {:?} | LAST SFX [U/I/O/L/J/H]: {}",
            eng_status,
            state.audio_debug.reverb_preset(),
            state.audio_debug.last_sfx_triggered
        ),
        20,
        352,
        1,
        cyan,
        true,
    );

    let bank_samples = state
        .audio_bank
        .as_ref()
        .map(|b| b.sddata_entry_count())
        .unwrap_or(0);
    let bank_rate = state
        .audio_bank
        .as_ref()
        .map(|b| b.primary_sample_rate())
        .unwrap_or(0);
    let spatial_eval = compute_se_call_pos_distance_f(
        p_pos,
        state.sim_core.scenario.player_yaw,
        p_pos
            + glam::Vec3::new(1.0, 0.0, 1.0).normalize() * state.audio_debug.test_emitter_distance,
        1.0,
    );

    state.menu_rasterizer.draw_text(
        &format!(
            "SFX BANK (SH2-61): {} / 417 WAVs ({} Hz) | Footstep Surface [M]: {} | 3D Shot [H]: dist={:.0}u (v={:.2}, gain={:.0}%, pan={:+.2})",
            bank_samples,
            bank_rate,
            state.audio_debug.test_surface_material.label(),
            state.audio_debug.test_emitter_distance,
            spatial_eval.proximity_v,
            spatial_eval.effective_gain * 100.0,
            spatial_eval.stereo_pan
        ),
        20,
        372,
        1,
        green,
        true,
    );

    state.menu_rasterizer.draw_text(
        &format!(
            "3D SOUND AREA WIREFRAMES [V]: {} | E3 PROTOTYPE .SDB OVERLAY [G]: {} ({} - {} boxes)",
            if state.audio_debug.show_sound_boxes_3d {
                "ON"
            } else {
                "OFF"
            },
            if state.audio_debug.show_proto_tgs_sdb_boxes {
                "ON (Magenta)"
            } else {
                "OFF"
            },
            E3_PROTOTYPE_SDB_CATALOG[state.audio_debug.proto_sdb_index].filename,
            E3_PROTOTYPE_SDB_CATALOG[state.audio_debug.proto_sdb_index].record_count
        ),
        20,
        392,
        1,
        white,
        true,
    );
}

fn render_cut_and_prototype_panel(
    state: &mut AppState,
    yellow: [u8; 4],
    orange: [u8; 4],
    cyan: [u8; 4],
    green: [u8; 4],
    white: [u8; 4],
    muted: [u8; 4],
) {
    let cut_entry = CUT_AND_RARE_AUDIO_CATALOG[state.audio_debug.cut_catalog_index];
    let laura_slice = LAURA_LETTER_VOICEOVER_SLICES[state.audio_debug.laura_slice_index];
    let proto_sdb = E3_PROTOTYPE_SDB_CATALOG[state.audio_debug.proto_sdb_index];

    state.menu_rasterizer.draw_text(
        "--- CUT / UNUSED RETAIL AUDIO, RESTORED VOICEOVER & PS2 PROTOTYPE SOUNDBANKS [SH2-62] ---",
        20,
        418,
        1,
        yellow,
        true,
    );

    state.menu_rasterizer.draw_text(
        &format!(
            "RESTORED CUT AUDIO [R]: {} | CUT CATALOG [[ / ], Enter] ({}/{} - {}): {}",
            if state
                .menu_ctrl
                .options_menu
                .original_config
                .audio
                .enable_restored_cut_audio
            {
                "ENABLED"
            } else {
                "DISABLED (Vanilla)"
            },
            state.audio_debug.cut_catalog_index + 1,
            CUT_AND_RARE_AUDIO_CATALOG.len(),
            cut_entry.category.label(),
            cut_entry.title
        ),
        20,
        438,
        1,
        orange,
        true,
    );

    state.menu_rasterizer.draw_text(
        &format!(
            "LAURA LETTER VOICEOVER [K] (voice.afs[119] / SCENE_45): Page {}/11 ({:.2}s..{:.2}s) \"{}\"",
            laura_slice.page_index + 1,
            laura_slice.start_sec,
            laura_slice.end_sec,
            laura_slice.subtitle_excerpt
        ),
        20,
        458,
        1,
        cyan,
        true,
    );

    state.menu_rasterizer.draw_text(
        &format!(
            "PS2 PROTOTYPE .SDB [G]: {} ({} B, {} recs) - {} | July 13 SOUND.DAT: 79 VAG banks (1,422 samples) + 33 .sfc",
            proto_sdb.filename, proto_sdb.byte_length, proto_sdb.record_count, proto_sdb.notes
        ),
        20,
        478,
        1,
        green,
        true,
    );

    state.menu_rasterizer.draw_text(
        "[B] Swap SH2:EE 48kHz / Original PS2  |  [N/P] Prev/Next Stage Preset  |  [PgUp/PgDn] Browse All 48 BGM Pages",
        20,
        648,
        1,
        yellow,
        true,
    );
    state.menu_rasterizer.draw_text(
        "[[ / ] + Enter] Audition Cut/Rare SFX (sddata360..364 / 40000+)  |  [K] Play Mary Letter Slice  |  [R] Restored Toggle  |  [G] E3 .sdb",
        20,
        668,
        1,
        white,
        true,
    );
    state.menu_rasterizer.draw_text(
        "[WASD] Walk  |  [M] Footstep Surface  |  [T] Situation  |  [1-7, 0] Solo Layer  |  [U/I/O/L/J/H] SFX & 3D Shot  |  [E] Reverb  |  [V] Boxes",
        20,
        688,
        1,
        muted,
        true,
    );
}
