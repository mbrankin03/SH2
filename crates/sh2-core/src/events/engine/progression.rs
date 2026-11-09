//! Flashlight state check (`LightSpotOnOffCheck`) and story progression milestone evaluation
//! (`EventProgressCheck`) from `SLUS_202.28` `src/Event/event.c`.

use super::super::EventFlags;

/// Evaluates `LightSpotOnOffCheck` (`SLUS_202.28` `0x001aad70`):
/// returns `true` when the player possesses the Flashlight (`has_flashlight`), the flashlight
/// toggle is switched on (`flashlight_switched_on`), and global flag `272` (`0x01126450` bit 16,
/// flashlight disabled/dead battery) is NOT set.
#[inline]
pub fn light_spot_on_off_check(
    has_flashlight: bool,
    flashlight_switched_on: bool,
    flags: &EventFlags,
) -> bool {
    has_flashlight && flashlight_switched_on && !flags.get(272)
}

/// Evaluates `EventProgressCheck` (`SLUS_202.28` `0x001aaf00`), returning the early/mid-game
/// story progression milestone (`0..=10`) based on `stage_glb_crd`, `room_id`, `player_pos`,
/// and `EventFlags`.
pub fn event_progress_check(
    stage_glb_crd: i32,
    room_id: u8,
    player_pos: [f32; 3],
    flags: &EventFlags,
) -> i32 {
    // 1. Forest trail / graveyard (`glb_crd == 6` and `!flag(32)`)
    if stage_glb_crd == 6 && !flags.get(32) {
        return 1;
    }
    // 2. East South Vale streets (`glb_crd == 1` and `!flag(36)` and `px > -20000` and `pz > -20000`)
    if stage_glb_crd == 1
        && !flags.get(36)
        && player_pos[0] > -20_000.0
        && player_pos[2] > -20_000.0
    {
        return 2;
    }
    // 3. Martin Street / apartment gate (`glb_crd == 2` and `!flag(43)`)
    if stage_glb_crd == 2 && !flags.get(43) {
        return 3;
    }
    // 4. Wood Side Apartments flashlight / corpse (`flag(69) && !flag(70)`)
    if flags.get(69) && !flags.get(70) {
        return 4;
    }
    // 5. Wood Side Apartments Room 208 (`flag(69) && room_id == 0x18`)
    if flags.get(69) && room_id == 0x18 {
        return 5;
    }
    // 6. West South Vale / Rosewater Park (`glb_crd == 3` and `!flag(150)`)
    if stage_glb_crd == 3 && !flags.get(150) {
        return 6;
    }
    // 7. After meeting Maria at Rosewater Park (`flag(150) && !flag(152)`)
    if flags.get(150) && !flags.get(152) {
        return 7;
    }
    // 8. Bowling alley / Heaven's Night (`glb_crd == 3 && flag(152) && !flag(157)`)
    if stage_glb_crd == 3 && flags.get(152) && !flags.get(157) {
        return 8;
    }
    // 9. Pursuing Laura toward Brookhaven (`glb_crd == 3 && flag(162) && !flag(163)`)
    if stage_glb_crd == 3 && flags.get(162) && !flags.get(163) {
        return 9;
    }
    // 10. Lakeview Hotel (`flag(379) && !flag(380)`)
    if flags.get(379) && !flags.get(380) {
        return 10;
    }
    0
}
