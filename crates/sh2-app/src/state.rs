//! Shared desktop application runtime state (`AppState`).

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use winit::window::Window;

use crate::assets::{
    find_map_file_for_room_for_quality, load_anm_file, load_map_for_quality_from_roots,
    ActiveDramaDemoSession, CharacterAsset, EnemyAssetCache, EnemyAssetSlot, MapTierAssets,
};
use crate::rooms::audio_room::AudioDebugState;
use crate::rooms::fmv_room::ActiveFmvSession;
use crate::rooms::horror_triggers_room::HorrorTriggersRoomState;
use crate::rooms::inventory_room::InventoryDebugRoomState;
use crate::rooms::loading_screen_room::LoadingScreenTestState;
use crate::rooms::puzzle_cutaway_room::PuzzleCutawayRoomState;
use crate::rooms::story_progression_room::StoryProgressionRoomState;
use crate::rooms::surface_sfx_room::SurfaceSfxRoomState;
use sh2_app::gamepad::GilrsGamepadPoller;
use sh2_app::AppMenuController;
use sh2_assets::formats::anm::AnimationFile;
use sh2_audio::{AudioEngine, BgmEventFlags, BgmManager, MenuAudioBank, SePoolManager};
use sh2_core::campaign::{CampaignOrchestrator, CampaignTickInput};
use sh2_core::{FramePacer, SimulationCore};
use sh2_debug::{
    FmvVideoSource, FmvViewerState, GrappleViewerState, JamesAnmSource, ModelViewerState,
};
use sh2_renderer::{FmvVideoPipeline, GpuMap, RenderContext, SurfaceBlitter};
use sh2_ui::menu::{LaunchTarget, TestRoomKind};
use sh2_ui::{ButtonSpriteSheet, LoadingScreen, TextRasterizer};

/// Identifies which map source is currently resident in `AppState::gpu_map` so
/// [`AppState::apply_texture_quality`] can reload ONLY the active tier on demand (`SH2-181`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ActiveMapSource {
    TestRoom(TestRoomKind),
    StoryCheckpoint(usize),
    Campaign,
}

pub(crate) struct AppState {
    pub window: Arc<Window>,
    pub surface: wgpu::Surface<'static>,
    pub surface_config: wgpu::SurfaceConfiguration,
    pub render_ctx: RenderContext,
    pub blitter: SurfaceBlitter,
    pub blit_bind_group: wgpu::BindGroup,
    pub splash_konami_bg: Option<Arc<wgpu::BindGroup>>,
    pub splash_title_bg: Option<Arc<wgpu::BindGroup>>,
    pub splash_title_bg_original: Option<Arc<wgpu::BindGroup>>,
    /// Boot sequence logo textures `[CesaWarning, SceWarning, KonamiLogo, SplashLogo]`.
    pub boot_logo_bgs: [Option<Arc<wgpu::BindGroup>>; 4],
    pub menu_uniform_buffer: wgpu::Buffer,
    pub title_bg_uniform_buffer: wgpu::Buffer,
    /// Double-buffered uniform slots for the 1x1 GPU dim scrim so multiple scrims within the same
    /// `CommandEncoder` (e.g. `ExitConfirm` backdrop + title fade) never clobber each other (`SH2-175`).
    pub dim_scrim_uniform_buffers: [wgpu::Buffer; 2],
    /// Pre-bound 1x1 black GPU texture + uniform buffer slots for zero-CPU dim scrims (`SH2-175`).
    pub dim_scrim_bind_groups: [wgpu::BindGroup; 2],
    /// Next dim scrim uniform slot (`0` or `1`) for [`Self::draw_fullscreen_dim_scrim`] (`SH2-175`).
    pub dim_scrim_slot: usize,
    pub menu_text_texture: wgpu::Texture,
    pub menu_text_bind_group: wgpu::BindGroup,
    pub inventory_rasterizer: TextRasterizer,
    pub inventory_texture: wgpu::Texture,
    pub inventory_bind_group: wgpu::BindGroup,
    /// Last rasterized & uploaded 64-bit state fingerprint for `inventory_rasterizer` (`SH2-177`).
    /// When the next frame's fingerprint matches, clear + rasterize + 11.06 MB `write_texture` are skipped.
    pub inventory_overlay_fingerprint: Option<u64>,
    pub menu_rasterizer: TextRasterizer,
    /// Last rasterized & uploaded 64-bit state fingerprint for `menu_rasterizer` (`SH2-177`).
    pub menu_overlay_fingerprint: Option<u64>,
    pub button_sprites: Option<ButtonSpriteSheet>,
    pub audio_bank: Option<MenuAudioBank>,
    pub audio_engine: Option<AudioEngine>,
    pub bgm_manager: BgmManager,
    pub se_pool: SePoolManager,
    /// Stage ambient/horror SFX coordinator for non-campaign direct stage / checkpoint rooms (`ca11`).
    pub standalone_stage_audio: sh2_core::campaign::StageAudioCoordinator,
    /// Environmental horror trigger state for non-campaign direct stage / checkpoint rooms.
    pub standalone_horror_triggers: sh2_core::events::horror_triggers::StageEventDispatcher,
    /// Deterministic RNG for `standalone_stage_audio` / `standalone_horror_triggers`.
    pub standalone_stage_rng: sh2_core::events::Sh2Rng,
    pub bgm_stage_override: Option<u32>,
    pub audio_debug: AudioDebugState,
    pub surface_sfx_room: SurfaceSfxRoomState,
    pub inventory_room: InventoryDebugRoomState,
    pub puzzle_cutaway_room: PuzzleCutawayRoomState,
    pub horror_triggers_room: HorrorTriggersRoomState,
    pub story_progression_room: StoryProgressionRoomState,
    pub loading_screen_test: LoadingScreenTestState,
    pub sim_core: SimulationCore,
    pub gamepad: GilrsGamepadPoller,
    pub gpu_map: Arc<GpuMap>,
    pub map_texture_bind_groups: Vec<Arc<wgpu::BindGroup>>,
    /// Multi-tile outdoor `.map` + `.cld` background streaming pool (`SH2-183`).
    pub outdoor_tile_pool: sh2_renderer::GpuOutdoorTilePool,
    /// Tracks which map source is currently loaded in `gpu_map` / `map_texture_bind_groups` (`SH2-181`).
    pub active_map_source: ActiveMapSource,
    /// Active [`sh2_core::config::WorldTextureQuality`] tier currently resident in VRAM (`SH2-181`).
    pub loaded_texture_quality: sh2_core::config::WorldTextureQuality,
    pub character: Option<CharacterAsset>,
    pub maria: Option<CharacterAsset>,
    /// On-demand cache of James stage `.anm` files (`None` = attempted and missing on disk, `SH2-181`).
    pub james_anm_cache: HashMap<JamesAnmSource, Option<AnimationFile>>,
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
    /// Cached retail `.mes` tables and active event message HUD state for Campaign mode (`SH2-157`).
    pub campaign_messages: crate::rooms::campaign_room::messages::CampaignMessageState,
    pub fog_uniforms: sh2_renderer::fog::FogUniforms,
    pub fog_particles: sh2_core::fog::FogParticleVolume,
    pub fog_particle_scratch: Vec<sh2_renderer::fog::GpuFogParticle>,
    pub screen_fog_state: sh2_renderer::fog::ScreenFogState,
    pub last_screen_fog_cam: glam::Vec3,
    pub fog_blow_system: sh2_renderer::fog::FogBlowSystem,
    pub dust_system: sh2_core::lighting::DustSystem,
    pub dust_particle_scratch: Vec<sh2_renderer::effects::GpuDustParticle>,
    pub dust_spotlight: sh2_core::lighting::DustSpotlightParams,
    pub blood_system: sh2_core::effects::BloodEffectSystem,
    pub blood_item_scratch: Vec<sh2_core::effects::BloodQuadRenderItem>,
    pub blood_quad_scratch: Vec<sh2_renderer::effects::GpuBloodQuad>,
    pub jms_spot_man: sh2_renderer::light::JmsSpotManager,
    pub char_light_pool: sh2_renderer::light::CharacterLightPool,
    pub last_fog_time: f32,
    pub last_fog_blow_tick: u64,
    pub last_dust_tick: u64,
    pub last_blood_tick: u64,
    pub menu_ctrl: AppMenuController,
    pub ingame_menus: crate::ingame_menus::InGameMenuState,
    /// Active campaign ending credits sequence (`EndingCreditsSequence`).
    pub campaign_credits_seq: Option<sh2_ui::credits::EndingCreditsSequence>,
    pub combat_arena: sh2_core::combat::CombatArena,
    /// Lazy-loaded enemy/boss models & animations evicted on stage transitions (`SH2-181`).
    pub enemy_assets: EnemyAssetCache,
    pub key_ready_held: bool,
    pub key_attack_pressed: bool,
    pub key_attack_held: bool,
    /// Rising-edge trigger for the Run / Guard key (`ShiftLeft`/`ShiftRight`; `_DAT_0116db3c` in
    /// `SLUS_202.28` `PlayerChectGuardSuccess` `0x001101d0`, `DAT_00932ca4` in `sh2pc.exe` `0x00534210`).
    pub key_guard_pressed: bool,
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
    /// Last cursor visibility passed to `window.set_cursor_visible` so Wayland/X11 is only
    /// notified on actual state transitions (`SH2-175`).
    pub last_cursor_visible: Option<bool>,
    /// Native-resolution CPU/GPU cutaway surface allocated on demand while a cutaway is active
    /// and dropped (`None`) when cutaways close or rooms transition (`SH2-175`).
    pub native_cutaway_surface: Option<crate::rooms::campaign_room::cutaway::NativeCutawaySurface>,
    pub last_menu_state_log: String,
    pub last_menu_revision: u64,
    pub active_audio_device: Option<String>,
    pub loading_screen: Option<LoadingScreen>,
    pub debug_collision_overlay: bool,
    pub debug_camera_hud: bool,
    pub skin_matrices_scratch: Vec<[[f32; 4]; 4]>,
    pub mirror_skin_matrices_scratch: Vec<[[f32; 4]; 4]>,
    /// Reusable per-frame scratch list for head-tracking look targets (`SH2-182`).
    pub look_candidates_scratch: Vec<sh2_core::LookTargetCandidate>,
    /// Dirty-check key `(cutscene_active, stage_id, room_id, flags_hash)` for
    /// `sync_map_geo_visibility_overrides` (`SH2-182`).
    pub last_map_geo_vis_key: Option<(bool, Option<u8>, u8, u64)>,
    /// Unified per-frame `CommandEncoder` shared between 3D scene/GLB passes and swapchain
    /// presentation so the entire frame submits in a single `queue.submit()` (`SH2-182`).
    pub frame_encoder: Option<wgpu::CommandEncoder>,
    pub frame_pacer: FramePacer,
    pub last_frame: Instant,
    pub frame_counter: u32,
    pub sim_time: f32,
    pub cockroach_swarm: sh2_core::CockroachSwarmState,
    pub dog_room_levers: sh2_core::DogRoomLeversState,
    pub sh2ee_ty3_glb: Option<sh2_renderer::GlbGpuModel>,
    pub sh2ee_mon_glb: Option<sh2_renderer::GlbGpuModel>,
}

impl AppState {
    /// Lazy-loads a James stage `.anm` entry into [`Self::james_anm_cache`] on first demand
    /// instead of loading all 15 `.anm` files eagerly at startup (`SH2-181`).
    pub fn ensure_james_anm(&mut self, src: JamesAnmSource) -> Option<&AnimationFile> {
        if src == JamesAnmSource::Basic {
            return self.character.as_ref().map(|c| &c.anm);
        }
        if !self.james_anm_cache.contains_key(&src) {
            let loaded = self
                .character
                .as_ref()
                .and_then(|c| load_anm_file(src.rel_path(), c.model.bone_count, &c.model.parents));
            self.james_anm_cache.insert(src, loaded);
        }
        self.james_anm_cache.get(&src).and_then(|opt| opt.as_ref())
    }

    /// Returns an immutable reference to a loaded creature asset by its Bestiary ID (`1..=9`).
    pub fn enemy_asset(&self, creature_id: u32) -> Option<&CharacterAsset> {
        let slot = EnemyAssetSlot::from_creature_id(creature_id)?;
        self.enemy_assets.get(slot)
    }

    /// Ensures the creature asset for Bestiary ID (`1..=9`) is lazy-loaded at the active
    /// texture quality tier and returns a mutable reference (`SH2-181`).
    pub fn ensure_enemy_asset_mut(&mut self, creature_id: u32) -> Option<&mut CharacterAsset> {
        let slot = EnemyAssetSlot::from_creature_id(creature_id)?;
        self.enemy_assets
            .ensure_loaded(&self.render_ctx, slot, self.loaded_texture_quality)
    }

    /// Returns an immutable reference to a loaded creature asset by [`EnemySpecies`] (`0x200..=0x20b`).
    pub fn enemy_asset_for_species(
        &self,
        species: sh2_core::ai::creatures::EnemySpecies,
        is_dark_nurse: bool,
    ) -> Option<&CharacterAsset> {
        let slot = EnemyAssetSlot::from_species(species, is_dark_nurse)?;
        self.enemy_assets.get(slot)
    }

    /// Ensures the creature asset for [`EnemySpecies`] (`0x200..=0x20b`) is lazy-loaded at the
    /// active texture quality tier and returns a mutable reference (`SH2-181`).
    pub fn ensure_enemy_asset_for_species_mut(
        &mut self,
        species: sh2_core::ai::creatures::EnemySpecies,
        is_dark_nurse: bool,
    ) -> Option<&mut CharacterAsset> {
        let slot = EnemyAssetSlot::from_species(species, is_dark_nurse)?;
        self.enemy_assets
            .ensure_loaded(&self.render_ctx, slot, self.loaded_texture_quality)
    }

    /// Resets the menu uniform tint to opaque white without clearing `menu_rasterizer` (`SH2-177`).
    pub fn reset_hud_overlay_tint(&self) {
        let tint = [1.0f32, 1.0, 1.0, 1.0];
        self.render_ctx.queue.write_buffer(
            &self.menu_uniform_buffer,
            0,
            bytemuck::cast_slice(&tint),
        );
    }

    /// Resets the menu uniform tint to opaque white, invalidates the cached HUD fingerprint,
    /// and clears the software text rasterizer.
    pub fn begin_hud_overlay(&mut self) {
        self.reset_hud_overlay_tint();
        self.menu_overlay_fingerprint = None;
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
                | TestRoomKind::LoadingScreenTest
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
        self.key_attack_pressed = false;
        self.key_attack_held = false;
        self.key_guard_pressed = false;
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

    /// Calls `window.set_cursor_visible` only when the visibility state actually changes.
    fn set_cursor_visible_if_changed(&mut self, visible: bool) {
        if self.last_cursor_visible != Some(visible) {
            self.window.set_cursor_visible(visible);
            self.last_cursor_visible = Some(visible);
        }
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

        let in_pc_menu = !self.menu_ctrl.in_game_active
            || self.menu_ctrl.pause_menu.is_open
            || self.ingame_menus.is_any_open();
        self.set_cursor_visible_if_changed(!want_grab && !in_pc_menu);
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
                self.set_cursor_visible_if_changed(false);
            }
            self.cursor_grabbed_for_camera = true;
        } else {
            let _ = self
                .window
                .set_cursor_grab(winit::window::CursorGrabMode::None);
            self.set_cursor_visible_if_changed(!in_pc_menu);
            self.cursor_grabbed_for_camera = false;
        }
    }

    /// Exits the current in-game test room and returns to `target_phase` (e.g. `TitlePhase::TestRoomsSelect`)
    /// with full menu opacity, resetting held input keys and stopping room-specific audio streams.
    pub fn exit_room_to_menu(&mut self, target_phase: sh2_ui::menu::TitlePhase) {
        self.clear_held_gameplay_keys();
        self.ingame_menus.close_all();
        self.story_progression_room.cutaway_cache.clear();
        self.native_cutaway_surface = None;
        self.sh2ee_ty3_glb = None;
        self.sh2ee_mon_glb = None;
        self.campaign_orchestrator = None;
        self.campaign_loaded_room = None;
        self.campaign_pending_input = CampaignTickInput::default();
        self.campaign_messages.reset();
        self.enemy_assets.evict_all();
        self.bgm_manager.stop_all(self.audio_engine.as_mut());
        self.se_pool.reset();
        self.standalone_stage_audio.reset();
        if let Some(ref mut bank) = self.audio_bank {
            bank.stop_all_looping_se();
            bank.stop_all_spatial_handles();
            bank.stop_active_cut_audio();
            bank.clear_non_core_voice_afs_cache();
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
    /// entire swapchain (edge-to-edge on 16:9, 21:9, 32:9, and 4K windows) with no 4:3 seam,
    /// using a 1x1 black GPU texture + uniform alpha (`SH2-175`).
    pub fn draw_fullscreen_dim_scrim(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        surface_view: &wgpu::TextureView,
        alpha: u8,
    ) {
        let uniforms = [
            1.0f32,
            1.0,
            1.0,
            f32::from(alpha) / 255.0,
            0.0,
            0.0,
            1.0,
            1.0,
        ];
        let slot = self.dim_scrim_slot;
        self.dim_scrim_slot ^= 1;
        self.render_ctx.queue.write_buffer(
            &self.dim_scrim_uniform_buffers[slot],
            0,
            bytemuck::cast_slice(&uniforms),
        );
        self.blitter.render_with_bind_group_and_load_op(
            encoder,
            &self.dim_scrim_bind_groups[slot],
            surface_view,
            wgpu::LoadOp::Load,
        );
    }

    /// Uploads (when `upload == true`) and blits the software text rasterizer buffer edge-to-edge
    /// over `surface_view` (`SH2-177`). When `upload == false`, reuses the already-uploaded GPU
    /// texture with zero CPU-to-GPU transfer.
    pub fn present_hud_overlay(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        surface_view: &wgpu::TextureView,
        upload: bool,
    ) {
        if upload {
            self.upload_hud_overlay_texture();
        }
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
        self.present_hud_overlay(encoder, surface_view, true);
    }

    /// Uploads (when `upload == true`) and blits the dedicated 4:3 (`1920x1440`) PS2 UI rasterizer
    /// buffer into `viewport` (`ps2_ui_canvas_rect`) (`SH2-177`).
    pub fn present_inventory_overlay_in_viewport(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        surface_view: &wgpu::TextureView,
        viewport: (f32, f32, f32, f32),
        upload: bool,
    ) {
        if upload {
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
        }
        self.blitter.render_with_bind_group_in_viewport(
            encoder,
            &self.inventory_bind_group,
            surface_view,
            wgpu::LoadOp::Load,
            viewport,
        );
    }

    /// Uploads the dedicated 4:3 (`1920x1440`) PS2 UI rasterizer buffer and blits it into
    /// `viewport` (`ps2_ui_canvas_rect`) with 1:1 square pixels and zero horizontal font squish.
    pub fn finish_inventory_overlay_in_viewport(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        surface_view: &wgpu::TextureView,
        viewport: (f32, f32, f32, f32),
    ) {
        self.inventory_overlay_fingerprint = None;
        self.present_inventory_overlay_in_viewport(encoder, surface_view, viewport, true);
    }

    /// Reloads ONLY the active map geometry + texture bind groups, characters, resident enemies,
    /// and active cutscene stage assets on demand when [`sh2_core::config::WorldTextureQuality`]
    /// changes (`Ai4kMaster` / `Sh2eeHd` / `OriginalPs2`), keeping only a single tier resident in
    /// VRAM (`SH2-181`).
    pub fn apply_texture_quality(&mut self, quality: sh2_core::config::WorldTextureQuality) {
        if self.loaded_texture_quality == quality {
            return;
        }

        match self.active_map_source {
            ActiveMapSource::TestRoom(room) => {
                let (map, _, _, _) = find_map_file_for_room_for_quality(room, quality);
                let assets = MapTierAssets::from_map(&self.render_ctx, map);
                self.gpu_map = assets.gpu_map;
                self.map_texture_bind_groups = assets.texture_bind_groups;
            }
            ActiveMapSource::StoryCheckpoint(cp_idx) => {
                let cp = &crate::rooms::combat_arena::story_progression_3d::STORY_3D_CHECKPOINTS
                    [cp_idx
                        % crate::rooms::combat_arena::story_progression_3d::STORY_3D_CHECKPOINT_COUNT];
                let map = load_map_for_quality_from_roots(cp.map_rel_path, cp.room_id, quality)
                    .unwrap_or_else(|| {
                        find_map_file_for_room_for_quality(cp.target_room, quality).0
                    });
                let assets = MapTierAssets::from_map(&self.render_ctx, map);
                self.gpu_map = assets.gpu_map;
                self.map_texture_bind_groups = assets.texture_bind_groups;
            }
            ActiveMapSource::Campaign => {
                if let Some((ref manifest, ref mut loaded_q)) = self.campaign_loaded_room {
                    if *loaded_q != quality {
                        if let Some(map) = load_map_for_quality_from_roots(
                            &manifest.map_rel_path,
                            &manifest.room_id,
                            quality,
                        ) {
                            let assets = MapTierAssets::from_map(&self.render_ctx, map);
                            self.gpu_map = assets.gpu_map;
                            self.map_texture_bind_groups = assets.texture_bind_groups;
                            *loaded_q = quality;
                        }
                    }
                }
            }
        }

        if let Some(ref mut asset) = self.character {
            asset.reload_textures_for_quality(&self.render_ctx, quality);
        }
        if let Some(ref mut asset) = self.maria {
            asset.reload_textures_for_quality(&self.render_ctx, quality);
        }
        if let Some(ref mut asset) = self.viewer_asset {
            asset.reload_textures_for_quality(&self.render_ctx, quality);
        }
        if let Some(ref mut asset) = self.inventory_room.item_3d_asset {
            asset.reload_textures_for_quality(&self.render_ctx, quality);
        }
        self.enemy_assets.reload_textures(&self.render_ctx, quality);
        if let Some(ref mut session) = self.drama_demo_session {
            session.reload_for_quality(&self.render_ctx, quality);
        }

        self.loaded_texture_quality = quality;
    }

    /// Maps the active Campaign [`ScenarioState`] (or [`TestRoomKind`] / user stage override)
    /// to a retail Silent Hill 2 stage ID and [`BgmEventFlags`] so `SeBgmChange` (`0x00515fd0`)
    /// and `SeBgmManager` (`0x00516430`) stream and spatially mix the authentic stage BGM / AIX layers.
    pub fn active_bgm_stage_and_flags(&self) -> (u32, BgmEventFlags) {
        let enable_restored_cut_audio = self
            .menu_ctrl
            .options_menu
            .original_config
            .audio
            .enable_restored_cut_audio;

        if matches!(self.active_launch_target, LaunchTarget::Campaign { .. })
            || self.campaign_orchestrator.is_some()
        {
            let stage_id = self
                .bgm_stage_override
                .unwrap_or_else(|| u32::from(self.sim_core.scenario.current_stage));
            // Match `sh2pc.exe` `0x00932194 & 0x40` (`EvBgmControl` event-mode gate): asserted
            // while any `DramaDemo` or stage `ev_prog` is active, and while transitioning into
            // Stage 3 (`ob01`) before `tenbou` (`flag(33)` = `0x21`) has played so `ob01` Calm
            // ambient tracks (`Tracks 2..4`) cannot bleed into `tenbou` before frame 0 (`SH2-160`).
            let in_campaign_demo_or_pending_tenbou =
                self.campaign_orchestrator.as_ref().is_some_and(|o| {
                    matches!(o.phase, sh2_core::campaign::CampaignPhase::DramaDemo { .. })
                        || (stage_id == 3
                            && !self.sim_core.scenario.has_flag(0x0021)
                            && matches!(
                                o.phase,
                                sh2_core::campaign::CampaignPhase::DoorTransition { .. }
                            ))
                });
            let event_mode_active = in_campaign_demo_or_pending_tenbou
                || self.sim_core.scenario.prog_runner.is_running()
                || self
                    .drama_demo_session
                    .as_ref()
                    .is_some_and(|s| s.player.step != sh2_core::cutscene::DramaDemoStep::Finished);
            let hotel_music_box_active = stage_id == 40
                && self
                    .sim_core
                    .scenario
                    .cutaway_overlay
                    .active_picture()
                    .is_some();
            let mut flags = BgmEventFlags::from_event_flag_bytes(
                self.sim_core.scenario.flags.as_bytes(),
                hotel_music_box_active,
                event_mode_active,
                enable_restored_cut_audio,
            );
            flags.active_demo_no = self
                .campaign_orchestrator
                .as_ref()
                .and_then(|o| match o.phase {
                    sh2_core::campaign::CampaignPhase::DramaDemo { demo_no, .. } => Some(demo_no),
                    _ => None,
                })
                .or_else(|| {
                    self.drama_demo_session
                        .as_ref()
                        .filter(|s| s.player.step != sh2_core::cutscene::DramaDemoStep::Finished)
                        .map(|s| u32::from(s.player.info.demo_no))
                });
            return (stage_id, flags);
        }

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
