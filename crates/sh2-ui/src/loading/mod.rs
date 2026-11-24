//! Authentic Retail PC Footstep Loading Screen (`sh2pc.exe` `FUN_00406ab0`, `FUN_00406be0`,
//! `FUN_004c8500..0x004c8bc0`) and PS2 prototype `now_loading.c` (`0x00298fa0..0x00299170`).
//!
//! Prototype (`SLUS_202.28`) vs Retail PC (`sh2pc.exe`) drift:
//! - `SLUS_202.28` `now_loading.c` (`0x00298fa0..0x00299170`) has no footsteps; it triggers
//!   `ScreenEffectFadeStart(4.0f, 1)` (`0x40800000`) and draws jittered `\c\hNow loading` every
//!   64 frames (`_count_51 & 0x3f == 0`) at `((count * 0xce5fdd) % 5 + 0xfe, (count * 0x2f6f) % 5 + 0xfe)`.
//! - Retail PC `sh2pc.exe` loads `data/etc/effect/footmark.tbn2` (`FUN_004c8500`), stamps
//!   alternating footsteps every `1.0f` second (`FUN_00406be0`), decays them over `4.0f` seconds
//!   (`FUN_00406ab0` -> `FUN_004783b0(4.0f)`), and renders `"NOW LOADING..."` (`common_msg_e.mes`
//!   ID `25`) at `(480/640 * W, 424/480 * H)` with `fontWide(16, 24)`.

mod render;
mod sprite;

pub use sprite::FootstepSprite;

/// Exact footstep cadence period in seconds (`sh2pc.exe` `FUN_00406be0` `0x0062edf4` = `1.0f`).
pub const FOOTSTEP_CADENCE_SECS: f32 = 1.0;

/// Exact footstep fade-to-black decay duration in seconds (`sh2pc.exe` `FUN_00406ab0` ->
/// `FUN_004783b0(4.0f)` `0x40800000`; also `SLUS_202.28` `NowLoadingCheck` `0x00298fc0` = `4.0f`).
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

/// PS2 prototype `NowLoadingDraw` (`SLUS_202.28` `0x00299170`) redraw mask (`_count_51 & 0x3f == 0`).
pub const PS2_NOW_LOADING_PERIOD_MASK: i32 = 0x3f;

/// PS2 prototype `NowLoadingDraw` (`SLUS_202.28` `0x00299170`) X hash multiplier (`_count_51 * 0xce5fdd`).
pub const PS2_NOW_LOADING_X_MULT: i32 = 0x00ce_5fdd;

/// PS2 prototype `NowLoadingDraw` (`SLUS_202.28` `0x00299170`) Y hash multiplier (`_count_51 * 0x2f6f`).
pub const PS2_NOW_LOADING_Y_MULT: i32 = 0x2f6f;

/// PS2 prototype `NowLoadingDraw` (`SLUS_202.28` `0x00299170`) jitter modulus (`% 5`).
pub const PS2_NOW_LOADING_JITTER_MOD: i32 = 5;

/// PS2 prototype `NowLoadingDraw` (`SLUS_202.28` `0x00299170`) base X/Y virtual coordinate (`0xfe` = `254`).
pub const PS2_NOW_LOADING_BASE_XY: i32 = 0xfe;

/// Visual presentation mode for [`LoadingScreen`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LoadingScreenMode {
    /// Retail PC `sh2pc.exe` (`FUN_00406ab0` / `FUN_00406be0` / `FUN_004c8500..0x004c8bc0`)
    /// animated `footmark.tbn2` footprints + `"NOW LOADING..."`.
    #[default]
    RetailPcFootsteps,
    /// PS2 prototype `SLUS_202.28` `now_loading.c` (`NowLoadingCheck` `0x00298fc0`,
    /// `NowLoadingDraw` `0x00299170`): jittered `"Now loading"` stamped every 64 frames
    /// (`_count_51 & 0x3f == 0`) with 4.0s fade decay (`ScreenEffectFadeStart(4.0f, 1)`).
    Ps2NowLoadingJitter,
}

impl LoadingScreenMode {
    /// Human-readable label for debug/preview HUDs.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::RetailPcFootsteps => "Retail PC Footsteps (sh2pc.exe FUN_004c8500)",
            Self::Ps2NowLoadingJitter => "PS2 Prototype Text Jitter (SLUS_202.28 NowLoadingDraw)",
        }
    }

    /// Toggles between [`Self::RetailPcFootsteps`] and [`Self::Ps2NowLoadingJitter`].
    #[must_use]
    pub const fn toggle(self) -> Self {
        match self {
            Self::RetailPcFootsteps => Self::Ps2NowLoadingJitter,
            Self::Ps2NowLoadingJitter => Self::RetailPcFootsteps,
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

    /// Human-readable label for debug/preview HUDs.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::TopEdgeRight => "0: TopEdgeRight (y=-230, x+=128, +135 deg)",
            Self::BottomEdgeRight => "1: BottomEdgeRight (y=+230, x+=128, -135 deg)",
            Self::LeftEdgeDown => "2: LeftEdgeDown (x=-230, y+=128, -45 deg)",
            Self::RightEdgeDown => "3: RightEdgeDown (x=+230, y+=128, -135 deg)",
            Self::BottomEdgeLeft => "4: BottomEdgeLeft (y=+230, x-=128, -45 deg)",
            Self::TopEdgeLeft => "5: TopEdgeLeft (y=-230, x-=128, +45 deg)",
            Self::RightEdgeUp => "6: RightEdgeUp (x=+230, y-=128, +135 deg)",
            Self::LeftEdgeUp => "7: LeftEdgeUp (x=-230, y-=128, +45 deg)",
        }
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

/// Loading screen state machine porting `sh2pc.exe` `FUN_00406ab0` / `FUN_00406be0` /
/// `FUN_004c8590..0x004c8bc0` and `SLUS_202.28` `NowLoadingDraw` (`0x00299170`).
#[derive(Debug, Clone)]
pub struct LoadingScreen {
    pub elapsed_seconds: f32,
    pub footsteps: Vec<FootstepPrint>,
    pub step_counter: usize,
    pub next_step_timer: f32,
    pub step_cadence: f32,
    pub path_pattern: LoadingPathPattern,
    pub mode: LoadingScreenMode,
    /// Set to `true` during [`Self::tick`] when a new footstep print was stamped this frame.
    pub step_spawned_this_tick: bool,
    /// Foot side (`true` = right, `false` = left) of the most recently stamped footstep (`FUN_004c8bc0`).
    pub last_step_is_right: bool,
    pub load_completed: bool,
    pub transition_ready: bool,
    pub min_duration: f32,
    pub require_press_to_continue: bool,
    pub fade_alpha: f32,
    pub target_room_label: String,
    /// PS2 prototype `_count_51` 30 Hz frame counter (`SLUS_202.28` `NowLoadingDraw` `0x00299170`).
    pub ps2_frame_counter: i32,
    /// Current jittered X coordinate in 512×480 virtual space (`(count * 0xce5fdd) % 5 + 0xfe`).
    pub ps2_text_x_512: i32,
    /// Current jittered Y coordinate in 512×480 virtual space (`(count * 0x2f6f) % 5 + 0xfe`).
    pub ps2_text_y_480: i32,
    /// Current `"Now loading"` opacity decaying over 4.0s (`ScreenEffectFadeStart(4.0f, 1)`).
    pub ps2_text_alpha: f32,
    ps2_frame_accum: f32,
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
            mode: LoadingScreenMode::RetailPcFootsteps,
            step_spawned_this_tick: false,
            last_step_is_right: false,
            load_completed: false,
            transition_ready: false,
            min_duration,
            require_press_to_continue: require_press,
            fade_alpha: 1.0,
            target_room_label: target_label.into(),
            ps2_frame_counter: 0,
            ps2_text_x_512: PS2_NOW_LOADING_BASE_XY,
            ps2_text_y_480: PS2_NOW_LOADING_BASE_XY,
            ps2_text_alpha: 1.0,
            ps2_frame_accum: 0.0,
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

    /// Sets the visual mode (`RetailPcFootsteps` vs `Ps2NowLoadingJitter`).
    pub fn with_mode(mut self, mode: LoadingScreenMode) -> Self {
        self.mode = mode;
        self
    }

    /// Marks the background asset load as complete.
    pub fn mark_completed(&mut self) {
        self.load_completed = true;
    }

    /// Returns `Some(is_right)` if a footstep was stamped on the most recent [`Self::tick`].
    #[must_use]
    pub const fn spawned_step_this_tick(&self) -> Option<bool> {
        if self.step_spawned_this_tick {
            Some(self.last_step_is_right)
        } else {
            None
        }
    }

    /// Computes exact PS2 `NowLoadingDraw` (`SLUS_202.28` `0x00299170`) virtual `(x, y)` coordinates
    /// for frame counter `_count_51`.
    #[must_use]
    pub const fn ps2_now_loading_jitter_xy(count: i32) -> (i32, i32) {
        let x = (count.wrapping_mul(PS2_NOW_LOADING_X_MULT) % PS2_NOW_LOADING_JITTER_MOD)
            + PS2_NOW_LOADING_BASE_XY;
        let y = (count.wrapping_mul(PS2_NOW_LOADING_Y_MULT) % PS2_NOW_LOADING_JITTER_MOD)
            + PS2_NOW_LOADING_BASE_XY;
        (x, y)
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

    /// Advances loading screen timers, footstep cadence (`1.0s`), `4.0s` fade-to-black decay,
    /// and PS2 prototype `NowLoadingDraw` (`0x00299170`) 30 Hz jitter counter.
    pub fn tick(&mut self, dt: f32) {
        let dt = dt.max(0.0);
        self.elapsed_seconds += dt;
        self.step_spawned_this_tick = false;

        match self.mode {
            LoadingScreenMode::RetailPcFootsteps => {
                // Cadence timer (`FUN_00406be0`: decrement by `dt`, when `<= 0` add `1.0f` and stamp)
                self.next_step_timer -= dt;
                if self.next_step_timer <= 0.0 {
                    self.next_step_timer += self.step_cadence.max(0.1);
                    self.spawn_next_footstep();
                }

                // Exact 4.0s linear decay (`FUN_00406ab0` -> `FUN_004783b0(4.0f)`)
                let decay = dt / FOOTSTEP_DECAY_SECS;
                for step in &mut self.footsteps {
                    step.alpha = (step.alpha - decay).max(0.0);
                }
                self.footsteps.retain(|s| s.alpha > 0.01);
            }
            LoadingScreenMode::Ps2NowLoadingJitter => {
                // `SLUS_202.28` `NowLoadingDraw` (`0x00299170`): `_count_51` advances at 30 Hz,
                // redrawing jittered `Now loading` whenever `_count_51 & 0x3f == 0` and decaying
                // continuously over `4.0s` (`ScreenEffectFadeStart(4.0f, 1)` at `0x00298fc0`).
                self.ps2_frame_accum += dt * 30.0;
                let mut did_reset = false;
                let mut frames_after_reset = 0u32;
                while self.ps2_frame_accum >= 1.0 {
                    self.ps2_frame_accum -= 1.0;
                    if (self.ps2_frame_counter & PS2_NOW_LOADING_PERIOD_MASK) == 0 {
                        let (jx, jy) = Self::ps2_now_loading_jitter_xy(self.ps2_frame_counter);
                        self.ps2_text_x_512 = jx;
                        self.ps2_text_y_480 = jy;
                        self.ps2_text_alpha = 1.0;
                        did_reset = true;
                        frames_after_reset = 0;
                    } else if did_reset {
                        frames_after_reset += 1;
                    }
                    self.ps2_frame_counter = self.ps2_frame_counter.wrapping_add(1);
                }
                let decay_dt = if did_reset {
                    (frames_after_reset as f32 + self.ps2_frame_accum) / 30.0
                } else {
                    dt
                };
                self.ps2_text_alpha =
                    (self.ps2_text_alpha - decay_dt / FOOTSTEP_DECAY_SECS).max(0.0);
            }
        }

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
        let is_right = self.path_state.foot_side == 1;
        let local_x = if is_right {
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

        self.step_spawned_this_tick = true;
        self.last_step_is_right = is_right;

        self.footsteps.push(FootstepPrint {
            x: vx + 256.0,
            y: vy + 240.0,
            angle: self.path_state.angle_rad,
            scale: self.path_state.scale,
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::atlas_font::AtlasFont;
    use crate::font::TextRasterizer;

    #[test]
    fn test_loading_screen_retail_cadence_decay_and_audio_flag() {
        let mut loading = LoadingScreen::new_with_seed("Observation Deck", 2.0, true, 0);
        assert_eq!(loading.step_cadence, 1.0);
        assert!(!loading.step_spawned_this_tick);

        // First tick stamps step 1 and sets `step_spawned_this_tick` (`FUN_00406be0`)
        loading.tick(0.016);
        assert_eq!(loading.footsteps.len(), 1);
        assert!(loading.step_spawned_this_tick);
        assert_eq!(loading.spawned_step_this_tick(), Some(true));
        assert!((loading.footsteps[0].alpha - (1.0 - 0.016 / 4.0)).abs() < 1e-4);

        // Mid-cadence tick must clear `step_spawned_this_tick`
        loading.tick(0.1);
        assert!(!loading.step_spawned_this_tick);
        assert_eq!(loading.spawned_step_this_tick(), None);

        // 0.9s later (total 1.0s since step 1) stamps step 2 with opposite foot side (`FUN_004c8bc0`)
        loading.tick(0.9);
        assert_eq!(loading.footsteps.len(), 2);
        assert!(loading.step_spawned_this_tick);
        assert_eq!(loading.spawned_step_this_tick(), Some(false));
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
    fn test_ps2_now_loading_jitter_formula_matches_slus_decomp() {
        // Exact `SLUS_202.28` `NowLoadingDraw` (`0x00299170`):
        // count = 0 -> (0 % 5 + 254, 0 % 5 + 254) = (254, 254)
        assert_eq!(LoadingScreen::ps2_now_loading_jitter_xy(0), (254, 254));
        // count = 64 -> x = (64 * 0xce5fdd) % 5 + 254 = 256, y = (64 * 0x2f6f) % 5 + 254 = 258
        let (x64, y64) = LoadingScreen::ps2_now_loading_jitter_xy(64);
        assert_eq!(x64, (64_i32.wrapping_mul(0x00ce_5fdd) % 5) + 0xfe);
        assert_eq!(y64, (64_i32.wrapping_mul(0x2f6f) % 5) + 0xfe);
        assert!((250..=258).contains(&x64));
        assert!((250..=258).contains(&y64));

        // Verify Ps2NowLoadingJitter mode never spawns footsteps and decays at exact 4.0s rate
        // identically across single-step, 60 Hz, 120 Hz, and 240 Hz frame pacing.
        let mut loading = LoadingScreen::new_with_seed("PS2 Test", 1.0, false, 0)
            .with_mode(LoadingScreenMode::Ps2NowLoadingJitter);
        loading.tick(1.0);
        assert!(loading.footsteps.is_empty());
        assert!(!loading.step_spawned_this_tick);
        let expected_after_1s = 1.0 - (29.0 / 30.0) / FOOTSTEP_DECAY_SECS;
        assert!((loading.ps2_text_alpha - expected_after_1s).abs() < 1e-4);

        for &hz in &[60usize, 120, 240] {
            let mut paced = LoadingScreen::new_with_seed("PS2 Paced", 1.0, false, 0)
                .with_mode(LoadingScreenMode::Ps2NowLoadingJitter);
            let dt = 1.0 / hz as f32;
            for _ in 0..hz {
                paced.tick(dt);
            }
            assert!(
                (paced.ps2_text_alpha - expected_after_1s).abs() < 1e-3,
                "Expected exact 4.0s decay parity at {hz} Hz: got {}, expected {expected_after_1s}",
                paced.ps2_text_alpha
            );
        }

        // Render both modes and verify non-empty output
        let font = AtlasFont::load_embedded();
        let mut rast = TextRasterizer::new(640, 480);
        loading.render(&font, &mut rast);
        let non_zero = rast.pixels.iter().step_by(4).filter(|&&p| p > 0).count();
        assert!(non_zero > 20);
    }

    #[test]
    fn test_loading_screen_rendering_and_retail_footmark_sprite() {
        let mut loading = LoadingScreen::new_with_seed("Restroom Start", 0.5, false, 3);
        loading.tick(0.016);
        loading.tick(1.0);
        loading.tick(1.0);
        loading.mark_completed();

        let font = AtlasFont::load_embedded();
        let mut rast = TextRasterizer::new(640, 480);
        loading.render(&font, &mut rast);

        let non_zero = rast.pixels.iter().step_by(4).filter(|&&p| p > 0).count();
        assert!(non_zero > 50);

        if let Ok(out_dir) = std::env::var("SH2_PREVIEW_OUT") {
            let _ = std::fs::create_dir_all(&out_dir);
            let _ = image::save_buffer(
                format!("{out_dir}/loading_screen_footsteps.png"),
                &rast.pixels,
                rast.width as u32,
                rast.height as u32,
                image::ColorType::Rgba8,
            );

            let mut ps2_loading = LoadingScreen::new_with_seed("PS2 Jitter", 1.0, false, 0)
                .with_mode(LoadingScreenMode::Ps2NowLoadingJitter);
            ps2_loading.tick(0.033);
            let mut ps2_rast = TextRasterizer::new(640, 480);
            ps2_loading.render(&font, &mut ps2_rast);
            let _ = image::save_buffer(
                format!("{out_dir}/loading_screen_ps2_jitter.png"),
                &ps2_rast.pixels,
                ps2_rast.width as u32,
                ps2_rast.height as u32,
                image::ColorType::Rgba8,
            );
        }

        let sprite = FootstepSprite::get();
        assert!(sprite.width > 0 && sprite.height > 0);
        assert!(sprite.rgba.as_chunks::<4>().0.iter().any(|px| px[3] > 128));
    }

    #[test]
    fn test_retail_footmark_tbn2_asset_loads_when_present() {
        let retail_path = std::path::Path::new(
            "../../Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2/data/etc/effect/footmark.tbn2",
        );
        if !retail_path.is_file() {
            return;
        }
        let sprite = FootstepSprite::load_from_retail_assets()
            .expect("retail data/etc/effect/footmark.tbn2 must resolve and decode");
        assert!(sprite.from_retail_asset);
        assert_eq!(sprite.width, 64);
        assert_eq!(sprite.height, 64);
        assert_eq!(sprite.rgba.len(), 64 * 64 * 4);
        let pixels = sprite.rgba.as_chunks::<4>().0;
        assert!(pixels.iter().any(|px| px[3] > 128));
        // Verify full RGBA tread detail is preserved (not overwritten with flat solid white):
        // retail `footmark.tbn2` has opaque interior pixels (`alpha > 200`) with dark sole-tread RGB.
        assert!(pixels.iter().any(|px| px[3] > 200 && px[0] < 128));
        assert!(pixels.iter().any(|px| px[3] > 200 && px[0] > 160));
    }
}
