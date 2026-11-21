//! Silent Hill 2 `.mes` Control-Code Interpreter, Palette, Dictionary & Yes/No Prompt Engine.
//!
//! 1:1 port of `SLUS_202.28` `Font/font.c` & `Font/dic.c`:
//! - `fontPrintStrMain` (`0x001b4a10`): 2-pass layout & control-code interpreter (`0x0000`,
//!   `0x0001..0x7FFE`, `0x8000` byte-packed mode, `0xF800..0xFFFF` opcodes).
//! - `fontSetColor` (`0x001b73a0`): 17-entry top/bottom gradient palette (`@1369` at `0x002baec0`,
//!   noting `sh2pc.exe` `0x0047e7f0` index 10 drift `0x00909090` -> `0x00808080`).
//! - `dicSetStr` (`0x00171220`) & `fontSetMes` (`0x001b9850`): backslash escape compiler & 10
//!   dynamic substitution slots (`\v0`..`\v9`, `0xFFE0..0xFFE9`).
//! - `fontSetYesNo` (`0x001b5d00`) & `fontPutYesNoSelectBar` (`0x001b90a0`): Yes (`x=200`) /
//!   No (`x=312`) layout & pulsing blue selection bar (`RGB(16, 32, 192)`, `alpha 16..80` GS).
//! - `FontSize` (`0x002bacd8`): Type 0 Normal (`20×30`) vs Type 1 Small (`16×24`, `fontCrushOn` `0x001b9f60`).

/// Font size type (`SLUS_202.28` `FontSize` at `0x002bacd8`, `sh2pc.exe` `0x00803898`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MesFontType {
    /// Type 0 (`fontCrushOff` `0x001b9f80`): `20 × 30` px base glyph cell.
    #[default]
    Normal = 0,
    /// Type 1 (`fontCrushOn` `0x001b9f60`): `16 × 24` px small/crushed glyph cell.
    Small = 1,
}

impl MesFontType {
    /// Returns `(base_width, base_height)` in virtual 512×448 pixels (`0x002bacd8`).
    #[must_use]
    pub const fn base_dimensions(self) -> (u16, u16) {
        match self {
            Self::Normal => (20, 30),
            Self::Small => (16, 24),
        }
    }
}

/// Exact 17-entry `(top_rgb, bot_rgb)` color palette from `SLUS_202.28` `fontSetColor` (`0x001b73a0`,
/// table `@1369` at `0x002baec0`). Stored as `(R, G, B)` tuples decoded from PS2 `0x00BBGGRR`.
///
/// Prototype vs Retail PC drift (`sh2pc.exe` `FUN_0047e7f0` `0x0047e7f0`):
/// Index `10` top color is `(144, 144, 144)` (`0x00909090`) in `SLUS_202.28` and `(128, 128, 128)`
/// (`0x00808080`) in retail `sh2pc.exe`; all other 16 entries are identical.
pub const FONT_COLOR_PALETTE_PS2: [([u8; 3], [u8; 3]); 17] = [
    ([128, 128, 128], [128, 128, 128]), // 0: White (PS2 1.0)
    ([96, 96, 128], [96, 96, 128]),     // 1: Blue-grey
    ([128, 48, 48], [128, 48, 48]),     // 2: Red
    ([96, 128, 96], [96, 128, 96]),     // 3: Green
    ([128, 128, 48], [128, 128, 48]),   // 4: Yellow
    ([48, 128, 128], [48, 128, 128]),   // 5: Cyan
    ([128, 48, 128], [128, 48, 128]),   // 6: Magenta
    ([80, 80, 160], [80, 160, 80]),     // 7: Blue -> Green vertical gradient
    ([48, 0, 0], [80, 32, 32]),         // 8: Dark crimson gradient
    ([0, 0, 0], [0, 0, 0]),             // 9: Black
    ([144, 144, 144], [32, 32, 32]),    // 10: White -> Dark grey gradient (PC top: 128,128,128)
    ([224, 16, 40], [224, 16, 40]),     // 11: Bright red
    ([112, 88, 160], [112, 88, 160]),   // 12: Lavender/purple
    ([136, 208, 56], [136, 208, 56]),   // 13: Lime green
    ([248, 248, 16], [248, 248, 16]),   // 14: Bright yellow
    ([32, 184, 168], [32, 184, 168]),   // 15: Teal
    ([32, 32, 32], [32, 32, 32]),       // 16: Dark grey
];

/// Returns the `(top_rgb, bot_rgb)` palette entry for `index` (`0..=16`, clamped to `0` if invalid,
/// `fontSetColor` `0x001b73a0`).
#[must_use]
pub const fn font_set_color(index: u8, retail_pc_palette: bool) -> ([u8; 3], [u8; 3]) {
    if index as usize >= FONT_COLOR_PALETTE_PS2.len() {
        return FONT_COLOR_PALETTE_PS2[0];
    }
    if retail_pc_palette && index == 10 {
        ([128, 128, 128], [32, 32, 32])
    } else {
        FONT_COLOR_PALETTE_PS2[index as usize]
    }
}

/// Horizontal alignment mode in `fontPrintStrMain` (`0x001b4a10`, `iStack_c0`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MesAlign {
    /// `0` (`\l` `0xFFFC`): Block horizontal center (`anchor_x - max_line_w / 2`).
    #[default]
    BlockCenter = 0,
    /// `1` (`\c` `0xFFFB`): Per-line horizontal center (`anchor_x - line_w / 2`).
    LineCenter = 1,
    /// `2` (`\xN` `0xFA00..0xFBFF`): Explicit left-aligned X.
    Left = 2,
}

/// Page wait/advance mode decoded from the 16-bit word after `0xFFFF` (`fontNextMessage` `0x001b9cb0`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MesWaitMode {
    /// `0`: None / immediate end.
    #[default]
    None,
    /// `1` (`\pk`/`\ek`): Wait for button press.
    WaitButton,
    /// `2` (`\pwN`/`\ewN`): Timed auto-advance, unskippable (`N` frames at 60 Hz).
    TimedUnskippable(u16),
    /// `3` (`\paN`/`\eaN`): Timed auto-advance, skippable (`N` frames at 60 Hz).
    TimedSkippable(u16),
    /// `4`: Interactive choice or Yes/No prompt (`_DAT_0115867a != 0`).
    InteractiveSelect,
}

/// A positioned glyph or primitive emitted by Pass 2 of `fontPrintStrMain` (`0x001b4a10`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MesDrawCmd {
    /// Glyph code (`0x0001..0x7FFE`) at virtual 512×448 `(x, y)` with palette index `0..=16`.
    Glyph {
        code: u16,
        x: i32,
        y: i32,
        color_idx: u8,
        font_type: MesFontType,
    },
    /// Horizontal rule (`0xFE00..0xFEFF` -> `fontSetLine` `0x001b47b0`).
    Rule {
        x: i32,
        y: i32,
        width: u16,
        color_idx: u8,
    },
    /// Redacted/blank box (`0xFFF7` pair -> `fontSetBlankBox` `0x001b46a0`).
    BlankBox { x0: i32, x1: i32, y: i32 },
    /// Yes/No prompt (`0xFFF5` / `0xFFF6` -> `fontSetYesNo` `0x001b5d00`: Yes at `x=200`, No at `x=312`).
    YesNoPrompt {
        y: i32,
        yes_center_x: i32,
        no_center_x: i32,
        default_index: u8,
    },
}

/// Result of executing `fontPrintStrMain` (`0x001b4a10`) on a `.mes` token stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MesLayoutResult {
    pub commands: Vec<MesDrawCmd>,
    pub line_count: usize,
    pub max_line_width: i32,
    pub is_final_page: bool,
    pub wait_mode: MesWaitMode,
    /// `Some((y, default_idx))` if a `0xFFF5` (`No=1`) or `0xFFF6` (`Yes=0`) prompt was emitted.
    pub yes_no: Option<(i32, u8)>,
    /// Selectable `\s` (`0xFFF9`) choice Y-bounds `(top_y, bot_y)`.
    pub choice_bounds: Vec<(i32, i32)>,
    /// Default highlighted choice index (`\d` `0xFFF8` or Yes/No default).
    pub default_selection: usize,
}

/// Exact `fontSetYesNo` (`0x001b5d00`) center X coordinates in 512×448 space.
pub const YES_NO_YES_CENTER_X: i32 = 200; // `0xC8`
pub const YES_NO_NO_CENTER_X: i32 = 312; // `0x138`

/// Exact `fontPutYesNoSelectBar` (`0x001b90a0`) RGB color (`0xC02010` ABGR -> `RGB(16, 32, 192)`).
pub const YES_NO_BAR_RGB: [u8; 3] = [0x10, 0x20, 0xC0];

/// Computes the exact `fontBarBlink` (`0x001b74c0`) + `fontPutYesNoSelectBar` (`0x001b90a0`)
/// pulsing alpha in PS2 GS `0..=128` scale (`[16, 80]`) for phase `t ∈ [0.0, 1.0)`.
#[must_use]
pub fn yes_no_bar_alpha_gs(phase_0_to_1: f32) -> u8 {
    let t = phase_0_to_1.rem_euclid(1.0);
    let tri = if t < 0.5 { t } else { 1.0 - t };
    ((tri / 0.5) * 64.0) as u8 + 16
}

/// Stateful `.mes` interpreter holding the 10 dynamic substitution slots (`\v0`..`\v9`,
/// `fontSetMes` `0x001b9850`) and active font type (`Normal` 20×30 vs `Small` 16×24).
#[derive(Debug, Clone)]
pub struct MesInterpreter {
    pub font_type: MesFontType,
    pub override_size: Option<(u16, u16)>,
    /// 10 dynamic substitution slots (`0x0115867e + slot * 0x80`, `fontSetMes` `0x001b9850`).
    pub slots: [Vec<u16>; 10],
}

impl Default for MesInterpreter {
    fn default() -> Self {
        Self::new()
    }
}

impl MesInterpreter {
    /// Creates a new interpreter initialized to Normal (`20×30`) font and empty dynamic slots.
    #[must_use]
    pub fn new() -> Self {
        Self {
            font_type: MesFontType::Normal,
            override_size: None,
            slots: std::array::from_fn(|_| vec![0xFFFF, 0x8000]),
        }
    }

    /// Sets a dynamic substitution slot (`0..=9`, `fontSetMes` `0x001b9850`). Out-of-range
    /// slot indices clamp to `0` matching `SLUS_202.28` `0x001b9850`.
    pub fn font_set_mes(&mut self, slot: usize, tokens: Vec<u16>) {
        let idx = if slot < 10 { slot } else { 0 };
        self.slots[idx] = tokens;
    }

    /// Compiles an ASCII string with Team Silent backslash escapes into a `0xFFFF`-terminated
    /// `u16` `.mes` token buffer (`Font/dic.c` `dicSetStr` `0x00171220`).
    #[must_use]
    pub fn dic_set_str(input: &str) -> Vec<u16> {
        let mut out = Vec::with_capacity(input.len() + 2);
        let bytes = input.as_bytes();
        let mut i = 0usize;
        let mut half_width = true;
        let mut end_ctrl: u16 = 0x8000;

        while i < bytes.len() {
            let b = bytes[i];
            if b == b'\\' && i + 1 < bytes.len() {
                i += 1;
                match bytes[i] {
                    b'h' | b'H' | b'!' => {
                        half_width = true;
                        i += 1;
                    }
                    b'z' | b'Z' => {
                        half_width = false;
                        i += 1;
                    }
                    b'n' => {
                        out.push(0xFFFD);
                        i += 1;
                    }
                    b'l' => {
                        out.push(0xFFFC);
                        i += 1;
                    }
                    b'c' => {
                        out.push(0xFFFB);
                        i += 1;
                    }
                    b'a' => {
                        out.push(0xFFFA);
                        i += 1;
                    }
                    b's' => {
                        out.push(0xFFF9);
                        i += 1;
                    }
                    b'd' => {
                        out.push(0xFFF8);
                        i += 1;
                    }
                    b'b' => {
                        out.push(0xFFF7);
                        i += 1;
                    }
                    b'v' => {
                        i += 1;
                        let slot = if i < bytes.len() && bytes[i].is_ascii_digit() {
                            let d = (bytes[i] - b'0') as u16;
                            i += 1;
                            d
                        } else {
                            0
                        };
                        out.push(0xFFE0 + slot);
                    }
                    b'+' | b'-' | b'_' | b'x' | b'y' => {
                        let tag = bytes[i];
                        i += 1;
                        let val = parse_decimal(bytes, &mut i);
                        let code = match tag {
                            b'+' => 0xFD00 | (val & 0xFF),
                            b'-' => 0xFC00 | (val & 0xFF),
                            b'_' => 0xFE00 | (val & 0xFF),
                            b'x' => 0xFA00 | (val & 0x1FF),
                            _ => 0xF800 | (val & 0x1FF),
                        };
                        out.push(code);
                    }
                    b'p' | b'e' => {
                        let is_end = bytes[i] == b'e';
                        i += 1;
                        let mut ctrl: u16 = if is_end { 0x8000 } else { 0 };
                        if i < bytes.len() {
                            match bytes[i] {
                                b'k' => {
                                    ctrl |= 0x1000;
                                    i += 1;
                                }
                                b'w' => {
                                    i += 1;
                                    let n = parse_decimal(bytes, &mut i) & 0x0FFF;
                                    ctrl |= 0x2000 | n;
                                }
                                b'a' => {
                                    i += 1;
                                    let n = parse_decimal(bytes, &mut i) & 0x0FFF;
                                    ctrl |= 0x3000 | n;
                                }
                                _ => {}
                            }
                        }
                        if is_end {
                            end_ctrl = ctrl;
                            break;
                        } else {
                            out.push(0xFFFF);
                            out.push(ctrl);
                        }
                    }
                    d if d.is_ascii_digit() => {
                        let color = parse_decimal(bytes, &mut i).min(99);
                        out.push(0xFF00 + color);
                    }
                    other => {
                        if other >= 0x20 {
                            out.push((other - 0x20) as u16);
                        }
                        i += 1;
                    }
                }
            } else if b == b'\n' {
                out.push(0xFFFD);
                i += 1;
            } else if b == b' ' {
                out.push(if half_width { 0x0000 } else { 0xFFFE });
                i += 1;
            } else if b >= 0x20 {
                out.push((b - 0x20) as u16);
                i += 1;
            } else {
                i += 1;
            }
        }

        out.push(0xFFFF);
        out.push(end_ctrl);
        out
    }

    /// Expands a `.mes` `u16` stream (supporting `0x8000` byte-packed mode and 1-level `0xFFE0..0xFFE9`
    /// dynamic slot substitution) and runs the 2-pass `fontPrintStrMain` (`0x001b4a10`) layout.
    #[must_use]
    pub fn font_print_str_main(
        &self,
        tokens: &[u16],
        explicit_x: Option<i32>,
        explicit_y: Option<i32>,
    ) -> MesLayoutResult {
        let (base_w, base_h) = self.font_type.base_dimensions();
        let (draw_w, draw_h) = self.override_size.unwrap_or((base_w, base_h));
        let (expanded, end_ctrl) = self.expand_page_tokens(tokens);

        // Pass 1: measure line widths, alignment, vertical center, and explicit X/Y
        let mut align = if explicit_x.is_some() {
            MesAlign::Left
        } else {
            MesAlign::BlockCenter
        };
        let mut anchor_x = explicit_x.unwrap_or(256); // 0x800 - 0x700 = 256
        let mut anchor_y = explicit_y;
        let mut vcenter = false;
        let mut line_widths: Vec<i32> = Vec::new();
        let mut cur_w = 0i32;

        for &code in &expanded {
            match code {
                0x0000 => cur_w += (draw_w as i32 * 5) / (base_w as i32) + 2,
                0x0001..=0x00DF => {
                    let adv = (draw_w as i32 * 10) / (base_w as i32)
                        + 2
                        + i32::from(self.font_type == MesFontType::Small);
                    cur_w += adv;
                }
                0x00E0..=0x7FFE | 0xFFFE => cur_w += draw_w as i32 + 2,
                0xF800..=0xF9FF => anchor_y = Some((code & 0x1FF) as i32),
                0xFA00..=0xFBFF => {
                    anchor_x = (code & 0x1FF) as i32;
                    align = MesAlign::Left;
                }
                0xFC00..=0xFCFF => cur_w -= (code & 0xFF) as i32,
                0xFD00..=0xFDFF => cur_w += (code & 0xFF) as i32,
                0xFE00..=0xFEFF => cur_w += (code & 0xFF) as i32 + 2,
                0xFFFA => vcenter = true,
                0xFFFB => align = MesAlign::LineCenter,
                0xFFFC => align = MesAlign::BlockCenter,
                0xFFFD | 0xFFF9 | 0xFFF5 | 0xFFF6 => {
                    line_widths.push((cur_w - 2).max(0));
                    cur_w = 0;
                }
                _ => {}
            }
        }
        line_widths.push((cur_w - 2).max(0));

        let line_count = line_widths.len();
        let max_line_width = line_widths.iter().copied().max().unwrap_or(0);
        let line_h = draw_h as i32;

        // Auto Y from `fontPrintStrMain` (`0x001b4a10`):
        // vcenter: `256 - (line_count * line_h) / 2`; default subtitle: `476 - ((line_count + 4) * line_h) / 2`
        let start_y = match (anchor_y, vcenter) {
            (Some(y), true) => y - (line_count as i32 * line_h) / 2,
            (Some(y), false) => y,
            (None, true) => 256 - (line_count as i32 * line_h) / 2,
            (None, false) => 476 - ((line_count as i32 + 4) * line_h) / 2,
        };

        let line_start_x = |line_idx: usize| -> i32 {
            match align {
                MesAlign::BlockCenter => anchor_x - max_line_width / 2,
                MesAlign::LineCenter => {
                    anchor_x - line_widths.get(line_idx).copied().unwrap_or(0) / 2
                }
                MesAlign::Left => anchor_x,
            }
        };

        // Pass 2: emit positioned commands
        let mut commands = Vec::new();
        let mut line_idx = 0usize;
        let mut x = line_start_x(0);
        let mut y = start_y;
        let mut color_idx = 0u8;
        let mut blank_start_x: Option<i32> = None;
        let mut yes_no: Option<(i32, u8)> = None;
        let mut choice_bounds: Vec<(i32, i32)> = Vec::new();
        let mut default_selection = 0usize;

        for &code in &expanded {
            match code {
                0x0000 => x += (draw_w as i32 * 5) / (base_w as i32) + 2,
                0x0001..=0x00DF => {
                    if blank_start_x.is_none() {
                        commands.push(MesDrawCmd::Glyph {
                            code,
                            x,
                            y,
                            color_idx,
                            font_type: self.font_type,
                        });
                    }
                    x += (draw_w as i32 * 10) / (base_w as i32)
                        + 2
                        + i32::from(self.font_type == MesFontType::Small);
                }
                0x00E0..=0x7FFE => {
                    if blank_start_x.is_none() {
                        commands.push(MesDrawCmd::Glyph {
                            code,
                            x,
                            y,
                            color_idx,
                            font_type: self.font_type,
                        });
                    }
                    x += draw_w as i32 + 2;
                }
                0xFC00..=0xFCFF => x -= (code & 0xFF) as i32,
                0xFD00..=0xFDFF => x += (code & 0xFF) as i32,
                0xFE00..=0xFEFF => {
                    let w = code & 0xFF;
                    commands.push(MesDrawCmd::Rule {
                        x,
                        y,
                        width: w,
                        color_idx,
                    });
                    x += w as i32 + 2;
                }
                0xFF00..=0xFF63 => {
                    color_idx = ((code - 0xFF00) as u8).min(16);
                }
                0xFFF5 | 0xFFF6 => {
                    let def_idx = if code == 0xFFF5 { 1 } else { 0 };
                    y += line_h;
                    yes_no = Some((y, def_idx));
                    default_selection = def_idx as usize;
                    commands.push(MesDrawCmd::YesNoPrompt {
                        y,
                        yes_center_x: YES_NO_YES_CENTER_X,
                        no_center_x: YES_NO_NO_CENTER_X,
                        default_index: def_idx,
                    });
                    line_idx += 1;
                    x = line_start_x(line_idx);
                }
                0xFFF7 => {
                    if let Some(sx) = blank_start_x.take() {
                        commands.push(MesDrawCmd::BlankBox {
                            x0: sx,
                            x1: x - 2,
                            y,
                        });
                    } else {
                        blank_start_x = Some(x);
                    }
                }
                0xFFF8 => {
                    default_selection = choice_bounds.len();
                }
                0xFFF9 => {
                    y += line_h;
                    choice_bounds.push((y, y + line_h));
                    line_idx += 1;
                    x = line_start_x(line_idx);
                }
                0xFFFD => {
                    y += line_h;
                    line_idx += 1;
                    x = line_start_x(line_idx);
                }
                0xFFFE => x += draw_w as i32 + 2,
                _ => {}
            }
        }

        let is_final_page = (end_ctrl & 0x8000) != 0;
        let wait_mode = if yes_no.is_some() || !choice_bounds.is_empty() {
            MesWaitMode::InteractiveSelect
        } else {
            let n = end_ctrl & 0x0FFF;
            match (end_ctrl >> 12) & 7 {
                1 => MesWaitMode::WaitButton,
                2 => MesWaitMode::TimedUnskippable(n),
                3 => MesWaitMode::TimedSkippable(n),
                _ => MesWaitMode::None,
            }
        };

        MesLayoutResult {
            commands,
            line_count,
            max_line_width,
            is_final_page,
            wait_mode,
            yes_no,
            choice_bounds,
            default_selection,
        }
    }

    /// Expands `0x8000` byte-packed streams and 1-level `0xFFE0..0xFFE9` dynamic slots up to `0xFFFF`.
    fn expand_page_tokens(&self, tokens: &[u16]) -> (Vec<u16>, u16) {
        let mut out = Vec::with_capacity(tokens.len());
        let mut i = 0usize;
        let mut end_ctrl = 0x8000u16;

        while i < tokens.len() {
            let code = tokens[i];
            i += 1;
            if code == 0xFFFF {
                end_ctrl = tokens.get(i).copied().unwrap_or(0x8000);
                break;
            }
            if code == 0x8000 {
                // Byte-packed stream mode (`fontPrintStrMain` `0x001b4a10`)
                let mut bytes = Vec::with_capacity((tokens.len() - i) * 2);
                for &w in &tokens[i..] {
                    bytes.push((w & 0xFF) as u8);
                    bytes.push((w >> 8) as u8);
                }
                let mut bi = 0usize;
                while bi < bytes.len() {
                    let b = bytes[bi];
                    bi += 1;
                    let decoded = if b < 0xE0 {
                        b as u16
                    } else if b == 0xE0 && bi + 1 < bytes.len() {
                        let v = u16::from_le_bytes([bytes[bi], bytes[bi + 1]]);
                        bi += 2;
                        v
                    } else if bi < bytes.len() {
                        let v = ((b as u16) << 8) | (bytes[bi] as u16);
                        bi += 1;
                        v
                    } else {
                        0xFFFF
                    };
                    if decoded == 0xFFFF {
                        let aligned = (bi + 1) & !1;
                        if aligned + 1 < bytes.len() {
                            end_ctrl = u16::from_le_bytes([bytes[aligned], bytes[aligned + 1]]);
                        }
                        break;
                    }
                    self.push_or_expand_slot(decoded, &mut out);
                }
                break;
            }
            self.push_or_expand_slot(code, &mut out);
        }

        (out, end_ctrl)
    }

    fn push_or_expand_slot(&self, code: u16, out: &mut Vec<u16>) {
        if (0xFFE0..=0xFFE9).contains(&code) {
            let slot = (code - 0xFFE0) as usize;
            for &sub in &self.slots[slot] {
                if sub == 0xFFFF {
                    break;
                }
                out.push(sub);
            }
        } else {
            out.push(code);
        }
    }
}

fn parse_decimal(bytes: &[u8], i: &mut usize) -> u16 {
    let mut val = 0u16;
    while *i < bytes.len() && bytes[*i].is_ascii_digit() {
        val = val
            .saturating_mul(10)
            .saturating_add((bytes[*i] - b'0') as u16);
        *i += 1;
    }
    val
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_font_color_palette_ps2_vs_pc_drift() {
        assert_eq!(font_set_color(0, false), ([128, 128, 128], [128, 128, 128]));
        assert_eq!(font_set_color(7, false), ([80, 80, 160], [80, 160, 80]));
        // Index 10 drift: PS2 (144,144,144) vs Retail PC (128,128,128)
        assert_eq!(font_set_color(10, false), ([144, 144, 144], [32, 32, 32]));
        assert_eq!(font_set_color(10, true), ([128, 128, 128], [32, 32, 32]));
        assert_eq!(font_set_color(16, false), ([32, 32, 32], [32, 32, 32]));
    }

    #[test]
    fn test_dic_set_str_and_dynamic_slot_substitution() {
        let mut interp = MesInterpreter::new();
        interp.font_set_mes(0, MesInterpreter::dic_set_str("42"));
        let tokens = MesInterpreter::dic_set_str("\\c\\2Code: \\v0\\n\\ek");
        let res = interp.font_print_str_main(&tokens, None, None);

        assert!(res.is_final_page);
        assert_eq!(res.wait_mode, MesWaitMode::WaitButton);
        assert_eq!(res.line_count, 2);
        // Verify color 2 was applied to all emitted glyphs and '4','2' were substituted
        let glyphs: Vec<(u16, u8)> = res
            .commands
            .iter()
            .filter_map(|c| match *c {
                MesDrawCmd::Glyph {
                    code, color_idx, ..
                } => Some((code, color_idx)),
                _ => None,
            })
            .collect();
        assert!(glyphs.iter().all(|&(_, col)| col == 2));
        assert!(glyphs.iter().any(|&(c, _)| c == (b'4' - 0x20) as u16));
        assert!(glyphs.iter().any(|&(c, _)| c == (b'2' - 0x20) as u16));
    }

    #[test]
    fn test_yes_no_prompt_and_small_type1_font() {
        let mut interp = MesInterpreter::new();
        interp.font_type = MesFontType::Small;
        assert_eq!(interp.font_type.base_dimensions(), (16, 24));

        // 0xFFF5 = Yes/No defaulting to No (1)
        let tokens = [
            (b'O' - 0x20) as u16,
            (b'K' - 0x20) as u16,
            0xFFF5,
            0xFFFF,
            0x8000,
        ];
        let res = interp.font_print_str_main(&tokens, None, Some(100));
        assert_eq!(res.wait_mode, MesWaitMode::InteractiveSelect);
        assert_eq!(res.yes_no, Some((124, 1)));
        assert_eq!(res.default_selection, 1);
        assert_eq!(yes_no_bar_alpha_gs(0.0), 16);
        assert_eq!(yes_no_bar_alpha_gs(0.5), 80);
    }

    #[test]
    fn test_byte_packed_stream_mode_0x8000() {
        let interp = MesInterpreter::new();
        // Enter byte-packed mode (0x8000), emit 'A' (0x21), 'B' (0x22), then 0xFFFF, 0x9000 (final+WaitButton)
        let packed_words = [
            0x8000u16,
            u16::from_le_bytes([0x21, 0x22]),
            u16::from_le_bytes([0xFF, 0xFF]),
            0x9000u16,
        ];
        let res = interp.font_print_str_main(&packed_words, Some(50), Some(100));
        assert!(res.is_final_page);
        assert_eq!(res.wait_mode, MesWaitMode::WaitButton);
        assert_eq!(res.commands.len(), 2);
    }
}
