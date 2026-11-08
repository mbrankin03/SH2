//! SH2:EE (Enhanced Edition) and PS2 timing constants for decoupled 60 FPS gameplay
//! and high-refresh rate (120–240 Hz+) sub-frame interpolation.
//!
//! Ground truth references:
//! - `Silent-Hill-2-Enhancements/Patches/SixtyFPSPatch.cpp`
//! - `Silent-Hill-2-Enhancements/Patches/RowboatAnimation.cpp`
//! - `Silent-Hill-2-Enhancements/Patches/RowboatFix.cpp`
//! - `Silent-Hill-2-Enhancements/Patches/WaterMoveSpeed.cpp`
//! - Original PS2 frame rate base: 30 FPS / 60 Hz field timing.

use glam::{Quat, Vec3};

/// Timing constants and scaling coefficients applied during 60 Hz simulation updates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimingConstants {
    /// Water surface animation step per tick (original SH2:EE WaterOvrd = 0.0166665f).
    pub water_anim_step: f32,
    /// Low health red aura pulsation rate per tick (SH2:EE LowHealthOvrd = 0.0166665f).
    pub low_health_flash_rate: f32,
    /// Staircase flames lighting variation rate (SH2:EE StaircaseLightingOvrd = 0.00027777222565f).
    pub staircase_flames_rate: f32,
    /// Water draining step lowering rate (SH2:EE WaterLowOvrd = 20.0f).
    pub water_drain_rate: f32,
    /// Water filling step rising rate (SH2:EE WaterRisOvrd = 5.333333492f).
    pub water_rise_rate: f32,
    /// Bug room flashlight oscillation frequency modifier (SH2:EE BugRoomFlashlightOvrd = 0.093333332985f).
    pub bug_room_flashlight_rate: f32,
    /// Flashlight click/toggle cycle delay in ticks (SH2:EE FlashlightOnOvrd = 0x78 = 120 ticks at 60 Hz).
    pub flashlight_toggle_ticks: u16,
    /// Motion blur accumulation weight (SH2:EE MotionBlurOvrd = 0.125f for 60 FPS).
    pub motion_blur_factor: f32,
    /// Fog horizontal/vertical particle drift speed multiplier (halved for 60 FPS tick decoupled rate).
    pub fog_animation_rate: f32,
    /// Ejected bullet shell physics falling rate multiplier (doubled for 60 FPS).
    pub bullet_shell_rate: f32,
    /// Toluca lake rowboat physics propulsion factor per stroke at 60 Hz.
    pub boat_rowing_impulse: f32,
    /// Flashlight pocket-mount angular sweep smoothing factor per 60 Hz tick.
    pub flashlight_sweep_rate: f32,
}

impl Default for TimingConstants {
    fn default() -> Self {
        Self {
            water_anim_step: 0.0166665,
            low_health_flash_rate: 0.0166665,
            staircase_flames_rate: 0.00027777223,
            water_drain_rate: 20.0,
            water_rise_rate: 5.3333335,
            bug_room_flashlight_rate: 0.09333333,
            flashlight_toggle_ticks: 120,
            motion_blur_factor: 0.125,
            fog_animation_rate: 0.5,
            bullet_shell_rate: 2.0,
            boat_rowing_impulse: 1.0,
            flashlight_sweep_rate: 0.28,
        }
    }
}

impl TimingConstants {
    /// Computes deterministic fog flow time in seconds from elapsed 60 Hz `tick_counter`
    /// and sub-frame remainder `alpha` (`0.0..1.0`), invariant of render frame rate (30–240 Hz+).
    #[inline]
    pub fn deterministic_fog_time(&self, tick_counter: u64, alpha: f32) -> f32 {
        let sim_seconds = (tick_counter as f64 + alpha.clamp(0.0, 1.0) as f64) / 60.0;
        (sim_seconds * self.fog_animation_rate as f64 * 2.0) as f32
    }
}

/// Shortest-arc angle interpolation in radians on $[-\pi, \pi]$.
#[inline]
pub fn lerp_angle_shortest(a: f32, b: f32, alpha: f32) -> f32 {
    let two_pi = std::f32::consts::TAU;
    let mut diff = (b - a) % two_pi;
    if diff > std::f32::consts::PI {
        diff -= two_pi;
    } else if diff < -std::f32::consts::PI {
        diff += two_pi;
    }
    a + diff * alpha.clamp(0.0, 1.0)
}

/// Deterministic 60 Hz pocket flashlight sweep and toggle controller (`sh2_JmsSpot_Man.c`).
///
/// Decouples flashlight beam sweep inertia, chest pocket offset, and Bug Room battery flicker
/// oscillation from render framerate, providing sub-frame Slerp interpolation at 120–240 Hz+.
#[derive(Debug, Clone, Copy)]
pub struct FlashlightSweepState {
    /// Previous 60 Hz tick flashlight origin in world space.
    pub prev_origin: Vec3,
    /// Current 60 Hz tick flashlight origin in world space.
    pub curr_origin: Vec3,
    /// Previous 60 Hz tick beam orientation quaternion.
    pub prev_orientation: Quat,
    /// Current 60 Hz tick beam orientation quaternion.
    pub curr_orientation: Quat,
    /// Smoothed flashlight yaw angle in radians.
    pub beam_yaw: f32,
    /// Smoothed flashlight pitch angle in radians.
    pub beam_pitch: f32,
    /// Bug room oscillation phase accumulator (advanced at 60 Hz by `bug_room_flashlight_rate`).
    pub bug_room_phase: f32,
    /// Remaining toggle cooldown ticks (`FlashlightOnOvrd = 120` ticks at 60 Hz during battery replacement).
    pub toggle_cooldown_ticks: u16,
}

impl Default for FlashlightSweepState {
    fn default() -> Self {
        Self::new(Vec3::ZERO, 0.0)
    }
}

impl FlashlightSweepState {
    /// Chest pocket vertical offset in SH2 world units (`-Y` is up; `-1250.0` is upper chest).
    pub const CHEST_HEIGHT_OFFSET: f32 = -1250.0;
    /// Forward offset from torso centerline in SH2 world units.
    pub const CHEST_FORWARD_OFFSET: f32 = 45.0;
    /// Default downward beam pitch (`+0.05` radians along `+Y` down).
    pub const BASE_BEAM_PITCH: f32 = 0.05;

    /// Creates a new flashlight sweep state aligned to the character's initial pose.
    pub fn new(player_position: Vec3, player_yaw: f32) -> Self {
        let origin = Self::compute_pocket_origin(player_position, player_yaw);
        let orient = Self::orientation_from_yaw_pitch(player_yaw, Self::BASE_BEAM_PITCH);
        Self {
            prev_origin: origin,
            curr_origin: origin,
            prev_orientation: orient,
            curr_orientation: orient,
            beam_yaw: player_yaw,
            beam_pitch: Self::BASE_BEAM_PITCH,
            bug_room_phase: 0.0,
            toggle_cooldown_ticks: 0,
        }
    }

    #[inline]
    fn compute_pocket_origin(player_position: Vec3, yaw: f32) -> Vec3 {
        let (sy, cy) = yaw.sin_cos();
        player_position
            + Vec3::new(
                sy * Self::CHEST_FORWARD_OFFSET,
                Self::CHEST_HEIGHT_OFFSET,
                cy * Self::CHEST_FORWARD_OFFSET,
            )
    }

    #[inline]
    fn orientation_from_yaw_pitch(yaw: f32, pitch: f32) -> Quat {
        Quat::from_euler(glam::EulerRot::YXZ, yaw, pitch, 0.0)
    }

    /// Snaps the flashlight beam immediately to the given character pose (e.g. room transition).
    pub fn reset(&mut self, player_position: Vec3, player_yaw: f32) {
        *self = Self::new(player_position, player_yaw);
    }

    /// Steps the flashlight sweep inertia and oscillation deterministically for one 60 Hz tick.
    pub fn tick(
        &mut self,
        player_position: Vec3,
        player_yaw: f32,
        in_bug_room: bool,
        constants: &TimingConstants,
    ) {
        self.prev_origin = self.curr_origin;
        self.prev_orientation = self.curr_orientation;

        if self.toggle_cooldown_ticks > 0 {
            self.toggle_cooldown_ticks -= 1;
        }

        // Smoothly sweep beam yaw toward player torso yaw at 60 Hz tick rate
        let sweep = constants.flashlight_sweep_rate.clamp(0.01, 1.0);
        self.beam_yaw = lerp_angle_shortest(self.beam_yaw, player_yaw, sweep);

        let mut target_pitch = Self::BASE_BEAM_PITCH;
        if in_bug_room {
            self.bug_room_phase =
                (self.bug_room_phase + constants.bug_room_flashlight_rate) % std::f32::consts::TAU;
            target_pitch += self.bug_room_phase.sin() * 0.035;
        }
        self.beam_pitch += (target_pitch - self.beam_pitch) * sweep;

        self.curr_origin = Self::compute_pocket_origin(player_position, player_yaw);
        self.curr_orientation = Self::orientation_from_yaw_pitch(self.beam_yaw, self.beam_pitch);
    }

    /// Evaluates the sub-frame interpolated flashlight `(origin, unit_direction)` for `alpha` in `[0.0, 1.0]`.
    #[inline]
    pub fn interpolated_ray(&self, alpha: f32) -> (Vec3, Vec3) {
        let t = alpha.clamp(0.0, 1.0);
        let origin = self.prev_origin.lerp(self.curr_origin, t);
        let rot = self.prev_orientation.slerp(self.curr_orientation, t);
        let dir = (rot * Vec3::Z).normalize_or_zero();
        (origin, dir)
    }
}

/// Deterministic 60 Hz Toluca Lake rowboat controller (`m3_boat.c`).
///
/// Implements native fixes for two classic high-framerate / PC quirks:
/// 1. **`RowboatAnimation.cpp`**: Clamps and wraps the oar stroke animation frame at `0x4C` (`76`)
///    when rowing forward and turning simultaneously, preventing out-of-bounds animation glitching.
/// 2. **`RowboatFix.cpp`**: Normalizes oar stroke propulsion and turn rate strictly by 60 Hz
///    simulation ticks and ignores equipped weapon weight (fixing the bug where equipping the
///    Great Knife before boarding the boat permanently halved rowing speed).
#[derive(Debug, Clone, Copy)]
pub struct RowboatController {
    /// Previous 60 Hz tick world position on Toluca Lake.
    pub prev_position: Vec3,
    /// Current 60 Hz tick world position on Toluca Lake.
    pub position: Vec3,
    /// Previous 60 Hz tick boat heading yaw in radians.
    pub prev_yaw: f32,
    /// Current 60 Hz tick boat heading yaw in radians.
    pub yaw: f32,
    /// Linear forward velocity in SH2 units/sec.
    pub forward_speed: f32,
    /// Angular turning velocity in radians/sec.
    pub angular_velocity: f32,
    /// Continuous oar animation frame in `jms_boat.anm` (`0.0 ..= 76.0`, `0x4C` wrap boundary).
    pub oar_anim_frame: f32,
    /// Previous tick oar animation frame for sub-frame interpolation.
    pub prev_oar_anim_frame: f32,
}

impl Default for RowboatController {
    fn default() -> Self {
        Self::new(Vec3::ZERO, 0.0)
    }
}

impl RowboatController {
    /// Maximum valid oar stroke frame in `jms_boat.anm` (`0x4C` = 76, from `RowboatAnimation.cpp`).
    pub const MAX_OAR_STROKE_FRAME: f32 = 76.0;
    /// Oar animation playback rate in frames per 60 Hz tick (30 FPS base = 0.5 frames/tick).
    pub const OAR_FRAMES_PER_TICK: f32 = 0.5;
    /// Maximum rowboat forward speed in SH2 world units/sec.
    pub const MAX_FORWARD_SPEED: f32 = 1650.0;
    /// Maximum rowboat turn rate in radians/sec.
    pub const MAX_TURN_RATE: f32 = 1.15;

    /// Creates a new Toluca Lake rowboat controller at the given lake position and heading.
    pub fn new(position: Vec3, yaw: f32) -> Self {
        Self {
            prev_position: position,
            position,
            prev_yaw: yaw,
            yaw,
            forward_speed: 0.0,
            angular_velocity: 0.0,
            oar_anim_frame: 0.0,
            prev_oar_anim_frame: 0.0,
        }
    }

    /// Advances rowboat physics and oar animation deterministically for one 60 Hz simulation tick.
    ///
    /// Note: `great_knife_equipped` is accepted for diagnostic parity with `RowboatFix.cpp` and
    /// intentionally does **not** scale down propulsion or oar speed.
    pub fn tick(
        &mut self,
        left_oar_input: f32,
        right_oar_input: f32,
        _great_knife_equipped: bool,
        constants: &TimingConstants,
    ) {
        let dt = 1.0 / 60.0;
        self.prev_position = self.position;
        self.prev_yaw = self.yaw;
        self.prev_oar_anim_frame = self.oar_anim_frame;

        let left = left_oar_input.clamp(-1.0, 1.0);
        let right = right_oar_input.clamp(-1.0, 1.0);

        // Symmetric oar input drives forward propulsion; differential oar input drives turning
        let stroke_power = 0.5 * (left + right) * constants.boat_rowing_impulse;
        let turn_drive = (left - right).clamp(-1.0, 1.0) * constants.boat_rowing_impulse;

        let target_speed = stroke_power * Self::MAX_FORWARD_SPEED;
        let accel_rate = if stroke_power.abs() > 0.01 { 2.4 } else { 0.85 };
        self.forward_speed += (target_speed - self.forward_speed) * (accel_rate * dt);

        let target_turn = turn_drive * Self::MAX_TURN_RATE;
        self.angular_velocity += (target_turn - self.angular_velocity) * (3.2 * dt);

        self.yaw = (self.yaw + self.angular_velocity * dt).rem_euclid(std::f32::consts::TAU);
        let (sy, cy) = self.yaw.sin_cos();
        self.position.x += sy * self.forward_speed * dt;
        self.position.z += cy * self.forward_speed * dt;

        // Advance `jms_boat.anm` oar cycle and enforce the `0x4C` (76.0) wrap fix (`RowboatAnimation.cpp`)
        let active_oar_effort = left.abs().max(right.abs());
        if active_oar_effort > 0.05 {
            self.oar_anim_frame += Self::OAR_FRAMES_PER_TICK * active_oar_effort;
            if self.oar_anim_frame > Self::MAX_OAR_STROKE_FRAME {
                self.oar_anim_frame -= Self::MAX_OAR_STROKE_FRAME;
            }
        }
    }

    /// Evaluates sub-frame interpolated boat `(position, yaw, oar_frame)` for `alpha` in `[0.0, 1.0]`.
    #[inline]
    pub fn interpolated_pose(&self, alpha: f32) -> (Vec3, f32, f32) {
        let t = alpha.clamp(0.0, 1.0);
        let pos = self.prev_position.lerp(self.position, t);
        let yaw = lerp_angle_shortest(self.prev_yaw, self.yaw, t);
        let oar = if (self.oar_anim_frame - self.prev_oar_anim_frame).abs()
            > Self::MAX_OAR_STROKE_FRAME * 0.5
        {
            // Handle wrap-around across 0x4C (76.0)
            let unwrapped = self.oar_anim_frame + Self::MAX_OAR_STROKE_FRAME;
            (self.prev_oar_anim_frame + (unwrapped - self.prev_oar_anim_frame) * t)
                % Self::MAX_OAR_STROKE_FRAME
        } else {
            self.prev_oar_anim_frame + (self.oar_anim_frame - self.prev_oar_anim_frame) * t
        };
        (pos, yaw, oar)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_timing_constants() {
        let constants = TimingConstants::default();
        assert_eq!(constants.flashlight_toggle_ticks, 120);
        assert!((constants.motion_blur_factor - 0.125).abs() < f32::EPSILON);
        assert!((constants.fog_animation_rate - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn test_sh2_56_rowboat_flashlight_and_fog_tick_normalization() {
        let constants = TimingConstants::default();

        // 1. Fog time must depend purely on elapsed 60 Hz ticks + alpha, invariant of render FPS
        let fog_t_60 = constants.deterministic_fog_time(60, 0.0);
        let fog_t_half = constants.deterministic_fog_time(30, 0.5);
        assert!((fog_t_60 - 1.0).abs() < 1e-5);
        assert!((fog_t_half - (30.5 / 60.0)).abs() < 1e-5);

        // 2. Rowboat: Great Knife equipped must NOT slow rowing speed (`RowboatFix.cpp`),
        // and simultaneous forward+turn rowing must wrap cleanly at frame 0x4C = 76.0 (`RowboatAnimation.cpp`)
        let mut boat_normal = RowboatController::new(Vec3::ZERO, 0.0);
        let mut boat_great_knife = RowboatController::new(Vec3::ZERO, 0.0);
        for _ in 0..200 {
            boat_normal.tick(1.0, 0.5, false, &constants);
            boat_great_knife.tick(1.0, 0.5, true, &constants);
            assert!(
                boat_normal.oar_anim_frame <= RowboatController::MAX_OAR_STROKE_FRAME,
                "Oar frame {} exceeded 0x4C (76.0) cap!",
                boat_normal.oar_anim_frame
            );
        }
        assert_eq!(boat_normal.position, boat_great_knife.position);
        assert_eq!(boat_normal.yaw, boat_great_knife.yaw);
        assert!(boat_normal.position.length() > 100.0);

        // 3. Flashlight sweep: smoothly interpolates between 60 Hz ticks via Slerp
        let mut fl = FlashlightSweepState::new(Vec3::ZERO, 0.0);
        fl.tick(Vec3::new(0.0, 0.0, 50.0), 1.0, true, &constants);
        let (orig_0, dir_0) = fl.interpolated_ray(0.0);
        let (orig_half, dir_half) = fl.interpolated_ray(0.5);
        let (orig_1, dir_1) = fl.interpolated_ray(1.0);
        assert!((orig_half.z - 0.5 * (orig_0.z + orig_1.z)).abs() < 1e-3);
        assert!(dir_half.x > dir_0.x && dir_half.x < dir_1.x);
    }
}
