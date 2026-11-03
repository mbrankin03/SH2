//! Standalone direct pipeline test to eliminate any compute pass blackouts
use std::sync::Arc;
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

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
struct App {
    state: Option<AppState>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() { return; }

        let window = Arc::new(event_loop.create_window(
            Window::default_attributes()
                .with_title("SH2 Direct Geometry Render Test")
                .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 720.0))
        ).unwrap());

        let size = window.inner_size();
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN,
            ..Default::default()
        });

        let surface = unsafe {
            let target = wgpu::SurfaceTargetUnsafe::from_window(&*window).unwrap();
            instance.create_surface_unsafe(target).unwrap()
        };

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        })).unwrap();

        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default(), None)).unwrap();
        let device = Arc::new(device);
        let queue = Arc::new(queue);

        let caps = surface.get_capabilities(&adapter);
        let format = caps.formats[0];

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: caps.present_modes[0],
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &surface_config);

        let render_config = RendererConfig {
            width: size.width.max(1),
            height: size.height.max(1),
            enable_hdr: false,
            enable_fog: false,
            enable_post: false,
        };

        let render_ctx = RenderContext::from_device(instance, adapter, device.clone(), queue.clone(), render_config);
        let blitter = SurfaceBlitter::new(&device, format);

        // Generate simple bright colored cube directly in view
        use sh2_assets::formats::map::{BatchGroup, MapBatch, MapFile, MapMaterial};
        let mut map = MapFile::default();
        map.materials.push(MapMaterial {
            mode: 1,
            texture_id: 0,
            color: 0xFFFFFFFF,
            overlay: 0,
            specularity: 0.5,
        });

        let s = 1000.0;
        let positions = vec![
            [-s, -s, s], [s, -s, s], [s, s, s], [-s, s, s], // Front
            [-s, -s, -s], [-s, s, -s], [s, s, -s], [s, -s, -s], // Back
        ];
        let normals = vec![[0.0, 0.0, 1.0]; 8];
        let uvs = vec![[0.0, 0.0]; 8];
        let colors = vec![
            [255, 50, 50, 255], [50, 255, 50, 255], [50, 50, 255, 255], [255, 255, 50, 255],
            [255, 50, 255, 255], [50, 255, 255, 255], [255, 128, 50, 255], [128, 50, 255, 255],
        ];
        let indices = vec![
            0, 1, 2, 2, 3, 0, // Front
            1, 7, 6, 6, 2, 1, // Right
            7, 4, 5, 5, 6, 7, // Back
            4, 0, 3, 3, 5, 4, // Left
            3, 2, 6, 6, 5, 3, // Top
            4, 7, 1, 1, 0, 4, // Bottom
        ];

        map.batches.push(MapBatch {
            group: BatchGroup::Opaque,
            material_index: 0,
            geo_id: 1,
            positions,
            normals,
            uvs,
            colors,
            indices,
        });

        let gpu_map = GpuMap::from_map(&device, &map);
        let mut sim_core = SimulationCore::new(ScenarioKind::LetterFromSilentHeaven);
        sim_core.scenario.player_position = glam::Vec3::new(0.0, 0.0, -3000.0);
        sim_core.camera.reset_to_player(sim_core.scenario.player_position, 0.0);

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

    fn window_event(&mut self, el: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(ref mut state) = self.state else { return };
        match event {
            WindowEvent::CloseRequested => el.exit(),
            WindowEvent::RedrawRequested => {
                let now = Instant::now();
                let dt = now.duration_since(state.last_frame);
                state.last_frame = now;

                state.sim_core.update(dt);
                let alpha = state.sim_core.interpolation_alpha();
                let view = state.sim_core.camera.interpolated_view_matrix(alpha);
                let fov = state.sim_core.camera.interpolated_fov(alpha);
                let aspect = state.surface_config.width as f32 / state.surface_config.height.max(1) as f32;
                let proj = glam::Mat4::perspective_rh(fov, aspect, 10.0, 30000.0);
                let view_proj = proj * view;
                let inv_proj = proj.inverse();

                state.render_ctx.update_camera(view_proj, inv_proj, state.sim_core.camera.curr_snapshot.eye);

                let spot = sh2_renderer::light::SpotlightUniforms {
                    position: [0.0, -500.0, -2000.0, 1.0],
                    direction_inner: [0.0, 0.0, 1.0, 0.8],
                    color_outer: [2.0, 2.0, 2.0, 0.5],
                };
                state.render_ctx.update_spotlight(&spot);

                // Render into HDR color view directly
                let frame = match state.surface.get_current_texture() {
                    Ok(f) => f,
                    Err(_) => return,
                };
                let surface_view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());

                let mut encoder = state.render_ctx.device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
                {
                    let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("Direct Render Pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &surface_view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.1, g: 0.1, b: 0.15, a: 1.0 }),
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                            view: &state.render_ctx.depth_view,
                            depth_ops: Some(wgpu::Operations {
                                load: wgpu::LoadOp::Clear(1.0),
                                store: wgpu::StoreOp::Store,
                            }),
                            stencil_ops: None,
                        }),
                        timestamp_writes: None,
                        occlusion_query_set: None,
                    });

                    rpass.set_pipeline(&state.render_ctx.geometry_pipeline.pipeline);
                    rpass.set_bind_group(0, &state.render_ctx.geometry_bind_group, &[]);

                    for batch in &state.gpu_map.batches {
                        rpass.set_vertex_buffer(0, batch.vertex_buffer.slice(..));
                        rpass.set_index_buffer(batch.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                        rpass.draw_indexed(0..batch.num_indices, 0, 0..1);
                    }
                }

                state.render_ctx.queue.submit(std::iter::once(encoder.finish()));
                frame.present();
                state.window.request_redraw();
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _el: &ActiveEventLoop) {
        if let Some(ref state) = self.state {
            state.window.request_redraw();
        }
    }
}

fn main() {
    let el = EventLoop::new().unwrap();
    el.set_control_flow(ControlFlow::Poll);
    let mut app = App::default();
    el.run_app(&mut app).unwrap();
}
