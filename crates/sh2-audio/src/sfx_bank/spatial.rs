//! 3D distance attenuation (`SeCallPosDistanceF` / `SeCallPosDistance`) and stereo pan
//! (`SeCallPosDirection`) from `SLUS_202.28` and `sh2pc.exe`.

use glam::{Mat4, Vec3, Vec4};

/// Inner 3D distance radius (in world units, mm) within which positional sound effects play at unattenuated full volume
/// (`SLUS_202.28` `SeCallPosDistanceF` `0x00192bc0` / `sh2pc.exe` `0x005155e0` `0x006B4038`).
pub const SFX_MIN_DISTANCE: f32 = 1500.0;

/// Outer 3D distance cutoff radius (in world units, mm) beyond which positional sound effects attenuate to silence
/// (`1500.0 + 1.0 / 0.0001 = 11500.0`, `SeCallPosDistanceF` `0x00192bc0`).
pub const SFX_MAX_DISTANCE: f32 = 11500.0;

/// Distance attenuation span ($d_{\max} - d_{\min} = 10000.0$ mm, derived from reciprocal `0.0001` at `0x00192bc0` / `0x006C5D70`).
pub const SFX_DISTANCE_SPAN: f32 = SFX_MAX_DISTANCE - SFX_MIN_DISTANCE;

/// Exact reciprocal attenuation span (`0.0001`, `SLUS_202.28` `0x00192bc0` / `sh2pc.exe` `0x006C5D70`).
pub const SFX_DISTANCE_INV_SPAN: f32 = 0.0001;

/// Camera-space X offset in mm subtracted in `SeCallPosDirection` (`SLUS_202.28` `0x00192990` / `sh2pc.exe` `0x00515e60` `0x006a8478`).
pub const SE_DIR_CAMERA_X_OFFSET_MM: f32 = 750.0;

/// Pan angle scale in degrees (`70.0°`, `SLUS_202.28` `SeCallPosDirection` `0x00192990` / `sh2pc.exe` `0x0069f3d0`).
pub const SE_DIR_PAN_SCALE_DEG: f32 = 70.0;

/// Computes the normalized proximity factor $1.0 - \text{clamp}((d - d_{\min}) / (d_{\max} - d_{\min}), 0, 1)$
/// for a single distance in mm (`SeCallPosDistanceF` `0x00192bc0` / `sh2pc.exe` `0x005155e0`).
#[inline]
pub fn compute_sfx_distance_param_v(distance: f32, min_dist: f32, max_dist: f32) -> f32 {
    let span = (max_dist - min_dist).max(1.0);
    let t = ((distance - min_dist) / span).clamp(0.0, 1.0);
    1.0 - t
}

/// Evaluates `SeCallPosDistanceF` (`SLUS_202.28` `0x00192bc0`) exact dual-distance linear product:
///
/// $$\text{atten} = \left(1 - \text{clamp}((d_{\text{cam}} - 1500)\cdot 0.0001, 0, 1)\right)
///                \cdot \left(1 - \text{clamp}((d_{\text{src}} - 1500)\cdot 0.0001, 0, 1)\right)$$
///
/// where $d_{\text{cam}} = \|\text{camera\_pos} - \text{player\_pos}\|$ (`vcWork.cam_pos` `0x010e5b40` vs `sh2jms + 0x20`)
/// and $d_{\text{src}} = \|\text{source\_pos} - \text{player\_pos}\|$ in world mm.
#[inline]
pub fn se_call_pos_distance_f(camera_pos: Vec3, player_pos: Vec3, source_pos: Vec3) -> f32 {
    if !camera_pos.is_finite() || !player_pos.is_finite() || !source_pos.is_finite() {
        return 0.0;
    }
    let d_cam = (camera_pos - player_pos).length();
    let d_src = (source_pos - player_pos).length();
    let f_cam = ((d_cam - SFX_MIN_DISTANCE) * SFX_DISTANCE_INV_SPAN).clamp(0.0, 1.0);
    let f_src = ((d_src - SFX_MIN_DISTANCE) * SFX_DISTANCE_INV_SPAN).clamp(0.0, 1.0);
    (1.0 - f_cam) * (1.0 - f_src)
}

/// Evaluates `SeCallPosDistance` (`SLUS_202.28` `0x00192a70` / `sh2pc.exe` `0x005155e0`),
/// returning the `0..=255` driver attenuation byte (`0` = loudest, `255` = silent):
/// `(int)(255.0 - base_volume * se_call_pos_distance_f(...) * 255.0)`.
#[inline]
pub fn se_call_pos_distance(
    base_volume: f32,
    camera_pos: Vec3,
    player_pos: Vec3,
    source_pos: Vec3,
) -> u8 {
    if !base_volume.is_finite() {
        return 255;
    }
    let atten = se_call_pos_distance_f(camera_pos, player_pos, source_pos);
    let raw = 255.0 - base_volume.clamp(0.0, 1.0) * atten * 255.0;
    raw.clamp(0.0, 255.0) as u8
}

/// Evaluates `SeCallPosDirection` (`SLUS_202.28` `0x00192990` / `sh2pc.exe` `0x00515e60`):
/// 1. Transforms `[source_pos.x, source_pos.y, source_pos.z, 1.0]` by `wvs_matrix` (`VbWvsMatrix` `0x010e59d0`).
/// 2. Subtracts `750.0` mm from camera-space `x` and zeroes `y`.
/// 3. Normalizes `(x, 0, z)` (`FUN_00515d00` in `sh2pc.exe`, VU0 `vrsqrt` in `SLUS_202.28`).
/// 4. Returns `(int)(sign(-x) * (70.0 - z * 70.0))` in degrees (`0` ahead, `-70` right, `+70` left, `±140` behind).
#[inline]
pub fn se_call_pos_direction(wvs_matrix: Mat4, source_pos: Vec3) -> i32 {
    if !source_pos.is_finite() {
        return 0;
    }
    let view = wvs_matrix * Vec4::new(source_pos.x, source_pos.y, source_pos.z, 1.0);
    let vx = view.x - SE_DIR_CAMERA_X_OFFSET_MM;
    let vz = view.z;
    let len_sq = vx * vx + vz * vz;
    let (nx, nz) = if len_sq > 0.0 {
        let inv = len_sq.sqrt().recip();
        if inv.is_finite() && inv <= f32::MAX {
            (vx * inv, vz * inv)
        } else {
            (vx, vz)
        }
    } else {
        (vx, vz)
    };
    let neg_x = -nx;
    let sign = if neg_x > 0.0 {
        1.0
    } else if neg_x < 0.0 {
        -1.0
    } else {
        0.0
    };
    (sign * (SE_DIR_PAN_SCALE_DEG - nz * SE_DIR_PAN_SCALE_DEG)) as i32
}

/// Builds a PS2 `VbWvsMatrix` (`view/vb_main.c` `0x010e59d0`: `+X` right, `+Y` down, `+Z` forward)
/// for a camera at `camera_pos` facing horizontal `yaw` radians (`0.0` = `+Z`, `+pi/2` = `+X`).
#[inline]
pub fn build_ps2_wvs_matrix(camera_pos: Vec3, yaw: f32) -> Mat4 {
    let (sin_y, cos_y) = yaw.sin_cos();
    // Matches `vb_main.c` lines 267-290:
    // x_axis = (cos_y, 0, -sin_y), y_axis = (0, 1, 0), z_axis = (sin_y, 0, cos_y)
    let x_axis = Vec3::new(cos_y, 0.0, -sin_y);
    let y_axis = Vec3::Y;
    let z_axis = Vec3::new(sin_y, 0.0, cos_y);
    let tx = -x_axis.dot(camera_pos);
    let ty = -y_axis.dot(camera_pos);
    let tz = -z_axis.dot(camera_pos);
    Mat4::from_cols(
        Vec4::new(x_axis.x, y_axis.x, z_axis.x, 0.0),
        Vec4::new(x_axis.y, y_axis.y, z_axis.y, 0.0),
        Vec4::new(x_axis.z, y_axis.z, z_axis.z, 0.0),
        Vec4::new(tx, ty, tz, 1.0),
    )
}

/// Complete 3D positional SFX evaluation result (`SeCallPosDistanceF` `0x00192bc0` & `SeCallPosDirection` `0x00192990`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SfxSpatialEval {
    /// 3D Euclidean distance between player (`sh2jms`) and emitter in world units (mm).
    pub distance: f32,
    /// 3D Euclidean distance between active camera (`vcWork.cam_pos`) and player in world units (mm).
    pub camera_distance: f32,
    /// Dual-distance linear attenuation factor `(1 - f_cam) * (1 - f_src) * base_volume` in `[0.0, 1.0]`.
    pub proximity_v: f32,
    /// Final effective linear amplitude gain (`0.0..=1.0`), identical to `proximity_v`.
    pub effective_gain: f32,
    /// Integer byte attenuation `0..=255` passed to `shSdCall` (`0` = loudest, `255` = silent).
    pub driver_attenuation_byte: u8,
    /// Raw `SeCallPosDirection` (`0x00192990`) pan angle in degrees (`0` ahead, `-70` right, `+70` left, `±140` behind).
    pub direction_deg: i32,
    /// Normalized stereo pan in `[-1.0, 1.0]` (`-1.0` = full left, `0.0` = center, `+1.0` = full right),
    /// derived from `(-direction_deg as f32 / 70.0).clamp(-1.0, 1.0)`.
    pub stereo_pan: f32,
}

/// Evaluates full dual-distance `SeCallPosDistanceF` (`SLUS_202.28` `0x00192bc0` / `sh2pc.exe` `0x005155e0`)
/// and `SeCallPosDirection` (`0x00192990` / `0x00515e60`) using separate camera and player positions plus `VbWvsMatrix`.
pub fn compute_se_call_pos_spatial(
    camera_pos: Vec3,
    player_pos: Vec3,
    wvs_matrix: Mat4,
    source_pos: Vec3,
    base_volume: f32,
) -> SfxSpatialEval {
    if !camera_pos.is_finite()
        || !player_pos.is_finite()
        || !source_pos.is_finite()
        || !base_volume.is_finite()
    {
        return SfxSpatialEval {
            distance: SFX_MAX_DISTANCE,
            camera_distance: SFX_MAX_DISTANCE,
            proximity_v: 0.0,
            effective_gain: 0.0,
            driver_attenuation_byte: 255,
            direction_deg: 0,
            stereo_pan: 0.0,
        };
    }

    let distance = (source_pos - player_pos).length();
    let camera_distance = (camera_pos - player_pos).length();
    let atten = se_call_pos_distance_f(camera_pos, player_pos, source_pos);
    let clamped_vol = base_volume.clamp(0.0, 1.0);
    let proximity_v = (atten * clamped_vol).clamp(0.0, 1.0);
    let driver_attenuation_byte =
        se_call_pos_distance(clamped_vol, camera_pos, player_pos, source_pos);
    let direction_deg = se_call_pos_direction(wvs_matrix, source_pos);
    // In `SeCallPosDirection`, negative degrees = right ear (+X), positive degrees = left ear (-X).
    let stereo_pan = (-(direction_deg as f32) / SE_DIR_PAN_SCALE_DEG).clamp(-1.0, 1.0);

    SfxSpatialEval {
        distance,
        camera_distance,
        proximity_v,
        effective_gain: proximity_v,
        driver_attenuation_byte,
        direction_deg,
        stereo_pan,
    }
}

/// Evaluates `SeCallPosDistanceF` (`0x00192bc0` / `0x005155e0`) and `SeCallPosDirection` (`0x00192990` / `0x00515e60`)
/// when the listener camera and player share a position (`d_cam = 0`), constructing `VbWvsMatrix` from `listener_yaw`.
///
/// For separate camera (`vcWork.cam_pos`) and player (`sh2jms`) positions, call [`compute_se_call_pos_spatial`].
pub fn compute_se_call_pos_distance_f(
    listener_pos: Vec3,
    listener_yaw: f32,
    source_pos: Vec3,
    base_volume: f32,
) -> SfxSpatialEval {
    if !listener_yaw.is_finite() {
        return SfxSpatialEval {
            distance: SFX_MAX_DISTANCE,
            camera_distance: SFX_MAX_DISTANCE,
            proximity_v: 0.0,
            effective_gain: 0.0,
            driver_attenuation_byte: 255,
            direction_deg: 0,
            stereo_pan: 0.0,
        };
    }
    let wvs = build_ps2_wvs_matrix(listener_pos, listener_yaw);
    compute_se_call_pos_spatial(listener_pos, listener_pos, wvs, source_pos, base_volume)
}
