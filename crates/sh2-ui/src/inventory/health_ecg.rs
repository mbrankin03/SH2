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
    /// inside a swapchain of `(viewport_w, viewport_h)`, derived 1:1 from `SLUS_202.28` `look_hp`
    /// (`0x001d9b6c..0x001d9bfc`) inside the centered 4:3 PS2 UI canvas (`ps2_ui_canvas_rect`).
    pub fn snapshot_viewport_rect(viewport_w: f32, viewport_h: f32) -> (f32, f32, f32, f32) {
        let (ox, oy, cw, ch) = crate::inventory::ps2_ui_canvas_rect(viewport_w, viewport_h);
        let sx = cw / 512.0;
        let sy = ch / 448.0;
        (
            ox + STATUS_SNAPSHOT_X_512 * sx,
            oy + STATUS_SNAPSHOT_Y_448 * sy,
            STATUS_SNAPSHOT_W_512 * sx,
            STATUS_SNAPSHOT_H_448 * sy,
        )
    }

    /// Renders the `SLUS_202.28` `look_hp` (`0x001d9a3c..0x001d9b50`) health tint wash over the live
    /// frozen 3D snapshot cutout inside the full `512x448` rasterizer canvas (which is blitted into
    /// `ps2_ui_canvas_rect` by `sh2-app`), with **no ECG waveform line and no status text**.
    pub fn draw_snapshot_tint(&self, hp: f32, max_hp: f32, rasterizer: &mut TextRasterizer) {
        let sx = rasterizer.width as f32 / 512.0;
        let sy = rasterizer.height as f32 / 448.0;
        let x0 = (STATUS_SNAPSHOT_X_512 * sx).round() as usize;
        let y0 = (STATUS_SNAPSHOT_Y_448 * sy).round() as usize;
        let w = (STATUS_SNAPSHOT_W_512 * sx).round().max(1.0) as usize;
        let h = (STATUS_SNAPSHOT_H_448 * sy).round().max(1.0) as usize;

        let ratio = if max_hp > 0.0 {
            (hp / max_hp).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let color = self.status_color_rgba(hp, max_hp);
        if ratio >= ECG_DANGER_COLOR_RATIO {
            // `look_hp` (`0x001d9afc`): neutral `R=G=B=160*pulse`; when `pulse < 1.0` (`hp < 0.8`),
            // dim the snapshot proportionally to `(1.0 - pulse) * ECG_GS_ALPHA`.
            let dim_a =
                ((1.0 - self.pulse_brightness).clamp(0.0, 1.0) * ECG_GS_ALPHA as f32).round() as u8;
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
                    let gx = ((STATUS_SNAPSHOT_X_512 + x_off) * sx).round() as usize;
                    let gy = ((STATUS_SNAPSHOT_Y_448 + y_off) * sy).round() as usize;
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

        // At 100% HP, zero glitch pixels are drawn; below 100% HP, glitch scanlines appear
        let mut rast = TextRasterizer::new(512, 448);
        ecg.pulse_brightness = 1.0;
        ecg.draw_snapshot_tint(100.0, 100.0, &mut rast);
        assert!(rast.pixels.iter().all(|&b| b == 0));

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
}
