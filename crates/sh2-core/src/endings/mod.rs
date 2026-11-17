//! Silent Hill 2 Ending Evaluation Engine (`playing_info.c`), 10-Star Result Ranking (`player_result.c`),
//! Secret NG+ Weapons (`Chainsaw` & `Hyper Spray`), Special Endings (`Rebirth`, `Dog`, `UFO`),
//! Epilogue Letter Readings, and Per-Ending Staff Credits Roll (`SH2-118`).
//!
//! Decomposed into single-responsibility submodules verified 1:1 against `SLUS_202.28`, `sh2pc.exe`,
//! and Silent Hill 2: Enhanced Edition patches (`PreventChainsawSpawn.cpp`, `SprayEffect.cpp`,
//! `ChainsawSoundFix.cpp`, `LeaveEndingDrawDistance.cpp`).

#![forbid(unsafe_code)]

pub mod epilogue_credits;
pub mod playing_info;
pub mod point_evaluator;
pub mod ranking;
pub mod secret_weapons;
pub mod special_endings;
pub mod ufo_slideshow;

pub use epilogue_credits::{
    active_mary_letter_stanza, credits_descriptor_for_ending, leave_ending_cemetery_draw_distance,
    CreditsMusicTheme, CreditsPresentationKind, EndingCreditsDescriptor, MaryLetterStanza,
    StaffCreditsSection, ENDING_CREDITS_TABLE, LEAVE_CEMETERY_ENHANCED_DRAW_DISTANCE_MM,
    LEAVE_CEMETERY_VANILLA_DRAW_DISTANCE_MM, MARY_FULL_LETTER_STANZAS,
    MARY_FULL_LETTER_VOICE_AFS_INDEX, SE_IN_WATER_DROWNING_STREAM, SE_MARY_FULL_LETTER_VOICEOVER,
    TEAM_SILENT_STAFF_SECTIONS,
};
pub use playing_info::{
    ClearEndKind, GameEnding, MainEndingKind, PlayingInfo, FLAG_ANGELAS_KNIFE_EXAMINED,
    FLAG_ENDING_BORN_FROM_A_WISH, FLAG_ENDING_DOG, FLAG_ENDING_IN_WATER, FLAG_ENDING_LEAVE,
    FLAG_ENDING_MARIA, FLAG_ENDING_REBIRTH, FLAG_ENDING_UFO, FLAG_HOSPITAL_ROOF_DIARY,
    FLAG_HOSPITAL_S3_MARIA_SECOND_CHECK, FLAG_HOTEL_READING_ROOM_TAPE,
    FLAG_LABYRINTH_MARIA_REVISIT, FLAG_MARIA_REMINDED_ABOUT_MARY, FLAG_MARIA_WRONG_DIRECTION,
    FLAG_MARY_HALLWAY_MONOLOGUE_COMPLETE, FLAG_NEELYS_BAR_SECOND_MESSAGE, IN_WATER_START_POINTS,
    ITEM_ANGELAS_KNIFE, ITEM_LETTER_FROM_MARY, ITEM_PHOTO_OF_MARY, LEAVE_START_POINTS,
    LOW_HP_RATIO_THRESHOLD, LOW_HP_WEIGHTED_SECS_TIER_1, LOW_HP_WEIGHTED_SECS_TIER_2,
    MARIA_BUMP_COUNTER_CAP, MARIA_BUMP_HIGH_THRESHOLD, MARIA_BUMP_LOW_THRESHOLD,
    MARIA_CLOSE_DISTANCE_MM, MARIA_CLOSE_TIME_THRESHOLD_SECS, MARIA_ENEMY_DAMAGE_RATIO_LIMIT,
    MARIA_FAR_DISTANCE_MM, MARIA_FAR_TIME_THRESHOLD_SECS, MARIA_HP_SCALE_EASY,
    MARIA_HP_SCALE_NORMAL_HARD, MARIA_START_POINTS, MARY_CORRIDOR_COMPLETE_SUBTITLE_FRAMES,
    MARY_CORRIDOR_START_DELAY_SECS, MAX_VALID_MOVE_DELTA_MM, OVERHEAL_RATIO_THRESHOLD,
    PC_PLAYER_ATTACK_ID_LIMIT, PLAYING_INFO_SIZE_BYTES, ROOM_HOTEL_CORRIDOR_MONOLOGUE,
    SE_MARY_CORRIDOR_MONOLOGUE,
};
pub use point_evaluator::{
    EndingEvalFlags, EndingEvaluationReport, EndingPointTracker, MaryCorridorMonologueState,
};
pub use ranking::{
    calc_rank_battle_level, calc_rank_boat_time, calc_rank_clear_time, calc_rank_ending_kind,
    calc_rank_item_get, calc_rank_james_damage, calc_rank_kills, calc_rank_riddle_level,
    calc_rank_save_count, evaluate_star_ranking, resolve_spray_power, RankingRuleset,
    StarRankBreakdown, GREEN_SPRAY_MIN_RANK, MAX_RANK_POINTS, PC_PURPLE_SPRAY_MAX_RANK,
    PS2_PROTO_PURPLE_SPRAY_MAX_RANK, YELLOW_SPRAY_MIN_RANK,
};
pub use secret_weapons::{
    evaluate_ngplus_secret_spawns, evaluate_spray_enemy_reaction, try_pickup_chainsaw,
    HyperSprayCanState, HyperSprayColor, HyperSprayTickResult, NgPlusSecretSpawns,
    SprayEnemyReaction, SprayParticleTimingConfig, ATK_CHAINSAW_OVERHEAD, ATK_CHAINSAW_SWEEP,
    ATK_CHAINSAW_THRUST_1, ATK_HYPER_SPRAY_STAND, ATK_HYPER_SPRAY_WALK, FLAG_CHAINSAW_PICKED_UP,
    FLAG_HYPER_SPRAY_PICKED_UP, FLAG_SPAWN_BLUE_GEM, FLAG_SPAWN_CHAINSAW, FLAG_SPAWN_DOG_KEY,
    FLAG_SPAWN_HYPER_SPRAY, HYPER_SPRAY_BASE_MAX_RANGE_MM, HYPER_SPRAY_BASE_MIN_RANGE_MM,
    HYPER_SPRAY_CONE_HALF_ANGLE_RAD, HYPER_SPRAY_FULL_DURATION_SECS,
    HYPER_SPRAY_INITIAL_SFX_VOLUME, HYPER_SPRAY_RAY_STEP_MM, HYPER_SPRAY_SUSTAIN_SFX_VOLUME,
    HYPER_SPRAY_TAPER_THRESHOLD_SECS, ITEM_CHAINSAW, ITEM_HYPER_SPRAY, ROOM_FOREST_TRAIL_CHAINSAW,
    ROOM_SAUL_STREET_MOTORHOME, SE_CHAINSAW_PICKUP_CRANK, SE_HYPER_SPRAY_HISS,
};
pub use special_endings::{
    count_rebirth_ritual_items, try_trigger_dog_ending, use_blue_gem, BlueGemLocation,
    BlueGemUseResult, FLAG_BLUE_GEM_HOSPITAL_GARDEN, FLAG_BLUE_GEM_PICKED_UP,
    FLAG_BLUE_GEM_TOLUCA_DOCK, FLAG_DOG_KEY_PICKED_UP, FLAG_HOTEL_VIDEOTAPE_WATCHED,
    FLAG_OBSERVATION_ROOM_UNLOCKED, ITEM_BLUE_GEM, ITEM_BOOK_CRIMSON_CEREMONY,
    ITEM_BOOK_LOST_MEMORIES, ITEM_DOG_KEY, ITEM_OBSIDIAN_GOBLET, ITEM_WHITE_CHRISM,
    REBIRTH_RITUAL_ITEM_IDS, ROOM_DARK_HOSPITAL_GARDEN, ROOM_HOTEL_312,
    ROOM_HOTEL_3F_OBSERVATION_CORRIDOR, ROOM_RESTROOM_BLUE_GEM, ROOM_TOLUCA_LAKE_DOCK,
    ROOM_WEST_TOWN_DOGHOUSE, SE_DOG_KEY_UNLOCK, SE_UFO_ENDING_STREAM, SE_UFO_GEM_PULSE_STREAM,
};
pub use ufo_slideshow::{UfoSlideEntry, UfoSlideShowPlayer, UFO_SLIDE_TABLE, UFO_TEXTURE_PATHS};
