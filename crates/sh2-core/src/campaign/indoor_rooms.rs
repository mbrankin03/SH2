//! Data-driven indoor room resolver mapping `(stage_id, [x, y, z])` to the exact retail
//! indoor room identifier (`er01`/`er05`/`er09`/`er13`, `ap01..ap108`, `hp01..hp180`,
//! `ps01..ps195`, `rr01..rr95`, `ru01..ru73`, `ma01..ma85`, etc.).
//!
//! Each indoor room's `.cld` file contains its exact world-space 3D floor triangles (`fldofs`,
//! `SLUS_202.28` `_CL_CLDHEADER`) and shares its canonical room ID with its `.map` and `.cam`
//! counterparts (`loadBgAll_SetLoadRequestIndoor2x2` `0x00271420`). Area footprints are parsed
//! once per process and cached by area directory.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use sh2_assets::formats::cld::{parse_cld, FloorTriangle};
use sh2_assets::AssetPathResolver;

use crate::room_loader::stage_room_descriptor;
use crate::world::outdoor_grid::OutdoorArea;

use super::stage_loader::room_area_dir;

/// Cached 2D XZ bounding box and 3D floor triangles for one retail indoor room `.cld`.
struct IndoorRoomFootprint {
    room_id: String,
    min_x: f32,
    max_x: f32,
    min_z: f32,
    max_z: f32,
    floors: Vec<FloorTriangle>,
}

static INDOOR_AREA_CACHE: OnceLock<Mutex<HashMap<String, Vec<IndoorRoomFootprint>>>> =
    OnceLock::new();

fn load_area_footprints(area_dir: &str) -> Vec<IndoorRoomFootprint> {
    let resolver = AssetPathResolver::resolve();
    let dir_path = resolver.game_root.join("data").join(area_dir);
    let Ok(entries) = std::fs::read_dir(&dir_path) else {
        return Vec::new();
    };

    let mut footprints = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("cld") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let room_id = stem.to_ascii_lowercase();
        // Ensure a matching `.map` exists (skips orphan `.cld` files like `hp26.cld`).
        if !dir_path.join(format!("{room_id}.map")).is_file() {
            continue;
        }
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        let Ok(cld) = parse_cld(&bytes) else {
            continue;
        };
        if cld.floors.is_empty() {
            continue;
        }
        let mut min_x = f32::INFINITY;
        let mut max_x = f32::NEG_INFINITY;
        let mut min_z = f32::INFINITY;
        let mut max_z = f32::NEG_INFINITY;
        for tri in &cld.floors {
            for v in &tri.v {
                min_x = min_x.min(v[0]);
                max_x = max_x.max(v[0]);
                min_z = min_z.min(v[2]);
                max_z = max_z.max(v[2]);
            }
        }
        footprints.push(IndoorRoomFootprint {
            room_id,
            min_x,
            max_x,
            min_z,
            max_z,
            floors: cld.floors,
        });
    }
    // Deterministic ordering by room_id for reproducible tie-breaking.
    footprints.sort_by(|a, b| a.room_id.cmp(&b.room_id));
    footprints
}

/// Resolves the retail indoor room identifier (e.g. `"er05"`, `"ap34"`, `"hp101"`, `"rr49"`)
/// whose `.cld` floor triangles cover `pos = [x, y, z]` (PS2 world millimeters) for indoor
/// `stage_id` (`1..=58`).
///
/// Returns `None` for outdoor stages (`OutdoorArea::from_stage_id(stage_id).is_some()`),
/// stages without `.cld` floor files (e.g. Stage 38 `qt01` Toluca Lake Boat), or when retail
/// assets are unavailable.
#[must_use]
pub fn resolve_indoor_room_id(stage_id: u16, pos: [f32; 3]) -> Option<String> {
    if OutdoorArea::from_stage_id(stage_id).is_some() {
        return None;
    }
    let desc = stage_room_descriptor(stage_id)?;
    let area_dir = room_area_dir(desc.default_room_id);

    let cache = INDOOR_AREA_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let mut guard = cache.lock().ok()?;
    let footprints = guard
        .entry(area_dir.clone())
        .or_insert_with(|| load_area_footprints(&area_dir));

    let [px, py, pz] = pos;
    let mut best_match: Option<(&str, f32)> = None;

    for fp in footprints.iter() {
        if px < fp.min_x || px > fp.max_x || pz < fp.min_z || pz > fp.max_z {
            continue;
        }
        for tri in &fp.floors {
            if let Some(floor_y) = tri.sample_height(px, pz) {
                let dy = (floor_y - py).abs();
                let is_better = match best_match {
                    None => true,
                    Some((_, best_dy)) => dy < best_dy,
                };
                if is_better {
                    best_match = Some((&fp.room_id, dy));
                }
            }
        }
    }

    best_match.map(|(id, _)| id.to_string())
}
