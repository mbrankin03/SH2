//! Retail PC (`sh2pc.exe`) post-prototype Memo puzzle overlays (`0x35` Lakeview Hotel briefcase
//! & `0x36` *Born From a Wish* Baldwin Mansion board).
//!
//! # Ground Truth Citations (`sh2pc.exe`)
//! - `MemoPictureLayerDraw` (`FUN_0049a6a0` `0x0049a6a0`, `case 0x35`):
//!   - 5-bit word index from `DAT_01f7aa10` bits `22..=26` (`EventFlags` `406..=410`).
//!   - Word table `0x0088ca88` (19 × 4 ASCII bytes), letter LUT `0x0088cad4` (19 bytes).
//!   - Letter Q4 positions `0x0088c8b8` (`19 × 4 × [i16; 2]`), letter UV origins `0x0088c9e8` (`19 × [i16; 2]`).
//!   - Smudge base offset `0x0088ca34` (`(-2949, -2423)` Q4) and 5 smudge rects `0x0088ca40..0x0088ca68`.
//!   - Native texture: `512x512` `pic/htl/maker.tex`.
//! - `FUN_00499b00` (`0x00499b00`, `case 0x36`, *Born From a Wish* Baldwin Mansion board):
//!   - Q4 UV table `0x0088cae8..0x0088cb08` (3 tablet UVs + 1 white board cover UV).
//!   - Q4 screen rects `0x0088cb08` (card) and `0x0088cb10` (board cover).
//!   - Flags: `704` (`DAT_01f7aa38` bit 0) hides cover; `679`/`688`/`697` select base tablet;
//!     `680`/`689`/`698` select first rotated tablet; `681`/`690`/`699` select second rotated tablet;
//!     `(682..=684)`, `(691..=693)`, `(700..=702)` encode 0°/90°/180°/270° rotation per tablet.
//!   - Native texture: `512x512` `pic/add/teien2.tex`.

use sh2_core::events::EventFlags;

use crate::memo::overlays::MemoLayerQuad;

/// Exact 19-byte briefcase letter lookup table (`sh2pc.exe` `0x0088cad4`).
pub const BRIEFCASE_LETTERS: [u8; 19] = *b"acdehiklmnoprstuvwx";

/// Exact 19 × 4-letter Lakeview Hotel briefcase solution words (`sh2pc.exe` `0x0088ca88`).
#[rustfmt::skip]
pub const BRIEFCASE_WORDS: [[u8; 4]; 19] = [
    *b"open", *b"damn", *b"hell", *b"town", *b"dark",
    *b"mama", *b"down", *b"love", *b"lock", *b"mist",
    *b"luck", *b"lose", *b"dose", *b"over", *b"dust",
    *b"time", *b"help", *b"kill", *b"null",
];

/// Exact pixel UV origins `[u0, v0]` in `512x512` `pic/htl/maker.tex` (`sh2pc.exe` `0x0088c9e8`).
/// Each glyph is `48x48` px (`u1 = u0 + 47`, `v1 = v0 + 47`, Q4 extent `+0x300`).
#[rustfmt::skip]
pub const BRIEFCASE_LETTER_UVS: [[i16; 2]; 19] = [
    [336, 16],  [384, 16],  [432, 16],  [336, 64],  [384, 64],
    [432, 64],  [336, 112], [384, 112], [432, 112], [368, 160],
    [416, 160], [464, 160], [0, 288],   [48, 288],  [96, 288],
    [0, 336],   [64, 336],  [0, 384],   [64, 384],
];

/// Exact Q4 top-left positions `[x_q4, y_q4]` for all 19 letters × 4 briefcase word slots
/// (`sh2pc.exe` `0x0088c8b8`).
#[rustfmt::skip]
pub const BRIEFCASE_LETTER_POS_Q4: [[[i16; 2]; 4]; 19] = [
    [[-724, -702], [-420, -582], [-137, -465], [159, -357]], // a
    [[-753, -704], [-463, -599], [-164, -474], [113, -347]], // c
    [[-800, -688], [-501, -570], [-199, -443], [86,  -340]], // d
    [[-733, -696], [-435, -578], [-136, -466], [140, -350]], // e
    [[-758, -696], [-470, -572], [-177, -446], [112, -332]], // h
    [[-802, -691], [-508, -575], [-218, -456], [70,  -332]], // i
    [[-775, -696], [-479, -584], [-188, -456], [110, -341]], // k
    [[-711, -672], [-430, -558], [-136, -430], [147, -313]], // l
    [[-754, -718], [-460, -604], [-173, -478], [121, -355]], // m
    [[-751, -704], [-456, -584], [-167, -454], [124, -337]], // n
    [[-771, -676], [-482, -560], [-193, -448], [97,  -338]], // o
    [[-808, -690], [-514, -568], [-214, -452], [62,  -328]], // p
    [[-783, -704], [-490, -586], [-203, -456], [87,  -347]], // r
    [[-801, -704], [-508, -585], [-230, -466], [75,  -353]], // s
    [[-788, -704], [-500, -583], [-206, -466], [76,  -359]], // t
    [[-807, -700], [-507, -591], [-222, -456], [59,  -350]], // u
    [[-717, -688], [-417, -574], [-135, -454], [146, -331]], // v
    [[-789, -706], [-499, -590], [-208, -462], [84,  -347]], // w
    [[-746, -698], [-448, -584], [-158, -458], [134, -343]], // x
];

/// Exact Q4 base offset (`sh2pc.exe` `0x0088ca34`) for the 5 briefcase ink-smudge overlay quads.
pub const BRIEFCASE_SMUDGE_BASE_Q4: [i16; 2] = [-2949, -2423];

/// Exact pixel rects `[u0, v0, u1, v1]` in `pic/htl/maker.tex` for the 5 briefcase ink smudges
/// (`sh2pc.exe` `0x0088ca40..0x0088ca68`).
#[rustfmt::skip]
pub const BRIEFCASE_SMUDGE_RECTS: [[i16; 4]; 5] = [
    [0,   0,   159, 287],
    [160, 0,   191, 255],
    [192, 0,   335, 223],
    [336, 160, 367, 223],
    [304, 224, 335, 255],
];

/// Exact Q4 UVs `[u0, v0, u1, v1]` (`>> 4` = px in `512x512` `pic/add/teien2.tex`) for the 3 BFAW
/// Baldwin Mansion board tablets (`0..3`) and the bottom cover plate (`3`) (`sh2pc.exe` `0x0088cae8..0x0088cb08`).
#[rustfmt::skip]
pub const TEIEN_UVS_Q4: [[u16; 4]; 4] = [
    [2560, 0,    5104, 3312], // tablet 0 (0x0088cae8)
    [0,    0,    2544, 3312], // tablet 1 (0x0088caf0)
    [0,    3312, 2544, 6640], // tablet 2 (0x0088caf8)
    [2560, 3312, 3056, 4592], // board cover (0x0088cb00)
];

/// Exact Q4 screen bounds `[x0, y0, x1, y1]` for BFAW board tablets (`sh2pc.exe` `0x0088cb08`).
pub const TEIEN_CARD_RECT_Q4: [i16; 4] = [-1285, -2110, 1275, 1218];

/// Exact Q4 screen bounds `[x0, y0, x1, y1]` for BFAW board bottom cover (`sh2pc.exe` `0x0088cb10`).
pub const TEIEN_BOARD_RECT_Q4: [i16; 4] = [-209, 1153, 303, 2433];

/// Exact 1:1 port of `MemoPictureLayerDraw` `case 0x35` (`sh2pc.exe` `0x0049a6a0`, lines 6361–6422):
/// Lakeview Hotel briefcase 4-letter word + 5 ink smudge quads over `pic/htl/p_marker.tex`.
#[must_use]
pub fn build_briefcase_quads(flags: &EventFlags) -> Vec<MemoLayerQuad> {
    let mut word_idx: usize = 0;
    for bit in 0..5usize {
        if flags.get(406 + bit) {
            word_idx |= 1 << bit;
        }
    }
    let word = BRIEFCASE_WORDS[word_idx.min(BRIEFCASE_WORDS.len() - 1)];
    let mut out = Vec::with_capacity(9);

    for (slot, &ch) in word.iter().enumerate() {
        if let Some(letter_idx) = BRIEFCASE_LETTERS.iter().position(|&b| b == ch) {
            let [px0, py0] = BRIEFCASE_LETTER_POS_Q4[letter_idx][slot];
            let [u0, v0] = BRIEFCASE_LETTER_UVS[letter_idx];
            out.push(MemoLayerQuad::from_q4_rect(
                px0,
                py0,
                px0.saturating_add(0x0300),
                py0.saturating_add(0x0300),
                [u0 as f32, v0 as f32, (u0 + 0x2f) as f32, (v0 + 0x2f) as f32],
            ));
        }
    }

    let [bx, by] = BRIEFCASE_SMUDGE_BASE_Q4;
    for [x0, y0, x1, y1] in BRIEFCASE_SMUDGE_RECTS {
        let px0 = x0 * 16 + bx;
        let py0 = y0 * 16 + by;
        let px1 = (x1 + 1) * 16 + bx;
        let py1 = (y1 + 1) * 16 + by;
        out.push(MemoLayerQuad::from_q4_rect(
            px0,
            py0,
            px1,
            py1,
            [x0 as f32, y0 as f32, x1 as f32, y1 as f32],
        ));
    }
    out
}

/// Exact 1:1 port of `FUN_00499b00` (`sh2pc.exe` `0x00499b00`, `case 0x36`):
/// *Born From a Wish* Baldwin Mansion board cover + up to 3 rotated tablet layers over `pic/add/p_teien.tex`.
#[must_use]
pub fn build_teien_quads(flags: &EventFlags) -> Vec<MemoLayerQuad> {
    let mut out = Vec::with_capacity(4);
    let uv_px = |idx: usize| -> [f32; 4] {
        let [u0, v0, u1, v1] = TEIEN_UVS_Q4[idx];
        [
            (u0 as f32) / 16.0,
            (v0 as f32) / 16.0,
            (u1 as f32) / 16.0,
            (v1 as f32) / 16.0,
        ]
    };

    // Board cover plate drawn while flag 704 (DAT_01f7aa38 bit 0) is clear
    if !flags.get(704) {
        let [x0, y0, x1, y1] = TEIEN_BOARD_RECT_Q4;
        out.push(MemoLayerQuad::from_q4_rect(x0, y0, x1, y1, uv_px(3)));
    }

    // Layer 1 (upright base tablet): flags 679 / 688 / 697
    let Some(base_idx) = (if flags.get(679) {
        Some(0usize)
    } else if flags.get(688) {
        Some(1usize)
    } else if flags.get(697) {
        Some(2usize)
    } else {
        None
    }) else {
        return out;
    };
    let [cx0, cy0, cx1, cy1] = TEIEN_CARD_RECT_Q4;
    out.push(MemoLayerQuad::from_q4_rect(
        cx0,
        cy0,
        cx1,
        cy1,
        uv_px(base_idx),
    ));

    // Layer 2 (first rotated tablet): flags 680 / 689 / 698
    let Some(rot1_idx) = (if flags.get(680) {
        Some(0usize)
    } else if flags.get(689) {
        Some(1usize)
    } else if flags.get(698) {
        Some(2usize)
    } else {
        None
    }) else {
        return out;
    };
    out.push(build_teien_rotated_card(rot1_idx, flags, uv_px(rot1_idx)));

    // Layer 3 (second rotated tablet): flags 681 / 690 / 699
    let Some(rot2_idx) = (if flags.get(681) {
        Some(0usize)
    } else if flags.get(690) {
        Some(1usize)
    } else if flags.get(699) {
        Some(2usize)
    } else {
        None
    }) else {
        return out;
    };
    out.push(build_teien_rotated_card(rot2_idx, flags, uv_px(rot2_idx)));
    out
}

/// Builds a 4-corner rotated `TEIEN_CARD_RECT_Q4` quad matching `FUN_00499b00` (`0x00499b00`).
fn build_teien_rotated_card(
    tablet_idx: usize,
    flags: &EventFlags,
    uv_px: [f32; 4],
) -> MemoLayerQuad {
    let [x0, y0, x1, y1] = TEIEN_CARD_RECT_Q4.map(|v| (v as f32) / 16.0);
    let (f_0, f_90, f_180) = match tablet_idx {
        0 => (682, 683, 684), // DAT_01f7aa34 bits 10, 11, 12
        1 => (691, 692, 693), // DAT_01f7aa34 bits 19, 20, 21
        _ => (700, 701, 702), // DAT_01f7aa34 bits 28, 29, 30
    };
    let corners_ps2 = if flags.get(f_0) {
        [(x0, y0), (x1, y0), (x0, y1), (x1, y1)] // 0 deg (LAB_00499f61)
    } else if flags.get(f_90) {
        [(x1, y0), (x1, y1), (x0, y0), (x0, y1)] // 90 deg CW (LAB_00499f93)
    } else if flags.get(f_180) {
        [(x1, y1), (x0, y1), (x1, y0), (x0, y0)] // 180 deg
    } else {
        [(x0, y1), (x0, y0), (x1, y1), (x1, y0)] // 270 deg CW (LAB_00499d30)
    };
    MemoLayerQuad { corners_ps2, uv_px }
}
