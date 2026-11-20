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

pub mod health_ecg;
pub mod render;

pub use health_ecg::{HealthEcgState, HealthStatus};
pub use render::draw_inventory_screen;

use crate::menu::{MenuNavAction, MenuSoundCue};
use sh2_core::{combine_items, item_metadata, InventoryManager, ItemCategory, CATALOG_ITEM_COUNT};

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
    /// Equip weapon by item ID (`4, 6, 8, 10, 12, 13, 14, 15, 79, 80`).
    EquipWeapon(u8),
    /// Unequip currently held weapon.
    UnequipWeapon,
    /// Manual inventory ammo-to-firearm reload (`ammo_id` -> `firearm_id`).
    ReloadFirearm { ammo_id: u8, firearm_id: u8 },
    /// Toggle Flashlight power state.
    ToggleFlashlight,
    /// Toggle Radio power state.
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

/// Maps an ammunition item ID (`5, 7, 9, 11`) to its corresponding firearm item ID (`4, 6, 8, 10` / `80`).
/// Grounded in `sh2-core::inventory::catalog` & `otn_itemmain.c` `command_kind` (`0x001d3610`).
pub const fn firearm_for_ammo(ammo_id: u8) -> Option<u8> {
    match ammo_id {
        5 => Some(4),   // Handgun bullets -> Handgun
        7 => Some(6),   // Shotgun shells -> Shotgun
        9 => Some(8),   // Rifle shells -> Hunting Rifle
        11 => Some(80), // Revolver bullets -> Maria's Revolver (Born from a Wish)
        _ => None,
    }
}

/// Maps a firearm item ID to its corresponding ammunition item ID.
pub const fn ammo_for_firearm(firearm_id: u8) -> Option<u8> {
    match firearm_id {
        4 => Some(5),
        6 => Some(7),
        8 => Some(9),
        80 => Some(11),
        _ => None,
    }
}

/// Authentic PS2 / SH2:EE Single-Screen Status & Inventory state machine (`Item/otn_itemmain.c`).
#[derive(Debug, Clone)]
pub struct InventoryScreen {
    /// Whether the Status / Inventory screen is currently open.
    pub is_open: bool,
    /// Ordered list of currently owned item IDs (`1..=85`) shown in the carousel.
    pub owned_items: Vec<u8>,
    /// Currently selected index in `owned_items`.
    pub selected_index: usize,
    /// Active input focus zone.
    pub focus: InventoryFocus,
    /// Available commands in the open command pop-up window.
    pub active_commands: Vec<ItemCommand>,
    /// Highlighted row index in `active_commands`.
    pub command_index: usize,
    /// First item ID chosen when `focus == InventoryFocus::CombineTarget`.
    pub combine_first_item_id: Option<u8>,
    /// 3D carousel turntable yaw angle in radians (`item_screen_obj.c`).
    pub item_yaw_rad: f32,
    /// Selected carousel item scale (`0.8..=1.0`, `itemmainmain` `0x001ce188`).
    pub selected_scale: f32,
    /// Top-left Health ECG Status Box animation state (`look_hp` `0x001d9660`).
    pub health_ecg: HealthEcgState,
    /// Optional status feedback toast message shown briefly in the footer.
    pub status_toast: Option<String>,
    /// Remaining duration in seconds for `status_toast`.
    pub status_toast_timer: f32,
}

impl Default for InventoryScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl InventoryScreen {
    /// Creates a closed `InventoryScreen`.
    pub fn new() -> Self {
        Self {
            is_open: false,
            owned_items: Vec::new(),
            selected_index: 0,
            focus: InventoryFocus::Carousel,
            active_commands: Vec::new(),
            command_index: 0,
            combine_first_item_id: None,
            item_yaw_rad: 0.0,
            selected_scale: CAROUSEL_UNSELECTED_SCALE,
            health_ecg: HealthEcgState::new(),
            status_toast: None,
            status_toast_timer: 0.0,
        }
    }

    /// Opens the Status / Inventory screen, refreshing the owned item carousel from `inventory`.
    /// Note (`PatchInventoryBGM.cpp` `0x005166e8`): opening Inventory freezes world simulation
    /// while **preserving** active ambient room BGM playback.
    pub fn open(&mut self, inventory: &InventoryManager) {
        self.is_open = true;
        self.focus = InventoryFocus::Carousel;
        self.active_commands.clear();
        self.command_index = 0;
        self.combine_first_item_id = None;
        self.selected_scale = CAROUSEL_UNSELECTED_SCALE;
        self.status_toast = None;
        self.status_toast_timer = 0.0;
        self.refresh_owned_items(inventory);
    }

    /// Closes the Status / Inventory screen.
    pub fn close(&mut self) {
        self.is_open = false;
        self.focus = InventoryFocus::Carousel;
        self.active_commands.clear();
        self.combine_first_item_id = None;
    }

    /// Rebuilds `owned_items` in canonical Director's Cut catalog order (`1..=85`).
    pub fn refresh_owned_items(&mut self, inventory: &InventoryManager) {
        let prev_id = self.selected_item_id();
        self.owned_items.clear();
        for id in 1..=(CATALOG_ITEM_COUNT as u8) {
            if inventory.has_item(id) {
                self.owned_items.push(id);
            }
        }
        if let Some(prev) = prev_id {
            if let Some(idx) = self.owned_items.iter().position(|&id| id == prev) {
                self.selected_index = idx;
                return;
            }
        }
        if self.selected_index >= self.owned_items.len() {
            self.selected_index = self.owned_items.len().saturating_sub(1);
        }
    }

    /// Returns the currently selected item ID in the carousel, if any.
    pub fn selected_item_id(&self) -> Option<u8> {
        self.owned_items.get(self.selected_index).copied()
    }

    /// Advances ECG animation, 3D carousel rotation, and scale lerp (`itemmainmain` `0x001ce188`).
    pub fn tick(&mut self, dt: f32, snapshot: &InventoryPlayerSnapshot) {
        if !self.is_open {
            return;
        }
        let dt = dt.clamp(0.0, 0.1);
        self.health_ecg.tick(dt, snapshot.hp, snapshot.max_hp);
        // Smooth turntable rotation (`item_screen_obj.c`)
        self.item_yaw_rad = (self.item_yaw_rad + dt * 0.9) % std::f32::consts::TAU;
        // Scale lerp toward 1.0 (`itemmainmain` 0x001ce188..0x001ce1bc)
        if self.selected_scale < CAROUSEL_SELECTED_SCALE {
            self.selected_scale = (self.selected_scale + CAROUSEL_SCALE_STEP * (dt * 30.0))
                .min(CAROUSEL_SELECTED_SCALE);
        }
        if self.status_toast_timer > 0.0 {
            self.status_toast_timer = (self.status_toast_timer - dt).max(0.0);
            if self.status_toast_timer == 0.0 {
                self.status_toast = None;
            }
        }
    }

    /// Displays a transient status message in the inventory footer.
    pub fn show_toast(&mut self, msg: impl Into<String>) {
        self.status_toast = Some(msg.into());
        self.status_toast_timer = 2.5;
    }

    /// Builds the authentic command pop-up options for `item_id` (`command_kind` @ `0x001d3610`).
    pub fn commands_for_item(
        &self,
        item_id: u8,
        inventory: &InventoryManager,
        snapshot: &InventoryPlayerSnapshot,
    ) -> Vec<ItemCommand> {
        let Some(meta) = item_metadata(item_id) else {
            return vec![ItemCommand::Look];
        };
        let mut cmds = Vec::with_capacity(3);
        match meta.category {
            ItemCategory::Consumable => {
                // `command_kind` group 2 (`[1, 2, 3]`): Use / Look
                cmds.push(ItemCommand::Use);
                cmds.push(ItemCommand::Look);
            }
            ItemCategory::Weapon => {
                // `command_kind` group 3 & 4: Equip / Unequip, plus Reload if firearm has reserve ammo
                if snapshot.equipped_weapon_item_id == Some(item_id) {
                    cmds.push(ItemCommand::Unequip);
                } else {
                    cmds.push(ItemCommand::Equip);
                }
                if let Some(ammo_id) = ammo_for_firearm(item_id) {
                    if inventory.count(ammo_id) > 0 {
                        cmds.push(ItemCommand::Reload);
                    }
                }
                cmds.push(ItemCommand::Look);
            }
            ItemCategory::Ammunition => {
                // `command_kind` group 5 (`[5, 7, 9]` + `11`): Reload (if firearm owned) / Look
                if let Some(firearm_id) = firearm_for_ammo(item_id) {
                    if inventory.has_item(firearm_id) {
                        cmds.push(ItemCommand::Reload);
                    }
                }
                cmds.push(ItemCommand::Look);
            }
            ItemCategory::Equipment => {
                // `command_kind` group 6 (`15` Flashlight) & group 7 (`16` Radio): ON/OFF + Look
                if item_id == 16 || item_id == 18 {
                    cmds.push(if snapshot.flashlight_on {
                        ItemCommand::TurnOff
                    } else {
                        ItemCommand::TurnOn
                    });
                } else if item_id == 17 || item_id == 19 {
                    cmds.push(if snapshot.radio_on {
                        ItemCommand::TurnOff
                    } else {
                        ItemCommand::TurnOn
                    });
                } else {
                    cmds.push(ItemCommand::Use);
                }
                cmds.push(ItemCommand::Look);
            }
            ItemCategory::KeyItem | ItemCategory::PuzzleItem | ItemCategory::SubScenarioItem => {
                // `command_kind` group 1 & 10: Use / Look / Combine (3-command window!)
                cmds.push(ItemCommand::Use);
                cmds.push(ItemCommand::Look);
                cmds.push(ItemCommand::Combine);
            }
            ItemCategory::SecretItem => {
                // `command_kind` group 8 & 9: Look / Use
                cmds.push(ItemCommand::Look);
                cmds.push(ItemCommand::Use);
            }
        }
        cmds
    }

    /// Handles directional / confirm / cancel navigation input from gamepad or keyboard.
    pub fn handle_nav(
        &mut self,
        nav: MenuNavAction,
        inventory: &InventoryManager,
        snapshot: &InventoryPlayerSnapshot,
    ) -> (MenuSoundCue, InventoryAction) {
        if !self.is_open {
            return (MenuSoundCue::None, InventoryAction::None);
        }

        match self.focus {
            InventoryFocus::Carousel => self.handle_carousel_nav(nav, inventory, snapshot),
            InventoryFocus::Corner(btn) => self.handle_corner_nav(btn, nav),
            InventoryFocus::CommandWindow => {
                self.handle_command_window_nav(nav, inventory, snapshot, false)
            }
            InventoryFocus::CombineTarget => self.handle_combine_nav(nav),
            InventoryFocus::ExamineOverlay => match nav {
                MenuNavAction::Confirm | MenuNavAction::Cancel => {
                    self.focus = InventoryFocus::Carousel;
                    (MenuSoundCue::Cancel, InventoryAction::None)
                }
                _ => (MenuSoundCue::None, InventoryAction::None),
            },
        }
    }

    /// Handles mouse hover movement in viewport pixel coordinates (`CommandWindowMouseFix.cpp`).
    pub fn handle_mouse_move(
        &mut self,
        mouse_x: f32,
        mouse_y: f32,
        viewport_w: f32,
        viewport_h: f32,
    ) {
        if !self.is_open || viewport_w <= 0.0 || viewport_h <= 0.0 {
            return;
        }
        let lx = mouse_x * 640.0 / viewport_w;
        let ly = mouse_y * 480.0 / viewport_h;

        if self.focus == InventoryFocus::CommandWindow {
            // Exact SH2:EE `CommandWindowMouseFix.cpp` (`0x00472428`) hitbox for 2- and 3-option windows
            if (CMD_WIN_MOUSE_X_MIN..=CMD_WIN_MOUSE_X_MAX).contains(&lx)
                && (CMD_WIN_MOUSE_Y_MIN..=CMD_WIN_MOUSE_Y_MAX).contains(&ly)
            {
                let row = if ly < CMD_WIN_ROW_0_MAX_Y {
                    0
                } else if ly < CMD_WIN_ROW_1_MAX_Y {
                    1
                } else {
                    2
                };
                if row < self.active_commands.len() {
                    self.command_index = row;
                }
            }
            return;
        }

        // Corner button hover highlight when in Carousel or Corner focus
        if matches!(
            self.focus,
            InventoryFocus::Carousel | InventoryFocus::Corner(_)
        ) {
            for btn in CornerButton::ALL {
                let (x0, y0, x1, y1) = btn.hitbox_640x480();
                if lx >= x0 && lx <= x1 && ly >= y0 && ly <= y1 {
                    self.focus = InventoryFocus::Corner(btn);
                    return;
                }
            }
        }
    }

    /// Handles a left mouse click in viewport pixel coordinates.
    pub fn handle_mouse_click(
        &mut self,
        mouse_x: f32,
        mouse_y: f32,
        viewport_w: f32,
        viewport_h: f32,
        inventory: &InventoryManager,
        snapshot: &InventoryPlayerSnapshot,
    ) -> (MenuSoundCue, InventoryAction) {
        if !self.is_open || viewport_w <= 0.0 || viewport_h <= 0.0 {
            return (MenuSoundCue::None, InventoryAction::None);
        }
        self.handle_mouse_move(mouse_x, mouse_y, viewport_w, viewport_h);
        let lx = mouse_x * 640.0 / viewport_w;
        let ly = mouse_y * 480.0 / viewport_h;

        if self.focus == InventoryFocus::CommandWindow {
            if (CMD_WIN_MOUSE_X_MIN..=CMD_WIN_MOUSE_X_MAX).contains(&lx)
                && (CMD_WIN_MOUSE_Y_MIN..=CMD_WIN_MOUSE_Y_MAX).contains(&ly)
            {
                return self.handle_command_window_nav(
                    MenuNavAction::Confirm,
                    inventory,
                    snapshot,
                    true,
                );
            }
            self.focus = InventoryFocus::Carousel;
            return (MenuSoundCue::Cancel, InventoryAction::None);
        }

        for btn in CornerButton::ALL {
            let (x0, y0, x1, y1) = btn.hitbox_640x480();
            if lx >= x0 && lx <= x1 && ly >= y0 && ly <= y1 {
                self.focus = InventoryFocus::Corner(btn);
                return self.handle_corner_nav(btn, MenuNavAction::Confirm);
            }
        }

        // Clicking center carousel opens the command window
        if (150.0..=340.0).contains(&ly) && !self.owned_items.is_empty() {
            return self.handle_carousel_nav(MenuNavAction::Confirm, inventory, snapshot);
        }

        (MenuSoundCue::None, InventoryAction::None)
    }

    fn handle_carousel_nav(
        &mut self,
        nav: MenuNavAction,
        inventory: &InventoryManager,
        snapshot: &InventoryPlayerSnapshot,
    ) -> (MenuSoundCue, InventoryAction) {
        match nav {
            MenuNavAction::Left => {
                if !self.owned_items.is_empty() {
                    let len = self.owned_items.len();
                    self.selected_index = (self.selected_index + len - 1) % len;
                    self.selected_scale = CAROUSEL_UNSELECTED_SCALE;
                    (MenuSoundCue::CursorMove, InventoryAction::None)
                } else {
                    (MenuSoundCue::None, InventoryAction::None)
                }
            }
            MenuNavAction::Right => {
                if !self.owned_items.is_empty() {
                    self.selected_index = (self.selected_index + 1) % self.owned_items.len();
                    self.selected_scale = CAROUSEL_UNSELECTED_SCALE;
                    (MenuSoundCue::CursorMove, InventoryAction::None)
                } else {
                    (MenuSoundCue::None, InventoryAction::None)
                }
            }
            MenuNavAction::Up => {
                self.focus = InventoryFocus::Corner(CornerButton::Map);
                (MenuSoundCue::CursorMove, InventoryAction::None)
            }
            MenuNavAction::Down => {
                self.focus = InventoryFocus::Corner(CornerButton::Exit);
                (MenuSoundCue::CursorMove, InventoryAction::None)
            }
            MenuNavAction::Confirm => {
                if let Some(item_id) = self.selected_item_id() {
                    self.active_commands = self.commands_for_item(item_id, inventory, snapshot);
                    self.command_index = 0;
                    self.focus = InventoryFocus::CommandWindow;
                    (MenuSoundCue::Confirm, InventoryAction::None)
                } else {
                    (MenuSoundCue::Cancel, InventoryAction::None)
                }
            }
            MenuNavAction::Cancel => {
                self.close();
                (MenuSoundCue::Cancel, InventoryAction::Close)
            }
        }
    }

    fn handle_corner_nav(
        &mut self,
        btn: CornerButton,
        nav: MenuNavAction,
    ) -> (MenuSoundCue, InventoryAction) {
        match nav {
            MenuNavAction::Left => {
                let next = match btn {
                    CornerButton::Map => CornerButton::Memo,
                    CornerButton::Memo => CornerButton::Map,
                    CornerButton::Exit => CornerButton::Option,
                    CornerButton::Option => CornerButton::Exit,
                };
                self.focus = InventoryFocus::Corner(next);
                (MenuSoundCue::CursorMove, InventoryAction::None)
            }
            MenuNavAction::Right => {
                let next = match btn {
                    CornerButton::Memo => CornerButton::Map,
                    CornerButton::Map => CornerButton::Memo,
                    CornerButton::Option => CornerButton::Exit,
                    CornerButton::Exit => CornerButton::Option,
                };
                self.focus = InventoryFocus::Corner(next);
                (MenuSoundCue::CursorMove, InventoryAction::None)
            }
            MenuNavAction::Up | MenuNavAction::Down => {
                self.focus = InventoryFocus::Carousel;
                (MenuSoundCue::CursorMove, InventoryAction::None)
            }
            MenuNavAction::Confirm => match btn {
                CornerButton::Memo => (MenuSoundCue::Confirm, InventoryAction::OpenMemo),
                CornerButton::Map => (MenuSoundCue::Confirm, InventoryAction::OpenMap),
                CornerButton::Option => (MenuSoundCue::Confirm, InventoryAction::OpenOptions),
                CornerButton::Exit => {
                    self.close();
                    (MenuSoundCue::Cancel, InventoryAction::Close)
                }
            },
            MenuNavAction::Cancel => {
                self.focus = InventoryFocus::Carousel;
                (MenuSoundCue::Cancel, InventoryAction::None)
            }
        }
    }

    fn handle_command_window_nav(
        &mut self,
        nav: MenuNavAction,
        inventory: &InventoryManager,
        _snapshot: &InventoryPlayerSnapshot,
        from_mouse: bool,
    ) -> (MenuSoundCue, InventoryAction) {
        match nav {
            MenuNavAction::Up => {
                if !self.active_commands.is_empty() {
                    let len = self.active_commands.len();
                    self.command_index = (self.command_index + len - 1) % len;
                }
                (MenuSoundCue::CursorMove, InventoryAction::None)
            }
            MenuNavAction::Down => {
                if !self.active_commands.is_empty() {
                    self.command_index = (self.command_index + 1) % self.active_commands.len();
                }
                (MenuSoundCue::CursorMove, InventoryAction::None)
            }
            MenuNavAction::Left | MenuNavAction::Right => {
                (MenuSoundCue::None, InventoryAction::None)
            }
            MenuNavAction::Cancel => {
                self.focus = InventoryFocus::Carousel;
                (MenuSoundCue::Cancel, InventoryAction::None)
            }
            MenuNavAction::Confirm => {
                let Some(item_id) = self.selected_item_id() else {
                    self.focus = InventoryFocus::Carousel;
                    return (MenuSoundCue::Cancel, InventoryAction::None);
                };
                let Some(&cmd) = self.active_commands.get(self.command_index) else {
                    self.focus = InventoryFocus::Carousel;
                    return (MenuSoundCue::Cancel, InventoryAction::None);
                };
                match cmd {
                    ItemCommand::Use => {
                        self.focus = InventoryFocus::Carousel;
                        if matches!(item_id, 1..=3) {
                            (MenuSoundCue::Confirm, InventoryAction::UseHealing(item_id))
                        } else {
                            (
                                MenuSoundCue::Confirm,
                                InventoryAction::UseKeyOrPuzzleItem(item_id),
                            )
                        }
                    }
                    ItemCommand::Equip => {
                        self.focus = InventoryFocus::Carousel;
                        (MenuSoundCue::Confirm, InventoryAction::EquipWeapon(item_id))
                    }
                    ItemCommand::Unequip => {
                        self.focus = InventoryFocus::Carousel;
                        (MenuSoundCue::Confirm, InventoryAction::UnequipWeapon)
                    }
                    ItemCommand::Reload => {
                        self.focus = InventoryFocus::Carousel;
                        if let Some(firearm_id) = firearm_for_ammo(item_id) {
                            (
                                MenuSoundCue::Confirm,
                                InventoryAction::ReloadFirearm {
                                    ammo_id: item_id,
                                    firearm_id,
                                },
                            )
                        } else if let Some(ammo_id) = ammo_for_firearm(item_id) {
                            if inventory.count(ammo_id) > 0 {
                                (
                                    MenuSoundCue::Confirm,
                                    InventoryAction::ReloadFirearm {
                                        ammo_id,
                                        firearm_id: item_id,
                                    },
                                )
                            } else {
                                self.show_toast("No ammunition remaining.");
                                (MenuSoundCue::Cancel, InventoryAction::None)
                            }
                        } else {
                            (MenuSoundCue::Cancel, InventoryAction::None)
                        }
                    }
                    ItemCommand::Look => {
                        self.focus = InventoryFocus::ExamineOverlay;
                        (MenuSoundCue::Confirm, InventoryAction::None)
                    }
                    ItemCommand::Combine => {
                        self.combine_first_item_id = Some(item_id);
                        // SH2:EE `SkipResetSelectionAfterCombineASM` (`CommandWindowMouseFix.cpp:81`):
                        // preserve command_index for mouse clicks, reset to 0 only for keyboard/pad.
                        if !from_mouse {
                            self.command_index = 0;
                        }
                        self.focus = InventoryFocus::CombineTarget;
                        self.show_toast("Select another item to combine.");
                        (MenuSoundCue::Confirm, InventoryAction::None)
                    }
                    ItemCommand::TurnOn | ItemCommand::TurnOff => {
                        self.focus = InventoryFocus::Carousel;
                        if item_id == 16 || item_id == 18 {
                            (MenuSoundCue::Confirm, InventoryAction::ToggleFlashlight)
                        } else {
                            (MenuSoundCue::Confirm, InventoryAction::ToggleRadio)
                        }
                    }
                }
            }
        }
    }

    fn handle_combine_nav(&mut self, nav: MenuNavAction) -> (MenuSoundCue, InventoryAction) {
        match nav {
            MenuNavAction::Left => {
                if !self.owned_items.is_empty() {
                    let len = self.owned_items.len();
                    self.selected_index = (self.selected_index + len - 1) % len;
                }
                (MenuSoundCue::CursorMove, InventoryAction::None)
            }
            MenuNavAction::Right => {
                if !self.owned_items.is_empty() {
                    self.selected_index = (self.selected_index + 1) % self.owned_items.len();
                }
                (MenuSoundCue::CursorMove, InventoryAction::None)
            }
            MenuNavAction::Cancel => {
                self.combine_first_item_id = None;
                self.focus = InventoryFocus::Carousel;
                (MenuSoundCue::Cancel, InventoryAction::None)
            }
            MenuNavAction::Confirm => {
                let first = self.combine_first_item_id.take();
                let second = self.selected_item_id();
                self.focus = InventoryFocus::Carousel;
                if let (Some(a), Some(b)) = (first, second) {
                    if a != b {
                        if let Some(result_id) = combine_items(a, b, 0) {
                            return (
                                MenuSoundCue::Confirm,
                                InventoryAction::CombinedItems {
                                    first_id: a,
                                    second_id: b,
                                    result_id,
                                },
                            );
                        }
                    }
                }
                self.show_toast("These items cannot be combined.");
                (MenuSoundCue::Cancel, InventoryAction::None)
            }
            _ => (MenuSoundCue::None, InventoryAction::None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inventory_command_window_mouse_fix_and_reload() {
        let mut inv = InventoryManager::new();
        inv.item_get_now(4); // Handgun
        inv.item_get_now(5); // Handgun bullets
        inv.item_get_now(24); // Key item (3 commands: Use, Look, Combine)

        let snap = InventoryPlayerSnapshot::default();
        let mut screen = InventoryScreen::new();
        screen.open(&inv);
        assert_eq!(screen.owned_items, vec![4, 5, 24]);

        // Select Handgun (#4) -> Equip, Reload, Look
        let cmds = screen.commands_for_item(4, &inv, &snap);
        assert_eq!(
            cmds,
            vec![ItemCommand::Equip, ItemCommand::Reload, ItemCommand::Look]
        );

        // Open command window on key item (#24) -> 3 options
        screen.selected_index = 2;
        screen.handle_nav(MenuNavAction::Confirm, &inv, &snap);
        assert_eq!(screen.focus, InventoryFocus::CommandWindow);
        assert_eq!(screen.active_commands.len(), 3);

        // Test SH2:EE CommandWindowMouseFix 3-option hitboxes (640x480 space)
        screen.handle_mouse_move(390.0, 75.0, 640.0, 480.0);
        assert_eq!(screen.command_index, 0);
        screen.handle_mouse_move(390.0, 100.0, 640.0, 480.0);
        assert_eq!(screen.command_index, 1);
        screen.handle_mouse_move(390.0, 128.0, 640.0, 480.0);
        assert_eq!(screen.command_index, 2);

        // Clicking Combine preserves command_index = 2 (`SkipResetSelectionAfterCombineASM`)
        let (cue, _) = screen.handle_mouse_click(390.0, 128.0, 640.0, 480.0, &inv, &snap);
        assert_eq!(cue, MenuSoundCue::Confirm);
        assert_eq!(screen.focus, InventoryFocus::CombineTarget);
        assert_eq!(screen.command_index, 2);
    }

    #[test]
    fn test_inventory_corner_buttons_and_ammo_reload_action() {
        let mut inv = InventoryManager::new();
        inv.item_get_now(4); // Handgun
        inv.item_get_now(5); // Handgun bullets
        let snap = InventoryPlayerSnapshot::default();
        let mut screen = InventoryScreen::new();
        screen.open(&inv);

        // Move to bullets (#5), open command window, confirm Reload
        screen.handle_nav(MenuNavAction::Right, &inv, &snap);
        assert_eq!(screen.selected_item_id(), Some(5));
        screen.handle_nav(MenuNavAction::Confirm, &inv, &snap);
        let (_, action) = screen.handle_nav(MenuNavAction::Confirm, &inv, &snap);
        assert_eq!(
            action,
            InventoryAction::ReloadFirearm {
                ammo_id: 5,
                firearm_id: 4,
            }
        );

        // Navigate up to corner button (MAP) and confirm
        screen.handle_nav(MenuNavAction::Up, &inv, &snap);
        assert_eq!(screen.focus, InventoryFocus::Corner(CornerButton::Map));
        let (_, map_action) = screen.handle_nav(MenuNavAction::Confirm, &inv, &snap);
        assert_eq!(map_action, InventoryAction::OpenMap);
    }
}
