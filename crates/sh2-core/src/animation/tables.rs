//! Authentic PS2/PC character animation clip tables (`pjames_anim[34]` at `0x6ceac4`, stage overlays, and Maria).
//!
//! # Ground Truth Reference
//! - PlayStation 2 Debug Prototype (`SLUS_202.28`, `Chacter/m3_play.c`) & PC Director's Cut (`sh2pc.exe`):
//!   - Base locomotion & action table `0x6ceac4` (`pjames_anim[34]`, IDs `101..133`).
//!   - Both James (`jms_basic1.anm`, `jms_wp*.anm`) and Maria (`xmar_wp*.anm`) share the exact same
//!     frame ranges and base fixed-point speeds for Clips `101..127`.

use super::clip::AnimationClip;

/// Default standard animation clips for James Sunderland in `jms_basic1.anm` (`pjames_anim[34]` at `0x6ceac4`).
///
/// Authentic frame ranges and fixed-point speeds (`2048 = 30.0 FPS`) matching original PS2/PC engine:
/// - `Clip 101` (`STAND` / `IDLE`): frames `1..=10` (speed `256` = `3.75 fps` subtle breathing, or `15.0 fps` legacy)
/// - `Clip 102` (`TURN_RIGHT`): frames `14..=28` (speed `1280` = `18.75 fps`, scaled by `tx`)
/// - `Clip 103` (`TURN_LEFT`): frames `34..=48` (speed `1280` = `18.75 fps`, scaled by `tx`)
/// - `Clip 104` (`WALK`): frames `51..=74` (speed `1408` = `20.625 fps`, scaled by `ty`)
/// - `Clip 105` (`BACK`): frames `75..=98` (speed `1536` = `22.5 fps`, scaled by `ty`)
/// - `Clip 106` (`RUN1`): frames `99..=122` (speed `1536` = `22.5 fps`)
/// - `Clip 107` (`RUN2`): frames `123..=146` (speed `1792` = `26.25 fps`)
/// - `Clip 108` (`RUN3`): frames `147..=170` (speed `2048` = `30.0 fps`)
/// - `Clip 109` (`QUICK_TURN_RIGHT`): frames `171..=190` (speed `2048` = `30.0 fps`, non-looping)
/// - `Clip 110` (`QUICK_TURN_LEFT`): frames `191..=210` (speed `2048` = `30.0 fps`, non-looping)
/// - `Clip 113` (`TIRED_1`): frames `281..=300` (speed `1024` = `15.0 fps`)
/// - `Clip 114` (`TIRED_2`): frames `301..=320` (speed `1024` = `15.0 fps`)
/// - `Clip 115..117` (`RELAX`): frames `321..=350`, `351..=380`, `381..=410` (speed `384` = `5.625 fps`, non-looping)
/// - `Clip 118` (`ALERT`): frames `411..=440` (speed `384` = `5.625 fps`, non-looping)
/// - `Clip 124` (`STRAFE_RIGHT_WALK`): frames `516..=539` (speed `1536` = `22.5 fps`)
/// - `Clip 125` (`STRAFE_LEFT_WALK`): frames `540..=563` (speed `1536` = `22.5 fps`)
/// - `Clip 126` (`STRAFE_RIGHT_RUN`): frames `564..=587` (speed `2048` = `30.0 fps`)
/// - `Clip 127` (`STRAFE_LEFT_RUN`): frames `588..=611` (speed `2048` = `30.0 fps`)
pub struct JamesBasicClips;

impl JamesBasicClips {
    /// Authentic standing idle loop (`Clip 101`, frames `1..=10`, 15.0 fps nominal / 3.75 fps PS2 speed 256).
    pub const IDLE: AnimationClip = AnimationClip::new(1, 10, 15.0, true);
    /// Exact PS2 fixed-point standing idle (`Clip 101`, frames `1..=10`, speed `256` = `3.75 fps`).
    pub const STAND_PS2: AnimationClip = AnimationClip::from_ps2_speed(1, 10, 256, true);
    /// Turn right on the spot (`Clip 102`, frames `14..=28`, speed `1280` = `18.75 fps`).
    pub const TURN_RIGHT: AnimationClip = AnimationClip::from_ps2_speed(14, 28, 1280, true);
    /// Turn left on the spot (`Clip 103`, frames `34..=48`, speed `1280` = `18.75 fps`).
    pub const TURN_LEFT: AnimationClip = AnimationClip::from_ps2_speed(34, 48, 1280, true);
    /// Forward walk cycle (`Clip 104`, frames `51..=74`, speed `1408` = `20.625 fps` base, 30.0 nominal).
    pub const WALK: AnimationClip = AnimationClip::from_ps2_speed(51, 74, 1408, true);
    /// Backward step cycle (`Clip 105`, frames `75..=98`, speed `1536` = `22.5 fps`).
    pub const BACK: AnimationClip = AnimationClip::from_ps2_speed(75, 98, 1536, true);
    /// Run stage 1 cycle (`Clip 106`, frames `99..=122`, speed `1536` = `22.5 fps`).
    pub const RUN1: AnimationClip = AnimationClip::from_ps2_speed(99, 122, 1536, true);
    /// Run stage 2 cycle (`Clip 107`, frames `123..=146`, speed `1792` = `26.25 fps`).
    pub const RUN2: AnimationClip = AnimationClip::from_ps2_speed(123, 146, 1792, true);
    /// Run stage 3 full sprint cycle (`Clip 108`, frames `147..=170`, speed `2048` = `30.0 fps`).
    pub const RUN3: AnimationClip = AnimationClip::from_ps2_speed(147, 170, 2048, true);
    /// Legacy alias for Run stage 1 (`Clip 106`, frames `99..=122` @ 22.5 fps).
    pub const RUN: AnimationClip = Self::RUN1;
    /// Quick 180° turn right (`Clip 109`, frames `171..=190`, speed `2048` = `30.0 fps`, non-looping).
    pub const QUICK_TURN_RIGHT: AnimationClip =
        AnimationClip::from_ps2_speed(171, 190, 2048, false);
    /// Quick 180° turn left (`Clip 110`, frames `191..=210`, speed `2048` = `30.0 fps`, non-looping).
    pub const QUICK_TURN_LEFT: AnimationClip = AnimationClip::from_ps2_speed(191, 210, 2048, false);
    /// Tired breathing loop stage 1 (`Clip 113`, frames `281..=300`, speed `1024` = `15.0 fps`).
    pub const TIRED_1: AnimationClip = AnimationClip::from_ps2_speed(281, 300, 1024, true);
    /// Tired heavy panting loop stage 2 (`Clip 114`, frames `301..=320`, speed `1024` = `15.0 fps`).
    pub const TIRED_2: AnimationClip = AnimationClip::from_ps2_speed(301, 320, 1024, true);
    /// Relax idle variant 1 (`Clip 115`, frames `321..=350`, speed `384` = `5.625 fps`, non-looping).
    pub const RELAX_1: AnimationClip = AnimationClip::from_ps2_speed(321, 350, 384, false);
    /// Relax idle variant 2 (`Clip 116`, frames `351..=380`, speed `384` = `5.625 fps`, non-looping).
    pub const RELAX_2: AnimationClip = AnimationClip::from_ps2_speed(351, 380, 384, false);
    /// Relax idle variant 3 (`Clip 117`, frames `381..=410`, speed `384` = `5.625 fps`, non-looping).
    pub const RELAX_3: AnimationClip = AnimationClip::from_ps2_speed(381, 410, 384, false);
    /// Alert look-around idle when enemy is nearby (`Clip 118`, frames `411..=440`, speed `384` = `5.625 fps`, non-looping).
    pub const ALERT: AnimationClip = AnimationClip::from_ps2_speed(411, 440, 384, false);
    /// Right strafe walk cycle (`Clip 124`, frames `516..=539`, speed `1536` = `22.5 fps`).
    pub const STRAFE_RIGHT_WALK: AnimationClip =
        AnimationClip::from_ps2_speed(516, 539, 1536, true);
    /// Left strafe walk cycle (`Clip 125`, frames `540..=563`, speed `1536` = `22.5 fps`).
    pub const STRAFE_LEFT_WALK: AnimationClip = AnimationClip::from_ps2_speed(540, 563, 1536, true);
    /// Right strafe run cycle (`Clip 126`, frames `564..=587`, speed `2048` = `30.0 fps`).
    pub const STRAFE_RIGHT_RUN: AnimationClip = AnimationClip::from_ps2_speed(564, 587, 2048, true);
    /// Left strafe run cycle (`Clip 127`, frames `588..=611`, speed `2048` = `30.0 fps`).
    pub const STRAFE_LEFT_RUN: AnimationClip = AnimationClip::from_ps2_speed(588, 611, 2048, true);

    /// Weapon aim readiness stance (frames `200..=230`).
    pub const AIM: AnimationClip = AnimationClip::new(200, 230, 30.0, true);
    /// Weapon strike / attack execution (frames `231..=270`).
    pub const ATTACK: AnimationClip = AnimationClip::new(231, 270, 30.0, false);
    /// Damage / hurt flinch reaction (frames `453..=470`, PS2 index `122`, speed `1024` = `15.0 fps`).
    pub const HURT: AnimationClip = AnimationClip::new(453, 470, 15.0, false);
    /// Authentic PS2/PC James struggle clip against overhead neck chokes / Flesh Lips & Pyramid Head
    /// (`pjames_anim` aid `121` frames `453..=470` / `457..=468`, speed `1024` = `15.0 fps`, PS2 `lower_hold` lines `0x21dc80-0x21dca4`).
    pub const STRUGGLE_OVERHEAD: AnimationClip = AnimationClip::new(457, 468, 15.0, true);
    /// Generic struggle held animation (used as fallback: frames `457..=461` inside `J_HURT`, `15.0 fps`).
    pub const STRUGGLE_HELD: AnimationClip = AnimationClip::new(457, 461, 15.0, true);
    /// Authentic PS2/PC James struggle clip against leg attacks / Creepers
    /// (`pjames_anim` aid `127` frames `588..=611`, speed `2048` = `30.0 fps`).
    pub const STRUGGLE_LEG: AnimationClip = AnimationClip::new(588, 611, 30.0, true);
    /// Authentic PS2/PC James struggle clip against torso grabs / Lying Figure & Abstract Daddy
    /// (`pjames_anim` aid `128` frames `612..=629`, speed `1024` = `15.0 fps`).
    pub const STRUGGLE_TORSO: AnimationClip = AnimationClip::new(612, 629, 15.0, true);
    /// Authentic PS2/PC James struggle clip against standing throat / tentacle choke holds / Mary Boss
    /// (`pjames_anim` aid `129` frames `632..=649`, speed `1024` = `15.0 fps`).
    pub const STRUGGLE_CHOKE_MARY: AnimationClip = AnimationClip::new(632, 649, 15.0, true);
    /// Authentic PS2/PC James breakout release push when breaking free from grab
    /// (`pjames_anim` aid `130` frames `652..=670`, speed `2048` = `30.0 fps`).
    pub const STRUGGLE_RELEASE: AnimationClip = AnimationClip::new(652, 670, 30.0, false);
    /// Authentic PS2/PC James struggle clip against torso pinning / standing grapples / Abstract Daddy & Lying Figure
    /// (`pjames_anim` aid `128` frames `612..=629`, PS2 speed `1024` = `15.0 fps`).
    pub const STRUGGLE_DADDY: AnimationClip = AnimationClip::new(612, 629, 15.0, true);
    /// Authentic PS2/PC James struggle clip against Flesh Lips overhead ceiling choke & Mary Boss choke
    /// (`pjames_anim` aid `129` frames `632..=649`, PS2 speed `1024` = `15.0 fps`, looping).
    pub const STRUGGLE_FLESH_LIPS: AnimationClip = AnimationClip::new(632, 649, 15.0, true);
    /// Authentic PS2/PC James struggle clip against Pyramid Head one-handed overhead choke lift
    /// (`pjames_anim` aid `121` frames `457..=468`, speed `1024` = `15.0 fps`).
    pub const STRUGGLE_CHOKE_LIFT: AnimationClip = AnimationClip::new(457, 468, 15.0, true);
    /// Authentic PS2/PC James breakout release push from Mary / Lying Figure choke (`pjames_anim` aid `130` frames `652..=670`, speed `2048` = `30.0 fps`).
    pub const STRUGGLE_CHOKE_RELEASE: AnimationClip = AnimationClip::new(652, 670, 30.0, false);
    /// Authentic PS2/PC James downed enemy kick action (`pjames_anim` aid `132` frames `682..=701`, speed `768` = `11.25 fps`).
    pub const KICK_DOWNED: AnimationClip = AnimationClip::new(682, 701, 11.25, false);
    /// Authentic PS2/PC James downed enemy weapon stomp action (`pjames_anim` aid `133` frames `702..=719`, speed `512` = `7.5 fps`).
    pub const STOMP_DOWNED: AnimationClip = AnimationClip::new(702, 719, 7.5, false);
    /// Collapse / death fall animation (frames `256..=280`, PS2 index `112`, speed `1024` = `15.0 fps`).
    pub const FALL: AnimationClip = AnimationClip::new(256, 280, 15.0, false);
}

/// Stage-specific animation clips for James Sunderland (boss grapple sequences).
/// In authentic retail Silent Hill 2 (`SLUS_202.28`), James's stage-specific struggle animations
/// (`aid=20029..20057`) are loaded from each stage overlay's `.anm` file via `PlayerGetStageAnime`:
/// - Pyramid Head (`gx_ast`): `jms_stg_apart_stair.anm` (`440..=538`, `aid=20029..20032`)
/// - Flesh Lips (`gy_hxw`): `jms_stg_hospital_1fw_b.anm` (`230..=328`, `aid=20038..20041` — byte-identical 99-frame sequence to Pyramid Head)
/// - Abstract Daddy (`gz_lre`): `jms_stg_labyrinth_e.anm` (`298..=465`, `aid=20042..20049`)
/// - Mary Final Boss (`gz_erc`): `jms_stg_end_recovery.anm` (`218..=301`, `aid=20054..20057`)
pub struct JamesStageClips;

impl JamesStageClips {
    /// Authentic Flesh Lips overhead ceiling choke struggle loop (`aid=20039`)
    /// (loaded from `jms_stg_hospital_1fw_b.anm` frames `245..=268` @ 30fps, looping).
    pub const FLESH_LIPS_CHOKE_LOOP: AnimationClip = AnimationClip::new(245, 268, 30.0, true);

    /// Flesh Lips initial ceiling choke grab hit (`aid=20038`)
    /// (loaded from `jms_stg_hospital_1fw_b.anm` frames `230..=244` @ 15fps).
    pub const FLESH_LIPS_CHOKE_GRAB: AnimationClip = AnimationClip::new(230, 244, 15.0, false);

    /// Flesh Lips ceiling choke release / breakout drop (`aid=20040`)
    /// (loaded from `jms_stg_hospital_1fw_b.anm` frames `269..=298` @ 15fps).
    pub const FLESH_LIPS_CHOKE_RELEASE: AnimationClip = AnimationClip::new(269, 298, 15.0, false);

    /// Flesh Lips ceiling choke death drop (`aid=20041`)
    /// (loaded from `jms_stg_hospital_1fw_b.anm` frames `299..=328` @ 15fps).
    pub const FLESH_LIPS_CHOKE_DEATH: AnimationClip = AnimationClip::new(299, 328, 15.0, false);

    /// Full Flesh Lips choke sequence including grab entry and struggle loop
    /// (loaded from `jms_stg_hospital_1fw_b.anm` frames `230..=268` @ 15fps).
    pub const FLESH_LIPS_CHOKE_FULL: AnimationClip = AnimationClip::new(230, 268, 15.0, true);

    /// Authentic Flesh Lips overhead ceiling choke struggle loop alias (`aid=20039`, `245..=268` @ 30fps).
    pub const FLESH_LIPS_CEILING_CHOKE_LOOP: AnimationClip =
        AnimationClip::new(245, 268, 30.0, true);

    /// Authentic Mary Final Boss tentacle choke struggle loop (`aid=20054`)
    /// (loaded from `jms_stg_end_recovery.anm` frames `218..=241` @ 30fps, looping).
    pub const MARY_TENTACLE_CHOKE_LOOP: AnimationClip = AnimationClip::new(218, 241, 30.0, true);

    /// Authentic Mary Final Boss Phase 1 initial tentacle catch & lift (`aid=20056`)
    /// (loaded from `jms_stg_end_recovery.anm` frames `272..=286` @ 15fps, non-looping).
    pub const MARY_TENTACLE_CHOKE_FULL: AnimationClip = AnimationClip::new(272, 286, 15.0, false);

    /// Mary Final Boss tentacle choke breakout release (`aid=20055`)
    /// (loaded from `jms_stg_end_recovery.anm` frames `287..=301` @ 15fps, non-looping).
    pub const MARY_TENTACLE_CHOKE_RELEASE: AnimationClip =
        AnimationClip::new(287, 301, 15.0, false);

    /// Mary Final Boss tentacle choke death collapse (`aid=20057`)
    /// (loaded from `jms_stg_end_recovery.anm` frames `242..=271` @ 15fps, non-looping).
    pub const MARY_TENTACLE_CHOKE_DEATH: AnimationClip = AnimationClip::new(242, 271, 15.0, false);

    /// Authentic Pyramid Head staircase one-handed choke lift hold (`aid=20030`)
    /// (loaded from `jms_stg_apart_stair.anm` frames `455..=478` @ 30fps / `spd=2048`, looping).
    pub const PYRAMID_HEAD_STAIR_CHOKE_LOOP: AnimationClip =
        AnimationClip::new(455, 478, 30.0, true);

    /// Pyramid Head staircase initial neck grab hit (`aid=20029`)
    /// (loaded from `jms_stg_apart_stair.anm` frames `440..=454` @ 15fps / `spd=1024`).
    pub const PYRAMID_HEAD_STAIR_CHOKE_GRAB: AnimationClip =
        AnimationClip::new(440, 454, 15.0, false);

    /// Pyramid Head staircase choke release drop (`aid=20031`)
    /// (loaded from `jms_stg_apart_stair.anm` frames `479..=508` @ 15fps / `spd=1024`).
    pub const PYRAMID_HEAD_STAIR_CHOKE_RELEASE: AnimationClip =
        AnimationClip::new(479, 508, 15.0, false);

    /// Pyramid Head staircase choke death drop (`aid=20032`)
    /// (loaded from `jms_stg_apart_stair.anm` frames `509..=538` @ 15fps / `spd=1024`).
    pub const PYRAMID_HEAD_STAIR_CHOKE_DEATH: AnimationClip =
        AnimationClip::new(509, 538, 15.0, false);

    /// Pyramid Head apartment 2F/3F choke lift reaction
    /// (loaded from `jms_stg_apart_w2f.anm` frames `351..=357` @ 15fps, looping).
    pub const PYRAMID_HEAD_APART_CHOKE_LOOP: AnimationClip =
        AnimationClip::new(351, 357, 15.0, true);

    /// Authentic Abstract Daddy Labyrinth East front smother hold loop (`aid=20043`)
    /// (loaded from `jms_stg_labyrinth_e.anm` frames `313..=336` @ 30fps, looping).
    pub const ABSTRACT_DADDY_STRUGGLE_LIFT: AnimationClip =
        AnimationClip::new(313, 336, 30.0, true);

    /// Abstract Daddy Labyrinth East front grab entry (`aid=20042`)
    /// (loaded from `jms_stg_labyrinth_e.anm` frames `298..=312` @ 15fps).
    pub const ABSTRACT_DADDY_FRONT_GRAB: AnimationClip = AnimationClip::new(298, 312, 15.0, false);

    /// Abstract Daddy Labyrinth East front breakout release (`aid=20045`)
    /// (loaded from `jms_stg_labyrinth_e.anm` frames `337..=351` @ 15fps).
    pub const ABSTRACT_DADDY_FRONT_RELEASE: AnimationClip =
        AnimationClip::new(337, 351, 15.0, false);

    /// Abstract Daddy Labyrinth East front death collapse (`aid=20044`)
    /// (loaded from `jms_stg_labyrinth_e.anm` frames `352..=381` @ 15fps).
    pub const ABSTRACT_DADDY_FRONT_DEATH: AnimationClip = AnimationClip::new(352, 381, 15.0, false);

    /// Abstract Daddy Labyrinth East rear smother hold loop (`aid=20047`)
    /// (loaded from `jms_stg_labyrinth_e.anm` frames `397..=420` @ 30fps, looping).
    pub const ABSTRACT_DADDY_BACK_LOOP: AnimationClip = AnimationClip::new(397, 420, 30.0, true);
}

/// Standard locomotion animation clips for Maria in `xmar_wpnone.anm` (*Born From a Wish*).
///
/// Maria's playable `.anm` files (`xmar_wpnone.anm`, `xmar_wpcolt.anm`, `xmar_wpknif.anm`) hold
/// the exact same base moves at the exact same frames and fixed-point speeds as James (`0x531a00`).
pub struct MariaLocomotionClips;

impl MariaLocomotionClips {
    /// Standing idle loop (`Clip 101`, frames `1..=10`).
    pub const IDLE: AnimationClip = JamesBasicClips::IDLE;
    /// Turn right on the spot (`Clip 102`, frames `14..=28`, `18.75 fps`).
    pub const TURN_RIGHT: AnimationClip = JamesBasicClips::TURN_RIGHT;
    /// Turn left on the spot (`Clip 103`, frames `34..=48`, `18.75 fps`).
    pub const TURN_LEFT: AnimationClip = JamesBasicClips::TURN_LEFT;
    /// Forward walk cycle (`Clip 104`, frames `51..=74`, `20.625 fps`).
    pub const WALK: AnimationClip = JamesBasicClips::WALK;
    /// Backward step cycle (`Clip 105`, frames `75..=98`, `22.5 fps`).
    pub const BACK: AnimationClip = JamesBasicClips::BACK;
    /// Run stage 1 cycle (`Clip 106`, frames `99..=122`, `22.5 fps`).
    pub const RUN1: AnimationClip = JamesBasicClips::RUN1;
    /// Run stage 2 cycle (`Clip 107`, frames `123..=146`, `26.25 fps`).
    pub const RUN2: AnimationClip = JamesBasicClips::RUN2;
    /// Run stage 3 cycle (`Clip 108`, frames `147..=170`, `30.0 fps`).
    pub const RUN3: AnimationClip = JamesBasicClips::RUN3;
    /// Legacy alias for Run stage 1 (`Clip 106`).
    pub const RUN: AnimationClip = JamesBasicClips::RUN1;
    /// Quick 180° turn right (`Clip 109`, frames `171..=190`, `30.0 fps`).
    pub const QUICK_TURN_RIGHT: AnimationClip = JamesBasicClips::QUICK_TURN_RIGHT;
    /// Quick 180° turn left (`Clip 110`, frames `191..=210`, `30.0 fps`).
    pub const QUICK_TURN_LEFT: AnimationClip = JamesBasicClips::QUICK_TURN_LEFT;
    /// Tired breathing loop (`Clip 113`, frames `281..=300`, `15.0 fps`).
    pub const TIRED: AnimationClip = JamesBasicClips::TIRED_1;
    /// Relax idle (`Clip 115`, frames `321..=350`, `5.625 fps`).
    pub const RELAX: AnimationClip = JamesBasicClips::RELAX_1;
    /// Alert look-around (`Clip 118`, frames `411..=440`, `5.625 fps`).
    pub const ALERT: AnimationClip = JamesBasicClips::ALERT;
    /// Right strafe walk (`Clip 124`, frames `516..=539`, `22.5 fps`).
    pub const STRAFE_RIGHT_WALK: AnimationClip = JamesBasicClips::STRAFE_RIGHT_WALK;
    /// Left strafe walk (`Clip 125`, frames `540..=563`, `22.5 fps`).
    pub const STRAFE_LEFT_WALK: AnimationClip = JamesBasicClips::STRAFE_LEFT_WALK;
    /// Right strafe run (`Clip 126`, frames `564..=587`, `30.0 fps`).
    pub const STRAFE_RIGHT_RUN: AnimationClip = JamesBasicClips::STRAFE_RIGHT_RUN;
    /// Left strafe run (`Clip 127`, frames `588..=611`, `30.0 fps`).
    pub const STRAFE_LEFT_RUN: AnimationClip = JamesBasicClips::STRAFE_LEFT_RUN;
}

/// Complete locomotion clip set configuration for the active playable character (`0x6ceac4`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CharacterLocomotionClips {
    pub idle: AnimationClip,
    pub relax: AnimationClip,
    pub alert: AnimationClip,
    pub tired: AnimationClip,
    pub turn_left: AnimationClip,
    pub turn_right: AnimationClip,
    pub walk: AnimationClip,
    pub back: AnimationClip,
    pub strafe_left_walk: AnimationClip,
    pub strafe_right_walk: AnimationClip,
    pub run: AnimationClip,
    pub run2: AnimationClip,
    pub run3: AnimationClip,
    pub strafe_left_run: AnimationClip,
    pub strafe_right_run: AnimationClip,
    pub quick_turn_left: AnimationClip,
    pub quick_turn_right: AnimationClip,
}

impl Default for CharacterLocomotionClips {
    fn default() -> Self {
        Self::JAMES
    }
}

impl CharacterLocomotionClips {
    /// James Sunderland standard locomotion clips (`jms_basic1.anm`).
    pub const JAMES: Self = Self {
        idle: JamesBasicClips::IDLE,
        relax: JamesBasicClips::RELAX_1,
        alert: JamesBasicClips::ALERT,
        tired: JamesBasicClips::TIRED_1,
        turn_left: JamesBasicClips::TURN_LEFT,
        turn_right: JamesBasicClips::TURN_RIGHT,
        walk: JamesBasicClips::WALK,
        back: JamesBasicClips::BACK,
        strafe_left_walk: JamesBasicClips::STRAFE_LEFT_WALK,
        strafe_right_walk: JamesBasicClips::STRAFE_RIGHT_WALK,
        run: JamesBasicClips::RUN1,
        run2: JamesBasicClips::RUN2,
        run3: JamesBasicClips::RUN3,
        strafe_left_run: JamesBasicClips::STRAFE_LEFT_RUN,
        strafe_right_run: JamesBasicClips::STRAFE_RIGHT_RUN,
        quick_turn_left: JamesBasicClips::QUICK_TURN_LEFT,
        quick_turn_right: JamesBasicClips::QUICK_TURN_RIGHT,
    };

    /// Maria playable locomotion clips (`xmar_wpnone.anm`).
    pub const MARIA: Self = Self {
        idle: MariaLocomotionClips::IDLE,
        relax: MariaLocomotionClips::RELAX,
        alert: MariaLocomotionClips::ALERT,
        tired: MariaLocomotionClips::TIRED,
        turn_left: MariaLocomotionClips::TURN_LEFT,
        turn_right: MariaLocomotionClips::TURN_RIGHT,
        walk: MariaLocomotionClips::WALK,
        back: MariaLocomotionClips::BACK,
        strafe_left_walk: MariaLocomotionClips::STRAFE_LEFT_WALK,
        strafe_right_walk: MariaLocomotionClips::STRAFE_RIGHT_WALK,
        run: MariaLocomotionClips::RUN1,
        run2: MariaLocomotionClips::RUN2,
        run3: MariaLocomotionClips::RUN3,
        strafe_left_run: MariaLocomotionClips::STRAFE_LEFT_RUN,
        strafe_right_run: MariaLocomotionClips::STRAFE_RIGHT_RUN,
        quick_turn_left: MariaLocomotionClips::QUICK_TURN_LEFT,
        quick_turn_right: MariaLocomotionClips::QUICK_TURN_RIGHT,
    };
}
