#!/usr/bin/env python3
"""
scripts/extract_symbols.py

Extracts symbols and DWARF debug data from the PS2 prototype ELFs:
- SLUS_202.28 (July 13, 2001 Prototype)
- SLPM_123.45 (E3 2001 Trial)

Outputs:
- prototypes/symbols_july13.json
- prototypes/symbols_e3.json
- include/sh2/sh2_types.h (and crates/sh2-core/src/types/)
"""

import os
import sys
import json
import struct
import subprocess

def parse_elf_symbols(elf_path):
    """Parse all symbols using readelf -s --wide."""
    cmd = ['readelf', '-s', '--wide', elf_path]
    proc = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    stdout, stderr = proc.communicate()
    if proc.returncode != 0:
        raise RuntimeError(f"readelf failed on {elf_path}: {stderr}")

    symbols = []
    symbol_map = {}
    categories = {
        "functions": [],
        "objects": [],
        "files": [],
        "debug_routines": [],
        "other": []
    }

    for line in stdout.splitlines():
        line = line.strip()
        if not line or not line[0].isdigit():
            continue
        parts = line.split(maxsplit=7)
        if len(parts) >= 8:
            idx = int(parts[0].rstrip(':'))
            val = parts[1]
            size_str = parts[2]
            size = int(size_str, 16) if size_str.startswith(('0x', '0X')) else int(size_str)
            stype = parts[3]
            bind = parts[4]
            vis = parts[5]
            ndx = parts[6]
            name = parts[7]

            entry = {
                "num": idx,
                "addr": f"0x{val.lower()}",
                "size": size,
                "type": stype,
                "bind": bind,
                "visibility": vis,
                "section": ndx,
                "name": name
            }
            symbols.append(entry)
            symbol_map[name] = entry

            if stype == "FUNC":
                categories["functions"].append(name)
                if any(k in name.lower() for k in ["dbg", "debug", "vrinteract", "printr"]):
                    categories["debug_routines"].append(name)
            elif stype == "OBJECT":
                categories["objects"].append(name)
            elif stype == "FILE":
                categories["files"].append(name)
            else:
                categories["other"].append(name)

    return {
        "file": elf_path,
        "total_symbols": len(symbols),
        "categories": {
            "functions_count": len(categories["functions"]),
            "objects_count": len(categories["objects"]),
            "files_count": len(categories["files"]),
            "debug_routines_count": len(categories["debug_routines"]),
            "other_count": len(categories["other"])
        },
        "debug_routines": sorted(list(set(categories["debug_routines"]))),
        "symbols": symbols
    }

def main():
    root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    os.chdir(root)
    os.makedirs("prototypes", exist_ok=True)
    os.makedirs("include/sh2", exist_ok=True)

    july_path = "prototype_extracted/SLUS_202.28"
    e3_path = "e3_extracted/SLPM_123.45"

    print(f"Extracting symbols from {july_path}...")
    july_data = parse_elf_symbols(july_path)
    july_out = "prototypes/symbols_july13.json"
    with open(july_out, "w") as f:
        json.dump(july_data, f, indent=2)
    print(f"Wrote {july_data['total_symbols']} symbols to {july_out}")

    print(f"Extracting symbols from {e3_path}...")
    e3_data = parse_elf_symbols(e3_path)
    e3_out = "prototypes/symbols_e3.json"
    with open(e3_out, "w") as f:
        json.dump(e3_data, f, indent=2)
    print(f"Wrote {e3_data['total_symbols']} symbols to {e3_out}")

if __name__ == "__main__":
    main()
