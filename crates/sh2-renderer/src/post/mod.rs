//! Post-processing compute passes: optional ACES tonemapping (`native-upgrade`), authentic PS2
//! 8-level brightness LUT (`0x001757b0`), 128×128 LCG 4×4 tiled noise (`0x00177c50`/`0x00177f30`),
//! `Swap_GlowSoft` bloom (`0x00178d60`), `Swap_Soft` N×N DoF (`0x001785e0`), `Filter_Retain`
//! feedback/pause freeze (`0x00179b10`), and Black/White/Red screen fades (`0x00179d60`/`0x0017a2e0`).

pub mod clear_color;
pub mod ps2_filters;

pub use clear_color::{
    pack_gs_clearcolor_rgbaq, ps2_clear_rgba_to_wgpu, resolve_ps2_room_clear_rgba,
    PS2_DEFAULT_DAY_CLEAR_RGBA, PS2_DEFAULT_NIGHT_CLEAR_RGBA,
};

pub use ps2_filters::{
    compute_swap_tap_offsets_12_4, ps2_brightness_byte, ps2_brightness_scale,
    sample_ps2_noise_tiled_4x4, FilterRetainState, Ps2FadeKind, Ps2NoiseGenerator,
    SwapGlowSoftParams, SwapSoftParams, SwapSoftTestMode, GS_FILTER_RETAIN_ALPHA_1,
    PS2_BRIGHTNESS_LUT_U8, PS2_BRIGHTNESS_NEUTRAL_U8, PS2_FILTER_CMD_PAUSE_RETAIN,
    PS2_NOISE_LCG_INC, PS2_NOISE_LCG_MUL, PS2_NOISE_TEX_SIZE, PS2_NOISE_TILE_GRID,
};

/// Bitflag in [`PostUniforms::flags`]: enables modern ACES filmic tonemapping (`native-upgrade`).
/// When cleared (strict 1:1 Vanilla PS2 mode), ACES is bypassed and HDR values clamp directly
/// after PS2 brightness LUT scaling (`sh2gfw_Set_Brightness` `0x001757b0`).
pub const POST_FLAG_ACES_TONEMAP: u32 = 1 << 0;

/// Bitflag in [`PostUniforms::flags`]: uses the 128×128 LCG 4×4 tiled PS2 noise pattern
/// (`sh2gfw_test_MakeNoise` `0x00177c50`, `sh2gfw_SendDraw_Noise` `0x00177f30`).
pub const POST_FLAG_PS2_LCG_NOISE: u32 = 1 << 1;

/// Uniform buffer payload passed to the post-processing compute shader (`SH2-105`, `SH2-136`).
///
/// Layout is 64 bytes (`4 * vec4<f32>`, 16-byte aligned for WGSL uniform buffers).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PostUniforms {
    /// Advancing simulation / frame time in seconds (used as noise seed).
    pub time: f32,
    /// Film grain noise intensity in percentage `[0.0, 1.0]` (derived from options `grain_intensity`).
    pub grain_intensity: f32,
    /// Scene exposure / PS2 brightness LUT multiplier (`ps2_brightness_scale(level)`).
    pub exposure: f32,
    /// Frame counter to guarantee animated pseudo-randomness across frames.
    pub frame_count: u32,
    /// PS2 `Kari_hisyakai` / `Swap_Soft` screen-space DoF tap offset in pixels `(dx, dy)`.
    pub dof_tap_offset_px: [f32; 2],
    /// Normalized PS2 DoF / `Swap_Soft` blend weight `[0.0, 1.0]`.
    pub dof_strength: f32,
    /// Normalized PS2 `enSetBlur` motion blur strength `[0.0, 1.0]`.
    pub motion_blur_strength: f32,
    /// Feature bitflags ([`POST_FLAG_ACES_TONEMAP`], [`POST_FLAG_PS2_LCG_NOISE`]).
    pub flags: u32,
    /// PS2 `Swap_GlowSoft` (`0x00178d60`) normalized luminance threshold (`aref / 255.0`).
    pub glow_threshold: f32,
    /// PS2 `Swap_GlowSoft` (`0x00178d60`) normalized additive bloom strength (`blend_alpha / 128.0`).
    pub glow_strength: f32,
    /// PS2 `Swap_Soft` / `Swap_GlowSoft` kernel tap radius in pixels (`step_param * 0.25`).
    pub glow_step_px: f32,
    /// PS2 Black/White/Red fade overlay (`[r, g, b, alpha]` from [`Ps2FadeKind::evaluate_rgba`]).
    pub fade_rgba: [f32; 4],
}

impl Default for PostUniforms {
    fn default() -> Self {
        Self {
            time: 0.0,
            grain_intensity: 0.65,
            exposure: 1.0,
            frame_count: 0,
            dof_tap_offset_px: [1.0 / 3.0, 2.0 / 3.0],
            dof_strength: 0.0,
            motion_blur_strength: 0.0,
            flags: POST_FLAG_PS2_LCG_NOISE,
            glow_threshold: 0.0,
            glow_strength: 0.0,
            glow_step_px: 0.0,
            fade_rgba: [0.0, 0.0, 0.0, 0.0],
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
    dof_tap_offset_px: vec2<f32>,
    dof_strength: f32,
    motion_blur_strength: f32,
    flags: u32,
    glow_threshold: f32,
    glow_strength: f32,
    glow_step_px: f32,
    fade_rgba: vec4<f32>,
};

const POST_FLAG_ACES_TONEMAP: u32 = 1u;
const POST_FLAG_PS2_LCG_NOISE: u32 = 2u;

@group(0) @binding(0) var<uniform> post: PostUniforms;
@group(0) @binding(1) var hdr_in: texture_2d<f32>;
@group(0) @binding(2) var ldr_out: texture_storage_2d<rgba8unorm, write>;

// Optional ACES filmic tonemapping approximation (native-upgrade, disabled in strict Vanilla PS2 mode)
fn aces_tonemap(x: vec3<f32>) -> vec3<f32> {
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    return clamp((x * (a * x + b)) / (x * (c * x + d) + e), vec3<f32>(0.0), vec3<f32>(1.0));
}

// PS2 `sh2gfw_test_MakeNoise` (0x00177c50) LCG (`state * 0x19660d + 0x3c6ef35f`)
// evaluated on 128x128 texel coordinates tiled 4x4 across the screen (`0x00177f30`).
fn ps2_lcg_noise_4x4(pixel: vec2<u32>, dims: vec2<u32>, frame_seed: u32) -> f32 {
    let u = fract((f32(pixel.x) / max(f32(dims.x), 1.0)) * 4.0);
    let v = fract((f32(pixel.y) / max(f32(dims.y), 1.0)) * 4.0);
    let tx = min(u32(u * 128.0), 127u);
    let ty = min(u32(v * 128.0), 127u);
    let idx = ty * 128u + tx;
    var state = (idx ^ (frame_seed * 1664525u)) * 1664525u + 1013904223u;
    state = state * 1664525u + 1013904223u;
    return f32((state >> 24u) & 255u) / 255.0;
}

// Resolution-independent PCG hash fallback when PS2 tiled noise flag is off
fn hash_u32(x: u32) -> u32 {
    var state = x * 747796405u + 2891336453u;
    let word = ((state >> ((state >> 28u) + 4u)) ^ state) * 277803737u;
    return (word >> 22u) ^ word;
}

fn random_val(pixel: vec2<u32>, frame: u32) -> f32 {
    let seed = (pixel.x * 1597334677u) ^ (pixel.y * 3812015801u) ^ (frame * 104729u);
    return f32(hash_u32(seed)) / 4294967295.0;
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let dims = textureDimensions(hdr_in);
    if (global_id.x >= dims.x || global_id.y >= dims.y) {
        return;
    }
    let pixel_coords = vec2<i32>(global_id.xy);
    let max_coord = vec2<i32>(dims) - vec2<i32>(1, 1);
    var hdr_color = textureLoad(hdr_in, pixel_coords, 0).rgb * post.exposure;

    // 0a. PS2 `Swap_Soft` / `Kari_hisyakai` Screen-Space DoF (`0x001785e0`)
    if (post.dof_strength > 0.001) {
        let tap_delta = vec2<i32>(round(post.dof_tap_offset_px));
        let tap_double = vec2<i32>(round(post.dof_tap_offset_px * 2.0));
        let c1 = textureLoad(hdr_in, clamp(pixel_coords + tap_delta, vec2<i32>(0), max_coord), 0).rgb * post.exposure;
        let c2 = textureLoad(hdr_in, clamp(pixel_coords - tap_delta, vec2<i32>(0), max_coord), 0).rgb * post.exposure;
        let c3 = textureLoad(hdr_in, clamp(pixel_coords + tap_double, vec2<i32>(0), max_coord), 0).rgb * post.exposure;
        let c4 = textureLoad(hdr_in, clamp(pixel_coords - tap_double, vec2<i32>(0), max_coord), 0).rgb * post.exposure;
        let dof_avg = (c1 + c2) * 0.30 + (c3 + c4) * 0.20;
        let w_dof = clamp(post.dof_strength, 0.0, 0.90);
        hdr_color = mix(hdr_color, dof_avg, w_dof);
    }

    // 0b. PS2 `Swap_GlowSoft` additive bloom (`0x00178d60`)
    if (post.glow_strength > 0.001) {
        let step_px = max(i32(round(post.glow_step_px)), 1);
        var glow_acc = vec3<f32>(0.0);
        for (var dy: i32 = -1; dy <= 1; dy = dy + 1) {
            for (var dx: i32 = -1; dx <= 1; dx = dx + 1) {
                let sc = clamp(pixel_coords + vec2<i32>(dx * step_px, dy * step_px), vec2<i32>(0), max_coord);
                let s = textureLoad(hdr_in, sc, 0).rgb * post.exposure;
                let luma = max(s.r, max(s.g, s.b));
                if (luma >= post.glow_threshold) {
                    glow_acc = glow_acc + s;
                }
            }
        }
        hdr_color = hdr_color + (glow_acc * (1.0 / 9.0)) * post.glow_strength;
    }

    // 0c. PS2 `enSetBlur` Motion Blur Pass
    if (post.motion_blur_strength > 0.001) {
        let center = vec2<f32>(dims) * 0.5;
        let radial = (vec2<f32>(global_id.xy) - center) * (0.015 * post.motion_blur_strength);
        let step1 = vec2<i32>(round(radial));
        let step2 = vec2<i32>(round(radial * 2.0));
        let m1 = textureLoad(hdr_in, clamp(pixel_coords - step1, vec2<i32>(0), max_coord), 0).rgb * post.exposure;
        let m2 = textureLoad(hdr_in, clamp(pixel_coords - step2, vec2<i32>(0), max_coord), 0).rgb * post.exposure;
        let blur_rgb = (hdr_color + m1 + m2) * (1.0 / 3.0);
        let w_mb = clamp(post.motion_blur_strength, 0.0, 0.85);
        hdr_color = mix(hdr_color, blur_rgb, w_mb);
    }

    // 1. Optional ACES tonemapping (`native-upgrade`) vs strict 1:1 Vanilla PS2 linear clamp
    var ldr: vec3<f32>;
    if ((post.flags & POST_FLAG_ACES_TONEMAP) != 0u) {
        ldr = aces_tonemap(hdr_color);
    } else {
        ldr = clamp(hdr_color, vec3<f32>(0.0), vec3<f32>(1.0));
    }

    // 2. PS2 `sh2gfw_SendDraw_Noise` (0x00177f30) 128x128 4x4 tiled LCG film grain
    if (post.grain_intensity > 0.001) {
        let frame_seed = u32(post.time * 30.0);
        var rand_f: f32;
        if ((post.flags & POST_FLAG_PS2_LCG_NOISE) != 0u) {
            rand_f = ps2_lcg_noise_4x4(global_id.xy, dims, frame_seed);
        } else {
            rand_f = random_val(global_id.xy, frame_seed);
        }

        let grain_weight = clamp(0.05 * post.grain_intensity, 0.0, 0.05);
        let grain_rgb = vec3<f32>(rand_f, rand_f, rand_f);
        ldr = clamp(mix(ldr, grain_rgb, grain_weight), vec3<f32>(0.0), vec3<f32>(1.0));
    }

    // 3. PS2 Black/White/Red screen fade (`sh2gfw_Fade2` 0x00179d60, `sh2gfw_Fade3` 0x0017a2e0)
    if (post.fade_rgba.a > 0.001) {
        ldr = mix(ldr, post.fade_rgba.rgb, clamp(post.fade_rgba.a, 0.0, 1.0));
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
        assert_eq!(std::mem::size_of::<PostUniforms>(), 64);
        let default = PostUniforms::default();
        assert_eq!(default.frame_count, 0);
        assert_eq!(default.exposure, 1.0);
        assert_eq!(default.grain_intensity, 0.65);
        assert_eq!(default.dof_strength, 0.0);
        assert_eq!(default.motion_blur_strength, 0.0);
        // ACES is OFF by default for strict 1:1 Vanilla PS2 parity
        assert_eq!(default.flags & POST_FLAG_ACES_TONEMAP, 0);
        assert_ne!(default.flags & POST_FLAG_PS2_LCG_NOISE, 0);
    }

    #[test]
    fn test_post_uniforms_pod_roundtrip() {
        let uniforms = PostUniforms {
            time: 12.34,
            grain_intensity: 0.8,
            exposure: 1.25,
            frame_count: 742,
            dof_tap_offset_px: [1.5, -1.5],
            dof_strength: 0.85,
            motion_blur_strength: 0.45,
            flags: POST_FLAG_ACES_TONEMAP | POST_FLAG_PS2_LCG_NOISE,
            glow_threshold: 0.6,
            glow_strength: 0.35,
            glow_step_px: 2.0,
            fade_rgba: [1.0, 0.0, 0.0, 0.5],
        };

        let bytes = bytemuck::bytes_of(&uniforms);
        let back: &PostUniforms = bytemuck::from_bytes(bytes);
        assert_eq!(&uniforms, back);
    }
}
