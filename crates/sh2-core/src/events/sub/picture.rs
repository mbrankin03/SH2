//! Event Cutaway Picture & Cursor Coroutines (`SLUS_202.28` `Event/event_sub.c`
//! `0x001ac8b0..0x001ad557`, and `sh2pc.exe` `0x004a2760..0x004a2f02`).
//!
//! Emits [`PictureDrawCommand`] and [`PictureLoadImageCommand`] descriptors built via
//! [`super::picture_draw`].

use crate::events::engine::{EventEngineEffects, EventStepState};

use super::picture_draw::{
    picture_draw, picture_load_image, PictureAlphaBlend, PictureDrawCommand, PictureDrawParams,
    PictureFlags, PictureLoadImageCommand, PictureUv,
};

/// Background dimming filter ramp speed in units/second (`4.0` in `EvSubPictureEnd`
/// `SLUS_202.28` `0x001ace70` and `sh2pc.exe` `0x004a2880`).
pub const EV_FILTER_RATE_PER_SEC: f32 = 4.0;

/// Maximum GS `ALPHA_1.FIX` value for the background dimming quad (`64.0` in `EvSubPictureEnd`
/// `0x001ace70`, corresponding to 50% darkening since PS2 GS alpha `0x80` = 1.0).
pub const EV_FILTER_ALPHA_SCALE: f32 = 64.0;

/// Analog stick lower deadzone boundary (`< 0x65`, i.e., `<= 100` in `EvSubPictureCursor`
/// `SLUS_202.28` `0x001ad010` and `sh2pc.exe` `0x004a2a50`).
pub const CURSOR_DEADZONE_LOW: u8 = 0x65;

/// Analog stick upper deadzone boundary (`>= 0x9b`, i.e., `>= 155` in `EvSubPictureCursor`
/// `SLUS_202.28` `0x001ad010` and `sh2pc.exe` `0x004a2a50`).
pub const CURSOR_DEADZONE_HIGH: u8 = 0x9b;

/// Horizontal cursor speed in virtual pixels/second (`153.6` in `EvSubPictureCursor`
/// `SLUS_202.28` `0x001ad010` and `sh2pc.exe` `0x004a2a50`).
pub const CURSOR_SPEED_X_PX_PER_SEC: f32 = 153.6;

/// Vertical cursor speed in virtual pixels/second (`192.0` in `EvSubPictureCursor`
/// `SLUS_202.28` `0x001ad010` and `sh2pc.exe` `0x004a2a50`).
pub const CURSOR_SPEED_Y_PX_PER_SEC: f32 = 192.0;

/// Screen-space clamping bounds for [`ev_sub_picture_cursor`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CursorBounds {
    /// Maximum absolute X coordinate in virtual pixels (`256.0` on both PS2 and PC).
    pub half_width: f32,
    /// Maximum absolute Y coordinate in virtual pixels (`192.0` on PS2 `SLUS_202.28` `0x001ad010`,
    /// `240.0` on retail PC `sh2pc.exe` `0x004a2a50`).
    pub half_height: f32,
}

impl CursorBounds {
    /// PS2 prototype bounds (`±256.0 x ±192.0`, `SLUS_202.28` `0x001ad010`).
    pub const PS2: Self = Self {
        half_width: 256.0,
        half_height: 192.0,
    };
    /// Retail PC bounds (`±256.0 x ±240.0`, `sh2pc.exe` `0x004a2a50`).
    pub const PC: Self = Self {
        half_width: 256.0,
        half_height: 240.0,
    };
}

/// Cursor tint mode (`param_1` in `EvSubPictureCursor` `SLUS_202.28` `0x001ad010` and
/// `sh2pc.exe` `0x004a2a50`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum CursorTintMode {
    /// `0`: Neutral tint (`0x80, 0x80, 0x80`, flags `0x27`).
    #[default]
    Neutral = 0,
    /// `1`: Blue tint (`0x60, 0x60, 0x80`, flags `0x37`).
    Blue = 1,
    /// `2`: Red tint (`0x80, 0x60, 0x60`, flags `0x37`).
    Red = 2,
    /// `4`: Green tint (`0x60, 0x80, 0x60`, flags `0x37`).
    Green = 4,
}

/// Controller input for one frame of [`ev_sub_picture_cursor`] (`0x001ad010`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CursorInput {
    /// Left analog stick X byte (`0..=255`, center `128`).
    pub stick_x: u8,
    /// Left analog stick Y byte (`0..=255`, center `128`).
    pub stick_y: u8,
    /// D-pad right held (`0x100`).
    pub dpad_right: bool,
    /// D-pad left held (`0x200`).
    pub dpad_left: bool,
    /// D-pad down held (`0x800`).
    pub dpad_down: bool,
    /// D-pad up held (`0x400`).
    pub dpad_up: bool,
}

impl Default for CursorInput {
    fn default() -> Self {
        Self {
            stick_x: 0x80,
            stick_y: 0x80,
            dpad_right: false,
            dpad_left: false,
            dpad_down: false,
            dpad_up: false,
        }
    }
}

/// Decodes a raw `0..=255` analog stick byte using the exact `EvSubPictureCursor` deadzone
/// (`SLUS_202.28` `0x001ad010` lines 651-661 and `sh2pc.exe` `0x004a2a50` lines 1870-1880).
pub fn decode_cursor_axis(raw: u8) -> f32 {
    if raw < CURSOR_DEADZONE_LOW {
        (raw as i32 - 100) as f32 / 100.0
    } else if raw >= CURSOR_DEADZONE_HIGH {
        (raw as i32 - 0x9b) as f32 / 100.0
    } else {
        0.0
    }
}

/// Persistent state for `EvSubPicture*` coroutines (`ev_filter_on` `0x011268d0`,
/// `ev_filter` `0x01126398`, `ev_cursor_x` `0x01126390`, `ev_cursor_y` `0x01126388`).
#[derive(Debug, Clone, Default)]
pub struct EvSubPictureState {
    /// `ev_filter_on` (`0x011268d0`): set each frame by [`ev_sub_picture_filter`], cleared by [`ev_sub_picture_end`].
    pub filter_on: bool,
    /// `ev_filter` (`0x01126398`): dimming progress in `0.0..=1.0`, ramped at `4.0/s`.
    pub filter_alpha: f32,
    /// `ev_cursor_x` (`0x01126390`): cursor X in virtual screen space (`-256.0..=256.0`).
    pub cursor_x: f32,
    /// `ev_cursor_y` (`0x01126388`): cursor Y in virtual screen space (`-192..192` PS2 / `-240..240` PC).
    pub cursor_y: f32,
    /// `true` if `EvSubPictureStart` requested a black clear + 2D overlay mode flag (`_DAT_010f08a4 |= 1`).
    pub frame_started: bool,
    /// Picture file ID queued by `EvSubPictureDisplay` step `0`, if any.
    pub queued_file_id: Option<u32>,
    /// Texture upload commands emitted this frame (`PictureLoadImage`).
    pub load_commands: Vec<PictureLoadImageCommand>,
    /// Draw commands emitted this frame (`PictureDraw`).
    pub draw_commands: Vec<PictureDrawCommand>,
}

impl EvSubPictureState {
    /// Clears per-frame command queues at the start of a new frame.
    pub fn clear_frame_commands(&mut self) {
        self.frame_started = false;
        self.queued_file_id = None;
        self.load_commands.clear();
        self.draw_commands.clear();
    }
}

/// `EvSubPictureInit` (`SLUS_202.28` `0x001ace10`, `sh2pc.exe` `0x004a2840`).
pub fn ev_sub_picture_init(pic: &mut EvSubPictureState) {
    pic.filter_on = false;
    pic.filter_alpha = 0.0;
}

/// `EvSubPictureFilter` (`SLUS_202.28` `0x001acdf0`, `sh2pc.exe` `0x004a2830`).
pub fn ev_sub_picture_filter(pic: &mut EvSubPictureState) {
    pic.filter_on = true;
}

/// `EvSubPictureStart` (`SLUS_202.28` `0x001ace30`, `sh2pc.exe` `0x004a2870`).
pub fn ev_sub_picture_start(pic: &mut EvSubPictureState) {
    pic.frame_started = true;
}

/// `EvSubPictureDisplayOnly` (`SLUS_202.28` `0x001ac910`, `sh2pc.exe` `0x004a2760`).
///
/// Uploads main picture slot `0` and emits a full-screen textured [`PictureDrawCommand`] at
/// priority `1` (`flags = 0x01`).
pub fn ev_sub_picture_display_only(pic: &mut EvSubPictureState) {
    pic.load_commands.push(picture_load_image(0, 0, -1, -1, 0));
    pic.draw_commands.push(picture_draw(PictureDrawParams {
        tex_slot: 0,
        priority: 1,
        flags: PictureFlags(PictureFlags::TEXTURED),
        ..PictureDrawParams::default()
    }));
}

/// `EvSubPictureLayer` (`SLUS_202.28` `0x001acce0`, `sh2pc.exe` `0x004a27a0`).
///
/// Uploads layer texture slot `2` and emits an alpha-blended textured [`PictureDrawCommand`] at
/// priority `3` (`flags = 0x23`, `ALPHA_1 = (0, 1, 0, 1, 0x80)`).
pub fn ev_sub_picture_layer(rect_q4: [i16; 4], alpha: u8, pic: &mut EvSubPictureState) {
    pic.load_commands.push(picture_load_image(2, 2, -1, -1, 0));
    pic.draw_commands.push(picture_draw(PictureDrawParams {
        tex_slot: 2,
        rect_q4,
        rgba: [0x80, 0x80, 0x80, alpha],
        alpha: PictureAlphaBlend {
            a: 0,
            b: 1,
            c: 0,
            d: 1,
            fix: 0x80,
        },
        priority: 3,
        flags: PictureFlags(
            PictureFlags::TEXTURED | PictureFlags::CUSTOM_RECT | PictureFlags::ALPHA_BLEND,
        ),
        ..PictureDrawParams::default()
    }));
}

/// `EvSubPictureEnd` (`SLUS_202.28` `0x001ace70`, `sh2pc.exe` `0x004a2880`).
///
/// Steps `ev_filter` toward `1.0` (if `filter_on`) or `0.0` at `4.0/s`, resets `filter_on`, and
/// when `ev_filter > 0.0` emits the black dimming quad at priority `8` (`flags = 0x30`,
/// `ALPHA_1 = (2, 1, 2, 1, (ev_filter * 64.0) as u8)`).
pub fn ev_sub_picture_end(dt: f32, pic: &mut EvSubPictureState) {
    let dt = dt.max(0.0);
    if pic.filter_on {
        pic.filter_alpha = (pic.filter_alpha + dt * EV_FILTER_RATE_PER_SEC).min(1.0);
    } else {
        pic.filter_alpha = (pic.filter_alpha - dt * EV_FILTER_RATE_PER_SEC).max(0.0);
    }
    pic.filter_on = false;

    if pic.filter_alpha > 0.0 {
        let fix = (pic.filter_alpha * EV_FILTER_ALPHA_SCALE) as i32 as u8;
        pic.draw_commands.push(picture_draw(PictureDrawParams {
            rgba: [0, 0, 0, 0x80],
            alpha: PictureAlphaBlend {
                a: 2,
                b: 1,
                c: 2,
                d: 1,
                fix,
            },
            priority: 8,
            flags: PictureFlags(PictureFlags::CUSTOM_RGB | PictureFlags::ALPHA_BLEND),
            ..PictureDrawParams::default()
        }));
    }
}

/// `EvSubPictureDisplayAndFadeIn` (`SLUS_202.28` `0x001ac8b0`).
pub fn ev_sub_picture_display_and_fade_in(
    dt: f32,
    fade_complete: bool,
    state: &mut EventStepState,
    pic: &mut EvSubPictureState,
    effects: &mut EventEngineEffects,
) -> bool {
    if state.ev_s_step == 0 {
        effects.fade_command = Some(4);
        state.ev_s_step = 3;
    }
    ev_sub_picture_start(pic);
    ev_sub_picture_display_only(pic);
    ev_sub_picture_end(dt, pic);
    fade_complete
}

/// `EvSubPictureDisplayAndFadeOut` (`SLUS_202.28` `0x001ac9a0`).
pub fn ev_sub_picture_display_and_fade_out(
    dt: f32,
    fade_complete: bool,
    state: &mut EventStepState,
    pic: &mut EvSubPictureState,
    effects: &mut EventEngineEffects,
) -> bool {
    if state.ev_s_step == 0 {
        effects.fade_command = Some(1);
        state.ev_s_step = 3;
    }
    ev_sub_picture_start(pic);
    ev_sub_picture_display_only(pic);
    ev_sub_picture_end(dt, pic);
    fade_complete
}

/// Inputs for one frame of [`ev_sub_picture_display`] (`EvSubPictureDisplay` `0x001aca00`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EvSubPictureDisplayInput {
    /// Cutaway picture file ID (`param_1`).
    pub file_id: u32,
    /// Stage `.mes` message ID displayed when the player inspects the cutaway (`param_2`).
    pub msg_id: u16,
    /// Frame delta time in seconds (`shGetDT()`).
    pub dt: f32,
    /// `true` when `fsSync(1, -1) >= 0`.
    pub fs_sync_ready: bool,
    /// `true` when `ScreenEffectFadeCheck() != 0`.
    pub fade_complete: bool,
    /// `true` when confirm or cancel button triggered on step `2`.
    pub confirm_or_cancel_trigger: bool,
    /// `true` when `fontGetStatus() == -2` on step `3`.
    pub message_dismissed: bool,
}

/// Executes one frame of `EvSubPictureDisplay` (`SLUS_202.28` `0x001aca00`).
///
/// Exact step machine (`ev_s_step`): `0 -> 4 -> 7 -> 2 -> 3 -> 6 -> 11 (0xb)`.
pub fn ev_sub_picture_display(
    input: &EvSubPictureDisplayInput,
    state: &mut EventStepState,
    pic: &mut EvSubPictureState,
    effects: &mut EventEngineEffects,
) -> bool {
    match state.ev_s_step {
        0xb => {
            effects.player_lock_change = Some(false);
            effects.fade_command = Some(4);
            true
        }
        6 => {
            ev_sub_picture_start(pic);
            ev_sub_picture_display_only(pic);
            ev_sub_picture_end(input.dt, pic);
            if input.fade_complete {
                state.ev_s_step = 0xb;
            }
            false
        }
        3 => {
            ev_sub_picture_start(pic);
            ev_sub_picture_display_only(pic);
            ev_sub_picture_filter(pic);
            ev_sub_picture_end(input.dt, pic);
            if input.message_dismissed {
                effects.clear_message = true;
                effects.fade_command = Some(2);
                state.ev_s_step = 6;
            }
            false
        }
        2 => {
            ev_sub_picture_start(pic);
            ev_sub_picture_display_only(pic);
            ev_sub_picture_end(input.dt, pic);
            if input.confirm_or_cancel_trigger {
                effects.stage_message = Some(input.msg_id);
                state.ev_s_step = 3;
            }
            false
        }
        7 => {
            ev_sub_picture_start(pic);
            ev_sub_picture_display_only(pic);
            ev_sub_picture_end(input.dt, pic);
            if input.fade_complete {
                state.ev_s_step = 2;
            }
            false
        }
        4 => {
            if input.fs_sync_ready && input.fade_complete {
                effects.fade_command = Some(4);
                state.ev_s_step = 7;
            }
            false
        }
        0 => {
            pic.queued_file_id = Some(input.file_id);
            effects.fade_command = Some(1);
            ev_sub_picture_init(pic);
            effects.player_lock_change = Some(true);
            state.ev_s_step = 4;
            false
        }
        _ => false,
    }
}

/// `EvSubMapGet` (`SLUS_202.28` `0x001accc0`) — thin alias directly forwarding to
/// [`ev_sub_picture_display`].
pub fn ev_sub_map_get(
    input: &EvSubPictureDisplayInput,
    state: &mut EventStepState,
    pic: &mut EvSubPictureState,
    effects: &mut EventEngineEffects,
) -> bool {
    ev_sub_picture_display(input, state, pic, effects)
}

/// Updates the interactive cutaway cursor and emits its sprite draw command
/// (`EvSubPictureCursor` in `SLUS_202.28` `0x001ad010` and `sh2pc.exe` `0x004a2a50`).
///
/// Prototype-vs-retail drift: `SLUS_202.28` clamps Y to `±192.0` ([`CursorBounds::PS2`]),
/// whereas retail PC `sh2pc.exe` (`0x004a2a50`) clamps Y to `±240.0` ([`CursorBounds::PC`]).
pub fn ev_sub_picture_cursor(
    tint: CursorTintMode,
    input: CursorInput,
    dt: f32,
    bounds: CursorBounds,
    pic: &mut EvSubPictureState,
) {
    let dt = dt.max(0.0);
    let ax = decode_cursor_axis(input.stick_x);
    let ay = decode_cursor_axis(input.stick_y);

    if ax == 0.0 && ay == 0.0 {
        if input.dpad_right {
            pic.cursor_x += dt * CURSOR_SPEED_X_PX_PER_SEC;
        }
        if input.dpad_left {
            pic.cursor_x -= dt * CURSOR_SPEED_X_PX_PER_SEC;
        }
        if input.dpad_down {
            pic.cursor_y += dt * CURSOR_SPEED_Y_PX_PER_SEC;
        }
        if input.dpad_up {
            pic.cursor_y -= dt * CURSOR_SPEED_Y_PX_PER_SEC;
        }
    } else {
        pic.cursor_x += ax * dt * CURSOR_SPEED_X_PX_PER_SEC;
        pic.cursor_y += ay * dt * CURSOR_SPEED_Y_PX_PER_SEC;
    }

    pic.cursor_x = pic.cursor_x.clamp(-bounds.half_width, bounds.half_width);
    pic.cursor_y = pic.cursor_y.clamp(-bounds.half_height, bounds.half_height);

    pic.load_commands.push(picture_load_image(8, 8, -1, -1, 0));

    // Q4 fixed-point conversion (`_vftoi4(cursor - offset)`): 16 units = 1 virtual pixel.
    let x0 = ((pic.cursor_x - 5.0) * 16.0) as i16;
    let y0 = ((pic.cursor_y - 6.0) * 16.0) as i16;
    let x1 = x0.saturating_add(0x180); // +24.0 px
    let y1 = y0.saturating_add(0x200); // +32.0 px

    let base_flags = PictureFlags::TEXTURED
        | PictureFlags::CUSTOM_RECT
        | PictureFlags::INTEGER_UV
        | PictureFlags::ALPHA_BLEND;
    let (rgb, flags) = match tint {
        CursorTintMode::Neutral => ([0x80, 0x80, 0x80], PictureFlags(base_flags)),
        CursorTintMode::Blue => (
            [0x60, 0x60, 0x80],
            PictureFlags(base_flags | PictureFlags::CUSTOM_RGB),
        ),
        CursorTintMode::Red => (
            [0x80, 0x60, 0x60],
            PictureFlags(base_flags | PictureFlags::CUSTOM_RGB),
        ),
        CursorTintMode::Green => (
            [0x60, 0x80, 0x60],
            PictureFlags(base_flags | PictureFlags::CUSTOM_RGB),
        ),
    };

    pic.draw_commands.push(picture_draw(PictureDrawParams {
        tex_slot: 8,
        rect_q4: [x0, y0, x1, y1],
        uv: PictureUv::IntegerQ4([0x600, 0x400, 0x8f0, 0x7f0]),
        rgba: [rgb[0], rgb[1], rgb[2], 0x80],
        alpha: PictureAlphaBlend {
            a: 0,
            b: 1,
            c: 0,
            d: 1,
            fix: 0x80,
        },
        priority: 9,
        flags,
        ..PictureDrawParams::default()
    }));
}
