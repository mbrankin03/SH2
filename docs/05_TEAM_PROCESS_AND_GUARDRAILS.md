# Engineering Team Architecture & Operating Model (SH2)

## 1. Team Structure & Roles
- **Engineering Manager (EM):**
  - **Matthew (`mbrankin` / `mbrankin677@gmail.com`):** Primary EM & Project Lead.
  - *Governance:* Final PR sign-off and architectural decisions must be approved by Matthew along with an AI peer review before merging code into `main`.
- **AI Agent Software Engineers:**
  - **Frontend / Rendering Engineer:** Handles `wgpu`/Vulkan pipelines, volumetric compute shaders, atmospheric lighting, and AYN Thor dual-screen swapchains.
  - **Core Simulation Engineer:** Handles game loop timing (60 Hz decoupled ticks), camera systems, physics, puzzle state machines, and *Born from a Wish* stage programs.
  - **Asset & Pipeline Engineer:** Handles binary parsing of `.map`, `.mdls`, `.anm`, `.sdm`, DDS textures, and prototype symbol extractions.
  - **QA & Testing Engineer:** Headless verification against vanilla assets, regression checks, and automated unit tests.
  - **DevOps / Build Engineer:** Manages cargo workspace, Android NDK (`cargo-ndk`), ADB workflows, and packaging.

---

## 2. Scrum Methodology & Jira Guard Rails
- **Project Tracker:** Jira Cloud (`https://jauffre.atlassian.net`)
- **Project Key:** `SH2` (Project: *Silent Hill 2 Open Port*)
- **Issue Hierarchy:**
  - **Epic:** High-level project capability (e.g., *Phase 2: Prototype Ground Truth Extraction*, *Phase 4: Modern wgpu Renderer & Volumetric Atmosphere*, *Phase 6: Dual-Screen & Touch Companion UI*).
  - **Task / Story:** Discrete engineering deliverables with clear acceptance criteria.
  - **Subtask / Bug:** Isolated implementation items or regressions caught during QA.

---

## 3. Git Branching & Workflow Rules

### Branch Naming Convention
Branches **must** use developer-namespaced prefixes matching the Jira issue key:
```
mb-u/<Discipline>/<JIRA-KEY>
```
**Examples:**
- `mb-u/Pipeline/SH2-1`
- `mb-u/FrontEnd/SH2-5`
- `mb-u/Core/SH2-8`
- `mb-u/DevOps/SH2-12`

### Rules of Engagement
1. **No Monolithic Commits:** Code changes must be modular, highly cohesive, loosely coupled, and split across specialized crates (`crates/sh2-*`).
2. **Never Commit Directly to `main`:** All work goes through feature branches and Pull Requests.
3. **Workspace Isolation:** Subagents operate in isolated workspaces to prevent race conditions.

---

## 4. Software Engineering & Code Quality Standards

### Reverse-Engineering & Reference Ground-Truth Priority (CRITICAL)
When implementing game mechanics, animations, physics, FSM logic, enemy AI, damage tables, or collision systems:
1. **Primary Ground Truth: PS2 Debug Prototype ISO (`SLUS_202.28`)**:
   - Located at `Silent Hill 2 (Jul 13, 2001 prototype)/SILENT_HILL_2__VW047_U1__V0_10/SILENT_HILL_2__VW047_U1__V0_10.iso`.
   - Contains 26,211 unstripped symbols from Team Silent (`PlayerDamageMotionNo`, `lower_hold`, `upper_hold`, `lower_damage`, `shBattleAttackByEnemyHug`, `en*Function`).
   - **MANDATORY**: Always treat this Emotion Engine ELF as the definitive source of truth for original Konami logic, animation layouts, bone hierarchies, collision formulas, and physics parameters.
2. **Secondary Ground Truth: Retail PC Binary (`sh2pc.exe`) & Data Assets (`.anm`, `.mdl`, `.cld`)**:
   - Located at `Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2/sh2pc.exe`.
   - Contains final retail x86 assembly, damage lookup tables (`damage_no[2][2][30]` at `0x48a4e0`), exact keyframe intervals, and bug fixes applied in the final PC release.
3. **Tertiary Reference ONLY (Never Ground Truth): Downstream Community Ports (`sh23ds`)**:
   - Community ports such as `sh23ds` contain heavy approximations, clamped loops, and missing enemy-specific animations (e.g. replacing James's dedicated struggle clips with a generic `J_HURT` clamp).
   - **RULE**: Never prioritize or trust `sh23ds` shortcuts over the authentic PS2 ISO or retail PC binary. Only consult `sh23ds` as a secondary helper for how C structs were previously arranged, and verify every behavior against the PS2 ISO or `sh2pc.exe`.

- **Naming Conventions:**
  - Types, Structs, Enums, Traits: `PascalCase` (e.g. `VolumetricFogUniforms`, `MapMarkerState`).
  - Functions, Variables, Fields: `snake_case`.
  - Constants & Statics: `SCREAMING_SNAKE_CASE`.
- **Inline Documentation & Comments:**
  - Public items must include doc comments (`///`) explaining their purpose, invariants, and units.
  - Math matrices, MIPS opcode decoders, and format quirks must have inline comments (`//`) explaining the non-obvious *why*.
- **Formatting & Formatting Discipline:**
  - Strictly adhere to `cargo fmt` standards.
- **Linting & Safety:**
  - Zero compiler warnings allowed (`cargo clippy --workspace -- -D warnings`).
  - Maintain `#![forbid(unsafe_code)]` in asset format parser crates (`crates/sh2-assets`).

---

## 5. Pull Request (PR) Standard & Review Policy
Every Pull Request must strictly follow the **What / Why / Testing** template:

```markdown
## What
Detailed description of the technical changes, components modified, and new crates/modules introduced.

## Why
The engineering rationale behind the design decision, Jira ticket link (`SH2-XXX`), and architectural context.

## Testing
Comprehensive verification performed:
- Cargo tests run (`cargo test --workspace`)
- Linting and formatting status (`cargo clippy`, `cargo fmt`)
- Synthetic fixtures tested or in-engine validation against vanilla data.
- Edge cases evaluated.
```

### Tech Lead Review Policy & GitHub PR Comments
- **Mandatory PR Comment:** Tech Lead subagent reviews must **always be posted as comments directly on the GitHub Pull Request** (via GitHub MCP tool `add_issue_comment`), not merely stored in Antigravity chat or scratch artifacts.
- **Audit Trail & Documentation:** Leaving the review directly on the PR ensures easy documentation, historical traceability, and clear visibility for the Project Lead prior to merging.
- **Never Self-Merge:** Under no circumstances should an agent self-merge a PR into `main`. Merging is reserved exclusively for the Project Lead (Matthew).
