// PS2 4-pass ping-pong screen-space soft stencil shadow blur & composite shader
// (`SLUS_202.28` `shdw_shadow_filter_main` `0x001767e0`, `shdw_Work2Work` `0x001766a0`,
//  `shdw_Composite_Shadow` `0x001761e0`).

struct BlurUniforms {
    params: vec4<f32>,
};

@group(0) @binding(0) var src_tex: texture_2d<f32>;
@group(0) @binding(1) var src_sampler: sampler;
@group(0) @binding(2) var<uniform> u_blur: BlurUniforms;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_fullscreen(@builtin(vertex_index) vid: u32) -> VsOut {
    let x = f32((vid << 1u) & 2u) * 2.0 - 1.0;
    let y = f32(vid & 2u) * 2.0 - 1.0;
    var out: VsOut;
    out.pos = vec4<f32>(x, y, 0.0, 1.0);
    out.uv = vec2<f32>(x * 0.5 + 0.5, 0.5 - y * 0.5);
    return out;
}

@fragment
fn fs_mask_extract() -> @location(0) vec4<f32> {
    return vec4<f32>(1.0, 1.0, 1.0, 1.0);
}

// PS2 GS bilinear tent filter (`shdw_Work2Work` 0x001766a0):
// 4 symmetric half-texel bilinear taps emulate the GS sub-pixel bilinear copy without net drift.
@fragment
fn fs_blur(in: VsOut) -> @location(0) vec4<f32> {
    let off = u_blur.params.xy;
    let s0 = textureSample(src_tex, src_sampler, in.uv + vec2<f32>(-off.x, -off.y)).r;
    let s1 = textureSample(src_tex, src_sampler, in.uv + vec2<f32>( off.x, -off.y)).r;
    let s2 = textureSample(src_tex, src_sampler, in.uv + vec2<f32>(-off.x,  off.y)).r;
    let s3 = textureSample(src_tex, src_sampler, in.uv + vec2<f32>( off.x,  off.y)).r;
    let m = (s0 + s1 + s2 + s3) * 0.25;
    return vec4<f32>(m, m, m, m);
}

// `shdw_Composite_Shadow` (0x001761e0) -> RGB/Alpha scaled by room shadow density.
@fragment
fn fs_composite(in: VsOut) -> @location(0) vec4<f32> {
    let mask = textureSample(src_tex, src_sampler, in.uv).r;
    let a = clamp(mask * u_blur.params.z, 0.0, 1.0);
    return vec4<f32>(a, a, a, a);
}
