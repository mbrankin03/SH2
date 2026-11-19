//! Lens flare 7-sprite halo projector (`Lens/lens_flare.c`, `SLUS_202.28` `shLensFlareInit`
//! @ `0x00187a40`, `shLensFlareExec` @ `0x00187af0`, `kari_shLensFlareEffect_Draw` @ `0x00185140`).
//!
//! Uses the exact 7-sprite UV atlas table (`0x002ae0e0`) and base RGB tint table (`0x002ae230`)
//! extracted from `SLUS_202.28` targeting `data/pic/effect/lens_flare.tbn2` (`128x128`).

use glam::{Mat4, Vec2, Vec3, Vec4};

/// Number of halo flare sprites emitted per visible light source (`0x0018519c`: `addiu $4, $zero, 7`).
pub const LENS_FLARE_SPRITE_COUNT: usize = 7;

/// Static UV rectangle and RGB base tint for one of the 7 halo sprites in `lens_flare.tbn2`
/// (`SLUS_202.28` `0x002ae0e0` & `0x002ae230`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LensFlareSpriteSpec {
    /// Minimum `(u, v)` in `lens_flare.tbn2`.
    pub uv_min: [f32; 2],
    /// Maximum `(u, v)` in `lens_flare.tbn2`.
    pub uv_max: [f32; 2],
    /// Base PS2 RGB tint (`0..255` from `0x002ae230`, normalized to `0.0..1.0`).
    pub color_rgb: [f32; 3],
    /// Position along the flare axis (`1.0` = light source screen position, `-1.0` = mirrored across screen center).
    pub axis_t: f32,
    /// Relative screen-height radius scale.
    pub radius_scale: f32,
}

/// Exact 7-sprite `lens_flare.tbn2` table extracted from `SLUS_202.28` (`0x002ae0e0` & `0x002ae230`).
pub const LENS_FLARE_SPRITES: [LensFlareSpriteSpec; LENS_FLARE_SPRITE_COUNT] = [
    // [0]: UV (0.0, 0.0)..(0.5, 0.5), RGB (72, 48, 72)
    LensFlareSpriteSpec {
        uv_min: [0.0, 0.0],
        uv_max: [0.5, 0.5],
        color_rgb: [72.0 / 255.0, 48.0 / 255.0, 72.0 / 255.0],
        axis_t: -0.75,
        radius_scale: 0.14,
    },
    // [1]: UV (0.5, 0.0)..(1.0, 0.5), RGB (40, 44, 44)
    LensFlareSpriteSpec {
        uv_min: [0.5, 0.0],
        uv_max: [1.0, 0.5],
        color_rgb: [40.0 / 255.0, 44.0 / 255.0, 44.0 / 255.0],
        axis_t: -0.45,
        radius_scale: 0.09,
    },
    // [2]: UV (0.25, 0.5)..(0.625, 1.0), RGB (40, 46, 40)
    LensFlareSpriteSpec {
        uv_min: [0.25, 0.5],
        uv_max: [0.625, 1.0],
        color_rgb: [40.0 / 255.0, 46.0 / 255.0, 40.0 / 255.0],
        axis_t: -0.15,
        radius_scale: 0.07,
    },
    // [3]: UV (0.625, 0.5)..(1.0, 1.0), RGB (36, 36, 36)
    LensFlareSpriteSpec {
        uv_min: [0.625, 0.5],
        uv_max: [1.0, 1.0],
        color_rgb: [36.0 / 255.0, 36.0 / 255.0, 36.0 / 255.0],
        axis_t: 0.25,
        radius_scale: 0.08,
    },
    // [4]: UV (0.055, 0.555)..(0.25, 0.75), RGB (40, 40, 32)
    LensFlareSpriteSpec {
        uv_min: [0.055, 0.555],
        uv_max: [0.25, 0.75],
        color_rgb: [40.0 / 255.0, 40.0 / 255.0, 32.0 / 255.0],
        axis_t: 0.55,
        radius_scale: 0.06,
    },
    // [5]: UV (0.055, 0.75)..(0.25, 1.0), RGB (64, 48, 40)
    LensFlareSpriteSpec {
        uv_min: [0.055, 0.75],
        uv_max: [0.25, 1.0],
        color_rgb: [64.0 / 255.0, 48.0 / 255.0, 40.0 / 255.0],
        axis_t: 0.80,
        radius_scale: 0.11,
    },
    // [6]: Primary core glow — UV (0.0, 0.5)..(0.25, 1.0), RGB (128, 128, 128)
    LensFlareSpriteSpec {
        uv_min: [0.0, 0.5],
        uv_max: [0.25, 1.0],
        color_rgb: [128.0 / 255.0, 128.0 / 255.0, 128.0 / 255.0],
        axis_t: 1.0,
        radius_scale: 0.22,
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

/// Evaluates all visible [`LensFlareSource`] emitters against the active camera and produces
/// 7 screen-space [`LensFlareQuad`]s per visible light (`shLensFlareExec` @ `0x00187af0`).
#[must_use]
pub fn evaluate_lens_flares(
    sources: &[LensFlareSource],
    view_proj: Mat4,
    cam_pos: Vec3,
    aspect_ratio: f32,
) -> Vec<LensFlareQuad> {
    let mut quads = Vec::with_capacity(sources.len() * LENS_FLARE_SPRITE_COUNT);
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
            let half_h = spec.radius_scale * (0.5 + 0.5 * dist_fade);
            let half_w = half_h / aspect;
            quads.push(LensFlareQuad {
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
        // Primary core sprite [6] sits directly on the light's NDC position
        assert!((quads[6].ndc_center[0] - quads[0].ndc_center[0] / -0.75).abs() < 1e-3);
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
