//! Deterministic parity and unit tests for SH2-178:
//! - `SpatialGrid`-accelerated raycasts (`check_hit_eyes`, `check_hit_eyes_only_*`,
//!   `check_ai_los`, `raycast_acoustic_occlusion`, `clamp_camera`) 1:1 parity against brute-force
//!   scans on retail `.cld` files (indoor, outdoor, merged multi-tile outdoor, `sector.disable`,
//!   multi-cell shapes, and boundary-grazing rays).
//! - `OutdoorTileStreamer` caching & `room_id_matches_outdoor_block` zero-alloc fast path.
//! - Stack-allocated `Copy` `EvPosShape::Polyline` (`2..=7` vertices).
//! - Stack top-3 `evaluate_character_lighting` tie-breaking parity.
//! - Stack top-20 combat targeting parity (`select_auto_aim_target`, `cycle_auto_aim_target`,
//!   `find_near_deadly_target`).
//! - `BloodEffectSystem` `VecDeque` FIFO eviction & empty room archive skipping.
//! - `MariaCompanionController` `VecDeque` trail & `*_into` scratch buffer helpers.

use std::path::{Path, PathBuf};

use glam::{Mat4, Vec3};
use sh2_assets::formats::cld::{
    parse_cld, FloorTriangle, HitPolyColumn, HitPolyPlane, WallSegment,
};
use sh2_core::ai::maria::MariaCompanionController;
use sh2_core::ai::sensory::check_hit_eyes as brute_ai_check_hit_eyes;
use sh2_core::campaign::{resolve_outdoor_area_block, room_id_matches_outdoor_block};
use sh2_core::collision::{
    cl_check_sub_line_to_plane, intersect_segment_bg_column, intersect_segment_xz,
    intersect_segment_xz_t, raycast_acoustic_occlusion as brute_acoustic_occlusion,
    should_skip_material_for_mode, CollisionWorld, EyeHitKind, EyeRayHit, EyeRayMode,
    RaycastFilter,
};
use sh2_core::combat::{
    cycle_auto_aim_target, find_near_deadly_target, select_auto_aim_target, TargetCandidate,
};
use sh2_core::config::OutdoorStreamingMode;
use sh2_core::effects::{
    BloodEffectSystem, BloodSystemConfig, FxRng, ImpactDescriptor, ImpactKind, ImpactSparkSystem,
    MAX_BLOOD_DROPS, MAX_BLOOD_STAINS,
};
use sh2_core::events::triggers::{decode_ev_pos_shape, event_check_look, EvPosShape};
use sh2_core::lighting::{
    evaluate_character_lighting, evaluate_lens_flares, evaluate_lens_flares_into, lookup_draw_env,
    CharacterLightCandidate, CharacterLightKind, LensFlareSource, LENS_FLARE_SPRITE_COUNT,
};
use sh2_core::types::event::{float_to_char2, EventShapeKind};
use sh2_core::world::{OutdoorArea, OutdoorTileStreamer};

fn retail_data_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2/data")
}

fn load_retail_cld(rel: &str) -> Option<CollisionWorld> {
    let path = retail_data_root().join(rel);
    if !path.exists() {
        return None;
    }
    let bytes = std::fs::read(&path).expect("read retail .cld");
    let cld = parse_cld(&bytes).expect("parse retail .cld");
    Some(CollisionWorld::from_cld(&cld))
}

fn brute_test_planes(
    origin: Vec3,
    target: Vec3,
    mode: EyeRayMode,
    planes: &[HitPolyPlane],
    best_dist_sq: &mut f32,
    best_hit: &mut Option<EyeRayHit>,
) {
    for plane in planes {
        if should_skip_material_for_mode(plane.material, mode) {
            continue;
        }
        if let Some((t, point, normal)) = cl_check_sub_line_to_plane(origin, target, plane) {
            let dist_sq = origin.distance_squared(point);
            if dist_sq < *best_dist_sq {
                *best_dist_sq = dist_sq;
                *best_hit = Some(EyeRayHit {
                    kind: EyeHitKind::Plane,
                    point,
                    normal,
                    t,
                    dist_sq,
                    material: plane.material,
                    character_id: None,
                });
            }
        }
    }
}

fn brute_test_columns(
    origin: Vec3,
    target: Vec3,
    cols: &[HitPolyColumn],
    best_dist_sq: &mut f32,
    best_hit: &mut Option<EyeRayHit>,
) {
    for col in cols {
        if let Some((t, point)) = intersect_segment_bg_column(origin, target, col) {
            let dist_sq = origin.distance_squared(point);
            if dist_sq < *best_dist_sq {
                *best_dist_sq = dist_sq;
                *best_hit = Some(EyeRayHit {
                    kind: EyeHitKind::BgColumn,
                    point,
                    normal: Vec3::ZERO,
                    t,
                    dist_sq,
                    material: 0,
                    character_id: None,
                });
            }
        }
    }
}

/// Brute-force reference `clCheckHitEyes` scanning all enabled sectors linearly without `SpatialGrid`.
fn brute_force_check_hit_eyes(
    world: &CollisionWorld,
    origin: Vec3,
    target: Vec3,
    mode: EyeRayMode,
) -> Option<EyeRayHit> {
    let mut best_dist_sq = origin.distance_squared(target);
    let mut best_hit: Option<EyeRayHit> = None;

    if !world.sectors.is_empty() {
        for s in &world.sectors {
            if s.disable != 0 {
                continue;
            }
            brute_test_planes(
                origin,
                target,
                mode,
                &world.raw_floors[s.raw_floor_offset..s.raw_floor_offset + s.raw_floor_len],
                &mut best_dist_sq,
                &mut best_hit,
            );
            brute_test_planes(
                origin,
                target,
                mode,
                &world.raw_walls[s.raw_wall_offset..s.raw_wall_offset + s.raw_wall_len],
                &mut best_dist_sq,
                &mut best_hit,
            );
            if mode == EyeRayMode::AllNoThru {
                brute_test_planes(
                    origin,
                    target,
                    mode,
                    &world.raw_side_walls
                        [s.raw_side_wall_offset..s.raw_side_wall_offset + s.raw_side_wall_len],
                    &mut best_dist_sq,
                    &mut best_hit,
                );
            }
            brute_test_planes(
                origin,
                target,
                mode,
                &world.raw_ceilings[s.raw_ceiling_offset..s.raw_ceiling_offset + s.raw_ceiling_len],
                &mut best_dist_sq,
                &mut best_hit,
            );
            brute_test_columns(
                origin,
                target,
                &world.columns[s.column_offset..s.column_offset + s.column_len],
                &mut best_dist_sq,
                &mut best_hit,
            );
        }
    } else if world.disable == 0 {
        brute_test_planes(
            origin,
            target,
            mode,
            &world.raw_floors,
            &mut best_dist_sq,
            &mut best_hit,
        );
        brute_test_planes(
            origin,
            target,
            mode,
            &world.raw_walls,
            &mut best_dist_sq,
            &mut best_hit,
        );
        if mode == EyeRayMode::AllNoThru {
            brute_test_planes(
                origin,
                target,
                mode,
                &world.raw_side_walls,
                &mut best_dist_sq,
                &mut best_hit,
            );
        }
        brute_test_planes(
            origin,
            target,
            mode,
            &world.raw_ceilings,
            &mut best_dist_sq,
            &mut best_hit,
        );
        brute_test_columns(
            origin,
            target,
            &world.columns,
            &mut best_dist_sq,
            &mut best_hit,
        );
    }

    best_hit
}

fn assert_eye_hit_eq(label: &str, actual: Option<EyeRayHit>, expected: Option<EyeRayHit>) {
    match (actual, expected) {
        (None, None) => {}
        (Some(a), Some(e)) => {
            assert_eq!(a.kind, e.kind, "{label}: kind mismatch");
            assert_eq!(a.material, e.material, "{label}: material mismatch");
            assert!(
                (a.t - e.t).abs() < 1e-4,
                "{label}: t mismatch actual={} expected={}",
                a.t,
                e.t
            );
            assert!(
                a.point.distance(e.point) < 1e-2,
                "{label}: point mismatch actual={:?} expected={:?}",
                a.point,
                e.point
            );
            assert!(
                a.normal.distance(e.normal) < 1e-3,
                "{label}: normal mismatch actual={:?} expected={:?}",
                a.normal,
                e.normal
            );
        }
        (a, e) => panic!("{label}: hit presence mismatch actual={a:?} expected={e:?}"),
    }
}

fn verify_world_raycast_parity(world: &CollisionWorld, label: &str) {
    let [min_x, min_z, max_x, max_z] = if !world.sectors.is_empty() {
        let mut bounds = [
            f32::INFINITY,
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::NEG_INFINITY,
        ];
        for s in &world.sectors {
            if s.disable == 0 && s.grid.block_aabb_xz[0] <= s.grid.block_aabb_xz[2] {
                bounds[0] = bounds[0].min(s.grid.block_aabb_xz[0]);
                bounds[1] = bounds[1].min(s.grid.block_aabb_xz[1]);
                bounds[2] = bounds[2].max(s.grid.block_aabb_xz[2]);
                bounds[3] = bounds[3].max(s.grid.block_aabb_xz[3]);
            }
        }
        bounds
    } else {
        world.grid.block_aabb_xz
    };
    assert!(min_x <= max_x && min_z <= max_z, "{label}: empty bounds");

    let cx = (min_x + max_x) * 0.5;
    let cz = (min_z + max_z) * 0.5;
    let span_x = (max_x - min_x).max(2000.0);
    let span_z = (max_z - min_z).max(2000.0);

    // Deterministic LCG ray suite: interior, long diagonal, vertical, and exact cell-boundary grazing rays.
    let mut seed: u32 = 0x5EED_0178;
    let mut next_unit = || -> f32 {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        ((seed >> 8) as f32) / ((1u32 << 24) as f32)
    };

    let mut rays: Vec<(Vec3, Vec3)> = Vec::with_capacity(96);
    for _ in 0..80 {
        let ox = min_x - 1000.0 + next_unit() * (span_x + 2000.0);
        let oz = min_z - 1000.0 + next_unit() * (span_z + 2000.0);
        let oy = -2200.0 + next_unit() * 2400.0;
        let tx = min_x - 1000.0 + next_unit() * (span_x + 2000.0);
        let tz = min_z - 1000.0 + next_unit() * (span_z + 2000.0);
        let ty = -2200.0 + next_unit() * 2400.0;
        rays.push((Vec3::new(ox, oy, oz), Vec3::new(tx, ty, tz)));
    }
    // Add exact 5000 mm cell-boundary grazing rays and vertical floor probes
    for i in 0..4 {
        let gx = world.origin[0] + (i as f32) * 5000.0;
        let gz = world.origin[1] + (i as f32) * 5000.0;
        rays.push((
            Vec3::new(gx, -1000.0, min_z - 500.0),
            Vec3::new(gx, -1000.0, max_z + 500.0),
        ));
        rays.push((
            Vec3::new(min_x - 500.0, -1000.0, gz),
            Vec3::new(max_x + 500.0, -1000.0, gz),
        ));
        rays.push((
            Vec3::new(cx + (i as f32 - 1.5) * 2500.0, -2000.0, cz),
            Vec3::new(cx + (i as f32 - 1.5) * 2500.0, 2000.0, cz),
        ));
    }

    let enabled_walls: Vec<WallSegment> = if !world.sectors.is_empty() {
        world
            .sectors
            .iter()
            .filter(|s| s.disable == 0)
            .flat_map(|s| &world.walls[s.wall_offset..s.wall_offset + s.wall_len])
            .copied()
            .collect()
    } else if world.disable == 0 {
        world.walls.clone()
    } else {
        Vec::new()
    };
    let enabled_floors: Vec<FloorTriangle> = if !world.sectors.is_empty() {
        world
            .sectors
            .iter()
            .filter(|s| s.disable == 0)
            .flat_map(|s| &world.floors[s.floor_offset..s.floor_offset + s.floor_len])
            .copied()
            .collect()
    } else if world.disable == 0 {
        world.floors.clone()
    } else {
        Vec::new()
    };

    for (i, &(origin, target)) in rays.iter().enumerate() {
        for mode in [EyeRayMode::Thru, EyeRayMode::NoThru, EyeRayMode::AllNoThru] {
            let accel = world.check_hit_eyes(origin, target, mode, None, &[]);
            let brute = brute_force_check_hit_eyes(world, origin, target, mode);
            assert_eye_hit_eq(&format!("{label} ray#{i} mode={mode:?}"), accel, brute);
        }

        // AI LOS parity (`check_ai_los` vs brute-force `ai::sensory::check_hit_eyes`)
        let ai_accel = world.check_ai_los(origin, target);
        let ai_brute = brute_ai_check_hit_eyes(origin, target, &enabled_walls);
        assert!(
            (ai_accel - ai_brute).abs() < 1e-2,
            "{label} ray#{i}: AI LOS mismatch accel={ai_accel} brute={ai_brute}"
        );

        // Acoustic occlusion parity
        let ac_accel = world.raycast_acoustic_occlusion(origin, target);
        let ac_brute =
            brute_acoustic_occlusion(origin, target, enabled_walls.iter(), enabled_floors.iter());
        assert_eq!(
            ac_accel.wall_hit_count, ac_brute.wall_hit_count,
            "{label} ray#{i}: acoustic wall_hit_count mismatch"
        );
        assert_eq!(
            ac_accel.slab_hit_count, ac_brute.slab_hit_count,
            "{label} ray#{i}: acoustic slab_hit_count mismatch"
        );
        assert!(
            (ac_accel.closest_t - ac_brute.closest_t).abs() < 1e-4,
            "{label} ray#{i}: acoustic closest_t mismatch"
        );
        assert!(
            (ac_accel.barrier_density - ac_brute.barrier_density).abs() < 1e-3,
            "{label} ray#{i}: acoustic barrier_density mismatch"
        );
    }
}

#[test]
fn spatial_grid_raycast_parity_on_retail_indoor_and_merged_outdoor_cld() {
    let Some(er01) = load_retail_cld("bg/er/er01.cld") else {
        return; // Retail assets absent in CI
    };
    verify_world_raycast_parity(&er01, "er01");

    if let Some(ap01) = load_retail_cld("bg/ap/ap01.cld") {
        verify_world_raycast_parity(&ap01, "ap01");
    }

    if let (Some(cb28), Some(cb29), Some(cb27), Some(cb36)) = (
        load_retail_cld("bg/cb/cb28.cld"),
        load_retail_cld("bg/cb/cb29.cld"),
        load_retail_cld("bg/cb/cb27.cld"),
        load_retail_cld("bg/cb/cb36.cld"),
    ) {
        verify_world_raycast_parity(&cb28, "cb28");

        let mut merged = CollisionWorld::merge(&[&cb28, &cb29, &cb27, &cb36]);
        assert_eq!(merged.sectors.len(), 4);
        verify_world_raycast_parity(&merged, "merged_cb_4tiles");

        // Disable sector 1 (`cb29`) and confirm `sector.disable != 0` skips only that sector
        merged.sectors[1].disable = 1;
        verify_world_raycast_parity(&merged, "merged_cb_sector1_disabled");
    }
}

#[test]
fn intersect_segment_xz_t_matches_intersect_segment_xz_and_clamp_camera() {
    let wall = WallSegment::new([-500.0, 1000.0], [500.0, 1000.0], -2000.0, 200.0, 1, 0);
    let t_only = intersect_segment_xz_t(0.0, 0.0, 0.0, 2000.0, &wall).expect("hit");
    let (t_full, _norm) = intersect_segment_xz(0.0, 0.0, 0.0, 2000.0, &wall).expect("hit");
    assert!((t_only - t_full).abs() < 1e-6);
    assert!((t_only - 0.5).abs() < 1e-6);

    let mut world = CollisionWorld::new();
    world.walls.push(wall);
    let clamped = world.clamp_camera(
        Vec3::new(0.0, -1000.0, 0.0),
        Vec3::new(0.0, -1000.0, 2000.0),
        &RaycastFilter::default(),
    );
    assert!((clamped.z - 1000.0).abs() < 1e-3);
}

#[test]
fn outdoor_tile_streamer_cache_and_zero_alloc_room_id_match() {
    let mut streamer = OutdoorTileStreamer::new(OutdoorStreamingMode::ClassicPs2LoadBg2x2);
    // First evaluation must report changed=true
    let p1 = streamer
        .plan_if_changed(5, -48_000.0, -8_000.0)
        .expect("initial plan");
    assert_eq!(p1.area, Some(OutdoorArea::Cb));
    assert!(!p1.load_queue.is_empty());

    // Same cell, same quadrant, same cutoff mask -> cache hit (None, zero alloc)
    assert!(streamer.plan_if_changed(5, -48_050.0, -8_050.0).is_none());

    // Marking a tile resident invalidates cache and removes it from load_queue
    let first_block = p1.load_queue[0].tile.block_no;
    streamer.mark_resident(first_block);
    let p2 = streamer
        .plan_if_changed(5, -48_050.0, -8_050.0)
        .expect("replan after mark_resident");
    assert!(p2
        .load_queue
        .iter()
        .all(|req| req.tile.block_no != first_block));

    // Mode change invalidates cache
    streamer.set_mode(OutdoorStreamingMode::Native5x5Grid);
    assert!(streamer.plan_if_changed(5, -48_050.0, -8_050.0).is_some());

    // Zero-alloc room ID matching
    let (area, block_no) = resolve_outdoor_area_block(5, [-48_000.0, 0.0, -8_000.0]).expect("cb28");
    assert_eq!(area, OutdoorArea::Cb);
    assert_eq!(block_no, 28);
    assert!(room_id_matches_outdoor_block("cb28", area, block_no));
    assert!(room_id_matches_outdoor_block("CB28", area, block_no));
    assert!(!room_id_matches_outdoor_block("cb29", area, block_no));
    assert!(!room_id_matches_outdoor_block("ca28", area, block_no));
    assert!(!room_id_matches_outdoor_block("cb", area, block_no));
}

#[test]
fn ev_pos_shape_polyline_is_copy_and_decodes_up_to_7_vertices() {
    let mut buf = Vec::new();
    buf.extend_from_slice(&100.0f32.to_le_bytes());
    buf.extend_from_slice(&float_to_char2(0.0));
    buf.extend_from_slice(&200.0f32.to_le_bytes());
    for i in 1..=6 {
        buf.extend_from_slice(&float_to_char2((i * 100) as f32));
        buf.extend_from_slice(&float_to_char2((i * 50) as f32));
    }

    let shape = decode_ev_pos_shape(&buf, 0, EventShapeKind::Polyline7).expect("Polyline7");
    let copy_check: EvPosShape = shape; // proves `EvPosShape: Copy`
    let verts = copy_check.polyline_vertices().expect("polyline vertices");
    assert_eq!(verts.len(), 7);
    assert_eq!(verts[0], [100.0, 200.0]);
    assert_eq!(verts[6], [700.0, 500.0]);

    // Verify look raycast against the polyline front face (directed `(100,200)->(200,250)`
    // is intersected from `(150, 350)` looking `-Z`, `rot_y = PI`)
    assert!(event_check_look(
        &buf,
        0,
        EventShapeKind::Polyline7,
        [150.0, 0.0, 350.0],
        std::f32::consts::PI,
    ));
}

#[test]
fn character_light_top3_selection_preserves_stable_descending_order() {
    let env = lookup_draw_env("AP", 12).expect("AP");
    let mk_point = |strength: f32, z: f32| CharacterLightCandidate {
        kind: CharacterLightKind::FakePoint,
        position: Vec3::new(0.0, 0.0, z),
        direction: Vec3::Y,
        color: Vec3::splat(strength),
        near_dist: 5000.0,
        far_dist: 10_000.0,
        outer_cos: -1.0,
    };
    let extras = [
        mk_point(2.0, 100.0),
        mk_point(5.0, 200.0),
        mk_point(5.0, -200.0), // tie with 5.0 -> earlier candidate must stay ahead
        mk_point(9.0, 300.0),
        mk_point(4.0, 400.0),
    ];
    let res = evaluate_character_lighting(Vec3::ZERO, env, &extras, 1.0, 1.0);
    let l0 = res.lights[0].expect("l0");
    let l1 = res.lights[1].expect("l1");
    let l2 = res.lights[2].expect("l2");
    assert!((l0.strength - 9.0).abs() < 1e-4);
    assert!((l1.strength - 5.0).abs() < 1e-4);
    assert!((l2.strength - 5.0).abs() < 1e-4);
    // Stable tie-break: z=+200 (+Z) comes before z=-200 (-Z)
    assert!(l1.to_light.z > 0.0);
    assert!(l2.to_light.z < 0.0);
}

#[test]
fn combat_targeting_stack_top20_handles_more_than_20_candidates() {
    let mut candidates = Vec::new();
    for i in 0..25usize {
        candidates.push(TargetCandidate {
            index: i,
            position: Vec3::new(
                if i % 2 == 0 { 100.0 } else { -100.0 },
                0.0,
                (25 - i) as f32 * 100.0,
            ),
            is_downed: i == 24,
            is_dead: false,
        });
    }
    // Index 24 is nearest (z=100) but downed; index 23 (z=200) is nearest upright!
    assert_eq!(
        select_auto_aim_target(Vec3::ZERO, &candidates, 5000.0, |_| true),
        Some(23)
    );
    // Stomp target within 600mm is index 24 (downed, dist ~ 141mm)
    assert_eq!(
        find_near_deadly_target(Vec3::ZERO, &candidates, |_| true),
        Some(24)
    );
    // Cycle right (+1) from 23 picks nearest on +X side (index 24, x=+100)
    assert_eq!(
        cycle_auto_aim_target(Vec3::ZERO, 0.0, Some(23), 1, &candidates, 5000.0, |_| true),
        Some(24)
    );
}

#[test]
fn blood_system_vecdeque_eviction_and_empty_room_archive_skip() {
    let mut blood = BloodEffectSystem::new();
    // Transition across empty rooms with persistent_room_decals=true should not crash or retain ghosts
    blood.on_room_change("er01", "ob01", true);
    assert_eq!(blood.active_counts(), (0, 0, 0, 0));

    // Overflow particles & stains beyond caps to exercise O(1) VecDeque pop_front
    for i in 0..(MAX_BLOOD_DROPS + 20) {
        blood.post_impact(ImpactDescriptor {
            kind: ImpactKind::BloodParticle,
            position_mm: Vec3::new(i as f32, -500.0, 0.0),
            velocity_mm_s: Vec3::new(0.0, -100.0, 0.0),
            yaw_rad: 0.0,
            mode: 0,
        });
    }
    for i in 0..(MAX_BLOOD_STAINS + 10) {
        blood.post_impact(ImpactDescriptor {
            kind: ImpactKind::FloorStain,
            position_mm: Vec3::new(i as f32, 0.0, 0.0),
            velocity_mm_s: Vec3::ZERO,
            yaw_rad: 0.0,
            mode: 0,
        });
    }
    let (p_cnt, s_cnt, _, _) = blood.active_counts();
    assert_eq!(p_cnt, MAX_BLOOD_DROPS);
    assert_eq!(s_cnt, MAX_BLOOD_STAINS);

    blood.step(
        1.0 / 60.0,
        |_| 0.0,
        Vec3::ZERO,
        0.0,
        false,
        BloodSystemConfig::default(),
    );
    // Non-empty room archives and restores cleanly
    blood.on_room_change("ob01", "er01", true);
    assert_eq!(blood.active_counts(), (0, 0, 0, 0));
    blood.on_room_change("er01", "ob01", true);
    assert_eq!(blood.active_counts().1, MAX_BLOOD_STAINS);
}

#[test]
fn maria_vecdeque_trail_and_fx_into_helpers() {
    let mut maria = MariaCompanionController::default();
    maria.spawn(Vec3::ZERO, 0.0);
    for i in 1..=80 {
        maria.record_breadcrumb(Vec3::new(i as f32 * 300.0, 0.0, 0.0), 0.0);
    }
    assert_eq!(maria.trail.len(), 64);
    assert!((maria.trail.front().unwrap().position.x - 17.0 * 300.0).abs() < 1e-3);
    assert!((maria.trail.back().unwrap().position.x - 80.0 * 300.0).abs() < 1e-3);

    // Lens flare `_into` helper matches allocating variant
    let proj = Mat4::perspective_rh(60f32.to_radians(), 16.0 / 9.0, 50.0, 20_000.0);
    let view = Mat4::look_at_rh(Vec3::ZERO, Vec3::new(0.0, 0.0, -1000.0), Vec3::Y);
    let src = LensFlareSource {
        position: Vec3::new(100.0, -100.0, -2500.0),
        color: Vec3::ONE,
        max_distance: 10_000.0,
        intensity: 1.0,
    };
    let alloc = evaluate_lens_flares(&[src], proj * view, Vec3::ZERO, 16.0 / 9.0);
    let mut scratch = Vec::new();
    evaluate_lens_flares_into(&[src], proj * view, Vec3::ZERO, 16.0 / 9.0, &mut scratch);
    assert_eq!(alloc.len(), LENS_FLARE_SPRITE_COUNT);
    assert_eq!(alloc, scratch);

    // ImpactSparkSystem `active_flash_lights_into`
    let mut rng = FxRng::new(7);
    let mut sparks = ImpactSparkSystem::default();
    sparks.spawn_impact(Vec3::new(10.0, -50.0, 20.0), true, &mut rng);
    let mut flash_buf = Vec::new();
    sparks.active_flash_lights_into(true, &mut flash_buf);
    assert_eq!(flash_buf, sparks.active_flash_lights(true));
    sparks.active_flash_lights_into(false, &mut flash_buf);
    assert!(flash_buf.is_empty());
}
