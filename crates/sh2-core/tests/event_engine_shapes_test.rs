//! Integration tests for PS2 `src/Event/event.c` data structures (`EventListEntry`, `ItemListEntry`,
//! `StageDataHeader`), `EventListElement` bitfield selectors, 10-byte `ev_pos` spatial trigger shapes
//! (`0..=13`), exact boolean look raycast geometry, and Ending stage IDs (`48..=52`).

use std::f32::consts::{FRAC_PI_2, PI};
use std::mem::{offset_of, size_of};

use sh2_core::events::stages_act2::{
    stage_table_act2, Act2StageId, STAGE_END_DOG, STAGE_END_MARIA, STAGE_END_REBIRTH,
    STAGE_END_RECOVERY, STAGE_END_SUICIDE,
};
use sh2_core::types::{
    char_to_float2, char_to_float4, float_to_char2, wrap_angle, EnemyListEntry, EventCamSetData,
    EventDestShapeKind, EventDoorSound, EventListEntry, EventResultKind, EventShapeKind,
    EventTriggerKind, ItemKindEntry, ItemListEntry, ModelListEntry, StageDataHeader, DOOR_SE_TABLE,
    ITEM_KIND_TABLE,
};
use sh2_core::{
    decode_ev_pos_shape, event_check_in, event_check_look, event_check_look_line,
    event_check_look_point, event_position_set, event_result_move_position,
    is_connected_stage_pair, item_check_look_point, player_height_matches_event_y, EvPosShape,
};

fn push_f32(buf: &mut Vec<u8>, v: f32) {
    buf.extend_from_slice(&v.to_le_bytes());
}

fn push_f16(buf: &mut Vec<u8>, v: f32) {
    buf.extend_from_slice(&float_to_char2(v));
}

#[test]
fn ps2_event_struct_sizes_and_bitfields_match_rosetta_stone() {
    assert_eq!(size_of::<StageDataHeader>(), 52);
    assert_eq!(size_of::<EventListEntry>(), 16);
    assert_eq!(size_of::<ItemListEntry>(), 16);
    assert_eq!(size_of::<ItemKindEntry>(), 8);
    assert_eq!(size_of::<EventDoorSound>(), 12);
    assert_eq!(size_of::<ModelListEntry>(), 48);
    assert_eq!(size_of::<EnemyListEntry>(), 20);
    assert_eq!(size_of::<EventCamSetData>(), 64);

    // Exact `ItemListEntry` ABI offsets (`SLUS_202.28` `ItemCheckLookPoint` `0x001a93b0` /
    // `ItemListElement` `0x001a9320` / `EventExecItem` `0x001aa260`).
    assert_eq!(offset_of!(ItemListEntry, pos_x), 0x00);
    assert_eq!(offset_of!(ItemListEntry, pos_z), 0x04);
    assert_eq!(offset_of!(ItemListEntry, pos_y), 0x08);
    assert_eq!(offset_of!(ItemListEntry, rot_y), 0x0A);
    assert_eq!(offset_of!(ItemListEntry, flag), 0x0C);

    // Exact `CharToFloat2` (`SLUS_202.28` `0x001a9280`, `sh2pc.exe` `0x0048b400`) golden vectors.
    assert_eq!(char_to_float2([0x14, 0x61]), 650.0);
    assert!((char_to_float2(float_to_char2(-500.0)) - (-500.0)).abs() < 1e-3);
    assert!((char_to_float2([0x00, 0x80]) - (-3.051_757_8e-5)).abs() < 1e-10);
    assert_eq!(char_to_float4(123.5f32.to_le_bytes()), 123.5);
    assert!(
        (wrap_angle(3.0 * PI) - (-PI)).abs() < 1e-5 || (wrap_angle(3.0 * PI) - PI).abs() < 1e-5
    );

    let entry = EventListEntry {
        flag: (1 << 31) | (1 << 30) | (283 << 16) | (1 << 14) | 648,
        cond: (2 << 28) | (0x120 << 16) | (3 << 12) | (28 << 4) | (1 << 3) | (1 << 2),
        rslt0: (6 << 28) | (0x240 << 16) | (4 << 12) | (7 << 5) | (1 << 4),
        rslt1: (11 << 22) | (5 << 14) | 649,
    };

    assert_eq!(entry.element(1), 1);
    assert_eq!(entry.element(2), 1);
    assert_eq!(entry.element(3), 283);
    assert_eq!(entry.element(4), 0);
    assert_eq!(entry.element(5), 1);
    assert_eq!(entry.element(6), 648);
    assert_eq!(entry.element(7), 2);
    assert_eq!(entry.element(8), 0x120);
    assert_eq!(entry.element(9), 3);
    assert_eq!(entry.element(10), 28);
    assert_eq!(entry.element(11), 1);
    assert_eq!(entry.element(12), 1);
    assert_eq!(entry.element(13), 6);
    assert_eq!(entry.element(14), 0x240);
    assert_eq!(entry.element(15), 4);
    assert_eq!(entry.element(16), 7);
    assert_eq!(entry.element(17), 1);
    assert_eq!(entry.element(18), 11);
    assert_eq!(entry.element(19), 5);
    assert_eq!(entry.element(20), 5);
    assert_eq!(entry.element(21), 5);
    assert_eq!(entry.element(22), 649);

    assert_eq!(entry.trigger_kind(), Some(EventTriggerKind::ActionLook));
    // Cardinal shape `3` = `-Z`, dest shape `4` = `+Z` (`SLUS_202.28` `0x001a8780` & `0x001a9730`).
    assert_eq!(entry.shape_kind(), Some(EventShapeKind::LineNegZ));
    assert_eq!(entry.result_kind(), Some(EventResultKind::MovePlusProgram));
    assert_eq!(
        entry.dest_shape_kind(),
        Some(EventDestShapeKind::DoorLinePosZ)
    );

    // Exact `ev_pos` record byte lengths (`SLUS_202.28` `0x001a8780`, `0x001a8d70`).
    assert_eq!(EventShapeKind::Point.byte_len(), 10);
    assert_eq!(EventShapeKind::LinePosX.byte_len(), 12);
    assert_eq!(EventShapeKind::LineNegX.byte_len(), 12);
    assert_eq!(EventShapeKind::LineNegZ.byte_len(), 12);
    assert_eq!(EventShapeKind::LinePosZ.byte_len(), 12);
    assert_eq!(EventShapeKind::Polyline2.byte_len(), 14);
    assert_eq!(EventShapeKind::Polyline3.byte_len(), 18);
    assert_eq!(EventShapeKind::Polyline7.byte_len(), 34);
    assert_eq!(EventShapeKind::Rectangle.byte_len(), 14);
    assert_eq!(EventShapeKind::Triangle.byte_len(), 18);
    assert_eq!(EventShapeKind::Quad.byte_len(), 22);

    assert_eq!(DOOR_SE_TABLE[1].open, 11000);
    assert_eq!(DOOR_SE_TABLE[1].lock, 11002);
    assert_eq!(ITEM_KIND_TABLE[0].item_id, 1);
    assert_eq!(ITEM_KIND_TABLE[0].chr_id, 0x0401);
    assert!(is_connected_stage_pair(6, 7));
    assert!(is_connected_stage_pair(11, 10));
    assert!(!is_connected_stage_pair(6, 11));
}

#[test]
fn spatial_trigger_geometry_look_point_line_and_polygons() {
    // `EventCheckLookPoint` (`0x001a8b00`): boolean 400.0 mm radius, 30° (0.5235988 rad) cone.
    assert!(event_check_look_point(0.0, 0.0, 0.0, 0.0, 350.0));
    assert!(event_check_look_point(0.0, 0.0, 0.0, 0.0, 400.0));
    assert!(!event_check_look_point(0.0, 0.0, 0.0, 0.0, 400.1));
    assert!(!event_check_look_point(0.0, 0.0, FRAC_PI_2, 0.0, 300.0));
    // Cone boundary: apex at (0, -50), target at 29° vs 31° from apex within 400 mm.
    let dist_from_apex = 300.0f32;
    let a_in = 0.50f32; // < 0.5235988 rad
    assert!(event_check_look_point(
        0.0,
        0.0,
        0.0,
        dist_from_apex * a_in.sin(),
        -50.0 + dist_from_apex * a_in.cos()
    ));
    let a_out = 0.55f32; // > 0.5235988 rad
    assert!(!event_check_look_point(
        0.0,
        0.0,
        0.0,
        dist_from_apex * a_out.sin(),
        -50.0 + dist_from_apex * a_out.cos()
    ));

    // `EventCheckLookLine` (`0x001a8c80` / `0x0048b0c0`): 400 mm forward ray vs directed segment.
    // Segment along +X from (-500, 300) -> (500, 300): front side is -Z looking +Z (`rot_y = 0`).
    assert!(event_check_look_line(
        0.0, 0.0, 0.0, 500.0, 300.0, -500.0, 300.0
    ));
    // Beyond 400 mm ray length -> miss.
    assert!(!event_check_look_line(
        0.0, 0.0, 0.0, 500.0, 450.0, -500.0, 450.0
    ));
    // Opposite side looking back (PI) -> miss (directed segment).
    assert!(!event_check_look_line(
        0.0, 600.0, PI, 500.0, 300.0, -500.0, 300.0
    ));
    // Collinear non-overlapping segment ahead of 400 mm ray -> rejected by retail AABB check.
    assert!(!event_check_look_line(
        0.0, 0.0, 0.0, 0.0, 500.0, 0.0, 900.0
    ));

    // `ItemCheckLookPoint` (`0x001a93b0`): strict [-750.0, +100.0] height (NO 45000 bypass).
    let make_item = |x: f32, y: f32, z: f32| ItemListEntry {
        pos_x: x,
        pos_z: z,
        pos_y: u16::from_le_bytes(float_to_char2(y)),
        rot_y: 0,
        flag: (1 << 29) | 500,
    };
    assert!(item_check_look_point(
        &make_item(0.0, -750.0, 250.0),
        [0.0, 0.0, 0.0],
        0.0
    ));
    assert!(item_check_look_point(
        &make_item(0.0, 100.0, 250.0),
        [0.0, 0.0, 0.0],
        0.0
    ));
    assert!(!item_check_look_point(
        &make_item(0.0, -756.0, 250.0),
        [0.0, 0.0, 0.0],
        0.0
    ));
    assert!(!item_check_look_point(
        &make_item(0.0, 104.0, 250.0),
        [0.0, 0.0, 0.0],
        0.0
    ));
    // `ItemCheckLookPoint` must NOT bypass height check at 45000.0!
    assert!(!item_check_look_point(
        &make_item(0.0, 45_000.0, 250.0),
        [0.0, 0.0, 0.0],
        0.0
    ));

    assert!(player_height_matches_event_y(0.0, -500.0));
    assert!(player_height_matches_event_y(0.0, 100.0));
    assert!(!player_height_matches_event_y(0.0, -501.0));
    assert!(!player_height_matches_event_y(0.0, 101.0));
    assert!(player_height_matches_event_y(0.0, 45_000.0));

    let mut ev_pos = Vec::new();
    // Offset 0: Rectangle at (100, 0, 200) with w=300, d=400 (14 bytes).
    push_f32(&mut ev_pos, 100.0);
    push_f16(&mut ev_pos, 0.0);
    push_f32(&mut ev_pos, 200.0);
    push_f16(&mut ev_pos, 300.0);
    push_f16(&mut ev_pos, 400.0);

    // Offset 14: Triangle (100, 200) + rel (0, 1000), (1000, 0) (CCW in XZ for shOuterXZ >= 0) (18 bytes).
    push_f32(&mut ev_pos, 100.0);
    push_f16(&mut ev_pos, 0.0);
    push_f32(&mut ev_pos, 200.0);
    push_f16(&mut ev_pos, 0.0);
    push_f16(&mut ev_pos, 1000.0);
    push_f16(&mut ev_pos, 1000.0);
    push_f16(&mut ev_pos, 0.0);

    // Offset 32: Quad (100, 200) + rel (0, 1000), (1000, 1000), (1000, 0) (22 bytes).
    push_f32(&mut ev_pos, 100.0);
    push_f16(&mut ev_pos, 0.0);
    push_f32(&mut ev_pos, 200.0);
    push_f16(&mut ev_pos, 0.0);
    push_f16(&mut ev_pos, 1000.0);
    push_f16(&mut ev_pos, 1000.0);
    push_f16(&mut ev_pos, 1000.0);
    push_f16(&mut ev_pos, 1000.0);
    push_f16(&mut ev_pos, 0.0);

    assert!(event_check_in(
        &ev_pos,
        0,
        EventShapeKind::Rectangle,
        [250.0, 0.0, 400.0]
    ));
    assert!(!event_check_in(
        &ev_pos,
        0,
        EventShapeKind::Rectangle,
        [50.0, 0.0, 400.0]
    ));

    assert!(event_check_in(
        &ev_pos,
        14,
        EventShapeKind::Triangle,
        [300.0, 0.0, 400.0]
    ));
    assert!(!event_check_in(
        &ev_pos,
        14,
        EventShapeKind::Triangle,
        [900.0, 0.0, 1000.0]
    ));

    assert!(event_check_in(
        &ev_pos,
        32,
        EventShapeKind::Quad,
        [900.0, 0.0, 1000.0]
    ));
    assert!(!event_check_in(
        &ev_pos,
        32,
        EventShapeKind::Quad,
        [1300.0, 0.0, 1000.0]
    ));

    // Polyline3 with relative (dx, dz) deltas added to (x0=1000, z0=2000).
    let mut poly_pos = Vec::new();
    push_f32(&mut poly_pos, 1000.0);
    push_f16(&mut poly_pos, 0.0);
    push_f32(&mut poly_pos, 2000.0);
    push_f16(&mut poly_pos, -1000.0);
    push_f16(&mut poly_pos, 0.0);
    push_f16(&mut poly_pos, -1000.0);
    push_f16(&mut poly_pos, 1000.0);
    let decoded = decode_ev_pos_shape(&poly_pos, 0, EventShapeKind::Polyline3).unwrap();
    assert_eq!(
        decoded,
        EvPosShape::Polyline {
            y: char_to_float2(float_to_char2(0.0)),
            vertices: vec![[1000.0, 2000.0], [0.0, 2000.0], [0.0, 3000.0]],
        }
    );
    assert!(event_check_look(
        &poly_pos,
        0,
        EventShapeKind::Polyline3,
        [500.0, 0.0, 1750.0],
        0.0,
    ));

    // Cardinal axes `1=+X, 2=-X, 3=-Z, 4=+Z` (`EventCheckLook` `0x001a8780`, `EventPositionSet` `0x001a9570`).
    let mut line_pos = Vec::new();
    push_f32(&mut line_pos, 1000.0);
    push_f16(&mut line_pos, 0.0);
    push_f32(&mut line_pos, 2000.0);
    push_f16(&mut line_pos, 800.0);

    let y0 = char_to_float2(float_to_char2(0.0));
    assert_eq!(
        decode_ev_pos_shape(&line_pos, 0, EventShapeKind::LinePosX),
        Some(EvPosShape::Polyline {
            y: y0,
            vertices: vec![[1000.0, 2000.0], [1800.0, 2000.0]],
        })
    );
    assert_eq!(
        decode_ev_pos_shape(&line_pos, 0, EventShapeKind::LineNegX),
        Some(EvPosShape::Polyline {
            y: y0,
            vertices: vec![[1000.0, 2000.0], [200.0, 2000.0]],
        })
    );
    assert_eq!(
        decode_ev_pos_shape(&line_pos, 0, EventShapeKind::LineNegZ),
        Some(EvPosShape::Polyline {
            y: y0,
            vertices: vec![[1000.0, 2000.0], [1000.0, 1200.0]],
        })
    );
    assert_eq!(
        decode_ev_pos_shape(&line_pos, 0, EventShapeKind::LinePosZ),
        Some(EvPosShape::Polyline {
            y: y0,
            vertices: vec![[1000.0, 2000.0], [1000.0, 2800.0]],
        })
    );

    assert_eq!(
        event_position_set(&line_pos, 0, EventShapeKind::LinePosX),
        Some([1400.0, y0, 2000.0])
    );
    assert_eq!(
        event_position_set(&line_pos, 0, EventShapeKind::LineNegZ),
        Some([1000.0, y0, 1600.0])
    );

    // `EventResultMovePosition` (`0x001a9730`):
    // Case 0 (`Point`): preserves James's current `rot_y`!
    let (pt_pos, pt_rot) =
        event_result_move_position(&line_pos, 0, EventDestShapeKind::Point, 1.234).unwrap();
    assert_eq!(pt_pos, [1000.0, y0, 2000.0]);
    assert!((pt_rot - 1.234).abs() < 1e-5);

    // Case 3 (`DoorLineNegZ`, `0x001a9890`): x = x0 + 250, z = z0 - len/2, rot_y = +PI/2.
    let (dest_p, dest_rot) =
        event_result_move_position(&line_pos, 0, EventDestShapeKind::DoorLineNegZ, 0.0).unwrap();
    assert_eq!(dest_p, [1250.0, y0, 1600.0]);
    assert!((dest_rot - FRAC_PI_2).abs() < 1e-5);
}

#[test]
fn ending_stage_ids_match_slus_and_sh2pc_48_to_52() {
    // Verified 1:1 against `SLUS_202.28` `StgOverlayGetFileID` (`0x00228300` cases `0x30..=0x34`)
    // and `sh2pc.exe` `StgOverlayGetStageData` (`0x00494ec0` / jump table `0x00495048`).
    assert_eq!(Act2StageId::EndingLeave.as_u16(), 48);
    assert_eq!(Act2StageId::EndingMaria.as_u16(), 49);
    assert_eq!(Act2StageId::EndingInWater.as_u16(), 50);
    assert_eq!(Act2StageId::EndingRebirth.as_u16(), 51);
    assert_eq!(Act2StageId::EndingDog.as_u16(), 52);

    assert_eq!(stage_table_act2(48), Some(&STAGE_END_RECOVERY));
    assert_eq!(stage_table_act2(49), Some(&STAGE_END_MARIA));
    assert_eq!(stage_table_act2(50), Some(&STAGE_END_SUICIDE));
    assert_eq!(stage_table_act2(51), Some(&STAGE_END_REBIRTH));
    assert_eq!(stage_table_act2(52), Some(&STAGE_END_DOG));

    assert_eq!(STAGE_END_RECOVERY.name, "stage_end_recovery");
    assert_eq!(STAGE_END_MARIA.name, "stage_end_maria");
    assert_eq!(STAGE_END_SUICIDE.name, "stage_end_suicide");
    assert_eq!(STAGE_END_REBIRTH.name, "stage_end_rebirth");
    assert_eq!(STAGE_END_DOG.name, "stage_end_dog");
}
