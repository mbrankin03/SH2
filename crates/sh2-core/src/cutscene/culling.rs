//! Smart off-screen `4:3` parked actor frustum culling for widescreen (`16:9`) and ultrawide
//! (`21:9` / `32:9`) real-time 3D cutscenes (`SH2-77`).
//!
//! # Ground Truth & Problem Statement
//! In Silent Hill 2 (`Event/demoview.c`), `.dds` cutscene scripts keep actors (`char_visible[i] = true`)
//! parked just outside the `4:3` camera frame waiting to walk into a shot or frozen in place after
//! walking out of a `4:3` shot, or after a hard camera cut (`Key 0x10`).
//!
//! When rendering with a native Hor+ widescreen/ultrawide projection (`16:9`, `21:9`, `32:9`),
//! the horizontal field of view widens beyond `fov_x_4_3 = 2 * atan(tan(fov_y / 2) * (4.0 / 3.0))`,
//! exposing these stationary T-posing or frozen actors in the side margins.
//!
//! # Algorithm
//! For each actor slot marked visible by the `.dds` script (`script_visible == true`):
//! 1. Compute the authored `4:3` horizontal frustum cone (`fov_y_4x3`, `fov_x_4x3`) from the current
//!    camera plane and per-demo FOV scale.
//! 2. Transform the actor's world bounding sphere center into camera view space (`right`, `up`, `forward`,
//!    accounting for `roll_radians` around `forward`).
//! 3. Test whether the bounding sphere intersects the authored `4:3` frustum cone:
//!    - If the sphere intersects or lies inside the `4:3` frustum, the actor is **inside the authored frame**
//!      and is always rendered (`cull = false`).
//! 4. If the actor's bounding sphere lies completely outside the `4:3` frustum cone (i.e. inside the
//!    widescreen/ultrawide side margins or behind the camera):
//!    - Measure the actor's keyframe velocity `delta_pos = |next_key_pos - last_key_pos|` across adjacent
//!      30 FPS `.dds` keyframes.
//!    - If the actor is **stationary / parked** (`delta_pos <= PARKED_ACTOR_VELOCITY_EPSILON`, waiting to
//!      enter or frozen after exiting) or moving away from the `4:3` cone, cull the actor (`cull = true`)
//!      until it begins moving toward or enters the active `4:3` staging cone.

use glam::{Mat4, Vec3};
use sh2_assets::formats::demo::{CutsceneWidescreenMode, AUTHORED_4X3_ASPECT};

use super::anim_info::DramaDemoAnimInfo;

/// Maximum per-frame root displacement (in PS2 world units / mm per 30 FPS frame) below which
/// an off-screen actor outside the `4:3` frustum is classified as stationary ("parked").
pub const PARKED_ACTOR_VELOCITY_EPSILON: f32 = 1.5;

/// Default horizontal bounding radius for humanoid cutscene actors (`480.0` mm),
/// covering outstretched arms, leaning shoulders, and sitting/lying poses near the `4:3` border.
pub const DEFAULT_HUMANOID_BOUNDING_RADIUS: f32 = 480.0;

/// Default horizontal bounding radius for large boss actors (`520.0` mm).
pub const DEFAULT_BOSS_BOUNDING_RADIUS: f32 = 520.0;

/// Default horizontal bounding radius for doors, furniture, and room props (`460.0` mm).
pub const DEFAULT_PROP_BOUNDING_RADIUS: f32 = 460.0;

/// Default horizontal bounding radius for hand-held cutscene items (`250.0` mm).
pub const DEFAULT_ITEM_BOUNDING_RADIUS: f32 = 250.0;

/// Approximate world-space bounding sphere for a cutscene character or prop model (`SH2-77`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ActorBoundingSphere {
    /// Bounding sphere radius in PS2 world units (mm).
    pub radius: f32,
    /// Vertical offset in PS2 Y-down world units (mm) from the `.dds` root position to the sphere center.
    /// Note: `.dds` `char_positions` are already authored at the character's pelvis/torso elevation
    /// (`y ≈ -550` mm for standing adults, `-309` mm for sitting Eddie, `-75` mm for lying Angela),
    /// so `center_y_offset` is `0.0`.
    pub center_y_offset: f32,
}

impl ActorBoundingSphere {
    /// Returns the conservative world-space bounding sphere for a `.dds` actor slot.
    pub fn for_actor(actor_name: &str, anim_info: Option<&DramaDemoAnimInfo>) -> Self {
        let clean = actor_name.trim_matches('\0').trim();
        if clean.eq_ignore_ascii_case("red_pos")
            || clean.eq_ignore_ascii_case("pap_pos")
            || clean.eq_ignore_ascii_case("bos_pos")
        {
            // Tall boss / Pyramid Head / Abstract Daddy / Mary Boss
            return Self {
                radius: DEFAULT_BOSS_BOUNDING_RADIUS,
                center_y_offset: 0.0,
            };
        }
        if clean.eq_ignore_ascii_case("boat_pos") {
            return Self {
                radius: 750.0,
                center_y_offset: 0.0,
            };
        }
        if let Some(info) = anim_info {
            match info.kind_id {
                // Humanoid protagonists, NPCs, and monsters (`0x0100..=0x0209`)
                0x0100..=0x0209 => Self {
                    radius: DEFAULT_HUMANOID_BOUNDING_RADIUS,
                    center_y_offset: 0.0,
                },
                // Doors, furniture, and large room props (`b_*`)
                _ if clean.starts_with("b_") || clean.starts_with("B_") => Self {
                    radius: DEFAULT_PROP_BOUNDING_RADIUS,
                    center_y_offset: 0.0,
                },
                // Hand-held items (`i_*`)
                _ => Self {
                    radius: DEFAULT_ITEM_BOUNDING_RADIUS,
                    center_y_offset: 0.0,
                },
            }
        } else if clean.starts_with("i_") || clean.starts_with("I_") {
            Self {
                radius: DEFAULT_ITEM_BOUNDING_RADIUS,
                center_y_offset: 0.0,
            }
        } else {
            Self {
                radius: DEFAULT_HUMANOID_BOUNDING_RADIUS,
                center_y_offset: 0.0,
            }
        }
    }

    /// Computes the world-space center of this bounding sphere given the actor's `.dds` root `position`.
    #[inline]
    pub fn world_center(&self, root_position: Vec3) -> Vec3 {
        Vec3::new(
            root_position.x,
            root_position.y + self.center_y_offset,
            root_position.z,
        )
    }
}

/// Computes the orthonormal camera basis `(right, up, forward)` in PS2 world space (Y-down),
/// matching [`super::CutsceneCameraSnapshot::view_matrix`].
#[inline]
pub fn camera_basis_vectors(eye: Vec3, interest: Vec3, roll_radians: f32) -> (Vec3, Vec3, Vec3) {
    let forward_raw = (interest - eye).normalize_or_zero();
    let forward = if forward_raw.length_squared() > 1e-6 {
        forward_raw
    } else {
        Vec3::Z
    };
    let base_up = Vec3::new(0.0, -1.0, 0.0);
    let rot_roll = Mat4::from_axis_angle(forward, roll_radians);
    let rolled_up = rot_roll.transform_vector3(base_up).normalize_or_zero();
    let up_hint = if rolled_up.length_squared() > 1e-6 {
        rolled_up
    } else {
        base_up
    };
    let right_raw = forward.cross(up_hint).normalize_or_zero();
    let right = if right_raw.length_squared() > 1e-6 {
        right_raw
    } else {
        Vec3::X
    };
    let up = right.cross(forward).normalize_or_zero();
    (right, up, forward)
}

/// Returns `true` if a world-space bounding sphere `(sphere_center, sphere_radius)` intersects or
/// lies inside the authored `4:3` horizontal camera frustum cone defined by
/// `(eye, interest, roll_radians, fov_y_4x3, fov_x_4x3)`.
///
/// # Why Only Horizontal `4:3` Planes Are Tested
/// Widescreen (`16:9`) and ultrawide (`21:9` / `32:9`) Hor+ projection ONLY widens the **horizontal**
/// field of view (`fov_x > fov_x_4x3`), while preserving the exact `4:3` vertical field of view (`fov_y`).
/// Furthermore, in close-up or over-the-shoulder shots (`gero_edi`, `knife_agl`, `bp_edi_2`, `papa_agl_a`,
/// `papa_agl`, `killer_edi`, `piano_lau_e`, `kokuchi`, `mry_yarinaoshi_a`), the camera is placed right next
/// to an actor's head or shoulder (`|rel| <= 850` mm), where the pelvis root is below or slightly behind
/// the camera eye while the upper body and head are visible in frame.
#[inline]
pub fn is_sphere_inside_4x3_frustum(
    eye: Vec3,
    interest: Vec3,
    roll_radians: f32,
    _fov_y_4x3: f32,
    fov_x_4x3: f32,
    sphere_center: Vec3,
    sphere_radius: f32,
) -> bool {
    let (right, _up, forward) = camera_basis_vectors(eye, interest, roll_radians);
    let rel = sphere_center - eye;
    let z_cam = rel.dot(forward);

    // Never cull immediate foreground / close-up / over-the-shoulder actors within 1,250 mm of the lens,
    // whose hands, knees, shoulders, or head are in-frame even when their pelvis root is off-axis.
    if rel.length_squared() <= 1250.0 * 1250.0 && z_cam >= -550.0 {
        return true;
    }

    if z_cam + sphere_radius <= 0.0 {
        return false;
    }

    let x_cam = rel.dot(right).abs();
    let half_x = (fov_x_4x3 * 0.5).clamp(0.01, std::f32::consts::FRAC_PI_2 - 0.01);
    let (sin_x, cos_x) = half_x.sin_cos();

    // Signed distance outside the left/right 4:3 horizontal frustum planes:
    let eff_z = z_cam.max(0.0);
    let dist_outside_x = x_cam * cos_x - eff_z * sin_x;
    dist_outside_x <= sphere_radius
}

/// Evaluates whether a `.dds` cutscene actor should be culled on a widescreen (`16:9`) or ultrawide
/// (`21:9` / `32:9`) viewport because it is parked outside the authored `4:3` camera frustum (`SH2-77`).
///
/// Returns `true` when the actor should be hidden (`actor.visible = false`, `actor.culled_offscreen_4x3 = true`).
#[allow(clippy::too_many_arguments)]
pub fn should_cull_parked_offscreen_actor(
    widescreen_mode: CutsceneWidescreenMode,
    viewport_aspect: f32,
    script_visible: bool,
    eye: Vec3,
    interest: Vec3,
    roll_radians: f32,
    fov_y_4x3: f32,
    fov_x_4x3: f32,
    actor_pos: Vec3,
    last_key_pos: Vec3,
    next_key_pos: Vec3,
    bounding_sphere: ActorBoundingSphere,
) -> bool {
    if !script_visible {
        return false;
    }
    if !widescreen_mode.uses_smart_offscreen_culling()
        || viewport_aspect <= AUTHORED_4X3_ASPECT + 1e-3
    {
        return false;
    }

    let sphere_center = bounding_sphere.world_center(actor_pos);
    if is_sphere_inside_4x3_frustum(
        eye,
        interest,
        roll_radians,
        fov_y_4x3,
        fov_x_4x3,
        sphere_center,
        bounding_sphere.radius,
    ) {
        return false;
    }

    // Actor is outside the authored 4:3 frustum cone (in the widescreen/ultrawide side margins or behind camera).
    // Check whether the actor is stationary ("parked") or moving across adjacent keyframes:
    let delta = next_key_pos - last_key_pos;
    let speed_per_frame = delta.length();
    if speed_per_frame <= PARKED_ACTOR_VELOCITY_EPSILON {
        return true;
    }

    // If the actor is moving (`speed_per_frame > 1.5` mm/frame), check if it is actively walking
    // toward the 4:3 camera cone and within a smooth entry band (`2.25 * radius`) of the 4:3 edge.
    // Ignore instantaneous large teleports (`speed_per_frame > 400.0` mm/frame) where `.dds`
    // repositions a hidden/parked actor between shots.
    if speed_per_frame > 400.0 {
        return true;
    }

    let (right, _up, forward) = camera_basis_vectors(eye, interest, roll_radians);
    let last_rel = bounding_sphere.world_center(last_key_pos) - eye;
    let next_rel = bounding_sphere.world_center(next_key_pos) - eye;
    let z_cam = (sphere_center - eye).dot(forward);
    if z_cam <= 0.0 {
        return true;
    }

    let last_abs_x = last_rel.dot(right).abs();
    let next_abs_x = next_rel.dot(right).abs();
    let moving_toward_center = next_abs_x + 0.1 < last_abs_x;

    if moving_toward_center {
        let half_x = (fov_x_4x3 * 0.5).clamp(0.01, std::f32::consts::FRAC_PI_2 - 0.01);
        let (sin_x, cos_x) = half_x.sin_cos();
        let x_cam = (sphere_center - eye).dot(right).abs();
        let dist_outside_x = x_cam * cos_x - z_cam * sin_x;
        // Allow actors actively walking into the 4:3 frame within 2.25x radius of the 4:3 border to stay visible
        if dist_outside_x <= bounding_sphere.radius * 2.25 {
            return false;
        }
    }

    true
}
