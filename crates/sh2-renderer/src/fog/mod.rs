//! 1:1 PS2-accurate multi-layered UV-scrolling and 3D raymarched volumetric fog (`SH2-32`).
//!
//! # Architecture
//! - [`FogUniforms`] & [`FogComputePipeline`]: Compute pass supporting both strict 1:1 PS2
//!   depth-fog + dual-layer UV-scrolling noise planes (`FogQuality::ClassicPs2`, matching
//!   `sh2gfw_Set_DepthFog` @ `0x00175df0`, `fog_view_screen_fog` @ `0x0016c1c0`, `FogFix.cpp`)
//!   and discrete-step 3D volumetric raymarching with spotlight shadow-tested god rays
//!   (`FogQuality::ModernVolumetric` + `VideoOptions::volumetric_light`).
//! - [`shadow::SpotlightShadowPipeline`]: Lightweight `512x512` `Depth32Float` shadow depth pass
//!   for flashlight volumetric inscattering occlusion.
//! - [`particles::FogParticlePipeline`]: Instanced camera-facing billboards for localized 3D
//!   proximity fog sprites (`fogMakePacket` @ `0x0016bb70`, `ObservationDeckFogFix.cpp`).

pub mod particles;
pub mod shadow;

pub use particles::{FogParticlePipeline, GpuFogParticle, MAX_GPU_FOG_PARTICLES};
pub use shadow::{
    compute_spotlight_view_proj, SpotlightShadowPipeline, SpotlightShadowUniforms,
    SPOTLIGHT_SHADOW_MAP_SIZE,
};

use sh2_core::config::FogQuality;
use sh2_core::fog::{Ps2FogLayerParams, RoomFogEvaluation};

/// Uniform buffer for the multi-layered & volumetric fog compute shader.
///
/// Layout is strictly 16-byte aligned for WGSL uniform compliance (total size: 160 bytes).
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct FogUniforms {
    /// Fog ambient color RGBA in 0..1 display/linear space (`a` scales effective density). (Offset 0, size 16)
    pub fog_color: [f32; 4],
    /// Near distance cutoff in PS2 mm where fog begins (`sh2gfw_Set_FogNear` @ `0x00175dc0`). (Offset 16, size 4)
    pub fog_near: f32,
    /// Far distance cutoff in PS2 mm where fog saturates (`sh2gfw_Set_Fogfar` @ `0x00175db0`). (Offset 20, size 4)
    pub fog_far: f32,
    /// Fog density multiplier (`0.0` disables fog). (Offset 24, size 4)
    pub density: f32,
    /// Deterministic elapsed simulation time in seconds for frame-rate-decoupled drift. (Offset 28, size 4)
    pub time: f32,
    /// Number of discrete volumetric raymarching steps (e.g., 16, 24, 32). (Offset 32, size 4)
    pub step_count: u32,
    /// Density curve falloff type: `0` = Linear, `1` = Exponential, `2` = Exponential Squared (`dbg_fogparm_ctl`). (Offset 36, size 4)
    pub curve_type: u32,
    /// Drift speed multiplier matching SH2:EE `FogSpeedFix` (`FogAdjustments.cpp:272-306`). (Offset 40, size 4)
    pub speed_mult: f32,
    /// Ambient-to-fog tint coupling factor (`dbg_ambifog_ctl`, `0.0..=1.0`). (Offset 44, size 4)
    pub ambient_fog_coupling: f32,
    /// Dual-layer UV scale `[fog_layer1_x1, fog_layer1_y1, fog_layer1_x2, fog_layer1_y2]` (`Settings.h:213-216`). (Offset 48, size 16)
    pub layer1_uv_scale: [f32; 4],
    /// Layer 2 params `[complexity, density_mult, density_add, outdoor_layers_enabled (0/1)]` (`FogFix.cpp`). (Offset 64, size 16)
    pub layer2_params: [f32; 4],
    /// `[layer1_alpha, layer2_alpha, volumetric_mode (0=ClassicPs2, 1=ModernVolumetric), volumetric_light_enabled (0/1)]`. (Offset 80, size 16)
    pub layer_transparencies: [f32; 4],
    /// Spotlight shadow-map view-projection matrix for god-ray occlusion sampling. (Offset 96, size 64)
    pub shadow_view_proj: [[f32; 4]; 4],
}

impl Default for FogUniforms {
    fn default() -> Self {
        let layers = Ps2FogLayerParams::default();
        Self {
            fog_color: [0.7, 0.7, 0.75, 1.0],
            fog_near: 2.0,
            fog_far: 25.0,
            density: 1.0,
            time: 0.0,
            step_count: 24,
            curve_type: 0,
            speed_mult: 1.0,
            ambient_fog_coupling: 1.0,
            layer1_uv_scale: [
                layers.layer1_x1,
                layers.layer1_y1,
                layers.layer1_x2,
                layers.layer1_y2,
            ],
            layer2_params: [
                layers.layer2_complexity,
                layers.layer2_density_mult,
                layers.layer2_density_add,
                1.0,
            ],
            layer_transparencies: [
                f32::from(layers.transparency_layer1) / 255.0,
                f32::from(layers.transparency_layer2) / 255.0,
                1.0, // ModernVolumetric default (matches VideoOptions::default())
                1.0, // volumetric_light enabled default
            ],
            shadow_view_proj: glam::Mat4::IDENTITY.to_cols_array_2d(),
        }
    }
}

impl FogUniforms {
    /// Builds [`FogUniforms`] from an evaluated [`RoomFogEvaluation`] and user video quality toggles.
    #[must_use]
    pub fn from_room_evaluation(
        eval: &RoomFogEvaluation,
        fog_quality: FogQuality,
        volumetric_light: bool,
    ) -> Self {
        Self {
            fog_color: eval.fog_color,
            fog_near: eval.fog_near_mm,
            fog_far: eval.fog_far_mm,
            density: eval.density,
            time: 0.0,
            step_count: match fog_quality {
                FogQuality::ClassicPs2 => 1,
                FogQuality::ModernVolumetric => 24,
            },
            curve_type: eval.curve as u32,
            speed_mult: eval.speed_mult,
            ambient_fog_coupling: 1.0,
            layer1_uv_scale: [
                eval.layers.layer1_x1,
                eval.layers.layer1_y1,
                eval.layers.layer1_x2,
                eval.layers.layer1_y2,
            ],
            layer2_params: [
                eval.layers.layer2_complexity,
                eval.layers.layer2_density_mult,
                eval.layers.layer2_density_add,
                if eval.outdoor_layers_enabled {
                    1.0
                } else {
                    0.0
                },
            ],
            layer_transparencies: [
                f32::from(eval.layers.transparency_layer1) / 255.0,
                f32::from(eval.layers.transparency_layer2) / 255.0,
                match fog_quality {
                    FogQuality::ClassicPs2 => 0.0,
                    FogQuality::ModernVolumetric => 1.0,
                },
                if volumetric_light { 1.0 } else { 0.0 },
            ],
            shadow_view_proj: glam::Mat4::IDENTITY.to_cols_array_2d(),
        }
    }

    /// Applies user [`FogQuality`] and `volumetric_light` toggle settings in place.
    pub fn apply_video_options(&mut self, fog_quality: FogQuality, volumetric_light: bool) {
        self.layer_transparencies[2] = match fog_quality {
            FogQuality::ClassicPs2 => 0.0,
            FogQuality::ModernVolumetric => 1.0,
        };
        self.layer_transparencies[3] = if volumetric_light { 1.0 } else { 0.0 };
        self.step_count = match fog_quality {
            FogQuality::ClassicPs2 => 1,
            FogQuality::ModernVolumetric => self.step_count.max(16),
        };
    }
}

/// Generates a seamless, tileable 256×256 RGBA8 procedural mist noise texture used as a fallback
/// when retail `data/pic/effect/fog.tex` is not loaded (e.g., headless tests or synthetic scenes).
#[must_use]
pub fn generate_procedural_fog_noise_rgba(size: u32) -> Vec<u8> {
    let dim = size.max(16) as usize;
    let mut out = vec![0u8; dim * dim * 4];
    let inv_dim = 1.0 / (dim as f32);

    // Tileable value-noise helper with periodic wrap
    let hash2 = |ix: i32, iy: i32, period: i32| -> f32 {
        let px = ix.rem_euclid(period) as u32;
        let py = iy.rem_euclid(period) as u32;
        let mut n = px
            .wrapping_mul(374_761_393)
            .wrapping_add(py.wrapping_mul(668_265_263));
        n = (n ^ (n >> 13)).wrapping_mul(1_274_126_177);
        ((n ^ (n >> 16)) & 0x00FF_FFFF) as f32 / (0x0100_0000 as f32)
    };

    let tile_noise = |u: f32, v: f32, period: i32| -> f32 {
        let fx = u * period as f32;
        let fy = v * period as f32;
        let x0 = fx.floor() as i32;
        let y0 = fy.floor() as i32;
        let tx = fx - x0 as f32;
        let ty = fy - y0 as f32;
        let sx = tx * tx * (3.0 - 2.0 * tx);
        let sy = ty * ty * (3.0 - 2.0 * ty);
        let n00 = hash2(x0, y0, period);
        let n10 = hash2(x0 + 1, y0, period);
        let n01 = hash2(x0, y0 + 1, period);
        let n11 = hash2(x0 + 1, y0 + 1, period);
        let nx0 = n00 + (n10 - n00) * sx;
        let nx1 = n01 + (n11 - n01) * sx;
        nx0 + (nx1 - nx0) * sy
    };

    for y in 0..dim {
        let v = y as f32 * inv_dim;
        for x in 0..dim {
            let u = x as f32 * inv_dim;
            let val = 0.50 * tile_noise(u, v, 4)
                + 0.28 * tile_noise(u, v, 8)
                + 0.14 * tile_noise(u, v, 16)
                + 0.08 * tile_noise(u, v, 32);
            let byte = (val.clamp(0.0, 1.0) * 255.0).round() as u8;
            let idx = (y * dim + x) * 4;
            out[idx] = byte;
            out[idx + 1] = byte;
            out[idx + 2] = byte;
            out[idx + 3] = 255;
        }
    }
    out
}

/// Compute pipeline for multi-layered UV-scrolling and 3D raymarched volumetric fog.
pub struct FogComputePipeline {
    pub pipeline: wgpu::ComputePipeline,
    pub bind_group_layout: wgpu::BindGroupLayout,
}

impl FogComputePipeline {
    /// Complete WGSL source for the volumetric fog compute shader.
    pub const WGSL_SHADER: &'static str = include_str!("shader.wgsl");

    /// Creates the volumetric fog compute pipeline and its 9-binding layout.
    #[must_use]
    pub fn new(device: &wgpu::Device) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Volumetric Fog Compute Shader"),
            source: wgpu::ShaderSource::Wgsl(Self::WGSL_SHADER.into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Fog Bind Group Layout"),
            entries: &[
                // 0: FogUniforms
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
                // 1: CameraUniforms
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
                // 2: Scene Depth32Float texture
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
                // 3: Scene HDR color_in texture
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
                // 4: Fogged HDR color_out storage texture
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
                // 5: SpotlightUniforms
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // 6: Spotlight shadow depth texture
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // 7: Tileable fog noise texture (fog.tex or procedural fallback)
                wgpu::BindGroupLayoutEntry {
                    binding: 7,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // 8: Linear repeat sampler for fog noise texture
                wgpu::BindGroupLayoutEntry {
                    binding: 8,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
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
    use sh2_core::fog::{evaluate_room_fog, RoomFogContext, R_OBSV_DECK_ID};

    #[test]
    fn test_fog_uniforms_layout_and_size() {
        assert_eq!(std::mem::size_of::<FogUniforms>(), 160);
        assert_eq!(std::mem::align_of::<FogUniforms>(), 4);
    }

    #[test]
    fn test_fog_uniforms_toggle_between_classic_and_volumetric() {
        let eval = evaluate_room_fog(&RoomFogContext {
            room_id: R_OBSV_DECK_ID,
            ..Default::default()
        });
        let classic = FogUniforms::from_room_evaluation(&eval, FogQuality::ClassicPs2, false);
        assert_eq!(classic.step_count, 1);
        assert_eq!(classic.layer_transparencies[2], 0.0);
        assert_eq!(classic.layer_transparencies[3], 0.0);

        let modern = FogUniforms::from_room_evaluation(&eval, FogQuality::ModernVolumetric, true);
        assert_eq!(modern.step_count, 24);
        assert_eq!(modern.layer_transparencies[2], 1.0);
        assert_eq!(modern.layer_transparencies[3], 1.0);
    }

    #[test]
    fn test_procedural_fog_noise_is_non_flat() {
        let rgba = generate_procedural_fog_noise_rgba(64);
        assert_eq!(rgba.len(), 64 * 64 * 4);
        let min = rgba.iter().step_by(4).copied().min().unwrap_or(0);
        let max = rgba.iter().step_by(4).copied().max().unwrap_or(0);
        assert!(
            max > min + 30,
            "procedural fog noise should have visible contrast"
        );
    }
}
