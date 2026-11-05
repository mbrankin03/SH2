//! sh2-core: Core engine simulation, state machines, math, camera, and timing.

pub mod animation;
pub mod camera;
pub mod clock;
pub mod config;
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
    AnimationClip, AnimationStateKind, CharacterAnimationController, CharacterLocomotionClips,
    JamesBasicClips, MariaLocomotionClips,
};
pub use camera::{CameraMovementType, CameraSnapshot, Ps2CameraController};
pub use clock::EngineClock;
pub use config::{
    ActionDifficulty, AppConfig, AudioOptions, BulletMultiplier, ButtonGlyphStyle, ControlOptions,
    FogQuality, FrameRateCap, GameLanguage, GameOptions, InternalResolution, RiddleDifficulty,
    SpeakerConfiguration, VideoOptions, WindowDisplayMode,
};
pub use input::{
    ControlScheme, ControllerConfig, ControllerSpeakerCue, GamepadInputState, HapticFeedback,
    InputManager,
};
pub use locomotion::{
    LocomotionInput, LocomotionOutput, LocomotionState, PlayerLocomotionController,
    MARIA_SPEED_MULTIPLIER, QUICK_TURN_SPEED_RAD_PER_SEC, SH2_WORLD_UNIT_SCALE,
};
pub use scenario::{GameItem, PlayableCharacter, ScenarioKind, ScenarioState, StageData};
pub use secondary_physics::{SecondaryBonePhysics, SecondaryJointConfig, SecondaryJointState};
pub use simulation::SimulationCore;
pub use timing::TimingConstants;
