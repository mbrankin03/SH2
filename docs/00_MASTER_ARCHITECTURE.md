# Silent Hill 2 Open Port: Master Architectural Blueprint

## Project Overview
An open-source, memory-safe, modern game engine port of **Silent Hill 2 (Director's Cut / Restless Dreams)** written in **Rust**, targeting **Linux** and **Android (specifically the dual-screen AYN Thor handheld)**.

The port is legally clean (zero proprietary game assets included) and dynamically ingests original game data files (PC Director's Cut / PS2 ISOs) alongside the asset packs of **[Silent Hill 2: Enhanced Edition (SH2:EE)](https://enhanced.townofsilenthill.com/SH2/)**.

---

## The Four Core Pillars

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              SH2 RUST ENGINE                                │
│                     (Memory-Safe, Multi-Threaded Core)                      │
├───────────────────────┬─────────────────────────────┬───────────────────────┤
│   1. Modern Engine    │    2. Rosetta Stone         │    3. Dual-Screen     │
│       & Hardware      │       Prototypes            │       Experience      │
│         Relief        │                             │       (AYN Thor)      │
├───────────────────────┼─────────────────────────────┼───────────────────────┤
│ • wgpu / Vulkan       │ • July 13, 2001 Prototype   │ • Top Display:        │
│ • Volumetric Fog      │   (SLUS_202.28 - 26k syms)  │   Clean 3D cinematic  │
│ • True 60+ FPS ticks  │ • E3 2001 Trial             │ • Bottom Display:     │
│ • 32-bit float depth  │   (SLPM_123.45 - debug draws│   Real-time town map, │
│ • Born from a Wish    │ • Native debug consoles     │   memo pad, heal slots│
│   (Stages 53-58)      │   & wireframe visualizers   │   & touch debug tools │
└───────────────────────┴─────────────────────────────┴───────────────────────┘
```

### 1. Modern Architecture & Emotion Engine Hardware Relief (Hallway Engine Philosophy)
The original PS2 Emotion Engine had massive hardware bottlenecks:
* **eDRAM Bandwidth:** 4MB limit caused color banding, low-precision depth buffers, and severe overdraw penalty.
* **Aggressive Fog Culling:** Fog was heavily used to disguise low draw distances and fill-rate limits.
* **Locked 30 Hz Step Rate:** Physics, animation, and logic were locked to 30 FPS.

**Our Engine Solution:**
* Full 32-bit floating point depth buffers and HDR framebuffers (zero distant Z-fighting).
* GPU compute-shader volumetric fog based on the original PS2 atmospheric parameters (`dbg_fogparm_ctl`, `dbg_ambifog_ctl`), running at native 60+ FPS without fill-rate penalties.
* Decoupled tick rate using fixed-step simulation with sub-frame interpolation.
* Ingestion of SH2:EE packs: 1080p60 FMVs, high-resolution fonts, restored audio scripts, and widescreen geometry.

### 2. The Rosetta Stone: Unstripped Prototypes
Using two unstripped PS2 MIPS prototypes as ground truth:
* **July 13, 2001 (`SLUS_202.28`):** 26,622 debug symbols, exact C file structure (`m3_play.c`, `clani.c`, `anime.c`), unstripped struct layouts.
* **E3 2001 Trial (`SLPM_123.45`):** Complete active wireframe and collision renderers (`clDbgDrawBGColiPoly`, `clDbgDrawHitPolygon`), spotlight controllers (`dbg_spotL_ctl`), and interactive Controller 2 debug loop (`vrInteractDbgSystem`).

### 3. Dual-Screen Companion UI (AYN Thor & sh23ds Architecture)
Using the proven design from `sh23ds/port/source/`:
* **Top Display:** Pure 3D world with zero HUD clutter.
* **Bottom Touch Display:**
  - Dynamic interactive town and building maps (Map 1–35 for James, Map 36 for Baldwin Mansion).
  - One-tap quick healing slots (`touchui_quick`).
  - Active puzzle memo viewer (`touchui_panel`).
  - Controller 2 developer console (free-cam, stage warp, hitbox toggles).

### 4. Complete Content Support: Main Scenario & "Born from a Wish"
* Full support for James Sunderland's quest (*Letter from Silent Heaven*).
* Full support for Maria's sub-scenario (*Born from a Wish*, Stages 53–58 in the Baldwin Mansion).
* Stage programs ported from `sh23ds/port/source/prog_wish.c` with map index 36.

---

## Crate Layout (`dev/SH2/crates`)

| Crate | Responsibility |
| :--- | :--- |
| `sh2-core` | Game state machines, entity component system, math vectors, camera logic, puzzle flags. |
| `sh2-assets` | Parsers for PC/PS2 data formats (`.map`, `.mdls`, `.anm`, `.sdm`, `.mes`, DDS/PNG textures). |
| `sh2-renderer` | Modern `wgpu`/Vulkan rendering pipeline, volumetric fog shaders, post-processing film grain. |
| `sh2-audio` | Streaming sound engine (`kira`/`rodio`), ambient sound loops, 3D spatial positioning. |
| `sh2-ui` | Dual-screen touch companion UI, dynamic map renderer, HUD overlays, text glyph caching. |
| `sh2-debug` | Restored developer tools: free-cam noclip, room warper, hitbox visualizer, memory monitor. |
| `sh2-app` | Cross-platform runtime: windowing (`winit`/SDL2), Android presentation layer, input mapping. |
