//! Scene-specific widescreen (`16:9`) and ultrawide (`21:9` / `32:9`) cutscene staging, prop
//! culling, draw-distance, and fog boundary fixes (`SH2-77`).
//!
//! # Ground Truth & SH2:EE Reference
//! 1. **Wood Side Room 307 Closet Cutscene (`ClosetCutsceneFix`, `sankaku1` / `wp_closet`, `demo_no = 14`, `CS_APT_RPT_CLOSET = 0x0E`)**:
//!    - `RPTClosetCutsceneMannequinDespawn` (`FogAdjustments.cpp:427`): Despawns the Mannequin (`mkn_1_pos` / `mkn_2_pos`)
//!      at `scene_frame >= 359.0` (`0x43B38000`) instead of `360.0` so it vanishes 1 frame before the hard camera cut
//!      at frame 360 exposes it parked off-screen in widescreen.
//!    - `RPTClosetCutsceneBlurredBarsDespawn` (`FogAdjustments.cpp:446` & `ClosetCutscene.cpp:33`): Despawns the
//!      blurred closet bars (`b_doo_pos` / `ClosetCutsceneObject`) at `scene_frame >= 1695.0` (`0x44D3E000`) instead
//!      of `1696.0`, and suppresses drawing the blurred bars object when camera X == `-21376.56445f`.
//!    - Flashlight suppression (`ClosetCutscene.cpp:164`): Disables James's flashlight when camera X == `-22256.61133f`.
//!    - First-person closet bar blur overlay (`SpecialFX.cpp:119..180` `CustomAddress2ASM` / `CustomAddress3ASM` &
//!      `ClosetCutscene.cpp:190..226`): Stretches/repeats the first-person closet bar blur overlay across the full
//!      ultrawide viewport width (`viewport_aspect / (4.0 / 3.0)`) and renders the film grain layer on top of the blur layer.
//! 2. **Labyrinth Abstract Daddy / Angela Cutscene (`PistonRoomFix`, `papa_agl` / `ug_angela`, `demo_no = 68`, `CS_PS_ANGELA_BOSS_FINISH = 0x44`)**:
//!    - `PistonRoom.cpp:35..58`: Once camera X reaches `20278.414` (`0x469E6CD4`, the shot where Angela opens the door
//!      to leave the piston room), the out-of-bounds wall piston prop (`PistonItem`, index 1 in `PistonList`) behind
//!      the open doorway is culled / offset by `+2.50` for the remainder of the cutscene.
//! 3. **Final Boss Stage (`FixFinalBossRoom`, `R_FINAL_BOSS_RM = 0xBB` / stage `ru73`)**:
//!    - `FinalBossRoomFix.cpp` & `FogAdjustments.cpp:279..315`: When `james_pos_y < -14600.0` in `R_FINAL_BOSS_RM`,
//!      lowers `FinalBossBottomWalkwaySpawn` (`-8000.0 -> -30000.0`), `FinalBossBottomFloorSpawn` (`-4000.0 -> -30000.0`),
//!      and `FinalBossBlackBoxSpawn` (`-12000.0 -> -30000.0`) to `-30000.0` and extends `FinalBossDrawDistance` to `30000.0`
//!      so the bottom metal grating and black skybox never pop out on wide vertical/horizontal frustums.
//! 4. **Observation Deck & Rosewater Park Fog (`ObservationDeckFogFix` & `MeetingMariaCutsceneFogCounter`, `ob01` `demo_no = 2` & `deai` `demo_no = 22`)**:
//!    - `ObservationDeckFogFix.cpp`: Expands the Observation Deck (`ob01` / `tenbou`) horizontal fog particle volume
//!      bounds from `±6500.0` to `kObservationDeckMaxPosX = 24500.0` with border fade `kFogBorderWidth = 300.0` and
//!      `kFogParticleCountMax = 2000`.
//!    - `FogAdjustments.cpp:391..412`: Adjusts `MeetingMariaCutsceneFogCounterOne` (`968.0 -> 965.5`, `0x44716000`)
//!      and `MeetingMariaCutsceneFogCounterTwo` (`1463.0 -> 1460.0`, `0x44B68000`) during `demo_no == 22` (`0x16`, `mar_deai_e`)
//!      so fog spans `21:9` and `32:9` ultrawide viewports without early cut-off.

use sh2_assets::formats::demo::AUTHORED_4X3_ASPECT;

/// `demo_no == 2` (`tenbou` / `ob01` Observation Deck monologue).
pub const CS_OB_VIEW_NEIGHBORHOOD_DEMO_NO: u16 = 2;

/// `demo_no == 14` (`0x0E`, `CS_APT_RPT_CLOSET`, `sankaku1` / `wp_closet` Wood Side Room 307 Closet Cutscene).
pub const CS_APT_RPT_CLOSET_DEMO_NO: u16 = 14;

/// `demo_no == 22` (`0x16`, `mar_deai_e` / `deai` Rosewater Park Meeting Maria Part 2).
pub const CS_TOWN_WEST_MARIA_MEET_DEMO_NO: u16 = 22;

/// `demo_no == 68` (`0x44`, `CS_PS_ANGELA_BOSS_FINISH`, `papa_agl` / `ug_angela` Labyrinth Abstract Daddy & Angela).
pub const CS_PS_ANGELA_BOSS_FINISH_DEMO_NO: u16 = 68;

/// Room ID for Wood Side Apartments Room 307 (`R_APT_E_RM_307 = 0x13` in `Patches.h`).
pub const R_APT_E_RM_307_ID: u32 = 0x13;

/// Room ID for the Final Boss metal grating arena (`R_FINAL_BOSS_RM = 0xBB` in `Patches.h`).
pub const R_FINAL_BOSS_RM_ID: u32 = 0xBB;

// ---------------------------------------------------------------------------
// 1. Wood Side Room 307 Closet Cutscene (`ClosetCutsceneFix`, `demo_no = 14`)
// ---------------------------------------------------------------------------

/// Original retail frame (`360.0`) at which the Mannequin despawns in `sankaku.dds`.
pub const CLOSET_MANNEQUIN_ORIG_DESPAWN_FRAME: f32 = 360.0;

/// Fixed widescreen frame (`359.0`, `0x43B38000` in `FogAdjustments.cpp`) at which the Mannequin
/// (`mkn_1_pos` / `mkn_2_pos`) despawns 1 frame before the hard camera cut at frame 360.
pub const CLOSET_MANNEQUIN_FIXED_DESPAWN_FRAME: f32 = 359.0;

/// Original retail frame (`1696.0`) at which the blurred closet bars despawn in `sankaku.dds`.
pub const CLOSET_BLURRED_BARS_ORIG_DESPAWN_FRAME: f32 = 1696.0;

/// Fixed widescreen frame (`1695.0`, `0x44D3E000` in `FogAdjustments.cpp`) at which the blurred
/// closet bars (`b_doo_pos`) despawn 1 frame before the hard camera cut at frame 1696.
pub const CLOSET_BLURRED_BARS_FIXED_DESPAWN_FRAME: f32 = 1695.0;

/// Camera X coordinate (`-21376.56445f` in `ClosetCutscene.cpp:33`) where the 3D blurred closet bars
/// object must be hidden to prevent double-bar artifacts.
pub const CLOSET_BLURRED_BARS_HIDE_CAM_X: f32 = -21_376.564;

/// Camera X coordinate (`-22256.61133f` in `ClosetCutscene.cpp:164`) where James's flashlight must
/// be suppressed inside the closet.
pub const CLOSET_DISABLE_FLASHLIGHT_CAM_X: f32 = -22_256.611;

/// Camera X coordinate (`-20133.99805f` in `DisableShadowCutscene.cpp:45`) where self-shadowing
/// inside the closet is suppressed.
pub const CLOSET_DISABLE_SHADOW_CAM_X: f32 = -20_133.998;

/// Evaluated per-frame state of `ClosetCutsceneFix` (`sankaku1` / `wp_closet`, `demo_no = 14`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClosetCutsceneFixState {
    /// Whether this cutscene is `CS_APT_RPT_CLOSET` (`demo_no == 14`).
    pub active: bool,
    /// Whether the Mannequin (`mkn_1_pos` / `mkn_2_pos`) is despawned by the `359.0` frame threshold.
    pub mannequin_despawned: bool,
    /// Whether the 3D blurred closet bars (`b_doo_pos`) are hidden (`scene_frame >= 1695.0` or `cam_x == -21376.56445`).
    pub blurred_bars_hidden: bool,
    /// Whether James's flashlight is suppressed for the current camera shot (`cam_x == -22256.61133`).
    pub flashlight_suppressed: bool,
    /// Whether self-shadows are suppressed for the current camera shot (`cam_x == -20133.99805`).
    pub shadow_suppressed: bool,
    /// Whether the first-person closet bar blur overlay is active (`scene_frame < 1695.0` while looking through bars).
    pub first_person_blur_overlay_active: bool,
    /// Horizontal UV / quad width scale factor (`(viewport_aspect / (4.0 / 3.0)).max(1.0)`) stretching
    /// the first-person closet bar blur overlay across the full widescreen/ultrawide viewport (`SpecialFX.cpp`).
    pub blur_overlay_width_scale: f32,
    /// Whether the PS2 film grain / noise filter must be composited on top of the closet bar blur layer (`ClosetCutscene.cpp:209`).
    pub render_grain_above_blur: bool,
}

impl Default for ClosetCutsceneFixState {
    fn default() -> Self {
        Self {
            active: false,
            mannequin_despawned: false,
            blurred_bars_hidden: false,
            flashlight_suppressed: false,
            shadow_suppressed: false,
            first_person_blur_overlay_active: false,
            blur_overlay_width_scale: 1.0,
            render_grain_above_blur: true,
        }
    }
}

impl ClosetCutsceneFixState {
    /// Evaluates `ClosetCutsceneFix` at `(demo_no, scene_frame, camera_pos_x, viewport_aspect)`.
    pub fn evaluate(
        demo_no: u16,
        scene_frame: f32,
        camera_pos_x: f32,
        viewport_aspect: f32,
    ) -> Self {
        if demo_no != CS_APT_RPT_CLOSET_DEMO_NO {
            return Self::default();
        }
        let mannequin_despawned = scene_frame >= CLOSET_MANNEQUIN_FIXED_DESPAWN_FRAME;
        let at_bars_hide_cam = (camera_pos_x - CLOSET_BLURRED_BARS_HIDE_CAM_X).abs() < 1.0;
        let blurred_bars_hidden =
            scene_frame >= CLOSET_BLURRED_BARS_FIXED_DESPAWN_FRAME || at_bars_hide_cam;
        let flashlight_suppressed = (camera_pos_x - CLOSET_DISABLE_FLASHLIGHT_CAM_X).abs() < 1.0;
        let shadow_suppressed = (camera_pos_x - CLOSET_DISABLE_SHADOW_CAM_X).abs() < 1.0;
        let first_person_blur_overlay_active =
            scene_frame < CLOSET_BLURRED_BARS_FIXED_DESPAWN_FRAME && at_bars_hide_cam;
        let blur_overlay_width_scale = (viewport_aspect / AUTHORED_4X3_ASPECT).max(1.0);

        Self {
            active: true,
            mannequin_despawned,
            blurred_bars_hidden,
            flashlight_suppressed,
            shadow_suppressed,
            first_person_blur_overlay_active,
            blur_overlay_width_scale,
            render_grain_above_blur: true,
        }
    }

    /// Returns `true` if a specific `.dds` actor slot should be hidden by `ClosetCutsceneFix`.
    #[inline]
    pub fn should_hide_actor(&self, actor_name: &str) -> bool {
        if !self.active {
            return false;
        }
        let clean = actor_name.trim_matches('\0').trim();
        if self.mannequin_despawned
            && (clean.eq_ignore_ascii_case("mkn_1_pos") || clean.eq_ignore_ascii_case("mkn_2_pos"))
        {
            return true;
        }
        if self.blurred_bars_hidden && clean.eq_ignore_ascii_case("b_doo_pos") {
            return true;
        }
        false
    }
}

// ---------------------------------------------------------------------------
// 2. Labyrinth Abstract Daddy / Angela Cutscene (`PistonRoomFix`, `demo_no = 68`)
// ---------------------------------------------------------------------------

/// Camera X coordinate (`20278.414`, `0x469E6CD4` in `PistonRoom.cpp:40`) where Angela opens the door
/// in `CS_PS_ANGELA_BOSS_FINISH` (`papa_agl.dds` offset `0x241b7`), exposing the out-of-bounds wall piston.
pub const PISTON_ROOM_DOOR_OPEN_CAM_X: f32 = f32::from_bits(0x469E_6CD4);

/// Offset (`+2.50` in `PistonRoom.cpp:52`) / cull displacement applied to the doorway wall piston (`PistonList[1]`).
pub const PISTON_DOORWAY_PROP_OFFSET: f32 = 2.50;

/// Stateful latch for `PistonRoomFix` during `CS_PS_ANGELA_BOSS_FINISH` (`papa_agl` / `ug_angela`, `demo_no = 68`).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PistonRoomFixState {
    /// Whether this cutscene is `CS_PS_ANGELA_BOSS_FINISH` (`demo_no == 68`).
    pub active: bool,
    /// Latches `true` as soon as camera X hits `20278.414` (`0x469E6CD4`) when Angela opens the door,
    /// remaining `true` until the cutscene finishes (`PistonRoom.cpp:40..58`).
    pub doorway_piston_culled: bool,
    /// Piston doorway prop offset (`2.50` when latched, `0.0` prior).
    pub doorway_piston_offset: f32,
}

impl PistonRoomFixState {
    /// Updates the `PistonRoomFix` latch for `(demo_no, camera_pos_x)`.
    pub fn update(&mut self, demo_no: u16, camera_pos_x: f32) {
        if demo_no != CS_PS_ANGELA_BOSS_FINISH_DEMO_NO {
            *self = Self::default();
            return;
        }
        self.active = true;
        if (camera_pos_x - PISTON_ROOM_DOOR_OPEN_CAM_X).abs() < 1.0 {
            self.doorway_piston_culled = true;
        }
        self.doorway_piston_offset = if self.doorway_piston_culled {
            PISTON_DOORWAY_PROP_OFFSET
        } else {
            0.0
        };
    }
}

// ---------------------------------------------------------------------------
// 3. Final Boss Stage (`FixFinalBossRoom`, `R_FINAL_BOSS_RM = 0xBB`)
// ---------------------------------------------------------------------------

/// James Y-elevation threshold (`-14600.0` in `FinalBossRoomFix.cpp:30`) above which the player is
/// on the upper scaffolding of the Final Boss arena (`R_FINAL_BOSS_RM`).
pub const FINAL_BOSS_JAMES_ELEVATION_THRESHOLD_Y: f32 = -14_600.0;

/// Extended spawn Y threshold (`-30000.0` in `FogAdjustments.cpp:286..300`) ensuring the bottom
/// walkway, bottom floor, and black skybox always spawn regardless of James's elevation in widescreen.
pub const FINAL_BOSS_LOWERED_SPAWN_Y: f32 = -30_000.0;

/// Extended draw distance (`30000.0` in `FinalBossRoomFix.cpp:33`) for `R_FINAL_BOSS_RM`.
pub const FINAL_BOSS_EXTENDED_DRAW_DISTANCE: f32 = 30_000.0;

/// Evaluated draw distance and geometry spawn thresholds for `FixFinalBossRoom` (`SH2-77`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FinalBossRoomFixState {
    /// Whether the active stage is `R_FINAL_BOSS_RM` (`0xBB` / `ru73`).
    pub in_final_boss_room: bool,
    /// `FinalBossBottomWalkwaySpawn` (`-30000.0` when `james_pos_y < -14600.0`, else `-8000.0`).
    pub bottom_walkway_spawn_y: f32,
    /// `FinalBossBottomFloorSpawn` (`-30000.0` when `james_pos_y < -14600.0`, else `-4000.0`).
    pub bottom_floor_spawn_y: f32,
    /// `FinalBossBlackBoxSpawn` (`-30000.0` when `james_pos_y < -14600.0`, else `-12000.0`).
    pub black_box_spawn_y: f32,
    /// Camera far draw distance (`30000.0` when `james_pos_y < -14600.0` in `R_FINAL_BOSS_RM`, else `25000.0`).
    pub draw_distance: f32,
}

impl FinalBossRoomFixState {
    /// Evaluates `FixFinalBossRoom` matching `FinalBossRoomFix.cpp` and `FogAdjustments.cpp`.
    pub fn evaluate(room_id: u32, james_pos_y: f32) -> Self {
        let in_final_boss_room = room_id == R_FINAL_BOSS_RM_ID;
        if in_final_boss_room && james_pos_y < FINAL_BOSS_JAMES_ELEVATION_THRESHOLD_Y {
            Self {
                in_final_boss_room: true,
                bottom_walkway_spawn_y: FINAL_BOSS_LOWERED_SPAWN_Y,
                bottom_floor_spawn_y: FINAL_BOSS_LOWERED_SPAWN_Y,
                black_box_spawn_y: FINAL_BOSS_LOWERED_SPAWN_Y,
                draw_distance: FINAL_BOSS_EXTENDED_DRAW_DISTANCE,
            }
        } else {
            Self {
                in_final_boss_room,
                bottom_walkway_spawn_y: -8_000.0,
                bottom_floor_spawn_y: -4_000.0,
                black_box_spawn_y: -12_000.0,
                draw_distance: if in_final_boss_room {
                    FINAL_BOSS_EXTENDED_DRAW_DISTANCE
                } else {
                    25_000.0
                },
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 4. Observation Deck & Rosewater Park Fog (`ObservationDeckFogFix` & `MeetingMariaCutsceneFog`)
// ---------------------------------------------------------------------------

/// Original `4:3` Observation Deck (`ob01` / `tenbou`) horizontal fog bound (`6500.0`).
pub const OBSERVATION_DECK_ORIG_MAX_POS_X: f32 = 6_500.0;

/// Ultrawide Observation Deck (`ob01` / `tenbou`) horizontal fog bound (`kObservationDeckMaxPosX = 24500.0f`
/// in `ObservationDeckFogFix.cpp:17`), spanning `16:9`, `21:9`, and `32:9` viewports.
pub const OBSERVATION_DECK_ULTRAWIDE_MAX_POS_X: f32 = 24_500.0;

/// Smooth alpha fade border width (`kFogBorderWidth = 300.0f` in `ObservationDeckFogFix.cpp:18`).
pub const OBSERVATION_DECK_FOG_BORDER_WIDTH: f32 = 300.0;

/// Expanded particle budget (`kFogParticleCountMax = 2000` in `ObservationDeckFogFix.cpp:19`).
pub const OBSERVATION_DECK_FOG_PARTICLE_MAX: usize = 2_000;

/// Original retail `MeetingMariaCutsceneFogCounterOne` (`968.0`) in `deai` (`demo_no = 22`).
pub const MEETING_MARIA_FOG_COUNTER_ONE_ORIG: f32 = 968.0;

/// Fixed widescreen `MeetingMariaCutsceneFogCounterOne` (`965.5`, `0x44716000` in `FogAdjustments.cpp:395`).
pub const MEETING_MARIA_FOG_COUNTER_ONE_FIXED: f32 = 965.5;

/// Original retail `MeetingMariaCutsceneFogCounterTwo` (`1463.0`) in `deai` (`demo_no = 22`).
pub const MEETING_MARIA_FOG_COUNTER_TWO_ORIG: f32 = 1_463.0;

/// Fixed widescreen `MeetingMariaCutsceneFogCounterTwo` (`1460.0`, `0x44B68000` in `FogAdjustments.cpp:408`).
pub const MEETING_MARIA_FOG_COUNTER_TWO_FIXED: f32 = 1_460.0;

/// Returns the Observation Deck (`ob01`) horizontal fog volume bounds `(min_x, max_x) = (-24500.0, 24500.0)`.
#[inline]
pub const fn observation_deck_fog_bounds_x() -> (f32, f32) {
    (
        -OBSERVATION_DECK_ULTRAWIDE_MAX_POS_X,
        OBSERVATION_DECK_ULTRAWIDE_MAX_POS_X,
    )
}

/// Wraps an Observation Deck fog particle X coordinate into `[-24500.0, 24500.0]`, matching
/// `WrapValue(pos.x, -kObservationDeckMaxPosX, kObservationDeckMaxPosX)` in `ObservationDeckFogFix.cpp:25..36`.
#[inline]
pub fn observation_deck_wrap_particle_x(pos_x: f32) -> f32 {
    let min = -OBSERVATION_DECK_ULTRAWIDE_MAX_POS_X;
    let max = OBSERVATION_DECK_ULTRAWIDE_MAX_POS_X;
    let width = max - min;
    if pos_x < min {
        max - (min - pos_x).rem_euclid(width)
    } else if pos_x > max {
        min + (pos_x - max).rem_euclid(width)
    } else {
        pos_x
    }
}

/// Computes the smooth border alpha fade factor `[0.0, 1.0]` for an Observation Deck fog particle
/// at `(pos_x, pos_z)` within `[-24500.0, 24500.0] x [min_z, max_z]`, matching `UpdateObservationDeckFogParticleFadeASM`
/// (`ObservationDeckFogFix.cpp:40..72`).
#[inline]
pub fn observation_deck_border_alpha_factor(pos_x: f32, pos_z: f32, min_z: f32, max_z: f32) -> f32 {
    let dist_x = (OBSERVATION_DECK_ULTRAWIDE_MAX_POS_X - pos_x.abs()).max(0.0);
    let dist_z = (pos_z - min_z).min(max_z - pos_z).max(0.0);
    let min_dist = dist_x.min(dist_z);
    if min_dist >= OBSERVATION_DECK_FOG_BORDER_WIDTH {
        1.0
    } else {
        (min_dist / OBSERVATION_DECK_FOG_BORDER_WIDTH).clamp(0.0, 1.0)
    }
}

/// Evaluates whether the Rosewater Park `MeetingMaria` (`demo_no == 22`, `mar_deai_e`) widescreen
/// fog layer triggers are active at `scene_frame`, using SH2:EE's widened frame thresholds
/// (`965.5` instead of `968.0`, and `1460.0` instead of `1463.0`).
#[inline]
pub fn evaluate_meeting_maria_fog_counters(demo_no: u16, scene_frame: f32) -> (bool, bool) {
    if demo_no != CS_TOWN_WEST_MARIA_MEET_DEMO_NO {
        return (false, false);
    }
    (
        scene_frame >= MEETING_MARIA_FOG_COUNTER_ONE_FIXED,
        scene_frame >= MEETING_MARIA_FOG_COUNTER_TWO_FIXED,
    )
}
