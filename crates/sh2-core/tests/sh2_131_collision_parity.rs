//! SH2-131 1:1 Collision Parity Tests
//!
//! Verifies golden-vector and retail-asset parity against `SLUS_202.28` (`Collision/cl_main.c`,
//! `cl_calc2.c`, `Chacter/m3_play.c`) and retail PC `sh2pc.exe`:
//! 1. `.cld` parsing of `_CL_HITPOLY_COLUMN` (`cldofs`/`csize`), `b2ofs`/`b3ofs`/`clofs`, `disable` (`+0x1c`),
//!    and `SpatialGrid::query_move_cells` (`clGetHitSectListMOVE` InDoor vs OutDoor 30,000^2 / 8,000^2).
//! 2. Dual player cylinders (`800mm` move `[-50..-850]` / `900mm` hit `[0..-900]`, `175mm` James /
//!    `150mm` playable Maria, `+25mm` forward offset) & `PlayerSetColumn_SetTarget`/`CloseToTarget`.
//! 3. `clCheckColumn2WallHit` (backface culling, face vs corner hits), `clCheckColumn2ColumnHit`,
//!    `clMakeWallHitCollectVector` (89/75/2/90/120 deg rules + `1/16` quantization), and `clAddWallCollectVector`.
//! 4. `clCollectCharaALL` 5 interleaved iterations + discrete weight-diff table `[1.0, 0.98, 0.94, 0.90, 0.86]`.

use glam::Vec3;
use sh2_assets::formats::cld::{parse_cld, CldFile, HitPolyColumn, HitPolyPlane, WallSegment};
use sh2_core::collision::{
    cl_add_wall_collect_vector, cl_check_column_to_bg_column_hit, cl_check_column_to_wall_hit,
    cl_collect_chara_all, cl_make_wall_hit_collect_vector, quantize_collect_vector_1_16, sh_atan2,
    weight_diff_recoil_factor, CharaCollectEntry, CharacterCylinder, CharacterId, CollisionColumn,
    CollisionConfig, CollisionController, CollisionWorld, PlayerColumnAction, PlayerColumnState,
    SpatialGrid, WallCollectContext, WallHitKind, WallHitRecord, CL_COLLECT_CHARA_ITERATIONS,
    CL_WEIGHT_DIFF_TABLE, JAMES_HIT_RADIUS, JAMES_MOVE_RADIUS, MARIA_HIT_RADIUS, MARIA_MOVE_RADIUS,
    PLAYER_BASE_FORWARD_OFFSET, PLAYER_HIT_COL_HEIGHT, PLAYER_MOVE_COL_HEIGHT,
};

#[test]
fn test_cld_header_disable_and_outdoor_spatial_grid_culling() {
    // Construct an outdoor CldFile with origin (-60000, -20000) and populated cells > 0
    let mut cld = CldFile {
        origin: [-60_000.0, -20_000.0],
        disable: 0,
        ..Default::default()
    };
    cld.b1_cells[5].push(0); // cell (row=1, col=1) -> corner (-55000, -15000)
    cld.cl_cells[12].push(0); // cell (row=3, col=0) -> corner (-60000, -5000)

    let grid = SpatialGrid::from_cld(&cld);
    assert!(grid.is_outdoor);

    let mut cells = Vec::new();
    // Point at (-55000, 0, -15000): dist_sq to block origin = 5000^2 + 5000^2 = 5e7 <= 9e8
    // Dist to cell (1,1) corner (-55000, -15000) = 0 < 6.4e7 -> included!
    // Dist to cell (3,3) corner (-45000, -5000) = 10000^2 + 10000^2 = 2e8 >= 6.4e7 -> excluded!
    grid.query_move_cells(-55_000.0, 0.0, -15_000.0, &mut cells);
    assert!(cells.contains(&5), "cell (1,1) must be active: {:?}", cells);
    assert!(
        !cells.contains(&15),
        "distant cell (3,3) must be culled: {:?}",
        cells
    );

    // Beyond 30,000 mm from block origin: entire block culled (> 9e8)
    grid.query_move_cells(0.0, 0.0, 0.0, &mut cells);
    assert!(
        cells.is_empty(),
        "block beyond 30,000 mm must be culled: {:?}",
        cells
    );

    // Header disable != 0 must always return empty
    let mut disabled_cld = cld.clone();
    disabled_cld.disable = 1;
    let disabled_grid = SpatialGrid::from_cld(&disabled_cld);
    disabled_grid.query_move_cells(-55_000.0, 0.0, -15_000.0, &mut cells);
    assert!(
        cells.is_empty(),
        "disabled .cld (+0x1c != 0) must emit zero cells"
    );
}

#[test]
fn test_retail_cld_columns_and_cell_tables() {
    let resolver = sh2_assets::AssetPathResolver::resolve();

    // 1. Indoor room: er01.cld (cell 0 only, b2/b3 populated)
    if let Some(er01_path) = resolver.resolve_file("bg/er/er01.cld") {
        let bytes = std::fs::read(&er01_path).expect("read er01.cld");
        let cld = parse_cld(&bytes).expect("parse er01.cld");
        assert!(!cld.is_disabled());
        assert!(!cld.has_outdoor_grid());
        assert!(!cld.b1_cells[0].is_empty());
        assert!(!cld.b2_cells[0].is_empty());
        assert!(!cld.b3_cells[0].is_empty());

        let grid = SpatialGrid::from_cld(&cld);
        let mut cells = Vec::new();
        grid.query_move_cells(-19_300.0, 0.0, 20_000.0, &mut cells);
        assert_eq!(cells, vec![0], "InDoor .cld must query cell 0 only");
    }

    // 2. Bowling alley: bw01.cld has 9 active static BG columns (`cldofs`/`csize`)
    if let Some(bw01_path) = resolver.resolve_file("bg/bw/bw01.cld") {
        let bytes = std::fs::read(&bw01_path).expect("read bw01.cld");
        let cld = parse_cld(&bytes).expect("parse bw01.cld");
        let active_cols: Vec<&HitPolyColumn> =
            cld.columns.iter().filter(|c| c.is_active()).collect();
        assert_eq!(
            active_cols.len(),
            9,
            "bw01.cld must parse all 9 active static BG columns"
        );
        // Verify collision against the first real static BG column in bw01.cld
        let c0 = active_cols[0];
        let world = CollisionWorld::from_cld(&cld);
        let cfg = CollisionConfig::default();
        let mut pos = Vec3::new(c0.center_x + c0.radius + 50.0, 0.0, c0.center_z);
        let pushed = world.resolve_wall_pushback(&mut pos, &cfg);
        assert!(pushed, "James must collide with retail bw01.cld BG column");
        let dist = ((pos.x - c0.center_x).powi(2) + (pos.z - c0.center_z).powi(2)).sqrt();
        assert!(
            dist >= c0.radius + cfg.radius - 0.5,
            "James must be pushed outside bw01.cld column: dist={} < {}",
            dist,
            c0.radius + cfg.radius
        );
    }

    // 3. Outdoor block: ca18.cld has outdoor 4x4 grid + 3 active static BG columns
    if let Some(ca18_path) = resolver.resolve_file("bg/ca/ca18.cld") {
        let bytes = std::fs::read(&ca18_path).expect("read ca18.cld");
        let cld = parse_cld(&bytes).expect("parse ca18.cld");
        assert!(
            cld.has_outdoor_grid(),
            "ca18.cld must be detected as OutDoor"
        );
        let active_cols = cld.columns.iter().filter(|c| c.is_active()).count();
        assert_eq!(active_cols, 3, "ca18.cld must parse 3 active BG columns");
        assert!(
            cld.cl_cells.iter().any(|c| !c.is_empty()),
            "ca18.cld must populate cl_cells (clofs)"
        );
    }
}

#[test]
fn test_player_dual_cylinders_and_close_to_target() {
    let cfg = CollisionConfig::default();
    assert_eq!(cfg.radius, JAMES_MOVE_RADIUS);
    assert_eq!(cfg.height, PLAYER_MOVE_COL_HEIGHT);
    assert_eq!(cfg.hit_radius, JAMES_HIT_RADIUS);
    assert_eq!(cfg.hit_height, PLAYER_HIT_COL_HEIGHT);
    assert_eq!(cfg.forward_offset, PLAYER_BASE_FORWARD_OFFSET);
    assert_eq!(cfg.max_iterations, CL_COLLECT_CHARA_ITERATIONS);

    // James normal state at origin, facing +Z (yaw = 0)
    // PlayerSetColumn_SetTarget sets hit_col target Y to [0..-900], while PlayerSetColumn_CloseToTarget
    // copies move_col Y [-50..-850] into the live hit_col (SLUS 0x00108010 / sh2pc 0x00535120).
    let mut james = PlayerColumnState::new(Vec3::ZERO, false);
    let normal_target = james.compute_target(Vec3::ZERO, PlayerColumnAction::Normal, false);
    assert_eq!(normal_target.hit_col.y_bottom, 0.0);
    assert_eq!(normal_target.hit_col.y_top, -900.0);
    james.close_to_target(Vec3::ZERO, 0.0, &normal_target);
    assert_eq!(james.move_col.radius, 175.0);
    assert_eq!(james.move_col.y_bottom, -50.0);
    assert_eq!(james.move_col.y_top, -850.0);
    assert_eq!(james.move_col.center_z, 25.0);
    assert_eq!(james.hit_col.radius, 150.0);
    assert_eq!(james.hit_col.y_bottom, -50.0);
    assert_eq!(james.hit_col.y_top, -850.0);
    assert_eq!(james.hit_col.center_z, 25.0);

    // Playable Maria (0x10e) at origin, facing +X (yaw = PI/2)
    let mut maria = PlayerColumnState::new(Vec3::ZERO, true);
    maria.step(
        Vec3::ZERO,
        std::f32::consts::FRAC_PI_2,
        PlayerColumnAction::Normal,
        false,
    );
    assert_eq!(maria.move_col.radius, MARIA_MOVE_RADIUS);
    assert_eq!(maria.hit_col.radius, MARIA_HIT_RADIUS);
    assert!((maria.move_col.center_x - 25.0).abs() < 1e-3);
    assert!(maria.move_col.center_z.abs() < 1e-3);

    // PlayerSetColumn_CloseToTarget converges at exact 5.0 mm/tick toward Great Knife aim (0.65 * 500 = 325.0)
    let target = james.compute_target(Vec3::ZERO, PlayerColumnAction::Aim { weapon_id: 3 }, false);
    assert_eq!(target.move_col.radius, 325.0);
    assert_eq!(target.move_fwd_offset, 100.0);
    james.close_to_target(Vec3::ZERO, 0.0, &target);
    assert_eq!(james.move_col.radius, 180.0); // 175 + 5
    assert_eq!(james.move_fwd_offset, 30.0); // 25 + 5
}

#[test]
fn test_cl_check_column_to_wall_hit_backface_face_and_corner() {
    // Wall along X = 0..1000 at Z = 0, normal pointing +Z (front side is Z >= 0)
    let wall = HitPolyPlane::from_wall_segment(&WallSegment::new(
        [0.0, 0.0],
        [1000.0, 0.0],
        -2000.0,
        100.0,
        1,
        0,
    ));

    // 1. Backface culling: cylinder on Z = -100 (behind wall) must NOT hit!
    let col_back = CollisionColumn::new(500.0, -50.0, -100.0, -850.0, 175.0, 1);
    assert!(
        cl_check_column_to_wall_hit(&wall, &col_back).is_none(),
        "Backface of one-sided wall must be culled"
    );

    // 2. Face hit: cylinder at (500, z=100) -> penetration = 75.0 along +Z
    let col_face = CollisionColumn::new(500.0, -50.0, 100.0, -850.0, 175.0, 1);
    let face_hit = cl_check_column_to_wall_hit(&wall, &col_face).expect("face hit");
    assert_eq!(face_hit.kind, WallHitKind::Face);
    assert!((face_hit.collect_vec.z - 75.0).abs() < 1e-3);
    assert!(face_hit.collect_vec.x.abs() < 1e-3);

    // 3. Corner hit: cylinder past endpoint p[0]=(0,0) at (-60, 80), dist = 100 < 175
    // Corner pushout includes +1.0 mm margin: scale = (175 - 100 + 1) / 100 = 0.76
    let col_corner = CollisionColumn::new(-60.0, -50.0, 80.0, -850.0, 175.0, 1);
    let corner_hit = cl_check_column_to_wall_hit(&wall, &col_corner).expect("corner hit");
    assert_eq!(corner_hit.kind, WallHitKind::Corner);
    assert!((corner_hit.collect_vec.x - (-60.0 * 0.76)).abs() < 1e-3);
    assert!((corner_hit.collect_vec.z - (80.0 * 0.76)).abs() < 1e-3);
}

#[test]
fn test_cl_check_column_to_bg_column_hit() {
    let bg_col = HitPolyColumn {
        kind: 1,
        shape: 3,
        weight: 4,
        material: 7,
        flg: 0,
        center_x: 1000.0,
        y_bottom: 0.0,
        center_z: 1000.0,
        y_top: -2500.0,
        radius: 100.0,
    };
    let chara = CollisionColumn::new(1200.0, -50.0, 1000.0, -850.0, 175.0, 1);
    // Combined radius = 275, dist = 200 -> pen = 75, +1.0 margin = 76.0 along +X
    let hit = cl_check_column_to_bg_column_hit(&bg_col, &chara).expect("bg column hit");
    assert_eq!(hit.kind, WallHitKind::BgColumn);
    assert!((hit.collect_vec.x - 76.0).abs() < 1e-3);
    assert!(hit.collect_vec.z.abs() < 1e-3);
}

#[test]
fn test_cl_make_wall_hit_collect_vector_suppression_and_quantization() {
    // 1. Parallel corner-vs-face suppression (within 2 deg)
    let wall_a = HitPolyPlane::from_wall_segment(&WallSegment::new(
        [0.0, 0.0],
        [1000.0, 0.0],
        -2000.0,
        100.0,
        1,
        0,
    ));
    let mut hits = vec![
        WallHitRecord {
            kind: WallHitKind::Face,
            collect_vec: Vec3::new(0.0, 0.0, 40.0),
            plane_eq: Some(wall_a.plane_equation()),
            normal_angle: 0.0,
        },
        WallHitRecord {
            kind: WallHitKind::Corner,
            collect_vec: Vec3::new(30.0, 0.0, 20.0),
            plane_eq: Some(wall_a.plane_equation()),
            normal_angle: 0.0,
        },
    ];
    let mut ctx = WallCollectContext::default();
    let (v, applied) = cl_make_wall_hit_collect_vector(&mut hits, &mut ctx);
    assert_eq!(hits[1].kind, WallHitKind::Suppressed);
    assert_eq!(applied, 1);
    assert_eq!(v, Vec3::new(0.0, 0.0, 40.0));

    // 2. Opposing-wall 120-deg pinch suppression (chara_type == 1)
    let wall_n = HitPolyPlane::from_wall_segment(&WallSegment::new(
        [0.0, 0.0],
        [1000.0, 0.0],
        -2000.0,
        100.0,
        1,
        0,
    ));
    let wall_s = HitPolyPlane::from_wall_segment(&WallSegment::new(
        [1000.0, 300.0],
        [0.0, 300.0],
        -2000.0,
        100.0,
        1,
        0,
    ));
    let mut pinch_hits = vec![
        WallHitRecord {
            kind: WallHitKind::Face,
            collect_vec: Vec3::new(0.0, 0.0, 25.0),
            plane_eq: Some(wall_n.plane_equation()),
            normal_angle: 0.0,
        },
        WallHitRecord {
            kind: WallHitKind::Face,
            collect_vec: Vec3::new(0.0, 0.0, -25.0),
            plane_eq: Some(wall_s.plane_equation()),
            normal_angle: 0.0,
        },
    ];
    let (pv, p_applied) = cl_make_wall_hit_collect_vector(&mut pinch_hits, &mut ctx);
    assert_eq!(pinch_hits[0].kind, WallHitKind::PinchSuppressed);
    assert_eq!(pinch_hits[1].kind, WallHitKind::PinchSuppressed);
    assert_eq!(p_applied, 0);
    assert_eq!(pv, Vec3::ZERO);

    // 3. Trailing-normal 75-deg suppression when moving in same direction as wall normal (+Z)
    let mut trail_hits = vec![
        WallHitRecord {
            kind: WallHitKind::Face,
            collect_vec: Vec3::new(0.0, 0.0, 15.0),
            plane_eq: Some(wall_n.plane_equation()), // normal +Z (angle 0)
            normal_angle: 0.0,
        },
        WallHitRecord {
            kind: WallHitKind::Face,
            collect_vec: Vec3::new(10.0, 0.0, 0.0),
            plane_eq: Some(
                HitPolyPlane::from_wall_segment(&WallSegment::new(
                    [0.0, 1000.0],
                    [0.0, 0.0],
                    -2000.0,
                    100.0,
                    1,
                    0,
                ))
                .plane_equation(),
            ), // normal +X (angle PI/2 = 90 deg > 75 deg)
            normal_angle: 0.0,
        },
    ];
    let mut move_ctx = WallCollectContext {
        move_angle: sh_atan2(0.0, 1.0), // moving +Z
        ..Default::default()
    };
    let (tv, t_applied) = cl_make_wall_hit_collect_vector(&mut trail_hits, &mut move_ctx);
    assert_eq!(trail_hits[0].kind, WallHitKind::Suppressed);
    assert_eq!(t_applied, 1);
    assert_eq!(tv, Vec3::new(10.0, 0.0, 0.0));

    // 4. clAddWallCollectVector clamped accumulation & 1/16 quantization
    let mut acc = Vec3::ZERO;
    let mut cnt = 0usize;
    cl_add_wall_collect_vector(&mut acc, Vec3::new(10.0, 0.0, -5.0), &mut cnt);
    cl_add_wall_collect_vector(&mut acc, Vec3::new(6.0, 0.0, -12.0), &mut cnt);
    // Clamped to [min(a,b), max(a,b)]: X=clamp(16, 6, 10)=10, Z=clamp(-17, -12, -5)=-12
    assert_eq!(acc, Vec3::new(10.0, 0.0, -12.0));
    assert_eq!(cnt, 2);

    let q = quantize_collect_vector_1_16(Vec3::new(10.19, 0.0, -5.07));
    assert_eq!(q, Vec3::new(10.1875, 0.0, -5.0625));
}

#[test]
fn test_cl_collect_chara_all_5_iterations_and_weight_table() {
    assert_eq!(CL_COLLECT_CHARA_ITERATIONS, 5);
    assert_eq!(CL_WEIGHT_DIFF_TABLE, [1.0, 0.98, 0.94, 0.90, 0.86]);

    // Discrete weight-diff recoil table verification
    assert!((weight_diff_recoil_factor(1, 2) - 0.0).abs() < 1e-6); // lighter -> 0 recoil on heavier
    assert!((weight_diff_recoil_factor(1, 1) - 0.02).abs() < 1e-6); // equal -> 1 - 0.98
    assert!((weight_diff_recoil_factor(2, 1) - 0.06).abs() < 1e-6); // +1 -> 1 - 0.94
    assert!((weight_diff_recoil_factor(3, 1) - 0.10).abs() < 1e-6); // +2 -> 1 - 0.90
    assert!((weight_diff_recoil_factor(4, 1) - 0.14).abs() < 1e-6); // +3 -> 1 - 0.86
    assert!((weight_diff_recoil_factor(9, 1) - 0.14).abs() < 1e-6); // clamped at index 4

    // Heavy boss (weight 4) vs James (weight 1): boss must NOT be pushed by James
    let mut entries = vec![
        CharaCollectEntry {
            col: CollisionColumn::new(0.0, -50.0, 0.0, -850.0, 175.0, 1),
            move_vec: Vec3::ZERO,
            push_vec: Vec3::ZERO,
            prev_pos: Vec3::ZERO,
            chara_type: 1,
        },
        CharaCollectEntry {
            col: CollisionColumn::new(200.0, -50.0, 0.0, -850.0, 175.0, 4),
            move_vec: Vec3::ZERO,
            push_vec: Vec3::ZERO,
            prev_pos: Vec3::new(200.0, 0.0, 0.0),
            chara_type: 2,
        },
    ];
    let iters = cl_collect_chara_all(&mut entries, |_| Vec3::ZERO);
    assert!((1..=5).contains(&iters));
    assert!(
        (entries[1].col.center_x - 200.0).abs() < 1e-3,
        "Heavy boss (weight 4) must not be pushed by James (weight 1): x={}",
        entries[1].col.center_x
    );
    assert!(
        entries[0].col.center_x <= 200.0 - 350.0,
        "James must be pushed out of boss cylinder: x={}",
        entries[0].col.center_x
    );

    // Interleaved BG check prevents character separation from pushing James through a wall
    let room = CollisionWorld::create_synthetic_room(2000.0, 2000.0, 3000.0, 0.0, 0.0);
    let mut ctrl = CollisionController::new(room, CollisionConfig::default());
    // West wall at X = -1000; James near west wall, boss pressing from east
    ctrl.register_character(CharacterCylinder::james(
        CharacterId(1),
        Vec3::new(-800.0, 0.0, 0.0),
    ));
    ctrl.register_character(CharacterCylinder::new(
        CharacterId(2),
        Vec3::new(-600.0, 0.0, 0.0),
        175.0,
        800.0,
        10.0,
    ));
    let iters = ctrl.resolve_character_separations();
    assert!((1..=5).contains(&iters));
    let james = ctrl
        .characters
        .iter()
        .find(|c| c.id == CharacterId(1))
        .unwrap();
    assert!(
        james.position.x >= -1000.0 + 175.0 - 0.5,
        "Interleaved BG check must keep James inside west wall: x={}",
        james.position.x
    );
}

#[test]
fn test_regulate_angle_sentinel_no_hang_and_disabled_cld_floor_gating() {
    use sh2_core::collision::{regulate_angle, CL_NO_MOVE_ANGLE};

    // 1. regulate_angle must terminate in O(1) on CL_NO_MOVE_ANGLE (f32::MAX) and non-finite inputs
    assert_eq!(regulate_angle(0.0 - CL_NO_MOVE_ANGLE), 0.0);
    assert_eq!(regulate_angle(f32::INFINITY), 0.0);
    assert_eq!(regulate_angle(f32::NAN), 0.0);
    assert!((regulate_angle(3.0 * std::f32::consts::PI) - std::f32::consts::PI).abs() < 1e-5);

    // 2. cl_make_wall_hit_collect_vector with post_flag_timer > 0 and stationary move_angle must not hang
    let wall = HitPolyPlane::from_wall_segment(&WallSegment::new(
        [0.0, 0.0],
        [1000.0, 0.0],
        -2000.0,
        100.0,
        1,
        0,
    ));
    let mut hits = vec![
        WallHitRecord {
            kind: WallHitKind::Face,
            collect_vec: Vec3::new(0.0, 0.0, 50.0),
            plane_eq: Some(wall.plane_equation()),
            normal_angle: 0.0,
        },
        WallHitRecord {
            kind: WallHitKind::Face,
            collect_vec: Vec3::new(0.0, 0.0, 30.0),
            plane_eq: Some(wall.plane_equation()),
            normal_angle: 0.0,
        },
    ];
    let mut ctx = WallCollectContext {
        move_angle: CL_NO_MOVE_ANGLE,
        post_flag_timer: 2,
        fps: 60.0,
        ..Default::default()
    };
    let (_v, applied) = cl_make_wall_hit_collect_vector(&mut hits, &mut ctx);
    assert!(applied > 0);
    assert_eq!(ctx.post_flag_timer, 1);

    // 3. Disabled .cld (`disable != 0`) must skip static floor sampling
    let mut room = CollisionWorld::create_synthetic_room(2000.0, 2000.0, 3000.0, 0.0, 0.0);
    assert!(room
        .sample_floor_surface(0.0, 0.0, 0.0, 250.0, 350.0)
        .is_some());
    room.disable = 1;
    assert!(
        room.sample_floor_surface(0.0, 0.0, 0.0, 250.0, 350.0)
            .is_none(),
        "Disabled .cld must not return static floor samples"
    );

    // 4. Explicit discrete weight setter
    let heavy = CharacterCylinder::james(CharacterId(9), Vec3::ZERO).with_weight(4);
    assert_eq!(heavy.weight, 4);
}
