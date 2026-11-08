//! sh2-audio: High-definition streaming sound engine using kira.

pub mod bgm_data;
pub mod bgm_manager;
pub mod cutscene_bgm;
pub mod engine;
pub mod menu_sfx;
pub mod sfx_bank;

pub use bgm_data::{
    bgm_grid_cell, bgm_page_entry, bgm_room_id, find_bgm_file_by_load_id, stage_glb_crd,
    BgmFileEntry, BgmPageEntry, SoundDataRecord, BGM_FILES, BGM_PAGES, STAGE_GLB_CRD,
    STAGE_SOUND_DATA,
};
pub use bgm_manager::{
    decode_bgm_layers_from_disk, eval_stage_ev_bgm_control, evaluate_situation_page,
    evaluate_sound_areas, extract_record_track_fades, pick_bgm_page, BgmEventFlags, BgmManager,
    BgmTrackState, BgmTrackStatus, SoundAreaEvaluation, BGM_MAX_TRACKS, ENEMY_NEAR_DIST_SQ,
    FADE_PRIO, FADE_TIME_SECS, LOW_HEALTH_RATIO_THRESHOLD, MAX_MATCHED_SOUND_AREAS,
    SITUATION_PRESETS_6_TO_15, SOUND_AREA_HALF_WIDTH,
};
pub use cutscene_bgm::{
    find_cutscene_audio_mapping, master_volume_for_start_id, resolve_bgm_stream_path,
    resolve_bgm_stream_path_for_source, CutsceneAudioMapping, CutsceneBgmAction,
    CUTSCENE_AUDIO_MAP,
};
pub use engine::{ActiveDialogue, AudioEngine, AudioError, EnvironmentalPreset};
pub use menu_sfx::{play_menu_sound, MenuAudioBank, SoundCueKind};
pub use sfx_bank::{
    compute_ps2_sfx_attenuation_gain, compute_se_call_pos_distance_f, compute_sfx_distance_param_v,
    footstep_sddata_indices, resolve_se_id_to_sddata_index, resolve_se_id_to_voice_afs_index,
    SfxSpatialEval, SDDATA_ENTRY_COUNT, SE_ID_TO_SDDATA_INDEX, SE_ID_TO_VOICE_AFS_INDEX,
    SFX_DISTANCE_SPAN, SFX_MAX_DISTANCE, SFX_MIN_DISTANCE,
};
