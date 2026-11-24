//! Multi-Floor Elevator Panels & Chase Fixes (`sh2pc.exe` `FUN_004a2a50` `0x004a2e29`,
//! `0x0057a671`, `FUN_004a8be0` `0x004a8bd8`; SH2:EE `Patches/ElevatorCursorColorFix.cpp`,
//! `Patches/HospitalChase.cpp`).

use crate::events::stages_act2::riddles_hotel_2f::try_board_employee_elevator;
use crate::events::EventFlags;
use crate::inventory::InventoryManager;

/// Room ID for Otherworld Hospital Pyramid Head elevator chase hallway
/// (`R_HSP_ALT_RPT_HALLWAY = 0x5b`, `Patches/Patches.h:248`).
pub const ROOM_HSP_ALT_RPT_HALLWAY: u16 = 0x5b;

/// X coordinate threshold in PS2 mm for the Otherworld Hospital elevator stabbing animation fix
/// (`Patches/HospitalChase.cpp:48`: `GetJamesPosX() > 33185.0f`).
pub const HOSPITAL_CHASE_STAB_X_THRESHOLD_MM: f32 = 33_185.0;

/// Fixed animation/state parameter written to `FUN_004a8be0` case `4` (`0x004a8cbe`)
/// by `Patches/HospitalChase.cpp:50` (`Value = 5`).
pub const HOSPITAL_CHASE_STAB_PARAM_FIXED: u8 = 5;

/// Elevator panel cursor color tint (`sh2pc.exe` `FUN_004a2a50` `0x004a2e29..0x004a2e78`,
/// `Patches/ElevatorCursorColorFix.cpp:23-27`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ElevatorCursorColor {
    /// `0` = Gray (`0x00808080`, `ElevatorCursorColorFix.cpp:41`).
    Gray = 0,
    /// `1` = Blue (`0x00606080`, `sh2pc.exe` `0x004a2e55`).
    Blue = 1,
    /// `2` = Red (`0x00806060`, `sh2pc.exe` `0x004a2e63`).
    Red = 2,
    /// `4` = Green (`0x00608060`, `sh2pc.exe` `0x004a2e71`).
    Green = 4,
}

impl ElevatorCursorColor {
    /// Returns the 24-bit RGB tint (`0x00RRGGBB` / BGR-packed PS2 GS tint) from `FUN_004a2a50`.
    #[must_use]
    pub const fn gs_tint_rgb(self) -> u32 {
        match self {
            Self::Gray => 0x0080_8080,
            Self::Blue => 0x0060_6080,
            Self::Red => 0x0080_6060,
            Self::Green => 0x0060_8060,
        }
    }
}

/// Returns the Hotel Employee Elevator cursor color (`sh2pc.exe` `0x0057a671`):
/// `Gray` (`0`) in vanilla vs `Green` (`4`) with `Patches/ElevatorCursorColorFix.cpp:71`.
#[must_use]
pub const fn hotel_employee_elevator_cursor_color(
    enable_sh2ee_world_fixes: bool,
) -> ElevatorCursorColor {
    if enable_sh2ee_world_fixes {
        ElevatorCursorColor::Green
    } else {
        ElevatorCursorColor::Gray
    }
}

/// Returns the RPT elevator stabbing animation override (`5`) when James crosses `x > 33185.0`
/// in `R_HSP_ALT_RPT_HALLWAY` (`0x5b`) with `Patches/HospitalChase.cpp` enabled.
#[must_use]
pub fn hospital_chase_elevator_stab_param(
    room_id: u16,
    james_pos_x_mm: f32,
    enable_sh2ee_world_fixes: bool,
) -> Option<u8> {
    if enable_sh2ee_world_fixes
        && room_id == ROOM_HSP_ALT_RPT_HALLWAY
        && james_pos_x_mm > HOSPITAL_CHASE_STAB_X_THRESHOLD_MM
    {
        Some(HOSPITAL_CHASE_STAB_PARAM_FIXED)
    } else {
        None
    }
}

/// Elevator installation kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElevatorKind {
    /// Brookhaven Hospital Normal Elevator (1F, 2F, 3F, RF — `flags 21, 22, 23`).
    HospitalNormal,
    /// Brookhaven Hospital Otherworld Elevator (1F, 2F, 3F, B1).
    HospitalOtherworld,
    /// Lakeview Hotel Employee Elevator (1F, 2F, 3F, B1 — weight-restricted to 1 person).
    HotelEmployee,
}

/// Selectable floor in a multi-floor elevator panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ElevatorFloor {
    Basement1,
    Floor1,
    Floor2,
    Floor3,
    Roof,
}

impl ElevatorFloor {
    /// Returns the Brookhaven Hospital Normal Elevator lock flag (`21`, `22`, `23`) if gated.
    #[must_use]
    pub const fn hospital_normal_door_flag(self) -> Option<usize> {
        match self {
            Self::Floor1 => Some(21),
            Self::Floor2 => Some(22),
            Self::Floor3 => Some(23),
            Self::Basement1 | Self::Roof => None,
        }
    }
}

/// Interactive multi-floor elevator button panel state machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultiFloorElevatorPanel {
    /// Which elevator this panel belongs to.
    pub kind: ElevatorKind,
    /// Currently highlighted floor index in [`Self::available_floors`].
    pub cursor_index: usize,
    /// Current floor where the cabin is parked.
    pub current_floor: ElevatorFloor,
    /// Cursor tint color (`ElevatorCursorColorFix.cpp`).
    pub cursor_color: ElevatorCursorColor,
}

impl MultiFloorElevatorPanel {
    /// Creates a new [`MultiFloorElevatorPanel`] for `kind` starting at `current_floor`.
    #[must_use]
    pub const fn new(
        kind: ElevatorKind,
        current_floor: ElevatorFloor,
        enable_sh2ee_world_fixes: bool,
    ) -> Self {
        let cursor_color = match kind {
            ElevatorKind::HospitalNormal => ElevatorCursorColor::Blue,
            ElevatorKind::HospitalOtherworld => ElevatorCursorColor::Red,
            ElevatorKind::HotelEmployee => {
                hotel_employee_elevator_cursor_color(enable_sh2ee_world_fixes)
            }
        };
        Self {
            kind,
            cursor_index: 0,
            current_floor,
            cursor_color,
        }
    }

    /// Returns the ordered list of floor buttons on this panel.
    #[must_use]
    pub const fn available_floors(&self) -> &'static [ElevatorFloor] {
        match self.kind {
            ElevatorKind::HospitalNormal => &[
                ElevatorFloor::Roof,
                ElevatorFloor::Floor3,
                ElevatorFloor::Floor2,
                ElevatorFloor::Floor1,
            ],
            ElevatorKind::HospitalOtherworld | ElevatorKind::HotelEmployee => &[
                ElevatorFloor::Floor3,
                ElevatorFloor::Floor2,
                ElevatorFloor::Floor1,
                ElevatorFloor::Basement1,
            ],
        }
    }

    /// Moves cursor up (`delta = -1`) or down (`delta = 1`), wrapping within [`Self::available_floors`].
    pub fn move_cursor(&mut self, delta: isize) {
        let len = self.available_floors().len() as isize;
        if len > 0 {
            self.cursor_index = (self.cursor_index as isize + delta).rem_euclid(len) as usize;
        }
    }

    /// Confirms the highlighted floor button, returning `Ok(dest_floor)` or `Err(se_id)` if blocked
    /// (e.g. Hotel Employee Elevator overweight alarm `0x44c4`).
    pub fn press_selected_floor(
        &mut self,
        flags: &mut EventFlags,
        inventory: &InventoryManager,
    ) -> Result<ElevatorFloor, u32> {
        let dest = self.available_floors()[self.cursor_index];
        if self.kind == ElevatorKind::HotelEmployee {
            try_board_employee_elevator(flags, inventory)?;
        }
        self.current_floor = dest;
        Ok(dest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn elevator_cursor_color_and_hospital_chase_respect_toggle() {
        assert_eq!(
            hotel_employee_elevator_cursor_color(false),
            ElevatorCursorColor::Gray
        );
        assert_eq!(
            hotel_employee_elevator_cursor_color(true),
            ElevatorCursorColor::Green
        );
        assert_eq!(ElevatorCursorColor::Green.gs_tint_rgb(), 0x0060_8060);

        assert_eq!(
            hospital_chase_elevator_stab_param(ROOM_HSP_ALT_RPT_HALLWAY, 33_186.0, false),
            None
        );
        assert_eq!(
            hospital_chase_elevator_stab_param(ROOM_HSP_ALT_RPT_HALLWAY, 33_186.0, true),
            Some(5)
        );
    }

    #[test]
    fn hotel_employee_elevator_blocks_when_carrying_items() {
        let mut panel =
            MultiFloorElevatorPanel::new(ElevatorKind::HotelEmployee, ElevatorFloor::Floor2, true);
        panel.move_cursor(2); // Floor1
        let mut flags = EventFlags::new();
        let mut inv = InventoryManager::new();
        inv.item_get_now(1); // Health drink
        assert!(panel.press_selected_floor(&mut flags, &inv).is_err());

        inv = InventoryManager::new();
        assert_eq!(
            panel.press_selected_floor(&mut flags, &inv),
            Ok(ElevatorFloor::Floor1)
        );
    }
}
