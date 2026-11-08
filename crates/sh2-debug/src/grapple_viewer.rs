//! Interactive Creature Grapple Animation & Elevation Debug Room (`SH2-57`).
//!
//! Provides a dedicated inspection room to view and debug pairwise creature grapple
//! animations (James Sunderland vs. Flesh Lips, Mary Boss, Pyramid Head, Abstract Daddy,
//! Lying Figure, and Creeper). Features a 360-degree free orbit / flying perspective
//! identical to the 3D Model Viewer, frame scrubbing, playback speed control, interactive
//! real-time James elevation adjustments, and bone skeleton / wireframe overlays.

use glam::Vec3;
use sh2_core::animation::{AnimationClip, JamesBasicClips, JamesStageClips};

use crate::model_viewer::OrbitCamera;

/// Source of animation data for James Sunderland.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JamesAnmSource {
    /// Basic character locomotion & struggle animations from `jms_basic1.anm`.
    Basic,
    /// Stage-specific Flesh Lips choke animations from `jms_stg_hospital_1fw_b.anm` (`gy_hxw`).
    HospitalStage,
    /// Stage-specific Mary Final Boss tentacle choke animations from `jms_stg_end_recovery.anm` (`gz_erc`).
    HotelStage,
    /// Unarmed combat, reactions & grabs from `jms_wpnone.anm`.
    WpNone,
    /// Apartments 2F/3F Pyramid Head encounter animations from `jms_stg_apart_w2f.anm`.
    ApartW2F,
    /// Apartments staircase Pyramid Head encounter animations from `jms_stg_apart_stair.anm`.
    ApartStair,
    /// Labyrinth East Abstract Daddy boss room animations from `jms_stg_labyrinth_e.anm`.
    LabyrinthE,
    /// Hospital Roof Pyramid Head shove animations from `jms_stg_hospital_rf_f.anm`.
    HospitalRoof,
    /// Injured / limping gameplay reactions from `jms_basic2_1.anm`.
    Basic2_1,
    /// Critical health exhausted gameplay reactions from `jms_basic3.anm`.
    Basic3,
    /// Heavy movement variant animations from `jms_basic4.anm`.
    Basic4,
    /// Alternative locomotion animations from `jms_basic5.anm`.
    Basic5,
    /// Toluca Prison Basement animations from `jms_stg_prison_bf.anm`.
    PrisonBF,
    /// Eddie meat locker boss fight animations from `jms_stg_eddie_boss.anm`.
    EddieBoss,
    /// Toluca Lake boat rowing animations from `jms_boat.anm`.
    Boat,
}

impl JamesAnmSource {
    /// All 15 major inspectable James animation sources.
    pub const fn all() -> &'static [Self] {
        &[
            Self::HospitalStage,
            Self::HotelStage,
            Self::Basic,
            Self::WpNone,
            Self::ApartW2F,
            Self::ApartStair,
            Self::LabyrinthE,
            Self::HospitalRoof,
            Self::Basic2_1,
            Self::Basic3,
            Self::Basic4,
            Self::Basic5,
            Self::PrisonBF,
            Self::EddieBoss,
            Self::Boat,
        ]
    }

    /// Relative path inside the game root directory.
    pub const fn rel_path(self) -> &'static str {
        match self {
            Self::Basic => "data/chr/jms/jms_basic1.anm",
            Self::HospitalStage => "data/chr/jms/jms_stg_hospital_1fw_b.anm",
            Self::HotelStage => "data/chr/jms/jms_stg_end_recovery.anm",
            Self::WpNone => "data/chr/jms/jms_wpnone.anm",
            Self::ApartW2F => "data/chr/jms/jms_stg_apart_w2f.anm",
            Self::ApartStair => "data/chr/jms/jms_stg_apart_stair.anm",
            Self::LabyrinthE => "data/chr/jms/jms_stg_labyrinth_e.anm",
            Self::HospitalRoof => "data/chr/jms/jms_stg_hospital_rf_f.anm",
            Self::Basic2_1 => "data/chr/jms/jms_basic2_1.anm",
            Self::Basic3 => "data/chr/jms/jms_basic3.anm",
            Self::Basic4 => "data/chr/jms/jms_basic4.anm",
            Self::Basic5 => "data/chr/jms/jms_basic5.anm",
            Self::PrisonBF => "data/chr/jms/jms_stg_prison_bf.anm",
            Self::EddieBoss => "data/chr/jms/jms_stg_eddie_boss.anm",
            Self::Boat => "data/chr/jms/jms_boat.anm",
        }
    }

    /// Resolve a [`JamesAnmSource`] from its relative `.anm` asset path.
    pub fn from_rel_path(rel_path: &str) -> Option<Self> {
        Self::all()
            .iter()
            .find(|&&src| src.rel_path() == rel_path)
            .copied()
    }

    /// Descriptive human-readable label with context.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Basic => "jms_basic1.anm (Standard Gameplay & Struggles)",
            Self::HospitalStage => "jms_stg_hospital_1fw_b.anm (Hospital 1FW Boss - Flesh Lips)",
            Self::HotelStage => "jms_stg_end_recovery.anm (Final Boss Stage - Mary Boss)",
            Self::WpNone => "jms_wpnone.anm (Unarmed Combat & Grapples)",
            Self::ApartW2F => "jms_stg_apart_w2f.anm (Apartments 2F/3F - Pyramid Head)",
            Self::ApartStair => "jms_stg_apart_stair.anm (Apartments Staircase - Pyramid Head)",
            Self::LabyrinthE => "jms_stg_labyrinth_e.anm (Labyrinth - Abstract Daddy)",
            Self::HospitalRoof => "jms_stg_hospital_rf_f.anm (Hospital Roof - Pyramid Head)",
            Self::Basic2_1 => "jms_basic2_1.anm (Injured Gameplay Reactions)",
            Self::Basic3 => "jms_basic3.anm (Critical Health Reactions)",
            Self::Basic4 => "jms_basic4.anm (Heavy Movement Variant)",
            Self::Basic5 => "jms_basic5.anm (Alternative Locomotion)",
            Self::PrisonBF => "jms_stg_prison_bf.anm (Toluca Prison Basement)",
            Self::EddieBoss => "jms_stg_eddie_boss.anm (Eddie Meat Locker Boss)",
            Self::Boat => "jms_boat.anm (Toluca Lake Rowing)",
        }
    }

    /// Total verified frame count in this animation file.
    pub const fn total_frames(self) -> u32 {
        match self {
            Self::Basic => 1112,
            Self::HospitalStage => 419,
            Self::HotelStage => 317,
            Self::WpNone => 1024,
            Self::ApartW2F => 455,
            Self::ApartStair => 554,
            Self::LabyrinthE => 481,
            Self::HospitalRoof => 410,
            Self::Basic2_1 => 1082,
            Self::Basic3 => 1002,
            Self::Basic4 => 1192,
            Self::Basic5 => 1282,
            Self::PrisonBF => 365,
            Self::EddieBoss => 230,
            Self::Boat => 225,
        }
    }
}

/// Generates all inspectable animation clips for the specified James ANM source file,
/// combining authentic curated motion sequences with complete sequential timeline window slices.
pub fn generate_clips_for_source(source: JamesAnmSource) -> Vec<(String, AnimationClip)> {
    let mut clips = Vec::new();

    // 1. Curated specific clips for known files (verified against PS2 pjames_stage_anim overlays)
    match source {
        JamesAnmSource::HospitalStage => {
            clips.push((
                "Flesh Lips Ceiling Choke Hold Loop [aid 20039] (245..=268 @ 30fps)".to_string(),
                AnimationClip::new(245, 268, 30.0, true),
            ));
            clips.push((
                "Flesh Lips Initial Choke Grab Hit [aid 20038] (230..=244 @ 15fps)".to_string(),
                AnimationClip::new(230, 244, 15.0, false),
            ));
            clips.push((
                "Flesh Lips Choke Release Drop [aid 20040] (269..=298 @ 15fps)".to_string(),
                AnimationClip::new(269, 298, 15.0, false),
            ));
            clips.push((
                "Flesh Lips Choke Death Collapse [aid 20041] (299..=328 @ 15fps)".to_string(),
                AnimationClip::new(299, 328, 15.0, false),
            ));
            clips.push((
                "Flesh Lips Grab + Choke Loop Combined (230..=268 @ 15fps)".to_string(),
                AnimationClip::new(230, 268, 15.0, true),
            ));
        }
        JamesAnmSource::HotelStage => {
            clips.push((
                "Mary Boss Phase 1: Tentacle Catch & Lift [aid 20056] (272..=286 @ 15fps)"
                    .to_string(),
                AnimationClip::new(272, 286, 15.0, false),
            ));
            clips.push((
                "Mary Boss Phase 2: Suspended Tentacle Choke Loop [aid 20054] (218..=241 @ 30fps)"
                    .to_string(),
                AnimationClip::new(218, 241, 30.0, true),
            ));
            clips.push((
                "Mary Boss Tentacle Breakout Release [aid 20055] (287..=301 @ 15fps)".to_string(),
                AnimationClip::new(287, 301, 15.0, false),
            ));
            clips.push((
                "Mary Boss Tentacle Choke Death Collapse [aid 20057] (242..=271 @ 15fps)"
                    .to_string(),
                AnimationClip::new(242, 271, 15.0, false),
            ));
        }
        JamesAnmSource::Basic => {
            clips.push((
                "Hoisted Aloft Choke [aid 121] (457..=468 @ 15fps)".to_string(),
                AnimationClip::new(457, 468, 15.0, true),
            ));
            clips.push((
                "Ground Push Reaction [aid 129] (632..=649 @ 15fps)".to_string(),
                AnimationClip::new(632, 649, 15.0, true),
            ));
            clips.push((
                "Torso Pin Struggle [aid 128] (612..=629 @ 15fps)".to_string(),
                AnimationClip::new(612, 629, 15.0, true),
            ));
            clips.push((
                "Leg Stomp & Creeper Struggle [aid 127] (588..=611 @ 30fps)".to_string(),
                AnimationClip::new(588, 611, 30.0, true),
            ));
            clips.push((
                "Breakout Shove / Kick Free [aid 130] (652..=670 @ 30fps)".to_string(),
                AnimationClip::new(652, 670, 30.0, false),
            ));
            clips.push((
                "Hurt / Damage Flinch [aid 122] (453..=470 @ 15fps)".to_string(),
                AnimationClip::new(453, 470, 15.0, false),
            ));
            clips.push((
                "Heavy Hurt / Knockback Recoil [aid 123] (471..=498 @ 15fps)".to_string(),
                AnimationClip::new(471, 498, 15.0, false),
            ));
            clips.push((
                "Ground Knockdown Stumble [aid 124] (499..=520 @ 15fps)".to_string(),
                AnimationClip::new(499, 520, 15.0, false),
            ));
            clips.push((
                "Collapse / Death Fall [aid 131] (256..=280 @ 15fps)".to_string(),
                AnimationClip::new(256, 280, 15.0, false),
            ));
            clips.push((
                "Downed Enemy Kick [aid 132] (682..=701 @ 11.25fps)".to_string(),
                AnimationClip::new(682, 701, 11.25, false),
            ));
            clips.push((
                "Downed Enemy Stomp [aid 133] (702..=719 @ 7.5fps)".to_string(),
                AnimationClip::new(702, 719, 7.5, false),
            ));
        }
        JamesAnmSource::ApartStair => {
            clips.push((
                "Pyramid Head One-Handed Choke Lift Loop [aid 20030] (455..=478 @ 30fps)"
                    .to_string(),
                AnimationClip::new(455, 478, 30.0, true),
            ));
            clips.push((
                "Pyramid Head Initial Neck Grab Hit [aid 20029] (440..=454 @ 15fps)".to_string(),
                AnimationClip::new(440, 454, 15.0, false),
            ));
            clips.push((
                "Pyramid Head Choke Release Drop [aid 20031] (479..=508 @ 15fps)".to_string(),
                AnimationClip::new(479, 508, 15.0, false),
            ));
            clips.push((
                "Pyramid Head Choke Death Drop [aid 20032] (509..=538 @ 15fps)".to_string(),
                AnimationClip::new(509, 538, 15.0, false),
            ));
        }
        JamesAnmSource::ApartW2F => {
            clips.push((
                "Apartments 2F High Lift Peak (391..=395 @ 15fps)".to_string(),
                AnimationClip::new(391, 395, 15.0, true),
            ));
            clips.push((
                "Apartments 2F Choke Reaction (351..=357 @ 15fps)".to_string(),
                AnimationClip::new(351, 357, 15.0, true),
            ));
        }
        JamesAnmSource::LabyrinthE => {
            clips.push((
                "Abstract Daddy Front Smother Hold Loop [aid 20043] (313..=336 @ 30fps)"
                    .to_string(),
                AnimationClip::new(313, 336, 30.0, true),
            ));
            clips.push((
                "Abstract Daddy Front Grab Hit [aid 20042] (298..=312 @ 15fps)".to_string(),
                AnimationClip::new(298, 312, 15.0, false),
            ));
            clips.push((
                "Abstract Daddy Front Breakout Release [aid 20045] (337..=351 @ 15fps)".to_string(),
                AnimationClip::new(337, 351, 15.0, false),
            ));
            clips.push((
                "Abstract Daddy Front Smother Death [aid 20044] (352..=381 @ 15fps)".to_string(),
                AnimationClip::new(352, 381, 15.0, false),
            ));
            clips.push((
                "Abstract Daddy Rear Smother Hold Loop [aid 20047] (397..=420 @ 30fps)".to_string(),
                AnimationClip::new(397, 420, 30.0, true),
            ));
            clips.push((
                "Abstract Daddy Rear Grab Hit [aid 20046] (382..=396 @ 15fps)".to_string(),
                AnimationClip::new(382, 396, 15.0, false),
            ));
            clips.push((
                "Abstract Daddy Rear Breakout Release [aid 20049] (421..=435 @ 15fps)".to_string(),
                AnimationClip::new(421, 435, 15.0, false),
            ));
            clips.push((
                "Abstract Daddy Rear Smother Death [aid 20048] (436..=465 @ 15fps)".to_string(),
                AnimationClip::new(436, 465, 15.0, false),
            ));
        }
        _ => {}
    }

    // 2. Uniform 25-frame window slices covering ALL frames of this file
    let total = source.total_frames();
    let window_step = 25;
    let mut s = 0;
    while s < total {
        let e = (s + window_step - 1).min(total - 1);
        clips.push((
            format!("Timeline Window [{}..={}]", s, e),
            AnimationClip::new(s, e, 15.0, true),
        ));
        s += window_step;
    }

    clips
}

/// Specification for a creature-to-James grapple pair animation in the viewer.
#[derive(Debug, Clone)]
pub struct GrapplePairEntry {
    /// Creature bestiary ID (1=scu, 4=red, 5=pap, 6=ike, 8=tyu, 9=bos).
    pub creature_id: u32,
    /// Human-readable title of the grapple encounter.
    pub name: &'static str,
    /// Creature species description.
    pub description: &'static str,
    /// Default creature animation clip.
    pub enemy_clip: AnimationClip,
    /// Human-readable label for the creature clip.
    pub enemy_clip_name: &'static str,
    /// Default James Sunderland animation clip.
    pub james_clip: AnimationClip,
    /// Human-readable label for the James clip.
    pub james_clip_name: &'static str,
    /// Animation data source for James.
    pub james_anm_source: JamesAnmSource,
    /// Alternative creature clips available for toggling (e.g. phases or stage clips).
    pub alt_enemy_clips: &'static [(AnimationClip, &'static str)],
    /// Alternative James clips available for toggling (e.g. hoisted vs grounded vs breakout).
    pub alt_james_clips: &'static [(AnimationClip, &'static str, JamesAnmSource)],
    /// Authentic relative offset $(X, Y, Z)$ of creature relative to James (in mm).
    pub default_anchor_offset: Vec3,
    /// Default vertical elevation of James in world space (in mm, where $-Y$ is up).
    pub default_james_y_offset: f32,
    /// Default vertical elevation of creature in world space (in mm).
    pub default_enemy_y_offset: f32,
    /// Default creature yaw rotation in radians.
    pub enemy_yaw: f32,
    /// Default James yaw rotation in radians.
    pub james_yaw: f32,
    /// Default camera orbit distance in mm.
    pub camera_distance: f32,
    /// Default camera focal target $Y$ elevation in mm.
    pub camera_target_y: f32,
    /// Authentic reverse-engineering notes from PS2 `SLUS_202.28`.
    pub notes: &'static str,
}

/// Master catalog of inspectable creature grapple pairs.
pub static GRAPPLE_CATALOG: &[GrapplePairEntry] = &[
    // --- 0: Flesh Lips (ike) ---
    GrapplePairEntry {
        creature_id: 6,
        name: "Flesh Lips (Ceiling Overhead Choke)",
        description: "Ceiling-mounted cage creature hanging overhead, strangling James with its lower legs",
        enemy_clip: AnimationClip::new(87, 110, 30.0, true),
        enemy_clip_name: "Ceiling Hang Choke Hold [aid 3304] (87..=110 @ 30fps)",
        james_clip: JamesStageClips::FLESH_LIPS_CEILING_CHOKE_LOOP,
        james_clip_name: "Flesh Lips Ceiling Choke Hold Loop [aid 20039] (245..=268 @ 30fps)",
        james_anm_source: JamesAnmSource::HospitalStage,
        alt_enemy_clips: &[
            (
                AnimationClip::new(72, 86, 15.0, false),
                "Ceiling Reach Down / Initial Grab [aid 3303] (72..=86 @ 15fps)",
            ),
            (
                AnimationClip::new(111, 140, 15.0, false),
                "Ceiling Choke Release [aid 3305] (111..=140 @ 15fps)",
            ),
        ],
        alt_james_clips: &[
            (
                JamesStageClips::FLESH_LIPS_CHOKE_GRAB,
                "Flesh Lips Initial Choke Grab Hit [aid 20038] (230..=244 @ 15fps)",
                JamesAnmSource::HospitalStage,
            ),
            (
                JamesStageClips::FLESH_LIPS_CHOKE_RELEASE,
                "Flesh Lips Choke Release Drop [aid 20040] (269..=298 @ 15fps)",
                JamesAnmSource::HospitalStage,
            ),
            (
                JamesStageClips::FLESH_LIPS_CHOKE_DEATH,
                "Flesh Lips Choke Death Collapse [aid 20041] (299..=328 @ 15fps)",
                JamesAnmSource::HospitalStage,
            ),
            (
                JamesStageClips::FLESH_LIPS_CHOKE_FULL,
                "Flesh Lips Grab + Choke Loop Combined (230..=268 @ 15fps)",
                JamesAnmSource::HospitalStage,
            ),
        ],
        default_anchor_offset: Vec3::new(52.1332, -1550.9032, 36.4947),
        default_james_y_offset: 0.0,
        default_enemy_y_offset: -1550.9032,
        enemy_yaw: std::f32::consts::PI,
        james_yaw: 0.0,
        camera_distance: 2400.0,
        camera_target_y: -800.0,
        notes: "PS2 gy_hxw overlay (jms_stg_hospital_1fw_b.anm 245..=268, aid=20039): 100% byte-identical 99-frame strangle block to Pyramid Head.",
    },
    // --- 1: Mary Final Boss (bos) Phase 1: Catch & Lift ---
    GrapplePairEntry {
        creature_id: 9,
        name: "Mary Final Boss (Phase 1: Catch & Lift)",
        description: "Floating bed frame cage lunges forward, catching James with prehensile tentacles",
        enemy_clip: AnimationClip::new(74, 88, 15.0, false),
        enemy_clip_name: "LIFT_CARRY Initial Catch & Lift [aid 5803] (74..=88 @ 15fps)",
        james_clip: JamesStageClips::MARY_TENTACLE_CHOKE_FULL,
        james_clip_name: "Mary Boss Phase 1: Tentacle Catch & Lift [aid 20056] (272..=286 @ 15fps)",
        james_anm_source: JamesAnmSource::HotelStage,
        alt_enemy_clips: &[
            (
                AnimationClip::new(59, 73, 30.0, false),
                "Forward Lunge Catch [aid 5802] (59..=73 @ 30fps)",
            ),
            (
                AnimationClip::new(89, 112, 30.0, true),
                "CHOKE_HOLD Looping Strangle [aid 5804] (89..=112 @ 30fps)",
            ),
        ],
        alt_james_clips: &[
            (
                JamesStageClips::MARY_TENTACLE_CHOKE_LOOP,
                "Mary Boss Phase 2: Suspended Tentacle Choke Loop [aid 20054] (218..=241 @ 30fps)",
                JamesAnmSource::HotelStage,
            ),
            (
                JamesStageClips::MARY_TENTACLE_CHOKE_RELEASE,
                "Mary Boss Tentacle Breakout Release [aid 20055] (287..=301 @ 15fps)",
                JamesAnmSource::HotelStage,
            ),
            (
                JamesStageClips::MARY_TENTACLE_CHOKE_DEATH,
                "Mary Boss Tentacle Choke Death Collapse [aid 20057] (242..=271 @ 15fps)",
                JamesAnmSource::HotelStage,
            ),
        ],
        default_anchor_offset: Vec3::new(0.0, -1496.4872, 0.0),
        default_james_y_offset: 0.0,
        default_enemy_y_offset: -1496.4872,
        enemy_yaw: std::f32::consts::PI,
        james_yaw: 0.0,
        camera_distance: 2600.0,
        camera_target_y: -950.0,
        notes: "PS2_HUGGING_POS[4] = [0.0, 1496.49, 0.0] (0x0021fbd4). Mary's cage floats at -1496.5mm overhead; tentacles reach down to James's neck.",
    },
    // --- 2: Mary Final Boss (bos) Phase 2: Looping Choke Hold ---
    GrapplePairEntry {
        creature_id: 9,
        name: "Mary Final Boss (Phase 2: Looping Choke Hold)",
        description: "Mary sustains strangulation with tentacles coiled around James's neck in lockstep loop",
        enemy_clip: AnimationClip::new(89, 112, 30.0, true),
        enemy_clip_name: "CHOKE_HOLD Looping Strangle [aid 5804] (89..=112 @ 30fps)",
        james_clip: JamesStageClips::MARY_TENTACLE_CHOKE_LOOP,
        james_clip_name: "Mary Boss Phase 2: Suspended Tentacle Choke Loop [aid 20054] (218..=241 @ 30fps)",
        james_anm_source: JamesAnmSource::HotelStage,
        alt_enemy_clips: &[
            (
                AnimationClip::new(113, 136, 30.0, false),
                "RELEASE Breakout Reaction [aid 5805] (113..=136 @ 30fps)",
            ),
            (
                AnimationClip::new(74, 88, 15.0, false),
                "LIFT_CARRY Catch [aid 5803] (74..=88 @ 15fps)",
            ),
        ],
        alt_james_clips: &[
            (
                JamesStageClips::MARY_TENTACLE_CHOKE_FULL,
                "Mary Boss Phase 1: Tentacle Catch & Lift [aid 20056] (272..=286 @ 15fps)",
                JamesAnmSource::HotelStage,
            ),
            (
                JamesStageClips::MARY_TENTACLE_CHOKE_RELEASE,
                "Mary Boss Tentacle Breakout Release [aid 20055] (287..=301 @ 15fps)",
                JamesAnmSource::HotelStage,
            ),
            (
                JamesStageClips::MARY_TENTACLE_CHOKE_DEATH,
                "Mary Boss Tentacle Choke Death Collapse [aid 20057] (242..=271 @ 15fps)",
                JamesAnmSource::HotelStage,
            ),
        ],
        default_anchor_offset: Vec3::new(0.0, -1496.4872, 0.0),
        default_james_y_offset: 0.0,
        default_enemy_y_offset: -1496.4872,
        enemy_yaw: std::f32::consts::PI,
        james_yaw: 0.0,
        camera_distance: 2500.0,
        camera_target_y: -950.0,
        notes: "PS2_HUGGING_POS[4] = [0.0, 1496.49, 0.0] (0x0021fbd4). Tentacle coil (+670mm local -> -826.5mm world) wraps James's throat (-824.9mm).",
    },
    // --- 3: Pyramid Head (red) ---
    GrapplePairEntry {
        creature_id: 4,
        name: "Pyramid Head (One-Handed Choke Lift)",
        description: "Pyramid Head lifts James clean off the floor with one hand around his neck",
        enemy_clip: AnimationClip::new(271, 294, 30.0, true),
        enemy_clip_name: "One-Handed Choke Lift [aid 5308] (271..=294 @ 30fps)",
        james_clip: JamesStageClips::PYRAMID_HEAD_STAIR_CHOKE_LOOP,
        james_clip_name: "Pyramid Head One-Handed Choke Lift Loop [aid 20030] (455..=478 @ 30fps)",
        james_anm_source: JamesAnmSource::ApartStair,
        alt_enemy_clips: &[
            (
                AnimationClip::new(256, 270, 15.0, false),
                "Initial One-Handed Neck Grab [aid 5307] (256..=270 @ 15fps)",
            ),
            (
                AnimationClip::new(295, 305, 30.0, false),
                "Choke Release Drop [aid 5309] (295..=305 @ 30fps)",
            ),
        ],
        alt_james_clips: &[
            (
                JamesStageClips::PYRAMID_HEAD_STAIR_CHOKE_GRAB,
                "Pyramid Head Initial Neck Grab Hit [aid 20029] (440..=454 @ 15fps)",
                JamesAnmSource::ApartStair,
            ),
            (
                JamesStageClips::PYRAMID_HEAD_STAIR_CHOKE_RELEASE,
                "Pyramid Head Choke Release Drop [aid 20031] (479..=508 @ 15fps)",
                JamesAnmSource::ApartStair,
            ),
            (
                JamesStageClips::PYRAMID_HEAD_STAIR_CHOKE_DEATH,
                "Pyramid Head Choke Death Drop [aid 20032] (509..=538 @ 15fps)",
                JamesAnmSource::ApartStair,
            ),
        ],
        default_anchor_offset: Vec3::new(43.5385, -93.4794, 353.9123),
        default_james_y_offset: -93.4794,
        default_enemy_y_offset: 0.0,
        enemy_yaw: std::f32::consts::PI,
        james_yaw: 0.0,
        camera_distance: 2400.0,
        camera_target_y: -850.0,
        notes: "PS2_HUGGING_POS[1] = [43.54, -93.48, 353.91] (0x0021fa84). Exact PS2 elevation (-93.48mm) and distance (+353.91mm) with zero neck clipping.",
    },
    // --- 4: Abstract Daddy (pap) ---
    GrapplePairEntry {
        creature_id: 5,
        name: "Abstract Daddy (Standing Bed Torso Pin)",
        description: "Upright frame pins James against its torso, attempting to suffocate him",
        enemy_clip: AnimationClip::new(70, 93, 30.0, true),
        enemy_clip_name: "Standing Bed Smother Hold [aid 5704] (70..=93 @ 30fps)",
        james_clip: JamesStageClips::ABSTRACT_DADDY_STRUGGLE_LIFT,
        james_clip_name: "Abstract Daddy Front Smother Hold Loop [aid 20043] (313..=336 @ 30fps)",
        james_anm_source: JamesAnmSource::LabyrinthE,
        alt_enemy_clips: &[
            (
                AnimationClip::new(50, 69, 30.0, false),
                "Initial Bed Embrace Grab [aid 5703] (50..=69 @ 30fps)",
            ),
            (
                AnimationClip::new(94, 113, 30.0, false),
                "Breakout Push Back [aid 5705] (94..=113 @ 30fps)",
            ),
        ],
        alt_james_clips: &[
            (
                JamesStageClips::ABSTRACT_DADDY_FRONT_GRAB,
                "Abstract Daddy Front Grab Hit [aid 20042] (298..=312 @ 15fps)",
                JamesAnmSource::LabyrinthE,
            ),
            (
                JamesStageClips::ABSTRACT_DADDY_FRONT_RELEASE,
                "Abstract Daddy Front Breakout Release [aid 20045] (337..=351 @ 15fps)",
                JamesAnmSource::LabyrinthE,
            ),
            (
                JamesStageClips::ABSTRACT_DADDY_FRONT_DEATH,
                "Abstract Daddy Front Smother Death [aid 20044] (352..=381 @ 15fps)",
                JamesAnmSource::LabyrinthE,
            ),
            (
                JamesStageClips::ABSTRACT_DADDY_BACK_LOOP,
                "Abstract Daddy Rear Smother Hold Loop [aid 20047] (397..=420 @ 30fps)",
                JamesAnmSource::LabyrinthE,
            ),
        ],
        default_anchor_offset: Vec3::new(0.0, 0.0, 328.0524),
        default_james_y_offset: 0.0,
        default_enemy_y_offset: 0.0,
        enemy_yaw: std::f32::consts::PI,
        james_yaw: 0.0,
        camera_distance: 2500.0,
        camera_target_y: -800.0,
        notes: "PS2 gz_lre overlay (jms_stg_labyrinth_e.anm 313..=336, aid=20043) & PS2_HUGGING_POS[2] = [0.0, 0.0, 328.05].",
    },
    // --- 5: Lying Figure (scu) ---
    GrapplePairEntry {
        creature_id: 1,
        name: "Lying Figure (Standing Acid Vomit Pin)",
        description: "Straightjacket creature grapples James upright, preparing to vomit acid point-blank",
        enemy_clip: AnimationClip::new(66, 88, 30.0, true),
        enemy_clip_name: "Standing Pin Hold (66..=88 @ 30fps)",
        james_clip: JamesBasicClips::STRUGGLE_TORSO,
        james_clip_name: "Torso Pin Struggle [aid 128] (612..=629 @ 15fps)",
        james_anm_source: JamesAnmSource::Basic,
        alt_enemy_clips: &[(
            AnimationClip::new(89, 104, 30.0, false),
            "Breakout Recoil (89..=104 @ 30fps)",
        )],
        alt_james_clips: &[
            (
                JamesStageClips::ABSTRACT_DADDY_STRUGGLE_LIFT,
                "Labyrinth East Smother Loop [aid 20043] (313..=336 @ 30fps)",
                JamesAnmSource::LabyrinthE,
            ),
            (
                JamesBasicClips::STRUGGLE_RELEASE,
                "Breakout Shove [aid 130] (652..=670 @ 30fps)",
                JamesAnmSource::Basic,
            ),
        ],
        default_anchor_offset: Vec3::new(0.0, 0.0, 480.0),
        default_james_y_offset: 0.0,
        default_enemy_y_offset: 0.0,
        enemy_yaw: std::f32::consts::PI,
        james_yaw: 0.0,
        camera_distance: 2200.0,
        camera_target_y: -800.0,
        notes: "Standard torso hold; mashing triggers breakout push before acid discharge.",
    },
    // --- 6: Creeper (tyu) ---
    GrapplePairEntry {
        creature_id: 8,
        name: "Creeper (Floor Leg Latch)",
        description: "Giant cockroach latches onto James's lower leg/foot from the floor",
        enemy_clip: AnimationClip::new(0, 15, 30.0, true),
        enemy_clip_name: "Floor Latch Scuttle (0..=15 @ 30fps)",
        james_clip: JamesBasicClips::STRUGGLE_LEG,
        james_clip_name: "Leg Stomp & Kick [aid 127] (588..=611 @ 30fps)",
        james_anm_source: JamesAnmSource::Basic,
        alt_enemy_clips: &[],
        alt_james_clips: &[(
            JamesBasicClips::KICK_DOWNED,
            "Downed Kick [aid 132] (682..=701 @ 11.25fps)",
            JamesAnmSource::Basic,
        )],
        default_anchor_offset: Vec3::new(0.0, 0.0, 280.0),
        default_james_y_offset: 0.0,
        default_enemy_y_offset: 0.0,
        enemy_yaw: 0.0,
        james_yaw: 0.0,
        camera_distance: 1800.0,
        camera_target_y: -400.0,
        notes: "Leg grapple. James kicks and stomps at the floor to dislodge the insect.",
    },
];

/// Comprehensive master catalog of all inspectable James Sunderland animation clips across basic and stage files.
pub static JAMES_MASTER_CLIP_CATALOG: &[(&str, AnimationClip, JamesAnmSource)] = &[
    // --- jms_basic1.anm struggles & combat reactions ---
    (
        "Hoisted Aloft Choke [aid 121] (457..=468 @ 15fps)",
        AnimationClip::new(457, 468, 15.0, true),
        JamesAnmSource::Basic,
    ),
    (
        "Throat Choke Struggle [aid 129] (632..=649 @ 15fps)",
        AnimationClip::new(632, 649, 15.0, true),
        JamesAnmSource::Basic,
    ),
    (
        "Torso Pin Struggle [aid 128] (612..=629 @ 15fps)",
        AnimationClip::new(612, 629, 15.0, true),
        JamesAnmSource::Basic,
    ),
    (
        "Leg Stomp & Creeper Struggle [aid 127] (588..=611 @ 30fps)",
        AnimationClip::new(588, 611, 30.0, true),
        JamesAnmSource::Basic,
    ),
    (
        "Breakout Shove / Kick Free [aid 130] (652..=670 @ 30fps)",
        AnimationClip::new(652, 670, 30.0, false),
        JamesAnmSource::Basic,
    ),
    (
        "Hurt / Damage Flinch [aid 122] (453..=470 @ 15fps)",
        AnimationClip::new(453, 470, 15.0, false),
        JamesAnmSource::Basic,
    ),
    (
        "Heavy Hurt / Knockback Recoil [aid 123] (471..=498 @ 15fps)",
        AnimationClip::new(471, 498, 15.0, false),
        JamesAnmSource::Basic,
    ),
    (
        "Ground Knockdown Stumble [aid 124] (499..=520 @ 15fps)",
        AnimationClip::new(499, 520, 15.0, false),
        JamesAnmSource::Basic,
    ),
    (
        "Collapse / Death Fall [aid 131] (256..=280 @ 15fps)",
        AnimationClip::new(256, 280, 15.0, false),
        JamesAnmSource::Basic,
    ),
    (
        "Downed Enemy Kick [aid 132] (682..=701 @ 11.25fps)",
        AnimationClip::new(682, 701, 11.25, false),
        JamesAnmSource::Basic,
    ),
    (
        "Downed Enemy Stomp [aid 133] (702..=719 @ 7.5fps)",
        AnimationClip::new(702, 719, 7.5, false),
        JamesAnmSource::Basic,
    ),
    (
        "Standing Idle Breathing [aid 01] (1..=10 @ 15fps)",
        AnimationClip::new(1, 10, 15.0, true),
        JamesAnmSource::Basic,
    ),
    (
        "Walk Forward [aid 02] (51..=74 @ 30fps)",
        AnimationClip::new(51, 74, 30.0, true),
        JamesAnmSource::Basic,
    ),
    (
        "Run Sprint [aid 03] (99..=146 @ 30fps)",
        AnimationClip::new(99, 146, 30.0, true),
        JamesAnmSource::Basic,
    ),
    (
        "Weapon Aim Readiness [aid 04] (200..=230 @ 30fps)",
        AnimationClip::new(200, 230, 30.0, true),
        JamesAnmSource::Basic,
    ),
    (
        "Weapon Strike Attack [aid 05] (231..=270 @ 30fps)",
        AnimationClip::new(231, 270, 30.0, false),
        JamesAnmSource::Basic,
    ),
    // --- jms_stg_hospital_1fw_b.anm (Hospital 1FW Boss Stage - Flesh Lips) ---
    (
        "Flesh Lips Ceiling Choke Hold Loop [aid 20039] (245..=268 @ 30fps)",
        AnimationClip::new(245, 268, 30.0, true),
        JamesAnmSource::HospitalStage,
    ),
    (
        "Flesh Lips Initial Choke Grab Hit [aid 20038] (230..=244 @ 15fps)",
        AnimationClip::new(230, 244, 15.0, false),
        JamesAnmSource::HospitalStage,
    ),
    (
        "Flesh Lips Choke Release Drop [aid 20040] (269..=298 @ 15fps)",
        AnimationClip::new(269, 298, 15.0, false),
        JamesAnmSource::HospitalStage,
    ),
    (
        "Flesh Lips Choke Death Collapse [aid 20041] (299..=328 @ 15fps)",
        AnimationClip::new(299, 328, 15.0, false),
        JamesAnmSource::HospitalStage,
    ),
    // --- jms_stg_end_recovery.anm (Final Boss Stage - Mary Boss) ---
    (
        "Mary Boss Phase 1: Tentacle Catch & Lift [aid 20056] (272..=286 @ 15fps)",
        AnimationClip::new(272, 286, 15.0, false),
        JamesAnmSource::HotelStage,
    ),
    (
        "Mary Boss Phase 2: Suspended Tentacle Choke Loop [aid 20054] (218..=241 @ 30fps)",
        AnimationClip::new(218, 241, 30.0, true),
        JamesAnmSource::HotelStage,
    ),
    (
        "Mary Boss Tentacle Breakout Release [aid 20055] (287..=301 @ 15fps)",
        AnimationClip::new(287, 301, 15.0, false),
        JamesAnmSource::HotelStage,
    ),
    (
        "Mary Boss Tentacle Choke Death Collapse [aid 20057] (242..=271 @ 15fps)",
        AnimationClip::new(242, 271, 15.0, false),
        JamesAnmSource::HotelStage,
    ),
];

/// Interactive state controller for the Grapple Animation Room.
#[derive(Debug, Clone)]
pub struct GrappleViewerState {
    /// Index into [`GRAPPLE_CATALOG`].
    pub selected_index: usize,
    /// Orbit / flight camera centered on the grapple contact point.
    pub camera: OrbitCamera,
    /// Whether animation playback is paused for frame-by-frame scrubbing.
    pub is_paused: bool,
    /// Playback speed multiplier (default 1.0, e.g. 0.25x, 0.5x, 1.0x, 2.0x).
    pub playback_speed: f32,
    /// Continuous playhead time for creature animation (in seconds).
    pub enemy_time: f32,
    /// Continuous playhead time for James animation (in seconds).
    pub james_time: f32,
    /// Interactive vertical elevation adjustment for James (in mm, where $-Y$ is up).
    pub james_y_offset: f32,
    /// Interactive vertical elevation adjustment for Creature (in mm).
    pub enemy_y_offset: f32,
    /// Interactive horizontal $Z$ distance offset between characters (in mm).
    pub relative_z_offset: f32,
    /// Active alternative creature clip index (0 = default).
    pub alt_enemy_clip_idx: usize,
    /// Active alternative James clip index (0 = default).
    pub alt_james_clip_idx: usize,
    /// Active ANM source file for James Sunderland.
    pub active_anm_source: JamesAnmSource,
    /// Active clip index within the active ANM source file.
    pub active_clip_idx: usize,
    /// Index into [`JAMES_MASTER_CLIP_CATALOG`] for full browser cycling.
    pub james_catalog_idx: usize,
    /// Whether free timeline scrubbing mode is currently active.
    pub is_free_scrubbing: bool,
    /// Start frame for free timeline scrubbing.
    pub scrub_start_frame: u32,
    /// Window duration for free timeline scrubbing (in frames).
    pub scrub_window_len: u32,
    /// Manual ANM source override (None = use clip's source).
    pub anm_source_override: Option<JamesAnmSource>,
    /// Cached clip list for the active ANM source to avoid per-frame heap allocations.
    cached_source_clips: Vec<(String, AnimationClip)>,
    /// Whether wireframe mesh overlay is displayed.
    pub wireframe_overlay: bool,
    /// Whether bone skeleton overlay is displayed.
    pub skeleton_overlay: bool,
}

impl Default for GrappleViewerState {
    fn default() -> Self {
        Self::new()
    }
}

impl GrappleViewerState {
    /// Initialize with the first catalog entry (Flesh Lips overhead choke).
    pub fn new() -> Self {
        let initial_src = GRAPPLE_CATALOG[0].james_anm_source;
        let mut state = Self {
            selected_index: 0,
            camera: OrbitCamera::default(),
            is_paused: false,
            playback_speed: 1.0,
            enemy_time: 0.0,
            james_time: 0.0,
            james_y_offset: 0.0,
            enemy_y_offset: 0.0,
            relative_z_offset: 0.0,
            alt_enemy_clip_idx: 0,
            alt_james_clip_idx: 0,
            active_anm_source: initial_src,
            active_clip_idx: 0,
            james_catalog_idx: 0,
            is_free_scrubbing: false,
            scrub_start_frame: 0,
            scrub_window_len: 25,
            anm_source_override: None,
            cached_source_clips: generate_clips_for_source(initial_src),
            wireframe_overlay: false,
            skeleton_overlay: false,
        };
        state.reset_for_current_entry();
        state
    }

    /// Reset camera framing and elevation offsets when selecting a new encounter.
    pub fn reset_for_current_entry(&mut self) {
        let entry = self.current_entry();
        self.camera.yaw = 0.0;
        self.camera.pitch = 0.12;
        self.camera.distance = entry.camera_distance;
        self.camera.target = Vec3::new(0.0, entry.camera_target_y, 0.0);
        self.james_y_offset = entry.default_james_y_offset;
        self.enemy_y_offset = entry.default_enemy_y_offset;
        self.relative_z_offset = 0.0;
        self.enemy_time = 0.0;
        self.james_time = 0.0;
        self.alt_enemy_clip_idx = 0;
        self.alt_james_clip_idx = 0;
        self.is_free_scrubbing = false;
        self.scrub_start_frame = 0;
        self.anm_source_override = None;

        // Auto-select the entry's declared authentic James ANM source and matching clip
        self.active_anm_source = entry.james_anm_source;
        self.cached_source_clips = generate_clips_for_source(self.active_anm_source);
        self.active_clip_idx = self
            .cached_source_clips
            .iter()
            .position(|(_, c)| {
                c.start_frame == entry.james_clip.start_frame
                    && c.end_frame == entry.james_clip.end_frame
            })
            .unwrap_or(0);
        self.james_catalog_idx = self.active_clip_idx;
    }

    /// Current grapple entry descriptor.
    pub fn current_entry(&self) -> &'static GrapplePairEntry {
        &GRAPPLE_CATALOG[self.selected_index % GRAPPLE_CATALOG.len()]
    }

    /// Active creature animation clip (taking alternative selection into account).
    pub fn active_enemy_clip(&self) -> AnimationClip {
        let entry = self.current_entry();
        if self.alt_enemy_clip_idx == 0 || entry.alt_enemy_clips.is_empty() {
            entry.enemy_clip
        } else {
            entry.alt_enemy_clips[(self.alt_enemy_clip_idx - 1) % entry.alt_enemy_clips.len()].0
        }
    }

    /// Active creature clip label.
    pub fn active_enemy_clip_name(&self) -> &'static str {
        let entry = self.current_entry();
        if self.alt_enemy_clip_idx == 0 || entry.alt_enemy_clips.is_empty() {
            entry.enemy_clip_name
        } else {
            entry.alt_enemy_clips[(self.alt_enemy_clip_idx - 1) % entry.alt_enemy_clips.len()].1
        }
    }

    /// Active James animation clip (taking active ANM file clips or free scrub window into account).
    pub fn active_james_clip(&self) -> AnimationClip {
        if self.is_free_scrubbing {
            AnimationClip::new(
                self.scrub_start_frame,
                self.scrub_start_frame + self.scrub_window_len.max(1) - 1,
                15.0,
                true,
            )
        } else {
            let clips = self.active_source_clips();
            if clips.is_empty() {
                AnimationClip::new(0, 20, 15.0, true)
            } else {
                clips[self.active_clip_idx % clips.len()].1
            }
        }
    }

    /// Active James clip label.
    pub fn active_james_clip_name(&self) -> String {
        if self.is_free_scrubbing {
            format!(
                "[Free Scrub] Frames {}..={} (Win: {})",
                self.scrub_start_frame,
                self.scrub_start_frame + self.scrub_window_len.max(1) - 1,
                self.scrub_window_len
            )
        } else {
            let clips = self.active_source_clips();
            if clips.is_empty() {
                "No clips".to_string()
            } else {
                let idx = self.active_clip_idx % clips.len();
                let (ref name, clip) = clips[idx];
                format!(
                    "[{}/{}] {} ({}..={})",
                    idx + 1,
                    clips.len(),
                    name,
                    clip.start_frame,
                    clip.end_frame
                )
            }
        }
    }

    /// All animation clips available in the currently selected James ANM file (zero-allocation cached slice).
    pub fn active_source_clips(&self) -> &[(String, AnimationClip)] {
        &self.cached_source_clips
    }

    /// Active James ANM source (basic vs hospital vs hotel vs other stage files).
    pub fn active_james_anm_source(&self) -> JamesAnmSource {
        if let Some(src) = self.anm_source_override {
            src
        } else {
            self.active_anm_source
        }
    }

    /// Cycle to next creature grapple encounter.
    pub fn next_entry(&mut self) {
        self.selected_index = (self.selected_index + 1) % GRAPPLE_CATALOG.len();
        self.reset_for_current_entry();
    }

    /// Cycle to previous creature grapple encounter.
    pub fn prev_entry(&mut self) {
        if self.selected_index == 0 {
            self.selected_index = GRAPPLE_CATALOG.len() - 1;
        } else {
            self.selected_index -= 1;
        }
        self.reset_for_current_entry();
    }

    /// Cycle creature clip variant.
    pub fn cycle_enemy_clip(&mut self) {
        let entry = self.current_entry();
        let total_variants = 1 + entry.alt_enemy_clips.len();
        self.alt_enemy_clip_idx = (self.alt_enemy_clip_idx + 1) % total_variants;
        self.enemy_time = 0.0;
    }

    /// Cycle James clip forward in the active ANM file.
    pub fn next_james_clip(&mut self) {
        let count = self.active_source_clips().len();
        if count > 0 {
            self.active_clip_idx = (self.active_clip_idx + 1) % count;
        }
        self.james_catalog_idx = self.active_clip_idx;
        self.james_time = 0.0;
        if self.is_free_scrubbing {
            self.scrub_start_frame = self.active_james_clip().start_frame;
        }
    }

    /// Cycle James clip backward in the active ANM file.
    pub fn prev_james_clip(&mut self) {
        let count = self.active_source_clips().len();
        if count > 0 {
            if self.active_clip_idx == 0 {
                self.active_clip_idx = count - 1;
            } else {
                self.active_clip_idx -= 1;
            }
        }
        self.james_catalog_idx = self.active_clip_idx;
        self.james_time = 0.0;
        if self.is_free_scrubbing {
            self.scrub_start_frame = self.active_james_clip().start_frame;
        }
    }

    /// Cycle James clip variant (calls next_james_clip).
    pub fn cycle_james_clip(&mut self) {
        self.next_james_clip();
    }

    /// Cycle active ANM source through all 15 major files.
    pub fn cycle_anm_source(&mut self, forward: bool) {
        let all = JamesAnmSource::all();
        let cur = all
            .iter()
            .position(|&s| s == self.active_james_anm_source())
            .unwrap_or(0);
        let next = if forward {
            (cur + 1) % all.len()
        } else if cur == 0 {
            all.len() - 1
        } else {
            cur - 1
        };
        self.anm_source_override = Some(all[next]);
        self.active_anm_source = all[next];
        self.cached_source_clips = generate_clips_for_source(all[next]);
        self.active_clip_idx = 0;
        self.james_catalog_idx = 0;
        self.james_time = 0.0;
        if self.is_free_scrubbing {
            self.scrub_start_frame = 0;
        }
    }

    /// Toggle free timeline scrubbing mode.
    pub fn toggle_scrub_mode(&mut self) {
        self.is_free_scrubbing = !self.is_free_scrubbing;
        if self.is_free_scrubbing {
            self.scrub_start_frame = self.active_james_clip().start_frame;
        }
        self.james_time = 0.0;
    }

    /// Shift free scrubbing window by `frames`.
    pub fn scrub_shift(&mut self, frames: i32) {
        if frames < 0 {
            self.scrub_start_frame = self.scrub_start_frame.saturating_sub((-frames) as u32);
        } else {
            self.scrub_start_frame = (self.scrub_start_frame + frames as u32).min(3000);
        }
        self.james_time = 0.0;
    }

    /// Jump free scrub window to frame 0.
    pub fn scrub_jump_start(&mut self) {
        self.scrub_start_frame = 0;
        self.james_time = 0.0;
    }

    /// Jump free scrub window towards end of file.
    pub fn scrub_jump_end(&mut self, max_frame: u32) {
        self.scrub_start_frame = max_frame.saturating_sub(self.scrub_window_len);
        self.james_time = 0.0;
    }

    /// Adjust free scrub window length.
    pub fn scrub_adjust_window(&mut self, delta: i32) {
        if delta < 0 {
            self.scrub_window_len = self.scrub_window_len.saturating_sub((-delta) as u32).max(5);
        } else {
            self.scrub_window_len = (self.scrub_window_len + delta as u32).min(200);
        }
        self.james_time = 0.0;
    }

    /// Toggle play / pause state.
    pub fn toggle_pause(&mut self) {
        self.is_paused = !self.is_paused;
    }

    /// Step playback by one frame forward or backward.
    pub fn step_frame(&mut self, forward: bool) {
        self.is_paused = true;
        let e_clip = self.active_enemy_clip();
        let j_clip = self.active_james_clip();
        let e_dt = 1.0 / e_clip.fps.max(1.0);
        let j_dt = 1.0 / j_clip.fps.max(1.0);

        if forward {
            self.enemy_time += e_dt;
            self.james_time += j_dt;
        } else {
            self.enemy_time = (self.enemy_time - e_dt).max(0.0);
            self.james_time = (self.james_time - j_dt).max(0.0);
        }
    }

    /// Adjust playback speed multiplier.
    pub fn adjust_speed(&mut self, increase: bool) {
        let speeds = [0.1, 0.25, 0.5, 1.0, 1.5, 2.0];
        let cur_idx = speeds
            .iter()
            .position(|&s| (s - self.playback_speed).abs() < 0.05)
            .unwrap_or(3);
        if increase && cur_idx + 1 < speeds.len() {
            self.playback_speed = speeds[cur_idx + 1];
        } else if !increase && cur_idx > 0 {
            self.playback_speed = speeds[cur_idx - 1];
        }
    }

    /// Interactively adjust James's vertical elevation (in mm).
    pub fn adjust_james_elevation(&mut self, delta_y: f32) {
        self.james_y_offset += delta_y;
    }

    /// Interactively adjust horizontal distance between characters (in mm).
    pub fn adjust_relative_distance(&mut self, delta_z: f32) {
        self.relative_z_offset += delta_z;
    }

    /// Advance playheads by `dt_sec`.
    pub fn tick(&mut self, dt_sec: f32) {
        if !self.is_paused {
            let eff_dt = dt_sec * self.playback_speed;
            self.enemy_time += eff_dt;
            self.james_time += eff_dt;
        }
    }

    /// Sample interpolated frame cursor for creature animation.
    pub fn sample_enemy_cursor(&self, total_frames: usize) -> (usize, usize, f32) {
        let clip = self.active_enemy_clip();
        self.sample_clip_cursor(&clip, self.enemy_time, total_frames)
    }

    /// Sample interpolated frame cursor for James animation.
    pub fn sample_james_cursor(&self, total_frames: usize) -> (usize, usize, f32) {
        let clip = self.active_james_clip();
        self.sample_clip_cursor(&clip, self.james_time, total_frames)
    }

    fn sample_clip_cursor(
        &self,
        clip: &AnimationClip,
        time: f32,
        total_frames: usize,
    ) -> (usize, usize, f32) {
        if total_frames == 0 {
            return (0, 0, 0.0);
        }
        let frame_span = clip.frame_count() as f32;
        if frame_span <= 1.0 {
            let f = (clip.start_frame as usize) % total_frames;
            return (f, f, 0.0);
        }

        let raw_frame_offset = time * clip.fps;
        let local_frame = if clip.looping {
            raw_frame_offset % frame_span
        } else {
            raw_frame_offset.min(frame_span - 1.0)
        };

        let abs_frame = clip.start_frame as f32 + local_frame;
        let f0 = (abs_frame.floor() as usize) % total_frames;
        let f1 = if clip.looping {
            (clip.start_frame as usize
                + (((abs_frame.floor() as u32).saturating_sub(clip.start_frame) + 1)
                    % (frame_span as u32)) as usize)
                % total_frames
        } else {
            ((abs_frame.floor() as usize + 1).min(clip.end_frame as usize)) % total_frames
        };
        let alpha = abs_frame.fract();

        (f0, f1, alpha)
    }

    /// Current fractional frame index of the creature animation.
    pub fn enemy_current_frame(&self) -> f32 {
        let clip = self.active_enemy_clip();
        let frame_span = clip.frame_count() as f32;
        if frame_span <= 1.0 {
            clip.start_frame as f32
        } else {
            let raw_offset = self.enemy_time * clip.fps;
            let local_frame = if clip.looping {
                raw_offset % frame_span
            } else {
                raw_offset.min(frame_span - 1.0)
            };
            clip.start_frame as f32 + local_frame
        }
    }

    /// Current fractional frame index of James's animation.
    pub fn james_current_frame(&self) -> f32 {
        let clip = self.active_james_clip();
        let frame_span = clip.frame_count() as f32;
        if frame_span <= 1.0 {
            clip.start_frame as f32
        } else {
            let raw_offset = self.james_time * clip.fps;
            let local_frame = if clip.looping {
                raw_offset % frame_span
            } else {
                raw_offset.min(frame_span - 1.0)
            };
            clip.start_frame as f32 + local_frame
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grapple_catalog_integrity() {
        assert!(!GRAPPLE_CATALOG.is_empty());
        for entry in GRAPPLE_CATALOG {
            assert!(!entry.name.is_empty());
            assert!(entry.enemy_clip.fps > 0.0);
            assert!(entry.james_clip.fps > 0.0);
            assert!(entry.enemy_clip.start_frame <= entry.enemy_clip.end_frame);
            assert!(entry.james_clip.start_frame <= entry.james_clip.end_frame);
        }
    }

    #[test]
    fn test_grapple_viewer_navigation_and_variants() {
        let mut viewer = GrappleViewerState::new();
        assert_eq!(viewer.selected_index, 0);
        assert_eq!(viewer.current_entry().creature_id, 6); // Flesh Lips

        viewer.next_entry();
        assert_eq!(viewer.selected_index, 1);
        assert_eq!(viewer.current_entry().creature_id, 9); // Mary Boss Phase 1

        viewer.cycle_enemy_clip();
        assert_ne!(viewer.alt_enemy_clip_idx, 0);

        let orig_idx = viewer.james_catalog_idx;
        viewer.cycle_james_clip();
        assert_ne!(viewer.james_catalog_idx, orig_idx);

        viewer.prev_entry();
        assert_eq!(viewer.selected_index, 0);
        assert_eq!(viewer.alt_enemy_clip_idx, 0);
    }

    #[test]
    fn test_grapple_viewer_playback_and_elevation() {
        let mut viewer = GrappleViewerState::new();
        let initial_y = viewer.james_y_offset;

        viewer.adjust_james_elevation(-50.0);
        assert_eq!(viewer.james_y_offset, initial_y - 50.0);

        viewer.toggle_pause();
        assert!(viewer.is_paused);

        viewer.step_frame(true);
        assert!(viewer.james_time > 0.0);
        assert!(viewer.enemy_time > 0.0);

        viewer.adjust_speed(true);
        assert!(viewer.playback_speed > 1.0);

        viewer.adjust_speed(false);
        assert_eq!(viewer.playback_speed, 1.0);

        viewer.reset_for_current_entry();
        assert_eq!(viewer.james_y_offset, initial_y);
    }

    #[test]
    fn test_stage_anm_source_selection_and_ranges() {
        let mut viewer = GrappleViewerState::new();
        // Encounter 0: Flesh Lips (Hospital 1FW Boss stage overlay: jms_stg_hospital_1fw_b.anm)
        assert_eq!(viewer.current_entry().creature_id, 6);
        assert_eq!(
            viewer.active_james_anm_source(),
            JamesAnmSource::HospitalStage
        );
        assert_eq!(viewer.active_james_clip().start_frame, 245);
        assert_eq!(viewer.active_james_clip().end_frame, 268);

        // Cycle through James clips within HospitalStage ANM file:
        // Next clip in HospitalStage is Initial Choke Grab Hit (230..=244)
        viewer.next_james_clip();
        assert_eq!(
            viewer.active_james_anm_source(),
            JamesAnmSource::HospitalStage
        );
        assert_eq!(viewer.active_james_clip().start_frame, 230);
        assert_eq!(viewer.active_james_clip().end_frame, 244);

        // Test cycle ANM source forward (next in all() after HospitalStage is HotelStage, then Basic)
        viewer.cycle_anm_source(true);
        assert_eq!(viewer.active_james_anm_source(), JamesAnmSource::HotelStage);
        viewer.cycle_anm_source(true);
        assert_eq!(viewer.active_james_anm_source(), JamesAnmSource::Basic);

        // Test free timeline scrub mode
        viewer.toggle_scrub_mode();
        assert!(viewer.is_free_scrubbing);
        let orig_f = viewer.scrub_start_frame;
        viewer.scrub_shift(15);
        assert_eq!(viewer.scrub_start_frame, orig_f + 15);
        viewer.scrub_adjust_window(10);
        assert_eq!(viewer.scrub_window_len, 35);
        viewer.toggle_scrub_mode();
        assert!(!viewer.is_free_scrubbing);

        // Encounter 1: Mary Final Boss (Phase 1: 272..=286 in jms_stg_end_recovery.anm)
        viewer.next_entry();
        assert_eq!(viewer.current_entry().creature_id, 9);
        assert_eq!(viewer.active_james_anm_source(), JamesAnmSource::HotelStage);
        assert_eq!(viewer.active_james_clip().start_frame, 272);
        assert_eq!(viewer.active_james_clip().end_frame, 286);

        // Encounter 2: Mary Final Boss (Phase 2: 218..=241 in jms_stg_end_recovery.anm)
        viewer.next_entry();
        assert_eq!(viewer.current_entry().creature_id, 9);
        assert_eq!(viewer.active_james_anm_source(), JamesAnmSource::HotelStage);
        assert_eq!(viewer.active_james_clip().start_frame, 218);
        assert_eq!(viewer.active_james_clip().end_frame, 241);

        // Encounter 3: Pyramid Head (Apartments Staircase ANM: 455..=478)
        viewer.next_entry();
        assert_eq!(viewer.current_entry().creature_id, 4);
        assert_eq!(viewer.active_james_anm_source(), JamesAnmSource::ApartStair);
        assert_eq!(viewer.active_james_clip().start_frame, 455);
        assert_eq!(viewer.active_james_clip().end_frame, 478);

        // Encounter 4: Abstract Daddy (Labyrinth East ANM: 313..=336)
        viewer.next_entry();
        assert_eq!(viewer.current_entry().creature_id, 5);
        assert_eq!(viewer.active_james_anm_source(), JamesAnmSource::LabyrinthE);
        assert_eq!(viewer.active_james_clip().start_frame, 313);
        assert_eq!(viewer.active_james_clip().end_frame, 336);
    }

    #[test]
    fn test_grapple_bone_alignment() {
        use sh2_assets::formats::anm::{compute_bind_locals, evaluate_pose, parse_anm};
        use sh2_assets::formats::mdls::parse_mdl;
        use std::path::Path;

        let root = "../../Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2";
        if !Path::new(root).exists() {
            return;
        }

        let load_pose = |mdl_rel: &str, anm_rel: &str, frame: usize| -> Vec<[[f32; 4]; 4]> {
            let mdl_bytes = std::fs::read(format!("{root}/{mdl_rel}")).unwrap();
            let model = parse_mdl(&mdl_bytes).unwrap();
            let bind_locals = compute_bind_locals(&model.parents, &model.initial_matrices);
            let anm_bytes = std::fs::read(format!("{root}/{anm_rel}")).unwrap();
            let anm = parse_anm(&anm_bytes, model.bone_count, &model.parents).unwrap();
            evaluate_pose(&model.parents, &bind_locals, &anm.frames[frame])
        };

        // Verify James neck/head in apart_stair f460 aligns with Flesh Lips feet (ike f95) at PS2_HUGGING_POS[0]
        let jms_stair = load_pose(
            "data/chr/jms/rlll_jms.mdl",
            "data/chr/jms/jms_stg_apart_stair.anm",
            460,
        );
        let jms_hosp = load_pose(
            "data/chr/jms/rlll_jms.mdl",
            "data/chr/jms/jms_stg_hospital_1fw_b.anm",
            250,
        );
        // Frame 460 of apart_stair and frame 250 of hospital_1fw_b are byte-identical (frame +5 of choke loop)
        assert!((jms_stair[8][1][3] - jms_hosp[8][1][3]).abs() < 1e-3);

        let ike_pose = load_pose("data/chr/ike/ike.mdl", "data/chr/ike/ike.anm", 95);
        let ike_foot_world_y = -1550.9032 + ike_pose[13][1][3];
        let jms_chin_y = jms_hosp[15][1][3];
        assert!(
            (ike_foot_world_y - jms_chin_y).abs() < 25.0,
            "Flesh Lips foot world Y ({}) must align with James neck/chin Y ({})",
            ike_foot_world_y,
            jms_chin_y
        );

        // Verify James root lift and Mary's overhead tentacle coil in Mary Boss choke loop (end_recovery f225 + bos f95)
        let jms_mary = load_pose(
            "data/chr/jms/rlll_jms.mdl",
            "data/chr/jms/jms_stg_end_recovery.anm",
            225,
        );
        assert!(
            (jms_mary[0][1][3] - (-602.3)).abs() < 2.0,
            "Mary Boss choke loop lifts James root to -602.3mm, got {}",
            jms_mary[0][1][3]
        );
        let bos_pose = load_pose("data/chr/bos/bos.mdl", "data/chr/bos/bos.anm", 95);
        let bos_tentacle_world_y = -1496.4872 + bos_pose[40][1][3];
        let jms_mary_throat_y = 0.5 * (jms_mary[8][1][3] + jms_mary[15][1][3]);
        assert!(
            (bos_tentacle_world_y - jms_mary_throat_y).abs() < 15.0,
            "Mary tentacle coil world Y ({}) must align with James throat Y ({})",
            bos_tentacle_world_y,
            jms_mary_throat_y
        );

        // Verify Pyramid Head palm Z and Y alignment at PS2_HUGGING_POS[1] = [43.5385, -93.4794, 353.9123]
        let red_pose = load_pose("data/chr/red/red.mdl", "data/chr/red/red.anm", 280);
        let red_palm_world_z = 353.9123 - red_pose[30][2][3];
        assert!(
            red_palm_world_z > 10.0 && red_palm_world_z < 25.0,
            "Pyramid Head palm world Z ({}) must rest flush in front of James throat without clipping",
            red_palm_world_z
        );

        // Verify James forward torso/head lean in Abstract Daddy smother hold (labyrinth_e f320)
        let jms_pap = load_pose(
            "data/chr/jms/rlll_jms.mdl",
            "data/chr/jms/jms_stg_labyrinth_e.anm",
            320,
        );
        let pap_pose = load_pose("data/chr/pap/pap.mdl", "data/chr/pap/pap.anm", 80);
        let pap_arm_world_z = 328.0524 - pap_pose[27][2][3];
        let jms_head_z = jms_pap[8][2][3];
        assert!(
            (pap_arm_world_z - jms_head_z).abs() < 20.0,
            "Abstract Daddy enveloping arm Z ({}) must align with James head Z ({})",
            pap_arm_world_z,
            jms_head_z
        );
    }
}
