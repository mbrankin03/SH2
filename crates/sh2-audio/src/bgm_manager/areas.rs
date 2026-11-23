//! `_SOUND_DATA` spatial volume bounding box evaluation (`sndGetSoundAryByCharaPos`).

use glam::Vec3;

use crate::bgm_data::{SoundDataRecord, STAGE_SOUND_DATA};

use super::types::{
    SoundAreaEvaluation, BGM_MAX_TRACKS, FADE_PRIO, MAX_MATCHED_SOUND_AREAS,
    SITUATION_PRESETS_6_TO_15, SOUND_AREA_HALF_WIDTH,
};

/// Extracts `(fade_in, fade_out)` commands for each of the 7 AIX tracks from a single `_SOUND_DATA` record
/// at the given `situation_page` (`0..=5`).
///
/// Matches the inner loop of `sndGetSoundAryByCharaPos` (`0x518945..0x518A58` in `sh2pc.exe`):
/// - If `(record.flags & (1 << situation_page)) != 0`, shifts `chanstat[track]` right by `situation_page * 5`.
/// - Otherwise uses the base calm page (`situation_page = 0`, low 5 bits of `chanstat[track]`).
/// - Within the 5-bit field:
///   - Bit `0` (`& 1`): track active (`0` -> `(0, 0)`, `1` -> active).
///   - Bits `1..=2` (`(bits >> 1) & 3`): `fade_in = val + 1` (`1..=4`).
///   - Bits `3..=4` (`(bits >> 3) & 3`): `fade_out = val + 1` (`1..=4`).
#[inline]
pub fn extract_record_track_fades(
    record: &SoundDataRecord,
    situation_page: u32,
) -> [(u8, u8); BGM_MAX_TRACKS] {
    let shift = if situation_page < 31 && (record.flags & (1i32 << situation_page)) != 0 {
        situation_page * 5
    } else {
        0
    };

    let mut out = [(0u8, 0u8); BGM_MAX_TRACKS];
    for (track_idx, &word) in record.chanstat.iter().enumerate() {
        let bits = word >> shift;
        if (bits & 1) != 0 {
            let fade_in = (((bits >> 1) & 0x3) as u8) + 1;
            let fade_out = (((bits >> 3) & 0x3) as u8) + 1;
            out[track_idx] = (fade_in, fade_out);
        }
    }
    out
}

/// Applies SH2:EE `Patches/MusicBoxVolume.cpp` 7th-layer (`Track 6`) Cinderella music box attenuation
/// overrides to a single `_SOUND_DATA` record when `enable_restored_cut_audio` is `true` (`SH2-62`):
/// - **Stage 40 (`STAGE_40_SOUND_DATA`, Lakeview Hotel 1F)**:
///   - Record `0` (default fallback / employee hallway): `chanstat[4] = 0x0000_0000` (no loud music box outside known areas).
///   - Records `6..=11` (1F West hallway): `chanstat[4] = 0x0000_0000`, `chanstat[6] = 0x0010_0000`
///     (quieter music box on Track 6 in the 1F West hallway when situation `4` is active).
///   - Records `12..=18` and `21..=27` (closed rooms other than lobby `19` and reception `20`):
///     `chanstat[4] = 0x0000_0000` (no music box inside closed rooms).
/// - **Stage 41 (`STAGE_41_SOUND_DATA`, Lakeview Hotel 2F)**:
///   - Records `3..=9` (2F hallways connected to the lobby) and Record `12` (2F cloak room):
///     `flags = 0x4000_0010` (`1_073_741_840`, bit 4 set for situation `4`) and `chanstat[6] = 0x0010_0000`
///     (quieter music box on Track 6 in 2F lobby-connected hallways after the music box puzzle is solved).
#[inline]
pub fn apply_music_box_volume_patch(
    stage_id: u32,
    rec_idx: usize,
    record: &SoundDataRecord,
    enable_restored_cut_audio: bool,
) -> SoundDataRecord {
    let mut rec = *record;
    if !enable_restored_cut_audio {
        return rec;
    }
    match stage_id {
        40 => {
            if rec_idx == 0 {
                rec.chanstat[4] = 0x0000_0000;
            } else if (6..=11).contains(&rec_idx) {
                rec.chanstat[4] = 0x0000_0000;
                rec.chanstat[6] = 0x0010_0000;
            } else if (12..=27).contains(&rec_idx) && rec_idx != 19 && rec_idx != 20 {
                rec.chanstat[4] = 0x0000_0000;
            }
        }
        41 if (3..=9).contains(&rec_idx) || rec_idx == 12 => {
            rec.flags = 0x4000_0010;
            rec.chanstat[6] = 0x0010_0000;
        }
        _ => {}
    }
    rec
}

/// Evaluates `_SOUND_DATA` spatial volume bounding boxes for `stage_id` at `player_pos` and `situation_page`.
///
/// Calls [`evaluate_sound_areas_with_restoration`] with `enable_restored_cut_audio = false` for 1:1 vanilla PC/PS2 parity.
pub fn evaluate_sound_areas(
    stage_id: u32,
    player_pos: Vec3,
    situation_page: u32,
) -> SoundAreaEvaluation {
    evaluate_sound_areas_with_restoration(stage_id, player_pos, situation_page, false)
}

/// Evaluates `_SOUND_DATA` spatial volume bounding boxes for `stage_id` at `player_pos` and `situation_page`,
/// optionally applying SH2:EE `Patches/MusicBoxVolume.cpp` 7th-layer (`Track 6`) Hotel 1F/2F music box
/// spatial attenuation when `enable_restored_cut_audio` is `true` (`SH2-62`).
///
/// Faithfully implements `sndGetSoundAryByCharaPos` (`0x5187E0` in `sh2pc.exe`, `0x221540` in `SLUS_202.28`)
/// and the priority reduction loop in `SeBgmManager` (`0x516372..0x5163FC`).
pub fn evaluate_sound_areas_with_restoration(
    stage_id: u32,
    player_pos: Vec3,
    situation_page: u32,
    enable_restored_cut_audio: bool,
) -> SoundAreaEvaluation {
    if (6..=15).contains(&situation_page) {
        let preset_idx = (situation_page - 6) as usize;
        let (ins, outs) = SITUATION_PRESETS_6_TO_15[preset_idx];
        let mut track_fades = [(0u8, 0u8); BGM_MAX_TRACKS];
        for i in 0..BGM_MAX_TRACKS {
            track_fades[i] = (ins[i], outs[i]);
        }
        return SoundAreaEvaluation {
            matched_box_count: 1,
            matched_record_indices: Vec::new(),
            used_default_record: false,
            track_fades,
        };
    }

    let records: &[SoundDataRecord] = STAGE_SOUND_DATA
        .get(stage_id as usize)
        .copied()
        .unwrap_or(&[]);

    if records.is_empty() {
        return SoundAreaEvaluation {
            matched_box_count: 0,
            matched_record_indices: Vec::new(),
            used_default_record: true,
            track_fades: [(0, 0); BGM_MAX_TRACKS],
        };
    }

    let px = player_pos.x;
    let py = player_pos.y;
    let pz = player_pos.z;

    let mut matched_indices = Vec::new();
    for (idx, rec) in records.iter().enumerate().skip(1) {
        if rec.flags < 0 {
            break;
        }
        let x_ok = (px + SOUND_AREA_HALF_WIDTH) >= rec.x0 && (px - SOUND_AREA_HALF_WIDTH) <= rec.x2;
        let z_ok = (pz + SOUND_AREA_HALF_WIDTH) >= rec.z2 && (pz - SOUND_AREA_HALF_WIDTH) <= rec.z0;
        let y_low = rec.max_y.min(rec.min_y);
        let y_high = rec.max_y.max(rec.min_y);
        let y_ok = py >= y_low && py <= y_high;

        if x_ok && z_ok && y_ok {
            matched_indices.push(idx);
            if matched_indices.len() >= MAX_MATCHED_SOUND_AREAS {
                break;
            }
        }
    }

    let used_default_record = matched_indices.is_empty();
    let eval_indices: &[usize] = if used_default_record {
        &[0]
    } else {
        &matched_indices
    };

    let mut resolved = [(0u8, 0u8); BGM_MAX_TRACKS];
    for &rec_idx in eval_indices {
        let patched_rec = apply_music_box_volume_patch(
            stage_id,
            rec_idx,
            &records[rec_idx],
            enable_restored_cut_audio,
        );
        let rec_fades = extract_record_track_fades(&patched_rec, situation_page);
        for track_idx in 0..BGM_MAX_TRACKS {
            let (cand_in, cand_out) = rec_fades[track_idx];
            let (cur_in, cur_out) = resolved[track_idx];

            let cur_in_prio = FADE_PRIO.get(cur_in as usize).copied().unwrap_or(0);
            let cand_in_prio = FADE_PRIO.get(cand_in as usize).copied().unwrap_or(0);
            let best_in = if cand_in_prio > cur_in_prio {
                cand_in
            } else {
                cur_in
            };

            let cur_out_prio = FADE_PRIO.get(cur_out as usize).copied().unwrap_or(0);
            let cand_out_prio = FADE_PRIO.get(cand_out as usize).copied().unwrap_or(0);
            let best_out = if cand_out_prio > cur_out_prio {
                cand_out
            } else {
                cur_out
            };

            resolved[track_idx] = (best_in, best_out);
        }
    }

    SoundAreaEvaluation {
        matched_box_count: eval_indices.len(),
        matched_record_indices: matched_indices,
        used_default_record,
        track_fades: resolved,
    }
}
