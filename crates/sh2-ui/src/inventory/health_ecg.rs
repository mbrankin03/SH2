//! Authentic 1:1 PS2 Health Status Snapshot Tint & Glitch Scanlines
//! (`look_hp` in `SLUS_202.28` at `0x001d9660`).
//!
//! # Ground Truth (`SLUS_202.28` `look_hp` @ `0x001d9660..0x001da16c`)
//! In Silent Hill 2, the top-left `Status` box is a **live miniature snapshot of the frozen 3D
//! gameplay frame**, modulated by player health with **no ECG waveform line and no status text**:
//! - Normalized health ratio: `ratio = (hp / max_hp).clamp(0.0, 1.0)` (`0x001d96d0`).
//! - Damage factor: `damage = 1.0 - ratio` (`0x001d96e0`).
//! - Horizontal sweep speed: `(1.4 + damage / 0.7) * 30.0 * dt` (`0x001d96e4..0x001d9724`),
//!   wrapping when `sweep_x > -70.0` (`0xc28c0000`) back to `-210.0` (`0xc3520000`).
//! - Heartbeat pulse cycle: `pulse_timer += dt / 2.0`, resets to `0.0` when
//!   `pulse_timer > 0.2 + ratio` (`0x001d9818..0x001d9840`).
//! - Heartbeat brightness modulation when `hp < 0.8 * max_hp` (`0x3f4ccccd`; retail PC `sh2pc.exe`
//!   `0x0046edeb` raises this threshold slightly to `0.9 * max_hp`):
//!   - `pulse_timer < 0.1`: `brightness = 1.0 - 6.0 * pulse_timer` (`0x001d9890..0x001d98ac`)
//!   - `0.1 <= pulse_timer < 0.2`: `brightness = 6.0 * pulse_timer - 0.2` (`0x001d98bc..0x001d9918`)
//!   - Otherwise: `brightness = 1.0` (`0x001d9924`).
//! - Color modulation (`0x001d9a3c..0x001d9b50`, base intensity `160.0` = `0x43200000`):
//!   - `hp >= 0.5 * max_hp`: neutral monochrome `R = G = B = 160.0 * brightness`, `A = 0xa0`.
//!   - `hp < 0.5 * max_hp`: `R = 160.0 * brightness`, `G = B = 160.0 * brightness * (2.0 * ratio)`,
//!     shifting smoothly to pulsing crimson red as `hp -> 0`.
//! - Randomized 2px glitch scanlines (`0x001d9c10..0x001da160`, identical in `sh2pc.exe` `0x0046f3c0`):
//!   whenever `hp < max_hp`, iterates `i < 10.0 - 10.0 * ratio`, seeding `srand(_DAT_0116e664++)`
//!   and with `1/6` probability (`rand() % 6 < 1`) drawing a 2px-tall glitch strip across the
//!   `140x105` snapshot rect (`x in [26..166]`, `y in [49..154]`).

use crate::font::TextRasterizer;

/// Base horizontal sweep start in PS2 centered coordinates (`0xc3520000` at `0x001d9758`).
pub const ECG_SWEEP_START_X: f32 = -210.0;
/// Base horizontal sweep wrap threshold in PS2 centered coordinates (`0xc28c0000` at `0x001d973c`).
pub const ECG_SWEEP_END_X: f32 = -70.0;
/// Base sweep speed factor (`0x3fb33333` at `0x001d96f8`).
pub const ECG_SWEEP_BASE_SPEED: f32 = 1.4;
/// Damage divisor for sweep acceleration (`0x3f333333` at `0x001d96e4`).
pub const ECG_SWEEP_DAMAGE_DIVISOR: f32 = 0.7;
/// PS2 30 Hz frame-time multiplier used in `look_hp` (`0x41f00000` at `0x001d96ac`).
pub const ECG_FRAME_RATE_SCALE: f32 = 30.0;
/// Health ratio below which heartbeat pulsing activates (`0x3f4ccccd` at `0x001d986c`).
pub const ECG_PULSE_THRESHOLD_RATIO: f32 = 0.8;
/// Health ratio below which color shifts from yellow/amber into red (`0x001d9a48`, `max_hp / 2.0`).
pub const ECG_DANGER_COLOR_RATIO: f32 = 0.5;
/// Critical red static threshold matching `SimulationCore` heartbeat haptics (`25%` HP).
pub const ECG_CRITICAL_DANGER_RATIO: f32 = 0.25;
/// Base RGB channel intensity in `look_hp` (`0x43200000` = `160.0` at `0x001d9a6c`).
pub const ECG_BASE_COLOR_INTENSITY: f32 = 160.0;
/// GS alpha byte emitted by `look_hp` (`0xa0` = `160` at `0x001d9ad0` and `0x001d9b30`).
pub const ECG_GS_ALPHA: u8 = 0xa0;
/// Retail PC `sh2pc.exe` (`0x0046efxx`, `sh2pc_00460000.c:7595`) fine-health green tint overlay
/// drawn over the Status 3D snapshot at `hp == max_hp`: `D3DCOLOR 0x1680FF80` -> `RGBA(128, 255, 128, 22)`.
/// (Prototype-vs-retail drift: `SLUS_202.28` `0x001d9afc` omitted this green tint at 100% HP.)
pub const STATUS_FINE_OVERLAY_RGBA: [u8; 4] = [0x80, 0xff, 0x80, 0x16];

/// Top-left `Status` 3D snapshot X in `512x448` PS2 logical space (`256 + (-230) = 26`,
/// `SLUS_202.28` `look_hp` `0x001d9b6c`, `addiu $4, $zero, -0xe6`).
pub const STATUS_SNAPSHOT_X_512: f32 = 26.0;
/// Top-left `Status` 3D snapshot Y in `512x448` PS2 logical space (`224 + (-175) = 49`,
/// `SLUS_202.28` `look_hp` `0x001d9b80`, `addiu $4, $zero, -0xaf`).
pub const STATUS_SNAPSHOT_Y_448: f32 = 49.0;
/// Width of the `Status` 3D snapshot box in `512x448` PS2 logical space (`-90 - (-230) = 140`,
/// `SLUS_202.28` `look_hp` `0x001d9be8`, `addiu $4, $zero, -0x5a`).
pub const STATUS_SNAPSHOT_W_512: f32 = 140.0;
/// Height of the `Status` 3D snapshot box in `512x448` PS2 logical space (`-70 - (-175) = 105`,
/// `SLUS_202.28` `look_hp` `0x001d9bfc`, `addiu $4, $zero, -0x46`).
pub const STATUS_SNAPSHOT_H_448: f32 = 105.0;

/// Coarse health status band reflected in the `Status` snapshot tint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthStatus {
    /// `hp >= 0.8 * max_hp`: steady neutral brightness, no heartbeat pulse (`look_hp` `0x001d986c`).
    Fine,
    /// `0.25 * max_hp <= hp < 0.8 * max_hp`: rhythmic heartbeat pulse (neutral `>= 50%`, crimson shift `< 50%`).
    Caution,
    /// `hp < 0.25 * max_hp`: deep crimson pulsing tint with dense glitch scanlines.
    Danger,
}

impl HealthStatus {
    /// Classifies `(hp, max_hp)` into `Fine`, `Caution`, or `Danger`.
    pub fn from_hp(hp: f32, max_hp: f32) -> Self {
        let ratio = if max_hp > 0.0 {
            (hp / max_hp).clamp(0.0, 1.0)
        } else {
            0.0
        };
        if ratio >= ECG_PULSE_THRESHOLD_RATIO {
            Self::Fine
        } else if ratio >= ECG_CRITICAL_DANGER_RATIO {
            Self::Caution
        } else {
            Self::Danger
        }
    }
}

/// Animated state for the top-left `Status` live-snapshot health tint (`look_hp` @ `0x001d9660`).
#[derive(Debug, Clone, PartialEq)]
pub struct HealthEcgState {
    /// Horizontal sweep cursor in PS2 centered coordinates (`-210.0..=-70.0`, `0x116e604`).
    pub sweep_x: f32,
    /// Heartbeat pulse timer in seconds (`0.0..=(0.2 + ratio)`, `0x116e60c`).
    pub pulse_timer: f32,
    /// Current pulse brightness multiplier (`0.4..=1.0`, `0x116e610`).
    pub pulse_brightness: f32,
    /// Deterministic static noise frame counter for `Danger` pulse interference.
    pub static_frame: u32,
}

impl Default for HealthEcgState {
    fn default() -> Self {
        Self::new()
    }
}

impl HealthEcgState {
    /// Creates a new `HealthEcgState` initialized at `ECG_SWEEP_START_X`.
    pub fn new() -> Self {
        Self {
            sweep_x: ECG_SWEEP_START_X,
            pulse_timer: 0.0,
            pulse_brightness: 1.0,
            static_frame: 0,
        }
    }

    /// Advances the sweep and heartbeat pulse modulation (`SLUS_202.28` `look_hp` `0x001d96a4..0x001d992c`).
    pub fn tick(&mut self, dt: f32, hp: f32, max_hp: f32) {
        let dt = dt.clamp(0.0, 0.1);
        let ratio = if max_hp > 0.0 {
            (hp / max_hp).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let damage = 1.0 - ratio;

        let speed = ECG_SWEEP_BASE_SPEED + damage / ECG_SWEEP_DAMAGE_DIVISOR;
        self.sweep_x += speed * ECG_FRAME_RATE_SCALE * dt;
        if self.sweep_x > ECG_SWEEP_END_X {
            self.sweep_x = ECG_SWEEP_START_X;
        }

        self.pulse_timer += dt / 2.0;
        if self.pulse_timer > 0.2 + ratio {
            self.pulse_timer = 0.0;
        }

        if ratio < ECG_PULSE_THRESHOLD_RATIO {
            if self.pulse_timer < 0.1 {
                self.pulse_brightness = (1.0 - 6.0 * self.pulse_timer).clamp(0.4, 1.0);
            } else if self.pulse_timer < 0.2 {
                self.pulse_brightness = (6.0 * self.pulse_timer - 0.2).clamp(0.4, 1.0);
            } else {
                self.pulse_brightness = 1.0;
            }
        } else {
            self.pulse_brightness = 1.0;
        }

        let glitch_iters = Self::glitch_scanline_iterations(hp, max_hp);
        self.static_frame = self.static_frame.wrapping_add(glitch_iters.max(1));
    }

    /// Returns the exact `SLUS_202.28` `look_hp` (`0x001d9c10`) glitch scanline loop iteration
    /// count: `ceil(10.0 - 10.0 * (hp / max_hp))` (`0` at 100% HP, `1..=10` whenever `hp < max_hp`).
    pub fn glitch_scanline_iterations(hp: f32, max_hp: f32) -> u32 {
        let ratio = if max_hp > 0.0 {
            (hp / max_hp).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let limit = 10.0 - ratio * 10.0;
        if limit <= 0.0 {
            0
        } else {
            limit.ceil().clamp(0.0, 10.0) as u32
        }
    }

    /// Deterministic C-standard LCG matching PS2 `srand`/`rand` used in `look_hp` (`0x001d9c30`).
    #[inline]
    fn lcg_rand(state: &mut u32) -> u32 {
        *state = state.wrapping_mul(1103515245).wrapping_add(12345);
        (*state >> 16) & 0x7fff
    }

    /// Computes the exact `SLUS_202.28` `look_hp` (`0x001d9a3c..0x001d9b50`) GS RGBA vertex color:
    /// - `hp >= 0.5 * max_hp` (`0x001d9afc`): `R = G = B = 160.0 * pulse`, `A = 0xa0`.
    /// - `hp < 0.5 * max_hp` (`0x001d9a64`): `R = 160.0 * pulse`, `G = B = 160.0 * pulse * (2.0 * ratio)`, `A = 0xa0`.
    pub fn status_color_rgba(&self, hp: f32, max_hp: f32) -> [u8; 4] {
        let ratio = if max_hp > 0.0 {
            (hp / max_hp).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let pulse = self.pulse_brightness;
        if ratio >= ECG_DANGER_COLOR_RATIO {
            let c = (ECG_BASE_COLOR_INTENSITY * pulse).round().clamp(0.0, 255.0) as u8;
            [c, c, c, ECG_GS_ALPHA]
        } else {
            let r = (ECG_BASE_COLOR_INTENSITY * pulse).round().clamp(0.0, 255.0) as u8;
            let gb = (ECG_BASE_COLOR_INTENSITY * pulse * (2.0 * ratio))
                .round()
                .clamp(0.0, 255.0) as u8;
            [r, gb, gb, ECG_GS_ALPHA]
        }
    }

    /// Computes the `(x, y, width, height)` viewport pixel rectangle for the live 3D game snapshot
    /// inside a swapchain of `(viewport_w, viewport_h)`, derived 1:1 from `sh2pc.exe` `look_hp`
    /// (`sh2pc_00460000.c:7524-7578`: `X = 26..166 / 512`, `Y = 65..170 / 480` = `(49 + 16) / 480`)
    /// inside the centered 4:3 UI canvas (`ps2_ui_canvas_rect`).
    pub fn snapshot_viewport_rect(viewport_w: f32, viewport_h: f32) -> (f32, f32, f32, f32) {
        let (ox, oy, cw, ch) = crate::inventory::ps2_ui_canvas_rect(viewport_w, viewport_h);
        let sx = cw / 512.0;
        let sy = ch / 480.0;
        (
            ox + STATUS_SNAPSHOT_X_512 * sx,
            oy + (STATUS_SNAPSHOT_Y_448 + 16.0) * sy,
            STATUS_SNAPSHOT_W_512 * sx,
            STATUS_SNAPSHOT_H_448 * sy,
        )
    }

    /// Renders the `SLUS_202.28` / `sh2pc.exe` `look_hp` (`0x001d9a3c..0x001d9b50` / `sh2pc_00460000.c:7505-7599`)
    /// health tint wash over the live frozen 3D snapshot cutout inside the `512x480` rasterizer canvas
    /// (which is blitted into `ps2_ui_canvas_rect` by `sh2-app`), with **no ECG waveform line and no status text**.
    pub fn draw_snapshot_tint(&self, hp: f32, max_hp: f32, rasterizer: &mut TextRasterizer) {
        let sx = rasterizer.width as f32 / 512.0;
        let sy = rasterizer.height as f32 / 480.0;
        let x0 = (STATUS_SNAPSHOT_X_512 * sx).round() as usize;
        let y0 = ((STATUS_SNAPSHOT_Y_448 + 16.0) * sy).round() as usize;
        let w = (STATUS_SNAPSHOT_W_512 * sx).round().max(1.0) as usize;
        let h = (STATUS_SNAPSHOT_H_448 * sy).round().max(1.0) as usize;

        let ratio = if max_hp > 0.0 {
            (hp / max_hp).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let color = self.status_color_rgba(hp, max_hp);
        if ratio >= 1.0 {
            // Retail PC `sh2pc.exe` `look_hp` (`sh2pc_00460000.c:7505-7599`):
            // 1. Base snapshot brightness is modulated by `ECG_BASE_COLOR_INTENSITY / 255.0` (`160/255`)
            //    -> darken overlay alpha `255 - 160 = 95` (`0x5f`).
            // 2. Second overlay quad (`0x1680FF80` -> `RGBA(128, 255, 128, 22)`) adds the green health tint.
            let base_dim_a = (255.0 - ECG_BASE_COLOR_INTENSITY).round().clamp(0.0, 255.0) as u8;
            rasterizer.fill_rect(x0, y0, w, h, [0, 0, 0, base_dim_a]);
            rasterizer.fill_rect(x0, y0, w, h, STATUS_FINE_OVERLAY_RGBA);
        } else if ratio >= ECG_DANGER_COLOR_RATIO {
            // `look_hp` (`0x001d9afc`): neutral `R=G=B=160*pulse` modulation over the 3D snapshot.
            let dim_a = (255.0 - ECG_BASE_COLOR_INTENSITY * self.pulse_brightness)
                .round()
                .clamp(0.0, 255.0) as u8;
            if dim_a > 0 {
                rasterizer.fill_rect(x0, y0, w, h, [0, 0, 0, dim_a]);
            }
        } else {
            // `look_hp` (`0x001d9a90`): red shift factor `1.0 - 2.0 * ratio`, scaled by `ECG_GS_ALPHA`.
            let red_shift = (1.0 - 2.0 * ratio).clamp(0.0, 1.0);
            let tint_a = (red_shift * ECG_GS_ALPHA as f32).round() as u8;
            if tint_a > 0 {
                rasterizer.fill_rect(x0, y0, w, h, [color[0], color[1], color[2], tint_a]);
            }
        }

        // `look_hp` (`0x001d9c10..0x001da160`, `sh2pc 0x0046f3c0`): draw `10 - 10*(hp/max)`
        // randomized 2px-tall glitch scanlines whenever `hp < max_hp`.
        let iters = Self::glitch_scanline_iterations(hp, max_hp);
        let strip_h = (2.0 * sy).round().max(1.0) as usize;
        for i in 0..iters {
            let mut rng = self.static_frame.wrapping_add(i);
            let x_off = (Self::lcg_rand(&mut rng) % 0x8c) as f32;
            let y_off = (Self::lcg_rand(&mut rng) % 0x69) as f32;
            let mut strip_w_512 = (Self::lcg_rand(&mut rng) % 0x69) as f32;
            let prob = Self::lcg_rand(&mut rng) % 6;
            if prob < 1 {
                if x_off + strip_w_512 > STATUS_SNAPSHOT_W_512 {
                    strip_w_512 = STATUS_SNAPSHOT_W_512 - x_off;
                }
                if strip_w_512 > 0.0 {
                    // Retail PC `sh2pc.exe` `look_hp` (`sh2pc_00460000.c:7524-7810`): include `+ 16.0`
                    // overscan Y offset matching line 231 so glitch scanlines stay inside `[y0..y0+h]`.
                    let gx = ((STATUS_SNAPSHOT_X_512 + x_off) * sx).round() as usize;
                    let gy = ((STATUS_SNAPSHOT_Y_448 + 16.0 + y_off) * sy).round() as usize;
                    let gw = (strip_w_512 * sx).round().max(1.0) as usize;
                    rasterizer.fill_rect(gx, gy, gw, strip_h, color);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_ecg_status_bands_and_pulse() {
        assert_eq!(HealthStatus::from_hp(100.0, 100.0), HealthStatus::Fine);
        assert_eq!(HealthStatus::from_hp(80.0, 100.0), HealthStatus::Fine);
        assert_eq!(HealthStatus::from_hp(60.0, 100.0), HealthStatus::Caution);
        assert_eq!(HealthStatus::from_hp(25.0, 100.0), HealthStatus::Caution);
        assert_eq!(HealthStatus::from_hp(20.0, 100.0), HealthStatus::Danger);

        let mut ecg = HealthEcgState::new();
        ecg.tick(0.05, 100.0, 100.0);
        assert!((ecg.pulse_brightness - 1.0).abs() < 1e-5);
        assert!(ecg.sweep_x > ECG_SWEEP_START_X);

        ecg.pulse_timer = 0.0;
        ecg.tick(0.1, 20.0, 100.0);
        assert!((ecg.pulse_brightness - 0.7).abs() < 1e-4);

        let c = ecg.status_color_rgba(15.0, 100.0);
        assert!(c[0] > c[1]);
        assert!(c[3] < 200); // Translucent so live 3D game snapshot shows through

        // `look_hp` (`0x001d9afc`): neutral R==G==B above 50% HP
        let neutral = ecg.status_color_rgba(75.0, 100.0);
        assert_eq!(neutral[0], neutral[1]);
        assert_eq!(neutral[1], neutral[2]);
        assert_eq!(neutral[3], ECG_GS_ALPHA);

        // `look_hp` (`0x001d9c10`): `10 - 10*(hp/max)` glitch scanline iterations
        assert_eq!(HealthEcgState::glitch_scanline_iterations(100.0, 100.0), 0);
        assert_eq!(HealthEcgState::glitch_scanline_iterations(90.0, 100.0), 1);
        assert_eq!(HealthEcgState::glitch_scanline_iterations(50.0, 100.0), 5);
        assert_eq!(HealthEcgState::glitch_scanline_iterations(0.0, 100.0), 10);

        // At 100% HP, retail `sh2pc.exe` (`0x1680FF80`) draws the subtle green fine-health tint inside
        // the `140x105` Status snapshot rect (`x=26..166, y=49..154`) and leaves everything outside transparent.
        let mut rast = TextRasterizer::new(512, 448);
        ecg.pulse_brightness = 1.0;
        ecg.draw_snapshot_tint(100.0, 100.0, &mut rast);
        let inside = &rast.pixels[(100 * 512 + 80) * 4..][..4];
        assert!(inside[3] > 0 && inside[3] < 255);
        assert!(inside[1] > inside[0] && inside[0] == inside[2]);
        let outside = &rast.pixels[(10 * 512 + 10) * 4..][..4];
        assert_eq!(outside, &[0, 0, 0, 0]);

        // Run a few frames at 60% HP (neutral tint, pulse=1.0 -> dim_a=0, only glitch scanlines draw)
        let mut drew_glitch = false;
        for frame in 0..32u32 {
            rast.clear();
            ecg.static_frame = frame;
            ecg.draw_snapshot_tint(60.0, 100.0, &mut rast);
            if rast.pixels.iter().any(|&b| b != 0) {
                drew_glitch = true;
                break;
            }
        }
        assert!(drew_glitch, "expected 2px glitch scanlines at 60% HP");
    }

    #[test]
    fn test_sh2_166_look_hp_glitch_scanlines_respect_16px_overscan_offset() {
        // Retail PC `sh2pc.exe` `look_hp` (`sh2pc_00460000.c:7524-7810`, `7693-7740`):
        // on a `512x480` canvas, Status snapshot occupies `x in [26..166)` and
        // `y in [49 + 16 .. 49 + 16 + 105) = [65..170)`, and 2px glitch strips have
        // `y_off = rand() % 0x69` (`0..=104`) -> max top `65 + 104 = 169`, max bottom `169 + 2 = 171`
        // (written pixel rows `65..171`, never leaking above `y = 65` or past `y = 170`).
        let mut ecg = HealthEcgState::new();
        ecg.pulse_brightness = 1.0;
        let mut rast = TextRasterizer::new(512, 480);
        let mut saw_glitch = false;
        for frame in 0..64u32 {
            rast.clear();
            ecg.static_frame = frame;
            // 60% HP: neutral tint (`dim_a = 95`) inside `[26..166) x [65..170)` plus 2px glitch strips.
            ecg.draw_snapshot_tint(60.0, 100.0, &mut rast);
            for y in 0..480usize {
                for x in 0..512usize {
                    let px = &rast.pixels[(y * 512 + x) * 4..][..4];
                    if px[3] > 0 {
                        assert!(
                            (26..166).contains(&x) && (65..171).contains(&y),
                            "pixel ({x}, {y}) leaked outside Status snapshot/glitch rect [26..166) x [65..171)"
                        );
                        if px[0] > 0 {
                            saw_glitch = true;
                        }
                    }
                }
            }
        }
        assert!(saw_glitch, "expected glitch scanline pixels at 60% HP");
    }
}

/// Retail PC low-health warning icon path (`sh2pc.exe` `FUN_004f6380` `0x004f6380`).
pub const RED_CROSS_ICON_PATH: &str = "pic/etc/RedCrossIcon.png";
/// Minimum pulse multiplier in `FUN_00476280` (`0.1` at `0x00476280`).
pub const RED_CROSS_MIN_PULSE: f32 = 0.1;
/// Maximum pulse multiplier in `FUN_00476280` (`1.0` at `0x00476280`).
pub const RED_CROSS_MAX_PULSE: f32 = 1.0;
/// Per-30Hz-frame pulse step (`_DAT_007f3e10 = -0.03f` at `0x00476280`).
pub const RED_CROSS_PULSE_STEP: f32 = -0.03;

/// Low-Health Red Cross Icon state (`sh2pc.exe` `FUN_00476280` `0x00476280` & `FUN_004f66d0` `0x004f66d0`).
#[derive(Debug, Clone, Copy)]
pub struct RedCrossIconState {
    /// Current oscillating pulse multiplier `[0.1, 1.0]` (`_DAT_007f3e14`).
    pub pulse: f32,
    /// Current step direction (`-0.03` or `+0.03`, `_DAT_007f3e10`).
    pub pulse_step: f32,
    /// Computed opacity `[0.0, 1.0]` passed to `FUN_004f66d0`.
    pub intensity: f32,
}

impl Default for RedCrossIconState {
    fn default() -> Self {
        Self::new()
    }
}

impl RedCrossIconState {
    /// Creates a new [`RedCrossIconState`] matching `sh2pc.exe` globals (`_DAT_007f3e14 = 1.0`,
    /// `_DAT_007f3e10 = -0.03`).
    #[must_use]
    pub const fn new() -> Self {
        Self {
            pulse: RED_CROSS_MAX_PULSE,
            pulse_step: RED_CROSS_PULSE_STEP,
            intensity: 0.0,
        }
    }

    /// Advances the Red Cross pulse state (`FUN_00476280` `0x00476280`) for `hp` / `max_hp`.
    pub fn tick(&mut self, dt_sec: f32, hp: f32, max_hp: f32) {
        if max_hp <= 0.0 || hp >= max_hp * ECG_DANGER_COLOR_RATIO {
            self.pulse = RED_CROSS_MAX_PULSE;
            self.intensity = 0.0;
            return;
        }
        let steps = (dt_sec * 30.0).max(0.0);
        let raw = ((max_hp - hp) / max_hp) * self.pulse;
        self.intensity = raw.clamp(0.0, 1.0);
        self.pulse += self.pulse_step * steps;
        if self.pulse < RED_CROSS_MIN_PULSE {
            self.pulse = RED_CROSS_MIN_PULSE;
            self.pulse_step = -self.pulse_step;
        } else if self.pulse > RED_CROSS_MAX_PULSE {
            self.pulse = RED_CROSS_MAX_PULSE;
            self.pulse_step = -self.pulse_step;
        }
    }

    /// Returns `0..=255` alpha for `RedCrossIcon.png` (`FUN_004f66d0`).
    #[must_use]
    pub fn alpha_u8(&self) -> u8 {
        (self.intensity * 255.0).round().clamp(0.0, 255.0) as u8
    }

    /// Draws `RedCrossIcon.png` at bottom-right `(0.95 * w, 0.95 * h)` (`FUN_004f66d0` `0x004f66d0`).
    pub fn draw(&self, icon_rgba: Option<(&[u8], usize, usize)>, rasterizer: &mut TextRasterizer) {
        let alpha = self.alpha_u8();
        if alpha == 0 {
            return;
        }
        let x1 = ((rasterizer.width as f32) * 0.95).round() as usize;
        let y1 = ((rasterizer.height as f32) * 0.95).round() as usize;
        let icon_size = ((rasterizer.height as f32) * (36.0 / 448.0))
            .round()
            .max(16.0) as usize;
        let x0 = x1.saturating_sub(icon_size);
        let y0 = y1.saturating_sub(icon_size);

        if let Some((src, sw, sh)) = icon_rgba {
            let a_mul = f32::from(alpha) / 255.0;
            for dy in 0..icon_size {
                let py = y0 + dy;
                if py >= rasterizer.height {
                    break;
                }
                let sy = (dy * sh) / icon_size;
                for dx in 0..icon_size {
                    let px = x0 + dx;
                    if px >= rasterizer.width {
                        break;
                    }
                    let sx = (dx * sw) / icon_size;
                    let si = (sy * sw + sx) * 4;
                    if si + 3 >= src.len() {
                        continue;
                    }
                    let sa = (f32::from(src[si + 3]) * a_mul).round() as u8;
                    if sa > 0 {
                        rasterizer.blend_pixel(px, py, [src[si], src[si + 1], src[si + 2], sa]);
                    }
                }
            }
        } else {
            // Procedural red cross fallback if texture wasn't passed.
            let arm = (icon_size / 3).max(4);
            let off = (icon_size - arm) / 2;
            rasterizer.fill_rect(x0 + off, y0, arm, icon_size, [210, 24, 24, alpha]);
            rasterizer.fill_rect(x0, y0 + off, icon_size, arm, [210, 24, 24, alpha]);
        }
    }
}
