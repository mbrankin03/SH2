//! Volumetric compute fog parameters matching PS2 ground-truth registers.

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct FogUniforms {
    pub fog_color: [f32; 4],
    pub fog_near: f32,
    pub fog_far: f32,
    pub density: f32,
    pub _padding: f32,
}

impl Default for FogUniforms {
    fn default() -> Self {
        Self {
            fog_color: [0.7, 0.7, 0.75, 1.0],
            fog_near: 2.0,
            fog_far: 25.0,
            density: 1.0,
            _padding: 0.0,
        }
    }
}
