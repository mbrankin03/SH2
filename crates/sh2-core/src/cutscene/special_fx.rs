//! PS2 Post-Processing Special Effects: Screen-Space Depth-of-Field / Slat Blur (`Kari_hisyakai`,
//! `EffectMode = 5`, `0x00478330`) and GS Frame-Accumulation Motion Blur (`enSetBlur`, `EffectMode = 2`,
//! `0x00478290`) (`SH2-105`).
//!
//! # Ground Truth (`SLUS_202.28`, `sh2pc.exe`, `SpecialFX.cpp`, `ClosetCutscene.cpp`, & `PistonRoom.cpp`)
//! 1. **Resolution-Independent DoF / Closet Slat Blur (`CustomAddress2ASM` at `0x0047775D`, `CustomAddress3ASM` at `0x0047777C`)**:
//!    - Computes `display_ratio = viewport_height / 1080.0` and scales the PS2 2-tap filter offset `(val1, val2)`:
//!      - Default (`0x0047775D`): `(1.0 / 3.0, 2.0 / 3.0)`
//!      - `CS_INTRO_BATHROOM` (`demo_no = 1`, `0x01`, `Toilet_Dof_Filter`): `(-3.0, -1.5) * display_ratio`
//!      - `CS_TUNNEL_RADIO` (`demo_no = 6`, `0x06`, `CB_1stMonster_Filter`): `(-3.0, -1.0) * display_ratio`
//!      - `CS_APT_RPT_CLOSET` (`demo_no = 14`, `0x0E`, `ap18_ap21_Closet`): `(1.5, -1.5) * display_ratio`
//!        (plus suppresses the signed-byte offset write at `0x00942636` when `CameraX == -21376.56445f` per `DepthOfFieldASM` at `0x00478360`,
//!        stretches horizontal coverage across widescreen viewports, and composites noise grain *above* the blurred closet bars).
//!      - `R_HTL_RM_312` (`room_id = 0xA2`, `RR91_Filter`): `(-2.0, -4.0) * display_ratio`
//! 2. **Universal PS2 Motion Blur Scale & Boss / Cutscene Ramps (`MotionBlurASM` at `0x004782AE`)**:
//!    - Scales raw integer motion blur retention (`0x00942634`) by `PS2_MOTION_BLUR_SCALE = 0.25f` (`MotionBlurValue`).
//!    - **Flesh Lips Boss Death (`enIKECtrlDead` at `0x004B9EA5` in `PistonRoom.cpp`)**:
//!      Calls `enSetBlur` during death sub-states `{2, 4, 6}` when `enemies_remaining == 0` and `demo_no == 0` (`CS_NONE`).
//!    - **Flesh Lips Post-Boss Cutscene (`CS_HSP_BOSS_FINISH = 0x2A`, `demo_no = 42` in `PistonRoom.cpp`)**:
//!      Overrides the motion blur blend factor at `0x0058B795` to `300` (`FleshLipsCutsceneMotionBlurFactor`)
//!      and forces the lowest texture alpha bit `| 0x01` (`MotionBlurTexAlphaASM` at `0x00478B05`).
//!    - **Angela Abstract Daddy Cutscene (`CS_PS_ANGELA_BOSS_START = 0x43` / `67` & `CS_PS_ANGELA_BOSS_FINISH = 0x44` / `68` in `SpecialFX.cpp:365..388`)**:
//!      Restores PS2 linear motion blur ramps (`400 -> 30` over frames `1..=191`, `400 -> 90` over `522..=610`,
//!      `180 -> 90` over `705..=931`, and `400 -> 160` in `CS_PS_ANGELA_BOSS_START` over `0..=211`).
//!    - **Maria Jail Cell Cutscene (`CS_PS_MARIA_MEET = 0x34`, `demo_no = 52`, `mar_saikai` in `SpecialFX.cpp:358..363`)**:
//!      Restores the 3-tier PS2 motion blur factors (`80`, `100`, `400`).
//!    - **Eddie Boss Death (`enEDBCtrlDead` at `0x004B3DA4`) & Mary/Maria Final Boss (`SpecialFX.cpp:178..286`)**:
//!      Scales Eddie's death timer increment by `0.375f` (completing at `>= 8.0f`) and Mary/Maria Boss transformation/death blur by `1.25f`.

use sh2_assets::formats::demo::AUTHORED_4X3_ASPECT;

use super::atmosphere::R_HTL_RM_312_ID;
use super::stage_fixes::{
    CLOSET_BLURRED_BARS_FIXED_DESPAWN_FRAME, CLOSET_BLURRED_BARS_HIDE_CAM_X,
    CS_APT_RPT_CLOSET_DEMO_NO, CS_PS_ANGELA_BOSS_FINISH_DEMO_NO,
};

/// `demo_no == 1` (`0x01`, `CS_INTRO_BATHROOM`, `first_toilet` Restroom Opening).
pub const CS_INTRO_BATHROOM_DEMO_NO: u16 = 1;

/// `demo_no == 6` (`0x06`, `CS_TUNNEL_RADIO`, `first_battle` Tunnel Radio & First Monster).
pub const CS_TUNNEL_RADIO_DEMO_NO: u16 = 6;

/// `demo_no == 42` (`0x2A`, `CS_HSP_BOSS_FINISH`, Brookhaven Flesh Lips Post-Boss Cutscene).
pub const CS_HSP_BOSS_FINISH_DEMO_NO: u16 = 42;

/// `demo_no == 52` (`0x34`, `CS_PS_MARIA_MEET`, `mar_saikai` Labyrinth Maria Jail Cell Cutscene).
pub const CS_PS_MARIA_MEET_DEMO_NO: u16 = 52;

/// `demo_no == 67` (`0x43`, `CS_PS_ANGELA_BOSS_START`, `papa_agl_a` Abstract Daddy Pre-Boss Cutscene).
pub const CS_PS_ANGELA_BOSS_START_DEMO_NO: u16 = 67;

/// Reference vertical resolution (`1080.0` in `SpecialFX.cpp:66`) used to compute `DisplayRatio = viewport_height / 1080.0`.
pub const SPECIAL_FX_REFERENCE_HEIGHT: f32 = 1080.0;

/// Global PC-to-PS2 motion blur retention scale (`MotionBlurValue = 0.25f` at `0x004782AE` in `SpecialFX.cpp:20`).
pub const PS2_MOTION_BLUR_SCALE: f32 = 0.25;

/// Flesh Lips post-boss cutscene (`CS_HSP_BOSS_FINISH = 0x2A`) motion blur blend factor (`300` at `0x0058B795` in `PistonRoom.cpp:176`).
pub const FLESH_LIPS_CUTSCENE_MOTION_BLUR_FACTOR: f32 = 300.0;

/// Flesh Lips boss death (`enIKECtrlDead` at `0x004B9EA5`) default PS2 motion blur factor when `enemies_remaining == 0`.
pub const FLESH_LIPS_DEATH_MOTION_BLUR_FACTOR: f32 = 180.0;

/// Eddie Boss death blur timer rate scale (`0.375f` in `SpecialFX.cpp:22`).
pub const EDDIE_BOSS_DEATH_TIMER_SCALE: f32 = 0.375;

/// Eddie Boss death blur completion threshold (`8.0f` in `SpecialFX.cpp:23`).
pub const EDDIE_BOSS_DEATH_TIMER_CHECK: f32 = 8.0;

/// Mary/Maria Final Boss transformation and death motion blur multiplier (`1.25f` in `SpecialFX.cpp:25`).
pub const MARY_BOSS_BLUR_SCALE: f32 = 1.25;

/// Maria Jail Cell (`CS_PS_MARIA_MEET = 0x34`) 3-tier PS2 motion blur constants (`[80, 100, 400]` in `SpecialFX.cpp:358..363`).
pub const MARIA_JAIL_CELL_BLUR_FACTORS: [f32; 3] = [80.0, 100.0, 400.0];

/// Linear motion blur interpolation window `(frame_start, frame_end, blur_start, blur_end)`
/// matching `0x005815D0` (`SpecialFX.cpp:365..388`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MotionBlurRampWindow {
    /// Cutscene `demo_no` (`67` for `CS_PS_ANGELA_BOSS_START`, `68` for `CS_PS_ANGELA_BOSS_FINISH`).
    pub demo_no: u16,
    /// Starting 30 FPS scene frame.
    pub frame_start: f32,
    /// Ending 30 FPS scene frame.
    pub frame_end: f32,
    /// PS2 motion blur factor at `frame_start` (`AngelaAbstractDaddy1/3/5/7`).
    pub blur_start: f32,
    /// PS2 motion blur factor at `frame_end` (`AngelaAbstractDaddy2/4/6/8`).
    pub blur_end: f32,
}

impl MotionBlurRampWindow {
    /// Evaluates the linearly interpolated motion blur factor if `(demo_no, scene_frame)` falls inside this window.
    #[inline]
    #[must_use]
    pub fn sample(&self, demo_no: u16, scene_frame: f32) -> Option<f32> {
        if demo_no != self.demo_no || scene_frame < self.frame_start || scene_frame > self.frame_end
        {
            return None;
        }
        let span = (self.frame_end - self.frame_start).max(1e-5);
        let t = ((scene_frame - self.frame_start) / span).clamp(0.0, 1.0);
        Some(self.blur_start + (self.blur_end - self.blur_start) * t)
    }
}

/// Complete table of Angela / Abstract Daddy (`papa_agl_a` & `papa_agl`) PS2 motion blur ramps
/// from `0x008D0610..0x008D063C` and `0x008D175C..0x008D1768` (`SpecialFX.cpp:365..388`).
pub const ANGELA_ABSTRACT_DADDY_BLUR_RAMPS: [MotionBlurRampWindow; 4] = [
    // Ramp 1 (`0x0058162A`): frames 1.0..=191.0, 400.0 -> 30.0 (retail had 120.0 -> 60.0)
    MotionBlurRampWindow {
        demo_no: CS_PS_ANGELA_BOSS_FINISH_DEMO_NO,
        frame_start: 1.0,
        frame_end: 191.0,
        blur_start: 400.0,
        blur_end: 30.0,
    },
    // Ramp 2 (`0x0058168D`): frames 522.0..=610.0, 400.0 -> 90.0 (retail had 90.0 -> 80.0)
    MotionBlurRampWindow {
        demo_no: CS_PS_ANGELA_BOSS_FINISH_DEMO_NO,
        frame_start: 522.0,
        frame_end: 610.0,
        blur_start: 400.0,
        blur_end: 90.0,
    },
    // Ramp 3 (`0x005816F0`): frames 705.0..=931.0, 180.0 -> 90.0 (retail had 80.0 -> 50.0)
    MotionBlurRampWindow {
        demo_no: CS_PS_ANGELA_BOSS_FINISH_DEMO_NO,
        frame_start: 705.0,
        frame_end: 931.0,
        blur_start: 180.0,
        blur_end: 90.0,
    },
    // Ramp 4 (`0x005834D2`): frames 0.0..=211.0, 400.0 -> 160.0 (retail had 100.0 -> 30.0)
    MotionBlurRampWindow {
        demo_no: CS_PS_ANGELA_BOSS_START_DEMO_NO,
        frame_start: 0.0,
        frame_end: 211.0,
        blur_start: 400.0,
        blur_end: 160.0,
    },
];

/// Input state for evaluating PS2 DoF (`Kari_hisyakai`) and Motion Blur (`enSetBlur`) (`SH2-105`).
#[derive(Debug, Clone, Copy)]
pub struct SpecialFxEvalInput {
    /// Active stage room ID (e.g. `R_HTL_RM_312 = 0xA2`).
    pub room_id: u32,
    /// Active `.dds` cutscene number (`0` when in normal gameplay).
    pub demo_no: u16,
    /// Current 30 FPS cutscene frame (`0.0` when in normal gameplay).
    pub scene_frame: f32,
    /// Current world-space camera X coordinate.
    pub camera_pos_x: f32,
    /// Current render target height in pixels (used to compute `display_ratio = height / 1080.0`).
    pub viewport_height: f32,
    /// Current render target aspect ratio (`width / height`).
    pub viewport_aspect: f32,
    /// Number of living enemies remaining in the current boss room (`0` when the last Flesh Lips is killed).
    pub enemies_remaining: u32,
    /// Optional Flesh Lips death state-machine sub-state (`byte[esi + 3]` in `enIKECtrlDead` at `0x004B9EA5`).
    pub flesh_lips_death_substate: Option<u8>,
}

impl Default for SpecialFxEvalInput {
    fn default() -> Self {
        Self {
            room_id: 0,
            demo_no: 0,
            scene_frame: 0.0,
            camera_pos_x: 0.0,
            viewport_height: SPECIAL_FX_REFERENCE_HEIGHT,
            viewport_aspect: 16.0 / 9.0,
            enemies_remaining: 1,
            flesh_lips_death_substate: None,
        }
    }
}

/// Evaluated PS2 Depth-of-Field (`Kari_hisyakai`) and Motion Blur (`enSetBlur`) state for a frame (`SH2-105`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpecialFxEvaluation {
    /// Resolution scale factor (`viewport_height / 1080.0`).
    pub display_ratio: f32,
    /// Whether the PS2 `Kari_hisyakai` screen-space DoF / closet bar blur filter (`EffectMode = 5`) is active.
    pub dof_active: bool,
    /// Resolution-scaled 2-tap pixel offset `(val1, val2)` from `UpdateDOFValues` (`SpecialFX.cpp:62..97`).
    pub dof_tap_offset_px: [f32; 2],
    /// Normalized DoF / closet bar blur blend weight (`[0.0, 1.0]`) passed to `PostUniforms`.
    pub dof_strength: f32,
    /// Whether the signed-byte DoF offset write (`0x00942636`) is suppressed at `CameraX == -21376.56445f` (`DepthOfFieldASM`).
    pub closet_dof_byte_write_suppressed: bool,
    /// Raw PS2 motion blur retention factor (`0.0..=400.0`).
    pub raw_motion_blur_factor: f32,
    /// Scaled motion blur retention factor (`raw_motion_blur_factor * 0.25`).
    pub scaled_motion_blur_factor: f32,
    /// Normalized shader motion blur strength (`[0.0, 1.0]`) passed to `PostUniforms`.
    pub motion_blur_strength: f32,
    /// Whether the lowest texture alpha bit (`| 0x01`) is forced (`MotionBlurTexAlphaASM` in `CS_HSP_BOSS_FINISH`).
    pub force_motion_blur_alpha_lsb: bool,
    /// Guarantees the PS2 film grain pass executes *after* the DoF / motion blur pass (`ClosetCutscene.cpp:209`).
    pub render_grain_above_blur: bool,
}

impl Default for SpecialFxEvaluation {
    fn default() -> Self {
        Self {
            display_ratio: 1.0,
            dof_active: false,
            dof_tap_offset_px: [1.0 / 3.0, 2.0 / 3.0],
            dof_strength: 0.0,
            closet_dof_byte_write_suppressed: false,
            raw_motion_blur_factor: 0.0,
            scaled_motion_blur_factor: 0.0,
            motion_blur_strength: 0.0,
            force_motion_blur_alpha_lsb: false,
            render_grain_above_blur: true,
        }
    }
}

impl SpecialFxEvaluation {
    /// Evaluates PS2 DoF (`Kari_hisyakai`) and Motion Blur (`enSetBlur`) for [`SpecialFxEvalInput`].
    #[must_use]
    pub fn evaluate(input: SpecialFxEvalInput) -> Self {
        let display_ratio = (input.viewport_height.max(240.0)) / SPECIAL_FX_REFERENCE_HEIGHT;

        // 1. Evaluate PS2 DoF / Slat Blur (`UpdateDOFValues` in `SpecialFX.cpp:62..97` & `ClosetCutscene.cpp`)
        let at_closet_bars_cam = input.demo_no == CS_APT_RPT_CLOSET_DEMO_NO
            && (input.camera_pos_x - CLOSET_BLURRED_BARS_HIDE_CAM_X).abs() < 1.0;

        let (dof_active, base_val1, base_val2, base_strength) = match input.demo_no {
            CS_INTRO_BATHROOM_DEMO_NO => (true, -3.0, -1.5, 0.45),
            CS_TUNNEL_RADIO_DEMO_NO => (true, -3.0, -1.0, 0.45),
            CS_APT_RPT_CLOSET_DEMO_NO => {
                let active = input.scene_frame < CLOSET_BLURRED_BARS_FIXED_DESPAWN_FRAME;
                let width_scale = (input.viewport_aspect / AUTHORED_4X3_ASPECT).max(1.0);
                let strength = if at_closet_bars_cam { 0.85 } else { 0.55 };
                (
                    active,
                    1.5 * width_scale,
                    -1.5,
                    if active { strength } else { 0.0 },
                )
            }
            _ if input.room_id == R_HTL_RM_312_ID => (true, -2.0, -4.0, 0.35),
            _ => (false, 1.0 / 3.0, 2.0 / 3.0, 0.0),
        };

        let dof_tap_offset_px = [base_val1 * display_ratio, base_val2 * display_ratio];

        // 2. Evaluate PS2 Motion Blur (`enSetBlur` / `MotionBlurASM` in `SpecialFX.cpp` & `PistonRoom.cpp`)
        let mut raw_motion_blur_factor = 0.0f32;
        let mut force_motion_blur_alpha_lsb = false;

        // 2a. Flesh Lips Boss Death (`enIKECtrlDead` at `0x004B9EA5` in `PistonRoom.cpp:113..133`):
        // Calls `enSetBlur` when `enemies_remaining == 0`, `demo_no == 0` (`CS_NONE`), and `sub_state in {2, 4, 6}`.
        if input.demo_no == 0
            && input.enemies_remaining == 0
            && matches!(input.flesh_lips_death_substate, Some(2 | 4 | 6))
        {
            raw_motion_blur_factor = FLESH_LIPS_DEATH_MOTION_BLUR_FACTOR;
        }

        // 2b. Flesh Lips Post-Boss Cutscene (`CS_HSP_BOSS_FINISH = 0x2A`, `demo_no = 42` in `PistonRoom.cpp:144..235`):
        if input.demo_no == CS_HSP_BOSS_FINISH_DEMO_NO {
            raw_motion_blur_factor = FLESH_LIPS_CUTSCENE_MOTION_BLUR_FACTOR;
            force_motion_blur_alpha_lsb = true;
        }

        // 2c. Angela Abstract Daddy Cutscene Ramps (`CS_PS_ANGELA_BOSS_START = 67` & `CS_PS_ANGELA_BOSS_FINISH = 68`):
        for ramp in &ANGELA_ABSTRACT_DADDY_BLUR_RAMPS {
            if let Some(factor) = ramp.sample(input.demo_no, input.scene_frame) {
                raw_motion_blur_factor = factor;
                break;
            }
        }

        // 2d. Maria Jail Cell Cutscene (`CS_PS_MARIA_MEET = 0x34`, `demo_no = 52`):
        if input.demo_no == CS_PS_MARIA_MEET_DEMO_NO {
            raw_motion_blur_factor = if input.scene_frame < 600.0 {
                MARIA_JAIL_CELL_BLUR_FACTORS[0]
            } else if input.scene_frame < 1800.0 {
                MARIA_JAIL_CELL_BLUR_FACTORS[1]
            } else {
                MARIA_JAIL_CELL_BLUR_FACTORS[2]
            };
        }

        let scaled_motion_blur_factor = raw_motion_blur_factor * PS2_MOTION_BLUR_SCALE;
        let motion_blur_strength = (scaled_motion_blur_factor / 128.0).clamp(0.0, 1.0);

        Self {
            display_ratio,
            dof_active,
            dof_tap_offset_px,
            dof_strength: if dof_active { base_strength } else { 0.0 },
            closet_dof_byte_write_suppressed: at_closet_bars_cam,
            raw_motion_blur_factor,
            scaled_motion_blur_factor,
            motion_blur_strength,
            force_motion_blur_alpha_lsb,
            render_grain_above_blur: true,
        }
    }
}

/// Evaluates Eddie Boss death motion blur timer progression (`EddieBossDeathASM` at `0x004B3DA4` in `SpecialFX.cpp:178..216`).
///
/// Advances `current_timer` by `delta_step * 0.375` (`EddieBossDeathTimerScale`) and returns `(new_timer, blur_finished)`
/// where `blur_finished` becomes `true` once `new_timer >= 8.0` (`EddieBossDeathTimerCheck`).
#[inline]
#[must_use]
pub fn evaluate_eddie_boss_death_blur(current_timer: f32, delta_step: f32) -> (f32, bool) {
    let next_timer = current_timer + delta_step.max(0.0) * EDDIE_BOSS_DEATH_TIMER_SCALE;
    let finished = next_timer >= EDDIE_BOSS_DEATH_TIMER_CHECK;
    (next_timer, finished)
}

/// Scales Mary/Maria Final Boss transformation and death motion blur intensity by `1.25f`
/// (`MaryBossTransformationASM` at `0x004C655D` & `MaryBossDeathASM` at `0x004C8923` in `SpecialFX.cpp:219..286`).
#[inline]
#[must_use]
pub fn evaluate_mary_boss_blur_factor(base_factor: f32) -> f32 {
    base_factor * MARY_BOSS_BLUR_SCALE * PS2_MOTION_BLUR_SCALE
}
