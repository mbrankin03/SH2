//! 3D Stage Exploration & Combat Testing Arena room controller (`SH2-13` / `SH2-57` / `SH2-63`).
//!
//! Modularized into:
//! - [`input`]: Keyboard & controller bindings, weapon shortcuts, fog/debug toggles, and cutscene lockouts.
//! - [`render`]: Fixed 60 Hz simulation step, skeletal animation skinning, cutscene camera/light overrides, and 3D draw submission.
//! - [`hud`]: 2D HUD telemetry overlay and `.mes` subtitle rendering.

mod hud;
mod input;
mod render;

use std::time::Duration;
use winit::keyboard::KeyCode;

use crate::rooms::RoomController;
use crate::state::AppState;

pub(crate) struct CombatArenaRoomController;

impl RoomController for CombatArenaRoomController {
    fn handle_key_input(state: &mut AppState, key_code: KeyCode, pressed: bool, repeat: bool) {
        input::handle_combat_arena_key_input(state, key_code, pressed, repeat);
    }

    fn tick_and_render_3d(
        state: &mut AppState,
        dt: Duration,
        _dt_sec: f32,
        aspect: f32,
        _grain_pct: u32,
        is_paused: bool,
        is_in_game: bool,
    ) {
        render::tick_and_render_combat_arena_3d(state, dt, aspect, is_paused, is_in_game);
    }

    fn render_hud(
        state: &mut AppState,
        encoder: &mut wgpu::CommandEncoder,
        surface_view: &wgpu::TextureView,
    ) {
        hud::render_combat_arena_hud(state, encoder, surface_view);
    }
}
