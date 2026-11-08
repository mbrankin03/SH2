//! Modular per-room controllers (`audio_room`, `combat_arena`, `grapple_room`, `model_viewer`, `fmv_room`).

pub(crate) mod audio_room;
pub(crate) mod combat_arena;
pub(crate) mod fmv_room;
pub(crate) mod grapple_room;
pub(crate) mod model_viewer;
pub(crate) mod surface_sfx_room;

use std::time::Duration;
use winit::keyboard::KeyCode;

use crate::state::AppState;
use sh2_ui::menu::TestRoomKind;

/// Common controller interface for interactive rooms and inspection environments.
pub(crate) trait RoomController {
    /// Handles keyboard input while this room is active in-game.
    fn handle_key_input(state: &mut AppState, key_code: KeyCode, pressed: bool, repeat: bool);

    /// Advances room simulation state and renders the 3D scene or video surface into the offscreen target.
    fn tick_and_render_3d(
        state: &mut AppState,
        dt: Duration,
        dt_sec: f32,
        aspect: f32,
        grain_pct: u32,
        is_paused: bool,
        is_in_game: bool,
    );

    /// Renders the room's 2D telemetry/controls HUD overlay onto the swapchain surface view.
    fn render_hud(
        state: &mut AppState,
        encoder: &mut wgpu::CommandEncoder,
        surface_view: &wgpu::TextureView,
    );
}

/// Dispatches in-game keyboard input to the active room's [`RoomController`].
pub(crate) fn dispatch_key_input(
    state: &mut AppState,
    key_code: KeyCode,
    pressed: bool,
    repeat: bool,
) {
    match state.active_test_room {
        TestRoomKind::FmvCutsceneViewer => {
            fmv_room::FmvRoomController::handle_key_input(state, key_code, pressed, repeat);
        }
        TestRoomKind::ModelAndTextureViewer => {
            model_viewer::ModelViewerRoomController::handle_key_input(
                state, key_code, pressed, repeat,
            );
        }
        TestRoomKind::GrappleAnimationRoom => {
            grapple_room::GrappleRoomController::handle_key_input(state, key_code, pressed, repeat);
        }
        TestRoomKind::AudioDebugRoom => {
            audio_room::AudioDebugRoomController::handle_key_input(
                state, key_code, pressed, repeat,
            );
        }
        TestRoomKind::SurfaceSfxTestRoom => {
            surface_sfx_room::SurfaceSfxRoomController::handle_key_input(
                state, key_code, pressed, repeat,
            );
        }
        _ => {
            combat_arena::CombatArenaRoomController::handle_key_input(
                state, key_code, pressed, repeat,
            );
        }
    }
}

/// Dispatches 3D scene or video simulation and rendering to the active room's [`RoomController`].
pub(crate) fn dispatch_tick_and_render_3d(
    state: &mut AppState,
    dt: Duration,
    dt_sec: f32,
    aspect: f32,
    grain_pct: u32,
    is_paused: bool,
    is_in_game: bool,
) {
    if is_in_game {
        match state.active_test_room {
            TestRoomKind::FmvCutsceneViewer => {
                fmv_room::FmvRoomController::tick_and_render_3d(
                    state, dt, dt_sec, aspect, grain_pct, is_paused, is_in_game,
                );
                return;
            }
            TestRoomKind::ModelAndTextureViewer => {
                model_viewer::ModelViewerRoomController::tick_and_render_3d(
                    state, dt, dt_sec, aspect, grain_pct, is_paused, is_in_game,
                );
                return;
            }
            TestRoomKind::GrappleAnimationRoom => {
                grapple_room::GrappleRoomController::tick_and_render_3d(
                    state, dt, dt_sec, aspect, grain_pct, is_paused, is_in_game,
                );
                return;
            }
            TestRoomKind::AudioDebugRoom => {
                audio_room::AudioDebugRoomController::tick_and_render_3d(
                    state, dt, dt_sec, aspect, grain_pct, is_paused, is_in_game,
                );
                return;
            }
            TestRoomKind::SurfaceSfxTestRoom => {
                surface_sfx_room::SurfaceSfxRoomController::tick_and_render_3d(
                    state, dt, dt_sec, aspect, grain_pct, is_paused, is_in_game,
                );
                return;
            }
            _ => {}
        }
    }

    combat_arena::CombatArenaRoomController::tick_and_render_3d(
        state, dt, dt_sec, aspect, grain_pct, is_paused, is_in_game,
    );
}

/// Dispatches 2D HUD overlay rendering to the active room's [`RoomController`].
pub(crate) fn dispatch_render_hud(
    state: &mut AppState,
    encoder: &mut wgpu::CommandEncoder,
    surface_view: &wgpu::TextureView,
) {
    match state.active_test_room {
        TestRoomKind::ModelAndTextureViewer => {
            model_viewer::ModelViewerRoomController::render_hud(state, encoder, surface_view);
        }
        TestRoomKind::FmvCutsceneViewer => {
            fmv_room::FmvRoomController::render_hud(state, encoder, surface_view);
        }
        TestRoomKind::GrappleAnimationRoom => {
            grapple_room::GrappleRoomController::render_hud(state, encoder, surface_view);
        }
        TestRoomKind::AudioDebugRoom => {
            audio_room::AudioDebugRoomController::render_hud(state, encoder, surface_view);
        }
        TestRoomKind::SurfaceSfxTestRoom => {
            surface_sfx_room::SurfaceSfxRoomController::render_hud(state, encoder, surface_view);
        }
        _ => {
            if state.debug_camera_hud {
                combat_arena::CombatArenaRoomController::render_hud(state, encoder, surface_view);
            }
        }
    }
}
