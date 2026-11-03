//! Pipeline configuration for 32-bit float depth and HDR rendering.

#[derive(Debug, Default)]
pub struct PipelineConfig {
    pub enable_hdr: bool,
    pub wireframe: bool,
}
