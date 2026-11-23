#![forbid(unsafe_code)]
//! sh2-audio: High-definition streaming sound engine using kira.

pub mod acoustics;
pub mod beta_audio_catalog;
pub mod bgm_data;
pub mod bgm_manager;
pub mod chainsaw_sfx;
pub mod cutscene_bgm;
pub mod device_routing;
pub mod dialogue;
pub mod dsp_resample;
pub mod engine;
pub mod hrtf;
pub mod looping;
pub mod menu_sfx;
pub mod occlusion;
pub mod ringbuffer;
pub mod se_manager;
pub mod sfx_bank;
pub mod spatial_voice;

pub use acoustics::{
    evaluate_room_acoustics, evaluate_room_acoustics_for_pos,
    evaluate_room_acoustics_for_pos_gated, evaluate_room_acoustics_gated, EnvironmentalPreset,
    RoomAcousticsProfile,
};
pub use beta_audio_catalog::{
    CutAudioRestoreAction, CutAudioRestorer, CutSfxCatalogEntry, CutSfxCategory,
    LauraLetterVoiceSlice, Ps2PrototypeSdbSummary, Ps2PrototypeSfcEntry, CS_APT_RPT_CLOSET_ID,
    CS_HSP_BOSS_FINISH_ID, CS_HTL_LAURA_PIANO_ID, CUT_AND_RARE_AUDIO_CATALOG,
    E3_PROTOTYPE_SDB_CATALOG, E3_TGS_TRIAL_SDB_SAMPLE_RECORDS, LAURA_LETTER_VOICEOVER_SLICES,
    PS2_PROTOTYPE_SFC_CATALOG, R_HTL_RESTAURANT_ID, UNREFERENCED_SDDATA_ENTRIES,
    VOICE_AFS_BORN_FROM_A_WISH_TSUIKA_INDEX, VOICE_AFS_LAURA_LETTER_INDEX,
    VOICE_AFS_MARIA_DEAI_TSUIKA_INDEX, VOICE_AFS_MARY_LETTER_FULL_INDEX, VOICE_AFS_SATSUGAI_INDEX,
};
pub use bgm_data::{
    bgm_grid_cell, bgm_page_entry, bgm_room_id, find_bgm_file_by_load_id, stage_glb_crd,
    BgmFileEntry, BgmPageEntry, SoundDataRecord, BGM_FILES, BGM_PAGES, STAGE_GLB_CRD,
    STAGE_SOUND_DATA,
};
pub use bgm_manager::{
    apply_music_box_volume_patch, bgm_ducking_mode_from_game_state, decode_bgm_layers_from_disk,
    eval_stage_ev_bgm_control, evaluate_situation_page, evaluate_sound_areas,
    evaluate_sound_areas_with_restoration, extract_record_track_fades, is_bgm_demo_duck_exempt,
    pick_bgm_page, step_global_bgm_fade, BgmDuckingMode, BgmEventFlags, BgmManager, BgmTrackState,
    BgmTrackStatus, SoundAreaEvaluation, BGM_DUCK_DEMO, BGM_DUCK_NORMAL, BGM_DUCK_PAUSED,
    BGM_DUCK_RAMP_DIVISOR, BGM_MAX_TRACKS, ENEMY_NEAR_DIST_SQ, FADE_PRIO, FADE_TIME_SECS,
    LOW_HEALTH_RATIO_THRESHOLD, MAX_MATCHED_SOUND_AREAS, SITUATION_PRESETS_6_TO_15,
    SOUND_AREA_HALF_WIDTH,
};
pub use chainsaw_sfx::{
    compute_chainsaw_wav_slice, ChainsawAudioOutput, ChainsawIdleState, ChainsawSoundController,
    ChainsawTickInput, ChainsawWavSlice, CHAINSAW_IDLE_FADE_DURATION_SECS,
    CHAINSAW_IDLE_FADE_START_TIME_SECS, CHAINSAW_MIN_ACTIVE_VOLUME, CRANK_LEFT_SIDE_TRIM_BYTES_48K,
    IDLE_LOOP_TRIM_BYTES_48K, IDLE_LOOP_TRIM_BYTES_ORIGINAL, SAMPLE_RATE_48KHZ, SE_CHAINSAW_ATTACK,
    SE_CHAINSAW_CRANK, SE_CHAINSAW_HIT, SE_CHAINSAW_IDLE_LOOP, SE_CHAINSAW_IDLE_START,
    WAV_RIFF_HEADER_BYTES,
};
pub use cutscene_bgm::{
    find_cutscene_audio_mapping, master_volume_for_start_id, resolve_bgm_stream_path,
    resolve_bgm_stream_path_for_source, CutsceneAudioMapping, CutsceneBgmAction,
    CUTSCENE_AUDIO_MAP,
};
pub use dialogue::ActiveDialogue;
pub use dsp_resample::{adx_to_frames_48k, wav_entry_to_frames, wav_entry_to_frames_48k};
pub use engine::{AudioEngine, AudioError};
pub use hrtf::{
    angles_from_azimuth_elevation, compute_listener_spherical_angles,
    compute_woodworth_itd_seconds, BinauralHrtfProcessor, HrtfSphericalAngles, HEAD_RADIUS_METERS,
    SPEED_OF_SOUND_MPS,
};
pub use looping::{
    apply_loop_boundary_crossfade, measure_loop_boundary_step_delta, LoopRegionSamples,
    DEFAULT_LOOP_CROSSFADE_SAMPLES_48K,
};
pub use menu_sfx::{play_menu_sound, MenuAudioBank, SoundCueKind};
pub use occlusion::{
    db_to_linear, evaluate_controller_occlusion, evaluate_occlusion_dsp, evaluate_world_occlusion,
    OcclusionDspParams, OcclusionFilterState, OCCLUDED_MAX_ATTENUATION_DB, OCCLUDED_MAX_CUTOFF_HZ,
    OCCLUDED_MIN_ATTENUATION_DB, OCCLUDED_MIN_CUTOFF_HZ, OCCLUSION_SMOOTH_TIME_SECS,
    UNOCCLUDED_CUTOFF_HZ,
};
pub use ringbuffer::{
    pack_stereo_frame, unpack_stereo_frame, AudioRingBuffer, DEFAULT_RINGBUFFER_FRAMES,
};
pub use se_manager::{
    Se2d3dMapping, Se2dSlot, Se3dChannel, SeBackendAction, SeCallResult, SeListenerContext,
    SePoolManager, SE_2D_SLEW_RATE_PER_SEC, SE_2D_SLOT_COUNT, SE_3D_CHANNEL_COUNT,
    SE_3D_ID_THRESHOLD, SE_3D_PRIORITY1_MAX, SE_FLAG_MANAGED_2D, SE_FLAG_NO_DISTANCE,
    SE_FLAG_PRIO_1,
};
pub use sfx_bank::{
    build_ps2_wvs_matrix, compute_se_call_pos_distance_f, compute_se_call_pos_spatial,
    compute_sfx_distance_param_v, footstep_sddata_indices, resolve_se_id_to_sddata_index,
    resolve_se_id_to_voice_afs_index, se_call_pos_direction, se_call_pos_distance,
    se_call_pos_distance_f, SfxSpatialEval, SDDATA_ENTRY_COUNT, SE_DIR_CAMERA_X_OFFSET_MM,
    SE_DIR_PAN_SCALE_DEG, SE_ID_TO_SDDATA_INDEX, SE_ID_TO_VOICE_AFS_INDEX, SFX_DISTANCE_INV_SPAN,
    SFX_DISTANCE_SPAN, SFX_MAX_DISTANCE, SFX_MIN_DISTANCE,
};
pub use spatial_voice::{
    SpatialDrySoundData, SpatialDryWetCore, SpatialVoiceControl, SpatialVoiceHandle,
    SpatialVoiceInitParams, SpatialWetSendCore, SpatialWetSendSoundData,
};
