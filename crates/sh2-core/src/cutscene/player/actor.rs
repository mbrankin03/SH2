//! Cutscene actor runtime state and `.anm`/`.cls` frame offset initialization (`Event/demoview.c`).

use glam::Vec3;

use super::super::anim_info::{
    derive_demo_actor_anm_and_cls_paths, find_drama_demo_anim_info, DramaDemoAnimInfo,
};
use super::super::catalog::DramaDemoPlayInfo;
use super::super::culling::ActorBoundingSphere;

/// Runtime state of a single actor inside an active `.dds` cutscene (`SubCharacter` in `Event/demoview.c`).
#[derive(Debug, Clone)]
pub struct CutsceneActorRuntime {
    /// Actor slot index (`0..n_chars`).
    pub slot_index: usize,
    /// Raw actor name from `.dds` header (e.g. `"hhh_jms_pos"`).
    pub actor_name: String,
    /// Resolved `DramaDemo_AnimInfo` entry if present in the 84-entry table.
    pub anim_info: Option<&'static DramaDemoAnimInfo>,
    /// Derived `.anm` relative asset path for this demo folder.
    pub anm_path: String,
    /// Derived `.cls` facial morph animation relative asset path if applicable.
    pub cls_path: Option<String>,
    /// Raw visibility flag authored in the `.dds` script (`status & 1`).
    pub script_visible: bool,
    /// Whether this actor is currently culled because it is parked outside the authored `4:3`
    /// camera frustum on a widescreen/ultrawide viewport or despawned by a scene-specific fix (`SH2-77`).
    pub culled_offscreen_4x3: bool,
    /// Effective visibility (`script_visible && !culled_offscreen_4x3`).
    pub visible: bool,
    /// World-space bounding sphere used for `4:3` off-screen frustum culling (`SH2-77`).
    pub bounding_sphere: ActorBoundingSphere,
    /// Current world-space position (`pos + offset`).
    pub position: Vec3,
    /// Current Y rotation in radians (`PI` during `DdsPlay`).
    pub yaw_radians: f32,
    /// Current animation ID (`aid`, initialized to `start_aid` and incremented on each `Key 0x11` after the first).
    pub current_aid: u16,
    /// Number of `Key 0x11` (`DramaDemoAnimationStart`) triggers processed for this actor.
    pub subclip_index: usize,
    /// Continuous 30 FPS animation frame within the current sub-clip (`scene_frame - subclip_start_frame`).
    pub local_anm_frame: f32,
    /// Optional scene-specific animation frame override (e.g. James's smooth widescreen pre-roll walk cycle
    /// in `hei_lau` during frames `172.0..240.0`, `SH2-77`).
    pub anm_frame_override: Option<f32>,
    /// Starting 30 FPS frame offset inside the actor's `.anm` file (accounts for multi-part
    /// cutscenes such as `mar_deai_e`, `papa_agl`, `piano_lau_e`, Laura's 226-frame `piano_lau_a` prefix,
    /// and NPC gameplay base clips prepended to `agl.anm`, `hhh_edi.anm`, `hhh_mar.anm`, `mry.anm`, `bos.anm`).
    pub anm_frame_offset: u16,
    /// Starting 30 FPS frame offset inside the actor's `.cls` facial morph cluster file.
    /// Unlike `.anm` files (which prepend NPC gameplay clips), `.cls` files only contain facial
    /// cutscene sub-clips (`clani_adr_list`).
    pub cls_frame_offset: u16,
}

impl CutsceneActorRuntime {
    /// Returns the continuous global 30 FPS frame index inside the actor's `.anm` file
    /// (`base_frame + anm_frame_offset`).
    #[inline]
    pub fn global_anm_frame(&self, scene_frame: f32) -> f32 {
        let base_frame = self.anm_frame_override.unwrap_or(scene_frame.max(0.0));
        base_frame + (self.anm_frame_offset as f32)
    }

    /// Returns the continuous global 30 FPS frame index inside the actor's `.cls` facial morph file
    /// (`base_frame + cls_frame_offset`).
    #[inline]
    pub fn global_cls_frame(&self, scene_frame: f32) -> f32 {
        let base_frame = self.anm_frame_override.unwrap_or(scene_frame.max(0.0));
        base_frame + (self.cls_frame_offset as f32)
    }

    /// Computes the 4x4 row-major world transformation matrix for this cutscene actor (`rot_y = PI`, `pos = position`).
    pub fn world_matrix_row_major(&self) -> [[f32; 4]; 4] {
        let (s, c) = self.yaw_radians.sin_cos();
        [
            [c, 0.0, s, self.position.x],
            [0.0, 1.0, 0.0, self.position.y],
            [-s, 0.0, c, self.position.z],
            [0.0, 0.0, 0.0, 1.0],
        ]
    }
}

/// Initializes per-actor runtime descriptors and `.anm`/`.cls` frame offsets from a `.dds` header.
pub(super) fn build_cutscene_actors(
    info: &DramaDemoPlayInfo,
    character_names: &[String],
) -> Vec<CutsceneActorRuntime> {
    let mut actors = Vec::with_capacity(character_names.len());
    for (idx, name) in character_names.iter().enumerate() {
        let anim_info = find_drama_demo_anim_info(name);
        let (anm_path, cls_path) = derive_demo_actor_anm_and_cls_paths(info.dds_path, name);
        let start_aid = anim_info.map(|a| a.start_aid).unwrap_or(0);
        let clean_actor = name.trim_matches('\0').trim();
        // 1. Sub-clip prefix shared by BOTH `.anm` and `.cls` (e.g. in `data/demo/piano_lau/`,
        // `lau.anm`, `lau.cls`, and `i_letter.anm` prepend the 226-frame `piano_lau_a` sub-clip
        // before the 3,221-frame `piano_lau_b` + `piano_lau_e` tracks `[226, 243, 720, 716, 781, 761]`).
        let extra_subclip_offset = if (info.dds_path.ends_with("piano_lau_b.dds")
            || info.dds_path.ends_with("piano_lau_e.dds")
            || info.dds_path.ends_with("piano_lau.dds"))
            && (clean_actor.eq_ignore_ascii_case("lau_pos")
                || clean_actor.eq_ignore_ascii_case("i_letter_pos"))
        {
            226u16
        } else if info.dds_path.contains("/sankaku1/")
            && clean_actor.eq_ignore_ascii_case("mkn_2_pos")
        {
            // In `data/demo/sankaku1/mkn.anm` (5,152 frames = 2,576 * 2), `mkn_1_pos` uses
            // frames [0..2576) and `mkn_2_pos` uses frames [2576..5152).
            2576u16
        } else {
            0u16
        };

        // 2. Gameplay base-clip prefix stored ONLY at the start of NPC/boss `.anm` files
        // (`dangela_anime_adr_list`, `deddie_anime_adr_list`, `dlaura_anime_adr_list`,
        // `dmaria_anime_adr_list`, `dmary_anime_adr_list`, `dboss_anime_adr_list`), which is
        // absent from `.cls` facial morph files (`clani_adr_list`).
        let anm_gameplay_prefix = if clean_actor.eq_ignore_ascii_case("agl_pos") {
            if info.dds_path.contains("/haka_agl/") {
                156u16
            } else if info.dds_path.contains("/papa_agl/") {
                69u16
            } else if info.dds_path.contains("/fire_agl/") {
                165u16
            } else {
                0u16
            }
        } else if clean_actor.eq_ignore_ascii_case("hhh_edi_pos") {
            if info.dds_path.contains("/gero_edi/") {
                72u16
            } else if info.dds_path.contains("/bp_edi_2/") {
                71u16
            } else {
                0u16
            }
        } else if clean_actor.eq_ignore_ascii_case("b_piz_pos")
            && info.dds_path.contains("/bp_edi_2/")
        {
            71u16
        } else if clean_actor.eq_ignore_ascii_case("hhh_mar_pos") {
            if info.dds_path.contains("/mar_deai/") {
                15u16
            } else if info.dds_path.contains("/mar_hagureru/") {
                72u16
            } else if info.dds_path.contains("/mar_ikiteruka/") {
                168u16
            } else if info.dds_path.contains("/reizouko/") {
                20u16
            } else {
                0u16
            }
        } else if clean_actor.eq_ignore_ascii_case("lau_pos")
            || clean_actor.eq_ignore_ascii_case("i_letter_pos")
        {
            if info.dds_path.contains("/hei_lau/") {
                73u16
            } else if info.dds_path.contains("/lau_damasu/") {
                91u16
            } else {
                0u16
            }
        } else if clean_actor.eq_ignore_ascii_case("mry_pos") {
            if info.dds_path.contains("/mry_yarinaoshi_a/")
                || info.dds_path.contains("/fukatsu_a/")
                || info.dds_path.contains("/jisatsu_a/")
            {
                67u16
            } else if info.dds_path.contains("/mar_isho_a/") {
                68u16
            } else {
                0u16
            }
        } else if clean_actor.eq_ignore_ascii_case("bos_pos")
            && (info.dds_path.contains("/mry_yarinaoshi_a/")
                || info.dds_path.contains("/fukatsu_a/")
                || info.dds_path.contains("/jisatsu_a/")
                || info.dds_path.contains("/mar_isho_a/"))
        {
            627u16
        } else {
            0u16
        };

        let cls_frame_offset = info.anm_frame_offset.saturating_add(extra_subclip_offset);
        let anm_frame_offset = cls_frame_offset.saturating_add(anm_gameplay_prefix);
        let bounding_sphere = ActorBoundingSphere::for_actor(name, anim_info);
        actors.push(CutsceneActorRuntime {
            slot_index: idx,
            actor_name: name.clone(),
            anim_info,
            anm_path,
            cls_path,
            script_visible: false,
            culled_offscreen_4x3: false,
            visible: false,
            bounding_sphere,
            position: Vec3::ZERO,
            yaw_radians: std::f32::consts::PI,
            current_aid: start_aid,
            subclip_index: 0,
            local_anm_frame: 0.0,
            anm_frame_override: None,
            anm_frame_offset,
            cls_frame_offset,
        });
    }
    actors
}
