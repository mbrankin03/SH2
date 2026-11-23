//! 1:1 port of `Chacter_Draw/light_n.c` (`SLUS_202.28` `0x001260b0..0x00127800`):
//! 12-slot `LightPointer` pool, `UpdateIntensity`, `UpdateFParam`, `UpdateSParam`,
//! `CalcInfluence`, `UpdateParallels`, `UpdateReflections` (NHM sphere-map basis),
//! and `UpdateExtras` (top-6 influence selection).

use glam::{Mat4, Vec3, Vec4};

use super::uniforms::compute_ps2_falloff_params;

/// Fixed character light pool capacity (`LightPointer` @ `0x001260b0`, `param_1 < 0xc`).
pub const CHARACTER_LIGHT_SLOT_COUNT: usize = 12;

/// Maximum number of active point/spot extra lights selected per character (`UpdateExtras` @ `0x00127360`,
/// `if (5 < __nmemb) __nmemb = 6`).
pub const CHARACTER_EXTRA_LIGHT_LIMIT: usize = 6;

/// PS2 luminance weights used by `UpdateIntensity` (`0x00126120`) and `UpdateReflections` (`0x00126f90`),
/// extracted from `SLUS_202.28` `0x002a9540`.
pub const PS2_LUMA_WEIGHTS: [f32; 3] = [0.3, 0.6, 0.1];

/// NHM row 3 UV bias vector from `SLUS_202.28` `0x002a9550`, copied into `0x419d10` / `0x419d50`
/// by `UpdateReflections` (`0x00126f90`).
pub const PS2_NHM_UV_BIAS_ROW: [f32; 4] = [0.5, 0.5, 0.0, 1.0];

/// Spot spread offset scale in `sh_Kari_LightSetSpot` (`0x001265f0`: `__s[0x23] = -*param_5 * 0.8`).
pub const KARI_SPOT_SPREAD_BIAS_SCALE: f32 = 0.8;

/// Light entry type discriminant (`*light_ptr` in `Chacter_Draw/light_n.c`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u32)]
pub enum CharacterLightType {
    /// Inactive slot (`LightDelete` `0x00126210`).
    #[default]
    None = 0,
    /// Infinite parallel light (`LightSetParallel` `0x00126290`).
    Parallel = 1,
    /// Omnidirectional point light (`LightSetPoint` `0x00126320`).
    Point = 2,
    /// Directional spotlight (`sh_Kari_LightSetSpot` `0x001265f0`).
    Spot = 3,
    /// Specular reflection highlight (`LightSetReflection` `0x00126740`).
    Reflection = 4,
}

/// Fake-light sub-kind (`*(u16*)(light + 0x16)` in `LightSetFakePoint` / `LightSetFakeSpot`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u16)]
pub enum FakeLightKind {
    #[default]
    Real = 0,
    FakePoint = 1,
    FakeSpot = 2,
}

/// Single `0xa0`-byte light slot in `light_work` (`LightPointer` `0x001260b0`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CharacterLightEntry {
    pub kind: CharacterLightType,
    pub fake_kind: FakeLightKind,
    /// `abs(signed_intensity)` (`+0x04`, `UpdateIntensity` `0x00126120`).
    pub abs_intensity: f32,
    /// `abs_intensity * atten` (`+0x08`, `CalcInfluence` `0x001269b0`).
    pub influence: f32,
    /// `dot(color, [0.3, 0.6, 0.1])` (`+0x0c`, `UpdateIntensity` `0x00126120`).
    pub signed_intensity: f32,
    /// `signed_intensity * atten` (`+0x10`, `CalcInfluence` `0x001269b0`).
    pub signed_influence: f32,
    /// Evaluated scalar attenuation (`+0x18`, `CalcInfluence` `0x001269b0`).
    pub atten_factor: f32,
    /// World-space position in PS2 mm (`+0x20`).
    pub world_pos: Vec3,
    /// Unit world-space direction (`+0x40`).
    pub world_dir: Vec3,
    /// Linear RGB color (`+0x60`).
    pub color: Vec3,
    /// Falloff near/far distances in PS2 mm (`+0x70`, `+0x74`).
    pub falloff_near_mm: f32,
    pub falloff_far_mm: f32,
    /// Spread inner/outer cosines (`+0x78`, `+0x7c`).
    pub spread_inner: f32,
    pub spread_outer: f32,
    /// Linear distance falloff `[fa, fb]` (`+0x80`, `+0x84`).
    pub linear_fa: f32,
    pub linear_fb: f32,
    /// Angular spread `[sa, sb]` (`+0x88`, `+0x8c`).
    pub spread_sa: f32,
    pub spread_sb: f32,
    /// Hyperbolic distance falloff `[h_fa, h_fb]` (`+0x90`, `+0x94`).
    pub hyper_fa: f32,
    pub hyper_fb: f32,
}

impl Default for CharacterLightEntry {
    fn default() -> Self {
        Self {
            kind: CharacterLightType::None,
            fake_kind: FakeLightKind::Real,
            abs_intensity: 0.0,
            influence: 0.0,
            signed_intensity: 0.0,
            signed_influence: 0.0,
            atten_factor: 0.0,
            world_pos: Vec3::ZERO,
            world_dir: Vec3::Z, // 0x2a94d0 = [0, 0, 1, 0]
            color: Vec3::ZERO,
            falloff_near_mm: 0.0,
            falloff_far_mm: 0.0,
            spread_inner: 1.0,
            spread_outer: -1.0,
            linear_fa: 0.0,
            linear_fb: 1.0,
            spread_sa: 1.0,
            spread_sb: 0.0,
            hyper_fa: 0.0,
            hyper_fb: 1.0,
        }
    }
}

impl CharacterLightEntry {
    /// Updates intensity from color (`UpdateIntensity` `0x00126120`).
    pub fn update_intensity(&mut self) {
        let luma = self.color.x * PS2_LUMA_WEIGHTS[0]
            + self.color.y * PS2_LUMA_WEIGHTS[1]
            + self.color.z * PS2_LUMA_WEIGHTS[2];
        self.signed_intensity = luma;
        self.abs_intensity = luma.abs();
    }

    /// Updates linear and hyperbolic distance falloff (`UpdateFParam` `0x00126170`).
    pub fn update_f_param(&mut self) {
        let [l_fa, l_fb, h_fa, h_fb] =
            compute_ps2_falloff_params(self.falloff_near_mm, self.falloff_far_mm);
        self.linear_fa = l_fa;
        self.linear_fb = l_fb;
        self.hyper_fa = h_fa;
        self.hyper_fb = h_fb;
    }

    /// Updates angular cone spread coefficients (`UpdateSParam` `0x001261d0`).
    pub fn update_s_param(&mut self) {
        let denom = self.spread_inner - self.spread_outer;
        if denom.abs() < 1e-6 {
            self.spread_sa = 0.0;
            self.spread_sb = 1.0;
        } else {
            self.spread_sa = 1.0 / denom;
            self.spread_sb = -(self.spread_sa * self.spread_outer);
        }
    }

    /// Evaluates `CalcInfluence` (`0x001269b0`) for a character centered at `char_pos` with bounding `radius_mm`.
    pub fn calc_influence(&mut self, radius_mm: f32, char_pos: Vec3) {
        match self.kind {
            CharacterLightType::Point | CharacterLightType::Spot => {
                let delta = char_pos - self.world_pos;
                let dist = delta.length();
                let dist_atten =
                    (self.linear_fb + self.linear_fa * (dist - radius_mm)).clamp(0.0, 1.0);
                let mut total_atten = dist_atten;

                if self.kind == CharacterLightType::Spot {
                    let dir = delta.normalize_or_zero();
                    let cos_a = dir.dot(self.world_dir);
                    let ang = (self.spread_sb + self.spread_sa * cos_a).clamp(0.0, 1.0);
                    total_atten *= ang;
                }
                self.atten_factor = total_atten;
                self.influence = self.abs_intensity * dist_atten;
                self.signed_influence = self.signed_intensity * dist_atten;
            }
            CharacterLightType::Parallel => {
                self.atten_factor = self.color.y * 0.2;
                self.influence = self.abs_intensity;
                self.signed_influence = self.signed_intensity;
            }
            CharacterLightType::Reflection => {
                self.influence = self.abs_intensity;
                self.signed_influence = self.signed_intensity;
            }
            CharacterLightType::None => {
                self.influence = 0.0;
                self.signed_influence = 0.0;
            }
        }
    }
}

/// 12-slot character lighting manager (`Chacter_Draw/light_n.c`).
#[derive(Debug, Clone)]
pub struct CharacterLightPool {
    slots: [CharacterLightEntry; CHARACTER_LIGHT_SLOT_COUNT],
    /// Selected top-6 point/spot extra light slot indices (`DAT_00419c90`, `UpdateExtras` `0x00127360`).
    valid_extras: [usize; CHARACTER_EXTRA_LIGHT_LIMIT],
    valid_extra_count: usize,
    /// Selected reflection highlight brightness (`_DAT_00419cc8`, `LightReflectionBrightness` `0x00127770`).
    reflection_brightness: f32,
    /// Selected reflection highlight color (`0x419cd0`, `LightGetReflectionColor` `0x00127780`).
    reflection_color: Vec3,
    /// Computed Normal-to-Halfway Matrix (`0x419d20`, `LightGetNthViewNHM` `0x001277c0`).
    nhm: Mat4,
}

impl Default for CharacterLightPool {
    fn default() -> Self {
        Self::new()
    }
}

impl CharacterLightPool {
    /// Creates an empty 12-slot character light pool (`LightDeleteAll` `0x00126240`).
    #[must_use]
    pub fn new() -> Self {
        Self {
            slots: [CharacterLightEntry::default(); CHARACTER_LIGHT_SLOT_COUNT],
            valid_extras: [0; CHARACTER_EXTRA_LIGHT_LIMIT],
            valid_extra_count: 0,
            reflection_brightness: 0.0,
            reflection_color: Vec3::ZERO,
            nhm: Mat4::IDENTITY,
        }
    }

    /// Returns a reference to slot `index` (`LightPointer` `0x001260b0`, `0..12`).
    #[must_use]
    pub fn slot(&self, index: usize) -> Option<&CharacterLightEntry> {
        self.slots.get(index)
    }

    /// Clears a single slot (`LightDelete` `0x00126210`).
    pub fn delete(&mut self, index: usize) {
        if let Some(s) = self.slots.get_mut(index) {
            s.kind = CharacterLightType::None;
        }
    }

    /// Clears all 12 slots (`LightDeleteAll` `0x00126240`).
    pub fn delete_all(&mut self) {
        for i in 0..CHARACTER_LIGHT_SLOT_COUNT {
            self.delete(i);
        }
    }

    /// Configures a parallel light (`LightSetParallel` `0x00126290`).
    pub fn set_parallel(&mut self, index: usize, dir: Vec3, color: Vec3) {
        if let Some(s) = self.slots.get_mut(index) {
            *s = CharacterLightEntry {
                kind: CharacterLightType::Parallel,
                world_dir: if dir.length_squared() == 0.0 {
                    Vec3::Z
                } else {
                    dir.normalize()
                },
                color,
                ..Default::default()
            };
            s.update_intensity();
        }
    }

    /// Configures a point light (`LightSetPoint` `0x00126320`).
    pub fn set_point(&mut self, index: usize, pos: Vec3, color: Vec3, near_mm: f32, far_mm: f32) {
        if let Some(s) = self.slots.get_mut(index) {
            *s = CharacterLightEntry {
                kind: CharacterLightType::Point,
                world_pos: pos,
                world_dir: Vec3::Z,
                color,
                falloff_near_mm: near_mm,
                falloff_far_mm: far_mm,
                ..Default::default()
            };
            s.update_intensity();
            s.update_f_param();
        }
    }

    /// Configures a spotlight (`sh_Kari_LightSetSpot` `0x001265f0`).
    ///
    /// `falloff_mm` is `(near_mm, far_mm)`.
    pub fn set_spot(
        &mut self,
        index: usize,
        pos: Vec3,
        dir: Vec3,
        color: Vec3,
        outer_cos: f32,
        falloff_mm: (f32, f32),
    ) {
        if let Some(s) = self.slots.get_mut(index) {
            *s = CharacterLightEntry {
                kind: CharacterLightType::Spot,
                world_pos: pos,
                world_dir: if dir.length_squared() == 0.0 {
                    Vec3::Z
                } else {
                    dir.normalize()
                },
                color,
                falloff_near_mm: falloff_mm.0,
                falloff_far_mm: falloff_mm.1,
                spread_inner: 1.0,
                spread_outer: outer_cos,
                spread_sa: 1.0,
                spread_sb: -outer_cos * KARI_SPOT_SPREAD_BIAS_SCALE,
                ..Default::default()
            };
            s.update_intensity();
            s.update_f_param();
        }
    }

    /// Configures a reflection highlight (`LightSetReflection` `0x00126740`).
    pub fn set_reflection(&mut self, index: usize, dir: Vec3, color: Vec3) {
        if let Some(s) = self.slots.get_mut(index) {
            *s = CharacterLightEntry {
                kind: CharacterLightType::Reflection,
                world_dir: if dir.length_squared() == 0.0 {
                    Vec3::Z
                } else {
                    dir.normalize()
                },
                color,
                ..Default::default()
            };
            s.update_intensity();
        }
    }

    /// Selects the strongest reflection light and builds the Normal-to-Halfway Matrix
    /// (`UpdateReflections` `0x00126f90`).
    pub fn update_reflections(&mut self, view_dir: Vec3, camera_up: Vec3) {
        self.reflection_brightness = 0.0;
        self.reflection_color = Vec3::ZERO;
        self.nhm = Mat4::IDENTITY;

        let mut refl_indices: Vec<usize> = self
            .slots
            .iter()
            .enumerate()
            .filter(|(_, s)| s.kind == CharacterLightType::Reflection)
            .map(|(i, _)| i)
            .collect();

        if refl_indices.is_empty() {
            return;
        }

        // `CompareByIntensity` (`0x00126ba0`): descending by `abs_intensity`
        refl_indices.sort_by(|&a, &b| {
            self.slots[b]
                .abs_intensity
                .total_cmp(&self.slots[a].abs_intensity)
        });

        let best = self.slots[refl_indices[0]];
        let half = (best.world_dir + view_dir).normalize_or(Vec3::X);
        let u_axis = camera_up.cross(half).normalize_or(Vec3::X);
        let v_axis = half.cross(u_axis).normalize_or(Vec3::Y);

        // `UpdateReflections` (`0x00126f90`): columns U, V, H transposed + row 3 = [0.5, 0.5, 0.0, 1.0] (`0x2a9550`)
        self.nhm = Mat4::from_cols(
            Vec4::new(u_axis.x, v_axis.x, half.x, PS2_NHM_UV_BIAS_ROW[0]),
            Vec4::new(u_axis.y, v_axis.y, half.y, PS2_NHM_UV_BIAS_ROW[1]),
            Vec4::new(u_axis.z, v_axis.z, half.z, PS2_NHM_UV_BIAS_ROW[2]),
            Vec4::new(0.0, 0.0, 0.0, PS2_NHM_UV_BIAS_ROW[3]),
        );
        self.reflection_brightness = best.signed_intensity;
        self.reflection_color = best.color;
    }

    /// Evaluates `CalcInfluence` for all Point/Spot slots and selects the top 6 descending by influence
    /// (`UpdateExtras` `0x00127360`, `CompareByInfluence` `0x00126c00`).
    pub fn update_extras(&mut self, radius_mm: f32, char_pos: Vec3) {
        let mut active: Vec<usize> = Vec::with_capacity(CHARACTER_LIGHT_SLOT_COUNT);
        for (i, slot) in self.slots.iter_mut().enumerate() {
            if matches!(
                slot.kind,
                CharacterLightType::Point | CharacterLightType::Spot
            ) {
                slot.calc_influence(radius_mm, char_pos);
                if slot.influence > 0.0 {
                    active.push(i);
                }
            }
        }

        // `CompareByInfluence` (`0x00126c00`): descending by `influence`
        active.sort_by(|&a, &b| self.slots[b].influence.total_cmp(&self.slots[a].influence));

        let count = active.len().min(CHARACTER_EXTRA_LIGHT_LIMIT);
        self.valid_extra_count = count;
        for (dst, &src) in self.valid_extras.iter_mut().zip(active.iter()).take(count) {
            *dst = src;
        }
    }

    /// Number of valid extra (Point/Spot) lights selected (`LightNValidExtras` `0x001277f0`, `<= 6`).
    #[must_use]
    pub fn valid_extra_count(&self) -> usize {
        self.valid_extra_count
    }

    /// Returns the `n`-th strongest valid extra light (`LightNthValidExtra` `0x00127800`).
    #[must_use]
    pub fn nth_valid_extra(&self, n: usize) -> Option<&CharacterLightEntry> {
        if n < self.valid_extra_count {
            Some(&self.slots[self.valid_extras[n]])
        } else {
            None
        }
    }

    /// Returns the active reflection brightness (`LightReflectionBrightness` `0x00127770`).
    #[must_use]
    pub fn reflection_brightness(&self) -> f32 {
        self.reflection_brightness
    }

    /// Returns the active reflection color (`LightGetReflectionColor` `0x00127780`).
    #[must_use]
    pub fn reflection_color(&self) -> Vec3 {
        self.reflection_color
    }

    /// Returns the computed Normal-to-Halfway Matrix (`LightGetNthViewNHM` `0x001277c0`).
    #[must_use]
    pub fn nhm(&self) -> Mat4 {
        self.nhm
    }
}
