//! Authentic PS2 Mode 3 (`SELF_VIEW`) camera matrix breathing/handheld noise (`SH2-134`).
//!
//! Ground truth:
//! - PS2 `SLUS_202.28` (`view/vc_main.c`):
//!   - `vcCamMatNoise` (`0x0019d8e0`)
//!   - `vcSetDataToVwSystem` (`0x0019d550`, Mode 3 branch at `view/vc_main.c:3962-3996`)

use crate::camera::locus_door::regulate_angle;
use glam::{Mat4, Vec3};

/// Pitch noise amplitude (`0x3b64c388`, `0.2 deg` in radians) in `vcSetDataToVwSystem` (`SLUS_202.28`).
pub const SELF_VIEW_NOISE_RX_AMP: f32 = f32::from_bits(0x3b64_c388);
/// Pitch noise frequency 1 (`0x401c61aa`, `140 deg/s` in rad/s) in `vcSetDataToVwSystem` (`SLUS_202.28`).
pub const SELF_VIEW_NOISE_RX_F1: f32 = f32::from_bits(0x401c_61aa);
/// Pitch noise frequency 2 (`0x3fb2b8c2`, `80 deg/s` in rad/s) in `vcSetDataToVwSystem` (`SLUS_202.28`).
pub const SELF_VIEW_NOISE_RX_F2: f32 = f32::from_bits(0x3fb2_b8c2);

/// Yaw noise amplitude (`0x3b0efa35`, `0.125 deg` in radians) in `vcSetDataToVwSystem` (`SLUS_202.28`).
pub const SELF_VIEW_NOISE_RY_AMP: f32 = f32::from_bits(0x3b0e_fa35);
/// Yaw noise frequency 1 (`0x3f32b8c2`, `40 deg/s` in rad/s) in `vcSetDataToVwSystem` (`SLUS_202.28`).
pub const SELF_VIEW_NOISE_RY_F1: f32 = f32::from_bits(0x3f32_b8c2);
/// Yaw noise frequency 2 (`0xbfb2b8c2`, `-80 deg/s` in rad/s) in `vcSetDataToVwSystem` (`SLUS_202.28`).
pub const SELF_VIEW_NOISE_RY_F2: f32 = f32::from_bits(0xbfb2_b8c2);

/// Matrix element noise amplitude (`0x3b83126f`, `0.004`) applied to columns 0 and 1 in `vcSetDataToVwSystem` (`SLUS_202.28`).
pub const SELF_VIEW_NOISE_MAT_AMP: f32 = f32::from_bits(0x3b83_126f);

/// Matrix noise frequency pairs `(f1, f2)` for rows 0..2 of columns 0 and 1 (`SLUS_202.28` `vcSetDataToVwSystem`).
pub const SELF_VIEW_NOISE_MAT_FREQS: [(f32, f32); 6] = [
    (f32::from_bits(0xbeb2_b8c2), f32::from_bits(0x4049_0fdb)), // m[0][0]
    (f32::from_bits(0xc006_0a92), f32::from_bits(0xbfb2_b8c2)), // m[1][0]
    (f32::from_bits(0xc006_0a92), f32::from_bits(0x3fb2_b8c2)), // m[2][0]
    (f32::from_bits(0x401c_61aa), f32::from_bits(0x401c_61aa)), // m[0][1]
    (f32::from_bits(0x4049_0fdb), f32::from_bits(0x3f32_b8c2)), // m[1][1]
    (f32::from_bits(0xbf9c_61aa), f32::from_bits(0xc011_361e)), // m[2][1]
];

/// Evaluates Team Silent's dual-cosine handheld camera noise oscillator.
///
/// Direct 1:1 port of `vcCamMatNoise` (`SLUS_202.28` `0x0019d8e0`):
/// `((cos(regulate(freq1 * timer)) + cos(regulate(freq2 * timer))) * 0.5) * amp`.
#[inline]
pub fn vc_cam_mat_noise(amp: f32, freq1: f32, freq2: f32, self_view_timer_sec: f32) -> f32 {
    let a1 = regulate_angle(freq1 * self_view_timer_sec);
    let a2 = regulate_angle(freq2 * self_view_timer_sec);
    ((a1.cos() + a2.cos()) * 0.5) * amp
}

/// Evaluates the Mode 3 (`SELF_VIEW`) pitch/yaw noise perturbation `(rx_rad, ry_rad)` at `self_view_timer_sec`
/// (`SLUS_202.28` `vcSetDataToVwSystem` `0x0019d550`).
#[inline]
pub fn self_view_angle_noise(self_view_timer_sec: f32) -> (f32, f32) {
    let rx = vc_cam_mat_noise(
        SELF_VIEW_NOISE_RX_AMP,
        SELF_VIEW_NOISE_RX_F1,
        SELF_VIEW_NOISE_RX_F2,
        self_view_timer_sec,
    );
    let ry = vc_cam_mat_noise(
        SELF_VIEW_NOISE_RY_AMP,
        SELF_VIEW_NOISE_RY_F1,
        SELF_VIEW_NOISE_RY_F2,
        self_view_timer_sec,
    );
    (rx, ry)
}

/// Builds the 4x4 Mode 3 (`SELF_VIEW`) perturbation matrix `noise_mat` multiplied onto `cam_mat`
/// in `vcSetDataToVwSystem` (`SLUS_202.28` `0x0019d550`).
pub fn build_self_view_noise_matrix(self_view_timer_sec: f32) -> Mat4 {
    let (rx, ry) = self_view_angle_noise(self_view_timer_sec);
    let mut m = Mat4::from_euler(glam::EulerRot::YXZ, ry, rx, 0.0);

    let [f00, f10, f20, f01, f11, f21] = SELF_VIEW_NOISE_MAT_FREQS;
    m.x_axis.x += vc_cam_mat_noise(SELF_VIEW_NOISE_MAT_AMP, f00.0, f00.1, self_view_timer_sec);
    m.y_axis.x += vc_cam_mat_noise(SELF_VIEW_NOISE_MAT_AMP, f10.0, f10.1, self_view_timer_sec);
    m.z_axis.x += vc_cam_mat_noise(SELF_VIEW_NOISE_MAT_AMP, f20.0, f20.1, self_view_timer_sec);
    m.x_axis.y += vc_cam_mat_noise(SELF_VIEW_NOISE_MAT_AMP, f01.0, f01.1, self_view_timer_sec);
    m.y_axis.y += vc_cam_mat_noise(SELF_VIEW_NOISE_MAT_AMP, f11.0, f11.1, self_view_timer_sec);
    m.z_axis.y += vc_cam_mat_noise(SELF_VIEW_NOISE_MAT_AMP, f21.0, f21.1, self_view_timer_sec);
    m
}

/// Applies the Mode 3 (`SELF_VIEW`) handheld angle noise `(rx, ry)` to a look direction from `eye` to `target`.
pub fn apply_self_view_noise_to_target(eye: Vec3, target: Vec3, self_view_timer_sec: f32) -> Vec3 {
    let dir = target - eye;
    let len = dir.length();
    if len <= 1e-4 {
        return target;
    }
    let (rx, ry) = self_view_angle_noise(self_view_timer_sec);
    let rot = Mat4::from_euler(glam::EulerRot::YXZ, ry, rx, 0.0);
    eye + rot.transform_vector3(dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vc_cam_mat_noise_t0_and_bounds() {
        // At t = 0: cos(0) == 1.0, so ((1 + 1) / 2) * amp == amp exactly
        let n0 = vc_cam_mat_noise(
            SELF_VIEW_NOISE_RX_AMP,
            SELF_VIEW_NOISE_RX_F1,
            SELF_VIEW_NOISE_RX_F2,
            0.0,
        );
        assert!((n0 - SELF_VIEW_NOISE_RX_AMP).abs() < 1e-7);

        for step in 0..120 {
            let t = step as f32 * (1.0 / 60.0);
            let (rx, ry) = self_view_angle_noise(t);
            assert!(rx.abs() <= SELF_VIEW_NOISE_RX_AMP + 1e-6);
            assert!(ry.abs() <= SELF_VIEW_NOISE_RY_AMP + 1e-6);
            let mat = build_self_view_noise_matrix(t);
            assert!(mat.is_finite());
        }
    }
}
