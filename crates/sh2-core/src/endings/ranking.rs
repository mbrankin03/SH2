//! 10-Star Post-Game Ranking & Hyper Spray Power Tier Calculator (`player_result.c`).
//!
//! Verified 1:1 against:
//! - PS2 Prototype ELF (`SLUS_202.28`, `src/Chacter/player_result.c`):
//!   - `GameCalcRankSaveCount` (`0x00290550`)
//!   - `GameCalcRankEndingKind` (`0x002905d0`)
//!   - `GameCalcRankBattleLevel` (`0x00290640`)
//!   - `GameCalcRankRiddleLevel` (`0x002906f0`)
//!   - `GameCalcRankItemGet` (`0x00290780`)
//!   - `GameCalcRankHiddenItemGet` (`0x002907d0`)
//!   - `GameCalcRankKillByShot` (`0x00290820`)
//!   - `GameCalcRankKillByFight` (`0x00290870`)
//!   - `GameCalcRankClearTime` (`0x002908c0`)
//!   - `GameCalcRankBoatClearTime` (`0x002909a0`)
//!   - `GameCalcRankJamesDamage` (`0x00290a70`)
//!   - `GameCalcRankTotal` (`0x00290b40`)
//!   - `GameSaveSprayPower` (`0x00290c20`)
//! - Retail PC Binary (`sh2pc.exe` v1.0 Director's Cut):
//!   - `0x00539c00..0x0053a1c2` (`player_result.c` retail 100-point / 10.0-star evaluator).

use super::playing_info::{ClearEndKind, PlayingInfo};

/// Maximum total star score (`100` = `10` Big Stars, `0x64` at `0x0053a19d`).
pub const MAX_RANK_POINTS: u8 = 100;

/// Retail PC rank cutoff (`rank <= 20` = `2.0` stars -> Purple Hyper Spray `-1` at `0x0053a185`).
pub const PC_PURPLE_SPRAY_MAX_RANK: u8 = 20;
/// PS2 July 2001 prototype rank cutoff (`rank < 6` -> Purple Hyper Spray `-1` at `0x00290c28`).
pub const PS2_PROTO_PURPLE_SPRAY_MAX_RANK: u8 = 5;
/// Rank threshold for Yellow Hyper Spray (`rank >= 80` = `8.0` stars -> `spray_pow = 1` at `0x0053a191`).
pub const YELLOW_SPRAY_MIN_RANK: u8 = 80;
/// Rank threshold for Green Hyper Spray (`rank >= 100` = `10.0` stars -> `spray_pow = 2` at `0x0053a19d`).
pub const GREEN_SPRAY_MIN_RANK: u8 = 100;

/// Selects between strict original PS2 save-count ranking and PC Director's Cut ranking rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RankingRuleset {
    /// Retail PS2 (`SLUS_202.28` `0x00290b40` + retail `<= 20` Purple Spray threshold):
    /// `GameCalcRankSaveCount` (`0..=5` points based on `savecount`).
    Ps2Original,
    /// Retail PC Director's Cut (`sh2pc.exe` `0x00539fa0` & `0x0053a180`):
    /// Awards a flat `+5` save-count bonus (`leal 0x5(%eax,%edx), %eax` at `0x0053a0d7`) because PC allows quick-saves.
    #[default]
    PcDirectorCut,
}

/// Per-category breakdown of the 100-point (`10.0`-star) result screen evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StarRankBreakdown {
    /// Save count score (`0..=5`, `0x00290550` / `0x00539c00` or flat `5` on PC `0x0053a0d7`).
    pub save_points: u8,
    /// Ending completion score (`4` per distinct main/special ending up to `20`, `0x00539c50`).
    pub ending_points: u8,
    /// Action difficulty score (`0, 1, 3, 5`, `0x00539ca0`).
    pub action_level_points: u8,
    /// Riddle difficulty score (`1, 3, 5, 5`, `0x00539ce0`).
    pub riddle_level_points: u8,
    /// Combined standard + hidden item pickup score (`0..=15`, `0x00539d20`).
    pub item_points: u8,
    /// Shooting kills score (`0..=15`, `0x00539db0`).
    pub shooting_kill_points: u8,
    /// Fighting kills score (`0..=15`, `0x00539de0`).
    pub fighting_kill_points: u8,
    /// Clear time score (`0..=10`, `0x00539e10`).
    pub clear_time_points: u8,
    /// Boat stage time score (`0..=5`, `0x00539ea0`).
    pub boat_time_points: u8,
    /// Total James damage score (`0..=5`, `0x00539f20`).
    pub damage_points: u8,
    /// Total rank points (`0..=100`, where `total / 10` = Big Stars and `total % 10` = Small Stars).
    pub total_points: u8,
    /// Resulting Hyper Spray power tier (`-1` Purple, `0` White, `1` Yellow, `2` Green).
    pub spray_pow: i8,
}

impl StarRankBreakdown {
    /// Number of Big Stars (`0..=10`).
    #[inline]
    pub const fn big_stars(self) -> u8 {
        self.total_points / 10
    }

    /// Number of Small Stars (`0..=9`).
    #[inline]
    pub const fn small_stars(self) -> u8 {
        self.total_points % 10
    }
}

/// Evaluates `GameCalcRankSaveCount` (`SLUS_202.28` `0x00290550` / `sh2pc.exe` `0x00539c00`).
pub const fn calc_rank_save_count(savecount: u16) -> u8 {
    if savecount <= 2 {
        5
    } else if savecount <= 5 {
        4
    } else if savecount <= 10 {
        3
    } else if savecount <= 20 {
        2
    } else if savecount <= 30 {
        1
    } else {
        0
    }
}

/// Evaluates `GameCalcRankEndingKind` (`sh2pc.exe` `0x00539c50`).
///
/// Each of the 6 main-scenario ending bits (`0x01..=0x20`) contributes `+4` points, capped at `20` (`4/4 (+2)`).
pub const fn calc_rank_ending_kind(clear_end_kind: ClearEndKind) -> u8 {
    let points = clear_end_kind.main_scenario_endings_count() * 4;
    if points > 20 {
        20
    } else {
        points
    }
}

/// Evaluates `GameCalcRankBattleLevel` (`sh2pc.exe` `0x00539ca0`, jump table `0x0053a0e0`).
///
/// `0` (Beginner) -> `0`, `1` (Easy) -> `1`, `2` (Normal) -> `3`, `3` (Hard) -> `5`.
pub const fn calc_rank_battle_level(battle_level: u8) -> u8 {
    match battle_level {
        1 => 1,
        2 => 3,
        3 => 5,
        _ => 0,
    }
}

/// Evaluates `GameCalcRankRiddleLevel` (`sh2pc.exe` `0x00539ce0`, jump table `0x0053a0f0`).
///
/// `0` (Easy) -> `1`, `1` (Normal) -> `3`, `2` (Hard) -> `5`, `3` (Extra) -> `5`.
pub const fn calc_rank_riddle_level(riddle_level: u8) -> u8 {
    match riddle_level {
        0 => 1,
        1 => 3,
        2 | 3 => 5,
        _ => 1,
    }
}

/// Evaluates retail `GameCalcRankItemGet` (`sh2pc.exe` `0x00539d20`).
///
/// Computes `(item_get / 20) + hidden_item_get.count_ones()`, capped at `15`.
pub const fn calc_rank_item_get(item_get: u16, hidden_item_get: u8) -> u8 {
    let total = (item_get / 20) as u32 + hidden_item_get.count_ones();
    if total > 15 {
        15
    } else {
        total as u8
    }
}

/// Evaluates `GameCalcRankKillByShot` / `GameCalcRankKillByFight` (`sh2pc.exe` `0x00539db0` & `0x00539de0`).
///
/// Computes `kills / 5`, capped at `15` (`>= 75` kills).
pub const fn calc_rank_kills(kills: u16) -> u8 {
    if kills > 75 {
        15
    } else {
        (kills / 5) as u8
    }
}

/// Evaluates `GameCalcRankClearTime` (`sh2pc.exe` `0x00539e10`).
///
/// Converts `time_secs` to minutes (`time_secs * (1.0 / 60.0)` at `0x006b3518`):
/// `<= 180` min (`3h`) -> `10`, `<= 240` (`4h`) -> `5`, `<= 360` (`6h`) -> `3`,
/// `<= 480` (`8h`) -> `2`, `<= 960` (`16h`) -> `1`, `> 960` -> `0`.
pub fn calc_rank_clear_time(time_secs: f32) -> u8 {
    let minutes = time_secs.max(0.0) * (1.0 / 60.0);
    if minutes <= 180.0 {
        10
    } else if minutes <= 240.0 {
        5
    } else if minutes <= 360.0 {
        3
    } else if minutes <= 480.0 {
        2
    } else if minutes <= 960.0 {
        1
    } else {
        0
    }
}

/// Evaluates `GameCalcRankBoatClearTime` (`sh2pc.exe` `0x00539ea0`).
///
/// `<= 80` s (`1m20s`) -> `5`, `<= 160` s (`2m40s`) -> `4`, `<= 240` s (`4m`) -> `3`,
/// `<= 480` s (`8m`) -> `2`, `<= 960` s (`16m`) -> `1`, `> 960` s -> `0`.
pub fn calc_rank_boat_time(boat_clear_time_secs: f32) -> u8 {
    let t = boat_clear_time_secs.max(0.0);
    if t <= 80.0 {
        5
    } else if t <= 160.0 {
        4
    } else if t <= 240.0 {
        3
    } else if t <= 480.0 {
        2
    } else if t <= 960.0 {
        1
    } else {
        0
    }
}

/// Evaluates `GameCalcRankJamesDamage` (`sh2pc.exe` `0x00539f20` / `SLUS_202.28` `0x00290a70`).
///
/// `<= 500` -> `5`, `<= 1000` -> `4`, `<= 1500` -> `3`, `<= 2000` -> `2`, `<= 2500` -> `1`, `> 2500` -> `0`.
pub fn calc_rank_james_damage(jms_damage_total: f32) -> u8 {
    let dmg = jms_damage_total.max(0.0);
    if dmg <= 500.0 {
        5
    } else if dmg <= 1000.0 {
        4
    } else if dmg <= 1500.0 {
        3
    } else if dmg <= 2000.0 {
        2
    } else if dmg <= 2500.0 {
        1
    } else {
        0
    }
}

/// Resolves `playing.spray_pow` (`-1..=2`) from `rank` (`0..=100`) per `GameSaveSprayPower` (`0x0053a180` PC).
pub const fn resolve_spray_power(rank: u8) -> i8 {
    if rank <= PC_PURPLE_SPRAY_MAX_RANK {
        -1
    } else if rank < YELLOW_SPRAY_MIN_RANK {
        0
    } else if rank < GREEN_SPRAY_MIN_RANK {
        1
    } else {
        2
    }
}

/// Computes the complete 100-point (`10.0`-star) ranking and Hyper Spray tier (`0x00539fa0` & `0x0053a180`).
pub fn evaluate_star_ranking(playing: &PlayingInfo, ruleset: RankingRuleset) -> StarRankBreakdown {
    let save_points = match ruleset {
        RankingRuleset::Ps2Original => calc_rank_save_count(playing.savecount),
        RankingRuleset::PcDirectorCut => 5,
    };
    let ending_points = calc_rank_ending_kind(playing.clear_end_kind);
    let action_level_points = calc_rank_battle_level(playing.battle_level);
    let riddle_level_points = calc_rank_riddle_level(playing.riddle_level);
    let item_points = calc_rank_item_get(playing.item_get, playing.hidden_item_get);
    let shooting_kill_points = calc_rank_kills(playing.kill_by_shot);
    let fighting_kill_points = calc_rank_kills(playing.kill_by_fight);
    let clear_time_points = calc_rank_clear_time(playing.time);
    let boat_time_points = calc_rank_boat_time(playing.boat_clear_time);
    let damage_points = calc_rank_james_damage(playing.jms_damage_total);

    let sum = save_points as u16
        + ending_points as u16
        + action_level_points as u16
        + riddle_level_points as u16
        + item_points as u16
        + shooting_kill_points as u16
        + fighting_kill_points as u16
        + clear_time_points as u16
        + boat_time_points as u16
        + damage_points as u16;
    let total_points = sum.min(MAX_RANK_POINTS as u16) as u8;
    let spray_pow = resolve_spray_power(total_points);

    StarRankBreakdown {
        save_points,
        ending_points,
        action_level_points,
        riddle_level_points,
        item_points,
        shooting_kill_points,
        fighting_kill_points,
        clear_time_points,
        boat_time_points,
        damage_points,
        total_points,
        spray_pow,
    }
}
