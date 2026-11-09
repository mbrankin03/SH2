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
        0x0100,
        2,
        1501,
        1859,
        "data/chr/jms/hhh_jms.mdl"
    ),
    anim_info!(
        "hhl_jms_pos",
        0x0100,
        3,
        1501,
        1859,
        "data/chr/jms/hhl_jms.mdl"
    ),
    anim_info!(
        "hll_jms_pos",
        0x0100,
        4,
        1501,
        1859,
        "data/chr/jms/hll_jms.mdl"
    ),
    anim_info!(
        "lll_jms_pos",
        0x0100,
        5,
        1501,
        1859,
        "data/chr/jms/lll_jms.mdl"
    ),
    anim_info!(
        "rhhh_jms_pos",
        0x0100,
        6,
        1501,
        1859,
        "data/chr/jms/rhhh_jms.mdl"
    ),
    anim_info!(
        "rhll_jms_pos",
        0x0100,
        7,
        1501,
        1859,
        "data/chr/jms/rhll_jms.mdl"
    ),
    anim_info!(
        "rlll_jms_pos",
        0x0100,
        8,
        1501,
        1859,
        "data/chr/jms/rlll_jms.mdl"
    ),
    anim_info!("DJ_pos", 0x0700, 82, 7872, 7875, "data/chr/item/dj.mdl"),
    anim_info!("scu_pos", 0x0300, 9, 2501, 2515, "data/chr/scu/scu.mdl"),
    anim_info!("lau_pos", 0x0E00, 23, 4001, 4050, "data/chr/lau/lau.mdl"),
    anim_info!("rlau_pos", 0x0700, 77, 7864, 7865, "data/chr/item/rlau.mdl"),
    anim_info!("agl_pos", 0x0D00, 21, 3501, 3531, "data/chr/agl/agl.mdl"),
    anim_info!("ragl_pos", 0x0D00, 22, 3501, 3531, "data/chr/agl/ragl.mdl"),
    anim_info!(
        "hhh_mar_pos",
        0x0200,
        19,
        2001,
        2092,
        "data/chr/mar/hhh_mar.mdl"
    ),
    anim_info!(
        "lll_mar_pos",
        0x0200,
        20,
        2001,
        2092,
        "data/chr/mar/lll_mar.mdl"
    ),
    anim_info!(
        "rhhh_mar_pos",
        0x0700,
        80,
        7869,
        7870,
        "data/chr/item/rhhh_mar.mdl"
    ),
    anim_info!(
        "hhh_edi_pos",
        0x0F00,
        24,
        4501,
        4527,
        "data/chr/edi/hhh_edi.mdl"
    ),
    anim_info!(
        "lll_edi_pos",
        0x0F00,
        25,
        4501,
        4527,
        "data/chr/edi/lll_edi.mdl"
    ),
    anim_info!(
        "rhhh_edi_pos",
        0x0F00,
        26,
        4501,
        4527,
        "data/chr/edi/rhhh_edi.mdl"
    ),
    anim_info!("mry_pos", 0x1000, 27, 5001, 5055, "data/chr/mry/mry.mdl"),
    anim_info!("rmry_pos", 0x0700, 78, 7866, 7866, "data/chr/item/rmry.mdl"),
    anim_info!("oni_pos", 0x0300, 13, 3001, 3003, "data/chr/oni/oni.mdl"),
    anim_info!("red_pos", 0x0A00, 12, 3001, 3022, "data/chr/red/red.mdl"),
    anim_info!("pap_pos", 0x0C00, 17, 3301, 3307, "data/chr/pap/pap.mdl"),
    anim_info!("bos_pos", 0x1100, 18, 3401, 3403, "data/chr/bos/bos.mdl"),
    anim_info!("tyu_pos", 0x0600, 15, 3101, 3106, "data/chr/tyu/tyu.mdl"),
    anim_info!("mkn_pos", 0x0500, 11, 2901, 2909, "data/chr/mkn/mkn.mdl"),
    anim_info!("ike_pos", 0x0B00, 16, 3201, 3202, "data/chr/ike/ike.mdl"),
    anim_info!(
        "p_light_pos",
        0x0700,
        28,
        7501,
        7516,
        "data/chr/item/p_light.mdl"
    ),
    anim_info!(
        "b_d00_pos",
        0x0700,
        29,
        7517,
        7541,
        "data/chr/item/b_d00.mdl"
    ),
    anim_info!(
        "b_s07_pos",
        0x0700,
        30,
        7542,
        7544,
        "data/chr/item/b_s07.mdl"
    ),
    anim_info!(
        "b_d10_pos",
        0x0700,
        31,
        7545,
        7550,
        "data/chr/item/b_d10.mdl"
    ),
    anim_info!("nad_pos", 0x0700, 32, 7551, 7554, "data/chr/item/nad.mdl"),
    anim_info!(
        "b_c01_pos",
        0x0700,
        33,
        7555,
        7561,
        "data/chr/item/b_c01.mdl"
    ),
    anim_info!("c12_pos", 0x0700, 34, 7562, 7565, "data/chr/item/c12.mdl"),
    anim_info!("d22_pos", 0x0700, 35, 7566, 7582, "data/chr/item/d22.mdl"),
    anim_info!("e02_pos", 0x0700, 36, 7583, 7589, "data/chr/item/e02.mdl"),
    anim_info!("f09_pos", 0x0700, 37, 7590, 7595, "data/chr/item/f09.mdl"),
    anim_info!("g05_pos", 0x0700, 38, 7596, 7606, "data/chr/item/g05.mdl"),
    anim_info!("h03_pos", 0x0700, 39, 7607, 7609, "data/chr/item/h03.mdl"),
    anim_info!("i02_pos", 0x0700, 40, 7610, 7617, "data/chr/item/i02.mdl"),
    anim_info!("j00_pos", 0x0700, 41, 7618, 7620, "data/chr/item/j00.mdl"),
    anim_info!("k00_pos", 0x0700, 42, 7621, 7627, "data/chr/item/k00.mdl"),
    anim_info!("l08_pos", 0x0700, 43, 7628, 7632, "data/chr/item/l08.mdl"),
    anim_info!("m02_pos", 0x0700, 44, 7633, 7651, "data/chr/item/m02.mdl"),
    anim_info!("n02_pos", 0x0700, 45, 7652, 7660, "data/chr/item/n02.mdl"),
    anim_info!("o00_pos", 0x0700, 46, 7661, 7679, "data/chr/item/o00.mdl"),
    anim_info!("p06_pos", 0x0700, 47, 7680, 7681, "data/chr/item/p06.mdl"),
    anim_info!("q09_pos", 0x0700, 48, 7682, 7687, "data/chr/item/q09.mdl"),
    anim_info!("r01_pos", 0x0700, 49, 7688, 7697, "data/chr/item/r01.mdl"),
    anim_info!("s00_pos", 0x0700, 50, 7698, 7707, "data/chr/item/s00.mdl"),
    anim_info!("t05_pos", 0x0700, 51, 7708, 7716, "data/chr/item/t05.mdl"),
    anim_info!("u04_pos", 0x0700, 52, 7717, 7724, "data/chr/item/u04.mdl"),
    anim_info!("v09_pos", 0x0700, 53, 7725, 7734, "data/chr/item/v09.mdl"),
    anim_info!("w03_pos", 0x0700, 54, 7735, 7742, "data/chr/item/w03.mdl"),
    anim_info!("x02_pos", 0x0700, 55, 7743, 7755, "data/chr/item/x02.mdl"),
    anim_info!("y08_pos", 0x0700, 56, 7756, 7760, "data/chr/item/y08.mdl"),
    anim_info!("z00_pos", 0x0700, 57, 7761, 7764, "data/chr/item/z00.mdl"),
    anim_info!("z06_pos", 0x0700, 58, 7765, 7774, "data/chr/item/z06.mdl"),
    anim_info!("ed2_pos", 0x0700, 59, 7775, 7777, "data/chr/item/ed2.mdl"),
    anim_info!("org_pos", 0x0700, 60, 7778, 7786, "data/chr/item/org.mdl"),
    anim_info!("piz_pos", 0x0700, 61, 7787, 7792, "data/chr/item/piz.mdl"),
    anim_info!("x13_pos", 0x0700, 62, 7793, 7802, "data/chr/item/x13.mdl"),
    anim_info!("c06_pos", 0x0700, 63, 7803, 7804, "data/chr/item/c06.mdl"),
    anim_info!("m03_pos", 0x0700, 64, 7805, 7811, "data/chr/item/m03.mdl"),
    anim_info!("fg_pos", 0x0700, 65, 7812, 7816, "data/chr/item/fg.mdl"),
    anim_info!("l06_pos", 0x0700, 66, 7817, 7824, "data/chr/item/l06.mdl"),
    anim_info!(
        "b_a04_pos",
        0x0700,
        67,
        7825,
        7826,
        "data/chr/item/b_a04.mdl"
    ),
    anim_info!("tl_pos", 0x0700, 68, 7827, 7830, "data/chr/item/tl.mdl"),
    anim_info!(
        "b_bol_pos",
        0x0700,
        69,
        7831,
        7835,
        "data/chr/item/b_bol.mdl"
    ),
    anim_info!("kum_pos", 0x0700, 70, 7836, 7843, "data/chr/item/kum.mdl"),
    anim_info!("ls_pos", 0x0700, 71, 7844, 7847, "data/chr/item/ls.mdl"),
    anim_info!("v08_pos", 0x0700, 72, 7848, 7850, "data/chr/item/v08.mdl"),
    anim_info!("n01_pos", 0x0700, 73, 7851, 7853, "data/chr/item/n01.mdl"),
    anim_info!("c54_pos", 0x0700, 74, 7854, 7857, "data/chr/item/c54.mdl"),
    anim_info!("r99_pos", 0x0700, 75, 7858, 7860, "data/chr/item/r99.mdl"),
    anim_info!("x22_pos", 0x0700, 76, 7861, 7863, "data/chr/item/x22.mdl"),
    anim_info!(
        "b_dt0_pos",
        0x0700,
        79,
        7867,
        7868,
        "data/chr/item/b_dt0.mdl"
    ),
    anim_info!("c16_pos", 0x0700, 81, 7871, 7871, "data/chr/item/c16.mdl"),
    anim_info!(
        "rlll_mar_pos",
        0x0700,
        83,
        7876,
        7878,
        "data/chr/item/rlll_mar.mdl"
    ),
    anim_info!("mxx_pos", 0x1000, 84, 5001, 5055, "data/chr/mxx/mxx.mdl"),
    anim_info!("c91_pos", 0x0700, 85, 7879, 7886, "data/chr/item/c91.mdl"),
    anim_info!(
        "wp_wall_pos",
        0x0700,
        86,
        7887,
        7892,
        "data/chr2/item/wp_wall.mdl"
    ),
    anim_info!(
        "ri02_pos",
        0x0700,
        87,
        7893,
        7898,
        "data/chr2/item/ri02.mdl"
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
