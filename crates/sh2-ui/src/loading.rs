//! Authentic Retail PC Footstep Loading Screen (`sh2pc.exe` `FUN_00406ab0`, `FUN_00406be0`,
//! `FUN_004c8500..0x004c8bc0`) and PS2 prototype `now_loading.c` (`0x00298fa0..0x00299170`).
//!
//! Prototype (`SLUS_202.28`) vs Retail PC (`sh2pc.exe`) drift:
//! - `SLUS_202.28` `now_loading.c` (`0x00298fa0..0x00299170`) has no footsteps; it triggers
//!   `ScreenEffectFadeStart(4.0f, 1)` and draws jittered `\c\hNow loading` every 64 frames.
//! - Retail PC `sh2pc.exe` loads `data/etc/effect/footmark.tbn2` (`FUN_004c8500`), stamps
//!   alternating footsteps every `1.0f` second (`FUN_00406be0`), decays them over `4.0f` seconds
//!   (`FUN_00406ab0` -> `FUN_004783b0(4.0f)`), and renders `"NOW LOADING..."` (`common_msg_e.mes`
//!   ID `25`) at `(480/640 * W, 424/480 * H)` with `fontWide(16, 24)`.

use crate::atlas_font::{AtlasFont, Sh2TextColor};
use crate::font::TextRasterizer;
use sh2_assets::formats::texture::parse_texture_container;
use sh2_assets::resolver::AssetPathResolver;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Exact footstep cadence period in seconds (`sh2pc.exe` `FUN_00406be0` `0x0062edf4` = `1.0f`).
pub const FOOTSTEP_CADENCE_SECS: f32 = 1.0;

/// Exact footstep fade-to-black decay duration in seconds (`sh2pc.exe` `FUN_00406ab0` ->
/// `FUN_004783b0(4.0f)` `0x40800000`).
pub const FOOTSTEP_DECAY_SECS: f32 = 4.0;

/// Retail PC activation delay while faded black (`sh2pc.exe` `FUN_00406ab0`, `_DAT_0093268c >= 3.0f`).
pub const LOADING_ACTIVATION_DELAY_SECS: f32 = 3.0;

/// Retail PC minimum display hold timer (`sh2pc.exe` `FUN_00406ab0`, `_DAT_00932694 = 1.0f`).
pub const LOADING_MIN_DISPLAY_SECS: f32 = 1.0;

/// Alternating left/right lateral offset in virtual units (`sh2pc.exe` `FUN_004c8bc0`:
/// `-30.0f` = `0xC1F00000` for `foot_side == 1`, `+30.0f` = `0x41F00000` for `foot_side == 0`).
pub const FOOTSTEP_LATERAL_OFFSET: f32 = 30.0;

/// Horizontal aspect correction applied in `FUN_004c87e0` (`0x004c87e0`: `vx *= 0.75`).
pub const FOOTSTEP_X_ASPECT: f32 = 0.75;

/// Path reset boundary in virtual coordinates (`FUN_004c87e0`: `|x| > 320.0 || |y| > 320.0`).
pub const FOOTSTEP_BOUNDS_LIMIT: f32 = 320.0;

/// Texture UV crop bounds (`sh2pc.exe` `0x00890af0` / `0x00890b10`: `3/64..61/64`).
pub const FOOTSTEP_UV_MIN: f32 = 3.0 / 64.0;
pub const FOOTSTEP_UV_MAX: f32 = 61.0 / 64.0;

/// `"NOW LOADING..."` normalized screen position (`FUN_00406be0`: `480/640`, `424/480`).
pub const NOW_LOADING_NORM_X: f32 = 480.0 / 640.0;
pub const NOW_LOADING_NORM_Y: f32 = 424.0 / 480.0;

/// Static footstep sprite loaded from `data/etc/effect/footmark.tbn2` (`FUN_004c8500`).
static FOOTSTEP_SPRITE: OnceLock<FootstepSprite> = OnceLock::new();

/// Pre-decoded footstep alpha silhouette sprite (`64×64` from `footmark.tbn2`).
pub struct FootstepSprite {
    pub width: u32,
    pub height: u32,
    pub alpha: Vec<u8>,
}

impl FootstepSprite {
    /// Loads `data/etc/effect/footmark.tbn2` (`sh2pc.exe` `FUN_004c8500`), falling back to a
    /// procedural 64×64 sole silhouette only when retail assets are absent (e.g. CI).
    pub fn get() -> &'static Self {
        FOOTSTEP_SPRITE.get_or_init(|| {
            if let Some(sprite) = Self::load_from_retail_assets() {
                return sprite;
            }
            Self::generate_procedural_fallback()
        })
    }

    fn load_from_retail_assets() -> Option<Self> {
        let resolver = AssetPathResolver::resolve();
        // Exact retail path first: `data/etc/effect/footmark.tbn2` (`FUN_004c8500`).
        let candidate_rel_paths = [
            "etc/effect/footmark.tbn2",
            "pic/effect/footmark.tbn2",
            "effect/footmark.tbn2",
        ];

        let mut tbn2_path: Option<PathBuf> = None;
        for rel in &candidate_rel_paths {
            if let Some(path) = resolver.resolve_file(rel) {
                tbn2_path = Some(path);
                break;
            }
        }

        if tbn2_path.is_none() {
            let direct_candidates = [
                "Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2/data/etc/effect/footmark.tbn2",
                "../Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2/data/etc/effect/footmark.tbn2",
                "../../Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2/data/etc/effect/footmark.tbn2",
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

    /// Procedural 64×64 sole fallback used only when `footmark.tbn2` is not present on disk.
    fn generate_procedural_fallback() -> Self {
        let width = 64u32;
        let height = 64u32;
        let mut alpha = vec![0u8; (width * height) as usize];

        for y in 0..height {
            let ny = y as f32 / height as f32;
            for x in 0..width {
                let nx = (x as f32 - 32.0) / 32.0;
                let mut intensity = 0.0f32;
                if (0.08..=0.52).contains(&ny) {
                    let dx = nx / 0.36;
                    let dy = (ny - 0.30) / 0.22;
                    let d = dx * dx + dy * dy;
                    if d <= 1.0 {
                        intensity = 1.0 - ((d - 0.75) / 0.25).clamp(0.0, 1.0);
                    }
                } else if (0.52..=0.66).contains(&ny) && (-0.28..=0.14).contains(&nx) {
                    intensity = 1.0;
                } else if (0.66..=0.92).contains(&ny) {
                    let dx = nx / 0.28;
                    let dy = (ny - 0.79) / 0.13;
                    let d = dx * dx + dy * dy;
                    if d <= 1.0 {
                        intensity = 1.0 - ((d - 0.75) / 0.25).clamp(0.0, 1.0);
                    }
                }
                if intensity > 0.0 {
                    alpha[(y * width + x) as usize] = (intensity * 255.0).clamp(0.0, 255.0) as u8;
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

/// The 8 edge-entry trajectory directions in `sh2pc.exe` `FUN_004c8590` (`dir = rand() & 7`,
/// jump tables `0x004c87a0` & `0x004c87c0`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum LoadingPathPattern {
    /// `dir 0`: `y0 = -230, x0 += 128`, `angle += 135°`
    TopEdgeRight = 0,
    /// `dir 1`: `y0 = +230, x0 += 128`, `angle -= 135°`
    BottomEdgeRight = 1,
    /// `dir 2`: `x0 = -230, y0 += 128`, `angle -= 45°`
    LeftEdgeDown = 2,
    /// `dir 3`: `x0 = +230, y0 += 128`, `angle -= 135°`
    RightEdgeDown = 3,
    /// `dir 4`: `y0 = +230, x0 -= 128`, `angle -= 45°`
    BottomEdgeLeft = 4,
    /// `dir 5`: `y0 = -230, x0 -= 128`, `angle += 45°`
    TopEdgeLeft = 5,
    /// `dir 6`: `x0 = +230, y0 -= 128`, `angle += 135°`
    RightEdgeUp = 6,
    /// `dir 7`: `x0 = -230, y0 -= 128`, `angle += 45°`
    LeftEdgeUp = 7,
}

impl LoadingPathPattern {
    pub const ALL: [Self; 8] = [
        Self::TopEdgeRight,
        Self::BottomEdgeRight,
        Self::LeftEdgeDown,
        Self::RightEdgeDown,
        Self::BottomEdgeLeft,
        Self::TopEdgeLeft,
        Self::RightEdgeUp,
        Self::LeftEdgeUp,
    ];

    /// Select one of the 8 `FUN_004c8590` directions from an index.
    #[must_use]
    pub const fn from_index(index: usize) -> Self {
        Self::ALL[index & 7]
    }
}

/// An individual stamped footstep quad in virtual 512×480 centered space (`FUN_004c87e0`).
#[derive(Debug, Clone, Copy)]
pub struct FootstepPrint {
    /// Center X in virtual 512×480 space (`0..512` top-left equivalent = `cx + 256`).
    pub x: f32,
    /// Center Y in virtual 512×480 space (`0..480` top-left equivalent = `cy + 240`).
    pub y: f32,
    /// Orientation angle in radians (`angle_deg.to_radians()`).
    pub angle: f32,
    /// Half-size scale in virtual 512×480 units (`[30.0, 39.0]`, `FUN_004c8590`).
    pub scale: f32,
    /// Alternating foot side (`true` = `foot_side == 1`, U mirrored `61/64 -> 3/64`).
    pub is_right: bool,
    /// Current alpha opacity `[0.0..1.0]` decaying over `FOOTSTEP_DECAY_SECS = 4.0s`.
    pub alpha: f32,
}

/// Active path parameters generated by `FUN_004c8590` (`0x004c8590`).
#[derive(Debug, Clone, Copy)]
struct FootstepPathState {
    origin_x: f32,
    origin_y: f32,
    angle_rad: f32,
    scale: f32,
    stride_dy: f32,
    local_y: f32,
    foot_side: u8,
    active: bool,
}

/// Loading screen state machine porting `sh2pc.exe` `FUN_00406ab0` / `FUN_00406be0` / `FUN_004c8590..0x004c8bc0`.
#[derive(Debug, Clone)]
pub struct LoadingScreen {
    pub elapsed_seconds: f32,
    pub footsteps: Vec<FootstepPrint>,
    pub step_counter: usize,
    pub next_step_timer: f32,
    pub step_cadence: f32,
    pub path_pattern: LoadingPathPattern,
    pub load_completed: bool,
    pub transition_ready: bool,
    pub min_duration: f32,
    pub require_press_to_continue: bool,
    pub fade_alpha: f32,
    pub target_room_label: String,
    rng_state: u32,
    path_state: FootstepPathState,
}

static TRANSITION_CYCLE_COUNTER: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

impl LoadingScreen {
    /// Creates a new footstep loading screen cycling deterministically through `dir = 0..8`.
    pub fn new(target_label: impl Into<String>, min_duration: f32, require_press: bool) -> Self {
        let cycle = TRANSITION_CYCLE_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Self::new_with_seed(target_label, min_duration, require_press, cycle)
    }

    /// Creates a new footstep loading screen with an explicit deterministic seed.
    pub fn new_with_seed(
        target_label: impl Into<String>,
        min_duration: f32,
        require_press: bool,
        seed: usize,
    ) -> Self {
        let path_pattern = LoadingPathPattern::from_index(seed);
        let rng_state = (seed as u32)
            .wrapping_mul(1_664_525)
            .wrapping_add(1_013_904_223);
        Self {
            elapsed_seconds: 0.0,
            footsteps: Vec::with_capacity(16),
            step_counter: 0,
            next_step_timer: 0.0, // Immediately stamps first footstep on tick (`FUN_00406be0`)
            step_cadence: FOOTSTEP_CADENCE_SECS,
            path_pattern,
            load_completed: false,
            transition_ready: false,
            min_duration,
            require_press_to_continue: require_press,
            fade_alpha: 1.0,
            target_room_label: target_label.into(),
            rng_state,
            path_state: FootstepPathState {
                origin_x: 0.0,
                origin_y: 0.0,
                angle_rad: 0.0,
                scale: 34.0,
                stride_dy: -90.0,
                local_y: 0.0,
                foot_side: 0,
                active: false,
            },
        }
    }

    /// Sets an explicit initial direction (`dir ∈ 0..8`, `FUN_004c8590`).
    pub fn with_pattern(mut self, pattern: LoadingPathPattern) -> Self {
        self.path_pattern = pattern;
        self.path_state.active = false;
        self
    }

    /// Marks the background asset load as complete.
    pub fn mark_completed(&mut self) {
        self.load_completed = true;
    }

    fn next_rand(&mut self) -> u32 {
        // Standard MSVC LCG matching `sh2pc.exe` `rand()` (`0..=32767`).
        self.rng_state = self.rng_state.wrapping_mul(214_013).wrapping_add(2_531_011);
        (self.rng_state >> 16) & 0x7fff
    }

    /// Initializes a new random footstep path matching `sh2pc.exe` `FUN_004c8590` (`0x004c8590`).
    fn init_footstep_path(&mut self, forced_dir: Option<LoadingPathPattern>) {
        let mut x0 = ((self.next_rand() % 384) as f32) - 192.0;
        let mut y0 = ((self.next_rand() % 384) as f32) - 192.0;
        let mut angle_deg = ((self.next_rand() % 60) as f32) + 60.0;
        let dir =
            forced_dir.unwrap_or_else(|| LoadingPathPattern::from_index(self.next_rand() as usize));
        self.path_pattern = dir;

        // Angle offset (`FUN_004c8590`)
        match dir {
            LoadingPathPattern::TopEdgeRight | LoadingPathPattern::RightEdgeUp => {
                angle_deg += 135.0;
            }
            LoadingPathPattern::BottomEdgeRight | LoadingPathPattern::RightEdgeDown => {
                angle_deg -= 135.0;
            }
            LoadingPathPattern::LeftEdgeDown | LoadingPathPattern::BottomEdgeLeft => {
                angle_deg -= 45.0;
            }
            LoadingPathPattern::TopEdgeLeft | LoadingPathPattern::LeftEdgeUp => {
                angle_deg += 45.0;
            }
        }

        // Edge snap `±230.0` + perpendicular shift `±128.0` (jump tables `0x004c87a0` & `0x004c87c0`)
        match dir {
            LoadingPathPattern::TopEdgeRight => {
                y0 = -230.0;
                x0 += 128.0;
            }
            LoadingPathPattern::BottomEdgeRight => {
                y0 = 230.0;
                x0 += 128.0;
            }
            LoadingPathPattern::LeftEdgeDown => {
                x0 = -230.0;
                y0 += 128.0;
            }
            LoadingPathPattern::RightEdgeDown => {
                x0 = 230.0;
                y0 += 128.0;
            }
            LoadingPathPattern::BottomEdgeLeft => {
                y0 = 230.0;
                x0 -= 128.0;
            }
            LoadingPathPattern::TopEdgeLeft => {
                y0 = -230.0;
                x0 -= 128.0;
            }
            LoadingPathPattern::RightEdgeUp => {
                x0 = 230.0;
                y0 -= 128.0;
            }
            LoadingPathPattern::LeftEdgeUp => {
                x0 = -230.0;
                y0 -= 128.0;
            }
        }

        let scale = ((self.next_rand() % 10) as f32) + 30.0;
        let stride_dy = (((self.next_rand() % 50) as f32) + 250.0) * scale * -0.01;

        self.path_state = FootstepPathState {
            origin_x: x0,
            origin_y: y0,
            angle_rad: angle_deg.to_radians(),
            scale,
            stride_dy,
            local_y: 0.0,
            foot_side: 0,
            active: true,
        };
    }

    /// Advances loading screen timers, footstep cadence (`1.0s`), and `4.0s` fade-to-black decay.
    pub fn tick(&mut self, dt: f32) {
        let dt = dt.max(0.0);
        self.elapsed_seconds += dt;

        // Cadence timer (`FUN_00406be0`: decrement by `dt`, when `<= 0` add `1.0f` and stamp)
        self.next_step_timer -= dt;
        if self.next_step_timer <= 0.0 {
            self.next_step_timer += self.step_cadence;
            self.spawn_next_footstep();
        }

        // Exact 4.0s linear decay (`FUN_00406ab0` -> `FUN_004783b0(4.0f)`)
        let decay = dt / FOOTSTEP_DECAY_SECS;
        for step in &mut self.footsteps {
            step.alpha = (step.alpha - decay).max(0.0);
        }
        self.footsteps.retain(|s| s.alpha > 0.01);

        if self.load_completed
            && !self.require_press_to_continue
            && self.elapsed_seconds >= self.min_duration
        {
            self.transition_ready = true;
        }
    }

    /// Stamps the next alternating footstep along the active `FUN_004c8590` path (`FUN_004c8bc0`).
    fn spawn_next_footstep(&mut self) {
        if !self.path_state.active {
            let forced = if self.step_counter == 0 {
                Some(self.path_pattern)
            } else {
                None
            };
            self.init_footstep_path(forced);
        }

        self.step_counter += 1;
        self.path_state.local_y += self.path_state.stride_dy;
        self.path_state.foot_side ^= 1;
        let local_x = if self.path_state.foot_side == 1 {
            -FOOTSTEP_LATERAL_OFFSET
        } else {
            FOOTSTEP_LATERAL_OFFSET
        };

        let cos_a = self.path_state.angle_rad.cos();
        let sin_a = self.path_state.angle_rad.sin();
        let rx = local_x * cos_a - self.path_state.local_y * sin_a;
        let ry = local_x * sin_a + self.path_state.local_y * cos_a;
        let vx = (self.path_state.origin_x + rx) * FOOTSTEP_X_ASPECT;
        let vy = self.path_state.origin_y + ry;

        if vx.abs() > FOOTSTEP_BOUNDS_LIMIT || vy.abs() > FOOTSTEP_BOUNDS_LIMIT {
            self.path_state.active = false;
        }

        self.footsteps.push(FootstepPrint {
            x: vx + 256.0,
            y: vy + 240.0,
            angle: self.path_state.angle_rad,
            scale: self.path_state.scale,
            is_right: self.path_state.foot_side == 1,
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

    /// Renders the retail PC footstep loading screen (`FUN_00406be0`, `FUN_004c87e0`) onto `rast`.
    pub fn render(&self, font: &AtlasFont, rast: &mut TextRasterizer) {
        rast.clear();
        let sprite = FootstepSprite::get();
        let scale_x = rast.width as f32 / 512.0;
        let scale_y = rast.height as f32 / 480.0;

        for step in &self.footsteps {
            let scr_x = (step.x * scale_x).round() as i32;
            let scr_y = (step.y * scale_y).round() as i32;
            let quad_h_px = (step.scale * 2.0) * scale_y;
            let print_scale = quad_h_px / sprite.height.max(1) as f32;
            draw_footstep_silhouette(rast, sprite, scr_x, scr_y, step, print_scale);
        }

        // `"NOW LOADING..."` (`common_msg_e.mes` ID 25) at `(480/640 * W, 424/480 * H)`, `fontWide(16, 24)`
        let text_x = (rast.width as f32 * NOW_LOADING_NORM_X).round() as usize;
        let text_y = (rast.height as f32 * NOW_LOADING_NORM_Y).round() as usize;
        let text_h = (24.0 * scale_y * 0.7).round().max(11.0) as usize;

        if self.load_completed
            && self.elapsed_seconds >= self.min_duration
            && self.require_press_to_continue
        {
            let blink_visible = (self.elapsed_seconds * 2.5).sin() > -0.2;
            if blink_visible {
                font.draw_line_centered(
                    rast,
                    "PRESS [ENTER / A] TO CONTINUE",
                    (rast.width / 2, text_y),
                    text_h,
                    Sh2TextColor::White,
                    true,
                );
            }
        } else {
            font.draw_line_centered(
                rast,
                "NOW LOADING...",
                (text_x.min(rast.width.saturating_sub(90)), text_y),
                text_h,
                Sh2TextColor::White,
                true,
            );
        }
    }
}

/// Blits a footstep silhouette using exact `sh2pc.exe` UV crop `[3/64..61/64]` (`0x00890af0` / `0x00890b10`).
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
    let uv_span = FOOTSTEP_UV_MAX - FOOTSTEP_UV_MIN;

    for y in y_start..=y_end {
        let dy = (y - center_y) as f32 * inv_scale;
        for x in x_start..=x_end {
            let dx = (x - center_x) as f32 * inv_scale;
            let local_x = dx * cos_a - dy * sin_a;
            let local_y = dx * sin_a + dy * cos_a;

            let norm_x = (local_x / spr_w) + 0.5;
            let norm_y = (local_y / spr_h) + 0.5;
            if !(0.0..1.0).contains(&norm_x) || !(0.0..1.0).contains(&norm_y) {
                continue;
            }

            let u = if step.is_right {
                FOOTSTEP_UV_MAX - norm_x * uv_span
            } else {
                FOOTSTEP_UV_MIN + norm_x * uv_span
            };
            let v = FOOTSTEP_UV_MIN + norm_y * uv_span;

            let fx = u * (spr_w - 1.0);
            let fy = v * (spr_h - 1.0);
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
            let interp =
                (a00 * (1.0 - rx) + a10 * rx) * (1.0 - ry) + (a01 * (1.0 - rx) + a11 * rx) * ry;
            if interp <= 0.001 {
                continue;
            }

            let effective_alpha = (interp / 255.0) * step.alpha;
            let dest_idx = (y as usize * rast.width + x as usize) * 4;
            let er = rast.pixels[dest_idx] as f32;
            let eg = rast.pixels[dest_idx + 1] as f32;
            let eb = rast.pixels[dest_idx + 2] as f32;

            rast.pixels[dest_idx] =
                (255.0 * effective_alpha + er * (1.0 - effective_alpha)).min(255.0) as u8;
            rast.pixels[dest_idx + 1] =
                (255.0 * effective_alpha + eg * (1.0 - effective_alpha)).min(255.0) as u8;
            rast.pixels[dest_idx + 2] =
                (255.0 * effective_alpha + eb * (1.0 - effective_alpha)).min(255.0) as u8;
            rast.pixels[dest_idx + 3] = 255;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_loading_screen_retail_cadence_and_decay() {
        let mut loading = LoadingScreen::new_with_seed("Observation Deck", 2.0, true, 0);
        assert_eq!(loading.step_cadence, 1.0);

        // First tick stamps step 1; 1.0s later stamps step 2 (`FUN_00406be0`)
        loading.tick(0.016);
        assert_eq!(loading.footsteps.len(), 1);
        assert!((loading.footsteps[0].alpha - (1.0 - 0.016 / 4.0)).abs() < 1e-4);

        loading.tick(1.0);
        assert_eq!(loading.footsteps.len(), 2);
        // Alternating left/right foot side (`FUN_004c8bc0`)
        assert_ne!(loading.footsteps[0].is_right, loading.footsteps[1].is_right);

        // Confirm gated until load_completed && elapsed >= min_duration
        assert!(!loading.handle_confirm());
        loading.mark_completed();
        assert!(!loading.handle_confirm());
        loading.tick(1.0);
        assert!(loading.handle_confirm());
        assert!(loading.transition_ready);
    }

    #[test]
    fn test_loading_screen_rendering_and_sprite() {
        let mut loading = LoadingScreen::new_with_seed("Restroom Start", 0.5, false, 3);
        loading.tick(0.016);
        loading.mark_completed();

        let font = AtlasFont::load_embedded();
        let mut rast = TextRasterizer::new(640, 480);
        loading.render(&font, &mut rast);

        let non_zero = rast.pixels.iter().step_by(4).filter(|&&p| p > 0).count();
        assert!(non_zero > 50);

        let sprite = FootstepSprite::get();
        assert!(sprite.width > 0 && sprite.height > 0);
    }
}
