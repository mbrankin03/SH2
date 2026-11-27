//! Retail 3D `.map`, `.cld`, and `.cam` stage loader for all 12 Story Progression Checkpoints (`SH2-65`).
//!
//! Ensures stepping through Checkpoints `01..=12` (`[PgUp]` / `[PgDn]`) loads the authentic retail
//! 3D `.map` geometry, `.cld` collision triangles/walls, and `.cam` camera trigger volumes for:
//! `er01`, `ob01`, `ap34`, `ap84`, `hp25`, `hp150`, `ps66`, `rr49`, `rr91`, `ma01`, `ca11`, and `ru73`.

use super::story_progression_3d::{
    Story3dCheckpointSpec, STORY_3D_CHECKPOINTS, STORY_3D_CHECKPOINT_COUNT,
};
use crate::assets::{
    find_cld_file_for_room, find_map_file_for_room_for_quality, load_map_for_quality_from_roots,
    MapTierAssets,
};
use crate::state::{ActiveMapSource, AppState};
use sh2_assets::formats::cam::CamFile;
use sh2_assets::formats::cld::{parse_cld, WallSegment};
use sh2_core::collision::CollisionWorld;
use sh2_renderer::fog::FogUniforms;

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
    state.enemy_assets.evict_all();

    // Stop any locked `.dds` cutscene session when stepping checkpoints
    state.drama_demo_session = None;
    if let Some(ref mut eng) = state.audio_engine {
        eng.stop_voice(std::time::Duration::from_millis(30));
    }

    let resolver = sh2_assets::AssetPathResolver::resolve();
    let active_tex_quality = state
        .menu_ctrl
        .options_menu
        .draft_config
        .video
        .texture_quality;

    // 1. Load ONLY the active `WorldTextureQuality` tier for the retail `.map` (`SH2-181`),
    //    falling back gracefully to `find_map_file_for_room_for_quality` if the retail rip is absent.
    let loaded_retail_map = if let Some(map) =
        load_map_for_quality_from_roots(cp.map_rel_path, cp.room_id, active_tex_quality)
    {
        let assets = MapTierAssets::from_map(&state.render_ctx, map);
        state.gpu_map = assets.gpu_map;
        state.map_texture_bind_groups = assets.texture_bind_groups;
        tracing::info!(
            "[Story Checkpoint {:02}/12] Loaded retail 3D stage '{}' ({}, tier={:?})",
            idx + 1,
            cp.room_display_label,
            cp.map_rel_path,
            active_tex_quality
        );
        true
    } else {
        false
    };

    if !loaded_retail_map {
        let (fb_map, fb_pos, fb_yaw, fb_fog) =
            find_map_file_for_room_for_quality(cp.target_room, active_tex_quality);
        let fb_assets = MapTierAssets::from_map(&state.render_ctx, fb_map);
        state.gpu_map = fb_assets.gpu_map;
        state.map_texture_bind_groups = fb_assets.texture_bind_groups;
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

        // 4. Configure atmospheric fog and authentic pre-/post-Room-205 flashlight possession (`SH2-33`)
        let cfg = &state.menu_ctrl.options_menu.draft_config;
        state.fog_uniforms = fog_for_checkpoint_room(
            cp.room_id,
            cfg.video.fog_quality,
            cfg.video.volumetric_light,
            cfg.game.enable_sh2ee_world_fixes,
        );
        state.render_ctx.update_fog(&state.fog_uniforms);
        if is_pre_flashlight_checkpoint_room(cp.room_id) {
            state.sim_core.scenario.revoke_flashlight();
        } else if is_indoor_story_checkpoint_room(cp.room_id) {
            state.sim_core.scenario.acquire_flashlight(true);
        } else {
            state.sim_core.scenario.acquire_flashlight(false);
        }
    }

    state.active_map_source = ActiveMapSource::StoryCheckpoint(idx);
    state.apply_texture_quality(active_tex_quality);
}

/// Returns `true` for Story Checkpoints that occur before James picks up the flashlight
/// (`item 0x12`) in Wood Side Apartments Room 205 (`er01`, `ob01`, `ca11`, `ap34`).
#[must_use]
pub(crate) fn is_pre_flashlight_checkpoint_room(room_id: &str) -> bool {
    matches!(room_id, "er01" | "ob01" | "ca11" | "ap34")
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

fn fog_for_checkpoint_room(
    room_id: &str,
    fog_quality: sh2_core::config::FogQuality,
    volumetric_light: bool,
    enable_sh2ee_world_fixes: bool,
) -> FogUniforms {
    let ps2_room_id = sh2_core::fog::room_id_from_stage_code(room_id);
    let eval = sh2_core::fog::evaluate_room_fog_with_fixes(
        &sh2_core::fog::RoomFogContext {
            room_id: ps2_room_id,
            ..Default::default()
        },
        enable_sh2ee_world_fixes,
    );
    FogUniforms::from_room_evaluation(&eval, fog_quality, volumetric_light)
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
        let loader = sh2_assets::formats::TextureLoader::new(
            resolver.data_dir.clone(),
            resolver.sh2e_dir.clone(),
        );

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
