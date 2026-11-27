//! Multi-slot resident outdoor `.cam` road bank (`SH2-184`).
//!
//! Mirrors the authentic PS2/PC dual global + per-tile `.cam` slot architecture so `<area>GB.cam`
//! (`bg/ca/caGB.cam`, `bg/cb/cbGB.cam`, `bg/cc/ccGB.cam`, `bg/cd/cdGB.cam`) stays resident
//! alongside active outdoor tile `.cam` files (`ca11.cam..ca28.cam`, `cb*.cam`) across tile
//! boundaries without resetting camera smoothing or losing `vcSetCurNearRoadInVC_WORK` hysteresis.
//!
//! Ground truth:
//! - PS2 `SLUS_202.28`:
//!   - `LoadBg/loadbg_cam.c`: `lbCAM_Ctrl` (`0x11 = 17` slots: slot `0` global via
//!     `loadBgCAM_ReplaceG` `0x00271d20`, slots `1..=16` per-block via `loadBgCAM_Replace`
//!     `0x00271e10`), `loadBgCAM_GetLoadedDataAddrList` (`0x00271f10` -> `FUN_00271f60` `0x00271f60`),
//!     and `loadBgCAM_vcReset` (`0x00271f90`, updates `vcWork.road_ary_list` `_DAT_010e5ad4` without
//!     resetting camera position/velocity/smoothing).
//!   - `LoadBg/loadbg_camdata.c`: `loadBgCAM_LoadData` (`0x00251b10`, loads `<area>GB.cam` into
//!     slot `0`).
//!   - `LoadBg/loadbg_2x2.c`: `loadBg2x2_ActivateRequestOutdoor` (`0x00270590`, activates up to `4`
//!     simultaneous outdoor 2x2 tile `.cam` slots).
//!   - `view/vc_main.c`: `vcPreSetDataInVC_WORK` (`0x00196be0`, `10000.0` mm scan radius
//!     `0x461c4000`), `vcSetNearRoadAryByCharaPos` (`0x00197310`, `0x80 = 128` max near roads),
//!     `vcGetNearestNEAR_ROAD_DATA` (`0x001988b0`, `fVar4 <= fVar1` tie-break), and
//!     `vcSetCurNearRoadInVC_WORK` (`0x00197af0`, matches prior active road by identity for
//!     hysteresis preservation).
//! - Retail PC `sh2pc.exe` (prototype-vs-retail drift):
//!   - `sh2pc_00510000.c`: `FUN_005132c0` (`0x005132c0`) loads global `<area>GB.cam` into
//!     `DAT_00ac6f18`; `FUN_00513360` (`0x00513360`) frees it; `FUN_00513260` (`0x00513260`) writes
//!     `DAT_00ac6f18` at `_DAT_00ac70f0` and calls `FUN_00513110` (`0x00513110`) to append up to `8`
//!     active block `.cam` pointers (`0x00ac6e3c` stride `0x1c`), assigning `vcWork.road_ary_list`
//!     (`DAT_01f811e4`) without resetting camera smoothing.
//!   - `FUN_0051dc00` (`0x0051dc00`, `vcGetNearestNEAR_ROAD_DATA`) preserves the same `<=` distance
//!     tie-break (so per-tile `.cam` records win ties at equal `rd_type` priority & distance against
//!     `<area>GB.cam`) while adding a `±0.5` mm Y-bound tolerance.

use sh2_assets::formats::cam::{CamFile, CamSourceKind};

/// Total `.cam` slots in PS2 `SLUS_202.28` `lbCAM_Ctrl` (`0x11 = 17`: slot `0` global `<area>GB.cam`
/// + slots `1..=16` per-block `.cam`; `LoadBg/loadbg_cam.c` `FUN_00271f60` `0x00271f60`).
pub const PS2_LOADBG_CAM_MAX_SLOTS: usize = 17;

/// Maximum simultaneous outdoor 2x2 tile `.cam` slots activated by `loadBg2x2_ActivateRequestOutdoor`
/// (`SLUS_202.28` `0x00270590`, `LoadBg/loadbg_2x2.c`).
pub const PS2_LOADBG_2X2_OUTDOOR_CAM_SLOTS: usize = 4;

/// Maximum simultaneous block `.cam` slots in retail PC `sh2pc.exe` (`FUN_00513110` `0x00513110`,
/// 8 entries at `0x00ac6e3c..0x00ac6f00` stride `0x1c` alongside global `DAT_00ac6f18`).
pub const PC_LOADBG_BLOCK_CAM_SLOTS: usize = 8;

/// Near-road XZ scan radius in PS2 world mm passed to `vcSetNearRoadAryByCharaPos`
/// (`0x461c4000` = `10000.0`, `SLUS_202.28` `vcPreSetDataInVC_WORK` `0x00196be0`, `vc_main.c:769`).
pub const VC_NEAR_ROAD_SCAN_RADIUS_MM: f32 = 10_000.0;

/// Maximum number of near roads stored in `vcWork.near_road_ary` (`0x80 = 128`,
/// `SLUS_202.28` `vcSetNearRoadAryByCharaPos` `0x00197310`, `vc_main.c:1261` /
/// `sh2pc.exe` `sh2pc_00510000.c:9899`).
pub const VC_NEAR_ROAD_MAX_COUNT: usize = 128;

/// Stable identity key for a resident `.cam` slot in [`OutdoorCamBank`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CamSlotKey {
    /// Area-wide global `.cam` in Slot 0 (e.g. `"cagb"`, `"cbgb"`, `"ccgb"`, `"cdgb"`).
    Global(String),
    /// Per-tile/per-block `.cam` in Slots `1..=N` (e.g. `"ca11"`, `"ca12"`, `"cb54"`).
    Tile(String),
}

impl CamSlotKey {
    /// Returns the canonical lowercase room/file identifier for this slot.
    #[inline]
    #[must_use]
    pub fn id(&self) -> &str {
        match self {
            Self::Global(s) | Self::Tile(s) => s.as_str(),
        }
    }
}

/// A single parsed `.cam` file kept resident inside [`OutdoorCamBank`].
#[derive(Debug, Clone, PartialEq)]
pub struct LoadedCamSlot {
    /// Slot identity (`Global` vs `Tile`).
    pub key: CamSlotKey,
    /// Asset pack origin (`Sh2eeOverride` vs `VanillaRetail`).
    pub source: CamSourceKind,
    /// Whether SH2:EE override lookup was enabled when this slot was loaded.
    pub prefer_sh2ee: bool,
    /// Parsed road records for this slot alone.
    pub cam_file: CamFile,
}

/// Multi-slot resident `.cam` bank keeping `<area>GB.cam` alongside active outdoor tile `.cam`s.
///
/// Rebuilds a flattened [`CamFile`] for `Ps2CameraController` in deterministic priority-tie order:
/// 1. Active tile `.cam` (highest tie-break precedence at equal `rd_type` priority & distance)
/// 2. Other resident neighborhood tile `.cam`s (sorted deterministically by room ID)
/// 3. Global `<area>GB.cam` (lowest tie-break precedence at equal `rd_type` priority & distance)
///
/// This order combined with `<` tie-breaking in `tick.rs` reproduces the exact semantics of PS2/PC
/// `loadBgCAM_GetLoadedDataAddrList` (`0x00271f10`, global slot 0 first + tile slots 1..N after)
/// paired with `vcGetNearestNEAR_ROAD_DATA` (`0x001988b0` / `sh2pc.exe` `0x0051dc00`, `<=` tie-break
/// where later tile slots beat earlier global slot 0), while keeping active-tile record `[0]` at
/// flattened index `0` for single-room compatibility.
#[derive(Debug, Clone, Default)]
pub struct OutdoorCamBank {
    /// Resident area-wide `<area>GB.cam` slot (PS2 `lbCAM_Ctrl` slot 0 / PC `DAT_00ac6f18`).
    global_slot: Option<LoadedCamSlot>,
    /// Resident per-tile `.cam` slots (PS2 `lbCAM_Ctrl` slots 1..N / PC 8-slot block table).
    tile_slots: Vec<LoadedCamSlot>,
    /// Parallel mapping from each flattened `CamFile::records` index to `(CamSlotKey, local_record_idx)`
    /// so `active_road_index` is preserved across tile swaps (matching PS2 `cur_near_road.road_p`
    /// pointer identity in `vcSetCurNearRoadInVC_WORK` `0x00197af0`, `vc_main.c:1350`).
    flat_origins: Vec<(CamSlotKey, usize)>,
}

impl OutdoorCamBank {
    /// Clears all resident global and tile `.cam` slots and origin mappings.
    pub fn clear(&mut self) {
        self.global_slot = None;
        self.tile_slots.clear();
        self.flat_origins.clear();
    }

    /// Returns `true` if no global or tile `.cam` slots are currently resident.
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.global_slot.is_none() && self.tile_slots.is_empty()
    }

    /// Returns the resident global `<area>GB.cam` slot, if any.
    #[inline]
    #[must_use]
    pub fn global_slot(&self) -> Option<&LoadedCamSlot> {
        self.global_slot.as_ref()
    }

    /// Returns the canonical lowercase ID of the resident global `.cam` (e.g. `"cagb"`), if any.
    #[inline]
    #[must_use]
    pub fn global_cam_id(&self) -> Option<&str> {
        self.global_slot.as_ref().map(|s| s.key.id())
    }

    /// Returns a slice of all resident per-tile `.cam` slots.
    #[inline]
    #[must_use]
    pub fn tile_slots(&self) -> &[LoadedCamSlot] {
        &self.tile_slots
    }

    /// Returns `true` if a global slot matching `(gb_id, prefer_sh2ee)` is already resident.
    #[must_use]
    pub fn has_global(&self, gb_id: &str, prefer_sh2ee: bool) -> bool {
        self.global_slot.as_ref().is_some_and(|s| {
            s.key.id().eq_ignore_ascii_case(gb_id) && s.prefer_sh2ee == prefer_sh2ee
        })
    }

    /// Returns `true` if a tile slot matching `(tile_id, prefer_sh2ee)` is already resident.
    #[must_use]
    pub fn has_tile(&self, tile_id: &str, prefer_sh2ee: bool) -> bool {
        self.tile_slots
            .iter()
            .any(|s| s.key.id().eq_ignore_ascii_case(tile_id) && s.prefer_sh2ee == prefer_sh2ee)
    }

    /// Sets or clears the resident global `<area>GB.cam` slot (`loadBgCAM_ReplaceG` `0x00271d20`).
    pub fn set_global_slot(&mut self, slot: Option<LoadedCamSlot>) {
        self.global_slot = slot;
    }

    /// Inserts or replaces a per-tile `.cam` slot (`loadBgCAM_Replace` `0x00271e10`).
    pub fn upsert_tile_slot(&mut self, slot: LoadedCamSlot) {
        if let Some(existing) = self
            .tile_slots
            .iter_mut()
            .find(|s| s.key.id().eq_ignore_ascii_case(slot.key.id()))
        {
            *existing = slot;
        } else {
            self.tile_slots.push(slot);
        }
    }

    /// Retains only tile slots whose canonical lowercase IDs appear in `keep_ids`.
    pub fn retain_tiles(&mut self, keep_ids: &[String]) {
        self.tile_slots
            .retain(|s| keep_ids.iter().any(|k| k.eq_ignore_ascii_case(s.key.id())));
    }

    /// Retains only tile slots belonging to the 2-letter `area_prefix` (e.g. `"ca"`), evicting the
    /// oldest non-matching entries if the count exceeds `max_slots`.
    pub fn retain_area_and_cap(&mut self, area_prefix: &str, max_slots: usize) {
        self.tile_slots.retain(|s| {
            s.key
                .id()
                .get(0..2)
                .is_some_and(|p| p.eq_ignore_ascii_case(area_prefix))
        });
        while self.tile_slots.len() > max_slots {
            self.tile_slots.remove(0);
        }
    }

    /// Returns the `(CamSlotKey, local_record_idx)` origin of flattened record `flat_index`.
    #[inline]
    #[must_use]
    pub fn origin_of(&self, flat_index: usize) -> Option<&(CamSlotKey, usize)> {
        self.flat_origins.get(flat_index)
    }

    /// Finds the new flattened index for a previously active `(CamSlotKey, local_record_idx)` after
    /// rebuilding the merged [`CamFile`].
    #[must_use]
    pub fn remap_active_road_index(
        &self,
        prev_origin: Option<&(CamSlotKey, usize)>,
    ) -> Option<usize> {
        let target = prev_origin?;
        self.flat_origins.iter().position(|origin| origin == target)
    }

    /// Rebuilds the flattened merged [`CamFile`] and parallel `flat_origins` table.
    ///
    /// Order:
    /// 1. `primary_tile_id` (if resident in `tile_slots`)
    /// 2. Remaining resident `tile_slots` sorted lexicographically by room ID
    /// 3. Resident `global_slot` (`<area>GB.cam`)
    pub fn build_merged_cam(&mut self, primary_tile_id: Option<&str>) -> Option<CamFile> {
        self.flat_origins.clear();
        let mut records = Vec::new();

        // 1. Active tile first (highest tie-break precedence and index-0 compatibility).
        if let Some(primary) = primary_tile_id {
            if let Some(slot) = self
                .tile_slots
                .iter()
                .find(|s| s.key.id().eq_ignore_ascii_case(primary))
            {
                for (local_idx, rec) in slot.cam_file.records.iter().enumerate() {
                    records.push(rec.clone());
                    self.flat_origins.push((slot.key.clone(), local_idx));
                }
            }
        }

        // 2. Remaining resident neighborhood tiles in deterministic room-ID order.
        let mut other_indices: Vec<usize> = self
            .tile_slots
            .iter()
            .enumerate()
            .filter(|(_, s)| primary_tile_id.is_none_or(|p| !s.key.id().eq_ignore_ascii_case(p)))
            .map(|(i, _)| i)
            .collect();
        other_indices
            .sort_by(|&a, &b| self.tile_slots[a].key.id().cmp(self.tile_slots[b].key.id()));
        for idx in other_indices {
            let slot = &self.tile_slots[idx];
            for (local_idx, rec) in slot.cam_file.records.iter().enumerate() {
                records.push(rec.clone());
                self.flat_origins.push((slot.key.clone(), local_idx));
            }
        }

        // 3. Global `<area>GB.cam` last (lowest tie-break precedence at equal priority & distance).
        if let Some(ref global) = self.global_slot {
            for (local_idx, rec) in global.cam_file.records.iter().enumerate() {
                records.push(rec.clone());
                self.flat_origins.push((global.key.clone(), local_idx));
            }
        }

        if records.is_empty() {
            None
        } else {
            Some(CamFile { records })
        }
    }
}
