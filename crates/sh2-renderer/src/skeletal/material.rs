//! Per-primitive PS2/PC character material conversion, dynamic uniform packing, and
//! `Model3` equipment/pose variant discovery (`SH2-37`).
//!
//! Ground-truth references:
//! - PS2 `SLUS_202.28`: `DrawPart1` (`0x0012d750`), `Model3WorkEquipmentFlag` (`0x00128030`),
//!   `DemoJamesCheckModelParts` (`0x001044d0`), `PlayerCheckModelParts` (`0x001098d0`).
//! - Retail PC `sh2pc.exe`: `ActorOpaqueDraw` (`0x00501540`, `1.0 / 128.0` specular scale at
//!   `0x00631624`, `D3DTOP_MODULATE2X` stage 0, `D3DTOP_MODULATEALPHA_ADDCOLOR` stage 1).

use bytemuck::{Pod, Zeroable};
use sh2_assets::formats::mdls::{ModelFile, ModelPrimitive};

/// PS2 GS fixed-point specular color normalization factor (`1.0 / 128.0 = 0.0078125`),
/// matching `sh2pc.exe` `ActorOpaqueDraw` (`0x00501a50`, `.rdata` `0x00631624`).
pub const PS2_SPECULAR_COLOR_SCALE: f32 = 1.0 / 128.0;

/// Upper bound on normalized per-channel specular intensity (`384/128 = 3.0` on Maria/Mary eyes
/// clamped to `1.35` so wet cornea catchlights stay crisp without eyelid over-brightening).
pub const MAX_NORMALIZED_SPECULAR_COMPONENT: f32 = 1.35;

/// Conversion factor from `.mdl` `specular_scale` (`h + 48`, typically `1.67..5.00`) to
/// per-fragment Blinn-Phong shininess exponent (`[24.0, 96.0]`).
pub const SHININESS_PER_SPECULAR_SCALE: f32 = 18.0;

/// Minimum Blinn-Phong shininess exponent for `material_type == 4` specular primitives.
pub const MIN_SPECULAR_SHININESS: f32 = 24.0;

/// Maximum Blinn-Phong shininess exponent for `material_type == 4` specular primitives.
pub const MAX_SPECULAR_SHININESS: f32 = 96.0;

/// PS2 GS `TEST_1` register (`0x47`) for character alpha-tested primitives
/// (`SLUS_202.28` `0x002a9780` referenced by `DrawPart1` `0x0012d750` in `Chacter_Draw/model3_vu1_n.c`):
/// `ATE=1`, `ATST=6 (GREATER)`, `AREF=0x00`, `AFAIL=0 (KEEP)`, `ZTE=1`, `ZTST=2 (GEQUAL)`.
pub const PS2_GS_TEST_ALPHA_GREATER_ZERO: u64 = 0x0005_000d;

/// PS2 GS `TEST_1` register (`0x47`) for opaque world/character geometry without alpha-test
/// (`SLUS_202.28` `SetupDrawEnvData` `0x00251370` line 408: `ATE=0, ZTE=1, ZTST=1`).
pub const PS2_GS_TEST_OPAQUE_NO_ATEST: u64 = 0x0003_0002;

/// Retail PC `sh2pc.exe` `FUN_00503800` (`0x00503800` lines 2406-2409) `D3DRS_ALPHAFUNC` value
/// (`D3DCMP_GREATER = 5` in Direct3D 8) paired with `D3DRS_ALPHAREF = 0` (`alpha > 0`).
pub const PC_D3DRS_ALPHAFUNC_GREATER: u32 = 5;

/// Retail PC `sh2pc.exe` `FUN_00503800` (`0x00503800` line 2409) `D3DRS_ALPHAREF` value (`0`).
pub const PC_D3DRS_ALPHAREF_ZERO: u8 = 0;

/// Exact normalized 8-bit UNORM midpoint threshold (`0.5 / 255.0`) equivalent to PS2 GS
/// `TEST_1 = 0x5000d` (`ATST = 6 (GREATER), AREF = 0`) and PC `D3DRS_ALPHAFUNC = D3DCMP_GREATER (5)`,
/// `D3DRS_ALPHAREF = 0` (`alpha_u8 > 0`).
///
/// Discards `alpha_u8 == 0` (`0.0`) and passes all `alpha_u8 >= 1` (`>= 1.0 / 255.0`) even under
/// float rounding, replacing the uncited `0.90` / `0.05` alpha-test thresholds.
pub const PS2_ALPHA_TEST_THRESHOLD: f32 = 0.5 / 255.0;

/// Exact normalized 8-bit UNORM midpoint threshold (`254.5 / 255.0`) separating fully opaque
/// texels (`alpha_u8 == 255`, depth-writing core `pass_mode == 1u`) from semi-transparent texels
/// (`1 <= alpha_u8 <= 254`, depth-read-only alpha-blended fringe `pass_mode == 2u`) while keeping
/// every `alpha_u8 > 0` texel (`TEST_1 = 0x5000d`, `SLUS_202.28` `0x002a9780`) visible without
/// double-blending.
pub const PS2_ALPHA_OPAQUE_THRESHOLD: f32 = 254.5 / 255.0;

/// Decoded PS2 GS `TEST_1` (`0x47`) alpha-test fields (`ATE`, `ATST`, `AREF`, `AFAIL`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GsAlphaTest {
    /// Bit 0 (`ATE`): alpha test enable.
    pub enabled: bool,
    /// Bits 1..=3 (`ATST`): comparison function (`0=NEVER, 1=ALWAYS, 2=LESS, 3=LEQUAL, 4=EQUAL,
    /// 5=GEQUAL, 6=GREATER, 7=NOTEQUAL`).
    pub atst: u8,
    /// Bits 4..=11 (`AREF`): 8-bit alpha reference value (`0..=255`).
    pub aref: u8,
    /// Bits 12..=13 (`AFAIL`): fail behavior (`0=KEEP`, `1=FB_ONLY`, `2=ZB_ONLY`, `3=RGB_ONLY`).
    pub afail: u8,
}

impl GsAlphaTest {
    /// Decodes the alpha-test fields from a 64-bit PS2 GS `TEST_1` (`0x47`) register value.
    #[must_use]
    pub const fn from_gs_test_1(test_1: u64) -> Self {
        Self {
            enabled: (test_1 & 1) != 0,
            atst: ((test_1 >> 1) & 0x7) as u8,
            aref: ((test_1 >> 4) & 0xff) as u8,
            afail: ((test_1 >> 12) & 0x3) as u8,
        }
    }

    /// Evaluates whether a fragment with 8-bit alpha `alpha_u8` passes this GS `TEST_1` alpha test.
    #[must_use]
    pub const fn passes(self, alpha_u8: u8) -> bool {
        if !self.enabled {
            return true;
        }
        match self.atst {
            0 => false,                 // NEVER
            1 => true,                  // ALWAYS
            2 => alpha_u8 < self.aref,  // LESS
            3 => alpha_u8 <= self.aref, // LEQUAL
            4 => alpha_u8 == self.aref, // EQUAL
            5 => alpha_u8 >= self.aref, // GEQUAL
            6 => alpha_u8 > self.aref,  // GREATER
            _ => alpha_u8 != self.aref, // NOTEQUAL (7)
        }
    }
}

/// GPU-aligned per-primitive material uniform block (64 bytes, 16-byte `vec4<f32>` aligned).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct SkeletalMaterialUniforms {
    /// `xyz`: per-primitive diffuse RGB reflection (`ModelPrimitive::diffuse`),
    /// `w`: `ModelPrimitive::material_type` (`1` = unlit, `2`/`3` = Lambertian, `4` = specular).
    pub diffuse_mtype: [f32; 4],
    /// `xyz`: per-primitive ambient RGB reflection (`ModelPrimitive::ambient`),
    /// `w`: Blinn-Phong shininess exponent derived from `ModelPrimitive::specular_scale`.
    pub ambient_shininess: [f32; 4],
    /// `xyz`: normalized specular RGB (`ModelPrimitive::specular * (1.0 / 128.0)`),
    /// `w`: backface culling flag (`1.0` = single-sided culled, `0.0` = double-sided card).
    pub specular_cull: [f32; 4],
    /// Padding to 64 bytes for clean `vec4<f32>` uniform layout alignment.
    pub _reserved: [f32; 4],
}

impl Default for SkeletalMaterialUniforms {
    fn default() -> Self {
        Self {
            diffuse_mtype: [0.70, 0.70, 0.70, 2.0],
            ambient_shininess: [0.30, 0.30, 0.30, MIN_SPECULAR_SHININESS],
            specular_cull: [0.0, 0.0, 0.0, 1.0],
            _reserved: [0.0; 4],
        }
    }
}

impl SkeletalMaterialUniforms {
    /// Converts a parsed [`ModelPrimitive`] material header into GPU uniform values.
    ///
    /// - Scales `specular` by `1.0 / 128.0` (`PS2_SPECULAR_COLOR_SCALE`) when `material_type == 4`
    ///   (and zeroes specular for `material_type != 4`, matching `SLUS_202.28` `DrawPart1`
    ///   `0x0012da24` and `sh2pc.exe` `ActorOpaqueDraw` `0x00501a47`).
    /// - Maps `specular_scale` (`1.67..5.00`) to a Blinn-Phong exponent in `[24.0, 96.0]`.
    pub fn from_primitive(prim: &ModelPrimitive) -> Self {
        let is_specular = prim.material_type == 4;
        let shininess =
            if is_specular && prim.specular_scale.is_finite() && prim.specular_scale > 0.0 {
                (prim.specular_scale * SHININESS_PER_SPECULAR_SCALE)
                    .clamp(MIN_SPECULAR_SHININESS, MAX_SPECULAR_SHININESS)
            } else {
                MIN_SPECULAR_SHININESS
            };

        let spec_rgb = if is_specular {
            [
                sanitize_channel(prim.specular[0] * PS2_SPECULAR_COLOR_SCALE)
                    .clamp(0.0, MAX_NORMALIZED_SPECULAR_COMPONENT),
                sanitize_channel(prim.specular[1] * PS2_SPECULAR_COLOR_SCALE)
                    .clamp(0.0, MAX_NORMALIZED_SPECULAR_COMPONENT),
                sanitize_channel(prim.specular[2] * PS2_SPECULAR_COLOR_SCALE)
                    .clamp(0.0, MAX_NORMALIZED_SPECULAR_COMPONENT),
            ]
        } else {
            [0.0, 0.0, 0.0]
        };

        let cull_flag = if prim.backface_culling != 0 { 1.0 } else { 0.0 };

        Self {
            diffuse_mtype: [
                sanitize_channel(prim.diffuse[0]).clamp(0.0, 2.0),
                sanitize_channel(prim.diffuse[1]).clamp(0.0, 2.0),
                sanitize_channel(prim.diffuse[2]).clamp(0.0, 2.0),
                prim.material_type as f32,
            ],
            ambient_shininess: [
                sanitize_channel(prim.ambient[0]).clamp(0.0, 2.0),
                sanitize_channel(prim.ambient[1]).clamp(0.0, 2.0),
                sanitize_channel(prim.ambient[2]).clamp(0.0, 2.0),
                shininess,
            ],
            specular_cull: [spec_rgb[0], spec_rgb[1], spec_rgb[2], cull_flag],
            // `MakeDrawPacket` (`0x0012d5a0` line 672) checks `*(char *)(part + 0x45) != 0`
            // (`ModelPrimitive::envmap_coeff`) before dispatching `MakeEnvironPacket` (`0x0012cf20`).
            _reserved: [f32::from(prim.envmap_coeff) / 128.0, 0.0, 0.0, 0.0],
        }
    }
}

#[inline]
fn sanitize_channel(v: f32) -> f32 {
    if v.is_finite() {
        v
    } else {
        0.0
    }
}

/// Packs per-primitive [`SkeletalMaterialUniforms`] into a single byte buffer respecting
/// `min_uniform_buffer_offset_alignment` (typically 256 bytes on Vulkan/Metal/DX12), returning
/// `(buffer_bytes, per_primitive_byte_offsets)`.
pub fn pack_aligned_material_buffer(
    primitives: &[ModelPrimitive],
    min_alignment: u32,
) -> (Vec<u8>, Vec<u32>) {
    let elem_size = std::mem::size_of::<SkeletalMaterialUniforms>();
    let align = (min_alignment as usize).max(16);
    let stride = elem_size.div_ceil(align) * align;

    if primitives.is_empty() {
        let mut buf = vec![0u8; stride];
        let def = SkeletalMaterialUniforms::default();
        buf[..elem_size].copy_from_slice(bytemuck::bytes_of(&def));
        return (buf, Vec::new());
    }

    let mut buf = vec![0u8; stride * primitives.len()];
    let mut offsets = Vec::with_capacity(primitives.len());

    for (idx, prim) in primitives.iter().enumerate() {
        let byte_offset = idx * stride;
        let mat = SkeletalMaterialUniforms::from_primitive(prim);
        buf[byte_offset..byte_offset + elem_size].copy_from_slice(bytemuck::bytes_of(&mat));
        offsets.push(byte_offset as u32);
    }

    (buf, offsets)
}

/// Returns whether a primitive with `pose_index` (`ModelMaterial::poseId` / PS2 `EquipmentId`)
/// should be active by default when loading `model`.
///
/// Matches PS2 `SLUS_202.28` `DemoJamesCheckModelParts` (`0x001044d0`) and
/// `PlayerCheckModelParts` (`0x001098d0`):
/// - `0`: Base body, head, clothing, and transparent hair/lashes (`tprim`).
/// - `3`: Default relaxed right hand (`hll_jms`, `lll_jms`, `lxx_mar`) or Great Knife (`red.mdl`).
/// - `14` (or `6` when `14` is absent): Default relaxed left hand (`hll_jms`/`lll_jms` = `14`,
///   `lxx_mar` = `6`).
/// - `21`: James Sunderland's breast-pocket flashlight clip & lens on `bone 3` (`hhh_jms`,
///   `hll_jms`, `lll_jms` — `0x00104514: addiu a3, zero, 21`).
pub fn is_default_active_pose(model: &ModelFile, pose_index: u8) -> bool {
    let has_variant_poses = model.primitives.iter().any(|pr| pr.pose_index > 0);
    if !has_variant_poses {
        return true;
    }
    let has_pose_14 = model.primitives.iter().any(|pr| pr.pose_index == 14);
    let has_pose_6 = model.primitives.iter().any(|pr| pr.pose_index == 6);

    pose_index == 0
        || pose_index == 3
        || pose_index == 21
        || (has_pose_14 && pose_index == 14)
        || (!has_pose_14 && has_pose_6 && pose_index == 6)
}

/// Named, mutually consistent equipment / hand-pose configuration for `.mdl` inspection (`SH2-37`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PoseVariantPreset {
    /// Human-readable description displayed in the Bestiary viewer HUD.
    pub label: &'static str,
    /// Set of `ModelPrimitive::pose_index` values enabled in this preset.
    pub active_poses: Vec<u8>,
}

/// Discovers all valid, non-overlapping [`PoseVariantPreset`] configurations for `model`
/// based on the `pose_index` (`EquipmentId`) values present in its primitives.
///
/// The first returned preset always matches [`is_default_active_pose`].
pub fn discover_pose_presets(model: &ModelFile) -> Vec<PoseVariantPreset> {
    let has = |p: u8| model.primitives.iter().any(|pr| pr.pose_index == p);

    if !model.primitives.iter().any(|pr| pr.pose_index > 0) {
        return vec![PoseVariantPreset {
            label: "BASE MODEL (POSE 0)",
            active_poses: vec![0],
        }];
    }

    // 1. James gameplay/low-LOD models (`hll_jms.mdl`, `lll_jms.mdl`: right hands 3..13, left hands 14..20, flashlight 21)
    if has(3) && has(14) && has(21) {
        let mut presets = vec![
            PoseVariantPreset {
                label: "RELAXED HANDS + POCKET FLASHLIGHT (0,3,14,21)",
                active_poses: vec![0, 3, 14, 21],
            },
            PoseVariantPreset {
                label: "RELAXED HANDS, NO FLASHLIGHT (0,3,14)",
                active_poses: vec![0, 3, 14],
            },
        ];
        let james_grips: &[(&str, u8, u8)] = &[
            ("HANDGUN GRIP + FLASHLIGHT (0,7,14,21)", 7, 14),
            ("PIPE / PLANK GRIP + FLASHLIGHT (0,8,14,21)", 8, 14),
            ("SHOTGUN GRIP + FLASHLIGHT (0,10,16,21)", 10, 16),
            ("HUNTING RIFLE GRIP + FLASHLIGHT (0,11,17,21)", 11, 17),
            ("GREAT KNIFE GRIP + FLASHLIGHT (0,12,18,21)", 12, 18),
            ("CHAINSAW / SPRAY GRIP + FLASHLIGHT (0,13,19,21)", 13, 19),
            ("ROWBOAT OAR GRIP + FLASHLIGHT (0,5,16,21)", 5, 16),
        ];
        for &(label, rh, lh) in james_grips {
            if has(rh) && has(lh) {
                presets.push(PoseVariantPreset {
                    label,
                    active_poses: vec![0, rh, lh, 21],
                });
            }
        }
        return presets;
    }

    // 2. High-LOD cutscene James (`hhh_jms.mdl`: base 0 + pocket flashlight 21)
    if has(21) && !has(3) {
        return vec![
            PoseVariantPreset {
                label: "POCKET FLASHLIGHT EQUIPPED (0,21)",
                active_poses: vec![0, 21],
            },
            PoseVariantPreset {
                label: "PRE-ROOM 205 / NO FLASHLIGHT (0)",
                active_poses: vec![0],
            },
        ];
    }

    // 3. Born from a Wish playable Maria (`lxx_mar.mdl`: right hand/weapons 3,4,5 + left hand 6,7)
    if has(3) && has(6) && (has(4) || has(5)) {
        let mut presets = vec![PoseVariantPreset {
            label: "UNARMED RELAXED HANDS (0,3,6)",
            active_poses: vec![0, 3, 6],
        }];
        if has(4) {
            presets.push(PoseVariantPreset {
                label: "REVOLVER EQUIPPED (0,4,6)",
                active_poses: vec![0, 4, 6],
            });
        }
        if has(5) {
            presets.push(PoseVariantPreset {
                label: "CHINESE CLEAVER EQUIPPED (0,5,6)",
                active_poses: vec![0, 5, 6],
            });
        }
        if has(7) {
            presets.push(PoseVariantPreset {
                label: "SECONDARY ITEM POSE (0,3,7)",
                active_poses: vec![0, 3, 7],
            });
        }
        return presets;
    }

    // 4. Pyramid Head (`red.mdl`: base 0 + Great Knife 3)
    if has(3) && !has(14) && !has(6) {
        return vec![
            PoseVariantPreset {
                label: "GREAT KNIFE EQUIPPED (0,3)",
                active_poses: vec![0, 3],
            },
            PoseVariantPreset {
                label: "UNARMED / BODY & HELMET ONLY (0)",
                active_poses: vec![0],
            },
        ];
    }

    // Generic fallback collecting default active poses
    let mut default_poses: Vec<u8> = model
        .primitives
        .iter()
        .map(|pr| pr.pose_index)
        .filter(|&p| is_default_active_pose(model, p))
        .collect();
    default_poses.sort_unstable();
    default_poses.dedup();
    vec![PoseVariantPreset {
        label: "DEFAULT POSE SET",
        active_poses: default_poses,
    }]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_prim(mtype: u8, cull: u32, spec: [f32; 3], spec_scale: f32) -> ModelPrimitive {
        ModelPrimitive {
            bone_indices: vec![0],
            pair_indices: vec![],
            texture_index: 0,
            start: 0,
            length: 3,
            pose_index: 0,
            material_type: mtype,
            backface_culling: cull,
            diffuse: [0.7, 0.7, 0.7],
            ambient: [0.3, 0.3, 0.3],
            specular: spec,
            specular_scale: spec_scale,
            envmap_coeff: 0,
            is_transparent: false,
        }
    }

    #[test]
    fn material_uniform_normalizes_ps2_specular_and_shininess() {
        // Pyramid Head `red.mdl` `prim[0]` (`material_type == 4`, `specular_scale == 4.68`, `spec ~= 82.43`)
        let red_helmet = sample_prim(4, 1, [82.43, 83.84, 82.43], 4.68);
        let u = SkeletalMaterialUniforms::from_primitive(&red_helmet);
        assert_eq!(u.diffuse_mtype[3], 4.0);
        assert_eq!(u.specular_cull[3], 1.0);
        assert!((u.specular_cull[0] - 82.43 / 128.0).abs() < 1e-5);
        assert!((u.ambient_shininess[3] - 4.68 * SHININESS_PER_SPECULAR_SCALE).abs() < 1e-4);

        // Maria `hhh_mar.mdl` `prim[3]` eyes (`specular == 384.0` -> clamped to `MAX_NORMALIZED_SPECULAR_COMPONENT`)
        let mar_eyes = sample_prim(4, 1, [384.0, 384.0, 384.0], 3.25);
        let u_eyes = SkeletalMaterialUniforms::from_primitive(&mar_eyes);
        assert_eq!(u_eyes.specular_cull[0], MAX_NORMALIZED_SPECULAR_COMPONENT);

        // `material_type == 2` and `3` must zero specular even if raw bytes are non-zero (`arm.mdl`, `tyu.mdl`)
        let mut arm_prim = sample_prim(3, 0, [85.38, 83.20, 84.61], 0.0);
        arm_prim.envmap_coeff = 0x40;
        let u_arm = SkeletalMaterialUniforms::from_primitive(&arm_prim);
        assert_eq!(u_arm.specular_cull, [0.0, 0.0, 0.0, 0.0]);
        assert!((u_arm._reserved[0] - 0.5).abs() < 1e-6);
    }

    #[test]
    fn packed_material_buffer_respects_256_byte_alignment() {
        let prims = vec![
            sample_prim(2, 1, [0.0; 3], 0.0),
            sample_prim(4, 0, [128.0; 3], 3.25),
        ];
        let (bytes, offsets) = pack_aligned_material_buffer(&prims, 256);
        assert_eq!(offsets, vec![0, 256]);
        assert_eq!(bytes.len(), 512);
    }

    #[test]
    fn gs_alpha_test_matches_slus_002a9780_and_sh2pc_00503800() {
        let atest = GsAlphaTest::from_gs_test_1(PS2_GS_TEST_ALPHA_GREATER_ZERO);
        assert!(atest.enabled);
        assert_eq!(atest.atst, 6);
        assert_eq!(atest.aref, 0);
        assert_eq!(atest.afail, 0);
        assert!(!atest.passes(0));
        assert!(atest.passes(1));
        assert!(atest.passes(255));

        let opaque = GsAlphaTest::from_gs_test_1(PS2_GS_TEST_OPAQUE_NO_ATEST);
        assert!(!opaque.enabled);
        assert!(opaque.passes(0));
        assert!(opaque.passes(128));

        // Verify normalized UNORM thresholds partition all 256 8-bit alpha values identically to `alpha_u8 > 0`:
        for a in 0u8..=255 {
            let norm = (a as f32) / 255.0;
            let in_core = norm >= PS2_ALPHA_OPAQUE_THRESHOLD;
            let in_fringe = (PS2_ALPHA_TEST_THRESHOLD..PS2_ALPHA_OPAQUE_THRESHOLD).contains(&norm);
            assert_eq!(in_core || in_fringe, atest.passes(a));
            assert!(!(in_core && in_fringe));
        }

        // Verify WGSL shader uses the exact PS2 alpha-test constants rather than uncited `0.90`.
        assert!(super::super::shader::WGSL_SKINNING_SHADER.contains("PS2_ALPHA_TEST_THRESHOLD"));
        assert!(super::super::shader::WGSL_SKINNING_SHADER.contains("PS2_ALPHA_OPAQUE_THRESHOLD"));
        assert!(!super::super::shader::WGSL_SKINNING_SHADER.contains("tex_color.a < 0.90"));
        assert!(!super::super::shader::WGSL_SKINNING_SHADER.contains("tex_color.a >= 0.90"));
    }
}
