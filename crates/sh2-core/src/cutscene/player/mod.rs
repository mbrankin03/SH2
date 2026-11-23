//! Runtime cutscene state machine (`DramaDemoPlayer`, `CutsceneCameraSnapshot`, `CutsceneActorRuntime`) (`SH2-63`).

use serde::{Deserialize, Serialize};
use sh2_assets::formats::demo::{
    CutsceneBorderMode, CutsceneWidescreenMode, DemoDdsEvaluator, DemoKeyframeState,
};

use super::atmosphere::AtmosphereGradingEvaluation;
use super::catalog::DramaDemoPlayInfo;
use super::piston_kinematics::{ClosetDoorReplacementState, PistonArrayRuntime};
use super::special_fx::SpecialFxEvaluation;
use super::stage_fixes::{ClosetCutsceneFixState, PistonRoomFixState};

mod actor;
mod playback;
mod sync;
mod voice_sync;

#[cfg(test)]
mod tests;

pub use actor::CutsceneActorRuntime;
pub use sync::CutsceneCameraSnapshot;
pub use voice_sync::{
    snap_demo_frame_boundary, DemoDeltaFrameHook, DemoVoiceBufferState, DramaDemoLetterboxRects,
};

/// Frame-boundary snap threshold (`0.9999`) in `DdsPlay` (`SLUS_202.28` `0x001b1980`, `sh2pc.exe` `0x004a13c0`).
pub const DEMO_FRAME_SNAP_EPSILON: f32 = 0.9999;

/// PS2 Q12 signed screen coordinate minimum (`-4096` / `0xf000`) in `DramaDemoFade` (`SLUS_202.28` `0x001b3390`).
pub const PS2_LETTERBOX_Q12_MIN: i16 = -4096;
/// PS2 Q12 signed screen coordinate maximum (`+4096` / `0x1000`) in `DramaDemoFade` (`SLUS_202.28` `0x001b3390`).
pub const PS2_LETTERBOX_Q12_MAX: i16 = 4096;
/// PS2 Q12 top letterbox bar bottom edge (`-3072` / `0xf400`) in `DramaDemoFade` (`SLUS_202.28` `0x001b3390`).
pub const PS2_LETTERBOX_TOP_Y1_Q12: i16 = -3072;
/// PS2 Q12 bottom letterbox bar top edge (`+3072` / `0x0c00`) in `DramaDemoFade` (`SLUS_202.28` `0x001b3390`).
pub const PS2_LETTERBOX_BOTTOM_Y0_Q12: i16 = 3072;
/// Normalized vertical fraction (`1024 / 8192 = 0.125`) of each top/bottom letterbox bar (`DramaDemoFade` `0x001b3390`).
pub const PS2_LETTERBOX_BAR_HEIGHT_FRACTION: f32 = 0.125;

/// Lifecycle phase of [`DramaDemoPlayer`] (`demo_step` in `DramaDemoMain` at `0x1b2b30`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DramaDemoStep {
    /// Cutscene is actively playing (`demo_step == 2`).
    Playing,
    /// Cutscene was skipped via `Start`/`Esc` and is fading out (`demo_step == 3`).
    SkippingFadeOut,
    /// Cutscene has completed (`demo_step >= 5`), ready to hand control back to gameplay.
    Finished,
}

/// Stateful in-engine real-time 3D cutscene player (`DramaDemoMain` / `DdsPlay` / `DramaDemoSkipLast`).
#[derive(Debug, Clone)]
pub struct DramaDemoPlayer {
    /// Static metadata descriptor (`DramaDemoPlayInfo`).
    pub info: DramaDemoPlayInfo,
    /// Underlying `.dds` binary keyframe evaluator.
    pub evaluator: DemoDdsEvaluator,
    /// Current lifecycle phase (`Playing`, `SkippingFadeOut`, `Finished`).
    pub step: DramaDemoStep,
    /// Continuous 30 FPS scene frame cursor (`Demo_Frame`).
    pub scene_frame: f32,
    /// Independent 30 FPS subtitle clock (`msg_frame`).
    pub msg_frame: f32,
    /// Whether the voice stream (`info.stream_no`) has been preloaded (`_demo_status & 0x80`, `DramaDemoMain` `0x001b0f00`).
    pub voice_preload_requested: bool,
    /// Whether the voice stream (`info.stream_no`) has been triggered at `scene_frame >= stream_start` (`_demo_status & 0x100`).
    pub voice_started: bool,
    /// Whether the voice stream is waiting to be started by the audio engine on this tick.
    pub pending_voice_trigger: bool,
    /// Whether the voice stream should be stopped immediately (`SeCall(0x3F4)` on skip/finish).
    pub pending_voice_stop: bool,
    /// Cumulative seconds spent stalled waiting for voice buffer readiness (`_stop_counter_896` at `0x001b0f00` / `_DAT_0094e2d8` at `0x004a2110`).
    pub voice_stall_secs: f32,
    /// Most recent `shSetDFZero` / `shResetDF` delta-frame hook emitted by `DramaDemoMain` (`0x001b0f00`).
    pub last_df_hook: DemoDeltaFrameHook,
    /// Active `.mes` message ID (`info.msg_start + window.msg_offset`) if a subtitle window is currently active.
    pub active_subtitle_msg_id: Option<u16>,
    /// Current screen fade alpha (`0.0` = clear, `1.0` = black).
    pub fade_alpha: f32,
    /// Whether Widescreen FOV reduction (`ReduceCutsceneFOV`) is enabled (`widescreen_mode == Sh2eeZoom16x9`).
    pub reduce_widescreen_fov: bool,
    /// Current viewport aspect ratio (`width / height`, e.g. `16.0/9.0`, `64.0/27.0`, `32.0/9.0`) (`SH2-77`).
    pub viewport_aspect: f32,
    /// Current viewport height in pixels (used to compute `display_ratio = height / 1080.0` in `SH2-105`).
    pub viewport_height: f32,
    /// Active widescreen framing and off-screen actor culling mode (`SH2-77`).
    pub widescreen_mode: CutsceneWidescreenMode,
    /// Policy for top/bottom cutscene letterbox bars (`DisableCutsceneBorders` in `SH2-77`).
    pub border_mode: CutsceneBorderMode,
    /// Scene-specific widescreen fix state for Wood Side Room 307 (`sankaku1` / `wp_closet`, `demo_no == 14`).
    pub closet_fix: ClosetCutsceneFixState,
    /// Scene-specific widescreen fix state for Labyrinth Piston Room (`papa_agl` / `ug_angela`, `demo_no == 68`).
    pub piston_fix: PistonRoomFixState,
    /// Scene-specific widescreen fog counter triggers `(counter_one, counter_two)` for Rosewater Park (`deai`, `demo_no == 22`).
    pub maria_fog_counters: (bool, bool),
    /// Resolved PS2 room/cutscene atmospheric grading (`TimeOfDay = 15/11`, Room 312 sunset, Leave/Maria tree lighting) (`SH2-105`).
    pub atmosphere: AtmosphereGradingEvaluation,
    /// Resolved PS2 screen-space DoF (`Kari_hisyakai`) and GS motion blur (`enSetBlur`) state (`SH2-105`).
    pub special_fx: SpecialFxEvaluation,
    /// Stateful 24-piston array kinematics for the Abstract Daddy boss room (`SH2-105`).
    pub piston_array: PistonArrayRuntime,
    /// Wood Side Room 307 `b_doo` closet door replacement transform and bonus audio trigger (`SH2-105`).
    pub closet_door: ClosetDoorReplacementState,
    /// Evaluated camera snapshot for the current frame.
    pub camera: CutsceneCameraSnapshot,
    /// Evaluated keyframe state for the current frame.
    pub current_state: DemoKeyframeState,
    /// Per-actor runtime states (`0..header.n_chars`).
    pub actors: Vec<CutsceneActorRuntime>,
}
