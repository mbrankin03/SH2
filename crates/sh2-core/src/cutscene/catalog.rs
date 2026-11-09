//! Cutscene descriptor catalog (`DramaDemoPlayInfo`, `DRAMA_DEMO_CATALOG`), subtitle timing windows
//! (`SubtitleWindow`, `adr_msg_time`), and hybrid sequence step chains (`SH2-63`).

use serde::{Deserialize, Serialize};

use super::anim_info::demo_fov_scale_for_demo_no;
use super::VOICE_AFS_STREAM_BASE;

/// Subtitle timing window (`adr_msg_time` entry `{ u16 start_frame, u16 end_frame }` in `Event/demoview.c`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubtitleWindow {
    /// 0-based offset added to `DramaDemoPlayInfo::msg_start` to select the `.mes` message ID.
    pub msg_offset: u16,
    /// Inclusive start frame (at 30 FPS `msg_frame`).
    pub start_frame: u16,
    /// Exclusive end frame (at 30 FPS `msg_frame`).
    pub end_frame: u16,
}

impl SubtitleWindow {
    pub const fn new(msg_offset: u16, start_frame: u16, end_frame: u16) -> Self {
        Self {
            msg_offset,
            start_frame,
            end_frame,
        }
    }

    /// Returns `true` if `msg_frame` falls within `[start_frame, end_frame)`.
    #[inline]
    pub fn is_active_at(self, msg_frame: f32) -> bool {
        msg_frame >= (self.start_frame as f32) && msg_frame < (self.end_frame as f32)
    }
}

/// Hybrid cutscene/movie step in a chained sequence (e.g., `deai` Rosewater Park Maria meeting,
/// `vtr` Room 312 Videotape, `bp_edi` Bowling Alley, or Endings).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CutsceneSequenceStep {
    /// Play an in-engine real-time 3D `.dds` cutscene (`demo_no`).
    InEngineDemo { demo_no: u16 },
    /// Play a pre-rendered Bink `.bik` / `.mp4` FMV movie (`"deai.bik"`, `"murder.bik"`, etc.).
    FmvMovie { movie_id: &'static str },
    /// Play a voice-only `.mes` subtitle sequence without a `.dds` script (`SubtitlesExec`).
    VoiceOnly {
        stream_no: i32,
        mes_path: &'static str,
        msg_start: u16,
    },
}

/// Static descriptor for an in-engine real-time 3D cutscene (`DramaDemoPlayInfo` in `Event/demoview.c`).
#[derive(Debug, Clone, Copy)]
pub struct DramaDemoPlayInfo {
    /// Engine cutscene identifier (`demo_no`, `1..=130`).
    pub demo_no: u16,
    /// Short identifier matching the demo folder name (e.g. `"tenbou"`, `"haka_agl"`, `"vtr"`).
    pub id: &'static str,
    /// Human-readable title for HUD & debug inspection.
    pub title: &'static str,
    /// Relative path to the `.dds` cutscene script file.
    pub dds_path: &'static str,
    /// Relative path to the stage `.map` geometry file under `data/` (e.g. `"bg/er/er01.map"`, `"bg/ob/ob01.map"`).
    pub stage_map_path: &'static str,
    /// Stage prefix used for SH2:EE HD `.tex` overrides (e.g. `"er01"`, `"ob01"`, `"ca11"`).
    pub stage_name: &'static str,
    /// Relative path to the stage `.mes` dialogue file (e.g. `"data/etc/message/stage_observation_msg_e.mes"`).
    pub mes_path: &'static str,
    /// Starting message index inside `mes_path` (`msg_start`).
    pub msg_start: u16,
    /// Subtitle timing intervals (`adr_msg_time`).
    pub subtitle_windows: &'static [SubtitleWindow],
    /// Voice stream number (`60001 + afs_index` in `voice.afs`, or `0` if silent).
    pub stream_no: i32,
    /// 30 FPS scene frame at which the voice stream begins playing (`stream_start`).
    pub stream_start: i16,
    /// Starting 30 FPS frame offset inside shared multi-segment `.anm` and `.cls` files
    /// (e.g. `1546` for `mar_deai_e`, `438` for `papa_agl`, `292` for `hei_lau_b`, `1679` for `piano_lau_e`).
    pub anm_frame_offset: u16,
    /// Stage world X coordinate offset applied to camera, lights, and actors (`offset_x`).
    pub offset_x: f32,
    /// Stage world Z coordinate offset applied to camera, lights, and actors (`offset_z`).
    pub offset_z: f32,
    /// Optional hybrid sequence chain when this cutscene transitions directly into/from an FMV or part B.
    pub sequence_chain: &'static [CutsceneSequenceStep],
}

impl DramaDemoPlayInfo {
    /// Returns the 0-based entry index in `data/sound/adx/voice/voice.afs` if `stream_no >= 60000`.
    #[inline]
    pub const fn voice_afs_index(&self) -> Option<usize> {
        if self.stream_no >= VOICE_AFS_STREAM_BASE {
            Some((self.stream_no - VOICE_AFS_STREAM_BASE) as usize)
        } else {
            None
        }
    }

    /// Returns the per-demo FOV scale multiplier (`1.0`, `1.25`, `1.35`, or `1.4`).
    #[inline]
    pub const fn fov_scale(&self) -> f32 {
        demo_fov_scale_for_demo_no(self.demo_no)
    }

    /// Returns `true` if subtitles are forced on regardless of user subtitle settings
    /// (`demo_no == 77` `radio_gero` in `DramaDemoMessage` at `0x1b1f80`).
    #[inline]
    pub const fn forces_subtitles(&self) -> bool {
        self.demo_no == 77
    }
}

/// `demo_no == 1` (`first_toilet` - Opening Restroom mirror monologue: "Mary... Could you really be in this town?")
pub const SUBTITLES_FIRST_TOILET: &[SubtitleWindow] = &[SubtitleWindow::new(0, 579, 786)];

/// `demo_no == 2` (`tenbou` - Observation Deck monologue & Mary's letter narration)
pub const SUBTITLES_TENBOU: &[SubtitleWindow] = &[
    SubtitleWindow::new(0, 570, 762),
    SubtitleWindow::new(1, 762, 846),
    SubtitleWindow::new(2, 870, 1014),
    SubtitleWindow::new(3, 1014, 1128),
    SubtitleWindow::new(4, 1128, 1281),
    SubtitleWindow::new(5, 1281, 1386),
    SubtitleWindow::new(6, 1386, 1455),
    SubtitleWindow::new(7, 1662, 1761),
    SubtitleWindow::new(8, 1761, 1977),
    SubtitleWindow::new(9, 1977, 2067),
    SubtitleWindow::new(10, 2067, 2280),
    SubtitleWindow::new(11, 2280, 2391),
    SubtitleWindow::new(12, 2391, 2526),
    SubtitleWindow::new(13, 2526, 2706),
    SubtitleWindow::new(14, 2706, 2862),
    SubtitleWindow::new(15, 2862, 3075),
    SubtitleWindow::new(16, 3075, 3204),
    SubtitleWindow::new(17, 3204, 3336),
    SubtitleWindow::new(18, 3336, 3603),
    SubtitleWindow::new(19, 3603, 3729),
    SubtitleWindow::new(20, 3729, 3954),
];

/// `demo_no == 3` (`haka_agl` - Graveyard meeting with Angela Orosco)
pub const SUBTITLES_HAKA_AGL: &[SubtitleWindow] = &[
    SubtitleWindow::new(0, 24, 177),
    SubtitleWindow::new(1, 177, 249),
    SubtitleWindow::new(2, 249, 372),
    SubtitleWindow::new(3, 372, 432),
    SubtitleWindow::new(4, 432, 501),
    SubtitleWindow::new(5, 501, 531),
    SubtitleWindow::new(6, 531, 570),
    SubtitleWindow::new(7, 570, 690),
    SubtitleWindow::new(8, 729, 942),
    SubtitleWindow::new(9, 942, 1038),
    SubtitleWindow::new(10, 1038, 1107),
    SubtitleWindow::new(11, 1107, 1272),
    SubtitleWindow::new(12, 1272, 1380),
    SubtitleWindow::new(13, 1380, 1458),
    SubtitleWindow::new(14, 1458, 1512),
    SubtitleWindow::new(15, 1512, 1677),
    SubtitleWindow::new(16, 1677, 1791),
    SubtitleWindow::new(17, 1791, 1866),
    SubtitleWindow::new(18, 1893, 1998),
    SubtitleWindow::new(19, 1998, 2079),
    SubtitleWindow::new(20, 2079, 2238),
    SubtitleWindow::new(21, 2238, 2361),
    SubtitleWindow::new(22, 2361, 2553),
    SubtitleWindow::new(23, 2553, 2682),
    SubtitleWindow::new(24, 2727, 2976),
    SubtitleWindow::new(25, 2991, 3111),
    SubtitleWindow::new(26, 3111, 3207),
    SubtitleWindow::new(27, 3207, 3282),
];

/// `demo_no == 6` (`saisho` - Radio & First Lying Figure Encounter)
pub const SUBTITLES_FIRST_BATTLE: &[SubtitleWindow] = &[];

/// `demo_no == 9` (`kagikeri` - Wood Side Apartments Laura Kicks the Key)
pub const SUBTITLES_KAGIKERI: &[SubtitleWindow] = &[
    SubtitleWindow::new(0, 418, 451),
    SubtitleWindow::new(1, 499, 523),
    SubtitleWindow::new(2, 523, 565),
    SubtitleWindow::new(3, 565, 610),
];

/// `demo_no == 13` (`gero_edi` - Wood Side Apartments Eddie Vomiting in Toilet)
pub const SUBTITLES_GERO_EDI: &[SubtitleWindow] = &[
    SubtitleWindow::new(0, 213, 294),
    SubtitleWindow::new(1, 294, 330),
    SubtitleWindow::new(2, 330, 438),
    SubtitleWindow::new(3, 438, 522),
    SubtitleWindow::new(4, 522, 639),
    SubtitleWindow::new(5, 639, 738),
    SubtitleWindow::new(6, 738, 858),
    SubtitleWindow::new(7, 858, 1044),
    SubtitleWindow::new(8, 1044, 1194),
    SubtitleWindow::new(9, 1194, 1338),
    SubtitleWindow::new(10, 1338, 1374),
    SubtitleWindow::new(11, 1374, 1515),
    SubtitleWindow::new(12, 1515, 1650),
    SubtitleWindow::new(13, 1650, 1794),
    SubtitleWindow::new(14, 1794, 1866),
    SubtitleWindow::new(15, 1866, 1983),
    SubtitleWindow::new(16, 1983, 2169),
    SubtitleWindow::new(17, 2259, 2511),
    SubtitleWindow::new(18, 2511, 2643),
    SubtitleWindow::new(19, 2643, 2718),
    SubtitleWindow::new(20, 2718, 2817),
    SubtitleWindow::new(21, 2817, 2934),
    SubtitleWindow::new(22, 2934, 3051),
    SubtitleWindow::new(23, 3051, 3174),
    SubtitleWindow::new(24, 3174, 3219),
    SubtitleWindow::new(25, 3219, 3408),
];

/// `demo_no == 14` (`sankaku1` - Wood Side Apartments Room 307 Pyramid Head Closet Cutscene)
pub const SUBTITLES_SANKAKU1: &[SubtitleWindow] = &[];

/// `demo_no == 20` (`hei_lau` - Laura on the Wall)
pub const SUBTITLES_HEI_LAU: &[SubtitleWindow] = &[];

/// `demo_no == 18` (`knife_agl` - Blue Creek Apartments Angela Mirror & Knife)
pub const SUBTITLES_KNIFE_AGL: &[SubtitleWindow] = &[
    SubtitleWindow::new(0, 21, 54),
    SubtitleWindow::new(1, 54, 123),
    SubtitleWindow::new(2, 165, 216),
    SubtitleWindow::new(3, 216, 276),
    SubtitleWindow::new(4, 276, 345),
    SubtitleWindow::new(5, 345, 465),
    SubtitleWindow::new(6, 465, 516),
    SubtitleWindow::new(7, 627, 681),
    SubtitleWindow::new(8, 681, 777),
    SubtitleWindow::new(9, 777, 867),
    SubtitleWindow::new(10, 867, 969),
    SubtitleWindow::new(11, 969, 1086),
    SubtitleWindow::new(12, 1110, 1224),
    SubtitleWindow::new(13, 1224, 1371),
    SubtitleWindow::new(14, 1416, 1464),
    SubtitleWindow::new(15, 1518, 1623),
    SubtitleWindow::new(16, 1623, 1761),
    SubtitleWindow::new(17, 1761, 1878),
    SubtitleWindow::new(18, 1878, 1950),
    SubtitleWindow::new(19, 1950, 2070),
    SubtitleWindow::new(20, 2070, 2163),
    SubtitleWindow::new(21, 2250, 2337),
    SubtitleWindow::new(22, 2337, 2358),
    SubtitleWindow::new(23, 2358, 2478),
    SubtitleWindow::new(24, 2478, 2565),
    SubtitleWindow::new(25, 2565, 2619),
    SubtitleWindow::new(26, 2646, 2703),
    SubtitleWindow::new(27, 2703, 2811),
    SubtitleWindow::new(28, 2865, 3024),
    SubtitleWindow::new(29, 3087, 3171),
    SubtitleWindow::new(30, 3204, 3348),
    SubtitleWindow::new(31, 3348, 3399),
    SubtitleWindow::new(32, 3450, 3600),
    SubtitleWindow::new(33, 3690, 3750),
    SubtitleWindow::new(34, 3750, 3825),
    SubtitleWindow::new(35, 3825, 4113),
    SubtitleWindow::new(36, 4113, 4200),
    SubtitleWindow::new(37, 4200, 4449),
    SubtitleWindow::new(38, 4449, 4545),
    SubtitleWindow::new(39, 4545, 4608),
    SubtitleWindow::new(40, 4608, 4650),
    SubtitleWindow::new(41, 4650, 4740),
    SubtitleWindow::new(42, 4740, 4965),
    SubtitleWindow::new(43, 4965, 5061),
    SubtitleWindow::new(44, 5100, 5193),
    SubtitleWindow::new(45, 5193, 5280),
    SubtitleWindow::new(46, 5280, 5358),
    SubtitleWindow::new(47, 5358, 5469),
    SubtitleWindow::new(48, 5541, 5574),
    SubtitleWindow::new(49, 5685, 5922),
];

/// `demo_no == 21` (`mar_deai_c` - Rosewater Park Meeting Maria Part 1, before `deai.bik`)
pub const SUBTITLES_DEAI_C: &[SubtitleWindow] = &[
    SubtitleWindow::new(0, 60, 147),
    SubtitleWindow::new(1, 147, 216),
    SubtitleWindow::new(2, 216, 312),
    SubtitleWindow::new(3, 312, 408),
    SubtitleWindow::new(4, 408, 477),
    SubtitleWindow::new(5, 477, 618),
    SubtitleWindow::new(6, 618, 732),
    SubtitleWindow::new(7, 789, 864),
    SubtitleWindow::new(8, 864, 1077),
    SubtitleWindow::new(9, 1077, 1155),
    SubtitleWindow::new(10, 1155, 1272),
    SubtitleWindow::new(11, 1296, 1467),
];

/// `demo_no == 22` (`mar_deai_e` - Rosewater Park Meeting Maria, Hybrid `deai.bik` -> Demo)
pub const SUBTITLES_DEAI: &[SubtitleWindow] = &[
    SubtitleWindow::new(0, 102, 195),
    SubtitleWindow::new(1, 195, 288),
    SubtitleWindow::new(2, 288, 420),
    SubtitleWindow::new(3, 420, 555),
    SubtitleWindow::new(4, 555, 645),
    SubtitleWindow::new(5, 645, 765),
    SubtitleWindow::new(6, 765, 945),
    SubtitleWindow::new(7, 1044, 1092),
    SubtitleWindow::new(8, 1092, 1152),
    SubtitleWindow::new(9, 1152, 1170),
    SubtitleWindow::new(10, 1170, 1230),
    SubtitleWindow::new(11, 1230, 1281),
    SubtitleWindow::new(12, 1281, 1467),
    SubtitleWindow::new(13, 1485, 1752),
    SubtitleWindow::new(14, 1752, 1890),
    SubtitleWindow::new(15, 1890, 1956),
    SubtitleWindow::new(16, 1956, 2001),
    SubtitleWindow::new(17, 2001, 2061),
];

/// `demo_no == 21/22` (`mar_deai.dds` - Full Rosewater Park Meeting Maria Part 1 + Part 2)
pub const SUBTITLES_MAR_DEAI_FULL: &[SubtitleWindow] = &[
    SubtitleWindow::new(0, 60, 147),
    SubtitleWindow::new(1, 147, 216),
    SubtitleWindow::new(2, 216, 312),
    SubtitleWindow::new(3, 312, 408),
    SubtitleWindow::new(4, 408, 477),
    SubtitleWindow::new(5, 477, 618),
    SubtitleWindow::new(6, 618, 732),
    SubtitleWindow::new(7, 789, 864),
    SubtitleWindow::new(8, 864, 1077),
    SubtitleWindow::new(9, 1077, 1155),
    SubtitleWindow::new(10, 1155, 1272),
    SubtitleWindow::new(11, 1296, 1467),
    SubtitleWindow::new(12, 1546 + 102, 1546 + 195),
    SubtitleWindow::new(13, 1546 + 195, 1546 + 288),
    SubtitleWindow::new(14, 1546 + 288, 1546 + 420),
    SubtitleWindow::new(15, 1546 + 420, 1546 + 555),
    SubtitleWindow::new(16, 1546 + 555, 1546 + 645),
    SubtitleWindow::new(17, 1546 + 645, 1546 + 765),
    SubtitleWindow::new(18, 1546 + 765, 1546 + 945),
    SubtitleWindow::new(19, 1546 + 1044, 1546 + 1092),
    SubtitleWindow::new(20, 1546 + 1092, 1546 + 1152),
    SubtitleWindow::new(21, 1546 + 1152, 1546 + 1170),
    SubtitleWindow::new(22, 1546 + 1170, 1546 + 1230),
    SubtitleWindow::new(23, 1546 + 1230, 1546 + 1281),
    SubtitleWindow::new(24, 1546 + 1281, 1546 + 1467),
    SubtitleWindow::new(25, 1546 + 1485, 1546 + 1752),
    SubtitleWindow::new(26, 1546 + 1752, 1546 + 1890),
    SubtitleWindow::new(27, 1546 + 1890, 1546 + 1956),
    SubtitleWindow::new(28, 1546 + 1956, 1546 + 2001),
    SubtitleWindow::new(29, 1546 + 2001, 1546 + 2061),
];

/// `demo_no == 25` (`bp_edi_1` - Pete's Bowl-O-Rama Part 1: Laura & Eddie)
pub const SUBTITLES_BP_EDI_1: &[SubtitleWindow] = &[
    SubtitleWindow::new(0, 126, 177),
    SubtitleWindow::new(1, 177, 237),
    SubtitleWindow::new(2, 237, 315),
    SubtitleWindow::new(3, 315, 420),
    SubtitleWindow::new(4, 420, 498),
    SubtitleWindow::new(5, 498, 591),
    SubtitleWindow::new(6, 591, 726),
    SubtitleWindow::new(7, 780, 870),
    SubtitleWindow::new(8, 870, 1053),
    SubtitleWindow::new(9, 1053, 1149),
    SubtitleWindow::new(10, 1149, 1239),
    SubtitleWindow::new(11, 1239, 1302),
];

/// `demo_no == 26` (`bp_edi_2` - Pete's Bowl-O-Rama Part 2: James & Eddie)
pub const SUBTITLES_BP_EDI_2: &[SubtitleWindow] = &[
    SubtitleWindow::new(0, 213, 270),
    SubtitleWindow::new(1, 270, 354),
    SubtitleWindow::new(2, 354, 390),
    SubtitleWindow::new(3, 390, 480),
    SubtitleWindow::new(4, 480, 630),
    SubtitleWindow::new(5, 726, 801),
    SubtitleWindow::new(6, 801, 870),
    SubtitleWindow::new(7, 1158, 1218),
    SubtitleWindow::new(8, 1218, 1275),
    SubtitleWindow::new(9, 1275, 1347),
    SubtitleWindow::new(10, 1347, 1461),
    SubtitleWindow::new(11, 1461, 1539),
    SubtitleWindow::new(12, 1539, 1611),
    SubtitleWindow::new(13, 1611, 1680),
    SubtitleWindow::new(14, 1680, 1752),
    SubtitleWindow::new(15, 1752, 1836),
    SubtitleWindow::new(16, 1836, 1944),
    SubtitleWindow::new(17, 1944, 2013),
];

/// `demo_no == 45` (`mar_saikai` - Labyrinth Maria Cell Reunion)
pub const SUBTITLES_MAR_SAIKAI: &[SubtitleWindow] = &[
    SubtitleWindow::new(0, 40, 166),
    SubtitleWindow::new(1, 166, 247),
    SubtitleWindow::new(2, 247, 391),
    SubtitleWindow::new(3, 391, 463),
    SubtitleWindow::new(4, 463, 574),
];

/// `demo_no == 68` (`papa_agl` - Labyrinth Abstract Daddy & Angela)
pub const SUBTITLES_PAPA_AGL: &[SubtitleWindow] = &[
    SubtitleWindow::new(0, 342, 423),
    SubtitleWindow::new(1, 948, 1011),
    SubtitleWindow::new(2, 1011, 1080),
    SubtitleWindow::new(3, 1080, 1176),
    SubtitleWindow::new(4, 1176, 1260),
    SubtitleWindow::new(5, 1260, 1404),
    SubtitleWindow::new(6, 1404, 1518),
    SubtitleWindow::new(7, 1518, 1596),
    SubtitleWindow::new(8, 1596, 1662),
    SubtitleWindow::new(9, 1662, 1779),
    SubtitleWindow::new(10, 1779, 2016),
    SubtitleWindow::new(11, 2016, 2100),
    SubtitleWindow::new(12, 2100, 2205),
    SubtitleWindow::new(13, 2484, 2517),
    SubtitleWindow::new(14, 2517, 2607),
    SubtitleWindow::new(15, 2649, 2760),
    SubtitleWindow::new(16, 3219, 3375),
    SubtitleWindow::new(17, 3375, 3486),
    SubtitleWindow::new(18, 3522, 3576),
    SubtitleWindow::new(19, 3576, 3639),
    SubtitleWindow::new(20, 3639, 3861),
    SubtitleWindow::new(21, 3957, 4122),
];

/// `demo_no == 72` (`killer_edi` - Meat Locker Eddie Boss Intro)
pub const SUBTITLES_KILLER_EDI: &[SubtitleWindow] = &[
    SubtitleWindow::new(0, 318, 408),
    SubtitleWindow::new(1, 408, 483),
    SubtitleWindow::new(2, 483, 558),
    SubtitleWindow::new(3, 558, 738),
    SubtitleWindow::new(4, 738, 846),
    SubtitleWindow::new(5, 846, 990),
    SubtitleWindow::new(6, 990, 1068),
    SubtitleWindow::new(7, 1068, 1206),
    SubtitleWindow::new(8, 1206, 1470),
    SubtitleWindow::new(9, 1470, 1593),
    SubtitleWindow::new(10, 1593, 1704),
    SubtitleWindow::new(11, 1704, 1812),
    SubtitleWindow::new(12, 1812, 1965),
    SubtitleWindow::new(13, 2046, 2181),
    SubtitleWindow::new(14, 2181, 2256),
    SubtitleWindow::new(15, 2256, 2394),
    SubtitleWindow::new(16, 2409, 2493),
    SubtitleWindow::new(17, 2493, 2574),
    SubtitleWindow::new(18, 2574, 2670),
    SubtitleWindow::new(19, 2670, 2739),
    SubtitleWindow::new(20, 2739, 2820),
];

/// `demo_no == 78` (`piano_lau_b` - Lakeview Hotel Piano Room Laura & Letter Part 1)
pub const SUBTITLES_PIANO_LAU: &[SubtitleWindow] = &[
    SubtitleWindow::new(0, 37, 94),
    SubtitleWindow::new(1, 94, 163),
    SubtitleWindow::new(2, 373, 499),
    SubtitleWindow::new(3, 499, 568),
    SubtitleWindow::new(4, 568, 685),
    SubtitleWindow::new(5, 685, 745),
    SubtitleWindow::new(6, 745, 850),
    SubtitleWindow::new(7, 850, 991),
    SubtitleWindow::new(8, 991, 1048),
    SubtitleWindow::new(9, 1048, 1117),
    SubtitleWindow::new(10, 1117, 1171),
    SubtitleWindow::new(11, 1225, 1303),
    SubtitleWindow::new(12, 1303, 1399),
    SubtitleWindow::new(13, 1399, 1450),
    SubtitleWindow::new(14, 1450, 1483),
    SubtitleWindow::new(15, 1483, 1588),
];

/// `demo_no == 78` (`piano_lau_e` - Lakeview Hotel Piano Room Laura & Letter Part 2)
pub const SUBTITLES_PIANO_LAU_E: &[SubtitleWindow] = &[
    SubtitleWindow::new(0, 84, 210),
    SubtitleWindow::new(1, 210, 369),
    SubtitleWindow::new(2, 369, 543),
    SubtitleWindow::new(3, 543, 651),
    SubtitleWindow::new(4, 651, 834),
    SubtitleWindow::new(5, 834, 921),
    SubtitleWindow::new(6, 921, 1062),
    SubtitleWindow::new(7, 1062, 1113),
    SubtitleWindow::new(8, 1113, 1191),
    SubtitleWindow::new(9, 1191, 1290),
    SubtitleWindow::new(10, 1290, 1341),
    SubtitleWindow::new(11, 1341, 1374),
    SubtitleWindow::new(12, 1374, 1467),
    SubtitleWindow::new(13, 1467, 1512),
];

/// `demo_no == 82` (`tomatta` - Lakeview Hotel Room 312 Videotape Stopped & Laura Truth)
pub const SUBTITLES_TOMATTA: &[SubtitleWindow] = &[
    SubtitleWindow::new(0, 1775, 1841),
    SubtitleWindow::new(1, 1841, 1916),
    SubtitleWindow::new(2, 1916, 1985),
    SubtitleWindow::new(3, 1985, 2036),
    SubtitleWindow::new(4, 2375, 2438),
    SubtitleWindow::new(5, 2438, 2534),
    SubtitleWindow::new(6, 2630, 2657),
    SubtitleWindow::new(7, 2657, 2720),
    SubtitleWindow::new(8, 2720, 2792),
    SubtitleWindow::new(9, 2792, 2999),
    SubtitleWindow::new(10, 2999, 3050),
    SubtitleWindow::new(11, 3050, 3155),
    SubtitleWindow::new(12, 3473, 3509),
    SubtitleWindow::new(13, 3509, 3587),
    SubtitleWindow::new(14, 3587, 3692),
    SubtitleWindow::new(15, 3692, 3779),
    SubtitleWindow::new(16, 3779, 3995),
    SubtitleWindow::new(17, 4112, 4340),
    SubtitleWindow::new(18, 4619, 4733),
    SubtitleWindow::new(19, 4826, 5012),
    SubtitleWindow::new(20, 5486, 5630),
    SubtitleWindow::new(21, 6065, 6161),
    SubtitleWindow::new(22, 6161, 6290),
    SubtitleWindow::new(23, 6290, 6362),
    SubtitleWindow::new(24, 6362, 6422),
    SubtitleWindow::new(25, 6422, 6500),
    SubtitleWindow::new(26, 6515, 6605),
    SubtitleWindow::new(27, 6605, 6680),
    SubtitleWindow::new(28, 6680, 6743),
    SubtitleWindow::new(29, 6743, 6887),
    SubtitleWindow::new(30, 6887, 7010),
    SubtitleWindow::new(31, 7010, 7079),
    SubtitleWindow::new(32, 7079, 7184),
    SubtitleWindow::new(33, 7184, 7244),
];

/// `demo_no == 81` (`vtr` - Lakeview Hotel Room 312 Inserting Videotape)
pub const SUBTITLES_VTR: &[SubtitleWindow] = &[];

/// `demo_no == 83` (`fire_agl` - Lakeview Hotel Burning Staircase Angela)
pub const SUBTITLES_FIRE_AGL: &[SubtitleWindow] = &[
    SubtitleWindow::new(0, 594, 642),
    SubtitleWindow::new(1, 642, 723),
    SubtitleWindow::new(2, 723, 801),
    SubtitleWindow::new(3, 801, 972),
    SubtitleWindow::new(4, 972, 1110),
    SubtitleWindow::new(5, 1233, 1335),
    SubtitleWindow::new(6, 1335, 1623),
    SubtitleWindow::new(7, 1623, 1689),
    SubtitleWindow::new(8, 1947, 2094),
    SubtitleWindow::new(9, 2094, 2214),
    SubtitleWindow::new(10, 2214, 2328),
    SubtitleWindow::new(11, 2328, 2388),
    SubtitleWindow::new(12, 2388, 2514),
    SubtitleWindow::new(13, 2667, 2937),
    SubtitleWindow::new(14, 3024, 3267),
    SubtitleWindow::new(15, 3267, 3477),
    SubtitleWindow::new(16, 3666, 3768),
    SubtitleWindow::new(17, 3999, 4239),
    SubtitleWindow::new(18, 4239, 4395),
    SubtitleWindow::new(19, 4395, 4512),
    SubtitleWindow::new(20, 4512, 4734),
    SubtitleWindow::new(21, 5304, 5409),
    SubtitleWindow::new(22, 5439, 5544),
    SubtitleWindow::new(23, 5583, 5748),
];

/// `demo_no == 86` (`kokuchi` - Hotel 2F Hallway Doctor's Diagnosis Flashback)
pub const SUBTITLES_RADIO_GERO: &[SubtitleWindow] = &[
    SubtitleWindow::new(0, 164, 386),
    SubtitleWindow::new(1, 386, 443),
    SubtitleWindow::new(2, 443, 551),
    SubtitleWindow::new(3, 551, 620),
    SubtitleWindow::new(4, 620, 686),
    SubtitleWindow::new(5, 686, 782),
    SubtitleWindow::new(6, 782, 923),
    SubtitleWindow::new(7, 923, 1004),
    SubtitleWindow::new(8, 1004, 1073),
    SubtitleWindow::new(9, 1073, 1202),
    SubtitleWindow::new(10, 1202, 1340),
];

/// `demo_no == 88` (`mry_yarinaoshi_a` - Leave / Maria Ending Final Confrontation Intro)
pub const SUBTITLES_MRY_YARINAOSHI_A: &[SubtitleWindow] = &[
    SubtitleWindow::new(0, 228, 273),
    SubtitleWindow::new(1, 501, 642),
    SubtitleWindow::new(2, 642, 744),
    SubtitleWindow::new(3, 744, 795),
    SubtitleWindow::new(4, 795, 930),
    SubtitleWindow::new(5, 930, 1068),
    SubtitleWindow::new(6, 1068, 1209),
    SubtitleWindow::new(7, 1209, 1317),
    SubtitleWindow::new(8, 1317, 1395),
    SubtitleWindow::new(9, 1395, 1581),
    SubtitleWindow::new(10, 1581, 1632),
    SubtitleWindow::new(11, 1632, 1761),
    SubtitleWindow::new(12, 1761, 1842),
    SubtitleWindow::new(13, 1842, 1962),
];

/// `demo_no == 96` (`fukatsu_a` - Rebirth Ending Final Confrontation)
pub const SUBTITLES_FUKATSU_A: &[SubtitleWindow] = &[
    SubtitleWindow::new(0, 339, 414),
    SubtitleWindow::new(1, 414, 447),
    SubtitleWindow::new(2, 447, 507),
    SubtitleWindow::new(3, 507, 636),
    SubtitleWindow::new(4, 636, 888),
    SubtitleWindow::new(5, 888, 1053),
    SubtitleWindow::new(6, 1053, 1161),
    SubtitleWindow::new(7, 1161, 1299),
    SubtitleWindow::new(8, 1299, 1425),
    SubtitleWindow::new(9, 1425, 1626),
    SubtitleWindow::new(10, 1626, 1677),
    SubtitleWindow::new(11, 1677, 1806),
];

/// `demo_no == 102` (`what_amy` - Born from a Wish Ernest Baldwin Mansion Door)
pub const SUBTITLES_GEKIJOU_A: &[SubtitleWindow] = &[
    SubtitleWindow::new(0, 166, 307),
    SubtitleWindow::new(1, 373, 481),
    SubtitleWindow::new(2, 481, 763),
    SubtitleWindow::new(3, 763, 880),
    SubtitleWindow::new(4, 952, 1006),
    SubtitleWindow::new(5, 1006, 1057),
    SubtitleWindow::new(6, 1057, 1165),
    SubtitleWindow::new(7, 1237, 1330),
    SubtitleWindow::new(8, 1330, 1693),
    SubtitleWindow::new(9, 1750, 1861),
    SubtitleWindow::new(10, 1861, 2092),
    SubtitleWindow::new(11, 2092, 2305),
    SubtitleWindow::new(12, 2380, 2578),
    SubtitleWindow::new(13, 2578, 2788),
    SubtitleWindow::new(14, 2788, 2899),
    SubtitleWindow::new(15, 2899, 2974),
    SubtitleWindow::new(16, 2974, 3247),
    SubtitleWindow::new(17, 3247, 3382),
    SubtitleWindow::new(18, 3382, 3535),
    SubtitleWindow::new(19, 3535, 3775),
    SubtitleWindow::new(20, 3775, 3865),
    SubtitleWindow::new(21, 3865, 3967),
    SubtitleWindow::new(22, 3967, 4054),
    SubtitleWindow::new(23, 4150, 4210),
    SubtitleWindow::new(24, 4567, 4738),
    SubtitleWindow::new(25, 4648, 4870),
    SubtitleWindow::new(26, 4870, 4915),
    SubtitleWindow::new(27, 4957, 5125),
    SubtitleWindow::new(28, 5125, 5200),
    SubtitleWindow::new(29, 5509, 5776),
    SubtitleWindow::new(30, 5776, 5926),
    SubtitleWindow::new(31, 5926, 5998),
    SubtitleWindow::new(32, 5998, 6169),
    SubtitleWindow::new(33, 6169, 6238),
    SubtitleWindow::new(34, 6238, 6355),
    SubtitleWindow::new(35, 6355, 6574),
    SubtitleWindow::new(36, 6697, 6766),
    SubtitleWindow::new(37, 6799, 6895),
    SubtitleWindow::new(38, 6895, 7087),
    SubtitleWindow::new(39, 7087, 7171),
    SubtitleWindow::new(40, 7366, 7648),
    SubtitleWindow::new(41, 7648, 7825),
    SubtitleWindow::new(42, 7825, 7990),
];

const SEQ_TENBOU: &[CutsceneSequenceStep] = &[CutsceneSequenceStep::InEngineDemo { demo_no: 2 }];
const SEQ_DEAI: &[CutsceneSequenceStep] = &[
    CutsceneSequenceStep::InEngineDemo { demo_no: 21 },
    CutsceneSequenceStep::FmvMovie { movie_id: "deai" },
    CutsceneSequenceStep::InEngineDemo { demo_no: 22 },
];
const SEQ_BP_EDI: &[CutsceneSequenceStep] = &[
    CutsceneSequenceStep::InEngineDemo { demo_no: 25 },
    CutsceneSequenceStep::InEngineDemo { demo_no: 26 },
];
const SEQ_PAPA_AGL: &[CutsceneSequenceStep] = &[
    CutsceneSequenceStep::InEngineDemo { demo_no: 67 },
    CutsceneSequenceStep::InEngineDemo { demo_no: 68 },
];
const SEQ_PIANO_LAU: &[CutsceneSequenceStep] = &[
    CutsceneSequenceStep::InEngineDemo { demo_no: 78 },
    CutsceneSequenceStep::InEngineDemo { demo_no: 79 },
];
const SEQ_VTR: &[CutsceneSequenceStep] = &[
    CutsceneSequenceStep::InEngineDemo { demo_no: 81 },
    CutsceneSequenceStep::FmvMovie { movie_id: "murder" },
    CutsceneSequenceStep::InEngineDemo { demo_no: 82 },
];
const SEQ_LEAVE_ENDING: &[CutsceneSequenceStep] = &[
    CutsceneSequenceStep::InEngineDemo { demo_no: 88 },
    CutsceneSequenceStep::FmvMovie { movie_id: "end" },
];

/// Curated catalog of major in-engine real-time 3D cutscenes (`DramaDemoPlayInfo`) across
/// *Letter from Silent Heaven* (`data/demo/`) and *Born from a Wish* (`data/demo2/`),
/// with exact `sh2pc.exe` `DramaDemo_PlayInfo` voice stream IDs, pre-roll start frames,
/// shared `.anm`/`.cls` sub-clip frame offsets, stage `.map` paths, and subtitle windows.
pub const DRAMA_DEMO_CATALOG: &[DramaDemoPlayInfo] = &[
    DramaDemoPlayInfo {
        demo_no: 1,
        id: "first_toilet",
        title: "Opening Restroom Mirror (first_toilet)",
        dds_path: "data/demo/first_toilet/first_toilet.dds",
        stage_map_path: "bg/er/er01.map",
        stage_name: "er01",
        mes_path: "data/etc/message/stage_toilet_msg_e.mes",
        msg_start: 0,
        subtitle_windows: SUBTITLES_FIRST_TOILET,
        stream_no: 60117,
        stream_start: 246,
        anm_frame_offset: 0,
        offset_x: 0.0,
        offset_z: 0.0,
        sequence_chain: &[],
    },
    DramaDemoPlayInfo {
        demo_no: 2,
        id: "tenbou",
        title: "Observation Deck Monologue (tenbou)",
        dds_path: "data/demo/tenbou/tenbou.dds",
        stage_map_path: "bg/ob/ob01.map",
        stage_name: "ob01",
        mes_path: "data/etc/message/stage_observation_msg_e.mes",
        msg_start: 7,
        subtitle_windows: SUBTITLES_TENBOU,
        stream_no: 60118,
        stream_start: 570,
        anm_frame_offset: 0,
        offset_x: 0.0,
        offset_z: 0.0,
        sequence_chain: SEQ_TENBOU,
    },
    DramaDemoPlayInfo {
        demo_no: 3,
        id: "haka_agl",
        title: "Graveyard: First Meeting with Angela (haka_agl)",
        dds_path: "data/demo/haka_agl/haka_agl.dds",
        stage_map_path: "bg/ca/ca11.map",
        stage_name: "ca11",
        mes_path: "data/etc/message/stage_forest_msg_e.mes",
        msg_start: 10,
        subtitle_windows: SUBTITLES_HAKA_AGL,
        stream_no: 60053,
        stream_start: 21,
        anm_frame_offset: 0,
        offset_x: 0.0,
        offset_z: 0.0,
        sequence_chain: &[],
    },
    DramaDemoPlayInfo {
        demo_no: 6,
        id: "first_battle",
        title: "Tunnel: Radio & First Lying Figure (saisho)",
        dds_path: "data/demo/saisho/saisho.dds",
        stage_map_path: "bg/cb/cb54.map",
        stage_name: "cb54",
        mes_path: "data/etc/message/stage_town_east_msg_e.mes",
        msg_start: 0,
        subtitle_windows: SUBTITLES_FIRST_BATTLE,
        stream_no: 60110,
        stream_start: 14,
        anm_frame_offset: 0,
        offset_x: 0.0,
        offset_z: 0.0,
        sequence_chain: &[],
    },
    DramaDemoPlayInfo {
        demo_no: 9,
        id: "kagikeri",
        title: "Wood Side Apt: Laura Kicks the Key (kagikeri)",
        dds_path: "data/demo/kagikeri/kagikeri.dds",
        stage_map_path: "bg/ap/ap104.map",
        stage_name: "ap104",
        mes_path: "data/etc/message/stage_apart_e3fw_msg_e.mes",
        msg_start: 5,
        subtitle_windows: SUBTITLES_KAGIKERI,
        stream_no: 60067,
        stream_start: 25,
        anm_frame_offset: 0,
        offset_x: 0.0,
        offset_z: 0.0,
        sequence_chain: &[],
    },
    DramaDemoPlayInfo {
        demo_no: 13,
        id: "gero_edi",
        title: "Wood Side Apt: Eddie in Bathroom (gero_edi)",
        dds_path: "data/demo/gero_edi/gero_edi.dds",
        stage_map_path: "bg/ap/ap100.map",
        stage_name: "ap100",
        mes_path: "data/etc/message/stage_apart_e1f_msg_e.mes",
        msg_start: 2,
        subtitle_windows: SUBTITLES_GERO_EDI,
        stream_no: 60048,
        stream_start: 39,
        anm_frame_offset: 0,
        offset_x: 0.0,
        offset_z: 0.0,
        sequence_chain: &[],
    },
    DramaDemoPlayInfo {
        demo_no: 14,
        id: "sankaku1",
        title: "Wood Side Apt Room 307: Pyramid Head Closet (sankaku1)",
        dds_path: "data/demo/sankaku1/sankaku.dds",
        stage_map_path: "bg/ap/ap30.map",
        stage_name: "ap30",
        mes_path: "data/etc/message/stage_apart_e3fe_msg_e.mes",
        msg_start: 0,
        subtitle_windows: SUBTITLES_SANKAKU1,
        stream_no: 60114,
        stream_start: 18,
        anm_frame_offset: 0,
        offset_x: 0.0,
        offset_z: 0.0,
        sequence_chain: &[],
    },
    DramaDemoPlayInfo {
        demo_no: 20,
        id: "hei_lau",
        title: "Outdoors: Laura on the Wall (hei_lau)",
        dds_path: "data/demo/hei_lau/hei_lau_a.dds",
        stage_map_path: "bg/cc/cc04.map",
        stage_name: "cc04",
        mes_path: "data/etc/message/stage_apart_out_msg_e.mes",
        msg_start: 0,
        subtitle_windows: SUBTITLES_HEI_LAU,
        stream_no: 60057,
        stream_start: 0,
        anm_frame_offset: 0,
        offset_x: 0.0,
        offset_z: 0.0,
        sequence_chain: &[],
    },
    DramaDemoPlayInfo {
        demo_no: 18,
        id: "knife_agl",
        title: "Blue Creek Apt: Angela's Knife (knife_agl)",
        dds_path: "data/demo/knife_agl/knife_agl.dds",
        stage_map_path: "bg/ap/ap84.map",
        stage_name: "ap84",
        mes_path: "data/etc/message/stage_apart_w1f_msg_e.mes",
        msg_start: 13,
        subtitle_windows: SUBTITLES_KNIFE_AGL,
        stream_no: 60075,
        stream_start: 0,
        anm_frame_offset: 0,
        offset_x: 0.0,
        offset_z: 0.0,
        sequence_chain: &[],
    },
    DramaDemoPlayInfo {
        demo_no: 21,
        id: "deai_c",
        title: "Rosewater Park Part 1: Meeting Maria Intro (mar_deai_c)",
        dds_path: "data/demo/mar_deai/mar_deai_c.dds",
        stage_map_path: "bg/cc/cc07.map",
        stage_name: "cc07",
        mes_path: "data/etc/message/stage_town_west_msg_e.mes",
        msg_start: 32,
        subtitle_windows: SUBTITLES_DEAI_C,
        stream_no: 60087,
        stream_start: 0,
        anm_frame_offset: 0,
        offset_x: 0.0,
        offset_z: 0.0,
        sequence_chain: SEQ_DEAI,
    },
    DramaDemoPlayInfo {
        demo_no: 22,
        id: "deai",
        title: "Rosewater Park Part 2: Meeting Maria (mar_deai_e)",
        dds_path: "data/demo/mar_deai/mar_deai_e.dds",
        stage_map_path: "bg/cc/cc07.map",
        stage_name: "cc07",
        mes_path: "data/etc/message/stage_town_west_msg_e.mes",
        msg_start: 44,
        subtitle_windows: SUBTITLES_DEAI,
        stream_no: 60088,
        stream_start: 0,
        anm_frame_offset: 1546,
        offset_x: 0.0,
        offset_z: 0.0,
        sequence_chain: SEQ_DEAI,
    },
    DramaDemoPlayInfo {
        demo_no: 25,
        id: "bp_edi_1",
        title: "Pete's Bowl-O-Rama Part 1: Laura & Eddie (bp_edi_a)",
        dds_path: "data/demo/bp_edi_1/bp_edi_a.dds",
        stage_map_path: "bg/bw/bw01.map",
        stage_name: "bw01",
        mes_path: "data/etc/message/stage_bowling_msg_e.mes",
        msg_start: 1,
        subtitle_windows: SUBTITLES_BP_EDI_1,
        stream_no: 60135,
        stream_start: 30,
        anm_frame_offset: 0,
        offset_x: 0.0,
        offset_z: 0.0,
        sequence_chain: SEQ_BP_EDI,
    },
    DramaDemoPlayInfo {
        demo_no: 26,
        id: "bp_edi_2",
        title: "Pete's Bowl-O-Rama Part 2: James & Eddie (bp_edi_c)",
        dds_path: "data/demo/bp_edi_2/bp_edi_c.dds",
        stage_map_path: "bg/bw/bw01.map",
        stage_name: "bw01",
        mes_path: "data/etc/message/stage_bowling_msg_e.mes",
        msg_start: 15,
        subtitle_windows: SUBTITLES_BP_EDI_2,
        stream_no: 60137,
        stream_start: 0,
        anm_frame_offset: 0,
        offset_x: 0.0,
        offset_z: 0.0,
        sequence_chain: SEQ_BP_EDI,
    },
    DramaDemoPlayInfo {
        demo_no: 45,
        id: "mar_saikai",
        title: "Labyrinth: Maria Cell Reunion (mar_saikai)",
        dds_path: "data/demo/mar_saikai/mar_saikai.dds",
        stage_map_path: "bg/hp/hp226.map",
        stage_name: "hp226",
        mes_path: "data/etc/message/stage_labyrinth_w_msg_e.mes",
        msg_start: 27,
        subtitle_windows: SUBTITLES_MAR_SAIKAI,
        stream_no: 60095,
        stream_start: 40,
        anm_frame_offset: 0,
        offset_x: 0.0,
        offset_z: 0.0,
        sequence_chain: &[],
    },
    DramaDemoPlayInfo {
        demo_no: 67,
        id: "papa_agl_a",
        title: "Labyrinth Part 1: Abstract Daddy Intro (papa_agl_a)",
        dds_path: "data/demo/papa_agl/papa_agl_a.dds",
        stage_map_path: "bg/ps/ps97.map",
        stage_name: "ps97",
        mes_path: "data/etc/message/stage_labyrinth_e_msg_e.mes",
        msg_start: 0,
        subtitle_windows: &[],
        stream_no: 60103,
        stream_start: 9,
        anm_frame_offset: 0,
        offset_x: 0.0,
        offset_z: 0.0,
        sequence_chain: SEQ_PAPA_AGL,
    },
    DramaDemoPlayInfo {
        demo_no: 68,
        id: "papa_agl",
        title: "Labyrinth Part 2: Angela & Abstract Daddy (papa_agl)",
        dds_path: "data/demo/papa_agl/papa_agl.dds",
        stage_map_path: "bg/ps/ps97.map",
        stage_name: "ps97",
        mes_path: "data/etc/message/stage_labyrinth_e_msg_e.mes",
        msg_start: 4,
        subtitle_windows: SUBTITLES_PAPA_AGL,
        stream_no: 60104,
        stream_start: 0,
        anm_frame_offset: 438,
        offset_x: 0.0,
        offset_z: 0.0,
        sequence_chain: SEQ_PAPA_AGL,
    },
    DramaDemoPlayInfo {
        demo_no: 72,
        id: "killer_edi",
        title: "Meat Locker: Eddie Boss Intro (killer_edi)",
        dds_path: "data/demo/killer_edi/killer_edi.dds",
        stage_map_path: "bg/ps/ps189.map",
        stage_name: "ps189",
        mes_path: "data/etc/message/stage_eddie_boss_msg_e.mes",
        msg_start: 1,
        subtitle_windows: SUBTITLES_KILLER_EDI,
        stream_no: 60070,
        stream_start: 0,
        anm_frame_offset: 0,
        offset_x: 0.0,
        offset_z: 0.0,
        sequence_chain: &[],
    },
    DramaDemoPlayInfo {
        demo_no: 78,
        id: "piano_lau",
        title: "Lakeview Hotel Part 1: Laura in Piano Room (piano_lau_b)",
        dds_path: "data/demo/piano_lau/piano_lau_b.dds",
        stage_map_path: "bg/rr/rr41.map",
        stage_name: "rr41",
        mes_path: "data/etc/message/stage_hotel_1f_f_msg_e.mes",
        msg_start: 30,
        subtitle_windows: SUBTITLES_PIANO_LAU,
        stream_no: 60105,
        stream_start: 0,
        anm_frame_offset: 0,
        offset_x: 0.0,
        offset_z: 0.0,
        sequence_chain: SEQ_PIANO_LAU,
    },
    DramaDemoPlayInfo {
        demo_no: 79,
        id: "piano_lau_e",
        title: "Lakeview Hotel Part 2: Laura's Letter (piano_lau_e)",
        dds_path: "data/demo/piano_lau/piano_lau_e.dds",
        stage_map_path: "bg/rr/rr41.map",
        stage_name: "rr41",
        mes_path: "data/etc/message/stage_hotel_1f_f_msg_e.mes",
        msg_start: 47,
        subtitle_windows: SUBTITLES_PIANO_LAU_E,
        stream_no: 60106,
        stream_start: 30,
        anm_frame_offset: 1679,
        offset_x: 0.0,
        offset_z: 0.0,
        sequence_chain: SEQ_PIANO_LAU,
    },
    DramaDemoPlayInfo {
        demo_no: 82,
        id: "vtr_s",
        title: "Room 312: After the Videotape & Laura (tomatta)",
        dds_path: "data/demo/room312/tomatta.dds",
        stage_map_path: "bg/rr/rr91.map",
        stage_name: "rr91",
        mes_path: "data/etc/message/stage_hotel_3f_f_msg_e.mes",
        msg_start: 14,
        subtitle_windows: SUBTITLES_TOMATTA,
        stream_no: 60099,
        stream_start: 125,
        anm_frame_offset: 0,
        offset_x: 0.0,
        offset_z: 0.0,
        sequence_chain: SEQ_VTR,
    },
    DramaDemoPlayInfo {
        demo_no: 81,
        id: "vtr",
        title: "Room 312: Inserting the Videotape (vtr)",
        dds_path: "data/demo/vtr/vtr.dds",
        stage_map_path: "bg/rr/rr91.map",
        stage_name: "rr91",
        mes_path: "data/etc/message/stage_hotel_3f_f_msg_e.mes",
        msg_start: 0,
        subtitle_windows: SUBTITLES_VTR,
        stream_no: 0,
        stream_start: 0,
        anm_frame_offset: 0,
        offset_x: 0.0,
        offset_z: 0.0,
        sequence_chain: SEQ_VTR,
    },
    DramaDemoPlayInfo {
        demo_no: 83,
        id: "fire_agl",
        title: "Hotel Burning Staircase: Angela's Farewell (fire_agl)",
        dds_path: "data/demo/fire_agl/fire_agl.dds",
        stage_map_path: "bg/ru/ru21.map",
        stage_name: "ru21",
        mes_path: "data/etc/message/stage_hotel_fire_msg_e.mes",
        msg_start: 0,
        subtitle_windows: SUBTITLES_FIRE_AGL,
        stream_no: 60010,
        stream_start: 24,
        anm_frame_offset: 0,
        offset_x: 0.0,
        offset_z: 0.0,
        sequence_chain: &[],
    },
    DramaDemoPlayInfo {
        demo_no: 86,
        id: "radio_gero",
        title: "Hotel 2F Hallway: Doctor's Diagnosis (kokuchi)",
        dds_path: "data/demo/kokuchi/kokuchi.dds",
        stage_map_path: "bg/ru/ru53.map",
        stage_name: "ru53",
        mes_path: "data/etc/message/stage_hotel_2f_b_msg_e.mes",
        msg_start: 3,
        subtitle_windows: SUBTITLES_RADIO_GERO,
        stream_no: 60076,
        stream_start: 80,
        anm_frame_offset: 0,
        offset_x: 0.0,
        offset_z: 0.0,
        sequence_chain: &[],
    },
    DramaDemoPlayInfo {
        demo_no: 88,
        id: "mry_yarinaoshi_a",
        title: "Final Confrontation: Mary/Maria Intro (mry_yarinaoshi_a)",
        dds_path: "data/demo/mry_yarinaoshi_a/mry_yarinaoshi_a.dds",
        stage_map_path: "bg/ru/ru73.map",
        stage_name: "ru73",
        mes_path: "data/etc/message/stage_end_recovery_msg_e.mes",
        msg_start: 0,
        subtitle_windows: SUBTITLES_MRY_YARINAOSHI_A,
        stream_no: 60127,
        stream_start: 0,
        anm_frame_offset: 0,
        offset_x: 0.0,
        offset_z: 0.0,
        sequence_chain: SEQ_LEAVE_ENDING,
    },
    DramaDemoPlayInfo {
        demo_no: 96,
        id: "fukatsu_a",
        title: "Rebirth Ending: Final Confrontation (end_rebirth_a)",
        dds_path: "data/demo/fukatsu_a/end_rebirth_a.dds",
        stage_map_path: "bg/ru/ru73.map",
        stage_name: "ru73",
        mes_path: "data/etc/message/stage_end_rebirth_msg_e.mes",
        msg_start: 0,
        subtitle_windows: SUBTITLES_FUKATSU_A,
        stream_no: 60011,
        stream_start: 3,
        anm_frame_offset: 0,
        offset_x: 0.0,
        offset_z: 0.0,
        sequence_chain: &[],
    },
    DramaDemoPlayInfo {
        demo_no: 102,
        id: "gekijou_a",
        title: "Born from a Wish: Baldwin Mansion (what_amy)",
        dds_path: "data/demo2/what_amy/what_amy.dds",
        stage_map_path: "bg/th/th01.map",
        stage_name: "th01",
        mes_path: "data/etc/message/stage_x_mansion_1f_msg_e.mes",
        msg_start: 0,
        subtitle_windows: SUBTITLES_GEKIJOU_A,
        stream_no: 60157,
        stream_start: 40,
        anm_frame_offset: 0,
        offset_x: 0.0,
        offset_z: 0.0,
        sequence_chain: &[],
    },
];

/// Finds a cutscene descriptor in [`DRAMA_DEMO_CATALOG`] by `demo_no`.
pub fn find_drama_demo_by_no(demo_no: u16) -> Option<&'static DramaDemoPlayInfo> {
    DRAMA_DEMO_CATALOG.iter().find(|d| d.demo_no == demo_no)
}

/// Finds a cutscene descriptor in [`DRAMA_DEMO_CATALOG`] by short ID (e.g. `"tenbou"`, `"haka_agl"`,
/// `"sankaku1"` / `"wp_closet"`, `"papa_agl"` / `"ug_angela"`).
pub fn find_drama_demo_by_id(id: &str) -> Option<&'static DramaDemoPlayInfo> {
    let normalized = match id.trim() {
        s if s.eq_ignore_ascii_case("wp_closet") => "sankaku1",
        s if s.eq_ignore_ascii_case("ug_angela") => "papa_agl",
        other => other,
    };
    DRAMA_DEMO_CATALOG
        .iter()
        .find(|d| d.id.eq_ignore_ascii_case(normalized))
}
