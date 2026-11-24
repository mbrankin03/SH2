//! Skeletal weapon socket attachment and enemy weapon visibility helpers (`SH2-35`).
//!
//! Ground-truth references:
//! - `SLUS_202.28` `Chacter/m3_red.c` `shCharacterSetWeaponRED` (`0x0011caf0`):
//!   toggles `pose_index = 3` (Great Knife) vs `0` on `red.mdl` (`0x208`).
//! - `SLUS_202.28` `Chacter/m3_wep.c` `shUpdateWeaponMatrixAfterAnime` (`0x00120120`):
//!   binds external weapon `.mdl` root transform to character bone `0x23` (`35`).

use sh2_assets::formats::anm::mat4_mul;
pub use sh2_assets::formats::enemy_rig::{
    enemy_sockets_for, evaluate_socket_world, EnemyModelKey, EnemyModelSpec, EvaluatedSocket,
    SkeletalSocketKind, WeaponSocketSpec, ENEMY_MODEL_SPECS,
};

use super::mesh::GpuSkeletalMesh;

/// Toggles visibility of an enemy's equipped weapon sub-mesh (`pose_index`) on `mesh`.
///
/// For Pyramid Head (`EnemyModelKey::Red`), toggles `pose_index = 3` (Great Knife) matching
/// `SLUS_202.28` `shCharacterSetWeaponRED` (`0x0011caf0`). For species whose weapon is baked into
/// `pose_index = 0` (`nse`, `xoo`, `oni`, `edi`), this is a no-op.
pub fn set_enemy_weapon_visible(mesh: &mut GpuSkeletalMesh, key: EnemyModelKey, equipped: bool) {
    if let Some(pose_idx) = key.spec().weapon_pose_index {
        mesh.set_pose_visible(pose_idx, equipped);
    }
}

/// Computes the world-space skinning matrix for an external `.mdl` prop/weapon attached to
/// `socket.bone_index` (e.g., James's `wp_*.mdl` on bone `0x23` (`35`) in
/// `shUpdateWeaponMatrixAfterAnime` `0x00120120`).
#[must_use]
pub fn compute_attachment_skin_matrix(
    entity_world: [[f32; 4]; 4],
    bone_poses: &[[[f32; 4]; 4]],
    socket: &WeaponSocketSpec,
) -> Option<[[f32; 4]; 4]> {
    let bone_pose = *bone_poses.get(socket.bone_index)?;
    let mut world = mat4_mul(entity_world, bone_pose);
    if socket.local_origin != [0.0, 0.0, 0.0] {
        let [ox, oy, oz] = socket.local_origin;
        world[0][3] += world[0][0] * ox + world[0][1] * oy + world[0][2] * oz;
        world[1][3] += world[1][0] * ox + world[1][1] * oy + world[1][2] * oz;
        world[2][3] += world[2][0] * ox + world[2][1] * oy + world[2][2] * oz;
    }
    Some(world)
}

#[cfg(test)]
mod tests {
    use super::*;

    const IDENTITY: [[f32; 4]; 4] = [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];

    #[test]
    fn attachment_skin_matrix_applies_bone_and_entity_translation() {
        let mut bone_poses = vec![IDENTITY; 36];
        bone_poses[35][1][3] = -900.0;
        let mut entity_world = IDENTITY;
        entity_world[0][3] = 250.0;
        entity_world[2][3] = -400.0;

        let mat = compute_attachment_skin_matrix(
            entity_world,
            &bone_poses,
            &SkeletalSocketKind::JamesRightHand.spec(),
        )
        .expect("bone 35 in bounds");

        assert!((mat[0][3] - 250.0).abs() < 1e-5);
        assert!((mat[1][3] - (-900.0)).abs() < 1e-5);
        assert!((mat[2][3] - (-400.0)).abs() < 1e-5);
    }
}
