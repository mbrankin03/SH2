//! Shared desktop application runtime state (`AppState`).

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use winit::window::Window;

use crate::assets::CharacterAsset;
use crate::rooms::audio_room::AudioDebugState;
use crate::rooms::fmv_room::ActiveFmvSession;
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
    pub sim_core: SimulationCore,
    pub gpu_map: GpuMap,
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

    /// Maps the active [`TestRoomKind`] (or user stage override) to a retail Silent Hill 2 stage ID
    /// and default [`BgmEventFlags`] so `SeBgmChange` and `SeBgmManager` stream and spatially mix
    /// the authentic stage BGM / AIX layers.
    pub fn active_bgm_stage_and_flags(&self) -> (u32, BgmEventFlags) {
        if let Some(stage_override) = self.bgm_stage_override {
            let mut flags = BgmEventFlags::default();
            if stage_override == 2 {
                flags.flag_5e4 = 0x02;
            }
            return (stage_override, flags);
        }

        let mut flags = BgmEventFlags::default();
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
            | TestRoomKind::LoadingScreenTest => 0,
        };
        (stage_id, flags)
    }
}
