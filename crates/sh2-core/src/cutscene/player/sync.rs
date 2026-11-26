//! Cutscene camera snapshot (`VbSetCameraMatrix`) and per-frame camera/actor/atmosphere synchronization.

use glam::{Mat4, Vec3};
use sh2_assets::formats::demo::DEMO_MAX_CHARS;

use super::super::atmosphere::{
    resolve_cutscene_room_id, AtmosphereEvalInput, AtmosphereGradingEvaluation,
    ScreenTransitionState,
};
use super::super::culling::should_cull_parked_offscreen_actor;
use super::super::piston_kinematics::ClosetDoorReplacementState;
use super::super::special_fx::{SpecialFxEvalInput, SpecialFxEvaluation};
use super::super::stage_fixes::{
    evaluate_hei_lau_widescreen_entry, evaluate_meeting_maria_fog_counters, ClosetCutsceneFixState,
};
use super::DramaDemoPlayer;

/// Evaluated camera state for a single cutscene frame (`VbSetCameraMatrix` / `vwViewPointInfo` in `Event/demoview.c`).
#[derive(Debug, Clone, Copy)]
pub struct CutsceneCameraSnapshot {
    /// World-space camera eye position.
    pub eye: Vec3,
    /// World-space camera look-at interest target.
    pub interest: Vec3,
    /// Camera roll angle in radians around the forward view axis.
    pub roll_radians: f32,
    /// Raw `.dds` projection plane distance (`plane`).
    pub plane: f32,
    /// Vertical field-of-view in radians (`2 * atan(224.0 / scr_z)`).
    pub fov_y_radians: f32,
    /// Whether this frame is a hard camera cut (`Key 0x10`).
    pub is_hard_cut: bool,
}

impl CutsceneCameraSnapshot {
    /// Builds the right-handed view matrix transforming PS2 world space (Y-down) into view space,
    /// incorporating `roll_radians` around the view axis (`VbSetCameraMatrix`).
    pub fn view_matrix(&self) -> Mat4 {
        let forward = (self.interest - self.eye).normalize_or_zero();
        let fwd = if forward.length_squared() > 1e-6 {
            forward
        } else {
            Vec3::Z
        };
        // In PS2 world space Y is negative-up (`(0, -1, 0)` is world up).
        let base_up = Vec3::new(0.0, -1.0, 0.0);
        let rot_roll = Mat4::from_axis_angle(fwd, self.roll_radians);
        let rolled_up = rot_roll.transform_vector3(base_up).normalize_or_zero();
        let up = if rolled_up.length_squared() > 1e-6 {
            rolled_up
        } else {
            base_up
        };
        Mat4::look_at_rh(self.eye, self.eye + fwd * 1000.0, up)
    }
}

impl DramaDemoPlayer {
    pub(super) fn sync_camera_and_actors(&mut self) {
        let fov_scale = self.info.fov_scale();
        let fov_y = self.current_state.vertical_fov_radians(
            fov_scale,
            self.viewport_aspect,
            self.widescreen_mode,
        );
        self.camera = CutsceneCameraSnapshot {
            eye: Vec3::from_array(self.current_state.camera_pos),
            interest: Vec3::from_array(self.current_state.camera_interest),
            roll_radians: self.current_state.camera_roll,
            plane: self.current_state.camera_plane,
            fov_y_radians: fov_y,
            // Preserve frame-0 hard-cut (`DramaDemoInit` `SLUS_202.28` `0x001b1130`) so
            // `ExternalCameraHandoff::sync_cutscene` snaps immediately into `CutsceneLocked`
            // instead of blending for ~0.5-1s from the gameplay camera (`SH2-150`).
            is_hard_cut: self.evaluator.is_cut || self.scene_frame <= 0.0,
        };

        // Evaluate scene-specific widescreen/ultrawide fixes (`SH2-77`)
        self.closet_fix = ClosetCutsceneFixState::evaluate(
            self.info.demo_no,
            self.scene_frame,
            self.current_state.camera_pos[0],
            self.viewport_aspect,
        );
        self.piston_fix
            .update(self.info.demo_no, self.current_state.camera_pos[0]);
        if self.piston_fix.active && !self.piston_fix.doorway_piston_culled {
            for rec in &self.evaluator.script.records[..self.evaluator.next_record_idx] {
                for &op in &rec.camera_ops {
                    if let sh2_assets::formats::demo::DemoCameraOp::Position {
                        values,
                        is_raw: true,
                    } = op
                    {
                        self.piston_fix
                            .update(self.info.demo_no, values[0] + self.info.offset_x);
                    }
                }
            }
        }
        self.maria_fog_counters =
            evaluate_meeting_maria_fog_counters(self.info.demo_no, self.scene_frame);

        if self.piston_fix.doorway_piston_culled
            && !self.piston_array.fix_state.doorway_piston_culled
        {
            self.piston_array.fix_state = self.piston_fix;
            self.piston_array
                .step(0.0, self.info.demo_no, self.current_state.camera_pos[0]);
        }

        let resolved_room_id = resolve_cutscene_room_id(self.info.demo_no, self.info.stage_name);
        self.atmosphere = AtmosphereGradingEvaluation::evaluate(AtmosphereEvalInput {
            room_id: resolved_room_id,
            demo_no: self.info.demo_no,
            scene_frame: self.scene_frame,
            camera_pos_x: self.current_state.camera_pos[0],
            james_pos_y: self.current_state.char_positions[0][1],
            viewport_aspect: self.viewport_aspect,
            event_index: 1,
            transition_state: ScreenTransitionState::Normal,
        });
        self.special_fx = SpecialFxEvaluation::evaluate(SpecialFxEvalInput {
            room_id: resolved_room_id,
            demo_no: self.info.demo_no,
            scene_frame: self.scene_frame,
            camera_pos_x: self.current_state.camera_pos[0],
            viewport_height: self.viewport_height,
            viewport_aspect: self.viewport_aspect,
            enemies_remaining: 1,
            flesh_lips_death_substate: None,
        });
        self.closet_door = ClosetDoorReplacementState::evaluate(
            resolved_room_id,
            self.info.demo_no,
            self.scene_frame,
            true,
        );

        self.sync_actors_from_state();
    }

    fn sync_actors_from_state(&mut self) {
        let trig_count = self.evaluator.anim_trigger_count;
        let subclip_idx = trig_count.saturating_sub(1);
        let local_frame =
            (self.scene_frame - (self.evaluator.last_anim_trigger_frame as f32)).max(0.0);
        let (fov_y_4x3, fov_x_4x3) = self
            .current_state
            .authored_4x3_fov_radians(self.info.fov_scale());
        let offset_vec = Vec3::new(self.info.offset_x, 0.0, self.info.offset_z);

        for actor in &mut self.actors {
            let idx = actor.slot_index;
            if idx < DEMO_MAX_CHARS {
                let raw_last = Vec3::from_array(self.evaluator.last.char_positions[idx]);
                let raw_next = Vec3::from_array(self.evaluator.next.char_positions[idx]);
                let clean_name = actor.actor_name.trim_matches('\0').trim();
                let is_handheld_item = clean_name.starts_with("i_")
                    || clean_name.starts_with("I_")
                    || clean_name.eq_ignore_ascii_case("bdcard_pos");
                let raw_at_origin =
                    raw_last.length_squared() < 1e-4 && raw_next.length_squared() < 1e-4;
                let despawned_to_origin = raw_at_origin
                    && (is_handheld_item
                        || clean_name.eq_ignore_ascii_case("dmr_pos")
                        || clean_name.eq_ignore_ascii_case("b_cha_pos")
                        || clean_name.eq_ignore_ascii_case("b_hul_pos"));
                let script_vis = self.current_state.char_visible[idx] && !despawned_to_origin;
                let pos = Vec3::from_array(self.current_state.char_positions[idx]);
                let last_pos = raw_last + offset_vec;
                let next_pos = raw_next + offset_vec;

                let hei_lau_entry = evaluate_hei_lau_widescreen_entry(
                    self.info.demo_no,
                    &actor.actor_name,
                    self.scene_frame,
                    self.widescreen_mode,
                    self.viewport_aspect,
                    pos,
                );

                let hidden_by_scene_fix = self.closet_fix.should_hide_actor(&actor.actor_name)
                    || hei_lau_entry.hidden_before_entry;
                let culled_by_frustum =
                    if hei_lau_entry.active && !hei_lau_entry.hidden_before_entry {
                        // During the smooth widescreen pre-roll entry walk (`172.0..240.0`) and authored
                        // walk (`240.0..292.0`) in `hei_lau`, keep James visible so he walks seamlessly in.
                        false
                    } else {
                        should_cull_parked_offscreen_actor(
                            self.widescreen_mode,
                            self.viewport_aspect,
                            script_vis,
                            self.camera.eye,
                            self.camera.interest,
                            self.camera.roll_radians,
                            fov_y_4x3,
                            fov_x_4x3,
                            pos,
                            last_pos,
                            next_pos,
                            actor.bounding_sphere,
                        )
                    };

                actor.script_visible = script_vis;
                actor.culled_offscreen_4x3 =
                    script_vis && (hidden_by_scene_fix || culled_by_frustum);
                actor.visible = script_vis && !actor.culled_offscreen_4x3;
                actor.position = hei_lau_entry.position_override.unwrap_or(pos);
                actor.yaw_radians = std::f32::consts::PI;
                actor.subclip_index = subclip_idx;
                actor.anm_frame_override = hei_lau_entry.anm_frame_override;
                actor.local_anm_frame = hei_lau_entry.anm_frame_override.unwrap_or(local_frame);
                if let Some(info) = actor.anim_info {
                    actor.current_aid = (info.start_aid + (subclip_idx as u16)).min(info.end_aid);
                }
            }
        }
    }
}
