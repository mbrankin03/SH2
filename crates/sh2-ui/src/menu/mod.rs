//! Title screen, boot logo sequence, difficulty selection, and main menu subsystem.
//!
//! Decomposed into cohesive submodules (`AGENTS.md` §2):
//! - [`boot`]: `logo.c` (`0x0020c910..0x0020d220`) & `sh2pc.exe` (`0x00405e80`) boot sequence.
//! - [`types`]: Shared enums, save-aware slot visibility (`titleCreateMainMenu` `0x001c1a90`), and
//!   Extra Riddle unlock logic (`sh2pc.exe` `0x0048aa20`).
//! - [`state`]: [`TitleStateMachine`] (`Event/title.c` `0x001c0b00..0x001c4740`).
//! - [`render`]: 4:3-scaled virtual 512×448 title, scenario, side-by-side difficulty, and credits rendering.

pub mod boot;
pub mod render;
pub mod state;
pub mod types;

pub use boot::{
    BootLogoKind, BootLogoStage, BootProfile, BootSequenceState, PC_LOGO_FADE_SECS,
    PC_LOGO_HOLD_SECS, PC_LOGO_SKIP_FADE_SECS, PS2_LOGO_FADE_SECS, PS2_LOGO_HOLD_SECS,
};
pub use render::{
    draw_difficulty_selection, draw_pc_mouse_cursor, draw_pulsing_title_logo_for_bg,
    draw_pulsing_title_logo_with_style, draw_scenario_selection, draw_test_rooms_screen,
    draw_title_screen,
};
pub use state::{
    TitleStateMachine, ACTION_DIFFICULTY_ROWS, RIDDLE_DIFFICULTY_ROWS, TITLE_ATTRACT_TIMEOUT_SECS,
    TITLE_CONFIRM_BLINK_RAD_PER_30HZ, TITLE_FADE_DURATION_SECS, TITLE_IDLE_PULSE_RAD_PER_30HZ,
};
pub use types::{
    resolve_effective_riddle, LaunchTarget, MainMenuOption, MenuNavAction, MenuSoundCue,
    SaveAvailability, TestRoomKind, TitlePhase,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::atlas_font::AtlasFont;
    use crate::font::TextRasterizer;
    use sh2_core::config::{ActionDifficulty, RiddleDifficulty};
    use sh2_core::scenario::ScenarioKind;

    #[test]
    fn test_boot_sequence_ps2_timing_3s_hold_0_3s_fade() {
        let mut boot = BootSequenceState::new(BootProfile::Ps2Original);
        assert_eq!(boot.current_logo(), Some(BootLogoKind::CesaWarning));
        assert_eq!(boot.stage, BootLogoStage::FadeIn);

        // 0.3s fade-in -> Hold
        boot.tick(0.3);
        assert_eq!(boot.stage, BootLogoStage::Hold);
        assert!((boot.alpha - 1.0).abs() < 1e-4);

        // Unskippable in strict PS2 mode unless explicitly allowed
        assert!(!boot.handle_skip(false));

        // 3.0s hold -> FadeOut
        boot.tick(3.0);
        assert_eq!(boot.stage, BootLogoStage::FadeOut);

        // 0.3s fade-out -> SCE Warning
        boot.tick(0.3);
        assert_eq!(boot.current_logo(), Some(BootLogoKind::SceWarning));
        assert_eq!(boot.stage, BootLogoStage::FadeIn);

        // Advance through SCE & Konami
        for _ in 0..2 {
            boot.tick(0.3);
            boot.tick(3.0);
            boot.tick(0.3);
        }
        assert_eq!(boot.stage, BootLogoStage::Complete);
        assert_eq!(boot.current_logo(), None);
    }

    #[test]
    fn test_boot_sequence_retail_pc_fast_skip() {
        let mut boot = BootSequenceState::new(BootProfile::RetailPc);
        assert_eq!(boot.current_logo(), Some(BootLogoKind::KonamiLogo));
        // Skip fade-in -> Hold
        assert!(boot.handle_skip(false));
        assert_eq!(boot.stage, BootLogoStage::Hold);
        // Skip during hold -> 0.5s fast fade-out (`sh2pc` 0x00405e80)
        assert!(boot.handle_skip(false));
        assert_eq!(boot.stage, BootLogoStage::FadeOut);
        assert!((boot.active_fade_out_secs - 0.5).abs() < 1e-5);
        boot.tick(0.5);
        assert_eq!(boot.current_logo(), Some(BootLogoKind::SplashLogo));
    }

    #[test]
    fn test_save_aware_main_menu_slots() {
        assert_eq!(
            SaveAvailability::None.visible_slots(true),
            vec![
                MainMenuOption::NewGame,
                MainMenuOption::Options,
                MainMenuOption::Movies,
                MainMenuOption::Exit,
            ]
        );
        assert_eq!(
            SaveAvailability::LoadAndContinue.visible_slots(false),
            vec![
                MainMenuOption::Load,
                MainMenuOption::Continue,
                MainMenuOption::NewGame,
                MainMenuOption::Options,
            ]
        );
        assert_eq!(SaveAvailability::LoadAndContinue.default_cursor_index(), 1);
    }

    #[test]
    fn test_title_attract_timeout_60s_and_reset() {
        let mut sm = TitleStateMachine::new();
        sm.show_phase_immediately(TitlePhase::MainMenu);
        sm.tick(30.0);
        assert!((sm.attract_timer - 30.0).abs() < 1e-4);
        assert!(!sm.attract_movie_requested);

        // Moving cursor resets timer back to 60.0s (`titleMainSelect` 0x001c1e40)
        sm.handle_input(MenuNavAction::Down);
        assert!((sm.attract_timer - 60.0).abs() < 1e-4);

        // Letting 60.0s elapse requests OP attract movie & starts 2.0s fade-out
        sm.tick(60.0);
        assert!(sm.attract_movie_requested);
        assert!(sm.is_fading_out);
        assert!((sm.fade_speed - 0.5).abs() < 1e-5);
    }

    #[test]
    fn test_start_prompt_pulse_not_frozen() {
        let mut sm = TitleStateMachine::new();
        sm.show_phase_immediately(TitlePhase::TitleLogo);
        let t0 = sm.elapsed_timer;
        let p0 = sm.pulse_phase_rad;
        sm.tick(0.5);
        assert!(sm.elapsed_timer > t0);
        assert_ne!(sm.pulse_phase_rad, p0);
    }

    #[test]
    fn test_action_beginner_and_extra_riddle_unlock() {
        let mut sm = TitleStateMachine::new();
        sm.show_phase_immediately(TitlePhase::MainMenu);
        sm.riddle_clear_mask = 0x07; // Cleared Easy + Normal + Hard

        // New Game -> Letter From Silent Heaven -> ActionDifficultySelect
        sm.handle_input(MenuNavAction::Confirm);
        assert_eq!(sm.phase, TitlePhase::ScenarioSelect);
        sm.handle_input(MenuNavAction::Confirm);
        assert_eq!(sm.selected_scenario, ScenarioKind::LetterFromSilentHeaven);
        assert_eq!(sm.phase, TitlePhase::ActionDifficultySelect);

        // Normal -> Easy -> Beginner
        assert_eq!(sm.action_difficulty, ActionDifficulty::Normal);
        sm.handle_input(MenuNavAction::Down);
        assert_eq!(sm.action_difficulty, ActionDifficulty::Easy);
        sm.handle_input(MenuNavAction::Down);
        assert_eq!(sm.action_difficulty, ActionDifficulty::Beginner);

        // Confirm Action -> RiddleDifficultySelect (Normal -> Hard)
        sm.handle_input(MenuNavAction::Confirm);
        assert_eq!(sm.phase, TitlePhase::RiddleDifficultySelect);
        sm.handle_input(MenuNavAction::Up);
        assert_eq!(sm.riddle_difficulty, RiddleDifficulty::Hard);

        // Confirming Hard with 0x07 clear mask silently upgrades to Extra (`sh2pc` 0x0048aa20)
        let cue = sm.handle_input(MenuNavAction::Confirm);
        assert_eq!(cue, MenuSoundCue::GameStartTransition);
        assert_eq!(sm.riddle_difficulty, RiddleDifficulty::Extra);
        assert_eq!(sm.phase, TitlePhase::TransitionToGame);
    }

    #[test]
    fn test_bfaw_skips_riddle_difficulty() {
        let mut sm = TitleStateMachine::new();
        sm.show_phase_immediately(TitlePhase::ScenarioSelect);
        sm.handle_input(MenuNavAction::Down); // Born From a Wish
        sm.handle_input(MenuNavAction::Confirm);
        assert_eq!(sm.selected_scenario, ScenarioKind::BornFromAWish);
        assert_eq!(sm.phase, TitlePhase::ActionDifficultySelect);

        // Confirming Action launches straight into TransitionToGame (`sh2pc` 0x004984c7)
        let cue = sm.handle_input(MenuNavAction::Confirm);
        assert_eq!(cue, MenuSoundCue::GameStartTransition);
        assert_eq!(sm.phase, TitlePhase::TransitionToGame);
    }

    #[test]
    fn test_menu_rendering_and_credits_director() {
        let font = AtlasFont::load_embedded();
        let mut rast = TextRasterizer::new(640, 480);

        let mut sm = TitleStateMachine::new();
        sm.show_phase_immediately(TitlePhase::ActionDifficultySelect);
        draw_difficulty_selection(&sm, None, &font, &mut rast);
        assert!(rast.pixels.chunks(4).any(|p| p[3] > 0));

        rast.clear();
        sm.show_phase_immediately(TitlePhase::Credits);
        draw_title_screen(&sm, None, &font, &mut rast);
        assert!(rast.pixels.chunks(4).any(|p| p[3] > 0));
    }

    #[test]
    fn test_main_menu_exit_confirm_flow() {
        let mut sm = TitleStateMachine::new();
        sm.show_phase_immediately(TitlePhase::MainMenu);
        // Move to EXIT (last visible slot when include_pc_menu_slots == true)
        let exit_idx = sm
            .visible_main_menu_options()
            .iter()
            .position(|&o| o == MainMenuOption::Exit)
            .expect("EXIT slot should be visible");
        sm.main_menu_index = exit_idx;

        assert_eq!(
            sm.handle_input(MenuNavAction::Confirm),
            MenuSoundCue::Confirm
        );
        assert_eq!(sm.phase, TitlePhase::ExitConfirm);
        assert_eq!(sm.exit_confirm_index, 1); // Defaults to "No"
        assert!(!sm.exit_requested);

        // Confirming "No" returns to MainMenu without exiting
        assert_eq!(
            sm.handle_input(MenuNavAction::Confirm),
            MenuSoundCue::Cancel
        );
        assert_eq!(sm.phase, TitlePhase::MainMenu);
        assert!(!sm.exit_requested);

        // Re-open ExitConfirm, toggle to "Yes", confirm -> exit_requested == true
        sm.handle_input(MenuNavAction::Confirm);
        assert_eq!(
            sm.handle_input(MenuNavAction::Left),
            MenuSoundCue::CursorMove
        );
        assert_eq!(sm.exit_confirm_index, 0);
        assert_eq!(
            sm.handle_input(MenuNavAction::Confirm),
            MenuSoundCue::Confirm
        );
        assert!(sm.exit_requested);
    }
}
