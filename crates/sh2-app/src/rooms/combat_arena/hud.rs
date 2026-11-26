//! 2D HUD telemetry & `.mes` subtitle rendering for 3D Stage Exploration & Combat Arena (`SH2-13` / `SH2-57` / `SH2-63`).

use crate::state::AppState;
use sh2_core::modern_camera::{GameplayCameraMode, OtsReticleState};
use sh2_core::DRAMA_DEMO_CATALOG;
use sh2_ui::menu::TestRoomKind;

pub(super) fn render_combat_arena_hud(
    state: &mut AppState,
    encoder: &mut wgpu::CommandEncoder,
    surface_view: &wgpu::TextureView,
) {
    state.begin_hud_overlay();

    render_subtle_aim_reticle(state);

    let yellow = [245, 210, 65, 255];
    let cyan = [105, 215, 235, 255];
    let green = [120, 230, 150, 255];
    let white = [225, 225, 225, 255];
    let muted = [165, 165, 170, 255];

    let is_campaign = state.campaign_orchestrator.is_some();

    let cutscene_locked = state
        .drama_demo_session
        .as_ref()
        .is_some_and(|s| s.player.locks_gameplay_input());

    if !is_campaign
        && state.debug_camera_hud
        && !state.story_progression_room.modal_3d_open
        && !cutscene_locked
    {
        render_camera_and_player_telemetry(state, yellow, green, cyan, white, muted);

        if state.active_test_room == TestRoomKind::CombatArena {
            render_combat_status_panel(state, yellow, cyan, green, muted);
        }
    }

    let active_cutscene_subtitle =
        if state.drama_demo_session.is_some() || (!is_campaign && state.debug_camera_hud) {
            render_cutscene_borders_and_telemetry(state, is_campaign, yellow, cyan, muted)
        } else {
            None
        };

    super::story_overlay::render_3d_story_overlay(state);

    state.finish_hud_overlay(encoder, surface_view);

    // Render 3D cutscene subtitles onto the high-resolution `1920x1440` 4:3 PS2 UI rasterizer
    // (`SH2-163`) and blit into `ps2_ui_viewport()` for crisp native-grade rendering with zero
    // horizontal stretching on widescreen/4K monitors.
    if let Some(sub_text) = active_cutscene_subtitle {
        state.inventory_rasterizer.clear();
        sh2_ui::atlas_font::draw_subtitle_ps2(
            &mut state.inventory_rasterizer,
            None,
            &sub_text,
            sh2_ui::atlas_font::PS2_SUBTITLE_COLOR_WHITE,
        );
        let vp = state.ps2_ui_viewport();
        state.finish_inventory_overlay_in_viewport(encoder, surface_view, vp);
    }
}

fn render_subtle_aim_reticle(state: &mut AppState) {
    let OtsReticleState::SubtleDot { opacity, spread } = state.sim_core.active_reticle_state()
    else {
        return;
    };
    let alpha = (opacity.clamp(0.0, 1.0) * 235.0) as u8;
    if alpha == 0 {
        return;
    }
    let (cx, cy) = (
        state.menu_rasterizer.width / 2,
        state.menu_rasterizer.height / 2,
    );
    let gap = (3.0 + spread.clamp(0.0, 1.0) * 6.0).round() as usize;
    let (col, tick_col) = ([235, 232, 220, alpha], [220, 215, 200, alpha / 2]);
    let r = &mut state.menu_rasterizer;
    r.fill_rect(cx.saturating_sub(1), cy.saturating_sub(1), 3, 3, col);
    r.fill_rect(cx.saturating_sub(gap + 2), cy, 2, 1, tick_col);
    r.fill_rect(cx + gap, cy, 2, 1, tick_col);
    r.fill_rect(cx, cy.saturating_sub(gap + 2), 1, 2, tick_col);
    r.fill_rect(cx, cy + gap, 1, 2, tick_col);
}

fn render_camera_and_player_telemetry(
    state: &mut AppState,
    yellow: [u8; 4],
    green: [u8; 4],
    cyan: [u8; 4],
    white: [u8; 4],
    muted: [u8; 4],
) {
    let p_pos = state.sim_core.scenario.player_position;
    let p_yaw = state.sim_core.scenario.player_yaw;
    let active_snap = state.sim_core.active_camera_snapshot(1.0);
    let cam_eye = active_snap.eye;
    let cam_tgt = active_snap.target;
    let cam_fov = active_snap.fov_y.to_degrees();
    let near_clip = state.sim_core.active_near_clip(1.0);
    let active_mode = state.sim_core.active_camera_mode();
    let active_rec = state.sim_core.camera.active_road_index;
    let mv_type = state.sim_core.camera.mv_type;
    let cp = super::story_progression_3d::active_3d_checkpoint(state);
    let room_label = if let Some((ref manifest, _)) = state.campaign_loaded_room {
        format!("{} ({}.map)", manifest.stage_name, manifest.room_id)
    } else if state.story_progression_room.checkpoint_3d_idx >= 2
        || state.sim_core.scenario.current_room == cp.room_id
    {
        cp.room_display_label.to_string()
    } else {
        state.active_test_room.label().to_string()
    };

    let mode_badge = match active_mode {
        GameplayCameraMode::ClassicPs2Rail => "PS2 Rail".to_string(),
        GameplayCameraMode::OverTheShoulder => format!(
            "OTS ({})",
            state.sim_core.modern_camera.ots.shoulder_side.label()
        ),
        GameplayCameraMode::FirstPerson => "First-Person".to_string(),
    };

    state.menu_rasterizer.draw_text(
        &format!(
            "[ROOM: {}] VIEW: {} | [V] Camera Mode  [X] Shoulder  [Mouse/IJKL] Look  [F1] HUD",
            room_label, mode_badge
        ),
        20,
        18,
        1,
        yellow,
        true,
    );
    state.menu_rasterizer.draw_text(
        &format!(
            "PLAYER POS: X: {:.1}  Y: {:.1}  Z: {:.1} | YAW: {:.2} rad ({:.1}°)",
            p_pos.x,
            p_pos.y,
            p_pos.z,
            p_yaw,
            p_yaw.to_degrees()
        ),
        20,
        38,
        1,
        green,
        true,
    );
    state.menu_rasterizer.draw_text(
        &format!(
            "CAMERA EYE: X: {:.1}  Y: {:.1}  Z: {:.1} | NEAR: {:.0}u | VIS: {:?}",
            cam_eye.x,
            cam_eye.y,
            cam_eye.z,
            near_clip,
            state.sim_core.player_mesh_visibility()
        ),
        20,
        58,
        1,
        cyan,
        true,
    );
    state.menu_rasterizer.draw_text(
        &format!(
            "CAMERA TGT: X: {:.1}  Y: {:.1}  Z: {:.1}",
            cam_tgt.x, cam_tgt.y, cam_tgt.z
        ),
        20,
        78,
        1,
        white,
        true,
    );
    let rec_str = active_rec.map_or_else(
        || "Fallback (None)".to_string(),
        |idx| format!("Record #{idx}"),
    );
    let cam_src_str = match (
        state.sim_core.camera.active_room_id.as_deref(),
        state.sim_core.camera.active_cam_source,
    ) {
        (Some(room), Some(src)) => format!("{room}.cam ({})", src.label()),
        (Some(room), None) => format!("{room}.cam"),
        _ => "Synthetic".to_string(),
    };
    state.menu_rasterizer.draw_text(
        &format!(
            "CAM: {cam_src_str} | ACTIVE: {} (PS2 {mv_type:?}, {rec_str}) | FOV: {cam_fov:.1}°",
            active_mode.label()
        ),
        20,
        98,
        1,
        muted,
        true,
    );
    state.menu_rasterizer.draw_text(
        &format!("{} [B: Pack | N: Stage]", state.bgm_manager.hud_summary()),
        20,
        116,
        1,
        cyan,
        true,
    );
    let tex_quality = state
        .menu_ctrl
        .options_menu
        .draft_config
        .video
        .texture_quality;
    let tex_color = match tex_quality {
        sh2_core::config::WorldTextureQuality::Ai4kMaster => green,
        sh2_core::config::WorldTextureQuality::Sh2eeHd => cyan,
        sh2_core::config::WorldTextureQuality::OriginalPs2 => yellow,
    };
    state.menu_rasterizer.draw_text(
        &format!(
            "TEXTURES: {} [T: Cycle 4K AI / SH2:EE / PS2]",
            tex_quality.label()
        ),
        20,
        134,
        1,
        tex_color,
        true,
    );
}

fn render_combat_status_panel(
    state: &mut AppState,
    yellow: [u8; 4],
    cyan: [u8; 4],
    green: [u8; 4],
    muted: [u8; 4],
) {
    let p = &state.combat_arena.player;
    let weapon_str = format!("{:?}", p.weapon);
    let ammo_str = if p.weapon.is_firearm() {
        format!("AMMO: {} / {}", p.loaded_ammo(), p.spare_ammo())
    } else {
        "MELEE".to_string()
    };
    let mode_str = format!("{:?}", p.mode);
    let atk_str = if let Some(atk) = p.active_attack {
        format!("ATTACKING (AP: {:.0}, SP: {:.0})", atk.ap, atk.sp)
    } else if p.mode == sh2_core::combat::CombatMode::Ready {
        "AIM / READY".to_string()
    } else {
        "IDLE".to_string()
    };

    state.menu_rasterizer.draw_text(
        &format!(
            "[COMBAT ARENA] WEAPON: {} | {} | MODE: {} [{}]",
            weapon_str, ammo_str, mode_str, atk_str
        ),
        20,
        154,
        1,
        yellow,
        true,
    );

    if !state.combat_arena.ai.enemies.is_empty() {
        let active_idx = state.combat_enemy_idx % state.combat_arena.ai.enemies.len();
        let enemy = &state.combat_arena.ai.enemies[active_idx];
        let stats = enemy.stats();
        let (status_str, color) = if enemy.is_dead() {
            ("DEAD (Dispatched)".to_string(), muted)
        } else if enemy.is_downed() {
            (
                format!(
                    "DOWNED (Stomp to Finish!) [Recover in {:.1}s]",
                    stats.down_timer.max(0.0)
                ),
                cyan,
            )
        } else if stats.flinch_timer > 0.0 {
            ("STAGGERED / FLINCHING".to_string(), muted)
        } else if enemy.is_attacking() {
            ("ATTACKING / GRAPPLING".to_string(), green)
        } else if enemy.is_aggroed() {
            ("ENGAGED (Approaching)".to_string(), green)
        } else {
            ("STANDBY (In Section)".to_string(), muted)
        };

        state.menu_rasterizer.draw_text(
            &format!(
                "ACTIVE TARGET [{}/{}]: Enemy #{}: {} | HP: {:.0}/{:.0} | STATUS: {} [TAB: Cycle]",
                active_idx + 1,
                state.combat_arena.ai.enemies.len(),
                enemy.id(),
                enemy.name(),
                stats.health,
                stats.max_health,
                status_str
            ),
            20,
            174,
            1,
            color,
            true,
        );
    }

    let cur_anim_clip = state.sim_core.animation.current_clip();
    let anim_state_name = format!("{:?}", state.sim_core.animation.state());
    state.menu_rasterizer.draw_text(
        &format!(
            "[JAMES ANIM: {}] CLIP: frames {}..={} @ {:.0}fps | TIME: {:.2}s",
            anim_state_name,
            cur_anim_clip.start_frame,
            cur_anim_clip.end_frame,
            cur_anim_clip.fps,
            state.sim_core.animation.clip_time()
        ),
        20,
        592,
        1,
        cyan,
        true,
    );

    if !state.combat_arena.ai.enemies.is_empty() {
        let active_idx = state.combat_enemy_idx % state.combat_arena.ai.enemies.len();
        let enemy = &state.combat_arena.ai.enemies[active_idx];
        let enemy_clip = enemy.anim().current_clip();
        let enemy_anim_state = format!("{:?}", enemy.anim().state());
        state.menu_rasterizer.draw_text(
            &format!(
                "[ENEMY ANIM: {}] CLIP: frames {}..={} @ {:.0}fps | TIME: {:.2}s",
                enemy_anim_state,
                enemy_clip.start_frame,
                enemy_clip.end_frame,
                enemy_clip.fps,
                enemy.anim().clip_time()
            ),
            620,
            592,
            1,
            green,
            true,
        );
    }
}

fn render_cutscene_borders_and_telemetry(
    state: &mut AppState,
    is_campaign: bool,
    yellow: [u8; 4],
    cyan: [u8; 4],
    muted: [u8; 4],
) -> Option<String> {
    if let Some(ref session) = state.drama_demo_session {
        let p = &session.player;
        // Render authentic 4:3 pillarbox / letterbox borders (`DisableCutsceneBorders` & `SetBlackPillarBoxes`, `SH2-77`)
        let border_bars = sh2_core::CutsceneBorderBars::evaluate(
            state.menu_rasterizer.width as f32,
            state.menu_rasterizer.height as f32,
            p.widescreen_mode,
            p.border_mode,
        );
        for bar in border_bars
            .letterbox_bars
            .iter()
            .chain(border_bars.pillarbox_bars.iter())
        {
            if bar.is_non_empty() {
                state.menu_rasterizer.fill_rect(
                    bar.x0.floor().max(0.0) as usize,
                    bar.y0.floor().max(0.0) as usize,
                    bar.width().ceil() as usize,
                    bar.height().ceil() as usize,
                    [0, 0, 0, 255],
                );
            }
        }

        // Suppress all debug telemetry during Campaign cutscenes (`SH2-163`).
        if !is_campaign {
            let actors_str: Vec<String> = p
                .actors
                .iter()
                .map(|a| {
                    let vis_tag = if a.culled_offscreen_4x3 {
                        "CULL-4:3"
                    } else if a.visible {
                        "Y"
                    } else {
                        "N"
                    };
                    format!("{}(aid={},vis={})", a.actor_name, a.current_aid, vis_tag)
                })
                .collect();
            state.menu_rasterizer.draw_text(
                &format!(
                    "[3D CUTSCENE #{}: {}] FRAME {:04.0}/{:04} | FOV {:.1}° ({}) | [←/→] Scrub ±5s  [V] Aspect  [,/.] Prev/Next  [Esc/G] Skip",
                    p.info.demo_no,
                    p.info.title,
                    p.scene_frame.floor(),
                    p.total_frames(),
                    p.camera.fov_y_radians.to_degrees(),
                    p.widescreen_mode.label(),
                ),
                20,
                575,
                1,
                yellow,
                true,
            );
            state.menu_rasterizer.draw_text(
                &format!(
                    "ACTORS ({}) : {} | VOICE: {} (@ {}) | ToD: {} | DoF: ({:.2},{:.2}) w={:.2} | Blur: {:.0}",
                    p.actors.len(),
                    actors_str.join(", "),
                    p.info.stream_no,
                    p.info.stream_start,
                    p.atmosphere.time_of_day_preset,
                    p.special_fx.dof_tap_offset_px[0],
                    p.special_fx.dof_tap_offset_px[1],
                    p.special_fx.dof_strength,
                    p.special_fx.raw_motion_blur_factor,
                ),
                20,
                595,
                1,
                cyan,
                true,
            );
        }

        return p.active_subtitle_msg_id.map(|msg_id| {
            session
                .messages
                .as_ref()
                .and_then(|m| m.get_text(msg_id as usize))
                .map(|s| s.replace('\x0C', "\n"))
                .unwrap_or_else(|| format!("[Subtitle #{}]", msg_id))
        });
    } else if !is_campaign && !state.story_progression_room.modal_3d_open {
        let demo_entry =
            &DRAMA_DEMO_CATALOG[state.selected_drama_demo_idx % DRAMA_DEMO_CATALOG.len()];
        state.menu_rasterizer.draw_text(
            &format!(
                "[3D CUTSCENE READY [{}/{}]: {} (demo_no={}) — Press [G] to Play, [, / .] to Cycle]",
                state.selected_drama_demo_idx + 1,
                DRAMA_DEMO_CATALOG.len(),
                demo_entry.title,
                demo_entry.demo_no
            ),
            20,
            612,
            1,
            muted,
            true,
        );
    }
    None
}
