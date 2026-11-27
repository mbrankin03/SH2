//! Streaming ADX/AIX multi-layer BGM and ringbuffer-backed FMV audio integration tests (`SH2-180`).

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use kira::manager::backend::mock::{MockBackend, MockBackendSettings};
use kira::manager::{AudioManager, AudioManagerSettings};
use kira::track::TrackBuilder;
use kira::Frame;
use sh2_audio::{
    StreamingBgmSoundData, StreamingBgmSource, StreamingFmvSoundData, StreamingResampler48k,
};

#[test]
fn test_sh2_180_streaming_resampler_48k_passthrough_and_upsample() {
    // 1. 48 kHz SH2:EE stream passes through bit-identically
    let mut pass = StreamingResampler48k::new(48_000, 0.78);
    assert_eq!(pass.output_sample_rate(), 48_000);
    let input = [(0.25f32, -0.5f32), (0.75, 0.125)];
    let mut out = Vec::new();
    pass.process_chunk(&input, &mut out);
    assert_eq!(out.len(), 2);
    assert_eq!(out[0], Frame::new(0.25, -0.5));
    assert_eq!(out[1], Frame::new(0.75, 0.125));

    // 2. 24 kHz PS2/PC stream upsamples 2x to 48 kHz and attenuates Nyquist-adjacent fizz
    let mut up = StreamingResampler48k::new(24_000, 0.78);
    assert_eq!(up.output_sample_rate(), 48_000);
    let n = 1200usize;
    let mut harsh = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f32 / 24_000.0;
        let v = (t * 10_000.0 * 2.0 * std::f32::consts::PI).sin() * 0.8;
        harsh.push((v, v));
    }
    let mut up_out = Vec::new();
    up.process_chunk(&harsh, &mut up_out);
    assert!(
        (up_out.len() as isize - (n * 2) as isize).abs() <= 4,
        "Expected ~2x upsampled frames (~{}), got {}",
        n * 2,
        up_out.len()
    );
    let rms = (up_out[100..].iter().map(|f| f.left * f.left).sum::<f32>()
        / (up_out.len() - 100) as f32)
        .sqrt();
    assert!(
        rms < 0.20,
        "Expected 10 kHz fizz attenuated below 0.20 RMS, got {rms}"
    );
}

#[test]
fn test_sh2_180_streaming_bgm_multilayer_lockstep_and_stop() {
    let mut manager = AudioManager::<MockBackend>::new(AudioManagerSettings {
        backend_settings: MockBackendSettings {
            sample_rate: 48_000,
        },
        ..Default::default()
    })
    .expect("mock AudioManager");
    let bgm_track = manager
        .add_sub_track(TrackBuilder::default())
        .expect("bgm sub-track");

    // 3 synchronized layers: layer 0 = +0.20, layer 1 = +0.40, layer 2 = -0.10
    let emitted = Arc::new(AtomicUsize::new(0));
    let emitted_clone = Arc::clone(&emitted);
    let source = StreamingBgmSource::Custom {
        sample_rate: 48_000,
        layer_count: 3,
        read_fn: Box::new(move |out_layers, max_frames| {
            let vals = [(0.20f32, 0.20f32), (0.40, 0.40), (-0.10, -0.10)];
            for (k, &v) in vals.iter().enumerate() {
                out_layers[k].clear();
                out_layers[k].resize(max_frames, v);
            }
            emitted_clone.fetch_add(max_frames, Ordering::Relaxed);
            max_frames
        }),
    };

    let sound_data = StreamingBgmSoundData {
        source,
        initial_gains: vec![1.0, 0.5, 0.0], // expected mix: 0.20*1.0 + 0.40*0.5 + (-0.10)*0.0 = 0.40
        initial_master_gain: 1.0,
        target_master_gain: 1.0,
        fade_in: Duration::ZERO,
        destination: bgm_track.id(),
    };

    let handle = manager.play(sound_data).expect("play streaming bgm");
    assert_eq!(handle.layer_count(), 3);

    let backend = manager.backend_mut();
    backend.on_start_processing();
    let mut settled = Frame::ZERO;
    for _ in 0..32 {
        settled = backend.process();
    }
    assert!(
        (settled.left - 0.40).abs() < 1e-3 && (settled.right - 0.40).abs() < 1e-3,
        "Expected synchronized 3-layer mix ~0.40, got {:?}",
        settled
    );

    // Enable layer 2 at gain 1.0 immediately -> 0.40 - 0.10 = 0.30
    handle.set_layer_gain(2, 1.0, Duration::ZERO);
    let mut updated = Frame::ZERO;
    for _ in 0..16 {
        updated = backend.process();
    }
    assert!(
        (updated.left - 0.30).abs() < 1e-3,
        "Expected updated 3-layer mix ~0.30, got {:?}",
        updated
    );

    // Pause outputs silence
    handle.set_paused(true);
    let paused_frame = backend.process();
    assert_eq!(paused_frame, Frame::ZERO);
    handle.set_paused(false);

    // Stop with short fade terminates cleanly
    handle.stop(Duration::from_millis(2));
    for _ in 0..256 {
        let _ = backend.process();
    }
    assert!(
        handle.is_finished(),
        "Streaming BGM handle should report finished after stop fade completes"
    );
}

#[test]
fn test_sh2_180_streaming_fmv_ringbuffer_eof_and_stop() {
    let mut manager = AudioManager::<MockBackend>::new(AudioManagerSettings {
        backend_settings: MockBackendSettings {
            sample_rate: 48_000,
        },
        ..Default::default()
    })
    .expect("mock AudioManager");
    let fmv_track = manager
        .add_sub_track(TrackBuilder::default())
        .expect("fmv sub-track");

    // Emit exactly 2048 stereo frames of constant (0.5, -0.25) then EOF
    let remaining = Arc::new(AtomicUsize::new(2048));
    let rem_clone = Arc::clone(&remaining);
    let sound_data =
        StreamingFmvSoundData::from_custom_producer(48_000, 0.5, fmv_track.id(), move |out| {
            let left = rem_clone.load(Ordering::Relaxed);
            if left == 0 {
                return 0;
            }
            let n = left.min(out.len());
            for slot in out.iter_mut().take(n) {
                *slot = (0.5, -0.25);
            }
            rem_clone.fetch_sub(n, Ordering::Relaxed);
            n
        });

    let handle = manager.play(sound_data).expect("play streaming fmv");
    // Give the background reader thread a few ms to push the 2048 frames + EOF into the ringbuffer
    std::thread::sleep(Duration::from_millis(20));

    let backend = manager.backend_mut();
    backend.on_start_processing();

    let mut saw_expected_signal = false;
    for _ in 0..2600 {
        let f = backend.process();
        if (f.left - 0.25).abs() < 1e-3 && (f.right - (-0.125)).abs() < 1e-3 {
            saw_expected_signal = true;
        }
        if handle.is_finished() {
            break;
        }
    }

    assert!(
        saw_expected_signal,
        "Expected scaled FMV audio frame (0.25, -0.125) from ringbuffer stream"
    );
    assert!(
        handle.is_finished(),
        "Streaming FMV handle should finish cleanly after draining EOF"
    );
}
