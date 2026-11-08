//! Interactive 3D Model, Animation, and Texture Viewer / Bestiary debug controller (`SH2-36`).
//!
//! Provides a 360-degree orbit camera, smooth turntable rotation, character/enemy/boss/weapon
//! catalog hot-swapping, animation frame scrubbing, playback speed control, and shading/texture
//! inspection modes.

use glam::{Mat4, Vec3};
use serde::{Deserialize, Serialize};

use crate::subclips::{sub_clips_for_anm, ViewerSubClip};

/// Category classification for entries in the 3D Model & Bestiary Viewer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ModelCategory {
    /// Playable protagonists (James Sunderland, Maria).
    Protagonist,
    /// Story NPCs (Eddie, Laura, Mary, Angela).
    StoryNpc,
    /// Standard monsters and creatures (Lying Figure, Mannequin, Nurse, Mandarin, Creeper).
    Enemy,
    /// Major boss encounters (Pyramid Head, Flesh Lip, Abstract Daddy, Mary Boss).
    Boss,
    /// Weapons and key inventory items (Steel Pipe, Wooden Plank, Handgun, Shotgun, Rifle, Great Knife, Spray, Keys).
    WeaponOrItem,
}

impl ModelCategory {
    /// Human-readable category label for the viewer HUD.
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Protagonist => "PROTAGONIST",
            Self::StoryNpc => "STORY NPC",
            Self::Enemy => "ENEMY / CREATURE",
            Self::Boss => "BOSS",
            Self::WeaponOrItem => "WEAPON / ITEM",
        }
    }
}

/// Metadata descriptor for an inspectable 3D model in the Bestiary catalog.
#[derive(Debug, Clone)]
pub struct ModelViewerEntry {
    /// Short CLI / internal identifier (e.g. `"hhh_jms"`, `"scu"`, `"red"`).
    pub id: &'static str,
    /// Display name shown in the HUD.
    pub display_name: &'static str,
    /// Model category classification.
    pub category: ModelCategory,
    /// Candidate relative `.mdl` paths under `data/` or `sh2e/`.
    pub mdl_paths: &'static [&'static str],
    /// Candidate relative `.anm` animation clip files associated with this model.
    pub anm_paths: &'static [&'static str],
    /// Optional SH2:EE HD override texture path (`.dds`, `.png`, or `.tex`) or external texture container.
    pub hd_override_dir: Option<&'static str>,
    /// Default camera orbit distance in PS2 world units (mm).
    pub default_distance: f32,
    /// Default vertical focus offset in PS2 world units (Y is negative-up, e.g. `-900.0` = 0.9m up).
    pub default_target_y: f32,
}

/// Complete catalog of Silent Hill 2 characters, NPCs, enemies, bosses, weapons, and key items.
pub const BESTIARY_CATALOG: &[ModelViewerEntry] = &[
    // 1. Playable Protagonists
    ModelViewerEntry {
        id: "hhh_jms",
        display_name: "James Sunderland (High-LOD - hhh_jms)",
        category: ModelCategory::Protagonist,
        mdl_paths: &["data/chr/jms/hhh_jms.mdl"],
        anm_paths: &[
            "data/chr/jms/jms_basic1.anm",
            "data/demo/tenbou/hhh_jms.anm",
            "data/demo/vtr/hhh_jms.anm",
            "data/chr/jms/jms_wphand.anm",
            "data/chr/jms/jms_wppipe.anm",
            "data/chr/jms/jms_wpkaku.anm",
            "data/chr/jms/jms_wpshot.anm",
            "data/chr/jms/jms_wprifl.anm",
            "data/chr/jms/jms_wpnata.anm",
            "data/chr/jms/jms_boat.anm",
        ],
        hd_override_dir: Some("sh2e/chr/jms"),
        default_distance: 2400.0,
        default_target_y: -850.0,
    },
    ModelViewerEntry {
        id: "hll_jms",
        display_name: "James Sunderland (Gameplay LOD - hll_jms)",
        category: ModelCategory::Protagonist,
        mdl_paths: &["data/chr/jms/hll_jms.mdl", "data/chr/jms/lll_jms.mdl"],
        anm_paths: &[
            "data/chr/jms/jms_basic1.anm",
            "data/chr/jms/jms_wphand.anm",
            "data/chr/jms/jms_wppipe.anm",
            "data/chr/jms/jms_wpkaku.anm",
            "data/chr/jms/jms_wpshot.anm",
            "data/chr/jms/jms_wprifl.anm",
            "data/chr/jms/jms_wpnata.anm",
            "data/chr/jms/jms_wpsp.anm",
            "data/chr/jms/jms_boat.anm",
        ],
        hd_override_dir: Some("sh2e/chr/jms"),
        default_distance: 2400.0,
        default_target_y: -850.0,
    },
    ModelViewerEntry {
        id: "xmar",
        display_name: "Maria (Born from a Wish - lxx_mar / xmar)",
        category: ModelCategory::Protagonist,
        mdl_paths: &[
            "sh2e/chr2/mar/lxx_mar.mdl",
            "data/chr2/mar/lxx_mar.mdl",
            "sh2e/chr/mar/lll_mar.mdl",
        ],
        anm_paths: &[
            "data/chr2/mar/xmar_wpnone.anm",
            "data/chr2/mar/xmar_wpcolt.anm",
            "data/chr2/mar/xmar_wpknif.anm",
        ],
        hd_override_dir: Some("sh2e/chr2/mar"),
        default_distance: 2400.0,
        default_target_y: -820.0,
    },
    ModelViewerEntry {
        id: "hhh_mar",
        display_name: "Maria (High-LOD Cutscene - hhh_mar)",
        category: ModelCategory::Protagonist,
        mdl_paths: &["data/chr/mar/hhh_mar.mdl", "data/chr2/mar/hhh_mar.mdl"],
        anm_paths: &[
            "data/chr/mar/p_hhh_mar.anm",
            "data/demo/mar_saikai/hhh_mar.anm",
            "data/demo/gekijou_b/hhh_mar.anm",
            "data/demo/teddy_b/hhh_mar.anm",
        ],
        hd_override_dir: Some("sh2e/chr/mar"),
        default_distance: 2400.0,
        default_target_y: -820.0,
    },
    ModelViewerEntry {
        id: "lll_mar",
        display_name: "Maria (Companion LOD - lll_mar)",
        category: ModelCategory::Protagonist,
        mdl_paths: &["sh2e/chr/mar/lll_mar.mdl", "data/chr/mar/lll_mar.mdl"],
        anm_paths: &[
            "data/chr/mar/lll_mar.anm",
            "data/chr2/mar/xmar_wpnone.anm",
            "data/chr/mar/lll_mar_back.anm",
        ],
        hd_override_dir: Some("sh2e/chr/mar"),
        default_distance: 2400.0,
        default_target_y: -820.0,
    },
    // 2. Story NPCs
    ModelViewerEntry {
        id: "hhh_edi",
        display_name: "Eddie Dombrowski (High-LOD - hhh_edi)",
        category: ModelCategory::StoryNpc,
        mdl_paths: &["data/chr/edi/hhh_edi.mdl", "data/chr/edi/rhhh_edi.mdl"],
        anm_paths: &[
            "data/chr/edi/p_edi.anm",
            "data/demo/gero_edi/hhh_edi.anm",
            "data/demo/edi_korosu/hhh_edi.anm",
            "data/demo/bp_edi_1/hhh_edi.anm",
            "data/demo/bp_edi_2/hhh_edi.anm",
            "data/demo/killer_edi/hhh_edi.anm",
            "data/demo/killer_edi_e/hhh_edi.anm",
        ],
        hd_override_dir: Some("sh2e/chr/edi"),
        default_distance: 2500.0,
        default_target_y: -850.0,
    },
    ModelViewerEntry {
        id: "lll_edi",
        display_name: "Eddie Dombrowski (Gameplay & Boss - lll_edi)",
        category: ModelCategory::StoryNpc,
        mdl_paths: &["data/chr/edi/lll_edi.mdl"],
        anm_paths: &[
            "data/chr/edi/lll_edi.anm",
            "data/demo/killer_edi_d/lll_edi.anm",
        ],
        hd_override_dir: Some("sh2e/chr/edi"),
        default_distance: 2500.0,
        default_target_y: -850.0,
    },
    ModelViewerEntry {
        id: "lau",
        display_name: "Laura (lau)",
        category: ModelCategory::StoryNpc,
        mdl_paths: &["data/chr/lau/lau.mdl", "data/chr/item/rlau.mdl"],
        anm_paths: &[
            "data/chr/lau/p_lau.anm",
            "data/demo/hei_lau/lau.anm",
            "data/demo/kagikeri/lau.anm",
            "data/demo/bp_edi_1/lau.anm",
            "data/demo/bp_edi_2/lau.anm",
            "data/demo/hp_lau/lau.anm",
            "data/demo/lau_damasu/lau.anm",
            "data/demo/lau_damasu_m/lau.anm",
            "data/demo/piano_lau/lau.anm",
            "data/demo/room312/lau.anm",
            "data/demo/exit/lau.anm",
            "data/demo/mry_yarinaoshi_i/lau.anm",
        ],
        hd_override_dir: Some("sh2e/chr/lau"),
        default_distance: 2000.0,
        default_target_y: -600.0,
    },
    ModelViewerEntry {
        id: "mry",
        display_name: "Mary Shepherd-Sunderland (mry)",
        category: ModelCategory::StoryNpc,
        mdl_paths: &["data/chr/mry/mry.mdl", "data/chr/item/rmry.mdl"],
        anm_paths: &[
            "data/chr/mry/p_mry.anm",
            "data/demo/fukatsu_a/mry.anm",
            "data/demo/jisatsu_a/mry.anm",
            "data/demo/mar_isho_a/mry.anm",
            "data/demo/mry_yarinaoshi_a/mry.anm",
        ],
        hd_override_dir: Some("sh2e/chr/mry"),
        default_distance: 2400.0,
        default_target_y: -820.0,
    },
    ModelViewerEntry {
        id: "agl",
        display_name: "Angela Orosco (agl)",
        category: ModelCategory::StoryNpc,
        mdl_paths: &["data/chr/agl/agl.mdl", "data/chr/agl/ragl.mdl"],
        anm_paths: &[
            "data/chr/agl/p_agl.anm",
            "data/demo/haka_agl/agl.anm",
            "data/demo/knife_agl/agl.anm",
            "data/demo/papa_agl/agl.anm",
            "data/demo/fire_agl/agl.anm",
        ],
        hd_override_dir: Some("sh2e/chr/agl"),
        default_distance: 2400.0,
        default_target_y: -800.0,
    },
    // 3. Enemies
    ModelViewerEntry {
        id: "scu",
        display_name: "Lying Figure (scu)",
        category: ModelCategory::Enemy,
        mdl_paths: &["sh2e/chr/scu/scu.mdl", "data/chr/scu/scu.mdl"],
        anm_paths: &["data/chr/scu/scu.anm"],
        hd_override_dir: Some("sh2e/chr/scu"),
        default_distance: 2400.0,
        default_target_y: -750.0,
    },
    ModelViewerEntry {
        id: "mkn",
        display_name: "Mannequin (mkn)",
        category: ModelCategory::Enemy,
        mdl_paths: &["data/chr/mkn/mkn.mdl"],
        anm_paths: &["data/chr/mkn/mkn.anm", "data/chr/mkn/mkn_311.anm"],
        hd_override_dir: Some("sh2e/chr/mkn"),
        default_distance: 2400.0,
        default_target_y: -800.0,
    },
    ModelViewerEntry {
        id: "nse",
        display_name: "Bubble Head Nurse (Pipe - nse)",
        category: ModelCategory::Enemy,
        mdl_paths: &["data/chr/nse/nse.mdl"],
        anm_paths: &["data/chr/nse/nse.anm"],
        hd_override_dir: Some("sh2e/chr/nse"),
        default_distance: 2400.0,
        default_target_y: -800.0,
    },
    ModelViewerEntry {
        id: "xoo",
        display_name: "Bubble Head Nurse (Variant - xoo)",
        category: ModelCategory::Enemy,
        mdl_paths: &["data/chr/xoo/xoo.mdl"],
        anm_paths: &["data/chr/xoo/xoo.anm"],
        hd_override_dir: Some("sh2e/chr/xoo"),
        default_distance: 2400.0,
        default_target_y: -800.0,
    },
    ModelViewerEntry {
        id: "arm",
        display_name: "Mandarin (Underground - arm)",
        category: ModelCategory::Enemy,
        mdl_paths: &["data/chr/arm/arm.mdl"],
        anm_paths: &["data/chr/arm/arm.anm"],
        hd_override_dir: Some("sh2e/chr/arm"),
        default_distance: 2800.0,
        default_target_y: -600.0,
    },
    ModelViewerEntry {
        id: "tyu",
        display_name: "Creeper (tyu)",
        category: ModelCategory::Enemy,
        mdl_paths: &["data/chr/tyu/tyu.mdl"],
        anm_paths: &["data/chr/tyu/tyu.anm"],
        hd_override_dir: Some("sh2e/chr/tyu"),
        default_distance: 1400.0,
        default_target_y: -150.0,
    },
    // 4. Bosses
    ModelViewerEntry {
        id: "red",
        display_name: "Pyramid Head (Great Knife - red)",
        category: ModelCategory::Boss,
        mdl_paths: &["data/chr/red/red.mdl"],
        anm_paths: &["data/chr/red/red.anm", "data/chr/red/red_wlk.anm"],
        hd_override_dir: Some("sh2e/chr/red"),
        default_distance: 2800.0,
        default_target_y: -950.0,
    },
    ModelViewerEntry {
        id: "oni",
        display_name: "Pyramid Head (Spear - oni)",
        category: ModelCategory::Boss,
        mdl_paths: &["data/chr/oni/oni.mdl"],
        anm_paths: &["data/chr/oni/oni.anm"],
        hd_override_dir: Some("sh2e/chr/oni"),
        default_distance: 2800.0,
        default_target_y: -950.0,
    },
    ModelViewerEntry {
        id: "ike",
        display_name: "Flesh Lip (ike)",
        category: ModelCategory::Boss,
        mdl_paths: &["data/chr/ike/ike.mdl"],
        anm_paths: &["data/chr/ike/ike.anm"],
        hd_override_dir: Some("sh2e/chr/ike"),
        default_distance: 3200.0,
        default_target_y: -1100.0,
    },
    ModelViewerEntry {
        id: "pap",
        display_name: "Abstract Daddy (pap)",
        category: ModelCategory::Boss,
        mdl_paths: &["data/chr/pap/pap.mdl"],
        anm_paths: &["data/chr/pap/pap.anm"],
        hd_override_dir: Some("sh2e/chr/pap"),
        default_distance: 3200.0,
        default_target_y: -900.0,
    },
    ModelViewerEntry {
        id: "bos",
        display_name: "Mary / Maria Final Boss (bos)",
        category: ModelCategory::Boss,
        mdl_paths: &["data/chr/bos/bos.mdl"],
        anm_paths: &["data/chr/bos/bos.anm"],
        hd_override_dir: Some("sh2e/chr/bos"),
        default_distance: 3200.0,
        default_target_y: -1100.0,
    },
    // 5. Weapons & Key Items
    ModelViewerEntry {
        id: "wp_pipe",
        display_name: "Steel Pipe (wp_pipe)",
        category: ModelCategory::WeaponOrItem,
        mdl_paths: &["data/chr/wp/wp_pipe.mdl", "data/chr/item/x_wp_pipe.mdl"],
        anm_paths: &["data/chr/wp/jms_weapon.anm"],
        hd_override_dir: None,
        default_distance: 1200.0,
        default_target_y: -300.0,
    },
    ModelViewerEntry {
        id: "wp_kakuzai",
        display_name: "Wooden Plank (wp_kakuzai)",
        category: ModelCategory::WeaponOrItem,
        mdl_paths: &["data/chr/wp/wp_kakuzai.mdl", "data/chr/item/i_kakuzai.mdl"],
        anm_paths: &["data/chr/wp/jms_weapon.anm"],
        hd_override_dir: None,
        default_distance: 1200.0,
        default_target_y: -300.0,
    },
    ModelViewerEntry {
        id: "wp_handgun",
        display_name: "Handgun (M92F - wp_handgun)",
        category: ModelCategory::WeaponOrItem,
        mdl_paths: &["data/chr/wp/wp_handgun.mdl", "data/chr/item/i_handgun.mdl"],
        anm_paths: &["data/chr/wp/jms_weapon.anm"],
        hd_override_dir: None,
        default_distance: 900.0,
        default_target_y: -200.0,
    },
    ModelViewerEntry {
        id: "wp_shotgun",
        display_name: "Shotgun (wp_shotgun)",
        category: ModelCategory::WeaponOrItem,
        mdl_paths: &[
            "data/chr/wp/wp_shotgun.mdl",
            "data/chr/item/x_wp_shotgun.mdl",
        ],
        anm_paths: &["data/chr/wp/jms_weapon.anm"],
        hd_override_dir: None,
        default_distance: 1200.0,
        default_target_y: -250.0,
    },
    ModelViewerEntry {
        id: "wp_riflgun",
        display_name: "Hunting Rifle (wp_riflgun)",
        category: ModelCategory::WeaponOrItem,
        mdl_paths: &[
            "data/chr/wp/wp_riflgun.mdl",
            "data/chr/item/x_wp_riflgun.mdl",
        ],
        anm_paths: &["data/chr/wp/jms_weapon.anm"],
        hd_override_dir: None,
        default_distance: 1300.0,
        default_target_y: -250.0,
    },
    ModelViewerEntry {
        id: "wp_nata",
        display_name: "Great Knife (wp_nata)",
        category: ModelCategory::WeaponOrItem,
        mdl_paths: &["data/chr/wp/wp_nata.mdl"],
        anm_paths: &["data/chr/wp/jms_weapon.anm"],
        hd_override_dir: None,
        default_distance: 1800.0,
        default_target_y: -400.0,
    },
    ModelViewerEntry {
        id: "wp_sp",
        display_name: "Hyper Spray Can (wp_sp)",
        category: ModelCategory::WeaponOrItem,
        mdl_paths: &["data/chr/wp/wp_sp.mdl", "data/chr/item/x_wp_sp.mdl"],
        anm_paths: &["data/chr/wp/jms_weapon.anm"],
        hd_override_dir: None,
        default_distance: 900.0,
        default_target_y: -200.0,
    },
    ModelViewerEntry {
        id: "wp_csaw",
        display_name: "Chainsaw (wp_csaw)",
        category: ModelCategory::WeaponOrItem,
        mdl_paths: &["data/chr/wp/wp_csaw.mdl", "data/chr/item/x_wp_csaw.mdl"],
        anm_paths: &["data/chr/wp/jms_weapon.anm"],
        hd_override_dir: None,
        default_distance: 1200.0,
        default_target_y: -250.0,
    },
    ModelViewerEntry {
        id: "i_letter",
        display_name: "Mary's Letter & Envelope (Key Item - i_letter)",
        category: ModelCategory::WeaponOrItem,
        mdl_paths: &["data/chr/item/i_letter.mdl", "data/chr/item/i_radio.mdl"],
        anm_paths: &["data/chr/item/p_i_letter.anm"],
        hd_override_dir: None,
        default_distance: 800.0,
        default_target_y: -150.0,
    },
];

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

/// Texture source comparison toggle (Vanilla PS2/PC vs SH2:EE High-Definition pack).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum TextureSourceMode {
    /// Prefer SH2:EE HD replacement textures when available.
    #[default]
    EnhancedHd,
    /// Force original Vanilla PS2/PC embedded model textures.
    VanillaPs2,
}

impl TextureSourceMode {
    pub const fn toggle(self) -> Self {
        match self {
            Self::EnhancedHd => Self::VanillaPs2,
            Self::VanillaPs2 => Self::EnhancedHd,
        }
    }

    pub const fn label(&self) -> &'static str {
        match self {
            Self::EnhancedHd => "SH2:EE HD TEXTURES (ENHANCED)",
            Self::VanillaPs2 => "VANILLA PS2/PC TEXTURES (ORIGINAL)",
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
    pub const fn next(self) -> Self {
        match self {
            Self::StudioNeutral => Self::HighContrastRim,
            Self::HighContrastRim => Self::FlashlightCone,
            Self::FlashlightCone => Self::FullBright,
            Self::FullBright => Self::StudioNeutral,
        }
    }

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

/// Interactive state machine for the 3D Model, Animation, and Texture Viewer room (`SH2-36`).
#[derive(Debug, Clone)]
pub struct ModelViewerState {
    /// Index into [`BESTIARY_CATALOG`].
    pub selected_model_index: usize,
    /// Index into the active model's `anm_paths` list.
    pub selected_anm_index: usize,
    /// Optional index into the active `.anm` file's named PS2 sub-clips (`None` = play entire `.anm` track).
    pub selected_subclip_index: Option<usize>,
    /// Total animation frames in the currently loaded `.anm` file.
    pub total_frames: usize,
    /// Continuous animation playhead position in frames `[0.0, total_frames)`.
    pub current_frame: f32,
    /// Whether animation playback is currently paused for frame-by-frame scrubbing.
    pub is_paused: bool,
    /// Playback speed multiplier (`-2.0` to `2.0`, default `1.0`).
    pub playback_speed: f32,
    /// Base animation framerate (30.0 FPS standard PS2 animation rate, or active sub-clip FPS).
    pub base_fps: f32,
    /// Whether the pedestal turntable auto-rotates the model continuously.
    pub turntable_enabled: bool,
    /// Current turntable rotation angle in radians around the vertical axis.
    pub turntable_yaw: f32,
    /// Turntable angular velocity in radians per second.
    pub turntable_speed: f32,
    /// Active shading / geometry inspection mode.
    pub render_mode: ViewerRenderMode,
    /// Whether wireframe overlay is drawn simultaneously over textured/clay modes.
    pub wireframe_overlay: bool,
    /// Whether bone skeleton and joint axes are drawn simultaneously over the mesh.
    pub skeleton_overlay: bool,
    /// Texture comparison toggle (Vanilla PS2 vs SH2:EE HD).
    pub texture_source: TextureSourceMode,
    /// Active neutral stage lighting rig preset.
    pub lighting_rig: StageLightingRig,
    /// Orbit camera controller.
    pub camera: OrbitCamera,
}

impl Default for ModelViewerState {
    fn default() -> Self {
        Self::new()
    }
}

impl ModelViewerState {
    /// Initializes the Model Viewer focused on the first catalog entry (`hhh_jms`).
    pub fn new() -> Self {
        let mut camera = OrbitCamera::default();
        camera.reset_for_entry(&BESTIARY_CATALOG[0]);

        Self {
            selected_model_index: 0,
            selected_anm_index: 0,
            selected_subclip_index: None,
            total_frames: 1,
            current_frame: 0.0,
            is_paused: false,
            playback_speed: 1.0,
            base_fps: 30.0,
            turntable_enabled: false,
            turntable_yaw: 0.0,
            turntable_speed: 0.65,
            render_mode: ViewerRenderMode::Textured,
            wireframe_overlay: false,
            skeleton_overlay: false,
            texture_source: TextureSourceMode::EnhancedHd,
            lighting_rig: StageLightingRig::StudioNeutral,
            camera,
        }
    }

    /// Returns the currently selected [`ModelViewerEntry`].
    pub fn current_entry(&self) -> &'static ModelViewerEntry {
        &BESTIARY_CATALOG[self.selected_model_index % BESTIARY_CATALOG.len()]
    }

    /// Returns the relative path of the currently selected `.anm` file, if any.
    pub fn current_anm_path(&self) -> Option<&'static str> {
        let entry = self.current_entry();
        if entry.anm_paths.is_empty() {
            None
        } else {
            Some(entry.anm_paths[self.selected_anm_index % entry.anm_paths.len()])
        }
    }

    /// Returns the slice of named PS2 sub-clips defined for the active `(model, anm)` pair.
    pub fn available_subclips(&self) -> &'static [ViewerSubClip] {
        let entry = self.current_entry();
        if let Some(anm_path) = self.current_anm_path() {
            sub_clips_for_anm(entry.id, anm_path)
        } else {
            &[]
        }
    }

    /// Returns the currently selected [`ViewerSubClip`], if any.
    pub fn active_subclip(&self) -> Option<ViewerSubClip> {
        let idx = self.selected_subclip_index?;
        let subclips = self.available_subclips();
        subclips.get(idx).copied()
    }

    /// Cycles to the next named PS2 sub-clip within the active `.anm` file, wrapping back to
    /// `None` ("ALL FRAMES") after the last sub-clip.
    pub fn next_subclip(&mut self) {
        let subclips = self.available_subclips();
        if subclips.is_empty() {
            self.selected_subclip_index = None;
            self.base_fps = 30.0;
            return;
        }
        self.selected_subclip_index = match self.selected_subclip_index {
            None => Some(0),
            Some(i) if i + 1 < subclips.len() => Some(i + 1),
            Some(_) => None,
        };
        if let Some(sc) = self.active_subclip() {
            self.current_frame = sc.clip.start_frame as f32;
            self.base_fps = sc.clip.fps;
        } else {
            self.current_frame = 0.0;
            self.base_fps = 30.0;
        }
    }

    /// Cycles to the previous named PS2 sub-clip within the active `.anm` file.
    pub fn prev_subclip(&mut self) {
        let subclips = self.available_subclips();
        if subclips.is_empty() {
            self.selected_subclip_index = None;
            self.base_fps = 30.0;
            return;
        }
        self.selected_subclip_index = match self.selected_subclip_index {
            None => Some(subclips.len() - 1),
            Some(0) => None,
            Some(i) => Some(i - 1),
        };
        if let Some(sc) = self.active_subclip() {
            self.current_frame = sc.clip.start_frame as f32;
            self.base_fps = sc.clip.fps;
        } else {
            self.current_frame = 0.0;
            self.base_fps = 30.0;
        }
    }

    /// Selects a model by matching its CLI name or substring (case-insensitive).
    /// Prioritizes exact `id` match, then `id` substring match, then `display_name` substring match.
    /// Returns `true` if a matching model was found.
    pub fn select_by_name(&mut self, query: &str) -> bool {
        let q = query.trim().to_ascii_lowercase();
        if q.is_empty() {
            return false;
        }

        // 1. Exact ID match first
        for (idx, entry) in BESTIARY_CATALOG.iter().enumerate() {
            if entry.id.eq_ignore_ascii_case(&q) {
                self.set_model_index(idx);
                return true;
            }
        }

        // 2. Substring match against ID
        for (idx, entry) in BESTIARY_CATALOG.iter().enumerate() {
            if entry.id.to_ascii_lowercase().contains(&q) {
                self.set_model_index(idx);
                return true;
            }
        }

        // 3. Substring match against display name
        for (idx, entry) in BESTIARY_CATALOG.iter().enumerate() {
            if entry.display_name.to_ascii_lowercase().contains(&q) {
                self.set_model_index(idx);
                return true;
            }
        }

        false
    }

    /// Sets the active model index and resets animation clip index and camera framing.
    pub fn set_model_index(&mut self, index: usize) {
        self.selected_model_index = index % BESTIARY_CATALOG.len();
        self.selected_anm_index = 0;
        self.selected_subclip_index = None;
        self.base_fps = 30.0;
        self.current_frame = 0.0;
        self.camera.reset_for_entry(self.current_entry());
    }

    /// Cycles to the next model in the Bestiary catalog.
    pub fn next_model(&mut self) {
        let next = (self.selected_model_index + 1) % BESTIARY_CATALOG.len();
        self.set_model_index(next);
    }

    /// Cycles to the previous model in the Bestiary catalog.
    pub fn prev_model(&mut self) {
        let prev = if self.selected_model_index == 0 {
            BESTIARY_CATALOG.len() - 1
        } else {
            self.selected_model_index - 1
        };
        self.set_model_index(prev);
    }

    /// Cycles to the next `.anm` animation file for the active model.
    pub fn next_animation_clip(&mut self) {
        let count = self.current_entry().anm_paths.len().max(1);
        self.selected_anm_index = (self.selected_anm_index + 1) % count;
        self.selected_subclip_index = None;
        self.base_fps = 30.0;
        self.current_frame = 0.0;
    }

    /// Cycles to the previous `.anm` animation file for the active model.
    pub fn prev_animation_clip(&mut self) {
        let count = self.current_entry().anm_paths.len().max(1);
        if self.selected_anm_index == 0 {
            self.selected_anm_index = count - 1;
        } else {
            self.selected_anm_index -= 1;
        }
        self.selected_subclip_index = None;
        self.base_fps = 30.0;
        self.current_frame = 0.0;
    }

    /// Updates total frame count when a new `.anm` file is loaded.
    pub fn set_total_frames(&mut self, frames: usize) {
        self.total_frames = frames.max(1);
        self.current_frame = self.current_frame.rem_euclid(self.total_frames as f32);
    }

    /// Toggles animation play/pause state.
    pub fn toggle_pause(&mut self) {
        self.is_paused = !self.is_paused;
    }

    /// Steps animation playhead forward or backward by `delta_frames` (automatically pauses).
    pub fn step_frame(&mut self, delta_frames: i32) {
        self.is_paused = true;
        if let Some(sc) = self.active_subclip() {
            let start = (sc.clip.start_frame as usize).min(self.total_frames.saturating_sub(1));
            let end = (sc.clip.end_frame as usize).min(self.total_frames.saturating_sub(1));
            let span = (end.saturating_sub(start) + 1).max(1) as f32;
            let rel =
                (self.current_frame.floor() - start as f32 + delta_frames as f32).rem_euclid(span);
            self.current_frame = start as f32 + rel;
        } else {
            let total = self.total_frames.max(1) as f32;
            self.current_frame =
                (self.current_frame.floor() + delta_frames as f32).rem_euclid(total);
        }
    }

    /// Adjusts playback speed multiplier within `[-2.0, 2.0]`.
    pub fn adjust_playback_speed(&mut self, delta: f32) {
        let next = ((self.playback_speed + delta) * 10.0).round() / 10.0;
        self.playback_speed = next.clamp(-2.0, 2.0);
    }

    /// Toggles reverse playback direction.
    pub fn reverse_playback(&mut self) {
        self.playback_speed = -self.playback_speed;
    }

    /// Advances animation playhead and turntable rotation by `dt` seconds.
    pub fn tick(&mut self, dt: f32) {
        if self.turntable_enabled {
            self.turntable_yaw =
                (self.turntable_yaw + self.turntable_speed * dt) % std::f32::consts::TAU;
        }

        if !self.is_paused && self.total_frames > 1 {
            if let Some(sc) = self.active_subclip() {
                let start = (sc.clip.start_frame as usize).min(self.total_frames - 1) as f32;
                let end = (sc.clip.end_frame as usize).min(self.total_frames - 1) as f32;
                let span = (end - start).max(0.0);
                if span <= f32::EPSILON {
                    self.current_frame = start;
                } else {
                    let frame_delta = sc.clip.fps * self.playback_speed * dt;
                    let rel = (self.current_frame - start + frame_delta).rem_euclid(span);
                    self.current_frame = start + rel;
                }
            } else {
                let total = self.total_frames as f32;
                let frame_delta = self.base_fps * self.playback_speed * dt;
                self.current_frame = (self.current_frame + frame_delta).rem_euclid(total);
            }
        }
    }

    /// Returns `(frame_0, frame_1, interpolation_alpha)` for sampling the active animation track.
    pub fn sample_cursor(&self) -> (usize, usize, f32) {
        if self.total_frames <= 1 {
            return (0, 0, 0.0);
        }
        if let Some(sc) = self.active_subclip() {
            let start = (sc.clip.start_frame as usize).min(self.total_frames - 1);
            let end = (sc.clip.end_frame as usize).min(self.total_frames - 1);
            let f0 = (self.current_frame.floor() as usize).clamp(start, end);
            let f1 = if f0 >= end {
                if sc.clip.looping {
                    start
                } else {
                    end
                }
            } else {
                f0 + 1
            };
            let alpha = (self.current_frame - self.current_frame.floor()).clamp(0.0, 1.0);
            (f0, f1, alpha)
        } else {
            let f0 = (self.current_frame.floor() as usize) % self.total_frames;
            let f1 = (f0 + 1) % self.total_frames;
            let alpha = (self.current_frame - self.current_frame.floor()).clamp(0.0, 1.0);
            (f0, f1, alpha)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_catalog_completeness_and_lookup() {
        let mut viewer = ModelViewerState::new();
        assert_eq!(viewer.current_entry().id, "hhh_jms");

        // Verify protagonists, NPCs, enemies, bosses, and weapons exist in catalog
        assert!(viewer.select_by_name("hhh_jms"));
        assert_eq!(viewer.current_entry().category, ModelCategory::Protagonist);

        assert!(viewer.select_by_name("xmar"));
        assert_eq!(viewer.current_entry().id, "xmar");

        assert!(viewer.select_by_name("lau"));
        assert_eq!(viewer.current_entry().category, ModelCategory::StoryNpc);

        assert!(viewer.select_by_name("scu"));
        assert_eq!(viewer.current_entry().category, ModelCategory::Enemy);

        assert!(viewer.select_by_name("red"));
        assert_eq!(viewer.current_entry().category, ModelCategory::Boss);

        assert!(viewer.select_by_name("pipe"));
        assert_eq!(viewer.current_entry().category, ModelCategory::WeaponOrItem);
    }

    #[test]
    fn test_animation_scrubbing_and_speed() {
        let mut viewer = ModelViewerState::new();
        viewer.set_total_frames(100);

        // Advance 1 second at 1.0x (30 fps) -> frame 30.0
        viewer.tick(1.0);
        assert!((viewer.current_frame - 30.0).abs() < 0.01);

        // Step frame pauses and moves playhead by 1 frame
        viewer.step_frame(1);
        assert!(viewer.is_paused);
        assert_eq!(viewer.current_frame, 31.0);

        // Step backward across 0 wraps to 99
        viewer.current_frame = 0.0;
        viewer.step_frame(-1);
        assert_eq!(viewer.current_frame, 99.0);

        // Reverse playback
        viewer.is_paused = false;
        viewer.reverse_playback();
        assert_eq!(viewer.playback_speed, -1.0);
        viewer.tick(0.5); // -15 frames -> 84.0
        assert!((viewer.current_frame - 84.0).abs() < 0.01);
    }

    #[test]
    fn test_orbit_camera_and_render_modes() {
        let mut viewer = ModelViewerState::new();
        let initial_dist = viewer.camera.distance;
        viewer.camera.zoom(-400.0);
        assert_eq!(viewer.camera.distance, initial_dist - 400.0);

        viewer.camera.orbit(0.5, 0.25);
        let view = viewer.camera.view_matrix();
        assert!(!view.is_nan());

        assert_eq!(viewer.render_mode, ViewerRenderMode::Textured);
        viewer.render_mode = viewer.render_mode.next();
        assert_eq!(viewer.render_mode, ViewerRenderMode::Wireframe);
        viewer.render_mode = viewer.render_mode.next();
        assert_eq!(viewer.render_mode, ViewerRenderMode::SkeletonJoints);
        viewer.render_mode = viewer.render_mode.next();
        assert_eq!(viewer.render_mode, ViewerRenderMode::UntexturedClay);
        assert_eq!(viewer.render_mode.shader_mode_code(), 1);
    }

    #[test]
    fn test_sh2_34_npc_subclip_cycling_and_playback_bounds() {
        let mut viewer = ModelViewerState::new();
        assert!(viewer.select_by_name("agl"));
        viewer.set_total_frames(298);
        assert_eq!(viewer.available_subclips().len(), 4);
        assert!(viewer.active_subclip().is_none());

        // Cycle to first sub-clip (3904: Idle Breathe, 133..=206 @ 1.875 fps)
        viewer.next_subclip();
        let sc0 = viewer.active_subclip().expect("subclip 0 active");
        assert_eq!(sc0.clip.start_frame, 133);
        assert_eq!(sc0.clip.end_frame, 206);
        assert!((viewer.base_fps - 1.875).abs() < 1e-4);

        // Cycle to second sub-clip (3901: Walk Cycle A, 0..=63 @ 30.0 fps)
        viewer.next_subclip();
        let sc1 = viewer.active_subclip().expect("subclip 1 active");
        assert_eq!(sc1.clip.start_frame, 0);
        assert_eq!(sc1.clip.end_frame, 63);
        assert!((viewer.current_frame - 0.0).abs() < 1e-4);

        // Stepping backward from start_frame wraps to end_frame within the sub-clip
        viewer.step_frame(-1);
        assert_eq!(viewer.current_frame, 63.0);
        let (f0, f1, _) = viewer.sample_cursor();
        assert_eq!(f0, 63);
        assert_eq!(f1, 0);

        // Also verify Eddie Boss (`lll_edi`) exposes all 21 combat/damage PS2 sub-clips
        assert!(viewer.select_by_name("lll_edi"));
        viewer.set_total_frames(437);
        assert_eq!(viewer.available_subclips().len(), 21);
    }

    #[test]
    fn test_all_bestiary_catalog_models_and_animations_skin_cleanly() {
        use sh2_assets::formats::anm::{
            compute_bind_locals, compute_skin_matrices, evaluate_pose, parse_anm,
        };
        use sh2_assets::formats::mdls::parse_mdl;
        use std::path::Path;

        let root = "../../Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2";
        if !Path::new(root).exists() {
            return;
        }

        for entry in BESTIARY_CATALOG {
            let mut mdl_bytes = None;
            for &mp in entry.mdl_paths {
                let p = format!("{root}/{mp}");
                if let Ok(b) = std::fs::read(&p) {
                    mdl_bytes = Some((mp, b));
                    break;
                }
            }
            let (mdl_path, mdl_bytes) = mdl_bytes
                .unwrap_or_else(|| panic!("Catalog entry '{}' missing all .mdl files", entry.id));
            let model = parse_mdl(&mdl_bytes)
                .unwrap_or_else(|e| panic!("Failed to parse '{}': {:?}", mdl_path, e));
            let bind_locals = compute_bind_locals(&model.parents, &model.initial_matrices);

            // Check bind-pose vertex weights are normalized and bone indices are in range
            for (vi, (bones, weights)) in model
                .vertices
                .bone_indices
                .iter()
                .zip(model.vertices.weights.iter())
                .enumerate()
            {
                let sum: f32 = weights.iter().sum();
                assert!(
                    (sum - 1.0).abs() < 1e-3,
                    "Entry '{}' ({}) vertex {} weights sum to {}",
                    entry.id,
                    mdl_path,
                    vi,
                    sum
                );
                for j in 0..4 {
                    if weights[j] > 0.0 {
                        assert!(
                            (bones[j] as usize) < model.bone_count,
                            "Entry '{}' ({}) vertex {} weight[{}]={} has out-of-range bone_idx={}",
                            entry.id,
                            mdl_path,
                            vi,
                            j,
                            weights[j],
                            bones[j]
                        );
                    }
                }
            }

            // Verify every listed .anm clip parses cleanly and skins without exploding/skewed vertices
            for &ap in entry.anm_paths {
                let p = format!("{root}/{ap}");
                let anm_bytes = std::fs::read(&p)
                    .unwrap_or_else(|_| panic!("Catalog entry '{}' missing '{}'", entry.id, ap));
                let anm =
                    parse_anm(&anm_bytes, model.bone_count, &model.parents).unwrap_or_else(|e| {
                        panic!(
                            "Entry '{}' ({}, {} bones) failed to parse '{}': {:?}",
                            entry.id, mdl_path, model.bone_count, ap, e
                        )
                    });
                assert!(
                    !anm.frames.is_empty(),
                    "Entry '{}' animation '{}' produced 0 frames",
                    entry.id,
                    ap
                );

                // Check frame 0, mid-frame, and last frame for finite, bounded vertex skinning
                let check_frames = [0, anm.frames.len() / 2, anm.frames.len() - 1];
                for &fi in &check_frames {
                    let pose = evaluate_pose(&model.parents, &bind_locals, &anm.frames[fi]);
                    let skin = compute_skin_matrices(&pose, &model.initial_matrices);

                    let mut min_pt = [f32::INFINITY; 3];
                    let mut max_pt = [f32::NEG_INFINITY; 3];

                    for (pos, (bones, weights)) in model.vertices.positions.iter().zip(
                        model
                            .vertices
                            .bone_indices
                            .iter()
                            .zip(model.vertices.weights.iter()),
                    ) {
                        let mut skinned = [0.0f32; 3];
                        for j in 0..4 {
                            let w = weights[j];
                            if w > 0.0 {
                                let m = &skin[bones[j] as usize];
                                skinned[0] += w
                                    * (m[0][0] * pos[0]
                                        + m[0][1] * pos[1]
                                        + m[0][2] * pos[2]
                                        + m[0][3]);
                                skinned[1] += w
                                    * (m[1][0] * pos[0]
                                        + m[1][1] * pos[1]
                                        + m[1][2] * pos[2]
                                        + m[1][3]);
                                skinned[2] += w
                                    * (m[2][0] * pos[0]
                                        + m[2][1] * pos[1]
                                        + m[2][2] * pos[2]
                                        + m[2][3]);
                            }
                        }
                        for c in 0..3 {
                            assert!(
                                skinned[c].is_finite(),
                                "Non-finite skinned coord in '{}' / '{}' frame {}",
                                entry.id,
                                ap,
                                fi
                            );
                            min_pt[c] = min_pt[c].min(skinned[c]);
                            max_pt[c] = max_pt[c].max(skinned[c]);
                        }
                    }

                    // Ensure bounding box span of the character/item in any single frame does not explode
                    let span_x = max_pt[0] - min_pt[0];
                    let span_y = max_pt[1] - min_pt[1];
                    let span_z = max_pt[2] - min_pt[2];
                    assert!(
                        span_x < 5000.0 && span_y < 5000.0 && span_z < 5000.0,
                        "Exploding/skewed vertices detected in '{}' ('{}' frame {}): span=({:.1}, {:.1}, {:.1})",
                        entry.id,
                        ap,
                        fi,
                        span_x,
                        span_y,
                        span_z
                    );
                }
            }
        }
    }
}
