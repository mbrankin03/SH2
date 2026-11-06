# Task Breakdown & Implementation Roadmap

## Milestone Overview

```
Phase 1: Environment & Tooling Setup
       ↓
Phase 2: Prototype Ground Truth Extraction
       ↓
Phase 3: Rust Workspace & Asset Ingestion (`sh2-assets`)
       ↓
Phase 4: Modern `wgpu` Renderer & Volumetric Atmosphere (`sh2-renderer`)
       ↓
Phase 5: Gameplay Logic & Dual-Scenario Execution (`sh2-core`)
       ↓
Phase 6: Dual-Screen & Touch Companion UI (`sh2-ui`)
       ↓
Phase 7: Packaging, Android NDK & AYN Thor Deployment (`sh2-app`)
```

---

## Phase 1: Environment & Tooling Setup
- [x] Install Rust toolchain on CachyOS (`rustup`, `cargo`, `cargo-ndk`).
- [x] Initialize Cargo workspace in `dev/SH2/Cargo.toml`.
- [x] Configure `ccache`, `clang`, and Android NDK toolchain targets (`aarch64-linux-android`).
- [x] Verify ADB connection and authorization to AYN Thor (`adb devices`).

## Phase 2: Prototype Ground Truth Extraction
- [x] Build extraction script for `SLUS_202.28` (July 13, 2001 Prototype) symbol table.
- [x] Build extraction script for `SLPM_123.45` (E3 2001 Trial) symbols & debug routines.
- [x] Generate unified symbol dictionary in JSON (`docs/symbols_july13.json`, `docs/symbols_e3.json`).
- [x] Extract original C struct headers from DWARF debug sections (`m3_play.h`, `clani.h`, `anime.h`).
- [x] Map SH2:EE patch addresses (`Patches/`) to real unstripped function names.

## Phase 3: Rust Workspace & Asset Ingestion (`sh2-assets`)
- [x] Implement VROM / archive reader for PC Restless Dreams and PS2 ISO formats.
- [x] Build parser for `.map` level geometry and collision meshes.
- [x] Build parser for `.mdls` 3D character models and skeletal bone hierarchies.
- [x] Build parser for `.anm` skeletal animation tracks.
- [x] Build parser for `.sdm` / `.sdb` audio banks and `.mes` dialogue script tables.
- [x] Implement high-resolution texture loader supporting SH2:EE DDS/PNG replacement packs.

## Phase 4: Modern `wgpu` Renderer & Hardware Relief (`sh2-renderer`)
- [x] Setup `wgpu` Vulkan/OpenGL ES backend with 32-bit floating-point depth buffer.
- [x] Implement compute-shader 3D volumetric fog based on `dbg_fogparm_ctl` / `dbg_ambifog_ctl`.
- [x] Implement dynamic spotlight & flashlight cone calculations (`dbg_spotL_ctl`).
- [x] Implement film grain post-processing shader with configurable intensity.
- [x] Build skeletal mesh skinning shader for James, Maria, and creatures.
- [x] Implement 1080p60 FMV video player with audio sync (FFmpeg / software decoder).
- [x] Cutscene & FMV music: per-cutscene BGM/audio-track mapping and playback (SH2-39):
  - Per-cutscene audio mapping table connecting all 21 FMVs and in-engine demos to retail BGM files, Bink audio tracks, start/stop actions, and master volumes (`0x8a73b4` / `SLUS_202.28`).
  - CRI AIX multi-channel / multi-stream container parser, demuxer, and stereo downmixer (`crates/sh2-assets/src/formats/aix.rs`).
  - Dedicated FMV audio sub-track and life cycle controls in `AudioEngine` allowing seamless independent streaming of paired ADX/AIX background music.
  - Interactive FMV Player HUD display for cutscene music tracks and live playback integration.
- [ ] **[Bonus / Extended Graphics]** Native HDR Display Pipeline:
  - Configure swapchain presentation for wide-gamut / high-dynamic-range surfaces (`Rgba16Float` / `Rgb10a2Unorm` with PQ / ScRGB Linear).
  - Implement HDR-calibrated color grading and filmic tonemapping (preserving inky pitch blacks while raising high-nit peaks for flashlights, streetlamps, and emergency strobes).
- [ ] **[Bonus / Extended Graphics]** Omnipresent Shadow Casting & Clustered Lighting:
  - Upgrade lighting system to Clustered Forward / Forward+ shading to support every scene spotlight/point light casting real-time shadows.
  - Implement a dynamic shadow map atlas allocator with static geometry depth caching.
  - Integrate spotlight shadow tests directly into volumetric fog raymarching for dynamic light shafts and god rays.

## Phase 5: Gameplay Core & Scenarios (`sh2-core`)
- [x] Decouple simulation clock from rendering (fixed 60 Hz physics/animation tick rate).
- [ ] Implement camera system (third-person exploration, fixed cinematic cameras, right-stick search).
- [ ] Port Main Scenario (*Letter from Silent Heaven*): James's inventory, state machines, and puzzles.
- [ ] Port Sub-Scenario (*Born from a Wish*): Maria's Baldwin Mansion stages 53–58 from `prog_wish.c`.
- [ ] **SH2-47: Scripted Sequence System & Environmental Horror Triggers**
  - Implement spatial event trigger bounds (aabb / action walls `clActWallList_*`).
  - Create Stage Event State Machine (`EvRoomInit`, `EvAllTimeFunc`, `ev_s_step`, `ev_p_step`).
  - Toluca Prison bathroom stall jumpscare and stall knocking sequence.
  - Blue Creek Apartments (Room 209 "See?") and Prison courtyard invisible execution whispers.
  - Link directional 3D audio cues to stage event state progression.
- [x] Wire in SH2:EE timing constants for 60 FPS pacing (flashlight speed, fog animation, boat rowing).

## Phase 6: Dual-Screen & Touch Companion UI (`sh2-ui`)
- [x] In-Game Pause Menu (`SH2-45`):
  - Authentic 1:1 Pause Menu state machine (`CONTINUE`, `SAVE`, `LOAD`, `OPTIONS`, `QUIT`) with PS2 parity (event `0x10`).
  - Simulation freeze: halts game tick, physics, character locomotion, and camera while keeping background sound effects and options responsive.
  - Quit confirmation dialog ("Exit to Main Menu? [YES / NO]") with default focus on NO.
  - Full seamless roundtrip integration with `OptionsMenu` for live audio/video/controls adjustment from inside active gameplay.
  - Authentic 50% alpha dark scrim backdrop and proportional PS2 bitmap atlas typography.
- [ ] Port `sh23ds/port/source/map.c`: Real-time town map (Maps 1–35) and Baldwin Mansion (Map 36).
- [ ] Implement real-time marker pen drawing system for discovered keys, memos, and locked doors.
- [ ] Port `touchui_quick.c`: One-tap bottom-screen healing item quick-slots and weapon hot-swap.
- [ ] Port `touchui_panel.c`: Memo notebook display for examined letters, codes, and documents.
- [ ] Port developer console overlay (`touchui_debug.c` / Controller 2 debug system):
  - Free-cam noclip mode.
  - Stage/room teleporter.
  - Collision & attack hitbox visualizer (`clDbgDrawHitPolygon`).
  - Real-time perf & memory meters.

## Phase 7: Packaging & AYN Thor Deployment (`sh2-app`)
- [ ] Android NativeActivity / dual-display Presentation API integration.
- [ ] Native controller input mapping (standard XInput / Android GameController + touchscreen).
- [ ] Multi-swapchain rendering: Primary OLED (3D gameplay) + Secondary screen (Companion UI).
- [ ] Automated Gradle / `cargo-apk` build pipeline for instant ADB install to Thor.
