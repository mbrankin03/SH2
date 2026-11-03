# SH2:EE Patches to Unstripped Prototype Symbol Cross-Reference

This document maps Silent Hill 2: Enhanced Edition (`Silent-Hill-2-Enhancements/Patches/`) fixes and hooks to original unstripped functions, structures, and global variables discovered in the PS2 prototype executables (`SLUS_202.28` July 13, 2001 and `SLPM_123.45` E3 2001 Trial).

## Core Architecture Mapping

| SH2:EE Patch (`Patches/`) | Prototype C Source File | Prototype Function / Global Symbol | Description & Engine Impact |
| :--- | :--- | :--- | :--- |
| `SixtyFPSPatch.cpp` | `m3_play.c` / `sh_fog.c` / `m3_boat.c` | `shExec`, `shPlayerWork`, `dbg_fogparm_ctl` | 60 FPS pacing fixes: decouples fog animation rate, bullet shell physics, and water animation speeds. |
| `FogFix.cpp`, `FogAdjustments.cpp` | `Fog/fogdata.c`, `view/vw_main.c` | `dbg_fogparm_ctl`, `dbg_ambifog_ctl`, `sh2gfw_fogtest_main` | Restores original volumetric fog density, ambient fog coupling, and depth curve falloff. |
| `LightingPatch.cpp`, `EnhancedFlashlight.cpp` | `GFW/sh2gfw_LightSet.c`, `Chacter_Draw/sh2_JmsSpot_Man.c` | `dbg_spotL_ctl`, `sh2gfw_POINTL_MATRIX`, `sh2gfw_SPOTL_MATRIX` | Spotlight cone calculations, dynamic light falloff, and flashlight beam direction. |
| `PS2NoiseFilter.cpp` | `Effect/ef_common.c`, `Effect2/hh_class_wrapper.c` | `dbg_noise_set`, `dbg_gamma_ctl` | Restores the hardware procedural film grain noise pattern from the EE rather than generic overlays. |
| `Camera.cpp` / WidescreenFix | `view/vc_main.c`, `view/vc_calc.c`, `view/vw_calc.c` | `vcExecCamera`, `vcInitCamera`, `_VC_WORK`, `_VbWVSMATRIX` | Fixes camera yaw/pitch speed, aspect ratio calculation, and far-view watch tracking. |
| `LightingTransition.cpp` | `Event/stage/stg_mansion.c`, `prog_wish.c` | `RoomNameMansion`, `EvProgGetBaldwinMap`, `m3_sc.c` | Lighting transitions across rooms in the Baldwin Mansion sub-scenario (*Born from a Wish*). |
| `Fonts.cpp` | `DBG/shDBG_fontHandle.c`, `ee.c` | `d3d_flush_batch`, glyph cache `0x943928` | High-resolution font rendering, dynamic glyph scaling, and UI layout adjustment. |
| `RowboatAnimation.cpp`, `RowboatFix.cpp` | `Chacter/m3_boat.c`, `Chacter/m3_boat_jms.c` | `shPlayerWork`, `m3_boat_jms` | Fixes rowboat turn speed and rowing cycle synchronization at high frame rates. |
| `WaterEnhancement.cpp`, `HotelWater.cpp` | `Effect/ef_rain.c`, `Chacter/lake_wave.c` | `water_road`, `strike_splash_flg` | Water surface animation speed, vertex displacement, and splash triggers. |
| `DoubleFootstepFix.cpp` | `sound/sh_sound.c` | `se_foot`, `shPlayerWork.se_foot` | Prevents duplicated footstep audio trigger frames during animation loop transitions. |
| `ChainsawSoundFix.cpp`, `PreventChainsawSpawn.cpp` | `Chacter/m3_wep.c`, `Event/item.c` | `csaw_se_vol`, `csaw_se_flg`, `shPlayerWork.weapon` | Fixes chainsaw sound looping volume and restricts chainsaw to New Game+ spawns. |
| `AtticShadowFixes.cpp`, `Room312Shadow.cpp` | `sh2shd/sh2shd_shadow_env.c` | `sh2shd_shadow_model`, `Shadow_Mini_Camera` | Directional stencil shadow projection and room-specific clipping planes. |
| `FireEscapeKey.cpp`, `ChangeClosetSpawn.cpp` | `Event/item.c`, `Event/event.c` | `shPlayerWork.pad`, `item_screen_obj_data` | Key item acquisition and apartment closet cutscene object state. |
| `CatacombsMeatRoom.cpp` | `Event/stage/stg_prison.c` | `shPlayerWork.col_atk_z_hosei` | Meat locker collision bounds and hanging carcass collision hulls. |
| `HospitalChase.cpp`, `MariaSpawnFix.cpp` | `Chacter/m3_maria_sub.c`, `Chacter/m3_play.c` | `SubCharacter`, `dmaria_anim` | Maria AI movement pacing, spawn coordinates, and elevator chase cutscene timing. |

## Interactive Debug Routines (E3 2001 Trial `SLPM_123.45`)

The E3 prototype contains unstripped debug hooks that directly empower developer visualization tools in `crates/sh2-debug`:

1. **Geometry & Hitbox Visualization**:
   - `clDbgDrawBGColiPoly`: Visualizes background world geometry collision polygons.
   - `clDbgDrawCharaColi`: Visualizes character and enemy bounding capsules.
   - `clDbgDrawPolyBOX`, `clDbgDrawPolySPHERE`, `clDbgDrawPolyCOLUMN`: Visualizes 3D collision primitives.
   - `clDbgDrawHitPolygon`: Visualizes attack cones, weapon sweeps, and damage hitboxes.

2. **Environmental Tuning**:
   - `dbg_fogparm_ctl`: Live parameter tuning for fog start, end, max density, and color.
   - `dbg_ambifog_ctl`: Ambient fog light coupling factor.
   - `dbg_spotL_ctl`: Flashlight beam angle, cutoff, attenuation, and color.
   - `dbg_gamma_ctl`: Real-time gamma adjustment.

3. **Controller 2 Debug Subsystem**:
   - `vrInteractDbgSystem`: System-wide developer debug menu and controller listener.
   - `printR_pad10`: Debug text overlay routine for input states and frame metrics.
