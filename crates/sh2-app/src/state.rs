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
use crate::rooms::story_progression_room::StoryProgressionRoomState;
use crate::rooms::surface_sfx_room::SurfaceSfxRoomState;
use sh2_app::AppMenuController;
use sh2_assets::formats::anm::AnimationFile;
use sh2_audio::{AudioEngine, BgmEventFlags, BgmManager, MenuAudioBank};
use sh2_core::campaign::{CampaignOrchestrator, CampaignTickInput};
use sh2_core::{FramePacer, SimulationCore};
use sh2_debug::{
    FmvVideoSource, FmvViewerState, GrappleViewerState, JamesAnmSource, ModelViewerState,
};
use sh2_renderer::{FmvVideoPipeline, GpuMap, RenderContext, SurfaceBlitter};
use sh2_ui::menu::{LaunchTarget, TestRoomKind};
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
    pub inventory_rasterizer: TextRasterizer,
    pub inventory_texture: wgpu::Texture,
    pub inventory_bind_group: wgpu::BindGroup,
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
    pub story_progression_room: StoryProgressionRoomState,
    pub sim_core: SimulationCore,
    pub gpu_map: GpuMap,
    pub map_hd_texture_bind_groups: Vec<Arc<wgpu::BindGroup>>,
    pub map_sh2e_texture_bind_groups: Vec<Arc<wgpu::BindGroup>>,
    pub map_vanilla_texture_bind_groups: Vec<Arc<wgpu::BindGroup>>,
    pub map_texture_bind_groups: Vec<Arc<wgpu::BindGroup>>,
    pub character: Option<CharacterAsset>,
    pub maria: Option<CharacterAsset>,
    pub james_anm_cache: HashMap<JamesAnmSource, AnimationFile>,
    pub active_maria: bool,
    pub active_test_room: TestRoomKind,
    /// Active session launch target (`Campaign` vs `TestRoom`, `SH2-112`).
    pub active_launch_target: LaunchTarget,
    /// Unified Campaign Game Loop orchestrator when `active_launch_target` is `LaunchTarget::Campaign`.
    pub campaign_orchestrator: Option<CampaignOrchestrator>,
    /// Active campaign room manifest + loaded texture tier for on-demand reload when Options change.
    pub campaign_loaded_room: Option<(
        sh2_core::campaign::RoomLoadManifest,
        sh2_core::config::WorldTextureQuality,
    )>,
    /// Per-frame campaign input latch consumed by `CampaignRoomController`.
    pub campaign_pending_input: CampaignTickInput,
    pub fog_uniforms: sh2_renderer::fog::FogUniforms,
    pub fog_particles: sh2_core::fog::FogParticleVolume,
    pub fog_particle_scratch: Vec<sh2_renderer::fog::GpuFogParticle>,
    pub screen_fog_state: sh2_renderer::fog::ScreenFogState,
    pub last_screen_fog_cam: glam::Vec3,
    pub fog_blow_system: sh2_renderer::fog::FogBlowSystem,
    pub jms_spot_man: sh2_renderer::light::JmsSpotManager,
    pub char_light_pool: sh2_renderer::light::CharacterLightPool,
    pub last_fog_time: f32,
    pub last_fog_blow_tick: u64,
    pub menu_ctrl: AppMenuController,
    pub ingame_menus: crate::ingame_menus::InGameMenuState,
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
    pub key_run_held: bool,
    pub key_look_up: bool,
    pub key_look_down: bool,
    pub key_look_left: bool,
    pub key_look_right: bool,
    pub key_zoom_in: bool,
    pub key_zoom_out: bool,
    pub key_pan_up: bool,
    pub key_pan_down: bool,
    pub window_focused: bool,
    pub cursor_grabbed_for_camera: bool,
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

    /// Returns `true` when `active_test_room` is a 3D exploration/gameplay room driven by `SimulationCore` (`SH2-93`).
    pub fn is_3d_gameplay_room(&self) -> bool {
        if matches!(self.active_launch_target, LaunchTarget::Campaign { .. }) {
            return true;
        }
        !matches!(
            self.active_test_room,
            TestRoomKind::ModelAndTextureViewer
                | TestRoomKind::FmvCutsceneViewer
                | TestRoomKind::GrappleAnimationRoom
                | TestRoomKind::InventoryAndItemsRoom
                | TestRoomKind::PuzzleAndCutawayRoom
                | TestRoomKind::StoryStagesAndEndingsRoom
        )
    }

    /// Clears all held gameplay locomotion, look, sprint, and weapon-ready keyboard flags (`SH2-93`).
    pub fn clear_held_gameplay_keys(&mut self) {
        self.key_forward = false;
        self.key_backward = false;
        self.key_left = false;
        self.key_right = false;
        self.key_run_held = false;
        self.key_ready_held = false;
        self.key_attack_held = false;
        self.key_look_up = false;
        self.key_look_down = false;
        self.key_look_left = false;
        self.key_look_right = false;
        self.key_zoom_in = false;
        self.key_zoom_out = false;
        self.key_pan_up = false;
        self.key_pan_down = false;
        self.sim_core.input.clear_host_keyboard_state();
    }

    /// Synchronizes Wayland/X11 relative cursor grab and cursor visibility when `OverTheShoulder`
    /// or `FirstPerson` camera mode is active in gameplay (`SH2-93`).
    pub fn sync_cursor_grab_for_camera(&mut self) {
        let cutscene_locked = self
            .drama_demo_session
            .as_ref()
            .is_some_and(|s| s.player.locks_gameplay_input());
        let want_grab = self.window_focused
            && self.menu_ctrl.in_game_active
            && !self.menu_ctrl.pause_menu.is_open
            && self.is_3d_gameplay_room()
            && !cutscene_locked
            && self.sim_core.active_camera_mode().is_modern()
            && !self.ingame_menus.is_any_open()
            && !self.sim_core.modern_camera.handoff.locks_camera_controls();

        if want_grab == self.cursor_grabbed_for_camera {
            return;
        }
        if want_grab {
            let grabbed = self
                .window
                .set_cursor_grab(winit::window::CursorGrabMode::Locked)
                .or_else(|_| {
                    self.window
                        .set_cursor_grab(winit::window::CursorGrabMode::Confined)
                })
                .is_ok();
            if grabbed {
                self.window.set_cursor_visible(false);
            }
            self.cursor_grabbed_for_camera = true;
        } else {
            let _ = self
                .window
                .set_cursor_grab(winit::window::CursorGrabMode::None);
            self.window.set_cursor_visible(true);
            self.cursor_grabbed_for_camera = false;
        }
    }

    /// Exits the current in-game test room and returns to `target_phase` (e.g. `TitlePhase::TestRoomsSelect`)
    /// with full menu opacity, resetting held input keys and stopping room-specific audio streams.
    pub fn exit_room_to_menu(&mut self, target_phase: sh2_ui::menu::TitlePhase) {
        self.clear_held_gameplay_keys();
        self.ingame_menus.close_all();
        self.campaign_orchestrator = None;
        self.campaign_loaded_room = None;
        self.campaign_pending_input = CampaignTickInput::default();
        self.bgm_manager.stop_all(self.audio_engine.as_mut());
        if let Some(ref mut bank) = self.audio_bank {
            bank.stop_all_spatial_handles();
            bank.stop_active_cut_audio();
        }
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
        self.sync_cursor_grab_for_camera();
    }

    fn upload_hud_overlay_texture(&self) {
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
    }

    /// Computes the centered 4:3 PS2 UI viewport `(x, y, w, h)` inside the live swapchain surface
    /// so authentic PS2 menus never stretch on 16:9, 16:10, 21:9, 32:9, 4K, or resized windows.
    #[must_use]
    pub fn ps2_ui_viewport(&self) -> (f32, f32, f32, f32) {
        sh2_ui::inventory::ps2_ui_canvas_rect(
            self.surface_config.width.max(1) as f32,
            self.surface_config.height.max(1) as f32,
        )
    }

    /// Blits a full-screen semi-transparent black dim scrim (`alpha` in `0..=255`) across the
    /// entire swapchain (edge-to-edge on 16:9, 21:9, 32:9, and 4K windows) with no 4:3 seam.
    pub fn draw_fullscreen_dim_scrim(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        surface_view: &wgpu::TextureView,
        alpha: u8,
    ) {
        let (chunks, _) = self.menu_rasterizer.pixels.as_chunks_mut::<4>();
        for px in chunks {
            *px = [0, 0, 0, alpha];
        }
        self.upload_hud_overlay_texture();
        self.blitter.render_with_bind_group_and_load_op(
            encoder,
            &self.menu_text_bind_group,
            surface_view,
            wgpu::LoadOp::Load,
        );
    }

    /// Uploads the software text rasterizer buffer to the GPU overlay texture and blits it
    /// edge-to-edge over `surface_view` (ensuring full-window campaign fades, FMV subtitles, and
    /// debug HUDs never leave 3D scene strips peeking out on non-16:9 maximized windows).
    pub fn finish_hud_overlay(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        surface_view: &wgpu::TextureView,
    ) {
        self.upload_hud_overlay_texture();
        self.blitter.render_with_bind_group_and_load_op(
            encoder,
            &self.menu_text_bind_group,
            surface_view,
            wgpu::LoadOp::Load,
        );
    }

    /// Uploads the dedicated 4:3 (`1920x1440`) PS2 UI rasterizer buffer and blits it into
    /// `viewport` (`ps2_ui_canvas_rect`) with 1:1 square pixels and zero horizontal font squish.
    pub fn finish_inventory_overlay_in_viewport(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        surface_view: &wgpu::TextureView,
        viewport: (f32, f32, f32, f32),
    ) {
        self.render_ctx.queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &self.inventory_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &self.inventory_rasterizer.pixels,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * self.inventory_rasterizer.width as u32),
                rows_per_image: Some(self.inventory_rasterizer.height as u32),
            },
            wgpu::Extent3d {
                width: self.inventory_rasterizer.width as u32,
                height: self.inventory_rasterizer.height as u32,
                depth_or_array_layers: 1,
            },
        );
        self.blitter.render_with_bind_group_in_viewport(
            encoder,
            &self.inventory_bind_group,
            surface_view,
            wgpu::LoadOp::Load,
            viewport,
        );
    }

    /// Synchronizes all active map, character, enemy, and cutscene texture bind groups with
    /// the given [`sh2_core::config::WorldTextureQuality`] (`Ai4kMaster` / `Sh2eeHd` / `OriginalPs2`).
    pub fn apply_texture_quality(&mut self, quality: sh2_core::config::WorldTextureQuality) {
        self.map_texture_bind_groups = match quality {
            sh2_core::config::WorldTextureQuality::Ai4kMaster => {
                self.map_hd_texture_bind_groups.clone()
            }
            sh2_core::config::WorldTextureQuality::Sh2eeHd => {
                if !self.map_sh2e_texture_bind_groups.is_empty() {
                    self.map_sh2e_texture_bind_groups.clone()
                } else {
                    self.map_hd_texture_bind_groups.clone()
                }
            }
            sh2_core::config::WorldTextureQuality::OriginalPs2 => {
                if !self.map_vanilla_texture_bind_groups.is_empty() {
                    self.map_vanilla_texture_bind_groups.clone()
                } else {
                    self.map_hd_texture_bind_groups.clone()
                }
            }
        };

        let sync_char = |c: &mut Option<CharacterAsset>| {
            if let Some(asset) = c.as_mut() {
                asset.texture_bind_groups = match quality {
                    sh2_core::config::WorldTextureQuality::Ai4kMaster => {
                        asset.hd_texture_bind_groups.clone()
                    }
                    sh2_core::config::WorldTextureQuality::Sh2eeHd => {
                        asset.sh2e_texture_bind_groups.clone()
                    }
                    sh2_core::config::WorldTextureQuality::OriginalPs2 => {
                        asset.vanilla_texture_bind_groups.clone()
                    }
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
            let chosen_stage_bgs = match quality {
                sh2_core::config::WorldTextureQuality::Ai4kMaster => {
                    session.stage_hd_texture_bind_groups.clone()
                }
                sh2_core::config::WorldTextureQuality::Sh2eeHd => session
                    .stage_sh2e_texture_bind_groups
                    .clone()
                    .or_else(|| session.stage_hd_texture_bind_groups.clone()),
                sh2_core::config::WorldTextureQuality::OriginalPs2 => session
                    .stage_vanilla_texture_bind_groups
                    .clone()
                    .or_else(|| session.stage_hd_texture_bind_groups.clone()),
            };
            if chosen_stage_bgs.is_some() {
                session.stage_texture_bind_groups = chosen_stage_bgs;
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
            | TestRoomKind::HorrorTriggersRoom
            | TestRoomKind::StoryStagesAndEndingsRoom => 0,
        };
        (stage_id, flags)
    }
}
