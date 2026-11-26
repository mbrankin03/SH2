// Screen blit shader with color tint / alpha modulation and UV crop uniform
struct BlitUniforms {
    color_tint: vec4<f32>,
    uv_rect: vec4<f32>,
};

@group(0) @binding(0) var src_texture: texture_2d<f32>;
@group(0) @binding(1) var src_sampler: sampler;
@group(0) @binding(2) var<uniform> uniforms: BlitUniforms;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) in_vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    let x = f32((in_vertex_index << 1u) & 2u);
    let y = f32(in_vertex_index & 2u);
    out.position = vec4<f32>(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
    out.uv = mix(uniforms.uv_rect.xy, uniforms.uv_rect.zw, vec2<f32>(x, y));
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let tex_color = textureSample(src_texture, src_sampler, in.uv);
    return vec4<f32>(tex_color.rgb * uniforms.color_tint.rgb, tex_color.a * uniforms.color_tint.a);
}

// IEC 61966-2-1 sRGB EOTF (display sRGB -> linear) for presenting `Rgba8Unorm` display-space
// `ldr_output_texture` frames onto sRGB swapchains (`Bgra8UnormSrgb` / `Rgba8UnormSrgb`) without
// requiring `TextureDescriptor::view_formats` on `STORAGE_BINDING` textures (which fails on DX12
// WARP / downlevel adapters).
fn srgb_to_linear_channel(c: f32) -> f32 {
    if c <= 0.04045 {
        return c / 12.92;
    }
    return pow((c + 0.055) / 1.055, 2.4);
}

@fragment
fn fs_main_srgb_input(in: VertexOutput) -> @location(0) vec4<f32> {
    let tex_color = textureSample(src_texture, src_sampler, in.uv);
    let linear_rgb = vec3<f32>(
        srgb_to_linear_channel(tex_color.r),
        srgb_to_linear_channel(tex_color.g),
        srgb_to_linear_channel(tex_color.b),
    );
    return vec4<f32>(linear_rgb * uniforms.color_tint.rgb, tex_color.a * uniforms.color_tint.a);
}
