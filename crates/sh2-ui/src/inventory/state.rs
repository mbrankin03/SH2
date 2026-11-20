//! State machine & input navigation for [`InventoryScreen`] (`Item/otn_itemmain.c`).

use crate::inventory::health_ecg::HealthEcgState;
use crate::inventory::{
    CornerButton, InventoryAction, InventoryFocus, InventoryPlayerSnapshot, ItemCommand,
    CAROUSEL_SCALE_STEP, CAROUSEL_SELECTED_SCALE, CAROUSEL_UNSELECTED_SCALE, CMD_WIN_MOUSE_X_MAX,
    CMD_WIN_MOUSE_X_MIN, CMD_WIN_MOUSE_Y_MAX, CMD_WIN_MOUSE_Y_MIN, CMD_WIN_ROW_0_MAX_Y,
    CMD_WIN_ROW_1_MAX_Y,
};
use crate::menu::{MenuNavAction, MenuSoundCue};
use sh2_core::inventory::{
    ammo_for_firearm, combine_items, firearm_for_ammo, is_equippable_weapon_item, item_metadata,
    InventoryManager, ItemCategory, CATALOG_ITEM_COUNT, ITEM_ID_RADIO,
};
use sh2_core::lighting::darkness::ITEM_ID_FLASHLIGHT;

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
    /// Most recently combined pseudo-item (`86..=96`) ready for contextual `Use`.
    pub active_combo_id: Option<u8>,
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
            active_combo_id: None,
            item_yaw_rad: 0.0,
            selected_scale: CAROUSEL_UNSELECTED_SCALE,
            health_ecg: HealthEcgState::new(),
            status_toast: None,
            status_toast_timer: 0.0,
        }
    }

    /// Opens the Status / Inventory screen, refreshing the owned item carousel from `inventory`.
    /// Per `PatchInventoryBGM.cpp` (`0x005166e8`), opening Inventory freezes world simulation
    /// while preserving ambient room BGM playback.
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
        // 30 Hz PS2 step: `CAROUSEL_SCALE_STEP` (`0.1`) per 1/30s frame (`itemmainmain` `0x001ce1a4`)
        self.item_yaw_rad = (self.item_yaw_rad + dt * 0.9) % std::f32::consts::TAU;
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
        if is_equippable_weapon_item(item_id) {
            // Covers both standard weapons and secret weapons (`12` Hyper Spray, `16` Chainsaw)
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
            return cmds;
        }
        match meta.category {
            ItemCategory::Consumable => {
                cmds.push(ItemCommand::Use);
                cmds.push(ItemCommand::Look);
            }
            ItemCategory::Ammunition => {
                if let Some(firearm_id) = firearm_for_ammo(item_id) {
                    if inventory.has_item(firearm_id) {
                        cmds.push(ItemCommand::Reload);
                    }
                }
                cmds.push(ItemCommand::Look);
            }
            ItemCategory::Equipment => {
                if item_id == ITEM_ID_FLASHLIGHT {
                    cmds.push(if snapshot.flashlight_on {
                        ItemCommand::TurnOff
                    } else {
                        ItemCommand::TurnOn
                    });
                } else if item_id == ITEM_ID_RADIO {
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
                cmds.push(ItemCommand::Use);
                cmds.push(ItemCommand::Look);
                cmds.push(ItemCommand::Combine);
            }
            ItemCategory::Weapon | ItemCategory::SecretItem => {
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
            InventoryFocus::Corner(btn) => self.handle_corner_nav(btn, nav, snapshot),
            InventoryFocus::CommandWindow => self.handle_command_window_nav(nav, inventory, false),
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
            // `CommandWindowMouseFix.cpp` (`0x00472428`) uses 512x448 PS2/PC logical coordinates
            // (`x in 356..439`, `y in 63..139`), which map directly onto the right-hand `Command` column.
            let ps2_x = mouse_x * 512.0 / viewport_w;
            let ps2_y = mouse_y * 448.0 / viewport_h;
            if (CMD_WIN_MOUSE_X_MIN..=CMD_WIN_MOUSE_X_MAX).contains(&ps2_x)
                && (CMD_WIN_MOUSE_Y_MIN..=CMD_WIN_MOUSE_Y_MAX).contains(&ps2_y)
            {
                let row = if ps2_y < CMD_WIN_ROW_0_MAX_Y {
                    0
                } else if ps2_y < CMD_WIN_ROW_1_MAX_Y {
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
            let ps2_x = mouse_x * 512.0 / viewport_w;
            let ps2_y = mouse_y * 448.0 / viewport_h;
            if (CMD_WIN_MOUSE_X_MIN..=CMD_WIN_MOUSE_X_MAX).contains(&ps2_x)
                && (CMD_WIN_MOUSE_Y_MIN..=CMD_WIN_MOUSE_Y_MAX).contains(&ps2_y)
            {
                return self.handle_command_window_nav(MenuNavAction::Confirm, inventory, true);
            }
            self.focus = InventoryFocus::Carousel;
            return (MenuSoundCue::Cancel, InventoryAction::None);
        }

        for btn in CornerButton::ALL {
            let (x0, y0, x1, y1) = btn.hitbox_640x480();
            if lx >= x0 && lx <= x1 && ly >= y0 && ly <= y1 {
                self.focus = InventoryFocus::Corner(btn);
                return self.handle_corner_nav(btn, MenuNavAction::Confirm, snapshot);
            }
        }

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
            MenuNavAction::Up | MenuNavAction::Down => {
                self.focus = InventoryFocus::Corner(CornerButton::Memo);
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
        snapshot: &InventoryPlayerSnapshot,
    ) -> (MenuSoundCue, InventoryAction) {
        match nav {
            MenuNavAction::Left => {
                let next = match btn {
                    CornerButton::Option => CornerButton::Map,
                    CornerButton::Memo => CornerButton::Option,
                    CornerButton::Map => CornerButton::Memo,
                };
                self.focus = InventoryFocus::Corner(next);
                (MenuSoundCue::CursorMove, InventoryAction::None)
            }
            MenuNavAction::Right => {
                let next = match btn {
                    CornerButton::Option => CornerButton::Memo,
                    CornerButton::Memo => CornerButton::Map,
                    CornerButton::Map => CornerButton::Option,
                };
                self.focus = InventoryFocus::Corner(next);
                (MenuSoundCue::CursorMove, InventoryAction::None)
            }
            MenuNavAction::Up | MenuNavAction::Down => {
                self.focus = InventoryFocus::Carousel;
                (MenuSoundCue::CursorMove, InventoryAction::None)
            }
            MenuNavAction::Confirm => match btn {
                CornerButton::Memo => {
                    // Gate on `MemoCommandCheck` (`0x00230570`)
                    if snapshot.has_any_memo {
                        (MenuSoundCue::Confirm, InventoryAction::OpenMemo)
                    } else {
                        self.show_toast("No memos collected yet.");
                        (MenuSoundCue::Cancel, InventoryAction::None)
                    }
                }
                CornerButton::Map => (MenuSoundCue::Confirm, InventoryAction::OpenMap),
                CornerButton::Option => (MenuSoundCue::Confirm, InventoryAction::OpenOptions),
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
                        if !from_mouse {
                            self.command_index = 0;
                        }
                        self.focus = InventoryFocus::CombineTarget;
                        self.show_toast("Select another item to combine.");
                        (MenuSoundCue::Confirm, InventoryAction::None)
                    }
                    ItemCommand::TurnOn | ItemCommand::TurnOff => {
                        self.focus = InventoryFocus::Carousel;
                        if item_id == ITEM_ID_FLASHLIGHT {
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
                            self.active_combo_id = Some(result_id);
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
    fn test_inventory_command_window_mouse_fix_and_secret_weapons() {
        let mut inv = InventoryManager::new();
        inv.item_get_now(4); // Handgun
        inv.item_get_now(5); // Handgun bullets
        inv.item_get_now(16); // Chainsaw (SecretItem, still equippable!)
        inv.item_get_now(18); // Flashlight
        inv.item_get_now(19); // Radio
        inv.item_get_now(24); // Key item (3 commands: Use, Look, Combine)

        let snap = InventoryPlayerSnapshot::default();
        let mut screen = InventoryScreen::new();
        screen.open(&inv);

        // Chainsaw (#16) must offer Equip & Look, NOT Flashlight ON/OFF!
        assert_eq!(
            screen.commands_for_item(16, &inv, &snap),
            vec![ItemCommand::Equip, ItemCommand::Look]
        );
        // Flashlight (#18) offers ON, Look; Radio (#19) offers OFF, Look
        assert_eq!(
            screen.commands_for_item(18, &inv, &snap),
            vec![ItemCommand::TurnOn, ItemCommand::Look]
        );
        assert_eq!(
            screen.commands_for_item(19, &inv, &snap),
            vec![ItemCommand::TurnOff, ItemCommand::Look]
        );

        // Open command window on key item (#24) -> 3 options
        screen.selected_index = 5;
        screen.handle_nav(MenuNavAction::Confirm, &inv, &snap);
        assert_eq!(screen.focus, InventoryFocus::CommandWindow);
        assert_eq!(screen.active_commands.len(), 3);

        // SH2:EE CommandWindowMouseFix 3-option hitboxes (512x448 PS2/PC logical space)
        screen.handle_mouse_move(390.0, 75.0, 512.0, 448.0);
        assert_eq!(screen.command_index, 0);
        screen.handle_mouse_move(390.0, 100.0, 512.0, 448.0);
        assert_eq!(screen.command_index, 1);
        screen.handle_mouse_move(390.0, 128.0, 512.0, 448.0);
        assert_eq!(screen.command_index, 2);

        let (cue, _) = screen.handle_mouse_click(390.0, 128.0, 512.0, 448.0, &inv, &snap);
        assert_eq!(cue, MenuSoundCue::Confirm);
        assert_eq!(screen.focus, InventoryFocus::CombineTarget);
        assert_eq!(screen.command_index, 2);
    }

    #[test]
    fn test_inventory_memo_gating_and_revolver_reload_action() {
        let mut inv = InventoryManager::new();
        inv.item_get_now(10); // Maria's Revolver
        inv.item_get_now(11); // Revolver bullets
        let snap = InventoryPlayerSnapshot {
            has_any_memo: false,
            ..Default::default()
        };
        let mut screen = InventoryScreen::new();
        screen.open(&inv);

        // Revolver bullets (#11) reload Revolver (#10)
        screen.handle_nav(MenuNavAction::Right, &inv, &snap);
        screen.handle_nav(MenuNavAction::Confirm, &inv, &snap);
        let (_, action) = screen.handle_nav(MenuNavAction::Confirm, &inv, &snap);
        assert_eq!(
            action,
            InventoryAction::ReloadFirearm {
                ammo_id: 11,
                firearm_id: 10,
            }
        );

        // CornerButton::Memo is gated when has_any_memo == false
        screen.focus = InventoryFocus::Corner(CornerButton::Memo);
        let (cue, memo_act) = screen.handle_nav(MenuNavAction::Confirm, &inv, &snap);
        assert_eq!(cue, MenuSoundCue::Cancel);
        assert_eq!(memo_act, InventoryAction::None);
    }
}
