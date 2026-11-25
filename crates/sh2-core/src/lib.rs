//! sh2-core: Core engine simulation, state machines, math, camera, and timing.

pub mod ai;
pub mod animation;
pub mod boat;
pub mod camera;
pub mod campaign;
pub mod clock;
pub mod collision;
pub mod combat;
pub mod config;
pub mod cutaway;
pub mod cutscene;
pub mod endings;
pub mod event_camera;
pub mod events;
pub mod fog;
pub mod input;
pub mod inventory;
pub mod lighting;
pub mod locomotion;
pub mod math;
pub mod modern_camera;
pub mod room_loader;
pub mod scenario;
pub mod secondary_physics;
pub mod simulation;
pub mod state;
pub mod timing;
pub mod types;
pub mod widescreen_rail;
pub mod world;

// Re-exports for convenience
pub use animation::{
    AnimationClip, AnimationStateKind, BlendMode, CharacterAnimationController,
    CharacterLocomotionClips, FootGroundSample, FootIkRigSpec, FootIkSolver, HeadLookTracker,
    JamesBasicClips, JamesStageClips, LegChainSpec, LocomotionAnimCommand, LocomotionAnimationSync,
    LookTargetCandidate, LookTargetKind, MariaLocomotionClips,
};
pub use camera::{CameraMovementType, CameraSnapshot, CameraSourceKind, Ps2CameraController};
pub use campaign::{
    apply_post_cutscene_handoff, evaluate_auto_stage_demo, resolve_stage_manifest, room_area_dir,
    AutoDemoRule, CampaignEffects, CampaignEnemySpawn, CampaignOrchestrator, CampaignPhase,
    CampaignPickupSpawn, CampaignTickInput, ConnectStep, DramaDemoTrigger, FadeDirection,
    FmvTrigger, OverlayMenuKind, RoomLoadManifest, RoomLoadTarget, ScreenFadeState,
    AUTO_STAGE_DEMO_RULES, MAIN_SCENARIO_START_STAGE, PS2_DEFAULT_FADE_SPEED, SE_GAME_OVER,
    SH2EE_DELAYED_FADE_IN_START, WISH_SCENARIO_START_STAGE,
};
pub use clock::{EngineClock, FramePacer, DEFAULT_MAX_ACCUMULATED};
pub use collision::{
    listener_head_position, AcousticOcclusionHit, FloorSample, SurfaceMaterial,
    JAMES_HEAD_OFFSET_Y, MARIA_HEAD_OFFSET_Y,
};
pub use combat::{
    aim_turn_toward_target, check_guard_success, cycle_auto_aim_target, find_near_deadly_target,
    generate_hyper_spray_rays, generate_shotgun_cone_rays, get_attack_spec,
    hyper_spray_effective_range, is_guardable_attack, select_auto_aim_target, AttackDispatchKind,
    AttackSpec, CombatArena, CombatController, CombatHitbox, CombatInputSnapshot, CombatMode,
    CombatRng, GuardState, HyperSprayColor, SweptBladeQuad, TargetCandidate, WeaponGrip,
    ATK_CHAINSAW_LIGHT, ATK_CHAINSAW_THRUST, ATK_GREAT_KNIFE_OVERHEAD_1,
    ATK_GREAT_KNIFE_OVERHEAD_2, ATK_GREAT_KNIFE_SWING_1, ATK_GREAT_KNIFE_SWING_2, ATK_HANDGUN,
    ATK_HANDGUN_STANDING, ATK_HUNTING_RIFLE, ATK_HYPER_SPRAY_1, ATK_HYPER_SPRAY_2, ATK_KICKS,
    ATK_MARIA_CLEAVER_COMBO, ATK_MARIA_CLEAVER_HEAVY, ATK_MARIA_CLEAVER_LIGHT,
    ATK_MARIA_REVOLVER_1, ATK_MARIA_REVOLVER_2, ATK_PIPE_COMBO, ATK_PIPE_LUNGE, ATK_PIPE_OVERHEAD,
    ATK_PIPE_SWING, ATK_PLANK_COMBO, ATK_PLANK_HEAVY, ATK_PLANK_LIGHT, ATK_SHOTGUN, ATK_STOMPS,
    HOLD_TIMER_THRESHOLD, PS2_PRESSURE_HEAVY_THRESHOLD, SH2_ATTACK_LIST,
};
pub use config::{
    ActionDifficulty, AppConfig, AudioOptions, AudioPackDetectionStatus, AudioPackPreference,
    AudioPackSource, BulletMultiplier, ButtonGlyphStyle, CombatInputMode, ControlOptions,
    CutawayQuality, EnhancedPreset, FogQuality, FrameRateCap, GameLanguage, GameOptions,
    InternalResolution, LightingProfile, ModEntryConfig, ModsConfig, OutdoorStreamingMode,
    RiddleDifficulty, ShadowQualityMode, SpatialAudioMode, SpeakerConfiguration, VideoOptions,
    WindowDisplayMode, WorldTextureQuality,
};
pub use cutaway::{
    is_cutaway_widescreen_scalable, original_sheet_dimensions, ClippedQuadUv, CutawayAspectMode,
    CutawayOverlayEngine, CutsceneBorderBars, FullscreenImageLayout, OverlayKind,
    OverlaySpriteQuad, PillarboxMaskRect, PillarboxViewportLayout,
    FULLSCREEN_MAX_WIDESCREEN_ASPECT, OV_BOX, OV_CLOCK_UV, OV_COIN, OV_DRUM_ABC, OV_DRUM_POS,
    OV_ELEVATOR, OV_ELEVATOR2, OV_MARKER, OV_TEIEN, OV_VIDEO, OV_WALL, SHEET_ORIGINAL_DIMENSIONS,
};
pub use cutscene::{
    authored_4x3_fov_radians, camera_basis_vectors, compute_widescreen_scr_z,
    demo_fov_scale_for_demo_no, demo_light_decay_bounds, derive_demo_actor_anm_and_cls_paths,
    effective_right_hand_pose, evaluate_eddie_boss_death_blur, evaluate_hei_lau_widescreen_entry,
    evaluate_mary_boss_blur_factor, evaluate_meeting_maria_fog_counters, find_drama_demo_anim_info,
    find_drama_demo_by_id, find_drama_demo_by_no, is_sphere_inside_4x3_frustum,
    observation_deck_border_alpha_factor, observation_deck_fog_bounds_x,
    observation_deck_wrap_particle_x, resolve_cutscene_room_id, should_cull_parked_offscreen_actor,
    should_unequip_weapon_in_cutscene, ActorBoundingSphere, AtmosphereEvalInput,
    AtmosphereGradingEvaluation, ClosetCutsceneFixState, ClosetDoorReplacementState,
    CutsceneActorRuntime, CutsceneBorderMode, CutsceneCameraSnapshot, CutsceneSequenceStep,
    CutsceneWidescreenMode, DogRoomLeversRenderCmd, DogRoomLeversState, DramaDemoAnimInfo,
    DramaDemoPlayInfo, DramaDemoPlayer, DramaDemoStep, FinalBossRoomFixState,
    HeiLauWidescreenEntryState, MotionBlurRampWindow, PistonArrayRuntime, PistonAxisDir,
    PistonRoomFixState, ScreenTransitionState, SpecialFxEvalInput, SpecialFxEvaluation,
    SubtitleWindow, TreeLightingMode, ANGELA_ABSTRACT_DADDY_BLUR_RAMPS, AUTHORED_4X3_ASPECT,
    CLOSET_BLURRED_BARS_FIXED_DESPAWN_FRAME, CLOSET_BLURRED_BARS_HIDE_CAM_X,
    CLOSET_BLURRED_BARS_ORIG_DESPAWN_FRAME, CLOSET_BONUS_AUDIO_TRIGGER_FRAME,
    CLOSET_BONUS_AUDIO_WAV_PATH, CLOSET_DARKENED_AMBIENT_SCALE, CLOSET_DISABLE_FLASHLIGHT_CAM_X,
    CLOSET_DISABLE_SHADOW_CAM_X, CLOSET_DOOR_HIDE_END_FRAME, CLOSET_DOOR_HIDE_START_FRAME,
    CLOSET_DOOR_WORLD_MATRIX, CLOSET_MANNEQUIN_FIXED_DESPAWN_FRAME,
    CLOSET_MANNEQUIN_ORIG_DESPAWN_FRAME, CS_APT_RPT_CLOSET_DEMO_NO, CS_APT_RPT_FIGHT_DEMO_NO,
    CS_END_LEAVE_LETTER_DEMO_NO, CS_END_MARIA_LETTER_DEMO_NO, CS_HSP_BOSS_FINISH_DEMO_NO,
    CS_INTRO_BATHROOM_DEMO_NO, CS_OB_VIEW_NEIGHBORHOOD_DEMO_NO, CS_OUT_LAURA_ON_WALL_DEMO_NO,
    CS_PS_ANGELA_BOSS_FINISH_DEMO_NO, CS_PS_ANGELA_BOSS_START_DEMO_NO, CS_PS_MARIA_MEET_DEMO_NO,
    CS_TOWN_WEST_MARIA_MEET_DEMO_NO, CS_TUNNEL_RADIO_DEMO_NO, CUTSCENE_NATIVE_FPS,
    DEFAULT_BOSS_BOUNDING_RADIUS, DEFAULT_HUMANOID_BOUNDING_RADIUS, DEFAULT_ITEM_BOUNDING_RADIUS,
    DEFAULT_PISTON_STROKE_AMPLITUDE, DEFAULT_PROP_BOUNDING_RADIUS, DOG_CUTSCENE_TIMER_HZ,
    DOG_MODEL_OFFSET_TABLE, DOG_ROOM_PRELOAD_EVENT_INDEX, DOORWAY_PISTON_INDEX,
    DRAMA_DEMO_ANIM_INFO, DRAMA_DEMO_CATALOG, EDDIE_BOSS_DEATH_TIMER_CHECK,
    EDDIE_BOSS_DEATH_TIMER_SCALE, FINAL_BOSS_EXTENDED_DRAW_DISTANCE,
    FINAL_BOSS_JAMES_ELEVATION_THRESHOLD_Y, FINAL_BOSS_LOWERED_SPAWN_Y,
    FLESH_LIPS_CUTSCENE_MOTION_BLUR_FACTOR, FLESH_LIPS_DEATH_MOTION_BLUR_FACTOR,
    HEI_LAU_JAMES_AUTHORED_WALK_START_FRAME, HEI_LAU_JAMES_PREROLL_START_FRAME,
    HEI_LAU_JAMES_WALK_VELOCITY_PER_FRAME, HOTEL_ROOM_312_SUNSET_TINT_RGB,
    JAMES_EMPTY_RIGHT_HAND_POSE, LEVERS_MODEL_OFFSET_TABLE, MARIA_ENDING_HORIZON_FOG_RGB,
    MARIA_JAIL_CELL_BLUR_FACTORS, MARY_BOSS_BLUR_SCALE, MEETING_MARIA_FOG_COUNTER_ONE_FIXED,
    MEETING_MARIA_FOG_COUNTER_ONE_ORIG, MEETING_MARIA_FOG_COUNTER_TWO_FIXED,
    MEETING_MARIA_FOG_COUNTER_TWO_ORIG, OBSERVATION_DECK_FOG_BORDER_WIDTH,
    OBSERVATION_DECK_FOG_PARTICLE_MAX, OBSERVATION_DECK_ORIG_MAX_POS_X,
    OBSERVATION_DECK_ULTRAWIDE_MAX_POS_X, PARKED_ACTOR_VELOCITY_EPSILON, PISTON_ARRAY_COUNT,
    PISTON_AXIS_DIR_TABLE, PISTON_DOORWAY_PROP_OFFSET, PISTON_ROOM_DOOR_OPEN_CAM_X,
    PISTON_SPEED_TABLE, PS2_MOTION_BLUR_SCALE, R_APT_E_RM_307_ID, R_END_DOG_RM_ID,
    R_FINAL_BOSS_RM_ID, R_HTL_RM_312_ID, R_LAB_BOTTOM_D_ID, R_LAB_TOP_H_ID, SH2EE_16X9_SCR_Z_ZOOM,
    SPECIAL_FX_REFERENCE_HEIGHT, STAGE_END_DOG_ID, STANDARD_16X9_ASPECT,
    SUPER_ULTRAWIDE_32X9_ASPECT, TIME_OF_DAY_APARTMENT_DAY, TIME_OF_DAY_CLOSET_DARKENED,
    ULTRAWIDE_21X9_ASPECT, VOICE_AFS_STREAM_BASE,
};
pub use event_camera::{
    effective_road_priority, evaluate_nearest_enemy, fov_y_from_ps2_projection,
    is_road_eligible_for_enemy, old_road_advantage_mm, ps2_projection_from_fov_y,
    rolled_ps2_up_vector, select_best_road_index, CutsceneCameraHandoff, CutsceneHandoffPhase,
    DemoCameraTransitionKind, DemoFrameSyncState, EnemyCameraAlertState, EnemyCameraTarget,
    EnemyEncounterFraming, EnemyLifeCameraState, EnemyProximityState, EventCameraAuthority,
    EventCameraSystem, FmvAdvanceSignal, FmvPlaybackState, HandoffStepOutcome,
    PlayerCameraSuppression, PostSequenceRestorePolicy, ProjectionChangeTiming,
    ProjectionTransitionState, SavedGameplayCameraState, ScriptCameraController, ScriptCameraMode,
    ScriptTransitionKind, ScriptedCameraShot, SelfViewGate, StageEventRoadGate,
    BOWLING_ALLEY_EDDIE_CAM_1, BOWLING_ALLEY_EDDIE_CAM_2, BOWLING_ALLEY_ENTRANCE_CAM,
    DEFAULT_USER_CAM_MV_PARAM, DEFAULT_USER_WATCH_MV_PARAM, FOREST_CEMETERY_ANGELA_CAM,
    PS2_DEFAULT_PROJECTION_SCR_Z, VC_ALL_WARP_FLAGS, VC_DEAD_ENEMY_GRACE_SECS,
    VC_DEAD_ENEMY_TIMER_MAX_SECS, VC_EVENT_EFFECT_ADVANTAGE_MM, VC_EXTRA_BOUNDARY_WIDTH_MM,
    VC_INHIBIT_FAR_WATCH_F, VC_MAX_CAMERA_TARGETS, VC_NEAR_ENEMY_MAX_DIST_MM,
    VC_PROJ_MOMENT_CHANGE_F, VC_RESET_ROAD_MARGIN_MM, VC_TICK_ROAD_MARGIN_MM, VC_USER_CAM_F,
    VC_USER_MODE_FLAGS, VC_USER_WATCH_F, VC_WARP_CAM_F, VC_WARP_CAM_TGT_F, VC_WARP_WATCH_F,
};
pub use events::{
    check_event_flag_conditions, decode_cursor_axis, decode_ev_pos_shape,
    ev_disp_control_model_entry, ev_disp_control_model_exec, ev_sub_file_load_and_fade_out,
    ev_sub_item_get, ev_sub_item_get_and_anim, ev_sub_item_use0, ev_sub_map_get, ev_sub_message,
    ev_sub_message_retail_pc, ev_sub_movie_end, ev_sub_movie_ready, ev_sub_movie_start,
    ev_sub_picture_cursor, ev_sub_picture_display, ev_sub_picture_display_and_fade_in,
    ev_sub_picture_display_and_fade_out, ev_sub_picture_display_only, ev_sub_picture_end,
    ev_sub_picture_filter, ev_sub_picture_init, ev_sub_picture_layer, ev_sub_picture_start,
    ev_sub_question, eval_monkey_kick_206, evaluate_baldwin_tombstone_boards, evaluate_event_entry,
    event_check, event_check_in, event_check_look, event_check_look_line, event_check_look_point,
    event_exec_common_fail_message, event_exec_door, event_exec_flag, event_exec_item,
    event_exec_message, event_exec_move, event_exec_save, event_exec_sub_flag_set,
    event_item_condition_check, event_position_set, event_progress_check,
    event_result_move_position, format_carbon_box_code, format_wallet_safe_code, init_new_game,
    init_new_game_wish, is_clock_solved, is_coin_desk_solved, is_connected_stage_pair,
    item_check_look_point, item_use_se_timing, light_spot_on_off_check, picture_draw,
    picture_load_image, place_baldwin_board, place_coin_in_desk, player_height_matches_event_y,
    riddle_level_index, step_clock_needles, step_safe_dial, try_complete_hospital_box,
    try_open_safe, use_key_on_hospital_box, verify_hospital_box_keypad, verify_hospital_box_wheels,
    verify_hospital_corridor_keypad, AnyoneCryProgramState, CameraOverrideCommand, ClearData,
    CollisionWallCommand, CursorBounds, CursorInput, CursorTintMode, DispControlModelEntry,
    DispControlModelTable, DispOnOffCommand, EvPosShape, EvSubItemGetAndAnimInput,
    EvSubItemUseParams, EvSubPictureDisplayInput, EvSubPictureState, EventEngine,
    EventEngineEffects, EventEvalInput, EventExecMode, EventFlags, EventPositionalSound,
    EventStepState, EventTriggerState, FileLoadPlan, InvalidFileLoadArgs, MessageInsertState,
    ModelStateCommand, MovieCommand, MovieReadyInput, MovieStartInput, MovieState,
    MovieTimingProfile, PictureAlphaBlend, PictureDrawCommand, PictureDrawParams, PictureFlags,
    PictureLoadImageCommand, PicturePrimitiveKind, PictureTestConfig, PictureUv, PlayerAnimSignal,
    ProgContext, PuzzleState, Sh2Rng, SpatialSoundCommand, StageEventDispatcher, StageHookContext,
    StageHookOutput, StageId, StageProgramRunner, StageTransitionCommand, ANIM_ID_PICKUP_CROUCH,
    ANIM_ID_PICKUP_STAND, APART_E3FE_ROOM13_CAMERA, CURSOR_DEADZONE_HIGH, CURSOR_DEADZONE_LOW,
    CURSOR_SPEED_X_PX_PER_SEC, CURSOR_SPEED_Y_PX_PER_SEC, DEMO_STATUS_MOVIE_READY_BIT,
    EVENT_IGNORE_HEIGHT_Y, EVENT_ITEM_INDEX_BASE, EV_FILTER_ALPHA_SCALE, EV_FILTER_RATE_PER_SEC,
    EV_FLAG_BYTES, EV_MAX_FLAGS, FILE_LAYER_ALIGN_BYTES, FLAG_APART_E3FE_FOOTSTEPS_DONE,
    FLAG_APART_ROOM209_WHISPER_DONE, FLAG_APART_W2F_CORRIDOR_DONE,
    FLAG_PRISON_BATHROOM_JUMPSCARE_DONE, FLAG_PRISON_BATHROOM_KNOCKED, FLAG_PRISON_BF_ROOM77_OPEN,
    FLAG_PRISON_N_ROOM66_WALL, FLAG_PRISON_S_BARS_ACTIVE, FLAG_PRISON_S_BARS_BOOST,
    FLAG_PRISON_S_BARS_COUNTING, FLAG_SOCIETY_ROOM5C_TRIGGERED, ITEM_CROUCH_PICKUP_THRESHOLD_MM,
    MAX_STAGE_EVENT_ENTRIES, MOVIE_MAIN_MODE_PC, MOVIE_MAIN_MODE_PS2, MOVIE_READY_LEAD_FRAMES_PC,
    MOVIE_READY_LEAD_FRAMES_PS2, PICTURE_DEFAULT_CBP, PROG_KEY_A, PROG_KEY_B, PROG_KEY_DOWN,
    PROG_KEY_LEFT, PROG_KEY_RIGHT, PROG_KEY_UP, PZ_BOX_CODE, PZ_CLOCK, PZ_DRUM, PZ_HANGMAN,
    PZ_PANEL, PZ_SAFE_CODE, PZ_SAFE_PROG, PZ_WHEEL, PZ_WHEEL_GOAL, SE_APART_ANYONE_CRY,
    SE_APART_E1F_AMBIENT, SE_APART_E3FE_FOOTSTEPS, SE_APART_MONKEY_KICK_206,
    SE_APART_ROOM205_TV_STATIC, SE_APART_ROOM209_WHISPER, SE_APART_STAIR_WATER_DRAIN,
    SE_APART_W2F_CORRIDOR, SE_ID_ITEM_GET, SE_PRISON_BATHROOM_STALL_BANG,
    SE_PRISON_INVISIBLE_PRISONER, SE_PRISON_ROOM68_MECHANISM, SE_SOCIETY_ROOM_5C_HORROR,
    SE_SOCIETY_STAIRCASE_AMBIENT, SYS_FLAG_DEMO_ACTIVE_BIT,
};
pub use fog::{
    dynamic_draw_distance_scale, evaluate_room_fog, evaluate_room_fog_with_fixes,
    ps2_depth_fog_linear_coeffs, room_draw_distance_mm, room_id_from_stage_code, FogFalloffCurve,
    FogParticle, FogParticleVolume, Ps2FogLayerParams, RoomFogContext, RoomFogEvaluation,
    BLUE_CREEK_FLASHLIGHT_FOG_NEAR_MM, BLUE_CREEK_ORIGINAL_FOG_NEAR_MM, DEFAULT_DRAW_DISTANCE_MM,
    FINAL_BOSS_CAMERA_Y_THRESHOLD_MM, FOG_SPEED_COURTYARD_MEDIUM, FOG_SPEED_OUTDOOR_SLOW,
    FOREST_TRAIL_MAX_Y_MM, FOREST_TRAIL_MIN_Y_MM, PS2_DEFAULT_FOG_HEIGHT_BAND_MM,
    PS2_DEFAULT_FOG_PARTICLE_COUNT, PS2_DEFAULT_FOG_VOLUME_HALF_EXTENT_MM,
    PS2_DEFAULT_JAMES_FOG_INFLUENCE_MM, PS2_FOREST_TRAIL_JAMES_FOG_INFLUENCE_MM,
    R_APT_E_COURTYARD_ID, R_APT_W_RM_208_209_ID, R_EDI_BOSS_RM_1_ID, R_EDI_BOSS_RM_2_ID,
    R_FOREST_CEMETERY_ID, R_MAN_BLUE_CREEK_ENTRANCE_ID, R_OBSV_DECK_ID, R_TOWN_EAST_ID,
    R_TOWN_WEST_ID, SH2EE_EXTENDED_DRAW_DISTANCE_MM,
};
pub use input::{
    ControlScheme, ControllerConfig, ControllerSpeakerCue, GamepadInputState, GamepadSnapshot,
    HapticFeedback, InputManager,
};
pub use inventory::{
    combine_items, item_combination_use_check, item_metadata, HealResult, InventoryManager,
    ItemCategory, ItemCombination, ItemExamineView, ItemMetadata,
    AMPOULE_FULL_EFFICACY_THRESHOLD_SECS, AMPOULE_STAMINA_DURATION_SECS, CATALOG_ITEM_COUNT,
    FIRST_COMBO_ITEM_ID, ITEM_CATALOG, ITEM_COMBOS, ITEM_SYSTEM_MESSAGES, LAST_COMBO_ITEM_ID,
    MAX_ITEM_STACK,
};
pub use locomotion::{
    LocomotionInput, LocomotionOutput, LocomotionState, PlayerLocomotionController,
    MARIA_COMPANION_SPEED_MULTIPLIER, MIN_FOOTSTEP_INTERVAL_SECS, QUICK_TURN_SPEED_RAD_PER_SEC,
    SH2_WORLD_UNIT_SCALE,
};
pub use modern_camera::{
    beam_quat_from_direction, blend_snapshots, constrain_eye_in_room, sample_local_ceiling_y,
    smoothstep01, snapshot_from_cutscene, solve_ots_spring_arm, sweep_ray_against_cld,
    CameraCyclePreset, CharacterHeadRigSpec, ExternalCameraHandoff, FirstPersonCameraRig,
    FirstPersonConfig, GameplayCameraMode, HeadBobMode, ModeTransitionState, ModernCameraFrame,
    ModernCameraRig, ModernCameraTickInput, OtsAimPhase, OtsBoomQuery, OtsCameraConfig,
    OtsCameraRig, OtsReticleState, OtsShoulderSide, PlayerMeshVisibility, QuickTurnPhase,
    ReticleMode, SpringArmConfig, SpringArmState, FIRST_PERSON_NEAR_CLIP_UNITS,
    STANDARD_NEAR_CLIP_UNITS,
};
pub use room_loader::{
    default_room_id_for_stage, resolve_room_id_for_stage_or_override, stage_room_descriptor,
    StageRoomDescriptor,
};
pub use scenario::{GameItem, PlayableCharacter, ScenarioKind, ScenarioState, StageData};
pub use secondary_physics::{
    SecondaryBonePhysics, SecondaryJointConfig, SecondaryJointState, ThighCapsuleSpec,
};
pub use simulation::SimulationCore;
pub use timing::{lerp_angle_shortest, FlashlightSweepState, RowboatController, TimingConstants};
pub use widescreen_rail::{
    apply_chase_rail_clearance, base_ps2_rail_margin_mm, clamp_axis_with_margin,
    clamp_to_road_rail_with_config, compute_aspect_rail_margin_mm, evaluate_intra_room_transition,
    is_outdoor_room_or_record, is_rail_constrained, switch_hysteresis_advantage_mm,
    AspectRailConfig, IntraRoomTransitionPolicy, IntraRoomTransitionStep, TransitionLerpFactors,
    WidescreenClearanceMode, ASPECT_16X10, ASPECT_16X9, ASPECT_20X9_THOR, ASPECT_21X9_ULTRAWIDE,
    DEFAULT_SMOOTH_WARP_EYE_LERP, DEFAULT_SMOOTH_WARP_TARGET_LERP, PS2_INDOOR_MIN_IN_ROAD_DIST_MM,
    PS2_OUTDOOR_MIN_IN_ROAD_DIST_MM, PS2_REFERENCE_4X3_ASPECT,
};
pub use world::{
    bg_chara_block_count, bg_chara_is_id, chara_bound_radius_mm, evaluate_roach_jiggle,
    event_main_sys_flag_mask, hospital_chase_elevator_stab_param,
    hotel_employee_elevator_cursor_color, is_within_outdoor_prop_range,
    resolve_active_room_pickups, ActiveRoomPickup, BgCharaSlots, BgObjectState,
    CockroachDrawAction, CockroachSwarmState, ElevatorCursorColor, ElevatorFloor, ElevatorKind,
    MirrorMode, MirrorPlane, MultiFloorElevatorPanel, OutdoorArea, OutdoorTileId,
    OutdoorTileStreamer, PushableObjectKind, RoachJiggleParams, StayObjectState, TileLoadRequest,
    TileStreamingPlan, BG_CHARA_BLOCK_SIZE_BYTES, BG_OBJECT_SHADOW_Y_OFFSET_MM, CA_OUTDOOR_BLOCKS,
    CB_OUTDOOR_BLOCKS, CC_OUTDOOR_BLOCKS, CD_OUTDOOR_BLOCKS, CHARA_ID_TY2, CHARA_ID_TY3,
    CHARA_ID_TYU, COCKROACH_TIMER_SEED_MAX_SEC, DEFAULT_CHARA_BOUND_R_MM, FLAG_APART_CLOCK_PUSHED,
    FLAG_APART_CLOCK_SOLVED, FLAG_HOSPITAL_B1_SHELF_PUSHED, FLASHLIGHT_CLOCK_PUSH_CAM_Y_MM,
    HOSPITAL_CHASE_STAB_PARAM_FIXED, HOSPITAL_CHASE_STAB_X_THRESHOLD_MM, LOADBG_DIST_BANDS_MM,
    LOADBG_PREFETCH_CUTOFF_MM, MAX_COCKROACH_TIMERS, MIRROR_WALL_POS_ROOM_01_X_MM,
    MIRROR_WALL_POS_ROOM_24_Z_MM, MODEL_ID_SPECIAL_PI_YAW, OUTDOOR_GRID_OFFSET,
    OUTDOOR_PROP_CULL_MARGIN_MM, OUTDOOR_TILE_HALF_MM, OUTDOOR_TILE_SIZE_MM, ROOM_APT_E_RM_208,
    ROOM_HSP_ALT_RPT_HALLWAY, R_BUG_RM_ID, STAY_OBJECT_SHADOW_Y_OFFSET_MM,
    SYS_FLAG_MASK_CLOCK_PUSH_FIX, SYS_FLAG_MASK_VANILLA, TY2_BODY_NUM_VERTICES,
    TY2_BODY_PRIM_COUNT, TY2_LIMB_PRIM_COUNT, TY3_BODY_NUM_VERTICES, TY3_BODY_PRIM_COUNT,
    TY3_LIMB_PRIM_COUNTS, TY3_SHADOW_NUM_VERTICES, TY3_SHADOW_PRIM_COUNT,
    VANILLA_ROACH_JIGGLE_POS_MAX, VANILLA_ROACH_JIGGLE_POS_MIN, VANILLA_ROACH_JIGGLE_YAW_MAX,
    VANILLA_ROACH_JIGGLE_YAW_MIN,
};
