//! Safe binary parser for Silent Hill 2 Skeletal Animations (`.anm`).
//!
//! # Format Architecture & Memory Layout
//! Silent Hill 2 `.anm` files contain keyframed joint transformations (rotation and translation)
//! compressed into sequential 8-bone blocks.
//!
//! ### Animation Block Structure
//! The animation stream consists of repeated chunks corresponding to groups of 8 bones:
//! - **Block Header (4 bytes)**: A 32-bit bitfield containing 8 nibbles (4 bits per bone).
//!   Each 4-bit track code indicates the transform data stored for that bone:
//!   - `1`: Fixed-point Euler rotation (3 $\times$ `i16`, scaled by $1 / 4096.0$ radians).
//!   - `2`: Translation followed by Euler rotation.
//!     If root bone: translation is 3 x `f32` (12 bytes).
//!     If non-root bone: translation is 3 x `f16` half-precision floats (6 bytes).
//!   - `3`: Extended Euler rotation (`3h` + 10-byte padding).
//!   - `4` / `5`: Extended translation and rotation tracks.
//!   - `6`: Dual translation and rotation tracks (used for root trajectories and complex joints).
//! - **Frame Boundaries**:
//!   When `(block * 8)` wraps across the aligned `per_frame = (bone_count + 7) & !7` boundary,
//!   a complete [`AnimationFrame`] has been decoded.

use super::binary::{read_f32, read_i16, read_u16, read_u32};
use crate::error::AssetError;

/// Transform track encoding types declared in the 4-bit nibbles of block headers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationTrackType {
    /// 3 x `i16` rotation angles (Euler radians / 4096).
    RotationOnly,
    /// Vector translation (3xf32 or 3xf16) and 3 x `i16` rotation.
    TranslationAndRotation,
    /// Rotation track with trailing metadata padding.
    ExtendedRotation,
    /// Extended translation (single-precision root or half-precision joint) and rotation.
    ExtendedTranslationAndRotation1,
    /// Alternate extended translation layout with padding.
    ExtendedTranslationAndRotation2,
    /// Dual translation vectors and rotation (used for complex root motion).
    DualTranslationAndRotation,
    /// Unrecognized track code.
    Unknown(u8),
}

impl From<u8> for AnimationTrackType {
    fn from(val: u8) -> Self {
        match val {
            1 => AnimationTrackType::RotationOnly,
            2 => AnimationTrackType::TranslationAndRotation,
            3 => AnimationTrackType::ExtendedRotation,
            4 => AnimationTrackType::ExtendedTranslationAndRotation1,
            5 => AnimationTrackType::ExtendedTranslationAndRotation2,
            6 => AnimationTrackType::DualTranslationAndRotation,
            other => AnimationTrackType::Unknown(other),
        }
    }
}

/// Rotation and translation for a specific bone in an animation frame.
///
/// Rotations are stored in Euler radians (scaled from original 4096 units/radian).
#[derive(Debug, Clone, PartialEq)]
pub struct BoneTransform {
    /// Euler rotation angles $(R_x, R_y, R_z)$ in radians, if present.
    pub rotation: Option<[f32; 3]>,
    /// Translation offset $(T_x, T_y, T_z)$ in world/parent space, if present.
    pub translation: Option<[f32; 3]>,
    /// Encoding descriptor from the frame block header.
    pub track_type: AnimationTrackType,
}

/// A complete animation frame containing transform entries for each bone.
#[derive(Debug, Clone, PartialEq)]
pub struct AnimationFrame {
    /// Array of bone transforms indexed by skeleton bone ID.
    pub bones: Vec<Option<BoneTransform>>,
}

/// Parsed skeletal animation sequence.
#[derive(Debug, Clone)]
pub struct AnimationFile {
    /// Total number of bones animated by this sequence.
    pub bone_count: usize,
    /// Ordered sequence of decoded animation frames.
    pub frames: Vec<AnimationFrame>,
    /// For each decoded frame `i` (`0..frames.len()`), the span `[prev_keyframe, next_keyframe]`
    /// enclosing `i` across sparse hold frames (`t == 0` for all bones in the frame).
    ///
    /// In Silent Hill 2 cutscene `.anm` files (`haka_agl`, `knife_agl`, `papa_agl`, `mar_deai`,
    /// `gero_edi`, `piano_lau`, `room312`, `fire_agl`, `mry_yarinaoshi_a`, `fukatsu_a`), the 30 FPS
    /// frame stream stores explicit bone keyframes every 3 frames (10 Hz) separated by 2 hold frames
    /// (`t == 0` for all bones). On PS2/PC (`shAnime3d` / `shExec`), the engine interpolates smoothly
    /// across the 3-frame span `[prev_keyframe, next_keyframe]` at 30/60 Hz rather than stepping in
    /// a staircase of 1-frame lerps followed by 2 frozen hold frames.
    pub keyframe_spans: Vec<(usize, usize)>,
}

impl AnimationFile {
    /// Evaluates the continuous keyframe interpolation pair `(f0, f1, frac)` for a continuous 30 FPS
    /// `actor_frame`, automatically bridging sparse hold frames (`t == 0`) so 10/15 Hz keyframed
    /// cutscene tracks interpolate smoothly at full 30/60+ FPS instead of freezing for 2 out of every 3 frames.
    #[inline]
    pub fn sample_span_at(&self, actor_frame: f32) -> (usize, usize, f32) {
        let n = self.frames.len();
        if n == 0 {
            return (0, 0, 0.0);
        }
        let clamped = actor_frame.clamp(0.0, (n - 1) as f32);
        let idx = (clamped.floor() as usize).min(n - 1);
        if let Some(&(k0, k1)) = self.keyframe_spans.get(idx) {
            if k1 > k0 && k1 < n {
                let span = (k1 - k0) as f32;
                let frac = ((clamped - (k0 as f32)) / span).clamp(0.0, 1.0);
                return (k0, k1, frac);
            }
        }
        let f1 = (idx + 1).min(n - 1);
        (idx, f1, clamped.fract())
    }
}

/// Checks whether `p` sits on a 2-byte `0x0000` 4-byte alignment pad at a frame boundary
/// (either at EOF or between concatenated sub-clips in a multi-clip `d*_anime_adr_list` drama `.anm`).
///
/// In PS2 `SLUS_202.28` (`dangela_anime_adr_list`, `deddie_anime_adr_list`, `dlaura_anime_adr_list`,
/// `dmary_anime_adr_list`, and prop/weapon `d*_anime_adr_list`), concatenated cutscene sub-clips are
/// 4-byte aligned so MIPS `lw` can read the initial block header. Every sub-clip begins at frame 0
/// with all `min(bone_count, 8)` bones in its first block initialized with translation+rotation
/// (`t & 7 == 2`, e.g. `0x22222222`/`0xaaaaaaaa` for $\ge 8$ bones, `0x0aaaaaaa` for 7-bone `i_letter`,
/// `0x00000aaa` for 3-bone `b_d1x`, or `0x00000002`/`0x0000000a` for 1-bone props).
#[inline]
fn is_interclip_or_eof_alignment_pad(data: &[u8], p: usize, bone_count: usize) -> bool {
    if p + 2 > data.len() || data[p] != 0 || data[p + 1] != 0 {
        return false;
    }
    if p + 2 == data.len() {
        return true;
    }
    if p + 6 > data.len() {
        return false;
    }
    if bone_count >= 8 {
        (data[p + 2] != 0 || data[p + 3] != 0)
            && data[p + 2] == data[p + 4]
            && data[p + 3] == data[p + 5]
    } else {
        let next_hdr = u32::from_le_bytes([data[p + 2], data[p + 3], data[p + 4], data[p + 5]]);
        let first_blk_bones = bone_count.min(8);
        let mut expected_mask = 0u32;
        for j in 0..first_blk_bones {
            expected_mask |= 2u32 << (4 * j);
        }
        (next_hdr & 0x7777_7777) == expected_mask
    }
}

/// Tests if the animation byte stream can be validly and completely decoded with the given bone count stride.
fn is_valid_anm_stride(data: &[u8], bone_count: usize, parents: &[u8]) -> bool {
    if bone_count == 0 {
        return false;
    }
    let per_frame = (bone_count + 7) & !7;
    let mut p = 0usize;
    let mut block = 0usize;

    while p < data.len() {
        if (block * 8).is_multiple_of(per_frame)
            && is_interclip_or_eof_alignment_pad(data, p, bone_count)
        {
            p += 2;
            if p == data.len() {
                break;
            }
        }
        if p + 4 > data.len() {
            return false;
        }
        let header = match read_u32(data, p) {
            Ok(h) => h,
            Err(_) => return false,
        };
        p += 4;

        for j in 0..8 {
            let t = ((header >> (4 * j)) & 7) as u8;
            let bone_idx = ((block * 8) % per_frame) + j;
            let is_root = if bone_idx < parents.len() {
                parents[bone_idx] == 255
            } else {
                false
            };

            let need = match t {
                0 => 0,
                1 => 6,
                2 => {
                    if is_root {
                        18
                    } else {
                        12
                    }
                }
                3 => 16,
                4 | 5 => {
                    if is_root {
                        28
                    } else {
                        22
                    }
                }
                6 => {
                    if is_root {
                        40
                    } else {
                        28
                    }
                }
                _ => return false,
            };

            if p + need > data.len() {
                return false;
            }
            p += need;
        }
        block += 1;
    }

    let valid_end = p == data.len() || (p + 2 == data.len() && data[p] == 0 && data[p + 1] == 0);
    valid_end && (block * 8).is_multiple_of(per_frame)
}

/// Parses raw `.anm` bytes into an [`AnimationFile`] given the target model's bone hierarchy.
///
/// # Arguments
/// - `data`: Raw byte slice of the `.anm` file.
/// - `bone_count`: Skeleton bone count from the matching [`.mdl` / `.mdls`](crate::formats::mdls::ModelFile).
/// - `parents`: Skeleton parent array used to identify root bones (`parents[i] == 255`).
///
/// If `bone_count` is 71 (cutscene model `hhh_jms.mdl`) whose first 41 bones `0..41` are identical
/// to the 41-bone gameplay skeleton (`hll_jms.mdl`), and the animation data represents a 41-bone
/// standard gameplay track (e.g. `jms_basic1.anm` with stride 48), the parser automatically
/// detects the 41-bone track stride and parses the animation frames accordingly. Unspecified finger
/// bones (`41..71`) automatically follow their wrist parents via forward kinematics in [`evaluate_pose`].
pub fn parse_anm(
    data: &[u8],
    bone_count: usize,
    parents: &[u8],
) -> Result<AnimationFile, AssetError> {
    if bone_count == 0 {
        return Ok(AnimationFile {
            bone_count: 0,
            frames: Vec::new(),
            keyframe_spans: Vec::new(),
        });
    }

    if is_valid_anm_stride(data, bone_count, parents) {
        return parse_anm_with_stride(data, bone_count, parents);
    }

    // Special case: 71-bone high-LOD James (`hhh_jms.mdl`) shares the exact same first 41 bones
    // (`0..41`) as 41-bone gameplay James (`hll_jms.mdl`), with finger bones appended at `41..71`.
    if bone_count == 71 && is_valid_anm_stride(data, 41, parents) {
        return parse_anm_with_stride(data, 41, parents);
    }

    // For synthetic unit test buffers or custom tracks, check if any candidate stride cleanly decodes
    // ONLY when the buffer is small or exact stride failed, EXCEPT never map a 35/36-bone gameplay
    // track onto a 64/73-bone high-LOD cutscene skeleton whose facial/hair bones are interleaved!
    if bone_count <= 48 {
        for &candidate in &[41usize, 36, 35, 32, 40, 48, 24, 16, 8] {
            if candidate != bone_count && is_valid_anm_stride(data, candidate, parents) {
                return parse_anm_with_stride(data, candidate, parents);
            }
        }
    }

    if data.is_empty() {
        return Ok(AnimationFile {
            bone_count,
            frames: Vec::new(),
            keyframe_spans: Vec::new(),
        });
    }

    // If this is a real multi-KB retail `.anm` stream that failed stride validation for `bone_count`,
    // return an error rather than force-decoding with a misaligned stride (which would interpret
    // rotation nibbles as root translations and skew vertices wildly).
    if data.len() > 256 {
        return Err(AssetError::InvalidData(format!(
            "Animation byte stream ({} bytes) is incompatible with skeleton bone_count={}",
            data.len(),
            bone_count
        )));
    }

    parse_anm_with_stride(data, bone_count, parents)
}

fn parse_anm_with_stride(
    data: &[u8],
    bone_count: usize,
    parents: &[u8],
) -> Result<AnimationFile, AssetError> {
    let per_frame = (bone_count + 7) & !7;
    let mut frames = Vec::new();
    let mut is_keyframe_flags: Vec<bool> = Vec::new();
    let mut cur_frame: Vec<Option<BoneTransform>> = vec![None; bone_count];
    let mut cur_frame_has_delta = false;

    let mut p = 0usize;
    let mut block = 0usize;

    while p < data.len() {
        if (block * 8).is_multiple_of(per_frame)
            && is_interclip_or_eof_alignment_pad(data, p, bone_count)
        {
            p += 2;
        }
        if p + 4 > data.len() {
            break;
        }
        let header = read_u32(data, p)?;
        p += 4;

        for j in 0..8 {
            let t = ((header >> (4 * j)) & 7) as u8;
            let bone_idx = ((block * 8) % per_frame) + j;
            let is_root = if bone_idx < parents.len() {
                parents[bone_idx] == 255
            } else {
                false
            };

            let mut rot: Option<[f32; 3]> = None;
            let mut trans: Option<[f32; 3]> = None;

            match t {
                1 => {
                    // Type 1: 3x i16 rotations
                    if p + 6 <= data.len() {
                        let rx = read_i16(data, p)? as f32 / 4096.0;
                        let ry = read_i16(data, p + 2)? as f32 / 4096.0;
                        let rz = read_i16(data, p + 4)? as f32 / 4096.0;
                        rot = Some([rx, ry, rz]);
                        p += 6;
                    }
                }
                2 => {
                    // Type 2: Root translation (3x f32) or joint translation (3x f16), then 3x i16 rot
                    if is_root {
                        if p + 12 <= data.len() {
                            let tx = read_f32(data, p)?;
                            let ty = read_f32(data, p + 4)?;
                            let tz = read_f32(data, p + 8)?;
                            trans = Some([tx, ty, tz]);
                            p += 12;
                        }
                    } else {
                        if p + 6 <= data.len() {
                            let tx = f16_to_f32(read_u16(data, p)?);
                            let ty = f16_to_f32(read_u16(data, p + 2)?);
                            let tz = f16_to_f32(read_u16(data, p + 4)?);
                            trans = Some([tx, ty, tz]);
                            p += 6;
                        }
                    }
                    if p + 6 <= data.len() {
                        let rx = read_i16(data, p)? as f32 / 4096.0;
                        let ry = read_i16(data, p + 2)? as f32 / 4096.0;
                        let rz = read_i16(data, p + 4)? as f32 / 4096.0;
                        rot = Some([rx, ry, rz]);
                        p += 6;
                    }
                }
                3 => {
                    // Type 3: 3x i16 rotations + 10 bytes padding
                    if p + 6 <= data.len() {
                        let rx = read_i16(data, p)? as f32 / 4096.0;
                        let ry = read_i16(data, p + 2)? as f32 / 4096.0;
                        let rz = read_i16(data, p + 4)? as f32 / 4096.0;
                        rot = Some([rx, ry, rz]);
                        p += 6 + 10;
                    }
                }
                4 | 5 => {
                    // Types 4 & 5: Extended translation tracks with 10 bytes padding
                    if t == 4 && is_root {
                        if p + 12 <= data.len() {
                            let tx = read_f32(data, p)?;
                            let ty = read_f32(data, p + 4)?;
                            let tz = read_f32(data, p + 8)?;
                            trans = Some([tx, ty, tz]);
                            p += 12;
                        }
                    } else if t == 5 && is_root {
                        if p + 12 <= data.len() {
                            let tx = f16_to_f32(read_u16(data, p)?);
                            let ty = f16_to_f32(read_u16(data, p + 4)?);
                            let tz = f16_to_f32(read_u16(data, p + 8)?);
                            trans = Some([tx, ty, tz]);
                            p += 12;
                        }
                    } else {
                        if p + 6 <= data.len() {
                            let tx = f16_to_f32(read_u16(data, p)?);
                            let ty = f16_to_f32(read_u16(data, p + 2)?);
                            let tz = f16_to_f32(read_u16(data, p + 4)?);
                            trans = Some([tx, ty, tz]);
                            p += 6;
                        }
                    }
                    if p + 6 <= data.len() {
                        let rx = read_i16(data, p)? as f32 / 4096.0;
                        let ry = read_i16(data, p + 2)? as f32 / 4096.0;
                        let rz = read_i16(data, p + 4)? as f32 / 4096.0;
                        rot = Some([rx, ry, rz]);
                        p += 6 + 10;
                    }
                }
                6 => {
                    // Type 6: Dual translation vectors (PC 0x54FC67)
                    if is_root {
                        if p + 12 <= data.len() {
                            let tx = read_f32(data, p)?;
                            let ty = read_f32(data, p + 4)?;
                            let tz = read_f32(data, p + 8)?;
                            trans = Some([tx, ty, tz]);
                            p += 24;
                        }
                    } else {
                        if p + 6 <= data.len() {
                            let tx = f16_to_f32(read_u16(data, p)?);
                            let ty = f16_to_f32(read_u16(data, p + 2)?);
                            let tz = f16_to_f32(read_u16(data, p + 4)?);
                            trans = Some([tx, ty, tz]);
                            p += 12;
                        }
                    }
                    if p + 6 <= data.len() {
                        let rx = read_i16(data, p)? as f32 / 4096.0;
                        let ry = read_i16(data, p + 2)? as f32 / 4096.0;
                        let rz = read_i16(data, p + 4)? as f32 / 4096.0;
                        rot = Some([rx, ry, rz]);
                        p += 6 + 10;
                    }
                }
                _ => {}
            }

            if bone_idx < bone_count && t != 0 {
                cur_frame_has_delta = true;
                let prev_rot = cur_frame[bone_idx].as_ref().and_then(|b| b.rotation);
                let prev_trans = cur_frame[bone_idx].as_ref().and_then(|b| b.translation);
                cur_frame[bone_idx] = Some(BoneTransform {
                    rotation: rot.or(prev_rot),
                    translation: trans.or(prev_trans),
                    track_type: AnimationTrackType::from(t),
                });
            }
        }

        block += 1;
        if (block * 8).is_multiple_of(per_frame) {
            frames.push(AnimationFrame {
                bones: cur_frame.clone(),
            });
            is_keyframe_flags.push(cur_frame_has_delta || is_keyframe_flags.is_empty());
            cur_frame_has_delta = false;
        }
    }

    let n_frames = frames.len();
    let mut keyframe_spans = vec![(0usize, 0usize); n_frames];
    if n_frames > 0 {
        let mut prev_kf = 0usize;
        let mut prev_kfs = vec![0usize; n_frames];
        for (i, &is_kf) in is_keyframe_flags.iter().enumerate() {
            if is_kf {
                prev_kf = i;
            }
            prev_kfs[i] = prev_kf;
        }
        let mut next_kf = n_frames - 1;
        for i in (0..n_frames).rev() {
            let k0 = prev_kfs[i];
            let k1 = if i + 1 < n_frames { next_kf } else { i };
            // Only bridge sparse hold intervals up to 6 frames (e.g. 10 Hz / 15 Hz cutscene sampling);
            // if a prop is stationary for longer than 6 frames, hold its pose rather than drifting.
            if k1 > k0 && (k1 - k0) <= 6 {
                keyframe_spans[i] = (k0, k1);
            } else {
                let adj = (i + 1).min(n_frames - 1);
                keyframe_spans[i] = (i, adj);
            }
            if is_keyframe_flags[i] {
                next_kf = i;
            }
        }
    }

    Ok(AnimationFile {
        bone_count,
        frames,
        keyframe_spans,
    })
}

/// Evaluates a 3x3 rotation matrix from ZYX Euler angles $(R_x, R_y, R_z)$ in radians:
/// $R = R_z \cdot R_y \cdot R_x$.
pub fn euler_to_matrix(angles: [f32; 3]) -> [[f32; 3]; 3] {
    let [rx, ry, rz] = angles;
    let (cx, sx) = (rx.cos(), rx.sin());
    let (cy, sy) = (ry.cos(), ry.sin());
    let (cz, sz) = (rz.cos(), rz.sin());

    [
        [cz * cy, cz * sy * sx - sz * cx, cz * sy * cx + sz * sx],
        [sz * cy, sz * sy * sx + cz * cx, sz * sy * cx - cz * sx],
        [-sy, cy * sx, cy * cx],
    ]
}

/// Converts ZYX Euler angles $(R_x, R_y, R_z)$ in radians ($R = R_z \cdot R_y \cdot R_x$)
/// into a unit quaternion $[x, y, z, w]$.
#[inline]
pub fn euler_to_quat(angles: [f32; 3]) -> [f32; 4] {
    let [rx, ry, rz] = angles;
    let (sx, cx) = (0.5 * rx).sin_cos();
    let (sy, cy) = (0.5 * ry).sin_cos();
    let (sz, cz) = (0.5 * rz).sin_cos();

    quat_normalize([
        sx * cy * cz - cx * sy * sz,
        cx * sy * cz + sx * cy * sz,
        cx * cy * sz - sx * sy * cz,
        cx * cy * cz + sx * sy * sz,
    ])
}

/// Evaluates a $3 \times 3$ rotation matrix from a unit quaternion $[x, y, z, w]$.
#[inline]
pub fn quat_to_matrix(q: [f32; 4]) -> [[f32; 3]; 3] {
    let [x, y, z, w] = quat_normalize(q);
    let xx = x * x;
    let yy = y * y;
    let zz = z * z;
    let xy = x * y;
    let xz = x * z;
    let yz = y * z;
    let wx = w * x;
    let wy = w * y;
    let wz = w * z;

    [
        [1.0 - 2.0 * (yy + zz), 2.0 * (xy - wz), 2.0 * (xz + wy)],
        [2.0 * (xy + wz), 1.0 - 2.0 * (xx + zz), 2.0 * (yz - wx)],
        [2.0 * (xz - wy), 2.0 * (yz + wx), 1.0 - 2.0 * (xx + yy)],
    ]
}

/// Extracts ZYX Euler angles $(R_x, R_y, R_z)$ in radians from a unit quaternion $[x, y, z, w]$.
#[inline]
pub fn quat_to_euler(q: [f32; 4]) -> [f32; 3] {
    let m = quat_to_matrix(q);
    let sin_y = (-m[2][0]).clamp(-1.0, 1.0);
    if sin_y.abs() < 0.999_999 {
        let ry = sin_y.asin();
        let rx = m[2][1].atan2(m[2][2]);
        let rz = m[1][0].atan2(m[0][0]);
        [rx, ry, rz]
    } else {
        let ry = std::f32::consts::FRAC_PI_2.copysign(sin_y);
        let rx = (-m[1][2]).atan2(m[1][1]);
        [rx, ry, 0.0]
    }
}

/// Normalizes a quaternion $[x, y, z, w]$ to unit length.
#[inline]
pub fn quat_normalize(q: [f32; 4]) -> [f32; 4] {
    let len_sq = q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3];
    if len_sq > 1e-12 {
        let inv = 1.0 / len_sq.sqrt();
        [q[0] * inv, q[1] * inv, q[2] * inv, q[3] * inv]
    } else {
        [0.0, 0.0, 0.0, 1.0]
    }
}

/// Hamilton product $a \otimes b$ for quaternions $[x, y, z, w]$.
#[inline]
pub fn quat_mul(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    [
        a[3] * b[0] + a[0] * b[3] + a[1] * b[2] - a[2] * b[1],
        a[3] * b[1] - a[0] * b[2] + a[1] * b[3] + a[2] * b[0],
        a[3] * b[2] + a[0] * b[1] - a[1] * b[0] + a[2] * b[3],
        a[3] * b[3] - a[0] * b[0] - a[1] * b[1] - a[2] * b[2],
    ]
}

/// Conjugate (inverse for unit quaternions) $[-x, -y, -z, w]$.
#[inline]
pub fn quat_conjugate(q: [f32; 4]) -> [f32; 4] {
    [-q[0], -q[1], -q[2], q[3]]
}

/// Ensures `target` lies on the same $S^3$ hemisphere as `reference` ($\text{dot} \ge 0$).
#[inline]
pub fn quat_align_hemisphere(reference: [f32; 4], target: [f32; 4]) -> [f32; 4] {
    let dot = reference[0] * target[0]
        + reference[1] * target[1]
        + reference[2] * target[2]
        + reference[3] * target[3];
    if dot < 0.0 {
        [-target[0], -target[1], -target[2], -target[3]]
    } else {
        target
    }
}

/// Spherical Linear Interpolation (Slerp) along the shortest geodesic arc on $SO(3)$.
#[inline]
pub fn quat_slerp(qa: [f32; 4], qb: [f32; 4], alpha: f32) -> [f32; 4] {
    let t = alpha.clamp(0.0, 1.0);
    let qb = quat_align_hemisphere(qa, qb);
    let dot = (qa[0] * qb[0] + qa[1] * qb[1] + qa[2] * qb[2] + qa[3] * qb[3]).clamp(-1.0, 1.0);

    if dot > 0.9995 {
        let inv_t = 1.0 - t;
        return quat_normalize([
            qa[0] * inv_t + qb[0] * t,
            qa[1] * inv_t + qb[1] * t,
            qa[2] * inv_t + qb[2] * t,
            qa[3] * inv_t + qb[3] * t,
        ]);
    }

    let theta_0 = dot.acos();
    let theta = theta_0 * t;
    let sin_theta = theta.sin();
    let sin_theta_0 = theta_0.sin();

    let s0 = theta.cos() - dot * sin_theta / sin_theta_0;
    let s1 = sin_theta / sin_theta_0;

    quat_normalize([
        qa[0] * s0 + qb[0] * s1,
        qa[1] * s0 + qb[1] * s1,
        qa[2] * s0 + qb[2] * s1,
        qa[3] * s0 + qb[3] * s1,
    ])
}

#[inline]
fn quat_ln(q: [f32; 4]) -> [f32; 3] {
    let v_len = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2]).sqrt();
    let w = q[3].clamp(-1.0, 1.0);
    if v_len > 1e-6 {
        let scale = v_len.atan2(w) / v_len;
        [q[0] * scale, q[1] * scale, q[2] * scale]
    } else {
        [q[0], q[1], q[2]]
    }
}

#[inline]
fn quat_exp(v: [f32; 3]) -> [f32; 4] {
    let theta = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if theta > 1e-6 {
        let scale = theta.sin() / theta;
        [v[0] * scale, v[1] * scale, v[2] * scale, theta.cos()]
    } else {
        quat_normalize([v[0], v[1], v[2], 1.0])
    }
}

#[inline]
fn quat_squad_tangent(q_prev: [f32; 4], q_curr: [f32; 4], q_next: [f32; 4]) -> [f32; 4] {
    let q_prev = quat_align_hemisphere(q_curr, q_prev);
    let q_next = quat_align_hemisphere(q_curr, q_next);
    let inv_curr = quat_conjugate(q_curr);
    let log_prev = quat_ln(quat_mul(inv_curr, q_prev));
    let log_next = quat_ln(quat_mul(inv_curr, q_next));
    let avg = [
        -0.25 * (log_prev[0] + log_next[0]),
        -0.25 * (log_prev[1] + log_next[1]),
        -0.25 * (log_prev[2] + log_next[2]),
    ];
    quat_normalize(quat_mul(q_curr, quat_exp(avg)))
}

/// Spherical Cubic Hermite / Squad quaternion interpolation across 4 consecutive keyframes
/// $(q_0, q_1, q_2, q_3)$ for parameter $\alpha \in [0, 1]$ between $q_1$ and $q_2$.
#[inline]
pub fn quat_hermite_slerp(
    q0: [f32; 4],
    q1: [f32; 4],
    q2: [f32; 4],
    q3: [f32; 4],
    alpha: f32,
) -> [f32; 4] {
    let t = alpha.clamp(0.0, 1.0);
    let q1 = quat_normalize(q1);
    let q2 = quat_align_hemisphere(q1, quat_normalize(q2));
    let q0 = quat_align_hemisphere(q1, quat_normalize(q0));
    let q3 = quat_align_hemisphere(q2, quat_normalize(q3));

    let s1 = quat_squad_tangent(q0, q1, q2);
    let s2 = quat_squad_tangent(q1, q2, q3);

    let slerp_q = quat_slerp(q1, q2, t);
    let slerp_s = quat_slerp(s1, s2, t);
    quat_slerp(slerp_q, slerp_s, 2.0 * t * (1.0 - t))
}

/// Cubic Catmull-Rom / Hermite spline interpolation for 3D bone translations.
#[inline]
pub fn vec3_hermite_catmull_rom(
    p0: [f32; 3],
    p1: [f32; 3],
    p2: [f32; 3],
    p3: [f32; 3],
    alpha: f32,
) -> [f32; 3] {
    let t = alpha.clamp(0.0, 1.0);
    let t2 = t * t;
    let t3 = t2 * t;
    let mut out = [0.0; 3];
    for k in 0..3 {
        let m1 = 0.5 * (p2[k] - p0[k]);
        let m2 = 0.5 * (p3[k] - p1[k]);
        out[k] = (2.0 * t3 - 3.0 * t2 + 1.0) * p1[k]
            + (t3 - 2.0 * t2 + t) * m1
            + (-2.0 * t3 + 3.0 * t2) * p2[k]
            + (t3 - t2) * m2;
    }
    out
}

/// Computes local parent-relative 4x4 bind transform matrices for all bones from initial bind matrices.
///
/// If bone `i` is a root bone (`parents[i] == 255`), `local[i] = initial[i]`.
/// Otherwise, `local[i] = initial[parents[i]]^{-1} * initial[i]`.
pub fn compute_bind_locals(parents: &[u8], initial: &[[[f32; 4]; 4]]) -> Vec<[[f32; 4]; 4]> {
    let n = parents.len().min(initial.len());
    let mut locals = Vec::with_capacity(n);

    for i in 0..n {
        let p = parents[i];
        if p == 255 || (p as usize) >= initial.len() {
            locals.push(initial[i]);
        } else {
            let inv_p = mat4_inverse(initial[p as usize]);
            locals.push(mat4_mul(inv_p, initial[i]));
        }
    }
    locals
}

/// Evaluates world/model-space transform matrices for each bone in a given animation frame.
///
/// Unspecified bone channels retain their local bind pose transform.
pub fn evaluate_pose(
    parents: &[u8],
    bind_locals: &[[[f32; 4]; 4]],
    frame: &AnimationFrame,
) -> Vec<[[f32; 4]; 4]> {
    let n = parents.len().min(bind_locals.len());
    let mut world = Vec::with_capacity(n);
    evaluate_pose_into(parents, bind_locals, frame, &mut world);
    world
}

/// Zero-allocation pose evaluation directly into an existing scratch vector.
pub fn evaluate_pose_into(
    parents: &[u8],
    bind_locals: &[[[f32; 4]; 4]],
    frame: &AnimationFrame,
    out_world: &mut Vec<[[f32; 4]; 4]>,
) {
    let n = parents.len().min(bind_locals.len());
    out_world.clear();
    if out_world.capacity() < n {
        out_world.reserve(n);
    }

    for i in 0..n {
        let mut loc = bind_locals[i];
        if let Some(Some(ref bone)) = frame.bones.get(i) {
            if let Some(rot) = bone.rotation {
                let rot_m = euler_to_matrix(rot);
                // Assign upper-left 3x3 rotation
                #[allow(clippy::needless_range_loop)]
                for r in 0..3 {
                    for c in 0..3 {
                        loc[r][c] = rot_m[r][c];
                    }
                }
            }
            if let Some(trans) = bone.translation {
                loc[0][3] = trans[0];
                loc[1][3] = trans[1];
                loc[2][3] = trans[2];
            }
        }

        let p = parents[i];
        if p == 255 || (p as usize) >= n {
            out_world.push(loc);
        } else {
            let parent_m = out_world[p as usize];
            out_world.push(mat4_mul(parent_m, loc));
        }
    }
}

/// Computes final skinning matrices ($M_{skin} = M_{world} \cdot M_{initial}^{-1}$) for each bone.
pub fn compute_skin_matrices(
    world_matrices: &[[[f32; 4]; 4]],
    initial_matrices: &[[[f32; 4]; 4]],
) -> Vec<[[f32; 4]; 4]> {
    let n = world_matrices.len().min(initial_matrices.len());
    let mut skin = Vec::with_capacity(n);
    for i in 0..n {
        let inv_init = mat4_inverse(initial_matrices[i]);
        skin.push(mat4_mul(world_matrices[i], inv_init));
    }
    skin
}

/// Zero-allocation in-place computation of skinning matrices using pre-inverted bind pose matrices.
pub fn compute_skin_matrices_with_inv_into(
    world_matrices: &[[[f32; 4]; 4]],
    inv_initial_matrices: &[[[f32; 4]; 4]],
    out_skin: &mut Vec<[[f32; 4]; 4]>,
) {
    let n = world_matrices.len().min(inv_initial_matrices.len());
    out_skin.clear();
    if out_skin.capacity() < n {
        out_skin.reserve(n);
    }
    for i in 0..n {
        out_skin.push(mat4_mul(world_matrices[i], inv_initial_matrices[i]));
    }
}

/// Computes final skinning matrices using pre-inverted bind pose matrices.
pub fn compute_skin_matrices_with_inv(
    world_matrices: &[[[f32; 4]; 4]],
    inv_initial_matrices: &[[[f32; 4]; 4]],
) -> Vec<[[f32; 4]; 4]> {
    let n = world_matrices.len().min(inv_initial_matrices.len());
    let mut skin = Vec::with_capacity(n);
    for i in 0..n {
        skin.push(mat4_mul(world_matrices[i], inv_initial_matrices[i]));
    }
    skin
}

/// Interpolates two ZYX Euler rotation triplets $(R_x, R_y, R_z)$ along the shortest
/// $SO(3)$ geodesic arc using unit quaternion Spherical Linear Interpolation (Slerp).
#[inline]
pub fn slerp_euler_zyx(ra: [f32; 3], rb: [f32; 3], alpha: f32) -> [f32; 3] {
    if alpha <= 0.0 {
        return ra;
    }
    if alpha >= 1.0 {
        return rb;
    }
    let qa = euler_to_quat(ra);
    let qb = euler_to_quat(rb);
    quat_to_euler(quat_slerp(qa, qb, alpha))
}

/// Interpolates four consecutive ZYX Euler rotation triplets $(r_0, r_1, r_2, r_3)$
/// using $C^1$ Spherical Cubic Hermite / Squad quaternion interpolation for $\alpha \in [0, 1]$
/// between $r_1$ and $r_2$.
#[inline]
pub fn hermite_slerp_euler_zyx(
    r0: [f32; 3],
    r1: [f32; 3],
    r2: [f32; 3],
    r3: [f32; 3],
    alpha: f32,
) -> [f32; 3] {
    if alpha <= 0.0 {
        return r1;
    }
    if alpha >= 1.0 {
        return r2;
    }
    let q0 = euler_to_quat(r0);
    let q1 = euler_to_quat(r1);
    let q2 = euler_to_quat(r2);
    let q3 = euler_to_quat(r3);
    quat_to_euler(quat_hermite_slerp(q0, q1, q2, q3, alpha))
}

/// Zero-allocation variant of [`interpolate_frames`] writing into a pre-allocated [`AnimationFrame`] buffer.
///
/// Uses unit quaternion Slerp (`slerp_euler_zyx`) for all bone rotations to prevent gimbal lock
/// and preserve constant angular velocity across 120–240 Hz+ sub-frame steps.
pub fn interpolate_frames_into(
    frame_a: &AnimationFrame,
    frame_b: &AnimationFrame,
    alpha: f32,
    out: &mut AnimationFrame,
) {
    let bone_count = frame_a.bones.len().max(frame_b.bones.len());
    out.bones.clear();
    if out.bones.capacity() < bone_count {
        out.bones.reserve(bone_count - out.bones.capacity());
    }

    let alpha = alpha.clamp(0.0, 1.0);
    let inv_alpha = 1.0 - alpha;

    for i in 0..bone_count {
        let b_a = frame_a.bones.get(i).and_then(|x| x.as_ref());
        let b_b = frame_b.bones.get(i).and_then(|x| x.as_ref());

        match (b_a, b_b) {
            (Some(a), Some(b)) => {
                let rot = match (a.rotation, b.rotation) {
                    (Some(ra), Some(rb)) => Some(slerp_euler_zyx(ra, rb, alpha)),
                    (Some(ra), None) => Some(ra),
                    (None, Some(rb)) => Some(rb),
                    (None, None) => None,
                };
                let trans = match (a.translation, b.translation) {
                    (Some(ta), Some(tb)) => Some([
                        ta[0] * inv_alpha + tb[0] * alpha,
                        ta[1] * inv_alpha + tb[1] * alpha,
                        ta[2] * inv_alpha + tb[2] * alpha,
                    ]),
                    (Some(ta), None) => Some(ta),
                    (None, Some(tb)) => Some(tb),
                    (None, None) => None,
                };
                out.bones.push(Some(BoneTransform {
                    rotation: rot,
                    translation: trans,
                    track_type: a.track_type,
                }));
            }
            (Some(a), None) => out.bones.push(Some(a.clone())),
            (None, Some(b)) => out.bones.push(Some(b.clone())),
            (None, None) => out.bones.push(None),
        }
    }
}

/// Zero-allocation 4-keyframe $C^1$ Spherical Cubic Hermite / Squad bone interpolation
/// between `frame_1` and `frame_2` using neighboring tangent frames `frame_0` and `frame_3`.
pub fn interpolate_frames_hermite_into(
    frame_0: &AnimationFrame,
    frame_1: &AnimationFrame,
    frame_2: &AnimationFrame,
    frame_3: &AnimationFrame,
    alpha: f32,
    out: &mut AnimationFrame,
) {
    let bone_count = frame_1.bones.len().max(frame_2.bones.len());
    out.bones.clear();
    if out.bones.capacity() < bone_count {
        out.bones.reserve(bone_count - out.bones.capacity());
    }

    let alpha = alpha.clamp(0.0, 1.0);

    for i in 0..bone_count {
        let b0 = frame_0.bones.get(i).and_then(|x| x.as_ref());
        let b1 = frame_1.bones.get(i).and_then(|x| x.as_ref());
        let b2 = frame_2.bones.get(i).and_then(|x| x.as_ref());
        let b3 = frame_3.bones.get(i).and_then(|x| x.as_ref());

        match (b1, b2) {
            (Some(a), Some(b)) => {
                let rot = match (a.rotation, b.rotation) {
                    (Some(r1), Some(r2)) => {
                        let r0 = b0.and_then(|x| x.rotation).unwrap_or(r1);
                        let r3 = b3.and_then(|x| x.rotation).unwrap_or(r2);
                        Some(hermite_slerp_euler_zyx(r0, r1, r2, r3, alpha))
                    }
                    (Some(r1), None) => Some(r1),
                    (None, Some(r2)) => Some(r2),
                    (None, None) => None,
                };
                let trans = match (a.translation, b.translation) {
                    (Some(t1), Some(t2)) => {
                        let t0 = b0.and_then(|x| x.translation).unwrap_or(t1);
                        let t3 = b3.and_then(|x| x.translation).unwrap_or(t2);
                        Some(vec3_hermite_catmull_rom(t0, t1, t2, t3, alpha))
                    }
                    (Some(t1), None) => Some(t1),
                    (None, Some(t2)) => Some(t2),
                    (None, None) => None,
                };
                out.bones.push(Some(BoneTransform {
                    rotation: rot,
                    translation: trans,
                    track_type: a.track_type,
                }));
            }
            (Some(a), None) => out.bones.push(Some(a.clone())),
            (None, Some(b)) => out.bones.push(Some(b.clone())),
            (None, None) => out.bones.push(None),
        }
    }
}

/// Evaluates 4-keyframe $C^1$ Spherical Cubic Hermite / Squad interpolation between `frame_1` and `frame_2`.
pub fn interpolate_frames_hermite(
    frame_0: &AnimationFrame,
    frame_1: &AnimationFrame,
    frame_2: &AnimationFrame,
    frame_3: &AnimationFrame,
    alpha: f32,
) -> AnimationFrame {
    let bone_count = frame_1.bones.len().max(frame_2.bones.len());
    let mut out = AnimationFrame {
        bones: Vec::with_capacity(bone_count),
    };
    interpolate_frames_hermite_into(frame_0, frame_1, frame_2, frame_3, alpha, &mut out);
    out
}

/// Blends `prev_frame` (weight `1.0 - alpha`) into `target` (weight `alpha`) in-place
/// with zero heap allocations using unit quaternion Slerp for rotations.
pub fn blend_frame_in_place(prev_frame: &AnimationFrame, alpha: f32, target: &mut AnimationFrame) {
    let alpha = alpha.clamp(0.0, 1.0);
    let inv_alpha = 1.0 - alpha;
    for (i, target_bone) in target.bones.iter_mut().enumerate() {
        let b_a = prev_frame.bones.get(i).and_then(|x| x.as_ref());
        let b_b = target_bone.take();
        *target_bone = match (b_a, b_b) {
            (Some(a), Some(b)) => {
                let rot = match (a.rotation, b.rotation) {
                    (Some(ra), Some(rb)) => Some(slerp_euler_zyx(ra, rb, alpha)),
                    (Some(ra), None) => Some(ra),
                    (None, Some(rb)) => Some(rb),
                    (None, None) => None,
                };
                let trans = match (a.translation, b.translation) {
                    (Some(ta), Some(tb)) => Some([
                        ta[0] * inv_alpha + tb[0] * alpha,
                        ta[1] * inv_alpha + tb[1] * alpha,
                        ta[2] * inv_alpha + tb[2] * alpha,
                    ]),
                    (Some(ta), None) => Some(ta),
                    (None, Some(tb)) => Some(tb),
                    (None, None) => None,
                };
                Some(BoneTransform {
                    rotation: rot,
                    translation: trans,
                    track_type: b.track_type,
                })
            }
            (Some(a), None) => Some(a.clone()),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        };
    }
}

/// Interpolates between two animation frames using unit quaternion Slerp for rotations
/// and linear interpolation for translations.
pub fn interpolate_frames(
    frame_a: &AnimationFrame,
    frame_b: &AnimationFrame,
    alpha: f32,
) -> AnimationFrame {
    let bone_count = frame_a.bones.len().max(frame_b.bones.len());
    let mut out = AnimationFrame {
        bones: Vec::with_capacity(bone_count),
    };
    interpolate_frames_into(frame_a, frame_b, alpha, &mut out);
    out
}

/// Row-major 4x4 matrix multiplication: $C = A \cdot B$.
#[inline]
pub fn mat4_mul(a: [[f32; 4]; 4], b: [[f32; 4]; 4]) -> [[f32; 4]; 4] {
    let mut out = [[0.0; 4]; 4];
    for r in 0..4 {
        for c in 0..4 {
            out[r][c] =
                a[r][0] * b[0][c] + a[r][1] * b[1][c] + a[r][2] * b[2][c] + a[r][3] * b[3][c];
        }
    }
    out
}

/// Inverts a standard affine $4 \times 4$ transformation matrix (row-major).
#[inline]
pub fn mat4_inverse(m: [[f32; 4]; 4]) -> [[f32; 4]; 4] {
    // Treat upper 3x3 as rotation/scale and translation column
    let a00 = m[0][0];
    let a01 = m[0][1];
    let a02 = m[0][2];
    let a10 = m[1][0];
    let a11 = m[1][1];
    let a12 = m[1][2];
    let a20 = m[2][0];
    let a21 = m[2][1];
    let a22 = m[2][2];

    let det3 = a00 * (a11 * a22 - a12 * a21) - a01 * (a10 * a22 - a12 * a20)
        + a02 * (a10 * a21 - a11 * a20);

    if det3.abs() < 1e-8 {
        // Fallback to identity if degenerate
        return [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
    }

    let inv_det = 1.0 / det3;
    let r00 = (a11 * a22 - a12 * a21) * inv_det;
    let r01 = (a02 * a21 - a01 * a22) * inv_det;
    let r02 = (a01 * a12 - a02 * a11) * inv_det;

    let r10 = (a12 * a20 - a10 * a22) * inv_det;
    let r11 = (a00 * a22 - a02 * a20) * inv_det;
    let r12 = (a02 * a10 - a00 * a12) * inv_det;

    let r20 = (a10 * a21 - a11 * a20) * inv_det;
    let r21 = (a01 * a20 - a00 * a21) * inv_det;
    let r22 = (a00 * a11 - a01 * a10) * inv_det;

    let tx = m[0][3];
    let ty = m[1][3];
    let tz = m[2][3];

    let itx = -(r00 * tx + r01 * ty + r02 * tz);
    let ity = -(r10 * tx + r11 * ty + r12 * tz);
    let itz = -(r20 * tx + r21 * ty + r22 * tz);

    [
        [r00, r01, r02, itx],
        [r10, r11, r12, ity],
        [r20, r21, r22, itz],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

/// Converts an IEEE 754 half-precision float (`u16`) to standard single-precision `f32`.
fn f16_to_f32(h: u16) -> f32 {
    let sign = ((h >> 15) & 1) as u32;
    let exp = ((h >> 10) & 0x1F) as u32;
    let mant = (h & 0x3FF) as u32;

    if exp == 0 {
        if mant == 0 {
            f32::from_bits(sign << 31)
        } else {
            // Denormalized half float
            let mut m = mant;
            let mut e = 113;
            while (m & 0x400) == 0 {
                m <<= 1;
                e -= 1;
            }
            m &= 0x3FF;
            f32::from_bits((sign << 31) | (e << 23) | (m << 13))
        }
    } else if exp == 31 {
        // Infinity or NaN
        f32::from_bits((sign << 31) | (0xFF << 23) | (mant << 13))
    } else {
        // Normalized half float
        f32::from_bits((sign << 31) | ((exp + 112) << 23) | (mant << 13))
    }
}
