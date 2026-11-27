//! Stage 4 (`stg_forest.c` / Forest Trail & Graveyard `ca11..ca28`) `ev_prog[1..=7]`,
//! `EvRoomInit` (`0x0059fb60`), `EvSoundCallAfterLoad` (`0x0059fc70`), `EvAllTime`
//! (`0x0059fcf0`), and `EvBgmControl` (`0x005a0260`).
//!
//! Disassembled 1:1 from retail `sh2pc.exe` (`0x0059f2b0..0x005a0804`) and cross-checked
//! against E3 PS2 prototype `SLPM_123.45` (`docs/decomp/slpm/Event/stage/stg_forest.c`):
//! - Prototype vs. Retail drift: In `SLPM_123.45`, `EvProgAngelaInGrave` (`0x0022c880`),
//!   `EvProgGraveSureQuiet` (`0x0022c890`), and `EvProgGraveLookingFor` (`0x0022c8a0`)
//!   are 12-byte stubs returning `1`. All full runtime logic below is taken 1:1 from
//!   retail `sh2pc.exe` `ACT1_FOREST_EV_PROGS` (`0x008e7438`) and `Stage_Data` (`0x008e7538`).
//!
//! `ev_prog[1..=7]` dispatch table (`sh2pc.exe` `0x008e7438`):
//! - `[1]` `0x0059f2b0`: Well save point (`data/pic/out/p_redpaper.tex`, stage msg `0`,
//!   `SeCall(0x2743)`, sets `flag(5)`, `SavePointSet(2)`, opens Save Menu `mode = 9`).
//! - `[2]` `0x005a0570`: Angela Orosco graveyard cutscene (`demo_no = 3` `haka_agl.dds`,
//!   voice `0xea95`, pre-pose `(-20.845, 2473.03, -1332.315)` yaw `-3π/4`, post-pose
//!   `(330.845, 2473.03, -1332.315)` yaw `-3π/4`, Angela `0x0107` anim `0x0f3e` at
//!   `(781.1213, 2505.674, -394.2832)` yaw `π`, completion `flag(36)`, 50% RNG `flag(37)`).
//! - `[3]` `0x0059f4a0`: `EvProgGraveSureQuiet` (`demo_no = 4` `haka_agl_h.dds`, voice `0xea96`,
//!   post-pose `(-179.005, 2487.65, -1286.69)` yaw `-1.321038`, re-flips `flag(37)`).
//! - `[4]` `0x0059f5c0`: `EvProgGraveLookingFor` (`demo_no = 4` `haka_agl_i.dds`, voice `0xea97`,
//!   post-pose `(-493.0, 2510.19, -1093.55)` yaw `-1.727716`, re-flips `flag(37)`).
//! - `[5]` `0x0059fac0`: NG+ Chainsaw pickup (`EvSubGetItem(0x10, 4)`, stops idle loop SE
//!   `0x4268`, plays positional SE `0x426b` at `(-61271.492, -508.48743, -577.02716)`,
//!   sets `flag(38)`).
//! - `[6]` `0x0059f6e0`: Leave Ending / extra demo `0x5a` (`90`, `flag(512) && !flag(513)`,
//!   voice `0x3f4` + `0xeae1`, `ClearDataRecordEnding(0x01)`, sets `flag(513)`).
//! - `[7]` `0x0059fb00`: Trial/demo gate exit (`flag(662)`, `SeCall(0x2744)`, mode `0x0c`).

use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, PI, TAU};

use crate::cutaway::CutawayOverlayEngine;
use crate::inventory::InventoryManager;

use super::{EventFlags, ProgContext, Sh2Rng, PROG_KEY_A, PROG_KEY_B};

/// Angela Orosco graveyard cutscene demo number (`sh2pc.exe` `0x005a057e`: `mov [0x1f7a7c4], 3`).
pub const STG4_ANGELA_GRAVE_DEMO_NO: u32 = 3;
/// Post-cutscene Angela graveyard talk demo number (`sh2pc.exe` `0x008e76d8` / `0x008e7720`).
pub const STG4_ANGELA_TALK_DEMO_NO: u32 = 4;
/// Leave Ending extra demo number (`0x5a` = `90`, `sh2pc.exe` `0x0059f6e7`).
pub const STG4_LEAVE_ENDING_DEMO_NO: u32 = 0x5a;

/// Well save point cutaway picture key (`sh2pc.exe` `0x007dccc8` -> `0x0065bc34`:
/// `"data/pic/out/p_redpaper.tex"`).
pub const STG4_WELL_PICTURE: &str = "out_p_redpaper";
/// Well save point stage message ID (`sh2pc.exe` `0x0059f3b9`: `push 0; call 0x4a22d0`).
pub const STG4_MSG_WELL_SAVE: i32 = 0;
/// NG+ Chainsaw pickup stage message ID (`sh2pc.exe` `0x0059fac1`: `push 4; push 0x10; call 0x4a2440`).
pub const STG4_MSG_GET_CHAINSAW: i32 = 4;
/// NG+ Chainsaw inventory item ID (`0x10` = `16`, `sh2pc.exe` `0x0059fac3`).
pub const STG4_ITEM_CHAINSAW: u8 = 0x10;
/// Well save point name ID (`2` = `"Forest Trail"`, `sh2pc.exe` `0x0059f41a`: `push 2; call 0x454f70`).
pub const STG4_WELL_SAVE_POINT_ID: u16 = 2;

/// Sound effect ID: Forest ambient wind loop (`0x2742` = `10050`, `voice.afs[23]`, `0x0059fc80`).
pub const STG4_SE_FOREST_WIND: u32 = 0x2742;
/// Sound effect ID: Save point chime (`0x2743` = `10051`, `voice.afs[35]`, `0x0059f3d3`).
pub const STG4_SE_SAVE_CHIME: u32 = 0x2743;
/// Sound effect ID: Trial/demo gate exit (`0x2744` = `10052`, `0x0059fb11`).
pub const STG4_SE_TRIAL_GATE_EXIT: u32 = 0x2744;
/// Sound effect ID: NG+ Chainsaw idle loop (`0x4268` = `17000`, `voice.afs[18]`, `0x0059fcd6`).
pub const STG4_SE_CHAINSAW_IDLE: u32 = 0x4268;
/// Sound effect ID: NG+ Chainsaw pickup/rev-down (`0x426b` = `17003`, `0x0059fae9`).
pub const STG4_SE_CHAINSAW_PICKUP: u32 = 0x426b;
/// Sound effect ID: Leave ending Mary's letter intro/voice (`0x03f4` & `0xeae1`, `0x0059f82f..0x0059f841`).
pub const STG4_SE_LEAVE_LETTER_A: u32 = 0x03f4;
/// Sound effect ID: Leave ending Mary's letter stream (`0xeae1`, `0x0059f83c`).
pub const STG4_SE_LEAVE_LETTER_B: u32 = 0xeae1;
/// Sound effect ID: Behind-James forest rustling (`0x9c86` = `40070`, `0x005a0095`).
pub const STG4_SE_AMBIENT_RUSTLE: u32 = 0x9c86;
/// Sound effect ID: Distant passing footsteps (`0x9c87` = `40071`, `0x005a01ed`).
pub const STG4_SE_AMBIENT_PASSER: u32 = 0x9c87;
/// Sound effect ID: Distant crow/forest ambience (`0x9c88` = `40072`, `0x0059ff86`).
pub const STG4_SE_AMBIENT_CROW: u32 = 0x9c88;

/// Event flag `5`: first-save explanation seen (`sh2pc.exe` `0x0059f411`: `0x01f7a9e0 |= 0x20`).
pub const STG4_FLAG_FIRST_SAVE_EXPLAINED: usize = 5;
/// Event flag `12` (`0x0c`): New Game+ clear flag (`ACT1_FOREST_EVENT_LIST[20]`, `0x0059fcad`).
pub const STG4_FLAG_NG_PLUS: usize = 12;
/// Event flag `36` (`0x24`): Angela graveyard cutscene (`demo_no = 3`) completed (`ACT1_FOREST_EVENT_LIST[12..=17]`).
pub const STG4_FLAG_ANGELA_MET: usize = 36;
/// Event flag `37` (`0x25`): Angela post-cutscene dialogue coin-flip selector (`0x005a07a9`: `0x01f7a9e4 |= 0x20`).
pub const STG4_FLAG_ANGELA_TALK_VARIANT: usize = 37;
/// Event flag `38` (`0x26`): NG+ Chainsaw picked up (`ACT1_FOREST_EVENT_LIST[20]`, `0x0059fcb7`).
pub const STG4_FLAG_CHAINSAW_TAKEN: usize = 38;
/// Event flag `512` (`0x200`): Leave Ending prerequisite (`ACT1_FOREST_EVENT_LIST[21]`, `0x0059fb60`).
pub const STG4_FLAG_LEAVE_ENDING_REQ: usize = 512;
/// Event flag `513` (`0x201`): Leave Ending completed (`ACT1_FOREST_EVENT_LIST[21]`).
pub const STG4_FLAG_LEAVE_ENDING_DONE: usize = 513;
/// Event flag `618` (`0x26a`): Global ambient SFX mute / cutscene inhibit (`0x0059fc70`: `0x01f7aa2c & 0x400`).
pub const STG4_FLAG_AMBIENT_MUTE: usize = 618;
/// Event flag `662` (`0x296`): Trial/demo gate flag (`ACT1_FOREST_EVENT_LIST[3..=5]`).
pub const STG4_FLAG_TRIAL_MODE: usize = 662;

/// Angela Orosco character kind ID (`0x0107`, `sh2pc.exe` `0x0059fbfb`, `0x005a07c7`).
pub const STG4_CHARA_ANGELA: u16 = 0x0107;
/// Angela idle animation ID before graveyard cutscene (`0x0f3d`, `sh2pc.exe` `0x0059fc22`).
pub const STG4_ANGELA_ANIM_BEFORE_DEMO: u16 = 0x0f3d;
/// Angela idle animation ID after graveyard cutscene (`0x0f3e`, `sh2pc.exe` `0x0059fc05`, `0x005a07d3`).
pub const STG4_ANGELA_ANIM_AFTER_DEMO: u16 = 0x0f3e;

/// Angela initial position before `haka_agl` (`sh2pc.exe` `0x008e757c`, `EvRoomInit` `0x0059fc11`).
pub const STG4_ANGELA_POS_BEFORE_DEMO: [f32; 3] = [2430.6165, 2636.6313, -1708.8447];
/// Angela position after `haka_agl` (`sh2pc.exe` `0x008e759c`, `EvRoomInit` `0x0059fbf4` & `0x005a07e3`).
pub const STG4_ANGELA_POS_AFTER_DEMO: [f32; 3] = [781.1213, 2505.674, -394.2832];
/// Angela facing yaw in graveyard (`0x40490fdb` = `π`, `sh2pc.exe` `0x0059fbe3`, `0x005a07de`).
pub const STG4_ANGELA_YAW: f32 = PI;

/// James position snapped before `haka_agl` (`sh2pc.exe` `0x008e76a4`, `0x005a05fc`).
pub const STG4_JAMES_PRE_DEMO3_POS: [f32; 3] = [-20.845001, 2473.03, -1332.315];
/// James position snapped after `haka_agl` (`sh2pc.exe` `0x008e76b4`, `0x005a07bb`).
pub const STG4_JAMES_POST_DEMO3_POS: [f32; 3] = [330.845, 2473.03, -1332.315];
/// James facing yaw before & after `haka_agl` (`0xc016cbe4` = `-2.3561945` rad = `-3π/4`, `0x005a05f7` / `0x005a07b6`).
pub const STG4_JAMES_DEMO3_YAW: f32 = -2.356_194_5;

/// James post-pose after `ev_prog[3]` (`EvProgGraveSureQuiet`, `sh2pc.exe` `0x008e7700`).
pub const STG4_JAMES_POST_PROG3_POS: [f32; 3] = [-179.005, 2487.65, -1286.69];
/// James yaw after `ev_prog[3]` (`0xbfa917c6` = `-1.321038` rad, `sh2pc.exe` `0x0059f4f3`).
pub const STG4_JAMES_POST_PROG3_YAW: f32 = -1.321_038;

/// James post-pose after `ev_prog[4]` (`EvProgGraveLookingFor`, `sh2pc.exe` `0x008e7748`).
pub const STG4_JAMES_POST_PROG4_POS: [f32; 3] = [-492.999_97, 2510.19, -1093.55];
/// James yaw after `ev_prog[4]` (`0xbfdd25cc` = `-1.727716` rad, `sh2pc.exe` `0x0059f613`).
pub const STG4_JAMES_POST_PROG4_YAW: f32 = -1.727_716;

/// NG+ Chainsaw 3D positional audio anchor in PS2 mm (`sh2pc.exe` `0x008e75bc`, used by
/// `EvSoundCallAfterLoad` `0x0059fccc` for `0x4268` and `ev_prog[5]` `0x0059fadf` for `0x426b`).
pub const STG4_CHAINSAW_SE_POS: [f32; 3] = [-61271.492, -508.487_43, -577.027_16];

/// `EvAllTime` (`0x0059fcf0`) wind distance attenuation boundary (`-20000.0 mm`, `sh2pc.exe` `0x006a91f8`).
pub const STG4_WIND_ATTEN_THRESHOLD_MM: f32 = -20_000.0;
/// `EvAllTime` (`0x0059fe53`) inverse wind attenuation range (`5.0e-5` = `1 / 20000.0`, `sh2pc.exe` `0x006ac094`).
pub const STG4_WIND_ATTEN_INV_RANGE: f32 = 5.0e-5;
/// `EvAllTime` (`0x0059fe74`) minimum clamped wind volume (`0.001f`, `0x3a83126f`).
pub const STG4_WIND_MIN_VOLUME: f32 = 0.001;
/// `EvAllTime` (`0x0059ff1f..0x0059ff3f`) distant crow SE `0x9c88` Z bounds (`-90000.0..-30000.0 mm`).
pub const STG4_CROW_Z_MIN_MM: f32 = -90_000.0;
/// Upper Z bound for distant crow SE `0x9c88` (`-30000.0 mm`, `sh2pc.exe` `0x0074b3c4`).
pub const STG4_CROW_Z_MAX_MM: f32 = -30_000.0;
/// `EvAllTime` (`0x0059fff5..0x005a0009`) rustle SE `0x9c86` yaw window (`-3π/4..-π/4`).
pub const STG4_RUSTLE_YAW_MIN_RAD: f32 = -2.356_194_5;
/// Upper yaw bound for rustle SE `0x9c86` (`-π/4` = `0xbf490fdb`, `sh2pc.exe` `0x006c9ca0`).
pub const STG4_RUSTLE_YAW_MAX_RAD: f32 = -FRAC_PI_4;
/// `EvAllTime` (`0x005a0055`) rustle SE `0x9c86` orbit radius behind James (`1500.0 mm`, `0x006b4038`).
pub const STG4_RUSTLE_RADIUS_MM: f32 = 1500.0;
/// `EvAllTime` (`0x005a0129`) minimum player speed to keep `0x9c86` active (`1.2`, `0x006a2b14`).
pub const STG4_RUSTLE_MIN_SPEED: f32 = 1.2;
/// `EvAllTime` (`0x005a01a8`) distant passing footsteps SE `0x9c87` Z threshold (`-25000.0 mm`, `0x0074b3b4`).
pub const STG4_PASSER_Z_MAX_MM: f32 = -25_000.0;
/// `EvAllTime` (`0x005a022e`) distant passing footsteps SE `0x9c87` X sweep speed (`1500.0 mm/s`, `0x006b4038`).
pub const STG4_PASSER_SWEEP_SPEED_MM_S: f32 = 1500.0;

/// Angela NPC spawn descriptor produced by `EvRoomInit` (`0x0059fb60`) or post-demo cleanup.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ForestAngelaSpawn {
    /// Character kind ID (`STG4_CHARA_ANGELA = 0x0107`).
    pub chara_kind: u16,
    /// Animation ID (`0x0f3d` before `flag(36)`, `0x0f3e` after `flag(36)`).
    pub anim_id: u16,
    /// World position `[x, y, z]` in PS2 mm.
    pub position_mm: [f32; 3],
    /// Facing yaw in radians (`π`).
    pub yaw_rad: f32,
}

/// Evaluates Stage 4 `EvRoomInit` (`sh2pc.exe` `0x0059fb60..0x0059fc68`) returning Angela's
/// spawn pose (or `None` during Leave Ending mode `flag(512)`).
#[must_use]
pub fn stage4_eval_room_init_angela(flags: &EventFlags) -> Option<ForestAngelaSpawn> {
    if flags.get(STG4_FLAG_LEAVE_ENDING_REQ) {
        return None;
    }
    if flags.get(STG4_FLAG_ANGELA_MET) {
        Some(ForestAngelaSpawn {
            chara_kind: STG4_CHARA_ANGELA,
            anim_id: STG4_ANGELA_ANIM_AFTER_DEMO,
            position_mm: STG4_ANGELA_POS_AFTER_DEMO,
            yaw_rad: STG4_ANGELA_YAW,
        })
    } else {
        Some(ForestAngelaSpawn {
            chara_kind: STG4_CHARA_ANGELA,
            anim_id: STG4_ANGELA_ANIM_BEFORE_DEMO,
            position_mm: STG4_ANGELA_POS_BEFORE_DEMO,
            yaw_rad: STG4_ANGELA_YAW,
        })
    }
}

/// Evaluates Stage 4 `EvSoundCallAfterLoad` (`sh2pc.exe` `0x0059fc70..0x0059fce3`), returning
/// `(ambient_2d_se_ids, positional_se_calls)` to start on room load.
#[must_use]
pub fn stage4_eval_sound_call_after_load(flags: &EventFlags) -> (Vec<u32>, Vec<(u32, [f32; 3])>) {
    let mut se_2d = Vec::new();
    let mut se_pos = Vec::new();
    if !flags.get(STG4_FLAG_AMBIENT_MUTE) {
        se_2d.push(STG4_SE_FOREST_WIND);
        if flags.get(STG4_FLAG_NG_PLUS) && !flags.get(STG4_FLAG_CHAINSAW_TAKEN) {
            se_pos.push((STG4_SE_CHAINSAW_IDLE, STG4_CHAINSAW_SE_POS));
        }
    }
    (se_2d, se_pos)
}

/// Persistent BSS state for Stage 4 `EvAllTime` (`sh2pc.exe` `0x0059fcf0..0x005a025a`:
/// `0x01d83b3c..0x01d83b74`).
#[derive(Debug, Clone)]
pub struct ForestAmbientState {
    /// Wind oscillator speed factor (`0x01d83b3c`, in `[0.0, 1.0)`).
    pub wind_speed_rand: f32,
    /// Wind oscillator amplitude factor (`0x01d83b40`, in `[0.0, 1.0)`).
    pub wind_amp_rand: f32,
    /// Wind oscillator phase in radians (`0x01d83b44`, wrapped mod `2π`).
    pub wind_phase: f32,
    /// Countdown timer in seconds for distant crow SE `0x9c88` (`0x01d83b54`).
    pub crow_timer: f32,
    /// Countdown timer in seconds for behind-James rustle SE `0x9c86` (`0x01d83b58`).
    pub rustle_timer: f32,
    /// Countdown timer in seconds for passing footsteps SE `0x9c87` (`0x01d83b60`).
    pub passer_timer: f32,
    /// Active rustle orbit angle in radians (`0x01d83b74`).
    pub rustle_angle: f32,
    /// Active passing footsteps world position (`0x01d83b64..0x01d83b6c`).
    pub passer_pos: [f32; 3],
    /// Whether `0x9c86` is currently playing.
    pub rustle_playing: bool,
    /// Whether `0x9c87` is currently playing.
    pub passer_playing: bool,
    /// Whether `0x9c88` is currently playing.
    pub crow_playing: bool,
}

impl Default for ForestAmbientState {
    fn default() -> Self {
        Self {
            wind_speed_rand: 0.5,
            wind_amp_rand: 0.5,
            wind_phase: 0.0,
            crow_timer: -1.0,
            rustle_timer: -1.0,
            passer_timer: 90.0,
            rustle_angle: 0.0,
            passer_pos: [0.0; 3],
            rustle_playing: false,
            passer_playing: false,
            crow_playing: false,
        }
    }
}

/// Per-frame output from [`ForestAmbientState::tick_all_time`] (`sh2pc.exe` `0x0059fcf0`).
#[derive(Debug, Default, Clone, PartialEq)]
pub struct ForestAllTimeEffects {
    /// Whether model group `0x14` (NG+ chainsaw log) is visible (`0` when `flag(12)`, `-1` otherwise).
    pub chainsaw_log_visible: bool,
    /// Whether model group `0x0a` (cutscene prop) is visible (`0` when `demo_active`, `-1` otherwise).
    pub demo_prop_visible: bool,
    /// Modulated volume for forest wind loop SE `0x2742` (when `!flag(512)`).
    pub wind_volume: Option<f32>,
    /// Newly spawned 3D ambient sound effects `(se_id, pos_mm)`.
    pub spawned_positional_se: Vec<(u32, [f32; 3])>,
    /// Updated 3D positions for currently playing ambient sound effects `(se_id, pos_mm)`.
    pub updated_positional_se: Vec<(u32, [f32; 3])>,
    /// Ambient sound effects to stop (e.g. `0x9c86` when James stops moving or turns away).
    pub stopped_se: Vec<u32>,
}

impl ForestAmbientState {
    /// Initializes timers matching `EvRoomInit` (`0x0059fc30..0x0059fc65`) and
    /// `EvSoundCallAfterLoad` (`0x0059fc8d..0x0059fcaa`).
    pub fn init_from_rng(rng: &mut Sh2Rng) -> Self {
        Self {
            wind_speed_rand: rng.next_f32(),
            wind_amp_rand: rng.next_f32(),
            wind_phase: 0.0,
            crow_timer: -1.0,
            rustle_timer: -1.0,
            passer_timer: rng.next_f32() * 120.0 + 30.0,
            rustle_angle: 0.0,
            passer_pos: [0.0; 3],
            rustle_playing: false,
            passer_playing: false,
            crow_playing: false,
        }
    }

    /// Evaluates one frame of `stg_forest.c` `EvAllTime` (`sh2pc.exe` `0x0059fcf0..0x005a025a`).
    #[allow(clippy::too_many_arguments)]
    pub fn tick_all_time(
        &mut self,
        player_pos: [f32; 3],
        player_yaw: f32,
        player_speed: f32,
        dt: f32,
        demo_active: bool,
        flags: &EventFlags,
        rng: &mut Sh2Rng,
    ) -> ForestAllTimeEffects {
        let mut out = ForestAllTimeEffects {
            chainsaw_log_visible: flags.get(STG4_FLAG_NG_PLUS),
            demo_prop_visible: demo_active,
            ..ForestAllTimeEffects::default()
        };

        // 1. Forest wind SE `0x2742` oscillator & spatial distance attenuation (`0x0059fd7a..0x0059fe8e`).
        if !flags.get(STG4_FLAG_LEAVE_ENDING_REQ) {
            self.wind_phase += dt * (self.wind_speed_rand * 1.5 + 0.5) * FRAC_PI_2;
            if self.wind_phase >= TAU {
                self.wind_phase -= TAU;
                self.wind_speed_rand = rng.next_f32();
                self.wind_amp_rand = rng.next_f32();
            }
            let mut vol = self.wind_phase.sin() * self.wind_amp_rand * 0.2 + 0.8;
            if player_pos[0] > STG4_WIND_ATTEN_THRESHOLD_MM
                && player_pos[2] > STG4_WIND_ATTEN_THRESHOLD_MM
            {
                let min_axis = player_pos[0].min(player_pos[2]);
                let scale = 1.0 - (min_axis + 20_000.0) * STG4_WIND_ATTEN_INV_RANGE;
                vol *= scale;
                if vol <= 0.0 {
                    vol = STG4_WIND_MIN_VOLUME;
                }
            }
            out.wind_volume = Some(vol);
        }

        // 2. Distant crow/forest SE `0x9c88` (`0x0059fec8..0x0059ff9c`).
        self.crow_timer -= dt;
        if self.crow_timer <= 0.0 {
            self.crow_timer = rng.next_f32() * 25.0 + 3.0;
            if !self.passer_playing
                && player_pos[0] > 0.0
                && player_pos[2] < STG4_CROW_Z_MAX_MM
                && player_pos[2] > STG4_CROW_Z_MIN_MM
            {
                let pos = [
                    player_pos[0] + 2000.0,
                    player_pos[1],
                    player_pos[2] + rng.next_f32() * 2000.0 - 1000.0,
                ];
                self.crow_playing = true;
                out.spawned_positional_se.push((STG4_SE_AMBIENT_CROW, pos));
            }
        }

        // 3. Behind-James rustling SE `0x9c86` (`0x0059ff9f..0x005a0160`).
        self.rustle_timer -= dt;
        if self.rustle_timer <= 0.0 {
            self.rustle_timer = rng.next_f32() * 15.0 + 5.0;
            if player_pos[0] < STG4_WIND_ATTEN_THRESHOLD_MM
                && player_yaw < STG4_RUSTLE_YAW_MAX_RAD
                && player_yaw > STG4_RUSTLE_YAW_MIN_RAD
            {
                let raw_angle = player_yaw + PI + rng.next_f32() * 0.392_698_8 - 0.196_349_38;
                self.rustle_angle = wrap_angle_pi(raw_angle);
                let pos = [
                    player_pos[0] + self.rustle_angle.cos() * STG4_RUSTLE_RADIUS_MM,
                    player_pos[1],
                    player_pos[2] + self.rustle_angle.sin() * STG4_RUSTLE_RADIUS_MM,
                ];
                self.rustle_playing = true;
                out.spawned_positional_se
                    .push((STG4_SE_AMBIENT_RUSTLE, pos));
            }
        }
        if self.rustle_playing {
            let pos = [
                player_pos[0] + self.rustle_angle.cos() * STG4_RUSTLE_RADIUS_MM,
                player_pos[1],
                player_pos[2] + self.rustle_angle.sin() * STG4_RUSTLE_RADIUS_MM,
            ];
            out.updated_positional_se
                .push((STG4_SE_AMBIENT_RUSTLE, pos));
            if player_speed < STG4_RUSTLE_MIN_SPEED
                || player_yaw > STG4_RUSTLE_YAW_MAX_RAD
                || player_yaw < STG4_RUSTLE_YAW_MIN_RAD
            {
                self.rustle_playing = false;
                out.stopped_se.push(STG4_SE_AMBIENT_RUSTLE);
            }
        }

        // 4. Distant passing footsteps SE `0x9c87` (`0x005a0163..0x005a0257`).
        if !self.crow_playing
            && self.passer_timer > 0.0
            && player_pos[0] > 0.0
            && player_pos[2] < STG4_PASSER_Z_MAX_MM
        {
            self.passer_timer -= dt;
            if self.passer_timer <= 0.0 {
                self.passer_pos = [
                    player_pos[0] - 1000.0,
                    player_pos[1],
                    player_pos[2] - 2000.0,
                ];
                self.passer_playing = true;
                out.spawned_positional_se
                    .push((STG4_SE_AMBIENT_PASSER, self.passer_pos));
            }
        }
        if self.passer_playing {
            self.passer_pos[0] += dt * STG4_PASSER_SWEEP_SPEED_MM_S;
            out.updated_positional_se
                .push((STG4_SE_AMBIENT_PASSER, self.passer_pos));
        }

        out
    }
}

/// Wraps an angle in radians into `[-π, π]` (`sh2pc.exe` `0x00448280` `shRadianFix`).
#[inline]
fn wrap_angle_pi(mut rad: f32) -> f32 {
    while rad > PI {
        rad -= TAU;
    }
    while rad < -PI {
        rad += TAU;
    }
    rad
}

/// Per-frame output returned by [`tick_stage4_prog`].
#[derive(Debug, Default)]
pub(super) struct Stage4TickOut {
    pub finished: bool,
    pub active_message_id: Option<i32>,
    pub sound_effects: Vec<u32>,
    pub stop_sound_effects: Vec<u32>,
    pub positional_sound_effects: Vec<(u32, [f32; 3])>,
    pub requested_demo: Option<u32>,
    pub requested_demo_id: Option<&'static str>,
    pub requested_demo_post_pose: Option<([f32; 3], f32)>,
    pub open_save_point_id: Option<u16>,
    pub triggered_ending: Option<u8>,
}

/// Advances one of `stg_forest.c`'s `ev_prog[1..=7]` state machines by a single frame.
#[allow(clippy::too_many_arguments)]
pub(super) fn tick_stage4_prog(
    p: &mut ProgContext,
    down: u32,
    active_message_id: Option<i32>,
    flags: &mut EventFlags,
    inventory: &mut InventoryManager,
    overlay: &mut CutawayOverlayEngine,
) -> Stage4TickOut {
    let mut out = Stage4TickOut {
        active_message_id,
        ..Stage4TickOut::default()
    };

    match p.prog_no {
        // Stage 4 #1 (`0x0059f2b0`): Well Save Point (`ACT1_FOREST_EVENT_LIST[11]`).
        // Retail `0x0059f2b0` jump-table flow:
        //   step 0 (`0x59f2cf`): lock player, open `data/pic/out/p_redpaper.tex` (`out_p_redpaper`)
        //   step 1 (`0x59f353`): wait for A/B -> open stage message `0` (`0x59f3b9`)
        //   step 2 (`0x59f3a2`): wait for A/B to dismiss stage message `0` -> play `SeCall(0x2743)`,
        //                        set `flag(5)` (`0x59f411`), `SavePointSet(2)` (`0x59f41a`),
        //                        open Save Menu (`mode = 9`, `0x59f42f`), close picture, unlock.
        1 => match p.step {
            0 => {
                p.locked = true;
                p.flag_set = false;
                overlay.set_picture(Some(STG4_WELL_PICTURE), flags);
                p.step = 1;
            }
            1 => {
                if (down & (PROG_KEY_A | PROG_KEY_B)) != 0 {
                    out.active_message_id = Some(STG4_MSG_WELL_SAVE);
                    p.step = 2;
                }
            }
            2 => {
                if (down & (PROG_KEY_A | PROG_KEY_B)) != 0 {
                    out.active_message_id = None;
                    out.sound_effects.push(STG4_SE_SAVE_CHIME);
                    flags.set(STG4_FLAG_FIRST_SAVE_EXPLAINED, true);
                    out.open_save_point_id = Some(STG4_WELL_SAVE_POINT_ID);
                    overlay.set_picture(None, flags);
                    p.locked = false;
                    out.finished = true;
                }
            }
            _ => out.finished = true,
        },
        // Stage 4 #2 (`0x005a0570`): Angela Orosco Graveyard cutscene (`demo_no = 3`, `haka_agl`).
        // Requests `demo_no = 3` WITHOUT setting `flag(36)` prematurely so `pick_bgm_page(4, ...)`
        // stays on Page `47` (`forest/bgm_114_b.aix`) during playback; `CampaignOrchestrator` runs
        // real `.dds` playback, snaps James to `STG4_JAMES_POST_DEMO3_POS` / `STG4_JAMES_DEMO3_YAW`,
        // and writes `flag(36)` + 50% RNG `flag(37)` (`0x005a07a0..0x005a07fe`) on completion.
        2 => {
            p.locked = true;
            p.flag_set = false;
            out.requested_demo = Some(STG4_ANGELA_GRAVE_DEMO_NO);
            out.requested_demo_id = Some("haka_agl");
            out.requested_demo_post_pose = Some((STG4_JAMES_POST_DEMO3_POS, STG4_JAMES_DEMO3_YAW));
            p.locked = false;
            out.finished = true;
        }
        // Stage 4 #3 (`0x0059f4a0`): `EvProgGraveSureQuiet` (`flag(36) && flag(37)`).
        // Plays `demo_no = 4` (`haka_agl_h.dds`, voice `0xea96`); on completion, snaps James to
        // `STG4_JAMES_POST_PROG3_POS` / `STG4_JAMES_POST_PROG3_YAW` (`0x008e7700`) and re-rolls
        // `flag(37)` via `shRand() & 1` (`0x0059f52e..0x0059f553`).
        3 => {
            p.locked = true;
            p.flag_set = false;
            out.requested_demo = Some(STG4_ANGELA_TALK_DEMO_NO);
            out.requested_demo_id = Some("haka_agl_h");
            out.requested_demo_post_pose =
                Some((STG4_JAMES_POST_PROG3_POS, STG4_JAMES_POST_PROG3_YAW));
            p.locked = false;
            out.finished = true;
        }
        // Stage 4 #4 (`0x0059f5c0`): `EvProgGraveLookingFor` (`flag(36) && !flag(37)`).
        // Plays `demo_no = 4` (`haka_agl_i.dds`, voice `0xea97`); on completion, snaps James to
        // `STG4_JAMES_POST_PROG4_POS` / `STG4_JAMES_POST_PROG4_YAW` (`0x008e7748`) and re-rolls
        // `flag(37)` via `shRand() & 1` (`0x0059f64e..0x0059f673`).
        4 => {
            p.locked = true;
            p.flag_set = false;
            out.requested_demo = Some(STG4_ANGELA_TALK_DEMO_NO);
            out.requested_demo_id = Some("haka_agl_i");
            out.requested_demo_post_pose =
                Some((STG4_JAMES_POST_PROG4_POS, STG4_JAMES_POST_PROG4_YAW));
            p.locked = false;
            out.finished = true;
        }
        // Stage 4 #5 (`0x0059fac0`): NG+ Chainsaw pickup (`flag(12) && !flag(38)`).
        // `EvSubGetItem(0x10, 4)` -> grants Chainsaw (`16`), shows stage message `4`,
        // stops idle loop SE `0x4268`, plays positional SE `0x426b` at `STG4_CHAINSAW_SE_POS`,
        // and sets completion `flag(38)` (`0x26`).
        5 => match p.step {
            0 => {
                p.locked = true;
                inventory.item_get_now(STG4_ITEM_CHAINSAW);
                out.active_message_id = Some(STG4_MSG_GET_CHAINSAW);
                out.stop_sound_effects.push(STG4_SE_CHAINSAW_IDLE);
                out.positional_sound_effects
                    .push((STG4_SE_CHAINSAW_PICKUP, STG4_CHAINSAW_SE_POS));
                p.step = 1;
            }
            1 => {
                if (down & (PROG_KEY_A | PROG_KEY_B)) != 0 {
                    out.active_message_id = None;
                    flags.set(STG4_FLAG_CHAINSAW_TAKEN, true);
                    p.locked = false;
                    out.finished = true;
                }
            }
            _ => out.finished = true,
        },
        // Stage 4 #6 (`0x0059f6e0`): Leave Ending (`flag(512) && !flag(513)`).
        // Requests `demo_no = 90` (`0x5a`), queues Mary's letter SEs (`0x3f4`, `0xeae1`),
        // records Leave Ending bit `0x01`, sets `flag(513)`.
        6 => {
            p.locked = true;
            if flags.get(STG4_FLAG_LEAVE_ENDING_REQ) && !flags.get(STG4_FLAG_LEAVE_ENDING_DONE) {
                out.requested_demo = Some(STG4_LEAVE_ENDING_DEMO_NO);
                out.sound_effects.push(STG4_SE_LEAVE_LETTER_A);
                out.sound_effects.push(STG4_SE_LEAVE_LETTER_B);
                out.triggered_ending = Some(0x01);
                flags.set(STG4_FLAG_LEAVE_ENDING_DONE, true);
            } else {
                p.flag_set = false;
            }
            p.locked = false;
            out.finished = true;
        }
        // Stage 4 #7 (`0x0059fb00`): Trial/demo gate exit (`flag(662)`).
        // Plays `SeCall(0x2744)` and completes.
        7 => {
            p.flag_set = false;
            out.sound_effects.push(STG4_SE_TRIAL_GATE_EXIT);
            out.finished = true;
        }
        _ => {
            out.finished = true;
        }
    }

    out
}
