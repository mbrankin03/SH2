//! Authentic PS2 (`SLUS_202.28`) and PC (`sh2pc.exe`) named sub-clip definitions for the
//! 3D Model, Animation & Texture Viewer (`SH2-34` / `SH2-36` / `SH2-57`).
//!
//! Maps each `.anm` track in [`crate::model_viewer::BESTIARY_CATALOG`] to its exact PS2 `aid`
//! sub-clip frame ranges and native playback speeds (`fps`).

use sh2_core::ai::creatures::anim::{
    AbstractDaddyAnimClips, CreeperAnimClips, FleshLipsAnimClips, LyingFigureAnimClips,
    MandarinAnimClips, MannequinAnimClips, MariaAnimClips, MaryBossAnimClips, NurseAnimClips,
    PyramidHeadAnimClips,
};
use sh2_core::ai::npcs::{
    AngelaAnimClips, EddieBossAnimClips, EddieNpcAnimClips, LauraAnimClips, MaryNpcAnimClips,
};
use sh2_core::animation::AnimationClip;

/// A named PS2 animation sub-clip within a multi-clip `.anm` file.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewerSubClip {
    /// Human-readable label including the PS2 `aid` and motion description.
    pub label: &'static str,
    /// Frame range `[start_frame..=end_frame]`, native PS2 FPS, and loop flag.
    pub clip: AnimationClip,
}

impl ViewerSubClip {
    pub const fn new(label: &'static str, clip: AnimationClip) -> Self {
        Self { label, clip }
    }
}

// ============================================================================
// Story NPC Play & Cutscene Sub-Clips (SH2-34)
// ============================================================================

const AGL_PLAY_SUBCLIPS: &[ViewerSubClip] = &[
    ViewerSubClip::new("3904: Idle Breathe", AngelaAnimClips::IDLE_BREATHE),
    ViewerSubClip::new("3901: Walk Cycle A", AngelaAnimClips::WALK_A),
    ViewerSubClip::new("3903: Walk Cycle B", AngelaAnimClips::WALK_B),
    ViewerSubClip::new("3905: Stair Walk", AngelaAnimClips::STAIR_WALK),
];

const AGL_HAKA_SUBCLIPS: &[ViewerSubClip] = &[
    ViewerSubClip::new(
        "3501/3509: Graveyard Part 1",
        AnimationClip::new(0, 839, 30.0, false),
    ),
    ViewerSubClip::new(
        "3510: Graveyard Part 2",
        AnimationClip::new(840, 1502, 30.0, false),
    ),
    ViewerSubClip::new(
        "3511: Graveyard Part 3",
        AnimationClip::new(1503, 1998, 30.0, false),
    ),
    ViewerSubClip::new(
        "3512: Graveyard Part 4",
        AnimationClip::new(1999, 2799, 30.0, false),
    ),
    ViewerSubClip::new(
        "3513: Graveyard Part 5",
        AnimationClip::new(2800, 3439, 30.0, false),
    ),
    ViewerSubClip::new(
        "3514: Graveyard Part 6",
        AnimationClip::new(3440, 3896, 30.0, false),
    ),
    ViewerSubClip::new(
        "3515: Graveyard Part 7",
        AnimationClip::new(3897, 4168, 30.0, false),
    ),
];

const AGL_KNIFE_SUBCLIPS: &[ViewerSubClip] = &[
    ViewerSubClip::new(
        "3516: Mirror Knife Part 1",
        AnimationClip::new(0, 559, 30.0, false),
    ),
    ViewerSubClip::new(
        "3517: Mirror Knife Part 2",
        AnimationClip::new(560, 1379, 30.0, false),
    ),
    ViewerSubClip::new(
        "3518: Mirror Knife Part 3",
        AnimationClip::new(1380, 2125, 30.0, false),
    ),
    ViewerSubClip::new(
        "3519: Mirror Knife Part 4",
        AnimationClip::new(2126, 2829, 30.0, false),
    ),
    ViewerSubClip::new(
        "3520: Mirror Knife Part 5",
        AnimationClip::new(2830, 3588, 30.0, false),
    ),
    ViewerSubClip::new(
        "3521: Mirror Knife Part 6",
        AnimationClip::new(3589, 4429, 30.0, false),
    ),
    ViewerSubClip::new(
        "3522: Mirror Knife Part 7",
        AnimationClip::new(4430, 4935, 30.0, false),
    ),
    ViewerSubClip::new(
        "3523: Mirror Knife Part 8",
        AnimationClip::new(4936, 5486, 30.0, false),
    ),
    ViewerSubClip::new(
        "3524: Mirror Knife Part 9",
        AnimationClip::new(5487, 5941, 30.0, false),
    ),
    ViewerSubClip::new(
        "3525: Mirror Knife Part 10",
        AnimationClip::new(5942, 6370, 30.0, false),
    ),
];

const AGL_PAPA_SUBCLIPS: &[ViewerSubClip] = &[
    ViewerSubClip::new(
        "3502/3503: Abstract Daddy Part 1",
        AnimationClip::new(0, 506, 30.0, false),
    ),
    ViewerSubClip::new(
        "3504: Abstract Daddy Part 2",
        AnimationClip::new(507, 1305, 30.0, false),
    ),
    ViewerSubClip::new(
        "3505: Abstract Daddy Part 3",
        AnimationClip::new(1306, 2156, 30.0, false),
    ),
    ViewerSubClip::new(
        "3506: Abstract Daddy Part 4",
        AnimationClip::new(2157, 2799, 30.0, false),
    ),
    ViewerSubClip::new(
        "3507: Abstract Daddy Part 5",
        AnimationClip::new(2800, 3512, 30.0, false),
    ),
    ViewerSubClip::new(
        "3508: Abstract Daddy Part 6",
        AnimationClip::new(3513, 4667, 30.0, false),
    ),
];

const AGL_FIRE_SUBCLIPS: &[ViewerSubClip] = &[
    ViewerSubClip::new(
        "3526: Burning Staircase Part 1",
        AnimationClip::new(0, 1109, 30.0, false),
    ),
    ViewerSubClip::new(
        "3527: Burning Staircase Part 2",
        AnimationClip::new(1110, 1780, 30.0, false),
    ),
    ViewerSubClip::new(
        "3528: Burning Staircase Part 3",
        AnimationClip::new(1781, 2653, 30.0, false),
    ),
    ViewerSubClip::new(
        "3529: Burning Staircase Part 4",
        AnimationClip::new(2654, 3965, 30.0, false),
    ),
    ViewerSubClip::new(
        "3530: Burning Staircase Part 5",
        AnimationClip::new(3966, 4825, 30.0, false),
    ),
    ViewerSubClip::new(
        "3531: Burning Staircase Part 6",
        AnimationClip::new(4826, 6103, 30.0, false),
    ),
];

const EDI_PLAY_SUBCLIPS: &[ViewerSubClip] = &[
    ViewerSubClip::new("4801: Idle Breathe A", EddieNpcAnimClips::IDLE_BREATHE_A),
    ViewerSubClip::new("4802: Idle Breathe B", EddieNpcAnimClips::IDLE_BREATHE_B),
    ViewerSubClip::new("Talk / Gesture", EddieNpcAnimClips::TALK_GESTURE),
    ViewerSubClip::new("Cower / Sit", EddieNpcAnimClips::COWER_SIT),
    ViewerSubClip::new("Stand Idle", EddieNpcAnimClips::STAND_IDLE),
    ViewerSubClip::new("4803: Static Pose", EddieNpcAnimClips::STATIC_POSE),
];

const EDI_GERO_SUBCLIPS: &[ViewerSubClip] = &[
    ViewerSubClip::new(
        "4502/4503: Toilet Vomiting Part 1",
        AnimationClip::new(0, 1055, 30.0, false),
    ),
    ViewerSubClip::new(
        "4504: Toilet Vomiting Part 2",
        AnimationClip::new(1056, 2688, 30.0, false),
    ),
    ViewerSubClip::new(
        "4505: Toilet Vomiting Part 3",
        AnimationClip::new(2689, 3479, 30.0, false),
    ),
    ViewerSubClip::new(
        "4506: Toilet Vomiting Part 4",
        AnimationClip::new(3480, 3650, 30.0, false),
    ),
    ViewerSubClip::new(
        "4507: Toilet Vomiting Part 5",
        AnimationClip::new(3651, 4143, 30.0, false),
    ),
];

const EDI_KOROSU_SUBCLIPS: &[ViewerSubClip] = &[
    ViewerSubClip::new(
        "4519: Prison Corpse Part 1",
        AnimationClip::new(0, 490, 30.0, false),
    ),
    ViewerSubClip::new(
        "4520: Prison Corpse Part 2",
        AnimationClip::new(491, 1301, 30.0, false),
    ),
    ViewerSubClip::new(
        "4521: Prison Corpse Part 3",
        AnimationClip::new(1302, 1688, 30.0, false),
    ),
];

const EDI_BP1_SUBCLIPS: &[ViewerSubClip] = &[
    ViewerSubClip::new(
        "4511: Bowling Alley 1 Part 1",
        AnimationClip::new(0, 763, 30.0, false),
    ),
    ViewerSubClip::new(
        "4512: Bowling Alley 1 Part 2",
        AnimationClip::new(764, 1189, 30.0, false),
    ),
];

const EDI_BP2_SUBCLIPS: &[ViewerSubClip] = &[
    ViewerSubClip::new(
        "4513: Bowling Alley 2 Part 1",
        AnimationClip::new(0, 731, 30.0, false),
    ),
    ViewerSubClip::new(
        "4514: Bowling Alley 2 Part 2",
        AnimationClip::new(732, 1112, 30.0, false),
    ),
    ViewerSubClip::new(
        "4515: Bowling Alley 2 Part 3",
        AnimationClip::new(1113, 1284, 30.0, false),
    ),
    ViewerSubClip::new(
        "4516: Bowling Alley 2 Part 4",
        AnimationClip::new(1285, 1661, 30.0, false),
    ),
    ViewerSubClip::new(
        "4517: Bowling Alley 2 Part 5",
        AnimationClip::new(1662, 2106, 30.0, false),
    ),
    ViewerSubClip::new(
        "4518: Bowling Alley 2 Part 6",
        AnimationClip::new(2107, 2317, 30.0, false),
    ),
];

const EDI_KILLER_SUBCLIPS: &[ViewerSubClip] = &[
    ViewerSubClip::new(
        "4508: Meat Locker Intro Part 1",
        AnimationClip::new(0, 967, 30.0, false),
    ),
    ViewerSubClip::new(
        "4509: Meat Locker Intro Part 2",
        AnimationClip::new(968, 1939, 30.0, false),
    ),
    ViewerSubClip::new(
        "4510: Meat Locker Intro Part 3",
        AnimationClip::new(1940, 2871, 30.0, false),
    ),
];

const EDI_KILLER_E_SUBCLIPS: &[ViewerSubClip] = &[
    ViewerSubClip::new(
        "4522: Eddie Death Part 1",
        AnimationClip::new(0, 671, 30.0, false),
    ),
    ViewerSubClip::new(
        "4523: Eddie Death Part 2",
        AnimationClip::new(672, 1450, 30.0, false),
    ),
    ViewerSubClip::new(
        "4524: Eddie Death Part 3",
        AnimationClip::new(1451, 1920, 30.0, false),
    ),
    ViewerSubClip::new(
        "4525: Eddie Death Part 4",
        AnimationClip::new(1921, 2571, 30.0, false),
    ),
    ViewerSubClip::new(
        "4526: Eddie Death Part 5",
        AnimationClip::new(2572, 2782, 30.0, false),
    ),
    ViewerSubClip::new(
        "4527: Eddie Death Part 6",
        AnimationClip::new(2783, 4053, 30.0, false),
    ),
];

const EDI_KILLER_I_SUBCLIPS: &[ViewerSubClip] = &[
    ViewerSubClip::new(
        "4526: Meat Locker Short Part 1",
        AnimationClip::new(0, 210, 30.0, false),
    ),
    ViewerSubClip::new(
        "4527: Meat Locker Short Part 2",
        AnimationClip::new(211, 1481, 30.0, false),
    ),
];

const EDB_BOSS_SUBCLIPS: &[ViewerSubClip] = &[
    ViewerSubClip::new("5901: Stand Idle", EddieBossAnimClips::STAND),
    ViewerSubClip::new("5902: Walk Forward", EddieBossAnimClips::WALK),
    ViewerSubClip::new("5903: Run / Rush", EddieBossAnimClips::RUN),
    ViewerSubClip::new("5904: Aim Step Left", EddieBossAnimClips::AIM_STEP_LEFT),
    ViewerSubClip::new("5905: Aim Step Right", EddieBossAnimClips::AIM_STEP_RIGHT),
    ViewerSubClip::new("5906: Shoot Revolver", EddieBossAnimClips::SHOOT_REVOLVER),
    ViewerSubClip::new(
        "5907: Aim Hold / Recoil",
        EddieBossAnimClips::AIM_HOLD_RECOIL,
    ),
    ViewerSubClip::new("5908: Melee Punch", EddieBossAnimClips::MELEE_PUNCH),
    ViewerSubClip::new("5909: Reload / Turn", EddieBossAnimClips::RELOAD_TURN),
    ViewerSubClip::new(
        "5910: Light Damage Front",
        EddieBossAnimClips::DAMAGE_CLIPS[0],
    ),
    ViewerSubClip::new(
        "5911: Light Damage Back",
        EddieBossAnimClips::DAMAGE_CLIPS[1],
    ),
    ViewerSubClip::new(
        "5912: Light Damage Left",
        EddieBossAnimClips::DAMAGE_CLIPS[2],
    ),
    ViewerSubClip::new(
        "5913: Light Damage Right",
        EddieBossAnimClips::DAMAGE_CLIPS[3],
    ),
    ViewerSubClip::new(
        "5914: Medium Damage Front",
        EddieBossAnimClips::DAMAGE_CLIPS[4],
    ),
    ViewerSubClip::new(
        "5915: Medium Damage Back",
        EddieBossAnimClips::DAMAGE_CLIPS[5],
    ),
    ViewerSubClip::new(
        "5916: Medium Damage Left",
        EddieBossAnimClips::DAMAGE_CLIPS[6],
    ),
    ViewerSubClip::new(
        "5917: Medium Damage Right",
        EddieBossAnimClips::DAMAGE_CLIPS[7],
    ),
    ViewerSubClip::new(
        "5918: Heavy Damage Front",
        EddieBossAnimClips::DAMAGE_CLIPS[8],
    ),
    ViewerSubClip::new(
        "5919: Heavy Damage Back",
        EddieBossAnimClips::DAMAGE_CLIPS[9],
    ),
    ViewerSubClip::new(
        "5920: Heavy Damage Left",
        EddieBossAnimClips::DAMAGE_CLIPS[10],
    ),
    ViewerSubClip::new(
        "5921: Heavy Damage Right",
        EddieBossAnimClips::DAMAGE_CLIPS[11],
    ),
];

const EDB_DEATH_SUBCLIPS: &[ViewerSubClip] = &[ViewerSubClip::new(
    "6801: Boss Defeat Cutscene",
    EddieBossAnimClips::DEATH_CUTSCENE,
)];

const LAU_PLAY_SUBCLIPS: &[ViewerSubClip] = &[ViewerSubClip::new(
    "2901: Walk / Idle Loop",
    LauraAnimClips::WALK_IDLE,
)];

const LAU_KAGIKERI_SUBCLIPS: &[ViewerSubClip] = &[ViewerSubClip::new(
    "2502: Key Kick Cutscene",
    AnimationClip::new(0, 632, 30.0, false),
)];

const LAU_HEI_SUBCLIPS: &[ViewerSubClip] = &[
    ViewerSubClip::new(
        "2503: Brick Wall Part 1",
        AnimationClip::new(0, 364, 30.0, false),
    ),
    ViewerSubClip::new(
        "2504: Brick Wall Part 2",
        AnimationClip::new(365, 467, 30.0, false),
    ),
];

const LAU_BP1_SUBCLIPS: &[ViewerSubClip] = &[
    ViewerSubClip::new(
        "2505: Bowling Alley 1 Part 1",
        AnimationClip::new(0, 763, 30.0, false),
    ),
    ViewerSubClip::new(
        "2506: Bowling Alley 1 Part 2",
        AnimationClip::new(764, 1189, 30.0, false),
    ),
];

const LAU_BP2_SUBCLIPS: &[ViewerSubClip] = &[
    ViewerSubClip::new(
        "2507: Bowling Alley 2 Part 1",
        AnimationClip::new(0, 660, 30.0, false),
    ),
    ViewerSubClip::new(
        "2508: Bowling Alley 2 Part 2",
        AnimationClip::new(661, 1041, 30.0, false),
    ),
    ViewerSubClip::new(
        "2509: Bowling Alley 2 Part 3",
        AnimationClip::new(1042, 1213, 30.0, false),
    ),
    ViewerSubClip::new(
        "2510: Bowling Alley 2 Part 4",
        AnimationClip::new(1214, 1590, 30.0, false),
    ),
    ViewerSubClip::new(
        "2511: Bowling Alley 2 Part 5",
        AnimationClip::new(1591, 2035, 30.0, false),
    ),
];

const LAU_HP_SUBCLIPS: &[ViewerSubClip] = &[
    ViewerSubClip::new(
        "2512: Hospital Bears Part 1",
        AnimationClip::new(0, 99, 30.0, false),
    ),
    ViewerSubClip::new(
        "2513: Hospital Bears Part 2",
        AnimationClip::new(100, 209, 30.0, false),
    ),
    ViewerSubClip::new(
        "2514: Hospital Bears Part 3",
        AnimationClip::new(210, 358, 30.0, false),
    ),
];

const LAU_DAMASU_SUBCLIPS: &[ViewerSubClip] = &[
    ViewerSubClip::new(
        "2515: Exam Room Trick Part 1",
        AnimationClip::new(0, 913, 30.0, false),
    ),
    ViewerSubClip::new(
        "2516: Exam Room Trick Part 2",
        AnimationClip::new(914, 1444, 30.0, false),
    ),
    ViewerSubClip::new(
        "2517: Exam Room Trick Part 3",
        AnimationClip::new(1445, 2324, 30.0, false),
    ),
    ViewerSubClip::new(
        "2518: Exam Room Trick Part 4",
        AnimationClip::new(2325, 3084, 30.0, false),
    ),
];

const LAU_DAMASU_M_SUBCLIPS: &[ViewerSubClip] = &[
    ViewerSubClip::new(
        "2519: Exam Room Lock-In Part 1",
        AnimationClip::new(0, 552, 30.0, false),
    ),
    ViewerSubClip::new(
        "2520: Exam Room Lock-In Part 2",
        AnimationClip::new(553, 962, 30.0, false),
    ),
];

const LAU_PIANO_SUBCLIPS: &[ViewerSubClip] = &[
    ViewerSubClip::new(
        "2521: Hotel Piano Part 1",
        AnimationClip::new(0, 225, 30.0, false),
    ),
    ViewerSubClip::new(
        "2522: Hotel Piano Part 2",
        AnimationClip::new(226, 468, 30.0, false),
    ),
    ViewerSubClip::new(
        "2523: Hotel Piano Part 3",
        AnimationClip::new(469, 1188, 30.0, false),
    ),
    ViewerSubClip::new(
        "2524: Hotel Piano Part 4",
        AnimationClip::new(1189, 1904, 30.0, false),
    ),
    ViewerSubClip::new(
        "2525: Hotel Piano Part 5",
        AnimationClip::new(1905, 2685, 30.0, false),
    ),
    ViewerSubClip::new(
        "2526: Hotel Piano Part 6",
        AnimationClip::new(2686, 3446, 30.0, false),
    ),
];

const LAU_ROOM312_SUBCLIPS: &[ViewerSubClip] = &[
    ViewerSubClip::new(
        "2527: Room 312 Part 1",
        AnimationClip::new(0, 1639, 30.0, false),
    ),
    ViewerSubClip::new(
        "2528: Room 312 Part 2",
        AnimationClip::new(1640, 1728, 30.0, false),
    ),
    ViewerSubClip::new(
        "2529: Room 312 Part 3",
        AnimationClip::new(1729, 2248, 30.0, false),
    ),
    ViewerSubClip::new(
        "2530: Room 312 Part 4",
        AnimationClip::new(2249, 2559, 30.0, false),
    ),
    ViewerSubClip::new(
        "2531: Room 312 Part 5",
        AnimationClip::new(2560, 3219, 30.0, false),
    ),
    ViewerSubClip::new(
        "2532: Room 312 Part 6",
        AnimationClip::new(3220, 4004, 30.0, false),
    ),
    ViewerSubClip::new(
        "2533: Room 312 Part 7",
        AnimationClip::new(4005, 4420, 30.0, false),
    ),
    ViewerSubClip::new(
        "2534: Room 312 Part 8",
        AnimationClip::new(4421, 5246, 30.0, false),
    ),
    ViewerSubClip::new(
        "2535: Room 312 Part 9",
        AnimationClip::new(5247, 5946, 30.0, false),
    ),
    ViewerSubClip::new(
        "2536: Room 312 Part 10",
        AnimationClip::new(5947, 6699, 30.0, false),
    ),
];

const LAU_EXIT_SUBCLIPS: &[ViewerSubClip] = &[ViewerSubClip::new(
    "2537: Leave Ending Walk",
    AnimationClip::new(0, 233, 30.0, false),
)];

const LAU_YARINAOSHI_SUBCLIPS: &[ViewerSubClip] = &[ViewerSubClip::new(
    "2538: Leave Ending Bedside",
    AnimationClip::new(0, 1331, 30.0, false),
)];

const MRY_PLAY_SUBCLIPS: &[ViewerSubClip] = &[
    ViewerSubClip::new("4091: Walk / Apparition A", MaryNpcAnimClips::WALK_A),
    ViewerSubClip::new("4092: Walk / Apparition B", MaryNpcAnimClips::WALK_B),
];

const MRY_FUKATSU_SUBCLIPS: &[ViewerSubClip] = &[
    ViewerSubClip::new(
        "4002: Rebirth Ending Part 1",
        AnimationClip::new(0, 400, 30.0, false),
    ),
    ViewerSubClip::new(
        "4003: Rebirth Ending Part 2",
        AnimationClip::new(401, 1106, 30.0, false),
    ),
    ViewerSubClip::new(
        "4004: Rebirth Ending Part 3",
        AnimationClip::new(1107, 1862, 30.0, false),
    ),
    ViewerSubClip::new(
        "4005: Rebirth Ending Part 4",
        AnimationClip::new(1863, 2050, 30.0, false),
    ),
];

const MRY_JISATSU_SUBCLIPS: &[ViewerSubClip] = &[
    ViewerSubClip::new(
        "4006: In Water Ending Part 1",
        AnimationClip::new(0, 559, 30.0, false),
    ),
    ViewerSubClip::new(
        "4007: In Water Ending Part 2",
        AnimationClip::new(560, 1170, 30.0, false),
    ),
    ViewerSubClip::new(
        "4008: In Water Ending Part 3",
        AnimationClip::new(1171, 1558, 30.0, false),
    ),
    ViewerSubClip::new(
        "4009: In Water Ending Part 4",
        AnimationClip::new(1559, 2041, 30.0, false),
    ),
    ViewerSubClip::new(
        "4010: In Water Ending Part 5",
        AnimationClip::new(2042, 2229, 30.0, false),
    ),
];

const MRY_MAR_ISHO_SUBCLIPS: &[ViewerSubClip] = &[
    ViewerSubClip::new(
        "4011: Maria Ending Part 1",
        AnimationClip::new(0, 477, 30.0, false),
    ),
    ViewerSubClip::new(
        "4012: Maria Ending Part 2",
        AnimationClip::new(478, 1321, 30.0, false),
    ),
    ViewerSubClip::new(
        "4013: Maria Ending Part 3",
        AnimationClip::new(1322, 1997, 30.0, false),
    ),
    ViewerSubClip::new(
        "4014: Maria Ending Part 4",
        AnimationClip::new(1998, 2831, 30.0, false),
    ),
    ViewerSubClip::new(
        "4015: Maria Ending Part 5",
        AnimationClip::new(2832, 3019, 30.0, false),
    ),
];

const MRY_YARINAOSHI_SUBCLIPS: &[ViewerSubClip] = &[
    ViewerSubClip::new(
        "4016: Leave Ending Part 1",
        AnimationClip::new(0, 467, 30.0, false),
    ),
    ViewerSubClip::new(
        "4017: Leave Ending Part 2",
        AnimationClip::new(468, 1120, 30.0, false),
    ),
    ViewerSubClip::new(
        "4018: Leave Ending Part 3",
        AnimationClip::new(1121, 1633, 30.0, false),
    ),
    ViewerSubClip::new(
        "4019: Leave Ending Part 4",
        AnimationClip::new(1634, 1975, 30.0, false),
    ),
    ViewerSubClip::new(
        "4020: Leave Ending Part 5",
        AnimationClip::new(1976, 2163, 30.0, false),
    ),
];

// ============================================================================
// Creature & Companion Sub-Clips (SH2-57)
// ============================================================================

const SCU_SUBCLIPS: &[ViewerSubClip] = &[
    ViewerSubClip::new("Stand Idle", LyingFigureAnimClips::STAND),
    ViewerSubClip::new("Walk", LyingFigureAnimClips::WALK),
    ViewerSubClip::new("Prone Crawl", LyingFigureAnimClips::PRONE_CRAWL),
    ViewerSubClip::new("Acid Spray Attack", LyingFigureAnimClips::ACID_SPRAY),
    ViewerSubClip::new("Flinch", LyingFigureAnimClips::FLINCH),
    ViewerSubClip::new("Knockdown / Writhing", LyingFigureAnimClips::KNOCKDOWN),
    ViewerSubClip::new("Recovery", LyingFigureAnimClips::RECOVERY),
];

const MKN_SUBCLIPS: &[ViewerSubClip] = &[
    ViewerSubClip::new("Dormant Stay", MannequinAnimClips::STAY),
    ViewerSubClip::new("Walk", MannequinAnimClips::WALK),
    ViewerSubClip::new("Arm Swing Attack", MannequinAnimClips::ARM_SWING),
    ViewerSubClip::new("Kick Attack", MannequinAnimClips::KICK),
    ViewerSubClip::new("Flinch", MannequinAnimClips::FLINCH),
    ViewerSubClip::new("Knockdown", MannequinAnimClips::KNOCKDOWN),
    ViewerSubClip::new("Writhing Loop", MannequinAnimClips::WRITHING),
];

const NSE_SUBCLIPS: &[ViewerSubClip] = &[
    ViewerSubClip::new("Stand Idle", NurseAnimClips::STAND),
    ViewerSubClip::new("Twitch Walk", NurseAnimClips::TWITCH_WALK),
    ViewerSubClip::new("Pipe Swing Attack", NurseAnimClips::PIPE_SWING),
    ViewerSubClip::new("Flinch", NurseAnimClips::FLINCH),
    ViewerSubClip::new("Knockdown", NurseAnimClips::KNOCKDOWN),
];

const MAR_SUBCLIPS: &[ViewerSubClip] = &[
    ViewerSubClip::new("Stand Idle", MariaAnimClips::STAND),
    ViewerSubClip::new("Walk", MariaAnimClips::WALK),
    ViewerSubClip::new("Run", MariaAnimClips::RUN),
    ViewerSubClip::new("Cower / Panic", MariaAnimClips::COWER_PANIC),
    ViewerSubClip::new("Flinch / Downed", MariaAnimClips::FLINCH_DOWNED),
];

const RED_SUBCLIPS: &[ViewerSubClip] = &[
    ViewerSubClip::new("5401: Stand Idle", PyramidHeadAnimClips::STAND),
    ViewerSubClip::new("5402: Walk", PyramidHeadAnimClips::WALK),
    ViewerSubClip::new(
        "5404: Overhead Great Knife",
        PyramidHeadAnimClips::KNIFE_SWING,
    ),
    ViewerSubClip::new("5406: One-Handed Grab", PyramidHeadAnimClips::CHOKE_HOLD),
];

const PAP_SUBCLIPS: &[ViewerSubClip] = &[
    ViewerSubClip::new("5701: Stand Idle", AbstractDaddyAnimClips::STAND),
    ViewerSubClip::new("5702: Lumbering Walk", AbstractDaddyAnimClips::WALK),
    ViewerSubClip::new("5706: Lunge Grab", AbstractDaddyAnimClips::LUNGE_GRAB),
    ViewerSubClip::new("5712: Flinch", AbstractDaddyAnimClips::FLINCH),
    ViewerSubClip::new("5717: Knockdown", AbstractDaddyAnimClips::KNOCKDOWN),
    ViewerSubClip::new("5718: Death", AbstractDaddyAnimClips::DEATH),
];

const IKE_SUBCLIPS: &[ViewerSubClip] = &[
    ViewerSubClip::new("4601: Hang Idle", FleshLipsAnimClips::HANG_IDLE),
    ViewerSubClip::new("4602: Glide Advance", FleshLipsAnimClips::GLIDE_ADVANCE),
    ViewerSubClip::new("4603: Grab Descend", FleshLipsAnimClips::GRAB_DESCEND),
    ViewerSubClip::new("4607: Kick Thrash", FleshLipsAnimClips::KICK_THRASH),
    ViewerSubClip::new("4613: Flinch", FleshLipsAnimClips::FLINCH),
    ViewerSubClip::new("4615: Fall Death", FleshLipsAnimClips::FALL_DEATH),
];

const ARM_SUBCLIPS: &[ViewerSubClip] = &[
    ViewerSubClip::new("5601: Hang Idle", MandarinAnimClips::HANG_IDLE),
    ViewerSubClip::new("5602: Under-Grate Crawl", MandarinAnimClips::CRAWL),
    ViewerSubClip::new("5605: Tentacle Strike", MandarinAnimClips::TENTACLE_STRIKE),
    ViewerSubClip::new("5608: Flinch", MandarinAnimClips::FLINCH),
    ViewerSubClip::new("5610: Drop Fall", MandarinAnimClips::KNOCKDOWN),
];

const TYU_SUBCLIPS: &[ViewerSubClip] = &[
    ViewerSubClip::new("5201: Scuttle", CreeperAnimClips::SCUTTLE),
    ViewerSubClip::new("5203: Bite", CreeperAnimClips::BITE),
];

const BOS_SUBCLIPS: &[ViewerSubClip] = &[
    ViewerSubClip::new("5801: Hover Idle", MaryBossAnimClips::HOVER_IDLE),
    ViewerSubClip::new("5802: Lunge Catch", MaryBossAnimClips::LUNGE_CATCH),
    ViewerSubClip::new("5803: Lift Carry", MaryBossAnimClips::LIFT_CARRY),
    ViewerSubClip::new("5804: Choke Hold", MaryBossAnimClips::CHOKE_HOLD),
    ViewerSubClip::new("5805: Release", MaryBossAnimClips::RELEASE),
    ViewerSubClip::new("5806: Flinch", MaryBossAnimClips::FLINCH),
    ViewerSubClip::new("5807: Stagger", MaryBossAnimClips::STAGGER),
    ViewerSubClip::new("5808: Death Fall", MaryBossAnimClips::DEATH_FALL),
    ViewerSubClip::new("5809: Floor Body", MaryBossAnimClips::FLOOR_BODY),
];

// ============================================================================
// Weapons & Key Items (`jms_weapon.anm` 8-weapon pose table & `p_i_letter.anm`)
// ============================================================================
// In `data/chr/wp/jms_weapon.anm` (8 frames total), each frame `0..=7` encodes
// the static 2-bone bind pose (`bone 0` = right-hand grip, `bone 1` = muzzle/tip)
// for one specific weapon ID. Playing across frames 0..=7 on a single weapon
// applies the other 7 weapons' tip positions to `bone 1`.

const WP_HANDGUN_SUBCLIPS: &[ViewerSubClip] = &[ViewerSubClip::new(
    "Weapon Pose 0: Handgun (M92F)",
    AnimationClip::new(0, 0, 30.0, true),
)];

const WP_SHOTGUN_SUBCLIPS: &[ViewerSubClip] = &[ViewerSubClip::new(
    "Weapon Pose 1: Shotgun",
    AnimationClip::new(1, 1, 30.0, true),
)];

const WP_RIFLGUN_SUBCLIPS: &[ViewerSubClip] = &[ViewerSubClip::new(
    "Weapon Pose 2: Hunting Rifle",
    AnimationClip::new(2, 2, 30.0, true),
)];

const WP_SP_SUBCLIPS: &[ViewerSubClip] = &[ViewerSubClip::new(
    "Weapon Pose 3: Hyper Spray Can",
    AnimationClip::new(3, 3, 30.0, true),
)];

const WP_KAKUZAI_SUBCLIPS: &[ViewerSubClip] = &[ViewerSubClip::new(
    "Weapon Pose 4: Wooden Plank",
    AnimationClip::new(4, 4, 30.0, true),
)];

const WP_PIPE_SUBCLIPS: &[ViewerSubClip] = &[ViewerSubClip::new(
    "Weapon Pose 5: Steel Pipe",
    AnimationClip::new(5, 5, 30.0, true),
)];

const WP_NATA_SUBCLIPS: &[ViewerSubClip] = &[ViewerSubClip::new(
    "Weapon Pose 6: Great Knife",
    AnimationClip::new(6, 6, 30.0, true),
)];

const WP_CSAW_SUBCLIPS: &[ViewerSubClip] = &[ViewerSubClip::new(
    "Weapon Pose 7: Chainsaw",
    AnimationClip::new(7, 7, 30.0, true),
)];

const I_LETTER_SUBCLIPS: &[ViewerSubClip] = &[ViewerSubClip::new(
    "Letter Unfold & Hold",
    AnimationClip::new(0, 72, 30.0, true),
)];

/// Returns the slice of named PS2 animation sub-clips for a given `(model_id, anm_rel_path)` pair.
pub fn sub_clips_for_anm(model_id: &str, anm_rel_path: &str) -> &'static [ViewerSubClip] {
    match (model_id, anm_rel_path) {
        ("agl", "data/chr/agl/p_agl.anm") => AGL_PLAY_SUBCLIPS,
        ("agl", "data/demo/haka_agl/agl.anm") => AGL_HAKA_SUBCLIPS,
        ("agl", "data/demo/knife_agl/agl.anm") => AGL_KNIFE_SUBCLIPS,
        ("agl", "data/demo/papa_agl/agl.anm") => AGL_PAPA_SUBCLIPS,
        ("agl", "data/demo/fire_agl/agl.anm") => AGL_FIRE_SUBCLIPS,

        ("hhh_edi", "data/chr/edi/p_edi.anm") => EDI_PLAY_SUBCLIPS,
        ("hhh_edi", "data/demo/gero_edi/hhh_edi.anm") => EDI_GERO_SUBCLIPS,
        ("hhh_edi", "data/demo/edi_korosu/hhh_edi.anm") => EDI_KOROSU_SUBCLIPS,
        ("hhh_edi", "data/demo/bp_edi_1/hhh_edi.anm") => EDI_BP1_SUBCLIPS,
        ("hhh_edi", "data/demo/bp_edi_2/hhh_edi.anm") => EDI_BP2_SUBCLIPS,
        ("hhh_edi", "data/demo/killer_edi/hhh_edi.anm") => EDI_KILLER_SUBCLIPS,
        ("hhh_edi", "data/demo/killer_edi_e/hhh_edi.anm") => EDI_KILLER_E_SUBCLIPS,
        ("hhh_edi", "data/demo/killer_edi_i/hhh_edi.anm") => EDI_KILLER_I_SUBCLIPS,

        ("lll_edi", "data/chr/edi/lll_edi.anm") => EDB_BOSS_SUBCLIPS,
        ("lll_edi", "data/demo/killer_edi_d/lll_edi.anm") => EDB_DEATH_SUBCLIPS,

        ("lau", "data/chr/lau/p_lau.anm") => LAU_PLAY_SUBCLIPS,
        ("lau", "data/demo/kagikeri/lau.anm") => LAU_KAGIKERI_SUBCLIPS,
        ("lau", "data/demo/hei_lau/lau.anm") => LAU_HEI_SUBCLIPS,
        ("lau", "data/demo/bp_edi_1/lau.anm") => LAU_BP1_SUBCLIPS,
        ("lau", "data/demo/bp_edi_2/lau.anm") => LAU_BP2_SUBCLIPS,
        ("lau", "data/demo/hp_lau/lau.anm") => LAU_HP_SUBCLIPS,
        ("lau", "data/demo/lau_damasu/lau.anm") => LAU_DAMASU_SUBCLIPS,
        ("lau", "data/demo/lau_damasu_m/lau.anm") => LAU_DAMASU_M_SUBCLIPS,
        ("lau", "data/demo/piano_lau/lau.anm") => LAU_PIANO_SUBCLIPS,
        ("lau", "data/demo/room312/lau.anm") => LAU_ROOM312_SUBCLIPS,
        ("lau", "data/demo/exit/lau.anm") => LAU_EXIT_SUBCLIPS,
        ("lau", "data/demo/mry_yarinaoshi_i/lau.anm") => LAU_YARINAOSHI_SUBCLIPS,

        ("mry", "data/chr/mry/p_mry.anm") => MRY_PLAY_SUBCLIPS,
        ("mry", "data/demo/fukatsu_a/mry.anm") => MRY_FUKATSU_SUBCLIPS,
        ("mry", "data/demo/jisatsu_a/mry.anm") => MRY_JISATSU_SUBCLIPS,
        ("mry", "data/demo/mar_isho_a/mry.anm") => MRY_MAR_ISHO_SUBCLIPS,
        ("mry", "data/demo/mry_yarinaoshi_a/mry.anm") => MRY_YARINAOSHI_SUBCLIPS,

        ("scu", "data/chr/scu/scu.anm") => SCU_SUBCLIPS,
        ("mkn", "data/chr/mkn/mkn.anm") => MKN_SUBCLIPS,
        ("nse" | "xoo", _) => NSE_SUBCLIPS,
        ("xmar", "data/chr/mar/lll_mar.anm") => MAR_SUBCLIPS,
        ("red", "data/chr/red/red.anm") => RED_SUBCLIPS,
        ("pap", "data/chr/pap/pap.anm") => PAP_SUBCLIPS,
        ("ike", "data/chr/ike/ike.anm") => IKE_SUBCLIPS,
        ("arm", "data/chr/arm/arm.anm") => ARM_SUBCLIPS,
        ("tyu", "data/chr/tyu/tyu.anm") => TYU_SUBCLIPS,
        ("bos", "data/chr/bos/bos.anm") => BOS_SUBCLIPS,

        ("wp_handgun", "data/chr/wp/jms_weapon.anm") => WP_HANDGUN_SUBCLIPS,
        ("wp_shotgun", "data/chr/wp/jms_weapon.anm") => WP_SHOTGUN_SUBCLIPS,
        ("wp_riflgun", "data/chr/wp/jms_weapon.anm") => WP_RIFLGUN_SUBCLIPS,
        ("wp_sp", "data/chr/wp/jms_weapon.anm") => WP_SP_SUBCLIPS,
        ("wp_kakuzai", "data/chr/wp/jms_weapon.anm") => WP_KAKUZAI_SUBCLIPS,
        ("wp_pipe", "data/chr/wp/jms_weapon.anm") => WP_PIPE_SUBCLIPS,
        ("wp_nata", "data/chr/wp/jms_weapon.anm") => WP_NATA_SUBCLIPS,
        ("wp_csaw", "data/chr/wp/jms_weapon.anm") => WP_CSAW_SUBCLIPS,
        ("i_letter", "data/chr/item/p_i_letter.anm") => I_LETTER_SUBCLIPS,
        _ => &[],
    }
}
