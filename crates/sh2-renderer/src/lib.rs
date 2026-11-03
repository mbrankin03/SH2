//! sh2-renderer: Modern wgpu/Vulkan pipeline, 32-bit float depth, volumetric compute fog.

pub mod fog;
pub mod pipeline;

#[derive(Debug)]
pub struct RenderContext {
    pub surface_format: wgpu::TextureFormat,
}

impl RenderContext {
    pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
}
