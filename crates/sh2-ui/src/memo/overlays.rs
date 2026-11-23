//! Exact 1:1 PS2 / PC Memo puzzle overlays and dynamic code formatters (`Event/memo.c`).
//!
//! # Ground Truth Citations (`SLUS_202.28` & `sh2pc.exe`)
//! - `MemoPictureLayerDraw` (`0x002316d0` / `0x0049a6a0`)
//! - `MemoPictureLayerDrawSafeLock` (`0x002317c0` / `0x00499250`)
//! - `MemoPictureLayerDrawGuruguru` (`0x00231b60` / `0x004994d0`, `tex$804` `0x0034d1a0`, `pos$805` `0x0034d2c0`)
//! - `MemoPictureLayerDrawAngelRing` (`0x00231d40` / `0x00499620`)
//! - `MemoPictureLayerDrawHair` (`0x00231f10` / `0x00499780`)
//! - `MemoPictureLayerDrawTablet` (`0x00231ff0` / `0x00499800`)
//! - `MemoMessageWallet` (`0x002321f0` / `0x004999b0`)
//! - Keypad dynamic code (`MemoDisplay` `0x00230f30` / `0x0049a960`)

use std::f32::consts::TAU;

use sh2_core::config::RiddleDifficulty;
use sh2_core::events::{EventFlags, PuzzleState, PZ_BOX_CODE, PZ_SAFE_CODE};

/// Exact `tex$804` (`SLUS_202.28` `0x0034d1a0` / `sh2pc.exe` `0x0088c708`) UV rects `[u0, v0, u1, v1]`
/// inside `512x256` `pic/hsp/p_boxnumber_2.tex` for digits `0..9` × 4 tumbler slots.
#[rustfmt::skip]
pub const GURUGURU_TEX_UVS: [[[i16; 4]; 4]; 9] = [
    [[0, 0, 32, 64],    [160, 0, 192, 64],   [320, 0, 352, 64],   [0, 128, 16, 176]],
    [[32, 0, 64, 64],   [192, 0, 224, 48],   [352, 0, 400, 64],   [16, 128, 48, 176]],
    [[64, 0, 96, 64],   [224, 0, 256, 48],   [400, 0, 432, 64],   [48, 128, 80, 192]],
    [[96, 0, 128, 64],  [256, 0, 288, 48],   [432, 0, 464, 80],   [80, 128, 112, 192]],
    [[128, 0, 160, 64], [288, 0, 320, 64],   [464, 0, 496, 64],   [112, 128, 144, 192]],
    [[0, 64, 32, 128],  [160, 64, 192, 128], [320, 64, 368, 112], [144, 128, 176, 192]],
    [[32, 64, 64, 128], [192, 48, 240, 112], [368, 64, 400, 112], [176, 128, 208, 224]],
    [[64, 64, 96, 128], [240, 48, 272, 112], [400, 80, 448, 144], [208, 128, 240, 192]],
    [[96, 64, 128, 128],[272, 64, 304, 128], [464, 64, 496, 128], [240, 128, 272, 192]],
];

/// Exact `pos$805` (`SLUS_202.28` `0x0034d2c0` / `sh2pc.exe` `0x0088c828`) top-left Q4 fixed-point
/// positions `[x_q4, y_q4]` (`>> 4` = PS2 centered `[-256..+256]` px) for digits `0..9` × 4 slots.
#[rustfmt::skip]
pub const GURUGURU_POS_Q4: [[[i16; 2]; 4]; 9] = [
    [[-768, -1120], [-384, -832], [64, -1152],  [656, -608]],
    [[-896, -1072], [-384, -736], [0, -1168],   [577, -736]],
    [[-896, -1056], [-432, -672], [80, -1088],  [560, -784]],
    [[-880, -1088], [-368, -672], [112, -1200], [560, -736]],
    [[-800, -1168], [-480, -944], [0, -1072],   [576, -784]],
    [[-800, -1184], [-480, -880], [0, -864],    [544, -624]],
    [[-768, -1152], [-464, -928], [128, -928],  [609, -800]],
    [[-864, -1120], [-416, -944], [-16, -1216], [560, -879]],
    [[-816, -1216], [-416, -976], [80, -1136],  [528, -720]],
];

/// A single textured quad to render in PS2 `512x512` centered `[-256..+256]` virtual coordinates.
/// Corners are ordered `[TL, TR, BL, BR]` with normalized UVs `[u0, v0, u1, v1]`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MemoLayerQuad {
    /// 4 corner coordinates in PS2 centered `[-256..+256]` virtual pixels: `[TL, TR, BL, BR]`.
    pub corners_ps2: [(f32, f32); 4],
    /// Source UV coordinates in texture pixels `[u0, v0, u1, v1]`.
    pub uv_px: [f32; 4],
}

impl MemoLayerQuad {
    /// Constructs an axis-aligned quad from Q4 fixed-point `[-4096..+4095]` bounds and pixel UVs.
    #[must_use]
    pub fn from_q4_rect(x0: i16, y0: i16, x1: i16, y1: i16, uv_px: [f32; 4]) -> Self {
        let fx0 = (x0 as f32) / 16.0;
        let fy0 = (y0 as f32) / 16.0;
        let fx1 = (x1 as f32) / 16.0;
        let fy1 = (y1 as f32) / 16.0;
        Self {
            corners_ps2: [(fx0, fy0), (fx1, fy0), (fx0, fy1), (fx1, fy1)],
            uv_px,
        }
    }
}

/// Computes the 0..4 [`MemoLayerQuad`]s for the active memo entry (`MemoPictureLayerDraw`
/// `SLUS_202.28` `0x002316d0` / `sh2pc.exe` `0x0049a6a0`).
#[must_use]
pub fn build_memo_layer_quads(
    catalog_index: usize,
    flags: &EventFlags,
    puzzle: &PuzzleState,
) -> Vec<MemoLayerQuad> {
    match catalog_index {
        0x12 if flags.get(607) => vec![build_safe_lock_quad(flags)],
        0x20 => build_guruguru_quads(puzzle),
        0x21 if flags.get(171) => vec![build_hair_quad()],
        0x23 if flags.get(227) || flags.get(228) => build_angel_ring_quads(flags),
        0x29 => build_tablet_quads(flags),
        _ => Vec::new(),
    }
}

/// `MemoPictureLayerDrawSafeLock` (`SLUS_202.28` `0x002317c0` / `sh2pc.exe` `0x00499250`).
///
/// Reads 5-bit dial index from `EventFlags` bits `103..=107` (`0x67..=0x6b`), rotates
/// `[-80..+80] x [-64..+64]` by `-(dial + 1) * 2π / 20`, scales X by `0.8`, and translates by
/// `(-14.5, -4.75)` with UV `[0, 0, 127, 127]` (`0x7f0 >> 4`) in `128x128` `p_safe_close2.tex`.
#[must_use]
pub fn build_safe_lock_quad(flags: &EventFlags) -> MemoLayerQuad {
    let mut dial: u32 = 0;
    for bit in 0..5u32 {
        if flags.get((0x67 + bit) as usize) {
            dial |= 1 << bit;
        }
    }
    let angle = -(((dial + 1) as f32) * TAU / 20.0);
    let s = angle.sin();
    let c = angle.cos();
    let local = [(-80.0, -64.0), (80.0, -64.0), (-80.0, 64.0), (80.0, 64.0)];
    let mut corners = [(0.0f32, 0.0f32); 4];
    for (i, (lx, ly)) in local.into_iter().enumerate() {
        let rx = (c * lx - s * ly) * 0.8 - 14.5;
        let ry = (c * ly + s * lx) - 4.75;
        corners[i] = (rx, ry);
    }
    MemoLayerQuad {
        corners_ps2: corners,
        uv_px: [0.0, 0.0, 127.0, 127.0],
    }
}

/// `MemoPictureLayerDrawGuruguru` (`SLUS_202.28` `0x00231b60` / `sh2pc.exe` `0x004994d0`).
///
/// Draws the 4 Hospital Louise's Box tumbler digits from `puzzle.raw()[8..12]` (`DAT_011268b8` /
/// `DAT_01f7ae68`) using `GURUGURU_TEX_UVS` (`tex$804`) and `GURUGURU_POS_Q4` (`pos$805`).
#[must_use]
pub fn build_guruguru_quads(puzzle: &PuzzleState) -> Vec<MemoLayerQuad> {
    let mut out = Vec::with_capacity(4);
    let raw = puzzle.raw();
    for slot in 0..4usize {
        let digit = (raw[8 + slot] as usize).min(8);
        let [u0, v0, u1, v1] = GURUGURU_TEX_UVS[digit][slot];
        let [px0, py0] = GURUGURU_POS_Q4[digit][slot];
        let px1 = px0.saturating_add((u1 - u0) * 16);
        let py1 = py0.saturating_add((v1 - v0) * 16);
        out.push(MemoLayerQuad::from_q4_rect(
            px0,
            py0,
            px1,
            py1,
            [u0 as f32, v0 as f32, (u1 - 1) as f32, (v1 - 1) as f32],
        ));
    }
    out
}

/// `MemoPictureLayerDrawAngelRing` (`SLUS_202.28` `0x00231d40` / `sh2pc.exe` `0x00499620`).
///
/// Draws Copper Ring (`flags.get(227)`) and/or Lead Ring (`flags.get(228)`) from `64x64`
/// `pic/hsp/p_female_ring.tex`.
#[must_use]
pub fn build_angel_ring_quads(flags: &EventFlags) -> Vec<MemoLayerQuad> {
    let mut out = Vec::with_capacity(2);
    if flags.get(227) {
        out.push(MemoLayerQuad::from_q4_rect(
            0x0939,
            0x0635,
            0x0a39,
            0x0735,
            [0.0, 0.0, 15.0, 15.0],
        ));
    }
    if flags.get(228) {
        out.push(MemoLayerQuad::from_q4_rect(
            0x0472,
            0x0006,
            0x0572,
            0x0106,
            [16.0, 0.0, 31.0, 15.0],
        ));
    }
    out
}

/// `MemoPictureLayerDrawHair` (`SLUS_202.28` `0x00231f10` / `sh2pc.exe` `0x00499780`).
///
/// Draws the full `256x64` `pic/hsp/p_hair_hair.tex` over the drain at Q4
/// `(0xfb90, 0xfcc0, 0x0b90, 0x00c0)` = `(-71.0, -52.0, +185.0, +12.0)`.
#[must_use]
pub fn build_hair_quad() -> MemoLayerQuad {
    MemoLayerQuad::from_q4_rect(
        0xfb90u16 as i16,
        0xfcc0u16 as i16,
        0x0b90,
        0x00c0,
        [0.0, 0.0, 256.0, 64.0],
    )
}

/// `MemoPictureLayerDrawTablet` (`SLUS_202.28` `0x00231ff0` / `sh2pc.exe` `0x00499800`).
///
/// Draws up to 3 Toluca Prison tablets (`flags.get(301)`, `302`, `303`) from `256x256`
/// `pic/dls/p_tablet_plate.tex`.
#[must_use]
pub fn build_tablet_quads(flags: &EventFlags) -> Vec<MemoLayerQuad> {
    let mut out = Vec::with_capacity(3);
    if flags.get(301) {
        out.push(MemoLayerQuad::from_q4_rect(
            0xf528u16 as i16,
            0xff90u16 as i16,
            0xfb68u16 as i16,
            0x05d0,
            [0.0, 0.0, 100.0, 100.0],
        ));
    }
    if flags.get(302) {
        out.push(MemoLayerQuad::from_q4_rect(
            0xfd3bu16 as i16,
            0xff90u16 as i16,
            0x037b,
            0x05d0,
            [100.0, 0.0, 200.0, 100.0],
        ));
    }
    if flags.get(303) {
        out.push(MemoLayerQuad::from_q4_rect(
            0x04ee,
            0xffceu16 as i16,
            0x0b2e,
            0x060e,
            [0.0, 100.0, 100.0, 200.0],
        ));
    }
    out
}

/// Exact 1:1 port of `MemoMessageWallet` (`sh2pc.exe` `0x004999b0`, cross-checked against
/// `SLUS_202.28` `0x002321f0`).
///
/// Formats the 4 Blue Creek Apartment Room 203 wallet safe dial codes (`PZ_SAFE_CODE` `0..4`):
/// - **Easy / Normal** (`sh2pc.exe` `< 2`): zero-padded 2-digit `"01".."20"` (`SLUS_202.28` Easy
///   used relative step offsets `(prev - cur + 20) % 20`, which retail unified with Normal).
/// - **Hard** (`== 2`): `20 -> "XX"`, `< 10 -> "{v}"`, `10..=19` even slot `-> "VV{v%10}"`, odd slot `-> "X{v%10}"`.
/// - **Extra** (`== 3`): `< 10 -> "{v}"`, `>= 10 -> char('a' + (v - 10))`.
#[must_use]
pub fn format_memo_wallet_codes(puzzle: &PuzzleState, riddle: RiddleDifficulty) -> [String; 4] {
    let raw = puzzle.raw();
    std::array::from_fn(|slot| {
        let b = raw[PZ_SAFE_CODE + slot];
        let v = (b as u32) + 1;
        match riddle {
            RiddleDifficulty::Easy | RiddleDifficulty::Normal => {
                format!("{}{}", v / 10, v % 10)
            }
            RiddleDifficulty::Hard => {
                if v >= 20 {
                    "XX".to_string()
                } else if v < 10 {
                    format!("{v}")
                } else if (slot & 1) == 0 {
                    format!("VV{}", v % 10)
                } else {
                    format!("X{}", v % 10)
                }
            }
            RiddleDifficulty::Extra => {
                if v < 10 {
                    format!("{v}")
                } else {
                    // sh2pc.exe 0x00499a10 / SLUS 0x00232510: b + 0x58 ('a' for b=9 -> v=10)
                    char::from(b.wrapping_add(0x58)).to_string()
                }
            }
        }
    })
}

/// Exact 1:1 port of the Hospital carbon-paper keypad dynamic code formatter
/// (`MemoDisplay` `SLUS_202.28` `0x00230f30` / `sh2pc.exe` `0x0049a960`).
///
/// Formats the 4 BCD nibbles of `puzzle.read_u16(PZ_BOX_CODE)` (`DAT_011268c2` / `DAT_01f7ae72`).
#[must_use]
pub fn format_memo_keypad_code(puzzle: &PuzzleState) -> String {
    let code = puzzle.read_u16(PZ_BOX_CODE);
    format!(
        "{}{}{}{}",
        (code >> 12) & 0x0f,
        (code >> 8) & 0x0f,
        (code >> 4) & 0x0f,
        code & 0x0f
    )
}

/// Substitutes dynamic `.mes` message insert tokens (`<e0>..<e3>` or `{0}..{3}`) for wallet
/// (catalog index `0x12`) and keypad (`0x1d`, `0x1e`) memos.
#[must_use]
pub fn apply_memo_dynamic_codes(
    catalog_index: usize,
    text: &str,
    puzzle: &PuzzleState,
    riddle: RiddleDifficulty,
) -> String {
    match catalog_index {
        0x12 => {
            let codes = format_memo_wallet_codes(puzzle, riddle);
            let mut out = text.to_string();
            for (i, code) in codes.iter().enumerate() {
                out = out.replace(&format!("<e{i}>"), code);
                out = out.replace(&format!("%{i}"), code);
            }
            out
        }
        0x1d | 0x1e => {
            let code = format_memo_keypad_code(puzzle);
            text.replace("<e0>", &code).replace("%0", &code)
        }
        _ => text.to_string(),
    }
}
