//! Safe binary parser and keyframe evaluator for Silent Hill 2 Facial Morph Cluster Animations (`.cls`).
//!
//! # Ground Truth & Format Architecture
//! Verified against:
//! - **Primary Ground Truth**: PS2 Debug Prototype ELF (`SLUS_202.28`, DWARF compile unit `Chacter_Draw/clani.c`:
//!   `ClaniDataHeaderCheck` at `0x1258c0`, `Calc1` at `0x125920`, `ClusterAnimeExec` at `0x125de0`).
//! - **Secondary Ground Truth**: Retail PC binary (`sh2pc.exe` NA v1.0: `0x506e50`, `0x506ea0`, `0x507120`)
//!   and all 113 retail `.cls` files (346 sub-clip blocks) across `data/demo/` and `data/demo2/`.
//!
//! ### `.cls` Sub-Clip Block Header (16 bytes + `4 * n_clusters` offset table)
//! Each `.cls` file on disk contains one or more 4-byte-aligned cluster animation blocks concatenated
//! back-to-back (addressed by `clani_adr_list` in `sh2pc.exe`):
//!
//! | Offset | Type | Field | Description |
//! |--------|------|-------|-------------|
//! | `0x00` | `u32` | `magic` | `0x29843918` or `0x29853918` (PS2 prototype also accepts `0x00020229`) |
//! | `0x04` | `u16` | `revision` | Format revision (`1` = `Calc1`) |
//! | `0x06` | `u16` | `flags` | Header flags (`0`) |
//! | `0x08` | `u16` | `n_clusters` | Number of facial morph target tracks (`12..=19`) |
//! | `0x0A` | `u16` | `pad_0a` | Alignment padding (`0`) |
//! | `0x0C` | `u16` | `n_frames` | Total 30 FPS frames in this sub-clip |
//! | `0x0E` | `u16` | `pad_0e` | Alignment padding (`0`) |
//! | `0x10` | `[u32; n_clusters]` | `cluster_offsets` | Byte offsets relative to block start for each `Cluster1` track |
//!
//! ### `Cluster1` Keyframe Track (`block_base + cluster_offsets[c]`)
//! | Offset | Type | Field | Description |
//! |--------|------|-------|-------------|
//! | `+0x00` | `u16` | `n_keys` | Number of keyframes in this cluster track |
//! | `+0x02` | `[ClsKeyframe; n_keys]` | `keys` | Packed 4-byte records `{ u16 frame, i16 weight_fp12 }` |

use super::binary::{read_i16, read_u16, read_u32};
use crate::error::AssetError;

/// Primary retail PC / PS2 `.cls` block magic identifier (`0x29843918`).
pub const CLS_MAGIC_A: u32 = 0x2984_3918;
/// Primary retail PC / PS2 `.cls` block magic bytes (`[0x18, 0x39, 0x84, 0x29]`).
pub const CLS_MAGIC_A_BYTES: [u8; 4] = CLS_MAGIC_A.to_le_bytes();
/// Secondary retail PC / PS2 `.cls` block magic identifier (`0x29853918`).
pub const CLS_MAGIC_B: u32 = 0x2985_3918;
/// Alternate PS2 debug prototype `.cls` header magic (`0x00020229`).
pub const CLS_MAGIC_PROTO: u32 = 0x0002_0229;

/// Maximum number of facial morph clusters per character model (`clani.c` `weight_work[32][20]`).
pub const MAX_FACIAL_CLUSTERS: usize = 20;

/// Single facial morph keyframe (`Element1` in `clani.c`, 4 bytes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClsKeyframe {
    /// Keyframe index in 30 FPS sub-clip frames.
    pub frame: u16,
    /// Signed Q4.12 fixed-point morph target weight (`4096 = 1.0`).
    pub weight_fp12: i16,
}

impl ClsKeyframe {
    /// Converts the signed Q4.12 fixed-point weight to a normalized `f32` (`weight_fp12 / 4096.0`).
    #[inline]
    pub fn weight_f32(self) -> f32 {
        (self.weight_fp12 as f32) * (1.0 / 4096.0)
    }
}

/// Keyframe track for a single facial morph cluster (`Cluster1` in `clani.c`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClsTrack {
    /// Sorted keyframes for this morph target.
    pub keys: Vec<ClsKeyframe>,
}

impl ClsTrack {
    /// Evaluates the cluster weight at `(frame, subframe_fp12)` matching `Calc1`
    /// (`0x125920` in `SLUS_202.28`, `0x506ea0` in `sh2pc.exe`).
    ///
    /// - `frame`: Integer frame within the sub-clip (`0..n_frames`).
    /// - `n_frames`: Total frames in the sub-clip header.
    /// - `subframe_fp12`: Fractional sub-frame in `0..4096` (`(frac * 4096.0) as u32`).
    pub fn evaluate_calc1(&self, frame: u16, n_frames: u16, subframe_fp12: u16) -> f32 {
        let n = self.keys.len();
        if n == 0 {
            return 0.0;
        }
        if n == 1 {
            return self.keys[0].weight_f32();
        }

        let (i0, i1, t) = if n_frames > 0 && frame + 1 == n_frames {
            // Last frame blends from the final key into key 0 (looping seam in `Calc1`)
            (n - 1, 0, (subframe_fp12 as f32) * (1.0 / 4096.0))
        } else {
            let last_key = self.keys[n - 1];
            if frame >= last_key.frame {
                return last_key.weight_f32();
            }

            // Binary search for `lo` such that `keys[lo].frame <= frame < keys[lo + 1].frame`
            let mut lo = 0usize;
            let mut hi = n - 1;
            while hi - lo > 1 {
                let mid = (lo + hi) / 2;
                if self.keys[mid].frame <= frame {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }

            let mut k0 = lo;
            let mut k1 = lo + 1;
            let mut f0 = self.keys[k0].frame;
            let mut f1 = self.keys[k1].frame;
            if frame.saturating_sub(f0) == f1.saturating_sub(f0) && k1 + 1 < n {
                k0 = k1;
                k1 = k0 + 1;
                f0 = self.keys[k0].frame;
                f1 = self.keys[k1].frame;
            }
            let span = f1.abs_diff(f0) as f32;
            let frac = if span > 0.0 {
                ((frame.saturating_sub(f0) as f32) + (subframe_fp12 as f32) * (1.0 / 4096.0)) / span
            } else {
                0.0
            };
            (k0, k1, frac.clamp(0.0, 1.0))
        };

        let w0 = self.keys[i0].weight_fp12 as f32;
        let w1 = self.keys[i1].weight_fp12 as f32;
        ((1.0 - t) * w0 + t * w1) * (1.0 / 4096.0)
    }
}

/// A single facial cluster animation sub-clip block (`Header` in `clani.c`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClsBlock {
    /// Byte offset of this block within the parent `.cls` file.
    pub byte_offset: usize,
    /// Byte length of this block (padded to 4-byte alignment).
    pub byte_len: usize,
    /// Block header magic (`0x29843918` or `0x29853918`).
    pub magic: u32,
    /// Format revision (`1`).
    pub revision: u16,
    /// Header flags (`0`).
    pub flags: u16,
    /// Total 30 FPS frames in this sub-clip.
    pub n_frames: u16,
    /// Per-cluster keyframe tracks (`len == n_clusters`).
    pub clusters: Vec<ClsTrack>,
}

impl ClsBlock {
    /// Evaluates all cluster weights at continuous 30 FPS `motion_frame` into `out_weights`
    /// with zero heap allocations (`ClusterAnimeExec` at `0x125de0` PS2 / `0x507120` PC).
    pub fn evaluate_weights_into(
        &self,
        motion_frame: f32,
        looping: bool,
        out_weights: &mut Vec<f32>,
    ) {
        out_weights.clear();
        if out_weights.capacity() < self.clusters.len() {
            out_weights.reserve(self.clusters.len() - out_weights.capacity());
        }

        let clamped_time = motion_frame.max(0.0);
        let mut frame = clamped_time.floor() as u32;
        let mut subframe_fp12 = ((clamped_time - (frame as f32)) * 4096.0)
            .round()
            .clamp(0.0, 4095.0) as u16;

        let n_frames = self.n_frames as u32;
        if n_frames > 0 && frame >= n_frames {
            if looping {
                frame %= n_frames;
            } else {
                frame = n_frames - 1;
                subframe_fp12 = 0;
            }
        }

        let frame_u16 = frame.min(u16::MAX as u32) as u16;
        for track in &self.clusters {
            out_weights.push(track.evaluate_calc1(frame_u16, self.n_frames, subframe_fp12));
        }
    }

    /// Evaluates all cluster weights at continuous 30 FPS `motion_frame`.
    pub fn evaluate_weights(&self, motion_frame: f32, looping: bool) -> Vec<f32> {
        let mut out = Vec::with_capacity(self.clusters.len());
        self.evaluate_weights_into(motion_frame, looping, &mut out);
        out
    }
}

/// Complete parsed `.cls` file containing one or more concatenated [`ClsBlock`] sub-clips.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClsFile {
    /// Sequential sub-clip blocks in the `.cls` file.
    pub blocks: Vec<ClsBlock>,
}

impl ClsFile {
    /// Returns the number of facial morph cluster tracks in the first block (or `0` if empty).
    #[inline]
    pub fn num_clusters(&self) -> usize {
        self.blocks.first().map(|b| b.clusters.len()).unwrap_or(0)
    }

    /// Returns the total number of 30 FPS frames summed across all blocks in this `.cls` file.
    #[inline]
    pub fn total_frames(&self) -> usize {
        self.blocks.iter().map(|b| b.n_frames as usize).sum()
    }

    /// Looks up a sub-clip block by its starting byte offset (`clani_adr_list` entry).
    pub fn block_at_offset(&self, byte_offset: usize) -> Option<&ClsBlock> {
        self.blocks.iter().find(|b| b.byte_offset == byte_offset)
    }

    /// Evaluates cluster weights at a global frame index across all concatenated blocks,
    /// useful when scrubbing a multi-clip `.anm`/`.cls` pair in the 3D Model Viewer.
    pub fn evaluate_global_frame_into(
        &self,
        global_frame: f32,
        looping: bool,
        out_weights: &mut Vec<f32>,
    ) {
        if self.blocks.is_empty() {
            out_weights.clear();
            return;
        }
        if self.blocks.len() == 1 {
            self.blocks[0].evaluate_weights_into(global_frame, looping, out_weights);
            return;
        }

        let mut remaining = global_frame.max(0.0);
        for (i, block) in self.blocks.iter().enumerate() {
            let nf = block.n_frames as f32;
            if remaining < nf || i + 1 == self.blocks.len() {
                block.evaluate_weights_into(remaining.min(nf), false, out_weights);
                return;
            }
            remaining -= nf;
        }
    }
}

/// Returns `true` if `magic` is a valid `.cls` block magic (`0x29843918`, `0x29853918`, or `0x00020229`).
#[inline]
pub const fn is_valid_cls_magic(magic: u32) -> bool {
    magic == CLS_MAGIC_A || magic == CLS_MAGIC_B || magic == CLS_MAGIC_PROTO
}

/// Parses a single [`ClsBlock`] starting at `offset` in `data`.
pub fn parse_cls_block_at(data: &[u8], offset: usize) -> Result<ClsBlock, AssetError> {
    if offset + 16 > data.len() {
        return Err(AssetError::BufferUnderflow {
            required: offset + 16,
            available: data.len(),
        });
    }

    let magic = read_u32(data, offset)?;
    if !is_valid_cls_magic(magic) {
        return Err(AssetError::InvalidMagic {
            expected: &CLS_MAGIC_A_BYTES,
            found: data[offset..offset + 4].to_vec(),
        });
    }

    let (revision, flags, n_clusters, n_frames, offsets_base, key_header_size) =
        if magic == CLS_MAGIC_PROTO {
            // Prototype 12-byte header: u32 magic, u32 n_frames, u32 n_clusters
            let nf = read_u32(data, offset + 4)? as u16;
            let nc = read_u32(data, offset + 8)? as u16;
            (1u16, 0u16, nc, nf, offset + 12, 4usize)
        } else {
            let rev = read_u16(data, offset + 4)?;
            let flg = read_u16(data, offset + 6)?;
            let nc = read_u16(data, offset + 8)?;
            let nf = read_u16(data, offset + 12)?;
            (rev, flg, nc, nf, offset + 16, 2usize)
        };

    let nc_usize = n_clusters as usize;
    let table_end = offsets_base.saturating_add(nc_usize * 4);
    if table_end > data.len() {
        return Err(AssetError::BufferUnderflow {
            required: table_end,
            available: data.len(),
        });
    }

    let mut clusters = Vec::with_capacity(nc_usize);
    let mut max_end = table_end;

    for c in 0..nc_usize {
        let rel_ofs = read_u32(data, offsets_base + c * 4)? as usize;
        let track_pos = offset.saturating_add(rel_ofs);
        if track_pos + key_header_size > data.len() {
            return Err(AssetError::BufferUnderflow {
                required: track_pos + key_header_size,
                available: data.len(),
            });
        }

        let n_keys = read_u16(data, track_pos)? as usize;
        let keys_start = track_pos + key_header_size;
        let keys_end = keys_start.saturating_add(n_keys * 4);
        if keys_end > data.len() {
            return Err(AssetError::BufferUnderflow {
                required: keys_end,
                available: data.len(),
            });
        }
        max_end = max_end.max(keys_end);

        let mut keys = Vec::with_capacity(n_keys);
        for k in 0..n_keys {
            let kp = keys_start + k * 4;
            let frame = read_u16(data, kp)?;
            let weight_fp12 = read_i16(data, kp + 2)?;
            keys.push(ClsKeyframe { frame, weight_fp12 });
        }
        clusters.push(ClsTrack { keys });
    }

    let aligned_end = ((max_end + 3) & !3).min(data.len());
    Ok(ClsBlock {
        byte_offset: offset,
        byte_len: aligned_end.saturating_sub(offset),
        magic,
        revision,
        flags,
        n_frames,
        clusters,
    })
}

/// Parses a complete `.cls` facial morph cluster animation file from raw bytes.
pub fn parse_cls(data: &[u8]) -> Result<ClsFile, AssetError> {
    if data.len() < 16 {
        return Err(AssetError::BufferUnderflow {
            required: 16,
            available: data.len(),
        });
    }

    let mut blocks = Vec::new();
    let mut offset = 0usize;

    while offset < data.len() {
        // Skip trailing zero padding at EOF if < 16 bytes remain
        if data.len() - offset < 16 && data[offset..].iter().all(|&b| b == 0) {
            break;
        }
        let block = parse_cls_block_at(data, offset)?;
        let step = block.byte_len.max(4);
        blocks.push(block);
        offset = offset.saturating_add(step);
    }

    Ok(ClsFile { blocks })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_synthetic_cls_and_calc1_interpolation() {
        // Build a synthetic 2-cluster, 100-frame .cls block with magic 0x29843918
        let mut buf = Vec::new();
        buf.extend_from_slice(&CLS_MAGIC_A.to_le_bytes());
        buf.extend_from_slice(&1u16.to_le_bytes()); // revision 1
        buf.extend_from_slice(&0u16.to_le_bytes()); // flags 0
        buf.extend_from_slice(&2u16.to_le_bytes()); // n_clusters = 2
        buf.extend_from_slice(&0u16.to_le_bytes()); // pad
        buf.extend_from_slice(&100u16.to_le_bytes()); // n_frames = 100
        buf.extend_from_slice(&0u16.to_le_bytes()); // pad

        // Cluster offsets at +16 (2 * 4 = 8 bytes -> tracks start at 24)
        let c0_ofs = 24u32;
        // Track 0 has 2 keys: 2 + 2*4 = 10 bytes -> Track 1 at 34
        let c1_ofs = 34u32;
        buf.extend_from_slice(&c0_ofs.to_le_bytes());
        buf.extend_from_slice(&c1_ofs.to_le_bytes());

        // Track 0: 2 keys: (frame 0, weight 0), (frame 10, weight 4096 = 1.0)
        buf.extend_from_slice(&2u16.to_le_bytes());
        buf.extend_from_slice(&0u16.to_le_bytes());
        buf.extend_from_slice(&0i16.to_le_bytes());
        buf.extend_from_slice(&10u16.to_le_bytes());
        buf.extend_from_slice(&4096i16.to_le_bytes());

        // Track 1: 1 key: (frame 0, weight 2048 = 0.5)
        buf.extend_from_slice(&1u16.to_le_bytes());
        buf.extend_from_slice(&0u16.to_le_bytes());
        buf.extend_from_slice(&2048i16.to_le_bytes());

        let cls = parse_cls(&buf).expect("valid synthetic .cls");
        assert_eq!(cls.blocks.len(), 1);
        let blk = &cls.blocks[0];
        assert_eq!(blk.n_frames, 100);
        assert_eq!(blk.clusters.len(), 2);

        // At frame 5.0 (halfway between 0 and 10), cluster 0 weight = 0.5, cluster 1 weight = 0.5
        let w = blk.evaluate_weights(5.0, false);
        assert!((w[0] - 0.5).abs() < 1e-4);
        assert!((w[1] - 0.5).abs() < 1e-4);

        // At frame 10.0, cluster 0 weight = 1.0
        let w10 = blk.evaluate_weights(10.0, false);
        assert!((w10[0] - 1.0).abs() < 1e-4);
    }
}
