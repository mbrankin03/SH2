//! Low-Level 2D Picture Drawing & GS Upload Commands (`SLUS_202.28` `Event/picture.c`
//! `0x001ae360..0x001af097`).
//!
//! Pure data/command layer (`sh2-core` has zero GPU/windowing dependencies): resolves the
//! 68-byte (`0x44`) `PictureDraw` parameter block and `PictureLoadImage` DMA upload descriptors
//! into typed commands that `sh2-renderer` / `sh2-ui` can submit directly.

/// Default GS CLUT base pointer (`0x3640` in `PictureLoadImage` `SLUS_202.28` `0x001aee70`).
pub const PICTURE_DEFAULT_CBP: i32 = 0x3640;

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
