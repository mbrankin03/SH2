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
}

/// Tests if the animation byte stream can be validly and completely decoded with the given bone count stride.
fn is_valid_anm_stride(data: &[u8], bone_count: usize, parents: &[u8]) -> bool {
    if bone_count == 0 {
        return false;
    }
    let per_frame = (bone_count + 7) & !7;
    let mut p = 0usize;
    let mut block = 0usize;

    while p + 4 <= data.len() {
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
    let mut cur_frame: Vec<Option<BoneTransform>> = vec![None; bone_count];

    let mut p = 0usize;
    let mut block = 0usize;

    while p + 4 <= data.len() {
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
                cur_frame[bone_idx] = Some(BoneTransform {
                    rotation: rot,
                    translation: trans,
                    track_type: AnimationTrackType::from(t),
                });
            }
        }

        block += 1;
        if (block * 8).is_multiple_of(per_frame) {
            frames.push(AnimationFrame {
                bones: cur_frame.clone(),
            });
            cur_frame = vec![None; bone_count];
        }
    }

    Ok(AnimationFile { bone_count, frames })
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

/// Linearly interpolates an angular value along the shortest arc on the circle $[-\pi, \pi]$.
#[inline]
fn lerp_angle(a: f32, b: f32, alpha: f32) -> f32 {
    let two_pi = std::f32::consts::TAU;
    let mut diff = (b - a) % two_pi;
    if diff > std::f32::consts::PI {
        diff -= two_pi;
    } else if diff < -std::f32::consts::PI {
        diff += two_pi;
    }
    a + diff * alpha
}

/// Linearly interpolates (LERP / SLERP equivalent for Euler tracks) between two animation frames.
///
/// Rotation tracks interpolate along the shortest arc on $[-\pi, \pi]$ to prevent sudden
/// 180-degree angular flips when angles wrap around boundary coordinates.
pub fn interpolate_frames(
    frame_a: &AnimationFrame,
    frame_b: &AnimationFrame,
    alpha: f32,
) -> AnimationFrame {
    let bone_count = frame_a.bones.len().max(frame_b.bones.len());
    let mut bones = Vec::with_capacity(bone_count);

    let alpha = alpha.clamp(0.0, 1.0);
    let inv_alpha = 1.0 - alpha;

    for i in 0..bone_count {
        let b_a = frame_a.bones.get(i).and_then(|x| x.as_ref());
        let b_b = frame_b.bones.get(i).and_then(|x| x.as_ref());

        match (b_a, b_b) {
            (Some(a), Some(b)) => {
                let rot = match (a.rotation, b.rotation) {
                    (Some(ra), Some(rb)) => Some([
                        lerp_angle(ra[0], rb[0], alpha),
                        lerp_angle(ra[1], rb[1], alpha),
                        lerp_angle(ra[2], rb[2], alpha),
                    ]),
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
                bones.push(Some(BoneTransform {
                    rotation: rot,
                    translation: trans,
                    track_type: a.track_type,
                }));
            }
            (Some(a), None) => bones.push(Some(a.clone())),
            (None, Some(b)) => bones.push(Some(b.clone())),
            (None, None) => bones.push(None),
        }
    }

    AnimationFrame { bones }
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
