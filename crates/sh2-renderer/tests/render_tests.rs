#[cfg(test)]
mod renderer_tests {
    use sh2_assets::formats::map::{BatchGroup, MapBatch, MapFile, MapMaterial, MapVertex};
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
        let vertices = vec![
            MapVertex {
                position: [-1.0, -1.0, 1.0],
                normal: [0.0, 0.0, 1.0],
                uv: [0.0, 0.0],
                color: [1.0, 1.0, 1.0, 1.0],
            },
            MapVertex {
                position: [1.0, -1.0, 1.0],
                normal: [0.0, 0.0, 1.0],
                uv: [1.0, 0.0],
                color: [1.0, 1.0, 1.0, 1.0],
            },
            MapVertex {
                position: [1.0, 1.0, 1.0],
                normal: [0.0, 0.0, 1.0],
                uv: [1.0, 1.0],
                color: [1.0, 1.0, 1.0, 1.0],
            },
            MapVertex {
                position: [-1.0, 1.0, 1.0],
                normal: [0.0, 0.0, 1.0],
                uv: [0.0, 1.0],
                color: [1.0, 1.0, 1.0, 1.0],
            },
            MapVertex {
                position: [-1.0, -1.0, -1.0],
                normal: [0.0, 0.0, -1.0],
                uv: [1.0, 0.0],
                color: [1.0, 1.0, 1.0, 1.0],
            },
            MapVertex {
                position: [-1.0, 1.0, -1.0],
                normal: [0.0, 0.0, -1.0],
                uv: [1.0, 1.0],
                color: [1.0, 1.0, 1.0, 1.0],
            },
            MapVertex {
                position: [1.0, 1.0, -1.0],
                normal: [0.0, 0.0, -1.0],
                uv: [0.0, 1.0],
                color: [1.0, 1.0, 1.0, 1.0],
            },
            MapVertex {
                position: [1.0, -1.0, -1.0],
                normal: [0.0, 0.0, -1.0],
                uv: [0.0, 0.0],
                color: [1.0, 1.0, 1.0, 1.0],
            },
        ];

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
            material_id: 0,
            geo_id: 1,
            vertices,
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
        ctx.update_post(&post::PostUniforms::default());

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
    }
}
