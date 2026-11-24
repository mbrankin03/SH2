//! Interactive Loading Screen Test Chamber (`TestRoomKind::LoadingScreenTest`, `SH2-75`).
//!
//! Allows live inspection and parity verification of both:
//! - Retail PC `sh2pc.exe` footstep loading screen (`FUN_00406ab0`, `FUN_00406be0`,
//!   `FUN_004c8500..0x004c8bc0`) across all 8 `LoadingPathPattern` edge-entry trajectories, and
//! - PS2 prototype `SLUS_202.28` `now_loading.c` (`NowLoadingDraw` `0x00299170`) 64-frame
//!   jittered `"Now loading"` text mode.
//!
//! Controls:
//! - `[Left]` / `[Right]` or `[A]` / `[D]`: Cycle all 8 `LoadingPathPattern` trajectories (`0..=7`)
//! - `[M]` / `[Tab]`: Toggle Retail PC Footsteps vs PS2 `NowLoadingDraw` text jitter mode
//! - `[Up]` / `[Down]` or `[+]` / `[-]`: Adjust footstep cadence (`0.25s..=2.50s`, default `1.00s`)
//! - `[S]`: Cycle footstep surface material SFX (8 materials)
//! - `[V]`: Toggle footstep SFX mute
//! - `[R]` / `[Space]` / `[Enter]`: Restart transition immediately
//! - `[H]`: Toggle telemetry HUD overlay (for clean 1:1 visual inspection)
//! - `[Esc]` / `[Backspace]`: Return to Test Rooms menu

use std::time::Duration;
use winit::keyboard::KeyCode;

use crate::rooms::RoomController;
use crate::state::AppState;
use sh2_core::SurfaceMaterial;
use sh2_ui::loading::FOOTSTEP_CADENCE_SECS;
use sh2_ui::menu::TitlePhase;
use sh2_ui::{
    AtlasFont, FootstepSprite, LoadingPathPattern, LoadingScreen, LoadingScreenMode, Sh2TextColor,
};

/// Ordered 8 floor surface materials for interactive footstep SFX previewing.
pub(crate) const SURFACE_MATERIALS: [SurfaceMaterial; 8] = [
    SurfaceMaterial::Concrete,
    SurfaceMaterial::Wood,
    SurfaceMaterial::Metal,
    SurfaceMaterial::Carpet,
    SurfaceMaterial::Water,
    SurfaceMaterial::Grass,
    SurfaceMaterial::Gravel,
    SurfaceMaterial::Blood,
];

/// Interactive runtime state for [`sh2_ui::menu::TestRoomKind::LoadingScreenTest`] (`SH2-75`).
#[derive(Debug, Clone)]
pub(crate) struct LoadingScreenTestState {
    pub loading_screen: LoadingScreen,
    pub pattern_index: usize,
    pub mode: LoadingScreenMode,
    pub cadence_secs: f32,
    pub material_index: usize,
    pub sfx_enabled: bool,
    pub show_hud: bool,
    pub restart_count: u32,
    pub last_sfx_trigger_count: u32,
}

impl Default for LoadingScreenTestState {
    fn default() -> Self {
        let mut state = Self {
            loading_screen: LoadingScreen::new_with_seed(
                "Loading Screen Preview",
                3600.0,
                false,
                0,
            ),
            pattern_index: 0,
            mode: LoadingScreenMode::RetailPcFootsteps,
            cadence_secs: FOOTSTEP_CADENCE_SECS,
            material_index: 0,
            // 1:1 Retail Parity (`SLUS_202.28` `now_loading.c` & `sh2pc.exe` `FUN_00406be0` /
            // `FUN_004c8bc0`): loading screens are silent by default; `[V]` toggles preview SFX.
            sfx_enabled: false,
            show_hud: true,
            restart_count: 0,
            last_sfx_trigger_count: 0,
        };
        state.loading_screen.step_cadence = state.cadence_secs;
        state
    }
}

impl LoadingScreenTestState {
    /// Returns the currently selected [`LoadingPathPattern`].
    #[must_use]
    pub fn current_pattern(&self) -> LoadingPathPattern {
        LoadingPathPattern::from_index(self.pattern_index)
    }

    /// Returns the currently selected [`SurfaceMaterial`] for footstep SFX.
    #[must_use]
    pub fn current_material(&self) -> SurfaceMaterial {
        SURFACE_MATERIALS[self.material_index % SURFACE_MATERIALS.len()]
    }

    /// Cycles the active [`LoadingPathPattern`] (`delta = +1` or `-1`) and restarts the preview.
    pub fn cycle_pattern(&mut self, delta: isize) {
        let len = LoadingPathPattern::ALL.len() as isize;
        self.pattern_index = ((self.pattern_index as isize + delta).rem_euclid(len)) as usize;
        self.restart_transition();
    }

    /// Toggles between [`LoadingScreenMode::RetailPcFootsteps`] and
    /// [`LoadingScreenMode::Ps2NowLoadingJitter`], restarting the preview cleanly.
    pub fn toggle_mode(&mut self) {
        self.mode = self.mode.toggle();
        self.restart_transition();
    }

    /// Adjusts footstep cadence in seconds (`0.25s..=2.50s`), updating the live loading screen.
    pub fn adjust_cadence(&mut self, delta_secs: f32) {
        self.cadence_secs = (self.cadence_secs + delta_secs).clamp(0.25, 2.50);
        self.loading_screen.step_cadence = self.cadence_secs;
    }

    /// Cycles through the 8 acoustic [`SurfaceMaterial`] presets.
    pub fn cycle_material(&mut self, delta: isize) {
        let len = SURFACE_MATERIALS.len() as isize;
        self.material_index = ((self.material_index as isize + delta).rem_euclid(len)) as usize;
    }

    /// Rebuilds the preview [`LoadingScreen`] with current mode, pattern, and cadence.
    pub fn restart_transition(&mut self) {
        self.restart_count = self.restart_count.wrapping_add(1);
        let pattern = self.current_pattern();
        let mut ls = LoadingScreen::new_with_seed(
            "Loading Screen Preview",
            3600.0,
            false,
            self.pattern_index,
        )
        .with_pattern(pattern)
        .with_mode(self.mode);
        ls.step_cadence = self.cadence_secs;
        self.loading_screen = ls;
    }

    /// Advances the preview loading screen by `dt_sec`, returning `true` when a footstep SFX
    /// should be triggered.
    pub fn tick(&mut self, dt_sec: f32) -> bool {
        self.loading_screen.tick(dt_sec);
        if self.loading_screen.step_spawned_this_tick && self.sfx_enabled {
            self.last_sfx_trigger_count = self.last_sfx_trigger_count.wrapping_add(1);
            true
        } else {
            false
        }
    }
}

pub(crate) struct LoadingScreenTestRoomController;

impl RoomController for LoadingScreenTestRoomController {
    fn handle_key_input(state: &mut AppState, key_code: KeyCode, pressed: bool, repeat: bool) {
        if !pressed || repeat {
            return;
        }
        match key_code {
            KeyCode::Escape | KeyCode::Backspace => {
                state.exit_room_to_menu(TitlePhase::TestRoomsSelect);
                tracing::info!("Returned from LoadingScreenTest room to Test Rooms menu.");
            }
            KeyCode::ArrowRight | KeyCode::KeyD => {
                state.loading_screen_test.cycle_pattern(1);
            }
            KeyCode::ArrowLeft | KeyCode::KeyA => {
                state.loading_screen_test.cycle_pattern(-1);
            }
            KeyCode::KeyM | KeyCode::Tab => {
                state.loading_screen_test.toggle_mode();
            }
            KeyCode::ArrowUp | KeyCode::KeyW | KeyCode::Equal | KeyCode::NumpadAdd => {
                state.loading_screen_test.adjust_cadence(0.25);
            }
            KeyCode::ArrowDown | KeyCode::Minus | KeyCode::NumpadSubtract => {
                state.loading_screen_test.adjust_cadence(-0.25);
            }
            KeyCode::KeyS => {
                state.loading_screen_test.cycle_material(1);
            }
            KeyCode::KeyV => {
                state.loading_screen_test.sfx_enabled = !state.loading_screen_test.sfx_enabled;
            }
            KeyCode::KeyR | KeyCode::Space | KeyCode::Enter | KeyCode::NumpadEnter => {
                state.loading_screen_test.restart_transition();
            }
            KeyCode::KeyH => {
                state.loading_screen_test.show_hud = !state.loading_screen_test.show_hud;
            }
            _ => {}
        }
    }

    fn tick_and_render_3d(
        state: &mut AppState,
        _dt: Duration,
        dt_sec: f32,
        _aspect: f32,
        _grain_pct: u32,
        is_paused: bool,
        _is_in_game: bool,
    ) {
        if !is_paused && state.loading_screen_test.tick(dt_sec) {
            let material = state.loading_screen_test.current_material();
            crate::frame::trigger_loading_footstep_sfx(state, material);
        }
    }

    fn render_hud(
        state: &mut AppState,
        encoder: &mut wgpu::CommandEncoder,
        surface_view: &wgpu::TextureView,
    ) {
        // 1. Clear full widescreen swapchain to pitch black (`#000000`).
        // Explicit `drop` ends the render pass before `finish_inventory_overlay_in_viewport`
        // opens `Surface Viewport Blit Pass` on the same `CommandEncoder` (wgpu 23 locks the
        // encoder until the `RenderPass` is dropped).
        drop(encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("LoadingScreenTest Clear Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: surface_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        }));

        // Sync SH2:EE HD (256×256) vs strict 1:1 Vanilla (64×64) footmark sprite from Options.
        let use_hd_footmark = state
            .menu_ctrl
            .options_menu
            .original_config
            .video
            .texture_quality
            != sh2_core::WorldTextureQuality::OriginalPs2;
        state.loading_screen_test.loading_screen.use_hd_footmark = use_hd_footmark;

        // 2. Render full-brightness LoadingScreen in centered 4:3 PS2 UI viewport
        let loading_alpha = state.loading_screen_test.loading_screen.fade_alpha;
        let tint = [1.0f32, 1.0, 1.0, loading_alpha];
        state.render_ctx.queue.write_buffer(
            &state.menu_uniform_buffer,
            0,
            bytemuck::cast_slice(&tint),
        );
        state.inventory_rasterizer.clear();
        let font = AtlasFont::shared(None);
        state
            .loading_screen_test
            .loading_screen
            .render(font, &mut state.inventory_rasterizer);
        let vp = state.ps2_ui_viewport();
        state.finish_inventory_overlay_in_viewport(encoder, surface_view, vp);

        // 3. Optional interactive telemetry HUD overlay (`[H]` to toggle)
        if !state.loading_screen_test.show_hud {
            return;
        }
        // Avoid overwriting `menu_uniform_buffer` before GPU submission: when the loading pass
        // used a non-opaque alpha, flush it on its own submission before writing the opaque HUD
        // tint; otherwise reuse `[1.0, 1.0, 1.0, 1.0]` without a second `write_buffer`.
        if (loading_alpha - 1.0).abs() > f32::EPSILON {
            let next_encoder =
                state
                    .render_ctx
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("LoadingScreenTest HUD Encoder"),
                    });
            let prev_encoder = std::mem::replace(encoder, next_encoder);
            state
                .render_ctx
                .queue
                .submit(std::iter::once(prev_encoder.finish()));
            state.begin_hud_overlay();
        } else {
            state.menu_rasterizer.clear();
        }
        let test = &state.loading_screen_test;
        let ls = &test.loading_screen;
        let sprite = FootstepSprite::get_for_hd(ls.use_hd_footmark);
        let sprite_src = if sprite.is_hd_override {
            "SH2:EE HD sh2e/effect/footmark.tbn2"
        } else if sprite.from_retail_asset {
            "retail data/etc/effect/footmark.tbn2"
        } else {
            "procedural 64x64 fallback"
        };

        let mut lines = vec![
            (
                "LOADING SCREEN TEST CHAMBER (SH2-75)".to_string(),
                Sh2TextColor::AmberHeader,
            ),
            (format!("Mode [M]: {}", test.mode.label()), Sh2TextColor::Cyan),
            (
                format!("Pattern [Left/Right]: {}", test.current_pattern().label()),
                Sh2TextColor::White,
            ),
            (
                format!(
                    "Cadence [Up/Down]: {:.2}s (Retail=1.00s) | Decay: 4.00s | Active Prints: {} | Steps: {}",
                    test.cadence_secs,
                    ls.footsteps.len(),
                    ls.step_counter
                ),
                Sh2TextColor::White,
            ),
            (
                format!(
                    "Footstep SFX [S/V]: {:?} ({}) | Triggered: {} | Sprite: {} ({}x{})",
                    test.current_material(),
                    if test.sfx_enabled {
                        "ON (Preview)"
                    } else {
                        "OFF (1:1 Silent)"
                    },
                    test.last_sfx_trigger_count,
                    sprite_src,
                    sprite.width,
                    sprite.height
                ),
                Sh2TextColor::Green,
            ),
        ];

        if test.mode == LoadingScreenMode::Ps2NowLoadingJitter {
            lines.push((
                format!(
                    "PS2 NowLoadingDraw (0x299170): _count_51={} | XY=({}, {}) | Alpha={:.2}",
                    ls.ps2_frame_counter, ls.ps2_text_x_512, ls.ps2_text_y_480, ls.ps2_text_alpha
                ),
                Sh2TextColor::GoldSelected,
            ));
        }

        lines.push((
            "[Left/Right] Pattern  [M] Mode  [Up/Down] Cadence  [S] Surface  [V] SFX  [R/Space] Restart  [H] HUD  [Esc] Back"
                .to_string(),
            Sh2TextColor::MutedFooter,
        ));

        let mut y = 18usize;
        for (line, color) in lines {
            font.draw_line_styled(&mut state.menu_rasterizer, &line, (20, y), 18, color, true);
            y += 22;
        }
        state.finish_hud_overlay(encoder, surface_view);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_loading_screen_test_state_controls() {
        let mut state = LoadingScreenTestState::default();
        assert_eq!(state.current_pattern(), LoadingPathPattern::TopEdgeRight);
        assert_eq!(state.mode, LoadingScreenMode::RetailPcFootsteps);
        assert_eq!(state.cadence_secs, FOOTSTEP_CADENCE_SECS);
        assert_eq!(state.current_material(), SurfaceMaterial::Concrete);

        // Cycle through all 8 patterns and wrap around
        for i in 1..=8 {
            state.cycle_pattern(1);
            assert_eq!(state.current_pattern(), LoadingPathPattern::from_index(i));
        }
        assert_eq!(state.current_pattern(), LoadingPathPattern::TopEdgeRight);

        // Reverse wrap
        state.cycle_pattern(-1);
        assert_eq!(state.current_pattern(), LoadingPathPattern::LeftEdgeUp);

        // Toggle PS2 jitter mode and back
        state.toggle_mode();
        assert_eq!(state.mode, LoadingScreenMode::Ps2NowLoadingJitter);
        assert_eq!(
            state.loading_screen.mode,
            LoadingScreenMode::Ps2NowLoadingJitter
        );
        // No footstep SFX should fire in PS2 jitter mode
        assert!(!state.tick(0.033));

        state.toggle_mode();
        assert_eq!(state.mode, LoadingScreenMode::RetailPcFootsteps);
        // Default is 1:1 retail silent (`sfx_enabled == false`): no SFX emitted on stamp
        assert!(!state.sfx_enabled);
        assert!(!state.tick(0.016));
        assert_eq!(state.last_sfx_trigger_count, 0);

        // Enabling preview SFX (`[V]`) emits SFX on subsequent footprint stamps
        state.sfx_enabled = true;
        assert!(state.tick(1.0));
        assert_eq!(state.last_sfx_trigger_count, 1);

        // Disabling suppresses SFX emission again
        state.sfx_enabled = false;
        assert!(!state.tick(1.0));
        assert_eq!(state.last_sfx_trigger_count, 1);

        // Cadence clamping and surface material cycling
        state.adjust_cadence(-10.0);
        assert_eq!(state.cadence_secs, 0.25);
        state.adjust_cadence(10.0);
        assert_eq!(state.cadence_secs, 2.50);

        state.cycle_material(1);
        assert_eq!(state.current_material(), SurfaceMaterial::Wood);
        let prev_restarts = state.restart_count;
        state.restart_transition();
        assert_eq!(state.restart_count, prev_restarts + 1);
    }
}
