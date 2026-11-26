//! Mouse hit-testing and scroll wheel input for [`SaveLoadScreen`].

use super::state::{SaveLoadAction, SaveLoadScreen, SaveLoadStep, YesNoChoice, MC_VISIBLE_ROWS};
use crate::menu::MenuNavAction;

impl SaveLoadScreen {
    /// Maps window pixel coordinates `(mouse_x, mouse_y)` into the `512 × 448` top-left coordinate
    /// space used by [`super::render::draw_save_load_screen`] (`Ps2Canvas`).
    fn window_to_save_load_virtual(
        mouse_x: f32,
        mouse_y: f32,
        win_w: f32,
        win_h: f32,
    ) -> Option<(f32, f32)> {
        let (ox, oy, cw, ch) = crate::inventory::ps2_ui_canvas_rect(win_w, win_h);
        if cw <= 0.0 || ch <= 0.0 {
            return None;
        }
        let vx = (mouse_x - ox) * 512.0 / cw;
        let vy = (mouse_y - oy) * 480.0 / ch - 16.0;
        Some((vx, vy))
    }

    fn hit_test_visible_row(&self, vx: f32, vy: f32) -> Option<usize> {
        if !(36.0..=476.0).contains(&vx) {
            return None;
        }
        let total_rows = self.total_selectable_rows();
        for vis in 0..MC_VISIBLE_ROWS {
            let row_idx = self.scroll_top + vis;
            if row_idx >= total_rows {
                break;
            }
            let y0 = 150.0 + (vis as f32) * 40.0;
            if vy >= y0 && vy <= y0 + 36.0 {
                return Some(row_idx);
            }
        }
        None
    }

    fn hit_test_yes_no(vx: f32, vy: f32) -> Option<YesNoChoice> {
        if !(432.0..=472.0).contains(&vy) {
            return None;
        }
        if (156.0..=244.0).contains(&vx) {
            Some(YesNoChoice::Yes)
        } else if (268.0..=356.0).contains(&vx) {
            Some(YesNoChoice::No)
        } else {
            None
        }
    }

    /// Updates hovered slot row or Yes/No choice. Returns `true` when selection changed.
    pub fn handle_mouse_move(
        &mut self,
        mouse_x: f32,
        mouse_y: f32,
        win_w: f32,
        win_h: f32,
    ) -> bool {
        let Some((vx, vy)) = Self::window_to_save_load_virtual(mouse_x, mouse_y, win_w, win_h)
        else {
            return false;
        };
        match self.step {
            SaveLoadStep::SelectSlot => {
                if let Some(row_idx) = self.hit_test_visible_row(vx, vy) {
                    if row_idx != self.cursor_row {
                        self.cursor_row = row_idx;
                        return true;
                    }
                }
                false
            }
            SaveLoadStep::ConfirmOverwrite | SaveLoadStep::ConfirmDelete => {
                if let Some(choice) = Self::hit_test_yes_no(vx, vy) {
                    if choice != self.yes_no {
                        self.yes_no = choice;
                        return true;
                    }
                }
                false
            }
            _ => false,
        }
    }

    /// Handles left-click on Memory Card tabs, slot rows, Yes/No prompts, or status banners.
    /// Returns `(ui_changed, action)`.
    pub fn handle_mouse_click(
        &mut self,
        mouse_x: f32,
        mouse_y: f32,
        win_w: f32,
        win_h: f32,
    ) -> (bool, SaveLoadAction) {
        let Some((vx, vy)) = Self::window_to_save_load_virtual(mouse_x, mouse_y, win_w, win_h)
        else {
            return (false, SaveLoadAction::None);
        };
        match self.step {
            SaveLoadStep::SelectSlot => {
                // Memory Card 1 (`x=0x8b=139`) / Memory Card 2 (`x=0x175=373`) tabs at `y=68`
                if !self.config.unlimited_slots && (54.0..=94.0).contains(&vy) {
                    if (56.0..=222.0).contains(&vx) && self.active_card != 0 {
                        self.switch_card(0);
                        return (true, SaveLoadAction::None);
                    }
                    if (290.0..=456.0).contains(&vx) && self.active_card != 1 {
                        self.switch_card(1);
                        return (true, SaveLoadAction::None);
                    }
                }
                if let Some(row_idx) = self.hit_test_visible_row(vx, vy) {
                    self.cursor_row = row_idx;
                    return (true, self.handle_nav(MenuNavAction::Confirm));
                }
                (false, SaveLoadAction::None)
            }
            SaveLoadStep::ConfirmOverwrite | SaveLoadStep::ConfirmDelete => {
                if let Some(choice) = Self::hit_test_yes_no(vx, vy) {
                    self.yes_no = choice;
                    return (true, self.handle_nav(MenuNavAction::Confirm));
                }
                (false, SaveLoadAction::None)
            }
            SaveLoadStep::SaveDone | SaveLoadStep::DeleteDone | SaveLoadStep::LoadError => {
                (true, self.handle_nav(MenuNavAction::Confirm))
            }
            SaveLoadStep::LoadDone => (false, SaveLoadAction::None),
        }
    }

    /// Scrolls the slot list by `steps` (`> 0` down, `< 0` up). Returns `true` when selection moved.
    pub fn handle_mouse_wheel(&mut self, steps: i32) -> bool {
        if self.step != SaveLoadStep::SelectSlot || steps == 0 {
            return false;
        }
        let prev = self.cursor_row;
        let nav = if steps > 0 {
            MenuNavAction::Down
        } else {
            MenuNavAction::Up
        };
        for _ in 0..steps.unsigned_abs().min(5) {
            let _ = self.handle_nav(nav);
        }
        self.cursor_row != prev
    }
}

#[cfg(test)]
mod tests {
    use super::super::state::*;
    use sh2_core::campaign::{
        CampaignSaveSnapshot, SaveSlotEntry, SaveSlotKind, SAVE_FORMAT_VERSION,
    };
    use sh2_core::config::{ActionDifficulty, RiddleDifficulty};
    use sh2_core::events::ClearData;
    use sh2_core::scenario::ScenarioKind;

    fn sl_virtual_to_win(vx: f32, vy: f32, win_w: f32, win_h: f32) -> (f32, f32) {
        let (ox, oy, cw, ch) = crate::inventory::ps2_ui_canvas_rect(win_w, win_h);
        (ox + vx * cw / 512.0, oy + (vy + 16.0) * ch / 480.0)
    }

    fn sample_slot(card_index: usize, file_index: usize) -> SaveSlotEntry {
        SaveSlotEntry {
            card_index,
            file_index,
            kind: SaveSlotKind::Manual,
            is_corrupted: false,
            snapshot: CampaignSaveSnapshot {
                scenario: ScenarioKind::LetterFromSilentHeaven,
                stage_id: 2,
                room_id: "er01".to_string(),
                player_position: [0.0, 0.0, 0.0],
                player_yaw: 0.0,
                action_difficulty: ActionDifficulty::Normal,
                riddle_difficulty: RiddleDifficulty::Normal,
                play_time_secs: 120.0,
                save_count: 1,
                save_version: SAVE_FORMAT_VERSION,
                save_point_name_id: 1,
                secondary_timer_secs: 0.0,
                player_hp: 100.0,
                player_max_hp: 100.0,
                clear_data: ClearData::default(),
                previous_total_rank: 0,
                spray_pow: 0,
                event_flags_words: Vec::new(),
                inventory_items: Vec::new(),
                thumbnail_rgba: None,
            },
        }
    }

    #[test]
    fn test_save_load_mouse_move_click_tabs_yes_no_and_wheel() {
        let cfg = SaveLoadConfig {
            unlimited_slots: false,
            ..SaveLoadConfig::default()
        };
        let mut screen = SaveLoadScreen::new(
            SaveLoadMode::Save,
            ScenarioKind::LetterFromSilentHeaven,
            cfg,
        );
        screen.set_slots(vec![
            sample_slot(0, 0),
            sample_slot(0, 1),
            sample_slot(0, 2),
        ]);

        // Widescreen (1920x1080): hover visible row 2 (slot 1, since row 0 is "New Save" in Save mode)
        let row2_vy = 150.0 + 2.0 * 40.0 + 18.0;
        let (rx, ry) = sl_virtual_to_win(256.0, row2_vy, 1920.0, 1080.0);
        assert!(screen.handle_mouse_move(rx, ry, 1920.0, 1080.0));
        assert_eq!(screen.cursor_row, 2);

        // Mouse wheel scrolls down and up cleanly
        assert!(screen.handle_mouse_wheel(1));
        assert_eq!(screen.cursor_row, 3);
        assert!(screen.handle_mouse_wheel(-2));
        assert_eq!(screen.cursor_row, 1);
        assert!(!screen.handle_mouse_wheel(0));

        // Click Memory Card 2 tab (vx=373, vy=70) -> switches active_card to 1
        let (mc2_x, mc2_y) = sl_virtual_to_win(373.0, 70.0, 1920.0, 1080.0);
        assert_eq!(
            screen.handle_mouse_click(mc2_x, mc2_y, 1920.0, 1080.0),
            (true, SaveLoadAction::None)
        );
        assert_eq!(screen.active_card, 1);

        // Click Memory Card 1 tab (vx=139, vy=70) -> switches back to 0
        let (mc1_x, mc1_y) = sl_virtual_to_win(139.0, 70.0, 1920.0, 1080.0);
        assert_eq!(
            screen.handle_mouse_click(mc1_x, mc1_y, 1920.0, 1080.0),
            (true, SaveLoadAction::None)
        );
        assert_eq!(screen.active_card, 0);

        // Portrait (< 4:3) window (600x1000): click occupied slot row 1 -> enters ConfirmOverwrite
        let row1_vy = 150.0 + 40.0 + 18.0;
        let (sx, sy) = sl_virtual_to_win(256.0, row1_vy, 600.0, 1000.0);
        let (changed, action) = screen.handle_mouse_click(sx, sy, 600.0, 1000.0);
        assert!(changed);
        assert_eq!(action, SaveLoadAction::None);
        assert_eq!(screen.step, SaveLoadStep::ConfirmOverwrite);
        assert_eq!(screen.yes_no, YesNoChoice::No);

        // Hover and click "Yes" in ConfirmOverwrite -> emits WriteSlot for card 0, file 1
        let (yx, yy) = sl_virtual_to_win(200.0, 450.0, 600.0, 1000.0);
        assert!(screen.handle_mouse_move(yx, yy, 600.0, 1000.0));
        assert_eq!(screen.yes_no, YesNoChoice::Yes);
        let (changed, action) = screen.handle_mouse_click(yx, yy, 600.0, 1000.0);
        assert!(changed);
        assert_eq!(
            action,
            SaveLoadAction::WriteSlot {
                card_index: 0,
                file_index: 1,
                kind: SaveSlotKind::Manual,
            }
        );

        // Wheel ignored outside SelectSlot; click on SaveDone dismisses back to SelectSlot
        screen.step = SaveLoadStep::SaveDone;
        assert!(!screen.handle_mouse_wheel(1));
        let (dismissed, action) = screen.handle_mouse_click(yx, yy, 600.0, 1000.0);
        assert!(dismissed);
        assert_eq!(action, SaveLoadAction::None);
        assert_eq!(screen.step, SaveLoadStep::SelectSlot);
    }
}
