//! Authentic PS2 Grapple Coupling, Relative Anchoring, and Struggle State Machines.
//!
//! Ground truth decompiled from `prototype_extracted/SLUS_202.28`:
//! - `PlayerCheckHuggedPos` (`0x0021bdc0`): Mathematical coupling matrix transforming creature coordinates
//!   relative to James based on character yaw.
//! - `lower_damage` (`0x0021f700`): Gripping damage-over-time and vector selection table (`0x0034c200`).
//! - `PlayerCheckHuggingAttack` (`0x00110070`): Breakout resistance timer table `hurihodoki_timer[5][5]` (`0x002a8e10`).
//! - `PlayerCheckAnimeLower` Case 17 (`0x0010e4d8`): Selects James struggle animations:
//!   - Torso struggle: `aid 128` (frames `612..=629 @ 15.0fps`)
//!   - Choke struggle: `aid 129` (frames `632..=649 @ 15.0fps`)
//!   - Recoil breakout: `aid 130` (frames `652..=670 @ 15.0fps`)
//!
//! # Pure Data Ingestion Note:
//! This open-source engine does NOT contain, rip, or bundle any proprietary Konami
//! meshes, textures, or audio assets. All animation parameters represent purely
//! skeletal keyframe indices and playback frame rates that direct the runtime animation
//! system to ingest data from user-supplied authentic retail game files.

pub mod anchor;
pub mod session;
pub mod tables;

pub use anchor::{compute_creature_grapple_transform, compute_grapple_coupling, GrappleAnchor};
pub use session::{GrapplePhase, GrappleSession};
pub use tables::{
    get_grapple_spec, BestiaryId, GrappleFacingMode, GrappleSpec, PS2_HUGGING_POS,
    PS2_HURIHODOKI_TIMER, SPEC_ABSTRACT_DADDY, SPEC_CREEPER, SPEC_FLESH_LIPS, SPEC_MARY_BOSS,
    SPEC_PYRAMID_HEAD,
};

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;

    #[test]
    fn test_ps2_hugging_pos_constants() {
        // Verify exact extracted values from SLUS_202.28 @ 0x0034c200
        assert!((PS2_HUGGING_POS[0][0] - 52.1332).abs() < 1e-4);
        assert!((PS2_HUGGING_POS[0][1] - 1550.9032).abs() < 1e-4);
        assert!((PS2_HUGGING_POS[0][2] - 36.4947).abs() < 1e-4);

        assert!((PS2_HUGGING_POS[1][0] - 43.5385).abs() < 1e-4);
        assert!((PS2_HUGGING_POS[1][2] - 353.9123).abs() < 1e-4);
    }

    #[test]
    fn test_ps2_hurihodoki_timer_difficulty_scaling() {
        // Normal difficulty (row index 2)
        assert_eq!(SPEC_PYRAMID_HEAD.breakout_duration(2), 0.70);
        assert_eq!(SPEC_FLESH_LIPS.breakout_duration(2), 0.80);
        assert_eq!(SPEC_ABSTRACT_DADDY.breakout_duration(2), 1.00);
        assert_eq!(SPEC_MARY_BOSS.breakout_duration(2), 1.30);

        // Hard difficulty (row index 3)
        assert_eq!(SPEC_PYRAMID_HEAD.breakout_duration(3), 1.40);
        assert_eq!(SPEC_FLESH_LIPS.breakout_duration(3), 1.60);
        assert_eq!(SPEC_ABSTRACT_DADDY.breakout_duration(3), 2.00);
        assert_eq!(SPEC_MARY_BOSS.breakout_duration(3), 2.60);
    }

    #[test]
    fn test_grapple_anchor_world_transformation() {
        let mary_anchor = GrappleAnchor::for_species(BestiaryId::MaryBoss);
        let player_pos = Vec3::ZERO;
        let player_yaw = 0.0; // Facing +Z

        let anchor = mary_anchor.compute_world_anchor(player_pos, player_yaw);
        // Facing +Z means forward = (0, 0, 1), right = (1, 0, 0)
        assert!((anchor.z - 353.9123).abs() < 1e-3);
        assert!((anchor.x - 43.5385).abs() < 1e-3);
    }

    #[test]
    fn test_ps2_grapple_struggle_clips_parity() {
        // 1:1 PS2 PlayerCheckAnimeUpper & PlayerCheckAnimeLower Case 17:
        // Choke holds (Flesh Lips overhead and Mary boss choke) map to pjames_anim aid 129 (632..=649 @ 15.0fps)
        let ike_spec = get_grapple_spec(BestiaryId::FleshLips).unwrap();
        assert_eq!(ike_spec.james_struggle_clip.start_frame, 632);
        assert_eq!(ike_spec.james_struggle_clip.end_frame, 649);
        assert_eq!(ike_spec.james_struggle_clip.fps, 15.0);
        assert!(ike_spec.james_struggle_clip.looping);

        let mary_spec = get_grapple_spec(BestiaryId::MaryBoss).unwrap();
        assert_eq!(mary_spec.james_struggle_clip.start_frame, 632);
        assert_eq!(mary_spec.james_struggle_clip.end_frame, 649);

        // Torso grabs (Daddy) map to pjames_anim aid 128 (612..=629 @ 15.0fps)
        let daddy_spec = get_grapple_spec(BestiaryId::AbstractDaddy).unwrap();
        assert_eq!(daddy_spec.james_struggle_clip.start_frame, 612);
        assert_eq!(daddy_spec.james_struggle_clip.end_frame, 629);

        // Breakout release push is aid 130 (652..=670 @ 15.0fps)
        assert_eq!(ike_spec.james_breakout_clip.start_frame, 652);
        assert_eq!(ike_spec.james_breakout_clip.end_frame, 670);
    }
}
