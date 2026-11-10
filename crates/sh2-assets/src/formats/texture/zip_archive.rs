//! Direct zero-copy reader and Central Directory indexer for `ZIP_STORED` (32-bit and ZIP64)
//! 4K AI texture pack archives (`sh2_4k_textures.zip`, `sh2_4k_normals.zip`).

use std::collections::HashMap;
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use super::{decode_texture_bytes, DecodedTexture};

pub(super) type ZipEntryIndex = HashMap<String, (u64, u64)>;
pub(super) type ZipIndexCache = Arc<Mutex<HashMap<PathBuf, ZipEntryIndex>>>;

/// Reads a `<sub_dir>/<tex_id>.{dds,png}` entry directly from a zero-compression (`ZIP_STORED`,
/// supporting standard 32-bit and ZIP64 local/central headers) `.zip` texture pack archive,
/// caching the parsed Central Directory index in `zip_index_cache`.
pub(super) fn read_texture_from_stored_zip(
    zip_index_cache: &ZipIndexCache,
    zip_path: &Path,
    sub: &str,
    tex_id: u32,
) -> Option<DecodedTexture> {
    let cached_entry = {
        let mut guard = zip_index_cache.lock().ok()?;
        if !guard.contains_key(zip_path) {
            if let Some(index) = build_stored_zip_central_directory_index(zip_path) {
                guard.insert(zip_path.to_path_buf(), index);
            }
        }
        let index = guard.get(zip_path)?;
        let dds_key = format!("{sub}/{tex_id}.dds");
        let png_key = format!("{sub}/{tex_id}.png");
        index
            .get(&dds_key)
            .copied()
            .map(|v| (dds_key, v))
            .or_else(|| index.get(&png_key).copied().map(|v| (png_key, v)))
    };

    let (entry_name, (local_hdr_ofs, comp_size)) = cached_entry?;
    let mut file = fs::File::open(zip_path).ok()?;
    let file_len = file.metadata().ok()?.len();

    let mut lhdr = [0u8; 30];
    file.seek(SeekFrom::Start(local_hdr_ofs)).ok()?;
    file.read_exact(&mut lhdr).ok()?;
    if &lhdr[0..4] != b"PK\x03\x04" {
        return None;
    }
    let lname_len = u16::from_le_bytes(lhdr[26..28].try_into().ok()?) as u64;
    let lextra_len = u16::from_le_bytes(lhdr[28..30].try_into().ok()?) as u64;
    let data_start = local_hdr_ofs
        .checked_add(30)?
        .checked_add(lname_len)?
        .checked_add(lextra_len)?;
    let data_end = data_start.checked_add(comp_size)?;
    if data_end > file_len || comp_size > 128 * 1024 * 1024 {
        return None;
    }
    file.seek(SeekFrom::Start(data_start)).ok()?;
    let mut payload = vec![0u8; comp_size as usize];
    file.read_exact(&mut payload).ok()?;
    decode_texture_bytes(&payload, Path::new(&entry_name)).ok()
}

/// Parses the Central Directory of a `ZIP_STORED` (32-bit or ZIP64) archive once and returns
/// a lookup map `entry_name -> (local_header_offset, compressed_size)`.
pub(super) fn build_stored_zip_central_directory_index(zip_path: &Path) -> Option<ZipEntryIndex> {
    let mut file = fs::File::open(zip_path).ok()?;
    let file_len = file.metadata().ok()?.len();
    if file_len < 22 {
        return None;
    }

    // Read the last 65 KB to locate the End of Central Directory (EOCD / ZIP64 EOCD Locator)
    let tail_len = file_len.min(65557) as usize;
    file.seek(SeekFrom::End(-(tail_len as i64))).ok()?;
    let mut tail = vec![0u8; tail_len];
    file.read_exact(&mut tail).ok()?;

    let mut eocd_pos = None;
    for i in (0..=tail_len.saturating_sub(22)).rev() {
        if &tail[i..i + 4] == b"PK\x05\x06" {
            eocd_pos = Some(i);
            break;
        }
    }
    let eocd_idx = eocd_pos?;

    let mut cd_size =
        u32::from_le_bytes(tail[eocd_idx + 12..eocd_idx + 16].try_into().ok()?) as u64;
    let mut cd_offset =
        u32::from_le_bytes(tail[eocd_idx + 16..eocd_idx + 20].try_into().ok()?) as u64;

    // Check for ZIP64 End of Central Directory Locator (`PK\x06\x07`) 20 bytes before EOCD
    if eocd_idx >= 20 && &tail[eocd_idx - 20..eocd_idx - 16] == b"PK\x06\x07" {
        let zip64_eocd_ofs = u64::from_le_bytes(tail[eocd_idx - 12..eocd_idx - 4].try_into().ok()?);
        let mut z64_hdr = [0u8; 56];
        if file.seek(SeekFrom::Start(zip64_eocd_ofs)).is_ok()
            && file.read_exact(&mut z64_hdr).is_ok()
            && &z64_hdr[0..4] == b"PK\x06\x06"
        {
            cd_size = u64::from_le_bytes(z64_hdr[40..48].try_into().ok()?);
            cd_offset = u64::from_le_bytes(z64_hdr[48..56].try_into().ok()?);
        }
    }

    if cd_offset.checked_add(cd_size)? > file_len || cd_size > 32 * 1024 * 1024 {
        return None;
    }

    file.seek(SeekFrom::Start(cd_offset)).ok()?;
    let mut cd_buf = vec![0u8; cd_size as usize];
    file.read_exact(&mut cd_buf).ok()?;

    let mut index = HashMap::new();
    let mut pos = 0usize;
    while pos + 46 <= cd_buf.len() && &cd_buf[pos..pos + 4] == b"PK\x01\x02" {
        let compression = u16::from_le_bytes(cd_buf[pos + 10..pos + 12].try_into().ok()?);
        let mut comp_size = u32::from_le_bytes(cd_buf[pos + 20..pos + 24].try_into().ok()?) as u64;
        let uncomp_size = u32::from_le_bytes(cd_buf[pos + 24..pos + 28].try_into().ok()?) as u64;
        let name_len = u16::from_le_bytes(cd_buf[pos + 28..pos + 30].try_into().ok()?) as usize;
        let extra_len = u16::from_le_bytes(cd_buf[pos + 30..pos + 32].try_into().ok()?) as usize;
        let comment_len = u16::from_le_bytes(cd_buf[pos + 32..pos + 34].try_into().ok()?) as usize;
        let mut local_hdr_ofs =
            u32::from_le_bytes(cd_buf[pos + 42..pos + 46].try_into().ok()?) as u64;

        let name_start = pos + 46;
        let extra_start = name_start + name_len;
        let next_pos = extra_start + extra_len + comment_len;
        if next_pos > cd_buf.len() {
            break;
        }

        if compression == 0 {
            if let Ok(entry_name) = std::str::from_utf8(&cd_buf[name_start..extra_start]) {
                // Parse ZIP64 extra field (`0x0001`) if any 32-bit field is `0xFFFF_FFFF`
                if uncomp_size == 0xFFFF_FFFF
                    || comp_size == 0xFFFF_FFFF
                    || local_hdr_ofs == 0xFFFF_FFFF
                {
                    let extra = &cd_buf[extra_start..extra_start + extra_len];
                    let mut ep = 0usize;
                    while ep + 4 <= extra.len() {
                        let header_id = u16::from_le_bytes(extra[ep..ep + 2].try_into().ok()?);
                        let data_sz =
                            u16::from_le_bytes(extra[ep + 2..ep + 4].try_into().ok()?) as usize;
                        ep += 4;
                        if header_id == 0x0001 && ep + data_sz <= extra.len() {
                            let mut zp = ep;
                            if uncomp_size == 0xFFFF_FFFF && zp + 8 <= ep + data_sz {
                                zp += 8;
                            }
                            if comp_size == 0xFFFF_FFFF && zp + 8 <= ep + data_sz {
                                comp_size = u64::from_le_bytes(extra[zp..zp + 8].try_into().ok()?);
                                zp += 8;
                            }
                            if local_hdr_ofs == 0xFFFF_FFFF && zp + 8 <= ep + data_sz {
                                local_hdr_ofs =
                                    u64::from_le_bytes(extra[zp..zp + 8].try_into().ok()?);
                            }
                            break;
                        }
                        ep += data_sz;
                    }
                }
                index.insert(entry_name.to_string(), (local_hdr_ofs, comp_size));
            }
        }
        pos = next_pos;
    }

    Some(index)
}
