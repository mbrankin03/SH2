//! Skeletal mesh representation, PS2/PC per-primitive material shading, two-pass hair transparency,
//! and GPU Linear Blend Skinning (LBS) pipeline (`SH2-37` / `SH2-93`).

mod body_region;
pub mod cutout;
pub mod material;
mod mesh;
mod pipeline;
mod shader;

pub use body_region::{
    classify_primitive_body_region, cranial_bone_mask, dominant_vertex_bone,
    find_cranial_root_bone, partition_first_person_triangles, SkeletalBodyRegion,
};
pub use cutout::{
    decode_model_textures_for_inspection, dilate_transparent_border_rgb,
    is_opaque_cutout_primitive, refine_4k_chr_texture_from_sources, DecodedModelTextureMap,
};
pub use material::{
    discover_pose_presets, is_default_active_pose, pack_aligned_material_buffer, PoseVariantPreset,
    SkeletalMaterialUniforms, MAX_NORMALIZED_SPECULAR_COMPONENT, MAX_SPECULAR_SHININESS,
    MIN_SPECULAR_SHININESS, PS2_SPECULAR_COLOR_SCALE, SHININESS_PER_SPECULAR_SCALE,
    TRANSPARENT_CORE_ALPHA_THRESHOLD,
};
pub use mesh::{
    GpuSkeletalMesh, GpuSkeletalPrimitive, SkeletalVertex, SkeletonBoneUniforms, MAX_SKELETON_BONES,
};
pub use pipeline::{SkeletalPassStage, SkeletalPipeline};
