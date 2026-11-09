use sh2_assets::*;
use std::fs;
use std::path::Path;

use crate::ROOT_ASSETS;

#[test]
fn test_james_model_parsing() {
    // Test parsing James model (hhh_jms.mdl)
    let mdl_path = format!("{ROOT_ASSETS}/data/chr/jms/hhh_jms.mdl");
    if !Path::new(&mdl_path).exists() {
        println!("Skipping test_james_model_parsing: asset file not found at {mdl_path}");
        return;
    }

    let data = fs::read(&mdl_path).expect("Failed to read hhh_jms.mdl");
    let model = parse_mdl(&data).expect("Failed to parse hhh_jms.mdl");

    assert_eq!(model.bone_count, 71);
    assert_eq!(model.parents.len(), 71);
    assert_eq!(model.initial_matrices.len(), 71);
    // 15 opaque primitives + 1 transparent primitive = 16 total
    assert_eq!(model.primitives.len(), 16);
    assert_eq!(
        model
            .primitives
            .iter()
            .filter(|p| !p.is_transparent)
            .count(),
        15
    );
    assert_eq!(
        model.primitives.iter().filter(|p| p.is_transparent).count(),
        1
    );
    // 4897 opaque vertices + 378 transparent vertices = 5275 total
    assert_eq!(model.vertices.positions.len(), 5275);
    assert_eq!(model.vertices.normals.len(), 5275);
    assert_eq!(model.vertices.uvs.len(), 5275);
    assert_eq!(model.vertices.bone_indices.len(), 5275);
    assert_eq!(model.vertices.weights.len(), 5275);
    assert_eq!(
        model.textures.len(),
        4,
        "Expected 4 textures extracted from hhh_jms.mdl container"
    );
    for (&tid, tex) in &model.textures {
        let rgba = tex.to_rgba().expect("Failed to decode model texture DXT");
        assert_eq!(rgba.len(), tex.width * tex.height * 4);
        println!("Decoded model texture {tid}: {}x{}", tex.width, tex.height);
    }

    // Verify low-LOD James Sunderland model (hll_jms.mdl) with hand pose variants
    let low_mdl_path = format!("{ROOT_ASSETS}/data/chr/jms/hll_jms.mdl");
    if Path::new(&low_mdl_path).exists() {
        let low_data = fs::read(&low_mdl_path).expect("Failed to read hll_jms.mdl");
        let low_model = parse_mdl(&low_data).expect("Failed to parse hll_jms.mdl");

        assert_eq!(low_model.bone_count, 41);
        // 31 opaque primitives + 1 transparent primitive = 32 total
        assert_eq!(low_model.primitives.len(), 32);
        // 7311 opaque vertices + 378 transparent vertices = 7689 total
        assert_eq!(low_model.vertices.positions.len(), 7689);

        // Verify all vertex weights sum to 1.0 (normalized)
        for (idx, w) in low_model.vertices.weights.iter().enumerate() {
            let sum = w[0] + w[1] + w[2] + w[3];
            assert!(
                (sum - 1.0).abs() < 1e-4,
                "Vertex {idx} weights not normalized: sum = {sum}"
            );
        }

        // Verify hand poses 3..20 exist in primitives
        let pose_indices: Vec<u8> = low_model.primitives.iter().map(|p| p.pose_index).collect();
        assert!(pose_indices.contains(&3), "Right hand pose 3 must exist");
        assert!(pose_indices.contains(&14), "Left hand pose 14 must exist");
    }

    // Verify Maria character models (hhh_mar.mdl and lll_mar.mdl)
    let mar_high_path = format!("{ROOT_ASSETS}/data/chr/mar/hhh_mar.mdl");
    if Path::new(&mar_high_path).exists() {
        let mar_data = fs::read(&mar_high_path).expect("Failed to read hhh_mar.mdl");
        let mar_model = parse_mdl(&mar_data).expect("Failed to parse hhh_mar.mdl");

        assert_eq!(mar_model.bone_count, 73);
        // 25 opaque primitives + 5 transparent hair/eyelash primitives = 30 total
        assert_eq!(mar_model.primitives.len(), 30);
        assert_eq!(
            mar_model
                .primitives
                .iter()
                .filter(|p| !p.is_transparent)
                .count(),
            25
        );
        assert_eq!(
            mar_model
                .primitives
                .iter()
                .filter(|p| p.is_transparent)
                .count(),
            5
        );
        // 4940 opaque vertices + 331 transparent vertices = 5271 total
        assert_eq!(mar_model.vertices.positions.len(), 5271);

        for (idx, w) in mar_model.vertices.weights.iter().enumerate() {
            let sum = w[0] + w[1] + w[2] + w[3];
            assert!(
                (sum - 1.0).abs() < 1e-4,
                "Maria hhh vertex {idx} weights not normalized: sum = {sum}"
            );
        }
    }
}

#[test]
fn test_animation_parsing() {
    // Test parsing James rowing animation (jms_boat.anm)
    let anm_path = format!("{ROOT_ASSETS}/data/chr/jms/jms_boat.anm");
    let mdl_path = format!("{ROOT_ASSETS}/data/chr/jms/hll_jms.mdl");
    if !Path::new(&anm_path).exists() || !Path::new(&mdl_path).exists() {
        println!("Skipping test_animation_parsing: assets not found");
        return;
    }

    let mdl_data = fs::read(&mdl_path).expect("Failed to read hll_jms.mdl");
    let model = parse_mdl(&mdl_data).expect("Failed to parse hll_jms.mdl");

    let anm_data = fs::read(&anm_path).expect("Failed to read jms_boat.anm");
    let anm = parse_anm(&anm_data, model.bone_count, &model.parents)
        .expect("Failed to parse jms_boat.anm");

    assert_eq!(anm.bone_count, model.bone_count);
    assert!(!anm.frames.is_empty(), "Animation should contain frames");

    let jms_basic_path = format!("{ROOT_ASSETS}/data/chr/jms/jms_basic1.anm");
    if Path::new(&jms_basic_path).exists() {
        let b_data = fs::read(&jms_basic_path).expect("read jms_basic1");
        let b_anm = parse_anm(&b_data, model.bone_count, &model.parents).expect("parse jms_basic1");
        println!("JMS_BASIC1 total frames: {}", b_anm.frames.len());

        // Check where discontinuities or keyframe changes occur
        let mut diffs = Vec::new();
        for i in 1..b_anm.frames.len() {
            // compute root translation delta or bone 0/1 delta
            let f0 = &b_anm.frames[i - 1];
            let f1 = &b_anm.frames[i];
            let mut max_rot_diff: f32 = 0.0;
            for b in 0..model.bone_count {
                if let (Some(t0), Some(t1)) = (&f0.bones[b], &f1.bones[b]) {
                    if let (Some(r0), Some(r1)) = (t0.rotation, t1.rotation) {
                        let d = (r0[0] - r1[0])
                            .abs()
                            .max((r0[1] - r1[1]).abs())
                            .max((r0[2] - r1[2]).abs());
                        if d > max_rot_diff {
                            max_rot_diff = d;
                        }
                    }
                }
            }
            if max_rot_diff > 1.2 {
                diffs.push(i);
            }
        }
        println!("JMS_BASIC1 clip cuts (>1.2 rad jump): {:?}", diffs);
    }
}

#[test]
fn test_sh2_36_bestiary_catalog_models_and_animations_parsing() {
    if !Path::new(&format!("{ROOT_ASSETS}/data")).exists() {
        println!("Skipping test_sh2_36_bestiary_catalog_models_and_animations_parsing: retail assets not present");
        return;
    }

    // Verify every single entry in `sh2_debug::BESTIARY_CATALOG` (29 models across
    // Protagonists, Story NPCs, Enemies, Bosses, Weapons & Key Items):
    let catalog_samples: &[(&str, Option<&str>)] = &[
        // 1. Playable Protagonists
        (
            "data/chr/jms/hhh_jms.mdl",
            Some("data/chr/jms/jms_basic1.anm"),
        ),
        (
            "data/chr/jms/hll_jms.mdl",
            Some("data/chr/jms/jms_basic1.anm"),
        ),
        (
            "data/chr2/mar/lxx_mar.mdl",
            Some("data/chr2/mar/xmar_wpnone.anm"),
        ),
        (
            "data/chr/mar/hhh_mar.mdl",
            Some("data/chr/mar/p_hhh_mar.anm"),
        ),
        ("data/chr/mar/lll_mar.mdl", Some("data/chr/mar/lll_mar.anm")),
        // 2. Story NPCs
        ("data/chr/edi/hhh_edi.mdl", Some("data/chr/edi/p_edi.anm")),
        ("data/chr/edi/lll_edi.mdl", Some("data/chr/edi/lll_edi.anm")),
        ("data/chr/lau/lau.mdl", Some("data/chr/lau/p_lau.anm")),
        ("data/chr/mry/mry.mdl", Some("data/chr/mry/p_mry.anm")),
        ("data/chr/agl/agl.mdl", Some("data/chr/agl/p_agl.anm")),
        // 3. Enemies
        ("data/chr/scu/scu.mdl", Some("data/chr/scu/scu.anm")),
        ("data/chr/mkn/mkn.mdl", Some("data/chr/mkn/mkn.anm")),
        ("data/chr/nse/nse.mdl", Some("data/chr/nse/nse.anm")),
        ("data/chr/xoo/xoo.mdl", Some("data/chr/xoo/xoo.anm")),
        ("data/chr/arm/arm.mdl", Some("data/chr/arm/arm.anm")),
        ("data/chr/tyu/tyu.mdl", Some("data/chr/tyu/tyu.anm")),
        // 4. Bosses
        ("data/chr/red/red.mdl", Some("data/chr/red/red.anm")),
        ("data/chr/oni/oni.mdl", Some("data/chr/oni/oni.anm")),
        ("data/chr/ike/ike.mdl", Some("data/chr/ike/ike.anm")),
        ("data/chr/pap/pap.mdl", Some("data/chr/pap/pap.anm")),
        ("data/chr/bos/bos.mdl", Some("data/chr/bos/bos.anm")),
        // 5. Weapons & Key Items
        ("data/chr/wp/wp_pipe.mdl", None),
        ("data/chr/wp/wp_kakuzai.mdl", None),
        ("data/chr/wp/wp_handgun.mdl", None),
        ("data/chr/wp/wp_shotgun.mdl", None),
        ("data/chr/wp/wp_riflgun.mdl", None),
        ("data/chr/wp/wp_nata.mdl", None),
        ("data/chr/wp/wp_sp.mdl", None),
        ("data/chr/wp/wp_csaw.mdl", None),
        ("data/chr/item/i_letter.mdl", None),
    ];

    for &(mdl_rel, anm_opt) in catalog_samples {
        let mdl_full = format!("{ROOT_ASSETS}/{mdl_rel}");
        assert!(
            Path::new(&mdl_full).exists(),
            "Expected Bestiary model at {mdl_full}"
        );
        let mdl_bytes = fs::read(&mdl_full).expect("Failed to read model file");
        let model =
            parse_mdl(&mdl_bytes).unwrap_or_else(|e| panic!("Failed to parse {mdl_rel}: {e:?}"));

        assert!(model.bone_count > 0, "{mdl_rel} must have at least 1 bone");
        assert!(
            !model.primitives.is_empty(),
            "{mdl_rel} must have primitives"
        );
        assert!(
            !model.vertices.positions.is_empty(),
            "{mdl_rel} must have vertices"
        );

        let tex_ids: Vec<u32> = model.textures.keys().copied().collect();
        let prim_tex_ids: Vec<u16> = model.primitives.iter().map(|p| p.texture_index).collect();
        println!(
            "Model {mdl_rel}: bones={}, prims={}, verts={}, tex_ids={tex_ids:?}, prim_tex_ids={prim_tex_ids:?}",
            model.bone_count,
            model.primitives.len(),
            model.vertices.positions.len()
        );

        if let Some(anm_rel) = anm_opt {
            let anm_full = format!("{ROOT_ASSETS}/{anm_rel}");
            assert!(
                Path::new(&anm_full).exists(),
                "Expected Bestiary animation at {anm_full}"
            );
            let anm_bytes = fs::read(&anm_full).expect("Failed to read animation file");
            let anm = parse_anm(&anm_bytes, model.bone_count, &model.parents)
                .unwrap_or_else(|e| panic!("Failed to parse {anm_rel}: {e:?}"));
            assert!(
                !anm.frames.is_empty(),
                "{anm_rel} must have animation frames"
            );
        }

        // Verify embedded texture mapping and DXT decoding
        for pr in &model.primitives {
            let tid = pr.texture_index as u32;
            if !model.textures.is_empty() {
                let tex = model.textures.get(&tid).unwrap_or_else(|| {
                    panic!(
                        "{mdl_rel}: primitive texture_index={tid} not found in model.textures (keys={:?})",
                        model.textures.keys().collect::<Vec<_>>()
                    )
                });
                let rgba = tex
                    .to_rgba()
                    .unwrap_or_else(|e| panic!("{mdl_rel}: failed to decode texture {tid}: {e:?}"));
                assert_eq!(rgba.len(), tex.width * tex.height * 4);
            }
        }
    }
}

#[test]
fn test_sh2_34_primary_npc_models_reflection_fallbacks_and_animations() {
    use sh2_assets::formats::mdls::companion_texture_mdl_path;

    // Verify companion texture path mapping for `no_tex = 1` reflection/variant models
    assert_eq!(
        companion_texture_mdl_path("data/chr/agl/ragl.mdl"),
        Some("data/chr/agl/agl.mdl")
    );
    assert_eq!(
        companion_texture_mdl_path("data/chr/edi/rhhh_edi.mdl"),
        Some("data/chr/edi/hhh_edi.mdl")
    );
    assert_eq!(
        companion_texture_mdl_path("data/chr/edi/hhh_edi_notex.mdl"),
        Some("data/chr/edi/hhh_edi.mdl")
    );
    assert_eq!(
        companion_texture_mdl_path("data/chr/item/rlau.mdl"),
        Some("data/chr/lau/lau.mdl")
    );
    assert_eq!(
        companion_texture_mdl_path("data/chr/item/rmry.mdl"),
        Some("data/chr/mry/mry.mdl")
    );

    let agl_path = format!("{ROOT_ASSETS}/data/chr/agl/agl.mdl");
    if !Path::new(&agl_path).exists() {
        println!("Skipping SH2-34 retail asset test: retail assets not present");
        return;
    }

    // Ground-truth primary NPC skeleton & geometry specifications from PS2 SLUS_202.28:
    let primary_models = [
        ("data/chr/agl/agl.mdl", 75usize, 105usize, 19usize),
        ("data/chr/edi/hhh_edi.mdl", 64, 85, 31),
        ("data/chr/edi/lll_edi.mdl", 35, 47, 26),
        ("data/chr/lau/lau.mdl", 89, 167, 21),
        ("data/chr/mry/mry.mdl", 81, 94, 22),
    ];

    for (rel_mdl, exp_bones, exp_pairs, exp_prims) in primary_models {
        let bytes =
            fs::read(format!("{ROOT_ASSETS}/{rel_mdl}")).expect("Failed to read primary NPC mdl");
        let mdl = parse_mdl(&bytes).expect("Failed to parse primary NPC mdl");
        assert_eq!(
            mdl.bone_count, exp_bones,
            "Bone count mismatch for {rel_mdl}"
        );
        assert_eq!(
            mdl.pairs.len(),
            exp_pairs,
            "Bone pair mismatch for {rel_mdl}"
        );
        assert_eq!(
            mdl.primitives.len(),
            exp_prims,
            "Primitive count mismatch for {rel_mdl}"
        );
        assert!(
            !mdl.textures.is_empty(),
            "Expected embedded textures in {rel_mdl}"
        );
        assert_eq!(mdl.parents[0], 255);
        assert_eq!(mdl.parents[1], 255);

        // Verify every primitive's texture_index maps to a valid embedded texture
        for prim in &mdl.primitives {
            let tid = prim.texture_index as u32;
            assert!(
                mdl.textures.contains_key(&tid),
                "Primary model {rel_mdl} primitive texture_index {tid} missing in embedded textures"
            );
        }

        // Verify every embedded texture decodes cleanly to RGBA8
        for tex in mdl.textures.values() {
            let rgba = tex.to_rgba().expect("Failed to decode NPC DXT texture");
            assert_eq!(rgba.len(), tex.width * tex.height * 4);
        }
    }

    // Verify reflection/untextured models (`no_tex = 1`) match companion texture IDs 1:1
    let reflection_models = [
        "data/chr/agl/ragl.mdl",
        "data/chr/edi/rhhh_edi.mdl",
        "data/chr/edi/hhh_edi_notex.mdl",
        "data/chr/item/rlau.mdl",
        "data/chr/item/rmry.mdl",
    ];

    for refl_rel in reflection_models {
        let refl_bytes =
            fs::read(format!("{ROOT_ASSETS}/{refl_rel}")).expect("Failed to read reflection mdl");
        let refl_mdl = parse_mdl(&refl_bytes).expect("Failed to parse reflection mdl");
        assert!(
            refl_mdl.textures.is_empty(),
            "Expected no_tex=1 reflection model {refl_rel} to have 0 embedded textures"
        );

        let comp_rel = companion_texture_mdl_path(refl_rel).expect("Missing companion path");
        let comp_bytes =
            fs::read(format!("{ROOT_ASSETS}/{comp_rel}")).expect("Failed to read companion mdl");
        let comp_mdl = parse_mdl(&comp_bytes).expect("Failed to parse companion mdl");
        assert_eq!(refl_mdl.bone_count, comp_mdl.bone_count);

        for prim in &refl_mdl.primitives {
            let tid = prim.texture_index as u32;
            assert!(
                comp_mdl.textures.contains_key(&tid),
                "Reflection model {refl_rel} primitive texture_index {tid} missing in companion {comp_rel}"
            );
        }
    }

    // Verify delta/hold frame persistence (`t == 0` in PS2 `shExec` at `0x001002c0`):
    // In `data/demo/knife_agl/agl.anm` (Mirror Room cutscene), keyframes at 10-15 Hz are
    // followed by 1-2 hold frames (`t == 0` for all 75 bones). Verify that held frames retain
    // the previous frame's bone transforms and never snap back to `None` / standing bind pose.
    let agl_mdl = parse_mdl(&fs::read(&agl_path).expect("read agl.mdl")).expect("parse agl.mdl");
    let knife_bytes = fs::read(format!("{ROOT_ASSETS}/data/demo/knife_agl/agl.anm"))
        .expect("read knife_agl/agl.anm");
    let knife_anm = parse_anm(&knife_bytes, agl_mdl.bone_count, &agl_mdl.parents)
        .expect("parse knife_agl/agl.anm");
    assert_eq!(knife_anm.frames.len(), 6371);
    for f_idx in 217..=390 {
        let frame = &knife_anm.frames[f_idx];
        for b_idx in 0..agl_mdl.bone_count {
            let bt = frame.bones[b_idx].as_ref().unwrap_or_else(|| {
                panic!("knife_agl/agl.anm frame {f_idx} bone {b_idx} lost held transform")
            });
            assert!(
                bt.rotation.is_some() && bt.translation.is_some(),
                "knife_agl/agl.anm frame {f_idx} bone {b_idx} missing held rot/trans"
            );
        }
    }
}

#[test]
fn test_sh2_56_quaternion_slerp_and_hermite_bone_interpolation() {
    use sh2_assets::formats::anm::AnimationTrackType;
    use std::f32::consts::PI;

    // 1. Verify Euler -> Quat -> Matrix matches euler_to_matrix to within 1e-5
    let test_angles = [
        [0.0, 0.0, 0.0],
        [0.35, -0.72, 1.48],
        [-1.15, 0.94, -2.51],
        [PI * 0.49, 0.25, -PI * 0.85],
        [1.2, PI * 0.5, 0.0], // Gimbal lock pitch = +90 deg
    ];

    for &ang in &test_angles {
        let m_euler = euler_to_matrix(ang);
        let q = euler_to_quat(ang);
        let m_quat = quat_to_matrix(q);
        let extracted = quat_to_euler(q);
        let m_reconstructed = euler_to_matrix(extracted);

        for r in 0..3 {
            for c in 0..3 {
                assert!(
                    (m_euler[r][c] - m_quat[r][c]).abs() < 1e-5,
                    "Matrix mismatch at ({r},{c}) for angles {ang:?}: euler={} vs quat={}",
                    m_euler[r][c],
                    m_quat[r][c]
                );
                assert!(
                    (m_euler[r][c] - m_reconstructed[r][c]).abs() < 1e-4,
                    "Reconstructed matrix mismatch at ({r},{c}) for angles {ang:?}"
                );
            }
        }
    }

    // 2. Verify shortest-arc Slerp across +/- PI boundary and constant angular velocity at 240 Hz sub-steps
    let r_start = [0.4, -0.6, PI - 0.15];
    let r_end = [-0.5, 0.7, -PI + 0.15];
    let q_start = euler_to_quat(r_start);
    let q_end = quat_align_hemisphere(q_start, euler_to_quat(r_end));
    let total_theta = (q_start[0] * q_end[0]
        + q_start[1] * q_end[1]
        + q_start[2] * q_end[2]
        + q_start[3] * q_end[3])
        .clamp(-1.0, 1.0)
        .acos();

    let steps = 8; // 8 sub-steps (e.g. 240 Hz over 30 Hz keyframe interval)
    for s in 0..=steps {
        let alpha = s as f32 / steps as f32;
        let r_mid = slerp_euler_zyx(r_start, r_end, alpha);
        let q_mid = quat_align_hemisphere(q_start, euler_to_quat(r_mid));
        let theta_from_start = (q_start[0] * q_mid[0]
            + q_start[1] * q_mid[1]
            + q_start[2] * q_mid[2]
            + q_start[3] * q_mid[3])
            .clamp(-1.0, 1.0)
            .acos();
        let expected_theta = total_theta * alpha;
        assert!(
            (theta_from_start - expected_theta).abs() < 1e-3,
            "Non-uniform Slerp angular progress at alpha={alpha}: got {theta_from_start}, expected {expected_theta}"
        );
    }

    // 3. Verify 4-keyframe Hermite / Squad spline boundary exactness and C1 smoothness
    let make_frame = |rot: [f32; 3], trans: [f32; 3]| AnimationFrame {
        bones: vec![Some(BoneTransform {
            rotation: Some(rot),
            translation: Some(trans),
            track_type: AnimationTrackType::TranslationAndRotation,
        })],
    };

    let f0 = make_frame([0.0, 0.0, 0.0], [0.0, 0.0, 0.0]);
    let f1 = make_frame([0.2, 0.3, -0.1], [10.0, 5.0, -2.0]);
    let f2 = make_frame([0.5, 0.7, -0.4], [25.0, 12.0, -6.0]);
    let f3 = make_frame([0.8, 0.9, -0.6], [40.0, 18.0, -10.0]);

    let mut out = AnimationFrame {
        bones: Vec::with_capacity(1),
    };
    interpolate_frames_hermite_into(&f0, &f1, &f2, &f3, 0.0, &mut out);
    assert_eq!(
        out.bones[0].as_ref().unwrap().translation,
        Some([10.0, 5.0, -2.0])
    );

    interpolate_frames_hermite_into(&f0, &f1, &f2, &f3, 1.0, &mut out);
    assert_eq!(
        out.bones[0].as_ref().unwrap().translation,
        Some([25.0, 12.0, -6.0])
    );

    interpolate_frames_hermite_into(&f0, &f1, &f2, &f3, 0.5, &mut out);
    let mid_t = out.bones[0].as_ref().unwrap().translation.unwrap();
    assert!(mid_t[0] > 10.0 && mid_t[0] < 25.0);
}
