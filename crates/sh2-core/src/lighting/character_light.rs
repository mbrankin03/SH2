//! Character dynamic lighting evaluation (`Chacter_Draw/light_n.c`, `sh2pc.exe` `0x508a90`,
//! `0x50ce10`, `0x5091c0`, ambient blend formula at `0x8a2ed8`).
//!
//! Builds parallel, spot, point, fake point, and fake spot lights around each character,
//! selects the strongest contributors (`0x50ce10` / `0x5091c0`), and evaluates Team Silent's
//! ambient/direct blend:
//! `final_light = lights * 0.7 + (2.1 * ambient + 0.3 * flashlight_level) * 0.3`.

use glam::Vec3;

use super::draw_env::DrawEnvData;

/// Direct lighting weight in Team Silent's character lighting formula (`sh2pc.exe` `0x8a2ed8`).
pub const CHAR_LIGHT_DIRECT_WEIGHT: f32 = 0.7;

/// Ambient lighting weight in Team Silent's character lighting formula (`sh2pc.exe` `0x8a2ed8`).
pub const CHAR_LIGHT_AMBIENT_WEIGHT: f32 = 0.3;

/// Room `Ambient` multiplier inside the ambient term (`2.1 * Ambient`, `sh2pc.exe` `0x8a2ed8`).
pub const CHAR_LIGHT_ROOM_AMBIENT_SCALE: f32 = 2.1;

/// Flashlight level multiplier inside the ambient term (`0.3 * flashlight_level`, `sh2pc.exe` `0x8a2ed8`).
pub const CHAR_LIGHT_FLASHLIGHT_AMBIENT_SCALE: f32 = 0.3;

/// Maximum number of active lights selected per character (`sh2pc.exe` `0x50ce10`).
pub const MAX_CHARACTER_ACTIVE_LIGHTS: usize = 3;

/// Kind of candidate light evaluated by `light_n.c` (`sh2pc.exe` `0x508a90`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CharacterLightKind {
    /// Infinite parallel directional light from `DrawEnvData`.
    Parallel,
    /// Dynamic spotlight (James's flashlight or cutscene spot).
    Spot,
    /// Room point light from `DrawEnvData` (`+0xb0..+0x1af`).
    Point,
    /// Fake point light (`FakeLightWork` @ `0x00489b80` in `SLUS_202.28`).
    FakePoint,
    /// Fake spotlight (`Get_NowFakeSpotNum` @ `0x00250e80` in `SLUS_202.28`).
    FakeSpot,
}

/// Candidate light source for character shading (`sh2pc.exe` `0x508a90`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CharacterLightCandidate {
    /// Light source category.
    pub kind: CharacterLightKind,
    /// World-space position in PS2 mm (unused for [`CharacterLightKind::Parallel`]).
    pub position: Vec3,
    /// Unit light direction in PS2 world space (for `Parallel`, `Spot`, `FakeSpot`).
    pub direction: Vec3,
    /// Linear RGB color.
    pub color: Vec3,
    /// Near attenuation distance in PS2 mm.
    pub near_dist: f32,
    /// Far attenuation cutoff in PS2 mm.
    pub far_dist: f32,
    /// Outer spotlight cone cosine (`-1.0` for non-spot lights).
    pub outer_cos: f32,
}

/// Selected light entry with computed effective intensity at the character's position.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SelectedCharacterLight {
    /// Original light category.
    pub kind: CharacterLightKind,
    /// Unit direction from character toward the light source.
    pub to_light: Vec3,
    /// Attenuated linear RGB contribution.
    pub color: Vec3,
    /// Scalar strength used to rank top contributors (`0x50ce10`).
    pub strength: f32,
}

/// Resolved per-character lighting snapshot ready for GPU uniform upload.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CharacterLightingResult {
    /// Up to 3 strongest active lights sorted descending by strength.
    pub lights: [Option<SelectedCharacterLight>; MAX_CHARACTER_ACTIVE_LIGHTS],
    /// Combined ambient RGB term `(2.1 * Ambient + 0.3 * flashlight_level) * 0.3` (`0x8a2ed8`).
    pub ambient_term: Vec3,
    /// Total scalar direct-light luminance at the character center.
    pub total_direct_rgb: Vec3,
    /// Combined final diffuse illumination `lights * 0.7 + (2.1 * Ambient + 0.3 * flashlight_level) * 0.3`.
    pub combined_rgb: Vec3,
}

/// Evaluates Team Silent's character lighting formula (`sh2pc.exe` `0x508a90`, `0x50ce10`,
/// `0x5091c0`, `0x8a2ed8`) at `char_pos` (PS2 world mm).
///
/// - `draw_env`: Active room `DrawEnvData`.
/// - `extra_lights`: Dynamic flashlight / fake spot / fake point candidates.
/// - `flashlight_level`: `1.0` when flashlight is on, or `env_flash_off_level(...)` (`1.0` / `0.3`) when off.
/// - `enemy_reveal_scale`: Optional multiplier for `EnemyRevealLighting` (`1.0` default; `0.065` -> `0.7` in Rm 205 / Demo 6).
#[must_use]
pub fn evaluate_character_lighting(
    char_pos: Vec3,
    draw_env: &DrawEnvData,
    extra_lights: &[CharacterLightCandidate],
    flashlight_level: f32,
    enemy_reveal_scale: f32,
) -> CharacterLightingResult {
    let mut scored: Vec<SelectedCharacterLight> = Vec::with_capacity(3 + 4 + extra_lights.len());

    // 1. Directional lights from DrawEnvData (+0x10..+0x6f)
    for d in &draw_env.directional_lights {
        let col = Vec3::from_array(d.color);
        let lum = col.x.max(col.y).max(col.z);
        if lum > 1e-4 {
            let dir = Vec3::from_array(d.direction).normalize_or(Vec3::Y);
            scored.push(SelectedCharacterLight {
                kind: CharacterLightKind::Parallel,
                to_light: -dir,
                color: col,
                strength: lum,
            });
        }
    }

    // 2. Room point lights from DrawEnvData (+0xb0..+0x1af)
    for pt in &draw_env.point_lights {
        if !pt.is_active() {
            continue;
        }
        let candidate = CharacterLightCandidate {
            kind: CharacterLightKind::Point,
            position: Vec3::from_array(pt.position),
            direction: Vec3::Y,
            color: Vec3::from_array(pt.color),
            near_dist: pt.near_dist,
            far_dist: pt.far_dist,
            outer_cos: -1.0,
        };
        if let Some(sel) = score_candidate(char_pos, &candidate) {
            scored.push(sel);
        }
    }

    // 3. Dynamic flashlight / fake lights
    for cand in extra_lights {
        if let Some(sel) = score_candidate(char_pos, cand) {
            scored.push(sel);
        }
    }

    // Select strongest lights (`0x50ce10` / `0x5091c0`)
    scored.sort_by(|a, b| b.strength.total_cmp(&a.strength));

    let mut lights = [None; MAX_CHARACTER_ACTIVE_LIGHTS];
    let mut total_direct_rgb = Vec3::ZERO;
    for (slot, item) in lights
        .iter_mut()
        .zip(scored.into_iter().take(MAX_CHARACTER_ACTIVE_LIGHTS))
    {
        total_direct_rgb += item.color;
        *slot = Some(item);
    }

    // Team Silent formula (`sh2pc.exe` `0x8a2ed8`):
    // `lights * 0.7 + (2.1 * Ambient + 0.3 * flashlight_level) * 0.3`
    let room_ambient = Vec3::from_array(draw_env.ambient);
    let fl_clamped = flashlight_level.clamp(0.0, 1.0);
    let ambient_term = (room_ambient * CHAR_LIGHT_ROOM_AMBIENT_SCALE
        + Vec3::splat(CHAR_LIGHT_FLASHLIGHT_AMBIENT_SCALE * fl_clamped))
        * CHAR_LIGHT_AMBIENT_WEIGHT;

    let reveal = enemy_reveal_scale.clamp(0.0, 2.0);
    let combined_rgb = (total_direct_rgb * CHAR_LIGHT_DIRECT_WEIGHT + ambient_term) * reveal;

    CharacterLightingResult {
        lights,
        ambient_term: ambient_term * reveal,
        total_direct_rgb: total_direct_rgb * reveal,
        combined_rgb,
    }
}

fn score_candidate(
    char_pos: Vec3,
    cand: &CharacterLightCandidate,
) -> Option<SelectedCharacterLight> {
    match cand.kind {
        CharacterLightKind::Parallel => {
            let lum = cand.color.x.max(cand.color.y).max(cand.color.z);
            if lum <= 1e-4 {
                return None;
            }
            Some(SelectedCharacterLight {
                kind: cand.kind,
                to_light: -cand.direction.normalize_or(Vec3::Y),
                color: cand.color,
                strength: lum,
            })
        }
        CharacterLightKind::Point | CharacterLightKind::FakePoint => {
            let delta = cand.position - char_pos;
            let dist = delta.length();
            if dist >= cand.far_dist || cand.far_dist <= 1.0 {
                return None;
            }
            let atten = linear_distance_attenuation(dist, cand.near_dist, cand.far_dist);
            let attenuated = cand.color * atten;
            let strength = attenuated.x.max(attenuated.y).max(attenuated.z);
            if strength <= 1e-4 {
                return None;
            }
            Some(SelectedCharacterLight {
                kind: cand.kind,
                to_light: delta.normalize_or(Vec3::Y),
                color: attenuated,
                strength,
            })
        }
        CharacterLightKind::Spot | CharacterLightKind::FakeSpot => {
            let from_light = char_pos - cand.position;
            let dist = from_light.length();
            if dist >= cand.far_dist || cand.far_dist <= 1.0 {
                return None;
            }
            let l_dir = from_light.normalize_or(Vec3::NEG_Z);
            let spot_dir = cand.direction.normalize_or(Vec3::NEG_Z);
            let cos_theta = l_dir.dot(spot_dir);
            if cos_theta < cand.outer_cos {
                return None;
            }
            let cone =
                ((cos_theta - cand.outer_cos) / (1.0 - cand.outer_cos).max(1e-4)).clamp(0.0, 1.0);
            let atten = linear_distance_attenuation(dist, cand.near_dist, cand.far_dist) * cone;
            let attenuated = cand.color * atten;
            let strength = attenuated.x.max(attenuated.y).max(attenuated.z);
            if strength <= 1e-4 {
                return None;
            }
            Some(SelectedCharacterLight {
                kind: cand.kind,
                to_light: -l_dir,
                color: attenuated,
                strength,
            })
        }
    }
}

#[inline]
fn linear_distance_attenuation(dist: f32, near: f32, far: f32) -> f32 {
    if dist <= near {
        1.0
    } else if dist >= far {
        0.0
    } else {
        ((far - dist) / (far - near).max(1.0)).clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lighting::draw_env::lookup_draw_env;

    #[test]
    fn formula_0x8a2ed8_matches_exact_weights() {
        let ob = lookup_draw_env("OB", 0).expect("OB");
        let res = evaluate_character_lighting(Vec3::ZERO, ob, &[], 1.0, 1.0);
        // Expected ambient term = (2.1 * 0.9 + 0.3 * 1.0) * 0.3 = (1.89 + 0.3) * 0.3 = 0.657
        assert!((res.ambient_term.x - 0.657).abs() < 1e-3);
        assert!(res.lights[0].is_some());
    }

    #[test]
    fn strongest_lights_selected_when_more_than_three_present() {
        let ap = lookup_draw_env("AP", 12).expect("AP");
        let strong_spot = CharacterLightCandidate {
            kind: CharacterLightKind::Spot,
            position: Vec3::new(0.0, -1000.0, 0.0),
            direction: Vec3::new(0.0, 0.0, 1.0),
            color: Vec3::splat(7.0),
            near_dist: 1500.0,
            far_dist: 8500.0,
            outer_cos: 0.707,
        };
        let res = evaluate_character_lighting(
            Vec3::new(0.0, -1000.0, 1000.0),
            ap,
            &[strong_spot],
            1.0,
            1.0,
        );
        let top = res.lights[0].expect("strongest light");
        assert_eq!(top.kind, CharacterLightKind::Spot);
        assert!(top.strength > 6.0);
    }
}
