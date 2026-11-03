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
- [ ] Install Rust toolchain on CachyOS (`rustup`, `cargo`, `cargo-ndk`).
- [ ] Initialize Cargo workspace in `dev/SH2/Cargo.toml`.
- [ ] Configure `ccache`, `clang`, and Android NDK toolchain targets (`aarch64-linux-android`).
- [ ] Verify ADB connection and authorization to AYN Thor (`adb devices`).

## Phase 2: Prototype Ground Truth Extraction
- [ ] Build extraction script for `SLUS_202.28` (July 13, 2001 Prototype) symbol table.
- [ ] Build extraction script for `SLPM_123.45` (E3 2001 Trial) symbols & debug routines.
- [ ] Generate unified symbol dictionary in JSON (`docs/symbols_july13.json`, `docs/symbols_e3.json`).
- [ ] Extract original C struct headers from DWARF debug sections (`m3_play.h`, `clani.h`, `anime.h`).
- [ ] Map SH2:EE patch addresses (`Patches/`) to real unstripped function names.

## Phase 3: Rust Workspace & Asset Ingestion (`sh2-assets`)
- [ ] Implement VROM / archive reader for PC Restless Dreams and PS2 ISO formats.
- [ ] Build parser for `.map` level geometry and collision meshes.
- [ ] Build parser for `.mdls` 3D character models and skeletal bone hierarchies.
- [ ] Build parser for `.anm` skeletal animation tracks.
- [ ] Build parser for `.sdm` / `.sdb` audio banks and `.mes` dialogue script tables.
- [ ] Implement high-resolution texture loader supporting SH2:EE DDS/PNG replacement packs.

## Phase 4: Modern `wgpu` Renderer & Hardware Relief (`sh2-renderer`)
- [ ] Setup `wgpu` Vulkan/OpenGL ES backend with 32-bit floating-point depth buffer.
- [ ] Implement compute-shader 3D volumetric fog based on `dbg_fogparm_ctl` / `dbg_ambifog_ctl`.
- [ ] Implement dynamic spotlight & flashlight cone calculations (`dbg_spotL_ctl`).
- [ ] Implement film grain post-processing shader with configurable intensity.
- [ ] Build skeletal mesh skinning shader for James, Maria, and creatures.
- [ ] Implement 1080p60 FMV video player with audio sync (FFmpeg / software decoder).

## Phase 5: Gameplay Core & Scenarios (`sh2-core`)
- [ ] Decouple simulation clock from rendering (fixed 60 Hz physics/animation tick rate).
- [ ] Implement camera system (third-person exploration, fixed cinematic cameras, right-stick search).
- [ ] Port Main Scenario (*Letter from Silent Heaven*): James's inventory, state machines, and puzzles.
- [ ] Port Sub-Scenario (*Born from a Wish*): Maria's Baldwin Mansion stages 53–58 from `prog_wish.c`.
- [ ] Wire in SH2:EE timing constants for 60 FPS pacing (flashlight speed, fog animation, boat rowing).

## Phase 6: Dual-Screen & Touch Companion UI (`sh2-ui`)
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
