//! Pushable Stage Objects & `FlashlightClockPush` (`Event/event.c` `EventMain` `0x001a7810`,
//! `sh2pc.exe` `0x0048e81b`, SH2:EE `Patches/FlashlightClockPush.cpp`).
//!
//! Handles stage objects that James can push aside to reveal hidden passages:
//! - **Wood Side Apartments Room 208 Grandfather Clock (`Stage 7`, `Room 0x18`, `chr/item/b_clo.mdl`)**:
//!   Unlocked when `flag(81)` (`PZ_CLOCK == 0x2400`) is set; pushing sets `flag(82)` (`ApartE2fAp52Wall`).
//!   In `sh2pc.exe` (`0x0048e81b`), `EventMain` clears `sys.flag` with `0xFFFFFF3F` (`~0xC0`), which
//!   breaks lighting/flashlight state during the Rm 208 clock push (`camera_pos_y == -112.6049728f`);
//!   SH2:EE `Patches/FlashlightClockPush.cpp` replaces the mask with `0xFFFFFFEF` (`~0x10`).
//! - **Brookhaven Otherworld B1 Storeroom Shelf (`Stage 24`, `flag(223)`)**:
//!   Pushed aside to reveal the hidden passage (`sh2pc.exe` `0x00588370`).

use crate::events::EventFlags;

/// Room ID for Wood Side Apartments Room 208 (`R_APT_E_RM_208 = 0x18`, `Patches.h:182`).
pub const ROOM_APT_E_RM_208: u16 = 0x18;

/// Exact in-game camera Y coordinate during the Room 208 grandfather clock push cutscene
/// (`Patches/FlashlightClockPush.cpp:58`: `-112.6049728f`).
pub const FLASHLIGHT_CLOCK_PUSH_CAM_Y_MM: f32 = -112.604_97;

/// Vanilla PS2/PC `EventMain` `sys.flag` bitmask (`SLUS_202.28` `0x001a785c`,
/// `sh2pc.exe` `0x0048e81b`: `_DAT_010f08a4 &= 0xffffff3f`).
pub const SYS_FLAG_MASK_VANILLA: u32 = 0xFFFF_FF3F;

/// SH2:EE `FlashlightClockPush.cpp:60` `sys.flag` bitmask (`0xFFFFFFEF`) applied in Room 208
/// during the grandfather clock push to preserve lighting/flashlight bits.
pub const SYS_FLAG_MASK_CLOCK_PUSH_FIX: u32 = 0xFFFF_FFEF;

/// Flag `81`: Wood Side Apartments Room 208 grandfather clock hands set to 9:10 (`sh2pc.exe` `0x0059b9e0`).
pub const FLAG_APART_CLOCK_SOLVED: usize = 81;

/// Flag `82`: Wood Side Apartments Room 208 grandfather clock pushed aside (`ApartE2fAp52Wall`).
pub const FLAG_APART_CLOCK_PUSHED: usize = 82;

/// Flag `223`: Brookhaven Otherworld B1 Storeroom shelf pushed aside (`sh2pc.exe` `0x00588370`).
pub const FLAG_HOSPITAL_B1_SHELF_PUSHED: usize = 223;

/// Computes the per-frame `EventMain` `sys.flag` (`0x010f08a4` / `DAT_00932194`) bitmask,
/// gating `Patches/FlashlightClockPush.cpp` behind `enable_sh2ee_world_fixes`.
#[must_use]
pub fn event_main_sys_flag_mask(
    room_id: u16,
    camera_pos_y_mm: f32,
    enable_sh2ee_world_fixes: bool,
) -> u32 {
    if enable_sh2ee_world_fixes
        && room_id == ROOM_APT_E_RM_208
        && camera_pos_y_mm == FLASHLIGHT_CLOCK_PUSH_CAM_Y_MM
    {
        SYS_FLAG_MASK_CLOCK_PUSH_FIX
    } else {
        SYS_FLAG_MASK_VANILLA
    }
}

/// Identifies a pushable 3D stage object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PushableObjectKind {
    /// Wood Side Apartments Room 208 Grandfather Clock (`Stage 7`, `Room 0x18`, `chr/item/b_clo.mdl`).
    Apart208GrandfatherClock,
    /// Brookhaven Otherworld B1 Storeroom Shelf (`Stage 24`).
    HospitalB1StoreroomShelf,
}

impl PushableObjectKind {
    /// Returns the relative `.mdl` path under `data/` if this pushable object has a dedicated `.mdl`.
    #[must_use]
    pub const fn asset_rel_path(self) -> Option<&'static str> {
        match self {
            Self::Apart208GrandfatherClock => Some("chr/item/b_clo.mdl"),
            Self::HospitalB1StoreroomShelf => None,
        }
    }

    /// Returns `(unlock_flag, pushed_flag)` in `EventFlags`.
    #[must_use]
    pub const fn flags(self) -> (Option<usize>, usize) {
        match self {
            Self::Apart208GrandfatherClock => {
                (Some(FLAG_APART_CLOCK_SOLVED), FLAG_APART_CLOCK_PUSHED)
            }
            Self::HospitalB1StoreroomShelf => (None, FLAG_HOSPITAL_B1_SHELF_PUSHED),
        }
    }

    /// Returns `true` if the object is unlocked and ready to be pushed.
    #[must_use]
    pub fn can_push(self, flags: &EventFlags) -> bool {
        let (unlock_flag, pushed_flag) = self.flags();
        if flags.get(pushed_flag) {
            return false;
        }
        unlock_flag.is_none_or(|f| flags.get(f))
    }

    /// Marks the object as pushed in `flags` if [`Self::can_push`] is `true`.
    pub fn trigger_push(self, flags: &mut EventFlags) -> bool {
        if !self.can_push(flags) {
            return false;
        }
        let (_, pushed_flag) = self.flags();
        flags.set(pushed_flag, true);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flashlight_clock_push_mask_respects_vanilla_vs_sh2ee_toggle() {
        assert_eq!(
            event_main_sys_flag_mask(ROOM_APT_E_RM_208, FLASHLIGHT_CLOCK_PUSH_CAM_Y_MM, false),
            SYS_FLAG_MASK_VANILLA
        );
        assert_eq!(
            event_main_sys_flag_mask(ROOM_APT_E_RM_208, FLASHLIGHT_CLOCK_PUSH_CAM_Y_MM, true),
            SYS_FLAG_MASK_CLOCK_PUSH_FIX
        );
        assert_eq!(
            event_main_sys_flag_mask(ROOM_APT_E_RM_208, 0.0, true),
            SYS_FLAG_MASK_VANILLA
        );
    }

    #[test]
    fn apart_clock_push_requires_solved_flag_81_before_setting_flag_82() {
        let mut flags = EventFlags::new();
        assert!(!PushableObjectKind::Apart208GrandfatherClock.can_push(&flags));
        assert!(!PushableObjectKind::Apart208GrandfatherClock.trigger_push(&mut flags));

        flags.set(FLAG_APART_CLOCK_SOLVED, true);
        assert!(PushableObjectKind::Apart208GrandfatherClock.can_push(&flags));
        assert!(PushableObjectKind::Apart208GrandfatherClock.trigger_push(&mut flags));
        assert!(flags.get(FLAG_APART_CLOCK_PUSHED));
        assert!(!PushableObjectKind::Apart208GrandfatherClock.can_push(&flags));
    }
}
