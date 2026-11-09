//! Interactive Stage Event Engine (`src/Event/event.c`) & Environmental Horror Triggers Chamber (`SH2-47`).
//!
//! Provides a real-time 3D and 2D studio for testing and verifying all `SH2-47` requirements:
//! 1. **7 Authentic Stage Environmental Horror Scenarios (`[1]`–`[7]` or `[Tab]`)**:
//!    - **1. Toluca Prison North (`stg_prison_n.c` Room `0x73`)**: Women's bathroom stall knock (`flag 648`)
//!      and randomized 60% exit jumpscare (`0x427D` -> `sddata[316]`, `flag 649`) vs 40% disarm across `Z > s_thr`.
//!    - **2. Blue Creek Apartments 2F (`stg_apart_w2f.c` Room `0x28` / Room 209)**: Whispering voice
//!      (`0x9C70` -> `sddata[387]`, `flag 547`) triggered across `X < s_thr`, orbiting the player at
//!      radius `2500.0` at `PI / 8 rad/s` relative to `player.rot.y`.
//!    - **3. Wood Side Apartments 3F (`stg_apart_e3fe.c` Room `0x1F`)**: Moving corridor footsteps
//!      (`0x9C51` -> `sddata[367]`, `flag 552`) triggered across `Z < 17000.0`, translating along `-X` at `700.0 units/s`.
//!    - **4. Toluca Prison South (`stg_prison_s.c` Room `0x72`)**: Invisible Prisoner chanting *"Ritual..."*
//!      (`0x4682` -> `voice.afs[27]`) with randomized 3D cell-block emitter coordinates (`X in [-146000, -134000]`,
//!      `Z in [10400, 32000]`) and Oppressor cell door timer (`flags 288..=290`).
//!    - **5. Blue Creek 2F Corridor (`stg_apart_w2f.c` Room `0x26`)**: Two-stage hysteresis trigger
//!      (arms at `Z > -6500.0`, fires `0x9C6C` -> `sddata[372]` and sets `flag 553` when backing below `Z < -6800.0`).
//!    - **6. Wood Side Apartments 2F (`stg_apart_e2f.c` Room `0x10` / `0x15` / `0x18`)**: Distant scream
//!      `EvProgAnyoneCry` (`(7, 1)`, `0x9C49` -> `sddata[378]`), Room 206 kick
//!      `EvProgMonkeyKick206` (`(7, 17)`, `0x9C47` -> `sddata[373]`), and Room 205 TV static (`0x3E80`).
//!    - **7. Historical Society (`stg_society.c` Room `0x5D` & `0x5C`)**: Long descending staircase cue
//!      (`0x9C73` -> `sddata[388]`) and Room `0x5C` horror cue (`0x4982` -> `voice.afs[42]`, `flag 551`).
//! 2. **Core `EventEngine` (`EventCheck` + `ev_pos` Shapes `0..=13` + 22-Entry `DOOR_SE_TABLE` Sandbox) (`[8]`)**:
//!    - Live 3D wireframe `ev_pos` Line Segment (`shape = 2`), Point (`shape = 0`),
//!      and Convex Quad (`shape = 13`) evaluated every tick via `EventEngine::tick_standard`, plus
//!      interactive locked/unlocked door testing across all 22 `DOOR_SE_TABLE` sound profiles (`0x241bc8`).

use std::time::Duration;
use winit::keyboard::KeyCode;

use crate::rooms::combat_arena::CombatArenaRoomController;
use crate::rooms::RoomController;
use crate::state::AppState;
use sh2_audio::{
    compute_se_call_pos_distance_f, play_menu_sound, SoundCueKind, SFX_MAX_DISTANCE,
    SFX_MIN_DISTANCE,
};
use sh2_core::types::{
    EventListEntry, EventResultKind, EventShapeKind, EventTriggerKind, DOOR_SE_TABLE,
};
use sh2_core::{
    EventEngine, EventEvalInput, Sh2Rng, SpatialSoundCommand, StageEventDispatcher,
    StageHookContext, StageId,
};
use sh2_debug::DebugLineVertex;
use sh2_ui::menu::TitlePhase;

/// Scenario metadata `(title, stage_id_opt, room_id, stage_anchor, initial_local_pos, initial_yaw, controls_hint)`.
#[allow(clippy::type_complexity)]
pub(crate) const HORROR_SCENARIOS: &[(
    &str,
    Option<StageId>,
    u8,
    [f32; 3],
    [f32; 3],
    f32,
    &str,
)] = &[
    (
        "1. Toluca Prison North: Bathroom Stall Knock & 60% Exit Jumpscare (Room 0x73)",
        Some(StageId::PrisonN),
        0x73,
        [-138_700.0, 0.0, -20_000.0],
        [0.0, 0.0, -850.0],
        std::f32::consts::PI,
        "[E] Knock Stall (0x2EF3 + Flag 648) | [Space] Trigger Jumpscare (0x427D) | [K] Test 60%/40% RNG Walk-Out | [R] Re-seed",
    ),
    (
        "2. Blue Creek Apt 2F: Room 209 Orbiting Whisper (Room 0x28, r=2500u @ PI/8 rad/s)",
        Some(StageId::ApartW2f),
        0x28,
        [20_600.0, 0.0, -28_950.0],
        [950.0, 0.0, 0.0],
        -std::f32::consts::FRAC_PI_2,
        "[Space / Walk West] Cross X < s_thr -> Starts Whisper 0x9C70 (Flag 547) Orbiting Player at r=2500 | [R] Reset & Re-seed",
    ),
    (
        "3. Wood Side Apt 3F: Moving Corridor Footsteps (Room 0x1F, -700 u/s along -X)",
        Some(StageId::ApartE3fe),
        0x1F,
        [60_000.0, 0.0, 12_500.0],
        [0.0, 0.0, 0.0],
        std::f32::consts::PI,
        "[Space / Walk South] Trigger Footsteps 0x9C51 (Flag 552) Translating at -700 u/s | [R] Reset",
    ),
    (
        "4. Toluca Prison South: Invisible Prisoner 'Ritual...' (Room 0x72, Randomized 3D Cell Block)",
        Some(StageId::PrisonS),
        0x72,
        [-140_000.0, 0.0, 20_000.0],
        [0.0, 0.0, 1200.0],
        0.0,
        "[Space] Force Immediate 'Ritual...' (0x4682 / voice.afs[27]) at Random Cell Pos | [K] Start Oppressor Cell Timer + SFX",
    ),
    (
        "5. Blue Creek 2F Corridor: Two-Stage Hysteresis Scare (Room 0x26, Arm Z>-6500 -> Fire Z<-6800)",
        Some(StageId::ApartW2f),
        0x26,
        [100_800.0, 0.0, -6_650.0],
        [0.0, 0.0, -500.0],
        0.0,
        "[Space] Step 1: Arm at Z > -6500, Step 2: Back Below Z < -6800 -> Fires 0x9C6C (Flag 553) | [R] Reset Corridor",
    ),
    (
        "6. Wood Side Apt 2F: Distant Scream (EvProgAnyoneCry), Room 206 Kick & Room 205 TV Static",
        Some(StageId::ApartE2f),
        0x18,
        [-99_400.0, 0.0, 25_200.0],
        [0.0, 0.0, 0.0],
        0.0,
        "[Space] Run EvProgAnyoneCry (0x9C49 Scream + 2.0s Msg) | [K] Room 206 Kick (0x9C47) | [T] Room 205 TV Static (0x3E80)",
    ),
    (
        "7. Historical Society: Long Descending Staircase (Room 0x5D, 0x9C73) & Room 0x5C (0x4982)",
        Some(StageId::Society),
        0x5D,
        [-11_520.0, 41_500.0, 40_150.0],
        [0.0, 0.0, 0.0],
        0.0,
        "[Space] Trigger Staircase Ambient (0x9C73) | [K] Trigger Room 0x5C Horror Cue (0x4982 / voice.afs[42], Flag 551)",
    ),
    (
        "8. Core EventEngine Sandbox: ev_pos Shapes (Line=2, Point=0, Quad=13) & 22-Entry DOOR_SE_TABLE",
        None,
        0x01,
        [0.0, 0.0, 0.0],
        [0.0, 0.0, 1850.0],
        0.0,
        "[E / Space] Action on North Door (Line=2) or West Point (Point=0) | [K] Toggle Key Item #1 | [T] Cycle Door Sound (0..21)",
    ),
];

/// Interactive runtime state for [`sh2_ui::menu::TestRoomKind::HorrorTriggersRoom`] (`SH2-47`).
pub(crate) struct HorrorTriggersRoomState {
    pub scenario_idx: usize,
    pub dispatcher: StageEventDispatcher,
    pub event_engine: EventEngine,
    pub rng: Sh2Rng,
    pub seed_counter: u32,
    /// Optional dynamic stage anchor override so multi-room scenarios (e.g. `ApartE2f` / `PrisonS` / `Society`)
    /// keep the listener within `SeCallPosDistanceF` audible range (`< 11,500u`) of the active emitter.
    pub stage_anchor_override: Option<[f32; 3]>,
    /// Active sound effect timers `(se_id, remaining_secs)` so `StageEventDispatcher::on_all_time`
    /// knows while `0x9C70` (orbiting whisper) or `0x9C51` (moving footsteps) is still playing.
    pub active_se_timers: Vec<(u32, f32)>,
    /// Current 3D world-space position of the active horror sound emitter.
    pub active_emitter_world_pos: [f32; 3],
    /// Last triggered `se_id` and its volume.
    pub last_se_id: u32,
    pub last_se_vol: f32,
    /// Queued one-shot action button press (`[E]` / `[Enter]`) for `EventEngine::tick_standard`.
    pub pending_action_down: bool,
    /// Door sound table index (`0..22`) for Scenario 8 (`DOOR_SE_TABLE`).
    pub door_se_idx: usize,
    /// Whether the player currently holds Key Item #1 in Scenario 8.
    pub sandbox_has_key: bool,
    /// Whether to render 3D wireframe trigger zones, threshold planes, and emitter rings.
    pub show_3d_guides: bool,
    /// Human-readable status log of the latest horror trigger or event execution.
    pub last_event_log: String,
    pub initialized: bool,
}

impl Default for HorrorTriggersRoomState {
    fn default() -> Self {
        Self {
            scenario_idx: 0,
            dispatcher: StageEventDispatcher::new(),
            event_engine: EventEngine::new(),
            rng: Sh2Rng::new(0x2001_0924),
            seed_counter: 1,
            stage_anchor_override: None,
            active_se_timers: Vec::new(),
            active_emitter_world_pos: [-138_700.0, -50.0, -21_000.0],
            last_se_id: 0x427D,
            last_se_vol: 1.0,
            pending_action_down: false,
            door_se_idx: 0,
            sandbox_has_key: false,
            show_3d_guides: true,
            last_event_log:
                "Press [1-8] or [Tab] to select a Stage Horror Scenario, [E] to Inspect, [Space] to Trigger, [R] to Reset"
                    .to_string(),
            initialized: false,
        }
    }
}

impl HorrorTriggersRoomState {
    pub fn active_sounds_array(&self) -> [u32; 4] {
        let mut arr = [0u32; 4];
        for (i, &(se_id, _)) in self.active_se_timers.iter().take(4).enumerate() {
            arr[i] = se_id;
        }
        arr
    }

    pub fn mark_se_playing(&mut self, se_id: u32, duration_secs: f32) {
        if let Some(entry) = self
            .active_se_timers
            .iter_mut()
            .find(|(id, _)| *id == se_id)
        {
            entry.1 = duration_secs;
        } else {
            self.active_se_timers.push((se_id, duration_secs));
        }
    }

    pub fn tick_se_timers(&mut self, dt_sec: f32) {
        for entry in &mut self.active_se_timers {
            entry.1 -= dt_sec;
        }
        self.active_se_timers.retain(|&(_, rem)| rem > 0.0);
    }
}

#[inline]
fn active_scenario_anchor(state: &AppState) -> [f32; 3] {
    let idx = state.horror_triggers_room.scenario_idx % HORROR_SCENARIOS.len();
    state
        .horror_triggers_room
        .stage_anchor_override
        .unwrap_or(HORROR_SCENARIOS[idx].3)
}

/// Initializes or resets the currently selected horror scenario (`0..8`), snapping James to the
/// calibrated starting pose and invoking `StageEventDispatcher::on_stage_init` + `on_room_init`.
pub(crate) fn init_horror_scenario(state: &mut AppState, reseed: bool) {
    let idx = state.horror_triggers_room.scenario_idx % HORROR_SCENARIOS.len();
    let (title, stage_id_opt, room_id, _anchor, init_local, init_yaw, _) = HORROR_SCENARIOS[idx];

    if reseed {
        state.horror_triggers_room.seed_counter =
            state.horror_triggers_room.seed_counter.wrapping_add(1);
        let seed = 0x2001_0924u32.wrapping_add(
            state
                .horror_triggers_room
                .seed_counter
                .wrapping_mul(0x9E37_79B9),
        );
        state.horror_triggers_room.rng = Sh2Rng::new(seed);
    }

    state.horror_triggers_room.stage_anchor_override = None;
    state.horror_triggers_room.active_se_timers.clear();
    if let Some(bank) = state.audio_bank.as_mut() {
        bank.stop_all_spatial_handles();
    }
    state.horror_triggers_room.event_engine.reset();
    {
        let s = &mut state.sim_core.scenario;
        s.prog_runner.abort(&mut s.cutaway_overlay, &s.flags);
    }

    // Clear scenario-relevant flags so the user can re-trigger one-shot horror events cleanly
    for &flg in &[
        68, 69, 70, 71, 72, 82, 85, 87, 89, 91, 98, 114, 123, 174, 192, 264, 283, 288, 289, 290,
        309, 544, 547, 548, 550, 551, 552, 553, 634, 642, 643, 648, 649, 700, 701,
    ] {
        state.sim_core.scenario.flags.set(flg, false);
    }

    let local_pos = glam::Vec3::from_array(init_local);
    state.sim_core.snap_player_transform(local_pos, init_yaw);
    state.sim_core.camera.reset_to_player(local_pos, init_yaw);

    if let Some(stage_id) = stage_id_opt {
        state
            .horror_triggers_room
            .dispatcher
            .on_stage_init(stage_id, &mut state.sim_core.scenario.flags);
        state.horror_triggers_room.dispatcher.on_room_init(
            stage_id,
            room_id,
            &mut state.sim_core.scenario.flags,
            &mut state.horror_triggers_room.rng,
        );
    }

    // Default emitter preview position per scenario
    state.horror_triggers_room.active_emitter_world_pos = match idx {
        0 => [-138_700.0, -50.0, -21_000.0],
        1 => [20_600.0, 0.0, -28_950.0],
        2 => [60_000.0, 0.0, 11_350.0],
        3 => state.horror_triggers_room.dispatcher.prison_s_prisoner_pos,
        4 => [100_807.1, -500.0, -6_053.485_4],
        5 => [-99_400.0, -500.0, 26_400.0],
        6 => [-11_520.0, 42_670.0, 40_151.7],
        _ => [0.0, 0.0, 2_200.0],
    };

    state.horror_triggers_room.initialized = true;
    state.horror_triggers_room.last_event_log = format!(
        "Initialized {} | Stage={:?} Room=0x{:02X}",
        title, stage_id_opt, room_id
    );
}

fn apply_spatial_sound_command(state: &mut AppState, cmd: SpatialSoundCommand) {
    let idx = state.horror_triggers_room.scenario_idx % HORROR_SCENARIOS.len();
    let p_local = state.sim_core.scenario.player_position;
    let p_yaw = state.sim_core.scenario.player_yaw;

    match cmd {
        SpatialSoundCommand::Call2D { se_id, volume } => {
            state.horror_triggers_room.last_se_id = se_id;
            state.horror_triggers_room.last_se_vol = volume;
            state.horror_triggers_room.mark_se_playing(se_id, 5.0);
            if let (Some(eng), Some(bank)) =
                (state.audio_engine.as_mut(), state.audio_bank.as_ref())
            {
                let _ = bank.play_se_id(eng, se_id, volume);
            }
            state.horror_triggers_room.last_event_log =
                format!("SeCall(0x{se_id:04X}, vol={volume:.2}) -> Triggered 2D Horror SFX");
        }
        SpatialSoundCommand::CallPos {
            se_id,
            volume,
            pos,
            channel,
        } => {
            state.horror_triggers_room.last_se_id = se_id;
            state.horror_triggers_room.last_se_vol = volume;

            // For Scenario 4 (`PrisonS` Room `0x72` random cell block `[-146000..-134000, 10400..32000]`),
            // keep the stage anchor within `~1600u` of the newly spawned cell so `"Ritual..."` (`0x4682`)
            // is always clearly audible with authentic directional stereo panning.
            if idx == 3 && se_id == 0x4682 {
                state.horror_triggers_room.stage_anchor_override =
                    Some([pos[0] + 900.0 - p_local.x, 0.0, pos[2] - 1200.0 - p_local.z]);
            }

            let anchor = active_scenario_anchor(state);
            let p_world = glam::Vec3::new(
                anchor[0] + p_local.x,
                anchor[1] + p_local.y,
                anchor[2] + p_local.z,
            );

            // For Room 209 whisper (`0x9C70`), `SeCallPos` passes `[0,0,0]` and `SePosMove` immediately sets the 2500u orbit pos
            let emit_world = if se_id == 0x9C70 && pos == [0.0, 0.0, 0.0] {
                let ang = state
                    .horror_triggers_room
                    .dispatcher
                    .apart_w2f_whisper_angle
                    + p_yaw;
                [
                    p_world.x + ang.sin() * 2500.0,
                    p_world.y,
                    p_world.z + ang.cos() * 2500.0,
                ]
            } else {
                pos
            };
            state.horror_triggers_room.active_emitter_world_pos = emit_world;
            let dur = if se_id == 0x9C70 { 6.5 } else { 5.0 };
            state.horror_triggers_room.mark_se_playing(se_id, dur);

            let emit_vec = glam::Vec3::from_array(emit_world);
            if let (Some(eng), Some(bank)) =
                (state.audio_engine.as_mut(), state.audio_bank.as_mut())
            {
                if let Ok(eval) =
                    bank.play_se_id_3d_tracked(eng, se_id, p_world, p_yaw, emit_vec, volume)
                {
                    state.horror_triggers_room.last_event_log = format!(
                        "SeCallPos(0x{:04X}, ch={}) @ [{:.0}, {:.0}, {:.0}] -> dist={:.0}u, gain={:.0}%, pan={:+.2}",
                        se_id,
                        channel,
                        emit_world[0],
                        emit_world[1],
                        emit_world[2],
                        eval.distance,
                        eval.effective_gain * 100.0,
                        eval.stereo_pan
                    );
                }
            }
        }
        SpatialSoundCommand::PosSet {
            se_id,
            volume,
            pos,
            channel: _,
        }
        | SpatialSoundCommand::PosMove { se_id, volume, pos } => {
            state.horror_triggers_room.last_se_id = se_id;
            state.horror_triggers_room.last_se_vol = volume;
            state.horror_triggers_room.active_emitter_world_pos = pos;
            let anchor = active_scenario_anchor(state);
            let p_world = glam::Vec3::new(
                anchor[0] + p_local.x,
                anchor[1] + p_local.y,
                anchor[2] + p_local.z,
            );
            if let (Some(eng), Some(bank)) =
                (state.audio_engine.as_ref(), state.audio_bank.as_mut())
            {
                let _ = bank.update_se_id_3d(
                    eng,
                    se_id,
                    p_world,
                    p_yaw,
                    glam::Vec3::from_array(pos),
                    volume,
                );
            }
        }
        SpatialSoundCommand::SetVolume { se_id, volume } => {
            state.horror_triggers_room.last_se_id = se_id;
            state.horror_triggers_room.last_se_vol = volume;
            let anchor = active_scenario_anchor(state);
            let p_world = glam::Vec3::new(
                anchor[0] + p_local.x,
                anchor[1] + p_local.y,
                anchor[2] + p_local.z,
            );
            let emit_pos =
                glam::Vec3::from_array(state.horror_triggers_room.active_emitter_world_pos);
            if let (Some(eng), Some(bank)) =
                (state.audio_engine.as_ref(), state.audio_bank.as_mut())
            {
                let _ = bank.update_se_id_3d(eng, se_id, p_world, p_yaw, emit_pos, volume);
            }
        }
    }
}

/// Helper to pack an `EventListEntry` matching PS2 `EventListElement` (`0x001a9090`).
#[allow(clippy::too_many_arguments)]
fn make_event_entry(
    flag_0: u16,
    flag_0_inv: bool,
    flag_1: u16,
    flag_1_inv: bool,
    trigger: EventTriggerKind,
    ev_pos_offset: u16,
    shape: EventShapeKind,
    result: EventResultKind,
    door_or_stage: u8,
    msg_or_prog: u8,
    result_flag: u16,
) -> EventListEntry {
    let flag = ((flag_0_inv as u32) << 31)
        | (((flag_0 as u32) & 0x3fff) << 16)
        | ((flag_1_inv as u32) << 15)
        | ((flag_1 as u32) & 0x3fff);
    let cond = ((trigger as u32) << 28)
        | (((ev_pos_offset as u32) & 0xfff) << 16)
        | ((shape as u32) << 12);
    let rslt0 = (result as u32) << 28;
    let rslt1 = (((door_or_stage as u32) & 0x3f) << 22)
        | ((msg_or_prog as u32) << 14)
        | ((result_flag as u32) & 0x3fff);
    EventListEntry {
        flag,
        cond,
        rslt0,
        rslt1,
    }
}

fn push_i16(buf: &mut Vec<u8>, v: i16) {
    buf.extend_from_slice(&v.to_le_bytes());
}

/// Builds the synthetic `EventListEntry` table and `ev_pos` byte stream (`i16` coordinates matching `CharToFloat2`)
/// for Scenario 1 (`PrisonN` Room `0x73` stall door inspect) or Scenario 8 (`EventEngine` sandbox).
fn build_scenario_event_tables(
    scenario_idx: usize,
    door_se_idx: usize,
    sandbox_has_key: bool,
) -> (Vec<EventListEntry>, Vec<u8>) {
    let mut ev_list = Vec::new();
    let mut ev_pos = Vec::new();

    if scenario_idx == 0 {
        // Scenario 1 (evaluated in local chamber coordinates around `(0, 0, -1000)`):
        // Offset 0 (8 bytes): `EventShapeKind::Point` `(x = 0, y = -200, z = -1000, radius = 650)`
        push_i16(&mut ev_pos, 0);
        push_i16(&mut ev_pos, -200);
        push_i16(&mut ev_pos, -1000);
        push_i16(&mut ev_pos, 650);

        ev_list.push(make_event_entry(
            648,
            true, // require `!flag(648)`
            0,
            false,
            EventTriggerKind::ActionLook,
            0,
            EventShapeKind::Point,
            EventResultKind::FlagSet,
            0,
            0,
            648, // sets `flag(648)` (arms bathroom stall jumpscare!)
        ));
        ev_list.push(EventListEntry {
            flag: 0x3fff,
            ..Default::default()
        });
        return (ev_list, ev_pos);
    }

    if scenario_idx == 7 {
        // Scenario 8: Core `EventEngine` Sandbox around `(0, 0, 0)`
        // 1. Offset 0 (10 bytes): `EventShapeKind::LinePosX` `(x = -800, y = -200, z = 2200, radius = 650, length = 1600)`
        push_i16(&mut ev_pos, -800);
        push_i16(&mut ev_pos, -200);
        push_i16(&mut ev_pos, 2200);
        push_i16(&mut ev_pos, 650);
        push_i16(&mut ev_pos, 1600);

        // 2. Offset 10 (8 bytes): `EventShapeKind::Point` `(x = -2000, y = -200, z = 0, radius = 850)`
        push_i16(&mut ev_pos, -2000);
        push_i16(&mut ev_pos, -200);
        push_i16(&mut ev_pos, 0);
        push_i16(&mut ev_pos, 850);

        // 3. Offset 18 (18 bytes): `EventShapeKind::Quad` `(x0, y, z0, x1, z1, x2, z2, x3, z3)` `[1400..2800] x [-800..800]`
        push_i16(&mut ev_pos, 1400);
        push_i16(&mut ev_pos, -200);
        push_i16(&mut ev_pos, -800);
        push_i16(&mut ev_pos, 1400);
        push_i16(&mut ev_pos, 800);
        push_i16(&mut ev_pos, 2800);
        push_i16(&mut ev_pos, 800);
        push_i16(&mut ev_pos, 2800);
        push_i16(&mut ev_pos, -800);

        let d_se = (door_se_idx % DOOR_SE_TABLE.len()) as u8;
        let door_res = if sandbox_has_key {
            EventResultKind::DoorUnlocked
        } else {
            EventResultKind::DoorLocked
        };

        // Event 0: North Door (`ActionLook` on `LinePosX` at offset 0)
        ev_list.push(make_event_entry(
            0,
            false,
            0,
            false,
            EventTriggerKind::ActionLook,
            0,
            EventShapeKind::LinePosX,
            door_res,
            d_se,
            0xfe, // "The door is unlocked." / locked message
            if sandbox_has_key { 701 } else { 0 },
        ));

        // Event 1: West Point Message (`ActionLook` on `Point` at offset 10)
        ev_list.push(make_event_entry(
            0,
            false,
            0,
            false,
            EventTriggerKind::ActionLook,
            10,
            EventShapeKind::Point,
            EventResultKind::Message,
            0,
            101,
            0,
        ));

        // Event 2: East Quad Rising-Edge Auto-Trigger (`ZoneEnterEdge` on `Quad` at offset 18 -> sets `flag(700)`)
        ev_list.push(make_event_entry(
            0,
            false,
            0,
            false,
            EventTriggerKind::ZoneEnterEdge,
            18,
            EventShapeKind::Quad,
            EventResultKind::FlagSet,
            0,
            0,
            700,
        ));

        ev_list.push(EventListEntry {
            flag: 0x3fff,
            ..Default::default()
        });
    }

    (ev_list, ev_pos)
}

/// Builds 3D wireframe guides for the active horror scenario (threshold planes, orbiting whisper track,
/// 3D sound emitter beacon + `SeCallPosDistanceF` rings, and `ev_pos` trigger zones).
pub(crate) fn build_horror_debug_lines(state: &AppState) -> Vec<DebugLineVertex> {
    let mut lines = Vec::with_capacity(512);
    let idx = state.horror_triggers_room.scenario_idx % HORROR_SCENARIOS.len();
    let anchor = active_scenario_anchor(state);
    let p_local = state.sim_core.scenario.player_position;

    let cyan = [0.25, 0.90, 1.0, 0.95];
    let red = [1.0, 0.25, 0.30, 0.95];
    let green = [0.25, 0.95, 0.45, 0.95];
    let gold = [1.0, 0.85, 0.20, 0.95];
    let y = -6.0f32;

    match idx {
        0 => {
            // Scenario 1 (`PrisonN` Room `0x73`): Stall inspect circle at `(0, -1000)` + exit threshold line `Z = s_thr - anchor.z`
            let stall_col = if state.sim_core.scenario.flags.get(648) {
                green
            } else {
                gold
            };
            push_circle_xz(&mut lines, 0.0, -1000.0, y, 650.0, 24, stall_col);
            let z_thr_local = state
                .horror_triggers_room
                .dispatcher
                .prison_n_bathroom_z_threshold
                - anchor[2];
            let line_col = if state.sim_core.scenario.flags.get(649) {
                red
            } else if state.sim_core.scenario.flags.get(648) {
                green
            } else {
                cyan
            };
            lines.push(DebugLineVertex::new([-2500.0, y, z_thr_local], line_col));
            lines.push(DebugLineVertex::new([2500.0, y, z_thr_local], line_col));
        }
        1 => {
            // Scenario 2 (`ApartW2f` Room `0x28` / Room 209): Whisper threshold `X = s_thr - anchor.x` + 2500u orbit ring around player
            let x_thr_local = state
                .horror_triggers_room
                .dispatcher
                .apart_w2f_room209_x_threshold
                - anchor[0];
            lines.push(DebugLineVertex::new([x_thr_local, y, -2500.0], cyan));
            lines.push(DebugLineVertex::new([x_thr_local, y, 2500.0], cyan));
            push_circle_xz(&mut lines, p_local.x, p_local.z, y, 2500.0, 36, gold);
        }
        2 => {
            // Scenario 3 (`ApartE3fe` Room `0x1F`): Footstep trigger line `Z = 17000 - anchor.z = +1000`
            let z_thr_local = 17_000.0 - anchor[2];
            lines.push(DebugLineVertex::new([-2500.0, y, z_thr_local], cyan));
            lines.push(DebugLineVertex::new([2500.0, y, z_thr_local], cyan));
        }
        4 => {
            // Scenario 5 (`ApartW2f` Room `0x26`): Arm line `Z = -6500` (+150) & Fire line `Z = -6800` (-150)
            let z_arm = -6500.0 - anchor[2];
            let z_fire = -6800.0 - anchor[2];
            lines.push(DebugLineVertex::new([-2500.0, y, z_arm], green));
            lines.push(DebugLineVertex::new([2500.0, y, z_arm], green));
            lines.push(DebugLineVertex::new([-2500.0, y, z_fire], red));
            lines.push(DebugLineVertex::new([2500.0, y, z_fire], red));
        }
        6 => {
            // Scenario 7 (`Society` Room `0x5D`): Staircase depth axis
            lines.push(DebugLineVertex::new([-2500.0, y, 0.0], cyan));
            lines.push(DebugLineVertex::new([2500.0, y, 0.0], cyan));
        }
        7 => {
            // Scenario 8 (`EventEngine` Sandbox):
            // 1. North Door Line Segment `(-800, 2200) -> (800, 2200)` + front normal arrow
            let door_col = if state.horror_triggers_room.sandbox_has_key
                || state.sim_core.scenario.flags.get(701)
            {
                green
            } else {
                red
            };
            lines.push(DebugLineVertex::new([-800.0, y, 2200.0], door_col));
            lines.push(DebugLineVertex::new([800.0, y, 2200.0], door_col));
            lines.push(DebugLineVertex::new([0.0, y, 2200.0], door_col));
            lines.push(DebugLineVertex::new([0.0, y, 1650.0], door_col));

            // 2. West Point + Radius at `(-2000, 0)`
            push_circle_xz(&mut lines, -2000.0, 0.0, y, 850.0, 24, cyan);

            // 3. East Auto-Trigger Quad `[1400..2800] x [-800..800]`
            let q_col = if state.sim_core.scenario.flags.get(700) {
                green
            } else {
                cyan
            };
            let corners = [
                [1400.0, y, -800.0],
                [1400.0, y, 800.0],
                [2800.0, y, 800.0],
                [2800.0, y, -800.0],
            ];
            for i in 0..4 {
                lines.push(DebugLineVertex::new(corners[i], q_col));
                lines.push(DebugLineVertex::new(corners[(i + 1) % 4], q_col));
            }
        }
        _ => {}
    }

    // Draw active 3D sound emitter beacon in local room coordinates (clamped into visible hall bounds if far away)
    let emit_w = state.horror_triggers_room.active_emitter_world_pos;
    let emit_local_x = (emit_w[0] - anchor[0]).clamp(-5200.0, 5200.0);
    let emit_local_z = (emit_w[2] - anchor[2]).clamp(-7200.0, 7200.0);

    let beacon = sh2_debug::build_chara_collision_debug_lines(
        [emit_local_x, 0.0, emit_local_z],
        180.0,
        1400.0,
        16,
        [1.0, 0.55, 0.15, 1.0],
    );
    lines.extend(beacon);
    push_circle_xz(
        &mut lines,
        emit_local_x,
        emit_local_z,
        -4.0,
        SFX_MIN_DISTANCE,
        32,
        [0.2, 0.95, 0.4, 0.85],
    );
    push_circle_xz(
        &mut lines,
        emit_local_x,
        emit_local_z,
        -4.0,
        SFX_MAX_DISTANCE,
        36,
        [0.95, 0.25, 0.25, 0.65],
    );

    lines
}

fn push_circle_xz(
    lines: &mut Vec<DebugLineVertex>,
    cx: f32,
    cz: f32,
    y: f32,
    radius: f32,
    segs: usize,
    color: [f32; 4],
) {
    for i in 0..segs {
        let a0 = (i as f32) * std::f32::consts::TAU / (segs as f32);
        let a1 = ((i + 1) as f32) * std::f32::consts::TAU / (segs as f32);
        lines.push(DebugLineVertex::new(
            [cx + a0.cos() * radius, y, cz + a0.sin() * radius],
            color,
        ));
        lines.push(DebugLineVertex::new(
            [cx + a1.cos() * radius, y, cz + a1.sin() * radius],
            color,
        ));
    }
}

pub(crate) struct HorrorTriggersRoomController;

impl RoomController for HorrorTriggersRoomController {
    fn handle_key_input(state: &mut AppState, key_code: KeyCode, pressed: bool, repeat: bool) {
        if pressed && !repeat {
            match key_code {
                KeyCode::Escape | KeyCode::Backspace => {
                    state.exit_room_to_menu(TitlePhase::TestRoomsSelect);
                    tracing::info!(
                        "Returned from Stage Events & Horror Triggers Room to Test Rooms menu."
                    );
                    return;
                }
                // [1]..=[8]: Select Scenario 1..=8
                KeyCode::Digit1
                | KeyCode::Digit2
                | KeyCode::Digit3
                | KeyCode::Digit4
                | KeyCode::Digit5
                | KeyCode::Digit6
                | KeyCode::Digit7
                | KeyCode::Digit8 => {
                    state.horror_triggers_room.scenario_idx = match key_code {
                        KeyCode::Digit1 => 0,
                        KeyCode::Digit2 => 1,
                        KeyCode::Digit3 => 2,
                        KeyCode::Digit4 => 3,
                        KeyCode::Digit5 => 4,
                        KeyCode::Digit6 => 5,
                        KeyCode::Digit7 => 6,
                        _ => 7,
                    };
                    init_horror_scenario(state, false);
                    if let Some(eng) = state.audio_engine.as_mut() {
                        let _ = play_menu_sound(
                            eng,
                            state.audio_bank.as_ref(),
                            SoundCueKind::CursorMove,
                        );
                    }
                    return;
                }
                KeyCode::Tab | KeyCode::PageDown => {
                    state.horror_triggers_room.scenario_idx =
                        (state.horror_triggers_room.scenario_idx + 1) % HORROR_SCENARIOS.len();
                    init_horror_scenario(state, false);
                    return;
                }
                KeyCode::PageUp => {
                    state.horror_triggers_room.scenario_idx =
                        if state.horror_triggers_room.scenario_idx == 0 {
                            HORROR_SCENARIOS.len() - 1
                        } else {
                            state.horror_triggers_room.scenario_idx - 1
                        };
                    init_horror_scenario(state, false);
                    return;
                }
                // [R]: Reset & re-seed current room (`on_room_init`)
                KeyCode::KeyR => {
                    init_horror_scenario(state, true);
                    return;
                }
                // [E] / [Enter]: Queue `action_pressed` for `EventEngine::tick_standard` (stall door knock / sandbox door inspect)
                KeyCode::KeyE | KeyCode::Enter => {
                    state.horror_triggers_room.pending_action_down = true;
                    return;
                }
                // [Space]: Step across the active scenario's spatial threshold or trigger the scripted horror event immediately
                KeyCode::Space => {
                    trigger_scenario_quick_step(state);
                    return;
                }
                // [K]: Secondary scenario action (Toggle Key in Scenario 8, Room 206 Kick in Scenario 6, Oppressor Timer in Scenario 4, Room 0x5C in Scenario 7)
                KeyCode::KeyK => {
                    trigger_scenario_secondary_action(state);
                    return;
                }
                // [T]: Cycle `DOOR_SE_TABLE` index in Scenario 8, or trigger Room 205 TV Static (`0x3E80`) in Scenario 6
                KeyCode::KeyT => {
                    let idx = state.horror_triggers_room.scenario_idx % HORROR_SCENARIOS.len();
                    if idx == 5 {
                        // Scenario 6 (`ApartE2f` Room `0x18` TV Static `0x3E80` requires `flag(69)`)
                        // Place stage anchor right inside Room 205 (`800u` from TV at `[-58082.375, -356.5, 19011.39]`)
                        let p_loc = state.sim_core.scenario.player_position;
                        state.horror_triggers_room.stage_anchor_override =
                            Some([-58_082.0 - p_loc.x, 0.0, 19_800.0 - p_loc.z]);
                        state.sim_core.scenario.flags.set(69, true);
                        let out = state
                            .horror_triggers_room
                            .dispatcher
                            .on_sound_call_after_load(
                                StageId::ApartE2f,
                                0x18,
                                &mut state.sim_core.scenario.flags,
                            );
                        for cmd in out.sounds {
                            apply_spatial_sound_command(state, cmd);
                        }
                    } else {
                        state.horror_triggers_room.door_se_idx =
                            (state.horror_triggers_room.door_se_idx + 1) % DOOR_SE_TABLE.len();
                        let d_idx = state.horror_triggers_room.door_se_idx;
                        let ds = DOOR_SE_TABLE[d_idx];
                        let preview_se = if state.horror_triggers_room.sandbox_has_key {
                            ds.unlock
                        } else {
                            ds.lock
                        } as u16 as u32;
                        if preview_se != 0 {
                            apply_spatial_sound_command(
                                state,
                                SpatialSoundCommand::CallPos {
                                    se_id: preview_se,
                                    volume: 1.0,
                                    pos: [0.0, -500.0, 2200.0],
                                    channel: 0,
                                },
                            );
                        }
                        state.horror_triggers_room.last_event_log = format!(
                            "Selected DOOR_SE_TABLE[{}]: open=0x{:04X}, close=0x{:04X}, lock=0x{:04X}, unlock=0x{:04X}",
                            d_idx, ds.open, ds.close, ds.lock, ds.unlock
                        );
                    }
                    return;
                }
                // [V]: Toggle 3D wireframe trigger guides
                KeyCode::KeyV => {
                    state.horror_triggers_room.show_3d_guides =
                        !state.horror_triggers_room.show_3d_guides;
                    return;
                }
                _ => {}
            }
        }

        CombatArenaRoomController::handle_key_input(state, key_code, pressed, repeat);
    }

    fn tick_and_render_3d(
        state: &mut AppState,
        dt: Duration,
        dt_sec: f32,
        aspect: f32,
        grain_pct: u32,
        is_paused: bool,
        is_in_game: bool,
    ) {
        if !state.horror_triggers_room.initialized {
            init_horror_scenario(state, false);
        }

        if !is_paused {
            state.horror_triggers_room.tick_se_timers(dt_sec);

            let idx = state.horror_triggers_room.scenario_idx % HORROR_SCENARIOS.len();
            let (_, stage_id_opt, room_id, _, _, _, _) = HORROR_SCENARIOS[idx];
            let anchor = active_scenario_anchor(state);
            let p_local = state.sim_core.scenario.player_position;
            let p_yaw = state.sim_core.scenario.player_yaw;
            let p_world = [
                anchor[0] + p_local.x,
                anchor[1] + p_local.y,
                anchor[2] + p_local.z,
            ];

            // 1. Evaluate declarative `EventEngine::tick_standard` (in local chamber coordinates for Scenario 1 stall inspect & Scenario 8 sandbox)
            let action_down = std::mem::take(&mut state.horror_triggers_room.pending_action_down);
            if idx == 0 || idx == 7 {
                let (ev_list, ev_pos) = build_scenario_event_tables(
                    idx,
                    state.horror_triggers_room.door_se_idx,
                    state.horror_triggers_room.sandbox_has_key,
                );
                let eval_in = EventEvalInput {
                    player_pos: [p_local.x, p_local.y, p_local.z],
                    player_rot_y: p_yaw,
                    action_pressed: action_down,
                    ..Default::default()
                };
                let mut inv = state.sim_core.scenario.inventory_mgr.clone();
                let effects = state.horror_triggers_room.event_engine.tick_standard(
                    &ev_list,
                    &[],
                    &ev_pos,
                    1,
                    &eval_in,
                    true,
                    true,
                    true,
                    true,
                    &mut state.sim_core.scenario.flags,
                    &mut inv,
                );
                if idx == 0 && action_down {
                    // Always arm `flag(648)` and play the bathroom stall knock (`0x2EF3`) when `[E]` is pressed in Scenario 1
                    state.sim_core.scenario.flags.set(648, true);
                    state.sim_core.scenario.flags.set(649, false);
                    state.horror_triggers_room.event_engine.reset();
                    apply_spatial_sound_command(
                        state,
                        SpatialSoundCommand::CallPos {
                            se_id: 0x2EF3,
                            volume: 0.95,
                            pos: [-138_700.0, -200.0, -21_000.0],
                            channel: 0,
                        },
                    );
                    state.horror_triggers_room.last_event_log = format!(
                        "Knocked on Bathroom Stall Door (0x2EF3) -> Flag 648 ON! Walk North across Z > {:.0} (or press [Space])",
                        state
                            .horror_triggers_room
                            .dispatcher
                            .prison_n_bathroom_z_threshold
                    );
                } else if idx == 7 {
                    for snd in &effects.positional_sounds {
                        apply_spatial_sound_command(
                            state,
                            SpatialSoundCommand::CallPos {
                                se_id: snd.se_id as u16 as u32,
                                volume: snd.volume,
                                pos: snd.pos,
                                channel: snd.channel,
                            },
                        );
                    }
                    if let Some(msg) = effects.stage_message {
                        state.horror_triggers_room.last_event_log = format!(
                            "EventEngine triggered Stage Message #{msg} (Quad Flag 700={})",
                            state.sim_core.scenario.flags.get(700)
                        );
                        state.horror_triggers_room.event_engine.reset();
                    } else if let Some(c_msg) = effects.common_message {
                        state.horror_triggers_room.last_event_log = format!(
                            "Door Interaction -> Common Message #{c_msg} (Door Unlocked Flag 701={}, Door SE #{})",
                            state.sim_core.scenario.flags.get(701),
                            state.horror_triggers_room.door_se_idx
                        );
                        state.horror_triggers_room.event_engine.reset();
                    }
                }
            }

            // 2. Advance `StageProgramRunner` if active (Scenario 6: `(7, 1)` `EvProgAnyoneCry` or `(7, 17)` `EvProgMonkeyKick206`)
            if state.sim_core.scenario.prog_runner.is_running() {
                let s = &mut state.sim_core.scenario;
                s.prog_runner.tick(
                    sh2_core::PROG_KEY_A,
                    0,
                    dt_sec,
                    &mut s.flags,
                    &mut s.puzzle_state,
                    &mut s.inventory_mgr,
                    &mut s.cutaway_overlay,
                    s.riddle_difficulty,
                );
                let pending_ses: Vec<u32> = s.prog_runner.sound_effects().to_vec();
                for se_id in pending_ses {
                    let emit_pos = if se_id == 0x9C49 {
                        [-99_400.0, -500.0, 26_400.0]
                    } else {
                        [-99_000.0, -500.0, 13_600.0]
                    };
                    apply_spatial_sound_command(
                        state,
                        SpatialSoundCommand::CallPos {
                            se_id,
                            volume: 1.0,
                            pos: emit_pos,
                            channel: 6,
                        },
                    );
                }
            }

            // 3. Advance per-frame `StageEventDispatcher::on_all_time`
            if let Some(stage_id) = stage_id_opt {
                let prev_648 = state.sim_core.scenario.flags.get(648);
                let prev_649 = state.sim_core.scenario.flags.get(649);
                let active_sounds = state.horror_triggers_room.active_sounds_array();
                let ctx = StageHookContext {
                    stage: stage_id,
                    room_id,
                    dt: dt_sec,
                    player_pos: p_world,
                    player_rot_y: p_yaw,
                    active_sounds,
                };
                let out = state.horror_triggers_room.dispatcher.on_all_time(
                    &ctx,
                    &mut state.sim_core.scenario.flags,
                    &mut state.horror_triggers_room.rng,
                );
                for cmd in out.sounds {
                    apply_spatial_sound_command(state, cmd);
                }
                // Detect 40% fake-out disarm in Prison North Bathroom (`flag 648` cleared without `flag 649` set)
                if idx == 0
                    && prev_648
                    && !prev_649
                    && !state.sim_core.scenario.flags.get(648)
                    && !state.sim_core.scenario.flags.get(649)
                {
                    state.horror_triggers_room.last_event_log =
                        "Crossed Z > s_thr: 40% RNG roll disarmed Flag 648 (No Jumpscare)! Press [Space] for guaranteed Jumpscare or [K] to re-roll"
                            .to_string();
                }
            }

            // 4. Continuously update stereo panning & distance attenuation of any active 3D emitter as James walks/turns
            if !state.horror_triggers_room.active_se_timers.is_empty() {
                let last_se = state.horror_triggers_room.last_se_id;
                let last_vol = state.horror_triggers_room.last_se_vol;
                let emit_pos =
                    glam::Vec3::from_array(state.horror_triggers_room.active_emitter_world_pos);
                let p_world_vec = glam::Vec3::from_array(p_world);
                if let (Some(eng), Some(bank)) =
                    (state.audio_engine.as_ref(), state.audio_bank.as_mut())
                {
                    let _ =
                        bank.update_se_id_3d(eng, last_se, p_world_vec, p_yaw, emit_pos, last_vol);
                }
            }
        }

        // Render 3D character, map, and horror trigger wireframe guides
        CombatArenaRoomController::tick_and_render_3d(
            state, dt, dt_sec, aspect, grain_pct, is_paused, false,
        );

        if is_in_game {
            let (player_pos, _) = state
                .sim_core
                .interpolated_player_pose(state.sim_core.interpolation_alpha());
            let mut skeletal_draw = Vec::new();
            let active_char_ref = if state.active_maria {
                state.maria.as_ref().or(state.character.as_ref())
            } else {
                state.character.as_ref().or(state.maria.as_ref())
            };
            if let Some(c) = active_char_ref {
                if !state.skin_matrices_scratch.is_empty() {
                    skeletal_draw.push((
                        &c.gpu_mesh,
                        state.skin_matrices_scratch.as_slice(),
                        Some(c.texture_bind_groups.as_slice()),
                    ));
                }
            }

            if state.horror_triggers_room.show_3d_guides {
                let mut debug_lines = build_horror_debug_lines(state);
                let chara_lines = sh2_debug::build_chara_collision_debug_lines(
                    player_pos.into(),
                    state.sim_core.collision_config.radius,
                    state.sim_core.collision_config.height,
                    16,
                    [1.0, 0.9, 0.1, 1.0],
                );
                debug_lines.extend(chara_lines);

                state.render_ctx.render_scene_with_debug_lines(
                    Some(&state.gpu_map),
                    Some(&state.map_texture_bind_groups),
                    &skeletal_draw,
                    &debug_lines,
                );
            } else {
                state.render_ctx.render_scene(
                    Some(&state.gpu_map),
                    Some(&state.map_texture_bind_groups),
                    &skeletal_draw,
                );
            }
        }
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

        let idx = state.horror_triggers_room.scenario_idx % HORROR_SCENARIOS.len();
        let (title, stage_id_opt, room_id, _, _, _, controls_hint) = HORROR_SCENARIOS[idx];
        let anchor = active_scenario_anchor(state);
        let p_local = state.sim_core.scenario.player_position;
        let p_yaw = state.sim_core.scenario.player_yaw;
        let p_world = glam::Vec3::new(
            anchor[0] + p_local.x,
            anchor[1] + p_local.y,
            anchor[2] + p_local.z,
        );
        let emit_world =
            glam::Vec3::from_array(state.horror_triggers_room.active_emitter_world_pos);
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
                title, stage_id_opt, room_id
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

        // Scenario-specific telemetry line
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

        // Scenario selector list
        state.menu_rasterizer.draw_text(
            "--- 8 STAGE EVENT & ENVIRONMENTAL HORROR SCENARIOS (Press [1-8] or [Tab / PgUp / PgDn]) ---",
            20,
            114,
            1,
            yellow,
            true,
        );
        for (i, &(label, _, _, _, _, _, _)) in HORROR_SCENARIOS.iter().enumerate() {
            let y = 134 + i * 20;
            let is_cur = i == idx;
            let col = if is_cur { green } else { white };
            let prefix = if is_cur { ">> " } else { "   " };
            state
                .menu_rasterizer
                .draw_text(&format!("{prefix}{label}"), 20, y, 1, col, true);
        }

        state.menu_rasterizer.draw_text(
            &format!("CONTROLS FOR THIS SCENARIO: {controls_hint}"),
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
}

fn trigger_scenario_quick_step(state: &mut AppState) {
    let idx = state.horror_triggers_room.scenario_idx % HORROR_SCENARIOS.len();
    let anchor = active_scenario_anchor(state);

    match idx {
        0 => {
            // Scenario 1 (`PrisonN` Room `0x73`): Arm `flag(648)`, clear `flag(649)`, step North across `Z > s_thr`,
            // and immediately trigger the bathroom stall bang jumpscare (`0x427D` / `sddata[316]`)
            state.sim_core.scenario.flags.set(648, true);
            state.sim_core.scenario.flags.set(649, true);
            let z_local = (state
                .horror_triggers_room
                .dispatcher
                .prison_n_bathroom_z_threshold
                - anchor[2])
                + 150.0;
            state
                .sim_core
                .snap_player_transform(glam::Vec3::new(0.0, 0.0, z_local), 0.0);
            apply_spatial_sound_command(
                state,
                SpatialSoundCommand::CallPos {
                    se_id: 0x427D,
                    volume: 1.0,
                    pos: [-138_700.0, -50.0, -21_000.0],
                    channel: 0,
                },
            );
        }
        1 => {
            // Scenario 2 (`ApartW2f` Room `0x28` / Room 209): Reset `flag(547)` and step West across `X < s_thr`
            state.sim_core.scenario.flags.set(547, false);
            let x_local = (state
                .horror_triggers_room
                .dispatcher
                .apart_w2f_room209_x_threshold
                - anchor[0])
                - 150.0;
            state.sim_core.snap_player_transform(
                glam::Vec3::new(x_local, 0.0, 0.0),
                -std::f32::consts::FRAC_PI_2,
            );
        }
        2 => {
            // Scenario 3 (`ApartE3fe` Room `0x1F`): Reset `flag(552)` and step South near the footstep emitter (`[60000, 0, 11350]`)
            state.sim_core.scenario.flags.set(552, false);
            state
                .horror_triggers_room
                .dispatcher
                .apart_e3fe_footstep_timer = 0.0;
            state
                .sim_core
                .snap_player_transform(glam::Vec3::new(0.0, 0.0, 0.0), std::f32::consts::PI);
        }
        3 => {
            // Scenario 4 (`PrisonS` Room `0x72`): Expire `prisoner_timer` to immediately fire `0x4682` (`"Ritual..."`)
            state
                .horror_triggers_room
                .dispatcher
                .prison_s_prisoner_timer = 0.0;
        }
        4 => {
            // Scenario 5 (`ApartW2f` Room `0x26`): Step 1 arms at `Z > -6500`, Step 2 fires `0x9C6C` at `Z < -6800`
            if !state
                .horror_triggers_room
                .dispatcher
                .apart_w2f_corridor_armed
            {
                let z_arm = (-6500.0 - anchor[2]) + 150.0;
                state
                    .sim_core
                    .snap_player_transform(glam::Vec3::new(0.0, 0.0, z_arm), 0.0);
                if let Some(eng) = state.audio_engine.as_mut() {
                    let _ = play_menu_sound(eng, state.audio_bank.as_ref(), SoundCueKind::Confirm);
                }
                state.horror_triggers_room.last_event_log =
                    "Step 1: Crossed Z > -6500 -> apart_w2f_corridor_armed = true! Press [Space] again to back below Z < -6800"
                        .to_string();
            } else {
                state.sim_core.scenario.flags.set(553, false);
                let z_fire = (-6800.0 - anchor[2]) - 150.0;
                state
                    .sim_core
                    .snap_player_transform(glam::Vec3::new(0.0, 0.0, z_fire), std::f32::consts::PI);
            }
        }
        5 => {
            // Scenario 6 (`ApartE2f`): Place stage anchor 1200u from `EvProgAnyoneCry` emitter `[-99400, -500, 26400]`
            let p_loc = state.sim_core.scenario.player_position;
            state.horror_triggers_room.stage_anchor_override =
                Some([-99_400.0 - p_loc.x, 0.0, 25_200.0 - p_loc.z]);
            state.sim_core.scenario.prog_runner.start(7, 1, 71);
            state.horror_triggers_room.last_event_log =
                "Started StageProgramRunner (7, 1) EvProgAnyoneCry -> SE 0x9C49 Scream + 2.0s Message!"
                    .to_string();
        }
        6 => {
            // Scenario 7 (`Society` Room `0x5D`): Reset staircase anchor and expire `society_staircase_timer` to fire `0x9C73`
            state.horror_triggers_room.stage_anchor_override = None;
            state
                .horror_triggers_room
                .dispatcher
                .society_staircase_timer = 0.0;
        }
        _ => {
            // Scenario 8 (`EventEngine` Sandbox): Snap in front of North Door (`[0, 0, 1850]`, yaw=0) and queue `action_pressed`
            state
                .sim_core
                .snap_player_transform(glam::Vec3::new(0.0, 0.0, 1850.0), 0.0);
            state.horror_triggers_room.event_engine.reset();
            state.horror_triggers_room.pending_action_down = true;
        }
    }
}

fn trigger_scenario_secondary_action(state: &mut AppState) {
    let idx = state.horror_triggers_room.scenario_idx % HORROR_SCENARIOS.len();
    match idx {
        0 => {
            // Scenario 1 (`PrisonN` Room `0x73`): Test authentic 60% Jumpscare / 40% Fake-out RNG roll across `Z > s_thr`
            let anchor = active_scenario_anchor(state);
            state.sim_core.scenario.flags.set(648, true);
            state.sim_core.scenario.flags.set(649, false);
            let z_local = (state
                .horror_triggers_room
                .dispatcher
                .prison_n_bathroom_z_threshold
                - anchor[2])
                + 150.0;
            state
                .sim_core
                .snap_player_transform(glam::Vec3::new(0.0, 0.0, z_local), 0.0);
        }
        3 => {
            // Scenario 4 (`PrisonS` Room `0x72`): Set `flag(288)` & `flag(289)` to start Oppressor cell timer + play Room `0x68` mechanism SFX (`0x427B`)
            state.sim_core.scenario.flags.set(288, true);
            state.sim_core.scenario.flags.set(289, true);
            state.sim_core.scenario.flags.set(290, false);
            state.horror_triggers_room.dispatcher.prison_s_bars_timer = 0.0;
            let p_world = active_scenario_anchor(state);
            apply_spatial_sound_command(
                state,
                SpatialSoundCommand::CallPos {
                    se_id: 0x427B,
                    volume: 0.9,
                    pos: [p_world[0], -300.0, p_world[2] + 800.0],
                    channel: 0,
                },
            );
            state.horror_triggers_room.last_event_log =
                "Set Flags 288 & 289 ON -> Started Prison South Oppressor Cell Door Timer + SFX 0x427B"
                    .to_string();
        }
        5 => {
            // Scenario 6 (`ApartE2f` Room 206): Place stage anchor 1000u from Room 206 `[-99000, -500, 13600]` and run `(7, 17)` `EvProgMonkeyKick206`
            let p_loc = state.sim_core.scenario.player_position;
            state.horror_triggers_room.stage_anchor_override =
                Some([-99_000.0 - p_loc.x, 0.0, 14_600.0 - p_loc.z]);
            state.sim_core.scenario.flags.set(69, false);
            state.sim_core.scenario.prog_runner.start(7, 17, 91);
            state.horror_triggers_room.last_event_log =
                "Started StageProgramRunner (7, 17) EvProgMonkeyKick206 -> SE 0x9C47 Room 206 Kick!"
                    .to_string();
        }
        6 => {
            // Scenario 7 (`Society` Room `0x5C`): Place stage anchor 1500u from Room `0x5C` emitter `[-22000, -500, -18900]` and evaluate `on_all_time`
            let p_loc = state.sim_core.scenario.player_position;
            state.horror_triggers_room.stage_anchor_override =
                Some([-20_500.0 - p_loc.x, 0.0, -18_900.0 - p_loc.z]);
            state.sim_core.scenario.flags.set(551, false);
            let ctx = StageHookContext {
                stage: StageId::Society,
                room_id: 0x5C,
                dt: 1.0 / 60.0,
                player_pos: [-19_000.0, 0.0, -18_900.0],
                player_rot_y: state.sim_core.scenario.player_yaw,
                active_sounds: state.horror_triggers_room.active_sounds_array(),
            };
            let out = state.horror_triggers_room.dispatcher.on_all_time(
                &ctx,
                &mut state.sim_core.scenario.flags,
                &mut state.horror_triggers_room.rng,
            );
            for cmd in out.sounds {
                apply_spatial_sound_command(state, cmd);
            }
        }
        _ => {
            // Scenario 8: Toggle Key Item #1 and play feedback cue
            state.horror_triggers_room.sandbox_has_key =
                !state.horror_triggers_room.sandbox_has_key;
            if let Some(eng) = state.audio_engine.as_mut() {
                let _ = play_menu_sound(eng, state.audio_bank.as_ref(), SoundCueKind::Confirm);
            }
            state.horror_triggers_room.last_event_log = format!(
                "Toggled Key Item #1 possession: {} (Press [Space / E] to test lock vs unlock SE)",
                state.horror_triggers_room.sandbox_has_key
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sh2_core::inventory::InventoryManager;
    use sh2_core::EventFlags;

    #[test]
    fn test_horror_triggers_room_state_timers_and_tables() {
        let mut st = HorrorTriggersRoomState::default();
        assert_eq!(st.active_sounds_array(), [0, 0, 0, 0]);

        st.active_se_timers.push((0x9C70, 1.5));
        assert_eq!(st.active_sounds_array(), [0x9C70, 0, 0, 0]);

        st.tick_se_timers(1.0);
        assert_eq!(st.active_sounds_array(), [0x9C70, 0, 0, 0]);

        st.tick_se_timers(0.6);
        assert_eq!(st.active_sounds_array(), [0, 0, 0, 0]);

        // Verify scenario 0 (Prison stall door inspect) and scenario 7 (EventEngine sandbox) tables
        let (ev0, pos0) = build_scenario_event_tables(0, 0, false);
        assert_eq!(ev0.len(), 2);
        assert_eq!(pos0.len(), 8);

        let mut flags = EventFlags::new();
        let mut inv = InventoryManager::new();
        let eval_in = EventEvalInput {
            player_pos: [0.0, 0.0, -700.0],
            player_rot_y: std::f32::consts::PI,
            action_pressed: true,
            ..Default::default()
        };
        let _ = st.event_engine.tick_standard(
            &ev0,
            &[],
            &pos0,
            1,
            &eval_in,
            true,
            true,
            true,
            true,
            &mut flags,
            &mut inv,
        );
        assert!(flags.get(648));

        let (ev7, pos7) = build_scenario_event_tables(7, 2, true);
        assert_eq!(ev7.len(), 4);
        assert_eq!(pos7.len(), 36);

        // Verify all horror scenario SE IDs map to `sddata.bin` or `voice.afs` indices
        for &se_id in &[
            0x427Du32, 0x2EF3, 0x9C70, 0x9C51, 0x4682, 0x9C6C, 0x9C49, 0x9C47, 0x3E80, 0x9C73,
            0x4982, 0x427B,
        ] {
            let mapped = sh2_audio::resolve_se_id_to_sddata_index(se_id).is_some()
                || sh2_audio::resolve_se_id_to_voice_afs_index(se_id).is_some();
            assert!(
                mapped,
                "Horror SE 0x{se_id:04X} must resolve in sddata.bin or voice.afs"
            );
        }

        // Verify every scenario's default anchor + initial local pos is within audible 3D range of its emitter
        let preview_emitters: [[f32; 3]; 8] = [
            [-138_700.0, -50.0, -21_000.0],
            [20_600.0, 0.0, -28_950.0],
            [60_000.0, 0.0, 11_350.0],
            [-140_000.0, -500.0, 20_000.0],
            [100_807.1, -500.0, -6_053.485_4],
            [-99_400.0, -500.0, 26_400.0],
            [-11_520.0, 42_670.0, 40_151.7],
            [0.0, 0.0, 2_200.0],
        ];
        for (i, &(_, _, _, anchor, init_local, yaw, _)) in HORROR_SCENARIOS.iter().enumerate() {
            let p_world = glam::Vec3::new(
                anchor[0] + init_local[0],
                anchor[1] + init_local[1],
                anchor[2] + init_local[2],
            );
            let eval = compute_se_call_pos_distance_f(
                p_world,
                yaw,
                glam::Vec3::from_array(preview_emitters[i]),
                1.0,
            );
            assert!(
                eval.effective_gain > 0.20,
                "Scenario {} initial distance {:.0}u produced silent gain {:.3}",
                i + 1,
                eval.distance,
                eval.effective_gain
            );
        }
    }
}
