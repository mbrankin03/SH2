//! Widescreen & ultrawide (`16:9` / `21:9` / `32:9`) 2D canvas pillarbox/letterbox layout,
//! quad + UV boundary clipping (`SetBlackPillarBoxes` / `FullScreenArtifact`), map marker clipping,
//! and `DisableCutsceneBorders = Auto` letterbox calculation (`SH2-77`).
//!
//! # Ground Truth & SH2:EE Reference
//! - **`SetBlackPillarBoxes` (`Wrappers/d3d8/IDirect3DDevice8.cpp:2240..2835`)**:
//!   - Computes the active `4:3` or `16:9` canvas bounds inside an arbitrary viewport (`16:9`, `16:10`,
//!     `20:9`, `21:9`, `32:9`).
//!   - Clips 2D sprite quads and map annotations whose vertices extend into the left/right pillarbox
//!     bars (`FullScreenArtifact` at lines `2330..2359`), interpolating texture coordinates `(u, v)`
//!     proportionally so partially visible sprites never stretch or distort.
//!   - Clips the green player position triangle marker on the map when panned into the pillarbox
//!     area (`lines 2772..2825`).
//!   - Emits pure black pillarbox/letterbox mask rectangles (`RGBA(0, 0, 0, 255)`) to prevent 3D
//!     background fog, level geometry, or film grain from bleeding outside the active 2D canvas.
//! - **`DisableCutsceneBorders` (`WidescreenFixesPack/WidescreenFixesPack.cpp:239, 444..457`)**:
//!   - In `Auto` (`FILL_MEDIA`) mode, collapses the top/bottom `4:3` cutscene letterbox bars
//!     (`f0 = 0.395 -> 0.5, f1 = 0.605 -> 0.5`) whenever `viewport_aspect > 4.0 / 3.0` so widescreen
//!     and ultrawide displays preserve full vertical height.

use sh2_assets::formats::demo::{CutsceneBorderMode, CutsceneWidescreenMode, AUTHORED_4X3_ASPECT};

use super::OverlaySpriteQuad;

/// Axis-aligned rectangle in viewport pixel coordinates (`[x0, y0, x1, y1]`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PillarboxMaskRect {
    /// Left X pixel coordinate.
    pub x0: f32,
    /// Top Y pixel coordinate.
    pub y0: f32,
    /// Right X pixel coordinate.
    pub x1: f32,
    /// Bottom Y pixel coordinate.
    pub y1: f32,
}

impl PillarboxMaskRect {
    /// Width of the mask rectangle in pixels (`(x1 - x0).max(0.0)`).
    #[inline]
    pub fn width(&self) -> f32 {
        (self.x1 - self.x0).max(0.0)
    }

    /// Height of the mask rectangle in pixels (`(y1 - y0).max(0.0)`).
    #[inline]
    pub fn height(&self) -> f32 {
        (self.y1 - self.y0).max(0.0)
    }

    /// Returns `true` if this mask rectangle has positive area (`width > 0.5 && height > 0.5`).
    #[inline]
    pub fn is_non_empty(&self) -> bool {
        self.width() > 0.5 && self.height() > 0.5
    }
}

/// Computed active canvas bounds and black pillarbox/letterbox mask bars for displaying content of
/// `content_aspect` (e.g. `4:3` or `16:9`) inside a viewport of `(viewport_w, viewport_h)` (`SetBlackPillarBoxes`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PillarboxViewportLayout {
    /// Full viewport width in pixels.
    pub viewport_w: f32,
    /// Full viewport height in pixels.
    pub viewport_h: f32,
    /// Active content canvas left X coordinate in pixels (`pillar_left`).
    pub canvas_left: f32,
    /// Active content canvas top Y coordinate in pixels (`letter_top`).
    pub canvas_top: f32,
    /// Active content canvas right X coordinate in pixels (`pillar_right`).
    pub canvas_right: f32,
    /// Active content canvas bottom Y coordinate in pixels (`letter_bottom`).
    pub canvas_bottom: f32,
    /// Left/Right (or Top/Bottom) black mask rectangles (`RGBA(0, 0, 0, 255)`).
    pub black_bars: [PillarboxMaskRect; 2],
}

/// Clipped axis-aligned textured quad `(x0, y0, x1, y1, u0, v0, u1, v1)`.
pub type ClippedQuadUv = (f32, f32, f32, f32, f32, f32, f32, f32);

impl PillarboxViewportLayout {
    /// Computes the centered active canvas and black pillarbox/letterbox mask bars for `content_aspect`
    /// (`4.0 / 3.0` for original 2D maps/stills, `16.0 / 9.0` for SH2:EE HD stills) inside `(viewport_w, viewport_h)`.
    pub fn for_aspect(viewport_w: f32, viewport_h: f32, content_aspect: f32) -> Self {
        let vw = viewport_w.max(1.0);
        let vh = viewport_h.max(1.0);
        let target_aspect = content_aspect.max(0.1);
        let vp_aspect = vw / vh;

        if vp_aspect > target_aspect + 1e-4 {
            // Viewport is wider than content (e.g. 4:3 or 16:9 still on 16:9 / 21:9 / 32:9 monitor) -> left/right pillarboxes
            let canvas_w = (vh * target_aspect).min(vw);
            let pad_x = (vw - canvas_w) * 0.5;
            let left = pad_x;
            let right = vw - pad_x;
            Self {
                viewport_w: vw,
                viewport_h: vh,
                canvas_left: left,
                canvas_top: 0.0,
                canvas_right: right,
                canvas_bottom: vh,
                black_bars: [
                    PillarboxMaskRect {
                        x0: 0.0,
                        y0: 0.0,
                        x1: left,
                        y1: vh,
                    },
                    PillarboxMaskRect {
                        x0: right,
                        y0: 0.0,
                        x1: vw,
                        y1: vh,
                    },
                ],
            }
        } else if vp_aspect + 1e-4 < target_aspect {
            // Viewport is taller than content (e.g. 16:9 still on 4:3 / 16:10 monitor) -> top/bottom letterboxes
            let canvas_h = (vw / target_aspect).min(vh);
            let pad_y = (vh - canvas_h) * 0.5;
            let top = pad_y;
            let bottom = vh - pad_y;
            Self {
                viewport_w: vw,
                viewport_h: vh,
                canvas_left: 0.0,
                canvas_top: top,
                canvas_right: vw,
                canvas_bottom: bottom,
                black_bars: [
                    PillarboxMaskRect {
                        x0: 0.0,
                        y0: 0.0,
                        x1: vw,
                        y1: top,
                    },
                    PillarboxMaskRect {
                        x0: 0.0,
                        y0: bottom,
                        x1: vw,
                        y1: vh,
                    },
                ],
            }
        } else {
            Self {
                viewport_w: vw,
                viewport_h: vh,
                canvas_left: 0.0,
                canvas_top: 0.0,
                canvas_right: vw,
                canvas_bottom: vh,
                black_bars: [
                    PillarboxMaskRect {
                        x0: 0.0,
                        y0: 0.0,
                        x1: 0.0,
                        y1: vh,
                    },
                    PillarboxMaskRect {
                        x0: vw,
                        y0: 0.0,
                        x1: vw,
                        y1: vh,
                    },
                ],
            }
        }
    }

    /// Width of the active canvas in pixels (`canvas_right - canvas_left`).
    #[inline]
    pub fn canvas_width(&self) -> f32 {
        (self.canvas_right - self.canvas_left).max(1.0)
    }

    /// Height of the active canvas in pixels (`canvas_bottom - canvas_top`).
    #[inline]
    pub fn canvas_height(&self) -> f32 {
        (self.canvas_bottom - self.canvas_top).max(1.0)
    }

    /// Returns `true` if a pixel coordinate `(x, y)` lies strictly inside the active 2D canvas
    /// (used to suppress background 3D fog/noise bleed in pillarbox regions on 21:9/32:9 displays).
    #[inline]
    pub fn contains_pixel(&self, x: f32, y: f32) -> bool {
        x >= self.canvas_left
            && x <= self.canvas_right
            && y >= self.canvas_top
            && y <= self.canvas_bottom
    }

    /// Clips an axis-aligned textured quad `(x0, y0, x1, y1, u0, v0, u1, v1)` against the active
    /// canvas bounds (`FullScreenArtifact` in `IDirect3DDevice8.cpp:2330..2359`), proportionally
    /// interpolating `(u0, v0, u1, v1)` when a quad crosses `canvas_left` or `canvas_right`.
    ///
    /// Returns `None` if the quad lies completely inside the pillarbox/letterbox region.
    #[allow(clippy::too_many_arguments)]
    pub fn clip_axis_aligned_quad(
        &self,
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        u0: f32,
        v0: f32,
        u1: f32,
        v1: f32,
    ) -> Option<ClippedQuadUv> {
        let (min_x, max_x) = if x0 <= x1 { (x0, x1) } else { (x1, x0) };
        let (min_y, max_y) = if y0 <= y1 { (y0, y1) } else { (y1, y0) };

        if max_x <= self.canvas_left
            || min_x >= self.canvas_right
            || max_y <= self.canvas_top
            || min_y >= self.canvas_bottom
        {
            return None;
        }

        let dx = x1 - x0;
        let dy = y1 - y0;
        let nx0 = x0.clamp(self.canvas_left, self.canvas_right);
        let nx1 = x1.clamp(self.canvas_left, self.canvas_right);
        let ny0 = y0.clamp(self.canvas_top, self.canvas_bottom);
        let ny1 = y1.clamp(self.canvas_top, self.canvas_bottom);

        let nu0 = if dx.abs() > 1e-5 {
            u0 + (u1 - u0) * ((nx0 - x0) / dx)
        } else {
            u0
        };
        let nu1 = if dx.abs() > 1e-5 {
            u0 + (u1 - u0) * ((nx1 - x0) / dx)
        } else {
            u1
        };
        let nv0 = if dy.abs() > 1e-5 {
            v0 + (v1 - v0) * ((ny0 - y0) / dy)
        } else {
            v0
        };
        let nv1 = if dy.abs() > 1e-5 {
            v0 + (v1 - v0) * ((ny1 - y0) / dy)
        } else {
            v1
        };

        Some((nx0, ny0, nx1, ny1, nu0, nv0, nu1, nv1))
    }

    /// Maps an [`OverlaySpriteQuad`] (`-256..256` PC sprite space) into the active 2D canvas and clips
    /// any protruding edges against the pillarbox/letterbox boundaries (`SetBlackPillarBoxes`).
    pub fn map_and_clip_overlay_quad(
        &self,
        quad: &OverlaySpriteQuad,
    ) -> Option<[(f32, f32, f32, f32); 6]> {
        let tris = quad.to_viewport_triangles(
            self.canvas_left,
            self.canvas_top,
            self.canvas_width(),
            self.canvas_height(),
        );
        // Check if all 6 vertices lie completely outside any single canvas edge
        let all_left = tris.iter().all(|v| v.0 <= self.canvas_left);
        let all_right = tris.iter().all(|v| v.0 >= self.canvas_right);
        let all_top = tris.iter().all(|v| v.1 <= self.canvas_top);
        let all_bottom = tris.iter().all(|v| v.1 >= self.canvas_bottom);
        if all_left || all_right || all_top || all_bottom {
            return None;
        }

        // For axis-aligned rectangles (`p0.x == p3.x && p1.x == p2.x`), apply exact UV-interpolated clipping
        let p0 = tris[0];
        let p1 = tris[1];
        let p2 = tris[2];
        let p3 = tris[5];
        let is_axis_aligned = (p0.0 - p3.0).abs() < 1e-3
            && (p1.0 - p2.0).abs() < 1e-3
            && (p0.1 - p1.1).abs() < 1e-3
            && (p2.1 - p3.1).abs() < 1e-3;

        if is_axis_aligned {
            let (nx0, ny0, nx1, ny1, nu0, nv0, nu1, nv1) =
                self.clip_axis_aligned_quad(p0.0, p0.1, p2.0, p2.1, p0.2, p0.3, p2.2, p2.3)?;
            let c0 = (nx0, ny0, nu0, nv0);
            let c1 = (nx1, ny0, nu1, nv0);
            let c2 = (nx1, ny1, nu1, nv1);
            let c3 = (nx0, ny1, nu0, nv1);
            return Some([c0, c1, c2, c0, c2, c3]);
        }

        // For rotated quads (e.g. clock needles, safe dial), clamp vertices to canvas bounds
        let mut clamped = tris;
        for v in &mut clamped {
            v.0 = v.0.clamp(self.canvas_left, self.canvas_right);
            v.1 = v.1.clamp(self.canvas_top, self.canvas_bottom);
        }
        Some(clamped)
    }

    /// Clips the green player map marker triangle (`[(x, y); 3]` in viewport pixels) against the
    /// active map canvas (`IDirect3DDevice8.cpp:2772..2825`).
    ///
    /// Returns `None` if the player marker has scrolled into the left/right pillarbox or top/bottom
    /// letterbox area, or returns the clamped triangle vertices within `[canvas_left, canvas_right]`.
    pub fn clip_map_marker_triangle(&self, tri: [(f32, f32); 3]) -> Option<[(f32, f32); 3]> {
        let cx = (tri[0].0 + tri[1].0 + tri[2].0) / 3.0;
        let cy = (tri[0].1 + tri[1].1 + tri[2].1) / 3.0;
        if !self.contains_pixel(cx, cy) {
            return None;
        }
        Some([
            (
                tri[0].0.clamp(self.canvas_left, self.canvas_right),
                tri[0].1.clamp(self.canvas_top, self.canvas_bottom),
            ),
            (
                tri[1].0.clamp(self.canvas_left, self.canvas_right),
                tri[1].1.clamp(self.canvas_top, self.canvas_bottom),
            ),
            (
                tri[2].0.clamp(self.canvas_left, self.canvas_right),
                tri[2].1.clamp(self.canvas_top, self.canvas_bottom),
            ),
        ])
    }
}

/// Evaluated cutscene border bars (`DisableCutsceneBorders` in `WidescreenFixesPack.cpp:444..457`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CutsceneBorderBars {
    /// Normalized vertical letterbox coordinates `(f0, f1)` (`(0.5, 0.5)` when letterbox bars are hidden,
    /// `(0.395, 0.605)` when 4:3 letterbox bars are enabled).
    pub letterbox_f0_f1: (f32, f32),
    /// Top and bottom letterbox rectangles in viewport pixels (empty when hidden in `Auto` widescreen mode).
    pub letterbox_bars: [PillarboxMaskRect; 2],
    /// Left and right pillarbox rectangles in viewport pixels (active when `widescreen_mode == Pillarbox4x3`
    /// on a viewport wider than `4:3`).
    pub pillarbox_bars: [PillarboxMaskRect; 2],
}

impl CutsceneBorderBars {
    /// Computes the cutscene border bars for `(viewport_w, viewport_h, widescreen_mode, border_mode)`.
    pub fn evaluate(
        viewport_w: f32,
        viewport_h: f32,
        widescreen_mode: CutsceneWidescreenMode,
        border_mode: CutsceneBorderMode,
    ) -> Self {
        let vw = viewport_w.max(1.0);
        let vh = viewport_h.max(1.0);
        let vp_aspect = vw / vh;

        let hide_letterbox = border_mode.should_hide_letterbox_bars(vp_aspect, widescreen_mode);
        let (f0, f1, letterbox_bars) = if hide_letterbox {
            (
                0.5,
                0.5,
                [
                    PillarboxMaskRect {
                        x0: 0.0,
                        y0: 0.0,
                        x1: vw,
                        y1: 0.0,
                    },
                    PillarboxMaskRect {
                        x0: 0.0,
                        y0: vh,
                        x1: vw,
                        y1: vh,
                    },
                ],
            )
        } else {
            // Original 4:3 letterbox bar height (~10.5% top and bottom, corresponding to f0=0.395, f1=0.605)
            let bar_h = (vh * 0.105).round();
            (
                0.395,
                0.605,
                [
                    PillarboxMaskRect {
                        x0: 0.0,
                        y0: 0.0,
                        x1: vw,
                        y1: bar_h,
                    },
                    PillarboxMaskRect {
                        x0: 0.0,
                        y0: vh - bar_h,
                        x1: vw,
                        y1: vh,
                    },
                ],
            )
        };

        let pillarbox_bars = if widescreen_mode == CutsceneWidescreenMode::Pillarbox4x3
            && vp_aspect > AUTHORED_4X3_ASPECT + 1e-4
        {
            PillarboxViewportLayout::for_aspect(vw, vh, AUTHORED_4X3_ASPECT).black_bars
        } else {
            [
                PillarboxMaskRect {
                    x0: 0.0,
                    y0: 0.0,
                    x1: 0.0,
                    y1: vh,
                },
                PillarboxMaskRect {
                    x0: vw,
                    y0: 0.0,
                    x1: vw,
                    y1: vh,
                },
            ]
        };

        Self {
            letterbox_f0_f1: (f0, f1),
            letterbox_bars,
            pillarbox_bars,
        }
    }
}
