//! Interactive Stage Event Engine (`src/Event/event.c`) & Environmental Horror Triggers Chamber (`SH2-47`).
//!
//! Modularized into:
//! - [`scenarios`]: Strongly typed [`HorrorScenarioSpec`] catalog, [`HorrorTriggersRoomState`], and synthetic `Event_List` / `ev_pos` builders.
//! - [`audio_dispatch`]: 3D spatial audio routing (`SeCallPos`, `SePosMove`, `SePosSet`, `SeCall`) via `MenuAudioBank`.
//! - [`input`]: Keyboard shortcuts (`[1]`–`[8]`, `[Tab]`, `[Space]`, `[E]`, `[K]`, `[T]`, `[R]`, `[V]`) and scenario step triggers.
//! - [`render`]: Per-frame `EventEngine::tick_standard` + `StageEventDispatcher::on_all_time` evaluation and 3D wireframe trigger guides.
//! - [`hud`]: 2D telemetry and scenario selector overlay.

pub(crate) mod audio_dispatch;
pub(crate) mod hud;
pub(crate) mod input;
pub(crate) mod render;
pub(crate) mod scenarios;

pub(crate) use scenarios::HorrorTriggersRoomState;

use std::time::Duration;
use winit::keyboard::KeyCode;

use crate::rooms::RoomController;
use crate::state::AppState;

pub(crate) struct HorrorTriggersRoomController;

impl RoomController for HorrorTriggersRoomController {
    fn handle_key_input(state: &mut AppState, key_code: KeyCode, pressed: bool, repeat: bool) {
        input::handle_horror_room_key_input(state, key_code, pressed, repeat);
    }

    fn tick_and_render_3d(
        state: &mut AppState,
        dt: Duration,
        dt_sec: f32,
        aspect: f32,
        grain_pct: u32,
        is_paused: bool,
        is_in_game: bool,
    ) {
        render::tick_and_render_horror_room_3d(
            state, dt, dt_sec, aspect, grain_pct, is_paused, is_in_game,
        );
    }

    fn render_hud(
        state: &mut AppState,
        encoder: &mut wgpu::CommandEncoder,
        surface_view: &wgpu::TextureView,
    ) {
        hud::render_horror_room_hud(state, encoder, surface_view);
    }
}
