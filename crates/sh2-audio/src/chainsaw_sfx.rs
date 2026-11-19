//! Chainsaw Revving Idle / Raise / Attack / Hit Audio Loop State Machine (`ChainsawSoundFix.cpp`).
//!
//! Verified 1:1 against:
//! - Retail PC Binary (`sh2pc.exe` v1.0 Director's Cut):
//!   - `0x00514916`: Chainsaw loop startup (`0x1642`).
//!   - `0x00517924`: DirectSound buffer length & start offset setup (`0x14E8`, `0x1642`).
//!   - `0x00530DAE`: Chainsaw idle start trigger (`0x2B24`).
//!   - `0x005316C7`: Chainsaw attack sound trigger (`0x2B25`).
//!   - `0x005358C3` & `0x00543612`: Chainsaw hit sound triggers (`0x2B26`).
//!   - `0x0054A592`: Chainsaw attack frame `0` check (`weapon_grip == 7`, `anim_speed >= 0`).
//! - Silent Hill 2: Enhanced Edition:
//!   - `Patches/ChainsawSoundFix.cpp` (`RunChainsawSoundFix`, `SilenceChainsawLoopASM`,
//!     `SetChainsawLoopBufferLengthASM`, `SetChainsawLoopBufferStartASM`,
//!     `CheckChainsawAttackStartASM`, `PlayChainsawHitSoundASM`).

use glam::Vec3;

/// Sound effect ID: Chainsaw pull-cord crank startup (`0x14E8` = `5352` in `ChainsawSoundFix.cpp`).
pub const SE_CHAINSAW_CRANK: u32 = 0x14E8;
/// Sound effect ID: Chainsaw idle loop (`0x1642` = `5698`, `sddata.bin[352]` in `ChainsawSoundFix.cpp`).
pub const SE_CHAINSAW_IDLE_LOOP: u32 = 0x1642;
/// Sound effect ID: Chainsaw idle/raise startup burst (`0x2B24` = `11044` in `ChainsawSoundFix.cpp`).
pub const SE_CHAINSAW_IDLE_START: u32 = 0x2B24;
/// Sound effect ID: Chainsaw attack rev (`0x2B25` = `11045` in `ChainsawSoundFix.cpp`).
pub const SE_CHAINSAW_ATTACK: u32 = 0x2B25;
/// Sound effect ID: Chainsaw flesh hit (`0x2B26` = `11046` in `ChainsawSoundFix.cpp`).
pub const SE_CHAINSAW_HIT: u32 = 0x2B26;

/// Standard RIFF/WAV header byte offset (`0x2C` = `44` bytes at `SetChainsawLoopBufferStartASM`).
pub const WAV_RIFF_HEADER_BYTES: usize = 0x2C;
/// 48 kHz sample rate identifier (`48_000` Hz in `SetChainsawLoopBufferLengthASM`).
pub const SAMPLE_RATE_48KHZ: u32 = 48_000;
/// Leading PCM bytes trimmed from `SE_CHAINSAW_CRANK` (`0x14E8`) at 48 kHz when started on James's left side (`0x7CFC`).
pub const CRANK_LEFT_SIDE_TRIM_BYTES_48K: usize = 0x7CFC;
/// Leading PCM bytes trimmed from `SE_CHAINSAW_IDLE_LOOP` (`0x1642`) at 48 kHz (`0x12180`).
pub const IDLE_LOOP_TRIM_BYTES_48K: usize = 0x12180;
/// Leading PCM bytes trimmed from `SE_CHAINSAW_IDLE_LOOP` (`0x1642`) at original PS2/PC sample rate (`0x4796`).
pub const IDLE_LOOP_TRIM_BYTES_ORIGINAL: usize = 0x4796;

/// Hold duration before crossfading from `SE_CHAINSAW_IDLE_START` into `SE_CHAINSAW_IDLE_LOOP` (`1.0` s).
pub const CHAINSAW_IDLE_FADE_START_TIME_SECS: f32 = 1.0;
/// Crossfade duration between `SE_CHAINSAW_IDLE_START` and `SE_CHAINSAW_IDLE_LOOP` (`0.1` s).
pub const CHAINSAW_IDLE_FADE_DURATION_SECS: f32 = 0.1;
/// Minimum weapon volume threshold below which the Chainsaw idle state resets (`1e-5` at `RunChainsawSoundFix`).
pub const CHAINSAW_MIN_ACTIVE_VOLUME: f32 = 1.0e-5;

/// WAV byte-slice adjustment computed by `SetChainsawLoopBufferStartASM` & `SetChainsawLoopBufferLengthASM`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChainsawWavSlice {
    /// Byte offset from the start of the `.wav` buffer (including `0x2C` RIFF header).
    pub pcm_start_byte_offset: usize,
    /// Trimmed PCM payload byte length (`original_pcm_bytes.saturating_sub(trim)`).
    pub pcm_length_bytes: usize,
}

/// Computes the exact PCM start offset and payload byte length for Chainsaw WAV buffers
/// (`SetChainsawLoopBufferLengthASM` & `SetChainsawLoopBufferStartASM` in `ChainsawSoundFix.cpp`).
///
/// When `enable_chainsaw_sound_fix` is `false`, returns the unmodified 1:1 vanilla PC slice
/// (`pcm_start_byte_offset = 0x2C`, `pcm_length_bytes = original_pcm_bytes`).
pub const fn compute_chainsaw_wav_slice(
    sound_id: u32,
    sample_rate_hz: u32,
    original_pcm_bytes: usize,
    left_side_collision_start: bool,
    enable_chainsaw_sound_fix: bool,
) -> ChainsawWavSlice {
    if !enable_chainsaw_sound_fix {
        return ChainsawWavSlice {
            pcm_start_byte_offset: WAV_RIFF_HEADER_BYTES,
            pcm_length_bytes: original_pcm_bytes,
        };
    }

    let trim = match sound_id {
        SE_CHAINSAW_CRANK => {
            if sample_rate_hz == SAMPLE_RATE_48KHZ && left_side_collision_start {
                CRANK_LEFT_SIDE_TRIM_BYTES_48K
            } else {
                0
            }
        }
        SE_CHAINSAW_IDLE_LOOP => {
            if sample_rate_hz == SAMPLE_RATE_48KHZ {
                IDLE_LOOP_TRIM_BYTES_48K
            } else {
                IDLE_LOOP_TRIM_BYTES_ORIGINAL
            }
        }
        _ => 0,
    };

    ChainsawWavSlice {
        pcm_start_byte_offset: WAV_RIFF_HEADER_BYTES + trim,
        pcm_length_bytes: original_pcm_bytes.saturating_sub(trim),
    }
}

/// Explicit state machine for Chainsaw idle audio crossfading (`ChainsawIdleState` `0..=3` in `ChainsawSoundFix.cpp`).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ChainsawIdleState {
    /// Chainsaw unequipped or silent (`state == 0`).
    #[default]
    Inactive,
    /// Waiting `1.0` s (`CHAINSAW_IDLE_FADE_START_TIME_SECS`) on `0x2B24` before fading in `0x1642` (`state == 1`).
    WaitingForLoopPoint {
        /// Remaining countdown in seconds before crossfade starts.
        remaining_secs: f32,
    },
    /// Cubic `0.1` s crossfade from `0x2B24` (`1 - t^3`) to `0x1642` (`1 - (1 - t)^3`) (`state == 2`).
    Crossfading {
        /// Normalized progress `t` in `[0.0, 1.0]`.
        factor: f32,
    },
    /// Steady looping `0x1642`, muted while an attack swing is active (`state == 3`).
    Looping,
}

/// Per-tick input snapshot driving [`ChainsawSoundController::tick`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChainsawTickInput {
    /// Elapsed tick duration in seconds (`1/60` at 60 Hz or `1/30` at 30 Hz).
    pub dt_secs: f32,
    /// James's world position in PS2 mm for spatial panning (`GetJamesPosXPointer()`).
    pub james_pos_mm: Vec3,
    /// Current weapon audio gain (`*WeaponVolumePtr`, `0.0..=1.0`).
    pub weapon_volume: f32,
    /// Whether the Chainsaw attack hitbox/swing is active (`*AttackActivePtr != 0`).
    pub attack_active: bool,
    /// Current animation frame of the Chainsaw clip (`0` at start of swing in `CheckChainsawAttackStartASM`).
    pub current_anim_frame: u32,
    /// Signed animation playback speed (`>= 0` forward swing; `< 0` recoil/reverse ignored at `0x0054A592`).
    pub anim_speed_q11: i16,
    /// Whether the Chainsaw blade connected with an enemy this frame.
    pub hit_this_frame: bool,
    /// `true` uses SH2:EE `ChainsawSoundFix.cpp`; `false` preserves strict 1:1 vanilla PC audio behavior.
    pub enable_chainsaw_sound_fix: bool,
}

/// Audio mixer commands emitted by [`ChainsawSoundController::tick`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChainsawAudioOutput {
    /// Spatial volume for `SE_CHAINSAW_IDLE_START` (`0x2B24`).
    pub idle_start_volume: f32,
    /// Spatial volume for `SE_CHAINSAW_IDLE_LOOP` (`0x1642`).
    pub idle_loop_volume: f32,
    /// `Some(volume)` when `SE_CHAINSAW_ATTACK` (`0x2B25`) should trigger as a directional one-shot.
    pub trigger_attack_sfx_volume: Option<f32>,
    /// `Some(1.0)` when `SE_CHAINSAW_HIT` (`0x2B26`) should trigger on hit impact.
    pub trigger_hit_sfx_volume: Option<f32>,
    /// Spatial source position in PS2 world mm.
    pub world_pos_mm: Vec3,
}

/// Stateful Chainsaw audio loop & one-shot controller (`ChainsawSoundFix.cpp`).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ChainsawSoundController {
    /// Current idle loop fade state (`0..=3`).
    pub idle_state: ChainsawIdleState,
    /// Latched attack window started at frame `0` (`ChainsawAttackActive`).
    pub attack_window_active: bool,
    /// Ensures `SE_CHAINSAW_ATTACK` (`0x2B25`) fires once per attack swing (`ChainsawAttackSoundPlayed`).
    pub attack_sound_played: bool,
    /// Tracks previous frame's hit state (`PlayerLastHitResult == 1` in `PlayChainsawHitSoundASM`).
    pub hit_active_prev_frame: bool,
}

impl ChainsawSoundController {
    /// Creates an inactive Chainsaw sound controller.
    pub const fn new() -> Self {
        Self {
            idle_state: ChainsawIdleState::Inactive,
            attack_window_active: false,
            attack_sound_played: false,
            hit_active_prev_frame: false,
        }
    }

    /// Starts the Chainsaw idle sequence when equipped or raised (`SilenceChainsawLoopASM` at `0x00514916`).
    pub fn start_idle(&mut self, enable_chainsaw_sound_fix: bool) {
        if enable_chainsaw_sound_fix {
            self.idle_state = ChainsawIdleState::WaitingForLoopPoint {
                remaining_secs: CHAINSAW_IDLE_FADE_START_TIME_SECS,
            };
        } else {
            self.idle_state = ChainsawIdleState::Looping;
        }
        self.attack_window_active = false;
        self.attack_sound_played = false;
    }

    /// Resets the Chainsaw audio controller to silent (`ChainsawIdleState = 0`).
    pub fn stop(&mut self) {
        *self = Self::new();
    }

    /// Advances the Chainsaw audio state machine by one tick (`RunChainsawSoundFix` + `PlayChainsawHitSoundASM`).
    pub fn tick(&mut self, input: ChainsawTickInput) -> ChainsawAudioOutput {
        let trigger_hit_sfx_volume = if input.hit_this_frame {
            if !input.enable_chainsaw_sound_fix || !self.hit_active_prev_frame {
                Some(1.0)
            } else {
                None
            }
        } else {
            None
        };
        self.hit_active_prev_frame = input.hit_this_frame;

        if matches!(self.idle_state, ChainsawIdleState::Inactive)
            || input.weapon_volume < CHAINSAW_MIN_ACTIVE_VOLUME
        {
            self.idle_state = ChainsawIdleState::Inactive;
            self.attack_window_active = false;
            self.attack_sound_played = false;
            return ChainsawAudioOutput {
                idle_start_volume: 0.0,
                idle_loop_volume: 0.0,
                trigger_attack_sfx_volume: None,
                trigger_hit_sfx_volume,
                world_pos_mm: input.james_pos_mm,
            };
        }

        if !input.enable_chainsaw_sound_fix {
            let trigger_attack_sfx_volume = if input.attack_active
                && input.current_anim_frame == 0
                && !self.attack_sound_played
            {
                self.attack_sound_played = true;
                Some(input.weapon_volume)
            } else {
                if !input.attack_active {
                    self.attack_sound_played = false;
                }
                None
            };
            return ChainsawAudioOutput {
                idle_start_volume: 0.0,
                idle_loop_volume: input.weapon_volume,
                trigger_attack_sfx_volume,
                trigger_hit_sfx_volume,
                world_pos_mm: input.james_pos_mm,
            };
        }

        // CheckChainsawAttackStartASM: only start on frame 0 when attack_active && anim_speed >= 0
        if input.current_anim_frame == 0 {
            self.attack_window_active = input.attack_active && input.anim_speed_q11 >= 0;
        } else if !input.attack_active {
            self.attack_window_active = false;
        }

        let trigger_attack_sfx_volume = if self.attack_window_active {
            if !self.attack_sound_played {
                self.attack_sound_played = true;
                Some(input.weapon_volume)
            } else {
                None
            }
        } else {
            self.attack_sound_played = false;
            None
        };

        let dt = input.dt_secs.max(0.0);
        let (idle_start_volume, idle_loop_volume) = match self.idle_state {
            ChainsawIdleState::Inactive => (0.0, 0.0),
            ChainsawIdleState::WaitingForLoopPoint { mut remaining_secs } => {
                remaining_secs -= dt;
                if remaining_secs < 0.0 {
                    self.idle_state = ChainsawIdleState::Crossfading { factor: 0.0 };
                } else {
                    self.idle_state = ChainsawIdleState::WaitingForLoopPoint { remaining_secs };
                }
                (input.weapon_volume, 0.0)
            }
            ChainsawIdleState::Crossfading { mut factor } => {
                factor += dt / CHAINSAW_IDLE_FADE_DURATION_SECS;
                if factor >= 1.0 {
                    factor = 1.0;
                    self.idle_state = ChainsawIdleState::Looping;
                } else {
                    self.idle_state = ChainsawIdleState::Crossfading { factor };
                }
                let start_vol = (1.0 - factor.powi(3)) * input.weapon_volume;
                let loop_vol = (1.0 - (1.0 - factor).powi(3)) * input.weapon_volume;
                if matches!(self.idle_state, ChainsawIdleState::Looping) {
                    (0.0, loop_vol)
                } else {
                    (start_vol, loop_vol)
                }
            }
            ChainsawIdleState::Looping => {
                let loop_vol = if input.attack_active {
                    0.0
                } else {
                    input.weapon_volume
                };
                (0.0, loop_vol)
            }
        };

        ChainsawAudioOutput {
            idle_start_volume,
            idle_loop_volume,
            trigger_attack_sfx_volume,
            trigger_hit_sfx_volume,
            world_pos_mm: input.james_pos_mm,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wav_slice_matches_chainsaw_sound_fix_offsets() {
        let loop_48k =
            compute_chainsaw_wav_slice(SE_CHAINSAW_IDLE_LOOP, 48_000, 0x30000, false, true);
        assert_eq!(loop_48k.pcm_start_byte_offset, 0x121AC);
        assert_eq!(loop_48k.pcm_length_bytes, 0x30000 - 0x12180);

        let loop_orig =
            compute_chainsaw_wav_slice(SE_CHAINSAW_IDLE_LOOP, 22_050, 0x10000, false, true);
        assert_eq!(loop_orig.pcm_start_byte_offset, 0x47C2);
        assert_eq!(loop_orig.pcm_length_bytes, 0x10000 - 0x4796);

        let crank_left = compute_chainsaw_wav_slice(SE_CHAINSAW_CRANK, 48_000, 0x20000, true, true);
        assert_eq!(crank_left.pcm_start_byte_offset, 0x7D28);
        assert_eq!(crank_left.pcm_length_bytes, 0x20000 - 0x7CFC);

        let vanilla =
            compute_chainsaw_wav_slice(SE_CHAINSAW_IDLE_LOOP, 48_000, 0x30000, true, false);
        assert_eq!(vanilla.pcm_start_byte_offset, WAV_RIFF_HEADER_BYTES);
        assert_eq!(vanilla.pcm_length_bytes, 0x30000);
    }

    #[test]
    fn chainsaw_state_machine_crossfades_and_debounces_attack_and_hit() {
        let mut ctrl = ChainsawSoundController::new();
        ctrl.start_idle(true);

        let base = ChainsawTickInput {
            dt_secs: 0.6,
            james_pos_mm: Vec3::new(100.0, 0.0, 200.0),
            weapon_volume: 0.8,
            attack_active: false,
            current_anim_frame: 0,
            anim_speed_q11: 2048,
            hit_this_frame: false,
            enable_chainsaw_sound_fix: true,
        };

        let out1 = ctrl.tick(base);
        assert!((out1.idle_start_volume - 0.8).abs() < 1e-5);
        assert_eq!(out1.idle_loop_volume, 0.0);

        // Crosses 1.0s into State 2
        let _ = ctrl.tick(base);
        assert!(matches!(
            ctrl.idle_state,
            ChainsawIdleState::Crossfading { .. }
        ));

        // Crossfade 0.05s -> mid-fade
        let out_mid = ctrl.tick(ChainsawTickInput {
            dt_secs: 0.05,
            ..base
        });
        assert!(out_mid.idle_start_volume > 0.0 && out_mid.idle_start_volume < 0.8);
        assert!(out_mid.idle_loop_volume > 0.0 && out_mid.idle_loop_volume < 0.8);

        // Finish crossfade -> State 3 Looping
        let out_loop = ctrl.tick(ChainsawTickInput {
            dt_secs: 0.06,
            ..base
        });
        assert_eq!(out_loop.idle_start_volume, 0.0);
        assert!((out_loop.idle_loop_volume - 0.8).abs() < 1e-5);

        // Attack swing at frame 0 mutes loop and fires 0x2B25 once; hit fires 0x2B26 only on first frame
        let atk1 = ctrl.tick(ChainsawTickInput {
            dt_secs: 1.0 / 60.0,
            attack_active: true,
            current_anim_frame: 0,
            hit_this_frame: true,
            ..base
        });
        assert_eq!(atk1.trigger_attack_sfx_volume, Some(0.8));
        assert_eq!(atk1.trigger_hit_sfx_volume, Some(1.0));

        let atk2 = ctrl.tick(ChainsawTickInput {
            dt_secs: 1.0 / 60.0,
            attack_active: true,
            current_anim_frame: 1,
            hit_this_frame: true,
            ..base
        });
        assert_eq!(atk2.idle_loop_volume, 0.0);
        assert_eq!(atk2.trigger_attack_sfx_volume, None);
        assert_eq!(atk2.trigger_hit_sfx_volume, None);

        // Reverse/recoil animation (anim_speed < 0) at frame 0 does NOT re-trigger attack SFX
        ctrl.tick(ChainsawTickInput {
            attack_active: false,
            hit_this_frame: false,
            ..base
        });
        let recoil = ctrl.tick(ChainsawTickInput {
            attack_active: true,
            current_anim_frame: 0,
            anim_speed_q11: -1024,
            ..base
        });
        assert_eq!(recoil.trigger_attack_sfx_volume, None);
    }
}
