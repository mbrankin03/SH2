//! Background-streamed multi-tile outdoor `.map` + `.cld` GPU residency pool (`SH2-78`, `SH2-183`).
//!
//! Executes [`sh2_core::world::TileStreamingPlan`] (`loadbg_2x2` parity, `5x5` grid, or
//! Whole-Town GPU residency) by loading `.map` tiles via [`TextureLoader`] (merging shared
//! `<area>.map` textures, `sh2e/` HD maps, and 4K AI overrides), preparing CPU mipmaps/BC
//! textures, and parsing `.cld` collision tiles on background worker threads, then uploading
//! completed [`GpuMap`] buffers and per-tile texture bind groups on the render thread without
//! frame hitches.
//!
//! Resident `.cld` collision tiles are merged deterministically via [`CollisionWorld::merge`]
//! matching PS2 `loadBgCLD_GetLoadedDataAddrList` (`SLUS_202.28` `0x00272330`) and
//! `clGetHitSectListMOVEOutDoor` (`0x00138bc0`).

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use sh2_assets::formats::cld::parse_cld;
use sh2_assets::formats::map::MapFile;
use sh2_assets::formats::texture::TextureLoader;
use sh2_core::collision::CollisionWorld;
use sh2_core::config::WorldTextureQuality;
use sh2_core::events::stageparts::MapPartVisibilityFilter;
use sh2_core::world::{OutdoorArea, TileStreamingPlan};

use crate::context::{prepare_map_texture_cpu_owned, PreparedGpuTexture, RenderContext};
use crate::gpu_map::GpuMap;
use crate::scene::MapDrawItem;

/// Number of background I/O + texture-prep worker threads serving outdoor tile streaming.
const OUTDOOR_WORKER_THREADS: usize = 2;

struct LoadJob {
    generation: u64,
    area: OutdoorArea,
    block_no: u16,
    map_rel_path: String,
    cld_rel_path: String,
    map_name: String,
    data_dir: PathBuf,
    sh2e_dir: Option<PathBuf>,
    texture_quality: WorldTextureQuality,
    supports_bc: bool,
}

struct PreparedTileTexture {
    tex_id: u32,
    wrap_u: bool,
    wrap_v: bool,
    prepared: PreparedGpuTexture,
}

struct LoadedTile {
    generation: u64,
    area: OutdoorArea,
    block_no: u16,
    map: Option<MapFile>,
    prepared_textures: Vec<PreparedTileTexture>,
    collision: Option<CollisionWorld>,
}

/// A single GPU-resident outdoor tile holding its compiled [`GpuMap`], per-material diffuse
/// texture bind groups, and parsed `.cld` [`CollisionWorld`] (`SH2-183`).
pub struct ResidentOutdoorTile {
    /// Numeric block index (e.g. `11` for `ca11.map`, `54` for `cb54.map`).
    pub block_no: u16,
    /// Consolidated GPU vertex/index buffers and sorted batches.
    pub gpu_map: Arc<GpuMap>,
    /// Per-material diffuse texture bind groups indexed by [`crate::GpuMapBatch::material_index`].
    pub texture_bind_groups: Vec<Arc<wgpu::BindGroup>>,
    /// Parsed `.cld` collision sector for this tile, if present on disk.
    pub collision: Option<CollisionWorld>,
}

/// Manages GPU-resident outdoor `.map` + `.cld` tiles and asynchronous background worker I/O.
pub struct GpuOutdoorTilePool {
    data_dir: PathBuf,
    sh2e_dir: Option<PathBuf>,
    texture_quality: WorldTextureQuality,
    supports_bc: bool,
    generation: u64,
    active_area: Option<OutdoorArea>,
    resident: HashMap<u16, ResidentOutdoorTile>,
    in_flight: HashSet<(OutdoorArea, u16)>,
    collision_dirty: bool,
    job_tx: Option<Sender<LoadJob>>,
    result_rx: Receiver<LoadedTile>,
    workers: Vec<JoinHandle<()>>,
}

impl GpuOutdoorTilePool {
    /// Creates a new outdoor tile pool backed by background `.map`/`.cld` worker threads using
    /// `data_dir` only (`OriginalPs2` tier, uncompressed textures).
    #[must_use]
    pub fn new(data_dir: PathBuf) -> Self {
        Self::with_roots(data_dir, None, WorldTextureQuality::OriginalPs2, false)
    }

    /// Creates a new outdoor tile pool backed by background worker threads using `TextureLoader`
    /// across `data_dir` and optional `sh2e_dir` at the requested `texture_quality` tier (`SH2-183`).
    #[must_use]
    pub fn with_roots(
        data_dir: PathBuf,
        sh2e_dir: Option<PathBuf>,
        texture_quality: WorldTextureQuality,
        supports_bc: bool,
    ) -> Self {
        let (job_tx, job_rx) = mpsc::channel::<LoadJob>();
        let shared_rx = Arc::new(Mutex::new(job_rx));
        let (result_tx, result_rx) = mpsc::channel::<LoadedTile>();

        let mut workers = Vec::with_capacity(OUTDOOR_WORKER_THREADS);
        for idx in 0..OUTDOOR_WORKER_THREADS {
            let rx = Arc::clone(&shared_rx);
            let tx = result_tx.clone();
            if let Ok(handle) = thread::Builder::new()
                .name(format!("sh2-outdoor-tile-loader-{idx}"))
                .spawn(move || loop {
                    let job = {
                        let Ok(guard) = rx.lock() else {
                            break;
                        };
                        match guard.recv() {
                            Ok(job) => job,
                            Err(_) => break,
                        }
                    };
                    let loaded = process_load_job(job);
                    if tx.send(loaded).is_err() {
                        break;
                    }
                })
            {
                workers.push(handle);
            }
        }

        Self {
            data_dir,
            sh2e_dir,
            texture_quality,
            supports_bc,
            generation: 0,
            active_area: None,
            resident: HashMap::new(),
            in_flight: HashSet::new(),
            collision_dirty: false,
            job_tx: Some(job_tx),
            result_rx,
            workers,
        }
    }

    /// Returns the active [`OutdoorArea`] currently resident in the pool, if any.
    #[inline]
    #[must_use]
    pub const fn active_area(&self) -> Option<OutdoorArea> {
        self.active_area
    }

    /// Updates the active [`WorldTextureQuality`] tier. When the tier changes, all resident
    /// outdoor tiles are cleared and `generation` is bumped so tiles re-stream at the new tier.
    ///
    /// Returns `true` if the quality tier changed and the caller should reset
    /// [`sh2_core::world::OutdoorTileStreamer`].
    pub fn set_texture_quality(&mut self, quality: WorldTextureQuality) -> bool {
        if self.texture_quality == quality {
            return false;
        }
        self.texture_quality = quality;
        self.clear();
        true
    }

    /// Clears all resident tiles and cancels in-flight results (e.g. when entering an indoor stage
    /// or returning to the title screen).
    pub fn clear(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.active_area = None;
        self.resident.clear();
        self.in_flight.clear();
        self.collision_dirty = false;
    }

    /// Seeds an already-loaded tile (e.g. the initial outdoor tile loaded synchronously on stage
    /// entry) directly into the resident pool so frame 0 renders immediately without a redundant
    /// disk load (`SH2-183`).
    pub fn seed_tile(
        &mut self,
        area: OutdoorArea,
        block_no: u16,
        gpu_map: Arc<GpuMap>,
        texture_bind_groups: Vec<Arc<wgpu::BindGroup>>,
        collision: Option<CollisionWorld>,
    ) {
        if self.active_area != Some(area) {
            self.generation = self.generation.wrapping_add(1);
            self.active_area = Some(area);
            self.resident.clear();
            self.in_flight.clear();
        }
        self.in_flight.remove(&(area, block_no));
        if collision.is_some() {
            self.collision_dirty = true;
        }
        self.resident.insert(
            block_no,
            ResidentOutdoorTile {
                block_no,
                gpu_map,
                texture_bind_groups,
                collision,
            },
        );
    }

    /// Submits new tile load requests and evicts out-of-range tiles according to `plan`.
    ///
    /// Returns the number of newly queued background load jobs.
    pub fn apply_plan(&mut self, plan: &TileStreamingPlan) -> usize {
        let Some(area) = plan.area else {
            return 0;
        };

        if self.active_area != Some(area) {
            self.generation = self.generation.wrapping_add(1);
            self.active_area = Some(area);
            self.resident.clear();
            self.in_flight.clear();
            self.collision_dirty = false;
        }

        for &block_no in &plan.evict_blocks {
            if let Some(removed) = self.resident.remove(&block_no) {
                if removed.collision.is_some() {
                    self.collision_dirty = true;
                }
            }
            self.in_flight.remove(&(area, block_no));
        }

        let mut queued = 0usize;
        if let Some(tx) = &self.job_tx {
            for req in &plan.load_queue {
                let block_no = req.tile.block_no;
                if self.resident.contains_key(&block_no)
                    || self.in_flight.contains(&(area, block_no))
                {
                    continue;
                }
                let map_name = format!("{}{:02}", area.prefix(), block_no);
                if tx
                    .send(LoadJob {
                        generation: self.generation,
                        area,
                        block_no,
                        map_rel_path: req.map_rel_path.clone(),
                        cld_rel_path: req.tile.cld_rel_path(),
                        map_name,
                        data_dir: self.data_dir.clone(),
                        sh2e_dir: self.sh2e_dir.clone(),
                        texture_quality: self.texture_quality,
                        supports_bc: self.supports_bc,
                    })
                    .is_ok()
                {
                    self.in_flight.insert((area, block_no));
                    queued += 1;
                }
            }
        }
        queued
    }

    /// Drains up to `max_uploads_per_frame` completed `.map` + `.cld` tiles from background
    /// workers and uploads their GPU geometry and pre-prepared textures on the render thread.
    ///
    /// Returns the list of `block_no` IDs newly uploaded this frame so the caller can notify
    /// [`sh2_core::world::OutdoorTileStreamer::mark_resident`].
    pub fn poll_uploads(
        &mut self,
        render_ctx: &RenderContext,
        max_uploads_per_frame: usize,
    ) -> Vec<u16> {
        let mut uploaded = Vec::new();
        let limit = max_uploads_per_frame.max(1);

        while uploaded.len() < limit {
            let Ok(done) = self.result_rx.try_recv() else {
                break;
            };
            // Check generation and active area BEFORE removing from `in_flight` so a stale
            // generation-N completion can never delete a live generation-(N+1) `in_flight`
            // entry, and stale completions do not consume `max_uploads_per_frame` budget.
            if done.generation != self.generation || self.active_area != Some(done.area) {
                continue;
            }
            self.in_flight.remove(&(done.area, done.block_no));
            if let Some(map) = done.map {
                let gpu_map = Arc::new(GpuMap::from_map_with_room_id(
                    &render_ctx.device,
                    &map,
                    done.block_no,
                ));
                let mut tex_bg_cache: HashMap<u32, Arc<wgpu::BindGroup>> =
                    HashMap::with_capacity(done.prepared_textures.len());
                for pt in &done.prepared_textures {
                    let bg = render_ctx.upload_prepared_texture_bind_group_with_wrap(
                        &pt.prepared,
                        pt.wrap_u,
                        pt.wrap_v,
                        Some(&format!("Outdoor Tile {} Texture {}", map.name, pt.tex_id)),
                    );
                    tex_bg_cache.insert(pt.tex_id, bg);
                }
                let mut texture_bind_groups = Vec::with_capacity(map.materials.len());
                for mat in &map.materials {
                    let tex_id = u32::from(mat.texture_id);
                    if let Some(bg) = tex_bg_cache.get(&tex_id) {
                        texture_bind_groups.push(Arc::clone(bg));
                    } else {
                        texture_bind_groups
                            .push(Arc::clone(&render_ctx.default_texture_bind_group));
                    }
                }
                if done.collision.is_some() {
                    self.collision_dirty = true;
                }
                self.resident.insert(
                    done.block_no,
                    ResidentOutdoorTile {
                        block_no: done.block_no,
                        gpu_map,
                        texture_bind_groups,
                        collision: done.collision,
                    },
                );
                uploaded.push(done.block_no);
            }
        }
        uploaded
    }

    /// Rebuilds the merged multi-tile outdoor [`CollisionWorld`] via [`CollisionWorld::merge`]
    /// whenever resident `.cld` tiles have been added or evicted (`loadBgCLD_GetLoadedDataAddrList`
    /// `SLUS_202.28` `0x00272330`, `SH2-183`).
    pub fn take_merged_collision_if_dirty(&mut self) -> Option<CollisionWorld> {
        if !self.collision_dirty {
            return None;
        }
        self.collision_dirty = false;
        let mut block_ids: Vec<u16> = self.resident.keys().copied().collect();
        block_ids.sort_unstable();
        let parts: Vec<&CollisionWorld> = block_ids
            .iter()
            .filter_map(|id| self.resident.get(id)?.collision.as_ref())
            .collect();
        if parts.is_empty() {
            None
        } else {
            Some(CollisionWorld::merge(&parts))
        }
    }

    /// Collects borrowed [`MapDrawItem`] references for all currently resident outdoor tiles in
    /// deterministic `block_no` order for unified single-pass multi-map rendering (`SH2-183`).
    pub fn collect_draw_items<'a>(&'a self, out: &mut Vec<MapDrawItem<'a>>) {
        out.clear();
        let mut tiles: Vec<&ResidentOutdoorTile> = self.resident.values().collect();
        tiles.sort_unstable_by_key(|t| t.block_no);
        for tile in tiles {
            out.push(MapDrawItem::new(
                &tile.gpu_map,
                Some(tile.texture_bind_groups.as_slice()),
            ));
        }
    }

    /// Applies the active per-frame `stageparts` visibility filter across all resident outdoor tiles.
    pub fn set_visibility_filter(&mut self, filter: Option<&MapPartVisibilityFilter>) {
        for tile in self.resident.values_mut() {
            if let Some(gpu_map) = Arc::get_mut(&mut tile.gpu_map) {
                gpu_map.set_visibility_filter(filter.cloned());
            }
        }
    }

    /// Returns a resident [`GpuMap`] by `block_no`.
    #[must_use]
    pub fn get(&self, block_no: u16) -> Option<&GpuMap> {
        self.resident.get(&block_no).map(|t| t.gpu_map.as_ref())
    }

    /// Returns a resident [`ResidentOutdoorTile`] by `block_no`.
    #[must_use]
    pub fn get_tile(&self, block_no: u16) -> Option<&ResidentOutdoorTile> {
        self.resident.get(&block_no)
    }

    /// Iterates over all currently GPU-resident `(block_no, &GpuMap)` outdoor tiles.
    pub fn resident_tiles(&self) -> impl Iterator<Item = (u16, &GpuMap)> {
        self.resident
            .iter()
            .map(|(&id, tile)| (id, tile.gpu_map.as_ref()))
    }

    /// Number of currently GPU-resident outdoor tiles.
    #[must_use]
    pub fn resident_count(&self) -> usize {
        self.resident.len()
    }

    /// Number of tiles currently being read/parsed on background worker threads.
    #[must_use]
    pub fn in_flight_count(&self) -> usize {
        self.in_flight.len()
    }

    /// Total GPU vertex + index buffer bytes currently resident across all loaded outdoor tiles.
    #[must_use]
    pub fn total_gpu_bytes(&self) -> usize {
        self.resident
            .values()
            .map(|t| t.gpu_map.total_gpu_bytes)
            .sum()
    }
}

fn process_load_job(job: LoadJob) -> LoadedTile {
    let loader = TextureLoader::new(job.data_dir.clone(), job.sh2e_dir.clone());
    let (prefer_sh2ee, apply_4k) = match job.texture_quality {
        WorldTextureQuality::Ai4kMaster => (true, true),
        WorldTextureQuality::Sh2eeHd => (true, false),
        WorldTextureQuality::OriginalPs2 => (false, false),
    };
    let mut map = loader
        .load_map_for_enhancement(&job.map_rel_path, &job.map_name, prefer_sh2ee, apply_4k)
        .or_else(|err| match job.texture_quality {
            WorldTextureQuality::Sh2eeHd => {
                loader.load_map_with_hd_overrides(&job.map_rel_path, &job.map_name)
            }
            WorldTextureQuality::OriginalPs2 => {
                loader.load_sh2e_map(&job.map_rel_path, &job.map_name)
            }
            WorldTextureQuality::Ai4kMaster => Err(err),
        })
        .ok();

    let mut prepared_textures = Vec::new();
    if let Some(ref mut map_file) = map {
        let mut seen = HashSet::new();
        let mut unique_tids = Vec::new();
        for mat in &map_file.materials {
            let tid = u32::from(mat.texture_id);
            if seen.insert(tid) {
                unique_tids.push(tid);
            }
        }
        for tid in unique_tids {
            let (wrap_u, wrap_v) = map_file.texture_uv_wrap_axes(tid);
            if let Some(tex) = map_file.textures.remove(&tid) {
                let (w, h) = (tex.width as u32, tex.height as u32);
                if let Ok(rgba) = tex.into_rgba() {
                    let prepared = prepare_map_texture_cpu_owned(w, h, rgba, job.supports_bc);
                    prepared_textures.push(PreparedTileTexture {
                        tex_id: tid,
                        wrap_u,
                        wrap_v,
                        prepared,
                    });
                }
            }
        }
    }

    let cld_path = job.data_dir.join(&job.cld_rel_path);
    let collision = std::fs::read(&cld_path)
        .ok()
        .and_then(|bytes| parse_cld(&bytes).ok())
        .map(|cld| CollisionWorld::from_cld(&cld));

    LoadedTile {
        generation: job.generation,
        area: job.area,
        block_no: job.block_no,
        map,
        prepared_textures,
        collision,
    }
}

impl Drop for GpuOutdoorTilePool {
    fn drop(&mut self) {
        self.job_tx.take();
        for handle in self.workers.drain(..) {
            let _ = handle.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RendererConfig;

    #[test]
    fn stale_generation_result_preserves_live_in_flight_and_upload_budget() {
        let Some(ctx) = RenderContext::new_headless(RendererConfig::default()) else {
            return;
        };

        let (result_tx, result_rx) = mpsc::channel::<LoadedTile>();
        let mut pool = GpuOutdoorTilePool {
            data_dir: PathBuf::new(),
            sh2e_dir: None,
            texture_quality: WorldTextureQuality::OriginalPs2,
            supports_bc: false,
            generation: 2,
            active_area: Some(OutdoorArea::Ca),
            resident: HashMap::new(),
            in_flight: HashSet::from([(OutdoorArea::Ca, 11), (OutdoorArea::Ca, 12)]),
            collision_dirty: false,
            job_tx: None,
            result_rx,
            workers: Vec::new(),
        };

        // Enqueue 3 stale generation-1 completions for (Ca, 11) followed by 1 live generation-2
        // completion for (Ca, 12) with an empty MapFile.
        for _ in 0..3 {
            result_tx
                .send(LoadedTile {
                    generation: 1,
                    area: OutdoorArea::Ca,
                    block_no: 11,
                    map: None,
                    prepared_textures: Vec::new(),
                    collision: None,
                })
                .unwrap();
        }
        result_tx
            .send(LoadedTile {
                generation: 2,
                area: OutdoorArea::Ca,
                block_no: 12,
                map: Some(MapFile {
                    name: "ca12".to_string(),
                    batches: Vec::new(),
                    materials: Vec::new(),
                    textures: HashMap::new(),
                }),
                prepared_textures: Vec::new(),
                collision: None,
            })
            .unwrap();

        // With max_uploads_per_frame = 1, all 3 stale results must be drained without consuming
        // upload budget, live (Ca, 11) must remain in_flight, and live (Ca, 12) must upload.
        let uploaded = pool.poll_uploads(&ctx, 1);
        assert_eq!(uploaded, vec![12]);
        assert!(
            pool.in_flight.contains(&(OutdoorArea::Ca, 11)),
            "stale generation-1 completion must not remove live generation-2 in_flight entry"
        );
        assert!(!pool.in_flight.contains(&(OutdoorArea::Ca, 12)));
        assert!(pool.get(12).is_some());
    }
}
