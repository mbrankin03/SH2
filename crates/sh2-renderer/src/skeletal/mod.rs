//! Skeletal mesh representation, PS2/PC per-primitive material shading, two-pass hair transparency,
//! and GPU Linear Blend Skinning (LBS) pipeline (`SH2-37` / `SH2-93`).

mod body_region;
pub mod cutout;
pub mod material;
mod mesh;
mod pipeline;
mod shader;
pub mod specular;

pub use body_region::{
    classify_primitive_body_region, cranial_bone_mask, dominant_vertex_bone,
    find_cranial_root_bone, partition_first_person_triangles, SkeletalBodyRegion,
};
pub use cutout::{
    decode_model_textures_for_inspection, dilate_transparent_border_rgb,
    is_opaque_cutout_primitive, refine_4k_chr_texture_from_sources, scaled_border_dilation_passes,
    DecodedModelTextureMap,
};
pub use material::{
    discover_pose_presets, is_default_active_pose, pack_aligned_material_buffer, GsAlphaTest,
    PoseVariantPreset, SkeletalMaterialUniforms, MAX_NORMALIZED_SPECULAR_COMPONENT,
    MAX_SPECULAR_SHININESS, MIN_SPECULAR_SHININESS, PC_D3DRS_ALPHAFUNC_GREATER,
    PC_D3DRS_ALPHAREF_ZERO, PS2_ALPHA_OPAQUE_THRESHOLD, PS2_ALPHA_TEST_THRESHOLD,
    PS2_GS_TEST_ALPHA_GREATER_ZERO, PS2_GS_TEST_OPAQUE_NO_ATEST, PS2_SPECULAR_COLOR_SCALE,
    SHININESS_PER_SPECULAR_SCALE,
};
pub use mesh::{
    GpuSkeletalMesh, GpuSkeletalPrimitive, SkeletalVertex, SkeletonBoneUniforms, MAX_SKELETON_BONES,
};
pub use pipeline::{SkeletalPassStage, SkeletalPipeline};
pub use specular::{
    composite_gs_specular, compute_sphere_map_uv, environ_packet_alpha,
    generate_specular_sphere_map_rgba, ENVIRON_PACKET_RGB_U8, GS_ALPHA_1_SPECULAR_DAY,
    GS_ALPHA_1_SPECULAR_NIGHT, SPECULAR_SPHERE_MAP_ALPHA_U8, SPECULAR_SPHERE_MAP_DIM,
    SPECULAR_SPHERE_MAP_EXPONENT,
};
