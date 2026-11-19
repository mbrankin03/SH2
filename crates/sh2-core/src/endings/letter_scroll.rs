//! Authentic Mary Epilogue Letter `.mes` Tables, Binary Parser & Upward Scroll Engine (`0x00482f60..0x004832e4`).
//!
//! Verified 1:1 against retail `sh2pc.exe` and `data/etc/message/letter*_msg_e.mes` + `data/movie/*.bik`:
//! - Leave (`0x0059f75b..0x0059f866`): `letter_msg_e.mes` (`35` entries), `0.8` s delay (`0x0069f3ec`), `0xEAE1` (`voice.afs[129]`, `300.000` s).
//! - Maria (`0x005a0f3e..0x005a10c8`): `letter_w1_msg_e.mes` (`33` entries) / `letter_msg_e.mes`, `0.0` s delay (`0x0062eddc`), `0xEA64` (`voice.afs[4]`, `351.02133` s).
//! - In Water (`0x0043ed62..0x0043f1c0`): `36.0` s (`1080.0` ticks @ 30 Hz `0x006388a4`) bedside scene -> `water.bik` (`290.290` s) ->
//!   `25.0` s (`0x0063889c`) closing scroll (`letter_w2_msg_e.mes` / `letter_w0_msg_e.mes`) triggering `0xEAB2` (`voice.afs[82]`, `5.65812` s) at `15.0` s (`0x006315e0`).

/// Initial Y coordinate (`0x200` = `512` px) of entry `0` in `0x00482fc0` (`movw $0x200, (%esi)` at `0x0048303b`).
pub const LETTER_SCROLL_INITIAL_Y_PX: i32 = 512;
/// Font `0` line height (`30` px at `0x0080389a`, selected via `0x0047eee0` at `0x00482f98`, multiplied at `0x0047f353`).
pub const LETTER_SCROLL_FONT_LINE_HEIGHT_PX: i32 = 30;
/// Base vertical gap (`+2` px) appended after every `.mes` entry (`add $0x2` at `0x0048304c` & `0x004831b7`).
pub const LETTER_SCROLL_ENTRY_GAP_PX: i32 = 2;
/// Upward letter scroll speed (`16.5` px/s at `0x006388a0`, applied at `0x0059f85c`, `0x005a10b9`, `0x0043f084`).
pub const LETTER_SCROLL_SPEED_PX_PER_SEC: f32 = 16.5;

/// Delay (`0.8` s at `0x0069f3ec`) before Leave triggers `SE_LEAVE_LETTER_VOICEOVER` (`0x0059f817`).
pub const LEAVE_LETTER_VOICE_DELAY_SECS: f32 = 0.8;
/// Delay (`0.0` s at `0x0062eddc`) before Maria triggers `SE_MARIA_LETTER_VOICEOVER` (`0x005a1072`).
pub const MARIA_LETTER_VOICE_DELAY_SECS: f32 = 0.0;
/// Duration (`36.0` s = `1080.0` ticks `[0x006388a4]` / `30.0` Hz `[0x006388a8]`) of In Water's bedside scene (`0x0043ee91..0x0043ef63`).
pub const IN_WATER_BEDSIDE_DURATION_SECS: f32 = 36.0;
/// Duration (`25.0` s at `0x0063889c`) of In Water's post-`water.bik` closing letter scroll (`0x0043f0ee`).
pub const IN_WATER_CLOSING_SCROLL_DURATION_SECS: f32 = 25.0;
/// Timestamp (`15.0` s at `0x006315e0`) in In Water's closing scroll when `SE_IN_WATER_DROWNING_STREAM` fires (`0x0043f0b4`).
pub const IN_WATER_DROWNING_STREAM_TRIGGER_SECS: f32 = 15.0;

/// Leave letter `voice.afs` slot index (`129`, `13,230,000` samples @ `44,100` Hz = `300.000` s).
pub const LEAVE_LETTER_VOICE_AFS_INDEX: u16 = 129;
/// Leave letter voice stream ID (`0xEA60 + 129` = `0xEAE1` = `60129`, pushed at `0x0059f83c`).
pub const SE_LEAVE_LETTER_VOICEOVER: u32 = 0xEAE1;
/// Exact sample duration of `voice.afs[129]` (`13_230_000.0 / 44_100.0` = `300.0` s).
pub const LEAVE_LETTER_VOICE_DURATION_SECS: f32 = 13_230_000.0 / 44_100.0;

/// Maria letter `voice.afs` slot index (`4`, `15,480,040` samples @ `44,100` Hz = `351.02133` s).
pub const MARIA_LETTER_VOICE_AFS_INDEX: u16 = 4;
/// Maria letter voice stream ID (`0xEA60 + 4` = `0xEA64` = `60004`, pushed at `0x005a1097`).
pub const SE_MARIA_LETTER_VOICEOVER: u32 = 0xEA64;
/// Exact sample duration of `voice.afs[4]` (`15_480_040.0 / 44_100.0` = `351.02133` s).
pub const MARIA_LETTER_VOICE_DURATION_SECS: f32 = 15_480_040.0 / 44_100.0;

/// In Water closing line `voice.afs` slot index (`82`, `249,523` samples @ `44,100` Hz = `5.658118` s).
pub const IN_WATER_DROWNING_VOICE_AFS_INDEX: u16 = 82;
/// In Water closing voice stream ID (`0xEA60 + 82` = `0xEAB2` = `60082`, pushed at `0x0043f0d1`).
pub const SE_IN_WATER_DROWNING_STREAM: u32 = 0xEAB2;
/// Exact sample duration of `voice.afs[82]` (`249_523.0 / 44_100.0` = `5.658118` s).
pub const IN_WATER_DROWNING_VOICE_DURATION_SECS: f32 = 249_523.0 / 44_100.0;

/// Retail `data/movie/credits.bik` duration (`2788` frames @ `2997 / 100` fps = `93.02636` s).
pub const CREDITS_BIK_DURATION_SECS: f32 = 2788.0 * 100.0 / 2997.0;
/// Retail `data/movie/end.bik` duration (`6900` frames @ `30000 / 1001` fps = `230.230` s; tracks `0..=3` mapped at `0x0043d754..0x0043d809`).
pub const END_BIK_DURATION_SECS: f32 = 6900.0 * 1001.0 / 30_000.0;
/// Retail `data/movie/end_dog.bik` duration (`4169` frames @ `2501400 / 83483` fps = `139.13834` s).
pub const END_DOG_BIK_DURATION_SECS: f32 = 4169.0 * 83_483.0 / 2_501_400.0;
/// Retail `data/movie/end_wish.bik` duration (`5609` frames @ `420675 / 14039` fps = `187.18668` s).
pub const END_WISH_BIK_DURATION_SECS: f32 = 5609.0 * 14_039.0 / 420_675.0;
/// Retail `data/movie/water.bik` duration (`8700` frames @ `30000 / 1001` fps = `290.290` s).
pub const WATER_BIK_DURATION_SECS: f32 = 8700.0 * 1001.0 / 30_000.0;

/// Static retail `.mes` entry in Mary's epilogue letter (`0x00483067..0x004831c8`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MaryLetterScrollEntry {
    /// Leading `0xf8XX` / `0xf9XX` vertical spacing offset in pixels (`0..=511`, decoded at `0x00483104..0x0048317e`).
    pub f8_spacing_px: u16,
    /// Complete English stanza text (`0xFF 0xFD` line breaks preserved verbatim).
    pub text: &'static str,
}

impl MaryLetterScrollEntry {
    /// Constructs a static `.mes` scroll entry.
    pub const fn new(f8_spacing_px: u16, text: &'static str) -> Self {
        Self {
            f8_spacing_px,
            text,
        }
    }

    /// Number of rendered text lines (`1 + count('\n')`, matching `0x0047f339..0x0047f353`).
    pub fn line_count(self) -> i32 {
        if self.text.is_empty() {
            0
        } else {
            1 + self.text.bytes().filter(|&b| b == b'\n').count() as i32
        }
    }

    /// Pixel block height (`line_count * 30` at `0x0047f353`).
    pub fn height_px(self) -> i32 {
        self.line_count() * LETTER_SCROLL_FONT_LINE_HEIGHT_PX
    }
}

/// Complete 35-entry retail `data/etc/message/letter_msg_e.mes` table (*Leave* / *Maria* full letter).
pub const LETTER_MSG_E_ENTRIES: [MaryLetterScrollEntry; 35] = [
    MaryLetterScrollEntry::new(0, "In my restless dreams,\nI see that town."),
    MaryLetterScrollEntry::new(25, "Silent Hill."),
    MaryLetterScrollEntry::new(35, "You promised you'd take me\nthere again someday.\nBut you never did."),
    MaryLetterScrollEntry::new(25, "Well I'm alone there now..."),
    MaryLetterScrollEntry::new(25, "In our \"special place\"...\nWaiting for you..."),
    MaryLetterScrollEntry::new(108, "Waiting for you to\ncome to see me."),
    MaryLetterScrollEntry::new(35, "But you never do."),
    MaryLetterScrollEntry::new(26, "And so I wait, wrapped in my\ncocoon of pain and loneliness."),
    MaryLetterScrollEntry::new(18, "I know I've done a terrible\nthing to you. Something you'll\nnever forgive me for."),
    MaryLetterScrollEntry::new(44, "I wish I could change \nthat, but I can't."),
    MaryLetterScrollEntry::new(27, "I feel so pathetic and ugly\nlaying here, waiting for you..."),
    MaryLetterScrollEntry::new(27, "Every day I stare up at the cracks\nin the ceiling and all I can think\nabout is how unfair it all is..."),
    MaryLetterScrollEntry::new(192, "The doctor came today.\nHe told me I could go\nhome for a short stay."),
    MaryLetterScrollEntry::new(33, "It's not that I'm getting better.\nIt's just that this may be\nmy last chance..."),
    MaryLetterScrollEntry::new(50, "I think you know what I mean..."),
    MaryLetterScrollEntry::new(100, "Even so, I'm glad to be coming\nhome. I've missed you terribly."),
    MaryLetterScrollEntry::new(25, "But I'm afraid James.\nI'm afraid you don't really\nwant me to come home."),
    MaryLetterScrollEntry::new(120, "Whenever you come see me,\nI can tell how hard it is on you..."),
    MaryLetterScrollEntry::new(40, "I don't know if you\nhate me or pity me...\nOr maybe I just disgust you...."),
    MaryLetterScrollEntry::new(45, "I'm sorry about that."),
    MaryLetterScrollEntry::new(35, "When I first learned that\nI was going to die, I just\ndidn't want to accept it."),
    MaryLetterScrollEntry::new(40, "I was so angry all the time and I\nstruck out at everyone I loved most.\nEspecially you, James."),
    MaryLetterScrollEntry::new(80, "That's why I understand\nif you do hate me."),
    MaryLetterScrollEntry::new(80, "But I want you to\nknow this, James."),
    MaryLetterScrollEntry::new(175, "I'll always love you."),
    MaryLetterScrollEntry::new(134, "Even though our life together had\nto end like this, I still wouldn't\ntrade it for the world. We had\nsome wonderful years together."),
    MaryLetterScrollEntry::new(121, "Well this letter has gone on\ntoo long so I'll say goodbye."),
    MaryLetterScrollEntry::new(40, "I told the nurse to give\nthis to you after I'm gone."),
    MaryLetterScrollEntry::new(40, "That means that as you read\nthis, I'm already dead."),
    MaryLetterScrollEntry::new(80, "I can't tell you to remember me,\nbut I can't bear for you to\nforget me."),
    MaryLetterScrollEntry::new(80, "These last few years since I\nbecame ill...I'm so sorry for\nwhat I did to you, did to us..."),
    MaryLetterScrollEntry::new(95, "You've given me so much and\nI haven't been able to return\na single thing."),
    MaryLetterScrollEntry::new(95, "That's why I want you to live\nfor yourself now. \nDo what's best for you, James."),
    MaryLetterScrollEntry::new(160, "James..."),
    MaryLetterScrollEntry::new(120, "You made me happy."),
];

/// Complete 33-entry retail `data/etc/message/letter_w1_msg_e.mes` slice (identical to `LETTER_MSG_E_ENTRIES[..33]`).
pub const LETTER_W1_MSG_E_ENTRIES: &[MaryLetterScrollEntry] = LETTER_MSG_E_ENTRIES.split_at(33).0;

/// Complete 2-entry retail `data/etc/message/letter_w2_msg_e.mes` table (*In Water* `25.0` s closing scroll at `0x00445a6f`).
pub const LETTER_W2_MSG_E_ENTRIES: [MaryLetterScrollEntry; 2] = [
    MaryLetterScrollEntry::new(0, "James..."),
    MaryLetterScrollEntry::new(120, "You made me happy."),
];

/// Complete 35-entry retail `data/etc/message/letter_w0_msg_e.mes` table (*In Water* full combined letter).
pub const LETTER_W0_MSG_E_ENTRIES: [MaryLetterScrollEntry; 35] = [
    MaryLetterScrollEntry::new(0, "In my restless dreams,\nI see that town."),
    MaryLetterScrollEntry::new(25, "Silent Hill."),
    MaryLetterScrollEntry::new(35, "You promised you'd take me\nthere again someday.\nBut you never did."),
    MaryLetterScrollEntry::new(25, "Well I'm alone there now..."),
    MaryLetterScrollEntry::new(25, "In our special place...\nWaiting for you..."),
    MaryLetterScrollEntry::new(105, "Waiting for you to\ncome to see me."),
    MaryLetterScrollEntry::new(34, "But you never do."),
    MaryLetterScrollEntry::new(26, "And so I wait, wrapped in my\ncocoon of pain and loneliness."),
    MaryLetterScrollEntry::new(17, "I know I've done a terrible\nthing to you. Something you'll\nnever forgive me for."),
    MaryLetterScrollEntry::new(43, "I wish I could change \nthat, but I can't."),
    MaryLetterScrollEntry::new(25, "I feel so pathetic and ugly\nlaying here, waiting for you..."),
    MaryLetterScrollEntry::new(25, "Every day I stare up at the cracks\nin the ceiling and all I can think\nabout is how unfair it all is..."),
    MaryLetterScrollEntry::new(205, "The doctor came today.\nHe told me I could go\nhome for a short stay."),
    MaryLetterScrollEntry::new(30, "It's not that I'm getting better.\nIt's just that this may be\nmy last chance..."),
    MaryLetterScrollEntry::new(50, "I think you know what I mean..."),
    MaryLetterScrollEntry::new(100, "Even so, I'm glad to be cominghome. I've missed you terribly."),
    MaryLetterScrollEntry::new(25, "But I'm afraid James.\nI'm afraid you don't reallywant me to come home."),
    MaryLetterScrollEntry::new(120, "Whenever you come see me,\nI can tell how hard it is on you..."),
    MaryLetterScrollEntry::new(40, "I don't know if you\nhate me or pity me...\nOr maybe I just disgust you...."),
    MaryLetterScrollEntry::new(45, "I'm sorry about that."),
    MaryLetterScrollEntry::new(35, "When I first learned that\nI was going to die, I just\ndidn't want to accept it."),
    MaryLetterScrollEntry::new(40, "I was so angry all the time and I\nstruck out at everyone I loved most.\nEspecially you, James."),
    MaryLetterScrollEntry::new(80, "That's why I understand\nif you do hate me."),
    MaryLetterScrollEntry::new(80, "But I want you to\nknow this, James."),
    MaryLetterScrollEntry::new(180, "I'll always love you."),
    MaryLetterScrollEntry::new(140, "Even though our life together had\nto end like this, I still wouldn't\ntrade it for the world. We had\nsome wonderful years together."),
    MaryLetterScrollEntry::new(100, "Well this letter has gone on\ntoo long so I'll say goodbye."),
    MaryLetterScrollEntry::new(35, "I told the nurse to give\nthis to you after I'm gone."),
    MaryLetterScrollEntry::new(35, "That means that as you readthis, I'm already dead."),
    MaryLetterScrollEntry::new(60, "I can't tell you to remember me,\nbut I can't bear for\nyou to forget me."),
    MaryLetterScrollEntry::new(70, "These last few years since I\nbecame ill...I'm so sorry for\nwhat I did to you, did to us..."),
    MaryLetterScrollEntry::new(110, "You've given me so much and\nI haven't been able to return\na single thing."),
    MaryLetterScrollEntry::new(110, "That's why I want you to live\nfor yourself now. \nDo what's best for you, James."),
    MaryLetterScrollEntry::new(150, "James..."),
    MaryLetterScrollEntry::new(120, "you made me happy."),
];

/// Binary-decoded `.mes` letter entry with its exact `0xf8XX` vertical spacing tag (`0x00483104..0x0048317e`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedLetterMesEntry {
    /// Leading `0xf8` / `0xf9` 9-bit vertical spacing in pixels (`0..=511`).
    pub f8_spacing_px: u16,
    /// Decoded English stanza text.
    pub text: String,
}

/// Decodes a retail `letter*_msg_e.mes` buffer using the exact control-word rules at `0x00483067..0x0048317e`.
pub fn parse_letter_mes_entries(data: &[u8]) -> Option<Vec<ParsedLetterMesEntry>> {
    if data.len() < 2 {
        return None;
    }
    let count = u16::from_le_bytes([data[0], data[1]]) as usize;
    let header_bytes = 2usize.checked_add(count.checked_mul(2)?)?;
    if data.len() < header_bytes {
        return None;
    }

    let mut offsets = Vec::with_capacity(count);
    for i in 0..count {
        let p = 2 + i * 2;
        offsets.push((u16::from_le_bytes([data[p], data[p + 1]]) as usize) * 2);
    }

    let mut entries = Vec::with_capacity(count);
    for (i, &start) in offsets.iter().enumerate() {
        let end = offsets
            .get(i + 1)
            .copied()
            .unwrap_or(data.len())
            .min(data.len());
        if start >= end {
            return None;
        }
        let mut p = start;
        let mut f8_spacing_px = 0u16;
        if p + 2 <= end {
            let w0 = u16::from_le_bytes([data[p], data[p + 1]]);
            if (w0 & 0xFE00) == 0xF800 {
                f8_spacing_px = w0 & 0x01FF;
                p += 2;
            } else if w0 == 0x8000 {
                p += 2;
                if p + 2 <= end && (data[p] & 0xFE) == 0xF8 {
                    f8_spacing_px = (((data[p] & 0x01) as u16) << 8) | (data[p + 1] as u16);
                    p += 2;
                }
            }
        }

        let mut text = String::new();
        while p < end {
            let b = data[p];
            if b == 0xFF && p + 1 < end {
                let code = data[p + 1];
                p += 2;
                if code == 0xFF {
                    break;
                }
                if code == 0xFD {
                    text.push('\n');
                }
                continue;
            }
            if b < 0x60 {
                text.push((b + 0x20) as char);
            }
            p += 1;
        }
        entries.push(ParsedLetterMesEntry {
            f8_spacing_px,
            text,
        });
    }
    Some(entries)
}

/// Positioned Mary letter entry in virtual 512p scroll space (`0x0048303b..0x004831c8`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PositionedLetterEntry {
    /// Entry index (`0..entries.len()`).
    pub index: usize,
    /// Virtual Y coordinate before subtracting cumulative scroll offset (`512 + ...`).
    pub start_y_px: i32,
    /// Rendered height in pixels (`line_count * 30`).
    pub height_px: i32,
    /// Stanza text.
    pub text: &'static str,
}

/// Computes the exact virtual Y positions of `entries` (`0x0048303b..0x004831c8`).
pub fn layout_letter_scroll_entries(
    entries: &'static [MaryLetterScrollEntry],
) -> Vec<PositionedLetterEntry> {
    let mut out = Vec::with_capacity(entries.len());
    let mut cursor_y = LETTER_SCROLL_INITIAL_Y_PX;
    for (index, entry) in entries.iter().copied().enumerate() {
        let start_y_px = cursor_y + i32::from(entry.f8_spacing_px);
        let height_px = entry.height_px();
        out.push(PositionedLetterEntry {
            index,
            start_y_px,
            height_px,
            text: entry.text,
        });
        cursor_y = start_y_px + height_px + LETTER_SCROLL_ENTRY_GAP_PX;
    }
    out
}

/// Total upward scroll duration (`s`) until the final entry's bottom edge exits `y <= 0` (`0x004831e8..0x004832c2`).
pub fn letter_scroll_total_duration_secs(entries: &'static [MaryLetterScrollEntry]) -> f32 {
    let Some(last) = layout_letter_scroll_entries(entries).pop() else {
        return 0.0;
    };
    ((last.start_y_px + last.height_px) as f32) / LETTER_SCROLL_SPEED_PX_PER_SEC
}

/// Returns `(screen_y_px, entry)` for all entries visible within `[0, viewport_height_px)` at `elapsed_secs` (`16.5` px/s).
pub fn visible_letter_entries_at(
    entries: &'static [MaryLetterScrollEntry],
    elapsed_secs: f32,
    viewport_height_px: i32,
) -> Vec<(i32, &'static MaryLetterScrollEntry)> {
    let scroll_px = (elapsed_secs.max(0.0) * LETTER_SCROLL_SPEED_PX_PER_SEC) as i32;
    let mut visible = Vec::new();
    let mut cursor_y = LETTER_SCROLL_INITIAL_Y_PX;
    for entry in entries {
        let start_y = cursor_y + i32::from(entry.f8_spacing_px);
        let h = entry.height_px();
        let screen_y = start_y - scroll_px;
        if screen_y + h > 0 && screen_y < viewport_height_px {
            visible.push((screen_y, entry));
        }
        cursor_y = start_y + h + LETTER_SCROLL_ENTRY_GAP_PX;
    }
    visible
}
