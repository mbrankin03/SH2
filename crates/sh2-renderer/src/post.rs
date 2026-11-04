//! Post-processing compute passes: ACES tonemapping and film grain.

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PostUniforms {
    pub time: f32,
    pub grain_intensity: f32,
    pub exposure: f32,
    pub _padding: f32,
}

impl Default for PostUniforms {
    fn default() -> Self {
        Self {
            time: 0.0,
            grain_intensity: 0.05,
            exposure: 1.0,
            _padding: 0.0,
        }
    }
}

pub struct PostProcessPipeline {
    pub pipeline: wgpu::ComputePipeline,
    pub bind_group_layout: wgpu::BindGroupLayout,
}

impl PostProcessPipeline {
    pub const WGSL_SHADER: &'static str = r#"
struct PostUniforms {
    time: f32,
    grain_intensity: f32,
    exposure: f32,
    _padding: f32,
};

@group(0) @binding(0) var<uniform> post: PostUniforms;
@group(0) @binding(1) var hdr_in: texture_2d<f32>;
@group(0) @binding(2) var ldr_out: texture_storage_2d<rgba8unorm, write>;

// ACES filmic tonemapping approximation
fn aces_tonemap(x: vec3<f32>) -> vec3<f32> {
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    return clamp((x * (a * x + b)) / (x * (c * x + d) + e), vec3<f32>(0.0), vec3<f32>(1.0));
}

// Pseudo-random noise for film grain
fn hash(p: vec2<f32>) -> f32 {
    let p3 = fract(vec3<f32>(p.xyx) * 0.1031);
    let dot_prod = dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * dot_prod);
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let dims = textureDimensions(hdr_in);
    if (global_id.x >= dims.x || global_id.y >= dims.y) {
        return;
    }
    let pixel_coords = vec2<i32>(global_id.xy);
    let hdr_color = textureLoad(hdr_in, pixel_coords, 0).rgb * post.exposure;

    // Apply ACES tonemapping
    var ldr = aces_tonemap(hdr_color);

    // Apply Silent Hill authentic film grain
    let uv = vec2<f32>(pixel_coords) + vec2<f32>(post.time * 123.45, post.time * 678.90);
    let noise = (hash(uv) - 0.5) * post.grain_intensity;
    ldr = clamp(ldr + vec3<f32>(noise), vec3<f32>(0.0), vec3<f32>(1.0));

    textureStore(ldr_out, pixel_coords, vec4<f32>(ldr, 1.0));
}
"#;

    pub fn new(device: &wgpu::Device) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Post Process Compute Shader"),
            source: wgpu::ShaderSource::Wgsl(Self::WGSL_SHADER.into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Post Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rgba8Unorm,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Post Process Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Post Process Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });

        Self {
            pipeline,
            bind_group_layout,
        }
    }
}
