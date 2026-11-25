//! GPU render-item builder for `BloodEffectSystem` (`sh2-renderer` `BloodEffectPipeline`).

use super::system::BloodEffectSystem;
use super::tables::{
    blood_color_non_subtractive_rgba_norm, blood_color_subtractive_rgb_norm, BLOOD_00_MOTION_INFO,
    BLOOD_00_SCALE_END_MM, BLOOD_00_UV_RECT, BLOOD_01_DROP_UV_RECTS, BLOOD_01_GRAVITY_MM_S2,
    BLOOD_01_HALF_EXTENT_MM, BLOOD_01_MIST_UV_RECTS, BLOOD_02_MOTION_INFO, BLOOD_02_UV_RECTS,
    BLOOD_FOOTMARK_BASE_ALPHA_GS, BLOOD_FOOTMARK_HALF_LENGTH_MM, BLOOD_FOOTMARK_HALF_WIDTH_MM,
    BLOOD_FOOTMARK_UV_RECTS,
};
use super::types::{
    evaluate_blood_parabola, evaluate_three_stage_envelope, BloodQuadKind, BloodQuadRenderItem,
    BloodSystemConfig,
};

impl BloodEffectSystem {
    /// Builds GPU render items for `sh2-renderer`'s `BloodEffectPipeline`.
    pub fn build_render_items(
        &self,
        config: BloodSystemConfig,
        out: &mut Vec<BloodQuadRenderItem>,
    ) {
        out.clear();
        let subtractive = !config.non_subtractive_fallback;
        let base_rgb = if subtractive {
            blood_color_subtractive_rgb_norm(config.blood_color)
        } else {
            let rgba = blood_color_non_subtractive_rgba_norm(config.blood_color);
            [rgba[0], rgba[1], rgba[2]]
        };

        for p in &self.pools {
            let (_, a_t) = evaluate_three_stage_envelope(p.elapsed_sec, &BLOOD_02_MOTION_INFO);
            let [hx, hz] = p.current_half_extents();
            if hx <= 0.5 || hz <= 0.5 {
                continue;
            }
            let coag_mul = 1.0 - 0.15 * p.coagulation;
            out.push(BloodQuadRenderItem {
                kind: BloodQuadKind::FloorDecal,
                position_mm: p.position_mm,
                half_extents_mm: [hx, hz],
                yaw_rad: p.yaw_rad,
                uv_rect: BLOOD_02_UV_RECTS[p.uv_variant % BLOOD_02_UV_RECTS.len()],
                color_rgb: [
                    base_rgb[0] * coag_mul,
                    base_rgb[1] * coag_mul,
                    base_rgb[2] * coag_mul,
                ],
                alpha: (0.35 + 0.65 * a_t).clamp(0.0, 1.0),
                subtractive,
            });
        }

        for s in &self.stains {
            let (s_t, _) = evaluate_three_stage_envelope(s.elapsed_sec, &BLOOD_00_MOTION_INFO);
            let end = BLOOD_00_SCALE_END_MM[s.variant % BLOOD_00_SCALE_END_MM.len()];
            let hx = end[0] * s_t * config.blood_size_scale;
            let hz = end[1] * s_t * config.blood_size_scale;
            if hx <= 0.5 || hz <= 0.5 {
                continue;
            }
            out.push(BloodQuadRenderItem {
                kind: BloodQuadKind::FloorDecal,
                position_mm: s.position_mm,
                half_extents_mm: [hx, hz],
                yaw_rad: s.yaw_rad,
                uv_rect: BLOOD_00_UV_RECT,
                color_rgb: base_rgb,
                alpha: s.fade_alpha.clamp(0.0, 1.0),
                subtractive,
            });
        }

        for f in &self.footmarks {
            if f.alpha_scale <= 0.004 {
                continue;
            }
            let side_idx = usize::from(f.side & 1);
            let var_idx = usize::from(f.variant & 1);
            out.push(BloodQuadRenderItem {
                kind: BloodQuadKind::FloorDecal,
                position_mm: f.position_mm,
                half_extents_mm: [BLOOD_FOOTMARK_HALF_WIDTH_MM, BLOOD_FOOTMARK_HALF_LENGTH_MM],
                yaw_rad: f.yaw_rad,
                uv_rect: BLOOD_FOOTMARK_UV_RECTS[side_idx][var_idx],
                color_rgb: base_rgb,
                alpha: (BLOOD_FOOTMARK_BASE_ALPHA_GS / 128.0 * f.alpha_scale).clamp(0.0, 1.0),
                subtractive,
            });
        }

        for p in &self.particles {
            let world_pos = p.origin_mm
                + evaluate_blood_parabola(p.vel_mm_s, BLOOD_01_GRAVITY_MM_S2, p.elapsed_sec);
            let r = BLOOD_01_HALF_EXTENT_MM * p.scale;
            let uv_rect = if p.mode == 0 {
                BLOOD_01_MIST_UV_RECTS[usize::from(p.quadrant & 3)]
            } else {
                BLOOD_01_DROP_UV_RECTS[usize::from(p.quadrant & 3)]
            };
            out.push(BloodQuadRenderItem {
                kind: BloodQuadKind::Billboard,
                position_mm: world_pos,
                half_extents_mm: [r, r],
                yaw_rad: 0.0,
                uv_rect,
                color_rgb: base_rgb,
                alpha: (p.alpha_gs / 128.0).clamp(0.0, 1.0),
                subtractive,
            });
        }
    }
}
