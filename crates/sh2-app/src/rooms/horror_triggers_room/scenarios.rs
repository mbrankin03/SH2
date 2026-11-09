//! Strongly typed scenario catalog ([`HorrorScenarioSpec`]), runtime state ([`HorrorTriggersRoomState`]),
//! and synthetic `Event_List` / `ev_pos` table builders for `SH2-47`.

use crate::state::AppState;
use sh2_core::types::{
    EventListEntry, EventResultKind, EventShapeKind, EventTriggerKind, DOOR_SE_TABLE,
};
use sh2_core::{EventEngine, Sh2Rng, StageEventDispatcher, StageId};

/// Strongly typed specification for an interactive `SH2-47` horror trigger scenario.
#[derive(Debug, Clone, Copy)]
pub(crate) struct HorrorScenarioSpec {
    pub title: &'static str,
    pub stage_id: Option<StageId>,
    pub room_id: u8,
    pub stage_anchor: [f32; 3],
    pub initial_local_pos: [f32; 3],
    pub initial_yaw: f32,
    pub default_emitter_pos: [f32; 3],
    pub controls_hint: &'static str,
}

pub(crate) const HORROR_SCENARIOS: &[HorrorScenarioSpec] = &[
    HorrorScenarioSpec {
        title: "1. Toluca Prison North: Bathroom Stall Knock & 60% Exit Jumpscare (Room 0x73)",
        stage_id: Some(StageId::PrisonN),
        room_id: 0x73,
        stage_anchor: [-138_700.0, 0.0, -20_000.0],
        initial_local_pos: [0.0, 0.0, -850.0],
        initial_yaw: std::f32::consts::PI,
        default_emitter_pos: [-138_700.0, -50.0, -21_000.0],
        controls_hint: "[E] Knock Stall (0x2EF3 + Flag 648) | [Space] Trigger Jumpscare (0x427D) | [K] Test 60%/40% RNG Walk-Out | [R] Re-seed",
    },
    HorrorScenarioSpec {
        title: "2. Blue Creek Apt 2F: Room 209 Orbiting Whisper (Room 0x28, r=2500u @ PI/8 rad/s)",
        stage_id: Some(StageId::ApartW2f),
        room_id: 0x28,
        stage_anchor: [20_600.0, 0.0, -28_950.0],
        initial_local_pos: [950.0, 0.0, 0.0],
        initial_yaw: -std::f32::consts::FRAC_PI_2,
        default_emitter_pos: [20_600.0, 0.0, -28_950.0],
        controls_hint: "[Space / Walk West] Cross X < s_thr -> Starts Whisper 0x9C70 (Flag 547) Orbiting Player at r=2500 | [R] Reset & Re-seed",
    },
    HorrorScenarioSpec {
        title: "3. Wood Side Apt 3F: Moving Corridor Footsteps (Room 0x1F, -700 u/s along -X)",
        stage_id: Some(StageId::ApartE3fe),
        room_id: 0x1F,
        stage_anchor: [60_000.0, 0.0, 12_500.0],
        initial_local_pos: [0.0, 0.0, 0.0],
        initial_yaw: std::f32::consts::PI,
        default_emitter_pos: [60_000.0, 0.0, 11_350.0],
        controls_hint: "[Space / Walk South] Trigger Footsteps 0x9C51 (Flag 552) Translating at -700 u/s | [R] Reset",
    },
    HorrorScenarioSpec {
        title: "4. Toluca Prison South: Invisible Prisoner 'Ritual...' (Room 0x72, Randomized 3D Cell Block)",
        stage_id: Some(StageId::PrisonS),
        room_id: 0x72,
        stage_anchor: [-140_000.0, 0.0, 20_000.0],
        initial_local_pos: [0.0, 0.0, 1200.0],
        initial_yaw: 0.0,
        default_emitter_pos: [-140_000.0, -500.0, 20_000.0],
        controls_hint: "[Space] Force Immediate 'Ritual...' (0x4682 / voice.afs[27]) at Random Cell Pos | [K] Start Oppressor Cell Timer + SFX",
    },
    HorrorScenarioSpec {
        title: "5. Blue Creek 2F Corridor: Two-Stage Hysteresis Scare (Room 0x26, Arm Z>-6500 -> Fire Z<-6800)",
        stage_id: Some(StageId::ApartW2f),
        room_id: 0x26,
        stage_anchor: [100_800.0, 0.0, -6_650.0],
        initial_local_pos: [0.0, 0.0, -500.0],
        initial_yaw: 0.0,
        default_emitter_pos: [100_807.1, -500.0, -6_053.485_4],
        controls_hint: "[Space] Step 1: Arm at Z > -6500, Step 2: Back Below Z < -6800 -> Fires 0x9C6C (Flag 553) | [R] Reset Corridor",
    },
    HorrorScenarioSpec {
        title: "6. Wood Side Apt 2F: Distant Scream (EvProgAnyoneCry), Room 206 Kick & Room 205 TV Static",
        stage_id: Some(StageId::ApartE2f),
        room_id: 0x18,
        stage_anchor: [-99_400.0, 0.0, 25_200.0],
        initial_local_pos: [0.0, 0.0, 0.0],
        initial_yaw: 0.0,
        default_emitter_pos: [-99_400.0, -500.0, 26_400.0],
        controls_hint: "[Space] Run EvProgAnyoneCry (0x9C49 Scream + 2.0s Msg) | [K] Room 206 Kick (0x9C47) | [T] Room 205 TV Static (0x3E80)",
    },
    HorrorScenarioSpec {
        title: "7. Historical Society: Long Descending Staircase (Room 0x5D, 0x9C73) & Room 0x5C (0x4982)",
        stage_id: Some(StageId::Society),
        room_id: 0x5D,
        stage_anchor: [-11_520.0, 41_500.0, 40_150.0],
        initial_local_pos: [0.0, 0.0, 0.0],
        initial_yaw: 0.0,
        default_emitter_pos: [-11_520.0, 42_670.0, 40_151.7],
        controls_hint: "[Space] Trigger Staircase Ambient (0x9C73) | [K] Trigger Room 0x5C Horror Cue (0x4982 / voice.afs[42], Flag 551)",
    },
    HorrorScenarioSpec {
        title: "8. Core EventEngine Sandbox: ev_pos Shapes (Line=2, Point=0, Quad=13) & 22-Entry DOOR_SE_TABLE",
        stage_id: None,
        room_id: 0x01,
        stage_anchor: [0.0, 0.0, 0.0],
        initial_local_pos: [0.0, 0.0, 1850.0],
        initial_yaw: 0.0,
        default_emitter_pos: [0.0, 0.0, 2_200.0],
        controls_hint: "[E / Space] Action on North Door (Line=2) or West Point (Point=0) | [K] Toggle Key Item #1 | [T] Cycle Door Sound (0..21)",
    },
];

/// Interactive runtime state for [`sh2_ui::menu::TestRoomKind::HorrorTriggersRoom`] (`SH2-47`).
pub(crate) struct HorrorTriggersRoomState {
    pub scenario_idx: usize,
    pub dispatcher: StageEventDispatcher,
    pub event_engine: EventEngine,
    pub rng: Sh2Rng,
    pub seed_counter: u32,
    /// Optional dynamic stage anchor override so multi-room scenarios keep the listener within
    /// `SeCallPosDistanceF` audible range (`< 11,500u`) of the active emitter.
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
pub(crate) fn active_scenario_anchor(state: &AppState) -> [f32; 3] {
    let idx = state.horror_triggers_room.scenario_idx % HORROR_SCENARIOS.len();
    state
        .horror_triggers_room
        .stage_anchor_override
        .unwrap_or(HORROR_SCENARIOS[idx].stage_anchor)
}

/// Initializes or resets the currently selected horror scenario (`0..8`), snapping James to the
/// calibrated starting pose and invoking `StageEventDispatcher::on_stage_init` + `on_room_init`.
pub(crate) fn init_horror_scenario(state: &mut AppState, reseed: bool) {
    let idx = state.horror_triggers_room.scenario_idx % HORROR_SCENARIOS.len();
    let spec = HORROR_SCENARIOS[idx];

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

    let local_pos = glam::Vec3::from_array(spec.initial_local_pos);
    state
        .sim_core
        .snap_player_transform(local_pos, spec.initial_yaw);
    state
        .sim_core
        .camera
        .reset_to_player(local_pos, spec.initial_yaw);

    if let Some(stage_id) = spec.stage_id {
        state
            .horror_triggers_room
            .dispatcher
            .on_stage_init(stage_id, &mut state.sim_core.scenario.flags);
        state.horror_triggers_room.dispatcher.on_room_init(
            stage_id,
            spec.room_id,
            &mut state.sim_core.scenario.flags,
            &mut state.horror_triggers_room.rng,
        );
    }

    state.horror_triggers_room.active_emitter_world_pos = if idx == 3 {
        state.horror_triggers_room.dispatcher.prison_s_prisoner_pos
    } else {
        spec.default_emitter_pos
    };

    state.horror_triggers_room.initialized = true;
    state.horror_triggers_room.last_event_log = format!(
        "Initialized {} | Stage={:?} Room=0x{:02X}",
        spec.title, spec.stage_id, spec.room_id
    );
}

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
pub(crate) fn build_scenario_event_tables(
    scenario_idx: usize,
    door_se_idx: usize,
    sandbox_has_key: bool,
) -> (Vec<EventListEntry>, Vec<u8>) {
    let mut ev_list = Vec::new();
    let mut ev_pos = Vec::new();

    if scenario_idx == 0 {
        push_i16(&mut ev_pos, 0);
        push_i16(&mut ev_pos, -200);
        push_i16(&mut ev_pos, -1000);
        push_i16(&mut ev_pos, 650);

        ev_list.push(make_event_entry(
            648,
            true,
            0,
            false,
            EventTriggerKind::ActionLook,
            0,
            EventShapeKind::Point,
            EventResultKind::FlagSet,
            0,
            0,
            648,
        ));
        ev_list.push(EventListEntry {
            flag: 0x3fff,
            ..Default::default()
        });
        return (ev_list, ev_pos);
    }

    if scenario_idx == 7 {
        push_i16(&mut ev_pos, -800);
        push_i16(&mut ev_pos, -200);
        push_i16(&mut ev_pos, 2200);
        push_i16(&mut ev_pos, 650);
        push_i16(&mut ev_pos, 1600);

        push_i16(&mut ev_pos, -2000);
        push_i16(&mut ev_pos, -200);
        push_i16(&mut ev_pos, 0);
        push_i16(&mut ev_pos, 850);

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
            0xfe,
            if sandbox_has_key { 701 } else { 0 },
        ));
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

#[cfg(test)]
mod tests {
    use super::*;
    use sh2_audio::compute_se_call_pos_distance_f;
    use sh2_core::inventory::InventoryManager;
    use sh2_core::{EventEvalInput, EventFlags};

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

        for (i, spec) in HORROR_SCENARIOS.iter().enumerate() {
            let p_world = glam::Vec3::new(
                spec.stage_anchor[0] + spec.initial_local_pos[0],
                spec.stage_anchor[1] + spec.initial_local_pos[1],
                spec.stage_anchor[2] + spec.initial_local_pos[2],
            );
            let eval = compute_se_call_pos_distance_f(
                p_world,
                spec.initial_yaw,
                glam::Vec3::from_array(spec.default_emitter_pos),
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
