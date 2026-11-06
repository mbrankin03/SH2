//! sh2-ui: Dual-screen companion UI, dynamic map (Maps 1-36), quick-slots, touch dev console.

pub mod atlas_font;
pub mod button_atlas;
pub mod font;
pub mod map;
pub mod menu;
pub mod options;
pub mod pause;
pub mod quickslots;

pub use atlas_font::{draw_subtitle, AtlasFont, Sh2TextColor};
pub use button_atlas::{AuthenticButtonSprite, ButtonSpriteSheet};
pub use font::TextRasterizer;
pub use menu::{
    draw_difficulty_selection, draw_scenario_selection, draw_test_rooms_screen, draw_title_screen,
    MainMenuOption, MenuNavAction, MenuSoundCue, TitlePhase, TitleStateMachine,
};
pub use options::{draw_options_screen, OptionsMenu, OptionsSoundCue, OptionsTab};
pub use pause::{
    draw_pause_screen, PauseAction, PauseMenu, PauseMenuOption, PauseSubPhase,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ActivePanel {
    #[default]
    Map,
    Memos,
    QuickSlots,
    DevConsole,
}
