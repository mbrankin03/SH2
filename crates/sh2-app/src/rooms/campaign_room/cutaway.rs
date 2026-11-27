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

use crate::rooms::story_progression_room::hud_common::{
    blit_cutaway_into_rasterizer, clip_quad_triangles_to_viewport, ensure_cutaway_cached,
};
use crate::state::AppState;
use sh2_assets::{CutawayTextureAsset, CutawayViewportTransform};
use sh2_core::{
    riddle_level_index, CutawayAspectMode, CutawayQuality, FullscreenImageLayout, OverlaySpriteQuad,
};
use sh2_ui::TextRasterizer;

/// Exact fullscreen black dim overlay alpha (`64 / 128 = 0.50 -> 128 / 255`) drawn over cutaway
/// stills while an item/inspection message is active.
///
/// Verified 1:1 against `SLUS_202.28` `EvSubPictureFilter` (`0x001acdf0`) + `EvSubPictureEnd`
/// (`0x001ace70`, `Event/event_sub.c:590-604`: `R=0, G=0, B=0, FIX = (int)(_ev_filter * 64.0)`)
/// and retail `sh2pc.exe` `0x005a0b3d` (Stage 3 `out_p_incar` map pickup step 2).
pub(crate) const EV_SUB_PICTURE_FILTER_ALPHA_U8: u8 = 128;

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
struct CutawayBaseKey {
    width: u32,
    height: u32,
    base_pic: String,
    quality: CutawayQuality,
    aspect_mode: CutawayAspectMode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CutawayQuadKey {
    quad_bits: Vec<(String, u32, [u32; 16])>,
}

impl CutawayQuadKey {
    fn from_quads(quads: &[OverlaySpriteQuad]) -> Self {
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
        Self { quad_bits }
    }
}

#[inline]
fn union_row_spans(a: Option<(usize, usize)>, b: Option<(usize, usize)>) -> Option<(usize, usize)> {
    match (a, b) {
        (Some((a0, a1)), Some((b0, b1))) => Some((a0.min(b0), a1.max(b1))),
        (Some(span), None) | (None, Some(span)) => Some(span),
        (None, None) => None,
    }
}

#[inline]
fn clipped_quad_row_span(
    clipped: &[(f32, f32, f32, f32); 6],
    height: usize,
) -> Option<(usize, usize)> {
    let min_y = clipped
        .iter()
        .map(|v| v.1)
        .fold(f32::INFINITY, f32::min)
        .floor()
        .max(0.0) as usize;
    let max_y = (clipped
        .iter()
        .map(|v| v.1)
        .fold(f32::NEG_INFINITY, f32::max)
        .ceil()
        .max(0.0) as usize)
        .min(height);
    (min_y < max_y).then_some((min_y, max_y))
}

/// Two-level CPU cutaway compositor that caches the static rasterized base still + pillarbox
/// backdrop (`base_pixels`) and only restores, composites, and uploads the dirty vertical row
/// band when animated [`OverlaySpriteQuad`] overlays (e.g., `out/p_incar_map` alpha fade,
/// `dls_p_manacles_on` fade, rotating clock hands) change frame-to-frame.
pub(crate) struct CutawayCompositor {
    pub(crate) rasterizer: TextRasterizer,
    base_key: Option<CutawayBaseKey>,
    base_pixels: Vec<u8>,
    base_vp_transform: CutawayViewportTransform,
    base_fs_layout: Option<FullscreenImageLayout>,
    last_quad_key: Option<CutawayQuadKey>,
    last_quad_dirty_rows: Option<(usize, usize)>,
}

impl CutawayCompositor {
    pub(crate) fn new(width: u32, height: u32) -> Self {
        Self {
            rasterizer: TextRasterizer::new(width as usize, height as usize),
            base_key: None,
            base_pixels: Vec::new(),
            base_vp_transform: CutawayViewportTransform {
                offset_x: 0.0,
                offset_y: 0.0,
                draw_width: width as f32,
                draw_height: height as f32,
                scale_x: 1.0,
                scale_y: 1.0,
            },
            base_fs_layout: None,
            last_quad_key: None,
            last_quad_dirty_rows: None,
        }
    }

    /// Updates `self.rasterizer` for `(base_pic, quality, aspect_mode, quads)` and returns the
    /// dirty vertical row band `Some((dirty_y0, dirty_y1))` that changed and must be uploaded to
    /// the GPU texture, or `None` when the frame is identical to the previous call.
    pub(crate) fn update(
        &mut self,
        cache: &mut HashMap<(String, CutawayQuality), CutawayTextureAsset>,
        base_pic: &str,
        quality: CutawayQuality,
        aspect_mode: CutawayAspectMode,
        quads: &[OverlaySpriteQuad],
    ) -> Option<(usize, usize)> {
        let w = self.rasterizer.width;
        let h = self.rasterizer.height;
        let base_key = CutawayBaseKey {
            width: w as u32,
            height: h as u32,
            base_pic: base_pic.to_string(),
            quality,
            aspect_mode,
        };
        let quad_key = CutawayQuadKey::from_quads(quads);

        if self.base_key.as_ref() != Some(&base_key) {
            let fs_layout =
                FullscreenImageLayout::compute(w as f32, h as f32, base_pic, aspect_mode);
            let vp_transform = render_cutaway_frame_into_rasterizer(
                &mut self.rasterizer,
                cache,
                base_pic,
                quality,
                aspect_mode,
                &[],
            );
            self.base_pixels.resize(self.rasterizer.pixels.len(), 0);
            self.base_pixels.copy_from_slice(&self.rasterizer.pixels);

            let clipped_quads = collect_clipped_active_quads(quads, &vp_transform, &fs_layout, h);
            let new_quad_dirty = clipped_quads
                .iter()
                .fold(None, |acc, &(_, _, span)| union_row_spans(acc, Some(span)));
            composite_clipped_quads_and_bars(
                &mut self.rasterizer,
                cache,
                quality,
                quads,
                &clipped_quads,
                &fs_layout,
                0,
                h,
            );

            self.base_key = Some(base_key);
            self.base_vp_transform = vp_transform;
            self.base_fs_layout = Some(fs_layout);
            self.last_quad_key = Some(quad_key);
            self.last_quad_dirty_rows = new_quad_dirty;
            return Some((0, h));
        }

        if self.last_quad_key.as_ref() == Some(&quad_key) {
            return None;
        }

        let fs_layout = self
            .base_fs_layout
            .as_ref()
            .expect("base_fs_layout populated when base_key matches");
        let clipped_quads =
            collect_clipped_active_quads(quads, &self.base_vp_transform, fs_layout, h);
        let new_quad_dirty = clipped_quads
            .iter()
            .fold(None, |acc, &(_, _, span)| union_row_spans(acc, Some(span)));
        let dirty_span = union_row_spans(self.last_quad_dirty_rows, new_quad_dirty);

        self.last_quad_key = Some(quad_key);
        self.last_quad_dirty_rows = new_quad_dirty;

        let (dirty_y0, dirty_y1) = dirty_span?;
        if dirty_y0 >= dirty_y1 {
            return None;
        }

        let stride = w * 4;
        let byte_start = dirty_y0 * stride;
        let byte_end = dirty_y1 * stride;
        self.rasterizer.pixels[byte_start..byte_end]
            .copy_from_slice(&self.base_pixels[byte_start..byte_end]);

        composite_clipped_quads_and_bars(
            &mut self.rasterizer,
            cache,
            quality,
            quads,
            &clipped_quads,
            fs_layout,
            dirty_y0,
            dirty_y1,
        );

        Some((dirty_y0, dirty_y1))
    }
}

type ClippedQuadEntry = (usize, [(f32, f32, f32, f32); 6], (usize, usize));

fn collect_clipped_active_quads(
    quads: &[OverlaySpriteQuad],
    vp_transform: &CutawayViewportTransform,
    fs_layout: &FullscreenImageLayout,
    height: usize,
) -> Vec<ClippedQuadEntry> {
    let cx0 = fs_layout.canvas.canvas_left;
    let cy0 = fs_layout.canvas.canvas_top;
    let cx1 = fs_layout.canvas.canvas_right;
    let cy1 = fs_layout.canvas.canvas_bottom;
    let mut out = Vec::with_capacity(quads.len());
    for (idx, quad) in quads.iter().enumerate() {
        if quad.alpha <= 0.0 {
            continue;
        }
        let tris = quad.to_viewport_triangles(
            vp_transform.offset_x,
            vp_transform.offset_y,
            vp_transform.draw_width,
            vp_transform.draw_height,
        );
        if let Some(clipped) = clip_quad_triangles_to_viewport(&tris, cx0, cy0, cx1, cy1) {
            if let Some(span) = clipped_quad_row_span(&clipped, height) {
                out.push((idx, clipped, span));
            }
        }
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn composite_clipped_quads_and_bars(
    rast: &mut TextRasterizer,
    cache: &mut HashMap<(String, CutawayQuality), CutawayTextureAsset>,
    quality: CutawayQuality,
    quads: &[OverlaySpriteQuad],
    clipped_quads: &[ClippedQuadEntry],
    fs_layout: &FullscreenImageLayout,
    dirty_y0: usize,
    dirty_y1: usize,
) {
    for &(quad_idx, ref clipped, _) in clipped_quads {
        let quad = &quads[quad_idx];
        ensure_cutaway_cached(cache, quad.sheet_name, quality);
        if let Some(sheet_tex) = cache.get(&(quad.sheet_name.to_string(), quality)) {
            rast.blit_overlay_triangles(
                clipped,
                &sheet_tex.texture.rgba,
                sheet_tex.texture.width,
                sheet_tex.texture.height,
                quad.alpha,
            );
        }
    }

    if !clipped_quads.is_empty() {
        for bar in &fs_layout.black_bars {
            if bar.is_non_empty() {
                let bx = bar.x0.floor().max(0.0) as usize;
                let by = bar.y0.floor().max(0.0) as usize;
                let bw = bar.width().ceil() as usize;
                let bh = bar.height().ceil() as usize;
                let iy0 = by.max(dirty_y0);
                let iy1 = by.saturating_add(bh).min(dirty_y1);
                if iy0 < iy1 {
                    rast.fill_rect(bx, iy0, bw, iy1 - iy0, [0, 0, 0, 255]);
                }
            }
        }
    }
}

struct NativeCutawaySurface {
    width: u32,
    height: u32,
    compositor: CutawayCompositor,
    texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
}

impl NativeCutawaySurface {
    fn new(state: &AppState, width: u32, height: u32) -> Self {
        let compositor = CutawayCompositor::new(width, height);
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
            compositor,
            texture,
            bind_group,
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
    let (quality, aspect_mode) =
        if video.texture_quality == sh2_core::config::WorldTextureQuality::OriginalPs2 {
            (
                CutawayQuality::OriginalPs2,
                CutawayAspectMode::Fit4x3Pillarbox,
            )
        } else {
            (video.cutaway_quality, video.cutaway_aspect_mode)
        };

    let target_w = state
        .surface_config
        .width
        .clamp(1, MAX_NATIVE_CUTAWAY_WIDTH);
    let target_h = state
        .surface_config
        .height
        .clamp(1, MAX_NATIVE_CUTAWAY_HEIGHT);

    NATIVE_CUTAWAY_SURFACE.with(|cell| {
        let mut slot = cell.borrow_mut();
        let needs_alloc = slot
            .as_ref()
            .is_none_or(|s| s.width != target_w || s.height != target_h);
        if needs_alloc {
            *slot = Some(NativeCutawaySurface::new(state, target_w, target_h));
        }
        let surface = slot.as_mut().expect("NativeCutawaySurface allocated");
        if let Some((dirty_y0, dirty_y1)) = surface.compositor.update(
            &mut state.story_progression_room.cutaway_cache,
            &base_pic,
            quality,
            aspect_mode,
            &quads,
        ) {
            let stride = 4 * surface.width as usize;
            let dirty_h = (dirty_y1 - dirty_y0) as u32;
            let byte_start = dirty_y0 * stride;
            let byte_end = dirty_y1 * stride;
            state.render_ctx.queue.write_texture(
                wgpu::ImageCopyTexture {
                    texture: &surface.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: 0,
                        y: dirty_y0 as u32,
                        z: 0,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                &surface.compositor.rasterizer.pixels[byte_start..byte_end],
                wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(4 * surface.width),
                    rows_per_image: Some(dirty_h),
                },
                wgpu::Extent3d {
                    width: surface.width,
                    height: dirty_h,
                    depth_or_array_layers: 1,
                },
            );
        }

        state.blitter.render_with_bind_group_and_load_op(
            encoder,
            &surface.bind_group,
            surface_view,
            wgpu::LoadOp::Load,
        );
    });
}
