//! sh2-audio: High-definition streaming sound engine using kira.

pub mod beta_audio_catalog;
pub mod bgm_data;
pub mod bgm_manager;
pub mod cutscene_bgm;
pub mod engine;
pub mod menu_sfx;
pub mod sfx_bank;

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
    apply_music_box_volume_patch, decode_bgm_layers_from_disk, eval_stage_ev_bgm_control,
    evaluate_situation_page, evaluate_sound_areas, evaluate_sound_areas_with_restoration,
    extract_record_track_fades, pick_bgm_page, BgmEventFlags, BgmManager, BgmTrackState,
    BgmTrackStatus, SoundAreaEvaluation, BGM_MAX_TRACKS, ENEMY_NEAR_DIST_SQ, FADE_PRIO,
    FADE_TIME_SECS, LOW_HEALTH_RATIO_THRESHOLD, MAX_MATCHED_SOUND_AREAS, SITUATION_PRESETS_6_TO_15,
    SOUND_AREA_HALF_WIDTH,
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
