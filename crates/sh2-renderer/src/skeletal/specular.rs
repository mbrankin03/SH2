//! 1:1 PS2/PC 64×64 sphere-map specular texture generation, GS dest-alpha additive blend
//! (`ALPHA_1 = 0x58`), and `MakeEnvironPacket` environment reflection helpers (`SH2-135`).
//!
//! # Ground-Truth Citations (`SLUS_202.28` / `sh2pc.exe`)
//! - `mdl_SpecularMappingMakeTexture` (`SLUS_202.28` `0x0012f0c0` / `sh2pc.exe` `0x00501750`):
//!   Generates a `64x64` (`0x40 x 0x40`) RGBA8 sphere-map with:
//!   - `fx = (x / 64.0) * 2.0 - 1.0`, `fy = (y / 64.0) * 2.0 - 1.0`
//!   - `val = powf(sqrtf(clamp(1.0 - (fx*fx + fy*fy), 0.0, 1.0)), 8.0)`
//!   - `rgb = fptosi(val * 255.0)` (truncating cast), `alpha = 0x80` (`128`, `0x43000000`)
//! - `sh2gfw_Set_SpecularAlpha` (`SLUS_202.28` `0x00181be0`):
//!   - Night / interior: GS `ALPHA_1 = 0x58` (`Cs * Ad + Cd`, destination-alpha additive blend)
//!   - Day: GS `ALPHA_1 = 0x54` (`(Cs - Cd) * Ad + Cd`)
//! - `MakeEnvironPacket` (`SLUS_202.28` `0x0012cf20`):
//!   - Environment pass vertex color `(0x80, 0x80, 0x80, *(u8*)(part + 0x45))`
//! - `UpdateReflections` (`SLUS_202.28` `0x00126f90`) & `sh2pc.exe` `FUN_00501540` (`0x00501750`):
//!   - Projects surface normal through the orthonormal half-vector basis `(U, V, H)` with
//!     UV bias `(0.5, 0.5)` (`0x002a9550` / `sh2pc.exe` `0x3f000000`).

use glam::Vec3;

/// Sphere-map texture dimension in texels (`0x40` in `mdl_SpecularMappingMakeTexture` `0x0012f0c0`).
pub const SPECULAR_SPHERE_MAP_DIM: u32 = 64;

/// Exact sphere-map falloff exponent (`powf(sqrtf(1 - r^2), 8.0)` in `0x0012f0c0`).
pub const SPECULAR_SPHERE_MAP_EXPONENT: f32 = 8.0;

/// Fixed PS2 GS alpha byte written into every texel (`fptosi(0x43000000)` = `128` = `0x80` in `0x0012f0c0`).
pub const SPECULAR_SPHERE_MAP_ALPHA_U8: u8 = 0x80;

/// GS `ALPHA_1` register low byte for night/interior specular pass (`sh2gfw_Set_SpecularAlpha` `0x00181be0`).
///
/// Decodes as `A=0 (Cs), B=2 (0), C=1 (Ad), D=1 (Cd)` -> `Cs * Ad + Cd` (destination-alpha additive).
pub const GS_ALPHA_1_SPECULAR_NIGHT: u64 = 0x58;

/// GS `ALPHA_1` register low byte for daytime specular pass (`sh2gfw_Set_SpecularAlpha` `0x00181be0`).
///
/// Decodes as `A=0 (Cs), B=1 (Cd), C=1 (Ad), D=1 (Cd)` -> `(Cs - Cd) * Ad + Cd`.
pub const GS_ALPHA_1_SPECULAR_DAY: u64 = 0x54;

/// Neutral PS2 GS RGB channel byte written by `MakeEnvironPacket` (`0x0012cf20`, `*puVar1 = 0x80`).
pub const ENVIRON_PACKET_RGB_U8: u8 = 0x80;

/// Generates the exact 64×64 RGBA8 specular sphere-map texture matching
/// `mdl_SpecularMappingMakeTexture` (`SLUS_202.28` `0x0012f0c0`).
#[must_use]
pub fn generate_specular_sphere_map_rgba() -> Vec<u8> {
    let dim = SPECULAR_SPHERE_MAP_DIM as usize;
    let mut out = vec![0u8; dim * dim * 4];
    let dim_f = SPECULAR_SPHERE_MAP_DIM as f32;

    for y in 0..dim {
        let fy = (y as f32 / dim_f) * 2.0 - 1.0;
        for x in 0..dim {
            let fx = (x as f32 / dim_f) * 2.0 - 1.0;
            let r2 = (1.0 - (fx * fx + fy * fy)).clamp(0.0, 1.0);
            let val = r2.sqrt().powf(SPECULAR_SPHERE_MAP_EXPONENT);
            // Truncating float-to-int cast matching MIPS `cvt.w.s` (`fptosi`) in `0x0012f0c0`
            let rgb = (val * 255.0).clamp(0.0, 255.0) as u8;
            let idx = (y * dim + x) * 4;
            out[idx] = rgb;
            out[idx + 1] = rgb;
            out[idx + 2] = rgb;
            out[idx + 3] = SPECULAR_SPHERE_MAP_ALPHA_U8;
        }
    }
    out
}

/// Computes the 64×64 sphere-map UV coordinates for a surface normal `n` given `light_dir`
/// (pointing toward the light) and `view_dir` (pointing toward the camera), matching
/// `UpdateReflections` (`0x00126f90`) NHM projection (`0x002a9550` UV bias `[0.5, 0.5]`).
#[must_use]
pub fn compute_sphere_map_uv(normal: Vec3, light_dir: Vec3, view_dir: Vec3) -> [f32; 2] {
    let half = (light_dir + view_dir).normalize_or(Vec3::Z);
    // Avoid collinear cross product when half is parallel to Y
    let ref_up = if half.y.abs() > 0.99 {
        Vec3::X
    } else {
        Vec3::Y
    };
    let u_axis = ref_up.cross(half).normalize_or(Vec3::X);
    let v_axis = half.cross(u_axis).normalize_or(Vec3::Y);
    let u = (0.5 * normal.dot(u_axis) + 0.5).clamp(0.0, 1.0);
    let v = (0.5 * normal.dot(v_axis) + 0.5).clamp(0.0, 1.0);
    [u, v]
}

/// Evaluates the PS2 GS `ALPHA_1` specular composite (`sh2gfw_Set_SpecularAlpha` `0x00181be0`):
/// - Night (`ALPHA_1 = 0x58`): `dest + spec * dest_alpha`
/// - Day (`ALPHA_1 = 0x54`): `dest + (spec - dest) * dest_alpha` (clamped non-negative)
#[must_use]
pub fn composite_gs_specular(
    dest_rgb: Vec3,
    spec_rgb: Vec3,
    dest_alpha: f32,
    is_night: bool,
) -> Vec3 {
    let a = dest_alpha.clamp(0.0, 1.0);
    if is_night {
        dest_rgb + spec_rgb * a
    } else {
        dest_rgb + (spec_rgb - dest_rgb) * a
    }
}

/// Evaluates the `MakeEnvironPacket` (`0x0012cf20`) environment reflection tint weight from
/// part header byte `+0x45` (`0..=128` GS alpha scale).
#[must_use]
#[inline]
pub fn environ_packet_alpha(part_env_alpha_u8: u8) -> f32 {
    f32::from(part_env_alpha_u8) / f32::from(ENVIRON_PACKET_RGB_U8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_specular_sphere_map_dimensions_and_peak() {
        let tex = generate_specular_sphere_map_rgba();
        assert_eq!(tex.len(), (64 * 64 * 4) as usize);

        // Center texel (32, 32): fx = 0.0, fy = 0.0 => sqrt(1)^8 = 1.0 => rgb = 255, alpha = 128
        let center_idx = (32 * 64 + 32) * 4;
        assert_eq!(tex[center_idx], 255);
        assert_eq!(tex[center_idx + 1], 255);
        assert_eq!(tex[center_idx + 2], 255);
        assert_eq!(tex[center_idx + 3], 0x80);

        // Corner texel (0, 0): fx = -1.0, fy = -1.0 => 1 - 2 < 0 => clamped 0 => rgb = 0, alpha = 128
        assert_eq!(tex[0], 0);
        assert_eq!(tex[1], 0);
        assert_eq!(tex[2], 0);
        assert_eq!(tex[3], 0x80);

        // Mid-radius texel (48, 32): fx = 0.5, fy = 0.0 => r2 = 0.75 => 0.75^4 = 0.31640625 => floor(80.6835) = 80
        let mid_idx = (32 * 64 + 48) * 4;
        let expected_mid = ((0.75_f32).sqrt().powf(8.0) * 255.0) as u8;
        assert_eq!(tex[mid_idx], expected_mid);
        assert_eq!(expected_mid, 80);
    }

    #[test]
    fn test_sphere_map_uv_peaks_at_half_vector() {
        let l = Vec3::new(0.0, 0.0, 1.0);
        let v = Vec3::new(0.0, 0.0, 1.0);
        let uv = compute_sphere_map_uv(Vec3::Z, l, v);
        assert!((uv[0] - 0.5).abs() < 1e-5);
        assert!((uv[1] - 0.5).abs() < 1e-5);
    }

    #[test]
    fn test_gs_alpha_1_0x58_dest_alpha_additive() {
        let dest = Vec3::new(0.2, 0.3, 0.4);
        let spec = Vec3::new(0.8, 0.8, 0.8);
        let out = composite_gs_specular(dest, spec, 0.5, true);
        assert!((out - Vec3::new(0.6, 0.7, 0.8)).length() < 1e-5);
    }

    #[test]
    fn test_gs_alpha_1_0x54_daytime_lerp() {
        let dest = Vec3::new(0.2, 0.3, 0.4);
        let spec = Vec3::new(0.8, 0.7, 0.6);
        let out = composite_gs_specular(dest, spec, 0.5, false);
        // (Cs - Cd) * Ad + Cd = mix(Cd, Cs, Ad) = (0.5, 0.5, 0.5)
        assert!((out - Vec3::new(0.5, 0.5, 0.5)).length() < 1e-5);
    }
}
