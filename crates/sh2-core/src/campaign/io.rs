//! Per-frame input & side-effect command types exchanged between `sh2-core::campaign` and the
//! thin `sh2-app` runtime shell (`AGENTS.md` §1).

use crate::events::engine::EventPositionalSound;
use crate::world::{ActiveRoomPickup, TileStreamingPlan};

use super::cutscene_handoff::{DramaDemoTrigger, FmvTrigger};
use super::stage_loader::RoomLoadManifest;
use super::state::OverlayMenuKind;

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
    /// Sound effect IDs to stop (`SeStop`, e.g. NG+ chainsaw idle loop `0x4268`).
    pub stop_sound_effects: Vec<u32>,
    /// 3D positional sound effects to trigger (`SeCallPos`).
    pub positional_sounds: Vec<EventPositionalSound>,
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
