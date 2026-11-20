//! Authentic 1:1 PS2 Health ECG Status Box (`look_hp` in `SLUS_202.28` at `0x001d9660`).
//!
//! # Ground Truth (`SLUS_202.28` `look_hp` @ `0x001d9660..0x001d9be0`)
//! - Normalized health ratio: `ratio = (hp / max_hp).clamp(0.0, 1.0)` (`0x001d96d0`).
//! - Damage factor: `damage = 1.0 - ratio` (`0x001d96e0`).
//! - Horizontal sweep speed: `(1.4 + damage / 0.7) * 30.0 * dt` (`0x001d96e4..0x001d9724`),
//!   wrapping when `sweep_x > -70.0` (`0xc28c0000`) back to `-210.0` (`0xc3520000`).
//! - Heartbeat pulse cycle: `pulse_timer += dt / 2.0`, resets to `0.0` when
//!   `pulse_timer > 0.2 + ratio` (`0x001d9818..0x001d9840`).
//! - Heartbeat flash brightness modulation when `hp < 0.8 * max_hp` (`0x3f4ccccd`):
//!   - `pulse_timer < 0.1`: `brightness = 1.0 - 6.0 * pulse_timer` (`0x001d9890..0x001d98ac`)
//!   - `0.1 <= pulse_timer < 0.2`: `brightness = 6.0 * pulse_timer - 0.2` (`0x001d98bc..0x001d9918`)
//!   - Otherwise: `brightness = 1.0` (`0x001d9924`).
//! - Color shift (`0x001d9a3c..0x001d9b50`, base intensity `160.0` = `0x43200000`):
//!   - `hp >= 0.5 * max_hp`: Fine green / healthy (`Fine`, or light `Caution` when `hp < 0.8 * max_hp`).
//!   - `hp < 0.5 * max_hp`: `R = 160.0 * brightness`, `G = B = 160.0 * brightness * (2.0 * ratio)`,
//!     smoothly shifting from yellow/orange (`Caution`) to pure red (`Danger`) as `hp -> 0`.
//! - Top-left anchor in PS2 centered 640x480 space: `(-230, -175)` (`0x001d9b6c..0x001d9b84`),
//!   equivalent to `(90, 65)` in 640x480 top-left coordinates.

use crate::atlas_font::{AtlasFont, Sh2TextColor};
use crate::font::TextRasterizer;

/// Base ECG horizontal sweep start in PS2 centered coordinates (`0xc3520000` at `0x001d9758`).
pub const ECG_SWEEP_START_X: f32 = -210.0;
/// Base ECG horizontal sweep wrap threshold in PS2 centered coordinates (`0xc28c0000` at `0x001d973c`).
pub const ECG_SWEEP_END_X: f32 = -70.0;
/// Base ECG horizontal sweep speed factor (`0x3fb33333` at `0x001d96f8`).
pub const ECG_SWEEP_BASE_SPEED: f32 = 1.4;
/// Damage divisor for ECG sweep acceleration (`0x3f333333` at `0x001d96e4`).
pub const ECG_SWEEP_DAMAGE_DIVISOR: f32 = 0.7;
/// PS2 30 Hz frame-time multiplier used in `look_hp` (`0x41f00000` at `0x001d96ac`).
pub const ECG_FRAME_RATE_SCALE: f32 = 30.0;
/// Health ratio below which heartbeat pulsing activates (`0x3f4ccccd` at `0x001d986c`).
pub const ECG_PULSE_THRESHOLD_RATIO: f32 = 0.8;
/// Health ratio below which color shifts from yellow/orange into red (`0x001d9a48`, `max_hp / 2.0`).
pub const ECG_DANGER_COLOR_RATIO: f32 = 0.5;
/// Critical red static threshold matching `SimulationCore` heartbeat haptics (`25%` HP).
pub const ECG_CRITICAL_DANGER_RATIO: f32 = 0.25;
/// Base RGB channel intensity in `look_hp` (`0x43200000` = `160.0` at `0x001d9a6c`).
pub const ECG_BASE_COLOR_INTENSITY: f32 = 160.0;
/// Top-left X coordinate of the ECG box in 640x480 logical space (`320 - 230 = 90`, `0x001d9b6c`).
pub const ECG_BOX_X_640: f32 = 90.0;
/// Top-left Y coordinate of the ECG box in 640x480 logical space (`240 - 175 = 65`, `0x001d9b80`).
pub const ECG_BOX_Y_480: f32 = 65.0;
/// Width of the ECG box in 640x480 logical space (`-70 - (-210) = 140`, `0x001d973c..0x001d9758`).
pub const ECG_BOX_W_640: f32 = 140.0;
/// Height of the ECG box in 640x480 logical space.
pub const ECG_BOX_H_480: f32 = 76.0;

/// Coarse health status band shown in the Status / Inventory screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthStatus {
    /// `hp >= 0.8 * max_hp`: steady green monitor, no pulse flash (`look_hp` `0x001d986c`).
    Fine,
    /// `0.25 * max_hp <= hp < 0.8 * max_hp`: yellow/orange monitor with rhythmic pulse.
    Caution,
    /// `hp < 0.25 * max_hp`: red static pulse monitor.
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

    /// Display label for the health status band.
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Fine => "FINE",
            Self::Caution => "CAUTION",
            Self::Danger => "DANGER",
        }
    }
}

/// Animated state for the top-left Health ECG Status Box (`look_hp` @ `0x001d9660`).
#[derive(Debug, Clone, PartialEq)]
pub struct HealthEcgState {
    /// Horizontal sweep cursor in PS2 centered coordinates (`-210.0..=-70.0`, `0x116e604`).
    pub sweep_x: f32,
    /// Heartbeat pulse timer in seconds (`0.0..=(0.2 + ratio)`, `0x116e60c`).
    pub pulse_timer: f32,
    /// Current pulse brightness multiplier (`0.4..=1.0`, `0x116e610`).
    pub pulse_brightness: f32,
    /// Deterministic static noise frame counter for `Danger` red static pulse.
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

    /// Advances the ECG sweep and heartbeat pulse modulation (`SLUS_202.28` `look_hp` `0x001d96a4..0x001d992c`).
    pub fn tick(&mut self, dt: f32, hp: f32, max_hp: f32) {
        let dt = dt.clamp(0.0, 0.1);
        let ratio = if max_hp > 0.0 {
            (hp / max_hp).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let damage = 1.0 - ratio;

        // Horizontal sweep (`0x001d96e4..0x001d9760`)
        let speed = ECG_SWEEP_BASE_SPEED + damage / ECG_SWEEP_DAMAGE_DIVISOR;
        self.sweep_x += speed * ECG_FRAME_RATE_SCALE * dt;
        if self.sweep_x > ECG_SWEEP_END_X {
            self.sweep_x = ECG_SWEEP_START_X;
        }

        // Pulse cycle (`0x001d97ec..0x001d9840`)
        self.pulse_timer += dt / 2.0;
        if self.pulse_timer > 0.2 + ratio {
            self.pulse_timer = 0.0;
        }

        // Pulse brightness modulation when `hp < 0.8 * max_hp` (`0x001d9844..0x001d992c`)
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

        self.static_frame = self.static_frame.wrapping_add(1);
    }

    /// Normalized sweep progress `0.0..=1.0` across the ECG box.
    pub fn sweep_normalized(&self) -> f32 {
        ((self.sweep_x - ECG_SWEEP_START_X) / (ECG_SWEEP_END_X - ECG_SWEEP_START_X)).clamp(0.0, 1.0)
    }

    /// Computes the authentic RGBA tint for the Health ECG status box (`look_hp` `0x001d9a3c..0x001d9b50`).
    pub fn status_color_rgba(&self, hp: f32, max_hp: f32) -> [u8; 4] {
        let ratio = if max_hp > 0.0 {
            (hp / max_hp).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let pulse = self.pulse_brightness;
        if ratio >= ECG_PULSE_THRESHOLD_RATIO {
            // Fine: authentic Silent Hill 2 soft CRT green
            let g = (ECG_BASE_COLOR_INTENSITY * pulse).round().clamp(0.0, 255.0) as u8;
            let r = (g as f32 * 0.28).round() as u8;
            let b = (g as f32 * 0.38).round() as u8;
            [r, g, b, 235]
        } else if ratio >= ECG_DANGER_COLOR_RATIO {
            // Upper Caution (50%..80%): green -> yellow-amber shift
            let t = (ratio - ECG_DANGER_COLOR_RATIO)
                / (ECG_PULSE_THRESHOLD_RATIO - ECG_DANGER_COLOR_RATIO);
            let r = (ECG_BASE_COLOR_INTENSITY * pulse * (1.0 - 0.55 * t))
                .round()
                .clamp(0.0, 255.0) as u8;
            let g = (ECG_BASE_COLOR_INTENSITY * pulse).round().clamp(0.0, 255.0) as u8;
            let b = (ECG_BASE_COLOR_INTENSITY * pulse * 0.18)
                .round()
                .clamp(0.0, 255.0) as u8;
            [r, g, b, 235]
        } else {
            // Lower Caution / Danger (< 50%): exact `look_hp` formula (`0x001d9a64..0x001d9ac4`)
            // R = 160 * pulse; G = 160 * pulse * (2 * ratio); B = 0..low
            let r = (ECG_BASE_COLOR_INTENSITY * pulse).round().clamp(0.0, 255.0) as u8;
            let g = (ECG_BASE_COLOR_INTENSITY * pulse * (2.0 * ratio))
                .round()
                .clamp(0.0, 255.0) as u8;
            let b = (g as f32 * 0.25).round() as u8;
            [r, g, b, 240]
        }
    }

    /// Renders the top-left Health ECG Status Box onto `rasterizer` scaled to its viewport dimensions.
    pub fn draw(&self, hp: f32, max_hp: f32, font: &AtlasFont, rasterizer: &mut TextRasterizer) {
        let sx = rasterizer.width as f32 / 640.0;
        let sy = rasterizer.height as f32 / 480.0;
        let x0 = (ECG_BOX_X_640 * sx).round() as usize;
        let y0 = (ECG_BOX_Y_480 * sy).round() as usize;
        let w = (ECG_BOX_W_640 * sx).round().max(48.0) as usize;
        let h = (ECG_BOX_H_480 * sy).round().max(32.0) as usize;

        let color = self.status_color_rgba(hp, max_hp);
        let status = HealthStatus::from_hp(hp, max_hp);

        // Dark CRT backing + tinted status wash
        rasterizer.fill_rect(x0, y0, w, h, [10, 12, 12, 230]);
        rasterizer.fill_rect(
            x0 + 2,
            y0 + 2,
            w.saturating_sub(4),
            h.saturating_sub(4),
            [color[0] / 3, color[1] / 3, color[2] / 3, 185],
        );

        // Danger red static scanline interference when HP < 25%
        if status == HealthStatus::Danger {
            let seed = self
                .static_frame
                .wrapping_mul(1664525)
                .wrapping_add(1013904223);
            for band in 0..6u32 {
                let row = ((seed.wrapping_add(band * 7919)) as usize) % h.max(1);
                rasterizer.fill_rect(x0 + 2, y0 + row, w.saturating_sub(4), 2, [220, 30, 30, 95]);
            }
        }

        // Draw animated ECG waveform trace across the box
        let mid_y = y0 as i32 + (h as i32 / 2);
        let amp = (h as f32 * 0.34).max(6.0);
        let sweep_t = self.sweep_normalized();
        let samples = 28usize;
        let mut prev: Option<(i32, i32)> = None;
        for i in 0..=samples {
            let t = i as f32 / samples as f32;
            let px = x0 as i32 + ((t * (w.saturating_sub(8)) as f32).round() as i32) + 4;
            // QRS heartbeat spike centered near the active sweep cursor
            let dist = ((t - sweep_t + 1.0) % 1.0).min((sweep_t - t + 1.0) % 1.0);
            let wave = if dist < 0.04 {
                -amp
            } else if dist < 0.08 {
                amp * 0.55
            } else if dist < 0.12 {
                -amp * 0.25
            } else {
                0.0
            };
            let py = mid_y + wave.round() as i32;
            if let Some((lx, ly)) = prev {
                rasterizer.draw_line(
                    lx,
                    ly,
                    px,
                    py,
                    2,
                    [
                        color[0].saturating_add(70),
                        color[1].saturating_add(70),
                        color[2].saturating_add(70),
                        255,
                    ],
                );
            }
            prev = Some((px, py));
        }

        // Crisp outer border + status text label
        rasterizer.draw_rect_outline(x0, y0, w, h, 2, [color[0], color[1], color[2], 255]);
        let label_color = match status {
            HealthStatus::Fine => Sh2TextColor::Green,
            HealthStatus::Caution => Sh2TextColor::GoldSelected,
            HealthStatus::Danger => Sh2TextColor::Custom {
                top: [245, 70, 70, 255],
                bottom: [195, 28, 28, 255],
            },
        };
        let font_px = ((13.0 * sy).round() as usize).clamp(10, 24);
        font.draw_line_styled(
            rasterizer,
            status.label(),
            (x0 + 8, y0 + 6),
            font_px,
            label_color,
            true,
        );
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
        // At full health (>= 0.8), pulse brightness stays 1.0
        ecg.tick(0.05, 100.0, 100.0);
        assert!((ecg.pulse_brightness - 1.0).abs() < 1e-5);
        assert!(ecg.sweep_x > ECG_SWEEP_START_X);

        // At low health (< 0.8), pulse brightness modulates during timer < 0.1
        ecg.pulse_timer = 0.0;
        ecg.tick(0.1, 20.0, 100.0); // pulse_timer = 0.05 -> 1.0 - 6 * 0.05 = 0.7
        assert!((ecg.pulse_brightness - 0.7).abs() < 1e-4);

        // Danger color has R > G
        let c = ecg.status_color_rgba(15.0, 100.0);
        assert!(c[0] > c[1]);
    }
}
