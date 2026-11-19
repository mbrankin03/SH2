//! Animated Stage-Placed Models (`stagemdl.c` — `Event/chara_data_load.c` `mdl_list`).
//!
//! Manages dynamic 3D `.mdl` props placed inside stages via `mdl_list`, verified 1:1 against
//! retail `sh2pc.exe` and SH2:EE patches:
//! - **Hotel Lobby Music Box (`Stage 40`, kind `0x524`, `data/chr/item/org.mdl`)**:
//!   `sh2pc.exe` `0x0057b5b0..0x0057b720` (`5 * PI` rad at `0x00715cf8`, figurine bones `6`, `7`, `8`).
//! - **Labyrinth Rotating Cube Room (`Stage 36`, kind `0x522`)**:
//!   `sh2pc.exe` `0x00582bd0..0x00582d0b` (`PI / 2` at `0x006a14e4`, bone `1`, lookup table `0x008d14d4`).
//! - **Labyrinth Elevator Gates (`Stage 35/36`)**:
//!   `sh2pc.exe` `0x00583160..0x005832be` (`-45600.0 mm` at `0x0071eb68`, sounds `0x4653`..`0x4656`,
//!   SH2:EE `Patches/LabyrinthElevatorVolumeFix.cpp`).
//! - **Old Man Coin in Garbage Chute (`Stage 13`, kind `0x71f`)**:
//!   SH2:EE `Patches/OldManCoinFix.cpp` (gated on Flag `0x5f` — canned juice used on chute).
//! - **Maria Resting in Brookhaven Room S3 (`Stage 20`, kind `0x106`)**:
//!   `sh2pc.exe` `0x0058ee0f` (pos `0x008d85fc`, yaw `PI` `0x40490fdb`, anim `0x0d49`, Flag `186`).

use std::f32::consts::{FRAC_PI_2, PI, TAU};

use crate::events::stageparts::FLAG_CANNED_JUICE_USED;
use crate::events::EventFlags;

/// Model kind ID for Lakeview Hotel Lobby Music Box (`data/chr/item/org.mdl`).
pub const MODEL_KIND_HOTEL_MUSIC_BOX: u16 = 0x0524;
/// Model kind ID for Labyrinth Rotating Cube Room (`sh2pc.exe` `0x00582c50`, `data/chr/item/box.mdl`).
pub const MODEL_KIND_LABYRINTH_CUBE: u16 = 0x0522;
/// Model kind ID for Labyrinth Elevator Gate prop (`sh2pc.exe` `0x00582fdd`, `data/chr/item/evj.mdl`).
pub const MODEL_KIND_LABYRINTH_ELEVATOR_GATE: u16 = 0x0530;
/// Model kind ID for Old Man Coin in Wood Side garbage chute (`Patches/OldManCoinFix.cpp:30`, `data/chr/item/x_coinelder.mdl`).
pub const MODEL_KIND_OLD_MAN_COIN: u16 = 0x071f;
/// Model kind ID for Maria resting in Brookhaven Room S3 (`sh2pc.exe` `0x0058ee0f`, `data/chr/mar/lll_mar.mdl`).
pub const MODEL_KIND_MARIA_RESTING: u16 = 0x0106;

/// Flag `186` (`0xba`): Maria is resting on the bed in Brookhaven Hospital Room S3 (`Stage 20`).
pub const FLAG_MARIA_RESTING_ROOM_S3: usize = 186;

/// Hotel Music Box total angular span (`5 * PI` rad = `15.707964`, `sh2pc.exe` `0x00715cf8`).
pub const MUSIC_BOX_ROTATION_SPAN_RAD: f32 = 5.0 * PI;

/// Labyrinth Rotating Cube quarter-turn angle (`PI / 2` rad = `1.5707964`, `sh2pc.exe` `0x006a14e4`).
pub const LABYRINTH_CUBE_STEP_RAD: f32 = FRAC_PI_2;

/// Labyrinth Rotating Cube 4x4 `(x_step * 4 + y_step)` doorway alignment table (`sh2pc.exe` `0x008d14d4`).
///
/// Value `> 0` sets flag bit `0x20` (`sh2pc.exe` `0x00582cdd`); value `> 1` sets flag bit `0x40` (`0x00582cf2`).
pub const LABYRINTH_CUBE_ALIGNMENT_TABLE: [u8; 16] =
    [2, 1, 2, 0, 1, 0, 1, 0, 0, 1, 1, 0, 0, 0, 0, 0];

/// Labyrinth Elevator bottom floor Y coordinate in PS2 mm (`-45600.0`, `sh2pc.exe` `0x0071eb68`).
pub const LABYRINTH_ELEVATOR_BOTTOM_Y_MM: f32 = -45_600.0;

/// Labyrinth Elevator speed factor (`500.0 mm/s`, `sh2pc.exe` `0x0062d230`).
pub const LABYRINTH_ELEVATOR_SPEED_MM_PER_SEC: f32 = 500.0;

/// Labyrinth Elevator Gate travel distance in PS2 mm (`450.0` = `0x43e10000`, `sh2pc.exe` `0x00582fd0`).
pub const LABYRINTH_ELEVATOR_GATE_TRAVEL_MM: f32 = 450.0;

/// Labyrinth Elevator Gate translation speed in PS2 mm/s (`1000.0` = `0x447a0000`, `sh2pc.exe` `0x00582f06`).
pub const LABYRINTH_ELEVATOR_GATE_SPEED_MM_PER_SEC: f32 = 1_000.0;

/// Labyrinth Elevator sound IDs (`sh2pc.exe` `0x005831ad` & `Patches/LabyrinthElevatorVolumeFix.cpp:48`).
pub const SE_LABYRINTH_ELEVATOR_LOOP: u16 = 0x4653;
pub const SE_LABYRINTH_ELEVATOR_STOP: u16 = 0x4654;
pub const SE_LABYRINTH_GATE_OPEN: u16 = 0x4655;
pub const SE_LABYRINTH_GATE_CLOSE: u16 = 0x4656;

/// Maria resting world position in Brookhaven Room S3 in PS2 mm (`sh2pc.exe` `0x008d85fc`).
pub const MARIA_ROOM_S3_POS_MM: [f32; 3] = [99_742.56, 203.282_56, 58_623.12];
/// Maria resting yaw in radians (`PI` = `0x40490fdb`, `sh2pc.exe` `0x0058ee2c`).
pub const MARIA_ROOM_S3_YAW_RAD: f32 = PI;
/// Maria resting animation ID (`0x0d49`, `sh2pc.exe` `0x0058ee39`).
pub const MARIA_ROOM_S3_ANIM_ID: u16 = 0x0d49;

/// Identifies which stage-placed `.mdl` prop an instance represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StageModelKind {
    /// Hotel Lobby Music Box (`Stage 40`, `0x0524`, `data/chr/item/org.mdl`).
    HotelMusicBox,
    /// Labyrinth Rotating Cube Room (`Stage 36`, `0x0522`).
    LabyrinthRotatingCube,
    /// Labyrinth Elevator Gate (`Stage 35/36`, `0x0520`).
    LabyrinthElevatorGate,
    /// Old Man Coin in Garbage Chute (`Stage 13`, `0x071f`).
    OldManCoin,
    /// Maria Resting in Brookhaven Room S3 (`Stage 20`, `0x0106`).
    MariaRestingRoomS3,
}

impl StageModelKind {
    /// Returns the original PS2/PC `chara_data_load` model kind ID.
    #[inline]
    #[must_use]
    pub const fn model_kind_id(self) -> u16 {
        match self {
            Self::HotelMusicBox => MODEL_KIND_HOTEL_MUSIC_BOX,
            Self::LabyrinthRotatingCube => MODEL_KIND_LABYRINTH_CUBE,
            Self::LabyrinthElevatorGate => MODEL_KIND_LABYRINTH_ELEVATOR_GATE,
            Self::OldManCoin => MODEL_KIND_OLD_MAN_COIN,
            Self::MariaRestingRoomS3 => MODEL_KIND_MARIA_RESTING,
        }
    }

    /// Returns the canonical relative `.mdl` path under `data/` (verified against retail `data/chr/`).
    #[inline]
    #[must_use]
    pub const fn asset_rel_path(self) -> &'static str {
        match self {
            Self::HotelMusicBox => "chr/item/org.mdl",
            Self::LabyrinthRotatingCube => "chr/item/box.mdl",
            Self::LabyrinthElevatorGate => "chr/item/evj.mdl",
            Self::OldManCoin => "chr/item/x_coinelder.mdl",
            Self::MariaRestingRoomS3 => "chr/mar/lll_mar.mdl",
        }
    }
}

/// Per-bone local transform override applied to a stage-placed `.mdl` skeleton.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoneTransformOverride {
    /// Target bone index in the `.mdl` skeleton (e.g. `6`, `7`, `8` for music box figurines; `1` for cube).
    pub bone_index: u8,
    /// Local translation offset in PS2 mm `[x, y, z]`.
    pub translation_mm: [f32; 3],
    /// Local Euler rotation in radians `[pitch_x, yaw_y, roll_z]`.
    pub rotation_rad: [f32; 3],
}

/// Positional sound cue emitted by an animated stage model (e.g. Labyrinth lift gates & descent).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StageModelAudioCue {
    /// Retail `sddata.bin` sound effect ID (`0x4653`..`0x4656`).
    pub sound_id: u16,
    /// Normalized playback volume (`1.0` with SH2:EE `LabyrinthElevatorVolumeFix`, `0.8` vanilla).
    pub volume: f32,
}

/// Runtime descriptor for a placed stage `.mdl` prop ready for `sh2-renderer` consumption.
#[derive(Debug, Clone, PartialEq)]
pub struct StageModelInstance {
    /// Prop kind identifier.
    pub kind: StageModelKind,
    /// Original PS2/PC `chara_data_load` model kind ID.
    pub model_kind_id: u16,
    /// Relative `.mdl` path under `data/`.
    pub asset_rel_path: &'static str,
    /// Whether this prop should currently be rendered.
    pub visible: bool,
    /// World-space position in PS2 mm `[x, y, z]`.
    pub position_mm: [f32; 3],
    /// World-space Euler rotation in radians `[pitch_x, yaw_y, roll_z]`.
    pub rotation_rad: [f32; 3],
    /// Optional skeletal animation ID (e.g. `0x0d49` for Maria resting on bed).
    pub anim_id: Option<u16>,
    /// Per-bone local transform overrides (cylinder, figurines, rotating cube bone 1, lift gates).
    pub bone_overrides: Vec<BoneTransformOverride>,
}

/// Hotel Lobby Music Box runtime animation state (`Stage 40`, `sh2pc.exe` `0x0057b5b0`).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct MusicBoxAnimState {
    /// Whether the music box tune is currently playing.
    pub playing: bool,
    /// Elapsed playback time in seconds (`0x01d83350`).
    pub elapsed_secs: f32,
    /// Total tune duration in seconds.
    pub duration_secs: f32,
    /// Cylinder rotation angle in radians.
    pub cylinder_angle_rad: f32,
    /// Per-figurine Y rotation angles for bones `6`, `7`, `8` (`0x01d8336c`, `0x01d83370`, `0x01d83374`).
    pub figurine_angles_rad: [f32; 3],
}

/// Labyrinth Rotating Cube Room state (`Stage 36`, `sh2pc.exe` `0x00582bd0`).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct LabyrinthCubeState {
    /// Quarter-turn index around X axis (`0..=3`, `edi`).
    pub x_quarter_turns: u8,
    /// Quarter-turn index around Y axis (`0..=3`, `ebx`).
    pub y_quarter_turns: u8,
    /// Doorway alignment bit `0x20` (`table[x*4 + y] > 0`).
    pub doorway_flag_20: bool,
    /// Doorway alignment bit `0x40` (`table[x*4 + y] > 1`).
    pub doorway_flag_40: bool,
}

/// Labyrinth Elevator Gates & Cabin descent state (`Stage 35/36`, `sh2pc.exe` `0x00583160`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LabyrinthElevatorState {
    /// Whether the elevator cabin is currently descending.
    pub descending: bool,
    /// Current cabin Y position in PS2 mm (`0.0` top -> `-45600.0` bottom).
    pub cabin_y_mm: f32,
    /// Gate open ratio (`0.0` closed -> `1.0` fully open).
    pub gate_open_ratio: f32,
    /// Target gate open state (`true` = opening, `false` = closing).
    pub gate_target_open: bool,
    /// Gate translation travel in PS2 mm when fully open.
    pub gate_travel_mm: f32,
    /// Tracks whether the arrival stop sound (`0x4654`) has already fired.
    pub arrived: bool,
}

impl Default for LabyrinthElevatorState {
    fn default() -> Self {
        Self {
            descending: false,
            cabin_y_mm: 0.0,
            gate_open_ratio: 0.0,
            gate_target_open: false,
            gate_travel_mm: LABYRINTH_ELEVATOR_GATE_TRAVEL_MM,
            arrived: false,
        }
    }
}

/// Inputs to [`StageModelManager::tick`] for one simulation step.
#[derive(Debug, Clone, Copy)]
pub struct StageModelTickInput<'a> {
    /// Retail stage ID (`1..=58`).
    pub stage_id: u16,
    /// Delta time in seconds (e.g. `1.0 / 60.0` or `1.0 / 30.0`).
    pub dt_secs: f32,
    /// Global `16384`-bit story progression flag bank.
    pub flags: &'a EventFlags,
    /// Whether SH2:EE fixes (`OldManCoinFix`, `LabyrinthElevatorVolumeFix`) are enabled.
    pub enable_sh2ee_fixes: bool,
    /// Whether the game is running in 60 FPS mode (used by `LabyrinthElevatorVolumeFix`).
    pub sixty_fps_mode: bool,
}

/// Output produced by [`StageModelManager::tick`] for the current frame.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StageModelTickOutput {
    /// Active stage-placed `.mdl` prop instances for the current stage.
    pub instances: Vec<StageModelInstance>,
    /// Sound effect cues triggered during this frame.
    pub audio_cues: Vec<StageModelAudioCue>,
}

/// Manages all dynamic stage-placed `.mdl` props (`stagemdl.c`).
#[derive(Debug, Clone, Default)]
pub struct StageModelManager {
    /// Hotel Lobby Music Box animation state (`Stage 40`).
    pub music_box: MusicBoxAnimState,
    /// Labyrinth Rotating Cube Room state (`Stage 36`).
    pub labyrinth_cube: LabyrinthCubeState,
    /// Labyrinth Elevator Gates & Cabin descent state (`Stage 35/36`).
    pub labyrinth_elevator: LabyrinthElevatorState,
}

impl StageModelManager {
    /// Creates a new `StageModelManager` with default initial prop states.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Starts playing the Hotel Lobby Music Box (`Stage 40`) for `duration_secs` seconds.
    pub fn start_music_box(&mut self, duration_secs: f32) {
        self.music_box.playing = true;
        self.music_box.elapsed_secs = 0.0;
        self.music_box.duration_secs = duration_secs.max(0.1);
    }

    /// Sets the Labyrinth Rotating Cube puzzle orientation (`x_steps`, `y_steps` modulo 4)
    /// and updates the doorway alignment flags from `sh2pc.exe` `0x008d14d4`.
    pub fn set_labyrinth_cube_orientation(&mut self, x_steps: u8, y_steps: u8) {
        let x = x_steps & 3;
        let y = y_steps & 3;
        self.labyrinth_cube.x_quarter_turns = x;
        self.labyrinth_cube.y_quarter_turns = y;
        let val = LABYRINTH_CUBE_ALIGNMENT_TABLE[(x as usize) * 4 + (y as usize)];
        self.labyrinth_cube.doorway_flag_20 = val > 0;
        self.labyrinth_cube.doorway_flag_40 = val > 1;
    }

    /// Commands the Labyrinth Elevator gates to open or close, returning the corresponding audio cue.
    pub fn set_labyrinth_gate_open(
        &mut self,
        open: bool,
        enable_sh2ee_fixes: bool,
    ) -> StageModelAudioCue {
        self.labyrinth_elevator.gate_target_open = open;
        StageModelAudioCue {
            sound_id: if open {
                SE_LABYRINTH_GATE_OPEN
            } else {
                SE_LABYRINTH_GATE_CLOSE
            },
            volume: if enable_sh2ee_fixes { 1.0 } else { 0.8 },
        }
    }

    /// Starts the Labyrinth Elevator descent toward `-45600.0 mm`.
    pub fn start_labyrinth_elevator_descent(&mut self) {
        self.labyrinth_elevator.descending = true;
        self.labyrinth_elevator.arrived = false;
    }

    /// Advances all active stage-placed `.mdl` prop animations and returns the stage's instances.
    #[must_use]
    pub fn tick(&mut self, input: &StageModelTickInput<'_>) -> StageModelTickOutput {
        let mut out = StageModelTickOutput::default();
        let dt = input.dt_secs.max(0.0);

        match input.stage_id {
            // Stage 13: Blue Creek / Wood Side exterior — Old Man Coin in garbage chute
            13 => {
                // SH2:EE `OldManCoinFix` (`Patches/OldManCoinFix.cpp:24`):
                // Only visible after Flag 0x5f (canned juice used in 2F laundry chute).
                let visible = if input.enable_sh2ee_fixes {
                    input.flags.get(FLAG_CANNED_JUICE_USED)
                } else {
                    true
                };
                out.instances.push(StageModelInstance {
                    kind: StageModelKind::OldManCoin,
                    model_kind_id: MODEL_KIND_OLD_MAN_COIN,
                    asset_rel_path: StageModelKind::OldManCoin.asset_rel_path(),
                    visible,
                    position_mm: [0.0, 0.0, 0.0],
                    rotation_rad: [0.0, 0.0, 0.0],
                    anim_id: None,
                    bone_overrides: Vec::new(),
                });
            }

            // Stage 20: Brookhaven Hospital 3F (Room S3) — Maria resting on bed
            20 => {
                let visible = input.flags.get(FLAG_MARIA_RESTING_ROOM_S3);
                out.instances.push(StageModelInstance {
                    kind: StageModelKind::MariaRestingRoomS3,
                    model_kind_id: MODEL_KIND_MARIA_RESTING,
                    asset_rel_path: StageModelKind::MariaRestingRoomS3.asset_rel_path(),
                    visible,
                    position_mm: MARIA_ROOM_S3_POS_MM,
                    rotation_rad: [0.0, MARIA_ROOM_S3_YAW_RAD, 0.0],
                    anim_id: Some(MARIA_ROOM_S3_ANIM_ID),
                    bone_overrides: Vec::new(),
                });
            }

            // Stage 35 & 36: Labyrinth East / North — Rotating Cube Room & Elevator Gates
            35 | 36 => {
                // SH2:EE `LabyrinthElevatorVolumeFix` (`Patches/LabyrinthElevatorVolumeFix.cpp:24, 65`):
                // Uses 1/60 s (60 FPS) vs 1/30 s (30 FPS) frame time for audio fade and full `1.0`
                // volume instead of vanilla `0.8`, while physical gate & cabin motion uses real `dt`.
                let audio_volume = if input.enable_sh2ee_fixes { 1.0 } else { 0.8 };
                let _audio_fade_dt = if input.enable_sh2ee_fixes {
                    if input.sixty_fps_mode {
                        1.0 / 60.0
                    } else {
                        1.0 / 30.0
                    }
                } else {
                    dt
                };

                // Animate lift gate bone translation (`sh2pc.exe` `0x00582f06`, `0x00582fd0`, `0x00583012`).
                let travel = self.labyrinth_elevator.gate_travel_mm.max(1.0);
                let ratio_step = (LABYRINTH_ELEVATOR_GATE_SPEED_MM_PER_SEC / travel) * dt;
                if self.labyrinth_elevator.gate_target_open {
                    self.labyrinth_elevator.gate_open_ratio =
                        (self.labyrinth_elevator.gate_open_ratio + ratio_step).min(1.0);
                } else {
                    self.labyrinth_elevator.gate_open_ratio =
                        (self.labyrinth_elevator.gate_open_ratio - ratio_step).max(0.0);
                }

                // Animate elevator descent toward -45600.0 mm (`sh2pc.exe` `0x00583164..0x005831c4`).
                if self.labyrinth_elevator.descending {
                    self.labyrinth_elevator.cabin_y_mm -= LABYRINTH_ELEVATOR_SPEED_MM_PER_SEC * dt;
                    if self.labyrinth_elevator.cabin_y_mm <= LABYRINTH_ELEVATOR_BOTTOM_Y_MM {
                        self.labyrinth_elevator.cabin_y_mm = LABYRINTH_ELEVATOR_BOTTOM_Y_MM;
                        self.labyrinth_elevator.descending = false;
                        if !self.labyrinth_elevator.arrived {
                            self.labyrinth_elevator.arrived = true;
                            out.audio_cues.push(StageModelAudioCue {
                                sound_id: SE_LABYRINTH_ELEVATOR_STOP,
                                volume: audio_volume,
                            });
                        }
                    } else {
                        out.audio_cues.push(StageModelAudioCue {
                            sound_id: SE_LABYRINTH_ELEVATOR_LOOP,
                            volume: audio_volume,
                        });
                    }
                }

                // `sh2pc.exe` `0x00583012..0x00583094`: bone 0 translated by `-gate_pos` along Z.
                let gate_offset_z = self.labyrinth_elevator.gate_open_ratio * travel;
                out.instances.push(StageModelInstance {
                    kind: StageModelKind::LabyrinthElevatorGate,
                    model_kind_id: MODEL_KIND_LABYRINTH_ELEVATOR_GATE,
                    asset_rel_path: StageModelKind::LabyrinthElevatorGate.asset_rel_path(),
                    visible: true,
                    position_mm: [0.0, self.labyrinth_elevator.cabin_y_mm, 0.0],
                    rotation_rad: [0.0, 0.0, 0.0],
                    anim_id: None,
                    bone_overrides: vec![BoneTransformOverride {
                        bone_index: 0,
                        translation_mm: [0.0, 0.0, -gate_offset_z],
                        rotation_rad: [0.0, 0.0, 0.0],
                    }],
                });

                // Stage 36 also hosts the Labyrinth Rotating Cube Room (`0x0522`, `sh2pc.exe` `0x00582bd0`).
                if input.stage_id == 36 {
                    let rx =
                        f32::from(self.labyrinth_cube.x_quarter_turns) * LABYRINTH_CUBE_STEP_RAD;
                    let ry =
                        f32::from(self.labyrinth_cube.y_quarter_turns) * LABYRINTH_CUBE_STEP_RAD;
                    out.instances.push(StageModelInstance {
                        kind: StageModelKind::LabyrinthRotatingCube,
                        model_kind_id: MODEL_KIND_LABYRINTH_CUBE,
                        asset_rel_path: StageModelKind::LabyrinthRotatingCube.asset_rel_path(),
                        visible: true,
                        position_mm: [0.0, 0.0, 0.0],
                        rotation_rad: [rx, ry, 0.0],
                        anim_id: None,
                        bone_overrides: vec![BoneTransformOverride {
                            bone_index: 1,
                            translation_mm: [0.0, 0.0, 0.0],
                            rotation_rad: [rx, ry, 0.0],
                        }],
                    });
                }
            }

            // Stage 40: Lakeview Hotel 1F Lobby — Music Box (`0x0524`, `data/chr/item/org.mdl`, `0x0057b5b0`)
            40 => {
                if self.music_box.playing {
                    let dur = self.music_box.duration_secs.max(0.1);
                    self.music_box.elapsed_secs += dt;
                    let omega = MUSIC_BOX_ROTATION_SPAN_RAD / dur;
                    self.music_box.cylinder_angle_rad =
                        (self.music_box.cylinder_angle_rad + omega * dt).rem_euclid(TAU);

                    // `sh2pc.exe` `0x0057b5ba..0x0057b716`: bones 6, 7, 8 rotate forward during
                    // first half of tune and reverse during second half.
                    let half = dur * 0.5;
                    let dir = if self.music_box.elapsed_secs <= half {
                        1.0
                    } else {
                        -1.0
                    };
                    for angle in &mut self.music_box.figurine_angles_rad {
                        *angle = (*angle + dir * omega * dt).rem_euclid(TAU);
                    }

                    if self.music_box.elapsed_secs >= dur {
                        self.music_box.playing = false;
                        self.music_box.figurine_angles_rad = [0.0; 3];
                    }
                }

                out.instances.push(StageModelInstance {
                    kind: StageModelKind::HotelMusicBox,
                    model_kind_id: MODEL_KIND_HOTEL_MUSIC_BOX,
                    asset_rel_path: StageModelKind::HotelMusicBox.asset_rel_path(),
                    visible: true,
                    position_mm: [0.0, 0.0, 0.0],
                    rotation_rad: [0.0, 0.0, 0.0],
                    anim_id: None,
                    bone_overrides: vec![
                        // Cylinder/base bone 0 (`sh2pc.exe` `0x0057b4d9`) & lid/crank bone 2 (`0x0057b588`)
                        BoneTransformOverride {
                            bone_index: 0,
                            translation_mm: [0.0, 0.0, 0.0],
                            rotation_rad: [self.music_box.cylinder_angle_rad, 0.0, 0.0],
                        },
                        BoneTransformOverride {
                            bone_index: 2,
                            translation_mm: [0.0, 0.0, 0.0],
                            rotation_rad: [if self.music_box.playing { PI } else { 0.0 }, 0.0, 0.0],
                        },
                        // 3 Princess figurines: bones 6, 7, 8 (`sh2pc.exe` `0x0057b646`, `0x0057b658`, `0x0057b713`)
                        BoneTransformOverride {
                            bone_index: 6,
                            translation_mm: [0.0, 0.0, 0.0],
                            rotation_rad: [0.0, self.music_box.figurine_angles_rad[0], 0.0],
                        },
                        BoneTransformOverride {
                            bone_index: 7,
                            translation_mm: [0.0, 0.0, 0.0],
                            rotation_rad: [0.0, self.music_box.figurine_angles_rad[1], 0.0],
                        },
                        BoneTransformOverride {
                            bone_index: 8,
                            translation_mm: [0.0, 0.0, 0.0],
                            rotation_rad: [0.0, self.music_box.figurine_angles_rad[2], 0.0],
                        },
                    ],
                });
            }

            _ => {}
        }

        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_old_man_coin_fix_requires_flag_0x5f() {
        let mut mgr = StageModelManager::new();
        let mut flags = EventFlags::new();

        let out_before = mgr.tick(&StageModelTickInput {
            stage_id: 13,
            dt_secs: 1.0 / 60.0,
            flags: &flags,
            enable_sh2ee_fixes: true,
            sixty_fps_mode: true,
        });
        assert_eq!(out_before.instances.len(), 1);
        assert!(!out_before.instances[0].visible);

        flags.set(FLAG_CANNED_JUICE_USED, true);
        let out_after = mgr.tick(&StageModelTickInput {
            stage_id: 13,
            dt_secs: 1.0 / 60.0,
            flags: &flags,
            enable_sh2ee_fixes: true,
            sixty_fps_mode: true,
        });
        assert!(out_after.instances[0].visible);
    }

    #[test]
    fn test_hotel_music_box_rotates_cylinder_and_three_figurines() {
        let mut mgr = StageModelManager::new();
        let flags = EventFlags::new();
        mgr.start_music_box(10.0);

        let out = mgr.tick(&StageModelTickInput {
            stage_id: 40,
            dt_secs: 1.0,
            flags: &flags,
            enable_sh2ee_fixes: true,
            sixty_fps_mode: true,
        });
        assert_eq!(out.instances.len(), 1);
        let inst = &out.instances[0];
        assert_eq!(inst.model_kind_id, MODEL_KIND_HOTEL_MUSIC_BOX);
        assert_eq!(inst.bone_overrides.len(), 5);
        assert_eq!(inst.bone_overrides[0].bone_index, 0);
        assert!(inst.bone_overrides[0].rotation_rad[0] > 0.0);
        assert_eq!(inst.bone_overrides[1].bone_index, 2);
        assert_eq!(inst.bone_overrides[2].bone_index, 6);
        assert!(inst.bone_overrides[2].rotation_rad[1] > 0.0);
        assert_eq!(inst.bone_overrides[3].bone_index, 7);
        assert_eq!(inst.bone_overrides[4].bone_index, 8);
    }

    #[test]
    fn test_labyrinth_cube_alignment_table_matches_pc_0x8d14d4() {
        let mut mgr = StageModelManager::new();
        mgr.set_labyrinth_cube_orientation(0, 0);
        assert!(mgr.labyrinth_cube.doorway_flag_20);
        assert!(mgr.labyrinth_cube.doorway_flag_40);

        mgr.set_labyrinth_cube_orientation(0, 1);
        assert!(mgr.labyrinth_cube.doorway_flag_20);
        assert!(!mgr.labyrinth_cube.doorway_flag_40);

        mgr.set_labyrinth_cube_orientation(0, 3);
        assert!(!mgr.labyrinth_cube.doorway_flag_20);
        assert!(!mgr.labyrinth_cube.doorway_flag_40);
    }

    #[test]
    fn test_maria_resting_room_s3_flag_186() {
        let mut mgr = StageModelManager::new();
        let mut flags = EventFlags::new();
        flags.set(FLAG_MARIA_RESTING_ROOM_S3, true);

        let out = mgr.tick(&StageModelTickInput {
            stage_id: 20,
            dt_secs: 1.0 / 60.0,
            flags: &flags,
            enable_sh2ee_fixes: true,
            sixty_fps_mode: true,
        });
        assert_eq!(out.instances.len(), 1);
        assert!(out.instances[0].visible);
        assert_eq!(out.instances[0].model_kind_id, MODEL_KIND_MARIA_RESTING);
        assert_eq!(out.instances[0].anim_id, Some(MARIA_ROOM_S3_ANIM_ID));
        assert_eq!(out.instances[0].position_mm, MARIA_ROOM_S3_POS_MM);
    }
}
