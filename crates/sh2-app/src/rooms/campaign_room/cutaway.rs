//! Fullscreen native-resolution 2D cutaway still & animated sprite overlay rendering for unified
//! Campaign mode (`SH2-157` / `SH2-162`).
//!
//! Reuses [`crate::rooms::story_progression_room::hud_common::blit_cutaway_into_rasterizer`]
//! and `AppState::story_progression_room.cutaway_cache` so retail cutaway `.tex` loading,
//! `FullscreenImageLayout` aspect-ratio framing, `OverlaySpriteQuad` compositing (e.g.,
//! `out_p_incar.tex` + `out/p_incar_map.tex` at `(-48.375, -31.625)` with alpha fade), and
//! SH2:EE pillarbox bars have a single source of truth — rendered at full native window resolution
//! (up to `3840x2160`) rather than downscaled into the fixed `1280x720` debug HUD buffer.

use std::cell::RefCell;
use std::collections::HashMap;

use crate::rooms::story_progression_room::hud_common::blit_cutaway_into_rasterizer;
use crate::state::AppState;
use sh2_assets::{CutawayTextureAsset, CutawayViewportTransform};
use sh2_core::{riddle_level_index, CutawayAspectMode, CutawayQuality, OverlaySpriteQuad};
use sh2_ui::TextRasterizer;

/// Maximum native cutaway surface width (4K UHD).
const MAX_NATIVE_CUTAWAY_WIDTH: u32 = 3840;
/// Maximum native cutaway surface height (4K UHD).
const MAX_NATIVE_CUTAWAY_HEIGHT: u32 = 2160;

/// Ratio of a square `512x512` PS2/PC cutaway texture's full height (`512`) to its active
/// `512x384` (`4:3`) image band (`y = 64..448`, `v = 0.125..0.875`).
///
/// Verified against `PictureDraw` (`SLUS_202.28` `0x001ae360`, drawing `[-256..+256]` in 12.4
/// GS fixed-point `0xf000..0x1000`) and retail `data/pic/*/*.tex` (`out/p_incar.tex`,
/// `apt/p_safe_close.tex`, `apt/p_desk_coin.tex`, `hsp/p_box.tex`, `apt/clock_close.tex`).
pub(crate) const PS2_CUTAWAY_LETTERBOX_HEIGHT_RATIO: f32 = 512.0 / 384.0;

/// Returns `true` when a square `512x512`-authored PS2/PC cutaway texture (`512x512`, `2048x2048`,
/// or `4096x4096`) contains the standard `64/512` (`12.5%`) baked top and bottom black letterbox
/// padding around its active `512x384` (`4:3`) artwork (`SH2-162`).
#[must_use]
pub(crate) fn has_baked_ps2_letterbox_bars(rgba: &[u8], width: usize, height: usize) -> bool {
    if width != height || width < 64 || rgba.len() < width * height * 4 {
        return false;
    }
    let bar_h = height / 8;
    // Leave a 1-tile seam margin inside the 1/8 boundary for 4x4 DXT blocks / upscaler halo.
    let seam_margin = (bar_h / 8).max(4);
    let top_end = bar_h.saturating_sub(seam_margin);
    let bot_start = (height - bar_h + seam_margin).min(height);
    let step_x = (width / 32).max(1);
    let step_y = (bar_h / 8).max(1);

    let is_near_black = |x: usize, y: usize| -> bool {
        let idx = (y * width + x) * 4;
        rgba[idx] <= 8 && rgba[idx + 1] <= 8 && rgba[idx + 2] <= 8
    };

    for y in (0..top_end).step_by(step_y) {
        for x in (0..width).step_by(step_x) {
            if !is_near_black(x, y) {
                return false;
            }
        }
    }
    for y in (bot_start..height).step_by(step_y) {
        for x in (0..width).step_by(step_x) {
            if !is_near_black(x, y) {
                return false;
            }
        }
    }

    let center_step_y = ((height - 2 * bar_h) / 16).max(1);
    for y in (bar_h + 2..height.saturating_sub(bar_h + 2)).step_by(center_step_y) {
        for x in (0..width).step_by(step_x) {
            let idx = (y * width + x) * 4;
            if rgba[idx] > 12 || rgba[idx + 1] > 12 || rgba[idx + 2] > 12 {
                return true;
            }
        }
    }
    false
}

/// Computes the exact destination quad transform for `base_tex` inside `(vp_w, vp_h)` under
/// `fs_layout`, expanding square `512x512`-authored PS2/PC cutaways with baked `64/512` top/bottom
/// padding (`has_baked_ps2_letterbox_bars`) so their active `512x384` artwork fills 100% of the
/// active `4:3` pillarbox height (zero 4-sided windowboxing) or 100% of the `16:9` widescreen
/// viewport while keeping `-256..+256` [`OverlaySpriteQuad`] overlays pixel-locked (`SH2-162`).
#[must_use]
pub(crate) fn compute_cutaway_draw_rect(
    vp_w: f32,
    vp_h: f32,
    fs_layout: &sh2_core::FullscreenImageLayout,
    base_tex: &CutawayTextureAsset,
) -> CutawayViewportTransform {
    let vw = vp_w.max(1.0);
    let vh = vp_h.max(1.0);
    if fs_layout.is_widescreen_cropped {
        return CutawayViewportTransform {
            offset_x: fs_layout.draw_x,
            offset_y: fs_layout.draw_y,
            draw_width: fs_layout.draw_w,
            draw_height: fs_layout.draw_h,
            scale_x: fs_layout.draw_w / vw,
            scale_y: fs_layout.draw_h / vh,
        };
    }

    if has_baked_ps2_letterbox_bars(
        &base_tex.texture.rgba,
        base_tex.texture.width,
        base_tex.texture.height,
    ) {
        let canvas_w = fs_layout.canvas.canvas_width();
        let canvas_h = fs_layout.canvas.canvas_height();
        let draw_w = canvas_w;
        let draw_h = canvas_h * PS2_CUTAWAY_LETTERBOX_HEIGHT_RATIO;
        let offset_x = fs_layout.canvas.canvas_left;
        let offset_y = fs_layout.canvas.canvas_top + (canvas_h - draw_h) * 0.5;
        return CutawayViewportTransform {
            offset_x,
            offset_y,
            draw_width: draw_w,
            draw_height: draw_h,
            scale_x: draw_w / vw,
            scale_y: draw_h / vh,
        };
    }

    base_tex.compute_viewport_transform(vw, vh)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CutawayRenderKey {
    width: u32,
    height: u32,
    base_pic: String,
    quality: CutawayQuality,
    aspect_mode: CutawayAspectMode,
    quad_bits: Vec<(String, u32, [u32; 16])>,
}

impl CutawayRenderKey {
    fn from_state(
        width: u32,
        height: u32,
        base_pic: &str,
        quality: CutawayQuality,
        aspect_mode: CutawayAspectMode,
        quads: &[OverlaySpriteQuad],
    ) -> Self {
        let quad_bits = quads
            .iter()
            .map(|q| {
                let mut v = [0u32; 16];
                for (i, &(x, y, tu, tv)) in q.vertices.iter().enumerate() {
                    v[i * 4] = x.to_bits();
                    v[i * 4 + 1] = y.to_bits();
                    v[i * 4 + 2] = tu.to_bits();
                    v[i * 4 + 3] = tv.to_bits();
                }
                (q.sheet_name.to_string(), q.alpha.to_bits(), v)
            })
            .collect();
        Self {
            width,
            height,
            base_pic: base_pic.to_string(),
            quality,
            aspect_mode,
            quad_bits,
        }
    }
}

struct NativeCutawaySurface {
    width: u32,
    height: u32,
    rasterizer: TextRasterizer,
    texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
    last_key: Option<CutawayRenderKey>,
}

impl NativeCutawaySurface {
    fn new(state: &AppState, width: u32, height: u32) -> Self {
        let rasterizer = TextRasterizer::new(width as usize, height as usize);
        let texture = state
            .render_ctx
            .device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("Campaign Native Cutaway Texture"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = state.blitter.create_bind_group_with_uniform_buffer(
            &state.render_ctx.device,
            &view,
            &state.menu_uniform_buffer,
        );
        Self {
            width,
            height,
            rasterizer,
            texture,
            bind_group,
            last_key: None,
        }
    }
}

thread_local! {
    static NATIVE_CUTAWAY_SURFACE: RefCell<Option<NativeCutawaySurface>> = const { RefCell::new(None) };
}

/// Returns `true` when `sim_core.scenario.cutaway_overlay` currently has an active 2D cutaway
/// background picture (e.g., `"out_p_incar"`).
#[must_use]
pub(crate) fn has_active_cutaway(state: &AppState) -> bool {
    state
        .sim_core
        .scenario
        .cutaway_overlay
        .active_picture()
        .is_some()
}

/// Renders a cutaway still (`base_pic`) and all active `OverlaySpriteQuad` overlays fullscreen
/// into an arbitrary target [`TextRasterizer`] (e.g., native `1920x1080` or `3840x2160`) with
/// opaque black pillarbox backdrop (`SH2-162`).
pub(crate) fn render_cutaway_frame_into_rasterizer(
    rast: &mut TextRasterizer,
    cache: &mut HashMap<(String, CutawayQuality), CutawayTextureAsset>,
    base_pic: &str,
    quality: CutawayQuality,
    aspect_mode: CutawayAspectMode,
    quads: &[OverlaySpriteQuad],
) -> CutawayViewportTransform {
    let w = rast.width;
    let h = rast.height;
    // Clear fullscreen backdrop to opaque black so 3D scene pixels never bleed around pillarboxes.
    rast.fill_rect(0, 0, w, h, [0, 0, 0, 255]);
    blit_cutaway_into_rasterizer(
        rast,
        cache,
        0,
        0,
        w,
        h,
        base_pic,
        quality,
        aspect_mode,
        quads,
    )
}

/// Renders the active `state.sim_core.scenario.cutaway_overlay` at full native window resolution
/// (`state.surface_config.width x state.surface_config.height`, clamped to `3840x2160`) and blits
/// it edge-to-edge onto `surface_view` (`SH2-162`).
pub(crate) fn render_campaign_cutaway_native(
    state: &mut AppState,
    encoder: &mut wgpu::CommandEncoder,
    surface_view: &wgpu::TextureView,
) {
    let Some(base_pic) = state
        .sim_core
        .scenario
        .cutaway_overlay
        .active_picture()
        .map(str::to_string)
    else {
        return;
    };

    // Ensure `frame_sprites` are up-to-date for the current flags/puzzle/alpha state.
    let riddle_idx = riddle_level_index(state.sim_core.scenario.riddle_difficulty);
    let quads = state
        .sim_core
        .scenario
        .cutaway_overlay
        .evaluate_frame(
            &state.sim_core.scenario.flags,
            &state.sim_core.scenario.puzzle_state,
            riddle_idx,
            0,
        )
        .to_vec();

    let video = &state.menu_ctrl.options_menu.draft_config.video;
    let quality = if video.texture_quality == sh2_core::config::WorldTextureQuality::OriginalPs2 {
        CutawayQuality::OriginalPs2
    } else {
        video.cutaway_quality
    };
    let aspect_mode = video.cutaway_aspect_mode;

    let target_w = state
        .surface_config
        .width
        .clamp(1, MAX_NATIVE_CUTAWAY_WIDTH);
    let target_h = state
        .surface_config
        .height
        .clamp(1, MAX_NATIVE_CUTAWAY_HEIGHT);
    let key =
        CutawayRenderKey::from_state(target_w, target_h, &base_pic, quality, aspect_mode, &quads);

    NATIVE_CUTAWAY_SURFACE.with(|cell| {
        let mut slot = cell.borrow_mut();
        let needs_alloc = slot
            .as_ref()
            .is_none_or(|s| s.width != target_w || s.height != target_h);
        if needs_alloc {
            *slot = Some(NativeCutawaySurface::new(state, target_w, target_h));
        }
        let surface = slot.as_mut().expect("NativeCutawaySurface allocated");
        if surface.last_key.as_ref() != Some(&key) {
            render_cutaway_frame_into_rasterizer(
                &mut surface.rasterizer,
                &mut state.story_progression_room.cutaway_cache,
                &base_pic,
                quality,
                aspect_mode,
                &quads,
            );
            state.render_ctx.queue.write_texture(
                wgpu::ImageCopyTexture {
                    texture: &surface.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &surface.rasterizer.pixels,
                wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(4 * surface.width),
                    rows_per_image: Some(surface.height),
                },
                wgpu::Extent3d {
                    width: surface.width,
                    height: surface.height,
                    depth_or_array_layers: 1,
                },
            );
            surface.last_key = Some(key);
        }

        state.blitter.render_with_bind_group_and_load_op(
            encoder,
            &surface.bind_group,
            surface_view,
            wgpu::LoadOp::Load,
        );
    });
}
