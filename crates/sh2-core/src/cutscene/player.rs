//! Runtime cutscene state machine (`DramaDemoPlayer`, `CutsceneCameraSnapshot`, `CutsceneActorRuntime`) (`SH2-63`).

use glam::{Mat4, Vec3};
use serde::{Deserialize, Serialize};
use sh2_assets::formats::demo::{
    CutsceneBorderMode, CutsceneWidescreenMode, DemoDdsEvaluator, DemoDdsFile, DemoKeyframeState,
    DemoLightKind, DemoLightState, DEMO_MAX_CHARS, DEMO_MAX_LIGHTS, STANDARD_16X9_ASPECT,
};

use super::anim_info::{
    derive_demo_actor_anm_and_cls_paths, find_drama_demo_anim_info, DramaDemoAnimInfo,
};
use super::atmosphere::{
    resolve_cutscene_room_id, AtmosphereEvalInput, AtmosphereGradingEvaluation,
    ScreenTransitionState,
};
use super::catalog::DramaDemoPlayInfo;
use super::culling::{should_cull_parked_offscreen_actor, ActorBoundingSphere};
use super::piston_kinematics::{ClosetDoorReplacementState, PistonArrayRuntime};
use super::special_fx::{SpecialFxEvalInput, SpecialFxEvaluation, SPECIAL_FX_REFERENCE_HEIGHT};
use super::stage_fixes::{
    evaluate_hei_lau_widescreen_entry, evaluate_meeting_maria_fog_counters, ClosetCutsceneFixState,
    PistonRoomFixState,
};
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
    /// Raw visibility flag authored in the `.dds` script (`status & 1`).
    pub script_visible: bool,
    /// Whether this actor is currently culled because it is parked outside the authored `4:3`
    /// camera frustum on a widescreen/ultrawide viewport or despawned by a scene-specific fix (`SH2-77`).
    pub culled_offscreen_4x3: bool,
    /// Effective visibility (`script_visible && !culled_offscreen_4x3`).
    pub visible: bool,
    /// World-space bounding sphere used for `4:3` off-screen frustum culling (`SH2-77`).
    pub bounding_sphere: ActorBoundingSphere,
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
    /// Optional scene-specific animation frame override (e.g. James's smooth widescreen pre-roll walk cycle
    /// in `hei_lau` during frames `172.0..240.0`, `SH2-77`).
    pub anm_frame_override: Option<f32>,
    /// Starting 30 FPS frame offset inside the actor's `.anm` file (accounts for multi-part
    /// cutscenes such as `mar_deai_e`, `papa_agl`, `piano_lau_e`, Laura's 226-frame `piano_lau_a` prefix,
    /// and NPC gameplay base clips prepended to `agl.anm`, `hhh_edi.anm`, `hhh_mar.anm`, `mry.anm`, `bos.anm`).
    pub anm_frame_offset: u16,
    /// Starting 30 FPS frame offset inside the actor's `.cls` facial morph cluster file.
    /// Unlike `.anm` files (which prepend NPC gameplay clips), `.cls` files only contain facial
    /// cutscene sub-clips (`clani_adr_list`).
    pub cls_frame_offset: u16,
}

impl CutsceneActorRuntime {
    /// Returns the continuous global 30 FPS frame index inside the actor's `.anm` file
    /// (`base_frame + anm_frame_offset`).
    #[inline]
    pub fn global_anm_frame(&self, scene_frame: f32) -> f32 {
        let base_frame = self.anm_frame_override.unwrap_or(scene_frame.max(0.0));
        base_frame + (self.anm_frame_offset as f32)
    }

    /// Returns the continuous global 30 FPS frame index inside the actor's `.cls` facial morph file
    /// (`base_frame + cls_frame_offset`).
    #[inline]
    pub fn global_cls_frame(&self, scene_frame: f32) -> f32 {
        let base_frame = self.anm_frame_override.unwrap_or(scene_frame.max(0.0));
        base_frame + (self.cls_frame_offset as f32)
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
    /// Whether Widescreen FOV reduction (`ReduceCutsceneFOV`) is enabled (`widescreen_mode == Sh2eeZoom16x9`).
    pub reduce_widescreen_fov: bool,
    /// Current viewport aspect ratio (`width / height`, e.g. `16.0/9.0`, `64.0/27.0`, `32.0/9.0`) (`SH2-77`).
    pub viewport_aspect: f32,
    /// Current viewport height in pixels (used to compute `display_ratio = height / 1080.0` in `SH2-105`).
    pub viewport_height: f32,
    /// Active widescreen framing and off-screen actor culling mode (`SH2-77`).
    pub widescreen_mode: CutsceneWidescreenMode,
    /// Policy for top/bottom cutscene letterbox bars (`DisableCutsceneBorders` in `SH2-77`).
    pub border_mode: CutsceneBorderMode,
    /// Scene-specific widescreen fix state for Wood Side Room 307 (`sankaku1` / `wp_closet`, `demo_no == 14`).
    pub closet_fix: ClosetCutsceneFixState,
    /// Scene-specific widescreen fix state for Labyrinth Piston Room (`papa_agl` / `ug_angela`, `demo_no == 68`).
    pub piston_fix: PistonRoomFixState,
    /// Scene-specific widescreen fog counter triggers `(counter_one, counter_two)` for Rosewater Park (`deai`, `demo_no == 22`).
    pub maria_fog_counters: (bool, bool),
    /// Resolved PS2 room/cutscene atmospheric grading (`TimeOfDay = 15/11`, Room 312 sunset, Leave/Maria tree lighting) (`SH2-105`).
    pub atmosphere: AtmosphereGradingEvaluation,
    /// Resolved PS2 screen-space DoF (`Kari_hisyakai`) and GS motion blur (`enSetBlur`) state (`SH2-105`).
    pub special_fx: SpecialFxEvaluation,
    /// Stateful 24-piston array kinematics for the Abstract Daddy boss room (`SH2-105`).
    pub piston_array: PistonArrayRuntime,
    /// Wood Side Room 307 `b_doo` closet door replacement transform and bonus audio trigger (`SH2-105`).
    pub closet_door: ClosetDoorReplacementState,
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
        let mode = if reduce_widescreen_fov {
            CutsceneWidescreenMode::Sh2eeZoom16x9
        } else {
            CutsceneWidescreenMode::UltrawideHorPlusSmartCull
        };
        Self::new_with_widescreen(info, script, STANDARD_16X9_ASPECT, mode)
    }

    /// Initializes a new real-time 3D cutscene session with explicit `viewport_aspect` and [`CutsceneWidescreenMode`] (`SH2-77`).
    pub fn new_with_widescreen(
        info: DramaDemoPlayInfo,
        script: DemoDdsFile,
        viewport_aspect: f32,
        widescreen_mode: CutsceneWidescreenMode,
    ) -> Self {
        let reduce_widescreen_fov = widescreen_mode == CutsceneWidescreenMode::Sh2eeZoom16x9;
        let mut actors = Vec::with_capacity(script.header.character_names.len());
        for (idx, name) in script.header.character_names.iter().enumerate() {
            let anim_info = find_drama_demo_anim_info(name);
            let (anm_path, cls_path) = derive_demo_actor_anm_and_cls_paths(info.dds_path, name);
            let start_aid = anim_info.map(|a| a.start_aid).unwrap_or(0);
            let clean_actor = name.trim_matches('\0').trim();
            // 1. Sub-clip prefix shared by BOTH `.anm` and `.cls` (e.g. in `data/demo/piano_lau/`,
            // `lau.anm`, `lau.cls`, and `i_letter.anm` prepend the 226-frame `piano_lau_a` sub-clip
            // before the 3,221-frame `piano_lau_b` + `piano_lau_e` tracks `[226, 243, 720, 716, 781, 761]`).
            let extra_subclip_offset = if (info.dds_path.ends_with("piano_lau_b.dds")
                || info.dds_path.ends_with("piano_lau_e.dds")
                || info.dds_path.ends_with("piano_lau.dds"))
                && (clean_actor.eq_ignore_ascii_case("lau_pos")
                    || clean_actor.eq_ignore_ascii_case("i_letter_pos"))
            {
                226u16
            } else if info.dds_path.contains("/sankaku1/")
                && clean_actor.eq_ignore_ascii_case("mkn_2_pos")
            {
                // In `data/demo/sankaku1/mkn.anm` (5,152 frames = 2,576 * 2), `mkn_1_pos` uses
                // frames [0..2576) and `mkn_2_pos` uses frames [2576..5152).
                2576u16
            } else {
                0u16
            };

            // 2. Gameplay base-clip prefix stored ONLY at the start of NPC/boss `.anm` files
            // (`dangela_anime_adr_list`, `deddie_anime_adr_list`, `dlaura_anime_adr_list`,
            // `dmaria_anime_adr_list`, `dmary_anime_adr_list`, `dboss_anime_adr_list`), which is
            // absent from `.cls` facial morph files (`clani_adr_list`).
            let anm_gameplay_prefix = if clean_actor.eq_ignore_ascii_case("agl_pos") {
                if info.dds_path.contains("/haka_agl/") {
                    156u16
                } else if info.dds_path.contains("/papa_agl/") {
                    69u16
                } else if info.dds_path.contains("/fire_agl/") {
                    165u16
                } else {
                    0u16
                }
            } else if clean_actor.eq_ignore_ascii_case("hhh_edi_pos") {
                if info.dds_path.contains("/gero_edi/") {
                    72u16
                } else if info.dds_path.contains("/bp_edi_2/") {
                    71u16
                } else {
                    0u16
                }
            } else if clean_actor.eq_ignore_ascii_case("b_piz_pos")
                && info.dds_path.contains("/bp_edi_2/")
            {
                71u16
            } else if clean_actor.eq_ignore_ascii_case("hhh_mar_pos") {
                if info.dds_path.contains("/mar_deai/") {
                    15u16
                } else if info.dds_path.contains("/mar_hagureru/") {
                    72u16
                } else if info.dds_path.contains("/mar_ikiteruka/") {
                    168u16
                } else if info.dds_path.contains("/reizouko/") {
                    20u16
                } else {
                    0u16
                }
            } else if clean_actor.eq_ignore_ascii_case("lau_pos")
                || clean_actor.eq_ignore_ascii_case("i_letter_pos")
            {
                if info.dds_path.contains("/hei_lau/") {
                    73u16
                } else if info.dds_path.contains("/lau_damasu/") {
                    91u16
                } else {
                    0u16
                }
            } else if clean_actor.eq_ignore_ascii_case("mry_pos") {
                if info.dds_path.contains("/mry_yarinaoshi_a/")
                    || info.dds_path.contains("/fukatsu_a/")
                    || info.dds_path.contains("/jisatsu_a/")
                {
                    67u16
                } else if info.dds_path.contains("/mar_isho_a/") {
                    68u16
                } else {
                    0u16
                }
            } else if clean_actor.eq_ignore_ascii_case("bos_pos")
                && (info.dds_path.contains("/mry_yarinaoshi_a/")
                    || info.dds_path.contains("/fukatsu_a/")
                    || info.dds_path.contains("/jisatsu_a/")
                    || info.dds_path.contains("/mar_isho_a/"))
            {
                627u16
            } else {
                0u16
            };

            let cls_frame_offset = info.anm_frame_offset.saturating_add(extra_subclip_offset);
            let anm_frame_offset = cls_frame_offset.saturating_add(anm_gameplay_prefix);
            let bounding_sphere = ActorBoundingSphere::for_actor(name, anim_info);
            actors.push(CutsceneActorRuntime {
                slot_index: idx,
                actor_name: name.clone(),
                anim_info,
                anm_path,
                cls_path,
                script_visible: false,
                culled_offscreen_4x3: false,
                visible: false,
                bounding_sphere,
                position: Vec3::ZERO,
                yaw_radians: std::f32::consts::PI,
                current_aid: start_aid,
                subclip_index: 0,
                local_anm_frame: 0.0,
                anm_frame_override: None,
                anm_frame_offset,
                cls_frame_offset,
            });
        }

        let mut evaluator = DemoDdsEvaluator::new(script);
        let (init_state, _) = evaluator.sample_at(0.0, info.offset_x, info.offset_z);
        let fov_y =
            init_state.vertical_fov_radians(info.fov_scale(), viewport_aspect, widescreen_mode);
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
            viewport_aspect,
            viewport_height: SPECIAL_FX_REFERENCE_HEIGHT,
            widescreen_mode,
            border_mode: CutsceneBorderMode::Auto,
            closet_fix: ClosetCutsceneFixState::default(),
            piston_fix: PistonRoomFixState::default(),
            maria_fog_counters: (false, false),
            atmosphere: AtmosphereGradingEvaluation::default(),
            special_fx: SpecialFxEvaluation::default(),
            piston_array: PistonArrayRuntime::new(),
            closet_door: ClosetDoorReplacementState::default(),
            camera,
            current_state: init_state,
            actors,
        };
        player.sync_camera_and_actors();
        player
    }

    /// Updates the active viewport aspect ratio (`width / height`) and widescreen mode,
    /// re-evaluating camera FOV, scene fixes, and off-screen 4:3 actor culling immediately.
    pub fn set_widescreen_config(
        &mut self,
        viewport_aspect: f32,
        widescreen_mode: CutsceneWidescreenMode,
    ) {
        self.viewport_aspect = viewport_aspect.max(0.5);
        self.widescreen_mode = widescreen_mode;
        self.reduce_widescreen_fov = widescreen_mode == CutsceneWidescreenMode::Sh2eeZoom16x9;
        self.sync_camera_and_actors();
    }

    /// Updates the active viewport dimensions `(width, height)` and widescreen mode, re-evaluating
    /// resolution-scaled PS2 DoF offsets (`display_ratio = height / 1080.0`) and camera framing (`SH2-105`).
    pub fn set_viewport_dimensions(
        &mut self,
        viewport_width: f32,
        viewport_height: f32,
        widescreen_mode: CutsceneWidescreenMode,
    ) {
        let safe_h = viewport_height.max(1.0);
        self.viewport_height = safe_h;
        self.viewport_aspect = (viewport_width / safe_h).max(0.5);
        self.widescreen_mode = widescreen_mode;
        self.reduce_widescreen_fov = widescreen_mode == CutsceneWidescreenMode::Sh2eeZoom16x9;
        self.sync_camera_and_actors();
    }

    /// Returns `true` if top/bottom `4:3` cutscene letterbox bars are suppressed on the current viewport
    /// (`DisableCutsceneBorders = Auto` in `SH2-77`).
    #[inline]
    pub fn letterbox_bars_hidden(&self) -> bool {
        self.border_mode
            .should_hide_letterbox_bars(self.viewport_aspect, self.widescreen_mode)
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
        if PistonArrayRuntime::is_piston_room_active(0, self.info.demo_no) {
            self.piston_array
                .step(dt_sec, self.info.demo_no, self.current_state.camera_pos[0]);
        }
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
        self.piston_fix = PistonRoomFixState::default();
        self.piston_array = PistonArrayRuntime::new();
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
            is_hard_cut: self.evaluator.is_cut,
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
