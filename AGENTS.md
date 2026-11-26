# Silent Hill 2 Open Port — Agent Engineering & Architecture Rules

These rules apply unconditionally to all agents, subagents, and `/swarm` workers (Coordinator, Researchers, Authors, and Tech Lead Verifiers) working on the Silent Hill 2 Rust port.

## 1. Strict Crate Layering & Boundaries (Zero Spaghetti)

Respect the unidirectional dependency hierarchy across `/home/matthewb/dev/SH2/crates`:

- **`sh2-assets`** (`#![forbid(unsafe_code)]`): Pure binary format parsers and typed asset representations (`.map`, `.mdl`, `.anm`, `.cls`, `.cam`, `.cld`, `.dds`, `.mes`, `.sdm`, `.aix`, textures). Zero dependencies on rendering, audio, windowing, or runtime game state.
- **`sh2-core`**: Deterministic 60 Hz simulation, math, camera algorithms, physics/collision, entity FSMs, inventory, puzzles, and stage event scripts. **Never** leak `wgpu`, `winit`, `kira`, `gilrs`, or UI rendering types into `sh2-core`.
- **`sh2-renderer` / `sh2-audio` / `sh2-ui` / `sh2-debug`**: Presentation and tooling subsystems that consume parsed `sh2-assets` data and clean state snapshots/commands from `sh2-core`.
- **`sh2-app`**: Thin cross-platform runtime shell (`winit`, Android NDK, input routing, frame orchestration). **Never dump domain logic, binary parsing, camera math, or UI layout algorithms into `sh2-app`.** Keep `sh2-app` limited to wiring subsystems together.

## 2. Anti-Monolith & Modularity Standards

- **No God-Files:** Keep files cohesive and single-responsibility. When a file grows past ~400–500 lines or gains a distinct subsystem, decompose it into focused submodules (`foo/mod.rs`, `foo/bar.rs`) instead of appending hundreds of lines to an existing file.
- **Small, Pure, Testable Functions:** Extract pure math, state transitions, and binary decoding into small functions that can be unit-tested in isolation without initializing GPU, audio, or windowing backends.
- **Typed State Machines Over Boolean Soup:** Model lifecycle, menu, camera, combat, and horror-event states with explicit Rust `enum`s and exhaustive `match` transitions—never interlocking booleans or undocumented magic integers.
- **Defensive Error Handling:** Use typed `thiserror` `Result`s across asset/IO boundaries. Never `unwrap()` or `expect()` on external files, offsets, or user config; validate buffer bounds explicitly.
- **Naming & Documentation:**
  - `PascalCase` types/traits/enums, `snake_case` functions/fields, `SCREAMING_SNAKE_CASE` constants.
  - Every `pub` item requires a `///` doc comment stating its purpose, coordinate/time units (e.g., PS2 world mm, Q12 fixed-point radians, 30 Hz vs 60 Hz ticks), and original Team Silent symbol name/address when ported.

## 3. Reverse-Engineering Ground-Truth Hierarchy & Zero-Guessing Mandate (CRITICAL — 1:1 PARITY)

**ABSOLUTE RULE — NEVER GUESS:** Never guess, invent, or infer constants, struct offsets, bitflags, `.cld`/`.map`/`.mdl` material IDs, attenuation curves, timing intervals, coordinate scales, or state transitions from comments or intuition. **1:1 parity is mandatory.** Before writing implementation code, read the decompiled C under `docs/decomp/` (and verify against raw disassembly / data tables when needed), and cite the symbol name + hex address in `///` doc comments:

0. **Start Here — Pre-Exported Ghidra C Decompilations (`/home/matthewb/Documents/SH2-Port/docs/decomp/`):**
   - **Always read `docs/decomp/` FIRST** before falling back to raw `objdump`/disassembly or launching Ghidra:
     - `docs/decomp/slus/<original_dwarf_dir>/<file>.c` + `INDEX.tsv` — Full Ghidra C decompilation of `SLUS_202.28` (Jul 13, 2001 PS2 prototype), grouped by Team Silent's original DWARF-1 `.c` filenames (e.g., `Item/otn_itemmain.c`, `Event/chizu.c`, `Event/memo.c`, `view/vc_calc.c`).
     - `docs/decomp/slpm/<original_dwarf_dir>/<file>.c` + `INDEX.tsv` — Full Ghidra C decompilation of `SLPM_123.45` (E3 2001 Trial Version), grouped identically.
     - `docs/decomp/sh2pc/sh2pc_<addr>.c` + `INDEX.tsv` + `STRINGS.tsv` — Full Ghidra C decompilation of retail PC `sh2pc.exe` (64 KB address shards + symbol/string cross-reference tables).
   - Reusable Ghidra projects live in `/home/matthewb/Documents/SH2-Port/ghidra_proj/{slus,slpm,sh2pc}` and can be re-exported or queried headlessly via `scripts/ghidra_export_decomp.sh --reexport`.
   - Fall back to raw disassembly (`objdump` / Python ELF/PE inspection) only to verify exact float/data-table bytes, MIPS COP2/MMI instructions, or anything Ghidra flagged with `DECOMPILE FAILED`.
1. **Primary Ground Truth — PS2 Prototype ELFs (`SLUS_202.28` & `SLPM_123.45`):**
   - Located under `/home/matthewb/Documents/SH2-Port/prototype_extracted/SLUS_202.28` (`Silent Hill 2 (Jul 13, 2001 prototype)/`) and `/home/matthewb/Documents/SH2-Port/e3_extracted/SLPM_123.45` (`Silent Hill 2 - Trial Version (E3 2001)/`).
   - Treat Team Silent's unstripped MIPS functions and DWARF-1 structs as the definitive specification. Inspect the relevant functions in `docs/decomp/slus/` & `docs/decomp/slpm/` (e.g., `clCheckHitEyeVectorWall` at `0x00139b90`, `SeCallPosDistanceF` at `0x00192bc0`) **before** coding.
2. **Secondary Ground Truth — Retail PC Binary (`sh2pc.exe`), Retail Assets & SH2:EE:**
   - Located under `/home/matthewb/Documents/SH2-Port/Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/` and `Silent-Hill-2-Enhancements/`.
   - Cross-check `docs/decomp/sh2pc/` (plus raw `.rdata`/`.data` tables, SH2:EE patches, and parsed retail binaries `.cld`, `.cam`, `.mdl`, `.anm`, `sddata.bin`) to confirm final retail parameter tables, layout constants, and bug fixes vs. the PS2 prototypes.
3. **Tertiary Reference ONLY — Community Ports (`sh23ds/`):**
   - **NEVER** trust or copy `sh23ds` shortcuts, clamped animation loops, or guessed constants as ground truth. Always verify every branch and constant against `SLUS_202.28` or `sh2pc.exe`.
4. **Modern `native-upgrade` Features (Mandatory Toggleability & 1:1 Vanilla Option):**
   - Every modern `native-upgrade` enhancement that did not exist on PS2 (e.g., `.cld` raycast acoustic wall occlusion, Binaural HRTF, OTS/FPS cameras, 4K textures, Foot IK) **must be exposed as an explicit, user-customizable toggle in the Options menu** so purist players can always configure a strict **1:1 original PS2** experience without forced modern enhancements.
   - Furthermore, **every touchpoint with game data** (coordinate axes, `.cld` `Thru`/material semantics, skeletal bone anchors, distance bounds, stage flags) **must still be verified 1:1 from `SLUS_202.28` / `sh2pc.exe`**. If any tuning parameter has no binary counterpart and is not specified in Jira, ask Matthew rather than guessing.
5. **Universal Full-Subsystem Verification & Zero-Placeholder Mandate (All Game Systems):**
   - **Binaries Always Override Jira & Design-Doc Prose:** Across **every** subsystem (UI/HUD, rendering/shaders/fog/lighting, audio, camera, collision/physics, skeletal animation, combat, enemy AI, puzzles, cutscenes, and stage events), Jira tickets and design docs are high-level summaries — never ground truth. Whenever ticket prose could differ from retail behavior or layout, disassemble **all** relevant functions in `SLUS_202.28` / `sh2pc.exe` (both logic/math **and** presentation/draw/dispatch/data-table functions) before writing code.
   - **Inspect Retail Assets Before Coding (Zero Placeholders):** Before implementing any subsystem that consumes retail assets (`.tex`, `.mdl`, `.anm`, `.map`, `.cld`, `.cam`, `.cls`, `.dds`, `.mes`, `.sdm`, `.aix`), inspect and decode the actual retail files on disk (including visual inspection of decoded `.tex` sheets for UI/2D/sprites and binary dumps of tables/structs). Never ship wireframe boxes, synthetic stand-ins, no-op stubs, or hardcoded `false`/disabled asset paths when the real retail assets exist on disk.
   - **Zero-Loophole Rule — No Inferred Geometry/Batch Roles:** Citing a real hex address or `.map` vertex coordinate does **not** excuse inferring what a `.map`/`.mdl` batch represents from bounding-box numbers alone (e.g., mistaking a mirror decal batch for a stain patch or replacing a solid room wall with a hardcoded fallback quad). Never clip, mask, or replace retail `.map` geometry unless proven 1:1 from `SLUS_202.28` / `sh2pc.exe` **and** visually confirmed against a retail PS2/PC (SH2:EE) reference frame.
   - **Strict Vanilla Baseline Protection:** No modern `wgpu` enhancement or re-implementation (custom planar mirror clipping, volumetric fog tuning, 4K texture sampling, post-processing) may alter or degrade retail geometry, fog, or lighting under the Vanilla PS2/PC baseline, nor be enabled by default without side-by-side visual verification against retail/SH2:EE.

## 4. Mandatory Quality Gates & 5-Layer 1:1 Parity Verification Protocol

Every PR must prove true 1:1 parity—not just "looks close"—through all five verification layers before `git commit`, Pull Request creation, or handoff to `@tech-lead`:

1. **Layer 1 — Complete Decomp Read (`docs/decomp/`):**
   - Read the entire relevant `.c` file(s) in `docs/decomp/{slus,slpm,sh2pc}` (not isolated snippets) before writing Rust.
   - Explicitly note any `SLUS_202.28` prototype vs. `sh2pc.exe` retail PC drift (e.g., retail balance fixes, +6 Born From a Wish attack ID shift, `ThroughDoorCam` retuning) in `///` doc comments and the PR `## Why` section.
2. **Layer 2 — Mandatory Pre-Commit Numeric Literal & Omitted-Function Audit:**
   - Inspect `git diff` for **every new or modified numeric literal, threshold, scale factor, struct offset, or flag**. Each literal must belong to one of three buckets:
     - Exact `SLUS_202.28` / `sh2pc.exe` symbol + hex address or retail asset offset (cited in `///` doc comments),
     - Exact mathematical/unit derivation from an already-cited constant, or
     - **No binary counterpart → STOP and ask Matthew** before writing or committing the value.
   - Cross-check the function index at the top of the target `docs/decomp/` `.c` file(s) to ensure no draw, dispatch, or state-transition functions were silently omitted. Never sneak a tuned "visual fix" float or shader hack into a commit during playtest iteration.
3. **Layer 3 — Deterministic Golden-Vector & Retail-Asset Tests:**
   - Every math, parser, collision, camera, combat, AI, audio, or event PR must include deterministic unit tests asserting exact agreement with `SLUS_202.28` / `sh2pc.exe` equations at boundary inputs (e.g., attenuation knees, angle cutoffs, struct byte layouts, HP/Endurance/hitback outputs) plus retail-asset integration tests against `Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/`.
4. **Layer 4 — Visual Reference & End-to-End Verification (UI, Rendering & FMV/Campaign PRs):**
   - **No Synthetic Stand-ins:** Headless tests that only assert `non_black_pixels > 0` or inject mocked completion flags (e.g., synthetic `fmv_finished = true`) do **not** satisfy Layer 4.
   - For **UI (`sh2-ui`)** PRs, render deterministic offscreen PNG snapshots (e.g., via `SH2_PREVIEW_OUT`) and **open/inspect them side-by-side** against retail PS2 / SH2:EE reference layouts before requesting review.
   - For **Rendering (`sh2-renderer`)** PRs, render the exact affected room(s) (e.g., Restroom `er01`, Observation Deck `ob01`, Forest Trail `wp01`, Wood Side Apartments) under **both** strict **Vanilla PS2** mode and **4K/Enhanced** mode (when applicable), and **visually inspect the PNGs side-by-side** against retail PS2 / SH2:EE reference frames (checking wall/mirror geometry, alpha-cutout foliage borders, fog density/color, shadows, and lighting).
   - For **FMV / Audio / Campaign** PRs, verify real retail assets end-to-end (e.g., full decode and exit transition of `toilet.bik` -> `first_toilet.dds`) rather than relying solely on synthetic state-machine triggers.
5. **Layer 5 — Static Checks & Adversarial Review:**
   - `cargo fmt --all -- --check` passes cleanly.
   - `cargo clippy --workspace --all-targets -- -D warnings` passes with **zero** warnings.
   - `cargo test --workspace` passes 100%.
   - Any modern `native-upgrade` feature is gated behind an explicit Options toggle so strict 1:1 Vanilla PS2 mode remains unaffected.

## 5. `/swarm`, Worktree Isolation & PR Governance

- **Design Out Collisions First:** In `/swarm`, the Coordinator assigns non-overlapping files/submodules per worker before coding starts. If a monolithic file blocks parallel work, split it into submodules first.
- **Worktree Isolation:** Launch `/swarm` workers with `workspace: "inherit"`. Coding workers never edit `/home/matthewb/dev/SH2` concurrently; each creates `/home/matthewb/dev/SH2-worktrees/<JIRA-KEY>` on branch `mb-u/<Discipline>/<JIRA-KEY>` off `origin/main` (symlinking `Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip`) and removes the worktree once the PR is reviewed.
- **PR Format:** Every PR must include `## What`, `## Why` (with `SH2-XXX` Jira key and PS2/PC symbol references), and `## Testing`.
- **Adversarial Tech Lead Review (Including Follow-Up Commits):** The `tech-lead` verifier must independently verify cited `SLUS_202.28` / `sh2pc.exe` symbols and **reject** any diff containing guessed constants, unverified material/flag assumptions, crate-layer violations, `sh2-app` bloat, monolithic god-files/functions, boolean-flag spaghetti, unverified `sh23ds` shortcuts, or missing tests/lints. Across **all** subsystems, Tech Lead reviews must also audit **omitted** retail functions and assets — rejecting any PR that relies on Jira prose over disassembly, leaves out key presentation/dispatch functions, ships placeholder visuals/stubs when retail assets exist, or hardcodes disabled preview/asset paths. Whenever playtest or follow-up commits are pushed after an initial approval, re-run the adversarial Tech Lead audit on the new commit range before merge. Final Tech Lead reviews **must** be posted directly on the GitHub PR via `add_issue_comment`.
- **No Self-Merging & Jira Status Discipline:** Never merge a PR into `main` (final merge sign-off belongs exclusively to Matthew), and **never** mark a Jira ticket `Done` before its PR is merged into `main` (keep it `In Progress` until merged).
