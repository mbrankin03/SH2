//! Unified Silent Hill 2 Collision Subsystem (1:1 PS2 `Collision/cl_main.c` / `cl_calc.c` Parity).
//!
//! Ground truth and architecture:
//! - `clAllInitCollisionData` / `clFrameInitCollisionData`: Lifecycle initialization and per-frame reset
//!   of transient dynamic obstacles, weapon hits, and collision work buffers.
//! - `clGetHitSectListMOVE` (`InDoor` / `OutDoor`): Authentic 4x4 spatial grid acceleration partitioning
//!   static rooms and open town blocks into 16 sectors using `.cld` index tables (`b0ofs`..`clofs`).
//! - `clCheckBg2Chara` & `col_push`: 3-iteration continuous cylinder-to-wall pushback and surface-normal sliding.
//! - `clCollectCharaHeightNormal` & `col_floor_climb`: Vertical floor height sampling in $XZ$ barycentric space
//!   with step-up and step-down thresholds (world $+Y$ down).
//! - `clCheckColumn2ColumnHit`: Multi-character cylinder-to-cylinder elastic separation (James vs. Maria / enemies).
//! - `clCheckHitEyes` & `col_clamp_ray`: Camera line-of-sight raycast clamping with safety offset to prevent
//!   wall clipping in 3rd-person view.
//! - `clAddDynamicWall` / `clAddDynamicFloor`: Transient stage obstacles (moving doors, elevator platforms, falling cages).
//! - `col_spawn`: Safe entrance coordinate calculation snapping to the centroid of the nearest valid floor polygon.
//! - `col_merge`: Seamless merging of adjacent rooms or town blocks during door traversal and world streaming.

pub mod character;
pub mod controller;
pub mod dynamic;
pub mod grid;
pub mod raycast;

pub use character::{CharacterCylinder, CharacterId};
pub use controller::{
    CollisionConfig, CollisionController, CollisionWorld, FloorSample, SurfaceMaterial,
};
pub use dynamic::{DynamicFloor, DynamicWall};
pub use grid::SpatialGrid;
pub use raycast::{RayHit, RaycastFilter};
