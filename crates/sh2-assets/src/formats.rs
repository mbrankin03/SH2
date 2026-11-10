//! Format parsers for SH2 binary files.

pub mod adx;
pub mod aix;
pub mod anm;
pub mod binary;
pub mod bink;
pub mod cam;
pub mod cld;
pub mod cls;
pub mod demo;
pub mod dxt;
pub mod map;
pub mod mdls;
pub mod mes;
pub mod sound;
pub mod texture;

pub use adx::{decode_adx, parse_adx_header, AdxHeader, DecodedAdx};
pub use aix::{decode_aix, decode_aix_mixed, demux_aix};
pub use anm::{
    blend_frame_in_place, compute_bind_locals, compute_skin_matrices,
    compute_skin_matrices_with_inv, compute_skin_matrices_with_inv_into, euler_to_matrix,
    euler_to_quat, evaluate_pose, evaluate_pose_into, hermite_slerp_euler_zyx, interpolate_frames,
    interpolate_frames_hermite, interpolate_frames_hermite_into, interpolate_frames_into,
    mat4_inverse, mat4_mul, parse_anm, quat_align_hemisphere, quat_conjugate, quat_hermite_slerp,
    quat_mul, quat_normalize, quat_slerp, quat_to_euler, quat_to_matrix, slerp_euler_zyx,
    vec3_hermite_catmull_rom, AnimationFile, AnimationFrame, BoneTransform,
};
pub use bink::{
    active_subtitle_for_frame, decode_bink_audio_pcm, fmv_subtitles_for_movie,
    parse_bink_file_header, parse_bink_header, BinkAudioTrack, BinkHeader, BinkVideoStream,
    DecodedFmvAudio, FmvSubtitleCue, Yuv420Frame, BINK_MAGICS,
};
pub use cam::{
    global_area_cam_rel_path, normalize_room_cam_rel_path, parse_cam, AreaSizeType, CamFile,
    CamMovementParams, CamMovementType, CamSourceKind, ChaseCamParam, FixAngCamParam,
    LocusCircleCamParam, ResolvedCamAsset, RoadArea, RoadRecord, RoadType, SettleCamParam,
    VC_RD_CAM_LIKE_SETTLE_F, VC_RD_END_DATA_F, VC_RD_INVALID_SV_F, VC_RD_LIM_UP_FAR_VIEW_F,
    VC_RD_MARGE_ROAD_F, VC_RD_NOT_WARP_F, VC_RD_NO_EXTRA_AREA_F, VC_RD_NO_FRONT_FLIP_F,
    VC_RD_USE_NEAR_ENEMY_F, VC_RD_USE_NO_ENEMY_F, VC_RD_WARP_IN_F, VC_RD_WARP_IN_OUT_FS,
    VC_RD_WARP_OUT_F,
};
pub use cld::{parse_cld, CldFile, FloorTriangle, WallSegment};
pub use cls::{
    is_valid_cls_magic, parse_cls, parse_cls_block_at, ClsBlock, ClsFile, ClsKeyframe, ClsTrack,
    CLS_MAGIC_A, CLS_MAGIC_B, CLS_MAGIC_PROTO, MAX_FACIAL_CLUSTERS,
};
pub use demo::{
    dds_f16_to_f32, dds_rotation_to_interest, parse_demo_dds, DemoCameraOp, DemoCharOp,
    DemoDdsEvaluator, DemoDdsFile, DemoDdsHeader, DemoFrameRecord, DemoKeyframeState,
    DemoLightKind, DemoLightOp, DemoLightState, DEMO_DDS_MAGIC, DEMO_MAX_CHARS, DEMO_MAX_LIGHTS,
    DEMO_SCREEN_SY, DEMO_SCR_Z_SCALE,
};
pub use dxt::{decode_dds_file, decode_dxt, encode_dds_rgba, encode_dxt_rgba};
pub use map::{parse_map, BatchGroup, MapBatch, MapFile, MapMaterial, MapTexture};
pub use mdls::{
    parse_mdl, ModelClusterDelta, ModelClusterTarget, ModelFile, ModelPrimitive, ModelTexture,
    ModelVertices,
};
pub use mes::{parse_mes, DialogueMessage, MessageTable};
pub use sound::{
    decode_afs_entry_to_wav, decode_afs_slice_to_wav, decode_spu2_vag_to_wav, parse_afs_index,
    parse_ps2_sdb_sound_data, parse_ps2_sfc_pcm, parse_ps2_sound_dat_bank, parse_sddata,
    parse_sdm_headers, parse_sdm_sdb, parse_wav_smpl_loop, slice_wav_time_range, AfsArchiveIndex,
    AfsEntry, Ps2SdbSoundDataRecord, Ps2VagSampleHeader, SdmEntryHeader, SoundBank, WavEntry,
    AFS_MAGIC, PS2_VAG_HEAD_MAGIC, PS2_VAG_VAGI_MAGIC, PS2_VAG_VERS_MAGIC,
};
pub use texture::{
    compute_alpha_coverage_rgba, decode_texture_bytes, generate_mipmap_chain_rgba,
    mip_level_count_for_size, parse_texture_container, preserve_alpha_coverage_in_place,
    probe_tex_dimensions, ContainerTexture, CutawayTextureAsset, CutawayViewportTransform,
    DecodedTexture, TextureLoader, ALPHA_CUTOUT_MIP_REF_THRESHOLD, TEX_MAGIC, TEX_MAGIC_BYTES,
};
