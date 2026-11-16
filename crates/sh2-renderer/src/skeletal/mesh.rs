//! GPU-resident skeletal mesh buffers, bone skinning uniforms, and sub-primitive draw batches (`SH2-37`).

use std::num::NonZeroU64;
use std::ops::Range;

use bytemuck::{Pod, Zeroable};
use sh2_assets::formats::mdls::ModelFile;
use sh2_core::PlayerMeshVisibility;
use wgpu::util::DeviceExt;

use super::body_region::{cranial_bone_mask, partition_first_person_triangles};
use super::material::{
    is_default_active_pose, pack_aligned_material_buffer, PoseVariantPreset,
    SkeletalMaterialUniforms,
};

/// Maximum number of bones supported in a single skeletal mesh uniform buffer.
/// Silent Hill 2 character models have up to 89 bones (e.g., Laura `lau.mdl` has 89 bones).
pub const MAX_SKELETON_BONES: usize = 128;

/// Per-vertex layout for skeletal character models undergoing GPU Linear Blend Skinning (LBS).
/// Total stride: 72 bytes (16-byte aligned attributes).
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct SkeletalVertex {
    /// Model bind-space coordinate position $(X, Y, Z)$ in PS2 world millimeters.
    pub position: [f32; 3],
    /// Padding for 16-byte alignment.
    pub _pad0: f32,
    /// Outward surface normal vector $(N_x, N_y, N_z)$ in model bind space.
    pub normal: [f32; 3],
    /// Padding for 16-byte alignment.
    pub _pad1: f32,
    /// Normalized texture mapping coordinates $(U, V)$.
    pub uv: [f32; 2],
    /// 4 skeleton bone indices influencing this vertex.
    pub bone_indices: [u32; 4],
    /// 4 bone blend weights corresponding to `bone_indices` (normalized to sum to `1.0`).
    pub weights: [f32; 4],
}

impl SkeletalVertex {
    /// Vertex buffer layout descriptor used in the WGSL skeletal mesh render pipeline.
    pub const fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<SkeletalVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: 16,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: 32,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: 40,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Uint32x4,
                },
                wgpu::VertexAttribute {
                    offset: 56,
                    shader_location: 4,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

/// Uniform structure storing current skinning transform matrices for all bones.
///
/// Each matrix transforms vertices from model bind pose to the current animated pose
/// ($M_{skin} = M_{world} \cdot M_{initial}^{-1}$).
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct SkeletonBoneUniforms {
    /// Column-major 4x4 bone skinning matrices uploaded to `@group(1) @binding(0)`.
    pub bone_matrices: [[[f32; 4]; 4]; MAX_SKELETON_BONES],
}

impl Default for SkeletonBoneUniforms {
    fn default() -> Self {
        let mut id = [[0.0; 4]; 4];
        id[0][0] = 1.0;
        id[1][1] = 1.0;
        id[2][2] = 1.0;
        id[3][3] = 1.0;

        Self {
            bone_matrices: [id; MAX_SKELETON_BONES],
        }
    }
}

impl SkeletonBoneUniforms {
    /// Builds a bone uniform structure from evaluated row-major CPU skinning matrices,
    /// transposing to column-major WGSL layout.
    pub fn from_matrices(matrices: &[[[f32; 4]; 4]]) -> Self {
        let mut s = Self::default();
        let count = matrices.len().min(MAX_SKELETON_BONES);
        for (i, m) in matrices.iter().take(count).enumerate() {
            #[allow(clippy::needless_range_loop)]
            for r in 0..4 {
                for c in 0..4 {
                    s.bone_matrices[i][c][r] = m[r][c];
                }
            }
        }
        s
    }
}

/// GPU-resident sub-primitive draw batch for a skeletal character model.
///
/// Indices are laid out `[below-neck triangles | cranial triangles]` so First-Person rendering can
/// draw the leading `body_index_count` indices and skip only the head/face/hair (`SH2-93`).
#[derive(Debug, Clone)]
pub struct GpuSkeletalPrimitive {
    /// Starting index in [`GpuSkeletalMesh::index_buffer`].
    pub first_index: u32,
    /// Total triangle index count (`body_index_count + cranial_index_count`).
    pub index_count: u32,
    /// Leading index count (`<= index_count`) of below-the-neck triangles kept in `HideHeadOnly`.
    pub body_index_count: u32,
    /// Dense slot index into the character's diffuse texture bind-group slice (or raw texture ID before remapping).
    pub texture_index: u16,
    /// Whether this submesh belongs to the transparent hair/eyelash stream (`tprim`).
    pub is_transparent: bool,
    /// Whether single-sided backface culling is enabled (`ModelPrimitive::backface_culling != 0`).
    pub backface_culling: bool,
    /// PS2/PC material shader code (`1` = unlit, `2`/`3` = Lambertian, `4` = specular + gloss mask).
    pub material_type: u8,
    /// Equipment / hand-pose variant identifier (`ModelPrimitive::pose_index`).
    pub pose_index: u8,
    /// Whether this primitive is currently active and should be drawn.
    pub is_active: bool,
    /// Dynamic byte offset into [`GpuSkeletalMesh::material_buffer`] for `@group(3) @binding(0)`.
    pub material_dynamic_offset: u32,
}

impl GpuSkeletalPrimitive {
    /// Returns the index range to draw for `visibility`, or `None` if nothing should be drawn.
    ///
    /// Planar mirror duplicates (`is_mirror_duplicate`, `er01.map`) always draw the full primitive
    /// including head and hair; the foreground `HideHeadOnly` player drops only cranial triangles (`SH2-93`).
    #[inline]
    pub fn draw_range(
        &self,
        visibility: PlayerMeshVisibility,
        is_mirror_duplicate: bool,
    ) -> Option<Range<u32>> {
        if !self.is_active {
            return None;
        }
        let count = match (visibility, is_mirror_duplicate) {
            (PlayerMeshVisibility::FullBody, _) | (_, true) => self.index_count,
            (PlayerMeshVisibility::HideHeadOnly, false) => self.body_index_count,
        };
        (count > 0).then(|| self.first_index..self.first_index + count)
    }
}

/// GPU-resident skeletal character mesh containing vertex, index, bone, and material uniform buffers.
pub struct GpuSkeletalMesh {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub bone_buffer: wgpu::Buffer,
    pub bone_bind_group: wgpu::BindGroup,
    /// Secondary bone uniform buffer & bind group used when rendering a planar mirror reflection
    /// of this skeletal mesh in the same render pass (`er01.map` Opening Restroom mirror at `X = -20000.0`).
    pub mirror_bone_buffer: wgpu::Buffer,
    pub mirror_bone_bind_group: wgpu::BindGroup,
    /// Aligned dynamic uniform buffer holding [`SkeletalMaterialUniforms`] for every primitive.
    pub material_buffer: wgpu::Buffer,
    /// Dynamic-offset bind group (`@group(3)`) bound per primitive via `material_dynamic_offset`.
    pub material_bind_group: wgpu::BindGroup,
    pub primitives: Vec<GpuSkeletalPrimitive>,
    pub total_indices: u32,
    /// Foreground submesh visibility directive (`FullBody` vs `HideHeadOnly` in First-Person) (`SH2-93`).
    pub player_visibility: PlayerMeshVisibility,
}

impl GpuSkeletalMesh {
    /// Compiles an ingested [`ModelFile`] into a GPU-resident skeletal mesh.
    ///
    /// Configures default equipment/pose visibility matching PS2 `SLUS_202.28` (`is_default_active_pose`),
    /// enabling James's breast-pocket flashlight (`pose_index == 21`) alongside base body (`0`) and
    /// default relaxed hands (`3`, `14`/`6`) while hiding inactive hand/weapon variants.
    pub fn from_model(
        device: &wgpu::Device,
        bone_layout: &wgpu::BindGroupLayout,
        material_layout: &wgpu::BindGroupLayout,
        model: &ModelFile,
    ) -> Self {
        let v_count = model.vertices.positions.len();
        let mut skeletal_vertices = Vec::with_capacity(v_count);

        for i in 0..v_count {
            let bones = model.vertices.bone_indices[i];
            skeletal_vertices.push(SkeletalVertex {
                position: model.vertices.positions[i],
                _pad0: 0.0,
                normal: model.vertices.normals[i],
                _pad1: 0.0,
                uv: model.vertices.uvs[i],
                bone_indices: [
                    bones[0] as u32,
                    bones[1] as u32,
                    bones[2] as u32,
                    bones[3] as u32,
                ],
                weights: model.vertices.weights[i],
            });
        }

        let min_align = device.limits().min_uniform_buffer_offset_alignment;
        let (material_bytes, material_offsets) =
            pack_aligned_material_buffer(&model.primitives, min_align);

        let mut indices: Vec<u32> = Vec::new();
        let mut prim_batches = Vec::with_capacity(model.primitives.len());
        let cranial_mask = cranial_bone_mask(model);

        for (k, pr) in model.primitives.iter().enumerate() {
            let tris = model.primitive_triangles(k);
            let (body_tris, cranial_tris) =
                partition_first_person_triangles(model, &tris, &cranial_mask);
            let first_index = indices.len() as u32;
            indices.extend(body_tris.iter().flatten().map(|&i| i as u32));
            let body_index_count = (indices.len() as u32) - first_index;
            indices.extend(cranial_tris.iter().flatten().map(|&i| i as u32));
            let index_count = (indices.len() as u32) - first_index;

            let is_active = is_default_active_pose(model, pr.pose_index);

            prim_batches.push(GpuSkeletalPrimitive {
                first_index,
                index_count,
                body_index_count,
                texture_index: pr.texture_index,
                is_transparent: pr.is_transparent,
                backface_culling: pr.backface_culling != 0,
                material_type: pr.material_type,
                pose_index: pr.pose_index,
                is_active,
                material_dynamic_offset: material_offsets.get(k).copied().unwrap_or(0),
            });
        }

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Skeletal Mesh Vertex Buffer"),
            contents: bytemuck::cast_slice(&skeletal_vertices),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Skeletal Mesh Index Buffer"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let bone_uniforms = SkeletonBoneUniforms::default();
        let bone_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Skeletal Mesh Bone Uniform Buffer"),
            contents: bytemuck::bytes_of(&bone_uniforms),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bone_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Skeletal Mesh Bone Bind Group"),
            layout: bone_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: bone_buffer.as_entire_binding(),
            }],
        });

        let mirror_bone_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Skeletal Mesh Mirror Bone Uniform Buffer"),
            contents: bytemuck::bytes_of(&bone_uniforms),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let mirror_bone_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Skeletal Mesh Mirror Bone Bind Group"),
            layout: bone_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: mirror_bone_buffer.as_entire_binding(),
            }],
        });

        let material_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Skeletal Mesh Material Dynamic Uniform Buffer"),
            contents: &material_bytes,
            usage: wgpu::BufferUsages::UNIFORM,
        });

        let mat_binding_size =
            NonZeroU64::new(std::mem::size_of::<SkeletalMaterialUniforms>() as u64);
        let material_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Skeletal Mesh Material Dynamic Bind Group"),
            layout: material_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &material_buffer,
                    offset: 0,
                    size: mat_binding_size,
                }),
            }],
        });

        let total_indices = indices.len() as u32;

        Self {
            vertex_buffer,
            index_buffer,
            bone_buffer,
            bone_bind_group,
            mirror_bone_buffer,
            mirror_bone_bind_group,
            material_buffer,
            material_bind_group,
            primitives: prim_batches,
            total_indices,
            player_visibility: PlayerMeshVisibility::FullBody,
        }
    }

    /// Updates foreground submesh visibility (`FullBody` vs `HideHeadOnly` in First-Person) (`SH2-93`).
    pub fn set_player_visibility(&mut self, visibility: PlayerMeshVisibility) {
        self.player_visibility = visibility;
    }

    /// Uploads newly evaluated skinning matrices to the GPU bone uniform buffer.
    pub fn update_bone_matrices(&self, queue: &wgpu::Queue, matrices: &[[[f32; 4]; 4]]) {
        let uniforms = SkeletonBoneUniforms::from_matrices(matrices);
        queue.write_buffer(&self.bone_buffer, 0, bytemuck::bytes_of(&uniforms));
    }

    /// Uploads planar-reflected skinning matrices to the secondary mirror GPU bone uniform buffer.
    pub fn update_mirror_bone_matrices(&self, queue: &wgpu::Queue, matrices: &[[[f32; 4]; 4]]) {
        let uniforms = SkeletonBoneUniforms::from_matrices(matrices);
        queue.write_buffer(&self.mirror_bone_buffer, 0, bytemuck::bytes_of(&uniforms));
    }

    /// Uploads morphed vertex positions (from `.cls` facial cluster weights via
    /// [`ModelFile::apply_cluster_weights_into`]) into the GPU skeletal vertex buffer
    /// using the caller-provided `scratch_vertices` buffer with zero heap allocations.
    pub fn update_morphed_positions(
        &self,
        queue: &wgpu::Queue,
        model: &ModelFile,
        morphed_positions: &[[f32; 3]],
        scratch_vertices: &mut Vec<SkeletalVertex>,
    ) {
        let v_count = model.vertices.positions.len().min(morphed_positions.len());
        scratch_vertices.clear();
        if scratch_vertices.capacity() < v_count {
            scratch_vertices.reserve(v_count - scratch_vertices.capacity());
        }

        for (i, &pos) in morphed_positions.iter().take(v_count).enumerate() {
            let bones = model.vertices.bone_indices[i];
            scratch_vertices.push(SkeletalVertex {
                position: pos,
                _pad0: 0.0,
                normal: model.vertices.normals[i],
                _pad1: 0.0,
                uv: model.vertices.uvs[i],
                bone_indices: [
                    bones[0] as u32,
                    bones[1] as u32,
                    bones[2] as u32,
                    bones[3] as u32,
                ],
                weights: model.vertices.weights[i],
            });
        }

        queue.write_buffer(
            &self.vertex_buffer,
            0,
            bytemuck::cast_slice(scratch_vertices.as_slice()),
        );
    }

    /// Applies a complete [`PoseVariantPreset`], enabling all primitives whose `pose_index`
    /// appears in `preset.active_poses` and deactivating all other pose variants.
    pub fn apply_pose_preset(&mut self, preset: &PoseVariantPreset) {
        for prim in &mut self.primitives {
            prim.is_active = preset.active_poses.contains(&prim.pose_index);
        }
    }

    /// Sets the active state for a given pose variation index.
    pub fn set_pose_active(&mut self, pose_index: u8, active: bool) {
        for prim in &mut self.primitives {
            if prim.pose_index == pose_index {
                prim.is_active = active;
            }
        }
    }

    /// Sets active right hand pose (`3..=13` in `hll_jms.mdl`) and deactivates other right hand variants.
    pub fn set_right_hand_pose(&mut self, pose_index: u8) {
        for prim in &mut self.primitives {
            if (3..=13).contains(&prim.pose_index) {
                prim.is_active = prim.pose_index == pose_index;
            }
        }
    }

    /// Sets active left hand pose (`14..=20` in `hll_jms.mdl`) and deactivates other left hand variants.
    pub fn set_left_hand_pose(&mut self, pose_index: u8) {
        for prim in &mut self.primitives {
            if (14..=20).contains(&prim.pose_index) {
                prim.is_active = prim.pose_index == pose_index;
            }
        }
    }
}
