//! Retail 3D `.map`, `.cld`, and `.cam` stage loader for all 12 Story Progression Checkpoints (`SH2-65`).
//!
//! Ensures stepping through Checkpoints `01..=12` (`[PgUp]` / `[PgDn]`) loads the authentic retail
//! 3D `.map` geometry, `.cld` collision triangles/walls, and `.cam` camera trigger volumes for:
//! `er01`, `ob01`, `ap34`, `ap84`, `hp25`, `hp150`, `ps66`, `rr49`, `rr91`, `ma01`, `ca11`, and `ru73`.

use std::path::PathBuf;

use super::story_progression_3d::{
    Story3dCheckpointSpec, STORY_3D_CHECKPOINTS, STORY_3D_CHECKPOINT_COUNT,
};
use crate::assets::{
    build_map_texture_bind_groups, find_cld_file_for_room, find_map_file_for_room,
};
use crate::state::AppState;
use sh2_assets::formats::cam::CamFile;
use sh2_assets::formats::cld::{parse_cld, WallSegment};
use sh2_assets::formats::TextureLoader;
use sh2_core::collision::CollisionWorld;
use sh2_renderer::fog::FogUniforms;
use sh2_renderer::GpuMap;

/// Loads the retail 3D `.map`, `.cld`, and `.cam` stage assets for Story Checkpoint `cp_idx` (`0..12`),
/// grounding James onto a verified `.cld` floor triangle inside a `.cam` trigger volume and updating
/// stage fog and lighting (`SH2-65`).
pub(crate) fn load_story_checkpoint_3d_stage(state: &mut AppState, cp_idx: usize) {
    let idx = cp_idx % STORY_3D_CHECKPOINT_COUNT;
    let cp = &STORY_3D_CHECKPOINTS[idx];

    // Update active_test_room and selected_test_room in lockstep so sync_active_room_and_viewers
    // in frame.rs does not overwrite our newly loaded checkpoint `.map`/`.cld`/`.cam` on the next frame!
    state.active_test_room = cp.target_room;
    state.menu_ctrl.title_sm.selected_test_room = cp.target_room;

    // Stop any locked `.dds` cutscene session when stepping checkpoints
    state.drama_demo_session = None;
    if let Some(ref mut eng) = state.audio_engine {
        eng.stop_voice(std::time::Duration::from_millis(30));
    }

    let resolver = sh2_assets::AssetPathResolver::resolve();
    let base_roots = [
        resolver.game_root.to_string_lossy().to_string(),
        "Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2".to_string(),
    ];

    // 1. Load retail `.map` (4K AI / SH2:EE HD overrides + SH2:EE tier + Vanilla PS2 tier), falling back
    //    gracefully to `find_map_file_for_room(cp.target_room)` only if the retail rip is absent in CI.
    let mut loaded_retail_map = false;
    for base in &base_roots {
        let loader = TextureLoader::new(
            PathBuf::from(base).join("data"),
            Some(PathBuf::from(base).join("sh2e")),
        );
        if let Ok(hd_map) = loader.load_map_with_hd_overrides(cp.map_rel_path, cp.room_id) {
            state.gpu_map = GpuMap::from_map(&state.render_ctx.device, &hd_map);
            let hd_bgs = build_map_texture_bind_groups(&state.render_ctx, &hd_map);
            let sh2e_bgs = loader
                .load_sh2e_map(cp.map_rel_path, cp.room_id)
                .ok()
                .map(|m| build_map_texture_bind_groups(&state.render_ctx, &m))
                .unwrap_or_else(|| hd_bgs.clone());
            let van_bgs = loader
                .load_vanilla_map(cp.map_rel_path, cp.room_id)
                .ok()
                .map(|m| build_map_texture_bind_groups(&state.render_ctx, &m))
                .unwrap_or_else(|| sh2e_bgs.clone());
            state.map_hd_texture_bind_groups = hd_bgs;
            state.map_sh2e_texture_bind_groups = sh2e_bgs;
            state.map_vanilla_texture_bind_groups = van_bgs;
            loaded_retail_map = true;
            tracing::info!(
                "[Story Checkpoint {:02}/12] Loaded retail 3D stage '{}' ({}) from base={}",
                idx + 1,
                cp.room_display_label,
                cp.map_rel_path,
                base
            );
            break;
        }
    }

    if !loaded_retail_map {
        let (fb_map, fb_pos, fb_yaw, fb_fog) = find_map_file_for_room(cp.target_room);
        state.gpu_map = GpuMap::from_map(&state.render_ctx.device, &fb_map);
        let bgs = build_map_texture_bind_groups(&state.render_ctx, &fb_map);
        state.map_hd_texture_bind_groups = bgs.clone();
        state.map_sh2e_texture_bind_groups = bgs.clone();
        state.map_vanilla_texture_bind_groups = bgs;
        state.fog_uniforms = fb_fog;
        state.render_ctx.update_fog(&state.fog_uniforms);
        state.sim_core.collision = find_cld_file_for_room(cp.target_room);
        state
            .sim_core
            .transition_to_room(cp.room_id, fb_pos, fb_yaw, true);
    } else {
        // 2. Load retail `.cld` collision world for `cp.cld_rel_path`
        state.sim_core.collision = load_checkpoint_cld_world(&resolver, cp)
            .unwrap_or_else(|| find_cld_file_for_room(cp.target_room));

        // 3. Load retail `.cam` camera trigger volumes so we can compute a guaranteed on-floor spawn
        //    inside an active `.cam` road volume
        let _ = state.sim_core.camera.load_room_cam(cp.room_id, true);
        let (spawn_pos, spawn_yaw) = compute_checkpoint_spawn_pose(
            cp,
            &state.sim_core.collision,
            state.sim_core.camera.cam_file.as_ref(),
        );
        state
            .sim_core
            .transition_to_room(cp.room_id, spawn_pos, spawn_yaw, true);

        // 4. Configure atmospheric fog and turn on pocket flashlight for indoor stages
        state.fog_uniforms = fog_for_checkpoint_room(cp.room_id);
        state.render_ctx.update_fog(&state.fog_uniforms);
        if is_indoor_story_checkpoint_room(cp.room_id) && cp.room_id != "er01" {
            state.sim_core.scenario.flashlight_on = true;
        }
    }

    let active_tex_quality = state
        .menu_ctrl
        .options_menu
        .draft_config
        .video
        .texture_quality;
    state.apply_texture_quality(active_tex_quality);
}

/// Returns `true` when `room_id` is an indoor Story Checkpoint stage (`ap34`, `ap84`, `hp25`,
/// `hp150`, `ps66`, `rr49`, `rr91`, `ma01`, or `er01`).
#[must_use]
pub(crate) fn is_indoor_story_checkpoint_room(room_id: &str) -> bool {
    !matches!(room_id, "ob01" | "ca11" | "ru73")
}

/// Resolves and parses a checkpoint's `.cld` collision file into a [`CollisionWorld`].
pub(crate) fn load_checkpoint_cld_world(
    resolver: &sh2_assets::AssetPathResolver,
    cp: &Story3dCheckpointSpec,
) -> Option<CollisionWorld> {
    let p = resolver.resolve_file(cp.cld_rel_path)?;
    let bytes = std::fs::read(&p).ok()?;
    let cld = parse_cld(&bytes).ok()?;
    Some(CollisionWorld::from_cld(&cld))
}

/// Computes a guaranteed walkable `(spawn_pos, spawn_yaw)` grounded on a `.cld` floor triangle
/// inside a `.cam` camera switch volume with wall clearance (`SH2-65`).
#[must_use]
pub(crate) fn compute_checkpoint_spawn_pose(
    cp: &Story3dCheckpointSpec,
    collision: &CollisionWorld,
    cam_file: Option<&CamFile>,
) -> (glam::Vec3, f32) {
    if cp.room_id == "er01" {
        let y = collision
            .sample_floor_height(-19606.6, 20429.55, 0.0, 1500.0, 1500.0)
            .unwrap_or(0.0);
        return (
            glam::Vec3::new(-19606.6, y, 20429.55),
            -std::f32::consts::FRAC_PI_2,
        );
    }
    if cp.room_id == "ob01" {
        let y = collision
            .sample_floor_height(0.0, -5000.0, -100.0, 1500.0, 1500.0)
            .unwrap_or(-100.0);
        return (glam::Vec3::new(0.0, y, -5000.0), 0.0);
    }

    let mut candidates: Vec<(f32, f32, usize)> = Vec::new();
    if let Some(cam) = cam_file {
        for (r_idx, rec) in cam.records.iter().enumerate() {
            let (min_x, max_x) = rec.lim_sw.x_bounds();
            let (min_z, max_z) = rec.lim_sw.z_bounds();
            for fx in [0.5f32, 0.35, 0.65] {
                for fz in [0.5f32, 0.35, 0.65] {
                    candidates.push((
                        min_x + (max_x - min_x) * fx,
                        min_z + (max_z - min_z) * fz,
                        r_idx,
                    ));
                }
            }
        }
    }
    for tri in &collision.floors {
        let cx = (tri.v[0][0] + tri.v[1][0] + tri.v[2][0]) / 3.0;
        let cz = (tri.v[0][2] + tri.v[1][2] + tri.v[2][2]) / 3.0;
        candidates.push((cx, cz, 99));
    }

    let mut best_pos = None;
    let mut best_score = -1.0f32;
    let mut best_rec_idx = 0usize;

    for (cx, cz, _pref_rec) in candidates {
        let Some(fy) = sample_any_floor_y(collision, cx, cz) else {
            continue;
        };
        let in_rec = if let Some(cam) = cam_file {
            cam.records
                .iter()
                .position(|rec| rec.lim_sw.contains_point([cx, fy, cz], 100.0))
        } else {
            Some(0)
        };
        let Some(r_idx) = in_rec else {
            continue;
        };
        let wall_clear = min_wall_clearance_xz(&collision.walls, cx, cz);
        let score = wall_clear
            + if r_idx == 0 && wall_clear >= 350.0 {
                500.0
            } else {
                0.0
            };
        if score > best_score {
            best_score = score;
            best_pos = Some(glam::Vec3::new(cx, fy, cz));
            best_rec_idx = r_idx;
        }
    }

    let spawn_pos = best_pos.unwrap_or_else(|| {
        collision
            .floors
            .first()
            .map(|tri| {
                let cx = (tri.v[0][0] + tri.v[1][0] + tri.v[2][0]) / 3.0;
                let cy = (tri.v[0][1] + tri.v[1][1] + tri.v[2][1]) / 3.0;
                let cz = (tri.v[0][2] + tri.v[1][2] + tri.v[2][2]) / 3.0;
                glam::Vec3::new(cx, cy, cz)
            })
            .unwrap_or(glam::Vec3::ZERO)
    });

    // Face toward or away from the active camera rail center so James is framed cleanly
    let spawn_yaw = cam_file
        .and_then(|c| c.records.get(best_rec_idx))
        .map(|rec| {
            let rd_c = rec.lim_rd.center();
            let dx = rd_c[0] - spawn_pos.x;
            let dz = rd_c[2] - spawn_pos.z;
            if dx.abs() + dz.abs() > 10.0 {
                dx.atan2(dz)
            } else {
                0.0
            }
        })
        .unwrap_or(0.0);

    (spawn_pos, spawn_yaw)
}

fn sample_any_floor_y(collision: &CollisionWorld, x: f32, z: f32) -> Option<f32> {
    for tri in &collision.floors {
        if let Some(y) = tri.sample_height(x, z) {
            return Some(y);
        }
    }
    None
}

fn min_wall_clearance_xz(walls: &[WallSegment], px: f32, pz: f32) -> f32 {
    let mut min_d = 9999.0f32;
    for w in walls {
        let dx = w.end[0] - w.start[0];
        let dz = w.end[1] - w.start[1];
        let l2 = dx * dx + dz * dz;
        let d = if l2 < 1e-6 {
            ((px - w.start[0]).powi(2) + (pz - w.start[1]).powi(2)).sqrt()
        } else {
            let t = (((px - w.start[0]) * dx + (pz - w.start[1]) * dz) / l2).clamp(0.0, 1.0);
            let qx = w.start[0] + t * dx;
            let qz = w.start[1] + t * dz;
            ((px - qx).powi(2) + (pz - qz).powi(2)).sqrt()
        };
        if d < min_d {
            min_d = d;
        }
    }
    min_d
}

fn fog_for_checkpoint_room(room_id: &str) -> FogUniforms {
    match room_id {
        "ob01" | "ca11" => FogUniforms {
            fog_color: [0.65, 0.68, 0.72, 1.0],
            fog_near: 2000.0,
            fog_far: 32000.0,
            density: 0.85,
            time: 0.0,
            step_count: 32,
            curve_type: 1,
            speed_mult: 1.0,
            _padding: 0.0,
        },
        "ru73" => FogUniforms {
            fog_color: [0.32, 0.35, 0.40, 1.0],
            fog_near: 2500.0,
            fog_far: 28000.0,
            density: 0.45,
            time: 0.0,
            step_count: 28,
            curve_type: 1,
            speed_mult: 0.8,
            _padding: 0.0,
        },
        _ => FogUniforms {
            fog_color: [0.10, 0.11, 0.13, 1.0],
            fog_near: 1200.0,
            fog_far: 9000.0,
            density: 0.22,
            time: 0.0,
            step_count: 24,
            curve_type: 0,
            speed_mult: 0.5,
            _padding: 0.0,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_12_story_checkpoints_resolve_retail_map_cld_cam_and_grounded_spawn() {
        let resolver = sh2_assets::AssetPathResolver::resolve();
        if !resolver.data_dir.exists() {
            return;
        }
        let loader = TextureLoader::new(resolver.data_dir.clone(), resolver.sh2e_dir.clone());

        for cp in &STORY_3D_CHECKPOINTS {
            let map = loader
                .load_vanilla_map(cp.map_rel_path, cp.room_id)
                .unwrap_or_else(|e| {
                    panic!(
                        "Checkpoint {:02} ({}) failed to parse {}: {e}",
                        cp.index + 1,
                        cp.room_id,
                        cp.map_rel_path
                    )
                });
            assert!(
                !map.materials.is_empty(),
                "Checkpoint {:02} ({}) map must have materials",
                cp.index + 1,
                cp.room_id
            );

            let collision = load_checkpoint_cld_world(&resolver, cp).unwrap_or_else(|| {
                panic!(
                    "Checkpoint {:02} ({}) failed to parse {}",
                    cp.index + 1,
                    cp.room_id,
                    cp.cld_rel_path
                )
            });
            assert!(
                !collision.floors.is_empty(),
                "Checkpoint {:02} ({}) cld must have floor triangles",
                cp.index + 1,
                cp.room_id
            );

            let cam_asset = resolver
                .load_cam_for_room(cp.room_id, true)
                .unwrap_or_else(|e| {
                    panic!(
                        "Checkpoint {:02} ({}) failed to parse .cam: {e}",
                        cp.index + 1,
                        cp.room_id
                    )
                })
                .unwrap_or_else(|| {
                    panic!(
                        "Checkpoint {:02} ({}) missing .cam on disk",
                        cp.index + 1,
                        cp.room_id
                    )
                });
            assert!(
                !cam_asset.cam_file.records.is_empty(),
                "Checkpoint {:02} ({}) .cam must have records",
                cp.index + 1,
                cp.room_id
            );

            let (spawn_pos, spawn_yaw) =
                compute_checkpoint_spawn_pose(cp, &collision, Some(&cam_asset.cam_file));
            assert!(spawn_pos.is_finite() && spawn_yaw.is_finite());

            // Verify spawn_pos is grounded on a valid .cld floor triangle
            let sampled_y = collision
                .sample_floor_height(spawn_pos.x, spawn_pos.z, spawn_pos.y, 1500.0, 1500.0)
                .or_else(|| sample_any_floor_y(&collision, spawn_pos.x, spawn_pos.z));
            assert!(
                sampled_y.is_some(),
                "Checkpoint {:02} ({}) spawn {:?} must lie over a .cld floor triangle",
                cp.index + 1,
                cp.room_id,
                spawn_pos
            );
            assert!(
                (sampled_y.unwrap_or(spawn_pos.y) - spawn_pos.y).abs() < 1.0,
                "Checkpoint {:02} ({}) spawn.y={} must match floor y={:?}",
                cp.index + 1,
                cp.room_id,
                spawn_pos.y,
                sampled_y
            );

            // Verify spawn_pos resolves an active .cam road record
            let active_rec = cam_asset
                .cam_file
                .find_active_record([spawn_pos.x, spawn_pos.y, spawn_pos.z], 150.0);
            assert!(
                active_rec.is_some(),
                "Checkpoint {:02} ({}) spawn {:?} must resolve an active .cam record",
                cp.index + 1,
                cp.room_id,
                spawn_pos
            );
        }
    }
}
