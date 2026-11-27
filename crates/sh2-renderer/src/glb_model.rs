//! GPU upload, skin-matrix evaluation, and wgpu render pass for optional SH2:EE `.glb`
//! replacement models (`sh2e/model/ty3.glb`, `sh2e/model/mon.glb`, `sh2e/model/dmr.glb`).

use std::sync::Arc;

use glam::{Mat4, Quat, Vec3};
use sh2_assets::formats::{GlbModel, GlbNodeLocalTransform, GlbVertex};
use wgpu::util::DeviceExt;

use crate::context::RenderContext;
use crate::skeletal::{
    SkeletalMaterialUniforms, SkeletalPassStage, SkeletalVertex, SkeletonBoneUniforms,
    MAX_SKELETON_BONES,
};

/// Single GPU-resident primitive section of a [`GlbGpuModel`].
pub struct GlbGpuPrimitive {
    /// Vertex buffer (`SkeletalVertex`).
    pub vertex_buffer: wgpu::Buffer,
    /// Triangle-list `u16` index buffer.
    pub index_buffer: wgpu::Buffer,
    /// Number of `u16` indices.
    pub index_count: u32,
    /// Optional index into [`GlbGpuModel::textures`].
    pub texture_index: Option<usize>,
    /// True when material specifies `alphaMode = "BLEND"` (e.g., Dog Room levers CRT/glass).
    pub alpha_blend: bool,
    /// Optional skin index in the source [`GlbModel`].
    pub skin_index: Option<usize>,
}

/// GPU-resident texture decoded from a `.glb` image buffer.
pub struct GlbGpuTexture {
    /// Underlying `wgpu::Texture` (`Rgba8UnormSrgb`).
    pub texture: wgpu::Texture,
    /// Default 2D texture view.
    pub view: wgpu::TextureView,
    /// Bind group matching `RenderContext::texture_bind_group_layout` (`@group(2)`).
    pub bind_group: Arc<wgpu::BindGroup>,
}

/// GPU-resident `.glb` replacement model ready for animated rendering via [`crate::SkeletalPipeline`].
pub struct GlbGpuModel {
    /// Source parsed `.glb` CPU representation (retained for keyframe evaluation).
    pub source: GlbModel,
    /// Uploaded primitive draw sections.
    pub primitives: Vec<GlbGpuPrimitive>,
    /// Uploaded RGBA8 textures and bind groups.
    pub textures: Vec<GlbGpuTexture>,
    /// Fallback 1x1 white bind group when a primitive has no texture.
    default_texture_bind_group: Arc<wgpu::BindGroup>,
    /// GPU uniform buffer for `@group(1)` bone skinning matrices.
    bone_buffer: wgpu::Buffer,
    /// Bind group for `@group(1)`.
    bone_bind_group: wgpu::BindGroup,
    /// Bind group for `@group(3)` material uniforms.
    material_bind_group: wgpu::BindGroup,
}

/// Converts a [`GlbVertex`] into [`SkeletalVertex`] for the shared skeletal pipeline.
#[must_use]
pub fn glb_vertex_to_skeletal(v: &GlbVertex) -> SkeletalVertex {
    SkeletalVertex {
        position: v.position,
        _pad0: 0.0,
        normal: v.normal,
        _pad1: 0.0,
        uv: v.uv,
        bone_indices: [
            u32::from(v.joints[0]),
            u32::from(v.joints[1]),
            u32::from(v.joints[2]),
            u32::from(v.joints[3]),
        ],
        weights: v.weights,
    }
}

/// Evaluates world-space node transforms and skinning matrices for `skin_index`
/// at `time_sec` (looping when `loop_anim` is `true`).
///
/// Includes a node cycle guard (`visited`) and returns an empty `Vec` if `skin_index` is invalid.
#[must_use]
pub fn evaluate_glb_skin_matrices(
    model: &GlbModel,
    skin_index: usize,
    time_sec: f32,
    loop_anim: bool,
) -> Vec<Mat4> {
    let Some(skin) = model.skins.get(skin_index) else {
        return Vec::new();
    };

    let locals = model.evaluate_node_locals(time_sec, loop_anim);
    let mut globals = vec![Mat4::IDENTITY; model.nodes.len()];
    let mut visited = vec![false; model.nodes.len()];

    fn visit(
        node_idx: usize,
        parent: Mat4,
        model: &GlbModel,
        locals: &[GlbNodeLocalTransform],
        globals: &mut [Mat4],
        visited: &mut [bool],
    ) {
        let Some(already) = visited.get_mut(node_idx) else {
            return;
        };
        if *already {
            return;
        }
        *already = true;

        let Some(local) = locals.get(node_idx) else {
            return;
        };
        let local_mat = match *local {
            GlbNodeLocalTransform::Matrix(m) => Mat4::from_cols_array(&m),
            GlbNodeLocalTransform::Trs {
                translation,
                rotation,
                scale,
            } => {
                let q = Quat::from_xyzw(rotation[0], rotation[1], rotation[2], rotation[3]);
                let q_norm = if q.length_squared() > f32::EPSILON {
                    q.normalize()
                } else {
                    Quat::IDENTITY
                };
                Mat4::from_scale_rotation_translation(
                    Vec3::from_array(scale),
                    q_norm,
                    Vec3::from_array(translation),
                )
            }
        };
        let world_mat = parent * local_mat;
        if let Some(slot) = globals.get_mut(node_idx) {
            *slot = world_mat;
        }
        if let Some(node) = model.nodes.get(node_idx) {
            for &child in &node.children {
                visit(child, world_mat, model, locals, globals, visited);
            }
        }
    }

    for &root in &model.root_nodes {
        visit(
            root,
            Mat4::IDENTITY,
            model,
            &locals,
            &mut globals,
            &mut visited,
        );
    }

    let count = skin.joints.len().min(MAX_SKELETON_BONES);
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let node_idx = skin.joints[i];
        let global = globals.get(node_idx).copied().unwrap_or(Mat4::IDENTITY);
        let ibm = skin
            .inverse_bind_matrices
            .get(i)
            .map(Mat4::from_cols_array)
            .unwrap_or(Mat4::IDENTITY);
        out.push(global * ibm);
    }
    out
}

impl GlbGpuModel {
    /// Uploads a parsed [`GlbModel`] using `ctx` so it can be drawn directly with [`crate::SkeletalPipeline`].
    pub fn from_glb(ctx: &RenderContext, model: GlbModel, label: &str) -> Self {
        let textures = model
            .textures
            .iter()
            .enumerate()
            .map(|(idx, tex)| {
                let size = wgpu::Extent3d {
                    width: tex.width.max(1),
                    height: tex.height.max(1),
                    depth_or_array_layers: 1,
                };
                let gpu_tex = ctx.device.create_texture(&wgpu::TextureDescriptor {
                    label: Some(&format!("{label}_tex_{idx}")),
                    size,
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba8UnormSrgb,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                    view_formats: &[],
                });
                ctx.queue.write_texture(
                    wgpu::ImageCopyTexture {
                        texture: &gpu_tex,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    &tex.rgba,
                    wgpu::ImageDataLayout {
                        offset: 0,
                        bytes_per_row: Some(4 * tex.width.max(1)),
                        rows_per_image: Some(tex.height.max(1)),
                    },
                    size,
                );
                let view = gpu_tex.create_view(&wgpu::TextureViewDescriptor::default());
                let bind_group = ctx
                    .create_texture_bind_group_from_view(&view, Some(&format!("{label}_bg_{idx}")));
                GlbGpuTexture {
                    texture: gpu_tex,
                    view,
                    bind_group,
                }
            })
            .collect();

        let default_tex = ctx.device.create_texture(&wgpu::TextureDescriptor {
            label: Some(&format!("{label}_white_fallback")),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        ctx.queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &default_tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &[255, 255, 255, 255],
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        let default_view = default_tex.create_view(&wgpu::TextureViewDescriptor::default());
        let default_texture_bind_group = ctx
            .create_texture_bind_group_from_view(&default_view, Some(&format!("{label}_white_bg")));

        let bone_uniforms = SkeletonBoneUniforms::default();
        let bone_buffer = ctx
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("{label}_bones")),
                contents: bytemuck::bytes_of(&bone_uniforms),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });
        let bone_bind_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(&format!("{label}_bone_bg")),
            layout: &ctx.skeletal_pipeline.bone_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: bone_buffer.as_entire_binding(),
            }],
        });

        let mat_uniform = SkeletalMaterialUniforms::default();
        let mat_buffer = ctx
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("{label}_mat")),
                contents: bytemuck::bytes_of(&mat_uniform),
                usage: wgpu::BufferUsages::UNIFORM,
            });
        let material_bind_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(&format!("{label}_mat_bg")),
            layout: &ctx.skeletal_pipeline.material_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &mat_buffer,
                    offset: 0,
                    size: std::num::NonZeroU64::new(
                        std::mem::size_of::<SkeletalMaterialUniforms>() as u64,
                    ),
                }),
            }],
        });

        let mut primitives = Vec::new();
        for (node_idx, node) in model.nodes.iter().enumerate() {
            let Some(mesh_idx) = node.mesh_index else {
                continue;
            };
            let Some(mesh) = model.meshes.get(mesh_idx) else {
                continue;
            };
            for (prim_idx, prim) in mesh.primitives.iter().enumerate() {
                if prim.vertices.is_empty() || prim.indices.is_empty() {
                    continue;
                }
                let verts: Vec<SkeletalVertex> =
                    prim.vertices.iter().map(glb_vertex_to_skeletal).collect();
                let vertex_buffer =
                    ctx.device
                        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some(&format!("{label}_n{node_idx}_p{prim_idx}_vb")),
                            contents: bytemuck::cast_slice(&verts),
                            usage: wgpu::BufferUsages::VERTEX,
                        });
                let index_buffer =
                    ctx.device
                        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some(&format!("{label}_n{node_idx}_p{prim_idx}_ib")),
                            contents: bytemuck::cast_slice(&prim.indices),
                            usage: wgpu::BufferUsages::INDEX,
                        });
                primitives.push(GlbGpuPrimitive {
                    vertex_buffer,
                    index_buffer,
                    index_count: prim.indices.len() as u32,
                    texture_index: prim.texture_index,
                    alpha_blend: prim.alpha_blend,
                    skin_index: node.skin_index,
                });
            }
        }

        Self {
            source: model,
            primitives,
            textures,
            default_texture_bind_group,
            bone_buffer,
            bone_bind_group,
            material_bind_group,
        }
    }

    /// Evaluates keyframe skinning (or rigid world transform) at `time_sec` and uploads bone
    /// matrices to `@group(1)`.
    pub fn update_skin_matrices(
        &self,
        queue: &wgpu::Queue,
        world_transform: Mat4,
        time_sec: f32,
        loop_anim: bool,
    ) {
        let mut uniforms = SkeletonBoneUniforms::default();
        let skin_idx = self
            .primitives
            .iter()
            .find_map(|p| p.skin_index)
            .unwrap_or(0);
        let mats = evaluate_glb_skin_matrices(&self.source, skin_idx, time_sec, loop_anim);
        if mats.is_empty() {
            uniforms.bone_matrices[0] = world_transform.to_cols_array_2d();
        } else {
            for (i, mat) in mats.iter().take(MAX_SKELETON_BONES).enumerate() {
                uniforms.bone_matrices[i] = (world_transform * *mat).to_cols_array_2d();
            }
        }
        queue.write_buffer(&self.bone_buffer, 0, bytemuck::bytes_of(&uniforms));
    }

    /// Draws either opaque (`alpha_pass == false`) or alpha-blended (`alpha_pass == true`)
    /// primitives into an active HDR render pass using [`crate::SkeletalPipeline`].
    pub fn draw<'a>(
        &'a self,
        rpass: &mut wgpu::RenderPass<'a>,
        ctx: &'a RenderContext,
        common_bind_group: &'a wgpu::BindGroup,
        alpha_pass: bool,
    ) {
        let stage = if alpha_pass {
            SkeletalPassStage::TransparentFringe
        } else {
            SkeletalPassStage::Opaque
        };
        let pipeline = ctx.skeletal_pipeline.select_pipeline(stage, true, false);
        rpass.set_pipeline(pipeline);
        rpass.set_bind_group(0, common_bind_group, &[]);
        rpass.set_bind_group(1, &self.bone_bind_group, &[]);
        rpass.set_bind_group(3, &self.material_bind_group, &[0]);

        for prim in &self.primitives {
            if prim.alpha_blend != alpha_pass {
                continue;
            }
            let tex_bg = prim
                .texture_index
                .and_then(|idx| self.textures.get(idx))
                .map(|t| &t.bind_group)
                .unwrap_or(&self.default_texture_bind_group);
            rpass.set_bind_group(2, tex_bg.as_ref(), &[]);
            rpass.set_vertex_buffer(0, prim.vertex_buffer.slice(..));
            rpass.set_index_buffer(prim.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            rpass.draw_indexed(0..prim.index_count, 0, 0..1);
        }
    }
}

impl RenderContext {
    /// Encodes active optional SH2:EE `.glb` replacement models (`ty3.glb`, `mon.glb`) into an
    /// existing `CommandEncoder` targeting the current HDR color + depth target (`SH2-182`).
    pub fn encode_glb_overlays(&self, encoder: &mut wgpu::CommandEncoder, models: &[&GlbGpuModel]) {
        if models.is_empty() {
            return;
        }
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("SH2EE GLB Overlay Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.hdr_color_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        for model in models {
            model.draw(&mut rpass, self, &self.geometry_bind_group, false);
        }
        for model in models {
            model.draw(&mut rpass, self, &self.geometry_bind_group, true);
        }
    }

    /// Renders active optional SH2:EE `.glb` replacement models (`ty3.glb`, `mon.glb`) into the
    /// current HDR color + depth target (opaque pass followed by alpha-blended pass).
    pub fn render_glb_overlays(&self, models: &[&GlbGpuModel]) {
        if models.is_empty() {
            return;
        }
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("SH2EE GLB Overlay Encoder"),
            });
        self.encode_glb_overlays(&mut encoder, models);
        self.queue.submit(Some(encoder.finish()));
    }
}
