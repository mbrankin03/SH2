//! Shadow rendering subsystem (`SH2-81`, `SH2-136`):
//! - [`volume`]: CPU `.kg1`/`.kg2` stencil shadow volume extrusion (`0x444b20` / `0x4449f0`)
//!   and floor drop-shadow flattening (`0x5a7c90`).
//! - [`stencil`]: Two-pass Z-fail stencil shadow volume & drop-shadow GPU pipeline (`0x445540`).
//! - [`blur`]: PS2 4-pass ping-pong screen-space soft stencil shadow blur (`0x001767e0`) and
//!   dual GS composite blend modes (`0x00175f90`: `0x8000000052` vs `0x8000000046`).
//! - [`draw_order`]: `Check_DrawOrder` (`0x001a08a0`), map shadow light overrides (`0x001a6600`),
//!   James self-shadow part exclusion (`0x001a5e60`/`0x001a5f60`), and height drop-shadow alpha (`0x0018dc50`).
//! - [`pcss`]: Native Enhanced `2048x2048` GPU PCSS spotlight shadow map (static map + 4-bone LBS
//!   skeletal self-shadowing) with contact-hardening penumbras.

pub mod blur;
pub mod draw_order;
pub mod pcss;
pub mod stencil;
pub mod volume;

pub use blur::{
    evaluate_ps2_shadow_color, ShadowBlurPipeline, ShadowBlurUniforms, ShadowCompositeMode,
    GS_SHADOW_ALPHA_MULTIPLICATIVE, GS_SHADOW_ALPHA_SUBTRACTIVE, GS_SHADOW_TEST_MULTIPLICATIVE,
    GS_SHADOW_TEST_SUBTRACTIVE, PS2_DEFAULT_ROOM_SHADOW_COLOR, PS2_SHADOW_BLUR_PASS_COUNT,
    PS2_SHADOW_WORK_SIZE,
};
pub use draw_order::{
    check_draw_order, compute_drop_shadow_height_range, drop_shadow_height_bones,
    evaluate_drop_shadow_height_alpha, map_shadow_light_override, DropShadowHeightBones,
    JamesSelfShadowExclusion, ShadowDrawOrder, ShadowLightOverride, DRAW_ORDER_SHADOW_FIRST_MAPS,
    DROP_SHADOW_HEIGHT_WINDOW_MM, JAMES_HEAD_EXCLUDED_PARTS, JAMES_UPPER_BODY_EXCLUDED_PARTS,
};
pub use pcss::{
    compute_pcss_spotlight_view_proj, PcssShadowPipeline, PcssShadowUniforms, PCSS_SHADOW_MAP_SIZE,
    WGSL_PCSS_FUNCTIONS,
};
pub use stencil::{ShadowDarkenUniforms, StencilShadowPipeline, MAX_SHADOW_VOLUME_VERTICES};
pub use volume::{
    extrude_kg1_stencil_volume, extrude_kg1_stencil_volume_filtered,
    extrude_kg1_stencil_volume_filtered_into, extrude_kg2_stencil_volume,
    extrude_kg2_stencil_volume_into, flatten_kg1_drop_shadow, ShadowVolumeScratch,
    ShadowVolumeVertex, DEFAULT_SHADOW_EXTRUDE_DIST_MM, DROP_SHADOW_FLOOR_BIAS_MM,
    MAX_KG1_SHADOW_CASTERS,
};
