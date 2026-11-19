//! Outdoor World Grid & Tile Streaming (`LoadBg/loadbg_2x2.c` & `loadbg_all.c`).
//!
//! Verified 1:1 against PS2 prototype `SLUS_202.28`:
//! - `BlockNumber` (`0x001b09f0`) & `BgIsOut` (`0x001b0cd0`)
//! - `loadBgCommon_GetBlockNoOutdoor` (`0x00251dc0`)
//! - `loadBg2x2_SetRequestOutdoor` (`0x0026fe80`)
//! - `loadBgAll_Init` (`0x00270ad0`), `loadBgAll_SetLoadRequestOutdoor2x2` (`0x00270dd0`),
//!   and `loadBgAll_PrepareAround` (`0x002715d0`).

pub mod loadbg;
pub mod outdoor_grid;

pub use loadbg::{
    OutdoorTileStreamer, TileLoadRequest, TileStreamingPlan, LOADBG_DIST_BANDS_MM,
    LOADBG_PREFETCH_CUTOFF_MM,
};
pub use outdoor_grid::{
    OutdoorArea, OutdoorTileId, CA_OUTDOOR_BLOCKS, CB_OUTDOOR_BLOCKS, CC_OUTDOOR_BLOCKS,
    CD_OUTDOOR_BLOCKS, OUTDOOR_GRID_OFFSET, OUTDOOR_TILE_HALF_MM, OUTDOOR_TILE_SIZE_MM,
};
