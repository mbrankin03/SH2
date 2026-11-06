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
use sh2_assets::formats::texture::parse_texture_container;
use sh2_assets::resolver::AssetPathResolver;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Static footstep sprite loaded and ingested at runtime from retail assets.
static FOOTSTEP_SPRITE: OnceLock<FootstepSprite> = OnceLock::new();

/// Pre-decoded footstep alpha silhouette sprite.
pub struct FootstepSprite {
    pub width: u32,
    pub height: u32,
    pub alpha: Vec<u8>,
}

impl FootstepSprite {
    /// Ingest the footstep texture from the retail  texture container
    /// (checking  or ),
    /// falling back to procedural generation if game assets have not been installed yet.
    pub fn get() -> &'static Self {
        FOOTSTEP_SPRITE.get_or_init(|| {
            if let Some(sprite) = Self::load_from_retail_assets() {
                return sprite;
            }
            Self::generate_procedural_fallback()
        })
    }

    /// Try to ingest the authentic retail footstep texture from disk.
    fn load_from_retail_assets() -> Option<Self> {
        let resolver = AssetPathResolver::resolve();
        let candidate_rel_paths = [
            "effect/footmark.tbn2",
            "pic/effect/footmark.tbn2",
            "data/pic/effect/footmark.tbn2",
        ];

        let mut tbn2_path: Option<PathBuf> = None;
        for rel in &candidate_rel_paths {
            if let Some(path) = resolver.resolve_file(rel) {
                tbn2_path = Some(path);
                break;
            }
        }

        // Direct candidate fallbacks if running from nested test working directories
        if tbn2_path.is_none() {
            let direct_candidates = [
                "Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2/sh2e/effect/footmark.tbn2",
                "../Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2/sh2e/effect/footmark.tbn2",
                "../../Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2/sh2e/effect/footmark.tbn2",
                "Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2/data/pic/effect/footmark.tbn2",
                "../Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2/data/pic/effect/footmark.tbn2",
                "../../Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2/data/pic/effect/footmark.tbn2",
            ];
            for cand in &direct_candidates {
                let p = Path::new(cand);
                if p.is_file() {
                    tbn2_path = Some(p.to_path_buf());
                    break;
                }
            }
        }

        let path = tbn2_path?;
        let bytes = std::fs::read(&path).ok()?;
        let textures = parse_texture_container(&bytes, 0).ok()?;
        let tex = textures.first()?;
        let rgba = tex.to_rgba().ok()?;

        let width = tex.width as u32;
        let height = tex.height as u32;
        let mut alpha = Vec::with_capacity((width * height) as usize);

        let (chunks, _) = rgba.as_chunks::<4>();
        for chunk in chunks {
            alpha.push(chunk[3]);
        }

        Some(Self {
            width,
            height,
            alpha,
        })
    }

    /// Procedural mathematical sole generation when retail asset files are absent (e.g. CI environments).
    fn generate_procedural_fallback() -> Self {
        let width = 64u32;
        let height = 128u32;
        let mut alpha = vec![0u8; (width * height) as usize];

        for y in 0..height {
            let ny = y as f32 / height as f32; // 0.0 (toe) to 1.0 (heel)
            for x in 0..width {
                let nx = (x as f32 - 32.0) / 32.0; // -1.0 (outer) to +1.0 (inner)

                let mut in_sole = false;
                let mut intensity = 0.0f32;

                if (0.06..=0.50).contains(&ny) {
                    // Forefoot & ball of foot
                    let cy = 0.28;
                    let rx = 0.62 + (0.28 - (ny - cy).abs()) * 0.25;
                    let ry = 0.22;
                    let dx = nx / rx;
                    let dy = (ny - cy) / ry;
                    let dist = dx * dx + dy * dy;
                    if dist <= 1.0 {
                        in_sole = true;
                        intensity = 1.0 - ((dist - 0.75) / 0.25).clamp(0.0, 1.0);
                    }
                } else if (0.50..=0.66).contains(&ny) {
                    // Arch & waist (inner cutout)
                    let arch_inner = 0.18 + (ny - 0.50) * 0.2;
                    let arch_outer = -0.52;
                    if (arch_outer..=arch_inner).contains(&nx) {
                        in_sole = true;
                        intensity = 1.0;
                    }
                } else if (0.66..=0.94).contains(&ny) {
                    // Heel
                    let cy = 0.80;
                    let rx = 0.48;
                    let ry = 0.14;
                    let dx = nx / rx;
                    let dy = (ny - cy) / ry;
                    let dist = dx * dx + dy * dy;
                    if dist <= 1.0 {
                        in_sole = true;
                        intensity = 1.0 - ((dist - 0.75) / 0.25).clamp(0.0, 1.0);
                    }
                }

                if in_sole {
                    // Subtle sole tread grooving
                    let tread = if y % 6 == 0 { 0.75 } else { 1.0 };
                    let val = (intensity * tread * 255.0).clamp(0.0, 255.0) as u8;
                    alpha[(y * width + x) as usize] = val;
                }
            }
        }

        Self {
            width,
            height,
            alpha,
        }
    }
}

/// Walking path trajectory patterns for authentic visual variety across transitions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadingPathPattern {
    /// Walking gently upward with slight organic sine wandering.
    UpwardWander,
    /// Stepping diagonally from bottom-left to upper-right.
    BottomLeftToTopRight,
    /// Stepping diagonally from bottom-right to upper-left.
    BottomRightToTopLeft,
    /// Gentle S-curve arc traversing across the center display.
    CurvedArc,
}

impl LoadingPathPattern {
    /// Select a pattern based on seed or cycling index.
    pub fn from_index(index: usize) -> Self {
        match index % 4 {
            0 => Self::UpwardWander,
            1 => Self::BottomLeftToTopRight,
            2 => Self::BottomRightToTopLeft,
            _ => Self::CurvedArc,
        }
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
    /// Trajectory pattern selected for this transition.
    pub path_pattern: LoadingPathPattern,
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
}

impl LoadingScreen {
    /// Create a new footstep loading screen for a destination stage.
    pub fn new(target_label: impl Into<String>, min_duration: f32, require_press: bool) -> Self {
        let label_str = target_label.into();
        let pattern_seed = label_str
            .bytes()
            .fold(0usize, |acc, b| acc.wrapping_add(b as usize));
        let path_pattern = LoadingPathPattern::from_index(pattern_seed);

        Self {
            elapsed_seconds: 0.0,
            footsteps: Vec::with_capacity(16),
            step_counter: 0,
            next_step_timer: 0.1, // Initial pause before first step
            step_cadence: 0.55,   // ~1.8 steps per second authentic walking cadence
            path_pattern,
            load_completed: false,
            transition_ready: false,
            min_duration,
            require_press_to_continue: require_press,
            fade_alpha: 1.0,
            target_room_label: label_str,
        }
    }

    /// Set an explicit trajectory path pattern.
    pub fn with_pattern(mut self, pattern: LoadingPathPattern) -> Self {
        self.path_pattern = pattern;
        self
    }

    /// Mark the background stage streaming thread as complete.
    pub fn mark_completed(&mut self) {
        self.load_completed = true;
    }

    /// Advance loading screen animation, step spawning, and alpha decay.
    pub fn tick(&mut self, dt: f32) {
        self.elapsed_seconds += dt;

        // Step timer & spawn
        self.next_step_timer -= dt;
        if self.next_step_timer <= 0.0 {
            self.next_step_timer = self.step_cadence;
            self.spawn_next_footstep();
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

        let lateral_offset = if is_right { 18.0 } else { -18.0 };
        let gait_angle_offset = if is_right { 0.08 } else { -0.08 };

        // Wrap step progression smoothly across screen (0..8 steps)
        let t = (step_num % 8) as f32;

        let (center_x, base_y, heading_angle) = match self.path_pattern {
            LoadingPathPattern::UpwardWander => {
                let y = 410.0 - t * 46.0;
                let wander = (t * 0.75).sin() * 22.0;
                let x = 320.0 + wander + lateral_offset;
                let angle = (t * 0.75).cos() * 0.15 + gait_angle_offset;
                (x, y, angle)
            }
            LoadingPathPattern::BottomLeftToTopRight => {
                let y = 420.0 - t * 45.0;
                let x = 200.0 + t * 30.0 + lateral_offset;
                let angle = 0.35 + gait_angle_offset;
                (x, y, angle)
            }
            LoadingPathPattern::BottomRightToTopLeft => {
                let y = 420.0 - t * 45.0;
                let x = 440.0 - t * 30.0 + lateral_offset;
                let angle = -0.35 + gait_angle_offset;
                (x, y, angle)
            }
            LoadingPathPattern::CurvedArc => {
                let y = 415.0 - t * 46.0;
                let arc = ((t - 3.5) * 0.5).sin() * 45.0;
                let x = 320.0 + arc + lateral_offset;
                let angle = ((t - 3.5) * 0.5).cos() * 0.25 + gait_angle_offset;
                (x, y, angle)
            }
        };

        self.footsteps.push(FootstepPrint {
            x: center_x,
            y: base_y,
            angle: heading_angle,
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
            // Scale footstep sprite to authentic PS2 footprint dimensions (~54px height in 640x480 space)
            let print_scale = scale_y * (64.0 / sprite.height as f32) * 0.85;

            draw_footstep_silhouette(rast, sprite, scr_x, scr_y, step, print_scale);
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

/// Blit footstep silhouette onto TextRasterizer with rotation, left/right mirroring, and alpha transparency.
fn draw_footstep_silhouette(
    rast: &mut TextRasterizer,
    sprite: &FootstepSprite,
    center_x: i32,
    center_y: i32,
    step: &FootstepPrint,
    scale: f32,
) {
    let spr_w = sprite.width as f32;
    let spr_h = sprite.height as f32;
    let rad = ((spr_w * spr_w + spr_h * spr_h).sqrt() * scale * 0.5).ceil() as i32;

    let x_start = (center_x - rad).max(0);
    let x_end = (center_x + rad).min(rast.width as i32 - 1);
    let y_start = (center_y - rad).max(0);
    let y_end = (center_y + rad).min(rast.height as i32 - 1);

    let cos_a = (-step.angle).cos();
    let sin_a = (-step.angle).sin();
    let inv_scale = 1.0 / scale.max(0.001);

    for y in y_start..=y_end {
        let dy = (y - center_y) as f32 * inv_scale;
        for x in x_start..=x_end {
            let dx = (x - center_x) as f32 * inv_scale;

            // Rotate back into sprite local coordinates
            let local_x = dx * cos_a - dy * sin_a;
            let local_y = dx * sin_a + dy * cos_a;

            let norm_x = (local_x / spr_w) + 0.5;
            let norm_y = (local_y / spr_h) + 0.5;

            if norm_x < 0.0 || norm_x >= 1.0 || norm_y < 0.0 || norm_y >= 1.0 {
                continue;
            }

            let sample_x = if step.is_right { 1.0 - norm_x } else { norm_x };
            let fx = sample_x * (spr_w - 1.0);
            let fy = norm_y * (spr_h - 1.0);

            let x0 = (fx.floor() as u32).min(sprite.width - 1);
            let y0 = (fy.floor() as u32).min(sprite.height - 1);
            let x1 = (x0 + 1).min(sprite.width - 1);
            let y1 = (y0 + 1).min(sprite.height - 1);

            let rx = fx - x0 as f32;
            let ry = fy - y0 as f32;

            let a00 = sprite.alpha[(y0 * sprite.width + x0) as usize] as f32;
            let a10 = sprite.alpha[(y0 * sprite.width + x1) as usize] as f32;
            let a01 = sprite.alpha[(y1 * sprite.width + x0) as usize] as f32;
            let a11 = sprite.alpha[(y1 * sprite.width + x1) as usize] as f32;

            let top = a00 * (1.0 - rx) + a10 * rx;
            let bot = a01 * (1.0 - rx) + a11 * rx;
            let interpolated_alpha = top * (1.0 - ry) + bot * ry;

            if interpolated_alpha <= 0.001 {
                continue;
            }

            let effective_alpha = (interpolated_alpha / 255.0) * step.alpha;
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

    #[test]
    fn test_footstep_sprite_ingestion_and_fallback() {
        let sprite = FootstepSprite::get();
        assert!(sprite.width > 0);
        assert!(sprite.height > 0);
        assert_eq!(sprite.alpha.len(), (sprite.width * sprite.height) as usize);
        let non_zeros = sprite.alpha.iter().filter(|&&a| a > 0).count();
        assert!(
            non_zeros > 500,
            "Footstep sprite should have substantive silhouette data"
        );

        // Test procedural generator directly
        let fallback = FootstepSprite::generate_procedural_fallback();
        assert_eq!(fallback.width, 64);
        assert_eq!(fallback.height, 128);
        let fb_non_zeros = fallback.alpha.iter().filter(|&&a| a > 0).count();
        assert!(
            fb_non_zeros > 1000,
            "Procedural fallback should generate full foot silhouette"
        );
    }

    #[test]
    fn test_loading_screen_patterns() {
        for i in 0..4 {
            let pattern = LoadingPathPattern::from_index(i);
            let mut loading = LoadingScreen::new("Test Room", 1.0, false).with_pattern(pattern);
            for _ in 0..60 {
                loading.tick(1.0 / 60.0);
            }
            assert!(!loading.footsteps.is_empty());
            let step = loading.footsteps[0];
            assert!(step.x > 50.0 && step.x < 590.0);
            assert!(step.y > 50.0 && step.y < 450.0);
        }
    }
}
