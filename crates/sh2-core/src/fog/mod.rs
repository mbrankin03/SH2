//! 1:1 PS2-accurate room fog parameters, `FogSpeedFix` timing constants, and localized
//! 3D proximity fog particle simulation (`SH2-32`, `SH2-153`).
//!
//! # Ground Truth Citations (`SLUS_202.28`, `sh2pc.exe`, & `Silent-Hill-2-Enhancements`)
//! - **Depth Fog Curve (`sh2gfw_Set_DepthFog` @ `0x00175df0`, `sh2gfw_Set_DepthFogFAFB` @ `0x00175e70`)**:
//!   Computes linear and perspective depth-fog coefficients `(fa, fb)` from `(fog_near, fog_far, fog_min, fog_max)`.
//! - **Screen-Space Multi-Layer Noise Planes (`fog_view_screen_fog` @ `0x0016c1c0`, `fog_asm_data2` @ `0x002ab470`,
//!   `sh2pc.exe` @ `0x0048734C` / `0x0048893D`, `FogFix.cpp` & `Settings.h:183-219`)**:
//!   Dual-layer animated UV-scrolling noise planes with `fog_layer1_x1/x2 = 0.250`, `fog_layer1_y1/y2 = 0.125`,
//!   `fog_layer2_complexity = 0.055`, `fog_layer2_density_add = 100.0`, `fog_layer2_density_mult = 1.4`,
//!   `fog_transparency_layer1 = 128`, `fog_transparency_layer2 = 112`.
//! - **Localized 3D Proximity Particles (`fog_defset_data` @ `0x002ab530`, `fog_asm_data1` @ `0x002ab400`,
//!   `fogInitParticle` @ `0x00169d90`, `fogMoveParticle` @ `0x0016a540`, `fog_part_clamp` @ `0x0016ba50`,
//!   `fog_part_alp` @ `0x0016b9a0`, `fog_part_obj` @ `0x0016b780`, `fogChangeWind` @ `0x00169380`,
//!   `shSway1f_asm` @ `0x0016f0a0`)**:
//!   Default `500` particles (expanded up to `2000` on Observation Deck per `ObservationDeckFogFix.cpp`),
//!   volume half-extent `2000.0` mm (`24500.0` mm X on Observation Deck), border fade `300.0` mm,
//!   and radial parting around James (`JamesFogInfluence = 200.0` mm, reduced to `10.0` mm on the Forest trail
//!   when `1125.0 <= james_y <= 1575.0` per `FogAdjustments.cpp:317-334`).
//! - **Context-Sensitive Room Switching (`FogAdjustments.cpp:34-125, 272-334`)**:
//!   Per-room `FogSpeedFix` drift multipliers (`0.25` outdoor/final boss, `0.50` courtyard/Eddie boss),
//!   Blue Creek Room 209 flashlight near-plane shift (`3000.0 -> 4200.0`), and Final Boss Arena
//!   upper-scaffolding camera elevation switch (`cam_pos_y < -15750.0`).

mod constants;
mod eval;
pub mod particles;
mod types;

#[cfg(test)]
mod tests;

pub use constants::*;
pub use eval::*;
pub use particles::{FogParticle, FogParticleVolume};
pub use types::*;
