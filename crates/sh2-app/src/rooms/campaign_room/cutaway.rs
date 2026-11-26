//! Fullscreen 2D cutaway still & animated sprite overlay rendering for unified Campaign mode
//! (`SH2-157`).
//!
//! Reuses [`crate::rooms::story_progression_room::hud_common::blit_cutaway_into_rasterizer`]
//! and `AppState::story_progression_room.cutaway_cache` so retail cutaway `.tex` loading,
//! `FullscreenImageLayout` aspect-ratio framing, `OverlaySpriteQuad` compositing (e.g.,
//! `out_p_incar.tex` + `out/p_incar_map.tex` at `(-48.375, -31.625)` with alpha fade), and
//! SH2:EE pillarbox bars have a single source of truth.

use crate::rooms::story_progression_room::hud_common::blit_cutaway_into_rasterizer;
use crate::state::AppState;
use sh2_core::riddle_level_index;

/// Returns `true` when `sim_core.scenario.cutaway_overlay` currently has an active 2D cutaway
/// background picture (e.g., `"out_p_incar"`).
#[must_use]
pub(crate) fn has_active_cutaway(state: &AppState) -> bool {
    state
        .sim_core
        .scenario
        .cutaway_overlay
        .active_picture()
        .is_some()
}

/// Renders the active `state.sim_core.scenario.cutaway_overlay` fullscreen onto
/// `state.menu_rasterizer`, including its background `.tex` and all active animated
/// `OverlaySpriteQuad` overlays (with live alpha fade).
pub(crate) fn render_campaign_cutaway(state: &mut AppState) {
    let Some(base_pic) = state
        .sim_core
        .scenario
        .cutaway_overlay
        .active_picture()
        .map(str::to_string)
    else {
        return;
    };

    // Ensure `frame_sprites` are up-to-date for the current flags/puzzle/alpha state.
    let riddle_idx = riddle_level_index(state.sim_core.scenario.riddle_difficulty);
    let quads = state
        .sim_core
        .scenario
        .cutaway_overlay
        .evaluate_frame(
            &state.sim_core.scenario.flags,
            &state.sim_core.scenario.puzzle_state,
            riddle_idx,
            0,
        )
        .to_vec();

    let w = state.menu_rasterizer.width;
    let h = state.menu_rasterizer.height;
    // Clear fullscreen backdrop to opaque black so 3D scene pixels never bleed around pillarboxes.
    state.menu_rasterizer.fill_rect(0, 0, w, h, [0, 0, 0, 255]);

    let video = &state.menu_ctrl.options_menu.draft_config.video;
    let quality = if video.texture_quality == sh2_core::config::WorldTextureQuality::OriginalPs2 {
        sh2_core::CutawayQuality::OriginalPs2
    } else {
        video.cutaway_quality
    };
    let aspect_mode = video.cutaway_aspect_mode;

    blit_cutaway_into_rasterizer(
        &mut state.menu_rasterizer,
        &mut state.story_progression_room.cutaway_cache,
        0,
        0,
        w,
        h,
        &base_pic,
        quality,
        aspect_mode,
        &quads,
    );
}
