//! Unified Silent Hill 2 Collision Subsystem (1:1 PS2 `Collision/cl_main.c` / `cl_calc2.c` Parity).
//!
//! Ground truth and architecture:
//! - `clAllInitCollisionData` / `clFrameInitCollisionData`: Lifecycle initialization and per-frame reset
//!   of transient dynamic obstacles, weapon hits, and collision work buffers.
//! - `clGetHitSectListMOVE` (`InDoor` / `OutDoor`): Authentic 4x4 spatial grid acceleration partitioning
//!   static rooms (cell 0) and open town blocks (`30,000^2` / `8,000^2` culling) using `.cld` index tables (`b0ofs`..`clofs`).
//! - `PlayerSetColumn_SetTarget` / `PlayerSetColumn_CloseToTarget`: Dual `move_col` (`800mm`, `[-50..-850]`,
//!   `175mm` James / `150mm` Maria) and `hit_col` (`900mm`, `[0..-900]`, `150mm` James / `125mm` Maria) cylinders
//!   with `+25mm` forward offset and `5.0 mm/tick` convergence.
//! - `clCheckBg2Chara`, `clCheckColumn2WallHit`, `clCheckColumn2ColumnHit`, `clMakeWallHitCollectVector`,
//!   `clAddWallCollectVector`: One-sided plane backface culling, face/corner/BG-column hits, 89/75/2/120 deg
//!   suppression, and `1/16` fixed-point quantization.
//! - `clCollectCharaALL`: 5 interleaved BG + character-to-character iterations with discrete weight-diff table
//!   `[1.0, 0.98, 0.94, 0.90, 0.86]`.
//! - `clCollectCharaHeightNormal`: Vertical floor height sampling in XZ barycentric space.
//! - `clCheckHitEyes`: Camera line-of-sight raycast clamping and acoustic occlusion.
//! - `clAddDynamicWall` / `clAddDynamicFloor`: Transient stage obstacles.

pub mod character;
pub mod controller;
pub mod dynamic;
pub mod eye_ray;
pub mod grid;
pub mod probes;
pub mod raycast;
pub mod solver;

pub use character::{
    close_to_value, CharacterCylinder, CharacterId, CollisionColumn, PlayerColumnAction,
    PlayerColumnState, PlayerColumnTarget, JAMES_HIT_RADIUS, JAMES_MOVE_RADIUS, MARIA_HIT_RADIUS,
    MARIA_MOVE_RADIUS, PLAYER_BASE_FORWARD_OFFSET, PLAYER_COLUMN_CLOSE_STEP, PLAYER_HIT_COL_HEIGHT,
    PLAYER_HIT_COL_Y_BOTTOM_OFFSET, PLAYER_HIT_COL_Y_TOP_OFFSET, PLAYER_MOVE_COL_HEIGHT,
    PLAYER_MOVE_COL_Y_BOTTOM_OFFSET, PLAYER_MOVE_COL_Y_TOP_OFFSET, PLAYER_WEAPON_COLUMN_TABLE,
};
pub use controller::{
    CollisionConfig, CollisionController, CollisionSector, CollisionWorld, FloorSample,
    SurfaceMaterial,
};
pub use dynamic::{DynamicFloor, DynamicWall};
pub use grid::{
    SpatialGrid, OUTDOOR_BLOCK_CULL_DIST_SQ, OUTDOOR_CELL_CULL_DIST_SQ, OUTDOOR_CELL_SIZE,
};
pub use probes::{
    cl_collect_chara_position_y_smooth, player_check_foot_line, player_check_side_line,
    player_check_straight_line, player_set_height, FootLineProbeResult, PlayerHeightResult,
    PlayerHeightState, SideLineProbeResult, StraightLineProbeResult,
    CL_POSITION_Y_SMOOTH_SPEED_MM_PER_SEC, PLAYER_FALL_STATE_THRESHOLD_MM,
    PLAYER_FOOT_LINE_DOWN_MM, PLAYER_FOOT_LINE_UP_MM, PLAYER_FREEFALL_GRAVITY_ACCEL,
    PLAYER_HEIGHT_RAY_DOWN_NORMAL, PLAYER_HEIGHT_RAY_STAGE_0E, PLAYER_HEIGHT_RAY_UP_NORMAL,
    PLAYER_SIDE_LINE_LEN_MM, PLAYER_SIDE_LINE_Y_OFFSET_MM, PLAYER_STEP_SNAP_THRESHOLD_MM,
    PLAYER_STRAIGHT_LINE_LEN_MM, PLAYER_STRAIGHT_LINE_LOW_Y_OFFSET_MM,
    PLAYER_STRAIGHT_LINE_SLOPE_RAD, PLAYER_WALL_AHEAD_DIST_MM, STAGE_ID_EXTENDED_HEIGHT_RAY,
};
pub use raycast::{
    cl_check_sub_line_to_column_plus, cl_check_sub_line_to_plane, cld_material_barrier_weight,
    intersect_ray_triangle_3d, intersect_segment_bg_column, is_cld_material_thru,
    listener_head_position, raycast_acoustic_occlusion, should_skip_material_for_mode,
    AcousticOcclusionHit, EyeHitKind, EyeRayHit, EyeRayMode, RayHit, RaycastFilter,
    JAMES_HEAD_OFFSET_Y, MARIA_HEAD_OFFSET_Y, MIN_DISTINCT_BARRIER_SEPARATION_UNITS,
};
pub use solver::{
    cl_add_collect_vector, cl_add_wall_collect_vector, cl_check_column_to_bg_column_hit,
    cl_check_column_to_wall_hit, cl_check_sub_column_to_column, cl_check_sub_wall_to_column,
    cl_collect_chara_all, cl_make_wall_hit_collect_vector, quantize_collect_vector_1_16,
    regulate_angle, sh_atan2, weight_diff_recoil_factor, CharaCollectEntry, WallCollectContext,
    WallHitKind, WallHitRecord, CL_COLLECT_CHARA_ITERATIONS, CL_CORNER_SUPPRESS_RAD,
    CL_MAX_WALL_HITS, CL_NO_MOVE_ANGLE, CL_PARALLEL_CORNER_SUPPRESS_RAD, CL_PINCH_SUPPRESS_RAD,
    CL_POST_FLAG_CLAMP_RAD, CL_POST_FLAG_CLAMP_SPEED, CL_TRAILING_NORMAL_SUPPRESS_RAD,
    CL_WEIGHT_DIFF_TABLE,
};
