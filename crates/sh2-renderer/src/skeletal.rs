//! Skeletal mesh representation and GPU Linear Blend Skinning (LBS) pipeline.

use bytemuck::{Pod, Zeroable};
use sh2_assets::formats::mdls::ModelFile;
use wgpu::util::DeviceExt;

/// Maximum number of bones supported in a single skeletal mesh uniform/storage buffer.
/// Silent Hill 2 character models have up to ~71-80 bones (e.g. James has 71 bones).
pub const MAX_SKELETON_BONES: usize = 128;

/// Per-vertex layout for skeletal character models undergoing GPU Linear Blend Skinning (LBS).
/// Total stride: 48 bytes (aligned to 16 bytes for GPU efficiency).
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct SkeletalVertex {
    /// Local / bind model-space coordinate position $(X, Y, Z)$.
    pub position: [f32; 3],
    /// Padding for 16-byte alignment.
    pub _pad0: f32,
    /// Surface normal vector $(N_x, N_y, N_z)$.
    pub normal: [f32; 3],
    /// Padding for 16-byte alignment.
    pub _pad1: f32,
    /// Texture mapping coordinates $(U, V)$.
    pub uv: [f32; 2],
    /// 4 bone indices influencing this vertex (packed as u32s for shader index lookups).
    pub bone_indices: [u32; 4],
    /// 4 bone blend weights corresponding to `bone_indices` (normalized to sum to 1.0).
    pub weights: [f32; 4],
}

impl SkeletalVertex {
    /// Vertex buffer layout descriptor used in the WGSL skeletal mesh render pipeline.
    pub const fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<SkeletalVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                // position: vec3<f32>
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // normal: vec3<f32>
                wgpu::VertexAttribute {
                    offset: 16,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // uv: vec2<f32>
                wgpu::VertexAttribute {
                    offset: 32,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // bone_indices: vec4<u32>
                wgpu::VertexAttribute {
                    offset: 40,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Uint32x4,
                },
                // weights: vec4<f32>
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
/// Each matrix is a row-major 4x4 matrix transforming vertices from model bind pose
/// to the current animated pose ($M_{skin} = M_{world} \cdot M_{initial}^{-1}$).
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct SkeletonBoneUniforms {
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
    /// Builds a bone uniform structure from evaluated skinning matrices.
    ///
    /// Transposes matrices from row-major CPU order ($M[row][col]$) to column-major WGSL order
    /// ($M[col][row]$) so that matrix multiplications in WGSL (`skin_matrix * local_pos`)
    /// correctly align with translation columns and rotation vectors.
    pub fn from_matrices(matrices: &[[[f32; 4]; 4]]) -> Self {
        let mut s = Self::default();
        let count = matrices.len().min(MAX_SKELETON_BONES);
        for (i, m) in matrices.iter().take(count).enumerate() {
            #[allow(clippy::needless_range_loop)]
            for r in 0..4 {
                for c in 0..4 {
                    s.bone_matrices[i][c][r] = m[r][c]; // Transpose row-major -> column-major
                }
            }
        }
        s
    }
}

/// GPU-resident sub-primitive draw batch for a skeletal character model.
#[derive(Debug, Clone)]
pub struct GpuSkeletalPrimitive {
    pub first_index: u32,
    pub index_count: u32,
    pub texture_index: u16,
    pub is_transparent: bool,
    /// Target pose identifier matching [`ModelPrimitive::pose_index`].
    pub pose_index: u8,
    /// Whether this primitive is currently active and should be drawn.
    pub is_active: bool,
}

/// GPU-resident skeletal character mesh containing vertex, index, and bone uniform buffers.
pub struct GpuSkeletalMesh {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub bone_buffer: wgpu::Buffer,
    pub bone_bind_group: wgpu::BindGroup,
    /// Secondary bone uniform buffer & bind group used when rendering a planar mirror reflection
    /// of this skeletal mesh in the same render pass (e.g., `er01.map` Opening Restroom mirror at `X = -20000.0`).
    pub mirror_bone_buffer: wgpu::Buffer,
    pub mirror_bone_bind_group: wgpu::BindGroup,
    pub primitives: Vec<GpuSkeletalPrimitive>,
    pub total_indices: u32,
}

impl GpuSkeletalMesh {
    /// Compiles an ingested [`ModelFile`] into a GPU-resident skeletal mesh.
    ///
    /// Primitives with variant poses (e.g. hand poses 3..20 in `hll_jms.mdl`) are configured
    /// with default visibility: base mesh (`pose_index == 0`), default right hand (`pose_index == 3`),
    /// and default left hand (`pose_index == 14`) are enabled by default; alternate pose variations
    /// are initialized inactive to prevent overlapping/distorted mesh deformations.
    pub fn from_model(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        model: &ModelFile,
    ) -> Self {
        let v_count = model.vertices.positions.len();
        let mut skeletal_vertices = Vec::with_capacity(v_count);

        for i in 0..v_count {
            let pos = model.vertices.positions[i];
            let nrm = model.vertices.normals[i];
            let uv = model.vertices.uvs[i];
            let bones = model.vertices.bone_indices[i];
            let weights = model.vertices.weights[i];

            skeletal_vertices.push(SkeletalVertex {
                position: pos,
                _pad0: 0.0,
                normal: nrm,
                _pad1: 0.0,
                uv,
                bone_indices: [
                    bones[0] as u32,
                    bones[1] as u32,
                    bones[2] as u32,
                    bones[3] as u32,
                ],
                weights,
            });
        }

        // Generate triangle index list across all primitives
        let mut indices: Vec<u32> = Vec::new();
        let mut prim_batches = Vec::with_capacity(model.primitives.len());

        // Check if this model contains multiple pose variations (e.g. low-LOD James hll_jms, Maria lxx_mar)
        let has_variant_poses = model.primitives.iter().any(|pr| pr.pose_index > 0);
        let has_pose_14 = model.primitives.iter().any(|pr| pr.pose_index == 14);
        let has_pose_6 = model.primitives.iter().any(|pr| pr.pose_index == 6);

        for (k, pr) in model.primitives.iter().enumerate() {
            let tris = model.primitive_triangles(k);
            let first_index = indices.len() as u32;
            for tri in tris {
                indices.push(tri[0] as u32);
                indices.push(tri[1] as u32);
                indices.push(tri[2] as u32);
            }
            let index_count = (indices.len() as u32) - first_index;

            // Default active state: if model uses pose variants, activate base mesh (0)
            // and default relaxed open hands:
            // - James (`hll_jms` / `lll_jms`): pose 3 (right hand) + pose 14 (left hand)
            // - Maria (`lxx_mar` / `rlxx_mar`): pose 3 (right hand) + pose 6 (left hand)
            // - Pyramid Head (`red`): pose 0 + pose 3
            let is_active = if has_variant_poses {
                pr.pose_index == 0
                    || pr.pose_index == 3
                    || (has_pose_14 && pr.pose_index == 14)
                    || (!has_pose_14 && has_pose_6 && pr.pose_index == 6)
            } else {
                true
            };

            prim_batches.push(GpuSkeletalPrimitive {
                first_index,
                index_count,
                texture_index: pr.texture_index,
                is_transparent: pr.is_transparent,
                pose_index: pr.pose_index,
                is_active,
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
            layout,
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
            layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: mirror_bone_buffer.as_entire_binding(),
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
            primitives: prim_batches,
            total_indices,
        }
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
            let nrm = model.vertices.normals[i];
            let uv = model.vertices.uvs[i];
            let bones = model.vertices.bone_indices[i];
            let weights = model.vertices.weights[i];

            scratch_vertices.push(SkeletalVertex {
                position: pos,
                _pad0: 0.0,
                normal: nrm,
                _pad1: 0.0,
                uv,
                bone_indices: [
                    bones[0] as u32,
                    bones[1] as u32,
                    bones[2] as u32,
                    bones[3] as u32,
                ],
                weights,
            });
        }

        queue.write_buffer(
            &self.vertex_buffer,
            0,
            bytemuck::cast_slice(scratch_vertices.as_slice()),
        );
    }

    /// Sets the active state for a given pose variation index.
    pub fn set_pose_active(&mut self, pose_index: u8, active: bool) {
        for prim in &mut self.primitives {
            if prim.pose_index == pose_index {
                prim.is_active = active;
            }
        }
    }

    /// Sets active right hand pose (3..13 in `hll_jms.mdl`) and deactivates other right hand variants.
    pub fn set_right_hand_pose(&mut self, pose_index: u8) {
        for prim in &mut self.primitives {
            if (3..=13).contains(&prim.pose_index) {
                prim.is_active = prim.pose_index == pose_index;
            }
        }
    }

    /// Sets active left hand pose (14..20 in `hll_jms.mdl`) and deactivates other left hand variants.
    pub fn set_left_hand_pose(&mut self, pose_index: u8) {
        for prim in &mut self.primitives {
            if (14..=20).contains(&prim.pose_index) {
                prim.is_active = prim.pose_index == pose_index;
            }
        }
    }
}

/// Rendering pipeline for skeletal character meshes with GPU Linear Blend Skinning (LBS).
pub struct SkeletalPipeline {
    pub pipeline: wgpu::RenderPipeline,
    pub transparent_pipeline: wgpu::RenderPipeline,
    pub bone_bind_group_layout: wgpu::BindGroupLayout,
}

impl SkeletalPipeline {
    pub const WGSL_SKINNING_SHADER: &'static str = r#"
struct CameraUniforms {
    view_proj: mat4x4<f32>,
    inv_proj: mat4x4<f32>,
    cam_pos: vec4<f32>,
};

struct SpotlightUniforms {
    position: vec4<f32>,
    direction_inner: vec4<f32>,
    color_outer: vec4<f32>,
    ambient_tint: vec4<f32>,
};

struct SkeletonBones {
    matrices: array<mat4x4<f32>, 128>,
};

@group(0) @binding(0) var<uniform> camera: CameraUniforms;
@group(0) @binding(1) var<uniform> spotlight: SpotlightUniforms;
@group(1) @binding(0) var<uniform> skeleton: SkeletonBones;
@group(2) @binding(0) var diffuse_texture: texture_2d<f32>;
@group(2) @binding(1) var diffuse_sampler: sampler;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) bone_indices: vec4<u32>,
    @location(4) weights: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) color: vec4<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    // 4-bone Linear Blend Skinning (LBS)
    let w = in.weights;
    let b = in.bone_indices;

    let m0 = skeleton.matrices[b.x];
    let m1 = skeleton.matrices[b.y];
    let m2 = skeleton.matrices[b.z];
    let m3 = skeleton.matrices[b.w];

    let skin_matrix = m0 * w.x + m1 * w.y + m2 * w.z + m3 * w.w;

    let local_pos = vec4<f32>(in.position, 1.0);
    let skinned_pos = skin_matrix * local_pos;
    let skinned_normal = normalize((skin_matrix * vec4<f32>(in.normal, 0.0)).xyz);

    var out: VertexOutput;
    out.world_pos = skinned_pos.xyz;
    out.clip_position = camera.view_proj * vec4<f32>(skinned_pos.xyz, 1.0);
    out.normal = skinned_normal;
    out.uv = in.uv;
    out.color = vec4<f32>(1.0, 1.0, 1.0, 1.0);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let n_len = length(in.normal);
    let norm = select(vec3<f32>(0.0, -1.0, 0.0), in.normal / n_len, n_len > 1e-4);

    // Sample diffuse texture unconditionally in uniform control flow before any early returns
    // so derivative evaluation (`textureSample`) satisfies strict WGSL uniformity rules across all backends.
    let tex_color = textureSample(diffuse_texture, diffuse_sampler, in.uv);

    // Inspect shading mode encoded in camera.cam_pos.w:
    // 0.0 = Textured (Spotlight + Ambient)
    // 1.0 = Untextured Clay (3-Point Studio Shading)
    // 2.0 = Surface Normals (RGB Vector Map)
    // 3.0 = Textured with 3-Point Studio Lighting (Key + Fill + Rim + Spotlight)
    let shade_mode = i32(round(camera.cam_pos.w));
    if shade_mode == 2 {
        let n_rgb = norm * 0.5 + vec3<f32>(0.5, 0.5, 0.5);
        return vec4<f32>(n_rgb, 1.0);
    }

    // Spotlight lighting evaluation
    let light_to_frag = in.world_pos - spotlight.position.xyz;
    let dist = length(light_to_frag);
    let l_dir = normalize(light_to_frag);

    let spot_dir = normalize(spotlight.direction_inner.xyz);
    let inner_cos = spotlight.direction_inner.w;
    let outer_cos = spotlight.color_outer.w;

    let current_cos = dot(l_dir, spot_dir);
    // Ignore any fragments behind the emitter plane (prevents self-illumination of wearer's head/face/eyes)
    let forward_gate = select(0.0, 1.0, current_cos > 0.0);
    let spot_cone = clamp((current_cos - outer_cos) / max(inner_cos - outer_cos, 0.0001), 0.0, 1.0) * forward_gate;

    let attenuation = 1.0 / max(dist * dist * 0.00005 + dist * 0.001 + 1.0, 1.0);
    let n_dot_l = max(dot(norm, -l_dir), 0.0);

    let ambient = vec3<f32>(0.04, 0.04, 0.045);
    let diffuse = spotlight.color_outer.xyz * (n_dot_l * spot_cone * attenuation);

    // 3-Point Studio Rig (Key + Warm Fill + Cool Rim) used in Model Viewer modes (1 and 3)
    let key_dir = normalize(vec3<f32>(0.45, -0.65, 0.60));
    let fill_dir = normalize(vec3<f32>(-0.65, -0.25, 0.70));
    let rim_dir = normalize(vec3<f32>(0.0, -0.40, -0.90));

    let k_diff = max(dot(norm, key_dir), 0.0) * vec3<f32>(0.78, 0.76, 0.72);
    let f_diff = max(dot(norm, fill_dir), 0.0) * vec3<f32>(0.28, 0.32, 0.38);
    let r_diff = pow(max(dot(norm, rim_dir), 0.0), 2.0) * vec3<f32>(0.45, 0.50, 0.60);
    let studio_light = vec3<f32>(0.16, 0.16, 0.18) + k_diff + f_diff + r_diff + diffuse * 0.65;

    if shade_mode == 1 {
        let clay_albedo = vec3<f32>(0.76, 0.74, 0.72);
        return vec4<f32>(clay_albedo * studio_light, 1.0);
    }

    if tex_color.a < 0.01 {
        discard;
    }
    let env_tint_rgb = select(vec3<f32>(1.0, 1.0, 1.0), spotlight.ambient_tint.xyz, spotlight.ambient_tint.w > 0.001);
    let env_ambient_scale = select(1.0, spotlight.ambient_tint.w, spotlight.ambient_tint.w > 0.001);
    let total_light = select((ambient * env_ambient_scale + diffuse) * env_tint_rgb, studio_light * env_tint_rgb, shade_mode == 3);
    let final_rgb = total_light * in.color.rgb * tex_color.rgb;
    return vec4<f32>(final_rgb, in.color.a * tex_color.a);
}
"#;

    pub fn new(
        device: &wgpu::Device,
        hdr_format: wgpu::TextureFormat,
        depth_format: wgpu::TextureFormat,
        common_bind_group_layout: &wgpu::BindGroupLayout,
        texture_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let bone_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Skeletal Bone Uniform Bind Group Layout"),
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
            label: Some("Skeletal Render Pipeline Layout"),
            bind_group_layouts: &[
                common_bind_group_layout,
                &bone_bind_group_layout,
                texture_bind_group_layout,
            ],
            push_constant_ranges: &[],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Skeletal WGSL Shader"),
            source: wgpu::ShaderSource::Wgsl(Self::WGSL_SKINNING_SHADER.into()),
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Skeletal Mesh Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[SkeletalVertex::desc()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: hdr_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None, // SH2 models use mixed/double-sided primitives
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: depth_format,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let transparent_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Skeletal Mesh Transparent Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[SkeletalVertex::desc()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: hdr_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
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
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        Self {
            pipeline,
            transparent_pipeline,
            bone_bind_group_layout,
        }
    }
}
