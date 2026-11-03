#[cfg(test)]
mod renderer_tests {
    use sh2_renderer::*;
    use sh2_assets::formats::map::MapFile;

    #[test]
    fn test_render_context_initialization_and_map_execution() {
        let config = RendererConfig {
            width: 320,
            height: 240,
            enable_hdr: true,
            enable_fog: true,
            enable_post: true,
        };

        let ctx = RenderContext::new_headless(config);
        assert_eq!(ctx.depth_format, wgpu::TextureFormat::Depth32Float);
        assert_eq!(ctx.hdr_format, wgpu::TextureFormat::Rgba16Float);

        // Test with real map file if available
        let path = "Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2/data/bg/rr/rr21.map";
        if let Ok(bytes) = std::fs::read(path) {
            let map = MapFile::parse(&bytes).expect("Failed to parse rr21.map");

            // Setup camera matrices
            let eye = glam::Vec3::new(-100500.0, -800.0, -60000.0);
            let target = glam::Vec3::new(-100500.0, -900.0, -60400.0);
            let up = glam::Vec3::Y;
            let view = glam::Mat4::look_at_rh(eye, target, up);
            let proj = glam::Mat4::perspective_rh(
                std::f32::consts::FRAC_PI_4,
                320.0 / 240.0,
                10.0,
                50000.0,
            );
            let view_proj = proj * view;
            let inv_proj = proj.inverse();

            ctx.update_camera(view_proj, inv_proj, eye);
            ctx.update_fog(&fog::FogUniforms::default());
            ctx.update_spotlight(&light::SpotlightUniforms::default());
            ctx.update_post(&post::PostUniforms::default());

            // Run render loop
            let start = std::time::Instant::now();
            ctx.render_map(&map);
            let _ = ctx.device.poll(wgpu::Maintain::wait());
            let duration = start.elapsed();
            println!("Map render + compute fog pass completed in: {:?}", duration);

            // Verify execution was well within frame budget
            assert!(duration.as_millis() < 500);
        }
    }
}
