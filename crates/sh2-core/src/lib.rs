//! sh2-core: Core engine simulation, state machines, math, camera, and timing.

pub mod ai;
pub mod animation;
pub mod camera;
pub mod clock;
pub mod collision;
pub mod combat;
pub mod config;
pub mod cutscene;
pub mod input;
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
pub use cutscene::{
    demo_fov_scale_for_demo_no, demo_light_decay_bounds, derive_demo_actor_anm_and_cls_paths,
    find_drama_demo_anim_info, find_drama_demo_by_id, find_drama_demo_by_no, CutsceneActorRuntime,
    CutsceneCameraSnapshot, CutsceneSequenceStep, DramaDemoAnimInfo, DramaDemoPlayInfo,
    DramaDemoPlayer, DramaDemoStep, SubtitleWindow, CUTSCENE_NATIVE_FPS, DRAMA_DEMO_ANIM_INFO,
    DRAMA_DEMO_CATALOG, VOICE_AFS_STREAM_BASE,
};
pub use input::{
    ControlScheme, ControllerConfig, ControllerSpeakerCue, GamepadInputState, HapticFeedback,
    InputManager,
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
