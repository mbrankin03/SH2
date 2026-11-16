//! Interactive 3D Model, Animation, and Texture Viewer / Bestiary debug controller (`SH2-36` / `SH2-37`).
//!
//! Provides a 360-degree orbit camera, smooth turntable rotation, character/enemy/boss/weapon
//! catalog hot-swapping, equipment/hand-pose preset cycling, animation frame scrubbing,
//! playback speed control, and shading/texture inspection modes.

pub mod catalog;
pub mod modes;

pub use catalog::{ModelCategory, ModelViewerEntry, BESTIARY_CATALOG};
pub use modes::{OrbitCamera, StageLightingRig, TextureSourceMode, ViewerRenderMode};

use crate::subclips::{sub_clips_for_anm, ViewerSubClip};

/// Interactive state machine for the 3D Model, Animation, and Texture Viewer room (`SH2-36` / `SH2-37`).
#[derive(Debug, Clone)]
pub struct ModelViewerState {
    /// Index into [`BESTIARY_CATALOG`].
    pub selected_model_index: usize,
    /// Index into the active model's `anm_paths` list.
    pub selected_anm_index: usize,
    /// Optional index into the active `.anm` file's named PS2 sub-clips (`None` = play entire `.anm` track).
    pub selected_subclip_index: Option<usize>,
    /// Index into the active model's discovered `PoseVariantPreset` list (`0` = default PS2 active pose set).
    pub selected_pose_preset_index: usize,
    /// Total animation frames in the currently loaded `.anm` file.
    pub total_frames: usize,
    /// Continuous animation playhead position in frames `[0.0, total_frames)`.
    pub current_frame: f32,
    /// Whether animation playback is currently paused for frame-by-frame scrubbing.
    pub is_paused: bool,
    /// Playback speed multiplier (`-2.0` to `2.0`, default `1.0`).
    pub playback_speed: f32,
    /// Base animation framerate (30.0 FPS standard PS2 animation rate, or active sub-clip FPS).
    pub base_fps: f32,
    /// Whether the pedestal turntable auto-rotates the model continuously.
    pub turntable_enabled: bool,
    /// Current turntable rotation angle in radians around the vertical axis.
    pub turntable_yaw: f32,
    /// Turntable angular velocity in radians per second.
    pub turntable_speed: f32,
    /// Active shading / geometry inspection mode.
    pub render_mode: ViewerRenderMode,
    /// Whether wireframe overlay is drawn simultaneously over textured/clay modes.
    pub wireframe_overlay: bool,
    /// Whether bone skeleton and joint axes are drawn simultaneously over the mesh.
    pub skeleton_overlay: bool,
    /// Texture comparison toggle (4K AI Master vs SH2:EE HD vs Vanilla PS2).
    pub texture_source: TextureSourceMode,
    /// Active neutral stage lighting rig preset.
    pub lighting_rig: StageLightingRig,
    /// Orbit camera controller.
    pub camera: OrbitCamera,
}

impl Default for ModelViewerState {
    fn default() -> Self {
        Self::new()
    }
}

impl ModelViewerState {
    /// Initializes the Model Viewer focused on the first catalog entry (`hhh_jms`).
    pub fn new() -> Self {
        let mut camera = OrbitCamera::default();
        camera.reset_for_entry(&BESTIARY_CATALOG[0]);

        Self {
            selected_model_index: 0,
            selected_anm_index: 0,
            selected_subclip_index: None,
            selected_pose_preset_index: 0,
            total_frames: 1,
            current_frame: 0.0,
            is_paused: false,
            playback_speed: 1.0,
            base_fps: 30.0,
            turntable_enabled: false,
            turntable_yaw: 0.0,
            turntable_speed: 0.65,
            render_mode: ViewerRenderMode::Textured,
            wireframe_overlay: false,
            skeleton_overlay: false,
            texture_source: TextureSourceMode::Ai4kMaster,
            lighting_rig: StageLightingRig::StudioNeutral,
            camera,
        }
    }

    /// Returns the currently selected [`ModelViewerEntry`].
    pub fn current_entry(&self) -> &'static ModelViewerEntry {
        &BESTIARY_CATALOG[self.selected_model_index % BESTIARY_CATALOG.len()]
    }

    /// Returns the relative path of the currently selected `.anm` file, if any.
    pub fn current_anm_path(&self) -> Option<&'static str> {
        let entry = self.current_entry();
        if entry.anm_paths.is_empty() {
            None
        } else {
            Some(entry.anm_paths[self.selected_anm_index % entry.anm_paths.len()])
        }
    }

    /// Returns the slice of named PS2 sub-clips defined for the active `(model, anm)` pair.
    pub fn available_subclips(&self) -> &'static [ViewerSubClip] {
        let entry = self.current_entry();
        if let Some(anm_path) = self.current_anm_path() {
            sub_clips_for_anm(entry.id, anm_path)
        } else {
            &[]
        }
    }

    /// Returns the currently selected [`ViewerSubClip`], if any.
    pub fn active_subclip(&self) -> Option<ViewerSubClip> {
        let idx = self.selected_subclip_index?;
        let subclips = self.available_subclips();
        subclips.get(idx).copied()
    }

    /// Cycles `selected_pose_preset_index` across `0..preset_count` and returns the new index.
    pub fn cycle_pose_preset(&mut self, preset_count: usize) -> usize {
        if preset_count <= 1 {
            self.selected_pose_preset_index = 0;
        } else {
            self.selected_pose_preset_index = (self.selected_pose_preset_index + 1) % preset_count;
        }
        self.selected_pose_preset_index
    }

    /// Returns `true` if the active `.anm` is the shared 8-weapon pose lookup table (`jms_weapon.anm`),
    /// where each frame `0..=7` belongs to a different weapon's 2-bone proportions and must stay
    /// locked to the active weapon's dedicated pose sub-clip.
    fn is_shared_weapon_pose_table(&self) -> bool {
        self.current_anm_path() == Some("data/chr/wp/jms_weapon.anm")
            && !self.available_subclips().is_empty()
    }

    fn apply_default_subclip_for_current_anm(&mut self) {
        if self.is_shared_weapon_pose_table() {
            self.selected_subclip_index = Some(0);
            if let Some(sc) = self.active_subclip() {
                self.current_frame = sc.clip.start_frame as f32;
                self.base_fps = sc.clip.fps;
            }
        } else {
            self.selected_subclip_index = None;
            self.base_fps = 30.0;
            self.current_frame = 0.0;
        }
    }

    /// Cycles to the next named PS2 sub-clip within the active `.anm` file, wrapping back to
    /// `None` ("ALL FRAMES") after the last sub-clip.
    pub fn next_subclip(&mut self) {
        let subclips = self.available_subclips();
        if subclips.is_empty() {
            self.selected_subclip_index = None;
            self.base_fps = 30.0;
            return;
        }
        if self.is_shared_weapon_pose_table() {
            self.selected_subclip_index = Some(0);
            if let Some(sc) = self.active_subclip() {
                self.current_frame = sc.clip.start_frame as f32;
                self.base_fps = sc.clip.fps;
            }
            return;
        }
        self.selected_subclip_index = match self.selected_subclip_index {
            None => Some(0),
            Some(i) if i + 1 < subclips.len() => Some(i + 1),
            Some(_) => None,
        };
        if let Some(sc) = self.active_subclip() {
            self.current_frame = sc.clip.start_frame as f32;
            self.base_fps = sc.clip.fps;
        } else {
            self.current_frame = 0.0;
            self.base_fps = 30.0;
        }
    }

    /// Cycles to the previous named PS2 sub-clip within the active `.anm` file.
    pub fn prev_subclip(&mut self) {
        let subclips = self.available_subclips();
        if subclips.is_empty() {
            self.selected_subclip_index = None;
            self.base_fps = 30.0;
            return;
        }
        if self.is_shared_weapon_pose_table() {
            self.selected_subclip_index = Some(0);
            if let Some(sc) = self.active_subclip() {
                self.current_frame = sc.clip.start_frame as f32;
                self.base_fps = sc.clip.fps;
            }
            return;
        }
        self.selected_subclip_index = match self.selected_subclip_index {
            None => Some(subclips.len() - 1),
            Some(0) => None,
            Some(i) => Some(i - 1),
        };
        if let Some(sc) = self.active_subclip() {
            self.current_frame = sc.clip.start_frame as f32;
            self.base_fps = sc.clip.fps;
        } else {
            self.current_frame = 0.0;
            self.base_fps = 30.0;
        }
    }

    /// Selects a model by matching its CLI name or substring (case-insensitive).
    /// Prioritizes exact `id` match, then `id` substring match, then `display_name` substring match.
    /// Returns `true` if a matching model was found.
    pub fn select_by_name(&mut self, query: &str) -> bool {
        let q = query.trim().to_ascii_lowercase();
        if q.is_empty() {
            return false;
        }

        // 1. Exact ID match first
        for (idx, entry) in BESTIARY_CATALOG.iter().enumerate() {
            if entry.id.eq_ignore_ascii_case(&q) {
                self.set_model_index(idx);
                return true;
            }
        }

        // 2. Substring match against ID
        for (idx, entry) in BESTIARY_CATALOG.iter().enumerate() {
            if entry.id.to_ascii_lowercase().contains(&q) {
                self.set_model_index(idx);
                return true;
            }
        }

        // 3. Substring match against display name
        for (idx, entry) in BESTIARY_CATALOG.iter().enumerate() {
            if entry.display_name.to_ascii_lowercase().contains(&q) {
                self.set_model_index(idx);
                return true;
            }
        }

        false
    }

    /// Sets the active model index and resets animation clip, pose preset, and camera framing.
    pub fn set_model_index(&mut self, index: usize) {
        self.selected_model_index = index % BESTIARY_CATALOG.len();
        self.selected_anm_index = 0;
        self.selected_pose_preset_index = 0;
        self.apply_default_subclip_for_current_anm();
        self.camera.reset_for_entry(self.current_entry());
    }

    /// Cycles to the next model in the Bestiary catalog.
    pub fn next_model(&mut self) {
        let next = (self.selected_model_index + 1) % BESTIARY_CATALOG.len();
        self.set_model_index(next);
    }

    /// Cycles to the previous model in the Bestiary catalog.
    pub fn prev_model(&mut self) {
        let prev = if self.selected_model_index == 0 {
            BESTIARY_CATALOG.len() - 1
        } else {
            self.selected_model_index - 1
        };
        self.set_model_index(prev);
    }

    /// Cycles to the next `.anm` animation file for the active model.
    pub fn next_animation_clip(&mut self) {
        let count = self.current_entry().anm_paths.len().max(1);
        self.selected_anm_index = (self.selected_anm_index + 1) % count;
        self.apply_default_subclip_for_current_anm();
    }

    /// Cycles to the previous `.anm` animation file for the active model.
    pub fn prev_animation_clip(&mut self) {
        let count = self.current_entry().anm_paths.len().max(1);
        if self.selected_anm_index == 0 {
            self.selected_anm_index = count - 1;
        } else {
            self.selected_anm_index -= 1;
        }
        self.apply_default_subclip_for_current_anm();
    }

    /// Updates total frame count when a new `.anm` file is loaded.
    pub fn set_total_frames(&mut self, frames: usize) {
        self.total_frames = frames.max(1);
        self.current_frame = self.current_frame.rem_euclid(self.total_frames as f32);
    }

    /// Toggles animation play/pause state.
    pub fn toggle_pause(&mut self) {
        self.is_paused = !self.is_paused;
    }

    /// Steps animation playhead forward or backward by `delta_frames` (automatically pauses).
    pub fn step_frame(&mut self, delta_frames: i32) {
        self.is_paused = true;
        if let Some(sc) = self.active_subclip() {
            let start = (sc.clip.start_frame as usize).min(self.total_frames.saturating_sub(1));
            let end = (sc.clip.end_frame as usize).min(self.total_frames.saturating_sub(1));
            let span = (end.saturating_sub(start) + 1).max(1) as f32;
            let rel =
                (self.current_frame.floor() - start as f32 + delta_frames as f32).rem_euclid(span);
            self.current_frame = start as f32 + rel;
        } else {
            let total = self.total_frames.max(1) as f32;
            self.current_frame =
                (self.current_frame.floor() + delta_frames as f32).rem_euclid(total);
        }
    }

    /// Adjusts playback speed multiplier within `[-2.0, 2.0]`.
    pub fn adjust_playback_speed(&mut self, delta: f32) {
        let next = ((self.playback_speed + delta) * 10.0).round() / 10.0;
        self.playback_speed = next.clamp(-2.0, 2.0);
    }

    /// Toggles reverse playback direction.
    pub fn reverse_playback(&mut self) {
        self.playback_speed = -self.playback_speed;
    }

    /// Advances animation playhead and turntable rotation by `dt` seconds.
    pub fn tick(&mut self, dt: f32) {
        if self.turntable_enabled {
            self.turntable_yaw =
                (self.turntable_yaw + self.turntable_speed * dt) % std::f32::consts::TAU;
        }

        if !self.is_paused && self.total_frames > 1 {
            if let Some(sc) = self.active_subclip() {
                let start = (sc.clip.start_frame as usize).min(self.total_frames - 1) as f32;
                let end = (sc.clip.end_frame as usize).min(self.total_frames - 1) as f32;
                let span = (end - start).max(0.0);
                if span <= f32::EPSILON {
                    self.current_frame = start;
                } else {
                    let frame_delta = sc.clip.fps * self.playback_speed * dt;
                    let rel = (self.current_frame - start + frame_delta).rem_euclid(span);
                    self.current_frame = start + rel;
                }
            } else {
                let total = self.total_frames as f32;
                let frame_delta = self.base_fps * self.playback_speed * dt;
                self.current_frame = (self.current_frame + frame_delta).rem_euclid(total);
            }
        }
    }

    /// Returns `(frame_0, frame_1, interpolation_alpha)` for sampling the active animation track.
    pub fn sample_cursor(&self) -> (usize, usize, f32) {
        if self.total_frames <= 1 {
            return (0, 0, 0.0);
        }
        if let Some(sc) = self.active_subclip() {
            let start = (sc.clip.start_frame as usize).min(self.total_frames - 1);
            let end = (sc.clip.end_frame as usize).min(self.total_frames - 1);
            let f0 = (self.current_frame.floor() as usize).clamp(start, end);
            let f1 = if f0 >= end {
                if sc.clip.looping {
                    start
                } else {
                    end
                }
            } else {
                f0 + 1
            };
            let alpha = (self.current_frame - self.current_frame.floor()).clamp(0.0, 1.0);
            (f0, f1, alpha)
        } else {
            let f0 = (self.current_frame.floor() as usize) % self.total_frames;
            let f1 = (f0 + 1) % self.total_frames;
            let alpha = (self.current_frame - self.current_frame.floor()).clamp(0.0, 1.0);
            (f0, f1, alpha)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_catalog_completeness_and_lookup() {
        assert_eq!(BESTIARY_CATALOG.len(), 31);

        let mut viewer = ModelViewerState::new();
        assert_eq!(viewer.current_entry().id, "hhh_jms");

        // Verify all 3 James LODs plus Maria, NPCs, enemies, bosses, and weapons exist in catalog
        assert!(viewer.select_by_name("hhh_jms"));
        assert_eq!(viewer.current_entry().category, ModelCategory::Protagonist);

        assert!(viewer.select_by_name("hll_jms"));
        assert_eq!(
            viewer.current_entry().mdl_paths,
            &["data/chr/jms/hll_jms.mdl"]
        );

        assert!(viewer.select_by_name("lll_jms"));
        assert_eq!(
            viewer.current_entry().mdl_paths,
            &["data/chr/jms/lll_jms.mdl"]
        );

        // Verify pose preset cycling resets on model change
        assert_eq!(viewer.cycle_pose_preset(9), 1);
        assert_eq!(viewer.cycle_pose_preset(9), 2);
        assert!(viewer.select_by_name("lll_mar"));
        assert_eq!(viewer.selected_pose_preset_index, 0);
        assert!(!viewer.available_subclips().is_empty());

        assert!(viewer.select_by_name("xmar"));
        assert_eq!(viewer.current_entry().id, "xmar");

        assert!(viewer.select_by_name("lau"));
        assert_eq!(viewer.current_entry().category, ModelCategory::StoryNpc);

        assert!(viewer.select_by_name("scu"));
        assert_eq!(viewer.current_entry().category, ModelCategory::Enemy);

        assert!(viewer.select_by_name("red"));
        assert_eq!(viewer.current_entry().category, ModelCategory::Boss);

        assert!(viewer.select_by_name("pipe"));
        assert_eq!(viewer.current_entry().category, ModelCategory::WeaponOrItem);
    }

    #[test]
    fn test_animation_scrubbing_and_speed() {
        let mut viewer = ModelViewerState::new();
        viewer.set_total_frames(100);

        viewer.tick(1.0);
        assert!((viewer.current_frame - 30.0).abs() < 0.01);

        viewer.step_frame(1);
        assert!(viewer.is_paused);
        assert_eq!(viewer.current_frame, 31.0);

        viewer.current_frame = 0.0;
        viewer.step_frame(-1);
        assert_eq!(viewer.current_frame, 99.0);

        viewer.is_paused = false;
        viewer.reverse_playback();
        assert_eq!(viewer.playback_speed, -1.0);
        viewer.tick(0.5);
        assert!((viewer.current_frame - 84.0).abs() < 0.01);
    }

    #[test]
    fn test_orbit_camera_and_render_modes() {
        let mut viewer = ModelViewerState::new();
        let initial_dist = viewer.camera.distance;
        viewer.camera.zoom(-400.0);
        assert_eq!(viewer.camera.distance, initial_dist - 400.0);

        viewer.camera.orbit(0.5, 0.25);
        let view = viewer.camera.view_matrix();
        assert!(!view.is_nan());

        assert_eq!(viewer.render_mode, ViewerRenderMode::Textured);
        viewer.render_mode = viewer.render_mode.next();
        assert_eq!(viewer.render_mode, ViewerRenderMode::Wireframe);
        viewer.render_mode = viewer.render_mode.next();
        assert_eq!(viewer.render_mode, ViewerRenderMode::SkeletonJoints);
        viewer.render_mode = viewer.render_mode.next();
        assert_eq!(viewer.render_mode, ViewerRenderMode::UntexturedClay);
        assert_eq!(viewer.render_mode.shader_mode_code(), 1);
    }

    #[test]
    fn test_sh2_34_npc_subclip_cycling_and_playback_bounds() {
        let mut viewer = ModelViewerState::new();
        assert!(viewer.select_by_name("agl"));
        viewer.set_total_frames(298);
        assert_eq!(viewer.available_subclips().len(), 4);
        assert!(viewer.active_subclip().is_none());

        viewer.next_subclip();
        let sc0 = viewer.active_subclip().expect("subclip 0 active");
        assert_eq!(sc0.clip.start_frame, 133);
        assert_eq!(sc0.clip.end_frame, 206);
        assert!((viewer.base_fps - 1.875).abs() < 1e-4);

        viewer.next_subclip();
        let sc1 = viewer.active_subclip().expect("subclip 1 active");
        assert_eq!(sc1.clip.start_frame, 0);
        assert_eq!(sc1.clip.end_frame, 63);
        assert!((viewer.current_frame - 0.0).abs() < 1e-4);

        viewer.step_frame(-1);
        assert_eq!(viewer.current_frame, 63.0);
        let (f0, f1, _) = viewer.sample_cursor();
        assert_eq!(f0, 63);
        assert_eq!(f1, 0);

        assert!(viewer.select_by_name("lll_edi"));
        viewer.set_total_frames(437);
        assert_eq!(viewer.available_subclips().len(), 21);

        let weapon_frames = [
            ("wp_handgun", 0usize),
            ("wp_shotgun", 1),
            ("wp_riflgun", 2),
            ("wp_sp", 3),
            ("wp_kakuzai", 4),
            ("wp_pipe", 5),
            ("wp_nata", 6),
            ("wp_csaw", 7),
        ];
        for (wp_id, expected_frame) in weapon_frames {
            assert!(viewer.select_by_name(wp_id));
            viewer.set_total_frames(8);
            viewer.is_paused = false;
            assert_eq!(viewer.selected_subclip_index, Some(0));
            viewer.tick(0.5);
            let (f0, f1, _) = viewer.sample_cursor();
            assert_eq!(f0, expected_frame);
            assert_eq!(f1, expected_frame);
        }
    }

    #[test]
    fn test_all_bestiary_catalog_models_and_animations_skin_cleanly() {
        use sh2_assets::formats::anm::{
            compute_bind_locals, compute_skin_matrices, evaluate_pose, parse_anm,
        };
        use sh2_assets::formats::mdls::parse_mdl;
        use std::path::Path;

        let root = "../../Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2";
        if !Path::new(root).exists() {
            return;
        }

        for entry in BESTIARY_CATALOG {
            let mut mdl_bytes = None;
            for &mp in entry.mdl_paths {
                let p = format!("{root}/{mp}");
                if let Ok(b) = std::fs::read(&p) {
                    mdl_bytes = Some((mp, b));
                    break;
                }
            }
            let (mdl_path, mdl_bytes) = mdl_bytes
                .unwrap_or_else(|| panic!("Catalog entry '{}' missing all .mdl files", entry.id));
            let model = parse_mdl(&mdl_bytes)
                .unwrap_or_else(|e| panic!("Failed to parse '{}': {:?}", mdl_path, e));
            let bind_locals = compute_bind_locals(&model.parents, &model.initial_matrices);

            for (vi, (bones, weights)) in model
                .vertices
                .bone_indices
                .iter()
                .zip(model.vertices.weights.iter())
                .enumerate()
            {
                let sum: f32 = weights.iter().sum();
                assert!(
                    (sum - 1.0).abs() < 1e-3,
                    "Entry '{}' ({}) vertex {} weights sum to {}",
                    entry.id,
                    mdl_path,
                    vi,
                    sum
                );
                for j in 0..4 {
                    if weights[j] > 0.0 {
                        assert!(
                            (bones[j] as usize) < model.bone_count,
                            "Entry '{}' ({}) vertex {} weight[{}]={} has out-of-range bone_idx={}",
                            entry.id,
                            mdl_path,
                            vi,
                            j,
                            weights[j],
                            bones[j]
                        );
                    }
                }
            }

            for &ap in entry.anm_paths {
                let p = format!("{root}/{ap}");
                let anm_bytes = std::fs::read(&p)
                    .unwrap_or_else(|_| panic!("Catalog entry '{}' missing '{}'", entry.id, ap));
                let anm =
                    parse_anm(&anm_bytes, model.bone_count, &model.parents).unwrap_or_else(|e| {
                        panic!(
                            "Entry '{}' ({}, {} bones) failed to parse '{}': {:?}",
                            entry.id, mdl_path, model.bone_count, ap, e
                        )
                    });
                assert!(
                    !anm.frames.is_empty(),
                    "Entry '{}' animation '{}' produced 0 frames",
                    entry.id,
                    ap
                );

                for sc in crate::subclips::sub_clips_for_anm(entry.id, ap) {
                    assert!(
                        sc.clip.start_frame <= sc.clip.end_frame
                            && (sc.clip.end_frame as usize) < anm.frames.len(),
                        "Entry '{}' animation '{}' sub-clip '{}' range {}..={} exceeds total frames {}",
                        entry.id,
                        ap,
                        sc.label,
                        sc.clip.start_frame,
                        sc.clip.end_frame,
                        anm.frames.len()
                    );
                }

                let check_frames = [0, anm.frames.len() / 2, anm.frames.len() - 1];
                for &fi in &check_frames {
                    let pose = evaluate_pose(&model.parents, &bind_locals, &anm.frames[fi]);
                    let skin = compute_skin_matrices(&pose, &model.initial_matrices);

                    let mut min_pt = [f32::INFINITY; 3];
                    let mut max_pt = [f32::NEG_INFINITY; 3];

                    for (pos, (bones, weights)) in model.vertices.positions.iter().zip(
                        model
                            .vertices
                            .bone_indices
                            .iter()
                            .zip(model.vertices.weights.iter()),
                    ) {
                        let mut skinned = [0.0f32; 3];
                        for j in 0..4 {
                            let w = weights[j];
                            if w > 0.0 {
                                let m = &skin[bones[j] as usize];
                                skinned[0] += w
                                    * (m[0][0] * pos[0]
                                        + m[0][1] * pos[1]
                                        + m[0][2] * pos[2]
                                        + m[0][3]);
                                skinned[1] += w
                                    * (m[1][0] * pos[0]
                                        + m[1][1] * pos[1]
                                        + m[1][2] * pos[2]
                                        + m[1][3]);
                                skinned[2] += w
                                    * (m[2][0] * pos[0]
                                        + m[2][1] * pos[1]
                                        + m[2][2] * pos[2]
                                        + m[2][3]);
                            }
                        }
                        for c in 0..3 {
                            assert!(
                                skinned[c].is_finite(),
                                "Non-finite skinned coord in '{}' / '{}' frame {}",
                                entry.id,
                                ap,
                                fi
                            );
                            min_pt[c] = min_pt[c].min(skinned[c]);
                            max_pt[c] = max_pt[c].max(skinned[c]);
                        }
                    }

                    let span_x = max_pt[0] - min_pt[0];
                    let span_y = max_pt[1] - min_pt[1];
                    let span_z = max_pt[2] - min_pt[2];
                    assert!(
                        span_x < 5000.0 && span_y < 5000.0 && span_z < 5000.0,
                        "Exploding/skewed vertices detected in '{}' ('{}' frame {}): span=({:.1}, {:.1}, {:.1})",
                        entry.id,
                        ap,
                        fi,
                        span_x,
                        span_y,
                        span_z
                    );
                }
            }
        }
    }
}
