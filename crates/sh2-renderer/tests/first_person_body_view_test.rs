//! Retail `.mdl` / `.anm` First-Person body-view integration test (`SH2-93`).
//!
//! Skins retail `lll_jms`, `hll_jms`, `hhh_jms`, `lll_mar`, and `hhh_mar` (bind pose) plus every frame
//! of the engine locomotion clips (`IDLE`, `TURN_*`, `WALK`, `BACK`, `RUN1`, `RUN3`, `STRAFE_*`) on
//! `hhh_jms` + `jms_basic1.anm` and `lll_mar` + `xmar_wpnone.anm`, partitions each
//! active primitive into below-the-neck vs cranial triangles exactly like
//! `GpuSkeletalMesh::from_model`, evaluates the First-Person camera at level and max downward
//! pitch, and asserts the player sees the **outside** of the jacket/torso and legs and never into
//! the hollow neck/torso cavity.

use glam::{Mat4, Vec3, Vec4};
use sh2_assets::formats::anm::{
    compute_bind_locals, compute_skin_matrices_with_inv_into, evaluate_pose_into, parse_anm,
};
use sh2_assets::formats::mdls::{parse_mdl, ModelFile};
use sh2_core::animation::{AnimationClip, JamesBasicClips, MariaLocomotionClips};
use sh2_core::collision::CollisionWorld;
use sh2_core::modern_camera::head_rig::model_to_world;
use sh2_core::scenario::PlayableCharacter;
use sh2_core::{
    CharacterHeadRigSpec, FirstPersonCameraRig, ModernCameraTickInput, SpringArmConfig,
};
use sh2_renderer::{
    classify_primitive_body_region, cranial_bone_mask, find_cranial_root_bone,
    is_default_active_pose, partition_first_person_triangles, SkeletalBodyRegion,
};

const RETAIL_ROOT: &str = "../../Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2";
/// Matthew's `er01` playtest transform (`PLAYER POS`, `YAW = -2.00 rad`) plus the origin.
const PLACEMENTS: [(Vec3, f32); 2] = [(Vec3::ZERO, 0.0), (Vec3::new(-19225.8, 0.0, 20359.1), -2.0)];
/// Kept geometry closer than this to the eye inside the frustum would fill the screen / clip open.
const MIN_VISIBLE_DEPTH_UNITS: f32 = 20.0;
/// Bind-height band below the top of the kept mesh treated as the neck stump (checked at every gaze).
const STUMP_BAND_UNITS: f32 = 25.0;
/// Bind-height band below the top of the kept mesh treated as collar + upper chest (max-down gaze).
const COLLAR_BAND_UNITS: f32 = 60.0;
/// Bind-space half-width (`|x|`) of the collar / sternum strip (inside both shoulders).
const COLLAR_HALF_WIDTH_UNITS: f32 = 45.0;

/// Triangles kept in `HideHeadOnly`, tagged with their primitive's body region and pose index.
struct KeptGeometry {
    tris: Vec<([u16; 3], SkeletalBodyRegion, u8)>,
    cranial_tri_count: usize,
    /// Bind-pose `Y` of the highest kept vertex (top of the neck stump).
    top_bind_y: f32,
}

/// Mirrors `GpuSkeletalMesh::from_model` default-active pose selection and cranial partitioning.
fn kept_first_person_geometry(model: &ModelFile) -> KeptGeometry {
    let mask = cranial_bone_mask(model);
    let mut tris = Vec::new();
    let mut cranial_tri_count = 0;
    for (k, pr) in model.primitives.iter().enumerate() {
        if !is_default_active_pose(model, pr.pose_index) {
            continue;
        }
        let prim_tris = model.primitive_triangles(k);
        let region = classify_primitive_body_region(model, k, &prim_tris);
        let (body, cranial) = partition_first_person_triangles(model, &prim_tris, &mask);
        cranial_tri_count += cranial.len();
        tris.extend(body.into_iter().map(|t| (t, region, pr.pose_index)));
    }
    let top_bind_y = tris
        .iter()
        .flat_map(|(t, _, _)| t.iter())
        .map(|&v| model.vertices.positions[v as usize][1])
        .fold(f32::MAX, f32::min);
    KeptGeometry {
        tris,
        cranial_tri_count,
        top_bind_y,
    }
}

/// Linear-blend skins vertex `vi` with row-major `skin` matrices (model space, `+Y` down).
fn skin_vertex(model: &ModelFile, skin: &[[[f32; 4]; 4]], vi: usize) -> Vec3 {
    let p = model.vertices.positions[vi];
    let bones = model.vertices.bone_indices[vi];
    let weights = model.vertices.weights[vi];
    let mut out = Vec3::ZERO;
    for (b, w) in bones.iter().zip(weights) {
        if w <= 0.0 {
            continue;
        }
        let Some(m) = skin.get(*b as usize) else {
            continue;
        };
        let x = m[0][0] * p[0] + m[0][1] * p[1] + m[0][2] * p[2] + m[0][3];
        let y = m[1][0] * p[0] + m[1][1] * p[1] + m[1][2] * p[2] + m[1][3];
        let z = m[2][0] * p[0] + m[2][1] * p[1] + m[2][2] * p[2] + m[2][3];
        out += Vec3::new(x, y, z) * w;
    }
    out
}

/// Look-pitch scenario evaluated per pose.
#[derive(Clone, Copy, Debug)]
enum Gaze {
    Level,
    MaxDown,
}

struct PoseCase<'a> {
    label: &'a str,
    model: &'a ModelFile,
    kept: &'a KeptGeometry,
    ch: PlayableCharacter,
    pose: &'a [[[f32; 4]; 4]],
    skin: &'a [[[f32; 4]; 4]],
    /// Require the jacket/torso + legs to be on screen (bind-pose standing check at max pitch).
    require_body_on_screen: bool,
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

/// Evaluates the FP camera for `case` at every placement and gaze and asserts the body view.
fn assert_first_person_body_view(case: &PoseCase<'_>) {
    let collision = CollisionWorld::new();
    let spring = SpringArmConfig::default();
    for (pos, yaw) in PLACEMENTS {
        for gaze in [Gaze::Level, Gaze::MaxDown] {
            let mut rig = FirstPersonCameraRig::default();
            rig.reset_to_player(case.ch, pos, yaw);
            rig.update_from_skeleton_pose(case.ch, Some(case.pose));
            let flick = match gaze {
                Gaze::Level => 0.0,
                Gaze::MaxDown => 5.0,
            };
            let mut snap = rig
                .tick(&tick_input(case.ch, pos, yaw, flick), &collision, &spring)
                .0;
            for _ in 0..30 {
                snap = rig
                    .tick(&tick_input(case.ch, pos, yaw, 0.0), &collision, &spring)
                    .0;
            }
            let tag = format!("{} yaw {yaw} {gaze:?}", case.label);

            let view = Mat4::look_at_rh(snap.eye, snap.target, snap.up);
            let near = rig.config.near_clip_units;
            let proj = Mat4::perspective_rh(snap.fov_y, 16.0 / 9.0, near, 20000.0);
            let view_xz = Vec3::new(yaw.sin(), 0.0, yaw.cos());
            let depression = rig.look_pitch + 0.5 * snap.fov_y;
            let cot = depression.cos() / depression.sin();

            let mut torso_low = 0usize;
            let mut legs_low = 0usize;
            let mut stump_front_max = f32::MIN;
            let mut collar_front_max = f32::MIN;
            for (tri, region, pose_idx) in &case.kept.tris {
                for &vi in tri {
                    let bind = model_bind(case.model, vi);
                    let w =
                        model_to_world(pos, yaw, skin_vertex(case.model, case.skin, vi as usize));
                    let view_pos = view * Vec4::new(w.x, w.y, w.z, 1.0);
                    let clip = proj * view_pos;
                    let on_screen = clip.w > 1e-3
                        && (clip.x / clip.w).abs() <= 1.0
                        && (clip.y / clip.w).abs() <= 1.0;
                    // 1. Nothing visible sits on/near the near plane (no hollow cross-sections, no
                    //    neck stump filling the screen from an eye stuck behind it).
                    assert!(
                        !(on_screen && -view_pos.z < MIN_VISIBLE_DEPTH_UNITS),
                        "{tag}: kept vertex {vi} only {:.1} units in front of the eye",
                        -view_pos.z
                    );
                    if on_screen && clip.y / clip.w < 0.0 {
                        match region {
                            SkeletalBodyRegion::UpperTorso => torso_low += 1,
                            SkeletalBodyRegion::LowerOrHands
                                if *pose_idx == 0 && bind.y > -300.0 =>
                            {
                                legs_low += 1
                            }
                            _ => {}
                        }
                    }
                    // 2. Neck-stump lip (the only opening into the hollow mesh): the steepest
                    //    bottom-edge ray must pass in front of every stump vertex below the eye.
                    //    Collar + upper chest: the eye must sit in front of it when looking down.
                    let rel = w - snap.eye;
                    let fwd_v = rel.dot(view_xz);
                    let depth_below_top = bind.y - case.kept.top_bind_y;
                    if depth_below_top <= STUMP_BAND_UNITS && rel.y > 0.0 {
                        let ray_fwd = rel.y * cot;
                        assert!(
                            ray_fwd > fwd_v,
                            "{tag}: bottom-edge ray ({ray_fwd:.1}) behind neck-stump vertex {vi} ({fwd_v:.1}) -> would see inside torso"
                        );
                    }
                    // Bind-space centreline selects the neck/collar/sternum surface (not swinging arms).
                    if depth_below_top <= COLLAR_BAND_UNITS
                        && bind.x.abs() < COLLAR_HALF_WIDTH_UNITS
                    {
                        collar_front_max = collar_front_max.max(fwd_v);
                        if depth_below_top <= STUMP_BAND_UNITS {
                            stump_front_max = stump_front_max.max(fwd_v);
                        }
                    }
                }
            }
            // 3. The eye always sits in front of the neck stump (never above/behind it), and when
            //    looking down it is also in front of the whole collar + upper chest.
            assert!(
                stump_front_max < -8.0,
                "{tag}: eye must be >= 8 units in front of the neck stump (got {stump_front_max:.1})"
            );
            if matches!(gaze, Gaze::MaxDown) {
                assert!(
                    collar_front_max < 0.0,
                    "{tag}: eye must be strictly in front of collar/chest (got {collar_front_max:.1})"
                );
            }
            if case.require_body_on_screen && matches!(gaze, Gaze::MaxDown) {
                assert!(torso_low > 50, "{tag}: jacket/torso visible ({torso_low})");
                assert!(legs_low > 30, "{tag}: legs/shoes visible ({legs_low})");
            }
        }
    }
}

fn model_bind(model: &ModelFile, vi: u16) -> Vec3 {
    Vec3::from_array(model.vertices.positions[vi as usize])
}

fn load_model(rel: &str) -> Option<ModelFile> {
    let bytes = std::fs::read(format!("{RETAIL_ROOT}/{rel}")).ok()?;
    Some(parse_mdl(&bytes).expect("parse retail mdl"))
}

#[test]
fn test_sh2_93_first_person_bind_pose_sees_outside_of_retail_jacket_and_legs() {
    let cases = [
        ("data/chr/jms/lll_jms.mdl", PlayableCharacter::James),
        ("data/chr/jms/hll_jms.mdl", PlayableCharacter::James),
        ("data/chr/jms/hhh_jms.mdl", PlayableCharacter::James),
        ("data/chr/mar/lll_mar.mdl", PlayableCharacter::Maria),
        ("data/chr/mar/hhh_mar.mdl", PlayableCharacter::Maria),
    ];
    for (rel, ch) in cases {
        let Some(model) = load_model(rel) else {
            continue;
        };
        let spec = CharacterHeadRigSpec::for_character(ch);

        // Only the cranial subtree is hidden: root bone matches the camera head rig; its parent
        // (neck) and the pelvis stay visible.
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

        let identity = vec![Mat4::IDENTITY.transpose().to_cols_array_2d(); model.bone_count];
        assert_first_person_body_view(&PoseCase {
            label: rel,
            model: &model,
            kept: &kept,
            ch,
            pose: &model.initial_matrices,
            skin: &identity,
            require_body_on_screen: true,
        });
    }
}

/// Engine `_AnimeInfo` locomotion clips played while the player walks/runs/strafes/turns in place.
fn locomotion_frames(clips: &[AnimationClip]) -> Vec<usize> {
    clips
        .iter()
        .flat_map(|c| c.start_frame as usize..=c.end_frame as usize)
        .collect()
}

#[test]
fn test_sh2_93_first_person_animated_gaits_never_see_inside_retail_torso() {
    let james = locomotion_frames(&[
        JamesBasicClips::IDLE,
        JamesBasicClips::TURN_RIGHT,
        JamesBasicClips::TURN_LEFT,
        JamesBasicClips::WALK,
        JamesBasicClips::BACK,
        JamesBasicClips::RUN1,
        JamesBasicClips::RUN3,
        JamesBasicClips::STRAFE_RIGHT_WALK,
        JamesBasicClips::STRAFE_LEFT_WALK,
        JamesBasicClips::STRAFE_RIGHT_RUN,
        JamesBasicClips::STRAFE_LEFT_RUN,
    ]);
    let maria = locomotion_frames(&[
        MariaLocomotionClips::IDLE,
        MariaLocomotionClips::TURN_RIGHT,
        MariaLocomotionClips::TURN_LEFT,
        MariaLocomotionClips::WALK,
        MariaLocomotionClips::BACK,
        MariaLocomotionClips::RUN1,
        MariaLocomotionClips::RUN3,
        MariaLocomotionClips::STRAFE_RIGHT_WALK,
        MariaLocomotionClips::STRAFE_LEFT_WALK,
        MariaLocomotionClips::STRAFE_RIGHT_RUN,
        MariaLocomotionClips::STRAFE_LEFT_RUN,
    ]);
    let cases = [
        (
            "data/chr/jms/hhh_jms.mdl",
            "data/chr/jms/jms_basic1.anm",
            PlayableCharacter::James,
            &james,
        ),
        (
            "data/chr/mar/lll_mar.mdl",
            "data/chr2/mar/xmar_wpnone.anm",
            PlayableCharacter::Maria,
            &maria,
        ),
    ];
    for (mdl_rel, anm_rel, ch, frames) in cases {
        let Some(model) = load_model(mdl_rel) else {
            continue;
        };
        let Ok(anm_bytes) = std::fs::read(format!("{RETAIL_ROOT}/{anm_rel}")) else {
            continue;
        };
        let anm = parse_anm(&anm_bytes, model.bone_count, &model.parents).expect("parse anm");
        let kept = kept_first_person_geometry(&model);
        let locals = compute_bind_locals(&model.parents, &model.initial_matrices);
        let sampled: Vec<usize> = frames
            .iter()
            .copied()
            .filter(|&f| f < anm.frames.len())
            .collect();
        assert!(
            sampled.len() >= 200,
            "{anm_rel}: {} locomotion frames",
            sampled.len()
        );

        let mut pose = Vec::new();
        let mut skin = Vec::new();
        for f in sampled {
            evaluate_pose_into(&model.parents, &locals, &anm.frames[f], &mut pose);
            compute_skin_matrices_with_inv_into(&pose, &model.inv_initial_matrices, &mut skin);
            assert_first_person_body_view(&PoseCase {
                label: &format!("{mdl_rel} + {anm_rel} frame {f}"),
                model: &model,
                kept: &kept,
                ch,
                pose: &pose,
                skin: &skin,
                require_body_on_screen: false,
            });
        }
    }
}
