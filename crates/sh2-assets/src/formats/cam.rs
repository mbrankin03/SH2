#![forbid(unsafe_code)]
//! Parser for Silent Hill 2 `.cam` binary camera volume definition files.
//!
//! Authentic layout:
//! Each `.cam` file contains an array of 128-byte road records (`_VC_ROAD_DATA`)
//! terminated by a record with flag `VC_RD_END_DATA_F` (`0x0001`).
//!
//! Ground truth references:
//! - PS2 beta `view/vc_main.c`, `sh2cam.h`, `sh2cam.c` (`vcConvertCamFile`)
//! - Sentinel values (e.g. `ofs_watch_hy == 10000.0` -> `-300.0`, `projection == 0.0` -> `448.0`)

use crate::error::AssetError;

/// Indicates whether a `.cam` camera trigger volume file was resolved from the
/// Silent Hill 2: Enhanced Edition override folder (`sh2e/bg/...` or `sh2e/bg2/...`)
/// or the vanilla retail folder (`data/bg/...` or `data/bg2/...`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CamSourceKind {
    /// Loaded from `sh2e/bg/<area>/<room>.cam` or `sh2e/bg2/<area>/<room>.cam`
    /// (widescreen/high-framerate corrected trigger volumes).
    Sh2eeOverride,
    /// Loaded from vanilla retail `data/bg/<area>/<room>.cam` or `data/bg2/<area>/<room>.cam`.
    #[default]
    VanillaRetail,
}

impl CamSourceKind {
    /// Short display label suitable for HUD telemetry and logging.
    #[inline]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Sh2eeOverride => "SH2:EE Override",
            Self::VanillaRetail => "Vanilla Retail",
        }
    }
}

/// Normalizes a room identifier (`"er01"`, `"ob01"`, `"rr21"`, `"ap01"`, `"ma61"`, `"ccGB"`)
/// or relative stage path (`"bg/er/er01.map"`, `"data/bg2/ma/ma61.cam"`, `"sh2e/bg/ap/ap01.cam"`)
/// into a canonical relative `.cam` path (`"bg/<area>/<room>.cam"` or `"bg2/ma/<room>.cam"`).
///
/// Returns `(canonical_room_id, relative_cam_path)` on valid input, or `None` if the identifier
/// does not contain at least a 2-letter area prefix.
pub fn normalize_room_cam_rel_path(room_or_rel_path: &str) -> Option<(String, String)> {
    let trimmed = room_or_rel_path
        .trim()
        .trim_start_matches("data/")
        .trim_start_matches("sh2e/")
        .trim_start_matches('/');
    if trimmed.is_empty() {
        return None;
    }

    // Extract the bare filename stem (e.g. "bg/er/er01.map" -> "er01", "ma61.cam" -> "ma61")
    let file_part = trimmed.rsplit('/').next().unwrap_or(trimmed);
    let stem = file_part
        .strip_suffix(".cam")
        .or_else(|| file_part.strip_suffix(".CAM"))
        .or_else(|| file_part.strip_suffix(".map"))
        .or_else(|| file_part.strip_suffix(".MAP"))
        .or_else(|| file_part.strip_suffix(".cld"))
        .or_else(|| file_part.strip_suffix(".CLD"))
        .unwrap_or(file_part);

    if stem.len() < 2 || !stem.as_bytes()[0..2].iter().all(|b| b.is_ascii_alphabetic()) {
        return None;
    }

    let area = stem[0..2].to_ascii_lowercase();
    // Preserve canonical room ID (lowercase area + suffix; note caGB/cbGB/ccGB/cdGB exist on disk)
    let room_id = if stem.len() == 4 && stem[2..4].eq_ignore_ascii_case("gb") {
        format!("{area}GB")
    } else {
        stem.to_ascii_lowercase()
    };

    let bg_root = if area == "ma" || trimmed.starts_with("bg2/") {
        "bg2"
    } else {
        "bg"
    };

    let rel_path = format!("{bg_root}/{area}/{room_id}.cam");
    Some((room_id.to_ascii_lowercase(), rel_path))
}

/// Returns the optional global area `.cam` relative path (`"bg/<area>/<area>GB.cam"`) for town
/// outdoor areas (`ca`, `cb`, `cc`, `cd`) when `room_or_rel_path` belongs to one of those areas
/// and is not already the `*GB.cam` file itself.
pub fn global_area_cam_rel_path(room_or_rel_path: &str) -> Option<String> {
    let (room_id, _) = normalize_room_cam_rel_path(room_or_rel_path)?;
    if room_id.ends_with("gb") || room_id.len() < 2 {
        return None;
    }
    let area = &room_id[0..2];
    match area {
        "ca" | "cb" | "cc" | "cd" => Some(format!("bg/{area}/{area}GB.cam")),
        _ => None,
    }
}

/// Flag indicating the terminal record in a `.cam` file array.
pub const VC_RD_END_DATA_F: u16 = 0x0001;
/// Camera warp in upon entering volume.
pub const VC_RD_WARP_IN_F: u16 = 0x0002;
/// Camera warp out upon exiting volume.
pub const VC_RD_WARP_OUT_F: u16 = 0x0004;
/// Warp in and out flags combined.
pub const VC_RD_WARP_IN_OUT_FS: u16 = 0x0006;
/// Inhibit flipping front orientation.
pub const VC_RD_NO_FRONT_FLIP_F: u16 = 0x0008;
/// Limit up far view.
pub const VC_RD_LIM_UP_FAR_VIEW_F: u16 = 0x0010;
/// Inhibit camera use when enemy is near.
pub const VC_RD_USE_NO_ENEMY_F: u16 = 0x0020;
/// Activate camera only when enemy is near.
pub const VC_RD_USE_NEAR_ENEMY_F: u16 = 0x0040;
/// Merge road volume boundaries.
pub const VC_RD_MARGE_ROAD_F: u16 = 0x0080;
/// Inhibit extra boundary padding.
pub const VC_RD_NO_EXTRA_AREA_F: u16 = 0x0100;
/// Camera behavior like settle mode.
pub const VC_RD_CAM_LIKE_SETTLE_F: u16 = 0x0200;
/// Inhibit camera warp.
pub const VC_RD_NOT_WARP_F: u16 = 0x0400;
/// Invalid self-view camera mode.
pub const VC_RD_INVALID_SV_F: u16 = 0x0800;

/// Road volume priority and behavioral category (`_VC_ROAD_TYPE`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum RoadType {
    #[default]
    Road = 0,
    Effect = 1,
    Event = 2,
    RoadPrioLow = 3,
    RoadPrioHigh = 4,
    SelfViewOnly = 5,
}

impl RoadType {
    /// Decode integer road type identifier into enum.
    pub fn from_raw(raw: i32) -> Self {
        match raw {
            0 => Self::Road,
            1 => Self::Effect,
            2 => Self::Event,
            3 => Self::RoadPrioLow,
            4 => Self::RoadPrioHigh,
            5 => Self::SelfViewOnly,
            _ => Self::Road,
        }
    }

    /// Evaluates priority when choosing candidate camera road (`vcRetRoadUsePriority`).
    pub fn priority(&self) -> i32 {
        match self {
            Self::Road => 2,
            Self::Effect => 5,
            Self::Event => 6,
            Self::RoadPrioLow => 1,
            Self::RoadPrioHigh => 3,
            Self::SelfViewOnly => 0,
        }
    }
}

/// Area size scaling profile (`_VC_AREA_SIZE_TYPE`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum AreaSizeType {
    #[default]
    Tiny = 0,
    Small = 1,
    Wide = 2,
    Outdoor = 3,
}

impl AreaSizeType {
    pub fn from_raw(raw: i32) -> Self {
        match raw {
            0 => Self::Tiny,
            1 => Self::Small,
            2 => Self::Wide,
            3 => Self::Outdoor,
            _ => Self::Tiny,
        }
    }
}

/// Camera movement algorithm (`_VC_CAM_MV_TYPE`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum CamMovementType {
    /// Chase camera tracking along rail behind character.
    #[default]
    Chase = 0,
    /// Stationary base position with tracking look target.
    Settle = 1,
    /// Completely fixed angle and position (cinematic room angles).
    FixedAngle = 2,
    /// First-person inspection search camera.
    SelfView = 3,
    /// Locus circular orbital track around interest point.
    LocusCircle = 4,
    /// Door transit portal tracking.
    ThroughDoor = 5,
}

impl CamMovementType {
    pub fn from_raw(raw: i32) -> Self {
        match raw {
            0 => Self::Chase,
            1 => Self::Settle,
            2 => Self::FixedAngle,
            3 => Self::SelfView,
            4 => Self::LocusCircle,
            5 => Self::ThroughDoor,
            _ => Self::Chase,
        }
    }
}

/// Bounding region in 3D world space (`_VC_ROAD_AREA`, 32 bytes).
///
/// In PS2 world coordinates:
/// - `(x0, z0)`, `(x1, z1)`, `(x2, z2)` define the horizontal rectangular envelope.
/// - `min_hy`: Lower Y limit (numerically larger float because Y points down).
/// - `max_hy`: Upper Y limit (numerically smaller float because Y points down).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct RoadArea {
    pub x0: f32,
    pub z0: f32,
    pub x1: f32,
    pub z1: f32,
    pub x2: f32,
    pub z2: f32,
    pub min_hy: f32,
    pub max_hy: f32,
}

impl RoadArea {
    /// Returns the minimum and maximum X coordinate envelope.
    pub fn x_bounds(&self) -> (f32, f32) {
        let min_x = self.x0.min(self.x1).min(self.x2);
        let max_x = self.x0.max(self.x1).max(self.x2);
        (min_x, max_x)
    }

    /// Returns the minimum and maximum Z coordinate envelope.
    pub fn z_bounds(&self) -> (f32, f32) {
        let min_z = self.z0.min(self.z1).min(self.z2);
        let max_z = self.z0.max(self.z1).max(self.z2);
        (min_z, max_z)
    }

    /// Returns the vertical Y bounds `(min_y, max_y)`.
    /// Note: in PS2 coords, `max_hy` is numerically lower than `min_hy`.
    pub fn y_bounds(&self) -> (f32, f32) {
        let min_y = self.min_hy.min(self.max_hy);
        let max_y = self.min_hy.max(self.max_hy);
        (min_y, max_y)
    }

    /// Returns center point `(x, y, z)` in world units.
    pub fn center(&self) -> [f32; 3] {
        let (min_x, max_x) = self.x_bounds();
        let (min_z, max_z) = self.z_bounds();
        let y = (self.min_hy + self.max_hy) * 0.5;
        [(min_x + max_x) * 0.5, y, (min_z + max_z) * 0.5]
    }

    /// Checks if a 3D point `[x, y, z]` lies inside this bounding volume.
    pub fn contains_point(&self, pos: [f32; 3], margin: f32) -> bool {
        let (min_x, max_x) = self.x_bounds();
        let (min_z, max_z) = self.z_bounds();
        let (min_y, max_y) = self.y_bounds();

        pos[0] >= (min_x - margin)
            && pos[0] <= (max_x + margin)
            && pos[2] >= (min_z - margin)
            && pos[2] <= (max_z + margin)
            && pos[1] >= (min_y - margin)
            && pos[1] <= (max_y + margin)
    }

    /// Computes the 2D Euclidean distance from `(x, z)` to the horizontal bounding rectangle.
    /// Returns 0.0 if inside.
    pub fn distance_to_xz(&self, x: f32, z: f32) -> f32 {
        let (min_x, max_x) = self.x_bounds();
        let (min_z, max_z) = self.z_bounds();

        let dx = if x < min_x {
            min_x - x
        } else if x > max_x {
            x - max_x
        } else {
            0.0
        };

        let dz = if z < min_z {
            min_z - z
        } else if z > max_z {
            z - max_z
        } else {
            0.0
        };

        (dx * dx + dz * dz).sqrt()
    }
}

/// Chase camera parameters (`_VC_CHASE_CAM_PARAM`).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ChaseCamParam {
    pub ofs_hy: f32,
    pub ratio_r_xz: f32,
    pub lr_lim_ang_y: f32,
    pub rr_lim_ang_y: f32,
}

/// Settle camera parameters (`_VC_SETTLE_CAM_PARAM`).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct SettleCamParam {
    pub sta_base_ang_y: f32,
    pub end_base_ang_y: f32,
    pub lr_lim_ang_y: f32,
    pub rr_lim_ang_y: f32,
}

/// Fixed angle camera parameters (`_VC_FIX_ANG_CAM_PARAM`).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct FixAngCamParam {
    pub ang_x: f32,
    pub ang_y: f32,
    pub ofs_hy: f32,
    pub cam2wth_dist: f32,
}

/// Locus circle camera parameters (`_VC_LOCUS_CIRCLE_CAM_PARAM`).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct LocusCircleCamParam {
    pub origin_x: f32,
    pub origin_z: f32,
    pub ang_y: f32,
    pub radius: f32,
}

/// Mode-specific camera parameters (16 bytes).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CamMovementParams {
    Chase(ChaseCamParam),
    Settle(SettleCamParam),
    FixedAngle(FixAngCamParam),
    LocusCircle(LocusCircleCamParam),
    Raw([u8; 16]),
}

impl Default for CamMovementParams {
    fn default() -> Self {
        Self::Raw([0u8; 16])
    }
}

/// Decoded 128-byte road record (`_VC_ROAD_DATA`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RoadRecord {
    /// Switch trigger area (`lim_sw`).
    pub lim_sw: RoadArea,
    /// Camera track / rail area (`lim_rd`).
    pub lim_rd: RoadArea,
    /// Sub-area classification identifier.
    pub kind_id: i32,
    /// Bitwise flags (`_VC_ROAD_FLAGS`).
    pub flags: u16,
    /// Size type scaling (`_VC_AREA_SIZE_TYPE`).
    pub area_size_type: AreaSizeType,
    /// Road priority type (`_VC_ROAD_TYPE`).
    pub rd_type: RoadType,
    /// Vertical motion profile type (`mv_y_type`).
    pub mv_y_type: i32,
    /// Watch target height offset.
    pub ofs_watch_hy: f32,
    /// Trace bottom height.
    pub trace_btm_hy: f32,
    /// Primary direction axis (0 = along X, 1 = along Z).
    pub rd_dir_type: i32,
    /// Projection screen Z distance (focal length).
    pub projection: f32,
    /// Projection volume (unused by retail engine).
    pub proj_volume: f32,
    /// Blend duration in seconds when transitioning projection.
    pub proj_sec: f32,
    /// Camera motion algorithm type.
    pub cam_mv_type: CamMovementType,
    /// Mode-specific movement parameters.
    pub params: CamMovementParams,
}

/// Parsed `.cam` file containing camera trigger volumes and rails.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CamFile {
    /// Active road records (terminator record omitted).
    pub records: Vec<RoadRecord>,
}

impl CamFile {
    /// Evaluates which road record is the best candidate for the given player position `[x, y, z]`.
    ///
    /// Implements priority hierarchy matching PS2 / PC `vcGetBestNewCurNearRoad`:
    /// 1. Candidate must contain the point (within `margin`) or have the smallest distance.
    /// 2. Higher `road_type.priority()` wins among intersecting volumes.
    pub fn find_active_record(&self, pos: [f32; 3], margin: f32) -> Option<&RoadRecord> {
        let mut best_record: Option<&RoadRecord> = None;
        let mut best_dist = f32::MAX;
        let mut best_prio = -1;

        for rec in &self.records {
            let inside = rec.lim_sw.contains_point(pos, margin);
            let dist = if inside {
                0.0
            } else {
                rec.lim_sw.distance_to_xz(pos[0], pos[2])
            };

            let prio = rec.rd_type.priority();

            if inside {
                if prio > best_prio || (prio == best_prio && dist < best_dist) {
                    best_record = Some(rec);
                    best_prio = prio;
                    best_dist = dist;
                }
            } else if best_prio <= 0 && dist < best_dist {
                best_record = Some(rec);
                best_dist = dist;
            }
        }

        best_record
    }

    /// Appends additional road records from `other` (e.g., an area-wide `<area>GB.cam` file
    /// or adjacent room `.cam` file in a multi-room stage).
    pub fn extend_records(&mut self, other: CamFile) {
        self.records.extend(other.records);
    }
}

/// Resolved and parsed `.cam` asset along with its room identifier, source pack, and disk path (`SH2-50`).
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedCamAsset {
    /// Canonical lowercase room identifier (e.g. `"er01"`, `"ob01"`, `"rr21"`, `"ap01"`, `"ma61"`).
    pub room_id: String,
    /// Relative stage path under `data/` or `sh2e/` (e.g. `"bg/er/er01.cam"`, `"bg2/ma/ma61.cam"`).
    pub relative_path: String,
    /// Full resolved filesystem path.
    pub resolved_path: std::path::PathBuf,
    /// Whether the `.cam` file came from `sh2e/` (`Sh2eeOverride`) or `data/` (`VanillaRetail`).
    pub source: CamSourceKind,
    /// Parsed camera road records.
    pub cam_file: CamFile,
}

fn read_f32_le(slice: &[u8], offset: usize) -> f32 {
    let bytes = [
        slice[offset],
        slice[offset + 1],
        slice[offset + 2],
        slice[offset + 3],
    ];
    f32::from_le_bytes(bytes)
}

fn read_i32_le(slice: &[u8], offset: usize) -> i32 {
    let bytes = [
        slice[offset],
        slice[offset + 1],
        slice[offset + 2],
        slice[offset + 3],
    ];
    i32::from_le_bytes(bytes)
}

fn read_u16_le(slice: &[u8], offset: usize) -> u16 {
    let bytes = [slice[offset], slice[offset + 1]];
    u16::from_le_bytes(bytes)
}

fn read_road_area(slice: &[u8], offset: usize) -> RoadArea {
    RoadArea {
        x0: read_f32_le(slice, offset),
        z0: read_f32_le(slice, offset + 4),
        x1: read_f32_le(slice, offset + 8),
        z1: read_f32_le(slice, offset + 12),
        x2: read_f32_le(slice, offset + 16),
        z2: read_f32_le(slice, offset + 20),
        min_hy: read_f32_le(slice, offset + 24),
        max_hy: read_f32_le(slice, offset + 28),
    }
}

/// Parses a binary `.cam` file into a [`CamFile`], applying authentic PS2 sentinel substitutions.
///
/// Each 128-byte record matches `_VC_ROAD_DATA`:
/// - `+0x00`: `lim_sw` (32 bytes)
/// - `+0x20`: `lim_rd` (32 bytes)
/// - `+0x40`: `kind_id` (i32)
/// - `+0x44`: `flags` (u16 flags, u16 padding)
/// - `+0x48`: `area_size_type` (i32)
/// - `+0x4C`: `rd_type` (i32)
/// - `+0x50`: `mv_y_type` (i32)
/// - `+0x54`: `ofs_watch_hy` (f32)
/// - `+0x58`: `trace_btm_hy` (f32)
/// - `+0x5C`: `rd_dir_type` (i32)
/// - `+0x60`: `projection` (f32)
/// - `+0x64`: `proj_volume` (f32)
/// - `+0x68`: `proj_sec` (f32)
/// - `+0x6C`: `cam_mv_type` (i32)
/// - `+0x70`: `tmp` parameters (16 bytes)
pub fn parse_cam(data: &[u8]) -> Result<CamFile, AssetError> {
    const RECORD_SIZE: usize = 128;

    if data.len() < RECORD_SIZE {
        return Err(AssetError::BufferUnderflow {
            required: RECORD_SIZE,
            available: data.len(),
        });
    }

    if !data.len().is_multiple_of(RECORD_SIZE) {
        return Err(AssetError::InvalidData(format!(
            "CAM data length {} is not a multiple of 128 bytes",
            data.len()
        )));
    }

    let record_count = data.len() / RECORD_SIZE;
    let mut records = Vec::with_capacity(record_count);

    for i in 0..record_count {
        let offset = i * RECORD_SIZE;
        let rec = &data[offset..offset + RECORD_SIZE];

        let flags = read_u16_le(rec, 0x44);

        // Terminal record check
        if (flags & VC_RD_END_DATA_F) != 0 {
            break;
        }

        let lim_sw = read_road_area(rec, 0x00);
        let lim_rd = read_road_area(rec, 0x20);

        let kind_id = (read_i32_le(rec, 0x40) as i16) as i32;
        let area_size_type = AreaSizeType::from_raw(read_i32_le(rec, 0x48));
        let rd_type = RoadType::from_raw(read_i32_le(rec, 0x4C));
        let mv_y_type = read_i32_le(rec, 0x50);

        let mut ofs_watch_hy = read_f32_le(rec, 0x54);
        let mut trace_btm_hy = read_f32_le(rec, 0x58);
        let rd_dir_type = read_i32_le(rec, 0x5C);
        let mut projection = read_f32_le(rec, 0x60);
        let proj_volume = read_f32_le(rec, 0x64);
        let proj_sec = read_f32_le(rec, 0x68);
        let cam_mv_type = CamMovementType::from_raw(read_i32_le(rec, 0x6C));

        // Authentic PS2 / PC `vcConvertCamFile` sentinel substitutions:
        // ofs_watch_hy
        if (ofs_watch_hy - 10000.0).abs() < f32::EPSILON {
            ofs_watch_hy = -300.0;
        } else if (ofs_watch_hy - 10001.0).abs() < f32::EPSILON {
            ofs_watch_hy = -375.0;
        } else if (ofs_watch_hy - 10002.0).abs() < f32::EPSILON {
            ofs_watch_hy = -600.0;
        }

        // projection
        if projection.abs() < f32::EPSILON {
            projection = 448.0;
        } else if (projection - 10001.0).abs() < f32::EPSILON {
            projection = 384.0;
        }

        // trace_btm_hy
        if (trace_btm_hy - 10000.0).abs() < f32::EPSILON {
            trace_btm_hy = -300.0;
        } else if (trace_btm_hy - 10001.0).abs() < f32::EPSILON {
            trace_btm_hy = -150.0;
        } else if (trace_btm_hy - 10002.0).abs() < f32::EPSILON {
            trace_btm_hy = 150.0;
        }

        // Decode tmp parameter block based on cam_mv_type with sentinels
        let params = match cam_mv_type {
            CamMovementType::Chase => {
                let mut ofs_hy = read_f32_le(rec, 0x70);
                if (ofs_hy - 10000.0).abs() < f32::EPSILON {
                    ofs_hy = -500.0;
                } else if (ofs_hy - 10001.0).abs() < f32::EPSILON {
                    ofs_hy = -125.0;
                }
                CamMovementParams::Chase(ChaseCamParam {
                    ofs_hy,
                    ratio_r_xz: read_f32_le(rec, 0x74),
                    lr_lim_ang_y: read_f32_le(rec, 0x78),
                    rr_lim_ang_y: read_f32_le(rec, 0x7C),
                })
            }
            CamMovementType::Settle => CamMovementParams::Settle(SettleCamParam {
                sta_base_ang_y: read_f32_le(rec, 0x70),
                end_base_ang_y: read_f32_le(rec, 0x74),
                lr_lim_ang_y: read_f32_le(rec, 0x78),
                rr_lim_ang_y: read_f32_le(rec, 0x7C),
            }),
            CamMovementType::FixedAngle => {
                let mut ang_x = read_f32_le(rec, 0x70);
                if (ang_x - 10000.0).abs() < f32::EPSILON {
                    ang_x = -std::f32::consts::FRAC_PI_6; // -30 degrees in radians (-0.5235988)
                }
                CamMovementParams::FixedAngle(FixAngCamParam {
                    ang_x,
                    ang_y: read_f32_le(rec, 0x74),
                    ofs_hy: read_f32_le(rec, 0x78),
                    cam2wth_dist: read_f32_le(rec, 0x7C),
                })
            }
            CamMovementType::LocusCircle => CamMovementParams::LocusCircle(LocusCircleCamParam {
                origin_x: read_f32_le(rec, 0x70),
                origin_z: read_f32_le(rec, 0x74),
                ang_y: read_f32_le(rec, 0x78),
                radius: read_f32_le(rec, 0x7C),
            }),
            _ => {
                let mut raw = [0u8; 16];
                raw.copy_from_slice(&rec[0x70..0x80]);
                CamMovementParams::Raw(raw)
            }
        };

        records.push(RoadRecord {
            lim_sw,
            lim_rd,
            kind_id,
            flags,
            area_size_type,
            rd_type,
            mv_y_type,
            ofs_watch_hy,
            trace_btm_hy,
            rd_dir_type,
            projection,
            proj_volume,
            proj_sec,
            cam_mv_type,
            params,
        });
    }

    Ok(CamFile { records })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_cam_buffer_underflow() {
        let small = [0u8; 64];
        assert!(matches!(
            parse_cam(&small),
            Err(AssetError::BufferUnderflow { .. })
        ));
    }

    #[test]
    fn test_parse_cam_invalid_length() {
        let invalid = [0u8; 130];
        assert!(matches!(
            parse_cam(&invalid),
            Err(AssetError::InvalidData(..))
        ));
    }

    #[test]
    fn test_parse_cam_sentinels() {
        let mut buffer = [0u8; 256];
        // Record 0: Normal with sentinels
        // flags = 0
        // ofs_watch_hy = 10000.0
        buffer[0x54..0x58].copy_from_slice(&10000.0f32.to_le_bytes());
        // projection = 0.0 -> 448.0
        buffer[0x60..0x64].copy_from_slice(&0.0f32.to_le_bytes());
        // trace_btm_hy = 10001.0 -> -150.0
        buffer[0x58..0x5C].copy_from_slice(&10001.0f32.to_le_bytes());
        // cam_mv_type = Chase (0)
        buffer[0x6C..0x70].copy_from_slice(&0i32.to_le_bytes());
        // tmp.chs.ofs_hy = 10000.0 -> -500.0
        buffer[0x70..0x74].copy_from_slice(&10000.0f32.to_le_bytes());

        // Record 1: End marker
        buffer[0x80 + 0x44] = 0x01; // VC_RD_END_DATA_F

        let cam = parse_cam(&buffer).expect("Should parse test cam buffer");
        assert_eq!(cam.records.len(), 1);
        let rec = &cam.records[0];
        assert_eq!(rec.ofs_watch_hy, -300.0);
        assert_eq!(rec.projection, 448.0);
        assert_eq!(rec.trace_btm_hy, -150.0);
        if let CamMovementParams::Chase(ref chs) = rec.params {
            assert_eq!(chs.ofs_hy, -500.0);
        } else {
            panic!("Expected Chase params");
        }
    }

    #[test]
    fn test_normalize_room_cam_rel_path() {
        assert_eq!(
            normalize_room_cam_rel_path("er01"),
            Some(("er01".to_string(), "bg/er/er01.cam".to_string()))
        );
        assert_eq!(
            normalize_room_cam_rel_path("bg/ob/ob01.map"),
            Some(("ob01".to_string(), "bg/ob/ob01.cam".to_string()))
        );
        assert_eq!(
            normalize_room_cam_rel_path("data/bg/rr/rr21.cld"),
            Some(("rr21".to_string(), "bg/rr/rr21.cam".to_string()))
        );
        assert_eq!(
            normalize_room_cam_rel_path("ma61"),
            Some(("ma61".to_string(), "bg2/ma/ma61.cam".to_string()))
        );
        assert_eq!(
            normalize_room_cam_rel_path("sh2e/bg2/ma/ma69.cam"),
            Some(("ma69".to_string(), "bg2/ma/ma69.cam".to_string()))
        );
        assert_eq!(
            normalize_room_cam_rel_path("ccGB"),
            Some(("ccgb".to_string(), "bg/cc/ccGB.cam".to_string()))
        );
        assert_eq!(
            global_area_cam_rel_path("cc11"),
            Some("bg/cc/ccGB.cam".to_string())
        );
        assert_eq!(global_area_cam_rel_path("er01"), None);
    }
}
