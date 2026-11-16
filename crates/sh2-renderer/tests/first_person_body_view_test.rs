//! Retail `.mdl` First-Person body-view integration test (`SH2-93`).
//!
//! Skins the bind pose of retail `lll_jms`, `hll_jms`, `hhh_jms`, `lll_mar`, and `hhh_mar`,
//! partitions each active primitive into below-the-neck vs cranial triangles exactly like
//! `GpuSkeletalMesh::from_model`, evaluates the First-Person camera at max downward pitch, and
//! asserts the player sees the **outside** of the jacket/torso and legs and never into the
//! hollow neck/torso cavity.

use glam::{Mat4, Vec3, Vec4};
use sh2_assets::formats::mdls::{parse_mdl, ModelFile};
use sh2_core::collision::CollisionWorld;
use sh2_core::modern_camera::head_rig::model_to_world;
use sh2_core::scenario::PlayableCharacter;
use sh2_core::{
    CharacterHeadRigSpec, FirstPersonCameraRig, ModernCameraTickInput, SpringArmConfig,
};
use sh2_renderer::{
    classify_primitive_body_region, cranial_bone_mask, find_cranial_root_bone,
    partition_first_person_triangles, SkeletalBodyRegion,
};

const RETAIL_ROOT: &str = "../../Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2";

/// Triangles kept in `HideHeadOnly`, tagged with their primitive's body region.
struct KeptGeometry {
    tris: Vec<([u16; 3], SkeletalBodyRegion, u8)>,
    cranial_tri_count: usize,
}

/// Mirrors `GpuSkeletalMesh::from_model` default-active pose selection and cranial partitioning.
fn kept_first_person_geometry(model: &ModelFile) -> KeptGeometry {
    let has_variants = model.primitives.iter().any(|p| p.pose_index > 0);
    let has_14 = model.primitives.iter().any(|p| p.pose_index == 14);
    let has_6 = model.primitives.iter().any(|p| p.pose_index == 6);
    let mask = cranial_bone_mask(model);
    let mut out = KeptGeometry {
        tris: Vec::new(),
        cranial_tri_count: 0,
    };
    for (k, pr) in model.primitives.iter().enumerate() {
        let active = !has_variants
            || pr.pose_index == 0
            || pr.pose_index == 3
            || (has_14 && pr.pose_index == 14)
            || (!has_14 && has_6 && pr.pose_index == 6);
        if !active {
            continue;
        }
        let tris = model.primitive_triangles(k);
        let region = classify_primitive_body_region(model, k, &tris);
        let (body, cranial) = partition_first_person_triangles(model, &tris, &mask);
        out.cranial_tri_count += cranial.len();
        out.tris
            .extend(body.into_iter().map(|t| (t, region, pr.pose_index)));
    }
    out
}

fn tick_input(
    ch: PlayableCharacter,
    pos: Vec3,
    yaw: f32,
    pitch_delta: f32,
) -> ModernCameraTickInput {
    ModernCameraTickInput {
        character: ch,
        player_pos: pos,
        player_yaw: yaw,
        linear_speed: 0.0,
        right_stick_x: 0.0,
        right_stick_y: 0.0,
        extra_yaw_delta_rad: 0.0,
        extra_pitch_delta_rad: pitch_delta,
        sensitivity: 1.0,
        ready_weapon: false,
        quick_turn_pressed: false,
        shoulder_swap_pressed: false,
        in_bug_room: false,
    }
}

#[test]
fn test_sh2_93_first_person_sees_outside_of_retail_jacket_and_legs_at_max_pitch() {
    let cases = [
        ("data/chr/jms/lll_jms.mdl", PlayableCharacter::James),
        ("data/chr/jms/hll_jms.mdl", PlayableCharacter::James),
        ("data/chr/jms/hhh_jms.mdl", PlayableCharacter::James),
        ("data/chr/mar/lll_mar.mdl", PlayableCharacter::Maria),
        ("data/chr/mar/hhh_mar.mdl", PlayableCharacter::Maria),
    ];
    // Origin plus Matthew's `er01` playtest transform (`PLAYER POS`, `YAW = -2.00 rad`).
    let placements = [
        (Vec3::ZERO, 0.0_f32),
        (Vec3::new(-19225.8, 0.0, 20359.1), -2.0),
    ];
    let collision = CollisionWorld::new();
    let spring = SpringArmConfig::default();

    for (rel, ch) in cases {
        let full = format!("{RETAIL_ROOT}/{rel}");
        let Ok(bytes) = std::fs::read(&full) else {
            continue;
        };
        let model = parse_mdl(&bytes).expect("parse retail mdl");
        let spec = CharacterHeadRigSpec::for_character(ch);

        // 1. Only the cranial subtree is hidden: root bone matches the camera head rig; its parent
        //    (neck) and the pelvis stay visible.
        assert_eq!(
            find_cranial_root_bone(&model),
            Some(spec.head_bone_index),
            "{rel}"
        );
        let mask = cranial_bone_mask(&model);
        let neck = model.parents[spec.head_bone_index] as usize;
        assert!(
            mask[spec.head_bone_index] && !mask[neck] && !mask[0],
            "{rel}"
        );
        if model.bone_count >= spec.min_eye_rig_bones {
            assert!(mask[spec.left_eye_bone_index] && mask[spec.right_eye_bone_index]);
        }
        let kept = kept_first_person_geometry(&model);
        assert!(
            kept.cranial_tri_count > 100,
            "{rel}: head/hair triangles hidden"
        );
        let torso_tris = kept
            .tris
            .iter()
            .filter(|(_, r, _)| *r == SkeletalBodyRegion::UpperTorso)
            .count();
        assert!(
            torso_tris > 200,
            "{rel}: jacket/torso triangles kept ({torso_tris})"
        );
        let kept_top_y = kept
            .tris
            .iter()
            .flat_map(|(t, _, _)| t.iter())
            .map(|&v| model.vertices.positions[v as usize][1])
            .fold(f32::MAX, f32::min);

        for (pos, yaw) in placements {
            let mut rig = FirstPersonCameraRig::default();
            rig.reset_to_player(ch, pos, yaw);
            rig.update_from_skeleton_pose(ch, pos, yaw, Some(&model.initial_matrices));
            let mut snap = rig
                .tick(&tick_input(ch, pos, yaw, 5.0), &collision, &spring)
                .0;
            for _ in 0..30 {
                snap = rig
                    .tick(&tick_input(ch, pos, yaw, 0.0), &collision, &spring)
                    .0;
            }
            assert!((rig.look_pitch - rig.config.max_pitch_radians).abs() < 1e-5);

            let view = Mat4::look_at_rh(snap.eye, snap.target, snap.up);
            let near = rig.config.near_clip_units;
            let proj = Mat4::perspective_rh(snap.fov_y, 16.0 / 9.0, near, 20000.0);
            let view_xz = Vec3::new(yaw.sin(), 0.0, yaw.cos());
            let right_xz = Vec3::new(yaw.cos(), 0.0, -yaw.sin());
            let depression = rig.look_pitch + 0.5 * snap.fov_y;
            let cot = depression.cos() / depression.sin();

            let mut torso_visible_low = 0usize;
            let mut legs_visible_low = 0usize;
            let mut collar_front_max = f32::MIN;
            for (tri, region, pose) in &kept.tris {
                for &vi in tri {
                    let p = Vec3::from_array(model.vertices.positions[vi as usize]);
                    let w = model_to_world(pos, yaw, p);
                    let view_pos = view * Vec4::new(w.x, w.y, w.z, 1.0);
                    let depth = -view_pos.z;
                    let clip = proj * view_pos;
                    let in_lateral = clip.w > 1e-3
                        && (clip.x / clip.w).abs() <= 1.0
                        && (clip.y / clip.w).abs() <= 1.0;
                    // 2. Nothing visible pokes through the near plane (no hollow cross-sections).
                    assert!(
                        !(in_lateral && depth > 0.0 && depth <= near),
                        "{rel}: kept vertex {vi} inside near plane"
                    );
                    if in_lateral && clip.y / clip.w < 0.0 {
                        match region {
                            SkeletalBodyRegion::UpperTorso => torso_visible_low += 1,
                            SkeletalBodyRegion::LowerOrHands if *pose == 0 && p.y > -300.0 => {
                                legs_visible_low += 1
                            }
                            _ => {}
                        }
                    }

                    // 3. Collar / neck-stump rim: the eye sits strictly in front, and the steepest
                    //    bottom-edge ray passes in front of every rim vertex below the eye.
                    let rel_w = w - snap.eye;
                    if p.y <= kept_top_y + 60.0 && rel_w.dot(right_xz).abs() < 70.0 {
                        let fwd_v = rel_w.dot(view_xz);
                        collar_front_max = collar_front_max.max(fwd_v);
                        if rel_w.y > 0.0 {
                            let ray_fwd = rel_w.y * cot;
                            assert!(
                                ray_fwd > fwd_v,
                                "{rel} yaw {yaw}: bottom-edge ray ({ray_fwd:.1}) behind collar vertex ({fwd_v:.1}) -> would see inside torso"
                            );
                        }
                    }
                }
            }
            assert!(
                collar_front_max < -8.0,
                "{rel} yaw {yaw}: eye must be >= 8 units in front of collar/chest at collar height (got {collar_front_max:.1})"
            );
            assert!(
                torso_visible_low > 50,
                "{rel}: jacket/torso visible ({torso_visible_low})"
            );
            assert!(
                legs_visible_low > 30,
                "{rel}: legs/shoes visible ({legs_visible_low})"
            );
        }
    }
}
