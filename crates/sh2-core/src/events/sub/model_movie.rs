//! Background Model Visibility Control & FMV Movie Event Coroutines
//! (`SLUS_202.28` `Event/event_sub.c` `0x001ad560..0x001ad85b` and
//! `sh2pc.exe` `0x004a2f10..0x004a30aa`).

/// System flag bit (`_DAT_010f08a4 >> 6 & 1`) indicating a drama demo is currently active
/// (`EvSubMovieReady` `0x001ad680` / `sh2pc.exe` `0x004a2fc0`).
pub const SYS_FLAG_DEMO_ACTIVE_BIT: u32 = 1 << 6;

/// Drama demo status bit (`_demo_status |= 0x200`) preventing cutscene skip once movie preload
/// begins (`EvSubMovieReady` `0x001ad680` / `sh2pc.exe` `0x004a2fc0`).
pub const DEMO_STATUS_MOVIE_READY_BIT: u32 = 0x200;

/// Lead-in frames before demo end when movie preload may start on PS2 (`30.0` in `SLUS_202.28`
/// `0x001ad680`).
pub const MOVIE_READY_LEAD_FRAMES_PS2: f32 = 30.0;

/// Lead-in frames before demo end when movie preload may start on retail PC (`5.0` in `sh2pc.exe`
/// `0x004a2fc0`).
pub const MOVIE_READY_LEAD_FRAMES_PC: f32 = 5.0;

/// Main game mode ID entered on movie start in `SLUS_202.28` (`0x0e` at `0x001ad780`).
pub const MOVIE_MAIN_MODE_PS2: u32 = 0x0e;

/// Main game mode ID entered on movie start in retail `sh2pc.exe` (`0x0f` at `0x004a3040`).
pub const MOVIE_MAIN_MODE_PC: u32 = 0x0f;

/// Single `(key, bitmask)` entry in a stage's background display-control table
/// (`EvDispControlModelEntry` `SLUS_202.28` `0x001ad560`, `sh2pc.exe` `0x004a2f10`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DispControlModelEntry {
    /// Packed key `(stage_id << 16) | (model_id & 0xffff)`.
    pub key: u32,
    /// Bitmask of toggled sub-parts (`1 << (sub_part_bit & 0x1f)`).
    pub mask: u32,
}

/// Table of background display-control entries built via [`ev_disp_control_model_entry`] and
/// flushed via [`ev_disp_control_model_exec`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DispControlModelTable {
    entries: Vec<DispControlModelEntry>,
}

impl DispControlModelTable {
    /// Creates an empty display-control table.
    pub const fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Returns all registered entries in insertion order.
    pub fn entries(&self) -> &[DispControlModelEntry] {
        &self.entries
    }

    /// Clears all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

/// Command emitted by [`ev_disp_control_model_exec`] (`EvDispControlModelExec` `0x001ad5e0`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispOnOffCommand {
    /// `sh2gfw_Init_DispOnOffObj()` (emitted first when `BgIsOut() != 0`).
    Init,
    /// `sh2gfw_Set_DispOnOffObj(key, mask)` (when `BgIsOut() == 0`).
    Set { key: u32, mask: u32 },
    /// `sh2gfw_FastSet_DispOnOffObj(key, mask)` (when `BgIsOut() != 0`).
    FastSet { key: u32, mask: u32 },
}

/// Registers or updates a background model visibility mask entry (`EvDispControlModelEntry` in
/// `SLUS_202.28` `0x001ad560` and `sh2pc.exe` `0x004a2f10`).
///
/// Packs `key = (stage_id << 16) | (model_id & 0xffff)`. If `sub_part_bit >= 0`, sets bit
/// `1 << ((sub_part_bit as u32) & 0x1f)` on the entry's mask; if `sub_part_bit < 0`, ensures the
/// entry exists without modifying its mask bits.
pub fn ev_disp_control_model_entry(
    table: &mut DispControlModelTable,
    stage_id: u32,
    model_id: u32,
    sub_part_bit: i32,
) {
    let key = (stage_id << 16) | (model_id & 0xffff);
    let entry = if let Some(existing) = table.entries.iter_mut().find(|e| e.key == key) {
        existing
    } else {
        table.entries.push(DispControlModelEntry { key, mask: 0 });
        table.entries.last_mut().expect("entry was just pushed")
    };
    if sub_part_bit >= 0 {
        entry.mask |= 1u32 << ((sub_part_bit as u32) & 0x1f);
    }
}

/// Flushes a [`DispControlModelTable`] to renderer visibility commands (`EvDispControlModelExec`
/// in `SLUS_202.28` `0x001ad5e0` and `sh2pc.exe` `0x004a2f60`).
///
/// - When `bg_is_out == false` (`BgIsOut() == 0`): emits [`DispOnOffCommand::Set`] per entry.
/// - When `bg_is_out == true` (`BgIsOut() != 0`): emits [`DispOnOffCommand::Init`] followed by
///   [`DispOnOffCommand::FastSet`] per entry.
pub fn ev_disp_control_model_exec(
    table: &DispControlModelTable,
    bg_is_out: bool,
) -> Vec<DispOnOffCommand> {
    let mut cmds = Vec::with_capacity(table.entries.len() + usize::from(bg_is_out));
    if bg_is_out {
        cmds.push(DispOnOffCommand::Init);
        for e in &table.entries {
            cmds.push(DispOnOffCommand::FastSet {
                key: e.key,
                mask: e.mask,
            });
        }
    } else {
        for e in &table.entries {
            cmds.push(DispOnOffCommand::Set {
                key: e.key,
                mask: e.mask,
            });
        }
    }
    cmds
}

/// Binary timing/mode profile for `EvSubMovieReady` / `EvSubMovieStart` capturing the documented
/// `SLUS_202.28` prototype vs retail `sh2pc.exe` drift.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MovieTimingProfile {
    /// PS2 prototype (`SLUS_202.28` `0x001ad680` / `0x001ad780`): `30.0`-frame demo lead-in,
    /// `sd_stat & 0xf0 in {0x00, 0x50}`, main mode `0x0e`, emits `SetPauseRetain`.
    Ps2,
    /// Retail PC (`sh2pc.exe` `0x004a2fc0` / `0x004a3040`): `5.0`-frame demo lead-in,
    /// `sd_stat & 0xf0 not in {0x10, 0x20}`, checks `!movie_is_sleeping` on start, main mode `0x0f`.
    #[default]
    RetailPc,
}

/// Runtime state mutated by `EvSubMovieReady` / `EvSubMovieStart` / `EvSubMovieEnd`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MovieState {
    /// `_DAT_010f08a8` (`DAT_00932198` PC): set to `0` on `Ready`, `1` on `End`.
    pub movie_fade_guard: u8,
    /// `_demo_status` (`DAT_01f7a7a4` PC): receives [`DEMO_STATUS_MOVIE_READY_BIT`] (`0x200`).
    pub demo_status: u32,
    /// Active game main mode (`_DAT_010f0888` PS2 / `DAT_00932178` PC) when movie starts.
    pub main_mode: u32,
    /// 5 sub-step registers cleared to `0` on `EvSubMovieStart` (`0x010f088c..0x010f089c`).
    pub sub_steps: [u32; 5],
}

/// Side-effect command emitted by `EvSubMovieReady` / `EvSubMovieStart` / `EvSubMovieEnd`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MovieCommand {
    /// `MoviePreSet(movie_id)` (`0x001ad680` / `0x004a2fc0`).
    PreSet { movie_id: u32 },
    /// `movieSetSubTitleData(stage_msg, start_id, end_id)` (`0x001ad680` / `0x004a2fc0`).
    SetSubtitleData { start_id: u32, end_id: u32 },
    /// `sh2gfw_Set_PauseRetain()` (PS2 `0x001ad780`).
    SetPauseRetain,
    /// `sh2gfw_Reset_FilterCommand()` (PS2 `0x001ad830`).
    ResetFilterCommand,
}

/// Inputs for [`ev_sub_movie_ready`] (`EvSubMovieReady` `0x001ad680` / `0x004a2fc0`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MovieReadyInput {
    pub movie_id: u32,
    pub subtitle_start: u32,
    pub subtitle_end: u32,
    /// System flags (`_DAT_010f08a4` PS2 / `DAT_00932194` PC).
    pub sys_flags: u32,
    /// Current drama demo frame (`_demo_frame` / `DAT_01f7a7b8`).
    pub demo_frame: f32,
    /// Total drama demo frames (`_total_demo_frame` / `DAT_01f7a7bc`).
    pub total_demo_frame: f32,
    /// Sound driver stream status byte (`shSdStat()`).
    pub sd_stat: u8,
    /// `true` when `MovieCheckSleep() != 0`.
    pub movie_is_sleeping: bool,
    pub profile: MovieTimingProfile,
}

/// Executes `EvSubMovieReady` (`SLUS_202.28` `0x001ad680`, `sh2pc.exe` `0x004a2fc0`).
pub fn ev_sub_movie_ready(
    input: &MovieReadyInput,
    movie_state: &mut MovieState,
) -> Vec<MovieCommand> {
    let mut cmds = Vec::new();
    let demo_active = (input.sys_flags & SYS_FLAG_DEMO_ACTIVE_BIT) != 0;
    let stat_hi = input.sd_stat & 0xf0;

    match input.profile {
        MovieTimingProfile::Ps2 => {
            let can_preload = if !demo_active {
                true
            } else {
                movie_state.demo_status |= DEMO_STATUS_MOVIE_READY_BIT;
                input.demo_frame >= (input.total_demo_frame - MOVIE_READY_LEAD_FRAMES_PS2)
                    && (stat_hi == 0x00 || stat_hi == 0x50)
            };
            if can_preload && input.movie_is_sleeping {
                movie_state.movie_fade_guard = 0;
                cmds.push(MovieCommand::PreSet {
                    movie_id: input.movie_id,
                });
                cmds.push(MovieCommand::SetSubtitleData {
                    start_id: input.subtitle_start,
                    end_id: input.subtitle_end,
                });
            }
        }
        MovieTimingProfile::RetailPc => {
            movie_state.movie_fade_guard = 0;
            if demo_active {
                if input.demo_frame < (input.total_demo_frame - MOVIE_READY_LEAD_FRAMES_PC) {
                    return cmds;
                }
                if stat_hi == 0x20 || stat_hi == 0x10 {
                    return cmds;
                }
            }
            if input.movie_is_sleeping {
                cmds.push(MovieCommand::PreSet {
                    movie_id: input.movie_id,
                });
                cmds.push(MovieCommand::SetSubtitleData {
                    start_id: input.subtitle_start,
                    end_id: input.subtitle_end,
                });
                movie_state.demo_status |= DEMO_STATUS_MOVIE_READY_BIT;
            }
        }
    }

    cmds
}

/// Inputs for [`ev_sub_movie_start`] (`EvSubMovieStart` `0x001ad780` / `0x004a3040`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MovieStartInput {
    /// `true` when `MovieCheckSleep() != 0` (checked on retail PC `0x004a3040`).
    pub movie_is_sleeping: bool,
    /// `true` when `MovieWaitReady() != 0`.
    pub movie_ready: bool,
    /// Caller-supplied trigger condition (`param_1 != 0`).
    pub trigger: bool,
    /// Global frame counter (`DAT_002ab75c` PS2 / `DAT_00942600` PC); must satisfy `% 3 == 1`.
    pub frame_counter: u32,
    pub profile: MovieTimingProfile,
}

/// Executes `EvSubMovieStart` (`SLUS_202.28` `0x001ad780`, `sh2pc.exe` `0x004a3040`).
///
/// Returns `(started, commands)`. When `started` is `true`, transitions `movie_state.main_mode`
/// to `0x0e` (PS2) or `0x0f` (PC) and clears `movie_state.sub_steps`.
pub fn ev_sub_movie_start(
    input: &MovieStartInput,
    movie_state: &mut MovieState,
) -> (bool, Vec<MovieCommand>) {
    let mut cmds = Vec::new();
    if input.profile == MovieTimingProfile::RetailPc && input.movie_is_sleeping {
        return (false, cmds);
    }
    if !input.movie_ready || !input.trigger || (input.frame_counter % 3 != 1) {
        return (false, cmds);
    }

    match input.profile {
        MovieTimingProfile::Ps2 => {
            cmds.push(MovieCommand::SetPauseRetain);
            movie_state.main_mode = MOVIE_MAIN_MODE_PS2;
        }
        MovieTimingProfile::RetailPc => {
            movie_state.main_mode = MOVIE_MAIN_MODE_PC;
        }
    }
    movie_state.sub_steps = [0; 5];
    (true, cmds)
}

/// Executes `EvSubMovieEnd` (`SLUS_202.28` `0x001ad830`, `sh2pc.exe` `0x004a30a0`).
pub fn ev_sub_movie_end(
    profile: MovieTimingProfile,
    movie_state: &mut MovieState,
) -> Option<MovieCommand> {
    movie_state.movie_fade_guard = 1;
    match profile {
        MovieTimingProfile::Ps2 => Some(MovieCommand::ResetFilterCommand),
        MovieTimingProfile::RetailPc => None,
    }
}
