//! Spotlight shadow depth pass (`512x512` `Depth32Float`) for volumetric flashlight
//! god-ray occlusion inside [`crate::fog::FogComputePipeline`] (`SH2-32`).

use bytemuck::{Pod, Zeroable};

use crate::gpu_map::GpuMap;
use crate::light::SpotlightUniforms;
use sh2_assets::formats::map::BatchGroup;

/// Shadow map resolution in texels (512x512 gives crisp volumetric shafts at <0.1 ms GPU cost).
pub const SPOTLIGHT_SHADOW_MAP_SIZE: u32 = 512;

/// Uniform buffer for the spotlight shadow-map depth pass (64 bytes, 16-byte aligned).
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct SpotlightShadowUniforms {
    /// Spotlight perspective view-projection matrix transforming world mm -> shadow clip space.
    pub light_view_proj: [[f32; 4]; 4],
}

impl Default for SpotlightShadowUniforms {
    fn default() -> Self {
        Self {
            light_view_proj: glam::Mat4::IDENTITY.to_cols_array_2d(),
        }
    }
}

/// Computes the spotlight view-projection matrix from [`SpotlightUniforms`] for shadow depth rendering
/// and volumetric god-ray occlusion tests.
#[must_use]
pub fn compute_spotlight_view_proj(spot: &SpotlightUniforms) -> glam::Mat4 {
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
    // Outer cone half-angle -> full FOV (clamped to valid perspective range 15..120 deg)
    let outer_cos = spot.color_outer[3].clamp(-0.5, 0.99);
    let half_angle = outer_cos.acos();
    let fov_y = (half_angle * 2.15).clamp(0.26, 2.1);
    let proj = glam::Mat4::perspective_rh(fov_y, 1.0, 50.0, 18000.0);
    proj * view
}

/// Depth-only render pipeline for the spotlight shadow map.
pub struct SpotlightShadowPipeline {
    pub pipeline: wgpu::RenderPipeline,
    pub bind_group_layout: wgpu::BindGroupLayout,
}

impl SpotlightShadowPipeline {
    const WGSL_SHADOW_SHADER: &'static str = r#"
struct ShadowUniforms {
    light_view_proj: mat4x4<f32>,
};

@group(0) @binding(0) var<uniform> shadow_u: ShadowUniforms;

struct VertexInput {
    @location(0) position: vec3<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> @builtin(position) vec4<f32> {
    return shadow_u.light_view_proj * vec4<f32>(in.position, 1.0);
}
"#;

    /// Creates the depth-only spotlight shadow pipeline matching [`crate::gpu_map::GpuMap`] vertex stride.
    #[must_use]
    pub fn new(device: &wgpu::Device, depth_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Spotlight Shadow Depth Shader"),
            source: wgpu::ShaderSource::Wgsl(Self::WGSL_SHADOW_SHADER.into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Spotlight Shadow Bind Group Layout"),
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

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Spotlight Shadow Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // Match MapVertex stride (48 bytes: pos[3], norm[3], uv[2], color[4]), reading only @location(0) pos
        let vertex_buffers = [wgpu::VertexBufferLayout {
            array_stride: 48,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32x3,
            }],
        }];

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Spotlight Shadow Depth Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &vertex_buffers,
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
            depth_stencil: Some(wgpu::DepthStencilState {
                format: depth_format,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState {
                    constant: 2,
                    slope_scale: 1.5,
                    clamp: 0.0,
                },
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        Self {
            pipeline,
            bind_group_layout,
        }
    }

    /// Encodes the spotlight shadow depth render pass for static opaque map geometry.
    pub fn encode_pass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        shadow_depth_view: &wgpu::TextureView,
        shadow_bind_group: &wgpu::BindGroup,
        gpu_map: Option<&GpuMap>,
    ) {
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Spotlight Shadow Depth Pass"),
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
            rpass.set_pipeline(&self.pipeline);
            rpass.set_bind_group(0, shadow_bind_group, &[]);
            for batch in &map.batches {
                if batch.group != BatchGroup::Opaque {
                    continue;
                }
                rpass.set_vertex_buffer(0, batch.vertex_buffer.slice(..));
                rpass.set_index_buffer(batch.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                rpass.draw_indexed(0..batch.num_indices, 0, 0..1);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spotlight_shadow_uniforms_alignment() {
        assert_eq!(std::mem::size_of::<SpotlightShadowUniforms>(), 64);
        assert_eq!(std::mem::align_of::<SpotlightShadowUniforms>(), 4);
    }

    #[test]
    fn compute_spotlight_view_proj_produces_finite_matrix() {
        let vp = compute_spotlight_view_proj(&SpotlightUniforms::default());
        assert!(vp.is_finite());
    }
}
