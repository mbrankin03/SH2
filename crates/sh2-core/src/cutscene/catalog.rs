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
    /// Stage world X coordinate offset applied to camera, lights, and actors (`offset_x`).
    pub offset_x: f32,
    /// Stage world Z coordinate offset applied to camera, lights, and actors (`offset_z`).
    pub offset_z: f32,
    /// Optional hybrid sequence chain when this cutscene transitions directly into/from an FMV or part B.
    pub sequence_chain: &'static [CutsceneSequenceStep],
}

impl DramaDemoPlayInfo {
    /// Returns the 0-based entry index in `data/sound/adx/voice/voice.afs` if `stream_no > 60000`.
    #[inline]
    pub const fn voice_afs_index(&self) -> Option<usize> {
        if self.stream_no > VOICE_AFS_STREAM_BASE {
            Some((self.stream_no - VOICE_AFS_STREAM_BASE - 1) as usize)
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

/// `demo_no == 1` (`tenbou` - Observation Deck monologue: "Mary... Could you really be in this town?")
pub const SUBTITLES_TENBOU: &[SubtitleWindow] = &[
    SubtitleWindow::new(0, 740, 810),
    SubtitleWindow::new(1, 825, 960),
];

/// `demo_no == 2` (`haka_agl` - Graveyard meeting with Angela Orosco)
pub const SUBTITLES_HAKA_AGL: &[SubtitleWindow] = &[
    SubtitleWindow::new(0, 290, 385),
    SubtitleWindow::new(1, 405, 560),
    SubtitleWindow::new(2, 590, 760),
    SubtitleWindow::new(3, 845, 1080),
    SubtitleWindow::new(4, 1120, 1460),
    SubtitleWindow::new(5, 1510, 1890),
    SubtitleWindow::new(6, 1940, 2320),
    SubtitleWindow::new(7, 2380, 2780),
    SubtitleWindow::new(8, 2820, 3390),
    SubtitleWindow::new(9, 3445, 3880),
    SubtitleWindow::new(10, 3910, 4150),
];

/// `demo_no == 6` (`first_battle` - Radio & First Lying Figure Encounter)
pub const SUBTITLES_FIRST_BATTLE: &[SubtitleWindow] = &[
    SubtitleWindow::new(0, 420, 560),
    SubtitleWindow::new(1, 1410, 1560),
    SubtitleWindow::new(2, 2360, 2500),
];

/// `demo_no == 11` (`gero_edi` - Wood Side Apartments Eddie Vomiting in Toilet)
pub const SUBTITLES_GERO_EDI: &[SubtitleWindow] = &[
    SubtitleWindow::new(0, 210, 360),
    SubtitleWindow::new(1, 410, 780),
    SubtitleWindow::new(2, 820, 1045),
    SubtitleWindow::new(3, 1080, 1520),
    SubtitleWindow::new(4, 1560, 2140),
    SubtitleWindow::new(5, 2180, 2675),
    SubtitleWindow::new(6, 2710, 3150),
    SubtitleWindow::new(7, 3190, 3470),
    SubtitleWindow::new(8, 3660, 4100),
];

/// `demo_no == 17` (`hei_lau` - Laura Steps on James's Hand on the Wall)
pub const SUBTITLES_HEI_LAU: &[SubtitleWindow] = &[
    SubtitleWindow::new(0, 290, 380),
    SubtitleWindow::new(1, 410, 520),
];

/// `demo_no == 19` (`knife_agl` - Blue Creek Apartments Angela Mirror & Knife)
pub const SUBTITLES_KNIFE_AGL: &[SubtitleWindow] = &[
    SubtitleWindow::new(0, 310, 545),
    SubtitleWindow::new(1, 575, 1020),
    SubtitleWindow::new(2, 1060, 1365),
    SubtitleWindow::new(3, 1400, 1890),
    SubtitleWindow::new(4, 1920, 2115),
    SubtitleWindow::new(5, 2145, 2810),
    SubtitleWindow::new(6, 2850, 3570),
    SubtitleWindow::new(7, 3610, 4410),
    SubtitleWindow::new(8, 4450, 4920),
    SubtitleWindow::new(9, 4950, 5470),
    SubtitleWindow::new(10, 5500, 5930),
];

/// `demo_no == 21` (`deai` - Rosewater Park Meeting Maria, Hybrid Demo -> `deai.bik` -> Demo)
pub const SUBTITLES_DEAI: &[SubtitleWindow] = &[
    SubtitleWindow::new(0, 260, 360),
    SubtitleWindow::new(1, 420, 760),
    SubtitleWindow::new(2, 800, 1280),
    SubtitleWindow::new(3, 1320, 1890),
    SubtitleWindow::new(4, 1940, 2560),
    SubtitleWindow::new(5, 2600, 3240),
    SubtitleWindow::new(6, 3290, 3950),
];

/// `demo_no == 25` (`bp_edi_1` / `bp_edi_2` - Pete's Bowl-O-Rama Laura & Eddie)
pub const SUBTITLES_BP_EDI_2: &[SubtitleWindow] = &[
    SubtitleWindow::new(0, 60, 380),
    SubtitleWindow::new(1, 410, 720),
    SubtitleWindow::new(2, 745, 1100),
    SubtitleWindow::new(3, 1125, 1275),
    SubtitleWindow::new(4, 1300, 1650),
    SubtitleWindow::new(5, 1680, 2095),
    SubtitleWindow::new(6, 2120, 2305),
];

/// `demo_no == 41` (`mar_saikai` - Labyrinth Maria Cell Reunion)
pub const SUBTITLES_MAR_SAIKAI: &[SubtitleWindow] = &[
    SubtitleWindow::new(0, 140, 480),
    SubtitleWindow::new(1, 510, 1120),
    SubtitleWindow::new(2, 1160, 1980),
    SubtitleWindow::new(3, 2020, 2890),
    SubtitleWindow::new(4, 2940, 3860),
    SubtitleWindow::new(5, 3910, 4720),
    SubtitleWindow::new(6, 4770, 5620),
];

/// `demo_no == 50` (`papa_agl` - Labyrinth Abstract Daddy & Angela)
pub const SUBTITLES_PAPA_AGL: &[SubtitleWindow] = &[
    SubtitleWindow::new(0, 120, 495),
    SubtitleWindow::new(1, 520, 1290),
    SubtitleWindow::new(2, 1320, 2140),
    SubtitleWindow::new(3, 2170, 2785),
    SubtitleWindow::new(4, 2815, 3500),
    SubtitleWindow::new(5, 3530, 4640),
];

/// `demo_no == 55` (`killer_edi` - Meat Locker Eddie Boss Intro)
pub const SUBTITLES_KILLER_EDI: &[SubtitleWindow] = &[
    SubtitleWindow::new(0, 110, 540),
    SubtitleWindow::new(1, 570, 955),
    SubtitleWindow::new(2, 980, 1520),
    SubtitleWindow::new(3, 1550, 1925),
    SubtitleWindow::new(4, 1955, 2460),
    SubtitleWindow::new(5, 2490, 2850),
];

/// `demo_no == 60` (`piano_lau` - Lakeview Hotel Piano Room Laura & Letter)
pub const SUBTITLES_PIANO_LAU: &[SubtitleWindow] = &[
    SubtitleWindow::new(0, 150, 580),
    SubtitleWindow::new(1, 610, 1240),
    SubtitleWindow::new(2, 1280, 1960),
    SubtitleWindow::new(3, 2000, 2780),
    SubtitleWindow::new(4, 2820, 3580),
    SubtitleWindow::new(5, 3620, 4390),
];

/// `demo_no == 63` (`vtr` - Lakeview Hotel Room 312 Videotape & Laura Truth)
pub const SUBTITLES_VTR: &[SubtitleWindow] = &[
    SubtitleWindow::new(0, 180, 560),
    SubtitleWindow::new(1, 1150, 1680),
    SubtitleWindow::new(2, 1720, 2380),
    SubtitleWindow::new(3, 2420, 3150),
    SubtitleWindow::new(4, 3190, 3980),
    SubtitleWindow::new(5, 4020, 4850),
];

/// `demo_no == 68` (`fire_agl` - Lakeview Hotel Burning Staircase Angela)
pub const SUBTITLES_FIRE_AGL: &[SubtitleWindow] = &[
    SubtitleWindow::new(0, 220, 680),
    SubtitleWindow::new(1, 710, 1095),
    SubtitleWindow::new(2, 1125, 1765),
    SubtitleWindow::new(3, 1795, 2640),
    SubtitleWindow::new(4, 2670, 3420),
    SubtitleWindow::new(5, 3450, 3950),
    SubtitleWindow::new(6, 3980, 4810),
    SubtitleWindow::new(7, 4840, 5680),
];

/// `demo_no == 77` (`radio_gero` - Room 312 Mary Radio Transmission, forced subtitles)
pub const SUBTITLES_RADIO_GERO: &[SubtitleWindow] = &[
    SubtitleWindow::new(0, 30, 260),
    SubtitleWindow::new(1, 280, 540),
    SubtitleWindow::new(2, 560, 820),
];

/// `demo_no == 80` (`mry_yarinaoshi_a` - Leave / Maria Ending Final Confrontation Intro)
pub const SUBTITLES_MRY_YARINAOSHI_A: &[SubtitleWindow] = &[
    SubtitleWindow::new(0, 140, 580),
    SubtitleWindow::new(1, 610, 1280),
    SubtitleWindow::new(2, 1320, 2040),
    SubtitleWindow::new(3, 2080, 2620),
];

/// `demo_no == 85` (`fukatsu_a` - Rebirth Ending Final Confrontation)
pub const SUBTITLES_FUKATSU_A: &[SubtitleWindow] = &[
    SubtitleWindow::new(0, 120, 540),
    SubtitleWindow::new(1, 570, 1180),
    SubtitleWindow::new(2, 1220, 1740),
];

/// `demo_no == 101` (`gekijou_a` - Born from a Wish Ernest Baldwin Mansion Door)
pub const SUBTITLES_GEKIJOU_A: &[SubtitleWindow] = &[
    SubtitleWindow::new(0, 110, 490),
    SubtitleWindow::new(1, 520, 1060),
    SubtitleWindow::new(2, 1100, 1680),
];

const SEQ_TENBOU: &[CutsceneSequenceStep] = &[CutsceneSequenceStep::InEngineDemo { demo_no: 1 }];
const SEQ_DEAI: &[CutsceneSequenceStep] = &[
    CutsceneSequenceStep::FmvMovie { movie_id: "deai" },
    CutsceneSequenceStep::InEngineDemo { demo_no: 21 },
];
const SEQ_BP_EDI: &[CutsceneSequenceStep] = &[
    CutsceneSequenceStep::InEngineDemo { demo_no: 24 },
    CutsceneSequenceStep::InEngineDemo { demo_no: 25 },
];
const SEQ_VTR: &[CutsceneSequenceStep] = &[
    CutsceneSequenceStep::InEngineDemo { demo_no: 62 },
    CutsceneSequenceStep::FmvMovie { movie_id: "murder" },
    CutsceneSequenceStep::InEngineDemo { demo_no: 63 },
];
const SEQ_LEAVE_ENDING: &[CutsceneSequenceStep] = &[
    CutsceneSequenceStep::InEngineDemo { demo_no: 80 },
    CutsceneSequenceStep::InEngineDemo { demo_no: 81 },
    CutsceneSequenceStep::FmvMovie { movie_id: "end" },
];

/// Curated catalog of major in-engine real-time 3D cutscenes (`DramaDemoPlayInfo`) across
/// *Letter from Silent Heaven* (`data/demo/`) and *Born from a Wish* (`data/demo2/`).
pub const DRAMA_DEMO_CATALOG: &[DramaDemoPlayInfo] = &[
    DramaDemoPlayInfo {
        demo_no: 1,
        id: "tenbou",
        title: "Observation Deck Monologue (tenbou)",
        dds_path: "data/demo/tenbou/tenbou.dds",
        mes_path: "data/etc/message/stage_observation_msg_e.mes",
        msg_start: 15,
        subtitle_windows: SUBTITLES_TENBOU,
        stream_no: 60001,
        stream_start: 716,
        offset_x: 0.0,
        offset_z: 0.0,
        sequence_chain: SEQ_TENBOU,
    },
    DramaDemoPlayInfo {
        demo_no: 2,
        id: "haka_agl",
        title: "Graveyard: First Meeting with Angela (haka_agl)",
        dds_path: "data/demo/haka_agl/haka_agl.dds",
        mes_path: "data/etc/message/stage_forest_msg_e.mes",
        msg_start: 1,
        subtitle_windows: SUBTITLES_HAKA_AGL,
        stream_no: 60002,
        stream_start: 280,
        offset_x: 0.0,
        offset_z: 0.0,
        sequence_chain: &[],
    },
    DramaDemoPlayInfo {
        demo_no: 6,
        id: "first_battle",
        title: "Tunnel: Radio & First Lying Figure (saisho)",
        dds_path: "data/demo/saisho/saisho.dds",
        mes_path: "data/etc/message/stage_town_east_msg_e.mes",
        msg_start: 10,
        subtitle_windows: SUBTITLES_FIRST_BATTLE,
        stream_no: 60004,
        stream_start: 400,
        offset_x: 0.0,
        offset_z: 0.0,
        sequence_chain: &[],
    },
    DramaDemoPlayInfo {
        demo_no: 7,
        id: "kagikeri",
        title: "Wood Side Apt: Laura Kicks the Key (kagikeri)",
        dds_path: "data/demo/kagikeri/kagikeri.dds",
        mes_path: "data/etc/message/stage_apart_e2f_msg_e.mes",
        msg_start: 20,
        subtitle_windows: &[],
        stream_no: 60006,
        stream_start: 120,
        offset_x: 0.0,
        offset_z: 0.0,
        sequence_chain: &[],
    },
    DramaDemoPlayInfo {
        demo_no: 11,
        id: "gero_edi",
        title: "Wood Side Apt: Eddie in Bathroom (gero_edi)",
        dds_path: "data/demo/gero_edi/gero_edi.dds",
        mes_path: "data/etc/message/stage_apart_e1f_msg_e.mes",
        msg_start: 35,
        subtitle_windows: SUBTITLES_GERO_EDI,
        stream_no: 60009,
        stream_start: 195,
        offset_x: 0.0,
        offset_z: 0.0,
        sequence_chain: &[],
    },
    DramaDemoPlayInfo {
        demo_no: 17,
        id: "hei_lau",
        title: "Outdoors: Laura on the Wall (hei_lau)",
        dds_path: "data/demo/hei_lau/hei_lau.dds",
        mes_path: "data/etc/message/stage_apart_out_msg_e.mes",
        msg_start: 5,
        subtitle_windows: SUBTITLES_HEI_LAU,
        stream_no: 60013,
        stream_start: 280,
        offset_x: 0.0,
        offset_z: 0.0,
        sequence_chain: &[],
    },
    DramaDemoPlayInfo {
        demo_no: 19,
        id: "knife_agl",
        title: "Blue Creek Apt: Angela's Knife (knife_agl)",
        dds_path: "data/demo/knife_agl/knife_agl.dds",
        mes_path: "data/etc/message/stage_apart_w1f_msg_e.mes",
        msg_start: 15,
        subtitle_windows: SUBTITLES_KNIFE_AGL,
        stream_no: 60014,
        stream_start: 295,
        offset_x: 0.0,
        offset_z: 0.0,
        sequence_chain: &[],
    },
    DramaDemoPlayInfo {
        demo_no: 21,
        id: "deai",
        title: "Rosewater Park: Meeting Maria (mar_deai)",
        dds_path: "data/demo/mar_deai/mar_deai.dds",
        mes_path: "data/etc/message/stage_town_west_msg_e.mes",
        msg_start: 1,
        subtitle_windows: SUBTITLES_DEAI,
        stream_no: 60016,
        stream_start: 245,
        offset_x: 0.0,
        offset_z: 0.0,
        sequence_chain: SEQ_DEAI,
    },
    DramaDemoPlayInfo {
        demo_no: 24,
        id: "bp_edi_1",
        title: "Pete's Bowl-O-Rama Part 1: Laura & Eddie (bp_edi_a)",
        dds_path: "data/demo/bp_edi_1/bp_edi_a.dds",
        mes_path: "data/etc/message/stage_bowling_msg_e.mes",
        msg_start: 1,
        subtitle_windows: &[],
        stream_no: 60019,
        stream_start: 40,
        offset_x: 0.0,
        offset_z: 0.0,
        sequence_chain: SEQ_BP_EDI,
    },
    DramaDemoPlayInfo {
        demo_no: 25,
        id: "bp_edi_2",
        title: "Pete's Bowl-O-Rama Part 2: James & Eddie (bp_edi_c)",
        dds_path: "data/demo/bp_edi_2/bp_edi_c.dds",
        mes_path: "data/etc/message/stage_bowling_msg_e.mes",
        msg_start: 8,
        subtitle_windows: SUBTITLES_BP_EDI_2,
        stream_no: 60020,
        stream_start: 50,
        offset_x: 0.0,
        offset_z: 0.0,
        sequence_chain: SEQ_BP_EDI,
    },
    DramaDemoPlayInfo {
        demo_no: 41,
        id: "mar_saikai",
        title: "Labyrinth: Maria Cell Reunion (mar_saikai)",
        dds_path: "data/demo/mar_saikai/mar_saikai.dds",
        mes_path: "data/etc/message/stage_labyrinth_w_msg_e.mes",
        msg_start: 1,
        subtitle_windows: SUBTITLES_MAR_SAIKAI,
        stream_no: 60038,
        stream_start: 120,
        offset_x: 0.0,
        offset_z: 0.0,
        sequence_chain: &[],
    },
    DramaDemoPlayInfo {
        demo_no: 50,
        id: "papa_agl",
        title: "Labyrinth: Angela & Abstract Daddy (papa_agl)",
        dds_path: "data/demo/papa_agl/papa_agl.dds",
        mes_path: "data/etc/message/stage_labyrinth_n_msg_e.mes",
        msg_start: 25,
        subtitle_windows: SUBTITLES_PAPA_AGL,
        stream_no: 60043,
        stream_start: 100,
        offset_x: 0.0,
        offset_z: 0.0,
        sequence_chain: &[],
    },
    DramaDemoPlayInfo {
        demo_no: 55,
        id: "killer_edi",
        title: "Meat Locker: Eddie Boss Intro (killer_edi)",
        dds_path: "data/demo/killer_edi/killer_edi.dds",
        mes_path: "data/etc/message/stage_eddie_boss_msg_e.mes",
        msg_start: 1,
        subtitle_windows: SUBTITLES_KILLER_EDI,
        stream_no: 60047,
        stream_start: 90,
        offset_x: 0.0,
        offset_z: 0.0,
        sequence_chain: &[],
    },
    DramaDemoPlayInfo {
        demo_no: 60,
        id: "piano_lau",
        title: "Lakeview Hotel: Laura in Piano Room (piano_lau)",
        dds_path: "data/demo/piano_lau/piano_lau.dds",
        mes_path: "data/etc/message/stage_hotel_2f_f_msg_e.mes",
        msg_start: 10,
        subtitle_windows: SUBTITLES_PIANO_LAU,
        stream_no: 60053,
        stream_start: 130,
        offset_x: 0.0,
        offset_z: 0.0,
        sequence_chain: &[],
    },
    DramaDemoPlayInfo {
        demo_no: 62,
        id: "vtr_s",
        title: "Room 312: Videotape Stopped (tomatta)",
        dds_path: "data/demo/room312/tomatta.dds",
        mes_path: "data/etc/message/stage_hotel_3f_f_msg_e.mes",
        msg_start: 1,
        subtitle_windows: &[],
        stream_no: 0,
        stream_start: 0,
        offset_x: 0.0,
        offset_z: 0.0,
        sequence_chain: SEQ_VTR,
    },
    DramaDemoPlayInfo {
        demo_no: 63,
        id: "vtr",
        title: "Room 312: After the Videotape & Laura (vtr)",
        dds_path: "data/demo/vtr/vtr.dds",
        mes_path: "data/etc/message/stage_hotel_3f_f_msg_e.mes",
        msg_start: 2,
        subtitle_windows: SUBTITLES_VTR,
        stream_no: 60055,
        stream_start: 160,
        offset_x: 0.0,
        offset_z: 0.0,
        sequence_chain: SEQ_VTR,
    },
    DramaDemoPlayInfo {
        demo_no: 68,
        id: "fire_agl",
        title: "Hotel Burning Staircase: Angela's Farewell (fire_agl)",
        dds_path: "data/demo/fire_agl/fire_agl.dds",
        mes_path: "data/etc/message/stage_hotel_fire_msg_e.mes",
        msg_start: 1,
        subtitle_windows: SUBTITLES_FIRE_AGL,
        stream_no: 60061,
        stream_start: 200,
        offset_x: 0.0,
        offset_z: 0.0,
        sequence_chain: &[],
    },
    DramaDemoPlayInfo {
        demo_no: 77,
        id: "radio_gero",
        title: "Room 312: Mary's Radio Transmission (kokuchi)",
        dds_path: "data/demo/kokuchi/kokuchi.dds",
        mes_path: "data/etc/message/stage_hotel_3f_b_msg_e.mes",
        msg_start: 1,
        subtitle_windows: SUBTITLES_RADIO_GERO,
        stream_no: 60064,
        stream_start: 20,
        offset_x: 0.0,
        offset_z: 0.0,
        sequence_chain: &[],
    },
    DramaDemoPlayInfo {
        demo_no: 80,
        id: "mry_yarinaoshi_a",
        title: "Final Confrontation: Mary/Maria Intro (mry_yarinaoshi_a)",
        dds_path: "data/demo/mry_yarinaoshi_a/mry_yarinaoshi_a.dds",
        mes_path: "data/etc/message/stage_end_recovery_msg_e.mes",
        msg_start: 1,
        subtitle_windows: SUBTITLES_MRY_YARINAOSHI_A,
        stream_no: 60070,
        stream_start: 120,
        offset_x: 0.0,
        offset_z: 0.0,
        sequence_chain: SEQ_LEAVE_ENDING,
    },
    DramaDemoPlayInfo {
        demo_no: 85,
        id: "fukatsu_a",
        title: "Rebirth Ending: Final Confrontation (end_rebirth_a)",
        dds_path: "data/demo/fukatsu_a/end_rebirth_a.dds",
        mes_path: "data/etc/message/stage_end_rebirth_msg_e.mes",
        msg_start: 1,
        subtitle_windows: SUBTITLES_FUKATSU_A,
        stream_no: 60080,
        stream_start: 100,
        offset_x: 0.0,
        offset_z: 0.0,
        sequence_chain: &[],
    },
    DramaDemoPlayInfo {
        demo_no: 101,
        id: "gekijou_a",
        title: "Born from a Wish: Baldwin Mansion (what_amy)",
        dds_path: "data/demo2/what_amy/what_amy.dds",
        mes_path: "data/etc/message/stage_x_mansion_1f_msg_e.mes",
        msg_start: 1,
        subtitle_windows: SUBTITLES_GEKIJOU_A,
        stream_no: 60105,
        stream_start: 90,
        offset_x: 0.0,
        offset_z: 0.0,
        sequence_chain: &[],
    },
];

/// Finds a cutscene descriptor in [`DRAMA_DEMO_CATALOG`] by `demo_no`.
pub fn find_drama_demo_by_no(demo_no: u16) -> Option<&'static DramaDemoPlayInfo> {
    DRAMA_DEMO_CATALOG.iter().find(|d| d.demo_no == demo_no)
}

/// Finds a cutscene descriptor in [`DRAMA_DEMO_CATALOG`] by short ID (e.g. `"tenbou"`, `"haka_agl"`).
pub fn find_drama_demo_by_id(id: &str) -> Option<&'static DramaDemoPlayInfo> {
    DRAMA_DEMO_CATALOG
        .iter()
        .find(|d| d.id.eq_ignore_ascii_case(id))
}
