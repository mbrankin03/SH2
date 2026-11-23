//! Sleep culling (`enCheckSleepIn` `0x00142950`, `enCheckSleepOut` `0x001429f0`)
//! and 8-slot tribe alert broadcast pool (`enSetCommunication` `0x00146d80`, `enCommunicateTribe` `0x00146e40`).

use glam::Vec3;

/// Outdoor sleep-in distance threshold (`enCheckSleepIn` `SLUS 0x00142950` / `sh2pc 0x004a63f0`).
pub const ENEMY_SLEEP_IN_DIST_MM: f32 = 15_000.0;

/// Outdoor wake-up distance threshold (`enCheckSleepOut` `SLUS 0x001429f0` / `sh2pc 0x004a6400`).
pub const ENEMY_SLEEP_OUT_DIST_MM: f32 = 7_500.0;

/// Evaluate whether an active enemy should enter sleep culling (`enCheckSleepIn` `SLUS 0x00142950` / `sh2pc 0x004a63f0`).
///
/// Returns `true` when the enemy should sleep (outdoor distance `> 15000.0` mm or not in active room and not forced-awake).
pub fn en_check_sleep_in(
    in_active_room: bool,
    force_awake: bool,
    is_outdoor: bool,
    player_dist_mm: f32,
) -> bool {
    !((in_active_room && (!is_outdoor || player_dist_mm <= ENEMY_SLEEP_IN_DIST_MM)) || force_awake)
}

/// Evaluate whether a sleeping enemy should wake up (`enCheckSleepOut` `SLUS 0x001429f0` / `sh2pc 0x004a6400`).
///
/// Returns `true` when the enemy should wake (in active room and, if outdoor, within `< 7500.0` mm of player).
pub fn en_check_sleep_out(in_active_room: bool, is_outdoor: bool, player_dist_mm: f32) -> bool {
    in_active_room && (!is_outdoor || player_dist_mm < ENEMY_SLEEP_OUT_DIST_MM)
}

/// Single alert slot in the 8-slot tribe communication pool (`0x005522a0`, `enSetCommunication` `0x00146d80`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TribeCommSlot {
    pub radius_mm: f32,
    pub tribe_id: u8,
    pub msg_type: u8,
    pub pos: Vec3,
    pub timer_ticks: i16,
}

/// 8-slot tribe alert broadcast pool (`enSetCommunication` `0x00146d80` / `enCommunicateTribe` `0x00146e40`).
///
/// - SCU (`tribe_id = 1`): `radius = 4000.0` mm, `duration = 120` ticks (`enSCUCtrlChase` `0x00161190`).
/// - MKN (`tribe_id = 2`): `radius = 1000.0` mm, `duration = 2` ticks (`enMKNCtrlMannequin` `0x001539e0`).
#[derive(Debug, Clone, Default)]
pub struct TribeCommPool {
    slots: [Option<TribeCommSlot>; 8],
}

impl TribeCommPool {
    /// Post a tribe alert into the first free slot (`enSetCommunication` `SLUS 0x00146d80`).
    pub fn post(&mut self, radius_mm: f32, tribe_id: u8, msg_type: u8, pos: Vec3, duration: i16) {
        for slot in &mut self.slots {
            if slot.is_none() {
                *slot = Some(TribeCommSlot {
                    radius_mm,
                    tribe_id,
                    msg_type,
                    pos,
                    timer_ticks: duration,
                });
                return;
            }
        }
    }

    /// Decrement all active alert timers by `ticks` (`enExecTask` `SLUS 0x001413f0`).
    pub fn tick(&mut self, ticks: i16) {
        for slot in &mut self.slots {
            if let Some(s) = slot {
                s.timer_ticks -= ticks;
                if s.timer_ticks <= 0 {
                    *slot = None;
                }
            }
        }
    }

    /// Query the closest active alert for `tribe_id` within its broadcast radius (`enCommunicateTribe` `SLUS 0x00146e40`).
    pub fn query_nearest(&self, tribe_id: u8, pos: Vec3) -> Option<TribeCommSlot> {
        let mut best: Option<(f32, TribeCommSlot)> = None;
        for s in self.slots.iter().flatten() {
            if s.tribe_id != tribe_id {
                continue;
            }
            let d = pos.distance(s.pos);
            if d < s.radius_mm && best.is_none_or(|(bd, _)| d < bd) {
                best = Some((d, *s));
            }
        }
        best.map(|(_, s)| s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sleep_culling_thresholds_and_tribe_comm() {
        assert!(!en_check_sleep_in(true, false, true, 15_000.0));
        assert!(en_check_sleep_in(true, false, true, 15_001.0));
        assert!(!en_check_sleep_out(true, true, 7_500.0));
        assert!(en_check_sleep_out(true, true, 7_499.0));

        let mut pool = TribeCommPool::default();
        pool.post(4000.0, 1, 0, Vec3::new(1000.0, 0.0, 0.0), 2);
        assert!(pool.query_nearest(1, Vec3::ZERO).is_some());
        assert!(pool.query_nearest(2, Vec3::ZERO).is_none());
        pool.tick(2);
        assert!(pool.query_nearest(1, Vec3::ZERO).is_none());
    }
}
