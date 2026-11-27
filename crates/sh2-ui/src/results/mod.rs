//! 1:1 PS2 / Retail PC 10-Star Post-Game Results Screen (`SH2_common/result.c` & `Chacter/player_result.c`).
//!
//! # Ground Truth References
//! - **PS2 Prototype (`SLUS_202.28`):**
//!   - `ResultMain` (`0x00293e40`), `clear_screen` (`0x00293d20`), `color1` (`0x00293de0`), `color2` (`0x00293e10`).
//!   - `result_message` table (`0x0037d1b0`, 15 `result_msg_e.mes` IDs `[1, 2, 6, 12..=23]`).
//!   - Format strings `0x0039c520..0x0039c5a8` (`"%d/4(+1)"`, `"%dh %02dm %02ds"`, `"%.2fkm"`, `"%.2fm/s"`, `"%d pt"`).
//!   - `GameSavePreviousTotalRank` (`0x00290bf0`) & `GameSaveSprayPower` (`0x00290c20`).
//! - **Retail PC (`sh2pc.exe`):**
//!   - `0x00539c00..0x0053a1c2` (`RankingRuleset::PcDirectorCut`).

pub mod render;
pub mod state;

pub use render::{
    draw_results_screen, results_render_fingerprint, RESULT_LABEL_X_512, RESULT_MSG_IDS,
    RESULT_ROW_PITCH_512, RESULT_ROW_Y0_512, RESULT_VALUE_X_512,
};
pub use state::{ResultsRowData, ResultsScreen};
