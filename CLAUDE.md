# Project: Silent Hill 2 Open Port (Rust)

Modern, open-source, memory-safe engine port of **Silent Hill 2 (Director's Cut / Restless Dreams)** written in Rust.
Target platforms: **Linux (x86_64)** and the **AYN Thor** dual-screen handheld (Android / ARM64).

## Architecture Overview
This is a Cargo workspace consisting of decoupled crates under `crates/`:
- `crates/sh2-core`: Game state, math wrappers, scenario manager, physics/collision structures.
- `crates/sh2-assets`: Asset ingestion, parsing formats (MDL, MAP, TXD, AFS, ADX, BIK).
- `crates/sh2-renderer`: `wgpu` 23.0 graphics pipeline, volumetric fog compute shaders, 32-bit float depth.
- `crates/sh2-audio`: `kira` audio backend, multi-channel environmental and streaming audio.
- `crates/sh2-ui`: UI rendering, companion dual-screen layout, touch UX, real-time map, developer console.
- `crates/sh2-debug`: Debug symbols, prototype inspection (E3 & July 13, 2001 builds), collision visualizers.
- `crates/sh2-app`: Main application entry point (`winit` 0.30 event loop).

## Primary Documentation Index
Before proposing architecture changes or writing features, consult the specs in `docs/`:
1. `docs/00_MASTER_ARCHITECTURE.md`: Core pillars, engine loop, rendering & audio philosophies.
2. `docs/01_TASK_ROADMAP.md`: Phase breakdown, current milestones, and task checklists.
3. `docs/02_FEATURE_PARITY_MATRIX.md`: EE relief and PS2/PC feature parity mapping.
4. `docs/03_DUAL_SCREEN_THOR_SPEC.md`: Clamshell dual-screen layouts, touch input, and lower screen companion specs.
5. `docs/04_PROTOTYPE_ROSETTA_STONE.md`: Prototype debug symbols, function signatures, and mapping guide.
6. `docs/05_TEAM_PROCESS_AND_GUARDRAILS.md`: Team roles, branch rules, Jira standards, and PR template.

## Git, Branching, Jira & Review Governance
All development must follow `docs/05_TEAM_PROCESS_AND_GUARDRAILS.md`:
1. **Never commit directly to `main`:** All work happens on feature branches.
2. **Branch Naming Standard:**
   ```
   mb-u/<Discipline>/<JIRA-KEY>
   ```
   *Examples:* `mb-u/UI/SH2-18`, `mb-u/Pipeline/SH2-1`, `mb-u/Core/SH2-8`, `mb-u/FrontEnd/SH2-5`
3. **Jira Ticket Standards (`https://jauffre.atlassian.net`):**
   - Every piece of work must map to an `SH2-XXX` issue key.
   - Describe Jira tickets with clear **Acceptance Criteria**, technical components touched, and reproduction/verification steps.
4. **Independent "Tech Lead" Review Gate (Mandatory Before Merge):**
   - **No Self-Approval:** An agent that authored or generated the code cannot approve or merge its own PR.
   - **Unbiased Architectural Review:** Every PR requires an independent, rigorous code review from an unbiased **Tech Lead** persona (either a separate reviewer session, a subagent reviewer, or an AI peer).
   - **Reviewer Checklist:**
     - Checks adherence to prototype ground truth (`docs/04_PROTOTYPE_ROSETTA_STONE.md`).
     - Verifies no silent regressions, memory safety violations, or hidden unwrap/panics.
     - Confirms code adheres to zero compiler warnings (`-D warnings`) and `#![forbid(unsafe_code)]` boundaries.
     - Validates test coverage and verifies that the PR template is completely fulfilled.
   - **Final Approval:** Only merged once both the **Tech Lead Review** and **Engineering Manager (Matthew)** sign off.
5. **Pull Request (PR) Requirements:**
   Every PR must strictly adhere to the **What / Why / Testing** standard:
   ```markdown
   ## What
   Detailed description of technical changes, modified components, and crates/modules introduced.

   ## Why
   Engineering rationale, Jira ticket reference (`SH2-XXX`), and architectural alignment.

   ## Testing
   Comprehensive verification performed:
   - `cargo test --workspace` status
   - `cargo clippy --workspace --all-targets -- -D warnings`
   - `cargo fmt --all -- --check`
   - Fixture and in-engine verification notes
   ```

## Build & Test Commands
- **Check Workspace**: `cargo check --workspace`
- **Build Development**: `cargo build`
- **Build Release**: `cargo build --release`
- **Run Tests**: `cargo test --workspace`
- **Clippy / Lint**: `cargo clippy --workspace --all-targets -- -D warnings`
- **Format Check**: `cargo fmt --all -- --check`

## Engineering Guardrails
1. **Never parse retail assets at runtime without validation:** Use clean, typed decoders in `crates/sh2-assets`. Maintain `#![forbid(unsafe_code)]` in asset format parser crates.
2. **Never scan large asset dumps directly:** Disc dumps (`Silent*`, `*_extracted/`, `target/`) are ignored via `.claudeignore`. Focus strictly on code in `crates/`, documentation in `docs/`, and test fixtures.
3. **No `unsafe` unless strictly isolated:** Any raw FFI or transmute operations must be isolated with safety documentation inside `sh2-core` or `sh2-assets`.
4. **Preserve Prototype Ground Truth:** Refer to `docs/04_PROTOTYPE_ROSETTA_STONE.md` when naming internal game structs and variables to match authentic Konami naming conventions wherever possible.

## Shared System Skills & Knowledge
Claude has access to Matthew's centralized skills repository at `~/.gemini/config/skills/`. For general homelab, hardware, or operational guidance, reference:
- Global instructions: `~/.claude/CLAUDE.md`
- Workstation architecture: `/home/matthewb/.gemini/config/skills/cachyos-migration-ops/SKILL.md`
- Homelab server setup: `/home/matthewb/.gemini/config/skills/homelab-manager/SKILL.md`
