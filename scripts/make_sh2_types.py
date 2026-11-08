#!/usr/bin/env python3
"""
Modular Silent Hill 2 C Header Verification & Generation Script
===============================================================
Validates the modular C11 headers under `include/sh2/`:
  - `include/sh2/sh2_math.h`
  - `include/sh2/sh2_collision.h`
  - `include/sh2/sh2_pad.h`
  - `include/sh2/sh2_chara.h`
  - `include/sh2/sh2_camera.h`
  - `include/sh2/sh2_player.h`
  - `include/sh2/sh2_types.h` (umbrella header)

Ensures that `include/sh2/sh2_types.h` includes all subsystem headers and passes
C11 `_Static_assert` checks on 64-bit host builds via `sh2_ptr32_t`.
"""

import os
import subprocess
import sys

REPO_ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
INCLUDE_DIR = os.path.join(REPO_ROOT, "include", "sh2")

MODULAR_HEADERS = [
    "sh2_math.h",
    "sh2_collision.h",
    "sh2_pad.h",
    "sh2_chara.h",
    "sh2_camera.h",
    "sh2_player.h",
]

UMBRELLA_CONTENT = """/**
 * @file sh2_types.h
 * @brief Silent Hill 2 Core Engine Types Umbrella Header
 *
 * Extracted from PS2 EE DWARF-1 symbols in SLUS_202.28.
 * Includes modular subsystem headers covering math, collision, controller input,
 * characters/skeletons/combat, virtual camera, and player workspace state.
 */

#ifndef SH2_TYPES_H
#define SH2_TYPES_H

#include "sh2_math.h"
#include "sh2_collision.h"
#include "sh2_pad.h"
#include "sh2_chara.h"
#include "sh2_camera.h"
#include "sh2_player.h"

#endif /* SH2_TYPES_H */
"""


def main() -> int:
    os.makedirs(INCLUDE_DIR, exist_ok=True)
    umbrella_path = os.path.join(INCLUDE_DIR, "sh2_types.h")
    with open(umbrella_path, "w", encoding="utf-8") as f:
        f.write(UMBRELLA_CONTENT)

    missing = [
        h for h in MODULAR_HEADERS if not os.path.isfile(os.path.join(INCLUDE_DIR, h))
    ]
    if missing:
        print(f"ERROR: Missing modular headers in {INCLUDE_DIR}: {missing}", file=sys.stderr)
        return 1

    cmd = [
        "gcc",
        "-std=c11",
        "-Wall",
        "-Wextra",
        "-Werror",
        "-fsyntax-only",
        umbrella_path,
    ]
    res = subprocess.run(cmd, capture_output=True, text=True)
    if res.returncode != 0:
        print(f"ERROR: C11 validation failed:\n{res.stderr}", file=sys.stderr)
        return res.returncode

    print(f"Verified modular headers in {INCLUDE_DIR} (all C11 _Static_assert checks passed).")
    return 0


if __name__ == "__main__":
    sys.exit(main())
