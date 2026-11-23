//! Voice-stream buffer synchronization, `0.9999` frame-boundary snap, and PS2 Q12 letterbox constants
//! (`DramaDemoMain` `0x001b0f00` / `DdsPlay` `0x001b1980` / `DramaDemoFade` `0x001b3390`).

use serde::{Deserialize, Serialize};

use super::{
    DEMO_FRAME_SNAP_EPSILON, PS2_LETTERBOX_BAR_HEIGHT_FRACTION, PS2_LETTERBOX_BOTTOM_Y0_Q12,
    PS2_LETTERBOX_Q12_MAX, PS2_LETTERBOX_Q12_MIN, PS2_LETTERBOX_TOP_Y1_Q12,
};

/// Snaps `frame` to `floor(frame) + 1.0` when `frame - floor(frame) > 0.9999`, matching
/// `DdsPlay` (`SLUS_202.28` `0x001b1980`, `sh2pc.exe` `0x004a13c0`).
#[inline]
pub fn snap_demo_frame_boundary(frame: f32) -> f32 {
    let whole = (frame as i32) as f32;
    if frame - whole > DEMO_FRAME_SNAP_EPSILON {
        whole + 1.0
    } else {
        frame
    }
}

/// Hardware voice stream readiness state polled by `DramaDemoMain` (`shSdStat() & 0xf0`, `0x001b0f00` / `0x004a2110`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum DemoVoiceBufferState {
    /// Voice stream has not yet been requested or is idle.
    Idle,
    /// Voice stream is still buffering; `DramaDemoMain` calls `shSetDFZero()` to freeze cutscene delta-time.
    Buffering,
    /// Voice stream buffer is ready (`0x40` on PS2, `0x10` on PC); `DramaDemoMain` starts playback and calls `shResetDF()`.
    #[default]
    Ready,
    /// Voice stream is actively playing.
    Playing,
}

/// Frame-delta synchronization hook returned by `DramaDemoPlayer::tick_with_voice_buffer_sync`,
/// mirroring `shSetDFZero()` / `shResetDF()` in `DramaDemoMain` (`SLUS_202.28` `0x001b0f00`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum DemoDeltaFrameHook {
    /// Advance frame delta normally.
    #[default]
    Normal,
    /// Reset accumulated delta-frame counter (`shResetDF`, e.g. on stream preload or buffer-ready release).
    ResetDeltaFrame,
    /// Freeze frame delta to zero (`shSetDFZero`) while waiting for the voice stream buffer to fill.
    FreezeDeltaFrame,
}

/// Authentic PS2 `DramaDemoFade` (`0x001b3390`) top and bottom letterbox bar geometry in Q12
/// signed screen space (`[-4096, +4096]`) and normalized `[0.0, 1.0]` screen coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DramaDemoLetterboxRects {
    /// Top bar `(x0, y0, x1, y1)` in PS2 Q12 coordinates (`[-4096, -4096, 4096, -3072]`).
    pub top_q12: [i16; 4],
    /// Bottom bar `(x0, y0, x1, y1)` in PS2 Q12 coordinates (`[-4096, 3072, 4096, 4096]`).
    pub bottom_q12: [i16; 4],
    /// Normalized vertical height fraction of each bar (`0.125`, leaving `0.75` active vertical band).
    pub bar_height_fraction: f32,
}

impl DramaDemoLetterboxRects {
    /// Returns the authentic PS2 `DramaDemoFade` (`0x001b3390`) Q12 letterbox bars.
    pub const fn ps2_authentic() -> Self {
        Self {
            top_q12: [
                PS2_LETTERBOX_Q12_MIN,
                PS2_LETTERBOX_Q12_MIN,
                PS2_LETTERBOX_Q12_MAX,
                PS2_LETTERBOX_TOP_Y1_Q12,
            ],
            bottom_q12: [
                PS2_LETTERBOX_Q12_MIN,
                PS2_LETTERBOX_BOTTOM_Y0_Q12,
                PS2_LETTERBOX_Q12_MAX,
                PS2_LETTERBOX_Q12_MAX,
            ],
            bar_height_fraction: PS2_LETTERBOX_BAR_HEIGHT_FRACTION,
        }
    }
}
