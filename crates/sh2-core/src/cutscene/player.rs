//! Runtime cutscene state machine (`DramaDemoPlayer`, `CutsceneCameraSnapshot`, `CutsceneActorRuntime`) (`SH2-63`).

use glam::{Mat4, Vec3};
use serde::{Deserialize, Serialize};
use sh2_assets::formats::demo::{
    DemoDdsEvaluator, DemoDdsFile, DemoKeyframeState, DemoLightKind, DemoLightState,
    DEMO_MAX_CHARS, DEMO_MAX_LIGHTS,
};

use super::anim_info::{
    derive_demo_actor_anm_and_cls_paths, find_drama_demo_anim_info, DramaDemoAnimInfo,
};
use super::catalog::DramaDemoPlayInfo;
use super::CUTSCENE_NATIVE_FPS;

/// Runtime state of a single actor inside an active `.dds` cutscene (`SubCharacter` in `Event/demoview.c`).
#[derive(Debug, Clone)]
pub struct CutsceneActorRuntime {
    /// Actor slot index (`0..n_chars`).
    pub slot_index: usize,
    /// Raw actor name from `.dds` header (e.g. `"hhh_jms_pos"`).
    pub actor_name: String,
    /// Resolved `DramaDemo_AnimInfo` entry if present in the 84-entry table.
    pub anim_info: Option<&'static DramaDemoAnimInfo>,
    /// Derived `.anm` relative asset path for this demo folder.
    pub anm_path: String,
    /// Derived `.cls` facial morph animation relative asset path if applicable.
    pub cls_path: Option<String>,
    /// Whether this actor is currently visible (`status & 1`).
    pub visible: bool,
    /// Current world-space position (`pos + offset`).
    pub position: Vec3,
    /// Current Y rotation in radians (`PI` during `DdsPlay`).
    pub yaw_radians: f32,
    /// Current animation ID (`aid`, initialized to `start_aid` and incremented on each `Key 0x11` after the first).
    pub current_aid: u16,
    /// Number of `Key 0x11` (`DramaDemoAnimationStart`) triggers processed for this actor.
    pub subclip_index: usize,
    /// Continuous 30 FPS animation frame within the current sub-clip (`scene_frame - subclip_start_frame`).
    pub local_anm_frame: f32,
    /// Starting 30 FPS frame offset inside the actor's `.anm` and `.cls` files (accounts for multi-part
    /// cutscenes such as `mar_deai_e`, `papa_agl`, `piano_lau_e`, and Laura's 226-frame `piano_lau_a` prefix).
    pub anm_frame_offset: u16,
}

impl CutsceneActorRuntime {
    /// Returns the continuous global 30 FPS frame index inside the actor's `.anm` and `.cls` files
    /// (`scene_frame + anm_frame_offset`).
    #[inline]
    pub fn global_anm_frame(&self, scene_frame: f32) -> f32 {
        scene_frame.max(0.0) + (self.anm_frame_offset as f32)
    }

    /// Computes the 4x4 row-major world transformation matrix for this cutscene actor (`rot_y = PI`, `pos = position`).
    pub fn world_matrix_row_major(&self) -> [[f32; 4]; 4] {
        let (s, c) = self.yaw_radians.sin_cos();
        [
            [c, 0.0, s, self.position.x],
            [0.0, 1.0, 0.0, self.position.y],
            [-s, 0.0, c, self.position.z],
            [0.0, 0.0, 0.0, 1.0],
        ]
    }
}

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

/// Lifecycle phase of [`DramaDemoPlayer`] (`demo_step` in `DramaDemoMain` at `0x1b2b30`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DramaDemoStep {
    /// Cutscene is actively playing (`demo_step == 2`).
    Playing,
    /// Cutscene was skipped via `Start`/`Esc` and is fading out (`demo_step == 3`).
    SkippingFadeOut,
    /// Cutscene has completed (`demo_step >= 5`), ready to hand control back to gameplay.
    Finished,
}

/// Stateful in-engine real-time 3D cutscene player (`DramaDemoMain` / `DdsPlay` / `DramaDemoSkipLast`).
#[derive(Debug, Clone)]
pub struct DramaDemoPlayer {
    /// Static metadata descriptor (`DramaDemoPlayInfo`).
    pub info: DramaDemoPlayInfo,
    /// Underlying `.dds` binary keyframe evaluator.
    pub evaluator: DemoDdsEvaluator,
    /// Current lifecycle phase (`Playing`, `SkippingFadeOut`, `Finished`).
    pub step: DramaDemoStep,
    /// Continuous 30 FPS scene frame cursor (`Demo_Frame`).
    pub scene_frame: f32,
    /// Independent 30 FPS subtitle clock (`msg_frame`).
    pub msg_frame: f32,
    /// Whether the voice stream (`info.stream_no`) has been triggered at `scene_frame >= stream_start`.
    pub voice_started: bool,
    /// Whether the voice stream is waiting to be started by the audio engine on this tick.
    pub pending_voice_trigger: bool,
    /// Whether the voice stream should be stopped immediately (`SeCall(0x3F4)` on skip/finish).
    pub pending_voice_stop: bool,
    /// Active `.mes` message ID (`info.msg_start + window.msg_offset`) if a subtitle window is currently active.
    pub active_subtitle_msg_id: Option<u16>,
    /// Current screen fade alpha (`0.0` = clear, `1.0` = black).
    pub fade_alpha: f32,
    /// Whether Widescreen FOV reduction (`ReduceCutsceneFOV`) is enabled.
    pub reduce_widescreen_fov: bool,
    /// Evaluated camera snapshot for the current frame.
    pub camera: CutsceneCameraSnapshot,
    /// Evaluated keyframe state for the current frame.
    pub current_state: DemoKeyframeState,
    /// Per-actor runtime states (`0..header.n_chars`).
    pub actors: Vec<CutsceneActorRuntime>,
}

impl DramaDemoPlayer {
    /// Initializes a new real-time 3D cutscene session at Frame 0 (`DramaDemoInit` at `0x1b2080`).
    pub fn new(info: DramaDemoPlayInfo, script: DemoDdsFile, reduce_widescreen_fov: bool) -> Self {
        let mut actors = Vec::with_capacity(script.header.character_names.len());
        for (idx, name) in script.header.character_names.iter().enumerate() {
            let anim_info = find_drama_demo_anim_info(name);
            let (anm_path, cls_path) = derive_demo_actor_anm_and_cls_paths(info.dds_path, name);
            let start_aid = anim_info.map(|a| a.start_aid).unwrap_or(0);
            // In `data/demo/piano_lau/`, `lau.anm` and `lau.cls` prepend the 226-frame `piano_lau_a`
            // sub-clip before the 3,221-frame `piano_lau_b` + `piano_lau_e` tracks (`[226, 243, 720, 716, 781, 761]`),
            // whereas `hhh_jms.anm`/`.cls` starts directly at `piano_lau_b` (`[243, 720, 716, 781, 761]`).
            let extra_actor_offset = if (info.dds_path.ends_with("piano_lau_b.dds")
                || info.dds_path.ends_with("piano_lau_e.dds")
                || info.dds_path.ends_with("piano_lau.dds"))
                && name
                    .trim_matches('\0')
                    .trim()
                    .eq_ignore_ascii_case("lau_pos")
            {
                226u16
            } else {
                0u16
            };
            let anm_frame_offset = info.anm_frame_offset.saturating_add(extra_actor_offset);
            actors.push(CutsceneActorRuntime {
                slot_index: idx,
                actor_name: name.clone(),
                anim_info,
                anm_path,
                cls_path,
                visible: false,
                position: Vec3::ZERO,
                yaw_radians: std::f32::consts::PI,
                current_aid: start_aid,
                subclip_index: 0,
                local_anm_frame: 0.0,
                anm_frame_offset,
            });
        }

        let mut evaluator = DemoDdsEvaluator::new(script);
        let (init_state, _) = evaluator.sample_at(0.0, info.offset_x, info.offset_z);
        let fov_y = init_state.vertical_fov_radians(info.fov_scale(), reduce_widescreen_fov);
        let camera = CutsceneCameraSnapshot {
            eye: Vec3::from_array(init_state.camera_pos),
            interest: Vec3::from_array(init_state.camera_interest),
            roll_radians: init_state.camera_roll,
            plane: init_state.camera_plane,
            fov_y_radians: fov_y,
            is_hard_cut: true,
        };

        let mut player = Self {
            info,
            evaluator,
            step: DramaDemoStep::Playing,
            scene_frame: 0.0,
            msg_frame: 0.0,
            voice_started: false,
            pending_voice_trigger: false,
            pending_voice_stop: false,
            active_subtitle_msg_id: None,
            fade_alpha: 0.0,
            reduce_widescreen_fov,
            camera,
            current_state: init_state,
            actors,
        };
        player.sync_actors_from_state();
        player
    }

    /// Returns `true` if gameplay input and the pause menu should be locked while this cutscene is active.
    #[inline]
    pub fn locks_gameplay_input(&self) -> bool {
        self.step != DramaDemoStep::Finished
    }

    /// Total duration of the `.dds` cutscene in 30 FPS frames.
    #[inline]
    pub fn total_frames(&self) -> u16 {
        self.evaluator.script.header.total_frames.max(1) as u16
    }

    /// Triggers an immediate cutscene skip (`Start` / `Esc` button), matching `DramaDemoSkipLast`
    /// (`0x1b1be0` PS2 / `0x4a09f0` PC): fast-forwards all `.dds` records to `0xFFFF`, stops voice
    /// streaming (`SeCall(0x3F4)`), clears subtitles, and positions actors at their final coordinates (`CharaSetPos`).
    pub fn skip(&mut self) {
        if self.step == DramaDemoStep::Finished {
            return;
        }
        self.evaluator.skip_to_last();
        let total_f = self.total_frames() as f32;
        self.scene_frame = total_f;
        self.msg_frame = total_f;
        self.active_subtitle_msg_id = None;
        if self.voice_started {
            self.pending_voice_stop = true;
        }
        self.pending_voice_trigger = false;

        let mut final_state = self.evaluator.next.clone();
        final_state.camera_pos[0] += self.info.offset_x;
        final_state.camera_pos[2] += self.info.offset_z;
        final_state.camera_interest[0] += self.info.offset_x;
        final_state.camera_interest[2] += self.info.offset_z;
        for c in 0..DEMO_MAX_CHARS {
            final_state.char_positions[c][0] += self.info.offset_x;
            final_state.char_positions[c][2] += self.info.offset_z;
        }
        self.current_state = final_state;
        self.sync_camera_and_actors();
        self.fade_alpha = 1.0;
        self.step = DramaDemoStep::Finished;
    }

    /// Advances the cutscene by `dt_sec` seconds, evaluating `.dds` keyframes, `Key 0x11` animation
    /// triggers, voice stream pre-roll (`stream_start`), and `adr_msg_time` subtitle windows.
    pub fn tick(&mut self, dt_sec: f32, subtitles_enabled: bool) {
        self.tick_with_voice_sync(dt_sec, subtitles_enabled, None);
    }

    /// Advances the cutscene by `dt_sec` seconds while locking `scene_frame` and `msg_frame` to the
    /// hardware audio clock `voice_pos_secs` (`DramaDemoMain` in `Event/demoview.c`, `SH2-63`).
    pub fn tick_with_voice_sync(
        &mut self,
        dt_sec: f32,
        subtitles_enabled: bool,
        voice_pos_secs: Option<f32>,
    ) {
        if self.step == DramaDemoStep::Finished {
            return;
        }

        let delta_frames = (dt_sec.max(0.0)) * CUTSCENE_NATIVE_FPS;
        let total_f = self.total_frames() as f32;

        // Check voice stream pre-roll trigger (`scene_frame - 1 >= stream_start`)
        let stream_start_f = self.info.stream_start.max(0) as f32;
        if !self.voice_started
            && self.info.stream_no > 0
            && (self.scene_frame + delta_frames) >= stream_start_f
        {
            self.voice_started = true;
            self.pending_voice_trigger = true;
        }

        // Advance scene_frame and msg_frame, resynchronizing to the active voice stream hardware clock
        // if drift exceeds 2 frames (66 ms), matching `DramaDemoMain`.
        if self.voice_started {
            if let Some(pos_sec) = voice_pos_secs {
                let want = stream_start_f + 1.0 + pos_sec * CUTSCENE_NATIVE_FPS;
                if (self.msg_frame + delta_frames - want).abs() > 2.0 {
                    self.msg_frame = want;
                    self.scene_frame = want.min(total_f);
                } else {
                    self.msg_frame += delta_frames;
                    self.scene_frame = (self.scene_frame + delta_frames).min(total_f);
                }
            } else {
                self.msg_frame += delta_frames;
                self.scene_frame += delta_frames;
            }
        } else {
            self.msg_frame += delta_frames;
            self.scene_frame += delta_frames;
        }

        // Evaluate active subtitle window (`DramaDemoMessage` at `0x1b1f80`)
        let show_subs = subtitles_enabled || self.info.forces_subtitles();
        if show_subs {
            self.active_subtitle_msg_id = self
                .info
                .subtitle_windows
                .iter()
                .find(|w| w.is_active_at(self.msg_frame))
                .map(|w| self.info.msg_start + w.msg_offset);
        } else {
            self.active_subtitle_msg_id = None;
        }

        let last_subtitle_end = self
            .info
            .subtitle_windows
            .iter()
            .map(|w| w.end_frame as f32)
            .fold(0.0f32, f32::max);

        if self.scene_frame >= total_f {
            self.scene_frame = total_f;
            // In cutscenes where the voiceover outlasts the `.dds` camera track (e.g. Mary's letter
            // narration at the end of `tenbou`, where `.dds` ends at frame 1927 while `voice.afs[118]`
            // and subtitles continue up to frame 3954), hold the final `.dds` frame until the voice
            // stream finishes!
            let voice_still_active = voice_pos_secs.is_some()
                || (self.voice_started && self.msg_frame < last_subtitle_end);
            if !voice_still_active {
                self.skip();
                return;
            }
        }

        let (mut sampled, _triggers) =
            self.evaluator
                .sample_at(self.scene_frame, self.info.offset_x, self.info.offset_z);

        // Apply `demo_no == 6` (`first_battle`) Lying Figure reveal lighting override (`0x4a1670`)
        if self.info.demo_no == 6 && self.scene_frame >= 1599.0 && self.scene_frame < 2365.0 {
            let mut lr = sampled.lights[0];
            lr.visible = true;
            lr.kind = DemoLightKind::Point;
            lr.position = [
                75.28 + self.info.offset_x,
                -882.43,
                39660.15 + self.info.offset_z,
            ];
            lr.color = [1.0, 1.0, 1.0];
            lr.decay = [500.0, 2000.0];
            sampled.lights[0] = lr;
        }

        self.current_state = sampled;
        self.sync_camera_and_actors();
    }

    /// Scrubs directly to `target_frame` (for interactive inspection in the debug viewer).
    pub fn seek_to_frame(&mut self, target_frame: f32, subtitles_enabled: bool) {
        let clamped = target_frame.clamp(0.0, self.total_frames() as f32);
        self.scene_frame = clamped;
        self.msg_frame = clamped;
        self.step = DramaDemoStep::Playing;
        self.fade_alpha = 0.0;

        let show_subs = subtitles_enabled || self.info.forces_subtitles();
        self.active_subtitle_msg_id = if show_subs {
            self.info
                .subtitle_windows
                .iter()
                .find(|w| w.is_active_at(self.msg_frame))
                .map(|w| self.info.msg_start + w.msg_offset)
        } else {
            None
        };

        let (sampled, _) =
            self.evaluator
                .sample_at(self.scene_frame, self.info.offset_x, self.info.offset_z);
        self.current_state = sampled;
        self.sync_camera_and_actors();
    }

    /// Returns the primary active dynamic light from the `.dds` light slots (`0..DEMO_MAX_LIGHTS`),
    /// suitable for driving the stage spotlight / point light shader uniform.
    pub fn primary_active_light(&self) -> Option<&DemoLightState> {
        let total_lights = self
            .evaluator
            .script
            .header
            .total_lights()
            .min(DEMO_MAX_LIGHTS);
        self.current_state
            .lights
            .iter()
            .take(total_lights)
            .find(|l| l.visible)
    }

    /// Returns the final player (`hhh_jms_pos` / `hhl_jms_pos` / `hll_jms_pos` / `lll_jms_pos` / `hhh_mar_pos` / `lll_mar_pos`)
    /// world position and yaw (`CharaSetPos` hand-off at cutscene completion).
    pub fn player_handoff_transform(&self) -> Option<(Vec3, f32)> {
        self.actors
            .iter()
            .find(|a| {
                a.anim_info
                    .map(|info| matches!(info.kind_id, 0x0100..=0x0103 | 0x0105..=0x0106))
                    .unwrap_or(false)
            })
            .map(|a| (a.position, a.yaw_radians))
    }

    fn sync_camera_and_actors(&mut self) {
        let fov_y = self
            .current_state
            .vertical_fov_radians(self.info.fov_scale(), self.reduce_widescreen_fov);
        self.camera = CutsceneCameraSnapshot {
            eye: Vec3::from_array(self.current_state.camera_pos),
            interest: Vec3::from_array(self.current_state.camera_interest),
            roll_radians: self.current_state.camera_roll,
            plane: self.current_state.camera_plane,
            fov_y_radians: fov_y,
            is_hard_cut: self.evaluator.is_cut,
        };
        self.sync_actors_from_state();
    }

    fn sync_actors_from_state(&mut self) {
        let trig_count = self.evaluator.anim_trigger_count;
        let subclip_idx = trig_count.saturating_sub(1);
        let local_frame =
            (self.scene_frame - (self.evaluator.last_anim_trigger_frame as f32)).max(0.0);

        for actor in &mut self.actors {
            let idx = actor.slot_index;
            if idx < DEMO_MAX_CHARS {
                actor.visible = self.current_state.char_visible[idx];
                actor.position = Vec3::from_array(self.current_state.char_positions[idx]);
                actor.yaw_radians = std::f32::consts::PI;
                actor.subclip_index = subclip_idx;
                actor.local_anm_frame = local_frame;
                if let Some(info) = actor.anim_info {
                    actor.current_aid = (info.start_aid + (subclip_idx as u16)).min(info.end_aid);
                }
            }
        }
    }
}
