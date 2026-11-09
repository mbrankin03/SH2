//! Interactive Puzzle Logic, Stage Event (`ev_prog`) & Cutaway Sprite Overlay Debug Room (`SH2-15`).
//!
//! Provides an interactive 2D/3D studio for verifying all `SH2-15` puzzle state machines,
//! `StageProgramRunner` (`ev_prog`) scripts, and `CutawayOverlayEngine` sprite overlays:
//! 1. **18 Cutaway & Puzzle Presets (`[Tab / PgUp / PgDn]`)**:
//!    - Stage 3 Car Map (`out_p_incar` + `out/p_incar_map` 1.0s fade + SE `0x2b17` & `s03_nothing_in_car`),
//!    - Apartment 208 Clock (`apt_clock_close` & `apt_clock_open` + `apt/clock_hari`, `9:10` window),
//!    - Apartment W2F Safe & Toilet Wallet (`apt_p_safe_close` + `apt/p_safe_close2`),
//!    - Apartment 105 Coin Desk (`apt_p_desk_coin` + `apt/p_desk_coin_coin`, `COIN_FLAG(c, h)`),
//!    - Hospital 2F Locked Box (`hsp_p_box` + `hsp/pboxkey01`) & Wall Blood Numbers (`hsp_p_boxnumber`),
//!    - Hospital 3F Corridor Keypad (`7335`/`1328`) & Trick Chest Quiz (`hsp_p_quiz` + `hsp/p_quiz_btn`),
//!    - Hotel 204 Photos Marker (`htl_p_marker`) & Briefcase Trunk Drums (`htl_p_trank`),
//!    - Hotel Egg Doors (`htl_p_door_egg`), VCR (`htl_p_video`), Elevators, Labyrinth Manacles (`dls_p_manacles_on`),
//!    - *Born from a Wish* Baldwin Mansion Tombstone (`add_p_teien` + `add/teien2`),
//!    - Special Format-8 Paletted (`htl_p_hanging`) and PS2 `0xa7a7a7a7` CLUT (`item_x_coin_snake`) `.tex` decoders.
//! 2. **Live Asset Source & Aspect-Ratio Verification (`[P]`)**:
//!    - Hot-swap between `Sh2EeUpscaled` (`sh2e/pic/`), `OriginalPc` (`data/pic/`), and `OriginalPs2`
//!      while inspecting `CutawayViewportTransform` and normalized sprite-sheet UVs.
//! 3. **Difficulty & `FlagInit` Randomization (`[R]`)**:
//!    - Cycle `Easy` -> `Normal` -> `Hard` -> `Extra` riddle difficulties and re-seed `init_new_game`.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;
use winit::keyboard::KeyCode;

use crate::rooms::RoomController;
use crate::state::AppState;
use sh2_assets::{CutawayTextureAsset, CutawayViewportTransform, DecodedTexture, TextureLoader};
use sh2_audio::{play_menu_sound, SoundCueKind};
use sh2_core::{
    evaluate_baldwin_tombstone_boards, format_carbon_box_code, format_wallet_safe_code,
    init_new_game, is_clock_solved, is_coin_desk_solved, place_baldwin_board, place_coin_in_desk,
    riddle_level_index, step_clock_needles, step_safe_dial, try_open_safe,
    use_key_on_hospital_box, verify_hospital_box_keypad, verify_hospital_box_wheels,
    verify_hospital_corridor_keypad, ActionDifficulty, ClearData, MessageInsertState,
    RiddleDifficulty, Sh2Rng, PROG_KEY_A, PROG_KEY_LEFT, PROG_KEY_RIGHT, PZ_CLOCK, PZ_DRUM,
    PZ_SAFE_CODE, PZ_SAFE_PROG, PZ_WHEEL, PZ_WHEEL_GOAL,
};
use sh2_ui::menu::TitlePhase;

/// Curated cutaway and interactive puzzle presets `(picture_id, title, controls_hint)`.
pub(crate) const CUTAWAY_PRESETS: &[(&str, &str, &str)] = &[
    (
        "out_p_incar",
        "1. Stage 3 Car Seat Map (out_p_incar + out/p_incar_map)",
        "[Space] Start ev_prog #1/#2 | [Enter] Press A (Take Map -> 1.0s fade & SE 0x2b17) | [S] Reset Flag 28",
    ),
    (
        "apt_clock_close",
        "2. Apt 208 Clock Closed (apt_clock_close + apt/clock_hari)",
        "[Left/Right] Turn Clock Needles (1x/8x/64x accel) | [S] Snap to 9:10 (0x8980)",
    ),
    (
        "apt_clock_open",
        "3. Apt 208 Clock Open Face (apt_clock_open + apt/clock_hari)",
        "[Left/Right] Turn Clock Needles (1x/8x/64x accel) | [S] Snap to 9:10 (0x8980)",
    ),
    (
        "apt_p_safe_close",
        "4. Apt W2F Safe Dial & Wallet (apt_p_safe_close + apt/p_safe_close2)",
        "[Left/Right] Turn Safe Dial (+/-1) | [S] Auto-Solve Safe & Grant 40 Handgun Bullets",
    ),
    (
        "apt_p_desk_coin",
        "5. Apt 105 Five-Hole Coin Desk (apt_p_desk_coin + apt/p_desk_coin_coin)",
        "[Left/Right] Select Hole 1..5 | [1/2/3] Place Snake/OldMan/Prisoner | [S] Auto-Solve for Riddle Level",
    ),
    (
        "hsp_p_box",
        "6. Hospital 2F Locked Box (hsp_p_box + hsp/pboxkey01)",
        "[Up/Down] Select Wheel 1..4 | [Left/Right] Turn Symbol | [1/2/3] Toggle Key/Wheel/Keypad Locks | [S] Unlock All",
    ),
    (
        "hsp_p_boxnumber",
        "7. Hospital Wall Blood Numbers (hsp_p_boxnumber + hsp/p_boxnumber_2)",
        "[R] Re-roll PZ_WHEEL_GOAL (Paired on Easy Riddle, 4 Distinct on Normal/Hard/Extra)",
    ),
    (
        "hsp_p_quiz",
        "8. Hospital Trick Chest Quiz & 3F Keypad (hsp_p_quiz + hsp/p_quiz_btn)",
        "[1/2/3] Cycle Quiz Row 1..3 Button | [S] Verify 3F Keypad (7335 Easy/Normal, 1328 Hard/Extra)",
    ),
    (
        "htl_p_marker",
        "9. Hotel 204 Photos & Briefcase Word (htl_p_marker + htl/maker)",
        "[Left/Right] Cycle Secret Word (0..18) | [S] Use Thinner (Toggle Flag 411 Marker Wipe)",
    ),
    (
        "htl_p_trank",
        "10. Hotel 204 Briefcase 4-Drum Lock (htl_p_trank + htl/dram)",
        "[Up/Down] Select Drum 1..4 | [Left/Right] Rotate Drum Letter | [S] Toggle Flag 405",
    ),
    (
        "htl_p_egghollow",
        "11. Hotel Egg Doors (htl_p_egghollow + htl/egg)",
        "[1] Show Rust-Colored Egg | [2] Show Scarlet Egg | [S] Clear Egg Hollow",
    ),
    (
        "htl_p_video",
        "12. Hotel Office Safe VCR & Can Opener (htl_p_video + htl/p_video02)",
        "[1] Toggle Videotape (#23) | [2] Toggle Can Opener (#70) | [S] Toggle Safe Empty (Flag 391)",
    ),
    (
        "add_p_teien",
        "13. Born from a Wish: Baldwin Tombstone (add_p_teien + add/teien2)",
        "[1/2/3] Place/Rotate White/Black/Red Board | [S] Solve Boards & Take Acacia Key (Flag 704)",
    ),
    (
        "hsp_p_h_elevator",
        "14. Hospital Elevator Panel (hsp_p_h_elevator + hsp/p_h_elevator_botan)",
        "[1/2/3/4] Toggle Lit Floor Buttons (1F, 2F, 3F, B1)",
    ),
    (
        "hsp_p_h_elevator_2",
        "15. Otherworld Hospital Elevator (hsp_p_h_elevator_2 + hsp/p_h_elevator_botan)",
        "[1/2/3/4] Toggle Lit Floor Buttons (1F, 2F, 3F, B1)",
    ),
    (
        "dls_p_manacles_on",
        "16. Labyrinth Catacombs Manacles (dls_p_manacles_on + dls/p_manacles_off)",
        "[S] Toggle Flag 304 (Fades dls/p_manacles_off 0.0 -> 1.0 over 1.0s)",
    ),
    (
        "htl_p_hanging",
        "17. Hotel Hanging Man Still (PC Format-8 8-bit Paletted .tex Decoder)",
        "[P] Swap SH2:EE vs Original PC Paletted Format 8 Texture",
    ),
    (
        "item_x_coin_snake",
        "18. Snake Coin Inspect (PS2 0xa7a7a7a7 GS-Swizzled 8-bit CLUT .tex Decoder)",
        "[P] Inspect Decoded PS2 Swizzled CSM1 CLUT Texture",
    ),
];

/// Runtime state for [`sh2_ui::menu::TestRoomKind::PuzzleAndCutawayRoom`] (`SH2-15`).
pub(crate) struct PuzzleCutawayRoomState {
    pub preset_idx: usize,
    pub prefer_sh2ee: bool,
    pub rng: Sh2Rng,
    pub seed_counter: u32,
    pub clock_accel_timer: f32,
    pub selected_coin_hole: usize,
    pub selected_box_wheel: usize,
    pub selected_drum_idx: usize,
    pub baldwin_rotations: [u8; 3],
    pub elevator_floor_mask: i32,
    pub egg_putting: i32,
    pub loaded_base_key: Option<(usize, bool)>,
    pub base_tex: Option<CutawayTextureAsset>,
    pub sheet_cache: HashMap<(&'static str, bool), CutawayTextureAsset>,
    pub last_event_desc: String,
}

impl Default for PuzzleCutawayRoomState {
    fn default() -> Self {
        Self {
            preset_idx: 0,
            prefer_sh2ee: true,
            rng: Sh2Rng::new(0x2001_0924),
            seed_counter: 1,
            clock_accel_timer: 0.0,
            selected_coin_hole: 0,
            selected_box_wheel: 0,
            selected_drum_idx: 0,
            baldwin_rotations: [0, 0, 0],
            elevator_floor_mask: 0b0101,
            egg_putting: 1,
            loaded_base_key: None,
            base_tex: None,
            sheet_cache: HashMap::new(),
            last_event_desc:
                "Press [Tab/PgUp/PgDn] to switch preset, [Space] to run Car Map ev_prog, [P] to swap sh2e/data"
                    .to_string(),
        }
    }
}

pub(crate) struct PuzzleCutawayRoomController;

impl RoomController for PuzzleCutawayRoomController {
    fn handle_key_input(state: &mut AppState, key_code: KeyCode, pressed: bool, repeat: bool) {
        match key_code {
            KeyCode::ArrowLeft | KeyCode::KeyA => {
                state.key_left = pressed;
            }
            KeyCode::ArrowRight | KeyCode::KeyD => {
                state.key_right = pressed;
            }
            _ => {}
        }

        if !pressed {
            return;
        }

        match key_code {
            KeyCode::Escape | KeyCode::Backspace if !repeat => {
                state.exit_room_to_menu(TitlePhase::TestRoomsSelect);
                tracing::info!("Returned from Puzzle & Cutaway Overlays Room to Test Rooms menu.");
                return;
            }
            KeyCode::Tab | KeyCode::PageDown if !repeat => {
                state.puzzle_cutaway_room.preset_idx =
                    (state.puzzle_cutaway_room.preset_idx + 1) % CUTAWAY_PRESETS.len();
                sync_overlay_to_preset(state);
                play_cursor_sound(state);
                return;
            }
            KeyCode::PageUp if !repeat => {
                if state.puzzle_cutaway_room.preset_idx == 0 {
                    state.puzzle_cutaway_room.preset_idx = CUTAWAY_PRESETS.len() - 1;
                } else {
                    state.puzzle_cutaway_room.preset_idx -= 1;
                }
                sync_overlay_to_preset(state);
                play_cursor_sound(state);
                return;
            }
            // [P]: Toggle SH2:EE HD (sh2e/pic) vs Original PS2/PC (data/pic)
            KeyCode::KeyP if !repeat => {
                state.puzzle_cutaway_room.prefer_sh2ee = !state.puzzle_cutaway_room.prefer_sh2ee;
                state.puzzle_cutaway_room.loaded_base_key = None;
                let label = if state.puzzle_cutaway_room.prefer_sh2ee {
                    "SH2:EE HD (sh2e/pic)"
                } else {
                    "Original PS2/PC (data/pic)"
                };
                state.puzzle_cutaway_room.last_event_desc =
                    format!("Switched Cutaway Asset Source -> {label}");
                play_confirm_sound(state);
                return;
            }
            // [R]: Cycle RiddleDifficulty (Easy -> Normal -> Hard -> Extra) & re-run FlagInit
            KeyCode::KeyR if !repeat => {
                let next_riddle = match state.sim_core.scenario.riddle_difficulty {
                    RiddleDifficulty::Easy => RiddleDifficulty::Normal,
                    RiddleDifficulty::Normal => RiddleDifficulty::Hard,
                    RiddleDifficulty::Hard => RiddleDifficulty::Extra,
                    RiddleDifficulty::Extra => RiddleDifficulty::Easy,
                };
                state.sim_core.scenario.riddle_difficulty = next_riddle;
                state.puzzle_cutaway_room.seed_counter =
                    state.puzzle_cutaway_room.seed_counter.wrapping_add(1);
                state.puzzle_cutaway_room.rng = Sh2Rng::new(
                    0x2001_0924u32.wrapping_add(state.puzzle_cutaway_room.seed_counter * 7919),
                );
                let mut clear = ClearData::default();
                if next_riddle == RiddleDifficulty::Extra {
                    clear.cleared_levels = 0x07;
                    clear.endings_seen = 0x07;
                }
                init_new_game(
                    &mut state.sim_core.scenario.flags,
                    &mut state.sim_core.scenario.puzzle_state,
                    &mut state.puzzle_cutaway_room.rng,
                    next_riddle,
                    ActionDifficulty::Normal,
                    clear,
                );
                sync_overlay_to_preset(state);
                state.puzzle_cutaway_room.last_event_desc = format!(
                    "Re-initialized FlagInit with RiddleDifficulty::{:?} (Seed #{})",
                    next_riddle, state.puzzle_cutaway_room.seed_counter
                );
                play_confirm_sound(state);
                return;
            }
            // [Space]: Start Stage 3 Car Map Program (#1 if flag 28 is clear, #2 if flag 28 is set)
            KeyCode::Space if !repeat => {
                let has_map = state.sim_core.scenario.flags.get(28);
                if !has_map {
                    state.sim_core.scenario.prog_runner.start(3, 1, 28);
                    state.puzzle_cutaway_room.preset_idx = 0;
                    state.puzzle_cutaway_room.last_event_desc =
                        "Started Stage 3 ev_prog #1 (s03_item_in_car). Press [Enter] to take Silent Hill map!"
                            .to_string();
                } else {
                    state.sim_core.scenario.prog_runner.start(3, 2, 0);
                    state.puzzle_cutaway_room.last_event_desc =
                        "Started Stage 3 ev_prog #2 (s03_nothing_in_car: map already taken)."
                            .to_string();
                }
                play_confirm_sound(state);
                return;
            }
            // [Enter]: Send PROG_KEY_A to StageProgramRunner if running
            KeyCode::Enter
                if !repeat && state.sim_core.scenario.prog_runner.is_running() =>
            {
                let s = &mut state.sim_core.scenario;
                s.prog_runner.tick(
                    PROG_KEY_A,
                    0,
                    0.016,
                    &mut s.flags,
                    &mut s.puzzle_state,
                    &mut s.inventory_mgr,
                    &mut s.cutaway_overlay,
                    s.riddle_difficulty,
                );
                if let Some(&se_id) = s.prog_runner.sound_effects().first() {
                    state.puzzle_cutaway_room.last_event_desc =
                        format!("ev_prog triggered SE 0x{se_id:04X} ({se_id})");
                }
                play_confirm_sound(state);
                return;
            }
            _ => {}
        }

        // Preset-specific interactive controls
        let preset_pic = CUTAWAY_PRESETS[state.puzzle_cutaway_room.preset_idx].0;
        match preset_pic {
            "out_p_incar" if key_code == KeyCode::KeyS && !repeat => {
                state.sim_core.scenario.flags.set(28, false);
                state
                    .sim_core
                    .scenario
                    .cutaway_overlay
                    .set_picture(Some("out_p_incar"), &state.sim_core.scenario.flags);
                state.puzzle_cutaway_room.last_event_desc =
                    "Reset Flag 28 (Map returned to car seat; incar_alpha = 1.0)".to_string();
            }
            "apt_clock_close" | "apt_clock_open" if key_code == KeyCode::KeyS && !repeat => {
                state
                    .sim_core
                    .scenario
                    .puzzle_state
                    .write_u16(PZ_CLOCK, 0x8980);
                state.sim_core.scenario.flags.set(117, true);
                state.puzzle_cutaway_room.last_event_desc =
                    "Snapped Clock hands to 9:10 (0x8980) -> SOLVED (Flag 117 set)!".to_string();
                play_confirm_sound(state);
            }
            "apt_p_safe_close" => {
                if (key_code == KeyCode::ArrowLeft || key_code == KeyCode::KeyA) && !repeat {
                    let pos = step_safe_dial(
                        &mut state.sim_core.scenario.flags,
                        &mut state.sim_core.scenario.puzzle_state,
                        false,
                    );
                    state.puzzle_cutaway_room.last_event_desc = format!(
                        "Turned Safe Dial Left (<<) -> points at {} (Solved={})",
                        pos + 1,
                        state.sim_core.scenario.flags.get(108)
                    );
                    play_cursor_sound(state);
                } else if (key_code == KeyCode::ArrowRight || key_code == KeyCode::KeyD) && !repeat
                {
                    let pos = step_safe_dial(
                        &mut state.sim_core.scenario.flags,
                        &mut state.sim_core.scenario.puzzle_state,
                        true,
                    );
                    state.puzzle_cutaway_room.last_event_desc = format!(
                        "Turned Safe Dial Right (>>) -> points at {} (Solved={})",
                        pos + 1,
                        state.sim_core.scenario.flags.get(108)
                    );
                    play_cursor_sound(state);
                } else if key_code == KeyCode::KeyS && !repeat {
                    let code: [u8; 4] = state.sim_core.scenario.puzzle_state.raw()
                        [PZ_SAFE_CODE..PZ_SAFE_CODE + 4]
                        .try_into()
                        .unwrap_or([0; 4]);
                    state.sim_core.scenario.puzzle_state.raw_mut()
                        [PZ_SAFE_PROG..PZ_SAFE_PROG + 4]
                        .copy_from_slice(&code);
                    for b in 0..5 {
                        state
                            .sim_core
                            .scenario
                            .flags
                            .set(103 + b, ((code[3] >> b) & 1) != 0);
                    }
                    let opened = try_open_safe(
                        &mut state.sim_core.scenario.flags,
                        &state.sim_core.scenario.puzzle_state,
                        &mut state.sim_core.scenario.inventory_mgr,
                    );
                    state.sim_core.scenario.sync_legacy_views();
                    state.puzzle_cutaway_room.last_event_desc = format!(
                        "Auto-solved Safe Combination -> opened={opened}, Flag 108={}, Handgun Bullets={}",
                        state.sim_core.scenario.flags.get(108),
                        state.sim_core.scenario.inventory_mgr.count(5)
                    );
                    play_confirm_sound(state);
                }
            }
            "apt_p_desk_coin" => {
                if (key_code == KeyCode::ArrowLeft || key_code == KeyCode::KeyA) && !repeat {
                    state.puzzle_cutaway_room.selected_coin_hole =
                        (state.puzzle_cutaway_room.selected_coin_hole + 4) % 5;
                } else if (key_code == KeyCode::ArrowRight || key_code == KeyCode::KeyD) && !repeat
                {
                    state.puzzle_cutaway_room.selected_coin_hole =
                        (state.puzzle_cutaway_room.selected_coin_hole + 1) % 5;
                } else if matches!(
                    key_code,
                    KeyCode::Digit1 | KeyCode::Digit2 | KeyCode::Digit3
                ) && !repeat
                {
                    let coin_id = match key_code {
                        KeyCode::Digit1 => 50u8, // Snake
                        KeyCode::Digit2 => 51u8, // Old Man
                        _ => 52u8,               // Prisoner
                    };
                    let hole = state.puzzle_cutaway_room.selected_coin_hole;
                    if !state.sim_core.scenario.inventory_mgr.has_item(coin_id) {
                        state.sim_core.scenario.inventory_mgr.item_get_now(coin_id);
                    }
                    let s = &mut state.sim_core.scenario;
                    let res = place_coin_in_desk(
                        &mut s.flags,
                        &mut s.inventory_mgr,
                        s.riddle_difficulty,
                        hole,
                        coin_id,
                    );
                    s.sync_legacy_views();
                    state.puzzle_cutaway_room.last_event_desc = format!(
                        "Placed Coin #{coin_id} into Hole #{} -> result={res:?}, Solved={}",
                        hole + 1,
                        s.flags.get(142)
                    );
                    play_confirm_sound(state);
                } else if key_code == KeyCode::KeyS && !repeat {
                    for f in 126..=142 {
                        state.sim_core.scenario.flags.set(f, false);
                    }
                    for c in 50..=52 {
                        if !state.sim_core.scenario.inventory_mgr.has_item(c) {
                            state.sim_core.scenario.inventory_mgr.item_get_now(c);
                        }
                    }
                    // Exact hole placements per RiddleDifficulty:
                    // Easy:   (50, 2), (51, 0), (52, 4) -> [128, 131, 140]
                    // Normal: (50, 4), (51, 1), (52, 2) -> [130, 132, 138]
                    // Hard:   (50, 3), (51, 1), (52, 4) -> [129, 132, 140]
                    // Extra:  (50, 2), (51, 0), (52, 3) -> [128, 131, 139]
                    let placements = match state.sim_core.scenario.riddle_difficulty {
                        RiddleDifficulty::Easy => [(50u8, 2usize), (51, 0), (52, 4)],
                        RiddleDifficulty::Normal => [(50, 4), (51, 1), (52, 2)],
                        RiddleDifficulty::Hard => [(50, 3), (51, 1), (52, 4)],
                        RiddleDifficulty::Extra => [(50, 2), (51, 0), (52, 3)],
                    };
                    let s = &mut state.sim_core.scenario;
                    for (coin_id, hole) in placements {
                        place_coin_in_desk(
                            &mut s.flags,
                            &mut s.inventory_mgr,
                            s.riddle_difficulty,
                            hole,
                            coin_id,
                        );
                    }
                    s.sync_legacy_views();
                    state.puzzle_cutaway_room.last_event_desc = format!(
                        "Auto-placed {:?} 3-Coin Solution -> Flag 142={}, Lyne Key (#31)={}",
                        s.riddle_difficulty,
                        s.flags.get(142),
                        s.inventory_mgr.has_item(31)
                    );
                    play_confirm_sound(state);
                }
            }
            "hsp_p_box" => {
                if (key_code == KeyCode::ArrowUp || key_code == KeyCode::KeyW) && !repeat {
                    state.puzzle_cutaway_room.selected_box_wheel =
                        (state.puzzle_cutaway_room.selected_box_wheel + 3) % 4;
                } else if (key_code == KeyCode::ArrowDown || key_code == KeyCode::KeyJ) && !repeat {
                    state.puzzle_cutaway_room.selected_box_wheel =
                        (state.puzzle_cutaway_room.selected_box_wheel + 1) % 4;
                } else if (key_code == KeyCode::ArrowLeft || key_code == KeyCode::KeyA) && !repeat {
                    let w = state.puzzle_cutaway_room.selected_box_wheel;
                    let cur = state.sim_core.scenario.puzzle_state.raw()[PZ_WHEEL + w];
                    state.sim_core.scenario.puzzle_state.raw_mut()[PZ_WHEEL + w] = (cur + 8) % 9;
                } else if (key_code == KeyCode::ArrowRight || key_code == KeyCode::KeyD) && !repeat
                {
                    let w = state.puzzle_cutaway_room.selected_box_wheel;
                    let cur = state.sim_core.scenario.puzzle_state.raw()[PZ_WHEEL + w];
                    state.sim_core.scenario.puzzle_state.raw_mut()[PZ_WHEEL + w] = (cur + 1) % 9;
                } else if key_code == KeyCode::Digit1 && !repeat {
                    let v = !state.sim_core.scenario.flags.get(195);
                    state.sim_core.scenario.flags.set(195, v);
                    state.puzzle_cutaway_room.last_event_desc =
                        format!("Toggled Hospital Box Key Locks (Flag 195) -> {v}");
                } else if key_code == KeyCode::Digit2 && !repeat {
                    let v = !state.sim_core.scenario.flags.get(197);
                    state.sim_core.scenario.flags.set(197, v);
                    state.puzzle_cutaway_room.last_event_desc =
                        format!("Toggled Hospital Box Wheel Lock (Flag 197) -> {v}");
                } else if key_code == KeyCode::Digit3 && !repeat {
                    let v = !state.sim_core.scenario.flags.get(198);
                    state.sim_core.scenario.flags.set(198, v);
                    state.puzzle_cutaway_room.last_event_desc =
                        format!("Toggled Hospital Box Keypad Lock (Flag 198) -> {v}");
                } else if key_code == KeyCode::KeyS && !repeat {
                    let s = &mut state.sim_core.scenario;
                    s.flags.set(203, true);
                    let goals: [u8; 4] = s.puzzle_state.raw()[PZ_WHEEL_GOAL..PZ_WHEEL_GOAL + 4]
                        .try_into()
                        .unwrap_or([0; 4]);
                    s.puzzle_state.raw_mut()[PZ_WHEEL..PZ_WHEEL + 4].copy_from_slice(&goals);
                    verify_hospital_box_wheels(
                        &mut s.flags,
                        &mut s.puzzle_state,
                        &mut state.puzzle_cutaway_room.rng,
                    );
                    let mut ins = MessageInsertState::default();
                    let code_str = format_carbon_box_code(&s.puzzle_state, &mut ins);
                    let digits: [u8; 4] = [
                        code_str.as_bytes()[0] - b'0',
                        code_str.as_bytes()[1] - b'0',
                        code_str.as_bytes()[2] - b'0',
                        code_str.as_bytes()[3] - b'0',
                    ];
                    verify_hospital_box_keypad(&mut s.flags, &s.puzzle_state, digits);
                    s.inventory_mgr.item_get_now(35);
                    s.inventory_mgr.item_get_now(36);
                    use_key_on_hospital_box(&mut s.flags, &mut s.inventory_mgr, 86);
                    s.sync_legacy_views();
                    state.puzzle_cutaway_room.last_event_desc = format!(
                        "Unlocked all 4 Hospital Box locks -> Flag 199={}, Piece of Hair (#53)={}",
                        s.flags.get(199),
                        s.inventory_mgr.has_item(53)
                    );
                    play_confirm_sound(state);
                }
            }
            "hsp_p_quiz" => {
                if matches!(
                    key_code,
                    KeyCode::Digit1 | KeyCode::Digit2 | KeyCode::Digit3
                ) && !repeat
                {
                    let r = match key_code {
                        KeyCode::Digit1 => 0usize,
                        KeyCode::Digit2 => 1,
                        _ => 2,
                    };
                    let a = state.sim_core.scenario.flags.get(234 + 2 * r);
                    let b = state.sim_core.scenario.flags.get(235 + 2 * r);
                    let cur = (if a { 1 } else { 0 }) | (if b { 2 } else { 0 });
                    let next = (cur + 1) % 4;
                    state
                        .sim_core
                        .scenario
                        .flags
                        .set(234 + 2 * r, (next & 1) != 0);
                    state
                        .sim_core
                        .scenario
                        .flags
                        .set(235 + 2 * r, (next & 2) != 0);
                } else if key_code == KeyCode::KeyS && !repeat {
                    let s = &mut state.sim_core.scenario;
                    let code = match s.riddle_difficulty {
                        RiddleDifficulty::Easy | RiddleDifficulty::Normal => 7335,
                        RiddleDifficulty::Hard | RiddleDifficulty::Extra => 1328,
                    };
                    let ok =
                        verify_hospital_corridor_keypad(&mut s.flags, s.riddle_difficulty, code);
                    // Also set Quiz winning pattern (Row 1 = 3 [1,1], Row 2 = 1 [1,0], Row 3 = 3 [1,1])
                    for (f, val) in [
                        (234, true),
                        (235, true),
                        (236, true),
                        (237, false),
                        (238, true),
                        (239, true),
                    ] {
                        s.flags.set(f, val);
                    }
                    state.puzzle_cutaway_room.last_event_desc = format!(
                        "Verified 3F Keypad ({code} -> {ok}, Flag 181={}) & set Trick Chest Quiz answers",
                        s.flags.get(181)
                    );
                    play_confirm_sound(state);
                }
            }
            "htl_p_marker" => {
                let mut w = 0usize;
                for b in 0..5usize {
                    if state.sim_core.scenario.flags.get(406 + b) {
                        w |= 1 << b;
                    }
                }
                if (key_code == KeyCode::ArrowLeft || key_code == KeyCode::KeyA) && !repeat {
                    w = (w + 18) % 19;
                    for b in 0..5usize {
                        state
                            .sim_core
                            .scenario
                            .flags
                            .set(406 + b, ((w >> b) & 1) != 0);
                    }
                } else if (key_code == KeyCode::ArrowRight || key_code == KeyCode::KeyD) && !repeat
                {
                    w = (w + 1) % 19;
                    for b in 0..5usize {
                        state
                            .sim_core
                            .scenario
                            .flags
                            .set(406 + b, ((w >> b) & 1) != 0);
                    }
                } else if key_code == KeyCode::KeyS && !repeat {
                    let gone = !state.sim_core.scenario.flags.get(404);
                    state.sim_core.scenario.flags.set(404, gone);
                    state.sim_core.scenario.flags.set(411, gone);
                    state.puzzle_cutaway_room.last_event_desc = format!(
                        "Toggled Thinner Marker Wipe (Flags 404/411) -> marker_hidden={gone}"
                    );
                }
            }
            "htl_p_trank" => {
                state.sim_core.scenario.flags.set(405, true);
                if (key_code == KeyCode::ArrowUp || key_code == KeyCode::KeyW) && !repeat {
                    state.puzzle_cutaway_room.selected_drum_idx =
                        (state.puzzle_cutaway_room.selected_drum_idx + 3) % 4;
                } else if (key_code == KeyCode::ArrowDown || key_code == KeyCode::KeyJ) && !repeat {
                    state.puzzle_cutaway_room.selected_drum_idx =
                        (state.puzzle_cutaway_room.selected_drum_idx + 1) % 4;
                } else if (key_code == KeyCode::ArrowLeft || key_code == KeyCode::KeyA) && !repeat {
                    let d = state.puzzle_cutaway_room.selected_drum_idx;
                    let cur = state.sim_core.scenario.puzzle_state.raw()[PZ_DRUM + d];
                    state.sim_core.scenario.puzzle_state.raw_mut()[PZ_DRUM + d] = (cur + 7) & 7;
                } else if (key_code == KeyCode::ArrowRight || key_code == KeyCode::KeyD) && !repeat
                {
                    let d = state.puzzle_cutaway_room.selected_drum_idx;
                    let cur = state.sim_core.scenario.puzzle_state.raw()[PZ_DRUM + d];
                    state.sim_core.scenario.puzzle_state.raw_mut()[PZ_DRUM + d] = (cur + 1) & 7;
                } else if key_code == KeyCode::KeyS && !repeat {
                    let v = !state.sim_core.scenario.flags.get(405);
                    state.sim_core.scenario.flags.set(405, v);
                }
            }
            "htl_p_egghollow" => {
                if key_code == KeyCode::Digit1 && !repeat {
                    let v = !state.sim_core.scenario.flags.get(380);
                    state.sim_core.scenario.flags.set(380, v);
                } else if key_code == KeyCode::Digit2 && !repeat {
                    let v = !state.sim_core.scenario.flags.get(381);
                    state.sim_core.scenario.flags.set(381, v);
                } else if key_code == KeyCode::KeyS && !repeat {
                    state.sim_core.scenario.flags.set(380, true);
                    state.sim_core.scenario.flags.set(381, true);
                }
            }
            "htl_p_video" => {
                if key_code == KeyCode::Digit1 && !repeat {
                    let v = !state.sim_core.scenario.flags.get(388);
                    state.sim_core.scenario.flags.set(388, v);
                } else if key_code == KeyCode::Digit2 && !repeat {
                    let v = !state.sim_core.scenario.flags.get(389);
                    state.sim_core.scenario.flags.set(389, v);
                } else if key_code == KeyCode::KeyS && !repeat {
                    state.sim_core.scenario.flags.set(388, true);
                    state.sim_core.scenario.flags.set(389, !state.sim_core.scenario.flags.get(389));
                }
            }
            "add_p_teien" => {
                if matches!(
                    key_code,
                    KeyCode::Digit1 | KeyCode::Digit2 | KeyCode::Digit3
                ) && !repeat
                {
                    let b = match key_code {
                        KeyCode::Digit1 => 0usize,
                        KeyCode::Digit2 => 1,
                        _ => 2,
                    };
                    let item_id = 79 + (b as u8);
                    let rot = (state.puzzle_cutaway_room.baldwin_rotations[b] + 1) & 3;
                    state.puzzle_cutaway_room.baldwin_rotations[b] = rot;
                    if !state.sim_core.scenario.inventory_mgr.has_item(item_id) {
                        state.sim_core.scenario.inventory_mgr.item_get_now(item_id);
                    }
                    place_baldwin_board(
                        &mut state.sim_core.scenario.flags,
                        &mut state.sim_core.scenario.inventory_mgr,
                        item_id,
                        rot,
                    );
                    state.puzzle_cutaway_room.last_event_desc = format!(
                        "Placed Board #{item_id} at rotation {} ({}°)",
                        rot,
                        (rot as u32) * 90
                    );
                } else if key_code == KeyCode::KeyS && !repeat {
                    let s = &mut state.sim_core.scenario;
                    for id in 79..=81 {
                        if !s.inventory_mgr.has_item(id) {
                            s.inventory_mgr.item_get_now(id);
                        }
                    }
                    place_baldwin_board(&mut s.flags, &mut s.inventory_mgr, 79, 2);
                    place_baldwin_board(&mut s.flags, &mut s.inventory_mgr, 80, 0);
                    place_baldwin_board(&mut s.flags, &mut s.inventory_mgr, 81, 1);
                    let solved =
                        evaluate_baldwin_tombstone_boards(&mut s.flags, &mut s.inventory_mgr);
                    s.sync_legacy_views();
                    state.puzzle_cutaway_room.last_event_desc = format!(
                        "Solved Baldwin Tombstone Boards -> solved={solved}, Flag 704={}, Acacia Key (#82)={}",
                        s.flags.get(704),
                        s.inventory_mgr.has_item(82)
                    );
                    play_confirm_sound(state);
                }
            }
            "hsp_p_h_elevator" | "hsp_p_h_elevator_2"
                if matches!(
                    key_code,
                    KeyCode::Digit1 | KeyCode::Digit2 | KeyCode::Digit3 | KeyCode::Digit4
                ) && !repeat =>
            {
                let bit = match key_code {
                    KeyCode::Digit1 => 0,
                    KeyCode::Digit2 => 1,
                    KeyCode::Digit3 => 2,
                    _ => 3,
                };
                state.puzzle_cutaway_room.elevator_floor_mask ^= 1 << bit;
            }
            "dls_p_manacles_on" if key_code == KeyCode::KeyS && !repeat => {
                let next = !state.sim_core.scenario.flags.get(304);
                state.sim_core.scenario.flags.set(304, next);
                if !next {
                    state
                        .sim_core
                        .scenario
                        .cutaway_overlay
                        .set_picture(Some("dls_p_manacles_on"), &state.sim_core.scenario.flags);
                }
                state.puzzle_cutaway_room.last_event_desc =
                    format!("Toggled Labyrinth Manacles Flag 304 -> {next}");
            }
            _ => {}
        }
    }

    fn tick_and_render_3d(
        state: &mut AppState,
        _dt: Duration,
        dt_sec: f32,
        _aspect: f32,
        _grain_pct: u32,
        is_paused: bool,
        _is_in_game: bool,
    ) {
        let preset_idx = state.puzzle_cutaway_room.preset_idx % CUTAWAY_PRESETS.len();
        let preset_pic = CUTAWAY_PRESETS[preset_idx].0;

        if state.sim_core.scenario.cutaway_overlay.active_picture() != Some(preset_pic)
            && !state.sim_core.scenario.prog_runner.is_running()
        {
            sync_overlay_to_preset(state);
        }

        if !is_paused {
            // Advance StageProgramRunner if active, else tick CutawayOverlayEngine directly
            if state.sim_core.scenario.prog_runner.is_running() {
                let s = &mut state.sim_core.scenario;
                s.prog_runner.tick(
                    0,
                    0,
                    dt_sec,
                    &mut s.flags,
                    &mut s.puzzle_state,
                    &mut s.inventory_mgr,
                    &mut s.cutaway_overlay,
                    s.riddle_difficulty,
                );
            } else {
                // Continuous clock needle turning when holding Left/Right on clock presets
                if matches!(preset_pic, "apt_clock_close" | "apt_clock_open") {
                    let mut held = 0u32;
                    if state.key_left {
                        held |= PROG_KEY_LEFT;
                    }
                    if state.key_right {
                        held |= PROG_KEY_RIGHT;
                    }
                    step_clock_needles(
                        &mut state.sim_core.scenario.puzzle_state,
                        held,
                        dt_sec,
                        &mut state.puzzle_cutaway_room.clock_accel_timer,
                    );
                }

                let extra = match preset_pic {
                    "hsp_p_h_elevator" | "hsp_p_h_elevator_2" => {
                        state.puzzle_cutaway_room.elevator_floor_mask
                    }
                    "htl_p_egghollow" => state.puzzle_cutaway_room.egg_putting,
                    _ => 0,
                };
                let s = &mut state.sim_core.scenario;
                let riddle_idx = riddle_level_index(s.riddle_difficulty);
                s.cutaway_overlay.tick(dt_sec, &s.flags);
                s.cutaway_overlay
                    .evaluate_frame(&s.flags, &s.puzzle_state, riddle_idx, extra);
            }
        }

        // Ensure base cutaway `.tex` and any required sprite sheets are loaded in cache
        let pref = state.puzzle_cutaway_room.prefer_sh2ee;
        if state.puzzle_cutaway_room.loaded_base_key != Some((preset_idx, pref)) {
            state.puzzle_cutaway_room.loaded_base_key = Some((preset_idx, pref));
            state.puzzle_cutaway_room.base_tex = load_cutaway_from_roots(preset_pic, pref);
        }

        let needed_sheets: Vec<&'static str> = state
            .sim_core
            .scenario
            .cutaway_overlay
            .frame_sprites()
            .iter()
            .map(|q| q.sheet_name)
            .collect();
        for sheet in needed_sheets {
            state
                .puzzle_cutaway_room
                .sheet_cache
                .entry((sheet, pref))
                .or_insert_with(|| {
                    load_cutaway_from_roots(sheet, pref).unwrap_or(CutawayTextureAsset {
                        texture: DecodedTexture {
                            width: 1,
                            height: 1,
                            rgba: vec![255, 255, 255, 255],
                        },
                        original_width: 1,
                        original_height: 1,
                        is_sh2ee_hd: pref,
                    })
                });
        }

        // Render clean dark studio backdrop
        state.render_ctx.render_scene(
            Some(&state.gpu_map),
            Some(&state.map_texture_bind_groups),
            &[],
        );
    }

    fn render_hud(
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

        state.menu_rasterizer.fill_rect(10, 10, 1260, 700, [8, 10, 15, 235]);
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
            vp_transform =
                base_tex.compute_viewport_transform(vp_box_w as f32, vp_box_h as f32);
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
        let mut holes_desc = [
            "(empty)", "(empty)", "(empty)", "(empty)", "(empty)",
        ];
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
            "open", "damn", "hell", "town", "dark", "mama", "down", "love", "lock", "mist",
            "luck", "lose", "dose", "over", "dust", "time", "help", "kill", "null",
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
            &format!("LAST EVENT     : {}", state.puzzle_cutaway_room.last_event_desc),
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
}

fn sync_overlay_to_preset(state: &mut AppState) {
    let idx = state.puzzle_cutaway_room.preset_idx % CUTAWAY_PRESETS.len();
    let pic = CUTAWAY_PRESETS[idx].0;
    state.puzzle_cutaway_room.loaded_base_key = None;

    // Ensure hotel briefcase & video overlays have visible initial flags when selected
    if pic == "htl_p_trank" {
        state.sim_core.scenario.flags.set(405, true);
    } else if pic == "htl_p_egghollow"
        && !state.sim_core.scenario.flags.get(380)
        && !state.sim_core.scenario.flags.get(381)
    {
        state.sim_core.scenario.flags.set(380, true);
    } else if pic == "htl_p_video" {
        state.sim_core.scenario.flags.set(388, true);
    }

    let s = &mut state.sim_core.scenario;
    s.cutaway_overlay.set_picture(Some(pic), &s.flags);
    let extra = match pic {
        "hsp_p_h_elevator" | "hsp_p_h_elevator_2" => state.puzzle_cutaway_room.elevator_floor_mask,
        "htl_p_egghollow" => state.puzzle_cutaway_room.egg_putting,
        _ => 0,
    };
    let riddle_idx = riddle_level_index(s.riddle_difficulty);
    s.cutaway_overlay
        .evaluate_frame(&s.flags, &s.puzzle_state, riddle_idx, extra);
}

fn load_cutaway_from_roots(
    pic_or_sheet: &str,
    prefer_sh2ee: bool,
) -> Option<CutawayTextureAsset> {
    let resolver = sh2_assets::AssetPathResolver::resolve();
    let base_roots = [
        resolver.game_root.clone(),
        PathBuf::from("Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2"),
    ];
    for base in &base_roots {
        let loader = TextureLoader::new(base.join("data"), Some(base.join("sh2e")));
        if let Ok(tex) = loader.load_cutaway_texture(pic_or_sheet, prefer_sh2ee) {
            return Some(tex);
        }
    }
    None
}

fn play_cursor_sound(state: &mut AppState) {
    if let Some(eng) = state.audio_engine.as_mut() {
        let _ = play_menu_sound(eng, state.audio_bank.as_ref(), SoundCueKind::CursorMove);
    }
}

fn play_confirm_sound(state: &mut AppState) {
    if let Some(eng) = state.audio_engine.as_mut() {
        let _ = play_menu_sound(eng, state.audio_bank.as_ref(), SoundCueKind::Confirm);
    }
}
