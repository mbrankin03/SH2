//! Volumetric compute fog parameters matching PS2 ground-truth registers (`dbg_fogparm_ctl`, `dbg_ambifog_ctl`).

/// Uniform buffer for volumetric fog shader.
///
/// Layout is aligned to 16 bytes for WGSL uniform buffer compliance (total size: 32 bytes).
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct FogUniforms {
    /// Fog ambient color (RGBA). (Offset 0, size 16)
    pub fog_color: [f32; 4],
    /// Near distance cutoff where fog starts taking effect. (Offset 16, size 4)
    pub fog_near: f32,
    /// Far distance cutoff where fog density reaches maximum. (Offset 20, size 4)
    pub fog_far: f32,
    /// Fog density multiplier. (Offset 24, size 4)
    pub density: f32,
    /// Explicit padding to satisfy 16-byte WGSL alignment. (Offset 28, size 4)
    pub _padding: f32,
}

impl Default for FogUniforms {
    fn default() -> Self {
        Self {
            fog_color: [0.7, 0.7, 0.75, 1.0],
            fog_near: 2.0,
            fog_far: 25.0,
            density: 1.0,
            _padding: 0.0,
        }
    }
}

/// Compute pipeline for raymarched volumetric fog pass.
pub struct FogComputePipeline {
    pub pipeline: wgpu::ComputePipeline,
    pub bind_group_layout: wgpu::BindGroupLayout,
}

impl FogComputePipeline {
    pub const WGSL_SHADER: &'static str = r#"
struct FogUniforms {
    fog_color: vec4<f32>,
    fog_near: f32,
    fog_far: f32,
    density: f32,
    _padding: f32,
};

struct CameraUniforms {
    view_proj: mat4x4<f32>,
    inv_proj: mat4x4<f32>,
    cam_pos: vec4<f32>,
};

@group(0) @binding(0) var<uniform> fog: FogUniforms;
@group(0) @binding(1) var<uniform> camera: CameraUniforms;
@group(0) @binding(2) var depth_tex: texture_2d<f32>;
@group(0) @binding(3) var color_in: texture_2d<f32>;
@group(0) @binding(4) var color_out: texture_storage_2d<rgba16float, write>;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let dims = textureDimensions(color_in);
    if (global_id.x >= dims.x || global_id.y >= dims.y) {
        return;
    }
    let pixel_coords = vec2<i32>(global_id.xy);
    let original_color = textureLoad(color_in, pixel_coords, 0);
    let depth_val = textureLoad(depth_tex, pixel_coords, 0).r;

    // Linearize view distance using camera inverse projection matrix
    let uv = (vec2<f32>(pixel_coords) + 0.5) / vec2<f32>(dims);
    let clip_pos = vec4<f32>(uv.x * 2.0 - 1.0, (1.0 - uv.y) * 2.0 - 1.0, depth_val, 1.0);
    let view_pos_h = camera.inv_proj * clip_pos;
    let view_dist = length(view_pos_h.xyz / max(1e-5, view_pos_h.w));

    let fog_factor = clamp((view_dist - fog.fog_near) / max(1e-4, fog.fog_far - fog.fog_near), 0.0, 1.0) * fog.density;

    let final_color = mix(original_color, fog.fog_color, fog_factor);
    textureStore(color_out, pixel_coords, final_color);
}
"#;

    pub fn new(device: &wgpu::Device) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Volumetric Fog Compute Shader"),
            source: wgpu::ShaderSource::Wgsl(Self::WGSL_SHADER.into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Fog Bind Group Layout"),
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
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rgba16Float,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Fog Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Fog Compute Pipeline"),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fog_uniforms_layout_and_size() {
        assert_eq!(std::mem::size_of::<FogUniforms>(), 32);
        assert_eq!(std::mem::align_of::<FogUniforms>(), 4);
    }
}
