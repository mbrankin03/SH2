//! 2D renderer for the 1:1 PS2 / SH2:EE Fullscreen Paper Map (`Event/chizu.c`).
//!
//! Renders strictly what Team Silent's `ChizuDisplay` (`0x001c7ba0`) draws — zero synthetic
//! header/footer bars:
//! 1. Base map `.tex` cropped by `ChizuDisplay`'s exact UV window inside the `4:3` canvas,
//! 2. Dual-layer `ChizuMarkerDraw` (`0x001c7e90`) red-pen decals scaled by `1 / (2 - disp_rate)`,
//! 3. `ChizuConnectArrowDraw` (`0x001c88f0`) directional arrows from `pic/etc/carsol.tex`
//!    fading linearly from `0x80` to `0` over `disp_rate = 0.0..0.5`,
//! 4. `ChizuCurrentPositionDraw` (`0x001c9000`) 2-layer rotated player arrow (foreground + shadow)
//!    from `pic/etc/carsol.tex`,
//! 5. Pure black `SetBlackPillarBoxes` bars outside the `4:3` canvas.

use crate::fingerprint::UiFingerprintHasher;
use crate::font::TextRasterizer;
use crate::map::markers::{active_markers_for_map, ChizuDecodedMarker};
use crate::map::position::{chizu_arrow_yaw_offset, chizu_should_draw_player_arrow};
use crate::map::tables::{
    chizu_connect_check, MapDirection, CHIZU_ARROW_DEF_POS, CHIZU_ARROW_TEX_UVS,
    CHIZU_PLAYER_ARROW_UVS,
};
use crate::map::viewer::{PaperMapViewer, TRANSCRIBE_FADE_TARGET_ALPHA};
use crate::map::MapCanvasClipper;
use sh2_core::events::EventFlags;

/// Inputs describing the player's current world room & heading for `ChizuCurrentPositionDraw`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PaperMapPlayerState {
    /// Player's PS2 `RoomName` ID (`0..214`).
    pub room_id: usize,
    /// Player's position in PS2 centered `[-256..+256]` virtual map pixels (`ChizuCurrentPositionCheck`).
    pub pos_ps2: (f32, f32),
    /// Player's facing yaw in radians (`_sh2jms + 0x34` / `0x30`).
    pub yaw_rad: f32,
}

/// Optional decoded retail RGBA textures for the fullscreen paper map.
#[derive(Debug, Clone, Copy, Default)]
pub struct PaperMapTextures<'a> {
    /// Decoded `1024x1024` base map sheet (`pic/map/*.tex` or `pic/add/mapboldwin.tex`).
    pub sheet_rgba: Option<(&'a [u8], usize, usize)>,
    /// Decoded marker overlay atlas (`pic/map/*mark.tex` or `pic/add/mapmark.tex`).
    pub mark_atlas_rgba: Option<(&'a [u8], usize, usize)>,
    /// Decoded `256x128` cursor/arrow sheet (`pic/etc/carsol.tex`).
    pub cursor_rgba: Option<(&'a [u8], usize, usize)>,
}

/// Computes a deterministic 64-bit state fingerprint for [`draw_paper_map`] (`SH2-177`).
///
/// Captures all visible map state (active sheet, zoom/pan coordinates, transcription alpha,
/// fade scrim, player arrow room/pos/yaw, event flags, loaded texture identities, and viewport).
#[must_use]
pub fn paper_map_render_fingerprint(
    viewer: &PaperMapViewer,
    flags: &EventFlags,
    player: PaperMapPlayerState,
    textures: PaperMapTextures<'_>,
    viewport: (usize, usize),
) -> u64 {
    let mut h = UiFingerprintHasher::with_tag(0x5041_5045_524D_4150);
    h.write_usize(viewport.0);
    h.write_usize(viewport.1);
    h.write_bool(viewer.is_open);
    h.write_u8(viewer.current_map_id);
    h.write_u8(viewer.selected_map_id);
    h.write_bool(viewer.is_maria_scenario);
    h.write_u8(viewer.zoom_step as u8);
    h.write_f32(viewer.zoom_t);
    h.write_f32(viewer.center_x);
    h.write_f32(viewer.center_y);
    h.write_u8(viewer.transcription_phase as u8);
    match viewer.transcribing_flag_id {
        Some(id) => {
            h.write_u8(1);
            h.write_u16(id);
        }
        None => h.write_u8(0),
    }
    h.write_u8(viewer.transcription_alpha);
    h.write_f32(viewer.screen_fade_alpha);
    h.write_usize(player.room_id);
    h.write_f32(player.pos_ps2.0);
    h.write_f32(player.pos_ps2.1);
    h.write_f32(player.yaw_rad);
    h.write_bytes(flags.as_bytes());
    h.write_opt_texture(textures.sheet_rgba);
    h.write_opt_texture(textures.mark_atlas_rgba);
    h.write_opt_texture(textures.cursor_rgba);
    h.finish()
}

/// Renders the 1:1 PS2 / SH2:EE Fullscreen Paper Map (`ChizuDisplay` `0x001c7ba0`).
pub fn draw_paper_map(
    viewer: &PaperMapViewer,
    flags: &EventFlags,
    player: PaperMapPlayerState,
    textures: PaperMapTextures<'_>,
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

    // 2. Base map sheet (`ChizuDisplay` 0x001c7ba0)
    let x0 = clipper.layout.canvas_left;
    let y0 = clipper.layout.canvas_top;
    let x1 = clipper.layout.canvas_right;
    let y1 = clipper.layout.canvas_bottom;
    let (u0, v0, u1, v1) = viewer.sheet_uv_rect();
    if let Some((rgba, sw, sh)) = textures.sheet_rgba {
        rasterizer.blit_rgba_subrect_scaled((x0, y0, x1, y1), (u0, v0, u1, v1), rgba, sw, sh, 1.0);
    } else {
        let dx = x0.floor().max(0.0) as usize;
        let dy = y0.floor().max(0.0) as usize;
        let dw = (x1 - x0).ceil().max(1.0) as usize;
        let dh = (y1 - y0).ceil().max(1.0) as usize;
        rasterizer.fill_rect(dx, dy, dw, dh, [186, 172, 142, 255]);
    }

    // 3. Dual-layer red marker pen decals (`ChizuMarkerDraw` 0x001c7e90)
    let markers = active_markers_for_map(viewer.current_map_id, flags);
    for mark in &markers {
        draw_atlas_marker(viewer, mark, &clipper, textures.mark_atlas_rgba, rasterizer);
    }

    // 4. Directional floor/wing connect arrows (`ChizuConnectArrowDraw` 0x001c88f0)
    draw_connect_arrows(viewer, flags, &clipper, textures.cursor_rgba, rasterizer);

    // 5. Textured 2-layer rotated player heading arrow (`ChizuCurrentPositionDraw` 0x001c9000)
    if chizu_should_draw_player_arrow(
        viewer.current_map_id,
        viewer.selected_map_id,
        player.room_id,
        flags,
    ) {
        draw_player_position_arrow(viewer, player, &clipper, textures.cursor_rgba, rasterizer);
    }

    // 6. Enforce pure black widescreen/ultrawide pillarboxes (`SetBlackPillarBoxes`)
    clipper.draw_black_pillarbox_bars(rasterizer);

    // 7. `Event/chizu.c:107` (`ScreenEffectFadeStart(0, 4)`) fade-in scrim
    if viewer.screen_fade_alpha > 0.001 {
        let a = (viewer.screen_fade_alpha.clamp(0.0, 1.0) * 255.0).round() as u8;
        rasterizer.fill_rect(0, 0, rasterizer.width, rasterizer.height, [0, 0, 0, a]);
    }
}

/// Renders a single `ChizuMarkerDraw` (`0x001c7e90`) decal using exact top-left anchor `(x_ps2, y_ps2)`
/// and `1.0 / (2.0 - disp_rate)` scaling (`(f + 1.0) * (u1 - u0) / 2.0`).
fn draw_atlas_marker(
    viewer: &PaperMapViewer,
    mark: &ChizuDecodedMarker,
    clipper: &MapCanvasClipper,
    mark_atlas_rgba: Option<(&[u8], usize, usize)>,
    rasterizer: &mut TextRasterizer,
) {
    let cw = clipper.layout.canvas_right - clipper.layout.canvas_left;
    let ch = clipper.layout.canvas_bottom - clipper.layout.canvas_top;
    let (mx0, my0) = viewer.project_ps2_to_viewport(mark.x_ps2, mark.y_ps2, clipper);
    let scale = viewer.marker_scale();
    let mw = (mark.width_px * scale) * (cw / 512.0);
    let mh = (mark.height_px * scale) * (ch / 512.0);
    let mx1 = mx0 + mw;
    let my1 = my0 + mh;

    let alpha_mul = if viewer.transcribing_flag_id == Some(mark.flag_id) {
        (viewer.transcription_alpha as f32) / (TRANSCRIBE_FADE_TARGET_ALPHA as f32)
    } else {
        1.0
    };

    if let Some((atlas, aw, ah)) = mark_atlas_rgba {
        // Normalize UVs against native PS2/PC *mark.tex dimensions so both native and 4x HD
        // SH2:EE atlases sample the exact same [0..1] sub-rectangle (`apartmapmark.tex` is 256x512;
        // all other *mark.tex files are 512x512).
        let (nat_w, nat_h) = if matches!(viewer.current_map_id, 4..=11) {
            (256.0f32, 512.0f32)
        } else {
            (512.0f32, 512.0f32)
        };
        let u0 = (mark.uv_px[0] as f32) / nat_w;
        let v0 = (mark.uv_px[1] as f32) / nat_h;
        let u1 = (mark.uv_px[2] as f32) / nat_w;
        let v1 = (mark.uv_px[3] as f32) / nat_h;
        if let Some((cx0, cy0, cx1, cy1, cu0, cv0, cu1, cv1)) =
            clipper.clip_decal_quad(mx0, my0, mx1, my1, u0, v0, u1, v1)
        {
            rasterizer.blit_rgba_subrect_scaled(
                (cx0, cy0, cx1, cy1),
                (cu0, cv0, cu1, cv1),
                atlas,
                aw,
                ah,
                alpha_mul,
            );
        }
    }
}

/// Renders directional map-switch arrows (`ChizuConnectArrowDraw` `0x001c88f0` / `sh2pc.exe` `0x0049d2d0`).
fn draw_connect_arrows(
    viewer: &PaperMapViewer,
    flags: &EventFlags,
    clipper: &MapCanvasClipper,
    cursor_rgba: Option<(&[u8], usize, usize)>,
    rasterizer: &mut TextRasterizer,
) {
    // Exact alpha ramp: 0x80 - disp_rate * 256.0 when disp_rate <= 0.5, else 0 (`0x001c89bc`).
    if viewer.zoom_t > 0.5 {
        return;
    }
    let alpha_mul = ((128.0 - viewer.zoom_t * 256.0).clamp(0.0, 128.0)) / 128.0;
    if alpha_mul <= 0.0 {
        return;
    }
    let Some((atlas, aw, ah)) = cursor_rgba else {
        return;
    };

    let cw = clipper.layout.canvas_right - clipper.layout.canvas_left;
    let ch = clipper.layout.canvas_bottom - clipper.layout.canvas_top;
    let scale = viewer.zoom_factor(); // (f + 1.0) = 2.0 / (2.0 - disp_rate)
    let dirs = [
        MapDirection::Up,
        MapDirection::Down,
        MapDirection::Right,
        MapDirection::Left,
    ];

    for (dir_idx, dir) in dirs.into_iter().enumerate() {
        if chizu_connect_check(
            viewer.current_map_id,
            dir,
            flags,
            viewer.selected_map_id,
            viewer.is_maria_scenario,
        )
        .is_none()
        {
            continue;
        }
        // Special arrow coordinates (`ChizuConnectArrowDraw` `0x001c8a80..0x001c8ad4`):
        // map*4 + dir == 0x29 (map 10 Down) or 0x2d (map 11 Down) -> (-216, 151)
        // map*4 + dir == 0x35 (map 13 Down) -> (-240, 172)
        let key = (viewer.current_map_id as usize) * 4 + dir_idx;
        let [ax, ay] = match key {
            0x29 | 0x2d => [-216.0, 151.0],
            0x35 => [-240.0, 172.0],
            _ => CHIZU_ARROW_DEF_POS[dir_idx],
        };
        let uv = CHIZU_ARROW_TEX_UVS[dir_idx];
        let w_ps2 = ((uv[2] as i32 - uv[0] as i32) as f32) * scale;
        let h_ps2 = ((uv[3] as i32 - uv[1] as i32) as f32) * scale;

        let (vx0, vy0) = viewer.project_ps2_to_viewport(ax, ay, clipper);
        let vx1 = vx0 + w_ps2 * (cw / 512.0);
        let vy1 = vy0 + h_ps2 * (ch / 512.0);

        // Native `carsol.tex` is 256x128 (SH2:EE HD is 4x = 1024x512).
        let u0 = (uv[0] as f32) / 256.0;
        let v0 = (uv[1] as f32) / 128.0;
        let u1 = (uv[2] as f32) / 256.0;
        let v1 = (uv[3] as f32) / 128.0;

        if let Some((cx0, cy0, cx1, cy1, cu0, cv0, cu1, cv1)) =
            clipper.clip_decal_quad(vx0, vy0, vx1, vy1, u0, v0, u1, v1)
        {
            rasterizer.blit_rgba_subrect_scaled(
                (cx0, cy0, cx1, cy1),
                (cu0, cv0, cu1, cv1),
                atlas,
                aw,
                ah,
                alpha_mul,
            );
        }
    }
}

/// Renders the 2-layer rotated textured player heading arrow (`ChizuCurrentPositionDraw`
/// `SLUS_202.28` `0x001c9000` / `sh2pc.exe` `0x0049d750`).
///
/// Draws shadow layer (`1`, offset `(+2.0, +0.5)`) beneath foreground layer (`0`), with exact
/// `[-10..+10] x [-16..+16]` local corners, `0.8` X aspect correction, and `uv$1582` UVs.
fn draw_player_position_arrow(
    viewer: &PaperMapViewer,
    player: PaperMapPlayerState,
    clipper: &MapCanvasClipper,
    cursor_rgba: Option<(&[u8], usize, usize)>,
    rasterizer: &mut TextRasterizer,
) {
    let angle = player.yaw_rad + chizu_arrow_yaw_offset(viewer.current_map_id, player.room_id);
    let s = angle.sin();
    let c = angle.cos();

    // Base local corners: TL(-10,-16), TR(10,-16), BL(-10,16), BR(10,16) (`0x001c9124..0x001c91b8`)
    let base_corners = [
        (c * -10.0 - s * -16.0, c * -16.0 + s * -10.0),
        (c * 10.0 - s * -16.0, c * -16.0 + s * 10.0),
        (c * -10.0 - s * 16.0, c * 16.0 + s * -10.0),
        (c * 10.0 - s * 16.0, c * 16.0 + s * 10.0),
    ];

    if let Some((atlas, aw, ah)) = cursor_rgba {
        // Draw shadow (layer 1, priority 7) first, then foreground (layer 0, priority 8)
        for layer in [1usize, 0usize] {
            let (ox, oy) = if layer == 1 { (2.0, 0.5) } else { (0.0, 0.0) };
            let mut v = [(0.0f32, 0.0f32); 4];
            for (i, (lx, ly)) in base_corners.iter().enumerate() {
                let px_ps2 = (lx + ox) * 0.8 + player.pos_ps2.0;
                let py_ps2 = (ly + oy) + player.pos_ps2.1;
                v[i] = viewer.project_ps2_to_viewport(px_ps2, py_ps2, clipper);
            }
            // Native `carsol.tex` is 256x128 (SH2:EE HD is 4x = 1024x512).
            let uv_q4 = CHIZU_PLAYER_ARROW_UVS[layer];
            let u0 = (uv_q4[0] as f32 / 16.0) / 256.0;
            let v0 = (uv_q4[1] as f32 / 16.0) / 128.0;
            let u1 = (uv_q4[2] as f32 / 16.0) / 256.0;
            let v1 = (uv_q4[3] as f32 / 16.0) / 128.0;
            let tris = [
                (v[0].0, v[0].1, u0, v0),
                (v[1].0, v[1].1, u1, v0),
                (v[3].0, v[3].1, u1, v1),
                (v[0].0, v[0].1, u0, v0),
                (v[3].0, v[3].1, u1, v1),
                (v[2].0, v[2].1, u0, v1),
            ];
            rasterizer.blit_overlay_triangles(&tris, atlas, aw, ah, 1.0);
        }
    }
}
