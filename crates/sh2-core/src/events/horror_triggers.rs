//! Scripted Environmental Horror Triggers & PS2 `Stage_Data` Lifecycle Dispatcher
//! (`EvStageInit`, `EvRoomInit`, `EvAllTimeFunc`, `EvSoundCallAfterLoad`, `EvBgmControl`).
//!
//! Verified 1:1 against the PS2 prototype (`SLUS_202.28` + `/GX/*.BIN`) and retail PC binary
//! (`sh2pc.exe`):
//! - Toluca Prison North (`stg_prison_n.c`, Stage 31): Room `0x73` women's bathroom stall knock &
//!   jumpscare (`SE 0x427d`, `flag(648)`, `flag(649)`), Room `0x66` (`flag(283)`, `clActWallList_ps45`,
//!   model `0x2d`), Room `0x67` (`flag(28)`, model `0x13`).
//! - Toluca Prison South (`stg_prison_s.c`, Stage 32): Room `0x72` southern cellblock invisible
//!   prisoner chanting `"Ritual..."` (`SE 0x4682` with randomized 3D cellblock position & timer),
//!   Room `0x68` (`SE 0x70d` controlling model `0x1d`), Oppressor cell bars timer (`flag(288..=290)`).
//! - Toluca Prison Basement (`stg_prison_bf.c`, Stage 33): Room `0x77` (`flag(309)`, `clActWallList_ps57`,
//!   model `0x39`).
//! - Silent Hill Historical Society (`stg_society.c`, Stage 28): Room `0x5d` long descending
//!   staircase periodic eerie sound (`SE 0x9c73` with depth-attenuated volume and random 3D offset),
//!   Room `0x5c` (`!flag(551) && px > -19600.0` triggers `SE 0x4982` at `(-22000, -500, -18900)`).
//! - Wood Side & Blue Creek Apartments (Stages 6–13):
//!   - Blue Creek W 2F (`stg_apart_w2f.c`, Stage 11): Room `0x28` (Room 209) whisper `"See?"`
//!     (`SE 0x9c70`, `flag(547)`, orbiting 3D position at `r = 2500.0` around the player while playing),
//!     Room `0x26` corridor horror sound (`SE 0x9c6c`, `flag(553)`), Room `0x27` models `0x41` & `0x43`.
//!   - Wood Side E 3F East (`stg_apart_e3fe.c`, Stage 9): Room `0x1f` walking corridor footsteps
//!     (`SE 0x9c51`, `flag(552)`, moving along `-X` at `700.0` units/sec while playing), Room `0x13`
//!     key behind bars (`flag(91)`, `flag(89)`, model `0x13` & `Event_CamSetData`).
//!   - Wood Side E 2F (`stg_apart_e2f.c`, Stage 7): `EvProgAnyoneCry` (`SE 0x9c49`, stage anim `0x4e22`,
//!     2.0s timer, message 0 `"What was that!?"`), `EvProgMonkeyKick206` (`SE 0x9c47` unless `flag(69)`),
//!     Room `0x18` TV static (`SE 0x3e80`) & dynamic collision walls/floors (`clActWallList_ap42`).

use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, PI};

use crate::types::{wrap_angle, EventCamSetData};

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

/// Wood Side Apartments E 3F East Room `0x13` fixed camera (`e3fe_cam` at `0x8e0fa4..0x8e0fd4`).
pub const APART_E3FE_ROOM13_CAMERA: EventCamSetData = EventCamSetData {
    flag: 1,
    pad: [0; 3],
    cam_pos: [-21_966.0, -560.0, -101_420.0, 1.0],
    cam_watch: [-22_250.0, -50.0, -101_430.0, 1.0],
    fov: 70.0,
    roll: 0.0,
    reserve: [0.0; 2],
};

/// Coroutine state for `EvProgAnyoneCry` (`stg_apart_e2f.c` at `0x59b1f0`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AnyoneCryProgramState {
    /// `ev_s_step` (`0`, `27` / `0x1b`, or `13` / `0x0d`).
    pub s_step: i32,
    /// `ev_timer` (`0x011263d4`): waits until `>= 2.0` seconds before showing stage message `0`.
    pub timer: f32,
    /// Whether the player is locked (`JmsEventMode(player, 1)`).
    pub player_locked: bool,
    /// Stage animation requested (`0x4e22` = `20002`).
    pub stage_anim: Option<u16>,
    /// Stage message ID requested (`0` = `"What was that!?"`).
    pub stage_message: Option<u16>,
}

impl Default for AnyoneCryProgramState {
    fn default() -> Self {
        Self::new()
    }
}

impl AnyoneCryProgramState {
    pub const EMITTER_POS: [f32; 3] = [-99_400.0, -500.0, 26_400.0];
    pub const STAGE_ANIM_ID: u16 = 0x4e22;

    pub const fn new() -> Self {
        Self {
            s_step: 0,
            timer: 0.0,
            player_locked: false,
            stage_anim: None,
            stage_message: None,
        }
    }

    /// Advances `EvProgAnyoneCry` (`0x59b1f0`) by one frame (`dt` seconds).
    ///
    /// Returns `(finished, sound_command)`:
    /// - Step `0`: locks player, starts stage anim `0x4e22` (`20002`), resets timer to `0.0`,
    ///   emits `SeCallPos(0x9c49, 1.0, [-99400.0, -500.0, 26400.0], 6)`, advances to step `27` (`0x1b`).
    /// - Step `27` (`0x1b`): emits `SePosMove(0x9c49, 1.0, [-99400.0, -500.0, 26400.0])`, advances
    ///   `timer += dt`, displays stage message `0` once `timer >= 2.0`, and when both `anim_done`
    ///   and `msg_dismissed` are true, advances to step `13` (`0x0d`).
    /// - Step `13` (`0x0d`): unlocks player and returns `true`.
    pub fn tick(
        &mut self,
        dt: f32,
        anim_done: bool,
        msg_dismissed: bool,
    ) -> (bool, Option<SpatialSoundCommand>) {
        match self.s_step {
            0 => {
                self.player_locked = true;
                self.stage_anim = Some(Self::STAGE_ANIM_ID);
                self.timer = 0.0;
                self.s_step = 0x1b;
                (
                    false,
                    Some(SpatialSoundCommand::CallPos {
                        se_id: SE_APART_ANYONE_CRY,
                        volume: 1.0,
                        pos: Self::EMITTER_POS,
                        channel: 6,
                    }),
                )
            }
            0x1b => {
                self.timer += dt;
                if self.timer >= 2.0 {
                    self.stage_message = Some(0);
                }
                if anim_done && self.timer >= 2.0 && msg_dismissed {
                    self.stage_message = None;
                    self.s_step = 0x0d;
                }
                (
                    false,
                    Some(SpatialSoundCommand::PosMove {
                        se_id: SE_APART_ANYONE_CRY,
                        volume: 1.0,
                        pos: Self::EMITTER_POS,
                    }),
                )
            }
            0x0d => {
                self.player_locked = false;
                (true, None)
            }
            _ => (true, None),
        }
    }
}

/// Evaluates `EvProgMonkeyKick206` (`stg_apart_e2f.c` at `0x59b980`):
/// if `!flag(69)` (before getting the flashlight in Room 205), emits `SeCallPos(0x9c47, 1.0, [-99000.0, -500.0, 13600.0], 2)`.
pub fn eval_monkey_kick_206(flags: &EventFlags) -> Option<SpatialSoundCommand> {
    if !flags.get(69) {
        Some(SpatialSoundCommand::CallPos {
            se_id: SE_APART_MONKEY_KICK_206,
            volume: 1.0,
            pos: [-99_000.0, -500.0, 13_600.0],
            channel: 2,
        })
    } else {
        None
    }
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

    fn tick_prison_n(
        &mut self,
        ctx: &StageHookContext,
        flags: &mut EventFlags,
        rng: &mut Sh2Rng,
        out: &mut StageHookOutput,
    ) {
        match ctx.room_id {
            // 0x5859f6..0x585a47: Room 0x66
            0x66 => {
                if flags.get(FLAG_PRISON_N_ROOM66_WALL) {
                    out.collision_commands
                        .push(CollisionWallCommand::PrisonNPs45Wall);
                    out.model_states.push(ModelStateCommand {
                        model_id: 0x2d,
                        state: 0,
                    });
                } else {
                    out.model_states.push(ModelStateCommand {
                        model_id: 0x2d,
                        state: 1,
                    });
                }
            }
            // 0x585a49..0x585a77: Room 0x67
            0x67 => {
                let state = if flags.get(28) { -1 } else { 0 };
                out.model_states.push(ModelStateCommand {
                    model_id: 0x13,
                    state,
                });
            }
            // 0x585a79..0x585b13: Room 0x73 (women's bathroom stall knock & jumpscare)
            0x73 if flags.get(FLAG_PRISON_BATHROOM_KNOCKED)
                && !flags.get(FLAG_PRISON_BATHROOM_JUMPSCARE_DONE)
                && ctx.player_pos[2] > self.prison_n_bathroom_z_threshold =>
            {
                if rng.next_f32() > 0.4 {
                    out.sounds.push(SpatialSoundCommand::CallPos {
                        se_id: SE_PRISON_BATHROOM_STALL_BANG,
                        volume: 1.0,
                        pos: [-138_700.0, -50.0, -21_000.0],
                        channel: 0,
                    });
                    flags.set(FLAG_PRISON_BATHROOM_JUMPSCARE_DONE, true);
                } else {
                    flags.set(FLAG_PRISON_BATHROOM_KNOCKED, false);
                }
            }
            _ => {}
        }
    }

    fn tick_prison_s(
        &mut self,
        ctx: &StageHookContext,
        flags: &mut EventFlags,
        rng: &mut Sh2Rng,
        out: &mut StageHookOutput,
    ) {
        // 0x58462c..0x584688: Oppressor cell bars timer
        if flags.get(FLAG_PRISON_S_BARS_ACTIVE) && flags.get(FLAG_PRISON_S_BARS_COUNTING) {
            self.prison_s_bars_timer += ctx.dt;
            if flags.get(FLAG_PRISON_S_BARS_BOOST) {
                self.prison_s_bars_timer += 10.0;
                flags.set(FLAG_PRISON_S_BARS_BOOST, false);
            }
            if self.prison_s_bars_timer > 20.0 {
                flags.set(FLAG_PRISON_S_BARS_ACTIVE, false);
            }
        } else {
            self.prison_s_bars_timer = 0.0;
        }

        match ctx.room_id {
            // 0x584691..0x5846be: Room 0x68
            0x68 => {
                let state = if ctx.is_se_playing(SE_PRISON_ROOM68_MECHANISM) {
                    -1
                } else {
                    0
                };
                out.model_states.push(ModelStateCommand {
                    model_id: 0x1d,
                    state,
                });
            }
            // 0x5846c0..0x58479c: Room 0x72 (southern cellblock invisible prisoner chanting "Ritual...")
            0x72 => {
                self.prison_s_prisoner_timer -= ctx.dt;
                out.sounds.push(SpatialSoundCommand::PosSet {
                    se_id: SE_PRISON_INVISIBLE_PRISONER,
                    volume: 1.0,
                    pos: self.prison_s_prisoner_pos,
                    channel: 0,
                });
                if self.prison_s_prisoner_timer <= 0.0 {
                    let x = rng.next_f32() * 12_000.0 - 146_000.0;
                    let z = rng.next_f32() * 21_600.0 + 10_400.0;
                    // Preserve exact LCG advancement from 0x584705..0x58472e
                    let _ = rng.next_i32();
                    let _ = rng.next_f32();
                    self.prison_s_prisoner_pos = [x, -500.0, z];
                    self.prison_s_prisoner_timer = rng.next_f32() * 10.0 + 5.0;
                    out.sounds.push(SpatialSoundCommand::CallPos {
                        se_id: SE_PRISON_INVISIBLE_PRISONER,
                        volume: 1.0,
                        pos: self.prison_s_prisoner_pos,
                        channel: 0,
                    });
                }
            }
            _ => {}
        }
    }

    fn tick_prison_bf(
        &mut self,
        ctx: &StageHookContext,
        flags: &mut EventFlags,
        out: &mut StageHookOutput,
    ) {
        // 0x5841f0..0x58422d: Room 0x77
        if ctx.room_id == 0x77 {
            if flags.get(FLAG_PRISON_BF_ROOM77_OPEN) {
                out.model_states.push(ModelStateCommand {
                    model_id: 0x39,
                    state: -1,
                });
            } else {
                out.collision_commands
                    .push(CollisionWallCommand::PrisonBfPs57Wall);
                out.model_states.push(ModelStateCommand {
                    model_id: 0x39,
                    state: 0,
                });
            }
        }
    }

    fn tick_society(
        &mut self,
        ctx: &StageHookContext,
        flags: &mut EventFlags,
        rng: &mut Sh2Rng,
        out: &mut StageHookOutput,
    ) {
        match ctx.room_id {
            // 0x587a78..0x587b8d: Room 0x5d (long descending staircase)
            0x5d => {
                self.society_staircase_timer -= ctx.dt;
                if self.society_staircase_timer <= 0.0 {
                    let x = rng.next_f32() * 2500.0 - 11_520.0 - 1250.0;
                    let y = 42_670.0;
                    let z = rng.next_f32() * 2500.0 + 40_151.7 - 1250.0;
                    let raw_vol = 1.0 - (35_000.0 - ctx.player_pos[1]) * (1.0 / 35_000.0);
                    let vol = raw_vol.clamp(0.2, 1.0);
                    out.sounds.push(SpatialSoundCommand::CallPos {
                        se_id: SE_SOCIETY_STAIRCASE_AMBIENT,
                        volume: vol * 0.5,
                        pos: [x, y, z],
                        channel: 6,
                    });
                    self.society_staircase_timer =
                        ((1.0 - vol) * 12.5 + 2.0) * rng.next_f32() + 1.5;
                }
            }
            // 0x587b8e..0x587bda: Room 0x5c
            0x5c if !flags.get(FLAG_SOCIETY_ROOM5C_TRIGGERED) && ctx.player_pos[0] > -19_600.0 => {
                out.sounds.push(SpatialSoundCommand::CallPos {
                    se_id: SE_SOCIETY_ROOM_5C_HORROR,
                    volume: 1.0,
                    pos: [-22_000.0, -500.0, -18_900.0],
                    channel: 8,
                });
                flags.set(FLAG_SOCIETY_ROOM5C_TRIGGERED, true);
            }
            _ => {}
        }
    }

    fn tick_apart_w2f(
        &mut self,
        ctx: &StageHookContext,
        flags: &mut EventFlags,
        rng: &mut Sh2Rng,
        out: &mut StageHookOutput,
    ) {
        if ctx.room_id == 0x27 {
            // 0x597313..0x597366: Room 0x27 models 0x41 and 0x43
            let state_41 = if flags.get(102) { 0 } else { -1 };
            let state_43 = if flags.get(108) { 0 } else { 1 };
            out.model_states.push(ModelStateCommand {
                model_id: 0x41,
                state: state_41,
            });
            out.model_states.push(ModelStateCommand {
                model_id: 0x43,
                state: state_43,
            });
        }

        match ctx.room_id {
            // 0x597376..0x5974a2: Room 0x28 (Room 209 whisper "See?")
            0x28 => {
                let mut just_started = false;
                if !flags.get(FLAG_APART_ROOM209_WHISPER_DONE)
                    && ctx.player_pos[0] < self.apart_w2f_room209_x_threshold
                {
                    self.apart_w2f_whisper_angle = rng.next_f32() * FRAC_PI_4 + FRAC_PI_2;
                    out.sounds.push(SpatialSoundCommand::CallPos {
                        se_id: SE_APART_ROOM209_WHISPER,
                        volume: 1.0,
                        pos: [0.0, 0.0, 0.0],
                        channel: 6,
                    });
                    flags.set(FLAG_APART_ROOM209_WHISPER_DONE, true);
                    just_started = true;
                }

                // While SE 0x9c70 is playing (including the frame it starts), orbit 2500.0 units around the player
                if just_started || ctx.is_se_playing(SE_APART_ROOM209_WHISPER) {
                    self.apart_w2f_whisper_angle =
                        wrap_angle(self.apart_w2f_whisper_angle + ctx.dt * (PI / 8.0));
                    let world_angle = self.apart_w2f_whisper_angle + ctx.player_rot_y;
                    let pos = [
                        ctx.player_pos[0] + world_angle.sin() * 2500.0,
                        ctx.player_pos[1],
                        ctx.player_pos[2] + world_angle.cos() * 2500.0,
                    ];
                    out.sounds.push(SpatialSoundCommand::PosMove {
                        se_id: SE_APART_ROOM209_WHISPER,
                        volume: 1.0,
                        pos,
                    });
                }
            }
            // 0x5974a3..0x597532: Room 0x26 (W2F corridor horror sound)
            0x26 => {
                if self.apart_w2f_corridor_armed {
                    if ctx.player_pos[2] < -6800.0
                        && !flags.get(FLAG_APART_W2F_CORRIDOR_DONE)
                        && !flags.get(114)
                    {
                        flags.set(FLAG_APART_W2F_CORRIDOR_DONE, true);
                        out.sounds.push(SpatialSoundCommand::CallPos {
                            se_id: SE_APART_W2F_CORRIDOR,
                            volume: 1.0,
                            pos: [100_807.1, -500.0, -6053.4854],
                            channel: 2,
                        });
                    }
                } else if ctx.player_pos[2] > -6500.0 {
                    self.apart_w2f_corridor_armed = true;
                }
            }
            _ => {}
        }
    }

    fn tick_apart_e3fe(
        &mut self,
        ctx: &StageHookContext,
        flags: &mut EventFlags,
        out: &mut StageHookOutput,
    ) {
        match ctx.room_id {
            // 0x599339..0x599414: Room 0x1f (3F East Corridor walking footsteps)
            0x1f => {
                if !flags.get(FLAG_APART_E3FE_FOOTSTEPS_DONE) && ctx.player_pos[2] < 17_000.0 {
                    out.sounds.push(SpatialSoundCommand::CallPos {
                        se_id: SE_APART_E3FE_FOOTSTEPS,
                        volume: 1.0,
                        pos: [60_000.0, 0.0, 11_350.0],
                        channel: 2,
                    });
                    flags.set(FLAG_APART_E3FE_FOOTSTEPS_DONE, true);
                } else if ctx.is_se_playing(SE_APART_E3FE_FOOTSTEPS) {
                    self.apart_e3fe_footstep_timer += ctx.dt;
                    let x = 60_000.0 - self.apart_e3fe_footstep_timer * 700.0;
                    out.sounds.push(SpatialSoundCommand::PosMove {
                        se_id: SE_APART_E3FE_FOOTSTEPS,
                        volume: 1.0,
                        pos: [x, 0.0, 11_350.0],
                    });
                }
            }
            // 0x599415..0x5994fd: Room 0x13 (key on floor behind bars)
            0x13 => {
                if flags.get(91) {
                    out.model_states.push(ModelStateCommand {
                        model_id: 0x13,
                        state: 0,
                    });
                    if ctx.player_pos[0] < -21_600.0
                        && ctx.player_pos[2] < -101_150.0
                        && !flags.get(89)
                    {
                        self.apart_e3fe_cam_active = true;
                        out.camera_override =
                            Some(CameraOverrideCommand::Lock(APART_E3FE_ROOM13_CAMERA));
                    } else if self.apart_e3fe_cam_active {
                        self.apart_e3fe_cam_active = false;
                        out.camera_override = Some(CameraOverrideCommand::Release);
                    }
                } else {
                    out.model_states.push(ModelStateCommand {
                        model_id: 0x13,
                        state: -1,
                    });
                }
            }
            _ => {}
        }
    }

    fn tick_apart_e2f(
        &mut self,
        ctx: &StageHookContext,
        flags: &mut EventFlags,
        out: &mut StageHookOutput,
    ) {
        match ctx.room_id {
            // 0x59bae1..0x59bc41: Room 0x17
            0x17 => {
                let visible = flags.get(39) || flags.get(40) || flags.get(41);
                out.model_states.push(ModelStateCommand {
                    model_id: 0x20,
                    state: if visible { 0 } else { -1 },
                });
            }
            // 0x59bc42..0x59be9d: Room 0x18 (Room 205/208 corpse in front of TV & clock wall)
            0x18 => {
                let corpse_present = flags.get(69);
                out.collision_commands
                    .push(CollisionWallCommand::ApartE2fAp42Wall { corpse_present });
                out.collision_commands
                    .push(CollisionWallCommand::ApartE2fAp42Floor { corpse_present });
                out.collision_commands
                    .push(CollisionWallCommand::ApartE2fAp52Wall {
                        clock_pushed: flags.get(82),
                    });

                out.sounds.push(SpatialSoundCommand::PosSet {
                    se_id: SE_APART_ROOM205_TV_STATIC,
                    volume: 0.3,
                    pos: [-58_082.375, -356.5, 19_011.39],
                    channel: 12,
                });
            }
            _ => {}
        }
    }

    fn tick_apart_e1f(
        &mut self,
        ctx: &StageHookContext,
        flags: &mut EventFlags,
        out: &mut StageHookOutput,
    ) {
        // 0x59d010..0x59d02e: Room 0x1e
        if ctx.room_id == 0x1e && !flags.get(88) {
            out.collision_commands
                .push(CollisionWallCommand::ApartE1fRoom1eWall);
        }
    }

    fn tick_apart_stair(
        &mut self,
        ctx: &StageHookContext,
        flags: &mut EventFlags,
        out: &mut StageHookOutput,
    ) {
        match ctx.room_id {
            0x10 => {
                out.model_states.push(ModelStateCommand {
                    model_id: 0x19,
                    state: if flags.get(57) { -1 } else { 0 },
                });
            }
            0x20 => {
                out.model_states.push(ModelStateCommand {
                    model_id: 0x6e,
                    state: if flags.get(58) { -1 } else { 0 },
                });
            }
            0x21 => {
                if !flags.get(148) {
                    out.collision_commands
                        .push(CollisionWallCommand::ApartStairWaterWall);
                }
                if flags.get(148) && !flags.get(149) {
                    flags.set(149, true);
                    out.sounds.push(SpatialSoundCommand::Call2D {
                        se_id: SE_APART_STAIR_WATER_DRAIN,
                        volume: 1.0,
                    });
                }
            }
            _ => {}
        }
    }

    fn tick_apart_out(
        &mut self,
        ctx: &StageHookContext,
        flags: &mut EventFlags,
        out: &mut StageHookOutput,
    ) {
        // 0x595e90..0x595edd
        if ctx.room_id != 0x07 {
            let state = if flags.get(95) { 2 } else { 1 };
            out.model_states.push(ModelStateCommand {
                model_id: 0x2e,
                state,
            });
        }
    }
}
