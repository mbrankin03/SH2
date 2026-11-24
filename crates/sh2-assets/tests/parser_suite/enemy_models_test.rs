//! Retail-asset integration tests for all 12 `Enemy_List` character models, skeletal
//! hierarchies, animations, SH2:EE overrides, and weapon/attack sockets (`SH2-35`).

use std::fs;
use std::path::Path;

use sh2_assets::formats::anm::{
    compute_bind_locals, compute_skin_matrices_with_inv_into, evaluate_pose_into, parse_anm,
};
use sh2_assets::formats::enemy_rig::{
    enemy_sockets_for, evaluate_socket_world, EnemyModelKey, ENEMY_MODEL_SPECS,
};
use sh2_assets::formats::mdls::parse_mdl;

use crate::ROOT_ASSETS;

#[test]
fn test_sh2_35_all_12_enemy_list_models_rigs_anms_and_sockets() {
    if !Path::new(ROOT_ASSETS).exists() {
        println!("Skipping SH2-35 retail enemy test: assets not present");
        return;
    }

    assert_eq!(ENEMY_MODEL_SPECS.len(), 12);

    const IDENTITY: [[f32; 4]; 4] = [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];

    for spec in &ENEMY_MODEL_SPECS {
        let mdl_path = format!("{ROOT_ASSETS}/{}", spec.mdl_rel_path);
        let mdl_bytes = fs::read(&mdl_path).unwrap_or_else(|e| panic!("read {mdl_path}: {e}"));
        let model = parse_mdl(&mdl_bytes).unwrap_or_else(|e| panic!("parse {mdl_path}: {e:?}"));

        let raw_char_id = u32::from_le_bytes(mdl_bytes[4..8].try_into().unwrap());
        assert_eq!(
            raw_char_id, spec.char_id as u32,
            "{}: char_id mismatch",
            spec.code
        );
        assert_eq!(
            model.bone_count, spec.expected_bone_count,
            "{}: bone_count mismatch",
            spec.code
        );
        assert_eq!(model.parents.len(), model.bone_count);
        for (b, &p) in model.parents.iter().enumerate() {
            assert!(
                p == 0xff || (p as usize) < model.bone_count,
                "{}: bone {b} has out-of-range parent {p}",
                spec.code
            );
        }
        assert!(
            !model.primitives.is_empty() && !model.vertices.positions.is_empty(),
            "{}: empty geometry",
            spec.code
        );

        // Verify weapon pose_index when applicable (e.g. `red.mdl` Great Knife = pose 3)
        if let Some(wp_pose) = spec.weapon_pose_index {
            assert!(
                model.primitives.iter().any(|pr| pr.pose_index == wp_pose),
                "{}: expected weapon pose_index {wp_pose}",
                spec.code
            );
        }

        // Verify SH2:EE HD override `.mdl` when present (`sh2e/chr/scu/scu.mdl`)
        if let Some(sh2e_rel) = spec.sh2e_mdl_rel_path {
            let sh2e_path = format!("{ROOT_ASSETS}/{sh2e_rel}");
            if Path::new(&sh2e_path).exists() {
                let sh2e_bytes = fs::read(&sh2e_path).expect("read sh2e mdl");
                let sh2e_model = parse_mdl(&sh2e_bytes).expect("parse sh2e mdl");
                let sh2e_char_id = u32::from_le_bytes(sh2e_bytes[4..8].try_into().unwrap());
                assert_eq!(sh2e_model.bone_count, spec.expected_bone_count);
                assert_eq!(sh2e_char_id, spec.char_id as u32);
            }
        }

        // Verify primary + secondary `.anm` files
        let mut anm_paths = vec![spec.primary_anm_rel_path];
        anm_paths.extend_from_slice(spec.secondary_anm_rel_paths);

        for anm_rel in anm_paths {
            let anm_path = format!("{ROOT_ASSETS}/{anm_rel}");
            let anm_bytes = fs::read(&anm_path).unwrap_or_else(|e| panic!("read {anm_path}: {e}"));
            let anm = parse_anm(&anm_bytes, model.bone_count, &model.parents)
                .unwrap_or_else(|e| panic!("parse {anm_path}: {e:?}"));
            assert!(!anm.frames.is_empty(), "{anm_rel} must have frames");

            // Evaluate pose & skin matrices at frame 0 and mid-frame
            let bind_locals = compute_bind_locals(&model.parents, &model.initial_matrices);
            let mut pose_scratch = Vec::new();
            let mut skin_scratch = Vec::new();
            let sample_frames = [0usize, anm.frames.len() / 2];

            for &f_idx in &sample_frames {
                evaluate_pose_into(
                    &model.parents,
                    &bind_locals,
                    &anm.frames[f_idx],
                    &mut pose_scratch,
                );
                assert_eq!(pose_scratch.len(), model.bone_count);

                compute_skin_matrices_with_inv_into(
                    &pose_scratch,
                    &model.inv_initial_matrices,
                    &mut skin_scratch,
                );
                assert_eq!(skin_scratch.len(), model.bone_count);

                for (bi, mat) in skin_scratch.iter().enumerate() {
                    for row in mat {
                        for &val in row {
                            assert!(
                                val.is_finite(),
                                "{anm_rel} frame {f_idx} bone {bi} non-finite skin matrix"
                            );
                        }
                    }
                }

                // Evaluate all weapon/attack/foot sockets for this species
                for &sock in enemy_sockets_for(spec.key) {
                    let eval = evaluate_socket_world(IDENTITY, &pose_scratch, &sock.spec())
                        .unwrap_or_else(|| {
                            panic!("{}: socket {:?} bone out of bounds", spec.code, sock)
                        });
                    assert!(eval.origin.iter().all(|v| v.is_finite()));
                    assert!(eval.tip.iter().all(|v| v.is_finite()));
                    let dir_len = (eval.direction[0].powi(2)
                        + eval.direction[1].powi(2)
                        + eval.direction[2].powi(2))
                    .sqrt();
                    assert!(
                        (dir_len - 1.0).abs() < 1e-4,
                        "{}: socket {:?} non-unit direction len={dir_len}",
                        spec.code,
                        sock
                    );
                }
            }
        }
    }

    // Also verify High-LOD cutscene Eddie (`hhh_edi.mdl` + `p_edi.anm`)
    let hhh_edi_path = format!("{ROOT_ASSETS}/data/chr/edi/hhh_edi.mdl");
    let p_edi_path = format!("{ROOT_ASSETS}/data/chr/edi/p_edi.anm");
    let hhh_edi_bytes = fs::read(&hhh_edi_path).expect("hhh_edi.mdl");
    let hhh_edi = parse_mdl(&hhh_edi_bytes).expect("parse hhh_edi");
    assert_eq!(
        u32::from_le_bytes(hhh_edi_bytes[4..8].try_into().unwrap()),
        0x108
    );
    assert_eq!(hhh_edi.bone_count, 64);
    let p_edi = parse_anm(
        &fs::read(&p_edi_path).expect("p_edi.anm"),
        hhh_edi.bone_count,
        &hhh_edi.parents,
    )
    .expect("parse p_edi.anm");
    assert!(p_edi.frames.len() >= 368);
    let _ = EnemyModelKey::Edi;
}
