//! Keyframe animation parsing and sampling for `.glb` models.

use super::accessors::{parse_scalar_f32_accessor, parse_vec3_accessor, parse_vec4_accessor};
use super::raw::RawGltf;
use crate::error::AssetError;
use std::collections::HashMap;

/// Supported glTF 2.0 sampler interpolation modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GlbInterpolation {
    /// Linear interpolation / spherical linear interpolation.
    #[default]
    Linear,
    /// Constant step hold until next keyframe.
    Step,
}

/// Keyframe channel targeting a single scene node.
#[derive(Debug, Clone)]
pub struct GlbAnimTrack {
    /// Target node index in [`super::GlbModel::nodes`].
    pub target_node: usize,
    /// Translation interpolation mode.
    pub translation_interp: GlbInterpolation,
    /// Translation keyframes `(time_sec, [x, y, z])`.
    pub translations: Vec<(f32, [f32; 3])>,
    /// Rotation interpolation mode.
    pub rotation_interp: GlbInterpolation,
    /// Rotation keyframes `(time_sec, [x, y, z, w])`.
    pub rotations: Vec<(f32, [f32; 4])>,
    /// Scale interpolation mode.
    pub scale_interp: GlbInterpolation,
    /// Scale keyframes `(time_sec, [sx, sy, sz])`.
    pub scales: Vec<(f32, [f32; 3])>,
}

fn parse_interpolation(raw: Option<&str>) -> Result<GlbInterpolation, AssetError> {
    match raw.unwrap_or("LINEAR") {
        "LINEAR" => Ok(GlbInterpolation::Linear),
        "STEP" => Ok(GlbInterpolation::Step),
        other => Err(AssetError::UnsupportedFormat(format!(
            "Unsupported glTF animation interpolation {other}"
        ))),
    }
}

pub(super) fn parse_animations(
    raw: &RawGltf,
    bin: &[u8],
) -> Result<(Vec<GlbAnimTrack>, f32), AssetError> {
    let Some(anim) = raw.animations.first() else {
        return Ok((Vec::new(), 0.0));
    };
    let mut by_node: HashMap<usize, GlbAnimTrack> = HashMap::new();
    let mut max_time = 0.0f32;

    for ch in &anim.channels {
        let Some(node_idx) = ch.target.node else {
            continue;
        };
        let sampler = anim
            .samplers
            .get(ch.sampler)
            .ok_or_else(|| AssetError::InvalidData("Missing animation sampler".into()))?;
        let interp = parse_interpolation(sampler.interpolation.as_deref())?;
        let times = parse_scalar_f32_accessor(raw, bin, sampler.input)?;
        for &t in &times {
            max_time = max_time.max(t);
        }

        let track = by_node.entry(node_idx).or_insert_with(|| GlbAnimTrack {
            target_node: node_idx,
            translation_interp: GlbInterpolation::Linear,
            translations: Vec::new(),
            rotation_interp: GlbInterpolation::Linear,
            rotations: Vec::new(),
            scale_interp: GlbInterpolation::Linear,
            scales: Vec::new(),
        });

        match ch.target.path.as_str() {
            "translation" => {
                let vals = parse_vec3_accessor(raw, bin, sampler.output)?;
                if times.len() != vals.len() {
                    return Err(AssetError::InvalidData(format!(
                        "Animation translation keyframe count mismatch: {} times vs {} values",
                        times.len(),
                        vals.len()
                    )));
                }
                track.translation_interp = interp;
                track.translations = times.into_iter().zip(vals).collect();
            }
            "rotation" => {
                let vals = parse_vec4_accessor(raw, bin, sampler.output)?;
                if times.len() != vals.len() {
                    return Err(AssetError::InvalidData(format!(
                        "Animation rotation keyframe count mismatch: {} times vs {} values",
                        times.len(),
                        vals.len()
                    )));
                }
                track.rotation_interp = interp;
                track.rotations = times.into_iter().zip(vals).collect();
            }
            "scale" => {
                let vals = parse_vec3_accessor(raw, bin, sampler.output)?;
                if times.len() != vals.len() {
                    return Err(AssetError::InvalidData(format!(
                        "Animation scale keyframe count mismatch: {} times vs {} values",
                        times.len(),
                        vals.len()
                    )));
                }
                track.scale_interp = interp;
                track.scales = times.into_iter().zip(vals).collect();
            }
            _ => {}
        }
    }

    let mut tracks: Vec<GlbAnimTrack> = by_node.into_values().collect();
    tracks.sort_by_key(|t| t.target_node);
    Ok((tracks, max_time))
}

pub(super) fn sample_vec3(
    frames: &[(f32, [f32; 3])],
    t: f32,
    interp: GlbInterpolation,
) -> [f32; 3] {
    if frames.len() == 1 || t <= frames[0].0 {
        return frames[0].1;
    }
    for w in frames.windows(2) {
        let (t0, v0) = w[0];
        let (t1, v1) = w[1];
        if t <= t1 {
            if interp == GlbInterpolation::Step {
                return v0;
            }
            let span = (t1 - t0).max(f32::EPSILON);
            let a = ((t - t0) / span).clamp(0.0, 1.0);
            return [
                v0[0] + (v1[0] - v0[0]) * a,
                v0[1] + (v1[1] - v0[1]) * a,
                v0[2] + (v1[2] - v0[2]) * a,
            ];
        }
    }
    frames.last().map(|f| f.1).unwrap_or([0.0, 0.0, 0.0])
}

pub(super) fn sample_quat(
    frames: &[(f32, [f32; 4])],
    t: f32,
    interp: GlbInterpolation,
) -> [f32; 4] {
    if frames.len() == 1 || t <= frames[0].0 {
        return frames[0].1;
    }
    for w in frames.windows(2) {
        let (t0, q0) = w[0];
        let (t1, mut q1) = w[1];
        if t <= t1 {
            if interp == GlbInterpolation::Step {
                return q0;
            }
            let span = (t1 - t0).max(f32::EPSILON);
            let a = ((t - t0) / span).clamp(0.0, 1.0);
            let mut dot = q0[0] * q1[0] + q0[1] * q1[1] + q0[2] * q1[2] + q0[3] * q1[3];
            if dot < 0.0 {
                q1 = [-q1[0], -q1[1], -q1[2], -q1[3]];
                dot = -dot;
            }
            let out = if dot > 0.9995 {
                [
                    q0[0] + (q1[0] - q0[0]) * a,
                    q0[1] + (q1[1] - q0[1]) * a,
                    q0[2] + (q1[2] - q0[2]) * a,
                    q0[3] + (q1[3] - q0[3]) * a,
                ]
            } else {
                let theta0 = dot.clamp(-1.0, 1.0).acos();
                let theta = theta0 * a;
                let s0 = (theta0 - theta).sin() / theta0.sin();
                let s1 = theta.sin() / theta0.sin();
                [
                    q0[0] * s0 + q1[0] * s1,
                    q0[1] * s0 + q1[1] * s1,
                    q0[2] * s0 + q1[2] * s1,
                    q0[3] * s0 + q1[3] * s1,
                ]
            };
            let len =
                (out[0] * out[0] + out[1] * out[1] + out[2] * out[2] + out[3] * out[3]).sqrt();
            if len > f32::EPSILON {
                return [out[0] / len, out[1] / len, out[2] / len, out[3] / len];
            }
            return [0.0, 0.0, 0.0, 1.0];
        }
    }
    frames.last().map(|f| f.1).unwrap_or([0.0, 0.0, 0.0, 1.0])
}
