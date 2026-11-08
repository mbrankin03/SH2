//! Dynamic channel spatialization and environmental reverb modeling (`SH2-55`).
//!
//! Evaluates room reverb (`feedback`, `damping`, `wet_mix`) and stereo spatial spread
//! directly from active `.cam` camera volume properties (`rd_type`, `area_size_type`, and
//! 3D bounding-box geometry) without requiring pre-processed audio files.

use glam::Vec3;
use sh2_assets::{AreaSizeType, CamFile, RoadRecord, RoadType};

/// Environmental reverb preset matching original PS2 / PC acoustics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EnvironmentalPreset {
    /// Dry direct sound with no reverb (default / outdoor open area).
    #[default]
    Off,
    /// Thick foggy exterior with soft low-frequency damping.
    FoggyStreets,
    /// Enclosed interior room with short reverberation.
    InteriorRoom,
    /// Resonant metallic hallway or apartment corridor.
    Corridor,
    /// Wet cavernous echo (e.g. Prison basement, labyrinth).
    CavernousEcho,
}

impl EnvironmentalPreset {
    /// Returns the corresponding `(feedback, damping, mix)` parameters for Kira reverb.
    pub fn reverb_params(&self) -> Option<(f64, f64, f64)> {
        match self {
            Self::Off => None,
            Self::FoggyStreets => Some((0.35, 0.70, 0.20)),
            Self::InteriorRoom => Some((0.55, 0.40, 0.30)),
            Self::Corridor => Some((0.75, 0.25, 0.45)),
            Self::CavernousEcho => Some((0.90, 0.15, 0.60)),
        }
    }
}

/// Evaluated dynamic room acoustics profile derived from `.cam` [`RoadRecord`] volume properties (`SH2-55`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RoomAcousticsProfile {
    /// Closest discrete [`EnvironmentalPreset`] classification.
    pub preset: EnvironmentalPreset,
    /// Reverb feedback decay factor (`0.0..=0.95`).
    pub feedback: f64,
    /// High-frequency reverb damping (`0.0..=1.0`, higher = warmer/darker reflections).
    pub damping: f64,
    /// Wet/dry reverb mix ratio (`0.0..=1.0`).
    pub wet_mix: f64,
    /// Dynamic stereo channel spatialization width multiplier (`0.5..=1.25`: narrower in tight corridors, wider in open spaces).
    pub spatial_spread: f32,
    /// Horizontal aspect ratio ($\max(\Delta X, \Delta Z) / \min(\Delta X, \Delta Z)$) of the active camera volume.
    pub horizontal_aspect_ratio: f32,
}

impl RoomAcousticsProfile {
    /// Creates an acoustics profile directly from a discrete [`EnvironmentalPreset`].
    pub fn from_preset(preset: EnvironmentalPreset) -> Self {
        match preset.reverb_params() {
            Some((feedback, damping, wet_mix)) => {
                let spatial_spread = match preset {
                    EnvironmentalPreset::Off => 1.0,
                    EnvironmentalPreset::FoggyStreets => 1.15,
                    EnvironmentalPreset::InteriorRoom => 0.90,
                    EnvironmentalPreset::Corridor => 0.70,
                    EnvironmentalPreset::CavernousEcho => 1.20,
                };
                Self {
                    preset,
                    feedback,
                    damping,
                    wet_mix,
                    spatial_spread,
                    horizontal_aspect_ratio: 1.0,
                }
            }
            None => Self {
                preset: EnvironmentalPreset::Off,
                feedback: 0.0,
                damping: 0.5,
                wet_mix: 0.0,
                spatial_spread: 1.0,
                horizontal_aspect_ratio: 1.0,
            },
        }
    }
}

/// Evaluates dynamic room acoustics (`preset`, `feedback`, `damping`, `wet_mix`, `spatial_spread`)
/// from an active `.cam` [`RoadRecord`] (`rd_type`, `area_size_type`, and 3D bounding-box geometry)
/// and whether the stage is an enclosed interior map (`SH2-55`).
///
/// # Acoustic Rules
/// 1. **`AreaSizeType::Outdoor`**:
///    - Models open foggy exterior streets (`EnvironmentalPreset::FoggyStreets`), with soft high-frequency
///      fog absorption (`damping = 0.70`), gentle tail (`feedback = 0.35`, `wet_mix = 0.20`), and wide stereo field (`1.15`).
/// 2. **Elongated Corridor Geometry (`aspect_ratio >= 2.75`) or `AreaSizeType::Tiny` with `RoadType::Road` / `RoadPrioHigh`**:
///    - Models narrow apartment/hospital/prison hallways (`EnvironmentalPreset::Corridor`), with metallic/reflective
///      flutter (`feedback = 0.72..0.78`, `damping = 0.25`, `wet_mix = 0.42..0.48`) and focused stereo width (`0.70`).
/// 3. **Large/Deep Interior Chambers (`AreaSizeType::Wide` in an interior map, or `RoadType::Effect` / `RoadType::Event` with high ceiling)**:
///    - Models cavernous spaces like Toluca Prison, Labyrinth, and Boiler Room (`EnvironmentalPreset::CavernousEcho`,
///      `feedback = 0.85..0.90`, `damping = 0.15..0.20`, `wet_mix = 0.55..0.60`, `spatial_spread = 1.20`).
/// 4. **Standard Enclosed Rooms (`AreaSizeType::Small` / `Tiny` in an interior map)**:
///    - Models enclosed rooms (`EnvironmentalPreset::InteriorRoom`, `feedback = 0.55`, `damping = 0.40`, `wet_mix = 0.30`).
pub fn evaluate_room_acoustics(
    record: Option<&RoadRecord>,
    is_interior_map: bool,
) -> RoomAcousticsProfile {
    let Some(rec) = record else {
        return if is_interior_map {
            RoomAcousticsProfile::from_preset(EnvironmentalPreset::InteriorRoom)
        } else {
            RoomAcousticsProfile::from_preset(EnvironmentalPreset::FoggyStreets)
        };
    };

    let (min_x, max_x) = rec.lim_rd.x_bounds();
    let (min_z, max_z) = rec.lim_rd.z_bounds();
    let (min_y, max_y) = rec.lim_rd.y_bounds();

    let dx = (max_x - min_x).abs().max(100.0);
    let dz = (max_z - min_z).abs().max(100.0);
    let dy = (max_y - min_y).abs().max(100.0);

    let major = dx.max(dz);
    let minor = dx.min(dz).max(100.0);
    let aspect = (major / minor).clamp(1.0, 20.0);

    // 1. Outdoor volumes always use open foggy street acoustics
    if rec.area_size_type == AreaSizeType::Outdoor || !is_interior_map {
        let mut prof = RoomAcousticsProfile::from_preset(EnvironmentalPreset::FoggyStreets);
        prof.horizontal_aspect_ratio = aspect;
        // Narrow outdoor alleys get slightly more early reflection feedback than wide plazas
        if aspect >= 3.0 {
            prof.feedback = 0.42;
            prof.wet_mix = 0.25;
            prof.spatial_spread = 1.05;
        }
        return prof;
    }

    // 2. Interior Map Evaluation: distinguish Cavernous, Corridor, and InteriorRoom
    let is_high_ceiling_or_effect =
        dy >= 4500.0 || matches!(rec.rd_type, RoadType::Effect | RoadType::Event);

    if rec.area_size_type == AreaSizeType::Wide || (is_high_ceiling_or_effect && major >= 6000.0) {
        let mut prof = RoomAcousticsProfile::from_preset(EnvironmentalPreset::CavernousEcho);
        prof.horizontal_aspect_ratio = aspect;
        return prof;
    }

    // Elongated hallway (`aspect >= 2.75`) or `Tiny` road rail (`Road` / `RoadPrioHigh` / `RoadPrioLow`)
    let is_corridor = aspect >= 2.75
        || (rec.area_size_type == AreaSizeType::Tiny
            && matches!(
                rec.rd_type,
                RoadType::Road | RoadType::RoadPrioHigh | RoadType::RoadPrioLow
            )
            && aspect >= 1.8);

    if is_corridor {
        let mut prof = RoomAcousticsProfile::from_preset(EnvironmentalPreset::Corridor);
        prof.horizontal_aspect_ratio = aspect;
        // Scale feedback slightly with corridor elongation (`0.72..=0.80`)
        let t = ((aspect - 1.8) / 6.0).clamp(0.0, 1.0) as f64;
        prof.feedback = 0.72 + 0.08 * t;
        prof.wet_mix = 0.42 + 0.06 * t;
        return prof;
    }

    let mut prof = RoomAcousticsProfile::from_preset(EnvironmentalPreset::InteriorRoom);
    prof.horizontal_aspect_ratio = aspect;
    if matches!(rec.rd_type, RoadType::SelfViewOnly) {
        // Closet / tight first-person inspection nook: drier, intimate reflections
        prof.feedback = 0.42;
        prof.damping = 0.55;
        prof.wet_mix = 0.22;
        prof.spatial_spread = 0.75;
    }
    prof
}

/// Evaluates the active camera road volume in `cam` for `player_pos` and returns its [`RoomAcousticsProfile`].
pub fn evaluate_room_acoustics_for_pos(
    cam: &CamFile,
    player_pos: Vec3,
    is_interior_map: bool,
) -> RoomAcousticsProfile {
    let active_rec = cam.find_active_record([player_pos.x, player_pos.y, player_pos.z], 50.0);
    evaluate_room_acoustics(active_rec, is_interior_map)
}
