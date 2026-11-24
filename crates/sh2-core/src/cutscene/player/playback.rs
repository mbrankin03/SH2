//! `DramaDemoPlayer` initialization, playback ticking, voice sync, seeking, skipping, and queries.

use glam::Vec3;
use sh2_assets::formats::demo::{
    CutsceneBorderMode, CutsceneWidescreenMode, DemoDdsEvaluator, DemoDdsFile, DemoLightKind,
    DemoLightState, DEMO_MAX_LIGHTS, STANDARD_16X9_ASPECT,
};

use super::super::atmosphere::AtmosphereGradingEvaluation;
use super::super::catalog::DramaDemoPlayInfo;
use super::super::piston_kinematics::{ClosetDoorReplacementState, PistonArrayRuntime};
use super::super::special_fx::{SpecialFxEvaluation, SPECIAL_FX_REFERENCE_HEIGHT};
use super::super::stage_fixes::{ClosetCutsceneFixState, PistonRoomFixState};
use super::super::CUTSCENE_NATIVE_FPS;
use super::actor::build_cutscene_actors;
use super::{
    snap_demo_frame_boundary, CutsceneCameraSnapshot, DemoDeltaFrameHook, DemoVoiceBufferState,
    DramaDemoLetterboxRects, DramaDemoPlayer, DramaDemoStep,
};

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
        let actors = build_cutscene_actors(&info, &script.header.character_names);

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
            voice_preload_requested: false,
            voice_started: false,
            pending_voice_trigger: false,
            pending_voice_stop: false,
            voice_stall_secs: 0.0,
            last_df_hook: DemoDeltaFrameHook::Normal,
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

    /// Returns the authentic PS2 `DramaDemoFade` (`SLUS_202.28` `0x001b3390`) Q12 letterbox bar
    /// geometry when `Check_Filter_Soft() == 0` (`!filter_soft_active`) and letterbox bars are not
    /// hidden by the active widescreen border mode.
    #[inline]
    pub fn ps2_letterbox_geometry(
        &self,
        filter_soft_active: bool,
    ) -> Option<DramaDemoLetterboxRects> {
        if filter_soft_active || self.letterbox_bars_hidden() {
            None
        } else {
            Some(DramaDemoLetterboxRects::ps2_authentic())
        }
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
        for pos in &mut final_state.char_positions {
            pos[0] += self.info.offset_x;
            pos[2] += self.info.offset_z;
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
        let _ = self.tick_with_voice_buffer_sync(
            dt_sec,
            subtitles_enabled,
            voice_pos_secs,
            DemoVoiceBufferState::Ready,
        );
    }

    /// Advances the cutscene by `dt_sec` seconds with full `DramaDemoMain` (`0x001b0f00` / `0x004a2110`)
    /// voice-buffer stall (`shSetDFZero` / `shResetDF`) and `DdsPlay` (`0x001b1980`) `0.9999`
    /// frame-boundary snap parity (`SH2-134`).
    pub fn tick_with_voice_buffer_sync(
        &mut self,
        dt_sec: f32,
        subtitles_enabled: bool,
        voice_pos_secs: Option<f32>,
        buffer_state: DemoVoiceBufferState,
    ) -> DemoDeltaFrameHook {
        if self.step == DramaDemoStep::Finished {
            self.last_df_hook = DemoDeltaFrameHook::Normal;
            return self.last_df_hook;
        }

        let safe_dt = dt_sec.max(0.0);
        let mut df_hook = DemoDeltaFrameHook::Normal;
        let mut effective_dt = safe_dt;
        let stream_start_f = self.info.stream_start.max(0) as f32;

        // `DramaDemoMain` (`SLUS_202.28` `0x001b0f00` `Event/demoview.c:93-116`,
        // `sh2pc.exe` `FUN_004a2110` `0x004a2110`): stream preload (`0x80`) and stream-start
        // readiness gate (`0x100`) are mutually exclusive (`if / else if`) per tick.
        if self.info.stream_no > 0 {
            if !self.voice_preload_requested {
                self.voice_preload_requested = true;
                df_hook = DemoDeltaFrameHook::ResetDeltaFrame;
            } else if !self.voice_started
                && (self.scene_frame + safe_dt * CUTSCENE_NATIVE_FPS) >= stream_start_f
            {
                match buffer_state {
                    DemoVoiceBufferState::Ready | DemoVoiceBufferState::Playing => {
                        self.voice_started = true;
                        self.pending_voice_trigger = true;
                        df_hook = DemoDeltaFrameHook::ResetDeltaFrame;
                    }
                    DemoVoiceBufferState::Buffering | DemoVoiceBufferState::Idle => {
                        // Stream buffer not ready: accumulate `_stop_counter_896` and emit `shSetDFZero()`.
                        self.voice_stall_secs += safe_dt;
                        effective_dt = 0.0;
                        df_hook = DemoDeltaFrameHook::FreezeDeltaFrame;
                    }
                }
            }
        }

        let delta_frames = effective_dt * CUTSCENE_NATIVE_FPS;
        let total_f = self.total_frames() as f32;

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

        // `DdsPlay` (`SLUS_202.28` `0x001b1980`, `sh2pc.exe` `0x004a13c0`): snap fractional frames
        // exceeding `0.9999` up to the next integer frame boundary.
        self.scene_frame = snap_demo_frame_boundary(self.scene_frame);
        self.msg_frame = snap_demo_frame_boundary(self.msg_frame);

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
                self.last_df_hook = df_hook;
                return df_hook;
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
            self.piston_array.step(
                effective_dt,
                self.info.demo_no,
                self.current_state.camera_pos[0],
            );
        }
        self.sync_camera_and_actors();
        self.last_df_hook = df_hook;
        df_hook
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

    /// Returns all currently visible dynamic lights from the `.dds` light slots
    /// (`0..total_lights().min(DEMO_MAX_LIGHTS)`), preserving exact `.dds` slot order
    /// (`Point`, then `Spot`, then `Directional`) matching `DramaDemoMain`
    /// (`SLUS_202.28` `Event/demoview.c` `0x001b0f00` lines 444–551).
    pub fn active_lights(&self) -> impl Iterator<Item = &DemoLightState> {
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
            .filter(|l| l.visible)
    }

    /// Returns the primary active dynamic light from the `.dds` light slots (`0..DEMO_MAX_LIGHTS`),
    /// suitable for driving the stage spotlight / point light shader uniform.
    pub fn primary_active_light(&self) -> Option<&DemoLightState> {
        self.active_lights().next()
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
}
