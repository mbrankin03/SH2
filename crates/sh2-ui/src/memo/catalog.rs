//! Exact 57-entry retail Director's Cut Memo Catalog extracted 1:1 from `sh2pc.exe`
//! (`0x0088c378`, cross-checked against `SLUS_202.28` `0x0034ced0` 45-entry prototype).
//!
//! # Prototype vs. Retail Drift
//! - `SLUS_202.28` (`0x0034ced0`) has 45 entries (`0..=44`), with entries `39..=44` pointing
//!   to `data/pic/etc/comingsoon.tex` placeholders.
//! - Retail `sh2pc.exe` (`0x0088c378`) expands the catalog to 57 entries (`0..=56`), replacing
//!   every placeholder with its final retail `.tex` and adding the *Born From a Wish* memos
//!   (`54..=56`), matching `data/etc/message/memo_x_msg_e.mes` on disk.

use sh2_core::events::EventFlags;

/// Total number of memos in retail `sh2pc.exe` `memo_list` (`0x0088c378`, `57` entries).
pub const MEMO_CATALOG_COUNT: usize = 57;

/// Default memo inspection still backdrop (`data_pic_etc_p_memo_tex` at `0x003a1c60`, loaded in `MemoInit` `0x00230704`).
pub const DEFAULT_MEMO_BACKDROP_TEX: &str = "pic/etc/p_memo.tex";

/// Alternate statue box texture loaded for memo `0x24` when flag `257` is clear and flag `260` is set
/// (`sh2pc.exe` `MemoPictureLoad` `0x00499000` line 5603).
pub const STATUE_BOX_TEX: &str = "pic/out/statuebox.tex";

/// 3-mode presentation style stored at `memo_list[i] + 0x02` (`MemoDisplay` `0x00230c70` / `0x0049a750`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoDisplayMode {
    /// Mode `0`: Full-brightness image (`0x80`) only; Confirm/Cancel returns to list.
    ImageOnly = 0,
    /// Mode `1`: Dimmed backdrop (`0x40`) with immediate `.mes` text.
    TextImmediate = 1,
    /// Mode `2`: Full-brightness image (`0x80`) first; Confirm dims `0x80 -> 0x40` at `1.0/s` and shows text.
    ImageThenText = 2,
}

/// Static metadata for a single collectible memo/letter/diary (`sh2pc.exe` `0x0088c378`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoEntry {
    /// Zero-based catalog index (`0..57`) preserving canonical chronological order.
    pub index: usize,
    /// `EventFlags` bit index marking this memo as collected (`+0x00`).
    pub flag_id: u16,
    /// 3-mode display behavior (`+0x02`).
    pub display_mode: MemoDisplayMode,
    /// Message index for the memo title in `memo_x_msg_e.mes` (`+0x04`).
    pub title_msg_idx: u16,
    /// Message index for the memo body pages in `memo_x_msg_e.mes` (`+0x06`).
    pub body_msg_idx: u16,
    /// Relative `.tex` inspection backdrop path (`+0x08`), or `None` to keep `p_memo.tex`.
    pub backdrop_tex_path: Option<&'static str>,
    /// Relative `.tex` puzzle overlay layer path (`+0x0c`), or `None`.
    pub layer_tex_path: Option<&'static str>,
}

/// All 57 chronological memos extracted verbatim from `sh2pc.exe` `0x0088c378`.
#[rustfmt::skip]
pub const MEMO_CATALOG: [MemoEntry; MEMO_CATALOG_COUNT] = [
    MemoEntry { index:  0, flag_id:  50, display_mode: MemoDisplayMode::TextImmediate, title_msg_idx:  38, body_msg_idx:  85, backdrop_tex_path: None, layer_tex_path: None },
    MemoEntry { index:  1, flag_id:  52, display_mode: MemoDisplayMode::TextImmediate, title_msg_idx:  39, body_msg_idx:  86, backdrop_tex_path: None, layer_tex_path: None },
    MemoEntry { index:  2, flag_id:  54, display_mode: MemoDisplayMode::TextImmediate, title_msg_idx:  40, body_msg_idx:  87, backdrop_tex_path: None, layer_tex_path: None },
    MemoEntry { index:  3, flag_id:  56, display_mode: MemoDisplayMode::TextImmediate, title_msg_idx:  41, body_msg_idx:  88, backdrop_tex_path: None, layer_tex_path: None },
    MemoEntry { index:  4, flag_id:  58, display_mode: MemoDisplayMode::TextImmediate, title_msg_idx:  42, body_msg_idx:  89, backdrop_tex_path: None, layer_tex_path: None },
    MemoEntry { index:  5, flag_id: 610, display_mode: MemoDisplayMode::TextImmediate, title_msg_idx:  43, body_msg_idx:  90, backdrop_tex_path: None, layer_tex_path: None },
    MemoEntry { index:  6, flag_id:  44, display_mode: MemoDisplayMode::TextImmediate, title_msg_idx:   0, body_msg_idx:  49, backdrop_tex_path: Some("pic/out/p_swamp.tex"), layer_tex_path: None },
    MemoEntry { index:  7, flag_id: 644, display_mode: MemoDisplayMode::ImageOnly    , title_msg_idx: 105, body_msg_idx:   0, backdrop_tex_path: Some("pic/out/p_outmape.tex"), layer_tex_path: None },
    MemoEntry { index:  8, flag_id: 614, display_mode: MemoDisplayMode::TextImmediate, title_msg_idx:  44, body_msg_idx:  92, backdrop_tex_path: None, layer_tex_path: None },
    MemoEntry { index:  9, flag_id: 615, display_mode: MemoDisplayMode::TextImmediate, title_msg_idx:  47, body_msg_idx:  91, backdrop_tex_path: None, layer_tex_path: None },
    MemoEntry { index: 10, flag_id:  63, display_mode: MemoDisplayMode::TextImmediate, title_msg_idx:   1, body_msg_idx:  50, backdrop_tex_path: Some("pic/apt/p_endhint.tex"), layer_tex_path: None },
    MemoEntry { index: 11, flag_id:  64, display_mode: MemoDisplayMode::TextImmediate, title_msg_idx:   2, body_msg_idx:  51, backdrop_tex_path: Some("pic/apt/p_endhint.tex"), layer_tex_path: None },
    MemoEntry { index: 12, flag_id:  65, display_mode: MemoDisplayMode::TextImmediate, title_msg_idx:   3, body_msg_idx:  52, backdrop_tex_path: Some("pic/apt/p_endhint.tex"), layer_tex_path: None },
    MemoEntry { index: 13, flag_id:  74, display_mode: MemoDisplayMode::ImageThenText, title_msg_idx:   4, body_msg_idx:  53, backdrop_tex_path: Some("pic/apt/clock_name.tex"), layer_tex_path: None },
    MemoEntry { index: 14, flag_id:  78, display_mode: MemoDisplayMode::TextImmediate, title_msg_idx:   5, body_msg_idx:  54, backdrop_tex_path: Some("pic/apt/clock_memo.tex"), layer_tex_path: None },
    MemoEntry { index: 15, flag_id:  75, display_mode: MemoDisplayMode::TextImmediate, title_msg_idx:   6, body_msg_idx:  55, backdrop_tex_path: Some("pic/apt/clock_memo.tex"), layer_tex_path: None },
    MemoEntry { index: 16, flag_id:  96, display_mode: MemoDisplayMode::TextImmediate, title_msg_idx:   7, body_msg_idx:  56, backdrop_tex_path: Some("pic/apt/p_dust_out.tex"), layer_tex_path: None },
    MemoEntry { index: 17, flag_id:  84, display_mode: MemoDisplayMode::TextImmediate, title_msg_idx:   8, body_msg_idx:  57, backdrop_tex_path: Some("pic/apt/p_tourist.tex"), layer_tex_path: None },
    MemoEntry { index: 18, flag_id: 102, display_mode: MemoDisplayMode::TextImmediate, title_msg_idx:   9, body_msg_idx:  58, backdrop_tex_path: Some("pic/apt/p_safe_close.tex"), layer_tex_path: Some("pic/apt/p_safe_close2.tex") },
    MemoEntry { index: 19, flag_id: 111, display_mode: MemoDisplayMode::TextImmediate, title_msg_idx:  11, body_msg_idx:  59, backdrop_tex_path: None, layer_tex_path: None },
    MemoEntry { index: 20, flag_id: 123, display_mode: MemoDisplayMode::TextImmediate, title_msg_idx:  12, body_msg_idx:  60, backdrop_tex_path: Some("pic/apt/p_desk_hint.tex"), layer_tex_path: None },
    MemoEntry { index: 21, flag_id: 124, display_mode: MemoDisplayMode::TextImmediate, title_msg_idx:  13, body_msg_idx:  61, backdrop_tex_path: Some("pic/apt/p_desk_hint.tex"), layer_tex_path: None },
    MemoEntry { index: 22, flag_id: 125, display_mode: MemoDisplayMode::TextImmediate, title_msg_idx:  14, body_msg_idx:  62, backdrop_tex_path: Some("pic/apt/p_desk_hint.tex"), layer_tex_path: None },
    MemoEntry { index: 23, flag_id: 609, display_mode: MemoDisplayMode::TextImmediate, title_msg_idx:  15, body_msg_idx:  63, backdrop_tex_path: Some("pic/apt/p_desk_hint.tex"), layer_tex_path: None },
    MemoEntry { index: 24, flag_id: 661, display_mode: MemoDisplayMode::ImageOnly    , title_msg_idx: 106, body_msg_idx:   0, backdrop_tex_path: Some("pic/out/p_outmapw.tex"), layer_tex_path: None },
    MemoEntry { index: 25, flag_id: 179, display_mode: MemoDisplayMode::TextImmediate, title_msg_idx:  16, body_msg_idx:  64, backdrop_tex_path: Some("pic/hsp/p_patient.tex"), layer_tex_path: None },
    MemoEntry { index: 26, flag_id: 169, display_mode: MemoDisplayMode::TextImmediate, title_msg_idx:  17, body_msg_idx:  65, backdrop_tex_path: Some("pic/hsp/p_doctormemo.tex"), layer_tex_path: None },
    MemoEntry { index: 27, flag_id: 180, display_mode: MemoDisplayMode::TextImmediate, title_msg_idx:  18, body_msg_idx:  66, backdrop_tex_path: Some("pic/hsp/whiteboard.tex"), layer_tex_path: None },
    MemoEntry { index: 28, flag_id: 608, display_mode: MemoDisplayMode::TextImmediate, title_msg_idx:  48, body_msg_idx:  84, backdrop_tex_path: None, layer_tex_path: None },
    MemoEntry { index: 29, flag_id: 183, display_mode: MemoDisplayMode::TextImmediate, title_msg_idx:  19, body_msg_idx:  67, backdrop_tex_path: Some("pic/hsp/carbon.tex"), layer_tex_path: None },
    MemoEntry { index: 30, flag_id: 613, display_mode: MemoDisplayMode::TextImmediate, title_msg_idx:  19, body_msg_idx:  68, backdrop_tex_path: Some("pic/hsp/p_carbon.tex"), layer_tex_path: None },
    MemoEntry { index: 31, flag_id: 204, display_mode: MemoDisplayMode::TextImmediate, title_msg_idx:  20, body_msg_idx:  69, backdrop_tex_path: Some("pic/hsp/p_diary.tex"), layer_tex_path: None },
    MemoEntry { index: 32, flag_id: 192, display_mode: MemoDisplayMode::ImageThenText, title_msg_idx:  21, body_msg_idx:  70, backdrop_tex_path: Some("pic/hsp/p_boxnumber.tex"), layer_tex_path: Some("pic/hsp/p_boxnumber_2.tex") },
    MemoEntry { index: 33, flag_id: 191, display_mode: MemoDisplayMode::TextImmediate, title_msg_idx:  22, body_msg_idx:  71, backdrop_tex_path: Some("pic/hsp/p_hair.tex"), layer_tex_path: Some("pic/hsp/p_hair_hair.tex") },
    MemoEntry { index: 34, flag_id: 651, display_mode: MemoDisplayMode::TextImmediate, title_msg_idx:  97, body_msg_idx:  98, backdrop_tex_path: None, layer_tex_path: None },
    MemoEntry { index: 35, flag_id: 231, display_mode: MemoDisplayMode::TextImmediate, title_msg_idx:  23, body_msg_idx:  72, backdrop_tex_path: Some("pic/hsp/p_female.tex"), layer_tex_path: Some("pic/hsp/p_female_ring.tex") },
    MemoEntry { index: 36, flag_id: 242, display_mode: MemoDisplayMode::TextImmediate, title_msg_idx:  24, body_msg_idx:  73, backdrop_tex_path: Some("pic/out/statuekey.tex"), layer_tex_path: None },
    MemoEntry { index: 37, flag_id: 218, display_mode: MemoDisplayMode::ImageThenText, title_msg_idx:  93, body_msg_idx:  95, backdrop_tex_path: Some("pic/hsp/p_musiuminfo.tex"), layer_tex_path: None },
    MemoEntry { index: 38, flag_id: 650, display_mode: MemoDisplayMode::ImageThenText, title_msg_idx:  94, body_msg_idx:  96, backdrop_tex_path: Some("pic/out/p_spana.tex"), layer_tex_path: None },
    MemoEntry { index: 39, flag_id: 263, display_mode: MemoDisplayMode::TextImmediate, title_msg_idx:  26, body_msg_idx:  74, backdrop_tex_path: None, layer_tex_path: None },
    MemoEntry { index: 40, flag_id: 262, display_mode: MemoDisplayMode::ImageThenText, title_msg_idx:  27, body_msg_idx:  83, backdrop_tex_path: Some("pic/dls/p_sankaku.tex"), layer_tex_path: None },
    MemoEntry { index: 41, flag_id: 287, display_mode: MemoDisplayMode::ImageThenText, title_msg_idx:  28, body_msg_idx:  75, backdrop_tex_path: Some("pic/dls/p_tablet.tex"), layer_tex_path: Some("pic/dls/p_tablet_plate.tex") },
    MemoEntry { index: 42, flag_id: 279, display_mode: MemoDisplayMode::TextImmediate, title_msg_idx:  29, body_msg_idx:  76, backdrop_tex_path: Some("pic/dls/p_magazin.tex"), layer_tex_path: None },
    MemoEntry { index: 43, flag_id: 642, display_mode: MemoDisplayMode::TextImmediate, title_msg_idx: 107, body_msg_idx: 108, backdrop_tex_path: None, layer_tex_path: None },
    MemoEntry { index: 44, flag_id: 645, display_mode: MemoDisplayMode::ImageThenText, title_msg_idx:  99, body_msg_idx: 102, backdrop_tex_path: Some("pic/dls/p_fireman.tex"), layer_tex_path: None },
    MemoEntry { index: 45, flag_id: 646, display_mode: MemoDisplayMode::ImageThenText, title_msg_idx: 100, body_msg_idx: 103, backdrop_tex_path: Some("pic/dls/p_flingwoman.tex"), layer_tex_path: None },
    MemoEntry { index: 46, flag_id: 647, display_mode: MemoDisplayMode::ImageThenText, title_msg_idx: 101, body_msg_idx: 104, backdrop_tex_path: Some("pic/dls/p_436.tex"), layer_tex_path: None },
    MemoEntry { index: 47, flag_id: 331, display_mode: MemoDisplayMode::TextImmediate, title_msg_idx:  30, body_msg_idx:  77, backdrop_tex_path: Some("pic/dls/p_newspaper.tex"), layer_tex_path: None },
    MemoEntry { index: 48, flag_id: 361, display_mode: MemoDisplayMode::TextImmediate, title_msg_idx:  31, body_msg_idx:  78, backdrop_tex_path: Some("pic/dls/p_hanging_easy.tex"), layer_tex_path: None },
    MemoEntry { index: 49, flag_id: 362, display_mode: MemoDisplayMode::TextImmediate, title_msg_idx:  32, body_msg_idx:  79, backdrop_tex_path: Some("pic/dls/p_hanging_normal.tex"), layer_tex_path: None },
    MemoEntry { index: 50, flag_id: 363, display_mode: MemoDisplayMode::TextImmediate, title_msg_idx:  33, body_msg_idx:  80, backdrop_tex_path: Some("pic/dls/p_hanging_hard.tex"), layer_tex_path: None },
    MemoEntry { index: 51, flag_id: 612, display_mode: MemoDisplayMode::TextImmediate, title_msg_idx:  34, body_msg_idx:  81, backdrop_tex_path: Some("pic/dls/p_hanging_extra.tex"), layer_tex_path: None },
    MemoEntry { index: 52, flag_id: 390, display_mode: MemoDisplayMode::TextImmediate, title_msg_idx:  35, body_msg_idx:  82, backdrop_tex_path: Some("pic/htl/p_receipt.tex"), layer_tex_path: None },
    MemoEntry { index: 53, flag_id: 412, display_mode: MemoDisplayMode::ImageOnly    , title_msg_idx:  36, body_msg_idx:   0, backdrop_tex_path: Some("pic/htl/p_marker.tex"), layer_tex_path: Some("pic/htl/maker.tex") },
    MemoEntry { index: 54, flag_id: 676, display_mode: MemoDisplayMode::ImageThenText, title_msg_idx: 109, body_msg_idx: 112, backdrop_tex_path: Some("pic/add/p_teien.tex"), layer_tex_path: Some("pic/add/teien2.tex") },
    MemoEntry { index: 55, flag_id: 715, display_mode: MemoDisplayMode::TextImmediate, title_msg_idx: 110, body_msg_idx: 113, backdrop_tex_path: Some("pic/add/p_zukan.tex"), layer_tex_path: None },
    MemoEntry { index: 56, flag_id: 719, display_mode: MemoDisplayMode::TextImmediate, title_msg_idx: 111, body_msg_idx: 114, backdrop_tex_path: Some("pic/add/p_lostbook.tex"), layer_tex_path: None },
];

/// Exact 1:1 port of `MemoPictureLoad` (`SLUS_202.28` `0x00231390` / `sh2pc.exe` `0x00499000`)
/// flag-gated texture suppression and alternate backdrop selection.
///
/// Returns `(backdrop_tex_path, layer_tex_path)` to load for `entry` given `flags`:
/// - Entry `0x12` (SafeLock, SLUS `0x11`): suppresses both if flag `607` (`DAT_01126478` bit 31) is clear.
/// - Entry `0x21` (Hair, SLUS `0x1f`): suppresses both if flag `171` (`DAT_01126444` bit 11) is clear.
/// - Entry `0x23` (AngelRing, SLUS `0x20`): suppresses both if neither flag `227` nor `228` is set.
/// - Entry `0x24` (Statue Key/Box, retail `sh2pc.exe`): if flag `257` is clear, swaps to `statuebox.tex`
///   when flag `260` is set or suppresses the backdrop otherwise.
#[must_use]
pub fn resolve_memo_textures(
    entry: &MemoEntry,
    flags: &EventFlags,
) -> (&'static str, Option<&'static str>) {
    match entry.index {
        0x12 if !flags.get(607) => return (DEFAULT_MEMO_BACKDROP_TEX, None),
        0x21 if !flags.get(171) => return (DEFAULT_MEMO_BACKDROP_TEX, None),
        0x23 if !flags.get(227) && !flags.get(228) => return (DEFAULT_MEMO_BACKDROP_TEX, None),
        0x24 if !flags.get(257) => {
            let base = if flags.get(260) {
                STATUE_BOX_TEX
            } else {
                DEFAULT_MEMO_BACKDROP_TEX
            };
            return (base, entry.layer_tex_path);
        }
        _ => {}
    }
    (
        entry.backdrop_tex_path.unwrap_or(DEFAULT_MEMO_BACKDROP_TEX),
        entry.layer_tex_path,
    )
}

/// Exact 1:1 port of `MemoCommandCheck` (`SLUS_202.28` `0x00230570` / `sh2pc.exe` `0x00498de0`).
///
/// Returns `true` if the player has collected at least one memo (`EventFlags` bit set).
#[must_use]
pub fn memo_command_check(flags: &EventFlags) -> bool {
    MEMO_CATALOG.iter().any(|e| flags.get(e.flag_id as usize))
}

/// Returns the ordered list of all collected memos in canonical chronological order (`MemoSelect` `0x00230810`).
#[must_use]
pub fn collected_memos(flags: &EventFlags) -> Vec<&'static MemoEntry> {
    MEMO_CATALOG
        .iter()
        .filter(|e| flags.get(e.flag_id as usize))
        .collect()
}
