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
        "3502: Graveyard Part 1",
        AnimationClip::new(0, 882, 30.0, false),
    ),
    ViewerSubClip::new(
        "3503: Graveyard Part 2",
        AnimationClip::new(883, 1320, 30.0, false),
    ),
    ViewerSubClip::new(
        "3504: Graveyard Part 3",
        AnimationClip::new(1321, 2119, 30.0, false),
    ),
    ViewerSubClip::new(
        "3505: Graveyard Part 4",
        AnimationClip::new(2120, 2969, 30.0, false),
    ),
    ViewerSubClip::new(
        "3506: Graveyard Part 5",
        AnimationClip::new(2970, 3612, 30.0, false),
    ),
    ViewerSubClip::new(
        "3507: Graveyard Part 6",
        AnimationClip::new(3613, 4325, 30.0, false),
    ),
    ViewerSubClip::new(
        "3508: Graveyard Part 7",
        AnimationClip::new(4326, 5480, 30.0, false),
    ),
];

const AGL_KNIFE_SUBCLIPS: &[ViewerSubClip] = &[
    ViewerSubClip::new(
        "3509: Mirror Knife Part 1",
        AnimationClip::new(0, 683, 30.0, false),
    ),
    ViewerSubClip::new(
        "3510: Mirror Knife Part 2",
        AnimationClip::new(684, 1346, 30.0, false),
    ),
    ViewerSubClip::new(
        "3511: Mirror Knife Part 3",
        AnimationClip::new(1347, 1842, 30.0, false),
    ),
    ViewerSubClip::new(
        "3512: Mirror Knife Part 4",
        AnimationClip::new(1843, 2643, 30.0, false),
    ),
    ViewerSubClip::new(
        "3513: Mirror Knife Part 5",
        AnimationClip::new(2644, 3283, 30.0, false),
    ),
    ViewerSubClip::new(
        "3514: Mirror Knife Part 6",
        AnimationClip::new(3284, 3740, 30.0, false),
    ),
    ViewerSubClip::new(
        "3515: Mirror Knife Part 7",
        AnimationClip::new(3741, 4012, 30.0, false),
    ),
];

const AGL_PAPA_SUBCLIPS: &[ViewerSubClip] = &[
    ViewerSubClip::new(
        "3516: Abstract Daddy Part 1",
        AnimationClip::new(0, 559, 30.0, false),
    ),
    ViewerSubClip::new(
        "3517: Abstract Daddy Part 2",
        AnimationClip::new(560, 1379, 30.0, false),
    ),
    ViewerSubClip::new(
        "3518: Abstract Daddy Part 3",
        AnimationClip::new(1380, 2125, 30.0, false),
    ),
    ViewerSubClip::new(
        "3519: Abstract Daddy Part 4",
        AnimationClip::new(2126, 2829, 30.0, false),
    ),
    ViewerSubClip::new(
        "3520: Abstract Daddy Part 5",
        AnimationClip::new(2830, 3588, 30.0, false),
    ),
    ViewerSubClip::new(
        "3521: Abstract Daddy Part 6",
        AnimationClip::new(3589, 4429, 30.0, false),
    ),
    ViewerSubClip::new(
        "3522: Abstract Daddy Part 7",
        AnimationClip::new(4430, 4935, 30.0, false),
    ),
    ViewerSubClip::new(
        "3523: Abstract Daddy Part 8",
        AnimationClip::new(4936, 5486, 30.0, false),
    ),
    ViewerSubClip::new(
        "3524: Abstract Daddy Part 9",
        AnimationClip::new(5487, 5941, 30.0, false),
    ),
    ViewerSubClip::new(
        "3525: Abstract Daddy Part 10",
        AnimationClip::new(5942, 6370, 30.0, false),
    ),
];

const AGL_FIRE_SUBCLIPS: &[ViewerSubClip] = &[
    ViewerSubClip::new(
        "3526: Burning Staircase Part 1",
        AnimationClip::new(0, 944, 30.0, false),
    ),
    ViewerSubClip::new(
        "3527: Burning Staircase Part 2",
        AnimationClip::new(945, 1615, 30.0, false),
    ),
    ViewerSubClip::new(
        "3528: Burning Staircase Part 3",
        AnimationClip::new(1616, 2488, 30.0, false),
    ),
    ViewerSubClip::new(
        "3529: Burning Staircase Part 4",
        AnimationClip::new(2489, 3800, 30.0, false),
    ),
    ViewerSubClip::new(
        "3530: Burning Staircase Part 5",
        AnimationClip::new(3801, 4660, 30.0, false),
    ),
    ViewerSubClip::new(
        "3531: Burning Staircase Part 6",
        AnimationClip::new(4661, 5938, 30.0, false),
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
        "4502: Toilet Vomiting Part 1",
        AnimationClip::new(0, 84, 30.0, true),
    ),
    ViewerSubClip::new(
        "4503: Toilet Vomiting Part 2",
        AnimationClip::new(85, 1068, 30.0, false),
    ),
    ViewerSubClip::new(
        "4504: Toilet Vomiting Part 3",
        AnimationClip::new(1069, 2701, 30.0, false),
    ),
    ViewerSubClip::new(
        "4505: Toilet Vomiting Part 4",
        AnimationClip::new(2702, 3492, 30.0, false),
    ),
    ViewerSubClip::new(
        "4506: Toilet Vomiting Part 5",
        AnimationClip::new(3493, 3663, 30.0, false),
    ),
    ViewerSubClip::new(
        "4507: Toilet Vomiting Part 6",
        AnimationClip::new(3664, 4156, 30.0, false),
    ),
];

const EDI_KOROSU_SUBCLIPS: &[ViewerSubClip] = &[
    ViewerSubClip::new(
        "4513: Prison Corpse Part 1",
        AnimationClip::new(0, 660, 30.0, false),
    ),
    ViewerSubClip::new(
        "4514: Prison Corpse Part 2",
        AnimationClip::new(661, 1041, 30.0, false),
    ),
    ViewerSubClip::new(
        "4515: Prison Corpse Part 3",
        AnimationClip::new(1042, 1213, 30.0, false),
    ),
    ViewerSubClip::new(
        "4516: Prison Corpse Part 4",
        AnimationClip::new(1214, 1590, 30.0, false),
    ),
    ViewerSubClip::new(
        "4517: Prison Corpse Part 5",
        AnimationClip::new(1591, 2035, 30.0, false),
    ),
    ViewerSubClip::new(
        "4518: Prison Corpse Part 6",
        AnimationClip::new(2036, 2246, 30.0, false),
    ),
];

const EDI_BP1_SUBCLIPS: &[ViewerSubClip] = &[
    ViewerSubClip::new(
        "4508: Bowling Alley 1 Part 1",
        AnimationClip::new(0, 967, 30.0, false),
    ),
    ViewerSubClip::new(
        "4509: Bowling Alley 1 Part 2",
        AnimationClip::new(968, 1939, 30.0, false),
    ),
    ViewerSubClip::new(
        "4510: Bowling Alley 1 Part 3",
        AnimationClip::new(1940, 2871, 30.0, false),
    ),
];

const EDI_BP2_SUBCLIPS: &[ViewerSubClip] = &[
    ViewerSubClip::new(
        "4511: Bowling Alley 2 Part 1",
        AnimationClip::new(0, 763, 30.0, false),
    ),
    ViewerSubClip::new(
        "4512: Bowling Alley 2 Part 2",
        AnimationClip::new(764, 1189, 30.0, false),
    ),
];

const EDI_KILLER_SUBCLIPS: &[ViewerSubClip] = &[
    ViewerSubClip::new(
        "4519: Meat Locker Intro Part 1",
        AnimationClip::new(0, 490, 30.0, false),
    ),
    ViewerSubClip::new(
        "4520: Meat Locker Intro Part 2",
        AnimationClip::new(491, 1301, 30.0, false),
    ),
    ViewerSubClip::new(
        "4521: Meat Locker Intro Part 3",
        AnimationClip::new(1302, 1688, 30.0, false),
    ),
];

const EDI_KILLER_E_SUBCLIPS: &[ViewerSubClip] = &[
    ViewerSubClip::new(
        "4522: Eddie Death Part 1",
        AnimationClip::new(0, 669, 30.0, false),
    ),
    ViewerSubClip::new(
        "4523: Eddie Death Part 2",
        AnimationClip::new(670, 1448, 30.0, false),
    ),
    ViewerSubClip::new(
        "4524: Eddie Death Part 3",
        AnimationClip::new(1449, 1918, 30.0, false),
    ),
    ViewerSubClip::new(
        "4525: Eddie Death Part 4",
        AnimationClip::new(1919, 2569, 30.0, false),
    ),
    ViewerSubClip::new(
        "4526: Eddie Death Part 5",
        AnimationClip::new(2570, 2780, 30.0, false),
    ),
    ViewerSubClip::new(
        "4527: Eddie Death Part 6",
        AnimationClip::new(2781, 4051, 30.0, false),
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

const LAU_KICK_SUBCLIPS: &[ViewerSubClip] = &[
    ViewerSubClip::new(
        "2502: Key Kick Part 1",
        AnimationClip::new(0, 623, 30.0, false),
    ),
    ViewerSubClip::new(
        "2503: Key Kick Part 2",
        AnimationClip::new(624, 1521, 30.0, false),
    ),
];

const LAU_WALL_SUBCLIPS: &[ViewerSubClip] = &[
    ViewerSubClip::new(
        "2504: Brick Wall Part 1",
        AnimationClip::new(0, 504, 30.0, false),
    ),
    ViewerSubClip::new(
        "2505: Brick Wall Part 2",
        AnimationClip::new(505, 845, 30.0, false),
    ),
    ViewerSubClip::new(
        "2506: Brick Wall Part 3",
        AnimationClip::new(846, 1344, 30.0, false),
    ),
    ViewerSubClip::new(
        "2507: Brick Wall Part 4",
        AnimationClip::new(1345, 1988, 30.0, false),
    ),
    ViewerSubClip::new(
        "2508: Brick Wall Part 5",
        AnimationClip::new(1989, 2529, 30.0, false),
    ),
    ViewerSubClip::new(
        "2509: Brick Wall Part 6",
        AnimationClip::new(2530, 3098, 30.0, false),
    ),
    ViewerSubClip::new(
        "2510: Brick Wall Part 7",
        AnimationClip::new(3099, 3812, 30.0, false),
    ),
    ViewerSubClip::new(
        "2511: Brick Wall Part 8",
        AnimationClip::new(3813, 4199, 30.0, false),
    ),
    ViewerSubClip::new(
        "2512: Brick Wall Part 9",
        AnimationClip::new(4200, 4449, 30.0, false),
    ),
];

const LAU_BP1_SUBCLIPS: &[ViewerSubClip] = &[
    ViewerSubClip::new(
        "2513: Bowling Alley 1 Part 1",
        AnimationClip::new(0, 967, 30.0, false),
    ),
    ViewerSubClip::new(
        "2514: Bowling Alley 1 Part 2",
        AnimationClip::new(968, 1939, 30.0, false),
    ),
    ViewerSubClip::new(
        "2515: Bowling Alley 1 Part 3",
        AnimationClip::new(1940, 2871, 30.0, false),
    ),
];

const LAU_BP2_SUBCLIPS: &[ViewerSubClip] = &[
    ViewerSubClip::new(
        "2516: Bowling Alley 2 Part 1",
        AnimationClip::new(0, 763, 30.0, false),
    ),
    ViewerSubClip::new(
        "2517: Bowling Alley 2 Part 2",
        AnimationClip::new(764, 1189, 30.0, false),
    ),
];

const LAU_TEDDY_SUBCLIPS: &[ViewerSubClip] = &[
    ViewerSubClip::new(
        "2518: Hospital Teddy Part 1",
        AnimationClip::new(0, 434, 30.0, false),
    ),
    ViewerSubClip::new(
        "2519: Hospital Teddy Part 2",
        AnimationClip::new(435, 1134, 30.0, false),
    ),
    ViewerSubClip::new(
        "2520: Hospital Teddy Part 3",
        AnimationClip::new(1135, 1919, 30.0, false),
    ),
    ViewerSubClip::new(
        "2521: Hospital Teddy Part 4",
        AnimationClip::new(1920, 2504, 30.0, false),
    ),
    ViewerSubClip::new(
        "2522: Hospital Teddy Part 5",
        AnimationClip::new(2505, 3260, 30.0, false),
    ),
    ViewerSubClip::new(
        "2523: Hospital Teddy Part 6",
        AnimationClip::new(3261, 4100, 30.0, false),
    ),
    ViewerSubClip::new(
        "2524: Hospital Teddy Part 7",
        AnimationClip::new(4101, 4291, 30.0, false),
    ),
];

const LAU_DAMASI_SUBCLIPS: &[ViewerSubClip] = &[
    ViewerSubClip::new(
        "2525: Exam Room Trick Part 1",
        AnimationClip::new(0, 569, 30.0, false),
    ),
    ViewerSubClip::new(
        "2526: Exam Room Trick Part 2",
        AnimationClip::new(570, 1289, 30.0, false),
    ),
    ViewerSubClip::new(
        "2527: Exam Room Trick Part 3",
        AnimationClip::new(1290, 2005, 30.0, false),
    ),
    ViewerSubClip::new(
        "2528: Exam Room Trick Part 4",
        AnimationClip::new(2006, 2609, 30.0, false),
    ),
    ViewerSubClip::new(
        "2529: Exam Room Trick Part 5",
        AnimationClip::new(2610, 3157, 30.0, false),
    ),
];

const LAU_PIANO1_SUBCLIPS: &[ViewerSubClip] = &[
    ViewerSubClip::new(
        "2530: Hotel Piano 1 Part 1",
        AnimationClip::new(0, 811, 30.0, false),
    ),
    ViewerSubClip::new(
        "2531: Hotel Piano 1 Part 2",
        AnimationClip::new(812, 1699, 30.0, false),
    ),
    ViewerSubClip::new(
        "2532: Hotel Piano 1 Part 3",
        AnimationClip::new(1700, 2248, 30.0, false),
    ),
    ViewerSubClip::new(
        "2533: Hotel Piano 1 Part 4",
        AnimationClip::new(2249, 2569, 30.0, false),
    ),
    ViewerSubClip::new(
        "2534: Hotel Piano 1 Part 5",
        AnimationClip::new(2570, 3269, 30.0, false),
    ),
    ViewerSubClip::new(
        "2535: Hotel Piano 1 Part 6",
        AnimationClip::new(3270, 4050, 30.0, false),
    ),
];

const LAU_PIANO2_SUBCLIPS: &[ViewerSubClip] = &[
    ViewerSubClip::new(
        "2536: Hotel Piano 2 Part 1",
        AnimationClip::new(0, 419, 30.0, false),
    ),
    ViewerSubClip::new(
        "2537: Hotel Piano 2 Part 2",
        AnimationClip::new(420, 1084, 30.0, false),
    ),
    ViewerSubClip::new(
        "2538: Hotel Piano 2 Part 3",
        AnimationClip::new(1085, 1986, 30.0, false),
    ),
];

const LAU_KOKUHAKU_SUBCLIPS: &[ViewerSubClip] = &[
    ViewerSubClip::new(
        "2539: Room 312 Confession 1",
        AnimationClip::new(0, 629, 30.0, false),
    ),
    ViewerSubClip::new(
        "2540: Room 312 Confession 2",
        AnimationClip::new(630, 1184, 30.0, false),
    ),
    ViewerSubClip::new(
        "2541: Room 312 Confession 3",
        AnimationClip::new(1185, 1939, 30.0, false),
    ),
    ViewerSubClip::new(
        "2542: Room 312 Confession 4",
        AnimationClip::new(1940, 2589, 30.0, false),
    ),
    ViewerSubClip::new(
        "2543: Room 312 Confession 5",
        AnimationClip::new(2590, 3259, 30.0, false),
    ),
    ViewerSubClip::new(
        "2544: Room 312 Confession 6",
        AnimationClip::new(3260, 4169, 30.0, false),
    ),
    ViewerSubClip::new(
        "2545: Room 312 Confession 7",
        AnimationClip::new(4170, 4800, 30.0, false),
    ),
    ViewerSubClip::new(
        "2546: Room 312 Confession 8",
        AnimationClip::new(4801, 5605, 30.0, false),
    ),
];

const MRY_PLAY_SUBCLIPS: &[ViewerSubClip] = &[
    ViewerSubClip::new("4091: Walk / Apparition A", MaryNpcAnimClips::WALK_A),
    ViewerSubClip::new("4092: Walk / Apparition B", MaryNpcAnimClips::WALK_B),
];

const MRY_BEG_SUBCLIPS: &[ViewerSubClip] = &[
    ViewerSubClip::new(
        "4002: Final Boss Intro Part 1",
        AnimationClip::new(0, 725, 30.0, false),
    ),
    ViewerSubClip::new(
        "4003: Final Boss Intro Part 2",
        AnimationClip::new(726, 1344, 30.0, false),
    ),
    ViewerSubClip::new(
        "4004: Final Boss Intro Part 3",
        AnimationClip::new(1345, 2248, 30.0, false),
    ),
    ViewerSubClip::new(
        "4005: Final Boss Intro Part 4",
        AnimationClip::new(2249, 2585, 30.0, false),
    ),
    ViewerSubClip::new(
        "4006: Final Boss Intro Part 5",
        AnimationClip::new(2586, 3034, 30.0, false),
    ),
    ViewerSubClip::new(
        "4007: Final Boss Intro Part 6",
        AnimationClip::new(3035, 3600, 30.0, false),
    ),
    ViewerSubClip::new(
        "4008: Final Boss Intro Part 7",
        AnimationClip::new(3601, 4126, 30.0, false),
    ),
];

const MRY_END_A_SUBCLIPS: &[ViewerSubClip] = &[
    ViewerSubClip::new(
        "4009: Leave Ending Part 1",
        AnimationClip::new(0, 629, 30.0, false),
    ),
    ViewerSubClip::new(
        "4010: Leave Ending Part 2",
        AnimationClip::new(630, 1299, 30.0, false),
    ),
    ViewerSubClip::new(
        "4011: Leave Ending Part 3",
        AnimationClip::new(1300, 1794, 30.0, false),
    ),
    ViewerSubClip::new(
        "4012: Leave Ending Part 4",
        AnimationClip::new(1795, 2329, 30.0, false),
    ),
    ViewerSubClip::new(
        "4013: Leave Ending Part 5",
        AnimationClip::new(2330, 3059, 30.0, false),
    ),
    ViewerSubClip::new(
        "4014: Leave Ending Part 6",
        AnimationClip::new(3060, 3644, 30.0, false),
    ),
    ViewerSubClip::new(
        "4015: Leave Ending Part 7",
        AnimationClip::new(3645, 4564, 30.0, false),
    ),
    ViewerSubClip::new(
        "4016: Leave Ending Part 8",
        AnimationClip::new(4565, 5134, 30.0, false),
    ),
    ViewerSubClip::new(
        "4017: Leave Ending Part 9",
        AnimationClip::new(5135, 5884, 30.0, false),
    ),
    ViewerSubClip::new(
        "4018: Leave Ending Part 10",
        AnimationClip::new(5885, 6506, 30.0, false),
    ),
    ViewerSubClip::new(
        "4019: Leave Ending Part 11",
        AnimationClip::new(6507, 7440, 30.0, false),
    ),
];

const MRY_END_B_SUBCLIPS: &[ViewerSubClip] = &[
    ViewerSubClip::new(
        "4020: In Water Ending Part 1",
        AnimationClip::new(0, 819, 30.0, false),
    ),
    ViewerSubClip::new(
        "4021: In Water Ending Part 2",
        AnimationClip::new(820, 1469, 30.0, false),
    ),
    ViewerSubClip::new(
        "4022: In Water Ending Part 3",
        AnimationClip::new(1470, 2114, 30.0, false),
    ),
    ViewerSubClip::new(
        "4023: In Water Ending Part 4",
        AnimationClip::new(2115, 2864, 30.0, false),
    ),
    ViewerSubClip::new(
        "4024: In Water Ending Part 5",
        AnimationClip::new(2865, 3449, 30.0, false),
    ),
    ViewerSubClip::new(
        "4025: In Water Ending Part 6",
        AnimationClip::new(3450, 4214, 30.0, false),
    ),
    ViewerSubClip::new(
        "4026: In Water Ending Part 7",
        AnimationClip::new(4215, 5024, 30.0, false),
    ),
    ViewerSubClip::new(
        "4027: In Water Ending Part 8",
        AnimationClip::new(5025, 5609, 30.0, false),
    ),
    ViewerSubClip::new(
        "4028: In Water Ending Part 9",
        AnimationClip::new(5610, 6280, 30.0, false),
    ),
];

const MRY_END_D_SUBCLIPS: &[ViewerSubClip] = &[
    ViewerSubClip::new(
        "4029: Rebirth Ending Part 1",
        AnimationClip::new(0, 654, 30.0, false),
    ),
    ViewerSubClip::new(
        "4030: Rebirth Ending Part 2",
        AnimationClip::new(655, 1344, 30.0, false),
    ),
    ViewerSubClip::new(
        "4031: Rebirth Ending Part 3",
        AnimationClip::new(1345, 2005, 30.0, false),
    ),
    ViewerSubClip::new(
        "4032: Rebirth Ending Part 4",
        AnimationClip::new(2006, 2639, 30.0, false),
    ),
    ViewerSubClip::new(
        "4033: Rebirth Ending Part 5",
        AnimationClip::new(2640, 3158, 30.0, false),
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

        ("lll_edi", "data/chr/edi/lll_edi.anm") => EDB_BOSS_SUBCLIPS,
        ("lll_edi", "data/demo/killer_edi_d/lll_edi.anm") => EDB_DEATH_SUBCLIPS,

        ("lau", "data/chr/lau/p_lau.anm") => LAU_PLAY_SUBCLIPS,
        ("lau", "data/demo/kick_key/lau.anm") => LAU_KICK_SUBCLIPS,
        ("lau", "data/demo/lau_wall/lau.anm") => LAU_WALL_SUBCLIPS,
        ("lau", "data/demo/bp_edi_1/lau.anm") => LAU_BP1_SUBCLIPS,
        ("lau", "data/demo/bp_edi_2/lau.anm") => LAU_BP2_SUBCLIPS,
        ("lau", "data/demo/teddy_a/lau.anm") => LAU_TEDDY_SUBCLIPS,
        ("lau", "data/demo/lau_damasi/lau.anm") => LAU_DAMASI_SUBCLIPS,
        ("lau", "data/demo/piano_lau_1/lau.anm") => LAU_PIANO1_SUBCLIPS,
        ("lau", "data/demo/piano_lau_2/lau.anm") => LAU_PIANO2_SUBCLIPS,
        ("lau", "data/demo/kokuhaku/lau.anm") => LAU_KOKUHAKU_SUBCLIPS,

        ("mry", "data/chr/mry/p_mry.anm") => MRY_PLAY_SUBCLIPS,
        ("mry", "data/demo/mry_yarinaosi_beg/mry.anm") => MRY_BEG_SUBCLIPS,
        ("mry", "data/demo/mry_yarinaosi_end_a/mry.anm") => MRY_END_A_SUBCLIPS,
        ("mry", "data/demo/mry_yarinaosi_end_b/mry.anm") => MRY_END_B_SUBCLIPS,
        ("mry", "data/demo/mry_yarinaosi_end_d/mry.anm") => MRY_END_D_SUBCLIPS,

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
        _ => &[],
    }
}
