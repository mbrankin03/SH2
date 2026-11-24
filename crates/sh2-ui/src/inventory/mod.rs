//! Authentic 1:1 PS2 / SH2:EE Single-Screen Status & Inventory Menu (`Item/otn_itemmain.c`).
//!
//! # Ground Truth References
//! - **PS2 Prototype (`SLUS_202.28`):**
//!   - `itemmain` (`0x001cdb40`), `itemmainmain` (`0x001cddd0`): 10-slot item carousel with center
//!     selected slot `5`, smooth scale lerp `0.8 -> 1.0` (`0x3f4ccccd..0x3f800000`, step `0.1`).
//!   - `item_turn` (`0x001d31a0`), `item_position` (`0x001d3270`).
//!   - `command_kind` (`0x001d3610`, jump table `0x00392f30`): item-to-command-group classification.
//!   - `command_main` (`0x001cf9e0`), `weapon_command_main` (`0x001d15e0`),
//!     `combine_item_use` (`0x001d38d0`), `examine_main` (`0x001d3a00`).
//!   - `look_hp` (`0x001d9660`): top-left Health ECG Status Box.
//!   - `item_screen_obj_data` (`0x002a7410`): 3D `.mdl` carousel scale/rotation metadata.
//! - **SH2:EE Patches:**
//!   - `CommandWindowMouseFix.cpp` (`0x00472428`): 3-option command window mouse hitboxes
//!     (`x in 356..440`, `y in 63..140`, rows `< 88`, `< 114`, `>= 114` in 640x480 space) and
//!     preserving command selection on mouse Combine.
//!   - `PatchInventoryBGM.cpp` (`0x005166e8`): keep room BGM playing while in `EVENT_INVENTORY`,
//!     `EVENT_MAP`, `EVENT_MEMO_LIST`, and `EVENT_OPTIONS_FMV`.

pub mod atlas;
pub mod health_ecg;
pub mod render;
pub mod state;
pub mod widgets;

pub use atlas::{item_icon_rect, project_carousel_slot, CarouselSlotProjection, ItemIconRect};
pub use health_ecg::{HealthEcgState, HealthStatus, RedCrossIconState, RED_CROSS_ICON_PATH};
pub use render::draw_inventory_screen;
pub use state::InventoryScreen;

/// Computes the centered 4:3 PS2 UI canvas `(origin_x, origin_y, canvas_w, canvas_h)` inside
/// an arbitrary viewport `(viewport_w, viewport_h)` so `512x448` PS2 UI elements (the 4:3
/// `Status` snapshot box, square reticle, and 3-column bars) never stretch horizontally on
/// 16:9 / 21:9 widescreen displays (matching SH2:EE 4:3 menu pillarboxing).
pub fn ps2_ui_canvas_rect(viewport_w: f32, viewport_h: f32) -> (f32, f32, f32, f32) {
    if viewport_w <= 0.0 || viewport_h <= 0.0 {
        return (0.0, 0.0, viewport_w.max(0.0), viewport_h.max(0.0));
    }
    let target_aspect = 4.0 / 3.0;
    let current_aspect = viewport_w / viewport_h;
    if current_aspect > target_aspect {
        let canvas_h = viewport_h;
        let canvas_w = viewport_h * target_aspect;
        ((viewport_w - canvas_w) * 0.5, 0.0, canvas_w, canvas_h)
    } else {
        let canvas_w = viewport_w;
        let canvas_h = viewport_w / target_aspect;
        (0.0, (viewport_h - canvas_h) * 0.5, canvas_w, canvas_h)
    }
}

/// Number of visible slots in the PS2 carousel (`item_turn` @ `0x001d3258`, `addiu $3, $8, 0xa`).
pub const CAROUSEL_VISIBLE_SLOTS: usize = 10;
/// Center selected slot index in the carousel (`itemmainmain` @ `0x001ce178`, `addiu $3, $zero, 0x5`).
pub const CAROUSEL_CENTER_SLOT: usize = 5;
/// Selected item scale in `itemmainmain` (`0x3f800000` at `0x001ce188`).
pub const CAROUSEL_SELECTED_SCALE: f32 = 1.0;
/// Unselected item scale in `itemmainmain` (`0x3f4ccccd` at `0x001ce1d4`).
pub const CAROUSEL_UNSELECTED_SCALE: f32 = 0.8;
/// Per-tick scale transition step in `itemmainmain` (`0x3dcccccd` at `0x001ce1a4`).
pub const CAROUSEL_SCALE_STEP: f32 = 0.1;

/// SH2:EE `CommandWindowMouseFix.cpp` (`0x00472428`) 512x448 PS2/PC logical X min (exclusive `355`).
pub const CMD_WIN_MOUSE_X_MIN: f32 = 356.0;
/// SH2:EE `CommandWindowMouseFix.cpp` (`0x00472428`) 512x448 PS2/PC logical X max (exclusive `440`).
pub const CMD_WIN_MOUSE_X_MAX: f32 = 439.0;
/// SH2:EE `CommandWindowMouseFix.cpp` (`0x00472428`) 512x448 PS2/PC logical Y min (exclusive `62`).
pub const CMD_WIN_MOUSE_Y_MIN: f32 = 63.0;
/// SH2:EE `CommandWindowMouseFix.cpp` (`0x00472428`) 512x448 PS2/PC logical Y max (exclusive `140`).
pub const CMD_WIN_MOUSE_Y_MAX: f32 = 139.0;
/// Row 0/1 split Y boundary in 512x448 PS2/PC space (`CommandWindowMouseFix.cpp:45`, `MouseY < 88`).
pub const CMD_WIN_ROW_0_MAX_Y: f32 = 88.0;
/// Row 1/2 split Y boundary in 512x448 PS2/PC space (`CommandWindowMouseFix.cpp:47`, `MouseY < 114`).
pub const CMD_WIN_ROW_1_MAX_Y: f32 = 114.0;

/// Three bottom-row navigation buttons on the Status / Inventory screen
/// (`SLUS_202.28` `font_print` @ `0x001d5c50..0x001d5ca8` & retail `item_msg_e.mes` `0x09..0x0b`,
/// `y = 0x1ac = 428` in `512x448` space: `Option | Map | Memo`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CornerButton {
    /// Open in-game Options menu (`0x09` `"Option"` at `x = 0x40 = 64, y = 0x1ac = 428`).
    Option = 0,
    /// Open Fullscreen Paper Map (`0x0a` `"Map"` at `x = 0xf0 = 240, y = 0x1ac = 428`, `Event/chizu.c`).
    Map = 1,
    /// Open collected Memo List (`0x0b` `"Memo"` at `x = 0x187 = 391, y = 0x1ac = 428`, gated by `MemoCommandCheck`).
    Memo = 2,
}

impl CornerButton {
    /// Ordered list of the three PS2 bottom-row buttons (`Option`, `Map`, `Memo`).
    pub const ALL: [CornerButton; 3] =
        [CornerButton::Option, CornerButton::Map, CornerButton::Memo];

    /// Authentic PS2 / retail `item_msg_e.mes` (`0x09..0x0b`) display label.
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Option => "Option",
            Self::Map => "Map",
            Self::Memo => "Memo",
        }
    }

    /// Logical `512x448` PS2 hitbox `(x0, y0, x1, y1)` matching `SLUS_202.28` `sprite`
    /// (`0x001d8c80..0x001d8d58`: column width `140` at `x = 26, 186, 346`) and `font_print` (`y = 428`).
    pub const fn hitbox_512x448(&self) -> (f32, f32, f32, f32) {
        match self {
            Self::Option => (26.0, 408.0, 166.0, 442.0),
            Self::Map => (186.0, 408.0, 326.0, 442.0),
            Self::Memo => (346.0, 408.0, 486.0, 442.0),
        }
    }
}

/// Action verbs displayed inside the Inventory Command Pop-up Window (`command_kind` @ `0x001d3610`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemCommand {
    /// Use consumable healing item or context-sensitive key/puzzle item.
    Use,
    /// Equip weapon into active hand.
    Equip,
    /// Unequip currently held weapon.
    Unequip,
    /// Reload matching firearm directly from ammunition stack in inventory.
    Reload,
    /// Inspect item closely (3D turntable / 2D cutaway + examination text).
    Look,
    /// Combine two or three puzzle items (`ITEM_COMBOS` `86..=96`).
    Combine,
    /// Turn Flashlight or Radio ON.
    TurnOn,
    /// Turn Flashlight or Radio OFF.
    TurnOff,
    /// Adjust Pocket Radio volume (`look_volume` `0x001d9480`, levels `0..=15`).
    Volume,
}

impl ItemCommand {
    /// Authentic Silent Hill 2 command window label.
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Use => "Use",
            Self::Equip => "Equip",
            Self::Unequip => "Unequip",
            Self::Reload => "Reload",
            Self::Look => "Look",
            Self::Combine => "Combine",
            Self::TurnOn => "ON",
            Self::TurnOff => "OFF",
            Self::Volume => "Volume",
        }
    }
}

/// Active input focus zone within the Status / Inventory screen (`otn_itemmain.c`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InventoryFocus {
    /// Navigating the horizontal 3D item carousel.
    #[default]
    Carousel,
    /// Navigating the 3 bottom-row buttons (`Option`, `Map`, `Memo`).
    Corner(CornerButton),
    /// Choosing an action inside the command pop-up window for the selected item.
    CommandWindow,
    /// Adjusting the 16-bar Pocket Radio volume meter (`DAT_0116e3e4 == 1`, `look_volume` `0x001d9480`).
    RadioVolume,
    /// Selecting a second item in the carousel to combine with `combine_first_item_id`.
    CombineTarget,
    /// Close-up `Look` examination overlay showing extended examination text.
    ExamineOverlay,
}

/// High-level action emitted by [`InventoryScreen`] for `sh2-app` to apply to `SimulationCore`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InventoryAction {
    /// No external state change required.
    None,
    /// Exit inventory screen and resume active gameplay.
    Close,
    /// Transition to Fullscreen Paper Map (`Event/chizu.c`).
    OpenMap,
    /// Transition to Memo Reader (`Event/memo.c`).
    OpenMemo,
    /// Transition to Options Menu (returning to Inventory on Cancel).
    OpenOptions,
    /// Consume a healing item (`1` = Health Drink, `2` = First-Aid Kit, `3` = Ampoule).
    UseHealing(u8),
    /// Equip weapon by Director's Cut catalog ID (`4, 6, 8, 10, 12, 13, 14, 15, 16, 17`).
    EquipWeapon(u8),
    /// Unequip currently held weapon.
    UnequipWeapon,
    /// Manual inventory ammo-to-firearm reload (`ammo_id` -> `firearm_id`).
    ReloadFirearm { ammo_id: u8, firearm_id: u8 },
    /// Toggle Flashlight power state (`ITEM_ID_FLASHLIGHT = 18`).
    ToggleFlashlight,
    /// Toggle Radio power state (`ITEM_ID_RADIO = 19`).
    ToggleRadio,
    /// Update Pocket Radio volume level (`0..=15`, `DAT_01126900` in `command_main` `0x001cf9e0`).
    SetRadioVolume(u8),
    /// Attempt to use a key/puzzle item (`item_id`, or combined pseudo-item `86..=96`).
    UseKeyOrPuzzleItem(u8),
    /// Combined items produced pseudo-item `result_id` (`86..=96`).
    CombinedItems {
        first_id: u8,
        second_id: u8,
        result_id: u8,
    },
}

/// Default Pocket Radio volume level (`SLUS_202.28` `ItemDataInit` `0x001ad860`: `DAT_01126900 = 0xc`).
pub const DEFAULT_RADIO_VOLUME: u8 = 0x0c;

/// Snapshot of runtime player status passed into [`InventoryScreen`] for rendering & commands.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InventoryPlayerSnapshot {
    /// Current player HP (`0.0..=max_hp`).
    pub hp: f32,
    /// Maximum player HP (typically `100.0`).
    pub max_hp: f32,
    /// Currently equipped weapon item ID (`None` if unarmed).
    pub equipped_weapon_item_id: Option<u8>,
    /// Loaded rounds in the currently equipped firearm magazine (`None` for melee/unarmed).
    pub equipped_magazine_rounds: Option<u32>,
    /// Magazine capacity of the currently equipped firearm (`None` for melee/unarmed).
    pub equipped_magazine_capacity: Option<u32>,
    /// Reserve ammunition count for the currently equipped firearm (`None` for melee/unarmed).
    pub equipped_reserve_ammo: Option<u16>,
    /// Whether the pocket flashlight is currently switched ON.
    pub flashlight_on: bool,
    /// Whether the pocket radio is currently switched ON.
    pub radio_on: bool,
    /// Pocket radio volume level (`0..=15`, `DAT_01126900` in `SLUS_202.28`).
    pub radio_volume: u8,
    /// Whether the player has collected at least one memo (`MemoCommandCheck` `0x00230570`).
    pub has_any_memo: bool,
}

impl Default for InventoryPlayerSnapshot {
    fn default() -> Self {
        Self {
            hp: 100.0,
            max_hp: 100.0,
            equipped_weapon_item_id: None,
            equipped_magazine_rounds: None,
            equipped_magazine_capacity: None,
            equipped_reserve_ammo: None,
            flashlight_on: false,
            radio_on: true,
            radio_volume: DEFAULT_RADIO_VOLUME,
            has_any_memo: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ps2_ui_canvas_and_status_snapshot_all_monitor_aspects() {
        let monitors: [(&str, f32, f32); 6] = [
            ("4:3 640x480", 640.0, 480.0),
            ("16:10 1920x1200", 1920.0, 1200.0),
            ("16:9 4K 3840x2160", 3840.0, 2160.0),
            ("21:9 3440x1440", 3440.0, 1440.0),
            ("32:9 5120x1440", 5120.0, 1440.0),
            ("Portrait 1080x1920", 1080.0, 1920.0),
        ];

        for (label, vw, vh) in monitors {
            let (ox, oy, cw, ch) = ps2_ui_canvas_rect(vw, vh);
            assert!(ox >= 0.0 && oy >= 0.0, "{label}: negative origin");
            assert!(
                ox + cw <= vw + 1e-3 && oy + ch <= vh + 1e-3,
                "{label}: overflow"
            );
            assert!(
                ((cw / ch) - (4.0 / 3.0)).abs() < 1e-3,
                "{label}: canvas must preserve 4:3 aspect (got {})",
                cw / ch
            );

            let (sx, sy, sw, sh) = HealthEcgState::snapshot_viewport_rect(vw, vh);
            assert!(sx >= ox && sy >= oy, "{label}: snapshot outside canvas");
            assert!(
                sx + sw <= ox + cw + 1e-3 && sy + sh <= oy + ch + 1e-3,
                "{label}: snapshot overflows canvas"
            );
            // `look_hp` (140x105 inside 512x448 -> 4:3): verify exact relative placement inside canvas
            assert!(((sx - ox) / cw - (26.0 / 512.0)).abs() < 1e-4, "{label}");
            assert!(((sy - oy) / ch - (49.0 / 448.0)).abs() < 1e-4, "{label}");
        }
    }
}
