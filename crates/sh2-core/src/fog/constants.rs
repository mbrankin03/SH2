//! Authentic PS2 (`SLUS_202.28`), retail PC (`sh2pc.exe`), and SH2:EE (`Patches/`) fog constants
//! and room identifiers (`SH2-32`, `SH2-153`).

/// Room ID for Opening Restroom (`er01.map`, `RoomNameJms() == 1` in `PlayerSetReverseMode` `0x0024c190` line 198).
pub const R_RESTROOM_START_ID: u32 = 0x01;
/// Room ID for Toluca Lake Observation Deck (`R_OBSV_DECK = 0x02` in `Patches.h`).
pub const R_OBSV_DECK_ID: u32 = 0x02;
/// Room ID for Forest Trail & Cemetery (`R_FOREST_CEMETERY = 0x03` in `Patches.h`).
pub const R_FOREST_CEMETERY_ID: u32 = 0x03;
/// Room ID for Silent Hill Town East (`R_TOWN_EAST = 0x04` in `Patches.h`).
pub const R_TOWN_EAST_ID: u32 = 0x04;
/// Room ID for Wood Side Apartments Courtyard (`R_APT_E_COURTYARD = 0x07` in `Patches.h`).
pub const R_APT_E_COURTYARD_ID: u32 = 0x07;
/// Room ID for Silent Hill Town West (`R_TOWN_WEST = 0x08` in `Patches.h`).
pub const R_TOWN_WEST_ID: u32 = 0x08;
/// Room ID for Blue Creek Apartments Rooms 208/209 (`R_APT_W_RM_208_209 = 0x28` in `Patches.h`).
pub const R_APT_W_RM_208_209_ID: u32 = 0x28;
/// Room ID for Meat Locker Corridor (`ps185`, `fogSetBlowPoint` `SLUS_202.28` `0x00274cb0` `param_1 == 0x8e`).
pub const R_EDI_BOSS_CORRIDOR_ID: u32 = 0x8E;
/// Room ID for Eddie Boss Room 1 (`R_EDI_BOSS_RM_1 = 0x8F` in `Patches.h`; `ps189` in `fogSetBlowPoint` `0x00274cb0`).
pub const R_EDI_BOSS_RM_1_ID: u32 = 0x8F;
/// Room ID for Eddie Boss Room 2 (`R_EDI_BOSS_RM_2 = 0x90` in `Patches.h`; `ps193` in `fogSetBlowPoint` `0x00274cb0`).
pub const R_EDI_BOSS_RM_2_ID: u32 = 0x90;
/// Room ID for Otherworld Hotel Reading Room (`R_HTL_ALT_READING_RM = 0xB0` in `Patches.h`).
pub const R_HTL_ALT_READING_RM_ID: u32 = 0xB0;
/// Room ID for Dog Ending Room (`R_END_DOG_RM = 0xBF` in `Patches.h`).
pub const R_END_DOG_RM_ID: u32 = 0xBF;
/// Room ID for Baldwin Mansion Grand Entrance (`R_MAN_GRAND_ENTRANCE = 0xC0` in `Patches.h`).
pub const R_MAN_GRAND_ENTRANCE_ID: u32 = 0xC0;
/// Room ID for Baldwin Mansion 2F Lounge (`R_MAN_LOUNGE_2F = 0xC2` in `Patches.h`).
pub const R_MAN_LOUNGE_2F_ID: u32 = 0xC2;
/// Room ID for Baldwin Mansion Servant Room (`R_MAN_SERV_RM = 0xC4` in `Patches.h`).
pub const R_MAN_SERV_RM_ID: u32 = 0xC4;
/// Room ID for Baldwin Mansion Long Hallway (`R_MAN_LONG_HALLWAY = 0xCA` in `Patches.h`).
pub const R_MAN_LONG_HALLWAY_ID: u32 = 0xCA;
/// Room ID for Baldwin Mansion Outside Entrance (`R_MAN_OUTSIDE_ENTRANCE = 0xD3` in `Patches.h`).
pub const R_MAN_OUTSIDE_ENTRANCE_ID: u32 = 0xD3;
/// Room ID for Baldwin Mansion / Blue Creek Entrance (`R_MAN_BLUE_CREEK_ENTRANCE = 0xD4` in `Patches.h`).
pub const R_MAN_BLUE_CREEK_ENTRANCE_ID: u32 = 0xD4;

/// Exact PS2 `obGB.fcl` (`FogEnv` at file offset `0x84..0x9b`, parsed by `fogSetData` `SLUS_202.28`
/// `0x00168f40` / `Fog/fog.c:238-325`):
/// - `+0x00` (`0x84`): `particle_count = 700` (`0x02bc`)
/// - `+0x02` (`0x86`): `area_half_extent = 7000.0` mm (`0x1b58`)
/// - `+0x04` (`0x88`): `particle_size = 700.0` mm (`0x02bc`)
/// - `+0x08` (`0x8c`): `y_fixed = +10000.0` mm (`0x2710`)
/// - `+0x13` (`0x97`): `fog_alpha_param = 48` (`0x30`, `48 / 128.0 = 0.375`)
pub const OB_FOG_ENV_PARTICLE_COUNT: usize = 700;
/// Exact `obGB.fcl` `+0x02` (`0x1b58 = 7000.0` mm) horizontal area half-extent.
pub const OB_FOG_ENV_AREA_SIZE_MM: f32 = 7_000.0;
/// Exact `obGB.fcl` `+0x04` (`0x02bc = 700.0` mm) billboard radius/half-size.
pub const OB_FOG_ENV_PARTICLE_SIZE_MM: f32 = 700.0;
/// Exact `obGB.fcl` `+0x13` (`0x30 = 48` / `128.0`) PS2 GS particle alpha scale.
pub const OB_FOG_ENV_ALPHA_SCALE: f32 = 48.0 / 128.0;
/// Exact PS2 `fogInit` (`SLUS_202.28` `0x001687d0`, `Fog/fog.c:67`) `fogSetColor(0x80, 0x80, 0x80, 0x80)`:
/// on PS2 GS `0x80` (`128`) is exact unity (`1.0`) RGB modulation of `data/pic/effect/fog.tex`.
pub const PS2_FOG_PARTICLE_MIST_RGB: [f32; 3] = [1.0, 1.0, 1.0];
/// Exact PS2 fixed Y altitude (`+10,000.0` mm, PS2 Y-down = 10 m below the parking lot cliff)
/// for Observation Deck (`ob01`) 3D fog particles from retail `data/bg/ob/obGB.fcl`
/// (`FogEnv +0x08` at file offset `0x8c`, `subdiv (+0x12) = 4 != 0`), applied to
/// `fog_work.local_pos.y` (`0x00581484`) in `fogSetLocalPosV` (`SLUS_202.28` `0x0016c7c8..0x0016c7e4`).
pub const OB_FOG_ENV_FIXED_Y_MM: f32 = 10_000.0;
/// Exact PS2 daytime default GS `FOGCOL` RGB byte (`0x46` = `70` / `255.0` ≈ `0.2745`) written by
/// `SetupDrawEnvData` (`SLUS_202.28` `0x00251804..0x00251838`, `sh2pc.exe` `sh2pc_00470000.c:9147-9152`)
/// when `sh2gfw_Get_NightOrDay() == 0` and `DrawEnvData +0x9c == 0.0f` (as in `OB_DrawEnvData` `0x00362e70`,
/// `CA_DrawEnvData[1]` `0x00361d00`, `CB_DrawEnvData[0]` `0x00361ef0`, `ER_DrawEnvData[2]` `0x00363440`).
pub const PS2_DAYTIME_DEFAULT_FOG_COLOR_U8: u8 = 0x46;
/// Normalized `[0.0..=1.0]` channel value of [`PS2_DAYTIME_DEFAULT_FOG_COLOR_U8`] (`70.0 / 255.0`).
pub const PS2_DAYTIME_DEFAULT_FOG_COLOR_F32: f32 =
    (PS2_DAYTIME_DEFAULT_FOG_COLOR_U8 as f32) / 255.0;
/// Exact PS2/PC daytime default fog near distance in mm (`0x453b8000 = 3000.0f`) written by
/// `SetupDrawEnvData` (`SLUS_202.28` `0x00251370:348`, `sh2pc.exe` `sh2pc_00470000.c:9142`) when
/// `DrawEnvData.fog_far (+0xa0) == 0.0` (e.g. `CA_DrawEnvData[1]` `0x00361d00` Forest Trail `ca11`).
pub const PS2_DAYTIME_DEFAULT_FOG_NEAR_MM: f32 = 3_000.0;
/// Exact PS2/PC daytime default fog far distance in mm (`0x45abe000 = 5500.0f`) written by
/// `SetupDrawEnvData` (`SLUS_202.28` `0x00251370:347`, `sh2pc.exe` `sh2pc_00470000.c:9141`) when
/// `DrawEnvData.fog_far (+0xa0) == 0.0`.
pub const PS2_DAYTIME_DEFAULT_FOG_FAR_MM: f32 = 5_500.0;
/// Exact PS2/PC daytime default normalized minimum depth-fog blend factor (`1.0 - 235.0 / 255.0 = 20.0 / 255.0`)
/// from `SetupDrawEnvData` (`0x436b0000 = 235.0` at `SLUS_202.28` `0x00251370:350`, `sh2pc.exe` `sh2pc_00470000.c:9144`).
pub const PS2_DAYTIME_DEFAULT_FOG_MIN_NORM: f32 = (255.0 - 235.0) / 255.0;
/// Exact PS2/PC daytime default normalized maximum depth-fog blend factor (`1.0 - 20.0 / 255.0 = 235.0 / 255.0`)
/// from `SetupDrawEnvData` (`0x41a00000 = 20.0` at `SLUS_202.28` `0x00251370:349`, `sh2pc.exe` `sh2pc_00470000.c:9143`).
pub const PS2_DAYTIME_DEFAULT_FOG_MAX_NORM: f32 = (255.0 - 20.0) / 255.0;
/// Exact PS2/PC nighttime default fog near distance in mm (`0x453b8000 = 3000.0f`) written by
/// `SetupDrawEnvData` (`SLUS_202.28` `0x00251370:376`, `sh2pc.exe` `sh2pc_00470000.c:9177`).
pub const PS2_NIGHTTIME_DEFAULT_FOG_NEAR_MM: f32 = 3_000.0;
/// Exact PS2 nighttime default fog far distance in mm (`8000.0f`) written by `SetupDrawEnvData`
/// (`SLUS_202.28` `0x00251370:375`; note `sh2pc.exe` `sh2pc_00470000.c:9176` drifted to `0x45ea6000 = 7500.0f`).
pub const PS2_NIGHTTIME_DEFAULT_FOG_FAR_MM: f32 = 8_000.0;
/// Exact PS2/PC nighttime default normalized minimum depth-fog blend factor (`1.0 - 255.0 / 255.0 = 0.0`)
/// from `SetupDrawEnvData` (`0x437f0000 = 255.0` at `SLUS_202.28` `0x00251370:378`, `sh2pc.exe` `sh2pc_00470000.c:9179`).
pub const PS2_NIGHTTIME_DEFAULT_FOG_MIN_NORM: f32 = 0.0;
/// Exact PS2 nighttime default normalized maximum depth-fog blend factor (`1.0 - 100.0 / 255.0 = 155.0 / 255.0`)
/// from `SetupDrawEnvData` (`0x42c80000 = 100.0` at `SLUS_202.28` `0x00251370:377`; `sh2pc.exe` `sh2pc_00470000.c:9178` drifted to `0.0` -> `1.0`).
pub const PS2_NIGHTTIME_DEFAULT_FOG_MAX_NORM: f32 = (255.0 - 100.0) / 255.0;
/// Exact retail `data/bg/ca/caGB.fcl` default `FogEnv[0]` (`0x1100`, `fogSetAreaEnvironment` `0x0016e4a0:878`):
/// `particle_count = 400` (`0x0190`), `area = 2000 mm` (`0x07d0`), `size = 400 mm` (`0x0190`), `alpha = 16` (`0x10`).
pub const CA_FOG_ENV_PARTICLE_COUNT: usize = 400;
/// Exact retail `data/bg/ca/caGB.fcl` `FogEnv[0] +0x13` (`0x1113 = 0x10 = 16`) `fog_alpha_param`.
pub const CA_FOG_ENV_ALPHA_U8: u8 = 0x10;
/// Exact `RU_DrawEnvData[0]` (`SLUS_202.28` `0x0036bdd0`, `room_no = 5, stage_id = 13`) fog near in mm.
pub const RU_LOWER_FOG_NEAR_MM: f32 = 8_000.0;
/// Exact `RU_DrawEnvData[0]` (`SLUS_202.28` `0x0036bdd0`) fog far in mm.
pub const RU_LOWER_FOG_FAR_MM: f32 = 12_000.0;
/// Exact `RU_DrawEnvData[0]` (`SLUS_202.28` `0x0036bdd0`, `+0xac = 200.0`) normalized fog min (`55/255`).
pub const RU_LOWER_FOG_MIN_NORM: f32 = (255.0 - 200.0) / 255.0;
/// Exact `RU_DrawEnvData[0]` (`SLUS_202.28` `0x0036bdd0`, `+0xa8 = 30.0`) normalized fog max (`225/255`).
pub const RU_LOWER_FOG_MAX_NORM: f32 = (255.0 - 30.0) / 255.0;
/// Exact `RU_DrawEnvData[10]` (`SLUS_202.28` `0x0036d130`, `room_no = 73`, `ru73` Final Boss) fog near in mm.
pub const RU73_FOG_NEAR_MM: f32 = 12_000.0;
/// Exact `RU_DrawEnvData[10]` (`SLUS_202.28` `0x0036d130`, `room_no = 73`) fog far in mm.
pub const RU73_FOG_FAR_MM: f32 = 18_000.0;
/// Exact `RU_DrawEnvData[10]` (`SLUS_202.28` `0x0036d130`, `+0xac = 50.0`) normalized fog min (`205/255`).
pub const RU73_FOG_MIN_NORM: f32 = (255.0 - 50.0) / 255.0;
/// Exact `RU_DrawEnvData[10]` (`SLUS_202.28` `0x0036d130`, `+0xa8 = 30.0`) normalized fog max (`225/255`).
pub const RU73_FOG_MAX_NORM: f32 = (255.0 - 30.0) / 255.0;
/// Exact `OB_DrawEnvData` (`SLUS_202.28` `0x00362e70` / `sh2pc.exe` `0x3f5530`, `+0xac = 230.0`)
/// normalized minimum depth-fog blend factor (`1.0 - 230.0 / 255.0 = 25.0 / 255.0`), verified in
/// `sh2pc.exe` `FUN_00458c20(_DAT_00942c3c * 0.003921569)` (`sh2pc_00470000.c:7921`).
pub const OB_FOG_MIN_NORM: f32 = (255.0 - 230.0) / 255.0;
/// Exact `OB_DrawEnvData` (`SLUS_202.28` `0x00362e70` / `sh2pc.exe` `0x3f5530`, `+0xa8 = 40.0`)
/// normalized maximum depth-fog blend factor (`1.0 - 40.0 / 255.0 = 215.0 / 255.0`), verified in
/// `sh2pc.exe` `FUN_00458c30(_DAT_00942c38 * 0.003921569)` (`sh2pc_00470000.c:7922`).
pub const OB_FOG_MAX_NORM: f32 = (255.0 - 40.0) / 255.0;
/// Exact `ob01` (`DAT_00581534 == 0x05`) fixed valley fog anchor Y in PS2 mm (`0x447a0000 = 1000.0f`),
/// verified 1:1 in `SLUS_202.28` `Fog/fogdata.c:169` (`0x0016cb40`).
pub const OB_FOG_ANCHOR_Y_MM: f32 = 1000.0;
/// Exact `ob01` (`DAT_00581534 == 0x05`) fixed valley fog anchor Z in PS2 mm (`0x45dac000 = 7000.0f`),
/// verified 1:1 in `SLUS_202.28` `Fog/fogdata.c:170` (`0x0016cb40`).
pub const OB_FOG_ANCHOR_Z_MM: f32 = 7000.0;
/// Exact `ob01` vertical particle span in PS2 mm from `sh2pc.exe` `FUN_00486d10` (`0x00486d10`,
/// `sh2pc_00480000.c:4194-4205`): `(area * 2.0) / (1 << subdiv) = (7000.0 * 2.0) / 16.0 = 875.0 mm`
/// (`local_y ∈ [-875.0, 0.0]` -> world `Y ∈ [+125.0, +1000.0] mm`).
pub const OB_FOG_Y_SPAN_MM: f32 = 875.0;
/// Exact `obGB.fcl +0x13` (`0x97 = 0x30 = 48`) normalized D3D8 `SRCALPHA` sprite alpha
/// (`48.0 / 255.0 ≈ 0.1882`), verified in `sh2pc.exe` `sh2pc_00480000.c:5684,5719-5720`
/// (`(0x80 * 48) * 0.0078125 = 48 -> (48 << 24) | 0x808080`, blended via `D3DBLEND_SRCALPHA`
/// `FUN_00486fc0` `0x00486fc0` `sh2pc_00480000.c:4271-4272`).
pub const OB_FOG_ALPHA_NORM: f32 = 48.0 / 255.0;
/// Exact `obGB.fcl +0x10 = 0x12` (`wind_type = 0x12`) steady-state 3D valley fog wind velocity
/// in PS2 mm/s, derived 1:1 from `SLUS_202.28` `fogChangeWind(0x12)` (`0x00169380`,
/// `Fog/fog.c:504-525`: direction `normalize([1.0..2.0, 0.0..0.1, 0.1..0.2])` mid `[1.5, 0.05, 0.15]`,
/// accel `uVar8 = 0x3f333333 = 0.7 mm/frame²`) and `sh2pc.exe` `FUN_00486260`
/// (`sh2pc_00480000.c:3958-3961`: `vel = (vel + accel) * 0.9` at 60 Hz):
/// `v_term = (0.9 / (1.0 - 0.9)) * 0.7 * 60.0 = 378.0 mm/s`.
pub const OB_FOG_WIND_VELOCITY_MM_PER_SEC: [f32; 3] = [375.90, 12.53, 37.59];

/// Default PC/PS2 base draw distance in mm (`0x45ea6000 = 7500.0f` at `sh2pc.exe` `0x0047c199`).
pub const DEFAULT_DRAW_DISTANCE_MM: f32 = 7500.0;
/// SH2:EE extended base draw distance in mm (`Patches/DrawDistance.cpp:90`: `11000.0f`).
pub const SH2EE_EXTENDED_DRAW_DISTANCE_MM: f32 = 11_000.0;

/// Default PS2 fog particle count (`fog_defset_data + 0x00` @ `0x002ab530` in `SLUS_202.28`).
pub const PS2_DEFAULT_FOG_PARTICLE_COUNT: usize = 500;
/// Default PS2 fog particle volume half-extent in mm (`fog_defset_data + 0x02` @ `0x002ab530`,
/// `fog_asm_data1 + 0x40` @ `0x002ab440`).
pub const PS2_DEFAULT_FOG_VOLUME_HALF_EXTENT_MM: f32 = 2000.0;
/// Default PS2 fog particle height band half-extent in mm (`fog_defset_data + 0x06` @ `0x002ab530`,
/// `fog_asm_data1 + 0x10` @ `0x002ab410`).
pub const PS2_DEFAULT_FOG_HEIGHT_BAND_MM: f32 = 100.0;
/// Default PS2 radial parting influence of James on nearby fog particles in mm
/// (`JamesFogInfluence` in `FogAdjustments.cpp:331`).
pub const PS2_DEFAULT_JAMES_FOG_INFLUENCE_MM: f32 = 200.0;
/// Reduced radial parting influence on the Forest trail (`1125.0 <= james_y <= 1575.0`)
/// preventing fog sprites from sticking to James (`FogAdjustments.cpp:317-323`).
pub const PS2_FOREST_TRAIL_JAMES_FOG_INFLUENCE_MM: f32 = 10.0;
/// Lower Y-elevation bound (PS2 mm) of the Forest trail sticky-fog zone (`FogAdjustments.cpp:317`).
pub const FOREST_TRAIL_MIN_Y_MM: f32 = 1125.0;
/// Upper Y-elevation bound (PS2 mm) of the Forest trail sticky-fog zone (`FogAdjustments.cpp:317`).
pub const FOREST_TRAIL_MAX_Y_MM: f32 = 1575.0;

/// Blue Creek Apt Room 209 original front fog distance in mm (`FogAdjustments.cpp:39`).
pub const BLUE_CREEK_ORIGINAL_FOG_NEAR_MM: f32 = 3000.0;
/// Blue Creek Apt Room 209 front fog distance in mm when flashlight is active (`FogAdjustments.cpp:38`).
pub const BLUE_CREEK_FLASHLIGHT_FOG_NEAR_MM: f32 = 4200.0;
/// Final Boss Arena camera Y threshold (PS2 mm) triggering upper-scaffolding fog switch (`FogAdjustments.cpp:40`).
pub const FINAL_BOSS_CAMERA_Y_THRESHOLD_MM: f32 = -15750.0;

/// `FogSpeedFix` drift speed multiplier (`0.25`) for outdoor streets, forest, and final boss (`FogAdjustments.cpp:288`).
pub const FOG_SPEED_OUTDOOR_SLOW: f32 = 0.25;
/// `FogSpeedFix` drift speed multiplier (`0.50`) for apartment courtyard and Eddie boss rooms (`FogAdjustments.cpp:299`).
pub const FOG_SPEED_COURTYARD_MEDIUM: f32 = 0.50;

/// Evaluates the exact `ob01` (`R_OBSV_DECK`) valley-mist particle world-space Y (PS2 mm) and
/// effective alpha (`0.0..=1.0`) from a particle's symmetric `[-100, +100]` local Y and base
/// border/sway alpha.
///
/// Matches `SLUS_202.28` `Fog/fogdata.c:166-171` (`0x0016cb40`: `anchor_y = +1000.0 mm`) and
/// `sh2pc.exe` `FUN_00486d10` (`0x00486d10`, `sh2pc_00480000.c:4194-4240`): `subdiv = 4`,
/// `area = 7000.0 mm` -> `local_y ∈ [-875.0, 0.0]` (`world_y ∈ [+125.0, +1000.0] mm`) with a
/// `300.0 mm` upper fade (`local_y < 300.0 - 875.0`) and `obGB.fcl +0x13 = 48 / 128` alpha.
#[must_use]
#[inline]
pub fn evaluate_ob01_particle_y_and_alpha(
    anchor_y_mm: f32,
    local_y_symmetric_mm: f32,
    base_alpha: f32,
) -> (f32, f32) {
    let norm_y =
        ((local_y_symmetric_mm / PS2_DEFAULT_FOG_HEIGHT_BAND_MM) * 0.5 + 0.5).clamp(0.0, 1.0);
    let local_y = -norm_y * OB_FOG_Y_SPAN_MM;
    let top_fade = if local_y < 300.0 - OB_FOG_Y_SPAN_MM {
        ((OB_FOG_Y_SPAN_MM + local_y) / 300.0).clamp(0.0, 1.0)
    } else {
        1.0
    };
    (
        anchor_y_mm + local_y,
        base_alpha * top_fade * OB_FOG_ALPHA_NORM,
    )
}
