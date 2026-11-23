//! Picture Draw & Event Cutaway Coroutines (`SLUS_202.28` `Event/picture.c`
//! `0x001ae360..0x001af097`, `Event/event_sub.c` `0x001ac8b0..0x001ad557`, and
//! `sh2pc.exe` `0x004a2760..0x004a2f02`).
//!
//! Pure data/command layer (`sh2-core` has zero GPU/windowing dependencies): produces typed
//! [`PictureDrawCommand`] and [`PictureLoadImageCommand`] descriptors that `sh2-renderer` /
//! `sh2-ui` can submit directly.

use crate::events::engine::{EventEngineEffects, EventStepState};

/// Default GS CLUT base pointer (`0x3640` in `PictureLoadImage` `SLUS_202.28` `0x001aee70`).
pub const PICTURE_DEFAULT_CBP: i32 = 0x3640;

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

/// Bitflags at offset `+0x42` of the 68-byte (`0x44`) `PictureDraw` parameter block
/// (`SLUS_202.28` `PictureDraw` `0x001ae360`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PictureFlags(pub u16);

impl PictureFlags {
    /// Bit `0x01`: Textured primitive (if clear, draws an untextured solid sprite rectangle).
    pub const TEXTURED: u16 = 0x01;
    /// Bit `0x02`: Custom screen rectangle (`x0, y0, x1, y1` in Q4); otherwise full-screen
    /// `[-4096, -4096, +4096, +4096]` (`-256.0..+256.0` px).
    pub const CUSTOM_RECT: u16 = 0x02;
    /// Bit `0x04`: Integer Q4 texel UV mode (`GS UV` register) instead of normalized STQ floats.
    pub const INTEGER_UV: u16 = 0x04;
    /// Bit `0x08`: Custom normalized STQ float UVs (`[u0, v0, u1, v1]`); if neither `0x04` nor
    /// `0x08` is set, defaults to `[0.0, 0.0, 1.0, 1.0]`.
    pub const FLOAT_STQ: u16 = 0x08;
    /// Bit `0x10`: Custom RGB tint (`[r, g, b]`); otherwise defaults to `(0x80, 0x80, 0x80)`.
    pub const CUSTOM_RGB: u16 = 0x10;
    /// Bit `0x20`: Alpha blending enabled (`ALPHA_1` emitted, `ABE` bit set); otherwise `a = 0x80`.
    pub const ALPHA_BLEND: u16 = 0x20;
    /// Bit `0x40`: Custom GS `TEST_1` register emitted.
    pub const CUSTOM_TEST: u16 = 0x40;
    /// Bit `0x80`: 4-vertex triangle strip (`PRIM = 4`, using `x2, y2, x3, y3`) instead of
    /// 2-vertex sprite (`PRIM = 6`).
    pub const QUAD_STRIP: u16 = 0x80;

    /// Returns `true` if all bits in `mask` are set.
    #[inline]
    pub const fn contains(self, mask: u16) -> bool {
        (self.0 & mask) == mask
    }
}

/// PS2 GS `ALPHA_1` blend register fields (`+0x30..+0x34` in `PictureDraw` `0x001ae360`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PictureAlphaBlend {
    /// Blend equation `A` (`0`=Cs, `1`=Cd, `2`=0).
    pub a: u8,
    /// Blend equation `B` (`0`=Cs, `1`=Cd, `2`=0).
    pub b: u8,
    /// Blend equation `C` (`0`=As, `1`=Ad, `2`=FIX).
    pub c: u8,
    /// Blend equation `D` (`0`=Cs, `1`=Cd, `2`=0).
    pub d: u8,
    /// Fixed alpha constant `FIX` (`0..=0x80`).
    pub fix: u8,
}

impl PictureAlphaBlend {
    /// Packs the fields into the 64-bit PS2 GS `ALPHA_1` register (`0x42`) value
    /// (`PictureDraw` `0x001ae360` lines 54-58 / 149-153).
    pub const fn pack_gs_alpha1(self) -> u64 {
        (self.a as u64)
            | ((self.b as u64) << 2)
            | ((self.c as u64) << 4)
            | ((self.d as u64) << 6)
            | ((self.fix as u64) << 32)
    }
}

/// PS2 GS `TEST_1` register fields (`+0x35..+0x3c` in `PictureDraw` `0x001ae360`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PictureTestConfig {
    pub ate: u8,
    pub atst: u8,
    pub aref: u8,
    pub afail: u8,
    pub date: u8,
    pub datm: u8,
    pub zte: u8,
    pub ztst: u8,
}

impl PictureTestConfig {
    /// Packs the fields into the 64-bit PS2 GS `TEST_1` register (`0x47`) value
    /// (`PictureDraw` `0x001ae360` lines 66-73 / 134-141).
    pub const fn pack_gs_test1(self) -> u64 {
        (self.ate as u64)
            | ((self.atst as u64) << 1)
            | ((self.aref as u64) << 4)
            | ((self.afail as u64) << 12)
            | ((self.date as u64) << 14)
            | ((self.datm as u64) << 15)
            | ((self.zte as u64) << 16)
            | ((self.ztst as u64) << 17)
    }
}

/// Texture coordinate representation resolved by [`picture_draw`] (`0x001ae360`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PictureUv {
    /// Normalized STQ floats `[s0, t0, s1, t1]` (`flags & 0x04 == 0`).
    FloatStq([f32; 4]),
    /// Integer Q4 texel coordinates `[u0, v0, u1, v1]` (`flags & 0x04 != 0`).
    IntegerQ4([u32; 4]),
}

/// Primitive topology emitted by [`picture_draw`] (`0x001ae360`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PicturePrimitiveKind {
    /// Untextured 2-vertex solid rectangle (`PRIM = 6`, `flags & 0x01 == 0`).
    SolidSprite,
    /// Textured 2-vertex axis-aligned sprite (`PRIM = 6`, `flags & 0x81 == 0x01`).
    TexturedSprite,
    /// Textured 4-vertex triangle strip (`PRIM = 4`, `flags & 0x81 == 0x81`).
    TexturedQuadStrip,
}

/// Input parameter block mirroring the 68-byte (`0x44`) stack struct passed to `PictureDraw`
/// (`SLUS_202.28` `Event/picture.c` `0x001ae360`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PictureDrawParams {
    /// Logical texture buffer/slot handle (`+0x00`).
    pub tex_slot: u32,
    /// GS texture base pointer (`+0x04`; `-1` = default 2D TBP0).
    pub tbp0: i32,
    /// GS CLUT base pointer (`+0x08`; `-1` = default CLUT `0x3640`).
    pub cbp: i32,
    /// Screen corners in Q4 fixed-point (`+0x0c..+0x1b`; 16 units = 1 virtual pixel).
    pub rect_q4: [i16; 4],
    /// Extra two corners `(x2, y2, x3, y3)` in Q4 when `PictureFlags::QUAD_STRIP` is set.
    pub extra_verts_q4: [i16; 4],
    /// Texture coordinates (`+0x1c..+0x2b`).
    pub uv: PictureUv,
    /// RGBA color in PS2 GS scale (`0x80` = 1.0 neutral, `+0x2c..+0x2f`).
    pub rgba: [u8; 4],
    /// GS `ALPHA_1` blend configuration (`+0x30..+0x34`).
    pub alpha: PictureAlphaBlend,
    /// GS `TEST_1` configuration (`+0x35..+0x3c`).
    pub test: PictureTestConfig,
    /// Draw priority (`+0x40`; GIF sort key is `(0x100 - priority) & 0xffff`).
    pub priority: i16,
    /// Feature bitflags (`+0x42`).
    pub flags: PictureFlags,
}

impl Default for PictureDrawParams {
    fn default() -> Self {
        Self {
            tex_slot: 0,
            tbp0: -1,
            cbp: -1,
            rect_q4: [0; 4],
            extra_verts_q4: [0; 4],
            uv: PictureUv::FloatStq([0.0, 0.0, 1.0, 1.0]),
            rgba: [0; 4],
            alpha: PictureAlphaBlend::default(),
            test: PictureTestConfig::default(),
            priority: 0,
            flags: PictureFlags(0),
        }
    }
}

/// Resolved draw command produced by [`picture_draw`] (`PictureDraw` `0x001ae360`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PictureDrawCommand {
    /// Primitive kind (`SolidSprite`, `TexturedSprite`, or `TexturedQuadStrip`).
    pub primitive: PicturePrimitiveKind,
    /// Logical texture slot (`0` for untextured solid rects).
    pub tex_slot: u32,
    /// Resolved screen rect `[x0, y0, x1, y1]` in Q4 fixed-point (`-4096..+4096` = `-256..+256` px).
    pub rect_q4: [i16; 4],
    /// Resolved extra corners `[x2, y2, x3, y3]` in Q4 (used when `TexturedQuadStrip`).
    pub extra_verts_q4: [i16; 4],
    /// Resolved texture coordinates (or `None` for `SolidSprite`).
    pub uv: Option<PictureUv>,
    /// Resolved RGBA color in PS2 GS `0..=0x80` scale.
    pub rgba: [u8; 4],
    /// Packed `ALPHA_1` register if alpha blending is enabled (`flags & 0x20`).
    pub alpha_blend: Option<PictureAlphaBlend>,
    /// Packed `TEST_1` register if custom test is enabled (`flags & 0x40`).
    pub test_config: Option<PictureTestConfig>,
    /// Draw priority (`+0x40`).
    pub priority: i16,
    /// GIF sort key `(0x100u16.wrapping_sub(priority as u16))` (`0x001ae360` line 44 / 124).
    pub sort_key: u16,
}

/// Resolves default fields and produces a [`PictureDrawCommand`] matching `PictureDraw`
/// (`SLUS_202.28` `Event/picture.c` `0x001ae360`).
pub fn picture_draw(mut params: PictureDrawParams) -> PictureDrawCommand {
    // Default screen rect: [-4096, -4096, +4096, +4096] (0xf000..0x1000) when bit 0x02 clear
    if !params.flags.contains(PictureFlags::CUSTOM_RECT) {
        params.rect_q4 = [-0x1000, -0x1000, 0x1000, 0x1000];
    }
    // Default RGB: 0x80, 0x80, 0x80 when bit 0x10 clear
    if !params.flags.contains(PictureFlags::CUSTOM_RGB) {
        params.rgba[0] = 0x80;
        params.rgba[1] = 0x80;
        params.rgba[2] = 0x80;
    }
    let alpha_enabled = params.flags.contains(PictureFlags::ALPHA_BLEND);
    if !alpha_enabled {
        params.rgba[3] = 0x80;
    }

    let textured = params.flags.contains(PictureFlags::TEXTURED);
    let quad_strip = params.flags.contains(PictureFlags::QUAD_STRIP);
    let sort_key = 0x0100u16.wrapping_sub(params.priority as u16);

    let uv = if textured {
        if !params.flags.contains(PictureFlags::INTEGER_UV)
            && !params.flags.contains(PictureFlags::FLOAT_STQ)
        {
            Some(PictureUv::FloatStq([0.0, 0.0, 1.0, 1.0]))
        } else {
            Some(params.uv)
        }
    } else {
        None
    };

    let primitive = if !textured {
        PicturePrimitiveKind::SolidSprite
    } else if quad_strip {
        PicturePrimitiveKind::TexturedQuadStrip
    } else {
        PicturePrimitiveKind::TexturedSprite
    };

    PictureDrawCommand {
        primitive,
        tex_slot: if textured { params.tex_slot } else { 0 },
        rect_q4: params.rect_q4,
        extra_verts_q4: params.extra_verts_q4,
        uv,
        rgba: params.rgba,
        alpha_blend: alpha_enabled.then_some(params.alpha),
        test_config: params
            .flags
            .contains(PictureFlags::CUSTOM_TEST)
            .then_some(params.test),
        priority: params.priority,
        sort_key,
    }
}

/// Texture upload descriptor emitted by [`picture_load_image`] (`PictureLoadImage` `0x001aee70`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PictureLoadImageCommand {
    /// Logical texture buffer/slot being uploaded (`0` = main picture, `2` = layer, `8` = cursor).
    pub tex_slot: u32,
    /// DMA environment priority (`slot + 0x10` in `PictureLoadImage` `0x001aee70` line 300).
    pub dma_priority: i16,
    /// Resolved GS texture base pointer (`tbp0` or `default_tbp0` when `-1`).
    pub tbp0: i32,
    /// Resolved GS CLUT base pointer (`cbp` or [`PICTURE_DEFAULT_CBP`] `0x3640` when `-1`).
    pub cbp: i32,
}

/// Constructs a [`PictureLoadImageCommand`] matching `PictureLoadImage`
/// (`SLUS_202.28` `Event/picture.c` `0x001aee70`).
pub const fn picture_load_image(
    tex_slot: u32,
    slot_priority: i16,
    tbp0: i32,
    cbp: i32,
    default_tbp0: i32,
) -> PictureLoadImageCommand {
    PictureLoadImageCommand {
        tex_slot,
        dma_priority: slot_priority + 0x10,
        tbp0: if tbp0 == -1 { default_tbp0 } else { tbp0 },
        cbp: if cbp == -1 { PICTURE_DEFAULT_CBP } else { cbp },
    }
}

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
