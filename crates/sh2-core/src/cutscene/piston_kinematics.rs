//! Abstract Daddy 24-Piston Array Kinematics (`0x00581320..0x005815B0`, `PistonList` at `0x01D83570`),
//! Wood Side Rm 307 `b_doo` Closet Door Model Replacement (`ClosetRoomEnhanced`), and Bonus Audio
//! Trigger (`sankaku_toujyou_2.wav`) (`SH2-105`).
//!
//! # Ground Truth (`SLUS_202.28`, `sh2pc.exe`, `PistonRoom.cpp`, & `ClosetCutscene.cpp`)
//! 1. **Abstract Daddy 24-Piston Array (`R_LAB_BOTTOM_D = 0x89` / `R_LAB_TOP_H = 0x8A`, `0x00581320..0x005815B0`)**:
//!    - Maintains 24 piston phase floats (`PistonList[0..24]` at `0x01D83570..0x01D835D0`).
//!    - Each frame (`0x005814B4`), advances `phase[i] = wrap(phase[i] + dt_sec * speed[i] * 0.5, 0..TAU)`
//!      using the 24-entry speed table at `0x008D0518` and computes sinusoidal extension
//!      `disp = (cos(phase[i]) + 1.0) * amplitude` along the piston's axis direction (`0x008D0578[i]` in `{+X, -X, +Z, -Z}`).
//!    - During `CS_PS_ANGELA_BOSS_FINISH` (`demo_no = 68`, `0x44`), once `CameraX == 20278.414f` (`0x469E6CD4`,
//!      when Angela opens the door to leave the piston room), `PistonRoom.cpp:35..58` latches `ConditionVal = 1`
//!      and adds `+2.50f` (`0x40200000`) to `PistonList[1]` (`0x01D83574`) so piston #1 stays retracted inside
//!      its wall housing instead of clipping through the open doorway.
//! 2. **Wood Side Rm 307 Closet Door Model Replacement (`b_doo` / `ClosetRoomEnhanced` in `ClosetCutscene.cpp:118..155`)**:
//!    - Replaces `kClosetModelTable` in `R_APT_E_RM_307` (`0x13`) / `CS_APT_RPT_CLOSET` (`0x0E`) / `CS_APT_RPT_FIGHT` (`0x13`)
//!      with the animated closet doors model (`b_doo.mdl`) transformed by `CLOSET_DOOR_WORLD_MATRIX` (`closetXForm`),
//!      visible when `scene_frame < 359.5 || scene_frame > 1695.0` and hidden during `359.5..=1695.0` while James
//!      looks out from inside the closet.
//! 3. **Closet Cutscene Bonus Audio (`ClosetCutsceneBonusAudio` in `ClosetCutscene.cpp:249..267`)**:
//!    - When enabled and `demo_no == 14` (`CS_APT_RPT_CLOSET`) reaches `scene_frame > 1900.0`, plays
//!      `"data/sound/adx/amb/sankaku_toujyou_2.wav"` once per cutscene session.

use std::f32::consts::TAU;

use glam::Vec3;

use super::atmosphere::{CS_APT_RPT_FIGHT_DEMO_NO, R_LAB_BOTTOM_D_ID, R_LAB_TOP_H_ID};
use super::stage_fixes::{
    PistonRoomFixState, CS_APT_RPT_CLOSET_DEMO_NO, PISTON_DOORWAY_PROP_OFFSET, R_APT_E_RM_307_ID,
};

/// Number of animated wall pistons in the Abstract Daddy boss room (`0x01D83570..0x01D835D0`).
pub const PISTON_ARRAY_COUNT: usize = 24;

/// Index (`1`, corresponding to `0x01D83574` in `PistonRoom.cpp:52`) of the doorway piston that is
/// retracted/offset by `+2.50f` when Angela opens the exit door at `CameraX == 20278.414f`.
pub const DOORWAY_PISTON_INDEX: usize = 1;

/// Default peak sinusoidal stroke half-amplitude for the Labyrinth wall pistons (`(cos(phase) + 1.0) * amplitude`).
pub const DEFAULT_PISTON_STROKE_AMPLITUDE: f32 = 125.0;

/// Exact 24-entry piston angular speed table (`0x008D0518..0x008D0578` in `sh2pc.exe`).
pub const PISTON_SPEED_TABLE: [f32; PISTON_ARRAY_COUNT] = [
    0.846_213, 0.461_351, 0.854_768, 0.987_341, 0.768_454, 0.846_245, 0.564_845, 0.874_651,
    0.213_578, 0.987_544, 0.246_870, 0.674_534, 0.443_547, 0.687_412, 0.789_654, 0.687_840,
    0.454_878, 0.157_684, 0.654_875, 0.526_464, 0.984_671, 0.548_721, 0.784_525, 0.487_146,
];

/// Piston stroke axis direction encoded in `0x008D0578[0..24]` (`0x00581523..0x0058159B` in `sh2pc.exe`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PistonAxisDir {
    /// Case `0`: translates along `+X`.
    PosX = 0,
    /// Case `1`: translates along `-X`.
    NegX = 1,
    /// Case `2`: translates along `+Z`.
    PosZ = 2,
    /// Case `3`: translates along `-Z`.
    NegZ = 3,
}

impl PistonAxisDir {
    /// Returns the unit world-space stroke direction vector for this piston axis.
    #[inline]
    #[must_use]
    pub const fn unit_vector(self) -> Vec3 {
        match self {
            Self::PosX => Vec3::new(1.0, 0.0, 0.0),
            Self::NegX => Vec3::new(-1.0, 0.0, 0.0),
            Self::PosZ => Vec3::new(0.0, 0.0, 1.0),
            Self::NegZ => Vec3::new(0.0, 0.0, -1.0),
        }
    }
}

/// Exact 24-entry piston stroke axis table (`0x008D0578` in `sh2pc.exe`:
/// `[2, 0, 0, 0, 1, 0, 0, 1, 2, 2, 1, 1, 3, 3, 3, 0, 1, 3, 3, 1, 2, 2, 1, 1]`).
pub const PISTON_AXIS_DIR_TABLE: [PistonAxisDir; PISTON_ARRAY_COUNT] = [
    PistonAxisDir::PosZ,
    PistonAxisDir::PosX,
    PistonAxisDir::PosX,
    PistonAxisDir::PosX,
    PistonAxisDir::NegX,
    PistonAxisDir::PosX,
    PistonAxisDir::PosX,
    PistonAxisDir::NegX,
    PistonAxisDir::PosZ,
    PistonAxisDir::PosZ,
    PistonAxisDir::NegX,
    PistonAxisDir::NegX,
    PistonAxisDir::NegZ,
    PistonAxisDir::NegZ,
    PistonAxisDir::NegZ,
    PistonAxisDir::PosX,
    PistonAxisDir::NegX,
    PistonAxisDir::NegZ,
    PistonAxisDir::NegZ,
    PistonAxisDir::NegX,
    PistonAxisDir::PosZ,
    PistonAxisDir::PosZ,
    PistonAxisDir::NegX,
    PistonAxisDir::NegX,
];

/// Stateful 24-piston array controller for the Abstract Daddy boss room (`R_LAB_BOTTOM_D` / `R_LAB_TOP_H`, `SH2-105`).
#[derive(Debug, Clone, PartialEq)]
pub struct PistonArrayRuntime {
    /// Current phase values (`PistonList[0..24]` at `0x01D83570..0x01D835D0`).
    pub phases: [f32; PISTON_ARRAY_COUNT],
    /// Evaluated world-space displacement offsets for all 24 pistons.
    pub displacements: [Vec3; PISTON_ARRAY_COUNT],
    /// Sinusoidal stroke half-amplitude in world units.
    pub stroke_amplitude: f32,
    /// Stateful doorway piston retraction latch (`PistonRoom.cpp:35..58`).
    pub fix_state: PistonRoomFixState,
}

impl Default for PistonArrayRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl PistonArrayRuntime {
    /// Initializes all 24 pistons with deterministic staggered starting phases (`0x00581340`).
    #[must_use]
    pub fn new() -> Self {
        let mut phases = [0.0f32; PISTON_ARRAY_COUNT];
        for (i, phase) in phases.iter_mut().enumerate() {
            *phase = ((i as f32) * PISTON_SPEED_TABLE[i] * 1.37).rem_euclid(TAU);
        }
        let mut runtime = Self {
            phases,
            displacements: [Vec3::ZERO; PISTON_ARRAY_COUNT],
            stroke_amplitude: DEFAULT_PISTON_STROKE_AMPLITUDE,
            fix_state: PistonRoomFixState::default(),
        };
        runtime.recompute_displacements();
        runtime
    }

    /// Returns `true` if `room_id` or `demo_no` corresponds to the Abstract Daddy piston room.
    #[inline]
    #[must_use]
    pub const fn is_piston_room_active(room_id: u32, demo_no: u16) -> bool {
        matches!(room_id, R_LAB_BOTTOM_D_ID | R_LAB_TOP_H_ID) || matches!(demo_no, 67 | 68)
    }

    /// Advances all 24 pistons by `dt_sec` and applies the `PistonRoom.cpp` `+2.50f` housing retraction
    /// latch on `PistonList[1]` (`0x01D83574`) once `CameraX == 20278.414f` in `CS_PS_ANGELA_BOSS_FINISH`.
    pub fn step(&mut self, dt_sec: f32, demo_no: u16, camera_pos_x: f32) {
        self.fix_state.update(demo_no, camera_pos_x);
        let dt = dt_sec.max(0.0);

        for (i, (phase, &speed)) in self.phases.iter_mut().zip(&PISTON_SPEED_TABLE).enumerate() {
            if i == DOORWAY_PISTON_INDEX && self.fix_state.doorway_piston_culled {
                // `PistonRoom.cpp:52`: `add dword ptr [ecx], 0x40200000` (`+2.50f` at `PistonList[1]`)
                *phase = PISTON_DOORWAY_PROP_OFFSET;
            } else {
                let delta = dt * speed * 0.5;
                *phase = (*phase + delta).rem_euclid(TAU);
            }
        }
        self.recompute_displacements();
    }

    fn recompute_displacements(&mut self) {
        for (i, ((disp, &phase), &axis_dir)) in self
            .displacements
            .iter_mut()
            .zip(&self.phases)
            .zip(&PISTON_AXIS_DIR_TABLE)
            .enumerate()
        {
            let axis = axis_dir.unit_vector();
            if i == DOORWAY_PISTON_INDEX && self.fix_state.doorway_piston_culled {
                // Keep doorway piston #1 retracted into its housing along its axis so it never
                // protrudes into the open doorway behind Angela.
                let retracted = (PISTON_DOORWAY_PROP_OFFSET.cos() + 1.0) * self.stroke_amplitude;
                *disp = axis * retracted;
            } else {
                let ext = (phase.cos() + 1.0) * self.stroke_amplitude;
                *disp = axis * ext;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Wood Side Rm 307 `b_doo` Closet Door Replacement & Bonus Audio (`ClosetCutscene.cpp`)
// ---------------------------------------------------------------------------

/// Cutscene frame (`359.5f` in `ClosetCutscene.cpp:150`) at which the replacement 3D closet door
/// (`b_doo.mdl`) hides when switching to the interior closet POV.
pub const CLOSET_DOOR_HIDE_START_FRAME: f32 = 359.5;

/// Cutscene frame (`1695.0f` in `ClosetCutscene.cpp:150`) after which the replacement 3D closet door
/// (`b_doo.mdl`) reappears when switching back to the exterior Room 307 camera.
pub const CLOSET_DOOR_HIDE_END_FRAME: f32 = 1695.0;

/// Cutscene frame (`1900.0f` in `ClosetCutscene.cpp:259`) at which the optional bonus audio cue
/// (`data/sound/adx/amb/sankaku_toujyou_2.wav`) is triggered in `CS_APT_RPT_CLOSET` (`demo_no = 14`).
pub const CLOSET_BONUS_AUDIO_TRIGGER_FRAME: f32 = 1900.0;

/// Relative path to the Pyramid Head closet cutscene bonus audio file (`ClosetCutscene.cpp:262`).
pub const CLOSET_BONUS_AUDIO_WAV_PATH: &str = "data/sound/adx/amb/sankaku_toujyou_2.wav";

/// Exact 4x4 row-major world transformation matrix (`closetXForm` in `ClosetCutscene.cpp:128`)
/// positioning the replacement `b_doo.mdl` closet door in Wood Side Apartments Room 307 (`R_APT_E_RM_307`).
pub const CLOSET_DOOR_WORLD_MATRIX: [[f32; 4]; 4] = [
    [-0.999_675, 0.000_000, 0.025_510, -21_059.994],
    [-0.025_510, 0.000_240, -0.999_675, -550.000_55],
    [-0.000_006, -1.000_000, -0.000_240, -101_184.02],
    [0.000_000, 0.000_000, 0.000_000, 1.000_000],
];

/// Evaluated state for the Wood Side Room 307 `b_doo` closet door model replacement and bonus audio (`ClosetCutscene.cpp`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClosetDoorReplacementState {
    /// Whether the replacement `b_doo.mdl` closet door should be rendered in the current frame (`ClosetCutscene.cpp:144..154`).
    pub render_replacement_door: bool,
    /// World transform matrix (`closetXForm`) for the replacement `b_doo.mdl` closet door.
    pub world_matrix: [[f32; 4]; 4],
    /// Whether the `sankaku_toujyou_2.wav` bonus audio trigger threshold (`scene_frame > 1900.0`) has been reached.
    pub bonus_audio_triggered: bool,
}

impl Default for ClosetDoorReplacementState {
    fn default() -> Self {
        Self {
            render_replacement_door: false,
            world_matrix: CLOSET_DOOR_WORLD_MATRIX,
            bonus_audio_triggered: false,
        }
    }
}

impl ClosetDoorReplacementState {
    /// Evaluates `ClosetDoorReplacementState` for `(room_id, demo_no, scene_frame, bonus_audio_enabled)`.
    #[inline]
    #[must_use]
    pub fn evaluate(
        room_id: u32,
        demo_no: u16,
        scene_frame: f32,
        bonus_audio_enabled: bool,
    ) -> Self {
        let in_closet_cutscene = demo_no == CS_APT_RPT_CLOSET_DEMO_NO;
        let in_room_307 = room_id == R_APT_E_RM_307_ID
            || in_closet_cutscene
            || demo_no == CS_APT_RPT_FIGHT_DEMO_NO;

        let render_replacement_door = if !in_room_307 {
            false
        } else if in_closet_cutscene {
            !(CLOSET_DOOR_HIDE_START_FRAME..=CLOSET_DOOR_HIDE_END_FRAME).contains(&scene_frame)
        } else {
            true
        };

        let bonus_audio_triggered = bonus_audio_enabled
            && in_closet_cutscene
            && scene_frame > CLOSET_BONUS_AUDIO_TRIGGER_FRAME;

        Self {
            render_replacement_door,
            world_matrix: CLOSET_DOOR_WORLD_MATRIX,
            bonus_audio_triggered,
        }
    }
}
