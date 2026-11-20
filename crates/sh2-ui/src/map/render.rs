//! 2D renderer for the 1:1 PS2 / SH2:EE Fullscreen Paper Map (`Event/chizu.c`).

use crate::atlas_font::{AtlasFont, Sh2TextColor};
use crate::font::TextRasterizer;
use crate::map::markers::{active_markers_for_map, ChizuDecodedMarker};
use crate::map::tables::map_spec;
use crate::map::viewer::{MapZoomStep, PaperMapViewer, TRANSCRIBE_FADE_TARGET_ALPHA};
use crate::map::{MapCanvasClipper, MapPlayerMarker};
use sh2_core::events::EventFlags;

/// Renders the authentic `4:3` pillarboxed Fullscreen Paper Map with:
/// - Loaded retail `data/pic/map/*.tex` RGBA sheet (or aged paper fallback if missing),
/// - Real `SLUS_202.28` `ChizuMarkerDraw` (`0x001c7e90`) red marker pen atlas decals (`*mark.tex`),
/// - Clipped green player heading triangle (`MapCanvasClipper::clip_player_marker`),
/// - Pure black widescreen/ultrawide pillarboxes (`SetBlackPillarBoxes`).
#[allow(clippy::too_many_arguments)]
pub fn draw_paper_map(
    viewer: &PaperMapViewer,
    flags: &EventFlags,
    player_norm_pos: (f32, f32),
    player_yaw_rad: f32,
    sheet_rgba: Option<(&[u8], usize, usize)>,
    mark_atlas_rgba: Option<(&[u8], usize, usize)>,
    font: &AtlasFont,
    rasterizer: &mut TextRasterizer,
) {
    if !viewer.is_open {
        return;
    }
    let vw = rasterizer.width as f32;
    let vh = rasterizer.height as f32;
    let clipper = MapCanvasClipper::new_4x3(vw, vh);

    // 1. Clear full screen to black
    rasterizer.fill_rect(0, 0, rasterizer.width, rasterizer.height, [0, 0, 0, 255]);

    // 2. Draw paper map sheet inside active 4:3 canvas (with zoom & pan)
    let (x0, y0) = viewer.project_to_viewport(0.0, 0.0, &clipper);
    let (x1, y1) = viewer.project_to_viewport(1.0, 1.0, &clipper);
    if let Some((cx0, cy0, cx1, cy1, cu0, cv0, cu1, cv1)) =
        clipper.clip_decal_quad(x0, y0, x1, y1, 0.0, 0.0, 1.0, 1.0)
    {
        let dx = cx0.floor().max(0.0) as usize;
        let dy = cy0.floor().max(0.0) as usize;
        let dw = (cx1 - cx0).ceil().max(1.0) as usize;
        let dh = (cy1 - cy0).ceil().max(1.0) as usize;
        if let Some((rgba, sw, sh)) = sheet_rgba {
            let tris = [
                (cx0, cy0, cu0, cv0),
                (cx1, cy0, cu1, cv0),
                (cx1, cy1, cu1, cv1),
                (cx0, cy0, cu0, cv0),
                (cx1, cy1, cu1, cv1),
                (cx0, cy1, cu0, cv1),
            ];
            rasterizer.blit_overlay_triangles(&tris, rgba, sw, sh, 1.0);
        } else {
            rasterizer.fill_rect(dx, dy, dw, dh, [186, 172, 142, 255]);
            rasterizer.draw_rect_outline(
                dx + 8,
                dy + 8,
                dw.saturating_sub(16),
                dh.saturating_sub(16),
                2,
                [92, 76, 56, 220],
            );
        }
    }

    // 3. Draw real `SLUS_202.28` `ChizuMarkerDraw` red marker pen decals
    let markers = active_markers_for_map(viewer.current_map_id, flags);
    for mark in &markers {
        draw_atlas_marker(viewer, mark, &clipper, mark_atlas_rgba, rasterizer);
    }

    // 4. Draw clipped green player heading triangle (`MapCanvasClipper`)
    let (px, py) = viewer.project_to_viewport(player_norm_pos.0, player_norm_pos.1, &clipper);
    let marker = MapPlayerMarker {
        x: px,
        y: py,
        yaw_radians: player_yaw_rad,
        radius: 9.0 * (1.0 + viewer.zoom_t * 0.35),
    };
    if let Some(verts) = clipper.clip_player_marker(&marker) {
        rasterizer.fill_triangle(verts, [32, 224, 72, 245]);
    }

    // 5. Enforce pure black widescreen/ultrawide pillarboxes (`SetBlackPillarBoxes`)
    clipper.draw_black_pillarbox_bars(rasterizer);

    // 6. Subtle header & controls bar inside canvas bounds
    let left_px = clipper.layout.canvas_left.round().max(0.0) as usize;
    let cw_px = (clipper.layout.canvas_right - clipper.layout.canvas_left)
        .round()
        .max(1.0) as usize;
    let title = map_spec(viewer.current_map_id)
        .map(|s| s.title)
        .unwrap_or("Silent Hill Map");
    let zoom_label = if viewer.zoom_step == MapZoomStep::FullSheet {
        "[Confirm] Zoom In   [Arrows] Floors/Areas   [Cancel/M] Close"
    } else {
        "[Confirm] Zoom Out   [Arrows] Pan   [Cancel/M] Close"
    };
    rasterizer.fill_rect(left_px, 0, cw_px, 34, [0, 0, 0, 175]);
    font.draw_line_centered(
        rasterizer,
        title,
        (left_px + cw_px / 2, 8),
        16,
        Sh2TextColor::GoldSelected,
        true,
    );
    let bottom_y = rasterizer.height.saturating_sub(28);
    rasterizer.fill_rect(left_px, bottom_y, cw_px, 28, [0, 0, 0, 175]);
    font.draw_line_centered(
        rasterizer,
        zoom_label,
        (left_px + cw_px / 2, bottom_y + 6),
        13,
        Sh2TextColor::White,
        true,
    );
}

fn draw_atlas_marker(
    viewer: &PaperMapViewer,
    mark: &ChizuDecodedMarker,
    clipper: &MapCanvasClipper,
    mark_atlas_rgba: Option<(&[u8], usize, usize)>,
    rasterizer: &mut TextRasterizer,
) {
    let (mx, my) = viewer.project_to_viewport(mark.norm_x, mark.norm_y, clipper);
    let zoom = 1.0 + viewer.zoom_t;
    let sx = (clipper.layout.canvas_right - clipper.layout.canvas_left) / 640.0;
    let sy = (clipper.layout.canvas_bottom - clipper.layout.canvas_top) / 480.0;
    let hw = (mark.width_px * 0.5) * sx * zoom;
    let hh = (mark.height_px * 0.5) * sy * zoom;
    let alpha_mul = if viewer.transcribing_flag_id == Some(mark.flag_id) {
        (viewer.transcription_alpha as f32) / (TRANSCRIBE_FADE_TARGET_ALPHA as f32)
    } else {
        1.0
    };

    if let Some((atlas, aw, ah)) = mark_atlas_rgba {
        let u0 = (mark.uv_px[0] as f32) / (aw.max(1) as f32);
        let v0 = (mark.uv_px[1] as f32) / (ah.max(1) as f32);
        let u1 = (mark.uv_px[2] as f32) / (aw.max(1) as f32);
        let v1 = (mark.uv_px[3] as f32) / (ah.max(1) as f32);
        if let Some((cx0, cy0, cx1, cy1, cu0, cv0, cu1, cv1)) =
            clipper.clip_decal_quad(mx - hw, my - hh, mx + hw, my + hh, u0, v0, u1, v1)
        {
            let tris = [
                (cx0, cy0, cu0, cv0),
                (cx1, cy0, cu1, cv0),
                (cx1, cy1, cu1, cv1),
                (cx0, cy0, cu0, cv0),
                (cx1, cy1, cu1, cv1),
                (cx0, cy1, cu0, cv1),
            ];
            rasterizer.blit_overlay_triangles(&tris, atlas, aw, ah, alpha_mul);
        }
    } else if let Some((cx0, cy0, cx1, cy1, _, _, _, _)) =
        clipper.clip_decal_quad(mx - hw, my - hh, mx + hw, my + hh, 0.0, 0.0, 1.0, 1.0)
    {
        let alpha = (alpha_mul * 210.0).round() as u8;
        rasterizer.draw_rect_outline(
            cx0.max(0.0) as usize,
            cy0.max(0.0) as usize,
            (cx1 - cx0).max(4.0) as usize,
            (cy1 - cy0).max(4.0) as usize,
            2,
            [195, 24, 24, alpha],
        );
    }
}
