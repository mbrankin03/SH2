// 1:1 PS2-accurate multi-layered UV-scrolling + 3D raymarched volumetric fog compute shader (`SH2-32`).
//
// Ground-truth citations:
// - Depth fog curve: `sh2gfw_Set_DepthFog` (0x00175df0) & `dbg_fogparm_ctl` / `dbg_ambifog_ctl`
// - Dual-layer UV-scrolling noise planes: `fog_view_screen_fog` (0x0016c1c0), `fog_asm_data2` (0x002ab470),
//   `sh2pc.exe` (0x0048734C / 0x0048893D), `FogFix.cpp`, `Settings.h:183-219`
// - Volumetric raymarching & flashlight shadow-tested god rays: `VideoOptions::fog_quality` / `volumetric_light`

struct FogUniforms {
    fog_color: vec4<f32>,
    fog_near: f32,
    fog_far: f32,
    density: f32,
    time: f32,
    step_count: u32,
    curve_type: u32,
    speed_mult: f32,
    ambient_fog_coupling: f32,
    // [fog_layer1_x1, fog_layer1_y1, fog_layer1_x2, fog_layer1_y2]
    layer1_uv_scale: vec4<f32>,
    // [fog_layer2_complexity, fog_layer2_density_mult, fog_layer2_density_add, outdoor_layers_enabled]
    layer2_params: vec4<f32>,
    // [layer1_alpha, layer2_alpha, volumetric_mode (0=ClassicPs2, 1=ModernVolumetric), volumetric_light_enabled]
    layer_transparencies: vec4<f32>,
    shadow_view_proj: mat4x4<f32>,
    // Exact PS2 depth-fog coefficients [persp_fa, persp_fb, fog_min, fog_max] (`sh2gfw_Set_DepthFog` 0x00175df0)
    depth_fog_params: vec4<f32>,
    // `fog_view_screen_fog` (0x0016c1c0) UV window [u_center, v_center, half_span (0.125), gs_alpha_norm]
    screen_fog_uv: vec4<f32>,
};

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

@group(0) @binding(0) var<uniform> fog: FogUniforms;
@group(0) @binding(1) var<uniform> camera: CameraUniforms;
@group(0) @binding(2) var depth_tex: texture_2d<f32>;
@group(0) @binding(3) var color_in: texture_2d<f32>;
@group(0) @binding(4) var color_out: texture_storage_2d<rgba16float, write>;
@group(0) @binding(5) var<uniform> spotlight: SpotlightUniforms;
@group(0) @binding(6) var shadow_depth_tex: texture_2d<f32>;
@group(0) @binding(7) var fog_noise_tex: texture_2d<f32>;
@group(0) @binding(8) var fog_noise_sampler: sampler;

// Interleaved gradient noise for banding-free raymarch jitter
fn interleaved_gradient_noise(pixel: vec2<f32>) -> f32 {
    let magic = vec3<f32>(0.06711056, 0.00583715, 52.9829189);
    return fract(magic.z * fract(dot(pixel, magic.xy)));
}

// Evaluate PS2 depth-fog curve (`sh2gfw_Set_DepthFog` @ 0x00175df0):
// Hyperbolic: clamp(FA / z + FB, FogMin, FogMax); Linear/Exp curves bounded by [FogMin, FogMax].
fn eval_depth_curve(dist_mm: f32) -> f32 {
    let fog_min = min(fog.depth_fog_params.z, fog.depth_fog_params.w);
    let fog_max = max(fog.depth_fog_params.z, fog.depth_fog_params.w);
    let span = max(1e-4, fog.fog_far - fog.fog_near);
    if (fog.curve_type == 0u && fog.fog_near > 1e-4 && fog.fog_far > fog.fog_near + 1e-4) {
        // Exact `sh2gfw_Set_DepthFog` (`SLUS_202.28` `0x00175df0`) computed from `(fog_near, fog_far, fog_min, fog_max)`
        // so direct uniform updates to `fog_near`/`fog_far` stay in lockstep with `FA / z + FB`.
        let persp_fa = (fog.fog_far * fog.fog_near * (fog_min - fog_max)) / span;
        let persp_fb = (fog.fog_far * fog_max - fog.fog_near * fog_min) / span;
        let hyp = persp_fa / max(dist_mm, 1e-4) + persp_fb;
        return clamp(hyp, fog_min, fog_max);
    }
    let linear_t = clamp((dist_mm - fog.fog_near) / span, 0.0, 1.0);
    if (fog.curve_type == 1u) {
        return clamp(mix(fog_min, fog_max, 1.0 - exp(-3.0 * linear_t)), fog_min, fog_max);
    } else if (fog.curve_type == 2u) {
        return clamp(mix(fog_min, fog_max, 1.0 - exp(-4.0 * linear_t * linear_t)), fog_min, fog_max);
    }
    return clamp(mix(fog_min, fog_max, linear_t), fog_min, fog_max);
}

// Sample the spotlight shadow depth texture at a world-space point (returns 1.0 if lit, 0.0 if occluded)
fn sample_spotlight_shadow(world_pos: vec3<f32>) -> f32 {
    let light_clip = fog.shadow_view_proj * vec4<f32>(world_pos, 1.0);
    if (light_clip.w <= 1e-4) {
        return 1.0;
    }
    let ndc = light_clip.xyz / light_clip.w;
    let shadow_uv = vec2<f32>(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5);
    if (shadow_uv.x < 0.0 || shadow_uv.x > 1.0 || shadow_uv.y < 0.0 || shadow_uv.y > 1.0 || ndc.z < 0.0 || ndc.z > 1.0) {
        return 1.0;
    }
    let s_dims = textureDimensions(shadow_depth_tex);
    let texel = vec2<i32>(
        clamp(i32(shadow_uv.x * f32(s_dims.x)), 0, i32(s_dims.x) - 1),
        clamp(i32(shadow_uv.y * f32(s_dims.y)), 0, i32(s_dims.y) - 1),
    );
    let stored_depth = textureLoad(shadow_depth_tex, texel, 0).r;
    // Small depth bias to avoid shadow acne inside volumetric steps
    return select(0.0, 1.0, ndc.z <= stored_depth + 0.0025);
}

// Henyey-Greenstein forward scattering phase function for flashlight mist shafts
fn hg_phase(cos_theta: f32, g: f32) -> f32 {
    let g2 = g * g;
    let denom = max(1e-4, 1.0 + g2 - 2.0 * g * cos_theta);
    return (1.0 - g2) / (4.0 * 3.14159265 * pow(denom, 1.5));
}

// Evaluate flashlight spotlight volumetric inscattering at a single world-space raymarch sample
fn eval_spotlight_scatter(sample_world: vec3<f32>, ray_dir: vec3<f32>) -> vec3<f32> {
    let spot_intensity_sum = spotlight.color_outer.x + spotlight.color_outer.y + spotlight.color_outer.z;
    // Only raymarch volumetric flashlight god rays for an actual forward spotlight cone (`outer_cos > 0.0`),
    // never for omnidirectional fills or parallel directional lights (`sh2gfw_setparal3LGT_blk`).
    if (spot_intensity_sum <= 1e-4 || spotlight.color_outer.w <= 0.0) {
        return vec3<f32>(0.0);
    }
    let light_to_sample = sample_world - spotlight.position.xyz;
    let dist = length(light_to_sample);
    if (dist < 25.0 || dist > fog.fog_far) {
        return vec3<f32>(0.0);
    }
    let l_dir = light_to_sample / dist;
    let spot_dir = normalize(spotlight.direction_inner.xyz);
    let cos_angle = dot(l_dir, spot_dir);
    let inner_cos = spotlight.direction_inner.w;
    let outer_cos = spotlight.color_outer.w;
    let cone = clamp((cos_angle - outer_cos) / max(1e-4, inner_cos - outer_cos), 0.0, 1.0);
    if (cone <= 0.0) {
        return vec3<f32>(0.0);
    }

    let shadow_vis = sample_spotlight_shadow(sample_world);
    if (shadow_vis <= 0.0) {
        return vec3<f32>(0.0);
    }

    // Distance attenuation normalized by `DrawEnvData::flashlight_range` (`spotlight.fill_and_range.z`,
    // `SLUS_202.28` `0x00251370` default `6000.0` mm, SH2:EE `8500.0` mm).
    let spot_range = max(1000.0, spotlight.fill_and_range.z);
    let norm_d = dist / spot_range;
    let atten = 1.0 / (1.0 + norm_d * norm_d);
    // Forward Mie scattering phase (Henyey-Greenstein g = 0.5)
    let phase = hg_phase(dot(ray_dir, l_dir), 0.5);
    return spotlight.color_outer.xyz * (cone * cone * atten * shadow_vis * (0.5 + phase * 3.14159265));
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let dims = textureDimensions(color_in);
    if (global_id.x >= dims.x || global_id.y >= dims.y) {
        return;
    }
    let pixel_coords = vec2<i32>(global_id.xy);
    let original_color = textureLoad(color_in, pixel_coords, 0);
    let depth_val = textureLoad(depth_tex, pixel_coords, 0).r;

    // Reconstruct view-space Z depth (`r1.w = clip.w = |view_pos.z|`, matching `sh2pc.exe`
    // `VS[1..4]` `0x008ef700..0x008ef940` `oFog.x` & `SLUS_202.28` `sh2gfw_Set_DepthFog` `FA / z + FB`)
    // and Euclidean view distance for optional ModernVolumetric raymarching.
    let uv = (vec2<f32>(pixel_coords) + 0.5) / vec2<f32>(dims);
    let clip_pos = vec4<f32>(uv.x * 2.0 - 1.0, (1.0 - uv.y) * 2.0 - 1.0, depth_val, 1.0);

    let view_pos_h = camera.inv_proj * clip_pos;
    let view_pos = view_pos_h.xyz / max(1e-5, abs(view_pos_h.w));
    let view_z = abs(view_pos.z);
    let view_dist = length(view_pos);

    let world_pos_h = camera.inv_view_proj * clip_pos;
    let world_pos = world_pos_h.xyz / select(1e-5, world_pos_h.w, abs(world_pos_h.w) > 1e-5);
    let cam_pos = camera.cam_pos.xyz;
    let cam_to_world = world_pos - cam_pos;
    let world_dist = length(cam_to_world);
    let ray_dir = cam_to_world / max(1e-4, world_dist);

    let drift_t = fog.time * fog.speed_mult;
    let outdoor_enabled = fog.layer2_params.w > 0.5;
    let volumetric_mode = fog.layer_transparencies.z > 0.5;
    let volumetric_light_enabled = fog.layer_transparencies.w > 0.5;

    // Effective fog color coupled with ambient tint (`dbg_ambifog_ctl`)
    let env_tint = select(vec3<f32>(1.0), spotlight.ambient_tint.xyz, spotlight.ambient_tint.w > 0.001);
    let coupled_fog_rgb = mix(fog.fog_color.rgb, fog.fog_color.rgb * env_tint, clamp(fog.ambient_fog_coupling, 0.0, 1.0));
    let effective_density = clamp(fog.density * fog.fog_color.a, 0.0, 2.0);

    var fog_factor: f32 = 0.0;
    var screen_fog_texel: f32 = 0.0;
    var screen_fog_alpha: f32 = 0.0;
    var inscatter_rgb = vec3<f32>(0.0);

    let is_classic_ps2 = !volumetric_mode || fog.step_count <= 1u;

    // 1:1 `fog_view_screen_fog` (`SLUS_202.28` `0x0016c1c0`, `Fog/fog.c:2695-2711`) &
    // `sh2pc.exe` `FUN_00487210` (`0x00487210`, `sh2pc_00480000.c:4354-4366`) + `FogFix.cpp:81-88`:
    // - In `ClassicPs2` mode (`is_classic_ps2`), enforce exact PS2 square `±0.125` UV half-span
    //   (`fog.screen_fog_uv.z`) and pure vertex alpha `As = (fog_color_a * fog_alpha_param) >> 9`.
    // - In `ModernVolumetric` / Enhanced mode, allow SH2:EE `layer1_uv_scale.xy` (`[0.250, 0.125]`)
    //   and texture-modulated blend (`screen_fog_alpha * screen_fog_texel`).
    if (fog.screen_fog_uv.w > 0.001) {
        let classic_half_span = vec2<f32>(fog.screen_fog_uv.z, fog.screen_fog_uv.z);
        let enhanced_half_span = select(
            classic_half_span,
            fog.layer1_uv_scale.xy,
            fog.layer1_uv_scale.x > 0.001 && fog.layer1_uv_scale.y > 0.001,
        );
        let half_span = select(enhanced_half_span, classic_half_span, is_classic_ps2);
        let screen_uv_window = fog.screen_fog_uv.xy + (uv - vec2<f32>(0.5)) * (half_span * 2.0);
        screen_fog_texel = textureSampleLevel(fog_noise_tex, fog_noise_sampler, screen_uv_window, 0.0).r;
        screen_fog_alpha = clamp(fog.screen_fog_uv.w * effective_density, 0.0, 1.0);
    }

    if (is_classic_ps2) {
        // 1:1 Classic PS2/PC analytical depth-fog (`sh2gfw_Set_DepthFog` `FA/z + FB` @ `0x00175df0`,
        // `sh2pc.exe` `VS[1..4]` `0x008ef700..0x008ef940` using view-space `z = clip.w`).
        let base_curve = eval_depth_curve(view_z);
        fog_factor = clamp(base_curve * effective_density, 0.0, 1.0);
    } else {
        // ModernVolumetric (`VideoOptions::fog_quality = ModernVolumetric`, `SH2-153`):
        // Exact Beer-Lambert raymarching whose differential optical depth `d_tau = -ln((1 - f1)/(1 - f0))`
        // telescopes identically to Classic PS2 `eval_depth_curve(view_z) * effective_density` when
        // noise modulation is unity, preventing white fog-wall blowout on `ca11`/`wp01` and all rooms.
        let steps = clamp(fog.step_count, 4u, 64u);
        let z_end = min(view_z, fog.fog_far);
        let z_start = min(fog.fog_near, z_end);
        let z_span = max(0.0, z_end - z_start);
        let dz = z_span / f32(steps);
        let jitter = interleaved_gradient_noise(vec2<f32>(pixel_coords));
        let ray_per_z = world_dist / max(view_z, 1e-4);

        // Initialize transmittance at `z_start` to match `fog_min` (`sh2gfw_Set_DepthFog`).
        let f_start = clamp(eval_depth_curve(z_start) * effective_density, 0.0, 0.9995);
        var transmittance: f32 = 1.0 - f_start;

        // World-space UV frequency derived from `fog_layer2_complexity` (`0.055`, `FogFix.cpp:89`)
        // and `PS2_DEFAULT_FOG_VOLUME_HALF_EXTENT_MM` (`2000.0` mm, `fog_defset_data +0x02`).
        let noise_freq = max(0.005, fog.layer2_params.x) / 2000.0;
        let wind_drift = vec2<f32>(drift_t * 0.005, -drift_t * 0.005);

        for (var i: u32 = 0u; i < steps; i = i + 1u) {
            let z0 = z_start + f32(i) * dz;
            let z1 = z0 + dz;
            let z_sample = z0 + jitter * dz;
            let sample_world = cam_pos + ray_dir * (z_sample * ray_per_z);

            // Exact Beer-Lambert differential optical depth matching Classic PS2 curve on `[z0, z1]`.
            let f0 = clamp(eval_depth_curve(z0) * effective_density, 0.0, 0.9995);
            let f1 = clamp(eval_depth_curve(z1) * effective_density, 0.0, 0.9995);
            let d_tau = max(0.0, log((1.0 - f0) / max(1e-4, 1.0 - f1)));

            // Mean-preserving 3D mist density modulation around 1.0 using `fog.tex` and
            // `fog_transparency_layer2 / 255.0` (`112/255`, `FogFix.cpp` / `Settings.h:184`).
            let sample_uv = sample_world.xz * noise_freq + wind_drift;
            let local_n = textureSampleLevel(fog_noise_tex, fog_noise_sampler, sample_uv, 0.0).r;
            let noise_mod = select(1.0, 1.0 + (local_n - 0.5) * fog.layer_transparencies.y, outdoor_enabled);
            let step_tau = d_tau * noise_mod;

            // Flashlight volumetric inscattering / shadow-tested god rays weighted by physical `step_tau`.
            if (volumetric_light_enabled) {
                let scatter = eval_spotlight_scatter(sample_world, ray_dir);
                inscatter_rgb = inscatter_rgb + transmittance * scatter * step_tau;
            }

            transmittance = transmittance * exp(-step_tau);
        }

        fog_factor = clamp(1.0 - transmittance, 0.0, 1.0);
    }

    // Outdoor sky/horizon pixels at far depth (depth == 1.0) blend fully into mist color.
    // All `.map` batches (`stride == 0x14..0x24`, including `ob01.map` Toluca Lake & hills)
    // receive depth fog per `sh2pc.exe` `VS[1..4]` (`0x008ef700..0x008ef940` `oFog.x`)
    // and `FUN_005b4cc0` (`0x005b4cc0`, `D3DRS_FOGENABLE = 1`).
    if (depth_val >= 0.99999 && effective_density > 0.5 && outdoor_enabled) {
        fog_factor = 1.0;
    }

    var fogged_rgb = mix(original_color.rgb, coupled_fog_rgb, fog_factor) + inscatter_rgb;
    if (screen_fog_alpha > 0.001) {
        // Exact 1:1 `fog_view_screen_fog` (`SLUS_202.28` `0x0016c1c0`) / `sh2pc.exe` `FUN_00487210`
        // (`0x00487210`, `sh2pc_00480000.c:4366`): fullscreen scrolling `fog.tex` quad modulated by
        // `0x808080` (`MODULATE2X` -> `fog.tex` RGB, NOT dark `0x46` `FOGCOL`) with `ALPHA_1 = 0x44`
        // (`As = (fog_color_a * fog_alpha_param) >> 9`).
        // ClassicPs2 uses pure vertex alpha `screen_fog_alpha`; Enhanced modulates by `screen_fog_texel`.
        let screen_fog_rgb = vec3<f32>(screen_fog_texel);
        let screen_blend = select(
            clamp(screen_fog_alpha * screen_fog_texel, 0.0, 1.0),
            clamp(screen_fog_alpha, 0.0, 1.0),
            is_classic_ps2,
        );
        fogged_rgb = mix(fogged_rgb, screen_fog_rgb, screen_blend);
    }
    textureStore(color_out, pixel_coords, vec4<f32>(fogged_rgb, 1.0));
}
