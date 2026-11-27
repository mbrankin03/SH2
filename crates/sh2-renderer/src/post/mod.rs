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

/// Bitflag in [`PostUniforms::flags`]: enables subtle screen-space refractive heat-haze
/// distortion for fire scenes (`native-upgrade`, `SH2-100`). Cleared in strict 1:1 Vanilla PS2 mode.
pub const POST_FLAG_FIRE_HEAT_HAZE: u32 = 1 << 2;

/// Bitflag in [`PostUniforms::flags`]: enables `sh2pc.exe` (`0x00477d30` / `0x0047c680` /
/// `0x00478070`) + SH2:EE `PS2NoiseFilter.cpp` (`BrightnessControl = 7.4`, `FilterByteEDX = 0x1d7`)
/// finer `128px`-tiled noise filter instead of strict `SLUS_202.28` `448p` `3.5`-tile GS grain.
pub const POST_FLAG_SH2EE_NOISE: u32 = 1 << 3;

/// Bitflag in [`PostUniforms::flags`]: activates PS2 `sh2gfw_Filter_Retain` (`0x00179b10`,
/// `sh2gfw_Set_PauseRetain` `0x0017ae90`, `ALPHA_1 = 0x8000000062`) previous-frame freeze.
pub const POST_FLAG_FILTER_RETAIN: u32 = 1 << 4;

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
    /// Dual-plane DoF (`EffectMode = 8`, `sh2pc.exe` `0x005a1f30` / `SLUS_202.28` `0x001785e0`)
    /// parameters: `[focus_dist_mm, near_strength, far_strength, step_ps2_px]`.
    pub dof_focus_params: [f32; 4],
}

impl Default for PostUniforms {
    fn default() -> Self {
        Self {
            time: 0.0,
            // Strict 1:1 PS2 grain weight (`sh2gfw_SendDraw_Noise` `0x00177f30`,
            // `GFW/sh2gfw_2d_filters.c:658-666`, `ALPHA_1 = (param_2 << 32) | 0x21` with
            // `param_2 = DAT_002adb52 = 0x60` -> `1 - 96/128 = 32/128`): unattenuated `1.0` default.
            grain_intensity: 1.0,
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
            dof_focus_params: [0.0, 0.0, 0.0, 0.0],
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
    /// Stateful CPU LCG generator (`DAT_002adaf0`) and reusable `128x128` upload scratch buffer
    /// stepped on the 30 Hz PS2 cadence without per-frame heap allocations (`SH2-182`).
    ps2_noise_state: std::sync::Mutex<(
        Ps2NoiseGenerator,
        Box<[u8; PS2_NOISE_TEX_SIZE * PS2_NOISE_TEX_SIZE]>,
    )>,
    /// Last 30 Hz tick index (`floor(time * 30.0)`) uploaded to [`Self::ps2_noise_texture`],
    /// or `u64::MAX` before the first timed step.
    ps2_noise_last_tick: std::sync::atomic::AtomicU64,
}

impl PostProcessPipeline {
    /// Post-processing compute shader (`Swap_Soft`, `Swap_GlowSoft`, `Filter_Blur`,
    /// optional ACES tonemapping, PS2 LCG film grain, and `Fade2`/`Fade3`).
    pub const WGSL_SHADER: &'static str = include_str!("shader.wgsl");

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
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Previous-frame LDR feedback texture for `sh2gfw_Filter_Retain` (`0x00179b10`,
                // `ALPHA_1 = 0x8000000062`) and `sh2gfw_Filter_Blur` (`0x00177440`, `ALPHA_1 = 0x44`).
                wgpu::BindGroupLayoutEntry {
                    binding: 7,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
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
        let mut scratch = Box::new([0u8; PS2_NOISE_TEX_SIZE * PS2_NOISE_TEX_SIZE]);
        gen.generate_128x128_clut_into(&mut scratch);
        let this = Self {
            pipeline,
            bind_group_layout,
            ps2_noise_texture,
            ps2_noise_view,
            ps2_noise_sampler,
            ps2_noise_state: std::sync::Mutex::new((gen, scratch)),
            ps2_noise_last_tick: std::sync::atomic::AtomicU64::new(u64::MAX),
        };
        if let Ok(guard) = this.ps2_noise_state.lock() {
            this.upload_ps2_noise_bytes(queue, guard.1.as_slice());
        }
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
    /// decodes through `noise_clut` (`0x002ab5c0`) into the reusable scratch buffer, uploads it
    /// to [`Self::ps2_noise_texture`], and returns the updated LCG seed.
    pub fn step_ps2_noise(&self, queue: &wgpu::Queue) -> u32 {
        let mut guard = match self.ps2_noise_state.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        let (ref mut gen, ref mut scratch) = *guard;
        gen.generate_128x128_clut_into(scratch);
        let seed = gen.seed;
        self.upload_ps2_noise_bytes(queue, scratch.as_slice());
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
        assert_eq!(std::mem::size_of::<PostUniforms>(), 80);
        let default = PostUniforms::default();
        assert_eq!(default.frame_count, 0);
        assert_eq!(default.exposure, 1.0);
        assert_eq!(default.grain_intensity, 1.0);
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
            dof_focus_params: [450.0, 0.75, 0.0, 2.0],
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
        assert!(src.contains("mix(ldr, cs_srgb, grain_weight)"));
        assert!(!src.contains("0.05 * post.grain_intensity"));
        assert!(!src.contains("1.0 / 9.0"));
        assert!(!src.contains("* 0.30"));
        assert!(!src.contains("* 0.20"));
        assert!(!src.contains("0.015"));
        assert!(!src.contains("0.85"));
    }

    #[test]
    fn ps2_noise_clut_into_matches_allocating_path_bit_for_bit() {
        let mut gen_ref = Ps2NoiseGenerator::new(0x1234_5678);
        let mut gen_fast = Ps2NoiseGenerator::new(0x1234_5678);
        let mut scratch = Box::new([0u8; PS2_NOISE_TEX_SIZE * PS2_NOISE_TEX_SIZE]);

        for _ in 0..3 {
            let expected = decode_ps2_noise_clut_128x128(&gen_ref.generate_128x128());
            gen_fast.generate_128x128_clut_into(&mut scratch);
            assert_eq!(gen_ref.seed, gen_fast.seed);
            assert_eq!(expected.as_slice(), scratch.as_slice());
        }
    }
}
