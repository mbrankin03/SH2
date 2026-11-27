//! sh2-renderer: Modern wgpu/Vulkan pipeline, 32-bit float depth, volumetric compute fog.

pub mod blitter;
mod context;
pub mod effects;
pub mod fmv;
pub mod fog;
pub mod frustum;
pub mod glb_model;
pub mod gpu_map;
pub mod light;
mod lighting_ops;
pub mod mirror;
pub mod pipeline;
pub mod post;
pub mod scene;
pub mod shadow;
pub mod skeletal;
pub mod tile_pool;
pub mod water;

pub use blitter::SurfaceBlitter;
pub use context::{
    prepare_map_texture_cpu, prepare_map_texture_cpu_owned, prepare_mipmapped_texture_cpu,
    prepare_mipmapped_texture_cpu_owned, PreparedGpuTexture, RenderContext, RendererConfig,
    SkeletalDrawItem,
};
pub use fmv::{FmvUniforms, FmvVideoPipeline, GpuYuvVideoSurface};
pub use frustum::Frustum;
pub use glb_model::{
    evaluate_glb_skin_matrices, glb_vertex_to_skeletal, GlbGpuModel, GlbGpuPrimitive, GlbGpuTexture,
};
pub use gpu_map::{GpuMap, GpuMapBatch};
pub use skeletal::{
    classify_primitive_body_region, cranial_bone_mask, decode_model_textures_for_inspection,
    dilate_transparent_border_rgb, discover_pose_presets, dominant_vertex_bone,
    find_cranial_root_bone, is_default_active_pose, is_opaque_cutout_primitive,
    partition_first_person_triangles, refine_4k_chr_texture_from_sources,
    scaled_border_dilation_passes, DecodedModelTextureMap, GpuSkeletalMesh, GpuSkeletalPrimitive,
    PoseVariantPreset, SkeletalBodyRegion, SkeletalMaterialUniforms, SkeletalPassStage,
    SkeletalPipeline, SkeletalVertex, SkeletonBoneUniforms, MAX_SKELETON_BONES,
};
pub use tile_pool::GpuOutdoorTilePool;
