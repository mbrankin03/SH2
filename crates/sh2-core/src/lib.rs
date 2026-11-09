//! sh2-core: Core engine simulation, state machines, math, camera, and timing.

pub mod ai;
pub mod animation;
pub mod camera;
pub mod clock;
pub mod collision;
pub mod combat;
pub mod config;
pub mod cutaway;
pub mod cutscene;
pub mod events;
pub mod input;
pub mod inventory;
pub mod locomotion;
pub mod math;
pub mod scenario;
pub mod secondary_physics;
pub mod simulation;
pub mod state;
pub mod timing;
pub mod types;

// Re-exports for convenience
pub use animation::{
    AnimationClip, AnimationStateKind, BlendMode, CharacterAnimationController,
    CharacterLocomotionClips, JamesBasicClips, JamesStageClips, LocomotionAnimCommand,
    LocomotionAnimationSync, MariaLocomotionClips,
};
pub use camera::{CameraMovementType, CameraSnapshot, Ps2CameraController};
pub use clock::{EngineClock, FramePacer, DEFAULT_MAX_ACCUMULATED};
pub use collision::{FloorSample, SurfaceMaterial};
pub use combat::{
    AttackSpec, CombatArena, CombatController, CombatHitbox, CombatMode, CombatTargetEnemy,
    WeaponGrip, ATK_HANDGUN, ATK_HUNTING_RIFLE, ATK_KICKS, ATK_PIPE_COMBO, ATK_PIPE_LUNGE,
    ATK_PIPE_OVERHEAD, ATK_PIPE_SWING, ATK_PLANK_COMBO, ATK_PLANK_HEAVY, ATK_PLANK_LIGHT,
    ATK_SHOTGUN, ATK_STOMPS,
};
pub use config::{
    ActionDifficulty, AppConfig, AudioOptions, AudioPackDetectionStatus, AudioPackPreference,
    AudioPackSource, BulletMultiplier, ButtonGlyphStyle, ControlOptions, FogQuality, FrameRateCap,
    GameLanguage, GameOptions, InternalResolution, RiddleDifficulty, SpeakerConfiguration,
    VideoOptions, WindowDisplayMode,
};
pub use cutaway::{
    original_sheet_dimensions, CutawayOverlayEngine, OverlayKind, OverlaySpriteQuad, OV_BOX,
    OV_CLOCK_UV, OV_COIN, OV_DRUM_ABC, OV_DRUM_POS, OV_ELEVATOR, OV_ELEVATOR2, OV_MARKER, OV_TEIEN,
    OV_VIDEO, OV_WALL, SHEET_ORIGINAL_DIMENSIONS,
};
pub use cutscene::{
    demo_fov_scale_for_demo_no, demo_light_decay_bounds, derive_demo_actor_anm_and_cls_paths,
    find_drama_demo_anim_info, find_drama_demo_by_id, find_drama_demo_by_no, CutsceneActorRuntime,
    CutsceneCameraSnapshot, CutsceneSequenceStep, DramaDemoAnimInfo, DramaDemoPlayInfo,
    DramaDemoPlayer, DramaDemoStep, SubtitleWindow, CUTSCENE_NATIVE_FPS, DRAMA_DEMO_ANIM_INFO,
    DRAMA_DEMO_CATALOG, VOICE_AFS_STREAM_BASE,
};
pub use events::{
    check_event_flag_conditions, decode_ev_pos_shape, eval_monkey_kick_206,
    evaluate_baldwin_tombstone_boards, evaluate_event_entry, event_check, event_check_in,
    event_check_look, event_check_look_line, event_check_look_point,
    event_exec_common_fail_message, event_exec_door, event_exec_flag, event_exec_item,
    event_exec_message, event_exec_move, event_exec_save, event_exec_sub_flag_set,
    event_item_condition_check, event_position_set, event_progress_check,
    event_result_move_position, format_carbon_box_code, format_wallet_safe_code, init_new_game,
    init_new_game_wish, is_clock_solved, is_coin_desk_solved, is_connected_stage_pair,
    item_check_look_point, light_spot_on_off_check, place_baldwin_board, place_coin_in_desk,
    player_height_matches_event_y, riddle_level_index, step_clock_needles, step_safe_dial,
    try_complete_hospital_box, try_open_safe, use_key_on_hospital_box, verify_hospital_box_keypad,
    verify_hospital_box_wheels, verify_hospital_corridor_keypad, AnyoneCryProgramState,
    CameraOverrideCommand, ClearData, CollisionWallCommand, EvPosShape, EventEngine,
    EventEngineEffects, EventEvalInput, EventExecMode, EventFlags, EventPositionalSound,
    EventStepState, EventTriggerState, MessageInsertState, ModelStateCommand, ProgContext,
    PuzzleState, Sh2Rng, SpatialSoundCommand, StageEventDispatcher, StageHookContext,
    StageHookOutput, StageId, StageProgramRunner, StageTransitionCommand, APART_E3FE_ROOM13_CAMERA,
    EVENT_IGNORE_HEIGHT_Y, EVENT_ITEM_INDEX_BASE, EV_FLAG_BYTES, EV_MAX_FLAGS,
    FLAG_APART_E3FE_FOOTSTEPS_DONE, FLAG_APART_ROOM209_WHISPER_DONE, FLAG_APART_W2F_CORRIDOR_DONE,
    FLAG_PRISON_BATHROOM_JUMPSCARE_DONE, FLAG_PRISON_BATHROOM_KNOCKED, FLAG_PRISON_BF_ROOM77_OPEN,
    FLAG_PRISON_N_ROOM66_WALL, FLAG_PRISON_S_BARS_ACTIVE, FLAG_PRISON_S_BARS_BOOST,
    FLAG_PRISON_S_BARS_COUNTING, FLAG_SOCIETY_ROOM5C_TRIGGERED, MAX_STAGE_EVENT_ENTRIES,
    PROG_KEY_A, PROG_KEY_B, PROG_KEY_DOWN, PROG_KEY_LEFT, PROG_KEY_RIGHT, PROG_KEY_UP, PZ_BOX_CODE,
    PZ_CLOCK, PZ_DRUM, PZ_HANGMAN, PZ_PANEL, PZ_SAFE_CODE, PZ_SAFE_PROG, PZ_WHEEL, PZ_WHEEL_GOAL,
    SE_APART_ANYONE_CRY, SE_APART_E1F_AMBIENT, SE_APART_E3FE_FOOTSTEPS, SE_APART_MONKEY_KICK_206,
    SE_APART_ROOM205_TV_STATIC, SE_APART_ROOM209_WHISPER, SE_APART_STAIR_WATER_DRAIN,
    SE_APART_W2F_CORRIDOR, SE_PRISON_BATHROOM_STALL_BANG, SE_PRISON_INVISIBLE_PRISONER,
    SE_PRISON_ROOM68_MECHANISM, SE_SOCIETY_ROOM_5C_HORROR, SE_SOCIETY_STAIRCASE_AMBIENT,
};
pub use input::{
    ControlScheme, ControllerConfig, ControllerSpeakerCue, GamepadInputState, HapticFeedback,
    InputManager,
};
pub use inventory::{
    combine_items, item_metadata, HealResult, InventoryManager, ItemCategory, ItemCombination,
    ItemExamineView, ItemMetadata, AMPOULE_STAMINA_DURATION_SECS, CATALOG_ITEM_COUNT,
    FIRST_COMBO_ITEM_ID, ITEM_CATALOG, ITEM_COMBOS, ITEM_SYSTEM_MESSAGES, LAST_COMBO_ITEM_ID,
    MAX_ITEM_STACK,
};
pub use locomotion::{
    LocomotionInput, LocomotionOutput, LocomotionState, PlayerLocomotionController,
    MARIA_COMPANION_SPEED_MULTIPLIER, MIN_FOOTSTEP_INTERVAL_SECS, QUICK_TURN_SPEED_RAD_PER_SEC,
    SH2_WORLD_UNIT_SCALE,
};
pub use scenario::{GameItem, PlayableCharacter, ScenarioKind, ScenarioState, StageData};
pub use secondary_physics::{SecondaryBonePhysics, SecondaryJointConfig, SecondaryJointState};
pub use simulation::SimulationCore;
pub use timing::{lerp_angle_shortest, FlashlightSweepState, RowboatController, TimingConstants};
