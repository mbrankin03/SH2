//! State machine & input navigation for [`InventoryScreen`] (`Item/otn_itemmain.c`).

use crate::inventory::health_ecg::HealthEcgState;
use crate::inventory::widgets::{
    move_near, ITEM_ALLAY_FADE_STEP, ITEM_ALLAY_MAX_FADE, ITEM_ALLAY_PULSE_MAX,
    ITEM_ALLAY_PULSE_MIN, ITEM_ALLAY_PULSE_RATE, LOOK_COMBINE_FADE_RATE, LOOK_COMMAND_GLOW_STEP,
    LOOK_COMMAND_MAX_GLOW, RADIO_VOLUME_BARS,
};
use crate::inventory::{
    CornerButton, ExaminePhase, InventoryAction, InventoryFadeTransition, InventoryFocus,
    InventoryPlayerSnapshot, ItemCommand, CAROUSEL_SCALE_STEP, CAROUSEL_SELECTED_SCALE,
    CAROUSEL_UNSELECTED_SCALE, CMD_WIN_MOUSE_X_MAX, CMD_WIN_MOUSE_X_MIN, CMD_WIN_MOUSE_Y_MAX,
    CMD_WIN_MOUSE_Y_MIN, CMD_WIN_ROW_0_MAX_Y, CMD_WIN_ROW_1_MAX_Y, DEFAULT_RADIO_VOLUME,
};
use crate::menu::{MenuNavAction, MenuSoundCue};
use sh2_core::inventory::{
    ammo_for_firearm, combine_items, firearm_for_ammo, is_equippable_weapon_item, item_metadata,
    InventoryManager, ItemCategory, CATALOG_ITEM_COUNT, ITEM_COMBOS, ITEM_ID_RADIO,
};
use sh2_core::lighting::darkness::ITEM_ID_FLASHLIGHT;

/// Inline examine open rect in `512x448` PS2 space (`sh2pc.exe` `FUN_0046af30` `case 0xc`:
/// `(-100, -150) .. (+100, +50)` centered -> `(156, 74) .. (356, 274)`).
pub const EXAMINE_POPUP_OPEN_RECT: [f32; 4] = [156.0, 74.0, 356.0, 274.0];

/// Inline examine dismiss flip-collapse slit rect in `512x448` PS2 space (`sh2pc.exe` `FUN_0046af30`
/// `case 0xd`: `(-55, -52) .. (+55, -48)` centered -> `(201, 172) .. (311, 176)`, 4 px tall).
pub const EXAMINE_POPUP_DISMISS_SLIT_RECT: [f32; 4] = [201.0, 172.0, 311.0, 176.0];

/// Command Window focus rect in `512x448` PS2 space (`sh2pc.exe` `FUN_0046af30` `case 5`:
/// `(336, 21) .. (496, 144)`), used as the starting quad when opening an inline examine popup.
pub const EXAMINE_POPUP_CMD_START_RECT: [f32; 4] = [336.0, 21.0, 496.0, 144.0];

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
    /// Up to 3 staged combine preview slots (`_DAT_0116e644.._DAT_0116e648`, `look_combine` `0x001da490`).
    pub combine_slots: [Option<u8>; 3],
    /// Combine overlay fade (`0.0..=1.0`, `_DAT_0116e64c`, `±4.0/s`).
    pub combine_fade: f32,
    /// Most recently combined pseudo-item (`86..=96`) ready for contextual `Use`.
    pub active_combo_id: Option<u8>,
    /// 3D carousel turntable yaw angle in radians (`item_screen_obj.c`).
    pub item_yaw_rad: f32,
    /// Selected carousel item scale (`0.8..=1.0`, `itemmainmain` `0x001ce188`).
    pub selected_scale: f32,
    /// `item_allay` (`0x001d7640`) brightness triangle-wave phase (`_DAT_0116e614`, `0..350`, wraps to `50`).
    pub arrow_pulse: f32,
    /// `item_allay` (`0x001d7640`) fade alpha (`_DAT_0116e618`, `0.0..=32.0`, `±4.0` per 30 Hz tick).
    pub arrow_fade: f32,
    /// `sprite` (`0x001d8890`) active-tab pulse phase (`_DAT_0116e61c`, `0.0..2.0`, `+0.04` per 30 Hz tick).
    pub tab_pulse: f32,
    /// `look_command` (`0x001d8f50`) smoothly eased command row index (`_DAT_0116e3dc`, via `move_near`).
    pub command_cursor_y: f32,
    /// `look_command` (`0x001d8f50`) green-teal glow intensity (`_DAT_0116e3d8`, `0.0..=40.0`, `±6` per 30 Hz tick).
    pub command_glow: f32,
    /// Pocket Radio volume level (`0..=15`, `DAT_01126900`, default `0xc = 12` from `ItemDataInit` `0x001ad860`).
    pub radio_volume: u8,
    /// Top-left Health ECG Status Box animation state (`look_hp` `0x001d9660`).
    pub health_ecg: HealthEcgState,
    /// Optional status feedback toast message shown briefly in the footer.
    pub status_toast: Option<String>,
    /// Remaining duration in seconds for `status_toast`.
    pub status_toast_timer: f32,
    /// Native-upgrade toggle: allow free 3D item orbit & zoom in `ExamineOverlay` (`false` = 1:1 PS2).
    pub enable_free_item_orbit: bool,
    /// Free-orbit yaw in radians when `enable_free_item_orbit` is enabled.
    pub examine_yaw_rad: f32,
    /// Free-orbit pitch in radians when `enable_free_item_orbit` is enabled.
    pub examine_pitch_rad: f32,
    /// Free-orbit zoom multiplier `[0.5, 2.5]` when `enable_free_item_orbit` is enabled.
    pub examine_zoom: f32,
    /// Active page index in multi-page `\x0c`-delimited `examine_text` (`EvSubMessage` pagination).
    pub examine_page: usize,
    /// Active `ExamineOverlay` phase (`InlinePopup` vs 2-phase `examine_main` `0x001d3a00`).
    pub examine_phase: ExaminePhase,
    /// Fullscreen examine image brightness `_examine_rgb` (`0x80 = 128.0` -> `0x45 = 69.0` at `-300.0 * dt`,
    /// `SLUS_202.28` `examine_main` `0x001d3a00` lines 1678–1681).
    pub examine_rgb: f32,
    /// Animated 4-edge quad `[x0, y0, x1, y1]` in `512x448` PS2 space for `InlinePopup` /
    /// `InlineClosing` (`sh2pc.exe` `FUN_0046af30` `case 0xc <-> case 0xd` via `move_near(2.0, 1.0)`).
    pub examine_popup_rect: [f32; 4],
    /// Front (`0`) vs back (`1`) side during 2-sided Wood Side coin examine (`sh2pc.exe`
    /// `sh2pc_00460000.c:4874-4876`: retail PC IDs `0x32..=0x34` = Snake / Old Man / Prisoner,
    /// shifted `+3` from `SLUS_202.28` `otn_itemmain.c:646-666` `0x2f..=0x31`).
    pub examine_coin_side: u8,
    /// Smooth carousel angular scroll offset in slots (`sh2pc.exe` `FUN_0046bcf0`, eased to `0.0`
    /// via `move_near(20.0, 110.0)` and clamped to `±1.5`).
    pub carousel_anim_offset: f32,
    /// Animated 4-edge focus reticle `[x0, y0, x1, y1]` in `512x448` PS2 space (`FUN_0046af30`).
    pub focus_rect: [f32; 4],
    /// Previous frame's `focus_rect` for `look_zanzo` (`0x001daab0`) motion-trail rendering.
    pub prev_focus_rect: [f32; 4],
    /// Inventory screen fade-to-black alpha (`1.0` = solid black, `0.0` = fully visible;
    /// `SLUS_202.28` `itemmain` `0x001cdb40:86` & `item_fade_out` `0x001db0e0:3740`: `0.5` s).
    pub screen_fade_alpha: f32,
    /// Pending fade-to-black screen transition (`DAT_0116e65c` in `otn_itemmain.c`).
    pub fade_transition: InventoryFadeTransition,
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
            combine_slots: [None; 3],
            combine_fade: 0.0,
            active_combo_id: None,
            item_yaw_rad: 0.0,
            selected_scale: CAROUSEL_UNSELECTED_SCALE,
            arrow_pulse: ITEM_ALLAY_PULSE_MIN,
            arrow_fade: ITEM_ALLAY_MAX_FADE,
            tab_pulse: 0.0,
            command_cursor_y: 0.0,
            command_glow: 0.0,
            radio_volume: DEFAULT_RADIO_VOLUME,
            health_ecg: HealthEcgState::new(),
            status_toast: None,
            status_toast_timer: 0.0,
            enable_free_item_orbit: false,
            examine_yaw_rad: 0.0,
            examine_pitch_rad: 0.0,
            examine_zoom: 1.0,
            examine_page: 0,
            examine_phase: ExaminePhase::InlinePopup,
            examine_rgb: 128.0,
            examine_popup_rect: EXAMINE_POPUP_DISMISS_SLIT_RECT,
            examine_coin_side: 0,
            carousel_anim_offset: 0.0,
            focus_rect: [201.0, 159.0, 311.0, 289.0],
            prev_focus_rect: [201.0, 159.0, 311.0, 289.0],
            screen_fade_alpha: 0.0,
            fade_transition: InventoryFadeTransition::None,
        }
    }

    /// Returns `true` when a fullscreen `examine_main` (`0x001d3a00`) overlay (`Letter from Mary`,
    /// `Laura's Letter`, Rebirth books) is active and the top-left 3D `Status` snapshot must be hidden.
    pub fn is_fullscreen_examine(&self) -> bool {
        self.focus == InventoryFocus::ExamineOverlay
            && matches!(
                self.examine_phase,
                ExaminePhase::FullscreenInspect | ExaminePhase::FullscreenReading
            )
    }

    /// Opens the Status / Inventory screen, refreshing the owned item carousel from `inventory`.
    /// Per `PatchInventoryBGM.cpp` (`0x005166e8`), opening Inventory freezes world simulation
    /// while preserving ambient room BGM playback.
    pub fn open(&mut self, inventory: &InventoryManager) {
        self.is_open = true;
        self.fade_transition = InventoryFadeTransition::None;
        self.screen_fade_alpha = 1.0;
        self.examine_popup_rect = EXAMINE_POPUP_DISMISS_SLIT_RECT;
        self.examine_coin_side = 0;
        self.carousel_anim_offset = 0.0;
        self.focus_rect = [201.0, 159.0, 311.0, 289.0];
        self.prev_focus_rect = self.focus_rect;
        self.focus = InventoryFocus::Carousel;
        self.active_commands.clear();
        self.command_index = 0;
        self.combine_first_item_id = None;
        self.combine_slots = [None; 3];
        self.combine_fade = 0.0;
        self.selected_scale = CAROUSEL_UNSELECTED_SCALE;
        self.arrow_pulse = ITEM_ALLAY_PULSE_MIN;
        self.arrow_fade = ITEM_ALLAY_MAX_FADE;
        self.tab_pulse = 0.0;
        self.command_cursor_y = 0.0;
        self.command_glow = 0.0;
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
        self.combine_slots = [None; 3];
        self.combine_fade = 0.0;
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

    /// Returns the exact `sh2pc.exe` `FUN_0046af30` target reticle rect `[x0, y0, x1, y1]` in
    /// `512x448` PS2 space for the current [`InventoryFocus`].
    #[must_use]
    pub fn target_focus_rect(&self) -> [f32; 4] {
        match self.focus {
            InventoryFocus::Carousel | InventoryFocus::CombineTarget => {
                [201.0, 159.0, 311.0, 289.0]
            }
            InventoryFocus::CommandWindow | InventoryFocus::RadioVolume => {
                [336.0, 21.0, 496.0, 144.0]
            }
            InventoryFocus::Corner(CornerButton::Option) => [16.0, 397.0, 176.0, 427.0],
            InventoryFocus::Corner(CornerButton::Memo) => [176.0, 397.0, 336.0, 427.0],
            InventoryFocus::Corner(CornerButton::Map) => [336.0, 397.0, 496.0, 427.0],
            InventoryFocus::ExamineOverlay => self.examine_popup_rect,
        }
    }

    /// Returns `true` when `focus_rect` has settled on `target_focus_rect()` (`DAT_0116e3e6 == 0`
    /// in `otn_itemmain.c:2611`), controlling visibility of the outer L-bracket corners.
    #[must_use]
    pub fn is_focus_rect_settled(&self) -> bool {
        let tgt = self.target_focus_rect();
        self.focus_rect
            .iter()
            .zip(tgt.iter())
            .all(|(cur, t)| (cur - t).abs() <= 0.5)
    }

    /// Advances ECG animation, 3D carousel rotation, scale lerp, `item_allay` pulse/fade,
    /// `sprite` tab pulse, `look_command` eased cursor/glow, `look_combine` fade, and `0.5` s
    /// `ScreenEffectFadeStart` transitions (`SLUS_202.28` `itemmainmain` `0x001cddd0`).
    ///
    /// Returns a deferred [`InventoryAction`] (e.g. `OpenOptions` / `OpenMemo` / `OpenMap`) once a
    /// `0.5` s fade-to-black subscreen transition finishes (`otn_itemmain.c:497, 530, 567`).
    pub fn tick(&mut self, dt: f32, snapshot: &InventoryPlayerSnapshot) -> Option<InventoryAction> {
        if !self.is_open {
            return None;
        }
        let dt = dt.clamp(0.0, 0.1);
        let ticks_30hz = dt * 30.0;
        self.health_ecg.tick(dt, snapshot.hp, snapshot.max_hp);
        // 30 Hz PS2 step: `CAROUSEL_SCALE_STEP` (`0.1`) per 1/30s frame (`itemmainmain` `0x001ce1a4`)
        self.item_yaw_rad = (self.item_yaw_rad + dt * 0.9) % std::f32::consts::TAU;
        if self.selected_scale < CAROUSEL_SELECTED_SCALE {
            self.selected_scale = (self.selected_scale + CAROUSEL_SCALE_STEP * ticks_30hz)
                .min(CAROUSEL_SELECTED_SCALE);
        }

        // `sprite` (`0x001d8890`) active tab pulse (`_DAT_0116e61c`: `+0.04` per 30 Hz tick, wraps at `2.0`)
        self.tab_pulse += 0.04 * ticks_30hz;
        if self.tab_pulse >= 2.0 {
            self.tab_pulse -= 2.0;
        }

        // `item_allay` (`0x001d7640`) arrow brightness triangle-wave (`50..350` at `180/s`) & fade (`0..32`)
        self.arrow_pulse += dt * ITEM_ALLAY_PULSE_RATE;
        if self.arrow_pulse > ITEM_ALLAY_PULSE_MAX {
            self.arrow_pulse = ITEM_ALLAY_PULSE_MIN;
        }
        if self.focus == InventoryFocus::Carousel {
            self.arrow_fade =
                (self.arrow_fade + ITEM_ALLAY_FADE_STEP * ticks_30hz).min(ITEM_ALLAY_MAX_FADE);
        } else {
            self.arrow_fade = (self.arrow_fade - ITEM_ALLAY_FADE_STEP * ticks_30hz).max(0.0);
        }

        // `look_command` (`0x001d8f50`) eased cursor Y via `move_near(2.0, 0.2, ...)` & glow (`0..40`)
        let cur_scaled = self.command_cursor_y * 1000.0;
        let tgt_scaled = (self.command_index as f32) * 1000.0;
        let next_scaled = move_near(2.0, 0.2, cur_scaled, tgt_scaled, dt);
        self.command_cursor_y = (next_scaled as i32) as f32 / 1000.0;

        if self.focus == InventoryFocus::CommandWindow {
            self.command_glow = (self.command_glow + LOOK_COMMAND_GLOW_STEP * ticks_30hz)
                .min(LOOK_COMMAND_MAX_GLOW);
        } else if self.focus == InventoryFocus::RadioVolume {
            self.command_glow = 0.0;
        } else {
            self.command_glow = (self.command_glow - LOOK_COMMAND_GLOW_STEP * ticks_30hz).max(0.0);
        }

        // `look_combine` (`0x001da490`) 3-slot green overlay fade (`±4.0/s`)
        self.combine_slots[0] = self.combine_first_item_id;
        let has_combine = self.combine_slots.iter().any(Option::is_some);
        if has_combine {
            self.combine_fade = (self.combine_fade + dt * LOOK_COMBINE_FADE_RATE).min(1.0);
        } else {
            self.combine_fade = (self.combine_fade - dt * LOOK_COMBINE_FADE_RATE).max(0.0);
        }

        // Inventory & fullscreen Examine fade-in / fade-out (`SLUS_202.28` `itemmain` `0x001cdb40:86`,
        // `item_fade_out` `0x001db0e0:3740`, `item_examine_fade_out` `0x001db170:3769`: `0.5` s).
        let fade_rate = 1.0 / super::INVENTORY_FADE_DURATION_SEC;
        match self.fade_transition {
            InventoryFadeTransition::None => {
                if self.screen_fade_alpha > 0.0 {
                    self.screen_fade_alpha = (self.screen_fade_alpha - dt * fade_rate).max(0.0);
                }
            }
            InventoryFadeTransition::ClosingToGameplay => {
                self.screen_fade_alpha = (self.screen_fade_alpha + dt * fade_rate).min(1.0);
                if self.screen_fade_alpha >= 1.0 {
                    self.fade_transition = InventoryFadeTransition::None;
                    self.close();
                    return Some(InventoryAction::Close);
                }
            }
            InventoryFadeTransition::OpeningSubscreen(btn) => {
                self.screen_fade_alpha = (self.screen_fade_alpha + dt * fade_rate).min(1.0);
                if self.screen_fade_alpha >= 1.0 {
                    self.fade_transition = InventoryFadeTransition::None;
                    return Some(match btn {
                        CornerButton::Option => InventoryAction::OpenOptions,
                        CornerButton::Memo => InventoryAction::OpenMemo,
                        CornerButton::Map => InventoryAction::OpenMap,
                    });
                }
            }
            InventoryFadeTransition::EnteringFullscreenExamine => {
                self.screen_fade_alpha = (self.screen_fade_alpha + dt * fade_rate).min(1.0);
                if self.screen_fade_alpha >= 1.0 {
                    self.fade_transition = InventoryFadeTransition::None;
                    self.examine_page = 0;
                    self.examine_rgb = 128.0;
                    self.examine_phase = ExaminePhase::FullscreenInspect;
                    self.focus = InventoryFocus::ExamineOverlay;
                }
            }
            InventoryFadeTransition::ExitingFullscreenExamine => {
                self.screen_fade_alpha = (self.screen_fade_alpha + dt * fade_rate).min(1.0);
                if self.screen_fade_alpha >= 1.0 {
                    self.fade_transition = InventoryFadeTransition::None;
                    self.examine_page = 0;
                    self.examine_rgb = 128.0;
                    self.focus = InventoryFocus::Carousel;
                }
            }
        }

        // `sh2pc.exe` `FUN_0046bcf0` (`0x0046bcf0:5847`): smooth carousel angular scroll easing
        // via `move_near(20.0, 110.0, cur * 1000.0, 0.0) * 0.001`.
        if self.carousel_anim_offset.abs() > 0.001 {
            let next_milli = move_near(20.0, 110.0, self.carousel_anim_offset * 1000.0, 0.0, dt);
            self.carousel_anim_offset = next_milli * 0.001;
            if self.carousel_anim_offset.abs() < 0.002 {
                self.carousel_anim_offset = 0.0;
            }
        }

        // `sh2pc.exe` `FUN_0046af30` (`0x0046af30`) + `look_zanzo` (`0x001daab0`): glide the 4-edge
        // blue focus reticle toward `target_focus_rect()` via `move_near(2.0, 1.0)`.
        self.prev_focus_rect = self.focus_rect;
        if self.focus != InventoryFocus::ExamineOverlay {
            let tgt = self.target_focus_rect();
            for (cur, &t) in self.focus_rect.iter_mut().zip(tgt.iter()) {
                *cur = move_near(2.0, 1.0, *cur, t, dt);
            }
        }

        // `InlinePopup` open/dismiss vertical flip-collapse animation (`sh2pc.exe` `FUN_0046af30`
        // `case 0xc` `(156, 74)..(356, 274)` <-> `case 0xd` `(201, 172)..(311, 176)` 4 px slit,
        // `move_near(div = 2.0, min_step = 1.0)` at 30 Hz).
        if self.focus == InventoryFocus::ExamineOverlay {
            match self.examine_phase {
                ExaminePhase::InlinePopup => {
                    for (cur, &tgt) in self
                        .examine_popup_rect
                        .iter_mut()
                        .zip(EXAMINE_POPUP_OPEN_RECT.iter())
                    {
                        *cur = move_near(2.0, 1.0, *cur, tgt, dt);
                    }
                }
                ExaminePhase::InlineClosing => {
                    let mut settled = true;
                    for (cur, &tgt) in self
                        .examine_popup_rect
                        .iter_mut()
                        .zip(EXAMINE_POPUP_DISMISS_SLIT_RECT.iter())
                    {
                        *cur = move_near(2.0, 1.0, *cur, tgt, dt);
                        if (*cur - tgt).abs() > 0.5 {
                            settled = false;
                        }
                    }
                    if settled {
                        self.examine_popup_rect = EXAMINE_POPUP_DISMISS_SLIT_RECT;
                        // `sh2pc.exe` `sh2pc_00460000.c:4874-4876` (shifted `+3` from `SLUS_202.28`
                        // `otn_itemmain.c:660-666` `0x2f..=0x31`): 2-sided Wood Side Apartments coins
                        // (`0x32` Snake, `0x33` Old Man, `0x34` Prisoner) flip from front (`0`) to back (`1`).
                        let is_two_sided_coin =
                            matches!(self.selected_item_id(), Some(0x32..=0x34));
                        if is_two_sided_coin && self.examine_coin_side == 0 {
                            self.examine_coin_side = 1;
                            self.examine_phase = ExaminePhase::InlinePopup;
                        } else {
                            self.examine_coin_side = 0;
                            self.examine_phase = ExaminePhase::InlinePopup;
                            self.focus = InventoryFocus::Carousel;
                            self.focus_rect = EXAMINE_POPUP_DISMISS_SLIT_RECT;
                        }
                    }
                }
                ExaminePhase::FullscreenInspect | ExaminePhase::FullscreenReading => {}
            }
        }

        // `examine_main` (`0x001d3a00` lines 1678–1681): dim `_examine_rgb` from `0x80` (128) down
        // to `0x45` (69) at `-300.0 * dt` while reading fullscreen text over the examine image.
        if self.focus == InventoryFocus::ExamineOverlay
            && self.examine_phase == ExaminePhase::FullscreenReading
            && self.examine_rgb > 69.0
        {
            self.examine_rgb = (self.examine_rgb - dt * 300.0).max(69.0);
        }

        if self.status_toast_timer > 0.0 {
            self.status_toast_timer = (self.status_toast_timer - dt).max(0.0);
            if self.status_toast_timer == 0.0 {
                self.status_toast = None;
            }
        }
        None
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
                    cmds.push(ItemCommand::Volume);
                } else {
                    cmds.push(ItemCommand::Use);
                }
                cmds.push(ItemCommand::Look);
            }
            ItemCategory::KeyItem | ItemCategory::PuzzleItem | ItemCategory::SubScenarioItem => {
                // Exact `sh2pc.exe` `FUN_0046a110` (`0x0046a110`, `command_kind`):
                // Only combinable puzzle items (`case 1`: `0x23, 0x35, 0x36, 0x3e, 0x3f, 0x40` and
                // `case 10`: `0x24, 0x38, 0x39, 0x3b, 0x3c, 0x3d`) expose `Combine`; non-combinable
                // items like `Photo of Mary` (`0x14`) and `Letter from Mary` (`0x15`) show only
                // `Use` + `Examine` (`case 0xb` / `case 9`).
                cmds.push(ItemCommand::Use);
                let is_combinable = matches!(
                    item_id,
                    0x23 | 0x24
                        | 0x35
                        | 0x36
                        | 0x38
                        | 0x39
                        | 0x3b
                        | 0x3c
                        | 0x3d
                        | 0x3e
                        | 0x3f
                        | 0x40
                );
                if is_combinable {
                    cmds.push(ItemCommand::Combine);
                }
                cmds.push(ItemCommand::Look);
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
        if !self.is_open || self.fade_transition != InventoryFadeTransition::None {
            return (MenuSoundCue::None, InventoryAction::None);
        }

        match self.focus {
            InventoryFocus::Carousel => self.handle_carousel_nav(nav, inventory, snapshot),
            InventoryFocus::Corner(btn) => self.handle_corner_nav(btn, nav, snapshot),
            InventoryFocus::CommandWindow => self.handle_command_window_nav(nav, inventory, false),
            InventoryFocus::RadioVolume => self.handle_radio_volume_nav(nav),
            InventoryFocus::CombineTarget => self.handle_combine_nav(nav),
            InventoryFocus::ExamineOverlay => match nav {
                MenuNavAction::Confirm => match self.examine_phase {
                    ExaminePhase::InlinePopup => {
                        self.examine_page = 0;
                        self.examine_phase = ExaminePhase::InlineClosing;
                        (MenuSoundCue::Cancel, InventoryAction::None)
                    }
                    ExaminePhase::InlineClosing => (MenuSoundCue::None, InventoryAction::None),
                    ExaminePhase::FullscreenInspect => {
                        // `examine_main` (`0x001d3a00` lines 1731–1734): Confirm transitions from
                        // Phase 0 (clean full-brightness image) to Phase 1 (dim image + text pages).
                        self.examine_phase = ExaminePhase::FullscreenReading;
                        self.examine_page = 0;
                        (MenuSoundCue::Confirm, InventoryAction::None)
                    }
                    ExaminePhase::FullscreenReading => {
                        let page_count = self
                            .selected_item_id()
                            .and_then(item_metadata)
                            .map(|m| {
                                m.examine_text
                                    .unwrap_or(m.description)
                                    .split('\x0c')
                                    .count()
                            })
                            .unwrap_or(1)
                            .max(1);
                        if self.examine_page + 1 < page_count {
                            self.examine_page += 1;
                            (MenuSoundCue::Confirm, InventoryAction::None)
                        } else {
                            // `examine_main` (`0x001d3a00:1704`): fade to black, then return to Inventory
                            self.fade_transition =
                                InventoryFadeTransition::ExitingFullscreenExamine;
                            (MenuSoundCue::Cancel, InventoryAction::None)
                        }
                    }
                },
                MenuNavAction::Cancel => match self.examine_phase {
                    ExaminePhase::InlinePopup => {
                        self.examine_page = 0;
                        self.examine_phase = ExaminePhase::InlineClosing;
                        (MenuSoundCue::Cancel, InventoryAction::None)
                    }
                    ExaminePhase::InlineClosing => (MenuSoundCue::None, InventoryAction::None),
                    ExaminePhase::FullscreenInspect | ExaminePhase::FullscreenReading => {
                        self.fade_transition = InventoryFadeTransition::ExitingFullscreenExamine;
                        (MenuSoundCue::Cancel, InventoryAction::None)
                    }
                },
                MenuNavAction::Left if self.enable_free_item_orbit => {
                    self.examine_yaw_rad -= 0.25;
                    (MenuSoundCue::None, InventoryAction::None)
                }
                MenuNavAction::Right if self.enable_free_item_orbit => {
                    self.examine_yaw_rad += 0.25;
                    (MenuSoundCue::None, InventoryAction::None)
                }
                MenuNavAction::Up if self.enable_free_item_orbit => {
                    self.examine_pitch_rad = (self.examine_pitch_rad - 0.2).clamp(-1.2, 1.2);
                    (MenuSoundCue::None, InventoryAction::None)
                }
                MenuNavAction::Down if self.enable_free_item_orbit => {
                    self.examine_pitch_rad = (self.examine_pitch_rad + 0.2).clamp(-1.2, 1.2);
                    (MenuSoundCue::None, InventoryAction::None)
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
        // Authentic PS2/PC `512x448` logical coordinate space inside the centered 4:3 UI canvas
        // (`font_print` `0x001d4c00` & `CommandWindowMouseFix.cpp` `0x00472428`).
        let (ox, oy, cw, ch) = super::ps2_ui_canvas_rect(viewport_w, viewport_h);
        if cw <= 0.0 || ch <= 0.0 {
            return;
        }
        // Retail PC `sh2pc.exe` `512x480` space (`FUN_004707b0:525-526`): subtract `16.0` overscan
        // offset to obtain the `512x448` logical coordinates used by hitboxes.
        let ps2_x = (mouse_x - ox) * 512.0 / cw;
        let ps2_y = (mouse_y - oy) * 480.0 / ch - 16.0;

        if self.focus == InventoryFocus::CommandWindow {
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
                let (x0, y0, x1, y1) = btn.hitbox_512x448();
                if ps2_x >= x0 && ps2_x <= x1 && ps2_y >= y0 && ps2_y <= y1 {
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
        let (ox, oy, cw, ch) = super::ps2_ui_canvas_rect(viewport_w, viewport_h);
        if cw <= 0.0 || ch <= 0.0 {
            return (MenuSoundCue::None, InventoryAction::None);
        }
        // Retail PC `sh2pc.exe` `512x480` space (`FUN_004707b0:525-526`): subtract `16.0` overscan
        // offset to obtain the `512x448` logical coordinates used by hitboxes.
        let ps2_x = (mouse_x - ox) * 512.0 / cw;
        let ps2_y = (mouse_y - oy) * 480.0 / ch - 16.0;

        if self.focus == InventoryFocus::ExamineOverlay {
            return self.handle_nav(MenuNavAction::Confirm, inventory, snapshot);
        }

        if self.focus == InventoryFocus::RadioVolume {
            self.focus = InventoryFocus::Carousel;
            return (MenuSoundCue::Confirm, InventoryAction::None);
        }

        if self.focus == InventoryFocus::CommandWindow {
            if (CMD_WIN_MOUSE_X_MIN..=CMD_WIN_MOUSE_X_MAX).contains(&ps2_x)
                && (CMD_WIN_MOUSE_Y_MIN..=CMD_WIN_MOUSE_Y_MAX).contains(&ps2_y)
            {
                return self.handle_command_window_nav(MenuNavAction::Confirm, inventory, true);
            }
            self.focus = InventoryFocus::Carousel;
            return (MenuSoundCue::Cancel, InventoryAction::None);
        }

        for btn in CornerButton::ALL {
            let (x0, y0, x1, y1) = btn.hitbox_512x448();
            if ps2_x >= x0 && ps2_x <= x1 && ps2_y >= y0 && ps2_y <= y1 {
                self.focus = InventoryFocus::Corner(btn);
                return self.handle_corner_nav(btn, MenuNavAction::Confirm, snapshot);
            }
        }

        if (154.0..=320.0).contains(&ps2_y) && !self.owned_items.is_empty() {
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
                    // `sh2pc.exe` `FUN_0046bcf0:5851`: kick smooth angular scroll clamped to `±1.5`
                    self.carousel_anim_offset = (self.carousel_anim_offset + 1.0).clamp(-1.5, 1.5);
                    (MenuSoundCue::CursorMove, InventoryAction::None)
                } else {
                    (MenuSoundCue::None, InventoryAction::None)
                }
            }
            MenuNavAction::Right => {
                if !self.owned_items.is_empty() {
                    self.selected_index = (self.selected_index + 1) % self.owned_items.len();
                    self.selected_scale = CAROUSEL_UNSELECTED_SCALE;
                    self.carousel_anim_offset = (self.carousel_anim_offset - 1.0).clamp(-1.5, 1.5);
                    (MenuSoundCue::CursorMove, InventoryAction::None)
                } else {
                    (MenuSoundCue::None, InventoryAction::None)
                }
            }
            MenuNavAction::Up | MenuNavAction::Down => {
                // Retail PC `sh2pc.exe` (`0x00473fc2..0x00473fd9`, `sh2pc_00470000.c:2957-2978`):
                // pressing Up/Down from Carousel (`DAT_009417a9 == 1`) enters the center bottom-row
                // button (`DAT_009417a9 = 3` = `CornerButton::Memo`, `x = 176..336`).
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
                // `item_fade_out` (`0x001db0e0`): smoothly fade Inventory to black over `0.5` s;
                // `tick()` emits `Some(InventoryAction::Close)` once opaque black is reached so
                // gameplay starts its `PlayableMain` `case 3` (`gamemain.c:448`) fade-in from black.
                self.fade_transition = InventoryFadeTransition::ClosingToGameplay;
                (MenuSoundCue::Cancel, InventoryAction::None)
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
                // Retail PC `sh2pc.exe` (`0x00472c20..0x00472c50`) order: `Options | Memo | Map`
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
                    // Gate on `MemoCommandCheck` (`0x00230570`); fade out over `0.5` s (`otn_itemmain.c:530`)
                    if snapshot.has_any_memo {
                        self.fade_transition =
                            InventoryFadeTransition::OpeningSubscreen(CornerButton::Memo);
                        (MenuSoundCue::Confirm, InventoryAction::None)
                    } else {
                        self.show_toast("No memos collected yet.");
                        (MenuSoundCue::Cancel, InventoryAction::None)
                    }
                }
                CornerButton::Map => {
                    // `otn_itemmain.c:567` (`DAT_0116e65c = 1; DAT_0116e65d = 5`): `0.5` s fade out
                    self.fade_transition =
                        InventoryFadeTransition::OpeningSubscreen(CornerButton::Map);
                    (MenuSoundCue::Confirm, InventoryAction::None)
                }
                CornerButton::Option => {
                    // `otn_itemmain.c:497` (`DAT_0116e65c = 1; DAT_0116e65d = 7`): `0.5` s fade out
                    self.fade_transition =
                        InventoryFadeTransition::OpeningSubscreen(CornerButton::Option);
                    (MenuSoundCue::Confirm, InventoryAction::None)
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
                        self.examine_page = 0;
                        self.examine_rgb = 128.0;
                        self.examine_coin_side = 0;
                        self.examine_popup_rect = EXAMINE_POPUP_CMD_START_RECT;
                        // `sh2pc.exe` `FUN_0046a110` (`0x0046a110`) `case 9`: fullscreen `examine_main`
                        // (`0x001d3a00`) items fade out over `0.5` s (`command_main` `0x001cf9e0:1053`)
                        // then fade back in on `FullscreenInspect` (`item_examine_fade_out` `0x001db170:3769`).
                        // All other examinable items (`Photo of Mary` `0x14`, etc.) use the inline `200x200` popup.
                        let is_fullscreen_examine = matches!(
                            item_id,
                            0x15 | 0x16 | 0x4a | 0x4b | 0x4f | 0x50 | 0x51 | 0x54
                        );
                        if is_fullscreen_examine {
                            self.fade_transition =
                                InventoryFadeTransition::EnteringFullscreenExamine;
                        } else {
                            self.examine_phase = ExaminePhase::InlinePopup;
                            self.focus = InventoryFocus::ExamineOverlay;
                        }
                        (MenuSoundCue::Confirm, InventoryAction::ExamineItem(item_id))
                    }
                    ItemCommand::Combine => {
                        self.combine_first_item_id = Some(item_id);
                        self.combine_slots = [Some(item_id), None, None];
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
                    ItemCommand::Volume => {
                        self.focus = InventoryFocus::RadioVolume;
                        self.command_glow = 0.0;
                        (MenuSoundCue::Confirm, InventoryAction::None)
                    }
                }
            }
        }
    }

    /// Handles Left/Right volume adjustments (`0..=15`) in `command_main` case 7 (`0x001d0790`).
    fn handle_radio_volume_nav(&mut self, nav: MenuNavAction) -> (MenuSoundCue, InventoryAction) {
        let max_vol = (RADIO_VOLUME_BARS - 1) as u8;
        match nav {
            MenuNavAction::Left => {
                if self.radio_volume > 0 {
                    self.radio_volume -= 1;
                    (
                        MenuSoundCue::CursorMove,
                        InventoryAction::SetRadioVolume(self.radio_volume),
                    )
                } else {
                    (MenuSoundCue::None, InventoryAction::None)
                }
            }
            MenuNavAction::Right => {
                if self.radio_volume < max_vol {
                    self.radio_volume += 1;
                    (
                        MenuSoundCue::CursorMove,
                        InventoryAction::SetRadioVolume(self.radio_volume),
                    )
                } else {
                    (MenuSoundCue::None, InventoryAction::None)
                }
            }
            MenuNavAction::Confirm | MenuNavAction::Cancel => {
                self.focus = InventoryFocus::Carousel;
                (MenuSoundCue::Confirm, InventoryAction::None)
            }
            _ => (MenuSoundCue::None, InventoryAction::None),
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
                self.combine_slots = [None; 3];
                self.focus = InventoryFocus::Carousel;
                (MenuSoundCue::Cancel, InventoryAction::None)
            }
            MenuNavAction::Confirm => {
                let Some(selected) = self.selected_item_id() else {
                    self.combine_first_item_id = None;
                    self.combine_slots = [None; 3];
                    self.focus = InventoryFocus::Carousel;
                    self.show_toast("These items cannot be combined.");
                    return (MenuSoundCue::Cancel, InventoryAction::None);
                };

                // Stage 2 -> 3 (`SLUS_202.28` `combine_item_use` `0x001d38d0`, `otn_itemmain.c:1624-1656`
                // & `command_main` `0x001cf9e0`, `otn_itemmain.c:722-727`): when two items are already
                // staged in `_DAT_0116e644`/`_DAT_0116e646`, evaluate the 3-item recipe (`#92` Prison
                // Tablets or `#96` Toluca Prison Hatch), or confirm the 2-item recipe (`#89..=91`) if
                // re-selecting an already-staged item.
                if let (Some(a), Some(b)) = (self.combine_slots[0], self.combine_slots[1]) {
                    self.combine_first_item_id = None;
                    self.combine_slots = [None; 3];
                    self.focus = InventoryFocus::Carousel;
                    if selected != a && selected != b {
                        if let Some(result_id) = combine_items(a, b, selected) {
                            self.active_combo_id = Some(result_id);
                            return (
                                MenuSoundCue::Confirm,
                                InventoryAction::CombinedItems {
                                    first_id: a,
                                    second_id: b,
                                    third_id: Some(selected),
                                    result_id,
                                },
                            );
                        }
                    } else if let Some(result_id) = combine_items(a, b, 0) {
                        let is_complete_two_item = ITEM_COMBOS
                            .iter()
                            .find(|c| c.result_id == result_id)
                            .is_some_and(|c| c.consumes_parts_on_use);
                        if is_complete_two_item {
                            self.active_combo_id = Some(result_id);
                            return (
                                MenuSoundCue::Confirm,
                                InventoryAction::CombinedItems {
                                    first_id: a,
                                    second_id: b,
                                    third_id: None,
                                    result_id,
                                },
                            );
                        }
                    }
                    self.show_toast("These items cannot be combined.");
                    return (MenuSoundCue::Cancel, InventoryAction::None);
                }

                // Stage 1 -> 2 (`SLUS_202.28` `command_main` `0x001cf9e0`, `otn_itemmain.c:734-749`):
                // stage `b` into `combine_slots[1]` (`_DAT_0116e646`) when `(a, b)` is a partial
                // 2-item match (`consumes_parts_on_use == false`, Toluca Prison Hatch `#93..=95`) OR
                // when a 3-item superset exists whose 3rd item is owned (e.g. 3 Prison Tablets `#92`).
                let first = self.combine_first_item_id.or(self.combine_slots[0]);
                if let Some(a) = first {
                    let b = selected;
                    if a != b {
                        if let Some(result_id) = combine_items(a, b, 0) {
                            let is_partial_for_third = ITEM_COMBOS
                                .iter()
                                .find(|c| c.result_id == result_id)
                                .is_some_and(|c| !c.consumes_parts_on_use);
                            let can_extend_to_owned_third = ITEM_COMBOS.iter().any(|c| {
                                c.parts[0] != 0
                                    && c.parts.contains(&a)
                                    && c.parts.contains(&b)
                                    && c.parts
                                        .iter()
                                        .find(|&&p| p != a && p != b)
                                        .is_some_and(|third| self.owned_items.contains(third))
                            });
                            if is_partial_for_third || can_extend_to_owned_third {
                                self.combine_first_item_id = Some(a);
                                self.combine_slots = [Some(a), Some(b), None];
                                self.show_toast("Select a third item to combine.");
                                return (MenuSoundCue::Confirm, InventoryAction::None);
                            }
                            self.combine_first_item_id = None;
                            self.combine_slots = [None; 3];
                            self.focus = InventoryFocus::Carousel;
                            self.active_combo_id = Some(result_id);
                            return (
                                MenuSoundCue::Confirm,
                                InventoryAction::CombinedItems {
                                    first_id: a,
                                    second_id: b,
                                    third_id: None,
                                    result_id,
                                },
                            );
                        }
                    }
                }
                self.combine_first_item_id = None;
                self.combine_slots = [None; 3];
                self.focus = InventoryFocus::Carousel;
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
        inv.item_get_now(0x24); // Combinable puzzle item (sh2pc.exe FUN_0046a110 case 10: Use, Combine, Examine)

        let snap = InventoryPlayerSnapshot::default();
        let mut screen = InventoryScreen::new();
        screen.open(&inv);

        // Chainsaw (#16) must offer Equip & Examine, NOT Flashlight ON/OFF!
        assert_eq!(
            screen.commands_for_item(16, &inv, &snap),
            vec![ItemCommand::Equip, ItemCommand::Look]
        );
        // Flashlight (#18) offers ON, Examine; Radio (#19) offers OFF, Volume, Examine
        assert_eq!(
            screen.commands_for_item(18, &inv, &snap),
            vec![ItemCommand::TurnOn, ItemCommand::Look]
        );
        assert_eq!(
            screen.commands_for_item(19, &inv, &snap),
            vec![ItemCommand::TurnOff, ItemCommand::Volume, ItemCommand::Look]
        );

        // Open command window on combinable puzzle item (0x24) -> 3 options (Use, Combine, Examine)
        screen.selected_index = 5;
        screen.handle_nav(MenuNavAction::Confirm, &inv, &snap);
        assert_eq!(screen.focus, InventoryFocus::CommandWindow);
        assert_eq!(screen.active_commands.len(), 3);
        assert_eq!(
            screen.active_commands,
            vec![ItemCommand::Use, ItemCommand::Combine, ItemCommand::Look]
        );

        // SH2:EE CommandWindowMouseFix 3-option hitboxes inside a 4:3 (640x480) viewport:
        // Retail PC `512x480` maps `y_448 -> y_448 + 16.0` (`FUN_004707b0:526`).
        let sx = 640.0 / 512.0;
        screen.handle_mouse_move(390.0 * sx, 75.0 + 16.0, 640.0, 480.0);
        assert_eq!(screen.command_index, 0);
        screen.handle_mouse_move(390.0 * sx, 100.0 + 16.0, 640.0, 480.0);
        assert_eq!(screen.command_index, 1);
        screen.handle_mouse_move(390.0 * sx, 128.0 + 16.0, 640.0, 480.0);
        assert_eq!(screen.command_index, 2);

        // Click row 1 (`Combine`) at y = 100 -> enters CombineTarget preserving command_index == 1
        let (cue, _) =
            screen.handle_mouse_click(390.0 * sx, 100.0 + 16.0, 640.0, 480.0, &inv, &snap);
        assert_eq!(cue, MenuSoundCue::Confirm);
        assert_eq!(screen.focus, InventoryFocus::CombineTarget);
        assert_eq!(screen.command_index, 1);

        // Mouse click while in `RadioVolume` exits cleanly back to `Carousel` without hitting corner buttons
        screen.focus = InventoryFocus::RadioVolume;
        let (vol_cue, vol_act) =
            screen.handle_mouse_click(60.0 * sx, 420.0 + 16.0, 640.0, 480.0, &inv, &snap);
        assert_eq!(vol_cue, MenuSoundCue::Confirm);
        assert_eq!(vol_act, InventoryAction::None);
        assert_eq!(screen.focus, InventoryFocus::Carousel);
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

    #[test]
    fn test_sh2_152_retail_labels_non_combinable_mary_items_and_examine_pagination() {
        // Retail PC `sh2pc.exe` (`0x00472c20..0x00472c50`): `Options | Memo | Map`
        assert_eq!(
            CornerButton::ALL.map(|b| b.label()),
            ["Options", "Memo", "Map"]
        );
        // Retail `item_msg_e.mes` (`0x06`): `"Examine"`
        assert_eq!(ItemCommand::Look.label(), "Examine");

        let mut inv = InventoryManager::new();
        inv.item_get_now(20); // Photo of Mary (0x14)
        inv.item_get_now(21); // Letter from Mary (0x15)
        let snap = InventoryPlayerSnapshot::default();
        let mut screen = InventoryScreen::new();
        screen.open(&inv);

        // `sh2pc.exe` `FUN_0046a110` (`0x0046a110`): Photo & Letter expose `Use` + `Examine` ONLY (no `Combine`)
        assert_eq!(
            screen.commands_for_item(20, &inv, &snap),
            vec![ItemCommand::Use, ItemCommand::Look]
        );
        assert_eq!(
            screen.commands_for_item(21, &inv, &snap),
            vec![ItemCommand::Use, ItemCommand::Look]
        );

        // Select Photo of Mary (idx 0), Examine -> InlinePopup (`sh2pc.exe` `FUN_0046af30` `case 0xc`)
        screen.selected_index = 0;
        screen.handle_nav(MenuNavAction::Confirm, &inv, &snap);
        screen.handle_nav(MenuNavAction::Down, &inv, &snap); // row 1 = Examine
        screen.handle_nav(MenuNavAction::Confirm, &inv, &snap);
        assert_eq!(screen.focus, InventoryFocus::ExamineOverlay);
        assert_eq!(screen.examine_phase, ExaminePhase::InlinePopup);
        assert!(!screen.is_fullscreen_examine());
        for _ in 0..20 {
            screen.tick(1.0 / 30.0, &snap);
        }
        assert!((screen.examine_popup_rect[0] - EXAMINE_POPUP_OPEN_RECT[0]).abs() < 1.0);
        assert!((screen.examine_popup_rect[1] - EXAMINE_POPUP_OPEN_RECT[1]).abs() < 1.0);
        // Dismissing InlinePopup plays `case 0xd` vertical flip-collapse into the 4 px slit `(201,172)..(311,176)`
        let (close_cue, _) = screen.handle_nav(MenuNavAction::Confirm, &inv, &snap);
        assert_eq!(close_cue, MenuSoundCue::Cancel);
        assert_eq!(screen.examine_phase, ExaminePhase::InlineClosing);
        for _ in 0..25 {
            screen.tick(1.0 / 30.0, &snap);
        }
        assert_eq!(screen.focus, InventoryFocus::Carousel);
        assert_eq!(screen.examine_popup_rect, EXAMINE_POPUP_DISMISS_SLIT_RECT);

        // Select Letter from Mary (idx 1), open CommandWindow, choose Examine -> 2-phase `examine_main`:
        // `0.5` s fade-out (`command_main` `0x001cf9e0:1053`) -> Phase 0 (`FullscreenInspect`, bright
        // image, fades in over `0.5` s) -> Confirm -> Phase 1 (`FullscreenReading`, dims `examine_rgb`
        // 128 -> 69 and paginates 4 `\x0c` pages) -> `0.5` s fade-out back to Carousel.
        screen.selected_index = 1;
        screen.handle_nav(MenuNavAction::Confirm, &inv, &snap);
        assert_eq!(screen.focus, InventoryFocus::CommandWindow);
        screen.handle_nav(MenuNavAction::Down, &inv, &snap); // row 1 = Examine
        screen.handle_nav(MenuNavAction::Confirm, &inv, &snap);
        assert_eq!(
            screen.fade_transition,
            InventoryFadeTransition::EnteringFullscreenExamine
        );
        for _ in 0..10 {
            screen.tick(0.06, &snap);
        }
        assert_eq!(screen.fade_transition, InventoryFadeTransition::None);
        assert_eq!(screen.focus, InventoryFocus::ExamineOverlay);
        assert_eq!(screen.examine_phase, ExaminePhase::FullscreenInspect);
        assert!((screen.examine_rgb - 128.0).abs() < 1e-3);

        // Confirm enters Phase 1 (`FullscreenReading`, page 0)
        let (cue0, _) = screen.handle_nav(MenuNavAction::Confirm, &inv, &snap);
        assert_eq!(cue0, MenuSoundCue::Confirm);
        assert_eq!(screen.examine_phase, ExaminePhase::FullscreenReading);
        assert_eq!(screen.examine_page, 0);
        // Tick dims `examine_rgb` down to `69.0` (`0x45`) at `-300.0 * dt`
        for _ in 0..10 {
            screen.tick(0.05, &snap);
        }
        assert!((screen.examine_rgb - 69.0).abs() < 1e-3);

        for expected_page in 1..=3 {
            let (cue, _) = screen.handle_nav(MenuNavAction::Confirm, &inv, &snap);
            assert_eq!(cue, MenuSoundCue::Confirm);
            assert_eq!(screen.focus, InventoryFocus::ExamineOverlay);
            assert_eq!(screen.examine_page, expected_page);
        }
        // Final Confirm after page 3 triggers `0.5` s fade-out back to Carousel
        let (cue, _) = screen.handle_nav(MenuNavAction::Confirm, &inv, &snap);
        assert_eq!(cue, MenuSoundCue::Cancel);
        assert_eq!(
            screen.fade_transition,
            InventoryFadeTransition::ExitingFullscreenExamine
        );
        for _ in 0..10 {
            screen.tick(0.06, &snap);
        }
        assert_eq!(screen.fade_transition, InventoryFadeTransition::None);
        assert_eq!(screen.focus, InventoryFocus::Carousel);
        assert_eq!(screen.examine_page, 0);

        // Confirming a bottom subscreen (`Options` / `Map` / `Memo`) fades out over `0.5` s before emitting the action
        screen.focus = InventoryFocus::Corner(CornerButton::Option);
        let (opt_cue, opt_act) = screen.handle_nav(MenuNavAction::Confirm, &inv, &snap);
        assert_eq!(opt_cue, MenuSoundCue::Confirm);
        assert_eq!(opt_act, InventoryAction::None);
        assert_eq!(
            screen.fade_transition,
            InventoryFadeTransition::OpeningSubscreen(CornerButton::Option)
        );
        let mut deferred = None;
        for _ in 0..12 {
            if let Some(act) = screen.tick(0.05, &snap) {
                deferred = Some(act);
                break;
            }
        }
        assert_eq!(deferred, Some(InventoryAction::OpenOptions));
        assert_eq!(screen.fade_transition, InventoryFadeTransition::None);

        // Exiting Inventory (`Cancel` on Carousel) fades to black over `0.5` s then emits `InventoryAction::Close`
        screen.focus = InventoryFocus::Carousel;
        screen.screen_fade_alpha = 0.0;
        let (exit_cue, exit_act) = screen.handle_nav(MenuNavAction::Cancel, &inv, &snap);
        assert_eq!(exit_cue, MenuSoundCue::Cancel);
        assert_eq!(exit_act, InventoryAction::None);
        assert_eq!(
            screen.fade_transition,
            InventoryFadeTransition::ClosingToGameplay
        );
        let mut close_deferred = None;
        for _ in 0..12 {
            if let Some(act) = screen.tick(0.05, &snap) {
                close_deferred = Some(act);
                break;
            }
        }
        assert_eq!(close_deferred, Some(InventoryAction::Close));
        assert!(!screen.is_open);
        // Re-open for subsequent Carousel <-> Corner checks
        screen.open(&inv);
        screen.screen_fade_alpha = 0.0;

        // Mouse click during ExamineOverlay advances/closes pages instead of hitting corner buttons
        screen.focus = InventoryFocus::ExamineOverlay;
        screen.examine_phase = ExaminePhase::FullscreenReading;
        screen.examine_page = 0;
        let (mcue, mact) = screen.handle_mouse_click(60.0, 420.0, 512.0, 448.0, &inv, &snap);
        assert_eq!(mcue, MenuSoundCue::Confirm);
        assert_eq!(mact, InventoryAction::None);
        assert_eq!(screen.focus, InventoryFocus::ExamineOverlay);
        assert_eq!(screen.examine_page, 1);

        // Carousel Down -> center bottom button `Memo` (`sh2pc.exe` `0x00473fd9`) regardless of `has_any_memo`,
        // plus Left/Right wraparound across `Options | Memo | Map`
        screen.focus = InventoryFocus::Carousel;
        for has_memo in [false, true] {
            let s = InventoryPlayerSnapshot {
                has_any_memo: has_memo,
                ..Default::default()
            };
            screen.handle_nav(MenuNavAction::Down, &inv, &s);
            assert_eq!(screen.focus, InventoryFocus::Corner(CornerButton::Memo));
            screen.handle_nav(MenuNavAction::Left, &inv, &s);
            assert_eq!(screen.focus, InventoryFocus::Corner(CornerButton::Option));
            screen.handle_nav(MenuNavAction::Left, &inv, &s);
            assert_eq!(screen.focus, InventoryFocus::Corner(CornerButton::Map));
            screen.handle_nav(MenuNavAction::Right, &inv, &s);
            assert_eq!(screen.focus, InventoryFocus::Corner(CornerButton::Option));
            screen.handle_nav(MenuNavAction::Right, &inv, &s);
            assert_eq!(screen.focus, InventoryFocus::Corner(CornerButton::Memo));
            screen.handle_nav(MenuNavAction::Up, &inv, &s);
            assert_eq!(screen.focus, InventoryFocus::Carousel);
        }
        // 2-sided Wood Side Apartments coin (`0x33` Coin [Old Man] in retail PC `sh2pc_00460000.c:4874-4876`,
        // shifted `+3` from `SLUS_202.28` `0x2f..=0x31`) flips front (`0`) -> back (`1`) on first dismiss
        // and exits to Carousel on second dismiss.
        inv.item_get_now(0x33);
        screen.open(&inv);
        screen.screen_fade_alpha = 0.0;
        screen.selected_index = screen
            .owned_items
            .iter()
            .position(|&id| id == 0x33)
            .expect("coin 0x33 must be owned");
        screen.handle_nav(MenuNavAction::Right, &inv, &snap);
        assert!((screen.carousel_anim_offset + 1.0).abs() < 1e-4);
        for _ in 0..20 {
            screen.tick(1.0 / 30.0, &snap);
        }
        assert!(screen.carousel_anim_offset.abs() < 0.05);
        screen.selected_index = screen
            .owned_items
            .iter()
            .position(|&id| id == 0x33)
            .expect("coin 0x33 must be owned");
        screen.handle_nav(MenuNavAction::Confirm, &inv, &snap);
        screen.handle_nav(MenuNavAction::Down, &inv, &snap); // Examine
        screen.handle_nav(MenuNavAction::Confirm, &inv, &snap);
        assert_eq!(screen.examine_coin_side, 0);
        for _ in 0..20 {
            screen.tick(1.0 / 30.0, &snap);
        }
        screen.handle_nav(MenuNavAction::Confirm, &inv, &snap);
        for _ in 0..25 {
            screen.tick(1.0 / 30.0, &snap);
        }
        assert_eq!(screen.focus, InventoryFocus::ExamineOverlay);
        assert_eq!(screen.examine_coin_side, 1);
        screen.handle_nav(MenuNavAction::Confirm, &inv, &snap);
        for _ in 0..25 {
            screen.tick(1.0 / 30.0, &snap);
        }
        assert_eq!(screen.focus, InventoryFocus::Carousel);
        assert_eq!(screen.examine_coin_side, 0);
    }

    #[test]
    fn test_sh2_166_three_item_combine_toluca_prison_hatch() {
        // Toluca Prison Hatch: Horseshoe (62) + Lighter (63) + Wax doll (64) -> combo #96
        // (`SLUS_202.28` `combine_item_use` `0x001d38d0`, `otn_itemmain.c:1624-1656`).
        let mut inv = InventoryManager::new();
        inv.item_get_now(35); // Purple Bull key
        inv.item_get_now(36); // Lapis Eye key
        inv.item_get_now(62); // Horseshoe
        inv.item_get_now(63); // Lighter
        inv.item_get_now(64); // Wax doll

        let snap = InventoryPlayerSnapshot::default();
        let mut screen = InventoryScreen::new();
        screen.open(&inv);
        screen.screen_fade_alpha = 0.0;

        // Select Wax doll (64), open CommandWindow, choose Combine (row 1)
        let idx_wax = screen.owned_items.iter().position(|&id| id == 64).unwrap();
        let idx_shoe = screen.owned_items.iter().position(|&id| id == 62).unwrap();
        let idx_lighter = screen.owned_items.iter().position(|&id| id == 63).unwrap();
        screen.selected_index = idx_wax;
        screen.handle_nav(MenuNavAction::Confirm, &inv, &snap);
        assert_eq!(screen.focus, InventoryFocus::CommandWindow);
        screen.handle_nav(MenuNavAction::Down, &inv, &snap); // Combine
        let (cue1, act1) = screen.handle_nav(MenuNavAction::Confirm, &inv, &snap);
        assert_eq!(cue1, MenuSoundCue::Confirm);
        assert_eq!(act1, InventoryAction::None);
        assert_eq!(screen.focus, InventoryFocus::CombineTarget);
        assert_eq!(screen.combine_slots, [Some(64), None, None]);

        // Select 2nd item: Horseshoe (62) -> partial combo #94, must stage into combine_slots[1]
        // and stay in CombineTarget (NO softlock!).
        screen.selected_index = idx_shoe;
        let (cue2, act2) = screen.handle_nav(MenuNavAction::Confirm, &inv, &snap);
        assert_eq!(cue2, MenuSoundCue::Confirm);
        assert_eq!(act2, InventoryAction::None);
        assert_eq!(screen.focus, InventoryFocus::CombineTarget);
        assert_eq!(screen.combine_slots, [Some(64), Some(62), None]);

        // Tick must preserve staged slot 1 while updating combine_fade
        screen.tick(1.0 / 30.0, &snap);
        assert_eq!(screen.combine_slots, [Some(64), Some(62), None]);

        // Select 3rd item: Lighter (63) -> completes 3-item combo #96!
        screen.selected_index = idx_lighter;
        let (cue3, act3) = screen.handle_nav(MenuNavAction::Confirm, &inv, &snap);
        assert_eq!(cue3, MenuSoundCue::Confirm);
        assert_eq!(
            act3,
            InventoryAction::CombinedItems {
                first_id: 64,
                second_id: 62,
                third_id: Some(63),
                result_id: 96,
            }
        );
        assert_eq!(screen.active_combo_id, Some(96));
        assert_eq!(screen.focus, InventoryFocus::Carousel);
        assert_eq!(screen.combine_slots, [None, None, None]);

        // 2-item complete combo (Purple Bull 35 + Lapis Eye 36 -> 86) still completes in 2 steps
        let idx_bull = screen.owned_items.iter().position(|&id| id == 35).unwrap();
        let idx_lapis = screen.owned_items.iter().position(|&id| id == 36).unwrap();
        screen.selected_index = idx_bull;
        screen.handle_nav(MenuNavAction::Confirm, &inv, &snap);
        screen.handle_nav(MenuNavAction::Down, &inv, &snap); // Combine
        screen.handle_nav(MenuNavAction::Confirm, &inv, &snap);
        screen.selected_index = idx_lapis;
        let (cue_2, act_2) = screen.handle_nav(MenuNavAction::Confirm, &inv, &snap);
        assert_eq!(cue_2, MenuSoundCue::Confirm);
        assert_eq!(
            act_2,
            InventoryAction::CombinedItems {
                first_id: 35,
                second_id: 36,
                third_id: None,
                result_id: 86,
            }
        );

        // 3-item Prison Tablets (59 + 60 + 61 -> #92) when all 3 tablets are owned
        let mut inv_tab = InventoryManager::new();
        inv_tab.item_get_now(59); // Tablet of "The Oppressor"
        inv_tab.item_get_now(60); // Tablet of "Gluttonous Pig"
        inv_tab.item_get_now(61); // Tablet of "The Seductress"
        let mut tab_screen = InventoryScreen::new();
        tab_screen.open(&inv_tab);
        tab_screen.screen_fade_alpha = 0.0;
        let idx_59 = tab_screen
            .owned_items
            .iter()
            .position(|&id| id == 59)
            .unwrap();
        let idx_60 = tab_screen
            .owned_items
            .iter()
            .position(|&id| id == 60)
            .unwrap();
        let idx_61 = tab_screen
            .owned_items
            .iter()
            .position(|&id| id == 61)
            .unwrap();
        tab_screen.selected_index = idx_59;
        tab_screen.handle_nav(MenuNavAction::Confirm, &inv_tab, &snap);
        tab_screen.handle_nav(MenuNavAction::Down, &inv_tab, &snap); // Combine
        tab_screen.handle_nav(MenuNavAction::Confirm, &inv_tab, &snap);
        assert_eq!(tab_screen.focus, InventoryFocus::CombineTarget);
        tab_screen.selected_index = idx_60;
        let (tc2, ta2) = tab_screen.handle_nav(MenuNavAction::Confirm, &inv_tab, &snap);
        assert_eq!(tc2, MenuSoundCue::Confirm);
        assert_eq!(ta2, InventoryAction::None);
        assert_eq!(tab_screen.focus, InventoryFocus::CombineTarget);
        assert_eq!(tab_screen.combine_slots, [Some(59), Some(60), None]);
        tab_screen.selected_index = idx_61;
        let (tc3, ta3) = tab_screen.handle_nav(MenuNavAction::Confirm, &inv_tab, &snap);
        assert_eq!(tc3, MenuSoundCue::Confirm);
        assert_eq!(
            ta3,
            InventoryAction::CombinedItems {
                first_id: 59,
                second_id: 60,
                third_id: Some(61),
                result_id: 92,
            }
        );
        assert_eq!(tab_screen.active_combo_id, Some(92));

        // 2-item Prison Tablets (59 + 60 -> #89) completes in 2 steps when 3rd tablet is NOT owned
        let mut inv_tab2 = InventoryManager::new();
        inv_tab2.item_get_now(59);
        inv_tab2.item_get_now(60);
        let mut tab2_screen = InventoryScreen::new();
        tab2_screen.open(&inv_tab2);
        tab2_screen.screen_fade_alpha = 0.0;
        tab2_screen.selected_index = 0; // 59
        tab2_screen.handle_nav(MenuNavAction::Confirm, &inv_tab2, &snap);
        tab2_screen.handle_nav(MenuNavAction::Down, &inv_tab2, &snap); // Combine
        tab2_screen.handle_nav(MenuNavAction::Confirm, &inv_tab2, &snap);
        tab2_screen.selected_index = 1; // 60
        let (t2c, t2a) = tab2_screen.handle_nav(MenuNavAction::Confirm, &inv_tab2, &snap);
        assert_eq!(t2c, MenuSoundCue::Confirm);
        assert_eq!(
            t2a,
            InventoryAction::CombinedItems {
                first_id: 59,
                second_id: 60,
                third_id: None,
                result_id: 89,
            }
        );
    }
}
