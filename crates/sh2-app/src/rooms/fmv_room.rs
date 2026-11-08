//! 1080p60 FMV Cutscene Player & Theater room controller (`SH2-10`).

use std::path::PathBuf;
use std::time::Duration;
use winit::keyboard::KeyCode;

use crate::rooms::RoomController;
use crate::state::AppState;
use sh2_assets::formats::bink::{decode_bink_audio_track, BinkHeader, BinkVideoStream};
use sh2_audio::AudioEngine;
use sh2_debug::{FmvCatalogEntry, FmvVideoSource, FmvViewerState, FMV_CATALOG};
use sh2_renderer::{FmvUniforms, FmvVideoPipeline, GpuYuvVideoSurface};
use sh2_ui::menu::TitlePhase;

/// Active decoded FMV movie stream, GPU planar YUV surface, and metadata.
pub(crate) struct ActiveFmvSession {
    pub entry: &'static FmvCatalogEntry,
    pub header: BinkHeader,
    pub stream: BinkVideoStream,
    pub gpu_surface: GpuYuvVideoSurface,
    #[allow(dead_code)]
    pub is_hd_source: bool,
    pub frame_accumulator_sec: f32,
    /// Cached decoded audio (sample rate, s16le PCM) so loops/restarts can replay it.
    pub audio: Option<(u32, Vec<u8>)>,
    /// Cutscene paired background music (ADX/AIX decoded data, retail volume, fade) to replay on restart.
    pub bgm_stream: Option<(sh2_assets::DecodedAdx, f32, std::time::Duration)>,
}

/// Locate and open a `.bik` movie stream for the requested [`FmvCatalogEntry`] and source preference.
#[allow(clippy::too_many_arguments)]
pub(crate) fn open_fmv_session(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    fmv_pipeline: &FmvVideoPipeline,
    audio_engine: &mut Option<AudioEngine>,
    fmv_viewer: &mut FmvViewerState,
    entry: &'static FmvCatalogEntry,
    source_pref: FmvVideoSource,
    audio_pack_source: sh2_core::AudioPackSource,
) -> Option<ActiveFmvSession> {
    let upscaled_mp4_roots = ["tools/vsr_pipeline/output", "movie", "sh2e/movie"];

    let resolver = sh2_assets::AssetPathResolver::resolve();
    let base_roots = [
        resolver.game_root.to_string_lossy().to_string(),
        "Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2".to_string(),
    ];

    let og_hq_roots = ["sh2_enhanced_edition_og_hq_fmvs-50-1-0-1770228966/movie"];

    // MP4 candidate filenames for locally upscaled VSR masters (e.g. `toilet_4k_master.mp4`, `toilet.mp4`)
    let mp4_cands = [
        format!("{}_4k_master.mp4", entry.id),
        format!("{}.mp4", entry.id),
    ];

    let mut chosen_path: Option<(PathBuf, bool)> = None;
    match source_pref {
        FmvVideoSource::Upscaled4kMp4 => {
            for dir in &upscaled_mp4_roots {
                for mp4_name in &mp4_cands {
                    let cand = PathBuf::from(dir).join(mp4_name);
                    if cand.exists() {
                        chosen_path = Some((cand, true));
                        break;
                    }
                }
                if chosen_path.is_some() {
                    break;
                }
            }
        }
        FmvVideoSource::OgHqPs2Masters => {
            for dir in &og_hq_roots {
                let cand = PathBuf::from(dir).join(entry.filename);
                if cand.exists() {
                    chosen_path = Some((cand, true));
                    break;
                }
            }
        }
        FmvVideoSource::Enhanced1080p60 => {
            for base in &base_roots {
                let sh2e_cand = PathBuf::from(base).join("sh2e/movie").join(entry.filename);
                if sh2e_cand.exists() {
                    chosen_path = Some((sh2e_cand, true));
                    break;
                }
            }
        }
        FmvVideoSource::VanillaPc => {
            for base in &base_roots {
                let data_cand = PathBuf::from(base).join("data/movie").join(entry.filename);
                if data_cand.exists() {
                    chosen_path = Some((data_cand, false));
                    break;
                }
            }
        }
    }

    // Graceful fallback chain if user preference is missing for this cutscene
    if chosen_path.is_none() {
        for dir in &upscaled_mp4_roots {
            for mp4_name in &mp4_cands {
                let cand = PathBuf::from(dir).join(mp4_name);
                if cand.exists() {
                    chosen_path = Some((cand, true));
                    break;
                }
            }
            if chosen_path.is_some() {
                break;
            }
        }
    }
    if chosen_path.is_none() {
        for dir in &og_hq_roots {
            let cand = PathBuf::from(dir).join(entry.filename);
            if cand.exists() {
                chosen_path = Some((cand, true));
                break;
            }
        }
    }
    if chosen_path.is_none() {
        for base in &base_roots {
            let sh2e_cand = PathBuf::from(base).join("sh2e/movie").join(entry.filename);
            if sh2e_cand.exists() {
                chosen_path = Some((sh2e_cand, true));
                break;
            }
        }
    }
    if chosen_path.is_none() {
        for base in &base_roots {
            let data_cand = PathBuf::from(base).join("data/movie").join(entry.filename);
            if data_cand.exists() {
                chosen_path = Some((data_cand, false));
                break;
            }
        }
    }

    let synth_header = BinkHeader {
        magic: *b"BIKi",
        file_size: 1920 * 1080 * 300,
        frame_count: 600,
        largest_frame_size: 65536,
        width: 1920,
        height: 1080,
        fps_num: 2997,
        fps_den: 50,
        video_flags: 0,
        audio_tracks: Vec::new(),
        indexed_frames: 600,
    };

    let mut fmv_audio: Option<(u32, Vec<u8>)> = None;
    let mut cutscene_bgm_data: Option<(sh2_assets::DecodedAdx, f32, std::time::Duration)> = None;
    let (mut stream, is_hd_source) = if let Some((ref path, is_hd)) = chosen_path {
        match BinkVideoStream::open(path) {
            Ok(s) => {
                tracing::info!(
                    "[FMV Player] Opened Bink movie '{}' ({}) -> {}x{} @ {:.2} fps ({} frames, synthetic={})",
                    entry.display_name,
                    path.display(),
                    s.header.width,
                    s.header.height,
                    s.header.fps(),
                    s.header.frame_count,
                    s.path.is_none()
                );
                if let Some(ref mut engine) = audio_engine {
                    // sh2e murder.bik: track 1 is the restored uncut VHS audio (Mary), matching
                    // the Enhancements VHSAudio patch when `enable_restored_cut_audio` is true;
                    // open.bik track 1 is the voiced mix.
                    let default_track = fmv_viewer.default_audio_track_for(entry.filename);
                    let track = fmv_viewer.audio_track.unwrap_or(default_track);
                    tracing::info!(
                        "[FMV Player] Audio track {} of {}",
                        track,
                        s.header.audio_tracks.len()
                    );
                    match decode_bink_audio_track(path, &s.header, track) {
                        Ok(decoded) if !decoded.pcm_s16le.is_empty() => {
                            if let Err(e) =
                                engine.play_fmv_pcm(decoded.sample_rate, &decoded.pcm_s16le, 1.0)
                            {
                                tracing::warn!("[FMV Player] Audio playback warning: {:?}", e);
                            }
                            fmv_audio = Some((decoded.sample_rate, decoded.pcm_s16le));
                        }
                        _ => {
                            engine.stop_fmv_audio(std::time::Duration::from_millis(50));
                        }
                    }

                    // Handle paired cutscene BGM streaming (SH2-39 CRI ADX / AIX containers)
                    if let Some(cutscene_map) =
                        sh2_audio::cutscene_bgm::find_cutscene_audio_mapping(entry.id)
                    {
                        match cutscene_map.bgm_action {
                            sh2_audio::cutscene_bgm::CutsceneBgmAction::StartNew => {
                                if let Some(rel_bgm) = cutscene_map.bgm_file {
                                    let mut resolved_bgm: Option<PathBuf> = None;
                                    for base in &base_roots {
                                        if let Some(p) =
                                            sh2_audio::cutscene_bgm::resolve_bgm_stream_path_for_source(
                                                base,
                                                rel_bgm,
                                                audio_pack_source,
                                            )
                                        {
                                            resolved_bgm = Some(p);
                                            break;
                                        }
                                    }
                                    if let Some(full_bgm_path) = resolved_bgm {
                                        if let Ok(bytes) = std::fs::read(&full_bgm_path) {
                                            let decoded_res = if rel_bgm.ends_with(".aix") {
                                                sh2_assets::formats::aix::decode_aix_mixed(&bytes)
                                            } else {
                                                sh2_assets::decode_adx(&bytes)
                                            };
                                            match decoded_res {
                                                Ok(adx) => {
                                                    tracing::info!(
                                                        "[FMV Player] Playing cutscene BGM '{}' (vol {:.2}, fade {:?})",
                                                        rel_bgm,
                                                        cutscene_map.retail_master_volume,
                                                        cutscene_map.fade_in
                                                    );
                                                    if let Err(e) = engine.play_bgm_with_fade(
                                                        &adx,
                                                        cutscene_map.retail_master_volume,
                                                        cutscene_map.fade_in,
                                                    ) {
                                                        tracing::warn!(
                                                            "[FMV Player] BGM playback warning: {:?}",
                                                            e
                                                        );
                                                    }
                                                    cutscene_bgm_data = Some((
                                                        adx,
                                                        cutscene_map.retail_master_volume,
                                                        cutscene_map.fade_in,
                                                    ));
                                                }
                                                Err(e) => {
                                                    tracing::warn!(
                                                        "[FMV Player] Failed to decode cutscene BGM '{}': {:?}",
                                                        rel_bgm,
                                                        e
                                                    );
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            sh2_audio::cutscene_bgm::CutsceneBgmAction::KeepActive => {
                                tracing::info!(
                                    "[FMV Player] Cutscene '{}' keeps active ambient BGM (retail parity)",
                                    entry.id
                                );
                            }
                            sh2_audio::cutscene_bgm::CutsceneBgmAction::Silence => {
                                tracing::info!(
                                    "[FMV Player] Cutscene '{}' silences BGM track (self-contained mix)",
                                    entry.id
                                );
                                engine.stop_bgm(cutscene_map.fade_out);
                            }
                        }
                    }
                }
                (s, is_hd)
            }
            Err(e) => {
                tracing::warn!(
                    "[FMV Player] Failed to parse Bink header at {:?}: {:?}; using synthetic 1080p60 stream",
                    path,
                    e
                );
                (
                    BinkVideoStream::new_synthetic(synth_header.clone()),
                    source_pref != FmvVideoSource::VanillaPc,
                )
            }
        }
    } else {
        tracing::info!(
            "[FMV Player] Movie '{}' not found on disk; launching synthetic 1080p60 test stream",
            entry.filename
        );
        if let Some(ref mut engine) = audio_engine {
            engine.stop_bgm(std::time::Duration::from_millis(50));
        }
        (
            BinkVideoStream::new_synthetic(synth_header),
            source_pref != FmvVideoSource::VanillaPc,
        )
    };

    let header = stream.header.clone();
    fmv_viewer.configure_stream_meta(
        header.width,
        header.height,
        header.frame_count,
        header.fps(),
    );
    let gpu_surface = fmv_pipeline.create_video_surface(device, header.width, header.height);

    // Decode and upload initial frame 0 immediately so the first render pass shows video
    if let Some(first_frame) = stream.next_frame() {
        fmv_pipeline.upload_frame(queue, &gpu_surface, first_frame);
    }

    Some(ActiveFmvSession {
        entry,
        header,
        stream,
        gpu_surface,
        is_hd_source,
        frame_accumulator_sec: 0.0,
        audio: fmv_audio,
        bgm_stream: cutscene_bgm_data,
    })
}

pub(crate) struct FmvRoomController;

impl RoomController for FmvRoomController {
    fn handle_key_input(state: &mut AppState, key_code: KeyCode, pressed: bool, _repeat: bool) {
        if !pressed {
            return;
        }
        match key_code {
            KeyCode::Escape | KeyCode::Backspace => {
                if let Some(ref mut engine) = state.audio_engine {
                    engine.stop_bgm(std::time::Duration::from_millis(120));
                    engine.stop_fmv_audio(std::time::Duration::from_millis(120));
                }
                state.fmv_session = None;
                state.menu_ctrl.in_game_active = false;
                state.menu_ctrl.title_sm.phase = TitlePhase::OptionsMenu;
                state.menu_ctrl.title_sm.fade_alpha = 1.0;
                tracing::info!("Returned from FMV Cutscene Player to Options menu.");
            }
            KeyCode::KeyN | KeyCode::PageDown | KeyCode::ArrowRight | KeyCode::KeyD => {
                state.fmv_viewer.audio_track = None;
                state.fmv_viewer.next_movie();
                tracing::info!(
                    "[FMV Player] Selected cutscene [{}/{}]: {}",
                    state.fmv_viewer.selected_movie_index + 1,
                    FMV_CATALOG.len(),
                    state.fmv_viewer.current_entry().display_name
                );
            }
            KeyCode::KeyP | KeyCode::PageUp | KeyCode::ArrowLeft | KeyCode::KeyA => {
                state.fmv_viewer.audio_track = None;
                state.fmv_viewer.prev_movie();
                tracing::info!(
                    "[FMV Player] Selected cutscene [{}/{}]: {}",
                    state.fmv_viewer.selected_movie_index + 1,
                    FMV_CATALOG.len(),
                    state.fmv_viewer.current_entry().display_name
                );
            }
            KeyCode::Space => {
                state.fmv_viewer.toggle_pause();
                if let Some(ref mut engine) = state.audio_engine {
                    engine.set_bgm_paused(state.fmv_viewer.is_paused);
                }
                tracing::info!(
                    "[FMV Player] Playback {}",
                    if state.fmv_viewer.is_paused {
                        "PAUSED"
                    } else {
                        "PLAYING"
                    }
                );
            }
            KeyCode::KeyT => {
                let n = state
                    .fmv_session
                    .as_ref()
                    .map(|x| x.header.audio_tracks.len())
                    .unwrap_or(1)
                    .max(1);
                let cur = state.fmv_viewer.audio_track.unwrap_or(
                    state
                        .fmv_viewer
                        .default_audio_track_for(state.fmv_viewer.current_entry().filename),
                );
                state.fmv_viewer.audio_track = Some((cur + 1) % n);
                state.fmv_viewer.is_paused = false;
                let entry = state.fmv_viewer.current_entry();
                let source_pref = state.fmv_viewer.video_source;
                let audio_pack = state.bgm_manager.audio_pack_source;
                state.fmv_session = open_fmv_session(
                    &state.render_ctx.device,
                    &state.render_ctx.queue,
                    &state.fmv_pipeline,
                    &mut state.audio_engine,
                    &mut state.fmv_viewer,
                    entry,
                    source_pref,
                    audio_pack,
                );
            }
            KeyCode::KeyU => {
                state.fmv_viewer.sharpen_level = (state.fmv_viewer.sharpen_level + 1) % 4;
                tracing::info!(
                    "[FMV Player] Upscale sharpening level: {}",
                    state.fmv_viewer.sharpen_level
                );
            }
            KeyCode::KeyR => {
                state.fmv_viewer.is_paused = false;
                let entry = state.fmv_viewer.current_entry();
                let source_pref = state.fmv_viewer.video_source;
                let audio_pack = state.bgm_manager.audio_pack_source;
                state.fmv_session = open_fmv_session(
                    &state.render_ctx.device,
                    &state.render_ctx.queue,
                    &state.fmv_pipeline,
                    &mut state.audio_engine,
                    &mut state.fmv_viewer,
                    entry,
                    source_pref,
                    audio_pack,
                );
                tracing::info!(
                    "[FMV Player] Restarted cutscene: {}",
                    state.fmv_viewer.current_entry().display_name
                );
            }
            KeyCode::KeyH => {
                state.fmv_viewer.toggle_hd_source();
                tracing::info!(
                    "[FMV Player] Switched video source preference: {}",
                    if state.fmv_viewer.prefer_hd_source {
                        "SH2:EE 1080p60 (sh2e/movie/)"
                    } else {
                        "Vanilla PC 30fps (data/movie/)"
                    }
                );
            }
            KeyCode::KeyV => {
                state.fmv_viewer.aspect_mode = state.fmv_viewer.aspect_mode.next();
                tracing::info!(
                    "[FMV Player] Aspect ratio mode: {}",
                    state.fmv_viewer.aspect_mode.label()
                );
            }
            KeyCode::KeyG => {
                state.fmv_viewer.film_grain_enabled = !state.fmv_viewer.film_grain_enabled;
                tracing::info!(
                    "[FMV Player] Film grain over cutscene: {}",
                    state.fmv_viewer.film_grain_enabled
                );
            }
            KeyCode::KeyS => {
                state.fmv_viewer.subtitles_enabled = !state.fmv_viewer.subtitles_enabled;
                tracing::info!(
                    "[FMV Player] Cutscene subtitles: {}",
                    state.fmv_viewer.subtitles_enabled
                );
            }
            KeyCode::Tab => {
                state.fmv_viewer.show_hud = !state.fmv_viewer.show_hud;
            }
            _ => {}
        }
    }

    fn tick_and_render_3d(
        state: &mut AppState,
        _dt: Duration,
        dt_sec: f32,
        aspect: f32,
        grain_pct: u32,
        _is_paused: bool,
        _is_in_game: bool,
    ) {
        if let Some(ref mut session) = state.fmv_session {
            let fps = (session.header.fps() as f32).max(1.0);
            let frame_duration = 1.0 / fps;
            if !state.fmv_viewer.is_paused {
                session.frame_accumulator_sec += dt_sec.min(0.1);
                let mut advanced = 0;
                while session.frame_accumulator_sec >= frame_duration && advanced < 4 {
                    session.frame_accumulator_sec -= frame_duration;
                    advanced += 1;
                    if let Some(frame) = session.stream.next_frame() {
                        state.fmv_pipeline.upload_frame(
                            &state.render_ctx.queue,
                            &session.gpu_surface,
                            frame,
                        );
                    } else {
                        // Loop seamlessly on completion in theater mode
                        session.stream.restart();
                        if let Some(engine) = state.audio_engine.as_mut() {
                            if let Some((sr, pcm)) = session.audio.as_ref() {
                                let _ = engine.play_fmv_pcm(*sr, pcm, 1.0);
                            }
                            if let Some((adx, vol, fade)) = session.bgm_stream.as_ref() {
                                let _ = engine.play_bgm_with_fade(adx, *vol, *fade);
                            }
                        }
                        if let Some(frame) = session.stream.next_frame() {
                            state.fmv_pipeline.upload_frame(
                                &state.render_ctx.queue,
                                &session.gpu_surface,
                                frame,
                            );
                        }
                        break;
                    }
                }
            }

            let (uv_y_offset, uv_y_scale, video_aspect) =
                state.fmv_viewer.compute_uv_crop_and_aspect();
            let fmv_grain = if state.fmv_viewer.film_grain_enabled {
                ((grain_pct as f32) / 100.0) * 0.45
            } else {
                0.0
            };
            let fmv_uniforms = FmvUniforms {
                viewport_aspect: aspect,
                video_aspect,
                uv_y_offset,
                uv_y_scale,
                grain_intensity: fmv_grain,
                time: state.sim_time,
                fade_alpha: 1.0,
                sharpen: match state.fmv_viewer.sharpen_level {
                    0 => 0.0,
                    1 => 0.35,
                    2 => 0.7,
                    _ => 1.0,
                },
            };
            state
                .fmv_pipeline
                .update_uniforms(&state.render_ctx.queue, &fmv_uniforms);

            let mut fmv_encoder =
                state
                    .render_ctx
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("FMV Render Encoder"),
                    });
            state.fmv_pipeline.render(
                &mut fmv_encoder,
                &session.gpu_surface,
                &state.render_ctx.ldr_output_view,
            );
            state
                .render_ctx
                .queue
                .submit(std::iter::once(fmv_encoder.finish()));
        }
    }

    fn render_hud(
        state: &mut AppState,
        encoder: &mut wgpu::CommandEncoder,
        surface_view: &wgpu::TextureView,
    ) {
        state.begin_hud_overlay();

        let white = [235, 235, 235, 255];
        let yellow = [245, 210, 65, 255];
        let cyan = [105, 215, 235, 255];
        let green = [120, 230, 150, 255];
        let muted = [165, 165, 170, 255];

        if let Some(ref session) = state.fmv_session {
            let cur_frame = session.stream.current_frame_index.saturating_sub(1);
            let total_frames = session.header.frame_count.max(1);
            let fps = session.header.fps();
            let cur_sec = (cur_frame as f64) / fps.max(1.0);
            let tot_sec = session.header.duration_secs();

            // Render synchronized subtitle cue near bottom of frame
            if state.fmv_viewer.subtitles_enabled {
                let movie_path = session.stream.path.as_deref();
                if let Some(sub_line) = sh2_assets::formats::bink::active_subtitle_text(
                    session.entry.id,
                    &session.header,
                    cur_frame,
                    movie_path,
                ) {
                    let game_root =
                        movie_path.and_then(sh2_assets::formats::bink::game_root_from_movie_path);
                    let w = state.menu_rasterizer.width;
                    sh2_ui::draw_subtitle(
                        &mut state.menu_rasterizer,
                        game_root.as_deref(),
                        &sub_line,
                        w / 2,
                        645,
                        32,
                        white,
                    );
                }
            }

            if state.fmv_viewer.show_hud {
                let total_movies = FMV_CATALOG.len();
                let cur_idx = state.fmv_viewer.selected_movie_index + 1;
                let src_label = if session.stream.path.is_none() {
                    "SYNTHETIC 1080P60 STREAM"
                } else {
                    state.fmv_viewer.video_source.label()
                };
                let play_str = if state.fmv_viewer.is_paused {
                    "PAUSED"
                } else {
                    "PLAYING"
                };

                state.menu_rasterizer.draw_text(
                    "SILENT HILL 2 - 1080P60 FMV CUTSCENE THEATER [SH2-10]",
                    24,
                    18,
                    2,
                    yellow,
                    true,
                );
                state.menu_rasterizer.draw_text(
                    &format!(
                        "CUTSCENE [{}/{}] : {} ({}) - {}",
                        cur_idx,
                        total_movies,
                        session.entry.display_name,
                        session.entry.filename,
                        session.entry.description
                    ),
                    24,
                    46,
                    2,
                    white,
                    true,
                );
                let audio_label = state.fmv_viewer.audio_track_label(
                    session.entry.filename,
                    session.header.audio_tracks.len().max(1),
                );
                let sharpen_label = match state.fmv_viewer.sharpen_level {
                    0 => "OFF",
                    1 => "SOFT (0.35)",
                    2 => "MEDIUM (0.70)",
                    _ => "STRONG (1.00)",
                };

                state.menu_rasterizer.draw_text(
                    &format!(
                        "STREAM : {} | {}x{} @ {:.2} FPS | {} | {:05.1}s / {:05.1}s (FRAME {}/{})",
                        src_label,
                        session.header.width,
                        session.header.height,
                        fps,
                        play_str,
                        cur_sec,
                        tot_sec,
                        cur_frame,
                        total_frames
                    ),
                    24,
                    74,
                    1,
                    green,
                    true,
                );
                state.menu_rasterizer.draw_text(
                    &format!("AUDIO  : {}", audio_label),
                    24,
                    92,
                    1,
                    yellow,
                    true,
                );
                let cutscene_map =
                    sh2_audio::cutscene_bgm::find_cutscene_audio_mapping(session.entry.id);
                let bgm_desc = cutscene_map.map(|m| m.description).unwrap_or("None");
                let bgm_file = cutscene_map.and_then(|m| m.bgm_file);
                let bgm_label = state.fmv_viewer.bgm_stream_label(bgm_file, bgm_desc);
                state.menu_rasterizer.draw_text(
                    &format!("MUSIC  : {}", bgm_label),
                    24,
                    108,
                    1,
                    yellow,
                    true,
                );
                state.menu_rasterizer.draw_text(
                    &format!(
                        "ASPECT : {} | CAS SHARPEN : {} | GRAIN : {} | SUBS : {}",
                        state.fmv_viewer.aspect_mode.label(),
                        sharpen_label,
                        if state.fmv_viewer.film_grain_enabled {
                            "ON"
                        } else {
                            "OFF"
                        },
                        if state.fmv_viewer.subtitles_enabled {
                            "ON"
                        } else {
                            "OFF"
                        }
                    ),
                    24,
                    126,
                    1,
                    cyan,
                    true,
                );
                state.menu_rasterizer.draw_text(
                    "[N/P or Left/Right] Movie  [Space] Pause  [R] Restart  [H] Source (1080p60/OG-HQ/PC)  [T] Audio Track  [U] Sharpen  [V] Aspect  [G] Grain  [S] Subs  [Tab] HUD  [Esc] Options",
                    24,
                    688,
                    1,
                    muted,
                    true,
                );
            }
        }

        state.finish_hud_overlay(encoder, surface_view);
    }
}
