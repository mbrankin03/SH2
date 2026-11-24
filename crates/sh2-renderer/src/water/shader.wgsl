// Toluca Lake water surface shader (`SH2-80`).
//
// Supports two strictly separated modes:
// - `enable_water_reflections == 0.0` (Strict 1:1 Vanilla PS2):
//   PS2 water tint + `Specular_Calculator` (`0x0025bb80`, cos_cutoff = 0.939692) + depth fog.
// - `enable_water_reflections > 0.5` (Native-Upgrade):
//   Planar reflection sampling perturbed by animated wave + oar ripple normals and Fresnel blend.

struct WaterUniforms {
    view_proj: mat4x4<f32>,
    cam_pos: vec4<f32>,
    light_dir: vec4<f32>,
    water_tint: vec4<f32>,
    fog_color: vec4<f32>,
    /// x = water_height_mm, y = time_secs, z = enable_water_reflections (0/1), w = enable_oar_ripples (0/1)
    params: vec4<f32>,
    /// x = fog_start, y = fog_end, z = reflection_strength, w = specular_strength
    fog_and_spec: vec4<f32>,
};

struct OarRipple {
    /// xyz = world_pos_mm, w = age_secs (negative = inactive)
    pos_age: vec4<f32>,
    /// x = radius_mm, y = amplitude, z = wavelength_mm, w = max_age_secs
    params: vec4<f32>,
};

struct RippleBuffer {
    ripples: array<OarRipple, 8>,
};

@group(0) @binding(0) var<uniform> u_water: WaterUniforms;
@group(0) @binding(1) var<uniform> u_ripples: RippleBuffer;
@group(0) @binding(2) var reflection_tex: texture_2d<f32>;
@group(0) @binding(3) var reflection_samp: sampler;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) uv: vec2<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let world = vec3<f32>(in.position.x, u_water.params.x, in.position.z);
    out.world_pos = world;
    out.uv = in.uv;
    out.clip_pos = u_water.view_proj * vec4<f32>(world, 1.0);
    return out;
}

fn eval_ripple_normal(world_xz: vec2<f32>) -> vec3<f32> {
    var d_xz = vec2<f32>(0.0, 0.0);
    if (u_water.params.w > 0.5) {
        for (var i = 0u; i < 8u; i = i + 1u) {
            let r = u_ripples.ripples[i];
            if (r.pos_age.w >= 0.0 && r.pos_age.w < r.params.w) {
                let delta = world_xz - r.pos_age.xz;
                let dist = length(delta);
                let ring_r = r.params.x;
                let band = r.params.z;
                let diff = dist - ring_r;
                if (abs(diff) < band && dist > 1.0) {
                    let fade = (1.0 - r.pos_age.w / r.params.w) * (1.0 - abs(diff) / band);
                    let phase = (diff / band) * 6.2831853;
                    let slope = cos(phase) * r.params.y * fade;
                    d_xz = d_xz + (delta / dist) * slope;
                }
            }
        }
    }
    return normalize(vec3<f32>(-d_xz.x, 1.0, -d_xz.y));
}

// `Effect2/hh_class_water_00.c` Specular_Calculator (`0x0025bb80`): cos_cutoff = 0.939692
fn ps2_specular(view_dir: vec3<f32>, light_dir: vec3<f32>, normal: vec3<f32>) -> f32 {
    let n_dot_l = abs(dot(light_dir, normal));
    let reflected = normalize(normal * (2.0 * n_dot_l) - light_dir);
    let cos_v = abs(dot(reflected, view_dir));
    if (cos_v < 0.939692) {
        return 0.0;
    }
    return 1.0 - (1.0 - cos_v) / 0.06030798;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let view_vec = u_water.cam_pos.xyz - in.world_pos;
    let dist = length(view_vec);
    let view_dir = normalize(view_vec);
    let light_dir = normalize(u_water.light_dir.xyz);

    let time = u_water.params.y;
    let wave_dx = sin(in.world_pos.x * 0.0012 + time * 1.1) * 0.025;
    let wave_dz = cos(in.world_pos.z * 0.0015 + time * 0.9) * 0.025;
    var normal = normalize(vec3<f32>(wave_dx, 1.0, wave_dz));
    if (u_water.params.w > 0.5) {
        normal = normalize(normal + eval_ripple_normal(in.world_pos.xz) - vec3<f32>(0.0, 1.0, 0.0));
    }

    var color = u_water.water_tint.rgb;
    let spec = ps2_specular(view_dir, light_dir, normal) * u_water.fog_and_spec.w;
    color = color + vec3<f32>(spec, spec, spec);

    if (u_water.params.z > 0.5) {
        let ndc = in.clip_pos.xy / vec2<f32>(textureDimensions(reflection_tex));
        let refl_uv = clamp(
            vec2<f32>(ndc.x, 1.0 - ndc.y) + normal.xz * 0.045,
            vec2<f32>(0.002, 0.002),
            vec2<f32>(0.998, 0.998)
        );
        let refl_rgb = textureSample(reflection_tex, reflection_samp, refl_uv).rgb;
        let fresnel = pow(1.0 - clamp(dot(view_dir, normal), 0.0, 1.0), 3.0);
        let blend = clamp(fresnel * u_water.fog_and_spec.z, 0.0, 0.85);
        color = mix(color, refl_rgb, blend);
    }

    let fog_start = u_water.fog_and_spec.x;
    let fog_end = max(u_water.fog_and_spec.y, fog_start + 1.0);
    let fog_t = clamp((dist - fog_start) / (fog_end - fog_start), 0.0, 1.0);
    let final_rgb = mix(color, u_water.fog_color.rgb, fog_t);
    return vec4<f32>(final_rgb, u_water.water_tint.a);
}
