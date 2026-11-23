//! Binary `.cam` record decoding and authentic PS2 sentinel substitutions (`vcConvertCamFile`).

use crate::error::AssetError;

use super::area::RoadArea;
use super::types::{
    AreaSizeType, CamFile, CamMovementParams, CamMovementType, ChaseCamParam, FixAngCamParam,
    LocusCircleCamParam, RoadRecord, RoadType, SettleCamParam, ThroughDoorCamParam,
    VC_RD_END_DATA_F,
};

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
            CamMovementType::ThroughDoor => CamMovementParams::ThroughDoor(ThroughDoorCamParam {
                rail_ang_y: read_f32_le(rec, 0x70),
                rail_sta_pos: [
                    read_f32_le(rec, 0x74),
                    read_f32_le(rec, 0x78),
                    read_f32_le(rec, 0x7C),
                ],
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
