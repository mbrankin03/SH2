//! Unified Start-to-Credits Campaign Game Loop (`gamemain.c` / `PlayableMain` `0x0020b3c0` /
//! `connectMain` `0x0020bf80`, `SH2-112`).
//!
//! Decomposed into cohesive, zero-GPU-dependency submodules (`AGENTS.md` §1–§2):
//! - [`state`]: Typed campaign lifecycle states (`CampaignPhase`, `ConnectStep`, `ScreenFadeState`).
//! - [`stage_loader`]: Unified 58-stage asset & entity manifest resolver (`RoomLoadManifest`).
//! - [`cutscene_handoff`]: Automatic `.dds`/`.bik` trigger evaluation and zero-smear camera handoff.
//! - [`orchestrator`]: Deterministic [`CampaignOrchestrator`] driving `SimulationCore`, `EventEngine`,
//!   and `OutdoorTileStreamer` while emitting typed [`CampaignEffects`] for `sh2-app`.

pub(crate) mod connect;
pub mod cutscene_handoff;
pub(crate) mod gameplay;
pub mod indoor_rooms;
pub mod io;
pub mod orchestrator;
pub mod retail_events;
pub mod save;
pub mod stage_loader;
pub mod state;

pub use cutscene_handoff::{
    apply_post_cutscene_handoff, evaluate_auto_stage_demo, AutoDemoRule, DramaDemoTrigger,
    FmvTrigger, AUTO_STAGE_DEMO_RULES,
};
pub use indoor_rooms::resolve_indoor_room_id;
pub use io::{CampaignEffects, CampaignTickInput};
pub use orchestrator::CampaignOrchestrator;
pub use save::CampaignSaveSnapshot;
pub use stage_loader::{
    resolve_outdoor_room_id, resolve_room_id_for_stage_pos, resolve_stage_manifest, room_area_dir,
    CampaignEnemySpawn, CampaignPickupSpawn, RoomLoadManifest,
};
pub use state::{
    CampaignPhase, ConnectStep, FadeDirection, OverlayMenuKind, RoomLoadTarget, ScreenFadeState,
    MAIN_SCENARIO_START_STAGE, PS2_DEFAULT_FADE_SPEED, SE_GAME_OVER, SH2EE_DELAYED_FADE_IN_START,
    WISH_SCENARIO_START_STAGE,
};
