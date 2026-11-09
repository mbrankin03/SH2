//! `.cam` volume dynamic room acoustics modeling and lock-free SPSC `AudioRingBuffer` tests (`SH2-55`).

use glam::Vec3;
use sh2_audio::*;

#[test]
fn test_sh2_55_dynamic_room_acoustics_from_cam_volumes() {
    use sh2_assets::{
        AreaSizeType, CamFile, CamMovementParams, CamMovementType, RoadArea, RoadRecord, RoadType,
    };

    let make_record =
        |x_span: f32, z_span: f32, y_span: f32, size: AreaSizeType, rd_type: RoadType| RoadRecord {
            lim_sw: RoadArea {
                x0: 0.0,
                z0: 0.0,
                x1: x_span,
                z1: z_span,
                x2: x_span,
                z2: z_span,
                min_hy: 0.0,
                max_hy: -y_span,
            },
            lim_rd: RoadArea {
                x0: 0.0,
                z0: 0.0,
                x1: x_span,
                z1: z_span,
                x2: x_span,
                z2: z_span,
                min_hy: 0.0,
                max_hy: -y_span,
            },
            kind_id: 0,
            flags: 0,
            area_size_type: size,
            rd_type,
            mv_y_type: 0,
            ofs_watch_hy: -300.0,
            trace_btm_hy: -300.0,
            rd_dir_type: 0,
            projection: 448.0,
            proj_volume: 0.0,
            proj_sec: 0.5,
            cam_mv_type: CamMovementType::Chase,
            params: CamMovementParams::default(),
        };

    // 1. Outdoor street volume (`AreaSizeType::Outdoor`) -> `EnvironmentalPreset::FoggyStreets`
    let rec_outdoor = make_record(
        12000.0,
        10000.0,
        3000.0,
        AreaSizeType::Outdoor,
        RoadType::Road,
    );
    let prof_outdoor = evaluate_room_acoustics(Some(&rec_outdoor), false);
    assert_eq!(prof_outdoor.preset, EnvironmentalPreset::FoggyStreets);
    assert!((prof_outdoor.damping - 0.70).abs() < 1e-4);

    // 2. Elongated narrow apartment hallway (`1600 x 8000`, aspect = 5.0) -> `EnvironmentalPreset::Corridor`
    let rec_corridor = make_record(1600.0, 8000.0, 2600.0, AreaSizeType::Tiny, RoadType::Road);
    let prof_corridor = evaluate_room_acoustics(Some(&rec_corridor), true);
    assert_eq!(prof_corridor.preset, EnvironmentalPreset::Corridor);
    assert!(prof_corridor.feedback >= 0.72);
    assert!(prof_corridor.spatial_spread < 0.80);

    // 3. Standard square interior room (`3000 x 3200`, `AreaSizeType::Small`) -> `EnvironmentalPreset::InteriorRoom`
    let rec_room = make_record(3000.0, 3200.0, 2600.0, AreaSizeType::Small, RoadType::Road);
    let prof_room = evaluate_room_acoustics(Some(&rec_room), true);
    assert_eq!(prof_room.preset, EnvironmentalPreset::InteriorRoom);

    // 4. Wide interior prison/labyrinth chamber (`AreaSizeType::Wide`) -> `EnvironmentalPreset::CavernousEcho`
    let rec_cavern = make_record(9000.0, 9000.0, 6000.0, AreaSizeType::Wide, RoadType::Effect);
    let prof_cavern = evaluate_room_acoustics(Some(&rec_cavern), true);
    assert_eq!(prof_cavern.preset, EnvironmentalPreset::CavernousEcho);
    assert!(prof_cavern.wet_mix >= 0.55);

    // 5. Evaluate directly via `CamFile` at player position
    let cam = CamFile {
        records: vec![rec_corridor],
    };
    let prof_pos = evaluate_room_acoustics_for_pos(&cam, Vec3::new(800.0, -1000.0, 4000.0), true);
    assert_eq!(prof_pos.preset, EnvironmentalPreset::Corridor);
}

#[test]
fn test_sh2_55_lock_free_audio_ringbuffer_multithreaded_spsc() {
    use std::sync::Arc;

    let rb = Arc::new(AudioRingBuffer::with_capacity(1024));
    assert_eq!(rb.capacity(), 1024);
    assert_eq!(rb.available_read(), 0);
    assert_eq!(rb.available_write(), 1024);
    assert!(rb.needs_refill());

    let total_frames_to_stream = 16_384usize;
    let rb_producer = Arc::clone(&rb);

    let producer = std::thread::spawn(move || {
        let mut sent = 0usize;
        let mut batch = [kira::Frame::new(0.0, 0.0); 128];
        while sent < total_frames_to_stream {
            let n = (total_frames_to_stream - sent).min(batch.len());
            for (i, slot) in batch.iter_mut().take(n).enumerate() {
                let idx = (sent + i) as f32;
                *slot = kira::Frame::new(idx, -idx);
            }
            let written = rb_producer.push_frames(&batch[..n]);
            sent += written;
            if written == 0 {
                std::thread::yield_now();
            }
        }
    });

    let mut received = 0usize;
    let mut out_buf = [kira::Frame::new(0.0, 0.0); 96];
    while received < total_frames_to_stream {
        let avail = rb.available_read();
        if avail == 0 {
            std::thread::yield_now();
            continue;
        }
        let to_pop = avail.min(out_buf.len());
        let popped = rb.pop_frames(&mut out_buf[..to_pop]);
        for (i, frame) in out_buf.iter().take(popped).enumerate() {
            let expected = (received + i) as f32;
            assert_eq!(frame.left, expected);
            assert_eq!(frame.right, -expected);
        }
        received += popped;
    }

    producer
        .join()
        .expect("Producer thread should complete cleanly");
    assert_eq!(rb.available_read(), 0);
    assert_eq!(rb.underrun_count(), 0);
}
