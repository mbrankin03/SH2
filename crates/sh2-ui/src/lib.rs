//! sh2-ui: Dual-screen companion UI, dynamic map (Maps 1-36), quick-slots, touch dev console.

pub mod atlas_font;
pub mod button_atlas;
pub mod credits;
pub mod font;
pub mod inventory;
pub mod loading;
pub mod map;
pub mod memo;
pub mod menu;
pub mod mes;
pub mod options;
pub mod pause;
pub mod quickslots;

pub use atlas_font::{draw_subtitle, AtlasFont, Sh2TextColor};
pub use button_atlas::{AuthenticButtonSprite, ButtonSpriteSheet};
pub use credits::{
    CreditsSequencePhase, EndingCreditsSequence, CREDITS_BIK_DURATION_SECS, END_BIK_DURATION_SECS,
    END_DOG_BIK_DURATION_SECS, END_WISH_BIK_DURATION_SECS, IN_WATER_BEDSIDE_DURATION_SECS,
    IN_WATER_CLOSING_SCROLL_DURATION_SECS, WATER_BIK_DURATION_SECS,
};
pub use font::TextRasterizer;
pub use inventory::{
    draw_inventory_screen, CornerButton, HealthEcgState, HealthStatus, InventoryAction,
    InventoryFocus, InventoryPlayerSnapshot, InventoryScreen, ItemCommand,
};
pub use loading::{FootstepPrint, FootstepSprite, LoadingScreen};
pub use map::{
    active_markers_for_map, chizu_can_read_map, chizu_connect_neighbor, chizu_possession_check,
    chizu_world_to_sheet_norm, draw_paper_map, map_spec, ChizuDecodedMarker, ChizuPackedMarker,
    MapCanvasClipper, MapDescriptor, MapDirection, MapPlayerMarker, MapReadCheck, MapSheetSpec,
    MapTranscriptionPhase, MapZoomStep, PaperMapAction, PaperMapViewer,
};
pub use memo::{
    collected_memos, draw_memo_reader, memo_command_check, MemoAction, MemoEntry, MemoPhase,
    MemoReader,
};
pub use menu::{
    draw_difficulty_selection, draw_scenario_selection, draw_test_rooms_screen, draw_title_screen,
    resolve_effective_riddle, BootLogoKind, BootLogoStage, BootProfile, BootSequenceState,
    LaunchTarget, MainMenuOption, MenuNavAction, MenuSoundCue, SaveAvailability, TitlePhase,
    TitleStateMachine,
};
pub use mes::{
    font_set_color, yes_no_bar_alpha_gs, MesAlign, MesDrawCmd, MesFontType, MesInterpreter,
    MesLayoutResult, MesWaitMode, FONT_COLOR_PALETTE_PS2, YES_NO_BAR_RGB, YES_NO_NO_CENTER_X,
    YES_NO_YES_CENTER_X,
};
pub use options::{
    draw_options_screen, OptionsMenu, OptionsSoundCue, OptionsSubScreen, OptionsTab,
};
pub use pause::{draw_pause_screen, PauseAction, PauseMenu, PauseMenuOption, PauseSubPhase};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ActivePanel {
    #[default]
    Map,
    Memos,
    QuickSlots,
    DevConsole,
}
