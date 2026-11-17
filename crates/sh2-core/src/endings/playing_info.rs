//! Live Ending Point Evaluation Engine (`playing_info.c` & `stg_hotel_1f_b.c`).
//!
//! Verified 1:1 against:
//! - PS2 Prototype ELF (`SLUS_202.28`):
//!   - `Playing_Info` struct (`DWARF @0x2539f`, global `playing` at `0x0122b310`, `140` bytes).
//!   - `PlayingInfoColdInit` (`0x0020a730`) & `PlayingInfoHotInit` (`0x0020a780`).
//!   - `GameMoveDistanceCountUp` (`0x00290310`) & `GameMariaDamagedCountUp` (`0x00290500`).
//! - Retail PC Binary (`sh2pc.exe` v1.0 Director's Cut):
//!   - `playing` global at `0x01dbbf80` (`140` bytes).
//!   - `EvProgEndingJudge` (`0x00576490..0x0057672a`, Stage 44 `stg_hotel_1f_b` `ev_prog[6]`).
//!   - Low-HP weighted timer & Maria escort proximity/bump accumulators (`0x0048eccb..0x0048ed8c`).
//!   - Overheal ratio accumulator in `ItemMedicineUse` (`0x0049ecf5..0x0049ed32`).
//!   - Item examination hooks (`Photo of Mary` `20`, `Letter from Mary` `21`, `Angela's knife` `24`
//!     at `0x00471ca6..0x0047219b`).
//!   - Mary's long corridor monologue timer (`EvAllTimeFunc` `0x005767e0`, Room `0xba`).
//!   - Clear-ending recorder (`prog_clear_record` at `0x0048d330`).

use crate::events::ClearData;

/// Byte size of the authentic `Playing_Info` runtime struct (`0x8c` = `140` bytes at `0x0122b310` PS2 / `0x01dbbf80` PC).
pub const PLAYING_INFO_SIZE_BYTES: usize = 0x8c;

/// Starting score for the *Leave* (`Recovery`) ending (`%edi = 5` at `0x0057649f` in `sh2pc.exe`).
pub const LEAVE_START_POINTS: i32 = 5;
/// Starting score for the *Maria* ending (`%ecx = 3` at `0x005764a8` in `sh2pc.exe`).
pub const MARIA_START_POINTS: i32 = 3;
/// Starting score for the *In Water* (`Suicide`) ending (`%esi = 2` at `0x005764ad` in `sh2pc.exe`).
pub const IN_WATER_START_POINTS: i32 = 2;

/// Maximum per-frame displacement counted by `GameMoveDistanceCountUp` (`200.0` mm at `0x00290310` PS2).
/// Larger single-frame jumps (room warps / teleports) are ignored.
pub const MAX_VALID_MOVE_DELTA_MM: f32 = 200.0;

/// Player attack ID cutoff in `GameMariaDamagedCountUp` (`0x24` at `0x00290500` PS2; `0x2a` at `0x00539bd0` PC).
/// Attack IDs strictly below this threshold are James's attacks (friendly fire); IDs at or above are enemy attacks.
pub const PC_PLAYER_ATTACK_ID_LIMIT: u16 = 0x2a;

/// Low-HP ratio threshold (`hp / hp_max < 0.5` at `0x0048ece1`, `0x0062fe1c` in `sh2pc.exe`).
pub const LOW_HP_RATIO_THRESHOLD: f32 = 0.5;
/// First low-HP weighted time threshold (`> 60.0` at `0x00576541`, `0x006a7958` -> `+1` In Water).
pub const LOW_HP_WEIGHTED_SECS_TIER_1: f32 = 60.0;
/// Second low-HP weighted time threshold (`> 240.0` at `0x00576550`, `0x006b1240` -> `+1` In Water).
pub const LOW_HP_WEIGHTED_SECS_TIER_2: f32 = 240.0;

/// Overheal ratio sum threshold (`> 2.0` at `0x00576563`, `0x006315bc` -> `+1` Leave).
pub const OVERHEAL_RATIO_THRESHOLD: f32 = 2.0;

/// Maria close escort distance threshold (`< 1000.0` mm at `0x0048ed2f`, `0x006388c8`).
pub const MARIA_CLOSE_DISTANCE_MM: f32 = 1000.0;
/// Maria close escort cumulative time threshold (`> 600.0` s = 10 min at `0x00576572`, `0x0063b850` -> `+1` Maria).
pub const MARIA_CLOSE_TIME_THRESHOLD_SECS: f32 = 600.0;
/// Maria far escort distance threshold (`> 5000.0` mm at `0x0048ed4c`, `0x006a9244`).
pub const MARIA_FAR_DISTANCE_MM: f32 = 5000.0;
/// Maria far escort cumulative time threshold (`> 300.0` s = 5 min at `0x00576581`, `0x0069f3fc` -> `-1` Maria).
pub const MARIA_FAR_TIME_THRESHOLD_SECS: f32 = 300.0;

/// Maximum Maria bump count tracked (`100` at `0x0048ed78`).
pub const MARIA_BUMP_COUNTER_CAP: u8 = 100;
/// Low Maria bump count threshold (`< 3` at `0x00576598` -> `+1` Maria).
pub const MARIA_BUMP_LOW_THRESHOLD: u8 = 3;
/// High Maria bump count threshold (`> 30` at `0x00576592` -> `-1` Maria).
pub const MARIA_BUMP_HIGH_THRESHOLD: u8 = 30;

/// Maria base HP scale on Beginner/Easy action difficulty (`battle_level <= 1`: `160.0` at `0x0069f388`).
pub const MARIA_HP_SCALE_EASY: f32 = 160.0;
/// Maria base HP scale on Normal/Hard action difficulty (`battle_level >= 2`: `80.0` at `0x0069f3d0`).
pub const MARIA_HP_SCALE_NORMAL_HARD: f32 = 80.0;
/// Maria enemy-damage to James-damage protection ratio (`< 0.03` at `0x00576644`, `0x006c3240` -> `+1` Maria).
pub const MARIA_ENEMY_DAMAGE_RATIO_LIMIT: f32 = 0.03;

/// Delay before Mary's long corridor monologue begins in Hotel 1F B Room `0xba` (`3.5` s at `0x006cdda0`).
pub const MARY_CORRIDOR_START_DELAY_SECS: f32 = 3.5;
/// Subtitle frame threshold (`2400.0` frames = `80.0` s at 30 Hz, `0x0070fdd4` at `0x005767fd`) that sets Flag 624.
pub const MARY_CORRIDOR_COMPLETE_SUBTITLE_FRAMES: f32 = 2400.0;
/// Voice stream ID for Mary's long corridor monologue (`0xead8` = `60120` = `voice.afs[120]`).
pub const SE_MARY_CORRIDOR_MONOLOGUE: u32 = 0xead8;
/// Room ID for the final Hotel 1F B long corridor (`0xba` at `0x005767e5`).
pub const ROOM_HOTEL_CORRIDOR_MONOLOGUE: u16 = 0x00ba;

/// Item ID: `Photo of Mary` (`20`, `0x00471c98`).
pub const ITEM_PHOTO_OF_MARY: u8 = 20;
/// Item ID: `Letter from Mary` (`21`, `0x00471dc0`).
pub const ITEM_LETTER_FROM_MARY: u8 = 21;
/// Item ID: `Angela's knife` (`24`, `0x00471cb6`).
pub const ITEM_ANGELAS_KNIFE: u8 = 24;

/// Event flag: Hospital Roof suicidal diary read (`Flag 204` at `0x005764dc` -> `+1` In Water).
pub const FLAG_HOSPITAL_ROOF_DIARY: usize = 204;
/// Event flag: Revisited Maria's Labyrinth room after her death (`Flag 371` at `0x005764e9` -> `+1` Maria).
pub const FLAG_LABYRINTH_MARIA_REVISIT: usize = 371;
/// Event flag: Listened to Hotel 2F Reading Room headphones (`Flag 500` at `0x005764f8` -> `+1` In Water).
pub const FLAG_HOTEL_READING_ROOM_TAPE: usize = 500;
/// Event flag: Read Neely's Bar 2nd visit message in Dark South Vale (`Flag 615`, `Stage 2 ev[13] @0x008e7db8` -> `+1` In Water).
pub const FLAG_NEELYS_BAR_SECOND_MESSAGE: usize = 615;
/// Event flag: Result flag for *Leave* ending (`Flag 618` at `0x0057669f`).
pub const FLAG_ENDING_LEAVE: usize = 618;
/// Event flag: Result flag for *Maria* ending (`Flag 619` at `0x005766ad`).
pub const FLAG_ENDING_MARIA: usize = 619;
/// Event flag: Result flag for *In Water* ending (`Flag 620` at `0x005766e7`).
pub const FLAG_ENDING_IN_WATER: usize = 620;
/// Event flag: Result flag for *Rebirth* ending (`Flag 621` at `0x0057665d`).
pub const FLAG_ENDING_REBIRTH: usize = 621;
/// Event flag: Result flag for *Dog* ending (`Flag 622` at `0x005745b4`).
pub const FLAG_ENDING_DOG: usize = 622;
/// Event flag: Listened to Mary's full hallway monologue (`Flag 624` at `0x00576807` -> `+1` Leave, `+1` In Water).
pub const FLAG_MARY_HALLWAY_MONOLOGUE_COMPLETE: usize = 624;
/// Event flag: Checked on Maria resting in Brookhaven Room S3 a 2nd time (`Flag 629` at `0x0058ee99` -> `+1` Maria).
pub const FLAG_HOSPITAL_S3_MARIA_SECOND_CHECK: usize = 629;
/// Event flag: Examined Angela's Knife in inventory (`Flag 630` at `0x00471cbf` -> `+1` In Water).
pub const FLAG_ANGELAS_KNIFE_EXAMINED: usize = 630;
/// Event flag: Walked wrong direction away from Rosewater Park / Hotel (`Flag 655` at `0x00592b82` -> `-1` Leave).
pub const FLAG_MARIA_WRONG_DIRECTION: usize = 655;
/// Event flag: Reminded Maria about Mary in West South Vale (`Flag 656` at `0x0059304b` -> `-1` Maria).
pub const FLAG_MARIA_REMINDED_ABOUT_MARY: usize = 656;
/// Event flag: Result flag for *UFO* ending (`Flag 727` at `0x00578ee8`).
pub const FLAG_ENDING_UFO: usize = 727;
/// Event flag: Result flag for *Born from a Wish* (`Flag 728` at `0x00573b9f`).
pub const FLAG_ENDING_BORN_FROM_A_WISH: usize = 728;

/// Authentic `playing.clear_end_kind` bitmask (`0x01dbbf84` in `sh2pc.exe`, recorded by `0x0048d330`).
///
/// Verified 1:1 against every `call 0x0048d330` callsite in `sh2pc.exe`:
/// - `0x01`: Leave / Recovery (`0x0059fa0f`)
/// - `0x02`: Maria (`0x005a12a5`)
/// - `0x04`: In Water / Suicide (`0x0043f242`, `end_scde.sfd`)
/// - `0x08`: Rebirth (`0x0057df70`, `end_rbrt.sfd`)
/// - `0x10`: Dog (`0x005745ba`, `end_dog.sfd`)
/// - `0x20`: UFO (`0x00578ed4`)
/// - `0x40`: Born from a Wish (`0x00573b8b`, `end_wish.sfd`)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ClearEndKind(pub u8);

impl ClearEndKind {
    /// Leave (`Recovery`) ending bit (`0x01`).
    pub const LEAVE: u8 = 0x01;
    /// Maria ending bit (`0x02`).
    pub const MARIA: u8 = 0x02;
    /// In Water (`Suicide`) ending bit (`0x04`).
    pub const IN_WATER: u8 = 0x04;
    /// Rebirth ending bit (`0x08`).
    pub const REBIRTH: u8 = 0x08;
    /// Dog ending bit (`0x10`).
    pub const DOG: u8 = 0x10;
    /// UFO ending bit (`0x20`).
    pub const UFO: u8 = 0x20;
    /// *Born from a Wish* sub-scenario ending bit (`0x40`).
    pub const BORN_FROM_A_WISH: u8 = 0x40;

    /// Returns `true` if `bit` has been cleared.
    #[inline]
    pub const fn contains(self, bit: u8) -> bool {
        (self.0 & bit) != 0
    }

    /// Count of cleared main point-based endings (`Leave`, `Maria`, `In Water`: `0..=3`, `%esi` at `0x0048aa89`).
    #[inline]
    pub const fn base_endings_count(self) -> u8 {
        (self.0 & 1) + ((self.0 >> 1) & 1) + ((self.0 >> 2) & 1)
    }

    /// Count of cleared James scenario endings (`0x01..=0x20`: `0..=6`, excluding *Born from a Wish*).
    #[inline]
    pub const fn main_scenario_endings_count(self) -> u8 {
        (self.0 & 0x3f).count_ones() as u8
    }

    /// Converts from `crate::events::ClearData` (`endings_seen` bit order) into `playing.clear_end_kind` bit order.
    pub const fn from_clear_data(clear: ClearData) -> Self {
        let e = clear.endings_seen;
        let mut raw = 0u8;
        if (e & ClearData::ENDING_LEAVE) != 0 {
            raw |= Self::LEAVE;
        }
        if (e & ClearData::ENDING_MARIA) != 0 {
            raw |= Self::MARIA;
        }
        if (e & ClearData::ENDING_IN_WATER) != 0 {
            raw |= Self::IN_WATER;
        }
        if (e & ClearData::ENDING_REBIRTH) != 0 {
            raw |= Self::REBIRTH;
        }
        if (e & ClearData::ENDING_DOG) != 0 {
            raw |= Self::DOG;
        }
        if (e & ClearData::ENDING_UFO) != 0 {
            raw |= Self::UFO;
        }
        if (e & ClearData::ENDING_BORN_FROM_A_WISH) != 0 {
            raw |= Self::BORN_FROM_A_WISH;
        }
        Self(raw)
    }

    /// Converts this `ClearEndKind` bitmask into the corresponding `ClearData::ENDING_*` mask.
    pub const fn to_clear_data_bits(self) -> u8 {
        let mut out = 0u8;
        if self.contains(Self::LEAVE) {
            out |= ClearData::ENDING_LEAVE;
        }
        if self.contains(Self::MARIA) {
            out |= ClearData::ENDING_MARIA;
        }
        if self.contains(Self::IN_WATER) {
            out |= ClearData::ENDING_IN_WATER;
        }
        if self.contains(Self::REBIRTH) {
            out |= ClearData::ENDING_REBIRTH;
        }
        if self.contains(Self::DOG) {
            out |= ClearData::ENDING_DOG;
        }
        if self.contains(Self::UFO) {
            out |= ClearData::ENDING_UFO;
        }
        if self.contains(Self::BORN_FROM_A_WISH) {
            out |= ClearData::ENDING_BORN_FROM_A_WISH;
        }
        out
    }
}

/// All 7 playable Silent Hill 2 & Director's Cut endings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GameEnding {
    /// *Leave* (`stg_end_recovery.c`, Stage 48 -> Stage 4 cemetery).
    Leave,
    /// *Maria* (`stg_end_maria.c`, Stage 49).
    Maria,
    /// *In Water* (`stg_end_suicide.c`, Stage 50).
    InWater,
    /// *Rebirth* (`stg_end_rebirth.c`, Stage 51).
    Rebirth,
    /// *Dog* (`stg_end_dog.c`, Stage 52).
    Dog,
    /// *UFO* (Director's Cut Room 312 `EvProgUfoMado` `0x00578c50`).
    Ufo,
    /// *Born from a Wish* Maria sub-scenario (`stg_end_wish`, Stage 58).
    BornFromAWish,
}

impl GameEnding {
    /// Authentic `playing.clear_end_kind` bit (`0x0048d330`).
    pub const fn clear_end_bit(self) -> u8 {
        match self {
            Self::Leave => ClearEndKind::LEAVE,
            Self::Maria => ClearEndKind::MARIA,
            Self::InWater => ClearEndKind::IN_WATER,
            Self::Rebirth => ClearEndKind::REBIRTH,
            Self::Dog => ClearEndKind::DOG,
            Self::Ufo => ClearEndKind::UFO,
            Self::BornFromAWish => ClearEndKind::BORN_FROM_A_WISH,
        }
    }

    /// Result event flag set when this ending triggers (`618..=622`, `727`, `728`).
    pub const fn result_flag(self) -> usize {
        match self {
            Self::Leave => FLAG_ENDING_LEAVE,
            Self::Maria => FLAG_ENDING_MARIA,
            Self::InWater => FLAG_ENDING_IN_WATER,
            Self::Rebirth => FLAG_ENDING_REBIRTH,
            Self::Dog => FLAG_ENDING_DOG,
            Self::Ufo => FLAG_ENDING_UFO,
            Self::BornFromAWish => FLAG_ENDING_BORN_FROM_A_WISH,
        }
    }

    /// Primary ending stage ID (`1..=58` in `0x00495048`).
    pub const fn stage_id(self) -> u8 {
        match self {
            Self::Leave => 48,
            Self::Maria => 49,
            Self::InWater => 50,
            Self::Rebirth => 51,
            Self::Dog => 52,
            Self::Ufo => 42,
            Self::BornFromAWish => 58,
        }
    }
}

/// Outcome of the main-scenario ending evaluation (`EvProgEndingJudge` at `0x00576490`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MainEndingKind {
    /// *Leave* (`Flag 618`).
    Leave,
    /// *Maria* (`Flag 619`).
    Maria,
    /// *In Water* (`Flag 620`).
    InWater,
    /// *Rebirth* (`Flag 621`, triggered when all 4 ritual items are held).
    Rebirth,
}

impl MainEndingKind {
    /// Converts to [`GameEnding`].
    pub const fn to_game_ending(self) -> GameEnding {
        match self {
            Self::Leave => GameEnding::Leave,
            Self::Maria => GameEnding::Maria,
            Self::InWater => GameEnding::InWater,
            Self::Rebirth => GameEnding::Rebirth,
        }
    }
}

/// Authentic `Playing_Info` runtime telemetry structure (`SLUS_202.28` `0x0122b310` / `sh2pc.exe` `0x01dbbf80`).
#[derive(Debug, Clone, PartialEq)]
pub struct PlayingInfo {
    /// Current stage ID (`+0x00`).
    pub stage: u8,
    /// Debug enemy-disable toggle (`+0x01`).
    pub enemy_off: u8,
    /// Cleared difficulty bitmask (`+0x02` on PC `0x01dbbf82`).
    pub cleared_difficulty_mask: u8,
    /// Memo selection index (`+0x03`).
    pub memo_select: u8,
    /// Seen endings bitmask (`+0x04`, `0x01dbbf84`).
    pub clear_end_kind: ClearEndKind,
    /// Main-scenario clear count (`+0x05`, `0x01dbbf85`).
    pub clear_end_number: u8,
    /// Total star ranking points (`0..=100`, `+0x06`, `0x01dbbf86`; initialized to `6` in `PlayingInfoHotInit`).
    pub rank: u8,
    /// Game stage / result mode (`+0x07`, `0x01dbbf87`).
    pub game_stage: i8,
    /// Bonus/hidden items found bitmask (`+0x08`, `0x01dbbf88`, bits `0x01..0x80`).
    pub hidden_item_get: u8,
    /// Hyper Spray power tier (`-1..=2`, `+0x09`, `0x01dbbf89`).
    pub spray_pow: i8,
    /// Number of saves recorded (`+0x0a`, `0x01dbbf8a`).
    pub savecount: u16,
    /// Total clears (`+0x0c`, `0x01dbbf8c`).
    pub clearcount: u16,
    /// Standard items picked up (`+0x0e`, `0x01dbbf8e`).
    pub item_get: u16,
    /// Enemies killed by firearms (`+0x10`, `0x01dbbf90`).
    pub kill_by_shot: u16,
    /// Enemies killed by melee/fighting (`+0x12`, `0x01dbbf92`).
    pub kill_by_fight: u16,
    /// Active playthrough clear time in seconds (`+0x14`, `0x01dbbf94`).
    pub time: f32,
    /// Total walking distance in PS2 world mm (`+0x18`, `0x01dbbf98`).
    pub walk_distance: f32,
    /// Total running distance in PS2 world mm (`+0x1c`, `0x01dbbf9c`).
    pub run_distance: f32,
    /// Toluca Lake boat stage clear time in seconds (`+0x20`, `0x01dbbfa0`).
    pub boat_clear_time: f32,
    /// Toluca Lake boat maximum speed (`+0x24`, `0x01dbbfa4`).
    pub boat_max_speed: f32,
    /// Cumulative HP damage taken by James (`+0x28`, `0x01dbbfa8`).
    pub jms_damage_total: f32,
    /// Cumulative HP damage dealt to Maria by enemies (`+0x2c`, `0x01dbbfac`).
    pub mar_damage_by_enemy: f32,
    /// Cumulative HP damage dealt to Maria by James (`+0x30`, `0x01dbbfb0`).
    pub mar_damage_by_jms: f32,
    /// Cumulative play time across clears (`+0x70`, `0x01dbbff0`).
    pub total_time: f32,
    /// Action difficulty index (`0` Beginner, `1` Easy, `2` Normal, `3` Hard at `+0x74`, `0x01dbbff4`).
    pub battle_level: u8,
    /// Riddle difficulty index (`0` Easy, `1` Normal, `2` Hard, `3` Extra at `+0x75`, `0x01dbbff5`).
    pub riddle_level: u8,
}

impl Default for PlayingInfo {
    fn default() -> Self {
        let mut info = Self {
            stage: 0,
            enemy_off: 0,
            cleared_difficulty_mask: 0,
            memo_select: 0,
            clear_end_kind: ClearEndKind(0),
            clear_end_number: 0,
            rank: 6,
            game_stage: 0,
            hidden_item_get: 0,
            spray_pow: 0,
            savecount: 0,
            clearcount: 0,
            item_get: 0,
            kill_by_shot: 0,
            kill_by_fight: 0,
            time: 0.0,
            walk_distance: 0.0,
            run_distance: 0.0,
            boat_clear_time: 0.0,
            boat_max_speed: 0.0,
            jms_damage_total: 0.0,
            mar_damage_by_enemy: 0.0,
            mar_damage_by_jms: 0.0,
            total_time: 0.0,
            battle_level: 2,
            riddle_level: 1,
        };
        info.hot_init();
        info
    }
}

impl PlayingInfo {
    /// Resets per-run counters while preserving persistent `clear_end_kind`, `clear_end_number`,
    /// `spray_pow`, and `total_time` (`PlayingInfoHotInit` at `0x0020a780` in `SLUS_202.28`).
    pub fn hot_init(&mut self) {
        self.rank = 6;
        self.hidden_item_get = 0;
        self.savecount = 0;
        self.item_get = 0;
        self.kill_by_shot = 0;
        self.kill_by_fight = 0;
        self.time = 0.0;
        self.walk_distance = 0.0;
        self.run_distance = 0.0;
        self.boat_clear_time = 0.0;
        self.boat_max_speed = 0.0;
        self.jms_damage_total = 0.0;
        self.mar_damage_by_enemy = 0.0;
        self.mar_damage_by_jms = 0.0;
    }

    /// Accumulates step distance (`GameMoveDistanceCountUp` at `0x00290310` in `SLUS_202.28`),
    /// discarding room-transition warps where `delta_mm >= 200.0`.
    pub fn record_move_distance(&mut self, delta_mm: f32, is_running: bool) {
        if (0.0..MAX_VALID_MOVE_DELTA_MM).contains(&delta_mm) {
            if is_running {
                self.run_distance += delta_mm;
            } else {
                self.walk_distance += delta_mm;
            }
        }
    }

    /// Records damage taken by James (`0x00539bb0` in `sh2pc.exe`).
    pub fn record_james_damage(&mut self, damage: f32) {
        if damage > 0.0 {
            self.jms_damage_total += damage;
        }
    }

    /// Splits damage taken by Maria into James friendly-fire vs enemy damage (`0x00539bd0` PC / `0x00290500` PS2).
    pub fn record_maria_damage_by_attack(&mut self, attack_id: u16, damage: f32) {
        if damage <= 0.0 {
            return;
        }
        if attack_id < PC_PLAYER_ATTACK_ID_LIMIT {
            self.mar_damage_by_jms += damage;
        } else {
            self.mar_damage_by_enemy += damage;
        }
    }
}
