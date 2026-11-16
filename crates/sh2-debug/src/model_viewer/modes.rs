//! Shading modes, texture source selectors, lighting rigs, and spherical orbit camera
//! for the 3D Model & Bestiary Viewer (`SH2-36` / `SH2-37`).

use glam::{Mat4, Vec3};
use serde::{Deserialize, Serialize};

use super::catalog::ModelViewerEntry;

/// Shading and inspection modes supported by the 3D Model Viewer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ViewerRenderMode {
    /// Standard textured shading with Key/Fill/Rim or Flashlight illumination.
    #[default]
    Textured = 0,
    /// Wireframe polygon mesh overlay on top of or replacing the surface.
    Wireframe = 1,
    /// Bone skeleton hierarchy links and local XYZ joint axes visualizer.
    SkeletonJoints = 2,
    /// Untextured neutral clay / surface normal inspection view.
    UntexturedClay = 3,
    /// Normal map / world-space surface normal RGB visualization.
    SurfaceNormals = 4,
}

impl ViewerRenderMode {
    /// All supported viewer shading modes in cycle order.
    pub const ALL: [ViewerRenderMode; 5] = [
        Self::Textured,
        Self::Wireframe,
        Self::SkeletonJoints,
        Self::UntexturedClay,
        Self::SurfaceNormals,
    ];

    /// Cycles to the next shading mode.
    pub const fn next(self) -> Self {
        match self {
            Self::Textured => Self::Wireframe,
            Self::Wireframe => Self::SkeletonJoints,
            Self::SkeletonJoints => Self::UntexturedClay,
            Self::UntexturedClay => Self::SurfaceNormals,
            Self::SurfaceNormals => Self::Textured,
        }
    }

    /// Cycles to the previous shading mode.
    pub const fn prev(self) -> Self {
        match self {
            Self::Textured => Self::SurfaceNormals,
            Self::Wireframe => Self::Textured,
            Self::SkeletonJoints => Self::Wireframe,
            Self::UntexturedClay => Self::SkeletonJoints,
            Self::SurfaceNormals => Self::UntexturedClay,
        }
    }

    /// Human-readable label for the viewer HUD.
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Textured => "TEXTURED (FULL SHADING + STAGE LIGHT)",
            Self::Wireframe => "WIREFRAME (POLYGON MESH OVERLAY)",
            Self::SkeletonJoints => "BONE SKELETON & JOINT AXES (sh2-debug)",
            Self::UntexturedClay => "UNTEXTURED CLAY (GEOMETRY RELIEF)",
            Self::SurfaceNormals => "SURFACE NORMALS (RGB VECTOR MAP)",
        }
    }

    /// Integer code passed to the WGSL fragment shader (`0` = textured, `1` = clay, `2` = normals).
    pub const fn shader_mode_code(&self) -> u32 {
        match self {
            Self::Textured | Self::Wireframe | Self::SkeletonJoints => 0,
            Self::UntexturedClay => 1,
            Self::SurfaceNormals => 2,
        }
    }
}

/// Texture source comparison selector (4K AI Master 2048x2048 vs SH2:EE HD vs Vanilla PS2/PC).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum TextureSourceMode {
    /// Prefer 4K AI Master (2048x2048) replacement textures from `mods/sh2_4k_textures`.
    #[default]
    Ai4kMaster,
    /// Prefer SH2:EE HD (`sh2e/chr/`) or 2x enhanced replacement textures.
    EnhancedHd,
    /// Force original Vanilla PS2/PC (`data/chr/`, 512x512) embedded model textures.
    VanillaPs2,
}

impl TextureSourceMode {
    /// Cycles through `Ai4kMaster -> VanillaPs2 -> EnhancedHd -> Ai4kMaster`.
    pub const fn toggle(self) -> Self {
        match self {
            Self::Ai4kMaster => Self::VanillaPs2,
            Self::VanillaPs2 => Self::EnhancedHd,
            Self::EnhancedHd => Self::Ai4kMaster,
        }
    }

    /// Toggles directly between `4K AI Master (2048x2048)` and `Vanilla PS2 (512x512)`.
    pub const fn toggle_4k_vanilla(self) -> Self {
        match self {
            Self::Ai4kMaster => Self::VanillaPs2,
            Self::VanillaPs2 | Self::EnhancedHd => Self::Ai4kMaster,
        }
    }

    /// Human-readable label for the viewer HUD.
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Ai4kMaster => "4K AI MASTER (2048x2048 MOD PACK)",
            Self::EnhancedHd => "SH2:EE HD TEXTURES (ENHANCED)",
            Self::VanillaPs2 => "VANILLA PS2/PC TEXTURES (512x512 ORIGINAL)",
        }
    }
}

/// Lighting preset on the neutral inspection stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum StageLightingRig {
    /// Balanced 3-point studio lighting (key, warm fill, cool rim).
    #[default]
    StudioNeutral,
    /// Dramatic high-contrast rim & key spotlight.
    HighContrastRim,
    /// Classic pocket flashlight beam focused on the subject.
    FlashlightCone,
    /// Bright uniform 360-degree inspection illumination.
    FullBright,
}

impl StageLightingRig {
    /// Cycles to the next stage lighting preset.
    pub const fn next(self) -> Self {
        match self {
            Self::StudioNeutral => Self::HighContrastRim,
            Self::HighContrastRim => Self::FlashlightCone,
            Self::FlashlightCone => Self::FullBright,
            Self::FullBright => Self::StudioNeutral,
        }
    }

    /// Human-readable label for the viewer HUD.
    pub const fn label(&self) -> &'static str {
        match self {
            Self::StudioNeutral => "3-POINT STUDIO NEUTRAL (KEY + FILL + RIM)",
            Self::HighContrastRim => "HIGH-CONTRAST DRAMATIC RIM",
            Self::FlashlightCone => "POCKET FLASHLIGHT SPOTLIGHT",
            Self::FullBright => "UNIFORM FULL-BRIGHT INSPECTION",
        }
    }
}

/// 360-degree spherical orbit camera centered on the active model on the pedestal.
#[derive(Debug, Clone)]
pub struct OrbitCamera {
    /// Horizontal orbit angle in radians around the Y axis.
    pub yaw: f32,
    /// Vertical elevation angle in radians (clamped to avoid gimbal lock).
    pub pitch: f32,
    /// Radial distance from `target` in PS2 world units (mm).
    pub distance: f32,
    /// Orbit focal point $(X, Y, Z)$ in PS2 world coordinates (Y is negative-up).
    pub target: Vec3,
    /// Vertical field of view in radians.
    pub fov_y: f32,
}

impl Default for OrbitCamera {
    fn default() -> Self {
        Self {
            yaw: 0.0,
            pitch: 0.12,
            distance: 2400.0,
            target: Vec3::new(0.0, -850.0, 0.0),
            fov_y: 42.0_f32.to_radians(),
        }
    }
}

impl OrbitCamera {
    /// Resets orbit camera framing for a specific [`ModelViewerEntry`].
    pub fn reset_for_entry(&mut self, entry: &ModelViewerEntry) {
        self.yaw = 0.0;
        self.pitch = 0.10;
        self.distance = entry.default_distance;
        self.target = Vec3::new(0.0, entry.default_target_y, 0.0);
    }

    /// Rotates camera orbit angles by `(delta_yaw, delta_pitch)` in radians.
    pub fn orbit(&mut self, delta_yaw: f32, delta_pitch: f32) {
        self.yaw = (self.yaw + delta_yaw) % std::f32::consts::TAU;
        let max_pitch = 1.45; // ~83 degrees
        self.pitch = (self.pitch + delta_pitch).clamp(-max_pitch, max_pitch);
    }

    /// Zooms camera distance in or out (`delta_zoom < 0` zooms in, `> 0` zooms out).
    pub fn zoom(&mut self, delta_zoom: f32) {
        self.distance = (self.distance + delta_zoom).clamp(300.0, 9000.0);
    }

    /// Pans the camera focal target vertically and horizontally in camera-local space.
    pub fn pan(&mut self, right_delta: f32, up_delta: f32) {
        let right = Vec3::new(self.yaw.cos(), 0.0, -self.yaw.sin());
        // In PS2 world space, UP is -Y
        let up = Vec3::new(0.0, -1.0, 0.0);
        self.target += right * right_delta + up * up_delta;
    }

    /// Computes the camera eye position in PS2 world space (where $-Y$ is up).
    pub fn eye_position(&self) -> Vec3 {
        let horiz = self.distance * self.pitch.cos();
        // Pitch > 0 elevates camera upward (-Y in PS2 coordinates)
        let eye_x = self.target.x + horiz * self.yaw.sin();
        let eye_y = self.target.y - self.distance * self.pitch.sin();
        let eye_z = self.target.z + horiz * self.yaw.cos();
        Vec3::new(eye_x, eye_y, eye_z)
    }

    /// Computes the right-handed view matrix looking at `self.target` with PS2 up vector $(0, -1, 0)$.
    pub fn view_matrix(&self) -> Mat4 {
        let eye = self.eye_position();
        Mat4::look_at_rh(eye, self.target, Vec3::new(0.0, -1.0, 0.0))
    }
}
