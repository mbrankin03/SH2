//! Scripted Environmental Horror Triggers & PS2 `Stage_Data` Lifecycle Dispatcher
//! (`EvStageInit`, `EvRoomInit`, `EvAllTimeFunc`, `EvSoundCallAfterLoad`, `EvBgmControl`).
//!
//! Verified 1:1 against the PS2 prototype (`SLUS_202.28` + `/GX/*.BIN`) and retail PC binary
//! (`sh2pc.exe`).

pub mod apartments;
pub mod prison_society;

pub use apartments::{eval_monkey_kick_206, AnyoneCryProgramState, APART_E3FE_ROOM13_CAMERA};

use std::f32::consts::FRAC_PI_2;

use crate::types::EventCamSetData;

use super::{EventFlags, Sh2Rng};

// ============================================================================
// Sound Effect Constants (Verified against SLUS_202.28 / sh2pc.exe)
// ============================================================================

/// Toluca Prison North Room `0x73` (women's bathroom): loud stall door bang/scream jumpscare (`0x427d` = `17021`).
pub const SE_PRISON_BATHROOM_STALL_BANG: u32 = 0x427d;

/// Toluca Prison South Room `0x72` (southern cellblock): invisible prisoner chanting `"Ritual..."` (`0x4682` = `18050`).
pub const SE_PRISON_INVISIBLE_PRISONER: u32 = 0x4682;

/// Toluca Prison South Room `0x68`: gallows / mechanism sound checked for model `0x1d` (`0x070d` = `1805`).
pub const SE_PRISON_ROOM68_MECHANISM: u32 = 0x070d;

/// Historical Society Room `0x5d` (long descending staircase): depth-attenuated ambient horn/groan (`0x9c73` = `40051`).
pub const SE_SOCIETY_STAIRCASE_AMBIENT: u32 = 0x9c73;

/// Historical Society Room `0x5c`: one-shot environmental horror cue (`0x4982` = `18818`).
pub const SE_SOCIETY_ROOM_5C_HORROR: u32 = 0x4982;

/// Blue Creek Apartments W 2F Room `0x28` (Room 209): orbiting whisper `"See?"` (`0x9c70` = `40048`).
pub const SE_APART_ROOM209_WHISPER: u32 = 0x9c70;

/// Blue Creek Apartments W 2F Room `0x26` (corridor): one-shot horror sound (`0x9c6c` = `40044`).
pub const SE_APART_W2F_CORRIDOR: u32 = 0x9c6c;

/// Wood Side Apartments E 3F East Room `0x1f` (corridor): walking footsteps moving along `-X` (`0x9c51` = `40017`).
pub const SE_APART_E3FE_FOOTSTEPS: u32 = 0x9c51;

/// Wood Side Apartments E 2F `EvProgAnyoneCry`: distant scream (`0x9c49` = `40009`).
pub const SE_APART_ANYONE_CRY: u32 = 0x9c49;

/// Wood Side Apartments E 2F `EvProgMonkeyKick206`: thud inside Room 206 (`0x9c47` = `40007`).
pub const SE_APART_MONKEY_KICK_206: u32 = 0x9c47;

/// Wood Side Apartments E 2F Room `0x18` (Room 205/208): TV static loop (`0x3e80` = `16000`).
pub const SE_APART_ROOM205_TV_STATIC: u32 = 0x3e80;

/// Wood Side Apartments E 1F Room `0x1d`: ambient loop before `flag(85)` (`0x3e90` = `16016`).
pub const SE_APART_E1F_AMBIENT: u32 = 0x3e90;

/// Wood Side Apartments Staircase Room `0x21`: water draining sound (`0x2716` = `10006`).
pub const SE_APART_STAIR_WATER_DRAIN: u32 = 0x2716;

// ============================================================================
// Stage & Flag Constants
// ============================================================================

/// Flag `648` (`0x1f7aa28` bit 8): Player knocked on the locked stall door in Toluca Prison North Room `0x73`.
pub const FLAG_PRISON_BATHROOM_KNOCKED: usize = 648;
/// Flag `649` (`0x1f7aa28` bit 9): Toluca Prison North Room `0x73` bathroom stall jumpscare has triggered.
pub const FLAG_PRISON_BATHROOM_JUMPSCARE_DONE: usize = 649;
/// Flag `283` (`0x1f7a9fc` bit 27): Toluca Prison North Room `0x66` gate/wall state.
pub const FLAG_PRISON_N_ROOM66_WALL: usize = 283;
/// Flag `288` (`0x1f7aa00` bit 0): Toluca Prison South Oppressor cell bars active.
pub const FLAG_PRISON_S_BARS_ACTIVE: usize = 288;
/// Flag `289` (`0x1f7aa00` bit 1): Toluca Prison South Oppressor cell bars timer running.
pub const FLAG_PRISON_S_BARS_COUNTING: usize = 289;
/// Flag `290` (`0x1f7aa00` bit 2): Toluca Prison South Oppressor cell bars `+10.0s` boost.
pub const FLAG_PRISON_S_BARS_BOOST: usize = 290;
/// Flag `309` (`0x1f7aa04` bit 21): Toluca Prison Basement Room `0x77` wall/model state.
pub const FLAG_PRISON_BF_ROOM77_OPEN: usize = 309;
/// Flag `551` (`0x1f7aa24` bit 7): Historical Society Room `0x5c` sound trigger fired (70% pre-set on `FlagInit`).
pub const FLAG_SOCIETY_ROOM5C_TRIGGERED: usize = 551;
/// Flag `547` (`0x1f7aa24` bit 3): Blue Creek Apartments Room 209 (`0x28`) whisper triggered (50% pre-set on `FlagInit`).
pub const FLAG_APART_ROOM209_WHISPER_DONE: usize = 547;
/// Flag `552` (`0x1f7aa24` bit 8): Wood Side Apartments E 3F East Room `0x1f` footsteps triggered.
pub const FLAG_APART_E3FE_FOOTSTEPS_DONE: usize = 552;
/// Flag `553` (`0x1f7aa24` bit 9): Blue Creek Apartments W 2F Room `0x26` corridor sound triggered.
pub const FLAG_APART_W2F_CORRIDOR_DONE: usize = 553;

/// Stage identifier matching the 58 `Stage_Data` entries in `sh2pc.exe` / `SLUS_202.28`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum StageId {
    ApartE1f = 6,
    ApartE2f = 7,
    ApartE3fw = 8,
    ApartE3fe = 9,
    ApartW1f = 10,
    ApartW2f = 11,
    ApartStair = 12,
    ApartOut = 13,
    Society = 28,
    PrisonN = 31,
    PrisonS = 32,
    PrisonBf = 33,
}

impl StageId {
    pub const fn from_raw(raw: u8) -> Option<Self> {
        match raw {
            6 => Some(Self::ApartE1f),
            7 => Some(Self::ApartE2f),
            8 => Some(Self::ApartE3fw),
            9 => Some(Self::ApartE3fe),
            10 => Some(Self::ApartW1f),
            11 => Some(Self::ApartW2f),
            12 => Some(Self::ApartStair),
            13 => Some(Self::ApartOut),
            28 => Some(Self::Society),
            31 => Some(Self::PrisonN),
            32 => Some(Self::PrisonS),
            33 => Some(Self::PrisonBf),
            _ => None,
        }
    }
}

/// 3D / 2D audio command emitted by stage environmental horror triggers.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SpatialSoundCommand {
    /// `SeCallPos(se_id, volume, &pos, channel)` (`0x00192270` / `0x516e90`): starts a 3D positional sound.
    CallPos {
        se_id: u32,
        volume: f32,
        pos: [f32; 3],
        channel: u8,
    },
    /// `SePosMove(se_id, volume, &pos)` (`0x515910`): moves an active 3D sound emitter in world space.
    PosMove {
        se_id: u32,
        volume: f32,
        pos: [f32; 3],
    },
    /// `SePosSet(se_id, volume, &pos, channel)` (`0x515740`): updates position/volume of a looping emitter.
    PosSet {
        se_id: u32,
        volume: f32,
        pos: [f32; 3],
        channel: u8,
    },
    /// `SeVolume(se_id, volume)` (`0x516d50`): updates volume of an active sound effect.
    SetVolume { se_id: u32, volume: f32 },
    /// `SeCall(se_id)` (`0x001921c0` / `0x515f10`): triggers a 2D non-positional sound effect.
    Call2D { se_id: u32, volume: f32 },
}

/// Dynamic collision wall/floor activation command (`clChangeActWall` / `clChangeActFloor` at `0x522c90` / `0x522cc0`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollisionWallCommand {
    /// Toluca Prison North Room `0x66`: `clActWallList_ps45` (`0x8d2b6c`).
    PrisonNPs45Wall,
    /// Toluca Prison Basement Room `0x77`: `clActWallList_ps57` (`0x8d1b28`).
    PrisonBfPs57Wall,
    /// Wood Side Apartments E 2F Room `0x18`: `clActWallList_ap42` (`flag(69)` active state).
    ApartE2fAp42Wall { corpse_present: bool },
    /// Wood Side Apartments E 2F Room `0x18`: `clActFloorList_ap42` (`flag(69)` active state).
    ApartE2fAp42Floor { corpse_present: bool },
    /// Wood Side Apartments E 2F Room `0x18`: `clActWallList_ap52` (`flag(82)` clock pushed state).
    ApartE2fAp52Wall { clock_pushed: bool },
    /// Wood Side Apartments E 1F Room `0x1e`: `clActWallList_e1f_1e` (`0x8e36d0`).
    ApartE1fRoom1eWall,
    /// Wood Side Apartments Staircase Room `0x21`: `clActWallList_stair_water` (`0x8df078`).
    ApartStairWaterWall,
}

/// Stage model visibility/animation state override (`ModelStateSet` at `0x4a2f10`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelStateCommand {
    pub model_id: u8,
    pub state: i8,
}

/// Camera lock/release command (`Event_CamSet` at `0x51d280` / `vcReleaseLockCam` at `0x51d520`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CameraOverrideCommand {
    Lock(EventCamSetData),
    Release,
}

/// Per-frame context passed to [`StageEventDispatcher`].
#[derive(Debug, Clone, Copy)]
pub struct StageHookContext {
    pub stage: StageId,
    pub room_id: u8,
    pub dt: f32,
    pub player_pos: [f32; 3],
    pub player_rot_y: f32,
    /// Bitmask or list of currently playing sound effect IDs (queried by `SeCheck` at `0x515890`).
    pub active_sounds: [u32; 4],
}

impl StageHookContext {
    /// Returns `true` if `se_id` is currently playing (`SeCheck(se_id)`).
    #[inline]
    pub fn is_se_playing(&self, se_id: u32) -> bool {
        self.active_sounds.contains(&se_id)
    }
}

/// Commands and state changes produced by a stage lifecycle hook (`EvStageInit`, `EvRoomInit`,
/// `EvAllTimeFunc`, `EvSoundCallAfterLoad`).
#[derive(Debug, Clone, Default)]
pub struct StageHookOutput {
    pub sounds: Vec<SpatialSoundCommand>,
    pub collision_commands: Vec<CollisionWallCommand>,
    pub model_states: Vec<ModelStateCommand>,
    pub camera_override: Option<CameraOverrideCommand>,
}

/// Stage-level runtime state for all environmental horror triggers (`Stage_Data` lifecycle hooks).
#[derive(Debug, Clone)]
pub struct StageEventDispatcher {
    /// Toluca Prison North Room `0x73` randomized Z threshold (`[-20000.0, -19200.0]`, `0x1d83668`).
    pub prison_n_bathroom_z_threshold: f32,
    /// Toluca Prison South Oppressor cell bars timer (`0x1d83654`).
    pub prison_s_bars_timer: f32,
    /// Toluca Prison South Room `0x72` invisible prisoner chant countdown timer (`0x1d8365c`).
    pub prison_s_prisoner_timer: f32,
    /// Toluca Prison South Room `0x72` invisible prisoner current 3D emitter position (`0x8d1e74`).
    pub prison_s_prisoner_pos: [f32; 3],
    /// Historical Society Room `0x5d` descending staircase sound countdown timer (`0x1d836bc`).
    pub society_staircase_timer: f32,
    /// Blue Creek Apartments Room `0x28` (Room 209) randomized X threshold (`[20000.0, 21200.0]`, `0x1d83a8c`).
    pub apart_w2f_room209_x_threshold: f32,
    /// Blue Creek Apartments Room `0x28` (Room 209) orbiting whisper local angle in radians (`0x1d83a90`).
    pub apart_w2f_whisper_angle: f32,
    /// Blue Creek Apartments Room `0x26` corridor trigger armed flag (`0x1d83a94`).
    pub apart_w2f_corridor_armed: bool,
    /// Wood Side Apartments E 3F East Room `0x1f` corridor footsteps elapsed time (`0x1d83ab4`).
    pub apart_e3fe_footstep_timer: f32,
    /// Wood Side Apartments E 3F East Room `0x13` camera lock active flag (`0x1d83ab0`).
    pub apart_e3fe_cam_active: bool,
}

impl Default for StageEventDispatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl StageEventDispatcher {
    pub const fn new() -> Self {
        Self {
            prison_n_bathroom_z_threshold: -20_000.0,
            prison_s_bars_timer: 0.0,
            prison_s_prisoner_timer: 10.0,
            prison_s_prisoner_pos: [-140_000.0, -500.0, 20_000.0],
            society_staircase_timer: 2.0,
            apart_w2f_room209_x_threshold: 20_600.0,
            apart_w2f_whisper_angle: FRAC_PI_2,
            apart_w2f_corridor_armed: false,
            apart_e3fe_footstep_timer: 0.0,
            apart_e3fe_cam_active: false,
        }
    }

    /// Executes `Stage_Data::EvStageInit` when a stage is loaded.
    pub fn on_stage_init(&mut self, stage: StageId, flags: &mut EventFlags) {
        match stage {
            StageId::ApartE3fe => {
                // 0x5992f0: resets Room 0x13 camera lock flag
                self.apart_e3fe_cam_active = false;
                self.apart_e3fe_footstep_timer = 0.0;
            }
            StageId::ApartE3fw => {
                // 0x599a60: sets flag(68) (0x1f7a9e8 |= 0x10)
                flags.set(68, true);
            }
            StageId::ApartOut => {
                // 0x595e80: clears flag(87) (0x1f7a9e8 &= ~0x800000)
                flags.set(87, false);
            }
            _ => {}
        }
    }

    /// Executes `Stage_Data::EvRoomInit` when entering `room_id` inside `stage`.
    pub fn on_room_init(
        &mut self,
        stage: StageId,
        room_id: u8,
        flags: &mut EventFlags,
        rng: &mut Sh2Rng,
    ) {
        match stage {
            StageId::PrisonN => {
                // 0x5859a0: Room 0x73 (women's bathroom)
                if room_id == 0x73 {
                    self.prison_n_bathroom_z_threshold = rng.next_f32() * 800.0 - 20_000.0;
                    // 70% chance (frand() > 0.3) to clear flags 648 & 649 so stall can be knocked again
                    if rng.next_f32() > 0.3 {
                        flags.set(FLAG_PRISON_BATHROOM_KNOCKED, false);
                        flags.set(FLAG_PRISON_BATHROOM_JUMPSCARE_DONE, false);
                    }
                }
            }
            StageId::PrisonS => {
                // 0x5845f0: Room 0x72 (southern cellblock invisible prisoner)
                if room_id == 0x72 {
                    self.prison_s_prisoner_timer = rng.next_f32() * 5.0 + 10.0;
                }
            }
            StageId::Society => {
                // 0x587a50: initializes descending staircase sound timer to (frand() + 1.0) * 2.0
                self.society_staircase_timer = (rng.next_f32() + 1.0) * 2.0;
            }
            StageId::ApartW2f => {
                // 0x5972d0: Room 0x28 (Room 209) sets whisper X threshold = 21200.0 - frand() * 1200.0
                if room_id == 0x28 {
                    self.apart_w2f_room209_x_threshold = 21_200.0 - rng.next_f32() * 1200.0;
                }
                self.apart_w2f_corridor_armed = false;
            }
            StageId::ApartStair => {
                // 0x596480: clears flag(87)
                flags.set(87, false);
            }
            _ => {}
        }
    }

    /// Executes `Stage_Data::EvSoundCallAfterLoad` after room/stage assets finish loading.
    pub fn on_sound_call_after_load(
        &mut self,
        stage: StageId,
        room_id: u8,
        flags: &mut EventFlags,
    ) -> StageHookOutput {
        let mut out = StageHookOutput::default();
        match stage {
            // 0x59ba80: Room 0x18 (Room 205/208 corpse in front of TV)
            StageId::ApartE2f if room_id == 0x18 && flags.get(69) => {
                out.sounds.push(SpatialSoundCommand::CallPos {
                    se_id: SE_APART_ROOM205_TV_STATIC,
                    volume: 0.3,
                    pos: [-58_082.375, -356.5, 19_011.39],
                    channel: 12,
                });
                flags.set(70, true);
            }
            // 0x59cfe0: Room 0x1d ambient loop before flag(85)
            StageId::ApartE1f if room_id == 0x1d && !flags.get(85) => {
                out.sounds.push(SpatialSoundCommand::CallPos {
                    se_id: SE_APART_E1F_AMBIENT,
                    volume: 0.8,
                    pos: [-21_400.0, -500.0, 18_200.0],
                    channel: 8,
                });
            }
            _ => {}
        }
        out
    }

    /// Executes `Stage_Data::EvAllTimeFunc` every frame for the active stage and room.
    pub fn on_all_time(
        &mut self,
        ctx: &StageHookContext,
        flags: &mut EventFlags,
        rng: &mut Sh2Rng,
    ) -> StageHookOutput {
        let mut out = StageHookOutput::default();
        match ctx.stage {
            StageId::PrisonN => self.tick_prison_n(ctx, flags, rng, &mut out),
            StageId::PrisonS => self.tick_prison_s(ctx, flags, rng, &mut out),
            StageId::PrisonBf => self.tick_prison_bf(ctx, flags, &mut out),
            StageId::Society => self.tick_society(ctx, flags, rng, &mut out),
            StageId::ApartW2f => self.tick_apart_w2f(ctx, flags, rng, &mut out),
            StageId::ApartE3fe => self.tick_apart_e3fe(ctx, flags, &mut out),
            StageId::ApartE2f => self.tick_apart_e2f(ctx, flags, &mut out),
            StageId::ApartE1f => self.tick_apart_e1f(ctx, flags, &mut out),
            StageId::ApartStair => self.tick_apart_stair(ctx, flags, &mut out),
            StageId::ApartOut => self.tick_apart_out(ctx, flags, &mut out),
            StageId::ApartE3fw | StageId::ApartW1f => {}
        }
        out
    }
}
