//! Authentic 1:1 PS2 Health Status Snapshot Tint (`look_hp` in `SLUS_202.28` at `0x001d9660`).
//!
//! # Ground Truth (`SLUS_202.28` `look_hp` @ `0x001d9660..0x001d9be0`)
//! In Silent Hill 2, the top-left `Status` box is a **live miniature snapshot of the frozen 3D
//! gameplay frame**, tinted by player health (`Fine` green -> `Caution` yellow/amber -> `Danger`
//! pulsing crimson red) with **no ECG waveform line and no status text label**:
//! - Normalized health ratio: `ratio = (hp / max_hp).clamp(0.0, 1.0)` (`0x001d96d0`).
//! - Damage factor: `damage = 1.0 - ratio` (`0x001d96e0`).
//! - Horizontal sweep speed: `(1.4 + damage / 0.7) * 30.0 * dt` (`0x001d96e4..0x001d9724`),
//!   wrapping when `sweep_x > -70.0` (`0xc28c0000`) back to `-210.0` (`0xc3520000`).
//! - Heartbeat pulse cycle: `pulse_timer += dt / 2.0`, resets to `0.0` when
//!   `pulse_timer > 0.2 + ratio` (`0x001d9818..0x001d9840`).
//! - Heartbeat brightness modulation when `hp < 0.8 * max_hp` (`0x3f4ccccd`):
//!   - `pulse_timer < 0.1`: `brightness = 1.0 - 6.0 * pulse_timer` (`0x001d9890..0x001d98ac`)
//!   - `0.1 <= pulse_timer < 0.2`: `brightness = 6.0 * pulse_timer - 0.2` (`0x001d98bc..0x001d9918`)
//!   - Otherwise: `brightness = 1.0` (`0x001d9924`).
//! - Color shift (`0x001d9a3c..0x001d9b50`, base intensity `160.0` = `0x43200000`):
//!   - `hp >= 0.5 * max_hp`: soft emerald green -> yellow-amber tint wash over the 3D snapshot.
//!   - `hp < 0.5 * max_hp`: `R = 160.0 * brightness`, `G = 160.0 * brightness * (2.0 * ratio)`,
//!     shifting smoothly to pulsing red as `hp -> 0`.

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

/// Top-left `Status` 3D snapshot rectangle in 640x480 logical space `(x, y, w, h)`
/// matching PS2 Final Release layout (`look_hp` `0x001d9b6c..0x001d9b84` under `Status` `y = 50`).
pub const STATUS_SNAPSHOT_X_640: f32 = 50.0;
/// Top-left Y of the `Status` 3D snapshot box in 640x480 space.
pub const STATUS_SNAPSHOT_Y_480: f32 = 64.0;
/// Width of the `Status` 3D snapshot box in 640x480 space.
pub const STATUS_SNAPSHOT_W_640: f32 = 166.0;
/// Height of the `Status` 3D snapshot box in 640x480 space.
pub const STATUS_SNAPSHOT_H_480: f32 = 108.0;

/// Coarse health status band reflected in the `Status` snapshot tint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthStatus {
    /// `hp >= 0.8 * max_hp`: steady emerald green tint, no heartbeat pulse (`look_hp` `0x001d986c`).
    Fine,
    /// `0.25 * max_hp <= hp < 0.8 * max_hp`: yellow/amber tint with rhythmic pulse.
    Caution,
    /// `hp < 0.25 * max_hp`: crimson red pulsing tint with subtle CRT interference.
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

        self.static_frame = self.static_frame.wrapping_add(1);
    }

    /// Computes the authentic RGBA tint for the `Status` snapshot box (`look_hp` `0x001d9a3c..0x001d9b50`).
    pub fn status_color_rgba(&self, hp: f32, max_hp: f32) -> [u8; 4] {
        let ratio = if max_hp > 0.0 {
            (hp / max_hp).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let pulse = self.pulse_brightness;
        if ratio >= ECG_PULSE_THRESHOLD_RATIO {
            // Fine: authentic translucent green tint wash over frozen 3D snapshot
            let g = (ECG_BASE_COLOR_INTENSITY * pulse).round().clamp(0.0, 255.0) as u8;
            let r = (g as f32 * 0.30).round() as u8;
            let b = (g as f32 * 0.35).round() as u8;
            [r, g, b, 92]
        } else if ratio >= ECG_DANGER_COLOR_RATIO {
            // Upper Caution (50%..80%): green -> yellow/amber wash
            let t = (ratio - ECG_DANGER_COLOR_RATIO)
                / (ECG_PULSE_THRESHOLD_RATIO - ECG_DANGER_COLOR_RATIO);
            let r = (ECG_BASE_COLOR_INTENSITY * pulse * (1.0 - 0.55 * t))
                .round()
                .clamp(0.0, 255.0) as u8;
            let g = (ECG_BASE_COLOR_INTENSITY * pulse).round().clamp(0.0, 255.0) as u8;
            let b = (ECG_BASE_COLOR_INTENSITY * pulse * 0.18)
                .round()
                .clamp(0.0, 255.0) as u8;
            [r, g, b, 110]
        } else {
            // Lower Caution / Danger (< 50%): exact `look_hp` formula (`0x001d9a64..0x001d9ac4`)
            let r = (ECG_BASE_COLOR_INTENSITY * pulse).round().clamp(0.0, 255.0) as u8;
            let g = (ECG_BASE_COLOR_INTENSITY * pulse * (2.0 * ratio))
                .round()
                .clamp(0.0, 255.0) as u8;
            let b = (g as f32 * 0.25).round() as u8;
            [r, g, b, 132]
        }
    }

    /// Computes the `(x, y, width, height)` viewport pixel rectangle for the live 3D game snapshot
    /// inside a swapchain of `(viewport_w, viewport_h)`.
    pub fn snapshot_viewport_rect(viewport_w: f32, viewport_h: f32) -> (f32, f32, f32, f32) {
        let sx = viewport_w / 640.0;
        let sy = viewport_h / 480.0;
        (
            STATUS_SNAPSHOT_X_640 * sx,
            STATUS_SNAPSHOT_Y_480 * sy,
            STATUS_SNAPSHOT_W_640 * sx,
            STATUS_SNAPSHOT_H_480 * sy,
        )
    }

    /// Renders the translucent `look_hp` health color wash over the live 3D snapshot cutout.
    /// Leaves the underlying frozen 3D gameplay frame visible inside `(x0, y0, w, h)` with
    /// **no ECG waveform line and no status text** (1:1 PS2 Final Release).
    pub fn draw_snapshot_tint(&self, hp: f32, max_hp: f32, rasterizer: &mut TextRasterizer) {
        let (fx, fy, fw, fh) =
            Self::snapshot_viewport_rect(rasterizer.width as f32, rasterizer.height as f32);
        let x0 = fx.round() as usize;
        let y0 = fy.round() as usize;
        let w = fw.round().max(16.0) as usize;
        let h = fh.round().max(16.0) as usize;

        let color = self.status_color_rgba(hp, max_hp);
        rasterizer.fill_rect(x0, y0, w, h, color);

        // Subtle CRT pulse scanline interference only in Danger (< 25% HP)
        if HealthStatus::from_hp(hp, max_hp) == HealthStatus::Danger {
            let seed = self
                .static_frame
                .wrapping_mul(1664525)
                .wrapping_add(1013904223);
            for band in 0..4u32 {
                let row = ((seed.wrapping_add(band * 7919)) as usize) % h.max(1);
                rasterizer.fill_rect(x0, y0 + row, w, 2, [215, 25, 25, 85]);
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
    }
}
