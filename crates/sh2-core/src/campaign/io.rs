//! Per-frame input & side-effect command types exchanged between `sh2-core::campaign` and the
//! thin `sh2-app` runtime shell (`AGENTS.md` §1).

use crate::events::engine::EventPositionalSound;
use crate::world::{ActiveRoomPickup, TileStreamingPlan};

use super::cutscene_handoff::{DramaDemoTrigger, FmvTrigger};
use super::stage_loader::RoomLoadManifest;
use super::state::OverlayMenuKind;

/// 3D / managed positional stage ambient or horror sound effect (`SeCallPos` `0x00192270` /
/// `sh2pc.exe` `0x00516e90`, `SePosMove` `0x00515910`, `SePosSet` `0x00515740`).
///
/// Uses full `u32` `se_id` so 3D stage cues `>= 0x8000` (e.g., `0x9c86`, `0x9c87`, `0x9c88`,
/// `0x9c70`, `0x9c73`) never truncate or sign-extend through `i16`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StagePositionalSound {
    /// Retail sound effect ID (`u32`, e.g., `0x9c88`, `0x4268`, `0x3e80`, `0x4682`).
    pub se_id: u32,
    /// World-space emitter position `[x, y, z]` in PS2 mm.
    pub pos: [f32; 3],
    /// Linear base volume in `[0.0, 1.0]`.
    pub volume: f32,
    /// PS2 `SeCallPos` flags (`1` = `SE_FLAG_PRIO_1`, `4` = `SE_FLAG_NO_DISTANCE`, `8` = `SE_FLAG_MANAGED_2D`).
    pub flags: u32,
}

/// Per-frame gameplay & UI inputs supplied by the thin `sh2-app` shell to
/// [`CampaignOrchestrator::step`](super::CampaignOrchestrator::step).
#[derive(Debug, Clone, Default)]
pub struct CampaignTickInput {
    /// Action/interact button newly pressed this frame (`EventCheck` button triggers).
    pub action_pressed: bool,
    /// Cancel/skip button newly pressed this frame (skips FMV/DramaDemo or dismisses message).
    pub cancel_pressed: bool,
    /// Directional keys newly pressed this frame (`PROG_KEY_UP | PROG_KEY_DOWN | PROG_KEY_LEFT | PROG_KEY_RIGHT`)
    /// forwarded to interactive `StageProgramRunner` puzzles (e.g. Room 208 Grandfather Clock).
    pub prog_dir_down: u32,
    /// Directional keys currently held this frame (`PROG_KEY_UP | PROG_KEY_DOWN | PROG_KEY_LEFT | PROG_KEY_RIGHT`)
    /// forwarded to interactive `StageProgramRunner` puzzles (e.g. `step_clock_needles`).
    pub prog_dir_held: u32,
    /// Active dialog message dismissed by player.
    pub message_dismissed: bool,
    /// Player requested toggling an in-game overlay menu (`Map`, `Inventory`, `Options`, `Memo`, `Pause`).
    pub overlay_toggle: Option<OverlayMenuKind>,
    /// Set to `true` when the active `.dds` `DramaDemoPlayer` finished playing this frame.
    pub drama_demo_finished: bool,
    /// Current playback frame of the active `.dds` `DramaDemoPlayer` (`0x01f7a7b8`), used for
    /// mid-cutscene stage hooks such as `tenbou`'s `1400.0` BGM switch (`sh2pc.exe` `0x005a0936`).
    pub drama_demo_frame: Option<f32>,
    /// Set to `true` when the active `.bik` FMV finished playing this frame.
    pub fmv_finished: bool,
    /// Set to `true` when the player confirmed exit from `GameOver` or `EndingCredits`.
    pub confirm_terminal_exit: bool,
    /// Optional item ID actively used from inventory this frame (`used_item_id`).
    pub item_use_id: u8,
    /// Prefer SH2:EE `.cam`/asset overrides (`true` by default).
    pub prefer_sh2ee: bool,
    /// Enable SH2:EE world & event fixes (`FlashlightClockPush`, `ElevatorCursorColorFix`,
    /// `HospitalChase`, `CreatureVehicleSpawn`). `false` in strict 1:1 Vanilla PS2 mode.
    pub enable_sh2ee_world_fixes: bool,
    /// Optional distance in PS2 mm between James and Maria when Maria is accompanying
    /// (`flag 15`, `sh2pc.exe` `0x0048ed1e..0x0048ed8c` `EndingPointTracker::tick_maria_escort`).
    pub maria_distance_mm: Option<f32>,
    /// `true` when James bumped/shoved Maria on this tick (`0x0048ed72`).
    pub maria_bumped: bool,
    /// Currently playing stage ambient / horror SE IDs reported by `sh2-app` for `SeCheck`
    /// (`sh2pc.exe` `0x00515890`) queries in `ForestAmbientState` and `StageEventDispatcher`.
    pub active_stage_se_ids: Vec<u32>,
}

/// Pure side-effect commands emitted by [`CampaignOrchestrator::step`](super::CampaignOrchestrator::step)
/// for the thin `sh2-app` shell to execute on GPU, audio, video, and UI subsystems.
#[derive(Debug, Clone, Default)]
pub struct CampaignEffects {
    /// When `Some`, the shell must load the room's `.map`, `.cld`, `.cam`, textures, and BGM bank,
    /// then call [`CampaignOrchestrator::notify_room_assets_loaded`](super::CampaignOrchestrator::notify_room_assets_loaded).
    pub load_room: Option<RoomLoadManifest>,
    /// When `Some`, the shell should launch a `.dds` real-time 3D cutscene session.
    pub start_drama_demo: Option<DramaDemoTrigger>,
    /// When `Some`, the shell should launch a `.bik` FMV playback session.
    pub start_fmv: Option<FmvTrigger>,
    /// When `Some(label)`, activate the PS2 footstep loading screen (`NowLoadingEnable` `0x00298fa0`).
    pub show_loading_screen: Option<String>,
    /// `true` when the PS2 footstep loading screen should be dismissed.
    pub hide_loading_screen: bool,
    /// Current screen black overlay alpha in `[0.0, 1.0]` (`0.0` = clear, `1.0` = black).
    pub fade_alpha: f32,
    /// Non-positional 2D sound effects to trigger (`shSdCall` / `SeCall`).
    pub sound_effects: Vec<u32>,
    /// Looping 2D ambient sound effects to start idempotently `(se_id, initial_volume)`
    /// (`EvSoundCallAfterLoad` `SeCallPos` with `pos = NULL`, `flags & 8 != 0`).
    pub looping_se_starts: Vec<(u32, f32)>,
    /// Live volume modulations `(se_id, volume)` for active looping 2D ambient SEs (`SeVolume` `0x00516d50`).
    pub looping_se_volumes: Vec<(u32, f32)>,
    /// Sound effect IDs to stop (`SeStop` `0x00193a80` / `0x00515f10`, e.g. `0x4268`, `0x9c86`).
    pub stop_sound_effects: Vec<u32>,
    /// 3D positional sound effects emitted by `EventEngine` (`SeCallPos`).
    pub positional_sounds: Vec<EventPositionalSound>,
    /// 3D / managed positional stage ambient & horror SE spawns (`u32` `se_id`).
    pub stage_positional_sounds: Vec<StagePositionalSound>,
    /// Live 3D / managed positional SE world-space position & volume updates (`SePosMove` / `SePosSet`).
    pub updated_positional_sounds: Vec<StagePositionalSound>,
    /// When `true`, fade out all active stage looping 2D and 3D ambient SEs (e.g. stage/room exit).
    pub stop_all_stage_ambient_se: bool,
    /// Stage ID for BGM manager synchronization whenever a stage load completes.
    pub bgm_stage_id: Option<u32>,
    /// Active `EvBgmControl` (`Stage_Data::bgm_control`) channel bitmask for the current stage
    /// and demo state (e.g. `stg_ovservation` `0x005a1390`: `0x04` -> `0x0e` during `tenbou`,
    /// `0`/`1` in gameplay).
    pub bgm_control_mask: Option<u32>,
    /// Outdoor `.map` multi-tile streaming plan (`LoadBgSync` `0x0020b240`) when outdoors.
    pub outdoor_plan: Option<TileStreamingPlan>,
    /// Common `.mes` message ID emitted by `EventMainStandard`.
    pub common_message: Option<u16>,
    /// Stage `.mes` message ID emitted by `EventMainStandard`.
    pub stage_message: Option<u16>,
    /// `true` when the active dialog message should be cleared.
    pub clear_message: bool,
    /// `true` when `GameOver` or `EndingCredits` completed and the app should return to Title.
    pub return_to_title: bool,
    /// Active 3D `Item_List` (`0x07xx`) room pickups for the current stage/room.
    pub active_room_pickups: Vec<ActiveRoomPickup>,
    /// `true` when gameplay simulation ticks (`SimulationCore::update`) are permitted this frame.
    pub allow_gameplay_sim: bool,
}
