//! 4K AI VSR FMV Pack `manifest.json` parser and pure-Rust SHA-256 verifier (`SH2-90`).

use std::fs::File;
use std::io::Read;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::AssetError;

/// Metadata and SHA-256 checksum entry for a single 4K VSR FMV master (`*_4k_master.mp4`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FmvManifestEntry {
    /// Catalog identifier matching [`FMV_CATALOG`] (e.g., `"open"`, `"toilet"`, `"murder"`).
    pub id: String,
    /// Master filename on disk (e.g., `"open_4k_master.mp4"`).
    pub filename: String,
    /// Lowercase hexadecimal SHA-256 digest of the `.mp4` file.
    pub sha256: String,
    /// Exact byte size of the `.mp4` file.
    pub size_bytes: u64,
    /// Coded video width (typically `3840`).
    pub width: u32,
    /// Coded video height (typically `2160`).
    pub height: u32,
    /// Nominal frame rate (e.g., `29.97`).
    #[serde(default)]
    pub fps: f32,
    /// Total video frame count.
    #[serde(default)]
    pub frame_count: u64,
    /// Audio track descriptions (e.g., `["Track 0: Stereo FLAC", "Track 1: Voiced / VHS Audio"]`).
    #[serde(default)]
    pub audio_tracks: Vec<String>,
}

/// Top-level `manifest.json` distributed with `SH2-4K-VSR-FMV-Pack-v1.0`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FmvPackManifest {
    /// Pack release version (e.g., `"1.0.0"`).
    pub version: String,
    /// All 21 catalogued 4K VSR FMV entries.
    pub entries: Vec<FmvManifestEntry>,
}

/// Verification result for a single FMV entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FmvEntryVerifyStatus {
    Verified,
    Missing,
    SizeMismatch { expected: u64, actual: u64 },
    HashMismatch { expected: String, actual: String },
}

/// Aggregated verification report produced by [`verify_fmv_pack`].
#[derive(Debug, Clone)]
pub struct FmvPackVerificationReport {
    pub version: String,
    pub total: usize,
    pub verified: usize,
    pub results: Vec<(String, FmvEntryVerifyStatus)>,
}

impl FmvPackVerificationReport {
    /// Returns `true` when every entry in `manifest.json` exists and matches its SHA-256 digest.
    #[must_use]
    pub fn all_verified(&self) -> bool {
        self.total > 0 && self.verified == self.total
    }
}

/// Loads `manifest.json` from `dir` and verifies every entry's file size and SHA-256 checksum.
pub fn verify_fmv_pack(dir: &Path) -> Result<FmvPackVerificationReport, AssetError> {
    let manifest_path = dir.join("manifest.json");
    let raw = std::fs::read_to_string(&manifest_path)?;
    let manifest: FmvPackManifest = serde_json::from_str(&raw).map_err(|e| {
        AssetError::InvalidData(format!("Failed to parse {}: {e}", manifest_path.display()))
    })?;

    let mut verified = 0usize;
    let mut results = Vec::with_capacity(manifest.entries.len());

    for entry in &manifest.entries {
        let file_path = dir.join(&entry.filename);
        if !file_path.is_file() {
            results.push((entry.filename.clone(), FmvEntryVerifyStatus::Missing));
            continue;
        }
        let meta = std::fs::metadata(&file_path)?;
        if meta.len() != entry.size_bytes {
            results.push((
                entry.filename.clone(),
                FmvEntryVerifyStatus::SizeMismatch {
                    expected: entry.size_bytes,
                    actual: meta.len(),
                },
            ));
            continue;
        }
        let actual_hash = sha256_file(&file_path)?;
        if actual_hash.eq_ignore_ascii_case(&entry.sha256) {
            verified += 1;
            results.push((entry.filename.clone(), FmvEntryVerifyStatus::Verified));
        } else {
            results.push((
                entry.filename.clone(),
                FmvEntryVerifyStatus::HashMismatch {
                    expected: entry.sha256.clone(),
                    actual: actual_hash,
                },
            ));
        }
    }

    Ok(FmvPackVerificationReport {
        version: manifest.version,
        total: manifest.entries.len(),
        verified,
        results,
    })
}

/// Computes the lowercase hex SHA-256 digest of a file on disk using streaming 64 KiB reads.
pub fn sha256_file(path: &Path) -> Result<String, AssetError> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 65536];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher.finalize_hex())
}

/// Computes the lowercase hex SHA-256 digest of an in-memory byte slice.
#[must_use]
pub fn sha256_bytes(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize_hex()
}

/// Minimal FIPS 180-4 SHA-256 implementation (`#![forbid(unsafe_code)]`, zero external dependencies).
struct Sha256 {
    state: [u32; 8],
    bit_len: u64,
    buf: [u8; 64],
    buf_len: usize,
}

impl Sha256 {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];

    fn new() -> Self {
        Self {
            state: [
                0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
                0x5be0cd19,
            ],
            bit_len: 0,
            buf: [0u8; 64],
            buf_len: 0,
        }
    }

    fn update(&mut self, mut input: &[u8]) {
        self.bit_len = self.bit_len.wrapping_add((input.len() as u64) * 8);
        if self.buf_len > 0 {
            let take = (64 - self.buf_len).min(input.len());
            self.buf[self.buf_len..self.buf_len + take].copy_from_slice(&input[..take]);
            self.buf_len += take;
            input = &input[take..];
            if self.buf_len == 64 {
                let block = self.buf;
                self.compress(&block);
                self.buf_len = 0;
            }
        }
        while input.len() >= 64 {
            let mut block = [0u8; 64];
            block.copy_from_slice(&input[..64]);
            self.compress(&block);
            input = &input[64..];
        }
        if !input.is_empty() {
            self.buf[..input.len()].copy_from_slice(input);
            self.buf_len = input.len();
        }
    }

    fn finalize_hex(mut self) -> String {
        self.buf[self.buf_len] = 0x80;
        self.buf_len += 1;
        if self.buf_len > 56 {
            self.buf[self.buf_len..64].fill(0);
            let block = self.buf;
            self.compress(&block);
            self.buf_len = 0;
        }
        self.buf[self.buf_len..56].fill(0);
        self.buf[56..64].copy_from_slice(&self.bit_len.to_be_bytes());
        let block = self.buf;
        self.compress(&block);

        let mut out = String::with_capacity(64);
        for word in self.state {
            use std::fmt::Write;
            let _ = write!(out, "{word:08x}");
        }
        out
    }

    fn compress(&mut self, block: &[u8; 64]) {
        let mut w = [0u32; 64];
        for (i, &chunk) in block.as_chunks::<4>().0.iter().enumerate() {
            w[i] = u32::from_be_bytes(chunk);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = self.state;
        for (i, &w_i) in w.iter().enumerate() {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(Self::K[i])
                .wrapping_add(w_i);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }
        self.state[0] = self.state[0].wrapping_add(a);
        self.state[1] = self.state[1].wrapping_add(b);
        self.state[2] = self.state[2].wrapping_add(c);
        self.state[3] = self.state[3].wrapping_add(d);
        self.state[4] = self.state[4].wrapping_add(e);
        self.state[5] = self.state[5].wrapping_add(f);
        self.state[6] = self.state[6].wrapping_add(g);
        self.state[7] = self.state[7].wrapping_add(h);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_matches_nist_abc_vector() {
        assert_eq!(
            sha256_bytes(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn verify_fmv_pack_reports_verified_and_mismatch() {
        let tmp = std::env::temp_dir().join(format!(
            "sh2_90_manifest_test_{}_{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let payload = b"SH2_4K_VSR_MASTER_TEST";
        let hash = sha256_bytes(payload);
        std::fs::write(tmp.join("toilet_4k_master.mp4"), payload).unwrap();

        let manifest = FmvPackManifest {
            version: "1.0.0".into(),
            entries: vec![
                FmvManifestEntry {
                    id: "toilet".into(),
                    filename: "toilet_4k_master.mp4".into(),
                    sha256: hash,
                    size_bytes: payload.len() as u64,
                    width: 3840,
                    height: 2160,
                    fps: 29.97,
                    frame_count: 602,
                    audio_tracks: vec!["Track 0: Stereo FLAC".into()],
                },
                FmvManifestEntry {
                    id: "open".into(),
                    filename: "open_4k_master.mp4".into(),
                    sha256: "0".repeat(64),
                    size_bytes: 123,
                    width: 3840,
                    height: 2160,
                    fps: 29.97,
                    frame_count: 1000,
                    audio_tracks: vec![],
                },
            ],
        };
        std::fs::write(
            tmp.join("manifest.json"),
            serde_json::to_string_pretty(&manifest).unwrap(),
        )
        .unwrap();

        let report = verify_fmv_pack(&tmp).unwrap();
        assert_eq!(report.total, 2);
        assert_eq!(report.verified, 1);
        assert!(!report.all_verified());
        assert_eq!(report.results[0].1, FmvEntryVerifyStatus::Verified);
        assert_eq!(report.results[1].1, FmvEntryVerifyStatus::Missing);

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
