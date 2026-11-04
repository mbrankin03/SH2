//! sh2-ui: Dual-screen companion UI, dynamic map (Maps 1-36), quick-slots, touch dev console.

pub mod button_atlas;
pub mod font;
pub mod map;
pub mod menu;
pub mod options;
pub mod quickslots;

pub use button_atlas::{AuthenticButtonSprite, ButtonSpriteSheet};
pub use font::TextRasterizer;
pub use menu::{MainMenuOption, MenuNavAction, MenuSoundCue, TitlePhase, TitleStateMachine};
pub use options::{OptionsMenu, OptionsSoundCue, OptionsTab};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ActivePanel {
    #[default]
    Map,
    Memos,
    QuickSlots,
    DevConsole,
}
