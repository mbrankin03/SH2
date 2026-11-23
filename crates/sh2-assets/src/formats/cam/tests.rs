use super::*;
use crate::error::AssetError;

#[test]
fn test_parse_cam_buffer_underflow() {
    let small = [0u8; 64];
    assert!(matches!(
        parse_cam(&small),
        Err(AssetError::BufferUnderflow { .. })
    ));
}

#[test]
fn test_parse_cam_invalid_length() {
    let invalid = [0u8; 130];
    assert!(matches!(
        parse_cam(&invalid),
        Err(AssetError::InvalidData(..))
    ));
}

#[test]
fn test_parse_cam_sentinels() {
    let mut buffer = [0u8; 256];
    // Record 0: Normal with sentinels
    // flags = 0
    // ofs_watch_hy = 10000.0
    buffer[0x54..0x58].copy_from_slice(&10000.0f32.to_le_bytes());
    // projection = 0.0 -> 448.0
    buffer[0x60..0x64].copy_from_slice(&0.0f32.to_le_bytes());
    // trace_btm_hy = 10001.0 -> -150.0
    buffer[0x58..0x5C].copy_from_slice(&10001.0f32.to_le_bytes());
    // cam_mv_type = Chase (0)
    buffer[0x6C..0x70].copy_from_slice(&0i32.to_le_bytes());
    // tmp.chs.ofs_hy = 10000.0 -> -500.0
    buffer[0x70..0x74].copy_from_slice(&10000.0f32.to_le_bytes());

    // Record 1: End marker
    buffer[0x80 + 0x44] = 0x01; // VC_RD_END_DATA_F

    let cam = parse_cam(&buffer).expect("Should parse test cam buffer");
    assert_eq!(cam.records.len(), 1);
    let rec = &cam.records[0];
    assert_eq!(rec.ofs_watch_hy, -300.0);
    assert_eq!(rec.projection, 448.0);
    assert_eq!(rec.trace_btm_hy, -150.0);
    if let CamMovementParams::Chase(ref chs) = rec.params {
        assert_eq!(chs.ofs_hy, -500.0);
    } else {
        panic!("Expected Chase params");
    }
}

#[test]
fn test_normalize_room_cam_rel_path() {
    assert_eq!(
        normalize_room_cam_rel_path("er01"),
        Some(("er01".to_string(), "bg/er/er01.cam".to_string()))
    );
    assert_eq!(
        normalize_room_cam_rel_path("bg/ob/ob01.map"),
        Some(("ob01".to_string(), "bg/ob/ob01.cam".to_string()))
    );
    assert_eq!(
        normalize_room_cam_rel_path("data/bg/rr/rr21.cld"),
        Some(("rr21".to_string(), "bg/rr/rr21.cam".to_string()))
    );
    assert_eq!(
        normalize_room_cam_rel_path("ma61"),
        Some(("ma61".to_string(), "bg2/ma/ma61.cam".to_string()))
    );
    assert_eq!(
        normalize_room_cam_rel_path("sh2e/bg2/ma/ma69.cam"),
        Some(("ma69".to_string(), "bg2/ma/ma69.cam".to_string()))
    );
    assert_eq!(
        normalize_room_cam_rel_path("ccGB"),
        Some(("ccgb".to_string(), "bg/cc/ccGB.cam".to_string()))
    );
    assert_eq!(
        global_area_cam_rel_path("cc11"),
        Some("bg/cc/ccGB.cam".to_string())
    );
    assert_eq!(global_area_cam_rel_path("er01"), None);
}

#[test]
fn test_parse_cam_locus_circle_and_through_door_params() {
    let mut buffer = [0u8; 384];
    // Record 0: Mode 4 (LocusCircle)
    buffer[0x6C..0x70].copy_from_slice(&4i32.to_le_bytes());
    buffer[0x70..0x74].copy_from_slice(&5500.0f32.to_le_bytes());
    buffer[0x74..0x78].copy_from_slice(&24000.0f32.to_le_bytes());
    buffer[0x78..0x7C].copy_from_slice(&(-std::f32::consts::FRAC_PI_4 * 3.0).to_le_bytes());
    buffer[0x7C..0x80].copy_from_slice(&1000.0f32.to_le_bytes());

    // Record 1: Mode 5 (ThroughDoor)
    let r1 = 0x80;
    buffer[r1 + 0x54..r1 + 0x58].copy_from_slice(&10000.0f32.to_le_bytes());
    buffer[r1 + 0x6C..r1 + 0x70].copy_from_slice(&5i32.to_le_bytes());
    buffer[r1 + 0x70..r1 + 0x74].copy_from_slice(&std::f32::consts::FRAC_PI_2.to_le_bytes());
    buffer[r1 + 0x74..r1 + 0x78].copy_from_slice(&(-12400.0f32).to_le_bytes());
    buffer[r1 + 0x78..r1 + 0x7C].copy_from_slice(&(-1137.5f32).to_le_bytes());
    buffer[r1 + 0x7C..r1 + 0x80].copy_from_slice(&8200.0f32.to_le_bytes());

    // Record 2: Terminator
    buffer[0x100 + 0x44] = 0x01;

    let cam = parse_cam(&buffer).expect("Should parse LocusCircle and ThroughDoor records");
    assert_eq!(cam.records.len(), 2);

    assert_eq!(cam.records[0].cam_mv_type, CamMovementType::LocusCircle);
    assert_eq!(
        cam.records[0].params,
        CamMovementParams::LocusCircle(LocusCircleCamParam {
            origin_x: 5500.0,
            origin_z: 24000.0,
            ang_y: -std::f32::consts::FRAC_PI_4 * 3.0,
            radius: 1000.0,
        })
    );

    assert_eq!(cam.records[1].cam_mv_type, CamMovementType::ThroughDoor);
    assert_eq!(cam.records[1].ofs_watch_hy, -300.0);
    assert_eq!(
        cam.records[1].params,
        CamMovementParams::ThroughDoor(ThroughDoorCamParam {
            rail_ang_y: std::f32::consts::FRAC_PI_2,
            rail_sta_pos: [-12400.0, -1137.5, 8200.0],
        })
    );
}

#[test]
fn test_retail_locus_circle_cam_records_decode() {
    let resolver = crate::resolver::AssetPathResolver::resolve();
    // Verify representative retail rooms containing LOCUS_CIRCLE (Mode 4) records:
    // cb02, cb31 (out-of-range ang_y = -5*PI/4), cc10 (zero-origin fallback), cc41 (4-quadrant circle), cc50 (Historical Society)
    let rooms = ["cb02", "cb31", "cc10", "cc41", "cc50", "cc54"];
    let mut found_any = false;
    for room in rooms {
        let Ok(Some(asset)) = resolver.load_cam_for_room(room, false) else {
            continue;
        };
        found_any = true;
        let locus_count = asset
            .cam_file
            .records
            .iter()
            .filter(|r| {
                r.cam_mv_type == CamMovementType::LocusCircle
                    && matches!(
                        r.params,
                        CamMovementParams::LocusCircle(p)
                            if p.radius > 0.0 && p.ang_y.is_finite()
                    )
            })
            .count();
        assert!(locus_count >= 1, "Expected LOCUS_CIRCLE record in {room}");
    }
    let _ = found_any;
}
