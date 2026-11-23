//! Global BGM ducking mode and ramp logic (`SeBgmManager` `SLUS_202.28` `0x00194300` / `sh2pc.exe` `0x00516430`).

/// Normal gameplay BGM global fade target (`SeBgmManager` `SLUS_202.28` `0x00194300` / `sh2pc.exe` `0x00516430`).
pub const BGM_DUCK_NORMAL: f32 = 1.0;

/// Non-exempt drama demo / cutscene BGM global fade target (`SeBgmManager` `0x00194300` / `0x00516430`).
pub const BGM_DUCK_DEMO: f32 = 0.7;

/// Pause / Inventory / Map / Memo / Options subscreen (`game_mode` `5..=9`) BGM global fade target
/// (`SeBgmManager` `0x00194300` / `0x00516430`).
pub const BGM_DUCK_PAUSED: f32 = 0.5;

/// Ramp divisor for `global_bgm_fade` (`dt / 2.0`, i.e., `0.5/s` in both `SLUS_202.28` `0x00194300` and `sh2pc.exe` `0x00516430`).
pub const BGM_DUCK_RAMP_DIVISOR: f32 = 2.0;

/// Game/demo state controlling `SeBgmManager` (`0x00194300`) global BGM ducking (`bgm.vol` / `_DAT_010e5890`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BgmDuckingMode {
    /// Normal gameplay or exempt drama demos: target `1.0`.
    #[default]
    Normal,
    /// Non-exempt drama demo / cutscene: target `0.7`.
    DramaDemo,
    /// Pause / Inventory / Map / Memo / Options subscreen (`game_mode` `5..=9`): target `0.5`.
    PausedOrMenu,
}

impl BgmDuckingMode {
    /// Target `global_bgm_fade` scalar (`0.5`, `0.7`, or `1.0`, `SeBgmManager` `0x00194300` / `0x00516430`).
    #[inline]
    pub const fn target_volume(self) -> f32 {
        match self {
            Self::Normal => BGM_DUCK_NORMAL,
            Self::DramaDemo => BGM_DUCK_DEMO,
            Self::PausedOrMenu => BGM_DUCK_PAUSED,
        }
    }
}

/// Returns `true` if `drama_demo_id` is exempt from `0.7` BGM ducking in retail `sh2pc.exe` (`0x00516430`).
///
/// Prototype vs. Retail drift: `SLUS_202.28` (`0x00194300`) only exempted `{0, 1, 2, 0x0e}`;
/// retail `sh2pc.exe` (`0x00516430`) expanded the exemption list to these 27 demo IDs.
#[inline]
pub const fn is_bgm_demo_duck_exempt(drama_demo_id: i32) -> bool {
    matches!(
        drama_demo_id,
        0 | 1
            | 2
            | 3
            | 0x0e
            | 0x10
            | 0x12
            | 0x15
            | 0x16
            | 0x26
            | 0x27
            | 0x28
            | 0x29
            | 0x31
            | 0x32
            | 0x35
            | 0x36
            | 0x37
            | 0x38
            | 0x39
            | 0x3a
            | 0x3b
            | 0x3c
            | 0x3d
            | 0x3e
            | 0x50
            | 0x61
    )
}

/// Resolves [`BgmDuckingMode`] from `game_mode` (`_DAT_010f0888` / `DAT_00932178`) and `DramaDemoNumber()`
/// (`SeBgmManager` `SLUS_202.28` `0x00194300` / `sh2pc.exe` `0x00516430`).
#[inline]
pub const fn bgm_ducking_mode_from_game_state(
    game_mode: i32,
    drama_demo_id: i32,
) -> BgmDuckingMode {
    if game_mode >= 5 && game_mode <= 9 {
        BgmDuckingMode::PausedOrMenu
    } else if is_bgm_demo_duck_exempt(drama_demo_id) {
        BgmDuckingMode::Normal
    } else {
        BgmDuckingMode::DramaDemo
    }
}

/// Advances `global_bgm_fade` (`_DAT_010e5890` / `_DAT_01f80bd4`) toward `mode.target_volume()` at `dt / 2.0`
/// (`SeBgmManager` `SLUS_202.28` `0x00194300` / `sh2pc.exe` `0x00516430`).
#[inline]
pub fn step_global_bgm_fade(current: f32, mode: BgmDuckingMode, dt_secs: f32) -> f32 {
    let target = mode.target_volume();
    let step = dt_secs.max(0.0) / BGM_DUCK_RAMP_DIVISOR;
    if target < current {
        (current - step).max(target)
    } else if current < target {
        (current + step).min(target)
    } else {
        current
    }
}
