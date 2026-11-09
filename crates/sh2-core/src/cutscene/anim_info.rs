//! 84-entry `DramaDemo_AnimInfo` table (`0x2ba080` in `SLUS_202.28`, `0x88ed20` in `sh2pc.exe`),
//! light distance decay ramp (`0x50cff0`), and per-demo FOV scale overrides (`SH2-63`).

use serde::{Deserialize, Serialize};

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

macro_rules! anim_info {
    ($actor:expr, $kind:expr, $model:expr, $start:expr, $end:expr, $path:expr) => {
        DramaDemoAnimInfo {
            actor_name: $actor,
            kind_id: $kind,
            model_id: $model,
            start_aid: $start,
            end_aid: $end,
            mdl_path: $path,
        }
    };
}

/// Complete 84-entry `DramaDemo_AnimInfo` table extracted from `sh2pc.exe` (`0x88ed20`) and verified against `SLUS_202.28` (`0x2ba080`).
pub const DRAMA_DEMO_ANIM_INFO: &[DramaDemoAnimInfo; 84] = &[
    anim_info!(
        "hhh_jms_pos",
        0x0103,
        0,
        1000,
        1165,
        "data/chr/jms/hhh_jms.mdl"
    ),
    anim_info!(
        "hhl_jms_pos",
        0x0102,
        0,
        1166,
        1187,
        "data/chr/jms/hhl_jms.mdl"
    ),
    anim_info!(
        "hll_jms_pos",
        0x0101,
        0,
        952,
        972,
        "data/chr/jms/hll_jms.mdl"
    ),
    anim_info!(
        "lll_jms_pos",
        0x0100,
        0,
        973,
        979,
        "data/chr/jms/lll_jms.mdl"
    ),
    anim_info!(
        "hhh_mar_pos",
        0x0106,
        0,
        3000,
        3052,
        "data/chr/mar/hhh_mar.mdl"
    ),
    anim_info!("lll_mar_pos", 0x0105, 0, 42, 46, "data/chr/mar/lll_mar.mdl"),
    anim_info!("lau_pos", 0x0104, 0, 2500, 2540, "data/chr/lau/lau.mdl"),
    anim_info!(
        "hhh_edi_pos",
        0x0108,
        0,
        4503,
        4527,
        "data/chr/edi/hhh_edi.mdl"
    ),
    anim_info!(
        "lll_edi_pos",
        0x0205,
        0,
        6801,
        6801,
        "data/chr/edi/lll_edi.mdl"
    ),
    anim_info!("agl_pos", 0x0107, 0, 3500, 3531, "data/chr/agl/agl.mdl"),
    anim_info!("mry_pos", 0x0109, 0, 4002, 4020, "data/chr/mry/mry.mdl"),
    anim_info!("mxx_pos", 0x010A, 0, 4102, 4110, "data/chr/mxx/mxx.mdl"),
    anim_info!("inu_pos", 0x010D, 0, 4901, 4902, "data/chr/item/inu.mdl"),
    anim_info!("boat_pos", 0x010B, 0, 56, 58, "data/chr/boat/boat.mdl"),
    anim_info!("scu_pos", 0x0200, 0, 6003, 6009, "data/chr/scu/scu.mdl"),
    anim_info!("pap_pos", 0x0204, 0, 6602, 6607, "data/chr/pap/pap.mdl"),
    anim_info!("red_pos", 0x0208, 0, 6300, 6349, "data/chr/red/red.mdl"),
    anim_info!("mkn_1_pos", 0x0201, 0, 6102, 6104, "data/chr/mkn/mkn.mdl"),
    anim_info!("mkn_2_pos", 0x0201, 1, 6105, 6107, "data/chr/mkn/mkn.mdl"),
    anim_info!("ike_1_pos", 0x0203, 0, 6502, 6506, "data/chr/ike/ike.mdl"),
    anim_info!("ike_2_pos", 0x0203, 1, 6507, 6511, "data/chr/ike/ike.mdl"),
    anim_info!("oni_pos", 0x0209, 0, 6352, 6353, "data/chr/oni/oni.mdl"),
    anim_info!("oni_1_pos", 0x0209, 0, 6354, 6356, "data/chr/oni/oni.mdl"),
    anim_info!("oni_2_pos", 0x0209, 1, 6357, 6359, "data/chr/oni/oni.mdl"),
    anim_info!("bos_pos", 0x0206, 0, 6701, 6720, "data/chr/bos/bos.mdl"),
    anim_info!("dmr_pos", 0x0425, 0, 10163, 10164, "data/chr/item/dmr.mdl"),
    anim_info!("dm2_pos", 0x042B, 0, 10165, 10166, "data/chr/item/dm2.mdl"),
    anim_info!(
        "i_keycou_pos",
        0x0402,
        0,
        10001,
        10001,
        "data/chr/item/i_keycou.mdl"
    ),
    anim_info!(
        "i_radio_pos",
        0x0403,
        0,
        10002,
        10018,
        "data/chr/item/i_radio.mdl"
    ),
    anim_info!(
        "i_kakuzai_pos",
        0x0404,
        0,
        10019,
        10024,
        "data/chr/item/i_kakuzai.mdl"
    ),
    anim_info!(
        "i_flight_pos",
        0x0407,
        0,
        10035,
        10040,
        "data/chr/item/i_flight.mdl"
    ),
    anim_info!(
        "i_bear_pos",
        0x0405,
        0,
        10025,
        10030,
        "data/chr/item/i_bear.mdl"
    ),
    anim_info!(
        "i_bear2_pos",
        0x0406,
        0,
        10031,
        10034,
        "data/chr/item/i_bear2.mdl"
    ),
    anim_info!(
        "i_letter_pos",
        0x0408,
        0,
        10041,
        10048,
        "data/chr/item/i_letter.mdl"
    ),
    anim_info!(
        "i_handgun_pos",
        0x040D,
        0,
        10064,
        10066,
        "data/chr/item/i_handgun.mdl"
    ),
    anim_info!(
        "i_magazine_pos",
        0x040E,
        0,
        10067,
        10069,
        "data/chr/item/i_magazine.mdl"
    ),
    anim_info!(
        "i_lring_pos",
        0x040F,
        0,
        10075,
        10076,
        "data/chr/item/i_lring.mdl"
    ),
    anim_info!(
        "i_purse_pos",
        0x0416,
        0,
        10096,
        10096,
        "data/chr/item/i_purse.mdl"
    ),
    anim_info!(
        "i_j_light_pos",
        0x0418,
        0,
        10100,
        10112,
        "data/chr/item/i_j_light.mdl"
    ),
    anim_info!(
        "i_keyspe_pos",
        0x0417,
        0,
        10097,
        10099,
        "data/chr/item/i_keyspe.mdl"
    ),
    anim_info!(
        "i_mtablet_1_pos",
        0x041B,
        0,
        10122,
        10123,
        "data/chr/item/i_mtablet.mdl"
    ),
    anim_info!(
        "i_mtablet_2_pos",
        0x041B,
        1,
        10124,
        10125,
        "data/chr/item/i_mtablet.mdl"
    ),
    anim_info!(
        "i_mdrug_pos",
        0x041A,
        0,
        10120,
        10121,
        "data/chr/item/i_mdrug.mdl"
    ),
    anim_info!(
        "i_needle_pos",
        0x041D,
        0,
        10126,
        10126,
        "data/chr/item/i_needle.mdl"
    ),
    anim_info!(
        "i_keyelevator_p",
        0x041E,
        0,
        10127,
        10127,
        "data/chr/item/i_keyelevator.mdl"
    ),
    anim_info!(
        "i_key_clock_pos",
        0x041F,
        0,
        10129,
        10131,
        "data/chr/item/i_key_clock.mdl"
    ),
    anim_info!(
        "i_photo_pos",
        0x0424,
        0,
        10150,
        10159,
        "data/chr/item/i_photo.mdl"
    ),
    anim_info!(
        "i_knife_pos",
        0x0423,
        0,
        10140,
        10149,
        "data/chr/item/i_knife.mdl"
    ),
    anim_info!(
        "i_juice_pos",
        0x042D,
        0,
        10188,
        10188,
        "data/chr/item/i_juice.mdl"
    ),
    anim_info!(
        "i_video_pos",
        0x0432,
        0,
        10220,
        10220,
        "data/chr/item/i_video.mdl"
    ),
    anim_info!(
        "i_colt_pos",
        0x042A,
        0,
        10175,
        10187,
        "data/chr/item/i_colt.mdl"
    ),
    anim_info!(
        "i_hari_pos",
        0x0430,
        0,
        10213,
        10214,
        "data/chr/item/i_hari.mdl"
    ),
    anim_info!(
        "i_letterm_pos",
        0x0433,
        0,
        10228,
        10240,
        "data/chr/item/i_letterm.mdl"
    ),
    anim_info!(
        "i_headphone_pos",
        0x0434,
        0,
        10246,
        10246,
        "data/chr/item/i_headphone.mdl"
    ),
    anim_info!(
        "b_tel_pos",
        0x0409,
        0,
        10049,
        10054,
        "data/chr/item/b_tel.mdl"
    ),
    anim_info!(
        "b_d00_pos",
        0x040A,
        0,
        10055,
        10060,
        "data/chr/item/b_d00.mdl"
    ),
    anim_info!(
        "b_d01_pos",
        0x0419,
        0,
        10113,
        10119,
        "data/chr/item/b_d01.mdl"
    ),
    anim_info!(
        "b_d02_pos",
        0x0412,
        0,
        10085,
        10094,
        "data/chr/item/b_d02.mdl"
    ),
    anim_info!(
        "b_d03_pos",
        0x0410,
        0,
        10077,
        10078,
        "data/chr/item/b_d03.mdl"
    ),
    anim_info!(
        "b_d05_pos",
        0x0420,
        0,
        10132,
        10134,
        "data/chr/item/b_d05.mdl"
    ),
    anim_info!(
        "b_rei_pos",
        0x040B,
        0,
        10070,
        10072,
        "data/chr/item/b_rei.mdl"
    ),
    anim_info!(
        "b_doo_pos",
        0x040C,
        0,
        10061,
        10063,
        "data/chr/item/b_doo.mdl"
    ),
    anim_info!(
        "b_pia_pos",
        0x0411,
        0,
        10079,
        10084,
        "data/chr/item/b_pia.mdl"
    ),
    anim_info!(
        "b_ami_pos",
        0x0414,
        0,
        10095,
        10095,
        "data/chr/item/b_ami.mdl"
    ),
    anim_info!(
        "b_rop_pos",
        0x041C,
        6,
        10128,
        10128,
        "data/chr/item/b_rop.mdl"
    ),
    anim_info!(
        "b_clo_pos",
        0x0422,
        0,
        10136,
        10139,
        "data/chr/item/b_clo.mdl"
    ),
    anim_info!(
        "b_tan_pos",
        0x0427,
        0,
        10215,
        10216,
        "data/chr/item/b_tan.mdl"
    ),
    anim_info!(
        "b_dor_pos",
        0x042C,
        0,
        10217,
        10219,
        "data/chr/item/b_dor.mdl"
    ),
    anim_info!(
        "b_d08_pos",
        0x0431,
        0,
        10189,
        10196,
        "data/chr/item/b_d08.mdl"
    ),
    anim_info!(
        "b_piz_pos",
        0x042E,
        0,
        10197,
        10204,
        "data/chr/item/b_piz.mdl"
    ),
    anim_info!(
        "b_bol_pos",
        0x042F,
        0,
        10205,
        10212,
        "data/chr/item/b_bol.mdl"
    ),
    anim_info!(
        "b_ori_pos",
        0x043A,
        0,
        10221,
        10221,
        "data/chr/item/b_ori.mdl"
    ),
    anim_info!(
        "b_d06_pos",
        0x0429,
        0,
        10167,
        10170,
        "data/chr/item/b_d06.mdl"
    ),
    anim_info!(
        "b_sti_pos",
        0x0437,
        0,
        10171,
        10174,
        "data/chr/item/b_sti.mdl"
    ),
    anim_info!(
        "b_d10_pos",
        0x0438,
        0,
        10222,
        10227,
        "data/chr/item/b_d10.mdl"
    ),
    anim_info!(
        "b_hul_pos",
        0x0435,
        0,
        10242,
        10243,
        "data/chr/item/b_hul.mdl"
    ),
    anim_info!(
        "b_cha_pos",
        0x0436,
        0,
        10244,
        10245,
        "data/chr/item/b_cha.mdl"
    ),
    anim_info!("mx2_pos", 0x0439, 0, 10241, 10241, "data/chr/item/mx2.mdl"),
    anim_info!(
        "b_do4_pos",
        0x0426,
        0,
        10160,
        10162,
        "data/chr/item/b_do4.mdl"
    ),
    anim_info!(
        "b_d1x_pos",
        0x043B,
        0,
        10253,
        10265,
        "data/chr2/item/b_d1x.mdl"
    ),
    anim_info!(
        "b_d8x_pos",
        0x043D,
        0,
        10276,
        10281,
        "data/chr2/item/b_d8x.mdl"
    ),
    anim_info!(
        "bdcard_pos",
        0x043F,
        0,
        10247,
        10252,
        "data/chr2/item/bdcard.mdl"
    ),
    anim_info!(
        "i_oil_pos",
        0x043E,
        0,
        10266,
        10271,
        "data/chr2/item/i_oil.mdl"
    ),
    anim_info!(
        "i_revo_pos",
        0x0440,
        0,
        10272,
        10275,
        "data/chr2/item/i_revo.mdl"
    ),
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
    let trimmed = actor_name.trim_matches('\0').trim();
    let stem_raw = trimmed
        .strip_suffix("_pos")
        .or_else(|| trimmed.strip_suffix("_p"))
        .unwrap_or(trimmed);
    // Multi-instance actors (`mkn_1_pos`, `ike_2_pos`, `oni_1_pos`, `i_mtablet_1_pos`) share a single `.anm` stem
    let file_stem = stem_raw
        .strip_suffix("_1")
        .or_else(|| stem_raw.strip_suffix("_2"))
        .unwrap_or(stem_raw);
    let anm_path = format!("{dir}/{file_stem}.anm");
    let has_cls = matches!(
        file_stem,
        "hhh_jms"
            | "hhl_jms"
            | "hhh_mar"
            | "agl"
            | "ragl"
            | "hhh_edi"
            | "rhhh_edi"
            | "lau"
            | "mry"
            | "mxx"
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
