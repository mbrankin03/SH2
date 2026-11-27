//! Direct reader and Central Directory indexer for `ZIP_STORED` and `DEFLATE` (32-bit and ZIP64)
//! 4K AI texture pack and community mod `.zip` archives.

use std::collections::HashMap;
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use super::{decode_texture_bytes, DecodedTexture};

/// Parsed Central Directory index for a `.zip` archive supporting O(1) case-insensitive
/// and single-wrapper-folder-stripped lookups (`SH2-179`).
#[derive(Debug, Clone, Default)]
pub(crate) struct ZipEntryIndex {
    /// Lowercased, `/`-normalized exact archive path -> `(local_hdr_ofs, comp_size)`.
    pub(crate) entries_lower: HashMap<String, (u64, u64)>,
    /// Lowercased, `/`-normalized path with a single top-level wrapper folder stripped
    /// (e.g. `borat mod/sh2e/menu/mc/savebg.tbn2` -> `sh2e/menu/mc/savebg.tbn2`) -> `(local_hdr_ofs, comp_size)`.
    pub(crate) wrapper_stripped_lower: HashMap<String, (u64, u64)>,
    /// Original `/`-normalized entry names (preserving case) for mod override discovery.
    pub(crate) original_names: Vec<String>,
}

pub(crate) type ZipIndexCache = Arc<Mutex<HashMap<PathBuf, ZipEntryIndex>>>;

const MAX_ZIP_ENTRY_BYTES: u64 = 256 * 1024 * 1024;
const MAX_ZIP_TEXTURE_BYTES: u64 = 128 * 1024 * 1024;

/// Reads and (if `DEFLATE`-compressed) decompresses a single ZIP entry at `local_hdr_ofs`,
/// enforcing `max_bytes` on both compressed and decompressed payload sizes to prevent zip bombs.
fn read_zip_entry_payload(
    zip_path: &Path,
    local_hdr_ofs: u64,
    comp_size: u64,
    max_bytes: u64,
) -> Option<Vec<u8>> {
    let mut file = fs::File::open(zip_path).ok()?;
    let file_len = file.metadata().ok()?.len();

    let mut lhdr = [0u8; 30];
    file.seek(SeekFrom::Start(local_hdr_ofs)).ok()?;
    file.read_exact(&mut lhdr).ok()?;
    if &lhdr[0..4] != b"PK\x03\x04" {
        return None;
    }
    let compression = u16::from_le_bytes(lhdr[8..10].try_into().ok()?);
    let lname_len = u16::from_le_bytes(lhdr[26..28].try_into().ok()?) as u64;
    let lextra_len = u16::from_le_bytes(lhdr[28..30].try_into().ok()?) as u64;
    let data_start = local_hdr_ofs
        .checked_add(30)?
        .checked_add(lname_len)?
        .checked_add(lextra_len)?;
    let data_end = data_start.checked_add(comp_size)?;
    if data_end > file_len || comp_size > max_bytes {
        return None;
    }
    file.seek(SeekFrom::Start(data_start)).ok()?;
    let mut payload = vec![0u8; comp_size as usize];
    file.read_exact(&mut payload).ok()?;
    match compression {
        0 => Some(payload),
        8 => {
            let decoder = flate2::read::DeflateDecoder::new(&payload[..]);
            let mut out = Vec::new();
            decoder.take(max_bytes + 1).read_to_end(&mut out).ok()?;
            if (out.len() as u64) > max_bytes {
                return None;
            }
            Some(out)
        }
        _ => None,
    }
}

/// Reads raw uncompressed bytes for an exact or case-insensitive `entry_path` using an already
/// built [`ZipEntryIndex`] without re-reading the Central Directory (`SH2-179`).
pub(crate) fn read_indexed_entry_bytes(
    zip_path: &Path,
    index: &ZipEntryIndex,
    entry_path: &str,
) -> Option<Vec<u8>> {
    let key = entry_path.replace('\\', "/").to_ascii_lowercase();
    let (local_hdr_ofs, comp_size) = index.entries_lower.get(&key).copied()?;
    read_zip_entry_payload(zip_path, local_hdr_ofs, comp_size, MAX_ZIP_ENTRY_BYTES)
}

/// Reads raw uncompressed bytes for an exact or case-insensitive `entry_path` from a `ZIP_STORED`
/// or `DEFLATE` archive in O(1), caching the parsed Central Directory index in `zip_index_cache` (`SH2-179`).
#[cfg(test)]
pub(crate) fn read_entry_bytes_from_stored_zip(
    zip_index_cache: &ZipIndexCache,
    zip_path: &Path,
    entry_path: &str,
) -> Option<Vec<u8>> {
    let key = entry_path.replace('\\', "/").to_ascii_lowercase();
    let cached_entry = {
        let mut guard = zip_index_cache.lock().ok()?;
        if !guard.contains_key(zip_path) {
            if let Some(index) = build_stored_zip_central_directory_index(zip_path) {
                guard.insert(zip_path.to_path_buf(), index);
            }
        }
        let index = guard.get(zip_path)?;
        index.entries_lower.get(&key).copied()
    };

    let (local_hdr_ofs, comp_size) = cached_entry?;
    read_zip_entry_payload(zip_path, local_hdr_ofs, comp_size, MAX_ZIP_ENTRY_BYTES)
}

/// Reads a mod override entry from a `.zip` archive in O(1) under a single cache lock, checking
/// direct `sh2e/<rel>`, `data/<rel>`, `<rel>` first and then single-wrapper-folder-stripped
/// equivalents (e.g. `<Mod Name>/sh2e/<rel>`) without scanning all entries (`SH2-179`).
pub(crate) fn read_mod_entry_bytes_from_stored_zip(
    zip_index_cache: &ZipIndexCache,
    zip_path: &Path,
    rel_str: &str,
) -> Option<Vec<u8>> {
    let rel_lower = rel_str.replace('\\', "/").to_ascii_lowercase();
    let sh2e_cand = format!("sh2e/{rel_lower}");
    let data_cand = format!("data/{rel_lower}");
    let cached_entry = {
        let mut guard = zip_index_cache.lock().ok()?;
        if !guard.contains_key(zip_path) {
            if let Some(index) = build_stored_zip_central_directory_index(zip_path) {
                guard.insert(zip_path.to_path_buf(), index);
            }
        }
        let index = guard.get(zip_path)?;
        index
            .entries_lower
            .get(&sh2e_cand)
            .or_else(|| index.entries_lower.get(&data_cand))
            .or_else(|| index.entries_lower.get(&rel_lower))
            .or_else(|| index.wrapper_stripped_lower.get(&sh2e_cand))
            .or_else(|| index.wrapper_stripped_lower.get(&data_cand))
            .or_else(|| index.wrapper_stripped_lower.get(&rel_lower))
            .copied()
    };

    let (local_hdr_ofs, comp_size) = cached_entry?;
    read_zip_entry_payload(zip_path, local_hdr_ofs, comp_size, MAX_ZIP_ENTRY_BYTES)
}

/// Reads a `<sub_dir>/<tex_id>.{dds,png}` entry directly from a `.zip` texture pack archive
/// (`ZIP_STORED` or `DEFLATE`, 32-bit or ZIP64), caching the Central Directory index.
pub(super) fn read_texture_from_stored_zip(
    zip_index_cache: &ZipIndexCache,
    zip_path: &Path,
    sub: &str,
    tex_id: u32,
) -> Option<DecodedTexture> {
    read_relative_texture_from_stored_zip(zip_index_cache, zip_path, &format!("{sub}/{tex_id}"))
}

/// Reads a `<rel_no_ext>.{dds,png}` entry directly from a `.zip` texture pack archive
/// (`ZIP_STORED` or `DEFLATE`, 32-bit or ZIP64), caching the Central Directory index.
pub(super) fn read_relative_texture_from_stored_zip(
    zip_index_cache: &ZipIndexCache,
    zip_path: &Path,
    rel_no_ext: &str,
) -> Option<DecodedTexture> {
    let lower = rel_no_ext.replace('\\', "/").to_ascii_lowercase();
    let dds_key = format!("{lower}.dds");
    let png_key = format!("{lower}.png");
    let cached_entry = {
        let mut guard = zip_index_cache.lock().ok()?;
        if !guard.contains_key(zip_path) {
            if let Some(index) = build_stored_zip_central_directory_index(zip_path) {
                guard.insert(zip_path.to_path_buf(), index);
            }
        }
        let index = guard.get(zip_path)?;
        index
            .entries_lower
            .get(&dds_key)
            .copied()
            .map(|v| (dds_key, v))
            .or_else(|| {
                index
                    .entries_lower
                    .get(&png_key)
                    .copied()
                    .map(|v| (png_key, v))
            })
    };

    let (entry_name, (local_hdr_ofs, comp_size)) = cached_entry?;
    let payload =
        read_zip_entry_payload(zip_path, local_hdr_ofs, comp_size, MAX_ZIP_TEXTURE_BYTES)?;
    decode_texture_bytes(&payload, Path::new(&entry_name)).ok()
}

/// Parses the Central Directory of a `ZIP_STORED` (32-bit or ZIP64) archive once and returns
/// a lookup map `entry_name -> (local_header_offset, compressed_size)`.
pub(crate) fn build_stored_zip_central_directory_index(zip_path: &Path) -> Option<ZipEntryIndex> {
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

    let mut entries_lower = HashMap::new();
    let mut wrapper_stripped_lower = HashMap::new();
    let mut original_names = Vec::new();
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

        if compression == 0 || compression == 8 {
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
                let normalized = entry_name.replace('\\', "/");
                let lower = normalized.to_ascii_lowercase();
                if let Some((_, rest)) = lower.split_once('/') {
                    if !rest.is_empty() {
                        wrapper_stripped_lower
                            .entry(rest.to_string())
                            .or_insert((local_hdr_ofs, comp_size));
                    }
                }
                entries_lower.insert(lower, (local_hdr_ofs, comp_size));
                original_names.push(normalized);
            }
        }
        pos = next_pos;
    }

    Some(ZipEntryIndex {
        entries_lower,
        wrapper_stripped_lower,
        original_names,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_deflate_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
        use flate2::{write::DeflateEncoder, Compression};
        use std::io::Write;

        let mut out = Vec::new();
        let mut central = Vec::new();
        for &(name, raw) in entries {
            let mut enc = DeflateEncoder::new(Vec::new(), Compression::fast());
            enc.write_all(raw).unwrap();
            let comp = enc.finish().unwrap();

            let offset = out.len() as u32;
            let name_bytes = name.as_bytes();
            let comp_sz = comp.len() as u32;
            let uncomp_sz = raw.len() as u32;
            out.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
            out.extend_from_slice(&20u16.to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes());
            out.extend_from_slice(&8u16.to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes());
            out.extend_from_slice(&0u32.to_le_bytes());
            out.extend_from_slice(&comp_sz.to_le_bytes());
            out.extend_from_slice(&uncomp_sz.to_le_bytes());
            out.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes());
            out.extend_from_slice(name_bytes);
            out.extend_from_slice(&comp);

            central.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
            central.extend_from_slice(&20u16.to_le_bytes());
            central.extend_from_slice(&20u16.to_le_bytes());
            central.extend_from_slice(&0u16.to_le_bytes());
            central.extend_from_slice(&8u16.to_le_bytes());
            central.extend_from_slice(&0u16.to_le_bytes());
            central.extend_from_slice(&0u16.to_le_bytes());
            central.extend_from_slice(&0u32.to_le_bytes());
            central.extend_from_slice(&comp_sz.to_le_bytes());
            central.extend_from_slice(&uncomp_sz.to_le_bytes());
            central.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
            central.extend_from_slice(&0u16.to_le_bytes());
            central.extend_from_slice(&0u16.to_le_bytes());
            central.extend_from_slice(&0u16.to_le_bytes());
            central.extend_from_slice(&0u16.to_le_bytes());
            central.extend_from_slice(&0u32.to_le_bytes());
            central.extend_from_slice(&offset.to_le_bytes());
            central.extend_from_slice(name_bytes);
        }
        let cd_offset = out.len() as u32;
        let cd_size = central.len() as u32;
        let count = entries.len() as u16;
        out.extend_from_slice(&central);
        out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&count.to_le_bytes());
        out.extend_from_slice(&count.to_le_bytes());
        out.extend_from_slice(&cd_size.to_le_bytes());
        out.extend_from_slice(&cd_offset.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out
    }

    #[test]
    fn read_relative_texture_supports_deflate_png() {
        let mut png_bytes = Vec::new();
        let img = image::RgbaImage::from_pixel(2, 2, image::Rgba([255, 0, 128, 255]));
        image::DynamicImage::ImageRgba8(img)
            .write_to(
                &mut std::io::Cursor::new(&mut png_bytes),
                image::ImageFormat::Png,
            )
            .unwrap();

        let zip_bytes = build_deflate_zip(&[("pic/title.png", &png_bytes)]);
        let tmp = std::env::temp_dir().join(format!(
            "sh2_deflate_tex_test_{}_{:?}.zip",
            std::process::id(),
            std::thread::current().id()
        ));
        fs::write(&tmp, &zip_bytes).unwrap();

        let cache: ZipIndexCache = Arc::new(Mutex::new(HashMap::new()));
        let decoded = read_relative_texture_from_stored_zip(&cache, &tmp, "pic/title");
        let _ = fs::remove_file(&tmp);

        let tex =
            decoded.expect("DEFLATE PNG should decode via read_relative_texture_from_stored_zip");
        assert_eq!((tex.width, tex.height), (2, 2));
    }

    #[test]
    fn read_entry_bytes_rejects_oversized_deflate_bomb() {
        // 257 MiB of zeros compresses down to ~260 KB, well under comp_size cap,
        // so only the decompressed `.take(MAX_ZIP_ENTRY_BYTES + 1)` guard catches it.
        let bomb_raw = vec![0u8; 257 * 1024 * 1024];
        let zip_bytes = build_deflate_zip(&[("bomb.bin", &bomb_raw)]);
        drop(bomb_raw);

        let tmp = std::env::temp_dir().join(format!(
            "sh2_deflate_bomb_test_{}_{:?}.zip",
            std::process::id(),
            std::thread::current().id()
        ));
        fs::write(&tmp, &zip_bytes).unwrap();

        let cache: ZipIndexCache = Arc::new(Mutex::new(HashMap::new()));
        let res = read_entry_bytes_from_stored_zip(&cache, &tmp, "bomb.bin");
        let _ = fs::remove_file(&tmp);

        assert!(res.is_none(), "Oversized DEFLATE payload must be rejected");
    }

    #[test]
    fn read_mod_entry_bytes_o1_case_insensitive_and_wrapper_stripped_parity() {
        let zip_bytes = build_deflate_zip(&[
            ("MyMod/SH2E/pic/Title.PNG", b"WRAP_SH2E"),
            ("MyMod/Data/pic/Title.PNG", b"WRAP_DATA"),
            ("MyMod/pic/OnlyRoot.dds", b"WRAP_ROOT"),
            ("a/b/pic/TooDeep.dds", b"TOO_DEEP"),
        ]);
        let tmp = std::env::temp_dir().join(format!(
            "sh2_o1_mod_zip_test_{}_{:?}.zip",
            std::process::id(),
            std::thread::current().id()
        ));
        fs::write(&tmp, &zip_bytes).unwrap();

        let cache: ZipIndexCache = Arc::new(Mutex::new(HashMap::new()));
        // Case-insensitive + wrapper-stripped `sh2e/` beats `data/`
        assert_eq!(
            read_mod_entry_bytes_from_stored_zip(&cache, &tmp, "PIC/title.png").as_deref(),
            Some(b"WRAP_SH2E".as_slice())
        );
        // Single-wrapper bare root match
        assert_eq!(
            read_mod_entry_bytes_from_stored_zip(&cache, &tmp, "pic/onlyroot.DDS").as_deref(),
            Some(b"WRAP_ROOT".as_slice())
        );
        // Multi-level wrapper (`a/b/...`) must NOT match single-wrapper stripping
        assert!(read_mod_entry_bytes_from_stored_zip(&cache, &tmp, "pic/toodeep.dds").is_none());
        // Missing key returns None in O(1)
        assert!(read_mod_entry_bytes_from_stored_zip(&cache, &tmp, "pic/missing.dds").is_none());

        let _ = fs::remove_file(&tmp);
    }
}
