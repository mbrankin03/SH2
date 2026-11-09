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
use sh2_assets::{CutawayTextureAsset, DecodedTexture, TextureLoader};
use sh2_audio::{play_menu_sound, SoundCueKind};
use sh2_core::{riddle_level_index, step_clock_needles, Sh2Rng, PROG_KEY_LEFT, PROG_KEY_RIGHT};

mod hud;
mod input;

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
    /// Edge-triggered `PROG_KEY_*` button mask queued since the last simulation frame.
    pub pending_prog_down: u32,
    /// Currently held `PROG_KEY_*` button mask.
    pub held_prog_keys: u32,
    pub loaded_base_key: Option<(usize, bool)>,
    pub base_tex: Option<CutawayTextureAsset>,
    pub sheet_cache: HashMap<(&'static str, bool), CutawayTextureAsset>,
    pub last_event_desc: String,
}

impl PuzzleCutawayRoomState {
    /// Updates the held `PROG_KEY_*` bitmask and queues non-repeat key-down transitions for
    /// deterministic single-tick consumption in [`PuzzleCutawayRoomController::tick_and_render_3d`].
    pub fn queue_prog_key(&mut self, mask: u32, pressed: bool, repeat: bool) {
        if pressed {
            self.held_prog_keys |= mask;
            if !repeat {
                self.pending_prog_down |= mask;
            }
        } else {
            self.held_prog_keys &= !mask;
        }
    }

    /// Atomically drains the queued edge-triggered `down` mask and returns `(down, held)`.
    pub fn take_frame_prog_input(&mut self) -> (u32, u32) {
        let down = std::mem::take(&mut self.pending_prog_down);
        (down, self.held_prog_keys)
    }
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
            pending_prog_down: 0,
            held_prog_keys: 0,
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
        input::handle_puzzle_cutaway_key_input(state, key_code, pressed, repeat);
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
            let (prog_down, prog_held) = state.puzzle_cutaway_room.take_frame_prog_input();

            // Advance StageProgramRunner if active, else tick CutawayOverlayEngine directly
            if state.sim_core.scenario.prog_runner.is_running() {
                let s = &mut state.sim_core.scenario;
                s.prog_runner.tick(
                    prog_down,
                    prog_held,
                    dt_sec,
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
        hud::render_puzzle_cutaway_hud(state, encoder, surface_view);
    }
}

pub(super) fn sync_overlay_to_preset(state: &mut AppState) {
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

fn load_cutaway_from_roots(pic_or_sheet: &str, prefer_sh2ee: bool) -> Option<CutawayTextureAsset> {
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

pub(super) fn play_cursor_sound(state: &mut AppState) {
    if let Some(eng) = state.audio_engine.as_mut() {
        let _ = play_menu_sound(eng, state.audio_bank.as_ref(), SoundCueKind::CursorMove);
    }
}

pub(super) fn play_confirm_sound(state: &mut AppState) {
    if let Some(eng) = state.audio_engine.as_mut() {
        let _ = play_menu_sound(eng, state.audio_bank.as_ref(), SoundCueKind::Confirm);
    }
}
