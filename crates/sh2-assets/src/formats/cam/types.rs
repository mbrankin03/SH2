//! `.cam` road flags, enums, movement parameter blocks, and [`CamFile`] container.

use super::area::RoadArea;

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

/// Locus circle camera parameters (`_VC_LOCUS_CIRCLE_CAM_PARAM`, 16 bytes at `+0x70`).
///
/// Ground truth:
/// - PS2 `SLUS_202.28` DWARF `_VC_LOCUS_CIRCLE_CAM_PARAM` (`0x1aff8f`) &
///   `vcMakeIdealCamPosForLocusCircleCam` (`0x0019a9c0`).
/// - Retail PC `sh2pc.exe` `vcMakeIdealCamPosForLocusCircleCam` (`0x0051b1e0`).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct LocusCircleCamParam {
    /// Orbit corner origin X in PS2 world millimeters (`0.0` selects `lim_sw` quadrant corner).
    pub origin_x: f32,
    /// Orbit corner origin Z in PS2 world millimeters (`0.0` selects `lim_sw` quadrant corner).
    pub origin_z: f32,
    /// Sector center yaw angle in radians (`< -PI` or `>= PI` also inverts vertical switch ratio).
    pub ang_y: f32,
    /// Orbital extension radius in PS2 world millimeters.
    pub radius: f32,
}

/// Through-door camera parameters (`_VC_THROUGH_DOOR_CAM_PARAM` 16-byte `.cam` rail block at `+0x70`).
///
/// Ground truth:
/// - PS2 `SLUS_202.28` DWARF `_VC_THROUGH_DOOR_CAM_PARAM` (`0x1afc54`),
///   `vcMakeIdealCamPosForThroughDoorCam` (`0x0019a820`), and `vcConvertCamFile` (`0x001a6c20`).
/// - Retail PC `sh2pc.exe` `vcMakeIdealCamPosForThroughDoorCam` (`0x0051b0c0`) and
///   `vcConvertCamFile` (`0x00521960`).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ThroughDoorCamParam {
    /// Outward doorway rail yaw angle in radians (`0.0` = `+Z`, `+PI/2` = `+X`).
    pub rail_ang_y: f32,
    /// Doorway rail anchor position `[x, y, z]` in PS2 world millimeters (Y-down).
    pub rail_sta_pos: [f32; 3],
}

/// Mode-specific camera parameters (16 bytes at `_VC_ROAD_DATA + 0x70`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CamMovementParams {
    /// Mode 0 (`Chase`) parameters (`_VC_CHASE_CAM_PARAM`).
    Chase(ChaseCamParam),
    /// Mode 1 (`Settle`) parameters (`_VC_SETTLE_CAM_PARAM`).
    Settle(SettleCamParam),
    /// Mode 2 (`FixedAngle`) parameters (`_VC_FIX_ANG_CAM_PARAM`).
    FixedAngle(FixAngCamParam),
    /// Mode 4 (`LocusCircle`) parameters (`_VC_LOCUS_CIRCLE_CAM_PARAM`).
    LocusCircle(LocusCircleCamParam),
    /// Mode 5 (`ThroughDoor`) parameters (`_VC_THROUGH_DOOR_CAM_PARAM` rail payload).
    ThroughDoor(ThroughDoorCamParam),
    /// Uninterpreted 16-byte payload (e.g. Mode 3 `SelfView`).
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
