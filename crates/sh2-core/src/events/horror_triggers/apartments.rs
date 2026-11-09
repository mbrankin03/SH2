//! Wood Side & Blue Creek Apartments (`stg_apart_*.c`, Stages 6–13) environmental horror triggers,
//! orbiting Room 209 whisper (`SE 0x9c70`), moving 3F corridor footsteps (`SE 0x9c51`),
//! and `EvProgAnyoneCry` / `EvProgMonkeyKick206` coroutines.

use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, PI};

use crate::types::{wrap_angle, EventCamSetData};

use super::super::{EventFlags, Sh2Rng};
use super::{
    CameraOverrideCommand, CollisionWallCommand, ModelStateCommand, SpatialSoundCommand,
    StageEventDispatcher, StageHookContext, StageHookOutput, FLAG_APART_E3FE_FOOTSTEPS_DONE,
    FLAG_APART_ROOM209_WHISPER_DONE, FLAG_APART_W2F_CORRIDOR_DONE, SE_APART_ANYONE_CRY,
    SE_APART_E3FE_FOOTSTEPS, SE_APART_MONKEY_KICK_206, SE_APART_ROOM205_TV_STATIC,
    SE_APART_ROOM209_WHISPER, SE_APART_STAIR_WATER_DRAIN, SE_APART_W2F_CORRIDOR,
};

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

impl StageEventDispatcher {
    pub(crate) fn tick_apart_w2f(
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

    pub(crate) fn tick_apart_e3fe(
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

    pub(crate) fn tick_apart_e2f(
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

    pub(crate) fn tick_apart_e1f(
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

    pub(crate) fn tick_apart_stair(
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

    pub(crate) fn tick_apart_out(
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
