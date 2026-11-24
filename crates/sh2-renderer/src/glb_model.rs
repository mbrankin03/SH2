//! GPU upload and keyframe skin-matrix evaluation for optional SH2:EE `.glb` replacement models
//! (`sh2e/model/ty3.glb`, `sh2e/model/mon.glb`, `sh2e/model/dmr.glb`).

use glam::{Mat4, Quat, Vec3};
use sh2_assets::formats::{GlbModel, GlbVertex};
use wgpu::util::DeviceExt;

use crate::skeletal::{SkeletalVertex, MAX_SKELETON_BONES};

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
}

/// GPU-resident `.glb` replacement model ready for animated rendering.
pub struct GlbGpuModel {
    /// Source parsed `.glb` CPU representation (retained for keyframe evaluation).
    pub source: GlbModel,
    /// Uploaded primitive draw sections.
    pub primitives: Vec<GlbGpuPrimitive>,
    /// Uploaded RGBA8 textures.
    pub textures: Vec<GlbGpuTexture>,
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

/// Evaluates world-space node transforms and skinning matrices (`[Mat4; MAX_SKELETON_BONES]`)
/// for `model` at `time_sec` (looping when `loop_anim` is `true`).
#[must_use]
pub fn evaluate_glb_skin_matrices(
    model: &GlbModel,
    skin_index: usize,
    time_sec: f32,
    loop_anim: bool,
) -> Vec<Mat4> {
    let locals = model.evaluate_node_locals(time_sec, loop_anim);
    let mut globals = vec![Mat4::IDENTITY; model.nodes.len()];

    fn visit(
        node_idx: usize,
        parent: Mat4,
        model: &GlbModel,
        locals: &[([f32; 3], [f32; 4], [f32; 3])],
        globals: &mut [Mat4],
    ) {
        let Some((t, r, s)) = locals.get(node_idx) else {
            return;
        };
        let local_mat = Mat4::from_scale_rotation_translation(
            Vec3::from_array(*s),
            Quat::from_xyzw(r[0], r[1], r[2], r[3]).normalize(),
            Vec3::from_array(*t),
        );
        let world_mat = parent * local_mat;
        if let Some(slot) = globals.get_mut(node_idx) {
            *slot = world_mat;
        }
        if let Some(node) = model.nodes.get(node_idx) {
            for &child in &node.children {
                visit(child, world_mat, model, locals, globals);
            }
        }
    }

    for &root in &model.root_nodes {
        visit(root, Mat4::IDENTITY, model, &locals, &mut globals);
    }

    let Some(skin) = model.skins.get(skin_index) else {
        return vec![Mat4::IDENTITY];
    };

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
    /// Uploads a parsed [`GlbModel`] to `device` and `queue`.
    pub fn from_glb(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        model: GlbModel,
        label: &str,
    ) -> Self {
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
                let gpu_tex = device.create_texture(&wgpu::TextureDescriptor {
                    label: Some(&format!("{label}_tex_{idx}")),
                    size,
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba8UnormSrgb,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                    view_formats: &[],
                });
                queue.write_texture(
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
                GlbGpuTexture {
                    texture: gpu_tex,
                    view,
                }
            })
            .collect();

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
                let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(&format!("{label}_n{node_idx}_p{prim_idx}_vb")),
                    contents: bytemuck::cast_slice(&verts),
                    usage: wgpu::BufferUsages::VERTEX,
                });
                let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
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
        }
    }
}
