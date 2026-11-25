//! sh2-ui: Dual-screen companion UI, dynamic map (Maps 1-36), quick-slots, touch dev console.

pub mod atlas_font;
pub mod button_atlas;
pub mod credits;
pub mod font;
pub mod gameover;
pub mod inventory;
pub mod loading;
pub mod map;
pub mod memo;
pub mod menu;
pub mod mes;
pub mod options;
pub mod pause;
pub mod quickslots;
pub mod results;
pub mod save_load;

pub use atlas_font::{draw_subtitle, AtlasFont, Sh2TextColor};
pub use button_atlas::{AuthenticButtonSprite, ButtonSpriteSheet};
pub use credits::{
    CreditsSequencePhase, EndingCreditsSequence, CREDITS_BIK_DURATION_SECS, END_BIK_DURATION_SECS,
    END_DOG_BIK_DURATION_SECS, END_WISH_BIK_DURATION_SECS, IN_WATER_BEDSIDE_DURATION_SECS,
    IN_WATER_CLOSING_SCROLL_DURATION_SECS, WATER_BIK_DURATION_SECS,
};
pub use font::TextRasterizer;
pub use gameover::{draw_gameover_screen, GameOverPhase, GameOverScreen, GAMEOVER_TEX_PATH};
pub use inventory::{
    draw_inventory_screen, CornerButton, HealthEcgState, HealthStatus, InventoryAction,
    InventoryFocus, InventoryPlayerSnapshot, InventoryScreen, ItemCommand, RedCrossIconState,
    RED_CROSS_ICON_PATH,
};
pub use loading::{
    FootstepPrint, FootstepSprite, LoadingPathPattern, LoadingScreen, LoadingScreenMode,
};
pub use map::{
    active_markers_for_map, chizu_can_read_map, chizu_connect_check, chizu_connect_neighbor,
    chizu_current_position_check, chizu_marker_layers, chizu_possession_check, chizu_select,
    chizu_world_to_sheet_norm, draw_paper_map, map_spec, stage_to_default_room_id,
    ChizuDecodedMarker, ChizuPackedMarker, MapCanvasClipper, MapDescriptor, MapDirection,
    MapPlayerMarker, MapReadCheck, MapSheetSpec, MapTranscriptionPhase, MapZoomStep,
    PaperMapAction, PaperMapPlayerState, PaperMapTextures, PaperMapViewer, CHIZU_CURSOR_TEX_PATH,
    CHIZU_PAPER_SE_ID,
};
pub use memo::{
    collected_memos, draw_memo_reader, memo_command_check, resolve_memo_textures, MemoAction,
    MemoDisplayMode, MemoEntry, MemoPhase, MemoReader, MemoRenderContext, MemoRenderTextures,
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
    draw_options_screen, ModUiEntry, OptionsMenu, OptionsSoundCue, OptionsSubScreen, OptionsTab,
};
pub use pause::{draw_pause_screen, PauseAction, PauseMenu, PauseMenuOption, PauseSubPhase};
pub use results::{draw_results_screen, ResultsRowData, ResultsScreen};
pub use save_load::{
    draw_save_load_screen, SaveAddInfoMode, SaveLoadAction, SaveLoadConfig, SaveLoadMode,
    SaveLoadScreen, SaveLoadStep, YesNoChoice, SAVEBG2_TBN2_PATH, SAVEBG_TBN2_PATH,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ActivePanel {
    #[default]
    Map,
    Memos,
    QuickSlots,
    DevConsole,
}
