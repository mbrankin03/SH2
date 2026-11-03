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
            .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 720.0))
            .with_visible(true);

        let window = Arc::new(event_loop.create_window(window_attributes).unwrap());
        let size = window.inner_size();

        // Use Vulkan instance explicitly for desktop Linux Wayland
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN,
            ..Default::default()
        });

        // Create surface safely bound to the window
        let surface = unsafe {
            let target = wgpu::SurfaceTargetUnsafe::from_window(&*window).unwrap();
            instance.create_surface_unsafe(target).unwrap()
        };

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .expect("Failed to find suitable Vulkan GPU adapter compatible with window surface");

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("SH2 Render Device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: Default::default(),
            },
            None,
        ))
        .expect("Failed to acquire Vulkan logical device");

        let device = Arc::new(device);
        let queue = Arc::new(queue);

        let surface_caps = surface.get_capabilities(&adapter);
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
        surface.configure(&device, &surface_config);

        let render_config = RendererConfig {
            width: size.width.max(1),
            height: size.height.max(1),
            enable_hdr: true,
            enable_fog: true,
            enable_post: true,
        };

        let render_ctx = RenderContext::from_device(
            instance,
            adapter,
            device.clone(),
            queue.clone(),
            render_config,
        );

        // Load authentic Silent Hill 2 level geometry map
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

        let gpu_map = GpuMap::from_map(&device, &map_file);
        let blitter = SurfaceBlitter::new(&device, surface_format);

        // Spawn player and camera directly inside rr21.map center:
        // rr21 bounds: X: [-101550, -98482], Y: [-1915, 102], Z: [-61211, -58796]
        let mut sim_core = SimulationCore::new(ScenarioKind::LetterFromSilentHeaven);
        sim_core.input.set_control_scheme(ControlScheme::Tank);
        sim_core.scenario.player_position = glam::Vec3::new(-100000.0, -500.0, -60000.0);
        sim_core.scenario.player_yaw = 0.0;
        sim_core.scenario.flashlight_on = true;
        sim_core.camera.reset_to_player(
            sim_core.scenario.player_position,
            sim_core.scenario.player_yaw,
        );

        // Atmospheric Silent Hill lighting & fog calibrated for rr21 bounds
        let fog_uniforms = sh2_renderer::fog::FogUniforms {
            fog_color: [0.08, 0.09, 0.11, 1.0],
            fog_near: 1500.0,
            fog_far: 8000.0,
            density: 0.65,
            _padding: 0.0,
        };
        render_ctx.update_fog(&fog_uniforms);

        tracing::info!("Simulation and Renderer initialized successfully on Vulkan!");
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
                let proj = glam::Mat4::perspective_rh(fov, aspect, 10.0, 30000.0);
                let view_proj = proj * view;
                let inv_proj = proj.inverse();

                let cam_pos = state.sim_core.camera.curr_snapshot.eye;
                state.render_ctx.update_camera(view_proj, inv_proj, cam_pos);

                // 3. Dynamic Flashlight tracking James
                let player_pos = state.sim_core.scenario.player_position;
                let player_yaw = state.sim_core.scenario.player_yaw;
                let light_dir =
                    glam::Vec3::new(player_yaw.sin(), 0.05, player_yaw.cos()).normalize_or_zero();
                let spot_pos = player_pos + glam::Vec3::new(0.0, -800.0, 0.0);

                let light_color = if state.sim_core.scenario.flashlight_on {
                    [1.8, 1.7, 1.4, 1.0]
                } else {
                    [0.0, 0.0, 0.0, 0.0]
                };

                let spot_uniforms = sh2_renderer::light::SpotlightUniforms {
                    position: [spot_pos.x, spot_pos.y, spot_pos.z, 1.0],
                    direction_inner: [light_dir.x, light_dir.y, light_dir.z, 0.92], // ~23 deg inner
                    color_outer: [light_color[0], light_color[1], light_color[2], 0.78], // ~38 deg outer
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
