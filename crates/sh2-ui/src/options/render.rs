//! Authentic PS2 `optmenu.c` (`0x45e750`, `0x461600`, `0x45bf10`) screen rasterization,
//! octagon panel geometry, volume slider blocks, and contextual help strings.

use super::{OptionsMenu, OptionsTab};
use crate::atlas_font::{AtlasFont, Sh2TextColor};
use crate::font::TextRasterizer;

/// Draws a convex polygon with optional fill and outline colors directly onto `rast`.
pub fn draw_polygon(
    rast: &mut TextRasterizer,
    points: &[(i32, i32)],
    fill: Option<[u8; 4]>,
    outline: Option<[u8; 4]>,
) {
    if points.len() < 3 {
        return;
    }

    let min_y = points.iter().map(|p| p.1).min().unwrap_or(0).max(0) as usize;
    let max_y = points
        .iter()
        .map(|p| p.1)
        .max()
        .unwrap_or(0)
        .min(rast.height as i32 - 1) as usize;

    if let Some(fill_color) = fill {
        let n = points.len();
        for y in min_y..=max_y {
            let py = y as f32 + 0.5;
            let mut nodes = Vec::new();
            for i in 0..n {
                let j = (i + 1) % n;
                let (x0, y0) = (points[i].0 as f32, points[i].1 as f32);
                let (x1, y1) = (points[j].0 as f32, points[j].1 as f32);

                if (y0 < py && y1 >= py) || (y1 < py && y0 >= py) {
                    let intersect_x = x0 + (py - y0) / (y1 - y0) * (x1 - x0);
                    nodes.push(intersect_x);
                }
            }
            nodes.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            for chunk in nodes.chunks(2) {
                if chunk.len() == 2 {
                    let start_x = chunk[0].round().max(0.0) as usize;
                    let end_x = (chunk[1].round() as usize).min(rast.width - 1);
                    for x in start_x..=end_x {
                        let idx = (y * rast.width + x) * 4;
                        let s_a = fill_color[3] as f32 / 255.0;
                        let d_a = rast.pixels[idx + 3] as f32 / 255.0;
                        let out_a = s_a + d_a * (1.0 - s_a);
                        if out_a > 0.0 {
                            let r = (fill_color[0] as f32 * s_a
                                + rast.pixels[idx] as f32 * d_a * (1.0 - s_a))
                                / out_a;
                            let g = (fill_color[1] as f32 * s_a
                                + rast.pixels[idx + 1] as f32 * d_a * (1.0 - s_a))
                                / out_a;
                            let b = (fill_color[2] as f32 * s_a
                                + rast.pixels[idx + 2] as f32 * d_a * (1.0 - s_a))
                                / out_a;
                            rast.pixels[idx] = r.clamp(0.0, 255.0) as u8;
                            rast.pixels[idx + 1] = g.clamp(0.0, 255.0) as u8;
                            rast.pixels[idx + 2] = b.clamp(0.0, 255.0) as u8;
                            rast.pixels[idx + 3] = (out_a * 255.0).clamp(0.0, 255.0) as u8;
                        }
                    }
                }
            }
        }
    }

    if let Some(outline_color) = outline {
        let n = points.len();
        for i in 0..n {
            let j = (i + 1) % n;
            draw_thick_line(
                rast,
                points[i].0,
                points[i].1,
                points[j].0,
                points[j].1,
                2,
                outline_color,
            );
        }
    }
}

/// Bresenham's line algorithm with thickness.
fn draw_thick_line(
    rast: &mut TextRasterizer,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    thickness: i32,
    color: [u8; 4],
) {
    let dx = (x1 - x0).abs();
    let dy = (y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx - dy;

    let mut cx = x0;
    let mut cy = y0;
    let radius = thickness / 2;

    loop {
        for oy in -radius..=radius {
            for ox in -radius..=radius {
                let px = cx + ox;
                let py = cy + oy;
                if px >= 0 && px < rast.width as i32 && py >= 0 && py < rast.height as i32 {
                    let idx = (py as usize * rast.width + px as usize) * 4;
                    let s_a = color[3] as f32 / 255.0;
                    let d_a = rast.pixels[idx + 3] as f32 / 255.0;
                    let out_a = s_a + d_a * (1.0 - s_a);
                    if out_a > 0.0 {
                        let r = (color[0] as f32 * s_a
                            + rast.pixels[idx] as f32 * d_a * (1.0 - s_a))
                            / out_a;
                        let g = (color[1] as f32 * s_a
                            + rast.pixels[idx + 1] as f32 * d_a * (1.0 - s_a))
                            / out_a;
                        let b = (color[2] as f32 * s_a
                            + rast.pixels[idx + 2] as f32 * d_a * (1.0 - s_a))
                            / out_a;
                        rast.pixels[idx] = r.clamp(0.0, 255.0) as u8;
                        rast.pixels[idx + 1] = g.clamp(0.0, 255.0) as u8;
                        rast.pixels[idx + 2] = b.clamp(0.0, 255.0) as u8;
                        rast.pixels[idx + 3] = (out_a * 255.0).clamp(0.0, 255.0) as u8;
                    }
                }
            }
        }

        if cx == x1 && cy == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 > -dy {
            err -= dy;
            cx += sx;
        }
        if e2 < dx {
            err += dx;
            cy += sy;
        }
    }
}

/// Draws an authentic 3D beveled volume block (15 total blocks matching `0x45bf10`).
fn draw_volume_block(rast: &mut TextRasterizer, x: i32, y: i32, lit: bool) {
    let w = 8;
    let h = 23;

    let (top_fill, bottom_fill, border_light, border_dark) = if lit {
        (
            [176u8, 176, 176, 255],
            [137u8, 137, 137, 255],
            [220u8, 220, 220, 255],
            [80u8, 80, 80, 255],
        )
    } else {
        (
            [80u8, 80, 80, 255],
            [60u8, 60, 60, 255],
            [100u8, 100, 100, 255],
            [40u8, 40, 40, 255],
        )
    };

    for dy in 0..h {
        let py = y + dy;
        if py < 0 || py >= rast.height as i32 {
            continue;
        }
        for dx in 0..w {
            let px = x + dx;
            if px < 0 || px >= rast.width as i32 {
                continue;
            }
            let idx = (py as usize * rast.width + px as usize) * 4;

            let c = if dx == 0 || dy == 0 {
                border_light
            } else if dx == w - 1 || dy == h - 1 {
                border_dark
            } else if dy < h / 2 {
                top_fill
            } else {
                bottom_fill
            };

            rast.pixels[idx] = c[0];
            rast.pixels[idx + 1] = c[1];
            rast.pixels[idx + 2] = c[2];
            rast.pixels[idx + 3] = c[3];
        }
    }
}

/// Contextual description for an option row based on active tab and item name.
pub(crate) fn option_help_text(tab: OptionsTab, name: &str) -> &'static str {
    match (tab, name) {
        (OptionsTab::Video, "Resolution") => "Internal rendering resolution for 3D world",
        (OptionsTab::Video, "Display Mode") => {
            "Display window mode (Windowed, Borderless, Fullscreen)"
        }
        (OptionsTab::Video, "V-Sync") => "Synchronize frame rate with display refresh rate",
        (OptionsTab::Video, "Frame Rate Cap") => "Maximum frame rate limit for game simulation",
        (OptionsTab::Video, "Fog Quality") => "Volumetric 3D fog raymarching and density quality",
        (OptionsTab::Video, "Film Grain / Noise") => "Authentic film grain noise filter intensity",
        (OptionsTab::Video, "Volumetric Lighting") => {
            "Dynamic volumetric light scattering from streetlamps and flashlight"
        }
        (OptionsTab::Video, "Lens Flare") => "Camera lens flare effects on bright light sources",
        (OptionsTab::Video, "Depth of Field") => {
            "Cinematic camera focal blur and distance depth of field"
        }
        (OptionsTab::Video, "World & Character Textures") => {
            "3-tier 3D world (.map) & character (.mdl) texture quality (4K AI / SH2:EE / PS2)"
        }
        (OptionsTab::Video, "FMV Cutscene Viewer") => {
            "Launch the interactive 1080p60 cutscene theater"
        }

        (OptionsTab::Audio, "Master Volume") => "Overall master sound and music volume level",
        (OptionsTab::Audio, "BGM Volume") => "Background music volume level",
        (OptionsTab::Audio, "SFX Volume") => "Sound effects and monster audio volume level",
        (OptionsTab::Audio, "Dialogue Volume") => {
            "Spoken character dialogue and voice volume level"
        }
        (OptionsTab::Audio, "Output Device") => {
            "Select active audio hardware output device (Speakers, Headphones, Interface)"
        }
        (OptionsTab::Audio, "Speaker Config") => {
            "Audio channel configuration (Stereo, Surround, Headphones)"
        }
        (OptionsTab::Audio, "Mute in Background") => "Silence audio when game window loses focus",
        (OptionsTab::Audio, "Restored Cut Audio") => {
            "Restore cut audio cues (Mary's Laura letter voiceover, closet & street bonus SFX)"
        }
        (OptionsTab::Audio, "Audio Pack") => {
            "Select BGM & SFX audio source (Auto, SH2:EE 48 kHz Lossless, or Original PS2/PC)"
        }

        (OptionsTab::Controls, "Movement Style") => {
            "Character control scheme (3D Tank or 2D Directional)"
        }
        (OptionsTab::Controls, "Button Prompt Glyphs") => {
            "Gamepad button prompt style (PlayStation, Xbox, Keyboard)"
        }
        (OptionsTab::Controls, "Stick Deadzone") => "Analog stick center deadzone threshold",
        (OptionsTab::Controls, "Stick Sensitivity") => {
            "Analog stick camera and rotation sensitivity"
        }
        (OptionsTab::Controls, "Vibration Intensity") => {
            "Gamepad haptic vibration and rumble intensity"
        }

        (OptionsTab::Game, "Action Difficulty") => {
            "Combat challenge and monster durability difficulty"
        }
        (OptionsTab::Game, "Riddle Difficulty") => {
            "Puzzle complexity and riddle challenge difficulty"
        }
        (OptionsTab::Game, "Subtitles") => "Display spoken dialogue subtitles on screen",
        (OptionsTab::Game, "Language") => "Select in-game text and subtitle language",
        (OptionsTab::Game, "Bullet Multiplier") => {
            "Multiplier for ammunition acquired from pickups"
        }
        (OptionsTab::Game, "Save Anywhere") => {
            "Allow saving anytime via pause menu (PC) or restrict to Red Squares (PS2)"
        }
        (OptionsTab::Game, "Test Rooms") => {
            "Select debug sandbox stage, fog calibration, or model viewer"
        }

        (OptionsTab::Debug, "Instant Room Cuts") => {
            "Speedrunner mode: cut instantly between rooms without loading screen"
        }
        (OptionsTab::Debug, "Min Loading Display") => {
            "Minimum duration to hold animated footstep loading screen (seconds)"
        }
        (OptionsTab::Debug, "Prompt To Continue") => {
            "Require pressing Enter / A to continue once loading completes"
        }
        (OptionsTab::Debug, "Loading Screen Preview") => {
            "Preview and test authentic PS2 animated footstep loading screen"
        }
        (OptionsTab::Debug, "Test Rooms Launcher") => {
            "Select debug sandbox stage, fog calibration, or model viewer"
        }
        (OptionsTab::Debug, "Collision Wireframe") => {
            "Render authentic CLD collision polygons and wall segments in world"
        }

        _ => "Adjust option settings",
    }
}

/// Draws the authentic options screen matching retail PC / PS2 `optmenu.c` (`0x45e750`, `0x461600`)
/// hosting our modern options (Resolution, Fog, Noise, Display Mode, Audio, Controls, Game).
#[allow(clippy::excessive_precision)]
pub fn draw_options_screen(menu: &OptionsMenu, font: &AtlasFont, rast: &mut TextRasterizer) {
    // Keep 4:3 authentic aspect ratio inside widescreen rasterizer
    let target_4_3_w = rast.height as f32 * (4.0 / 3.0);
    let (x_offset, base_w) = if target_4_3_w <= rast.width as f32 {
        ((rast.width as f32 - target_4_3_w) * 0.5, target_4_3_w)
    } else {
        (0.0, rast.width as f32)
    };

    let scale_x = base_w / 640.0;
    let scale_y = rast.height as f32 / 480.0;

    // Authentic PS2 octagon containers calibrated from retail PS2 SLUS-20228 / optmenu.c 0x45e750
    let panel_pts: Vec<(i32, i32)> = [
        (48.0, 64.0),
        (593.0, 64.0),
        (604.0, 76.0),
        (604.0, 407.0),
        (36.0, 407.0),
        (36.0, 76.0),
    ]
    .iter()
    .map(|(x, y)| {
        (
            (x_offset + x * scale_x).round() as i32,
            (y * scale_y).round() as i32,
        )
    })
    .collect();

    let box_pts: Vec<(i32, i32)> = [
        (36.0, 414.0),
        (604.0, 414.0),
        (604.0, 436.0),
        (580.0, 461.0),
        (60.0, 461.0),
        (36.0, 436.0),
    ]
    .iter()
    .map(|(x, y)| {
        (
            (x_offset + x * scale_x).round() as i32,
            (y * scale_y).round() as i32,
        )
    })
    .collect();

    let outline_pts: Vec<(i32, i32)> = [
        (55.0, 58.0),
        (586.0, 58.0),
        (612.0, 83.0),
        (612.0, 440.0),
        (586.0, 467.0),
        (55.0, 467.0),
        (30.0, 440.0),
        (30.0, 83.0),
    ]
    .iter()
    .map(|(x, y)| {
        (
            (x_offset + x * scale_x).round() as i32,
            (y * scale_y).round() as i32,
        )
    })
    .collect();

    let fill_color = [12, 12, 13, 255];
    let border_color = [4, 4, 18, 255];

    draw_polygon(rast, &panel_pts, Some(fill_color), Some(border_color));
    draw_polygon(rast, &box_pts, Some(fill_color), Some(border_color));
    draw_polygon(rast, &outline_pts, None, Some(border_color));

    let title_x = rast.width / 2;
    let title_y = (28.0 * scale_y).round() as usize;

    let category_header = match menu.active_tab {
        OptionsTab::Video => "<   V I D E O   O P T I O N S   >",
        OptionsTab::Audio => "<   A U D I O   O P T I O N S   >",
        OptionsTab::Controls => "<   C O N T R O L   O P T I O N S   >",
        OptionsTab::Game => "<   G A M E   O P T I O N S   >",
        OptionsTab::Debug => "<   D E B U G   T O O L S   >",
    };

    font.draw_line_centered(
        rast,
        category_header,
        (title_x, title_y),
        (22.0 * scale_y).round() as usize,
        Sh2TextColor::White,
        true,
    );

    let base_y = 84.0 * scale_y;
    let step_y = 29.5 * scale_y;

    let center_panel_x = (x_offset + 320.0 * scale_x).round() as usize;
    let label_right_x = (x_offset + 300.0 * scale_x).round() as usize;
    let val_left_x = (x_offset + 336.0 * scale_x).round() as usize;
    let font_size = (18.0 * scale_y).round() as usize;

    let item_count = menu.item_count();

    for idx in 0..item_count {
        if let Some((label, val)) = menu.item_display(idx) {
            let y = (base_y + idx as f32 * step_y).round() as usize;
            let is_selected = idx == menu.selected_index;

            let style = if is_selected {
                Sh2TextColor::White
            } else {
                Sh2TextColor::SilverUnselected
            };

            let is_slider = menu.active_tab == OptionsTab::Audio
                && (label == "BGM Volume"
                    || label == "SFX Volume"
                    || label == "Master Volume"
                    || label == "Dialogue Volume");

            if label == "FMV Cutscene Viewer"
                || label == "Test Rooms"
                || label == "Loading Screen Preview"
                || label == "Test Rooms Launcher"
            {
                if is_selected {
                    font.draw_line_centered(
                        rast,
                        label,
                        (center_panel_x.saturating_sub(2), y.saturating_sub(2)),
                        font_size,
                        Sh2TextColor::White,
                        false,
                    );
                }
                font.draw_line_centered(rast, label, (center_panel_x, y), font_size, style, true);
            } else {
                let label_w = font.text_width(label, font_size);
                let lx = label_right_x.saturating_sub(label_w);

                if is_selected {
                    font.draw_line_styled(
                        rast,
                        label,
                        (lx.saturating_sub(2), y.saturating_sub(2)),
                        font_size,
                        Sh2TextColor::White,
                        false,
                    );
                }
                font.draw_line_styled(rast, label, (lx, y), font_size, style, true);

                if is_slider {
                    let vol_pct = match label {
                        "Master Volume" => menu.draft_config.audio.master_volume,
                        "BGM Volume" => menu.draft_config.audio.bgm_volume,
                        "SFX Volume" => menu.draft_config.audio.sfx_volume,
                        "Dialogue Volume" => menu.draft_config.audio.dialogue_volume,
                        _ => 1.0,
                    };
                    let active_blocks = (vol_pct * 15.0).round() as usize;
                    let block_x_start = val_left_x as i32;
                    let block_y = (y as f32 + 1.0 * scale_y).round() as i32;

                    for b in 0..15 {
                        let bx = block_x_start + (b as f32 * 14.0 * scale_x).round() as i32;
                        let lit = b < active_blocks;
                        draw_volume_block(rast, bx, block_y, lit);
                    }
                } else {
                    if is_selected {
                        font.draw_line_styled(
                            rast,
                            &val,
                            (val_left_x.saturating_sub(2), y.saturating_sub(2)),
                            font_size,
                            Sh2TextColor::White,
                            false,
                        );
                    }
                    font.draw_line_styled(rast, &val, (val_left_x, y), font_size, style, true);
                }
            }
        }
    }

    let help_x = (x_offset + 90.0 * scale_x).round() as usize;
    let help_y = (436.0 * scale_y).round() as usize;

    let selected_label = menu
        .item_display(menu.selected_index)
        .map(|(l, _)| l)
        .unwrap_or("");

    let help_text = option_help_text(menu.active_tab, selected_label);

    font.draw_line_styled(
        rast,
        help_text,
        (help_x, help_y),
        font_size,
        Sh2TextColor::White,
        true,
    );
}
