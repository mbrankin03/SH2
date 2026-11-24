// Toluca Lake water surface shader (`SH2-80`).
//
// Strictly separated modes:
// - `enable_water_reflections == 0.0` & `enable_oar_ripples == 0.0` (Strict 1:1 Vanilla PS2):
//   Uses `WaterGridSim` (`hh_class_water_00.c` `Object_Motion_00` `0x0025ba70`, 48x48 grid @ 500 mm)
//   finite-difference heights & normals + `Specular_Calculator` (`0x0025bb80`, cos_cutoff = 0.939692) + fog.
// - `enable_water_reflections > 0.5` / `enable_oar_ripples > 0.5` (Native-Upgrade):
//   Samples planar `reflection_tex` perturbed by `WaterGridSim` + oar ripple normals and Fresnel blend.

struct WaterUniforms {
    view_proj: mat4x4<f32>,
    cam_pos: vec4<f32>,
    light_dir: vec4<f32>,
    water_tint: vec4<f32>,
    fog_color: vec4<f32>,
    /// x = water_height_mm, y = grid_origin_x_mm, z = enable_water_reflections (0/1), w = enable_oar_ripples (0/1)
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
@group(0) @binding(4) var<storage, read> grid_heights: array<f32, 2304>;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) grid_xy: vec2<u32>,
};

struct VertexOutput {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
};

fn sample_grid_height(gx: u32, gz: u32) -> f32 {
    let cx = min(gx, 47u);
    let cz = min(gz, 47u);
    return grid_heights[cx + cz * 48u];
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let gx = in.grid_xy.x;
    let gz = in.grid_xy.y;
    let h = sample_grid_height(gx, gz);
    let h_l = sample_grid_height(max(gx, 1u) - 1u, gz);
    let h_r = sample_grid_height(min(gx + 1u, 47u), gz);
    let h_d = sample_grid_height(gx, max(gz, 1u) - 1u);
    let h_u = sample_grid_height(gx, min(gz + 1u, 47u));

    // `hh_class_water_00.c` `Object_Draw` (`0x0025bd00`): 500.0 mm grid spacing
    let world = vec3<f32>(in.position.x, u_water.params.x + h, in.position.z);
    out.world_pos = world;
    out.normal = normalize(vec3<f32>(h_l - h_r, 1000.0, h_d - h_u));
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

    var normal = normalize(in.normal);
    if (u_water.params.w > 0.5) {
        let ripple_n = eval_ripple_normal(in.world_pos.xz);
        normal = normalize(normal + ripple_n - vec3<f32>(0.0, 1.0, 0.0));
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
