//! WGSL GPU Linear Blend Skinning (LBS), PS2/PC per-primitive material shading,
//! `material_type == 4` specular gloss masking, and two-pass hair transparency shader (`SH2-37`).

/// Complete WGSL source for skeletal character rendering (`vs_main`, `fs_opaque`,
/// `fs_transparent_core`, and `fs_transparent_fringe`).
pub const WGSL_SKINNING_SHADER: &str = r#"
struct CameraUniforms {
    view_proj: mat4x4<f32>,
    inv_proj: mat4x4<f32>,
    cam_pos: vec4<f32>,
    inv_view_proj: mat4x4<f32>,
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
};

struct SkeletonBones {
    matrices: array<mat4x4<f32>, 128>,
};

struct SkeletalMaterialUniforms {
    diffuse_mtype: vec4<f32>,
    ambient_shininess: vec4<f32>,
    specular_cull: vec4<f32>,
    _reserved: vec4<f32>,
};

@group(0) @binding(0) var<uniform> camera: CameraUniforms;
@group(0) @binding(1) var<uniform> spotlight: SpotlightUniforms;
@group(0) @binding(2) var<uniform> pcss_u: PcssShadowUniforms;
@group(0) @binding(3) var pcss_shadow_map: texture_depth_2d;
@group(0) @binding(4) var specular_sphere_tex: texture_2d<f32>;
@group(0) @binding(5) var specular_sphere_sampler: sampler;
@group(1) @binding(0) var<uniform> skeleton: SkeletonBones;
@group(2) @binding(0) var diffuse_texture: texture_2d<f32>;
@group(2) @binding(1) var diffuse_sampler: sampler;
@group(3) @binding(0) var<uniform> material: SkeletalMaterialUniforms;

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
    let raw_n = (skin_matrix * vec4<f32>(in.normal, 0.0)).xyz;
    let n_len = length(raw_n);
    let skinned_normal = select(vec3<f32>(0.0, -1.0, 0.0), raw_n / n_len, n_len > 1e-4);

    var out: VertexOutput;
    out.world_pos = skinned_pos.xyz;
    out.clip_position = camera.view_proj * vec4<f32>(skinned_pos.xyz, 1.0);
    out.normal = skinned_normal;
    out.uv = in.uv;
    out.color = vec4<f32>(1.0, 1.0, 1.0, 1.0);
    return out;
}

// `pass_mode`:
// 0u = Opaque pass (`is_transparent == false`)
// 1u = Transparent hair/fringe core pass (`tex_color.a >= 0.90`, writes depth)
// 2u = Transparent hair/fringe soft edge pass (`0.004 <= tex_color.a < 0.90`, reads depth)
fn shade_skeletal_fragment(in: VertexOutput, pass_mode: u32) -> vec4<f32> {
    // Sample diffuse texture unconditionally in uniform control flow before any early returns
    // so derivative evaluation (`textureSample`) satisfies strict WGSL uniformity rules across all backends.
    let tex_color = textureSample(diffuse_texture, diffuse_sampler, in.uv);

    let n_len = length(in.normal);
    let raw_norm = select(vec3<f32>(0.0, -1.0, 0.0), in.normal / n_len, n_len > 1e-4);

    let to_cam = camera.cam_pos.xyz - in.world_pos;
    let cam_dist = length(to_cam);
    let view_dir = select(vec3<f32>(0.0, 0.0, 1.0), to_cam / cam_dist, cam_dist > 1e-4);

    // Orient normals toward the viewer on double-sided hair/fringe cards (`cull == 0`) while keeping
    // smooth interpolated normals intact on curved single-sided limbs (`cull == 1`).
    let norm = select(raw_norm, -raw_norm, material.specular_cull.w < 0.5 && dot(raw_norm, view_dir) < 0.0);

    let mat_type = u32(round(material.diffuse_mtype.w));

    // Per-pass alpha discard before shading modes so cutout/hair silhouettes match across all modes:
    // - `pass_mode == 0u` (`Opaque`): solid & PS2 specular gloss-mask submeshes (`red.mdl` `1303`,
    //   Maria's `1245` fingernails/rings, `scu.mdl` `1282`, `pap.mdl` `1293`) NEVER discard texels
    //   (`sh2pc.exe` `0x00500008` `ALPHATESTENABLE = 0`), as genuine cutout `prim` cards (`lll_jms`,
    //   `lll_mar`, `lxx_mar` hair) are promoted to `pass_mode == 1u / 2u` at mesh compilation time.
    // - `pass_mode == 1u / 2u`: transparent `tprim` and promoted cutout hair submeshes split cleanly
    //   at `0.90` into depth-writing core (`1u`) + depth-read-only soft alpha-blended fringe (`2u`).
    if pass_mode == 1u {
        if tex_color.a < 0.90 {
            discard;
        }
    } else if pass_mode == 2u {
        if tex_color.a < (1.0 / 255.0) || tex_color.a >= 0.90 {
            discard;
        }
    }

    let out_alpha = select(clamp(in.color.a * tex_color.a, 0.0, 1.0), 1.0, pass_mode == 0u);

    // Inspect shading mode encoded in camera.cam_pos.w:
    // 0 = Textured (Spotlight + Ambient)
    // 1 = Untextured Clay (3-Point Studio Shading)
    // 2 = Surface Normals (RGB Vector Map)
    // 3 = Textured with 3-Point Studio Lighting (Key + Fill + Rim + Spotlight)
    let shade_mode = i32(round(camera.cam_pos.w));
    if shade_mode == 2 {
        let n_rgb = norm * 0.5 + vec3<f32>(0.5, 0.5, 0.5);
        return vec4<f32>(n_rgb, out_alpha);
    }

    // Spotlight lighting evaluation (world units are PS2 millimeters: `1000.0 == 1.0 m`)
    let light_to_frag = in.world_pos - spotlight.position.xyz;
    let dist = length(light_to_frag);
    let l_dir = select(vec3<f32>(0.0, 1.0, 0.0), light_to_frag / dist, dist > 1e-4);

    let spot_dir_len = length(spotlight.direction_inner.xyz);
    let spot_dir = select(vec3<f32>(0.0, 0.0, -1.0), spotlight.direction_inner.xyz / spot_dir_len, spot_dir_len > 1e-4);
    let inner_cos = spotlight.direction_inner.w;
    let outer_cos = spotlight.color_outer.w;

    let current_cos = dot(l_dir, spot_dir);
    // Reject fragments behind narrow pocket-flashlight emitter plane (`outer_cos >= 0`) while allowing
    // wide stage/ceiling lights (`outer_cos < 0`) to wrap smoothly across their hemisphere.
    let forward_gate = select(0.0, 1.0, current_cos > min(outer_cos, 0.0));
    let spot_cone = select(
        clamp((current_cos - outer_cos) / max(inner_cos - outer_cos, 0.0001), 0.0, 1.0) * forward_gate,
        1.0,
        inner_cos <= -0.99 && outer_cos <= -0.99
    );

    // Exact PS2 linear/hyperbolic falloff (`sh2gfw_setsp1para2LGT_blk` 0x0017ddd0 / `UpdateFParam` 0x00126170)
    let lin_atten = clamp(spotlight.ps2_falloff_params.x * dist + spotlight.ps2_falloff_params.y, 0.0, 1.0);
    let hyp_atten = clamp(spotlight.ps2_falloff_params.z / max(dist, 1e-4) + spotlight.ps2_falloff_params.w, 0.0, 1.0);
    let local_atten = select(lin_atten, hyp_atten, spotlight.fill_and_range.w > 0.5);
    let attenuation = select(local_atten, 1.0, inner_cos <= -0.99 && outer_cos <= -0.99);
    let n_dot_l = max(dot(norm, -l_dir), 0.0);
    let pcss_vis = evaluate_pcss_spotlight_visibility(in.world_pos, norm, -l_dir);

    let base_ambient = vec3<f32>(0.06, 0.06, 0.068);
    var spot_diffuse = spotlight.color_outer.xyz * (n_dot_l * spot_cone * attenuation * pcss_vis);
    for (var pi: u32 = 0u; pi < 3u; pi = pi + 1u) {
        let p_dir_en = spotlight.parallel_dirs[pi];
        if (p_dir_en.w > 0.5) {
            let p_len = length(p_dir_en.xyz);
            let p_dir = select(vec3<f32>(0.0, 1.0, 0.0), p_dir_en.xyz / p_len, p_len > 1e-4);
            spot_diffuse = spot_diffuse + spotlight.parallel_colors[pi].xyz * max(dot(norm, -p_dir), 0.0);
        }
    }
    for (var pti: u32 = 0u; pti < 3u; pti = pti + 1u) {
        let pt_pos_near = spotlight.point_positions[pti];
        let pt_col_far = spotlight.point_colors[pti];
        if (pt_col_far.w > pt_pos_near.w + 1e-3) {
            let pt_vec = pt_pos_near.xyz - in.world_pos;
            let pt_dist = length(pt_vec);
            let pt_dir = select(vec3<f32>(0.0, 1.0, 0.0), pt_vec / pt_dist, pt_dist > 1e-4);
            let pt_atten = clamp((pt_col_far.w - pt_dist) / max(pt_col_far.w - pt_pos_near.w, 1e-4), 0.0, 1.0);
            spot_diffuse = spot_diffuse + pt_col_far.xyz * (max(dot(norm, pt_dir), 0.0) * pt_atten);
        }
    }
    let camera_fill = vec3<f32>(spotlight.fill_and_range.x * max(dot(norm, view_dir), 0.0));
    let gun_boost = vec3<f32>(spotlight.gun_and_dark_trim.x);
    let dark_trim_scale = 1.0 + spotlight.gun_and_dark_trim.y * spotlight.gun_and_dark_trim.z;

    // 3-Point Studio Rig (Key + Warm Fill + Cool Rim) used in Model Viewer modes (1 and 3)
    let key_dir = normalize(vec3<f32>(0.45, -0.65, 0.60));
    let fill_dir = normalize(vec3<f32>(-0.65, -0.25, 0.70));
    let rim_dir = normalize(vec3<f32>(0.0, -0.40, -0.90));

    let k_ndotl = max(dot(norm, key_dir), 0.0);
    let f_ndotl = max(dot(norm, fill_dir), 0.0);
    let r_ndotl = max(dot(norm, rim_dir), 0.0);

    let k_diff = k_ndotl * vec3<f32>(0.68, 0.66, 0.63);
    let f_diff = f_ndotl * vec3<f32>(0.24, 0.28, 0.33);
    let r_diff = pow(r_ndotl, 2.0) * vec3<f32>(0.36, 0.40, 0.48);
    let studio_amb = vec3<f32>(0.18, 0.18, 0.20);
    let studio_direct = k_diff + f_diff + r_diff + spot_diffuse * 0.45;

    if shade_mode == 1 {
        let clay_albedo = vec3<f32>(0.76, 0.74, 0.72);
        return vec4<f32>(clay_albedo * (studio_amb + studio_direct), out_alpha);
    }

    // Material Type 1 (`SLUS_202.28` `DrawPart1` `0x12d934` / `sh2pc.exe` `0x501586`): unlit full-bright
    if mat_type == 1u {
        return vec4<f32>(in.color.rgb * tex_color.rgb, out_alpha);
    }

    // PS2 GS / D3D8 `MODULATE2X` (`2.0 * material.{ambient, diffuse}`, `sh2pc.exe` `0x501ab3` / `0x501bfa`)
    let mat_diffuse = material.diffuse_mtype.xyz * 2.0;
    let mat_ambient = material.ambient_shininess.xyz * 2.0;

    let env_tint_rgb = select(vec3<f32>(1.0, 1.0, 1.0), spotlight.ambient_tint.xyz, spotlight.ambient_tint.w > 0.001);
    let env_ambient_scale = select(1.0, spotlight.ambient_tint.w, spotlight.ambient_tint.w > 0.001);

    let game_diffuse_light = (mat_ambient * base_ambient * env_ambient_scale * dark_trim_scale + gun_boost + mat_diffuse * (spot_diffuse + camera_fill)) * env_tint_rgb;
    let studio_diffuse_light = (mat_ambient * studio_amb + mat_diffuse * studio_direct) * env_tint_rgb;
    let total_diffuse_light = select(game_diffuse_light, studio_diffuse_light, shade_mode == 3);

    var final_rgb = total_diffuse_light * in.color.rgb * tex_color.rgb;

    // MakeEnvironPacket (`SLUS_202.28` `0x0012cf20`): optional environment reflection pass
    // modulated by part header byte `+0x45` (`reflection_color.w` in `0..=1`).
    if spotlight.reflection_color.w > 0.001 {
        let env_ref_up = select(vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(1.0, 0.0, 0.0), abs(view_dir.y) > 0.99);
        let env_u_axis = normalize(cross(env_ref_up, view_dir));
        let env_v_axis = normalize(cross(view_dir, env_u_axis));
        let env_uv = clamp(vec2<f32>(0.5 * dot(norm, env_u_axis) + 0.5, 0.5 * dot(norm, env_v_axis) + 0.5), vec2<f32>(0.0), vec2<f32>(1.0));
        let env_sample = textureSampleLevel(specular_sphere_tex, specular_sphere_sampler, env_uv, 0.0).rgb;
        final_rgb = final_rgb + env_sample * spotlight.reflection_color.xyz * spotlight.reflection_color.w;
    }

    // Material Type 4 (`SLUS_202.28` `mdl_SpecularMappingMakeTexture` `0x0012f0c0`,
    // `MakeSpecularPacket` `0x0012d090`, `UpdateReflections` `0x00126f90`, `sh2gfw_Set_SpecularAlpha`
    // `0x00181be0` `ALPHA_1 = 0x58` / `sh2pc.exe` `0x00501750`):
    // 64x64 `pow(sqrt(1 - r^2), 8.0)` sphere-map lookup projected via NHM `(U, V, H)` basis
    // and composited with GS destination-alpha additive blend (`Cs * Ad + Cd`).
    if mat_type == 4u {
        let spec_rgb = material.specular_cull.xyz;
        let gloss_mask = select(1.0, tex_color.a, pass_mode == 0u);

        let h_spot_raw = -l_dir + view_dir;
        let h_spot = select(view_dir, normalize(h_spot_raw), length(h_spot_raw) > 1e-4);
        let ref_up = select(vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(1.0, 0.0, 0.0), abs(h_spot.y) > 0.99);
        let u_axis = normalize(cross(ref_up, h_spot));
        let v_axis = normalize(cross(h_spot, u_axis));
        let spot_spec_uv = clamp(
            vec2<f32>(0.5 * dot(norm, u_axis) + 0.5, 0.5 * dot(norm, v_axis) + 0.5),
            vec2<f32>(0.0),
            vec2<f32>(1.0),
        );
        let sphere_val = textureSampleLevel(specular_sphere_tex, specular_sphere_sampler, spot_spec_uv, 0.0).rgb;
        let spec_spot = sphere_val
            * spot_cone
            * attenuation
            * pcss_vis
            * select(0.0, 1.0, n_dot_l > 0.0)
            * spotlight.color_outer.xyz;

        // Studio inspection mode (`shade_mode == 3`) also samples the exact 64x64 sphere-map
        let h_key = normalize(key_dir + view_dir);
        let k_ref_up = select(vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(1.0, 0.0, 0.0), abs(h_key.y) > 0.99);
        let k_u = normalize(cross(k_ref_up, h_key));
        let k_v = normalize(cross(h_key, k_u));
        let k_uv = clamp(vec2<f32>(0.5 * dot(norm, k_u) + 0.5, 0.5 * dot(norm, k_v) + 0.5), vec2<f32>(0.0), vec2<f32>(1.0));
        let spec_studio = textureSampleLevel(specular_sphere_tex, specular_sphere_sampler, k_uv, 0.0).rgb
            * vec3<f32>(0.90, 0.88, 0.84)
            * select(0.0, 1.0, k_ndotl > 0.0)
            + spec_spot;

        let spec_scale = select(1.0, pcss_u.params.w, pcss_u.params.w > 0.01);
        let active_spec = select(spec_spot, spec_studio, shade_mode == 3) * env_tint_rgb * spec_scale * spec_rgb;
        // GS `ALPHA_1 = 0x58` (night/interior): `Cd + Cs * Ad`; `0x54` (day): `Cd + (Cs - Cd) * Ad`
        let is_night = spotlight.gun_and_dark_trim.w > 0.5;
        final_rgb = select(
            final_rgb + (active_spec - final_rgb) * gloss_mask,
            final_rgb + active_spec * gloss_mask,
            is_night,
        );
    }

    return vec4<f32>(final_rgb, out_alpha);
}

@fragment
fn fs_opaque(in: VertexOutput) -> @location(0) vec4<f32> {
    return shade_skeletal_fragment(in, 0u);
}

@fragment
fn fs_transparent_core(in: VertexOutput) -> @location(0) vec4<f32> {
    return shade_skeletal_fragment(in, 1u);
}

@fragment
fn fs_transparent_fringe(in: VertexOutput) -> @location(0) vec4<f32> {
    return shade_skeletal_fragment(in, 2u);
}
"#;
