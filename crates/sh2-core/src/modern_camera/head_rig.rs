//! Retail skeletal head-rig specs, natural neck-tilt eye arc, and the chest-clear neck-opening
//! envelope for the First-Person camera (`SH2-93`).
//!
//! # Coordinates
//! Model space is `+X` right, `+Y` **down**, `+Z` forward (SH2 world units, `500 units = 1 m`).
//! Pitch `> 0` looks down (`+Y`).
//!
//! # Why an envelope?
//! In `FirstPerson` only the cranial head subtree is hidden, so the visible mesh ends in an open
//! neck stump ring just below the head joint. The steepest ray of the view frustum (bottom edge,
//! depression `pitch + fov_y / 2`, which exceeds `90 deg` at max pitch) must pass in **front** of
//! that ring's front lip, otherwise the player sees down into the hollow torso. Because every ray
//! on the bottom frustum edge shares the same forward-per-drop slope (`cot(depression)`), checking
//! the central column against the ring's front lip is sufficient for the whole edge.

use glam::Vec3;

use crate::scenario::PlayableCharacter;

/// Fraction of downward look pitch applied as forward head nod about the head joint (`0.45`),
/// so looking at your chest/feet tilts the chin forward over the jacket like a real neck.
pub const NECK_TILT_SHARE: f32 = 0.45;

/// Extra forward clearance in SH2 units (`12.0` = `2.4 cm`) kept between the steepest frustum ray
/// and the neck-opening front lip.
pub const NECK_OPENING_CLEAR_MARGIN_UNITS: f32 = 12.0;

/// Ground-truth skeletal head rig specification extracted from retail `.mdl` bind poses (`SH2-8`/`SH2-12`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CharacterHeadRigSpec {
    /// Skeleton head bone index (`15` for James, `9` for Maria).
    pub head_bone_index: usize,
    /// Left eye bone index (`20` for James, `23` for high-LOD Maria).
    pub left_eye_bone_index: usize,
    /// Right eye bone index (`22` for James, `25` for high-LOD Maria).
    pub right_eye_bone_index: usize,
    /// Minimum skeleton `bone_count` required for `left_eye_bone_index`/`right_eye_bone_index` to be valid eye bones
    /// (`41` on `lll_jms`/`hll_jms`/`hhh_jms`; `73` on `hhh_mar`, since 36-bone `lll_mar` reuses `23`/`25` for arms).
    pub min_eye_rig_bones: usize,
    /// Eye midpoint offset `[x, y, z]` in head-bone local coordinates (SH2 units, `+Y` down, `+Z` forward).
    pub head_local_eye_offset: Vec3,
    /// Fallback model-space eye `(x, y, z)` when skeletal pose matrices have not been supplied (`+Y` down).
    pub fallback_model_eye: Vec3,
    /// Fallback model-space head joint (bind translation of `head_bone_index`, `+Y` down).
    pub fallback_model_head: Vec3,
    /// Horizontal radius (SH2 units) from the head joint to the front lip of the visible neck stump
    /// once cranial triangles are hidden (measured on retail bind poses).
    pub neck_opening_radius: f32,
    /// Vertical drop (SH2 units, `+Y` down) from the head joint to the neck stump's front lip.
    pub neck_opening_drop: f32,
}

impl CharacterHeadRigSpec {
    /// Ground-truth rig spec for James Sunderland (`lll_jms.mdl` / `hll_jms.mdl` 41 bones & `hhh_jms.mdl` 71 bones).
    /// Head bone `15` sits at `(0.0, -809.1, -26.6)`; eyes `20`/`22` at `(±15.9, -844.6, +23.4)`. With
    /// cranial triangles hidden the neck stump tops out at `Y = -809.5`, its ring spans model
    /// `Z ∈ [-62.0, +8.3]` (`≈ ±35` about the head joint) with the front lip at `Y ≈ -800`.
    pub const JAMES: Self = Self {
        head_bone_index: 15,
        left_eye_bone_index: 20,
        right_eye_bone_index: 22,
        min_eye_rig_bones: 41,
        head_local_eye_offset: Vec3::new(38.1311, 0.0, 47.9967),
        fallback_model_eye: Vec3::new(0.0, -844.6, 23.4),
        fallback_model_head: Vec3::new(0.0, -809.1, -26.6),
        neck_opening_radius: 35.0,
        neck_opening_drop: 10.0,
    };

    /// Ground-truth rig spec for Maria (`lll_mar.mdl` 36 bones & `hhh_mar.mdl` 73 bones).
    /// Head bone `9` sits at `(0.0, -794.4, -15.6)` (`M_init[9]^-1 * eye = (45.3310, 0.0019, 34.3863)`);
    /// eyes `23`/`25` in `hhh_mar` at `(±15.4, -825.6, +32.0)`. With cranial triangles hidden the neck
    /// stump tops out at `Y = -774.9` with ring `Z ∈ [-43.5, +11.1]` (`≈ ±28` about the head joint).
    pub const MARIA: Self = Self {
        head_bone_index: 9,
        left_eye_bone_index: 23,
        right_eye_bone_index: 25,
        min_eye_rig_bones: 73,
        head_local_eye_offset: Vec3::new(45.3310, 0.0019, 34.3863),
        fallback_model_eye: Vec3::new(0.0, -825.6, 32.0),
        fallback_model_head: Vec3::new(0.0, -794.4, -15.6),
        neck_opening_radius: 28.0,
        neck_opening_drop: 20.0,
    };

    /// Returns the skeletal head rig specification for `character`.
    #[inline]
    pub const fn for_character(character: PlayableCharacter) -> Self {
        match character {
            PlayableCharacter::James => Self::JAMES,
            PlayableCharacter::Maria => Self::MARIA,
        }
    }

    /// Evaluates the character's 3D world-space eye position in SH2 world units (`+Y` down)
    /// from evaluated row-major model-space bone matrices (`evaluate_pose_into`), or falls back
    /// cleanly to `fallback_model_eye` rotated by `(player_pos, player_yaw)`.
    pub fn resolve_world_eye_anchor(
        &self,
        player_pos: Vec3,
        player_yaw: f32,
        pose_matrices: Option<&[[[f32; 4]; 4]]>,
    ) -> Vec3 {
        let model_eye = match pose_matrices {
            Some(mats)
                if mats.len() >= self.min_eye_rig_bones
                    && self.left_eye_bone_index < mats.len()
                    && self.right_eye_bone_index < mats.len() =>
            {
                let ml = &mats[self.left_eye_bone_index];
                let mr = &mats[self.right_eye_bone_index];
                Vec3::new(
                    0.5 * (ml[0][3] + mr[0][3]),
                    0.5 * (ml[1][3] + mr[1][3]),
                    0.5 * (ml[2][3] + mr[2][3]),
                )
            }
            Some(mats) => match mats.get(self.head_bone_index) {
                Some(mh) => {
                    let off = self.head_local_eye_offset;
                    Vec3::new(
                        mh[0][0] * off.x + mh[0][1] * off.y + mh[0][2] * off.z + mh[0][3],
                        mh[1][0] * off.x + mh[1][1] * off.y + mh[1][2] * off.z + mh[1][3],
                        mh[2][0] * off.x + mh[2][1] * off.y + mh[2][2] * off.z + mh[2][3],
                    )
                }
                None => self.fallback_model_eye,
            },
            None => self.fallback_model_eye,
        };
        model_to_world(player_pos, player_yaw, model_eye)
    }

    /// Evaluates the world-space head joint (translation of `head_bone_index`) from evaluated
    /// model-space pose matrices, falling back to `fallback_model_head` (`+Y` down SH2 units).
    pub fn resolve_world_head_joint(
        &self,
        player_pos: Vec3,
        player_yaw: f32,
        pose_matrices: Option<&[[[f32; 4]; 4]]>,
    ) -> Vec3 {
        let model_head = pose_matrices
            .and_then(|mats| mats.get(self.head_bone_index))
            .map(|m| Vec3::new(m[0][3], m[1][3], m[2][3]))
            .unwrap_or(self.fallback_model_head);
        model_to_world(player_pos, player_yaw, model_head)
    }

    /// Model-space eye displacement (`+Z` forward, `+Y` down) produced by nodding the head forward
    /// about the head joint by `NECK_TILT_SHARE * look_pitch` (zero when looking level or up).
    pub fn neck_tilt_model_offset(&self, look_pitch: f32) -> Vec3 {
        let nod = look_pitch.max(0.0) * NECK_TILT_SHARE;
        let rel = self.fallback_model_eye - self.fallback_model_head;
        let (s, c) = nod.sin_cos();
        // Rotate (z forward, y down) toward +Z/+Y: forward' = z c - y s, down' = z s + y c.
        let rotated = Vec3::new(rel.x, rel.z * s + rel.y * c, rel.z * c - rel.y * s);
        rotated - rel
    }

    /// Minimum horizontal distance (SH2 units, along the horizontal view direction) from the head
    /// joint that the eye must keep so the bottom frustum edge at `depression_rad` (`pitch + fov_y/2`)
    /// passes in front of the neck opening's front lip lying `eye_drop_to_lip` units below the eye.
    pub fn min_clear_forward(&self, eye_drop_to_lip: f32, depression_rad: f32) -> f32 {
        let drop = eye_drop_to_lip.max(0.0);
        let (s, c) = depression_rad.sin_cos();
        let cot = c / s.max(1e-3);
        self.neck_opening_radius + NECK_OPENING_CLEAR_MARGIN_UNITS - drop * cot
    }

    /// Maximum look pitch in radians for which the bottom frustum edge (`pitch + half_fov_y`) still
    /// clears the neck opening, given the eye's horizontal `eye_forward` from the head joint and its
    /// height `eye_drop_to_lip` above the lip. Returns `f32::INFINITY` when the lip is not below the eye.
    pub fn max_clear_pitch(&self, eye_forward: f32, eye_drop_to_lip: f32, half_fov_y: f32) -> f32 {
        if eye_drop_to_lip <= 1e-3 {
            return f32::INFINITY;
        }
        let available = eye_forward - (self.neck_opening_radius + NECK_OPENING_CLEAR_MARGIN_UNITS);
        std::f32::consts::FRAC_PI_2 + available.atan2(eye_drop_to_lip) - half_fov_y
    }

    /// World-space `Y` of the neck opening's front lip for a world-space `head_joint`.
    #[inline]
    pub fn neck_lip_world_y(&self, head_joint: Vec3) -> f32 {
        head_joint.y + self.neck_opening_drop
    }
}

/// Rotates model-space `v` by body `yaw` (radians, `0.0` = `+Z`) and translates by `pos`.
#[inline]
pub fn model_to_world(pos: Vec3, yaw: f32, v: Vec3) -> Vec3 {
    let (s, c) = yaw.sin_cos();
    Vec3::new(
        c * v.x + s * v.z + pos.x,
        v.y + pos.y,
        -s * v.x + c * v.z + pos.z,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neck_tilt_moves_eye_forward_and_down_only_when_looking_down() {
        let spec = CharacterHeadRigSpec::JAMES;
        assert_eq!(spec.neck_tilt_model_offset(0.0), Vec3::ZERO);
        assert_eq!(spec.neck_tilt_model_offset(-0.8), Vec3::ZERO);
        let d = spec.neck_tilt_model_offset(1.10);
        assert!(d.z > 8.0 && d.z < 15.0, "forward nod {d:?}");
        assert!(d.y > 15.0 && d.y < 35.0, "downward nod {d:?}");
        assert!(d.x.abs() < 1e-4);
    }

    #[test]
    fn envelope_is_monotonic_and_consistent_with_pitch_cap() {
        let spec = CharacterHeadRigSpec::MARIA;
        let half_fov = 37.5_f32.to_radians();
        // Steeper bottom-edge depression demands more forward clearance once past vertical.
        let shallow = spec.min_clear_forward(30.0, 1.2);
        let steep = spec.min_clear_forward(30.0, 1.9);
        assert!(steep > shallow);
        // An eye placed exactly on the envelope yields a pitch cap equal to the probing pitch.
        let pitch = 1.1;
        let need = spec.min_clear_forward(30.0, pitch + half_fov);
        let cap = spec.max_clear_pitch(need, 30.0, half_fov);
        assert!((cap - pitch).abs() < 1e-3, "cap {cap} vs {pitch}");
        assert_eq!(spec.max_clear_pitch(0.0, -5.0, half_fov), f32::INFINITY);
    }

    #[test]
    fn model_to_world_matches_body_yaw_convention() {
        let w = model_to_world(
            Vec3::new(10.0, 0.0, 20.0),
            std::f32::consts::FRAC_PI_2,
            Vec3::Z,
        );
        assert!((w - Vec3::new(11.0, 0.0, 20.0)).length() < 1e-5);
    }
}
