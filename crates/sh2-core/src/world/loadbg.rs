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

/// PS2 / PC outdoor resident distance threshold (`7,000 mm`) for immediate active tiles.
pub const LOADBG_RESIDENT_RADIUS_MM: f32 = 7_000.0;

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

/// Stateful outdoor `.map` tile streaming planner supporting PS2 `2x2` parity, Native `5x5`,
/// and Native Whole-Town GPU residency modes.
#[derive(Debug, Clone)]
pub struct OutdoorTileStreamer {
    mode: OutdoorStreamingMode,
    active_area: Option<OutdoorArea>,
    resident_blocks: HashSet<u16>,
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
        self.mode = mode;
    }

    /// Marks `block_no` as resident in GPU memory (called when a background load finishes uploading).
    pub fn mark_resident(&mut self, block_no: u16) {
        self.resident_blocks.insert(block_no);
    }

    /// Marks `block_no` as evicted from GPU memory.
    pub fn mark_evicted(&mut self, block_no: u16) {
        self.resident_blocks.remove(&block_no);
    }

    /// Clears all resident tracking (e.g. on stage transition).
    pub fn reset(&mut self) {
        self.active_area = None;
        self.resident_blocks.clear();
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

    /// Evaluates the outdoor streaming plan for `(stage_id, player_x_mm, player_z_mm)`.
    #[must_use]
    pub fn plan(&mut self, stage_id: u16, player_x_mm: f32, player_z_mm: f32) -> TileStreamingPlan {
        let Some(area) = OutdoorArea::from_stage_id(stage_id) else {
            return TileStreamingPlan::default();
        };

        if self.active_area != Some(area) {
            self.active_area = Some(area);
            self.resident_blocks.clear();
        }

        let (player_gx, player_gz) = OutdoorArea::world_to_grid(player_x_mm, player_z_mm);
        let mut candidates: Vec<TileLoadRequest> = match self.mode {
            OutdoorStreamingMode::ClassicPs2LoadBg2x2 => {
                Self::collect_ps2_2x2_tiles(area, player_x_mm, player_z_mm, player_gx, player_gz)
            }
            OutdoorStreamingMode::Native5x5Grid => Self::collect_grid_radius_tiles(
                area,
                player_x_mm,
                player_z_mm,
                player_gx,
                player_gz,
                2,
            ),
            OutdoorStreamingMode::NativeWholeTown => {
                Self::collect_whole_town_tiles(area, player_x_mm, player_z_mm)
            }
        };

        // Sort nearest-first (`loadBgAll_Sort2x2Block` `0x00270b60`), deterministic tie-break by block_no.
        candidates.sort_by(|a, b| {
            a.distance_mm
                .total_cmp(&b.distance_mm)
                .then_with(|| a.tile.block_no.cmp(&b.tile.block_no))
        });

        let desired_set: HashSet<u16> = candidates.iter().map(|c| c.tile.block_no).collect();
        let desired_blocks: Vec<u16> = candidates.iter().map(|c| c.tile.block_no).collect();

        let load_queue: Vec<TileLoadRequest> = candidates
            .into_iter()
            .filter(|c| !self.resident_blocks.contains(&c.tile.block_no))
            .collect();

        let mut evict_blocks: Vec<u16> = self
            .resident_blocks
            .iter()
            .copied()
            .filter(|b| !desired_set.contains(b))
            .collect();
        evict_blocks.sort_unstable();

        TileStreamingPlan {
            area: Some(area),
            priority_gb_cam: area.global_cam_rel_path(),
            desired_blocks,
            load_queue,
            evict_blocks,
        }
    }

    /// Exact PS2 `loadBgAll_PrepareAround` (`0x002715d0`) + `loadBgAll_SetLoadRequestOutdoor2x2`
    /// (`0x00270dd0`) `2x2` quadrant selection around `(player_x_mm, player_z_mm)`.
    fn collect_ps2_2x2_tiles(
        area: OutdoorArea,
        player_x_mm: f32,
        player_z_mm: f32,
        player_gx: i16,
        player_gz: i16,
    ) -> Vec<TileLoadRequest> {
        let (cx, cz) = OutdoorArea::grid_cell_center_mm(player_gx, player_gz);
        let dir_x: i16 = if player_x_mm < cx { -1 } else { 1 };
        let dir_z: i16 = if player_z_mm < cz { -1 } else { 1 };

        let quadrant = [
            (player_gx, player_gz),
            (player_gx + dir_x, player_gz),
            (player_gx, player_gz + dir_z),
            (player_gx + dir_x, player_gz + dir_z),
        ];

        let mut out = Vec::with_capacity(4);
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
                    out.push(TileLoadRequest {
                        map_rel_path: tile.map_rel_path(),
                        tile,
                        distance_mm: dist,
                        priority_band: Self::ps2_priority_band(dist),
                    });
                }
            }
        }
        out
    }

    /// Collects all valid tiles within Chebyshev radius `radius` (e.g. `radius = 2` -> `5x5` grid).
    fn collect_grid_radius_tiles(
        area: OutdoorArea,
        player_x_mm: f32,
        player_z_mm: f32,
        player_gx: i16,
        player_gz: i16,
        radius: i16,
    ) -> Vec<TileLoadRequest> {
        let mut out = Vec::new();
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
                out.push(TileLoadRequest {
                    map_rel_path: tile.map_rel_path(),
                    tile,
                    distance_mm: dist,
                    priority_band: Self::ps2_priority_band(dist),
                });
            }
        }
        out
    }

    /// Collects ALL tiles in the outdoor area for Native Whole-Town GPU Residency (`< 45 MB` VRAM).
    fn collect_whole_town_tiles(
        area: OutdoorArea,
        player_x_mm: f32,
        player_z_mm: f32,
    ) -> Vec<TileLoadRequest> {
        area.block_table()
            .iter()
            .map(|&(gx, gz, block_no)| {
                let (tcx, tcz) = OutdoorArea::grid_cell_center_mm(gx, gz);
                let dist = (player_x_mm - tcx).hypot(player_z_mm - tcz);
                let tile = OutdoorTileId {
                    area,
                    block_no,
                    gx,
                    gz,
                };
                TileLoadRequest {
                    map_rel_path: tile.map_rel_path(),
                    tile,
                    distance_mm: dist,
                    priority_band: Self::ps2_priority_band(dist),
                }
            })
            .collect()
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
