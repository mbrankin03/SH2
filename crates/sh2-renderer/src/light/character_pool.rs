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

/// NHM column 3 UV translation bias vector from `SLUS_202.28` `0x002a9550`, copied into
/// `0x419d10` / `0x419d50` (column 3) by `UpdateReflections` (`0x00126f90`).
pub const PS2_NHM_UV_BIAS_COL: [f32; 4] = [0.5, 0.5, 0.0, 1.0];

/// Maximum number of VU parallel light matrices populated by `UpdateParallels` (`0x00126d30`).
///
/// Matrix 0 holds up to 3 parallel lights (`iVar6 == 3 && iVar5 == 0`), Matrix 1 holds up to 4
/// (`iVar6 == 4`), for a total of up to 7 parallel lights across 2 matrices.
pub const CHARACTER_PARALLEL_MATRIX_LIMIT: usize = 2;

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
    /// Packed parallel Normal-Light Matrices (`0x419b10` / `0x419b90`, `UpdateParallels` `0x00126d30`).
    parallel_nlm: [Mat4; CHARACTER_PARALLEL_MATRIX_LIMIT],
    /// Packed parallel Light-Color Matrices (`0x419c10`, `UpdateParallels` `0x00126d30`).
    parallel_lcm: [Mat4; CHARACTER_PARALLEL_MATRIX_LIMIT],
    /// Total active parallel lights packed (`_DAT_00419b00`, up to 7 across 2 matrices).
    valid_parallel_count: usize,
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
            parallel_nlm: [Mat4::ZERO; CHARACTER_PARALLEL_MATRIX_LIMIT],
            parallel_lcm: [Mat4::ZERO; CHARACTER_PARALLEL_MATRIX_LIMIT],
            valid_parallel_count: 0,
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

    /// Packs active parallel lights into up to 2 VU matrices (3 in matrix 0 + 4 in matrix 1)
    /// matching `UpdateParallels` (`Chacter_Draw/light_n.c` `0x00126d30`).
    pub fn update_parallels(&mut self) {
        self.parallel_nlm = [Mat4::ZERO; CHARACTER_PARALLEL_MATRIX_LIMIT];
        self.parallel_lcm = [Mat4::ZERO; CHARACTER_PARALLEL_MATRIX_LIMIT];
        self.valid_parallel_count = 0;

        let mut mat_idx: usize = 0;
        let mut slot_in_mat: usize = 0;

        for slot in &self.slots {
            if slot.kind == CharacterLightType::Parallel
                && mat_idx < CHARACTER_PARALLEL_MATRIX_LIMIT
            {
                let mut nlm_cols = self.parallel_nlm[mat_idx].to_cols_array_2d();
                // `UpdateParallels` (`0x00126d30` lines 539-545): direction stored in rows 0..3 at column `slot_in_mat`
                nlm_cols[0][slot_in_mat] = slot.world_dir.x;
                nlm_cols[1][slot_in_mat] = slot.world_dir.y;
                nlm_cols[2][slot_in_mat] = slot.world_dir.z;
                self.parallel_nlm[mat_idx] = Mat4::from_cols_array_2d(&nlm_cols);

                // `UpdateParallels` (`0x00126d30` lines 546-550): color stored as column `slot_in_mat` of LCM
                let mut lcm_cols = self.parallel_lcm[mat_idx].to_cols_array_2d();
                lcm_cols[slot_in_mat] = [slot.color.x, slot.color.y, slot.color.z, 0.0];
                self.parallel_lcm[mat_idx] = Mat4::from_cols_array_2d(&lcm_cols);

                slot_in_mat += 1;
                if (slot_in_mat == 3 && mat_idx == 0) || slot_in_mat == 4 {
                    mat_idx += 1;
                    slot_in_mat = 0;
                }
                self.valid_parallel_count += 1;
            }
        }
    }

    /// Number of valid parallel light matrices (`LightNValidParallelMatrices` `0x00127660`, `(n + 3) >> 2`).
    #[must_use]
    pub fn valid_parallel_matrix_count(&self) -> usize {
        (self.valid_parallel_count + 3) >> 2
    }

    /// Number of active parallel lights packed by [`Self::update_parallels`] (`_DAT_00419b00`).
    #[must_use]
    pub fn valid_parallel_count(&self) -> usize {
        self.valid_parallel_count
    }

    /// Returns the `n`-th Normal-Light Matrix (`LightGetNthViewNLM` `0x00127690`, `n < 2`).
    #[must_use]
    pub fn nth_parallel_nlm(&self, n: usize) -> Option<Mat4> {
        self.parallel_nlm.get(n).copied()
    }

    /// Returns the `n`-th Light-Color Matrix (`LightGetNthLCM` `0x00127700`, `n < 2`).
    #[must_use]
    pub fn nth_parallel_lcm(&self, n: usize) -> Option<Mat4> {
        self.parallel_lcm.get(n).copied()
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

        // `UpdateReflections` (`0x00126f90` lines 623-630): transposes `[U, V, H, (0,0,0,1)]` so
        // rows 0..3 hold U, V, H, then copies `[0.5, 0.5, 0.0, 1.0]` (`0x002a9550`) into column 3
        // (`0x419d10` / `0x419d50`), yielding `nhm * Vec4(n, 1.0) = (dot(u,n)+0.5, dot(v,n)+0.5, dot(h,n), 1.0)`.
        self.nhm = Mat4::from_cols(
            Vec4::new(u_axis.x, v_axis.x, half.x, 0.0),
            Vec4::new(u_axis.y, v_axis.y, half.y, 0.0),
            Vec4::new(u_axis.z, v_axis.z, half.z, 0.0),
            Vec4::from_array(PS2_NHM_UV_BIAS_COL),
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
