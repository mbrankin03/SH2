//! PS2 shadow draw order (`Check_DrawOrder`), map-specific shadow light overrides
//! (`stencil_shadow_main`), James self-shadow part exclusion (`sh2shd_del_jms_upper_body`,
//! `sh2shd_del_jms_head`), and height-based drop-shadow alpha (`sh2shd_get_height_for_alpha`).
//!
//! Ground-truth references (`SLUS_202.28`):
//! - `Check_DrawOrder` (`0x001a08a0`, `sh2gfw_drawloop_main.c`): returns `0` (`ShadowBeforeCharacter`)
//!   only when `DramaDemoNumber() == 0` and `NowJmsMapId` is `0x000a007d` or `0x00090016`; otherwise `1`.
//! - `stencil_shadow_main` (`0x001a6600`, `sh2shd/sh2shd_stencil_shadow_main.c`):
//!   - Stage `0x0c` (`12`), Room `0x5b` (`91`): parallel light type `4`, dir `(0x3d95bca0, 0x3f2f0154, 0x3f39e673)`.
//!   - Stage `0x09` (`9`), Room `0x2a` (`42`) with James flashlight off: spot light type `0`,
//!     pos `(0xc763ee00, 0xc42f0000, 0x46969600, 1.0)`, dir `(0xbeb95810, 0x3f44dd2f, 0x3ea6e979, 0.0)`.
//! - `sh2shd_del_jms_upper_body` (`0x001a5e60`) & `sh2shd_del_jms_head` (`0x001a5f60`, `sh2shd/sh2shd_shadow_model.c`).
//! - `sh2shd_get_height_for_alpha` (`0x0018dc50` + caller `0x0018db00`, `sh2shd/sh2shd_char_jms.c`):
//!   700.0 mm vertical fade window (`high = low - 700.0`) with character-specific head/foot bone IDs.

use glam::Vec3;

/// Map IDs where `Check_DrawOrder` (`0x001a08a0`) draws shadows BEFORE characters during gameplay
/// (`DramaDemoNumber() == 0`).
pub const DRAW_ORDER_SHADOW_FIRST_MAPS: [u32; 2] = [0x000a_007d, 0x0009_0016];

/// James `.kg1` part IDs excluded by `sh2shd_del_jms_upper_body` (`0x001a5e60`, `sh2shd_shadow_model.c`).
pub const JAMES_UPPER_BODY_EXCLUDED_PARTS: [u16; 10] =
    [1, 3, 0x1c, 0x1b, 0x22, 0x21, 0x23, 0x28, 8, 0x0f];

/// James `.kg1` part IDs excluded by `sh2shd_del_jms_head` (`0x001a5f60`, `sh2shd_shadow_model.c`).
pub const JAMES_HEAD_EXCLUDED_PARTS: [u16; 2] = [8, 0x0f];

/// Vertical drop-shadow fade window in PS2 world mm (`sh2shd_char_jms.c` line 1083: `fStack_8 = fStack_4 - 700.0`).
pub const DROP_SHADOW_HEIGHT_WINDOW_MM: f32 = 700.0;

/// Character vs. shadow render order returned by `Check_DrawOrder` (`0x001a08a0`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShadowDrawOrder {
    /// Return value `0`: draw shadow-only character pass + `sh2gfw_ShadowControl_Main` BEFORE
    /// main character pass (`Draw_Character_And_Shadow` `0x001a0500`).
    ShadowBeforeCharacter = 0,
    /// Return value `1`: draw characters first, then `sh2gfw_ShadowControl_Main`.
    CharacterBeforeShadow = 1,
}

/// Evaluates `Check_DrawOrder` (`SLUS_202.28` `0x001a08a0`, `sh2gfw_drawloop_main.c`).
#[must_use]
pub const fn check_draw_order(now_map_id: u32, drama_demo_number: u32) -> ShadowDrawOrder {
    if drama_demo_number == 0
        && (now_map_id == DRAW_ORDER_SHADOW_FIRST_MAPS[0]
            || now_map_id == DRAW_ORDER_SHADOW_FIRST_MAPS[1])
    {
        ShadowDrawOrder::ShadowBeforeCharacter
    } else {
        ShadowDrawOrder::CharacterBeforeShadow
    }
}

/// Map-specific shadow light override returned by `stencil_shadow_main` (`0x001a6600`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ShadowLightOverride {
    /// Stage `0x0c` (`12`), Room `0x5b` (`91`): parallel light (`uVar3 = 4`) with exact IEEE-754 direction
    /// `(0x3d95bca0, 0x3f2f0154, 0x3f39e673)`.
    Parallel { light_type: u16, direction: Vec3 },
    /// Stage `0x09` (`9`), Room `0x2a` (`42`) when James flashlight is OFF: spotlight (`uVar3 = 0`)
    /// with exact IEEE-754 position `(0xc763ee00, 0xc42f0000, 0x46969600)` and direction
    /// `(0xbeb95810, 0x3f44dd2f, 0x3ea6e979)`.
    Spotlight {
        light_type: u16,
        position: Vec3,
        direction: Vec3,
    },
}

/// Evaluates map-specific shadow light overrides from `stencil_shadow_main`
/// (`SLUS_202.28` `0x001a6600`, `sh2shd/sh2shd_stencil_shadow_main.c`).
#[must_use]
pub fn map_shadow_light_override(
    stage_id: u32,
    room_id: u32,
    jms_spot_on: bool,
) -> Option<ShadowLightOverride> {
    if stage_id == 0x0c && room_id == 0x5b {
        Some(ShadowLightOverride::Parallel {
            light_type: 4,
            direction: Vec3::new(
                f32::from_bits(0x3d95_bca0),
                f32::from_bits(0x3f2f_0154),
                f32::from_bits(0x3f39_e673),
            ),
        })
    } else if stage_id == 9 && room_id == 0x2a && !jms_spot_on {
        Some(ShadowLightOverride::Spotlight {
            light_type: 0,
            position: Vec3::new(
                f32::from_bits(0xc763_ee00),
                f32::from_bits(0xc42f_0000),
                f32::from_bits(0x4696_9600),
            ),
            direction: Vec3::new(
                f32::from_bits(0xbeb9_5810),
                f32::from_bits(0x3f44_dd2f),
                f32::from_bits(0x3ea6_e979),
            ),
        })
    } else {
        None
    }
}

/// James self-shadow exclusion mode (`make_chara_reftag_pool_and_kick_packet_for_self` `0x001a4400`,
/// `sh2shd_del_jms_upper_body` `0x001a5e60`, `sh2shd_del_jms_head` `0x001a5f60`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JamesSelfShadowExclusion {
    /// No parts excluded.
    None,
    /// Exclude head parts `[8, 0x0f]` (`self_shadow_flag == 3 || self_shadow_flag == 8`).
    Head,
    /// Exclude upper-body + head parts `[1, 3, 0x1c, 0x1b, 0x22, 0x21, 0x23, 0x28, 8, 0x0f]`
    /// (`self_shadow_flag == 2 || self_shadow_flag == 7`).
    UpperBody,
}

impl JamesSelfShadowExclusion {
    /// Resolves the self-shadow exclusion mode for a character ID and PS2 shadow mode flag
    /// (`sh2shd_shadow_model.c` lines 991-996 & 1082-1087).
    #[must_use]
    pub const fn for_character(chara_id: u16, shadow_mode_flag: i16) -> Self {
        // James model IDs: 0x100..=0x103 (`sh2shd_shadow_model.c` line 1074)
        if chara_id < 0x100 || chara_id > 0x103 {
            return Self::None;
        }
        match shadow_mode_flag {
            3 | 8 => Self::Head,
            2 | 7 => Self::UpperBody,
            _ => Self::None,
        }
    }

    /// Returns the exact slice of excluded `.kg1` part IDs (`0x001a5e60` / `0x001a5f60`).
    #[must_use]
    pub const fn excluded_parts(self) -> &'static [u16] {
        match self {
            Self::None => &[],
            Self::Head => &JAMES_HEAD_EXCLUDED_PARTS,
            Self::UpperBody => &JAMES_UPPER_BODY_EXCLUDED_PARTS,
        }
    }

    /// Returns `true` if `part_id` is excluded from casting a self-shadow.
    #[must_use]
    pub fn is_part_excluded(self, part_id: u16) -> bool {
        self.excluded_parts().contains(&part_id)
    }
}

/// Character bone IDs sampled by `sh2shd_get_height_for_alpha` (`0x0018dc50`, `sh2shd_char_jms.c`)
/// to establish the `(high_y, low_y)` height bounds for drop-shadow alpha attenuation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DropShadowHeightBones {
    /// Upper/head bone IDs (minimum Y in PS2 Y-down space).
    pub high_bones: &'static [usize],
    /// Lower/foot bone IDs (maximum Y in PS2 Y-down space).
    pub low_bones: &'static [usize],
}

/// Returns the exact bone IDs used by `sh2shd_get_height_for_alpha` (`0x0018dc50`) for `chara_id`.
///
/// Returns `None` for James held items (`0x801..=0x808`), which inherit James's cached
/// `(_jms_high_1393, _jms_low_1394)` range (`sh2shd_char_jms.c` lines 1088-1091), or unknown IDs.
#[must_use]
pub const fn drop_shadow_height_bones(chara_id: u16) -> Option<DropShadowHeightBones> {
    match chara_id {
        // James variants (`0x100..=0x103`): high=8, low=max(0x1e, 0x1d)
        0x100..=0x103 => Some(DropShadowHeightBones {
            high_bones: &[8],
            low_bones: &[0x1e, 0x1d],
        }),
        // Playable Maria (`0x105`): high=9, low=max(0x16, 0x14)
        0x105 => Some(DropShadowHeightBones {
            high_bones: &[9],
            low_bones: &[0x16, 0x14],
        }),
        // Maria (`0x106`): high=9, low=max(0x1e, 0x1b)
        0x106 => Some(DropShadowHeightBones {
            high_bones: &[9],
            low_bones: &[0x1e, 0x1b],
        }),
        // Angela / Eddie / Laura (`0x107`): high=9, low=max(0x22, 0x24)
        0x107 => Some(DropShadowHeightBones {
            high_bones: &[9],
            low_bones: &[0x22, 0x24],
        }),
        // Lying Figure (`0x200`): high=0xb, low=max(0x14, 0x15)
        0x200 => Some(DropShadowHeightBones {
            high_bones: &[0x0b],
            low_bones: &[0x14, 0x15],
        }),
        // Mannequin (`0x201`): high=min(9, 0xb), low=max(0xd, 0xe)
        0x201 => Some(DropShadowHeightBones {
            high_bones: &[9, 0x0b],
            low_bones: &[0x0d, 0x0e],
        }),
        _ => None,
    }
}

/// Computes the `(high_y, low_y)` height bounds (PS2 world mm, Y-down) from posed bone matrices,
/// matching `sh2shd_get_height_for_alpha` (`0x0018dc50`) and its caller (`sh2shd_char_jms.c` lines 1078-1106).
///
/// - `low_y` is clamped to `>= raw_high_y`.
/// - `high_y = low_y - 700.0` (unless `drama_demo_number == 3`, where `high_y = low_y`).
#[must_use]
pub fn compute_drop_shadow_height_range(
    chara_id: u16,
    bone_matrices: &[[[f32; 4]; 4]],
    drama_demo_number: u32,
) -> Option<(f32, f32)> {
    let bones = drop_shadow_height_bones(chara_id)?;
    let mut raw_high = f32::INFINITY;
    for &b in bones.high_bones {
        if let Some(m) = bone_matrices.get(b) {
            raw_high = raw_high.min(m[3][1]);
        }
    }
    let mut low = f32::NEG_INFINITY;
    for &b in bones.low_bones {
        if let Some(m) = bone_matrices.get(b) {
            low = low.max(m[3][1]);
        }
    }
    if !raw_high.is_finite() || !low.is_finite() {
        return None;
    }
    if low < raw_high {
        low = raw_high;
    }
    let high = if drama_demo_number == 3 {
        low
    } else {
        low - DROP_SHADOW_HEIGHT_WINDOW_MM
    };
    Some((high, low))
}

/// Evaluates the normalized drop-shadow alpha factor in `[0.0, 1.0]` for a vertex at height `y_mm`
/// (PS2 Y-down world mm) given `(high_y, low_y)` from [`compute_drop_shadow_height_range`].
///
/// Feet (`y_mm >= low_y`) receive `1.0`; vertices `>= 700.0` mm above the feet (`y_mm <= high_y`)
/// fade to `0.0`. When `high_y == low_y` (e.g. `drama_demo_number == 3`), returns `1.0`.
#[must_use]
pub fn evaluate_drop_shadow_height_alpha(y_mm: f32, high_y: f32, low_y: f32) -> f32 {
    let span = low_y - high_y;
    if span <= 1e-4 {
        return 1.0;
    }
    ((y_mm - high_y) / span).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_draw_order_matches_slus_001a08a0() {
        assert_eq!(
            check_draw_order(0x000a_007d, 0),
            ShadowDrawOrder::ShadowBeforeCharacter
        );
        assert_eq!(
            check_draw_order(0x0009_0016, 0),
            ShadowDrawOrder::ShadowBeforeCharacter
        );
        // During cutscenes (`drama_demo_number != 0`), always CharacterBeforeShadow
        assert_eq!(
            check_draw_order(0x000a_007d, 1),
            ShadowDrawOrder::CharacterBeforeShadow
        );
        assert_eq!(
            check_draw_order(0x0006_0001, 0),
            ShadowDrawOrder::CharacterBeforeShadow
        );
    }

    #[test]
    fn map_shadow_light_overrides_match_slus_001a6600() {
        // Stage 0x0c, Room 0x5b -> Parallel type 4
        let ov1 = map_shadow_light_override(0x0c, 0x5b, true).expect("override for 0c:5b");
        match ov1 {
            ShadowLightOverride::Parallel {
                light_type,
                direction,
            } => {
                assert_eq!(light_type, 4);
                assert_eq!(direction.x.to_bits(), 0x3d95_bca0);
                assert_eq!(direction.y.to_bits(), 0x3f2f_0154);
                assert_eq!(direction.z.to_bits(), 0x3f39_e673);
            }
            _ => panic!("expected Parallel override"),
        }

        // Stage 9, Room 0x2a with flashlight OFF -> Spotlight type 0
        let ov2 = map_shadow_light_override(9, 0x2a, false).expect("override for 9:2a spot off");
        match ov2 {
            ShadowLightOverride::Spotlight {
                light_type,
                position,
                direction,
            } => {
                assert_eq!(light_type, 0);
                assert_eq!(position.x.to_bits(), 0xc763_ee00);
                assert_eq!(position.y.to_bits(), 0xc42f_0000);
                assert_eq!(position.z.to_bits(), 0x4696_9600);
                assert_eq!(direction.x.to_bits(), 0xbeb9_5810);
                assert_eq!(direction.y.to_bits(), 0x3f44_dd2f);
                assert_eq!(direction.z.to_bits(), 0x3ea6_e979);
            }
            _ => panic!("expected Spotlight override"),
        }
        // Stage 9, Room 0x2a with flashlight ON -> no override
        assert!(map_shadow_light_override(9, 0x2a, true).is_none());
    }

    #[test]
    fn james_self_shadow_exclusion_masks_match_slus_001a5e60_and_001a5f60() {
        let upper = JamesSelfShadowExclusion::for_character(0x100, 2);
        assert_eq!(upper, JamesSelfShadowExclusion::UpperBody);
        assert_eq!(
            upper.excluded_parts(),
            &[1, 3, 0x1c, 0x1b, 0x22, 0x21, 0x23, 0x28, 8, 0x0f]
        );
        assert!(upper.is_part_excluded(0x28));
        assert!(!upper.is_part_excluded(0));

        let head = JamesSelfShadowExclusion::for_character(0x102, 8);
        assert_eq!(head, JamesSelfShadowExclusion::Head);
        assert_eq!(head.excluded_parts(), &[8, 0x0f]);
        assert!(head.is_part_excluded(8));
        assert!(!head.is_part_excluded(1));

        // Non-James character is never excluded
        assert_eq!(
            JamesSelfShadowExclusion::for_character(0x200, 2),
            JamesSelfShadowExclusion::None
        );
    }

    #[test]
    fn drop_shadow_height_alpha_matches_slus_0018dc50() {
        let mut mats = vec![glam::Mat4::IDENTITY.to_cols_array_2d(); 40];
        // James (0x100): head bone 8 at y=-1600 mm, feet bones 0x1d/0x1e at y=0 mm and y=50 mm
        mats[8][3][1] = -1600.0;
        mats[0x1d][3][1] = 0.0;
        mats[0x1e][3][1] = 50.0;

        let (high, low) = compute_drop_shadow_height_range(0x100, &mats, 0).unwrap();
        assert_eq!(low, 50.0);
        assert_eq!(high, 50.0 - 700.0);

        // At feet (`y = 50.0`) -> alpha 1.0; at midpoint (`y = -300.0`) -> 0.5; at head (`y = -1600.0`) -> 0.0
        assert!((evaluate_drop_shadow_height_alpha(50.0, high, low) - 1.0).abs() < 1e-6);
        assert!((evaluate_drop_shadow_height_alpha(-300.0, high, low) - 0.5).abs() < 1e-6);
        assert!((evaluate_drop_shadow_height_alpha(-1600.0, high, low) - 0.0).abs() < 1e-6);

        // Drama demo 3 collapses high == low -> uniform 1.0 alpha
        let (d3_high, d3_low) = compute_drop_shadow_height_range(0x100, &mats, 3).unwrap();
        assert_eq!(d3_high, d3_low);
        assert_eq!(
            evaluate_drop_shadow_height_alpha(-1600.0, d3_high, d3_low),
            1.0
        );
    }
}
