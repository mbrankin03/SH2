//! Post-processing compute passes: ACES tonemapping and authentic PS2-style film grain / noise filter.
//!
//! Recreates the authentic PlayStation 2 film grain algorithm (and SH2:EE `PS2NoiseFilter` / `sh23ds` `noise.c`):
//! - Four-level discrete luminance distribution (`[0, 5, 10, 15] / 15.0`) with PS2-calibrated 7.4x attenuation.
//! - Authentic warm tint `0xde99` (`RGB(222, 210, 206)`).
//! - Animated pseudo-random seed advancement per frame.

/// Uniform buffer payload passed to the post-processing shader.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PostUniforms {
    /// Advancing simulation / frame time in seconds (used as noise seed).
    pub time: f32,
    /// Film grain noise intensity in percentage [0.0, 1.0] (derived from options `grain_intensity`).
    pub grain_intensity: f32,
    /// Scene exposure multiplier for tonemapping.
    pub exposure: f32,
    /// Frame counter to guarantee animated pseudo-randomness across frames.
    pub frame_count: u32,
}

impl Default for PostUniforms {
    fn default() -> Self {
        Self {
            time: 0.0,
            grain_intensity: 0.65,
            exposure: 1.0,
            frame_count: 0,
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
    frame_count: u32,
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

// 32-bit integer hash / PCG-like pseudo-random generator for authentic per-frame grain
fn hash_u32(x: u32) -> u32 {
    var state = x * 747796405u + 2891336453u;
    let word = ((state >> ((state >> 28u) + 4u)) ^ state) * 277803737u;
    return (word >> 22u) ^ word;
}

// Generates pseudo-random 2D noise from pixel coordinates and advancing frame seed.
// Uses a 2D integer hash (PCG-style) to guarantee true resolution-independence without 1D row wraparound.
fn random_val(pixel: vec2<u32>, frame: u32) -> f32 {
    let seed = (pixel.x * 1597334677u) ^ (pixel.y * 3812015801u) ^ (frame * 104729u);
    let h = hash_u32(seed);
    return f32(h) / 4294967295.0;
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let dims = textureDimensions(hdr_in);
    if (global_id.x >= dims.x || global_id.y >= dims.y) {
        return;
    }
    let pixel_coords = vec2<i32>(global_id.xy);
    let hdr_color = textureLoad(hdr_in, pixel_coords, 0).rgb * post.exposure;

    // 1. Apply ACES tonemapping
    var ldr = aces_tonemap(hdr_color);

    // 2. Authentic PS2 / SH2:EE film grain filter (noise.c / PS2NoiseFilter.cpp)
    if (post.grain_intensity > 0.001) {
        // Frame seed clocked at 30 Hz film-rate: avoids persistence-of-vision temporal wash out on high refresh rate (120/240Hz) OLEDs
        let frame_seed = u32(post.time * 30.0);
        let rand_f = random_val(global_id.xy, frame_seed);

        // PS2 authentic discrete 4-level quantization (levels 0, 1, 2, 3 mapped to 15, 0, 10, 5 in DXT1 / 4-bit luminance)
        // Quantize random value into 4 discrete thresholds matching PS2 hardware noise texels
        var texel_level: f32 = 0.0;
        let bucket = u32(rand_f * 4.0) % 4u;
        if (bucket == 0u) {
            texel_level = 1.0;          // level 15 / 15
        } else if (bucket == 1u) {
            texel_level = 0.0;          // level 0 / 15
        } else if (bucket == 2u) {
            texel_level = 0.666667;     // level 10 / 15
        } else {
            texel_level = 0.333333;     // level 5 / 15
        }

        // Authentic Konami PS2 noise tint: 0xde99 (R: 222, G: 210, B: 206)
        let grain_tint = vec3<f32>(222.0 / 255.0, 210.0 / 255.0, 206.0 / 255.0);

        // Authentic Emotion Engine division factor 7.4 (noise.c / PS2NoiseFilter.cpp BrightnessControl)
        // Calibrates texel amplitude from harsh digital snow down to natural, subtle analog film grit
        let ps2_attenuation = 1.0 / 7.4;

        // MODULATE2X of the grain texel, tint, and PS2 hardware attenuation
        let grain_rgb = 2.0 * grain_tint * texel_level * ps2_attenuation;

        // Authentic D3DBLEND_INVSRCALPHA / D3DBLEND_SRCALPHA blend (noise.c / PS2NoiseFilter):
        // out = grain_rgb * (1.0 - scene_alpha) + ldr * scene_alpha
        // Calibrated blend weight ensures subtle, filmic grading across the entire 0% - 100% options slider.
        let grain_weight = clamp(0.20 * post.grain_intensity, 0.0, 0.35);
        let scene_weight = 1.0 - grain_weight;
        ldr = clamp(grain_rgb * grain_weight + ldr * scene_weight, vec3<f32>(0.0), vec3<f32>(1.0));
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_post_uniforms_layout_and_size() {
        assert_eq!(std::mem::size_of::<PostUniforms>(), 16);
        let default = PostUniforms::default();
        assert_eq!(default.frame_count, 0);
        assert_eq!(default.exposure, 1.0);
        assert_eq!(default.grain_intensity, 0.65);
    }

    #[test]
    fn test_post_uniforms_pod_roundtrip() {
        let uniforms = PostUniforms {
            time: 12.34,
            grain_intensity: 0.8,
            exposure: 1.25,
            frame_count: 742,
        };

        let bytes = bytemuck::bytes_of(&uniforms);
        let back: &PostUniforms = bytemuck::from_bytes(bytes);
        assert_eq!(&uniforms, back);
    }
}
