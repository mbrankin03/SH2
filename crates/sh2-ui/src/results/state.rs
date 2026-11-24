//! Data model & state for the 10-Star Post-Game Results Screen (`ResultMain` `0x00293e40`).

use sh2_core::config::{ActionDifficulty, RiddleDifficulty};
use sh2_core::endings::{
    evaluate_star_ranking, ClearEndKind, PlayingInfo, RankingRuleset, StarRankBreakdown,
};

/// Single formatted row on the 15-row Results table (`ResultMain` `0x00293e40`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResultsRowData {
    /// Label text (or fallback for `result_msg_e.mes` ID).
    pub label: &'static str,
    /// Formatted value string matching `0x0039c520..0x0039c5a8`.
    pub value: String,
    /// `true` when `color2()` (`fontSetColor(4)` Yellow) is selected for max category rank.
    pub is_max_rank: bool,
}

/// Interactive state for the 10-Star Results Screen (`ResultMain` `0x00293e40`).
#[derive(Debug, Clone)]
pub struct ResultsScreen {
    /// Computed 100-point (`10.0`-star) breakdown (`GameCalcRankTotal`).
    pub breakdown: StarRankBreakdown,
    /// Active ruleset (`Ps2Original` vs `PcDirectorCut`).
    pub ruleset: RankingRuleset,
    /// Pre-formatted 15 rows matching `ResultMain` (`0x00293e40`).
    pub rows: Vec<ResultsRowData>,
    /// `true` once the user confirms exit (`shPadTrigger` at `0x00293e40`).
    pub exit_requested: bool,
}

impl ResultsScreen {
    /// Builds a [`ResultsScreen`] directly from `PlayingInfo` (`player_result.c` + `ResultMain`).
    #[must_use]
    pub fn from_playing_info(
        info: &PlayingInfo,
        action: ActionDifficulty,
        riddle: RiddleDifficulty,
        ruleset: RankingRuleset,
    ) -> Self {
        let breakdown = evaluate_star_ranking(info, ruleset);
        let rows = build_result_rows(info, action, riddle, &breakdown);
        Self {
            breakdown,
            ruleset,
            rows,
            exit_requested: false,
        }
    }

    /// Confirms and requests return to Title (`ResultMain` `0x00293e40` confirm branch).
    pub fn confirm_exit(&mut self) {
        self.exit_requested = true;
    }
}

/// Formats all 15 rows of `ResultMain` (`0x00293e40`) using exact `SLUS_202.28` unit conversions
/// (`1` PS2 meter = `500.0` mm at `0x00293e40` lines 259, 260, 312).
#[must_use]
pub fn build_result_rows(
    info: &PlayingInfo,
    action: ActionDifficulty,
    riddle: RiddleDifficulty,
    b: &StarRankBreakdown,
) -> Vec<ResultsRowData> {
    let action_str = match action {
        ActionDifficulty::Beginner => "Beginner",
        ActionDifficulty::Easy => "Easy",
        ActionDifficulty::Normal => "Normal",
        ActionDifficulty::Hard => "Hard",
    };
    let riddle_str = match riddle {
        RiddleDifficulty::Easy => "Easy",
        RiddleDifficulty::Normal => "Normal",
        RiddleDifficulty::Hard => "Hard",
        RiddleDifficulty::Extra => "Extra",
    };

    let (ending_str, ending_yellow) = if info.clear_end_kind.contains(ClearEndKind::UFO) {
        ("UFO", true)
    } else if info.clear_end_kind.contains(ClearEndKind::DOG) {
        ("Dog", true)
    } else if info.clear_end_kind.contains(ClearEndKind::REBIRTH) {
        ("Rebirth", true)
    } else if info.clear_end_kind.contains(ClearEndKind::IN_WATER) {
        ("In Water", false)
    } else if info.clear_end_kind.contains(ClearEndKind::MARIA) {
        ("Maria", false)
    } else {
        ("Leave", false)
    };

    let has_extra_end = info.clear_end_kind.contains(ClearEndKind::DOG)
        || info.clear_end_kind.contains(ClearEndKind::UFO);
    let base_endings_count = info.clear_end_kind.0.count_ones();
    let endings_cleared_str = if has_extra_end {
        format!("{}/4(+1)", base_endings_count.saturating_sub(1).min(4))
    } else {
        format!("{}/4", base_endings_count.min(4))
    };

    let total_secs = info.time.max(0.0) as u32;
    let total_time_str = format!(
        "{}h {:02}m {:02}s",
        total_secs / 3600,
        (total_secs / 60) % 60,
        total_secs % 60
    );

    // `SLUS_202.28` `ResultMain` (`0x00293e40` lines 259-260): `(dist / 500.0) / 1000.0` km.
    let walk_km = (info.walk_distance / 500.0) / 1000.0;
    let run_km = (info.run_distance / 500.0) / 1000.0;

    let hidden_count = info.hidden_item_get.count_ones();
    let items_str = if hidden_count == 0 {
        format!("{}", info.item_get)
    } else {
        format!("{} (+{})", info.item_get, hidden_count)
    };

    let boat_secs = info.boat_clear_time.max(0.0) as u32;
    let boat_time_str = if boat_secs < 3600 {
        format!("{}m {:02}s", (boat_secs / 60) % 60, boat_secs % 60)
    } else {
        format!(
            "{}h {:02}m {:02}s",
            boat_secs / 3600,
            (boat_secs / 60) % 60,
            boat_secs % 60
        )
    };

    // `ResultMain` (`0x00293e40` line 312): `boat_max_speed / 500.0` m/s.
    let boat_speed_ms = info.boat_max_speed / 500.0;

    vec![
        ResultsRowData {
            label: "Action Level",
            value: action_str.to_string(),
            is_max_rank: b.action_level_points >= 5,
        },
        ResultsRowData {
            label: "Riddle Level",
            value: riddle_str.to_string(),
            is_max_rank: b.riddle_level_points >= 5,
        },
        ResultsRowData {
            label: "Ending",
            value: ending_str.to_string(),
            is_max_rank: ending_yellow,
        },
        ResultsRowData {
            label: "Endings Cleared",
            value: endings_cleared_str,
            is_max_rank: b.ending_points >= 20,
        },
        ResultsRowData {
            label: "Saves",
            value: format!("{}", info.savecount),
            is_max_rank: b.save_points >= 5,
        },
        ResultsRowData {
            label: "Total Time",
            value: total_time_str,
            is_max_rank: b.clear_time_points >= 10,
        },
        ResultsRowData {
            label: "Walking Distance",
            value: format!("{walk_km:.2}km"),
            is_max_rank: false,
        },
        ResultsRowData {
            label: "Running Distance",
            value: format!("{run_km:.2}km"),
            is_max_rank: false,
        },
        ResultsRowData {
            label: "Items",
            value: items_str,
            is_max_rank: b.item_points >= 10,
        },
        ResultsRowData {
            label: "Defeated Enemy By Shooting",
            value: format!("{}", info.kill_by_shot),
            is_max_rank: b.shooting_kill_points >= 15,
        },
        ResultsRowData {
            label: "Defeated Enemy By Fighting",
            value: format!("{}", info.kill_by_fight),
            is_max_rank: b.fighting_kill_points >= 15,
        },
        ResultsRowData {
            label: "Boat Stage Time",
            value: boat_time_str,
            is_max_rank: b.boat_time_points >= 5,
        },
        ResultsRowData {
            label: "Boat Max Speed",
            value: format!("{boat_speed_ms:.2}m/s"),
            is_max_rank: false,
        },
        ResultsRowData {
            label: "Total Damage",
            value: format!("{} pt", info.jms_damage_total as i32),
            is_max_rank: b.damage_points >= 5,
        },
        ResultsRowData {
            label: "Your Rank",
            value: String::new(), // Rendered as Big + Small stars in `render.rs`
            is_max_rank: true,
        },
    ]
}
