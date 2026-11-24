//! Screen-space additive 7-sprite lens flare pipeline (`Lens/lens_flare.c`, `lens_flare.tbn2`).

use bytemuck::{Pod, Zeroable};
use sh2_core::lighting::LensFlareQuad;

/// Maximum simultaneous lens flare quads in the reusable GPU vertex buffer (`16 lights * 7 sprites = 112`).
pub const MAX_GPU_LENS_FLARE_QUADS: usize = 128;

/// Exponential visibility smoothing rate (`0.3`) from `SLUS_202.28` `kari_shLensFlareDraw`
/// (`0x00184ef0`, `Lens/kari_lf_draw.c` line 236: `rate += delta * 0.3`).
pub const PS2_LENS_FLARE_SMOOTH_RATE: f32 = 0.3;

/// PS2 NTSC pixel aspect multiplier (`512.0 / 448.0`) applied to lens flare vertical radius in
/// `SLUS_202.28` `shLensFlareDrawCommon` (`0x00184bf0`, `Lens/kari_lf_draw.c` line 183).
pub const PS2_LENS_FLARE_Y_ASPECT: f32 = 512.0 / 448.0;

/// Consecutive visible frames required before `shLensFlareLightCenterIsVisible` (`0x00187090`,
/// `Lens/lens_flare.c` line 67: `_lf_flicker_on < 4`) reports the flare center as unoccluded.
pub const PS2_LENS_FLARE_FLICKER_FRAMES: u32 = 4;

/// Authentic PS2 lens flare evaluation cadence (`30.0` Hz `draw_main` loop) used by
/// [`LensFlareSmoother::step_dt`] to make the `0.3` exponential smoothing and 4-frame debounce
/// framerate-independent at 60/120/240/unlimited Hz.
pub const PS2_LENS_FLARE_CADENCE_HZ: f32 = 30.0;

/// Stateful PS2 lens flare visibility smoother matching `SLUS_202.28` `kari_shLensFlareDraw`
/// (`0x00184ef0`), `shLensFlareDrawCommon` (`0x00184bf0`), and `shLensFlareLightCenterIsVisible`
/// (`0x00187090`).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct LensFlareSmoother {
    /// Smoothed effect intensity rate (`_DAT_010137d8`, range `0.0..=2.0`).
    pub current_rate: f32,
    /// Consecutive unoccluded frames counter (`_lf_flicker_on` in `0x00187090`).
    pub flicker_on_frames: u32,
    /// Continuous-time unoccluded accumulator in seconds for framerate-independent [`Self::step_dt`].
    pub flicker_on_secs: f32,
}

impl LensFlareSmoother {
    /// Advances the visibility debounce (`0x00187090`) and `0.3` exponential smoothing (`0x00184ef0`)
    /// for one frame, returning the updated smoothed rate.
    ///
    /// When `camera_is_smooth` is `false` (`shLensFlareCameraIsSmooth` `0x00187070`, camera cut),
    /// snaps `current_rate` immediately to `0.0`.
    pub fn step(&mut self, center_visible: bool, target_rate: f32, camera_is_smooth: bool) -> f32 {
        let debounced_visible = if !center_visible {
            self.flicker_on_frames = 0;
            false
        } else if self.flicker_on_frames < PS2_LENS_FLARE_FLICKER_FRAMES {
            self.flicker_on_frames += 1;
            false
        } else {
            true
        };

        let delta = if debounced_visible {
            target_rate - self.current_rate
        } else {
            -self.current_rate
        };
        self.current_rate += delta * PS2_LENS_FLARE_SMOOTH_RATE;

        if !camera_is_smooth {
            self.current_rate = 0.0;
        }
        self.current_rate
    }

    /// Framerate-independent equivalent of [`Self::step`] for 60/120/240/unlimited Hz rendering.
    ///
    /// Converts PS2's 30 Hz `0.3` per-frame smoothing (`0x00184ef0`) into exact continuous time
    /// `alpha = 1.0 - (1.0 - 0.3).powf(dt_secs * 30.0)` and measures the 4-frame debounce
    /// (`0x00187090`) as `4.0 / 30.0` seconds. At `dt_secs == 1.0 / 30.0`, matches [`Self::step`]
    /// identically.
    pub fn step_dt(
        &mut self,
        center_visible: bool,
        target_rate: f32,
        camera_is_smooth: bool,
        dt_secs: f32,
    ) -> f32 {
        let safe_dt = if dt_secs.is_finite() && dt_secs > 0.0 {
            dt_secs
        } else {
            0.0
        };
        let debounce_secs = (PS2_LENS_FLARE_FLICKER_FRAMES as f32) / PS2_LENS_FLARE_CADENCE_HZ;
        let debounced_visible = if !center_visible {
            self.flicker_on_secs = 0.0;
            self.flicker_on_frames = 0;
            false
        } else if self.flicker_on_secs < debounce_secs - 1e-6 {
            self.flicker_on_secs += safe_dt;
            self.flicker_on_frames =
                (self.flicker_on_secs * PS2_LENS_FLARE_CADENCE_HZ).floor() as u32;
            false
        } else {
            self.flicker_on_frames = PS2_LENS_FLARE_FLICKER_FRAMES;
            true
        };

        let alpha =
            1.0 - (1.0 - PS2_LENS_FLARE_SMOOTH_RATE).powf(safe_dt * PS2_LENS_FLARE_CADENCE_HZ);
        let delta = if debounced_visible {
            target_rate - self.current_rate
        } else {
            -self.current_rate
        };
        self.current_rate += delta * alpha;

        if !camera_is_smooth {
            self.current_rate = 0.0;
        }
        self.current_rate
    }

    /// Evaluates the PS2 sprite radius scale factor `(rate + 3.0) / 4.0`
    /// (`shLensFlareDrawCommon` `0x00184bf0` line 180).
    #[must_use]
    pub fn size_scale(rate: f32) -> f32 {
        (rate + 3.0) / 4.0
    }

    /// Evaluates the PS2 `(half_w, half_h)` for a sprite with `base_half_w` and smoothed `rate`,
    /// applying `(rate + 3.0) / 4.0` size scaling and `512.0 / 448.0` Y-aspect ratio
    /// (`shLensFlareDrawCommon` `0x00184bf0` lines 180-183).
    #[must_use]
    pub fn evaluate_sprite_half_extents(base_half_w: f32, rate: f32) -> [f32; 2] {
        let rx = base_half_w * Self::size_scale(rate);
        let ry = rx * PS2_LENS_FLARE_Y_ASPECT;
        [rx, ry]
    }
}

/// Applies PS2 `kari_shLensFlareDraw` (`0x00184ef0`) / `shLensFlareDrawCommon` (`0x00184bf0`)
/// size scaling `(rate + 3.0) / 4.0`, `512.0 / 448.0` Y aspect, and color intensity scaling
/// in-place to a slice of [`GpuLensFlareQuad`]s.
pub fn apply_ps2_lens_flare_smoothing(quads: &mut [GpuLensFlareQuad], rate: f32) {
    let size_mul = LensFlareSmoother::size_scale(rate);
    for q in quads {
        let half_w = q.rect[2] * size_mul;
        q.rect[2] = half_w;
        q.rect[3] = half_w * PS2_LENS_FLARE_Y_ASPECT;
        q.color[0] = (q.color[0] * rate).clamp(0.0, 1.0);
        q.color[1] = (q.color[1] * rate).clamp(0.0, 1.0);
        q.color[2] = (q.color[2] * rate).clamp(0.0, 1.0);
    }
}

/// Instanced GPU quad record for [`LensFlarePipeline`] (`48` bytes, 16-byte aligned).
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct GpuLensFlareQuad {
    /// `(ndc_cx, ndc_cy, ndc_half_w, ndc_half_h)`.
    pub rect: [f32; 4],
    /// `(u_min, v_min, u_max, v_max)` in `lens_flare.tbn2`.
    pub uv_rect: [f32; 4],
    /// Additive linear RGBA tint.
    pub color: [f32; 4],
}

impl From<LensFlareQuad> for GpuLensFlareQuad {
    fn from(q: LensFlareQuad) -> Self {
        Self {
            rect: [
                q.ndc_center[0],
                q.ndc_center[1],
                q.ndc_half_size[0],
                q.ndc_half_size[1],
            ],
            uv_rect: [q.uv_min[0], q.uv_min[1], q.uv_max[0], q.uv_max[1]],
            color: q.color_rgba,
        }
    }
}

/// Additive-blended screen-space pipeline for 7-sprite `lens_flare.tbn2` halo flares.
pub struct LensFlarePipeline {
    pub pipeline: wgpu::RenderPipeline,
    pub texture_bind_group_layout: wgpu::BindGroupLayout,
}

impl LensFlarePipeline {
    const WGSL_SHADER: &'static str = r#"
@group(0) @binding(0) var flare_tex: texture_2d<f32>;
@group(0) @binding(1) var flare_sampler: sampler;

struct InstanceInput {
    @location(0) rect: vec4<f32>,
    @location(1) uv_rect: vec4<f32>,
    @location(2) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vid: u32, inst: InstanceInput) -> VertexOutput {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 1.0, -1.0),
        vec2<f32>( 1.0,  1.0),
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 1.0,  1.0),
        vec2<f32>(-1.0,  1.0),
    );
    var uvs = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(0.0, 0.0),
    );
    let c = corners[vid];
    let t = uvs[vid];
    let pos = inst.rect.xy + c * inst.rect.zw;
    let uv = mix(inst.uv_rect.xy, inst.uv_rect.zw, t);

    var out: VertexOutput;
    out.clip_position = vec4<f32>(pos, 0.0, 1.0);
    out.uv = uv;
    out.color = inst.color;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let sample_rgba = textureSample(flare_tex, flare_sampler, in.uv);
    let rgb = sample_rgba.rgb * in.color.rgb * sample_rgba.a;
    return vec4<f32>(rgb, sample_rgba.a * in.color.a);
}
"#;

    /// Creates the additive-blended screen-space lens flare render pipeline.
    #[must_use]
    pub fn new(device: &wgpu::Device, hdr_color_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Lens Flare Shader"),
            source: wgpu::ShaderSource::Wgsl(Self::WGSL_SHADER.into()),
        });

        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Lens Flare Texture Bind Group Layout"),
                entries: &[
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
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Lens Flare Pipeline Layout"),
            bind_group_layouts: &[&texture_bind_group_layout],
            push_constant_ranges: &[],
        });

        let instance_vb = [wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<GpuLensFlareQuad>() as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: 16,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: 32,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }];

        let additive = wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent::OVER,
        };

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Lens Flare Pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &instance_vb,
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: hdr_color_format,
                    blend: Some(additive),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        Self {
            pipeline,
            texture_bind_group_layout,
        }
    }
}

/// Generates a procedural 128x128 radial halo fallback texture matching the 4-quadrant layout of
/// `lens_flare.tbn2` when retail assets are unavailable (e.g. headless unit tests).
#[must_use]
pub fn generate_procedural_lens_flare_rgba(size: u32) -> Vec<u8> {
    let s = size.max(16);
    let mut out = vec![0u8; (s * s * 4) as usize];
    let half = (s as f32) * 0.5;
    for y in 0..s {
        for x in 0..s {
            // 2x2 sub-atlas of soft radial glows
            let lx = ((x as f32) % half) / half * 2.0 - 1.0;
            let ly = ((y as f32) % half) / half * 2.0 - 1.0;
            let r = (lx * lx + ly * ly).sqrt();
            let v = (1.0 - r).clamp(0.0, 1.0).powf(2.2);
            let b = (v * 255.0).round() as u8;
            let idx = ((y * s + x) * 4) as usize;
            out[idx] = b;
            out[idx + 1] = b;
            out[idx + 2] = b;
            out[idx + 3] = b;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lens_flare_smoother_matches_slus_00184ef0_and_00187090() {
        let mut s = LensFlareSmoother::default();
        // First 4 visible frames debounce `_lf_flicker_on < 4` -> rate stays 0.0
        for _ in 0..PS2_LENS_FLARE_FLICKER_FRAMES {
            assert_eq!(s.step(true, 1.0, true), 0.0);
        }
        // Frame 5: visible -> 0.0 + (1.0 - 0.0) * 0.3 = 0.3
        let r1 = s.step(true, 1.0, true);
        assert!((r1 - 0.3).abs() < 1e-6);
        // Frame 6: 0.3 + (1.0 - 0.3) * 0.3 = 0.51
        let r2 = s.step(true, 1.0, true);
        assert!((r2 - 0.51).abs() < 1e-6);
        // Occluded: 0.51 + (-0.51) * 0.3 = 0.357, and flicker counter resets to 0
        let r3 = s.step(false, 1.0, true);
        assert!((r3 - 0.357).abs() < 1e-6);
        assert_eq!(s.flicker_on_frames, 0);
        // Camera cut (`camera_is_smooth == false`): snaps to 0.0
        let r4 = s.step(true, 1.0, false);
        assert_eq!(r4, 0.0);
    }

    #[test]
    fn lens_flare_size_scale_and_y_aspect_match_slus_00184bf0() {
        // `(rate + 3.0) / 4.0` at rate=1.0 -> 1.0; at rate=0.0 -> 0.75
        assert!((LensFlareSmoother::size_scale(1.0) - 1.0).abs() < 1e-6);
        assert!((LensFlareSmoother::size_scale(0.0) - 0.75).abs() < 1e-6);
        let [rx, ry] = LensFlareSmoother::evaluate_sprite_half_extents(0.1, 1.0);
        assert!((rx - 0.1).abs() < 1e-6);
        assert!((ry - (0.1 * 512.0 / 448.0)).abs() < 1e-6);
    }
}
