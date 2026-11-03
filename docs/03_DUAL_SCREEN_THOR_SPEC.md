# Dual-Screen & Touch UX Specification (AYN Thor)

## Hardware Target: AYN Thor
* **Form Factor:** Dual-display clamshell Android gaming handheld.
* **Top Display:** Primary screen (16:9 / high-resolution OLED), controller-focused.
* **Bottom Display:** Secondary display (touchscreen), companion & input-focused.
* **Input Interfaces:** Standard game controller (D-Pad, dual analog sticks, triggers, bumpers, face buttons) + multi-touch capacitive screen.

---

## Screen Allocation

```
┌────────────────────────────────────────────────────────┐
│                      TOP DISPLAY                       │
│                                                        │
│             3D Gameplay / Cutscenes / FMVs             │
│        (No HUD, No Status Bars, Full Immersion)        │
│                                                        │
└────────────────────────────────────────────────────────┘
┌────────────────────────────────────────────────────────┐
│                     BOTTOM DISPLAY                     │
│                                                        │
│  ┌──────────────────────┬───────────────────────────┐  │
│  │                      │   QUICK SLOTS & INVENTORY │  │
│  │                      │   [Drink] [Medkit] [Amp]  │  │
│  │   REAL-TIME MAP      ├───────────────────────────┤  │
│  │   - Player Arrow     │   ACTIVE MEMO / PUZZLE    │  │
│  │   - Red Pen Markings │   "The key is behind..."  │  │
│  │   - Interactive Pan  ├───────────────────────────┤  │
│  │                      │   DEV CONSOLE / DEBUG     │  │
│  │                      │   [Cam] [Warp] [Hitbox]   │  │
│  └──────────────────────┴───────────────────────────┘  │
└────────────────────────────────────────────────────────┘
```

---

## Bottom Screen Subsystems (Adapted from `sh23ds`)

### 1. Real-Time Dynamic Map (`map.c`)
- **Map Indices:**
  - `1..3`: Silent Hill Town (East/West)
  - `4..11`: Wood Side & Blue Creek Apartments
  - `12..15`: Brookhaven Hospital (Normal / Otherworld)
  - `16..19`: Silent Hill Historical Society & Toluca Prison
  - `20..35`: Lakeview Hotel (Guest / Employee sections)
  - `36`: **Baldwin Mansion (*Born from a Wish*)**
- **Dynamic Features:**
  - Real-time James / Maria orientation arrow and position coordinate tracking.
  - Interactive touch-to-pan and pinch-to-zoom without pausing gameplay.
  - Red pen marking animations as doors are tried, locks are picked, or notes are found.

### 2. One-Tap Quick Slots (`touchui_quick.c`)
- Tap slot 1: Use Health Drink.
- Tap slot 2: Use First Aid Kit.
- Tap slot 3: Use Ampoule.
- Quick weapon switch ribbon (Wooden Plank, Steel Pipe, Handgun, Shotgun, Hunting Rifle).

### 3. Memo & Riddle Notebook (`touchui_panel.c`)
- Whenever a puzzle note, diary page, or letter is read in the world, it pins automatically to the memo panel on the bottom screen.
- Allows solving complex riddles (clock puzzle, coin puzzle, hospital box) without opening and closing the inventory repeatedly.

### 4. Integrated Developer Console (`touchui_debug.c`)
- Toggled via on-screen corner button or clicking both thumbsticks (`L3 + R3`):
  - **Free-Camera:** Detach camera to explore outside level boundaries.
  - **Stage Warp:** Instant teleporter to any room in Main or Maria campaigns.
  - **Hitbox Visualizer:** Toggle wireframe rendering for attack ranges and enemy colliders.
  - **Performance HUD:** Real-time frame times, VRAM usage, active draw calls, and compute-shader dispatch timings.
