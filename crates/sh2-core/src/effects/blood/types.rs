//! Public types and pure math evaluators for the `Effect2` Impact Queue Blood System.

use glam::Vec3;

use super::tables::SH2EE_BLOOD_SIZE_SCALE;
use crate::config::BloodColor;

/// Maximum simultaneous airborne blood spray/drop particles (`Class 5`).
pub const MAX_BLOOD_DROPS: usize = 128;

/// Maximum simultaneous floor blood stains (`Class 10`).
pub const MAX_BLOOD_STAINS: usize = 128;

/// Maximum simultaneous expanding death blood pools (`Class 12`).
pub const MAX_BLOOD_POOLS: usize = 64;

/// Maximum simultaneous bloody footprints (`Class 9`).
pub const MAX_BLOOD_FOOTMARKS: usize = 64;

/// Blood color mode wrapper.
pub type BloodColorMode = BloodColor;

/// Runtime configuration for [`super::BloodEffectSystem`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BloodSystemConfig {
    /// PS2 Extra Options Blood Color (`Normal`, `Green`, `Violet`, `Black`).
    pub blood_color: BloodColor,
    /// SH2:EE `IncreaseBlood` scale multiplier (`1.0` when enabled, `1.0` default).
    pub blood_size_scale: f32,
    /// Native-upgrade Persistent Room Decals & Coagulation (`false` = strict 1:1 PS2 `blood_clear()`).
    pub persistent_room_decals: bool,
    /// Use non-subtractive alpha blending fallback (`DAT_00a3339c == 0`).
    pub non_subtractive_fallback: bool,
}

impl Default for BloodSystemConfig {
    fn default() -> Self {
        Self {
            blood_color: BloodColor::Normal,
            blood_size_scale: SH2EE_BLOOD_SIZE_SCALE,
            persistent_room_decals: false,
            non_subtractive_fallback: false,
        }
    }
}

/// Impact Queue event type (`Effect2/hh_class_manager.c` `ImpactDescriptor_Post`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImpactKind {
    /// `Class 3` (`HH_Class_Blood_Pool_Phenomenon_00` `0x002560a0`).
    BloodPoolEmitter,
    /// `Class 11` (`HH_Class_Blood_Splash_Phenomenon_00` `0x00256ba0`).
    BloodSplashEmitter,
    /// `Class 5` (`HH_Class_Blood_01` `0x002548e0`).
    BloodParticle,
    /// `Class 10` (`HH_Class_Blood_00` `0x00253b00`).
    FloorStain,
    /// `Class 12` (`HH_Class_Blood_02` `0x002551f0`).
    ExpandingPool,
    /// `Class 9` (`HH_Class_Blood_FootMark_00` `0x00255f40`).
    Footmark,
}

/// Descriptor queued into `HH_Effect_Object_Impact_Post` (`0x0026a1e0`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ImpactDescriptor {
    pub kind: ImpactKind,
    pub position_mm: Vec3,
    pub velocity_mm_s: Vec3,
    pub yaw_rad: f32,
    /// Sub-mode (`0` = mist/right foot, `1` = falling drop/left foot).
    pub mode: u8,
}

/// Renderable blood quad classification for `sh2-renderer`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BloodQuadKind {
    /// Camera-facing 3D billboard (`Class 5` `Blood01`).
    Billboard,
    /// Floor-aligned decal (`Class 10` `Blood00`, `Class 12` `Blood02`, `Class 9` `FootMark00`).
    FloorDecal,
}

/// Renderable blood quad emitted by [`super::BloodEffectSystem::build_render_items`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BloodQuadRenderItem {
    pub kind: BloodQuadKind,
    /// World-space center in PS2 mm.
    pub position_mm: Vec3,
    /// Half-extents `(hx, hz)` in PS2 mm (for billboards, `hx == hz` radius).
    pub half_extents_mm: [f32; 2],
    /// Y-axis rotation in radians (for floor decals).
    pub yaw_rad: f32,
    /// Sub-atlas UV rect `[u_min, v_min, u_max, v_max]` in `bloodtex0.tbn2`.
    pub uv_rect: [f32; 4],
    /// Normalized RGB in `[0.0, 1.0]` (subtractive `Cs` or fallback color).
    pub color_rgb: [f32; 3],
    /// Normalized alpha `As` in `[0.0, 1.0]` (`alpha_gs / 128.0`).
    pub alpha: f32,
    /// `true` for PS2 GS subtractive (`Cd - Cs * As`), `false` for normal alpha blend.
    pub subtractive: bool,
}

/// Evaluates a piecewise-linear 3-stage motion envelope `[dt_sec, dscale, dalpha]`
/// matching `Object_Motion_00` in `hh_class_blood_00.c` (`0x00253c80`) & `hh_class_blood_02.c` (`0x00254c70`).
///
/// `Object_Motion_00` accumulates `keyframes[0]` (`uVar5 = 0`) before interpolating segments `1..=3`.
#[must_use]
pub fn evaluate_three_stage_envelope(elapsed_sec: f32, keyframes: &[[f32; 3]; 4]) -> (f32, f32) {
    let mut t_acc = keyframes[0][0];
    let mut s_acc = keyframes[0][1];
    let mut a_acc = keyframes[0][2];

    for kf in &keyframes[1..] {
        let dt = kf[0];
        let ds = kf[1];
        let da = kf[2];
        let next_t = t_acc + dt;
        if elapsed_sec <= next_t || dt <= 1e-6 {
            let frac = if dt > 1e-6 {
                ((elapsed_sec - t_acc) / dt).clamp(0.0, 1.0)
            } else {
                1.0
            };
            return (
                (s_acc + ds * frac).clamp(0.0, 1.0),
                (a_acc + da * frac).clamp(0.0, 1.0),
            );
        }
        t_acc = next_t;
        s_acc += ds;
        a_acc += da;
    }

    (s_acc.clamp(0.0, 1.0), a_acc.clamp(0.0, 1.0))
}

/// Exact port of `ParabolaMotion_Calculator` (`hh_class_blood_01.c` `0x00254380`):
/// `offset = vel * t + (0, gravity_y * t * t, 0)`.
#[must_use]
pub fn evaluate_blood_parabola(vel_mm_s: Vec3, gravity_y_mm_s2: f32, t_sec: f32) -> Vec3 {
    Vec3::new(
        vel_mm_s.x * t_sec,
        vel_mm_s.y * t_sec + gravity_y_mm_s2 * t_sec * t_sec,
        vel_mm_s.z * t_sec,
    )
}
