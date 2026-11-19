//! Dynamic Map Part Switches (`stageparts.c` — `EvDispControlModelEntry` / `EvDispControlModelExec`).
//!
//! Verified 1:1 against:
//! - PS2 Prototype `SLUS_202.28`: `EvDispControlModelEntry` (`0x001ad560`),
//!   `EvDispControlModelExec` (`0x001ad5e0`), `sh2gfw_Set_DispOnOffObj` (`0x00183ea0`),
//!   `sh2gfw_FastSet_DispOnOffObj` (`0x00183fb0`).
//! - Retail PC `sh2pc.exe`: `shDisplayControlEntry` (`0x004a2f10`),
//!   `shDisplayControlExec` (`0x004a2f60`), `SetDispOnOffMask` (`0x005b4760`).
//! - SH2:EE `Patches/MapMeshToggle.cpp` & `Patches/Langs.cpp` (`PatchTownGateEvents`).

pub mod switches;
pub mod traffic_lights;

use std::collections::BTreeMap;

use crate::events::{EventFlags, Sh2Rng};
pub use switches::{
    apply_stage_switches, FLAG_CANNED_JUICE_USED, FLAG_ENTERED_APARTMENT_GATE, FLAG_MET_EDDIE,
    FLAG_PLACED_COPPER_RING, FLAG_PLACED_LEAD_RING, FLAG_USED_BLUE_GEM_FIRST,
    FLAG_USED_BLUE_GEM_SECOND, FLAG_USED_VIDEO_TAPE, FLAG_WOODSIDE_GATE_UNLOCKED,
};
pub use traffic_lights::{
    TrafficLightPhase, TrafficLightState, TRAFFIC_AMBER_END_MS, TRAFFIC_CYCLE_MS,
    TRAFFIC_FLICKER_MAX_EXTRA_MS, TRAFFIC_FLICKER_MIN_MS, TRAFFIC_GREEN_END_MS,
    TRAFFIC_PED_WALK_END_MS, TRAFFIC_PED_WALK_START_MS,
};

/// Single `(stage_id, room_id) -> bitmask` record matching the 8-byte pair written by
/// `EvDispControlModelEntry` (`SLUS_202.28` `0x001ad560` / `sh2pc.exe` `0x004a2f10`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DisplayControlEntry {
    /// Active stage identifier (`stage_work->id` at offset `+0x24`, shifted left by 16 bits on PS2/PC).
    pub stage_id: u16,
    /// Target map block / room identifier (`0x0000..=0xffff`).
    pub room_id: u16,
    /// 16-bit variant visibility bitmask where bit `k` enables `.map` `variant_index == k + 1`.
    pub bitmask: u16,
}

impl DisplayControlEntry {
    /// Returns the packed 32-bit key `(stage_id << 16) | room_id` used by `EvDispControlModelEntry`.
    #[inline]
    #[must_use]
    pub const fn packed_key(self) -> u32 {
        ((self.stage_id as u32) << 16) | (self.room_id as u32)
    }
}

/// Accumulator list mirroring the stack-allocated `uint32_t display_list[]` passed to
/// `EvDispControlModelEntry` (`0x001ad560` / `0x004a2f10`) and committed via
/// `EvDispControlModelExec` (`0x001ad5e0` / `0x004a2f60`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DisplayControlList {
    entries: Vec<DisplayControlEntry>,
}

impl DisplayControlList {
    /// Creates an empty display control list.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Appends or OR-merges a variant bit into the entry for `(stage_id, room_id)`.
    ///
    /// Exact 1:1 port of `EvDispControlModelEntry` (`SLUS_202.28` `0x001ad560..0x001ad5d0` /
    /// `sh2pc.exe` `0x004a2f10..0x004a2f5d`):
    /// - If `(stage_id, room_id)` does not exist yet, inserts it with `bitmask = 0`.
    /// - If `variant_no >= 0` (and `< 16`), sets bit `1u16 << variant_no`.
    /// - If `variant_no < 0` (e.g. `-1`), leaves `bitmask` unchanged (hiding all switchable
    ///   variants when no other bits were set for this room).
    pub fn entry(&mut self, stage_id: u16, room_id: u16, variant_no: i32) {
        let bit = if (0..16).contains(&variant_no) {
            1u16 << (variant_no as u16)
        } else {
            0
        };

        if let Some(existing) = self
            .entries
            .iter_mut()
            .find(|e| e.stage_id == stage_id && e.room_id == room_id)
        {
            existing.bitmask |= bit;
        } else {
            self.entries.push(DisplayControlEntry {
                stage_id,
                room_id,
                bitmask: bit,
            });
        }
    }

    /// Returns all accumulated entries in insertion order.
    #[must_use]
    pub fn entries(&self) -> &[DisplayControlEntry] {
        &self.entries
    }

    /// Commits the list into a [`MapPartVisibilityFilter`] (`EvDispControlModelExec` `0x001ad5e0`).
    #[must_use]
    pub fn exec(&self) -> MapPartVisibilityFilter {
        let mut filter = MapPartVisibilityFilter::new();
        for entry in &self.entries {
            filter.set_room_mask(entry.room_id, entry.bitmask);
        }
        filter
    }
}

/// Per-frame `.map` sub-mesh visibility filter applied by `GpuMap` (`sh2-renderer`).
///
/// Exact 1:1 port of `sh2pc.exe` `0x0047d120` + `0x005b4760` (`SetDispOnOffMask`):
/// - For a loaded `.map` whose base room ID is `loaded_base_room_id`, a batch with
///   `part_group = (geo_id >> 8)` and `variant_index = (geo_id & 0xff)` targets
///   `effective_room_id = loaded_base_room_id + part_group`.
/// - `variant_index == 0` is ALWAYS visible.
/// - `variant_index in 1..=16` is visible iff `(mask_for(effective_room_id) & (1 << (variant_index - 1))) != 0`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MapPartVisibilityFilter {
    /// Active 16-bit display masks keyed by `room_id`.
    room_masks: BTreeMap<u16, u16>,
}

impl MapPartVisibilityFilter {
    /// Creates an empty visibility filter (all switchable variants default to hidden unless enabled).
    #[must_use]
    pub const fn new() -> Self {
        Self {
            room_masks: BTreeMap::new(),
        }
    }

    /// Sets the 16-bit visibility mask for `room_id`.
    pub fn set_room_mask(&mut self, room_id: u16, bitmask: u16) {
        self.room_masks.insert(room_id, bitmask);
    }

    /// Returns the active 16-bit mask for `room_id`, or `None` if untouched this frame.
    #[must_use]
    pub fn room_mask(&self, room_id: u16) -> Option<u16> {
        self.room_masks.get(&room_id).copied()
    }

    /// Returns all active `(room_id, bitmask)` pairs.
    #[must_use]
    pub fn masks(&self) -> &BTreeMap<u16, u16> {
        &self.room_masks
    }

    /// Evaluates whether a `.map` batch with `(part_group, variant_index)` inside a `.map` file
    /// rooted at `loaded_base_room_id` should be rendered.
    ///
    /// Matches `sh2pc.exe` `0x0047d16e..0x0047d186` and `0x005b47a0..0x005b47c8`.
    #[must_use]
    pub fn is_batch_visible(
        &self,
        loaded_base_room_id: u16,
        part_group: u8,
        variant_index: u8,
    ) -> bool {
        if variant_index == 0 {
            return true;
        }
        let bit = variant_index - 1;
        if bit >= 16 {
            return false;
        }
        let target_room = loaded_base_room_id.saturating_add(u16::from(part_group));
        let mask = self.room_masks.get(&target_room).copied().unwrap_or(0);
        (mask & (1u16 << bit)) != 0
    }
}

/// Inputs required by [`stageparts_tick`] for a single deterministic simulation frame.
#[derive(Debug, Clone, Copy)]
pub struct StagePartsContext<'a> {
    /// Retail stage ID (`1..=58`).
    pub stage_id: u16,
    /// Current room or outdoor tile numeric ID (e.g. `0x1d` for `cb29`, `0x5c` for Blue Creek 1F).
    pub room_id: u16,
    /// Active cutscene ID (`0` when no cutscene is playing; matches `demo_number()`).
    pub cutscene_id: u16,
    /// Delta time in seconds for this tick (e.g. `1.0 / 60.0`).
    pub dt_secs: f32,
    /// Global `16384`-bit story progression flag bank.
    pub flags: &'a EventFlags,
    /// Whether James currently carries the Blue Gem in inventory (`InventoryItemPointer()[0x07] & 0x40`).
    pub has_blue_gem: bool,
    /// Whether SH2:EE `MapMeshToggle` & `FixTownGateEvents` fixes are enabled (false for strict PS2/PC bug parity).
    pub enable_sh2ee_fixes: bool,
}

/// Output produced by [`stageparts_tick`] each frame.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StagePartsTickOutput {
    /// Compiled display-control filter ready for `GpuMap` batch visibility filtering.
    pub visibility: MapPartVisibilityFilter,
    /// Raw `EvDispControlModelEntry` list emitted this frame (useful for diagnostics/tests).
    pub display_list: DisplayControlList,
    /// SH2:EE `HotelRoom312DisplayControl` (`Patches/MapMeshToggle.cpp:125`):
    /// Event 19 low flag word (`0x02d6` while the Blue Gem hint is visible, `0` otherwise).
    pub hotel_room_312_memo_flag: u16,
}

/// Persistent state across frames for dynamic map part switches (traffic light timers & flicker RNG).
#[derive(Debug, Clone)]
pub struct StagePartsState {
    /// Town East (`Stage 5`) & Town West (`Stage 14`) traffic light timer & flicker state.
    pub traffic_lights: TrafficLightState,
}

impl Default for StagePartsState {
    fn default() -> Self {
        Self::new(0x51_48_32_78)
    }
}

impl StagePartsState {
    /// Creates a new `StagePartsState` seeded deterministically for traffic-light flicker RNG.
    #[must_use]
    pub fn new(rng_seed: u32) -> Self {
        Self {
            traffic_lights: TrafficLightState::new(Sh2Rng::new(rng_seed)),
        }
    }
}

/// Evaluates all dynamic map part switches (`stageparts.c`), Town traffic lights, story/puzzle
/// toggles, and SH2:EE `MapMeshToggle` / `FixTownGateEvents` for the current frame.
#[must_use]
pub fn stageparts_tick(
    state: &mut StagePartsState,
    ctx: &StagePartsContext<'_>,
) -> StagePartsTickOutput {
    let mut list = DisplayControlList::new();

    // 1. Town East (Stage 5) & Town West (Stage 14) traffic light timers & flicker.
    state
        .traffic_lights
        .tick(ctx.stage_id, ctx.dt_secs, ctx.flags, &mut list);

    // 2. Story/puzzle switches & SH2:EE MapMeshToggle / FixTownGateEvents.
    let hotel_room_312_memo_flag = apply_stage_switches(ctx, &mut list);

    let visibility = list.exec();
    StagePartsTickOutput {
        visibility,
        display_list: list,
        hotel_room_312_memo_flag,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_control_entry_or_merges_and_negative_hides() {
        let mut list = DisplayControlList::new();
        // Negative variant_no creates entry with bitmask 0 (all variants hidden).
        list.entry(25, 0xd6, -1);
        assert_eq!(list.entries().len(), 1);
        assert_eq!(list.entries()[0].bitmask, 0);

        // Positive variants OR-merge bits into the same (stage_id, room_id) entry.
        list.entry(25, 0xd6, 0);
        list.entry(25, 0xd6, 1);
        assert_eq!(list.entries().len(), 1);
        assert_eq!(list.entries()[0].bitmask, 0b11);
        assert_eq!(list.entries()[0].packed_key(), (25u32 << 16) | 0xd6);

        let filter = list.exec();
        // Base geometry (variant 0) is always visible.
        assert!(filter.is_batch_visible(0xd6, 0, 0));
        // Variant 1 -> bit 0 set; Variant 2 -> bit 1 set; Variant 3 -> bit 2 unset.
        assert!(filter.is_batch_visible(0xd6, 0, 1));
        assert!(filter.is_batch_visible(0xd6, 0, 2));
        assert!(!filter.is_batch_visible(0xd6, 0, 3));
    }

    #[test]
    fn test_multi_room_part_group_offset_matches_pc_0x5b47a0() {
        let mut filter = MapPartVisibilityFilter::new();
        // Suppose loaded .map base room is 0x50, and display control set mask on room 0x52 (offset +2).
        filter.set_room_mask(0x52, 1 << 0);
        assert!(filter.is_batch_visible(0x50, 2, 1));
        assert!(!filter.is_batch_visible(0x50, 2, 2));
        assert!(!filter.is_batch_visible(0x50, 1, 1));
        // Base geometry in any sub-room offset is always visible.
        assert!(filter.is_batch_visible(0x50, 1, 0));
    }
}
