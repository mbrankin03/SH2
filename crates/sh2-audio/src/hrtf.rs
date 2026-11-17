//! Zero-allocation Binaural HRTF (Head-Related Transfer Function) 3D spatializer (`SH2-84`).
//!
//! Tailored for headphones and IEMs (desktop interfaces such as Arturia MiniFuse 2 and handhelds
//! such as AYN Thor), synthesizing full 3D directional localization cues without heap allocations:
//! 1. **Interaural Time Difference (ITD)**: Woodworth spherical-head diffraction model
//!    ($\Delta t = \frac{a}{c}(|\alpha| + \sin|\alpha|)$, $a = 8.75\text{ cm}$, $c = 343\text{ m/s}$,
//!    $\Delta t_{\max} \approx 655.8\ \mu\text{s}$) via a stack-allocated fractional delay line.
//! 2. **Interaural Level Difference (ILD)**: Frequency-dependent spherical head-shadowing TPT high-shelf
//!    filter (`1.4 kHz` corner), preserving natural low-frequency diffraction around the skull while
//!    attenuating contralateral high frequencies up to `-11 dB`.
//! 3. **Pinna Elevation & Front/Back Cues**: Concha interference notch migration (`4.4 kHz` below ->
//!    `7.2 kHz` horizon -> `10.0 kHz` overhead), Blauert `~5.5–8 kHz` overhead presence vs sub-floor
//!    warmth, and rear pinna-rim high-frequency shadowing (`> 3.2 kHz`) so players accurately perceive
//!    enemies **above, below, behind, or beside** them in Toluca Prison, Labyrinth shafts, and Brookhaven stairwells.

use glam::Vec3;
use kira::Frame;

/// Effective human acoustic head radius in meters ($8.75\text{ cm}$).
pub const HEAD_RADIUS_METERS: f32 = 0.0875;

/// Speed of sound in air at room temperature ($343.0\text{ m/s}$).
pub const SPEED_OF_SOUND_MPS: f32 = 343.0;

/// Fixed per-ear circular delay buffer length in samples (`256` samples covers Woodworth max ITD `655.8 µs`
/// = `125.9` samples up to `192 kHz` desktop interfaces like Arturia MiniFuse 2 with generous margin).
const HRTF_DELAY_BUFFER_LEN: usize = 256;
const HRTF_DELAY_BUFFER_MASK: usize = HRTF_DELAY_BUFFER_LEN - 1;

/// Listener-relative 3D spherical coordinates and Woodworth ITD for a point-source emitter (`SH2-84`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HrtfSphericalAngles {
    /// Horizontal azimuth angle in radians (`0.0` = front, `+PI/2` = right ear, `-PI/2` = left ear, `±PI` = behind).
    pub azimuth_rad: f32,
    /// Vertical elevation angle in radians (`+PI/2` = directly above head, `0.0` = horizon, `-PI/2` = directly below).
    pub elevation_rad: f32,
    /// Projection onto listener's right-ear interaural axis in `[-1.0, 1.0]` (`+1.0` = right ear, `-1.0` = left ear).
    pub lateral_sin: f32,
    /// Projection onto listener's forward look axis in `[-1.0, 1.0]` (`+1.0` = front, `-1.0` = behind).
    pub front_cos: f32,
    /// Signed Woodworth interaural time difference in seconds (`> 0` = right ear leads / left ear delayed).
    pub itd_seconds: f32,
}

impl Default for HrtfSphericalAngles {
    fn default() -> Self {
        Self {
            azimuth_rad: 0.0,
            elevation_rad: 0.0,
            lateral_sin: 0.0,
            front_cos: 1.0,
            itd_seconds: 0.0,
        }
    }
}

/// Evaluates Woodworth's spherical-head Interaural Time Difference (in seconds) from `lateral_sin`
/// ($\sin\alpha \in [-1.0, 1.0]$, where $+1.0$ is full right ear and $-1.0$ is full left ear):
///
/// $$\Delta t(\alpha) = \operatorname{sign}(\alpha)\,\frac{a}{c}\,\left(|\alpha| + \sin|\alpha|\right)$$
#[inline]
pub fn compute_woodworth_itd_seconds(lateral_sin: f32) -> f32 {
    if !lateral_sin.is_finite() {
        return 0.0;
    }
    let s = lateral_sin.clamp(-1.0, 1.0);
    let abs_s = s.abs();
    let abs_alpha = abs_s.asin();
    let mag = (HEAD_RADIUS_METERS / SPEED_OF_SOUND_MPS) * (abs_alpha + abs_s);
    mag.copysign(s)
}

/// Computes listener-relative [`HrtfSphericalAngles`] from Silent Hill 2 world coordinates (`+Y` downward,
/// `500.0` SH2 units = `1.0 m`), matching `sfx_bank::compute_se_call_pos_distance_f`'s horizontal ear basis
/// (`SLUS_202.28` `SeCallPosDirection` `0x00192990` / `VbWvsMatrix` `0x010e59d0`) 1:1 so toggling between
/// Classic PS2 Stereo and Binaural HRTF preserves exact screen-space Left/Right ear polarity.
///
/// - `listener_head_pos`: Listener ear/head world position (e.g. `listener_head_position(player_pos, is_maria)`).
/// - `listener_yaw`: Active camera / listener horizontal facing yaw in radians (`0.0` = `+Z`, `+PI/2` = `+X`).
/// - `listener_pitch`: Active camera / listener vertical pitch in radians (`0.0` = level, `< 0` = looking up `-Y`, `> 0` = looking down `+Y`).
/// - `source_pos`: 3D world position of the sound emitter.
pub fn compute_listener_spherical_angles(
    listener_head_pos: Vec3,
    listener_yaw: f32,
    listener_pitch: f32,
    source_pos: Vec3,
) -> HrtfSphericalAngles {
    if !listener_head_pos.is_finite()
        || !source_pos.is_finite()
        || !listener_yaw.is_finite()
        || !listener_pitch.is_finite()
    {
        return HrtfSphericalAngles::default();
    }
    let delta = source_pos - listener_head_pos;
    let len_sq = delta.length_squared();
    if !len_sq.is_finite() || len_sq <= 1e-4 {
        return HrtfSphericalAngles::default();
    }

    let u = delta * len_sq.sqrt().recip();
    let (sin_y, cos_y) = listener_yaw.sin_cos();
    let (sin_p, cos_p) = listener_pitch.sin_cos();

    // Right-handed `+Y`-down basis matching `Mat4::look_at_rh(..., Vec3::new(0.0, -1.0, 0.0))`
    // and `compute_se_call_pos_distance_f` (`0x00192990` / `0x517D10`):
    // Forward = (sin(yaw), 0, cos(yaw)), World Up = (0, -1, 0), Right = fwd × world_up = (cos(yaw), 0, -sin(yaw))
    let fwd_horiz = Vec3::new(sin_y, 0.0, cos_y);
    let right = Vec3::new(cos_y, 0.0, -sin_y);
    let world_up = Vec3::new(0.0, -1.0, 0.0);

    // Pitch > 0 tilts forward toward +Y (downward), matching `ModernCameraRig`
    let fwd = fwd_horiz * cos_p - world_up * sin_p;
    let up = world_up * cos_p + fwd_horiz * sin_p;

    let x_right = u.dot(right).clamp(-1.0, 1.0);
    let y_up = u.dot(up).clamp(-1.0, 1.0);
    let z_front = u.dot(fwd).clamp(-1.0, 1.0);

    let azimuth_rad = x_right.atan2(z_front);
    let elevation_rad = y_up.asin();
    let itd_seconds = compute_woodworth_itd_seconds(x_right);

    HrtfSphericalAngles {
        azimuth_rad,
        elevation_rad,
        lateral_sin: x_right,
        front_cos: z_front,
        itd_seconds,
    }
}

/// Reconstructs [`HrtfSphericalAngles`] directly from `(azimuth_rad, elevation_rad)` on the audio thread.
#[inline]
pub fn angles_from_azimuth_elevation(azimuth_rad: f32, elevation_rad: f32) -> HrtfSphericalAngles {
    if !azimuth_rad.is_finite() || !elevation_rad.is_finite() {
        return HrtfSphericalAngles::default();
    }
    let clamped_el = elevation_rad.clamp(-std::f32::consts::FRAC_PI_2, std::f32::consts::FRAC_PI_2);
    let (sin_az, cos_az) = azimuth_rad.sin_cos();
    let (_sin_el, cos_el) = clamped_el.sin_cos();
    let lateral_sin = (sin_az * cos_el).clamp(-1.0, 1.0);
    let front_cos = (cos_az * cos_el).clamp(-1.0, 1.0);
    HrtfSphericalAngles {
        azimuth_rad,
        elevation_rad: clamped_el,
        lateral_sin,
        front_cos,
        itd_seconds: compute_woodworth_itd_seconds(lateral_sin),
    }
}

/// Compact Cytomic TPT 2-pole State-Variable Filter state used for pinna notch & shelf stages.
#[derive(Debug, Clone, Copy, Default)]
struct Svf2Pole {
    ic1eq: f32,
    ic2eq: f32,
}

impl Svf2Pole {
    /// Evaluates `(low_pass, band_pass, high_pass)` outputs simultaneously for `v0`.
    #[inline]
    fn step(&mut self, v0: f32, g: f32, k: f32) -> (f32, f32, f32) {
        let a1 = 1.0 / (1.0 + g * (g + k));
        let a2 = g * a1;
        let a3 = g * a2;
        let v3 = v0 - self.ic2eq;
        let v1 = a1 * self.ic1eq + a2 * v3;
        let v2 = self.ic2eq + a2 * self.ic1eq + a3 * v3;
        self.ic1eq = 2.0 * v1 - self.ic1eq;
        self.ic2eq = 2.0 * v2 - self.ic2eq;
        let hp = v0 - k * v1 - v2;
        (v2, v1, hp)
    }
}

/// Zero-allocation Binaural HRTF real-time audio processor (`SH2-84`).
#[derive(Debug, Clone)]
pub struct BinauralHrtfProcessor {
    // Circular fractional delay buffers (inline on struct — zero heap allocation)
    delay_buf_l: [f32; HRTF_DELAY_BUFFER_LEN],
    delay_buf_r: [f32; HRTF_DELAY_BUFFER_LEN],
    write_idx: usize,
    // Smoothed per-ear delay in samples (`2.0..=120.0`)
    cur_delay_samples_l: f32,
    cur_delay_samples_r: f32,
    target_delay_samples_l: f32,
    target_delay_samples_r: f32,
    // Smoothed ILD & pinna spectral parameters
    cur_low_gain_l: f32,
    cur_high_gain_l: f32,
    cur_low_gain_r: f32,
    cur_high_gain_r: f32,
    target_low_gain_l: f32,
    target_high_gain_l: f32,
    target_low_gain_r: f32,
    target_high_gain_r: f32,
    // Pinna filter states & cached coefficients
    pinna_notch_svf: Svf2Pole,
    pinna_elev_svf: Svf2Pole,
    pinna_rear_svf: Svf2Pole,
    head_shadow_l_z1: f32,
    head_shadow_r_z1: f32,
    notch_g: f32,
    notch_k: f32,
    elev_g: f32,
    cur_elev_high_scale: f32,
    cur_elev_low_scale: f32,
    target_elev_high_scale: f32,
    target_elev_low_scale: f32,
    rear_g: f32,
    cur_rear_high_scale: f32,
    target_rear_high_scale: f32,
    head_lp_alpha: f32,
    sample_smooth: f32,
    last_az: f32,
    last_el: f32,
    last_fs: f32,
}

impl Default for BinauralHrtfProcessor {
    fn default() -> Self {
        Self::new()
    }
}

impl BinauralHrtfProcessor {
    /// Creates a zero-allocation binaural HRTF processor centered straight ahead (`az = 0, el = 0`).
    pub fn new() -> Self {
        let mut p = Self {
            delay_buf_l: [0.0; HRTF_DELAY_BUFFER_LEN],
            delay_buf_r: [0.0; HRTF_DELAY_BUFFER_LEN],
            write_idx: 0,
            cur_delay_samples_l: 2.0,
            cur_delay_samples_r: 2.0,
            target_delay_samples_l: 2.0,
            target_delay_samples_r: 2.0,
            cur_low_gain_l: 1.0,
            cur_high_gain_l: 1.0,
            cur_low_gain_r: 1.0,
            cur_high_gain_r: 1.0,
            target_low_gain_l: 1.0,
            target_high_gain_l: 1.0,
            target_low_gain_r: 1.0,
            target_high_gain_r: 1.0,
            pinna_notch_svf: Svf2Pole::default(),
            pinna_elev_svf: Svf2Pole::default(),
            pinna_rear_svf: Svf2Pole::default(),
            head_shadow_l_z1: 0.0,
            head_shadow_r_z1: 0.0,
            notch_g: 0.5,
            notch_k: 0.35,
            elev_g: 0.4,
            cur_elev_high_scale: 1.0,
            cur_elev_low_scale: 1.0,
            target_elev_high_scale: 1.0,
            target_elev_low_scale: 1.0,
            rear_g: 0.25,
            cur_rear_high_scale: 1.0,
            target_rear_high_scale: 1.0,
            head_lp_alpha: 0.16,
            sample_smooth: 0.0025,
            last_az: f32::NAN,
            last_el: f32::NAN,
            last_fs: 48_000.0,
        };
        p.update_angles(0.0, 0.0, 48_000.0, true);
        p
    }

    /// Updates target HRTF filter coefficients and ITD/ILD targets from `(azimuth_rad, elevation_rad)`.
    #[inline]
    pub fn update_angles(
        &mut self,
        azimuth_rad: f32,
        elevation_rad: f32,
        sample_rate: f32,
        snap_immediately: bool,
    ) {
        if !azimuth_rad.is_finite() || !elevation_rad.is_finite() || !sample_rate.is_finite() {
            return;
        }
        let fs = sample_rate.clamp(8_000.0, 384_000.0);
        if !snap_immediately
            && (azimuth_rad - self.last_az).abs() < 1e-4
            && (elevation_rad - self.last_el).abs() < 1e-4
            && (fs - self.last_fs).abs() < 1.0
        {
            return;
        }
        self.last_az = azimuth_rad;
        self.last_el = elevation_rad;
        self.last_fs = fs;
        self.sample_smooth = (0.0025 * (48_000.0 / fs)).clamp(0.0004, 0.02);

        let angles = angles_from_azimuth_elevation(azimuth_rad, elevation_rad);

        // 1. Woodworth spherical-head ITD sample delays (`base = 2.0` samples for clean interpolation margin)
        let itd_samples = (angles.itd_seconds.abs() * fs).min((HRTF_DELAY_BUFFER_LEN - 8) as f32);
        if angles.itd_seconds >= 0.0 {
            // Source in right hemisphere: right ear arrives first, left ear delayed
            self.target_delay_samples_r = 2.0;
            self.target_delay_samples_l = 2.0 + itd_samples;
        } else {
            // Source in left hemisphere: left ear arrives first, right ear delayed
            self.target_delay_samples_l = 2.0;
            self.target_delay_samples_r = 2.0 + itd_samples;
        }

        // 2. Frequency-dependent spherical head-shadow ILD (`fc = 1400 Hz` diffraction split)
        // Low frequencies diffract around the head (`±2 dB`); high frequencies experience up to `-11 dB` shadow.
        let lat = angles.lateral_sin; // +1 = right ear, -1 = left ear
        if lat >= 0.0 {
            self.target_low_gain_r = 1.0 + 0.12 * lat;
            self.target_high_gain_r = 1.0 + 0.24 * lat;
            self.target_low_gain_l = 1.0 - 0.26 * lat;
            self.target_high_gain_l = 1.0 - 0.72 * lat; // -11.0 dB HF head shadow at +90 deg
        } else {
            let a = -lat;
            self.target_low_gain_l = 1.0 + 0.12 * a;
            self.target_high_gain_l = 1.0 + 0.24 * a;
            self.target_low_gain_r = 1.0 - 0.26 * a;
            self.target_high_gain_r = 1.0 - 0.72 * a; // -11.0 dB HF head shadow at -90 deg
        }

        let head_fc = 1_400.0_f32.min(0.42 * fs);
        self.head_lp_alpha = 1.0 - (-2.0 * std::f32::consts::PI * head_fc / fs).exp();

        // 3. Pinna concha notch + vertical Above / Below spectral tilt (`elev_norm in -1..+1`)
        let elev_norm = (angles.elevation_rad / std::f32::consts::FRAC_PI_2).clamp(-1.0, 1.0);
        let notch_fc = (7_200.0 + 2_800.0 * elev_norm).clamp(3_800.0, 0.44 * fs);
        self.notch_g = (std::f32::consts::PI * notch_fc / fs).tan();
        self.notch_k = 0.32; // Q ≈ 3.1 pinna notch

        let elev_split_fc = 4_800.0_f32.min(0.42 * fs);
        self.elev_g = (std::f32::consts::PI * elev_split_fc / fs).tan();
        if elev_norm >= 0.0 {
            // Above listener: overhead Blauert 5–10 kHz air boost (`+5.0 dB` max) + slightly leaner low-end
            self.target_elev_high_scale = 1.0 + 0.78 * elev_norm;
            self.target_elev_low_scale = 1.0 - 0.14 * elev_norm;
        } else {
            // Below listener: upper-treble floor/body damping (`-6.5 dB`) + subtle sub-grate warmth (`+1.8 dB`)
            let b = -elev_norm;
            self.target_elev_high_scale = 1.0 - 0.53 * b;
            self.target_elev_low_scale = 1.0 + 0.23 * b;
        }

        // 4. Front vs Behind pinna-rim high-frequency shadowing (`fc = 3200 Hz`)
        let rear_fc = 3_200.0_f32.min(0.42 * fs);
        self.rear_g = (std::f32::consts::PI * rear_fc / fs).tan();
        if angles.front_cos < 0.0 {
            let behind = -angles.front_cos; // 0..=1
            self.target_rear_high_scale = 1.0 - 0.45 * behind; // up to -5.2 dB rear pinna shadow above 3.2 kHz
        } else {
            self.target_rear_high_scale = 1.0 + 0.16 * angles.front_cos; // subtle frontal presence
        }

        if snap_immediately {
            self.cur_delay_samples_l = self.target_delay_samples_l;
            self.cur_delay_samples_r = self.target_delay_samples_r;
            self.cur_low_gain_l = self.target_low_gain_l;
            self.cur_high_gain_l = self.target_high_gain_l;
            self.cur_low_gain_r = self.target_low_gain_r;
            self.cur_high_gain_r = self.target_high_gain_r;
            self.cur_elev_high_scale = self.target_elev_high_scale;
            self.cur_elev_low_scale = self.target_elev_low_scale;
            self.cur_rear_high_scale = self.target_rear_high_scale;
        }
    }

    #[inline]
    fn read_fractional(
        buf: &[f32; HRTF_DELAY_BUFFER_LEN],
        write_idx: usize,
        delay_samples: f32,
    ) -> f32 {
        let d = delay_samples.clamp(0.0, (HRTF_DELAY_BUFFER_LEN - 2) as f32);
        let d_int = d as usize;
        let frac = d - (d_int as f32);
        let idx0 = write_idx.wrapping_sub(d_int) & HRTF_DELAY_BUFFER_MASK;
        let idx1 = write_idx.wrapping_sub(d_int + 1) & HRTF_DELAY_BUFFER_MASK;
        buf[idx0] * (1.0 - frac) + buf[idx1] * frac
    }

    /// Spatializes a single mono/stereo input [`Frame`] into a binaural stereo [`Frame`] with zero heap allocations.
    #[inline]
    pub fn process_frame(&mut self, input: Frame) -> Frame {
        let mono = if input.left.is_finite() && input.right.is_finite() {
            0.5 * (input.left + input.right)
        } else {
            0.0
        };

        // Sample-rate-invariant per-sample smoothing (`~8 ms` time constant across 8 kHz..192 kHz)
        let smooth = self.sample_smooth;
        self.cur_delay_samples_l +=
            (self.target_delay_samples_l - self.cur_delay_samples_l) * smooth;
        self.cur_delay_samples_r +=
            (self.target_delay_samples_r - self.cur_delay_samples_r) * smooth;
        self.cur_low_gain_l += (self.target_low_gain_l - self.cur_low_gain_l) * smooth;
        self.cur_high_gain_l += (self.target_high_gain_l - self.cur_high_gain_l) * smooth;
        self.cur_low_gain_r += (self.target_low_gain_r - self.cur_low_gain_r) * smooth;
        self.cur_high_gain_r += (self.target_high_gain_r - self.cur_high_gain_r) * smooth;
        self.cur_elev_high_scale +=
            (self.target_elev_high_scale - self.cur_elev_high_scale) * smooth;
        self.cur_elev_low_scale += (self.target_elev_low_scale - self.cur_elev_low_scale) * smooth;
        self.cur_rear_high_scale +=
            (self.target_rear_high_scale - self.cur_rear_high_scale) * smooth;

        // 1. Pinna concha notch (`mono - 0.55 * k * bp` produces a ~-7.5 dB narrow pinna spectral notch)
        let (_lp_n, bp_n, _hp_n) = self.pinna_notch_svf.step(mono, self.notch_g, self.notch_k);
        let after_notch = mono - 0.55 * self.notch_k * bp_n;

        // 2. Elevation spectral tilt (Above 5 kHz presence vs Below damping/warmth)
        let (lp_e, bp_e, hp_e) =
            self.pinna_elev_svf
                .step(after_notch, self.elev_g, std::f32::consts::SQRT_2);
        let after_elev = lp_e * self.cur_elev_low_scale
            + std::f32::consts::SQRT_2
                * bp_e
                * (0.5 * (self.cur_elev_low_scale + self.cur_elev_high_scale))
            + hp_e * self.cur_elev_high_scale;

        // 3. Front/Back pinna rim shadow above 3.2 kHz
        let (lp_fb, bp_fb, hp_fb) =
            self.pinna_rear_svf
                .step(after_elev, self.rear_g, std::f32::consts::SQRT_2);
        let pinna_out = lp_fb
            + std::f32::consts::SQRT_2 * bp_fb * (0.5 * (1.0 + self.cur_rear_high_scale))
            + hp_fb * self.cur_rear_high_scale;

        // 4. Write into circular ITD delay buffers and read fractional ear delays
        self.delay_buf_l[self.write_idx] = pinna_out;
        self.delay_buf_r[self.write_idx] = pinna_out;

        let delayed_l =
            Self::read_fractional(&self.delay_buf_l, self.write_idx, self.cur_delay_samples_l);
        let delayed_r =
            Self::read_fractional(&self.delay_buf_r, self.write_idx, self.cur_delay_samples_r);

        self.write_idx = (self.write_idx + 1) & HRTF_DELAY_BUFFER_MASK;

        // 5. Spherical head-shadow ILD (`1.4 kHz` low/high shelf per ear)
        self.head_shadow_l_z1 += self.head_lp_alpha * (delayed_l - self.head_shadow_l_z1);
        let low_l = self.head_shadow_l_z1;
        let high_l = delayed_l - low_l;
        let out_l = low_l * self.cur_low_gain_l + high_l * self.cur_high_gain_l;

        self.head_shadow_r_z1 += self.head_lp_alpha * (delayed_r - self.head_shadow_r_z1);
        let low_r = self.head_shadow_r_z1;
        let high_r = delayed_r - low_r;
        let out_r = low_r * self.cur_low_gain_r + high_r * self.cur_high_gain_r;

        Frame::new(out_l, out_r)
    }
}
