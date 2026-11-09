//! Toluca Prison (`stg_prison_n.c`, `stg_prison_s.c`, `stg_prison_bf.c`) and Silent Hill
//! Historical Society (`stg_society.c`) environmental horror triggers (`EvAllTimeFunc`).

use super::super::{EventFlags, Sh2Rng};
use super::{
    CollisionWallCommand, ModelStateCommand, SpatialSoundCommand, StageEventDispatcher,
    StageHookContext, StageHookOutput, FLAG_PRISON_BATHROOM_JUMPSCARE_DONE,
    FLAG_PRISON_BATHROOM_KNOCKED, FLAG_PRISON_BF_ROOM77_OPEN, FLAG_PRISON_N_ROOM66_WALL,
    FLAG_PRISON_S_BARS_ACTIVE, FLAG_PRISON_S_BARS_BOOST, FLAG_PRISON_S_BARS_COUNTING,
    FLAG_SOCIETY_ROOM5C_TRIGGERED, SE_PRISON_BATHROOM_STALL_BANG, SE_PRISON_INVISIBLE_PRISONER,
    SE_PRISON_ROOM68_MECHANISM, SE_SOCIETY_ROOM_5C_HORROR, SE_SOCIETY_STAIRCASE_AMBIENT,
};

impl StageEventDispatcher {
    pub(crate) fn tick_prison_n(
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

    pub(crate) fn tick_prison_s(
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

    pub(crate) fn tick_prison_bf(
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

    pub(crate) fn tick_society(
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
}
