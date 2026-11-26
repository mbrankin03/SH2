//! Interactive Desktop Playable Test Build for Silent Hill 2 Open Port.
//! Runs the pure PS2 camera, 60 Hz simulation core, volumetric fog, character skeletal animation, and gamepad controls.

mod assets;
mod audio_dispatch;
mod bootstrap;
mod cli;
mod frame;
mod ingame_menus;
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
                        if state.menu_ctrl.title_sm.exit_requested {
                            tracing::info!("Main Menu: Exit confirmed, closing application.");
                            event_loop.exit();
                        }
                        return;
                    }

                    if state.menu_ctrl.in_game_active && state.menu_ctrl.pause_menu.is_open {
                        handle_pause_menu_key_input(state, key_code, pressed);
                        return;
                    }

                    if state.menu_ctrl.in_game_active && state.ingame_menus.is_any_open() {
                        ingame_menus::handle_key_input(state, key_code, pressed);
                        return;
                    }

                    if pressed
                        && !key_event.repeat
                        && state.menu_ctrl.in_game_active
                        && state.is_3d_gameplay_room()
                    {
                        match key_code {
                            winit::keyboard::KeyCode::Tab | winit::keyboard::KeyCode::KeyI => {
                                ingame_menus::open_inventory(state);
                                return;
                            }
                            winit::keyboard::KeyCode::KeyM => {
                                ingame_menus::open_map_direct(state, false);
                                return;
                            }
                            _ => {}
                        }
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
                    let vw = state.surface_config.width.max(1) as f32;
                    let vh = state.surface_config.height.max(1) as f32;
                    let sound = if button == winit::event::MouseButton::Right {
                        state.menu_ctrl.handle_nav(MenuNavAction::Cancel)
                    } else if let Some((mx, my)) = state.last_cursor_pos {
                        state
                            .menu_ctrl
                            .handle_mouse_click(mx as f32, my as f32, vw, vh)
                    } else {
                        state.menu_ctrl.handle_nav(MenuNavAction::Confirm)
                    };
                    emit_menu_sound(state, sound, "Menu");
                    if state.menu_ctrl.title_sm.exit_requested {
                        tracing::info!("Main Menu: Exit confirmed via mouse, closing application.");
                        event_loop.exit();
                    }
                } else if pressed
                    && state.menu_ctrl.in_game_active
                    && state.ingame_menus.is_any_open()
                {
                    if button == winit::event::MouseButton::Right
                        && state.ingame_menus.options_from_inventory
                    {
                        ingame_menus::handle_key_input(
                            state,
                            winit::keyboard::KeyCode::Escape,
                            true,
                        );
                    } else if button == winit::event::MouseButton::Left {
                        if let Some((mx, my)) = state.last_cursor_pos {
                            ingame_menus::handle_mouse_click(state, mx as f32, my as f32);
                        }
                    }
                } else if state.menu_ctrl.in_game_active
                    && !state.menu_ctrl.pause_menu.is_open
                    && !state.ingame_menus.is_any_open()
                    && state.is_3d_gameplay_room()
                {
                    if button == winit::event::MouseButton::Right {
                        state.key_ready_held = pressed;
                    } else if button == winit::event::MouseButton::Left
                        && state.active_test_room == TestRoomKind::CombatArena
                    {
                        if pressed && !state.key_attack_held {
                            state.key_attack_pressed = true;
                        }
                        state.key_attack_held = pressed;
                    }
                    if pressed
                        && state.active_test_room == TestRoomKind::CombatArena
                        && state.combat_arena.player.mode == sh2_core::combat::CombatMode::Held
                    {
                        state.struggle_button_press_count += 1;
                    }
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                let vw = state.surface_config.width.max(1) as f32;
                let vh = state.surface_config.height.max(1) as f32;
                if !state.menu_ctrl.in_game_active
                    || (state.menu_ctrl.pause_menu.is_open
                        && state.menu_ctrl.pause_menu.sub_phase
                            == sh2_ui::pause::PauseSubPhase::InOptionsMenu)
                {
                    let cue = state.menu_ctrl.handle_mouse_move(
                        position.x as f32,
                        position.y as f32,
                        vw,
                        vh,
                    );
                    if cue != sh2_ui::menu::MenuSoundCue::None {
                        emit_menu_sound(state, cue, "MenuHover");
                    }
                } else if state.menu_ctrl.in_game_active && state.ingame_menus.is_any_open() {
                    ingame_menus::handle_mouse_move(state, position.x as f32, position.y as f32);
                }
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
                let raw_lines = match delta {
                    winit::event::MouseScrollDelta::LineDelta(_, y) => y,
                    winit::event::MouseScrollDelta::PixelDelta(pos) => (pos.y as f32) / 36.0,
                };
                if !state.menu_ctrl.in_game_active
                    || (state.menu_ctrl.pause_menu.is_open
                        && state.menu_ctrl.pause_menu.sub_phase
                            == sh2_ui::pause::PauseSubPhase::InOptionsMenu)
                    || state.ingame_menus.options_from_inventory
                {
                    let steps = if raw_lines.abs() >= 0.1 {
                        let s = (-raw_lines).round() as i32;
                        if s == 0 {
                            if raw_lines > 0.0 {
                                -1
                            } else {
                                1
                            }
                        } else {
                            s
                        }
                    } else {
                        0
                    };
                    if steps != 0 {
                        if state.ingame_menus.options_from_inventory {
                            ingame_menus::handle_mouse_wheel(state, steps);
                        } else {
                            let vw = state.surface_config.width.max(1) as f32;
                            let vh = state.surface_config.height.max(1) as f32;
                            let cue = state.menu_ctrl.handle_mouse_wheel(
                                steps,
                                state.last_cursor_pos,
                                vw,
                                vh,
                            );
                            if cue != sh2_ui::menu::MenuSoundCue::None {
                                emit_menu_sound(state, cue, "MenuScroll");
                            }
                        }
                    }
                } else if state.menu_ctrl.in_game_active {
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
                state.window_focused = focused;
                if !focused {
                    state.clear_held_gameplay_keys();
                }
                state.sync_cursor_grab_for_camera();
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

    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _device_id: winit::event::DeviceId,
        event: winit::event::DeviceEvent,
    ) {
        let Some(ref mut state) = self.state else {
            return;
        };
        if let winit::event::DeviceEvent::MouseMotion { delta: (dx, dy) } = event {
            if state.window_focused
                && state.menu_ctrl.in_game_active
                && !state.menu_ctrl.pause_menu.is_open
                && !state.ingame_menus.is_any_open()
                && state.is_3d_gameplay_room()
            {
                state
                    .sim_core
                    .accumulate_mouse_motion_dots(dx as f32, dy as f32);
            }
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
    if cli_args.verify_fmv_pack {
        let resolver = sh2_assets::AssetPathResolver::resolve();
        let Some(dir) = resolver.resolve_4k_vsr_fmv_dir() else {
            eprintln!("4K VSR FMV pack directory not found. Set SH2_VSR_DIR or place files in mods/4k_vsr_fmvs/ or sh2e/movie_4k/.");
            std::process::exit(1);
        };
        match sh2_assets::verify_fmv_pack(&dir) {
            Ok(report) => {
                println!(
                    "SH2 4K VSR FMV Pack v{} ({}): {}/{} verified",
                    report.version,
                    dir.display(),
                    report.verified,
                    report.total
                );
                for (name, status) in &report.results {
                    println!("  {name:<28} {status:?}");
                }
                std::process::exit(if report.all_verified() { 0 } else { 1 });
            }
            Err(err) => {
                eprintln!("Failed to verify FMV pack at {}: {err}", dir.display());
                std::process::exit(1);
            }
        }
    }

    let event_loop = EventLoop::new().expect("Failed to create winit event loop");
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = SilentHillApp {
        state: None,
        cli_args,
    };
    event_loop.run_app(&mut app).expect("Event loop error");
}
