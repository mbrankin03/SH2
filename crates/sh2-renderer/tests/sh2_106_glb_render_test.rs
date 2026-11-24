use glam::Mat4;
use sh2_assets::formats::{
    GlbMesh, GlbModel, GlbNode, GlbPrimitive, GlbSkin, GlbTexture, GlbVertex,
};
use sh2_renderer::{evaluate_glb_skin_matrices, GlbGpuModel, RenderContext, RendererConfig};

fn make_test_glb_model(alpha_blend: bool) -> GlbModel {
    let vertices = vec![
        GlbVertex {
            position: [0.0, 0.0, 0.0],
            normal: [0.0, 0.0, 1.0],
            uv: [0.0, 0.0],
            joints: [0, 0, 0, 0],
            weights: [1.0, 0.0, 0.0, 0.0],
        },
        GlbVertex {
            position: [100.0, 0.0, 0.0],
            normal: [0.0, 0.0, 1.0],
            uv: [1.0, 0.0],
            joints: [0, 0, 0, 0],
            weights: [1.0, 0.0, 0.0, 0.0],
        },
        GlbVertex {
            position: [0.0, 100.0, 0.0],
            normal: [0.0, 0.0, 1.0],
            uv: [0.0, 1.0],
            joints: [0, 0, 0, 0],
            weights: [1.0, 0.0, 0.0, 0.0],
        },
    ];
    GlbModel {
        meshes: vec![GlbMesh {
            primitives: vec![GlbPrimitive {
                vertices,
                indices: vec![0, 1, 2],
                texture_index: Some(0),
                alpha_blend,
            }],
        }],
        nodes: vec![GlbNode {
            mesh_index: Some(0),
            skin_index: Some(0),
            matrix: None,
            translation: [10.0, 20.0, 30.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: [1.0, 1.0, 1.0],
            children: Vec::new(),
        }],
        root_nodes: vec![0],
        skins: vec![GlbSkin {
            joints: vec![0],
            inverse_bind_matrices: vec![Mat4::IDENTITY.to_cols_array()],
        }],
        anim_tracks: Vec::new(),
        anim_duration_sec: 0.0,
        textures: vec![GlbTexture {
            width: 1,
            height: 1,
            rgba: vec![255, 200, 128, 255],
        }],
    }
}

#[test]
fn glb_skin_matrix_cycle_guard_and_invalid_skin_handling() {
    let mut model = make_test_glb_model(false);
    // Invalid skin index must return empty slice, not a 1-element fallback
    assert!(evaluate_glb_skin_matrices(&model, 99, 0.0, true).is_empty());

    // Self-referential cycle must terminate cleanly without stack overflow
    model.nodes[0].children.push(0);
    let mats = evaluate_glb_skin_matrices(&model, 0, 0.0, true);
    assert_eq!(mats.len(), 1);
    assert!((mats[0].w_axis.x - 10.0).abs() < 1e-4);
}

#[test]
fn glb_gpu_model_renders_opaque_and_alpha_blend_passes() {
    let Some(ctx) = RenderContext::new_headless(RendererConfig {
        width: 64,
        height: 64,
        ..RendererConfig::default()
    }) else {
        return;
    };

    let opaque = GlbGpuModel::from_glb(&ctx, make_test_glb_model(false), "ty3_test");
    let alpha = GlbGpuModel::from_glb(&ctx, make_test_glb_model(true), "mon_test");
    opaque.update_skin_matrices(&ctx.queue, Mat4::IDENTITY, 0.25, true);
    alpha.update_skin_matrices(&ctx.queue, Mat4::IDENTITY, 0.5, true);

    let mut encoder = ctx
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("sh2_106_glb_render_test"),
        });
    {
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("glb_hdr_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &ctx.hdr_color_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &ctx.depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        opaque.draw(&mut rpass, &ctx, &ctx.geometry_bind_group, false);
        alpha.draw(&mut rpass, &ctx, &ctx.geometry_bind_group, true);
    }
    ctx.queue.submit(Some(encoder.finish()));
}
