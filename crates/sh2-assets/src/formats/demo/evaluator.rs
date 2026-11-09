//! Stateful `.dds` keyframe evaluator (`DdsPlay` / `DramaDemoInit` / `DramaDemoSkipLast`) (`SH2-63`).

use super::widescreen::{
    authored_4x3_fov_radians, compute_widescreen_scr_z, CutsceneWidescreenMode,
};
use super::{
    dds_rotation_to_interest, DemoCameraOp, DemoCharOp, DemoDdsFile, DemoLightKind, DemoLightOp,
    DEMO_MAX_CHARS, DEMO_MAX_LIGHTS, DEMO_SCREEN_SY,
};

/// Evaluates state of a single dynamic light at a keyframe (`SceneLight` in `DdsPlayLight`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DemoLightState {
    /// Light kind (Point, Spot, or Directional).
    pub kind: DemoLightKind,
    /// Whether this light is active (`sub_key 1` vs `2`).
    pub visible: bool,
    /// Light position in world space (or `-direction` for `Directional` lights).
    pub position: [f32; 3],
    /// Light target / interest point in world space (for `Spot` lights).
    pub interest: [f32; 3],
    /// RGB light color (in `[0.0, 2.0]`).
    pub color: [f32; 3],
    /// Distance attenuation ramp `[falloff_start, falloff_end]` (`1.0` up to `start`, `0.0` from `end`).
    pub decay: [f32; 2],
    /// Spot light cone angles `[inner_half_angle_rad, outer_param]`.
    pub cone: [f32; 2],
}

impl Default for DemoLightState {
    fn default() -> Self {
        Self {
            kind: DemoLightKind::Point,
            visible: false,
            position: [0.0; 3],
            interest: [0.0, 0.0, 1.0],
            color: [1.0; 3],
            decay: [500.0, 3000.0],
            cone: [0.5, 0.7],
        }
    }
}

impl DemoLightState {
    /// Linearly interpolates between two light states `a` and `b` by `t in [0.0, 1.0]`.
    #[inline]
    pub fn lerp(a: &Self, b: &Self, t: f32) -> Self {
        let lerp_f = |x: f32, y: f32| x + (y - x) * t;
        Self {
            kind: b.kind,
            visible: b.visible,
            position: [
                lerp_f(a.position[0], b.position[0]),
                lerp_f(a.position[1], b.position[1]),
                lerp_f(a.position[2], b.position[2]),
            ],
            interest: [
                lerp_f(a.interest[0], b.interest[0]),
                lerp_f(a.interest[1], b.interest[1]),
                lerp_f(a.interest[2], b.interest[2]),
            ],
            color: [
                lerp_f(a.color[0], b.color[0]),
                lerp_f(a.color[1], b.color[1]),
                lerp_f(a.color[2], b.color[2]),
            ],
            decay: [
                lerp_f(a.decay[0], b.decay[0]),
                lerp_f(a.decay[1], b.decay[1]),
            ],
            cone: [lerp_f(a.cone[0], b.cone[0]), lerp_f(a.cone[1], b.cone[1])],
        }
    }

    /// Evaluates the authentic PS2/PC linear distance attenuation ramp (`0x50cff0`):
    /// `1.0` up to `decay[0]`, falling linearly to `0.0` at `decay[1]`.
    #[inline]
    pub fn evaluate_distance_decay(&self, distance: f32) -> f32 {
        let start = self.decay[0];
        let end = self.decay[1].max(start + 1.0);
        if distance <= start {
            1.0
        } else if distance >= end {
            0.0
        } else {
            (end - distance) / (end - start)
        }
    }
}

/// Snapshot of the cutscene camera, character transforms, and dynamic lights at an integer frame (`KeyState` in `DdsPlay`).
#[derive(Debug, Clone, PartialEq)]
pub struct DemoKeyframeState {
    /// Camera eye position in world space.
    pub camera_pos: [f32; 3],
    /// Camera look-at target (interest) in world space.
    pub camera_interest: [f32; 3],
    /// Camera roll angle in radians about the view axis.
    pub camera_roll: f32,
    /// Camera projection focal plane distance (`scr_z` before `1.14702` scale).
    pub camera_plane: f32,
    /// Character world root positions (`[x, y, z]`).
    pub char_positions: [[f32; 3]; DEMO_MAX_CHARS],
    /// Character visibility flags.
    pub char_visible: [bool; DEMO_MAX_CHARS],
    /// Scene dynamic light states.
    pub lights: [DemoLightState; DEMO_MAX_LIGHTS],
}

impl Default for DemoKeyframeState {
    fn default() -> Self {
        Self {
            camera_pos: [0.0, -1500.0, -2000.0],
            camera_interest: [0.0, -1000.0, 0.0],
            camera_roll: 0.0,
            camera_plane: 300.0,
            char_positions: [[0.0; 3]; DEMO_MAX_CHARS],
            char_visible: [false; DEMO_MAX_CHARS],
            lights: [DemoLightState::default(); DEMO_MAX_LIGHTS],
        }
    }
}

impl DemoKeyframeState {
    /// Linearly interpolates between `last` and `next` by `t in [0.0, 1.0]`, adding stage `(offset_x, offset_z)`.
    /// When `is_cut` is true (`Key 0x10`), `t` is forced to `0.0` so camera cuts never smear across frames.
    pub fn interpolate(
        last: &Self,
        next: &Self,
        raw_t: f32,
        is_cut: bool,
        offset_x: f32,
        offset_z: f32,
    ) -> Self {
        let t = if is_cut { 0.0 } else { raw_t.clamp(0.0, 1.0) };
        let lerp_f = |a: f32, b: f32| a + (b - a) * t;

        let mut char_positions = [[0.0; 3]; DEMO_MAX_CHARS];
        let mut char_visible = [false; DEMO_MAX_CHARS];
        for i in 0..DEMO_MAX_CHARS {
            char_positions[i] = [
                lerp_f(last.char_positions[i][0], next.char_positions[i][0]) + offset_x,
                lerp_f(last.char_positions[i][1], next.char_positions[i][1]),
                lerp_f(last.char_positions[i][2], next.char_positions[i][2]) + offset_z,
            ];
            char_visible[i] = next.char_visible[i];
        }

        let mut lights = [DemoLightState::default(); DEMO_MAX_LIGHTS];
        for (i, slot) in lights.iter_mut().enumerate().take(DEMO_MAX_LIGHTS) {
            let mut l = DemoLightState::lerp(&last.lights[i], &next.lights[i], t);
            if l.kind != DemoLightKind::Directional {
                l.position[0] += offset_x;
                l.position[2] += offset_z;
                l.interest[0] += offset_x;
                l.interest[2] += offset_z;
            }
            *slot = l;
        }

        Self {
            camera_pos: [
                lerp_f(last.camera_pos[0], next.camera_pos[0]) + offset_x,
                lerp_f(last.camera_pos[1], next.camera_pos[1]),
                lerp_f(last.camera_pos[2], next.camera_pos[2]) + offset_z,
            ],
            camera_interest: [
                lerp_f(last.camera_interest[0], next.camera_interest[0]) + offset_x,
                lerp_f(last.camera_interest[1], next.camera_interest[1]),
                lerp_f(last.camera_interest[2], next.camera_interest[2]) + offset_z,
            ],
            camera_roll: lerp_f(last.camera_roll, next.camera_roll),
            camera_plane: lerp_f(last.camera_plane, next.camera_plane),
            char_positions,
            char_visible,
            lights,
        }
    }

    /// Computes aspect-aware vertical field-of-view in radians (`fov_y`) from `camera_plane`,
    /// `demo_fov_scale`, `viewport_aspect`, and [`CutsceneWidescreenMode`] (`DdsPlay` at `0x4a176b` / `SH2-77`):
    ///
    /// $$\text{scr\_z} = \text{plane} \times 1.14702 \times \text{demo\_fov\_scale}$$
    /// $$\text{fov\_y} = 2 \arctan\left(\frac{224.0}{\text{scr\_z}}\right)$$
    #[inline]
    pub fn vertical_fov_radians(
        &self,
        demo_fov_scale: f32,
        viewport_aspect: f32,
        mode: CutsceneWidescreenMode,
    ) -> f32 {
        let scr_z =
            compute_widescreen_scr_z(self.camera_plane, demo_fov_scale, viewport_aspect, mode);
        2.0 * (DEMO_SCREEN_SY * 0.5 / scr_z).atan()
    }

    /// Returns the authored `4:3` vertical and horizontal field-of-view `(fov_y_4x3, fov_x_4x3)` in radians
    /// for 4:3 off-screen parked actor frustum culling (`SH2-77`).
    #[inline]
    pub fn authored_4x3_fov_radians(&self, demo_fov_scale: f32) -> (f32, f32) {
        authored_4x3_fov_radians(self.camera_plane, demo_fov_scale)
    }
}

/// Stateful `.dds` keyframe evaluator (`DdsPlay` / `DramaDemoInit` / `DramaDemoSkipLast`).
#[derive(Debug, Clone)]
pub struct DemoDdsEvaluator {
    /// Loaded `.dds` cutscene script.
    pub script: DemoDdsFile,
    /// Index of the next unread frame record in `script.records`.
    pub next_record_idx: usize,
    /// Integer frame held in `next` (`-1` before initialization).
    pub loaded_frame: i32,
    /// Whether the transition from `last` to `next` is a hard camera cut (`Key 0x10`).
    pub is_cut: bool,
    /// Count of `Key 0x11` animation triggers encountered so far (`0..animation_trigger_frames.len()`).
    pub anim_trigger_count: usize,
    /// Frame number of the most recent `Key 0x11` animation trigger.
    pub last_anim_trigger_frame: u16,
    /// Previous integer frame state (`f - 1`).
    pub last: DemoKeyframeState,
    /// Current target integer frame state (`f`).
    pub next: DemoKeyframeState,
    /// Base camera `[position, interest]` updated on raw (`Key 0x10`) cuts.
    pub cam_base: [[f32; 3]; 2],
    /// Base character positions updated on raw (`Key 0x10`) cuts.
    pub char_base: [[f32; 3]; DEMO_MAX_CHARS],
    /// Base light `[position, interest]` updated on raw (`Key 0x10`) cuts.
    pub light_base: [[[f32; 3]; 2]; DEMO_MAX_LIGHTS],
}

impl DemoDdsEvaluator {
    /// Initializes the evaluator at Frame 0, matching `DramaDemoInit` (`0x1b1130` PS2 / `0x4a1052` PC).
    pub fn new(script: DemoDdsFile) -> Self {
        let mut init_state = DemoKeyframeState::default();
        for (i, slot) in init_state.lights.iter_mut().enumerate() {
            slot.kind = script.header.light_kind(i);
        }
        // Ground truth (`SLUS_202.28` `DramaDemoInit` at `0x1b12e8` / `sh2pc.exe` at `0x4a1244`):
        // every declared actor slot `0..n_chars` has `work[i]->status |= 0x10` enabled unconditionally.
        for vis in init_state
            .char_visible
            .iter_mut()
            .take(script.header.character_names.len().min(DEMO_MAX_CHARS))
        {
            *vis = true;
        }

        let mut eval = Self {
            script,
            next_record_idx: 0,
            loaded_frame: -1,
            is_cut: false,
            anim_trigger_count: 0,
            last_anim_trigger_frame: 0,
            last: init_state.clone(),
            next: init_state,
            cam_base: [[0.0; 3]; 2],
            char_base: [[0.0; 3]; DEMO_MAX_CHARS],
            light_base: [[[0.0; 3]; 2]; DEMO_MAX_LIGHTS],
        };
        eval.advance_to_frame(0);
        eval.last = eval.next.clone();
        eval.is_cut = false;
        eval
    }

    /// Resets the evaluator to Frame 0.
    pub fn reset(&mut self) {
        let script = self.script.clone();
        *self = Self::new(script);
    }

    /// Advances `loaded_frame` up to `target_frame` (where `DdsPlay` passes `cur_frame as i32 + 1`),
    /// returning the number of `Key 0x11` (`DramaDemoAnimationStart`) triggers that fired during this step.
    pub fn advance_to_frame(&mut self, target_frame: i32) -> usize {
        let prev_triggers = self.anim_trigger_count;
        while self.loaded_frame < target_frame {
            self.last = self.next.clone();
            self.is_cut = false;
            self.loaded_frame += 1;

            while self.next_record_idx < self.script.records.len()
                && (self.script.records[self.next_record_idx].frame as i32) <= self.loaded_frame
            {
                self.apply_record(self.next_record_idx);
                self.next_record_idx += 1;
            }
        }
        self.anim_trigger_count.saturating_sub(prev_triggers)
    }

    /// Fast-forwards through all remaining records in the `.dds` script to `0xFFFF` and copies `next`
    /// to `last`, matching `DramaDemoSkipLast` (`0x1b1be0` PS2 / `0x4a09f0` PC).
    pub fn skip_to_last(&mut self) {
        while self.next_record_idx < self.script.records.len() {
            self.apply_record(self.next_record_idx);
            self.next_record_idx += 1;
        }
        self.loaded_frame = self.script.header.total_frames as i32;
        self.last = self.next.clone();
        self.is_cut = true;
    }

    /// Evaluates the interpolated [`DemoKeyframeState`] at continuous 30 FPS `scene_frame`.
    pub fn sample_at(
        &mut self,
        scene_frame: f32,
        offset_x: f32,
        offset_z: f32,
    ) -> (DemoKeyframeState, usize) {
        let clamped = scene_frame.clamp(0.0, self.script.header.total_frames as f32);
        let target_int = (clamped.floor() as i32) + 1;
        if target_int < self.loaded_frame {
            self.reset();
        }
        let triggers = self.advance_to_frame(target_int);
        let frac = clamped - ((self.loaded_frame - 1) as f32);
        let state = DemoKeyframeState::interpolate(
            &self.last,
            &self.next,
            frac,
            self.is_cut,
            offset_x,
            offset_z,
        );
        (state, triggers)
    }

    fn apply_record(&mut self, rec_idx: usize) {
        let rec = &self.script.records[rec_idx];
        if rec.is_cut {
            self.is_cut = true;
        }
        if rec.trigger_animations {
            self.anim_trigger_count += 1;
            self.last_anim_trigger_frame = rec.frame;
        }

        for &op in &rec.camera_ops {
            match op {
                DemoCameraOp::Position { values, is_raw } => {
                    if is_raw {
                        self.cam_base[0] = values;
                        self.next.camera_pos = values;
                    } else {
                        for (k, &v) in values.iter().enumerate() {
                            self.next.camera_pos[k] = self.cam_base[0][k] + v;
                        }
                    }
                }
                DemoCameraOp::Interest { values, is_raw } => {
                    if is_raw {
                        self.cam_base[1] = values;
                        self.next.camera_interest = values;
                    } else {
                        for (k, &v) in values.iter().enumerate() {
                            self.next.camera_interest[k] = self.cam_base[1][k] + v;
                        }
                    }
                }
                DemoCameraOp::Rotation(rot) => {
                    self.next.camera_interest = dds_rotation_to_interest(self.next.camera_pos, rot);
                    self.next.camera_roll = rot[2];
                }
                DemoCameraOp::Roll(roll) => {
                    self.next.camera_roll = roll;
                }
                DemoCameraOp::Plane(plane) => {
                    self.next.camera_plane = plane;
                }
            }
        }

        for &(light_idx, op) in &rec.light_ops {
            if light_idx >= DEMO_MAX_LIGHTS {
                continue;
            }
            let l = &mut self.next.lights[light_idx];
            let base = &mut self.light_base[light_idx];
            match op {
                DemoLightOp::Visible(v) => l.visible = v,
                DemoLightOp::Position { values, is_raw } => {
                    if is_raw {
                        base[0] = values;
                        l.position = values;
                    } else {
                        for (k, &v) in values.iter().enumerate() {
                            l.position[k] = base[0][k] + v;
                        }
                    }
                }
                DemoLightOp::Interest { values, is_raw } => {
                    if is_raw {
                        base[1] = values;
                        l.interest = values;
                    } else {
                        for (k, &v) in values.iter().enumerate() {
                            l.interest[k] = base[1][k] + v;
                        }
                    }
                }
                DemoLightOp::Rotation(rot) => {
                    l.interest = dds_rotation_to_interest(l.position, rot);
                }
                DemoLightOp::Color(col) => l.color = col,
                DemoLightOp::Decay(dec) => l.decay = dec,
                DemoLightOp::Cone(cone) => l.cone = cone,
            }
        }

        for &(char_idx, op) in &rec.char_ops {
            if char_idx >= DEMO_MAX_CHARS {
                continue;
            }
            match op {
                DemoCharOp::Visible(_v) => {
                    // Ground truth (`SLUS_202.28` `DdsPlay` at `0x1b2348` / `sh2pc.exe` at `0x4a1ca3`):
                    // `DdsPlayCharacter` (`0x1b2d90` PS2 / `0x49ffb0` PC) writes `sub_key 1/2` to
                    // `next.chara[i].visible` (`0x11337d0` PS2 / `0x94dfe4` PC), which only records
                    // whether the `*_pos` null locator was visible in the Softimage 3D viewport when
                    // the scene was exported (e.g. `hhh_mar_pos` has `sub_key == 2` for the first
                    // 966 frames of `mar_deai_e.dds` and all of `mar_deai_c.dds` while Maria stands
                    // center-screen). Unlike `light[i].visible`, `DdsPlay` never reads `chara[i].visible`.
                    self.next.char_visible[char_idx] = true;
                }
                DemoCharOp::Position { values, is_raw } => {
                    if is_raw {
                        self.char_base[char_idx] = values;
                        self.next.char_positions[char_idx] = values;
                    } else {
                        for (k, &v) in values.iter().enumerate() {
                            self.next.char_positions[char_idx][k] = self.char_base[char_idx][k] + v;
                        }
                    }
                }
            }
        }
    }
}
