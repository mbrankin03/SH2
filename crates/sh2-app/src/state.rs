//! Shared desktop application runtime state (`AppState`).

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use winit::window::Window;

use crate::assets::{ActiveDramaDemoSession, CharacterAsset};
use crate::rooms::audio_room::AudioDebugState;
use crate::rooms::fmv_room::ActiveFmvSession;
use crate::rooms::horror_triggers_room::HorrorTriggersRoomState;
use crate::rooms::inventory_room::InventoryDebugRoomState;
use crate::rooms::puzzle_cutaway_room::PuzzleCutawayRoomState;
use crate::rooms::surface_sfx_room::SurfaceSfxRoomState;
use sh2_app::AppMenuController;
use sh2_assets::formats::anm::AnimationFile;
use sh2_audio::{AudioEngine, BgmEventFlags, BgmManager, MenuAudioBank};
use sh2_core::{FramePacer, SimulationCore};
use sh2_debug::{
    FmvVideoSource, FmvViewerState, GrappleViewerState, JamesAnmSource, ModelViewerState,
};
use sh2_renderer::{FmvVideoPipeline, GpuMap, RenderContext, SurfaceBlitter};
use sh2_ui::menu::TestRoomKind;
use sh2_ui::{ButtonSpriteSheet, LoadingScreen, TextRasterizer};

pub(crate) struct AppState {
    pub window: Arc<Window>,
    pub surface: wgpu::Surface<'static>,
    pub surface_config: wgpu::SurfaceConfiguration,
    pub render_ctx: RenderContext,
    pub blitter: SurfaceBlitter,
    pub blit_bind_group: wgpu::BindGroup,
    pub splash_konami_bg: Option<Arc<wgpu::BindGroup>>,
    pub splash_title_bg: Option<Arc<wgpu::BindGroup>>,
    pub menu_uniform_buffer: wgpu::Buffer,
    pub menu_text_texture: wgpu::Texture,
    pub menu_text_bind_group: wgpu::BindGroup,
    pub menu_rasterizer: TextRasterizer,
    pub button_sprites: Option<ButtonSpriteSheet>,
    pub audio_bank: Option<MenuAudioBank>,
    pub audio_engine: Option<AudioEngine>,
    pub bgm_manager: BgmManager,
    pub bgm_stage_override: Option<u32>,
    pub audio_debug: AudioDebugState,
    pub surface_sfx_room: SurfaceSfxRoomState,
    pub inventory_room: InventoryDebugRoomState,
    pub puzzle_cutaway_room: PuzzleCutawayRoomState,
    pub horror_triggers_room: HorrorTriggersRoomState,
    pub sim_core: SimulationCore,
    pub gpu_map: GpuMap,
    pub map_hd_texture_bind_groups: Vec<Arc<wgpu::BindGroup>>,
    pub map_vanilla_texture_bind_groups: Vec<Arc<wgpu::BindGroup>>,
    pub map_texture_bind_groups: Vec<Arc<wgpu::BindGroup>>,
    pub character: Option<CharacterAsset>,
    pub maria: Option<CharacterAsset>,
    pub james_anm_cache: HashMap<JamesAnmSource, AnimationFile>,
    pub active_maria: bool,
    pub active_test_room: TestRoomKind,
    pub fog_uniforms: sh2_renderer::fog::FogUniforms,
    pub menu_ctrl: AppMenuController,
    pub combat_arena: sh2_core::combat::CombatArena,
    pub enemy_scu_asset: Option<CharacterAsset>,
    pub enemy_mkn_asset: Option<CharacterAsset>,
    pub enemy_nse_asset: Option<CharacterAsset>,
    pub enemy_red_asset: Option<CharacterAsset>,
    pub enemy_pap_asset: Option<CharacterAsset>,
    pub enemy_ike_asset: Option<CharacterAsset>,
    pub enemy_arm_asset: Option<CharacterAsset>,
    pub enemy_tyu_asset: Option<CharacterAsset>,
    pub enemy_bos_asset: Option<CharacterAsset>,
    pub key_ready_held: bool,
    pub key_attack_pressed: bool,
    pub key_attack_held: bool,
    pub combat_enemy_idx: usize,
    pub struggle_button_press_count: u32,
    pub model_viewer: ModelViewerState,
    pub grapple_viewer: GrappleViewerState,
    pub viewer_asset: Option<CharacterAsset>,
    pub loaded_viewer_model_idx: usize,
    pub loaded_viewer_anm_idx: usize,
    pub fmv_pipeline: FmvVideoPipeline,
    pub fmv_viewer: FmvViewerState,
    pub fmv_session: Option<ActiveFmvSession>,
    pub loaded_fmv_movie_idx: usize,
    pub loaded_fmv_source: FmvVideoSource,
    pub drama_demo_session: Option<ActiveDramaDemoSession>,
    pub selected_drama_demo_idx: usize,
    pub mouse_left_pressed: bool,
    pub mouse_right_pressed: bool,
    pub last_cursor_pos: Option<(f64, f64)>,
    pub key_forward: bool,
    pub key_backward: bool,
    pub key_left: bool,
    pub key_right: bool,
    pub key_zoom_in: bool,
    pub key_zoom_out: bool,
    pub key_pan_up: bool,
    pub key_pan_down: bool,
    pub last_menu_state_log: String,
    pub last_menu_revision: u64,
    pub active_audio_device: Option<String>,
    pub loading_screen: Option<LoadingScreen>,
    pub debug_collision_overlay: bool,
    pub debug_camera_hud: bool,
    pub skin_matrices_scratch: Vec<[[f32; 4]; 4]>,
    pub mirror_skin_matrices_scratch: Vec<[[f32; 4]; 4]>,
    pub frame_pacer: FramePacer,
    pub last_frame: Instant,
    pub frame_counter: u32,
    pub sim_time: f32,
}

impl AppState {
    /// Returns an immutable reference to a preloaded creature asset by its Bestiary ID (`1..=9`).
    pub fn enemy_asset(&self, creature_id: u32) -> Option<&CharacterAsset> {
        match creature_id {
            1 => self.enemy_scu_asset.as_ref(),
            2 => self.enemy_mkn_asset.as_ref(),
            3 => self.enemy_nse_asset.as_ref(),
            4 => self.enemy_red_asset.as_ref(),
            5 => self.enemy_pap_asset.as_ref(),
            6 => self.enemy_ike_asset.as_ref(),
            7 => self.enemy_arm_asset.as_ref(),
            8 => self.enemy_tyu_asset.as_ref(),
            9 => self.enemy_bos_asset.as_ref(),
            _ => None,
        }
    }

    /// Returns a mutable reference to a preloaded creature asset by its Bestiary ID (`1..=9`).
    pub fn enemy_asset_mut(&mut self, creature_id: u32) -> Option<&mut CharacterAsset> {
        match creature_id {
            1 => self.enemy_scu_asset.as_mut(),
            2 => self.enemy_mkn_asset.as_mut(),
            3 => self.enemy_nse_asset.as_mut(),
            4 => self.enemy_red_asset.as_mut(),
            5 => self.enemy_pap_asset.as_mut(),
            6 => self.enemy_ike_asset.as_mut(),
            7 => self.enemy_arm_asset.as_mut(),
            8 => self.enemy_tyu_asset.as_mut(),
            9 => self.enemy_bos_asset.as_mut(),
            _ => None,
        }
    }

    /// Resets the menu uniform tint to opaque white and clears the software text rasterizer.
    pub fn begin_hud_overlay(&mut self) {
        let tint = [1.0f32, 1.0, 1.0, 1.0];
        self.render_ctx.queue.write_buffer(
            &self.menu_uniform_buffer,
            0,
            bytemuck::cast_slice(&tint),
        );
        self.menu_rasterizer.clear();
    }

    /// Exits the current in-game test room and returns to `target_phase` (e.g. `TitlePhase::TestRoomsSelect`)
    /// with full menu opacity, resetting held input keys and stopping room-specific audio streams.
    pub fn exit_room_to_menu(&mut self, target_phase: sh2_ui::menu::TitlePhase) {
        self.key_forward = false;
        self.key_backward = false;
        self.key_left = false;
        self.key_right = false;
        self.key_zoom_in = false;
        self.key_zoom_out = false;
        self.key_pan_up = false;
        self.key_pan_down = false;
        self.bgm_manager.stop_all(self.audio_engine.as_mut());
        if let Some(ref mut engine) = self.audio_engine {
            engine.stop_bgm(std::time::Duration::from_millis(120));
            engine.stop_fmv_audio(std::time::Duration::from_millis(120));
            engine.stop_voice(std::time::Duration::from_millis(50));
            engine.set_bgm_paused(false);
            let _ = sh2_audio::play_menu_sound(
                engine,
                self.audio_bank.as_ref(),
                sh2_audio::SoundCueKind::Cancel,
            );
        }
        self.menu_ctrl.return_to_menu(target_phase);
    }

    /// Uploads the software text rasterizer buffer to the GPU overlay texture and blits it over `surface_view`.
    pub fn finish_hud_overlay(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        surface_view: &wgpu::TextureView,
    ) {
        self.render_ctx.queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &self.menu_text_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &self.menu_rasterizer.pixels,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * self.menu_rasterizer.width as u32),
                rows_per_image: Some(self.menu_rasterizer.height as u32),
            },
            wgpu::Extent3d {
                width: self.menu_rasterizer.width as u32,
                height: self.menu_rasterizer.height as u32,
                depth_or_array_layers: 1,
            },
        );

        self.blitter.render_with_bind_group_and_load_op(
            encoder,
            &self.menu_text_bind_group,
            surface_view,
            wgpu::LoadOp::Load,
        );
    }

    /// Synchronizes all active map, character, enemy, and cutscene texture bind groups with
    /// the given [`sh2_core::config::WorldTextureQuality`] (`Ai4kMaster` vs `OriginalPs2`).
    pub fn apply_texture_quality(&mut self, quality: sh2_core::config::WorldTextureQuality) {
        let use_hd = quality == sh2_core::config::WorldTextureQuality::Ai4kMaster;
        self.map_texture_bind_groups = if use_hd || self.map_vanilla_texture_bind_groups.is_empty()
        {
            self.map_hd_texture_bind_groups.clone()
        } else {
            self.map_vanilla_texture_bind_groups.clone()
        };

        let sync_char = |c: &mut Option<CharacterAsset>| {
            if let Some(asset) = c.as_mut() {
                asset.texture_bind_groups = if use_hd {
                    asset.hd_texture_bind_groups.clone()
                } else {
                    asset.vanilla_texture_bind_groups.clone()
                };
            }
        };

        sync_char(&mut self.character);
        sync_char(&mut self.maria);
        sync_char(&mut self.enemy_scu_asset);
        sync_char(&mut self.enemy_mkn_asset);
        sync_char(&mut self.enemy_nse_asset);
        sync_char(&mut self.enemy_red_asset);
        sync_char(&mut self.enemy_pap_asset);
        sync_char(&mut self.enemy_ike_asset);
        sync_char(&mut self.enemy_arm_asset);
        sync_char(&mut self.enemy_tyu_asset);
        sync_char(&mut self.enemy_bos_asset);
        sync_char(&mut self.viewer_asset);

        if let Some(ref mut session) = self.drama_demo_session {
            for actor_asset in &mut session.actor_assets {
                sync_char(actor_asset);
            }
            if use_hd {
                if let Some(ref hd) = session.stage_hd_texture_bind_groups {
                    session.stage_texture_bind_groups = Some(hd.clone());
                }
            } else if let Some(ref van) = session.stage_vanilla_texture_bind_groups {
                session.stage_texture_bind_groups = Some(van.clone());
            }
        }
    }

    /// Maps the active [`TestRoomKind`] (or user stage override) to a retail Silent Hill 2 stage ID
    /// and default [`BgmEventFlags`] so `SeBgmChange` and `SeBgmManager` stream and spatially mix
    /// the authentic stage BGM / AIX layers.
    pub fn active_bgm_stage_and_flags(&self) -> (u32, BgmEventFlags) {
        let enable_restored_cut_audio = self
            .menu_ctrl
            .options_menu
            .original_config
            .audio
            .enable_restored_cut_audio;

        if let Some(stage_override) = self.bgm_stage_override {
            let mut flags = BgmEventFlags {
                enable_restored_cut_audio,
                ..Default::default()
            };
            if stage_override == 2 {
                flags.flag_5e4 = 0x02;
            }
            return (stage_override, flags);
        }

        let mut flags = BgmEventFlags {
            enable_restored_cut_audio,
            ..Default::default()
        };
        let stage_id = match self.active_test_room {
            TestRoomKind::RestroomStart => {
                // Stage 2 (`Stg_toilet`, `er01`): set `flag_5e4 = 0x02` so `SeBgmChange` selects Page 47 (`forest/bgm_114_b.aix`, 6 layers)
                flags.flag_5e4 = 0x02;
                2
            }
            TestRoomKind::ObservationDeck | TestRoomKind::AudioDebugRoom => {
                // Stage 3 (`Stg_observation`, `ob01`): Page 47 (`forest/bgm_114_b.aix`, 6 layers with 5 spatial bounding boxes)
                3
            }
            TestRoomKind::HotelBoilerRoom => {
                // Stage 45 (`Stg_hotel_2f_b`, `ru`): Page 40 (`hotel/bgm_122.aix`, 5 layers) with active multi-channel spatial mixing
                45
            }
            TestRoomKind::CombatArena => {
                // Stage 24 (`Stg_hospital_3f_b`): Page 20 (`hospital/bgm_105.aix`, 5 layers) with dynamic Calm (0), LowHP (2) & EnemyNear (3) mixing
                24
            }
            TestRoomKind::SyntheticFogRunway | TestRoomKind::FallbackSandbox => {
                // Stage 4 (`Stg_forest`): Page 47 (`forest/bgm_114_b.aix`)
                4
            }
            TestRoomKind::ModelAndTextureViewer
            | TestRoomKind::FmvCutsceneViewer
            | TestRoomKind::GrappleAnimationRoom
            | TestRoomKind::LoadingScreenTest
            | TestRoomKind::SurfaceSfxTestRoom
            | TestRoomKind::InventoryAndItemsRoom
            | TestRoomKind::PuzzleAndCutawayRoom
            | TestRoomKind::HorrorTriggersRoom => 0,
        };
        (stage_id, flags)
    }
}
