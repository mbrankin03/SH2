//! Prison Bug Room (`R_BUG_RM = 0x65`) 3D creeping cockroach swarm replacement
//! (`Patches/CockroachesReplacement.cpp`).
//!
//! Ground truth:
//! - `SLUS_202.28` `Event/stg_name.c` `RoomNameDelusion` (`0x001afcb0` -> `0x65`)
//! - `SLUS_202.28` `Enemy/en_common.c` (`0x00147370` line 3927, `R_BUG_RM = 0x65` swarm flag)
//! - `SLUS_202.28` `Enemy/en_tyu.c` (`enTYUInitData` `0x00165000`, `enTY2InitData` `0x00167ee0`,
//!   `enTY3InitData` `0x001683e0`) & `Chacter/m3_ty23.c` (`EnemyTY23Function` `0x00291510`)
//! - `sh2pc.exe` `FUN_004ae350` (`0x004ae350`) cockroach position/yaw jiggle & `0x004ae4b0`
//!   (`CharaWorkCreate(0x202, ...)`, max `0x0C` = 12 active)
//! - SH2:EE `Patches/CockroachesReplacement.cpp` (`0x004AE37C`, `0x004AE3BB`, `sh2e/model/ty3.glb`).

/// Room ID for the Prison Bug Room (`R_BUG_RM = 0x65`, `SLUS_202.28` `RoomNameDelusion` `0x001afcb0`).
pub const R_BUG_RM_ID: u32 = 0x65;

/// Character ID for `data/chr/item/ty2.mdl` (`524`, `SLUS_202.28` `enTY2InitData` `0x00167ee0`).
pub const CHARA_ID_TY2: u16 = 0x20C;

/// Character ID for `data/chr/item/ty3.mdl` (`525`, `SLUS_202.28` `enTY3InitData` `0x001683e0`).
pub const CHARA_ID_TY3: u16 = 0x20D;

/// Character ID for creeping floor/wall/ceiling cockroach (`514`, `SLUS_202.28` `enTYUInitData`
/// `0x00165000`, `sh2pc.exe` `0x004ae4b0`).
pub const CHARA_ID_TYU: u16 = 0x202;

/// Maximum number of per-cockroach animation phase timers (`CockroachesReplacement.cpp:15`).
pub const MAX_COCKROACH_TIMERS: usize = 32;

/// Upper bound (exclusive) in seconds for randomized cockroach animation phase timers
/// (`CockroachesReplacement.cpp:58`: `std::uniform_real_distribution<float>(0.0f, 8.0f)`).
pub const COCKROACH_TIMER_SEED_MAX_SEC: f32 = 8.0;

/// Vanilla `sh2pc.exe` `FUN_004ae350` (`0x004AE37C`) X/Z jiggle lower bound (`0xC0A00000` = `-5.0` mm).
pub const VANILLA_ROACH_JIGGLE_POS_MIN: f32 = -5.0;

/// Vanilla `sh2pc.exe` `FUN_004ae350` (`0x004ae381`) X/Z jiggle upper bound (`0x3F800000` = `1.0` mm).
pub const VANILLA_ROACH_JIGGLE_POS_MAX: f32 = 1.0;

/// Vanilla `sh2pc.exe` `FUN_004ae350` (`0x004ae3c0`) yaw jiggle lower bound (`0xBC8EFA35` ≈ `-0.017453292` rad).
pub const VANILLA_ROACH_JIGGLE_YAW_MIN: f32 = -0.017_453_292;

/// Vanilla `sh2pc.exe` `FUN_004ae350` (`0x004AE3BB`) yaw jiggle upper bound (`0x3DCCCCCD` = `0.1` rad).
pub const VANILLA_ROACH_JIGGLE_YAW_MAX: f32 = 0.1;

/// Vanilla `ty3.mdl` body DrawIndexedPrimitive signature (`CockroachesReplacement.cpp:100`).
pub const TY3_BODY_NUM_VERTICES: u32 = 161;
/// Vanilla `ty3.mdl` body primitive count (`CockroachesReplacement.cpp:100`).
pub const TY3_BODY_PRIM_COUNT: u32 = 336;
/// Vanilla `ty3.mdl` shadow DrawIndexedPrimitive signature (`CockroachesReplacement.cpp:102`).
pub const TY3_SHADOW_NUM_VERTICES: u32 = 189;
/// Vanilla `ty3.mdl` shadow primitive count (`CockroachesReplacement.cpp:102`).
pub const TY3_SHADOW_PRIM_COUNT: u32 = 560;
/// Vanilla `ty2.mdl` body DrawIndexedPrimitive signature (`CockroachesReplacement.cpp:101`).
pub const TY2_BODY_NUM_VERTICES: u32 = 23;
/// Vanilla `ty2.mdl` body primitive count (`CockroachesReplacement.cpp:101`).
pub const TY2_BODY_PRIM_COUNT: u32 = 48;
/// Vanilla `ty3.mdl` limb DrawPrimitive counts (`CockroachesReplacement.cpp:136`).
pub const TY3_LIMB_PRIM_COUNTS: [u32; 2] = [256, 160];
/// Vanilla `ty2.mdl` limb DrawPrimitive count when bound texture is DXT3 (`CockroachesReplacement.cpp:138-147`).
pub const TY2_LIMB_PRIM_COUNT: u32 = 96;

/// Jiggle amplitudes applied to cockroach actors (`sh2pc.exe` `0x004ae350`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RoachJiggleParams {
    /// X/Z position jitter range `(min, max)` in mm. Zeroed when `ty3.glb` replacement is active.
    pub pos_range: (f32, f32),
    /// Yaw jitter range `(min, max)` in radians. Zeroed when `ty3.glb` replacement is active.
    pub yaw_range: (f32, f32),
}

/// Evaluates cockroach jiggle parameters (`CockroachesReplacement.cpp:31-53`).
///
/// In strict 1:1 Vanilla PS2/PC mode (`enable_sh2ee_world_fixes == false` or `ty3.glb` absent),
/// returns the exact `sh2pc.exe` `FUN_004ae350` (`0x004AE37C`, `0x004AE3BB`) jitter bounds.
#[must_use]
pub const fn evaluate_roach_jiggle(
    enable_sh2ee_world_fixes: bool,
    ty3_glb_available: bool,
) -> RoachJiggleParams {
    if enable_sh2ee_world_fixes && ty3_glb_available {
        RoachJiggleParams {
            pos_range: (0.0, 0.0),
            yaw_range: (0.0, 0.0),
        }
    } else {
        RoachJiggleParams {
            pos_range: (VANILLA_ROACH_JIGGLE_POS_MIN, VANILLA_ROACH_JIGGLE_POS_MAX),
            yaw_range: (VANILLA_ROACH_JIGGLE_YAW_MIN, VANILLA_ROACH_JIGGLE_YAW_MAX),
        }
    }
}

/// Draw call classification for `R_BUG_RM` (`0x65`) cockroach meshes (`CockroachesReplacement.cpp:88-153`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CockroachDrawAction {
    /// Render the original vanilla `.mdl` draw call untouched.
    RenderVanilla,
    /// Replace the vanilla `ty2`/`ty3` body draw with animated `sh2e/model/ty3.glb` at `timer_slot`.
    ReplaceWithTy3Glb { timer_slot: usize },
    /// Suppress the vanilla `ty3` shadow or `ty2`/`ty3` limb draw call.
    SuppressVanillaLimbOrShadow,
}

/// Runtime state for the Prison Bug Room 3D cockroach swarm (`CockroachesReplacement.cpp`).
#[derive(Debug, Clone)]
pub struct CockroachSwarmState {
    /// Per-instance animation phase timers (`[0.0, 8.0)` initial range, `MAX_COCKROACH_TIMERS`).
    pub update_timers: [f32; MAX_COCKROACH_TIMERS],
    /// Frame draw counter indexing into `update_timers`.
    pub frame_counter: usize,
}

impl Default for CockroachSwarmState {
    fn default() -> Self {
        Self::new_seeded(0x5348_3236)
    }
}

impl CockroachSwarmState {
    /// Creates a deterministic swarm state with `32` phase-staggered timers in `[0.0, 8.0)`.
    #[must_use]
    pub fn new_seeded(seed: u32) -> Self {
        let mut s = if seed == 0 { 0x6D2B_79F5 } else { seed };
        let mut timers = [0.0f32; MAX_COCKROACH_TIMERS];
        for slot in &mut timers {
            // Deterministic xorshift32 for reproducible unit tests & staggered phases.
            s ^= s << 13;
            s ^= s >> 17;
            s ^= s << 5;
            let unit = (s as f32) / (u32::MAX as f32 + 1.0);
            *slot = unit * COCKROACH_TIMER_SEED_MAX_SEC;
        }
        Self {
            update_timers: timers,
            frame_counter: 0,
        }
    }

    /// Resets the per-frame instance counter at the start of a render frame.
    pub fn begin_frame(&mut self) {
        self.frame_counter = 0;
    }

    /// Advances the timer for `slot` by `delta_sec` and returns the updated time.
    pub fn advance_timer(&mut self, slot: usize, delta_sec: f32) -> f32 {
        let idx = slot % MAX_COCKROACH_TIMERS;
        self.update_timers[idx] += delta_sec.max(0.0);
        self.update_timers[idx]
    }

    /// Classifies an indexed draw call (`DrawCockroachesReplacement_DIP`, `CockroachesReplacement.cpp:88-124`).
    pub fn classify_indexed_draw(
        &mut self,
        room_id: u32,
        is_triangle_strip: bool,
        num_vertices: u32,
        prim_count: u32,
        ty3_glb_available: bool,
        enable_sh2ee_world_fixes: bool,
    ) -> CockroachDrawAction {
        if !enable_sh2ee_world_fixes
            || !ty3_glb_available
            || room_id != R_BUG_RM_ID
            || !is_triangle_strip
        {
            return CockroachDrawAction::RenderVanilla;
        }

        let is_ty3_body =
            num_vertices == TY3_BODY_NUM_VERTICES && prim_count == TY3_BODY_PRIM_COUNT;
        let is_ty2_body =
            num_vertices == TY2_BODY_NUM_VERTICES && prim_count == TY2_BODY_PRIM_COUNT;
        if is_ty3_body || is_ty2_body {
            let slot = self.frame_counter % MAX_COCKROACH_TIMERS;
            self.frame_counter = self.frame_counter.saturating_add(1);
            return CockroachDrawAction::ReplaceWithTy3Glb { timer_slot: slot };
        }

        let is_ty3_shadow =
            num_vertices == TY3_SHADOW_NUM_VERTICES && prim_count == TY3_SHADOW_PRIM_COUNT;
        if is_ty3_shadow {
            return CockroachDrawAction::SuppressVanillaLimbOrShadow;
        }

        CockroachDrawAction::RenderVanilla
    }

    /// Classifies a non-indexed limb draw call (`DrawCockroachesReplacement_DP`, `CockroachesReplacement.cpp:126-153`).
    #[must_use]
    pub const fn classify_limb_draw(
        room_id: u32,
        is_triangle_list: bool,
        start_vertex: u32,
        prim_count: u32,
        is_dxt3_texture: bool,
        ty3_glb_available: bool,
        enable_sh2ee_world_fixes: bool,
    ) -> CockroachDrawAction {
        if !enable_sh2ee_world_fixes
            || !ty3_glb_available
            || room_id != R_BUG_RM_ID
            || !is_triangle_list
            || start_vertex != 0
        {
            return CockroachDrawAction::RenderVanilla;
        }

        if prim_count == TY3_LIMB_PRIM_COUNTS[0]
            || prim_count == TY3_LIMB_PRIM_COUNTS[1]
            || (prim_count == TY2_LIMB_PRIM_COUNT && is_dxt3_texture)
        {
            CockroachDrawAction::SuppressVanillaLimbOrShadow
        } else {
            CockroachDrawAction::RenderVanilla
        }
    }
}
