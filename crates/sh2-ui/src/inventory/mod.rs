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

pub use atlas::{item_icon_rect, project_carousel_slot, CarouselSlotProjection, ItemIconRect};
pub use health_ecg::{HealthEcgState, HealthStatus};
pub use render::draw_inventory_screen;
pub use state::InventoryScreen;

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

/// SH2:EE `CommandWindowMouseFix.cpp` (`0x00472428`) 640x480 logical X min (exclusive `355`).
pub const CMD_WIN_MOUSE_X_MIN: f32 = 356.0;
/// SH2:EE `CommandWindowMouseFix.cpp` (`0x00472428`) 640x480 logical X max (exclusive `440`).
pub const CMD_WIN_MOUSE_X_MAX: f32 = 439.0;
/// SH2:EE `CommandWindowMouseFix.cpp` (`0x00472428`) 640x480 logical Y min (exclusive `62`).
pub const CMD_WIN_MOUSE_Y_MIN: f32 = 63.0;
/// SH2:EE `CommandWindowMouseFix.cpp` (`0x00472428`) 640x480 logical Y max (exclusive `140`).
pub const CMD_WIN_MOUSE_Y_MAX: f32 = 139.0;
/// Row 0/1 split Y boundary in 640x480 space (`CommandWindowMouseFix.cpp:45`, `MouseY < 88`).
pub const CMD_WIN_ROW_0_MAX_Y: f32 = 88.0;
/// Row 1/2 split Y boundary in 640x480 space (`CommandWindowMouseFix.cpp:47`, `MouseY < 114`).
pub const CMD_WIN_ROW_1_MAX_Y: f32 = 114.0;

/// Four corner navigation buttons on the Status / Inventory screen (`otn_itemmain.c`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CornerButton {
    /// Open collected Memo List (`Event/memo.c`).
    Memo = 0,
    /// Open Fullscreen Paper Map (`Event/chizu.c`).
    Map = 1,
    /// Open in-game Options menu.
    Option = 2,
    /// Exit Status / Inventory menu back to gameplay.
    Exit = 3,
}

impl CornerButton {
    /// Ordered list of the four corner buttons.
    pub const ALL: [CornerButton; 4] = [
        CornerButton::Memo,
        CornerButton::Map,
        CornerButton::Option,
        CornerButton::Exit,
    ];

    /// Display label on screen.
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Memo => "MEMO",
            Self::Map => "MAP",
            Self::Option => "OPTION",
            Self::Exit => "EXIT",
        }
    }

    /// Logical 640x480 hitbox `(x0, y0, x1, y1)` for mouse hover/click.
    pub const fn hitbox_640x480(&self) -> (f32, f32, f32, f32) {
        match self {
            Self::Memo => (470.0, 26.0, 550.0, 54.0),
            Self::Map => (556.0, 26.0, 622.0, 54.0),
            Self::Option => (470.0, 424.0, 550.0, 452.0),
            Self::Exit => (556.0, 424.0, 622.0, 452.0),
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
        }
    }
}

/// Active input focus zone within the Status / Inventory screen (`otn_itemmain.c`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InventoryFocus {
    /// Navigating the horizontal 3D item carousel.
    #[default]
    Carousel,
    /// Navigating the 4 corner buttons (`MEMO`, `MAP`, `OPTION`, `EXIT`).
    Corner(CornerButton),
    /// Choosing an action inside the command pop-up window for the selected item.
    CommandWindow,
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
    /// Attempt to use a key/puzzle item (`item_id`, or combined pseudo-item `86..=96`).
    UseKeyOrPuzzleItem(u8),
    /// Combined items produced pseudo-item `result_id` (`86..=96`).
    CombinedItems {
        first_id: u8,
        second_id: u8,
        result_id: u8,
    },
}

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
            has_any_memo: false,
        }
    }
}
