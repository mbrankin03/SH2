//! Authentic PS2 Footstep Loading Screen and Room Transition Renderer.
//!
//! Replicates the iconic Silent Hill 2 room transition and stage loading screens:
//! - Pure white footprint sole silhouettes on pitch-black backdrop (authentic retail parity).
//! - Alternating left/right walking gait cadence.
//! - Smooth alpha decay trail for trailing footsteps.
//! - Modern fast hardware gating: blinking "Press to Continue" prompt when asset streaming completes.
//! - Configurable instant transitions and minimum display duration.

use crate::atlas_font::{AtlasFont, Sh2TextColor};
use crate::font::TextRasterizer;
use image::ImageReader;
use std::io::Cursor;
use std::sync::OnceLock;

/// Static embedded 64x64 footstep sprite decoded from authentic retail .
static FOOTSTEP_SPRITE: OnceLock<FootstepSprite> = OnceLock::new();

/// Pre-decoded footstep alpha silhouette sprite.
pub struct FootstepSprite {
    pub width: u32,
    pub height: u32,
    pub alpha: Vec<u8>,
}

impl FootstepSprite {
    /// Retrieve or lazily decode the embedded footstep sprite.
    pub fn get() -> &'static Self {
        FOOTSTEP_SPRITE.get_or_init(|| {
            let bytes = include_bytes!("../assets/footstep.png");
            let img = ImageReader::new(Cursor::new(bytes))
                .with_guessed_format()
                .expect("Valid footstep PNG format")
                .decode()
                .expect("Decode footstep image")
                .to_rgba8();

            let width = img.width();
            let height = img.height();
            let mut alpha = Vec::with_capacity((width * height) as usize);

            for pixel in img.pixels() {
                // Use alpha channel or luminance
                let lum = ((pixel[0] as u32 * 299 + pixel[1] as u32 * 587 + pixel[2] as u32 * 114)
                    / 1000) as u8;
                let a = pixel[3].max(lum);
                alpha.push(a);
            }

            FootstepSprite {
                width,
                height,
                alpha,
            }
        })
    }
}

/// An individual animated footstep print on the loading screen.
#[derive(Debug, Clone, Copy)]
pub struct FootstepPrint {
    /// Screen center X coordinate in 640x480 virtual frame buffer.
    pub x: f32,
    /// Screen center Y coordinate.
    pub y: f32,
    /// Footstep orientation angle in radians (heading).
    pub angle: f32,
    /// Is this the right foot (true) or left foot (false)?
    pub is_right: bool,
    /// Current alpha opacity [0.0..1.0].
    pub alpha: f32,
}

/// Loading screen state machine coordinating walking footsteps, timing, and input gating.
#[derive(Debug, Clone)]
pub struct LoadingScreen {
    /// Accumulated active display elapsed time in seconds.
    pub elapsed_seconds: f32,
    /// Active footprints currently visible on screen.
    pub footsteps: Vec<FootstepPrint>,
    /// Step index counter.
    pub step_counter: usize,
    /// Timer until the next footstep appears.
    pub next_step_timer: f32,
    /// Cadence period between consecutive footsteps (seconds).
    pub step_cadence: f32,
    /// Has the background asset streaming completed?
    pub load_completed: bool,
    /// Has the user or auto-timer acknowledged and begun fading out into gameplay?
    pub transition_ready: bool,
    /// Minimum duration to hold loading screen before allowing continuation (seconds).
    pub min_duration: f32,
    /// Require button press when loaded instead of instant auto-proceed.
    pub require_press_to_continue: bool,
    /// Fade-in / fade-out alpha for the overall screen transition [0.0..1.0].
    pub fade_alpha: f32,
    /// Stage or room name being loaded.
    pub target_room_label: String,
    /// Flag indicating if a footstep sound effect should be triggered this frame (true = sound cue).
    pub footstep_sound_trigger: bool,
}

impl LoadingScreen {
    /// Create a new footstep loading screen for a destination stage.
    pub fn new(target_label: impl Into<String>, min_duration: f32, require_press: bool) -> Self {
        Self {
            elapsed_seconds: 0.0,
            footsteps: Vec::with_capacity(16),
            step_counter: 0,
            next_step_timer: 0.1, // Initial pause before first step
            step_cadence: 0.55,   // ~1.8 steps per second authentic walking cadence
            load_completed: false,
            transition_ready: false,
            min_duration,
            require_press_to_continue: require_press,
            fade_alpha: 1.0,
            target_room_label: target_label.into(),
            footstep_sound_trigger: false,
        }
    }

    /// Mark the background stage streaming thread as complete.
    pub fn mark_completed(&mut self) {
        self.load_completed = true;
    }

    /// Advance loading screen animation, step spawning, and alpha decay.
    pub fn tick(&mut self, dt: f32) {
        self.elapsed_seconds += dt;
        self.footstep_sound_trigger = false;

        // Step timer & spawn
        self.next_step_timer -= dt;
        if self.next_step_timer <= 0.0 {
            self.next_step_timer = self.step_cadence;
            self.spawn_next_footstep();
            self.footstep_sound_trigger = true;
        }

        // Decay existing footprints
        let decay_rate = 0.22 * dt; // Fade out gradually over ~4.5 seconds
        for step in &mut self.footsteps {
            step.alpha = (step.alpha - decay_rate).max(0.0);
        }
        self.footsteps.retain(|s| s.alpha > 0.01);

        // Auto-continue check if "Press to Continue" is disabled
        if self.load_completed
            && !self.require_press_to_continue
            && self.elapsed_seconds >= self.min_duration
        {
            self.transition_ready = true;
        }
    }

    /// Spawn alternating left/right walking gait footsteps progressing upward / across the screen.
    fn spawn_next_footstep(&mut self) {
        let is_right = (self.step_counter % 2) == 1;
        let step_num = self.step_counter;
        self.step_counter += 1;

        // Walking path: Start near bottom-center and walk gently upward-right
        // Screen coords: 640x480 virtual
        let base_y = 400.0 - (step_num % 7) as f32 * 46.0;
        let lateral_offset = if is_right { 18.0 } else { -18.0 };
        let center_x = 320.0 + (step_num % 7) as f32 * 8.0 + lateral_offset;

        // Angle slightly forward (pointing upward ~ -PI/2)
        let angle = -std::f32::consts::FRAC_PI_2 + if is_right { 0.08 } else { -0.08 };

        self.footsteps.push(FootstepPrint {
            x: center_x,
            y: base_y,
            angle,
            is_right,
            alpha: 1.0,
        });
    }

    /// User pressed confirm / continue button.
    pub fn handle_confirm(&mut self) -> bool {
        if self.load_completed && self.elapsed_seconds >= self.min_duration {
            self.transition_ready = true;
            true
        } else {
            false
        }
    }

    /// Render authentic PS2 footstep loading screen onto the target rasterizer.
    pub fn render(&self, font: &AtlasFont, rast: &mut TextRasterizer) {
        // 1. Clear to pitch-black backdrop (authentic retail PS2 / PC parity)
        rast.clear();

        let sprite = FootstepSprite::get();
        let target_w = rast.width as f32;
        let target_h = rast.height as f32;
        let scale_x = target_w / 640.0;
        let scale_y = target_h / 480.0;

        // 2. Draw footprint trail
        for step in &self.footsteps {
            let scr_x = (step.x * scale_x).round() as i32;
            let scr_y = (step.y * scale_y).round() as i32;
            let print_scale = scale_y * 0.85;

            draw_footstep_silhouette(
                rast,
                sprite,
                scr_x,
                scr_y,
                step.is_right,
                step.alpha,
                print_scale,
            );
        }

        // 3. Status text or "Press to Continue" prompt
        if self.load_completed && self.elapsed_seconds >= self.min_duration {
            // Blinking prompt in bottom-right / bottom-center
            let blink_visible = (self.elapsed_seconds * 2.5).sin() > -0.2;
            if blink_visible {
                let prompt = if self.require_press_to_continue {
                    "PRESS [ENTER / A] TO CONTINUE"
                } else {
                    "STARTING..."
                };
                let center_x = rast.width / 2;
                font.draw_line_centered(
                    rast,
                    prompt,
                    (center_x, (430.0 * scale_y).round() as usize),
                    20,
                    Sh2TextColor::White,
                    true,
                );
            }
        } else {
            // Subtle loading stage text
            let center_x = rast.width / 2;
            font.draw_line_centered(
                rast,
                &format!("LOADING {}...", self.target_room_label.to_uppercase()),
                (center_x, (440.0 * scale_y).round() as usize),
                18,
                Sh2TextColor::White,
                true,
            );
        }
    }
}

/// Blit footstep silhouette onto TextRasterizer with left/right mirroring and alpha transparency.
fn draw_footstep_silhouette(
    rast: &mut TextRasterizer,
    sprite: &FootstepSprite,
    center_x: i32,
    center_y: i32,
    is_right: bool,
    opacity: f32,
    scale: f32,
) {
    let spr_w = sprite.width as f32;
    let spr_h = sprite.height as f32;
    let half_w = (spr_w * scale * 0.5).round() as i32;
    let half_h = (spr_h * scale * 0.5).round() as i32;

    let x_start = (center_x - half_w).max(0);
    let x_end = (center_x + half_w).min(rast.width as i32 - 1);
    let y_start = (center_y - half_h).max(0);
    let y_end = (center_y + half_h).min(rast.height as i32 - 1);

    for y in y_start..=y_end {
        let norm_y = (y - (center_y - half_h)) as f32 / (2.0 * half_h as f32).max(1.0);
        let src_y = ((norm_y * spr_h).round() as u32).min(sprite.height - 1);

        for x in x_start..=x_end {
            let norm_x = (x - (center_x - half_w)) as f32 / (2.0 * half_w as f32).max(1.0);
            // Mirror horizontally if right foot
            let sample_x = if is_right { 1.0 - norm_x } else { norm_x };
            let src_x = ((sample_x * spr_w).round() as u32).min(sprite.width - 1);

            let src_idx = (src_y * sprite.width + src_x) as usize;
            let val = sprite.alpha[src_idx];
            if val == 0 {
                continue;
            }

            let effective_alpha = (val as f32 / 255.0) * opacity;
            let dest_idx = (y as usize * rast.width + x as usize) * 4;

            // Blend pure white (255, 255, 255) over black
            let existing_r = rast.pixels[dest_idx] as f32;
            let existing_g = rast.pixels[dest_idx + 1] as f32;
            let existing_b = rast.pixels[dest_idx + 2] as f32;

            let out_r =
                (255.0 * effective_alpha + existing_r * (1.0 - effective_alpha)).min(255.0) as u8;
            let out_g =
                (255.0 * effective_alpha + existing_g * (1.0 - effective_alpha)).min(255.0) as u8;
            let out_b =
                (255.0 * effective_alpha + existing_b * (1.0 - effective_alpha)).min(255.0) as u8;

            rast.pixels[dest_idx] = out_r;
            rast.pixels[dest_idx + 1] = out_g;
            rast.pixels[dest_idx + 2] = out_b;
            rast.pixels[dest_idx + 3] = 255;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_loading_screen_lifecycle() {
        let mut loading = LoadingScreen::new("Observation Deck", 2.0, true);
        assert_eq!(loading.elapsed_seconds, 0.0);
        assert!(!loading.load_completed);
        assert!(!loading.transition_ready);

        // Advance 1 second: footsteps should spawn
        for _ in 0..60 {
            loading.tick(1.0 / 60.0);
        }
        assert!(loading.elapsed_seconds >= 0.99);
        assert!(!loading.footsteps.is_empty());
        assert!(!loading.transition_ready);

        // Cannot confirm before loaded
        assert!(!loading.handle_confirm());

        // Mark background asset streaming complete
        loading.mark_completed();
        assert!(loading.load_completed);

        // Cannot confirm before min duration (2.0s)
        assert!(!loading.handle_confirm());

        // Advance past 2.0 seconds
        for _ in 0..70 {
            loading.tick(1.0 / 60.0);
        }
        assert!(loading.elapsed_seconds >= 2.0);

        // Now confirm works and initiates gameplay transition
        assert!(loading.handle_confirm());
        assert!(loading.transition_ready);
    }

    #[test]
    fn test_loading_screen_rendering() {
        let mut loading = LoadingScreen::new("Restroom Start", 0.5, false);
        for _ in 0..30 {
            loading.tick(1.0 / 60.0);
        }
        loading.mark_completed();

        let font = AtlasFont::load_embedded();
        let mut rast = TextRasterizer::new(640, 480);
        loading.render(&font, &mut rast);

        // Verify that footprints were drawn (non-zero pixels)
        let non_zero_count = rast.pixels.iter().step_by(4).filter(|&&p| p > 0).count();
        assert!(
            non_zero_count > 50,
            "Expected rendered footstep pixels on black backdrop"
        );
    }
}
