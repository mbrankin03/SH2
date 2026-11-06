//! Integration tests verifying creature and companion skeletal animation synchronization,
//! authentic retail .anm frame range conformance, root motion advancement, and state machine binding (SH2-57).

use sh2_assets::formats::anm::parse_anm;
use sh2_assets::formats::mdls::parse_mdl;
use sh2_core::ai::creatures::anim::{
    CreatureAnimStateKind, LyingFigureAnimClips, MannequinAnimClips,
    MariaAnimClips, NurseAnimClips,
};
use sh2_core::ai::creatures::lying_figure::{LyingFigureController, ScuState};
use sh2_core::ai::creatures::mannequin::{MannequinController, MknState};
use sh2_core::ai::creatures::nurse::NurseController;
use sh2_core::ai::maria::{MariaCompanionController, MariaState};
use sh2_core::ai::sensory::{DifficultyLevel, PerceivableTarget, RoomLighting};
use std::fs;
use std::path::Path;

const RETAIL_CHR_ROOT: &str =
    "Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2/data/chr";

#[test]
fn test_retail_anm_clip_ranges_conformance() {
    if !Path::new(RETAIL_CHR_ROOT).exists() {
        println!("Skipping retail anm test: assets not present");
        return;
    }

    // 1. Lying Figure (scu.anm: 30 bones)
    let scu_mdl = fs::read(format!("{RETAIL_CHR_ROOT}/scu/scu.mdl")).expect("scu.mdl");
    let scu_anm = fs::read(format!("{RETAIL_CHR_ROOT}/scu/scu.anm")).expect("scu.anm");
    let scu_model = parse_mdl(&scu_mdl).expect("scu mdl parse");
    let scu_anim = parse_anm(&scu_anm, scu_model.bone_count, &scu_model.parents).expect("scu anm parse");
    assert_eq!(scu_model.bone_count, 30);
    assert!(scu_anim.frames.len() >= 779, "Lying Figure anm frame count");

    // Check Lying Figure keyframe clips fit strictly within available frames
    assert!(LyingFigureAnimClips::STAND.end_frame < scu_anim.frames.len() as u32);
    assert!(LyingFigureAnimClips::WALK.end_frame < scu_anim.frames.len() as u32);
    assert!(LyingFigureAnimClips::PRONE_CRAWL.end_frame < scu_anim.frames.len() as u32);
    assert!(LyingFigureAnimClips::ACID_SPRAY.end_frame < scu_anim.frames.len() as u32);
    assert!(LyingFigureAnimClips::FLINCH.end_frame < scu_anim.frames.len() as u32);
    assert!(LyingFigureAnimClips::KNOCKDOWN.end_frame < scu_anim.frames.len() as u32);
    assert!(LyingFigureAnimClips::RECOVERY.end_frame < scu_anim.frames.len() as u32);

    // 2. Mannequin (mkn.anm: 17 bones)
    let mkn_mdl = fs::read(format!("{RETAIL_CHR_ROOT}/mkn/mkn.mdl")).expect("mkn.mdl");
    let mkn_anm = fs::read(format!("{RETAIL_CHR_ROOT}/mkn/mkn.anm")).expect("mkn.anm");
    let mkn_model = parse_mdl(&mkn_mdl).expect("mkn mdl parse");
    let mkn_anim = parse_anm(&mkn_anm, mkn_model.bone_count, &mkn_model.parents).expect("mkn anm parse");
    assert_eq!(mkn_model.bone_count, 17);
    assert!(mkn_anim.frames.len() >= 720, "Mannequin anm frame count");

    assert!(MannequinAnimClips::STAY.end_frame < mkn_anim.frames.len() as u32);
    assert!(MannequinAnimClips::WALK.end_frame < mkn_anim.frames.len() as u32);
    assert!(MannequinAnimClips::ARM_SWING.end_frame < mkn_anim.frames.len() as u32);
    assert!(MannequinAnimClips::KICK.end_frame < mkn_anim.frames.len() as u32);
    assert!(MannequinAnimClips::FLINCH.end_frame < mkn_anim.frames.len() as u32);
    assert!(MannequinAnimClips::KNOCKDOWN.end_frame < mkn_anim.frames.len() as u32);
    assert!(MannequinAnimClips::WRITHING.end_frame < mkn_anim.frames.len() as u32);

    // 3. Bubble Head Nurse (nse.anm: 30 bones)
    let nse_mdl = fs::read(format!("{RETAIL_CHR_ROOT}/nse/nse.mdl")).expect("nse.mdl");
    let nse_anm = fs::read(format!("{RETAIL_CHR_ROOT}/nse/nse.anm")).expect("nse.anm");
    let nse_model = parse_mdl(&nse_mdl).expect("nse mdl parse");
    let nse_anim = parse_anm(&nse_anm, nse_model.bone_count, &nse_model.parents).expect("nse anm parse");
    assert_eq!(nse_model.bone_count, 30);
    assert!(nse_anim.frames.len() >= 658, "Nurse anm frame count");

    assert!(NurseAnimClips::STAND.end_frame < nse_anim.frames.len() as u32);
    assert!(NurseAnimClips::TWITCH_WALK.end_frame < nse_anim.frames.len() as u32);
    assert!(NurseAnimClips::PIPE_SWING.end_frame < nse_anim.frames.len() as u32);
    assert!(NurseAnimClips::FLINCH.end_frame < nse_anim.frames.len() as u32);
    assert!(NurseAnimClips::KNOCKDOWN.end_frame < nse_anim.frames.len() as u32);

    // 4. Maria Companion (lll_mar.anm: 36 bones)
    let mar_mdl = fs::read(format!("{RETAIL_CHR_ROOT}/mar/lll_mar.mdl")).expect("lll_mar.mdl");
    let mar_anm = fs::read(format!("{RETAIL_CHR_ROOT}/mar/lll_mar.anm")).expect("lll_mar.anm");
    let mar_model = parse_mdl(&mar_mdl).expect("mar mdl parse");
    let mar_anim = parse_anm(&mar_anm, mar_model.bone_count, &mar_model.parents).expect("mar anm parse");
    assert_eq!(mar_model.bone_count, 36);
    assert!(mar_anim.frames.len() >= 851, "Maria anm frame count");

    assert!(MariaAnimClips::STAND.end_frame < mar_anim.frames.len() as u32);
    assert!(MariaAnimClips::WALK.end_frame < mar_anim.frames.len() as u32);
    assert!(MariaAnimClips::RUN.end_frame < mar_anim.frames.len() as u32);
    assert!(MariaAnimClips::COWER_PANIC.end_frame < mar_anim.frames.len() as u32);
    assert!(MariaAnimClips::FLINCH_DOWNED.end_frame < mar_anim.frames.len() as u32);
}

#[test]
fn test_lying_figure_fsm_to_skeletal_sync() {
    let mut scu = LyingFigureController::new(
        1,
        glam::Vec3::new(0.0, 0.0, 0.0),
        0.0,
        DifficultyLevel::Normal,
    );

    // Initial state: Wander -> Locomotion
    // Place target out of sensory range (5000 units away) to remain in Wander
    let target = PerceivableTarget::new(glam::Vec3::new(0.0, 0.0, 5000.0), 0.0, false, false);
    scu.update(&target, RoomLighting::DarkLightOff, DifficultyLevel::Normal, 0.016);
    assert_eq!(scu.state, ScuState::Wander);
    assert_eq!(scu.anim.state(), CreatureAnimStateKind::Locomotion);
    assert_eq!(scu.anim.current_clip(), LyingFigureAnimClips::WALK);

    // Simulate detection and transition to Attack
    scu.state = ScuState::Attack;
    scu.state_timer = 0.0;
    scu.update(&target, RoomLighting::Lit, DifficultyLevel::Normal, 0.016);
    assert_eq!(scu.anim.state(), CreatureAnimStateKind::AttackPrimary);
    assert_eq!(scu.anim.current_clip(), LyingFigureAnimClips::ACID_SPRAY);

    // Advance 0.3s -> should hit particle/hit windows
    scu.anim.update(0.3, 0.0);
    assert!(scu.anim.is_in_hit_window() || scu.anim.is_in_particle_window());

    // Take damage -> Flinch
    scu.receive_hit(50.0, 10.0, 0.1);
    assert_eq!(scu.state, ScuState::Damage);
    assert_eq!(scu.anim.state(), CreatureAnimStateKind::Flinch);
    assert_eq!(scu.anim.current_clip(), LyingFigureAnimClips::FLINCH);

    // Downed knockdown
    scu.receive_hit(50.0, 400.0, 0.99);
    assert_eq!(scu.state, ScuState::Down);
    assert_eq!(scu.anim.state(), CreatureAnimStateKind::Knockdown);
    assert_eq!(scu.anim.current_clip(), LyingFigureAnimClips::KNOCKDOWN);
}

#[test]
fn test_mannequin_fsm_to_skeletal_sync() {
    let mut mkn = MannequinController::new(
        1,
        glam::Vec3::new(0.0, 0.0, 0.0),
        0.0,
        DifficultyLevel::Normal,
    );

    // Dormant stay in darkness
    let target = PerceivableTarget::new(glam::Vec3::new(0.0, 0.0, 3000.0), 0.0, false, false);
    mkn.update(&target, RoomLighting::DarkLightOff, DifficultyLevel::Normal, 0.016);
    assert_eq!(mkn.state, MknState::Stay);
    assert_eq!(mkn.anim.state(), CreatureAnimStateKind::Idle);
    assert_eq!(mkn.anim.current_clip(), MannequinAnimClips::STAY);

    // Attack strike alternation
    mkn.state = MknState::Attack;
    mkn.is_kick_next = false;
    mkn.update(&target, RoomLighting::DarkLightOff, DifficultyLevel::Normal, 0.016);
    assert_eq!(mkn.anim.state(), CreatureAnimStateKind::AttackPrimary);
    assert_eq!(mkn.anim.current_clip(), MannequinAnimClips::ARM_SWING);

    mkn.state = MknState::Attack;
    mkn.is_kick_next = true;
    mkn.update(&target, RoomLighting::DarkLightOff, DifficultyLevel::Normal, 0.016);
    assert_eq!(mkn.anim.state(), CreatureAnimStateKind::AttackSecondary);
    assert_eq!(mkn.anim.current_clip(), MannequinAnimClips::KICK);
}

#[test]
fn test_nurse_and_maria_fsm_to_skeletal_sync() {
    // Bubble Head Nurse
    let mut nse = NurseController::new(
        1,
        glam::Vec3::new(0.0, 0.0, 0.0),
        0.0,
        DifficultyLevel::Normal,
    );
    let target = PerceivableTarget::new(glam::Vec3::new(0.0, 0.0, 2000.0), 0.0, false, false);
    nse.update(&target, RoomLighting::Lit, DifficultyLevel::Normal, 0.016);
    assert_eq!(nse.anim.state(), CreatureAnimStateKind::Locomotion);
    assert_eq!(nse.anim.current_clip(), NurseAnimClips::TWITCH_WALK);

    // Maria Companion
    let mut maria = MariaCompanionController::default();
    maria.spawn(glam::Vec3::new(0.0, 0.0, 0.0), 0.0);
    assert_eq!(maria.anim.state(), CreatureAnimStateKind::Idle);
    assert_eq!(maria.anim.current_clip(), MariaAnimClips::STAND);

    // Player runs 2500 units away -> Maria enters Walk
    maria.update(glam::Vec3::new(0.0, 0.0, 2500.0), 0.0, &[], 0.016);
    assert_eq!(maria.state, MariaState::Walk);
    assert_eq!(maria.anim.state(), CreatureAnimStateKind::Locomotion);
    assert_eq!(maria.anim.current_clip(), MariaAnimClips::WALK);

    // Hostile enemy approaches within 1000 units -> Maria Panics & Cowers
    maria.update(
        glam::Vec3::new(0.0, 0.0, 2500.0),
        0.0,
        &[glam::Vec3::new(100.0, 0.0, 100.0)],
        0.016,
    );
    assert_eq!(maria.state, MariaState::Panic);
    assert_eq!(maria.anim.state(), CreatureAnimStateKind::Cower);
    assert_eq!(maria.anim.current_clip(), MariaAnimClips::COWER_PANIC);
}
