//! Geometry rendering pipeline for level meshes (.map) with Depth32Float & HDR (Rgba16Float).

use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct CameraUniforms {
    pub view_proj: [[f32; 4]; 4],
    pub inv_proj: [[f32; 4]; 4],
    /// Camera eye position `(x, y, z)` and debug shading mode in `w` (`0.0` = Textured, `1.0` = Clay, `2.0` = Normals, `3.0` = Studio 3-Point).
    pub cam_pos: [f32; 4],
    /// Inverse view-projection matrix for world-space ray reconstruction in volumetric compute passes (`SH2-32`).
    pub inv_view_proj: [[f32; 4]; 4],
    /// Optional world-space mirror clip plane `(nx, ny, nz, d)` (`dot(n, p) + d < 0 => discard`).
    /// All zeros disables clipping.
    pub clip_plane: [f32; 4],
}

impl Default for CameraUniforms {
    fn default() -> Self {
        let id = [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        Self {
            view_proj: id,
            inv_proj: id,
            cam_pos: [0.0, 0.0, 0.0, 1.0],
            inv_view_proj: id,
            clip_plane: [0.0; 4],
        }
    }
}

/// Authentic PS2/PC quantized 8-bit alpha-test cutoff (`alpha > 0` in `[0..=255]`):
/// - `SLUS_202.28` `shGs_InitDefaultRegsEnv` (`0x001909e0`) & `InitEnv1` (`0x002a9780`):
///   `TEST_1 = 0x5000d` (`ATE = 1`, `ATST = 6 [GREATER]`, `AREF = 0`, `AFAIL = 0 [KEEP]`).
/// - `sh2pc.exe` `FUN_005b4cc0` (`0x005b4cc0`, Opaque/Transparent `.map` pass) and
///   `0x005b1694` (`sh2pc_005a0000.c:7565-7569`, Decal pass):
///   `D3DRS_ALPHATESTENABLE = 1`, `D3DRS_ALPHAFUNC = 5 (D3DCMP_GREATER)`, `D3DRS_ALPHAREF = 0`.
pub const MAP_ALPHA_TEST_THRESHOLD: f32 = 0.5 / 255.0;

/// Verified hardware render-state contract for a `.map` [`sh2_assets::formats::map::BatchGroup`].
///
/// Derived 1:1 from `sh2pc.exe`:
/// - `D3DRS_CULLMODE (0x16) = 2 (D3DCULL_CW)` -> CCW front-face with hardware backface culling
///   across `Opaque`, `Transparent` (`FUN_005b4cc0` `0x005b4cc0`), and `Decal`
///   (`sh2pc_005a0000.c:7534,7547`).
/// - `D3DRS_ALPHATESTENABLE = 1`, `D3DCMP_GREATER`, `D3DRS_ALPHAREF = 0` across all three groups
///   (`FUN_005b4cc0` `0x005b4cc0` & `SLUS_202.28` `shGs_InitDefaultRegsEnv` `0x001909e0`).
/// - `MapMaterial.mode` (`FUN_005b2d40` `0x005b2d40` / `DAT_008f0338`) selects lighting/specular
///   shader mode only and never overrides `D3DRS_CULLMODE` or `D3DRS_ALPHATESTENABLE`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MapBatchRenderState {
    /// Hardware primitive cull mode (`Some(Face::Back)` = `D3DCULL_CW` with `FrontFace::Ccw`).
    pub cull_mode: Option<wgpu::Face>,
    /// Front-face winding (`FrontFace::Ccw`, matching `D3DCULL_CW` in `sh2pc.exe`).
    pub front_face: wgpu::FrontFace,
    /// Whether depth writes are enabled (`true` for `Opaque`, `false` for `Decal`/`Transparent`).
    pub depth_write_enabled: bool,
    /// Whether alpha blending is enabled (`false` for `Opaque`, `true` for `Decal`/`Transparent`).
    pub alpha_blending: bool,
    /// Quantized alpha-test discard threshold (`MAP_ALPHA_TEST_THRESHOLD` = `0.5 / 255.0`).
    pub alpha_test_threshold: f32,
}

/// Returns the 1:1 `sh2pc.exe` / `SLUS_202.28` hardware render state for `group`.
#[must_use]
pub const fn map_batch_render_state(
    group: sh2_assets::formats::map::BatchGroup,
) -> MapBatchRenderState {
    use sh2_assets::formats::map::BatchGroup;
    match group {
        BatchGroup::Opaque => MapBatchRenderState {
            cull_mode: Some(wgpu::Face::Back),
            front_face: wgpu::FrontFace::Ccw,
            depth_write_enabled: true,
            alpha_blending: false,
            alpha_test_threshold: MAP_ALPHA_TEST_THRESHOLD,
        },
        BatchGroup::Decal | BatchGroup::Transparent => MapBatchRenderState {
            cull_mode: Some(wgpu::Face::Back),
            front_face: wgpu::FrontFace::Ccw,
            depth_write_enabled: false,
            alpha_blending: true,
            alpha_test_threshold: MAP_ALPHA_TEST_THRESHOLD,
        },
    }
}

pub struct GeometryPipeline {
    /// Primary opaque + alpha-cutout level geometry pipeline (`depth_write_enabled: true`).
    pub pipeline: wgpu::RenderPipeline,
    /// Stenciled planar mirror reflected opaque pipeline (`FrontFace::Cw`, `Depth24PlusStencil8`, `stencil == 1`).
    pub mirror_reflected_pipeline: wgpu::RenderPipeline,
    /// Coplanar decal pipeline (`ALPHA_BLENDING`, `depth_write_enabled: false`, negative depth bias).
    pub decal_pipeline: wgpu::RenderPipeline,
    /// Semi-transparent glass/water/foliage fringe pipeline (`ALPHA_BLENDING`, `depth_write_enabled: false`).
    pub transparent_pipeline: wgpu::RenderPipeline,
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub texture_bind_group_layout: wgpu::BindGroupLayout,
}

impl GeometryPipeline {
    pub const WGSL_SHADER: &'static str = r#"
struct CameraUniforms {
    view_proj: mat4x4<f32>,
    inv_proj: mat4x4<f32>,
    cam_pos: vec4<f32>,
    inv_view_proj: mat4x4<f32>,
    clip_plane: vec4<f32>,
};

struct SpotlightUniforms {
    position: vec4<f32>,
    direction_inner: vec4<f32>,
    color_outer: vec4<f32>,
    ambient_tint: vec4<f32>,
    fill_and_range: vec4<f32>,
    parallel_dirs: array<vec4<f32>, 3>,
    parallel_colors: array<vec4<f32>, 3>,
    point_positions: array<vec4<f32>, 3>,
    point_colors: array<vec4<f32>, 3>,
    ps2_falloff_params: vec4<f32>,
    gun_and_dark_trim: vec4<f32>,
    reflection_dir: vec4<f32>,
    reflection_color: vec4<f32>,
    map_ambient_c19: vec4<f32>,
    map_base_color_c20: vec4<f32>,
    map_base_ambient_c21: vec4<f32>,
    map_vertex_ambient_c22: vec4<f32>,
    char_global_ambient: vec4<f32>,
};

@group(0) @binding(0) var<uniform> camera: CameraUniforms;
@group(0) @binding(1) var<uniform> spotlight: SpotlightUniforms;
@group(0) @binding(2) var<uniform> pcss_u: PcssShadowUniforms;
@group(0) @binding(3) var pcss_shadow_map: texture_depth_2d;
@group(0) @binding(4) var specular_sphere_tex: texture_2d<f32>;
@group(0) @binding(5) var specular_sphere_sampler: sampler;

@group(1) @binding(0) var diffuse_texture: texture_2d<f32>;
@group(1) @binding(1) var diffuse_sampler: sampler;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) color: vec4<f32>,
    @location(4) shader_mode: u32,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) color: vec4<f32>,
    @location(4) @interpolate(flat) shader_mode: u32,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.world_pos = in.position;
    out.clip_position = camera.view_proj * vec4<f32>(in.position, 1.0);
    out.normal = in.normal;
    out.uv = in.uv;
    out.color = in.color;
    out.shader_mode = in.shader_mode;
    return out;
}

fn shade_geometry_fragment(in: VertexOutput) -> vec4<f32> {
    // Sample diffuse texture in uniform control flow before alpha-test discard.
    // Exact PS2/PC alpha-test (`SLUS_202.28` `shGs_InitDefaultRegsEnv` `0x001909e0` & `InitEnv1`
    // GIF packet `0x002a9780`: `TEST_1 = 0x5000d` -> `ATE = 1`, `ATST = 6 (GREATER)`, `AREF = 0`,
    // `AFAIL = 0 (KEEP)`; `sh2pc.exe` `FUN_005b4cc0` `0x005b4cc0` & `FUN_00503800` `0x00503800`:
    // `D3DRS_ALPHATESTENABLE = 1`, `D3DRS_ALPHAFUNC = 5 (D3DCMP_GREATER)`, `D3DRS_ALPHAREF = 0`):
    // discard quantized 8-bit `alpha == 0`. Backface culling is performed in hardware via
    // `PrimitiveState { front_face: Ccw, cull_mode: Some(Face::Back) }` (`D3DRS_CULLMODE = 2`).
    let tex_color = textureSample(diffuse_texture, diffuse_sampler, in.uv);
    let out_alpha = in.color.a * tex_color.a;
    if (out_alpha < (0.5 / 255.0)) {
        discard;
    }
    if (dot(camera.clip_plane.xyz, camera.clip_plane.xyz) > 0.0
        && dot(vec4<f32>(in.world_pos, 1.0), camera.clip_plane) < 0.0) {
        discard;
    }

    // Ambient floor from `SLUS_202.28` `DrawEnvData::base_ambient` (`+0x1c0` -> `DAT_002ad900` = `0x3d3851ec` = `0.045f`
    // in `SetupDrawEnvData` `0x00251370`, `sh2gfw_setparal3LGT_blk` `0x0017d7f0`, `sh2gfw_setsp1para2LGT_blk` `0x0017ddd0`).
    // Pre-lit skybox/vista geometry (`stride == 0x14`, `has_normal == false`) uses `0.045 * 4.0 = 0.18`
    // (`PS2_VU1_WORLD_LIGHT_SCALE = 0x40800000 = 4.0f` in `0x0017d7f0` / `0x0017ddd0`).
    let n_len = length(in.normal);
    let has_normal = n_len > 1e-4;
    let base_ambient = select(vec3<f32>(0.18, 0.18, 0.18), vec3<f32>(0.045, 0.045, 0.045), has_normal);

    // Dynamic Spotlight calculation matching dbg_spotL_ctl
    let light_to_frag = in.world_pos - spotlight.position.xyz;
    let dist = length(light_to_frag);
    let l_dir = normalize(light_to_frag);
    let spot_dir = normalize(spotlight.direction_inner.xyz);
    let cos_theta = dot(l_dir, spot_dir);

    let inner_cos = spotlight.direction_inner.w;
    let outer_cos = spotlight.color_outer.w;
    let spot_intensity = select(
        clamp((cos_theta - outer_cos) / max(inner_cos - outer_cos, 0.0001), 0.0, 1.0),
        1.0,
        inner_cos <= -0.99 && outer_cos <= -0.99,
    );

    // Exact PS2 distance attenuation (`sh2gfw_setsp1para2LGT_blk` 0x0017ddd0 / `UpdateFParam` 0x00126170):
    // Linear: clamp(fa * dist + fb, 0.0, 1.0); Hyperbolic: clamp(h_fa / dist + h_fb, 0.0, 1.0).
    // Parallel directional lights (`inner_cos <= -0.99 && outer_cos <= -0.99`, `sh2gfw_setparal3LGT_blk`
    // at `SLUS_202.28` `0x0017d7f0`) have no distance falloff.
    let lin_atten = clamp(spotlight.ps2_falloff_params.x * dist + spotlight.ps2_falloff_params.y, 0.0, 1.0);
    let hyp_atten = clamp(spotlight.ps2_falloff_params.z / max(dist, 1e-4) + spotlight.ps2_falloff_params.w, 0.0, 1.0);
    let local_atten = select(lin_atten, hyp_atten, spotlight.fill_and_range.w > 0.5);
    let atten = select(local_atten, 1.0, inner_cos <= -0.99 && outer_cos <= -0.99);
    let norm = select(vec3<f32>(0.0, 1.0, 0.0), in.normal / n_len, has_normal);
    let n_dot_l = max(0.0, dot(norm, -l_dir));
    let pcss_vis = evaluate_pcss_spotlight_visibility(in.world_pos, norm, -l_dir);
    let is_primary_parallel = inner_cos <= -0.99 && outer_cos <= -0.99;
    let primary_diffuse = spotlight.color_outer.xyz * (n_dot_l * spot_intensity * atten * pcss_vis);
    var diffuse = primary_diffuse;

    // Auxiliary parallel directional lights (`sh2gfw_setparal3LGT_blk` 0x0017d7f0: 3 parallel lights;
    // `sh2gfw_setsp1para2LGT_blk` 0x0017ddd0: 2 parallel lights in slots 1..2).
    var aux_parallel_diffuse = vec3<f32>(0.0, 0.0, 0.0);
    for (var pi: u32 = 0u; pi < 3u; pi = pi + 1u) {
        let p_dir_en = spotlight.parallel_dirs[pi];
        if (p_dir_en.w > 0.5) {
            let p_len = length(p_dir_en.xyz);
            let p_dir = select(vec3<f32>(0.0, 1.0, 0.0), p_dir_en.xyz / p_len, p_len > 1e-4);
            let p_ndotl = select(1.0, max(dot(norm, -p_dir), 0.0), has_normal);
            aux_parallel_diffuse = aux_parallel_diffuse + spotlight.parallel_colors[pi].xyz * p_ndotl;
        }
    }
    diffuse = diffuse + aux_parallel_diffuse;

    // Up to 3 demo/room point lights (`sh2gfw_Set_DemoPointLight` 0x0017d600) with PS2 linear falloff
    // (only applied in legacy fallback mode; `sh2pc.exe` `.map` `VS[1..8]` bakes room point lights into `v5`).
    var point_diffuse = vec3<f32>(0.0, 0.0, 0.0);
    for (var pti: u32 = 0u; pti < 3u; pti = pti + 1u) {
        let pt_pos_near = spotlight.point_positions[pti];
        let pt_col_far = spotlight.point_colors[pti];
        if (pt_col_far.w > pt_pos_near.w + 1e-3) {
            let pt_vec = pt_pos_near.xyz - in.world_pos;
            let pt_dist = length(pt_vec);
            let pt_dir = select(vec3<f32>(0.0, 1.0, 0.0), pt_vec / pt_dist, pt_dist > 1e-4);
            let pt_atten = clamp((pt_col_far.w - pt_dist) / max(pt_col_far.w - pt_pos_near.w, 1e-4), 0.0, 1.0);
            let pt_ndotl = select(1.0, max(dot(norm, pt_dir), 0.0), has_normal);
            point_diffuse = point_diffuse + pt_col_far.xyz * (pt_ndotl * pt_atten);
        }
    }
    diffuse = diffuse + point_diffuse;

    // Authentic camera view-space directional fill light (`sh2pc.exe` `0x50b036`, `k = 0.3 * s_flash_level * 0.25`)
    let cam_vec = camera.cam_pos.xyz - in.world_pos;
    let cam_len = length(cam_vec);
    let view_dir = select(vec3<f32>(0.0, 0.0, 1.0), cam_vec / cam_len, cam_len > 1e-4);
    let cam_fill_ndotl = select(1.0, max(dot(norm, view_dir), 0.0), has_normal);
    let camera_fill = vec3<f32>(spotlight.fill_and_range.x * cam_fill_ndotl);

    // Gun muzzle flash ambient boost (`(sh2gfw_Get_GunLight() * 2.0) / 4.0 = gun_light * 0.5` in
    // 1.0-normalized ambient space, factoring out `4.0` VU1 ambient scale in `0x0017d7f0` / `0x0017ddd0`)
    // and `DARK_Hosei` ambient trim (`0x002ab710` = 0.5, `sh2gfw_Get_DarkTrimFactor` 0x0028e350).
    let gun_boost = vec3<f32>(spotlight.gun_and_dark_trim.x);
    let dark_trim_scale = 1.0 + spotlight.gun_and_dark_trim.y * spotlight.gun_and_dark_trim.z;

    // `shade_mode >= 1` is exclusively the non-PS2 Model Viewer / Studio inspection rig;
    // Vanilla PS2 gameplay (`shade_mode == 0`) uses exact `sh2pc.exe` `VS[1..8]` (`0x008ef230..0x008efb68`).
    let shade_mode = i32(round(camera.cam_pos.w));
    let env_tint_rgb = select(vec3<f32>(1.0, 1.0, 1.0), spotlight.ambient_tint.xyz, spotlight.ambient_tint.w > 0.001);

    if (shade_mode == 0 && spotlight.map_ambient_c19.w > 0.5) {
        // Exact `sh2pc.exe` `.map` `VS[1..8]` (`0x008ef230..0x008efb68`) + `FUN_005b2d40` (`D3DTOP_MODULATE`):
        // - Parallel light colors are scaled by `4.0 * DAT_007f4120[0xc] = 0.25` (`FUN_0047a420:8121-8125`).
        // - Flashlight spotlight (`!is_primary_parallel`) is scaled by `0.5` to invert `2.0 * r3` (`oD0 = 2.0 * r3`).
        let bg_primary = select(primary_diffuse * 0.5, primary_diffuse * 0.25, is_primary_parallel);
        let bg_diffuse = bg_primary + aux_parallel_diffuse * 0.25;
        var od0: vec3<f32>;
        if (in.shader_mode == 0u) {
            // `VS[1]` (`0x008ef700`) / `VS[3]` (`0x008ef880`): unlit fullbright `mov oD0, c14.wwww` (`1.0`).
            od0 = vec3<f32>(1.0, 1.0, 1.0);
        } else if (in.shader_mode == 3u || !has_normal) {
            // `VS[2]` (`0x008ef7c0`) / `VS[4]` (`0x008ef940`): unlit pre-lit vertex color `mov oD0, v5`.
            od0 = in.color.rgb;
        } else if (in.shader_mode == 1u || in.shader_mode == 2u || in.shader_mode == 6u) {
            // `VS[5]` (`0x008ef230`) / `VS[7]` (`0x008efa10`): `r3 = (diffuse + c19) * c26`, `oD0 = clamp(2.0 * r3, 0, 1)`.
            let r3 = (bg_diffuse + spotlight.map_ambient_c19.xyz + camera_fill * 0.5) * in.color.rgb;
            od0 = clamp(r3 * 2.0, vec3<f32>(0.0), vec3<f32>(1.0));
        } else {
            // `VS[6]` (`0x008ef490`) / `VS[8]` (`0x008efb68`, modes `4, 5, 7`):
            // `r3 = (diffuse + c21) * c20 + v5 * c22`, `oD0 = clamp(2.0 * r3, 0, 1)`.
            let r3 = (bg_diffuse + spotlight.map_base_ambient_c21.xyz) * spotlight.map_base_color_c20.xyz
                + in.color.rgb * spotlight.map_vertex_ambient_c22.xyz
                + camera_fill * 0.5;
            od0 = clamp(r3 * 2.0, vec3<f32>(0.0), vec3<f32>(1.0));
        }
        return vec4<f32>(tex_color.rgb * od0 * env_tint_rgb, out_alpha);
    }

    let studio_ambient = select(base_ambient, vec3<f32>(0.22, 0.23, 0.25), shade_mode >= 1);
    let env_ambient_scale = select(1.0, spotlight.ambient_tint.w, spotlight.ambient_tint.w > 0.001);
    let lit_color = in.color.rgb * tex_color.rgb * (studio_ambient * env_tint_rgb * env_ambient_scale * dark_trim_scale + gun_boost + camera_fill * env_tint_rgb + diffuse * env_tint_rgb);
    return vec4<f32>(lit_color, out_alpha);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return shade_geometry_fragment(in);
}

// Opaque geometry entry point (`BlendState::REPLACE`): encodes PS2 GS `PRIM.FGE` (Fog Enable)
// in `color_in.a` (`1.0` for lit 3D world geometry `stride >= 0x20`, `0.0` for pre-lit
// skybox/vista geometry `stride == 0x14` such as Toluca Lake & distant hills in `ob01.map`).
@fragment
fn fs_opaque(in: VertexOutput) -> @location(0) vec4<f32> {
    let shaded = shade_geometry_fragment(in);
    let fge = select(0.0, 1.0, length(in.normal) > 1e-4);
    return vec4<f32>(shaded.rgb, fge);
}
"#;

    pub fn new(
        device: &wgpu::Device,
        color_format: wgpu::TextureFormat,
        depth_format: wgpu::TextureFormat,
    ) -> Self {
        let shader_src = format!(
            "{}\n{}",
            crate::shadow::WGSL_PCSS_FUNCTIONS,
            Self::WGSL_SHADER
        );
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Geometry Shader Module"),
            source: wgpu::ShaderSource::Wgsl(shader_src.into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Geometry Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Geometry Diffuse Texture Bind Group Layout"),
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

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Geometry Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout, &texture_bind_group_layout],
            push_constant_ranges: &[],
        });

        let vertex_attributes = [
            // position
            wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32x3,
            },
            // normal
            wgpu::VertexAttribute {
                offset: 12,
                shader_location: 1,
                format: wgpu::VertexFormat::Float32x3,
            },
            // uv
            wgpu::VertexAttribute {
                offset: 24,
                shader_location: 2,
                format: wgpu::VertexFormat::Float32x2,
            },
            // color
            wgpu::VertexAttribute {
                offset: 32,
                shader_location: 3,
                format: wgpu::VertexFormat::Float32x4,
            },
            // shader_mode (`MapMaterial.mode & 0xff`)
            wgpu::VertexAttribute {
                offset: 48,
                shader_location: 4,
                format: wgpu::VertexFormat::Uint32,
            },
        ];

        let vertex_buffer_layout = [wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<sh2_assets::formats::map::MapVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &vertex_attributes,
        }];

        let opaque_state = map_batch_render_state(sh2_assets::formats::map::BatchGroup::Opaque);
        // Hardware backface culling matching `sh2pc.exe` `D3DRS_CULLMODE (0x16) = 2 (D3DCULL_CW)`
        // inherited by `FUN_005b4cc0` (`0x005b4cc0`) and restored for decals (`0x005b1694`).
        let primitive_state = wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: opaque_state.front_face,
            cull_mode: opaque_state.cull_mode,
            polygon_mode: wgpu::PolygonMode::Fill,
            unclipped_depth: false,
            conservative: false,
        };

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Geometry Opaque & Cutout Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &vertex_buffer_layout,
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_opaque"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: color_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: primitive_state,
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

        let decal_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Geometry Coplanar Decal Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &vertex_buffer_layout,
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: color_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: primitive_state,
            depth_stencil: Some(wgpu::DepthStencilState {
                format: depth_format,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState {
                    constant: -2,
                    slope_scale: -2.0,
                    clamp: 0.0,
                },
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let transparent_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Geometry Transparent Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &vertex_buffer_layout,
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: color_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: primitive_state,
            depth_stencil: Some(wgpu::DepthStencilState {
                format: depth_format,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState {
                    constant: -1,
                    slope_scale: -1.0,
                    clamp: 0.0,
                },
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let stencil_equal_keep = wgpu::StencilFaceState {
            compare: wgpu::CompareFunction::Equal,
            fail_op: wgpu::StencilOperation::Keep,
            depth_fail_op: wgpu::StencilOperation::Keep,
            pass_op: wgpu::StencilOperation::Keep,
        };
        let mut mirror_primitive = primitive_state;
        mirror_primitive.front_face = wgpu::FrontFace::Cw;
        let mirror_reflected_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Geometry Stenciled Mirror Reflected Pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    buffers: &vertex_buffer_layout,
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_opaque"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: color_format,
                        blend: Some(wgpu::BlendState::REPLACE),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: mirror_primitive,
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: crate::shadow::StencilShadowPipeline::STENCIL_FORMAT,
                    depth_write_enabled: true,
                    depth_compare: wgpu::CompareFunction::LessEqual,
                    stencil: wgpu::StencilState {
                        front: stencil_equal_keep,
                        back: stencil_equal_keep,
                        read_mask: 0xff,
                        write_mask: 0x00,
                    },
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            });

        Self {
            pipeline,
            mirror_reflected_pipeline,
            decal_pipeline,
            transparent_pipeline,
            bind_group_layout,
            texture_bind_group_layout,
        }
    }
}

pub struct DebugLinePipeline {
    pub pipeline: wgpu::RenderPipeline,
}

impl DebugLinePipeline {
    pub const WGSL_LINE_SHADER: &'static str = r#"
struct CameraUniforms {
    view_proj: mat4x4<f32>,
    inv_proj: mat4x4<f32>,
    cam_pos: vec4<f32>,
    inv_view_proj: mat4x4<f32>,
};

@group(0) @binding(0) var<uniform> camera: CameraUniforms;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = camera.view_proj * vec4<f32>(in.position, 1.0);
    out.color = in.color;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}
"#;

    pub fn new(
        device: &wgpu::Device,
        color_format: wgpu::TextureFormat,
        depth_format: wgpu::TextureFormat,
        common_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Debug Line WGSL Shader"),
            source: wgpu::ShaderSource::Wgsl(Self::WGSL_LINE_SHADER.into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Debug Line Pipeline Layout"),
            bind_group_layouts: &[common_bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Debug Line Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<sh2_debug::DebugLineVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 0,
                            format: wgpu::VertexFormat::Float32x3,
                        },
                        wgpu::VertexAttribute {
                            offset: 12,
                            shader_location: 1,
                            format: wgpu::VertexFormat::Float32x4,
                        },
                    ],
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: color_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::LineList,
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
                depth_compare: wgpu::CompareFunction::Always,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        Self { pipeline }
    }
}

#[cfg(test)]
mod tests {
    use super::GeometryPipeline;

    #[test]
    fn map_shader_uses_slus_draw_env_base_ambient_002ad900() {
        let src = GeometryPipeline::WGSL_SHADER;
        assert!(src.contains("vec3<f32>(0.045, 0.045, 0.045)"));
        assert!(src.contains("vec3<f32>(0.18, 0.18, 0.18)"));
        assert!(!src.contains("0.07, 0.075, 0.085"));
        assert!(!src.contains("0.18, 0.19, 0.21"));
    }
}
