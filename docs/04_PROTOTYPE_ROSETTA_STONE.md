# Prototype Ground Truth & Rosetta Stone Map

## Analyzed Executables

### 1. July 13, 2001 Prototype (`SLUS_202.28`)
* **Platform:** Sony PlayStation 2 (Emotion Engine MIPS-III, 32-bit little-endian)
* **Status:** Statically linked, **not stripped**, contains complete `.symtab`, `.strtab`, `.debug`, and `.line` sections.
* **Symbol Count:** **26,622 symbols** intact.
* **Key Strengths:**
  - Preserves original C source file paths and line numbers: `m3_play.c`, `clani.c`, `anime.c`, `sh_character_battle.c`, `m3_maria_sub.c`.
  - Full struct definitions and global state names (`item_screen_obj_data`, `alldata_Vu0_Dblbuffer`, `dmaria_anim`).
  - Matches final game data layout and asset formats closely.

### 2. E3 2001 Trial Version (`SLPM_123.45`)
* **Platform:** Sony PlayStation 2 (Emotion Engine MIPS-III, 32-bit little-endian)
* **Status:** Statically linked, **not stripped**, contains active debug subsystem.
* **Key Strengths:**
  - Active wireframe & collision rendering pipeline:
    - `clDbgDrawBGColiPoly`: Background world geometry collision polygons.
    - `clDbgDrawCharaColi`: Character and enemy bounding capsules.
    - `clDbgDrawPolyBOX`, `clDbgDrawPolySPHERE`, `clDbgDrawPolyCOLUMN`: 3D collision primitives.
    - `clDbgDrawHitPolygon`: Weapon attack cones and enemy damage hitboxes.
  - Live environmental & lighting debuggers:
    - `dbg_fogparm_ctl`: Fog density, color, and depth curve parameters.
    - `dbg_ambifog_ctl`: Ambient fog coupling.
    - `dbg_spotL_ctl`: Flashlight cone angle, falloff, and color.
    - `dbg_gamma_ctl` & `dbg_noise_set`: Film grain and gamma parameters.
  - Interactive debug system (`vrInteractDbgSystem`):
    - Controller 2 debug hook functions (`printR_pad10`, `HH_DBG_Wrapper_Controller_KeyAssign_Check`).

---

## Direct Cross-Reference with SH2:EE Patches

| SH2:EE Patch (`Patches/`) | Prototype C Source File | Prototype Function / Global Symbol |
| :--- | :--- | :--- |
| `SixtyFPSPatch.cpp` | `m3_play.c` | `sub_004F6CE0` (30 Hz limiter), `sub_00518AD0` (60 Hz limiter) |
| `Fog.cpp` | `sh_fog.c` / `m3_sc.c` | `dbg_fogparm_ctl`, `dbg_ambifog_ctl` |
| `Lighting.cpp` / `Flashlight.cpp` | `sh_light.c` | `dbg_spotL_ctl`, `dbg_parallel_set` |
| `NoiseFilter.cpp` | `sh_effect.c` | `dbg_noise_set` |
| `Camera.cpp` | `m3_play_3d.c` | `CamFollowJames`, `CamMatrixUpdate`, `getRmat` |
| `LightingTransition.cpp` | `prog_wish.c` / `m3_sc.c` | `EvProgGetBaldwinMap`, `RoomNameMansion` |
| `Font.cpp` | `ee.c` | `d3d_flush_batch`, glyph cache `0x943928` |

---

## Action Plan for Extraction
1. Run automated extractor script on `SLUS_202.28` and `SLPM_123.45` to generate structured JSON symbol catalogs.
2. Parse `.debug` section to extract original C struct definitions for player state, camera matrices, and collision hulls into `crates/sh2-core/src/types/`.
3. Use unstripped symbol names directly in the Rust engine codebase rather than reverse-engineered `sub_004xxxxx` designations.
