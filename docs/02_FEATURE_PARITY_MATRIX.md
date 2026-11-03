# Feature Parity Matrix: SH2:EE & Emotion Engine Relief

## 1. Silent Hill 2: Enhanced Edition (SH2:EE) Parity

| SH2:EE Feature | Original PC Limitation | Our Rust Engine Implementation | Status |
| :--- | :--- | :--- | :--- |
| **60 FPS Gameplay** | Logic tied to 30 Hz timer (`sub_004F6CE0`); 60 FPS caused bugs in fog, boat, and flashlight | Decoupled tick rate with fixed 60 Hz simulation and sub-frame interpolation | Planned (Phase 5) |
| **Volumetric 3D Fog** | Missing PS2 complexity; flat 2D sprites on PC | GPU compute-shader raymarched volumetric fog using PS2 prototype math | Planned (Phase 4) |
| **Widescreen / Ultrawide** | Hor+ stretched 4:3 viewport | Native responsive aspect ratio calculation; dynamic FOV scaling | Planned (Phase 4) |
| **High-Res Font Textures** | Low-res 256x256 font textures | Vector font rendering or high-res texture atlas streaming from `sh2e/font` | Planned (Phase 3) |
| **1080p60 FMV Pack** | Low-res 240p/480p Bink videos | High-definition MPEG/H.264/AV1 video playback with soft subtitles | Planned (Phase 4) |
| **Restored Audio & SFX** | Compressed, missing ambient loops on PC | High-fidelity 48kHz audio streaming (`kira`), accurate loop points | Planned (Phase 3) |
| **Right-Stick Search Camera** | Rigid fixed camera without free look | Full 360-degree analog right-stick search camera with collision clamping | Planned (Phase 5) |
| **Dual Controller Layouts** | DirectInput quirks, missing vibration | Modern gamepad mapping with analog trigger sensitivity & haptic feedback | Planned (Phase 7) |

---

## 2. Relief from Emotion Engine Hardware Sacrifices

| Emotion Engine Limitation | Historical Sacrifice | Our Modern Unrestricted Engine |
| :--- | :--- | :--- |
| **4MB GS eDRAM** | Severe overdraw penalties; 16-bit color banding; low-res depth buffer | 32-bit floating point depth buffer and HDR framebuffers; zero Z-fighting |
| **Aggressive Fog Pop-In** | Fog distance choked to avoid rendering too many background polygons | Extended geometry streaming; realistic atmospheric depth without pop-in |
| **VU0/VU1 Pipeline Bottlenecks** | Strict polygon budget per room; low-poly character LODs | Modern skeletal compute skinning; uncompressed polygon streaming |
| **2MB Sound RAM (SPU2)** | Heavy downsampling and short looping samples | Zero memory budget constraint on audio buffers; dynamic acoustic reverb |
| **Single 30 Hz Main Loop** | Frame drops in heavy rooms or during disk streaming | True multi-threading: asset decompression and audio stream on background threads |

---

## 3. Scenario & Dual-Screen Support

| Component | Main Game (*Letter from Silent Heaven*) | Sub-Scenario (*Born from a Wish*) |
| :--- | :--- | :--- |
| **Playable Character** | James Sunderland | Maria |
| **Primary Location** | Town of Silent Hill, Apartments, Hospital, Prison, Hotel | Heaven's Night, Baldwin Mansion (Stages 53–58) |
| **Top Display View** | Immersive 3D world, full atmospheric effects | Immersive 3D world, full atmospheric effects |
| **Bottom Display Companion** | Town & indoor building maps (Maps 1–35), item quick-slots | Baldwin Mansion map (Map 36), Maria's inventory, memos |
| **Restored Debug Tools** | Room warps, free-cam noclip, collision hitboxes | Room warps to stages 53–58, Baldwin lighting debug |
