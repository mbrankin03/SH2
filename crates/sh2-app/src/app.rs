//! Application lifecycle state and multi-swapchain controller.

pub struct SilentHill2App {
    pub is_running: bool,
}

impl Default for SilentHill2App {
    fn default() -> Self {
        Self { is_running: true }
    }
}

impl SilentHill2App {
    pub fn new() -> Self {
        Self::default()
    }
}
