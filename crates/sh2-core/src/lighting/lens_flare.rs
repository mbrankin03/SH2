//! Lens flare 7-sprite halo projector (`Lens/lens_flare.c`, `SLUS_202.28` `shLensFlareInit`
//! @ `0x00187a40`, `shLensFlareExec` @ `0x00187af0`, `kari_shLensFlareEffect_Draw` @ `0x00185140`).
//!
//! Uses the exact 7-sprite UV atlas table (`0x002ae0e0`) and base RGB tint table (`0x002ae230`)
//! extracted from `SLUS_202.28` targeting `data/pic/effect/lens_flare.tbn2` (`128x128`).

use glam::{Mat4, Vec2, Vec3, Vec4};

/// Number of halo flare sprites emitted per visible light source (`0x0018519c`: `addiu $4, $zero, 7`).
pub const LENS_FLARE_SPRITE_COUNT: usize = 7;

/// PS2 half-screen vertical resolution (`448 / 2 = 224.0` px) used to normalize `0x002ae1e0`
/// pixel radii into NDC half-heights.
pub const PS2_LENS_FLARE_HALF_HEIGHT_PX: f32 = 224.0;

/// Static UV rectangle, axis position, pixel radius, and RGB base tint for one of the 7 halo
/// sprites in `lens_flare.tbn2` (`SLUS_202.28` `0x002ae0e0`, `0x002ae1c0`, `0x002ae1e0`, `0x002ae230`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LensFlareSpriteSpec {
    /// Minimum `(u, v)` in `lens_flare.tbn2` (`0x002ae0e0`).
    pub uv_min: [f32; 2],
    /// Maximum `(u, v)` in `lens_flare.tbn2` (`0x002ae0e0`).
    pub uv_max: [f32; 2],
    /// Base PS2 RGB tint (`0..255` from `0x002ae230`, normalized to `0.0..1.0`).
    pub color_rgb: [f32; 3],
    /// Axis multiplier from `SLUS_202.28` `0x002ae1c0` (`[0.4, 0.2, -0.3, -0.7, -1.4, -2.2, -2.5]`).
    pub axis_t: f32,
    /// Sprite radius in PS2 screen pixels from `SLUS_202.28` `0x002ae1e0` (`[12, 10, 28, 20, 20, 58, 50]`).
    pub radius_px: f32,
}

/// Exact 7-sprite `lens_flare.tbn2` table extracted verbatim from `SLUS_202.28`:
/// - UVs @ `0x002ae0e0` (7 × 32 bytes)
/// - `axis_t` @ `0x002ae1c0` (`[0.4, 0.2, -0.3, -0.7, -1.4, -2.2, -2.5]`)
/// - `radius_px` @ `0x002ae1e0` (`[12.0, 10.0, 28.0, 20.0, 20.0, 58.0, 50.0]`)
/// - RGB @ `0x002ae230` (7 × 16 bytes)
pub const LENS_FLARE_SPRITES: [LensFlareSpriteSpec; LENS_FLARE_SPRITE_COUNT] = [
    // [0]: UV (0.0, 0.0)..(0.5, 0.5), axis=0.4, r=12px, RGB (72, 48, 72)
    LensFlareSpriteSpec {
        uv_min: [0.0, 0.0],
        uv_max: [0.5, 0.5],
        color_rgb: [72.0 / 255.0, 48.0 / 255.0, 72.0 / 255.0],
        axis_t: 0.4,
        radius_px: 12.0,
    },
    // [1]: UV (0.5, 0.0)..(1.0, 0.5), axis=0.2, r=10px, RGB (40, 44, 44)
    LensFlareSpriteSpec {
        uv_min: [0.5, 0.0],
        uv_max: [1.0, 0.5],
        color_rgb: [40.0 / 255.0, 44.0 / 255.0, 44.0 / 255.0],
        axis_t: 0.2,
        radius_px: 10.0,
    },
    // [2]: UV (0.25, 0.5)..(0.625, 1.0), axis=-0.3, r=28px, RGB (40, 46, 40)
    LensFlareSpriteSpec {
        uv_min: [0.25, 0.5],
        uv_max: [0.625, 1.0],
        color_rgb: [40.0 / 255.0, 46.0 / 255.0, 40.0 / 255.0],
        axis_t: -0.3,
        radius_px: 28.0,
    },
    // [3]: UV (0.625, 0.5)..(1.0, 1.0), axis=-0.7, r=20px, RGB (36, 36, 36)
    LensFlareSpriteSpec {
        uv_min: [0.625, 0.5],
        uv_max: [1.0, 1.0],
        color_rgb: [36.0 / 255.0, 36.0 / 255.0, 36.0 / 255.0],
        axis_t: -0.7,
        radius_px: 20.0,
    },
    // [4]: UV (7/128 = 0.0546875, 71/128 = 0.5546875)..(0.25, 1.0) from 0x002ae160..0x002ae190,
    //      axis=-1.4, r=20px, RGB (40, 40, 32)
    LensFlareSpriteSpec {
        uv_min: [7.0 / 128.0, 71.0 / 128.0],
        uv_max: [0.25, 1.0],
        color_rgb: [40.0 / 255.0, 40.0 / 255.0, 32.0 / 255.0],
        axis_t: -1.4,
        radius_px: 20.0,
    },
    // [5]: UV (7/128 = 0.0546875, 71/128 = 0.5546875)..(0.25, 1.0) from 0x002ae180..0x002ae190,
    //      axis=-2.2, r=58px, RGB (64, 48, 40)
    LensFlareSpriteSpec {
        uv_min: [7.0 / 128.0, 71.0 / 128.0],
        uv_max: [0.25, 1.0],
        color_rgb: [64.0 / 255.0, 48.0 / 255.0, 40.0 / 255.0],
        axis_t: -2.2,
        radius_px: 58.0,
    },
    // [6]: UV (0.0, 0.5)..(0.25, 1.0), axis=-2.5, r=50px, RGB (128, 128, 128)
    LensFlareSpriteSpec {
        uv_min: [0.0, 0.5],
        uv_max: [0.25, 1.0],
        color_rgb: [128.0 / 255.0, 128.0 / 255.0, 128.0 / 255.0],
        axis_t: -2.5,
        radius_px: 50.0,
    },
];

/// World-space street/tunnel light emitter that produces a 7-sprite lens flare.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LensFlareSource {
    /// World-space position in PS2 mm (Y-down).
    pub position: Vec3,
    /// Linear RGB tint multiplier.
    pub color: Vec3,
    /// Maximum visibility distance in PS2 mm.
    pub max_distance: f32,
    /// Intensity multiplier (`0.0..=2.0`).
    pub intensity: f32,
}

/// Evaluated screen-space flare quad ready for additive GPU rendering.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LensFlareQuad {
    /// Center position in Normalized Device Coordinates (`-1..1`).
    pub ndc_center: [f32; 2],
    /// Half-size `(half_w, half_h)` in NDC.
    pub ndc_half_size: [f32; 2],
    /// Minimum `(u, v)` in `lens_flare.tbn2`.
    pub uv_min: [f32; 2],
    /// Maximum `(u, v)` in `lens_flare.tbn2`.
    pub uv_max: [f32; 2],
    /// Pre-scaled additive RGBA color.
    pub color_rgba: [f32; 4],
}

/// Evaluates all visible [`LensFlareSource`] emitters against the active camera and writes
/// 7 screen-space [`LensFlareQuad`]s per visible light into `out` (`shLensFlareExec` @ `0x00187af0`).
///
/// Reusing `out` across frames avoids per-frame heap allocations.
pub fn evaluate_lens_flares_into(
    sources: &[LensFlareSource],
    view_proj: Mat4,
    cam_pos: Vec3,
    aspect_ratio: f32,
    out: &mut Vec<LensFlareQuad>,
) {
    out.clear();
    let aspect = aspect_ratio.max(0.1);

    for src in sources {
        if src.intensity <= 1e-4 || src.max_distance <= 1.0 {
            continue;
        }
        let dist = (src.position - cam_pos).length();
        if dist >= src.max_distance {
            continue;
        }

        let clip = view_proj * Vec4::new(src.position.x, src.position.y, src.position.z, 1.0);
        if clip.w <= 1e-4 {
            continue;
        }
        let ndc = Vec2::new(clip.x / clip.w, clip.y / clip.w);
        // Cull flares far outside the viewport (`shLensFlareLightCenterIsVisible` @ 0x00187090)
        if ndc.x.abs() > 1.25 || ndc.y.abs() > 1.25 {
            continue;
        }

        let dist_fade = (1.0 - dist / src.max_distance).clamp(0.0, 1.0);
        let edge_fade = (1.0 - (ndc.length() / 1.35).clamp(0.0, 1.0)).powf(0.75);
        let alpha = (src.intensity * dist_fade * edge_fade).clamp(0.0, 1.0);
        if alpha <= 1e-4 {
            continue;
        }

        for spec in &LENS_FLARE_SPRITES {
            let center = ndc * spec.axis_t;
            let half_h = (spec.radius_px / PS2_LENS_FLARE_HALF_HEIGHT_PX) * (0.5 + 0.5 * dist_fade);
            let half_w = half_h / aspect;
            out.push(LensFlareQuad {
                ndc_center: [center.x, center.y],
                ndc_half_size: [half_w, half_h],
                uv_min: spec.uv_min,
                uv_max: spec.uv_max,
                color_rgba: [
                    (spec.color_rgb[0] * src.color.x * alpha).clamp(0.0, 1.0),
                    (spec.color_rgb[1] * src.color.y * alpha).clamp(0.0, 1.0),
                    (spec.color_rgb[2] * src.color.z * alpha).clamp(0.0, 1.0),
                    alpha,
                ],
            });
        }
    }
}

/// Evaluates all visible [`LensFlareSource`] emitters against the active camera and produces
/// 7 screen-space [`LensFlareQuad`]s per visible light (`shLensFlareExec` @ `0x00187af0`).
#[must_use]
pub fn evaluate_lens_flares(
    sources: &[LensFlareSource],
    view_proj: Mat4,
    cam_pos: Vec3,
    aspect_ratio: f32,
) -> Vec<LensFlareQuad> {
    if sources.is_empty() {
        return Vec::new();
    }
    let mut quads = Vec::with_capacity(sources.len() * LENS_FLARE_SPRITE_COUNT);
    evaluate_lens_flares_into(sources, view_proj, cam_pos, aspect_ratio, &mut quads);
    quads
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visible_light_emits_exact_seven_halo_sprites() {
        let proj = Mat4::perspective_rh(60f32.to_radians(), 16.0 / 9.0, 50.0, 20_000.0);
        let view = Mat4::look_at_rh(Vec3::ZERO, Vec3::new(0.0, 0.0, -1000.0), Vec3::Y);
        let src = LensFlareSource {
            position: Vec3::new(150.0, -100.0, -2500.0),
            color: Vec3::ONE,
            max_distance: 10_000.0,
            intensity: 1.0,
        };
        let quads = evaluate_lens_flares(&[src], proj * view, Vec3::ZERO, 16.0 / 9.0);
        assert_eq!(quads.len(), LENS_FLARE_SPRITE_COUNT);
        // Verify exact SLUS_202.28 0x002ae1c0 axis ratio between sprite [0] (0.4) and sprite [6] (-2.5)
        assert!((quads[6].ndc_center[0] / -2.5 - quads[0].ndc_center[0] / 0.4).abs() < 1e-3);
    }

    #[test]
    fn light_behind_camera_is_culled() {
        let proj = Mat4::perspective_rh(60f32.to_radians(), 16.0 / 9.0, 50.0, 20_000.0);
        let view = Mat4::look_at_rh(Vec3::ZERO, Vec3::new(0.0, 0.0, -1000.0), Vec3::Y);
        let src = LensFlareSource {
            position: Vec3::new(0.0, 0.0, 1500.0),
            color: Vec3::ONE,
            max_distance: 10_000.0,
            intensity: 1.0,
        };
        let quads = evaluate_lens_flares(&[src], proj * view, Vec3::ZERO, 16.0 / 9.0);
        assert!(quads.is_empty());
    }
}
