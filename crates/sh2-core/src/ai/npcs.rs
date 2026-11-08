//! Primary story NPC character animation tables, state synchronization, and Eddie Boss (`en_edb`)
//! combat animation state bindings (SH2-34, 1:1 PS2 `SLUS_202.28` & PC `sh2pc.exe` parity).
//!
//! # Ground Truth:
//! - PS2 prototype unstripped (`SLUS_202.28`):
//!   - **Angela Orosco (`agl.mdl` / `ragl.mdl`, `model_id = 0x107`, 75 bones):**
//!     - `shCharacterHumanAGLAnimeSetP` (`0x001037e0` / PC `0x00540b70`)
//!     - `pangela_anim` (`0x00389240` / PC `0x006e5a38`, `p_agl.anm`, 952 B/frame, 298 frames):
//!       - `aid = 3901`: Walk A (`0..=63`, 64f, 30.0 fps, loop, velocity `1984.63` mm/s)
//!       - `aid = 3903`: Walk B (`64..=132`, 69f, 30.0 fps, loop, velocity `-500.0` mm/s)
//!       - `aid = 3904`: Idle Breathe (`133..=206`, 74f, speed `128` = `1.875` fps, loop)
//!       - `aid = 3905`: Stair Walk (`207..=297`, 91f, speed `128` = `1.875` fps, single-shot)
//!     - `dangela_anim` (`0x003890c0` / PC `0x006e58c4`, `aid = 3501..=3531`, `dangela_anime_adr_list` `0x002a7bd0`)
//!   - **Eddie Dombrowski High-LOD (`hhh_edi.mdl` / `rhhh_edi.mdl`, `model_id = 0x108`, 64 bones):**
//!     - `shCharacterHumanEDIAnimeSetP` (`0x00105020` / PC `0x00540a00`)
//!     - `peddie_anim` (`0x0038aa90` / PC `0x006e4d70`, `p_edi.anm`, 812 B/frame, 368 frames):
//!       - `aid = 4801`: Idle Breathe A (`0..=71`, 72f, speed `512` = `7.5` fps, loop, velocity `-242.20` mm/s)
//!       - `aid = 4802`: Idle Breathe B (`72..=142`, 71f, speed `512` = `7.5` fps, loop, velocity `-432.20` mm/s)
//!       - Sub-clip 3: Talk / Gesture (`143..=224`, 82f, 30.0 fps, loop)
//!       - Sub-clip 4: Cower / Sit (`225..=297`, 73f, 30.0 fps, loop)
//!       - Sub-clip 5: Stand Pose (`298..=365`, 68f, 30.0 fps, loop)
//!       - `aid = 4803`: Static Pose (`366..=367`, 2f, 30.0 fps, loop)
//!     - `eddie_anim` (`0x0038a940` / PC `0x006e4c2c`, `aid = 4501..=4527`, `deddie_anime_adr_list` `0x002a8790`)
//!   - **Eddie Boss (`en_edb`, `lll_edi.mdl`, `model_id = 0x208`, 35 bones):**
//!     - `enEDBAnimeSet` (`0x0014d080`), `enEDBAnimeExec` (`0x0014d1d0`), `enEDBGetDamageMotion` (`0x0014cb10`)
//!     - `EnEDBAnime` (`0x002aa010`) & `edb_anim` (`0x00395b30` / PC `0x006b28b8`, `lll_edi.anm`, 437 frames):
//!       - `aid = 5901`: Stand (`0..=9`, 10f, speed `384` = `5.625` fps, loop)
//!       - `aid = 5902`: Walk (`10..=33`, 24f, 30.0 fps, loop; footstep SE `18200` on frames 14 & 26)
//!       - `aid = 5903`: Run (`34..=57`, 24f, 30.0 fps, loop; footstep SE `18200` on frames 39 & 51)
//!       - `aid = 5904`: Aim Step Left (`58..=77`, 20f, 30.0 fps, single-shot)
//!       - `aid = 5905`: Aim Step Right (`78..=97`, 20f, 30.0 fps, single-shot)
//!       - `aid = 5906`: Shoot Revolver (`98..=121`, 24f, 30.0 fps, single-shot; `enEDBCtrlShoot`)
//!       - `aid = 5907`: Aim Hold / Recoil (`122..=163`, 42f, 30.0 fps, single-shot; `enEDBCtrlShoot`)
//!       - `aid = 5908`: Melee Punch (`164..=196`, 33f, speed `1536` = `22.5` fps, single-shot; `enEDBCtrlPunch`)
//!       - `aid = 5909`: Reload / Turn (`197..=256`, 60f, 30.0 fps, single-shot; triggers `enEDBSetCartridge` & SE `18202`)
//!       - `aid = 5910..=5921`: 12 directional damage reactions (`257..=436`, 15f each, 30.0 fps, single-shot)
//!       - `aid = 6801`: Defeat cutscene (`d_edb_anim` `0x00395c40`, `d_lll_edi.anm`, `0..=75`, 76f, 30.0 fps)
//!   - **Laura (`lau.mdl` / `rlau.mdl`, `model_id = 0x104`, 89 bones):**
//!     - `shCharacterHumanLAUAnimeSetP` (`0x00105810` / PC `0x00540e60`)
//!     - `plaura_anim` (`0x0038ae40` / PC `0x006e6784`, `p_lau.anm`, 1,128 B/frame, 73 frames):
//!       - `aid = 2901`: Walk / Idle (`0..=72`, 73f, 30.0 fps, loop)
//!     - `laura_anim` (`0x0038ac50` / PC `0x006e65a4`, `aid = 2501..=2540`, `dlaura_anime_adr_list` `0x002a88b0`)
//!   - **Mary Shepherd-Sunderland (`mry.mdl` / `rmry.mdl`, `model_id = 0x109`, 81 bones):**
//!     - `shCharacterHumanMRYAnimeSetP` (`0x002394f0` / PC `0x0053ad40`)
//!     - `pmary_anim` (`0x00399230` / PC `0x006d4ee4`, `p_mry.anm`, 1,028 B/frame, 135 frames):
//!       - `aid = 4091`: Walk A (`0..=66`, 67f, 30.0 fps, loop)
//!       - `aid = 4092`: Walk B (`67..=134`, 68f, 30.0 fps, loop)
//!     - `dmary_anim` (`0x00399130` / PC `0x006d4df4`, `aid = 4001..=4020`, `dmary_anime_adr_list` `0x0034e910`)

use crate::ai::creatures::anim::{CreatureAnimStateKind, CreatureAnimationController};
use crate::animation::AnimationClip;
use serde::{Deserialize, Serialize};

/// Clip definitions for Angela Orosco (`chr/agl/agl.mdl`, 75 bones, `p_agl.anm` 298 frames).
///
/// Sourced from PS2 `pangela_anim` (`0x00389240`) and `shCharacterHumanAGLAnimeSetP` (`0x001037e0`).
pub struct AngelaAnimClips;

impl AngelaAnimClips {
    /// Walk cycle A (`aid = 3901`, frames `0..=63`, 30.0 fps, looping).
    pub const WALK_A: AnimationClip = AnimationClip::new(0, 63, 30.0, true);
    /// Walk cycle B (`aid = 3903`, frames `64..=132`, 30.0 fps, looping).
    pub const WALK_B: AnimationClip = AnimationClip::new(64, 132, 30.0, true);
    /// Slow idle breathing pose (`aid = 3904`, frames `133..=206`, speed `128` = `1.875` fps, looping).
    pub const IDLE_BREATHE: AnimationClip = AnimationClip::new(133, 206, 1.875, true);
    /// Staircase ascent/descent motion (`aid = 3905`, frames `207..=297`, speed `128` = `1.875` fps, single-shot).
    pub const STAIR_WALK: AnimationClip = AnimationClip::new(207, 297, 1.875, false);

    /// Authentic forward root velocity from `shCharacterHumanAGLAnimeSetP` (`0x44f85432` = 1984.63 mm/s -> 66.15 mm/frame at 30 fps).
    pub const WALK_A_ROOT_MOTION_PER_FRAME: f32 = 66.15;
    /// Secondary walk root velocity (`0x44db7592` = 1755.67 mm/s -> 58.52 mm/frame at 30 fps).
    pub const WALK_B_ROOT_MOTION_PER_FRAME: f32 = 58.52;
}

/// Clip definitions for High-LOD Cutscene/NPC Eddie Dombrowski (`chr/edi/hhh_edi.mdl`, 64 bones, `p_edi.anm` 368 frames).
///
/// Sourced from PS2 `peddie_anim` (`0x0038aa90`) and `shCharacterHumanEDIAnimeSetP` (`0x00105020`).
pub struct EddieNpcAnimClips;

impl EddieNpcAnimClips {
    /// Heavy breathing idle A (`aid = 4801`, frames `0..=71`, speed `512` = `7.5` fps, looping).
    pub const IDLE_BREATHE_A: AnimationClip = AnimationClip::new(0, 71, 7.5, true);
    /// Heavy breathing idle B (`aid = 4802`, frames `72..=142`, speed `512` = `7.5` fps, looping).
    pub const IDLE_BREATHE_B: AnimationClip = AnimationClip::new(72, 142, 7.5, true);
    /// Dialogue gesture / talking cycle (frames `143..=224`, 30.0 fps, looping).
    pub const TALK_GESTURE: AnimationClip = AnimationClip::new(143, 224, 30.0, true);
    /// Seated / cowering posture (frames `225..=297`, 30.0 fps, looping).
    pub const COWER_SIT: AnimationClip = AnimationClip::new(225, 297, 30.0, true);
    /// Standing idle posture (frames `298..=365`, 30.0 fps, looping).
    pub const STAND_IDLE: AnimationClip = AnimationClip::new(298, 365, 30.0, true);
    /// 2-frame static reference pose (`aid = 4803`, frames `366..=367`, 30.0 fps, looping).
    pub const STATIC_POSE: AnimationClip = AnimationClip::new(366, 367, 30.0, true);
}

/// Clip definitions for Gameplay & Boss Eddie Dombrowski (`en_edb`, `chr/edi/lll_edi.mdl`, 35 bones, `lll_edi.anm` 437 frames).
///
/// Sourced from PS2 `edb_anim` (`0x00395b30`), `EnEDBAnime` (`0x002aa010`), `enEDBAnimeExec` (`0x0014d1d0`),
/// and `sh2pc.exe` `0x006b28b8`.
pub struct EddieBossAnimClips;

impl EddieBossAnimClips {
    /// Standing idle (`aid = 5901`, frames `0..=9`, speed `384` = `5.625` fps, looping).
    pub const STAND: AnimationClip = AnimationClip::new(0, 9, 5.625, true);
    /// Walk forward (`aid = 5902`, frames `10..=33`, 30.0 fps, looping; footstep SE `18200` on frames 14 & 26).
    pub const WALK: AnimationClip = AnimationClip::new(10, 33, 30.0, true);
    /// Run / rush (`aid = 5903`, frames `34..=57`, 30.0 fps, looping; footstep SE `18200` on frames 39 & 51).
    pub const RUN: AnimationClip = AnimationClip::new(34, 57, 30.0, true);
    /// Aim revolver while stepping left (`aid = 5904`, frames `58..=77`, 30.0 fps, single-shot).
    pub const AIM_STEP_LEFT: AnimationClip = AnimationClip::new(58, 77, 30.0, false);
    /// Aim revolver while stepping right (`aid = 5905`, frames `78..=97`, 30.0 fps, single-shot).
    pub const AIM_STEP_RIGHT: AnimationClip = AnimationClip::new(78, 97, 30.0, false);
    /// Fire revolver (`aid = 5906`, frames `98..=121`, 30.0 fps, single-shot; `enEDBCtrlShoot`).
    pub const SHOOT_REVOLVER: AnimationClip = AnimationClip::new(98, 121, 30.0, false);
    /// Post-shot recoil & aim hold (`aid = 5907`, frames `122..=163`, 30.0 fps, single-shot; `enEDBCtrlShoot`).
    pub const AIM_HOLD_RECOIL: AnimationClip = AnimationClip::new(122, 163, 30.0, false);
    /// Close-range revolver melee punch (`aid = 5908`, frames `164..=196`, speed `1536` = `22.5` fps, single-shot; `enEDBCtrlPunch`).
    pub const MELEE_PUNCH: AnimationClip = AnimationClip::new(164, 196, 22.5, false);
    /// Reload revolver / catch breath (`aid = 5909`, frames `197..=256`, 30.0 fps, single-shot; triggers `enEDBSetCartridge`).
    pub const RELOAD_TURN: AnimationClip = AnimationClip::new(197, 256, 30.0, false);

    /// Directional damage reaction clips (`aid = 5910..=5921`, 12 clips × 15 frames `257..=436`, 30.0 fps, single-shot).
    pub const DAMAGE_CLIPS: [AnimationClip; 12] = [
        AnimationClip::new(257, 271, 30.0, false), // 5910: Light Damage Front
        AnimationClip::new(272, 286, 30.0, false), // 5911: Light Damage Back
        AnimationClip::new(287, 301, 30.0, false), // 5912: Light Damage Left
        AnimationClip::new(302, 316, 30.0, false), // 5913: Light Damage Right
        AnimationClip::new(317, 331, 30.0, false), // 5914: Medium Damage Front
        AnimationClip::new(332, 346, 30.0, false), // 5915: Medium Damage Back
        AnimationClip::new(347, 361, 30.0, false), // 5916: Medium Damage Left
        AnimationClip::new(362, 376, 30.0, false), // 5917: Medium Damage Right
        AnimationClip::new(377, 391, 30.0, false), // 5918: Heavy Damage Front
        AnimationClip::new(392, 406, 30.0, false), // 5919: Heavy Damage Back
        AnimationClip::new(407, 421, 30.0, false), // 5920: Heavy Damage Left
        AnimationClip::new(422, 436, 30.0, false), // 5921: Heavy Damage Right
    ];

    /// Defeat cutscene clip (`d_edb_anim` `0x00395c40`, `aid = 6801`, `data/demo/killer_edi_d/lll_edi.anm`, frames `0..=75`).
    pub const DEATH_CUTSCENE: AnimationClip = AnimationClip::new(0, 75, 30.0, false);

    /// Root motion per frame for Eddie Boss walk (`enEDBGetWalkSpeed`).
    pub const WALK_ROOT_MOTION_PER_FRAME: f32 = 22.0;
    /// Root motion per frame for Eddie Boss run (`enEDBGetRunSpeed`).
    pub const RUN_ROOT_MOTION_PER_FRAME: f32 = 48.0;

    /// Active hit window for `SHOOT_REVOLVER` (`aid = 5906`, frames `102..=106`).
    pub const SHOOT_HIT_START: u32 = 102;
    pub const SHOOT_HIT_END: u32 = 106;

    /// Active hit window for `MELEE_PUNCH` (`aid = 5908`, frames `174..=182`).
    pub const PUNCH_HIT_START: u32 = 174;
    pub const PUNCH_HIT_END: u32 = 182;

    /// Cartridge eject/reload trigger frame in `RELOAD_TURN` (`aid = 5909`, `19200 / 1024 = 18` frames into clip -> frame 215).
    pub const RELOAD_CARTRIDGE_FRAME: u32 = 215;
}

/// Clip definitions for Laura (`chr/lau/lau.mdl`, 89 bones, `p_lau.anm` 73 frames).
///
/// Sourced from PS2 `plaura_anim` (`0x0038ae40`) and `shCharacterHumanLAUAnimeSetP` (`0x00105810`).
pub struct LauraAnimClips;

impl LauraAnimClips {
    /// Playable-scene walk / idle loop (`aid = 2901`, frames `0..=72`, 30.0 fps, looping).
    pub const WALK_IDLE: AnimationClip = AnimationClip::new(0, 72, 30.0, true);
    /// Extended drama walk clip (`aid = 2902`, frames `0..=90` when bound to drama track, 30.0 fps, looping).
    pub const DRAMA_WALK: AnimationClip = AnimationClip::new(0, 90, 30.0, true);

    /// Root motion per frame along local forward (`0x43fa0000` = 500.0 mm/s -> 16.67 mm/frame at 30 fps).
    pub const WALK_ROOT_MOTION_PER_FRAME: f32 = 16.67;
}

/// Clip definitions for Mary Shepherd-Sunderland (`chr/mry/mry.mdl`, 81 bones, `p_mry.anm` 135 frames).
///
/// Sourced from PS2 `pmary_anim` (`0x00399230`) and `shCharacterHumanMRYAnimeSetP` (`0x002394f0`).
pub struct MaryNpcAnimClips;

impl MaryNpcAnimClips {
    /// Walk / apparition cycle A (`aid = 4091`, frames `0..=66`, 30.0 fps, looping).
    pub const WALK_A: AnimationClip = AnimationClip::new(0, 66, 30.0, true);
    /// Walk / apparition cycle B (`aid = 4092`, frames `67..=134`, 30.0 fps, looping).
    pub const WALK_B: AnimationClip = AnimationClip::new(67, 134, 30.0, true);

    /// Root motion per frame (`0x43fa0000` = 500.0 mm/s -> 16.67 mm/frame at 30 fps).
    pub const WALK_ROOT_MOTION_PER_FRAME: f32 = 16.67;
}

/// Story NPC state for non-combat playable/cutscene character animation synchronization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NpcAnimState {
    /// Default standing or breathing idle (`3904` for Angela, `4801` for Eddie, `2901` for Laura, `4091` for Mary).
    Idle,
    /// Primary walk cycle (`3901` for Angela, `2901` for Laura, `4091` for Mary).
    WalkPrimary,
    /// Secondary walk or breathing variant (`3903` for Angela, `4802` for Eddie, `4092` for Mary).
    WalkSecondary,
    /// Dialogue / talking gesture (`143..=224` for Eddie, `STAIR_WALK` for Angela).
    TalkOrGesture,
    /// Cowering or seated pose (`225..=297` for Eddie).
    CowerOrSpecial,
}

/// Authentic AI & combat state machine states for Eddie Boss (`en_edb`, `SLUS_202.28` `0x001494c0..0x0014d900`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EdbState {
    /// Standing idle (`enEDBCtrlAutomatic`, `aid = 5901`).
    Stand,
    /// Walking approach or search (`enEDBCtrlApproach` / `enEDBCtrlSearch` / `enEDBCtrlAway`, `aid = 5902`).
    Walk,
    /// Running between meat-locker cover positions (`enEDBCtrlHidden` / `enEDBCtrlEscape` / `enEDBCtrlEscape2`, `aid = 5903`).
    Run,
    /// Stepping left while aiming revolver (`aid = 5904`).
    AimStepLeft,
    /// Stepping right while aiming revolver (`aid = 5905`).
    AimStepRight,
    /// Firing revolver at James (`enEDBCtrlShoot`, `aid = 5906`).
    ShootRevolver,
    /// Post-shot recoil & aim hold (`enEDBCtrlShoot`, `aid = 5907`).
    AimHoldRecoil,
    /// Close-quarters revolver melee punch (`enEDBCtrlPunch`, `aid = 5908`).
    MeleePunch,
    /// Reloading revolver / turning (`aid = 5909`, calls `enEDBSetCartridge`).
    ReloadTurn,
    /// Directional damage hit reaction (`enEDBCtrlDamage` / `enEDBGetDamageMotion`, index `0..12` -> `aid = 5910..=5921`).
    Damage(u8),
}

impl CreatureAnimationController {
    /// Factory for Angela Orosco (`agl.mdl`, 75 bones).
    pub fn new_angela() -> Self {
        Self::new(
            CreatureAnimStateKind::Idle,
            AngelaAnimClips::IDLE_BREATHE,
            0.0,
        )
    }

    /// Factory for High-LOD Cutscene/NPC Eddie (`hhh_edi.mdl`, 64 bones).
    pub fn new_eddie_npc() -> Self {
        Self::new(
            CreatureAnimStateKind::Idle,
            EddieNpcAnimClips::IDLE_BREATHE_A,
            0.0,
        )
    }

    /// Factory for Gameplay/Boss Eddie (`en_edb`, `lll_edi.mdl`, 35 bones).
    pub fn new_eddie_boss() -> Self {
        Self::new(CreatureAnimStateKind::Idle, EddieBossAnimClips::STAND, 0.0)
    }

    /// Factory for Laura (`lau.mdl`, 89 bones).
    pub fn new_laura() -> Self {
        Self::new(CreatureAnimStateKind::Idle, LauraAnimClips::WALK_IDLE, 0.0)
    }

    /// Factory for Mary Shepherd-Sunderland (`mry.mdl`, 81 bones).
    pub fn new_mary_npc() -> Self {
        Self::new(CreatureAnimStateKind::Idle, MaryNpcAnimClips::WALK_A, 0.0)
    }

    /// Synchronize Angela's skeletal animation controller with her [`NpcAnimState`].
    pub fn sync_angela(&mut self, state: NpcAnimState) {
        match state {
            NpcAnimState::Idle | NpcAnimState::CowerOrSpecial => {
                self.transition_to(
                    CreatureAnimStateKind::Idle,
                    AngelaAnimClips::IDLE_BREATHE,
                    0.0,
                    None,
                    None,
                );
            }
            NpcAnimState::WalkPrimary => {
                self.transition_to(
                    CreatureAnimStateKind::Locomotion,
                    AngelaAnimClips::WALK_A,
                    AngelaAnimClips::WALK_A_ROOT_MOTION_PER_FRAME,
                    None,
                    None,
                );
            }
            NpcAnimState::WalkSecondary => {
                self.transition_to(
                    CreatureAnimStateKind::Locomotion,
                    AngelaAnimClips::WALK_B,
                    AngelaAnimClips::WALK_B_ROOT_MOTION_PER_FRAME,
                    None,
                    None,
                );
            }
            NpcAnimState::TalkOrGesture => {
                self.transition_to(
                    CreatureAnimStateKind::Locomotion,
                    AngelaAnimClips::STAIR_WALK,
                    0.0,
                    None,
                    None,
                );
            }
        }
    }

    /// Synchronize High-LOD NPC Eddie's skeletal animation controller with his [`NpcAnimState`].
    pub fn sync_eddie_npc(&mut self, state: NpcAnimState) {
        match state {
            NpcAnimState::Idle => {
                self.transition_to(
                    CreatureAnimStateKind::Idle,
                    EddieNpcAnimClips::IDLE_BREATHE_A,
                    0.0,
                    None,
                    None,
                );
            }
            NpcAnimState::WalkPrimary | NpcAnimState::WalkSecondary => {
                self.transition_to(
                    CreatureAnimStateKind::Locomotion,
                    EddieNpcAnimClips::IDLE_BREATHE_B,
                    0.0,
                    None,
                    None,
                );
            }
            NpcAnimState::TalkOrGesture => {
                self.transition_to(
                    CreatureAnimStateKind::Idle,
                    EddieNpcAnimClips::TALK_GESTURE,
                    0.0,
                    None,
                    None,
                );
            }
            NpcAnimState::CowerOrSpecial => {
                self.transition_to(
                    CreatureAnimStateKind::Cower,
                    EddieNpcAnimClips::COWER_SIT,
                    0.0,
                    None,
                    None,
                );
            }
        }
    }

    /// Synchronize Eddie Boss (`en_edb`, `lll_edi.mdl`) with his [`EdbState`] combat state machine.
    pub fn sync_eddie_boss(&mut self, state: EdbState) {
        match state {
            EdbState::Stand => {
                self.transition_to(
                    CreatureAnimStateKind::Idle,
                    EddieBossAnimClips::STAND,
                    0.0,
                    None,
                    None,
                );
            }
            EdbState::Walk => {
                self.transition_to(
                    CreatureAnimStateKind::Locomotion,
                    EddieBossAnimClips::WALK,
                    EddieBossAnimClips::WALK_ROOT_MOTION_PER_FRAME,
                    None,
                    None,
                );
            }
            EdbState::Run => {
                self.transition_to(
                    CreatureAnimStateKind::Locomotion,
                    EddieBossAnimClips::RUN,
                    EddieBossAnimClips::RUN_ROOT_MOTION_PER_FRAME,
                    None,
                    None,
                );
            }
            EdbState::AimStepLeft => {
                self.transition_to(
                    CreatureAnimStateKind::Locomotion,
                    EddieBossAnimClips::AIM_STEP_LEFT,
                    0.0,
                    None,
                    None,
                );
            }
            EdbState::AimStepRight => {
                self.transition_to(
                    CreatureAnimStateKind::Locomotion,
                    EddieBossAnimClips::AIM_STEP_RIGHT,
                    0.0,
                    None,
                    None,
                );
            }
            EdbState::ShootRevolver => {
                self.transition_to(
                    CreatureAnimStateKind::AttackPrimary,
                    EddieBossAnimClips::SHOOT_REVOLVER,
                    0.0,
                    Some((
                        EddieBossAnimClips::SHOOT_HIT_START,
                        EddieBossAnimClips::SHOOT_HIT_END,
                    )),
                    None,
                );
            }
            EdbState::AimHoldRecoil => {
                self.transition_to(
                    CreatureAnimStateKind::Recovery,
                    EddieBossAnimClips::AIM_HOLD_RECOIL,
                    0.0,
                    None,
                    None,
                );
            }
            EdbState::MeleePunch => {
                self.transition_to(
                    CreatureAnimStateKind::AttackSecondary,
                    EddieBossAnimClips::MELEE_PUNCH,
                    0.0,
                    Some((
                        EddieBossAnimClips::PUNCH_HIT_START,
                        EddieBossAnimClips::PUNCH_HIT_END,
                    )),
                    None,
                );
            }
            EdbState::ReloadTurn => {
                self.transition_to(
                    CreatureAnimStateKind::Recovery,
                    EddieBossAnimClips::RELOAD_TURN,
                    0.0,
                    None,
                    Some((
                        EddieBossAnimClips::RELOAD_CARTRIDGE_FRAME,
                        EddieBossAnimClips::RELOAD_CARTRIDGE_FRAME + 2,
                    )),
                );
            }
            EdbState::Damage(variant) => {
                let idx = (variant as usize).min(EddieBossAnimClips::DAMAGE_CLIPS.len() - 1);
                self.transition_to(
                    CreatureAnimStateKind::Flinch,
                    EddieBossAnimClips::DAMAGE_CLIPS[idx],
                    0.0,
                    None,
                    None,
                );
            }
        }
    }

    /// Synchronize Laura's skeletal animation controller with her [`NpcAnimState`].
    pub fn sync_laura(&mut self, state: NpcAnimState) {
        match state {
            NpcAnimState::Idle | NpcAnimState::TalkOrGesture | NpcAnimState::CowerOrSpecial => {
                self.transition_to(
                    CreatureAnimStateKind::Idle,
                    LauraAnimClips::WALK_IDLE,
                    0.0,
                    None,
                    None,
                );
            }
            NpcAnimState::WalkPrimary | NpcAnimState::WalkSecondary => {
                self.transition_to(
                    CreatureAnimStateKind::Locomotion,
                    LauraAnimClips::WALK_IDLE,
                    LauraAnimClips::WALK_ROOT_MOTION_PER_FRAME,
                    None,
                    None,
                );
            }
        }
    }

    /// Synchronize Mary's skeletal animation controller with her [`NpcAnimState`].
    pub fn sync_mary_npc(&mut self, state: NpcAnimState) {
        match state {
            NpcAnimState::Idle | NpcAnimState::TalkOrGesture | NpcAnimState::CowerOrSpecial => {
                self.transition_to(
                    CreatureAnimStateKind::Idle,
                    MaryNpcAnimClips::WALK_A,
                    0.0,
                    None,
                    None,
                );
            }
            NpcAnimState::WalkPrimary => {
                self.transition_to(
                    CreatureAnimStateKind::Locomotion,
                    MaryNpcAnimClips::WALK_A,
                    MaryNpcAnimClips::WALK_ROOT_MOTION_PER_FRAME,
                    None,
                    None,
                );
            }
            NpcAnimState::WalkSecondary => {
                self.transition_to(
                    CreatureAnimStateKind::Locomotion,
                    MaryNpcAnimClips::WALK_B,
                    MaryNpcAnimClips::WALK_ROOT_MOTION_PER_FRAME,
                    None,
                    None,
                );
            }
        }
    }
}
