//! Integration tests verifying creature and companion skeletal animation synchronization,
//! authentic retail .anm frame range conformance, root motion advancement, and state machine binding (SH2-57).

use sh2_assets::formats::anm::parse_anm;
use sh2_assets::formats::mdls::parse_mdl;
use sh2_core::ai::creatures::anim::{
    AngelaAnimClips, CreatureAnimStateKind, CreatureAnimationController, EdbState,
    EddieBossAnimClips, EddieNpcAnimClips, LauraAnimClips, LyingFigureAnimClips,
    MannequinAnimClips, MariaAnimClips, MaryNpcAnimClips, NpcAnimState, NurseAnimClips,
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
    let scu_anim =
        parse_anm(&scu_anm, scu_model.bone_count, &scu_model.parents).expect("scu anm parse");
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
    let mkn_anim =
        parse_anm(&mkn_anm, mkn_model.bone_count, &mkn_model.parents).expect("mkn anm parse");
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
    let nse_anim =
        parse_anm(&nse_anm, nse_model.bone_count, &nse_model.parents).expect("nse anm parse");
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
    let mar_anim =
        parse_anm(&mar_anm, mar_model.bone_count, &mar_model.parents).expect("mar anm parse");
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
    scu.update(
        &target,
        RoomLighting::DarkLightOff,
        DifficultyLevel::Normal,
        0.016,
    );
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
    mkn.update(
        &target,
        RoomLighting::DarkLightOff,
        DifficultyLevel::Normal,
        0.016,
    );
    assert_eq!(mkn.state, MknState::Stay);
    assert_eq!(mkn.anim.state(), CreatureAnimStateKind::Idle);
    assert_eq!(mkn.anim.current_clip(), MannequinAnimClips::STAY);

    // Attack strike alternation
    mkn.state = MknState::Attack;
    mkn.is_kick_next = false;
    mkn.update(
        &target,
        RoomLighting::DarkLightOff,
        DifficultyLevel::Normal,
        0.016,
    );
    assert_eq!(mkn.anim.state(), CreatureAnimStateKind::AttackPrimary);
    assert_eq!(mkn.anim.current_clip(), MannequinAnimClips::ARM_SWING);

    mkn.state = MknState::Attack;
    mkn.is_kick_next = true;
    mkn.update(
        &target,
        RoomLighting::DarkLightOff,
        DifficultyLevel::Normal,
        0.016,
    );
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

#[test]
fn test_sh2_34_npc_and_eddie_boss_retail_anm_conformance() {
    if !Path::new(RETAIL_CHR_ROOT).exists() {
        println!("Skipping SH2-34 retail anm test: assets not present");
        return;
    }

    // 1. Angela Orosco (`agl.mdl`: 75 bones, `p_agl.anm`: 298 frames)
    let agl_mdl = parse_mdl(&fs::read(format!("{RETAIL_CHR_ROOT}/agl/agl.mdl")).expect("agl.mdl"))
        .expect("agl mdl parse");
    let agl_anm = parse_anm(
        &fs::read(format!("{RETAIL_CHR_ROOT}/agl/p_agl.anm")).expect("p_agl.anm"),
        agl_mdl.bone_count,
        &agl_mdl.parents,
    )
    .expect("p_agl anm parse");
    assert_eq!(agl_mdl.bone_count, 75);
    assert_eq!(agl_anm.frames.len(), 298);
    assert!(AngelaAnimClips::WALK_A.end_frame < agl_anm.frames.len() as u32);
    assert!(AngelaAnimClips::WALK_B.end_frame < agl_anm.frames.len() as u32);
    assert!(AngelaAnimClips::IDLE_BREATHE.end_frame < agl_anm.frames.len() as u32);
    assert_eq!(
        AngelaAnimClips::STAIR_WALK.end_frame,
        (agl_anm.frames.len() - 1) as u32
    );

    // 2. Eddie High-LOD (`hhh_edi.mdl`: 64 bones, `p_edi.anm`: 368 frames)
    let edi_mdl =
        parse_mdl(&fs::read(format!("{RETAIL_CHR_ROOT}/edi/hhh_edi.mdl")).expect("hhh_edi.mdl"))
            .expect("hhh_edi mdl parse");
    let edi_anm = parse_anm(
        &fs::read(format!("{RETAIL_CHR_ROOT}/edi/p_edi.anm")).expect("p_edi.anm"),
        edi_mdl.bone_count,
        &edi_mdl.parents,
    )
    .expect("p_edi anm parse");
    assert_eq!(edi_mdl.bone_count, 64);
    assert_eq!(edi_anm.frames.len(), 368);
    assert!(EddieNpcAnimClips::IDLE_BREATHE_A.end_frame < edi_anm.frames.len() as u32);
    assert!(EddieNpcAnimClips::IDLE_BREATHE_B.end_frame < edi_anm.frames.len() as u32);
    assert!(EddieNpcAnimClips::TALK_GESTURE.end_frame < edi_anm.frames.len() as u32);
    assert!(EddieNpcAnimClips::COWER_SIT.end_frame < edi_anm.frames.len() as u32);
    assert!(EddieNpcAnimClips::STAND_IDLE.end_frame < edi_anm.frames.len() as u32);
    assert_eq!(
        EddieNpcAnimClips::STATIC_POSE.end_frame,
        (edi_anm.frames.len() - 1) as u32
    );

    // 3. Eddie Gameplay & Boss (`en_edb`, `lll_edi.mdl`: 35 bones, `lll_edi.anm`: 437 frames)
    let edb_mdl =
        parse_mdl(&fs::read(format!("{RETAIL_CHR_ROOT}/edi/lll_edi.mdl")).expect("lll_edi.mdl"))
            .expect("lll_edi mdl parse");
    let edb_anm = parse_anm(
        &fs::read(format!("{RETAIL_CHR_ROOT}/edi/lll_edi.anm")).expect("lll_edi.anm"),
        edb_mdl.bone_count,
        &edb_mdl.parents,
    )
    .expect("lll_edi anm parse");
    assert_eq!(edb_mdl.bone_count, 35);
    assert_eq!(edb_anm.frames.len(), 437);
    assert!(EddieBossAnimClips::STAND.end_frame < edb_anm.frames.len() as u32);
    assert!(EddieBossAnimClips::WALK.end_frame < edb_anm.frames.len() as u32);
    assert!(EddieBossAnimClips::RUN.end_frame < edb_anm.frames.len() as u32);
    assert!(EddieBossAnimClips::AIM_STEP_LEFT.end_frame < edb_anm.frames.len() as u32);
    assert!(EddieBossAnimClips::AIM_STEP_RIGHT.end_frame < edb_anm.frames.len() as u32);
    assert!(EddieBossAnimClips::SHOOT_REVOLVER.end_frame < edb_anm.frames.len() as u32);
    assert!(EddieBossAnimClips::AIM_HOLD_RECOIL.end_frame < edb_anm.frames.len() as u32);
    assert!(EddieBossAnimClips::MELEE_PUNCH.end_frame < edb_anm.frames.len() as u32);
    assert!(EddieBossAnimClips::RELOAD_TURN.end_frame < edb_anm.frames.len() as u32);
    for dmg_clip in &EddieBossAnimClips::DAMAGE_CLIPS {
        assert!(dmg_clip.end_frame < edb_anm.frames.len() as u32);
    }
    assert_eq!(
        EddieBossAnimClips::DAMAGE_CLIPS[11].end_frame,
        (edb_anm.frames.len() - 1) as u32
    );

    // 4. Laura (`lau.mdl`: 89 bones, `p_lau.anm`: 73 frames)
    let lau_mdl = parse_mdl(&fs::read(format!("{RETAIL_CHR_ROOT}/lau/lau.mdl")).expect("lau.mdl"))
        .expect("lau mdl parse");
    let lau_anm = parse_anm(
        &fs::read(format!("{RETAIL_CHR_ROOT}/lau/p_lau.anm")).expect("p_lau.anm"),
        lau_mdl.bone_count,
        &lau_mdl.parents,
    )
    .expect("p_lau anm parse");
    assert_eq!(lau_mdl.bone_count, 89);
    assert_eq!(lau_anm.frames.len(), 73);
    assert_eq!(
        LauraAnimClips::WALK_IDLE.end_frame,
        (lau_anm.frames.len() - 1) as u32
    );

    // 5. Mary Shepherd-Sunderland (`mry.mdl`: 81 bones, `p_mry.anm`: 135 frames)
    let mry_mdl = parse_mdl(&fs::read(format!("{RETAIL_CHR_ROOT}/mry/mry.mdl")).expect("mry.mdl"))
        .expect("mry mdl parse");
    let mry_anm = parse_anm(
        &fs::read(format!("{RETAIL_CHR_ROOT}/mry/p_mry.anm")).expect("p_mry.anm"),
        mry_mdl.bone_count,
        &mry_mdl.parents,
    )
    .expect("p_mry anm parse");
    assert_eq!(mry_mdl.bone_count, 81);
    assert_eq!(mry_anm.frames.len(), 135);
    assert!(MaryNpcAnimClips::WALK_A.end_frame < mry_anm.frames.len() as u32);
    assert_eq!(
        MaryNpcAnimClips::WALK_B.end_frame,
        (mry_anm.frames.len() - 1) as u32
    );
}

#[test]
fn test_sh2_26_james_and_maria_locomotion_anm_conformance() {
    if !Path::new(RETAIL_CHR_ROOT).exists() {
        println!("Skipping SH2-26 retail anm test: assets not present");
        return;
    }

    let jms_mdl =
        parse_mdl(&fs::read(format!("{RETAIL_CHR_ROOT}/jms/lll_jms.mdl")).expect("lll_jms.mdl"))
            .expect("lll_jms mdl parse");
    let jms_anm = parse_anm(
        &fs::read(format!("{RETAIL_CHR_ROOT}/jms/jms_basic1.anm")).expect("jms_basic1.anm"),
        jms_mdl.bone_count,
        &jms_mdl.parents,
    )
    .expect("jms_basic1 anm parse");

    let jms_frames = jms_anm.frames.len() as u32;
    let clips = sh2_core::CharacterLocomotionClips::JAMES;
    for clip in [
        clips.idle,
        clips.relax,
        clips.alert,
        clips.tired,
        clips.turn_left,
        clips.turn_right,
        clips.walk,
        clips.back,
        clips.strafe_left_walk,
        clips.strafe_right_walk,
        clips.run,
        clips.run2,
        clips.run3,
        clips.strafe_left_run,
        clips.strafe_right_run,
        clips.quick_turn_left,
        clips.quick_turn_right,
    ] {
        assert!(
            clip.end_frame < jms_frames,
            "James clip {:?} exceeds jms_basic1.anm frame count {}",
            clip,
            jms_frames
        );
    }

    // Playable Maria (Born From a Wish: chr2/mar/lxx_mar.mdl & xmar_wpnone.anm)
    let chr2_mar_dir = "Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2/data/chr2/mar";
    if Path::new(chr2_mar_dir).exists() {
        let xmar_mdl =
            parse_mdl(&fs::read(format!("{chr2_mar_dir}/lxx_mar.mdl")).expect("lxx_mar.mdl"))
                .expect("lxx_mar mdl parse");
        let xmar_anm = parse_anm(
            &fs::read(format!("{chr2_mar_dir}/xmar_wpnone.anm")).expect("xmar_wpnone.anm"),
            xmar_mdl.bone_count,
            &xmar_mdl.parents,
        )
        .expect("xmar_wpnone anm parse");
        let xmar_frames = xmar_anm.frames.len() as u32;
        let m_clips = sh2_core::CharacterLocomotionClips::MARIA;
        assert!(m_clips.strafe_left_run.end_frame < xmar_frames);
        assert!(m_clips.run3.end_frame < xmar_frames);
    }
}

#[test]
fn test_sh2_34_npc_and_eddie_boss_state_synchronization() {
    // 1. Angela state transitions & root motion
    let mut agl = CreatureAnimationController::new_angela();
    assert_eq!(agl.state(), CreatureAnimStateKind::Idle);
    assert_eq!(agl.current_clip(), AngelaAnimClips::IDLE_BREATHE);
    agl.sync_angela(NpcAnimState::WalkPrimary);
    assert_eq!(agl.state(), CreatureAnimStateKind::Locomotion);
    assert_eq!(agl.current_clip(), AngelaAnimClips::WALK_A);
    agl.update(0.25, 0.0);
    assert!(agl.root_motion_delta().z > 0.0);

    // 2. Eddie High-LOD NPC dialogue & cower synchronization
    let mut edi_npc = CreatureAnimationController::new_eddie_npc();
    assert_eq!(edi_npc.current_clip(), EddieNpcAnimClips::IDLE_BREATHE_A);
    edi_npc.sync_eddie_npc(NpcAnimState::TalkOrGesture);
    assert_eq!(edi_npc.current_clip(), EddieNpcAnimClips::TALK_GESTURE);
    edi_npc.sync_eddie_npc(NpcAnimState::CowerOrSpecial);
    assert_eq!(edi_npc.state(), CreatureAnimStateKind::Cower);
    assert_eq!(edi_npc.current_clip(), EddieNpcAnimClips::COWER_SIT);

    // 3. Eddie Boss (`en_edb`) combat state machine synchronization
    let mut edb = CreatureAnimationController::new_eddie_boss();
    assert_eq!(edb.current_clip(), EddieBossAnimClips::STAND);

    edb.sync_eddie_boss(EdbState::Run);
    assert_eq!(edb.state(), CreatureAnimStateKind::Locomotion);
    assert_eq!(edb.current_clip(), EddieBossAnimClips::RUN);
    edb.update(0.2, 0.0);
    assert!(edb.root_motion_delta().z > 0.0);

    // Shoot revolver (frames 98..=121, hit window 102..=106)
    edb.sync_eddie_boss(EdbState::ShootRevolver);
    assert_eq!(edb.state(), CreatureAnimStateKind::AttackPrimary);
    assert_eq!(edb.current_clip(), EddieBossAnimClips::SHOOT_REVOLVER);
    edb.update(5.0 / 30.0, 0.0); // frame 103
    assert!(edb.is_in_hit_window());

    // Melee punch (frames 164..=196 at 22.5 fps, hit window 174..=182)
    edb.sync_eddie_boss(EdbState::MeleePunch);
    assert_eq!(edb.state(), CreatureAnimStateKind::AttackSecondary);
    assert_eq!(edb.current_clip(), EddieBossAnimClips::MELEE_PUNCH);
    edb.update(12.0 / 22.5, 0.0); // frame 176
    assert!(edb.is_in_hit_window());

    // Reload turn (frames 197..=256 at 30 fps, cartridge event at 215..=217)
    edb.sync_eddie_boss(EdbState::ReloadTurn);
    assert_eq!(edb.state(), CreatureAnimStateKind::Recovery);
    edb.update(19.0 / 30.0, 0.0); // frame 216
    assert!(edb.is_in_particle_window());

    // Directional damage reactions (5910..=5921)
    for variant in 0..12u8 {
        edb.sync_eddie_boss(EdbState::Damage(variant));
        assert_eq!(edb.state(), CreatureAnimStateKind::Flinch);
        assert_eq!(
            edb.current_clip(),
            EddieBossAnimClips::DAMAGE_CLIPS[variant as usize]
        );
    }

    // 4. Laura & Mary NPC locomotion synchronization
    let mut lau = CreatureAnimationController::new_laura();
    lau.sync_laura(NpcAnimState::WalkPrimary);
    lau.update(0.1, 0.0);
    assert!(lau.root_motion_delta().z > 0.0);

    let mut mry = CreatureAnimationController::new_mary_npc();
    mry.sync_mary_npc(NpcAnimState::WalkSecondary);
    assert_eq!(mry.current_clip(), MaryNpcAnimClips::WALK_B);
    mry.update(0.1, 0.0);
    assert!(mry.root_motion_delta().z > 0.0);
}
