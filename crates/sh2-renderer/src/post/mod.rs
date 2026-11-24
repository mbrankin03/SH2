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
    compute_swap_tap_offsets_12_4, decode_ps2_noise_clut_128x128, ps2_brightness_byte,
    ps2_brightness_scale, sample_ps2_noise_tiled_4x4, FilterRetainState, Ps2FadeKind,
    Ps2NoiseGenerator, SwapGlowSoftParams, SwapSoftParams, SwapSoftTestMode,
    GS_FILTER_RETAIN_ALPHA_1, PS2_BRIGHTNESS_LUT_U8, PS2_BRIGHTNESS_NEUTRAL_U8,
    PS2_FILTER_CMD_PAUSE_RETAIN, PS2_NOISE_BLEND_FIX, PS2_NOISE_CLUT_RGB, PS2_NOISE_LCG_INC,
    PS2_NOISE_LCG_MUL, PS2_NOISE_TEX_SIZE, PS2_NOISE_TILE_GRID, PS2_NOISE_VERTEX_RGB,
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

/// Authentic PS2 film-grain refresh cadence in Hz (`draw_main` 30 Hz loop in
/// `SLUS_202.28` `sh2gfw_drawloop_main.c` `0x0019feb0`). Decoupling noise updates to 30 Hz
/// prevents temporal washout and redundant GPU texture uploads at 120/240/unlimited FPS.
pub const PS2_NOISE_CADENCE_HZ: f32 = 30.0;

pub struct PostProcessPipeline {
    pub pipeline: wgpu::ComputePipeline,
    pub bind_group_layout: wgpu::BindGroupLayout,
    /// `128x128` `R8Unorm` GPU texture holding the bit-exact PS2 LCG noise frame
    /// (`SLUS_202.28` `sh2gfw_test_MakeNoise` `0x00177c50`).
    pub ps2_noise_texture: wgpu::Texture,
    /// Texture view for [`Self::ps2_noise_texture`].
    pub ps2_noise_view: wgpu::TextureView,
    /// Repeat bilinear sampler for screen-space noise tiling
    /// (`SLUS_202.28` `sh2gfw_SendDraw_Noise` `0x00177f30`, `TEX1_1 = 0x60`).
    pub ps2_noise_sampler: wgpu::Sampler,
    /// Stateful CPU LCG generator (`DAT_002adaf0`) stepped on the 30 Hz PS2 cadence.
    ps2_noise_gen: std::sync::Mutex<Ps2NoiseGenerator>,
    /// Last 30 Hz tick index (`floor(time * 30.0)`) uploaded to [`Self::ps2_noise_texture`],
    /// or `u64::MAX` before the first timed step.
    ps2_noise_last_tick: std::sync::atomic::AtomicU64,
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

// PS2 `sh2gfw_SendDraw_Noise` (`0x00177f30`) & `Noise_and_FontDraw` (`0x0019ff70`) GS constants:
// - `noise_clut` (`0x002ab5c0`): 16-entry PSMT4 CLUT with RGB = `0x00` or `0x18` (`24 / 255.0`).
// - `DAT_002adb55 = 0xe6` (`230`): vertex RGB modulation (`TFX = 0 MODULATE` -> `230.0 / 128.0`).
// - `DAT_002adb52 = 0x60` (`96`): `ALPHA_1 = (0x60 << 32) | 0x21` (`A=Cd, B=Cs, C=FIX(96), D=Cs`
//   => `Out = (Cd - Cs) * (96/128) + Cs = mix(Cd, Cs, 32/128)`).
const PS2_NOISE_CLUT_HIGH: f32 = 24.0 / 255.0;
const PS2_NOISE_VTX_MODULATE: f32 = 230.0 / 128.0;
const PS2_NOISE_BLEND_WEIGHT: f32 = (128.0 - 96.0) / 128.0;
// PS2 `512x448` framebuffer displays `4.0` horizontal x `448 / 128 = 3.5` vertical tiles of `128x128`
// noise (`sh2gfw_SendDraw_Noise` `0x00177f30`). Scaling X by `aspect` gives exact `4.0 x 3.5` at
// `512x448` and keeps grain texels square on 16:9 / 21:9 / 4K displays.
const PS2_NOISE_TILES_Y: f32 = 448.0 / 128.0;

// PS2 `sh2gfw_Swap_Soft` (`0x001785e0`) & `sh2gfw_Swap_GlowSoft` (`0x00178d60`) 12.4 sub-pixel
// half-texel bias (`-4` in 12.4 fixed-point = `-4.0 / 16.0 = -0.25` px) and PS2 NTSC vertical
// framebuffer height (`448.0` px) used to scale virtual tap offsets proportionally at 1080p/1440p/4K.
const PS2_SWAP_SUBPIXEL_BIAS_PX: f32 = -0.25;
const PS2_VIRTUAL_HEIGHT_PX: f32 = 448.0;

@group(0) @binding(0) var<uniform> post: PostUniforms;
@group(0) @binding(1) var hdr_in: texture_2d<f32>;
@group(0) @binding(2) var ldr_out: texture_storage_2d<rgba8unorm, write>;
@group(0) @binding(3) var ps2_noise_tex: texture_2d<f32>;
@group(0) @binding(4) var ps2_noise_sampler: sampler;

// Optional ACES filmic tonemapping approximation (native-upgrade, disabled in strict Vanilla PS2 mode)
fn aces_tonemap(x: vec3<f32>) -> vec3<f32> {
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    return clamp((x * (a * x + b)) / (x * (c * x + d) + e), vec3<f32>(0.0), vec3<f32>(1.0));
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
    let res_scale = max(f32(dims.y) / PS2_VIRTUAL_HEIGHT_PX, 1.0);
    var hdr_color = textureLoad(hdr_in, pixel_coords, 0).rgb * post.exposure;

    // 0a. PS2 `sh2gfw_Swap_Soft` (`0x001785e0`) 2x2 ping-pong DoF blur:
    //     Tap offsets (`compute_swap_tap_offsets_12_4(2, step)`): `[-1.5 * step - 0.25, +0.5 * step - 0.25]` px.
    //     GS `ALPHA_1 = (FIX << 32) | 0x64` (`A=Cs, B=Cd, C=FIX, D=Cd` => `dst = mix(dst, tap, FIX / 128.0)`).
    if (post.dof_strength > 0.001) {
        let dof_alpha = clamp(post.dof_strength, 0.0, 1.0);
        let ox0 = -1.5 * post.dof_tap_offset_px.x + PS2_SWAP_SUBPIXEL_BIAS_PX;
        let ox1 = 0.5 * post.dof_tap_offset_px.x + PS2_SWAP_SUBPIXEL_BIAS_PX;
        let oy0 = -1.5 * post.dof_tap_offset_px.y + PS2_SWAP_SUBPIXEL_BIAS_PX;
        let oy1 = 0.5 * post.dof_tap_offset_px.y + PS2_SWAP_SUBPIXEL_BIAS_PX;
        var buf_a = hdr_color;
        var buf_b = hdr_color;
        var flip: u32 = 1u;
        for (var iy: i32 = 0; iy < 2; iy = iy + 1) {
            let dy = select(oy0, oy1, iy == 1);
            for (var ix: i32 = 0; ix < 2; ix = ix + 1) {
                let dx = select(ox0, ox1, ix == 1);
                let delta = vec2<i32>(round(vec2<f32>(dx, dy) * res_scale));
                let tap = textureLoad(hdr_in, clamp(pixel_coords + delta, vec2<i32>(0), max_coord), 0).rgb * post.exposure;
                if (flip == 1u) {
                    buf_b = mix(buf_b, tap, dof_alpha);
                } else {
                    buf_a = mix(buf_a, tap, dof_alpha);
                }
                flip = flip ^ 1u;
            }
        }
        hdr_color = buf_a;
    }

    // 0b. PS2 `sh2gfw_Swap_GlowSoft` (`0x00178d60`) 2x2 ping-pong additive bloom:
    //     Tap offsets (`compute_swap_tap_offsets_12_4(2, step_param)`): `[-1.5 * step_px - 0.25, +0.5 * step_px - 0.25]` px.
    //     GS `TEST_1 = (aref << 4) | 0x3800b` (`ATST = 5 GEQUAL`) and `ALPHA_1 = (FIX << 32) | 0x68`
    //     (`A=Cs, B=0, C=FIX, D=Cd` => `dst = dst + tap * (FIX / 128.0)`).
    if (post.glow_strength > 0.001) {
        let glow_alpha = clamp(post.glow_strength, 0.0, 1.0);
        let go0 = -1.5 * post.glow_step_px + PS2_SWAP_SUBPIXEL_BIAS_PX;
        let go1 = 0.5 * post.glow_step_px + PS2_SWAP_SUBPIXEL_BIAS_PX;
        var buf_a = hdr_color;
        var buf_b = hdr_color;
        var flip: u32 = 1u;
        for (var iy: i32 = 0; iy < 2; iy = iy + 1) {
            let dy = select(go0, go1, iy == 1);
            for (var ix: i32 = 0; ix < 2; ix = ix + 1) {
                let dx = select(go0, go1, ix == 1);
                let delta = vec2<i32>(round(vec2<f32>(dx, dy) * res_scale));
                let tap = textureLoad(hdr_in, clamp(pixel_coords + delta, vec2<i32>(0), max_coord), 0).rgb * post.exposure;
                let luma = max(tap.r, max(tap.g, tap.b));
                if (luma >= post.glow_threshold) {
                    if (flip == 1u) {
                        buf_b = buf_b + tap * glow_alpha;
                    } else {
                        buf_a = buf_a + tap * glow_alpha;
                    }
                }
                flip = flip ^ 1u;
            }
        }
        hdr_color = buf_a;
    }

    // 0c. PS2 `sh2gfw_Filter_Blur` (`0x00177440` / `enSetBlur` `0x00146c30` `param_2 = 0x60`):
    //     GS `ALPHA_1 = 0x44` (`A=Cs, B=Cd, C=As, D=Cd` => `dst = mix(dst, tap, As / 128.0)`)
    //     evaluated over the `Swap_Soft` 2x2 unit-step (`step_px = 1.0`) tap grid.
    if (post.motion_blur_strength > 0.001) {
        let mb_alpha = clamp(post.motion_blur_strength, 0.0, 1.0);
        let mo0 = -1.5 + PS2_SWAP_SUBPIXEL_BIAS_PX;
        let mo1 = 0.5 + PS2_SWAP_SUBPIXEL_BIAS_PX;
        var buf_a = hdr_color;
        var buf_b = hdr_color;
        var flip: u32 = 1u;
        for (var iy: i32 = 0; iy < 2; iy = iy + 1) {
            let dy = select(mo0, mo1, iy == 1);
            for (var ix: i32 = 0; ix < 2; ix = ix + 1) {
                let dx = select(mo0, mo1, ix == 1);
                let delta = vec2<i32>(round(vec2<f32>(dx, dy) * res_scale));
                let tap = textureLoad(hdr_in, clamp(pixel_coords + delta, vec2<i32>(0), max_coord), 0).rgb * post.exposure;
                if (flip == 1u) {
                    buf_b = mix(buf_b, tap, mb_alpha);
                } else {
                    buf_a = mix(buf_a, tap, mb_alpha);
                }
                flip = flip ^ 1u;
            }
        }
        hdr_color = buf_a;
    }

    // 1. Optional ACES tonemapping (`native-upgrade`) vs strict 1:1 Vanilla PS2 linear clamp
    var ldr: vec3<f32>;
    if ((post.flags & POST_FLAG_ACES_TONEMAP) != 0u) {
        ldr = aces_tonemap(hdr_color);
    } else {
        ldr = clamp(hdr_color, vec3<f32>(0.0), vec3<f32>(1.0));
    }

    // 2. PS2 `sh2gfw_test_MakeNoise` (`0x00177c50`) + `sh2gfw_SendDraw_Noise` (`0x00177f30`):
    //    Samples the 128x128 PSMT4 `noise_clut` (`0x002ab5c0`: `0x00` / `0x18`) texture tiled
    //    `4.0 x 3.5` on PS2 `512x448` (aspect-scaled for square grain texels at 16:9 / 4K),
    //    modulates by `DAT_002adb55 = 0xe6` (`230 / 128.0`), and blends with `ALPHA_1 = (0x60 << 32) | 0x21`
    //    (`DAT_002adb52 = 0x60 = 96` => `mix(Cd, Cs, 32 / 128.0)` scaled by user `grain_intensity`).
    if (post.grain_intensity > 0.001) {
        var noise_texel: f32;
        if ((post.flags & POST_FLAG_PS2_LCG_NOISE) != 0u) {
            let fdims = max(vec2<f32>(dims), vec2<f32>(1.0, 1.0));
            let uv = (vec2<f32>(global_id.xy) + vec2<f32>(0.5, 0.5)) / fdims;
            let tiles = vec2<f32>(PS2_NOISE_TILES_Y * (fdims.x / fdims.y), PS2_NOISE_TILES_Y);
            noise_texel = textureSampleLevel(ps2_noise_tex, ps2_noise_sampler, uv * tiles, 0.0).r;
        } else {
            let frame_seed = u32(post.time * 30.0);
            let rand_f = random_val(global_id.xy, frame_seed);
            noise_texel = select(0.0, PS2_NOISE_CLUT_HIGH, rand_f >= 0.5);
        }

        let cs = vec3<f32>(noise_texel * PS2_NOISE_VTX_MODULATE);
        let grain_weight = PS2_NOISE_BLEND_WEIGHT * clamp(post.grain_intensity, 0.0, 1.0);
        ldr = clamp(mix(ldr, cs, grain_weight), vec3<f32>(0.0), vec3<f32>(1.0));
    }

    // 3. PS2 Black/White/Red screen fade (`sh2gfw_Fade2` 0x00179d60, `sh2gfw_Fade3` 0x0017a2e0)
    if (post.fade_rgba.a > 0.001) {
        ldr = mix(ldr, post.fade_rgba.rgb, clamp(post.fade_rgba.a, 0.0, 1.0));
    }

    textureStore(ldr_out, pixel_coords, vec4<f32>(ldr, 1.0));
}
"#;

    /// Creates the post-processing compute pipeline and uploads the initial `128x128`
    /// `R8Unorm` PS2 LCG noise texture (`sh2gfw_test_MakeNoise` `0x00177c50`).
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
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
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
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

        let ps2_noise_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("PS2 LCG 128x128 Film Grain Noise Texture (0x00177c50)"),
            size: wgpu::Extent3d {
                width: PS2_NOISE_TEX_SIZE as u32,
                height: PS2_NOISE_TEX_SIZE as u32,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let ps2_noise_view = ps2_noise_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let ps2_noise_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("PS2 LCG Noise 4x4 Repeat Sampler (0x00177f30)"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let mut gen = Ps2NoiseGenerator::new(0);
        let initial_bytes = decode_ps2_noise_clut_128x128(&gen.generate_128x128());
        let this = Self {
            pipeline,
            bind_group_layout,
            ps2_noise_texture,
            ps2_noise_view,
            ps2_noise_sampler,
            ps2_noise_gen: std::sync::Mutex::new(gen),
            ps2_noise_last_tick: std::sync::atomic::AtomicU64::new(u64::MAX),
        };
        this.upload_ps2_noise_bytes(queue, &initial_bytes);
        this
    }

    /// Uploads a raw `128x128` (`16,384`-byte) `R8Unorm` noise texture to the GPU
    /// (`sh2gfw_test_MakeNoise` `0x00177c50`). Returns `false` if `bytes.len() != 16_384`.
    pub fn upload_ps2_noise_bytes(&self, queue: &wgpu::Queue, bytes: &[u8]) -> bool {
        if bytes.len() != PS2_NOISE_TEX_SIZE * PS2_NOISE_TEX_SIZE {
            return false;
        }
        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &self.ps2_noise_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            bytes,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(PS2_NOISE_TEX_SIZE as u32),
                rows_per_image: Some(PS2_NOISE_TEX_SIZE as u32),
            },
            wgpu::Extent3d {
                width: PS2_NOISE_TEX_SIZE as u32,
                height: PS2_NOISE_TEX_SIZE as u32,
                depth_or_array_layers: 1,
            },
        );
        true
    }

    /// Advances the internal [`Ps2NoiseGenerator`] by one `128x128` frame (`0x00177c50`),
    /// decodes through `noise_clut` (`0x002ab5c0`), uploads it to [`Self::ps2_noise_texture`],
    /// and returns the updated LCG seed.
    pub fn step_ps2_noise(&self, queue: &wgpu::Queue) -> u32 {
        let (bytes, seed) = match self.ps2_noise_gen.lock() {
            Ok(mut gen) => {
                let bytes = decode_ps2_noise_clut_128x128(&gen.generate_128x128());
                (bytes, gen.seed)
            }
            Err(poisoned) => {
                let mut gen = poisoned.into_inner();
                let bytes = decode_ps2_noise_clut_128x128(&gen.generate_128x128());
                (bytes, gen.seed)
            }
        };
        self.upload_ps2_noise_bytes(queue, &bytes);
        seed
    }

    /// Steps and uploads the PS2 LCG noise texture only when `time_secs` crosses a new
    /// 30 Hz PS2 frame boundary ([`PS2_NOISE_CADENCE_HZ`]), returning `Some(new_seed)` when
    /// stepped or `None` when the current 30 Hz frame is already uploaded.
    ///
    /// Keeps film grain crisp and framerate-independent at 120/240/unlimited Hz.
    pub fn step_ps2_noise_at_time(&self, queue: &wgpu::Queue, time_secs: f32) -> Option<u32> {
        let tick = if time_secs.is_finite() && time_secs > 0.0 {
            (time_secs * PS2_NOISE_CADENCE_HZ).floor() as u64
        } else {
            0
        };
        let prev = self
            .ps2_noise_last_tick
            .swap(tick, std::sync::atomic::Ordering::Relaxed);
        if prev == tick {
            return None;
        }
        Some(self.step_ps2_noise(queue))
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

    #[test]
    fn test_post_wgsl_has_no_uncited_filter_constants() {
        let src = PostProcessPipeline::WGSL_SHADER;
        assert!(src.contains("PS2_NOISE_VTX_MODULATE"));
        assert!(src.contains("PS2_NOISE_BLEND_WEIGHT"));
        assert!(src.contains("PS2_NOISE_TILES_Y"));
        assert!(src.contains("PS2_VIRTUAL_HEIGHT_PX"));
        assert!(src.contains("PS2_SWAP_SUBPIXEL_BIAS_PX"));
        assert!(!src.contains("0.05 * post.grain_intensity"));
        assert!(!src.contains("1.0 / 9.0"));
        assert!(!src.contains("* 0.30"));
        assert!(!src.contains("* 0.20"));
        assert!(!src.contains("0.015"));
        assert!(!src.contains("0.85"));
    }
}
