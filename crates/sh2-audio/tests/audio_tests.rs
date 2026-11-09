//! Modular integration test harness for `sh2-audio`.
//!
//! Uses the single-binary submodule pattern (`tests/audio_tests.rs` + `tests/suite/*.rs`)
//! so Cargo compiles and links a single test binary while keeping each audio subsystem's
//! test suite isolated, maintainable, and non-monolithic.
//!
//! Hardware-device tests (`engine_hardware`) are gated on `#[cfg(not(target_os = "windows"))]`
//! while all pure DSP, looping, room acoustics, lock-free SPSC ringbuffer, BGM, SFX, and
//! beta/cut audio tests run across all host platforms (Linux, macOS, and Windows).

#[path = "suite/common.rs"]
mod common;

#[path = "suite/acoustics_and_ringbuffer.rs"]
mod acoustics_and_ringbuffer;
#[path = "suite/beta_cut_audio.rs"]
mod beta_cut_audio;
#[path = "suite/bgm_parity.rs"]
mod bgm_parity;
#[path = "suite/dsp_and_looping.rs"]
mod dsp_and_looping;
#[cfg(not(target_os = "windows"))]
#[path = "suite/engine_hardware.rs"]
mod engine_hardware;
#[path = "suite/sfx_and_footsteps.rs"]
mod sfx_and_footsteps;
