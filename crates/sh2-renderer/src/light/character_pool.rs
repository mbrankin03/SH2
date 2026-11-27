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

/// Default character bounding radius in PS2 mm passed by `Model3UpdateLightEnv` (`SLUS_202.28`
/// `0x00128e00`) to `LightUpdateInfoByPos` (`0x00127640`): `radius$946` at `0x002a9700`
/// (`0x44480000` = `800.0f` mm).
pub const MODEL3_LIGHT_ENV_RADIUS_MM: f32 = 800.0;

/// Nighttime reflection highlight dimming factor (`0x3e4ccccd = 0.2f`) applied by `UpdateExtras`
/// (`SLUS_202.28` `Chacter_Draw/light_n.c` `0x00127360` lines 681-689) when no extra point/spot
/// lights influence a character during non-cutscene night gameplay (`sh2gfw_Get_NightOrDay() != 0
/// && DramaDemoNumber() == 0`).
pub const UPDATE_EXTRAS_NIGHT_REFLECTION_SCALE: f32 = 0.2;

/// Minimum fake-light attenuation threshold (`1e-5f`) below which `UpdateFake_DirandColor`
/// (`SLUS_202.28` `Chacter_Draw/sh2gfw_model_light.c` `0x0012f450` line 171) deactivates a fake light.
pub const FAKE_LIGHT_MIN_ATTENUATION: f32 = 1e-5;

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
    /// Unattenuated base RGB color for `FakePoint`/`FakeSpot` lights (`*(Vec4*)puVar4[7]` in
    /// `UpdateFake_DirandColor` `0x0012f450`, `sh2gfw_model_light.c:156-166`).
    pub fake_base_color: Vec3,
    /// Spotlight axis direction for `FakeSpot` lights (`puVar1 + 0xc` in `UpdateFake_DirandColor`
    /// `0x0012f450`, `sh2gfw_model_light.c:144`).
    pub fake_spot_dir: Vec3,
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
            fake_base_color: Vec3::ZERO,
            fake_spot_dir: Vec3::Z,
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
    /// Base reflection color computed by `UpdateReflections` (`0x00126f90`) prior to per-character
    /// `UpdateExtras` (`0x00127360`) night dimming (`0.2`) or highlight restoration (`1.0`).
    base_reflection_color: Vec3,
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
            base_reflection_color: Vec3::ZERO,
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

    /// Configures a fake point light (`LightSetFakePoint` `0x001263f0`, `Chacter_Draw/light_n.c:180-197`).
    ///
    /// Stored as `kind = Parallel` with `fake_kind = FakePoint (1)` and evaluated per-character by
    /// [`Self::update_fake_lights`] (`UpdateFake_DirandColor` `0x0012f450`).
    pub fn set_fake_point(
        &mut self,
        index: usize,
        pos: Vec3,
        color: Vec3,
        near_mm: f32,
        far_mm: f32,
    ) {
        if let Some(s) = self.slots.get_mut(index) {
            *s = CharacterLightEntry {
                kind: CharacterLightType::Parallel,
                fake_kind: FakeLightKind::FakePoint,
                world_pos: pos,
                world_dir: Vec3::Z,
                color,
                fake_base_color: color,
                falloff_near_mm: near_mm,
                falloff_far_mm: far_mm,
                ..Default::default()
            };
            s.update_intensity();
            s.update_f_param();
        }
    }

    /// Configures a fake spotlight (`LightSetFakeSpot` `0x001264c0`, `Chacter_Draw/light_n.c:200-218`).
    ///
    /// Stored as `kind = Parallel` with `fake_kind = FakeSpot (2)` and evaluated per-character by
    /// [`Self::update_fake_lights`] (`UpdateFake_DirandColor` `0x0012f450`).
    #[allow(clippy::too_many_arguments)]
    pub fn set_fake_spot(
        &mut self,
        index: usize,
        pos: Vec3,
        spot_dir: Vec3,
        color: Vec3,
        near_mm: f32,
        far_mm: f32,
        spread_inner: f32,
        spread_outer: f32,
    ) {
        if let Some(s) = self.slots.get_mut(index) {
            *s = CharacterLightEntry {
                kind: CharacterLightType::Parallel,
                fake_kind: FakeLightKind::FakeSpot,
                world_pos: pos,
                world_dir: Vec3::Z,
                color,
                fake_base_color: color,
                fake_spot_dir: if spot_dir.length_squared() == 0.0 {
                    Vec3::Z
                } else {
                    spot_dir.normalize()
                },
                falloff_near_mm: near_mm,
                falloff_far_mm: far_mm,
                spread_inner,
                spread_outer,
                ..Default::default()
            };
            s.update_intensity();
            s.update_f_param();
            s.update_s_param();
        }
    }

    /// Evaluates all `FakePoint`/`FakeSpot` light slots for a character at `char_pos` matching
    /// `SLUS_202.28` `UpdateFake_DirandColor` (`Chacter_Draw/sh2gfw_model_light.c` `0x0012f450`,
    /// lines 80-187).
    ///
    /// Converts each fake light into an attenuated directional (`Parallel`) light pointing from
    /// `world_pos` to `char_pos`, or disables it (`CharacterLightType::None`) when attenuation
    /// `<= 1e-5` (`FAKE_LIGHT_MIN_ATTENUATION`).
    pub fn update_fake_lights(&mut self, char_pos: Vec3) {
        for slot in &mut self.slots {
            if slot.fake_kind == FakeLightKind::Real {
                continue;
            }
            let delta = char_pos - slot.world_pos;
            slot.world_dir = delta.normalize_or(Vec3::Z);
            let dist = delta.length();
            let mut atten = (slot.linear_fb + slot.linear_fa * dist).clamp(0.0, 1.0);

            if slot.fake_kind == FakeLightKind::FakeSpot {
                // `UpdateFake_DirandColor` (`0x0012f450` lines 124-155):
                // Project onto XZ if `dy^2 <= dx^2 || dy^2 <= dz^2`, else onto XY.
                let sq = delta * delta;
                let vertical_dominant = sq.y > sq.x && sq.y > sq.z;
                let proj_delta = if vertical_dominant {
                    Vec3::new(delta.x, delta.y, 0.0).normalize_or_zero()
                } else {
                    Vec3::new(delta.x, 0.0, delta.z).normalize_or_zero()
                };
                let proj_spot = if vertical_dominant {
                    Vec3::new(slot.fake_spot_dir.x, slot.fake_spot_dir.y, 0.0).normalize_or_zero()
                } else {
                    Vec3::new(slot.fake_spot_dir.x, 0.0, slot.fake_spot_dir.z).normalize_or_zero()
                };
                let cos_a = proj_delta.dot(proj_spot);
                let ang = (slot.spread_sb + slot.spread_sa * cos_a).clamp(0.0, 1.0);
                atten *= ang;
            }

            if atten <= FAKE_LIGHT_MIN_ATTENUATION {
                slot.kind = CharacterLightType::None;
                slot.color = Vec3::ZERO;
            } else {
                slot.kind = CharacterLightType::Parallel;
                slot.color = slot.fake_base_color * atten;
                slot.update_intensity();
            }
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
        self.base_reflection_color = best.color;
    }

    /// Evaluates `CalcInfluence` for all Point/Spot slots and selects the top 6 descending by influence
    /// (`UpdateExtras` `0x00127360`, `CompareByInfluence` `0x00126c00`).
    pub fn update_extras(&mut self, radius_mm: f32, char_pos: Vec3) {
        self.update_extras_with_context(radius_mm, char_pos, false, false, false);
    }

    /// Full `SLUS_202.28` `UpdateExtras` (`Chacter_Draw/light_n.c` `0x00127360` lines 652-724):
    /// - Evaluates `CalcInfluence` for all Point/Spot slots and selects up to 6 descending by influence.
    /// - When `valid_extra_count == 0`, `is_night` (`sh2gfw_Get_NightOrDay() != 0`), and `!is_cutscene`
    ///   (`DramaDemoNumber() == 0`), dims `reflection_color` to `[1, 1, 1] * 0.2` (`0x3e4ccccd`,
    ///   `SLUS_202.28` `0x002a9560`).
    /// - When `valid_extra_count > 0`, `demo_refl_highlight` (`sh2gfw_Check_DemoRefrectionHightLight()`),
    ///   and `!is_cutscene`, restores `reflection_color` to `[1, 1, 1] * 1.0` (`0x3f800000`,
    ///   `SLUS_202.28` `0x002a9570`).
    pub fn update_extras_with_context(
        &mut self,
        radius_mm: f32,
        char_pos: Vec3,
        is_night: bool,
        is_cutscene: bool,
        demo_refl_highlight: bool,
    ) {
        self.reflection_color = self.base_reflection_color;
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

        if active.is_empty() {
            self.valid_extra_count = 0;
            if is_night && !is_cutscene {
                // `UpdateExtras` (`0x00127360` lines 684-688): `_1181` (`0x2a9560` = `[1,1,1,1]`) * `0.2f`
                self.reflection_color = Vec3::splat(UPDATE_EXTRAS_NIGHT_REFLECTION_SCALE);
            }
            return;
        }

        // `CompareByInfluence` (`0x00126c00`): descending by `influence`
        active.sort_by(|&a, &b| self.slots[b].influence.total_cmp(&self.slots[a].influence));

        let count = active.len().min(CHARACTER_EXTRA_LIGHT_LIMIT);
        self.valid_extra_count = count;
        for (dst, &src) in self.valid_extras.iter_mut().zip(active.iter()).take(count) {
            *dst = src;
        }

        if demo_refl_highlight && !is_cutscene {
            // `UpdateExtras` (`0x00127360` lines 716-720): `_1204` (`0x2a9570` = `[1,1,1,1]`) * `1.0f`
            self.reflection_color = Vec3::ONE;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_extras_night_reflection_dimming_and_fake_lights() {
        let mut pool = CharacterLightPool::new();
        pool.set_reflection(0, Vec3::Y, Vec3::ONE);
        pool.update_reflections(Vec3::ZERO, Vec3::Z);
        // With no active Point/Spot extras at night in gameplay, `UpdateExtras` (`0x00127360`)
        // dims reflection_color to `0.2` (`0x3e4ccccd`).
        pool.update_extras_with_context(1000.0, Vec3::ZERO, true, false, false);
        assert_eq!(pool.valid_extra_count(), 0);
        assert!((pool.reflection_color().x - UPDATE_EXTRAS_NIGHT_REFLECTION_SCALE).abs() < 1e-6);

        // `UpdateFake_DirandColor` (`0x0012f450`): fake point light attenuates and converts to Parallel.
        pool.set_fake_point(
            1,
            Vec3::new(0.0, 0.0, 2000.0),
            Vec3::new(1.0, 0.8, 0.6),
            500.0,
            4500.0,
        );
        pool.update_fake_lights(Vec3::ZERO);
        assert_eq!(pool.slots[1].kind, CharacterLightType::Parallel);
        assert!(pool.slots[1].color.x > 0.0 && pool.slots[1].color.x < 1.0);
        assert!((pool.slots[1].world_dir - (-Vec3::Z)).length() < 1e-5);

        // Fake spot outside cone (`spread_outer = 0.7`, perpendicular offset) attenuates to `1e-5`.
        pool.set_fake_spot(
            2,
            Vec3::new(1000.0, 0.0, 0.0),
            Vec3::Z,
            Vec3::ONE,
            100.0,
            5000.0,
            0.9,
            0.7,
        );
        pool.update_fake_lights(Vec3::ZERO);
        assert!(pool.slots[2].color.x <= FAKE_LIGHT_MIN_ATTENUATION + 1e-6);
    }
}
