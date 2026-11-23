//! Per-state locomotion dynamics (`m3_play_3d.c` / `m3_play_common.c`).
//!
//! Extracted from `locomotion/mod.rs` to keep state kinematics isolated and readable (`AGENTS.md` Rule 2).

use std::f32::consts::PI;

use super::tables::{
    state_bit, DIRECTION_OFFSET_EASE_RUN_RAD_PER_SEC, DIRECTION_OFFSET_EASE_WALK_RAD_PER_SEC,
    GREAT_KNIFE_BACK_SPEEDS, GREAT_KNIFE_QUICK_TURN_SPEED_RAD_PER_SEC, GREAT_KNIFE_STRAFE_SPEEDS,
    GREAT_KNIFE_WALK_SPEEDS, QUICK_TURN_SPEED_RAD_PER_SEC, WATER_TURN_MULTIPLIER,
};
use super::{LocomotionInput, LocomotionState, PlayerLocomotionController};

/// Intermediate per-tick kinematic output from [`PlayerLocomotionController::execute_state_dynamics`].
pub(crate) struct StateDynamicsOutput {
    pub target_offset: f32,
    pub offset_ease_rate: f32,
    pub snap_offset_directly: bool,
    pub yaw_rate: f32,
    pub effective_speed_mps: f32,
}

impl PlayerLocomotionController {
    /// Executes the active lower-body state dynamics (`m3_play_3d.c` / `m3_play_common.c`).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn execute_state_dynamics(
        &mut self,
        input: &LocomotionInput,
        current_yaw: &mut f32,
        dt: f32,
        tx: f32,
        ty: f32,
        run_held: bool,
        stamina_limit: f32,
    ) -> StateDynamicsOutput {
        // 3. Execute State Dynamics & Speeds (`m3_play_3d.c` / `m3_play_common.c`)
        let target_offset;
        let mut offset_ease_rate = DIRECTION_OFFSET_EASE_WALK_RAD_PER_SEC;
        let mut snap_offset_directly = false;
        let mut yaw_rate = 0.0f32;
        let effective_speed_mps;
        let frame_idx = (input.anim_frame as usize) % 24;

        match self.state {
            LocomotionState::Stand
            | LocomotionState::Relax
            | LocomotionState::Tired
            | LocomotionState::Alert => {
                self.still_duration += dt;
                self.stamina_frames = (self.stamina_frames - dt * 30.0).max(0.0);
                // Halting deceleration (`PlayerSpeedDownToStand` 0x0021c560): 20 m/s^2 post-run, 10 m/s^2 normally
                let decel = if self.prev_state.is_running() {
                    20.0
                } else {
                    10.0
                };
                self.speed_mps = (self.speed_mps - decel * dt).max(0.0);
                effective_speed_mps = self.speed_mps;
                target_offset = self.direction_offset;
            }
            LocomotionState::TurnLeft | LocomotionState::TurnRight => {
                let decel = if self.prev_state.is_running() {
                    20.0
                } else {
                    10.0
                };
                self.speed_mps = (self.speed_mps - decel * dt).max(0.0);
                effective_speed_mps = self.speed_mps;
                // `lower_lround_3d` (0x00116330) / `lower_lround_3d_nata` (0x00115c10)
                if input.is_great_knife {
                    yaw_rate = if input.is_digital_input { 1.0 } else { tx };
                } else {
                    let base = if input.is_digital_input {
                        2.4
                    } else {
                        2.4 * tx
                    };
                    let run_multiplier = if run_held { 1.5 } else { 1.0 };
                    yaw_rate = base * run_multiplier;
                }
                target_offset = self.direction_offset;
            }
            LocomotionState::Walk => {
                // `lower_walk_3d` (0x00116630) / `lower_walk_3d_nata` (0x00115d30)
                if input.is_great_knife {
                    self.speed_mps = GREAT_KNIFE_WALK_SPEEDS[frame_idx];
                } else if input.is_digital_input {
                    if self.speed_mps > 0.0 && self.prev_state == LocomotionState::Back {
                        self.speed_mps = (self.speed_mps - 7.5 * dt).max(0.0);
                    } else {
                        self.ramp_speed(1.5, 2.5, 5.0, dt);
                    }
                } else {
                    self.speed_mps = ty * 1.5;
                }

                // Aim + Run speed boost (`0x00116630` lines 831-848: 1.5x capped at 2.25 m/s)
                effective_speed_mps = if input.is_aiming && run_held {
                    (self.speed_mps * 1.5).min(2.25)
                } else {
                    self.speed_mps
                };

                if !input.is_aiming {
                    yaw_rate = if input.is_great_knife {
                        if self.speed_mps > 0.5 {
                            0.875
                        } else {
                            0.0
                        }
                    } else if input.is_digital_input {
                        1.9
                    } else {
                        1.9 * tx
                    };
                    if input.in_water {
                        yaw_rate *= WATER_TURN_MULTIPLIER;
                    }
                }
                target_offset = 0.0;
                offset_ease_rate = DIRECTION_OFFSET_EASE_WALK_RAD_PER_SEC;
                self.stamina_frames = (self.stamina_frames - dt * 15.0).max(0.0);
            }
            LocomotionState::Back => {
                // `lower_back_3d` (0x00116c70) / `lower_back_3d_nata` (0x00115e50)
                if input.is_great_knife {
                    self.speed_mps = GREAT_KNIFE_BACK_SPEEDS[frame_idx];
                } else if input.is_digital_input {
                    if self.speed_mps > 0.0 && self.prev_state != LocomotionState::Back {
                        self.speed_mps = (self.speed_mps - 10.0 * dt).max(0.0);
                        if self.speed_mps == 0.0 {
                            self.prev_state = LocomotionState::Back;
                        }
                    } else {
                        self.ramp_speed(1.2, 5.0, 5.0, dt);
                    }
                } else {
                    self.speed_mps = ty * 1.2;
                }

                // Back + Run speed boost (`0x00116c70` lines 977-992: 1.5x capped at 1.8 m/s)
                effective_speed_mps = if run_held {
                    (self.speed_mps * 1.5).min(1.800_000_1)
                } else {
                    self.speed_mps
                };

                if !input.is_aiming {
                    yaw_rate = if input.is_great_knife {
                        if self.speed_mps > 0.5 {
                            0.65
                        } else {
                            0.0
                        }
                    } else if input.is_digital_input {
                        1.3
                    } else {
                        1.3 * tx
                    };
                    if input.in_water {
                        yaw_rate *= WATER_TURN_MULTIPLIER;
                    }
                }
                target_offset = -PI;
                snap_offset_directly = true;
                self.stamina_frames = (self.stamina_frames - dt * 15.0).max(0.0);
            }
            LocomotionState::StrafeLeftWalk | LocomotionState::StrafeRightWalk => {
                // `lower_lswalk_3d` (0x00117320) / `lower_rswalk_3d` (0x001178f0) / `*_nata`
                if input.is_great_knife {
                    self.speed_mps = GREAT_KNIFE_STRAFE_SPEEDS[frame_idx];
                } else if self.speed_mps > 0.0 && self.prev_state == LocomotionState::Back {
                    // Reversal deceleration from Back (`0x00117320` line 1097: -5.1 m/s^2)
                    self.speed_mps = (self.speed_mps - 5.100_000_4 * dt).max(0.0);
                } else {
                    self.ramp_speed(1.3, 1.7, 3.4, dt);
                }

                // Aim + Run speed boost (`0x00117320` lines 1105-1116: 1.5x capped at 1.8 m/s)
                effective_speed_mps = if input.is_aiming && run_held {
                    (self.speed_mps * 1.5).min(1.800_000_1)
                } else {
                    self.speed_mps
                };

                if !input.is_aiming {
                    yaw_rate = if input.is_great_knife {
                        if self.speed_mps > 0.5 {
                            0.875
                        } else {
                            0.0
                        }
                    } else if input.is_digital_input {
                        1.9
                    } else {
                        1.9 * tx
                    };
                }
                target_offset = if self.state == LocomotionState::StrafeLeftWalk {
                    -PI * 0.5
                } else {
                    PI * 0.5
                };
                offset_ease_rate = DIRECTION_OFFSET_EASE_WALK_RAD_PER_SEC;
            }
            LocomotionState::Run1 | LocomotionState::Run2 | LocomotionState::Run3 => {
                // `lower_run1_3d` (0x00117ec0), `lower_run2_3d` (0x001183a0), `lower_run3_3d` (0x001189d0)
                let top_speeds = [3.5f32, 4.0f32, 4.8f32];
                let idx = match self.state {
                    LocomotionState::Run1 => 0,
                    LocomotionState::Run2 => 1,
                    _ => 2,
                };
                let target = if input.is_digital_input {
                    top_speeds[idx]
                } else {
                    top_speeds[idx] * ty
                };

                if self.state == LocomotionState::Run1 {
                    // Run1 (`0x00117ec0`): ZERO stamina drain!
                    self.ramp_speed(target, 3.0, 3.0, dt);
                    if !input.is_digital_input {
                        // Analog Run1 floor 2.0, then capped at target (`0x00117ec0` lines 1328-1335)
                        if self.speed_mps < 2.0 {
                            self.speed_mps = 2.0;
                        }
                        if self.speed_mps > target {
                            self.speed_mps = target;
                        }
                    }
                    if self.speed_mps >= 3.5 {
                        self.allowed_states = (self.allowed_states
                            | state_bit(LocomotionState::Run2))
                            & !state_bit(LocomotionState::Run1);
                    }
                } else if self.state == LocomotionState::Run2 {
                    // Run2 (`0x001183a0`): drains stamina only when Ampoule is NOT active
                    if !input.ampoule_active {
                        self.stamina_frames += dt * 30.0;
                    }
                    self.ramp_speed(target, 3.0, 3.0, dt);
                    // Analog speed-drop downgrade Run2 -> Run1 (`0x001183a0` line 1465)
                    if !input.is_digital_input && self.speed_mps <= 3.5 {
                        self.allowed_states = (self.allowed_states
                            | state_bit(LocomotionState::Run1))
                            & !state_bit(LocomotionState::Run2);
                    }
                    // Upgrade Run2 -> Run3 (`0x001183a0` lines 1469, 1504): requires `!in_water`
                    if self.speed_mps >= 4.0
                        && self.stamina_frames < stamina_limit
                        && !input.in_water
                    {
                        self.allowed_states = (self.allowed_states
                            | state_bit(LocomotionState::Run3))
                            & !state_bit(LocomotionState::Run2);
                    }
                } else {
                    // Run3 (`0x001189d0`): drains stamina only when Ampoule is NOT active
                    if !input.ampoule_active {
                        self.stamina_frames += dt * 30.0;
                    }
                    self.ramp_speed(target, 3.0, 3.0, dt);
                    // Downgrade Run3 -> Run2 on stamina exhaustion OR analog speed drop <= 4.0 (`0x001189d0` lines 1591, 1615)
                    if self.stamina_frames >= stamina_limit
                        || (!input.is_digital_input && self.speed_mps <= 4.0)
                    {
                        self.allowed_states = (self.allowed_states
                            | state_bit(LocomotionState::Run2))
                            & !state_bit(LocomotionState::Run3);
                    }
                }

                effective_speed_mps = self.speed_mps;
                if !input.is_aiming {
                    yaw_rate = if input.is_digital_input {
                        1.7
                    } else {
                        1.7 * tx
                    };
                }
                target_offset = 0.0;
                offset_ease_rate = DIRECTION_OFFSET_EASE_RUN_RAD_PER_SEC;
            }
            LocomotionState::StrafeLeftRun | LocomotionState::StrafeRightRun => {
                // `lower_lsrun_3d` (0x00118f20) / `lower_rsrun_3d` (0x001192f0)
                if !input.ampoule_active {
                    self.stamina_frames += dt * 30.0;
                }
                self.ramp_speed(3.8, 6.0, 6.0, dt);
                effective_speed_mps = self.speed_mps;
                if !input.is_aiming {
                    yaw_rate = if input.is_digital_input {
                        1.7
                    } else {
                        1.7 * tx
                    };
                }
                target_offset = if self.state == LocomotionState::StrafeLeftRun {
                    -PI * 0.5
                } else {
                    PI * 0.5
                };
                offset_ease_rate = DIRECTION_OFFSET_EASE_WALK_RAD_PER_SEC;
            }
            LocomotionState::QuickTurnLeft | LocomotionState::QuickTurnRight => {
                // `lower_lturn180_3d` (0x001196c0) / `lower_rturn180_3d` (0x00119860)
                self.speed_mps = 0.0;
                effective_speed_mps = 0.0;
                let qt_speed = if input.is_great_knife {
                    GREAT_KNIFE_QUICK_TURN_SPEED_RAD_PER_SEC
                } else {
                    QUICK_TURN_SPEED_RAD_PER_SEC
                };
                let step = qt_speed * dt;
                self.quick_turn_progress += step;
                if self.quick_turn_progress >= PI {
                    *current_yaw = self.quick_turn_target_yaw;
                    self.enter_state(LocomotionState::Stand);
                } else if self.state == LocomotionState::QuickTurnLeft {
                    *current_yaw -= step;
                } else {
                    *current_yaw += step;
                }
                target_offset = 0.0;
                snap_offset_directly = true;
            }
            LocomotionState::WallF => {
                // `lower_wall_f_3d` (`0x0011a170`):
                // Frame 4 or > 9 -> speed = 0.0; Frames 5..=9 -> recoil opposite entry at +5.0 m/s^2 up to 1.2 m/s.
                let wf_frame = self.wall_f_frame as u32;
                if wf_frame == 4 || wf_frame > 9 {
                    self.speed_mps = 0.0;
                    target_offset = self.direction_offset;
                } else if (5..=9).contains(&wf_frame) {
                    target_offset = match self.prev_state {
                        LocomotionState::StrafeLeftRun => PI * 0.5,
                        LocomotionState::StrafeRightRun => -PI * 0.5,
                        _ => -PI,
                    };
                    snap_offset_directly = true;
                    self.speed_mps = (self.speed_mps + 5.0 * dt).min(1.2);
                } else {
                    target_offset = self.direction_offset;
                }
                effective_speed_mps = self.speed_mps;
                self.wall_f_frame += dt * 30.0;
            }
            LocomotionState::Fall => {
                // `lower_fall` (`0x0021f5c0`, `m3_play_common.c:1508-1528`):
                // Frames `< 0x15` (21): ramp toward `0.9 m/s` (`+2.5 m/s^2`), or `PlayerSpeedDownToStand` if `> 0.9`.
                // Frames `>= 0x15` (21): `PlayerSpeedDownToStand` (`20.0` post-run / `10.0` normal decel).
                let decel = if self.prev_state.is_running() {
                    20.0
                } else {
                    10.0
                };
                if (self.fall_frame as u32) < 21 {
                    if self.speed_mps > 0.9 {
                        self.speed_mps = (self.speed_mps - decel * dt).max(0.0);
                    } else {
                        self.speed_mps = (self.speed_mps + 2.5 * dt).min(0.9);
                    }
                } else {
                    self.speed_mps = (self.speed_mps - decel * dt).max(0.0);
                }
                effective_speed_mps = self.speed_mps;
                target_offset = self.direction_offset;
                self.fall_frame += dt * 30.0;
            }
        }

        StateDynamicsOutput {
            target_offset,
            offset_ease_rate,
            snap_offset_directly,
            yaw_rate,
            effective_speed_mps,
        }
    }
}
