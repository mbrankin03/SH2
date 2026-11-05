//! Hardware-accelerated YUV420p (`I420`) FMV video presentation pipeline (`SH2-10`).
//!
//! Streams planar `Y` (`R8Unorm` full resolution) and `U`/`V` (`R8Unorm` half resolution)
//! planes from a [`Yuv420Frame`](sh2_assets::formats::bink::Yuv420Frame) into GPU textures,
//! converts BT.601 studio-swing (`16..235` luma, `16..240` chroma) to RGB in a WGSL fragment shader,
//! applies widescreen/letterbox/pillarbox aspect scaling, and layers configurable film grain noise
//! underneath subtitle overlays (`Patches/FmvSubtitles.cpp`).

use bytemuck::{Pod, Zeroable};
use sh2_assets::formats::bink::Yuv420Frame;
use wgpu::util::DeviceExt;

/// Uniform parameters controlling FMV YUV420p presentation, aspect scaling, cropping, and film grain.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct FmvUniforms {
    /// Target viewport aspect ratio (`viewport_width / viewport_height`, e.g. `16.0 / 9.0`).
    pub viewport_aspect: f32,
    /// Video active frame aspect ratio (`16.0 / 9.0`, `4.0 / 3.0`, or raw `width / height`).
    pub video_aspect: f32,
    /// Normalized vertical UV start offset for cropping `1920x1230` SH2:EE frames (`75.0 / 1230.0` or `0.0`).
    pub uv_y_offset: f32,
    /// Normalized vertical UV scale for cropping `1920x1230` SH2:EE frames (`1080.0 / 1230.0` or `1.0`).
    pub uv_y_scale: f32,
    /// Procedural film grain intensity (`0.0` = clean, `0.25` = subtle cinema grain).
    pub grain_intensity: f32,
    /// Elapsed playback time in seconds (animates film grain noise).
    pub time: f32,
    /// Overall brightness / fade multiplier (`0.0` = black fade, `1.0` = full brightness).
    pub fade_alpha: f32,
    /// Padding to 32-byte alignment.
    pub _pad: f32,
}

impl Default for FmvUniforms {
    fn default() -> Self {
        Self {
            viewport_aspect: 16.0 / 9.0,
            video_aspect: 16.0 / 9.0,
            uv_y_offset: 0.0,
            uv_y_scale: 1.0,
            grain_intensity: 0.18,
            time: 0.0,
            fade_alpha: 1.0,
            _pad: 0.0,
        }
    }
}

/// GPU YUV420p video texture set (`Y`, `U`, `V` single-channel `R8Unorm` planes) and bind group.
pub struct GpuYuvVideoSurface {
    pub width: u32,
    pub height: u32,
    pub y_texture: wgpu::Texture,
    pub u_texture: wgpu::Texture,
    pub v_texture: wgpu::Texture,
    pub bind_group: wgpu::BindGroup,
}

/// Fullscreen WGSL render pipeline for planar YUV420p (`I420`) FMV cutscene playback.
pub struct FmvVideoPipeline {
    pub pipeline: wgpu::RenderPipeline,
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub sampler: wgpu::Sampler,
    pub uniform_buffer: wgpu::Buffer,
}

impl FmvVideoPipeline {
    pub const WGSL_SHADER: &'static str = r#"
struct FmvUniforms {
    viewport_aspect: f32,
    video_aspect: f32,
    uv_y_offset: f32,
    uv_y_scale: f32,
    grain_intensity: f32,
    time: f32,
    fade_alpha: f32,
    _pad: f32,
};

@group(0) @binding(0) var y_tex: texture_2d<f32>;
@group(0) @binding(1) var u_tex: texture_2d<f32>;
@group(0) @binding(2) var v_tex: texture_2d<f32>;
@group(0) @binding(3) var yuv_sampler: sampler;
@group(0) @binding(4) var<uniform> params: FmvUniforms;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) screen_uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) in_vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    let x = f32((in_vertex_index << 1u) & 2u);
    let y = f32(in_vertex_index & 2u);
    out.position = vec4<f32>(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
    out.screen_uv = vec2<f32>(x, y);
    return out;
}

fn hash12(p: vec2<f32>) -> f32 {
    var p3 = fract(vec3<f32>(p.xyx) * 0.1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Letterbox / pillarbox aspect ratio mapping
    let vp_aspect = max(params.viewport_aspect, 0.01);
    let vid_aspect = max(params.video_aspect, 0.01);

    var mapped_uv = in.screen_uv;
    if (vp_aspect > vid_aspect) {
        // Viewport is wider than video -> pillarbox left & right
        let scale_x = vp_aspect / vid_aspect;
        mapped_uv.x = (in.screen_uv.x - 0.5) * scale_x + 0.5;
    } else if (vp_aspect < vid_aspect) {
        // Viewport is taller than video -> letterbox top & bottom
        let scale_y = vid_aspect / vp_aspect;
        mapped_uv.y = (in.screen_uv.y - 0.5) * scale_y + 0.5;
    }

    // Black bars outside active video region
    if (mapped_uv.x < 0.0 || mapped_uv.x > 1.0 || mapped_uv.y < 0.0 || mapped_uv.y > 1.0) {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }

    // Apply vertical crop window (e.g. 1920x1230 -> center 1920x1080)
    let sample_uv = vec2<f32>(
        clamp(mapped_uv.x, 0.0, 1.0),
        clamp(params.uv_y_offset + mapped_uv.y * params.uv_y_scale, 0.0, 1.0)
    );

    // Sample planar YUV420p (BT.601 studio-swing tv range)
    let y_raw = textureSample(y_tex, yuv_sampler, sample_uv).r;
    let u_raw = textureSample(u_tex, yuv_sampler, sample_uv).r;
    let v_raw = textureSample(v_tex, yuv_sampler, sample_uv).r;

    let y_lin = max(y_raw - (16.0 / 255.0), 0.0) * (255.0 / 219.0);
    let u_off = u_raw - (128.0 / 255.0);
    let v_off = v_raw - (128.0 / 255.0);

    var rgb = vec3<f32>(
        y_lin + 1.402 * v_off,
        y_lin - 0.344136 * u_off - 0.714136 * v_off,
        y_lin + 1.772 * u_off
    );
    rgb = clamp(rgb, vec3<f32>(0.0), vec3<f32>(1.0));

    // Apply subtle film grain noise over the active video area (below subtitle pass)
    if (params.grain_intensity > 0.001) {
        let seed = in.position.xy + vec2<f32>(params.time * 137.0, params.time * 293.0);
        let noise = (hash12(seed) - 0.5) * 0.14 * params.grain_intensity;
        rgb = clamp(rgb + vec3<f32>(noise), vec3<f32>(0.0), vec3<f32>(1.0));
    }

    let faded = rgb * clamp(params.fade_alpha, 0.0, 1.0);
    return vec4<f32>(faded, 1.0);
}
"#;

    /// Creates a new `FmvVideoPipeline` targeting the specified surface or render target format.
    pub fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("FMV YUV420p Presentation Shader"),
            source: wgpu::ShaderSource::Wgsl(Self::WGSL_SHADER.into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("FMV YUV420p Bind Group Layout"),
            entries: &[
                // Y plane
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // U plane
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // V plane
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // Bilinear ClampToEdge sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // FmvUniforms buffer
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("FMV YUV420p Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("FMV YUV420p Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("FMV YUV420p Linear Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let default_uniforms = FmvUniforms::default();
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("FMV Uniform Buffer"),
            contents: bytemuck::bytes_of(&default_uniforms),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        Self {
            pipeline,
            bind_group_layout,
            sampler,
            uniform_buffer,
        }
    }

    /// Allocates GPU `R8Unorm` planar textures (`Y`, `U`, `V`) sized for `(width, height)`.
    pub fn create_video_surface(
        &self,
        device: &wgpu::Device,
        width: u32,
        height: u32,
    ) -> GpuYuvVideoSurface {
        let w = width.max(2);
        let h = height.max(2);
        let cw = w.div_ceil(2);
        let ch = h.div_ceil(2);

        let create_plane = |label: &str, pw: u32, ph: u32| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width: pw,
                    height: ph,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            })
        };

        let y_texture = create_plane("FMV Y Plane Texture", w, h);
        let u_texture = create_plane("FMV U Plane Texture", cw, ch);
        let v_texture = create_plane("FMV V Plane Texture", cw, ch);

        let y_view = y_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let u_view = u_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let v_view = v_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("FMV YUV420p Surface Bind Group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&y_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&u_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&v_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: self.uniform_buffer.as_entire_binding(),
                },
            ],
        });

        GpuYuvVideoSurface {
            width: w,
            height: h,
            y_texture,
            u_texture,
            v_texture,
            bind_group,
        }
    }

    /// Uploads a decoded [`Yuv420Frame`] into the `GpuYuvVideoSurface` textures.
    pub fn upload_frame(
        &self,
        queue: &wgpu::Queue,
        surface: &GpuYuvVideoSurface,
        frame: &Yuv420Frame,
    ) {
        let w = surface.width;
        let h = surface.height;
        let cw = w.div_ceil(2);
        let ch = h.div_ceil(2);

        if frame.y_plane.len() >= (w * h) as usize {
            queue.write_texture(
                wgpu::ImageCopyTexture {
                    texture: &surface.y_texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &frame.y_plane[..(w * h) as usize],
                wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(w),
                    rows_per_image: Some(h),
                },
                wgpu::Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: 1,
                },
            );
        }

        if frame.u_plane.len() >= (cw * ch) as usize {
            queue.write_texture(
                wgpu::ImageCopyTexture {
                    texture: &surface.u_texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &frame.u_plane[..(cw * ch) as usize],
                wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(cw),
                    rows_per_image: Some(ch),
                },
                wgpu::Extent3d {
                    width: cw,
                    height: ch,
                    depth_or_array_layers: 1,
                },
            );
        }

        if frame.v_plane.len() >= (cw * ch) as usize {
            queue.write_texture(
                wgpu::ImageCopyTexture {
                    texture: &surface.v_texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &frame.v_plane[..(cw * ch) as usize],
                wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(cw),
                    rows_per_image: Some(ch),
                },
                wgpu::Extent3d {
                    width: cw,
                    height: ch,
                    depth_or_array_layers: 1,
                },
            );
        }
    }

    /// Updates the uniform buffer with current aspect scaling, vertical crop, and grain settings.
    pub fn update_uniforms(&self, queue: &wgpu::Queue, uniforms: &FmvUniforms) {
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(uniforms));
    }

    /// Renders the YUV420p video frame into `dst_view`.
    pub fn render(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        surface: &GpuYuvVideoSurface,
        dst_view: &wgpu::TextureView,
    ) {
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("FMV YUV420p Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: dst_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        rpass.set_pipeline(&self.pipeline);
        rpass.set_bind_group(0, &surface.bind_group, &[]);
        rpass.draw(0..3, 0..1);
    }
}
