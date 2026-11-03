//! Interactive Desktop Playable Test Build for Silent Hill 2 Open Port.
//! Runs the pure PS2 camera, 60 Hz simulation core, volumetric fog, and gamepad controls.

use std::sync::Arc;
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

use sh2_assets::formats::map::MapFile;
use sh2_core::{ControlScheme, ScenarioKind, SimulationCore};
use sh2_renderer::{GpuMap, RenderContext, RendererConfig, SurfaceBlitter};

struct AppState {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
    render_ctx: RenderContext,
    blitter: SurfaceBlitter,
    sim_core: SimulationCore,
    gpu_map: GpuMap,
    last_frame: Instant,
}

#[derive(Default)]
struct SilentHillApp {
    state: Option<AppState>,
}

impl ApplicationHandler for SilentHillApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }

        let window_attributes = Window::default_attributes()
            .with_title("Silent Hill 2 Open Port - Interactive PS2 Simulation Test")
            .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 720.0));

        let window = Arc::new(event_loop.create_window(window_attributes).unwrap());
        let size = window.inner_size();

        let render_config = RendererConfig {
            width: size.width.max(1),
            height: size.height.max(1),
            enable_hdr: true,
            enable_fog: true,
            enable_post: true,
        };

        let render_ctx = RenderContext::new_headless(render_config)
            .expect("Failed to initialize wgpu render context");

        // Create presentation surface
        let surface = unsafe {
            // Safety: Window is wrapped in Arc and lives as long as the surface
            let target = wgpu::SurfaceTargetUnsafe::from_window(&*window).unwrap();
            render_ctx.instance.create_surface_unsafe(target).unwrap()
        };

        let surface_caps = surface.get_capabilities(&render_ctx.adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: surface_caps.present_modes[0],
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&render_ctx.device, &surface_config);

        let blitter = SurfaceBlitter::new(&render_ctx.device, surface_format);

        // Load an authentic Silent Hill 2 level geometry map
        let map_path =
            "Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2/data/bg/rr/rr21.map";
        let map_file = match std::fs::read(map_path) {
            Ok(bytes) => {
                tracing::info!("Loaded authentic level geometry from {}", map_path);
                MapFile::parse(&bytes).unwrap_or_default()
            }
            Err(e) => {
                tracing::warn!(
                    "Could not read {}: {}. Generating test arena geometry.",
                    map_path,
                    e
                );
                MapFile::default()
            }
        };

        let gpu_map = GpuMap::from_map(&render_ctx.device, &map_file);
        let mut sim_core = SimulationCore::new(ScenarioKind::LetterFromSilentHeaven);
        sim_core.input.set_control_scheme(ControlScheme::Tank);

        tracing::info!("Simulation and Renderer initialized successfully!");
        tracing::info!("Controls: Gamepad Left Stick: Move/Turn (Tank) | Right Stick: PS2 Search Look | Y/Triangle: Toggle Flashlight");

        self.state = Some(AppState {
            window,
            surface,
            surface_config,
            render_ctx,
            blitter,
            sim_core,
            gpu_map,
            last_frame: Instant::now(),
        });
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
            WindowEvent::CloseRequested => {
                tracing::info!("Close requested, shutting down...");
                event_loop.exit();
            }
            WindowEvent::Resized(new_size) => {
                if new_size.width > 0 && new_size.height > 0 {
                    state.surface_config.width = new_size.width;
                    state.surface_config.height = new_size.height;
                    state
                        .surface
                        .configure(&state.render_ctx.device, &state.surface_config);
                }
            }
            WindowEvent::RedrawRequested => {
                let now = Instant::now();
                let dt = now.duration_since(state.last_frame);
                state.last_frame = now;

                // 1. Advance simulation at fixed 60 Hz tick
                state.sim_core.update(dt);

                // 2. Compute smooth sub-frame interpolated view matrix for high refresh OLED
                let alpha = state.sim_core.interpolation_alpha();
                let view = state.sim_core.camera.interpolated_view_matrix(alpha);
                let fov = state.sim_core.camera.interpolated_fov(alpha);
                let aspect =
                    state.surface_config.width as f32 / state.surface_config.height.max(1) as f32;
                let proj = glam::Mat4::perspective_rh(fov, aspect, 100.0, 50000.0);
                let view_proj = proj * view;
                let inv_proj = proj.inverse();

                let cam_pos = state.sim_core.camera.curr_snapshot.eye;
                state.render_ctx.update_camera(view_proj, inv_proj, cam_pos);

                // 3. Update flashlight spotlight tracking James
                let player_pos = state.sim_core.scenario.player_position;
                let player_yaw = state.sim_core.scenario.player_yaw;
                let light_dir = glam::Vec3::new(player_yaw.sin(), 0.1, player_yaw.cos());
                let spot_pos = player_pos + glam::Vec3::new(0.0, -1200.0, 0.0);

                let light_color = if state.sim_core.scenario.flashlight_on {
                    [1.2, 1.15, 0.95, 1.0]
                } else {
                    [0.0, 0.0, 0.0, 0.0]
                };

                let spot_uniforms = sh2_renderer::light::SpotlightUniforms {
                    position: [spot_pos.x, spot_pos.y, spot_pos.z, 1.0],
                    direction_inner: [light_dir.x, light_dir.y, light_dir.z, 0.95],
                    color_outer: [light_color[0], light_color[1], light_color[2], 0.85],
                };
                state.render_ctx.update_spotlight(&spot_uniforms);

                // 4. Render level geometry with volumetric fog and post-processing
                state.render_ctx.render_gpu_map(&state.gpu_map);

                // 5. Present to window surface swapchain
                let surface_texture = match state.surface.get_current_texture() {
                    Ok(frame) => frame,
                    Err(wgpu::SurfaceError::Outdated) => {
                        state
                            .surface
                            .configure(&state.render_ctx.device, &state.surface_config);
                        return;
                    }
                    Err(e) => {
                        tracing::error!("Surface error: {:?}", e);
                        return;
                    }
                };

                let surface_view = surface_texture
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());
                let mut encoder = state.render_ctx.device.create_command_encoder(
                    &wgpu::CommandEncoderDescriptor {
                        label: Some("Surface Present Encoder"),
                    },
                );

                state.blitter.render(
                    &state.render_ctx.device,
                    &mut encoder,
                    &state.render_ctx.ldr_output_view,
                    &surface_view,
                );

                state
                    .render_ctx
                    .queue
                    .submit(std::iter::once(encoder.finish()));
                surface_texture.present();

                state.window.request_redraw();
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(ref state) = self.state {
            state.window.request_redraw();
        }
    }
}

fn main() {
    tracing_subscriber::fmt::init();
    tracing::info!("Starting Silent Hill 2 Open Port Interactive Playable Test...");

    let event_loop = EventLoop::new().expect("Failed to create winit event loop");
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = SilentHillApp::default();
    event_loop.run_app(&mut app).expect("Event loop error");
}
