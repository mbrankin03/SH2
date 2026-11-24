//! Toluca Lake Rowboat subsystem (`Chacter/m3_boat.c`, `m3_boat_jms.c`, `lake_wave.c`,
//! `Effect2/hh_class_water_*.c`, `player_result.c`).
//!
//! Pure 60 Hz deterministic simulation with zero rendering/audio/windowing dependencies.

pub mod input;
pub mod physics;
pub mod wave;

#[cfg(test)]
mod tests;

use glam::Vec3;
use std::f32::consts::TAU;

use crate::endings::PlayingInfo;
use crate::timing::{lerp_angle_shortest, TimingConstants};

pub use input::{
    regulate_angle, BoatRowMode, RowboatControlMode, RowboatInput, RowboatInputProcessor,
    BOAT_STATUS_TABLE,
};
pub use physics::{
    close_to_value, collect_oar_events, resolve_sh2ee_rowboat_spawn, sync_james_to_boat_pose,
    BoatJamesSyncPose, RowboatOarEvent, RowboatOarState, RowboatWaveState,
    BOAT_ANGULAR_ACCEL_SCALE, BOAT_ANGULAR_DAMPING, BOAT_ANGULAR_DEADBAND, BOAT_ANIME_ACCEL_STEP,
    BOAT_ANIME_DECEL_STEP, BOAT_IDLE_ANIME_SPEED_Q12, BOAT_IDLE_ANIM_EXCLUDE_MAX,
    BOAT_IDLE_ANIM_EXCLUDE_MIN, BOAT_LINEAR_ACCEL_SCALE, BOAT_LINEAR_DAMPING, BOAT_LINEAR_DEADBAND,
    BOAT_MAX_ANGULAR_SPEED, BOAT_MAX_ANIME_SPEED_Q12, BOAT_MAX_FORWARD_SPEED_PC,
    BOAT_MIN_FORWARD_SPEED, BOAT_OAR_WATER_THRESHOLD_MM, BOAT_WATER_BASE_OFFSET_PC,
    BOAT_WATER_BASE_OFFSET_PS2, BOAT_WORLD_SCALE_MM, FLAG_ENTERED_ROWBOAT,
    ROWBOAT_ANIM_FIX_MAX_FRAME, ROWBOAT_HOTEL_DOCK_POS_MM, ROWBOAT_HOTEL_DOCK_YAW_RAD,
    SFX_BOAT_OAR_LEFT, SFX_BOAT_OAR_RIGHT,
};
pub use wave::{
    ps2_water_specular, LakeWaveRing, LakeWaveSample, LakeWaveSystem, WaterGridSim,
    LAKE_WAVE_COUNT, LAKE_WAVE_MAX_POS, WATER_GRID_DAMPING_DIVISOR, WATER_GRID_NEIGHBOR_DIVISOR,
    WATER_GRID_SIZE, WATER_SPECULAR_COS_CUTOFF,
};

/// Outputs emitted by one 60 Hz tick of [`RowboatController::tick_full`].
#[derive(Debug, Clone, Default)]
pub struct RowboatTickOutput {
    /// Active rowing mode (`Both`, `Right`, `Left`).
    pub row_mode: BoatRowMode,
    /// James boat animation ID (`0x4e24`, `0x4e25`, `0x4e26`).
    pub james_anim_id: u16,
    /// Signed Q12 animation speed (`_DAT_0120256c` / `DAT_01f96820`).
    pub anim_speed_q12: i16,
    /// Synchronized James pose (`m3_boat_jms.c` `BoatPlayerCheckControl` `0x0026be30`).
    pub james_sync: BoatJamesSyncPose,
    /// Oar water entry/exit splash & SFX events (`BoatCheckEffect`).
    pub oar_events: Vec<RowboatOarEvent>,
}

/// Deterministic 60 Hz Toluca Lake rowboat controller (`Chacter/m3_boat.c` & `m3_boat_jms.c`).
#[derive(Debug, Clone)]
pub struct RowboatController {
    /// Previous tick world position in mm (for sub-frame interpolation).
    pub prev_position: Vec3,
    /// Current world position in mm (`_sh2bot + 0x20/0x24/0x28`).
    pub position: Vec3,
    /// Previous tick yaw angle in radians.
    pub prev_yaw: f32,
    /// Current yaw angle in radians (`_sh2bot + 0x34` SLUS / `+0x30` PC).
    pub yaw: f32,
    /// Current boat roll angle in radians (`_sh2bot + 0x38` SLUS / `+0x34` PC).
    pub roll: f32,
    /// Current boat pitch angle in radians (`_sh2bot + 0x30` SLUS / `+0x2c` PC).
    pub pitch: f32,
    /// Forward/reverse speed (`_sh2bot + 0xac` SLUS / `+0xa8` PC).
    pub forward_speed: f32,
    /// Yaw angular velocity (`_sh2bot + 0x3c` SLUS / `+0x38` PC).
    pub angular_velocity: f32,
    /// Previous tick oar animation frame (for sub-frame interpolation).
    pub prev_oar_anim_frame: f32,
    /// Current oar animation frame (`0.0..=76.0`, capped at `0x4C` per `RowboatAnimation.cpp`).
    pub oar_anim_frame: f32,
    /// Active rowing mode (`DAT_01202554` / `DAT_01f96804`).
    pub row_mode: BoatRowMode,
    /// Previous rowing mode (`anime_change_check_1323` / `DAT_008bb78c`).
    pub prev_row_mode: BoatRowMode,
    /// Q12 left oar animation speed (`_anm_speed_left` / `DAT_01d7f760`).
    pub anm_speed_left: i16,
    /// Q12 right oar animation speed (`_anm_speed_right` / `DAT_01d7f758`).
    pub anm_speed_right: i16,
    /// Combined Q12 animation speed (`_DAT_0120256c` / `DAT_01f96820`).
    pub anim_speed_q12: i16,
    /// Input stick history & power processor (`BoatCheckKeyInput`).
    pub input_processor: RowboatInputProcessor,
    /// Heave/roll/pitch wave energy state (`BoatCheckRollEnergy`).
    pub wave_state: RowboatWaveState,
    /// Concentric Toluca Lake wave rings (`lake_wave.c`).
    pub lake_waves: LakeWaveSystem,
    /// Oar water submersion state (`BoatCheckOarStatus`).
    pub oar_state: RowboatOarState,
    /// Base water height offset (`7.38` PC vs `7.18` PS2).
    pub water_base_offset: f32,
    /// Maximum forward speed (`3.5` PC vs `4.0` PS2).
    pub max_forward_speed: f32,
}

impl Default for RowboatController {
    fn default() -> Self {
        Self::new(Vec3::ZERO, 0.0)
    }
}

impl RowboatController {
    /// Maximum oar stroke frame before wrap (`0x4C` = `76` in `RowboatAnimation.cpp`).
    pub const MAX_OAR_STROKE_FRAME: f32 = ROWBOAT_ANIM_FIX_MAX_FRAME;
    /// Maximum forward speed in retail PC (`3.5` at `0x0053f1d0`).
    pub const MAX_FORWARD_SPEED: f32 = BOAT_MAX_FORWARD_SPEED_PC;
    /// Maximum turn rate (`1.5` rad/s at `0x0053f1d0`).
    pub const MAX_TURN_RATE: f32 = BOAT_MAX_ANGULAR_SPEED;
    /// Q12 fixed-point (`4096.0`) at 30 Hz (`1 / 2` per 60 Hz tick) -> `8192.0` Q12 units per frame per 60 Hz tick.
    pub const Q12_TO_60HZ_FRAME_DIVISOR: f32 = 4096.0 * 2.0;

    /// Initializes the rowboat at `(position, yaw)` (`HumanBOTInit` `0x0026c170`).
    pub fn new(position: Vec3, yaw: f32) -> Self {
        Self {
            prev_position: position,
            position,
            prev_yaw: yaw,
            yaw,
            roll: 0.0,
            pitch: 0.0,
            forward_speed: 0.0,
            angular_velocity: 0.0,
            prev_oar_anim_frame: 0.0,
            oar_anim_frame: 0.0,
            row_mode: BoatRowMode::Both,
            prev_row_mode: BoatRowMode::Both,
            anm_speed_left: 0,
            anm_speed_right: 0,
            anim_speed_q12: 0,
            input_processor: RowboatInputProcessor::new(),
            wave_state: RowboatWaveState::default(),
            lake_waves: LakeWaveSystem::new(),
            oar_state: RowboatOarState::default(),
            water_base_offset: BOAT_WATER_BASE_OFFSET_PC,
            max_forward_speed: BOAT_MAX_FORWARD_SPEED_PC,
        }
    }

    /// Legacy compatibility wrapper for `SH2-56` callers (`left_oar_input`, `right_oar_input` in `[-1, 1]`).
    ///
    /// Routes through [`Self::tick_full`] using `SingleStick2D`-equivalent direct oar powers.
    pub fn tick(
        &mut self,
        left_oar_input: f32,
        right_oar_input: f32,
        _great_knife_equipped: bool,
        constants: &TimingConstants,
    ) {
        let left = left_oar_input.clamp(-1.0, 1.0) * 0.5 * constants.boat_rowing_impulse;
        let right = right_oar_input.clamp(-1.0, 1.0) * 0.5 * constants.boat_rowing_impulse;
        let input = RowboatInput {
            left_trigger: left.max(0.0) * 2.0,
            right_trigger: right.max(0.0) * 2.0,
            trigger_reverse: left < 0.0 && right < 0.0,
            enable_analog_trigger_rowing: true,
            full_power_mode: true,
            current_boat_anim_id: 0x20,
            ..Default::default()
        };
        let _ = self.tick_full(&input, None, true);
    }

    /// Advances rowboat physics, wave coupling, oar animation sync, and `PlayingInfo` metrics
    /// for one 60 Hz tick (`HumanBOTFunction` `0x0026e970` / `m3_boat.c`).
    pub fn tick_full(
        &mut self,
        input: &RowboatInput,
        playing_info: Option<&mut PlayingInfo>,
        enable_sh2ee_anim_fix: bool,
    ) -> RowboatTickOutput {
        let dt = 1.0 / 60.0;
        self.prev_position = self.position;
        self.prev_yaw = self.yaw;
        self.prev_oar_anim_frame = self.oar_anim_frame;

        // 1. BoatCheckKeyInput (`0x0026c420` / `0x0053eb10`)
        self.input_processor.process(input, self.forward_speed);

        // 2. BoatCheckSetParameter (`0x0026cd40` / `0x0053efa0`) & PlayingInfo metrics
        if let Some(info) = playing_info {
            info.boat_clear_time += dt; // GameBoatTimerCountUp (`0x00290450`)
            if self.forward_speed > info.boat_max_speed {
                info.boat_max_speed = self.forward_speed; // GameBoatMaxSpeedCheck (`0x00290490`)
            }
        }
        if self.input_processor.mode_cooldown > 0.0 {
            self.input_processor.mode_cooldown = (self.input_processor.mode_cooldown - dt).max(0.0);
        }
        self.position.y = (self.wave_state.height_y + self.water_base_offset) * BOAT_WORLD_SCALE_MM;

        // 3. BoatCheckStatus (`0x0026d000`) & BoatCheckAnime (`0x0026dd30` / `0x0053f960`)
        let pending = self.input_processor.pending_row_mode;
        self.row_mode = match (self.row_mode, pending) {
            (BoatRowMode::Both, BoatRowMode::Right) => BoatRowMode::Right,
            (BoatRowMode::Both, BoatRowMode::Left) => BoatRowMode::Left,
            (BoatRowMode::Right, BoatRowMode::Right) => BoatRowMode::Right,
            (BoatRowMode::Right, BoatRowMode::Both) => BoatRowMode::Both,
            (BoatRowMode::Left, BoatRowMode::Left) => BoatRowMode::Left,
            (BoatRowMode::Left, BoatRowMode::Both) => BoatRowMode::Both,
            (current, _) => current,
        };
        if self.row_mode != self.prev_row_mode {
            self.prev_row_mode = self.row_mode;
        }

        // 4. BoatCheckControl (`0x0026d040` / `0x0053f1d0`)
        self.forward_speed -= self.forward_speed * BOAT_LINEAR_DAMPING * dt;
        self.angular_velocity -= self.angular_velocity * BOAT_ANGULAR_DAMPING * dt;

        if self.forward_speed.abs() < BOAT_LINEAR_DEADBAND
            && self.input_processor.straight_power == 0.0
        {
            self.forward_speed = 0.0;
        }
        if self.angular_velocity.abs() < BOAT_ANGULAR_DEADBAND
            && self.input_processor.round_power == 0.0
        {
            self.angular_velocity = 0.0;
        }

        let pos_aspd = self.input_processor.straight_power * BOAT_LINEAR_ACCEL_SCALE;
        self.forward_speed = (self.forward_speed + pos_aspd * dt)
            .clamp(BOAT_MIN_FORWARD_SPEED, self.max_forward_speed);

        let rot_aspd = (self.input_processor.round_power * BOAT_ANGULAR_ACCEL_SCALE)
            / (self.forward_speed.abs() + 1.0);
        self.angular_velocity = (self.angular_velocity + rot_aspd * dt)
            .clamp(-BOAT_MAX_ANGULAR_SPEED, BOAT_MAX_ANGULAR_SPEED);

        let heading = regulate_angle(self.yaw);
        let (sy, cy) = heading.sin_cos();
        self.position.x += self.forward_speed * dt * sy * BOAT_WORLD_SCALE_MM;
        self.position.z += self.forward_speed * dt * cy * BOAT_WORLD_SCALE_MM;
        self.yaw = regulate_angle(self.yaw + self.angular_velocity * dt).rem_euclid(TAU);

        let (roll, pitch) = self.wave_state.integrate_control(dt);
        self.roll = roll;
        self.pitch = pitch;

        // 5. BoatCheckAnimeSpeed (`0x0026da20` / `0x0053f770`)
        let target_l = (self.input_processor.lstick_power * 4096.0 * 8.0) as i16;
        let target_r = (self.input_processor.rstick_power * 4096.0 * 8.0) as i16;
        let step_l = if self.anm_speed_left.unsigned_abs() < target_l.unsigned_abs() {
            BOAT_ANIME_ACCEL_STEP
        } else {
            BOAT_ANIME_DECEL_STEP
        };
        let step_r = if self.anm_speed_right.unsigned_abs() < target_r.unsigned_abs() {
            BOAT_ANIME_ACCEL_STEP
        } else {
            BOAT_ANIME_DECEL_STEP
        };
        self.anm_speed_left = close_to_value(self.anm_speed_left, target_l, step_l)
            .clamp(-BOAT_MAX_ANIME_SPEED_Q12, BOAT_MAX_ANIME_SPEED_Q12);
        self.anm_speed_right = close_to_value(self.anm_speed_right, target_r, step_r)
            .clamp(-BOAT_MAX_ANIME_SPEED_Q12, BOAT_MAX_ANIME_SPEED_Q12);

        self.anim_speed_q12 = match self.row_mode {
            BoatRowMode::Left | BoatRowMode::Both => self.anm_speed_left.saturating_mul(2),
            BoatRowMode::Right => self.anm_speed_right.saturating_mul(2),
        };

        // Retail PC idle animation speed override (`0x0053f770` lines 9413-9420):
        // When both sticks are idle (`status & 2`) and anim_id is outside `0x28..=0x2a`,
        // apply `+0x400` if `forward_speed >= 0.0` else `-0x400`.
        if (self.input_processor.lstick_status & 2) != 0
            && (self.input_processor.rstick_status & 2) != 0
            && (input.current_boat_anim_id < BOAT_IDLE_ANIM_EXCLUDE_MIN
                || input.current_boat_anim_id > BOAT_IDLE_ANIM_EXCLUDE_MAX)
        {
            self.anim_speed_q12 = if self.forward_speed >= 0.0 {
                BOAT_IDLE_ANIME_SPEED_Q12
            } else {
                -BOAT_IDLE_ANIME_SPEED_Q12
            };
        }

        // Advance oar stroke frame from Q12 speed & enforce SH2:EE RowboatAnimationFix at frame 0x4C (76.0).
        if self.anim_speed_q12 != 0 {
            let frame_delta = self.anim_speed_q12 as f32 / Self::Q12_TO_60HZ_FRAME_DIVISOR;
            self.oar_anim_frame += frame_delta;
            if enable_sh2ee_anim_fix {
                while self.oar_anim_frame > Self::MAX_OAR_STROKE_FRAME {
                    self.oar_anim_frame -= Self::MAX_OAR_STROKE_FRAME;
                }
                while self.oar_anim_frame < 0.0 {
                    self.oar_anim_frame += Self::MAX_OAR_STROKE_FRAME;
                }
            }
        }

        // 6. BoatCheckOarStatus (`0x0026def0` / `0x0053fa60`) & BoatCheckEffect (`0x0026e040` / `0x0053fb40`)
        // Oar tip Y > 3500.0 mm (`BOAT_OAR_WATER_THRESHOLD_MM`) indicates water submersion.
        // Callers with evaluated `boat.mdl` bone transforms can override via `update_oar_status_from_bone_y`.
        let stroke_phase = (self.oar_anim_frame / Self::MAX_OAR_STROKE_FRAME) * TAU;
        let stroke_submerged = stroke_phase.sin() > 0.0 && self.anim_speed_q12 != 0;
        self.oar_state.prev_left_in_water = self.oar_state.left_in_water;
        self.oar_state.prev_right_in_water = self.oar_state.right_in_water;
        self.oar_state.left_in_water = match self.row_mode {
            BoatRowMode::Both | BoatRowMode::Left => {
                stroke_submerged && self.position.y > BOAT_OAR_WATER_THRESHOLD_MM
            }
            BoatRowMode::Right => false,
        };
        self.oar_state.right_in_water = match self.row_mode {
            BoatRowMode::Both | BoatRowMode::Right => {
                stroke_submerged && self.position.y > BOAT_OAR_WATER_THRESHOLD_MM
            }
            BoatRowMode::Left => false,
        };

        let oar_events = collect_oar_events(
            &self.oar_state,
            self.position,
            self.yaw,
            self.anim_speed_q12,
        );

        // 7. BoatCheckRollEnergy (`0x0026e150` / `0x0053fd60`) & lake_wave.c (`0x0026f050`)
        let wave_sample = self.lake_waves.update(self.position, dt);
        self.wave_state.apply_roll_energy(
            self.input_processor.round_power,
            self.input_processor.straight_power,
            self.yaw,
            wave_sample,
            dt,
        );

        // 8. m3_boat_jms.c `BoatPlayerCheckControl` (`0x0026be30`) & `shUpdateBoatJamesPosAfterAnime` (`0x0026ee10`)
        let james_sync = sync_james_to_boat_pose(
            self.position,
            self.yaw,
            self.pitch,
            self.roll,
            self.oar_anim_frame,
            self.anim_speed_q12,
        );

        RowboatTickOutput {
            row_mode: self.row_mode,
            james_anim_id: self.row_mode.james_anim_id(),
            anim_speed_q12: self.anim_speed_q12,
            james_sync,
            oar_events,
        }
    }

    /// Evaluates `BoatCheckOarStatus` (`0x0026def0` / `0x0053fa60`) & `BoatCheckEffect` (`0x0053fb40`)
    /// directly from `boat.mdl` skeletal oar-tip world Y coordinates (`> 3500.0` mm).
    pub fn update_oar_status_from_bone_y(
        &mut self,
        left_oar_world_y_mm: f32,
        right_oar_world_y_mm: f32,
    ) -> Vec<RowboatOarEvent> {
        self.oar_state.prev_left_in_water = self.oar_state.left_in_water;
        self.oar_state.prev_right_in_water = self.oar_state.right_in_water;
        self.oar_state.left_in_water = left_oar_world_y_mm > BOAT_OAR_WATER_THRESHOLD_MM;
        self.oar_state.right_in_water = right_oar_world_y_mm > BOAT_OAR_WATER_THRESHOLD_MM;
        collect_oar_events(
            &self.oar_state,
            self.position,
            self.yaw,
            self.anim_speed_q12,
        )
    }

    /// Evaluates sub-frame interpolated `(position, yaw, oar_frame)` for `alpha` in `[0.0, 1.0]`.
    #[inline]
    pub fn interpolated_pose(&self, alpha: f32) -> (Vec3, f32, f32) {
        let t = alpha.clamp(0.0, 1.0);
        let pos = self.prev_position.lerp(self.position, t);
        let yaw = lerp_angle_shortest(self.prev_yaw, self.yaw, t);
        let oar = if (self.oar_anim_frame - self.prev_oar_anim_frame).abs()
            > Self::MAX_OAR_STROKE_FRAME * 0.5
        {
            let unwrapped = self.oar_anim_frame + Self::MAX_OAR_STROKE_FRAME;
            (self.prev_oar_anim_frame + (unwrapped - self.prev_oar_anim_frame) * t)
                % Self::MAX_OAR_STROKE_FRAME
        } else {
            self.prev_oar_anim_frame + (self.oar_anim_frame - self.prev_oar_anim_frame) * t
        };
        (pos, yaw, oar)
    }
}
