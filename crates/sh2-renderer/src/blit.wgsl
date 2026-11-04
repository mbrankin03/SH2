// Screen blit shader to copy LDR output texture to window surface swapchain
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) in_vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    // Generate a fullscreen triangle covering NDC (-1 to 1)
    if in_vertex_index == 0u {
        out.position = vec4<f32>(-1.0, -1.0, 0.0, 1.0);
        out.uv = vec2<f32>(0.0, 1.0);
    } else if in_vertex_index == 1u {
        out.position = vec4<f32>(3.0, -1.0, 0.0, 1.0);
        out.uv = vec2<f32>(2.0, 1.0);
    } else {
        out.position = vec4<f32>(-1.0, 3.0, 0.0, 1.0);
        out.uv = vec2<f32>(0.0, -1.0);
    }
    return out;
}

@group(0) @binding(0) var src_texture: texture_2d<f32>;
@group(0) @binding(1) var src_sampler: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv = clamp(in.uv, vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 1.0));
    var color = textureSample(src_texture, src_sampler, uv);
    // If output is completely black or unrendered, fallback to dark ambient fog tone so it's not blinding white
    if color.a == 0.0 {
        return vec4<f32>(0.05, 0.05, 0.06, 1.0);
    }
    return vec4<f32>(color.rgb, 1.0);
}
