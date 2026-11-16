//! Dynamic viewport aspect-ratio camera rail clearance and intra-room warp smoothing (`SH2-53`).
//!
//! Ground-truth specification verified against PS2 prototype `SLUS_202.28` and SH2:EE `.cam` corpus:
//! - `vcGetMinInRoadDist` (`0x001a6bd0`, `sh2cam.c`) + `BgIsOut` (`0x001b0cd0`, `stg_name.c:963`):
//!   `500.0` mm for outdoor town stages (`Glb_crd` `1..=4`: `ca`, `cb`, `cc`, `cd`), `400.0` mm indoor.
//! - `vcAdjustXzInLimAreaUsingMIN_IN_ROAD_DIST` (`0x0019b700`, `vc_calc.c`): pads `_VC_ROAD_AREA`
//!   rail bounds inward by `min_in_road_dist`, collapsing narrow axes (`max - m < min + m`) to midpoint.
//! - `vcAdvantageDistOfOldCurRoad` (`0x00198ae0`) & `vcSetCurNearRoadInVC_WORK` (`0x00197af0`):
//!   `VC_RD_NO_EXTRA_AREA_F` (`0x0100`) zeroes `lim_sw` switch hysteresis (`[75, 125, 175, 250]` / `50` mm)
//!   without altering `lim_rd` wall clearance; `VC_RD_NOT_WARP_F` (`0x0400`) vetoes intra-room warps.

use glam::Vec3;
use sh2_assets::formats::cam::{
    AreaSizeType, CamMovementParams, CamMovementType, RoadArea, RoadRecord, RoadType,
    VC_RD_CAM_LIKE_SETTLE_F, VC_RD_NOT_WARP_F, VC_RD_NO_EXTRA_AREA_F, VC_RD_WARP_IN_F,
    VC_RD_WARP_OUT_F,
};
use std::f32::consts::{FRAC_PI_6, PI, TAU};

#[cfg(test)]
mod tests;

/// Authored PS2 4:3 CRT reference aspect ratio (`W / H` = `4.0 / 3.0`, dimensionless).
pub const PS2_REFERENCE_4X3_ASPECT: f32 = 4.0 / 3.0;
/// Standard 16:10 widescreen aspect ratio (`W / H` = `1.6`, dimensionless).
pub const ASPECT_16X10: f32 = 16.0 / 10.0;
/// Standard 16:9 widescreen aspect ratio (`W / H` = `1.7777778`, dimensionless).
pub const ASPECT_16X9: f32 = 16.0 / 9.0;
/// 20:9 handheld / mobile ultrawide aspect ratio (`W / H` = `2.2222223`, AYN Thor profile).
pub const ASPECT_20X9_THOR: f32 = 20.0 / 9.0;
/// 21:9 desktop ultrawide aspect ratio (`W / H` = `2.3333333`, dimensionless).
pub const ASPECT_21X9_ULTRAWIDE: f32 = 21.0 / 9.0;
/// PS2 indoor minimum rail clearance in world mm (`vcGetMinInRoadDist` `0x001a6bd0`, `BgIsOut == 0`).
pub const PS2_INDOOR_MIN_IN_ROAD_DIST_MM: f32 = 400.0;
/// PS2 outdoor minimum rail clearance in world mm (`vcGetMinInRoadDist` `0x001a6bd0`, `BgIsOut != 0`).
pub const PS2_OUTDOOR_MIN_IN_ROAD_DIST_MM: f32 = 500.0;
/// Default 60 Hz per-tick camera eye smoothing coefficient (`0.0..=1.0`, dimensionless).
pub const DEFAULT_SMOOTH_WARP_EYE_LERP: f32 = 0.15;
/// Default 60 Hz per-tick camera look-target smoothing coefficient (`0.0..=1.0`, dimensionless).
pub const DEFAULT_SMOOTH_WARP_TARGET_LERP: f32 = 0.18;

/// Camera rail wall-clearance scaling mode (`SH2-53`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum WidescreenClearanceMode {
    /// Preserve exact 4:3 CRT PS2 `vcGetMinInRoadDist` (`0x001a6bd0`) margins (`400.0`/`500.0` mm).
    #[default]
    Authentic4x3Crt,
    /// Scale clearance when `viewport_aspect > 4.0 / 3.0` by `(viewport_aspect / (4.0 / 3.0)).sqrt()`.
    DynamicAspectScaled,
}

/// 60 Hz per-tick camera eye and look-target exponential lerp coefficients (`0.0..=1.0`, `SH2-53`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransitionLerpFactors {
    /// Per-tick 60 Hz eye interpolation factor (`0.0..=1.0`).
    pub eye_lerp: f32,
    /// Per-tick 60 Hz look-target interpolation factor (`0.0..=1.0`).
    pub target_lerp: f32,
}

impl Default for TransitionLerpFactors {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl TransitionLerpFactors {
    /// Default 60 Hz smoothing coefficients (`0.15` eye, `0.18` look target).
    pub const DEFAULT: Self = Self {
        eye_lerp: DEFAULT_SMOOTH_WARP_EYE_LERP,
        target_lerp: DEFAULT_SMOOTH_WARP_TARGET_LERP,
    };

    /// Clamps finite coefficients to `[0.01, 1.0]` and substitutes defaults on `NaN`/`Inf`.
    #[must_use]
    pub fn sanitized(self) -> Self {
        let fix = |v: f32, d: f32| match v.is_finite() {
            true => v.clamp(0.01, 1.0),
            false => d,
        };
        Self {
            eye_lerp: fix(self.eye_lerp, DEFAULT_SMOOTH_WARP_EYE_LERP),
            target_lerp: fix(self.target_lerp, DEFAULT_SMOOTH_WARP_TARGET_LERP),
        }
    }
}

/// Policy governing intra-room `.cam` volume cuts (`VC_RD_WARP_OUT_F` / `VC_RD_WARP_IN_F`, `SH2-53`).
///
/// Room and door transitions (`Ps2CameraController::reset_to_player`) always cut instantly.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum IntraRoomTransitionPolicy {
    /// Preserve 1:1 PS2 `vcSetCurNearRoadInVC_WORK` (`0x00197af0`) instant cuts unless `VC_RD_NOT_WARP_F` is set.
    #[default]
    AuthenticPs2InstantWarp,
    /// Smooth harsh `VC_RD_WARP_OUT_F` exits at 60 Hz (SH2:EE `er01.cam`) while keeping `VC_RD_WARP_IN_F` cuts instant.
    SmoothWarpOutOnly(TransitionLerpFactors),
    /// Smooth all intra-room `VC_RD_WARP_OUT_F` and `VC_RD_WARP_IN_F` transitions at 60 Hz (SH2:EE `hp121.cam`).
    SmoothAllIntraRoomWarps(TransitionLerpFactors),
}

impl IntraRoomTransitionPolicy {
    /// Smooth `VC_RD_WARP_OUT_F` exits using default 60 Hz smoothing (`0.15` / `0.18`).
    #[must_use]
    pub const fn smooth_warp_out_default() -> Self {
        Self::SmoothWarpOutOnly(TransitionLerpFactors::DEFAULT)
    }

    /// Smooth all intra-room warp flags using default 60 Hz smoothing (`0.15` / `0.18`).
    #[must_use]
    pub const fn smooth_all_default() -> Self {
        Self::SmoothAllIntraRoomWarps(TransitionLerpFactors::DEFAULT)
    }
}

/// Resolved per-tick camera transition step across intra-room `.cam` volumes (`SH2-53`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum IntraRoomTransitionStep {
    /// Snap `ideal_eye`, `ideal_target`, and `prev_snapshot` immediately with zero sub-frame smear.
    InstantWarpCut,
    /// Blend `ideal_eye` and `ideal_target` smoothly over 60 Hz ticks.
    SmoothBlend(TransitionLerpFactors),
}

/// Runtime configuration for dynamic aspect-ratio rail clearance and intra-room warp smoothing (`SH2-53`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AspectRailConfig {
    /// Wall-clearance scaling mode (`Authentic4x3Crt` vs `DynamicAspectScaled`).
    pub clearance_mode: WidescreenClearanceMode,
    /// Viewport aspect ratio (`width / height`, dimensionless).
    pub viewport_aspect: f32,
    /// Intra-room `VC_RD_WARP_OUT_F` / `VC_RD_WARP_IN_F` transition policy.
    pub transition_policy: IntraRoomTransitionPolicy,
}

impl Default for AspectRailConfig {
    fn default() -> Self {
        Self::authentic_ps2_4x3()
    }
}

impl AspectRailConfig {
    /// Authentic 4:3 CRT PS2 configuration (`400.0`/`500.0` mm clearance + instant warp cuts).
    #[must_use]
    pub const fn authentic_ps2_4x3() -> Self {
        Self {
            clearance_mode: WidescreenClearanceMode::Authentic4x3Crt,
            viewport_aspect: PS2_REFERENCE_4X3_ASPECT,
            transition_policy: IntraRoomTransitionPolicy::AuthenticPs2InstantWarp,
        }
    }

    /// Dynamic widescreen wall-clearance configuration for `viewport_aspect` (`W / H`).
    #[must_use]
    pub fn dynamic_widescreen(viewport_aspect: f32) -> Self {
        let mut cfg = Self::authentic_ps2_4x3();
        cfg.clearance_mode = WidescreenClearanceMode::DynamicAspectScaled;
        if viewport_aspect.is_finite() && viewport_aspect > 0.0 {
            cfg.viewport_aspect = viewport_aspect;
        }
        cfg
    }

    /// Dynamic widescreen configuration derived safely from pixel `width` and `height`.
    #[must_use]
    pub fn dynamic_from_dimensions(width: u32, height: u32) -> Self {
        if width == 0 || height == 0 {
            return Self::dynamic_widescreen(PS2_REFERENCE_4X3_ASPECT);
        }
        Self::dynamic_widescreen(width as f32 / height as f32)
    }

    /// Builder returning `self` with an updated [`IntraRoomTransitionPolicy`].
    #[must_use]
    pub const fn with_transition_policy(mut self, policy: IntraRoomTransitionPolicy) -> Self {
        self.transition_policy = policy;
        self
    }
}

/// Returns `true` when a room ID or `.cam` record uses outdoor PS2 clearance (`500.0` mm),
/// matching `BgIsOut` (`0x001b0cd0`: `ca`, `cb`, `cc`, `cd`) and `AreaSizeType::Outdoor`.
#[must_use]
pub fn is_outdoor_room_or_record(room_id: Option<&str>, area_size_type: AreaSizeType) -> bool {
    let p = room_id.map(str::trim).and_then(|s| s.as_bytes().get(..2));
    area_size_type == AreaSizeType::Outdoor
        || matches!(
            p,
            Some(b"ca" | b"cb" | b"cc" | b"cd" | b"CA" | b"CB" | b"CC" | b"CD")
        )
}

/// Returns base 4:3 PS2 minimum rail clearance in world mm (`vcGetMinInRoadDist` `0x001a6bd0`).
#[must_use]
pub const fn base_ps2_rail_margin_mm(is_outdoor: bool) -> f32 {
    [
        PS2_INDOOR_MIN_IN_ROAD_DIST_MM,
        PS2_OUTDOOR_MIN_IN_ROAD_DIST_MM,
    ][is_outdoor as usize]
}

/// Computes effective camera rail wall-clearance margin in PS2 world millimeters (`SH2-53`).
#[must_use]
pub fn compute_aspect_rail_margin_mm(is_outdoor: bool, config: &AspectRailConfig) -> f32 {
    let base = base_ps2_rail_margin_mm(is_outdoor);
    if config.clearance_mode == WidescreenClearanceMode::DynamicAspectScaled
        && config.viewport_aspect.is_finite()
        && config.viewport_aspect > PS2_REFERENCE_4X3_ASPECT
    {
        return base * (config.viewport_aspect / PS2_REFERENCE_4X3_ASPECT).sqrt();
    }
    base
}

/// Computes `lim_sw` switch-volume hysteresis advantage in PS2 world mm (`vcAdvantageDistOfOldCurRoad`
/// `0x00198ae0` + table `0x00390600`); returns `0.0` mm when `VC_RD_NO_EXTRA_AREA_F` (`0x0100`) is set.
#[must_use]
pub const fn switch_hysteresis_advantage_mm(rec: &RoadRecord) -> f32 {
    if (rec.flags & VC_RD_NO_EXTRA_AREA_F) != 0 {
        return 0.0;
    }
    match (rec.rd_type, rec.area_size_type) {
        (RoadType::Effect | RoadType::Event, _) => 50.0,
        (RoadType::SelfViewOnly, _) => 0.0,
        (_, AreaSizeType::Tiny) => 75.0,
        (_, AreaSizeType::Small) => 125.0,
        (_, AreaSizeType::Wide) => 175.0,
        (_, AreaSizeType::Outdoor) => 250.0,
    }
}

/// Returns `true` when `_VC_ROAD_AREA` defines a non-degenerate horizontal rail (`> 0.01` mm span).
#[must_use]
pub fn is_rail_constrained(rail: &RoadArea) -> bool {
    let ((min_x, max_x), (min_z, max_z)) = (rail.x_bounds(), rail.z_bounds());
    (max_x - min_x).abs() > 0.01 || (max_z - min_z).abs() > 0.01
}

/// Clamps one axis (PS2 world mm) within `[min_bound + margin_mm, max_bound - margin_mm]`, collapsing
/// narrow rails (`max_bound - margin_mm < min_bound + margin_mm`) to midpoint (`0x0019b700`).
#[must_use]
pub fn clamp_axis_with_margin(min_bound: f32, max_bound: f32, margin_mm: f32, value: f32) -> f32 {
    let (lo, hi) = (min_bound + margin_mm, max_bound - margin_mm);
    if hi < lo {
        return (lo + hi) * 0.5;
    }
    value.clamp(lo, hi)
}

/// Clamps `pos` (PS2 world mm, Y-down) inside a `_VC_ROAD_AREA` rail using
/// PS2 `vcAdjustXzInLimAreaUsingMIN_IN_ROAD_DIST` (`0x0019b700`) and [`AspectRailConfig`].
#[must_use]
pub fn clamp_to_road_rail_with_config(
    rail: &RoadArea,
    pos: Vec3,
    is_outdoor: bool,
    config: &AspectRailConfig,
) -> Vec3 {
    let m = compute_aspect_rail_margin_mm(is_outdoor, config);
    let ((x0, x1), (z0, z1), (y0, y1)) = (rail.x_bounds(), rail.z_bounds(), rail.y_bounds());
    Vec3::new(
        clamp_axis_with_margin(x0, x1, m, pos.x),
        pos.y.clamp(y0, y1),
        clamp_axis_with_margin(z0, z1, m, pos.z),
    )
}

fn norm_rad(mut a: f32) -> f32 {
    while a < -PI {
        a += TAU;
    }
    while a >= PI {
        a -= TAU;
    }
    a
}

/// Applies [`AspectRailConfig`] rail clearance to `Chase` records and reframes `eval_look`
/// (`vcSetWatchTgtXzPos` `0x00199ca0`, `vcAdjCamOfsAngByCharaInScreen` `0x0019d240`,
/// `vcCorrectCamMatAngForcibly` `0x002527d0`) whenever widescreen/outdoor padding shifts `eval_eye`.
#[must_use]
pub fn apply_chase_rail_clearance(
    rec: &RoadRecord,
    room_id: Option<&str>,
    eval_eye: Vec3,
    eval_look: Vec3,
    player_pos: Vec3,
    player_yaw: f32,
    config: &AspectRailConfig,
) -> (Vec3, Vec3) {
    if rec.cam_mv_type != CamMovementType::Chase || !is_rail_constrained(&rec.lim_rd) {
        return (eval_eye, eval_look);
    }
    let is_out = is_outdoor_room_or_record(room_id, rec.area_size_type);
    let eye = clamp_to_road_rail_with_config(&rec.lim_rd, eval_eye, is_out, config);
    if (eye - eval_eye).length_squared() <= 1e-6 {
        return (eye, eval_look);
    }
    let (cir_r, min_d) = match rec.area_size_type {
        AreaSizeType::Tiny => (400.0f32, 1400.0f32),
        AreaSizeType::Small => (600.0, 1800.0),
        AreaSizeType::Wide => (1000.0, 2800.0),
        AreaSizeType::Outdoor => (1250.0, 3200.0),
    };
    let (dx, dz) = (player_pos.x - eye.x, player_pos.z - eye.z);
    let r_p = dx.hypot(dz);
    let extra = (min_d + cir_r - r_p).max(0.0);
    let c2c = dz.atan2(dx);
    let tgt = Vec3::new(
        player_pos.x + player_yaw.sin() * cir_r + c2c.sin() * extra,
        rec.ofs_watch_hy + (player_pos.y - 250.0),
        player_pos.z + player_yaw.cos() * cir_r + c2c.cos() * extra,
    );
    let vt = tgt - eye;
    let raw_ax = (-vt.y).atan2(vt.x.hypot(vt.z));
    let mut yaw = vt.x.atan2(vt.z);
    let scr_z = if rec.projection > 0.0 {
        rec.projection
    } else {
        448.0
    };
    let half_wy = 448.0f32.atan2(scr_z) * 0.5;
    let w_btm = norm_rad((-(rec.trace_btm_hy + player_pos.y - 250.0 - eye.y)).atan2(r_p) - raw_ax);
    let w_top = norm_rad((-(player_pos.y - 937.5 - eye.y)).atan2(r_p) - raw_ax);
    let mut adj_x = if -half_wy > w_btm {
        w_btm + half_wy
    } else {
        0.0
    };
    if w_top - adj_x > half_wy {
        adj_x += (w_top - adj_x) - half_wy;
    }
    let pitch = raw_ax + adj_x.clamp(-FRAC_PI_6, FRAC_PI_6);
    if (rec.flags & VC_RD_CAM_LIKE_SETTLE_F) != 0 {
        if let CamMovementParams::Chase(ref chs) = rec.params {
            if chs.lr_lim_ang_y != -17.451548 || chs.rr_lim_ang_y != 17.451548 {
                let ol = norm_rad(yaw - chs.lr_lim_ang_y);
                let or = norm_rad(yaw - chs.rr_lim_ang_y);
                if !(ol >= 0.0 && or <= 0.0) {
                    yaw = norm_rad(match ol.abs() < or.abs() {
                        true => chs.lr_lim_ang_y,
                        false => chs.rr_lim_ang_y,
                    });
                }
            }
        }
    }
    let fwd = Vec3::new(
        pitch.cos() * yaw.sin(),
        -pitch.sin(),
        pitch.cos() * yaw.cos(),
    );
    (eye, eye + fwd * 2000.0)
}

/// Evaluates intra-room `.cam` transition behavior across `old_flags` -> `new_flags`, honoring
/// PS2 `vcSetCurNearRoadInVC_WORK` (`0x00197af0`, `VC_RD_NOT_WARP_F` `0x0400`) and `policy`.
#[must_use]
pub fn evaluate_intra_room_transition(
    old_flags: u16,
    new_flags: u16,
    policy: IntraRoomTransitionPolicy,
) -> IntraRoomTransitionStep {
    use IntraRoomTransitionStep::{InstantWarpCut, SmoothBlend};
    let w_out = (old_flags & VC_RD_WARP_OUT_F) != 0;
    let w_in = (new_flags & VC_RD_WARP_IN_F) != 0;
    if ((old_flags | new_flags) & VC_RD_NOT_WARP_F) != 0 || (!w_out && !w_in) {
        return SmoothBlend(TransitionLerpFactors::DEFAULT);
    }
    match policy {
        IntraRoomTransitionPolicy::AuthenticPs2InstantWarp => InstantWarpCut,
        IntraRoomTransitionPolicy::SmoothWarpOutOnly(_) if w_in => InstantWarpCut,
        IntraRoomTransitionPolicy::SmoothWarpOutOnly(f)
        | IntraRoomTransitionPolicy::SmoothAllIntraRoomWarps(f) => SmoothBlend(f.sanitized()),
    }
}
