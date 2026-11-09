//! In-Engine Real-Time 3D Cutscene Player (`Event/demoview.c`, `Chacter_Draw/clani.c`) (`SH2-63`).
//!
//! # Ground Truth & Architecture
//! Verified against:
//! - **Primary Ground Truth**: PS2 Debug Prototype ELF (`SLUS_202.28`, DWARF compile unit `Event/demoview.c`:
//!   `DramaDemoMain` at `0x1b2b30`, `DramaDemoInit` at `0x1b2080`, `DdsPlay` at `0x1b23c0`,
//!   `DramaDemoSkipLast` at `0x1b1be0`, `DramaDemoAnimationStart` at `0x1b2260`, `DramaDemoMessage` at `0x1b1f80`,
//!   `SubtitlesExec` at `0x1b3bf0`, and `DramaDemo_AnimInfo` at `0x2ba080`).
//! - **Secondary Ground Truth**: Retail PC binary (`sh2pc.exe` NA v1.0: `0x4a1980`, `0x4a0fc0`, `0x4a1240`,
//!   `0x4a09f0`, `DramaDemo_AnimInfo` at `0x88ed20`, linear light decay ramp table at `0x50cff0`,
//!   and per-demo FOV scale overrides at `0x4a176b`).
//! - **Tertiary Reference**: `sh23ds/port/source/demo.c` and `tools/sh2conv/demos.py`.
//!
//! ## Core Subsystems
//! 1. **`DramaDemo_AnimInfo` (`DRAMA_DEMO_ANIM_INFO`)**:
//!    Complete 84-entry table mapping `.dds` actor node names (`"hhh_jms_pos"`, `"agl_pos"`, `"lau_pos"`, etc.)
//!    to `kind_id`, `model_id`, `start_aid..=end_aid`, and `.mdl` / `.anm` / `.cls` asset paths.
//! 2. **Cutscene Catalog & Subtitle Windows (`DramaDemoPlayInfo`, `DRAMA_DEMO_CATALOG`, `SubtitleWindow`)**:
//!    Authentic metadata for in-engine real-time cutscenes (`demo_no`), `.dds` script paths, stage `.mes`
//!    files, `msg_start`, `adr_msg_time` (`{ start_frame, end_frame }`) subtitle windows, `voice.afs`
//!    stream indices (`stream_no = 60000 + afs_index`, `stream_start` pre-roll frame), stage `offset_x`/`offset_z`,
//!    per-demo FOV scale overrides, and hybrid cutscene-movie (`deai`, `vtr`, `bp_edi`, endings) step chains.
//! 3. **`DramaDemoPlayer` Runtime State Machine**:
//!    - Synchronizes the 30 FPS scene clock (`scene_frame`) with voice audio (`stream_start`) and an independent
//!      subtitle clock (`msg_frame`).
//!    - Evaluates camera eye position, interest target, roll rotation around the view axis, and vertical FOV
//!      (`scr_z = plane * 1.14702 * scale`, `fov_y = 2 * atan(224.0 / scr_z)`).
//!    - Evaluates dynamic scene lights (point/omni, spot, and directional lights with the 9-bucket PS2 linear
//!      decay ramp table `0x50cff0` and the `demo_no == 6` Lying Figure reveal lighting override).
//!    - Evaluates per-actor world transforms (`pos + offset`, `rot_y = PI`), `Key 0x11` sub-clip progression,
//!      and `Start` / `Esc` cutscene skipping (`DramaDemoSkipLast`) with fade-to-black and `CharaSetPos`
//!      final coordinate placement.

use glam::{Mat4, Vec3};
use serde::{Deserialize, Serialize};
use sh2_assets::formats::demo::{
    DemoDdsEvaluator, DemoDdsFile, DemoKeyframeState, DemoLightKind, DemoLightState,
    DEMO_MAX_CHARS, DEMO_MAX_LIGHTS,
};

/// Native keyframe rate of Silent Hill 2 `.dds`, `.anm`, and `.cls` cutscene data (30.0 FPS).
pub const CUTSCENE_NATIVE_FPS: f32 = 30.0;

/// Base stream number offset for `data/sound/adx/voice/voice.afs` (`stream_no = 60001 + afs_index` in 0-based AFS indexing).
pub const VOICE_AFS_STREAM_BASE: i32 = 60000;

/// Entry in the 84-element `DramaDemo_AnimInfo` table (`0x2ba080` in `SLUS_202.28`, `0x88ed20` in `sh2pc.exe`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DramaDemoAnimInfo {
    /// Actor node name in `.dds` header (up to 16 chars, e.g. `"hhh_jms_pos"`).
    pub actor_name: &'static str,
    /// Character kind identifier (`0x100` = James, `0x200` = Maria, `0x300` = Lying Figure, `0xD00` = Angela, `0xE00` = Laura, `0xF00` = Eddie, `0x1000` = Mary, `0x700` = Prop).
    pub kind_id: u16,
    /// Engine `ModelID` enum value (`1..=87`).
    pub model_id: u16,
    /// First animation ID (`aid`) triggered on the first `Key 0x11` event in the `.dds` script.
    pub start_aid: u16,
    /// Last animation ID (`aid`) in the sequential cutscene sub-clip chain.
    pub end_aid: u16,
    /// Relative `.mdl` asset path under `data/`.
    pub mdl_path: &'static str,
}

/// Complete 84-entry `DramaDemo_AnimInfo` table extracted from `sh2pc.exe` (`0x88ed20`) and verified against `SLUS_202.28` (`0x2ba080`).
pub const DRAMA_DEMO_ANIM_INFO: &[DramaDemoAnimInfo; 84] = &[
    DramaDemoAnimInfo {
        actor_name: "hhh_jms_pos",
        kind_id: 0x0100,
        model_id: 2,
        start_aid: 1501,
        end_aid: 1859,
        mdl_path: "data/chr/jms/hhh_jms.mdl",
    },
    DramaDemoAnimInfo {
        actor_name: "hhl_jms_pos",
        kind_id: 0x0100,
        model_id: 3,
        start_aid: 1501,
        end_aid: 1859,
        mdl_path: "data/chr/jms/hhl_jms.mdl",
    },
    DramaDemoAnimInfo {
        actor_name: "hll_jms_pos",
        kind_id: 0x0100,
        model_id: 4,
        start_aid: 1501,
        end_aid: 1859,
        mdl_path: "data/chr/jms/hll_jms.mdl",
    },
    DramaDemoAnimInfo {
        actor_name: "lll_jms_pos",
        kind_id: 0x0100,
        model_id: 5,
        start_aid: 1501,
        end_aid: 1859,
        mdl_path: "data/chr/jms/lll_jms.mdl",
    },
    DramaDemoAnimInfo {
        actor_name: "rhhh_jms_pos",
        kind_id: 0x0100,
        model_id: 6,
        start_aid: 1501,
        end_aid: 1859,
        mdl_path: "data/chr/jms/rhhh_jms.mdl",
    },
    DramaDemoAnimInfo {
        actor_name: "rhll_jms_pos",
        kind_id: 0x0100,
        model_id: 7,
        start_aid: 1501,
        end_aid: 1859,
        mdl_path: "data/chr/jms/rhll_jms.mdl",
    },
    DramaDemoAnimInfo {
        actor_name: "rlll_jms_pos",
        kind_id: 0x0100,
        model_id: 8,
        start_aid: 1501,
        end_aid: 1859,
        mdl_path: "data/chr/jms/rlll_jms.mdl",
    },
    DramaDemoAnimInfo {
        actor_name: "DJ_pos",
        kind_id: 0x0700,
        model_id: 82,
        start_aid: 7872,
        end_aid: 7875,
        mdl_path: "data/chr/item/dj.mdl",
    },
    DramaDemoAnimInfo {
        actor_name: "scu_pos",
        kind_id: 0x0300,
        model_id: 9,
        start_aid: 2501,
        end_aid: 2515,
        mdl_path: "data/chr/scu/scu.mdl",
    },
    DramaDemoAnimInfo {
        actor_name: "lau_pos",
        kind_id: 0x0E00,
        model_id: 23,
        start_aid: 4001,
        end_aid: 4050,
        mdl_path: "data/chr/lau/lau.mdl",
    },
    DramaDemoAnimInfo {
        actor_name: "rlau_pos",
        kind_id: 0x0700,
        model_id: 77,
        start_aid: 7864,
        end_aid: 7865,
        mdl_path: "data/chr/item/rlau.mdl",
    },
    DramaDemoAnimInfo {
        actor_name: "agl_pos",
        kind_id: 0x0D00,
        model_id: 21,
        start_aid: 3501,
        end_aid: 3531,
        mdl_path: "data/chr/agl/agl.mdl",
    },
    DramaDemoAnimInfo {
        actor_name: "ragl_pos",
        kind_id: 0x0D00,
        model_id: 22,
        start_aid: 3501,
        end_aid: 3531,
        mdl_path: "data/chr/agl/ragl.mdl",
    },
    DramaDemoAnimInfo {
        actor_name: "hhh_mar_pos",
        kind_id: 0x0200,
        model_id: 19,
        start_aid: 2001,
        end_aid: 2092,
        mdl_path: "data/chr/mar/hhh_mar.mdl",
    },
    DramaDemoAnimInfo {
        actor_name: "lll_mar_pos",
        kind_id: 0x0200,
        model_id: 20,
        start_aid: 2001,
        end_aid: 2092,
        mdl_path: "data/chr/mar/lll_mar.mdl",
    },
    DramaDemoAnimInfo {
        actor_name: "rhhh_mar_pos",
        kind_id: 0x0700,
        model_id: 80,
        start_aid: 7869,
        end_aid: 7870,
        mdl_path: "data/chr/item/rhhh_mar.mdl",
    },
    DramaDemoAnimInfo {
        actor_name: "hhh_edi_pos",
        kind_id: 0x0F00,
        model_id: 24,
        start_aid: 4501,
        end_aid: 4527,
        mdl_path: "data/chr/edi/hhh_edi.mdl",
    },
    DramaDemoAnimInfo {
        actor_name: "lll_edi_pos",
        kind_id: 0x0F00,
        model_id: 25,
        start_aid: 4501,
        end_aid: 4527,
        mdl_path: "data/chr/edi/lll_edi.mdl",
    },
    DramaDemoAnimInfo {
        actor_name: "rhhh_edi_pos",
        kind_id: 0x0F00,
        model_id: 26,
        start_aid: 4501,
        end_aid: 4527,
        mdl_path: "data/chr/edi/rhhh_edi.mdl",
    },
    DramaDemoAnimInfo {
        actor_name: "mry_pos",
        kind_id: 0x1000,
        model_id: 27,
        start_aid: 5001,
        end_aid: 5055,
        mdl_path: "data/chr/mry/mry.mdl",
    },
    DramaDemoAnimInfo {
        actor_name: "rmry_pos",
        kind_id: 0x0700,
        model_id: 78,
        start_aid: 7866,
        end_aid: 7866,
        mdl_path: "data/chr/item/rmry.mdl",
    },
    DramaDemoAnimInfo {
        actor_name: "oni_pos",
        kind_id: 0x0300,
        model_id: 13,
        start_aid: 3001,
        end_aid: 3003,
        mdl_path: "data/chr/oni/oni.mdl",
    },
    DramaDemoAnimInfo {
        actor_name: "red_pos",
        kind_id: 0x0A00,
        model_id: 12,
        start_aid: 3001,
        end_aid: 3022,
        mdl_path: "data/chr/red/red.mdl",
    },
    DramaDemoAnimInfo {
        actor_name: "pap_pos",
        kind_id: 0x0C00,
        model_id: 17,
        start_aid: 3301,
        end_aid: 3307,
        mdl_path: "data/chr/pap/pap.mdl",
    },
    DramaDemoAnimInfo {
        actor_name: "bos_pos",
        kind_id: 0x1100,
        model_id: 18,
        start_aid: 3401,
        end_aid: 3403,
        mdl_path: "data/chr/bos/bos.mdl",
    },
    DramaDemoAnimInfo {
        actor_name: "tyu_pos",
        kind_id: 0x0600,
        model_id: 15,
        start_aid: 3101,
        end_aid: 3106,
        mdl_path: "data/chr/tyu/tyu.mdl",
    },
    DramaDemoAnimInfo {
        actor_name: "mkn_pos",
        kind_id: 0x0500,
        model_id: 11,
        start_aid: 2901,
        end_aid: 2909,
        mdl_path: "data/chr/mkn/mkn.mdl",
    },
    DramaDemoAnimInfo {
        actor_name: "ike_pos",
        kind_id: 0x0B00,
        model_id: 16,
        start_aid: 3201,
        end_aid: 3202,
        mdl_path: "data/chr/ike/ike.mdl",
    },
    DramaDemoAnimInfo {
        actor_name: "p_light_pos",
        kind_id: 0x0700,
        model_id: 28,
        start_aid: 7501,
        end_aid: 7516,
        mdl_path: "data/chr/item/p_light.mdl",
    },
    DramaDemoAnimInfo {
        actor_name: "b_d00_pos",
        kind_id: 0x0700,
        model_id: 29,
        start_aid: 7517,
        end_aid: 7541,
        mdl_path: "data/chr/item/b_d00.mdl",
    },
    DramaDemoAnimInfo {
        actor_name: "b_s07_pos",
        kind_id: 0x0700,
        model_id: 30,
        start_aid: 7542,
        end_aid: 7544,
        mdl_path: "data/chr/item/b_s07.mdl",
    },
    DramaDemoAnimInfo {
        actor_name: "b_d10_pos",
        kind_id: 0x0700,
        model_id: 31,
        start_aid: 7545,
        end_aid: 7550,
        mdl_path: "data/chr/item/b_d10.mdl",
    },
    DramaDemoAnimInfo {
        actor_name: "nad_pos",
        kind_id: 0x0700,
        model_id: 32,
        start_aid: 7551,
        end_aid: 7554,
        mdl_path: "data/chr/item/nad.mdl",
    },
    DramaDemoAnimInfo {
        actor_name: "b_c01_pos",
        kind_id: 0x0700,
        model_id: 33,
        start_aid: 7555,
        end_aid: 7561,
        mdl_path: "data/chr/item/b_c01.mdl",
    },
    DramaDemoAnimInfo {
        actor_name: "c12_pos",
        kind_id: 0x0700,
        model_id: 34,
        start_aid: 7562,
        end_aid: 7565,
        mdl_path: "data/chr/item/c12.mdl",
    },
    DramaDemoAnimInfo {
        actor_name: "d22_pos",
        kind_id: 0x0700,
        model_id: 35,
        start_aid: 7566,
        end_aid: 7582,
        mdl_path: "data/chr/item/d22.mdl",
    },
    DramaDemoAnimInfo {
        actor_name: "e02_pos",
        kind_id: 0x0700,
        model_id: 36,
        start_aid: 7583,
        end_aid: 7589,
        mdl_path: "data/chr/item/e02.mdl",
    },
    DramaDemoAnimInfo {
        actor_name: "f09_pos",
        kind_id: 0x0700,
        model_id: 37,
        start_aid: 7590,
        end_aid: 7595,
        mdl_path: "data/chr/item/f09.mdl",
    },
    DramaDemoAnimInfo {
        actor_name: "g05_pos",
        kind_id: 0x0700,
        model_id: 38,
        start_aid: 7596,
        end_aid: 7606,
        mdl_path: "data/chr/item/g05.mdl",
    },
    DramaDemoAnimInfo {
        actor_name: "h03_pos",
        kind_id: 0x0700,
        model_id: 39,
        start_aid: 7607,
        end_aid: 7609,
        mdl_path: "data/chr/item/h03.mdl",
    },
    DramaDemoAnimInfo {
        actor_name: "i02_pos",
        kind_id: 0x0700,
        model_id: 40,
        start_aid: 7610,
        end_aid: 7617,
        mdl_path: "data/chr/item/i02.mdl",
    },
    DramaDemoAnimInfo {
        actor_name: "j00_pos",
        kind_id: 0x0700,
        model_id: 41,
        start_aid: 7618,
        end_aid: 7620,
        mdl_path: "data/chr/item/j00.mdl",
    },
    DramaDemoAnimInfo {
        actor_name: "k00_pos",
        kind_id: 0x0700,
        model_id: 42,
        start_aid: 7621,
        end_aid: 7627,
        mdl_path: "data/chr/item/k00.mdl",
    },
    DramaDemoAnimInfo {
        actor_name: "l08_pos",
        kind_id: 0x0700,
        model_id: 43,
        start_aid: 7628,
        end_aid: 7632,
        mdl_path: "data/chr/item/l08.mdl",
    },
    DramaDemoAnimInfo {
        actor_name: "m02_pos",
        kind_id: 0x0700,
        model_id: 44,
        start_aid: 7633,
        end_aid: 7651,
        mdl_path: "data/chr/item/m02.mdl",
    },
    DramaDemoAnimInfo {
        actor_name: "n02_pos",
        kind_id: 0x0700,
        model_id: 45,
        start_aid: 7652,
        end_aid: 7660,
        mdl_path: "data/chr/item/n02.mdl",
    },
    DramaDemoAnimInfo {
        actor_name: "o00_pos",
        kind_id: 0x0700,
        model_id: 46,
        start_aid: 7661,
        end_aid: 7679,
        mdl_path: "data/chr/item/o00.mdl",
    },
    DramaDemoAnimInfo {
        actor_name: "p06_pos",
        kind_id: 0x0700,
        model_id: 47,
        start_aid: 7680,
        end_aid: 7681,
        mdl_path: "data/chr/item/p06.mdl",
    },
    DramaDemoAnimInfo {
        actor_name: "q09_pos",
        kind_id: 0x0700,
        model_id: 48,
        start_aid: 7682,
        end_aid: 7687,
        mdl_path: "data/chr/item/q09.mdl",
    },
    DramaDemoAnimInfo {
        actor_name: "r01_pos",
        kind_id: 0x0700,
        model_id: 49,
        start_aid: 7688,
        end_aid: 7697,
        mdl_path: "data/chr/item/r01.mdl",
    },
    DramaDemoAnimInfo {
        actor_name: "s00_pos",
        kind_id: 0x0700,
        model_id: 50,
        start_aid: 7698,
        end_aid: 7707,
        mdl_path: "data/chr/item/s00.mdl",
    },
    DramaDemoAnimInfo {
        actor_name: "t05_pos",
        kind_id: 0x0700,
        model_id: 51,
        start_aid: 7708,
        end_aid: 7716,
        mdl_path: "data/chr/item/t05.mdl",
    },
    DramaDemoAnimInfo {
        actor_name: "u04_pos",
        kind_id: 0x0700,
        model_id: 52,
        start_aid: 7717,
        end_aid: 7724,
        mdl_path: "data/chr/item/u04.mdl",
    },
    DramaDemoAnimInfo {
        actor_name: "v09_pos",
        kind_id: 0x0700,
        model_id: 53,
        start_aid: 7725,
        end_aid: 7734,
        mdl_path: "data/chr/item/v09.mdl",
    },
    DramaDemoAnimInfo {
        actor_name: "w03_pos",
        kind_id: 0x0700,
        model_id: 54,
        start_aid: 7735,
        end_aid: 7742,
        mdl_path: "data/chr/item/w03.mdl",
    },
    DramaDemoAnimInfo {
        actor_name: "x02_pos",
        kind_id: 0x0700,
        model_id: 55,
        start_aid: 7743,
        end_aid: 7755,
        mdl_path: "data/chr/item/x02.mdl",
    },
    DramaDemoAnimInfo {
        actor_name: "y08_pos",
        kind_id: 0x0700,
        model_id: 56,
        start_aid: 7756,
        end_aid: 7760,
        mdl_path: "data/chr/item/y08.mdl",
    },
    DramaDemoAnimInfo {
        actor_name: "z00_pos",
        kind_id: 0x0700,
        model_id: 57,
        start_aid: 7761,
        end_aid: 7764,
        mdl_path: "data/chr/item/z00.mdl",
    },
    DramaDemoAnimInfo {
        actor_name: "z06_pos",
        kind_id: 0x0700,
        model_id: 58,
        start_aid: 7765,
        end_aid: 7774,
        mdl_path: "data/chr/item/z06.mdl",
    },
    DramaDemoAnimInfo {
        actor_name: "ed2_pos",
        kind_id: 0x0700,
        model_id: 59,
        start_aid: 7775,
        end_aid: 7777,
        mdl_path: "data/chr/item/ed2.mdl",
    },
    DramaDemoAnimInfo {
        actor_name: "org_pos",
        kind_id: 0x0700,
        model_id: 60,
        start_aid: 7778,
        end_aid: 7786,
        mdl_path: "data/chr/item/org.mdl",
    },
    DramaDemoAnimInfo {
        actor_name: "piz_pos",
        kind_id: 0x0700,
        model_id: 61,
        start_aid: 7787,
        end_aid: 7792,
        mdl_path: "data/chr/item/piz.mdl",
    },
    DramaDemoAnimInfo {
        actor_name: "x13_pos",
        kind_id: 0x0700,
        model_id: 62,
        start_aid: 7793,
        end_aid: 7802,
        mdl_path: "data/chr/item/x13.mdl",
    },
    DramaDemoAnimInfo {
        actor_name: "c06_pos",
        kind_id: 0x0700,
        model_id: 63,
        start_aid: 7803,
        end_aid: 7804,
        mdl_path: "data/chr/item/c06.mdl",
    },
    DramaDemoAnimInfo {
        actor_name: "m03_pos",
        kind_id: 0x0700,
        model_id: 64,
        start_aid: 7805,
        end_aid: 7811,
        mdl_path: "data/chr/item/m03.mdl",
    },
    DramaDemoAnimInfo {
        actor_name: "fg_pos",
        kind_id: 0x0700,
        model_id: 65,
        start_aid: 7812,
        end_aid: 7816,
        mdl_path: "data/chr/item/fg.mdl",
    },
    DramaDemoAnimInfo {
        actor_name: "l06_pos",
        kind_id: 0x0700,
        model_id: 66,
        start_aid: 7817,
        end_aid: 7824,
        mdl_path: "data/chr/item/l06.mdl",
    },
    DramaDemoAnimInfo {
        actor_name: "b_a04_pos",
        kind_id: 0x0700,
        model_id: 67,
        start_aid: 7825,
        end_aid: 7826,
        mdl_path: "data/chr/item/b_a04.mdl",
    },
    DramaDemoAnimInfo {
        actor_name: "tl_pos",
        kind_id: 0x0700,
        model_id: 68,
        start_aid: 7827,
        end_aid: 7830,
        mdl_path: "data/chr/item/tl.mdl",
    },
    DramaDemoAnimInfo {
        actor_name: "b_bol_pos",
        kind_id: 0x0700,
        model_id: 69,
        start_aid: 7831,
        end_aid: 7835,
        mdl_path: "data/chr/item/b_bol.mdl",
    },
    DramaDemoAnimInfo {
        actor_name: "kum_pos",
        kind_id: 0x0700,
        model_id: 70,
        start_aid: 7836,
        end_aid: 7843,
        mdl_path: "data/chr/item/kum.mdl",
    },
    DramaDemoAnimInfo {
        actor_name: "ls_pos",
        kind_id: 0x0700,
        model_id: 71,
        start_aid: 7844,
        end_aid: 7847,
        mdl_path: "data/chr/item/ls.mdl",
    },
    DramaDemoAnimInfo {
        actor_name: "v08_pos",
        kind_id: 0x0700,
        model_id: 72,
        start_aid: 7848,
        end_aid: 7850,
        mdl_path: "data/chr/item/v08.mdl",
    },
    DramaDemoAnimInfo {
        actor_name: "n01_pos",
        kind_id: 0x0700,
        model_id: 73,
        start_aid: 7851,
        end_aid: 7853,
        mdl_path: "data/chr/item/n01.mdl",
    },
    DramaDemoAnimInfo {
        actor_name: "c54_pos",
        kind_id: 0x0700,
        model_id: 74,
        start_aid: 7854,
        end_aid: 7857,
        mdl_path: "data/chr/item/c54.mdl",
    },
    DramaDemoAnimInfo {
        actor_name: "r99_pos",
        kind_id: 0x0700,
        model_id: 75,
        start_aid: 7858,
        end_aid: 7860,
        mdl_path: "data/chr/item/r99.mdl",
    },
    DramaDemoAnimInfo {
        actor_name: "x22_pos",
        kind_id: 0x0700,
        model_id: 76,
        start_aid: 7861,
        end_aid: 7863,
        mdl_path: "data/chr/item/x22.mdl",
    },
    DramaDemoAnimInfo {
        actor_name: "b_dt0_pos",
        kind_id: 0x0700,
        model_id: 79,
        start_aid: 7867,
        end_aid: 7868,
        mdl_path: "data/chr/item/b_dt0.mdl",
    },
    DramaDemoAnimInfo {
        actor_name: "c16_pos",
        kind_id: 0x0700,
        model_id: 81,
        start_aid: 7871,
        end_aid: 7871,
        mdl_path: "data/chr/item/c16.mdl",
    },
    DramaDemoAnimInfo {
        actor_name: "rlll_mar_pos",
        kind_id: 0x0700,
        model_id: 83,
        start_aid: 7876,
        end_aid: 7878,
        mdl_path: "data/chr/item/rlll_mar.mdl",
    },
    DramaDemoAnimInfo {
        actor_name: "mxx_pos",
        kind_id: 0x1000,
        model_id: 84,
        start_aid: 5001,
        end_aid: 5055,
        mdl_path: "data/chr/mxx/mxx.mdl",
    },
    DramaDemoAnimInfo {
        actor_name: "c91_pos",
        kind_id: 0x0700,
        model_id: 85,
        start_aid: 7879,
        end_aid: 7886,
        mdl_path: "data/chr/item/c91.mdl",
    },
    DramaDemoAnimInfo {
        actor_name: "wp_wall_pos",
        kind_id: 0x0700,
        model_id: 86,
        start_aid: 7887,
        end_aid: 7892,
        mdl_path: "data/chr2/item/wp_wall.mdl",
    },
    DramaDemoAnimInfo {
        actor_name: "ri02_pos",
        kind_id: 0x0700,
        model_id: 87,
        start_aid: 7893,
        end_aid: 7898,
        mdl_path: "data/chr2/item/ri02.mdl",
    },
];

/// Looks up the [`DramaDemoAnimInfo`] entry for a `.dds` actor name (e.g. `"hhh_jms_pos"`).
pub fn find_drama_demo_anim_info(actor_name: &str) -> Option<&'static DramaDemoAnimInfo> {
    let trimmed = actor_name.trim_matches('\0').trim();
    DRAMA_DEMO_ANIM_INFO
        .iter()
        .find(|info| info.actor_name.eq_ignore_ascii_case(trimmed))
}

/// Derives the per-demo `.anm` and `.cls` file stems for a `.dds` actor node name within a demo folder.
///
/// For example, in `"data/demo/haka_agl/haka_agl.dds"`:
/// - `"hhh_jms_pos"` -> `("data/demo/haka_agl/hhh_jms.anm", Some("data/demo/haka_agl/hhh_jms.cls"))`
/// - `"agl_pos"` -> `("data/demo/haka_agl/agl.anm", Some("data/demo/haka_agl/agl.cls"))`
pub fn derive_demo_actor_anm_and_cls_paths(
    dds_rel_path: &str,
    actor_name: &str,
) -> (String, Option<String>) {
    let dir = dds_rel_path
        .rsplit_once('/')
        .map(|(d, _)| d)
        .unwrap_or("data/demo");
    let stem = actor_name
        .trim_matches('\0')
        .trim()
        .strip_suffix("_pos")
        .unwrap_or(actor_name);
    // High-LOD cutscene `.anm`/`.cls` files for `hhl_jms_pos` are named `hhh_jms.anm`/`hhh_jms.cls` inside demo folders
    let file_stem = if stem.eq_ignore_ascii_case("hhl_jms") {
        "hhh_jms"
    } else {
        stem
    };
    let anm_path = format!("{dir}/{file_stem}.anm");
    let has_cls = matches!(
        file_stem,
        "hhh_jms" | "hhh_mar" | "agl" | "ragl" | "hhh_edi" | "rhhh_edi" | "lau" | "mry" | "mxx"
    );
    let cls_path = has_cls.then(|| format!("{dir}/{file_stem}.cls"));
    (anm_path, cls_path)
}

/// Authentic PS2/PC 9-bucket linear distance decay ramp table (`0x50cff0` in `sh2pc.exe`).
///
/// Each entry is `[near_distance, far_distance]` in PS2 world units (mm) selected by
/// `(trunc(light.range / 1500.0) as usize).clamp(0, 8)`.
pub const DEMO_LIGHT_DECAY_RAMP: [[f32; 2]; 9] = [
    [500.0, 2000.0],
    [1000.0, 3000.0],
    [1000.0, 4000.0],
    [1500.0, 4500.0],
    [1500.0, 5000.0],
    [1500.0, 5500.0],
    [2000.0, 6000.0],
    [2000.0, 6500.0],
    [3000.0, 7500.0],
];

/// Returns the `(near_decay, far_decay)` distance ramp for a point or spot light range
/// matching `DdsPlay` (`0x4a1240` in `sh2pc.exe`).
#[inline]
pub fn demo_light_decay_bounds(range: f32) -> (f32, f32) {
    let idx = ((range.max(0.0) / 1500.0).trunc() as usize).min(8);
    let pair = DEMO_LIGHT_DECAY_RAMP[idx];
    (pair[0], pair[1])
}

/// Per-demo FOV scale multiplier matching `DdsPlay` (`0x4a176b` in `sh2pc.exe` / `0x1b23c0` in `SLUS_202.28`).
#[inline]
pub const fn demo_fov_scale_for_demo_no(demo_no: u16) -> f32 {
    match demo_no {
        7 | 90 => 1.25,
        93 => 1.4,
        100 => 1.35,
        _ => 1.0,
    }
}

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
    /// Relative path to the stage `.mes` dialogue file (e.g. `"data/pic/mes/stage_ob.mes"`).
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

// ============================================================================
// Authentic Subtitle Timing Tables (`adr_msg_time` from `sh2pc.exe` / `SLUS_202.28`)
// ============================================================================

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

// Hybrid cutscene-movie step chains
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

/// Runtime state of a single actor inside an active `.dds` cutscene (`SubCharacter` in `Event/demoview.c`).
#[derive(Debug, Clone)]
pub struct CutsceneActorRuntime {
    /// Actor slot index (`0..n_chars`).
    pub slot_index: usize,
    /// Raw actor name from `.dds` header (e.g. `"hhh_jms_pos"`).
    pub actor_name: String,
    /// Resolved `DramaDemo_AnimInfo` entry if present in the 84-entry table.
    pub anim_info: Option<&'static DramaDemoAnimInfo>,
    /// Derived `.anm` relative asset path for this demo folder.
    pub anm_path: String,
    /// Derived `.cls` facial morph animation relative asset path if applicable.
    pub cls_path: Option<String>,
    /// Whether this actor is currently visible (`status & 1`).
    pub visible: bool,
    /// Current world-space position (`pos + offset`).
    pub position: Vec3,
    /// Current Y rotation in radians (`PI` during `DdsPlay`).
    pub yaw_radians: f32,
    /// Current animation ID (`aid`, initialized to `start_aid` and incremented on each `Key 0x11` after the first).
    pub current_aid: u16,
    /// Number of `Key 0x11` (`DramaDemoAnimationStart`) triggers processed for this actor.
    pub subclip_index: usize,
    /// Continuous 30 FPS animation frame within the current sub-clip (`scene_frame - subclip_start_frame`).
    pub local_anm_frame: f32,
}

impl CutsceneActorRuntime {
    /// Computes the 4x4 row-major world transformation matrix for this cutscene actor (`rot_y = PI`, `pos = position`).
    pub fn world_matrix_row_major(&self) -> [[f32; 4]; 4] {
        let (s, c) = self.yaw_radians.sin_cos();
        [
            [c, 0.0, s, self.position.x],
            [0.0, 1.0, 0.0, self.position.y],
            [-s, 0.0, c, self.position.z],
            [0.0, 0.0, 0.0, 1.0],
        ]
    }
}

/// Evaluated camera state for a single cutscene frame (`VbSetCameraMatrix` / `vwViewPointInfo` in `Event/demoview.c`).
#[derive(Debug, Clone, Copy)]
pub struct CutsceneCameraSnapshot {
    /// World-space camera eye position.
    pub eye: Vec3,
    /// World-space camera look-at interest target.
    pub interest: Vec3,
    /// Camera roll angle in radians around the forward view axis.
    pub roll_radians: f32,
    /// Raw `.dds` projection plane distance (`plane`).
    pub plane: f32,
    /// Vertical field-of-view in radians (`2 * atan(224.0 / scr_z)`).
    pub fov_y_radians: f32,
    /// Whether this frame is a hard camera cut (`Key 0x10`).
    pub is_hard_cut: bool,
}

impl CutsceneCameraSnapshot {
    /// Builds the right-handed view matrix transforming PS2 world space (Y-down) into view space,
    /// incorporating `roll_radians` around the view axis (`VbSetCameraMatrix`).
    pub fn view_matrix(&self) -> Mat4 {
        let forward = (self.interest - self.eye).normalize_or_zero();
        let fwd = if forward.length_squared() > 1e-6 {
            forward
        } else {
            Vec3::Z
        };
        // In PS2 world space Y is negative-up (`(0, -1, 0)` is world up).
        let base_up = Vec3::new(0.0, -1.0, 0.0);
        let rot_roll = Mat4::from_axis_angle(fwd, self.roll_radians);
        let rolled_up = rot_roll.transform_vector3(base_up).normalize_or_zero();
        let up = if rolled_up.length_squared() > 1e-6 {
            rolled_up
        } else {
            base_up
        };
        Mat4::look_at_rh(self.eye, self.eye + fwd * 1000.0, up)
    }
}

/// Lifecycle phase of [`DramaDemoPlayer`] (`demo_step` in `DramaDemoMain` at `0x1b2b30`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DramaDemoStep {
    /// Cutscene is actively playing (`demo_step == 2`).
    Playing,
    /// Cutscene was skipped via `Start`/`Esc` and is fading out (`demo_step == 3`).
    SkippingFadeOut,
    /// Cutscene has completed (`demo_step >= 5`), ready to hand control back to gameplay.
    Finished,
}

/// Stateful in-engine real-time 3D cutscene player (`DramaDemoMain` / `DdsPlay` / `DramaDemoSkipLast`).
#[derive(Debug, Clone)]
pub struct DramaDemoPlayer {
    /// Static metadata descriptor (`DramaDemoPlayInfo`).
    pub info: DramaDemoPlayInfo,
    /// Underlying `.dds` binary keyframe evaluator.
    pub evaluator: DemoDdsEvaluator,
    /// Current lifecycle phase (`Playing`, `SkippingFadeOut`, `Finished`).
    pub step: DramaDemoStep,
    /// Continuous 30 FPS scene frame cursor (`Demo_Frame`).
    pub scene_frame: f32,
    /// Independent 30 FPS subtitle clock (`msg_frame`).
    pub msg_frame: f32,
    /// Whether the voice stream (`info.stream_no`) has been triggered at `scene_frame >= stream_start`.
    pub voice_started: bool,
    /// Whether the voice stream is waiting to be started by the audio engine on this tick.
    pub pending_voice_trigger: bool,
    /// Whether the voice stream should be stopped immediately (`SeCall(0x3F4)` on skip/finish).
    pub pending_voice_stop: bool,
    /// Active `.mes` message ID (`info.msg_start + window.msg_offset`) if a subtitle window is currently active.
    pub active_subtitle_msg_id: Option<u16>,
    /// Current screen fade alpha (`0.0` = clear, `1.0` = black).
    pub fade_alpha: f32,
    /// Whether Widescreen FOV reduction (`ReduceCutsceneFOV`) is enabled.
    pub reduce_widescreen_fov: bool,
    /// Evaluated camera snapshot for the current frame.
    pub camera: CutsceneCameraSnapshot,
    /// Evaluated keyframe state for the current frame.
    pub current_state: DemoKeyframeState,
    /// Per-actor runtime states (`0..header.n_chars`).
    pub actors: Vec<CutsceneActorRuntime>,
}

impl DramaDemoPlayer {
    /// Initializes a new real-time 3D cutscene session at Frame 0 (`DramaDemoInit` at `0x1b2080`).
    pub fn new(info: DramaDemoPlayInfo, script: DemoDdsFile, reduce_widescreen_fov: bool) -> Self {
        let mut actors = Vec::with_capacity(script.header.character_names.len());
        for (idx, name) in script.header.character_names.iter().enumerate() {
            let anim_info = find_drama_demo_anim_info(name);
            let (anm_path, cls_path) = derive_demo_actor_anm_and_cls_paths(info.dds_path, name);
            let start_aid = anim_info.map(|a| a.start_aid).unwrap_or(0);
            actors.push(CutsceneActorRuntime {
                slot_index: idx,
                actor_name: name.clone(),
                anim_info,
                anm_path,
                cls_path,
                visible: false,
                position: Vec3::ZERO,
                yaw_radians: std::f32::consts::PI,
                current_aid: start_aid,
                subclip_index: 0,
                local_anm_frame: 0.0,
            });
        }

        let mut evaluator = DemoDdsEvaluator::new(script);
        let (init_state, _) = evaluator.sample_at(0.0, info.offset_x, info.offset_z);
        let fov_y = init_state.vertical_fov_radians(info.fov_scale(), reduce_widescreen_fov);
        let camera = CutsceneCameraSnapshot {
            eye: Vec3::from_array(init_state.camera_pos),
            interest: Vec3::from_array(init_state.camera_interest),
            roll_radians: init_state.camera_roll,
            plane: init_state.camera_plane,
            fov_y_radians: fov_y,
            is_hard_cut: true,
        };

        let mut player = Self {
            info,
            evaluator,
            step: DramaDemoStep::Playing,
            scene_frame: 0.0,
            msg_frame: 0.0,
            voice_started: false,
            pending_voice_trigger: false,
            pending_voice_stop: false,
            active_subtitle_msg_id: None,
            fade_alpha: 0.0,
            reduce_widescreen_fov,
            camera,
            current_state: init_state,
            actors,
        };
        player.sync_actors_from_state();
        player
    }

    /// Returns `true` if gameplay input and the pause menu should be locked while this cutscene is active.
    #[inline]
    pub fn locks_gameplay_input(&self) -> bool {
        self.step != DramaDemoStep::Finished
    }

    /// Total duration of the `.dds` cutscene in 30 FPS frames.
    #[inline]
    pub fn total_frames(&self) -> u16 {
        self.evaluator.script.header.total_frames.max(1) as u16
    }

    /// Triggers an immediate cutscene skip (`Start` / `Esc` button), matching `DramaDemoSkipLast`
    /// (`0x1b1be0` PS2 / `0x4a09f0` PC): fast-forwards all `.dds` records to `0xFFFF`, stops voice
    /// streaming (`SeCall(0x3F4)`), clears subtitles, and positions actors at their final coordinates (`CharaSetPos`).
    pub fn skip(&mut self) {
        if self.step == DramaDemoStep::Finished {
            return;
        }
        self.evaluator.skip_to_last();
        let total_f = self.total_frames() as f32;
        self.scene_frame = total_f;
        self.msg_frame = total_f;
        self.active_subtitle_msg_id = None;
        if self.voice_started {
            self.pending_voice_stop = true;
        }
        self.pending_voice_trigger = false;

        let mut final_state = self.evaluator.next.clone();
        final_state.camera_pos[0] += self.info.offset_x;
        final_state.camera_pos[2] += self.info.offset_z;
        final_state.camera_interest[0] += self.info.offset_x;
        final_state.camera_interest[2] += self.info.offset_z;
        for c in 0..DEMO_MAX_CHARS {
            final_state.char_positions[c][0] += self.info.offset_x;
            final_state.char_positions[c][2] += self.info.offset_z;
        }
        self.current_state = final_state;
        self.sync_camera_and_actors();
        self.fade_alpha = 1.0;
        self.step = DramaDemoStep::Finished;
    }

    /// Advances the cutscene by `dt_sec` seconds, evaluating `.dds` keyframes, `Key 0x11` animation
    /// triggers, voice stream pre-roll (`stream_start`), and `adr_msg_time` subtitle windows.
    pub fn tick(&mut self, dt_sec: f32, subtitles_enabled: bool) {
        if self.step == DramaDemoStep::Finished {
            return;
        }

        let delta_frames = (dt_sec.max(0.0)) * CUTSCENE_NATIVE_FPS;
        self.scene_frame += delta_frames;
        self.msg_frame += delta_frames;

        let total_f = self.total_frames() as f32;

        // Check voice stream pre-roll trigger (`scene_frame >= stream_start`)
        if !self.voice_started
            && self.info.stream_no > 0
            && self.scene_frame >= (self.info.stream_start.max(0) as f32)
        {
            self.voice_started = true;
            self.pending_voice_trigger = true;
        }

        // Evaluate active subtitle window (`DramaDemoMessage` at `0x1b1f80`)
        let show_subs = subtitles_enabled || self.info.forces_subtitles();
        if show_subs {
            self.active_subtitle_msg_id = self
                .info
                .subtitle_windows
                .iter()
                .find(|w| w.is_active_at(self.msg_frame))
                .map(|w| self.info.msg_start + w.msg_offset);
        } else {
            self.active_subtitle_msg_id = None;
        }

        if self.scene_frame >= total_f {
            self.skip();
            return;
        }

        let (mut sampled, _triggers) =
            self.evaluator
                .sample_at(self.scene_frame, self.info.offset_x, self.info.offset_z);

        // Apply `demo_no == 6` (`first_battle`) Lying Figure reveal lighting override (`0x4a1670`)
        if self.info.demo_no == 6 && self.scene_frame >= 1599.0 && self.scene_frame < 2365.0 {
            let mut lr = sampled.lights[0];
            lr.visible = true;
            lr.kind = DemoLightKind::Point;
            lr.position = [
                75.28 + self.info.offset_x,
                -882.43,
                39660.15 + self.info.offset_z,
            ];
            lr.color = [1.0, 1.0, 1.0];
            lr.decay = [500.0, 2000.0];
            sampled.lights[0] = lr;
        }

        self.current_state = sampled;
        self.sync_camera_and_actors();
    }

    /// Scrubs directly to `target_frame` (for interactive inspection in the debug viewer).
    pub fn seek_to_frame(&mut self, target_frame: f32, subtitles_enabled: bool) {
        let clamped = target_frame.clamp(0.0, self.total_frames() as f32);
        self.scene_frame = clamped;
        self.msg_frame = clamped;
        self.step = DramaDemoStep::Playing;
        self.fade_alpha = 0.0;

        let show_subs = subtitles_enabled || self.info.forces_subtitles();
        self.active_subtitle_msg_id = if show_subs {
            self.info
                .subtitle_windows
                .iter()
                .find(|w| w.is_active_at(self.msg_frame))
                .map(|w| self.info.msg_start + w.msg_offset)
        } else {
            None
        };

        let (sampled, _) =
            self.evaluator
                .sample_at(self.scene_frame, self.info.offset_x, self.info.offset_z);
        self.current_state = sampled;
        self.sync_camera_and_actors();
    }

    /// Returns the primary active dynamic light from the `.dds` light slots (`0..DEMO_MAX_LIGHTS`),
    /// suitable for driving the stage spotlight / point light shader uniform.
    pub fn primary_active_light(&self) -> Option<&DemoLightState> {
        let total_lights = self
            .evaluator
            .script
            .header
            .total_lights()
            .min(DEMO_MAX_LIGHTS);
        self.current_state
            .lights
            .iter()
            .take(total_lights)
            .find(|l| l.visible)
    }

    /// Returns the final player (`hhh_jms_pos` / `hhl_jms_pos` / `lll_jms_pos` / `hhh_mar_pos`)
    /// world position and yaw (`CharaSetPos` hand-off at cutscene completion).
    pub fn player_handoff_transform(&self) -> Option<(Vec3, f32)> {
        self.actors
            .iter()
            .find(|a| {
                a.anim_info
                    .map(|info| info.kind_id == 0x0100 || info.kind_id == 0x0200)
                    .unwrap_or(false)
            })
            .map(|a| (a.position, a.yaw_radians))
    }

    fn sync_camera_and_actors(&mut self) {
        let fov_y = self
            .current_state
            .vertical_fov_radians(self.info.fov_scale(), self.reduce_widescreen_fov);
        self.camera = CutsceneCameraSnapshot {
            eye: Vec3::from_array(self.current_state.camera_pos),
            interest: Vec3::from_array(self.current_state.camera_interest),
            roll_radians: self.current_state.camera_roll,
            plane: self.current_state.camera_plane,
            fov_y_radians: fov_y,
            is_hard_cut: self.evaluator.is_cut,
        };
        self.sync_actors_from_state();
    }

    fn sync_actors_from_state(&mut self) {
        let trig_count = self.evaluator.anim_trigger_count;
        let subclip_idx = trig_count.saturating_sub(1);
        let local_frame =
            (self.scene_frame - (self.evaluator.last_anim_trigger_frame as f32)).max(0.0);

        for actor in &mut self.actors {
            let idx = actor.slot_index;
            if idx < DEMO_MAX_CHARS {
                actor.visible = self.current_state.char_visible[idx];
                actor.position = Vec3::from_array(self.current_state.char_positions[idx]);
                actor.yaw_radians = std::f32::consts::PI;
                actor.subclip_index = subclip_idx;
                actor.local_anm_frame = local_frame;
                if let Some(info) = actor.anim_info {
                    actor.current_aid = (info.start_aid + (subclip_idx as u16)).min(info.end_aid);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sh2_assets::formats::demo::parse_demo_dds;

    #[test]
    fn test_drama_demo_anim_info_table_integrity() {
        assert_eq!(DRAMA_DEMO_ANIM_INFO.len(), 84);
        let jms = find_drama_demo_anim_info("hhh_jms_pos").expect("hhh_jms_pos must exist");
        assert_eq!(jms.kind_id, 0x0100);
        assert_eq!(jms.model_id, 2);
        assert_eq!(jms.start_aid, 1501);
        assert_eq!(jms.end_aid, 1859);

        let agl = find_drama_demo_anim_info("agl_pos").expect("agl_pos must exist");
        assert_eq!(agl.kind_id, 0x0D00);
        assert_eq!(agl.start_aid, 3501);
        assert_eq!(agl.end_aid, 3531);

        let (anm, cls) =
            derive_demo_actor_anm_and_cls_paths("data/demo/haka_agl/haka_agl.dds", "agl_pos");
        assert_eq!(anm, "data/demo/haka_agl/agl.anm");
        assert_eq!(cls.as_deref(), Some("data/demo/haka_agl/agl.cls"));

        let (vtr_anm, vtr_cls) =
            derive_demo_actor_anm_and_cls_paths("data/demo/vtr/vtr.dds", "hhl_jms_pos");
        assert_eq!(vtr_anm, "data/demo/vtr/hhh_jms.anm");
        assert_eq!(vtr_cls.as_deref(), Some("data/demo/vtr/hhh_jms.cls"));
    }

    #[test]
    fn test_light_decay_ramp_and_fov_scales() {
        assert_eq!(demo_light_decay_bounds(800.0), (500.0, 2000.0));
        assert_eq!(demo_light_decay_bounds(1800.0), (1000.0, 3000.0));
        assert_eq!(demo_light_decay_bounds(20000.0), (3000.0, 7500.0));

        assert!((demo_fov_scale_for_demo_no(1) - 1.0).abs() < 1e-6);
        assert!((demo_fov_scale_for_demo_no(7) - 1.25).abs() < 1e-6);
        assert!((demo_fov_scale_for_demo_no(93) - 1.4).abs() < 1e-6);
        assert!((demo_fov_scale_for_demo_no(100) - 1.35).abs() < 1e-6);
    }

    #[test]
    fn test_drama_demo_player_with_retail_tenbou_and_haka_agl() {
        let root = "../../Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2";
        let tenbou_path = format!("{root}/data/demo/tenbou/tenbou.dds");
        if !std::path::Path::new(&tenbou_path).exists() {
            return;
        }

        let info = find_drama_demo_by_id("tenbou").expect("tenbou in catalog");
        assert_eq!(info.voice_afs_index(), Some(0));
        let dds_bytes = std::fs::read(&tenbou_path).expect("read tenbou.dds");
        let dds = parse_demo_dds(&dds_bytes).expect("parse tenbou.dds");

        let mut player = DramaDemoPlayer::new(*info, dds, false);
        assert!(player.locks_gameplay_input());
        assert_eq!(player.actors.len(), 1); // hhh_jms_pos
        assert_eq!(player.actors[0].actor_name, "hhh_jms_pos");
        assert!(!player.voice_started);

        // Advance past stream_start (716) into first subtitle window (740..810)
        player.tick(750.0 / 30.0, true);
        assert!(player.voice_started);
        assert!(player.pending_voice_trigger);
        assert_eq!(player.active_subtitle_msg_id, Some(15));
        assert!(player.camera.fov_y_radians > 0.1 && player.camera.fov_y_radians < 2.5);
        let view = player.camera.view_matrix();
        assert!(!view.is_nan());

        // Test skip (`DramaDemoSkipLast`)
        player.skip();
        assert_eq!(player.step, DramaDemoStep::Finished);
        assert!(!player.locks_gameplay_input());
        assert!(player.pending_voice_stop);
        let (final_pos, final_yaw) = player
            .player_handoff_transform()
            .expect("James handoff transform");
        assert!(final_pos.is_finite());
        assert!((final_yaw - std::f32::consts::PI).abs() < 1e-5);
    }

    #[test]
    fn test_all_drama_demo_catalog_assets_exist_and_parse() {
        let root = "../../Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2";
        if !std::path::Path::new(&format!("{root}/data/demo/tenbou/tenbou.dds")).exists() {
            return;
        }

        for entry in DRAMA_DEMO_CATALOG {
            let dds_full = format!("{root}/{}", entry.dds_path);
            let dds_bytes = std::fs::read(&dds_full)
                .unwrap_or_else(|e| panic!("Missing .dds for {}: {} ({e})", entry.id, dds_full));
            let dds = parse_demo_dds(&dds_bytes)
                .unwrap_or_else(|e| panic!("Failed to parse {}: {e}", entry.dds_path));
            assert!(dds.header.total_frames > 0, "{} has 0 frames", entry.id);

            let mes_full = format!("{root}/{}", entry.mes_path);
            let mes_bytes = std::fs::read(&mes_full)
                .unwrap_or_else(|e| panic!("Missing .mes for {}: {} ({e})", entry.id, mes_full));
            let mes = sh2_assets::formats::mes::parse_mes(&mes_bytes)
                .unwrap_or_else(|e| panic!("Failed to parse {}: {e}", entry.mes_path));
            assert!(!mes.messages.is_empty(), "{} has empty .mes", entry.id);
        }
    }
}
