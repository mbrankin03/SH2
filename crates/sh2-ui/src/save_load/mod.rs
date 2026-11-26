//! 1:1 PS2 / Retail PC Save & Load Screen (`MC/mc_menu.c` & `sh2pc.exe` `0x0044b3e0`).
//!
//! # Ground Truth References
//! - **PS2 Prototype (`SLUS_202.28`):**
//!   - `mcSaveMenu` (`0x00242c70`), `mcLoadMenu` (`0x00243e20`), `mcSelectData` (`0x00244a50`),
//!     `mcTellYesNo` (`0x00244940`), `mcGetBlinkAlpha` (`0x00245440`), `mcMenuControl` (`0x00244710`).
//!   - `mcDrawMenu` (`0x002456c0`), `mcDrawFrame` (`0x00245770`), `mcDrawSlot` (`0x00245e30`),
//!     `mcDrawPlace` (`0x002467a0`), `mcDrawAddInfo` (`0x00246910`), `mcDrawStatus` (`0x00246c30`),
//!     `mcDrawBG` (`0x00247310`), `mcDrawBGWord` (`0x00247630`).
//!   - `tag_curve1` (`0x0034f270`), `tag_curve2` (`0x0034f280`), `mc_cs_mes` (`0x0034f288`).
//! - **Retail PC (`sh2pc.exe`):**
//!   - `FUN_0044b3e0` (`0x0044b3e0`): loads `data/menu/mc/savebg.tbn2` (Main) or `savebg2.tbn2` (BfW).
//!   - `FUN_0044b520` (`0x0044b520`): `"%4d:%02d"` clamped to `599999` seconds.

mod input;
pub mod render;
pub mod state;

pub use render::{
    draw_save_load_screen, SAVEBG2_TBN2_PATH, SAVEBG_TBN2_PATH, TAG_CURVE_1, TAG_CURVE_2,
};
pub use state::{
    mc_get_blink_alpha, save_point_location_name, SaveAddInfoMode, SaveLoadAction, SaveLoadConfig,
    SaveLoadMode, SaveLoadScreen, SaveLoadStep, YesNoChoice, MC_VISIBLE_ROWS,
};
