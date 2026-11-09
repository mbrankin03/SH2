//! Interactive Desktop Playable Test Build for Silent Hill 2 Open Port.
//! Runs the pure PS2 camera, 60 Hz simulation core, volumetric fog, character skeletal animation, and gamepad controls.

mod assets;
mod bootstrap;
mod cli;
mod frame;
mod menu_events;
mod rooms;
mod state;

use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::WindowId;

use crate::bootstrap::bootstrap_app_state;
use crate::cli::CliArgs;
use crate::frame::redraw_frame;
use crate::menu_events::{
    emit_menu_sound, handle_pause_menu_key_input, handle_title_menu_key_input,
};
use crate::state::AppState;
use sh2_ui::menu::{MenuNavAction, TestRoomKind};

#[derive(Default)]
struct SilentHillApp {
    state: Option<AppState>,
    cli_args: CliArgs,
}

impl ApplicationHandler for SilentHillApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }
        self.state = Some(bootstrap_app_state(event_loop, &self.cli_args));
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        let Some(ref mut state) = self.state else {
            return;
        };

        match event {
            WindowEvent::KeyboardInput {
                event: key_event, ..
            } => {
                if let winit::keyboard::PhysicalKey::Code(key_code) = key_event.physical_key {
                    let pressed = key_event.state == winit::event::ElementState::Pressed;

                    if !state.menu_ctrl.in_game_active {
                        handle_title_menu_key_input(state, key_code, pressed);
                        return;
                    }

                    if state.menu_ctrl.in_game_active && state.menu_ctrl.pause_menu.is_open {
                        handle_pause_menu_key_input(state, key_code, pressed);
                        return;
                    }

                    rooms::dispatch_key_input(state, key_code, pressed, key_event.repeat);
                }
            }
            WindowEvent::MouseInput {
                state: element_state,
                button,
                ..
            } => {
                let pressed = element_state == winit::event::ElementState::Pressed;
                if button == winit::event::MouseButton::Left {
                    state.mouse_left_pressed = pressed;
                } else if button == winit::event::MouseButton::Right {
                    state.mouse_right_pressed = pressed;
                }

                if pressed && !state.menu_ctrl.in_game_active {
                    let sound = state.menu_ctrl.handle_nav(MenuNavAction::Confirm);
                    emit_menu_sound(state, sound, "Menu");
                } else if state.menu_ctrl.in_game_active
                    && state.active_test_room == TestRoomKind::CombatArena
                {
                    if button == winit::event::MouseButton::Right {
                        state.key_ready_held = pressed;
                    } else if button == winit::event::MouseButton::Left {
                        if pressed && !state.key_attack_held {
                            state.key_attack_pressed = true;
                        }
                        state.key_attack_held = pressed;
                    }
                    if pressed
                        && state.combat_arena.player.mode == sh2_core::combat::CombatMode::Held
                    {
                        state.struggle_button_press_count += 1;
                    }
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                if let Some((last_x, last_y)) = state.last_cursor_pos {
                    let dx = (position.x - last_x) as f32;
                    let dy = (position.y - last_y) as f32;
                    if state.menu_ctrl.in_game_active {
                        if state.active_test_room == TestRoomKind::ModelAndTextureViewer {
                            if state.mouse_left_pressed {
                                state.model_viewer.camera.orbit(-dx * 0.007, dy * 0.007);
                            } else if state.mouse_right_pressed {
                                state.model_viewer.camera.pan(-dx * 2.0, dy * 2.0);
                            }
                        } else if state.active_test_room == TestRoomKind::GrappleAnimationRoom {
                            if state.mouse_left_pressed {
                                state.grapple_viewer.camera.orbit(-dx * 0.007, dy * 0.007);
                            } else if state.mouse_right_pressed {
                                state.grapple_viewer.camera.pan(-dx * 2.0, dy * 2.0);
                            }
                        }
                    }
                }
                state.last_cursor_pos = Some((position.x, position.y));
            }
            WindowEvent::MouseWheel { delta, .. } => {
                if state.menu_ctrl.in_game_active {
                    let scroll = match delta {
                        winit::event::MouseScrollDelta::LineDelta(_, y) => y * 150.0,
                        winit::event::MouseScrollDelta::PixelDelta(pos) => pos.y as f32 * 1.5,
                    };
                    if state.active_test_room == TestRoomKind::ModelAndTextureViewer {
                        state.model_viewer.camera.zoom(-scroll);
                    } else if state.active_test_room == TestRoomKind::GrappleAnimationRoom {
                        state.grapple_viewer.camera.zoom(-scroll);
                    }
                }
            }
            WindowEvent::CloseRequested => {
                tracing::info!("Close requested, shutting down...");
                event_loop.exit();
            }
            WindowEvent::Focused(focused) => {
                let mute_in_bg = state
                    .menu_ctrl
                    .options_menu
                    .original_config
                    .audio
                    .mute_in_background;
                if mute_in_bg {
                    if let Some(ref mut audio) = state.audio_engine {
                        if !focused {
                            audio.set_master_volume(0.0);
                            tracing::info!("[Options] Focus lost; muted audio in background.");
                        } else {
                            let master_vol = state
                                .menu_ctrl
                                .options_menu
                                .original_config
                                .audio
                                .master_volume;
                            audio.set_master_volume(master_vol);
                            tracing::info!(
                                "[Options] Focus restored; restored audio volume to {:.0}%.",
                                master_vol * 100.0
                            );
                        }
                    }
                }
            }
            WindowEvent::Resized(new_size) => {
                if new_size.width > 0 && new_size.height > 0 {
                    state.surface_config.width = new_size.width;
                    state.surface_config.height = new_size.height;
                    state
                        .surface
                        .configure(&state.render_ctx.device, &state.surface_config);
                    state.render_ctx.resize(new_size.width, new_size.height);
                    state.blit_bind_group = state.blitter.create_bind_group(
                        &state.render_ctx.device,
                        &state.render_ctx.ldr_output_view,
                    );
                }
            }
            WindowEvent::RedrawRequested => {
                redraw_frame(state);
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(ref state) = self.state {
            if let Some(deadline) = state.frame_pacer.next_deadline() {
                event_loop.set_control_flow(ControlFlow::WaitUntil(deadline));
            } else {
                event_loop.set_control_flow(ControlFlow::Poll);
            }
            state.window.request_redraw();
        }
    }
}

fn main() {
    tracing_subscriber::fmt::init();
    tracing::info!(
        "Starting Silent Hill 2 Open Port Interactive Playable Test with Skeletal Mesh Runtime..."
    );

    let cli_args = CliArgs::parse_from_env();

    let event_loop = EventLoop::new().expect("Failed to create winit event loop");
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = SilentHillApp {
        state: None,
        cli_args,
    };
    event_loop.run_app(&mut app).expect("Event loop error");
}
