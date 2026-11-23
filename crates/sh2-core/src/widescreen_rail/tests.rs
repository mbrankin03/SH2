use super::*;
use crate::camera::Ps2CameraController;
use sh2_assets::formats::cam::{parse_cam, CamFile, ChaseCamParam};
use sh2_assets::AssetPathResolver;
use std::path::{Path, PathBuf};

fn collect_cams(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for p in rd.flatten().map(|e| e.path()) {
        if p.is_dir() {
            collect_cams(&p, out);
        } else if p.extension().is_some_and(|s| s.eq_ignore_ascii_case("cam")) {
            out.push(p);
        }
    }
}

fn box_xz(x0: f32, x1: f32, min_hy: f32, max_hy: f32) -> RoadArea {
    RoadArea {
        x0,
        z0: -2000.0,
        x1: x0,
        z1: 2000.0,
        x2: x1,
        z2: 2000.0,
        min_hy,
        max_hy,
    }
}

#[test]
fn test_margins_across_4x3_16x10_16x9_20x9_21x9_and_narrow_rail_collapse() {
    let crt = AspectRailConfig::authentic_ps2_4x3();
    assert!((compute_aspect_rail_margin_mm(false, &crt) - 400.0).abs() < 1e-4);
    assert!((compute_aspect_rail_margin_mm(true, &crt) - 500.0).abs() < 1e-4);
    assert!(
        (compute_aspect_rail_margin_mm(false, &AspectRailConfig::dynamic_widescreen(1.25)) - 400.0)
            .abs()
            < 1e-4
    );
    assert!(
        (compute_aspect_rail_margin_mm(false, &AspectRailConfig::dynamic_from_dimensions(0, 0))
            - 400.0)
            .abs()
            < 1e-4
    );

    for aspect in [
        ASPECT_16X10,
        ASPECT_16X9,
        ASPECT_20X9_THOR,
        ASPECT_21X9_ULTRAWIDE,
    ] {
        let cfg = AspectRailConfig::dynamic_widescreen(aspect);
        let scale = (aspect / PS2_REFERENCE_4X3_ASPECT).sqrt();
        assert!((compute_aspect_rail_margin_mm(false, &cfg) - 400.0 * scale).abs() < 1e-3);
        assert!((compute_aspect_rail_margin_mm(true, &cfg) - 500.0 * scale).abs() < 1e-3);
    }

    // Width 900 mm: [400, 500] in 4:3, collapses to midpoint 450.0 in 21:9 (~529.15 mm margin)
    let rail = box_xz(0.0, 900.0, -1000.0, -1000.0);
    let p43 = clamp_to_road_rail_with_config(&rail, Vec3::new(10.0, 0.0, -1900.0), false, &crt);
    let uw = AspectRailConfig::dynamic_widescreen(ASPECT_21X9_ULTRAWIDE);
    let p21 = clamp_to_road_rail_with_config(&rail, Vec3::new(10.0, 0.0, -1900.0), false, &uw);
    assert!((p43.x - 400.0).abs() < 1e-3 && (p43.z - (-1600.0)).abs() < 1e-3);
    assert!((p21.x - 450.0).abs() < 1e-3 && p21.z > p43.z);
}

#[test]
fn test_intra_room_transition_policies_not_warp_veto_and_door_reset() {
    let p_auth = IntraRoomTransitionPolicy::AuthenticPs2InstantWarp;
    let p_out = IntraRoomTransitionPolicy::smooth_warp_out_default();
    assert!(matches!(
        evaluate_intra_room_transition(VC_RD_WARP_OUT_F | VC_RD_NOT_WARP_F, 0, p_auth),
        IntraRoomTransitionStep::SmoothBlend(_)
    ));
    assert!(matches!(
        evaluate_intra_room_transition(0, VC_RD_WARP_IN_F | VC_RD_NOT_WARP_F, p_auth),
        IntraRoomTransitionStep::SmoothBlend(_)
    ));
    assert!(matches!(
        evaluate_intra_room_transition(VC_RD_WARP_OUT_F, 0, p_out),
        IntraRoomTransitionStep::SmoothBlend(_)
    ));
    assert_eq!(
        evaluate_intra_room_transition(VC_RD_WARP_OUT_F, VC_RD_WARP_IN_F, p_out),
        IntraRoomTransitionStep::InstantWarpCut
    );

    let mk_rec = |x0: f32, x1: f32, ex: f32, mv: CamMovementType, flags: u16| RoadRecord {
        lim_sw: box_xz(x0, x1, -2000.0, 2000.0),
        lim_rd: box_xz(ex, ex, -1400.0, -1400.0),
        flags,
        rd_type: RoadType::Event,
        ofs_watch_hy: -300.0,
        projection: 448.0,
        proj_sec: 1.0,
        cam_mv_type: mv,
        params: CamMovementParams::Chase(ChaseCamParam::default()),
        ..RoadRecord::default()
    };
    let r_no_ext = mk_rec(0.0, 1.0, 0.0, CamMovementType::Chase, VC_RD_NO_EXTRA_AREA_F);
    assert_eq!(switch_hysteresis_advantage_mm(&r_no_ext), 0.0);

    let mut ctrl = Ps2CameraController::new();
    ctrl.enable_dynamic_widescreen_clearance(ASPECT_21X9_ULTRAWIDE);
    ctrl.set_intra_room_transition_policy(IntraRoomTransitionPolicy::smooth_all_default());
    let st = CamMovementType::Settle;
    let records = vec![
        mk_rec(-1000.0, 0.0, -500.0, st, VC_RD_WARP_OUT_F),
        mk_rec(0.0, 1000.0, 500.0, st, VC_RD_WARP_IN_F),
    ];
    ctrl.set_cam_file(Some(CamFile { records }));
    ctrl.reset_to_player(Vec3::new(-250.0, 0.0, 0.0), 0.0);
    // At 60 Hz (`40 mm/tick`), 2 ticks advance `cam_tgt_pos` 80 mm past the authentic 50 mm
    // `vcRenewalCamData` (`0x0019c8d4..0x0019c908`) XZ deadband so eye begins smooth blend.
    ctrl.tick(Vec3::new(250.0, 0.0, 0.0), 0.0, 0.0, 0.0);
    ctrl.tick(Vec3::new(250.0, 0.0, 0.0), 0.0, 0.0, 0.0);
    assert!(ctrl.curr_snapshot.eye.x > -500.0 && ctrl.curr_snapshot.eye.x < 500.0);
    assert!((ctrl.prev_snapshot.eye.x - (-500.0)).abs() < 1e-3);
    ctrl.reset_to_player(Vec3::new(250.0, 0.0, 0.0), 0.0);
    assert!((ctrl.curr_snapshot.eye.x - 500.0).abs() < 1e-3);
    assert!((ctrl.prev_snapshot.eye.x - 500.0).abs() < 1e-3);
}

#[test]
fn test_widescreen_and_outdoor_chase_rail_reset_and_settle_yaw_limits() {
    let settle_like = RoadRecord {
        lim_sw: box_xz(-2000.0, 2000.0, -2000.0, 2000.0),
        lim_rd: box_xz(-1500.0, 500.0, -1400.0, -1400.0),
        flags: VC_RD_CAM_LIKE_SETTLE_F | sh2_assets::formats::cam::VC_RD_NO_FRONT_FLIP_F,
        area_size_type: AreaSizeType::Outdoor,
        rd_type: RoadType::Road,
        ofs_watch_hy: -300.0,
        projection: 448.0,
        proj_sec: 1.0,
        cam_mv_type: CamMovementType::Chase,
        params: CamMovementParams::Chase(ChaseCamParam {
            ofs_hy: -500.0,
            ratio_r_xz: 1.0,
            lr_lim_ang_y: -0.1,
            rr_lim_ang_y: 0.1,
        }),
        ..RoadRecord::default()
    };

    // Outdoor 4:3 enforces 500.0 mm margin (-1500 + 500 = -1000), 21:9 enforces ~661.4 mm (-838.6)
    let mut c_out_43 = Ps2CameraController::new();
    c_out_43.set_cam_file(Some(CamFile {
        records: vec![settle_like.clone()],
    }));
    c_out_43.reset_to_player(Vec3::new(-1400.0, 0.0, 0.0), 1.5707964);
    assert!((c_out_43.curr_snapshot.eye.x - (-1000.0)).abs() < 1e-2);

    let mut c_out_21 = Ps2CameraController::new();
    c_out_21.enable_dynamic_widescreen_clearance(ASPECT_21X9_ULTRAWIDE);
    c_out_21.set_cam_file(Some(CamFile {
        records: vec![settle_like],
    }));
    c_out_21.reset_to_player(Vec3::new(-1400.0, 0.0, 0.0), 1.5707964);
    let expected_x = -1500.0 + 500.0 * (ASPECT_21X9_ULTRAWIDE / PS2_REFERENCE_4X3_ASPECT).sqrt();
    assert!((c_out_21.curr_snapshot.eye.x - expected_x).abs() < 1e-2);

    // Zero drift on stationary subsequent tick & yaw remains clamped within [-0.1, 0.1] rad
    c_out_21.tick(Vec3::new(-1400.0, 0.0, 0.0), 1.5707964, 0.0, 0.0);
    assert!((c_out_21.curr_snapshot.eye.x - expected_x).abs() < 1e-2);
    let look_dir = c_out_21.curr_snapshot.target - c_out_21.curr_snapshot.eye;
    let clamped_yaw = look_dir.x.atan2(look_dir.z);
    assert!(clamped_yaw.abs() <= 0.105);
}

#[test]
fn test_retail_cam_corpus_all_rooms_across_all_aspects() -> Result<(), Box<dyn std::error::Error>> {
    let resolver = AssetPathResolver::resolve();
    if !resolver.data_dir.exists() {
        return Ok(());
    }
    let mut paths = Vec::new();
    collect_cams(&resolver.data_dir, &mut paths);
    paths.sort();
    assert!(paths.len() >= 354, "expected >=354 retail .cam files");

    let cfgs = [
        ASPECT_16X10,
        ASPECT_16X9,
        ASPECT_20X9_THOR,
        ASPECT_21X9_ULTRAWIDE,
    ]
    .map(AspectRailConfig::dynamic_widescreen);
    let mut active_rooms = 0usize;
    for path in &paths {
        let cam = parse_cam(&std::fs::read(path)?)?;
        active_rooms += usize::from(!cam.records.is_empty());
        let stem = path.file_stem().and_then(|s| s.to_str());
        for rec in &cam.records {
            let out = is_outdoor_room_or_record(stem, rec.area_size_type);
            let ((min_lx, max_lx), (min_lz, max_lz)) = rec.lim_rd.local_xz_bounds();
            let (px, pz) = rec
                .lim_rd
                .local_to_world_xz(min_lx - 5000.0, max_lz + 5000.0);
            let probe = Vec3::new(px, 0.0, pz);
            let mut prev_dx = 0.0f32;
            for cfg in std::iter::once(AspectRailConfig::authentic_ps2_4x3()).chain(cfgs) {
                let c = clamp_to_road_rail_with_config(&rec.lim_rd, probe, out, &cfg);
                let (clx, clz) = rec.lim_rd.world_to_local_xz(c.x, c.z);
                assert!(c.is_finite() && clx >= min_lx - 1e-2 && clx <= max_lx + 1e-2);
                assert!(clz >= min_lz - 1e-2 && clz <= max_lz + 1e-2);
                let dx = (clx - min_lx).min(max_lx - clx);
                assert!(dx + 1e-2 >= prev_dx);
                prev_dx = dx;
            }
        }
    }
    assert!(active_rooms >= 350);
    Ok(())
}
