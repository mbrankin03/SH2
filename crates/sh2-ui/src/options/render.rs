//! Authentic PS2 `optmenu.c` (`0x45e750`, `0x461600`, `0x45bf10`) screen rasterization,
//! octagon panel geometry, volume slider blocks, and contextual help strings.

use super::subscreens::{
    draw_brightness_subscreen, draw_controller_config_subscreen, viewport_512_448, OptionsSubScreen,
};
use super::{OptionsMenu, OptionsTab};
use crate::atlas_font::{AtlasFont, Sh2TextColor};
use crate::font::TextRasterizer;
use sh2_core::config::volume_gain_to_ps2_level;

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
                1,
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

/// Draws an authentic PS2 volume bar (`Item/otn_option.c` `look_bgm` `0x002159d0`, `look_se`
/// `0x00216500`):
/// - Active (`lit == true`): `9×23` outer bevel (`top-left 0xb0`, `bottom-right 0x40`) with
///   `5×19` inner face (`0x80`).
/// - Inactive (`lit == false`): `5×19` inner rect only (`0x50`), inset by `(2, 2)`.
/// - When the row is unselected, alpha is halved (`0x40` in PS2 GS 0..128 = `128` in 8-bit).
fn draw_volume_block(
    rast: &mut TextRasterizer,
    x: i32,
    y: i32,
    scale_x: f32,
    scale_y: f32,
    lit: bool,
    row_selected: bool,
) {
    let w = (9.0 * scale_x).round().max(4.0) as i32;
    let h = (23.0 * scale_y).round().max(8.0) as i32;
    let bx = (2.0 * scale_x).round().max(1.0) as i32;
    let by = (2.0 * scale_y).round().max(1.0) as i32;
    let a = if row_selected { 255u8 } else { 128u8 };

    if !lit {
        // Inactive bar: inner 5×19 rect only (`0x40505050`)
        let ix = (x + bx).max(0) as usize;
        let iy = (y + by).max(0) as usize;
        let iw = (w - 2 * bx).max(1) as usize;
        let ih = (h - 2 * by).max(1) as usize;
        rast.fill_rect(ix, iy, iw, ih, [0x50, 0x50, 0x50, a]);
        return;
    }

    let bevel_tl = [0xb0, 0xb0, 0xb0, a];
    let bevel_br = [0x40, 0x40, 0x40, a];
    let inner_fill = [0x80, 0x80, 0x80, a];

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
            let c = if dx < bx || dy < by {
                bevel_tl
            } else if dx >= w - bx || dy >= h - by {
                bevel_br
            } else {
                inner_fill
            };
            rast.blend_pixel(px as usize, py as usize, c);
        }
    }
}

/// Draws the PS2 dark gradient row selection highlight bar (`look_cur` `0x002184e0`):
/// 15 pairs of 1px horizontal strips above & below `cy` spanning `x ∈ [-226, +226]`,
/// pure black with alpha tapering from `15` at center to `1` at `±14px`.
fn draw_look_cur_bar(rast: &mut TextRasterizer, cy_512: f32) {
    let (x_off, sx, sy) = viewport_512_448(rast);
    let x0 = (x_off + (-226.0 + 256.0) * sx).round().max(0.0) as usize;
    let x1 = (x_off + (226.0 + 256.0) * sx)
        .round()
        .min(rast.width as f32) as usize;
    let bar_w = x1.saturating_sub(x0);
    if bar_w == 0 {
        return;
    }
    let strip_h = sy.ceil().max(1.0) as usize;
    for i in 0..15i32 {
        // PS2 alpha `(15 - i)` in 0..128 scale -> `((15 - i) * 2)` in 0..255
        let alpha = ((15 - i) * 2).min(255) as u8;
        let color = [0, 0, 0, alpha];
        let y_up = ((cy_512 - i as f32 + 224.0) * sy).round().max(0.0) as usize;
        let y_dn = ((cy_512 + i as f32 + 224.0) * sy).round().max(0.0) as usize;
        rast.fill_rect(x0, y_up, bar_w, strip_h, color);
        if i > 0 {
            rast.fill_rect(x0, y_dn, bar_w, strip_h, color);
        }
    }
}

/// Contextual description for an option row based on active tab and item name.
pub(crate) fn option_help_text(tab: OptionsTab, name: &str) -> &'static str {
    match (tab, name) {
        (OptionsTab::Root, "Game Options") => "View extra gameplay options",
        (OptionsTab::Root, "Advanced Options") => "View graphic and enhanced options",
        (OptionsTab::Root, "Audio Options") => "View audio and sound options",
        (OptionsTab::Root, "Control Options") => "View controller and camera options",
        (OptionsTab::Root, "Control Type") => "Select character movement control type",
        (OptionsTab::Root, "Vibration") => "Adjust controller vibration level",
        (OptionsTab::Root, "Language") => "Select game language",
        (OptionsTab::Root, "Subtitles") => "Display spoken dialogue subtitles on screen",
        (OptionsTab::Root, "Master Volume") => "Overall master sound and music volume level",
        (OptionsTab::Root, "BGM Volume") => "Background music volume level",
        (OptionsTab::Root, "SFX Volume") => "Sound effects and monster audio volume level",

        (OptionsTab::Video, "Enhanced Options") => {
            "Configure 1:1 PS2, SH2:EE, and Native Enhanced toggles"
        }
        (OptionsTab::Video, "Mod Manager") => {
            "Enable, disable, and reorder installed mods in mods/"
        }
        (OptionsTab::Video, "Title Screen Style") => {
            "Switch between SH2:EE title logo and original SILENT HILL 2 splash screen"
        }
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
        (OptionsTab::Video, "FMV Cutscene Source") => {
            "Select FMV source (Auto, 4K AI VSR, SH2:EE 1080p60, OG HQ PS2, or 2001 PC)"
        }
        (OptionsTab::Video, "Brightness Calibration") => {
            "Open PS2 20-bar brightness calibration screen (bright_main 0..7)"
        }
        (OptionsTab::Video, "ACES Tonemapping") => {
            "Modern ACES filmic HDR tonemapping (Native) vs strict PS2 linear clamp (1:1)"
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
        (OptionsTab::Audio, "3D Spatial Audio") => {
            "Binaural HRTF 3D headphone spatialization (ITD/ILD/elevation) vs Classic PS2 Stereo"
        }
        (OptionsTab::Audio, "Wall Occlusion") => {
            "Real-time .cld wall/door/slab LPF muffling + corridor reverb vs Original PS2"
        }
        (OptionsTab::Audio, "Mute in Background") => "Silence audio when game window loses focus",
        (OptionsTab::Audio, "Restored Cut Audio") => {
            "Restore cut audio cues (Mary's Laura letter voiceover, closet & street bonus SFX)"
        }
        (OptionsTab::Audio, "Dynamic Room Acoustics") => {
            "Dynamic .cam room-volume reverb presets (Native) vs dry Original PS2 mix"
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
        (OptionsTab::Controls, "Camera Mode") => {
            "Default camera (Classic PS2 Rail, Over-the-Shoulder 3D, or First-Person)"
        }
        (OptionsTab::Controls, "R3 Camera Cycle") => {
            "Camera modes cycled live in-game when pressing R3"
        }
        (OptionsTab::Controls, "OTS Shoulder") => {
            "Over-the-Shoulder framing side (Right or Left shoulder)"
        }
        (OptionsTab::Controls, "Aiming Reticle") => {
            "Diegetic aiming reticle visibility (Aim Only, Subtle Always, or Off)"
        }
        (OptionsTab::Controls, "First-Person FOV") => {
            "Vertical field of view in degrees for First-Person mode (55 to 100 deg)"
        }
        (OptionsTab::Controls, "First-Person Sway") => {
            "Subtle footstep and stride head-bob sway in First-Person mode"
        }
        (OptionsTab::Controls, "Controller Config (PS2)") => {
            "Open PS2 Controller Type 1/2/3 presets & 13-action remapping (config_main)"
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
        (OptionsTab::Game, "Delayed Fade-In (SH2:EE)") => {
            "Hold black ~4 frames after room transitions (SH2:EE) or fade immediately (PS2 1:1)"
        }
        (OptionsTab::Game, "Test Rooms") => {
            "Select debug sandbox stage, fog calibration, or model viewer"
        }
        (OptionsTab::Game, "Auto Load") => {
            "Automatically load the latest saved game on startup (PS2 Main Options)"
        }
        (OptionsTab::Game, "Blood Color") => {
            "Change blood splatter color: Normal, Green, Violet, or Black (PS2 Extra Options)"
        }
        (OptionsTab::Game, "View Control") => {
            "Search camera vertical/control inversion mode: Normal or Reverse (PS2 Extra Options)"
        }
        (OptionsTab::Game, "Retreat Turn") => {
            "Quick-turn retreat input direction: Normal or Reverse (PS2 Extra Options)"
        }
        (OptionsTab::Game, "Walk/Run Control") => {
            "Default movement speed: Normal (Walk), Reverse (Run), or Analog (PS2/PC Extra Options)"
        }
        (OptionsTab::Game, "Audio Options") => {
            "Configure 3D spatial audio, wall occlusion, and output device"
        }

        (OptionsTab::Enhanced, "Version Preset") => {
            "Instantly swap ALL enhanced toggles between PS2 Authentic, SH2:EE, and Native Enhanced"
        }
        (OptionsTab::Enhanced, "Textures") => {
            "3-tier world (.map) & character (.mdl) textures (4K AI Master / SH2:EE HD / PS2)"
        }
        (OptionsTab::Enhanced, "Fog Engine") => {
            "Modern 3D volumetric compute raymarched fog vs Classic PS2 atmospheric fog"
        }
        (OptionsTab::Enhanced, "Volumetric Lighting") => {
            "Dynamic volumetric flashlight god-rays & streetlamp scattering"
        }
        (OptionsTab::Enhanced, "Shadow Mode") => {
            "Native PCSS soft self-shadows vs SH2:EE soft stencil vs Classic PS2 .kg1/.kg2 stencil"
        }
        (OptionsTab::Enhanced, "Lighting Profile") => {
            "PS2 Authentic DrawEnvData vs SH2:EE lighting fixes vs Native Enhanced"
        }
        (OptionsTab::Enhanced, "3D Spatial Audio") => {
            "Binaural HRTF 3D headphone spatialization vs Classic PS2 Stereo panning"
        }
        (OptionsTab::Enhanced, "Wall Occlusion") => {
            "Real-time .cld wall/door/slab LPF muffling + corridor reverb vs Original PS2"
        }
        (OptionsTab::Enhanced, "Restored Cut Audio") => {
            "Restore cut audio cues (Mary's Laura letter voiceover, closet & street bonus SFX)"
        }
        (OptionsTab::Enhanced, "Camera Mode") => {
            "Classic PS2 Rail camera vs Over-the-Shoulder 3D vs First-Person immersion"
        }
        (OptionsTab::Enhanced, "Secondary Cloth Physics") => {
            "Native procedural spring-damper jacket/skirt physics vs Original PS2 keyframed cloth"
        }
        (OptionsTab::Enhanced, "Foot IK (Stairs)") => {
            "Analytical 2-bone Foot IK on .cld stairs & slopes (Native) vs Original PS2"
        }
        (OptionsTab::Enhanced, "ACES Tonemapping") => {
            "Modern ACES filmic HDR tonemapping (Native) vs strict PS2 linear clamp (1:1)"
        }
        (OptionsTab::Enhanced, "Dynamic Room Acoustics") => {
            "Dynamic .cam room-volume reverb presets (Native) vs dry Original PS2 mix"
        }
        (OptionsTab::Enhanced, "Unlimited Save Slots") => {
            "Unlimited save slots & JSON/binary export (Native) vs PS2 2 cards x 15 files (1:1)"
        }
        (OptionsTab::Enhanced, "Quicksave & Autosave") => {
            "Enable Quicksave/Autosave slots (Native) vs manual Red Square / Pause save (1:1)"
        }
        (OptionsTab::Enhanced, "Save Thumbnails") => {
            "Live 3D save thumbnails & Ending Tracker (Native) vs classic PS2 savebg (1:1)"
        }
        (OptionsTab::Enhanced, "Free 3D Item Orbit") => {
            "Free 3D item orbit & zoom during Examine (Native) vs fixed PS2 Examine (1:1)"
        }
        (OptionsTab::Enhanced, "Audio Pack") => {
            "Select BGM & SFX audio source (Auto, SH2:EE 48 kHz Lossless, or Original PS2/PC)"
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

/// Draws the authentic options screen matching PS2 `Item/otn_option.c` (`look_board` `0x002171a0`,
/// `option_font` `0x0021a320`, `look_cur` `0x002184e0`, `look_bgm`/`look_se` `0x002159d0`/`0x00216500`)
/// and dispatches active PS2 subscreens (`bright_main`, `config_main`, `position_main`).
#[allow(clippy::excessive_precision)]
pub fn draw_options_screen(menu: &OptionsMenu, font: &AtlasFont, rast: &mut TextRasterizer) {
    match menu.sub_screen {
        OptionsSubScreen::BrightnessCalibration => {
            draw_brightness_subscreen(menu, font, rast);
            return;
        }
        OptionsSubScreen::ControllerConfig => {
            draw_controller_config_subscreen(menu, font, rast);
            return;
        }
        OptionsSubScreen::None => {}
    }

    // 4:3 pillarboxed projection from PS2 512×448 centered space (`[-256..+256] × [-224..+224]`)
    let (x_offset, scale_x, scale_y) = viewport_512_448(rast);
    let to_centered = |cx: f32, cy: f32| -> (i32, i32) {
        (
            (x_offset + (cx + 256.0) * scale_x).round() as i32,
            ((cy + 224.0) * scale_y).round() as i32,
        )
    };

    // Exact `look_board` (`0x002171a0`) chamfered panels in centered 512×448 coordinates:
    // Top panel: (-210,-165), (+210,-165), (+227,-145), (+227,+155), (-227,+155), (-227,-145)
    let panel_pts: Vec<(i32, i32)> = [
        (-210.0, -165.0),
        (210.0, -165.0),
        (227.0, -145.0),
        (227.0, 155.0),
        (-227.0, 155.0),
        (-227.0, -145.0),
    ]
    .iter()
    .map(|&(x, y)| to_centered(x, y))
    .collect();

    // Bottom description box: (-227,+161), (+227,+161), (+227,+185), (+210,+205), (-210,+205), (-227,+185)
    let box_pts: Vec<(i32, i32)> = [
        (-227.0, 161.0),
        (227.0, 161.0),
        (227.0, 185.0),
        (210.0, 205.0),
        (-210.0, 205.0),
        (-227.0, 185.0),
    ]
    .iter()
    .map(|&(x, y)| to_centered(x, y))
    .collect();

    // Outer octagon border: (-212,-170), (+212,-170), (+232,-147), (+232,+187), (+212,+210), (-212,+210), (-232,+187), (-232,-147)
    let outline_pts: Vec<(i32, i32)> = [
        (-212.0, -170.0),
        (212.0, -170.0),
        (232.0, -147.0),
        (232.0, 187.0),
        (212.0, 210.0),
        (-212.0, 210.0),
        (-232.0, 187.0),
        (-232.0, -147.0),
    ]
    .iter()
    .map(|&(x, y)| to_centered(x, y))
    .collect();

    // Exact `look_board` (`0x002171a0`) colors converted from PS2 GS 8-bit gamma-space blend
    // (`fill: 0x20808080` -> 25% * 0x80 = 0x20; `border: 0x30d06060` -> 37.5% * (0x60,0x60,0xd0) = (0x24,0x24,0x4e))
    // so linear-space sRGB GPU blending matches PS2/SH2:EE's dark charcoal fill and subtle navy border.
    let fill_color = [0x20, 0x20, 0x20, 0xd8];
    let border_color = [0x24, 0x24, 0x4e, 0xff];

    draw_polygon(rast, &panel_pts, Some(fill_color), Some(border_color));
    draw_polygon(rast, &box_pts, Some(fill_color), Some(border_color));
    draw_polygon(rast, &outline_pts, None, Some(border_color));

    let title_x = (x_offset + 256.0 * scale_x).round() as usize;
    let title_y = (32.0 * scale_y).round() as usize;

    let category_header = match menu.active_tab {
        OptionsTab::Root => "O P T I O N S",
        OptionsTab::Video => "A D V A N C E D   O P T I O N S",
        OptionsTab::Audio => "A U D I O   O P T I O N S",
        OptionsTab::Controls => "C O N T R O L   O P T I O N S",
        OptionsTab::Game => "G A M E   O P T I O N S",
        OptionsTab::Enhanced => "E N H A N C E D   O P T I O N S",
        OptionsTab::Mods => "M O D   M A N A G E R",
        OptionsTab::Debug => "D E B U G   T O O L S",
    };

    font.draw_line_centered(
        rast,
        category_header,
        (title_x, title_y),
        (26.0 * scale_y).round().max(16.0) as usize,
        Sh2TextColor::White,
        true,
    );

    // `option_font` (`0x0021a320`): PS2 baseline `y = 100` (`0x64`), pitch `27` (`0x1b`) in 512×448.
    // Cap visible rows to 10 (`scroll_offset..scroll_offset+10`) so rows never overflow past `y = 379`.
    let item_count = menu.item_count();
    let (max_visible_rows, base_y_v, step_y_v) = if menu.active_tab == OptionsTab::Root {
        (11usize, 74.0f32, 25.0f32)
    } else {
        (10usize, 81.0f32, 27.0f32)
    };
    let visible_start = menu.scroll_offset.min(item_count.saturating_sub(1));
    let visible_end = (visible_start + max_visible_rows).min(item_count);
    let visible_sel_row = menu.selected_index.saturating_sub(visible_start);

    // Draw `look_cur` (`0x002184e0`) dark gradient highlight bar behind active row
    let sel_cy_512 = (base_y_v + visible_sel_row as f32 * step_y_v) - 224.0 + 12.0;
    draw_look_cur_bar(rast, sel_cy_512);

    // Draw right-edge vertical scrollbar track & beveled thumb on scrollable submenus (`item_count > max_visible_rows`)
    if item_count > max_visible_rows {
        let track_x = (x_offset + 468.0 * scale_x).round() as usize;
        let track_y = (80.0 * scale_y).round() as usize;
        let track_w = (4.0 * scale_x).round().max(3.0) as usize;
        let track_h = (264.0 * scale_y).round().max(40.0) as usize;

        // Recessed dark track with subtle border matching `look_board` (`0x002171a0`)
        rast.fill_rect(track_x, track_y, track_w, track_h, [0x12, 0x12, 0x15, 0xe0]);
        let max_scroll = (item_count - max_visible_rows).max(1);
        let thumb_h = ((track_h as f32 * max_visible_rows as f32) / item_count as f32)
            .round()
            .max(18.0 * scale_y) as usize;
        let avail_h = track_h.saturating_sub(thumb_h);
        let scroll_t = (visible_start as f32 / max_scroll as f32).clamp(0.0, 1.0);
        let thumb_y = track_y + (avail_h as f32 * scroll_t).round() as usize;
        let thumb_w = (6.0 * scale_x).round().max(4.0) as usize;
        let thumb_x = track_x.saturating_sub((thumb_w.saturating_sub(track_w)) / 2);
        let bevel = (1.0 * scale_x).round().max(1.0) as usize;

        // Beveled metallic grey thumb matching `look_bgm` (`0x002159d0`) volume bars
        rast.fill_rect(thumb_x, thumb_y, thumb_w, thumb_h, [0x45, 0x45, 0x4a, 0xff]);
        rast.fill_rect(
            thumb_x,
            thumb_y,
            thumb_w.saturating_sub(bevel),
            thumb_h.saturating_sub(bevel),
            [0xb8, 0xb8, 0xc0, 0xff],
        );
        if thumb_w > 2 * bevel && thumb_h > 2 * bevel {
            rast.fill_rect(
                thumb_x + bevel,
                thumb_y + bevel,
                thumb_w - 2 * bevel,
                thumb_h - 2 * bevel,
                [0x86, 0x86, 0x8e, 0xff],
            );
        }
    }

    let center_panel_x = (x_offset + 256.0 * scale_x).round() as usize;
    let label_right_x = (x_offset + 240.0 * scale_x).round() as usize;
    // `option_font` (`0x0021a320`): value column `x = 0x10e` (270) unselected, `0x10c` (268) selected.
    // Right bound is derived from existing `Mods` `[^]` button (`408.0`) or scrollbar `track_x` (`468.0`,
    // inside `look_board` `0x002171a0` right border `483.0`) minus a `6.0` virtual-px gutter.
    let val_left_x = (x_offset + 270.0 * scale_x).round() as usize;
    let val_right_v = if menu.active_tab == OptionsTab::Mods && !menu.mods.is_empty() {
        408.0 - 6.0
    } else {
        468.0 - 6.0
    };
    let val_right_x = (x_offset + val_right_v * scale_x).round() as usize;
    let max_val_w = val_right_x.saturating_sub(val_left_x);
    let font_size = (22.5 * scale_y).round().max(14.0) as usize;
    let min_val_font_size = (14.0 * scale_y).round().max(11.0) as usize;
    let pop_dx = (2.0 * scale_x).round().max(1.0) as usize;
    let pop_dy = (2.0 * scale_y).round().max(1.0) as usize;

    // Exact `option_font` (`0x0021a320`) unselected pass color: `alpha = 0x60` soft grey so
    // the selected `(-2, -2)` full-alpha white pass reads as a crisp pop with subtle shadow.
    let unselected_color = Sh2TextColor::Custom {
        top: [0x90, 0x90, 0x90, 0x60],
        bottom: [0x90, 0x90, 0x90, 0x60],
    };

    for (vis_row, idx) in (visible_start..visible_end).enumerate() {
        if let Some((label, val)) = menu.item_display_cow(idx) {
            let label: &str = label.as_ref();
            let y_v = base_y_v + vis_row as f32 * step_y_v;
            let y = (y_v * scale_y).round() as usize;
            let is_selected = idx == menu.selected_index;

            let is_slider = (menu.active_tab == OptionsTab::Audio
                || menu.active_tab == OptionsTab::Root)
                && (label == "BGM Volume"
                    || label == "SFX Volume"
                    || label == "Master Volume"
                    || label == "Dialogue Volume");

            if val.is_empty()
                || label == "FMV Cutscene Viewer"
                || label == "Test Rooms"
                || label == "Loading Screen Preview"
                || label == "Test Rooms Launcher"
            {
                font.draw_line_centered(
                    rast,
                    label,
                    (center_panel_x, y),
                    font_size,
                    unselected_color,
                    false,
                );
                if is_selected {
                    font.draw_line_centered(
                        rast,
                        label,
                        (
                            center_panel_x.saturating_sub(pop_dx),
                            y.saturating_sub(pop_dy),
                        ),
                        font_size,
                        Sh2TextColor::White,
                        false,
                    );
                }
            } else {
                let label_w = font.text_width(label, font_size);
                let lx = label_right_x.saturating_sub(label_w);

                // Unselected base pass (`alpha = 0x60`)
                font.draw_line_styled(rast, label, (lx, y), font_size, unselected_color, false);
                if is_selected {
                    // Selected crisp pass at `(-2, -2)` (`option_font` `0x0021a320`, `alpha = 0xff`)
                    font.draw_line_styled(
                        rast,
                        label,
                        (lx.saturating_sub(pop_dx), y.saturating_sub(pop_dy)),
                        font_size,
                        Sh2TextColor::White,
                        false,
                    );
                }

                if is_slider {
                    let vol_pct = match label {
                        "Master Volume" => menu.draft_config.audio.master_volume,
                        "BGM Volume" => menu.draft_config.audio.bgm_volume,
                        "SFX Volume" => menu.draft_config.audio.sfx_volume,
                        "Dialogue Volume" => menu.draft_config.audio.dialogue_volume,
                        _ => 1.0,
                    };
                    // `look_bgm`/`look_se` (`0x002159d0`/`0x00216500`): 16 levels (`0..=15`),
                    // 15 vertical bars starting at centered `x = 15` (`271` top-left), pitch `0xb` (11px).
                    let active_blocks = volume_gain_to_ps2_level(vol_pct) as usize;
                    let block_y = (y_v * scale_y).round() as i32;
                    for b in 0..15 {
                        let bx_v = 271.0 + (b as f32) * 11.0;
                        let bx = (x_offset + bx_v * scale_x).round() as i32;
                        draw_volume_block(
                            rast,
                            bx,
                            block_y,
                            scale_x,
                            scale_y,
                            b < active_blocks,
                            is_selected,
                        );
                    }
                } else {
                    let val_font_size =
                        fit_value_font_size(font, &val, font_size, min_val_font_size, max_val_w);
                    font.draw_line_styled(
                        rast,
                        &val,
                        (val_left_x, y),
                        val_font_size,
                        unselected_color,
                        false,
                    );
                    if is_selected {
                        font.draw_line_styled(
                            rast,
                            &val,
                            (val_left_x.saturating_sub(pop_dx), y.saturating_sub(pop_dy)),
                            val_font_size,
                            Sh2TextColor::White,
                            false,
                        );
                    }
                    if menu.active_tab == OptionsTab::Mods && !menu.mods.is_empty() {
                        let up_x = (x_offset + 408.0 * scale_x).round() as usize;
                        let dn_x = (x_offset + 436.0 * scale_x).round() as usize;
                        let order_color = if is_selected {
                            Sh2TextColor::White
                        } else {
                            unselected_color
                        };
                        let draw_y = if is_selected {
                            y.saturating_sub(pop_dy)
                        } else {
                            y
                        };
                        font.draw_line_styled(
                            rast,
                            "[^]",
                            (up_x, draw_y),
                            font_size,
                            order_color,
                            false,
                        );
                        font.draw_line_styled(
                            rast,
                            "[v]",
                            (dn_x, draw_y),
                            font_size,
                            order_color,
                            false,
                        );
                    }
                }
            }
        }
    }

    // Description text (`option_font` `0x0021a320`: `x = 0x46` (70), `y = 0x1a6` (422) in 512×448)
    let help_x = if menu.active_tab == OptionsTab::Mods {
        (x_offset + 54.0 * scale_x).round() as usize
    } else {
        (x_offset + 70.0 * scale_x).round() as usize
    };
    let help_y = (394.0 * scale_y).round() as usize;
    let help_font_size = if menu.active_tab == OptionsTab::Mods {
        (19.0 * scale_y).round().max(12.0) as usize
    } else {
        (21.0 * scale_y).round().max(13.0) as usize
    };

    let mod_help_owned;
    let help_text = if menu.active_tab == OptionsTab::Mods {
        mod_help_owned = menu.mod_help_text(menu.selected_index);
        mod_help_owned.as_str()
    } else {
        let selected_label = menu
            .item_display(menu.selected_index)
            .map(|(l, _)| l)
            .unwrap_or("");
        option_help_text(menu.active_tab, selected_label)
    };
    font.draw_line_styled(
        rast,
        help_text,
        (help_x, help_y),
        help_font_size,
        Sh2TextColor::Custom {
            top: [0xa8, 0xa8, 0xa8, 0xff],
            bottom: [0x88, 0x88, 0x88, 0xff],
        },
        true,
    );
}

/// Reduces `base_size` down to `min_size` as needed so `text` fits within `max_width` pixels.
fn fit_value_font_size(
    font: &AtlasFont,
    text: &str,
    base_size: usize,
    min_size: usize,
    max_width: usize,
) -> usize {
    let mut size = base_size;
    while size > min_size && font.text_width(text, size) > max_width {
        size -= 1;
    }
    size
}

#[cfg(test)]
mod tests {
    use super::*;
    use sh2_core::config::FmvCutsceneSource;

    #[test]
    fn fmv_cutscene_source_labels_fit_derived_value_column() {
        let font = AtlasFont::load_embedded();
        let all_sources = [
            FmvCutsceneSource::Auto,
            FmvCutsceneSource::Upscaled4kMp4,
            FmvCutsceneSource::Enhanced1080p60,
            FmvCutsceneSource::OgHqPs2Masters,
            FmvCutsceneSource::VanillaPc,
        ];
        for &(w, h) in &[(1280.0f32, 720.0f32), (3840.0f32, 2160.0f32)] {
            let scale_y = h / 448.0;
            let scale_x = scale_y;
            let x_offset = (w - 512.0 * scale_x) * 0.5;
            let val_left_x = (x_offset + 270.0 * scale_x).round() as usize;
            let val_right_x = (x_offset + (468.0 - 6.0) * scale_x).round() as usize;
            let max_val_w = val_right_x.saturating_sub(val_left_x);
            let base_size = (22.5 * scale_y).round().max(14.0) as usize;
            let min_size = (14.0 * scale_y).round().max(11.0) as usize;

            for src in all_sources {
                let fitted =
                    fit_value_font_size(&font, src.label(), base_size, min_size, max_val_w);
                assert!(
                    font.text_width(src.label(), fitted) <= max_val_w,
                    "FMV label {:?} overflowed max_val_w={} at {}x{} (fitted={})",
                    src.label(),
                    max_val_w,
                    w,
                    h,
                    fitted
                );
            }
        }
    }

    #[test]
    fn fit_value_font_size_clamps_long_text_and_respects_scaled_floor() {
        let font = AtlasFont::load_embedded();
        let scale_y: f32 = 2160.0 / 448.0;
        let scale_x = scale_y;
        let mods_max_w = ((408.0 - 6.0 - 270.0) * scale_x).round() as usize;
        let base_size = (22.5 * scale_y).round().max(14.0) as usize;
        let min_size = (14.0 * scale_y).round().max(11.0) as usize;

        let fitted =
            fit_value_font_size(&font, "ENABLED  Load #12", base_size, min_size, mods_max_w);
        assert!(fitted >= min_size);
        assert!(font.text_width("ENABLED  Load #12", fitted) <= mods_max_w);

        let extreme = fit_value_font_size(
            &font,
            "Extremely Long Value String That Exceeds Even Minimum Font Width",
            base_size,
            min_size,
            mods_max_w,
        );
        assert_eq!(extreme, min_size);
    }
}
