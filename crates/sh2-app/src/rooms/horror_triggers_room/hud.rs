//! 2D HUD telemetry and scenario selector overlay for `HorrorTriggersRoom` (`SH2-47`).

use crate::state::AppState;
use sh2_audio::compute_se_call_pos_distance_f;
use sh2_core::types::DOOR_SE_TABLE;

use super::scenarios::{active_scenario_anchor, HORROR_SCENARIOS};

pub(crate) fn render_horror_room_hud(
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

    let idx = state.horror_triggers_room.scenario_idx % HORROR_SCENARIOS.len();
    let spec = HORROR_SCENARIOS[idx];
    let anchor = active_scenario_anchor(state);
    let p_local = state.sim_core.scenario.player_position;
    let p_yaw = state.sim_core.scenario.player_yaw;
    let p_world = glam::Vec3::new(
        anchor[0] + p_local.x,
        anchor[1] + p_local.y,
        anchor[2] + p_local.z,
    );
    let emit_world = glam::Vec3::from_array(state.horror_triggers_room.active_emitter_world_pos);
    let eval = compute_se_call_pos_distance_f(
        p_world,
        p_yaw,
        emit_world,
        state.horror_triggers_room.last_se_vol,
    );

    state.menu_rasterizer.draw_text(
        "SILENT HILL 2 - STAGE EVENT ENGINE & ENVIRONMENTAL HORROR TRIGGERS CHAMBER [SH2-47]",
        20,
        16,
        2,
        yellow,
        true,
    );

    state.menu_rasterizer.draw_text(
        &format!(
            "ACTIVE SCENARIO [1-8 / Tab]: {} (Stage={:?}, Room=0x{:02X})",
            spec.title, spec.stage_id, spec.room_id
        ),
        20,
        44,
        1,
        green,
        true,
    );

    state.menu_rasterizer.draw_text(
        &format!(
            "STAGE WORLD POS: [{:.0}, {:.0}, {:.0}] | YAW: {:+.2} rad | EMITTER: [{:.0}, {:.0}, {:.0}] | 3D DIST: {:.0}u | GAIN: {:.0}% | PAN: {:+.2}",
            p_world.x,
            p_world.y,
            p_world.z,
            p_yaw,
            emit_world.x,
            emit_world.y,
            emit_world.z,
            eval.distance,
            eval.effective_gain * 100.0,
            eval.stereo_pan
        ),
        20,
        64,
        1,
        cyan,
        true,
    );

    let f = &state.sim_core.scenario.flags;
    let disp = &state.horror_triggers_room.dispatcher;
    let telemetry = match idx {
        0 => format!(
            "PRISON BATHROOM (0x73): s_thr={:.0} | Flag 648 (Stall Knocked)={} | Flag 649 (Jumpscare 0x427D Fired)={}",
            disp.prison_n_bathroom_z_threshold,
            f.get(648),
            f.get(649)
        ),
        1 => format!(
            "APT ROOM 209 WHISPER (0x28): s_thr={:.0} | Flag 547={} | Orbit Angle={:.2} rad (r=2500u) | Active SEs={:?}",
            disp.apart_w2f_room209_x_threshold,
            f.get(547),
            disp.apart_w2f_whisper_angle,
            state.horror_triggers_room.active_sounds_array()
        ),
        2 => format!(
            "APT 3F CORRIDOR FOOTSTEPS (0x1F): Trigger Z < 17000 | Flag 552={} | Step Timer={:.2}s (-700 u/s) | Active SEs={:?}",
            f.get(552),
            disp.apart_e3fe_footstep_timer,
            state.horror_triggers_room.active_sounds_array()
        ),
        3 => format!(
            "PRISON SOUTH (0x72): Invisible Prisoner Timer={:.2}s (0x4682) | Oppressor Flags 288={} 289={} 290={} (Timer={:.2}s)",
            disp.prison_s_prisoner_timer,
            f.get(288),
            f.get(289),
            f.get(290),
            disp.prison_s_bars_timer
        ),
        4 => format!(
            "BLUE CREEK CORRIDOR (0x26): corridor_armed={} (Z > -6500) | Flag 553 (0x9C6C Fired at Z < -6800)={}",
            disp.apart_w2f_corridor_armed,
            f.get(553)
        ),
        5 => format!(
            "WOOD SIDE 2F: EvProgAnyoneCry (7,1) / EvProgMonkeyKick206 (7,17) | Flag 69 (Flashlight)={} Flag 70 (TV Static)={} | prog_runner={}",
            f.get(69),
            f.get(70),
            state.sim_core.scenario.prog_runner.is_running()
        ),
        6 => format!(
            "HISTORICAL SOCIETY (0x5D / 0x5C): Staircase Timer={:.2}s (0x9C73) | Room 0x5C Flag 551 (0x4982 Fired)={}",
            disp.society_staircase_timer,
            f.get(551)
        ),
        _ => {
            let d_idx = state.horror_triggers_room.door_se_idx % DOOR_SE_TABLE.len();
            let ds = DOOR_SE_TABLE[d_idx];
            format!(
                "EVENT ENGINE SANDBOX: Key Item #1 [K]={} | Door Unlocked (Flag 701)={} | Quad Zone (Flag 700)={} | DOOR_SE[{}]: unlock=0x{:04X} lock=0x{:04X}",
                state.horror_triggers_room.sandbox_has_key,
                f.get(701),
                f.get(700),
                d_idx,
                ds.unlock,
                ds.lock
            )
        }
    };

    state
        .menu_rasterizer
        .draw_text(&telemetry, 20, 86, 1, orange, true);

    state.menu_rasterizer.draw_text(
        "--- 8 STAGE EVENT & ENVIRONMENTAL HORROR SCENARIOS (Press [1-8] or [Tab / PgUp / PgDn]) ---",
        20,
        114,
        1,
        yellow,
        true,
    );
    for (i, item) in HORROR_SCENARIOS.iter().enumerate() {
        let y = 134 + i * 20;
        let is_cur = i == idx;
        let col = if is_cur { green } else { white };
        let prefix = if is_cur { ">> " } else { "   " };
        state
            .menu_rasterizer
            .draw_text(&format!("{prefix}{}", item.title), 20, y, 1, col, true);
    }

    state.menu_rasterizer.draw_text(
        &format!("CONTROLS FOR THIS SCENARIO: {}", spec.controls_hint),
        20,
        310,
        1,
        yellow,
        true,
    );
    state.menu_rasterizer.draw_text(
        &format!(
            "LAST EVENT LOG: {}",
            state.horror_triggers_room.last_event_log
        ),
        20,
        332,
        1,
        green,
        true,
    );

    state.menu_rasterizer.draw_text(
        "[WASD + Shift] Walk/Run in 3D  |  [Space] Step Across Threshold / Trigger Horror Event  |  [E / Enter] Inspect / Door Action",
        20,
        658,
        1,
        yellow,
        true,
    );
    state.menu_rasterizer.draw_text(
        "[1-8 / Tab] Switch Scenario  |  [R] Reset & Re-seed Room  |  [K] Secondary Action  |  [T] Door SFX / TV Static  |  [V] 3D Guides",
        20,
        678,
        1,
        muted,
        true,
    );

    state.finish_hud_overlay(encoder, surface_view);
}
