//! Dynamic map companion system supporting James's maps (1-35), Maria's Baldwin Mansion map (36),
//! and SH2:EE `SetBlackPillarBoxes` (`Fix2D`) widescreen/ultrawide (`16:9` / `21:9` / `32:9`)
//! map decal & green player marker boundary clipping (`SH2-77`).
//!
//! # Map Architecture
//! - Top screen: Primary 3D horror gameplay.
//! - Bottom / Fullscreen Map: Real-time map displaying current floor, room bounds, green player
//!   position triangle marker, and red circle/cross annotation decals extracted from `data/pic/map/*.tex`.
//! - On widescreen (`16:9`) and ultrawide (`21:9`, `32:9`) monitors, `MapCanvasClipper` enforces
//!   `SetBlackPillarBoxes` (`Wrappers/d3d8/IDirect3DDevice8.cpp:2330..2835`), clipping map markings
//!   and the green player marker at the active `4:3` or `16:9` map canvas borders and masking the
//!   side margins with pure black pillarboxes (`RGBA(0, 0, 0, 255)`).

use sh2_core::{ClippedQuadUv, PillarboxMaskRect, PillarboxViewportLayout, AUTHORED_4X3_ASPECT};

use crate::font::TextRasterizer;

/// Identifying descriptor for active level map and floor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MapDescriptor {
    /// Numerical map identifier (1 = Town East, 2 = Apartments, etc., 36 = Baldwin Mansion).
    pub map_id: u8,
    /// Current building floor (-1 = Basement, 1 = 1F, 2 = 2F, 3 = 3F, 4 = Roof).
    pub floor: i8,
}

impl Default for MapDescriptor {
    fn default() -> Self {
        Self {
            map_id: 1, // Town East
            floor: 1,
        }
    }
}

/// Green triangular player marker on the 2D map (`IDirect3DDevice8.cpp:2772..2825`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MapPlayerMarker {
    /// Center X position in viewport pixels.
    pub x: f32,
    /// Center Y position in viewport pixels.
    pub y: f32,
    /// Player heading angle in radians.
    pub yaw_radians: f32,
    /// Marker radius in pixels (default `8.0`).
    pub radius: f32,
}

impl MapPlayerMarker {
    /// Computes the 3 triangle vertices `[(x0, y0), (x1, y1), (x2, y2)]` of the green player indicator.
    pub fn triangle_vertices(&self) -> [(f32, f32); 3] {
        let r = self.radius.max(2.0);
        let a0 = self.yaw_radians;
        let a1 = self.yaw_radians + 2.4;
        let a2 = self.yaw_radians - 2.4;
        [
            (self.x + a0.sin() * r, self.y - a0.cos() * r),
            (
                self.x + a1.sin() * (r * 0.75),
                self.y - a1.cos() * (r * 0.75),
            ),
            (
                self.x + a2.sin() * (r * 0.75),
                self.y - a2.cos() * (r * 0.75),
            ),
        ]
    }
}

/// Widescreen & ultrawide (`16:9` / `21:9` / `32:9`) map canvas clipper and black pillarbox
/// masker (`SetBlackPillarBoxes` in `IDirect3DDevice8.cpp`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MapCanvasClipper {
    /// Underlying pillarbox/letterbox viewport layout.
    pub layout: PillarboxViewportLayout,
}

impl MapCanvasClipper {
    /// Creates a `MapCanvasClipper` for an authored `4:3` map sheet inside `(viewport_w, viewport_h)`.
    #[inline]
    pub fn new_4x3(viewport_w: f32, viewport_h: f32) -> Self {
        Self {
            layout: PillarboxViewportLayout::for_aspect(
                viewport_w,
                viewport_h,
                AUTHORED_4X3_ASPECT,
            ),
        }
    }

    /// Creates a `MapCanvasClipper` for a map sheet of `content_aspect` (e.g. `4.0 / 3.0` or `16.0 / 9.0`).
    #[inline]
    pub fn new_with_aspect(viewport_w: f32, viewport_h: f32, content_aspect: f32) -> Self {
        Self {
            layout: PillarboxViewportLayout::for_aspect(viewport_w, viewport_h, content_aspect),
        }
    }

    /// Clips a map annotation decal quad `(x0, y0, x1, y1, u0, v0, u1, v1)` against the active map canvas,
    /// interpolating `(u0, v0, u1, v1)` when the decal crosses the left or right pillarbox boundary.
    #[allow(clippy::too_many_arguments)]
    #[inline]
    pub fn clip_decal_quad(
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
        self.layout
            .clip_axis_aligned_quad(x0, y0, x1, y1, u0, v0, u1, v1)
    }

    /// Clips the green player map marker against the active map canvas (`IDirect3DDevice8.cpp:2772..2825`).
    #[inline]
    pub fn clip_player_marker(&self, marker: &MapPlayerMarker) -> Option<[(f32, f32); 3]> {
        self.layout
            .clip_map_marker_triangle(marker.triangle_vertices())
    }

    /// Returns the two black pillarbox/letterbox mask bars (`RGBA(0, 0, 0, 255)`).
    #[inline]
    pub fn black_pillarbox_bars(&self) -> [PillarboxMaskRect; 2] {
        self.layout.black_bars
    }

    /// Renders pure black pillarbox/letterbox bars (`SetBlackPillarBoxes`) onto `rasterizer` to mask
    /// any 2D overlay or background bleed outside the active canvas.
    pub fn draw_black_pillarbox_bars(&self, rasterizer: &mut TextRasterizer) {
        for bar in &self.layout.black_bars {
            if bar.is_non_empty() {
                let x0 = (bar.x0.floor().max(0.0)) as usize;
                let y0 = (bar.y0.floor().max(0.0)) as usize;
                let w = (bar.width().ceil()) as usize;
                let h = (bar.height().ceil()) as usize;
                rasterizer.fill_rect(x0, y0, w, h, [0, 0, 0, 255]);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_canvas_clipper_21x9_and_32x9_pillarboxes() {
        let clipper_21_9 = MapCanvasClipper::new_4x3(3440.0, 1440.0);
        // 4:3 map on 3440x1440 has active width 1920 -> [760.0 .. 2680.0]
        assert!((clipper_21_9.layout.canvas_left - 760.0).abs() < 1e-3);
        assert!((clipper_21_9.layout.canvas_right - 2680.0).abs() < 1e-3);

        let marker_inside = MapPlayerMarker {
            x: 1720.0,
            y: 720.0,
            yaw_radians: 0.0,
            radius: 8.0,
        };
        assert!(clipper_21_9.clip_player_marker(&marker_inside).is_some());

        let marker_outside = MapPlayerMarker {
            x: 320.0,
            y: 720.0,
            yaw_radians: 0.0,
            radius: 8.0,
        };
        assert!(clipper_21_9.clip_player_marker(&marker_outside).is_none());

        let mut rast = TextRasterizer::new(344, 144);
        rast.fill_rect(0, 0, 344, 144, [200, 180, 160, 255]);
        let small_clipper = MapCanvasClipper::new_4x3(344.0, 144.0);
        small_clipper.draw_black_pillarbox_bars(&mut rast);
        // Left pillarbox pixel (10, 72) must be masked to pure black (0, 0, 0, 255)
        let left_ofs = (72 * 344 + 10) * 4;
        assert_eq!(&rast.pixels[left_ofs..left_ofs + 4], &[0, 0, 0, 255]);
        // Center canvas pixel (172, 72) must retain the map color
        let mid_ofs = (72 * 344 + 172) * 4;
        assert_eq!(&rast.pixels[mid_ofs..mid_ofs + 4], &[200, 180, 160, 255]);
    }
}
