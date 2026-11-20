//! Exact 45-entry PS2 Memo Catalog extracted 1:1 from `SLUS_202.28` (`Event/memo.c`).
//!
//! # Ground Truth Citations (`SLUS_202.28`)
//! - `MemoCommandCheck` (`0x00230570`): loops `0..0x2d` (`45` entries), checking `EventFlags`
//!   bit `entry.flag_id` at `0x01126430`.
//! - `memo_list` (`0x0034ced0`, `45 * 16` bytes): exact `flag_id`, `title_msg_idx`, `body_msg_idx`,
//!   and optional inspection backdrop texture (`pic/etc/p_memo.tex` default from `MemoInit` `0x00230704`).

use sh2_core::events::EventFlags;

/// Total number of memos in `SLUS_202.28` `MemoCommandCheck` (`0x2d` = `45` at `0x002305d0`).
pub const MEMO_CATALOG_COUNT: usize = 45;

/// Default memo inspection still backdrop (`data_pic_etc_p_memo_tex` at `0x003a1c60`, loaded in `MemoInit` `0x00230704`).
pub const DEFAULT_MEMO_BACKDROP_TEX: &str = "pic/etc/p_memo.tex";

/// Static metadata for a single collectible memo/letter/diary (`memo_list` @ `0x0034ced0`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoEntry {
    /// Zero-based catalog index (`0..45`) preserving canonical chronological order.
    pub index: usize,
    /// `EventFlags` bit index marking this memo as collected (`0x0034ced0 + index * 16`).
    pub flag_id: u16,
    /// Message index for the memo title in `memo_msg_e.mes` (`memo_x_msg_e.mes`).
    pub title_msg_idx: u16,
    /// Message index for the memo body pages in `memo_msg_e.mes` (`memo_x_msg_e.mes`).
    pub body_msg_idx: u16,
    /// Fallback English title when `memo_msg_e.mes` is not loaded on disk.
    pub fallback_title: &'static str,
    /// Relative `.tex` inspection backdrop path (`MemoPictureLoad` `0x00231390`).
    pub backdrop_tex_path: &'static str,
}

/// All 45 chronological memos from `SLUS_202.28` `0x0034ced0`.
pub const MEMO_CATALOG: [MemoEntry; MEMO_CATALOG_COUNT] = [
    MemoEntry {
        index: 0,
        flag_id: 50,
        title_msg_idx: 38,
        body_msg_idx: 85,
        fallback_title: "Memo on the Ground (Vachss Rd 1)",
        backdrop_tex_path: DEFAULT_MEMO_BACKDROP_TEX,
    },
    MemoEntry {
        index: 1,
        flag_id: 52,
        title_msg_idx: 39,
        body_msg_idx: 86,
        fallback_title: "Memo on the Ground (Vachss Rd 2)",
        backdrop_tex_path: DEFAULT_MEMO_BACKDROP_TEX,
    },
    MemoEntry {
        index: 2,
        flag_id: 54,
        title_msg_idx: 40,
        body_msg_idx: 87,
        fallback_title: "Memo on the Ground (Vachss Rd 3)",
        backdrop_tex_path: DEFAULT_MEMO_BACKDROP_TEX,
    },
    MemoEntry {
        index: 3,
        flag_id: 56,
        title_msg_idx: 41,
        body_msg_idx: 88,
        fallback_title: "Memo on the Ground (Martin St)",
        backdrop_tex_path: DEFAULT_MEMO_BACKDROP_TEX,
    },
    MemoEntry {
        index: 4,
        flag_id: 58,
        title_msg_idx: 42,
        body_msg_idx: 89,
        fallback_title: "Blood-Stained Note (Martin St)",
        backdrop_tex_path: DEFAULT_MEMO_BACKDROP_TEX,
    },
    MemoEntry {
        index: 5,
        flag_id: 610,
        title_msg_idx: 43,
        body_msg_idx: 90,
        fallback_title: "Neely's Bar Map Note",
        backdrop_tex_path: DEFAULT_MEMO_BACKDROP_TEX,
    },
    MemoEntry {
        index: 6,
        flag_id: 614,
        title_msg_idx: 44,
        body_msg_idx: 92,
        fallback_title: "Note Beside the Body (Saul St)",
        backdrop_tex_path: DEFAULT_MEMO_BACKDROP_TEX,
    },
    MemoEntry {
        index: 7,
        flag_id: 615,
        title_msg_idx: 47,
        body_msg_idx: 91,
        fallback_title: "Neely's Bar Wall Message",
        backdrop_tex_path: DEFAULT_MEMO_BACKDROP_TEX,
    },
    MemoEntry {
        index: 8,
        flag_id: 44,
        title_msg_idx: 0,
        body_msg_idx: 49,
        fallback_title: "Mary's Letter",
        backdrop_tex_path: "pic/etc/p_letterm.tex",
    },
    MemoEntry {
        index: 9,
        flag_id: 63,
        title_msg_idx: 1,
        body_msg_idx: 50,
        fallback_title: "Wood Side Apartments Notice",
        backdrop_tex_path: DEFAULT_MEMO_BACKDROP_TEX,
    },
    MemoEntry {
        index: 10,
        flag_id: 64,
        title_msg_idx: 2,
        body_msg_idx: 51,
        fallback_title: "Trash Chute Memo",
        backdrop_tex_path: DEFAULT_MEMO_BACKDROP_TEX,
    },
    MemoEntry {
        index: 11,
        flag_id: 65,
        title_msg_idx: 3,
        body_msg_idx: 52,
        fallback_title: "Clock Scratches (Room 208)",
        backdrop_tex_path: "pic/apt/clock_memo.tex",
    },
    MemoEntry {
        index: 12,
        flag_id: 74,
        title_msg_idx: 4,
        body_msg_idx: 53,
        fallback_title: "Clock Riddle Memo",
        backdrop_tex_path: "pic/apt/clock_memo_2.tex",
    },
    MemoEntry {
        index: 13,
        flag_id: 78,
        title_msg_idx: 5,
        body_msg_idx: 54,
        fallback_title: "Blue Creek Safe Combination",
        backdrop_tex_path: DEFAULT_MEMO_BACKDROP_TEX,
    },
    MemoEntry {
        index: 14,
        flag_id: 75,
        title_msg_idx: 6,
        body_msg_idx: 55,
        fallback_title: "Coin Desk Riddle (Room 105)",
        backdrop_tex_path: DEFAULT_MEMO_BACKDROP_TEX,
    },
    MemoEntry {
        index: 15,
        flag_id: 96,
        title_msg_idx: 7,
        body_msg_idx: 56,
        fallback_title: "Rosewater Park Monument",
        backdrop_tex_path: DEFAULT_MEMO_BACKDROP_TEX,
    },
    MemoEntry {
        index: 16,
        flag_id: 84,
        title_msg_idx: 8,
        body_msg_idx: 57,
        fallback_title: "Pete's Bowl-O-Rama Note",
        backdrop_tex_path: DEFAULT_MEMO_BACKDROP_TEX,
    },
    MemoEntry {
        index: 17,
        flag_id: 102,
        title_msg_idx: 9,
        body_msg_idx: 58,
        fallback_title: "Heaven's Night Flyer",
        backdrop_tex_path: DEFAULT_MEMO_BACKDROP_TEX,
    },
    MemoEntry {
        index: 18,
        flag_id: 111,
        title_msg_idx: 11,
        body_msg_idx: 59,
        fallback_title: "Reception Desk Memo",
        backdrop_tex_path: DEFAULT_MEMO_BACKDROP_TEX,
    },
    MemoEntry {
        index: 19,
        flag_id: 123,
        title_msg_idx: 12,
        body_msg_idx: 60,
        fallback_title: "Doctor's Memo (Brookhaven)",
        backdrop_tex_path: "pic/hsp/p_doctormemo.tex",
    },
    MemoEntry {
        index: 20,
        flag_id: 124,
        title_msg_idx: 13,
        body_msg_idx: 61,
        fallback_title: "Doctor's Key Memo",
        backdrop_tex_path: "pic/hsp/p_doctormemo_key.tex",
    },
    MemoEntry {
        index: 21,
        flag_id: 125,
        title_msg_idx: 14,
        body_msg_idx: 62,
        fallback_title: "Patient Records (Joseph, Jack, Joshua)",
        backdrop_tex_path: DEFAULT_MEMO_BACKDROP_TEX,
    },
    MemoEntry {
        index: 22,
        flag_id: 609,
        title_msg_idx: 15,
        body_msg_idx: 63,
        fallback_title: "Carbon Paper Keypad Memo",
        backdrop_tex_path: DEFAULT_MEMO_BACKDROP_TEX,
    },
    MemoEntry {
        index: 23,
        flag_id: 179,
        title_msg_idx: 16,
        body_msg_idx: 64,
        fallback_title: "Rooftop Diary",
        backdrop_tex_path: DEFAULT_MEMO_BACKDROP_TEX,
    },
    MemoEntry {
        index: 24,
        flag_id: 169,
        title_msg_idx: 17,
        body_msg_idx: 65,
        fallback_title: "Examining Room 3 Memo",
        backdrop_tex_path: DEFAULT_MEMO_BACKDROP_TEX,
    },
    MemoEntry {
        index: 25,
        flag_id: 180,
        title_msg_idx: 18,
        body_msg_idx: 66,
        fallback_title: "Director's Room Memo",
        backdrop_tex_path: DEFAULT_MEMO_BACKDROP_TEX,
    },
    MemoEntry {
        index: 26,
        flag_id: 608,
        title_msg_idx: 48,
        body_msg_idx: 84,
        fallback_title: "Louise's Box Code (M2)",
        backdrop_tex_path: DEFAULT_MEMO_BACKDROP_TEX,
    },
    MemoEntry {
        index: 27,
        flag_id: 183,
        title_msg_idx: 19,
        body_msg_idx: 67,
        fallback_title: "Trick or Treat Quiz",
        backdrop_tex_path: DEFAULT_MEMO_BACKDROP_TEX,
    },
    MemoEntry {
        index: 28,
        flag_id: 613,
        title_msg_idx: 19,
        body_msg_idx: 68,
        fallback_title: "Storeroom Prize Note",
        backdrop_tex_path: DEFAULT_MEMO_BACKDROP_TEX,
    },
    MemoEntry {
        index: 29,
        flag_id: 204,
        title_msg_idx: 20,
        body_msg_idx: 69,
        fallback_title: "Bloody Wall Writing (Hospital)",
        backdrop_tex_path: DEFAULT_MEMO_BACKDROP_TEX,
    },
    MemoEntry {
        index: 30,
        flag_id: 192,
        title_msg_idx: 21,
        body_msg_idx: 70,
        fallback_title: "Wrench Letter (Rosewater Park)",
        backdrop_tex_path: DEFAULT_MEMO_BACKDROP_TEX,
    },
    MemoEntry {
        index: 31,
        flag_id: 191,
        title_msg_idx: 22,
        body_msg_idx: 71,
        fallback_title: "Historical Society Plaque",
        backdrop_tex_path: DEFAULT_MEMO_BACKDROP_TEX,
    },
    MemoEntry {
        index: 32,
        flag_id: 231,
        title_msg_idx: 23,
        body_msg_idx: 72,
        fallback_title: "Toluca Prison Tablet Inscription",
        backdrop_tex_path: DEFAULT_MEMO_BACKDROP_TEX,
    },
    MemoEntry {
        index: 33,
        flag_id: 242,
        title_msg_idx: 24,
        body_msg_idx: 73,
        fallback_title: "Prisoner's Diary",
        backdrop_tex_path: DEFAULT_MEMO_BACKDROP_TEX,
    },
    MemoEntry {
        index: 34,
        flag_id: 263,
        title_msg_idx: 26,
        body_msg_idx: 74,
        fallback_title: "Gallows Poem (Toluca Prison)",
        backdrop_tex_path: DEFAULT_MEMO_BACKDROP_TEX,
    },
    MemoEntry {
        index: 35,
        flag_id: 262,
        title_msg_idx: 27,
        body_msg_idx: 83,
        fallback_title: "Labyrinth Wire-Cutter Note",
        backdrop_tex_path: DEFAULT_MEMO_BACKDROP_TEX,
    },
    MemoEntry {
        index: 36,
        flag_id: 287,
        title_msg_idx: 28,
        body_msg_idx: 75,
        fallback_title: "Angela's Newspaper Clipping",
        backdrop_tex_path: DEFAULT_MEMO_BACKDROP_TEX,
    },
    MemoEntry {
        index: 37,
        flag_id: 279,
        title_msg_idx: 29,
        body_msg_idx: 76,
        fallback_title: "Six Hanged Men Riddle",
        backdrop_tex_path: DEFAULT_MEMO_BACKDROP_TEX,
    },
    MemoEntry {
        index: 38,
        flag_id: 331,
        title_msg_idx: 30,
        body_msg_idx: 77,
        fallback_title: "Lakeview Reception Note",
        backdrop_tex_path: DEFAULT_MEMO_BACKDROP_TEX,
    },
    MemoEntry {
        index: 39,
        flag_id: 361,
        title_msg_idx: 31,
        body_msg_idx: 78,
        fallback_title: "Briefcase Clues (Room 204)",
        backdrop_tex_path: DEFAULT_MEMO_BACKDROP_TEX,
    },
    MemoEntry {
        index: 40,
        flag_id: 362,
        title_msg_idx: 32,
        body_msg_idx: 79,
        fallback_title: "Employee Elevator Notice",
        backdrop_tex_path: DEFAULT_MEMO_BACKDROP_TEX,
    },
    MemoEntry {
        index: 41,
        flag_id: 363,
        title_msg_idx: 33,
        body_msg_idx: 80,
        fallback_title: "Music Box Poem (Lobby)",
        backdrop_tex_path: DEFAULT_MEMO_BACKDROP_TEX,
    },
    MemoEntry {
        index: 42,
        flag_id: 612,
        title_msg_idx: 34,
        body_msg_idx: 81,
        fallback_title: "Laura's Letter",
        backdrop_tex_path: "pic/etc/p_laura_letter.tex",
    },
    MemoEntry {
        index: 43,
        flag_id: 390,
        title_msg_idx: 35,
        body_msg_idx: 82,
        fallback_title: "Hotel Manager's Memo",
        backdrop_tex_path: DEFAULT_MEMO_BACKDROP_TEX,
    },
    MemoEntry {
        index: 44,
        flag_id: 412,
        title_msg_idx: 36,
        body_msg_idx: 0,
        fallback_title: "Book: Lost Memories",
        backdrop_tex_path: "pic/etc/p_lostmemory.tex",
    },
];

/// Exact 1:1 port of `MemoCommandCheck` (`SLUS_202.28` `0x00230570`).
///
/// Returns `true` if the player has collected at least one memo (`EventFlags` bit set).
pub fn memo_command_check(flags: &EventFlags) -> bool {
    MEMO_CATALOG
        .iter()
        .any(|entry| flags.get(entry.flag_id as usize))
}

/// Returns all collected memos in canonical chronological order (`MEMO_CATALOG` `0..45`).
pub fn collected_memos(flags: &EventFlags) -> Vec<&'static MemoEntry> {
    MEMO_CATALOG
        .iter()
        .filter(|entry| flags.get(entry.flag_id as usize))
        .collect()
}
