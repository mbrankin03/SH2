//! SH2:EE (Enhanced Edition) and PS2 timing constants for decoupled 60 FPS gameplay.
//!
//! Ground truth references:
//! - `Silent-Hill-2-Enhancements/Patches/SixtyFPSPatch.cpp`
//! - `Silent-Hill-2-Enhancements/Patches/WaterMoveSpeed.cpp`
//! - Original PS2 frame rate base: 30 FPS / 60 Hz field timing.

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
        }
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
}
