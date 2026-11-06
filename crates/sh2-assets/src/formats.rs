//! Format parsers for SH2 binary files.

pub mod adx;
pub mod aix;
pub mod anm;
pub mod binary;
pub mod bink;
pub mod cam;
pub mod cld;
pub mod dxt;
pub mod map;
pub mod mdls;
pub mod mes;
pub mod sound;
pub mod texture;

pub use adx::{decode_adx, parse_adx_header, AdxHeader, DecodedAdx};
pub use anm::{
    compute_bind_locals, compute_skin_matrices, compute_skin_matrices_with_inv,
    compute_skin_matrices_with_inv_into, euler_to_matrix, evaluate_pose, evaluate_pose_into,
    interpolate_frames, mat4_inverse, mat4_mul, parse_anm, AnimationFile, AnimationFrame,
    BoneTransform,
};
pub use bink::{
    active_subtitle_for_frame, decode_bink_audio_pcm, fmv_subtitles_for_movie,
    parse_bink_file_header, parse_bink_header, BinkAudioTrack, BinkHeader, BinkVideoStream,
    DecodedFmvAudio, FmvSubtitleCue, Yuv420Frame, BINK_MAGICS,
};
pub use cam::{
    parse_cam, AreaSizeType, CamFile, CamMovementParams, CamMovementType, ChaseCamParam,
    FixAngCamParam, LocusCircleCamParam, RoadArea, RoadRecord, RoadType, SettleCamParam,
    VC_RD_CAM_LIKE_SETTLE_F, VC_RD_END_DATA_F, VC_RD_INVALID_SV_F, VC_RD_LIM_UP_FAR_VIEW_F,
    VC_RD_MARGE_ROAD_F, VC_RD_NOT_WARP_F, VC_RD_NO_EXTRA_AREA_F, VC_RD_NO_FRONT_FLIP_F,
    VC_RD_USE_NEAR_ENEMY_F, VC_RD_USE_NO_ENEMY_F, VC_RD_WARP_IN_F, VC_RD_WARP_IN_OUT_FS,
    VC_RD_WARP_OUT_F,
};
pub use cld::{parse_cld, CldFile, FloorTriangle, WallSegment};
pub use dxt::{decode_dds_file, decode_dxt};
pub use map::{parse_map, BatchGroup, MapBatch, MapFile, MapMaterial, MapTexture};
pub use mdls::{parse_mdl, ModelFile, ModelPrimitive, ModelTexture, ModelVertices};
pub use mes::{parse_mes, DialogueMessage, MessageTable};
pub use sound::{
    parse_sddata, parse_sdm_headers, parse_sdm_sdb, SdmEntryHeader, SoundBank, WavEntry,
};
pub use texture::{
    decode_texture_bytes, generate_mipmap_chain_rgba, mip_level_count_for_size,
    parse_texture_container, ContainerTexture, DecodedTexture, TextureLoader,
};
