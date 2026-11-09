#[cfg(test)]
mod renderer_tests {
    use sh2_assets::formats::map::{BatchGroup, MapBatch, MapFile, MapMaterial};
    use sh2_renderer::*;

    fn create_synthetic_cube_map() -> MapFile {
        let mut map = MapFile::default();
        map.materials.push(MapMaterial {
            mode: 1,
            texture_id: 0,
            color: 0xFFFFFFFF,
            overlay: 0,
            specularity: 0.5,
        });

        // 8 vertices for a 1x1x1 test cube centered at origin
        let positions = vec![
            [-1.0, -1.0, 1.0],
            [1.0, -1.0, 1.0],
            [1.0, 1.0, 1.0],
            [-1.0, 1.0, 1.0],
            [-1.0, -1.0, -1.0],
            [-1.0, 1.0, -1.0],
            [1.0, 1.0, -1.0],
            [1.0, -1.0, -1.0],
        ];

        let normals = vec![
            [0.0, 0.0, 1.0],
            [0.0, 0.0, 1.0],
            [0.0, 0.0, 1.0],
            [0.0, 0.0, 1.0],
            [0.0, 0.0, -1.0],
            [0.0, 0.0, -1.0],
            [0.0, 0.0, -1.0],
            [0.0, 0.0, -1.0],
        ];

        let uvs = vec![
            [0.0, 0.0],
            [1.0, 0.0],
            [1.0, 1.0],
            [0.0, 1.0],
            [1.0, 0.0],
            [1.0, 1.0],
            [0.0, 1.0],
            [0.0, 0.0],
        ];

        let colors = vec![[255, 255, 255, 255]; 8];

        let indices = vec![
            0, 1, 2, 2, 3, 0, // Front
            1, 7, 6, 6, 2, 1, // Right
            7, 4, 5, 5, 6, 7, // Back
            4, 0, 3, 3, 5, 4, // Left
            3, 2, 6, 6, 5, 3, // Top
            4, 7, 1, 1, 0, 4, // Bottom
        ];

        map.batches.push(MapBatch {
            group: BatchGroup::Opaque,
            material_index: 0,
            geo_id: 1,
            positions,
            uvs,
            colors,
            normals,
            indices,
        });

        map
    }

    #[test]
    fn test_render_context_initialization_and_synthetic_room() {
        let config = RendererConfig {
            width: 320,
            height: 240,
            enable_hdr: true,
            enable_fog: true,
            enable_post: true,
        };

        let ctx = match RenderContext::new_headless(config) {
            Some(ctx) => ctx,
            None => {
                println!(
                    "Skipping headless render test: no suitable GPU adapter in CI environment"
                );
                return;
            }
        };

        assert_eq!(ctx.depth_format, wgpu::TextureFormat::Depth32Float);
        assert_eq!(ctx.hdr_format, wgpu::TextureFormat::Rgba16Float);

        let eye = glam::Vec3::new(0.0, 0.0, 5.0);
        let target = glam::Vec3::ZERO;
        let up = glam::Vec3::Y;
        let view = glam::Mat4::look_at_rh(eye, target, up);
        let proj =
            glam::Mat4::perspective_rh(std::f32::consts::FRAC_PI_4, 320.0 / 240.0, 0.1, 100.0);
        let view_proj = proj * view;
        let inv_proj = proj.inverse();

        ctx.update_camera(view_proj, inv_proj, eye);
        ctx.update_fog(&fog::FogUniforms::default());
        ctx.update_spotlight(&light::SpotlightUniforms::default());
        ctx.update_post(&post::PostUniforms {
            time: 1.0 / 60.0,
            grain_intensity: 0.65,
            exposure: 1.0,
            frame_count: 1,
            ..post::PostUniforms::default()
        });

        // Test with synthetic cube geometry (always available in CI)
        let cube_map = create_synthetic_cube_map();
        let gpu_map = GpuMap::from_map(&ctx.device, &cube_map);
        assert_eq!(gpu_map.batches.len(), 1);
        assert_eq!(gpu_map.batches[0].num_indices, 36);

        ctx.render_gpu_map(&gpu_map);
        let _ = ctx.device.poll(wgpu::Maintain::wait());

        // Test with real map file if present on disk
        let path =
            "Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2/data/bg/rr/rr21.map";
        if let Ok(bytes) = std::fs::read(path) {
            let map = MapFile::parse(&bytes).expect("Failed to parse rr21.map");
            let real_gpu_map = GpuMap::from_map(&ctx.device, &map);
            ctx.render_gpu_map(&real_gpu_map);
            let _ = ctx.device.poll(wgpu::Maintain::wait());
        }

        // Test with real character model and skeletal animation pipeline (SH2-8)
        let mdl_path =
            "Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2/data/chr/jms/hll_jms.mdl";
        let anm_path =
            "Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2/data/chr/jms/jms_basic1.anm";

        if let (Ok(mdl_bytes), Ok(anm_bytes)) = (std::fs::read(mdl_path), std::fs::read(anm_path)) {
            use sh2_assets::formats::anm::{
                compute_bind_locals, compute_skin_matrices, evaluate_pose, parse_anm,
            };
            use sh2_assets::formats::mdls::parse_mdl;

            let model = parse_mdl(&mdl_bytes).expect("Failed to parse hll_jms.mdl");
            let anm = parse_anm(&anm_bytes, model.bone_count, &model.parents)
                .expect("Failed to parse jms_basic1.anm");

            assert_eq!(anm.bone_count, model.bone_count);
            assert!(!anm.frames.is_empty());

            let gpu_skeletal = ctx.compile_skeletal_mesh(&model);
            assert!(gpu_skeletal.total_indices > 0);

            // Verify pose variant initialization (James hll_jms has 31 prims, but inactive hand variants are omitted)
            let active_prims: Vec<_> = gpu_skeletal
                .primitives
                .iter()
                .filter(|p| p.is_active)
                .collect();
            assert_eq!(
                active_prims.len(),
                13,
                "Expected 13 active primitives (base mesh + default open hands)"
            );
            let inactive_prims: Vec<_> = gpu_skeletal
                .primitives
                .iter()
                .filter(|p| !p.is_active)
                .collect();
            assert_eq!(
                inactive_prims.len(),
                18,
                "Expected 18 inactive hand pose variants"
            );

            let locals = compute_bind_locals(&model.parents, &model.initial_matrices);
            let pose0 = evaluate_pose(&model.parents, &locals, &anm.frames[0]);
            let skin_matrices = compute_skin_matrices(&pose0, &model.initial_matrices);

            // Execute scene render pass containing both static world and skinned character
            ctx.render_scene(
                Some(&gpu_map),
                None,
                &[(&gpu_skeletal, &skin_matrices, None)],
            );
            let _ = ctx.device.poll(wgpu::Maintain::wait());

            // Test SH2-36 Model & Texture Viewer modes: Bone Skeleton + Wireframe + Clay/Normals shaders
            let world_id = [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ];
            let skel_lines =
                sh2_debug::build_skeleton_debug_lines(&model.parents, &pose0, &world_id, 45.0);
            assert!(!skel_lines.is_empty());

            let tris = model.primitive_triangles(0);
            let wire_lines = sh2_debug::build_wireframe_lines(
                &model.vertices.positions,
                &model.vertices.bone_indices,
                &model.vertices.weights,
                &skin_matrices,
                &tris,
                [0.15, 0.92, 0.65, 1.0],
            );
            assert!(!wire_lines.is_empty());

            // Render Untextured Clay (mode 1.0) and Surface Normals (mode 2.0) with debug lines
            for mode in [1.0f32, 2.0f32, 3.0f32] {
                ctx.update_camera_with_mode(view_proj, inv_proj, eye, mode);
                ctx.render_scene_with_debug_lines(
                    Some(&gpu_map),
                    None,
                    &[(&gpu_skeletal, &skin_matrices, None)],
                    &skel_lines,
                );
                let _ = ctx.device.poll(wgpu::Maintain::wait());
            }
        }

        // Test with real Maria model and animation (SH2-12)
        let mar_mdl_path =
            "Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2/data/chr/mar/lll_mar.mdl";
        let mar_anm_path =
            "Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2/data/chr/mar/lll_mar.anm";

        if let (Ok(mdl_bytes), Ok(anm_bytes)) =
            (std::fs::read(mar_mdl_path), std::fs::read(mar_anm_path))
        {
            use sh2_assets::formats::anm::{
                compute_bind_locals, compute_skin_matrices, evaluate_pose, parse_anm,
            };
            use sh2_assets::formats::mdls::parse_mdl;

            let model = parse_mdl(&mdl_bytes).expect("Failed to parse lll_mar.mdl");
            let anm = parse_anm(&anm_bytes, model.bone_count, &model.parents)
                .expect("Failed to parse lll_mar.anm");

            assert_eq!(anm.bone_count, model.bone_count);
            assert!(!anm.frames.is_empty());

            let gpu_skeletal = ctx.compile_skeletal_mesh(&model);
            assert!(gpu_skeletal.total_indices > 0);

            // Verify pose variant initialization (James hll_jms has 31 prims, but inactive hand variants are omitted)
            let active_prims: Vec<_> = gpu_skeletal
                .primitives
                .iter()
                .filter(|p| p.is_active)
                .collect();
            assert_eq!(
                active_prims.len(),
                13,
                "Expected 13 active primitives (base mesh + default open hands)"
            );
            let inactive_prims: Vec<_> = gpu_skeletal
                .primitives
                .iter()
                .filter(|p| !p.is_active)
                .collect();
            assert_eq!(
                inactive_prims.len(),
                18,
                "Expected 18 inactive hand pose variants"
            );

            let locals = compute_bind_locals(&model.parents, &model.initial_matrices);
            let pose0 = evaluate_pose(&model.parents, &locals, &anm.frames[0]);
            let skin_matrices = compute_skin_matrices(&pose0, &model.initial_matrices);

            ctx.render_scene(
                Some(&gpu_map),
                None,
                &[(&gpu_skeletal, &skin_matrices, None)],
            );
            let _ = ctx.device.poll(wgpu::Maintain::wait());

            // Test SH2-36 Model & Texture Viewer modes: Bone Skeleton + Wireframe + Clay/Normals shaders
            let world_id = [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ];
            let skel_lines =
                sh2_debug::build_skeleton_debug_lines(&model.parents, &pose0, &world_id, 45.0);
            assert!(!skel_lines.is_empty());

            let tris = model.primitive_triangles(0);
            let wire_lines = sh2_debug::build_wireframe_lines(
                &model.vertices.positions,
                &model.vertices.bone_indices,
                &model.vertices.weights,
                &skin_matrices,
                &tris,
                [0.15, 0.92, 0.65, 1.0],
            );
            assert!(!wire_lines.is_empty());

            // Render Untextured Clay (mode 1.0) and Surface Normals (mode 2.0) with debug lines
            for mode in [1.0f32, 2.0f32, 3.0f32] {
                ctx.update_camera_with_mode(view_proj, inv_proj, eye, mode);
                ctx.render_scene_with_debug_lines(
                    Some(&gpu_map),
                    None,
                    &[(&gpu_skeletal, &skin_matrices, None)],
                    &skel_lines,
                );
                let _ = ctx.device.poll(wgpu::Maintain::wait());
            }
        }

        // Test animated post-processing update over sequential frames
        for frame in 1..=5 {
            ctx.update_post(&post::PostUniforms {
                time: frame as f32 / 60.0,
                grain_intensity: 0.8,
                exposure: 1.0,
                frame_count: frame,
                ..post::PostUniforms::default()
            });
            ctx.render_gpu_map(&gpu_map);
            let _ = ctx.device.poll(wgpu::Maintain::wait());
        }

        // Test SH2-10: Planar YUV420p FMV Cutscene Pipeline & 16x Anisotropic Mipmapped Textures
        let fmv_pipeline = FmvVideoPipeline::new(&ctx.device, RenderContext::OUTPUT_FORMAT);
        let yuv_surface = fmv_pipeline.create_video_surface(&ctx.device, 320, 240);
        let synth_frame =
            sh2_assets::formats::bink::Yuv420Frame::generate_synthetic_test_frame(320, 240, 12);
        fmv_pipeline.upload_frame(&ctx.queue, &yuv_surface, &synth_frame);

        let mut fmv_encoder = ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Headless FMV Test Encoder"),
            });
        let fmv_uniforms = FmvUniforms::default();
        fmv_pipeline.update_uniforms(&ctx.queue, &fmv_uniforms);
        fmv_pipeline.render(&mut fmv_encoder, &yuv_surface, &ctx.ldr_output_view);
        ctx.queue.submit(std::iter::once(fmv_encoder.finish()));
        let _ = ctx.device.poll(wgpu::Maintain::wait());

        // Test mipmapped anisotropic texture bind group creation
        let rgba_64 = vec![180u8; 64 * 64 * 4];
        let mip_bg = ctx.create_mipmapped_texture_bind_group(
            64,
            64,
            &rgba_64,
            Some("Test 64x64 Mipmapped Anisotropic Texture"),
        );
        ctx.render_scene(Some(&gpu_map), Some(std::slice::from_ref(&mip_bg)), &[]);
        let _ = ctx.device.poll(wgpu::Maintain::wait());

        // Test SH2-70 & SH2-72: Multi-pass Opaque/Cutout, Coplanar Decal (depth-biased), and Transparent pipelines
        let mut multi_group_map = create_synthetic_cube_map();
        let base_batch = multi_group_map.batches[0].clone();
        multi_group_map.batches.push(MapBatch {
            group: BatchGroup::Decal,
            material_index: 0,
            geo_id: 2,
            ..base_batch.clone()
        });
        multi_group_map.batches.push(MapBatch {
            group: BatchGroup::Transparent,
            material_index: 0,
            geo_id: 3,
            ..base_batch
        });

        let gpu_multi = GpuMap::from_map(&ctx.device, &multi_group_map);
        assert_eq!(gpu_multi.batches.len(), 3);
        assert_eq!(gpu_multi.batches[0].group, BatchGroup::Opaque);
        assert!(!gpu_multi.batches[0].is_transparent);
        assert_eq!(gpu_multi.batches[1].group, BatchGroup::Decal);
        assert!(!gpu_multi.batches[1].is_transparent);
        assert_eq!(gpu_multi.batches[2].group, BatchGroup::Transparent);
        assert!(gpu_multi.batches[2].is_transparent);

        ctx.render_gpu_map(&gpu_multi);
        ctx.render_scene(Some(&gpu_multi), Some(&[mip_bg]), &[]);
        let _ = ctx.device.poll(wgpu::Maintain::wait());

        // Test SH2-105: Room 312 Sunset ambient tint, Rm 307 Closet darkened preset, and PS2 DoF + Motion Blur GPU compute passes
        ctx.update_spotlight(&light::SpotlightUniforms {
            ambient_tint: [1.0, 1.0, 227.0 / 255.0, 0.55],
            ..light::SpotlightUniforms::default()
        });
        ctx.update_post(&post::PostUniforms {
            time: 0.5,
            grain_intensity: 0.65,
            exposure: 1.0,
            frame_count: 15,
            dof_tap_offset_px: [1.5, -1.5],
            dof_strength: 0.85,
            motion_blur_strength: 0.60,
        });
        ctx.render_gpu_map(&gpu_multi);
        let _ = ctx.device.poll(wgpu::Maintain::wait());
    }
}
