//! 2D HUD and cutaway viewport overlay rendering for [`super::PuzzleCutawayRoomController`] (`SH2-15`).

use super::CUTAWAY_PRESETS;
use crate::state::AppState;
use sh2_assets::CutawayViewportTransform;
use sh2_core::{
    format_carbon_box_code, format_wallet_safe_code, is_clock_solved, is_coin_desk_solved,
    MessageInsertState, PZ_CLOCK, PZ_DRUM, PZ_SAFE_PROG, PZ_WHEEL, PZ_WHEEL_GOAL,
};

pub(super) fn render_puzzle_cutaway_hud(
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

    state
        .menu_rasterizer
        .fill_rect(10, 10, 1260, 700, [8, 10, 15, 235]);
    state
        .menu_rasterizer
        .draw_rect_outline(10, 10, 1260, 700, 2, [70, 85, 110, 255]);

    let preset_idx = state.puzzle_cutaway_room.preset_idx % CUTAWAY_PRESETS.len();
    let (preset_pic, preset_title, preset_controls) = CUTAWAY_PRESETS[preset_idx];

    state.menu_rasterizer.draw_text(
        "SILENT HILL 2 - PUZZLE LOGIC, STAGE EVENTS (EV_PROG) & CUTAWAY OVERLAYS ROOM [SH2-15]",
        22,
        18,
        2,
        yellow,
        true,
    );
    let src_pref_label = if state.puzzle_cutaway_room.prefer_sh2ee {
        "SH2:EE HD (sh2e/pic)"
    } else {
        "Original PS2/PC (data/pic)"
    };
    state.menu_rasterizer.draw_text(
        &format!(
            "PRESET [Tab/PgUp/PgDn]: {}  |  RIDDLE LEVEL [R]: {:?}  |  ASSET SOURCE [P]: {}",
            preset_title, state.sim_core.scenario.riddle_difficulty, src_pref_label
        ),
        22,
        44,
        1,
        cyan,
        true,
    );

    // Left Viewport: 560x420 Aspect-Corrected Cutaway Still + Live Sprite Overlays
    let vp_box_x = 24usize;
    let vp_box_y = 72usize;
    let vp_box_w = 560usize;
    let vp_box_h = 420usize;
    state.menu_rasterizer.fill_rect(
        vp_box_x - 4,
        vp_box_y - 4,
        vp_box_w + 8,
        vp_box_h + 8,
        [0, 0, 0, 255],
    );
    state.menu_rasterizer.draw_rect_outline(
        vp_box_x - 4,
        vp_box_y - 4,
        vp_box_w + 8,
        vp_box_h + 8,
        2,
        yellow,
    );

    let mut vp_transform = CutawayViewportTransform {
        offset_x: 0.0,
        offset_y: 0.0,
        draw_width: vp_box_w as f32,
        draw_height: vp_box_h as f32,
        scale_x: 1.0,
        scale_y: 1.0,
    };

    if let Some(ref base_tex) = state.puzzle_cutaway_room.base_tex {
        vp_transform = base_tex.compute_viewport_transform(vp_box_w as f32, vp_box_h as f32);
        let dst_x = vp_box_x + (vp_transform.offset_x.round().max(0.0) as usize);
        let dst_y = vp_box_y + (vp_transform.offset_y.round().max(0.0) as usize);
        let dst_w = (vp_transform.draw_width.round().max(1.0) as usize).min(vp_box_w);
        let dst_h = (vp_transform.draw_height.round().max(1.0) as usize).min(vp_box_h);
        state.menu_rasterizer.blit_rgba_scaled(
            dst_x,
            dst_y,
            dst_w,
            dst_h,
            &base_tex.texture.rgba,
            base_tex.texture.width,
            base_tex.texture.height,
            1.0,
        );
    }

    // Composite all active OverlaySpriteQuads onto the Cutaway Viewport
    let pref = state.puzzle_cutaway_room.prefer_sh2ee;
    let quads: Vec<_> = state
        .sim_core
        .scenario
        .cutaway_overlay
        .frame_sprites()
        .to_vec();
    let draw_x = (vp_box_x as f32) + vp_transform.offset_x;
    let draw_y = (vp_box_y as f32) + vp_transform.offset_y;
    for quad in &quads {
        if let Some(sheet_tex) = state
            .puzzle_cutaway_room
            .sheet_cache
            .get(&(quad.sheet_name, pref))
        {
            let tris = quad.to_viewport_triangles(
                draw_x,
                draw_y,
                vp_transform.draw_width,
                vp_transform.draw_height,
            );
            state.menu_rasterizer.blit_overlay_triangles(
                &tris,
                &sheet_tex.texture.rgba,
                sheet_tex.texture.width,
                sheet_tex.texture.height,
                quad.alpha,
            );
        }
    }

    // Right Panel (x = 604..1250): Telemetry & Interactive Puzzle State
    let rx = 604usize;
    state.menu_rasterizer.draw_text(
        "--- CUTAWAY TEXTURE & SPRITE OVERLAY TELEMETRY ---",
        rx,
        74,
        1,
        yellow,
        true,
    );

    if let Some(ref bt) = state.puzzle_cutaway_room.base_tex {
        let src_tag = if bt.is_sh2ee_hd {
            "SH2:EE HD (sh2e/pic)"
        } else {
            "Original PS2/PC (data/pic)"
        };
        state.menu_rasterizer.draw_text(
            &format!(
                "BASE STILL  : {} ({}x{}, Source={})",
                preset_pic, bt.texture.width, bt.texture.height, src_tag
            ),
            rx,
            94,
            1,
            green,
            true,
        );
        state.menu_rasterizer.draw_text(
            &format!(
                "VIEWPORT    : offset=({:.1}, {:.1}), size={:.0}x{:.0}, orig_ref={}x{}",
                vp_transform.offset_x,
                vp_transform.offset_y,
                vp_transform.draw_width,
                vp_transform.draw_height,
                bt.original_width,
                bt.original_height
            ),
            rx,
            112,
            1,
            white,
            true,
        );
    } else {
        state.menu_rasterizer.draw_text(
            &format!("BASE STILL  : {} (Not found on disk)", preset_pic),
            rx,
            94,
            1,
            orange,
            true,
        );
    }

    let ov = &state.sim_core.scenario.cutaway_overlay;
    state.menu_rasterizer.draw_text(
        &format!(
            "OVERLAY KIND: {:?} | SHEET: {} | ACTIVE QUADS: {} | InCarAlpha={:.2} | ManaclesAlpha={:.2}",
            ov.kind(),
            ov.sheet_name().unwrap_or("None"),
            quads.len(),
            ov.incar_alpha(),
            ov.manacles_alpha()
        ),
        rx,
        130,
        1,
        cyan,
        true,
    );

    // StageProgramRunner (ev_prog) Status
    let pr = &state.sim_core.scenario.prog_runner;
    state.menu_rasterizer.draw_text(
        "--- STAGE EVENT ENGINE (EV_PROG / StageProgramRunner) ---",
        rx,
        158,
        1,
        yellow,
        true,
    );
    state.menu_rasterizer.draw_text(
        &format!(
            "RUNNING: {} | PLAYER LOCKED: {} | ACTIVE MSG ID: {:?} | FLAG 28 (Map Taken): {}",
            pr.is_running(),
            pr.is_player_locked(),
            pr.active_message_id(),
            state.sim_core.scenario.flags.get(28)
        ),
        rx,
        178,
        1,
        if pr.is_running() { green } else { white },
        true,
    );
    if let Some(ctx) = pr.context() {
        state.menu_rasterizer.draw_text(
            &format!(
                "ACTIVE PROG: Stage={} | ProgNo=#{} | Step={} | Timer={:.2}s | CompletionFlag={}",
                ctx.stage, ctx.prog_no, ctx.step, ctx.timer, ctx.completion_flag
            ),
            rx,
            196,
            1,
            orange,
            true,
        );
    }

    // Interactive Puzzle State Readout
    state.menu_rasterizer.draw_text(
        "--- LIVE PUZZLE STATE MACHINES (Easy / Normal / Hard / Extra) ---",
        rx,
        224,
        1,
        yellow,
        true,
    );

    let pz = &state.sim_core.scenario.puzzle_state;
    let fl = &state.sim_core.scenario.flags;
    let riddle = state.sim_core.scenario.riddle_difficulty;

    // 1. Clock State
    let clock_val = pz.read_u16(PZ_CLOCK);
    let clock_mins = (clock_val as u32) / 64;
    let clock_h = (clock_mins / 60) % 12;
    let clock_m = clock_mins % 60;
    state.menu_rasterizer.draw_text(
        &format!(
            "1. APT 208 CLOCK    : PZ_CLOCK=0x{clock_val:04X} ({clock_h:02}:{clock_m:02}) | Target=9:10 (0x88C1..0x8A3F) | Solved={}",
            is_clock_solved(pz)
        ),
        rx,
        246,
        1,
        if is_clock_solved(pz) { green } else { white },
        true,
    );

    // 2. Wallet & Safe State
    let mut wallet_ins = MessageInsertState::default();
    format_wallet_safe_code(pz, riddle, &mut wallet_ins);
    let wallet_str = wallet_ins.expand("<e0> >> <e1> << <e2> >> <e3>");
    let mut safe_dial = 0u32;
    for b in 0..5usize {
        if fl.get(103 + b) {
            safe_dial |= 1 << b;
        }
    }
    state.menu_rasterizer.draw_text(
        &format!(
            "2. APT W2F SAFE     : Wallet=\"{wallet_str}\" | Dial={} | Prog={:?} | Open(108)={}",
            safe_dial + 1,
            &pz.raw()[PZ_SAFE_PROG..PZ_SAFE_PROG + 4],
            fl.get(108)
        ),
        rx,
        268,
        1,
        if fl.get(108) { green } else { white },
        true,
    );

    // 3. Coin Desk State
    let mut holes_desc = ["(empty)", "(empty)", "(empty)", "(empty)", "(empty)"];
    for (h, slot_label) in holes_desc.iter_mut().enumerate() {
        if fl.get(126 + h) {
            *slot_label = "Snake";
        } else if fl.get(131 + h) {
            *slot_label = "OldMan";
        } else if fl.get(136 + h) {
            *slot_label = "Prisoner";
        }
    }
    let coin_solved = is_coin_desk_solved(fl, riddle);
    state.menu_rasterizer.draw_text(
        &format!(
            "3. APT 105 COINS    : Hole#{} | {:?} | Solved(142)={}",
            state.puzzle_cutaway_room.selected_coin_hole + 1,
            holes_desc,
            coin_solved
        ),
        rx,
        290,
        1,
        if coin_solved { green } else { white },
        true,
    );

    // 4. Hospital 2F Box & Wall State
    let mut box_ins = MessageInsertState::default();
    let carbon_code = format_carbon_box_code(pz, &mut box_ins);
    state.menu_rasterizer.draw_text(
        &format!(
            "4. HOSPITAL 2F BOX  : Carbon={carbon_code} | Wheels={:?} vs Goal={:?} | Locks(195/197/198)={}/{}/{}",
            &pz.raw()[PZ_WHEEL..PZ_WHEEL + 4],
            &pz.raw()[PZ_WHEEL_GOAL..PZ_WHEEL_GOAL + 4],
            fl.get(195),
            fl.get(197),
            fl.get(198)
        ),
        rx,
        312,
        1,
        if fl.get(199) { green } else { white },
        true,
    );

    // 5. Hotel Marker & Briefcase Drums
    let mut word_idx = 0usize;
    for b in 0..5usize {
        if fl.get(406 + b) {
            word_idx |= 1 << b;
        }
    }
    const WORDS: [&str; 19] = [
        "open", "damn", "hell", "town", "dark", "mama", "down", "love", "lock", "mist", "luck",
        "lose", "dose", "over", "dust", "time", "help", "kill", "null",
    ];
    let word_str = WORDS.get(word_idx).copied().unwrap_or("open");
    state.menu_rasterizer.draw_text(
        &format!(
            "5. HOTEL BRIEFCASE  : Word[{word_idx}]=\"{word_str}\" | Thinner(411)={} | Drums={:?}",
            fl.get(411),
            &pz.raw()[PZ_DRUM..PZ_DRUM + 4]
        ),
        rx,
        334,
        1,
        cyan,
        true,
    );

    // 6. Baldwin Tombstone Boards
    state.menu_rasterizer.draw_text(
        &format!(
            "6. BALDWIN BOARDS   : Placed(691..693)=[{}, {}, {}] | Rot={:?} | AcaciaKey(704)={}",
            fl.get(691),
            fl.get(692),
            fl.get(693),
            state.puzzle_cutaway_room.baldwin_rotations,
            fl.get(704)
        ),
        rx,
        356,
        1,
        if fl.get(704) { green } else { white },
        true,
    );

    // 18-Preset Directory List (2 columns at bottom)
    state.menu_rasterizer.draw_text(
        "--- ALL 18 CUTAWAY & PUZZLE PRESETS ([Tab / PgUp / PgDn] to Select) ---",
        24,
        504,
        1,
        yellow,
        true,
    );
    for (idx, &(pic, title, _)) in CUTAWAY_PRESETS.iter().enumerate() {
        let col_x = if idx < 9 { 24 } else { 640 };
        let row_y = 522 + (idx % 9) * 14;
        let is_sel = idx == preset_idx;
        let color = if is_sel { green } else { muted };
        let prefix = if is_sel { ">>" } else { "  " };
        let short_title = title.split(" (").next().unwrap_or(pic);
        state.menu_rasterizer.draw_text(
            &format!("{prefix} {short_title:<44} [{pic}]"),
            col_x,
            row_y,
            1,
            color,
            true,
        );
    }

    // Active Controls & Last Event Banner
    state.menu_rasterizer.draw_text(
        &format!("ACTIVE CONTROLS: {preset_controls}"),
        24,
        656,
        1,
        yellow,
        true,
    );
    state.menu_rasterizer.draw_text(
        &format!(
            "LAST EVENT     : {}",
            state.puzzle_cutaway_room.last_event_desc
        ),
        24,
        674,
        1,
        orange,
        true,
    );
    state.menu_rasterizer.draw_text(
        "[Tab/PgUp/PgDn] Cycle 18 Presets | [S] Auto-Solve Active Puzzle | [R] Cycle Riddle Difficulty | [P] Swap sh2e/data/PS2 | [Esc] Exit",
        24,
        692,
        1,
        white,
        true,
    );

    state.finish_hud_overlay(encoder, surface_view);
}
