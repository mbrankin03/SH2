//! Background-streamed multi-tile outdoor `.map` GPU residency pool (`SH2-78`).
//!
//! Executes [`sh2_core::world::TileStreamingPlan`] (`loadbg_2x2` parity, `5x5` grid, or
//! Whole-Town GPU residency) by parsing `.map` files on a background worker thread and uploading
//! completed [`MapFile`]s into [`GpuMap`] buffers on the render thread without frame hitches.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};

use sh2_assets::formats::map::{parse_map, MapFile};
use sh2_core::events::stageparts::MapPartVisibilityFilter;
use sh2_core::world::{OutdoorArea, TileStreamingPlan};

use crate::gpu_map::GpuMap;

struct LoadJob {
    area: OutdoorArea,
    block_no: u16,
    abs_path: PathBuf,
    map_name: String,
}

struct LoadedTile {
    area: OutdoorArea,
    block_no: u16,
    map: Option<MapFile>,
}

/// Manages GPU-resident outdoor `.map` tiles and asynchronous disk/parse worker I/O.
pub struct GpuOutdoorTilePool {
    data_dir: PathBuf,
    active_area: Option<OutdoorArea>,
    resident: HashMap<u16, GpuMap>,
    in_flight: HashSet<(OutdoorArea, u16)>,
    job_tx: Option<Sender<LoadJob>>,
    result_rx: Receiver<LoadedTile>,
    worker: Option<JoinHandle<()>>,
}

impl GpuOutdoorTilePool {
    /// Creates a new outdoor tile pool backed by a dedicated background `.map` parser thread.
    #[must_use]
    pub fn new(data_dir: PathBuf) -> Self {
        let (job_tx, job_rx) = mpsc::channel::<LoadJob>();
        let (result_tx, result_rx) = mpsc::channel::<LoadedTile>();

        let worker = thread::Builder::new()
            .name("sh2-outdoor-tile-loader".to_string())
            .spawn(move || {
                while let Ok(job) = job_rx.recv() {
                    let map = std::fs::read(&job.abs_path)
                        .ok()
                        .and_then(|bytes| parse_map(&bytes, &job.map_name).ok());
                    if result_tx
                        .send(LoadedTile {
                            area: job.area,
                            block_no: job.block_no,
                            map,
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            })
            .ok();

        Self {
            data_dir,
            active_area: None,
            resident: HashMap::new(),
            in_flight: HashSet::new(),
            job_tx: Some(job_tx),
            result_rx,
            worker,
        }
    }

    /// Submits new tile load requests and evicts out-of-range tiles according to `plan`.
    ///
    /// Returns the number of newly queued background load jobs.
    pub fn apply_plan(&mut self, plan: &TileStreamingPlan) -> usize {
        let Some(area) = plan.area else {
            return 0;
        };

        if self.active_area != Some(area) {
            self.active_area = Some(area);
            self.resident.clear();
            self.in_flight.clear();
        }

        for &block_no in &plan.evict_blocks {
            self.resident.remove(&block_no);
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
                let abs_path = self.data_dir.join(&req.map_rel_path);
                let map_name = format!("{}{:02}", area.prefix(), block_no);
                if tx
                    .send(LoadJob {
                        area,
                        block_no,
                        abs_path,
                        map_name,
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

    /// Drains up to `max_uploads_per_frame` completed `.map` tiles from the background worker
    /// and uploads them into GPU buffers on the render thread.
    ///
    /// Returns the list of `block_no` IDs newly uploaded this frame so the caller can notify
    /// [`sh2_core::world::OutdoorTileStreamer::mark_resident`].
    pub fn poll_uploads(
        &mut self,
        device: &wgpu::Device,
        max_uploads_per_frame: usize,
    ) -> Vec<u16> {
        let mut uploaded = Vec::new();
        let limit = max_uploads_per_frame.max(1);

        while uploaded.len() < limit {
            let Ok(done) = self.result_rx.try_recv() else {
                break;
            };
            self.in_flight.remove(&(done.area, done.block_no));
            if self.active_area != Some(done.area) {
                continue;
            }
            if let Some(map) = done.map {
                let gpu_map = GpuMap::from_map_with_room_id(device, &map, done.block_no);
                self.resident.insert(done.block_no, gpu_map);
                uploaded.push(done.block_no);
            }
        }
        uploaded
    }

    /// Applies the active per-frame `stageparts` visibility filter across all resident outdoor tiles.
    pub fn set_visibility_filter(&mut self, filter: Option<&MapPartVisibilityFilter>) {
        for gpu_map in self.resident.values_mut() {
            gpu_map.set_visibility_filter(filter.cloned());
        }
    }

    /// Returns a resident [`GpuMap`] by `block_no`.
    #[must_use]
    pub fn get(&self, block_no: u16) -> Option<&GpuMap> {
        self.resident.get(&block_no)
    }

    /// Iterates over all currently GPU-resident `(block_no, &GpuMap)` outdoor tiles.
    pub fn resident_tiles(&self) -> impl Iterator<Item = (u16, &GpuMap)> {
        self.resident.iter().map(|(&id, map)| (id, map))
    }

    /// Number of currently GPU-resident outdoor tiles.
    #[must_use]
    pub fn resident_count(&self) -> usize {
        self.resident.len()
    }

    /// Number of tiles currently being read/parsed on the background worker thread.
    #[must_use]
    pub fn in_flight_count(&self) -> usize {
        self.in_flight.len()
    }

    /// Total GPU vertex + index buffer bytes currently resident across all loaded outdoor tiles.
    #[must_use]
    pub fn total_gpu_bytes(&self) -> usize {
        self.resident.values().map(|m| m.total_gpu_bytes).sum()
    }
}

impl Drop for GpuOutdoorTilePool {
    fn drop(&mut self) {
        self.job_tx.take();
        if let Some(handle) = self.worker.take() {
            let _ = handle.join();
        }
    }
}
