//! Real-time `.cld` raycast acoustic wall/door occlusion and 100 ms click-free low-pass filter DSP (`SH2-84`).
//!
//! When a 3D sound effect emitter is occluded by one or more solid `.cld` wall segments or floor slabs
//! between the source and James's/Maria's head position:
//! - Applies a dynamic 2-pole Butterworth TPT State-Variable low-pass filter (`650–850 Hz` cutoff) and
//!   direct-path attenuation (`-6.0 dB` to `-10.0 dB`) to the dry signal.
//! - Preserves the wet corridor reverb send (`wet_reverb_bleed_scale = 1.0` for single-wall hallway corners,
//!   `>= 0.85` for deep multi-wall occlusion) so monsters around apartment/hospital corners sound authentically
//!   muffled through walls while their echoes still carry down hallways (`RoomAcousticsProfile` from `SH2-55`).
//! - Smoothly interpolates filter cutoff in logarithmic frequency space over `100 ms` (`0.100 s`) to eliminate
//!   zipper noise and transient clicks when crossing doorway thresholds.

use glam::Vec3;
use kira::Frame;
use sh2_core::collision::{CollisionController, CollisionWorld};
use sh2_core::{listener_head_position, AcousticOcclusionHit};

/// Transparent dry-path low-pass filter cutoff (`Hz`) when line-of-sight is unoccluded.
pub const UNOCCLUDED_CUTOFF_HZ: f32 = 20_000.0;

/// Minimum dry-path low-pass filter cutoff (`Hz`) under heavy / multi-wall `.cld` occlusion (`SH2-84`).
pub const OCCLUDED_MIN_CUTOFF_HZ: f32 = 650.0;

/// Maximum dry-path low-pass filter cutoff (`Hz`) under single wooden-door / partition occlusion (`SH2-84`).
pub const OCCLUDED_MAX_CUTOFF_HZ: f32 = 850.0;

/// Direct-path dry attenuation (`dB`) for a single wooden door / light partition (`-6.0 dB`).
pub const OCCLUDED_MIN_ATTENUATION_DB: f32 = -6.0;

/// Direct-path dry attenuation (`dB`) for multi-wall / thick masonry occlusion (`-10.0 dB`).
pub const OCCLUDED_MAX_ATTENUATION_DB: f32 = -10.0;

/// Smooth cutoff and dry-gain interpolation window (`0.100 s` = `100 ms`) across doorway thresholds (`SH2-84`).
pub const OCCLUSION_SMOOTH_TIME_SECS: f32 = 0.100;

/// Evaluated dry/wet acoustic occlusion parameters derived from a `.cld` 3D line-of-sight raycast (`SH2-84`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OcclusionDspParams {
    /// Whether at least one solid `.cld` wall, door, or floor slab blocks direct line-of-sight.
    pub occluded: bool,
    /// Number of distinct `.cld` wall/door planes intersected.
    pub wall_hit_count: u32,
    /// Number of distinct `.cld` floor/ceiling slabs intersected vertically.
    pub slab_hit_count: u32,
    /// Target dry-path low-pass cutoff frequency in `Hz` (`20_000.0` clear; `650.0..=850.0` occluded).
    pub dry_cutoff_hz: f32,
    /// Target direct-path dry attenuation in decibels (`0.0` clear; `-10.0..=-6.0` occluded).
    pub dry_attenuation_db: f32,
    /// Linear amplitude multiplier (`10^(dB / 20)`) applied to the dry direct signal (`1.0` clear; `0.316..=0.501` occluded).
    pub dry_gain_scale: f32,
    /// Linear amplitude multiplier applied to the wet corridor reverb send (`1.0` clear or 1-wall corner; `0.85..=1.0` multi-wall).
    pub wet_reverb_bleed_scale: f32,
}

impl Default for OcclusionDspParams {
    fn default() -> Self {
        Self::unoccluded()
    }
}

impl OcclusionDspParams {
    /// Unoccluded direct line-of-sight parameters (`20 kHz` cutoff, `0 dB` attenuation, `100%` wet bleed).
    #[inline]
    pub const fn unoccluded() -> Self {
        Self {
            occluded: false,
            wall_hit_count: 0,
            slab_hit_count: 0,
            dry_cutoff_hz: UNOCCLUDED_CUTOFF_HZ,
            dry_attenuation_db: 0.0,
            dry_gain_scale: 1.0,
            wet_reverb_bleed_scale: 1.0,
        }
    }
}

/// Converts decibels (`dB`) to linear amplitude scale (`10^(dB / 20)`).
#[inline]
pub fn db_to_linear(db: f32) -> f32 {
    10.0_f32.powf(db * 0.05)
}

/// Maps a `.cld` [`AcousticOcclusionHit`] into target dry low-pass cutoff (`650–850 Hz`),
/// direct-path attenuation (`-6 dB` to `-10 dB`), and wet corridor reverb bleed (`SH2-84`).
#[inline]
pub fn evaluate_occlusion_dsp(hit: &AcousticOcclusionHit) -> OcclusionDspParams {
    if !hit.is_occluded() {
        return OcclusionDspParams::unoccluded();
    }

    // `barrier_density` is `0.85` for 1 wooden door (`.cld` mat 7), `1.0` for 1 concrete wall,
    // `1.15` for 1 heavy metal/stone wall, and `>= 2.0` for 2+ distinct room walls/slabs.
    let severity = ((hit.barrier_density - 0.85) / 1.15).clamp(0.0, 1.0);
    let dry_cutoff_hz = (OCCLUDED_MAX_CUTOFF_HZ
        - (OCCLUDED_MAX_CUTOFF_HZ - OCCLUDED_MIN_CUTOFF_HZ) * severity)
        .clamp(OCCLUDED_MIN_CUTOFF_HZ, OCCLUDED_MAX_CUTOFF_HZ);
    let dry_attenuation_db = (OCCLUDED_MIN_ATTENUATION_DB
        + (OCCLUDED_MAX_ATTENUATION_DB - OCCLUDED_MIN_ATTENUATION_DB) * severity)
        .clamp(OCCLUDED_MAX_ATTENUATION_DB, OCCLUDED_MIN_ATTENUATION_DB);
    let dry_gain_scale = db_to_linear(dry_attenuation_db);

    // Preserve wet corridor reverb tail (`1.0` around a single hallway corner/door; `0.88` across multiple rooms)
    let wet_reverb_bleed_scale = if hit.total_hits() <= 1 {
        1.0
    } else {
        (1.0 - 0.08 * (hit.total_hits().saturating_sub(1) as f32)).clamp(0.82, 1.0)
    };

    OcclusionDspParams {
        occluded: true,
        wall_hit_count: hit.wall_hit_count,
        slab_hit_count: hit.slab_hit_count,
        dry_cutoff_hz,
        dry_attenuation_db,
        dry_gain_scale,
        wet_reverb_bleed_scale,
    }
}

/// Casts a 3D acoustic line-of-sight ray from `emitter_pos` to James's/Maria's head against `world`
/// and returns the evaluated [`OcclusionDspParams`] (`SH2-84`).
#[inline]
pub fn evaluate_world_occlusion(
    world: &CollisionWorld,
    emitter_pos: Vec3,
    player_feet_pos: Vec3,
    is_maria: bool,
) -> OcclusionDspParams {
    let head_pos = listener_head_position(player_feet_pos, is_maria);
    let hit = world.raycast_acoustic_occlusion(emitter_pos, head_pos);
    evaluate_occlusion_dsp(&hit)
}

/// Casts a 3D acoustic line-of-sight ray against a [`CollisionController`]'s world (`SH2-84`).
#[inline]
pub fn evaluate_controller_occlusion(
    controller: &CollisionController,
    emitter_pos: Vec3,
    player_feet_pos: Vec3,
    is_maria: bool,
) -> OcclusionDspParams {
    evaluate_world_occlusion(&controller.world, emitter_pos, player_feet_pos, is_maria)
}

/// Zero-allocation stereo 2-pole Butterworth TPT State-Variable low-pass filter with `100 ms`
/// continuous log-frequency cutoff & gain interpolation for click-free doorway transitions (`SH2-84`).
#[derive(Debug, Clone, Copy)]
pub struct OcclusionFilterState {
    // Log-Hz ramp state (`ln(Hz)`) for perceptual click-free 100 ms transitions
    start_log_cutoff: f32,
    current_log_cutoff: f32,
    target_log_cutoff: f32,
    // Linear dry occlusion gain ramp state
    start_dry_gain: f32,
    current_dry_gain: f32,
    target_dry_gain: f32,
    // Elapsed seconds in active 100 ms transition (`0.0..=OCCLUSION_SMOOTH_TIME_SECS`)
    ramp_elapsed_secs: f32,
    ramp_duration_secs: f32,
    // Cached Cytomic TPT SVF coefficients (`Butterworth Q = 1 / sqrt(2)`)
    svf_a1: f32,
    svf_a2: f32,
    svf_a3: f32,
    coeff_countdown: u8,
    last_sample_rate: f32,
    // Stereo integrator states (`Left`, `Right`)
    ic1eq_l: f32,
    ic2eq_l: f32,
    ic1eq_r: f32,
    ic2eq_r: f32,
}

impl Default for OcclusionFilterState {
    fn default() -> Self {
        Self::new(UNOCCLUDED_CUTOFF_HZ, 1.0)
    }
}

impl OcclusionFilterState {
    /// Creates an occlusion filter snapped immediately to `(initial_cutoff_hz, initial_dry_gain)`.
    #[inline]
    pub fn new(initial_cutoff_hz: f32, initial_dry_gain: f32) -> Self {
        let clamped_fc = initial_cutoff_hz.clamp(200.0, UNOCCLUDED_CUTOFF_HZ);
        let log_fc = clamped_fc.ln();
        let gain = initial_dry_gain.clamp(0.0, 2.0);
        let mut s = Self {
            start_log_cutoff: log_fc,
            current_log_cutoff: log_fc,
            target_log_cutoff: log_fc,
            start_dry_gain: gain,
            current_dry_gain: gain,
            target_dry_gain: gain,
            ramp_elapsed_secs: OCCLUSION_SMOOTH_TIME_SECS,
            ramp_duration_secs: OCCLUSION_SMOOTH_TIME_SECS,
            svf_a1: 1.0,
            svf_a2: 0.0,
            svf_a3: 0.0,
            coeff_countdown: 0,
            last_sample_rate: 48_000.0,
            ic1eq_l: 0.0,
            ic2eq_l: 0.0,
            ic1eq_r: 0.0,
            ic2eq_r: 0.0,
        };
        s.recompute_svf_coefficients(48_000.0);
        s
    }

    /// Updates the target dry cutoff (`Hz`) and direct-path gain scale, smoothly interpolating
    /// over `OCCLUSION_SMOOTH_TIME_SECS` (`100 ms`) whenever the target changes.
    #[inline]
    pub fn set_target(&mut self, cutoff_hz: f32, dry_gain_scale: f32) {
        let clamped_fc = cutoff_hz.clamp(200.0, UNOCCLUDED_CUTOFF_HZ);
        let new_log_fc = clamped_fc.ln();
        let new_gain = dry_gain_scale.clamp(0.0, 2.0);

        if (new_log_fc - self.target_log_cutoff).abs() > 1e-4
            || (new_gain - self.target_dry_gain).abs() > 1e-4
        {
            self.start_log_cutoff = self.current_log_cutoff;
            self.target_log_cutoff = new_log_fc;
            self.start_dry_gain = self.current_dry_gain;
            self.target_dry_gain = new_gain;
            self.ramp_elapsed_secs = 0.0;
        }
    }

    /// Immediately snaps both cutoff and gain to the given values without ramping.
    #[inline]
    pub fn snap_to(&mut self, cutoff_hz: f32, dry_gain_scale: f32, sample_rate: f32) {
        let clamped_fc = cutoff_hz.clamp(200.0, UNOCCLUDED_CUTOFF_HZ);
        let log_fc = clamped_fc.ln();
        let gain = dry_gain_scale.clamp(0.0, 2.0);
        self.start_log_cutoff = log_fc;
        self.current_log_cutoff = log_fc;
        self.target_log_cutoff = log_fc;
        self.start_dry_gain = gain;
        self.current_dry_gain = gain;
        self.target_dry_gain = gain;
        self.ramp_elapsed_secs = self.ramp_duration_secs;
        self.recompute_svf_coefficients(sample_rate);
    }

    /// Returns the instantaneous smoothed cutoff frequency in `Hz`.
    #[inline]
    pub fn current_cutoff_hz(&self) -> f32 {
        self.current_log_cutoff.exp()
    }

    /// Returns the instantaneous smoothed direct-path gain multiplier.
    #[inline]
    pub fn current_dry_gain(&self) -> f32 {
        self.current_dry_gain
    }

    #[inline]
    fn recompute_svf_coefficients(&mut self, sample_rate: f32) {
        let fs = sample_rate.max(8_000.0);
        self.last_sample_rate = fs;
        // Clamp cutoff safely below Nyquist (`0.46 * fs`) for unconditional bilinear stability
        let fc = self.current_log_cutoff.exp().clamp(150.0, 0.46 * fs);
        let g = (std::f32::consts::PI * fc / fs).tan();
        // Butterworth maximally-flat damping factor `k = 1 / Q = sqrt(2)`
        const BUTTERWORTH_K: f32 = std::f32::consts::SQRT_2;
        let a1 = 1.0 / (1.0 + g * (g + BUTTERWORTH_K));
        let a2 = g * a1;
        let a3 = g * a2;
        self.svf_a1 = a1;
        self.svf_a2 = a2;
        self.svf_a3 = a3;
    }

    /// Processes a single stereo [`Frame`] on the real-time audio thread with zero heap allocations.
    #[inline]
    pub fn process_frame(&mut self, input: Frame, sample_rate: f32) -> Frame {
        let fs = sample_rate.max(8_000.0);
        let dt = 1.0 / fs;

        if self.ramp_elapsed_secs < self.ramp_duration_secs {
            self.ramp_elapsed_secs = (self.ramp_elapsed_secs + dt).min(self.ramp_duration_secs);
            let t = (self.ramp_elapsed_secs / self.ramp_duration_secs).clamp(0.0, 1.0);
            // Raised-cosine `C^1` smoothstep eliminates slope discontinuities at ramp start/end
            let smooth_t = 0.5 - 0.5 * (std::f32::consts::PI * t).cos();
            self.current_log_cutoff =
                self.start_log_cutoff + (self.target_log_cutoff - self.start_log_cutoff) * smooth_t;
            self.current_dry_gain =
                self.start_dry_gain + (self.target_dry_gain - self.start_dry_gain) * smooth_t;

            if self.coeff_countdown == 0 {
                self.recompute_svf_coefficients(fs);
                self.coeff_countdown = 15;
            } else {
                self.coeff_countdown -= 1;
            }
        } else if (fs - self.last_sample_rate).abs() > 1.0 {
            self.recompute_svf_coefficients(fs);
        }

        // Left channel 2-pole TPT SVF low-pass
        let v3_l = input.left - self.ic2eq_l;
        let v1_l = self.svf_a1 * self.ic1eq_l + self.svf_a2 * v3_l;
        let v2_l = self.ic2eq_l + self.svf_a2 * self.ic1eq_l + self.svf_a3 * v3_l;
        self.ic1eq_l = 2.0 * v1_l - self.ic1eq_l;
        self.ic2eq_l = 2.0 * v2_l - self.ic2eq_l;

        // Right channel 2-pole TPT SVF low-pass
        let v3_r = input.right - self.ic2eq_r;
        let v1_r = self.svf_a1 * self.ic1eq_r + self.svf_a2 * v3_r;
        let v2_r = self.ic2eq_r + self.svf_a2 * self.ic1eq_r + self.svf_a3 * v3_r;
        self.ic1eq_r = 2.0 * v1_r - self.ic1eq_r;
        self.ic2eq_r = 2.0 * v2_r - self.ic2eq_r;

        Frame::new(v2_l * self.current_dry_gain, v2_r * self.current_dry_gain)
    }
}
