//! Town East (`Stage 5`) & Town West (`Stage 14`) traffic light cycle and bulb flicker.
//!
//! Verified 1:1 against `sh2pc.exe`:
//! - `0x0059e4a0` (`cb29`, room `0x1d`): standard intersection cycle (`3100 ms` = `0xc1c`).
//! - `0x0059e540` (`cb35`, room `0x23`): `light_a` flickering intersection (`rand() % 1000 + 400`).
//! - `0x0059e630` (`cb36`, room `0x24`): standard intersection cycle.
//! - `0x0059e6d0` (`cb37`, room `0x25`): phase-shifted (`1800..2750..3050 ms`) flickering intersection.
//! - `cb27` (`0x1b`), `cb28` (`0x1c`), and `Stage 14` `cc10` (`0x0a`), `cc20` (`0x14`).

use crate::events::stageparts::DisplayControlList;
use crate::events::{EventFlags, Sh2Rng};

/// Total traffic light cycle period in milliseconds (`0x0c1c` in `sh2pc.exe` `0x0059e4a6`).
pub const TRAFFIC_CYCLE_MS: u32 = 3100;

/// End of Green phase in milliseconds (`0x04b0` = `1200 ms`, `sh2pc.exe` `0x0059e4ad`).
pub const TRAFFIC_GREEN_END_MS: u32 = 1200;

/// End of Amber phase in milliseconds (`0x05dc` = `1500 ms` -> `300 ms` amber, `sh2pc.exe` `0x0059e501`).
pub const TRAFFIC_AMBER_END_MS: u32 = 1500;

/// Cross-street pedestrian "Walk" signal start in milliseconds (`0x0708` = `1800 ms`, `sh2pc.exe` `0x0059e4d9`).
pub const TRAFFIC_PED_WALK_START_MS: u32 = 1800;

/// Cross-street pedestrian "Walk" signal end in milliseconds (`0x0a8c` = `2700 ms`, `sh2pc.exe` `0x0059e4e1`).
pub const TRAFFIC_PED_WALK_END_MS: u32 = 2700;

/// Minimum `light_a` bulb flicker interval in milliseconds (`0x0190` = `400 ms`, `sh2pc.exe` `0x0059e597`).
pub const TRAFFIC_FLICKER_MIN_MS: u32 = 400;

/// Random extra range added to `TRAFFIC_FLICKER_MIN_MS` (`0x03e8` = `1000` -> `400..=1399 ms`, `sh2pc.exe` `0x0059e58b`).
pub const TRAFFIC_FLICKER_MAX_EXTRA_MS: u32 = 1000;

/// Phase-shifted intersection (`cb37` = `0x25`) Green start (`0x0708` = `1800 ms`, `sh2pc.exe` `0x0059e6fd`).
const CB37_GREEN_START_MS: u32 = 1800;
/// Phase-shifted intersection (`cb37` = `0x25`) Green end (`0x0abe` = `2750 ms`, `sh2pc.exe` `0x0059e709`).
const CB37_GREEN_END_MS: u32 = 2750;
/// Phase-shifted intersection (`cb37` = `0x25`) Amber end (`0x0bea` = `3050 ms`, `sh2pc.exe` `0x0059e71a`).
const CB37_AMBER_END_MS: u32 = 3050;

/// Stage 5 (Town East) traffic light block IDs: `cb27..cb29` and `cb35..cb37`.
const STAGE5_CB27: u16 = 0x1b;
const STAGE5_CB28: u16 = 0x1c;
const STAGE5_CB29: u16 = 0x1d;
const STAGE5_CB35: u16 = 0x23;
const STAGE5_CB36: u16 = 0x24;
const STAGE5_CB37: u16 = 0x25;

/// Stage 14 (Town West) traffic light block IDs: `cc10`, `cc20`.
const STAGE14_CC10: u16 = 0x0a;
const STAGE14_CC20: u16 = 0x14;

/// Active signal phase of a traffic light intersection (`sh2pc.exe` `0x01d83b1c`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrafficLightPhase {
    /// Flickered bulb-off state (`0`, pushes variant `5` on `light_a` intersections).
    BulbOff,
    /// Red signal (`1`, pushes variant `2` on standard intersections).
    Red,
    /// Amber signal (`2`, pushes variant `1` on standard intersections).
    Amber,
    /// Green signal (`3`, pushes variant `0` on standard intersections).
    Green,
}

impl TrafficLightPhase {
    /// Returns the `.map` variant bit index (`0..=5`) for standard intersections (`sh2pc.exe` `0x0059e5a9..0x0059e5c6`).
    #[inline]
    #[must_use]
    pub const fn variant_no(self) -> i32 {
        match self {
            Self::Green => 0,
            Self::Amber => 1,
            Self::Red => 2,
            Self::BulbOff => 5,
        }
    }
}

/// Persistent timer & RNG state for Silent Hill Town East/West traffic lights (`sh2pc.exe` `0x01d83b10..0x01d83b1c`).
#[derive(Debug, Clone)]
pub struct TrafficLightState {
    /// Accumulated elapsed time in milliseconds (`0x01d83b10`).
    pub elapsed_ms: u32,
    /// Fractional millisecond accumulator so 60 Hz (`16.6667 ms`) ticks do not lose precision.
    frac_ms: f32,
    /// Next timestamp in milliseconds at which a `light_a` bulb flicker triggers (`0x01d83b14`).
    pub next_flicker_ms: u32,
    /// Deterministic PRNG for `rand() % 1000 + 400` flicker scheduling (`0x00448670`).
    rng: Sh2Rng,
    /// Last evaluated primary light phase.
    pub last_phase: TrafficLightPhase,
    /// Whether the cross-street pedestrian "Walk" signal is currently active.
    pub last_ped_walk: bool,
}

impl TrafficLightState {
    /// Creates a new `TrafficLightState` starting at `t = 0 ms`.
    #[must_use]
    pub fn new(rng: Sh2Rng) -> Self {
        Self {
            elapsed_ms: 0,
            frac_ms: 0.0,
            next_flicker_ms: TRAFFIC_FLICKER_MIN_MS,
            rng,
            last_phase: TrafficLightPhase::Green,
            last_ped_walk: false,
        }
    }

    /// Evaluates the standard `3100 ms` primary signal phase (`sh2pc.exe` `0x0059e4a0..0x0059e523`).
    #[inline]
    #[must_use]
    pub const fn standard_phase(cycle_ms: u32) -> TrafficLightPhase {
        if cycle_ms < TRAFFIC_GREEN_END_MS {
            TrafficLightPhase::Green
        } else if cycle_ms < TRAFFIC_AMBER_END_MS {
            TrafficLightPhase::Amber
        } else {
            TrafficLightPhase::Red
        }
    }

    /// Evaluates the phase-shifted `cb37` (`0x25`) primary signal phase (`sh2pc.exe` `0x0059e6fd..0x0059e723`).
    #[inline]
    #[must_use]
    pub const fn cb37_phase(cycle_ms: u32) -> TrafficLightPhase {
        if cycle_ms < CB37_GREEN_START_MS {
            TrafficLightPhase::Red
        } else if cycle_ms < CB37_GREEN_END_MS {
            TrafficLightPhase::Green
        } else if cycle_ms < CB37_AMBER_END_MS {
            TrafficLightPhase::Amber
        } else {
            TrafficLightPhase::Red
        }
    }

    /// Returns `true` when the cross-street pedestrian signal is in the `1800..2700 ms` Walk window
    /// (`sh2pc.exe` `0x0059e4d9..0x0059e4e7`).
    #[inline]
    #[must_use]
    pub const fn is_pedestrian_walk(cycle_ms: u32) -> bool {
        cycle_ms >= TRAFFIC_PED_WALK_START_MS && cycle_ms < TRAFFIC_PED_WALK_END_MS
    }

    /// Advances the traffic light timer and emits `EvDispControlModelEntry` commands for `Stage 5`
    /// (`cb27`, `cb28`, `cb29`, `cb35`, `cb36`, `cb37`) and `Stage 14` (`cc10`, `cc20`).
    pub fn tick(
        &mut self,
        stage_id: u16,
        dt_secs: f32,
        flags: &EventFlags,
        list: &mut DisplayControlList,
    ) {
        if stage_id != 5 && stage_id != 14 {
            return;
        }

        let dt_ms = dt_secs.max(0.0) * 1000.0 + self.frac_ms;
        let whole_ms = dt_ms.floor() as u32;
        self.frac_ms = dt_ms - (whole_ms as f32);
        self.elapsed_ms = self.elapsed_ms.wrapping_add(whole_ms);

        let cycle_ms = self.elapsed_ms % TRAFFIC_CYCLE_MS;
        let std_phase = Self::standard_phase(cycle_ms);
        let ped_walk = Self::is_pedestrian_walk(cycle_ms);
        let ped_variant = if ped_walk { 4 } else { 3 };

        // Check `light_a` flicker trigger (`sh2pc.exe` `0x0059e56c..0x0059e5a3`).
        let flicker_off = if self.elapsed_ms > self.next_flicker_ms {
            let extra = (self.rng.next_i32() as u32) % TRAFFIC_FLICKER_MAX_EXTRA_MS;
            self.next_flicker_ms = self
                .next_flicker_ms
                .wrapping_add(extra)
                .wrapping_add(TRAFFIC_FLICKER_MIN_MS);
            true
        } else {
            false
        };

        self.last_phase = if flicker_off {
            TrafficLightPhase::BulbOff
        } else {
            std_phase
        };
        self.last_ped_walk = ped_walk;

        if stage_id == 5 {
            // Standard intersections: cb27 (0x1b), cb28 (0x1c), cb29 (0x1d), cb36 (0x24)
            for room in [STAGE5_CB27, STAGE5_CB28, STAGE5_CB29, STAGE5_CB36] {
                list.entry(stage_id, room, std_phase.variant_no());
                list.entry(stage_id, room, ped_variant);
            }

            // Flickering `light_a` intersection: cb35 (0x23, `sh2pc.exe` `0x0059e540`)
            let cb35_phase = if flicker_off {
                TrafficLightPhase::BulbOff
            } else {
                std_phase
            };
            list.entry(stage_id, STAGE5_CB35, cb35_phase.variant_no());
            list.entry(stage_id, STAGE5_CB35, ped_variant);

            // Phase-shifted flickering intersection: cb37 (0x25, `sh2pc.exe` `0x0059e6d0`)
            // Flag bit `0x0800` at `0x01f7a9e4` (word 1 bit 11 relative to `flag_work` `0x01f7a9e0`
            // -> flag `43` / `0x2b`) enables static variant 0 on cb37.
            if flags.get(43) {
                list.entry(stage_id, STAGE5_CB37, 0);
            }
            let cb37_phase = if flicker_off {
                TrafficLightPhase::BulbOff
            } else {
                Self::cb37_phase(cycle_ms)
            };
            // On cb37, variants are shifted by +1 (1..=6) because variant 0 is the story prop.
            let cb37_var = match cb37_phase {
                TrafficLightPhase::Green => 1,
                TrafficLightPhase::Amber => 2,
                TrafficLightPhase::Red => 3,
                TrafficLightPhase::BulbOff => 6,
            };
            list.entry(stage_id, STAGE5_CB37, cb37_var);
            list.entry(stage_id, STAGE5_CB37, if ped_walk { 5 } else { 4 });
        } else if stage_id == 14 {
            // Town West traffic lights: cc10 (0x0a) & cc20 (0x14)
            for room in [STAGE14_CC10, STAGE14_CC20] {
                list.entry(stage_id, room, std_phase.variant_no());
                list.entry(stage_id, room, ped_variant);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_traffic_light_3100ms_cycle_matches_pc_0x59e4a0() {
        assert_eq!(
            TrafficLightState::standard_phase(0),
            TrafficLightPhase::Green
        );
        assert_eq!(
            TrafficLightState::standard_phase(1199),
            TrafficLightPhase::Green
        );
        assert_eq!(
            TrafficLightState::standard_phase(1200),
            TrafficLightPhase::Amber
        );
        assert_eq!(
            TrafficLightState::standard_phase(1499),
            TrafficLightPhase::Amber
        );
        assert_eq!(
            TrafficLightState::standard_phase(1500),
            TrafficLightPhase::Red
        );
        assert_eq!(
            TrafficLightState::standard_phase(3099),
            TrafficLightPhase::Red
        );

        assert!(!TrafficLightState::is_pedestrian_walk(1799));
        assert!(TrafficLightState::is_pedestrian_walk(1800));
        assert!(TrafficLightState::is_pedestrian_walk(2699));
        assert!(!TrafficLightState::is_pedestrian_walk(2700));
    }

    #[test]
    fn test_traffic_light_flicker_schedules_within_400_to_1399ms() {
        let flags = EventFlags::new();
        let mut state = TrafficLightState::new(Sh2Rng::new(42));
        let mut list = DisplayControlList::new();

        // Advance past initial 400 ms threshold -> triggers flicker off (variant 5 on cb35).
        state.tick(5, 0.45, &flags, &mut list);
        assert_eq!(state.last_phase, TrafficLightPhase::BulbOff);
        assert!(
            state.next_flicker_ms >= 800 && state.next_flicker_ms < 400 + 1400,
            "next_flicker_ms={} must advance by 400..=1399 ms",
            state.next_flicker_ms
        );

        let filter = list.exec();
        // cb35 (0x23): variant 6 (bit 5 = bulb off) & variant 4 (bit 3 = ped don't walk)
        assert!(filter.is_batch_visible(0x23, 0, 6));
        assert!(filter.is_batch_visible(0x23, 0, 4));
    }
}
