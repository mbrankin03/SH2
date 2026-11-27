//! PlayStation 2 Options Subscreens (`SLUS_202.28` `Item/otn_option.c`):
//! - `bright_main` (`0x00212ec0`): 20-bar brightness calibration (`level ∈ 0..=7`).
//! - `config_main` (`0x00213a50`): 13-action controller presets (`key_type1/2/3`) & remapping.
//! - `allow_u/d/l/r` (`0x00218ca0..0x00219bd0`): Pulsing lavender selection arrows (`RGB(0xb0,0xa0,0xff)`).
//!
//! Note: PS2 CRT overscan `position_main` (`0x00214c60`) is intentionally omitted on this modern native port.

use super::OptionsMenu;
use crate::atlas_font::{AtlasFont, Sh2TextColor};
use crate::font::TextRasterizer;
use sh2_core::config::{ControllerPreset, Ps2ControllerBindings, PS2_MAX_BRIGHTNESS_LEVEL};

/// Active PS2 Options subscreen (`option_main` `0x002108c0`, `t = 3, 4`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OptionsSubScreen {
    /// Main tabbed options screen (`t == 2`, `option_mainmain` `0x00210d50`).
    #[default]
    None,
    /// `t == 3`: 20-bar brightness calibration (`bright_main` `0x00212ec0`).
    BrightnessCalibration,
    /// `t == 4`: Controller presets & 13-action button config (`config_main` `0x00213a50`).
    ControllerConfig,
}

/// Direction for `allow_u/d/l/r` (`0x00218ca0..0x00219bd0`) pulsing arrow triangles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArrowDir {
    Up,
    Down,
    Left,
    Right,
}

/// Computes the exact PS2 `allow_u/d/l/r` (`0x00218ca0..0x00219bd0`) pulsing alpha (`0..=255`).
///
/// Formula: `f = frame % 40`, `p = (f < 20) ? f*3 : (120 - f*3)` (`0..=60`),
/// PS2 alpha = `p + (pressed ? 100 : 50)` (scaled ×2 from PS2 0..128 to 8-bit 0..255).
#[must_use]
pub fn arrow_pulse_alpha(frame: u32, pressed: bool) -> u8 {
    let f = frame % 40;
    let p = if f < 20 { f * 3 } else { 120 - f * 3 };
    let base = if pressed { 100 } else { 50 };
    ((p + base) * 2).min(255) as u8
}

/// Draws a pulsing PS2 options arrow (`allow_u/d/l/r` `0x00218ca0..0x00219bd0`) in centered
/// 512×448 coordinates `(cx, cy)` using exact lavender color `RGB(0xb0, 0xa0, 0xff)`.
pub fn draw_option_arrow(
    rast: &mut TextRasterizer,
    cx: f32,
    cy: f32,
    dir: ArrowDir,
    frame: u32,
    pressed: bool,
) {
    let (x_off, scale_x, scale_y) = viewport_512_448(rast);
    let to_px = |vx: f32, vy: f32| -> (f32, f32) {
        (x_off + (vx + 256.0) * scale_x, (vy + 224.0) * scale_y)
    };
    let alpha = arrow_pulse_alpha(frame, pressed);
    // PS2 ABGR `0x..ffa0b0` -> RGB `(0xb0, 0xa0, 0xff)` (verified in `sh2pc` `0x00460100`).
    let color = [0xb0, 0xa0, 0xff, alpha];
    let verts = match (dir, pressed) {
        (ArrowDir::Left, false) => [(cx, cy), (cx + 12.0, cy - 18.0), (cx + 12.0, cy + 18.0)],
        (ArrowDir::Left, true) => [
            (cx - 2.0, cy),
            (cx + 12.0, cy - 23.0),
            (cx + 12.0, cy + 23.0),
        ],
        (ArrowDir::Right, false) => [(cx, cy), (cx - 12.0, cy - 18.0), (cx - 12.0, cy + 18.0)],
        (ArrowDir::Right, true) => [
            (cx + 2.0, cy),
            (cx - 12.0, cy - 23.0),
            (cx - 12.0, cy + 23.0),
        ],
        (ArrowDir::Up, false) => [(cx, cy), (cx - 17.0, cy + 13.0), (cx + 17.0, cy + 13.0)],
        (ArrowDir::Up, true) => [
            (cx, cy - 2.0),
            (cx - 22.0, cy + 13.0),
            (cx + 22.0, cy + 13.0),
        ],
        (ArrowDir::Down, false) => [(cx, cy), (cx - 17.0, cy - 13.0), (cx + 17.0, cy - 13.0)],
        (ArrowDir::Down, true) => [
            (cx, cy + 2.0),
            (cx - 22.0, cy - 13.0),
            (cx + 22.0, cy - 13.0),
        ],
    };
    let screen_verts = [
        to_px(verts[0].0, verts[0].1),
        to_px(verts[1].0, verts[1].1),
        to_px(verts[2].0, verts[2].1),
    ];
    rast.fill_triangle(screen_verts, color);
}

/// Computes `(x_offset, scale_x, scale_y)` projecting 4:3 512×448 space into `rast`.
pub(crate) fn viewport_512_448(rast: &TextRasterizer) -> (f32, f32, f32) {
    let w = rast.width as f32;
    let h = rast.height as f32;
    let target_4_3_w = h * (4.0 / 3.0);
    let (x_offset, base_w) = if target_4_3_w <= w {
        ((w - target_4_3_w) * 0.5, target_4_3_w)
    } else {
        (0.0, w)
    };
    (x_offset, base_w / 512.0, h / 448.0)
}

/// Renders the PS2 20-bar Brightness Calibration subscreen (`bright_main` `0x00212ec0`).
///
/// Exact geometry from `SLUS_202.28` `0x00212ec0`:
/// - `20` (`0x14`) vertical lines at `x = i * 20 - 190` (`-190..=+190`), `y ∈ [-80, +150]`.
/// - Color: `RGB(0x80, 0x80, 0x80)`, PS2 alpha `level * 12` (`0..=84` in 0..128 GS scale).
/// - Arrows: `allow_l(10, 180)`, `allow_r(90, 180)`.
/// - Instruction text at `(25, 100)` and `"Level N"` readout at `(160, 420)` (512×448 top-left).
pub fn draw_brightness_subscreen(menu: &OptionsMenu, font: &AtlasFont, rast: &mut TextRasterizer) {
    let (x_off, sx, sy) = viewport_512_448(rast);
    let level = menu
        .draft_config
        .game
        .brightness_level
        .min(PS2_MAX_BRIGHTNESS_LEVEL);

    // Pure black backdrop (`look_scr` `0x0021bb90`)
    rast.fill_rect(0, 0, rast.width, rast.height, [0, 0, 0, 255]);

    // 20 vertical calibration lines (`i = 0..20`, `x = i * 20 - 190`, `y = -80..+150`)
    let line_alpha = ((level as u16) * 24).min(255) as u8;
    let line_color = [0x80, 0x80, 0x80, line_alpha];
    let y0 = ((-80.0 + 224.0) * sy).round() as i32;
    let y1 = ((150.0 + 224.0) * sy).round() as i32;
    let thick = sx.round().max(1.0) as usize;
    for i in 0..20 {
        let cx = (i as f32) * 20.0 - 190.0;
        let px = (x_off + (cx + 256.0) * sx).round() as i32;
        rast.draw_line(px, y0, px, y1, thick, line_color);
    }

    // Arrows `allow_l(10, 180)` & `allow_r(90, 180)`
    draw_option_arrow(rast, 10.0, 180.0, ArrowDir::Left, menu.anim_frame, false);
    draw_option_arrow(rast, 90.0, 180.0, ArrowDir::Right, menu.anim_frame, false);

    // Instruction text `(25, 100)` & level readout `(160, 420)` (`option_font` `0x0021a320`, `t == 3`)
    let font_sz = (15.0 * sy).round().max(11.0) as usize;
    let tx = (x_off + 25.0 * sx).round().max(0.0) as usize;
    let ty = (100.0 * sy).round().max(0.0) as usize;
    font.draw_line_styled(
        rast,
        "Adjust until vertical lines appear faint.",
        (tx, ty),
        font_sz,
        Sh2TextColor::White,
        true,
    );

    let lx = (x_off + 160.0 * sx).round().max(0.0) as usize;
    let ly = (420.0 * sy).round().max(0.0) as usize;
    font.draw_line_styled(
        rast,
        &format!("Brightness Level: {}", level),
        (lx, ly),
        font_sz,
        Sh2TextColor::White,
        true,
    );
}

/// Formats a PS2 pad bitmask (`key_type1/2/3` `0x00213610..0x00213750`) into readable button tokens.
#[must_use]
pub fn format_ps2_button_mask(mask: u32) -> String {
    const BITS: [(u32, &str); 12] = [
        (0x8000, "SQ"),
        (0x2000, "CI"),
        (0x1000, "TR"),
        (0x4000, "CR"),
        (0x0002, "R2"),
        (0x0001, "L2"),
        (0x0004, "L1"),
        (0x0008, "R1"),
        (0x10000, "SEL"),
        (0x40000, "R3"),
        (0x80000, "START"),
        (0x20000, "L3"),
    ];
    let mut parts = Vec::new();
    for (bit, name) in BITS {
        if (mask & bit) != 0 {
            parts.push(name);
        }
    }
    if parts.is_empty() {
        "---".to_string()
    } else {
        parts.join(" + ")
    }
}

/// Ordered PS2 remappable button bits (`config_main` `0x00213a50`).
pub const PS2_REMAPPABLE_BUTTON_BITS: [u32; 12] = [
    0x8000,  // Square
    0x2000,  // Circle
    0x1000,  // Triangle
    0x4000,  // Cross
    0x0002,  // R2
    0x0001,  // L2
    0x0004,  // L1
    0x0008,  // R1
    0x10000, // Select
    0x40000, // R3
    0x80000, // Start
    0x20000, // L3
];

/// Exact 1:1 port of `SLUS_202.28` `key_conf` (`Item/otn_option.c` `0x002137f0` lines 920-933):
/// strips any button bits in `new_mask` from `other_mask`, unless `require_non_empty` is `true`
/// and stripping would leave `other_mask == 0` (in which case those conflicting bits are instead
/// stripped from `new_mask` so essential actions can never become unbound).
pub fn key_conf(new_mask: &mut u32, other_mask: &mut u32, require_non_empty: bool) {
    let inv = !(*new_mask);
    if !require_non_empty || ((*other_mask & inv) != 0) {
        *other_mask &= inv;
    } else {
        *new_mask &= !(*other_mask);
    }
}

/// Exact 1:1 port of `SLUS_202.28` `key_conf_check` (`Item/otn_option.c` `0x00213840` lines 938-972):
/// resolves button collisions across the 13 `Ps2ControllerBindings::actions` slots when remapping
/// `action_idx` (`0..13`) to `new_mask`, protecting required actions (`param_3 == 1` in `otn_option.c`:
/// Action/Confirm `0`, Cancel `1`, Ready `5`, Map `8`, Inventory `10`).
pub fn key_conf_check(action_idx: usize, new_mask: u32, actions: &mut [u32; 13]) -> bool {
    if action_idx >= actions.len() || new_mask == 0 {
        return false;
    }
    let mut candidate = new_mask;
    // Required non-empty action indices matching `param_3 = 1` in `key_conf_check` (`0x00213840:948,957,963`)
    // and `config_main` (`0x00213a50:1188,1200`).
    const REQUIRED_NON_EMPTY: [usize; 5] = [0, 1, 5, 8, 10];
    for (idx, slot) in actions.iter_mut().enumerate() {
        if idx == action_idx {
            continue;
        }
        let req = REQUIRED_NON_EMPTY.contains(&idx);
        key_conf(&mut candidate, slot, req);
    }
    if candidate == 0 {
        return false;
    }
    actions[action_idx] = candidate;
    true
}

/// Renders the PS2 Controller Config subscreen (`config_main` `0x00213a50`).
///
/// Exact layout from `SLUS_202.28` `0x00213a50`:
/// - Top bar at `y = 60` (`0x3c`): `EXIT` (`x=30`), `Type 1` (`x=140`), `Type 2` (`x=230`),
///   `Type 3` (`x=320`), `RESET` (`x=410`).
/// - 13 action rows starting at `y = 102` (`0x66`), pitch `27px` (`0x1b`), button icons/masks
///   starting at `x = 140` (`0x8c`).
pub fn draw_controller_config_subscreen(
    menu: &OptionsMenu,
    font: &AtlasFont,
    rast: &mut TextRasterizer,
) {
    let (x_off, sx, sy) = viewport_512_448(rast);
    rast.fill_rect(0, 0, rast.width, rast.height, [0, 0, 0, 235]);

    let font_sz = (14.0 * sy).round().max(10.0) as usize;
    let top_items: [(f32, &str); 5] = [
        (30.0, "EXIT"),
        (140.0, "Type 1"),
        (230.0, "Type 2"),
        (320.0, "Type 3"),
        (410.0, "RESET"),
    ];
    for (col, &(vx, label)) in top_items.iter().enumerate() {
        let is_sel = menu.config_row == 0 && menu.config_preset_col == col;
        let is_active_preset = matches!(
            (col, menu.draft_config.controls.ps2_bindings.preset),
            (1, ControllerPreset::Type1)
                | (2, ControllerPreset::Type2)
                | (3, ControllerPreset::Type3)
        );
        let color = if is_sel {
            Sh2TextColor::GoldSelected
        } else if is_active_preset {
            Sh2TextColor::White
        } else {
            Sh2TextColor::SilverUnselected
        };
        let px = (x_off + vx * sx).round().max(0.0) as usize;
        let py = (60.0 * sy).round().max(0.0) as usize;
        font.draw_line_styled(rast, label, (px, py), font_sz, color, true);
    }

    // 13 action rows (`y = 102 + row * 27`, icons/buttons at `x = 140`)
    for (idx, label) in Ps2ControllerBindings::ACTION_LABELS.iter().enumerate() {
        let row_num = idx + 1;
        let is_sel = menu.config_row == row_num;
        let color = if is_sel {
            Sh2TextColor::White
        } else {
            Sh2TextColor::SilverUnselected
        };
        let vy = 102.0 + (idx as f32) * 27.0;
        let lx = (x_off + 40.0 * sx).round().max(0.0) as usize;
        let bx = (x_off + 170.0 * sx).round().max(0.0) as usize;
        let py = (vy * sy).round().max(0.0) as usize;

        font.draw_line_styled(rast, label, (lx, py), font_sz, color, true);
        let mask_str = format_ps2_button_mask(menu.draft_config.controls.ps2_bindings.actions[idx]);
        font.draw_line_styled(rast, &mask_str, (bx, py), font_sz, color, true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_conf_and_remap_resolve_conflicts_and_protect_required_actions() {
        // `key_conf` (`0x002137f0`): strips conflicting bit from non-required action.
        let mut new_mask = 0x0040u32; // Cross
        let mut other = 0x0040u32 | 0x0020u32;
        key_conf(&mut new_mask, &mut other, false);
        assert_eq!(new_mask, 0x0040);
        assert_eq!(other, 0x0020);

        // `key_conf` (`0x002137f0`): refuses to empty a required action (`other == new_mask`).
        let mut req_new = 0x0040u32;
        let mut req_other = 0x0040u32;
        key_conf(&mut req_new, &mut req_other, true);
        assert_eq!(req_new, 0);
        assert_eq!(req_other, 0x0040);

        // `key_conf_check` (`0x00213840`) updates an action slot cleanly when valid.
        let mut bindings = Ps2ControllerBindings::from_preset(ControllerPreset::Type1);
        let original = bindings.actions[10]; // Map
        let target_bit = PS2_REMAPPABLE_BUTTON_BITS
            .iter()
            .copied()
            .find(|&b| b != original && !bindings.actions.contains(&b))
            .unwrap_or(0x0002);
        assert!(key_conf_check(10, target_bit, &mut bindings.actions));
        assert_eq!(bindings.actions[10], target_bit);
    }
}
