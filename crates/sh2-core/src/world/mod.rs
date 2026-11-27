//! Outdoor World Grid & Tile Streaming (`LoadBg/loadbg_2x2.c` & `loadbg_all.c`).
//!
//! Verified 1:1 against PS2 prototype `SLUS_202.28`:
//! - `BlockNumber` (`0x001b09f0`) & `BgIsOut` (`0x001b0cd0`)
//! - `loadBgCommon_GetBlockNoOutdoor` (`0x00251dc0`)
//! - `loadBg2x2_SetRequestOutdoor` (`0x0026fe80`)
//! - `loadBgAll_Init` (`0x00270ad0`), `loadBgAll_SetLoadRequestOutdoor2x2` (`0x00270dd0`),
//!   and `loadBgAll_PrepareAround` (`0x002715d0`).

pub mod bg_obj;
pub mod cockroach_swarm;
pub mod elevator_panel;
pub mod loadbg;
pub mod outdoor_grid;
pub mod pushable_objects;
pub mod room_pickups;

pub use bg_obj::{
    bg_chara_block_count, bg_chara_is_id, chara_bound_radius_mm, is_within_outdoor_prop_range,
    BgCharaSlots, BgObjectState, MirrorMode, MirrorPlane, StayObjectState,
    BG_CHARA_BLOCK_SIZE_BYTES, BG_OBJECT_SHADOW_Y_OFFSET_MM, DEFAULT_CHARA_BOUND_R_MM,
    MIRROR_WALL_POS_ROOM_01_X_MM, MIRROR_WALL_POS_ROOM_24_Z_MM, MODEL_ID_SPECIAL_PI_YAW,
    OUTDOOR_PROP_CULL_MARGIN_MM, STAY_OBJECT_SHADOW_Y_OFFSET_MM,
};
pub use cockroach_swarm::{
    evaluate_roach_jiggle, CockroachDrawAction, CockroachSwarmState, RoachJiggleParams,
    CHARA_ID_TY2, CHARA_ID_TY3, CHARA_ID_TYU, COCKROACH_TIMER_SEED_MAX_SEC, MAX_COCKROACH_TIMERS,
    R_BUG_RM_ID, TY2_BODY_NUM_VERTICES, TY2_BODY_PRIM_COUNT, TY2_LIMB_PRIM_COUNT,
    TY3_BODY_NUM_VERTICES, TY3_BODY_PRIM_COUNT, TY3_LIMB_PRIM_COUNTS, TY3_SHADOW_NUM_VERTICES,
    TY3_SHADOW_PRIM_COUNT, VANILLA_ROACH_JIGGLE_POS_MAX, VANILLA_ROACH_JIGGLE_POS_MIN,
    VANILLA_ROACH_JIGGLE_YAW_MAX, VANILLA_ROACH_JIGGLE_YAW_MIN,
};
pub use elevator_panel::{
    hospital_chase_elevator_stab_param, hotel_employee_elevator_cursor_color, ElevatorCursorColor,
    ElevatorFloor, ElevatorKind, MultiFloorElevatorPanel, HOSPITAL_CHASE_STAB_PARAM_FIXED,
    HOSPITAL_CHASE_STAB_X_THRESHOLD_MM, ROOM_HSP_ALT_RPT_HALLWAY,
};
pub use loadbg::{
    OutdoorTileStreamer, TileLoadRequest, TileStreamingPlan, LOADBG_DIST_BANDS_MM,
    LOADBG_PREFETCH_CUTOFF_MM,
};
pub use outdoor_grid::{
    OutdoorArea, OutdoorTileId, CA_OUTDOOR_BLOCKS, CB_OUTDOOR_BLOCKS, CC_OUTDOOR_BLOCKS,
    CD_OUTDOOR_BLOCKS, OUTDOOR_GRID_OFFSET, OUTDOOR_TILE_HALF_MM, OUTDOOR_TILE_SIZE_MM,
};
pub use pushable_objects::{
    event_main_sys_flag_mask, PushableObjectKind, FLAG_APART_CLOCK_PUSHED, FLAG_APART_CLOCK_SOLVED,
    FLAG_HOSPITAL_B1_SHELF_PUSHED, FLASHLIGHT_CLOCK_PUSH_CAM_Y_MM, ROOM_APT_E_RM_208,
    SYS_FLAG_MASK_CLOCK_PUSH_FIX, SYS_FLAG_MASK_VANILLA,
};
pub use room_pickups::{
    resolve_active_room_pickups, resolve_active_room_pickups_into, ActiveRoomPickup,
};
