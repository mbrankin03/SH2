//! 2D HUD telemetry & `.mes` subtitle rendering for 3D Stage Exploration & Combat Arena (`SH2-13` / `SH2-57` / `SH2-63`).

use crate::state::AppState;
use sh2_core::DRAMA_DEMO_CATALOG;
use sh2_ui::menu::TestRoomKind;

pub(super) fn render_combat_arena_hud(
    state: &mut AppState,
    encoder: &mut wgpu::CommandEncoder,
    surface_view: &wgpu::TextureView,
) {
    state.begin_hud_overlay();

    let yellow = [245, 210, 65, 255];
    let cyan = [105, 215, 235, 255];
    let green = [120, 230, 150, 255];
    let white = [225, 225, 225, 255];
    let muted = [165, 165, 170, 255];

    render_camera_and_player_telemetry(state, yellow, green, cyan, white, muted);

    if state.active_test_room == TestRoomKind::CombatArena {
        render_combat_status_panel(state, yellow, cyan, green, muted);
    }

    render_cutscene_telemetry_and_subtitles(state, yellow, cyan, muted);

    state.finish_hud_overlay(encoder, surface_view);
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
    let cam_eye = state.sim_core.camera.curr_snapshot.eye;
    let cam_tgt = state.sim_core.camera.curr_snapshot.target;
    let cam_fov = state.sim_core.camera.curr_snapshot.fov_y.to_degrees();
    let active_rec = state.sim_core.camera.active_road_index;
    let mv_type = state.sim_core.camera.mv_type;
    let room_label = state.active_test_room.label();

    state.menu_rasterizer.draw_text(
        &format!(
            "[ROOM: {}] CAMERA & PLAYER TELEMETRY (F1/F3 Toggle)",
            room_label
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
            "CAMERA EYE: X: {:.1}  Y: {:.1}  Z: {:.1}",
            cam_eye.x, cam_eye.y, cam_eye.z
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
    let rec_str = match active_rec {
        Some(idx) => format!("Record #{}", idx),
        None => "Fallback (None)".to_string(),
    };
    state.menu_rasterizer.draw_text(
        &format!(
            "CAM MODE: {:?} | ACTIVE ROAD: {} | FOV: {:.1}°",
            mv_type, rec_str, cam_fov
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
        138,
        1,
        yellow,
        true,
    );

    if !state.combat_arena.enemies.is_empty() {
        let active_idx = state.combat_enemy_idx % state.combat_arena.enemies.len();
        let enemy = &state.combat_arena.enemies[active_idx];
        let status_str = if enemy.is_dead {
            "DEAD (Dispatched)".to_string()
        } else if enemy.is_downed {
            format!(
                "DOWNED (Stomp to Finish!) [Recover in {:.1}s]",
                enemy.down_timer.max(0.0)
            )
        } else if enemy.flinch_timer > 0.0 {
            "STAGGERED / FLINCHING".to_string()
        } else if enemy.is_attacking {
            "ATTACKING / GRAPPLING".to_string()
        } else if enemy.is_aggroed {
            "ENGAGED (Approaching)".to_string()
        } else {
            "STANDBY (In Section)".to_string()
        };

        let color = if enemy.is_dead {
            muted
        } else if enemy.is_downed {
            cyan
        } else if enemy.is_attacking || enemy.is_aggroed {
            green
        } else {
            muted
        };

        state.menu_rasterizer.draw_text(
            &format!(
                "ACTIVE TARGET [{}/{}]: Enemy #{}: {} | HP: {:.0}/{:.0} | STATUS: {}",
                active_idx + 1,
                state.combat_arena.enemies.len(),
                enemy.id,
                enemy.name,
                enemy.health,
                enemy.max_health,
                status_str
            ),
            20,
            158,
            1,
            color,
            true,
        );

        state.menu_rasterizer.draw_text(
            "[Press TAB to Cycle Active Test Creature]",
            20,
            178,
            1,
            yellow,
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
        625,
        1,
        cyan,
        true,
    );

    if !state.combat_arena.enemies.is_empty() {
        let active_idx = state.combat_enemy_idx % state.combat_arena.enemies.len();
        let enemy = &state.combat_arena.enemies[active_idx];
        let enemy_clip = enemy.anim.current_clip();
        let enemy_anim_state = format!("{:?}", enemy.anim.state());
        state.menu_rasterizer.draw_text(
            &format!(
                "[ENEMY ANIM: {}] CLIP: frames {}..={} @ {:.0}fps | TIME: {:.2}s",
                enemy_anim_state,
                enemy_clip.start_frame,
                enemy_clip.end_frame,
                enemy_clip.fps,
                enemy.anim.clip_time()
            ),
            20,
            645,
            1,
            green,
            true,
        );
    }

    state.menu_rasterizer.draw_text(
        "[Controls: Right-Click/R: Ready | Space: Attack | 1..6: Weapons | Tab: Cycle Enemy | WASD: Move]",
        20,
        670,
        1,
        muted,
        true,
    );
}

fn render_cutscene_telemetry_and_subtitles(
    state: &mut AppState,
    yellow: [u8; 4],
    cyan: [u8; 4],
    muted: [u8; 4],
) {
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
                "[IN-ENGINE 3D CUTSCENE #{}: {}] FRAME {:04.0}/{:04} | FOV {:.1}° ({}) | [V] Aspect Mode  [Esc/G] Skip  [,/.] Prev/Next",
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
                "ACTORS ({}) : {} | VOICE STREAM: {} (START @ {})",
                p.actors.len(),
                actors_str.join(", "),
                p.info.stream_no,
                p.info.stream_start
            ),
            20,
            595,
            1,
            cyan,
            true,
        );

        if let Some(msg_id) = p.active_subtitle_msg_id {
            let sub_text = session
                .messages
                .as_ref()
                .and_then(|m| m.get_text(msg_id as usize))
                .map(|s| s.replace('\x0C', "\n"))
                .unwrap_or_else(|| format!("[Subtitle #{}]", msg_id));
            let cx = state.menu_rasterizer.width / 2;
            let by = state.menu_rasterizer.height.saturating_sub(56);
            sh2_ui::draw_subtitle(
                &mut state.menu_rasterizer,
                None,
                &sub_text,
                cx,
                by,
                28,
                [240, 240, 240, 255],
            );
        }
    } else {
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
            692,
            1,
            muted,
            true,
        );
    }
}
