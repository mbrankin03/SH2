//! SH2:EE `FullscreenImages.cpp` / `FullscreenImages.h` aspect-ratio scaling & pixel-locked
//! `-256..256` overlay alignment across `4:3`, `16:9`, `20:9`, `21:9`, and `32:9` displays (`SH2-91`).
//!
//! # Ground Truth & SH2:EE Reference
//! - **`SetFullscreenImagesRes` (`Patches/FullscreenImages.cpp:553..578`)**:
//!   - `Ratio = GameResX / GameResY`
//!   - `alg1 = 4.0 / 3.0`, `alg2 = 16.0 / 9.0`
//!   - `alg3 = Ratio * 0.75`
//!   - `alg4 = 1.25 + (Ratio - 1.25) * (alg1 - 1.25) / (alg2 - 1.25)`
//!   - `FIT_MEDIA` (4:3 Pillarboxed): preserves full 4:3 framing without cropping (`min(1.25, alg3)` in SH2:EE D3D8,
//!     or exact `4:3` fit in native viewport coordinates).
//!   - `FILL_MEDIA` (16:9 Widescreen Zoom/Crop): scales `IsScaled == true` cutaways by `min(alg2 * 0.75, alg4)`
//!     (`4.0 / 3.0` on `16:9`, `21:9`, `32:9`) so `16:9` fills edge-to-edge while cropping `12.5%` top/bottom,
//!     and ultrawide `21:9`/`32:9` caps at `16:9` with black pillarboxes outside `16:9`.
//! - **`DefaultTextureList` (`Patches/FullscreenImages.h:13..393`)**:
//!   - Maps, map markings, inventory examine stills (`pic/item/x_*`, `p_laura_letter`, `p_letterm*`,
//!     `p_memo`, `p_lostmemory`, `p_redreling`, `x*houban`), `hsp/p_panel`, `htl/p_hanging2`,
//!     `gameover1`, `start00*`, and `savebg*` have `IsScaled = false` so edge-critical elements are
//!     never cropped even when `Fill16x9WidescreenCrop` is active.

use serde::{Deserialize, Serialize};
use sh2_assets::formats::demo::AUTHORED_4X3_ASPECT;

use super::pillarbox::{PillarboxMaskRect, PillarboxViewportLayout};
use super::OverlaySpriteQuad;

/// Maximum widescreen zoom aspect ratio (`16:9`) in `FullscreenImages.cpp:561` (`alg2 = 16.0f / 9.0f`).
pub const FULLSCREEN_MAX_WIDESCREEN_ASPECT: f32 = 16.0 / 9.0;

/// Cutaway & puzzle image aspect-ratio scaling mode (`FullscreenImages.cpp:566..578`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum CutawayAspectMode {
    /// Authentic `4:3` pillarboxed framing with zero top/bottom cropping (`FIT_MEDIA`).
    Fit4x3Pillarbox,
    /// SH2:EE `16:9` widescreen zoom/crop (`FILL_MEDIA`) for `IsScaled == true` cutaways;
    /// automatically preserves `4:3` on edge-critical puzzles/maps/items (`IsScaled == false`).
    #[default]
    Fill16x9WidescreenCrop,
}

impl CutawayAspectMode {
    /// User-facing label string for the Options menu.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Fit4x3Pillarbox => "4:3 Pillarboxed (Authentic)",
            Self::Fill16x9WidescreenCrop => "16:9 Widescreen Crop (SH2:EE)",
        }
    }

    /// Toggles between `Fit4x3Pillarbox` and `Fill16x9WidescreenCrop`.
    pub const fn toggle(self) -> Self {
        match self {
            Self::Fit4x3Pillarbox => Self::Fill16x9WidescreenCrop,
            Self::Fill16x9WidescreenCrop => Self::Fit4x3Pillarbox,
        }
    }
}

/// Normalizes a cutaway picture identifier (e.g. `"out_p_incar"`, `"data/pic/hsp/p_panel.tex"`,
/// `"apt/clock_open"`) into lowercase `"<dir>/<stem>"` form for `IsScaled` lookup.
fn normalize_pic_key(pic_name: &str) -> String {
    let trimmed = pic_name.trim().to_ascii_lowercase().replace('\\', "/");
    let stripped = trimmed
        .strip_prefix("data/")
        .or_else(|| trimmed.strip_prefix("sh2e/"))
        .unwrap_or(&trimmed);
    let stripped = stripped.strip_prefix("pic/").unwrap_or(stripped);
    let stripped = stripped
        .strip_suffix(".tex")
        .or_else(|| stripped.strip_suffix(".tbn2"))
        .or_else(|| stripped.strip_suffix(".png"))
        .unwrap_or(stripped);

    if stripped.contains('/') {
        stripped.to_string()
    } else if let Some((dir, rest)) = stripped.split_once('_') {
        format!("{dir}/{rest}")
    } else {
        stripped.to_string()
    }
}

/// Returns `true` if the cutaway still or overlay sheet is marked `IsScaled = 1` in SH2:EE
/// `Patches/FullscreenImages.h:13..393` (`DefaultTextureList`), meaning it may zoom/crop to `16:9`
/// in `Fill16x9WidescreenCrop` mode.
///
/// Maps, map markings, inventory examine stills (`item/*`, `p_laura_letter`, `p_letterm*`,
/// `p_lostmemory`, `p_redreling`, `x*houban`), `hsp/p_panel`, `htl/p_hanging2`, `gameover1`,
/// `start00*`, and `savebg*` return `false` (`IsScaled = 0`) so edge-critical UI/puzzle elements
/// are never cropped.
pub fn is_cutaway_widescreen_scalable(pic_name: &str) -> bool {
    let key = normalize_pic_key(pic_name);
    if key.is_empty() {
        return false;
    }

    // Maps & map markings (`IsMap = 1, IsScaled = 0`)
    if key.starts_with("map/") || key == "add/mapboldwin" || key == "add/mapmark" {
        return false;
    }

    // Inventory examine stills & letters (`IsScaled = 0`)
    if key.starts_with("item/")
        || key.starts_with("etc/")
        || key.starts_with("menu/")
        || key == "add/xblackhouban"
        || key == "add/xredhouban"
        || key == "add/xwhitehouban"
        || key == "htl/p_laura_letter"
        || key == "htl/p_redreling"
        || key == "out/p_lostmemory"
    {
        return false;
    }

    // Edge-critical puzzle & menu stills explicitly marked `IsScaled = 0` in `FullscreenImages.h`:
    // - `data/pic/add/itemmenux.tex` (line 54: `512, 512, 0, 0, 0`)
    // - `data/pic/hsp/p_panel.tex` (line 139: `512, 512, 1, 0, 0`)
    // - `data/pic/htl/p_hanging2.tex` (line 203: `512, 512, 0, 0, 0`)
    if key == "add/itemmenux" || key == "hsp/p_panel" || key == "htl/p_hanging2" {
        return false;
    }

    // All other `out/`, `apt/`, `hsp/`, `dls/`, `htl/`, `add/`, `ufo/` cutaways & overlays have `IsScaled = 1`.
    key.starts_with("out/")
        || key.starts_with("apt/")
        || key.starts_with("hsp/")
        || key.starts_with("dls/")
        || key.starts_with("htl/")
        || key.starts_with("add/")
        || key.starts_with("ufo/")
}

/// Computed draw rectangle, active visible canvas bounds, and black pillarbox/letterbox mask
/// rectangles for rendering a cutaway background still and its pixel-locked `-256..256` overlay
/// sprite quads (`FullscreenImages.cpp`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FullscreenImageLayout {
    /// Background still draw rectangle left X in viewport pixels (may be `0.0` or positive).
    pub draw_x: f32,
    /// Background still draw rectangle top Y in viewport pixels (negative when zoomed/cropped in `16:9`).
    pub draw_y: f32,
    /// Background still draw width in viewport pixels.
    pub draw_w: f32,
    /// Background still draw height in viewport pixels (exceeds `viewport_h` when zoomed/cropped in `16:9`).
    pub draw_h: f32,
    /// Visible active canvas bounds (clamped to `[0, viewport_w]` x `[0, viewport_h]`).
    pub canvas: PillarboxViewportLayout,
    /// Black pillarbox/letterbox mask bars outside the active canvas (`RGBA(0, 0, 0, 255)`).
    pub black_bars: [PillarboxMaskRect; 2],
    /// `true` if `Fill16x9WidescreenCrop` zoomed the still beyond `4:3` (`IsScaled == true`).
    pub is_widescreen_cropped: bool,
}

impl FullscreenImageLayout {
    /// Computes the `FullscreenImages.cpp` layout for `pic_name` inside `(viewport_w, viewport_h)`.
    pub fn compute(
        viewport_w: f32,
        viewport_h: f32,
        pic_name: &str,
        mode: CutawayAspectMode,
    ) -> Self {
        let vw = viewport_w.max(1.0);
        let vh = viewport_h.max(1.0);
        let vp_aspect = vw / vh;

        let scalable = mode == CutawayAspectMode::Fill16x9WidescreenCrop
            && is_cutaway_widescreen_scalable(pic_name);

        if scalable && vp_aspect > AUTHORED_4X3_ASPECT + 1e-4 {
            // `FullscreenImages.cpp:560..577`:
            //   alg1 = 4/3, alg2 = 16/9
            //   alg4 = 1.25 + (Ratio - 1.25) * (alg1 - 1.25) / (alg2 - 1.25)
            //   SizeFullscreen = min(alg2 * 0.75, alg4)
            // On 16:9..32:9, `SizeFullscreen = 4/3` (fills 16:9 width, crops 12.5% top/bottom).
            let alg1 = AUTHORED_4X3_ASPECT;
            let alg2 = FULLSCREEN_MAX_WIDESCREEN_ASPECT;
            let alg4 = 1.25 + (vp_aspect - 1.25) * (alg1 - 1.25) / (alg2 - 1.25);
            let size_fullscreen = (alg2 * 0.75).min(alg4).max(1.0);

            let draw_h = vh * size_fullscreen;
            let draw_w = draw_h * AUTHORED_4X3_ASPECT;
            let draw_x = (vw - draw_w) * 0.5;
            let draw_y = (vh - draw_h) * 0.5;

            let canvas_aspect = (draw_w / vh).min(vp_aspect);
            let canvas = PillarboxViewportLayout::for_aspect(vw, vh, canvas_aspect);

            Self {
                draw_x,
                draw_y,
                draw_w,
                draw_h,
                black_bars: canvas.black_bars,
                canvas,
                is_widescreen_cropped: true,
            }
        } else {
            let canvas = PillarboxViewportLayout::for_aspect(vw, vh, AUTHORED_4X3_ASPECT);
            Self {
                draw_x: canvas.canvas_left,
                draw_y: canvas.canvas_top,
                draw_w: canvas.canvas_width(),
                draw_h: canvas.canvas_height(),
                black_bars: canvas.black_bars,
                canvas,
                is_widescreen_cropped: false,
            }
        }
    }

    /// Transforms an [`OverlaySpriteQuad`] (`-256..256` PC sprite space) using the exact same
    /// `(draw_x, draw_y, draw_w, draw_h)` transform as the background cutaway still, and clips
    /// protruding edges against the visible active canvas bounds so overlay sprites remain 100%
    /// pixel-locked across `4:3`, `16:9`, `20:9`, `21:9`, and `32:9`.
    pub fn map_and_clip_overlay_quad(
        &self,
        quad: &OverlaySpriteQuad,
    ) -> Option<[(f32, f32, f32, f32); 6]> {
        let tris = quad.to_viewport_triangles(self.draw_x, self.draw_y, self.draw_w, self.draw_h);
        let all_left = tris.iter().all(|v| v.0 <= self.canvas.canvas_left);
        let all_right = tris.iter().all(|v| v.0 >= self.canvas.canvas_right);
        let all_top = tris.iter().all(|v| v.1 <= self.canvas.canvas_top);
        let all_bottom = tris.iter().all(|v| v.1 >= self.canvas.canvas_bottom);
        if all_left || all_right || all_top || all_bottom {
            return None;
        }

        let p0 = tris[0];
        let p1 = tris[1];
        let p2 = tris[2];
        let p3 = tris[5];
        let is_axis_aligned = (p0.0 - p3.0).abs() < 1e-3
            && (p1.0 - p2.0).abs() < 1e-3
            && (p0.1 - p1.1).abs() < 1e-3
            && (p2.1 - p3.1).abs() < 1e-3;

        if is_axis_aligned {
            let (nx0, ny0, nx1, ny1, nu0, nv0, nu1, nv1) = self
                .canvas
                .clip_axis_aligned_quad(p0.0, p0.1, p2.0, p2.1, p0.2, p0.3, p2.2, p2.3)?;
            let c0 = (nx0, ny0, nu0, nv0);
            let c1 = (nx1, ny0, nu1, nv0);
            let c2 = (nx1, ny1, nu1, nv1);
            let c3 = (nx0, ny1, nu0, nv1);
            return Some([c0, c1, c2, c0, c2, c3]);
        }

        let mut clamped = tris;
        for v in &mut clamped {
            v.0 = v.0.clamp(self.canvas.canvas_left, self.canvas.canvas_right);
            v.1 = v.1.clamp(self.canvas.canvas_top, self.canvas.canvas_bottom);
        }
        Some(clamped)
    }
}
