//! Integration tests for PS2 `src/Event/event.c` data structures (`EventListEntry`, `ItemListEntry`,
//! `StageDataHeader`), `EventListElement` bitfield selectors, and `ev_pos` spatial trigger shapes (`0..=13`).

use std::f32::consts::{FRAC_PI_2, PI};
use std::mem::size_of;

use sh2_core::types::{
    char_to_float2, char_to_float4, wrap_angle, EnemyListEntry, EventCamSetData,
    EventDestShapeKind, EventDoorSound, EventListEntry, EventResultKind, EventShapeKind,
    EventTriggerKind, ItemKindEntry, ItemListEntry, ModelListEntry, StageDataHeader, DOOR_SE_TABLE,
    ITEM_KIND_TABLE,
};
use sh2_core::{
    decode_ev_pos_shape, event_check_in, event_check_look, event_check_look_line,
    event_check_look_point, event_position_set, event_result_move_position,
    is_connected_stage_pair, player_height_matches_event_y, EvPosShape,
};

fn push_i16(buf: &mut Vec<u8>, v: i16) {
    buf.extend_from_slice(&v.to_le_bytes());
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

    assert_eq!(char_to_float2((-500i16).to_le_bytes()), -500.0);
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
    assert_eq!(entry.shape_kind(), Some(EventShapeKind::LinePosZ));
    assert_eq!(entry.result_kind(), Some(EventResultKind::MovePlusProgram));
    assert_eq!(
        entry.dest_shape_kind(),
        Some(EventDestShapeKind::DoorLineNegX)
    );

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
    let w_center = event_check_look_point(0.0, 0.0, 0.0, 0.0, 500.0, 1000.0);
    assert!((w_center - 0.5).abs() < 1e-4);

    assert_eq!(
        event_check_look_point(0.0, 0.0, FRAC_PI_2, 0.0, 500.0, 1000.0),
        0.0
    );

    let w_line = event_check_look_line(0.0, 500.0, 0.0, -500.0, 1000.0, 500.0, 1000.0, 1000.0);
    assert!((w_line - 0.5).abs() < 1e-4);
    let w_back = event_check_look_line(0.0, 1500.0, PI, -500.0, 1000.0, 500.0, 1000.0, 1000.0);
    assert_eq!(w_back, 0.0);

    assert!(player_height_matches_event_y(0.0, -500.0));
    assert!(player_height_matches_event_y(0.0, 100.0));
    assert!(!player_height_matches_event_y(0.0, -501.0));
    assert!(!player_height_matches_event_y(0.0, 101.0));
    assert!(player_height_matches_event_y(0.0, 45_000.0));

    let mut ev_pos = Vec::new();
    // Offset 0: Rectangle at (100, 0, 200) with w=300, d=400 (10 bytes)
    push_i16(&mut ev_pos, 100);
    push_i16(&mut ev_pos, 0);
    push_i16(&mut ev_pos, 200);
    push_i16(&mut ev_pos, 300);
    push_i16(&mut ev_pos, 400);

    // Offset 10: Triangle (0, 0)-(1000, 0)-(0, 1000) at y = 0 (14 bytes)
    push_i16(&mut ev_pos, 0);
    push_i16(&mut ev_pos, 0);
    push_i16(&mut ev_pos, 0);
    push_i16(&mut ev_pos, 1000);
    push_i16(&mut ev_pos, 0);
    push_i16(&mut ev_pos, 0);
    push_i16(&mut ev_pos, 1000);

    // Offset 24: Quad (0, 0)-(1000, 0)-(1000, 1000)-(0, 1000) at y = 0 (18 bytes)
    push_i16(&mut ev_pos, 0);
    push_i16(&mut ev_pos, 0);
    push_i16(&mut ev_pos, 0);
    push_i16(&mut ev_pos, 1000);
    push_i16(&mut ev_pos, 0);
    push_i16(&mut ev_pos, 1000);
    push_i16(&mut ev_pos, 1000);
    push_i16(&mut ev_pos, 0);
    push_i16(&mut ev_pos, 1000);

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
        10,
        EventShapeKind::Triangle,
        [200.0, 0.0, 200.0]
    ));
    assert!(!event_check_in(
        &ev_pos,
        10,
        EventShapeKind::Triangle,
        [800.0, 0.0, 800.0]
    ));

    assert!(event_check_in(
        &ev_pos,
        24,
        EventShapeKind::Quad,
        [800.0, 0.0, 800.0]
    ));
    assert!(!event_check_in(
        &ev_pos,
        24,
        EventShapeKind::Quad,
        [1200.0, 0.0, 800.0]
    ));

    let mut poly_pos = Vec::new();
    push_i16(&mut poly_pos, 0);
    push_i16(&mut poly_pos, 0);
    push_i16(&mut poly_pos, 0);
    push_i16(&mut poly_pos, 600);
    push_i16(&mut poly_pos, 1000);
    push_i16(&mut poly_pos, 0);
    push_i16(&mut poly_pos, 1000);
    push_i16(&mut poly_pos, 1000);
    let decoded = decode_ev_pos_shape(&poly_pos, 0, EventShapeKind::Polyline3).unwrap();
    assert_eq!(
        decoded,
        EvPosShape::Polyline {
            y: 0.0,
            radius: 600.0,
            vertices: vec![[0.0, 0.0], [1000.0, 0.0], [1000.0, 1000.0]],
        }
    );
    let w_poly = event_check_look(
        &poly_pos,
        0,
        EventShapeKind::Polyline3,
        [500.0, 0.0, -300.0],
        0.0,
        true,
    );
    assert!((w_poly - 0.5).abs() < 1e-4);

    let mut move_pos = Vec::new();
    push_i16(&mut move_pos, 1000);
    push_i16(&mut move_pos, 0);
    push_i16(&mut move_pos, 2000);
    push_i16(&mut move_pos, 500);
    push_i16(&mut move_pos, 800);
    assert_eq!(
        event_position_set(&move_pos, 0, EventShapeKind::LinePosZ),
        Some([1000.0, 0.0, 2400.0])
    );
    let (dest_p, dest_rot) =
        event_result_move_position(&move_pos, 0, EventDestShapeKind::DoorLinePosZ).unwrap();
    assert_eq!(dest_p, [1250.0, 0.0, 2400.0]);
    assert!((dest_rot - FRAC_PI_2).abs() < 1e-5);
}
