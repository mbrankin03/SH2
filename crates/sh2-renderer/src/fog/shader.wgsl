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
};

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

// Evaluate PS2 depth-fog curve (`sh2gfw_Set_DepthFog` @ 0x00175df0, `dbg_fogparm_ctl`)
fn eval_depth_curve(dist_mm: f32) -> f32 {
    let span = max(1e-4, fog.fog_far - fog.fog_near);
    let linear_t = clamp((dist_mm - fog.fog_near) / span, 0.0, 1.0);
    if (fog.curve_type == 1u) {
        return 1.0 - exp(-3.0 * linear_t);
    } else if (fog.curve_type == 2u) {
        return 1.0 - exp(-4.0 * linear_t * linear_t);
    }
    return linear_t;
}

// Sample dual-layer UV-scrolling PS2 fog planes (`fog_view_screen_fog` @ 0x0016c1c0, `FogFix.cpp`)
fn sample_dual_layer_planes(screen_uv: vec2<f32>, world_xz_mm: vec2<f32>, drift_t: f32) -> f32 {
    let l1_scale_a = fog.layer1_uv_scale.xy;
    let l1_scale_b = fog.layer1_uv_scale.zw;
    // Continuous shearing drift along X and Y decoupled from framerate (`FogSpeedFix`)
    let uv1_a = screen_uv * (l1_scale_a * 4.0) + vec2<f32>(drift_t * 0.028, drift_t * 0.011);
    let uv1_b = screen_uv * (l1_scale_b * 4.0) + vec2<f32>(-drift_t * 0.019, drift_t * 0.016);
    let n1_a = textureSampleLevel(fog_noise_tex, fog_noise_sampler, uv1_a, 0.0).r;
    let n1_b = textureSampleLevel(fog_noise_tex, fog_noise_sampler, uv1_b, 0.0).r;
    let layer1 = 0.5 * (n1_a + n1_b) * fog.layer_transparencies.x;

    // Layer 2: world-coupled shearing noise (`fog_layer2_complexity`, `density_mult`, `density_add`)
    let complexity = max(0.005, fog.layer2_params.x);
    let world_uv = world_xz_mm * (complexity * 0.0025);
    let uv2_a = world_uv + vec2<f32>(drift_t * 0.022, -drift_t * 0.014);
    let uv2_b = world_uv * 1.65 + vec2<f32>(-drift_t * 0.013, drift_t * 0.021);
    let n2 = 0.6 * textureSampleLevel(fog_noise_tex, fog_noise_sampler, uv2_a, 0.0).r
           + 0.4 * textureSampleLevel(fog_noise_tex, fog_noise_sampler, uv2_b, 0.0).r;
    let layer2_raw = clamp((n2 * fog.layer2_params.y) + (fog.layer2_params.z / 255.0) * 0.25, 0.0, 1.0);
    let layer2 = layer2_raw * fog.layer_transparencies.y;

    return clamp(layer1 + layer2 * 0.65, 0.0, 1.0);
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
    if (spot_intensity_sum <= 1e-4) {
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

    // Distance attenuation calibrated for PS2 mm coordinates
    let atten = 1.0 / (1.0 + 0.0000018 * dist * dist);
    // Forward Mie scattering phase (g = 0.55)
    let phase = hg_phase(dot(ray_dir, l_dir), 0.55);
    return spotlight.color_outer.xyz * (cone * cone * atten * shadow_vis * (0.35 + phase * 1.4));
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

    // Reconstruct view-space distance and world-space ray
    let uv = (vec2<f32>(pixel_coords) + 0.5) / vec2<f32>(dims);
    let clip_pos = vec4<f32>(uv.x * 2.0 - 1.0, (1.0 - uv.y) * 2.0 - 1.0, depth_val, 1.0);

    let view_pos_h = camera.inv_proj * clip_pos;
    let view_dist = length(view_pos_h.xyz / max(1e-5, abs(view_pos_h.w)));

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

    // Dual-layer PS2 UV-scrolling noise plane contribution
    let plane_noise = select(0.0, sample_dual_layer_planes(uv, world_pos.xz, drift_t), outdoor_enabled);

    var fog_factor: f32 = 0.0;
    var inscatter_rgb = vec3<f32>(0.0);

    if (!volumetric_mode || fog.step_count <= 1u) {
        // 1:1 Classic PS2 analytical depth-fog curve + dual-layer UV-scrolling noise planes
        let base_curve = eval_depth_curve(view_dist);
        let layered = clamp(base_curve + plane_noise * base_curve * 0.42, 0.0, 1.0);
        fog_factor = clamp(layered * effective_density, 0.0, 1.0);
    } else {
        // Discrete-step 3D Volumetric Raymarching (`step_count` steps)
        let steps = clamp(fog.step_count, 4u, 64u);
        let march_end = min(view_dist, fog.fog_far);
        let march_start = min(fog.fog_near * 0.15, march_end);
        let march_len = max(0.0, march_end - march_start);
        let step_len = march_len / f32(steps);
        let jitter = interleaved_gradient_noise(vec2<f32>(pixel_coords));

        var transmittance: f32 = 1.0;
        let inv_span = 1.0 / max(1e-4, fog.fog_far - fog.fog_near);

        for (var i: u32 = 0u; i < steps; i = i + 1u) {
            let t = march_start + (f32(i) + jitter) * step_len;
            let sample_world = cam_pos + ray_dir * t;

            // Local 3D animated mist density modulated by dual-layer scrolling noise
            let sample_uv = sample_world.xz * (max(0.005, fog.layer2_params.x) * 0.0018)
                          + vec2<f32>(drift_t * 0.025, -drift_t * 0.015);
            let local_n = textureSampleLevel(fog_noise_tex, fog_noise_sampler, sample_uv, 0.0).r;
            // PS2 coordinate system is Y-down (negative Y is UP into the sky).
            // Mist stays dense near ground level and gently thins high above the camera.
            let height_above_cam = max(0.0, cam_pos.y - sample_world.y - 600.0);
            let height_atten = exp(-height_above_cam * 0.00010);

            // Match curve falloff derivative along raymarch steps
            let norm_t = clamp((t - fog.fog_near) * inv_span, 0.0, 1.0);
            var curve_weight = 1.0;
            if (fog.curve_type == 1u) {
                curve_weight = 1.35 * exp(-0.4 * norm_t);
            } else if (fog.curve_type == 2u) {
                curve_weight = 2.2 * norm_t;
            }
            let plane_mod = select(1.0, 0.72 + 0.56 * local_n, outdoor_enabled);
            let step_sigma = effective_density * curve_weight * plane_mod * height_atten * (step_len * inv_span);

            // Flashlight volumetric inscattering / shadow-tested god rays
            if (volumetric_light_enabled) {
                let scatter = eval_spotlight_scatter(sample_world, ray_dir);
                let scatter_weight = max(0.015, step_sigma) * (0.45 + 0.55 * local_n);
                inscatter_rgb = inscatter_rgb + transmittance * scatter * scatter_weight * 0.85;
            }

            transmittance = transmittance * exp(-step_sigma * 1.45);
        }

        // Blend raymarched volumetric extinction with exact PS2 depth curve + dual-layer planes
        let volumetric_alpha = 1.0 - transmittance;
        let analytical_curve = eval_depth_curve(view_dist);
        let combined = mix(analytical_curve, volumetric_alpha, 0.65);
        let layered = clamp(combined + plane_noise * combined * 0.35, 0.0, 1.0);
        fog_factor = clamp(layered * effective_density, 0.0, 1.0);
    }

    // Outdoor sky/horizon pixels at far depth (depth == 1.0) blend fully into mist color
    if (depth_val >= 0.99999 && effective_density > 0.5 && outdoor_enabled) {
        fog_factor = 1.0;
    }

    let fogged_rgb = mix(original_color.rgb, coupled_fog_rgb, fog_factor) + inscatter_rgb;
    textureStore(color_out, pixel_coords, vec4<f32>(fogged_rgb, original_color.a));
}
