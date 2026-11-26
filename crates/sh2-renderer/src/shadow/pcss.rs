//! Native Enhanced GPU Percentage-Closer Soft Shadows (PCSS) & Character Self-Shadowing (`SH2-81`).
//!
//! Renders BOTH static opaque level geometry ([`GpuMap`]) AND animated skeletal meshes
//! ([`GpuSkeletalMesh`] with full 4-bone Linear Blend Skinning) into a high-resolution
//! (`2048x2048` `Depth32Float`) spotlight shadow map.
//!
//! In [`sh2_core::config::ShadowQualityMode::NativePcss`], both [`crate::pipeline::GeometryPipeline`]
//! and [`crate::skeletal::SkeletalPipeline`] sample this shadow map with a 16-tap Vogel-disk
//! blocker search, contact-hardening penumbra radius estimation, and 16-tap rotated PCF filter.

use bytemuck::{Pod, Zeroable};

use crate::gpu_map::GpuMap;
use crate::SkeletalDrawItem;
use sh2_assets::formats::map::BatchGroup;

/// High-resolution spotlight PCSS shadow map size in texels (`2048x2048` `Depth32Float`).
pub const PCSS_SHADOW_MAP_SIZE: u32 = 2048;

/// Computes the spotlight view-projection matrix for the `2048x2048` PCSS shadow map.
///
/// Uses a `240.0 mm` near clip plane so the emitter sits cleanly outside James's chest pocket
/// (`~150 mm` from torso centerline) without his own jacket occluding the flashlight lens,
/// while outstretched arms/weapons, enemies, and room furniture cast crisp contact-hardening shadows.
#[must_use]
pub fn compute_pcss_spotlight_view_proj(spot: &crate::light::SpotlightUniforms) -> glam::Mat4 {
    let pos = glam::Vec3::new(spot.position[0], spot.position[1], spot.position[2]);
    let raw_dir = glam::Vec3::new(
        spot.direction_inner[0],
        spot.direction_inner[1],
        spot.direction_inner[2],
    );
    let dir = raw_dir.normalize_or(glam::Vec3::NEG_Z);
    let up = if dir.y.abs() > 0.95 {
        glam::Vec3::X
    } else {
        glam::Vec3::Y
    };
    let view = glam::Mat4::look_to_rh(pos, dir, up);
    let outer_cos = spot.color_outer[3].clamp(0.05, 0.99);
    let half_angle = outer_cos.acos();
    let fov_y = (half_angle * 2.15).clamp(0.26, 2.1);
    let proj = glam::Mat4::perspective_rh(fov_y, 1.0, 240.0, 18_000.0);
    proj * view
}

/// Uniform buffer shared by [`crate::pipeline::GeometryPipeline`] and [`crate::skeletal::SkeletalPipeline`]
/// for Native PCSS sampling and SH2:EE lighting/specular parity (`80` bytes, 16-byte aligned).
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct PcssShadowUniforms {
    /// Spotlight perspective view-projection matrix transforming world mm -> shadow clip space.
    pub light_view_proj: [[f32; 4]; 4],
    /// - `x`: PCSS enabled (`1.0` = Native PCSS active, `0.0` = disabled for Classic PS2 / SH2:EE Stencil).
    /// - `y`: Effective shadow opacity (`0.0..=1.0`, incorporating per-room SH2:EE opacity & 33-frame fade).
    /// - `z`: Light emitter radius in shadow UV space (`0.024` default for contact-hardening penumbras).
    /// - `w`: Character specular intensity scale (`0.5` with SH2:EE `SpecularFix`, `1.0` for PS2 Authentic).
    pub params: [f32; 4],
}

impl Default for PcssShadowUniforms {
    fn default() -> Self {
        Self {
            light_view_proj: glam::Mat4::IDENTITY.to_cols_array_2d(),
            params: [1.0, 170.0 / 255.0, 0.024, 0.5],
        }
    }
}

/// Reusable WGSL PCSS blocker search + contact-hardening penumbra + 16-tap Vogel-disk PCF helper.
///
/// Expects `@group(0) @binding(2) var<uniform> pcss_u: PcssShadowUniforms;` and
/// `@group(0) @binding(3) var pcss_shadow_map: texture_depth_2d;` to be bound in group 0.
pub const WGSL_PCSS_FUNCTIONS: &str = r#"
struct PcssShadowUniforms {
    light_view_proj: mat4x4<f32>,
    params: vec4<f32>,
};

fn pcss_vogel_disk(sample_index: u32, sample_count: u32, phi: f32) -> vec2<f32> {
    let golden_angle = 2.39996323;
    let r = sqrt((f32(sample_index) + 0.5) / f32(sample_count));
    let theta = f32(sample_index) * golden_angle + phi;
    return vec2<f32>(cos(theta), sin(theta)) * r;
}

fn pcss_hash12(p: vec2<f32>) -> f32 {
    let h = dot(p, vec2<f32>(127.1, 311.7));
    return fract(sin(h) * 43758.5453123);
}

fn pcss_load_depth(uv: vec2<f32>, dims: vec2<i32>) -> f32 {
    let coord = clamp(
        vec2<i32>(floor(uv * vec2<f32>(dims))),
        vec2<i32>(0, 0),
        dims - vec2<i32>(1, 1)
    );
    return textureLoad(pcss_shadow_map, coord, 0);
}

// Evaluates contact-hardening PCSS spotlight visibility in [1.0 - opacity, 1.0].
fn evaluate_pcss_spotlight_visibility(world_pos: vec3<f32>, surf_normal: vec3<f32>, light_dir: vec3<f32>) -> f32 {
    if (pcss_u.params.x < 0.5 || pcss_u.params.y <= 0.001) {
        return 1.0;
    }

    // Normal offset bias to suppress self-shadow acne on curved limbs & walls
    let n_dot_l = clamp(dot(surf_normal, light_dir), 0.0, 1.0);
    let biased_pos = world_pos + surf_normal * (12.0 + 18.0 * (1.0 - n_dot_l));
    let clip = pcss_u.light_view_proj * vec4<f32>(biased_pos, 1.0);
    if (clip.w <= 0.001) {
        return 1.0;
    }

    let ndc = clip.xyz / clip.w;
    let uv = vec2<f32>(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5);
    if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0 || ndc.z < 0.0 || ndc.z > 1.0) {
        return 1.0;
    }

    let dims_u = textureDimensions(pcss_shadow_map);
    let dims = vec2<i32>(i32(dims_u.x), i32(dims_u.y));
    let receiver_depth = ndc.z;
    let depth_bias = 0.00035 + 0.0008 * (1.0 - n_dot_l);
    let light_size = max(pcss_u.params.z, 0.004);
    let phi = pcss_hash12(world_pos.xz * 0.13 + world_pos.yy * 0.07) * 6.2831853;

    // 1. Blocker search (16-tap Vogel disk)
    var blocker_sum = 0.0;
    var blocker_count = 0.0;
    let search_radius = light_size * 0.65;
    for (var i = 0u; i < 16u; i = i + 1u) {
        let offset = pcss_vogel_disk(i, 16u, phi) * search_radius;
        let sample_d = pcss_load_depth(uv + offset, dims);
        if (sample_d + depth_bias < receiver_depth) {
            blocker_sum = blocker_sum + sample_d;
            blocker_count = blocker_count + 1.0;
        }
    }

    if (blocker_count < 0.5) {
        return 1.0;
    }

    // 2. Contact-hardening penumbra radius estimation
    let avg_blocker = blocker_sum / blocker_count;
    let penumbra_ratio = clamp((receiver_depth - avg_blocker) / max(avg_blocker, 0.001), 0.02, 1.0);
    let filter_radius = clamp(penumbra_ratio * light_size, 0.0008, 0.032);

    // 3. Variable-radius 16-tap rotated PCF
    var lit_sum = 0.0;
    for (var i = 0u; i < 16u; i = i + 1u) {
        let offset = pcss_vogel_disk(i, 16u, phi + 1.13) * filter_radius;
        let sample_d = pcss_load_depth(uv + offset, dims);
        lit_sum = lit_sum + select(0.0, 1.0, receiver_depth - depth_bias <= sample_d);
    }

    let raw_vis = lit_sum / 16.0;
    // Softly fade shadow influence near shadow-map UV borders so no hard frustum rectangle ever appears
    let edge_dist = min(min(uv.x, 1.0 - uv.x), min(uv.y, 1.0 - uv.y));
    let border_fade = smoothstep(0.0, 0.04, edge_dist);
    let opacity = clamp(pcss_u.params.y, 0.0, 1.0) * border_fade;
    return mix(1.0 - opacity, 1.0, raw_vis);
}
"#;

/// Depth-only render pipelines for Native PCSS shadow mapping of BOTH static level meshes
/// and animated skeletal characters (enabling true character/weapon self-shadowing).
pub struct PcssShadowPipeline {
    /// Static `GpuMap` depth-only pipeline.
    pub static_pipeline: wgpu::RenderPipeline,
    /// Skinned `GpuSkeletalMesh` (4-bone LBS) depth-only pipeline.
    pub skeletal_pipeline: wgpu::RenderPipeline,
    /// Group 0 bind group layout (`PcssShadowUniforms` @ binding 0).
    pub bind_group_layout: wgpu::BindGroupLayout,
}

impl PcssShadowPipeline {
    const WGSL_STATIC_SHADOW_SHADER: &'static str = r#"
struct PcssShadowUniforms {
    light_view_proj: mat4x4<f32>,
    params: vec4<f32>,
};

@group(0) @binding(0) var<uniform> shadow_u: PcssShadowUniforms;

struct VertexInput {
    @location(0) position: vec3<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> @builtin(position) vec4<f32> {
    return shadow_u.light_view_proj * vec4<f32>(in.position, 1.0);
}
"#;

    const WGSL_SKELETAL_SHADOW_SHADER: &'static str = r#"
struct PcssShadowUniforms {
    light_view_proj: mat4x4<f32>,
    params: vec4<f32>,
};

struct SkeletonBones {
    matrices: array<mat4x4<f32>, 128>,
};

@group(0) @binding(0) var<uniform> shadow_u: PcssShadowUniforms;
@group(1) @binding(0) var<uniform> skeleton: SkeletonBones;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(3) bone_indices: vec4<u32>,
    @location(4) weights: vec4<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> @builtin(position) vec4<f32> {
    let w = in.weights;
    let b = in.bone_indices;
    let skin = skeleton.matrices[b.x] * w.x
             + skeleton.matrices[b.y] * w.y
             + skeleton.matrices[b.z] * w.z
             + skeleton.matrices[b.w] * w.w;
    let skinned_pos = skin * vec4<f32>(in.position, 1.0);
    return shadow_u.light_view_proj * vec4<f32>(skinned_pos.xyz, 1.0);
}
"#;

    /// Creates both static map and skeletal LBS depth-only PCSS shadow pipelines.
    #[must_use]
    pub fn new(
        device: &wgpu::Device,
        depth_format: wgpu::TextureFormat,
        bone_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let static_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("PCSS Static Shadow Depth Shader"),
            source: wgpu::ShaderSource::Wgsl(Self::WGSL_STATIC_SHADOW_SHADER.into()),
        });
        let skeletal_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("PCSS Skeletal Shadow Depth Shader"),
            source: wgpu::ShaderSource::Wgsl(Self::WGSL_SKELETAL_SHADOW_SHADER.into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("PCSS Shadow Depth Bind Group Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let static_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("PCSS Static Shadow Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let skeletal_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("PCSS Skeletal Shadow Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout, bone_bind_group_layout],
            push_constant_ranges: &[],
        });

        let depth_stencil = Some(wgpu::DepthStencilState {
            format: depth_format,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::Less,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState {
                constant: 2,
                slope_scale: 1.75,
                clamp: 0.0,
            },
        });

        // Match MapVertex stride, reading only @location(0) position
        let static_vb = [wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<sh2_assets::formats::map::MapVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32x3,
            }],
        }];

        let static_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("PCSS Static Shadow Depth Pipeline"),
            layout: Some(&static_layout),
            vertex: wgpu::VertexState {
                module: &static_shader,
                entry_point: Some("vs_main"),
                buffers: &static_vb,
                compilation_options: Default::default(),
            },
            fragment: None,
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: depth_stencil.clone(),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Match SkeletalVertex stride (64 bytes: pos[3]=0, norm[3]=12, uv[2]=24, bone_idx[4]=32, weights[4]=48)
        let skeletal_vb = [wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<crate::skeletal::SkeletalVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: 32,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Uint32x4,
                },
                wgpu::VertexAttribute {
                    offset: 48,
                    shader_location: 4,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }];

        let skeletal_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("PCSS Skeletal Shadow Depth Pipeline"),
            layout: Some(&skeletal_layout),
            vertex: wgpu::VertexState {
                module: &skeletal_shader,
                entry_point: Some("vs_main"),
                buffers: &skeletal_vb,
                compilation_options: Default::default(),
            },
            fragment: None,
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        Self {
            static_pipeline,
            skeletal_pipeline,
            bind_group_layout,
        }
    }

    /// Encodes the unified 2048x2048 PCSS spotlight shadow depth pass for both static level
    /// geometry and animated skeletal characters.
    pub fn encode_pass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        shadow_depth_view: &wgpu::TextureView,
        shadow_bind_group: &wgpu::BindGroup,
        gpu_map: Option<&GpuMap>,
        skeletal_meshes: &[SkeletalDrawItem],
        vis_filter: Option<&sh2_core::events::stageparts::MapPartVisibilityFilter>,
    ) {
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("PCSS Spotlight Shadow Depth Pass"),
            color_attachments: &[],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: shadow_depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        if let Some(map) = gpu_map {
            rpass.set_pipeline(&self.static_pipeline);
            rpass.set_bind_group(0, shadow_bind_group, &[]);
            for batch in &map.batches {
                if batch.group != BatchGroup::Opaque
                    || !map.is_batch_visible_with(batch, vis_filter)
                {
                    continue;
                }
                rpass.set_vertex_buffer(0, batch.vertex_buffer.slice(..));
                rpass.set_index_buffer(batch.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                rpass.draw_indexed(0..batch.num_indices, 0, 0..1);
            }
        }

        if !skeletal_meshes.is_empty() {
            rpass.set_pipeline(&self.skeletal_pipeline);
            rpass.set_bind_group(0, shadow_bind_group, &[]);
            for (mesh, _, _) in skeletal_meshes {
                rpass.set_bind_group(1, &mesh.bone_bind_group, &[]);
                rpass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                rpass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                for prim in &mesh.primitives {
                    if prim.is_transparent || prim.index_count == 0 {
                        continue;
                    }
                    let start = prim.first_index;
                    let end = start + prim.index_count;
                    rpass.draw_indexed(start..end, 0, 0..1);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pcss_uniforms_alignment() {
        assert_eq!(std::mem::size_of::<PcssShadowUniforms>(), 80);
        assert_eq!(std::mem::align_of::<PcssShadowUniforms>(), 4);
    }
}
