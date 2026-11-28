//! Pure binary parser for Silent Hill 2 `.fcl` fog collision & area environment files.
//!
//! # Ground Truth Citations (`SLUS_202.28` & `sh2pc.exe`)
//! - `fogSetCollisionMain` (`SLUS_202.28` `0x0016d440`, `Fog/fogdata.c:235-532`)
//! - `fogSetCollisionMain2` (`SLUS_202.28` `0x0016dc50`, `Fog/fogdata.c:539-826`)
//! - `fogSetAreaEnvironment` (`SLUS_202.28` `0x0016e4a0`, `Fog/fogdata.c:833-881`)
//! - `fogSetEnvironment` (`SLUS_202.28` `0x00168dd0`, `Fog/fog.c:213-327`)
//! - `FUN_00488f40` (`sh2pc.exe` `0x00488f40`, `sh2pc_00480000.c:5119-5186`)
//!
//! # Binary Layout (Little-Endian)
//! - Header (`0x00..0x0c`, 12 bytes = `6 × u16`):
//!   - `+0x00`: `wall0_count: u16` (12-byte vertical wall segments, `6 × i16` scaled by `50.0` mm)
//!   - `+0x02`: `wall1_count: u16` (24-byte quad wall segments, `12 × i16` scaled by `50.0` mm)
//!   - `+0x04`: `obj1_count: u16` (16-byte sphere obstacles, `4 × f32`)
//!   - `+0x06`: `obj2_count: u16` (16-byte cylinder obstacles, `4 × f32`)
//!   - `+0x08`: `area_count: u16` (10-byte area rectangles, `5 × i16`)
//!   - `+0x0a`: `env_count: u16` (24-byte `FogEnv` records after 4-byte alignment)
//! - Area records: `area_count × 10` bytes (`s16[5]` = `(x_min, z_min, x_max, z_max, env_idx)`,
//!   where `x`/`z` are quantized in `50.0` mm units).
//! - Padding to 4-byte boundary: `env_offset = (area_offset + area_count * 10 + 3) & !3`.
//! - `FogEnv` records: `env_count × 24` (`0x18`) bytes each.

use crate::error::AssetError;

/// Header size of an `.fcl` file in bytes (`6 × u16`, `fogSetCollisionMain` `0x0016d440`).
pub const FCL_HEADER_BYTES: usize = 12;

/// Byte size of one `wall0` vertical wall record (`pfVar2 + 3` floats = 12 bytes, `0x0016d440`).
pub const FCL_WALL0_RECORD_BYTES: usize = 12;

/// Byte size of one `wall1` quad wall record (`pfVar27 + 6` floats = 24 bytes, `0x0016d440`).
pub const FCL_WALL1_RECORD_BYTES: usize = 24;

/// Byte size of one `obj1` / `obj2` obstacle record (`pfVar27 + 4` floats = 16 bytes, `0x0016d440`).
pub const FCL_OBJ_RECORD_BYTES: usize = 16;

/// Byte size of one area zone rectangle record (`s16[5]` = 10 bytes, `fogSetAreaEnvironment` `0x0016e4a0`).
pub const FCL_AREA_RECORD_BYTES: usize = 10;

/// Byte size of one `FogEnv` entry (`0x18` = 24 bytes, `fogSetAreaEnvironment` `0x0016e4a0:873`).
pub const FCL_ENV_RECORD_BYTES: usize = 24;

/// Quantization scale of `.fcl` wall and area coordinates in PS2 mm (`0x42480000 = 50.0f`,
/// `fogSetCollisionMain` `0x0016d440:279-280,388` and `fogSetAreaEnvironment` `0x0016e4a0:866-867`).
pub const FCL_COORD_SCALE_MM: f32 = 50.0;

/// Parsed 12-byte `.fcl` header (`fogSetCollisionMain` `0x0016d440`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FclHeader {
    /// Number of 12-byte vertical wall records (`+0x00`).
    pub wall0_count: u16,
    /// Number of 24-byte quad wall records (`+0x02`).
    pub wall1_count: u16,
    /// Number of 16-byte sphere obstacle records (`+0x04`).
    pub obj1_count: u16,
    /// Number of 16-byte cylinder obstacle records (`+0x06`).
    pub obj2_count: u16,
    /// Number of 10-byte area environment rectangles (`+0x08`).
    pub area_count: u16,
    /// Number of 24-byte `FogEnv` entries (`+0x0a`).
    pub env_count: u16,
}

/// Single 10-byte (`s16[5]`) `.fcl` area rectangle (`fogSetAreaEnvironment` `0x0016e4a0`).
///
/// Coordinates are quantized in [`FCL_COORD_SCALE_MM`] (`50.0` mm) units.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FclAreaZone {
    /// Minimum X coordinate in `50.0` mm units (`psVar4[0]`).
    pub x_min: i16,
    /// Minimum Z coordinate in `50.0` mm units (`psVar4[1]`).
    pub z_min: i16,
    /// Maximum X coordinate in `50.0` mm units (`psVar4[2]`).
    pub x_max: i16,
    /// Maximum Z coordinate in `50.0` mm units (`psVar4[3]`).
    pub z_max: i16,
    /// Index into [`FclFile::envs`] (`(ushort)psVar4[4]`).
    pub env_idx: u16,
}

impl FclAreaZone {
    /// Returns `true` if quantized player coordinates `(sx, sz)` lie inclusively inside this zone
    /// (`*psVar4 <= sVar2 && sVar2 <= psVar4[2] && psVar4[1] <= sVar3 && sVar3 <= psVar4[3]`,
    /// `fogSetAreaEnvironment` `0x0016e4a0`, `Fog/fogdata.c:871-872`).
    #[must_use]
    #[inline]
    pub const fn contains_quantized(&self, sx: i16, sz: i16) -> bool {
        self.x_min <= sx && sx <= self.x_max && self.z_min <= sz && sz <= self.z_max
    }

    /// Returns world-space `(x_min_mm, z_min_mm, x_max_mm, z_max_mm)` in PS2 mm.
    #[must_use]
    #[inline]
    pub fn bounds_mm(&self) -> (f32, f32, f32, f32) {
        (
            f32::from(self.x_min) * FCL_COORD_SCALE_MM,
            f32::from(self.z_min) * FCL_COORD_SCALE_MM,
            f32::from(self.x_max) * FCL_COORD_SCALE_MM,
            f32::from(self.z_max) * FCL_COORD_SCALE_MM,
        )
    }
}

/// Single 24-byte (`0x18`) `.fcl` `FogEnv` record (`fogSetEnvironment` `0x00168dd0`,
/// `sh2pc.exe` `FUN_00488f40` `0x00488f40`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FclFogEnv {
    /// Particle count (`+0x00`, `param_1[0]`; halved at night unless `stage > 4`).
    pub part_num: i16,
    /// Volume half-extent in PS2 mm (`+0x02`, `param_1[1]`, `_DAT_00581508` / `DAT_01f7a678`).
    pub area: i16,
    /// Particle billboard half-size/radius in PS2 mm (`+0x04`, `param_1[2]`, `_DAT_00581510` / `DAT_01f7a680`).
    pub size: i16,
    /// Obstacle/wall soft boundary thickness in PS2 mm (`+0x06`, `param_1[3]`, `_DAT_00581518` / `DAT_01f7a688`).
    pub p3: i16,
    /// Fixed floor/anchor Y in PS2 mm (`+0x08`, `param_1[4]`, `_DAT_00581520` / `DAT_01f7a690`).
    pub y_fix: i16,
    /// Altitude fade upper Y threshold in PS2 mm when `flags & 0x01 != 0` (`+0x0a`, `param_1[5]`, `_DAT_005814e0`).
    pub p5: i16,
    /// Altitude fade span in PS2 mm when `flags & 0x01 != 0` (`+0x0c`, `param_1[6]`, `_DAT_005814e4 = p5 - p6`).
    pub p6: i16,
    /// Y threshold in PS2 mm above which target alpha boosts to `127` (`+0x0e`, `param_1[7]`, `_DAT_005814e8`).
    pub p7: i16,
    /// Wind preset type `0..=0x12` (`+0x10`, `*(byte*)(param_1 + 8)`, `_DAT_00581530` / `DAT_01f7a6a0`).
    pub wind_type: u8,
    /// Environment bitflags (`+0x11`, `*(byte*)((int)param_1 + 0x11)`, `_DAT_00581538 & 0xff`):
    /// - `0x01`: altitude fade (`p5`/`p6`) enabled
    /// - `0x04`: distance-from-player alpha falloff in `fogMakePacket`
    /// - `0x08`: `floor_y` tracks `player_pos.y` per frame (`fogMakePacket` `Fog/fog.c:2338-2339`)
    /// - `0x10`: lock projection offset to `64.0`
    /// - `0x20`: X-axis projection shift in `fogSetLocalPosV` (`obGB.fcl`)
    /// - `0x40`: stay-pos override
    /// - `0x80`: disable `fog_part_obj` / `fog_part_obj2`
    pub flags: u8,
    /// Vertical subdivision shift (`+0x12`, `(char)param_1[9]`, `DAT_00581532` / `DAT_01f7a6a2`).
    pub subdiv: u8,
    /// Base particle & screen-fog alpha byte (`+0x13`, `*(byte*)((int)param_1 + 0x13)`, `_DAT_0058152e` / `DAT_01f7a69e`).
    pub alpha: u8,
    /// `fog_part_grid` density-gradient repulsion strength (`+0x14`, `*(f32*)(param_1 + 10)`,
    /// `_DAT_0058151c` / `_DAT_01f7a68c`). Replaced by SH2:EE `FogSpeedFix` (`0.25` / `0.50`)
    /// at `sh2pc.exe` `0x0048683D` (`Patches/FogAdjustments.cpp:238-306`).
    pub grid_repulsion_f14: f32,
}

/// Parsed Silent Hill 2 `.fcl` fog collision and area environment file.
#[derive(Debug, Clone, PartialEq)]
pub struct FclFile {
    /// 12-byte file header counts.
    pub header: FclHeader,
    /// Ordered area rectangles checked by [`Self::evaluate_area_env`] (`fogSetAreaEnvironment` `0x0016e4a0`).
    pub areas: Vec<FclAreaZone>,
    /// Environment presets referenced by [`Self::areas`] (index `0` is the stage default).
    pub envs: Vec<FclFogEnv>,
}

impl FclFile {
    /// Evaluates `fogSetAreaEnvironment` (`SLUS_202.28` `0x0016e4a0`, `Fog/fogdata.c:866-878`)
    /// at player world coordinates `(player_x_mm, player_z_mm)` in PS2 mm.
    ///
    /// Returns `Some((env_idx, &FclFogEnv))` for the first matching area rectangle, or falls back
    /// to `env[0]` when no rectangle contains the player.
    #[must_use]
    pub fn evaluate_area_env(
        &self,
        player_x_mm: f32,
        player_z_mm: f32,
    ) -> Option<(usize, &FclFogEnv)> {
        if self.envs.is_empty() {
            return None;
        }
        // `sVar2 = (short)(int)(player_x / 50.0); sVar3 = (short)(int)(player_z / 50.0);`
        let sx = (player_x_mm / FCL_COORD_SCALE_MM)
            .trunc()
            .clamp(f32::from(i16::MIN), f32::from(i16::MAX)) as i16;
        let sz = (player_z_mm / FCL_COORD_SCALE_MM)
            .trunc()
            .clamp(f32::from(i16::MIN), f32::from(i16::MAX)) as i16;

        for area in &self.areas {
            if area.contains_quantized(sx, sz) {
                let idx = usize::from(area.env_idx);
                if let Some(env) = self.envs.get(idx) {
                    return Some((idx, env));
                }
            }
        }
        self.envs.first().map(|env| (0, env))
    }
}

#[inline]
fn read_u16_le(bytes: &[u8], offset: usize) -> Result<u16, AssetError> {
    let slice = bytes
        .get(offset..offset + 2)
        .ok_or(AssetError::BufferUnderflow {
            required: offset + 2,
            available: bytes.len(),
        })?;
    Ok(u16::from_le_bytes([slice[0], slice[1]]))
}

#[inline]
fn read_i16_le(bytes: &[u8], offset: usize) -> Result<i16, AssetError> {
    Ok(read_u16_le(bytes, offset)? as i16)
}

#[inline]
fn read_f32_le(bytes: &[u8], offset: usize) -> Result<f32, AssetError> {
    let slice = bytes
        .get(offset..offset + 4)
        .ok_or(AssetError::BufferUnderflow {
            required: offset + 4,
            available: bytes.len(),
        })?;
    Ok(f32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

/// Parses a Silent Hill 2 `.fcl` binary buffer with full bounds checking (`#![forbid(unsafe_code)]`).
///
/// Mirrors `fogSetCollisionMain` (`0x0016d440`) and `fogSetAreaEnvironment` (`0x0016e4a0`) in `SLUS_202.28`.
pub fn parse_fcl(bytes: &[u8]) -> Result<FclFile, AssetError> {
    if bytes.len() < FCL_HEADER_BYTES {
        return Err(AssetError::BufferUnderflow {
            required: FCL_HEADER_BYTES,
            available: bytes.len(),
        });
    }

    let header = FclHeader {
        wall0_count: read_u16_le(bytes, 0)?,
        wall1_count: read_u16_le(bytes, 2)?,
        obj1_count: read_u16_le(bytes, 4)?,
        obj2_count: read_u16_le(bytes, 6)?,
        area_count: read_u16_le(bytes, 8)?,
        env_count: read_u16_le(bytes, 10)?,
    };

    let area_offset = FCL_HEADER_BYTES
        .checked_add(usize::from(header.wall0_count) * FCL_WALL0_RECORD_BYTES)
        .and_then(|o| o.checked_add(usize::from(header.wall1_count) * FCL_WALL1_RECORD_BYTES))
        .and_then(|o| o.checked_add(usize::from(header.obj1_count) * FCL_OBJ_RECORD_BYTES))
        .and_then(|o| o.checked_add(usize::from(header.obj2_count) * FCL_OBJ_RECORD_BYTES))
        .ok_or(AssetError::MalformedHeader("FCL section offset overflow"))?;

    let area_bytes = usize::from(header.area_count) * FCL_AREA_RECORD_BYTES;
    let area_end = area_offset
        .checked_add(area_bytes)
        .ok_or(AssetError::MalformedHeader("FCL area table overflow"))?;
    if area_end > bytes.len() {
        return Err(AssetError::BufferUnderflow {
            required: area_end,
            available: bytes.len(),
        });
    }

    let mut areas = Vec::with_capacity(usize::from(header.area_count));
    for i in 0..usize::from(header.area_count) {
        let base = area_offset + i * FCL_AREA_RECORD_BYTES;
        areas.push(FclAreaZone {
            x_min: read_i16_le(bytes, base)?,
            z_min: read_i16_le(bytes, base + 2)?,
            x_max: read_i16_le(bytes, base + 4)?,
            z_max: read_i16_le(bytes, base + 6)?,
            env_idx: read_u16_le(bytes, base + 8)?,
        });
    }

    // `_DAT_00581554 = (int)pfVar27 + area_count * 10 + 3 & 0xfffffffc` (`0x0016d440:527`)
    let env_offset = (area_end + 3) & !3;
    let env_bytes = usize::from(header.env_count) * FCL_ENV_RECORD_BYTES;
    let env_end = env_offset
        .checked_add(env_bytes)
        .ok_or(AssetError::MalformedHeader("FCL env table overflow"))?;
    if env_end > bytes.len() {
        return Err(AssetError::BufferUnderflow {
            required: env_end,
            available: bytes.len(),
        });
    }

    let mut envs = Vec::with_capacity(usize::from(header.env_count));
    for i in 0..usize::from(header.env_count) {
        let base = env_offset + i * FCL_ENV_RECORD_BYTES;
        envs.push(FclFogEnv {
            part_num: read_i16_le(bytes, base)?,
            area: read_i16_le(bytes, base + 2)?,
            size: read_i16_le(bytes, base + 4)?,
            p3: read_i16_le(bytes, base + 6)?,
            y_fix: read_i16_le(bytes, base + 8)?,
            p5: read_i16_le(bytes, base + 10)?,
            p6: read_i16_le(bytes, base + 12)?,
            p7: read_i16_le(bytes, base + 14)?,
            wind_type: bytes[base + 16],
            flags: bytes[base + 17],
            subdiv: bytes[base + 18],
            alpha: bytes[base + 19],
            grid_repulsion_f14: read_f32_le(bytes, base + 20)?,
        });
    }

    Ok(FclFile {
        header,
        areas,
        envs,
    })
}

/// Maps a lowercase stage/room code (e.g. `"ob01"`, `"ca11"`, `"wp01"`, `"cb54"`, `"cc01"`,
/// `"cd01"`, `"ps189"`, `"rr91"`) to its retail `.fcl` relative path under `data/bg/`
/// (`fogSetCollision` `SLUS_202.28` `0x0016c8a0`, `Fog/fogdata.c:187-223`).
#[must_use]
pub fn fcl_rel_path_for_stage(stage_code: &str) -> Option<&'static str> {
    let lower = stage_code.to_ascii_lowercase();
    if lower.starts_with("ob") {
        Some("data/bg/ob/obGB.fcl")
    } else if lower.starts_with("ca") || lower == "wp01" {
        Some("data/bg/ca/caGB.fcl")
    } else if lower.starts_with("cb") || matches!(lower.as_str(), "tw01" | "tw02") {
        Some("data/bg/cb/cbGB.fcl")
    } else if lower.starts_with("cc") || matches!(lower.as_str(), "tw11" | "tw12") {
        Some("data/bg/cc/ccGB.fcl")
    } else if lower.starts_with("cd") {
        Some("data/bg/cd/cdGB.fcl")
    } else if lower.starts_with("ps") || lower.starts_with("ed") {
        Some("data/bg/ps/psGB.fcl")
    } else if lower.starts_with("rr") || lower.starts_with("hp") {
        Some("data/bg/rr/rrGB.fcl")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn retail_bg_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2/data/bg")
    }

    #[test]
    fn parse_fcl_rejects_truncated_buffers() {
        assert!(parse_fcl(&[]).is_err());
        assert!(parse_fcl(&[0u8; 11]).is_err());
        // Header claims 1 env (24 bytes) but file ends after header
        let mut buf = [0u8; 12];
        buf[10..12].copy_from_slice(&1u16.to_le_bytes());
        assert!(parse_fcl(&buf).is_err());
    }

    #[test]
    fn retail_fcl_files_parse_cleanly_and_evaluate_area_zones() {
        let bg = retail_bg_dir();
        if !bg.exists() {
            return;
        }

        let expected = [
            ("ob/obGB.fcl", 0usize, 1usize),
            ("ca/caGB.fcl", 9, 8),
            ("cb/cbGB.fcl", 2, 3),
            ("cc/ccGB.fcl", 8, 8),
            ("cc/ccnw.fcl", 0, 1),
            ("cc/park.fcl", 1, 2),
            ("cd/cdGB.fcl", 1, 2),
            ("ps/psGB.fcl", 6, 7),
            ("rr/rrGB.fcl", 6, 6),
        ];

        for (rel, exp_areas, exp_envs) in expected {
            let path = bg.join(rel);
            let data = std::fs::read(&path).unwrap_or_else(|e| panic!("read {path:?}: {e}"));
            let fcl = parse_fcl(&data).unwrap_or_else(|e| panic!("parse {rel}: {e}"));
            assert_eq!(fcl.areas.len(), exp_areas, "{rel} area count");
            assert_eq!(fcl.envs.len(), exp_envs, "{rel} env count");
            for a in &fcl.areas {
                assert!(
                    usize::from(a.env_idx) < fcl.envs.len(),
                    "{rel} area env_idx {} out of bounds ({})",
                    a.env_idx,
                    fcl.envs.len()
                );
            }
        }

        // Verify exact `obGB.fcl` Env[0]
        let ob = parse_fcl(&std::fs::read(bg.join("ob/obGB.fcl")).unwrap()).unwrap();
        let (ob_idx, ob_env) = ob.evaluate_area_env(0.0, 0.0).unwrap();
        assert_eq!(ob_idx, 0);
        assert_eq!(ob_env.part_num, 700);
        assert_eq!(ob_env.area, 7000);
        assert_eq!(ob_env.size, 700);
        assert_eq!(ob_env.p3, 50);
        assert_eq!(ob_env.y_fix, 10000);
        assert_eq!(ob_env.wind_type, 0x12);
        assert_eq!(ob_env.flags, 0x21);
        assert_eq!(ob_env.subdiv, 4);
        assert_eq!(ob_env.alpha, 0x30);
        assert!((ob_env.grid_repulsion_f14 - 1.0).abs() < 1e-6);

        // Verify `caGB.fcl` zone transitions (`fogSetAreaEnvironment` 0x0016e4a0)
        let ca = parse_fcl(&std::fs::read(bg.join("ca/caGB.fcl")).unwrap()).unwrap();
        // Outside all rectangles -> default Env[0]
        let (ca_def_idx, ca_def) = ca.evaluate_area_env(-500_000.0, -500_000.0).unwrap();
        assert_eq!(ca_def_idx, 0);
        assert_eq!(ca_def.part_num, 400);
        assert_eq!(ca_def.area, 2000);
        assert_eq!(ca_def.size, 400);
        assert_eq!(ca_def.flags, 0x08);
        assert_eq!(ca_def.alpha, 0x10);
        assert!((ca_def.grid_repulsion_f14 - 2.0).abs() < 1e-6);

        // Inside area[7] (-400..=400, -400..=400) * 50 mm -> Env[6] (wind_type = 0x0f, alpha = 0x20)
        let (ca_z6_idx, ca_z6) = ca.evaluate_area_env(0.0, 0.0).unwrap();
        assert_eq!(ca_z6_idx, 6);
        assert_eq!(ca_z6.wind_type, 0x0f);
        assert_eq!(ca_z6.alpha, 0x20);

        // Inside area[0] (-3200..=-2440, -800..=0) * 50 mm -> (-160000..=-122000, -40000..=0) -> Env[1]
        let (ca_z1_idx, ca_z1) = ca.evaluate_area_env(-140_000.0, -20_000.0).unwrap();
        assert_eq!(ca_z1_idx, 1);
        assert_eq!(ca_z1.size, 300);
        assert_eq!(ca_z1.y_fix, -100);
        assert_eq!(ca_z1.flags, 0x00);

        // Inside area[2] (-2400..=-2000, -400..=400) * 50 mm -> Env[3]
        let (ca_z3_idx, ca_z3) = ca.evaluate_area_env(-110_000.0, 0.0).unwrap();
        assert_eq!(ca_z3_idx, 3);
        assert_eq!(ca_z3.area, 1500);
        assert_eq!(ca_z3.alpha, 0x06);

        // Verify `cbGB.fcl` and `ccGB.fcl` zone transitions
        let cb = parse_fcl(&std::fs::read(bg.join("cb/cbGB.fcl")).unwrap()).unwrap();
        assert_eq!(cb.evaluate_area_env(0.0, 0.0).unwrap().0, 0);
        assert_eq!(cb.evaluate_area_env(90_000.0, -80_000.0).unwrap().0, 1);
        assert_eq!(cb.evaluate_area_env(50_000.0, 52_500.0).unwrap().0, 2);

        let cc = parse_fcl(&std::fs::read(bg.join("cc/ccGB.fcl")).unwrap()).unwrap();
        assert_eq!(cc.evaluate_area_env(-500_000.0, -500_000.0).unwrap().0, 0);
        assert_eq!(cc.evaluate_area_env(0.0, 0.0).unwrap().0, 4);
        assert_eq!(cc.evaluate_area_env(20_000.0, 47_500.0).unwrap().0, 1);
    }
}
