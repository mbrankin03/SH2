//! Outdoor Multi-Tile Streaming Planner (`LoadBg/loadbg_2x2.c` & `loadbg_all.c`).
//!
//! Verified 1:1 against PS2 prototype `SLUS_202.28`:
//! - `loadBgAll_SetLoadRequestOutdoor2x2` (`0x00270dd0`): 7 squared-distance priority bands
//!   (`7500`, `8500`, `9500`, `10500`, `11500`, `12500`, `14000 mm`) + `17500 mm` prefetch cutoff.
//! - `loadBgAll_PrepareAround` (`0x002715d0`): selects the `2x2` quadrant around the player
//!   sorted by distance to tile border (`10000.0 - |local_offset|`).
//! - Native Rust + `wgpu` upgrades (`OutdoorStreamingMode::Native5x5Grid` and
//!   `OutdoorStreamingMode::NativeWholeTown`) pre-stream a `5x5` neighborhood or the entire
//!   outdoor stage onto background-loaded GPU buffers while preserving `<area>GB.cam` priority.

use std::collections::HashSet;

use crate::config::OutdoorStreamingMode;
use crate::world::outdoor_grid::{OutdoorArea, OutdoorTileId};

/// Exact 7 priority distance bands in PS2 mm from `loadBgAll_SetLoadRequestOutdoor2x2`
/// (`SLUS_202.28` `0x00270e54..0x00270f54`: `0x4c5693a4`..`0x4d3aeb90`).
pub const LOADBG_DIST_BANDS_MM: [f32; 7] = [
    7_500.0, 8_500.0, 9_500.0, 10_500.0, 11_500.0, 12_500.0, 14_000.0,
];

/// Outer prefetch cutoff in PS2 mm (`0x4d920808` = `17,500.0^2`, `SLUS_202.28` `0x00270f70`).
pub const LOADBG_PREFETCH_CUTOFF_MM: f32 = 17_500.0;

/// A prioritized outdoor `.map` tile load request emitted by [`OutdoorTileStreamer`].
#[derive(Debug, Clone, PartialEq)]
pub struct TileLoadRequest {
    /// Target outdoor tile identifier.
    pub tile: OutdoorTileId,
    /// Relative `.map` path under `data/` (e.g. `"bg/cb/cb29.map"`).
    pub map_rel_path: String,
    /// Distance in PS2 mm from the player to the tile's center.
    pub distance_mm: f32,
    /// PS2 priority tier (`0` = highest / current tile, `1..=7` = distance bands, `8` = background prefetch).
    pub priority_band: u8,
}

/// Output of a single [`OutdoorTileStreamer::plan`] evaluation.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TileStreamingPlan {
    /// Active outdoor area (`None` for indoor stages).
    pub area: Option<OutdoorArea>,
    /// Global `<area>GB.cam` camera road path that must remain resident with highest priority.
    pub priority_gb_cam: Option<&'static str>,
    /// Complete set of tile block IDs that should be resident in GPU memory.
    pub desired_blocks: Vec<u16>,
    /// Tiles not yet resident that should be queued onto background worker threads (sorted nearest-first).
    pub load_queue: Vec<TileLoadRequest>,
    /// Currently resident tile block IDs that are outside the desired set and may be evicted.
    pub evict_blocks: Vec<u16>,
}

/// Cache key identifying when an [`OutdoorTileStreamer`] plan can be reused without replanning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PlanCacheKey {
    stage_id: u16,
    gx: i16,
    gz: i16,
    mode: OutdoorStreamingMode,
    quadrant: (i8, i8),
    cutoff_mask: u8,
}

/// Stateful outdoor `.map` tile streaming planner supporting PS2 `2x2` parity, Native `5x5`,
/// and Native Whole-Town GPU residency modes.
#[derive(Debug, Clone)]
pub struct OutdoorTileStreamer {
    mode: OutdoorStreamingMode,
    active_area: Option<OutdoorArea>,
    resident_blocks: HashSet<u16>,
    /// Cache key of [`Self::cached_plan`]; invalidated on resident/mode/reset changes.
    last_plan_key: Option<PlanCacheKey>,
    /// Cached most recent streaming plan (reuses its internal `Vec` capacities across replans).
    cached_plan: TileStreamingPlan,
    /// Reusable scratch buffer of `(tile, distance_mm, priority_band)` (no `String` formatting).
    scratch_candidates: Vec<(OutdoorTileId, f32, u8)>,
    /// Reusable scratch set of desired block IDs for eviction filtering.
    scratch_desired_set: HashSet<u16>,
}

impl Default for OutdoorTileStreamer {
    fn default() -> Self {
        Self::new(OutdoorStreamingMode::default())
    }
}

impl OutdoorTileStreamer {
    /// Creates a new `OutdoorTileStreamer` configured with `mode`.
    #[must_use]
    pub fn new(mode: OutdoorStreamingMode) -> Self {
        Self {
            mode,
            active_area: None,
            resident_blocks: HashSet::new(),
            last_plan_key: None,
            cached_plan: TileStreamingPlan::default(),
            scratch_candidates: Vec::with_capacity(80),
            scratch_desired_set: HashSet::with_capacity(80),
        }
    }

    /// Returns the active [`OutdoorStreamingMode`].
    #[inline]
    #[must_use]
    pub const fn mode(&self) -> OutdoorStreamingMode {
        self.mode
    }

    /// Updates the active [`OutdoorStreamingMode`] (e.g. when toggled live in the Options menu).
    pub fn set_mode(&mut self, mode: OutdoorStreamingMode) {
        if self.mode != mode {
            self.mode = mode;
            self.last_plan_key = None;
        }
    }

    /// Marks `block_no` as resident in GPU memory (called when a background load finishes uploading).
    pub fn mark_resident(&mut self, block_no: u16) {
        if self.resident_blocks.insert(block_no) {
            self.last_plan_key = None;
        }
    }

    /// Marks `block_no` as evicted from GPU memory.
    pub fn mark_evicted(&mut self, block_no: u16) {
        if self.resident_blocks.remove(&block_no) {
            self.last_plan_key = None;
        }
    }

    /// Clears all resident tracking (e.g. on stage transition).
    pub fn reset(&mut self) {
        self.active_area = None;
        self.resident_blocks.clear();
        self.last_plan_key = None;
        self.cached_plan.area = None;
        self.cached_plan.priority_gb_cam = None;
        self.cached_plan.desired_blocks.clear();
        self.cached_plan.load_queue.clear();
        self.cached_plan.evict_blocks.clear();
    }

    /// Computes the PS2 `loadBgAll_SetLoadRequestOutdoor2x2` (`0x00270e50..0x00270f68`) priority band
    /// (`0` = closest, `7` = farthest) for a given distance in PS2 mm.
    #[must_use]
    pub fn ps2_priority_band(distance_mm: f32) -> u8 {
        let mut band = 0u8;
        for &threshold in &LOADBG_DIST_BANDS_MM {
            if distance_mm > threshold {
                band += 1;
            }
        }
        band
    }

    /// Evaluates the outdoor streaming plan for `(stage_id, player_x_mm, player_z_mm)`, returning
    /// a borrowed reference to the cached plan and a boolean indicating whether the plan changed.
    ///
    /// Performs zero heap allocations when `(stage_id, gx, gz, mode, quadrant, cutoff_mask)` and
    /// residency state are unchanged since the previous tick.
    pub fn plan_ref(
        &mut self,
        stage_id: u16,
        player_x_mm: f32,
        player_z_mm: f32,
    ) -> (&TileStreamingPlan, bool) {
        let Some(area) = OutdoorArea::from_stage_id(stage_id) else {
            let changed = self.cached_plan.area.is_some() || self.last_plan_key.is_some();
            if changed {
                self.last_plan_key = None;
                self.cached_plan.area = None;
                self.cached_plan.priority_gb_cam = None;
                self.cached_plan.desired_blocks.clear();
                self.cached_plan.load_queue.clear();
                self.cached_plan.evict_blocks.clear();
            }
            return (&self.cached_plan, false);
        };

        if self.active_area != Some(area) {
            self.active_area = Some(area);
            self.resident_blocks.clear();
            self.last_plan_key = None;
        }

        let (player_gx, player_gz) = OutdoorArea::world_to_grid(player_x_mm, player_z_mm);
        let (quadrant, cutoff_mask) = match self.mode {
            OutdoorStreamingMode::ClassicPs2LoadBg2x2 => {
                let (cx, cz) = OutdoorArea::grid_cell_center_mm(player_gx, player_gz);
                let dir_x: i8 = if player_x_mm < cx { -1 } else { 1 };
                let dir_z: i8 = if player_z_mm < cz { -1 } else { 1 };
                let offsets = [
                    (player_gx, player_gz),
                    (player_gx + i16::from(dir_x), player_gz),
                    (player_gx, player_gz + i16::from(dir_z)),
                    (player_gx + i16::from(dir_x), player_gz + i16::from(dir_z)),
                ];
                let cutoff_sq = LOADBG_PREFETCH_CUTOFF_MM * LOADBG_PREFETCH_CUTOFF_MM;
                let mut mask = 1u8; // current cell always included
                for (i, &(gx, gz)) in offsets.iter().enumerate().skip(1) {
                    let (tcx, tcz) = OutdoorArea::grid_cell_center_mm(gx, gz);
                    let dx = player_x_mm - tcx;
                    let dz = player_z_mm - tcz;
                    if dx * dx + dz * dz < cutoff_sq {
                        mask |= 1u8 << i;
                    }
                }
                ((dir_x, dir_z), mask)
            }
            OutdoorStreamingMode::Native5x5Grid | OutdoorStreamingMode::NativeWholeTown => {
                ((0, 0), 0)
            }
        };

        let key = PlanCacheKey {
            stage_id,
            gx: player_gx,
            gz: player_gz,
            mode: self.mode,
            quadrant,
            cutoff_mask,
        };
        if self.last_plan_key == Some(key) {
            return (&self.cached_plan, false);
        }

        self.scratch_candidates.clear();
        match self.mode {
            OutdoorStreamingMode::ClassicPs2LoadBg2x2 => Self::collect_ps2_2x2_tiles(
                area,
                player_x_mm,
                player_z_mm,
                player_gx,
                player_gz,
                quadrant,
                &mut self.scratch_candidates,
            ),
            OutdoorStreamingMode::Native5x5Grid => Self::collect_grid_radius_tiles(
                area,
                player_x_mm,
                player_z_mm,
                player_gx,
                player_gz,
                2,
                &mut self.scratch_candidates,
            ),
            OutdoorStreamingMode::NativeWholeTown => Self::collect_whole_town_tiles(
                area,
                player_x_mm,
                player_z_mm,
                &mut self.scratch_candidates,
            ),
        }

        // Sort nearest-first (`loadBgAll_Sort2x2Block` `0x00270b60`), deterministic tie-break by block_no.
        self.scratch_candidates.sort_by(|a, b| {
            a.1.total_cmp(&b.1)
                .then_with(|| a.0.block_no.cmp(&b.0.block_no))
        });

        self.scratch_desired_set.clear();
        self.cached_plan.area = Some(area);
        self.cached_plan.priority_gb_cam = area.global_cam_rel_path();
        self.cached_plan.desired_blocks.clear();
        self.cached_plan.load_queue.clear();
        self.cached_plan.evict_blocks.clear();

        for &(tile, distance_mm, priority_band) in &self.scratch_candidates {
            self.scratch_desired_set.insert(tile.block_no);
            self.cached_plan.desired_blocks.push(tile.block_no);
            if !self.resident_blocks.contains(&tile.block_no) {
                self.cached_plan.load_queue.push(TileLoadRequest {
                    map_rel_path: tile.map_rel_path(),
                    tile,
                    distance_mm,
                    priority_band,
                });
            }
        }

        for &block_no in &self.resident_blocks {
            if !self.scratch_desired_set.contains(&block_no) {
                self.cached_plan.evict_blocks.push(block_no);
            }
        }
        self.cached_plan.evict_blocks.sort_unstable();

        self.last_plan_key = Some(key);
        (&self.cached_plan, true)
    }

    /// Evaluates the outdoor streaming plan for `(stage_id, player_x_mm, player_z_mm)`, returning
    /// `Some(plan)` only when the streaming plan changed since the previous call.
    ///
    /// Zero-allocation fast path for 60 Hz campaign ticks when the player remains in the same
    /// `(stage_id, gx, gz, mode, quadrant)` and residency state is unchanged.
    #[must_use]
    pub fn plan_if_changed(
        &mut self,
        stage_id: u16,
        player_x_mm: f32,
        player_z_mm: f32,
    ) -> Option<TileStreamingPlan> {
        let (plan, changed) = self.plan_ref(stage_id, player_x_mm, player_z_mm);
        if changed && plan.area.is_some() {
            Some(plan.clone())
        } else {
            None
        }
    }

    /// Evaluates the outdoor streaming plan for `(stage_id, player_x_mm, player_z_mm)`.
    #[must_use]
    pub fn plan(&mut self, stage_id: u16, player_x_mm: f32, player_z_mm: f32) -> TileStreamingPlan {
        self.plan_ref(stage_id, player_x_mm, player_z_mm).0.clone()
    }

    /// Exact PS2 `loadBgAll_PrepareAround` (`0x002715d0`) + `loadBgAll_SetLoadRequestOutdoor2x2`
    /// (`0x00270dd0`) `2x2` quadrant selection around `(player_x_mm, player_z_mm)`.
    fn collect_ps2_2x2_tiles(
        area: OutdoorArea,
        player_x_mm: f32,
        player_z_mm: f32,
        player_gx: i16,
        player_gz: i16,
        quadrant_dir: (i8, i8),
        out: &mut Vec<(OutdoorTileId, f32, u8)>,
    ) {
        let dir_x = i16::from(quadrant_dir.0);
        let dir_z = i16::from(quadrant_dir.1);

        let quadrant = [
            (player_gx, player_gz),
            (player_gx + dir_x, player_gz),
            (player_gx, player_gz + dir_z),
            (player_gx + dir_x, player_gz + dir_z),
        ];

        for (gx, gz) in quadrant {
            if let Some(block_no) = area.block_no_at(gx, gz) {
                let (tcx, tcz) = OutdoorArea::grid_cell_center_mm(gx, gz);
                let dist = (player_x_mm - tcx).hypot(player_z_mm - tcz);
                // Keep current tile unconditionally; neighbors within 17,500 mm prefetch cutoff.
                if (gx == player_gx && gz == player_gz) || dist < LOADBG_PREFETCH_CUTOFF_MM {
                    let tile = OutdoorTileId {
                        area,
                        block_no,
                        gx,
                        gz,
                    };
                    out.push((tile, dist, Self::ps2_priority_band(dist)));
                }
            }
        }
    }

    /// Collects all valid tiles within Chebyshev radius `radius` (e.g. `radius = 2` -> `5x5` grid).
    fn collect_grid_radius_tiles(
        area: OutdoorArea,
        player_x_mm: f32,
        player_z_mm: f32,
        player_gx: i16,
        player_gz: i16,
        radius: i16,
        out: &mut Vec<(OutdoorTileId, f32, u8)>,
    ) {
        for &(gx, gz, block_no) in area.block_table() {
            if (gx - player_gx).abs() <= radius && (gz - player_gz).abs() <= radius {
                let (tcx, tcz) = OutdoorArea::grid_cell_center_mm(gx, gz);
                let dist = (player_x_mm - tcx).hypot(player_z_mm - tcz);
                let tile = OutdoorTileId {
                    area,
                    block_no,
                    gx,
                    gz,
                };
                out.push((tile, dist, Self::ps2_priority_band(dist)));
            }
        }
    }

    /// Collects ALL tiles in the outdoor area for Native Whole-Town GPU Residency (`< 45 MB` VRAM).
    fn collect_whole_town_tiles(
        area: OutdoorArea,
        player_x_mm: f32,
        player_z_mm: f32,
        out: &mut Vec<(OutdoorTileId, f32, u8)>,
    ) {
        for &(gx, gz, block_no) in area.block_table() {
            let (tcx, tcz) = OutdoorArea::grid_cell_center_mm(gx, gz);
            let dist = (player_x_mm - tcx).hypot(player_z_mm - tcz);
            let tile = OutdoorTileId {
                area,
                block_no,
                gx,
                gz,
            };
            out.push((tile, dist, Self::ps2_priority_band(dist)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ps2_2x2_quadrant_and_gb_cam_priority() {
        let mut streamer = OutdoorTileStreamer::new(OutdoorStreamingMode::ClassicPs2LoadBg2x2);
        // Town East (`cb`, Stage 5): cell (-3, -1) is `cb28`, center (-50000, -10000).
        // Stand slightly south-east of center -> selects 2x2 quadrant of up to 4 tiles.
        let plan = streamer.plan(5, -48_000.0, -8_000.0);
        assert_eq!(plan.area, Some(OutdoorArea::Cb));
        assert_eq!(plan.priority_gb_cam, Some("bg/cb/cbGB.cam"));
        assert!(!plan.desired_blocks.is_empty());
        assert!(plan.desired_blocks.len() <= 4);
        assert_eq!(plan.desired_blocks[0], 28); // cb28 is nearest
    }

    #[test]
    fn test_native_5x5_and_whole_town_residency_modes() {
        let mut s_5x5 = OutdoorTileStreamer::new(OutdoorStreamingMode::Native5x5Grid);
        let plan_5x5 = s_5x5.plan(5, -50_000.0, -10_000.0);
        assert!(plan_5x5.desired_blocks.len() > 4);
        assert!(plan_5x5.desired_blocks.len() <= 25);

        let mut s_whole = OutdoorTileStreamer::new(OutdoorStreamingMode::NativeWholeTown);
        let plan_whole = s_whole.plan(5, -50_000.0, -10_000.0);
        assert_eq!(
            plan_whole.desired_blocks.len(),
            crate::world::CB_OUTDOOR_BLOCKS.len()
        );
        // Nearest tile is still queued first so background streaming prioritizes the player's immediate area.
        assert_eq!(plan_whole.load_queue[0].tile.block_no, 28);
        assert!(plan_whole.evict_blocks.is_empty());
    }
}
