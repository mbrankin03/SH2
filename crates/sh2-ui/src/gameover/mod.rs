//! 1:1 PS2 / Retail PC Game Over Screen (`Event/title.c` `GameoverMain` `0x001c4c70`).
//!
//! # Ground Truth References
//! - **PS2 Prototype (`SLUS_202.28`):**
//!   - `GameoverMain` (`0x001c4c70`): initializes `opd_work_init`, fades in, and ticks `draw_opd_wowk_main`.
//!   - `opd_work_init` (`0x001c4a70`): initializes 6 `opd_w` strips (`0..160`, `48`-px bands) and
//!     `org_title` (`u=0..160, v=0..48`, centered screen rect `-80..80, -24..23` in `512x448`).
//!   - `draw_opd_wowk_main` (`0x001c4de0`): 4-stage alpha/timer FSM (`32` frame delay, `+4` fade-in
//!     to `0x60`, `64` frame hold, `-8` fade-out to `0`) with `rand() % 30 - 15` strip jitter.
//! - **Retail PC (`sh2pc.exe`):**
//!   - `FUN_00497ad0` (`0x00497ad0`): loads `data/pic/etc/gameover1.tex` and emits `SE 0x3f4` / `0xeaef`.

pub mod render;
pub mod state;

pub use render::{draw_gameover_screen, GAMEOVER_TEX_PATH};
pub use state::{
    GameOverPhase, GameOverScreen, GAMEOVER_DELAY_FRAMES, GAMEOVER_FADE_IN_STEP,
    GAMEOVER_FADE_OUT_STEP, GAMEOVER_HOLD_FRAMES, GAMEOVER_MAX_PS2_ALPHA, GAMEOVER_STRIP_COUNT,
    GAMEOVER_STRIP_JITTER_RANGE,
};
